extern crate failure;
extern crate futures;
#[cfg(feature = "handlebars")]
extern crate handlebars as hbs;
extern crate mail_core;
extern crate mail_headers;
extern crate maybe_owned;
extern crate serde;
extern crate toml;
extern crate vec1;

#[cfg(all(feature = "handlebars", not(feature = "handlebars-bindings")))]
compile_error!("use feature `handlebars-bindings` instead of opt-dep-auto-feature `handlebars`");

use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

use failure::Error;
use futures::{
    future::{self, Either, Join},
    try_ready, Async, Future, Poll,
};
use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};
use vec1::Vec1;

use mail_core::{
    compose::{BodyPart, MailParts},
    Context, Data, Mail, Metadata, Resource, ResourceContainerLoadingFuture,
};
use mail_headers::{header_components::MediaType, headers, Header, HeaderKind};

mod additional_cid;
mod base_dir;
pub mod error;
mod path_rebase;
pub mod serde_impl;

#[cfg(feature = "handlebars")]
pub mod handlebars;

pub use self::additional_cid::*;
pub use self::base_dir::*;
pub use self::path_rebase::*;

/// Trait used to bind/implement template engines.
pub trait TemplateEngine: Sized {
    type Id: Debug;

    type LazyBodyTemplate: PathRebaseable + Debug + Send + Serialize + for<'a> Deserialize<'a>;

    fn load_body_template(
        &mut self,
        tmpl: Self::LazyBodyTemplate,
    ) -> Result<BodyTemplate<Self>, Error>;

    fn load_subject_template(&mut self, template_string: String) -> Result<Self::Id, Error>;
}

/// Additional trait a template engine needs to implement for the types it can process as input.
///
/// This could for example be implemented in a wild card impl for the template engine for
/// any data `D` which implements `Serialize`.
pub trait TemplateEngineCanHandleData<D>: TemplateEngine {
    fn render<'r, 'a>(
        &'r self,
        id: &'r Self::Id,
        data: &'r D,
        additional_cids: AdditionalCIds<'r>,
    ) -> Result<String, Error>;
}

/// Load a template as described in a toml file.
///
/// This will set the default of the base_dir to the
/// dir the template file loaded is in.
pub fn load_toml_template_from_path<TE, C>(
    engine: TE,
    path: PathBuf,
    ctx: &C,
) -> impl Future<Item = Template<TE>, Error = Error>
where
    TE: TemplateEngine + 'static,
    C: Context,
{
    let ctx2 = ctx.clone();
    ctx.offload_fn(move || {
        let content = fs::read_to_string(&path)?;
        let base: serde_impl::TemplateBase<TE> = toml::from_str(&content)?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let base_dir = CwdBaseDir::from_path(base_dir)?;
        Ok((base, base_dir))
    })
    .and_then(move |(base, base_dir)| base.load(engine, base_dir, &ctx2))
}

/// Load a template as described in a toml string;
pub fn load_toml_template_from_str<TE, C>(
    engine: TE,
    content: &str,
    ctx: &C,
) -> impl Future<Item = Template<TE>, Error = Error>
where
    TE: TemplateEngine,
    C: Context,
{
    let base: serde_impl::TemplateBase<TE> = match toml::from_str(content) {
        Ok(base) => base,
        Err(err) => {
            return Either::B(future::err(Error::from(err)));
        }
    };

    let base_dir = match CwdBaseDir::from_path(Path::new(".")) {
        Ok(base_dir) => base_dir,
        Err(err) => return Either::B(future::err(Error::from(err))),
    };

    Either::A(base.load(engine, base_dir, ctx))
}

/// A Mail template.
#[derive(Debug)]
pub struct Template<TE: TemplateEngine> {
    template_name: String,
    base_dir: CwdBaseDir,
    subject: Subject<TE>,
    /// This can only be in the loaded form _iff_ this is coupled
    /// with a template engine instance, as using it with the wrong
    /// template engine will lead to potential bugs and panics.
    bodies: Vec1<BodyTemplate<TE>>,
    //TODO: make sure
    embeddings: HashMap<String, Resource>,
    attachments: Vec<Resource>,
    engine: TE,
}

impl<TE> Template<TE>
where
    TE: TemplateEngine,
{
    pub fn inline_embeddings(&self) -> &HashMap<String, Resource> {
        &self.embeddings
    }

    pub fn attachments(&self) -> &[Resource] {
        &self.attachments
    }

    pub fn engine(&self) -> &TE {
        &self.engine
    }

    pub fn bodies(&self) -> &[BodyTemplate<TE>] {
        &self.bodies
    }

    pub fn subject_template_id(&self) -> &TE::Id {
        &self.subject.template_id
    }
}

/// Represents one of potentially many alternate bodies in a template.
#[derive(Debug)]
pub struct BodyTemplate<TE: TemplateEngine> {
    pub template_id: TE::Id,
    pub media_type: MediaType,
    pub inline_embeddings: HashMap<String, Resource>,
}

