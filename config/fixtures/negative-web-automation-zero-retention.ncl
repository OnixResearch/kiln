# This authoring fixture must fail Nickel's bounded retention contract.
let make = import "../web-automation.ncl" in
make {
  max_records = 0,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [],
  schedules = [],
  notifications = [],
}
