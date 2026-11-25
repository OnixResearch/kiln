//! Read-only Herdr Core generation 1 observation over its plain JSON-RPC socket.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use socket2::{Domain, SockAddr, Socket, Type};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
// Core gives its remote terminal.list calls up to three seconds each, in parallel.
const RPC_TIMEOUT: Duration = Duration::from_secs(5);
// Core sends a heartbeat every 15 seconds by default.
const STREAM_TIMEOUT: Duration = Duration::from_secs(30);
const COALESCE: Duration = Duration::from_millis(250);
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

// All outbound bytes are fixed. Configuration and Core's data never become RPC inputs.
const HELLO: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"hello\",\"params\":{\"generation\":1}}\n";
const LIST: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"terminal.list\",\"params\":{\"all\":false}}\n";
const SUBSCRIBE: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"events.subscribe\",\"params\":{}}\n";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CoreHost {
    pub(crate) connected: bool,
    pub(crate) terminals: Option<usize>,
    pub(crate) statuses: Option<BTreeMap<&'static str, usize>>,
}

impl CoreHost {
    fn new(connected: bool) -> Self {
        Self {
            connected,
            terminals: connected.then_some(0),
            statuses: connected.then(|| {
                BTreeMap::from([("idle", 0), ("working", 0), ("done", 0), ("blocked", 0), ("error", 0), ("unknown", 0)])
            }),
        }
    }
}

/// Own one subscription lifetime. The caller invalidates on return and reconnects.
/// A missed notification invalidates immediately, until another list succeeds.
pub(crate) fn observe(
    socket: &Path,
    version: &str,
    host_aliases: &BTreeMap<String, String>,
    mut update: impl FnMut(Option<BTreeMap<String, CoreHost>>),
) -> Result<(), &'static str> {
    let mut stream = Connection::hello(socket, version)?;
    let _: Subscription = stream.call(SUBSCRIBE, 3)?;
    // Subscribe before taking the baseline. Anything changing during the list
    // then has a notification buffered on the independent subscription.
    update(Some(snapshot(socket, version, host_aliases)?));

    let mut liveness = Instant::now() + STREAM_TIMEOUT;
    let mut refresh_at = None;
    let mut invalidated = false;
    loop {
        let now = Instant::now();
        if now >= liveness {
            return Err("Core subscription timeout");
        }
        if refresh_at.is_some_and(|when| now >= when) {
            update(Some(snapshot(socket, version, host_aliases)?));
            refresh_at = None;
            invalidated = false;
            continue;
        }
        if !stream.frame(refresh_at.map_or(liveness, |when| liveness.min(when)))? {
            continue;
        }
        let notice: Notification = stream.decode()?;
        if notice.id.is_some() {
            return Err("invalid Core notification");
        }
        if matches!(notice.method, Notice::Missed) && !invalidated {
            update(None);
            invalidated = true;
        }
        let now = Instant::now();
        liveness = now + STREAM_TIMEOUT;
        // Bound work even under a continuous event burst. Heartbeats also
        // refresh because a remote can disconnect without a terminal event.
        refresh_at.get_or_insert(now + COALESCE);
    }
}

fn snapshot(
    socket: &Path,
    version: &str,
    aliases: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, CoreHost>, &'static str> {
    let mut connection = Connection::hello(socket, version)?;
    let list: TerminalList<'_> = connection.call(LIST, 2)?;
    Ok(project(list, aliases))
}

struct Connection {
    reader: BufReader<UnixStream>,
    bytes: Vec<u8>,
    complete: bool,
}

