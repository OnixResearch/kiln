## Context

Existing COB browsing is read-only native history. The live HTTPS shell already authenticates viewer/operator/worker roles, enforces same-origin writes and retains source-bound CI observations. Existing signed Git COB storage is pinned in vendor/radicle-cob-git-store.

## Decisions

### Decision: Separate work application and authority

Add optional --work runtime policy with one explicit local Git store and private signer seed, bounded principals and write admission. Existing --cobs remains read-only. No inferred production path or default signing key. Attribute application commands to authenticated principals; the service signature is a configured application attestation, not the users Radicle signature. Personal records enforce principal ownership on reads and writes.

### Decision: Pure model, native history

Pure explicit command admission/reduction owns typed work fields and relations, discussion, planning, filters and attention. The imperative work shell owns Git, signing, clocks and HTTP. Reuse pinned native signed COB traits, namespaced refs, exact-head compare-and-swap and bounded verified replay; reject stale, malformed, unsupported and mismatched replays before effects. New work COB schemas never write existing Cairn or native issue/patch objects. CI outcomes remain read-only joined observations.

### Decision: Browser composition

Integrate durable work views, personal inbox, triage, planning and change detail into the existing navigation and URL restoration. Extend existing authenticated bounded transport, session erasure and inert text patterns. Commands have explicit failure/uncertainty; bulk edits are per-object exact-head results, not claimed atomic transactions. Keyboard navigation, peek and palette retain access semantics and destructive-action confirmation.

### Decision: Independent owners

Model owner owns work/model.rs and pure behavior tests. Store/API owner owns work.rs and work/store.rs plus store integration tests. Work UI owner owns work.js/work.css. Parent owns shared HTML/main/server/manifests, workspace adapter bridge, lifecycle and end-to-end proof. Each owner skips build/lint/test/format commands mid-flight; parent runs integrated checks after all edits.

## Risks / Trade-offs

A configured service signer attests the admitted application principal; it does not prove an end-user signature. Private application views in a shared Git store are access-controlled, not encrypted. Remote concurrent branches must fail explicitly rather than choose arbitrary state. Graph identity links do not establish acceptance or successful CI. No live installed store is mutated.

## Verification Strategy

Validate proposal/design/tasks before implementation. Exercise actual signed creation/update/restart/readback in disposable Git stores; paired tests cover valid/invalid fields and relations, privacy, stale heads, exact/mismatched replays, unsupported actions, corrupted history and provider failure. Exercise authenticated desktop/mobile browser work capture, planning, discussion, triage, personal attention, durable views, palette/keyboard/peek/bulk and linked change details. Run required Cargo/lint/Nix/lifecycle gates and retain bounded receipts with explicit non-claims.
