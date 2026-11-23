//! VM-specific bounded CH Unix/Iroh stream bridge. The caller supplies the
//! admitted Aspen reservation; this module does not allocate placement, create
//! work, or interpret transport completion as VMM cutover or source retirement.
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use iroh::endpoint::{Connection, QuicTransportConfig, RecvStream, SendStream, VarInt, presets};
use iroh::{Endpoint, RelayMode, SecretKey, TransportAddr};
use kiln_microvm::bridge::{BridgeEndpoint, BridgeExpectation, MigrationBridgePlan, admit_bridge};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;

pub const MIGRATION_ALPN: &[u8] = b"kiln/ch-migration/precopy/1";
const BUFFER_BYTES: usize = 65_536;
const STREAM_ABORT: u32 = 1;
const CURRENTNESS_POLL_MS: u64 = 100;
const ENDPOINT_BIND_DEADLINE_MS: u64 = 10_000;
const HEADER_BYTES: usize = 80;
const HEADER_MAGIC: &[u8; 16] = b"KILN-CH-BRIDGE1\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Source,
    Target,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeError {
    Denied,
    ProtectedSocket,
    /// Once the provider consumes the operation, every transport/socket/deadline
    /// error is uncertain. No error code here authorizes target stop or retry.
    ReconcileRequired,
}

#[derive(Clone, Debug)]
pub struct StreamReceipt {
    pub local_to_peer_bytes: u64,
    pub peer_to_local_bytes: u64,
    pub elapsed: Duration,
    pub initial_direct_address: std::net::SocketAddr,
    pub final_direct_address: std::net::SocketAddr,
    /// This receipt explicitly does not mean durable checkpoint or VM handoff.
    pub placement_committed: bool,
}

/// A real provider guard must hold current operation/attempt/placement and
/// resource authority until it is dropped. `consume` must durably admit exactly
/// one byte stream, refusing consumed/replayed/uncertain operations even after
/// endpoint/controller restart. A local operation marker is not this authority.
pub trait MigrationReservation {
    fn expected(&self) -> BridgeExpectation;
    fn consume(&mut self, plan: &MigrationBridgePlan) -> Result<(), BridgeError>;
    fn check_current(&mut self) -> Result<(), BridgeError>;
    /// Exact local VMM UID/PID resolved from its admitted process/cgroup.
    fn local_process(&self) -> (u32, u32);
    fn observe(&mut self, receipt: Option<&StreamReceipt>) -> Result<(), BridgeError>;
}

/// Dedicated admitted host migration endpoint. Relay/discovery fallback is
/// disabled in the builder, rather than merely sampled after RAM was relayed.
#[derive(Debug)]
pub struct DirectMigrationEndpoint(Endpoint);
impl DirectMigrationEndpoint {
    pub async fn bind(
        address: std::net::SocketAddr,
        key: SecretKey,
        reservation: &mut impl MigrationReservation,
    ) -> Result<Self, BridgeError> {
        reservation.check_current()?;
        if address.ip().is_unspecified() || address.ip().is_multicast() {
            return Err(BridgeError::Denied);
        }
        let transport = QuicTransportConfig::builder()
            .max_concurrent_bidi_streams(1_u32.into())
            .max_concurrent_uni_streams(0_u32.into())
            .stream_receive_window(VarInt::from_u32(BUFFER_BYTES as u32))
            .receive_window(VarInt::from_u32((BUFFER_BYTES * 2) as u32))
            .send_window((BUFFER_BYTES * 2) as u64)
            .datagram_receive_buffer_size(None)
            .build();
        // The minimal preset installs only the ring crypto provider: no address
        // lookup service, and relays stay disabled.
        let builder = Endpoint::builder(presets::Minimal)
            .secret_key(key)
            .relay_mode(RelayMode::Disabled)
            .transport_config(transport)
            .alpns(vec![MIGRATION_ALPN.to_vec()])
            .clear_ip_transports();
        let builder = match address {
            std::net::SocketAddr::V4(address) => builder.bind_addr(address).and_then(|builder| {
                builder.bind_addr(std::net::SocketAddrV6::new(
                    std::net::Ipv6Addr::LOCALHOST,
                    0,
                    0,
                    0,
                ))
            }),
            std::net::SocketAddr::V6(address) => builder.bind_addr(address).and_then(|builder| {
                builder.bind_addr(std::net::SocketAddrV4::new(
                    std::net::Ipv4Addr::LOCALHOST,
                    0,
                ))
            }),
        }
        .map_err(|_| BridgeError::Denied)?;
        tokio::time::timeout(
            Duration::from_millis(ENDPOINT_BIND_DEADLINE_MS),
            builder.bind(),
        )
        .await
        .map_err(|_| BridgeError::Denied)?
        .map(Self)
        .map_err(|_| BridgeError::Denied)
    }
    pub fn endpoint(&self) -> &Endpoint {
        &self.0
    }
    /// The selected path's address when every open path of `connection` is a
    /// direct IP path; any relay or custom-transport path denies.
    fn direct_address(&self, connection: &Connection) -> Result<std::net::SocketAddr, BridgeError> {
        let paths = connection.paths();
        let mut selected = None;
        for path in &paths {
            let TransportAddr::Ip(address) = path.remote_addr() else {
                return Err(BridgeError::Denied);
            };
            if path.is_selected() {
                selected = Some(*address);
            }
        }
        selected.ok_or(BridgeError::Denied)
    }
}

