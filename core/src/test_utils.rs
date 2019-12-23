use lazy_static::lazy_static;

use headers::header_components::Domain;

use crate::default_impl::simple_context::{self, Context, ContextSetupError};

pub struct CtxHolder {
    inner: Result<Context, ContextSetupError>,
}

impl CtxHolder {
    pub fn get(&self) -> Result<&Context, &ContextSetupError> {
        self.inner.as_ref()
    }

    pub fn unwrap(&self) -> &Context {
        self.get().unwrap()
    }

    pub fn expect(&self, msg: &'static str) -> &Context {
        self.get().expect(msg)
    }
}

lazy_static! {
    /// Provides a instance of a impl. of Context _for unit testing_.
    ///
    /// This should never be used in any way in production as it is:
    /// 1. using `example.com` for the domain of content ids
    /// 2. has a hard coded "unique" part so content ids are not
    ///    at all guaranteed to be world unique.
    pub static ref CTX: CtxHolder = {
        let domain = Domain::from_unchecked("example.com".to_owned());
        let ascii_unique_part = "xm3r2u".parse().unwrap();
        let ctx = simple_context::new(domain, ascii_unique_part);
        CtxHolder { inner: ctx }
    };
}
