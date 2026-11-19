//! HTTP listeners: one accept thread per configured address and one bounded
//! thread per connection. Each connection carries one request.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;

use crate::api::Service;
use crate::http::{Response, read_request, write_response};
use crate::log;

/// Concurrent connections served; more are answered 503 at once.
pub const MAX_CONNECTIONS: usize = 32;
/// Read and write deadline of one connection.
pub const CONNECTION_TIMEOUT_MS: u64 = 10_000;

/// Bind every address before serving any, so a bad address fails start-up.
///
/// # Errors
/// Returns the first bind failure.
pub fn bind(addresses: &[SocketAddr]) -> Result<Vec<TcpListener>, String> {
    addresses
        .iter()
        .map(|address| {
            TcpListener::bind(address)
                .map_err(|error| format!("cannot listen on {address}: {error}"))
        })
        .collect()
}

/// Serve every listener on its own thread.
///
/// # Errors
/// Returns a diagnostic when a thread cannot be started.
pub fn spawn(
    listeners: Vec<TcpListener>,
    service: &Arc<Service>,
) -> Result<Vec<JoinHandle<()>>, String> {
    let active = Arc::new(AtomicUsize::new(0));
    listeners
        .into_iter()
        .map(|listener| {
            let service = Arc::clone(service);
            let active = Arc::clone(&active);
            std::thread::Builder::new()
                .name("kiln-fleet-api".into())
                .spawn(move || serve(&listener, &service, &active))
                .map_err(|error| format!("cannot start an API thread: {error}"))
        })
        .collect()
}

fn serve(listener: &TcpListener, service: &Arc<Service>, active: &Arc<AtomicUsize>) {
    if let Ok(address) = listener.local_addr() {
        log::info("api.listening", json!({ "address": address.to_string() }));
    }
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            let _ = write_response(
                &mut stream,
                &Response::error(503, "fleet_busy", "too many concurrent connections"),
            );
            continue;
        }
        let service = Arc::clone(service);
        let counter = Arc::clone(active);
        let spawned = std::thread::Builder::new()
            .name("kiln-fleet-conn".into())
            .spawn(move || {
                connection(stream, &service);
                counter.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn connection(mut stream: TcpStream, service: &Service) {
    let timeout = Some(Duration::from_millis(CONNECTION_TIMEOUT_MS));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    let Ok(peer) = stream.peer_addr() else {
        return;
    };
    let response = match read_request(&mut stream) {
        Ok(request) => service.handle(peer.ip(), &request),
        Err(error) => error.response(),
    };
    let _ = write_response(&mut stream, &response);
}
