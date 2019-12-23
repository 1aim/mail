//! This module provides a type alias and constructor function for an simple context impl.
//!
//! It used the `FsResourceLoader` and `CpuPool` with a `CompositeContext`.
//!
//! Note this module is only available if the `default_impl_cpupool` feature
//! is enabled.
//!
//! # Example
//!
//! ```
//! # extern crate mail_core as mail;
//! # extern crate mail_headers as headers;
//! # use headers::header_components::Domain;
//! # // It's re-exported in the facade under `default_impl`.
//! # use std::str::FromStr;
//! use mail::default_impl::simple_context;
//!
//! # fn main() {
//! //TODO[FEAT]: use parse once the `Domain` component implements `FromStr`.
//! //let domain = "example.com".parse().unwrap();
//! let domain = Domain::from_unchecked("example.com".to_owned());
//! // This normally should be world unique for any usage with the same domain.
//! // This is necessary to generate `Content-Id` and `Message-Id` correctly.
//! let ascii_unique_part = "xm3r2u".parse().unwrap();
//! let ctx = simple_context::new(domain, ascii_unique_part).unwrap();
//! # }
//! ```
//!
use std::io;

use futures_cpupool::{Builder, CpuPool};
use soft_ascii_string::SoftAsciiString;

use headers::header_components::Domain;
use internals::error::EncodingError;

use context::CompositeContext;
use default_impl::{FsResourceLoader, HashedIdGen};

/// Error returned when creating a "simple_context" fails.
#[derive(Debug, Fail)]
pub enum ContextSetupError {
    /// Reading the env variables failed.
    ///
    /// (Mainly getting the current working dir failed).
    #[fail(display = "{}", _0)]
    ReadingEnv(io::Error),

    /// Punny encoding a non us-ascii domain failed.
    #[fail(display = "{}", _0)]
    PunyCodingDomain(EncodingError),
}

/// Type Alias for a the type returned by `simple_context::new`.
pub type Context = CompositeContext<FsResourceLoader, CpuPool, HashedIdGen>;

/// create a new CompositeContext<FsResourceLoader, CpuPool, HashedIdGen>
///
/// It uses the current working directory as root for the `FsResourceLoader`,
/// and the default settings for the `CpuPool`, both the `domain` and
/// `unique_part` are passed to the `HashedIdGen::new` constructor.
///
/// Note that the combination of `unique_part` and `domain` should be world
/// unique. This is needed to generate `Content-Id` and `Message-Id` reliably
/// correctly. This means if you run multiple instances of softer using a context
/// or you create multiple contexts they should _not_ use the same `unique_part`
/// under any circumstances (expect if they use different domains, but then you
/// also should only use domain you actually own).
pub fn new(domain: Domain, unique_part: SoftAsciiString) -> Result<Context, ContextSetupError> {
    let resource_loader =
        FsResourceLoader::with_cwd_root().map_err(|err| ContextSetupError::ReadingEnv(err))?;

    let cpu_pool = Builder::new().create();

    let id_gen = HashedIdGen::new(domain, unique_part)
        .map_err(|err| ContextSetupError::PunyCodingDomain(err))?;

    Ok(CompositeContext::new(resource_loader, cpu_pool, id_gen))
}
