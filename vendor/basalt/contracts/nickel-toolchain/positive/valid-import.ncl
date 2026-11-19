let shared = import "_shared-import.ncl" in
{
  evaluator = shared.evaluator,
  source_revision = shared.source_revision,
  imported = true,
}
