{
  maximum_ttl_ms = 3000,
  maximum_clock_skew_ms = 100,
  workers = [
    {
      worker_id = "local",
      principal = "local:worker",
      project_id = "fixture",
      target_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      maximum_slots = 1,
    }
  ],
}
