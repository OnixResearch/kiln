## Why

The operator selected a free/open-source Kiln product funded by paid hosted CI and inference, and requested launch planning before payment implementation. The hosted CI product connects to repositories on existing remote source hosts; it is not source hosting. Local CI and dashboard qualification do not establish a company, billable usage, safe customer hosting or permission to charge.

## What Changes

- Add `docs/commercial-launch.md` with the offer, evidence-backed readiness gaps, pricing and accounting decisions, company prerequisites and independently gated runner/inference launch sequence.
- Link the plan from the README without advertising an available paid service.
- Make external-source ownership explicit, define repository-connection onboarding and revocation, and specify the next hosted-CI implementation's end-to-end acceptance without changing runtime behavior.
- Record planning acceptance and document checks in this native lifecycle package. No application, configuration, accepted specification, license, deployment or payment-provider state changes.

## Impact

- **Current consumer:** Kiln's operator and self-CI team preparing a paid hosted-compute offer; external customer demand remains unvalidated.
- **Durable capability:** A repository-owned decision and go/no-go checklist prevents payment enablement from being mistaken for safe product readiness.
- **Owner:** Kiln maintainers own this plan and CI composition. The operator owns the company, prices, customer agreements and live activation; inference provider and hosting owners retain their separate boundaries.
- **Adoption:** Validate demand and unit economics first; authorize separate implementation changes and exact release candidates only after their prerequisites are satisfied.
- **Testing:** Validate native proposal/design/tasks gates and classification, check Markdown link targets, and review positive/negative launch scenarios. No runtime tests for prose-only changes.
- **No accepted-spec effect:** This is a planning document, not an implementation, runtime policy, accepted requirement or launch approval. Future implementation must supply its own specifications, gates and evidence.
