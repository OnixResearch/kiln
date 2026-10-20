# Verification Evidence

## Live defect reproduction

On 2026-08-28, production shadow operation `65c700f8886f9112db18c661bfc85a5b15005ef2562db1f69e01bb3ba464d809` dispatched one real Lattice workflow. The host exhausted 1,023 unpaced observations in 286 milliseconds and retained the operation as `Unknown`.

The provider completed after the host stopped polling. It published run `b3:abfea36e3697f9eff3a0c1758ca441291409db30b0d940f8c478b482785b0694` with mode `0640` and failure exit code `1`. The report exposed a separate missing-Git wrapper dependency, so it is not cutover evidence.

- report BLAKE3: `58f32f254cc8af1f37f8072db41a2068c196353f8bfe82093604d39dc5687a5b`;
- log BLAKE3: `0f8a28ffcd7e12c46298bf096a84f0b945417aa3b2520145f1ef46354f641e63`.

No production cutover occurred.

## Baseline

Before the core change:

- default `kiln-aspen-host` tests: PASS;
- all-feature `kiln-aspen-host` tests: PASS, including four hosted positive and negative fixtures.

Cargo prints the intentional malformed Aspen fixture diagnostic before each successful run.

## Changed implementation

The host now requires `--poll-interval-ms`. It rejects missing, duplicate, zero, and malformed values. Normal dispatch and `RecoverDispatch` wait only after a non-terminal observation. The existing finite count still bounds every loop.

Verification completed:

- focused delayed-terminal and exhaustion tests: PASS;
- workspace, all-target, all-feature host tests: PASS;
- strict all-target, all-feature Clippy with `-D warnings`: PASS;
- strict Cairn validation before and after implementation: PASS;
- Nix host package check: PASS;
- Nix Octet deny-all check: PASS with zero findings, warnings, or errors.

The first Nix Octet run rejected a six-parameter constructor. The corrected implementation uses `LatticeEffectPortOptions`; no lint allowance was added. A later run rejected system-clock reads in tests and an oversized test file. The final design injects an `ObservationPacer`, uses a deterministic counting adapter, and keeps support code in a narrow module. The official Nix deny-all gate is clean.

## Fresh status-disabled Onix shadow (2026-10-20)

The isolated host used `--poll-interval-ms 7068` with the Radicle profile's finite maximum of `1,023` polls. It reached the real Lattice endpoint and Nix provider under the deployed non-default runtime profile. The exact executable/profile identities, trigger, terminal acknowledgement, retained report/log identities, and isolation boundary are recorded in the [runtime-profile shadow evidence](../../1970-01-01-bind-aspen-extension-runtime-profile/evidence/verification.md#fresh-status-disabled-onix-shadow-2026-10-20).

Operation `60fe022c13e076c8309ec4b0f334165b2206c7ece1674612e89a309ecd67dd5d` persisted Lattice execution `b3:87b8bf217964f7c0e9de486088f6a2e4e876205c2fe8b8ce3d95c9e116c44de8`, sequence `2`, terminal `Failed`, outcome `DomainFailure`, and returned a matching terminal `Failed` acknowledgement. The provider published report `b3:3d0168547a6bfd25f02dfe96635e4433587daa6969b79a4859bf76ddefb7dba1` with failure exit code `1`; its log identifies a Seaglass fixed-output hash mismatch, not an observation timeout.

Unlike the original `286`-millisecond poll exhaustion, this run retained the real provider's terminal failure rather than premature `Unknown`. It supports terminal-failure observation through the paced host, not successful CI, production cutover, source trust, exactly-once execution, or release eligibility.

The exercised host and extension package are bound to already published immutable Kiln revision `8c9338e5c10a0e16ee3042d11583ccccf6efe7e9`. Its source NAR and both evaluated derivation paths exactly match the frozen inputs. A fresh, credential-isolated HTTPS source acquisition reproduced that source identity. The [published source binding](../../1970-01-01-bind-aspen-extension-runtime-profile/evidence/verification.md#published-source-binding) records the acquisition and derivations; this is not a claim about a newer source revision or production readiness.

### Successful delayed provider completion

The distinct second candidate used Seaglass revision `b22335f843fe2bae0d3951ddc7aabaf008df63f4`. Operation `c06fdaf09af704c42a7ccbcaf54a1320ea327a8c14b6696d488379be2921f71f` persisted Lattice execution `b3:a42fb21e42ffd0e7ad6e780c8ee8a2622f5cd61efb73f9bd8bc4ef3cfeb937e3`, sequence `2`, terminal `Completed`, successful `true`, outcome `Success`, and returned a matching terminal `Completed` acknowledgement.

The real provider published successful report `b3:0e3eb707f64cdf0f8deebfc3e4a0570f32a272e411445590d902ab5052d48084`, exit code `0`. The captured client invocation lasted `4,157.81` seconds, rather than exhausting the poll budget prematurely. No exact observe-call count is claimed. The [shared shadow evidence](../../1970-01-01-bind-aspen-extension-runtime-profile/evidence/verification.md#completed-success-observation) records the exact trigger, receipt, profile/executable identities, report/log hashes, and retained raw records for both outcomes.

The isolated host was stopped and removed after both terminal observations. Strict Cairn validation and traceability coverage passed; historical build/test gates were not rerun by this continuation. The exact published-source binding and the successful delayed real-provider result satisfy the outstanding live criterion for this exercised cohort, not production cutover or release eligibility.
