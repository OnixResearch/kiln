## ADDED Requirements

### Requirement: Grammar-based highlighting core

r[kiln.highlight.core]
`kiln-highlight` MUST derive per-line token spans from pinned Tree-sitter grammars without I/O, clocks or environment access. Spans MUST be UTF-16 offsets that are sorted, non-overlapping, within line bounds and drawn from a fixed class vocabulary. Inputs MUST be bounded by named byte, line, token and diff-file limits.

#### Scenario: Non-ASCII offsets

r[kiln.highlight.core.scenario.offsets]
- GIVEN source text containing multi-byte and astral characters before a string or keyword
- WHEN it is highlighted
- THEN each span MUST address exactly the characters of its token in UTF-16 code units

#### Scenario: Explicit skips

r[kiln.highlight.core.scenario.skipped]
- GIVEN an unsupported language, binary content, input over a bound, or a parser failure
- WHEN highlighting is requested
- THEN the result MUST report skipped with that reason and no tokens, never an error response or partial tokens beyond a bound

### Requirement: Opt-in highlighting API

r[kiln.highlight.api]
Source file and diff reads and the code-review change detail MUST add a `kiln.highlight.v1` object only when `highlight=1` is requested. Without the parameter, response bytes MUST be unchanged. Any other value MUST be rejected as a malformed query. Highlighting MUST NOT alter returned text, truncation or bounds.

### Requirement: Rendered server highlighting

r[kiln.highlight.render]
The Source file view, the Source Changes view and the code-review diff MUST render server tokens as inert spans whose concatenated text equals the returned text. Malformed or skipped highlighting MUST fall back to plain text with an explicit note.
