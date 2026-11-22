## Context

`BoundedGithubResolver::current_executable` stores `std::env::current_exe()` with the reserved argument and the caller's PID. `resolve` runs it under Bounded Exec with a cleared environment, a deadline and output bounds, and parses `ip:443` lines. The child side, `run_dns_child`, was public and dispatched by hand only in `kiln-adapter-github`'s `main`. `kiln-implementer` builds poll, publication, landing and source-status clients; `kiln-hosted` builds status clients through `server::runtime` in its connection tick. Neither dispatched the entry.

## Decision

`run_internal_resolver_if_requested` inspects the raw arguments before any CLI parser. With exactly the reserved argument and a numeric PID it runs the helper and exits 0 or `DNS_CHILD_FAILURE_EXIT` (1). A malformed entry exits `DNS_CHILD_USAGE_EXIT` (2). Every failure writes one line starting with `DNS_CHILD_FAILURE` to standard error, within the parent's 1024-byte bound. Otherwise it sets a process-wide flag and returns. `run_dns_child` becomes private, so the dispatch has one implementation.

`current_executable` refuses with `github_status_dns_unserved` while the flag is unset. A binary that forgets the call fails at client construction with a named error instead of re-running itself into its own argument parser. `kiln-hosted`'s `#[tokio::main]` becomes a plain `main` that dispatches the entry before building the same multi-thread runtime, so the helper never starts a runtime.

Binaries that construct a GitHub HTTPS client were found by tracing every `BoundedGithubResolver::current_executable` call (status, poll, publication, landing and source-status clients) to its constructors and their callers. The callers are `kiln-implementer` (poll, publish, recover-integration, close-source, source-status), `server::runtime` and `server::serve`, which `kiln-adapter-github` and `kiln-hosted` use. `kiln-github-job`, the web-service binaries and the examples construct none.

## Verification

An integration test runs each of the three real binaries with the reserved entry and the test's own PID. It accepts either `ip:443` addresses or the helper's own resolution failure, so it holds offline. It also checks malformed entries and a non-parent PID. Before the fix, `kiln-implementer` exited 2 with "unexpected argument", which the test rejects. A unit test shows that the libtest process, which never dispatches the entry, refuses to construct the resolver and the poll and status clients.

## Risks and non-claims

A future binary that builds a GitHub client must still call the entry; the construction-time refusal makes the omission fail with a named error rather than silently. The test does not prove GitHub reachability, credentials or rate limits.
