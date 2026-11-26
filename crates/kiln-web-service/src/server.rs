//! Bounded HTTPS shell. Authorization precedes every application call.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once};
use std::time::{Duration, Instant};

use rustls::{ServerConfig, ServerConnection, StreamOwned};
use rustls_pemfile::{certs, private_key};

use crate::access::{AccessProfile, IdentityResolver, Principal};
use crate::auth::Credentials;
use crate::engine::{ApiResponse, Engine};
use crate::profile::ServiceProfile;
use zeroize::Zeroizing;

/// Explicit runtime policies, never part of the canonical ServiceProfile identity.
#[derive(Debug, Default)]
pub struct Options {
    pub access: Option<PathBuf>,
    pub automation: Option<PathBuf>,
    pub cobs: Option<PathBuf>,
    pub sources: Option<PathBuf>,
    pub reviews: Option<PathBuf>,
    pub cache: Option<PathBuf>,
    pub workers: Option<PathBuf>,
    pub work: Option<PathBuf>,
    pub code_review: Option<PathBuf>,
    pub inventory: Option<PathBuf>,
    pub implementation_work: Option<PathBuf>,
    pub artifact_previews: Option<PathBuf>,
    pub preview_observations: Option<PathBuf>,
    /// Campaign control profile; requires `implementation_work`.
    pub campaign_controls: Option<PathBuf>,
    /// Optional reviewed runbook registry and per-runbook grant policy.
    pub runbooks: Option<PathBuf>,
    /// Optional on-disk Lattice runbook bundle catalog directory for BLAKE3 pin verification.
    pub runbooks_dir: Option<PathBuf>,
    /// Optional owner profile for original Lattice recovery; requires `implementation_work`.
    pub dispatch_recovery: Option<PathBuf>,
    /// Optional immutable native inspection profile for one frozen Lattice attempt.
    pub native_runtime_evidence: Option<PathBuf>,
    /// Runtime-only loopback fleet coordinator endpoint.
    pub fleet: Option<PathBuf>,
    /// Explicit bounded export from the deployment owner.
    pub delivery_observations: Option<PathBuf>,
}

impl Options {
    fn access_profile(&self) -> Result<AccessProfile, String> {
        self.access
            .as_deref()
            .map(AccessProfile::load)
            .unwrap_or_else(|| Ok(AccessProfile::default()))
    }
    fn automation_profile(&self) -> Result<Option<crate::automation::AutomationProfile>, String> {
        self.automation
            .as_deref()
            .map(crate::automation::AutomationProfile::load)
            .transpose()
    }

    fn observation_inputs(&self) -> Result<(), String> {
        if let Some(path) = &self.inventory {
            let input = crate::inventory::InventoryInput::load(path)?;
            kiln_core::inventory::admit_snapshot(&input.profile, &input.snapshot)
                .map_err(str::to_owned)?;
        }
        if let Some(path) = &self.implementation_work {
            kiln_implementer::observation::ImplementationObservation::open(path)
                .map_err(|_| "implementation work observation unavailable".to_owned())?;
        }
        if let Some(path) = &self.fleet {
            crate::fleet::FleetProfile::load(path)?;
        }
        if self
            .delivery_observations
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err("delivery observation path must be absolute".into());
        }
        Ok(())
    }

    fn campaign_controls(&self) -> Result<Option<crate::campaign_controls::Controls>, String> {
        self.campaign_controls
            .as_deref()
            .map(|path| {
                crate::campaign_controls::Controls::new(path, self.implementation_work.as_deref())
            })
            .transpose()
    }
    fn recovery_config(&self) -> Result<Option<crate::dispatch_recovery::RecoveryConfig>, String> {
        if self.dispatch_recovery.is_some() && self.implementation_work.is_none() {
            return Err("dispatch recovery requires implementation work".to_owned());
        }
        self.dispatch_recovery
            .as_deref()
            .map(crate::dispatch_recovery::RecoveryConfig::load)
            .transpose()
    }
    fn native_runtime_config(
        &self,
    ) -> Result<Option<crate::runtime_evidence::RuntimeEvidenceProfile>, String> {
        let Some(path) = self.native_runtime_evidence.as_deref() else {
            return Ok(None);
        };
        let profile = crate::runtime_evidence::RuntimeEvidenceProfile::load(path)?;
        if self.implementation_work.as_deref() != Some(profile.implementation_database.as_path()) {
            return Err("native runtime evidence requires the same implementation database".to_owned());
        }
        Ok(Some(profile))
    }
}

#[path = "server/admission.rs"]
mod admission;
#[path = "server/inventory.rs"]
mod inventory_api;
#[path = "server/notify.rs"]
mod notify;

// r[impl kiln.web.deploy.runtime]
/// Admit startup inputs without creating state, binding listeners or contacting providers.
pub fn check(path: &Path) -> Result<(), String> {
    check_with_options(path, &Options::default())
}

pub fn check_with_options(path: &Path, options: &Options) -> Result<(), String> {
    let profile = ServiceProfile::load(path)?;
    options.observation_inputs()?;
    let _controls = options.campaign_controls()?;
    let _recovery = options.recovery_config()?;
    let _native_runtime = options.native_runtime_config()?;
    let access = options.access_profile()?;
    let access_agents = access.agent_bindings();
    let _credentials = Credentials::load(&profile, access)?;
    let _automation = options.automation_profile()?;
    if let Some(runbooks_path) = options.runbooks.as_deref() {
        let runbooks = crate::runbooks::RunbookRegistry::load(runbooks_path)?;
        runbooks.admit_for_service(&profile)?;
        if let Some(runbooks_dir) = options.runbooks_dir.as_deref() {
            crate::runbooks::bundle::verify_registry_bundles(runbooks_dir, &runbooks)?;
        }
    }
    let _cobs = options
        .cobs
        .as_deref()
        .map(crate::cobs::Profile::load)
        .transpose()?;
    let _sources = options
        .sources
        .as_deref()
        .map(crate::sources::Profile::load)
        .transpose()?;
    let _reviews = options
        .reviews
        .as_deref()
        .map(crate::reviews::Profile::load)
        .transpose()?;
    let _cache = options
        .cache
        .as_deref()
        .map(crate::cache::CacheProfile::load)
        .transpose()?;
    let _workers = options
        .workers
        .as_deref()
        .map(crate::engine::WorkerStatusProfile::load)
        .transpose()?;
    let _work = crate::work::Workspace::new(
        options
            .work
            .as_deref()
            .map(crate::work::Profile::load)
            .transpose()?,
    )?;
    let review_profile = load_review_profile(options, access_agents)?;
    let mut code_review = crate::code_review::Workspace::for_service(review_profile, &profile)?;
    code_review.configure_previews(options.artifact_previews.as_deref())?;
    code_review.configure_preview_observations(options.preview_observations.as_deref())?;
    if let Some(cache) = &_cache {
        cache.preflight_for(&profile)?;
    }
    if let Some(workers) = &_workers {
        workers.preflight_for(&profile)?;
    }
    let _tls = tls_config(&profile)?;
    admission::runtime_inputs(&profile)
}

const ACCEPT_INTERVAL: Duration = Duration::from_millis(25);
const FEED_REFRESH_TICK: Duration = Duration::from_secs(10);
const MAX_HEADER_FIELDS: usize = 64;
const SECURITY_HEADERS: &str = "X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; frame-ancestors 'none'; form-action 'none'\r\nStrict-Transport-Security: max-age=31536000\r\n";

pub fn run(path: &Path) -> Result<(), String> {
    run_with_options(path, &Options::default())
}

/// Load the optional review profile and merge access declarations exactly once
/// so `check` and `serve` admit the same runtime inputs.
fn load_review_profile(
    options: &Options,
    access_agents: std::collections::BTreeMap<String, crate::access::AgentDeclaration>,
) -> Result<Option<crate::code_review::Profile>, String> {
    let mut review_profile = options
        .code_review
        .as_deref()
        .map(crate::code_review::Profile::load)
        .transpose()?;
    if let Some(profile) = &mut review_profile {
        for (principal, declaration) in access_agents {
            profile.merge_agent(
                principal,
                kiln_core::change_review::AgentIdentity {
                    harness: declaration.harness,
                    model: declaration.model,
                },
            )?;
        }
    }
    Ok(review_profile)
}

