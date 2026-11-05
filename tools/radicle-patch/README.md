# Radicle patch reducer helper

Read-only helper around the upstream `radicle` 0.25.1 patch store. It is a
separate workspace because the upstream crate links `sqlite3` through
`sqlite`, while Kiln links it through `rusqlite`.

```console
cargo run -- STORAGE_ROOT REPOSITORY_ID PATCH_ID
```

The helper prints one JSON object with the reduced patch state, author and
revision count. It performs no mutation and grants no Kiln authority.

`indexmap` is built with its `std` feature here: the upstream crate disables
that feature but still uses the default-hasher two-argument `IndexMap` form.
