//! Pure Kiln resource grammar: canonical request framing and segment-aligned grant
//! prefixes. No keys, clocks, stores or other I/O.
//!
//! Every string segment is `<decimal byte length>/<lowercase hex bytes>/`, every
//! number is canonical decimal followed by `/`, and every digest is 64 lowercase hex
//! characters followed by `/`. Path scope markers are single letters, so they can
//! never collide with a decimal length. A byte prefix that stops on a segment
//! boundary therefore names one exact subtree of the grammar.

use super::Denied;
use super::source::{PathScope, SourceAttempt};
use std::fmt::Write as _;

pub(super) const IMPLEMENTATION_V1: &str = "kiln://implementation/v1/";
pub(super) const IMPLEMENTATION_V2: &str = "kiln://implementation/v2/";
pub(super) const SOURCE_V1: &str = "kiln://source/v1/";
/// Explicit delegation ability for `kiln://implementation/v2/` chains.
pub(super) const IMPLEMENTATION_DELEGATE_ABILITY: &str = "kiln/delegate";
/// Explicit delegation ability for `kiln://source/v1/` chains.
pub(super) const SOURCE_DELEGATE_ABILITY: &str = "kiln/source/delegate";

pub(super) const MAX_FIELD_BYTES: usize = 512;
pub(super) const MAX_COMPONENT_BYTES: usize = 255;
pub(super) const MAX_COMPONENTS: usize = 64;
pub(super) const MAX_RESOURCE_BYTES: usize = 4_096;
pub(super) const DIGEST_BYTES: usize = 32;

const ENTRY_MARKER: &[u8] = b"e";
const SUBTREE_MARKER: &[u8] = b"s";
const NO_ATTEMPT_MARKER: &[u8] = b"n";
const ATTEMPT_MARKER: &[u8] = b"a";
const HEX: &[u8; 16] = b"0123456789abcdef";

/// One grammar position of an attenuable Kiln family.
#[derive(Debug, Clone, Copy)]
enum Segment {
    Text,
    Number,
    Digest,
    /// `n/` (no attempt) or `a/<attempt text>/<fence number>/`.
    Context,
    /// Zero or more `<component text>` segments, then `e/` (exact entry) or `s/` (subtree).
    Path,
}

/// An attenuable resource family: its prefix, grammar and explicit delegate ability.
#[derive(Debug, Clone, Copy)]
pub(super) struct Family {
    prefix: &'static str,
    pub(super) delegate: &'static str,
    grammar: &'static [Segment],
}

/// `audience/generation/project/work/work_version/attempt/fence/target/digest/candidate`.
pub(super) const IMPLEMENTATION_V2_FAMILY: Family = Family {
    prefix: IMPLEMENTATION_V2,
    delegate: IMPLEMENTATION_DELEGATE_ABILITY,
    grammar: &[
        Segment::Text,
        Segment::Number,
        Segment::Text,
        Segment::Text,
        Segment::Number,
        Segment::Text,
        Segment::Number,
        Segment::Text,
        Segment::Digest,
        Segment::Digest,
    ],
};

/// `audience/generation/repository/context/path…/terminal/snapshot/view/digest`.
pub(super) const SOURCE_V1_FAMILY: Family = Family {
    prefix: SOURCE_V1,
    delegate: SOURCE_DELEGATE_ABILITY,
    grammar: &[
        Segment::Text,
        Segment::Number,
        Segment::Text,
        Segment::Context,
        Segment::Path,
        Segment::Text,
        Segment::Text,
        Segment::Digest,
    ],
};

impl Family {
    /// Return whether a grant resource is inside this family, pins the audience and
    /// generation, and stops exactly on a segment boundary. A grant that ends inside a
    /// segment would cover unrelated siblings (`project` → `projectx`), and a grant that
    /// stops before the generation would survive a generation bump, so neither
    /// authorizes. Every family grammar starts with `text(audience) number(generation)`.
    pub(super) fn aligned(self, grant: &str) -> bool {
        grant
            .strip_prefix(self.prefix)
            .and_then(|rest| text(rest.as_bytes()))
            .and_then(number)
            .is_some_and(|rest| walk(rest, &self.grammar[PINNED_SEGMENTS..]))
    }
}

