# r[impl kiln.admission.host_owner_profile]
# Typed registration of one explicitly reviewed host-local resource owner.
# Exporting it grants nothing. The operator applies the export with
# `kiln-implementer host-owner-admit|observe|reserve|release`; Kiln re-admits it
# with the same bounds. Capacity is the host's declared registration only: it
# never describes remote, provider or serving capacity. Bump `generation`
# whenever `capacity` changes so older observations and grants become stale.
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun x =>
    std.is_number x && x >= minimum && x <= maximum && x == std.number.truncate x
  )
in
let Token =
  std.contract.from_predicate (fun x =>
    std.is_string x
    && std.string.length x >= 1
    && std.string.length x <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" x
  )
in
let Blake3 =
  std.contract.from_predicate (fun x =>
    std.is_string x && std.string.is_match "^[0-9a-f]{64}$" x
  )
in
let Repository =
  std.contract.from_predicate (fun x =>
    std.is_string x
    && std.string.length x <= 256
    && std.string.is_match "^[a-z0-9_.-]+/[a-z0-9_.-]+$" x
  )
in
let Amounts =
  std.contract.from_predicate (fun x =>
    std.is_record x
    && std.array.length (std.record.fields x) >= 1
    && std.array.length (std.record.fields x) <= 16
    && std.array.all
      (fun key =>
        std.string.is_match "^[A-Za-z0-9_.-]{1,128}$" key
        && std.is_number (std.record.get key x)
        && std.record.get key x > 0
        && std.record.get key x <= 9007199254740991
        && std.record.get key x == std.number.truncate (std.record.get key x)
      )
      (std.record.fields x)
  )
in
let Millis = Integer 1 9007199254740991 in
let Class = {
  name | Token,
  project | Token,
  verification_profile | Blake3,
}
in
let Admitted = {
  repository | Repository,
  classes | Array Class | std.contract.from_predicate (fun x => std.array.length x >= 1 && std.array.length x <= 16),
}
in
let unique = fun keys =>
  std.array.length (
    std.array.fold_left (fun seen k => if std.array.elem k seen then seen else seen @ [k]) [] keys
  ) == std.array.length keys
in
fun input =>
  let s =
    input
      | {
        provider | Token,
        host | Token,
        generation | Integer 1 9007199254740991,
        capacity | Amounts,
        observation_ttl_ms | Millis,
        reservation_ttl_ms | Millis,
        campaign | Token,
        tenant | Token,
        deadline_ms | Millis,
        quota | Amounts,
        per_work | Amounts,
        limits
          | {
            max_items | Integer 1 4096,
            max_bytes | Integer 1 16777216,
            max_wait_ms | Millis,
            max_observation_age_ms | Millis,
            max_retry_after_ms | Millis,
            max_children | Integer 1 4096,
            max_active | Integer 1 4096,
          },
        repositories | Array Admitted | std.contract.from_predicate (fun x => std.array.length x >= 1 && std.array.length x <= 4096),
      }
  in
  let dimensions = std.record.fields s.per_work in
  let ok =
    s.observation_ttl_ms <= s.limits.max_observation_age_ms
    && s.limits.max_active <= s.limits.max_items
    && std.record.fields s.quota == dimensions
    && std.array.all
      (fun d =>
        std.record.has_field d s.capacity
        && std.record.get d s.per_work <= std.record.get d s.capacity
        && std.record.get d s.per_work <= std.record.get d s.quota
      )
      dimensions
    && unique (std.array.map (fun r => r.repository) s.repositories)
    && std.array.all
      (fun r =>
        unique (std.array.map (fun c => c.name) r.classes)
        && unique (std.array.map (fun c => c.project ++ ":" ++ c.verification_profile) r.classes)
      )
      s.repositories
  in
  # Export forces every field, so an invalid registration cannot produce output.
  let checked =
    if ok then
      s
    else
      std.fail_with "host owner registration exceeds capacity, quota, window or uniqueness bounds"
  in
  {
    version = 1,
    owner = {
      provider = checked.provider,
      host = checked.host,
      generation = checked.generation,
      capacity = checked.capacity,
      observation_ttl_ms = checked.observation_ttl_ms,
      reservation_ttl_ms = checked.reservation_ttl_ms,
    },
    campaign = checked.campaign,
    tenant = checked.tenant,
    deadline_ms = checked.deadline_ms,
    quota = checked.quota,
    per_work = checked.per_work,
    limits = checked.limits,
    repositories = checked.repositories,
  }
