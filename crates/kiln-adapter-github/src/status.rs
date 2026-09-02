use std::ffi::OsString;
use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope, run,
};

use kiln_core::types::ContentId;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::secret::ResolvedSecret;
use crate::{domain, infrastructure};

const API: &str = "https://api.github.com";
const API_VERSION: &str = "2022-11-28";
const PAGE_SIZE: usize = 100;

/// Reserved executable entry: exactly this argument followed by the calling PID.
/// Hosts of this library must dispatch this before their normal CLI parser.
pub const DNS_CHILD_ARGUMENT: &str = "--internal-github-status-dns";
const DNS_DESTINATION: &str = "api.github.com:443";
const MAX_DNS_ADDRESSES: usize = 64;
const MAX_DNS_BYTES: usize = 4096;

/// Runs only in the credential-free leaf subprocess, never in the HTTP caller.
/// Parent-death registration and a subsequent PID check close the spawn race.
pub fn run_dns_child(parent_pid: u32) -> i32 {
    #[cfg(target_os = "linux")]
    {
        if parent_pid == 0
            || nix::sys::prctl::set_pdeathsig(nix::sys::signal::Signal::SIGKILL).is_err()
            || u32::try_from(nix::unistd::getppid().as_raw()).ok() != Some(parent_pid)
        {
            return 1;
        }
        let addresses = match DNS_DESTINATION.to_socket_addrs() {
            Ok(addresses) => addresses,
            Err(_) => return 1,
        };
        let mut output = String::new();
        let mut count = 0;
        for address in addresses {
            count += 1;
            if count > MAX_DNS_ADDRESSES || address.port() != 443 {
                return 1;
            }
            use std::fmt::Write as _;
            if writeln!(&mut output, "{address}").is_err() || output.len() > MAX_DNS_BYTES {
                return 1;
            }
        }
        if count == 0 {
            return 1;
        }
        use std::io::Write as _;
        if std::io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .is_err()
        {
            return 1;
        }
        0
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = parent_pid;
        1
    }
}

struct BoundedGithubResolver {
    program: PathBuf,
    args: Vec<OsString>,
    timeout_ms: u64,
}

impl BoundedGithubResolver {
    fn current_executable(timeout_ms: u64) -> PortResult<Self> {
        let program = std::env::current_exe().map_err(|_| {
            infrastructure(
                "github_status_dns_executable",
                "Status DNS helper executable cannot be identified",
            )
        })?;
        Ok(Self {
            program,
            args: vec![
                DNS_CHILD_ARGUMENT.into(),
                std::process::id().to_string().into(),
            ],
            timeout_ms,
        })
    }

