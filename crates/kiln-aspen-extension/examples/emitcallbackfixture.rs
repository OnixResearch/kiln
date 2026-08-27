use std::io::Write;

use kiln_adapter_aspen::profile::{
    DEFAULT_CALLBACK_TIMEOUT_MILLIS, INITIAL_SERVICE_GENERATION, KILN_EXTENSION_PROTOCOL_SCHEMA,
    admit_profile, profile_fixture,
};
use kiln_adapter_aspen::protocol::{
    CALLBACK_REQUEST_SCHEMA, CallbackCommand, CallbackRequest, CallbackState,
    encode_callback_request,
};
use kiln_core::types::ExternalRef;

fn main() -> Result<(), String> {
    let admitted = admit_profile(&profile_fixture())
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let operation_id = ExternalRef::new("molten", "operation", "producer-fixture-status")
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let request = CallbackRequest {
        schema: CALLBACK_REQUEST_SCHEMA.to_string(),
        protocol_schema: KILN_EXTENSION_PROTOCOL_SCHEMA.to_string(),
        profile_identity: admitted.profile_identity,
        generation: INITIAL_SERVICE_GENERATION,
        sequence: INITIAL_SERVICE_GENERATION,
        operation_id,
        authority_ref: admitted.profile.authority_ref,
        policy_ref: admitted.profile.policy_ref,
        resource_ref: admitted.profile.resource_ref,
        deadline_tick: DEFAULT_CALLBACK_TIMEOUT_MILLIS,
        prior_state: CallbackState::initial(INITIAL_SERVICE_GENERATION),
        command: CallbackCommand::Status,
    };
    let bytes = encode_callback_request(&request, admitted.profile.bounds.max_callback_input_bytes)
        .map_err(|error| error.to_string())?;
    std::io::stdout()
        .lock()
        .write_all(&bytes)
        .map_err(|error| error.to_string())
}
