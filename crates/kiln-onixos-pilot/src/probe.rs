use crate::auth::random_seed;
use crate::{KEY_BYTES, MAX_CONFIG_BYTES, PilotProfile, Result, load_json, now_unix, publish_new};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use kiln_adapter_http_observe::{
    HttpObservationProfile, UCAN_AUTH_KIND, UCAN_OBSERVATION_OPERATION_PROFILE,
    ucan_operation_profile_id, ucan_request_binding,
};
use kiln_observe_core::{
    ObservationRequest, ObservationView, ObservationWindow, ViewRequest, project_observation,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use ucan::{
    AudienceDid, CapabilitySet, Ed25519InMemorySigner, HolderInvocationLimits,
    HolderInvocationNonce, HolderInvocationSigningRequest, OperationBinding, ProofReferences,
    TokenSigner, TokenTimeBounds, VerificationLimits, issue_token_with_signer,
    sign_holder_invocation,
};

const TOKEN_LIFETIME_SECONDS: u64 = 600;
const NETWORK_TIMEOUT_SECONDS: u64 = 15;
const RESPONSE_CHUNK_BYTES: usize = 4096;
const INVOCATION_ABILITY: &str = "observe";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    issuer_seed: String,
    holder_seed: String,
}

#[derive(Debug, Serialize)]
pub struct ProbeReport {
    pub schema: &'static str,
    pub revision: u64,
    pub canonical_run_outcome: kiln_core::domain::Outcome,
    pub run_view_identity: kiln_core::ContentId,
    pub attempt_view_identity: kiln_core::ContentId,
    pub wrong_holder_status: u16,
    pub wrong_target_status: u16,
    pub replay_status: u16,
}

/// Creates private fresh keys and an exact public auth profile from a supplied TLS/core template.
pub fn init(
    template: &Path,
    profile: &PilotProfile,
    journal: &Path,
    directory: &Path,
) -> Result<()> {
    let (record, state) = crate::coordinator::load_replayed(profile, journal)?;
    init_for_run(template, &state, &record.request.request.run_id, directory)
}

/// Initializes isolated observation credentials for an existing canonical run.
pub fn init_for_run(
    template: &Path,
    state: &kiln_core::domain::State,
    run_id: &kiln_core::RunId,
    directory: &Path,
) -> Result<()> {
    let mut http: HttpObservationProfile = load_json(template, MAX_CONFIG_BYTES)?;
    let issuer_seed = random_seed()?;
    let holder_seed = random_seed()?;
    let issuer = Ed25519InMemorySigner::from_seed_bytes(issuer_seed);
    let holder = Ed25519InMemorySigner::from_seed_bytes(holder_seed);
    let issuer_did = issuer
        .issuer()
        .map_err(|error| format!("issuer: {error:?}"))?
        .as_str()
        .to_owned();
    let holder_did = holder
        .issuer()
        .map_err(|error| format!("holder: {error:?}"))?
        .as_str()
        .to_owned();
    http.auth.kind = UCAN_AUTH_KIND.into();
    http.auth.principal = issuer_did.clone();
    http.auth.issuer = issuer_did;
    http.auth.holder_keys = vec![holder_did];
    http.auth.resource = format!("kiln:observe:{}", run_id.as_str());
    http.auth.ability = INVOCATION_ABILITY.into();
    http.auth.operation_profile = UCAN_OBSERVATION_OPERATION_PROFILE.into();
    http.auth.proof_tokens.clear();
    http.auth.revoked_proof_references.clear();
    // Limits remain explicit template policy, never silently widened to fit a credential.
    crate::observation::admit_loopback(&http)?;
    if !state.runs.contains_key(run_id) {
        return Err("replayed run missing".into());
    }
    crate::storage::private_directory(directory)?;
    publish_new(
        &directory.join("credentials.json"),
        &Credentials {
            issuer_seed: URL_SAFE_NO_PAD.encode(issuer_seed),
            holder_seed: URL_SAFE_NO_PAD.encode(holder_seed),
        },
    )?;
    publish_new(&directory.join("http.json"), &http)
}

fn seed(text: &str) -> Result<[u8; KEY_BYTES]> {
    URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| "private seed encoding is invalid")?
        .try_into()
        .map_err(|_| "private seed width is invalid".into())
}
fn token(
    issuer: &Ed25519InMemorySigner,
    holder: &Ed25519InMemorySigner,
    http: &HttpObservationProfile,
    now: u64,
) -> Result<String> {
    let audience = AudienceDid::from(
        holder
            .issuer()
            .map_err(|error| format!("holder: {error:?}"))?,
    );
    let capabilities = CapabilitySet::single(&http.auth.resource, &http.auth.ability)
        .map_err(|error| format!("capability: {error:?}"))?;
    let expires = now
        .checked_add(TOKEN_LIFETIME_SECONDS)
        .ok_or("token expiry overflow")?;
    let bounds =
        TokenTimeBounds::new(now, expires).map_err(|error| format!("time bounds: {error:?}"))?;
    issue_token_with_signer(
        issuer,
        &audience,
        &capabilities,
        &ProofReferences::empty(),
        bounds,
    )
    .map(|token| token.as_str().to_owned())
    .map_err(|error| format!("UCAN issuance: {error:?}"))
}

