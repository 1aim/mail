//! Provides the context needed for building/encoding mails.
use std::fmt::Debug;
use std::sync::Arc;

use futures::{
    future::{self, Either},
    Future, IntoFuture,
};
use utils::SendBoxFuture;

use headers::header_components::{ContentId, MessageId};

use crate::{
    error::ResourceLoadingError,
    resource::{Data, EncData, Resource, Source},
};

/// Represents Data which might already have been transfer encoded.
pub enum MaybeEncData {
    /// The data is returned normally.
    Data(Data),

    /// The data is returned in a already transfer encoded variant.
    EncData(EncData),
}

impl MaybeEncData {
    pub fn to_resource(self) -> Resource {
        match self {
            MaybeEncData::Data(data) => Resource::Data(data),
            MaybeEncData::EncData(enc_data) => Resource::EncData(enc_data),
        }
    }

    pub fn encode(
        self,
        ctx: &impl Context,
    ) -> impl Future<Item = EncData, Error = ResourceLoadingError> {
        match self {
            MaybeEncData::Data(data) => {
                Either::A(ctx.load_transfer_encoded_resource(&Resource::Data(data)))
            }
            MaybeEncData::EncData(enc) => Either::B(future::ok(enc)),
        }
    }
}

/// This library needs a context for creating/encoding mails.
///
/// The context is _not_ meant to be a think you create once
/// per mail but something you create once one startup and then
/// re-user wherever it is needed in your application.
///
/// A context impl. provides following functionality to this library:
///
/// 1. Load a `Resource` based on a `Source` instance.
///    I.e. an IRI plus some optional meta information.
/// 2. Generate an unique message id. This should be
///    an world unique id to comply with the standard(s).
/// 3. Generate an unique content id. This should be
///    an world unique id to comply with the standard(s).
/// 4. A way to "offload" work to some other "place" e.g.
///    by scheduling it in an thread pool.
///
/// The `CompositeContext` provides a impl. for this trait
/// which delegates the different tasks to the components
/// it's composed of. This allow to reuse e.g. the message
/// if generation and offloading but use a custom resource
/// loading. Generally it's recommended to use the `CompositeContext`.
///
/// There is also an default implementation (using the
/// `CompositeContext` in the `default_impl` module).
///
/// # Clone / Send / Sync / 'static ?
///
/// `Context` are meant to be easily shareable, cloning them should be
/// cheap, as such if a implementor contains state it might make sense for an
/// implementor to have a outer+inner type where the inner type is wrapped
/// into a `Arc` e.g. `struct SomeCtx { inner: Arc<InnerSomeCtx> }`.
pub trait Context: Debug + Clone + Send + Sync + 'static {
    /// Loads and transfer encodes a `Data` instance.
    ///
    /// This is called when a `Mail` instance is converted into
    /// a encodable mail an a `Resource::Data` instance is found.
    ///
    /// This can potentially use the IRI to implement a simple
    /// caching scheme potentially directly caching the transfer
    /// encoded instance skipping the loading + encoding step for
    /// common resources like e.g. a logo.
    ///
    /// If the context implementation only caches enc data instances
    /// but not data instances it might return [`MaybeEncData::EncData`].
    /// If it newly load the resource it _should_ return a [`MaybeEncData::Data`]
    /// variant.
    ///
    /// # Async Considerations
    ///
    /// This function should not block and schedule the encoding
    /// in some other place e.g. by using the contexts offload
    /// functionality.
    fn load_resource(&self, source: &Source) -> SendBoxFuture<MaybeEncData, ResourceLoadingError>;

    /// Loads and Transfer encodes a `Resource` instance.
    ///
    /// This is called when a `Mail` instance is converted into
    /// a encodable mail.
    ///
    /// The default impl. of this function just:
    ///
    /// 1. calls load_resource and chains a "offloaded" transfer encoding
    ///    if a `Resource::Source` is found.
    /// 2. transfer encodes the data "offloaded" if `Resource::Data` is found
    /// 3. just returns the encoded data if `Resource::EncData` is found
    ///
    /// A more advances implementation could for example integrate
    /// a LRU cache.
    ///
    /// The default impl is available as the `default_impl_for_load_transfer_encoded_resource`
    /// function.
    ///
    /// # Async Considerations
    ///
    /// This function should not block and schedule the encoding
    /// in some other place e.g. by using the contexts offload
    /// functionality.
    fn load_transfer_encoded_resource(
        &self,
        resource: &Resource,
    ) -> SendBoxFuture<EncData, ResourceLoadingError> {
        default_impl_for_load_transfer_encoded_resource(self, resource)
    }

    /// generate a unique content id
    ///
    /// As message id's are used to reference messages they should be
    /// world unique this can be guaranteed through two aspects:
    ///
    /// 1. using a domain you own/control on the right hand side
    ///    of the `@` will make sure no id's from other persons/companies/...
    ///    will collide with your ids
    ///
    /// 2. using some internal mechanism for the left hand side, like including
    ///    the time and an internal counter, not that you have to make sure this
    ///    stays unique even if you run multiple instances or restart the current
    ///    running instance.
    ///
    fn generate_message_id(&self) -> MessageId;

    /// generate a unique content id
    ///
    /// Rfc 2045 states that content id's have to be world unique,
    /// while this really should be the case if it's used in combination
    /// with an `multipart/external` or similar body for it's other usage
    /// as reference for embeddings it being mail unique tends to be enough.
    ///
    /// As content id and message id are treated mostly the same wrt. the
    /// constraints applying when generating them this can be implemented
    /// in terms of calling `generate_message_id`.
    fn generate_content_id(&self) -> ContentId;

    //TODO[futures/v>=0.2]: integrate this with Context
    /// offloads the execution of the future `fut` to somewhere else e.g. a cpu pool
    fn offload<F>(&self, fut: F) -> SendBoxFuture<F::Item, F::Error>
    where
        F: Future + Send + 'static,
        F::Item: Send + 'static,
        F::Error: Send + 'static;

    //TODO[futures/v>=0.2]: integrate this with Context
    /// offloads the execution of the function `func` to somewhere else e.g. a cpu pool
    fn offload_fn<FN, I>(&self, func: FN) -> SendBoxFuture<I::Item, I::Error>
    where
        FN: FnOnce() -> I + Send + 'static,
        I: IntoFuture + 'static,
        I::Future: Send + 'static,
        I::Item: Send + 'static,
        I::Error: Send + 'static,
    {
        self.offload(future::lazy(func))
    }
}

