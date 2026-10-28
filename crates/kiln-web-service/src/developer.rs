//! Explicit, profile-pinned HTTPS developer client. Mutations are sent once.
// r[impl kiln.web.developer_loop.cli]
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_PROFILE_BYTES: usize = 65_536;
const MAX_HTTP_BYTES: usize = 16_777_216;
const MAX_LOG_BYTES: u64 = 1_073_741_824;
const MAX_LOG_PAGES: usize = 1_048_576;
const MAX_WATCH_MS: u64 = 86_400_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeveloperProfile {
    pub schema: String,
    pub service_origin: String,
    pub token_file: PathBuf,
    pub tls_ca_path: PathBuf,
    pub http_timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub watch_timeout_ms: u64,
    pub maximum_request_bytes: usize,
    pub maximum_response_bytes: usize,
}
impl DeveloperProfile {
    pub fn admit(&self) -> Result<(), String> {
        let url =
            reqwest::Url::parse(&self.service_origin).map_err(|_| "developer origin invalid")?;
        if self.schema != "kiln.web-developer.v1"
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || self.service_origin.ends_with('/')
            || !self.token_file.is_absolute()
            || !self.tls_ca_path.is_absolute()
            || self.http_timeout_ms == 0
            || self.http_timeout_ms > MAX_WATCH_MS
            || self.poll_interval_ms == 0
            || self.poll_interval_ms > self.watch_timeout_ms
            || self.watch_timeout_ms == 0
            || self.watch_timeout_ms > MAX_WATCH_MS
            || self.maximum_request_bytes == 0
            || self.maximum_request_bytes > MAX_HTTP_BYTES
            || self.maximum_response_bytes == 0
            || self.maximum_response_bytes > MAX_HTTP_BYTES
        {
            return Err("developer profile invalid".into());
        }
        Ok(())
    }
}
pub fn load_profile(path: &Path, identity: &str) -> Result<DeveloperProfile, String> {
    if !crate::snapshot::hex(identity, 32) {
        return Err("developer profile identity invalid".into());
    }
    let path = absolute(path)?;
    let bytes = crate::worker::files::regular(&path, MAX_PROFILE_BYTES, false)?;
    if blake3::hash(&bytes).to_hex().as_str() != identity {
        return Err("developer original profile identity mismatch".into());
    }
    let profile: DeveloperProfile =
        serde_json::from_slice(&bytes).map_err(|_| "developer profile JSON invalid")?;
    profile.admit()?;
    Ok(profile)
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()
            .map_err(|_| "developer directory unavailable")?
            .join(path))
    }
}
struct Client {
    http: reqwest::blocking::Client,
    authorization: reqwest::header::HeaderValue,
    profile: DeveloperProfile,
}
impl Client {
    fn new(profile: DeveloperProfile) -> Result<Self, String> {
        profile.admit()?;
        let ca = crate::worker::files::regular(&profile.tls_ca_path, 1_048_576, false)?;
        let ca = reqwest::Certificate::from_pem(&ca).map_err(|_| "developer CA invalid")?;
        let token = zeroize::Zeroizing::new(crate::worker::files::regular(
            &profile.token_file,
            64,
            true,
        )?);
        let token = std::str::from_utf8(&token).map_err(|_| "developer credential invalid")?;
        if !crate::snapshot::hex(token, 32) {
            return Err("developer credential invalid".into());
        }
        let mut authorization = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "developer credential invalid")?;
        authorization.set_sensitive(true);
        let http = reqwest::blocking::Client::builder()
            .https_only(true)
            .tls_built_in_root_certs(false)
            .add_root_certificate(ca)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .timeout(Duration::from_millis(profile.http_timeout_ms))
            .build()
            .map_err(|_| "developer HTTPS client unavailable")?;
        Ok(Self {
            http,
            authorization,
            profile,
        })
    }
    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .request(method, format!("{}{}", self.profile.service_origin, path))
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone());
        if let Some(body) = body {
            let bytes =
                serde_json::to_vec(body).map_err(|_| "developer request encoding failed")?;
            if bytes.len() > self.profile.maximum_request_bytes {
                return Err("developer request exceeds bound".into());
            }
            request = request
                .header(reqwest::header::ORIGIN, &self.profile.service_origin)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(bytes);
        }
        let response=request.send().map_err(|_|"developer HTTPS request failed; mutation outcome may be unknown; do not blindly retry")?;
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|n| n > self.profile.maximum_response_bytes as u64)
        {
            return Err("developer response exceeds bound".into());
        }
        let mut bytes = Vec::new();
        response
            .take(self.profile.maximum_response_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "developer response read failed")?;
        if bytes.len() > self.profile.maximum_response_bytes {
            return Err("developer response exceeds bound".into());
        }
        if !status.is_success() {
            return Err(format!(
                "developer service rejected request (HTTP {})",
                status.as_u16()
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| "developer response JSON invalid".into())
    }
    fn run(&self, id: &str) -> Result<Value, String> {
        self.request(reqwest::Method::GET, &format!("/api/runs/{id}"), None)
    }
    fn watch(&self, id: &str) -> Result<i32, String> {
        let deadline = Instant::now() + Duration::from_millis(self.profile.watch_timeout_ms);
        let mut previous = None;
        loop {
            let run = self.run(id)?;
            let state = (
                text(&run, "phase")?.to_owned(),
                text(&run, "outcome")?.to_owned(),
            );
            if previous.as_ref() != Some(&state) {
                print_json(&run)?;
                previous = Some(state);
            }
            if let Some(code) = outcome_exit(text(&run, "outcome")?)? {
                return Ok(code);
            }
            self.pause(deadline)?;
        }
    }
    fn pause(&self, deadline: Instant) -> Result<(), String> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("developer watch deadline exceeded; run was not cancelled".into());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(self.profile.poll_interval_ms)));
        if Instant::now() >= deadline {
            return Err("developer watch deadline exceeded; run was not cancelled".into());
        }
        Ok(())
    }
    fn logs(&self, id: &str, follow: bool) -> Result<i32, String> {
        let deadline = Instant::now() + Duration::from_millis(self.profile.watch_timeout_ms);
        let mut cursors: BTreeMap<(String, &str), u64> = BTreeMap::new();
        let mut transferred = 0u64;
        let mut pages = 0usize;
        loop {
            let run = self.run(id)?;
            let terminal = outcome_exit(text(&run, "outcome")?)?;
            if text(&run, "phase")? != "queued" {
                let nodes = run
                    .get("nodes")
                    .and_then(Value::as_array)
                    .ok_or("developer run nodes invalid")?;
                let max = run
                    .pointer("/log_bounds/maximum_chunk_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or(4096)
                    .min(65_536)
                    .min((self.profile.maximum_response_bytes / 8) as u64);
                if max == 0 {
                    return Err("developer log response bound too small".into());
                }
                for node in nodes {
                    let node_id = text(node, "id")?;
                    if !crate::profile::identifier(node_id) {
                        return Err("developer log node invalid".into());
                    }
                    for stream in ["stdout", "stderr"] {
                        let cursor = cursors.entry((node_id.to_owned(), stream)).or_default();
                        loop {
                            pages += 1;
                            if pages > MAX_LOG_PAGES || Instant::now() >= deadline {
                                return Err("developer log traversal bound exceeded".into());
                            }
                            let page=self.request(reqwest::Method::GET,&format!("/api/runs/{id}/logs?node_id={node_id}&stream={stream}&cursor={cursor}&max_bytes={max}"),None)?;
                            let next = number(&page, "next_cursor")?;
                            if page.get("gap").and_then(Value::as_bool) != Some(false)
                                || number(&page, "cursor")? != *cursor
                                || next < *cursor
                                || next - *cursor > max
                            {
                                return Err("developer log cursor gap or invalid page".into());
                            }
                            let content = base64::engine::general_purpose::STANDARD
                                .decode(text(&page, "body_base64")?)
                                .map_err(|_| "developer log encoding invalid")?;
                            if content.len() as u64 != next - *cursor {
                                return Err("developer log byte count mismatch".into());
                            }
                            transferred = transferred
                                .checked_add(next - *cursor)
                                .ok_or("developer log byte bound")?;
                            if transferred > MAX_LOG_BYTES {
                                return Err("developer log byte bound".into());
                            }
                            if next == *cursor {
                                if !content.is_empty()
                                    || page.get("truncated").and_then(Value::as_bool) == Some(true)
                                {
                                    return Err("developer logs unavailable or truncated".into());
                                }
                                break;
                            }
                            if stream == "stdout" {
                                let mut output = std::io::stdout().lock();
                                output
                                    .write_all(&content)
                                    .and_then(|()| output.flush())
                                    .map_err(|_| "developer log output failed")?;
                            } else {
                                let mut output = std::io::stderr().lock();
                                output
                                    .write_all(&content)
                                    .and_then(|()| output.flush())
                                    .map_err(|_| "developer log output failed")?;
                            }
                            *cursor = next;
                        }
                    }
                }
            }
            if !follow {
                return Ok(0);
            }
            if let Some(code) = terminal {
                return Ok(code);
            }
            self.pause(deadline)?;
        }
    }
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("developer response field {key} invalid"))
}
fn number(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("developer response field {key} invalid"))
}
pub fn outcome_exit(outcome: &str) -> Result<Option<i32>, String> {
    match outcome {
        "Pending" => Ok(None),
        "Success" => Ok(Some(0)),
        "Cancelled" => Ok(Some(130)),
        "Unknown" => Ok(Some(3)),
        "DomainFailure" | "Denied" | "TimedOut" | "ProviderError" => Ok(Some(1)),
        _ => Err("developer run outcome invalid".into()),
    }
}
fn print_json(value: &Value) -> Result<(), String> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, value).map_err(|_| "developer output failed")?;
    writeln!(output).map_err(|_| "developer output failed".into())
}

