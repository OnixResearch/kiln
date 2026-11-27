# Typed contract and default off-by-default configuration for projected source
# manifest persistence through Molten primary storage and an optional
# conditional-write S3 secondary tier (#80).
let MaxEnvelopeBytes = 16777216 in
let MaxObjectClosureEntries = 65536 in
let MaxEvictBatch = 1024 in
let MaxBrokerTicketTtlSecs = 3600 in
let MaxBrokerDownloadBytes = 16777216 in
let MaxBrokerScopePaths = 256 in
let MaxRevokedEntries = 4096 in
let MaxIdBytes = 128 in
let has_control = fun value => std.string.is_match "[\\x00-\\x1f\\x7f]" value in
let Exact = fun expected =>
  std.contract.from_predicate (fun value => value == expected)
in
let Text = fun max =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length (std.string.trim value) > 0
    && std.string.length value <= max
    && !(has_control value)
  )
in
let BoundedInt = fun min max =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && std.number.is_integer value
    && value >= min
    && value <= max
  )
in
let NonNegativeInt =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && std.number.is_integer value
    && value >= 0
  )
in
let MoltenPrimaryContract = {
  enabled | Exact true,
  port_id | Exact "molten.fabric.content-store",
  port_version | Exact "v1",
  verify_blake3_on_read | Exact true,
  max_envelope_bytes | BoundedInt 1 MaxEnvelopeBytes,
  max_object_closure_entries | BoundedInt 1 MaxObjectClosureEntries,
}
in
let S3SecondaryFields = {
  enabled | Bool,
  contract_version | Exact 1,
  endpoint_ref | String,
  bucket_ref | String,
  key_prefix | String,
  conditional_writes_required | Exact true,
  allow_bucket_listing | Exact false,
  allow_presigned_urls | Exact false,
  preserve_unknown_outcomes | Exact true,
  max_evict_batch | BoundedInt 1 MaxEvictBatch,
}
in
let S3SecondaryRules =
  std.contract.from_predicate (fun s3 =>
    if s3.enabled then
      std.string.length (std.string.trim s3.endpoint_ref) > 0
      && std.string.length s3.endpoint_ref <= MaxIdBytes
      && !(has_control s3.endpoint_ref)
      && std.string.length (std.string.trim s3.bucket_ref) > 0
      && std.string.length s3.bucket_ref <= MaxIdBytes
      && !(has_control s3.bucket_ref)
      && std.string.length s3.key_prefix <= MaxIdBytes
      && !(has_control s3.key_prefix)
    else
      s3.endpoint_ref == ""
      && s3.bucket_ref == ""
      && s3.key_prefix == ""
  )
in
let S3SecondaryContract = std.contract.Sequence [S3SecondaryFields, S3SecondaryRules] in
let EvictionWatermarksFields = {
  high_watermark_entries | BoundedInt 1 MaxObjectClosureEntries,
  low_watermark_entries | NonNegativeInt,
  high_watermark_bytes | BoundedInt 1 1099511627776,
  low_watermark_bytes | NonNegativeInt,
  require_secondary_before_primary_eviction | Bool,
  respect_retention_roots | Exact true,
}
in
let EvictionWatermarksRules =
  std.contract.from_predicate (fun w =>
    w.low_watermark_entries <= w.high_watermark_entries
    && w.low_watermark_bytes <= w.high_watermark_bytes
  )
in
let EvictionWatermarksContract =
  std.contract.Sequence [EvictionWatermarksFields, EvictionWatermarksRules]
in
let BrokerBoundsContract = {
  max_ticket_ttl_secs | BoundedInt 1 MaxBrokerTicketTtlSecs,
  max_download_bytes | BoundedInt 1 MaxBrokerDownloadBytes,
  max_scope_paths | BoundedInt 1 MaxBrokerScopePaths,
  max_revoked_entries | BoundedInt 1 MaxRevokedEntries,
  require_pre_read_validation | Exact true,
  expose_raw_credentials | Exact false,
  expose_object_keys | Exact false,
}
in
let StoreRules =
  std.contract.from_predicate (fun cfg =>
    cfg.s3_secondary.enabled || !(cfg.eviction_watermarks.require_secondary_before_primary_eviction)
  )
in
let ProjectedManifestStoreContract =
  std.contract.Sequence [
    {
      schema | Exact "kiln.projected-manifest-store.config.v1",
      profile_id | Text MaxIdBytes,
      molten_primary | MoltenPrimaryContract,
      s3_secondary | S3SecondaryContract,
      eviction_watermarks | EvictionWatermarksContract,
      broker | BrokerBoundsContract,
      non_claims | Array (Text 256),
    },
    StoreRules,
  ]
in
{
  schema = "kiln.projected-manifest-store.config.v1",
  profile_id = "projected-manifest-store-local-default-v1",
  molten_primary = {
    enabled = true,
    port_id = "molten.fabric.content-store",
    port_version = "v1",
    verify_blake3_on_read = true,
    max_envelope_bytes = MaxEnvelopeBytes,
    max_object_closure_entries = MaxObjectClosureEntries,
  },
  s3_secondary = {
    enabled = false,
    contract_version = 1,
    endpoint_ref = "",
    bucket_ref = "",
    key_prefix = "",
    conditional_writes_required = true,
    allow_bucket_listing = false,
    allow_presigned_urls = false,
    preserve_unknown_outcomes = true,
    max_evict_batch = MaxEvictBatch,
  },
  eviction_watermarks = {
    high_watermark_entries = 1024,
    low_watermark_entries = 768,
    high_watermark_bytes = 268435456,
    low_watermark_bytes = 201326592,
    require_secondary_before_primary_eviction = false,
    respect_retention_roots = true,
  },
  broker = {
    max_ticket_ttl_secs = MaxBrokerTicketTtlSecs,
    max_download_bytes = MaxBrokerDownloadBytes,
    max_scope_paths = MaxBrokerScopePaths,
    max_revoked_entries = MaxRevokedEntries,
    require_pre_read_validation = true,
    expose_raw_credentials = false,
    expose_object_keys = false,
  },
  non_claims = [
    "molten primary persistence does not prove multi-host durability",
    "s3 secondary conditional write does not provide cross-key transactions",
    "broker ticket admission does not recall previously disclosed bytes",
  ],
} | ProjectedManifestStoreContract