    fn resolve(&self, netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
        use std::io::{Error, ErrorKind};
        if netloc != DNS_DESTINATION {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Status DNS destination was not admitted",
            ));
        }
        let output = run(RunRequest {
            command: CommandSpec {
                program: self.program.clone(),
                args: self.args.clone(),
                current_dir: PathBuf::from("/"),
                environment_mode: EnvironmentMode::Clear,
                environment: Vec::new(),
                input: Input::Null,
            },
            limits: ExecutionLimits {
                timeout_ms: self.timeout_ms,
                stdin_max_bytes: 1,
                stdout_max_bytes: MAX_DNS_BYTES,
                stderr_max_bytes: 1024,
                poll_interval_ms: 10,
                teardown_timeout_ms: 2000,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: OutcomePolicy::new(vec![0], true, true)
                .map_err(|_| Error::other("Status DNS policy was invalid"))?,
        })
        .map_err(|_| Error::other("Status DNS helper could not complete"))?;
        if output.disposition == Disposition::TimedOut {
            return Err(Error::new(
                ErrorKind::TimedOut,
                "Status DNS helper exceeded its deadline",
            ));
        }
        if output.disposition != Disposition::Succeeded {
            return Err(Error::other(
                "Status DNS helper failed or exceeded its output bound",
            ));
        }
        let text = std::str::from_utf8(&output.stdout.bytes)
            .map_err(|_| Error::other("Status DNS helper returned invalid addresses"))?;
        let mut addresses = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let address: SocketAddr = line
                .parse()
                .map_err(|_| Error::other("Status DNS helper returned invalid addresses"))?;
            if address.port() != 443 || index >= MAX_DNS_ADDRESSES {
                return Err(Error::other("Status DNS helper returned invalid addresses"));
            }
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        if addresses.is_empty() {
            return Err(Error::other("Status DNS helper returned no addresses"));
        }
        Ok(addresses)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusConfig {
    pub owner: String,
    pub repository: String,
    pub context: String,
    pub maximum_response_bytes: usize,
    pub maximum_pages: usize,
    pub timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusState {
    Pending,
    Success,
    Failure,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusPlan {
    pub owner: String,
    pub repository: String,
    pub context: String,
    pub revision: String,
    pub run_identity: ContentId,
    pub state: StatusState,
}

impl StatusPlan {
    pub fn description(&self) -> String {
        let state = match self.state {
            StatusState::Pending => "pending",
            StatusState::Success => "success",
            StatusState::Failure => "failure",
            StatusState::Error => "error",
        };
        format!("Kiln run {} state {state}", self.run_identity.as_str())
    }
    #[cfg(test)]
    fn commit_url(&self) -> String {
        format!(
            "{API}/repos/{}/{}/statuses/{}",
            self.owner, self.repository, self.revision
        )
    }
    fn admits_commit_url(&self, url: &str) -> bool {
        let Some(path) = url.strip_prefix("https://api.github.com/repos/") else {
            return false;
        };
        let mut components = path.split('/');
        matches!((components.next(), components.next(), components.next(), components.next(), components.next()),
            (Some(owner), Some(repository), Some("statuses"), Some(revision), None)
                if owner.eq_ignore_ascii_case(&self.owner)
                    && repository.eq_ignore_ascii_case(&self.repository)
                    && revision == self.revision)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub status_id: u64,
    pub request_identity: ContentId,
}

pub trait StatusPort {
    fn publish(&mut self, plan: &StatusPlan) -> PortResult<Publication>;
    fn reconcile(&mut self, plan: &StatusPlan) -> PortResult<Option<Publication>>;
}

// r[impl kiln.github.status]
pub fn admit_status_config(config: &StatusConfig) -> PortResult<()> {
    admit_binding(&config.owner, &config.repository, &config.context)?;
    if !(1..=4_194_304).contains(&config.maximum_response_bytes)
        || !(1..=100).contains(&config.maximum_pages)
        || !(1..=120_000).contains(&config.timeout_ms)
    {
        return Err(domain(
            "github_status_config",
            "status",
            "GitHub status configuration is invalid or exceeds its bounds",
        ));
    }
    Ok(())
}

fn admit_binding(owner: &str, repository: &str, context: &str) -> PortResult<()> {
    let owner_valid = !owner.is_empty()
        && owner.len() <= 39
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let repo_valid = !repository.is_empty()
        && repository.len() <= 100
        && !matches!(repository, "." | "..")
        && repository
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b));
    if !owner_valid
        || !repo_valid
        || context.is_empty()
        || context.len() > 100
        || context.trim() != context
        || context
            .bytes()
            .any(|b| !b.is_ascii() || b.is_ascii_control())
    {
        return Err(domain(
            "github_status_config",
            "status",
            "GitHub status binding is invalid",
        ));
    }
    Ok(())
}

fn admit_identities(revision: &str, run_identity: &ContentId) -> PortResult<()> {
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || run_identity.as_str().len() != 64
        || !run_identity
            .as_str()
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(domain(
            "github_status_identity",
            "status",
            "Status requires exact canonical commit and run identities",
        ));
    }
    Ok(())
}

pub fn plan_status(
    config: &StatusConfig,
    revision: &str,
    run_identity: ContentId,
    state: StatusState,
) -> PortResult<StatusPlan> {
    admit_status_config(config)?;
    admit_identities(revision, &run_identity)?;
    Ok(StatusPlan {
        owner: config.owner.clone(),
        repository: config.repository.clone(),
        context: config.context.clone(),
        revision: revision.into(),
        run_identity,
        state,
    })
}

fn admit_plan(plan: &StatusPlan) -> PortResult<()> {
    admit_binding(&plan.owner, &plan.repository, &plan.context)?;
    admit_identities(&plan.revision, &plan.run_identity)
}

/// Length-delimited fields and a versioned domain bind all semantics independently of JSON formatting.
pub fn request_identity(plan: &StatusPlan) -> PortResult<ContentId> {
    admit_plan(plan)?;
    let mut hash = blake3::Hasher::new();
    let state = match plan.state {
        StatusState::Pending => "pending",
        StatusState::Success => "success",
        StatusState::Failure => "failure",
        StatusState::Error => "error",
    };
    for field in [
        "kiln.github.status.v1",
        API,
        API_VERSION,
        &plan.owner,
        &plan.repository,
        &plan.revision,
        &plan.context,
        plan.run_identity.as_str(),
        state,
    ] {
        hash.update(&(field.len() as u64).to_be_bytes());
        hash.update(field.as_bytes());
    }
    ContentId::from_blake3_hex(hash.finalize().to_hex().to_string()).map_err(PortFailure::Domain)
}

/// Credentials never appear in Debug, serialization or ambient fallback.
pub struct GithubStatusClient {
    config: StatusConfig,
    token: ResolvedSecret,
    endpoint: String,
    agent: ureq::Agent,
}

impl std::fmt::Debug for GithubStatusClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubStatusClient")
            .field("config", &self.config)
            .field("endpoint", &self.endpoint)
            .field("token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl GithubStatusClient {
    pub fn new(config: StatusConfig, token: ResolvedSecret) -> PortResult<Self> {
        Self::build(config, token, API.into(), true)
    }

    #[cfg(any(test, feature = "protocol-fixtures"))]
    pub fn new_fixture(
        config: StatusConfig,
        token: ResolvedSecret,
        endpoint: &str,
    ) -> PortResult<Self> {
        // Parse only a bare loopback origin. No DNS, credentials, paths or TLS bypass in production.
        let authority = endpoint.strip_prefix("http://").ok_or_else(|| {
            domain(
                "github_fixture_endpoint",
                "status",
                "Fixture endpoint must be a loopback HTTP origin",
            )
        })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "github_fixture_endpoint",
                "status",
                "Fixture endpoint must declare a loopback port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]")
            || port.parse::<u16>().ok().is_none_or(|port| port == 0)
        {
            return Err(domain(
                "github_fixture_endpoint",
                "status",
                "Fixture endpoint must be a loopback HTTP origin",
            ));
        }
        Self::build(config, token, endpoint.into(), false)
    }

    fn build(
        config: StatusConfig,
        token: ResolvedSecret,
        endpoint: String,
        https: bool,
    ) -> PortResult<Self> {
        admit_status_config(&config)?;
        if !token.as_bytes().iter().all(|b| (b'!'..=b'~').contains(b)) {
            return Err(domain(
                "github_status_token",
                "secret",
                "GitHub token is not valid header material",
            ));
        }
        let timeout = Duration::from_millis(config.timeout_ms);
        let mut builder = ureq::AgentBuilder::new()
            .https_only(https)
            .redirects(0)
            .try_proxy_from_env(false)
            .max_idle_connections(0)
            .timeout(timeout)
            .timeout_connect(timeout);
        if https {
            // libc DNS can block independently of ureq's deadline. Isolate that
            // fixed-host lookup in a killed/reaped child, not a detached thread.
            let resolver = BoundedGithubResolver::current_executable(config.timeout_ms)?;
            builder = builder.resolver(move |netloc: &str| resolver.resolve(netloc));
        }
        let agent = builder.build();
        Ok(Self {
            config,
            token,
            endpoint,
            agent,
        })
    }

    fn admit(&self, plan: &StatusPlan) -> PortResult<()> {
        admit_plan(plan)?;
        if plan.owner != self.config.owner
            || plan.repository != self.config.repository
            || plan.context != self.config.context
        {
            return Err(domain(
                "github_status_plan",
                "status",
                "Status plan does not match the admitted client",
            ));
        }
        Ok(())
    }

    fn request(&self, method: &str, path: &str) -> PortResult<ureq::Request> {
        let authorization = Zeroizing::new(format!("Bearer {}", self.token.as_str()?));
        Ok(self
            .agent
            .request(method, &format!("{}{path}", self.endpoint))
            .set("Authorization", &authorization)
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", API_VERSION)
            .set("User-Agent", "kiln-github-adapter"))
    }

    fn response(
        &self,
        result: Result<ureq::Response, ureq::Error>,
        mutation: bool,
        expected: u16,
    ) -> PortResult<ureq::Response> {
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(error)) => {
                let timed_out = error_chain_timeout(&error);
                return Err(if mutation {
                    unknown(
                        if timed_out {
                            "github_status_timeout_unknown"
                        } else {
                            "github_status_transport_unknown"
                        },
                        "Status write outcome is unknown; read-only reconciliation is required",
                    )
                } else if timed_out {
                    PortFailure::TimedOut(failure(
                        "github_status_timeout",
                        "GitHub status lookup exceeded its deadline",
                    ))
                } else {
                    infrastructure(
                        "github_status_transport",
                        "GitHub status lookup transport failed",
                    )
                });
            }
        };
        let code = response.status();
        if code == expected {
            return Ok(response);
        }
        if code == 429
            || (code == 403
                && (response.header("X-RateLimit-Remaining") == Some("0")
                    || response.header("Retry-After").is_some()))
        {
            return Err(PortFailure::Retryable(failure(
                "github_status_rate_limit",
                "GitHub status request was rate limited",
            )));
        }
        if matches!(code, 401 | 403 | 404) {
            return Err(domain(
                "github_status_denied",
                "status",
                "GitHub denied the status request",
            ));
        }
        if mutation && (code >= 500 || code == 408 || (200..300).contains(&code)) {
            return Err(unknown(
                "github_status_response_unknown",
                "Status write may have been accepted; read-only reconciliation is required",
            ));
        }
        if code == 408 {
            return Err(PortFailure::TimedOut(failure(
                "github_status_timeout",
                "GitHub status lookup exceeded its deadline",
            )));
        }
        if code >= 500 {
            return Err(PortFailure::Retryable(failure(
                "github_status_server",
                "GitHub status provider failed",
            )));
        }
        Err(domain(
            "github_status_rejected",
            "status",
            "GitHub rejected the status request or attempted a redirect",
        ))
    }

    fn body(&self, response: ureq::Response, mutation: bool) -> PortResult<Vec<u8>> {
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(self.config.maximum_response_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                if mutation {
                    unknown(
                        "github_status_body_unknown",
                        "Status write response was lost; reconciliation is required",
                    )
                } else if error.kind() == std::io::ErrorKind::TimedOut {
                    PortFailure::TimedOut(failure(
                        "github_status_timeout",
                        "GitHub status lookup exceeded its deadline",
                    ))
                } else {
                    unknown(
                        "github_status_lookup_unknown",
                        "GitHub status lookup response was incomplete",
                    )
                }
            })?;
        if bytes.len() > self.config.maximum_response_bytes {
            return Err(unknown(
                "github_status_body_bound",
                "GitHub status response exceeded its admitted bound",
            ));
        }
        Ok(bytes)
    }
}

