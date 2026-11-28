//! Canonical typed `kiln://` buffer URIs, perspectives, and bijective URL projection.

use alloc::format;
use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};

/// Exact lowercase hexadecimal length of a BLAKE3 content identity.
pub const BLAKE3_HEX_LEN: usize = 64;
/// Maximum admitted UTF-8 byte length of a named path/slug/change segment.
pub const MAX_SEGMENT_BYTES: usize = 256;
/// Maximum admitted UTF-8 byte length of an entire serialized query string.
pub const MAX_QUERY_BYTES: usize = 2048;

/// Top-level workspace perspective (`1`–`4` in `NORMAL` mode).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Perspective {
    /// `1`: 5-second factory pulse, priority attention triage, delivery spine, fleet pulse.
    #[default]
    Cockpit,
    /// `2`: Multi-agent live Herdr mosaic, swarm board, single-target steering bar.
    Mosaic,
    /// `3`: `jj` stack rail, unified/split diff, inline threads, exact-candidate CI & merge queue.
    Review,
    /// `4`: Failure-first CI DAG & logs, machines, active claim fences.
    CiFleet,
}

impl Perspective {
    /// Canonical lowercase slug used in URL query state and modeline.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Cockpit => "cockpit",
            Self::Mosaic => "mosaic",
            Self::Review => "review",
            Self::CiFleet => "ci-fleet",
        }
    }

    /// Number key (`1`..=`4`) bound to this perspective.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Cockpit => 1,
            Self::Mosaic => 2,
            Self::Review => 3,
            Self::CiFleet => 4,
        }
    }

    /// Parse a perspective from its canonical URL slug.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "cockpit" => Some(Self::Cockpit),
            "mosaic" => Some(Self::Mosaic),
            "review" => Some(Self::Review),
            "ci-fleet" => Some(Self::CiFleet),
            _ => None,
        }
    }

    /// Default primary collection buffer for this perspective.
    #[must_use]
    pub fn default_buffer(self) -> BufferUri {
        match self {
            Self::Cockpit => BufferUri::Triage,
            Self::Mosaic => BufferUri::Attempts,
            Self::Review => BufferUri::Queue {
                repo: String::new(),
            },
            Self::CiFleet => BufferUri::Runs,
        }
    }
}

/// Exact domain object kind carried on every typed selection and cross-jump.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Escalation,
    Decision,
    Claim,
    Attempt,
    HerdrPane,
    CairnChange,
    JjChange,
    CiRun,
    Machine,
    Repository,
    Swarm,
}

/// Rejection error for malformed `kiln://` URIs or query parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UriError {
    TooLong,
    InvalidCharacters,
    InvalidBlake3,
    InvalidFence,
    UnknownSchemeOrKind,
    MissingField(&'static str),
}

/// Addressable buffer inside the Kiln workspace (collections and exact-identity objects).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BufferUri {
    // Collections
    Triage,
    Fleet,
    Claims,
    Attempts,
    Runs,
    Repos,
    CairnIndex {
        repo: String,
    },
    Queue {
        repo: String,
    },
    SwarmBoard {
        board: String,
    },
    // Exact-identity objects
    HerdrPane {
        machine: String,
        pane_id: String,
    },
    Attempt {
        id: String,
    },
    Claim {
        id: String,
        fence: u64,
    },
    CairnChange {
        repo: String,
        change: String,
    },
    JjChange {
        repo: String,
        change_id: String,
        rev: String,
    },
    CiRun {
        id: String,
    },
    Decision {
        id: String,
    },
    Machine {
        name: String,
    },
}

/// Returns `true` iff `value` is a valid 64-character lowercase hexadecimal BLAKE3 digest.
#[must_use]
pub fn is_valid_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Validates a non-empty bounded segment free of control characters and path separators.
pub fn validate_segment(value: &str, allow_slash: bool) -> Result<(), UriError> {
    if value.len() > MAX_SEGMENT_BYTES {
        return Err(UriError::TooLong);
    }
    if value
        .bytes()
        .any(|b| b < 0x20 || b == 0x7f || b == b'&' || b == b'=' || b == b'#')
    {
        return Err(UriError::InvalidCharacters);
    }
    if !allow_slash && value.contains('/') {
        return Err(UriError::InvalidCharacters);
    }
    Ok(())
}

