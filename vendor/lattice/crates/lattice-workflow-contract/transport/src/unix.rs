#![cfg(target_os = "linux")]
#![allow(
    unknown_lints,
    ambient_clock,
    non_trait_imports,
    reason = "socket deadlines are explicit profile inputs; named imports keep this imperative shell readable without hiding core ownership"
)]

use std::fs::Permissions;
use std::io::Write;
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use nix::sys::socket::getsockopt;
use nix::sys::socket::sockopt::PeerCredentials;

use crate::ClientFailure;
use crate::ExchangeRequest;
use crate::ExchangeResponse;
use crate::FrameFailure;
use crate::SOCKET_PERMISSION_MODE;
use crate::ServeReport;
use crate::ServerFailure;
use crate::TransportProfile;
use crate::WorkflowExchangeHandler;
use crate::WorkflowExchangeService;
use crate::encode_frame;
use crate::read_frame;
use crate::validate_exchange_request;
use crate::validate_exchange_response;
use crate::validate_transport_profile;
use crate::write_frame;

#[derive(Debug, Clone)]
pub struct UnixEndpoint<'a> {
    pub socket_path: &'a Path,
    pub profile: &'a TransportProfile,
}

#[derive(Debug, Clone)]
pub struct UnixClient<'a> {
    endpoint: UnixEndpoint<'a>,
}

impl<'a> UnixClient<'a> {
    pub const fn new(endpoint: UnixEndpoint<'a>) -> Self {
        Self { endpoint }
    }

    // r[impl lattice.workflow_interchange.transport_ambiguity]
    pub fn exchange(&self, request: &ExchangeRequest) -> Result<ExchangeResponse, ClientFailure> {
        validate_transport_profile(self.endpoint.profile).map_err(|_| ClientFailure::InvalidProfile)?;
        validate_exchange_request(request).map_err(|_| ClientFailure::InvalidRequest)?;
        let limits = self.endpoint.profile.limits;
        let frame = encode_frame(request, limits.maximum_frame_bytes).map_err(|_| ClientFailure::InvalidRequest)?;
        let mut stream = UnixStream::connect(self.endpoint.socket_path).map_err(|_| ClientFailure::ConnectFailed)?;
        set_stream_deadlines(&stream, self.endpoint.profile).map_err(|_| ClientFailure::ConnectFailed)?;
        write_frame(&mut stream, &frame).map_err(classify_incomplete_write)?;
        stream.shutdown(Shutdown::Write).map_err(|_| ClientFailure::UnknownAfterWrite)?;
        let body = read_frame(&mut stream, limits.maximum_frame_bytes).map_err(|_| ClientFailure::UnknownAfterWrite)?;
        let response: ExchangeResponse = serde_json::from_slice(&body).map_err(|_| ClientFailure::UnknownAfterWrite)?;
        validate_exchange_response(request, &response).map_err(|_| ClientFailure::UnknownAfterWrite)?;
        crate::validate_response_limits(self.endpoint.profile, &response)
            .map_err(|_| ClientFailure::UnknownAfterWrite)?;
        Ok(response)
    }
}

// r[impl lattice.workflow_interchange.transport_bounds]
// r[impl lattice.workflow_interchange.transport_peer]
pub fn serve<H: WorkflowExchangeHandler>(
    endpoint: UnixEndpoint<'_>,
    service: &mut WorkflowExchangeService<H>,
) -> Result<ServeReport, ServerFailure> {
    validate_transport_profile(endpoint.profile).map_err(|_| ServerFailure::InvalidProfile)?;
    let listener = UnixListener::bind(endpoint.socket_path).map_err(|_| ServerFailure::BindFailed)?;
    let permissions = Permissions::from_mode(SOCKET_PERMISSION_MODE);
    if std::fs::set_permissions(endpoint.socket_path, permissions).is_err() {
        let _cleanup_result = std::fs::remove_file(endpoint.socket_path);
        return Err(ServerFailure::PermissionFailed);
    }
    let report = serve_listener(&listener, endpoint.profile, service);
    let cleanup = std::fs::remove_file(endpoint.socket_path);
    if cleanup.is_err() {
        return Err(ServerFailure::PermissionFailed);
    }
    report
}

