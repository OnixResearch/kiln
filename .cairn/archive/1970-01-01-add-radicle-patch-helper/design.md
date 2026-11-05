## Context

The upstream `radicle` crate exposes the patch reducer, but it links `sqlite`
while Kiln links `rusqlite`; Cargo rejects two crates linking the same native
`sqlite3`.

## Decision

Run the upstream reducer in a separate helper workspace and invoke it from the
adapter shell. The helper prints one JSON object. The adapter bounds stdout,
requires a clean stderr and a zero exit, and parses the object.

This is read-only. Mutation remains a separate design.

## Risks

- The helper is an external process dependency; its path must be operator
  configured.
- No mutation or review append is implemented.
