mod ledger;
mod model;
mod native;
mod state;
mod value;

use std::path::Path;
use std::sync::{Arc, Mutex};

use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use molten::system_extension::{
    DurableNativeHostJournal, SharedNativeCallbackValuePort, shared_native_callback_value_port,
};

pub use ledger::OperationLedger;
pub use model::{
    DURABLE_HOST_STATE_SCHEMA, DurableEffectIntent, DurableHostStateMarker, DurableOperationRecord,
    DurableOperationState,
};
use value::{DurableCallbackValuePort, DurableValueStoreLimits};

use crate::error::{HostBridgeError, Result};

const LEDGER_MATERIALIZED_VALUES_PER_RECORD: usize = 4;
const LEDGER_RECORD_OVERHEAD_BYTES: usize = 65_536;
const CALLBACK_LIFECYCLE_VALUE_ALLOWANCE: usize = 64;
const CALLBACK_STAGE_ATTEMPTS: usize = 8;

pub type HostJournal = DurableNativeHostJournal;
pub type SharedHostJournal = Arc<Mutex<HostJournal>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostStateMode {
    Fresh,
    Recovered,
}

pub struct HostDurability {
    pub mode: HostStateMode,
    pub ledger: Arc<Mutex<OperationLedger>>,
    pub journal: SharedHostJournal,
    pub values: SharedNativeCallbackValuePort,
}

impl std::fmt::Debug for HostDurability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HostDurability")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl HostDurability {
    // r[impl kiln.aspen_host.production_durability]
    pub fn open(
        state_root: &Path,
        profile: &AdmittedAspenRuntimeProfile,
        cohort_identity: &str,
    ) -> Result<Self> {
        state::require_directory(state_root, "host state root")?;
        let marker = state::expected_marker(cohort_identity);
        let marker_path = state_root.join(state::STATE_MARKER_FILE);
        let mode = if std::fs::symlink_metadata(&marker_path).is_ok() {
            let observed = state::load_marker(&marker_path)?;
            if observed != marker {
                return Err(HostBridgeError::State(
                    "host state marker differs from the admitted cohort".to_string(),
                ));
            }
            HostStateMode::Recovered
        } else {
            HostStateMode::Fresh
        };
        let ledger_root = state::prepare_directory(state_root, state::LEDGER_DIRECTORY, mode)?;
        let native_root =
            state::prepare_directory(state_root, state::NATIVE_JOURNAL_DIRECTORY, mode)?;
        let value_root =
            state::prepare_directory(state_root, state::CALLBACK_VALUE_DIRECTORY, mode)?;
        let ledger_path = ledger_root.join(state::LEDGER_FILE);
        let native_journal_path = native_root.join(state::NATIVE_JOURNAL_FILE);
        if mode == HostStateMode::Recovered {
            state::require_regular_file(&ledger_path, "operation ledger")?;
            state::require_regular_file(&native_journal_path, "native host journal")?;
        }

        let maximum_record_bytes = ledger_record_bound(profile)?;
        let ledger_mode = match mode {
            HostStateMode::Fresh => ledger::LedgerOpenMode::Fresh,
            HostStateMode::Recovered => ledger::LedgerOpenMode::Recover,
        };
        let ledger = Arc::new(Mutex::new(OperationLedger::open(
            &ledger_path,
            ledger_mode,
            cohort_identity,
            u64::try_from(profile.profile.bounds.max_records).map_err(|_| {
                HostBridgeError::Admission("ledger record count does not fit u64".to_string())
            })?,
            u64::try_from(maximum_record_bytes).map_err(|_| {
                HostBridgeError::Admission("ledger record byte bound does not fit u64".to_string())
            })?,
        )?));

        let durability_profile = native::profile(profile, cohort_identity)?;
        let descriptor = native::descriptor(
            profile,
            cohort_identity,
            &durability_profile.profile.profile_ref,
        )?;
        let adapter = molten::fabric_durability::RedbDurableStateAdapter::open(
            &native_root,
            durability_profile,
            descriptor,
        )
        .map_err(|error| HostBridgeError::State(format!("open native host journal: {error}")))?;
        let journal = Arc::new(Mutex::new(DurableNativeHostJournal::new(adapter)));
        let values = open_values(profile, value_root, ledger.clone())?;

        if mode == HostStateMode::Fresh {
            state::publish_marker(state_root, &marker)?;
        }
        Ok(Self {
            mode,
            ledger,
            journal,
            values,
        })
    }
}

fn ledger_record_bound(profile: &AdmittedAspenRuntimeProfile) -> Result<usize> {
    profile
        .profile
        .bounds
        .max_materialized_value_bytes
        .checked_mul(LEDGER_MATERIALIZED_VALUES_PER_RECORD)
        .and_then(|value| value.checked_add(LEDGER_RECORD_OVERHEAD_BYTES))
        .ok_or_else(|| HostBridgeError::Admission("ledger record bound overflow".to_string()))
}

fn open_values(
    profile: &AdmittedAspenRuntimeProfile,
    root: std::path::PathBuf,
    ledger: Arc<Mutex<OperationLedger>>,
) -> Result<SharedNativeCallbackValuePort> {
    let maximum_values = profile
        .profile
        .bounds
        .max_records
        .checked_mul(profile.profile.bounds.max_materialized_values)
        .and_then(|value| value.checked_add(CALLBACK_LIFECYCLE_VALUE_ALLOWANCE))
        .ok_or_else(|| {
            HostBridgeError::Admission("callback value item bound overflow".to_string())
        })?;
    let maximum_value_bytes = u64::try_from(profile.profile.bounds.max_materialized_value_bytes)
        .map_err(|_| {
            HostBridgeError::Admission("callback value bound does not fit u64".to_string())
        })?;
    let maximum_total_bytes = u64::try_from(maximum_values)
        .ok()
        .and_then(|count| count.checked_mul(maximum_value_bytes))
        .ok_or_else(|| {
            HostBridgeError::Admission("callback value total bound overflow".to_string())
        })?;
    let values = DurableCallbackValuePort::open(
        root,
        DurableValueStoreLimits {
            maximum_values,
            maximum_value_bytes,
            maximum_total_bytes,
            stage_collision_attempts: CALLBACK_STAGE_ATTEMPTS,
            maximum_effects: profile.profile.bounds.max_effects,
        },
        ledger,
    )
    .map_err(|error| {
        HostBridgeError::State(format!("open callback value store: {}", error.message))
    })?;
    Ok(shared_native_callback_value_port(values))
}
