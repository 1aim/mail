use std::env;
use std::path::Path;

use futures::Future;
use headers::header_components::{Domain, MediaType};
use mail_core::context::CompositeContext;
use mail_core::default_impl::{simple_context, simple_cpu_pool, FsResourceLoader, HashedIdGen};
use mail_core::{Context, EncData, Resource, Source, UseMediaType, IRI};
use soft_ascii_string::SoftAsciiString;

fn dumy_ctx(resource_loader: FsResourceLoader) -> simple_context::Context {
    let domain = Domain::from_unchecked("hy.test".to_owned());
    let unique_part = SoftAsciiString::from_unchecked("w09ad8f");
    let id_gen = HashedIdGen::new(domain, unique_part).unwrap();
    CompositeContext::new(resource_loader, simple_cpu_pool(), id_gen)
}

fn loaded_resource(path: &str, media_type: &str, name: Option<&str>) -> EncData {
    let resource_loader: FsResourceLoader = FsResourceLoader::new(
        env::current_dir()
            .unwrap()
            .join(Path::new("./test_resources/")),
    );

    let ctx = dumy_ctx(resource_loader);

    let source = Source {
        iri: IRI::from_parts("path", path).unwrap(),
        use_media_type: UseMediaType::Default(MediaType::parse(media_type).unwrap()),
        use_file_name: name.map(|s| s.to_owned()),
    };

    ctx.load_transfer_encoded_resource(&Resource::Source(source))
        .wait()
        .unwrap()
}

#[test]
fn get_name_from_path() {
    let enc_data = loaded_resource("img.png", "image/png", None);
    assert_eq!(enc_data.file_meta().file_name, Some("img.png".to_owned()));
}

#[test]
fn use_name_is_used() {
    let enc_data = loaded_resource("img.png", "image/png", Some("That Image"));

    assert_eq!(
        enc_data.file_meta().file_name,
        Some("That Image".to_owned())
    );
}
