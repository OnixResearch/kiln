use std::collections::BTreeSet;

use kiln_core::types::{CoreResult, Diagnostic};

use crate::profile::{
    AdmittedAspenRuntimeProfile, AspenPortPurpose, AspenRuntimeProfile, admit_profile,
};

pub const STANDALONE_RUNTIME_PROFILE_ID: &str = "standalone-runtime-v1";
pub const LATTICE_WORKFLOW_OWNER: &str = "lattice-workflow-v1";
pub const MANTLE_BUILD_OWNER: &str = "mantle-build-v1";
pub const CAIRN_GATE_OWNER: &str = "cairn-gate-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelection {
    Standalone,
    AspenV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAvailability {
    pub endpoint_available: bool,
    pub ports: BTreeSet<AspenPortPurpose>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeComposition {
    Standalone {
        profile_id: String,
    },
    Aspen {
        profile: Box<AdmittedAspenRuntimeProfile>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationComposition {
    pub runtime: RuntimeComposition,
    pub workflow_owner: String,
    pub build_owner: String,
    pub gate_owner: String,
}

// r[impl kiln.aspen_runtime.composition] r[impl kiln.aspen_runtime.boundary]
pub fn compose_application(
    selection: RuntimeSelection,
    aspen_profile: Option<AspenRuntimeProfile>,
    availability: Option<RuntimeAvailability>,
) -> CoreResult<ApplicationComposition> {
    let runtime = match selection {
        RuntimeSelection::Standalone => RuntimeComposition::Standalone {
            profile_id: STANDALONE_RUNTIME_PROFILE_ID.to_string(),
        },
        RuntimeSelection::AspenV1 => {
            let profile = aspen_profile.ok_or_else(|| {
                Diagnostic::new(
                    "aspen_composition_missing_profile",
                    "runtime.selection",
                    "Aspen was selected, but its exact runtime profile is missing",
                )
            })?;
            let availability = availability.ok_or_else(|| {
                Diagnostic::new(
                    "aspen_composition_unavailable",
                    "runtime.selection",
                    "Aspen was selected, but runtime availability is unknown",
                )
            })?;
            if !availability.endpoint_available {
                return Err(Diagnostic::new(
                    "aspen_composition_unavailable",
                    "runtime.endpoint",
                    "selected Aspen endpoint is unavailable; runtime fallback is forbidden",
                ));
            }
            let required = [
                AspenPortPurpose::Durability,
                AspenPortPurpose::Content,
                AspenPortPurpose::Execution,
                AspenPortPurpose::Transport,
                AspenPortPurpose::Time,
                AspenPortPurpose::Scheduling,
                AspenPortPurpose::Evidence,
            ]
            .into_iter()
            .collect::<BTreeSet<_>>();
            if availability.ports != required {
                return Err(Diagnostic::new(
                    "aspen_composition_port_unavailable",
                    "runtime.ports",
                    "selected Aspen runtime lacks an exact required fabric-port binding",
                ));
            }
            RuntimeComposition::Aspen {
                profile: Box::new(admit_profile(&profile)?),
            }
        }
    };
    Ok(ApplicationComposition {
        runtime,
        workflow_owner: LATTICE_WORKFLOW_OWNER.to_string(),
        build_owner: MANTLE_BUILD_OWNER.to_string(),
        gate_owner: CAIRN_GATE_OWNER.to_string(),
    })
}

#[must_use]
pub fn exact_availability() -> RuntimeAvailability {
    RuntimeAvailability {
        endpoint_available: true,
        ports: [
            AspenPortPurpose::Durability,
            AspenPortPurpose::Content,
            AspenPortPurpose::Execution,
            AspenPortPurpose::Transport,
            AspenPortPurpose::Time,
            AspenPortPurpose::Scheduling,
            AspenPortPurpose::Evidence,
        ]
        .into_iter()
        .collect(),
    }
}
