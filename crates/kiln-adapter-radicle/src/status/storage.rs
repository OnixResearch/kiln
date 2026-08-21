use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use cob_git2 as git2;
use kiln_core::types::{CoreResult, Diagnostic};
use radicle_cob::change::{self, EntryId};
use radicle_cob::object::{Commit, Objects, Reference, Storage as ObjectStorage};
use radicle_cob::{Manifest, ObjectId, TypeName};
use radicle_crypto::{PublicKey, Signer};
use radicle_git_ref_format::RefString;
use radicle_oid::Oid;

/// Storage-layer error carrying a Kiln diagnostic.
///
/// `radicle-cob` storage traits require `std::error::Error`; this wrapper
/// keeps the diagnostic intact so the shell can convert it back losslessly.
#[derive(Debug)]
pub struct StorageError {
    pub diagnostic: Diagnostic,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.diagnostic.message)
    }
}

impl std::error::Error for StorageError {}

impl From<Diagnostic> for StorageError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self { diagnostic }
    }
}

impl From<StorageError> for Diagnostic {
    fn from(error: StorageError) -> Self {
        error.diagnostic
    }
}

fn refs_error(code: &'static str, message: String) -> StorageError {
    StorageError {
        diagnostic: Diagnostic::new(code, "radicle_status.refs", &message),
    }
}

/// A `git2` repository handle bound to the pinned `radicle-cob` cohort.
///
/// Implements the two storage traits `collaboration::create` and
/// `collaboration::update` require. The reference layout matches Radicle:
/// `refs/namespaces/<node>/refs/cobs/<typename>/<object_id>`.
pub struct CobRepository<'a> {
    backend: &'a git2::Repository,
    expected_ref_target: Option<Oid>,
}

impl<'a> CobRepository<'a> {
    #[must_use]
    pub const fn new(backend: &'a git2::Repository) -> Self {
        Self {
            backend,
            expected_ref_target: None,
        }
    }

    #[must_use]
    pub const fn with_expected_ref(backend: &'a git2::Repository, expected: Oid) -> Self {
        Self {
            backend,
            expected_ref_target: Some(expected),
        }
    }

    #[must_use]
    pub const fn backend(&self) -> &'a git2::Repository {
        self.backend
    }

    fn cob_ref_name(
        &self,
        namespace: &PublicKey,
        typename: &TypeName,
        object_id: &ObjectId,
    ) -> CoreResult<RefString> {
        let name = format!("refs/namespaces/{namespace}/refs/cobs/{typename}/{object_id}");
        RefString::try_from(name).map_err(|_| name_diagnostic())
    }

    /// List distinct object identities while enforcing the caller's bound.
    pub fn object_ids_bounded(
        &self,
        typename: &TypeName,
        max_objects: usize,
    ) -> Result<Vec<ObjectId>, StorageError> {
        if max_objects == 0 {
            return Err(refs_error(
                "radicle_object_scan_bound",
                "ci-job object scan bound must be positive".to_string(),
            ));
        }
        let pattern = format!("refs/namespaces/*/refs/cobs/{typename}/*");
        let glob = self.backend.references_glob(&pattern).map_err(|error| {
            refs_error(
                "radicle_refs_unreadable",
                format!("local ci-job object listing could not be read: {error}"),
            )
        })?;
        let mut object_ids = BTreeSet::new();
        for reference in glob {
            let reference = reference.map_err(|error| {
                refs_error(
                    "radicle_refs_unreadable",
                    format!("local ci-job reference could not be read: {error}"),
                )
            })?;
            let raw_name = reference.name().map_err(|error| {
                refs_error(
                    "radicle_ref_name_invalid",
                    format!("ci-job reference name was unreadable: {error}"),
                )
            })?;
            let name = ref_string(raw_name)?;
            let (_, object_id) = radicle_cob::object::collaboration::parse_refstr(&name)
                .ok_or_else(|| {
                    refs_error(
                        "radicle_ref_unparsable",
                        "A ci-job reference did not parse into its declared identities".to_string(),
                    )
                })?;
            if !object_ids.contains(&object_id) && object_ids.len() == max_objects {
                return Err(refs_error(
                    "radicle_object_scan_bound",
                    "ci-job object discovery exceeded its named bound".to_string(),
                ));
            }
            object_ids.insert(object_id);
        }
        Ok(object_ids.into_iter().collect())
    }
}

impl fmt::Debug for CobRepository<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CobRepository")
            .finish_non_exhaustive()
    }
}

impl change::Storage for CobRepository<'_> {
    type StoreError = <git2::Repository as change::Storage>::StoreError;
    type LoadError = <git2::Repository as change::Storage>::LoadError;
    type ObjectId = Oid;
    type Parent = Oid;
    type PublicKey = PublicKey;
    type Signature = <git2::Repository as change::Storage>::Signature;

    fn store(
        &self,
        resource: Option<Self::Parent>,
        related: Vec<Self::Parent>,
        signer: &impl Signer,
        template: change::Template<Self::ObjectId>,
    ) -> Result<change::Entry, Self::StoreError> {
        self.backend.store(resource, related, signer, template)
    }

    fn load(&self, id: Self::ObjectId) -> Result<change::Entry, Self::LoadError> {
        self.backend.load(id)
    }

    fn parents_of(&self, id: &Oid) -> Result<Vec<Oid>, Self::LoadError> {
        self.backend.parents_of(id)
    }

    fn manifest_of(&self, id: &Oid) -> Result<Manifest, Self::LoadError> {
        self.backend.manifest_of(id)
    }
}