pub fn serve_listener<H: WorkflowExchangeHandler>(
    listener: &UnixListener,
    profile: &TransportProfile,
    service: &mut WorkflowExchangeService<H>,
) -> Result<ServeReport, ServerFailure> {
    debug_assert!(profile.limits.maximum_connections > 0);
    debug_assert!(!profile.peers.allowed_user_ids.is_empty());
    let mut report = ServeReport {
        accepted_connections: 0,
        completed_exchanges: 0,
        rejected_peers: 0,
        failed_exchanges: 0,
    };
    while report.accepted_connections < profile.limits.maximum_connections {
        let (mut stream, _address) = listener.accept().map_err(|_| ServerFailure::AcceptFailed)?;
        report.accepted_connections = report.accepted_connections.checked_add(1).ok_or(ServerFailure::AcceptFailed)?;
        if !peer_is_admitted(&stream, profile)? {
            report.rejected_peers = report.rejected_peers.checked_add(1).ok_or(ServerFailure::PeerDenied)?;
            continue;
        }
        if set_stream_deadlines(&stream, profile).is_err() {
            increment_failures(&mut report)?;
            continue;
        }
        if exchange_one(&mut stream, profile, service).is_err() {
            increment_failures(&mut report)?;
            continue;
        }
        report.completed_exchanges = report.completed_exchanges.checked_add(1).ok_or(ServerFailure::WriteFailed)?;
    }
    Ok(report)
}

fn exchange_one<H: WorkflowExchangeHandler>(
    stream: &mut UnixStream,
    profile: &TransportProfile,
    service: &mut WorkflowExchangeService<H>,
) -> Result<(), ServerFailure> {
    debug_assert!(profile.limits.maximum_frame_bytes > 0);
    debug_assert!(profile.limits.maximum_log_bytes > 0);
    let body = read_frame(stream, profile.limits.maximum_frame_bytes).map_err(map_read_failure)?;
    let request: ExchangeRequest = serde_json::from_slice(&body).map_err(|_| ServerFailure::ReadFailed)?;
    let candidate = service.exchange(&request).map_err(|_| ServerFailure::WriteFailed)?;
    let response = if crate::validate_response_limits(profile, &candidate).is_ok() {
        candidate
    } else {
        crate::new_response(
            request.identity.clone(),
            crate::ExchangeResult::Rejected(crate::Rejection {
                code: crate::RejectionCode::LogTooLarge,
                field: "exchange.log".to_string(),
            }),
        )
        .map_err(|_| ServerFailure::WriteFailed)?
    };
    let frame = encode_frame(&response, profile.limits.maximum_frame_bytes).map_err(map_write_failure)?;
    write_frame(stream, &frame).map_err(map_write_failure)?;
    stream.flush().map_err(|_| ServerFailure::WriteFailed)
}

fn peer_is_admitted(stream: &UnixStream, profile: &TransportProfile) -> Result<bool, ServerFailure> {
    let credentials = getsockopt(stream, PeerCredentials).map_err(|_| ServerFailure::PeerObservationFailed)?;
    Ok(profile.peers.allowed_user_ids.binary_search(&credentials.uid()).is_ok())
}

fn set_stream_deadlines(stream: &UnixStream, profile: &TransportProfile) -> Result<(), ServerFailure> {
    let read = Duration::from_millis(profile.limits.read_deadline_milliseconds);
    let write = Duration::from_millis(profile.limits.write_deadline_milliseconds);
    stream.set_read_timeout(Some(read)).map_err(|_| ServerFailure::ReadFailed)?;
    stream.set_write_timeout(Some(write)).map_err(|_| ServerFailure::WriteFailed)
}

fn increment_failures(report: &mut ServeReport) -> Result<(), ServerFailure> {
    report.failed_exchanges = report.failed_exchanges.checked_add(1).ok_or(ServerFailure::ReadFailed)?;
    Ok(())
}

// r[impl lattice.workflow_interchange.transport_ambiguity.scenario.not-accepted]
const fn classify_incomplete_write(_failure: FrameFailure) -> ClientFailure {
    ClientFailure::NotAccepted
}

fn map_read_failure(_failure: FrameFailure) -> ServerFailure {
    ServerFailure::ReadFailed
}

fn map_write_failure(_failure: FrameFailure) -> ServerFailure {
    ServerFailure::WriteFailed
}
