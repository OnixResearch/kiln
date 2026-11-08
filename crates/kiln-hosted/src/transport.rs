use crate::{EXCHANGE_TIMEOUT_MS, MAX_CONNECTIONS, MAX_FRAME_BYTES, Request, Response};
use iroh::endpoint::{TransportConfig, VarInt};
use iroh::{Endpoint, EndpointAddr, RelayMap, RelayMode, RelayUrl, SecretKey};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::task::JoinSet;

pub const ALPN: &[u8] = b"kiln.customer.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkProfile {
    pub key_file: PathBuf,
    pub bind_address: SocketAddr,
    pub relay_urls: Vec<String>,
}
impl NetworkProfile {
    // r[impl kiln.hosted.iroh]
    pub fn admit(&self) -> Result<(), String> {
        if !self.key_file.is_absolute() || self.relay_urls.len() > 8 {
            return Err("network profile rejected".into());
        }
        for value in &self.relay_urls {
            if !value.starts_with("https://")
                || value.contains(['@', '?', '#'])
                || value.len() > 2048
                || value.parse::<RelayUrl>().is_err()
            {
                return Err("explicit HTTPS relay required".into());
            }
        }
        Ok(())
    }
}

pub fn private_bytes(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "private file unavailable")?;
    let meta = file
        .metadata()
        .map_err(|_| "private file metadata unavailable")?;
    let uid = std::fs::metadata("/proc/self")
        .map_err(|_| "process owner unavailable")?
        .uid();
    if !meta.is_file()
        || meta.uid() != uid
        || meta.mode() & 0o077 != 0
        || meta.nlink() != 1
        || meta.len() > maximum as u64
    {
        return Err("private owned regular file required".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private file read failed")?;
    if bytes.len() > maximum {
        return Err("private file exceeds bound".into());
    }
    Ok(bytes)
}

pub fn keygen(path: &Path) -> Result<String, String> {
    let mut bytes = zeroize::Zeroizing::new([0_u8; 32]);
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(bytes.as_mut()))
        .map_err(|_| "key entropy unavailable")?;
    let key = SecretKey::from_bytes(&bytes);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "key target exists or unavailable")?;
    file.write_all(bytes.as_ref())
        .and_then(|()| file.sync_all())
        .map_err(|_| "key persistence failed")?;
    Ok(key.public().to_string())
}

pub async fn bind(profile: &NetworkProfile) -> Result<Endpoint, String> {
    profile.admit()?;
    let bytes = zeroize::Zeroizing::new(private_bytes(&profile.key_file, 32)?);
    let seed: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "key must contain 32 raw bytes")?;
    let relays = profile
        .relay_urls
        .iter()
        .map(|v| {
            v.parse::<RelayUrl>()
                .map_err(|_| "relay rejected".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let window =
        VarInt::from_u64((MAX_FRAME_BYTES + 4) as u64).map_err(|_| "receive window rejected")?;
    let mut transport = TransportConfig::default();
    transport
        .max_concurrent_bidi_streams(1_u32.into())
        .max_concurrent_uni_streams(0_u32.into())
        .stream_receive_window(window)
        .receive_window(window)
        .send_window((MAX_FRAME_BYTES + 4) as u64)
        .datagram_receive_buffer_size(None)
        .max_idle_timeout(Some(
            Duration::from_millis(EXCHANGE_TIMEOUT_MS)
                .try_into()
                .map_err(|_| "idle timeout rejected")?,
        ));
    let mut builder = Endpoint::builder()
        .secret_key(SecretKey::from_bytes(&seed))
        .clear_discovery()
        .relay_mode(if relays.is_empty() {
            RelayMode::Disabled
        } else {
            RelayMode::Custom(RelayMap::from_iter(relays))
        })
        .transport_config(transport)
        .alpns(vec![ALPN.to_vec()]);
    builder = match profile.bind_address {
        SocketAddr::V4(v) => {
            builder
                .bind_addr_v4(v)
                .bind_addr_v6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0))
        }
        SocketAddr::V6(v) => builder
            .bind_addr_v6(v)
            .bind_addr_v4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
    };
    tokio::time::timeout(Duration::from_millis(EXCHANGE_TIMEOUT_MS), builder.bind())
        .await
        .map_err(|_| "endpoint bind deadline")?
        .map_err(|_| "endpoint bind failed".into())
}

