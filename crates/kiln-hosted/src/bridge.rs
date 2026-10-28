use crate::{EXCHANGE_TIMEOUT_MS, MAX_CONNECTIONS, MAX_FRAME_BYTES, Request};
use base64::Engine;
use iroh::{Endpoint, EndpointAddr};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookBinding {
    pub project: String,
    pub connection: String,
    pub generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub network: crate::transport::NetworkProfile,
    pub remote: EndpointAddr,
    pub organization: String,
    pub listen_address: SocketAddr,
    #[serde(default)]
    pub webhook: Option<WebhookBinding>,
}
impl Profile {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let bytes = crate::transport::private_bytes(path, MAX_FRAME_BYTES)?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "client profile malformed")?;
        if profile.schema != crate::SCHEMA
            || !crate::identifier(&profile.organization)
            || !profile.listen_address.ip().is_loopback()
        {
            return Err("client profile or loopback binding rejected".into());
        }
        if profile.webhook.as_ref().is_some_and(|w| {
            !crate::identifier(&w.project) || !crate::identifier(&w.connection) || w.generation <= 0
        }) {
            return Err("webhook binding rejected".into());
        }
        profile.network.admit()?;
        Ok(profile)
    }
}
const MAX_HEADERS: usize = 16_384;
const SECURITY: &str = "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n";

async fn response(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n{SECURITY}\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .map_err(|_| "HTTP write failed")?;
    stream
        .write_all(body)
        .await
        .map_err(|_| "HTTP body failed")?;
    stream
        .shutdown()
        .await
        .map_err(|_| "HTTP shutdown failed".into())
}

