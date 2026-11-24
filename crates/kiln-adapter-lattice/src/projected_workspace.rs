//! The projected-workspace route check of `kiln-lattice-dispatch` (C9).
//!
//! A dispatch plan that pins a projected workspace runs only under an
//! authority profile that enables projection, only with the
//! `kiln.projected-workspace-route.v1` route derived again for that pin and
//! the plan's own permitted paths, and only when the Lattice worker request
//! names exactly the route's workspace root, read paths and write paths. A
//! plan without a pin takes no route. The check runs after the dispatch
//! authority pre-check and before the worker's credential read, any durable
//! step or transport start: no credential read, plan reservation, sealed
//! request or transport intent.
use std::collections::BTreeSet;
use std::path::Path;

use kiln_core::implementation::ImplementationDispatchPlan;
use kiln_core::implementation::authority::{AuthorityDenialReason, AuthorityProfile};
use kiln_core::projection::EntryOrigin;
use kiln_core::projection::authority::{
    AttemptFence, OriginScope, PathScope, SnapshotRule, SourceAuthority, SourceAuthorityRequest,
    SourceCurrentness, SourceViewOperation, check_request,
};
use kiln_core::projection::materialization::{
    ProjectedWorkspaceRoute, RUNTIME_MOUNTED_ROOTS, store_outside_runtime_mounts,
};
use kiln_implementer::authority_config::{ImplementationAuthorityConfig, ProfileConfig};

/// Source-authority enforcement inputs required for every pinned projected
/// workspace dispatch (`ProjectionEnabled` alone is insufficient).
pub struct SourceDispatchAuthority<'a> {
    pub authority: &'a mut dyn SourceAuthority,
    pub sign_request: &'a dyn Fn(&SourceAuthorityRequest) -> Result<AuthorityProfile, String>,
    pub current: &'a SourceCurrentness,
    pub now_unix_s: u64,
}

impl std::fmt::Debug for SourceDispatchAuthority<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceDispatchAuthority")
            .field("current", &self.current)
            .field("now_unix_s", &self.now_unix_s)
            .finish_non_exhaustive()
    }
}

/// The `kiln-lattice-dispatch` flag naming the route file.
pub const ROUTE_FLAG: &str = "--projected-workspace-route";
/// Largest route file read. A route carries its resolved manifest, whose
/// bound is `MAX_MANIFEST_ENTRIES` entries.
const MAX_ROUTE_BYTES: u64 = 64 * 1024 * 1024;

/// The authority profile selected for one dispatch, as far as a projected
/// workspace is concerned (C1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionAuthority {
    /// No profile is selected; every protected operation denies.
    Absent,
    /// The explicit local-operator compatibility profile. It never
    /// authorizes a projected workspace, whatever else it admits.
    LocalOperator,
    /// A verified profile whose configuration enables projection
    /// (`verified_ucan.projection_enabled`). Only the verified authority
    /// composition selects it.
    ProjectionEnabled,
}

impl ProjectionAuthority {
    /// The projection authority of the profile a dispatch selected: a
    /// `verified_ucan` profile whose configuration enables projection is
    /// `ProjectionEnabled`, the local-operator profile is `LocalOperator`,
    /// and every other selection (a verified profile without projection, or
    /// the absent profile) is `Absent`.
    #[must_use]
    pub fn of_selected(selection: &ImplementationAuthorityConfig) -> Self {
        match &selection.profile {
            ProfileConfig::VerifiedUcan(verified) if verified.projection_enabled => {
                Self::ProjectionEnabled
            }
            ProfileConfig::LocalOperator { .. } => Self::LocalOperator,
            ProfileConfig::VerifiedUcan(_) | ProfileConfig::Absent => Self::Absent,
        }
    }
}

