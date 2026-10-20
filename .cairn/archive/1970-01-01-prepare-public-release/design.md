# Design: public release preparation

## Context

The public source route and local standalone demo work. Direct and transitive SSH dependencies prevent anonymous reproduction. The accepted-spec traceability scope excludes an unfinished Onix requirement referenced by a stock rejection test. One fixed Pijul derivation produced a binary different from its recorded BLAKE3. The existing publication receipt and release input intentionally identify an earlier revision.

## Success Contract

An outside contributor can acquire all exact source inputs without a secret or sibling checkout. Maintained checks exercise the unchanged stock rejection boundary and an independently investigated Pijul cohort. Notices identify GPL/AGPL components without implying a transferable owner-only exception. Current evidence names its actual source inputs and does not reinterpret historical PASS declarations or authorize publication.

## Decisions

### Decision: Preserve immutable upstream source and provenance

**Choice:** Prefer an authorized anonymous immutable route for the existing revision. When no such route exists, use the repository's verified vendor-import convention with upstream revision, canonical archive BLAKE3, license files, and drift refusal. Include transitive source inputs and fixture producers; regenerate lock files with their owning tools.

**Rationale:** Replacing SSH with an inaccessible HTTPS URL is not a public-build fix. Mutable siblings and unreviewed source upgrades would change the contract.

### Decision: Diagnose cohort drift instead of accepting a new observed hash

**Choice:** Inspect source/toolchain/recipe inputs and build variation, eliminate identified nondeterminism, and admit one measured cohort only after independent reproduction. Preserve exact executable and library-cohort checks. Map the stock rejection annotation to its accepted requirement.

**Rationale:** A passing weakened check or unexplained digest update would hide the reported failure.

### Decision: Scope licenses and release claims

**Choice:** Maintain package-specific license notices and applicable upstream license texts. Keep the operator-authorized publication receipt unchanged; label it historical in current documentation. Record local verification separately and bind it to exact source inputs, with publication authority false unless separately granted and performed.

**Rationale:** Local preparation neither publishes a candidate nor extends an owner-only reuse grant to downstream recipients.

### Decision: Freeze source separately from candidate evidence

**Choice:** Select completed adapter and public-release changes in a separate local Jujutsu workspace, leaving unfinished Onix evaluator work untouched in the operator workspace. Freeze one exact revision and deterministic source archive, then exercise that archive without sibling repositories or operator credentials. Record the candidate's license map, lifecycle checks, unchanged hosting profile, and rollback identity in a later evidence-only revision so the source cannot claim to contain its own finalized receipt.

**Rationale:** A passing check on a dirty workspace or historical payload does not establish a new immutable candidate. Selection must not publish unfinished work or rewrite the historical publication receipt. Local bookmarks and checks grant no signing, push, synchronization, or publication authority.

## Functional Core and Imperative Shell

No provider decisions or domain transitions change. Source acquisition, builds, checks, and evidence capture remain imperative repository tooling. Existing pure cores remain independent of provider runtimes.

## Risks / Trade-offs

- Vendoring increases repository size; exact source records and guarded imports retain an auditable update path.
- Binary drift can reflect nondeterminism or an untrusted artifact; never select a replacement digest solely because it makes a test pass.
- A source revision cannot truthfully include its own finalized receipt; distinguish validated source payload from subsequently recorded local evidence and do not claim a new canonical publication.
- Quota exhaustion blocked the original-checkout application during preparation; retain isolated evidence and apply only after checking original bytes against the preserved baseline.

## Non-Claims

Local checks do not prove source trust, external provider correctness, universal reproducibility, downstream legal advice, publication, or release authority.
