//! Fleet orchestration of interactive coding agents, pure rules.
//!
//! Kiln is the claim authority for the agent fleet: it grants fenced, leased
//! claims over repository paths and machines, follows interactive sessions to
//! a candidate handoff, and raises operator decisions. Every function here is
//! a pure decision over recorded state and observations; shells own the
//! store, the herdr transport, the swarm mirror and clocks.

pub mod claims;
pub mod decision;
pub mod git_ai;
pub mod guest_move;
pub mod placement;
pub mod prompt;
pub mod scope;
pub mod session;
pub mod swarm;

pub use claims::*;
pub use decision::*;
pub use git_ai::*;
pub use guest_move::*;
pub use prompt::*;
pub use scope::*;
pub use session::*;
pub use swarm::*;

use alloc::string::ToString;
use alloc::vec::Vec;

use crate::types::{ContentId, CoreResult, Diagnostic};

/// Append one length-prefixed field, as the implementation identities do.
fn push_field(bytes: &mut Vec<u8>, value: &str, field: &str) -> CoreResult<()> {
    let length = u32::try_from(value.len()).map_err(|_| {
        Diagnostic::new(
            "fleet_encoding",
            field,
            "fleet identity input exceeds the encoding bound",
        )
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn content_identity(bytes: &[u8]) -> CoreResult<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string())
}

/// Bounded single-line text: non-empty, within `maximum` bytes, no control
/// characters and no surrounding whitespace.
fn bounded_line(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

/// Bounded token: ASCII, no whitespace or control characters.
fn bounded_token(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

/// First `count` characters of an identity, for human-facing labels.
fn short(identity: &ContentId, count: usize) -> &str {
    identity.as_str().get(..count).unwrap_or(identity.as_str())
}
