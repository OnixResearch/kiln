use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iroh::endpoint::{TransportConfig, VarInt};
use iroh::{Endpoint, EndpointAddr, RelayMap, RelayMode, RelayUrl, SecretKey};
use lattice_workflow_transport::{ExchangeRequest, ExchangeResponse};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::{ALPN, Error, Profile};

const CONNECTION_CLOSE_CODE: u32 = 0;
const CONNECTION_CLOSE_REASON: &[u8] = b"workflow exchange ended";
const EXCHANGE_STREAMS: u32 = 1;
const DISABLED_STREAMS: u32 = 0;

// r[impl kiln.iroh.workflow.profile]
pub async fn bind_endpoint(profile: &Profile, secret_key: SecretKey) -> Result<Endpoint, Error> {
    profile.admit()?;
    let relays: Vec<RelayUrl> = profile
        .relay_urls
        .iter()
        .map(|url| url.parse().map_err(|_| Error::InvalidProfile))
        .collect::<Result<_, _>>()?;
    let relay_mode = if relays.is_empty() {
        RelayMode::Disabled
    } else {
        RelayMode::Custom(RelayMap::from_iter(relays))
    };
    let frame_window = u64::from(profile.transport.limits.maximum_frame_bytes)
        + lattice_workflow_transport::FRAME_HEADER_BYTES as u64;
    let receive_window = VarInt::from_u64(frame_window).map_err(|_| Error::InvalidProfile)?;
    let mut quic = TransportConfig::default();
    quic.max_concurrent_bidi_streams(EXCHANGE_STREAMS.into())
        .max_concurrent_uni_streams(DISABLED_STREAMS.into())
        .stream_receive_window(receive_window)
        .receive_window(receive_window)
        .send_window(frame_window)
        .datagram_receive_buffer_size(None)
        .max_idle_timeout(Some(
            deadline(profile)
                .try_into()
                .map_err(|_| Error::InvalidProfile)?,
        ));
    let mut builder = Endpoint::builder()
        .secret_key(secret_key)
        .clear_discovery()
        .relay_mode(relay_mode)
        .transport_config(quic)
        .alpns(vec![ALPN.to_vec()]);
    // Do not leave the other address family listening on an ambient wildcard.
    builder = match profile.bind_address {
        SocketAddr::V4(address) => builder
            .bind_addr_v4(address)
            .bind_addr_v6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0)),
        SocketAddr::V6(address) => builder
            .bind_addr_v6(address)
            .bind_addr_v4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
    };
    timeout(deadline(profile), builder.bind())
        .await
        .map_err(|_| Error::Deadline)?
        .map_err(|_| Error::Transport)
}

#[derive(Clone, Debug)]
pub struct IrohClient {
    endpoint: Endpoint,
    remote: EndpointAddr,
    profile: Arc<Profile>,
}

impl IrohClient {
    pub fn new(endpoint: Endpoint, remote: EndpointAddr, profile: Profile) -> Result<Self, Error> {
        profile.admit()?;
        profile.admit_peer(remote.id)?;
        Ok(Self {
            endpoint,
            remote,
            profile: Arc::new(profile),
        })
    }

    pub async fn exchange(&self, request: &ExchangeRequest) -> Result<ExchangeResponse, Error> {
        self.profile.admit_request(request)?;
        let body = serde_json::to_vec(request).map_err(|_| Error::InvalidRequest)?;
        let (_body, response) = self.exchange_body(request, &body).await?;
        Ok(response)
    }

