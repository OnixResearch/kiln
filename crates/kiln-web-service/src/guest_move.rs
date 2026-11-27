//! Bounded web-bridge allowlist and operator/projection gate for running guest moves.
//!
//! Enforces `Role::Operator` and refuses `restricted_projection` before
//! forwarding to `kiln-fleet` `/v1/placement/guest-move*`.

/// Whether `target` targets the running guest move bridge routes.
#[must_use]
pub fn is_guest_move_route(target: &str) -> bool {
    let path = target.split('?').next().unwrap_or(target);
    path == "/api/fleet/placement/guest-move"
        || path.starts_with("/api/fleet/placement/guest-move/")
}

/// Enforce operator role and non-restricted projection on guest move routes.
pub fn authorize_guest_move(
    restricted_projection: bool,
    is_operator: bool,
) -> Result<(), (u16, &'static str)> {
    if restricted_projection {
        return Err((403, "fleet_projection_forbidden"));
    }
    if !is_operator {
        return Err((403, "fleet_operator"));
    }
    Ok(())
}

/// Strict allowlist mapping `/api/fleet/placement/guest-move*` to `/v1/placement/guest-move*`.
// r[impl kiln.sdlc.guest_move]
#[must_use]
pub fn upstream_path(method: &str, target: &str) -> Option<String> {
    let rest = target.strip_prefix("/api/fleet/")?;
    if rest.contains('#')
        || rest.contains('?')
        || rest
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return None;
    }
    let segments: Vec<&str> = rest.split('/').collect();
    match (method, segments.as_slice()) {
        ("GET", ["placement", "guest-move"]) => Some("/v1/placement/guest-move".into()),
        ("POST", ["placement", "guest-move", "plan"]) => {
            Some("/v1/placement/guest-move/plan".into())
        }
        ("POST", ["placement", "guest-move"]) => Some("/v1/placement/guest-move".into()),
        ("GET", ["placement", "guest-move", id]) if *id != "plan" && is_identity(id) => {
            Some(format!("/v1/placement/guest-move/{id}"))
        }
        _ => None,
    }
}

fn is_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    // r[verify kiln.sdlc.guest_move]
    #[test]
    fn guest_move_bridge_allowlist_and_auth_gate_fail_closed() {
        assert_eq!(
            upstream_path("GET", "/api/fleet/placement/guest-move").as_deref(),
            Some("/v1/placement/guest-move")
        );
        assert_eq!(
            upstream_path("POST", "/api/fleet/placement/guest-move/plan").as_deref(),
            Some("/v1/placement/guest-move/plan")
        );
        assert_eq!(
            upstream_path("POST", "/api/fleet/placement/guest-move").as_deref(),
            Some("/v1/placement/guest-move")
        );
        assert_eq!(
            upstream_path("GET", &format!("/api/fleet/placement/guest-move/{ID}")).as_deref(),
            Some(format!("/v1/placement/guest-move/{ID}").as_str())
        );

        for refused in [
            ("DELETE", "/api/fleet/placement/guest-move"),
            ("GET", "/api/fleet/placement/guest-move/plan"),
            ("GET", "/api/fleet/placement/guest-move/not-hex"),
            ("POST", "/api/fleet/placement/guest-move?x=1"),
            ("GET", "/api/fleet/placement/guest-move#frag"),
            ("POST", &format!("/api/fleet/placement/guest-move/{ID}")),
            ("GET", "/api/fleet/placement/guest-move/../claims"),
        ] {
            assert_eq!(upstream_path(refused.0, refused.1), None, "{refused:?}");
        }

        assert_eq!(
            authorize_guest_move(true, true),
            Err((403, "fleet_projection_forbidden"))
        );
        assert_eq!(
            authorize_guest_move(false, false),
            Err((403, "fleet_operator"))
        );
        assert_eq!(authorize_guest_move(false, true), Ok(()));
    }
}
