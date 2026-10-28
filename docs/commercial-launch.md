# Open-source Kiln, paid hosted compute

Commercial policy: charge for operating compute, not proprietary feature access. Keep the hosted-product implementation open source under explicitly reviewed licenses; keep credentials, customer data and private operational configuration private.

## Decision and current status

Kiln remains free/open-source software. The primary hosted product is **managed CI connected to repositories on external source hosts**, not a repository-hosting service. Customers pay for operated CI execution and, separately, optional inference, not permission to use the dashboard, CI core or self-hosted software. Existing licenses remain unchanged; this is not a proposal to make the tree uniformly permissive or to remove copyleft obligations.

**Status: launch plan, not a launched service.** No company formation, merchant approval, production activation, published prices, payment integration or live customer charge is established by this document. Future-tense controls and gates below are work to implement and qualify, not existing features.

Recommended route: an invite-only B2B pilot, one supported region/currency chosen by the operator, isolated customer execution, explicit usage caps and operator-reviewed monthly invoices. Runners and inference have independent enablement gates. Start with the runner pilot while qualifying inference in parallel; neither SKU may charge before its own gates pass.

Kiln maintainers own this plan and CI integration. The operator owns the commercial offer, company and launch decision. The durable contribution of this planning change is a repeatable, evidence-linked go/no-go process; it does not implement a billing platform.

## 1. Offer and customer validation

Positioning: **Keep your source where it is; connect it to open-source Kiln for managed CI.** Kiln hosts the CI control plane, execution, logs, artifacts and results. Customers retain their existing source/collaboration host, repository permissions and merge policy. Optional hosted inference remains a separately enabled compute offer. This is proposed positioning, not a claim of current commercial availability.

Initial customer hypothesis: small engineering teams using Nix/Linux CI or agent-assisted development that want source-bound execution, inspectable results and predictable compute bills. External demand is not yet validated. Kiln self-CI is the current engineering consumer, not evidence of a paying market.

| Offer | Customer receives | Proposed charge | Boundary |
| --- | --- | --- | --- |
| Self-hosted Kiln | Existing source, dashboard, CI/review functionality and ability to operate their own integrations | No software license or seat fee | Customer supplies infrastructure and model access; applicable source/dependency licenses still apply. |
| Hosted CI | A connection to an existing remote repository, authenticated triggers, managed execution, bounded logs/artifacts and exact-revision status returned to the source host | Execution seconds by fixed CPU/RAM runner class, displayed as runner-minutes | No repository migration, source-host subscription, queue-time fee, hidden retry fee or arbitrary public-fork execution in the pilot. |
| Hosted inference | An authenticated API for explicitly listed models, also usable by an admitted CI/agent client | Separately priced input/output token units by exact model/service revision | API access is a separate service, not a browser chat mock or a CI-success oracle. |

No initial seat subscription, expiring prepaid wallet, third-party runner marketplace, unlimited plan, broad model catalog, fine-tuning, image/audio service or guaranteed model correctness. These exclusions keep the first offer accountable; they do not restrict existing open-source rights. BYO infrastructure/model credentials remain separate from hosted consumption and must not be double-billed.

### External source, hosted CI

```text
Customer's existing source/collaboration host
  -> authenticated event for an enrolled repository and exact revision
  -> Kiln project, reviewed pipeline and admitted execution
  -> isolated runner acquires the exact source snapshot and runs the job
  -> Kiln results, logs, artifacts and usage
  -> exact-revision CI status on the existing source host
```

The upstream host remains the source of truth for branches, commits, pull requests, reviews, repository membership and merge authority. Kiln does not offer Git push endpoints, repository creation/migration, authoritative mirrors, an issue tracker or its own replacement pull-request system as part of this launch. Existing source browsing/review functionality may inspect external state; it does not transfer source ownership or grant merge permission. Kiln's own use of Radicle for publishing its software is unrelated to where customers must host their repositories.

