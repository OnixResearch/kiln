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
    now: u64,
) -> Result<Vec<ReviewPreview>, String> {
    let catalog = validate_catalog_path(catalog)?;
    let rows: Vec<ReviewPreview> = super::read_json(&catalog, 65536)?;
    if rows.len() > 16 {
        return Err("preview_catalog_exhausted".into());
    }
    Ok(rows
        .into_iter()
        .filter(|p| {
            p.binding.project == project
                && p.binding.repository == repository
                && p.binding.revision == revision
                && successful_runs
                    .iter()
                    .any(|r| r == p.binding.run_id.as_str())
                && p.expires_at > now
                && valid_launch_url(&p.launch_url)
                && [p.descriptor_identity.as_str(), p.engine_identity.as_str()]
                    .into_iter()
                    .all(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        })
        .collect())
}
fn valid_launch_url(url: &str) -> bool {
    let Some(tail) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, path)) = tail.split_once('/') else {
        return false;
    };
    let Some(token) = path.strip_suffix("/index.html") else {
        return false;
    };
    port.parse::<u16>().is_ok_and(|p| p > 0)
        && token.len() == 64
        && token.bytes().all(|b| b.is_ascii_hexdigit())
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
    pub fn prepare(
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
        })
    }
    pub fn entry(&self) -> Result<ReviewPreview, String> {
        let Payload::Admitted(descriptor) = &self.payload else {
            return Err("engine qualification has no canonical CI binding and cannot enter a review catalog".into());
        };
        Ok(ReviewPreview {
            descriptor_identity: self.identity.clone(),
            binding: descriptor.binding.clone(),
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
        Ok(format!("http://127.0.0.1:{port}/{}/index.html", self.token))
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
        loop {
            let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("preview_http_closed".into());
            }
            bytes.extend_from_slice(buffer.get(..n).ok_or("preview_http_invalid")?);
            if bytes.len() > MAX_HTTP_REQUEST {
                return Err("preview_http_exhausted".into());
            }
            if bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                break;
            }
        }
        let request = std::str::from_utf8(&bytes).map_err(|_| "preview_http_invalid")?;
        let mut lines = request.split("\r\n");
        let first = lines.next().ok_or("preview_http_invalid")?;
        let mut words = first.split(' ');
        if words.next() != Some("GET") {
            return response(stream, 405, "text/plain", b"preview is read only");
        }
        let target = words.next().ok_or("preview_http_invalid")?;
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
                if key.eq_ignore_ascii_case("origin") && value.trim() != format!("http://{host}") {
                    return response(stream, 403, "text/plain", b"preview origin mismatch");
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
        let prefix = format!("/{}/", self.token);
        let Some(path) = target.strip_prefix(&prefix) else {
            return response(stream, 404, "text/plain", b"preview not found");
        };
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
    let reason = if status == 200 {
        "OK"
    } else {
        "Preview unavailable"
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
    #[test]
    fn catalog_launch_urls_are_loopback_capabilities_not_external_execution() {
        assert!(valid_launch_url(&format!(
            "http://127.0.0.1:1234/{}/index.html",
            "a".repeat(64)
        )));
        assert!(!valid_launch_url("https://trynix.dev/?pkg=hello"));
        assert!(!valid_launch_url("http://127.0.0.1:1234/short/index.html"));
        assert!(!valid_launch_url(&format!(
            "http://127.0.0.1:1234/{}/index.html?revision=new",
            "a".repeat(64)
        )));
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
            engine_identity: "b".repeat(64),
            executable: "/nix/store/00000000000000000000000000000000-cli/bin/cli".into(),
            expires_at: 200,
            launch_url: format!("http://127.0.0.1:1234/{}/index.html", "a".repeat(64)),
        };
        let file = tempfile::NamedTempFile::new()?;
        let encoded = serde_json::to_vec(&vec![row])?;
        fs::write(file.path(), &encoded)?;
        let runs = vec![String::from("run")];
        assert_eq!(
            review_entries(
                file.path(),
                "project",
                "repository",
                &"c".repeat(40),
                &runs,
                101
            )?
            .len(),
            1
        );
        assert!(
            review_entries(
                file.path(),
                "project",
                "repository",
                &"d".repeat(40),
                &runs,
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
}
