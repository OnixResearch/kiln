//! Disposable loopback consumer. Payloads exist only in an owned TempDir;
//! sessions, connections, time, output and export size have finite bounds.
use super::{
    PreviewRequest, admit,
    engine::EngineBundle,
    nix::{NativeNix, verify_nar},
};
use kiln_core::preview::{PreviewBinding, PreviewDescriptor};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_HTTP_REQUEST: usize = 8192;
const MAX_HTTP_CONNECTIONS: usize = 2048;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

pub fn validate_catalog_path(path: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| "preview_catalog_unavailable")?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 65536
        || metadata.uid() != ::nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o022 != 0
    {
        return Err("preview_catalog_not_owner_controlled".into());
    }
    path.canonicalize()
        .map_err(|_| "preview_catalog_unavailable".into())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewPreview {
    pub descriptor_identity: String,
    pub binding: PreviewBinding,
    /// Canonical full CI run whose exact source this build run was derived from.
    pub review_run_id: String,
    pub engine_identity: String,
    pub executable: String,
    pub expires_at: u64,
    pub launch_url: String,
}

/// A projection only: exact canonical successful clean/full CI facts must be
/// supplied by the authenticated review handler, never by the browser/catalog.
/// The browser receives no build inputs, Nix commands, local paths or secrets.
pub fn review_entries(
    catalog: &Path,
    project: &str,
    repository: &str,
    revision: &str,
    successful_runs: &[String],
    canonical_builds: &std::collections::BTreeMap<String, crate::engine::PreviewBuildEvidence>,
    now: u64,
) -> Result<Vec<ReviewPreview>, String> {
    let catalog = validate_catalog_path(catalog)?;
    let rows: Vec<ReviewPreview> = super::read_json(&catalog, 65536)?;
    if rows.len() > 16 {
        return Err("preview_catalog_exhausted".into());
    }
    Ok(rows
        .into_iter()
        .filter_map(|mut preview| {
            if preview.binding.project != project
                || preview.binding.repository != repository
                || preview.binding.revision != revision
                || preview.expires_at <= now
                || !valid_launch_url_for(&preview.launch_url, &preview.descriptor_identity)
                || ![
                    preview.descriptor_identity.as_str(),
                    preview.engine_identity.as_str(),
                ]
                .into_iter()
                .all(|identity| {
                    identity.len() == 64 && identity.bytes().all(|b| b.is_ascii_hexdigit())
                })
            {
                return None;
            }
            let build = canonical_builds.get(preview.binding.run_id.as_str())?;
            if !successful_runs
                .iter()
                .any(|run_id| run_id == &build.source_run_id)
                || !canonical_build_matches(&preview.binding, &preview.executable, build)
            {
                return None;
            }
            preview.review_run_id.clone_from(&build.source_run_id);
            Some(preview)
        })
        .collect())
}

pub(crate) fn canonical_build_matches(
    binding: &PreviewBinding,
    executable: &str,
    build: &crate::engine::PreviewBuildEvidence,
) -> bool {
    let Some(observation) = build.observation.as_ref() else {
        return false;
    };
    let Some(product) = observation
        .products
        .iter()
        .find(|product| product.name == binding.product)
    else {
        return false;
    };
    let Some(root) = build.product_paths.get(binding.product.as_str()) else {
        return false;
    };
    let executable_is_product = executable
        .strip_prefix(root.as_str())
        .is_some_and(|suffix| suffix.starts_with("/bin/"));
    !build.node_id.as_str().is_empty()
        && build.plan.candidate_id == binding.candidate_id
        && build.plan.attempt_id == binding.attempt_id
        && build.plan.required_products.contains(&binding.product)
        && observation.outcome == kiln_core::build::BuildEvaluationOutcome::Success
        && observation.receipt_identity.as_ref() == Some(&binding.build_receipt)
        && product.artifact_identity == binding.artifact_identity
        && product.store_identity == binding.store_identity
        && product
            .store
            .as_ref()
            .is_some_and(|store| store.references_complete)
        && executable_is_product
}
fn valid_launch_url_for(url: &str, identity: &str) -> bool {
    let Some(tail) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, path)) = tail.split_once('/') else {
        return false;
    };
    let Some((token, descriptor)) = path
        .strip_suffix("/index.html")
        .and_then(|path| path.split_once('/'))
    else {
        return false;
    };
    port.parse::<u16>().is_ok_and(|port| port > 0)
        && token.len() == 64
        && token.bytes().all(|byte| byte.is_ascii_hexdigit())
        && descriptor == identity
        && descriptor.len() == 64
        && descriptor.bytes().all(|byte| byte.is_ascii_hexdigit())
}
pub fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(untagged)]
enum Payload {
    Admitted(Box<PreviewDescriptor>),
    Cohort(Box<super::cohort::EngineCohort>),
}
impl Payload {
    fn expires_at(&self) -> u64 {
        match self {
            Self::Admitted(d) => d.limits.expires_at,
            Self::Cohort(d) => d.limits.expires_at,
        }
    }
    fn closure(&self) -> &[kiln_core::preview::RuntimeMember] {
        match self {
            Self::Admitted(d) => &d.closure,
            Self::Cohort(d) => &d.closure,
        }
    }
}