#[derive(Debug)]
pub struct BridgeContext<'a, R> {
    pub endpoint: &'a DirectMigrationEndpoint,
    pub connection: &'a Connection,
    pub plan: &'a MigrationBridgePlan,
    pub side: Side,
    pub reservation: &'a mut R,
    pub cancelled: watch::Receiver<bool>,
    pub maximum_bytes_per_second: u64,
}

fn protected_directory(path: &Path) -> Result<(), BridgeError> {
    let parent = path.parent().ok_or(BridgeError::ProtectedSocket)?;
    let owner = fs::metadata("/proc/self")
        .map_err(|_| BridgeError::ProtectedSocket)?
        .uid();
    let metadata = fs::symlink_metadata(parent).map_err(|_| BridgeError::ProtectedSocket)?;
    if !metadata.is_dir()
        || metadata.uid() != owner
        || metadata.mode() & 0o777 != 0o700
        || fs::canonicalize(parent).map_err(|_| BridgeError::ProtectedSocket)? != parent
    {
        return Err(BridgeError::ProtectedSocket);
    }
    Ok(())
}

/// Accept only a fully established authenticated connection, never a 0-RTT
/// connection type. The exact peer and negotiated ALPN are checked before any
/// protected socket is inspected/opened. The endpoint must be configured with
/// max one bidirectional stream, zero unidirectional/datagram streams and an
/// explicitly admitted direct/relay policy. No relay policy is inferred here.
// r[impl kiln.hosted.mobility.runtime]
pub async fn bridge<R: MigrationReservation>(
    mut send: SendStream,
    mut receive: RecvStream,
    context: BridgeContext<'_, R>,
) -> Result<StreamReceipt, BridgeError> {
    let BridgeContext {
        endpoint,
        connection,
        plan,
        side,
        reservation,
        mut cancelled,
        maximum_bytes_per_second,
    } = context;
    let expected_local = match side {
        Side::Source => plan.source_endpoint,
        Side::Target => plan.target_endpoint,
    };
    if endpoint.0.id().as_bytes() != &expected_local.id {
        return Err(BridgeError::Denied);
    }
    let initial_direct_address = endpoint.direct_address(connection)?;
    let peer = BridgeEndpoint {
        id: *connection.remote_id().as_bytes(),
    };
    let expected_peer = match side {
        Side::Source => plan.target_endpoint,
        Side::Target => plan.source_endpoint,
    };
    if connection.alpn() != MIGRATION_ALPN
        || peer != expected_peer
        || receive.is_0rtt()
        || maximum_bytes_per_second == 0
        || *cancelled.borrow()
    {
        return Err(BridgeError::Denied);
    }
    let plan_identity =
        admit_bridge(plan, &reservation.expected()).map_err(|_| BridgeError::Denied)?;
    reservation.check_current()?;
    let path = Path::new(match side {
        Side::Source => &plan.source_socket,
        Side::Target => &plan.target_socket,
    });
    protected_directory(path)?;
    // Consume BEFORE the first local protected-resource use. Any subsequent
    // uncertainty retains this consumed operation through the provider ledger.
    reservation.consume(plan)?;
    let started = Instant::now();
    let run = async {
        reservation
            .check_current()
            .map_err(|_| BridgeError::ReconcileRequired)?;
        let mut header = [0_u8; HEADER_BYTES];
        header
            .get_mut(..16)
            .ok_or(BridgeError::ReconcileRequired)?
            .copy_from_slice(HEADER_MAGIC);
        header
            .get_mut(16..)
            .ok_or(BridgeError::ReconcileRequired)?
            .copy_from_slice(plan_identity.as_str().as_bytes());
        let mut peer_header = [0_u8; HEADER_BYTES];
        match side {
            Side::Source => {
                send.write_all(&header)
                    .await
                    .map_err(|_| BridgeError::ReconcileRequired)?;
                receive
                    .read_exact(&mut peer_header)
                    .await
                    .map_err(|_| BridgeError::ReconcileRequired)?;
            }
            Side::Target => {
                receive
                    .read_exact(&mut peer_header)
                    .await
                    .map_err(|_| BridgeError::ReconcileRequired)?;
                if peer_header != header {
                    return Err(BridgeError::ReconcileRequired);
                }
                send.write_all(&header)
                    .await
                    .map_err(|_| BridgeError::ReconcileRequired)?;
            }
        }
        if peer_header != header {
            return Err(BridgeError::ReconcileRequired);
        }
        let socket = match side {
            Side::Source => {
                // Never unlink a previous listener: it may belong to an in-flight
                // operation or an independently running CH instance.
                let listener =
                    UnixListener::bind(path).map_err(|_| BridgeError::ReconcileRequired)?;
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                    .map_err(|_| BridgeError::ReconcileRequired)?;
                listener
                    .accept()
                    .await
                    .map_err(|_| BridgeError::ReconcileRequired)?
                    .0
            }
            Side::Target => {
                let metadata =
                    fs::symlink_metadata(path).map_err(|_| BridgeError::ReconcileRequired)?;
                let owner = fs::metadata("/proc/self")
                    .map_err(|_| BridgeError::ReconcileRequired)?
                    .uid();
                if !metadata.file_type().is_socket()
                    || metadata.uid() != owner
                    || metadata.mode() & 0o077 != 0
                {
                    return Err(BridgeError::ReconcileRequired);
                }
                UnixStream::connect(path)
                    .await
                    .map_err(|_| BridgeError::ReconcileRequired)?
            }
        };
        let credentials = socket
            .peer_cred()
            .map_err(|_| BridgeError::ReconcileRequired)?;
        let (expected_uid, expected_pid) = reservation.local_process();
        if credentials.uid() != expected_uid
            || credentials.pid().and_then(|pid| u32::try_from(pid).ok()) != Some(expected_pid)
        {
            return Err(BridgeError::ReconcileRequired);
        }
        reservation
            .check_current()
            .map_err(|_| BridgeError::ReconcileRequired)?;
        let (mut local_read, mut local_write) = socket.into_split();
        let used = AtomicU64::new(0);
        let copying = async {
            tokio::try_join!(
                pump(
                    &mut local_read,
                    &mut send,
                    &used,
                    plan.max_stream_bytes,
                    maximum_bytes_per_second,
                    started
                ),
                pump(
                    &mut receive,
                    &mut local_write,
                    &used,
                    plan.max_stream_bytes,
                    maximum_bytes_per_second,
                    started
                ),
            )
        };
        tokio::pin!(copying);
        let mut currentness = tokio::time::interval(Duration::from_millis(CURRENTNESS_POLL_MS));
        let (outbound, inbound) = loop {
            tokio::select! {
                result = &mut copying => break result?,
                _ = currentness.tick() => {
                    reservation.check_current().map_err(|_| BridgeError::ReconcileRequired)?;
                    endpoint.direct_address(connection).map_err(|_| BridgeError::ReconcileRequired)?;
                },
            }
        };
        reservation
            .check_current()
            .map_err(|_| BridgeError::ReconcileRequired)?;
        Ok(StreamReceipt {
            local_to_peer_bytes: outbound,
            peer_to_local_bytes: inbound,
            elapsed: started.elapsed(),
            initial_direct_address,
            final_direct_address: endpoint
                .direct_address(connection)
                .map_err(|_| BridgeError::ReconcileRequired)?,
            placement_committed: false,
        })
    };
    let outcome = tokio::select! {
        result = tokio::time::timeout(Duration::from_millis(plan.deadline_ms), run) => result.unwrap_or(Err(BridgeError::ReconcileRequired)),
        _ = async {
            loop {
                if *cancelled.borrow() {
                    break;
                }
                if cancelled.changed().await.is_err() {
                    break;
                }
            }
        } => Err(BridgeError::ReconcileRequired),
    };
    match outcome {
        Ok(receipt) => {
            reservation
                .observe(Some(&receipt))
                .map_err(|_| BridgeError::ReconcileRequired)?;
            Ok(receipt)
        }
        Err(_) => {
            let _reset = send.reset(VarInt::from_u32(STREAM_ABORT));
            let _stop = receive.stop(VarInt::from_u32(STREAM_ABORT));
            let _observation = reservation.observe(None);
            Err(BridgeError::ReconcileRequired)
        }
    }
}

