use std::fs::File;
use std::io::Read;
use std::path::Path;

use molten::system_extension::{
    NativeCallbackValue, NativeValuePortFailure, NativeValuePortFailureKind,
    NativeValuePublicationReceipt, admit_native_callback_value, native_identity_ref,
};
use rustix::fs::{Mode, OFlags, openat};

use super::DurableCallbackValuePort;

const BLAKE3_HEX_CHARACTERS: usize = 64;
const VALUE_OVERFLOW_SENTINEL_BYTES: u64 = 1;

impl DurableCallbackValuePort {
    pub(super) fn scan_values(
        &mut self,
    ) -> Result<Vec<NativeCallbackValue>, NativeValuePortFailure> {
        let mut values = Vec::new();
        let mut total_bytes = 0_u64;
        let entries = std::fs::read_dir(&self.root).map_err(|error| {
            failure(
                NativeValuePortFailureKind::UnknownAfterAcceptance,
                format!("read callback value directory: {error}"),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                failure(
                    NativeValuePortFailureKind::UnknownAfterAcceptance,
                    format!("read callback value entry: {error}"),
                )
            })?;
            if values.len() >= self.limits.maximum_values {
                return Err(failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value store exceeds its item bound",
                ));
            }
            let leaf = entry.file_name().into_string().map_err(|_| {
                failure(
                    NativeValuePortFailureKind::IdentityMismatch,
                    "callback value leaf is not UTF-8",
                )
            })?;
            let value_ref = value_ref_from_leaf(&leaf)?;
            let value = self.read_value(&leaf, &value_ref, self.limits.maximum_value_bytes)?;
            let byte_count = u64::try_from(value.bytes.len()).map_err(|_| {
                failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value byte count does not fit u64",
                )
            })?;
            total_bytes = total_bytes.checked_add(byte_count).ok_or_else(|| {
                failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value store byte count overflow",
                )
            })?;
            if total_bytes > self.limits.maximum_total_bytes {
                return Err(failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value store exceeds its total byte bound",
                ));
            }
            values.push(value);
        }
        values.sort_by(|left, right| left.value_ref.cmp(&right.value_ref));
        Ok(values)
    }

    pub(super) fn read_value(
        &self,
        leaf: &str,
        value_ref: &str,
        maximum_bytes: u64,
    ) -> Result<NativeCallbackValue, NativeValuePortFailure> {
        let directory = File::open(&self.root).map_err(|error| {
            failure(
                NativeValuePortFailureKind::UnknownAfterAcceptance,
                format!("open callback value directory: {error}"),
            )
        })?;
        let descriptor = openat(
            &directory,
            leaf,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            failure(
                NativeValuePortFailureKind::Missing,
                format!("open callback value: {error}"),
            )
        })?;
        let file = File::from(descriptor);
        let metadata = file.metadata().map_err(|error| {
            failure(
                NativeValuePortFailureKind::UnknownAfterAcceptance,
                format!("inspect callback value: {error}"),
            )
        })?;
        if !metadata.is_file() || metadata.len() > maximum_bytes {
            return Err(failure(
                NativeValuePortFailureKind::BoundExceeded,
                "callback value is not a bounded regular file",
            ));
        }
        let read_limit = maximum_bytes
            .checked_add(VALUE_OVERFLOW_SENTINEL_BYTES)
            .ok_or_else(|| {
                failure(
                    NativeValuePortFailureKind::BoundExceeded,
                    "callback value read bound overflow",
                )
            })?;
        let mut bytes = Vec::new();
        file.take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                failure(
                    NativeValuePortFailureKind::UnknownAfterAcceptance,
                    format!("read callback value: {error}"),
                )
            })?;
        let value = NativeCallbackValue {
            value_ref: value_ref.to_string(),
            bytes,
        };
        admit_native_callback_value(&value, maximum_bytes)?;
        Ok(value)
    }
}

pub(super) fn leaf_from_value_ref(value_ref: &str) -> Result<String, NativeValuePortFailure> {
    let leaf = value_ref.strip_prefix("blake3:").ok_or_else(|| {
        failure(
            NativeValuePortFailureKind::IdentityMismatch,
            "callback value reference has no BLAKE3 prefix",
        )
    })?;
    if leaf.len() != BLAKE3_HEX_CHARACTERS
        || !leaf
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(failure(
            NativeValuePortFailureKind::IdentityMismatch,
            "callback value reference is not lowercase BLAKE3 hex",
        ));
    }
    Ok(leaf.to_string())
}

pub(super) fn value_ref_from_leaf(leaf: &str) -> Result<String, NativeValuePortFailure> {
    let value_ref = format!("blake3:{leaf}");
    leaf_from_value_ref(&value_ref).map(|_| value_ref)
}

pub(super) fn publication_receipt(
    value: &NativeCallbackValue,
) -> Result<NativeValuePublicationReceipt, NativeValuePortFailure> {
    let byte_count = u64::try_from(value.bytes.len()).map_err(|_| {
        failure(
            NativeValuePortFailureKind::BoundExceeded,
            "callback value byte count does not fit u64",
        )
    })?;
    Ok(NativeValuePublicationReceipt {
        value_ref: value.value_ref.clone(),
        publication_ref: native_identity_ref(&[
            "native-value-publication-v2",
            &value.value_ref,
            &byte_count.to_string(),
        ]),
        byte_count,
    })
}

pub(super) fn require_directory(path: &Path) -> Result<(), NativeValuePortFailure> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        failure(
            NativeValuePortFailureKind::RejectedBeforeAcceptance,
            format!("inspect callback value directory: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(failure(
            NativeValuePortFailureKind::RejectedBeforeAcceptance,
            "callback value root is not an admitted directory",
        ));
    }
    Ok(())
}

pub(super) fn maximum_as_usize(value: u64) -> Result<usize, NativeValuePortFailure> {
    usize::try_from(value).map_err(|_| {
        failure(
            NativeValuePortFailureKind::BoundExceeded,
            "callback value bound does not fit this host",
        )
    })
}

pub(super) fn failure(
    kind: NativeValuePortFailureKind,
    message: impl Into<String>,
) -> NativeValuePortFailure {
    NativeValuePortFailure::new(kind, message)
}
