# Apply to explicit operator-owned existing local roots, then export JSON.
# The runtime rejects symlinks, traversal, non-0700 roots, aliases and overwrite.
let c = import "github-contracts.ncl" in
fun input =>
  let settings =
    input
      | {
        service_id | c.NonEmpty,
        backup_root | c.AbsolutePath,
        restore_root | c.AbsolutePath,
        content_root | c.AbsolutePath,
        replay_roots | Array c.AbsolutePath,
        maximum_database_bytes | c.BoundedInteger 1 17179869184,
        maximum_content_bytes | c.BoundedInteger 1 16777216,
        maximum_content_records | c.BoundedInteger 1 1000000,
        maximum_backup_ms | c.BoundedInteger 1 300000,
      }
  in
  {
    service_id = settings.service_id,
    backup_root = settings.backup_root,
    restore_root = settings.restore_root,
    content_root = settings.content_root,
    replay_roots = settings.replay_roots,
    maximum_database_bytes = settings.maximum_database_bytes,
    maximum_content_bytes = settings.maximum_content_bytes,
    maximum_content_records = settings.maximum_content_records,
    maximum_backup_ms = settings.maximum_backup_ms,
  }