impl<TE> BodyTemplate<TE>
where
    TE: TemplateEngine,
{
    pub fn template_id(&self) -> &TE::Id {
        &self.template_id
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn inline_embeddings(&self) -> &HashMap<String, Resource> {
        &self.inline_embeddings
    }
}

/// Represents a template used for generating the subject of a mail.
#[derive(Debug)]
pub struct Subject<TE: TemplateEngine> {
    template_id: TE::Id,
}

impl<TE> Subject<TE>
where
    TE: TemplateEngine,
{
    pub fn template_id(&self) -> &TE::Id {
        &self.template_id
    }
}

/// Automatically provides the `prepare_to_render` method for all `Templates`
///
/// This trait is implemented for all `Templates`/`D`(data) combinations where
/// the templates template engine can handle the given data (impl. `TemplateEngineCanHandleData<D>`)
///
/// This trait should not be implemented by hand.
pub trait TemplateExt<TE, D>
where
    TE: TemplateEngine + TemplateEngineCanHandleData<D>,
{
    fn render_to_mail_parts<'r>(
        &self,
        data: LoadedTemplateData<'r, D>,
        ctx: &impl Context,
    ) -> Result<(MailParts, Header<headers::Subject>), Error>;

    fn render<'r>(
        &self,
        data: LoadedTemplateData<'r, D>,
        ctx: &impl Context,
    ) -> Result<Mail, Error> {
        let (parts, subject) = self.render_to_mail_parts(data, ctx)?;
        let mut mail = parts.compose();
        mail.insert_header(subject);
        Ok(mail)
    }
}

impl<TE, D> TemplateExt<TE, D> for Template<TE>
where
    TE: TemplateEngine + TemplateEngineCanHandleData<D>,
{
    fn render_to_mail_parts<'r>(
        &self,
        data: LoadedTemplateData<'r, D>,
        ctx: &impl Context,
    ) -> Result<(MailParts, Header<headers::Subject>), Error> {
        let TemplateData {
            data,
            inline_embeddings,
            mut attachments,
        } = data.into();

        let subject =
            self.engine()
                .render(self.subject_template_id(), &data, AdditionalCIds::new(&[]))?;

        let subject = headers::Subject::auto_body(subject)?;

        //TODO use Vec1 try_map instead of loop
        let mut bodies = Vec::new();
        for body in self.bodies().iter() {
            let raw = self.engine().render(
                body.template_id(),
                &data,
                AdditionalCIds::new(&[
                    &inline_embeddings,
                    body.inline_embeddings(),
                    self.inline_embeddings(),
                ]),
            )?;

            let data = Data::new(
                raw.into_bytes(),
                Metadata {
                    file_meta: Default::default(),
                    media_type: body.media_type().clone(),
                    content_id: ctx.generate_content_id(),
                },
            );

            let inline_embeddings = body.inline_embeddings().values().cloned().collect();

            bodies.push(BodyPart {
                resource: Resource::Data(data),
                inline_embeddings,
                attachments: Vec::new(),
            });
        }

        attachments.extend(self.attachments().iter().cloned());

        let mut inline_embeddings_vec = Vec::new();
        for (key, val) in self.inline_embeddings() {
            if !inline_embeddings.contains_key(key) {
                inline_embeddings_vec.push(val.clone())
            }
        }

        inline_embeddings_vec.extend(inline_embeddings.into_iter().map(|(_, v)| v));

        let parts = MailParts {
            //UNWRAP_SAFE (complexly mapping a Vec1 is safe)
            alternative_bodies: Vec1::try_from_vec(bodies).unwrap(),
            inline_embeddings: inline_embeddings_vec,
            attachments,
        };

        Ok((parts, subject))
    }
}

pub struct TemplateData<'a, D: 'a> {
    pub data: MaybeOwned<'a, D>,
    pub attachments: Vec<Resource>,
    pub inline_embeddings: HashMap<String, Resource>,
}

impl<'a, D> TemplateData<'a, D> {
    pub fn load(self, ctx: &impl Context) -> DataLoadingFuture<'a, D> {
        let TemplateData {
            data,
            attachments,
            inline_embeddings,
        } = self;

        let loading_fut = Resource::load_container(inline_embeddings, ctx)
            .join(Resource::load_container(attachments, ctx));

        DataLoadingFuture {
            payload: Some(data),
            loading_fut,
        }
    }
}
impl<D> From<D> for TemplateData<'static, D> {
    fn from(data: D) -> Self {
        TemplateData {
            data: data.into(),
            attachments: Default::default(),
            inline_embeddings: Default::default(),
        }
    }
}

impl<'a, D> From<&'a D> for TemplateData<'a, D> {
    fn from(data: &'a D) -> Self {
        TemplateData {
            data: data.into(),
            attachments: Default::default(),
            inline_embeddings: Default::default(),
        }
    }
}

pub struct LoadedTemplateData<'a, D: 'a>(TemplateData<'a, D>);

impl<'a, D> From<&'a D> for LoadedTemplateData<'a, D> {
    fn from(data: &'a D) -> Self {
        LoadedTemplateData(TemplateData::from(data))
    }
}

impl<D> From<D> for LoadedTemplateData<'static, D> {
    fn from(data: D) -> Self {
        LoadedTemplateData(TemplateData::from(data))
    }
}

impl<'a, D> Deref for LoadedTemplateData<'a, D> {
    type Target = TemplateData<'a, D>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, D> Into<TemplateData<'a, D>> for LoadedTemplateData<'a, D> {
    fn into(self) -> TemplateData<'a, D> {
        let LoadedTemplateData(data) = self;
        data
    }
}

/// Future returned when preparing a template for rendering.
pub struct DataLoadingFuture<'a, D: 'a> {
    payload: Option<MaybeOwned<'a, D>>,
    loading_fut: Join<
        ResourceContainerLoadingFuture<HashMap<String, Resource>>,
        ResourceContainerLoadingFuture<Vec<Resource>>,
    >,
}

impl<'a, D> Future for DataLoadingFuture<'a, D> {
    type Item = LoadedTemplateData<'a, D>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (inline_embeddings, attachments) = try_ready!(self.loading_fut.poll());

        //UNWRAP_SAFE only non if polled after resolved
        let data = self.payload.take().unwrap();

        let inner = TemplateData {
            data,
            inline_embeddings,
            attachments,
        };

        Ok(Async::Ready(LoadedTemplateData(inner)))
    }
}
