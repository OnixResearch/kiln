# Runtime pin verification

Issue #37 is a documentation/provenance cutover, not a runtime deployment.
Kiln operator, Radicle workflow, Lattice workflow, Aspen evidence and reference-pin guides now select external runtime `85a38b6a4bceeae4bb2057cf157e21752100f1b5` independently of the unchanged MIT `70496e67c7fd4a8b05914161a8e09de2759bebc8` contract import.

Primary source inspection:
- https://github.com/OnixResearch/lattice/commit/6339864d3c4e5de72ba4f0899b11b5d5b76704b6 establishes shell-result propagation and the `shell_success` requirement.
- https://github.com/OnixResearch/lattice/blob/85a38b6a4bceeae4bb2057cf157e21752100f1b5/src/workflow_exchange/application/shell.rs shows the current handler refusing non-Cancel controls and persisting cancellation requests before signaling its worker.
- The documented observed-cancellation revision remains `813076d7e42d0ddfb65042bee615bc5f12a2b4d3`.

The historical Aspen runtime `c513d94d89e901ffa56ae67f375f973e55958e42` is explicitly not qualification of the selected CI handler. No vendored crate, license record, schema, standalone host dependency, executable or deployed service was changed. Source inspection is not runtime qualification. Native proposal/design/tasks gates passed before implementation; final integrated document/lifecycle checks are owned by the parent issue closeout.
