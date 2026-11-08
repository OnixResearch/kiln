# Apply to explicit operator-owned existing local roots, then export JSON.
# The runtime rejects symlinks, traversal, non-0700 roots, aliases and overwrite.
let c = import "github-contracts.ncl" in
let Text = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let UniqueRoots =
  std.contract.from_predicate (fun settings =>
    let roots =
      [settings.backup_root, settings.restore_root, settings.content_root]
      @ settings.replay_roots
    in
    std.array.length roots == std.array.length (
      std.array.fold_left
        (fun seen value => if std.array.elem value seen then seen else seen @ [value])
        []
        roots
    )
  )
in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let UniquePaths =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.elem value state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
  )
in
let ExactFields = (import "field-contracts.ncl").ExactFields in
fun input =>
  let settings =
    input
      | {
        service_id | Text 128,
        backup_root | c.AbsolutePath,
        restore_root | c.AbsolutePath,
        content_root | c.AbsolutePath,
        replay_roots | Array c.AbsolutePath | AtMost 128 | UniquePaths,
        maximum_database_bytes | c.BoundedInteger 1 17179869184,
        maximum_content_bytes | c.BoundedInteger 1 16777216,
        maximum_content_records | c.BoundedInteger 1 1000000,
        maximum_backup_ms | c.BoundedInteger 1 300000,
      }
      | UniqueRoots
      | std.contract.from_predicate (ExactFields ["service_id", "backup_root", "restore_root", "content_root", "replay_roots", "maximum_database_bytes", "maximum_content_bytes", "maximum_content_records", "maximum_backup_ms"])
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
