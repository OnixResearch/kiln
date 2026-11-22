let Base = import "default.ncl" in
let Accepted = std.array.at 0 Base.traceability_policy.profiles in
# Delta-spec roots of changes in flight. A root stays here only until its change
# is accepted and archived. Add only active roots.
let ActiveRoots = [
  ".cairn/changes/bind-implementation-authority/specs/implementation-authority/spec.md",
  ".cairn/changes/complete-independent-verification/specs/independent-verification/spec.md",
  ".cairn/changes/recover-exact-publication/specs/publication-recovery/spec.md",
  ".cairn/changes/integrate-retained-mobility/specs/retained-mobility-runtime/spec.md",
  ".cairn/changes/materialize-projected-workspaces/specs/source-projection/spec.md",
  ".cairn/changes/bind-attempt-grant-lineage/specs/attempt-grant-lineage/spec.md",
  ".cairn/changes/scope-source-projection-authority/specs/source-projection/spec.md",
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
