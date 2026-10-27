# Separate reviewed automation policy; never part of the canonical ServiceProfile.
# Export this builder as JSON and pass it explicitly with --automation PATH.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Reference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 512
    && std.string.is_match "^refs/heads/[^ ~^:?*\\\\]+$" value
  )
in
fun input =>
  let settings =
    input
      | {
        triggers
          | Array {
            id | Identifier,
            project_id | Identifier,
            reference | Reference,
            repository
              | {
                owner | c.NonEmpty,
                repository | c.NonEmpty,
                repository_id | c.Positive,
                allow_fork_pull_requests | std.contract.from_predicate (fun value => value == false),
              },
            secret_file | c.AbsolutePath,
            enabled | Bool,
            push | Bool,
            pull_request | Bool,
            pull_request_base | c.Nullable String,
          },
        schedules
          | Array {
            id | Identifier,
            project_id | Identifier,
            reference | Reference,
            enabled | Bool,
            interval_ms | c.BoundedInteger 1000 31536000000,
            min_interval_ms | c.BoundedInteger 1000 31536000000,
            max_interval_ms | c.BoundedInteger 1000 31536000000,
            next_due_ms | c.Nullable (c.BoundedInteger 0 9007199254740991),
          },
        notifications
          | Array {
            id | Identifier,
            project_id | Identifier,
            enabled | Bool,
            on_success | Bool,
            on_failure | Bool,
            host | c.NonEmpty,
            port | c.BoundedInteger 1 65535,
            path | c.NonEmpty,
            addresses | Array String,
            bearer_secret_file | c.Nullable c.AbsolutePath,
            ca_certificate_file | c.Nullable c.AbsolutePath,
            allow_loopback | Bool,
            allow_private | Bool,
            timeout_ms | c.BoundedInteger 1 30000,
            max_request_bytes | c.BoundedInteger 1 1048576,
            max_response_bytes | c.BoundedInteger 1 1048576,
            max_resolver_candidates | c.BoundedInteger 1 16,
          },
        max_records | c.BoundedInteger 1 100000,
        status_limit | c.BoundedInteger 1 100,
        max_event_bytes | c.BoundedInteger 1 1048576,
        max_tick_effects | c.BoundedInteger 1 16,
      }
  in
  {
    schema = "kiln.web-automation.v1",
    triggers = settings.triggers,
    schedules = settings.schedules,
    notifications = settings.notifications,
    max_records = settings.max_records,
    status_limit = settings.status_limit,
    max_event_bytes = settings.max_event_bytes,
    max_tick_effects = settings.max_tick_effects,
  }
