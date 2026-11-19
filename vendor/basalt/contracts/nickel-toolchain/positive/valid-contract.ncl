let Policy = {
  schema | String,
  fail_closed | Bool,
} in
({ schema = "basalt.nickel-toolchain.fixture.v1", fail_closed = true } | Policy)
