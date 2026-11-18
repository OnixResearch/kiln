//! One authority mode and one publication owner per repository.
//!
//! A repository is either native-authoritative, where the native provider
//! publishes and the graph is a derived import, or explicitly
//! graph-authoritative, where the graph publishes roots and native refs are
//! declared exports. Every publication names its expected state; a stale or
//! independently moved side is refused, never merged by last writer. Records
//! here carry no capability, grant or delegation.
//!
//! r[impl kiln.source.graph.authority]

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    ExportRecord, Findings, ImportRecord, MAX_TEXT_BYTES, SNAPSHOT_SCHEMA, check_identity,
    valid_text,
};
use crate::projection::ComponentVersion;
use crate::types::{ContentId, Diagnostic, ExternalRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityMode {
    /// The native provider publishes; the graph is a derived import.
    NativeAuthoritative,
    /// The graph publishes roots; native refs are declared exports.
    GraphAuthoritative,
}

/// The recorded authority of one repository: its mode, its one publication
/// owner and the last recorded state of each side. Outside this module it only
/// starts native-authoritative ([`AuthorityState::native`]) and changes through
/// [`admit_publication`], [`record_import`], [`reconcile`] and [`migrate`], so
/// graph authority always follows an explicit migration. A deserialized state is
/// the authority owner's durable record and is trusted as such.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityState {
    pub(super) repository: ContentId,
    pub(super) mode: AuthorityMode,
    pub(super) owner: String,
    pub(super) graph_root: Option<ContentId>,
    pub(super) native_revision: Option<ExternalRef>,
}

/// What the shell observed on both sides immediately before a publication,
/// reconciliation or migration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedState {
    pub graph_root: Option<ContentId>,
    pub native_revision: Option<ExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublicationTarget {
    GraphRoot {
        expected: Option<ContentId>,
        next: ContentId,
    },
    NativeRevision {
        expected: Option<ExternalRef>,
        next: ExternalRef,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub publisher: String,
    pub target: PublicationTarget,
}

/// The owner's explicit decision that the next declared export replaces a
/// native revision that moved independently.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reconciliation {
    pub publisher: String,
    pub replace_native: ExternalRef,
}

/// An explicit mode change binding the last native revision to the first graph
/// root (or, rolling back, the final root's export to the native side).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeMigration {
    pub publisher: String,
    pub to: AuthorityMode,
    pub owner: String,
    pub native_revision: ExternalRef,
    pub graph_root: ContentId,
}

fn check_owner(findings: &mut Findings, state: &AuthorityState, publisher: &str) {
    if publisher != state.owner {
        findings.push(
            "not_publication_owner",
            "publisher",
            "only the recorded owner publishes for this repository",
        );
    }
}

fn check_side<T: PartialEq>(
    findings: &mut Findings,
    field: &str,
    expected: Option<&T>,
    recorded: Option<&T>,
    observed: Option<&T>,
) {
    if expected != recorded || expected != observed {
        findings.push(
            "stale_expected_state",
            field,
            "the expected state differs from the recorded or observed state; reconcile explicitly",
        );
    }
}

impl AuthorityState {
    /// A native-authoritative repository with one publication owner.
    pub fn native(
        repository: ContentId,
        owner: &str,
        native_revision: Option<ExternalRef>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut findings = Findings::default();
        if let Err(diagnostics) = check_identity(&repository, "repository") {
            findings.0.extend(diagnostics);
        }
        if !valid_text(owner, MAX_TEXT_BYTES) {
            findings.push("invalid_owner", "owner", "an owner must be bounded text");
        }
        if native_revision
            .as_ref()
            .is_some_and(|revision| revision.validate().is_err())
        {
            findings.push(
                "invalid_external_ref",
                "native_revision",
                "the native revision is not a valid reference",
            );
        }
        findings.finish(Self {
            repository,
            mode: AuthorityMode::NativeAuthoritative,
            owner: owner.into(),
            graph_root: None,
            native_revision,
        })
    }

    #[must_use]
    pub fn repository(&self) -> &ContentId {
        &self.repository
    }

