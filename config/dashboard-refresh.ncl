# r[impl kiln.dashboard.live.collection]
# Export this closed contract to JSON; paths are explicit producer-local paths.
# Every immediate root entry consumes a candidate slot, including ignored names.
# mtime ordering is observed update order, never newest-source completeness.
let MaximumCandidates = 1024 in
let MaximumHistories = 128 in
let MaximumJournalBytes = 16777216 in
let MaximumTotalJournalBytes = 268435456 in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Integer = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 4096
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
    && !std.string.is_match "(/$|//|/\\.(/|$)|/\\.\\.(/|$))" value
  )
in
let Label =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && std.string.is_match "[^[:space:]]" value
    && !std.string.is_match "[\\x00-\\x1f\\x7f-\\x9f]" value
  )
in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9._-]+$" value
  )
in
let Configuration = {
  schema_version | Exact 1,
  candidate_root | AbsolutePath,
  catalog_output | AbsolutePath,
  publication_config | AbsolutePath,
  project_id | Identifier,
  project_label | Label,
  max_candidates | Integer MaximumCandidates,
  max_histories | Integer MaximumHistories,
  max_journal_bytes | Integer MaximumJournalBytes,
}
in
let Bounds =
  std.contract.from_predicate (fun value =>
    value.max_histories <= value.max_candidates
    && value.max_candidates
    * value.max_journal_bytes <= MaximumTotalJournalBytes
  )
in
fun settings => settings | Configuration | Bounds
