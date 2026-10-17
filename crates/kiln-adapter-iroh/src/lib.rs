//! Authenticated Iroh transport for the existing native Lattice workflow contract.
//! CI consumers use their existing Unix workflow port through a controller-local proxy.

mod profile;
mod transport;

pub use profile::{ALPN, IROH_VERSION, PROFILE_SCHEMA, Profile};
pub use transport::{IrohClient, bind_endpoint, serve_proxy, serve_worker};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidProfile,
    PeerDenied,
    InvalidRequest,
    InvalidResponse,
    FrameTooLarge,
    Transport,
    Deadline,
    LocalSocket,
    Provider,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidProfile => "Iroh workflow profile is not admitted",
            Self::PeerDenied => "Iroh workflow peer is not enrolled",
            Self::InvalidRequest => "Iroh workflow request is not admitted",
            Self::InvalidResponse => "Iroh workflow response is not admitted",
            Self::FrameTooLarge => "Iroh workflow frame exceeds its bound",
            Self::Transport => "Iroh workflow transport failed; recover any possible dispatch",
            Self::Deadline => "Iroh workflow deadline expired; recover any possible dispatch",
            Self::LocalSocket => "Iroh workflow local socket failed",
            Self::Provider => "Iroh worker Lattice exchange failed; recover any possible dispatch",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for Error {}