// r[impl kiln.source.projection.route.scenario.dispatch]
/// Refuse a projected dispatch under a profile that does not enable
/// projection, a worker request that differs from the route of the plan's
/// pinned workspace, a route not derived for exactly the plan's permitted
/// paths, a pinned plan without its route, a route for an unpinned plan,
/// or a pinned dispatch whose `SourceAuthority` (`PrepareWorkspace`, read
/// paths `Discover`/`ReadContent`, write paths `ModifySource`/`CreateSource`)
/// is absent, ungranted, revoked, or stale.
pub fn check_route(
    route_file: Option<&Path>,
    plan: &ImplementationDispatchPlan,
    request: &serde_json::Value,
    authority: ProjectionAuthority,
    source_authority: Option<&mut SourceDispatchAuthority<'_>>,
) -> Result<(), String> {
    if plan.workspace.projection.is_none() && route_file.is_none() {
        return Ok(());
    }
    // Keyed on this dispatch: a pinned plan or a supplied route is a
    // projected dispatch, whatever the selected profile was built with.
    if authority != ProjectionAuthority::ProjectionEnabled {
        return Err(format!(
            "{}: a projected workspace runs only under an authority profile that enables projection, never the local-operator or an absent profile; nothing was dispatched",
            AuthorityDenialReason::ProjectionRequiresOperator.as_str()
        ));
    }
    let (pin, path) = match (&plan.workspace.projection, route_file) {
        (None, _) => {
            return Err(
                "a projected workspace route was supplied for a plan without a workspace pin; nothing was dispatched"
                    .into(),
            );
        }
        (Some(_), None) => {
            return Err(
                "the plan pins a projected workspace and requires its route; nothing was dispatched"
                    .into(),
            );
        }
        (Some(pin), Some(path)) => (pin, path),
    };
    let route = read_route(path)?;
    if route.pin != *pin {
        return Err(
            "the projected workspace route belongs to another pin; nothing was dispatched".into(),
        );
    }
    // The route must be the one derived for the pinned manifest and the
    // plan's own permitted paths, not merely a well-formed route of the pin.
    route
        .verify_permitted(&plan.permitted_paths)
        .map_err(|diagnostics| {
            let codes: Vec<String> = diagnostics.into_iter().map(|d| d.code).collect();
            format!(
                "the projected workspace route is invalid or not derived for the plan's permitted paths ({}); nothing was dispatched",
                codes.join(", ")
            )
        })?;
    let strings = |field: &str| -> Option<Vec<String>> {
        request
            .get(field)?
            .as_array()?
            .iter()
            .map(|value| value.as_str().map(str::to_owned))
            .collect()
    };
    let root = request
        .get("workspace_root")
        .and_then(serde_json::Value::as_str);
    let admitted = match (root, strings("read_paths"), strings("write_paths")) {
        (Some(root), Some(read), Some(write)) => route.admits(root, &read, &write),
        _ => false,
    };
    if !admitted || crate::request_network::request_network(request) != Some(false) {
        return Err(
            "the worker request's workspace root, read paths, write paths or network differ from the projected workspace route; nothing was dispatched"
                .into(),
        );
    }
    // The runtime canonicalizes its root: a linked root or ancestor would
    // mount another directory than the pinned overlay.
    let overlay = Path::new(&route.overlay_root);
    let real = std::fs::symlink_metadata(overlay).is_ok_and(|metadata| metadata.is_dir())
        && std::fs::canonicalize(overlay).is_ok_and(|canonical| canonical == overlay);
    if !real {
        return Err(
            "the projected workspace overlay is not a real directory at its route path; nothing was dispatched"
                .into(),
        );
    }
    // Lattice binds each runtime root through any symbolic link, so the store
    // must also lie outside every root's resolved source.
    let store = route
        .store()
        .ok_or("the projected workspace route names no store; nothing was dispatched")?;
    check_store_placement(Path::new(store), &RUNTIME_MOUNTED_ROOTS)?;
    let Some(source_auth) = source_authority else {
        return Err(
            "projected workspace dispatch requires verified SourceAuthority; nothing was dispatched"
                .into(),
        );
    };
    verify_route_source_authority(plan, &route, source_auth)
}

