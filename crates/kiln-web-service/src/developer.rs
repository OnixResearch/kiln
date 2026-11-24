//! Explicit, profile-pinned HTTPS developer client. Mutations are sent once.
// r[impl kiln.web.developer_loop.cli]
use base64::Engine as _;
use kiln_core::change_review::{Authorship, validate_authorship};
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
            if let Ok(error_body) = serde_json::from_slice::<Value>(&bytes)
                && let (Some(code), Some(message)) = (
                    error_body.get("code").and_then(Value::as_str),
                    error_body.get("message").and_then(Value::as_str),
                )
            {
                return Err(format!(
                    "developer service rejected request (HTTP {}, {code}): {message}",
                    status.as_u16()
                ));
            }
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
            "Usage: kiln-web-service ci --profile PATH --identity BLAKE3 COMMAND [OPTIONS]\n\nCommands:\n  submit --project ID --request-id ID --reference REF --revision COMMIT\n         [--dirty-repository PATH --include FILE ... --delete FILE ...]\n  review-submit --request-id ID --source-ref REF --title TITLE\n         [--description TEXT] [--expected-commit COMMIT] [--wip]\n         [--authorship undeclared|human|agent|mixed]\n         [--agent-harness TEXT [--agent-model TEXT]]\n  dispatch --runbook ID --target REPO@REF --request-id ID\n           [--param KEY=VALUE ...] [--output json|text]\n  watch --run ID\n  logs --run ID [--follow]\n  cancel --run ID --request-id ID\n  retry --run ID --request-id ID\n\nProfile: config/web-developer.ncl exported to JSON; --identity pins its original bytes.\nOnly explicit dirty selections are uploaded; ignored paths and links are denied.\nReview submission resolves an admitted branch in the service's configured local repository.\nDeclared authorship is self-declared and unverified; it grants no authority.\nWatch/follow exits: success 0, failed 1, unknown 3, cancelled 130.\nMutations are sent once; reuse the exact request ID and arguments to reconcile a review submission."
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
            "usage: ci --profile PATH --identity BLAKE3 submit|review-submit|dispatch|watch|logs|cancel|retry [options]"
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
                    | "--authorship"
                    | "--agent-harness"
                    | "--agent-model"
                    | "--jj-change"
                    | "--stack"
                    | "--change-id-header"
            ),
            "dispatch" => matches!(
                key,
                "--runbook" | "--target" | "--request-id" | "--param" | "--output"
            ),
            "watch" => key == "--run",
            "logs" => matches!(key, "--run" | "--follow"),
            "cancel" | "retry" => matches!(key, "--run" | "--request-id"),
            _ => false,
        };
        if !allowed {
            return Err("developer command or option invalid".into());
        }
        if options.contains_key(key)
            && !matches!(key, "--include" | "--delete" | "--jj-change" | "--param")
        {
            return Err("developer duplicate option".into());
        }
        let value = if matches!(key, "--follow" | "--wip" | "--stack" | "--change-id-header") {
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
    if command == "dispatch" {
        let (runbook, output, fallback_body) = dispatch_body(&options)?;
        let body = if options.contains_key("--param") {
            let param_kinds = client
                .request(reqwest::Method::GET, "/api/runbooks", None)
                .ok()
                .and_then(|resp| extract_runbook_param_kinds(&resp, &runbook));
            match param_kinds {
                Some(kinds) => dispatch_body_with_schema(&options, Some(&kinds))?.2,
                None => fallback_body,
            }
        } else {
            fallback_body
        };
        let response = client.request(
            reqwest::Method::POST,
            &format!("/api/runbooks/{runbook}/dispatch"),
            Some(&body),
        )?;
        if output == "text" {
            print!("{}", format_dispatch_text(&response)?);
        } else {
            print_json(&response)?;
        }
        return Ok(0);
    }
    if command == "review-submit" {
        let body = review_submit_body(&options)?;
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

/// Parse `review-submit` authorship flags into a validated self-declaration.
/// Harness/model are accepted only with `--authorship agent`; absent means undeclared.
// r[impl kiln.review.authorship]
pub fn review_authorship(
    kind: Option<&str>,
    harness: Option<&str>,
    model: Option<&str>,
) -> Result<Authorship, String> {
    let authorship = match (kind.unwrap_or("undeclared"), harness, model) {
        ("agent", Some(harness), model) => Authorship::Agent {
            harness: harness.into(),
            model: model.map(Into::into),
        },
        ("agent", None, _) => {
            return Err("developer --agent-harness is required for agent authorship".into());
        }
        (_, Some(_), _) | (_, _, Some(_)) => {
            return Err(
                "developer --agent-harness/--agent-model require --authorship agent".into(),
            );
        }
        ("undeclared", ..) => Authorship::Undeclared,
        ("human", ..) => Authorship::Human,
        ("mixed", ..) => Authorship::Mixed,
        _ => return Err("developer --authorship invalid".into()),
    };
    validate_authorship(&authorship)
        .map_err(|_| "developer agent harness or model invalid".to_owned())?;
    Ok(authorship)
}

/// Pure: build the `review-submit` command body. `--jj-change` (repeatable,
/// bottom to top) or `--source-ref … --change-id-header` select change-ID
/// intake; titles then come from commit messages, never from flags.
// r[impl kiln.review.jj_intake.surface]
fn review_submit_body(options: &BTreeMap<&str, Vec<String>>) -> Result<Value, String> {
    let first = |key| {
        options
            .get(key)
            .and_then(|values| values.first())
            .map(String::as_str)
    };
    let request_id =
        first("--request-id").ok_or("developer required option --request-id missing")?;
    if !crate::profile::identifier(request_id) {
        return Err("developer review source or request identity invalid".into());
    }
    let authorship = review_authorship(
        first("--authorship"),
        first("--agent-harness"),
        first("--agent-model"),
    )?;
    let wip = options.contains_key("--wip");
    let stack = options.contains_key("--stack");
    let header = options.contains_key("--change-id-header");
    let change_ids = options.get("--jj-change");
    let source_ref = first("--source-ref");
    if source_ref.is_some_and(|source| {
        !source.starts_with("refs/heads/") || !git2::Reference::is_valid_name(source)
    }) {
        return Err("developer review source or request identity invalid".into());
    }
    let mut action = if change_ids.is_some() || header {
        if ["--title", "--description", "--expected-commit"]
            .iter()
            .any(|key| options.contains_key(key))
        {
            return Err("developer change-ID intake takes titles from commit messages".into());
        }
        match (change_ids, source_ref, header) {
            (Some(ids), None, false) => {
                if ids
                    .iter()
                    .any(|id| !kiln_core::source::valid_jujutsu_change_id(id))
                {
                    return Err("developer --jj-change requires full exact jj change IDs".into());
                }
                if ids.len() > 1 && !stack {
                    return Err("developer several --jj-change values require --stack".into());
                }
                json!({"type":"jj_contribute","change_ids":ids,"stack":stack,"wip":wip})
            }
            (None, Some(source), true) => {
                json!({"type":"jj_contribute","source_ref":source,"stack":stack,"wip":wip})
            }
            _ => {
                return Err(
                    "developer use either --jj-change or --source-ref with --change-id-header"
                        .into(),
                );
            }
        }
    } else {
        if stack {
            return Err("developer --stack requires change-ID intake".into());
        }
        let source_ref = source_ref.ok_or("developer required option --source-ref missing")?;
        let title = first("--title").ok_or("developer required option --title missing")?;
        let expected_commit = first("--expected-commit");
        if expected_commit.is_some_and(|commit| !crate::snapshot::hex(commit, 20)) {
            return Err("developer review source or request identity invalid".into());
        }
        json!({"type":"contribute","source_ref":source_ref,
            "expected_commit":expected_commit,"title":title,
            "description":first("--description").unwrap_or(""),"wip":wip})
    };
    if !authorship.is_undeclared()
        && let Some(map) = action.as_object_mut()
    {
        map.insert(
            "authorship".into(),
            serde_json::to_value(&authorship).map_err(|_| "developer authorship encoding")?,
        );
    }
    Ok(json!({
        "request_id":request_id,"change_id":null,"expected_version":null,"action":action
    }))
}

pub fn parse_dispatch_target(raw: &str) -> Result<(&str, &str), String> {
    if raw.is_empty()
        || raw.len() > 1280
        || raw
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return Err("developer --target requires REPO@REF".into());
    }
    let Some((repo, reference)) = raw.split_once('@') else {
        return Err("developer --target requires REPO@REF".into());
    };
    if repo.is_empty()
        || repo.len() > 256
        || reference.is_empty()
        || reference.len() > 1024
        || reference.contains('@')
        || reference.starts_with('-')
        || reference.contains("..")
    {
        return Err("developer --target requires REPO@REF".into());
    }
    Ok((repo, reference))
}

fn is_canonical_i64(raw: &str) -> bool {
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
}

pub fn extract_runbook_param_kinds(
    list_response: &Value,
    runbook_id: &str,
) -> Option<BTreeMap<String, String>> {
    if list_response
        .get("schema")
        .and_then(Value::as_str)
        .is_some_and(|s| s != crate::runbooks::RUNBOOKS_VIEW_SCHEMA)
    {
        return None;
    }
    let runbooks = list_response.get("runbooks")?.as_array()?;
    let runbook = runbooks
        .iter()
        .find(|rb| rb.get("id").and_then(Value::as_str) == Some(runbook_id))?;
    let params = runbook.get("params")?.as_array()?;
    let mut kinds = BTreeMap::new();
    for param in params {
        if let (Some(name), Some(kind)) = (
            param.get("name").and_then(Value::as_str),
            param.get("kind").and_then(Value::as_str),
        ) {
            kinds.insert(name.to_owned(), kind.to_owned());
        }
    }
    Some(kinds)
}

pub fn parse_param_value(raw: &str) -> Value {
    match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ if is_canonical_i64(raw) => match raw.parse::<i64>() {
            Ok(n) => Value::Number(n.into()),
            Err(_) => Value::String(raw.to_owned()),
        },
        _ => Value::String(raw.to_owned()),
    }
}

