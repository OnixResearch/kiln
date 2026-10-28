let Base = import "default.ncl" in
let Accepted = std.array.at 0 Base.traceability_policy.profiles in
let ActiveRoots = [
  ".cairn/changes/add-dashboard-workers-view/specs",
  ".cairn/changes/add-patchgate-review-adapter/specs",
  ".cairn/changes/add-source-browser/specs",
  ".cairn/changes/build-dashboard-product-interface/specs",
  ".cairn/changes/complete-internal-developer-loop/specs",
  ".cairn/changes/enable-web-remote-lattice-workers/specs",
  ".cairn/changes/extend-live-project-registry/specs",
  ".cairn/changes/fix-cob-rendering/specs",
  ".cairn/changes/use-rats-browser-index/specs",
  ".cairn/changes/fix-log-viewing/specs",
  ".cairn/changes/improve-lattice-rendering/specs",
  ".cairn/changes/repair-workspace-usability/specs",
  ".cairn/changes/show-local-run-time-check-cobs/specs/dashboard-local-time",
  ".cairn/changes/add-cob-workspace/specs",
] in
let Active = Accepted & {
  id | force = "kiln-active-development",
  requirement_sources | force = Accepted.requirement_sources @
    std.array.map (fun source_root => {
      root = source_root,
      source_kind = "requirements_doc",
      extensions = ["md"],
    }) ActiveRoots,
  evidence_sources | force = Accepted.evidence_sources @ [
    { root = "crates", source_kind = "tool_source", extensions = ["js", "css", "html"] },
    { root = "web/dashboard-index/src", source_kind = "rust_source", extensions = ["rs"] },
    { root = "docs/observation-dashboard.md", source_kind = "tool_source", extensions = ["md"] },
    { root = "docs/operator.md", source_kind = "tool_source", extensions = ["md"] },
  ],
} in
let NegativeControl = Accepted & {
  id | force = "kiln-coverage-negative-control",
  requirement_sources | force = [{
    root = "requirements.md",
    source_kind = "requirements_doc",
    extensions = ["md"],
  }],
  evidence_sources | force = [{
    root = "evidence.rs",
    source_kind = "rust_source",
    extensions = ["rs"],
  }],
} in
Base & {
  traceability_policy | force = Base.traceability_policy & {
    profiles | force = Base.traceability_policy.profiles @ [Active, NegativeControl],
  },
}