fn verify_route_source_authority(
    plan: &ImplementationDispatchPlan,
    route: &ProjectedWorkspaceRoute,
    source_auth: &mut SourceDispatchAuthority<'_>,
) -> Result<(), String> {
    let request_digest = *blake3::hash(route.identity.as_str().as_bytes()).as_bytes();
    let attempt = Some(AttemptFence {
        attempt: plan.attempt_identity.clone(),
        fence: plan.fence,
    });
    let snapshot = SnapshotRule::Exact {
        revision: route.pin.source.clone(),
    };

    let authorize = |source_auth: &mut SourceDispatchAuthority<'_>,
                     operation: SourceViewOperation,
                     path: PathScope|
     -> Result<(), String> {
        let req = SourceAuthorityRequest {
            operation,
            repository: route.pin.repository.clone(),
            snapshot: snapshot.clone(),
            view_binding: route.pin.manifest.clone(),
            scope: OriginScope::Components { path },
            request_digest,
            attempt: attempt.clone(),
        };
        let profile = (source_auth.sign_request)(&req)
            .map_err(|error| format!("{error}; nothing was dispatched"))?;
        let decision = source_auth
            .authority
            .admit(&req, &profile, source_auth.current, source_auth.now_unix_s)
            .map_err(|denial| {
                format!(
                    "source authority denied ({}: {}); nothing was dispatched",
                    denial.ability,
                    denial.reason.as_str()
                )
            })?;
        let session = decision
            .session
            .ok_or("source authority decision returned no session; nothing was dispatched")?;
        if let Err(denial) = source_auth.authority.revalidate(
            session,
            &req,
            source_auth.current,
            source_auth.now_unix_s,
        ) {
            return Err(format!(
                "source authority revalidation denied ({}: {}); nothing was dispatched",
                denial.ability,
                denial.reason.as_str()
            ));
        }
        let scope = source_auth
            .authority
            .effective_scope(session)
            .cloned()
            .ok_or(
                "source authority session vanished after revalidation; nothing was dispatched",
            )?;
        let checked = check_request(&scope, &req, source_auth.current, source_auth.now_unix_s);
        source_auth.authority.close(session);
        checked.map_err(|reason| {
            format!(
                "source authority check denied ({}); nothing was dispatched",
                reason.as_str()
            )
        })
    };

    // 1. PrepareWorkspace bound to the attempt fence and exact route pin.
    authorize(
        source_auth,
        SourceViewOperation::PrepareWorkspace,
        PathScope::root(),
    )?;

    // 2. Read paths (Discover + ReadContent) over every native manifest origin.
    let mut read_origins = false;
    for entry in &route.manifest.entries {
        if let EntryOrigin::Native { path: origin, .. } = &entry.origin {
            read_origins = true;
            let scope_path = PathScope::from_path(origin, false).ok_or(
                "projected route manifest holds an unsafe origin path; nothing was dispatched",
            )?;
            authorize(
                source_auth,
                SourceViewOperation::Discover,
                scope_path.clone(),
            )?;
            authorize(source_auth, SourceViewOperation::ReadContent, scope_path)?;
        }
    }
    if !read_origins {
        for read_path in &route.read_paths {
            let scope_path = PathScope::from_path(read_path, true)
                .ok_or("projected route read path is unsafe; nothing was dispatched")?;
            authorize(source_auth, SourceViewOperation::Discover, scope_path)?;
        }
    }

    // 3. Write paths: ModifySource on existing writable origins and CreateSource
    // on writable creation destinations.
    for entry in &route.manifest.entries {
        if let EntryOrigin::Native { path: origin, .. } = &entry.origin
            && covered_by_permitted(origin, &plan.permitted_paths)
        {
            let scope_path = PathScope::from_path(origin, false)
                .ok_or("projected route writable origin path is unsafe; nothing was dispatched")?;
            authorize(source_auth, SourceViewOperation::ModifySource, scope_path)?;
        }
    }
    for write in &route.write_paths {
        let prefix = format!("{write}/");
        if route.definition.creation_destinations.iter().any(|cd| {
            cd == write
                || write.starts_with(&format!("{cd}/"))
                || cd.starts_with(&prefix)
                || cd.is_empty()
        }) {
            let probes = mapped_creation_probes(route, write);
            if probes.is_empty() {
                return Err(
                    "projected route creation destination has no matching mount; nothing was dispatched"
                        .into(),
                );
            }
            for source_create_path in probes {
                let scope_path = PathScope::from_path(&source_create_path, false).ok_or(
                    "projected route creation destination is unsafe; nothing was dispatched",
                )?;
                authorize(source_auth, SourceViewOperation::CreateSource, scope_path)?;
            }
        }
    }
    Ok(())
}

