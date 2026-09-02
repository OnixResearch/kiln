use std::env;

use kiln_adapter_celld::{CelldCohortIdentity, CelldCohortObservation};

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| unreachable!("the local cohort runner must provide {name}"))
}

fn identity(kind: &str) -> CelldCohortIdentity {
    let read = |field: &str| required(&format!("CELLD_LIVE_{kind}_{field}"));
    CelldCohortIdentity {
        artifact_blake3: read("ARTIFACT_BLAKE3"),
        platform: read("PLATFORM"),
        worker_bundle_blake3: read("WORKER_BUNDLE_BLAKE3"),
        deployment_id: read("DEPLOYMENT_ID"),
        storage_cohort: read("STORAGE_COHORT"),
        ingress_ca_blake3: read("INGRESS_CA_BLAKE3"),
    }
}

pub fn selected_and_observed() -> (CelldCohortIdentity, CelldCohortObservation) {
    let selected = identity("SELECTED");
    let observed = CelldCohortObservation {
        identity: identity("OBSERVED"),
        release: required("CELLD_LIVE_OBSERVED_RELEASE"),
        revision: required("CELLD_LIVE_OBSERVED_REVISION"),
        worker_protocol_version: required("CELLD_LIVE_OBSERVED_PROTOCOL")
            .parse()
            .unwrap_or_else(|_| unreachable!("observed Worker protocol integer")),
        schema_version: required("CELLD_LIVE_OBSERVED_SCHEMA")
            .parse()
            .unwrap_or_else(|_| unreachable!("observed Worker schema integer")),
        origin: required("CELLD_LIVE_OBSERVED_ORIGIN"),
        output_gate_enabled: required("CELLD_LIVE_OBSERVED_OUTPUT_GATE") == "1",
        storage_probe_succeeded: required("CELLD_LIVE_OBSERVED_STORAGE_PROBE") == "1",
        observed_at_seconds: required("CELLD_LIVE_OBSERVED_AT")
            .parse()
            .unwrap_or_else(|_| unreachable!("observed startup timestamp integer")),
    };
    (selected, observed)
}