/// Audience and generation: the segments every attenuable grant must pin.
const PINNED_SEGMENTS: usize = 2;

/// Admit one bounded ASCII request field (the v1 field rule).
pub(super) fn admit_text(field: &str) -> Result<(), Denied> {
    if field.is_empty()
        || field.len() > MAX_FIELD_BYTES
        || field.trim() != field
        || !field.is_ascii()
        || field.bytes().any(|byte| byte.is_ascii_control())
    {
        Err(Denied)
    } else {
        Ok(())
    }
}

/// Admit one normalized path component: non-empty, bounded, no separator, no control
/// character and never `.` or `..`. UTF-8 is framed by bytes, so no normalization is
/// applied or implied.
pub(super) fn admit_component(component: &str) -> Result<(), Denied> {
    if component.is_empty()
        || component.len() > MAX_COMPONENT_BYTES
        || component == "."
        || component == ".."
        || component.contains('/')
        || component.chars().any(char::is_control)
    {
        Err(Denied)
    } else {
        Ok(())
    }
}

fn push_hex(out: &mut String, bytes: &[u8]) {
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

fn push_framed(out: &mut String, bytes: &[u8]) -> Result<(), Denied> {
    // Hex encoding prevents delimiter/Unicode normalization ambiguity.
    write!(out, "{}/", bytes.len()).map_err(|_| Denied)?;
    push_hex(out, bytes);
    out.push('/');
    Ok(())
}

pub(super) fn push_text(out: &mut String, field: &str) -> Result<(), Denied> {
    admit_text(field)?;
    push_framed(out, field.as_bytes())
}

fn push_component(out: &mut String, component: &str) -> Result<(), Denied> {
    admit_component(component)?;
    push_framed(out, component.as_bytes())
}

pub(super) fn push_number(out: &mut String, value: u64) -> Result<(), Denied> {
    write!(out, "{value}/").map_err(|_| Denied)
}

pub(super) fn push_digest(out: &mut String, digest: &[u8; DIGEST_BYTES]) {
    push_hex(out, digest);
    out.push('/');
}

fn push_marker(out: &mut String, marker: &[u8]) {
    out.extend(marker.iter().copied().map(char::from));
    out.push('/');
}

pub(super) fn bounded(resource: String) -> Result<String, Denied> {
    if resource.len() > MAX_RESOURCE_BYTES {
        Err(Denied)
    } else {
        Ok(resource)
    }
}

/// The frozen `kiln://implementation/v1/` framing. Its bytes MUST NOT change.
pub(super) fn implementation_v1(request: &super::Request) -> Result<String, Denied> {
    let mut resource = String::from(IMPLEMENTATION_V1);
    for field in [
        request.subject.as_str(),
        request.audience.as_str(),
        request.project.as_str(),
        request.work.as_str(),
        request.attempt.as_str(),
        request.target.as_str(),
    ] {
        push_text(&mut resource, field)?;
    }
    for value in [request.work_version, request.fence, request.generation] {
        push_number(&mut resource, value)?;
    }
    push_digest(&mut resource, &request.request_digest);
    push_hex(&mut resource, &request.candidate);
    bounded(resource)
}

/// Optional v2 ceiling segments after audience and generation, in grammar order.
pub(super) enum Ceiling<'a> {
    Text(Option<&'a str>),
    Number(Option<u64>),
}

/// Frame a `kiln://implementation/v2/` prefix. A later segment without every
/// earlier one would not be a byte prefix of any request, so it denies.
pub(super) fn implementation_v2_prefix(
    audience: &str,
    generation: u64,
    ceiling: &[Ceiling<'_>],
) -> Result<String, Denied> {
    if generation == 0 {
        return Err(Denied);
    }
    let mut resource = String::from(IMPLEMENTATION_V2);
    push_text(&mut resource, audience)?;
    push_number(&mut resource, generation)?;
    let mut open = true;
    for segment in ceiling {
        match (open, segment) {
            (true, Ceiling::Text(Some(text))) => push_text(&mut resource, text)?,
            (true, Ceiling::Number(Some(number))) => push_number(&mut resource, *number)?,
            (_, Ceiling::Text(None) | Ceiling::Number(None)) => open = false,
            (false, Ceiling::Text(Some(_)) | Ceiling::Number(Some(_))) => return Err(Denied),
        }
    }
    bounded(resource)
}

/// The attenuable `kiln://implementation/v2/` framing; the holder is not framed.
pub(super) fn implementation_v2(request: &super::Request) -> Result<String, Denied> {
    admit_text(&request.subject)?;
    let mut resource = implementation_v2_prefix(
        &request.audience,
        request.generation,
        &[
            Ceiling::Text(Some(&request.project)),
            Ceiling::Text(Some(&request.work)),
            Ceiling::Number(Some(request.work_version)),
            Ceiling::Text(Some(&request.attempt)),
            Ceiling::Number(Some(request.fence)),
        ],
    )?;
    push_text(&mut resource, &request.target)?;
    push_digest(&mut resource, &request.request_digest);
    push_digest(&mut resource, &request.candidate);
    bounded(resource)
}

/// Frame a `kiln://source/v1/` scope through the path and optional terminal marker.
pub(super) fn source_prefix(
    audience: &str,
    generation: u64,
    repository: &str,
    attempt: Option<&SourceAttempt>,
    path: &[String],
    terminal: Option<PathScope>,
) -> Result<String, Denied> {
    if generation == 0
        || path.len() > MAX_COMPONENTS
        || (path.is_empty() && terminal == Some(PathScope::Entry))
    {
        return Err(Denied);
    }
    let mut resource = String::from(SOURCE_V1);
    push_text(&mut resource, audience)?;
    push_number(&mut resource, generation)?;
    push_text(&mut resource, repository)?;
    match attempt {
        None => push_marker(&mut resource, NO_ATTEMPT_MARKER),
        Some(attempt) => {
            push_marker(&mut resource, ATTEMPT_MARKER);
            push_text(&mut resource, &attempt.attempt)?;
            push_number(&mut resource, attempt.fence)?;
        }
    }
    for component in path {
        push_component(&mut resource, component)?;
    }
    match terminal {
        None => {}
        Some(PathScope::Entry) => push_marker(&mut resource, ENTRY_MARKER),
        Some(PathScope::Subtree) => push_marker(&mut resource, SUBTREE_MARKER),
    }
    bounded(resource)
}

/// The exact `kiln://source/v1/` request framing; the holder is not framed.
pub(super) fn source_v1(request: &super::SourceRequest) -> Result<String, Denied> {
    admit_text(&request.subject)?;
    let mut resource = source_prefix(
        &request.audience,
        request.generation,
        &request.repository,
        request.attempt.as_ref(),
        &request.path,
        Some(request.scope),
    )?;
    push_text(&mut resource, &request.snapshot)?;
    push_text(&mut resource, &request.view)?;
    push_digest(&mut resource, &request.request_digest);
    bounded(resource)
}

/// Return whether `rest` is a sequence of complete grammar segments (possibly none).
fn walk(mut rest: &[u8], grammar: &[Segment]) -> bool {
    for segment in grammar {
        if rest.is_empty() {
            return true;
        }
        match consume(*segment, rest) {
            Some(next) => rest = next,
            None => return false,
        }
    }
    rest.is_empty()
}

/// Consume one segment. Compound segments may stop at an inner boundary, which is
/// reported as an empty remainder.
fn consume(segment: Segment, rest: &[u8]) -> Option<&[u8]> {
    match segment {
        Segment::Text => text(rest),
        Segment::Number => number(rest),
        Segment::Digest => digest(rest),
        Segment::Context => {
            let (marker, rest) = token(rest)?;
            if marker == NO_ATTEMPT_MARKER {
                Some(rest)
            } else if marker == ATTEMPT_MARKER {
                if rest.is_empty() {
                    return Some(rest);
                }
                let rest = text(rest)?;
                if rest.is_empty() {
                    return Some(rest);
                }
                number(rest)
            } else {
                None
            }
        }
        Segment::Path => {
            let mut rest = rest;
            let mut components = 0_usize;
            loop {
                if rest.is_empty() {
                    return Some(rest);
                }
                let (marker, after) = token(rest)?;
                if marker == SUBTREE_MARKER || (marker == ENTRY_MARKER && components > 0) {
                    return Some(after);
                }
                rest = text(rest)?;
                components += 1;
                if components > MAX_COMPONENTS {
                    return None;
                }
            }
        }
    }
}

fn token(rest: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = rest.iter().position(|byte| *byte == b'/')?;
    Some((&rest[..end], &rest[end + 1..]))
}

fn canonical_decimal(digits: &[u8]) -> Option<u64> {
    if digits.is_empty()
        || (digits.len() > 1 && digits[0] == b'0')
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

fn lower_hex(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn number(rest: &[u8]) -> Option<&[u8]> {
    let (digits, rest) = token(rest)?;
    canonical_decimal(digits).map(|_| rest)
}

fn text(rest: &[u8]) -> Option<&[u8]> {
    let (length, rest) = token(rest)?;
    let length = usize::try_from(canonical_decimal(length)?).ok()?;
    if length == 0 || length > MAX_FIELD_BYTES {
        return None;
    }
    let (hex, rest) = token(rest)?;
    (hex.len() == length * 2 && lower_hex(hex)).then_some(rest)
}

fn digest(rest: &[u8]) -> Option<&[u8]> {
    let (hex, rest) = token(rest)?;
    (hex.len() == DIGEST_BYTES * 2 && lower_hex(hex)).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::{IMPLEMENTATION_V2_FAMILY, SOURCE_V1_FAMILY};

    #[test]
    fn segment_boundaries_after_the_generation_are_the_only_aligned_prefixes() {
        let family = IMPLEMENTATION_V2_FAMILY;
        // Wider than the family, or stopping before the generation, never aligns.
        assert!(!family.aligned(""));
        assert!(!family.aligned("kiln://"));
        assert!(!family.aligned("kiln://implementation/v2/"));
        assert!(!family.aligned("kiln://implementation/v2/1/61/"));
        assert!(!family.aligned("kiln://source/v1/1/61/7/"));
        assert!(family.aligned("kiln://implementation/v2/1/61/7/"));
        assert!(family.aligned("kiln://implementation/v2/1/61/7/1/62/"));
        assert!(!family.aligned("kiln://implementation/v2/1/61/7/1/6"));
        assert!(!family.aligned("kiln://implementation/v2/1/6"));
        assert!(!family.aligned("kiln://implementation/v2/1/61"));
        assert!(!family.aligned("kiln://implementation/v2/1/"));
        assert!(!family.aligned("kiln://implementation/v2/1/6A/"));
        assert!(!family.aligned("kiln://implementation/v2/01/61/"));
        assert!(!family.aligned("kiln://implementation/v2/1/61/07/"));
        assert!(!family.aligned("kiln://implementation/v2/1/61/7"));
    }

    #[test]
    fn source_paths_align_on_components_and_markers_only() {
        let family = SOURCE_V1_FAMILY;
        let repo = "kiln://source/v1/1/61/7/1/72/";
        assert!(family.aligned("kiln://source/v1/1/61/7/"));
        assert!(!family.aligned("kiln://source/v1/1/61/"));
        assert!(family.aligned(repo));
        assert!(family.aligned(&format!("{repo}n/")));
        assert!(family.aligned(&format!("{repo}a/")));
        assert!(family.aligned(&format!("{repo}a/1/78/")));
        assert!(family.aligned(&format!("{repo}a/1/78/5/")));
        assert!(family.aligned(&format!("{repo}n/1/61/1/62/")));
        assert!(family.aligned(&format!("{repo}n/1/61/1/62/e/")));
        assert!(family.aligned(&format!("{repo}n/1/61/s/")));
        assert!(family.aligned(&format!("{repo}n/s/")));
        assert!(!family.aligned(&format!("{repo}n/e/")));
        assert!(!family.aligned(&format!("{repo}x/")));
        assert!(!family.aligned(&format!("{repo}n/1/6")));
        assert!(!family.aligned(&format!("{repo}n/1/61/e")));
        assert!(!family.aligned(&format!("{repo}n/1/61/1/62/t/")));
    }
}
