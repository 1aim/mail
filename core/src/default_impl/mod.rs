//! This module provides an number of default implementations for some of the interfaces.
//!
//! For example it provides a default implementation for the context needed
//! to build/encode a mail.
#[cfg(feature = "default_impl_cpupool")]
mod cpupool;
#[cfg(feature = "default_impl_cpupool")]
pub use self::cpupool::*;

mod fs;
pub use self::fs::*;

mod message_id_gen;
pub use self::message_id_gen::*;

#[cfg(all(feature = "default_impl_cpupool"))]
pub mod simple_context;

#[cfg(all(test, not(feature = "default_impl_cpupool")))]
compile_error!("test need following (default) features: default_impl_cpupool, default_impl_fs, default_impl_message_id_gen");

#[cfg(test)]
use headers::header_components::Domain;
#[cfg(test)]
use soft_ascii_string::SoftAsciiString;

#[cfg(test)]
pub type TestContext = simple_context::Context;

//same crate so we can do this ;=)
#[cfg(test)]
pub fn test_context() -> TestContext {
    //TODO crate a test context which does not access the file system
    let domain = Domain::from_unchecked("fooblabar.test".to_owned());
    let unique_part = SoftAsciiString::from_unchecked("CM0U3c412");
    simple_context::new(domain, unique_part).unwrap()
}
