## Context

The current API is usable from examples, but reusable crates need stable standard trait impls, builders, time helpers, and realistic examples. Ergonomics should reduce downstream wrappers without weakening validation or leaking secrets.

## Goals / Non-Goals

**Goals:** add standard parsing/serde support, named builders, time helpers, and application-style examples.

**Non-Goals:** full CLI product, new token semantics, network fetchers, or remote DID resolution.

## Decisions

### 1. Implement standard traits only where validation remains enforced

**Choice:** Add `FromStr`, `TryFrom`, `Display`, `Serialize`, and `Deserialize` for public semantic wrappers when the implementation calls the same constructors as manual code.

**Rationale:** Downstream crates should not need local newtypes just to parse HTTP headers or JSON bodies.

**Alternative:** Leave parsing to constructors only. Rejected because it makes Rust ecosystem integration unnecessarily noisy.

**Implementation:** Do not implement secret-material serialization/debug traits unless the output is explicitly redacted or intentionally exported through a separate key-export API.

### 2. Prefer builders for multi-field workflows

**Choice:** Add builders for issue and verify request construction in this change. Authorization, resolver/proof-store, and invocation builders are scoped to their feature-specific changes once those APIs exist.

**Rationale:** Positional constructors with signer/audience/capabilities/proofs/time fields are easy to misread. Builders document intent and can name missing fields.

**Alternative:** Keep positional constructors and examples. Rejected for reusable library ergonomics.

**Implementation:** Keep existing constructors during migration; builders become the README path once stable.

### 3. Keep time helpers shell-only

**Choice:** Add `SystemTime`/`Duration` conversion helpers in the `ucan` crate only.

**Rationale:** Consumers want convenience, but verification should still accept deterministic timestamps and core should stay pure.

**Alternative:** Read ambient time inside verification. Rejected because hidden clocks hurt tests and reproducibility.

### 4. Treat integration examples as API tests

**Choice:** Add examples that model HTTP bearer parsing/verification and structured failure handling for the current issue/verify surface, then wire them into verification rails. Proof-store and invocation examples are scoped to their feature-specific changes.

**Rationale:** Examples prevent API drift and show downstream crates the intended idiom.

**Alternative:** Keep only smoke examples. Rejected because they do not cover reusable application patterns.

## Risks / Trade-offs

**Serde feature policy** → If compile-time feature flags are needed, default them in the std shell while keeping `ucan-core` unaffected.

**Builder sprawl** → Keep builders close to workflow types and avoid duplicate ways to express the same operation.
