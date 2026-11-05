## Context

The existing guest returns bounded archives before destruction; the host publishes only after successful job completion and recorded destruction. Store publication must preserve old entries across interruption.

## Decisions

Use the existing exclusive cache owner lock. Keep stages in a dedicated marked staging directory; recover them on open. Verify the staged envelope from disk, sync it, rename with RENAME_NOREPLACE, then sync the key directory. Hooks are explicit caller-supplied phase callbacks, never environment-controlled production fault switches. Corrupt retained entries remain untouched and are measured misses. Remote storage remains unsupported.

## Risks

Filesystem fsync/atomic rename guarantees depend on local filesystem semantics. Tests establish integrity, not a proof or hostile-isolation qualification. Hardware power-loss evidence is not inferred from SIGKILL tests.