async fn pump(
    input: &mut (impl AsyncRead + Unpin),
    output: &mut (impl AsyncWrite + Unpin),
    used: &AtomicU64,
    maximum: u64,
    bytes_per_second: u64,
    started: Instant,
) -> Result<u64, BridgeError> {
    let mut buffer = [0_u8; BUFFER_BYTES];
    let mut copied = 0_u64;
    loop {
        let count = input
            .read(&mut buffer)
            .await
            .map_err(|_| BridgeError::ReconcileRequired)?;
        if count == 0 {
            output
                .shutdown()
                .await
                .map_err(|_| BridgeError::ReconcileRequired)?;
            return Ok(copied);
        }
        let count_u64 = count as u64;
        let previous = used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(count_u64)
                    .filter(|total| *total <= maximum)
            })
            .map_err(|_| BridgeError::ReconcileRequired)?;
        let total = previous + count_u64;
        // Aggregate both directions under one bounded bandwidth budget. The
        // outer deadline/cancellation also covers backpressure and pacing.
        let paced = Duration::from_secs_f64(total as f64 / bytes_per_second as f64);
        if let Some(left) = paced.checked_sub(started.elapsed()) {
            tokio::time::sleep(left).await;
        }
        output
            .write_all(buffer.get(..count).ok_or(BridgeError::ReconcileRequired)?)
            .await
            .map_err(|_| BridgeError::ReconcileRequired)?;
        copied += count_u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn bounded_copy_preserves_bytes_and_half_close() {
        let (mut writer, mut reader) = tokio::io::duplex(BUFFER_BYTES);
        let (mut output, mut observer) = tokio::io::duplex(BUFFER_BYTES);
        let bytes = b"CH-protocol-bytes";
        writer
            .write_all(bytes)
            .await
            .unwrap_or_else(|_| unreachable!());
        writer.shutdown().await.unwrap_or_else(|_| unreachable!());
        let used = AtomicU64::new(0);
        assert_eq!(
            pump(
                &mut reader,
                &mut output,
                &used,
                100,
                1_000_000,
                Instant::now()
            )
            .await,
            Ok(bytes.len() as u64)
        );
        let mut actual = Vec::new();
        observer
            .read_to_end(&mut actual)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(actual, bytes);
    }
    #[tokio::test]
    async fn total_budget_is_shared_and_oversize_is_uncertain() {
        let (mut writer, mut reader) = tokio::io::duplex(BUFFER_BYTES);
        let (mut output, _observer) = tokio::io::duplex(BUFFER_BYTES);
        writer
            .write_all(b"123456")
            .await
            .unwrap_or_else(|_| unreachable!());
        writer.shutdown().await.unwrap_or_else(|_| unreachable!());
        let used = AtomicU64::new(7);
        assert_eq!(
            pump(
                &mut reader,
                &mut output,
                &used,
                10,
                1_000_000,
                Instant::now()
            )
            .await,
            Err(BridgeError::ReconcileRequired)
        );
        assert_eq!(used.load(Ordering::Acquire), 7);
    }
}
