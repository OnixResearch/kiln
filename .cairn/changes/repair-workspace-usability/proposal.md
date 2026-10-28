## Why

The operator requested a complete new-user UI walkthrough and repair. The live Kiln workspace exposes exact facts but buries common actions under hashes and verbose metadata, lacks guided run selection for evidence/comparison, and needs responsive and keyboard navigation fixes. The current consumer is the existing single-tenant live workspace.

## What Changes

- Repair discovery, navigation, run inspection and read-only source/review browsing using existing APIs.
- Present concise summaries before exact retained identities, preserving every authority boundary and unknown fact.
- Make mobile navigation, keyboard focus, contrast, loading, empty and error states usable.

## Impact

Kiln web-service maintainers own these frontend repairs and repeatable browser verification. No core CI semantics, provider calls, access policy, project registry, router, publication or lifecycle completion authority changes. Browser audits do not execute live CI controls. Verification uses current source assets against the real read-only service before any deployment.