pub fn parse_param_value_for_kind(raw: &str, kind: Option<&str>) -> Value {
    match kind {
        Some("bool") => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::String(raw.to_owned()),
        },
        Some("integer") => {
            if is_canonical_i64(raw)
                && let Ok(n) = raw.parse::<i64>()
            {
                Value::Number(n.into())
            } else {
                Value::String(raw.to_owned())
            }
        }
        Some("enum" | "relative_path") => Value::String(raw.to_owned()),
        _ => parse_param_value(raw),
    }
}

pub fn parse_dispatch_params(entries: &[String]) -> Result<serde_json::Map<String, Value>, String> {
    parse_dispatch_params_with_schema(entries, None)
}

pub fn parse_dispatch_params_with_schema(
    entries: &[String],
    param_kinds: Option<&BTreeMap<String, String>>,
) -> Result<serde_json::Map<String, Value>, String> {
    let mut map = serde_json::Map::new();
    for entry in entries {
        let Some((key, raw_value)) = entry.split_once('=') else {
            return Err("developer --param requires KEY=VALUE".into());
        };
        if !crate::runbooks::valid_param_name(key) {
            return Err("developer --param key invalid".into());
        }
        if map.contains_key(key) {
            return Err("developer duplicate --param key".into());
        }
        let kind = param_kinds
            .and_then(|kinds| kinds.get(key))
            .map(String::as_str);
        map.insert(key.to_owned(), parse_param_value_for_kind(raw_value, kind));
    }
    Ok(map)
}

