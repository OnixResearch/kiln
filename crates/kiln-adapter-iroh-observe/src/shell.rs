use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use std::time::Duration;

use iroh::endpoint::{Connection, TransportConfig, VarInt};
use iroh::{Endpoint, EndpointAddr, RelayMap, RelayMode, RelayUrl, SecretKey};
use kiln_adapter_http_observe::{ObservationSnapshot, ObservationSource};
use kiln_observe_core::{ObservationRequest, ObservationWindow, ViewRequest, project_observation};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::profile::{IROH_OBSERVATION_ALPN, IrohObservationProfile};
use crate::wire::{
    IrohObserveRequest, IrohObserveResponse, decode_request, decode_response, encode_request,
    encode_response,
};
use crate::{ObserveError, ObserveResult};

const CONNECTION_CLOSE_CODE: u32 = 0;
const CONNECTION_CLOSE_REASON: &[u8] = b"observation exchange ended";
const DENIED_CLOSE_CODE: u32 = 1;
const DENIED_CLOSE_REASON: &[u8] = b"observation peer denied";
const EXCHANGE_STREAMS: u32 = 1;
const DISABLED_STREAMS: u32 = 0;

// r[impl kiln.observe.iroh.serve]
pub async fn bind_endpoint(
    profile: &IrohObservationProfile,
    secret_key: SecretKey,
) -> ObserveResult<Endpoint> {
    profile.admit()?;
    let relays: Vec<RelayUrl> = profile
        .endpoint
        .relay_urls
        .iter()
        .map(|url| {
            url.parse()
                .map_err(|_| ObserveError::new("invalid_relay", "relay URL is malformed"))
        })
        .collect::<Result<_, _>>()?;
    let relay_mode = if relays.is_empty() {
        RelayMode::Disabled
    } else {
        RelayMode::Custom(RelayMap::from_iter(relays))
    };
    let frame_window_bytes = profile
        .endpoint
        .max_frame_bytes
        .checked_add(4)
        .ok_or_else(|| {
            ObserveError::new(
                "invalid_iroh_bounds",
                "Iroh frame window cannot be represented",
            )
        })?;
    let frame_window = VarInt::from_u64(frame_window_bytes).map_err(|_| {
        ObserveError::new(
            "invalid_iroh_bounds",
            "Iroh frame window cannot be represented",
        )
    })?;
    let mut quic = TransportConfig::default();
    quic.max_concurrent_bidi_streams(EXCHANGE_STREAMS.into())
        .max_concurrent_uni_streams(DISABLED_STREAMS.into())
        .stream_receive_window(frame_window)
        .receive_window(frame_window)
        .send_window(frame_window_bytes)
        .datagram_receive_buffer_size(None)
        .max_idle_timeout(Some(deadline(profile).try_into().map_err(|_| {
            ObserveError::new("invalid_deadline", "Iroh deadline is invalid")
        })?));
    let mut builder = Endpoint::builder()
        .secret_key(secret_key)
        .clear_discovery()
        .relay_mode(relay_mode)
        .transport_config(quic)
        .alpns(vec![IROH_OBSERVATION_ALPN.to_vec()]);
    builder = match profile.endpoint.bind_address {
        SocketAddr::V4(address) => builder
            .bind_addr_v4(address)
            .bind_addr_v6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0)),
        SocketAddr::V6(address) => builder
            .bind_addr_v6(address)
            .bind_addr_v4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
    };
    timeout(deadline(profile), builder.bind())
        .await
        .map_err(|_| ObserveError::new("iroh_deadline", "Iroh endpoint bind deadline expired"))?
        .map_err(|_| ObserveError::new("iroh_transport", "Iroh endpoint could not bind"))
}

#[must_use]
pub fn remote_address(endpoint: &Endpoint) -> EndpointAddr {
    let mut address = EndpointAddr::new(endpoint.id());
    if let Some(socket) = endpoint.bound_sockets().first() {
        address = address.with_ip_addr(*socket);
    }
    address
}

