let Contracts = import "contracts.ncl" in
let InitialGeneration = 1 in
let PortCount = 7 in
let NonClaimCount = 12 in
let MaxCallbackInputBytes = 1048576 in
let MaxCallbackOutputBytes = 1048576 in
let MaxMaterializedValueBytes = 262144 in
let MaxMaterializedValues = 64 in
let MaxContentBytes = 16777216 in
let MaxLogBytes = 262144 in
let MaxEffects = 64 in
let MaxRecords = 4096 in
let CallbackTimeoutMillis = 30000 in
let AuthorityRef = "kiln-aspen-authority-v1" in
let ResourceRef = "kiln-aspen-resources-v1" in
({
  schema = "kiln.aspen-runtime.profile.v2",
  profile_id = "aspen-runtime-v2",
  cohort = {
    source = "ssh://git@github.com/brittonr/aspen.git",
    revision = "22f8ded26ca1907c29948e08b53f35df23080733",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
  },
  native_host = {
    profile_schema = "molten.system-extension.native-host-profile.v2",
    callback_envelope_schema = "molten.system-extension.native-callback-envelope.v2",
    callback_outcome_schema = "molten.system-extension.native-callback-outcome.v2",
    instance_schema = "molten.system-extension.native-instance-state.v2",
    ingress_schema = "molten.system-extension.native-ingress.v2",
    extension_protocol_schema = "kiln.aspen-extension.protocol.v2",
    extension_id = "kiln-aspen-extension-v2",
    service_id = "kiln-ci-control-plane-v2",
    execution_profile = "native-host-local-pilot-v2",
    alpn = "molten/system-extension/native/v2",
    framing = "preserves-packed-materialized-values-v2",
    generation = InitialGeneration,
  },
  endpoint_ref = "kiln-aspen-local-endpoint-v1",
  transport_profile_ref = "kiln-aspen-local-transport-v1",
  policy_ref = "kiln-aspen-policy-v1",
  authority_ref = AuthorityRef,
  resource_ref = ResourceRef,
  claim_level = "local-live-materialized-values-pilot",
  workflow_provider_profile = "lattice-v1",
  ports = [
    {
      purpose = "durability",
      port_id = "molten.fabric.durability.ordered-state",
      version = "v1",
      input_schema = "molten.fabric.durability.atomic-batch.v1",
      output_schema = "molten.fabric.durability.atomic-receipt.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
    {
      purpose = "content",
      port_id = "molten.fabric.content-store",
      version = "v1",
      input_schema = "molten.content-store-adapter.command.v1",
      output_schema = "molten.content-store-adapter.event.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
    {
      purpose = "execution",
      port_id = "molten.fabric.execution.bounded-process",
      version = "v1",
      input_schema = "molten.fabric.execution-request.v1",
      output_schema = "molten.fabric.execution-receipt.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
    {
      purpose = "transport",
      port_id = "molten.fabric.transport.session",
      version = "v1",
      input_schema = "kiln.aspen-extension.effect.v2",
      output_schema = "kiln.aspen-provider-completion.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
    {
      purpose = "time",
      port_id = "molten.fabric.time",
      version = "v1",
      input_schema = "molten.fabric.time.request.v1",
      output_schema = "molten.fabric.time.observation.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
    {
      purpose = "scheduling",
      port_id = "molten.fabric.scheduler",
      version = "v1",
      input_schema = "molten.fabric.scheduler.request.v1",
      output_schema = "molten.fabric.scheduler.observation.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
    {
      purpose = "evidence",
      port_id = "molten.fabric.observation",
      version = "v1",
      input_schema = "molten.fabric.observation.event.v1",
      output_schema = "molten.fabric.observation.receipt.v1",
      authority_ref = AuthorityRef,
      resource_ref = ResourceRef,
    },
  ],
  port_count = PortCount,
  bounds = {
    max_callback_input_bytes = MaxCallbackInputBytes,
    max_callback_output_bytes = MaxCallbackOutputBytes,
    max_materialized_value_bytes = MaxMaterializedValueBytes,
    max_materialized_values = MaxMaterializedValues,
    max_content_bytes = MaxContentBytes,
    max_log_bytes = MaxLogBytes,
    max_effects = MaxEffects,
    max_records = MaxRecords,
    callback_timeout_millis = CallbackTimeoutMillis,
  },
  fallback = "none",
  non_claims = [
    "aspen acceptance does not prove CI correctness",
    "aspen execution does not prove provider correctness",
    "bounded execution does not prove sandboxing or hermeticity",
    "transport acknowledgement does not prove exact-once delivery",
    "content identity does not prove source trust",
    "durable acceptance does not prove global durability",
    "local conformance does not prove production availability",
    "runtime evidence does not prove release eligibility",
    "equal digest text does not merge identity roles",
    "provider observations require Kiln admission",
    "value identity does not prove application meaning",
    "in-memory conformance does not prove value durability",
  ],
  non_claim_count = NonClaimCount,
}
) | Contracts.AspenRuntimeProfile