fn header(
    http: &HttpObservationProfile,
    holder: &Ed25519InMemorySigner,
    token: &str,
    target: &str,
    revision: u64,
) -> Result<String> {
    let binding = ucan_request_binding("GET", target, b"", revision)
        .map_err(|error| format!("UCAN request binding: {error:?}"))?;
    let operation = OperationBinding::new(ucan_operation_profile_id(), binding);
    let nonce = HolderInvocationNonce::from_bytes(random_seed()?);
    let audience = AudienceDid::from(
        holder
            .issuer()
            .map_err(|error| format!("holder: {error:?}"))?,
    );
    let request = HolderInvocationSigningRequest::new(
        token,
        &audience,
        &http.auth.resource,
        &http.auth.ability,
        operation,
        nonce,
    );
    let limits = VerificationLimits::new(
        usize::try_from(http.auth.max_proof_depth).map_err(|error| error.to_string())?,
        usize::try_from(http.auth.max_proof_count).map_err(|error| error.to_string())?,
        usize::try_from(http.auth.max_token_bytes).map_err(|error| error.to_string())?,
    )
    .with_clock_skew_tolerance_seconds(http.auth.max_clock_skew_seconds);
    let holder_limits = HolderInvocationLimits::try_new(
        usize::try_from(http.auth.max_holder_audience_bytes).map_err(|error| error.to_string())?,
        usize::try_from(http.auth.max_holder_resource_bytes).map_err(|error| error.to_string())?,
        usize::try_from(http.auth.max_holder_ability_bytes).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("holder limits: {error:?}"))?;
    let signature = sign_holder_invocation(&request, holder, limits, holder_limits)
        .map_err(|error| format!("holder invocation: {error:?}"))?;
    Ok(format!(
        "UCAN {}.{}.{}",
        URL_SAFE_NO_PAD.encode(token),
        URL_SAFE_NO_PAD.encode(signature.as_bytes()),
        URL_SAFE_NO_PAD.encode(nonce.as_bytes())
    ))
}

fn exchange(
    http: &HttpObservationProfile,
    target: &str,
    authorization: &str,
    expected_view: Option<&ObservationView>,
) -> Result<u16> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut roots = rustls::RootCertStore::empty();
    // Parent supplies a certificate chain suitable as a trust anchor (fresh self-signed is supported).
    for certificate in rustls_pemfile::certs(&mut BufReader::new(
        File::open(&http.tls.certificate_path).map_err(|error| error.to_string())?,
    )) {
        roots
            .add(certificate.map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(http.origin.host.clone())
        .map_err(|error| error.to_string())?;
    let connection =
        rustls::ClientConnection::new(Arc::new(config), name).map_err(|error| error.to_string())?;
    let address = (http.origin.host.as_str(), http.origin.port);
    let tcp = TcpStream::connect(address).map_err(|error| error.to_string())?;
    let timeout = Some(Duration::from_secs(NETWORK_TIMEOUT_SECONDS));
    tcp.set_read_timeout(timeout)
        .map_err(|error| error.to_string())?;
    tcp.set_write_timeout(timeout)
        .map_err(|error| error.to_string())?;
    let request = format!(
        "GET {target} HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: {authorization}\r\nConnection: close\r\n\r\n",
        http.origin.host, http.origin.port
    );
    if request.len() as u64 > http.bounds.max_request_bytes
        || authorization.len() as u64 > http.bounds.max_header_bytes
    {
        return Err("probe invocation exceeds configured request/header bound".into());
    }
    let mut stream = rustls::StreamOwned::new(connection, tcp);
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let maximum = usize::try_from(
        http.content
            .max_view_bytes
            .checked_add(http.bounds.max_header_bytes)
            .ok_or("response bound overflow")?,
    )
    .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; RESPONSE_CHUNK_BYTES];
        let count = stream
            .read(&mut chunk)
            .map_err(|error| format!("TLS response read: {error}"))?;
        if count == 0 {
            return Err("TLS response ended before bounded Content-Length".into());
        }
        bytes.extend_from_slice(chunk.get(..count).ok_or("invalid read count")?);
        if bytes.len() > maximum {
            return Err("TLS response exceeds byte bound".into());
        }
        let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let body_start = split.checked_add(4).ok_or("HTTP header overflow")?;
        let headers = std::str::from_utf8(bytes.get(..split).ok_or("missing HTTP headers")?)
            .map_err(|_| "non-UTF8 HTTP headers")?;
        if headers.len() as u64 > http.bounds.max_header_bytes {
            return Err("response headers exceed bound".into());
        }
        let length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.trim())
            })
            .ok_or("response lacks Content-Length")?
            .parse::<usize>()
            .map_err(|error| error.to_string())?;
        let expected = body_start.checked_add(length).ok_or("HTTP body overflow")?;
        if expected > maximum {
            return Err("response Content-Length exceeds bound".into());
        }
        if bytes.len() < expected {
            continue;
        }
        if bytes.len() != expected {
            return Err("response contains trailing bytes".into());
        }
        let status = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or("response lacks HTTP status")?
            .parse::<u16>()
            .map_err(|error| error.to_string())?;
        if let Some(expected_view) = expected_view {
            verify_view(
                status,
                headers,
                bytes.get(body_start..).ok_or("missing response body")?,
                expected_view,
            )?;
        }
        return Ok(status);
    }
}