fn failure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.into(),
        message: message.into(),
    }
}
fn unknown(code: &str, message: &str) -> PortFailure {
    PortFailure::Unknown(failure(code, message))
}
fn error_chain_timeout(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut next = Some(error);
    while let Some(error) = next {
        if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            )
        }) {
            return true;
        }
        next = error.source();
    }
    false
}

#[derive(Serialize)]
struct StatusRequest<'a> {
    state: StatusState,
    context: &'a str,
    description: &'a str,
}

// GitHub adds provider-owned fields; only the exact semantic projection is decoded.
#[derive(Deserialize)]
struct RemoteStatus {
    id: u64,
    state: StatusState,
    context: String,
    description: Option<String>,
    url: String,
}

impl RemoteStatus {
    fn exact(&self, plan: &StatusPlan, description: &str) -> bool {
        self.id != 0
            && self.description.as_deref() == Some(description)
            && self.context == plan.context
            && self.state == plan.state
            && plan.admits_commit_url(&self.url)
    }
    fn publication(&self, plan: &StatusPlan) -> PortResult<Publication> {
        Ok(Publication {
            status_id: self.id,
            request_identity: request_identity(plan)?,
        })
    }
}

impl StatusPort for GithubStatusClient {
    // r[impl kiln.github.status]
    fn publish(&mut self, plan: &StatusPlan) -> PortResult<Publication> {
        self.admit(plan)?;
        let path = format!(
            "/repos/{}/{}/statuses/{}",
            plan.owner, plan.repository, plan.revision
        );
        let description = plan.description();
        let body = serde_json::to_vec(&StatusRequest {
            state: plan.state,
            context: &plan.context,
            description: &description,
        })
        .map_err(|_| {
            infrastructure(
                "github_status_encode",
                "Status request could not be encoded",
            )
        })?;
        // One POST only. ureq does not retry non-idempotent methods or nonempty bodies;
        // the connection pool is also disabled. Caller owns durable write reservation.
        let response = self.response(
            self.request("POST", &path)?
                .set("Content-Type", "application/json")
                .send_bytes(&body),
            true,
            201,
        )?;
        let remote: RemoteStatus =
            serde_json::from_slice(&self.body(response, true)?).map_err(|_| {
                unknown(
                    "github_status_malformed_unknown",
                    "Status write response was malformed; reconciliation is required",
                )
            })?;
        if !remote.exact(plan, &description) {
            return Err(unknown(
                "github_status_mismatch_unknown",
                "Status write response did not match its plan; reconciliation is required",
            ));
        }
        remote.publication(plan)
    }

