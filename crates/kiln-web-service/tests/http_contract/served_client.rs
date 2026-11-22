//! The networked half of the lockstep contract fixture. No production client is implied.
use super::{CLIENT_BODY_LIMIT, load_openapi, schema_for, validate};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

const CA: &[u8] = include_bytes!("../../../../fixtures/http-observation/tls/ca-cert.pem");
const CERT: &[u8] = include_bytes!("../../../../fixtures/http-observation/tls/cert.pem");
const KEY: &[u8] = include_bytes!("../../../../fixtures/http-observation/tls/key.pem");
#[derive(Clone, Copy)]
struct Deadlines {
    connect: Duration,
    total: Duration,
    body_idle: Duration,
}
const SERVED_DEADLINES: Deadlines = Deadlines {
    connect: Duration::from_secs(3),
    total: Duration::from_secs(10),
    body_idle: Duration::from_secs(3),
};
const STUB_DEADLINES: Deadlines = Deadlines {
    connect: Duration::from_millis(400),
    total: Duration::from_millis(900),
    body_idle: Duration::from_millis(250),
};
const READY_DEADLINE: Duration = Duration::from_secs(8);
const STUB_ACCEPT_DEADLINE: Duration = Duration::from_secs(2);
const STUB_RETRY_OBSERVATION: Duration = Duration::from_millis(80);
const DRIP_INTERVAL: Duration = Duration::from_millis(100);
const HEADER_LIMIT: usize = 8 * 1024;
const READ_CHUNK: usize = 4096;
const PAGE_SIZE: usize = 1;
const RUN_COUNT: usize = 3;
const TOKEN_LENGTH: usize = 64;

struct Service(Child);
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn present<T>(value: Option<T>) -> T {
    value.unwrap_or_else(|| unreachable!("fixture value missing"))
}

#[derive(Debug)]
struct Observed {
    class: &'static str,
    status: Option<u16>,
    body: Vec<u8>,
}
impl Observed {
    fn json(&self) -> Value {
        checked(serde_json::from_slice(&self.body))
    }
    fn assert_error(&self, class: &str, status: u16, code: &str) {
        assert_eq!(self.class, class, "{self:?}");
        assert_eq!(self.status, Some(status));
        assert_eq!(self.json().get("code").and_then(Value::as_str), Some(code));
    }
}

