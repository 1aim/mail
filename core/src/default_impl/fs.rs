use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    marker::PhantomData,
    path::{Path, PathBuf},
};

use checked_command::CheckedCommand;
use failure::Fail;
use futures::IntoFuture;

use headers::header_components::{FileMeta, MediaType};

use crate::{
    context::{Context, MaybeEncData, ResourceLoaderComponent},
    error::{ResourceLoadingError, ResourceLoadingErrorKind},
    iri::IRI,
    resource::{Data, Metadata, Source, UseMediaType},
    utils::{ConstSwitch, Enabled, SendBoxFuture},
};

// have a scheme ignoring variant for Mux as the scheme is preset
// allow a setup with different scheme path/file etc. the behavior stays the same!
// do not handle sandboxing/security as such do not handle "file" only "path" ~use open_at if available?~

//TODO more doc
/// By setting SchemeValidation to Disabled the FsResourceLoader can be used to simple
/// load a resource from a file based on a scheme tail as path independent of the rest,
/// so e.g. it it is used in a `Mux` which selects a `ResourceLoader` impl based on a scheme
/// the scheme would not be double validated.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FsResourceLoader<SchemeValidation: ConstSwitch = Enabled> {
    root: PathBuf,
    scheme: &'static str,
    _marker: PhantomData<SchemeValidation>,
}

impl<SVSw> FsResourceLoader<SVSw>
where
    SVSw: ConstSwitch,
{
    const DEFAULT_SCHEME: &'static str = "path";

    /// create a new file system based FileLoader, which will  "just" standard _blocking_ IO
    /// to read a file from the file system into a buffer
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self::new_with_scheme(root.into(), Self::DEFAULT_SCHEME)
    }

    pub fn new_with_scheme<P: Into<PathBuf>>(root: P, scheme: &'static str) -> Self {
        FsResourceLoader {
            root: root.into(),
            scheme,
            _marker: PhantomData,
        }
    }

    pub fn with_cwd_root() -> Result<Self, io::Error> {
        let cwd = env::current_dir()?;
        Ok(Self::new(cwd))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn scheme(&self) -> &'static str {
        self.scheme
    }

    pub fn does_validate_scheme(&self) -> bool {
        SVSw::ENABLED
    }

    pub fn iri_has_compatible_scheme(&self, iri: &IRI) -> bool {
        iri.scheme() == self.scheme
    }
}

impl<ValidateScheme> ResourceLoaderComponent for FsResourceLoader<ValidateScheme>
where
    ValidateScheme: ConstSwitch,
{
    fn load_resource(
        &self,
        source: &Source,
        ctx: &impl Context,
    ) -> SendBoxFuture<MaybeEncData, ResourceLoadingError> {
        if ValidateScheme::ENABLED && !self.iri_has_compatible_scheme(&source.iri) {
            let err = ResourceLoadingError::from(ResourceLoadingErrorKind::NotFound)
                .with_source_iri_or_else(|| Some(source.iri.clone()));

            return Box::new(Err(err).into_future());
        }

        let path = self.root().join(path_from_tail(&source.iri));
        let use_media_type = source.use_media_type.clone();
        let use_file_name = source.use_file_name.clone();

        load_data(path, use_media_type, use_file_name, ctx, |data| {
            Ok(MaybeEncData::EncData(
                data.transfer_encode(Default::default()),
            ))
        })
    }
}

//TODO add a PostProcess hook which can be any combination of
// FixNewline, SniffMediaType and custom postprocessing
// now this has new responsibilities
// 2. get and create File Meta
// 3. if source.media_type.is_none() do cautious mime sniffing
pub fn load_data<R, F>(
    path: PathBuf,
    use_media_type: UseMediaType,
    use_file_name: Option<String>,
    ctx: &impl Context,
    post_process: F,
) -> SendBoxFuture<R, ResourceLoadingError>
where
    R: Send + 'static,
    F: FnOnce(Data) -> Result<R, ResourceLoadingError> + Send + 'static,
{
    let content_id = ctx.generate_content_id();
    ctx.offload_fn(move || {
        let mut fd = File::open(&path).map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                err.context(ResourceLoadingErrorKind::NotFound)
            } else {
                err.context(ResourceLoadingErrorKind::LoadingFailed)
            }
        })?;

        let mut file_meta = file_meta_from_metadata(fd.metadata()?);

        if let Some(name) = use_file_name {
            file_meta.file_name = Some(name)
        } else {
            file_meta.file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        }

        let mut buffer = Vec::new();
        fd.read_to_end(&mut buffer)?;

        let media_type = match use_media_type {
            UseMediaType::Auto => sniff_media_type(&path)?,
            UseMediaType::Default(media_type) => media_type,
        };

        let data = Data::new(
            buffer,
            Metadata {
                file_meta,
                content_id,
                media_type,
            },
        );

        post_process(data)
    })
}

fn sniff_media_type(path: impl AsRef<Path>) -> Result<MediaType, ResourceLoadingError> {
    //TODO replace current  impl with conservative sniffing
    let output = CheckedCommand::new("file")
        .args(&["--brief", "--mime"])
        .arg(path.as_ref())
        .output()
        .map_err(|err| err.context(ResourceLoadingErrorKind::MediaTypeDetectionFailed))?;

    let raw_media_type = String::from_utf8(output.stdout)
        .map_err(|err| err.context(ResourceLoadingErrorKind::MediaTypeDetectionFailed))?;

    let media_type = MediaType::parse(raw_media_type.trim())
        .map_err(|err| err.context(ResourceLoadingErrorKind::MediaTypeDetectionFailed))?;

    Ok(media_type)
}

//TODO implement From<MetaDate> for FileMeta instead of this
fn file_meta_from_metadata(meta: fs::Metadata) -> FileMeta {
    FileMeta {
        file_name: None,
        creation_date: meta.created().ok().map(From::from),
        modification_date: meta.modified().ok().map(From::from),
        read_date: meta.accessed().ok().map(From::from),
        //TODO make FileMeta.size a u64
        size: get_file_size(&meta).map(|x| x as usize),
    }
}

fn get_file_size(meta: &fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Some(meta.size());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return Some(meta.file_size());
    }
    #[allow(unreachable_code)]
    None
}

fn path_from_tail(path_iri: &IRI) -> &Path {
    let tail = path_iri.tail();
    let path = if tail.starts_with("///") {
        &tail[2..]
    } else {
        &tail
    };
    Path::new(path)
}

#[cfg(test)]
mod tests {

    mod sniff_media_type {
        use super::super::*;

        #[test]
        fn works_reasonable_for_cargo_files() {
            let res = sniff_media_type("./Cargo.toml").unwrap();

            // it currently doesn't take advantage of file endings so
            // all pure "text" will be text/plain
            assert_eq!(res.as_str_repr(), "text/plain; charset=us-ascii");
        }
    }
}
