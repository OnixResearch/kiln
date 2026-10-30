## Why

The operator asked to add syntax highlighting and chose server-side Tree-sitter. Today only the Source file viewer is highlighted, by a small client tokenizer that covers just comments, strings, numbers and keywords in 12 languages. The Source Changes diffs and the code-review diffs are not highlighted at all. Grammar-based highlighting is more accurate and gives the same result in every view.

## What Changes

- Add `kiln-highlight`, a pure crate. Given a path or language and a text, it returns bounded per-line token spans. It uses `tree-sitter-highlight` with pinned grammar crates (MIT) for these languages:
  - Rust
  - JavaScript and TypeScript
  - Python
  - Nix and Nickel
  - TOML, JSON and YAML
  - CSS and HTML
  - Bash
  - Markdown
  - C and Go

  Captures map onto a fixed set of token classes, and spans are offsets in UTF-16 code units.
- Add an opt-in `highlight=1` parameter to the Source file and diff reads and to the code-review change detail diff, returning a `kiln.highlight.v1` object. Responses without the parameter are unchanged.
- The console renders server tokens in the file viewer, the Source Changes view and the code-review diff, with Kiln-palette light and dark token colours. The client tokenizer is removed.

## Impact

- **Consumers:** authenticated Kiln developers and reviewers.
- **Owners:** Kiln web-service maintainers own the shell integration; Kiln maintainers own `kiln-highlight`.
- **Durable contribution:** one highlighting core shared by every code view, built on maintained grammars.
- **Affected:**
  - the workspace manifests and `Cargo.lock`;
  - the web-service source and code-review read paths;
  - the console assets;
  - `LICENSES.md`;
  - tests and operator documentation.

## Out of Scope

- No change to source authority, bounds or text: highlighting never alters the returned text.
- No client-side grammar or WebAssembly.
- No highlighting of CI logs.
- No language beyond the pinned set; other languages render plain with an explicit reason.
- No Tree-sitter WebAssembly runtime feature.
