# Implementation service runbook

This runbook composes the local implementation pipeline. It does not qualify a
live Lattice worker, GitHub deployment or Cairn lifecycle effect.

## 1. Intake

Admit GitHub issues through the authenticated webhook/queue or one polling pass:

```console
kiln-implementer poll \
  --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --policy /etc/kiln/github-implementation.json \
  --secret-file /run/secrets/github-token \
  --max-pages 5
```

Admit a Cairn implementation-ready snapshot with `kiln-cairn-intake`:

```console
nix run .#kiln-cairn-intake -- \
  --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --snapshot /var/lib/cairn/implementation-ready.json \
  --policy /etc/kiln/cairn-implementation.json \
  --sequence 1
```

Policies must carry the exact resolved base when this work will be dispatched.
Polling and snapshot replay are deduplicated by the work ledger; intake never
dispatches.

## 2. Claim and reserve

```console
kiln-implementer claim --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json > /var/lib/kiln/attempt.json

kiln-implementer reserve-plan --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --plan-file /etc/kiln/dispatch-plan.json

kiln-implementer start --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --attempt ATTEMPT_ID --fence FENCE \
  --workflow-kind run --workflow-id RUN_ID
```

`start` records an externally observed Lattice workflow. For an ambiguous
dispatch, use `reconcile` with the same workflow identity; do not submit a
second execution.

## 3. Candidate handoff and verification

The worker/provider writes the host-derived candidate handoff and the trusted
shell writes retained canonical CI evidence:

```console
kiln-implementer handoff --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --handoff-file /var/lib/kiln/candidate-handoff.json

kiln-implementer verify --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --verification-file /var/lib/kiln/verification.json
```

Verification accepts only terminal, successful, clean, full, trusted CI facts
for the exact candidate and all-passing checks. It does not run CI.

## 4. Publication and landing

```console
kiln-implementer prepare-publication --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --publication-file /etc/kiln/publication.json

kiln-implementer publish --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --secret-file /run/secrets/github-token \
  --publication-file /etc/kiln/publication.json

kiln-implementer land --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --secret-file /run/secrets/github-token \
  --publication-file /etc/kiln/publication.json \
  --landing-plan /etc/kiln/landing.json
```

`publish` reuses an exact branch/PR and never force-pushes or merges. `land`
re-admits exact checks, approvals and base-head evidence, performs one guarded
merge commit, and records the read-back integration. A rerun recovers an
already merged result without a second merge.

## 5. Source reconciliation

```console
kiln-implementer close-source --database /var/lib/kiln/work.sqlite \
  --enrollment /etc/kiln/enrollment.json \
  --secret-file /run/secrets/github-token \
  --source-plan /etc/kiln/source-close.json
```

Only an integrated, exact GitHub issue may be closed. Already closed or
source-reconciled work is idempotent; no issue comments or Cairn lifecycle
operations are performed. Use `source-status` with the same plan for a read-only
comparison of retained and observed issue state.

## Boundaries

A verified attempt cannot be reclaimed; a changed source version is separately
claimable. Publication, landing, integration, source reconciliation and Cairn
lifecycle completion remain distinct. Live Lattice dispatch, verifier
production, GitHub deployment/qualification and Cairn lifecycle operations are
external prerequisites.