pub fn dispatch_body(
    options: &BTreeMap<&str, Vec<String>>,
) -> Result<(String, String, Value), String> {
    dispatch_body_with_schema(options, None)
}

pub fn dispatch_body_with_schema(
    options: &BTreeMap<&str, Vec<String>>,
    param_kinds: Option<&BTreeMap<String, String>>,
) -> Result<(String, String, Value), String> {
    let first = |key| {
        options
            .get(key)
            .and_then(|values| values.first())
            .map(String::as_str)
    };
    let runbook = first("--runbook").ok_or("developer required option --runbook missing")?;
    if !crate::runbooks::valid_runbook_id(runbook) {
        return Err("developer runbook identity invalid".into());
    }
    let request_id =
        first("--request-id").ok_or("developer required option --request-id missing")?;
    if !crate::profile::identifier(request_id) {
        return Err("developer request identity invalid".into());
    }
    let target = first("--target").ok_or("developer required option --target missing")?;
    parse_dispatch_target(target)?;
    let output = first("--output").unwrap_or("json");
    if !matches!(output, "json" | "text") {
        return Err("developer --output must be json or text".into());
    }
    let empty = Vec::new();
    let params =
        parse_dispatch_params_with_schema(options.get("--param").unwrap_or(&empty), param_kinds)?;
    Ok((
        runbook.to_owned(),
        output.to_owned(),
        json!({
            "request_id": request_id,
            "target": target,
            "params": Value::Object(params),
        }),
    ))
}

