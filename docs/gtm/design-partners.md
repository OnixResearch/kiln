# Hosted-CI design-partner discovery

Status: this guide supports launch gate L0 in the
[commercial plan](../commercial-launch.md). Hosted CI is a bounded local
implementation, not a launched paid service. No price is approved. This guide
authorizes no offer, contract, charge or deployment. If this guide conflicts
with the commercial plan, the plan wins.

## What L0 needs

The launch backlog defines L0 as:

> Three qualified workload/price/privacy interviews and credible paid-pilot
> intent; choose seller/jurisdiction, region/currency, first runner class,
> first model and loss/exposure budget. No demand or viable economics →
> revise before capacity purchase.

Three is a proposed discovery gate, not a customer count. An interviewed team
is not a customer.

- Evidence: for each candidate, a real workload, an expected monthly usage, an
  acceptable price or cap, privacy requirements and written willingness to join
  a paid pilot.
- Operator choices: seller and jurisdiction, region and currency, first runner
  class, first model, and a loss and exposure budget.

Gates L0 through L7 are all open. L0 authorizes no offer, contract or charge.

## Who to talk to

The plan's customer hypothesis, still unvalidated:

> small engineering teams using Nix/Linux CI or agent-assisted development that
> want source-bound execution, inspectable results and predictable compute
> bills

External demand is not yet validated, and Kiln self-CI is the current
engineering consumer, not evidence of a paying market. Record these signals,
each tied to a plan requirement:

| Signal | Why it matters |
| --- | --- |
| Source already on GitHub | GitHub is the proposed first pilot connector, and hosted connections remain GitHub-only in this plan. |
| Workloads are built on Linux | A runner class is defined by architecture, vCPU allocation, RAM allocation and isolation profile; the first class is an L0 choice. |
| Contributors are trusted, not anonymous | Public forks remain denied in the pilot, so a team that depends on fork pull requests cannot map its workload yet. |
| The team can state a spend cap | The pilot grants bounded credit explicitly; it is not unlimited postpaid access, so expected usage and a cap are admission inputs. |
| A dedicated deployment is acceptable | The proposed route begins with a dedicated control and data deployment per pilot organization. |
| Pipelines can be reviewed and bound | Compilation alone does not authorize execution; an owner or admin must review a pipeline revision before it runs. |
| The team can describe its privacy position | Private-repository access, retention, deletion, backup and region belong in the CI data policy and must be assessed first. |
| The workload can be estimated monthly | Execution time is sold by runner class, so a bounded reservation needs an expected monthly usage. |

## Who is not a fit now

- Needs arbitrary public-fork execution. Public forks remain disabled in the
  pilot until separately qualified.
- Needs GitLab, Bitbucket or arbitrary Git URL connectors. Each needs its own
  admitted adapter and qualification; none is implied today.
- Needs Kiln to host, create or migrate repositories. Hosted CI is not a
  repository-hosting service, a Git push endpoint or an authoritative mirror.
- Needs an SLA, uptime commitment, support response time or compliance
  attestation. None is offered, and unsupported SOC 2 or SLA claims are barred.
- Needs seat-based, unlimited or prepaid-wallet pricing. The first offer
  excludes seat subscriptions, expiring prepaid wallets and unlimited plans.
- Needs hostile multi-tenant isolation now. Namespace execution is trusted-only
  and the hostile-isolation gate is open; no receipt claims otherwise.
- Needs hosted inference bundled with CI. Inference is a separately enabled
  compute offer with its own gate, and no model route is approved.
- Needs existing GitHub Actions workflows to run unchanged. Pipelines are
  reviewed and typed, and a bounded migration subset fails closed.

## What a pilot would and would not include