    #[must_use]
    pub fn mode(&self) -> AuthorityMode {
        self.mode
    }

    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    #[must_use]
    pub fn graph_root(&self) -> Option<&ContentId> {
        self.graph_root.as_ref()
    }

    #[must_use]
    pub fn native_revision(&self) -> Option<&ExternalRef> {
        self.native_revision.as_ref()
    }
}

// r[impl kiln.source.graph.authority.scenario.conflict]
/// Admit one publication. Only the owner publishes, only the authoritative side
/// (or, when graph-authoritative, a declared export of the current root)
/// changes, and the expected state must equal both the recorded and the
/// observed state. A native ref that moved independently of a graph-authoritative
/// repository refuses every export until the owner reconciles it explicitly.
pub fn admit_publication(
    state: &AuthorityState,
    observed: &ObservedState,
    publication: &Publication,
    export: Option<&ExportRecord>,
) -> Result<AuthorityState, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    check_owner(&mut findings, state, &publication.publisher);
    let mut next_state = state.clone();
    match (state.mode, &publication.target) {
        (AuthorityMode::NativeAuthoritative, PublicationTarget::GraphRoot { .. }) => findings.push(
            "non_authoritative_side",
            "target",
            "a native-authoritative repository derives its graph by import, not publication",
        ),
        (AuthorityMode::GraphAuthoritative, PublicationTarget::GraphRoot { expected, next }) => {
            check_side(
                &mut findings,
                "graph_root",
                expected.as_ref(),
                state.graph_root.as_ref(),
                observed.graph_root.as_ref(),
            );
            if let Err(diagnostics) = check_identity(next, "next") {
                findings.0.extend(diagnostics);
            }
            next_state.graph_root = Some(next.clone());
        }
        (
            AuthorityMode::NativeAuthoritative,
            PublicationTarget::NativeRevision { expected, next },
        ) => {
            check_side(
                &mut findings,
                "native_revision",
                expected.as_ref(),
                state.native_revision.as_ref(),
                observed.native_revision.as_ref(),
            );
            if next.validate().is_err() {
                findings.push(
                    "invalid_external_ref",
                    "next",
                    "the next native revision is not a valid reference",
                );
            }
            next_state.native_revision = Some(next.clone());
        }
        (
            AuthorityMode::GraphAuthoritative,
            PublicationTarget::NativeRevision { expected, next },
        ) => {
            if observed.native_revision != state.native_revision {
                findings.push(
                    "native_moved_independently",
                    "native_revision",
                    "the native ref moved outside the declared export; reconcile explicitly",
                );
            }
            if expected.as_ref() != state.native_revision.as_ref() {
                findings.push(
                    "stale_expected_state",
                    "native_revision",
                    "the expected native revision is not the last declared export",
                );
            }
            match export {
                None => findings.push(
                    "export_required",
                    "export",
                    "a graph-authoritative native publication must be a declared export",
                ),
                Some(record) => {
                    if let Err(diagnostics) = record.verify() {
                        findings.0.extend(diagnostics);
                    }
                    if record.repository != state.repository
                        || Some(&record.root) != state.graph_root.as_ref()
                    {
                        findings.push(
                            "export_not_current_root",
                            "export",
                            "the export does not bind this repository's current graph root",
                        );
                    }
                }
            }
            if next.validate().is_err() {
                findings.push(
                    "invalid_external_ref",
                    "next",
                    "the next native revision is not a valid reference",
                );
            }
            next_state.native_revision = Some(next.clone());
        }
    }
    findings.finish(next_state)
}

/// Record the derived graph of a native-authoritative repository. The import
/// must bind the current native revision and apply to the last derived root.
pub fn record_import(
    state: &AuthorityState,
    import: &ImportRecord,
) -> Result<AuthorityState, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    if state.mode != AuthorityMode::NativeAuthoritative {
        findings.push(
            "non_authoritative_side",
            "import",
            "a graph-authoritative repository does not adopt native imports",
        );
    }
    if let Err(diagnostics) = import.verify() {
        findings.0.extend(diagnostics);
    }
    if import.repository != state.repository
        || Some(&import.revision) != state.native_revision.as_ref()
        || state
            .graph_root
            .as_ref()
            .is_some_and(|root| *root != import.base)
    {
        findings.push(
            "stale_import",
            "import",
            "the import does not bind the current native revision and derived root",
        );
    }
    let mut next = state.clone();
    next.graph_root = Some(import.root.clone());
    findings.finish(next)
}