// r[impl kiln.observe.iroh.peer]
pub async fn serve(
    endpoint: Endpoint,
    profile: IrohObservationProfile,
    source: Arc<dyn ObservationSource>,
    shutdown: impl Future<Output = ()>,
) -> ObserveResult<()> {
    profile.admit()?;
    let profile = Arc::new(profile);
    tokio::pin!(shutdown);
    let mut tasks = JoinSet::new();
    let result = loop {
        tokio::select! {
            _ = &mut shutdown => break Ok(()),
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if joined.is_some_and(|result| result.is_err()) {
                    break Err(ObserveError::new("iroh_task", "Iroh observation task failed"));
                }
            }
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break Ok(()); };
                if tasks.len() >= profile.endpoint.max_connections as usize {
                    incoming.refuse();
                    continue;
                }
                let profile = Arc::clone(&profile);
                let source = Arc::clone(&source);
                tasks.spawn(async move {
                    let _outcome = timeout(deadline(&profile), async {
                        let connection = ClosingConnection(
                            incoming
                                .await
                                .map_err(|_| ObserveError::new("iroh_transport", "Iroh connection failed"))?,
                        );
                        if profile.admit_peer(connection.0.remote_id()).is_err() {
                            connection
                                .0
                                .close(DENIED_CLOSE_CODE.into(), DENIED_CLOSE_REASON);
                            return Err(ObserveError::new(
                                "iroh_peer_denied",
                                "Iroh observation peer is denied",
                            ));
                        }
                        let (mut send, mut recv) = connection
                            .0
                            .accept_bi()
                            .await
                            .map_err(|_| ObserveError::new("iroh_transport", "Iroh request stream failed"))?;
                        let body = read_frame(&mut recv, &profile).await?;
                        require_end(&mut recv).await?;
                        let request = decode_request(&body, max_frame(&profile))?;
                        let response = observe(&profile, source.as_ref(), &request.request);
                        let response_body = encode_response(&response, max_frame(&profile))?;
                        write_frame(&mut send, &response_body, &profile).await?;
                        send.finish().map_err(|_| {
                            ObserveError::new("iroh_transport", "Iroh response stream could not finish")
                        })?;
                        let _acknowledged = send.stopped().await;
                        Ok::<(), ObserveError>(())
                    })
                    .await;
                });
            }
        }
    };
    stop_tasks(&mut tasks).await;
    endpoint.close().await;
    result
}

fn observe(
    profile: &IrohObservationProfile,
    source: &dyn ObservationSource,
    request: &ObservationRequest,
) -> IrohObserveResponse {
    let snapshot = source.snapshot();
    if request.state_revision != snapshot.state.revision {
        return IrohObserveResponse::error(
            "unknown_state_revision",
            "requested state revision does not match admitted state",
        );
    }
    if let Err(error) = admit_target(&request.view, &snapshot) {
        return IrohObserveResponse::error(error.code, &error.message);
    }
    let window = match observation_window(&request.view, &snapshot) {
        Ok(window) => window,
        Err(error) => return IrohObserveResponse::error(error.code, &error.message),
    };
    match project_observation(&profile.core, &snapshot.state, request, &window) {
        Ok(view) => match IrohObserveResponse::ok(view) {
            Ok(response) => response,
            Err(error) => IrohObserveResponse::error(error.code, &error.message),
        },
        Err(error) => IrohObserveResponse::error(
            error.code.as_str(),
            "observation projection could not be completed",
        ),
    }
}

fn admit_target(view: &ViewRequest, snapshot: &ObservationSnapshot) -> Result<(), ObserveError> {
    match view {
        ViewRequest::RunList {
            after: Some(after), ..
        } => {
            if snapshot.state.runs.contains_key(after) {
                Ok(())
            } else {
                Err(ObserveError::new(
                    "unknown_target",
                    "run cursor is not in the admitted state",
                ))
            }
        }
        ViewRequest::RunDetail { run_id } => {
            if snapshot.state.runs.contains_key(run_id) {
                Ok(())
            } else {
                Err(ObserveError::new(
                    "unknown_target",
                    "run is not in the admitted state",
                ))
            }
        }
        ViewRequest::AttemptDetail { attempt_id } => {
            if snapshot.state.attempts.contains_key(attempt_id) {
                Ok(())
            } else {
                Err(ObserveError::new(
                    "unknown_target",
                    "attempt is not in the admitted state",
                ))
            }
        }
        ViewRequest::LogRange { stream_id, .. } => {
            if snapshot
                .log
                .as_ref()
                .is_some_and(|log| log.meta.stream_id == *stream_id)
            {
                Ok(())
            } else {
                Err(ObserveError::new(
                    "unknown_target",
                    "log stream is not in the admitted window",
                ))
            }
        }
        ViewRequest::RunList { after: None, .. }
        | ViewRequest::StateSummary
        | ViewRequest::EventTail { .. } => Ok(()),
    }
}