fn verify_view(status: u16, headers: &str, body: &[u8], expected: &ObservationView) -> Result<()> {
    if status != 200 {
        return Err(format!("positive UCAN query denied: {status}"));
    }
    let actual: ObservationView =
        serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let header = headers.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("x-kiln-view-identity"))
            .map(|(_, value)| value.trim())
    });
    if &actual != expected || header != Some(expected.identity.as_str()) {
        return Err("actual TLS view differs from replayed canonical identity/outcome".into());
    }
    Ok(())
}
fn denied(status: u16) -> Result<u16> {
    if status != 401 {
        return Err(format!("negative UCAN probe was not denied: {status}"));
    }
    Ok(status)
}

// r[impl kiln.onixos_pilot.observation]
// r[impl kiln.onixos_pilot.failure]
pub fn run(profile: &PilotProfile, journal_path: &Path, directory: &Path) -> Result<ProbeReport> {
    let (journal, state) = crate::coordinator::load_replayed(profile, journal_path)?;
    run_for_attempt(
        &state,
        &journal.request.request.run_id,
        &journal.request.request.attempt_id,
        directory,
    )
}

/// Exercises authenticated views and denial cases against one replayed attempt.
pub fn run_for_attempt(
    state: &kiln_core::domain::State,
    run_id: &kiln_core::RunId,
    attempt_id: &kiln_core::AttemptId,
    directory: &Path,
) -> Result<ProbeReport> {
    let http: HttpObservationProfile = load_json(&directory.join("http.json"), MAX_CONFIG_BYTES)?;
    crate::observation::admit_loopback(&http)?;
    let credentials: Credentials =
        load_json(&directory.join("credentials.json"), MAX_CONFIG_BYTES)?;
    let issuer = Ed25519InMemorySigner::from_seed_bytes(seed(&credentials.issuer_seed)?);
    let holder = Ed25519InMemorySigner::from_seed_bytes(seed(&credentials.holder_seed)?);
    let wrong = Ed25519InMemorySigner::from_seed_bytes(random_seed()?);
    let run_target = format!("/v1/views/runs/{}", run_id.as_str());
    let attempt_target = format!("/v1/views/attempts/{}", attempt_id.as_str());
    let view = |request| {
        project_observation(
            &http.core,
            state,
            &ObservationRequest {
                state_revision: state.revision,
                view: request,
            },
            &ObservationWindow::empty(),
        )
        .map_err(|error| format!("canonical projection: {error:?}"))
    };
    let run_view = view(ViewRequest::RunDetail {
        run_id: run_id.clone(),
    })?;
    let attempt_view = view(ViewRequest::AttemptDetail {
        attempt_id: attempt_id.clone(),
    })?;
    let token = token(&issuer, &holder, &http, now_unix()?)?;
    let authorization = header(&http, &holder, &token, &run_target, state.revision)?;
    exchange(&http, &run_target, &authorization, Some(&run_view))?;
    let replay_status = denied(exchange(&http, &run_target, &authorization, None)?)?;
    let attempt_authorization = header(&http, &holder, &token, &attempt_target, state.revision)?;
    exchange(
        &http,
        &attempt_target,
        &attempt_authorization,
        Some(&attempt_view),
    )?;
    let wrong_target = header(&http, &holder, &token, &run_target, state.revision)?;
    let wrong_target_status = denied(exchange(&http, &attempt_target, &wrong_target, None)?)?;
    let wrong_token = self::token(&issuer, &wrong, &http, now_unix()?)?;
    let wrong_authorization = header(&http, &wrong, &wrong_token, &run_target, state.revision)?;
    let wrong_holder_status = denied(exchange(&http, &run_target, &wrong_authorization, None)?)?;
    let canonical_run_outcome = state
        .runs
        .get(run_id)
        .ok_or("replayed run missing")?
        .outcome;
    Ok(ProbeReport {
        schema: "kiln.onixos-pilot.ucan-probe.v1",
        revision: state.revision,
        canonical_run_outcome,
        run_view_identity: run_view.identity,
        attempt_view_identity: attempt_view.identity,
        wrong_holder_status,
        wrong_target_status,
        replay_status,
    })
}
