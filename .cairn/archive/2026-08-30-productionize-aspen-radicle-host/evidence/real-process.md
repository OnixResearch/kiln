# Real-process durable recovery fixture

Date: 2026-08-28

The fixture used the packaged Lattice application runtime at `/nix/store/fidjpdj5pg4zzxn8l3qpj8ai3fj5khbn-lattice-0.1.0/bin/lattice`, the real Kiln Aspen host binary, and the real native Kiln extension process.

The run performed these steps:

1. Imported the exact reviewed Lattice workflow.
2. Started Lattice and a fresh durable Aspen host process.
3. Submitted one accepted trigger and replayed it in the same process.
4. Stopped Lattice.
5. Restarted the host with no Lattice socket and replayed the exact request 16 times.
6. Restored an earlier full state-root backup and replayed the exact request again.
7. Truncated a copied SQLite ledger to eight bytes and proved startup failed before socket creation.

The first and restored acknowledgement files are byte-equal. Their BLAKE3 is `e48c770ed618a7e2d99bbd13497235993c3b20d9bb74f7e40170a754987d9f66`. The acknowledgement binds operation `96cc6bd2...`, run `kiln-aspen-run-f45e7126...`, decision `completed`, and receipt `a3b953e1...`.

All 16 no-Lattice replays were byte-equal to the first acknowledgement. Thus, process restart and bounded load did not send a second provider dispatch. The restored backup produced the same bytes. The corrupted copy exited with status `1` and reported `file is not a database`.

`evidence/real-process/state-hashes.txt` records BLAKE3 identities for the final marker, Redb journal, SQLite ledger, and callback values.

This fixture proves bounded behavior for one machine-local cohort. It does not prove remote persistence, storage-device durability, provider truth, distributed exactly-once execution, production availability, or release eligibility.