pub fn run_with_options(path: &Path, options: &Options) -> Result<(), String> {
    let profile = ServiceProfile::load(path)?;
    options.observation_inputs()?;
    let campaign_controls = options.campaign_controls()?;
    let dispatch_recovery = options.recovery_config()?;
    let native_runtime = options.native_runtime_config()?;
    let access = options.access_profile()?;
    let access_identity = access.identity()?;
    let access_agents = access.agent_bindings();
    let credentials = Credentials::load(&profile, access)?;
    let automation = options.automation_profile()?;
    if let Some(runbooks_path) = options.runbooks.as_deref() {
        let runbooks = crate::runbooks::RunbookRegistry::load(runbooks_path)?;
        runbooks.admit_for_service(&profile)?;
        if let Some(runbooks_dir) = options.runbooks_dir.as_deref() {
            crate::runbooks::bundle::verify_registry_bundles(runbooks_dir, &runbooks)?;
        }
    }
    let cache = options
        .cache
        .as_deref()
        .map(crate::cache::CacheProfile::load)
        .transpose()?;
    let workers = options
        .workers
        .as_deref()
        .map(crate::engine::WorkerStatusProfile::load)
        .transpose()?;
    let cobs = crate::cobs::Browser::new(
        options
            .cobs
            .as_deref()
            .map(crate::cobs::Profile::load)
            .transpose()?,
    )?;
    let sources_profile = options
        .sources
        .as_deref()
        .map(crate::sources::Profile::load)
        .transpose()?;
    let sources = if profile.restricted_projection {
        crate::sources::Browser::restricted(sources_profile)?
    } else {
        crate::sources::Browser::new(sources_profile)?
    };
    let reviews = crate::reviews::Browser::new(
        options
            .reviews
            .as_deref()
            .map(crate::reviews::Profile::load)
            .transpose()?,
    )?;
    let work = crate::work::Workspace::new(
        options
            .work
            .as_deref()
            .map(crate::work::Profile::load)
            .transpose()?,
    )?;
    let review_profile = load_review_profile(options, access_agents)?;
    let mut code_review = crate::code_review::Workspace::for_service(review_profile, &profile)?;
    code_review.configure_previews(options.artifact_previews.as_deref())?;
    code_review.configure_preview_observations(options.preview_observations.as_deref())?;
    let tls = tls_config(&profile)?;
    admission::runtime_inputs(&profile)?;
    let listener = TcpListener::bind(profile.listen_address)
        .map_err(|_| "listener could not bind".to_owned())?;
    let mut engine = Engine::open(profile.clone())?;
    engine.configure_access_identity(&access_identity)?;
    if let Some(automation) = automation {
        engine.configure_automation(automation)?;
    }
    if let Some(runbooks_path) = options.runbooks.as_deref() {
        engine.configure_runbooks_path(runbooks_path)?;
    }
    if let Some(runbooks_dir) = options.runbooks_dir.as_deref() {
        engine.configure_runbooks_dir(runbooks_dir)?;
    }
    if let Some(cache) = cache {
        engine.install_cache(cache)?;
    }
    if let Some(workers) = workers {
        engine.configure_worker_status(workers)?;
    }
    let engine = Mutex::new(engine);
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&stop))
            .map_err(|_| "shutdown handler could not register".to_owned())?;
    }
    listener
        .set_nonblocking(true)
        .map_err(|_| "listener mode failed".to_owned())?;
    let active = AtomicUsize::new(0);
    let poll_failed = AtomicBool::new(false);
    let fleet_profile = options
        .fleet
        .as_deref()
        .map(crate::fleet::FleetProfile::load)
        .transpose()?;
    let fleet = fleet_profile
        .as_ref()
        .map(crate::fleet::Client::new)
        .transpose()?;
    let event_hub = EventHub::new();
    let context = Context {
        profile: &profile,
        credentials: &credentials,
        engine: &engine,
        cobs: &cobs,
        sources: &sources,
        reviews: &reviews,
        work: &work,
        code_review: &code_review,
        inventory: options.inventory.as_deref(),
        implementation_work: options.implementation_work.as_deref(),
        campaign_controls: campaign_controls.as_ref(),
        dispatch_recovery: dispatch_recovery.as_ref(),
        native_runtime_evidence: native_runtime.as_ref(),
        fleet: fleet.as_ref(),
        delivery_observations: options.delivery_observations.as_deref(),
        events: Some(&event_hub),
        stop: &stop,
    };
    // r[impl kiln.web.deploy.runtime] Readiness follows TLS, listener and canonical state initialization.
    notify::ready()?;
    std::thread::scope(|scope| -> Result<(), String> {
        let stop_ref = &stop;
        let failed_ref = &poll_failed;
        let engine_ref = &engine;
        let review_ref = &code_review;
        let fleet_ref = fleet.as_ref();
        let hub_ref = &event_hub;
        let impl_work_ref = options.implementation_work.as_deref();
        let polling = scope.spawn(move || {
            while !stop_ref.load(Ordering::Acquire) {
                let result = (|| -> Result<(), ()> {
                    let now = crate::engine::host_time_ms().map_err(|_| ())?;
                    let now = i64::try_from(now).map_err(|_| ())?;
                    let fleet_snapshot = fleet_ref.map(|client| match client.live_claims() {
                        Ok(claims) => crate::engine::FleetClaimsSnapshot::Available(claims),
                        Err(()) => crate::engine::FleetClaimsSnapshot::Unavailable,
                    });
                    let notifications = {
                        let mut engine = engine_ref.lock().map_err(|_| ())?;
                        if let Some(snapshot) = fleet_snapshot {
                            engine.set_fleet_claims_snapshot(snapshot);
                        }
                        let notifications = engine.poll().map_err(|_| ())?;
                        review_ref.advance_queue(&mut engine, now).map_err(|_| ())?;
                        notifications
                    };
                    hub_ref.wake_watcher();
                    review_ref.deliver_notifications(now).map_err(|_| ())?;
                    // One bounded serial executor: no HTTPS runs under Engine's
                    // mutex and no detached deliveries accumulate across polls.
                    for notification in notifications {
                        let observation = notification.deliver().map_err(|_| ())?;
                        engine_ref
                            .lock()
                            .map_err(|_| ())?
                            .record_notification(observation)
                            .map_err(|_| ())?;
                        hub_ref.wake_watcher();
                    }
                    Ok(())
                })();
                if result.is_err() {
                    failed_ref.store(true, Ordering::Release);
                    stop_ref.store(true, Ordering::Release);
                    hub_ref.wake_watcher();
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(profile.bounds.poll_interval_ms));
            }
        });
        let watcher_tick = Duration::from_millis(profile.bounds.poll_interval_ms.clamp(100, 1_000));
        let _watcher = scope.spawn(move || {
            run_coordinator_watcher(
                hub_ref,
                engine_ref,
                fleet_ref,
                impl_work_ref,
                stop_ref,
                watcher_tick,
            );
        });
        let feeds_policy = profile
            .feeds
            .as_ref()
            .filter(|feeds| feeds.refresh_interval_ms != 0);
        let feeds_refresh = feeds_policy.map(|feeds| {
            let stop = stop_ref;
            let failed = failed_ref;
            let engine = engine_ref;
            scope.spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    let Ok(now) = crate::engine::host_time_ms() else {
                        failed.store(true, Ordering::Release);
                        stop.store(true, Ordering::Release);
                        break;
                    };
                    let due = {
                        let Ok(engine) = engine.lock() else {
                            failed.store(true, Ordering::Release);
                            stop.store(true, Ordering::Release);
                            break;
                        };
                        engine.due_feeds(now)
                    };
                    for feed in due {
                        let fetched = crate::feeds::fetch(
                            &feed.url,
                            crate::feeds::FetchOptions {
                                max_bytes: feeds.max_feed_bytes,
                                max_entries: feeds.max_entries_per_feed,
                                max_redirects: feeds.max_redirects,
                                timeout_seconds: feeds.timeout_seconds,
                                allow_private: feeds.allow_private_destinations,
                                etag: feed.etag.as_deref(),
                                last_modified: feed.last_modified.as_deref(),
                            },
                        );
                        let Ok(mut engine) = engine.lock() else {
                            failed.store(true, Ordering::Release);
                            stop.store(true, Ordering::Release);
                            break;
                        };
                        if engine.record_feed_refresh(&feed.id, fetched, now).is_err() {
                            failed.store(true, Ordering::Release);
                            stop.store(true, Ordering::Release);
                            break;
                        }
                    }
                    std::thread::park_timeout(FEED_REFRESH_TICK);
                }
            })
        });
        let _feed_refresh_guard = feeds_refresh;
        println!("kiln-web-service: listening {}", profile.origin);
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if active
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                            (count < profile.bounds.max_connections).then_some(count + 1)
                        })
                        .is_err()
                    {
                        continue;
                    }
                    let permit = Permit(&active);
                    let tls = Arc::clone(&tls);
                    let context = &context;
                    if std::thread::Builder::new()
                        .spawn_scoped(scope, move || {
                            let _permit = permit;
                            // Malformed/closed/expired client connections have no application effect.
                            let _ = connection(stream, tls, context);
                        })
                        .is_err()
                    {
                        stop.store(true, Ordering::Release);
                        polling.thread().unpark();
                        event_hub.wake_watcher();
                        return Err("connection worker could not start".to_owned());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_INTERVAL)
                }
                Err(_) => {
                    stop.store(true, Ordering::Release);
                    polling.thread().unpark();
                    event_hub.wake_watcher();
                    return Err("listener failed".to_owned());
                }
            }
        }
        polling.thread().unpark();
        event_hub.wake_watcher();
        Ok(())
    })?;
    if poll_failed.load(Ordering::Acquire) {
        Err("canonical workflow reconciliation failed; service stopped".to_owned())
    } else {
        Ok(())
    }
}

// r[impl kiln.web.network.boundaries]
/// Private nginx auth_request endpoint. Its credential is never returned to a browser.
/// The trusted nginx caller must overwrite X-Kiln-Peer from the TCP peer socket.
pub fn proxy_auth(path: &Path) -> Result<(), String> {
    let access = AccessProfile::load(path)?;
    let profile = access
        .proxy
        .as_ref()
        .ok_or("proxy identity policy is not configured")?;
    let secret = Zeroizing::new(crate::profile::read_token(&profile.credential_file)?);
    let verifier = crate::auth::TokenVerifier::new(&secret)?;
    let resolver = IdentityResolver::new(profile)?;
    let listener = TcpListener::bind(profile.listen_address)
        .map_err(|_| "private proxy listener unavailable")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "private proxy listener mode failed")?;
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&stop))
            .map_err(|_| "proxy shutdown handler unavailable")?;
    }
    notify::ready()?;
    while !stop.load(Ordering::Acquire) {
        let stream = match listener.accept() {
            Ok((stream, peer)) if peer.ip().is_loopback() => stream,
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_INTERVAL);
                continue;
            }
            Err(_) => return Err("private proxy listener failed".into()),
        };
        let mut stream = DeadlineStream {
            stream,
            deadline: Instant::now() + Duration::from_secs(5),
        };
        let request = read_request(&mut BufReader::new(&mut stream), 8192, 0);
        let principal = request
            .ok()
            .filter(|request| {
                request.method == "GET"
                    && request.target == "/verify"
                    && header(request, "host") == Some("kiln-proxy-auth")
                    && header(request, "authorization").is_none()
                    && header(request, "origin").is_none()
                    && header(request, "x-kiln-principal").is_none()
                    && header(request, "x-kiln-proxy-key")
                        .is_some_and(|key| key.len() == 64 && verifier.matches(key.as_bytes()))
            })
            .and_then(|request| {
                header(&request, "x-kiln-peer")
                    .and_then(|peer| resolver.resolve(profile, peer).ok())
            });
        // Never put labels, roles, credentials or error details in the auth response.
        if let Some(principal) = principal {
            let _ = write!(
                stream,
                "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\nCache-Control: no-store\r\nX-Kiln-Principal: {}\r\n\r\n",
                principal.id
            );
            let _ = stream.flush();
        } else {
            let _ = write_response(&mut stream, error_response(403, "peer_denied"));
        }
    }
    Ok(())
}

struct Permit<'a>(&'a AtomicUsize);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

struct Context<'a> {
    profile: &'a ServiceProfile,
    credentials: &'a Credentials,
    engine: &'a Mutex<Engine>,
    cobs: &'a crate::cobs::Browser,
    sources: &'a crate::sources::Browser,
    reviews: &'a crate::reviews::Browser,
    work: &'a crate::work::Workspace,
    code_review: &'a crate::code_review::Workspace,
    inventory: Option<&'a Path>,
    implementation_work: Option<&'a Path>,
    campaign_controls: Option<&'a crate::campaign_controls::Controls>,
    dispatch_recovery: Option<&'a crate::dispatch_recovery::RecoveryConfig>,
    native_runtime_evidence: Option<&'a crate::runtime_evidence::RuntimeEvidenceProfile>,
    fleet: Option<&'a crate::fleet::Client>,
    delivery_observations: Option<&'a Path>,
    events: Option<&'a EventHub>,
    stop: &'a AtomicBool,
}

type ExactRuntimeQuery = (
    kiln_core::types::ContentId,
    u64,
    kiln_core::types::ContentId,
);

fn native_runtime_query(target: &str) -> Option<ExactRuntimeQuery> {
    let query = target.strip_prefix("/api/implementation/runtime-evidence?attempt=")?;
    let (attempt, rest) = query.split_once("&fence=")?;
    let (fence, work) = rest.split_once("&work=")?;
    let identity = |value: &str| {
        if value.len() != blake3::OUT_LEN * 2
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        kiln_core::types::ContentId::from_blake3_hex(value).ok()
    };
    if fence.starts_with('0') || !fence.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((identity(attempt)?, fence.parse().ok()?, identity(work)?))
}

fn native_runtime_response(
    request: &Request,
    profile: Option<&crate::runtime_evidence::RuntimeEvidenceProfile>,
    principal: &Principal,
) -> ApiResponse {
    const MAX_NATIVE_EVIDENCE_BYTES: usize = 64 * 1024;
    if principal.role != crate::engine::Role::Operator {
        return error_response(403, "native_runtime_evidence_forbidden");
    }
    if request.method != "GET" || !request.body.is_empty() {
        return error_response(405, "native_runtime_evidence_read_only");
    }
    let Some((attempt, fence, work)) = native_runtime_query(&request.target) else {
        return error_response(400, "native_runtime_evidence_query_invalid");
    };
    let Some(profile) = profile else {
        return error_response(503, "native_runtime_evidence_not_configured");
    };
    let Ok(observation) = crate::runtime_evidence::read_lattice(profile, &work, &attempt, fence)
    else {
        return error_response(503, "native_runtime_evidence_unavailable");
    };
    match serde_json::to_vec(&observation) {
        Ok(body) if body.len() <= MAX_NATIVE_EVIDENCE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        _ => error_response(503, "native_runtime_evidence_response_invalid"),
    }
}

fn delivery_environments_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    if context.profile.restricted_projection || principal.role == crate::engine::Role::Worker {
        return error_response(403, "delivery_observation_denied");
    }
    if request.method != "GET" || !request.body.is_empty() {
        return error_response(405, "delivery_read_only");
    }
    if request.target != "/api/delivery/environments" {
        return error_response(400, "delivery_query_invalid");
    }
    let Ok(now) = crate::engine::host_time_ms() else {
        return error_response(503, "clock_unavailable");
    };
    match crate::delivery_observations::read(context.delivery_observations, now) {
        Ok(value) => ApiResponse {
            status: 200,
            content_type: "application/json",
            body: value.to_string().into_bytes(),
            download_name: None,
            headers: Vec::new(),
        },
        Err(code) => error_response(503, code),
    }
}

