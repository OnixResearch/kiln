## Context

- `kiln-radicle-source-contract` defines `SourceRequest`, `SignedRefsObservation`,
  `SnapshotEvidence` and `SourceReceipt`, all with `deny_unknown_fields`. The shell
  verifies one pinned signer's namespaced `refs/rad/sigrefs`, requires the signed
  branch and the namespaced branch to equal the requested revision, and rechecks
  after acquisition.
- The vendored `heartwood-canonical` crate provides `Rules::from_raw`, `Rules::canonical`,
  `Canonical::find_objects` and `Canonical::quorum` from heartwood
  `71a91042d96bb994c4cd4311d85f5cd1a6311b4e`, plus the DID, `Delegates` and
  `Threshold` types the rules resolve against.
- Radicle identity documents are canonical JSON with `delegates` (did:key),
  `threshold` and payloads; `xyz.radicle.crefs` holds per-pattern `allow` and
  `threshold` rules. A delegate's signed refs commit embeds the identity document
  revision under `embeds/radicle.json`.

## Goals and Success Contract

A canonical-mode request is accepted only when the requested revision is the
heartwood-computed canonical head for the configured reference under the pinned
identity document's rules and delegate threshold. The receipt names the identity
revision and root, the threshold, the delegate DIDs, the canonical object and each
observing delegate's verified object. A single vote below threshold, and
diverging tips, are refused with identifiable codes. Requests and receipts without
`canonical` are byte-identical to before.

## Decisions

### Decision: additive optional fields

**Choice:** `SourceRequest.canonical: Option<CanonicalRequest>` and
`SourceReceipt.canonical: Option<CanonicalEvidence>`, both `skip_serializing_if`
absent. `RadicleSourceSelection.canonical` mirrors the request for selection-time
admission.

**Rationale:** every existing profile, fixture and journal keeps its exact bytes,
and `deny_unknown_fields` stays meaningful. Admission links evidence to the request
and rejects evidence without a canonical request and vice versa.

### Decision: the identity document revision is operator-pinned

**Choice:** the request names `identity_commit`; the shell reads
`embeds/radicle.json` from that commit and requires every observing delegate's
signed refs to embed identical bytes. It does not verify the identity document's
own revision history or its acceptance by the delegate threshold.

**Rationale:** identity history verification needs the identity COB reducer and its
storage cohort, which the SQLite-free source shell deliberately excludes. The
receipt binds the run to the exact revision used, and the non-claim states the
limit. A reviewed identity revision is governance input.

### Decision: verify every observing delegate with the existing checks

**Choice:** for each delegate in the pinned document, look up its
`refs/namespaces/<key>/refs/rad/sigrefs`, verify the signature over the canonical
refs blob, require the RID match, the declared parent to equal the commit parent,
identical embedded identity bytes, and the namespaced branch target to equal the
signed value. Delegates that published no sigrefs or omitted the reference have
no vote rather than causing a failure.

**Rationale:** quorum is only meaningful over verified votes; absence is a vote
that does not count, and heartwood's computation already ignores missing
references.

### Decision: heartwood computes the quorum

**Choice:** build `RawRules` from `xyz.radicle.crefs`, synthesize a rule from the
document's threshold when no rule matches the configured reference, resolve
delegates through `Rules::from_raw`, and call `rules.canonical(refname, repo)`,
`find_objects`, `quorum`. Refuse when no rule matches, objects are missing, the
quorum fails, or the quorum object is not the requested revision.

**Rationale:** acceptance criterion 4 asks for parity with heartwood; using its
implementation removes the divergence risk of a reimplementation. The synthesized
rule mirrors the identity document's default rule.

### Decision: recompute after acquisition

**Choice:** canonical evidence is computed before snapshot acquisition and again
after the post-acquisition signed-refs revalidation; a changed quorum or
observation set fails the acquisition.

**Rationale:** refs can move during acquisition; the receipt must describe the
state that was actually read twice.

## Risks / Trade-offs

- Identity document history is not independently verified; a stale or wrong
  pinned revision is an operator error the receipt only records.
- The synthesized default rule uses the document threshold and all delegates; a
  repository whose intended rule differs must declare it in `xyz.radicle.crefs`.
- Only branch references (`refs/heads/…`) are admitted by the contract, so
  canonical tags are out of scope here (issue #21).
- Diverging commits can fail as either a quorum or an object-resolution error
  depending on heartwood's path; both refuse the run.
