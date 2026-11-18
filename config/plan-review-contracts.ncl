# Typed authoring contract for the frozen plan-review cases (#93). Rust
# (`kiln_core::implementation::plan_review_receipt`) is authoritative and
# replays every case through the review rules.
let MaxCases = 16 in
let MaxRevisions = 16 in
let MaxSteps = 64 in
let MaxTextBytes = 4096 in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Integer = fun low high =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= low && value <= high && std.number.floor value == value
  )
in
let Name =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-z][a-z0-9-]{0,63}$" value
  )
in
let Text =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length (std.string.trim value) >= 1
    && std.string.length value <= MaxTextBytes
  )
in
let Hex64 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let distinct = fun values =>
  std.array.all (fun value => std.array.length (std.array.filter (fun other => other == value) values) == 1) values
in
let Step = { id | Name, text | Text } in
let Span = { start | Integer 0 MaxTextBytes, end | Integer 1 MaxTextBytes } in
let Region = { step | Name, span | Span | optional } in
# A deletion names a whole step; a comment or replacement carries text.
let Note =
  std.contract.from_predicate (fun note =>
    std.is_record note
    && std.record.has_field "kind" note
    && (
      (note.kind == 'delete && std.record.fields note == ["kind"])
      || (
        (note.kind == 'comment || note.kind == 'replace)
        && std.record.fields note == ["kind", "text"]
        && std.is_string note.text
        && std.string.length (std.string.trim note.text) >= 1
        && std.string.length note.text <= MaxTextBytes
      )
    )
  )
in
let Annotation = {
  id | Name,
  revision | Integer 1 MaxRevisions,
  region | Region,
  note | Note,
  blocking | Bool,
}
in
let Resolution = { annotation | Name, revision | Integer 1 MaxRevisions } in
let Decision = { revision | Integer 1 MaxRevisions, verdict | [| 'accepted, 'changes_requested |] } in
let Entry = {
  id | Name,
  provenance | [| 'synthetic, 'agent_capture |],
  author | Name,
  revisions | Array (Array Step),
  annotations | Array Annotation,
  resolutions | Array Resolution,
  decisions | Array Decision,
}
in
let in_range = fun count items => std.array.all (fun item => item.revision <= count) items in
{
  Cases = {
    schema | Exact "kiln.plan-review.cases.v1",
    work_identity | Hex64,
    reviewer | Name,
    cases | Array Entry,
  },
  # Rules that span fields: bounded unique cases, an author who is not the
  # reviewer, bounded revisions with unique steps, unique annotations, and
  # commands that name an existing revision.
  Consistent =
    std.contract.from_predicate (fun value =>
      std.array.length value.cases >= 1
      && std.array.length value.cases <= MaxCases
      && distinct (std.array.map (fun entry => entry.id) value.cases)
      && std.array.all
        (fun entry =>
          let count = std.array.length entry.revisions in
          entry.author != value.reviewer
          && count <= MaxRevisions
          && std.array.all
            (fun steps =>
              std.array.length steps >= 1
              && std.array.length steps <= MaxSteps
              && distinct (std.array.map (fun step => step.id) steps)
            )
            entry.revisions
          && distinct (std.array.map (fun annotation => annotation.id) entry.annotations)
          && in_range count entry.annotations
          && in_range count entry.resolutions
          && in_range count entry.decisions
        )
        value.cases
    ),
  # Synthetic plans in the style of an agent's plan for Kiln stack
  # publication, plus a captured case without plan bytes and a plan that
  # never reached review.
  example = {
    schema = "kiln.plan-review.cases.v1",
    work_identity = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    reviewer = "reviewer",
    cases = [
      {
        id = "overwrite-branch",
        provenance = 'synthetic,
        author = "agent",
        revisions = [
          [
            { id = "types", text = "Add a stack plan type that binds every layer to its verified candidate." },
            { id = "push", text = "Force-push each layer branch so that a re-run replaces stale heads." },
            { id = "stack", text = "Create the GitHub stack after every pull request exists." },
          ],
          [
            { id = "types", text = "Add a stack plan type that binds every layer to its verified candidate." },
            { id = "push", text = "Push each layer branch only when it is absent, and refuse a branch whose head differs." },
            { id = "stack", text = "Create the GitHub stack after every pull request exists." },
          ],
        ],
        annotations = [
          {
            id = "no-overwrite",
            revision = 1,
            region = { step = "push" },
            note = { kind = 'replace, text = "Push each layer branch only when it is absent, and refuse a branch whose head differs." },
            blocking = true,
          },
        ],
        resolutions = [{ annotation = "no-overwrite", revision = 2 }],
        decisions = [
          { revision = 1, verdict = 'changes_requested },
          { revision = 2, verdict = 'accepted },
        ],
      },
      {
        id = "unverified-layers",
        provenance = 'synthetic,
        author = "agent",
        revisions = [
          [
            { id = "verify", text = "Verify only the top layer and reuse its result for the layers below." },
            { id = "merge", text = "Merge the stack from the bottom up through the API." },
          ],
        ],
        annotations = [
          {
            id = "layer-ci",
            revision = 1,
            region = { step = "verify" },
            note = { kind = 'comment, text = "Each lower layer needs its own canonical CI run." },
            blocking = true,
          },
          {
            id = "no-merge",
            revision = 1,
            region = { step = "merge" },
            note = { kind = 'delete },
            blocking = true,
          },
        ],
        resolutions = [],
        decisions = [{ revision = 1, verdict = 'changes_requested }],
      },
      {
        id = "comment-only",
        provenance = 'synthetic,
        author = "agent",
        revisions = [
          [
            { id = "route", text = "Add the route enum with the single pull request as the default." },
            { id = "origin", text = "Refuse Origin stacks before any effect." },
          ],
        ],
        annotations = [
          {
            id = "name-default",
            revision = 1,
            region = { step = "route", span = { start = 28, end = 47 } },
            note = { kind = 'comment, text = "Name this default in the guide as well." },
            blocking = false,
          },
        ],
        resolutions = [],
        decisions = [{ revision = 1, verdict = 'accepted }],
      },
      {
        id = "never-reviewed",
        provenance = 'synthetic,
        author = "agent",
        revisions = [[{ id = "pin", text = "Pin the plan by its content identity." }]],
        annotations = [],
        resolutions = [],
        decisions = [],
      },
      {
        id = "captured-agent-plan",
        provenance = 'agent_capture,
        author = "agent",
        revisions = [],
        annotations = [],
        resolutions = [],
        decisions = [],
      },
    ],
  },
}
