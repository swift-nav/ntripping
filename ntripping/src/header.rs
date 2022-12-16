// https://github.com/rust-lang/rust-clippy/issues/9776
#![allow(clippy::declare_interior_mutable_const)]

pub use hyper::http::header::*;

pub const NTRIP_GGA: HeaderName = HeaderName::from_static("ntrip-gga");
pub const NTRIP_VERSION: HeaderName = HeaderName::from_static("ntrip-version");
pub const SWIFT_CLIENT_ID: HeaderName = HeaderName::from_static("x-swiftnav-client-id");
