use std::iter::FromIterator;
use std::mem;

use futures::{Async, Future, Poll};

pub enum AltFuse<F: Future> {
    Future(F),
    Resolved(Result<F::Item, F::Error>),
}

impl<F> Future for AltFuse<F>
where
    F: Future,
{
    type Item = ();
    //TODO[futures/v>=0.2 |rust/! type]: use Never or !
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let result = match *self {
            AltFuse::Resolved(_) => return Ok(Async::Ready(())),
            AltFuse::Future(ref mut fut) => match fut.poll() {
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Ok(Async::Ready(val)) => Ok(val),
                Err(err) => Err(err),
            },
        };

        *self = AltFuse::Resolved(result);
        Ok(Async::Ready(()))
    }
}

pub struct ResolveAll<F>
where
    F: Future,
{
    all: Vec<AltFuse<F>>,
}

impl<F> Future for ResolveAll<F>
where
    F: Future,
{
    type Item = Vec<Result<F::Item, F::Error>>;
    //TODO[futures >= 0.2/rust ! type]: use Never or !
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut any_not_ready = false;
        for fut in self.all.iter_mut() {
            if let Ok(Async::NotReady) = fut.poll() {
                any_not_ready = true;
            }
        }
        if any_not_ready {
            Ok(Async::NotReady)
        } else {
            let results = mem::replace(&mut self.all, Vec::new())
                .into_iter()
                .map(|alt_fuse_fut| match alt_fuse_fut {
                    AltFuse::Resolved(res) => res,
                    AltFuse::Future(_) => unreachable!(),
                })
                .collect();
            Ok(Async::Ready(results))
        }
    }
}

impl<I> FromIterator<I> for ResolveAll<I>
where
    I: Future,
{
    fn from_iter<T>(all: T) -> Self
    where
        T: IntoIterator<Item = I>,
    {
        let all = all.into_iter().map(|fut| AltFuse::Future(fut)).collect();

        ResolveAll { all }
    }
}
