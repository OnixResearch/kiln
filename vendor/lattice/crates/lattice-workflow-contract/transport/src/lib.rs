//! Bounded Unix workflow exchange transport for Lattice consumers.
//!
//! The functional core owns wire identity, admission, linkage, and safe failure
//! classification. The Unix shell owns finite socket I/O and invokes only an
//! explicit application-owned workflow handler.

#![forbid(unsafe_code)]

mod core;
#[path = "service.rs"]
mod handler;
#[cfg(target_os = "linux")]
#[path = "unix.rs"]
mod local;
mod model;
#[path = "frame.rs"]
mod wire;

pub use core::new_request;
pub use core::new_response;
pub use core::request_identity;
pub use core::response_identity;
pub use core::validate_exchange_request;
pub use core::validate_exchange_response;
pub use core::validate_response_limits;
pub use core::validate_transport_profile;

pub use handler::WorkflowExchangeHandler;
pub use handler::WorkflowExchangeService;
#[cfg(target_os = "linux")]
pub use local::UnixClient;
#[cfg(target_os = "linux")]
pub use local::UnixEndpoint;
#[cfg(target_os = "linux")]
pub use local::serve;
#[cfg(target_os = "linux")]
pub use local::serve_listener;
pub use model::*;
pub use wire::FrameFailure;
pub use wire::encode_frame;
pub use wire::read_frame;
pub use wire::write_frame;

#[cfg(test)]
mod tests;
