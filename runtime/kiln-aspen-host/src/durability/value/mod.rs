use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use durable_file_publication::core::{
    DurabilityMode, PublicationDisposition, PublicationRequest, ReplacementMode,
};
use durable_file_publication::shell::{LinuxDirectory, StageNameSource, publish_one_file};
use kiln_adapter_aspen::protocol::decode_callback_outcome;
use kiln_adapter_aspen::service::build_service_ack;
use kiln_adapter_aspen::transport::AspenServiceDecision;
use molten::system_extension::{
    NativeCallbackValue, NativeValuePortFailure, NativeValuePortFailureKind,
    NativeValuePublicationReceipt, admit_native_callback_value,
};

use super::OperationLedger;
use io::{failure, leaf_from_value_ref, maximum_as_usize, publication_receipt, require_directory};

mod io;
mod port;

const VALUE_FILE_MODE: u32 = 0o600;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DurableValueStoreLimits {
    pub maximum_values: usize,
    pub maximum_value_bytes: u64,
    pub maximum_total_bytes: u64,
    pub stage_collision_attempts: usize,
    pub maximum_effects: usize,
}

pub(super) struct DurableCallbackValuePort {
    root: PathBuf,
    limits: DurableValueStoreLimits,
    ledger: Arc<Mutex<OperationLedger>>,
}

impl std::fmt::Debug for DurableCallbackValuePort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DurableCallbackValuePort")
            .field("root", &self.root)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct ValueStageNames {
    prefix: String,
    next: usize,
}

impl ValueStageNames {
    fn new(value: &NativeCallbackValue) -> Self {
        Self {
            prefix: value
                .value_ref
                .strip_prefix("blake3:")
                .unwrap_or("invalid")
                .to_string(),
            next: 0,
        }
    }
}

impl StageNameSource for ValueStageNames {
    fn next_stage_leaf(&mut self) -> String {
        let selected = self.next;
        self.next = self.next.saturating_add(1);
        format!("stage-{}-{selected}", self.prefix)
    }
}

impl DurableCallbackValuePort {
    pub(super) fn open(
        root: PathBuf,
        limits: DurableValueStoreLimits,
        ledger: Arc<Mutex<OperationLedger>>,
    ) -> Result<Self, NativeValuePortFailure> {
        if limits.maximum_values == 0
            || limits.maximum_value_bytes == 0
            || limits.maximum_total_bytes == 0
            || limits.stage_collision_attempts == 0
            || limits.maximum_effects == 0
        {
            return Err(failure(
                NativeValuePortFailureKind::RejectedBeforeAcceptance,
                "durable callback value store requires positive bounds",
            ));
        }
        require_directory(&root)?;
        let mut store = Self {
            root,
            limits,
            ledger,
        };
        let values = store.scan_values()?;
        for value in values {
            store.observe_callback_ack(&value)?;
        }
        Ok(store)
    }

    fn publish_value(
        &mut self,
        value: &NativeCallbackValue,
    ) -> Result<NativeValuePublicationReceipt, NativeValuePortFailure> {
        admit_native_callback_value(value, self.limits.maximum_value_bytes)?;
        let leaf = leaf_from_value_ref(&value.value_ref)?;
        let existing_path = self.root.join(&leaf);
        if std::fs::symlink_metadata(&existing_path).is_ok() {
            let existing =
                self.read_value(&leaf, &value.value_ref, self.limits.maximum_value_bytes)?;
            if existing != *value {
                return Err(failure(
                    NativeValuePortFailureKind::IdentityMismatch,
                    "callback value reference already names different bytes",
                ));
            }
            self.observe_callback_ack(value)?;
            return publication_receipt(value);
        }
        self.admit_capacity(value)?;
        let directory = File::open(&self.root).map_err(|error| {
            failure(
                NativeValuePortFailureKind::RejectedBeforeAcceptance,
                format!("open callback value directory: {error}"),
            )
        })?;
        let mut publication = LinuxDirectory::from_open_directory(directory).map_err(|error| {
            failure(
                NativeValuePortFailureKind::RejectedBeforeAcceptance,
                format!("admit callback value directory: {error}"),
            )
        })?;
        let maximum_payload_bytes = maximum_as_usize(self.limits.maximum_value_bytes)?;
        let request = PublicationRequest {
            destination_leaf: leaf,
            payload_bytes: value.bytes.len(),
            maximum_payload_bytes,
            collision_attempt_limit: self.limits.stage_collision_attempts,
            final_mode: VALUE_FILE_MODE,
            replacement: ReplacementMode::NoReplace,
            durability: DurabilityMode::DurabilityRequired,
        };
        let mut names = ValueStageNames::new(value);
        let classification = publish_one_file(&mut publication, &mut names, &request, &value.bytes)
            .map_err(|error| {
                failure(
                    NativeValuePortFailureKind::UnknownAfterAcceptance,
                    format!("publish callback value: {error:?}"),
                )
            })?;
        self.admit_publication(classification.disposition, &request, value)?;
        self.observe_callback_ack(value)?;
        publication_receipt(value)
    }

