## Context

Kiln already presents topology in flat forms: a run's node graph, Lattice
workflow plans, implementation campaign members and the Cairn changes
dependency graph. None is an atlas, and none labels a payload as declared or
observed. The console port (issue 68) plans a reusable flat dependency graph
from rats issue 24. Issue 66 owns exploration case IDs and comparable capture
evidence.

[system-atlas](https://github.com/inkboard/system-atlas) is an MIT-licensed
agent skill whose build turns one JavaScript data module into an interactive
isometric HTML map and a generated `SYSTEM.md`. Its page loads an external font
stylesheet. Issue 92 describes a recording posted by `GarshytHoel` whose caption
marks packet payloads as design-implied, not measured traffic.

## Decisions

### Decision: write the note before any implementation

**Choice:** a planning document in `docs/` with change-local acceptance IDs,
not accepted `r[...]` requirements.

**Rationale:** issue 92 asks for the note first. Accepted requirements without
an implementation would only add untraceable obligations; a later
spec-driven change can promote the note's rules when it builds the view.

### Decision: one inert, typed atlas document

**Choice:** the view renders one caller-supplied document with nodes, routes,
optional chapters, open questions and packets. Human-authored atlases use typed
Nickel exported to deterministic JSON, named by a BLAKE3 identity. A malformed
or over-bound document is refused, never truncated or partly drawn.

**Rationale:** Kiln keeps human-authored inputs in typed Nickel, and the
service's content security policy admits only same-origin scripts and styles.
An executable data module or an external stylesheet would add authority the
view does not need.

### Decision: provenance on every payload

**Choice:** each hop payload is either declared (design-implied) or observed
(bound to an exact retained run, attempt, node or receipt identity and content
digest). Declared payloads carry a fixed visible qualifier in every view and
export; unavailable evidence reads as unavailable.

**Rationale:** the caption reported in issue 92 shows how easily representative
shapes read as traffic. The label is the safety rule, so it is decided before
any drawing exists.

### Decision: navigation and prose are views of the same document

**Choice:** chapters, the pin list, the packet inspector and the prose twin
derive from the same document identity. Chapters cannot add or permanently
hide topology, and the twin is generated, never edited.

**Rationale:** a second source of truth would drift; identity comparison
detects a stale twin.

### Decision: off by default, bounded, with the shared flat fallback

**Choice:** a later implementation adds no route, navigation entry or API
without explicit operator opt-in, enforces named limits, and uses rats issue
24's graph for flat fallback. It must not block issue 68.

**Rationale:** the console stays unchanged for operators who do not ask for
the view, and a second dependency widget would duplicate the port's work.

## Risks / Trade-offs

- The initial limits are proposals; the implementing change may tighten them
  with rendering evidence.
- Component choices in issue 68 may change how the view is built; the note
  fixes the data and labeling rules, not the component tree.
- Declared topology can still be wrong. The note marks it as context, not
  evidence, and grants it no authority.

## Verification Strategy

Check that relative links resolve, that no media or upstream asset is added,
that the documentation index links the note, and that the Cairn proposal,
design and task gates and validation pass. The note is prose; no test repeats
its text.
