# Open-source Kiln, paid hosted compute

Commercial policy: charge for operating compute, not proprietary feature access. Keep the hosted-product implementation open source under explicitly reviewed licenses; keep credentials, customer data and private operational configuration private.

## Decision and current status

Kiln remains free/open-source software. The primary hosted product is **managed CI connected to repositories on external source hosts**, not a repository-hosting service. Customers pay for operated CI execution and, separately, optional inference, not permission to use the dashboard, CI core or self-hosted software. Existing licenses remain unchanged; this is not a proposal to make the tree uniformly permissive or to remove copyleft obligations.

**Status: bounded local implementation, not a launched paid service.** The optional [`kiln-hosted` composition](hosted-ci.md) now implements scoped accounts, upstream repository connections, reviewed pipelines, spend admission, trusted local usage settlement and invoice drafts/exports. The [retained verification](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/verification.json) records local engineering qualification, not production activation or paying customers, and retains traceability/release-readiness holds. No company formation, merchant approval, published commercial prices or live customer charge is established here. The controls below remain commercial qualification requirements; implemented mechanisms do not close those gates by themselves.

Recommended route: an invite-only B2B pilot, one supported region/currency chosen by the operator, isolated customer execution, explicit usage caps and operator-reviewed monthly invoices. Runners and inference have independent enablement gates. Start with the runner pilot while qualifying inference in parallel; neither SKU may charge before its own gates pass.

Kiln maintainers own this plan and CI integration. The operator owns the commercial offer, company and launch decision. This plan supplies an evidence-linked go/no-go process; the separately implemented `kiln-hosted` mechanisms are maintained through the [bounded operator procedure](hosted-ci.md), not qualified for paid operation by this document.

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

The journey below combines implemented operator-assisted mechanisms with still-open commercial acceptance requirements. Follow the [maintained hosted procedure](hosted-ci.md) for callable APIs and exact local boundaries; it is not public signup or production multi-tenant qualification:

1. **Admit the customer organization.** The hosted composition has durable peer-bound invitations, owner/admin/member/viewer/billing roles, project scopes and revocation through its [account APIs](hosted-ci.md#2-rpc-roles-and-project-scope). Pilot scope and terms acceptance remain operator prerequisites, not consequences of membership. Repository names and URLs are display/native facts, not Kiln organization/project identities. Begin with a dedicated control/data deployment per pilot organization; do not expose the separate single-tenant web service as a shared customer service.
2. **Authorize one repository connection.** Owner/admin connection APIs now bind organization/project/connection, stable native repository ID, exact operator-admitted source-profile bytes and connection generation. Follow [connection and route admission](hosted-ci.md#3-existing-upstream-repository-connections-and-native-routes): each generation returns an effective adapter/workflow identity requiring separate exact Lattice route admission. GitHub repository ID must match the event policy; a rename, transfer, URL change or different installation cannot silently authorize another repository or organization.
3. **Provision minimum permissions.** The existing GitHub path uses an operator-provisioned webhook secret and repository-scoped API credentials: Contents read for private checkout and Commit statuses write for results. A GitHub App installation is a future onboarding option, not existing token minting/renewal or Checks API support. No repository-contents write, administration or merge permission is required for the proposed CI offer. Credential values remain in the control-plane/acquisition boundary, never in customer argv, logs, artifacts or guest environment.
4. **Review and bind the pipeline.** Bounded typed pipeline compilation, a fail-closed GitHub Actions migration subset and a revisioned owner/admin review registry are [implemented](hosted-ci.md#5-repository-owned-pipelines-and-fail-closed-migration). Compilation alone does not authorize execution; registry review does not install a native source connection's Lattice route. Choose runner class, bounded execution policy and exact workflow revision/route. A signed webhook is not authority to execute webhook-supplied shell text or a changed pipeline; automatic repository discovery/admission must not be inferred from local authoring tools.
5. **Run and observe.** Native source intake and explicit `connections.tick` bind an exact commit to its connection generation and reviewed adapter route, with finance reserved before dispatch and source/status observations retained. The separately admitted local `execution.run` path executes reviewed plans and can settle trusted host-elapsed usage; it does not fetch upstream source itself. **Native source completion does not settle its reservation:** independently trusted exact-attempt metering remains required. The commercial journey still requires qualified joined source/run/results/usage visibility and exact-revision status. Status-publication failure is separate from CI outcome; manual reruns retain the selected revision and latest-source runs must admit a new candidate. Public forks remain denied.
6. **Pause, revoke or disconnect.** Membership revocation, connection disconnect/generation retirement and incident holds now deny new authority while preserving retained replay and financial obligations. Scoped source-credential revocation remains operator-owned, not an automatic effect of disconnect. Account for already accepted work explicitly; request cancellation where supported, reconcile unknown outcomes and do not silently free spend or claim cancellation. Source-host permission loss and installation removal must fail closed. Disconnecting CI must never delete or mutate the upstream repository; retained content follows the agreed CI retention policy.

The separate single-tenant web service's `ProjectProfile.repository_path` and enrollment API still refer to an operator-allowlisted **local** Git checkout. They are not the hosted account/remote-connection APIs above. Use the hosted composition for that bounded onboarding path; registering a local mirror, exposing its files or adding a source-hosting UI does not complete commercial qualification.

Before building a broad commercial platform, the operator should interview three candidate design partners. Three is a proposed small-cohort discovery gate, not an existing customer count: retain each partner's real workload, expected monthly usage, acceptable price/cap, privacy requirements and written willingness to join a paid pilot. If demand or contribution margin fails, revise the offer before buying capacity or building public self-service. Keep customer identities and agreements in private business records, not this repository.

## 2. What exists, and what does not

The following assessment updates the earlier source review with the implemented hosted composition and its [retained local verification](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/verification.json). This document correction performs no new deployment or hardware exercise; retained receipts apply only to their recorded cohorts.

| Area | Existing evidence | Commercial gap |
| --- | --- | --- |
| CI workspace and accounts | [Single-tenant HTTPS service](operator.md#single-tenant-live-web-service) remains separate from the [hosted Iroh/local-browser composition](hosted-ci.md#2-rpc-roles-and-project-scope), which implements durable scoped membership, invitations and revocation | No public signup or qualified production cross-customer boundary. Core tenant fairness is not hosted account isolation; local role enforcement is not a complete commercial identity/support program. |
| Remote-source CI | [Hosted scoped GitHub connections](hosted-ci.md#3-existing-upstream-repository-connections-and-native-routes), generation-bound replay/route identity and explicit ingest/tick/status over the existing adapter; [historical separate-worker exercise](../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json) and independent [native Radicle path](operator.md#native-radicle-peer-and-broker-canary) retain their own cohorts | Production credentials, public webhook/relay/TLS policy, exact per-generation worker routes and deployment-specific revocation/recovery evidence remain prerequisites. Historical live evidence does not qualify today's hosted candidate; hosted connections currently admit GitHub, not every core source adapter. |
| Pipelines | [Typed compilation, bounded Actions migration and revisioned review registry](hosted-ci.md#5-repository-owned-pipelines-and-fail-closed-migration) | Unsupported semantics fail closed. Local compilation/review is not automatic source discovery, worker route installation or proof of full Actions compatibility. |
| Execution | [Explicit namespace/QEMU backends, cgroup limits and recovery](hosted-ci.md#6-explicit-isolation-resource-limits-and-execution-recovery); retained real local trusted-namespace success/failure/cancellation and cleanup observations | Namespace execution is trusted-only. QEMU support and an operator qualification-reference field are not independent hostile-isolation evidence. Qualify the exact customer execution boundary and host cohort before selling arbitrary code execution. |
| Measurements | Hosted local execution uses trusted host elapsed time at the admitted price; [native source and fleet obligations stay unknown](hosted-ci.md#4-prices-reservations-invoices-and-unknown-obligations) without trusted metering | Independent exact-attempt usage integration for native source dispatch remains open. Worker CPU/peak-RSS observations are not authoritative money or aggregate memory billing units; ledger arithmetic reconciliation is not collection or resolution of unknown exposure. |
| Review workflow | [Required-CI qualification](../.cairn/archive/1970-01-01-complete-review-workflow/evidence/qualification.json) and [packaged checks](../.cairn/archive/1970-01-01-complete-review-workflow/evidence/checks.json) | Local qualification does not establish production operation, customer hosting or payment readiness. Existing accepted-spec traceability/release-readiness holds remain. |
| Finance and collection | [Durable account limits, immutable integer prices, reservations, trusted local usage, credits, invoice drafts/exports and outbox observations](hosted-ci.md#4-prices-reservations-invoices-and-unknown-obligations); a pinned Stripe test-only protocol boundary exists in [`finance.rs`](../crates/kiln-hosted/src/finance.rs) | No payment-sending CLI/customer payment mutation or demonstrated merchant collection. Sandbox protocol preparation is not a transmitted invoice, payment/refund or live-provider reconciliation. Approved commercial prices/consent, native-source metering, provider qualification and legal/financial gates remain open. |
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

The existing typed `config/hosted-prices.ncl` supports local price/account authoring; exact integer runner prices and limits are admitted through the [finance RPCs](hosted-ci.md#4-prices-reservations-invoices-and-unknown-obligations), not automatically loaded from that file. This is not an approved commercial catalog or inference token tariff. Before publication, bind currency, SKU/class/model, unit scale, rounding, tax presentation, effective version and customer quote. Preserve exact price identity in every reservation and usage receipt. Avoid floating-point money; round only at declared settlement boundaries. New prices apply to newly authorized work, never retroactively to active reservations or closed invoices. Initial spend caps should explicitly state whether they exclude tax; show estimated tax separately and obtain the applicable total-charge consent.

## 4. Financial authority and payment architecture

The optional `kiln-hosted` composition now implements the local account/reservation/usage/invoice portions of this architecture outside `kiln-core`. The full paid-service flow below remains a target, not evidence of provider transmission, collection or production qualification, and is not an instruction to extract a new generic billing repository.

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
| Access and support | Security/operations owner | Qualify the implemented hosted owner/admin/member/viewer/billing roles, peer revocation and attributable actions for the exact deployment; retain operator MFA, recovery, least privilege, customer isolation and audited support-access evidence. The separate single-tenant web viewer/operator tokens do not substitute for hosted membership or close these gates. |
| Reliability and cash exposure | Operations/finance owner | Named on-call/support coverage, realistic service objectives, backups with restore drill, upgrade/rollback, incident runbooks, supplier outage response and a global kill switch for new consumption/charges. No unsupported contractual uptime promise. |

Separate sensitive execution content from billing evidence. Store only necessary metadata in financial records; do not put prompts, source files, secret-bearing logs or private provider bodies in invoice descriptions or public receipts. Define deletion and access controls for content separately from the minimum financial/audit records that advisors determine must be retained.

## 6. Dependency-ordered launch backlog

All rows remain **open launch gates**, although bounded implementation now covers parts of L2a, L2, L3 and L5 as described in §2 and the [hosted procedure](hosted-ci.md). Implementation presence and local receipts do not mark a launch gate complete. Owner labels name accountable roles; the operator must assign actual people before activation. Do not promise calendar dates without capacity and business decisions.

| ID / stage | Deliverable and owner | Exit evidence / go-no-go |
| --- | --- | --- |
| L0 — validate offer | Design-partner discovery and commercial choices; founder/operator | Three qualified workload/price/privacy interviews and credible paid-pilot intent; choose seller/jurisdiction, region/currency, first runner class, first model and loss/exposure budget. No demand or viable economics → revise before capacity purchase. |
| L1 — establish authority | Company/terms/license prerequisites and scoped implementation proposals; operator, legal/accounting, release owner | Close §5 gates relevant to the pilot and resolve existing product release holds for the selected candidate; approved cost-based quotes, named owners and separately gated implementation requirements. Planning approval alone is insufficient. |
| L2a — connect remote source | Qualify implemented organization-scoped GitHub connections and hosted onboarding; Kiln hosted-CI composition owner | Use the maintained connection APIs, not the local-checkout enrollment API. Retain exact production-candidate permission, trigger/execution/status, route-admission, revocation and replay/restart evidence without creating/migrating a repository. Pilot organizations have dedicated control/data deployments. Bounded local qualification does not close this deployment gate. |
| L2 — account and meter | Qualify implemented scoped account/price/usage ledger, reservations and spend display; hosted-service composition owner | Retain exact-candidate cap/concurrency/restart/replay and correction evidence. Integrate independently trusted exact-attempt metering for native source dispatch, whose reservations currently remain unknown even after source completion. Provider meters/browser flags cannot bypass local admission; no paid dispatch without a bounded reservation. |
| L3 — qualify runners | Dedicated per-organization control/data boundary and ephemeral job VM execution; Kiln CI owner plus execution/operations owners | Real customer-shaped success/failure/cancellation jobs; cross-org access and cache/artifact poisoning attempts denied; no platform credentials or host sockets in guests, egress/metadata/privileged-device restrictions, teardown/sanitization and actual resource-limit enforcement. Do not promote bubblewrap to a hostile-tenant claim. Public forks remain disabled until separately qualified. |
| L4 — qualify inference | One authenticated model route and trustworthy usage settlement; inference composition owner with model-host/provider owners | Exact weights/license/engine/profile, context and output admission, load/capacity/latency measurements, stream disconnect/cancel/timeout/unknown-usage reconciliation, cross-customer access negatives and usage-to-cost reconciliation. Keep job execution off the model fleet. An internal `2 + 2` response is not this evidence. |
| L5 — qualify collection | Qualify the existing local invoice/outbox and test-only provider protocol boundary; complete hosted collection, refund/dispute integration; payments/finance owner | Independently authorized sandbox invoice/payment success and failure, signature/replay/order negatives, crash after provider acceptance, no duplicate invoice/charge, exact customer mapping, credits/refunds and ledger/provider reconciliation. Local drafts/protocol fixtures do not establish transmission or collection. Never use live cards or customer data as test fixtures. |
| L6 — admit paid pilot | Invite-only operation of each enabled SKU; operator plus operations/security/finance | L0–L3, including L2a, and L5 pass for hosted CI; L0–L2 and L4–L5 pass for standalone inference (which does not require a CI repository connection); both also need relevant §5 gates. Retain exact production candidate/configuration, approved terms/cap, explicit activation authority, restore/rollback drill and named support owner. A first live financial action needs explicit point-of-risk approval; it is not authorized by this plan. |
| L7 — public self-service | Signup, checkout/portal, abuse controls, scaling and published service commitments; product/operations owners | A complete pilot billing cycle reconciles customer statements, supplier costs, credits and cash; customer feedback validates value/margin. Independently review cross-tenant security, abuse/fraud controls, payment lifecycle and capacity failure. Public availability of one SKU never enables the other automatically. |

One complete billing cycle is proposed because consumption, period close, correction and collection must all be observed; it is not a guarantee of security, profitability or long-term reliability. No billing cycle or customer activation has occurred as part of this plan.

### Acceptance scenarios for implementation changes

These remain full commercial qualification requirements, not a claim that every row passes. Some mechanisms now have bounded local tests/runtime receipts in the [hosted verification](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/verification.json) and [operator checklist](hosted-ci.md#qualification-and-activation-checklist); that evidence does not establish the complete production journeys below:

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

## 7. Next qualification boundary

Organization-scoped connections, accounts/finance and reviewed pipelines are now implemented in the optional [`kiln-hosted` composition](hosted-ci.md); do not commission a duplicate L2a/L2 platform or mistake the separate local-checkout web enrollment API for that path. Kiln's operator/self-CI remains the engineering consumer; external paying demand is still an L0 gate. Kiln maintainers own the composition, and operators retain credentials, deployment and activation authority. Operator-assisted GitHub connections and dedicated deployments per pilot organization remain the proposed route; public signup, automatic App lifecycle and production shared tenancy are not established.

Use the maintained typed profiles, generation-bound source/replay namespaces, exact returned Lattice route identities, current-membership authorization and revisioned pipeline review. Reconnect must preserve historical replay/financial evidence and separately admit the new exact route, not overwrite an old store or widen worker authority. Keep connection/financial policy in the composition outside `kiln-core`; pure decisions and effect-owning shells retain their existing boundaries.

The next hosted-CI readiness work is deployment-specific end-to-end qualification: authorized administrator selects an admitted upstream repository → operator provisions exact credentials/routes → authentic event acquires the exact commit → reviewed isolated execution → matching results/status and trustworthy usage. Retain success and genuine failure, duplicate/restart without redispatch, wrong repository/organization/signature denial, revocation/reconnect with stale generation denial, moved-ref handling and unavailable-source/status reconciliation for one exact candidate/configuration. The [existing local verification](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/verification.json) and [historical live adapter exercise](../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json) are bounded evidence, not a new hosted production exercise. Native source settlement specifically still requires independently trusted exact-attempt metering; source success must not fabricate it.

Alongside this non-billable engineering path, close L0 business choices and the remaining L2/L3/L5 and §5 gates before enabling paid CI. Qualify dedicated-deployment access, source-credential exclusion, independent hostile-code isolation, dependency-aware restore/rollback and operator staffing; local trusted namespace jobs or a database-only bundle are insufficient. Independently qualify inference; it need not wait for a repository connector. Keep payment-provider work within separately approved sandbox scope until every proposed line item is explained and reconciled. No local mechanism, receipt or document authorizes charges, provider activation or removal of existing release holds.

Repository ownership:

- Kiln: CI meaning, external-source connections, runner request/result bindings and the consumer-facing hosted CI integration. Hosted account/financial mechanisms live in the separate `kiln-hosted` composition, not provider IDs in CI core. Source/collaboration hosts retain repository and merge authority; Kiln's own repository publication policy is not customer onboarding policy.
- Lattice and Aspen/Molten: their existing workflow/host/runtime mechanisms; component presence does not qualify a new commercial deployment.
- Animus: model/harness contracts and explicitly selected provider-host reuse where its actual protocol fits; it is not assigned general company billing ownership.
- spark-nixos and OnixOS: their hardware/service assembly and scoped operational evidence; host-specific capacity, secrets and deployment remain operator-owned.
- A standalone paid inference gateway's product owner and repository must be explicitly selected before implementation. Do not hide it inside Kiln's CI core or invent a product-neutral framework without an actual consumer.

Repeatability: maintain this plan with actual customer/cost evidence, use the ordered gates for each exact candidate and retain scope-limited receipts. The planning lifecycle package is [plan-hosted-compute-launch](../.cairn/archive/1970-01-01-plan-hosted-compute-launch/proposal.md). Its document checks cannot authorize live payment, legal acceptance, company registration, deployment or publication.
