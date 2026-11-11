## Decisions

Emission belongs in `kiln-core` as a pure function with no filesystem, clock,
environment, provider or transport access. The shell only retains the resulting
bytes under the existing report kind and displays them.

Canonical JSON is produced through `serde_json`'s allocation-only mode. Its
object map is ordered, so keys are lexicographic at every depth and output is
compact. A golden fixture pins the exact bytes; enabling `preserve_order` or
changing field layout fails that test instead of silently changing identities.

Products and native digests are sorted and deduplicated. Two native identities
that normalize to one digest key but carry different values or URIs are
refused; silently merging them would lose one admitted identity.

## Field mapping

| Statement field | Projection fact |
|---|---|
| `subject[].name` / `.digest.blake3` | each product `ContentId` |
| `predicate.buildDefinition.buildType` | `urn:kiln:provenance:ci-run:v1` |
| `buildDefinition.externalParameters.kilnControlPlane` | `roles.kiln_control_plane` |
| `buildDefinition.externalParameters.pipelineRevision` | `pipeline_revision` |
| `buildDefinition.externalParameters.sourceOrCandidate` | `source_or_candidate` |
| `buildDefinition.externalParameters.interchangeProfile` | `profile` |
| `buildDefinition.internalParameters.provider` | `roles.provider` |
| `buildDefinition.internalParameters.worker` | `roles.worker` or `null` |
| `buildDefinition.resolvedDependencies` | source/candidate plus native digests |
| `runDetails.builder.id` | `roles.builder` |
| `runDetails.metadata.invocationId` | `producing_attempt` |

No field is derived from another. `signer` is intentionally absent because the
statement is unsigned; a future DSSE envelope must keep it separate too.

## Non-claims

The statement is an identity projection over admitted facts. It does not prove
the build happened, that products match their digests, or that the builder acted
correctly. Schema validity and any later signature are not builder, signer,
SLSA-level, gate, merge or release authority. Kiln never names itself as the
builder unless a builder role says so.