fn tls_config(profile: &ServiceProfile) -> Result<Arc<ServerConfig>, String> {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    const MAX_TLS_BYTES: usize = 1_048_576;
    let certificate = crate::worker::files::regular(
        Path::new(&profile.tls.certificate_path),
        MAX_TLS_BYTES,
        false,
    )
    .map_err(|_| "TLS certificate unavailable or unsafe".to_owned())?;
    let certificates = certs(&mut BufReader::new(certificate.as_slice()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "TLS certificate malformed".to_owned())?;
    if certificates.is_empty() {
        return Err("TLS certificate empty".to_owned());
    }
    let key_bytes = Zeroizing::new(
        crate::worker::files::regular(
            Path::new(&profile.tls.private_key_path),
            MAX_TLS_BYTES,
            true,
        )
        .map_err(|_| "TLS key unavailable or unsafe".to_owned())?,
    );
    let key = private_key(&mut BufReader::new(key_bytes.as_slice()))
        .map_err(|_| "TLS key malformed".to_owned())?
        .ok_or_else(|| "TLS key empty".to_owned())?;
    let config = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map_err(|_| "TLS configuration invalid".to_owned())?;
    Ok(Arc::new(config))
}

fn connection(
    stream: TcpStream,
    tls: Arc<ServerConfig>,
    context: &Context<'_>,
) -> std::io::Result<()> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(context.profile.bounds.timeout_seconds))
        .ok_or_else(|| std::io::Error::other("connection deadline invalid"))?;
    let stream = DeadlineStream { stream, deadline };
    let connection = ServerConnection::new(tls).map_err(std::io::Error::other)?;
    let mut stream = StreamOwned::new(connection, stream);
    let request = match read_request(
        &mut BufReader::new(&mut stream),
        context.profile.bounds.max_header_bytes,
        context.profile.bounds.max_request_bytes,
    ) {
        Ok(request) => request,
        Err(status) => {
            return write_response(&mut stream, error_response(status, "request_rejected"));
        }
    };
    let request_path = request
        .target
        .split_once('?')
        .map_or(request.target.as_str(), |(path, _)| path);
    if request.method == "GET"
        && (request_path == "/api/fleet/events" || request_path == "/api/events")
    {
        stream.get_mut().deadline = Instant::now()
            .checked_add(SSE_STREAM_LIFETIME)
            .ok_or_else(|| std::io::Error::other("stream deadline invalid"))?;
        let mut disconnect_check = |tls_stream: &mut StreamOwned<ServerConnection, DeadlineStream>| {
            is_tls_peer_disconnected(tls_stream)
        };
        return if request_path == "/api/events" {
            events_stream(&mut stream, &request, context, &mut disconnect_check)
        } else {
            fleet_stream(&mut stream, &request, context, &mut disconnect_check)
        };
    }
    let is_post = request.method == "POST";
    let response = route(request, context);
    if is_post && (200..300).contains(&response.status) {
        if let Some(hub) = context.events {
            hub.wake_watcher();
        }
    }
    write_response(&mut stream, response)
}

struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
}

impl DeadlineStream {
    fn remaining(&self) -> std::io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "connection deadline elapsed")
            })
    }
}

impl Read for DeadlineStream {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(bytes)
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.flush()
    }
}

// Headers remain private, are bounded by the framing reader, and are never logged.
struct Request {
    method: String,
    target: String,
    body: Vec<u8>,
    headers: BTreeMap<String, String>,
}

fn line<R: BufRead>(reader: &mut R, remaining: &mut usize) -> Result<Vec<u8>, u16> {
    let mut bytes = Vec::new();
    let count = reader
        .take((*remaining as u64).saturating_add(1))
        .read_until(b'\n', &mut bytes)
        .map_err(|_| 400_u16)?;
    if count > *remaining {
        return Err(431);
    }
    *remaining -= count;
    if !bytes.ends_with(b"\r\n") {
        return Err(400);
    }
    bytes.truncate(bytes.len() - 2);
    if !bytes.iter().all(|byte| (b' '..=b'~').contains(byte)) {
        return Err(400);
    }
    Ok(bytes)
}

fn read_request<R: BufRead>(
    reader: &mut R,
    max_header: usize,
    max_body: usize,
) -> Result<Request, u16> {
    let mut remaining = max_header;
    let first = line(reader, &mut remaining)?;
    let first = std::str::from_utf8(&first).map_err(|_| 400_u16)?;
    let mut parts = first.split(' ');
    let method = parts.next().ok_or(400_u16)?;
    let target = parts.next().ok_or(400_u16)?;
    if parts.next() != Some("HTTP/1.1")
        || parts.next().is_some()
        || !target.starts_with('/')
        || target.starts_with("//")
        || target.contains('#')
    {
        return Err(400);
    }
    if !matches!(method, "GET" | "POST") {
        return Err(405);
    }
    let mut request = Request {
        method: method.to_owned(),
        target: target.to_owned(),
        body: Vec::new(),
        headers: BTreeMap::new(),
    };
    let mut length = None;
    let mut ended = false;
    for _ in 0..MAX_HEADER_FIELDS {
        let bytes = line(reader, &mut remaining)?;
        if bytes.is_empty() {
            ended = true;
            break;
        }
        let header = std::str::from_utf8(&bytes).map_err(|_| 400_u16)?;
        let (name, value) = header.split_once(':').ok_or(400_u16)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        {
            return Err(400);
        }
        let value = value.trim_matches(' ');
        let name = name.to_ascii_lowercase();
        if request.headers.contains_key(&name) {
            return Err(400);
        }
        match name.as_str() {
            "content-length" if length.is_none() => {
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(400);
                }
                let size = value.parse::<usize>().map_err(|_| 413_u16)?;
                if size > max_body {
                    return Err(413);
                }
                length = Some(size);
            }
            "transfer-encoding" | "expect" => return Err(400),
            _ => {}
        }
        request.headers.insert(name, value.to_owned());
    }
    if !ended {
        return Err(431);
    }
    if header(&request, "host").is_none_or(str::is_empty) {
        return Err(400);
    }
    if request.method == "GET" && length.unwrap_or(0) != 0 {
        return Err(400);
    }
    if request.method == "POST" && length.is_none() {
        return Err(411);
    }
    request.body.resize(length.unwrap_or(0), 0);
    reader.read_exact(&mut request.body).map_err(|_| 400_u16)?;
    Ok(request)
}

// r[impl kiln.web.authentication]
fn authenticate_http(
    request: &Request,
    context: &Context<'_>,
) -> Result<Option<Principal>, ApiResponse> {
    let principal = context.credentials.authenticate_request(
        header(request, "authorization"),
        header(request, "x-kiln-proxy-key"),
        header(request, "x-kiln-principal"),
    );
    let host = context
        .profile
        .origin
        .strip_prefix("https://")
        .unwrap_or("");
    if header(request, "host") != Some(host)
        || header(request, "origin").is_some_and(|origin| origin != context.profile.origin)
    {
        return Err(denied(
            request,
            context,
            principal.as_ref(),
            403,
            "origin_denied",
        ));
    }
    if forbidden_identity(request) {
        return Err(denied(
            request,
            context,
            None,
            403,
            "identity_header_denied",
        ));
    }
    if principal.is_none()
        && (header(request, "x-kiln-proxy-key").is_some()
            || header(request, "x-kiln-principal").is_some())
    {
        return Err(denied(request, context, None, 401, "proxy_identity_denied"));
    }
    Ok(principal)
}

fn route(request: Request, context: &Context<'_>) -> ApiResponse {
    let principal = match authenticate_http(&request, context) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    if request.method == "GET"
        && (request.target.starts_with("/feeds.xml")
            || request.target.starts_with("/feeds.rss")
            || request.target.starts_with("/feeds.opml")
            || request.target.starts_with("/feeds/"))
    {
        return feed_publication(&request, context);
    }
    if let Some(id) = trigger_id(&request) {
        // Only this exact signed intake has an Origin/bearer exception. Authentication,
        // repository/ref binding and replay admission belong to the registered trigger.
        if header(&request, "authorization").is_some()
            || header(&request, "x-kiln-proxy-key").is_some()
            || header(&request, "x-kiln-principal").is_some()
        {
            return denied(
                &request,
                context,
                principal.as_ref(),
                403,
                "trigger_identity_denied",
            );
        }
        if !json_content(&request) {
            return denied(&request, context, None, 415, "json_required");
        }
        let (Some(delivery), Some(event), Some(signature)) = (
            header(&request, "x-github-delivery"),
            header(&request, "x-github-event"),
            header(&request, "x-hub-signature-256"),
        ) else {
            return denied(&request, context, None, 401, "signature_required");
        };
        let headers = kiln_adapter_github::events::DeliveryHeaders {
            delivery: delivery.to_owned(),
            event: event.to_owned(),
            signature: signature.to_owned(),
        };
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .automation_trigger(id, &headers, &request.body)
                .unwrap_or_else(|_| error_response(503, "service_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.method == "GET"
        && (request.target == "/fleet/swarms" || request.target.starts_with("/fleet/swarms/"))
    {
        return swarm_html_response(&request, context, principal.as_ref());
    }
    if request.target.starts_with("/api/") {
        if let Err((status, code)) =
            api_admission(&request, &context.profile.origin, principal.as_ref())
        {
            return denied(&request, context, principal.as_ref(), status, code);
        }
        let Some(principal) = principal else {
            return error_response(401, "unauthorized");
        };
        let path = request
            .target
            .split_once('?')
            .map_or(request.target.as_str(), |(path, _)| path);
        if path == "/api/events" || path == "/api/fleet/events" {
            return error_response(405, "events_method_denied");
        }
        if path == "/api/delivery/environments" {
            return delivery_environments_response(&request, context, &principal);
        }
        if path == "/api/inventory" {
            return inventory_api::response(&request, context, &principal);
        }
        if path == "/api/implementation/attention" {
            return inventory_api::attention_response(&request, context, &principal);
        }
        if path == "/api/implementation/reconcile-termination" {
            return inventory_api::reconcile_termination_response(&request, context, &principal);
        }
        if path == "/api/implementation/plan-review" {
            return inventory_api::plan_review_response(&request, context, &principal);
        }
        if path.starts_with("/api/implementation/campaigns/") && path.ends_with("/wake") {
            return inventory_api::campaign_wake_response(&request, context, &principal);
        }
        if path == "/api/implementation/campaigns" {
            return inventory_api::campaign_response(&request, context, &principal);
        }
        if path == "/api/fleet" || path.starts_with("/api/fleet/") {
            if principal.role == crate::engine::Role::Worker {
                return denied(&request, context, Some(&principal), 403, "fleet_read_only");
            }
            return fleet_response(&request, context, &principal);
        }
        if crate::cairn_workspace::http::ROUTES.contains(&path) {
            if context.profile.restricted_projection {
                return crate::cairn_workspace::http::restricted_response(
                    context.implementation_work,
                    &request.method,
                    &request.target,
                    principal.role,
                    &|| inventory_api::cairn_observation(context, &principal),
                    None,
                );
            }
            return crate::cairn_workspace::http::response(
                context.implementation_work,
                &request.method,
                &request.target,
                principal.role,
                &|| inventory_api::cairn_observation(context, &principal),
            );
        }
        if path == crate::dispatch_recovery::ROUTE {
            return crate::dispatch_recovery::response(
                context.implementation_work,
                context.dispatch_recovery,
                &request.method,
                &request.target,
                principal.role,
                &principal.id,
                &request.body,
            );
        }
        if path == "/api/implementation/runtime-evidence" {
            return native_runtime_response(&request, context.native_runtime_evidence, &principal);
        }
        if crate::campaign_controls::ROUTES.contains(&path) {
            return campaign_control_response(&request, context, &principal);
        }
        if path == "/api/feeds" || path.starts_with("/api/feeds/") {
            if principal.role == crate::engine::Role::Worker {
                return denied(&request, context, Some(&principal), 403, "feeds_forbidden");
            }
            return feed_api(&request, context, &principal, path);
        }
        // r[impl kiln.review.authority]
        if path == "/api/code-review" || path.starts_with("/api/code-review/") {
            let now = match crate::engine::host_time_ms().and_then(|value| {
                i64::try_from(value).map_err(|_| "review clock exceeds bound".to_owned())
            }) {
                Ok(value) => value,
                Err(_) => return error_response(503, "clock_unavailable"),
            };
            // Hold canonical CI state stable through admission and the provider CAS.
            // The review workspace never locks Engine, so this lock order is one-way.
            let engine = match context.engine.lock() {
                Ok(engine) => engine,
                Err(_) => return error_response(503, "service_unavailable"),
            };
            let ci = engine.code_review_ci();
            let preview_builds = engine.canonical_preview_builds();
            return context.code_review.response_with_preview_builds(
                &request.method,
                &request.target,
                &request.body,
                crate::code_review::RequestContext {
                    role: principal.role,
                    principal: &principal.id,
                    now_ms: now,
                    ci: &ci,
                },
                &preview_builds,
            );
        }
        // r[impl kiln.work.authority] r[impl kiln.work.monorepo.facts]
        if path == "/api/work" || path.starts_with("/api/work/") {
            let now = match crate::engine::host_time_ms().and_then(|value| {
                i64::try_from(value).map_err(|_| "work clock exceeds bound".to_owned())
            }) {
                Ok(value) => value,
                Err(_) => return error_response(503, "clock_unavailable"),
            };
            // Snapshot the derived facts under the engine lock, then release it
            // before the work store performs signed reads or writes.
            let ci = match context.engine.lock() {
                Ok(engine) => engine.work_ci_facts(),
                Err(_) => return error_response(503, "service_unavailable"),
            };
            return context.work.response_with_ci(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
                &principal.id,
                now,
                &ci,
            );
        }
        if path == "/api/reviews" || path.starts_with("/api/reviews/") {
            if principal.role == crate::engine::Role::Worker || request.method != "GET" {
                return denied(
                    &request,
                    context,
                    Some(&principal),
                    403,
                    "reviews_read_only",
                );
            }
            return context.reviews.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
            );
        }
        if path == "/api/sources" || path.starts_with("/api/sources/") {
            if principal.role == crate::engine::Role::Worker || request.method != "GET" {
                return denied(
                    &request,
                    context,
                    Some(&principal),
                    403,
                    "sources_read_only",
                );
            }
            return context.sources.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
            );
        }
        if path == "/api/cobs" || path.starts_with("/api/cobs/") {
            if principal.role == crate::engine::Role::Worker || request.method != "GET" {
                return denied(&request, context, Some(&principal), 403, "cobs_read_only");
            }
            return context.cobs.response(
                &request.method,
                &request.target,
                &request.body,
                principal.role,
            );
        }
        let is_rollup_read = request.method == "GET"
            && (path == "/api/runbooks/dispatches"
                || (path.starts_with("/api/runbooks/dispatches/") && path.ends_with("/rollup"))
                || (path.starts_with("/api/runbooks/fanouts/") && path.ends_with("/rollup")));
        // Read-only ImplementationObservation lookup runs outside Engine's mutex
        // to preserve one-way lock discipline across stores.
        let impl_work = if is_rollup_read {
            context
                .implementation_work
                .map(crate::rollups::load_implementation_work_index)
                .unwrap_or(crate::rollups::ImplementationWorkIndex::Unconfigured)
        } else {
            crate::rollups::ImplementationWorkIndex::Unconfigured
        };
        let fleet_snapshot = if request.method == "POST"
            && path.starts_with("/api/runbooks/")
            && (path.ends_with("/fanout")
                || path.ends_with("/preview")
                || (path.starts_with("/api/runbooks/fanouts/") && path.ends_with("/continue")))
        {
            context.fleet.map(|client| match client.live_claims() {
                Ok(claims) => crate::engine::FleetClaimsSnapshot::Available(claims),
                Err(()) => crate::engine::FleetClaimsSnapshot::Unavailable,
            })
        } else {
            None
        };
        match context.engine.lock() {
            Ok(mut engine) => {
                if let Some(snapshot) = fleet_snapshot {
                    engine.set_fleet_claims_snapshot(snapshot);
                }
                engine
                    .api_as_with_work(
                        &request.method,
                        &request.target,
                        &request.body,
                        &principal,
                        &impl_work,
                    )
                    .unwrap_or_else(|_| error_response(503, "service_unavailable"))
            }
            Err(_) => error_response(503, "service_unavailable"),
        }
    } else if request.method == "GET" {
        let path = request
            .target
            .split_once('?')
            .map_or(request.target.as_str(), |(path, _)| path);
        // Compiled UI entrypoint; interaction logic is embedded from web.js.
        // r[impl kiln.dashboard.cobs.view]
        // r[impl kiln.dashboard.cobs.view.scenario.restore]
        // r[impl kiln.dashboard.cobs.view.scenario.cleared]
        let (content_type, bytes): (&'static str, &[u8]) = match path {
            "/" => ("text/html; charset=utf-8", include_bytes!("web.html")),
            // r[impl kiln.web.design_system] Pinned Lucide icon data, served from Kiln's own origin.
            "/icons.js" => ("text/javascript; charset=utf-8", include_bytes!("icons.js")),
            "/ui.js" => ("text/javascript; charset=utf-8", include_bytes!("ui.js")),
            "/web.js" => ("text/javascript; charset=utf-8", include_bytes!("web.js")),
            "/web.css" => ("text/css; charset=utf-8", include_bytes!("web.css")),
            "/work.js" => ("text/javascript; charset=utf-8", include_bytes!("work.js")),
            "/work.css" => ("text/css; charset=utf-8", include_bytes!("work.css")),
            "/code-review.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("code-review.js"),
            ),
            "/code-review.css" => ("text/css; charset=utf-8", include_bytes!("code-review.css")),
            "/preview-review.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("preview-review.js"),
            ),
            "/work-interaction.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("work-interaction.js"),
            ),
            "/work-interaction.css" => (
                "text/css; charset=utf-8",
                include_bytes!("work-interaction.css"),
            ),
            "/work-evidence.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("work-evidence.js"),
            ),
            "/work-board.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("work-board.js"),
            ),
            "/project.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("project.js"),
            ),
            "/runbooks.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("runbooks.js"),
            ),
            "/project.css" => ("text/css; charset=utf-8", include_bytes!("project.css")),
            // r[impl kiln.dashboard.source.navigation] Source page module, served like the project page.
            "/source.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("source.js"),
            ),
            "/source.css" => ("text/css; charset=utf-8", include_bytes!("source.css")),
            // r[impl kiln.dashboard.reviews.view] Patchgate receipts page module, served like the source page.
            "/receipts.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("receipts.js"),
            ),
            "/receipts.css" => ("text/css; charset=utf-8", include_bytes!("receipts.css")),
            // r[impl kiln.dashboard.workers.view] One Workers presentation and shared state vocabulary.
            "/workers.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("workers.js"),
            ),
            "/workers.css" => ("text/css; charset=utf-8", include_bytes!("workers.css")),
            // r[impl kiln.cairn.workspace] Cairn changes page module, served like the receipts page.
            "/cairn.js" => ("text/javascript; charset=utf-8", include_bytes!("cairn.js")),
            "/cairn.css" => ("text/css; charset=utf-8", include_bytes!("cairn.css")),
            "/fleet.js" => ("text/javascript; charset=utf-8", include_bytes!("fleet.js")),
            "/fleet.css" => ("text/css; charset=utf-8", include_bytes!("fleet.css")),
            "/swarm-view.css" => (
                "text/css; charset=utf-8",
                crate::swarm_view::SWARM_VIEW_CSS.as_bytes(),
            ),
            "/fleet-runtime.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("fleet-runtime.js"),
            ),
            "/fleet-capacity.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("fleet-capacity.js"),
            ),
            "/fleet-capacity.css" => (
                "text/css; charset=utf-8",
                include_bytes!("fleet-capacity.css"),
            ),
            "/inventory.js" => (
                "text/javascript; charset=utf-8",
                include_bytes!("inventory.js"),
            ),
            "/inventory.css" => ("text/css; charset=utf-8", include_bytes!("inventory.css")),
            // r[impl kiln.web.deploy.runtime] Minimal health exposes no private state.
            "/healthz" => ("application/json", b"{\"status\":\"ok\"}"),
            // r[impl kiln.identity.shared] The shared identity, then the catalog shell it styles.
            "/identity.css" => (
                "text/css; charset=utf-8",
                include_bytes!("../../kiln-dashboard/src/render/identity.css"),
            ),
            "/catalog.css" => (
                "text/css; charset=utf-8",
                include_bytes!("../../kiln-dashboard/src/render/catalog.css"),
            ),
            "/favicon.svg" => ("image/svg+xml", include_bytes!("favicon.svg")),
            _ => return error_response(404, "not_found"),
        };
        ApiResponse {
            status: 200,
            content_type,
            body: bytes.to_vec(),
            download_name: None,
            headers: Vec::new(),
        }
    } else {
        error_response(405, "method_denied")
    }
}