impl ObjectStorage for CobRepository<'_> {
    type ObjectsError = StorageError;
    type TypesError = StorageError;
    type UpdateError = StorageError;
    type RemoveError = StorageError;
    type Namespace = PublicKey;

    fn objects(
        &self,
        typename: &TypeName,
        object_id: &ObjectId,
    ) -> Result<Objects, Self::ObjectsError> {
        let pattern = format!("refs/namespaces/*/refs/cobs/{typename}/{object_id}");
        let mut references = Vec::new();
        let glob = self.backend.references_glob(&pattern).map_err(|error| {
            refs_error(
                "radicle_refs_unreadable",
                format!("local ci-job references could not be read: {error}"),
            )
        })?;
        for reference in glob {
            let reference = reference.map_err(|error| {
                refs_error(
                    "radicle_refs_unreadable",
                    format!("local ci-job reference could not be read: {error}"),
                )
            })?;
            let raw_name = reference.name().map_err(|error| {
                refs_error(
                    "radicle_ref_name_invalid",
                    format!("ci-job reference name was unreadable: {error}"),
                )
            })?;
            let target = peel_commit(&reference).map_err(|error| {
                refs_error(
                    "radicle_refs_unreadable",
                    format!("local ci-job reference could not be peeled: {error}"),
                )
            })?;
            references.push(Reference {
                name: ref_string(raw_name)?,
                target,
            });
        }
        Ok(Objects::from(references))
    }

    fn types(&self, typename: &TypeName) -> Result<BTreeMap<ObjectId, Objects>, Self::TypesError> {
        let pattern = format!("refs/namespaces/*/refs/cobs/{typename}/*");
        let mut objects: BTreeMap<ObjectId, Objects> = BTreeMap::new();
        let glob = self.backend.references_glob(&pattern).map_err(|error| {
            refs_error(
                "radicle_refs_unreadable",
                format!("local ci-job object listing could not be read: {error}"),
            )
        })?;
        for reference in glob {
            let reference = reference.map_err(|error| {
                refs_error(
                    "radicle_refs_unreadable",
                    format!("local ci-job reference could not be read: {error}"),
                )
            })?;
            let raw_name = reference.name().map_err(|error| {
                refs_error(
                    "radicle_ref_name_invalid",
                    format!("ci-job reference name was unreadable: {error}"),
                )
            })?;
            let target = peel_commit(&reference).map_err(|error| {
                refs_error(
                    "radicle_refs_unreadable",
                    format!("local ci-job reference could not be peeled: {error}"),
                )
            })?;
            let name = ref_string(raw_name)?;
            let parsed =
                radicle_cob::object::collaboration::parse_refstr(&name).ok_or_else(|| {
                    refs_error(
                        "radicle_ref_unparsable",
                        "A ci-job reference did not parse into its declared identities".to_string(),
                    )
                })?;
            let entry: &mut Objects = objects.entry(parsed.1).or_default();
            entry.push(Reference { name, target });
        }
        Ok(objects)
    }

    fn update(
        &self,
        namespace: &Self::Namespace,
        typename: &TypeName,
        object_id: &ObjectId,
        entry: &EntryId,
    ) -> Result<(), Self::UpdateError> {
        let name = self
            .cob_ref_name(namespace, typename, object_id)
            .map_err(StorageError::from)?;
        let entry: git2::Oid = (*entry).into();
        let message = format!("kiln status update {typename}/{object_id}");
        let result = match self.expected_ref_target {
            Some(expected) => self.backend.reference_matching(
                name.as_str(),
                entry,
                true,
                expected.into(),
                &message,
            ),
            None => self
                .backend
                .reference(name.as_str(), entry, false, &message),
        };
        result.map_err(|error| {
            refs_error(
                "radicle_ref_update_failed",
                format!("ci-job reference update failed: {error}"),
            )
        })?;
        Ok(())
    }

    fn remove(
        &self,
        namespace: &Self::Namespace,
        typename: &TypeName,
        object_id: &ObjectId,
    ) -> Result<(), Self::RemoveError> {
        let name = self
            .cob_ref_name(namespace, typename, object_id)
            .map_err(StorageError::from)?;
        let mut reference = self
            .backend
            .find_reference(name.as_str())
            .map_err(|error| {
                refs_error(
                    "radicle_ref_remove_failed",
                    format!("ci-job reference lookup failed: {error}"),
                )
            })?;
        reference.delete().map_err(|error| {
            refs_error(
                "radicle_ref_remove_failed",
                format!("ci-job reference removal failed: {error}"),
            )
        })?;
        Ok(())
    }
}

fn peel_commit(reference: &git2::Reference<'_>) -> Result<Commit, git2::Error> {
    let commit = reference.peel_to_commit()?;
    Ok(Commit {
        id: Oid::from(commit.id()),
    })
}

fn ref_string(raw: &str) -> CoreResult<RefString> {
    RefString::try_from(raw.to_string()).map_err(|_| name_diagnostic())
}

fn name_diagnostic() -> Diagnostic {
    Diagnostic::new(
        "radicle_ref_name_invalid",
        "radicle_status.refs",
        "A ci-job reference name was not a valid ref string",
    )
}