Proposed, subject to gates L0-L6 ([launch backlog](../commercial-launch.md#6-dependency-ordered-launch-backlog)).

| Would include | Would not include |
| --- | --- |
| A connection to an existing remote repository, GitHub first | Repository migration, creation or a Git push endpoint |
| Authenticated triggers for enrolled repositories and exact revisions | A source-host subscription or a queue-time fee |
| Managed execution on a fixed CPU and RAM runner class | A hidden retry fee or arbitrary public-fork execution |
| Bounded logs and artifacts inside a published allowance | A seat subscription, expiring prepaid wallet or unlimited plan |
| Exact-revision status returned to the source host | An authoritative mirror, issue tracker or replacement pull-request system |
| Isolated customer execution on a dedicated deployment | A third-party runner marketplace |
| Explicit usage caps and operator-reviewed monthly invoices | A broad model catalog, fine-tuning or an image and audio service |
| Execution seconds displayed as runner-minutes | A guarantee of model correctness or a CI-success oracle |

## Discovery interview guide

Plan a 30-minute first conversation. Pick the questions that matter for this
candidate, follow the answers rather than the script, and schedule a
follow-up for the rest. Do not quote a price, a date or an unapproved
capability.

### Workload

- Walk me through the last merged change. What had to run before it landed?
- Which jobs are slowest, flakiest or hardest to predict?
- How many repositories build on every push or on a schedule, and how often?
- Which parts of a build need Linux specifically, and which do not?
- What happens today when two changes need the same runner at once?

### Current CI and pain

- Tell me about the last time a CI result was wrong or hard to explain.
- What do you do when a job disappears, hangs or times out?
- How do you establish which revision a green check refers to?
- What happens to your team when the current CI provider has an outage?
- Which CI changes need review today, and who reviews them?

### Source host and permissions

- Where does your source live, and who administers that host?
- What is the minimum repository permission set you would grant a CI system,
  and would read-only contents and commit-status writes be enough?
- How are webhook secrets and API credentials handled today?
- What happens when a status cannot be published back to the source host, and
  how should that be surfaced?
- Who could approve a single-repository connection, and what would make you
  refuse one?

### Security, privacy and data

- What in your repository is most sensitive, and what would you require of a
  CI operator?
- How long must logs and artifacts exist, and what must be deleted sooner?
- Are there regions or jurisdictions your code or metadata must stay within?
- What deletion, export or backup commitments does your review require?
- What would you need before private-repository jobs ran on another host?

### Volume and spend

- What does a typical month of CI execution look like in jobs and duration?
- How much does that vary between quiet and busy weeks?
- What spend cap would you set for a first engagement, and who would own it?
- How would a monthly invoice fit your finance process, and who reviews it?
- What would a usage statement need to show for you to trust it?

### Decision and timing

- Who decides to try a different CI path, and who else is involved?
- What would you need to see before agreeing to a paid pilot?
- What would make you decline even if the capability fit?
- When does your current CI arrangement next come up for review?
- What would success look like three months into a pilot?
- What would it take for you to commit in writing to a paid pilot?

## What to record, and where

For each candidate, record the fields the plan requires: real workload,
expected monthly usage, acceptable price or cap, privacy requirements and
written willingness to join a paid pilot. Note open questions and the owner.

Customer identities and agreements stay in private business records, not this
repository; no partner name, repository URL, price or signed term belongs in a
tracked file.

| Field | Entry |
| --- | --- |
| Private partner reference | |
| Real workload | |
| Expected monthly usage | |
| Acceptable price or cap | |
| Privacy requirements | |
| Written willingness to join a paid pilot | |
| Follow-up owner | |

## Unit-economics worksheet

No numerical price is approved. Collect these inputs before any quote, as the
[price publication and economics section](../commercial-launch.md#price-publication-and-economics) requires:

| Input | What to collect |
| --- | --- |
| Runner class | Architecture, vCPU allocation, RAM allocation, isolation profile and stable class identity |
| Cold and warm job duration | Representative benchmarks at expected concurrency |
| Utilization | Expected paid usage and idle capacity |
| Compute cost | Lease or depreciation for the runner capacity |
| Power | Power cost where relevant to the operator |
| Storage and egress | Bounded log and artifact allowance plus transfer |
| Upstream usage | Third-party provider usage attributable to a job |
| Payment fees | Variable fees plus the fixed invoice fee at the expected invoice size |
| Expected credits and fraud | Platform-caused failure credits and unpaid capped accounts |
| Support time | Allocated operations and support time |
| Taxes | Tax presentation and obligations where relevant |

The required sensitivity cases are low utilization, cancellations, failures and
one unpaid capped account. The output is contribution margin from realistic
paid usage, including variable payment fees and allocated support and idle
costs. The operator chooses a target margin and an acceptable downside before
approving capacity and price. A successful benchmark is not a profitability
claim.

## What not to promise

- An availability date. Calendar dates need capacity and business decisions
  that do not exist yet.
- A price or currency amount. No numerical price is approved.
- An SLA, uptime figure or support response time.
- Zero retention, no logging or no training on customer data, unless backend
  evidence supports the exact statement.
- A SOC 2, ISO or other compliance attestation.
- Execution of arbitrary public-fork code.
- A model-quality or correctness guarantee for hosted inference.
- That local qualification equals production qualification. Local checks,
  fixtures and planning gates prove only their exercised boundaries.
- Exactly-once delivery or execution.
- A sandbox or hostile multi-tenant isolation claim.

## After the interviews

L0 can close only when the interviews show credible paid-pilot intent and the
operator's choices are recorded. If demand or
contribution margin fails, revise the offer before buying capacity or building
public self-service; do not publish a price or open signup on interest alone.

If both hold, proceed to L1 and close the company, terms and license
prerequisites relevant to the pilot. Nothing here authorizes a charge;
activation still needs L6 and point-of-risk approval.

Gate order is in the
[launch backlog](../commercial-launch.md#6-dependency-ordered-launch-backlog);
the bounded operator procedure and activation checklist are in
[hosted CI](../hosted-ci.md#qualification-and-activation-checklist). Neither
supplies a production or hostile-isolation claim.