const SSE_STREAM_LIFETIME: Duration = Duration::from_secs(45 * 60);
const SSE_DISCONNECT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const MAX_SSE_SUBSCRIBER_QUEUE: usize = 64;
const EVENT_HUB_SCHEMA: &str = "kiln.events.v1";
const CHANNEL_CI: usize = 0;
const CHANNEL_FLEET: usize = 1;
const CHANNEL_CAIRN: usize = 2;
const CHANNEL_ATTENTION: usize = 3;

#[derive(Clone, Debug)]
pub(super) struct CachedFleetData {
    pub(super) available: bool,
    pub(super) version: Option<String>,
    pub(super) snapshot_bytes: Option<Arc<Vec<u8>>>,
    pub(super) claims_bytes: Option<Arc<Vec<u8>>>,
    pub(super) decisions_bytes: Option<Arc<Vec<u8>>>,
    pub(super) attempts_bytes: Option<Arc<Vec<u8>>>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum BroadcastEvent {
    Ci {
        version: u64,
        data: Arc<Vec<u8>>,
    },
    Fleet {
        version: String,
        data: Arc<Vec<u8>>,
    },
    FleetUnavailable {
        data: Arc<Vec<u8>>,
    },
    Cairn {
        version: String,
        data: Arc<Vec<u8>>,
    },
    Attention {
        digest: String,
        data: Arc<Vec<u8>>,
    },
}

impl BroadcastEvent {
    const fn channel_index(&self) -> usize {
        match self {
            Self::Ci { .. } => CHANNEL_CI,
            Self::Fleet { .. } | Self::FleetUnavailable { .. } => CHANNEL_FLEET,
            Self::Cairn { .. } => CHANNEL_CAIRN,
            Self::Attention { .. } => CHANNEL_ATTENTION,
        }
    }

    fn event_name(&self) -> &'static str {
        match self {
            Self::Ci { .. } => "ci",
            Self::Fleet { .. } | Self::FleetUnavailable { .. } => "fleet",
            Self::Cairn { .. } => "cairn",
            Self::Attention { .. } => "attention",
        }
    }

    fn payload(&self) -> &[u8] {
        match self {
            Self::Ci { data, .. }
            | Self::Fleet { data, .. }
            | Self::FleetUnavailable { data }
            | Self::Cairn { data, .. }
            | Self::Attention { data, .. } => data.as_slice(),
        }
    }
}

#[derive(Default)]
struct SubscriberQueue {
    events: VecDeque<BroadcastEvent>,
    closed: bool,
}

struct SubscriberHandle {
    id: u64,
    channels: [bool; 4],
    queue: Arc<(Mutex<SubscriberQueue>, Condvar)>,
}

struct EventHubState {
    ci_version: u64,
    ci_snapshot: Option<Arc<Vec<u8>>>,
    fleet: Option<CachedFleetData>,
    cairn_version: Option<String>,
    cairn_event: Option<Arc<Vec<u8>>>,
    attention_digest: Option<String>,
    attention_event: Option<Arc<Vec<u8>>>,
    next_sub_id: u64,
    subscribers: Vec<SubscriberHandle>,
    watcher_wakeup: bool,
}

pub(crate) struct EventHub {
    state: Mutex<EventHubState>,
    watcher_cv: Condvar,
}

struct InitialSnapshot {
    ci_version: u64,
    fleet_version: Option<String>,
    events: Vec<BroadcastEvent>,
}

struct EventSubscription<'a> {
    id: u64,
    hub: &'a EventHub,
    queue: Arc<(Mutex<SubscriberQueue>, Condvar)>,
}

impl Drop for EventSubscription<'_> {
    fn drop(&mut self) {
        if let Ok(mut q) = self.queue.0.lock() {
            q.closed = true;
            self.queue.1.notify_all();
        }
        if let Ok(mut state) = self.hub.state.lock() {
            state.subscribers.retain(|sub| sub.id != self.id);
        }
    }
}