/// Dispatch after `kiln-web-service ci`. Profile pin is over original JSON bytes.
pub fn cli_main(args: &[String]) -> Result<i32, String> {
    if args == ["--help"] || args == ["-h"] {
        println!(
            "Usage: kiln-web-service ci --profile PATH --identity BLAKE3 COMMAND [OPTIONS]\n\nCommands:\n  submit --project ID --request-id ID --reference REF --revision COMMIT\n         [--dirty-repository PATH --include FILE ... --delete FILE ...]\n  review-submit --request-id ID --source-ref REF --title TITLE\n         [--description TEXT] [--expected-commit COMMIT] [--wip]\n  watch --run ID\n  logs --run ID [--follow]\n  cancel --run ID --request-id ID\n  retry --run ID --request-id ID\n\nProfile: config/web-developer.ncl exported to JSON; --identity pins its original bytes.\nOnly explicit dirty selections are uploaded; ignored paths and links are denied.\nReview submission resolves an admitted branch in the service's configured local repository.\nWatch/follow exits: success 0, failed 1, unknown 3, cancelled 130.\nMutations are sent once; reuse the exact request ID and arguments to reconcile a review submission."
        );
        return Ok(0);
    }
    let (profile_path, identity, command, arguments) = match args {
        [
            profile_option,
            profile_path,
            identity_option,
            identity,
            command,
            arguments @ ..,
        ] if profile_option == "--profile" && identity_option == "--identity" => {
            (profile_path, identity, command.as_str(), arguments)
        }
        _ => return Err(
            "usage: ci --profile PATH --identity BLAKE3 submit|review-submit|watch|logs|cancel|retry [options]"
                .into(),
        ),
    };
    let profile = load_profile(Path::new(profile_path), identity)?;
    let mut options: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut arguments = arguments.iter();
    while let Some(key) = arguments.next() {
        let key = key.as_str();
        let allowed = match command {
            "submit" => matches!(
                key,
                "--project"
                    | "--request-id"
                    | "--reference"
                    | "--revision"
                    | "--dirty-repository"
                    | "--include"
                    | "--delete"
            ),
            "review-submit" => matches!(
                key,
                "--request-id"
                    | "--source-ref"
                    | "--title"
                    | "--description"
                    | "--expected-commit"
                    | "--wip"
            ),
            "watch" => key == "--run",
            "logs" => matches!(key, "--run" | "--follow"),
            "cancel" | "retry" => matches!(key, "--run" | "--request-id"),
            _ => false,
        };
        if !allowed {
            return Err("developer command or option invalid".into());
        }
        if options.contains_key(key) && !matches!(key, "--include" | "--delete") {
            return Err("developer duplicate option".into());
        }
        let value = if matches!(key, "--follow" | "--wip") {
            "true".to_owned()
        } else {
            let value = arguments.next().ok_or("developer option value missing")?;
            if value.starts_with("--") {
                return Err("developer option value missing".into());
            }
            value.clone()
        };
        options.entry(key).or_default().push(value);
    }
    let required = |key| {
        options
            .get(key)
            .and_then(|v| v.first())
            .map(String::as_str)
            .ok_or_else(|| format!("developer required option {key} missing"))
    };
    let client = Client::new(profile)?;
    if command == "review-submit" {
        let request_id = required("--request-id")?;
        let source_ref = required("--source-ref")?;
        let title = required("--title")?;
        let expected_commit = options
            .get("--expected-commit")
            .and_then(|values| values.first());
        if !crate::profile::identifier(request_id)
            || !source_ref.starts_with("refs/heads/")
            || !git2::Reference::is_valid_name(source_ref)
            || expected_commit.is_some_and(|commit| !crate::snapshot::hex(commit, 20))
        {
            return Err("developer review source or request identity invalid".into());
        }
        let description = options
            .get("--description")
            .and_then(|values| values.first())
            .map(String::as_str)
            .unwrap_or("");
        let body = json!({
            "request_id":request_id,"change_id":null,"expected_version":null,
            "action":{"type":"contribute","source_ref":source_ref,
                "expected_commit":expected_commit,"title":title,"description":description,
                "wip":options.contains_key("--wip")}
        });
        print_json(&client.request(
            reqwest::Method::POST,
            "/api/code-review/commands",
            Some(&body),
        )?)?;
        return Ok(0);
    }
    if command == "submit" {
        let project = required("--project")?;
        let request = required("--request-id")?;
        let reference = required("--reference")?;
        let revision = required("--revision")?;
        if !crate::profile::identifier(project)
            || !crate::profile::identifier(request)
            || !crate::snapshot::hex(revision, 20)
        {
            return Err("developer submit identity invalid".into());
        }
        let mut body = json!({"project_id":project,"request_id":request,"reference":reference,"source_revision":revision});
        if let Some(repository) = options
            .get("--dirty-repository")
            .and_then(|paths| paths.first())
        {
            let selected_root = git2::Repository::open(absolute(Path::new(repository))?)
                .map_err(|_| "developer dirty repository unavailable")?;
            let selected_root = selected_root
                .workdir()
                .ok_or("developer dirty worktree required")?;
            if client.profile.token_file.starts_with(selected_root) {
                return Err("developer credential must be outside dirty worktree".into());
            }
            let empty = Vec::new();
            let manifest = crate::snapshot::capture(
                &absolute(Path::new(repository))?,
                project.into(),
                reference.into(),
                revision.into(),
                options.get("--include").unwrap_or(&empty),
                options.get("--delete").unwrap_or(&empty),
            )?;
            let record = client.request(
                reqwest::Method::POST,
                "/api/snapshots",
                Some(
                    &serde_json::to_value(&manifest)
                        .map_err(|_| "developer snapshot encoding failed")?,
                ),
            )?;
            let record: crate::snapshot::SnapshotRecord = serde_json::from_value(record)
                .map_err(|_| "developer snapshot response invalid")?;
            if record.manifest != manifest
                || record.provenance.manifest_id != manifest.identity()?
                || record.provenance.base_revision != revision
                || !crate::snapshot::hex(&record.provenance.synthetic_revision, 20)
            {
                return Err("developer snapshot response identity mismatch".into());
            }
            body.as_object_mut()
                .ok_or("developer submit body invalid")?
                .insert("snapshot_id".into(), json!(record.provenance.manifest_id));
        } else if options.contains_key("--include") || options.contains_key("--delete") {
            return Err("developer dirty selection requires --dirty-repository".into());
        }
        print_json(&client.request(reqwest::Method::POST, "/api/runs", Some(&body))?)?;
        return Ok(0);
    }
    let run = required("--run")?;
    if !crate::profile::identifier(run) {
        return Err("developer run identity invalid".into());
    }
    match command {
        "watch" => client.watch(run),
        "logs" => client.logs(run, options.contains_key("--follow")),
        "retry" => {
            let request = required("--request-id")?;
            if !crate::profile::identifier(request) {
                return Err("developer request identity invalid".into());
            }
            print_json(&client.request(
                reqwest::Method::POST,
                &format!("/api/runs/{run}/rerun"),
                Some(&json!({"request_id":request,"mode":"exact"})),
            )?)?;
            Ok(0)
        }
        "cancel" => {
            let request = required("--request-id")?;
            if !crate::profile::identifier(request) {
                return Err("developer request identity invalid".into());
            }
            let current = client.run(run)?;
            let body = if text(&current, "phase")? == "queued" {
                json!({"request_id":request,"action":"cancel"})
            } else {
                if current.get("execution").is_none_or(Value::is_null) {
                    return Err("developer run has no cancellable execution".into());
                }
                json!({"request_id":request,"action":"cancel","attempt_id":current.get("attempt_id").ok_or("developer attempt missing")?,
                    "expected_revision":number(&current,"revision")?,"expected_sequence":number(&current,"sequence")?,"execution":current.get("execution").ok_or("developer execution missing")?})
            };
            print_json(&client.request(
                reqwest::Method::POST,
                &format!("/api/runs/{run}/control"),
                Some(&body),
            )?)?;
            Ok(0)
        }
        _ => Err("developer command invalid".into()),
    }
}
