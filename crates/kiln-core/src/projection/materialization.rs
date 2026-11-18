//! Plan the materialization of an admitted complete projection (#78).
//!
//! A plan turns a [`CompleteProjection`] and a versioned materializer profile
//! into the exact entries to write and two identities: a reusable input
//! identity shared by every attempt over the same manifest and profile, and an
//! overlay identity bound to one attempt and fence. A [`WorkspacePin`] carries
//! both, with the source, definition, manifest and profile they came from, so
//! dispatch and the retained overlay agree on one workspace. The shell reads,
//! verifies and writes; this module performs no I/O and grants no read, write
//! or execution authority.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    CompleteProjection, ComponentVersion, EntryKind, EntryOrigin, Findings, ProjectionOperation,
    TargetProfile, shown, update_target,
};
use crate::monorepo::{finish, update_field};
use crate::types::{ContentId, Diagnostic, ExternalRef};

pub const MATERIALIZER_PROFILE_SCHEMA: &str = "kiln.materializer-profile.v1";
pub const WORKSPACE_PIN_SCHEMA: &str = "kiln.materialization.workspace-pin.v1";
const PROFILE_IDENTITY_SCHEMA: &str = "kiln.materializer-profile.identity.v1";
const INPUT_IDENTITY_SCHEMA: &str = "kiln.materialization.input.v1";
const OVERLAY_IDENTITY_SCHEMA: &str = "kiln.materialization.overlay.v1";

/// How a materializer constructs the initial tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializerKind {
    /// Every entry is written to a local directory before the attempt starts.
    EagerLocalDirectory,
}

impl MaterializerKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::EagerLocalDirectory => "eager_local_directory",
        }
    }
}

/// The materializer implementation and the platform it writes to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializerProfile {
    pub schema: String,
    pub kind: MaterializerKind,
    pub materializer: ComponentVersion,
    pub platform: TargetProfile,
}

impl MaterializerProfile {
    /// BLAKE3 over the schema, kind, materializer and platform.
    #[must_use]
    pub fn identity(&self) -> ContentId {
        let mut hasher = blake3::Hasher::new();
        update_field(&mut hasher, "schema", PROFILE_IDENTITY_SCHEMA.as_bytes());
        update_field(&mut hasher, "profile.schema", self.schema.as_bytes());
        update_field(&mut hasher, "kind", self.kind.as_str().as_bytes());
        update_field(
            &mut hasher,
            "materializer.name",
            self.materializer.name.as_bytes(),
        );
        update_field(
            &mut hasher,
            "materializer.version",
            self.materializer.version.as_bytes(),
        );
        update_target(&mut hasher, self.platform);
        finish(hasher)
    }
}

/// One entry to write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedEntry {
    pub destination: String,
    pub executable: bool,
    /// Provider-native object holding the bytes; the shell verifies it.
    pub object: ExternalRef,
}

/// The entries and identities of one materialization. Only [`plan`]
/// constructs one, from a completeness witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationPlan {
    repository: ContentId,
    source: ExternalRef,
    definition: ContentId,
    manifest: ContentId,
    profile: ContentId,
    operation: ProjectionOperation,
    input_identity: ContentId,
    entries: Vec<PlannedEntry>,
}

impl MaterializationPlan {
    /// Identity of the manifest this plan materializes.
    #[must_use]
    pub fn manifest(&self) -> &ContentId {
        &self.manifest
    }

    /// The operation the completeness witness admitted.
    #[must_use]
    pub fn operation(&self) -> ProjectionOperation {
        self.operation
    }

    /// Reusable identity of the immutable input every attempt shares.
    #[must_use]
    pub fn input_identity(&self) -> &ContentId {
        &self.input_identity
    }

    /// Entries in destination order.
    #[must_use]
    pub fn entries(&self) -> &[PlannedEntry] {
        &self.entries
    }