    // r[impl kiln.iroh.workflow.transport] r[impl kiln.iroh.workflow.recovery]
    async fn exchange_body(
        &self,
        request: &ExchangeRequest,
        body: &[u8],
    ) -> Result<(Vec<u8>, ExchangeResponse), Error> {
        check_length(body.len(), &self.profile)?;
        timeout(deadline(&self.profile), async {
            let connection = self
                .endpoint
                .connect(self.remote.clone(), ALPN)
                .await
                .map_err(|_| Error::Transport)?;
            let connection = ClosingConnection(connection);
            self.profile.admit_peer(connection.0.remote_id())?;
            let (mut send, mut recv) =
                connection.0.open_bi().await.map_err(|_| Error::Transport)?;
            write_body(&mut send, body).await?;
            send.finish().map_err(|_| Error::Transport)?;
            let response_body = read_body(&mut recv, &self.profile).await?;
            require_end(&mut recv).await?;
            let response: ExchangeResponse =
                serde_json::from_slice(&response_body).map_err(|_| Error::InvalidResponse)?;
            self.profile.admit_response(request, &response)?;
            Ok((response_body, response))
        })
        .await
        .map_err(|_| Error::Deadline)?
    }
}

// r[impl kiln.iroh.workflow.peers] r[impl kiln.iroh.workflow.transport]
pub async fn serve_worker(
    endpoint: Endpoint,
    profile: Profile,
    shutdown: impl Future<Output = ()>,
) -> Result<(), Error> {
    profile.admit()?;
    let profile = Arc::new(profile);
    tokio::pin!(shutdown);
    let mut tasks = JoinSet::new();
    let result = loop {
        tokio::select! {
            _ = &mut shutdown => break Ok(()),
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if joined.is_some_and(|result| result.is_err()) {
                    break Err(Error::Transport);
                }
            }
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break Ok(()); };
                if tasks.len() >= profile.transport.limits.maximum_connections as usize {
                    incoming.refuse();
                    continue;
                }
                let profile = profile.clone();
                tasks.spawn(async move {
                    let _outcome = timeout(deadline(&profile), async {
                        let connection = ClosingConnection(incoming.await.map_err(|_| Error::Transport)?);
                        profile.admit_peer(connection.0.remote_id())?;
                        let (mut send, mut recv) = connection.0.accept_bi().await.map_err(|_| Error::Transport)?;
                        let body = read_body(&mut recv, &profile).await?;
                        require_end(&mut recv).await?;
                        let request: ExchangeRequest = serde_json::from_slice(&body).map_err(|_| Error::InvalidRequest)?;
                        profile.admit_request(&request)?;
                        let response_body = forward_local(&profile, &request, &body).await?;
                        write_body(&mut send, &response_body).await?;
                        send.finish().map_err(|_| Error::Transport)?;
                        // Retain the connection until its response has been acknowledged.
                        let _acknowledged = send.stopped().await;
                        Ok::<(), Error>(())
                    }).await;
                });
            }
        }
    };
    stop_tasks(&mut tasks).await;
    // QUIC draining outlives an exchange deadline; do not cancel owned teardown.
    endpoint.close().await;
    result
}

// The original bytes and native identities cross the bridge unchanged. No errors
// here are promoted into a safe retry or a synthetic workflow result.
async fn forward_local(
    profile: &Profile,
    request: &ExchangeRequest,
    body: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut socket = UnixStream::connect(&profile.socket_path)
        .await
        .map_err(|_| Error::Provider)?;
    write_body(&mut socket, body).await?;
    socket.shutdown().await.map_err(|_| Error::Provider)?;
    let response_body = read_body(&mut socket, profile).await?;
    let response: ExchangeResponse =
        serde_json::from_slice(&response_body).map_err(|_| Error::InvalidResponse)?;
    profile.admit_response(request, &response)?;
    Ok(response_body)
}

