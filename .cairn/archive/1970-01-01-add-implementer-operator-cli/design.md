## Decisions

The binary lives in `kiln-adapter-github` because draining crosses the
authentication/normalization boundary and the durable ledger. It is a thin
operator shell over the library functions; no policy or transition logic is
duplicated.

The secret file is read as exact bytes. Operators must avoid adding a trailing
newline when the webhook secret does not contain one.

Only `drain` and `status` are exposed. A claim command would create a lease with
no dispatch route; that belongs with the Lattice implementation worker.

## Non-claims

The tool does not claim work, start a worker, verify a candidate or publish. It
reports and admits only.
