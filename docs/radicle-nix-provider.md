# Hosted Radicle Nix provider

`kiln-radicle-nix-provider` runs one bounded Nix check for one admitted Radicle repository.

The provider accepts these explicit values:

- a JSON profile from `config/radicle-nix-provider.ncl`;
- the exact Radicle repository ID;
- the exact lowercase Git revision.

The profile binds one user ID, source root, wrapper path, wrapper BLAKE3 identity, work directory, report root, namespace, URL, and all limits. Unknown profile fields fail admission.

The child process receives a cleared environment. The process gets only the fixed `nix flake check --no-update-lock-file` operation and a `git+file` source URI. The runtime limits stdin, stdout, stderr, time, polling, and teardown. It terminates the full process group when the limit policy requires teardown.

The provider writes one bounded log and one JSON report. It uses capability-relative, no-replace publication with required directory synchronization. A repeated run accepts an existing file only when its bytes are equal.

The report identity binds the schema, repository, revision, wrapper identity, and report namespace. A process failure, timeout, output flood, or uncertain teardown never becomes success.

## Authority boundary

The source root must be an exact, read-only Seaglass repository view. Do not give this process `/var/lib/radicle`, signing keys, node sockets, policy state, credentials, unrelated repositories, or network authority.

The provider reports bounded local observations. It does not prove source trust, sandboxing, hermeticity, CI correctness, external report delivery, distributed exactly-once execution, or release eligibility.