// r[impl kiln.iroh.workflow.transport] r[impl kiln.iroh.workflow.recovery]
pub async fn serve_proxy(
    endpoint: Endpoint,
    remote: EndpointAddr,
    profile: Profile,
    shutdown: impl Future<Output = ()>,
) -> Result<(), Error> {
    let client = IrohClient::new(endpoint.clone(), remote, profile)?;
    let profile = Arc::clone(&client.profile);
    let listener = UnixListener::bind(&profile.socket_path).map_err(|_| Error::LocalSocket)?;
    let metadata =
        std::fs::symlink_metadata(&profile.socket_path).map_err(|_| Error::LocalSocket)?;
    let _owned_socket = OwnedSocket {
        path: profile.socket_path.clone(),
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    std::fs::set_permissions(&profile.socket_path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| Error::LocalSocket)?;
    tokio::pin!(shutdown);
    let mut tasks = JoinSet::new();
    let result = loop {
        tokio::select! {
            _ = &mut shutdown => break Ok(()),
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if joined.is_some_and(|result| result.is_err()) {
                    break Err(Error::Transport);
                }
            }
            accepted = listener.accept() => {
                let (mut socket, _) = match accepted {
                    Ok(accepted) => accepted,
                    Err(_) => break Err(Error::LocalSocket),
                };
                let user_id = match socket.peer_cred() {
                    Ok(credentials) => credentials.uid(),
                    Err(_) => continue,
                };
                if !profile.transport.peers.allowed_user_ids.contains(&user_id)
                    || tasks.len() >= profile.transport.limits.maximum_connections as usize {
                    continue;
                }
                let client = client.clone();
                tasks.spawn(async move {
                    let _outcome = timeout(deadline(&client.profile), async {
                        let body = read_body(&mut socket, &client.profile).await?;
                        require_end(&mut socket).await?;
                        let request: ExchangeRequest = serde_json::from_slice(&body).map_err(|_| Error::InvalidRequest)?;
                        client.profile.admit_request(&request)?;
                        let (response_body, _response) = client.exchange_body(&request, &body).await?;
                        write_body(&mut socket, &response_body).await?;
                        socket.shutdown().await.map_err(|_| Error::LocalSocket)
                    }).await;
                    // Dropping the socket without a response preserves UnknownAfterWrite
                    // in the existing native consumer, including ambiguous provider errors.
                });
            }
        }
    };
    stop_tasks(&mut tasks).await;
    endpoint.close().await;
    result
}

fn deadline(profile: &Profile) -> Duration {
    Duration::from_millis(profile.exchange_timeout_ms)
}

fn check_length(length: usize, profile: &Profile) -> Result<(), Error> {
    if length == 0 || length > profile.transport.limits.maximum_frame_bytes as usize {
        Err(Error::FrameTooLarge)
    } else {
        Ok(())
    }
}

async fn read_body(
    reader: &mut (impl AsyncRead + Unpin),
    profile: &Profile,
) -> Result<Vec<u8>, Error> {
    let length = reader.read_u32().await.map_err(|_| Error::Transport)? as usize;
    check_length(length, profile)?;
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|_| Error::Transport)?;
    Ok(body)
}

async fn write_body(writer: &mut (impl AsyncWrite + Unpin), body: &[u8]) -> Result<(), Error> {
    let length = u32::try_from(body.len()).map_err(|_| Error::FrameTooLarge)?;
    writer
        .write_u32(length)
        .await
        .map_err(|_| Error::Transport)?;
    writer.write_all(body).await.map_err(|_| Error::Transport)
}

async fn require_end(reader: &mut (impl AsyncRead + Unpin)) -> Result<(), Error> {
    let mut extra = [0];
    match reader.read(&mut extra).await {
        Ok(0) => Ok(()),
        _ => Err(Error::InvalidRequest),
    }
}

async fn stop_tasks(tasks: &mut JoinSet<()>) {
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
}

struct ClosingConnection(iroh::endpoint::Connection);

impl Drop for ClosingConnection {
    fn drop(&mut self) {
        self.0
            .close(CONNECTION_CLOSE_CODE.into(), CONNECTION_CLOSE_REASON);
    }
}

struct OwnedSocket {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Drop for OwnedSocket {
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.path)
            .is_ok_and(|metadata| metadata.dev() == self.device && metadata.ino() == self.inode)
        {
            let _cleanup = std::fs::remove_file(&self.path);
        }
    }
}
