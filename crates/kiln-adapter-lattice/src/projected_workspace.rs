//! The projected-workspace route check of `kiln-lattice-dispatch` (C9).
//!
//! A dispatch plan that pins a projected workspace runs only under an
//! authority profile that enables projection, only with the
//! `kiln.projected-workspace-route.v1` route derived again for that pin and
//! the plan's own permitted paths, and only when the Lattice worker request
//! names exactly the route's workspace root, read paths and write paths. A
//! plan without a pin takes no route. The check runs before any durable step
//! or transport start: no plan reservation, sealed request or transport
//! intent.
use std::path::Path;

use kiln_core::implementation::ImplementationDispatchPlan;
use kiln_core::implementation::authority::AuthorityDenialReason;
use kiln_core::projection::materialization::{
    ProjectedWorkspaceRoute, RUNTIME_MOUNTED_ROOTS, store_outside_runtime_mounts,
};

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

// r[impl kiln.source.projection.route.scenario.dispatch]
/// Refuse a projected dispatch under a profile that does not enable
/// projection, a worker request that differs from the route of the plan's
/// pinned workspace, a route not derived for exactly the plan's permitted
/// paths, a pinned plan without its route, or a route for an unpinned plan.
pub fn check_route(
    route_file: Option<&Path>,
    plan: &ImplementationDispatchPlan,
    request: &serde_json::Value,
    authority: ProjectionAuthority,
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
    check_store_placement(Path::new(store), &RUNTIME_MOUNTED_ROOTS)
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