// r[impl kiln.web.http_contract.client_bounds]
/// One TCP/TLS attempt, one HTTP request, one bounded incremental read for every status.
/// Reading to connection close (requested explicitly) also detects Content-Length that
/// understates the actual bytes; blindly trusting the header cannot do that.
fn request(
    deadlines: Deadlines,
    addr: SocketAddr,
    method: &str,
    target: &str,
    token: &str,
    payload: Option<&Value>,
    contract: (&Value, &Value),
) -> Observed {
    let (schema, components) = contract;
    let transport = || Observed {
        class: "transport",
        status: None,
        body: Vec::new(),
    };
    let started = Instant::now();
    let Ok(socket) = TcpStream::connect_timeout(&addr, deadlines.connect) else {
        return transport();
    };
    // Use the same committed fixture CA as the real service; never disable verification.
    let mut roots = rustls::RootCertStore::empty();
    let mut pem = std::io::Cursor::new(CA);
    let Ok(certs) = rustls_pemfile::certs(&mut pem).collect::<Result<Vec<_>, _>>() else {
        return transport();
    };
    if roots.add_parsable_certificates(certs).0 == 0 {
        return transport();
    }
    // Workspace feature unification enables more than one rustls provider;
    // select ring as the service and other TLS tests do.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let Ok(name) = rustls::pki_types::ServerName::try_from("localhost") else {
        return transport();
    };
    let Ok(connection) = rustls::ClientConnection::new(Arc::new(tls), name) else {
        return transport();
    };
    let mut stream = rustls::StreamOwned::new(connection, socket);
    // The connect budget covers both TCP and certificate-verified TLS setup.
    let Some(connect_left) = deadlines.connect.checked_sub(started.elapsed()) else {
        return transport();
    };
    if stream.sock.set_read_timeout(Some(connect_left)).is_err()
        || stream.sock.set_write_timeout(Some(connect_left)).is_err()
        || stream.conn.complete_io(&mut stream.sock).is_err()
        || started.elapsed() > deadlines.connect
    {
        return transport();
    }
    let body = payload
        .map(|value| checked(serde_json::to_vec(value)))
        .unwrap_or_default();
    let head = format!(
        "{method} {target} HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {token}\r\nOrigin: https://localhost:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        addr.port(),
        addr.port(),
        body.len()
    );
    if stream
        .sock
        .set_write_timeout(Some(deadlines.body_idle))
        .is_err()
        || stream.write_all(head.as_bytes()).is_err()
        || stream.write_all(&body).is_err()
        || stream.flush().is_err()
    {
        return transport();
    }
    let mut bytes = Vec::new();
    let mut header_end: Option<usize> = None;
    let mut chunk = [0; READ_CHUNK];
    loop {
        let Some(remaining) = deadlines.total.checked_sub(started.elapsed()) else {
            return transport();
        };
        if stream
            .sock
            .set_read_timeout(Some(remaining.min(deadlines.body_idle)))
            .is_err()
        {
            return transport();
        }
        let allowance = header_end.map_or(READ_CHUNK, |end| {
            (CLIENT_BODY_LIMIT - (bytes.len() - end) + 1).min(READ_CHUNK)
        });
        let count = match stream.read(present(chunk.get_mut(..allowance))) {
            Ok(count) => count,
            // TLS peers may close without close_notify. Framing and declared length
            // are checked below; a missing byte is still a transport failure.
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => 0,
            Err(_) => return transport(),
        };
        if count == 0 {
            if started.elapsed() > deadlines.total {
                return transport();
            }
            break;
        }
        bytes.extend_from_slice(present(chunk.get(..count)));
        if header_end.is_none() {
            header_end = bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| index + 4);
        }
        if header_end.is_none() && bytes.len() > HEADER_LIMIT {
            return transport();
        }
        if let Some(end) = header_end
            && (end > HEADER_LIMIT || bytes.len() - end > CLIENT_BODY_LIMIT)
        {
            return transport();
        }
    }
    let Some(end) = header_end else {
        return transport();
    };
    let Ok(headers) = std::str::from_utf8(present(bytes.get(..end - 4))) else {
        return transport();
    };
    let Some(status) = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|text| text.parse::<u16>().ok())
    else {
        return transport();
    };
    let mut content_length = None;
    let mut chunked = false;
    for line in headers.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            return transport();
        };
        if name.eq_ignore_ascii_case("content-length")
            && content_length
                .replace(value.trim().parse::<usize>().unwrap_or(usize::MAX))
                .is_some()
        {
            return transport();
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            && value.trim().eq_ignore_ascii_case("chunked")
        {
            chunked = true
        }
    }
    let raw = present(bytes.get(end..));
    let body = if chunked {
        if content_length.is_some() {
            return transport();
        }
        let Some(body) = decode_chunked(raw) else {
            return transport();
        };
        body
    } else {
        if content_length != Some(raw.len()) {
            return transport();
        }
        bytes.drain(..end);
        bytes
    };
    let class = match status {
        401 => "authentication",
        403 => "authorization",
        404 => "not_found",
        200..=299 => match serde_json::from_slice::<Value>(&body) {
            Ok(value) if validate(&value, schema, components).is_ok() => "ok",
            _ => "schema_mismatch",
        },
        _ => "unknown",
    };
    Observed {
        class,
        status: Some(status),
        body,
    }
}

fn decode_chunked(raw: &[u8]) -> Option<Vec<u8>> {
    let mut remaining = raw;
    let mut body = Vec::new();
    loop {
        let end = remaining.windows(2).position(|part| part == b"\r\n")?;
        let size =
            usize::from_str_radix(std::str::from_utf8(remaining.get(..end)?).ok()?, 16).ok()?;
        remaining = remaining.get(end + 2..)?;
        if size == 0 {
            return (remaining == b"\r\n").then_some(body);
        }
        if size > CLIENT_BODY_LIMIT.saturating_sub(body.len()) {
            return None;
        }
        body.extend_from_slice(remaining.get(..size)?);
        if remaining.get(size..size + 2)? != b"\r\n" {
            return None;
        }
        remaining = remaining.get(size + 2..)?;
    }
}

