## Context

`kiln_core::projection::resolve` returns an `AdmittedPartialView` holding the complete trusted manifest: every destination, native object and origin. `implementation::authority` owns the protected implementation operations, the `ProtectedAuthority` port and the local-operator profile; its resources bind work and attempt, which a human browsing source does not have. Basalt's Kiln adapter has no source operations yet (Basalt B2).

## Goals and Success Contract

A holder never receives the trusted manifest. What a holder sees depends only on what it may see: changes confined to forbidden entries leave the visible result equal, a missing and a forbidden object look the same, and a handle, cursor, cached view, alias, equal-byte file or known digest grants nothing. Narrowing or revoking authority invalidates bindings without changing canonical identities. A share is never broader than its sharer or its view's frozen ceiling.

## Decisions

### Decision: a separate closed source vocabulary

**Choice:** `SourceViewOperation` (eight operations) with a coverage map to `kiln/source/{discover,read,history,prepare,modify,create,delegate,result}`, an effect class and an attempt rule; submission and publication reuse `ProtectedOperation`.

**Rationale:** forcing source reads through `AuthorityRequest` would fabricate work and attempt identities. One-to-one abilities let Basalt B2 adopt the map unchanged.

### Decision: per-component path scope, one revision per session

**Choice:** `PathScope { components, subtree }`, compared component by component. A session's `EffectiveSourceScope` binds one exact revision; a live reference is followed only by a fresh admission whose observation fits the rule's `fast_forward` or `rewrite` evolution.

**Rationale:** byte-prefix coverage would let `a/b` cover `a/bc`. Binding one revision makes a refresh an explicit, revalidated admission rather than a silent widening.

### Decision: filter by origin before anything is shaped

**Choice:** the shell enumerates the source over the holder's `ScopedSelection`, the ceiling intersected with the paths its current grants let it discover or read (the root only for full source), and wraps the result in a `ScopedEnumeration` recording that selection. `restrict(enumeration, scope, ceiling, key, now)` refuses an enumeration of any other selection as stale, then walks manifest entries in order and keeps an entry only when its native origin lies in the ceiling and a current grant lets the holder discover or read it. Generated entries and dependency identities, which have no native path, are disclosed only to a full-source holder. Truncation, paging and coverage are computed over the kept entries only; a truncated holder-scoped enumeration reports truncated coverage and unknown lookups.

**Rationale:** a post-filter over an unrestricted result leaks through counts, ordering and errors; judging aliases by origin stops a layout edit from granting access.

### Decision: keyed handles over a view binding

**Choice:** `ViewBinding` hashes repository, revision, definition, manifest, ceiling, audience, effective scope and both generations; its identity keys caches and stale checks. Handles, the view reference and cursor tags are `blake3::Hasher::new_keyed` over a separate keying identity of holder-visible data (audience, effective scope, ceiling, definition and visible content) and the entry; the shell holds the key. Every page, lookup and resolution first checks the binding against the holder's current scope.

**Rationale:** an unkeyed hash of a known path or object would be forgeable. Binding scope and generations means a narrower holder, a revocation or a new generation can never reuse a broader cursor or cache.

### Decision: one public unavailable shape with an internal reason

**Choice:** `Unavailable { reason: Missing | Forbidden | Stale }` whose `public()` is always `{"error":"unavailable"}`. Because a view is built from the holder-scoped enumeration, the owner attributes a refused probe with `AudienceView::attribute`, server-side only, against a complete audit enumeration over the whole-ceiling selection; any other enumeration, such as another holder's narrower cached view, gets no attribution. For a holder whose grants cover the whole ceiling, such as full source, its own enumeration is that audit. A holder that may discover a path receives `Entries`, `Unrequested`, `Absent`, `Unknown` or `Truncated`.

**Rationale:** the owner keeps refusals attributable for audit while probes stay indistinguishable; distinct states only where discovery is authorized satisfy the Seqair section without leaking existence.

### Decision: Cairn context through path authority

**Choice:** a Cairn item is visible when its `.cairn/changes/<id>/...` path is discoverable under the view's ceiling and the holder's grant; text needs read. A hidden prerequisite of a visible item appears as an opaque handle whose status is `unavailable` unless the holder may discover the prerequisite's `.cairn/changes/<id>/` change, so `blocked` still reports it as an obligation. `RestrictedContext` has no `Deserialize` and no conversion into `CairnObservation`.

**Rationale:** owner decision O4 chose path authority over a separate `kiln://cairn/` family; the compile-fail examples keep a presentation from standing in for the complete lifecycle snapshot.

### Decision: shares are attenuated plans, issued elsewhere

**Choice:** `admit_share(parent, ceiling, request, now)` checks request shape, sharer expiry, lifetime, path containment in the sharer's grants and the ceiling, `delegate` over every path, strictly smaller depth, operations within ceiling and grants, and snapshot containment, in that order, and returns a `SharePlan` binding the frozen ceiling and sharer scope. Issuance is a UCAN shell effect (S5).

**Rationale:** requests name origin components only, so filters, tags and aliases cannot widen a child; binding the frozen ceiling keeps an expanded saved view from expanding existing shares.

### Decision: a capability set confirms, it never extends

**Choice:** `EffectiveScopeFacts` separates `admitted` grants (each admitted request's operation over its own path, or a grant re-admitted on its own) from the session's raw `capabilities`. The scope keeps only admitted grants, narrowed to the operations the capability set confirms over the whole path. Scopes of several admissions combine with `EffectiveSourceScope::join`. Both ports keep only the admitted request in a session.

**Rationale:** the Basalt B1/B2 review found that a source session's effective capability set can include leaf grants the delegation chain rule never checked for that resource. Treating that set as authority would disclose siblings that `verify_source` itself denies.

### Decision: local and fake ports only

**Choice:** `LocalSourceAuthority` admits only the selected operator over an explicit full-source grant and refuses delegation. Tests use a fake verified port that maps a verified subject to a recorded grant and runs the shared `check_request`.

**Rationale:** Basalt B2 has not landed; the fake exercises the port contract without inventing a verifier. Nothing persists authority state, so owner decision O2 is not touched.

## Risks and Trade-offs

- The restricted view is computed per request; caching is the caller's, keyed by the binding identity.
- Dependency names come from the holder's definition and are shown; their identities are not, except to full source.
- Truncation reflects only the holder-scoped enumeration, which a real shell must request from the source adapter over the scoped selection; Kiln checks that the enumeration claims that selection and holds nothing outside it, but cannot see entries an adapter silently dropped.
- No general timing side-channel guarantee is claimed.
