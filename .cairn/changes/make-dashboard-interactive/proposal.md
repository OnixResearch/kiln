# Make the Kiln dashboard interactive

## Why

The local operator dashboard renders admitted state but cannot search runs or inspect another run or attempt without invoking the CLI again. Operators need a navigable offline page over the same bounded projections.

## What Changes

- Add client-side run search, outcome filtering, sorting, and run/attempt selection to the existing HTML format.
- Project drill-down views only for the bounded run list, its bounded attempt lists, active attempts, and explicit CLI selections.
- Embed trusted fixed JavaScript and CSS, escaped admitted data, and no external resources. Preserve deterministic generation, canonical view identities, redaction and non-claims.
- Retain watch-mode atomic replacement and reload cadence, preserving page selection and filters across reloads through the URL fragment.
- Replace the document-style layout with a responsive operator console: compact summary, adjacent run inspector, readable outcomes and accessible provenance disclosures.
- Add keyboard search/help, explicit theme selection and a repeatable offline preview workflow with representative admitted sample data and source rebuild instructions.

## Impact

- Affected specs: dashboard-html-surface, dashboard-live-html-report.
- Files: dashboard report projection, renderer, shell and tests; browser property specification; dashboard preview example and operator documentation.
- Compatibility: text output and observation ports are unchanged. HTML now contains inline script; the old script-free requirement is intentionally replaced.
- Current consumer and adoption: local Kiln operators use the existing `--format html --output` command.
- Maintenance owner: Kiln dashboard maintainers. Repeatability: deterministic crate checks and actual offline browser interaction.

## Out of Scope

No server, network reads, external resources, CI commands, provider effects, publication, deployment, or changes to canonical state and observation vocabulary.
