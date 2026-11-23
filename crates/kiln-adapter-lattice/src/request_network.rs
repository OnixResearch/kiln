//! The network a Lattice implementation request declares.
//!
//! Lattice removed the `network` field from implementation request v3 in
//! Lattice b46deb36: a v3 worker never receives a network device, and Lattice
//! rejects the field as unknown. Earlier request versions carried it.

/// Lattice's current implementation request schema.
pub const REQUEST_V3_SCHEMA: &str = "lattice.implementation-attempt-run.v3";

/// The worker network a request declares, or `None` when the declaration is
/// malformed. A v3 request without `network` declares none; a v3 request
/// claiming network is malformed. An earlier version must carry the field.
/// Absence never means network.
#[must_use]
pub fn request_network(request: &serde_json::Value) -> Option<bool> {
    let declared = request.get("network");
    if request.get("schema").and_then(serde_json::Value::as_str) == Some(REQUEST_V3_SCHEMA) {
        match declared {
            None => Some(false),
            Some(value) => value.as_bool().filter(|network| !network),
        }
    } else {
        declared?.as_bool()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn v3_absence_is_no_network_and_never_network() {
        let v3 = json!({"schema": REQUEST_V3_SCHEMA});
        assert_eq!(request_network(&v3), Some(false));
        // A plan asking for network is still refused by the comparison.
        assert_ne!(request_network(&v3), Some(true));
        assert_eq!(
            request_network(&json!({"schema": REQUEST_V3_SCHEMA, "network": true})),
            None
        );
        assert_eq!(
            request_network(&json!({"schema": REQUEST_V3_SCHEMA, "network": "no"})),
            None
        );
    }

    #[test]
    fn earlier_versions_must_declare_network() {
        let v2 = "lattice.implementation-attempt-run.v2";
        assert_eq!(request_network(&json!({"schema": v2})), None);
        assert_eq!(request_network(&json!({})), None);
        assert_eq!(
            request_network(&json!({"schema": v2, "network": false})),
            Some(false)
        );
        assert_eq!(
            request_network(&json!({"schema": v2, "network": true})),
            Some(true)
        );
    }
}