impl EventHub {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(EventHubState {
                ci_version: 0,
                ci_snapshot: None,
                fleet: None,
                cairn_version: None,
                cairn_event: None,
                attention_digest: None,
                attention_event: None,
                next_sub_id: 1,
                subscribers: Vec::new(),
                watcher_wakeup: false,
            }),
            watcher_cv: Condvar::new(),
        }
    }

    pub(crate) fn wake_watcher(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.watcher_wakeup = true;
            for sub in &state.subscribers {
                sub.queue.1.notify_all();
            }
            self.watcher_cv.notify_all();
        }
    }

    pub(super) fn cached_fleet_attention(&self) -> Option<CachedFleetData> {
        self.state.lock().ok().and_then(|state| state.fleet.clone())
    }

    fn subscribe(&self, channels: [bool; 4]) -> (EventSubscription<'_>, InitialSnapshot) {
        let queue = Arc::new((Mutex::new(SubscriberQueue::default()), Condvar::new()));
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let id = state.next_sub_id;
        state.next_sub_id = state.next_sub_id.saturating_add(1);
        state.subscribers.push(SubscriberHandle {
            id,
            channels,
            queue: Arc::clone(&queue),
        });
        let mut initial = Vec::new();
        if channels[CHANNEL_CI]
            && let Some(data) = &state.ci_snapshot
        {
            initial.push(BroadcastEvent::Ci {
                version: state.ci_version,
                data: Arc::clone(data),
            });
        }
        if channels[CHANNEL_FLEET]
            && let Some(fleet) = &state.fleet
        {
            if fleet.available {
                if let (Some(version), Some(data)) = (&fleet.version, &fleet.snapshot_bytes) {
                    initial.push(BroadcastEvent::Fleet {
                        version: version.clone(),
                        data: Arc::clone(data),
                    });
                }
            } else if let Some(data) = &fleet.snapshot_bytes {
                initial.push(BroadcastEvent::FleetUnavailable {
                    data: Arc::clone(data),
                });
            }
        }
        if channels[CHANNEL_CAIRN]
            && let (Some(version), Some(data)) = (&state.cairn_version, &state.cairn_event)
        {
            initial.push(BroadcastEvent::Cairn {
                version: version.clone(),
                data: Arc::clone(data),
            });
        }
        if channels[CHANNEL_ATTENTION]
            && let (Some(digest), Some(data)) = (&state.attention_digest, &state.attention_event)
        {
            initial.push(BroadcastEvent::Attention {
                digest: digest.clone(),
                data: Arc::clone(data),
            });
        }
        let snapshot = InitialSnapshot {
            ci_version: state.ci_version,
            fleet_version: state.fleet.as_ref().and_then(|f| f.version.clone()),
            events: initial,
        };
        (EventSubscription { id, hub: self, queue }, snapshot)
    }

    fn fanout_locked(state: &mut EventHubState, event: BroadcastEvent) {
        let ch = event.channel_index();
        state.subscribers.retain(|sub| {
            let Ok(mut q) = sub.queue.0.lock() else {
                return false;
            };
            if q.closed {
                return false;
            }
            if sub.channels[ch] {
                if q.events.len() >= MAX_SSE_SUBSCRIBER_QUEUE {
                    if let Some(pos) = q.events.iter().position(|old| old.channel_index() == ch) {
                        q.events.remove(pos);
                    } else {
                        q.events.pop_front();
                    }
                }
                q.events.push_back(event.clone());
                sub.queue.1.notify_one();
            }
            true
        });
    }

    fn publish_ci(&self, version: u64, data: Vec<u8>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.ci_snapshot.is_some() && state.ci_version == version {
            return false;
        }
        let arc = Arc::new(data);
        state.ci_version = version;
        state.ci_snapshot = Some(Arc::clone(&arc));
        Self::fanout_locked(&mut state, BroadcastEvent::Ci { version, data: arc });
        true
    }

    fn publish_fleet_available(&self, cached: CachedFleetData) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let same = state.fleet.as_ref().is_some_and(|prev| {
            prev.available && prev.version.is_some() && prev.version == cached.version
        });
        if same {
            return false;
        }
        let Some(version) = cached.version.clone() else {
            return false;
        };
        let Some(data) = cached.snapshot_bytes.clone() else {
            return false;
        };
        state.fleet = Some(cached);
        Self::fanout_locked(&mut state, BroadcastEvent::Fleet { version, data });
        true
    }

    fn publish_fleet_unavailable(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.fleet.as_ref().is_some_and(|prev| !prev.available) {
            return false;
        }
        let data = Arc::new(
            serde_json::json!({
                "unavailable": true,
                "error": "fleet_unavailable",
            })
            .to_string()
            .into_bytes(),
        );
        state.fleet = Some(CachedFleetData {
            available: false,
            version: None,
            snapshot_bytes: Some(Arc::clone(&data)),
            claims_bytes: None,
            decisions_bytes: None,
            attempts_bytes: None,
        });
        Self::fanout_locked(&mut state, BroadcastEvent::FleetUnavailable { data });
        true
    }

    fn publish_cairn(&self, version: String, data: Vec<u8>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.cairn_version.as_deref() == Some(version.as_str()) {
            return false;
        }
        let arc = Arc::new(data);
        state.cairn_version = Some(version.clone());
        state.cairn_event = Some(Arc::clone(&arc));
        Self::fanout_locked(&mut state, BroadcastEvent::Cairn { version, data: arc });
        true
    }

    fn publish_attention(&self, digest: String, data: Vec<u8>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.attention_digest.as_deref() == Some(digest.as_str()) {
            return false;
        }
        let arc = Arc::new(data);
        state.attention_digest = Some(digest.clone());
        state.attention_event = Some(Arc::clone(&arc));
        Self::fanout_locked(&mut state, BroadcastEvent::Attention { digest, data: arc });
        true
    }
}

fn read_cairn_version(database: &Path) -> Option<String> {
    let connection = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let _ = connection.pragma_update(None, "query_only", true);
    let table_sum = |table: &str| -> i64 {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                rusqlite::params![table],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !exists {
            return 0;
        }
        connection
            .query_row(
                &format!("SELECT COALESCE(SUM(version), 0) FROM {table}"),
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    };
    Some(format!(
        "{}:{}:{}",
        table_sum("implementation_state"),
        table_sum("implementation_campaign"),
        table_sum("implementation_campaign_continuation")
    ))
}

use rusqlite::{Connection, OpenFlags};

enum FleetPoll {
    Unchanged,
    Updated(CachedFleetData),
    Unavailable,
}

fn poll_fleet_once(client: &crate::fleet::Client, last_version: Option<&str>) -> FleetPoll {
    let Ok(version_reply) = client.call("GET", "/v1/version", None) else {
        return FleetPoll::Unavailable;
    };
    if !(200..300).contains(&version_reply.status) {
        return FleetPoll::Unavailable;
    }
    let Ok(version_val) = serde_json::from_slice::<serde_json::Value>(&version_reply.body) else {
        return FleetPoll::Unavailable;
    };
    let Some(version) = version_val
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .map(str::to_owned)
    else {
        return FleetPoll::Unavailable;
    };
    if last_version == Some(version.as_str()) {
        return FleetPoll::Unchanged;
    }
    let fetch_reply = |path: &str| -> Option<Vec<u8>> {
        let reply = client.call("GET", path, None).ok()?;
        (200..300).contains(&reply.status).then_some(reply.body)
    };
    let Some(health_bytes) = fetch_reply("/v1/health") else {
        return FleetPoll::Unavailable;
    };
    let claims_bytes = fetch_reply("/v1/claims?include=foreign,expired")
        .or_else(|| fetch_reply("/v1/claims"));
    let Some(claims_bytes) = claims_bytes else {
        return FleetPoll::Unavailable;
    };
    let Some(work_bytes) = fetch_reply("/v1/work") else {
        return FleetPoll::Unavailable;
    };
    let Some(attempts_bytes) = fetch_reply("/v1/attempts") else {
        return FleetPoll::Unavailable;
    };
    let Some(decisions_bytes) = fetch_reply("/v1/decisions") else {
        return FleetPoll::Unavailable;
    };
    let machines_bytes = fetch_reply("/v1/machines");
    let parse = |bytes: &[u8]| serde_json::from_slice::<serde_json::Value>(bytes).ok();
    let (Some(health), Some(claims), Some(work), Some(attempts), Some(decisions)) = (
        parse(&health_bytes),
        parse(&claims_bytes),
        parse(&work_bytes),
        parse(&attempts_bytes),
        parse(&decisions_bytes),
    ) else {
        return FleetPoll::Unavailable;
    };
    let machines = machines_bytes.as_deref().and_then(parse);
    let mut snapshot = serde_json::json!({
        "version": version,
        "health": health,
        "claims": claims,
        "work": work,
        "attempts": attempts,
        "decisions": decisions,
    });
    if let (Some(machines), Some(obj)) = (machines, snapshot.as_object_mut()) {
        obj.insert("machines".into(), machines);
    }
    let Ok(snapshot_bytes) = serde_json::to_vec(&snapshot) else {
        return FleetPoll::Unavailable;
    };
    FleetPoll::Updated(CachedFleetData {
        available: true,
        version: Some(version),
        snapshot_bytes: Some(Arc::new(snapshot_bytes)),
        claims_bytes: Some(Arc::new(claims_bytes)),
        decisions_bytes: Some(Arc::new(decisions_bytes)),
        attempts_bytes: Some(Arc::new(attempts_bytes)),
    })
}

pub(crate) fn watcher_step(
    hub: &EventHub,
    engine: &Mutex<Engine>,
    fleet: Option<&crate::fleet::Client>,
    implementation_work: Option<&Path>,
) {
    let mut changed = false;
    let (ci_version, ci_runs, ci_repos, ci_json) = {
        let last_ci = hub
            .state
            .lock()
            .map(|state| (state.ci_version, state.ci_snapshot.is_some()))
            .unwrap_or((0, false));
        if let Ok(guard) = engine.lock() {
            let ver = guard.state_version();
            let snapshot_json = if !last_ci.1 || ver != last_ci.0 {
                guard.live_snapshot_json().ok().map(|(_, bytes)| bytes)
            } else {
                None
            };
            (
                Some(ver),
                guard.attention_runs(),
                guard.project_repositories(),
                snapshot_json,
            )
        } else {
            (None, Vec::new(), Vec::new(), None)
        }
    };
    if let (Some(ver), Some(bytes)) = (ci_version, ci_json)
        && hub.publish_ci(ver, bytes)
    {
        changed = true;
    }

    if let Some(client) = fleet {
        let last_fleet_version = hub
            .state
            .lock()
            .ok()
            .and_then(|state| state.fleet.as_ref().filter(|f| f.available).and_then(|f| f.version.clone()));
        match poll_fleet_once(client, last_fleet_version.as_deref()) {
            FleetPoll::Unchanged => {}
            FleetPoll::Updated(cached) => {
                if hub.publish_fleet_available(cached) {
                    changed = true;
                }
            }
            FleetPoll::Unavailable => {
                if hub.publish_fleet_unavailable() {
                    changed = true;
                }
            }
        }
    }

    if let Some(db) = implementation_work
        && let Some(cairn_ver) = read_cairn_version(db)
    {
        let payload = serde_json::json!({
            "invalidated": true,
            "version": cairn_ver,
        })
        .to_string()
        .into_bytes();
        if hub.publish_cairn(cairn_ver, payload) {
            changed = true;
        }
    }

    let need_attention = changed
        || hub
            .state
            .lock()
            .is_ok_and(|state| state.attention_event.is_none());
    if need_attention && let Ok(now_ms) = crate::engine::host_time_ms() {
        let fleet_cached = hub.cached_fleet_attention();
        if let Some((digest, payload)) = inventory_api::attention_summary_for_event(
            now_ms,
            ci_runs,
            ci_repos,
            implementation_work,
            fleet_cached.as_ref(),
        ) {
            hub.publish_attention(digest, payload);
        }
    }
}

fn run_coordinator_watcher(
    hub: &EventHub,
    engine: &Mutex<Engine>,
    fleet: Option<&crate::fleet::Client>,
    implementation_work: Option<&Path>,
    stop: &AtomicBool,
    tick: Duration,
) {
    while !stop.load(Ordering::Acquire) {
        watcher_step(hub, engine, fleet, implementation_work);
        if stop.load(Ordering::Acquire) {
            break;
        }
        let Ok(mut state) = hub.state.lock() else {
            break;
        };
        if !state.watcher_wakeup {
            let (guard, _) = hub
                .watcher_cv
                .wait_timeout(state, tick)
                .unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
        state.watcher_wakeup = false;
    }
}

fn is_tls_peer_disconnected(stream: &mut StreamOwned<ServerConnection, DeadlineStream>) -> bool {
    if stream.get_ref().stream.set_nonblocking(true).is_err() {
        return false;
    }
    let mut peek_buf = [0u8; 1];
    let disconnected = match stream.get_ref().stream.peek(&mut peek_buf) {
        Ok(0) => true,
        Ok(_) => {
            let mut drain = [0u8; 64];
            match stream.read(&mut drain) {
                Ok(0) => true,
                Ok(_) => false,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    false
                }
                Err(_) => true,
            }
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            false
        }
        Err(_) => true,
    };
    let _ = stream.get_ref().stream.set_nonblocking(false);
    disconnected
}

fn parse_channels(target: &str) -> Result<[bool; 4], ()> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok([true, true, true, true]);
    };
    let value = query.strip_prefix("channels=").ok_or(())?;
    if value.is_empty() || value.contains('&') {
        return Err(());
    }
    let mut channels = [false; 4];
    for item in value.split(',') {
        match item {
            "ci" => channels[CHANNEL_CI] = true,
            "fleet" => channels[CHANNEL_FLEET] = true,
            "cairn" => channels[CHANNEL_CAIRN] = true,
            "attention" => channels[CHANNEL_ATTENTION] = true,
            _ => return Err(()),
        }
    }
    Ok(channels)
}

fn write_sse_frame<W: Write>(writer: &mut W, event: &str, data: &[u8]) -> std::io::Result<()> {
    let text = std::str::from_utf8(data)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "non-utf8 event"))?;
    write!(writer, "event: {event}\ndata: {text}\n\n")
}

