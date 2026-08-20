let Contracts = import "../contracts.ncl" in
({
  cdevents_spec_version = "0.5.1",
  cdevents_source_revision = "df02ce0b6563a13bf35cc109993b03b18da8a14a",
  cloudevents_spec_version = "1.0.2",
  cloudevents_context_version = "1.0",
  license = "Apache-2.0",
  supported_event_types = ["dev.cdevents.taskrun.started.0.3.0"],
  allow_lossy_mapping = true,
  bounds = {
    max_envelope_bytes = 1,
    max_text_bytes = 1,
    max_fields = 1,
  },
  non_claims = ["lossless only"],
}) | Contracts.CDEventsProfile
