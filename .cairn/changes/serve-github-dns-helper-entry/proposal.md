# Serve the GitHub DNS helper entry in every binary that needs it

## Why

The live #49 pilot could not make any GitHub API call from `kiln-implementer`. Every GitHub HTTPS client (status, issue polling, publication, landing and source status) resolves `api.github.com` by re-running the current executable as `<exe> --internal-github-status-dns <ppid>`. Only `kiln-adapter-github`'s `main` handled that entry. `kiln-implementer` rejected it as an unexpected argument, so `poll` failed with `github_poll_transport` and `publish` failed the same way. `kiln-hosted`, whose GitHub connections build status clients, had the same gap.

## What Changes

One library entry, `kiln_adapter_github::status::run_internal_resolver_if_requested`, MUST be the first call in `main` of every binary that can construct a GitHub HTTPS client: `kiln-adapter-github`, `kiln-implementer` and `kiln-hosted`. It serves the helper and exits when the process was started as one, and otherwise records that the executable serves the entry. Resolver construction MUST refuse with `github_status_dns_unserved` in a process that never made that call, so an executable that cannot serve the helper is never re-run as one. Helper failures are named on standard error with a fixed prefix.

## Impact

Kiln maintainers own the GitHub adapter's resolver and the three binaries' entry points. Binaries that construct no GitHub HTTPS client (`kiln-github-job`, `kiln-web-service`, `kiln-implementation-fixture`) are unchanged. The Gitea adapter keeps its own, already-dispatched helper entry. No wire format, schema or provider request changes.
