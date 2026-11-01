## Why

Kiln retains CI runs and repository history but offers no subscribable
projection. Operators also need a bounded way to follow external repository
RSS/Atom feeds without a separate reader service. The console should expose
Atom feeds for its own repositories and let operators subscribe to external
repository feeds under explicit network and retention bounds.

## What Changes

- Add an optional service feeds policy with a dedicated read-only feed token
  and explicit subscription, entry, byte, redirect and timeout bounds.
- Publish an Atom index and one Atom feed per canonical repository containing
  retained run outcomes.
- Add operator-managed external RSS/Atom subscriptions with add, refresh and
  remove operations, persisted entries and retained errors.
- Enforce HTTPS-only public destinations, DNS pinning, redirect
  re-admission, private-address rejection and manual refresh without proxy or
  credential substitution.
- Add a Feeds console page with copyable repository feed URLs and external
  feed entries.
- Add positive and negative unit, API and browser tests.

## Impact

- **Files**: `crates/kiln-web-service/src/feeds.rs`, profile/auth/engine/server
  wiring, `web.html`, `web.js`, docs, and Cairn evidence.
- **Testing**: pure URL/parser/Atom tests, engine feed tests, workspace tests,
  Clippy, rustfmt, Cairn validation.
- **Authority**: feeds are read-only projections. Subscriptions do not create CI
  state, repository authority, acceptance, or release evidence.
