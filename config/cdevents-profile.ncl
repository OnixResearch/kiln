let Contracts = import "contracts.ncl" in
let MaxEnvelopeBytes = 16384 in
let MaxTextBytes = 128 in
let MaxFields = 16 in
({
  cdevents_spec_version = "0.5.1",
  cdevents_source_revision = "df02ce0b6563a13bf35cc109993b03b18da8a14a",
  cloudevents_spec_version = "1.0.2",
  cloudevents_context_version = "1.0",
  license = "Apache-2.0",
  supported_event_types = [
    "dev.cdevents.taskrun.started.0.3.0",
    "dev.cdevents.taskrun.finished.0.3.0",
  ],
  allow_lossy_mapping = false,
  bounds = {
    max_envelope_bytes = MaxEnvelopeBytes,
    max_text_bytes = MaxTextBytes,
    max_fields = MaxFields,
  },
  non_claims = [
    "format compatibility does not prove external event truth",
    "imported events do not create canonical Kiln outcomes",
    "encoding does not prove delivery or provider correctness",
  ],
}) | Contracts.CDEventsProfile
