# r[impl kiln.radicle_ci.runtime] r[impl kiln.radicle_ci.status]
# Production status authority is independent of source/trigger authority.
let Status = (import "radicle-status-profile.ncl").profile in
let Aspen = import "aspen-runtime-profile.ncl" in
let NonEmpty = std.contract.from_predicate (fun v => std.is_string v && std.string.length v > 0) in
let Absolute = std.contract.from_predicate (fun v => std.is_string v && std.string.is_match "^/[^\\x00]+$" v) in
let Blake3 = std.contract.from_predicate (fun v => std.is_string v && std.string.is_match "^[0-9a-f]{64}$" v) in
let Positive = std.contract.from_predicate (fun v => std.is_number v && v > 0 && std.number.is_integer v) in
let Rids = std.contract.from_predicate (fun v => std.is_array v && std.array.length v > 0 && std.array.all (fun rid => std.is_string rid && std.string.is_match "^rad:z[1-9A-HJ-NP-Za-km-z]+$" rid) v) in
let Ref = { contract_version | Number, provider_namespace | NonEmpty, native_kind | NonEmpty, native_id | NonEmpty } in
let MaximumDocumentBytes = 4194304 in
let MaximumJournalRecords = 1024 in
let MaximumPolls = 1000 in
let PollIntervalMilliseconds = 250 in
let TransportTimeoutSeconds = 30 in
fun inputs =>
  let selected
    | {
      worker_profile_path | Absolute,
      worker_profile_identity | Blake3,
      socket_path | Absolute,
      status_storage_root | Absolute,
      status
        | {
          allowed_rids | Rids,
          signer_identity | Ref,
          signer
            | {
              project_root | Absolute,
              project | NonEmpty,
              profile | NonEmpty,
              secret_key | NonEmpty,
              provider | NonEmpty,
            },
        },
    }
    = inputs
  in
  {
    schema = "kiln.radicle-ci.profile.v1",
    worker_profile_path = selected.worker_profile_path,
    worker_profile_identity = selected.worker_profile_identity,
    runtime_profile = Aspen,
    socket_path = selected.socket_path,
    maximum_frame_bytes | Positive = MaximumDocumentBytes,
    transport_timeout_seconds | Positive = TransportTimeoutSeconds,
    max_journal_bytes | Positive = MaximumDocumentBytes,
    max_journal_records | Positive = MaximumJournalRecords,
    maximum_polls | Positive = MaximumPolls,
    poll_interval_milliseconds | Positive = PollIntervalMilliseconds,
    status_storage_root = selected.status_storage_root,
    status_profile = {
      id = Status.id,
      platform = Status.cohort.platform,
      radicle_cohort = "radicle-cob-0.21.1-crypto-0.19.0-oid-0.2.2-ref-format-0.2.0-git2-0.21.0",
      radicle_cob_version = Status.cohort.radicle_cob_version,
      radicle_cob_source_revision = Status.cohort.radicle_cob_source_revision,
      radicle_crypto_version = Status.cohort.radicle_crypto_version,
      radicle_crypto_source_revision = Status.cohort.radicle_crypto_source_revision,
      radicle_oid_version = Status.cohort.radicle_oid_version,
      radicle_oid_source_revision = Status.cohort.radicle_oid_source_revision,
      radicle_ref_format_version = Status.cohort.radicle_ref_format_version,
      radicle_ref_format_source_revision = Status.cohort.radicle_ref_format_source_revision,
      git2_version = Status.cohort.git2_version,
      git2_source_revision = Status.cohort.git2_source_revision,
      secretspec_version = Status.signer.secretspec_version,
      secretspec_source_revision = Status.signer.secretspec_source_revision,
      secretspec_nixpkgs_revision = Status.signer.nixpkgs_revision,
      secretspec_license = Status.signer.secretspec_license,
      radicle_license = Status.cohort.radicle_license,
      allowed_rids = selected.status.allowed_rids,
      allowed_object_type = Status.allowed_object_type,
      signer_identity = selected.status.signer_identity,
      signer = selected.status.signer,
      executable = {
        path = Status.signer.executable_path,
        blake3 = Status.signer.executable_blake3,
        version = Status.signer.secretspec_version,
      },
      max_payload_bytes = Status.limits.max_payload_bytes,
      max_objects_scanned = Status.limits.max_objects_scanned,
      allow_network_peers = false,
      allow_merge_authority = false,
      allow_canonical_state_mutation = false,
      non_claims = Status.non_claims,
    },
  }
