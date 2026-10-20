//! Verified KV entry version computation.
//!
//! Models the version bookkeeping for an etcd-compatible KV store: every
//! write assigns a version number, `create_revision`, and `mod_revision`.
//! New keys get version 1 and `create_revision` = `log_index`. Updates
//! increment the version (saturating at `u64::MAX`) and preserve
//! `create_revision`. `mod_revision` always equals the `log_index` of the write.
//!
//! Proves: version >= 1, `create_revision` stability across updates,
//! `mod_revision` = `log_index`, version monotonicity, saturation safety.

use vstd::prelude::*;

verus! {

/// Output of KV version computation.
pub struct KvVersions {
    /// Revision at which the key was first created.
    pub create_revision: u64,
    /// Revision at which the key was last modified.
    pub mod_revision: u64,
    /// Version counter (starts at 1, increments on each update).
    pub version: u64,
}

// --- compute_kv_versions ---

// r[depends kv-version.compute-versions]
/// Spec: compute the version fields for a KV write.
///
/// `prev` is None for new keys, Some((create_revision, version)) for updates.
/// `log_index` is the Raft log index of this write.
pub open spec fn compute_kv_versions_spec(
    prev: Option<(u64, u64)>,
    log_index: u64,
) -> KvVersions {
    match prev {
        None => KvVersions {
            create_revision: log_index,
            mod_revision: log_index,
            version: 1u64,
        },
        Some((create_rev, old_version)) => KvVersions {
            create_revision: create_rev,
            mod_revision: log_index,
            version: if old_version == u64::MAX { u64::MAX } else { (old_version + 1) as u64 },
        },
    }
}

// r[impl kv-version.compute-versions]
/// Compute KV version fields for a write operation.
pub fn compute_kv_versions(
    prev: Option<(u64, u64)>,
    log_index: u64,
) -> (result: KvVersions)
    ensures
        result.create_revision == compute_kv_versions_spec(prev, log_index).create_revision,
        result.mod_revision == compute_kv_versions_spec(prev, log_index).mod_revision,
        result.version == compute_kv_versions_spec(prev, log_index).version,
{
    match prev {
        None => KvVersions {
            create_revision: log_index,
            mod_revision: log_index,
            version: 1,
        },
        Some((create_rev, old_version)) => KvVersions {
            create_revision: create_rev,
            mod_revision: log_index,
            version: if old_version == u64::MAX { u64::MAX } else { old_version + 1 },
        },
    }
}

// --- Safety proofs ---

// r[impl kv-version.version-always-positive]
// r[verify kv-version.version-always-positive]
/// Version is always >= 1.
proof fn version_always_positive(prev: Option<(u64, u64)>, log_index: u64)
    ensures compute_kv_versions_spec(prev, log_index).version >= 1u64
{}

// r[impl kv-version.create-revision-preserved]
// r[verify kv-version.create-revision-preserved]
/// In the update case, create_revision is preserved from input.
proof fn create_revision_preserved(create_rev: u64, old_version: u64, log_index: u64)
    ensures
        compute_kv_versions_spec(Some((create_rev, old_version)), log_index).create_revision
            == create_rev,
{}

// r[impl kv-version.mod-revision-equals-log-index]
// r[verify kv-version.mod-revision-equals-log-index]
/// mod_revision always equals log_index.
proof fn mod_revision_equals_log_index(prev: Option<(u64, u64)>, log_index: u64)
    ensures compute_kv_versions_spec(prev, log_index).mod_revision == log_index
{}

// r[impl kv-version.version-increments]
// r[verify kv-version.version-increments]
/// In the update case with version below MAX, version increments by 1.
proof fn version_increments(create_rev: u64, old_version: u64, log_index: u64)
    requires old_version < u64::MAX
    ensures
        compute_kv_versions_spec(Some((create_rev, old_version)), log_index).version
            == old_version + 1,
{}

// r[impl kv-version.version-saturates]
// r[verify kv-version.version-saturates]
/// At u64::MAX, version stays at u64::MAX.
proof fn version_saturates(create_rev: u64, log_index: u64)
    ensures
        compute_kv_versions_spec(Some((create_rev, u64::MAX)), log_index).version
            == u64::MAX,
{}

// r[impl kv-version.deterministic]
// r[verify kv-version.deterministic]
/// Same inputs produce same output.
proof fn deterministic(
    prev1: Option<(u64, u64)>,
    prev2: Option<(u64, u64)>,
    li1: u64,
    li2: u64,
)
    requires prev1 == prev2, li1 == li2
    ensures
        compute_kv_versions_spec(prev1, li1).version
            == compute_kv_versions_spec(prev2, li2).version,
        compute_kv_versions_spec(prev1, li1).create_revision
            == compute_kv_versions_spec(prev2, li2).create_revision,
        compute_kv_versions_spec(prev1, li1).mod_revision
            == compute_kv_versions_spec(prev2, li2).mod_revision,
{}

// --- Additional proofs ---

// r[verify kv-version.compute-versions]
/// New key always gets version 1.
proof fn new_key_version_is_one(log_index: u64)
    ensures compute_kv_versions_spec(None, log_index).version == 1u64
{}

// r[verify kv-version.compute-versions]
/// New key create_revision matches log_index.
proof fn new_key_create_revision(log_index: u64)
    ensures compute_kv_versions_spec(None, log_index).create_revision == log_index
{}

// r[verify kv-version.version-always-positive]
/// After any number of updates, version stays >= 1.
proof fn version_positive_after_update(create_rev: u64, old_version: u64, log_index: u64)
    requires old_version >= 1u64
    ensures compute_kv_versions_spec(Some((create_rev, old_version)), log_index).version >= 1u64
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify kv-version.compute-versions]
    #[test]
    fn test_new_key() {
        let v = compute_kv_versions(None, 42);
        assert_eq!(v.version, 1);
        assert_eq!(v.create_revision, 42);
        assert_eq!(v.mod_revision, 42);
    }

    // r[verify kv-version.compute-versions]
    #[test]
    fn test_update_key() {
        let v = compute_kv_versions(Some((10, 3)), 50);
        assert_eq!(v.version, 4);
        assert_eq!(v.create_revision, 10);
        assert_eq!(v.mod_revision, 50);
    }

    // r[verify kv-version.version-saturates]
    #[test]
    fn test_saturation() {
        let v = compute_kv_versions(Some((1, u64::MAX)), 100);
        assert_eq!(v.version, u64::MAX);
    }

    proptest! {
        // r[verify kv-version.version-always-positive]
        #[test]
        fn prop_version_positive(
            prev_cr in proptest::option::of((any::<u64>(), 1..u64::MAX)),
            log_index: u64,
        ) {
            let result = compute_kv_versions(prev_cr, log_index);
            prop_assert!(result.version >= 1);
        }

        // r[verify kv-version.mod-revision-equals-log-index]
        #[test]
        fn prop_mod_revision_is_log_index(
            prev_cr in proptest::option::of((any::<u64>(), any::<u64>())),
            log_index: u64,
        ) {
            let result = compute_kv_versions(prev_cr, log_index);
            prop_assert_eq!(result.mod_revision, log_index);
        }

        // r[verify kv-version.create-revision-preserved]
        #[test]
        fn prop_create_revision_preserved(create_rev: u64, old_version: u64, log_index: u64) {
            let result = compute_kv_versions(Some((create_rev, old_version)), log_index);
            prop_assert_eq!(result.create_revision, create_rev);
        }

        // r[verify kv-version.version-increments]
        #[test]
        fn prop_version_increments(create_rev: u64, old_version in 1..u64::MAX, log_index: u64) {
            let result = compute_kv_versions(Some((create_rev, old_version)), log_index);
            prop_assert_eq!(result.version, old_version + 1);
        }

        // r[verify kv-version.deterministic]
        #[test]
        fn prop_deterministic(
            prev_cr in proptest::option::of((any::<u64>(), any::<u64>())),
            log_index: u64,
        ) {
            let r1 = compute_kv_versions(prev_cr, log_index);
            let r2 = compute_kv_versions(prev_cr, log_index);
            prop_assert_eq!(r1.version, r2.version);
            prop_assert_eq!(r1.create_revision, r2.create_revision);
            prop_assert_eq!(r1.mod_revision, r2.mod_revision);
        }

        // r[verify kv-version.version-always-positive]
        #[test]
        fn prop_new_key_version_one(log_index: u64) {
            let result = compute_kv_versions(None, log_index);
            prop_assert_eq!(result.version, 1);
        }
    }
}
