## Decisions

The manifest is admitted against a caller-supplied expectation because the
volume provider owns current disk generation facts; the handoff module does not
infer them. Source and target placements, session identity, operation and
execution are overwritten from the retained session and handoff intent before
admission so one stale field cannot widen the record.

Object identities must be distinct. Disk identity and generation must match the
independently resolved volume facts. The ordering state machine keeps migration
intent, source pause, object verification, target admission, resume and source
retirement distinct; reconciliation preserves capacity and cannot claim a
target.

## Non-claims

No bytes are copied or verified, no manifest is durably published, no target
placement is committed, and no VMM is paused, resumed or retired by this module.
Network and Iroh transfer remain separate work.