fn served() -> (tempfile::TempDir, super::ServiceProfile, Service) {
    let (root, mut profile, _) = super::fixture();
    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    profile.listen_address = checked(listener.local_addr());
    profile.origin = format!("https://localhost:{}", profile.listen_address.port());
    drop(listener);
    checked(fs::write(&profile.tls.certificate_path, CERT));
    checked(fs::write(&profile.tls.private_key_path, KEY));
    let viewer_path = root.path().join("viewer.token");
    for (path, token) in [
        (&profile.operator_token_file, b'a'),
        (&profile.worker_token_file, b'b'),
        (&viewer_path, b'c'),
    ] {
        checked(fs::write(path, vec![token; TOKEN_LENGTH]));
        checked(fs::set_permissions(path, fs::Permissions::from_mode(0o400)));
    }
    checked(fs::set_permissions(
        &profile.tls.private_key_path,
        fs::Permissions::from_mode(0o400),
    ));
    let profile_path = root.path().join("service.json");
    checked(fs::write(
        &profile_path,
        checked(serde_json::to_vec(&profile)),
    ));
    let access_path = root.path().join("access.json");
    checked(fs::write(
        &access_path,
        checked(serde_json::to_vec(
            &json!({"schema":"kiln.web-access.v1","viewers":[{"id":"reader","label":"Fixture reader","token_file":viewer_path}]}),
        )),
    ));
    let notify_path = root.path().join("notify.sock");
    let receiver = checked(UnixDatagram::bind(&notify_path));
    checked(receiver.set_read_timeout(Some(READY_DEADLINE)));
    let mut child = Service(checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["serve", "--profile"])
            .arg(profile_path)
            .arg("--access")
            .arg(access_path)
            .env("NOTIFY_SOCKET", notify_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn(),
    ));
    let mut notification = [0; 32];
    match receiver.recv(&mut notification) {
        Ok(n) => assert_eq!(notification.get(..n), Some(b"READY=1".as_slice())),
        Err(error) => {
            let exit = checked(child.0.try_wait());
            unreachable!("serve readiness failed: {error}; exit={exit:?}");
        }
    }
    (root, profile, child)
}

// r[verify kiln.web.http_contract.served_matrix]
// r[verify kiln.web.http_contract.served_matrix.scenario.errors]
// r[verify kiln.web.http_contract.served_matrix.scenario.pagination]
#[test]
fn served_https_matrix_and_complete_or_incomplete_pagination() {
    let (_root, profile, _service) = served();
    let api = load_openapi();
    let components = present(api.get("components"));
    let addr = profile.listen_address;
    let operator = "a".repeat(TOKEN_LENGTH);
    let worker = "b".repeat(TOKEN_LENGTH);
    let viewer = "c".repeat(TOKEN_LENGTH);
    let get = |path: &str, token: &str, schema: &str| {
        request(
            SERVED_DEADLINES,
            addr,
            "GET",
            path,
            token,
            None,
            (&schema_for(&api, schema), components),
        )
    };
    let post = |path: &str, value: Value| {
        request(
            SERVED_DEADLINES,
            addr,
            "POST",
            path,
            &operator,
            Some(&value),
            (&schema_for(&api, "RunView"), components),
        )
    };
    assert_eq!(get("/api/projects", &operator, "ProjectList").class, "ok");
    let enrolled = post(
        "/api/projects",
        json!({"project_id":"fixture","request_id":"enroll"}),
    );
    assert_eq!(enrolled.status, Some(200), "{enrolled:?}");
    let mut ids = Vec::new();
    for index in 0..RUN_COUNT {
        let created = post(
            "/api/runs",
            json!({"project_id":"fixture","request_id":format!("run-{index}")}),
        );
        assert_eq!(created.class, "ok", "{created:?}");
        ids.push(present(created.json().get("id").and_then(Value::as_str)).to_owned());
    }
    let id = present(ids.first());
    for (path, schema) in [
        (format!("/api/runs/{id}"), "RunView"),
        (
            format!("/api/runs/{id}/execution-binding"),
            "ExecutionBinding",
        ),
        (format!("/api/runs/{id}/artifacts"), "ArtifactList"),
    ] {
        let result = get(&path, &operator, schema);
        assert_eq!(result.class, "ok", "{path}: {result:?}");
    }
    get("/api/projects", &"x".repeat(TOKEN_LENGTH), "Error").assert_error(
        "authentication",
        401,
        "unauthorized",
    );
    get("/api/projects", &worker, "Error").assert_error("authorization", 403, "worker_scope");
    get(
        &format!("/api/runs/{id}/execution-binding"),
        &viewer,
        "Error",
    )
    .assert_error("authorization", 403, "execution_binding_scope");
    get("/api/runs/no-such-run", &operator, "Error").assert_error("not_found", 404, "run_missing");
    get("/api/runs?limit=0", &operator, "Error").assert_error("unknown", 400, "page_bound");
    let listing = list_pages(|path| get(path, &operator, "RunPage"));
    assert!(listing.complete, "{listing:?}");
    assert_eq!(listing.ids.len(), RUN_COUNT);
    assert!(ids.iter().all(|id| listing.ids.contains(id)));
    // A real cursor, modified without changing its query, is refused by the
    // real server. Neither that refusal nor a lost follow-up is completion.
    let first = get(
        &format!("/api/runs?limit={PAGE_SIZE}"),
        &operator,
        "RunPage",
    );
    assert_eq!(first.class, "ok");
    let cursor = present(first.json().get("next_cursor").and_then(Value::as_str)).to_owned();
    let refused = get(
        &format!("/api/runs?limit={PAGE_SIZE}&cursor={cursor}x"),
        &operator,
        "Error",
    );
    refused.assert_error("unknown", 400, "cursor_invalid");
    let lost_listener = checked(TcpListener::bind("127.0.0.1:0"));
    let lost_addr = checked(lost_listener.local_addr());
    drop(lost_listener);
    let mut calls = 0;
    let incomplete = list_pages(|path| {
        calls += 1;
        if calls == 1 {
            get(path, &operator, "RunPage")
        } else {
            request(
                SERVED_DEADLINES,
                lost_addr,
                "GET",
                path,
                &operator,
                None,
                (&schema_for(&api, "RunPage"), components),
            )
        }
    });
    assert!(!incomplete.complete);
    assert_eq!(incomplete.ids.len(), PAGE_SIZE);
    let mut calls = 0;
    let refused_listing = list_pages(|path| {
        calls += 1;
        if calls == 1 {
            get(path, &operator, "RunPage")
        } else {
            get(&format!("{path}x"), &operator, "RunPage")
        }
    });
    assert!(!refused_listing.complete);
    assert_eq!(refused_listing.ids.len(), PAGE_SIZE);
}