async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> Result<Vec<u8>, String> {
    let size = reader
        .read_u32()
        .await
        .map_err(|_| "frame header unavailable")? as usize;
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err("frame outside bound".into());
    }
    let mut body = vec![0; size];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|_| "frame incomplete")?;
    let mut extra = [0];
    if reader
        .read(&mut extra)
        .await
        .map_err(|_| "frame end unavailable")?
        != 0
    {
        return Err("trailing frame bytes rejected".into());
    }
    Ok(body)
}
async fn write_frame(writer: &mut (impl AsyncWrite + Unpin), body: &[u8]) -> Result<(), String> {
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err("response exceeds bound".into());
    }
    writer
        .write_u32(u32::try_from(body.len()).map_err(|_| "frame length rejected")?)
        .await
        .map_err(|_| "frame write failed")?;
    writer
        .write_all(body)
        .await
        .map_err(|_| "frame write failed".into())
}

pub async fn call(
    endpoint: &Endpoint,
    remote: &EndpointAddr,
    request: &Request,
) -> Result<Response, String> {
    request.admit()?;
    let body = serde_json::to_vec(request).map_err(|_| "request encoding failed")?;
    tokio::time::timeout(Duration::from_millis(EXCHANGE_TIMEOUT_MS), async {
        let connection = endpoint
            .connect(remote.clone(), ALPN)
            .await
            .map_err(|_| "customer connection unavailable")?;
        if connection.remote_id() != remote.id {
            return Err("server identity mismatch".into());
        }
        let result = async {
            let (mut send, mut recv) = connection
                .open_bi()
                .await
                .map_err(|_| "request stream unavailable")?;
            write_frame(&mut send, &body).await?;
            send.finish().map_err(|_| "request finish failed")?;
            let bytes = read_frame(&mut recv).await?;
            let response: Response =
                serde_json::from_slice(&bytes).map_err(|_| "response contract rejected")?;
            if response.schema != crate::SCHEMA || response.request_id != request.request_id {
                return Err("response identity mismatch".into());
            }
            Ok(response)
        }
        .await;
        connection.close(0_u32.into(), b"customer exchange ended");
        result
    })
    .await
    .map_err(|_| "customer exchange timed out; recover the same request identity".to_owned())?
}

// r[impl kiln.hosted.iroh]
pub async fn serve(
    endpoint: Endpoint,
    service: Arc<Mutex<crate::service::Service>>,
    shutdown: impl Future<Output = ()>,
) -> Result<(), String> {
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _=&mut shutdown=>break,
            joined=tasks.join_next(),if !tasks.is_empty()=>{if joined.is_some_and(|v|v.is_err()){return Err("customer handler failed".into());}},
            incoming=endpoint.accept()=>{
                let Some(incoming)=incoming else{break;};
                if tasks.len()>=MAX_CONNECTIONS{incoming.refuse();continue;}
                let service=Arc::clone(&service);
                tasks.spawn(async move{
                    let _outcome=tokio::time::timeout(Duration::from_millis(EXCHANGE_TIMEOUT_MS),async{
                        let connection=incoming.await.map_err(|_|"connection failed")?;
                        let peer=connection.remote_id().to_string();
                        let(mut send,mut recv)=connection.accept_bi().await.map_err(|_|"stream unavailable")?;
                        let body=read_frame(&mut recv).await?;
                        let request:Request=serde_json::from_slice(&body).map_err(|_|"request JSON rejected")?;
                        let response=tokio::task::spawn_blocking(move||{
                            let mut session=service.lock().map_err(|_|"service lock unavailable".to_owned())?.fork()?;
                            Ok::<Response,String>(session.handle(&peer,request))
                        }).await.map_err(|_|"service task failed")??;
                        let bytes=serde_json::to_vec(&response).map_err(|_|"response encoding failed")?;
                        write_frame(&mut send,&bytes).await?;send.finish().map_err(|_|"response finish failed")?;
                        let _ack=send.stopped().await;
                        Ok::<(),String>(())
                    }).await;
                });
            }
        }
    }
    // Accepted synchronous effects finish and retain their durable receipts before exit.
    while tasks.join_next().await.is_some() {}
    endpoint.close().await;
    Ok(())
}
