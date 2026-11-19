#![allow(dead_code)]
//! Shared fixtures: configuration documents, enrollment, work and scripted
//! executables written once per test binary.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use kiln_core::implementation::{
    AcceptanceObligation, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, SourceRef,
    TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use serde_json::{Value, json};

pub const COORDINATOR: &str = "britton-desktop";
pub const PEER: &str = "aspen3";
pub const PEER_NODE: &str = "aspen3-1.bison-tailor.ts.net.";
pub const REPOSITORY: &str = "onix/kiln";
pub const ACTOR: &str = "operator";

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

pub fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_owned()]),
        actors: BTreeSet::from([ACTOR.to_owned()]),
    }
}

/// Write an executable once per test process, before any test spawns it:
/// writing one while another thread forks races into `ETXTBSY`.
pub fn executable(name: &str, script: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-fleet-{name}-{}", std::process::id()));
    if !path.exists() {
        let staging = path.with_extension(format!("{:?}.tmp", std::thread::current().id()));
        fs::write(&staging, script).unwrap_or_else(|_| unreachable!());
        let mut permissions = fs::metadata(&staging)
            .unwrap_or_else(|_| unreachable!())
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&staging, permissions).unwrap_or_else(|_| unreachable!());
        fs::rename(&staging, &path).unwrap_or_else(|_| unreachable!());
    }
    path
}

/// A claims-only configuration document rooted in `root`.
pub fn config(root: &Path, tailscale: &Path) -> Value {
    json!({
        "schema": "kiln.fleet.v1",
        "machine": COORDINATOR,
        "listen": ["127.0.0.1:8095"],
        "peers": { PEER_NODE: PEER, "aspen1-1.bison-tailor.ts.net.": "aspen1" },
        "operators": [COORDINATOR],
        "tailscale": tailscale,
        "implementer": {
            "database": root.join("implementer.sqlite"),
            "enrollment": root.join("enrollment.json"),
        },
        "inventory": root.join("inventory.json"),
        "swarm": null,
        "capture": null,
        "policy": {},
        "dispatch": { "enabled": false, "kinds": [] },
    })
}

pub fn write_enrollment(root: &Path) -> TestResult {
    fs::write(
        root.join("enrollment.json"),
        serde_json::to_vec(&enrollment())?,
    )?;
    Ok(())
}

/// Interactive work on `REPOSITORY` at the exact `base` commit.
pub fn work(base: &str, profile: &ContentId, paths: &[&str]) -> TestResult<ImplementationWork> {
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
        revision: 1,
        actor: ACTOR.to_owned(),
        repository: REPOSITORY.to_owned(),
        project: "kiln".to_owned(),
        base: Some(ExternalRef::new("git", "commit", base).map_err(|error| error.message)?),
        title: "Fix the value".to_owned(),
        requested_outcome: "src/value reads accepted".to_owned(),
        acceptance: vec![AcceptanceObligation {
            id: "case".to_owned(),
            statement: "src/value reads accepted".to_owned(),
        }],
        dependencies: Vec::new(),
        permitted_paths: paths.iter().map(|path| (*path).to_owned()).collect(),
        permitted_effects: BTreeSet::from([
            "write_source".to_owned(),
            kiln_core::implementation::INTERACTIVE_AGENT_EFFECT.to_owned(),
        ]),
        verification_profile: profile.clone(),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 120,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#7")
                .map_err(|error| error.message)?,
            native_id: "onix/kiln#7".to_owned(),
            version: content('1'),
        }],
    })
}
