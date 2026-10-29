## Context

Kiln already admits six xyz.onixresearch COB types. Cairn owns their typed reducers; its installed CLI does not offer a complete six-type Git-store inventory. The live service already owns authentication, bounded libgit2 reads, separately admitted runtime policies and URL-restored application navigation.

## Decisions

### Decision: Observe native data without replacing producer semantics

**Choice:** Reuse the existing pinned libgit2 cohort and INITIAL_CAIRN_TYPES registry for bounded local ref discovery and operation inspection. Support plain refs/cobs and namespaced refs/namespaces/*/refs/cobs. Validate object/manifest/type bindings and expose every observed ref tip, operation parent, commit message and bounded JSON action. Do not select a current semantic winner, infer lifecycle transitions or verify signatures implicitly.

**Rationale:** A useful history/content view does not require a second Cairn reducer or an unreviewed producer import. Recorded action JSON remains explicitly raw observation; signed-ref and actor trust are not claimed.

### Decision: Separate read-only runtime policy and HTTP shell

**Choice:** Add optional --cobs JSON exported from typed config/web-cobs.ncl. Explicit repository entries contain a stable local key, label, native repository ID and absolute Git directory. Named bounds cap repositories, refs, objects, parents, operations, blobs and responses. Authenticated viewer/operator GET /api/cobs supplies inventory/filter/paging; GET /api/cobs/object selects repository/type/object exactly. Worker access, writes, arbitrary paths and unknown query parameters fail before reads. Read outside the CI engine mutex and do not persist canonical state.

**Rationale:** COB browsing must neither change the original ServiceProfile identity nor stall execution reconciliation. Missing/unavailable stores and bound exhaustion remain explicit, never false empty success. Cursor identity binds selected filters and observed object/ref tips; moved refs invalidate paging.

### Decision: Reuse the live application shell

**Choice:** Add one Onix COBs navigation entry. Reuse existing safe text rendering, request generations, session cancellation, responsive cards and URL state. Expose all six types, repository counts, explicit empty/error states and an object inspector with raw operation/action content and identity facts. Do not auto-select a different object when a link is stale.

**Rationale:** This is one view in the existing product, not a second dashboard or mutation console.

## Risks / Trade-offs

Raw history is not producer-reduced lifecycle state. Unverified signatures and multiple ref tips remain visible without a semantic conflict verdict. Empty local stores do not prove absence on remote peers. Private repository paths never enter responses. No ambient repository discovery, credentials or network effects are admitted. Local Git fixtures and actual browser/API readback establish only this bounded view; broader retained release holds remain unchanged.
