## Context

GPUI Kit `944803984cf0ba3111861d6e1aa9d6265c109ff9` (NAR `sha256-/w0H0zmgKPf+8aAFw3BOT8ifKl2xH+wdVKGEbSwXlRM=`, Apache-2.0) ships three things:

- Lucide icons under ISC at `crates/assets/assets/icons/*.svg`, with `crates/assets/LICENSE-LUCIDE`;
- a semantic theme schema at `.theme-schema.json`;
- normative Design Guides at `skills/gpui-kit-design-guides/references/design-guides.md`.

The live console has a token layer, shared components, `ui.js` (`KilnUi`), an icon rail, a page frame and per-page conventions. These come from the archived `modernize-live-console`, `restructure-live-console-layout`, `polish-console-pages` and `improve-ci-views` changes. The accepted `live-console-design` and `ci-views` requirements bind behaviour.

## Goals and Success Contract

All icons come from the pinned Lucide set, imported reproducibly and verified by hash. Every live-console stylesheet uses one semantic token vocabulary with no legacy aliases. Every page passes GPUI Kit's design review checklist, and documented web deviations stand in for desktop-only guidance.

False completion includes:

- icons copied by hand without a script or provenance;
- aliases left for old token names;
- icons fetched from a CDN;
- dropping visible focus or 44px touch targets;
- following desktop guidance where it would break web semantics, such as replacing navigation links with buttons;
- a changed dialog or button string that leaves a test or doc asserting the old text.

## Decisions

### Decision: Repo-owned icon import

**Choice:** Add `scripts/import-lucide-icons.sh`. It resolves the pinned GPUI Kit revision through `nix flake metadata` and refuses any revision or NAR mismatch, in the same style as `import-cairn-policy.sh`. It reads the reviewed name list `crates/kiln-web-service/src/icons.list`, checks that each source SVG exists, and generates `crates/kiln-web-service/src/icons.js`. That file is data only: `window.KilnIcons` maps each name to element descriptors, with no markup strings. The script also copies `LICENSE-LUCIDE` beside it and writes a provenance header with the revision, NAR hash and per-icon BLAKE3 values.

`/icons.js` loads before `ui.js`. `KilnUi.icon(name)` builds SVG elements from the data, and static markup uses `<span data-icon="name">` placeholders hydrated by `ui.js`. All existing hand-drawn SVGs are removed.

### Decision: Semantic tokens with Kiln values

**Choice:** Map current tokens onto GPUI Kit theme names, expressed as CSS custom properties with dots replaced by hyphens. The mapping table lives in `web.css` comments and the operator docs, and the old names are deleted everywhere.

| GPUI Kit token | CSS custom property |
| --- | --- |
| background | `--background` |
| foreground | `--foreground` |
| muted, muted.foreground | `--muted`, `--muted-foreground` |
| primary, primary.foreground | `--primary` (acid yellow), `--primary-foreground` |
| secondary | `--secondary` |
| accent | `--accent` |
| border | `--border` |
| input.border | `--input-border` |
| ring | `--ring` |
| danger, success, warning, info | `--danger`, `--success`, `--warning`, `--info`, each with `-background`/`-foreground` |
| popover | `--popover` |
| list.hover / list.active | `--list-hover` / `--list-active` |
| table.head | `--table-head` |
| sidebar | `--sidebar` |

Spacing, radius, motion and type scales keep their names; they are not theme colors.

### Decision: Design Guides with explicit web deviations

**Choice:** Each page is reviewed against the guide's design review and accessibility checklists, covering hierarchy, states, overlays, copy, density and data-heavy views. The guide is desktop-first, so these deviations are recorded:

- Navigation to console routes stays a link, which preserves open-in-new-tab and history. Commands are buttons.
- There are no OS menus or window chrome; the web shell supplies them.
- Mobile touch targets stay at 44px.

Confirmation copy names the object and the verb, for example `Cancel run "Kiln — Rust workspace tests"?` with a `Cancel run` button and a `Keep running` dismissal.

## Risks / Trade-offs

- Renaming tokens across six stylesheets risks missed references. Grep must find zero old names, and a browser review checks computed colours.
- Copy changes can break smoke scripts, tests and docs. Grep before changing, and update them in the same change.
- The repository dataset has under 3 GB free, so evidence is limited to viewport screenshots of key pages.

## Verification Strategy

- Run the import script twice: the second run must be byte-identical. A tampered revision must be refused.
- Check that no hand-drawn SVG paths remain in the console sources.
- Check that no old token names remain.
- Run a real-data browser review of every page at 320 and 1440px, in light and dark: zero errors, zero overflow, 44px targets, and icons rendered with no request outside the origin.
- Run the node tests, the repository gates, deployment and live verification.