impl Connection {
    fn hello(path: &Path, version: &str) -> Result<Self, &'static str> {
        let addr = SockAddr::unix(path).map_err(|_| "invalid Core socket path")?;
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None).map_err(|_| "Core socket creation failed")?;
        socket
            .connect_timeout(&addr, CONNECT_TIMEOUT)
            .map_err(|_| "Core socket connect failed")?;
        let stream = UnixStream::from(OwnedFd::from(socket));
        stream.set_write_timeout(Some(RPC_TIMEOUT)).map_err(|_| "Core socket timeout setup failed")?;
        let mut connection = Self { reader: BufReader::new(stream), bytes: Vec::new(), complete: false };
        let hello: Hello<'_> = connection.call(HELLO, 1)?;
        if hello.generation != 1 || hello.version != version || hello.framed {
            return Err("incompatible Core handshake");
        }
        Ok(connection)
    }

    fn call<'a, T: Deserialize<'a>>(&'a mut self, request: &[u8], id: u64) -> Result<T, &'static str> {
        let deadline = Instant::now() + RPC_TIMEOUT;
        self.reader.get_mut().write_all(request).map_err(|_| "Core request write failed")?;
        if !self.frame(deadline)? {
            return Err("Core response timeout");
        }
        let reply: Reply<T> = self.decode()?;
        if reply.id != id || reply.error.is_some() {
            return Err("invalid Core reply");
        }
        reply.result.ok_or("missing Core result")
    }

    /// Read one newline-delimited frame, including its delimiter in the size
    /// limit. Preserve a partial frame across a coalescing wake-up, but never
    /// let it extend the caller's absolute timeout.
    fn frame(&mut self, deadline: Instant) -> Result<bool, &'static str> {
        if self.complete {
            self.bytes.clear();
            self.complete = false;
        }
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(false);
            }
            if self.reader.buffer().is_empty() {
                self.reader.get_ref().set_read_timeout(Some(remaining)).map_err(|_| "Core socket timeout setup failed")?;
            }
            let data = match self.reader.fill_buf() {
                Ok([]) => return Err("Core socket closed"),
                Ok(data) => data,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Ok(false),
                Err(_) => return Err("Core socket read failed"),
            };
            let piece = data.split_inclusive(|&byte| byte == b'\n').next().ok_or("empty Core read")?;
            let count = piece.len();
            if self.bytes.len() + count > MAX_FRAME_BYTES {
                return Err("Core frame exceeds size limit");
            }
            let complete = piece.ends_with(b"\n");
            self.bytes.extend_from_slice(piece);
            self.reader.consume(count);
            if complete {
                self.complete = true;
                return Ok(true);
            }
        }
    }

    fn decode<'a, T: Deserialize<'a>>(&'a self) -> Result<T, &'static str> {
        serde_json::from_slice(&self.bytes).map_err(|_| "invalid Core JSON frame")
    }
}

#[derive(Deserialize)]
enum Protocol {
    #[serde(rename = "2.0")]
    JsonRpc2,
}

#[derive(Deserialize)]
struct Reply<T> {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: Protocol,
    id: u64,
    result: Option<T>,
    error: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct Hello<'a> {
    generation: u32,
    #[serde(borrow)]
    version: Cow<'a, str>,
    framed: bool,
}

#[derive(Deserialize)]
struct Subscription {
    #[serde(rename = "seq")]
    _seq: u64,
    #[serde(rename = "terminals")]
    _terminals: IgnoredAny,
}

#[derive(Deserialize)]
struct Notification {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: Protocol,
    id: Option<IgnoredAny>,
    method: Notice,
    #[serde(rename = "params")]
    _params: NoticeParams,
}

#[derive(Deserialize)]
struct NoticeParams {
    #[serde(rename = "seq")]
    _seq: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Notice {
    Event,
    Missed,
    Heartbeat,
}

#[derive(Deserialize)]
struct TerminalList<'a> {
    #[serde(borrow)]
    terminals: Vec<Terminal<'a>>,
    // Core 0.1.19 omits this field when no machines are configured.
    #[serde(borrow, default)]
    machines: Vec<Machine<'a>>,
}

#[derive(Deserialize)]
struct Machine<'a> {
    #[serde(borrow)]
    name: Cow<'a, str>,
    state: MachineState,
}

#[derive(Deserialize)]
struct Terminal<'a> {
    #[serde(borrow)]
    address: Cow<'a, str>,
    state: Lifecycle,
    status: Option<ProgramStatus>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum MachineState {
    Connected,
    #[serde(other)]
    Disconnected,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Lifecycle {
    Live,
    #[serde(other)]
    NotLive,
}

#[derive(Deserialize)]
struct ProgramStatus {
    state: ProgramState,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProgramState {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
    #[serde(other)]
    Unknown,
}

impl ProgramState {
    const fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Error => "error",
            Self::Unknown => "unknown",
        }
    }
}

