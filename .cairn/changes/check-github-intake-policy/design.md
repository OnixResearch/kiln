## Context

`poll` reads `--enrollment` as `Enrollment` and `--policy` as `ImplementationIssuePolicy` through the binary's bounded `read_json`, validates the policy's selection, pages the transport, and admits each normalized issue through `Implementer::admit`, whose `WorkLedger::admit` applies `validate_work` against the enrollment. Webhook intake validated the repository policy, base reference and selection inline. `read_json` discarded the serde error. The onix-core `policy-check` unit runs `qualification.py policy`, which inspects selected JSON keys in Python.

## Decision

Add `ImplementationIssuePolicy::validate`: the repository policy, the base reference and the selection, in that order. Webhook intake and polling both call it, and `repository_key` replaces their two copies of the `owner/repository` key. Polling previously checked only the selection, so a policy with, for example, `repository_id` 0 could admit polled work. Polling now refuses an invalid repository policy or base before any provider call, as webhook intake already did; this is an intended behaviour change.

`check_poll_policy` runs that validation, takes the first enrolled actor that passes poll's own record actor checks (non-empty, bounded, no control characters), builds one synthetic open issue, and passes it through poll's `validate_record`, `record_identity` and `issue_work`, then `WorkLedger::default().admit` with the same enrollment. Selection is not consulted for the probe, because selection decides which issues become work, not whether derived work is admissible. An enrollment with no such actor is refused: polling could admit no issue. An invalid extra actor does not reject a policy polling would still use. `kiln-implementer policy-check` reads the two files with the same `read_json` and types as `poll` and prints `{"policy":"admitted","repository":...}`; it needs no database, credential or transport.

`read_json` classifies the serde error. Syntax and end-of-input errors report "is not valid JSON" with serde's fixed message, which never quotes input. Data errors report "does not match its schema". Missing and duplicate field messages are kept because serde takes those names from the target type. Messages that quote input (an unknown member name, an invalid type, value or length, an unknown variant) are reduced to their kind and serde's trailing expectation, which also comes from the type: "unknown field (name withheld), expected `issues` or `labels`". Custom validation messages are dropped entirely. Every message ends with the line and column.

## Verification

Core tests in `poll.rs` show that the preflight admits a policy polling admits and that each policy-level refusal yields the same code as polling. They also show that ledger refusals (budget, unenrolled repository) and a missing actor fail the preflight. A CLI test runs `policy-check` on a valid policy and on the pilot shape without `base.contract_version`. Binary unit tests pin the schema, malformed and redaction messages. Gates: rustfmt, Clippy with warnings denied on the GitHub adapter, the focused tests and `cairn validate`.

## Risks and non-claims

The probe proves the policy-derived work fields are admissible, not that any real issue will be selected or that GitHub credentials, transport or rate limits work. The preflight does not check onix-core's deployment rules (terminal policy, budget limits, work-class binding); those stay in the deployment's own check. Other Kiln binaries with their own masked `read_json` copies are not changed here.
