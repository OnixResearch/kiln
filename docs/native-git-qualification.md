# Native Git delivery qualification

Issue #63 adds one operator-driven, non-PR delivery profile. It is not a general Git publisher, merge queue, or blanket remote qualification claim. The runner accepts HTTPS and SSH with a strict host-key pin; local bare repositories are supported only by adapter test fixtures and are rejected by the qualification command. It never creates a pull request and never force-pushes.

## Preconditions

Before invoking the runner, the operator must independently establish all of the following:

- the request refers to exact current work, attempt, fence, candidate handoff, successful verification, review/check evidence, and the currently admitted policy;
- the candidate is a newly verified exact commit/tree for this attempt, and the selected target/expected old commit are the exact observed values;
- the profile's remote URL, push URL, transport and repository authority are correct; the local checkout has the exact candidate object;
- repository branch protection allows the intended non-force fast-forward update and the operator has authorized publication.

The runner rechecks Kiln's durable work, attempt, verification, policy and current review snapshot before creating each first-effect intent. It persists each intent before transport. Restarted or ambiguous effects are read-only reconciliation; an unknown result is never resent. A target that has advanced or differs from the expected old commit remains an explicit rejection/unknown outcome. Prepare a newly verified candidate against the newly observed target instead of reusing checks.

## Qualification request

Write a JSON file using schema `kiln.native-git-qualification-request.v1` with fields `database`, `repository_path`, `change_id`, `actor`, `work`, `attempt`, `fence`, `plan`, and `remote`. Supply the separately trusted enrollment, service profile, and review profile as command-line inputs; none may be selected by the request JSON. The review observer opens the existing private collaboration store read-only, resolves the current candidate/source against the configured Git provider, and reads canonical CI facts from the retained Kiln engine. Its profile, source, and CI identities are content-pinned and re-resolved before effect reservation and acknowledgement. No caller-supplied review-state or passing-check JSON is accepted.

The `remote` object contains `remote_name`, exact `remote_url`, and `transport` (`https` or `ssh_strict_host_key`). SSH also requires `ssh_host_key_sha256`, the lowercase 64-hex SHA-256 host-key fingerprint; the pin is included in remote identity and checked by libgit2's certificate callback on push and read-back. Local bare fixtures are adapter-test-only and are rejected by the qualification command. Embedded URL credentials are rejected. HTTPS credentials are obtained through the operator's configured Git credential helper; SSH credentials come from the operator's SSH agent. Credential callbacks are bound to the exact enrolled URL.

The operator CLI exposes `native-git qualify --request <path> --enrollment <path> --service-profile <path> --review-profile <path> --execute`. Without `--execute`, it refuses. The JSON result schema is `kiln.native-git-qualification-result.v1`; `confirmed` requires fresh remote read-back of both the exact candidate branch and target plus a retained native integration receipt. Pending, unknown, stale and rejected outcomes are not success.

## Cairn lifecycle receipt

The lifecycle request schema `kiln.native-git-lifecycle-request.v1` carries `database`, `actor`, `work`, `attempt`, `fence`, `change_id`, `receipt_path`, `action`, and the bound `remote` profile; it carries no lifecycle observation. Invoke `native-git lifecycle-receipt --request <path> --enrollment <path> --service-profile <path> --review-profile <path> --repository <path> --report <path>`. The consumer loads the accepted native receipt from durable state, opens the accepted Git tree, reads the native Cairn receipt and independently hash-checks the authoritative completion report, then revalidates both remote refs and current review/policy before recording. It is read-only with respect to source and remote and invents no issue/PR identity.

## Qualification boundary

Automated local bare-repository fixtures prove only the exact mechanics exercised by those fixtures. They do not qualify a production host, transport installation, server protection policy, DNS/TLS/SSH trust, credential setup, or real remote landing. Live qualification requires a separately authorized operator scope and observed remote evidence; this change has not executed a live remote effect.
