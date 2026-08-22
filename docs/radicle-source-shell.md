# Radicle source shell

Kiln can resolve and acquire one exact local Radicle source snapshot through `RadicleSourceShell`.

The shell consumes the contract published at revision `0a5924dd2092243a0fe5df94d43008f0402e46f6`.

## Profile

`config/radicle-source-profile.ncl` defines the source-only profile.

Each deployment must replace the fixture values with these exact local facts:

- absolute bare repository path;
- Radicle RID;
- signer node identity;
- branch reference;
- expected signed-refs commit;
- expected source revision; and
- entry, path, file, and total byte bounds.

The profile denies network, credential, signing, mutation, and collaboration authority.

Export the profile with Nickel:

```console
nickel export config/radicle-source-profile.ncl --format json
```

## Verification order

The shell performs these steps:

1. Admit the typed profile and contract request.
2. Open the configured local repository.
3. Reject partial clones, replacement objects, alternates, and unsupported object formats.
4. Read the exact namespaced `refs/rad/sigrefs` commit.
5. Parse and canonicalize the bounded `refs` blob.
6. Verify the signature with the declared node key.
7. Resolve `refs/rad/root` and compare its identity document with the RID.
8. Require parent-level signed-reference linkage.
9. Compare the signed branch target with the namespaced branch target.
10. Acquire only the exact requested revision.
11. Build the canonical Git snapshot manifest under all bounds.
12. Read the signed-refs reference again and reject movement.
13. Admit the final contract receipt.

The returned result preserves both the provider-neutral `SourceResolution` and the signed-source receipt.

## Failure and recovery

This adapter performs local reads only. It does not produce an ambiguous external write outcome.

A missing object, invalid signature, stale signed-refs commit, moved reference, identity mismatch, malformed tree, or exceeded bound fails closed.

After a reference moves, the caller must form a new request with the new exact signed-refs and revision identities. Kiln does not silently retry a stale request.

Diagnostics do not include repository paths, object content, signatures, or provider error text.

## Capability separation

The source profile cannot dispatch a workflow, publish status, or mutate a patch.

The trigger, status, and patch-review profiles cannot use the source profile's repository authority.

## Non-claims

Passing source checks does not prove signer trust, global freshness, complete history, peer convergence, filesystem-path stability, remote availability, equal history, build correctness, or release readiness.
