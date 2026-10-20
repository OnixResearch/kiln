## Context

`verified_logic::ucan_invocation::check_invocation_authorized` proves invoker/audience equality, resource prefix refinement, and exact ability equality for a leaf capability. The shell authorization path also supports wildcard ability delegation and caveat policy evaluation, so blindly replacing shell authorization with the verified helper would reject valid wildcard delegations and bypass caveat-specific denial reporting.

## Decisions

1. Add a narrow bridge helper `invocation_request_is_authorized(...)` in `src/verified.rs`.
   - Rationale: keep byte-vector construction and verified-logic coupling in the bridge module.
   - Rejected: calling `ucan_invocation` directly from `src/token.rs`.

2. Invoke the helper only for concrete matched capabilities after `authorize_with_policies` has allowed the request.
   - Rationale: concrete capabilities match the verified primitive's exact-ability model; wildcard/caveat behavior remains under existing `VerifiedCapability` and policy logic.
   - Rejected: using the helper for wildcard matches because the primitive's ability equality would reject wildcard-delegated requests.

3. Treat token audience as invocation invoker.
   - Rationale: a UCAN delegates authority from issuer to audience; invocation admission should expose the audience as the exercising party.

## Risks

- The verified helper is a structural admission check only. It does not verify signatures, resolve DIDs, evaluate caveats, or fetch revocation state.
- If future verified-logic gains wildcard-aware invocation primitives, this slice can be expanded to cover wildcard matches too.