/// Provides the default impl for the `load_transfer_encoded_resource` method of `Context`.
///
/// This function guarantees to only call `load_resource` and `offload`/`offload_fn` on the
/// passed in context, to prevent infinite recursion.
pub fn default_impl_for_load_transfer_encoded_resource(
    ctx: &impl Context,
    resource: &Resource,
) -> SendBoxFuture<EncData, ResourceLoadingError> {
    match resource {
        Resource::Source(source) => {
            let ctx2 = ctx.clone();
            let fut = ctx
                .load_resource(&source)
                .and_then(move |me_data| match me_data {
                    MaybeEncData::Data(data) => Either::A(
                        ctx2.offload_fn(move || Ok(data.transfer_encode(Default::default()))),
                    ),
                    MaybeEncData::EncData(enc_data) => Either::B(future::ok(enc_data)),
                });
            Box::new(fut)
        }
        Resource::Data(data) => {
            let data = data.clone();
            ctx.offload_fn(move || Ok(data.transfer_encode(Default::default())))
        }
        Resource::EncData(enc_data) => Box::new(future::ok(enc_data.clone())),
    }
}

/// Trait needed to be implemented for providing the resource loading parts to a`CompositeContext`.
pub trait ResourceLoaderComponent: Debug + Send + Sync + 'static {
    /// Calls to `Context::load_resource` will be forwarded to this method.
    ///
    /// It is the same as `Context::load_resource` except that a reference
    /// to the context containing this component is passed in. To prevent
    /// infinite recursion the `Context.load_resource` method _must not_
    /// be called. Additionally the `Context.load_transfer_encoded_resource` _must not_
    /// be called if it uses `Context.load_resource`.
    fn load_resource(
        &self,
        source: &Source,
        ctx: &impl Context,
    ) -> SendBoxFuture<MaybeEncData, ResourceLoadingError>;

    /// Calls to `Context::transfer_encode_resource` will be forwarded to this method.
    ///
    /// It is the same as `Context::transfer_encode_resource` except that a reference
    /// to the context containing this component is passed in to make the `offload`
    /// and `load_resource` methods of `Context` available.
    ///
    /// To prevent infinite recursion the `load_transfer_encoded_resource` method
    /// of the context _must not_ be called.
    fn load_transfer_encoded_resource(
        &self,
        resource: &Resource,
        ctx: &impl Context,
    ) -> SendBoxFuture<EncData, ResourceLoadingError> {
        default_impl_for_load_transfer_encoded_resource(ctx, resource)
    }
}

/// Trait needed to be implemented for providing the offloading parts to a `CompositeContext`.
pub trait OffloaderComponent: Debug + Send + Sync + 'static {
    /// Calls to `Context::offload` and `Context::offload_fn` will be forwarded to this method.
    fn offload<F>(&self, fut: F) -> SendBoxFuture<F::Item, F::Error>
    where
        F: Future + Send + 'static,
        F::Item: Send + 'static,
        F::Error: Send + 'static;
}

