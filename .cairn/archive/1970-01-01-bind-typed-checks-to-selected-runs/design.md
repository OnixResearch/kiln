# Context

`checklist_at` already chooses an exact full, clean, nonempty-ID run by project, applicable scope and greatest ordinal. Its typed-check predicate incorrectly searches all CI facts instead. Reuse the selected run rather than introducing another selection algorithm or policy namespace.

## Decision

`Policy.required_checks` applies to the review project. In a monorepo the selected required run for that project must discharge these checks; other affected projects continue to require their own ordinary CI but cannot satisfy a coincidentally named typed requirement. If no run for the review project is selected, a nonempty typed requirement remains unmet. Selection never filters failed runs away to revive an older pass. Typed admission separately requires terminal successful current counting evidence.

The checklist retains its existing `ci_run` shape and explains the actual typed-run identity or missing review-project run in the checks item. Exact candidate and current scope movement continue to invalidate selection. No changes to provider implementations or policy serialization are required.

## Verification

Cover older-pass/newer-missing, every nonpass outcome, wrong project/scope/repository/revision, dirty/partial/empty-ID, failed/nonterminal/noncounting untrusted facts, current eligible pass, explicit multi-project behavior and integration/scope drift. Use existing fallible Rust test conventions and preserve no_std. Run the focused core rail, strict lint/format, native lifecycle gates and immutable website package checks before activation.

## Non-claims

This predicate repair is not formal-proof verification, provider truth, merge authority or release qualification. Source publication and web activation remain separately recorded operator-authorized effects.