    // r[impl kiln.github.status]
    fn reconcile(&mut self, plan: &StatusPlan) -> PortResult<Option<Publication>> {
        self.admit(plan)?;
        let mut found: Option<Publication> = None;
        let description = plan.description();
        for page in 1..=self.config.maximum_pages {
            let path = format!(
                "/repos/{}/{}/commits/{}/statuses?per_page={PAGE_SIZE}&page={page}",
                plan.owner, plan.repository, plan.revision
            );
            let response = self.response(self.request("GET", &path)?.call(), false, 200)?;
            let next_link = response
                .header("Link")
                .is_some_and(|value| value.contains("rel=\"next\""));
            let statuses: Vec<RemoteStatus> = serde_json::from_slice(&self.body(response, false)?)
                .map_err(|_| {
                    unknown(
                        "github_status_lookup_malformed",
                        "GitHub status lookup response was malformed",
                    )
                })?;
            if statuses.len() > PAGE_SIZE {
                return Err(unknown(
                    "github_status_page_bound",
                    "GitHub status lookup exceeded its page bound",
                ));
            }
            for remote in &statuses {
                if remote.description.as_deref() != Some(description.as_str()) {
                    continue;
                }
                if !remote.exact(plan, &description) {
                    return Err(unknown(
                        "github_status_conflict",
                        "GitHub has conflicting status evidence for this run",
                    ));
                }
                // Duplicate identical projections are harmless; retain stable native identity.
                if found
                    .as_ref()
                    .is_none_or(|publication| remote.id > publication.status_id)
                {
                    found = Some(remote.publication(plan)?);
                }
            }
            if statuses.len() < PAGE_SIZE && !next_link {
                return Ok(found);
            }
        }
        Err(unknown(
            "github_status_pages_exhausted",
            "Status lookup exhausted its page bound and cannot prove the complete projection",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    fn config() -> StatusConfig {
        StatusConfig {
            owner: "example".into(),
            repository: "project".into(),
            context: "kiln/ci".into(),
            maximum_response_bytes: 65_536,
            maximum_pages: 3,
            timeout_ms: 500,
        }
    }
    fn plan(state: StatusState) -> StatusPlan {
        plan_status(
            &config(),
            &"a".repeat(40),
            ContentId::from_blake3_hex("b".repeat(64))
                .unwrap_or_else(|_| unreachable!("fixture BLAKE3 identity is canonical")),
            state,
        )
        .unwrap_or_else(|_| unreachable!("fixture status plan is admitted"))
    }
    fn remote(plan: &StatusPlan, id: u64) -> Value {
        json!({"id": id, "state": plan.state, "context": plan.context, "description": plan.description(), "url": plan.commit_url()})
    }
    fn response(code: u16, body: &str, extra: &str) -> String {
        format!(
            "HTTP/1.1 {code} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{extra}\r\n{body}",
            body.len()
        )
    }
    enum Reply {
        Http(String),
        Disconnect,
        Delay,
    }
    struct Captured {
        headers: String,
        body: Vec<u8>,
    }

    fn fixture(
        config: StatusConfig,
        replies: Vec<Reply>,
    ) -> (GithubStatusClient, thread::JoinHandle<Vec<Captured>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|_| unreachable!("loopback HTTP fixture listener binds"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|_| unreachable!("fixture listener supports bounded accept"));
        let endpoint = format!(
            "http://{}",
            listener
                .local_addr()
                .unwrap_or_else(|_| unreachable!("fixture listener exposes its bound address"))
        );
        let server = thread::spawn(move || {
            let mut captures = Vec::new();
            for reply in replies {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(_) => unreachable!(
                            "fixture receives the expected HTTP request before its deadline"
                        ),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap_or_else(|_| unreachable!("fixture connection supports bounded reads"));
                let mut data = Vec::new();
                loop {
                    let mut byte = [0];
                    stream
                        .read_exact(&mut byte)
                        .unwrap_or_else(|_| unreachable!("fixture receives the complete request"));
                    data.extend_from_slice(&byte);
                    assert!(data.len() <= 16_384);
                    if data.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                let headers = String::from_utf8(data)
                    .unwrap_or_else(|_| unreachable!("fixture HTTP headers are UTF-8"));
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length").then(|| {
                            value.trim().parse::<usize>().unwrap_or_else(|_| {
                                unreachable!("fixture request has an integer content length")
                            })
                        })
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream
                    .read_exact(&mut body)
                    .unwrap_or_else(|_| unreachable!("fixture receives the complete request"));
                captures.push(Captured { headers, body });
                match reply {
                    Reply::Http(response) => {
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Reply::Disconnect => {}
                    Reply::Delay => thread::sleep(Duration::from_millis(750)),
                }
            }
            captures
        });
        let client = GithubStatusClient::new_fixture(
            config,
            ResolvedSecret::from_bytes(b"fixture-token".to_vec())
                .unwrap_or_else(|_| unreachable!("fixture secret is admitted")),
            &endpoint,
        )
        .unwrap_or_else(|_| unreachable!("loopback fixture status client is admitted"));
        (client, server)
    }

    // r[verify kiln.github.status]
    #[test]
    fn plan_identity_binds_every_semantic_field_and_full_run_marker() {
        let original = plan(StatusState::Success);
        assert!(original.description().len() <= 140);
        assert!(
            original
                .description()
                .contains(original.run_identity.as_str())
        );
        let identity = request_identity(&original)
            .unwrap_or_else(|_| unreachable!("admitted fixture plan has a request identity"));
        let mut changes = Vec::new();
        let mut changed = original.clone();
        changed.owner = "other".into();
        changes.push(changed);
        let mut changed = original.clone();
        changed.repository = "other".into();
        changes.push(changed);
        let mut changed = original.clone();
        changed.context = "kiln/other".into();
        changes.push(changed);
        let mut changed = original.clone();
        changed.revision = "c".repeat(40);
        changes.push(changed);
        let mut changed = original.clone();
        changed.run_identity = ContentId::from_blake3_hex("d".repeat(64))
            .unwrap_or_else(|_| unreachable!("fixture BLAKE3 identity is canonical"));
        changes.push(changed);
        let mut changed = original.clone();
        changed.state = StatusState::Failure;
        changes.push(changed);
        for changed in changes {
            assert_ne!(
                request_identity(&changed).unwrap_or_else(|_| unreachable!(
                    "admitted fixture plan has a request identity"
                )),
                identity
            );
        }
        assert_ne!(
            original.description(),
            plan(StatusState::Pending).description()
        );
        assert!(
            plan_status(
                &config(),
                "main",
                original.run_identity.clone(),
                original.state
            )
            .is_err()
        );
        let mut invalid = config();
        invalid.repository = "../other".into();
        assert!(admit_status_config(&invalid).is_err());
        invalid = config();
        invalid.maximum_pages = 0;
        assert!(admit_status_config(&invalid).is_err());
    }

    // r[verify kiln.github.status]
    #[test]
    fn real_http_publish_retains_exact_projection_and_uses_fixed_protocol() {
        let plan = plan(StatusState::Success);
        let (mut client, server) = fixture(
            config(),
            vec![Reply::Http(response(
                201,
                &remote(&plan, 42).to_string(),
                "",
            ))],
        );
        let publication = client
            .publish(&plan)
            .unwrap_or_else(|_| unreachable!("matching fixture publication succeeds"));
        assert_eq!(
            publication,
            Publication {
                status_id: 42,
                request_identity: request_identity(&plan).unwrap_or_else(|_| unreachable!(
                    "admitted fixture plan has a request identity"
                ))
            }
        );
        let captures = server
            .join()
            .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        let request = captures
            .first()
            .unwrap_or_else(|| unreachable!("POST fixture captures its request"));
        assert!(request.headers.starts_with(&format!(
            "POST /repos/example/project/statuses/{} HTTP/1.1",
            plan.revision
        )));
        let lower = request.headers.to_ascii_lowercase();
        for header in [
            "authorization: bearer fixture-token",
            "accept: application/vnd.github+json",
            "x-github-api-version: 2022-11-28",
        ] {
            assert!(lower.contains(header));
        }
        let body: Value = serde_json::from_slice(&request.body)
            .unwrap_or_else(|_| unreachable!("fixture HTTP request is JSON"));
        assert_eq!(
            body,
            json!({"context": plan.context, "description": plan.description(), "state": "success"})
        );
    }

    // r[verify kiln.github.status]
    #[test]
    fn post_mismatches_malformed_lost_timeout_and_oversized_responses_are_unknown() {
        let plan = plan(StatusState::Success);
        let original = remote(&plan, 7);
        for (field, replacement) in [
            ("context", json!("other")),
            ("state", json!("failure")),
            ("description", json!("truncated")),
            (
                "url",
                json!("https://api.github.com/repos/other/repo/statuses/wrong"),
            ),
            ("id", json!(0)),
        ] {
            let mut wrong = original.clone();
            *wrong.get_mut(field).unwrap_or_else(|| {
                unreachable!("remote status fixture contains the field being corrupted")
            }) = replacement;
            let (mut client, server) = fixture(
                config(),
                vec![Reply::Http(response(201, &wrong.to_string(), ""))],
            );
            assert!(matches!(
                client.publish(&plan),
                Err(PortFailure::Unknown(_))
            ));
            assert_eq!(
                server
                    .join()
                    .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"))
                    .len(),
                1
            );
        }
        for reply in [
            Reply::Http(response(201, "{", "")),
            Reply::Disconnect,
            Reply::Delay,
            Reply::Http(response(503, "provider private message", "")),
            Reply::Http(response(200, "{}", "")),
        ] {
            let (mut client, server) = fixture(config(), vec![reply]);
            let error = client
                .publish(&plan)
                .err()
                .unwrap_or_else(|| unreachable!("fixture operation must fail"));
            assert!(matches!(error, PortFailure::Unknown(_)));
            assert!(!format!("{error:?}").contains("private message"));
            assert_eq!(
                server
                    .join()
                    .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"))
                    .len(),
                1
            );
        }
        let mut small = config();
        small.maximum_response_bytes = 8;
        let (mut client, server) = fixture(
            small,
            vec![Reply::Http(response(201, &original.to_string(), ""))],
        );
        assert!(matches!(
            client.publish(&plan),
            Err(PortFailure::Unknown(_))
        ));
        server
            .join()
            .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
    }

    // r[verify kiln.github.status]
    #[test]
    fn denied_rate_limited_and_redirect_responses_never_follow_or_retry() {
        for (code, extra, rate_limit) in [
            (401, "", false),
            (403, "", false),
            (429, "", true),
            (403, "X-RateLimit-Remaining: 0\r\n", true),
            (302, "Location: http://127.0.0.1:1/leak\r\n", false),
        ] {
            let (mut client, server) = fixture(
                config(),
                vec![Reply::Http(response(
                    code,
                    "private-provider-response",
                    extra,
                ))],
            );
            let error = client
                .publish(&plan(StatusState::Pending))
                .err()
                .unwrap_or_else(|| unreachable!("fixture operation must fail"));
            if rate_limit {
                assert!(matches!(error, PortFailure::Retryable(_)));
            } else {
                assert!(matches!(error, PortFailure::Domain(_)));
            }
            assert!(!format!("{error:?}").contains("private-provider-response"));
            assert_eq!(
                server
                    .join()
                    .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"))
                    .len(),
                1
            );
        }
        for endpoint in [
            "http://example.com:80",
            "https://127.0.0.1:80",
            "http://127.0.0.1:80/path",
            "http://user@127.0.0.1:80",
            "http://127.0.0.1:0",
        ] {
            assert!(
                GithubStatusClient::new_fixture(
                    config(),
                    ResolvedSecret::from_bytes(b"fixture".to_vec())
                        .unwrap_or_else(|_| unreachable!("fixture secret is admitted")),
                    endpoint
                )
                .is_err()
            );
        }
    }

    // r[verify kiln.github.status]
    #[test]
    fn reconciliation_allows_lifecycle_history_but_rejects_conflicting_exact_markers() {
        let plan = plan(StatusState::Success);
        let pending = super::tests::plan(StatusState::Pending);
        let error = super::tests::plan(StatusState::Error);
        let statuses = json!([remote(&plan, 10), remote(&pending, 8), remote(&error, 9)]);
        let (mut client, server) = fixture(
            config(),
            vec![Reply::Http(response(200, &statuses.to_string(), ""))],
        );
        assert_eq!(
            client
                .reconcile(&plan)
                .unwrap_or_else(|_| unreachable!("complete fixture reconciliation succeeds")),
            Some(Publication {
                status_id: 10,
                request_identity: request_identity(&plan).unwrap_or_else(|_| unreachable!(
                    "admitted fixture plan has a request identity"
                ))
            })
        );
        let captures = server
            .join()
            .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        assert!(
            captures
                .first()
                .unwrap_or_else(|| unreachable!("reconciliation fixture captures its GET request"))
                .headers
                .starts_with(&format!(
                    "GET /repos/example/project/commits/{}/statuses?per_page=100&page=1 HTTP/1.1",
                    plan.revision
                ))
        );
        for (field, value) in [
            ("state", json!("failure")),
            ("context", json!("another-context")),
            (
                "url",
                json!("https://api.github.com/repos/example/project/statuses/other"),
            ),
        ] {
            let mut conflicting = remote(&plan, 11);
            *conflicting.get_mut(field).unwrap_or_else(|| {
                unreachable!("remote status fixture contains the conflicting field")
            }) = value;
            let (mut client, server) = fixture(
                config(),
                vec![Reply::Http(response(
                    200,
                    &json!([remote(&plan, 10), conflicting]).to_string(),
                    "",
                ))],
            );
            assert!(matches!(
                client.reconcile(&plan),
                Err(PortFailure::Unknown(_))
            ));
            server
                .join()
                .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        }
    }

    // r[verify kiln.github.status]
    #[test]
    fn reconciliation_paginates_and_exhaustion_cannot_prove_absence_or_complete_match() {
        let expected = plan(StatusState::Success);
        let unrelated = plan(StatusState::Pending);
        let page = Value::Array((1..=100).map(|id| remote(&unrelated, id)).collect());
        let (mut client, server) = fixture(
            config(),
            vec![
                Reply::Http(response(200, &page.to_string(), "")),
                Reply::Http(response(
                    200,
                    &json!([remote(&expected, 101)]).to_string(),
                    "",
                )),
            ],
        );
        assert_eq!(
            client
                .reconcile(&expected)
                .unwrap_or_else(|_| unreachable!("bounded exact reconciliation succeeds"))
                .unwrap_or_else(|| unreachable!("second page contains the expected status"))
                .status_id,
            101
        );
        let requests = server
            .join()
            .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        assert!(
            requests
                .get(1)
                .unwrap_or_else(|| unreachable!("reconciliation fetches a second page"))
                .headers
                .contains("page=2")
        );
        let mut bounded = config();
        bounded.maximum_pages = 1;
        for body in [
            page.to_string(),
            json!([remote(&expected, 101)]).to_string(),
        ] {
            let (mut client, server) = fixture(
                bounded.clone(),
                vec![Reply::Http(response(
                    200,
                    &body,
                    "Link: <https://api.github.com/ignored>; rel=\"next\"\r\n",
                ))],
            );
            assert!(matches!(
                client.reconcile(&expected),
                Err(PortFailure::Unknown(_))
            ));
            server
                .join()
                .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        }
        let (mut client, server) = fixture(config(), vec![Reply::Http(response(200, "[]", ""))]);
        assert_eq!(
            client
                .reconcile(&expected)
                .unwrap_or_else(|_| unreachable!("complete empty status lookup succeeds")),
            None
        );
        server
            .join()
            .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        for reply in [
            Reply::Http(response(503, "private", "")),
            Reply::Http(response(200, "not-json", "")),
            Reply::Delay,
        ] {
            let (mut client, server) = fixture(config(), vec![reply]);
            assert!(client.reconcile(&expected).is_err());
            server
                .join()
                .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        }
    }

    // r[verify kiln.github.status]
    #[test]
    fn provider_repository_case_normalization_preserves_exact_origin_endpoint_and_revision() {
        let config = StatusConfig {
            owner: "onixresearch".into(),
            repository: "kiln".into(),
            ..config()
        };
        let expected = plan_status(
            &config,
            &"a".repeat(40),
            ContentId::from_blake3_hex("b".repeat(64))
                .unwrap_or_else(|_| unreachable!("fixture BLAKE3 identity is canonical")),
            StatusState::Success,
        )
        .unwrap_or_else(|_| unreachable!("fixture status plan is admitted"));
        let mut mixed_case = remote(&expected, 91);
        *mixed_case
            .get_mut("url")
            .unwrap_or_else(|| unreachable!("remote status fixture contains its commit URL")) =
            json!(format!(
                "https://api.github.com/repos/OnixResearch/Kiln/statuses/{}",
                expected.revision
            ));
        let (mut client, server) = fixture(
            config.clone(),
            vec![
                Reply::Http(response(201, &mixed_case.to_string(), "")),
                Reply::Http(response(200, &json!([mixed_case]).to_string(), "")),
            ],
        );
        let publication = client
            .publish(&expected)
            .unwrap_or_else(|_| unreachable!("matching fixture publication succeeds"));
        assert_eq!(publication.status_id, 91);
        assert_eq!(
            client
                .reconcile(&expected)
                .unwrap_or_else(|_| unreachable!("case-normalized exact reconciliation succeeds")),
            Some(publication)
        );
        server
            .join()
            .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        for invalid_url in [
            format!(
                "http://api.github.com/repos/OnixResearch/Kiln/statuses/{}",
                expected.revision
            ),
            format!(
                "https://api.github.com.evil.invalid/repos/OnixResearch/Kiln/statuses/{}",
                expected.revision
            ),
            format!(
                "https://api.github.com:444/repos/OnixResearch/Kiln/statuses/{}",
                expected.revision
            ),
            format!(
                "https://user@api.github.com/repos/OnixResearch/Kiln/statuses/{}",
                expected.revision
            ),
            format!(
                "https://api.github.com/repos/OnixResearch/Kiln/Statuses/{}",
                expected.revision
            ),
            format!(
                "https://api.github.com/repos/OnixResearch/Kiln/statuses/{}",
                "A".repeat(40)
            ),
            format!(
                "https://api.github.com/repos/OnixResearch/Kiln/statuses/{}",
                "c".repeat(40)
            ),
            format!(
                "https://api.github.com/repos/OnixResearch/Kiln/statuses/{}?other=true",
                expected.revision
            ),
        ] {
            let mut remote = remote(&expected, 92);
            *remote
                .get_mut("url")
                .unwrap_or_else(|| unreachable!("remote status fixture contains its commit URL")) =
                json!(invalid_url);
            let (mut client, server) = fixture(
                config.clone(),
                vec![
                    Reply::Http(response(201, &remote.to_string(), "")),
                    Reply::Http(response(200, &json!([remote]).to_string(), "")),
                ],
            );
            assert!(matches!(
                client.publish(&expected),
                Err(PortFailure::Unknown(_))
            ));
            assert!(matches!(
                client.reconcile(&expected),
                Err(PortFailure::Unknown(_))
            ));
            server
                .join()
                .unwrap_or_else(|_| unreachable!("HTTP fixture finishes its exchange"));
        }
    }

    // r[verify kiln.github.secrets]
    #[test]
    fn client_debug_discloses_neither_token_material_nor_length() {
        let short = GithubStatusClient::new_fixture(
            config(),
            ResolvedSecret::from_bytes(b"private-short-token".to_vec())
                .unwrap_or_else(|_| unreachable!("fixture secret is admitted")),
            "http://127.0.0.1:1",
        )
        .unwrap_or_else(|_| unreachable!("loopback fixture status client is admitted"));
        let long = GithubStatusClient::new_fixture(
            config(),
            ResolvedSecret::from_bytes(b"private-much-longer-token-material".to_vec())
                .unwrap_or_else(|_| unreachable!("fixture secret is admitted")),
            "http://127.0.0.1:1",
        )
        .unwrap_or_else(|_| unreachable!("loopback fixture status client is admitted"));
        let debug = format!("{short:?}");
        assert!(
            !debug.contains(
                short
                    .token
                    .as_str()
                    .unwrap_or_else(|_| unreachable!("admitted fixture token is UTF-8"))
            )
        );
        assert_eq!(debug, format!("{long:?}"));
    }
}

#[cfg(test)]
mod dns_tests {
    use super::*;
    use std::time::Instant;

    fn process_is_running(pid: &str) -> bool {
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        // A killed descendant may briefly be a zombie awaiting the system reaper.
        stat.rsplit_once(") ")
            .is_some_and(|(_, rest)| !rest.starts_with('Z'))
    }

    // r[verify kiln.github.status]
    #[test]
    fn stalled_resolver_subprocess_deadline_kills_and_reaps_its_process_group() {
        let root = tempfile::tempdir()
            .unwrap_or_else(|_| unreachable!("fixture temporary directory is available"));
        let leader = root.path().join("leader");
        let descendant = root.path().join("descendant");
        // Pass the owned script as data to an immutable executable. A freshly
        // written executable can inherit transient writers into sibling forks
        // and fail exec with ETXTBSY under the parallel suite.
        let script = format!(
            "[ -z \"$HOME\" ] || exit 2\nprintf '%s' \"$$\" > '{}'\nstall() {{ while :; do :; done; }}\nstall &\nprintf '%s' \"$!\" > '{}'\nwait\n",
            leader.display(),
            descendant.display()
        );
        let resolver = BoundedGithubResolver {
            program: PathBuf::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from(script)],
            timeout_ms: 500,
        };
        let started = Instant::now();
        let error = resolver
            .resolve(DNS_DESTINATION)
            .err()
            .unwrap_or_else(|| unreachable!("fixture operation must fail"));
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(4));
        let leader = std::fs::read_to_string(leader)
            .unwrap_or_else(|_| unreachable!("resolver fixture records its process identity"));
        let descendant = std::fs::read_to_string(descendant)
            .unwrap_or_else(|_| unreachable!("resolver fixture records its process identity"));
        assert!(!process_is_running(&leader));
        assert!(!process_is_running(&descendant));
        // Bounded Exec owns and waits for the direct child; no unreaped leader.
        assert!(!std::path::Path::new(&format!("/proc/{leader}")).exists());
    }

    // r[verify kiln.github.status]
    #[test]
    fn dns_protocol_admits_only_bounded_fixed_port_addresses_and_fixed_destination() {
        let resolver = |output: &str| BoundedGithubResolver {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                OsString::from("-c"),
                OsString::from("printf '%s' \"$1\""),
                OsString::from("dns-fixture"),
                OsString::from(output),
            ],
            timeout_ms: 500,
        };
        let addresses = resolver("140.82.114.5:443\n[2606:50c0:8000::154]:443\n140.82.114.5:443\n")
            .resolve(DNS_DESTINATION)
            .unwrap_or_else(|_| unreachable!("fixture DNS subprocess returns admitted addresses"));
        assert_eq!(
            addresses,
            vec![
                "140.82.114.5:443"
                    .parse::<SocketAddr>()
                    .unwrap_or_else(|_| unreachable!("fixture IPv4 socket address is valid")),
                "[2606:50c0:8000::154]:443"
                    .parse::<SocketAddr>()
                    .unwrap_or_else(|_| unreachable!("fixture IPv6 socket address is valid"))
            ]
        );
        for output in [
            "",
            "not-an-address\n",
            "140.82.114.5:80\n",
            &"140.82.114.5:443\n".repeat(300),
        ] {
            assert!(resolver(output).resolve(DNS_DESTINATION).is_err());
        }
        assert_eq!(
            resolver("140.82.114.5:443\n")
                .resolve("other.example:443")
                .err()
                .unwrap_or_else(|| unreachable!("resolver rejects non-GitHub destinations"))
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
}