impl BufferUri {
    /// Validate that all fields of this URI obey Kiln exact-identity and bounding rules.
    pub fn validate(&self) -> Result<(), UriError> {
        match self {
            Self::Triage
            | Self::Fleet
            | Self::Claims
            | Self::Attempts
            | Self::Runs
            | Self::Repos => Ok(()),
            Self::CairnIndex { repo } | Self::Queue { repo } => {
                if !repo.is_empty() {
                    validate_segment(repo, true)?;
                }
                Ok(())
            }
            Self::SwarmBoard { board } => {
                if board.is_empty() {
                    return Err(UriError::MissingField("board"));
                }
                validate_segment(board, false)
            }
            Self::HerdrPane { machine, pane_id } => {
                if machine.is_empty() {
                    return Err(UriError::MissingField("machine"));
                }
                if pane_id.is_empty() {
                    return Err(UriError::MissingField("pane_id"));
                }
                validate_segment(machine, false)?;
                validate_segment(pane_id, false)
            }
            Self::Attempt { id } | Self::CiRun { id } | Self::Decision { id } => {
                if !is_valid_blake3_hex(id) {
                    return Err(UriError::InvalidBlake3);
                }
                Ok(())
            }
            Self::Claim { id, fence } => {
                if !is_valid_blake3_hex(id) {
                    return Err(UriError::InvalidBlake3);
                }
                if *fence == 0 {
                    return Err(UriError::InvalidFence);
                }
                Ok(())
            }
            Self::CairnChange { repo, change } => {
                if repo.is_empty() {
                    return Err(UriError::MissingField("repo"));
                }
                if change.is_empty() {
                    return Err(UriError::MissingField("change"));
                }
                validate_segment(repo, true)?;
                validate_segment(change, false)
            }
            Self::JjChange {
                repo,
                change_id,
                rev,
            } => {
                if repo.is_empty() {
                    return Err(UriError::MissingField("repo"));
                }
                if change_id.is_empty() {
                    return Err(UriError::MissingField("change_id"));
                }
                if rev.is_empty() {
                    return Err(UriError::MissingField("rev"));
                }
                validate_segment(repo, true)?;
                validate_segment(change_id, false)?;
                validate_segment(rev, false)
            }
            Self::Machine { name } => {
                if name.is_empty() {
                    return Err(UriError::MissingField("name"));
                }
                validate_segment(name, false)
            }
        }
    }

    /// Format this buffer as its canonical `kiln://` URI string.
    #[must_use]
    pub fn to_uri_string(&self) -> String {
        match self {
            Self::Triage => "kiln://triage".to_string(),
            Self::Fleet => "kiln://fleet".to_string(),
            Self::Claims => "kiln://claims".to_string(),
            Self::Attempts => "kiln://attempts".to_string(),
            Self::Runs => "kiln://runs".to_string(),
            Self::Repos => "kiln://repos".to_string(),
            Self::CairnIndex { repo } => {
                if repo.is_empty() {
                    "kiln://cairn".to_string()
                } else {
                    format!("kiln://cairn/{repo}")
                }
            }
            Self::Queue { repo } => {
                if repo.is_empty() {
                    "kiln://queue".to_string()
                } else {
                    format!("kiln://queue/{repo}")
                }
            }
            Self::SwarmBoard { board } => format!("kiln://swarm/{board}"),
            Self::HerdrPane { machine, pane_id } => format!("kiln://pane/{machine}/{pane_id}"),
            Self::Attempt { id } => format!("kiln://attempt/{id}"),
            Self::Claim { id, fence } => format!("kiln://claim/{id}@{fence}"),
            Self::CairnChange { repo, change } => format!("kiln://cairn-change/{repo}/{change}"),
            Self::JjChange {
                repo,
                change_id,
                rev,
            } => format!("kiln://jj/{repo}/{change_id}@{rev}"),
            Self::CiRun { id } => format!("kiln://run/{id}"),
            Self::Decision { id } => format!("kiln://decision/{id}"),
            Self::Machine { name } => format!("kiln://machine/{name}"),
        }
    }

