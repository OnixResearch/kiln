# Atlas view (planned, off by default)

This is a planning note for [issue 92](https://github.com/OnixResearch/kiln/issues/92).
No atlas view, route, profile option, document schema or asset exists. Nothing
here is implemented, measured or qualified. Building the view needs its own
spec-driven Cairn change, which may promote these rules into accepted
requirements.

## Related work: system-atlas

[system-atlas](https://github.com/inkboard/system-atlas) is an MIT-licensed
agent skill. Its build turns one JavaScript data module into two views: an
interactive isometric HTML map and a generated text twin, `SYSTEM.md`. Chapters
reveal a few structures at a time, an index beside the map pins parts, open
questions keep stable IDs and states, and packets moving along flow hops open a
panel with the route and a representative payload.

Issue 92 describes a recording of an atlas built with it, posted on X by
`GarshytHoel`. According to that description, the recording captions its packet
payloads as design-implied, not measured traffic. Kiln adopts that rule and
nothing else:

- The poster's claims are theirs, not Kiln evidence. That includes the atlas
  being more useful than documentation, the two-way design workflow and the
  67-pull-request stack.
- No recording frame, renderer, template, data module, palette, typeface or
  other upstream asset is copied, linked into the console or committed. The
  upstream page loads an external font stylesheet and its build executes the
  data module. The console serves same-origin assets under a content security
  policy, and Kiln keeps human-authored inputs in typed Nickel.
- It does not replace the bounded 2D dependency graph in
  [rats issue 24](https://github.com/brittonr/rats/issues/24) or restate the
  trace and state-graph visual in
  [chaoscontrol issue 50](https://github.com/brittonr/chaoscontrol/issues/50).

## The atlas document

A later view renders exactly one caller-supplied, inert document. Human-authored
atlases use typed Nickel exported to deterministic JSON; a producer may instead
derive the document from topology Kiln already retains. A BLAKE3 identity over
the exported bytes names the document, and every view states it.

| Part | Content | Admission |
| --- | --- | --- |
| Nodes | Stable ID, label, role, board position and an optional link to an existing console route | IDs are unique; a link only navigates |
| Routes | Directed pair of node IDs and a label | Both ends exist |
| Chapters (optional) | Ordered list; each names the nodes and routes it reveals | Only known IDs; every node and route is revealed by some chapter |
| Open questions (optional) | Stable ID, subject, text and state: open, answered with text and date, or routed to a named next step | An answer is an edit to the document |
| Packets (optional) | Ordered hops over declared routes; each hop has one payload and its provenance | A hop off the declared routes is refused |

A malformed or over-bound document is refused with a named reason. It is never
truncated, repaired or partly drawn. Board positions are presentation hints, not
topology facts.

## Declared and observed payloads

Every payload has exactly one provenance:

- **Declared.** The atlas author wrote it to show what the design implies. Each
  declared payload, and every packet containing one, carries the qualifier
  *Representative payload: design-implied, not measured traffic* in the
  inspector, the prose twin and any export.
- **Observed.** It is bound to one retained Kiln observation: an exact run,
  attempt, node or receipt identity and the BLAKE3 digest of the retained
  bytes. The inspector shows that identity and only bounded content under the
  existing retention and redaction rules. Unavailable or expired evidence reads
  as unavailable, never as declared or empty.

No view upgrades a declared payload to observed, infers provenance from a
matching shape or invents timing. Stepping or playing a packet replays its
recorded hop order. It executes nothing and contacts no provider.

## Navigation and the prose twin

The board, chapters, pin list, packet inspector and prose twin are views of one
document identity:

- Chapters reveal the board progressively and cannot add or permanently hide
  topology. Unrevealed nodes stay listed, dimmed, with their chapter.
- The pin list is a lettered directory of every node. Pinning isolates a subset
  for reading without changing data. It is also the keyboard and screen-reader
  path through the atlas.
- The prose twin is generated deterministically from the document and names the
  document identity. Nobody edits it by hand; a differing identity marks it
  stale.
- Open questions stay visible until the document answers or routes them.

## Rules for a later implementation

- **Off by default.** Without explicit operator opt-in there is no route,
  navigation entry or API, and the console is unchanged.
- **Bounded.** Admission enforces named limits, initially at most 64 nodes, 128
  routes, 16 chapters, 64 open questions, 16 packets, 32 hops per packet and
  16 KiB of rendered payload per hop. The implementing change may tighten them
  with rendering evidence; exceeding a limit refuses the document.
- **One flat graph.** Narrow layouts, reduced motion, forced colors and readers
  who want a list fall back to the flat dependency graph from rats issue 24,
  adopted through the console port in
  [issue 68](https://github.com/OnixResearch/kiln/issues/68). No second
  dependency widget is added.
- **Console identity.** The view uses the console's existing identity and
  same-origin assets, recorded in the [console port inventory](console-port-inventory.md),
  and does not block or reorder issue 68.
- **Evidence and authority.** Exploration cases and capture evidence follow
  [issue 66](https://github.com/OnixResearch/kiln/issues/66). Declared topology
  is context, not evidence of runtime structure. No part of the atlas grants CI,
  review, verification, dispatch or merge authority.

## Candidate sources

Kiln already retains topology that could seed an atlas: a run's node graph, a
Lattice workflow plan, an implementation campaign's members and a Cairn change's
dependencies. Observed packets would cite retained run, attempt or receipt
evidence. Choosing a first source belongs to the later change.

## Non-goals

- Replacing rats issue 24 or restating chaoscontrol issue 50.
- A general architecture-drawing tool, a skill marketplace or an editor for
  arbitrary diagrams.
- Showing declared payloads as captured traces.
- Blocking the console port in issue 68.
