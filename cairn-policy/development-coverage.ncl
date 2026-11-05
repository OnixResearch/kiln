let Base = import "default.ncl" in
let Accepted = std.array.at 0 Base.traceability_policy.profiles in
# The monorepo work-view change is in flight; its delta-spec root is selected
# here until the change is accepted and archived. Add only active roots.
let ActiveRoots = [
  ".cairn/changes/add-monorepo-work-views/specs/work-monorepo-views",
] in
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
    evidence_sources | force = Accepted.evidence_sources,
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
