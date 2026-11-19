## Why

Kiln's implementation workflow has work-scope checks, provider capability
advertisement, authenticated transport and historical approval records, but no
single current-authority contract binding an admitted work/attempt to every
protected command and provider effect. None of those facts is an authorization
grant.

Basalt now exposes the reviewed Kiln operation adapter
(`add-kiln-authority-adapter`, Basalt commit `cb16606`). Kiln must consume it at
its own boundaries: bind each protected operation to its enforcement owner, mint
live authority only from verified ingress, and enforce it before credentials or
effects. Kiln #69 owns request binding and enforcement call sites; Kiln #70 owns
durable grant lineage, attenuation, revocation and recovery.

## What Changes

- Add a closed protected-operation coverage map in `kiln-core` naming, for every
  protected path in the selected implementation profile, its Basalt operation,
  enforcement owner, effect class and candidate binding.
- Adopt one UCAN cohort: upgrade `vendor/ucan` from `d46816f` to Basalt's
  vendored `417f746` (the `c483c7b` drain tip is an ancestor) so observation and
  authority use one crate.
- Import Basalt through a repo-owned script into `vendor/basalt` with its nested
  UCAN copy omitted and the UCAN path dependencies rewritten to Kiln's single
  vendored snapshot; record the source revision and transform.
- Add a narrow versioned authorization port and Basalt adapter at the application
  composition boundary. The pure core keeps domain decisions over admitted facts.
- Require verified live authority for claim acquire/renew, dispatch, candidate
  submission, verification admission, cancellation, publication, integration and
  lifecycle completion, composed with existing work/fence/resource admission.
- Keep execution authority separate from verification, publication, integration
  and lifecycle authority. A caller-supplied boolean, role, receipt or provider
  capability cannot substitute for current authority.
- Preserve an explicit local-operator compatibility profile with recorded
  provenance and no widening; projection-enabled routes require an operator
  capability rather than a role-only bypass.
- Retain redacted decision references and reason codes only; no capability
  material, keys or sensitive payloads in logs or projections.

## Impact

- **Core**: New `implementation::authority` coverage module and typed port.
- **Adapters**: New `kiln-adapter-basalt` crate over `basalt::kiln`.
- **Vendored source**: UCAN snapshot upgrade and a recorded Basalt import.
- **Shells**: `kiln-implementer` admission, publication effects and the operator
  CLI gain authority checks before protected effects.
- **Licensing**: `LICENSES.md` records the linked AGPL-3.0-or-later Basalt source.
- **Lifecycle**: Kiln #69 is the owning issue; #70 extends the same vocabulary
  with attenuation, currentness and recovery lineages.