pub struct PreviewServer {
    listener: TcpListener,
    engine: EngineBundle,
    payload: Payload,
    identity: String,
    token: String,
    export: tempfile::TempDir,
    nars: Vec<PathBuf>,
    started: Instant,
    seconds: u64,
    browser_starts: std::cell::Cell<usize>,
    /// Operator-owned record store; absent means reports are refused.
    observations: Option<PathBuf>,
}

impl std::fmt::Debug for PreviewServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreviewServer")
            .field("identity", &self.identity)
            .field("engine_identity", &self.engine.identity)
            .field("seconds", &self.seconds)
            .field("browser_starts", &self.browser_starts.get())
            .finish_non_exhaustive()
    }
}
impl PreviewServer {
    /// Export only after exact admission and complete native verification. The
    /// scratch parent must be operator-owned (recommended /run/user/<uid>).
    pub(crate) fn prepare(
        request: PreviewRequest,
        current: &PreviewBinding,
        policy: &super::PreviewSelection,
        engine: EngineBundle,
        nix: &NativeNix,
        scratch: &Path,
    ) -> Result<Self, String> {
        let at = now()?;
        super::enforce_policy(&request.descriptor, policy)?;
        let identity = admit(&request, current, &engine, at)?;
        let export = super::retention::create(
            scratch,
            "admitted-runtime",
            request.descriptor.limits.expires_at,
        )?;
        let nars = nix.export(&request.descriptor, export.path())?;
        kiln_core::preview::validate_descriptor(&request.descriptor, now()?)
            .map_err(|e| format!("{}: {}", e.code, e.message))?;
        Self::from_payload(
            Payload::Admitted(Box::new(request.descriptor)),
            engine,
            identity,
            export,
            nars,
            at,
        )
    }
    pub fn prepare_engine_cohort(
        selection: super::PreviewSelection,
        engine: EngineBundle,
        nix: &NativeNix,
        scratch: &Path,
    ) -> Result<Self, String> {
        let at = now()?;
        let cohort = super::cohort::EngineCohort::observe(selection, &engine, nix, scratch, at)?;
        let identity = cohort.identity()?;
        let export =
            super::retention::create(scratch, "engine-qualification", cohort.limits.expires_at)?;
        let nars = nix.export_members(
            &cohort.closure,
            &cohort.public_keys,
            export.path(),
            cohort.limits.expires_at,
        )?;
        Self::from_payload(
            Payload::Cohort(Box::new(cohort)),
            engine,
            identity,
            export,
            nars,
            at,
        )
    }
    fn from_payload(
        payload: Payload,
        engine: EngineBundle,
        identity: String,
        export: tempfile::TempDir,
        nars: Vec<PathBuf>,
        at: u64,
    ) -> Result<Self, String> {
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|_| "preview_entropy_unavailable")?;
        let token = random.iter().map(|b| format!("{b:02x}")).collect();
        let listener =
            TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let seconds = payload.expires_at().saturating_sub(at);
        Ok(Self {
            listener,
            engine,
            payload,
            identity,
            token,
            export,
            nars,
            started: Instant::now(),
            seconds,
            browser_starts: std::cell::Cell::new(0),
            observations: None,
        })
    }
    /// Retain browser reports for this CI-bound descriptor in an operator-owned directory.
    pub fn with_observations(mut self, directory: &Path) -> Result<Self, String> {
        if !matches!(self.payload, Payload::Admitted(_)) {
            return Err(
                "engine qualification has no canonical CI binding and cannot retain review observations"
                    .into(),
            );
        }
        self.observations = Some(super::observation::validate_directory(directory)?);
        Ok(self)
    }
    pub fn entry(&self) -> Result<ReviewPreview, String> {
        let Payload::Admitted(descriptor) = &self.payload else {
            return Err("engine qualification has no canonical CI binding and cannot enter a review catalog".into());
        };
        Ok(ReviewPreview {
            descriptor_identity: self.identity.clone(),
            binding: descriptor.binding.clone(),
            review_run_id: String::new(),
            engine_identity: self.engine.identity.clone(),
            executable: descriptor.executable.clone(),
            expires_at: descriptor.limits.expires_at,
            launch_url: self.launch_url()?,
        })
    }
    pub fn launch_url(&self) -> Result<String, String> {
        let port = self
            .listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();
        Ok(format!(
            "http://127.0.0.1:{port}/{}/{}/index.html",
            self.token, self.identity
        ))
    }
    pub fn serve(self) -> Result<(), String> {
        self.serve_until(&std::sync::atomic::AtomicBool::new(false))
    }
    pub fn serve_until(self, stop: &std::sync::atomic::AtomicBool) -> Result<(), String> {
        let mut connections = 0;
        while !stop.load(std::sync::atomic::Ordering::Relaxed)
            && self.started.elapsed().as_secs() < self.seconds
            && now()? < self.payload.expires_at()
            && connections < MAX_HTTP_CONNECTIONS
        {
            match self.listener.accept() {
                Ok((mut stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    connections += 1;
                    stream
                        .set_read_timeout(Some(IO_TIMEOUT))
                        .map_err(|e| e.to_string())?;
                    stream
                        .set_write_timeout(Some(IO_TIMEOUT))
                        .map_err(|e| e.to_string())?;
                    // A broken browser request remains preview-only; the next
                    // bounded request can still inspect the exact failure.
                    if let Err(error) = self.handle(&mut stream) {
                        let bounded = error.as_bytes();
                        let _response = response(
                            &mut stream,
                            422,
                            "text/plain",
                            bounded.get(..512).unwrap_or(bounded),
                        );
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        // Owned export is removed on every success/error/drop path. No shared
        // store GC, source tree, browser profile or CI evidence is touched.
        self.export.close().map_err(|e| e.to_string())
    }
    fn handle(&self, stream: &mut TcpStream) -> Result<(), String> {
        let mut bytes = Vec::with_capacity(1024);
        let mut buffer = [0u8; 1024];
        let header_end = loop {
            let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("preview_http_closed".into());
            }
            bytes.extend_from_slice(buffer.get(..n).ok_or("preview_http_invalid")?);
            if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                break end;
            }
            if bytes.len() > MAX_HTTP_REQUEST {
                return Err("preview_http_exhausted".into());
            }
        };
        if header_end > MAX_HTTP_REQUEST {
            return Err("preview_http_exhausted".into());
        }
        let (head, rest) = bytes.split_at(header_end);
        let body_prefix = rest.get(4..).unwrap_or_default();
        let request = std::str::from_utf8(head).map_err(|_| "preview_http_invalid")?;
        let mut lines = request.split("\r\n");
        let first = lines.next().ok_or("preview_http_invalid")?;
        let mut words = first.split(' ');
        let method = words.next().ok_or("preview_http_invalid")?;
        if method != "GET" && method != "POST" {
            return response(stream, 405, "text/plain", b"preview is read only");
        }
        let target = words.next().ok_or("preview_http_invalid")?;
        let mut origin = None;
        let mut content_type = None;
        let mut content_length = None;
        let host = format!(
            "127.0.0.1:{}",
            self.listener
                .local_addr()
                .map_err(|e| e.to_string())?
                .port()
        );
        let mut host_ok = false;
        for line in lines {
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("host") {
                    host_ok = value.trim() == host;
                }
                if key.eq_ignore_ascii_case("origin") {
                    if value.trim() != format!("http://{host}") {
                        return response(stream, 403, "text/plain", b"preview origin mismatch");
                    }
                    origin = Some(value.trim());
                }
                if key.eq_ignore_ascii_case("content-type") {
                    content_type = Some(value.trim());
                }
                if key.eq_ignore_ascii_case("content-length") {
                    content_length = Some(value.trim());
                }
                if key.eq_ignore_ascii_case("cookie") {
                    return response(
                        stream,
                        403,
                        "text/plain",
                        b"preview must not receive credentials",
                    );
                }
            }
        }
        if !host_ok {
            return response(stream, 403, "text/plain", b"preview host mismatch");
        }
        if now()? >= self.payload.expires_at() {
            return response(stream, 410, "text/plain", b"preview expired");
        }
        let prefix = format!("/{}/{}/", self.token, self.identity);
        let Some(path) = target.strip_prefix(&prefix) else {
            return response(stream, 404, "text/plain", b"preview not found");
        };
        if method == "POST" {
            return self.retain_observation(
                stream,
                path,
                ObservationRequest {
                    origin,
                    content_type,
                    content_length,
                    body_prefix,
                },
            );
        }
        let (mime, data) = match path {
            "index.html" => ("text/html", include_bytes!("../preview.html").to_vec()),
            "engine/frame.html" => {
                const MAX_BROWSER_STARTS: usize = 2;
                if self.browser_starts.get() >= MAX_BROWSER_STARTS {
                    return response(
                        stream,
                        429,
                        "text/plain",
                        b"preview browser-session budget exhausted",
                    );
                }
                self.browser_starts.set(self.browser_starts.get() + 1);
                (
                    "text/html",
                    include_bytes!("../preview-frame.html").to_vec(),
                )
            }
            "preview-runtime.js" => (
                "text/javascript",
                include_bytes!("../preview-runtime.js").to_vec(),
            ),
            "preview-shell.js" => (
                "text/javascript",
                include_bytes!("../preview-shell.js").to_vec(),
            ),
            "preview.css" => ("text/css", include_bytes!("../preview.css").to_vec()),
            "descriptor.json" => {
                let manifest = serde_json::json!({"identity":self.identity,"descriptor":self.payload,"engine_manifest":self.engine.manifest,"guest_files":self.engine.guest_files()});
                (
                    "application/json",
                    serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
                )
            }
            // The pinned terminal resolves its WASM relative to the frame,
            // while the immutable engine manifest stores it under vendor/.
            "engine/ghostty-vt.wasm" => (
                "application/wasm",
                self.engine.read("vendor/ghostty-vt.wasm")?,
            ),
            p if p.starts_with("engine/") => {
                let name = &p[7..];
                (mime_type(name), self.engine.read(name)?)
            }
            p if p.starts_with("nar/") => {
                let index = p[4..]
                    .parse::<usize>()
                    .map_err(|_| "preview_member_unavailable")?;
                let file = self.nars.get(index).ok_or("preview_member_unavailable")?;
                let member = self
                    .payload
                    .closure()
                    .get(index)
                    .ok_or("preview_member_unavailable")?;
                verify_nar(file, member)?;
                (
                    "application/octet-stream",
                    fs::read(file).map_err(|_| "preview_member_unavailable")?,
                )
            }
            _ => return response(stream, 404, "text/plain", b"preview member unavailable"),
        };
        response(stream, 200, mime, &data)
    }
    /// Retain one bounded browser report composed against this server's admitted descriptor.
    fn retain_observation(
        &self,
        stream: &mut TcpStream,
        path: &str,
        request: ObservationRequest<'_>,
    ) -> Result<(), String> {
        let (Payload::Admitted(descriptor), Some(directory)) = (&self.payload, &self.observations)
        else {
            return response(
                stream,
                409,
                "text/plain",
                b"observations are not retained for this preview",
            );
        };
        let host = self.listener.local_addr().map_err(|e| e.to_string())?;
        let length = match admit_observation_request(path, &format!("http://{host}"), &request) {
            Ok(length) => length,
            Err((status, message)) => {
                return response(stream, status, "text/plain", message.as_bytes());
            }
        };
        let mut body = request.body_prefix.to_vec();
        let mut buffer = [0u8; 4096];
        while body.len() < length {
            let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("preview_http_closed".into());
            }
            body.extend_from_slice(buffer.get(..n).ok_or("preview_http_invalid")?);
        }
        if body.len() != length {
            return response(stream, 400, "text/plain", b"observation length mismatch");
        }
        let record = match super::observation::compose(&body, &self.identity, descriptor, now()?) {
            Ok(record) => record,
            Err(error) => return response(stream, 422, "text/plain", error.as_bytes()),
        };
        match super::observation::retain(directory, &record) {
            Ok(()) => {
                let document =
                    serde_json::to_vec(&serde_json::json!({"identity": record.identity}))
                        .map_err(|e| e.to_string())?;
                response(stream, 201, "application/json", &document)
            }
            Err(error) if error == "preview_observations_full" => {
                response(stream, 507, "text/plain", error.as_bytes())
            }
            Err(error) => response(stream, 503, "text/plain", error.as_bytes()),
        }
    }
}