#[derive(Debug)]
struct Listing {
    complete: bool,
    ids: Vec<String>,
}
// r[impl kiln.web.http_contract.served_matrix]
fn list_pages(mut fetch: impl FnMut(&str) -> Observed) -> Listing {
    let mut listing = Listing {
        complete: false,
        ids: Vec::new(),
    };
    let mut cursor: Option<String> = None;
    loop {
        let target = match &cursor {
            Some(cursor) => format!("/api/runs?limit={PAGE_SIZE}&cursor={cursor}"),
            None => format!("/api/runs?limit={PAGE_SIZE}"),
        };
        let response = fetch(&target);
        if response.class != "ok" {
            return listing;
        }
        let page = response.json();
        let Some(rows) = page.get("runs").and_then(Value::as_array) else {
            return listing;
        };
        for run in rows {
            let Some(id) = run.get("id").and_then(Value::as_str) else {
                return listing;
            };
            listing.ids.push(id.to_owned());
        }
        cursor = page
            .get("next_cursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if cursor.is_none() {
            listing.complete = true;
            return listing;
        }
    }
}

enum Fault {
    Missing,
    WrongType,
    Extra,
    InvalidJson,
    LongLength,
    ShortLength,
    Oversized,
    Drip,
    Stall,
    ErrorOversized,
}
fn stub(fault: Fault, real: &[u8]) -> (SocketAddr, std::thread::JoinHandle<usize>) {
    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    let addr = checked(listener.local_addr());
    checked(listener.set_nonblocking(true));
    let real = real.to_vec();
    let thread = std::thread::spawn(move || {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let certs = checked(
            rustls_pemfile::certs(&mut std::io::Cursor::new(CERT)).collect::<Result<Vec<_>, _>>(),
        );
        let key = present(checked(rustls_pemfile::private_key(
            &mut std::io::Cursor::new(KEY),
        )));
        let config = Arc::new(checked(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key),
        ));
        let start = Instant::now();
        let mut count = 0;
        while start.elapsed() < STUB_ACCEPT_DEADLINE {
            match listener.accept() {
                Ok((socket, _)) => {
                    count += 1;
                    checked(socket.set_read_timeout(Some(STUB_DEADLINES.body_idle)));
                    let mut stream = rustls::StreamOwned::new(
                        checked(rustls::ServerConnection::new(config.clone())),
                        socket,
                    );
                    let mut bytes = Vec::new();
                    let mut block = [0; READ_CHUNK];
                    while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                        match stream.read(&mut block) {
                            Ok(n) if n > 0 => bytes.extend_from_slice(present(block.get(..n))),
                            _ => break,
                        }
                        if bytes.len() > HEADER_LIMIT {
                            break;
                        }
                    }
                    let mut body = real.clone();
                    match fault {
                        Fault::Missing | Fault::WrongType | Fault::Extra => {
                            let mut value: Value = checked(serde_json::from_slice(&body));
                            let object = present(value.as_object_mut());
                            match fault {
                                Fault::Missing => {
                                    object.remove("enrolled");
                                }
                                Fault::WrongType => {
                                    object.insert("available".into(), json!("wrong type"));
                                }
                                Fault::Extra => {
                                    object.insert("unexpected".into(), json!(true));
                                }
                                _ => unreachable!(),
                            }
                            body = checked(serde_json::to_vec(&value));
                        }
                        Fault::InvalidJson => {
                            body.pop();
                        }
                        Fault::Oversized | Fault::ErrorOversized => {
                            body = vec![b' '; CLIENT_BODY_LIMIT + 1]
                        }
                        _ => {}
                    }
                    let status = if matches!(fault, Fault::ErrorOversized) {
                        "401 Unauthorized"
                    } else {
                        "200 OK"
                    };
                    if matches!(fault, Fault::Oversized | Fault::ErrorOversized) {
                        let head = format!(
                            "HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&body);
                        let _ = stream.write_all(b"\r\n0\r\n\r\n");
                    } else {
                        let length = match fault {
                            Fault::LongLength => body.len() + 1,
                            Fault::ShortLength => body.len() - 1,
                            _ => body.len(),
                        };
                        let head = format!(
                            "HTTP/1.1 {status}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
                        );
                        let _ = stream.write_all(head.as_bytes());
                        match fault {
                            Fault::Drip => {
                                for byte in &body {
                                    std::thread::sleep(DRIP_INTERVAL);
                                    if stream.write_all(&[*byte]).is_err() {
                                        break;
                                    }
                                }
                            }
                            Fault::Stall => std::thread::sleep(STUB_DEADLINES.total),
                            _ => {
                                let _ = stream.write_all(&body);
                            }
                        }
                    }
                    let _ = stream.flush();
                    // Observe extra connections after the first without extending the deadline.
                    let end = Instant::now() + STUB_RETRY_OBSERVATION;
                    while Instant::now() < end {
                        if listener.accept().is_ok() {
                            count += 1
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    return count;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => unreachable!("stub accept: {error}"),
            }
        }
        count
    });
    (addr, thread)
}

// r[verify kiln.web.http_contract.client_bounds]
// r[verify kiln.web.http_contract.client_bounds.scenario.transport]
// r[verify kiln.web.http_contract.client_bounds.scenario.schema]
#[test]
fn loopback_tls_replays_captured_body_with_faults_without_retries() {
    let (_root, profile, _service) = served();
    let api = load_openapi();
    let components = present(api.get("components"));
    let projects = schema_for(&api, "ProjectList");
    let captured = request(
        SERVED_DEADLINES,
        profile.listen_address,
        "GET",
        "/api/projects",
        &"a".repeat(TOKEN_LENGTH),
        None,
        (&projects, components),
    );
    assert_eq!(captured.class, "ok", "{captured:?}");
    for (fault, expected) in [
        (Fault::Missing, "schema_mismatch"),
        (Fault::WrongType, "schema_mismatch"),
        (Fault::Extra, "schema_mismatch"),
        (Fault::InvalidJson, "schema_mismatch"),
        (Fault::LongLength, "transport"),
        (Fault::ShortLength, "transport"),
        (Fault::Oversized, "transport"),
        (Fault::Drip, "transport"),
        (Fault::Stall, "transport"),
        (Fault::ErrorOversized, "transport"),
    ] {
        let (addr, server) = stub(fault, &captured.body);
        let result = request(
            STUB_DEADLINES,
            addr,
            "GET",
            "/api/projects",
            &"a".repeat(TOKEN_LENGTH),
            None,
            (&projects, components),
        );
        assert_eq!(result.class, expected, "{result:?}");
        assert_eq!(checked(server.join()), 1, "one request, no retry");
    }
    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    let addr = checked(listener.local_addr());
    drop(listener);
    assert_eq!(
        request(
            STUB_DEADLINES,
            addr,
            "GET",
            "/api/projects",
            "invalid",
            None,
            (&projects, components)
        )
        .class,
        "transport"
    );
}
