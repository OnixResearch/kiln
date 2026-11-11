## Decisions

Branch creation is reconciled by a GET before POST. A 404 creates; a 200 must
already point at the verified candidate; a 422/409 race re-reads instead of
blindly retrying. PR creation lists exact open head/base matches first and only
creates when zero matches, re-reading on a race.

The repository is represented as `owner/repository`; path/query branch values
are percent-encoded. HTTPS, redirects, proxy and response sizes are bounded like
the other GitHub clients. The shell records the branch before the PR so a crash
between effects can reconcile without redoing both.

## Non-claims

The publisher does not prove candidate correctness, integration or landing. It
does not force-push, update a branch, merge or alter source status. Remote
ambiguity remains unknown until a later read.
