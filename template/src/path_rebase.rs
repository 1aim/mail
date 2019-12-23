use std::{
    mem,
    path::{Path, PathBuf},
};

use failure::{Context, Fail};
use mail_core::{Resource, IRI};

#[derive(Fail, Debug)]
#[fail(
    display = "unsupported path, only paths with following constraint are allowed: {}",
    _0
)]
pub struct UnsupportedPathError(Context<&'static str>);

impl UnsupportedPathError {
    pub fn new(violated_constraint: &'static str) -> Self {
        UnsupportedPathError(Context::new(violated_constraint))
    }
}

pub trait PathRebaseable {
    /// Prefixes path in the type with `base_dir`.
    ///
    /// # Error
    ///
    /// Some implementors might not support all paths.
    /// For example a implementor requiring rust string
    /// compatible paths might return a
    /// `Err(UnsupportedPathError::new("utf-8"))`.
    fn rebase_to_include_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError>;

    /// Removes the `base_dir` prefix.
    ///
    /// # Error
    ///
    /// Some implementors might not support all paths.
    /// For example a implementor requiring rust string
    /// compatible paths might return a
    /// `Err(UnsupportedPathError::new("utf-8"))`.
    fn rebase_to_exclude_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError>;
}

impl PathRebaseable for PathBuf {
    fn rebase_to_include_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError> {
        let new_path;
        if self.is_relative() {
            new_path = base_dir.as_ref().join(&self);
        } else {
            return Ok(());
        }
        mem::replace(self, new_path);
        Ok(())
    }

    fn rebase_to_exclude_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError> {
        let new_path;
        if let Ok(path) = self.strip_prefix(base_dir) {
            new_path = path.to_owned();
        } else {
            return Ok(());
        }
        mem::replace(self, new_path);
        Ok(())
    }
}

impl PathRebaseable for IRI {
    fn rebase_to_include_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError> {
        if self.scheme() != "path" {
            return Ok(());
        }

        let new_tail = {
            let path = Path::new(self.tail());
            if path.is_relative() {
                base_dir.as_ref().join(path)
            } else {
                return Ok(());
            }
        };

        let new_tail = new_tail
            .to_str()
            .ok_or_else(|| UnsupportedPathError::new("utf-8"))?;

        let new_iri = self.with_tail(new_tail);
        mem::replace(self, new_iri);
        Ok(())
    }

    fn rebase_to_exclude_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError> {
        if self.scheme() != "path" {
            return Ok(());
        }

        let new_iri = {
            let path = Path::new(self.tail());

            if let Ok(path) = path.strip_prefix(base_dir) {
                //UNWRAP_SAFE: we just striped some parts, this can
                // not make it lose it's string-ness
                let new_tail = path.to_str().unwrap();
                self.with_tail(new_tail)
            } else {
                return Ok(());
            }
        };

        mem::replace(self, new_iri);
        Ok(())
    }
}

impl PathRebaseable for Resource {
    fn rebase_to_include_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError> {
        if let &mut Resource::Source(ref mut source) = self {
            source.iri.rebase_to_include_base_dir(base_dir)?;
        }
        Ok(())
    }

    fn rebase_to_exclude_base_dir(
        &mut self,
        base_dir: impl AsRef<Path>,
    ) -> Result<(), UnsupportedPathError> {
        if let &mut Resource::Source(ref mut source) = self {
            source.iri.rebase_to_exclude_base_dir(base_dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use mail_core::Source;

    #[test]
    fn rebase_on_path() {
        let mut path = Path::new("/prefix/suffix.yup").to_owned();
        path.rebase_to_exclude_base_dir("/prefix").unwrap();
        assert_eq!(path, Path::new("suffix.yup"));
        path.rebase_to_include_base_dir("./nfix").unwrap();
        path.rebase_to_include_base_dir("/mfix").unwrap();
        assert_eq!(path, Path::new("/mfix/nfix/suffix.yup"));
        path.rebase_to_exclude_base_dir("/wrong").unwrap();
        assert_eq!(path, Path::new("/mfix/nfix/suffix.yup"));
    }

    #[test]
    fn rebase_on_iri() {
        let mut iri: IRI = "path:/prefix/suffix.yup".parse().unwrap();
        iri.rebase_to_exclude_base_dir("/prefix").unwrap();
        assert_eq!(iri.as_str(), "path:suffix.yup");
        iri.rebase_to_include_base_dir("nfix").unwrap();
        iri.rebase_to_include_base_dir("/mfix").unwrap();
        assert_eq!(iri.as_str(), "path:/mfix/nfix/suffix.yup");
        iri.rebase_to_exclude_base_dir("/wrong").unwrap();
        assert_eq!(iri.as_str(), "path:/mfix/nfix/suffix.yup");
    }

    #[test]
    fn rebase_on_resource() {
        let mut resource = Resource::Source(Source {
            iri: "path:abc/def".parse().unwrap(),
            use_media_type: Default::default(),
            use_file_name: Default::default(),
        });

        resource.rebase_to_include_base_dir("./abc").unwrap();
        resource.rebase_to_include_base_dir("/pre").unwrap();
        resource.rebase_to_exclude_base_dir("/pre").unwrap();
        resource.rebase_to_exclude_base_dir("abc").unwrap();
        resource.rebase_to_include_base_dir("abc").unwrap();

        if let Resource::Source(Source { iri, .. }) = resource {
            assert_eq!(iri.as_str(), "path:abc/abc/def");
        } else {
            unreachable!()
        }
    }
}