pub fn format_dispatch_text(response: &Value) -> Result<String, String> {
    let runbook_id = text(response, "runbook_id")?;
    let run_id = text(response, "run_id")?;
    let target = text(response, "target")?;
    let source_revision = text(response, "source_revision")?;
    let request_id = text(response, "request_id")?;
    let replayed = response
        .get("replayed")
        .and_then(Value::as_bool)
        .ok_or("developer response field replayed invalid")?;
    Ok(format!(
        "runbook_id: {runbook_id}\nrun_id: {run_id}\ntarget: {target}\nsource_revision: {source_revision}\nreplayed: {replayed}\nrequest_id: {request_id}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify kiln.review.jj_intake.surface]
    #[test]
    fn review_submit_builds_change_id_intake_and_rejects_misuse() {
        const A: &str = "wnloytsvxwxwxtpqyuwxutmotlmslszv";
        const B: &str = "kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt";
        let build = |pairs: &[(&'static str, &[&str])]| {
            let options: BTreeMap<&str, Vec<String>> = pairs
                .iter()
                .map(|(key, values)| (*key, values.iter().map(|v| (*v).to_owned()).collect()))
                .collect();
            review_submit_body(&options)
        };
        let single = build(&[("--request-id", &["r1"]), ("--jj-change", &[A])]);
        assert_eq!(
            single
                .ok()
                .and_then(|body| body.pointer("/action").cloned()),
            Some(json!({"type":"jj_contribute","change_ids":[A],"stack":false,"wip":false}))
        );
        let stack = build(&[
            ("--request-id", &["r2"]),
            ("--jj-change", &[A, B]),
            ("--stack", &["true"]),
        ]);
        assert_eq!(
            stack
                .ok()
                .and_then(|body| body.pointer("/action/change_ids").cloned()),
            Some(json!([A, B]))
        );
        let header = build(&[
            ("--request-id", &["r3"]),
            ("--source-ref", &["refs/heads/review/x"]),
            ("--change-id-header", &["true"]),
            ("--stack", &["true"]),
        ]);
        assert_eq!(
            header
                .ok()
                .and_then(|body| body.pointer("/action/source_ref").cloned()),
            Some(json!("refs/heads/review/x"))
        );
        let branch = build(&[
            ("--request-id", &["r4"]),
            ("--source-ref", &["refs/heads/review/x"]),
            ("--title", &["T"]),
        ]);
        assert_eq!(
            branch
                .ok()
                .and_then(|body| body.pointer("/action/type").cloned()),
            Some(json!("contribute"))
        );
        for bad in [
            build(&[("--request-id", &["r"]), ("--jj-change", &["wnloyt"])]),
            build(&[("--request-id", &["r"]), ("--jj-change", &["@-"])]),
            build(&[("--request-id", &["r"]), ("--jj-change", &[A, B])]),
            build(&[
                ("--request-id", &["r"]),
                ("--jj-change", &[A]),
                ("--title", &["T"]),
            ]),
            build(&[
                ("--request-id", &["r"]),
                ("--jj-change", &[A]),
                ("--source-ref", &["refs/heads/review/x"]),
            ]),
            build(&[("--request-id", &["r"]), ("--change-id-header", &["true"])]),
            build(&[
                ("--request-id", &["r"]),
                ("--source-ref", &["refs/heads/review/x"]),
                ("--title", &["T"]),
                ("--stack", &["true"]),
            ]),
        ] {
            assert!(bad.is_err());
        }
    }

    // r[verify kiln.review.authorship.scenario.declared]
    // r[verify kiln.review.authorship.scenario.invalid]
    #[test]
    fn review_authorship_flags_build_declarations_and_reject_misuse() {
        let parse = |kind, harness, model| review_authorship(kind, harness, model);
        assert_eq!(parse(None, None, None), Ok(Authorship::Undeclared));
        assert_eq!(
            parse(Some("undeclared"), None, None),
            Ok(Authorship::Undeclared)
        );
        assert_eq!(parse(Some("human"), None, None), Ok(Authorship::Human));
        assert_eq!(parse(Some("mixed"), None, None), Ok(Authorship::Mixed));
        assert_eq!(
            parse(Some("agent"), Some("omp"), Some("claude")),
            Ok(Authorship::Agent {
                harness: "omp".into(),
                model: Some("claude".into())
            })
        );
        assert_eq!(
            parse(Some("agent"), Some("omp"), None),
            Ok(Authorship::Agent {
                harness: "omp".into(),
                model: None
            })
        );
        let oversized = "h".repeat(kiln_core::change_review::MAX_AUTHORSHIP_TEXT_BYTES + 1);
        for (kind, harness, model) in [
            (Some("agent"), None, None),
            (Some("agent"), None, Some("claude")),
            (Some("human"), Some("omp"), None),
            (None, None, Some("claude")),
            (Some("mixed"), Some("omp"), Some("claude")),
            (Some("robot"), None, None),
            (Some("Agent"), Some("omp"), None),
            (Some("agent"), Some(" omp"), None),
            (Some("agent"), Some("om\u{7}p"), None),
            (Some("agent"), Some(oversized.as_str()), None),
            (Some("agent"), Some("omp"), Some(oversized.as_str())),
        ] {
            assert!(parse(kind, harness, model).is_err(), "{kind:?} {harness:?}");
        }
    }

    #[test]
    fn dispatch_body_parses_targets_typed_params_and_rejects_invalid_flags() {
        let build = |pairs: &[(&'static str, &[&str])]| {
            let options: BTreeMap<&str, Vec<String>> = pairs
                .iter()
                .map(|(key, values)| (*key, values.iter().map(|v| (*v).to_owned()).collect()))
                .collect();
            dispatch_body(&options)
        };

        let (runbook, output, body) = build(&[
            ("--runbook", &["flake-update"]),
            ("--target", &["onixresearch/kiln@refs/heads/main"]),
            ("--request-id", &["req-1"]),
            (
                "--param",
                &["dry=true", "count=10", "negative=-3", "path=crates/kiln"],
            ),
            ("--output", &["text"]),
        ])
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(runbook, "flake-update");
        assert_eq!(output, "text");
        assert_eq!(
            body,
            json!({
                "request_id": "req-1",
                "target": "onixresearch/kiln@refs/heads/main",
                "params": {
                    "dry": true,
                    "count": 10,
                    "negative": -3,
                    "path": "crates/kiln",
                }
            })
        );

        let text = format_dispatch_text(&json!({
            "runbook_id": "flake-update",
            "run_id": "web-run-123",
            "target": "onixresearch/kiln@refs/heads/main",
            "source_revision": "a".repeat(40),
            "replayed": false,
            "request_id": "req-1"
        }))
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(text.contains("runbook_id: flake-update"));
        assert!(text.contains("run_id: web-run-123"));

        for bad in [
            build(&[("--target", &["repo@main"]), ("--request-id", &["r1"])]),
            build(&[
                ("--runbook", &["../bad"]),
                ("--target", &["repo@main"]),
                ("--request-id", &["r1"]),
            ]),
            build(&[
                ("--runbook", &["flake-update"]),
                ("--target", &["missing-at"]),
                ("--request-id", &["r1"]),
            ]),
            build(&[
                ("--runbook", &["flake-update"]),
                ("--target", &["repo@main"]),
                ("--request-id", &["r1"]),
                ("--param", &["no_equals"]),
            ]),
            build(&[
                ("--runbook", &["flake-update"]),
                ("--target", &["repo@main"]),
                ("--request-id", &["r1"]),
                ("--param", &["a=1", "a=2"]),
            ]),
            build(&[
                ("--runbook", &["flake-update"]),
                ("--target", &["repo@main"]),
                ("--request-id", &["r1"]),
                ("--output", &["yaml"]),
            ]),
        ] {
            assert!(bad.is_err());
        }
    }

    #[test]
    fn dispatch_body_with_schema_coerces_by_kind_and_enforces_target_bound() {
        let max_ok = format!("{}@{}", "a".repeat(256), "b".repeat(1023));
        assert_eq!(max_ok.len(), 1280);
        assert!(parse_dispatch_target(&max_ok).is_ok());

        let max_err = format!("{}@{}", "a".repeat(256), "b".repeat(1024));
        assert_eq!(max_err.len(), 1281);
        assert!(parse_dispatch_target(&max_err).is_err());

        let list_payload = json!({
            "schema": crate::runbooks::RUNBOOKS_VIEW_SCHEMA,
            "configured": true,
            "principal": "local:operator",
            "runbooks": [{
                "id": "edge-test",
                "params": [
                    { "name": "path", "kind": "relative_path" },
                    { "name": "mode_num", "kind": "enum" },
                    { "name": "flag", "kind": "bool" },
                    { "name": "limit", "kind": "integer" }
                ]
            }]
        });
        let kinds = extract_runbook_param_kinds(&list_payload, "edge-test")
            .unwrap_or_else(|| unreachable!("expected schema kinds"));
        assert!(extract_runbook_param_kinds(&list_payload, "missing").is_none());

        let options: BTreeMap<&str, Vec<String>> = [
            ("--runbook", vec!["edge-test".into()]),
            ("--target", vec!["onixresearch/kiln@refs/heads/main".into()]),
            ("--request-id", vec!["req-schema".into()]),
            (
                "--param",
                vec![
                    "path=123".into(),
                    "mode_num=true".into(),
                    "flag=true".into(),
                    "limit=-5".into(),
                    "unknown=42".into(),
                ],
            ),
        ]
        .into_iter()
        .collect();

        let (_, _, body) = dispatch_body_with_schema(&options, Some(&kinds))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            body,
            json!({
                "request_id": "req-schema",
                "target": "onixresearch/kiln@refs/heads/main",
                "params": {
                    "path": "123",
                    "mode_num": "true",
                    "flag": true,
                    "limit": -5,
                    "unknown": 42
                }
            })
        );
        assert_eq!(
            parse_param_value_for_kind("01", Some("integer")),
            json!("01")
        );
        assert_eq!(
            parse_param_value_for_kind("123", Some("bool")),
            json!("123")
        );
        assert_eq!(
            parse_param_value_for_kind("false", Some("relative_path")),
            json!("false")
        );
        assert_eq!(
            parse_param_value_for_kind("123", Some("enum")),
            json!("123")
        );
    }
}
