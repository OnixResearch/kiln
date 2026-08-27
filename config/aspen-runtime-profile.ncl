let Contracts = import "contracts.ncl" in
let InitialGeneration = 1 in
let PortCount = 7 in
let NonClaimCount = 10 in
let MaxCallbackInputBytes = 1048576 in
let MaxCallbackOutputBytes = 1048576 in
let MaxContentBytes = 16777216 in
let MaxLogBytes = 262144 in
let MaxEffects = 64 in
let MaxRecords = 4096 in
let CallbackTimeoutMillis = 30000 in
let AuthorityRef = "kiln-aspen-authority-v1" in
let ResourceRef = "kiln-aspen-resources-v1" in
({
  schema = "kiln.aspen-runtime.profile.v1",
  profile_id = "aspen-runtime-v1",
  cohort = {
    source = "ssh://git@github.com/OnixResearch/aspen.git",
    revision = "7fb4c9de293f815654e1e3b954f4fee3678ac87f",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
  },
  native_host = {
    profile_schema = "molten.system-extension.native-host-profile.v1",
    callback_envelope_schema = "molten.system-extension.native-callback-envelope.v1",
    callback_outcome_schema = "molten.system-extension.native-callback-outcome.v1",
    ingress_schema = "molten.system-extension.native-ingress.v1",
    extension_protocol_schema = "kiln.aspen-extension.protocol.v1",
    extension_id = "kiln-aspen-extension-v1",
    service_id = "kiln-ci-control-plane-v1",
    execution_profile = "native-host-local-pilot-v1",
    alpn = "molten/system-extension/native/v1",
    framing = "preserves-packed-single-frame-v1",
    generation = InitialGeneration,
  },
  endpoint_ref = "kiln-aspen-local-endpoint-v1",
  transport_profile_ref = "kiln-aspen-local-transport-v1",
  policy_ref = "kiln-aspen-policy-v1",
  authority_ref = AuthorityRef,
  resource_ref = ResourceRef,
  claim_level = "local-live-pilot",
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
      input_schema = "molten.fabric.transport.request.v1",
      output_schema = "molten.fabric.transport.observation.v1",
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
  ],
  non_claim_count = NonClaimCount,
}
) | Contracts.AspenRuntimeProfile