/// Headers of one report submission; every field is checked before the body is read.
struct ObservationRequest<'a> {
    origin: Option<&'a str>,
    content_type: Option<&'a str>,
    content_length: Option<&'a str>,
    body_prefix: &'a [u8],
}

/// Pure admission of a report submission; returns the declared body length.
fn admit_observation_request(
    path: &str,
    own_origin: &str,
    request: &ObservationRequest<'_>,
) -> Result<usize, (u16, &'static str)> {
    if path != "observation" {
        return Err((404, "preview not found"));
    }
    if request.origin != Some(own_origin) {
        return Err((403, "observation requires the preview origin"));
    }
    if request
        .content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err((415, "observation must be application/json"));
    }
    let length = request
        .content_length
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or((411, "observation length required"))?;
    if length > super::observation::MAX_REPORT_BYTES || request.body_prefix.len() > length {
        return Err((413, "observation exceeds its bound"));
    }
    Ok(length)
}
fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "text/javascript"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}
fn response(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        _ => "Preview unavailable",
    };
    let charset = if mime.starts_with("text/") || mime == "application/json" {
        "; charset=utf-8"
    } else {
        ""
    };
    // No cache/CORS/cookies, no service worker or console credentials. This
    // separate loopback origin is not the authenticated web-service origin.
    write!(stream,"HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}{charset}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; connect-src 'self'; frame-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'self'\r\n\r\n",body.len()).map_err(|e|e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn canonical_builds() -> std::collections::BTreeMap<String, crate::engine::PreviewBuildEvidence>
    {
        use kiln_core::build::{
            BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
            BuildProductObservation, BuildStoreObjectObservation,
        };
        use kiln_core::types::{
            Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId,
            ProviderProfileId,
        };
        let id = |byte: &str| {
            ContentId::from_blake3_hex(byte.repeat(64)).unwrap_or_else(|_| unreachable!())
        };
        let plan = BuildEffectPlan {
            effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
            idempotency_key: id("1"),
            candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
            pipeline_revision: PipelineRevisionId::new("pipeline")
                .unwrap_or_else(|_| unreachable!()),
            plan_identity: id("2"),
            policy_identity: id("3"),
            provider_profile: ProviderProfileId::new("nix").unwrap_or_else(|_| unreachable!()),
            platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
            required_products: vec![Alias::new("out").unwrap_or_else(|_| unreachable!())],
            constraints: None,
        };
        let product = BuildProductObservation {
            name: Alias::new("out").unwrap_or_else(|_| unreachable!()),
            artifact_identity: id("a"),
            store_identity: id("a"),
            store: Some(BuildStoreObjectObservation {
                nar_sha256: "b".repeat(64),
                nar_size: 16,
                references: Vec::new(),
                references_complete: true,
                deriver: None,
                content_address: None,
                signatures: Vec::new(),
            }),
        };
        let observation = BuildEvaluationObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision: plan.pipeline_revision.clone(),
            plan_identity: plan.plan_identity.clone(),
            policy_identity: plan.policy_identity.clone(),
            provider_profile: plan.provider_profile.clone(),
            platform: plan.platform.clone(),
            outcome: BuildEvaluationOutcome::Success,
            products: vec![product],
            constraints: None,
            inputs: Vec::new(),
            input_closure_identity: None,
            input_closure_complete: true,
            derivation_identity: None,
            builder_identity: id("4"),
            worker_identity: id("5"),
            store_identity: id("6"),
            cache_kind: BuildCacheKind::Local,
            cache_source_identity: None,
            log_identities: Vec::new(),
            metrics: Vec::new(),
            rebuild: Vec::new(),
            receipt_identity: Some(id("a")),
        };
        [(
            "run".to_owned(),
            crate::engine::PreviewBuildEvidence {
                source_run_id: "source-run".into(),
                request_id: "request".into(),
                profile_identity: id("7"),
                node_id: Alias::new("build").unwrap_or_else(|_| unreachable!()),
                plan,
                product_paths: [(
                    "out".to_owned(),
                    "/nix/store/00000000000000000000000000000000-cli".to_owned(),
                )]
                .into(),
                observation: Some(observation),
            },
        )]
        .into()
    }
    #[test]
    fn catalog_launch_urls_are_loopback_capabilities_not_external_execution() {
        let identity = "a".repeat(64);
        let token = "b".repeat(64);
        let launch = format!("http://127.0.0.1:1234/{token}/{identity}/index.html");
        assert!(valid_launch_url_for(&launch, &identity));
        assert!(!valid_launch_url_for(&launch, &token));
        assert!(!valid_launch_url_for(
            "https://trynix.dev/?pkg=hello",
            &identity
        ));
        assert!(!valid_launch_url_for(
            "http://127.0.0.1:1234/short/index.html",
            &identity
        ));
        assert!(!valid_launch_url_for(
            &format!("{launch}?revision=new"),
            &identity
        ));
    }
    #[test]
    fn catalog_requires_exact_canonical_run_source_and_unexpired_entry()
    -> Result<(), Box<dyn std::error::Error>> {
        use kiln_core::{Alias, AttemptId, CandidateId, ContentId, RunId};
        let content = ContentId::from_blake3_hex("a".repeat(64)).map_err(|e| e.message)?;
        let row = ReviewPreview {
            descriptor_identity: "a".repeat(64),
            binding: PreviewBinding {
                project: "project".into(),
                repository: "repository".into(),
                revision: "c".repeat(40),
                run_id: RunId::new("run").map_err(|e| e.message)?,
                attempt_id: AttemptId::new("attempt").map_err(|e| e.message)?,
                candidate_id: CandidateId::new("candidate").map_err(|e| e.message)?,
                build_receipt: content.clone(),
                product: Alias::new("out").map_err(|e| e.message)?,
                artifact_identity: content.clone(),
                store_identity: content,
            },
            review_run_id: "forged-source-run".into(),
            engine_identity: "b".repeat(64),
            executable: "/nix/store/00000000000000000000000000000000-cli/bin/cli".into(),
            expires_at: 200,
            launch_url: format!(
                "http://127.0.0.1:1234/{}/{}/index.html",
                "b".repeat(64),
                "a".repeat(64)
            ),
        };
        let file = tempfile::NamedTempFile::new()?;
        let encoded = serde_json::to_vec(&vec![row.clone()])?;
        fs::write(file.path(), &encoded)?;
        let runs = vec![String::from("source-run")];
        let builds = canonical_builds();
        let build = builds.get("run").unwrap_or_else(|| unreachable!());
        let matches =
            |row: &ReviewPreview| canonical_build_matches(&row.binding, &row.executable, build);
        assert!(matches(&row));
        let mut mismatched = row.clone();
        mismatched.binding.candidate_id =
            CandidateId::new("other-candidate").map_err(|e| e.message)?;
        assert!(!matches(&mismatched));
        mismatched = row.clone();
        mismatched.binding.build_receipt =
            ContentId::from_blake3_hex("c".repeat(64)).map_err(|e| e.message)?;
        assert!(!matches(&mismatched));
        mismatched = row.clone();
        mismatched.executable = "/nix/store/unrelated/bin/cli".into();
        assert!(!matches(&mismatched));
        let entries = review_entries(
            file.path(),
            "project",
            "repository",
            &"c".repeat(40),
            &runs,
            &builds,
            101,
        )?;
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries
                .first()
                .unwrap_or_else(|| unreachable!("one review entry"))
                .review_run_id,
            "source-run"
        );
        assert!(
            review_entries(
                file.path(),
                "project",
                "repository",
                &"c".repeat(40),
                &[String::from("stale-source-run")],
                &builds,
                101,
            )?
            .is_empty()
        );
        assert!(
            review_entries(
                file.path(),
                "project",
                "repository",
                &"d".repeat(40),
                &runs,
                &builds,
                101
            )?
            .is_empty()
        );
        assert!(
            review_entries(
                file.path(),
                "project",
                "repository",
                &"c".repeat(40),
                &[],
                &builds,
                101
            )?
            .is_empty()
        );
        assert!(
            review_entries(
                file.path(),
                "project",
                "repository",
                &"c".repeat(40),
                &runs,
                &builds,
                200
            )?
            .is_empty()
        );
        assert_eq!(fs::read(file.path())?, encoded);
        Ok(())
    }
    #[test]
    fn catalog_rejects_writable_or_symlink_inputs() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.json");
        fs::write(&path, b"[]")?;
        assert!(validate_catalog_path(&path).is_ok());
        let link = directory.path().join("catalog-link.json");
        symlink(&path, &link)?;
        assert!(validate_catalog_path(&link).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666))?;
        assert!(validate_catalog_path(&path).is_err());
        Ok(())
    }
    // r[verify kiln.preview.comparison]
    #[test]
    fn observation_submissions_require_own_origin_json_and_bounded_length() {
        const ORIGIN: &str = "http://127.0.0.1:4000";
        fn request(
            origin: Option<&'static str>,
            content_type: Option<&'static str>,
            content_length: Option<&'static str>,
        ) -> ObservationRequest<'static> {
            ObservationRequest {
                origin,
                content_type,
                content_length,
                body_prefix: b"{}",
            }
        }
        let json = Some("application/json");
        assert_eq!(
            admit_observation_request(
                "observation",
                ORIGIN,
                &request(Some(ORIGIN), json, Some("2"))
            ),
            Ok(2)
        );
        assert_eq!(
            admit_observation_request(
                "observation",
                ORIGIN,
                &request(
                    Some(ORIGIN),
                    Some("application/json; charset=utf-8"),
                    Some("64")
                )
            ),
            Ok(64)
        );
        for (path, submitted, expected) in [
            ("index.html", request(Some(ORIGIN), json, Some("2")), 404),
            ("observation", request(None, json, Some("2")), 403),
            (
                "observation",
                request(Some("http://127.0.0.1:4001"), json, Some("2")),
                403,
            ),
            (
                "observation",
                request(Some(ORIGIN), Some("text/plain"), Some("2")),
                415,
            ),
            ("observation", request(Some(ORIGIN), json, None), 411),
            ("observation", request(Some(ORIGIN), json, Some("-1")), 411),
            (
                "observation",
                request(Some(ORIGIN), json, Some("99999999")),
                413,
            ),
            ("observation", request(Some(ORIGIN), json, Some("1")), 413),
        ] {
            assert_eq!(
                admit_observation_request(path, ORIGIN, &submitted).map_err(|(status, _)| status),
                Err(expected),
                "{path}"
            );
        }
    }
}