// r[impl kiln.web.multiplexed_sse_events]
fn events_stream<S: Read + Write>(
    stream: &mut S,
    request: &Request,
    context: &Context<'_>,
    is_disconnected: &mut dyn FnMut(&mut S) -> bool,
) -> std::io::Result<()> {
    let principal = match authenticate_http(request, context) {
        Ok(Some(principal)) => principal,
        Ok(None) => return write_response(stream, error_response(401, "unauthorized")),
        Err(response) => return write_response(stream, response),
    };
    if principal.role == crate::engine::Role::Worker {
        return write_response(
            stream,
            denied(request, context, Some(&principal), 403, "events_forbidden"),
        );
    }
    if let Err((status, code)) = api_admission(request, &context.profile.origin, Some(&principal)) {
        return write_response(
            stream,
            denied(request, context, Some(&principal), status, code),
        );
    }
    let Ok(channels) = parse_channels(&request.target) else {
        return write_response(stream, error_response(400, "events_query_invalid"));
    };
    let Some(hub) = context.events else {
        return write_response(stream, error_response(503, "events_unavailable"));
    };
    let (sub, initial) = hub.subscribe(channels);
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n{SECURITY_HEADERS}\r\n"
    )?;
    let channel_names: Vec<&'static str> = [
        channels[CHANNEL_CI].then_some("ci"),
        channels[CHANNEL_FLEET].then_some("fleet"),
        channels[CHANNEL_CAIRN].then_some("cairn"),
        channels[CHANNEL_ATTENTION].then_some("attention"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let ready_payload = serde_json::json!({
        "schema": EVENT_HUB_SCHEMA,
        "channels": channel_names,
        "fleet_configured": context.fleet.is_some(),
        "cairn_configured": context.implementation_work.is_some(),
        "ci_version": initial.ci_version,
        "fleet_version": initial.fleet_version,
    })
    .to_string()
    .into_bytes();
    write_sse_frame(stream, "ready", &ready_payload)?;
    for event in initial.events {
        write_sse_frame(stream, event.event_name(), event.payload())?;
    }
    stream.flush()?;

    let deadline = Instant::now()
        .checked_add(SSE_STREAM_LIFETIME)
        .ok_or_else(|| std::io::Error::other("stream deadline invalid"))?;
    let mut last_keepalive = Instant::now();
    while Instant::now() < deadline && !context.stop.load(Ordering::Acquire) {
        if is_disconnected(stream) {
            break;
        }
        let next_event = {
            let Ok(mut q) = sub.queue.0.lock() else {
                break;
            };
            if q.events.is_empty() && !q.closed {
                let (guard, _) = sub
                    .queue
                    .1
                    .wait_timeout(q, SSE_DISCONNECT_POLL_INTERVAL)
                    .unwrap_or_else(|e| e.into_inner());
                q = guard;
            }
            if q.closed {
                break;
            }
            q.events.pop_front()
        };
        if let Some(event) = next_event {
            if write_sse_frame(stream, event.event_name(), event.payload()).is_err()
                || stream.flush().is_err()
            {
                break;
            }
            last_keepalive = Instant::now();
        } else if last_keepalive.elapsed() >= SSE_KEEPALIVE_INTERVAL {
            if stream.write_all(b": keepalive\n\n").is_err() || stream.flush().is_err() {
                break;
            }
            last_keepalive = Instant::now();
        }
    }
    Ok(())
}

fn event_after(target: &str) -> Result<Option<String>, ()> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    let mut after = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or(())?;
        if name != "after"
            || after.is_some()
            || value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(());
        }
        after = Some(value.to_owned());
    }
    Ok(after)
}

// r[impl kiln.web.network.boundaries] The console projects only the reviewed
// fleet coordinator routes over its loopback listener. It never stores fleet
// authority, and every mutation still passes the fleet service's own checks.
// r[impl kiln.web.multiplexed_sse_events] Consumes the single shared watcher.
fn fleet_stream<S: Read + Write>(
    writer: &mut S,
    request: &Request,
    context: &Context<'_>,
    is_disconnected: &mut dyn FnMut(&mut S) -> bool,
) -> std::io::Result<()> {
    let principal = match authenticate_http(request, context) {
        Ok(Some(principal)) => principal,
        Ok(None) => return write_response(writer, error_response(401, "unauthorized")),
        Err(response) => return write_response(writer, response),
    };
    if principal.role == crate::engine::Role::Worker {
        return write_response(
            writer,
            denied(request, context, Some(&principal), 403, "fleet_read_only"),
        );
    }
    if let Err((status, code)) = api_admission(request, &context.profile.origin, Some(&principal)) {
        return write_response(
            writer,
            denied(request, context, Some(&principal), status, code),
        );
    }
    let Some(client) = context.fleet else {
        return write_response(writer, error_response(503, "fleet_unavailable"));
    };
    let Ok(mut after) = event_after(&request.target) else {
        return write_response(writer, error_response(400, "fleet_after_invalid"));
    };
    write!(
        writer,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n{SECURITY_HEADERS}\r\n"
    )?;
    writer.flush()?;
    let deadline = Instant::now()
        .checked_add(SSE_STREAM_LIFETIME)
        .ok_or_else(|| std::io::Error::other("stream deadline invalid"))?;

    if let Some(hub) = context.events {
        let (sub, initial) = hub.subscribe([false, true, false, false]);
        for event in initial.events {
            match event {
                BroadcastEvent::Fleet { version, data }
                    if after.as_deref() != Some(version.as_str()) =>
                {
                    after = Some(version);
                    if write_sse_frame(writer, "snapshot", &data).is_err()
                        || writer.flush().is_err()
                    {
                        return Ok(());
                    }
                }
                BroadcastEvent::FleetUnavailable { .. } => {
                    let _ = writer.write_all(b": unavailable\n\n");
                    let _ = writer.flush();
                    return Ok(());
                }
                _ => {}
            }
        }
        let mut last_heartbeat = Instant::now();
        while Instant::now() < deadline && !context.stop.load(Ordering::Acquire) {
            if is_disconnected(writer) {
                break;
            }
            let next_event = {
                let Ok(mut q) = sub.queue.0.lock() else {
                    break;
                };
                if q.events.is_empty() && !q.closed {
                    let (guard, _) = sub
                        .queue
                        .1
                        .wait_timeout(q, SSE_DISCONNECT_POLL_INTERVAL)
                        .unwrap_or_else(|e| e.into_inner());
                    q = guard;
                }
                if q.closed {
                    break;
                }
                q.events.pop_front()
            };
            match next_event {
                Some(BroadcastEvent::Fleet { version, data }) => {
                    if after.as_deref() != Some(version.as_str()) {
                        after = Some(version);
                        if write_sse_frame(writer, "snapshot", &data).is_err()
                            || writer.flush().is_err()
                        {
                            break;
                        }
                        last_heartbeat = Instant::now();
                    }
                }
                Some(BroadcastEvent::FleetUnavailable { .. }) => {
                    let _ = writer.write_all(b": unavailable\n\n");
                    let _ = writer.flush();
                    break;
                }
                _ => {
                    if last_heartbeat.elapsed() >= Duration::from_secs(4) {
                        if writer.write_all(b": heartbeat\n\n").is_err()
                            || writer.flush().is_err()
                        {
                            break;
                        }
                        last_heartbeat = Instant::now();
                    }
                }
            }
        }
        return Ok(());
    }

    while Instant::now() < deadline && !context.stop.load(Ordering::Acquire) {
        if is_disconnected(writer) {
            break;
        }
        let (wrote, failed) = match client.events(after.clone()) {
            Ok(crate::fleet::EventFrame::Snapshot { version, data }) => {
                let Ok(data) = String::from_utf8(data) else {
                    break;
                };
                after = Some(version);
                (write!(writer, "event: snapshot\ndata: {data}\n\n"), false)
            }
            Ok(crate::fleet::EventFrame::Heartbeat) => (write!(writer, ": heartbeat\n\n"), false),
            Err(()) => (write!(writer, ": unavailable\n\n"), true),
        };
        if wrote.is_err() || writer.flush().is_err() || failed {
            break;
        }
    }
    Ok(())
}

fn fleet_response(request: &Request, context: &Context<'_>, principal: &Principal) -> ApiResponse {
    if request.method == "POST" && principal.role != crate::engine::Role::Operator {
        return denied(request, context, Some(principal), 403, "fleet_operator");
    }
    let Some(client) = context.fleet else {
        return error_response(503, "fleet_unavailable");
    };
    let Some(path) = crate::fleet::upstream_path(&request.method, &request.target) else {
        return error_response(404, "fleet_route_denied");
    };
    if request.body.len() > crate::fleet::MAX_FLEET_REQUEST_BYTES {
        return error_response(413, "fleet_request_too_large");
    }
    let body = (!request.body.is_empty()).then_some(request.body.as_slice());
    match client.call(&request.method, &path, body) {
        Ok(reply) => ApiResponse {
            status: reply.status,
            content_type: reply.content_type,
            body: reply.body,
            download_name: None,
            headers: vec![("Cache-Control", "no-store".to_owned())],
        },
        Err(()) => error_response(503, "fleet_unavailable"),
    }
}

// r[impl kiln.fleet.swarm.console]
fn swarm_html_response(
    request: &Request,
    context: &Context<'_>,
    principal: Option<&Principal>,
) -> ApiResponse {
    let Some(principal) = principal else {
        return error_response(401, "unauthorized");
    };
    if principal.role == crate::engine::Role::Worker {
        return denied(request, context, Some(principal), 403, "fleet_read_only");
    }
    let Some(client) = context.fleet else {
        return error_response(503, "fleet_unavailable");
    };
    let (path, query) = match request.target.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (request.target.as_str(), None),
    };
    let html_headers = vec![("Cache-Control", "no-store".to_owned())];
    if path == "/fleet/swarms" {
        if query.is_some() {
            return error_response(400, "swarm_query_invalid");
        }
        let Ok(reply) = client.call("GET", "/v1/swarms", None) else {
            return error_response(503, "fleet_unavailable");
        };
        if !(200..300).contains(&reply.status) {
            return error_response(reply.status, "fleet_swarm_unavailable");
        }
        let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&reply.body) else {
            return error_response(502, "fleet_swarm_malformed");
        };
        let Ok(runs) = serde_json::from_value::<Vec<kiln_core::fleet::SwarmRun>>(
            payload
                .get("swarms")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        ) else {
            return error_response(502, "fleet_swarm_malformed");
        };
        let document = crate::swarm_view::render_swarm_list_html(&runs);
        return ApiResponse {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: document.into_bytes(),
            download_name: None,
            headers: html_headers,
        };
    }
    let Some(id) = path.strip_prefix("/fleet/swarms/") else {
        return error_response(404, "not_found");
    };
    if id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return error_response(404, "not_found");
    }
    let up_to_seq = match query {
        None | Some("") => None,
        Some(raw) => {
            let Some((key, value)) = raw.split_once('=') else {
                return error_response(400, "swarm_query_invalid");
            };
            if key != "up_to_seq" || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit())
            {
                return error_response(400, "swarm_query_invalid");
            }
            let Ok(parsed) = value.parse::<u64>() else {
                return error_response(400, "swarm_query_invalid");
            };
            Some(parsed)
        }
    };
    let Ok(reply) = client.call("GET", &format!("/v1/swarms/{id}"), None) else {
        return error_response(503, "fleet_unavailable");
    };
    if reply.status == 404 {
        return error_response(404, "fleet_swarm_unknown");
    }
    if !(200..300).contains(&reply.status) {
        return error_response(reply.status, "fleet_swarm_unavailable");
    }
    let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&reply.body) else {
        return error_response(502, "fleet_swarm_malformed");
    };
    let Ok(run) = serde_json::from_value::<kiln_core::fleet::SwarmRun>(
        payload
            .get("swarm")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    ) else {
        return error_response(502, "fleet_swarm_malformed");
    };
    let document = crate::swarm_view::render_swarm_detail_html(&run, up_to_seq);
    ApiResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: document.into_bytes(),
        download_name: None,
        headers: html_headers,
    }
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.headers.get(name).map(String::as_str)
}

fn forbidden_identity(request: &Request) -> bool {
    request.headers.keys().any(|name| {
        (name.starts_with("x-kiln-")
            && !matches!(name.as_str(), "x-kiln-proxy-key" | "x-kiln-principal"))
            || name.starts_with("tailscale-")
            || matches!(
                name.as_str(),
                "x-forwarded-user" | "x-auth-request-user" | "remote-user"
            )
    })
}

fn trigger_id(request: &Request) -> Option<&str> {
    if request.method != "POST" {
        return None;
    }
    request
        .target
        .strip_prefix("/api/triggers/")
        .filter(|id| crate::profile::identifier(id))
}

fn json_content(request: &Request) -> bool {
    matches!(
        header(request, "content-type"),
        Some("application/json" | "application/json; charset=utf-8")
    )
}