fn project(list: TerminalList<'_>, aliases: &BTreeMap<String, String>) -> BTreeMap<String, CoreHost> {
    let machines: BTreeMap<_, _> = list.machines.iter().map(|m| (m.name.as_ref(), matches!(m.state, MachineState::Connected))).collect();
    let mut hosts = BTreeMap::new();
    for alias in aliases.values().map(String::as_str) {
        let connected = if alias.is_empty() { Some(true) } else { machines.get(alias).copied() };
        if let Some(connected) = connected {
            hosts.entry(alias).or_insert_with(|| CoreHost::new(connected));
        }
    }

    for term in &list.terminals {
        if !matches!(term.state, Lifecycle::Live) {
            continue;
        }
        let alias = match term.address.split_once('/') {
            Some((alias, id)) if !alias.is_empty() && !id.is_empty() && !id.contains('/') => alias,
            Some(_) => continue,
            None if !term.address.is_empty() => "",
            None => continue,
        };
        let Some(CoreHost { terminals: Some(count), statuses: Some(statuses), .. }) = hosts.get_mut(alias) else {
            continue;
        };
        *count += 1;
        *statuses.entry(term.status.as_ref().map_or("unknown", |status| status.state.label())).or_default() += 1;
    }

    aliases
        .iter()
        .filter_map(|(label, alias)| {
            let host = hosts.get(alias.as_str())?;
            // Core can omit a timed-out remote list while still describing its
            // line as connected. Only a local zero is a confirmed empty list.
            let host = if !alias.is_empty() && host.terminals == Some(0) { CoreHost::new(false) } else { host.clone() };
            Some((label.clone(), host))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::{Value, json};
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::thread;

    const VERSION: &str = "0.1.19";

    #[test]
    fn projects_only_enrolled_hosts_and_closed_status_totals() {
        let list = serde_json::from_str(r#"{
            "terminals": [
                {"address":"raw-address","state":"live","name":"raw-name","title":"raw-title","cwd":"/secret/cwd","argv":["secret-argument"],"labels":{"secret-label":"secret-value"},"status":{"state":"working","msg":"secret-message"}},
                {"address":"t2","state":"live","status":{"state":"idle"}},
                {"address":"t3","state":"live","status":{"state":"done"}},
                {"address":"t4","state":"live","status":{"state":"blocked"}},
                {"address":"t5","state":"live","status":{"state":"error"}},
                {"address":"t6","state":"live","status":{"state":"sensitive custom state"}},
                {"address":"t7","state":"live","status":null},
                {"address":"t8","state":"live"},
                {"address":"t9","state":"finished","status":{"state":"done"}},
                {"address":"up/t1","state":"live","status":{"state":"working"}},
                {"address":"down/t1","state":"live","status":{"state":"working"}},
                {"address":"unmapped/t1","state":"live","status":{"state":"working"}}
            ],
            "machines":[
                {"name":"up","state":"connected","host":"secret-host","target":"secret-target"},
                {"name":"down","state":"needs_update"},
                {"name":"empty","state":"connected"},
                {"name":"unmapped","state":"connected"}
            ]
        }"#).unwrap();
        let hosts = project(list, &aliases(&[("here", ""), ("there", "up"), ("lost", "down"), ("unconfirmed", "empty"), ("absent", "missing")]));
        assert_eq!(serde_json::to_value(hosts).unwrap(), json!({
            "here": {"connected":true,"terminals":8,"statuses":{"idle":1,"working":1,"done":1,"blocked":1,"error":1,"unknown":3}},
            "there": {"connected":true,"terminals":1,"statuses":{"idle":0,"working":1,"done":0,"blocked":0,"error":0,"unknown":0}},
            "lost": {"connected":false,"terminals":null,"statuses":null},
            "unconfirmed": {"connected":false,"terminals":null,"statuses":null}
        }));
    }

    #[test]
    fn list_without_machines_reports_local_zero_only() {
        let list = serde_json::from_str(r#"{"hidden_finished":0,"terminals":[]}"#).unwrap();
        let hosts = project(list, &aliases(&[("local", ""), ("remote", "peer")]));
        assert_eq!(hosts, BTreeMap::from([("local".to_owned(), CoreHost::new(true))]));
    }

    #[test]
    fn refreshes_after_missed_and_heartbeat_on_separate_connections() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("core");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            hello(&mut stream, 1, VERSION, false);
            assert_eq!(request(&mut stream), json!({"jsonrpc":"2.0","id":3,"method":"events.subscribe","params":{}}));
            reply(&mut stream, 3, json!({"seq":10,"channel":null,"terminals":[{"secret":"subscription is not the list"}]}));
            list(&listener, "connected", json!([{"address":"peer/t1","state":"live","status":{"state":"working"}}]));
            // Only the following list can supply facts, including after losing continuity.
            let burst = concat!(
                "{\"jsonrpc\":\"2.0\",\"method\":\"event\",\"params\":{\"seq\":11,\"data\":{\"status\":\"working\"}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"missed\",\"params\":{\"seq\":12,\"terminals\":[]}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"event\",\"params\":{\"seq\":13,\"data\":{\"status\":\"error\"}}}\n"
            );
            stream.get_mut().write_all(burst.as_bytes()).unwrap();
            list(&listener, "connected", json!([{"address":"peer/t2","state":"live","status":{"state":"done"}}]));
            writeln!(stream.get_mut(), "{}", json!({"jsonrpc":"2.0","method":"heartbeat","params":{"seq":13}})).unwrap();
            list(&listener, "unreachable", json!([]));
        });

        let mut seen = Vec::new();
        let labels = aliases(&[("local", ""), ("remote", "peer")]);
        assert_eq!(observe(&socket, VERSION, &labels, |value| seen.push(value)), Err("Core socket closed"));
        server.join().unwrap();
        let [first, missed, refreshed, heartbeat]: [_; 4] = seen.try_into().unwrap();
        let first = first.unwrap();
        assert_eq!(first.get("local"), Some(&CoreHost::new(true)));
        assert_eq!(first.get("remote").unwrap().statuses.as_ref().unwrap().get("working"), Some(&1));
        assert_eq!(missed, None);
        assert_eq!(refreshed.unwrap().get("remote").unwrap().statuses.as_ref().unwrap().get("done"), Some(&1));
        assert_eq!(heartbeat.unwrap().get("remote"), Some(&CoreHost::new(false)));
    }

    #[test]
    fn rejects_incompatible_hello_before_sending_any_other_rpc() {
        for (generation, version, framed) in [(2, VERSION, false), (1, "0.1.18", false), (1, VERSION, true)] {
            let directory = tempfile::tempdir().unwrap();
            let socket = directory.path().join("core");
            let listener = UnixListener::bind(&socket).unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = thread::spawn(move || {
                let mut stream = accept(&listener);
                hello(&mut stream, generation, version, framed);
                let mut line = String::new();
                assert_eq!(stream.read_line(&mut line).unwrap(), 0);
            });
            let mut published = false;
            assert_eq!(observe(&socket, VERSION, &BTreeMap::new(), |_| published = true), Err("incompatible Core handshake"));
            assert!(!published);
            server.join().unwrap();
        }
    }

    #[test]
    fn enforces_frame_budget_including_delimiter() {
        let (client, mut server) = UnixStream::pair().unwrap();
        let sender = thread::spawn(move || {
            std::io::copy(&mut std::io::repeat(b' ').take(MAX_FRAME_BYTES as u64), &mut server).unwrap();
            server.write_all(b"\n").unwrap();
        });
        let mut connection = Connection { reader: BufReader::new(client), bytes: Vec::new(), complete: false };
        assert_eq!(connection.frame(Instant::now() + RPC_TIMEOUT), Err("Core frame exceeds size limit"));
        sender.join().unwrap();
    }

    #[test]
    fn partial_frame_survives_a_coalescing_timeout() {
        let (client, mut server) = UnixStream::pair().unwrap();
        let mut connection = Connection { reader: BufReader::new(client), bytes: Vec::new(), complete: false };
        server.write_all(b"{\"first\":").unwrap();
        assert_eq!(connection.frame(Instant::now() + Duration::from_millis(30)), Ok(false));
        server.write_all(b"true}\n{\"second\":true}\n").unwrap();
        assert_eq!(connection.frame(Instant::now() + RPC_TIMEOUT), Ok(true));
        assert_eq!(connection.decode::<Value>().unwrap(), json!({"first":true}));
        assert_eq!(connection.frame(Instant::now() + RPC_TIMEOUT), Ok(true));
        assert_eq!(connection.decode::<Value>().unwrap(), json!({"second":true}));
    }

    fn aliases(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|&(label, alias)| (label.into(), alias.into())).collect()
    }

    fn accept(listener: &UnixListener) -> BufReader<UnixStream> {
        let deadline = Instant::now() + RPC_TIMEOUT;
        let stream = loop {
            match listener.accept() {
                Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                result => break result.unwrap().0,
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(RPC_TIMEOUT)).unwrap();
        BufReader::new(stream)
    }

    fn request(stream: &mut BufReader<UnixStream>) -> Value {
        let mut line = String::new();
        stream.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn reply(stream: &mut BufReader<UnixStream>, id: u64, result: Value) {
        writeln!(stream.get_mut(), "{}", json!({"jsonrpc":"2.0","id":id,"result":result})).unwrap();
    }

    fn hello(stream: &mut BufReader<UnixStream>, generation: u32, version: &str, framed: bool) {
        assert_eq!(request(stream), json!({"jsonrpc":"2.0","id":1,"method":"hello","params":{"generation":1}}));
        reply(stream, 1, json!({"generation":generation,"version":version,"framed":framed,"core":{"name":"test","id":"fixture"},"methods":[],"features":[]}));
    }

    fn list(listener: &UnixListener, state: &str, terminals: Value) {
        let mut stream = accept(listener);
        hello(&mut stream, 1, VERSION, false);
        assert_eq!(request(&mut stream), json!({"jsonrpc":"2.0","id":2,"method":"terminal.list","params":{"all":false}}));
        reply(&mut stream, 2, json!({"terminals":terminals,"machines":[{"name":"peer","state":state}],"hidden_finished":0}));
    }
}