    fn admit_capacity(
        &mut self,
        value: &NativeCallbackValue,
    ) -> Result<(), NativeValuePortFailure> {
        let existing = self.scan_values()?;
        let current_bytes = existing.iter().try_fold(0_u64, |total, item| {
            let byte_count = u64::try_from(item.bytes.len()).map_err(|_| {
                failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value byte count does not fit u64",
                )
            })?;
            total.checked_add(byte_count).ok_or_else(|| {
                failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value store byte count overflow",
                )
            })
        })?;
        let next_count = existing.len().checked_add(1).ok_or_else(|| {
            failure(
                NativeValuePortFailureKind::BoundExceeded,
                "callback value count overflow",
            )
        })?;
        let value_bytes = u64::try_from(value.bytes.len()).map_err(|_| {
            failure(
                NativeValuePortFailureKind::BoundExceeded,
                "callback value byte count does not fit u64",
            )
        })?;
        let next_bytes = current_bytes.checked_add(value_bytes).ok_or_else(|| {
            failure(
                NativeValuePortFailureKind::BoundExceeded,
                "callback value total byte count overflow",
            )
        })?;
        if next_count > self.limits.maximum_values || next_bytes > self.limits.maximum_total_bytes {
            return Err(failure(
                NativeValuePortFailureKind::BoundExceeded,
                "callback value store reached its admitted capacity",
            ));
        }
        Ok(())
    }

    fn admit_publication(
        &self,
        disposition: PublicationDisposition,
        request: &PublicationRequest,
        value: &NativeCallbackValue,
    ) -> Result<(), NativeValuePortFailure> {
        match disposition {
            PublicationDisposition::CommittedAndParentSynchronized => Ok(()),
            PublicationDisposition::DestinationExists { .. } => {
                let existing = self.read_value(
                    &request.destination_leaf,
                    &value.value_ref,
                    self.limits.maximum_value_bytes,
                )?;
                if existing == *value {
                    Ok(())
                } else {
                    Err(failure(
                        NativeValuePortFailureKind::IdentityMismatch,
                        "callback value destination contains conflicting bytes",
                    ))
                }
            }
            PublicationDisposition::NotCommitted { .. } => Err(failure(
                NativeValuePortFailureKind::RejectedBeforeAcceptance,
                "callback value publication did not commit",
            )),
            PublicationDisposition::CommittedVisibilityOnly
            | PublicationDisposition::CommittedDurabilityUnknown => Err(failure(
                NativeValuePortFailureKind::UnknownAfterAcceptance,
                "callback value durability is unknown after visibility",
            )),
        }
    }

    fn observe_callback_ack(
        &mut self,
        value: &NativeCallbackValue,
    ) -> Result<(), NativeValuePortFailure> {
        let Ok(outcome) = decode_callback_outcome(
            &value.bytes,
            maximum_as_usize(self.limits.maximum_value_bytes)?,
            self.limits.maximum_effects,
        ) else {
            return Ok(());
        };
        if !outcome.effects.is_empty() {
            return Ok(());
        }
        let mut ledger = self.ledger.lock().map_err(|_| {
            failure(
                NativeValuePortFailureKind::UnknownAfterAcceptance,
                "operation ledger lock is unavailable",
            )
        })?;
        let Some(record) = ledger.operation(&outcome.operation_id).map_err(|error| {
            failure(
                NativeValuePortFailureKind::UnknownAfterAcceptance,
                error.to_string(),
            )
        })?
        else {
            return Ok(());
        };
        let acknowledgement =
            build_service_ack(&record.ingress, &outcome, &value.bytes).map_err(|error| {
                failure(
                    NativeValuePortFailureKind::IdentityMismatch,
                    format!("build durable service acknowledgement: {error}"),
                )
            })?;
        if acknowledgement.decision == AspenServiceDecision::Accepted {
            return Ok(());
        }
        ledger
            .record_acknowledgement(&outcome.operation_id, acknowledgement)
            .map(|_| ())
            .map_err(|error| {
                failure(
                    NativeValuePortFailureKind::UnknownAfterAcceptance,
                    error.to_string(),
                )
            })
    }
}
