## Context

The Source reads live in `crates/kiln-web-service/src/sources.rs`: `file` and `diff` return text that already has explicit bounds and truncation. The code-review change detail calls `provider.compare(...)` in `code_review.rs` and returns its diff to `code-review.js`. The Source file viewer (`source.js`) uses a client tokenizer, `KilnSource.tokenizeLines`/`highlightPlan`, with token classes `tok-comment|string|number|keyword|heading` and CSS tokens `--syntax-*` in `web.css`. Workspace lints forbid `unsafe_code` in Kiln crates; external crates may contain it.

## Goals and Success Contract

Every code view (Source file, Source Changes, code-review diff) highlights the same text identically, using grammars from one pinned core.

Highlighting must never:
- change the returned text, its truncation or its bounds;
- run without the `highlight=1` opt-in;
- exceed a deterministic size bound;
- turn a parser failure into an error response.

Unsupported, oversized, binary and failed inputs return an explicit skipped status with a reason.

False completion includes:
- tokens that address the wrong characters, such as byte rather than UTF-16 offsets on non-ASCII text;
- tokens that overlap or fall outside the line;
- a client that still uses its own tokenizer;
- enabling Tree-sitter's WASM feature;
- unpinned grammar versions.

## Decisions

### Decision: Pure `kiln-highlight` core

**Choice:** a new crate `crates/kiln-highlight` that performs no I/O, reads no clock and touches no environment.

API:
- `language_for_path(path) -> Option<Language>`
- `highlight(language, text, bounds) -> Highlight`
- `highlight_diff(diff_text, bounds) -> Highlight`

Diff highlighting splits the unified diff per file and detects each file's language from its `+++`/`---` path. It reconstructs each side's hunk lines and highlights the old and new sides separately, as context-free fragments. Tokens are then mapped back to diff line indices with offsets shifted by the one-character prefix. Header lines have no tokens.

`Highlight { schema: "kiln.highlight.v1", language, status: highlighted|skipped, reason, lines: Vec<Vec<(u32 start, u32 end, Class)>> }` serializes to JSON, with each token as `[start, end, "class"]`.

Bounds are named constants:
- maximum bytes: 512 KiB;
- maximum lines: 20,000;
- maximum tokens per line;
- maximum diff files.

A single `HighlightConfiguration` per language is built once and reused. Grammar highlight queries come from the grammar crates, with Kiln-maintained query files only where a crate ships none.

### Decision: Fixed class vocabulary

**Choice:** Tree-sitter capture names map by prefix to these classes:

| Class | Class | Class | Class |
| --- | --- | --- | --- |
| keyword | string | comment | number |
| constant | type | function | macro |
| attribute | property | variable | operator |
| punctuation | tag | escape | heading |
| link | | | |

Captures that do not map are not emitted. Nested captures resolve to the innermost one, so spans never overlap.

### Decision: Opt-in API extension

**Choice:** `GET /api/sources/{id}/file?…&highlight=1`, `GET /api/sources/{id}/diff?…&highlight=1` and the code-review change detail with `highlight=1` each add a `highlight` field. File responses carry file lines; diff responses carry diff-text lines. Without the parameter the response bytes are unchanged, and any other value of `highlight` is rejected as a malformed query. Highlighting runs after the existing bounded read in the shell and never alters the other fields.

### Decision: Client renders server tokens only

**Choice:** `source.js` and `code-review.js` request `highlight=1` and render spans by slicing each line with UTF-16 offsets into `tok-<class>` spans through `textContent`. Malformed token data falls back to plain text and records an explicit note. The client tokenizer and its tests are removed and replaced with rendering tests. `web.css` defines `--syntax-<class>` tokens for light and dark that meet AA contrast on the code surface.

## Risks / Trade-offs

- The grammar crates compile C code, so builds need a C compiler. That is available in the Nix stdenv and dev shell.
- Highlighting diff fragments without surrounding context can misparse, for example inside an unterminated block comment. This is cosmetic only.
- Binary size grows with grammars; the growth is measured and recorded.
- Licensing: tree-sitter and the grammars are MIT and are recorded in `LICENSES.md`.

## Verification Strategy

- **`kiln-highlight` unit tests**, with positive and negative cases:
  - each language;
  - UTF-16 offsets on non-ASCII text;
  - non-overlap and in-bounds tokens;
  - the size bounds;
  - unsupported and binary inputs;
  - diff mapping, including renames and `/dev/null` sides.
- **Web-service tests:** opt-in, malformed-parameter rejection and byte-identical responses without the opt-in.
- **Console:** node tests for token rendering and fallback.
- **Real-data browser review:** Source file (Rust, JS, Nix, Markdown), Changes, and a code-review diff harness, at 320 and 1440px in light and dark.
- **Release:** full repository gates, deployment and live verification.