async fn exchange(
    mut stream: TcpStream,
    endpoint: Endpoint,
    remote: EndpointAddr,
    organization: String,
    address: SocketAddr,
    webhook: Option<WebhookBinding>,
) -> Result<(), String> {
    let mut bytes = Vec::new();
    let end = loop {
        if bytes.len() >= MAX_HEADERS {
            return response(&mut stream, 400, "text/plain", b"Header bound exceeded").await;
        }
        let mut chunk = [0; 1024];
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|_| "HTTP read failed")?;
        if n == 0 {
            return Err("HTTP header incomplete".into());
        }
        bytes.extend_from_slice(chunk.get(..n).ok_or("read length rejected")?);
        if let Some(offset) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            break offset + 4;
        }
    };
    if end > MAX_HEADERS {
        return response(&mut stream, 400, "text/plain", b"Header bound exceeded").await;
    }
    let headers = std::str::from_utf8(bytes.get(..end).ok_or("header range rejected")?)
        .map_err(|_| "HTTP header encoding rejected")?;
    let mut lines = headers.split("\r\n");
    let first = lines.next().ok_or("HTTP request line missing")?;
    let parts = first.split(' ').collect::<Vec<_>>();
    let [method, path, version] = parts.as_slice() else {
        return Err("HTTP request line malformed".into());
    };
    if *version != "HTTP/1.1" {
        return response(&mut stream, 400, "text/plain", b"HTTP version rejected").await;
    }
    let mut fields = std::collections::BTreeMap::new();
    for line in lines.filter(|v| !v.is_empty()) {
        let (key, value) = line.split_once(':').ok_or("header malformed")?;
        if fields
            .insert(key.to_ascii_lowercase(), value.trim())
            .is_some()
        {
            return response(&mut stream, 400, "text/plain", b"Duplicate header rejected").await;
        }
    }
    let host = address.to_string();
    let origin = format!("http://{host}");
    if fields.get("host").copied() != Some(host.as_str())
        || fields.contains_key("transfer-encoding")
        || fields.get("origin").is_some_and(|v| *v != origin)
    {
        return response(
            &mut stream,
            403,
            "text/plain",
            b"Loopback host/origin rejected",
        )
        .await;
    }
    if *method == "GET" {
        if fields.get("content-length").is_some_and(|v| *v != "0") {
            return response(&mut stream, 400, "text/plain", b"GET body rejected").await;
        }
        return match *path {
            "/" => {
                response(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    include_bytes!("web.html"),
                )
                .await
            }
            "/app.js" => {
                response(
                    &mut stream,
                    200,
                    "text/javascript; charset=utf-8",
                    include_bytes!("web.js"),
                )
                .await
            }
            "/app.css" => {
                response(
                    &mut stream,
                    200,
                    "text/css; charset=utf-8",
                    include_bytes!("web.css"),
                )
                .await
            }
            "/.well-known/openid-configuration" | "/jwks.json" => {
                let request = Request {
                    schema: crate::SCHEMA.into(),
                    organization: organization.clone(),
                    request_id: crate::identity(&(
                        endpoint.id().to_string(),
                        crate::now_ms()?,
                        *path,
                    ))?,
                    action: if *path == "/jwks.json" {
                        "secrets.jwks"
                    } else {
                        "secrets.discovery"
                    }
                    .into(),
                    input: serde_json::json!({}),
                };
                match crate::transport::call(&endpoint, &remote, &request).await {
                    Ok(reply) if reply.error.is_none() => {
                        let body = serde_json::to_vec(&reply.result)
                            .map_err(|_| "issuer metadata encoding failed")?;
                        response(&mut stream, 200, "application/json", &body).await
                    }
                    _ => {
                        response(
                            &mut stream,
                            503,
                            "application/json",
                            b"{\"error\":\"admitted issuer unavailable\"}",
                        )
                        .await
                    }
                }
            }
            "/session" => {
                let body=serde_json::to_vec(&serde_json::json!({"organization":organization,"peer":endpoint.id().to_string(),"server":remote.id.to_string(),"transport":"iroh"})).map_err(|_|"session encoding failed")?;
                response(&mut stream, 200, "application/json", &body).await
            }
            _ => response(&mut stream, 404, "text/plain", b"Not found").await,
        };
    }
    let source_webhook = *path == "/github/webhook" && webhook.is_some();
    if *method != "POST" || (*path != "/rpc" && !source_webhook) {
        return response(&mut stream, 404, "text/plain", b"Not found").await;
    }
    if fields.get("content-type").copied() != Some("application/json")
        || (!source_webhook && fields.get("x-kiln-local-client").copied() != Some("1"))
    {
        return response(
            &mut stream,
            403,
            "text/plain",
            b"JSON client or signed source delivery required",
        )
        .await;
    }
    let delivery = if source_webhook {
        Some(
            serde_json::json!({"delivery":fields.get("x-github-delivery").ok_or("delivery header required")?,"event":fields.get("x-github-event").ok_or("event header required")?,"signature":fields.get("x-hub-signature-256").ok_or("signature header required")?}),
        )
    } else {
        None
    };
    let length = fields
        .get("content-length")
        .ok_or("content length required")?
        .parse::<usize>()
        .map_err(|_| "content length malformed")?;
    if length == 0 || length > MAX_FRAME_BYTES {
        return response(
            &mut stream,
            400,
            "text/plain",
            b"Request body exceeds bound",
        )
        .await;
    }
    let wanted = end.checked_add(length).ok_or("HTTP length overflow")?;
    while bytes.len() < wanted {
        let mut chunk = [0; 4096];
        let remaining = wanted - bytes.len();
        let size = remaining.min(chunk.len());
        let n = stream
            .read(chunk.get_mut(..size).ok_or("body read rejected")?)
            .await
            .map_err(|_| "body read failed")?;
        if n == 0 {
            return Err("body incomplete".into());
        }
        bytes.extend_from_slice(chunk.get(..n).ok_or("body length rejected")?);
    }
    if bytes.len() != wanted {
        return response(&mut stream, 400, "text/plain", b"Pipelining rejected").await;
    }
    let body = bytes.get(end..).ok_or("body range rejected")?;
    let request: Request = if let (Some(headers), Some(binding)) = (delivery, webhook) {
        let request_id = crate::identity(&(
            "kiln.hosted.webhook.v1",
            &binding.project,
            &binding.connection,
            binding.generation,
            &headers,
            blake3::hash(body).to_hex().to_string(),
        ))?;
        Request {
            schema: crate::SCHEMA.into(),
            organization: organization.clone(),
            request_id,
            action: "connections.ingest".into(),
            input: serde_json::json!({"project":binding.project,"connection":binding.connection,"generation":binding.generation,"headers":headers,"body_base64":base64::engine::general_purpose::STANDARD.encode(body)}),
        }
    } else {
        serde_json::from_slice(body).map_err(|_| "RPC JSON rejected")?
    };
    if request.organization != organization {
        return response(
            &mut stream,
            403,
            "text/plain",
            b"Bridge organization mismatch",
        )
        .await;
    }
    match crate::transport::call(&endpoint,&remote,&request).await{
        Ok(result)=>{let status=if source_webhook&&result.error.is_some(){400}else{200};let body=serde_json::to_vec(&result).map_err(|_|"RPC encoding failed")?;response(&mut stream,status,"application/json",&body).await},
        Err(_)=>response(&mut stream,503,"application/json",b"{\"error\":\"Iroh exchange unavailable; inspect the same request identity before retry\"}").await,
    }
}

pub async fn serve(
    profile: Profile,
    endpoint: Endpoint,
    shutdown: impl Future<Output = ()>,
) -> Result<(), String> {
    if !profile.listen_address.ip().is_loopback() {
        return Err("browser bridge requires loopback".into());
    }
    let listener = TcpListener::bind(profile.listen_address)
        .await
        .map_err(|_| "browser bridge bind failed")?;
    let address = listener
        .local_addr()
        .map_err(|_| "bridge address unavailable")?;
    println!("http://{address}");
    let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let mut tasks = tokio::task::JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _=&mut shutdown=>break,
            _=tasks.join_next(),if !tasks.is_empty()=>{},
            accepted=listener.accept()=>{let(stream,peer)=accepted.map_err(|_|"bridge accept failed")?;if !peer.ip().is_loopback(){continue;}let Ok(permit)=Arc::clone(&permits).try_acquire_owned()else{continue;};let endpoint=endpoint.clone();let remote=profile.remote.clone();let org=profile.organization.clone();let webhook=profile.webhook.clone();tasks.spawn(async move{let _permit=permit;let _result=tokio::time::timeout(Duration::from_millis(EXCHANGE_TIMEOUT_MS+1000),exchange(stream,endpoint,remote,org,address,webhook)).await;});}
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    endpoint.close().await;
    Ok(())
}