    /// Parse a canonical `kiln://` URI string with strict fail-closed validation.
    pub fn parse_uri(input: &str) -> Result<Self, UriError> {
        if input.len() > MAX_QUERY_BYTES {
            return Err(UriError::TooLong);
        }
        let rest = input
            .strip_prefix("kiln://")
            .ok_or(UriError::UnknownSchemeOrKind)?;
        let (head, tail) = match rest.split_once('/') {
            Some((h, t)) => (h, Some(t)),
            None => (rest, None),
        };
        let uri = match (head, tail) {
            ("triage", None) => Self::Triage,
            ("fleet", None) => Self::Fleet,
            ("claims", None) => Self::Claims,
            ("attempts", None) => Self::Attempts,
            ("runs", None) => Self::Runs,
            ("repos", None) => Self::Repos,
            ("cairn", None) => Self::CairnIndex {
                repo: String::new(),
            },
            ("cairn", Some(repo)) => Self::CairnIndex {
                repo: repo.to_string(),
            },
            ("queue", None) => Self::Queue {
                repo: String::new(),
            },
            ("queue", Some(repo)) => Self::Queue {
                repo: repo.to_string(),
            },
            ("swarm", Some(board)) => Self::SwarmBoard {
                board: board.to_string(),
            },
            ("pane", Some(rest)) => {
                let (machine, pane_id) = rest
                    .split_once('/')
                    .ok_or(UriError::MissingField("pane_id"))?;
                Self::HerdrPane {
                    machine: machine.to_string(),
                    pane_id: pane_id.to_string(),
                }
            }
            ("attempt", Some(id)) => Self::Attempt { id: id.to_string() },
            ("claim", Some(spec)) => {
                let (id, fence_str) = spec
                    .split_once('@')
                    .ok_or(UriError::MissingField("fence"))?;
                let fence = fence_str
                    .parse::<u64>()
                    .map_err(|_| UriError::InvalidFence)?;
                Self::Claim {
                    id: id.to_string(),
                    fence,
                }
            }
            ("cairn-change", Some(spec)) => {
                let (repo, change) = spec
                    .rsplit_once('/')
                    .ok_or(UriError::MissingField("change"))?;
                Self::CairnChange {
                    repo: repo.to_string(),
                    change: change.to_string(),
                }
            }
            ("jj", Some(spec)) => {
                let (left, rev) = spec.rsplit_once('@').ok_or(UriError::MissingField("rev"))?;
                let (repo, change_id) = left
                    .rsplit_once('/')
                    .ok_or(UriError::MissingField("change_id"))?;
                Self::JjChange {
                    repo: repo.to_string(),
                    change_id: change_id.to_string(),
                    rev: rev.to_string(),
                }
            }
            ("run", Some(id)) => Self::CiRun { id: id.to_string() },
            ("decision", Some(id)) => Self::Decision { id: id.to_string() },
            ("machine", Some(name)) => Self::Machine {
                name: name.to_string(),
            },
            _ => return Err(UriError::UnknownSchemeOrKind),
        };
        uri.validate()?;
        Ok(uri)
    }

    /// Project perspective + buffer to a browser-safe query string (preserving `<a href>` deep links).
    #[must_use]
    pub fn to_query_string(&self, perspective: Perspective) -> String {
        format!("p={}&buf={}", perspective.slug(), self.to_uri_string())
    }

    /// Restore `(Perspective, BufferUri)` from a browser query string (without leading `?`).
    pub fn from_query_string(query: &str) -> Result<(Perspective, Self), UriError> {
        let stripped = query.strip_prefix('?').unwrap_or(query);
        if stripped.len() > MAX_QUERY_BYTES {
            return Err(UriError::TooLong);
        }
        let mut perspective = Perspective::Cockpit;
        let mut buf: Option<Self> = None;
        for part in stripped.split('&') {
            if part.is_empty() {
                continue;
            }
            let Some((k, v)) = part.split_once('=') else {
                continue;
            };
            match k {
                "p" => {
                    perspective = Perspective::from_slug(v).ok_or(UriError::UnknownSchemeOrKind)?;
                }
                "buf" => {
                    buf = Some(Self::parse_uri(v)?);
                }
                _ => {}
            }
        }
        let resolved = buf.unwrap_or_else(|| perspective.default_buffer());
        Ok((perspective, resolved))
    }

    /// Short human-readable title for the 26–28px pane header.
    #[must_use]
    pub fn header_title(&self) -> String {
        match self {
            Self::Triage => "ESCALATIONS & TRIAGE".to_string(),
            Self::Fleet => "FLEET PULSE".to_string(),
            Self::Claims => "ACTIVE CLAIM FENCES".to_string(),
            Self::Attempts => "AGENT ATTEMPTS".to_string(),
            Self::Runs => "CI RUNS".to_string(),
            Self::Repos => "REPOSITORIES".to_string(),
            Self::CairnIndex { repo } => {
                if repo.is_empty() {
                    "CAIRN SPECS".to_string()
                } else {
                    format!("CAIRN: {repo}")
                }
            }
            Self::Queue { repo } => {
                if repo.is_empty() {
                    "REVIEW & MERGE QUEUE".to_string()
                } else {
                    format!("QUEUE: {repo}")
                }
            }
            Self::SwarmBoard { board } => format!("SWARM: {board}"),
            Self::HerdrPane { machine, pane_id } => format!("HERDR: {machine} ({pane_id})"),
            Self::Attempt { id } => format!("ATTEMPT: {}", short_id(id)),
            Self::Claim { id, fence } => format!("CLAIM: {} #{fence}", short_id(id)),
            Self::CairnChange { change, .. } => format!("CAIRN: {change}"),
            Self::JjChange { change_id, .. } => format!("DIFF: jj:{change_id}"),
            Self::CiRun { id } => format!("RUN: {}", short_id(id)),
            Self::Decision { id } => format!("DECISION: {}", short_id(id)),
            Self::Machine { name } => format!("MACHINE: {name}"),
        }
    }
}

/// Safe 8-character prefix for displaying exact BLAKE3 digests without slice indexing panics.
#[must_use]
pub fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}
