# Radicle patch reducer helper

Read-only helper around the upstream `radicle` 0.25.1 patch store. It is a
separate workspace because the upstream crate links `sqlite3` through
`sqlite`, while Kiln links it through `rusqlite`.

```console
cargo run -- read STORAGE_ROOT REPOSITORY_ID PATCH_ID
RADICLE_SIGNER_SEED=<64 hex chars> cargo run -- review STORAGE_ROOT REPOSITORY_ID PATCH_ID REVISION_ID accept|reject SUMMARY
```

The `read` command prints one JSON object with the reduced patch state, author
and revision count. The `review` command appends one review through the upstream
authorization path and prints the review id. The signer seed is read from the
child environment, never argv. Neither command grants Kiln review or release
authority.

`indexmap` is built with its `std` feature here: the upstream crate disables
that feature but still uses the default-hasher two-argument `IndexMap` form.