fn covered_by_permitted(origin: &str, permitted_paths: &[String]) -> bool {
    permitted_paths.iter().any(|permitted| {
        origin == permitted
            || origin
                .strip_prefix(permitted.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn mapped_creation_probes(route: &ProjectedWorkspaceRoute, write: &str) -> Vec<String> {
    let mut probes = BTreeSet::new();
    for mount in &route.definition.mounts {
        let rest = if mount.destination_prefix.is_empty() {
            Some(write)
        } else if write == mount.destination_prefix {
            Some("")
        } else {
            write
                .strip_prefix(mount.destination_prefix.as_str())
                .and_then(|s| s.strip_prefix('/'))
        };
        if let Some(rest) = rest {
            let mapped = match (mount.source_prefix.is_empty(), rest.is_empty()) {
                (true, _) => rest.to_string(),
                (false, true) => mount.source_prefix.clone(),
                (false, false) => format!("{}/{rest}", mount.source_prefix),
            };
            let probe = if mapped.is_empty() {
                "_dispatch_create".to_string()
            } else {
                format!("{mapped}/_dispatch_create")
            };
            probes.insert(probe);
        } else if mount
            .destination_prefix
            .strip_prefix(write)
            .is_some_and(|tail| tail.starts_with('/'))
        {
            let probe = if mount.source_prefix.is_empty() {
                "_dispatch_create".to_string()
            } else {
                format!("{}/_dispatch_create", mount.source_prefix)
            };
            probes.insert(probe);
        }
    }
    probes.into_iter().collect()
}

/// Refuse a store that is not a canonical directory path, or that lies at or
/// below a runtime-mounted root by name or by its resolved source.
fn check_store_placement(store: &Path, roots: &[&str]) -> Result<(), String> {
    let canonical = std::fs::canonicalize(store)
        .ok()
        .filter(|canonical| canonical == store)
        .and_then(|canonical| canonical.to_str().map(str::to_owned))
        .ok_or(
            "the materialization store is not a canonical directory path; nothing was dispatched",
        )?;
    let resolved = resolve_mount_roots(roots)?;
    if !store_outside_runtime_mounts(&canonical, &resolved) {
        return Err(
            "the materialization store lies at or below a host root the restricted runtime mounts, by name or through a link; nothing was dispatched"
                .into(),
        );
    }
    Ok(())
}

/// The canonical source of each runtime-mounted root present on this host.
/// A root with no directory entry is not bound: Lattice binds a root only
/// when it exists (`src/candidate_worker/sandbox.rs:170,174`). Any other root
/// that cannot be resolved, a dangling link included, refuses the dispatch.
fn resolve_mount_roots(roots: &[&str]) -> Result<Vec<String>, String> {
    let unresolved = |root: &str| {
        format!(
            "the runtime-mounted root {root} cannot be resolved, so the store's exposure is unknown; nothing was dispatched"
        )
    };
    let mut resolved = Vec::new();
    for root in roots {
        match std::fs::symlink_metadata(root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(unresolved(root)),
            Ok(_) => {}
        }
        let canonical = std::fs::canonicalize(root).map_err(|_| unresolved(root))?;
        resolved.push(
            canonical
                .to_str()
                .ok_or_else(|| unresolved(root))?
                .to_owned(),
        );
    }
    Ok(resolved)
}

fn read_route(path: &Path) -> Result<ProjectedWorkspaceRoute, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_ROUTE_BYTES {
        return Err("the projected workspace route is not a bounded ordinary file".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|_| "the projected workspace route is malformed".into())
}

#[cfg(test)]
#[path = "projected_workspace_tests.rs"]
mod tests;