/// Trait needed to be implemented for providing the id generation parts to a `CompositeContext`.
///
/// It is possible/valid to use the same implementation (internal function etc.) for
/// both message and content ids. While they are used at different places they have
/// mostly the same constraints (their meaning is still different i.e. it's much
/// more important for an message id to be "world unique" then for an content id,
/// expect in some cases where external bodies are used).
pub trait MailIdGenComponent: Debug + Send + Sync + 'static {
    /// Calls to `Context::generate_message_id` will be forwarded to this method.
    fn generate_message_id(&self) -> MessageId;

    /// Calls to `Context::generate_content_id` will be forwarded to this method.
    fn generate_content_id(&self) -> ContentId;
}

/// The `CompositeContext` is the simplest way to get an `Context` implementation.
///
/// Any custom `Context` implementations should be realized through the `CompositeContext`
/// if possible.
///
/// This type consists of 3 components it forward all method calls from `Context` to.
/// This allows the library to have a single `Context` type but match and mix the
/// parts about resource loading, offloading and id generation in whichever way
/// it fits best.
///
/// The composite context will store the components inside of an `Arc` so that
/// it can be easily shared through an application, it also means non of the
/// components have to implement `Clone`.
#[derive(Debug)]
pub struct CompositeContext<
    R: ResourceLoaderComponent,
    O: OffloaderComponent,
    M: MailIdGenComponent,
> {
    inner: Arc<(R, O, M)>,
}

impl<R, O, M> Clone for CompositeContext<R, O, M>
where
    R: ResourceLoaderComponent,
    O: OffloaderComponent,
    M: MailIdGenComponent,
{
    fn clone(&self) -> Self {
        CompositeContext {
            inner: self.inner.clone(),
        }
    }
}

impl<R, O, M> CompositeContext<R, O, M>
where
    R: ResourceLoaderComponent,
    O: OffloaderComponent,
    M: MailIdGenComponent,
{
    /// Create a new context from the given components.
    pub fn new(resource_loader: R, offloader: O, message_id_gen: M) -> Self {
        CompositeContext {
            inner: Arc::new((resource_loader, offloader, message_id_gen)),
        }
    }

    /// Returns a reference to the resource loader component.
    pub fn resource_loader(&self) -> &R {
        &self.inner.0
    }

    /// Returns a reference to the offloader component.
    pub fn offloader(&self) -> &O {
        &self.inner.1
    }

    /// Returns a reference to the id generation component.
    pub fn id_gen(&self) -> &M {
        &self.inner.2
    }
}

impl<R, O, M> Context for CompositeContext<R, O, M>
where
    R: ResourceLoaderComponent,
    O: OffloaderComponent,
    M: MailIdGenComponent,
{
    fn load_resource(&self, source: &Source) -> SendBoxFuture<MaybeEncData, ResourceLoadingError> {
        self.resource_loader().load_resource(source, self)
    }

    fn load_transfer_encoded_resource(
        &self,
        resource: &Resource,
    ) -> SendBoxFuture<EncData, ResourceLoadingError> {
        self.resource_loader()
            .load_transfer_encoded_resource(resource, self)
    }

    fn offload<F>(&self, fut: F) -> SendBoxFuture<F::Item, F::Error>
    where
        F: Future + Send + 'static,
        F::Item: Send + 'static,
        F::Error: Send + 'static,
    {
        self.offloader().offload(fut)
    }

    fn generate_content_id(&self) -> ContentId {
        self.id_gen().generate_content_id()
    }

    fn generate_message_id(&self) -> MessageId {
        self.id_gen().generate_message_id()
    }
}

/// Allows using a part of an context as an component.
impl<C> MailIdGenComponent for C
where
    C: Context,
{
    fn generate_message_id(&self) -> MessageId {
        <Self as Context>::generate_message_id(self)
    }

    fn generate_content_id(&self) -> ContentId {
        <Self as Context>::generate_content_id(self)
    }
}

/// Allows using a part of an context as an component.
impl<C> OffloaderComponent for C
where
    C: Context,
{
    fn offload<F>(&self, fut: F) -> SendBoxFuture<F::Item, F::Error>
    where
        F: Future + Send + 'static,
        F::Item: Send + 'static,
        F::Error: Send + 'static,
    {
        <Self as Context>::offload(self, fut)
    }
}

/// Allows using a part of an context as an component.
impl<C> ResourceLoaderComponent for C
where
    C: Context,
{
    fn load_resource(
        &self,
        source: &Source,
        _: &impl Context,
    ) -> SendBoxFuture<MaybeEncData, ResourceLoadingError> {
        <Self as Context>::load_resource(self, source)
    }

    fn load_transfer_encoded_resource(
        &self,
        resource: &Resource,
        _: &impl Context,
    ) -> SendBoxFuture<EncData, ResourceLoadingError> {
        <Self as Context>::load_transfer_encoded_resource(self, resource)
    }
}