    /// Identity of one attempt's private writable overlay over this input.
    pub fn overlay_identity(
        &self,
        attempt: &ContentId,
        fence: u64,
    ) -> Result<ContentId, Diagnostic> {
        overlay_identity(&self.input_identity, attempt, fence)
    }

    /// The pin dispatch and the retained overlay of one attempt carry.
    // r[impl kiln.source.projection.materialization.scenario.pinned]
    pub fn pin(&self, attempt: &ContentId, fence: u64) -> Result<WorkspacePin, Diagnostic> {
        Ok(WorkspacePin {
            schema: WORKSPACE_PIN_SCHEMA.into(),
            repository: self.repository.clone(),
            source: self.source.clone(),
            definition: self.definition.clone(),
            manifest: self.manifest.clone(),
            profile: self.profile.clone(),
            input: self.input_identity.clone(),
            overlay: self.overlay_identity(attempt, fence)?,
            attempt: attempt.clone(),
            fence,
        })
    }
}

/// Identity of one attempt's private writable overlay over an input.
pub fn overlay_identity(
    input: &ContentId,
    attempt: &ContentId,
    fence: u64,
) -> Result<ContentId, Diagnostic> {
    input.validate()?;
    attempt.validate()?;
    if fence == 0 {
        return Err(Diagnostic::new(
            "materialization_fence",
            "materialization.fence",
            "an overlay needs a non-zero attempt fence",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", OVERLAY_IDENTITY_SCHEMA.as_bytes());
    update_field(&mut hasher, "input", input.as_str().as_bytes());
    update_field(&mut hasher, "attempt", attempt.as_str().as_bytes());
    update_field(&mut hasher, "fence", &fence.to_le_bytes());
    Ok(finish(hasher))
}

/// Everything one attempt's projected workspace is prepared from: the
/// admitted repository and exact source revision, the projection definition
/// and resolved manifest, the materializer profile, the shared input and this
/// attempt's overlay generation. Dispatch and the retained overlay binding
/// carry the same pin.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePin {
    pub schema: String,
    pub repository: ContentId,
    pub source: ExternalRef,
    pub definition: ContentId,
    pub manifest: ContentId,
    pub profile: ContentId,
    pub input: ContentId,
    pub overlay: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
}

impl WorkspacePin {
    /// The pin's identities are well formed and its overlay is the identity
    /// of its input, attempt and fence.
    // r[impl kiln.source.projection.materialization.scenario.pinned]
    pub fn validate(&self) -> Result<(), Diagnostic> {
        for identity in [
            &self.repository,
            &self.definition,
            &self.manifest,
            &self.profile,
        ] {
            identity.validate()?;
        }
        if self.schema != WORKSPACE_PIN_SCHEMA
            || overlay_identity(&self.input, &self.attempt, self.fence)? != self.overlay
        {
            return Err(Diagnostic::new(
                "materialization_pin",
                "materialization.pin",
                "workspace pin schema or overlay generation is inconsistent",
            ));
        }
        Ok(())
    }
}

// r[impl kiln.source.projection.materialization]
// r[impl kiln.source.projection.materialization.scenario.refused]
/// Plan the materialization of a complete projection under one profile.
///
/// Eager local directories write native file and executable entries only:
/// symbolic links and generated entries are refused until a profile
/// qualifies them, as are executable entries on a platform without
/// executable bits, a platform whose case rules differ from the projection
/// target and destinations longer than the platform bound. Every fault is
/// reported before any effect.
pub fn plan(
    complete: &CompleteProjection,
    profile: &MaterializerProfile,
) -> Result<MaterializationPlan, Vec<Diagnostic>> {
    let mut findings = Findings::default();
    let manifest = complete.manifest();
    if profile.schema != MATERIALIZER_PROFILE_SCHEMA {
        findings.push(
            "materializer_profile_schema",
            "profile.schema",
            "unsupported materializer profile schema",
        );
    }
    if !profile.materializer.admissible() {
        findings.push(
            "materializer_component",
            "profile.materializer",
            "materializer name or version is empty, oversized or malformed",
        );
    }
    let platform = profile.platform;
    if platform.case_sensitive != manifest.target.case_sensitive {
        findings.push(
            "materializer_case_rules",
            "profile.platform.case_sensitive",
            "the platform's case rules differ from the projection target",
        );
    }
    let mut entries = Vec::with_capacity(manifest.entries.len());
    for entry in &manifest.entries {
        let field = format!("destinations.{}", shown(&entry.destination));
        if entry.destination.len() > platform.max_path_bytes {
            findings.push(
                "materializer_path_length",
                field.clone(),
                "the destination exceeds the platform path bound",
            );
        }
        match (&entry.origin, entry.kind) {
            (EntryOrigin::Generated { .. }, _) => findings.push(
                "materializer_generated_entry",
                field,
                "eager local materialization writes source entries only; generated entries need a generator provider",
            ),
            (EntryOrigin::Native { .. }, EntryKind::Symlink) => findings.push(
                "materializer_symlink",
                field,
                "symbolic links are refused until a materializer profile qualifies them",
            ),
            (EntryOrigin::Native { .. }, EntryKind::Executable) if !platform.executable_bit => {
                findings.push(
                    "materializer_executable_bit",
                    field,
                    "the platform cannot represent an executable entry",
                );
            }
            (EntryOrigin::Native { object, .. }, kind) => entries.push(PlannedEntry {
                destination: entry.destination.clone(),
                executable: kind == EntryKind::Executable,
                object: object.clone(),
            }),
        }
    }
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", INPUT_IDENTITY_SCHEMA.as_bytes());
    update_field(
        &mut hasher,
        "manifest",
        manifest.identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "content",
        manifest.content_identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "materializer.kind",
        profile.kind.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "materializer.name",
        profile.materializer.name.as_bytes(),
    );
    update_field(
        &mut hasher,
        "materializer.version",
        profile.materializer.version.as_bytes(),
    );
    update_target(&mut hasher, platform);
    findings.finish(MaterializationPlan {
        repository: manifest.repository.clone(),
        source: manifest.exact_revision.clone(),
        definition: manifest.definition.clone(),
        manifest: manifest.identity.clone(),
        profile: profile.identity(),
        operation: complete.operation(),
        input_identity: finish(hasher),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::super::{
        AdmittedRepository, Coverage, DependencyFact, DependencyStatus, GeneratedEntry,
        GeneratedInputs, ProjectionDefinition, ProjectionFacts, ProviderBinding,
        RepositoryEnrollment, RepositoryLocator, SourceCapability, SourceEntry, admit_definition,
        admit_repositories, resolve,
    };
    use super::*;
    use crate::types::ProviderProfileId;
    use alloc::string::ToString;
    use alloc::vec;

    const EXPORTED_DEFINITION: &str =
        include_str!("../../../../fixtures/source-projection/definition.json");

    fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn digest(label: &str) -> ContentId {
        ok(ContentId::from_blake3_hex(
            blake3::hash(label.as_bytes()).to_hex().to_string(),
        ))
    }

    fn external(namespace: &str, kind: &str, id: &str) -> ExternalRef {
        ok(ExternalRef::new(namespace, kind, id))
    }

    fn component(name: &str, version: &str) -> ComponentVersion {
        ComponentVersion {
            name: name.into(),
            version: version.into(),
        }
    }

    fn repository() -> AdmittedRepository {
        let enrolled = [RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: external("github", "repository", "OnixResearch/kiln"),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/cache/kiln".into(),
            }],
        }];
        ok(admit_repositories(&enrolled, &[]))
            .pop()
            .unwrap_or_else(|| unreachable!("one admitted repository"))
    }

    fn entry(path: &str, blob: &str, kind: EntryKind) -> SourceEntry {
        SourceEntry {
            path: path.into(),
            object: external("git", "blob", blob),
            kind,
        }
    }

    fn facts(repository: &AdmittedRepository, entries: Vec<SourceEntry>) -> ProjectionFacts {
        ProjectionFacts {
            repository: repository.identity().clone(),
            exact_revision: external("git", "commit", &"a".repeat(40)),
            canonical_tree: digest("tree-1"),
            evaluator: component("kiln-projection", "1"),
            adapter: component("git-source", "1"),
            source_capabilities: vec![
                SourceCapability::ExactTree,
                SourceCapability::ExecutableBit,
                SourceCapability::Symlinks,
            ],
            coverage: Coverage::Complete,
            entries,
            dependencies: vec![DependencyFact {
                name: "workspace-manifest".into(),
                status: DependencyStatus::Present {
                    identity: digest("workspace-manifest"),
                },
            }],
            generated: Vec::new(),
        }
    }

    fn base_entries() -> Vec<SourceEntry> {
        vec![
            entry("crates/kiln-core/Cargo.toml", "b1", EntryKind::File),
            entry("crates/kiln-core/src/lib.rs", "b2", EntryKind::File),
            entry("crates/kiln-core/run.sh", "b3", EntryKind::Executable),
        ]
    }

    fn complete(definition: &ProjectionDefinition, facts: &ProjectionFacts) -> CompleteProjection {
        let repository = repository();
        ok(ok(resolve(
            &ok(admit_definition(definition)),
            &repository,
            facts,
        ))
        .require_complete(ProjectionOperation::Build))
    }

    fn definition() -> ProjectionDefinition {
        ok(serde_json::from_str(EXPORTED_DEFINITION))
    }

    fn profile() -> MaterializerProfile {
        MaterializerProfile {
            schema: MATERIALIZER_PROFILE_SCHEMA.into(),
            kind: MaterializerKind::EagerLocalDirectory,
            materializer: component("kiln-eager-local", "1"),
            platform: TargetProfile::POSIX,
        }
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    // r[verify kiln.source.projection.materialization]
    #[test]
    fn plan_identities_follow_the_manifest_profile_attempt_and_fence() {
        let repository = repository();
        let projection = complete(&definition(), &facts(&repository, base_entries()));
        let first = ok(plan(&projection, &profile()));
        assert_eq!(first, ok(plan(&projection, &profile())));
        assert_eq!(first.manifest(), &projection.manifest().identity);
        assert_eq!(first.operation(), ProjectionOperation::Build);
        let planned: Vec<(&str, bool)> = first
            .entries()
            .iter()
            .map(|entry| (entry.destination.as_str(), entry.executable))
            .collect();
        assert_eq!(
            planned,
            vec![
                ("kiln-core/Cargo.toml", false),
                ("kiln-core/run.sh", true),
                ("kiln-core/src/lib.rs", false),
                ("manifests/kiln-core.toml", false),
            ]
        );

        let mut newer = profile();
        newer.materializer.version = "2".into();
        assert_ne!(
            first.input_identity(),
            ok(plan(&projection, &newer)).input_identity()
        );
        let changed = vec![
            entry("crates/kiln-core/Cargo.toml", "b1", EntryKind::File),
            entry("crates/kiln-core/src/lib.rs", "b9", EntryKind::File),
            entry("crates/kiln-core/run.sh", "b3", EntryKind::Executable),
        ];
        let other = complete(&definition(), &facts(&repository, changed));
        assert_ne!(
            first.input_identity(),
            ok(plan(&other, &profile())).input_identity()
        );

        let attempt_a = digest("attempt-a");
        let attempt_b = digest("attempt-b");
        let overlay = ok(first.overlay_identity(&attempt_a, 1));
        assert_eq!(overlay, ok(first.overlay_identity(&attempt_a, 1)));
        assert_ne!(overlay, ok(first.overlay_identity(&attempt_a, 2)));
        assert_ne!(overlay, ok(first.overlay_identity(&attempt_b, 1)));
        assert_ne!(&overlay, first.input_identity());
        assert!(first.overlay_identity(&attempt_a, 0).is_err());
    }

    // r[verify kiln.source.projection.materialization.scenario.pinned]
    #[test]
    fn pins_bind_the_exact_source_definition_manifest_profile_and_overlay() {
        let repository = repository();
        let projection = complete(&definition(), &facts(&repository, base_entries()));
        let planned = ok(plan(&projection, &profile()));
        let attempt = digest("attempt-a");
        let pin = ok(planned.pin(&attempt, 3));
        let manifest = projection.manifest();
        assert_eq!(
            (&pin.repository, &pin.source, &pin.definition, &pin.manifest),
            (
                &manifest.repository,
                &manifest.exact_revision,
                &manifest.definition,
                &manifest.identity
            )
        );
        assert_eq!(pin.profile, profile().identity());
        assert_eq!(&pin.input, planned.input_identity());
        assert_eq!(pin.overlay, ok(planned.overlay_identity(&attempt, 3)));
        assert_eq!(pin.validate(), Ok(()));
        let mut newer = profile();
        newer.materializer.version = "2".into();
        assert_ne!(newer.identity(), profile().identity());
        assert_ne!(
            ok(plan(&projection, &newer)).pin(&attempt, 3),
            Ok(pin.clone())
        );
        type PinChange = fn(&mut WorkspacePin);
        let changes: [PinChange; 4] = [
            |pin| pin.fence = 4,
            |pin| pin.attempt = digest("attempt-b"),
            |pin| pin.input = digest("other-input"),
            |pin| pin.schema = "kiln.materialization.workspace-pin.v0".into(),
        ];
        for change in changes {
            let mut altered = pin.clone();
            change(&mut altered);
            assert!(altered.validate().is_err(), "{altered:?}");
        }
        assert!(planned.pin(&attempt, 0).is_err());
    }

    type ProfileChange = fn(&mut MaterializerProfile);

    // r[verify kiln.source.projection.materialization.scenario.refused]
    #[test]
    fn unsupported_entries_and_platforms_are_refused_before_any_effect() {
        let repository = repository();
        let projection = complete(&definition(), &facts(&repository, base_entries()));
        let cases: [(ProfileChange, &str); 5] = [
            (
                |p| p.schema = "kiln.materializer-profile.v0".into(),
                "materializer_profile_schema",
            ),
            (
                |p| p.materializer.version = String::new(),
                "materializer_component",
            ),
            (
                |p| p.platform.case_sensitive = false,
                "materializer_case_rules",
            ),
            (
                |p| p.platform.executable_bit = false,
                "materializer_executable_bit",
            ),
            (
                |p| p.platform.max_path_bytes = 12,
                "materializer_path_length",
            ),
        ];
        for (change, code) in cases {
            let mut refused = profile();
            change(&mut refused);
            let diagnostics = plan(&projection, &refused).err().unwrap_or_default();
            assert!(
                codes(&diagnostics).contains(&code),
                "{code}: {diagnostics:?}"
            );
        }

        let mut linked = base_entries();
        linked.push(entry("crates/kiln-core/link", "b4", EntryKind::Symlink));
        let with_link = complete(&definition(), &facts(&repository, linked));
        let diagnostics = plan(&with_link, &profile()).err().unwrap_or_default();
        assert_eq!(codes(&diagnostics), vec!["materializer_symlink"]);
        assert_eq!(
            diagnostics.first().map(|d| d.field.as_str()),
            Some("destinations.kiln-core/link")
        );

        let mut generating = definition();
        generating.generated_inputs = GeneratedInputs::Admitted;
        let mut generated = facts(&repository, base_entries());
        generated.generated.push(GeneratedEntry {
            destination: "kiln-core/src/generated.rs".into(),
            kind: EntryKind::File,
            generator: component("codegen", "1"),
            input_identity: digest("codegen-input"),
        });
        let with_generated = complete(&generating, &generated);
        let diagnostics = plan(&with_generated, &profile()).err().unwrap_or_default();
        assert_eq!(codes(&diagnostics), vec!["materializer_generated_entry"]);
    }
}