// r[impl kiln.web.authentication] r[impl kiln.web.network.boundaries]
// r[impl kiln.web.feeds.token]
fn feed_publication(request: &Request, context: &Context<'_>) -> ApiResponse {
    let (path, query_token) = match feed_target(&request.target) {
        Some(value) => value,
        None => return error_response(400, "feed_invalid"),
    };
    let bearer_token = header(request, "authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| is_feed_token(value));
    let token = match (query_token, bearer_token) {
        (Some(token), _) => token,
        (None, Some(token)) => token,
        (None, None) => return error_response(400, "feed_invalid"),
    };
    if !context.credentials.feed_matches(token.as_bytes()) {
        return error_response(403, "feed_denied");
    }
    match context.engine.lock() {
        Ok(engine) => match engine.render_feed(path) {
            Some(mut response) => {
                let etag = format!("\"{}\"", blake3::hash(&response.body).to_hex());
                response.headers.push(("ETag", etag.clone()));
                response
                    .headers
                    .push(("Cache-Control", "no-cache".to_string()));
                let not_modified = header(request, "if-none-match").is_some_and(|value| {
                    value == "*"
                        || value
                            .split(',')
                            .map(str::trim)
                            .any(|candidate| candidate == etag)
                });
                if not_modified {
                    response.status = 304;
                    response.body.clear();
                }
                response
            }
            None => error_response(404, "feed_missing"),
        },
        Err(_) => error_response(503, "service_unavailable"),
    }
}

/// Feed paths accept one optional `token` query parameter with a 64-hex value.
/// A missing query token may instead be supplied as a bearer header.
fn feed_target(target: &str) -> Option<(&str, Option<&str>)> {
    let Some((path, query)) = target.split_once('?') else {
        return Some((target, None));
    };
    if query.contains('&') {
        return None;
    }
    let token = query.strip_prefix("token=")?;
    if !is_feed_token(token) {
        return None;
    }
    Some((path, Some(token)))
}

fn is_feed_token(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

// r[impl kiln.web.feeds.subscriptions.scenario.viewer]
// r[impl kiln.web.feeds.ui.scenario.search]
fn feed_query(target: &str) -> Result<Option<String>, ()> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    let value = query.strip_prefix("q=").ok_or(())?;
    if value.len() > 1024 || value.contains('&') {
        return Err(());
    }
    let decoded = crate::engine::api::decoded(value)?;
    if decoded.len() > 256 || decoded.chars().any(char::is_control) {
        return Err(());
    }
    Ok(Some(decoded))
}

fn feed_api(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
    path: &str,
) -> ApiResponse {
    let now = match crate::engine::host_time_ms() {
        Ok(now) => now,
        Err(_) => return error_response(503, "clock_unavailable"),
    };
    if request.method == "GET" && path == "/api/feeds/export" {
        if principal.role != crate::engine::Role::Operator {
            return error_response(403, "operator_required");
        }
        return match context.engine.lock() {
            Ok(engine) => engine
                .feeds_opml()
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.method == "GET" && path == "/api/feeds" {
        let query = match feed_query(&request.target) {
            Ok(query) => query,
            Err(()) => return error_response(400, "feed_query_invalid"),
        };
        return match context.engine.lock() {
            Ok(engine) => engine
                .feeds_view(principal.role, query.as_deref())
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    let Some(profile) = &context.profile.feeds else {
        return error_response(404, "feeds_disabled");
    };
    if request.method == "POST" && path == "/api/feeds/import" {
        let admitted = match context.engine.lock() {
            Ok(engine) => engine.prepare_feed_import(&request.body),
            Err(_) => return error_response(503, "service_unavailable"),
        };
        let admitted = match admitted {
            Ok(admitted) => admitted,
            Err(message) => {
                let (status, code) = if message.contains("not configured") {
                    (404, "feeds_disabled")
                } else if message.contains("bound") {
                    (409, "feed_retention")
                } else {
                    (400, "feed_import_invalid")
                };
                return error_response(status, code);
            }
        };
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .finish_feed_import(admitted, &request.method, &request.target, principal)
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.method == "POST" && path == "/api/feeds" {
        let admitted = match context.engine.lock() {
            Ok(engine) => engine.prepare_feed_add(&request.body),
            Err(_) => return error_response(503, "service_unavailable"),
        };
        let admitted = match admitted {
            Ok(admitted) => admitted,
            Err(message) => {
                let (status, code) = if message.contains("not configured") {
                    (404, "feeds_disabled")
                } else if message.contains("bound") {
                    (409, "feed_retention")
                } else {
                    (400, "feed_invalid")
                };
                return error_response(status, code);
            }
        };
        let url = admitted.url.clone();
        let fetched = crate::feeds::fetch(
            &url,
            crate::feeds::FetchOptions {
                max_bytes: profile.max_feed_bytes,
                max_entries: profile.max_entries_per_feed,
                max_redirects: profile.max_redirects,
                timeout_seconds: profile.timeout_seconds,
                allow_private: profile.allow_private_destinations,
                etag: None,
                last_modified: None,
            },
        );
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .finish_feed_add(
                    admitted,
                    fetched,
                    now,
                    &request.method,
                    &request.target,
                    principal,
                )
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    let Some(rest) = path.strip_prefix("/api/feeds/") else {
        return error_response(404, "feed_route_invalid");
    };
    let mut parts = rest.split('/');
    let id = parts.next().unwrap_or_default();
    let action = parts.next();
    if id.is_empty() || parts.next().is_some() {
        return error_response(404, "feed_route_invalid");
    }
    if request.method == "POST" && action == Some("enabled") {
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .set_feed_enabled(
                    id,
                    &request.body,
                    &request.method,
                    &request.target,
                    principal,
                )
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.method == "POST" && action == Some("label") {
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .rename_feed(
                    id,
                    &request.body,
                    &request.method,
                    &request.target,
                    principal,
                )
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.method == "POST" && action == Some("remove") {
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .remove_feed(id, &request.method, &request.target, principal)
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    if request.method == "POST" && action == Some("refresh") {
        let target = match context.engine.lock() {
            Ok(engine) => engine.feed_refresh_target(id),
            Err(_) => return error_response(503, "service_unavailable"),
        };
        let Some((url, etag, last_modified)) = target else {
            return error_response(404, "feed_missing");
        };
        let fetched = crate::feeds::fetch(
            &url,
            crate::feeds::FetchOptions {
                max_bytes: profile.max_feed_bytes,
                max_entries: profile.max_entries_per_feed,
                max_redirects: profile.max_redirects,
                timeout_seconds: profile.timeout_seconds,
                allow_private: profile.allow_private_destinations,
                etag: etag.as_deref(),
                last_modified: last_modified.as_deref(),
            },
        );
        return match context.engine.lock() {
            Ok(mut engine) => engine
                .finish_feed_refresh(
                    id,
                    fetched,
                    now,
                    &request.method,
                    &request.target,
                    principal,
                )
                .unwrap_or_else(|_| error_response(503, "feed_state_unavailable")),
            Err(_) => error_response(503, "service_unavailable"),
        };
    }
    error_response(404, "feed_route_invalid")
}

fn api_admission(
    request: &Request,
    origin: &str,
    principal: Option<&Principal>,
) -> Result<(), (u16, &'static str)> {
    if principal.is_none() {
        return Err((401, "unauthorized"));
    }
    if request.method == "POST" {
        if header(request, "origin") != Some(origin) {
            return Err((403, "origin_required"));
        }
        if !json_content(request) {
            return Err((415, "json_required"));
        }
        if principal.is_some_and(|principal| principal.role == crate::engine::Role::Viewer)
            && request.target != "/api/work/commands"
            && request.target != "/api/code-review/commands"
        {
            return Err((403, "operator_required"));
        }
    }
    Ok(())
}

fn denied(
    request: &Request,
    context: &Context<'_>,
    principal: Option<&Principal>,
    status: u16,
    code: &str,
) -> ApiResponse {
    match context.engine.lock() {
        Ok(mut engine) => {
            match engine.record_denial(&request.method, &request.target, principal, code) {
                Ok(()) => error_response(status, code),
                Err(_) => error_response(503, "audit_unavailable"),
            }
        }
        Err(_) => error_response(503, "service_unavailable"),
    }
}

// r[impl kiln.implementation.campaign.controls.scenario.roles]
/// Campaign start previews, inbox and commands. Authority refusals join the
/// existing denial audit; nothing reaches the campaign owner without a grant.
fn campaign_control_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    let Some(controls) = context.campaign_controls else {
        return error_response(503, "campaign_controls_not_configured");
    };
    let Ok(now_ms) = crate::engine::host_time_ms() else {
        return error_response(503, "clock_unavailable");
    };
    let result = controls.response(&crate::campaign_controls::ControlRequest {
        method: &request.method,
        target: &request.target,
        body: &request.body,
        principal: &principal.id,
        role: principal.role,
        now_ms,
    });
    let Some(code) = result.denial else {
        return result.response;
    };
    match context.engine.lock() {
        Ok(mut engine) => {
            match engine.record_denial(&request.method, &request.target, Some(principal), code) {
                Ok(()) => result.response,
                Err(_) => error_response(503, "audit_unavailable"),
            }
        }
        Err(_) => error_response(503, "service_unavailable"),
    }
}

fn error_response(status: u16, code: &str) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: format!("{{\"code\":\"{code}\",\"message\":\"Request could not be admitted\"}}")
            .into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

fn write_response<W: Write>(writer: &mut W, response: ApiResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        304 => "Not Modified",
        503 => "Service Unavailable",
        _ => "Response",
    };
    let custom_cache = response
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("cache-control"));
    let cache = if custom_cache {
        String::new()
    } else {
        "Cache-Control: no-store\r\n".to_string()
    };
    let mut additional = String::new();
    for (name, value) in &response.headers {
        if value.contains(['\r', '\n']) {
            continue;
        }
        additional.push_str(name);
        additional.push_str(": ");
        additional.push_str(value);
        additional.push_str("\r\n");
    }
    let content_length = if response.status == 304 {
        String::new()
    } else {
        format!("Content-Length: {}\r\n", response.body.len())
    };
    write!(
        writer,
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\n{content_length}Connection: close\r\n{cache}{SECURITY_HEADERS}{additional}",
        response.status, response.content_type,
    )?;
    if let Some(name) = response.download_name {
        // The engine supplies a display name, never an HTTP header fragment.
        let safe: String = name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        write!(
            writer,
            "Content-Disposition: attachment; filename=\"{safe}\"\r\n"
        )?;
    }
    writer.write_all(b"\r\n")?;
    writer.write_all(&response.body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> Result<Request, u16> {
        read_request(&mut std::io::Cursor::new(bytes), 1024, 16)
    }

    #[test]
    fn fleet_event_cursor_is_one_exact_identity() {
        let identity = "a".repeat(64);
        assert_eq!(event_after("/api/fleet/events"), Ok(None));
        assert_eq!(
            event_after(&format!("/api/fleet/events?after={identity}")),
            Ok(Some(identity.clone()))
        );
        for target in [
            "/api/fleet/events?after=0".to_string(),
            format!("/api/fleet/events?after={identity}&after={identity}"),
            format!("/api/fleet/events?after={identity}&other=2"),
            "/api/fleet/events?after".to_string(),
            "/api/fleet/events?after=".to_string(),
            "/api/fleet/events?before=1".to_string(),
            format!("/api/fleet/events?after={}", "A".repeat(64)),
            format!("/api/fleet/events?after={}", "a".repeat(63)),
        ] {
            assert_eq!(event_after(&target), Err(()), "{target}");
        }
    }

    // r[verify kiln.web.authentication.scenario.denied]
    #[test]
    fn framing_accepts_one_bounded_json_request_and_rejects_ambiguous_bodies() {
        let request = parse(b"POST /api/projects HTTP/1.1\r\nHost: localhost:8443\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\r\n{}").unwrap_or_else(|_| unreachable!());
        assert_eq!(request.body, b"{}");
        for bytes in [
            b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}".as_slice(),
            b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"GET /api/runs HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer a\r\nAuthorization: Bearer b\r\n\r\n",
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\nx",
            b"GET / HTTP/1.1\r\nHost: localhost\r\n folded: value\r\n\r\n",
            b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nContent-Length: +2\r\n\r\n{}",
            b"GET /api/session HTTP/1.1\r\nHost: localhost\r\nX-Kiln-Principal: tailscale:1\r\nx-kiln-principal: tailscale:2\r\n\r\n",
            b"POST /api/triggers/github HTTP/1.1\r\nHost: localhost\r\nX-GitHub-Delivery: first\r\nX-GitHub-Delivery: second\r\nContent-Length: 2\r\n\r\n{}",
            b"POST /api/triggers/github HTTP/1.1\r\nHost: localhost\r\nX-Hub-Signature-256: first\r\nx-hub-signature-256: second\r\nContent-Length: 2\r\n\r\n{}",
        ] { assert!(parse(bytes).is_err()); }
        assert_eq!(
            parse(b"POST /api/projects HTTP/1.1\r\nHost: localhost\r\nContent-Length: 17\r\n\r\n")
                .err(),
            Some(413)
        );
        assert_eq!(
            read_request(
                &mut std::io::Cursor::new(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                16,
                16
            )
            .err(),
            Some(431)
        );
    }

    // r[verify kiln.web.authentication.scenario.operator] r[verify kiln.web.authentication.scenario.denied]
    // r[verify kiln.web.network.boundaries]
    #[test]
    fn viewer_reads_but_never_mutates_and_writes_require_exact_origin() {
        let viewer = Principal {
            id: "viewer:reader".into(),
            label: "Reader".into(),
            role: crate::engine::Role::Viewer,
            agent: None,
        };
        let operator = Principal {
            role: crate::engine::Role::Operator,
            ..viewer.clone()
        };
        let read = parse(b"GET /api/runs HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            api_admission(&read, "https://localhost", Some(&viewer)),
            Ok(())
        );
        assert_eq!(
            api_admission(&read, "https://localhost", None),
            Err((401, "unauthorized"))
        );
        let mut write = parse(b"POST /api/runs HTTP/1.1\r\nHost: localhost\r\nOrigin: https://localhost\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}").unwrap_or_else(|_| unreachable!());
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&operator)),
            Ok(())
        );
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&viewer)),
            Err((403, "operator_required"))
        );
        write.headers.remove("origin");
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&operator)),
            Err((403, "origin_required"))
        );
        write
            .headers
            .insert("origin".into(), "https://foreign.example".into());
        assert_eq!(
            api_admission(&write, "https://localhost", Some(&operator)),
            Err((403, "origin_required"))
        );
    }

    #[test]
    fn review_collaboration_does_not_grant_ci_operator_or_cross_origin_access() {
        let reviewer = Principal {
            id: "viewer:reviewer".into(),
            label: "Reviewer".into(),
            role: crate::engine::Role::Viewer,
            agent: None,
        };
        let mut request = parse(b"POST /api/code-review/commands HTTP/1.1\r\nHost: localhost\r\nOrigin: https://localhost\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            api_admission(&request, "https://localhost", Some(&reviewer)),
            Ok(())
        );
        for target in [
            "/api/runs",
            "/api/projects",
            "/api/code-review/commands?bypass=1",
            "/api/code-review/commands/extra",
        ] {
            request.target = target.into();
            assert_eq!(
                api_admission(&request, "https://localhost", Some(&reviewer)),
                Err((403, "operator_required"))
            );
        }
        request.target = "/api/code-review/commands".into();
        request
            .headers
            .insert("origin".into(), "https://foreign.example".into());
        assert_eq!(
            api_admission(&request, "https://localhost", Some(&reviewer)),
            Err((403, "origin_required"))
        );
        assert_eq!(
            api_admission(&request, "https://localhost", None),
            Err((401, "unauthorized"))
        );
    }

    #[test]
    fn only_exact_registered_trigger_path_can_enter_signed_ingress() {
        let raw =
            b"POST /api/triggers/github HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n{}";
        let mut request = parse(raw).unwrap_or_else(|_| unreachable!());
        assert_eq!(trigger_id(&request), Some("github"));
        for target in [
            "/api/triggers/",
            "/api/triggers/github/extra",
            "/api/triggers/github?bypass=1",
            "/api/runs",
        ] {
            request.target = target.into();
            assert_eq!(trigger_id(&request), None);
        }
        request.target = "/api/triggers/github".into();
        request.method = "GET".into();
        assert_eq!(trigger_id(&request), None);
        request
            .headers
            .insert("x-kiln-role".into(), "operator".into());
        assert!(forbidden_identity(&request));
    }

    // r[verify kiln.web.multiplexed_sse_events]
    #[test]
    fn event_hub_parses_channels_fans_out_to_five_subscribers_and_prunes_drops() {
        assert_eq!(parse_channels("/api/events"), Ok([true, true, true, true]));
        assert_eq!(
            parse_channels("/api/events?channels=ci,attention"),
            Ok([true, false, false, true])
        );
        assert_eq!(parse_channels("/api/events?channels="), Err(()));
        assert_eq!(parse_channels("/api/events?channels=bogus"), Err(()));
        assert_eq!(parse_channels("/api/events?other=1"), Err(()));

        let hub = EventHub::new();
        let subs: Vec<_> = (0..5)
            .map(|_| hub.subscribe([true, true, true, true]).0)
            .collect();
        assert_eq!(hub.state.lock().unwrap().subscribers.len(), 5);

        assert!(hub.publish_ci(1, b"{\"version\":1,\"runs\":[]}".to_vec()));
        assert!(!hub.publish_ci(1, b"{\"version\":1,\"runs\":[]}".to_vec()));
        assert!(hub.publish_cairn("1:1:0".into(), b"{\"invalidated\":true}".to_vec()));

        for sub in &subs {
            let q = sub.queue.0.lock().unwrap();
            assert_eq!(q.events.len(), 2);
            assert_eq!(q.events[0].event_name(), "ci");
            assert_eq!(q.events[1].event_name(), "cairn");
        }

        // Dropping 4 subscribers immediately prunes them from EventHub.
        let mut iter = subs.into_iter();
        let keeper = iter.next().unwrap();
        drop(iter);
        assert_eq!(hub.state.lock().unwrap().subscribers.len(), 1);
        drop(keeper);
        assert_eq!(hub.state.lock().unwrap().subscribers.len(), 0);
    }

    #[test]
    fn shared_watcher_polls_upstream_fleet_once_regardless_of_subscriber_count()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::atomic::AtomicUsize;

        let dir = tempfile::tempdir()?;
        let repo_dir = dir.path().join("repo");
        git2::Repository::init(&repo_dir)?;
        let op_tok = dir.path().join("op.token");
        let wk_tok = dir.path().join("wk.token");
        std::fs::write(&op_tok, "a".repeat(64))?;
        std::fs::write(&wk_tok, "b".repeat(64))?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&op_tok, std::fs::Permissions::from_mode(0o400))?;
            std::fs::set_permissions(&wk_tok, std::fs::Permissions::from_mode(0o400))?;
        }
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&cert_path, "dummy")?;
        std::fs::write(&key_path, "dummy")?;
        let profile: ServiceProfile = serde_json::from_value(serde_json::json!({
            "schema": "kiln.web-service.v1",
            "service_id": "watcher-test",
            "listen_address": "127.0.0.1:9443",
            "origin": "https://localhost:9443",
            "tls": {"certificate_path": cert_path, "private_key_path": key_path, "min_version": "TLS1.3"},
            "operator_token_file": op_tok,
            "worker_token_file": wk_tok,
            "state_directory": dir.path().join("state"),
            "projects": [{
                "id": "fixture",
                "label": "Fixture",
                "repository_path": repo_dir,
                "reference": "refs/heads/main",
                "source_limits": {"max_entries": 32, "max_path_bytes": 1024, "max_file_bytes": 4096, "max_total_bytes": 16384},
                "workflow": {
                    "socket_path": dir.path().join("lattice.sock"),
                    "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
                    "transport_limits": {"maximum_frame_bytes": 1048576, "maximum_log_bytes": 65536, "maximum_connections": 4, "read_deadline_milliseconds": 1000, "write_deadline_milliseconds": 1000},
                    "adapter_limits": {"maximum_request_bytes": 1048576, "maximum_response_bytes": 1048576},
                    "policy_id": "reviewed-policy",
                    "provider_profile_id": "reviewed-lattice",
                    "platform": "linux-x86_64",
                    "nodes": [{"id": "job", "kind": "test", "effect_class": "test", "adapter_kind": "local", "operation_identity": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
                    "edges": [],
                    "controls": {"cancel": true, "pause": false, "resume": false},
                    "bounds": {"max_nodes": 8, "max_edges": 16, "max_log_references": 32, "max_log_read_bytes": 4096, "max_total_log_bytes": 65536}
                }
            }],
            "bounds": {
                "max_request_bytes": 16384, "max_response_bytes": 1048576, "max_header_bytes": 4096,
                "max_connections": 8, "timeout_seconds": 2, "poll_interval_ms": 10, "max_projects": 2,
                "max_runs": 4, "max_artifact_bytes": 1024, "max_artifacts": 2, "max_run_artifact_bytes": 2048,
                "max_total_artifact_bytes": 4096, "max_log_read_bytes": 4096, "max_requests": 32,
                "max_events_per_run": 8, "max_state_bytes": 1048576
            }
        }))?;
        let capability = profile.projects[0].execution_capabilities(profile.bounds.max_runs)?;
        let engine = Mutex::new(Engine::open_with_ports(
            profile,
            BTreeMap::from([(
                "fixture".to_owned(),
                Box::new(inventory_api::tests::DummyWorkflowPort(capability))
                    as Box<dyn kiln_provider_api::WorkflowPort + Send>,
            )]),
        )?);

        let fleet_listener = TcpListener::bind("127.0.0.1:0")?;
        let fleet_addr = fleet_listener.local_addr()?;
        let version_calls = Arc::new(AtomicUsize::new(0));
        let snapshot_calls = Arc::new(AtomicUsize::new(0));
        let fleet_stop = Arc::new(AtomicBool::new(false));
        let fleet_version = Arc::new(Mutex::new("a".repeat(64)));

        let vc = Arc::clone(&version_calls);
        let sc = Arc::clone(&snapshot_calls);
        let fs = Arc::clone(&fleet_stop);
        let fv = Arc::clone(&fleet_version);
        fleet_listener.set_nonblocking(true)?;
        let mock_handle = std::thread::spawn(move || {
            while !fs.load(Ordering::Acquire) {
                match fleet_listener.accept() {
                    Ok((mut sock, _)) => {
                        let _ = sock.set_nonblocking(false);
                        let mut buf = [0u8; 2048];
                        let n = sock.read(&mut buf).unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let path = req
                            .lines()
                            .next()
                            .and_then(|l| l.split_whitespace().nth(1))
                            .unwrap_or("/");
                        let body = match path {
                            "/v1/version" => {
                                vc.fetch_add(1, Ordering::Relaxed);
                                let v = fv.lock().unwrap().clone();
                                format!("{{\"version\":\"{v}\"}}")
                            }
                            "/v1/health" => {
                                sc.fetch_add(1, Ordering::Relaxed);
                                "{\"api\":\"v1\",\"machine\":\"local\",\"version\":\"0.1.0\"}".to_owned()
                            }
                            "/v1/claims?include=foreign,expired" | "/v1/claims" => {
                                "{\"claims\":[],\"foreign_claims\":[]}".to_owned()
                            }
                            "/v1/work" => "{\"ready_unclaimed\":0,\"ranked\":[],\"blocked\":[]}".to_owned(),
                            "/v1/attempts" => "{\"attempts\":[]}".to_owned(),
                            "/v1/decisions" => "{\"decisions\":[]}".to_owned(),
                            "/v1/machines" => "{\"machines\":[]}".to_owned(),
                            _ => "{\"error\":\"not_found\"}".to_owned(),
                        };
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = sock.write_all(resp.as_bytes());
                        let _ = sock.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        let fleet_client = crate::fleet::Client::new(&crate::fleet::FleetProfile {
            schema: crate::fleet::FLEET_PROFILE_SCHEMA.into(),
            address: fleet_addr.to_string(),
            timeout_ms: 2_000,
            max_response_bytes: 1_048_576,
        })?;

        let hub = EventHub::new();
        let subs: Vec<_> = (0..5)
            .map(|_| hub.subscribe([true, true, true, true]).0)
            .collect();

        // First watcher_step: 1 upstream version check + 1 snapshot fetch -> fanned out to all 5 subscribers.
        watcher_step(&hub, &engine, Some(&fleet_client), None);
        assert_eq!(version_calls.load(Ordering::Relaxed), 1);
        assert_eq!(snapshot_calls.load(Ordering::Relaxed), 1);

        // Second watcher_step with unchanged fleet version: only 1 /v1/version check, 0 snapshot fetches!
        watcher_step(&hub, &engine, Some(&fleet_client), None);
        assert_eq!(version_calls.load(Ordering::Relaxed), 2);
        assert_eq!(snapshot_calls.load(Ordering::Relaxed), 1);

        // Advance fleet version -> 1 new snapshot fetch fanned out to all 5 subscribers.
        *fleet_version.lock().unwrap() = "b".repeat(64);
        watcher_step(&hub, &engine, Some(&fleet_client), None);
        assert_eq!(version_calls.load(Ordering::Relaxed), 3);
        assert_eq!(snapshot_calls.load(Ordering::Relaxed), 2);

        for sub in subs {
            let q = sub.queue.0.lock().unwrap();
            let fleet_events = q
                .events
                .iter()
                .filter(|ev| ev.event_name() == "fleet")
                .count();
            assert_eq!(fleet_events, 2, "each of the 5 tabs receives both fleet transitions");
        }

        fleet_stop.store(true, Ordering::Release);
        let _ = mock_handle.join();
        Ok(())
    }
}