Acquired snapshots and bounded caches are execution inputs, not a source-hosting product. They still contain customer code: private-repository access, retention, deletion, backup, region and subprocessors belong in the CI data policy. A disconnected source host does not make a cached branch tip current or authorize a substitute revision.

Recommend GitHub as the first pilot connector because [the existing adapter](operator.md#github-ci-adapter) and [historical combined live exercise](../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json) already cover authentic deliveries, exact-source jobs on a separate worker, status and recovery. That historical receipt is limited to its recorded cohort, not today's hosted candidate. [Radicle CI](operator.md#native-radicle-peer-and-broker-canary) is another existing adapter path, not a requirement for customer source migration. GitLab, Bitbucket and arbitrary Git URLs are not implied supported connectors; each needs its own admitted adapter and qualification.

### Customer journey and connection authority

The following is planned onboarding, not an existing signup or multi-tenant feature:

1. **Admit the customer organization.** An authenticated organization administrator accepts the pilot scope and selects an externally hosted repository. Repository names and URLs are display/native facts, not Kiln organization/project identities. Begin with a dedicated control/data deployment per pilot organization; do not expose the current single-tenant service as a shared customer service.
2. **Authorize one repository connection.** Record canonical organization/project/connection IDs, provider and stable native repository ID, approved repository locator, credential reference, connection generation and reviewed trigger/ref policy. Only an authorized organization administrator may create/change that mapping. GitHub repository ID must match the event policy; a rename, transfer, URL change or different installation cannot silently authorize another repository or organization.
3. **Provision minimum permissions.** The existing GitHub path uses an operator-provisioned webhook secret and repository-scoped API credentials: Contents read for private checkout and Commit statuses write for results. A GitHub App installation is a future onboarding option, not existing token minting/renewal or Checks API support. No repository-contents write, administration or merge permission is required for the proposed CI offer. Credential values remain in the control-plane/acquisition boundary, never in customer argv, logs, artifacts or guest environment.
4. **Review and bind the pipeline.** Choose runner class, bounded execution policy and exact workflow revision/route. Use the existing reviewed job/profile and Lattice routing contracts; a signed webhook is not authority to execute webhook-supplied shell text or a changed pipeline. Repository-authored pipeline discovery, if added, needs separate admission and hostile-input qualification rather than bypassing that review.
5. **Run and observe.** A valid push or admitted same-repository PR binds one exact commit to the connection generation, pipeline, run and attempt. Reserve spend before any paid dispatch. Show queued/running/terminal state, exact source revision, logs/artifacts and usage in Kiln; publish the existing commit-status contract back to that revision. A status-publication failure remains separate from the CI outcome. Manual reruns retain the selected revision; latest-source runs must explicitly admit a new candidate. Public forks remain denied.
6. **Pause, revoke or disconnect.** Deny new admissions under the old connection generation, revoke scoped credentials and pending-dispatch authority, and preserve replay protection and audit/financial evidence. Account for already accepted work explicitly; request cancellation where supported, reconcile unknown outcomes and do not silently free spend or claim cancellation. Source-host permission loss and installation removal must fail closed. Disconnecting CI must never delete or mutate the upstream repository; retained content follows the agreed CI retention policy.

The current web service's `ProjectProfile.repository_path` and enrollment API refer to an operator-allowlisted **local** Git checkout. They are not remote-source onboarding, a GitHub installation registry or a commercial account system. Connect hosted onboarding to the existing remote-source adapter path; do not claim that registering a local mirror, exposing its files or adding a source-hosting UI completes it.

Before building a broad commercial platform, the operator should interview three candidate design partners. Three is a proposed small-cohort discovery gate, not an existing customer count: retain each partner's real workload, expected monthly usage, acceptable price/cap, privacy requirements and written willingness to join a paid pilot. If demand or contribution margin fails, revise the offer before buying capacity or building public self-service. Keep customer identities and agreements in private business records, not this repository.

## 2. What exists, and what does not

The following assessment is source review, not a new deployment or hardware exercise.

| Area | Existing evidence | Commercial gap |
| --- | --- | --- |
| CI workspace | [Single-tenant HTTPS service](operator.md#single-tenant-live-web-service), source-bound jobs, recovery, logs/artifacts and exact controls | No organization onboarding, commercial entitlement service or qualified cross-customer authorization boundary. Core tenant fairness is not hosted account isolation. |
| Remote-source CI | [GitHub webhook/source/status adapter](operator.md#github-ci-adapter) and [recorded separate-worker exercise](../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json); independent [native Radicle path](operator.md#native-radicle-peer-and-broker-canary) | Operator-configured adapters are not customer-facing organization/repository onboarding. Add scoped connection lifecycle, dashboard integration and revocation/recovery evidence; do not build source hosting or imply universal forge support. |
| Execution | [Worker capacity and namespace isolation](operator.md#pipeline-measurements-and-worker-capacity) with authenticated observations, reviewed profiles and fail-closed preflight | Explicitly not hostile-code confinement or host-wide resource accounting. Qualify a customer execution boundary before selling arbitrary code execution. |
| Measurements | Same worker documentation: monotonic elapsed measurements, CPU observations and child-process peak RSS; interrupted measurements remain unknown | These observations are not an authoritative invoice ledger. Peak RSS is not aggregate memory, a reserved memory allocation or a billing unit. |
| Review workflow | [Required-CI qualification](../.cairn/changes/complete-review-workflow/evidence/qualification.json) and [packaged checks](../.cairn/changes/complete-review-workflow/evidence/checks.json) | Local qualification does not establish production operation, customer hosting or payment readiness. Existing accepted-spec traceability/release-readiness holds remain. |
| Billing | Targeted review of `crates/`, `config/`, `docs/` and `Cargo.toml` found no commercial payment, invoice, usage-metering or spend-admission implementation | Build durable financial accounting, customer authorization, limits, provider reconciliation and invoice support; do not infer these from the SaaS-style UI. |
| Inference clients | Reviewed `animus/docs/live-openai-provider-host.md`: exact endpoint cohorts, bounded response/SSE parsing, usage observations and explicit cancellation; its non-claims exclude billing correctness and production readiness | A provider client is not a public paid inference gateway, broad protocol compatibility, remote cancellation guarantee or validated commercial meter. Preserve exact existing model/control contracts when consuming it. |
| Model hosting | Reviewed `spark-nixos/README.md` §§ What has been tested / Models: three native GB10 hosts, CUDA/RDMA observations and real Docker-served chat completion | This is not paid-service load, tenant security, billable token reconciliation or availability evidence. The NixOS model containers' GPU/RDMA path remains untested. Its local README is newer than the workspace wiki's no-hardware account; do not repeat that stale claim. |
| Licensing | [License map](../LICENSES.md), including AGPL/MPL linkage, the GPL-2.0-only adapter and unresolved optional `NOASSERTION` components | Review the exact hosted/shipped closure, corresponding-source and notice duties, compatibility, model weights and provider commercial terms. A passing build or owned-code license is not clearance. |

Cross-repository source paths above are research references, not runtime dependencies or source imports. Before adoption, the producing owner must approve the contract and the consumer must record an immutable source/artifact pin, license notices, compatibility checks and bounded evidence. This plan changes no sibling repository.

## 3. Metering and pricing decisions

### Runner units

Define a runner class by architecture, vCPU allocation, RAM allocation, isolation profile and a stable class identity. Sell **execution time at that class's published rate**, not estimated actual CPU or RSS consumption.

The commercial meter must run outside customer-controlled job code, with scoped host/executor identity and independently enforceable duration/resource limits. A worker bearer token inside a customer guest, customer log text or a browser-reported duration cannot authenticate billable usage. Qualify clock/reset handling and replay/reconciliation across host/service restart rather than comparing unrelated monotonic clocks.

- Start billing at an authenticated executor start acknowledgement for the customer workload; stop at observed termination. Publish the exact start/stop boundary, including whether customer checkout/setup steps are inside it. Platform provisioning and queue waiting are excluded.
- Record integer monotonic duration in a named base unit. Aggregate per class and billing period, then apply a published rounding rule once; do not round every task up to a minute. Missing starts/stops or contradictory evidence enter reconciliation, not fabricated duration.
- Customer-code failure is billable for observed execution; cancellation stops new work and settles verified elapsed usage. Platform-caused failure receives a traceable credit under the agreed policy. Operator-generated retries are not hidden extra customer charges. Customer-requested reruns are new authorized attempts with visible usage.
- Include a bounded, published artifact/log retention and transfer allowance in the pilot price. Reject excess admission or obtain explicit approval; do not silently introduce storage/egress charges. A separately priced storage service needs its own meter later.

### Inference units

Start with one explicitly approved operator-hosted model and a documented text API subset. The initial public interface should document its supported OpenAI-compatible chat-completions operations, not claim full OpenAI API compatibility. Existing Animus Responses cohorts are distinct contracts; adapting between them needs explicit conformance evidence, not a model-ID rename.

- Publish exact model/weights, serving revision, context/output limits, supported stream behavior and price version. Do not silently substitute a model/provider. Customer-approved fallback needs its own price, limits and request authorization.
- Meter independently admitted input and output tokens. Define cached-input and reasoning-token semantics per provider; exclude subsets from the corresponding full-price category so they cannot be charged twice. Unsupported dimensions are not invented or sold.
- Bind each receipt to the authenticated customer, request/attempt, exact provider response, model revision and authoritative server-side usage observation. Client-supplied token counts and tokenizer estimates are previews, never sufficient invoice evidence.
- Interrupted streams, lost terminal usage, provider timeouts and unknown remote cancellation retain their reservation and need reconciliation. Do not blindly retry after output may have been produced. Customer-billable partial output requires reliable usage evidence and an agreed cancellation policy; otherwise hold the charge and record operator exposure.
- External paid API resale is disabled until the operator verifies provider permission, commercial terms, data handling and reconciliation. Model-weight availability alone does not grant every hosted commercial use. A successful internal inference demo does not establish these rights.
- Do not run arbitrary customer CI commands on model-serving GPU hosts. Separate secrets, workload identities, networks and capacity pools.

### Price publication and economics

No numerical price is approved here. Before quoting, benchmark representative cold/warm runner jobs and model workloads at expected concurrency and low utilization. Measure compute/GPU lease or depreciation, idle capacity, power, storage/egress, upstream usage, payment fees, expected credits/fraud, operations/support and taxes where relevant. Account for fixed invoice fees at the expected invoice size; do not confuse full-utilization GPU cost with the cost of serving a small pilot.

For a candidate unit price, calculate contribution margin from realistic paid usage, including variable payment fees and allocated support/idle costs. The operator must choose a target margin and acceptable downside before approving capacity and price. Sensitivity cases must include low utilization, slow/long-context requests, cancellations, failures and one unpaid capped account. Do not claim profitability from a successful benchmark.

A future human-authored Nickel price catalog should bind currency, SKU/class/model, integer or fixed-point rate, unit scale, rounding, tax presentation, effective version and customer quote. Preserve exact price identity in every reservation and usage receipt. Avoid floating-point money; round only at declared settlement boundaries. New prices apply to newly authorized work, never retroactively to active reservations or closed invoices. Initial spend caps should explicitly state whether they exclude tax; show estimated tax separately and obtain the applicable total-charge consent.

## 4. Financial authority and payment architecture

This is a proposed hosted-service composition, not a change to `kiln-core` or an instruction to extract a new generic billing repository.

```text
Authenticated organization + price + limits
  -> durable spend reservation and admission
  -> runner executor OR inference provider
  -> admitted usage receipt / unresolved observation
  -> rated usage + corrections
  -> draft invoice and reconciliation
  -> payment provider hosted page
  -> verified provider observations and local financial state
```

Keep canonical customer/organization, usage, reservation and invoice identities independent of Stripe or any executor/model provider's IDs. Use BLAKE3 for stack-owned content identities and preserve protocol-native IDs separately. Provider adapters remain outside pure admission/rating decisions; shells own clocks, storage, networks, credentials and reconciliation.

The ledger must keep consumed, reserved, credited, invoiced, paid, refunded and disputed amounts distinct. Payment success does not prove workload completion; workload success does not prove payment. Financial records need append-only, attributable corrections and auditable invoice line items, not mutable totals derived from browser state or telemetry.

Reserve a conservative maximum before effects: runner class rate × enforced maximum duration; inference maximum admitted input/context and output at the pinned rate. Atomically enforce the organization cap, per-request cap, concurrency and platform exposure limit. Reject work whose maximum cannot be bounded or reserved. Concurrency, request replay and service restart must not oversubscribe remaining spend. Missing usage cannot free an uncertain reservation. If usage exceeds the guaranteed bound, block further work and resolve the breach; do not silently exceed the customer's authorized charge.

Lattice's `crates/budget-ledger-core` is a local orchestration ledger with explicit external-promotion requirements and non-claims about billing truth. Review its reserve/settle pattern with the owner; do not copy/extract it or assume it supplies a money ledger. Reuse must replace real duplication and preserve both positive and negative behavior through licensed, pinned interfaces.

### Recommended collection path

Recommend **Stripe Invoicing with its hosted invoice page for the pilot**, subject to operator jurisdiction, processor acceptance and terms review. The business remains the merchant under this proposed direct-merchant model; do not assume that using Stripe makes it merchant of record or solves company/tax obligations. There is no marketplace or connected-account payout requirement in this offer.

1. An operator admits an organization after the customer accepts the quote, spend cap, service terms and privacy/data-processing terms. The pilot grants bounded credit explicitly; it is not unlimited postpaid access.
2. The service records usage and exposes the current consumed/reserved amounts and line-item export. A payment-provider outage cannot create unbounded credit or bypass local limits.
3. Close a billing period only after reconciling receipts, corrections, credits and provider outcomes. Prepare an invoice draft from exact ledger entries; operator review precedes finalization/sending in the pilot. Link the provider's private hosted invoice page only to the authorized billing administrator.
4. Verified payment, failure, refund and dispute observations update their separate local states. Delinquency suspends **new hosted consumption** under the agreed policy; it must not silently delete source/history or remove open-source/self-hosted rights. Define read/export retention and lawful record retention independently.
5. A later self-service phase can add hosted Checkout/payment-method setup, customer portal and metered Billing. Do not implement an internal card form or store card numbers/CVC. Hosted payment UI reduces exposure but does not itself establish PCI or broader compliance.

Primary provider references inspected for this plan:

- [Hosted invoice page](https://docs.stripe.com/invoicing/hosted-invoice-page): provider-hosted payment, invoice PDFs/receipts and payment authentication. Do not automatically save or reuse payment methods without applicable consent.
- [Webhooks](https://docs.stripe.com/webhooks): verify signatures over the raw bounded request, persist a durable inbox before acknowledging, deduplicate event and logical object transitions, handle unordered delivery and reconcile current provider state. A checkout return URL, event timestamp or duplicate webhook must not grant entitlement or charge twice. Pin the selected supported API/schema and separate sandbox/live credentials, endpoints, customer mappings and journals.
- [Usage recording](https://docs.stripe.com/billing/subscriptions/usage-based/recording-usage-api): meter processing is asynchronous. If adopted later, export from an idempotent durable outbox, handle validation/rejection and invoice cutoffs, reconcile before finalization and retain corrections. Meter summaries cannot enforce instantaneous spend caps or replace the local ledger.
- [Tax registration guidance](https://docs.stripe.com/tax/registering): registration, collection and remittance depend on the business and jurisdictions. An accountant/tax advisor must determine the actual obligations; a tax-calculation feature is not blanket registration or legal clearance.

No provider account, API key, webhook endpoint, invoice or payment is created by this planning work. Provider pricing, eligibility and legal terms must be rechecked before a real integration decision.

## 5. Company and operating readiness

The operator must close these gates before any paid pilot; the repository cannot establish them from code:

| Gate | Accountable role | Required evidence |
| --- | --- | --- |
| Legal seller and funds | Founder/operator with legal/accounting advisors | Chosen jurisdiction/entity or other lawful seller status, ownership/IP authority, business banking/accounting and approved merchant account; private KYB/tax details kept outside Git. Do not assume a particular entity form or country. |
| Name and offer | Founder/operator | Product/company name and domain clearance, approved pricing/currency/region, honest feature/security claims, contact and support channel. No unsupported SOC 2, SLA, zero-retention or model-quality claims. |
| Customer agreement | Operator with counsel | Reviewed service terms/order form, acceptable-use and abuse policy, cancellation/refund/credit/dispute process, tax treatment, inference/model limits and explicit consent for charges. This plan is not legal advice or contract text. |
| Privacy and data | Operator/security with counsel | Data inventory and roles, privacy notice/DPA as applicable, subprocessors/model-provider terms, region/retention/deletion policy, prompt/code training-use policy and incident notification process matching actual controls. Do not promise no training or no logging without backend evidence. |
| Licensing and supply chain | Release owner with legal review | Exact deployed/shipped dependency and model closure, license compatibility review, notices and required source availability; resolve/exclude unlicensed optional components. Open-source launch is not a blanket relicensing decision. |
| Access and support | Security/operations owner | Owner/billing-admin/member roles, attributable admin actions, operator MFA, customer/service-key revocation and recovery, least privilege, customer isolation and audited support access. Current viewer/operator tokens alone are not a complete customer account system. |
| Reliability and cash exposure | Operations/finance owner | Named on-call/support coverage, realistic service objectives, backups with restore drill, upgrade/rollback, incident runbooks, supplier outage response and a global kill switch for new consumption/charges. No unsupported contractual uptime promise. |

Separate sensitive execution content from billing evidence. Store only necessary metadata in financial records; do not put prompts, source files, secret-bearing logs or private provider bodies in invoice descriptions or public receipts. Define deletion and access controls for content separately from the minimum financial/audit records that advisors determine must be retained.

## 6. Dependency-ordered launch backlog

All rows are **open launch work**, not completed by this document. Owner labels name accountable roles; the operator must assign actual people before activation. Do not promise calendar dates without capacity and business decisions.

| ID / stage | Deliverable and owner | Exit evidence / go-no-go |
| --- | --- | --- |
| L0 — validate offer | Design-partner discovery and commercial choices; founder/operator | Three qualified workload/price/privacy interviews and credible paid-pilot intent; choose seller/jurisdiction, region/currency, first runner class, first model and loss/exposure budget. No demand or viable economics → revise before capacity purchase. |
| L1 — establish authority | Company/terms/license prerequisites and scoped implementation proposals; operator, legal/accounting, release owner | Close §5 gates relevant to the pilot and resolve existing product release holds for the selected candidate; approved cost-based quotes, named owners and separately gated implementation requirements. Planning approval alone is insufficient. |
| L2a — connect remote source | Organization-scoped repository connections and hosted-CI onboarding over the existing GitHub path; Kiln hosted-CI composition owner | Enroll an existing external repository without creating/migrating one; qualify permission checks, exact-revision trigger/execution/status, generation-bound revocation and replay/restart behavior. Pilot organizations have dedicated control/data deployments. The current local-checkout enrollment API does not satisfy this gate. |
| L2 — account and meter | Organization/billing-admin access, canonical usage/price ledger, reservations and spend display; hosted-service composition owner | Restart/concurrency tests prove atomic cap admission, replay safety, exact price binding and auditable corrections. Provider meters/browser flags cannot bypass local admission. No paid dispatch without a bounded reservation. |
| L3 — qualify runners | Dedicated per-organization control/data boundary and ephemeral job VM execution; Kiln CI owner plus execution/operations owners | Real customer-shaped success/failure/cancellation jobs; cross-org access and cache/artifact poisoning attempts denied; no platform credentials or host sockets in guests, egress/metadata/privileged-device restrictions, teardown/sanitization and actual resource-limit enforcement. Do not promote bubblewrap to a hostile-tenant claim. Public forks remain disabled until separately qualified. |
| L4 — qualify inference | One authenticated model route and trustworthy usage settlement; inference composition owner with model-host/provider owners | Exact weights/license/engine/profile, context and output admission, load/capacity/latency measurements, stream disconnect/cancel/timeout/unknown-usage reconciliation, cross-customer access negatives and usage-to-cost reconciliation. Keep job execution off the model fleet. An internal `2 + 2` response is not this evidence. |
| L5 — qualify collection | Hosted invoice adapter, verified webhook inbox, refund/dispute support; payments/finance owner | Sandbox invoice/payment success and failure, signature/replay/order negatives, crash after provider acceptance, no duplicate invoice/charge, exact customer mapping, credits/refunds and ledger/provider reconciliation. Never use live cards or customer data as test fixtures. |
| L6 — admit paid pilot | Invite-only operation of each enabled SKU; operator plus operations/security/finance | L0–L3, including L2a, and L5 pass for hosted CI; L0–L2 and L4–L5 pass for standalone inference (which does not require a CI repository connection); both also need relevant §5 gates. Retain exact production candidate/configuration, approved terms/cap, explicit activation authority, restore/rollback drill and named support owner. A first live financial action needs explicit point-of-risk approval; it is not authorized by this plan. |
| L7 — public self-service | Signup, checkout/portal, abuse controls, scaling and published service commitments; product/operations owners | A complete pilot billing cycle reconciles customer statements, supplier costs, credits and cash; customer feedback validates value/margin. Independently review cross-tenant security, abuse/fraud controls, payment lifecycle and capacity failure. Public availability of one SKU never enables the other automatically. |

One complete billing cycle is proposed because consumption, period close, correction and collection must all be observed; it is not a guarantee of security, profitability or long-term reliability. No billing cycle or customer activation has occurred as part of this plan.

### Acceptance scenarios for implementation changes

These are future executable acceptance cases, reviewed as plan coverage here, not passing runtime tests:

| Case | Required outcome |
| --- | --- |
| Enroll an existing remote repository and deliver a valid push/PR | Source remains upstream; the admitted connection binds organization/project, stable repository ID, exact commit and reviewed workflow to one run and matching source-host status. |
| Repository renamed/transferred, connection revoked, or old event replayed after reconnect | A display-name match never substitutes authority. Reject unapproved mappings/generations and pending admissions; reconcile accepted work without redispatch, stale status or loss of retained usage. |
| Source fetch unavailable, branch moved, or status publication fails | Never build a substitute/latest revision or report success from missing evidence. Preserve the exact candidate and distinguish acquisition, execution and status-publication outcomes. |
| Runner starts, completes, and its receipt is retried | Same attempt produces one usage fact and one rated charge; the exact class, price and observed interval can be independently traced to the invoice. |
| Customer runner job fails or is cancelled | Settle verified execution only; show outcome and cancellation boundary. Platform-failure credit remains a separate auditable correction, not erased usage. |
| Two requests compete for the last available spend | Only atomically affordable reservations admit; bounded in-flight work never silently authorizes spending above the customer cap. |
| Service crashes after dispatch or provider invoice acceptance | Recover the same operation/reservation; reconcile uncertainty before redispatch or reinvoice. Missing observations do not mean zero usage or a failed payment effect. |
| Price changes while work is active | Existing reservations settle at their original price; newly authorized work uses the newly admitted quote. |
| Inference completes or disconnects after partial output | Preserve exact provider/request binding and settle admitted non-overlapping token dimensions; unknown terminal usage remains held, not estimated into a bill or blindly retried. |
| Model license, provider resale right or privacy guarantee is absent | Reject paid enablement for that model/route; a technically reachable endpoint is insufficient. |
| Forged, duplicated, delayed or out-of-order payment event | Reject bad signatures/account/mode; durably deduplicate and reconcile the exact customer/object without duplicate charges or stale entitlement. |
| Customer tries another organization's job, artifact, cache, prompt or invoice | Deny before access/effects; scoped keys cannot acquire operator or billing-owner authority. |
| Processor outage, exhausted cap, delinquency or emergency shutdown | Deny new unaffordable consumption, retain bounded ongoing obligations and permit policy-allowed support/export; no deletion or arbitrary charge amplification. |

## 7. Next implementation boundary

The next CI implementation change should deliver **organization-scoped connection of an existing remote repository to the existing CI path** (L2a), not source hosting. Its current consumer is Kiln's operator/self-CI organization; external paying demand is still an L0 gate. Kiln maintainers own the connection composition, and operators retain source-host credentials, deployment and activation authority. Start with an operator-assisted GitHub connection and a dedicated deployment per organization; public signup, automatic App lifecycle and a shared multi-tenant service are separate later changes.

Keep connection/organization policy outside `kiln-core`, and use typed Nickel inputs plus BLAKE3 canonical identities. Compose the existing GitHub authentication, exact-source job, durable replay, Lattice execution and commit-status ports rather than a second webhook engine. The composition must retain a connection generation and exact adapter-profile/store binding; the existing replay database rejects changed profiles, so reconnection requires an explicit recovery/migration decision preserving old replay records, not overwriting configuration or deleting the store. Pure decisions cover authorization, lifecycle and admitted requests; shells own credentials, storage, source acquisition and provider calls.

Completion of that implementation requires an executable end-to-end path: authorized organization administrator selects an existing repository → validates scoped permissions/configuration → durably enrolls the connection → authentic event acquires the exact commit → reviewed isolated execution → results visible in Kiln and matching commit status upstream. Show both success and real code failure, duplicate delivery and restart recovery without redispatch, wrong repository/organization and forged-signature denial, revocation/reconnect with stale generation denial, moved-ref handling and unavailable-source/status reconciliation. Exercise dedicated-deployment access boundaries and source credential exclusion. Retain exact candidate/configuration and bounded receipts. Tests of an in-memory registry or a connection form alone do not complete this slice. None of these new onboarding cases has been exercised by this planning change.

Alongside that non-billable engineering path, close L0 business choices and commission L2 account/usage/spend admission and L3 isolated-runner qualification before enabling paid CI. L2a alone grants no billing, hostile-code or customer-launch authority. Independently qualify inference; it need not wait for a repository connector. Keep payment-provider work in sandbox until the financial ledger can explain every proposed line item. Do not start by bolting a Buy button onto the dashboard or moving payment code into `kiln-core`.

Repository ownership:

- Kiln: CI meaning, external-source connections, runner request/result bindings and the consumer-facing hosted CI integration. Hosted commercial account/financial decisions live in a separate composition, not provider IDs in CI core. Source/collaboration hosts retain repository and merge authority; Kiln's own repository publication policy is not customer onboarding policy.
- Lattice and Aspen/Molten: their existing workflow/host/runtime mechanisms; component presence does not qualify a new commercial deployment.
- Animus: model/harness contracts and explicitly selected provider-host reuse where its actual protocol fits; it is not assigned general company billing ownership.
- spark-nixos and OnixOS: their hardware/service assembly and scoped operational evidence; host-specific capacity, secrets and deployment remain operator-owned.
- A standalone paid inference gateway's product owner and repository must be explicitly selected before implementation. Do not hide it inside Kiln's CI core or invent a product-neutral framework without an actual consumer.

Repeatability: maintain this plan with actual customer/cost evidence, use the ordered gates for each exact candidate and retain scope-limited receipts. The planning lifecycle package is [plan-hosted-compute-launch](../.cairn/changes/plan-hosted-compute-launch/proposal.md). Its document checks cannot authorize live payment, legal acceptance, company registration, deployment or publication.