fn observation_window(
    view: &ViewRequest,
    snapshot: &ObservationSnapshot,
) -> Result<ObservationWindow, ObserveError> {
    match view {
        ViewRequest::EventTail { .. } => snapshot
            .events
            .as_ref()
            .map(|events| ObservationWindow::events(events.records.clone()))
            .ok_or_else(|| {
                ObserveError::new("missing_event_window", "event window is unavailable")
            }),
        ViewRequest::LogRange { .. } => snapshot
            .log
            .as_ref()
            .map(|log| ObservationWindow::log(log.meta.clone(), log.retained_bytes.clone()))
            .ok_or_else(|| ObserveError::new("unknown_target", "log stream is unavailable")),
        _ => Ok(ObservationWindow::empty()),
    }
}

// r[impl kiln.observe.iroh.serve.scenario.target]
// r[impl kiln.observe.iroh.serve.scenario.transport_failure]
pub async fn client_exchange(
    endpoint: &Endpoint,
    remote: EndpointAddr,
    profile: &IrohObservationProfile,
    request: &ObservationRequest,
) -> ObserveResult<IrohObserveResponse> {
    profile.admit()?;
    profile.admit_peer(remote.id)?;
    let body = encode_request(
        &IrohObserveRequest {
            schema: String::from(crate::wire::REQUEST_SCHEMA),
            request: request.clone(),
        },
        max_frame(profile),
    )?;
    timeout(deadline(profile), async {
        let connection = ClosingConnection(
            endpoint
                .connect(remote.clone(), IROH_OBSERVATION_ALPN)
                .await
                .map_err(|_| {
                    ObserveError::new("iroh_transport", "Iroh observation connection failed")
                })?,
        );
        profile.admit_peer(connection.0.remote_id())?;
        let (mut send, mut recv) = connection
            .0
            .open_bi()
            .await
            .map_err(|_| ObserveError::new("iroh_transport", "Iroh request stream failed"))?;
        write_frame(&mut send, &body, profile).await?;
        send.finish().map_err(|_| {
            ObserveError::new("iroh_transport", "Iroh request stream could not finish")
        })?;
        let response_body = read_frame(&mut recv, profile).await?;
        require_end(&mut recv).await?;
        decode_response(&response_body, max_frame(profile))
    })
    .await
    .map_err(|_| ObserveError::new("iroh_deadline", "Iroh observation deadline expired"))?
}

fn deadline(profile: &IrohObservationProfile) -> Duration {
    Duration::from_millis(profile.endpoint.request_timeout_milliseconds)
}

fn max_frame(profile: &IrohObservationProfile) -> usize {
    usize::try_from(profile.endpoint.max_frame_bytes).unwrap_or(usize::MAX)
}

async fn read_frame(
    reader: &mut (impl AsyncRead + Unpin),
    profile: &IrohObservationProfile,
) -> ObserveResult<Vec<u8>> {
    let length =
        reader.read_u32().await.map_err(|_| {
            ObserveError::new("iroh_transport", "Iroh frame length could not be read")
        })? as usize;
    if length == 0 || length > max_frame(profile) {
        return Err(ObserveError::new(
            "iroh_frame_bound",
            "Iroh frame length is outside its bound",
        ));
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|_| ObserveError::new("iroh_transport", "Iroh frame body could not be read"))?;
    Ok(body)
}

async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    body: &[u8],
    profile: &IrohObservationProfile,
) -> ObserveResult<()> {
    if body.is_empty() || body.len() > max_frame(profile) {
        return Err(ObserveError::new(
            "iroh_frame_bound",
            "Iroh frame body is outside its bound",
        ));
    }
    let length = u32::try_from(body.len())
        .map_err(|_| ObserveError::new("iroh_frame_bound", "Iroh frame length is invalid"))?;
    writer.write_u32(length).await.map_err(|_| {
        ObserveError::new("iroh_transport", "Iroh frame length could not be written")
    })?;
    writer
        .write_all(body)
        .await
        .map_err(|_| ObserveError::new("iroh_transport", "Iroh frame body could not be written"))
}

async fn require_end(reader: &mut (impl AsyncRead + Unpin)) -> ObserveResult<()> {
    let mut extra = [0_u8];
    match reader.read(&mut extra).await {
        Ok(0) => Ok(()),
        _ => Err(ObserveError::new(
            "iroh_frame_trailing",
            "Iroh frame has trailing bytes",
        )),
    }
}

async fn stop_tasks(tasks: &mut JoinSet<()>) {
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
}

struct ClosingConnection(Connection);

impl Drop for ClosingConnection {
    fn drop(&mut self) {
        self.0
            .close(CONNECTION_CLOSE_CODE.into(), CONNECTION_CLOSE_REASON);
    }
}