/// Record the owner's explicit decision that the next declared export replaces
/// an independently moved native revision of a graph-authoritative repository.
pub fn reconcile(
    state: &AuthorityState,
    observed: &ObservedState,
    reconciliation: &Reconciliation,
) -> Result<AuthorityState, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    check_owner(&mut findings, state, &reconciliation.publisher);
    if state.mode != AuthorityMode::GraphAuthoritative {
        findings.push(
            "non_authoritative_side",
            "mode",
            "a native-authoritative repository re-imports its moved native revision instead",
        );
    }
    if observed.native_revision.as_ref() != Some(&reconciliation.replace_native) {
        findings.push(
            "stale_expected_state",
            "native_revision",
            "the reconciliation must name the observed native revision",
        );
    }
    let mut next = state.clone();
    next.native_revision = Some(reconciliation.replace_native.clone());
    findings.finish(next)
}

/// Change mode explicitly. The migration binds the current native revision to
/// the current graph root, both recorded and observed, and names the new owner.
pub fn migrate(
    state: &AuthorityState,
    observed: &ObservedState,
    migration: &ModeMigration,
) -> Result<AuthorityState, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    check_owner(&mut findings, state, &migration.publisher);
    if migration.to == state.mode {
        findings.push(
            "mode_unchanged",
            "to",
            "a migration must change the authority mode",
        );
    }
    if !valid_text(&migration.owner, MAX_TEXT_BYTES) {
        findings.push("invalid_owner", "owner", "an owner must be bounded text");
    }
    let native = Some(&migration.native_revision);
    let root = Some(&migration.graph_root);
    if native != state.native_revision.as_ref()
        || native != observed.native_revision.as_ref()
        || root != state.graph_root.as_ref()
        || root != observed.graph_root.as_ref()
    {
        findings.push(
            "stale_migration",
            "migration",
            "a migration must bind the current native revision to the current graph root",
        );
    }
    let mut next = state.clone();
    next.mode = migration.to;
    next.owner.clone_from(&migration.owner);
    findings.finish(next)
}

/// The exact subject evidence was produced for.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSubject {
    pub schema: String,
    pub graph_root: ContentId,
    pub importer: Option<ComponentVersion>,
    pub exporter: Option<ComponentVersion>,
    pub layout: Option<ContentId>,
    pub export: Option<ContentId>,
}

impl EvidenceSubject {
    /// The subject of evidence produced over one export.
    #[must_use]
    pub fn of_export(record: &ExportRecord, importer: Option<&ComponentVersion>) -> Self {
        Self {
            schema: SNAPSHOT_SCHEMA.into(),
            graph_root: record.root.clone(),
            importer: importer.cloned(),
            exporter: Some(record.exporter.clone()),
            layout: Some(record.layout.clone()),
            export: Some(record.identity.clone()),
        }
    }
}

/// Whether evidence bound to `bound` still applies to `current`. Every changed
/// component is named; any change makes the evidence inapplicable.
pub fn evidence_applicable(
    bound: &EvidenceSubject,
    current: &EvidenceSubject,
) -> Result<(), Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let changes = [
        (
            bound.schema != current.schema,
            "evidence_schema_changed",
            "schema",
        ),
        (
            bound.graph_root != current.graph_root,
            "evidence_graph_root_changed",
            "graph_root",
        ),
        (
            bound.importer != current.importer,
            "evidence_importer_changed",
            "importer",
        ),
        (
            bound.exporter != current.exporter,
            "evidence_exporter_changed",
            "exporter",
        ),
        (
            bound.layout != current.layout,
            "evidence_layout_changed",
            "layout",
        ),
        (
            bound.export != current.export,
            "evidence_export_changed",
            "export",
        ),
    ];
    for (changed, code, field) in changes {
        if changed {
            findings.push(
                code,
                field,
                "evidence binds a different identity than the current subject",
            );
        }
    }
    findings.finish(())
}
