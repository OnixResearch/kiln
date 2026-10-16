use crate::profile::AdmittedAspenRuntimeProfile;

use super::{
    AspenServiceTransport, AspenUnixOperation, AspenUnixRequest, AspenUnixResponse,
    EXCHANGE_RESPONSE_SCHEMA, ServiceTransportOutcome,
};

// r[impl kiln.aspen_host.transport]
pub fn handle_aspen_unix_request<T: AspenServiceTransport>(
    profile: &AdmittedAspenRuntimeProfile,
    service: &mut T,
    request: AspenUnixRequest,
) -> AspenUnixResponse {
    let outcome = if request.profile_identity != profile.profile_identity {
        ServiceTransportOutcome::Malformed
    } else {
        match (request.operation, request.ingress) {
            (AspenUnixOperation::Submit, Some(ingress)) => service.submit(profile, &ingress),
            (AspenUnixOperation::Reconcile, None) => {
                service.reconcile(profile, &request.operation_id)
            }
            (AspenUnixOperation::Submit | AspenUnixOperation::Reconcile, _) => {
                ServiceTransportOutcome::Malformed
            }
        }
    };
    AspenUnixResponse {
        schema: EXCHANGE_RESPONSE_SCHEMA.to_string(),
        profile_identity: profile.profile_identity.clone(),
        operation_id: request.operation_id,
        outcome,
    }
}
