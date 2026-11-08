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
    && std.string.is_match "^refs/(heads|tags)/[^ ~^:?*\\\\]+$" value
  )
in
# Trigger filter globs: `*` and `?` within one `/` segment and whole-segment
# `**`; no negation, classes or alternation. Rust admission is authoritative.
let FilterPattern =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value >= 1
    && std.string.length value <= 256
    && !(std.string.is_match "^!|[\\[\\]{}+\\\\]|(^|/)\\.{0,2}(/|$)|[^/]\\*\\*|\\*\\*[^/]" value)
  )
in
let FilterPatterns =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= 32
  )
in
let BranchReference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 512
    && std.string.is_match "^refs/heads/[^ ~^:?*\\\\]+$" value
  )
in
let ScheduleAdmission =
  std.contract.from_predicate (fun schedules =>
    std.array.all
      (fun value =>
        value.min_interval_ms <= value.max_interval_ms
        && (
          if std.record.has_field "interval_ms" value then
            value.interval_ms >= value.min_interval_ms
            && value.interval_ms <= value.max_interval_ms
          else
            true
        )
        && (
          if value.enabled && std.record.has_field "interval_ms" value then
            value.next_due_ms != null
          else
            true
        )
      )
      schedules
  )
in
let PullRequestType =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["opened", "reopened", "synchronize", "closed", "merged"]
  )
in
# Optional per-trigger filter. Tags apply only to tag-bound pushes; Rust
# admission enforces the bound-reference namespace.
let TriggerFilter = {
  branches | FilterPatterns | Array FilterPattern | optional,
  tags | FilterPatterns | Array FilterPattern | optional,
  types | Array PullRequestType | optional,
  paths | FilterPatterns | Array FilterPattern | optional,
}
in
let Cron = {
  expression | c.NonEmpty,
  timezone | c.NonEmpty | default = "UTC",
}
in
# Exactly one cadence: an interval, or a cron calendar whose next occurrence
# is computed (next_due_ms stays null).
let Cadences =
  std.contract.from_predicate (fun schedules =>
    std.array.all
      (fun value =>
        if std.record.has_field "cron" value then
          !(std.record.has_field "interval_ms" value) && value.next_due_ms == null
        else
          std.record.has_field "interval_ms" value
      )
      schedules
  )
in
# r[impl kiln.untrusted.policy] Reviewed untrusted pull-request policy. `deny`
# (also when omitted) refuses forks exactly as before; `approve` holds untrusted
# pull requests in a blocked run until an operator approves the exact commit.
let UntrustedPolicy =
  std.contract.from_predicate (fun value => std.array.elem value ["deny", "approve"])
in
# Untrusted results may count for required CI only under `approve`.
let UntrustedCounting =
  std.contract.from_predicate (fun triggers =>
    std.array.all
      (fun trigger =>
        !(
          std.record.has_field "untrusted_results_satisfy_required_ci" trigger
          && trigger.untrusted_results_satisfy_required_ci
        )
        || (
          std.record.has_field "untrusted_pull_requests" trigger.repository
          && trigger.repository.untrusted_pull_requests == "approve"
        )
      )
      triggers
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
                # Retired: readable, and only `false` is admitted.
                allow_fork_pull_requests | std.contract.from_predicate (fun value => value == false),
                untrusted_pull_requests | UntrustedPolicy | optional,
              },
            secret_file | c.AbsolutePath,
            enabled | Bool,
            push | Bool,
            pull_request | Bool,
            pull_request_base | c.Nullable String,
            supersede_queued | Bool | optional,
            untrusted_results_satisfy_required_ci | Bool | optional,
            filter | TriggerFilter | optional,
          }
          | UntrustedCounting,
        schedules
          | Array {
            id | Identifier,
            project_id | Identifier,
            reference | BranchReference,
            enabled | Bool,
            interval_ms | c.BoundedInteger 1000 31536000000 | optional,
            cron | Cron | optional,
            min_interval_ms | c.BoundedInteger 1000 31536000000,
            max_interval_ms | c.BoundedInteger 1000 31536000000,
            next_due_ms | c.Nullable (c.BoundedInteger 0 9007199254740991),
          }
          | Cadences
          | ScheduleAdmission,
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
