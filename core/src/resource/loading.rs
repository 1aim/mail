//! This modules add some helpers to load all resources in some data type.
use std::{
    borrow::{Borrow, ToOwned},
    collections::HashMap,
    mem,
};

use futures::{
    future::{self, JoinAll},
    try_ready, Async, Future, Poll,
};

use crate::{error::ResourceLoadingError, utils::SendBoxFuture, Context, MaybeEncData, Resource};

pub trait ContainedResourcesAccess {
    type Key: ToOwned + ?Sized;

    /// Visit all resources.
    ///
    /// This method is not allowed to be implemented in a way that
    /// it might visit resources, which are not accessable with
    /// [`Self.access_resource_mut()`]. This means that without
    /// e.g. a `RwLock` this can not visit resources in a `Arc`.
    fn visit_resources(&self, visitor: &mut impl FnMut(&Self::Key, &Resource));

    /// Return a mut ref for a resource based on given key.
    ///
    /// If a resource is visited in a `Self.visit_resources()` call
    /// and the state of self was not changes this method has to
    /// be able to return a mut reference to it.
    ///
    /// To allow accessing resources in a mutext this does pass
    /// the mut ref to a closure instead of returning it.
    ///
    fn access_resource_mut<R>(
        &mut self,
        key: &Self::Key,
        modify: impl FnOnce(Option<&mut Resource>) -> R,
    ) -> R;

    /// Return a ref for a resource base on given key.
    fn access_resource<R>(&self, key: &Self::Key, modify: impl FnOnce(Option<&Resource>) -> R)
        -> R;
}

impl ContainedResourcesAccess for Vec<Resource> {
    type Key = usize;

    fn visit_resources(&self, visitor: &mut impl FnMut(&Self::Key, &Resource)) {
        for (idx, resource) in self.iter().enumerate() {
            visitor(&idx, resource)
        }
    }

    fn access_resource_mut<R>(
        &mut self,
        key: &Self::Key,
        modify: impl FnOnce(Option<&mut Resource>) -> R,
    ) -> R {
        modify(self.get_mut(*key))
    }

    /// Return a ref for a resource base on given key.
    fn access_resource<R>(
        &self,
        key: &Self::Key,
        modify: impl FnOnce(Option<&Resource>) -> R,
    ) -> R {
        modify(self.get(*key))
    }
}

impl ContainedResourcesAccess for HashMap<String, Resource> {
    type Key = str;

    fn visit_resources(&self, visitor: &mut impl FnMut(&Self::Key, &Resource)) {
        for (key, resource) in self.iter() {
            visitor(&key, resource)
        }
    }

    fn access_resource_mut<R>(
        &mut self,
        key: &Self::Key,
        modify: impl FnOnce(Option<&mut Resource>) -> R,
    ) -> R {
        modify(self.get_mut(key))
    }

    /// Return a ref for a resource base on given key.
    fn access_resource<R>(
        &self,
        key: &Self::Key,
        modify: impl FnOnce(Option<&Resource>) -> R,
    ) -> R {
        modify(self.get(key))
    }
}

//TODO[feat] impl. where applicable in std (Box, BTreeMap, other HashMap, etc.)

impl Resource {
    pub fn load_container<CO>(
        container: CO,
        ctx: &impl Context,
    ) -> ResourceContainerLoadingFuture<CO>
    where
        CO: ContainedResourcesAccess,
    {
        ResourceContainerLoadingFuture::start_loading(container, ctx)
    }
}

pub struct ResourceContainerLoadingFuture<C>
where
    C: ContainedResourcesAccess,
{
    inner: Option<InnerFuture<C>>,
}

struct InnerFuture<C>
where
    C: ContainedResourcesAccess,
{
    container: C,
    keys: Vec<<C::Key as ToOwned>::Owned>,
    futs: JoinAll<Vec<SendBoxFuture<MaybeEncData, ResourceLoadingError>>>,
}

impl<CO> ResourceContainerLoadingFuture<CO>
where
    CO: ContainedResourcesAccess,
{
    pub fn start_loading(container: CO, ctx: &impl Context) -> Self {
        let mut keys = Vec::new();
        let mut futs = Vec::new();

        container.visit_resources(&mut |key, resource| {
            if let &Resource::Source(ref source) = resource {
                let fut = ctx.load_resource(source);
                futs.push(fut);
                keys.push(key.to_owned());
            }
        });

        let futs = future::join_all(futs);

        ResourceContainerLoadingFuture {
            inner: Some(InnerFuture {
                container,
                keys,
                futs,
            }),
        }
    }
}

impl<C> Future for ResourceContainerLoadingFuture<C>
where
    C: ContainedResourcesAccess,
{
    type Item = C;
    type Error = ResourceLoadingError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let loaded;

        if let Some(loading) = self.inner.as_mut().map(|inner| &mut inner.futs) {
            loaded = try_ready!(loading.poll());
        } else {
            panic!("future called after it resolved");
        };

        //UNWRAP_SAFE: can only be reached if it was some
        let InnerFuture {
            mut container,
            keys,
            futs: _,
        } = self.inner.take().unwrap();

        for (key, new_resource) in keys.into_iter().zip(loaded.into_iter()) {
            container.access_resource_mut(key.borrow(), |resource_ref| {
                if let Some(resource_ref) = resource_ref {
                    mem::replace(resource_ref, new_resource.to_resource());
                }
            })
        }

        Ok(Async::Ready(container))
    }
}
