# Check the GitHub intake policy exactly as polling does

## Why

The live #49 pilot passed its `policy-check` stage and then failed at `poll`. The onix-core pilot policy's `base` lacked `contract_version`, which Kiln's `ExternalRef` requires. Two Kiln defects hid the cause. The implementer's JSON reader reported every deserialization failure as "<path> is not valid JSON", and no Kiln command admitted the intake policy with the type and validation `poll` uses, so the deployment preflight checked a hand-written subset of keys and could not disagree with `poll` less than by accident.

## What Changes

`kiln-implementer policy-check --policy PATH --enrollment PATH` MUST read both files with the same types and reader `poll` uses, apply the policy validation every intake pass applies, and admit one synthetic open issue through poll's record bounds, work construction and ledger admission in an empty in-memory ledger. It opens no database, reads no credential and contacts no provider. Webhook intake and polling MUST share that one policy validation; polling now also refuses an invalid repository policy or base reference before any provider call, as webhook intake already did.

Implementer JSON input failures MUST name the file, distinguish malformed JSON from a schema mismatch, and give the line and column. A schema mismatch MUST name a missing or duplicate field and MUST NOT echo input, including an unknown member name.

## Impact

Kiln maintainers own the GitHub adapter's policy validation, the implementer binary and the operator runbook. Deployment preflights should call `policy-check`; onix-core's `qualification.py policy` switches to it when onix-core repins to a Kiln that contains the command. No wire format, schema, ledger state or provider call changes. The other binaries that keep their own copy of the masked reader (`kiln-cairn-intake`, `kiln-lattice-dispatch`) are unchanged.
