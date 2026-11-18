let Base = import "default.ncl" in
let Accepted = std.array.at 0 Base.traceability_policy.profiles in
# Delta-spec roots of changes in flight. A root stays here only until its change
# is accepted and archived. Add only active roots.
let ActiveRoots = [
  ".cairn/changes/adopt-technical-graphics-standard/specs/visual-identity/spec.md",
  ".cairn/changes/adopt-technical-graphics-standard/specs/live-console-design/spec.md",
  ".cairn/changes/clarify-patchgate-receipts/specs/patchgate-review-adapter/spec.md",
  ".cairn/changes/clarify-workers-page/specs/dashboard-workers/spec.md",
  ".cairn/changes/rework-cairn-changes-page/specs/cairn-workspace/spec.md",
  ".cairn/changes/add-website-exploration-examples/specs/website-exploration/spec.md",
  ".cairn/changes/add-bombadil-web-exploration/specs/web-exploration-bombadil/spec.md",
  ".cairn/changes/fix-console-topbar-overflow/specs/live-console-design/spec.md",
  ".cairn/changes/complete-implementation-recovery/specs/implementation-work/spec.md",
  ".cairn/changes/complete-independent-verification/specs/independent-verification/spec.md",
  ".cairn/changes/recover-exact-publication/specs/publication-recovery/spec.md",
  ".cairn/changes/qualify-implementation-composition/specs/implementation-qualification/spec.md",
  ".cairn/changes/integrate-retained-mobility/specs/retained-mobility-runtime/spec.md",
  ".cairn/changes/benchmark-artifact-readiness/specs/artifact-readiness/spec.md",
  ".cairn/changes/evaluate-archil-workspace-provider/specs/workspace-provider-evaluation/spec.md",
  ".cairn/changes/materialize-projected-workspaces/specs/source-projection/spec.md",
  ".cairn/changes/resolve-projected-build-inputs/specs/source-projection/spec.md",
]
in
let Active =
  Accepted
  & {
    id | force = "kiln-active-development",
    requirement_sources | force =
      Accepted.requirement_sources
      @ std.array.map
        (fun source_root =>
          {
            root = source_root,
            source_kind = "requirements_doc",
            extensions = ["md"],
          }
        )
        ActiveRoots,
    evidence_sources | force =
      Accepted.evidence_sources
      @ [
        {
          root = "docs/implementation-runbook.md",
          source_kind = "tool_source",
          extensions = ["md"],
        },
      ],
  }
in
let NegativeControl =
  Accepted
  & {
    id | force = "kiln-coverage-negative-control",
    requirement_sources | force = [
      {
        root = "requirements.md",
        source_kind = "requirements_doc",
        extensions = ["md"],
      }
    ],
    evidence_sources | force = [
      {
        root = "evidence.rs",
        source_kind = "rust_source",
        extensions = ["rs"],
      }
    ],
  }
in
Base
& {
  traceability_policy | force =
    Base.traceability_policy
    & {
      profiles | force = Base.traceability_policy.profiles @ [Active, NegativeControl],
    },
}
