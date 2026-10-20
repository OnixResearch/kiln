let shared = import "_shared-import.ncl" in
{
  cohort = shared.cohort,
  prohibited = ["ambient", "floating"],
}
