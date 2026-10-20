use crate::{
    KEY_BYTES, MAX_CONFIG_BYTES, OperatorRequest, PilotProfile, REQUEST_SCHEMA, Result,
    SignedRequest, identity, load_json, publish_new,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kiln_adapter_aspen::{IngressSource, build_ingress, hosted_run_id};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const SIGNATURE_DOMAIN: &[u8] = b"kiln.onixos-pilot.operator-request.v1\0";
pub(crate) fn random_seed() -> Result<[u8; KEY_BYTES]> {
    let mut seed = [0; KEY_BYTES];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut seed))
        .map_err(|error| format!("fresh entropy unavailable: {error}"))?;
    Ok(seed)
}
pub fn now_unix() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(|error| error.to_string())
}
pub fn keygen(secret: &Path, public: &Path) -> Result<()> {
    let seed = random_seed()?;
    let key = SigningKey::from_bytes(&seed);
    publish_new(secret, &URL_SAFE_NO_PAD.encode(seed))?;
    publish_new(
        public,
        &URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes()),
    )
}
fn signed_bytes(request: &OperatorRequest) -> Result<Vec<u8>> {
    let mut bytes = SIGNATURE_DOMAIN.to_vec();
    serde_json::to_writer(&mut bytes, request).map_err(|error| error.to_string())?;
    Ok(bytes)
}
pub fn sign(secret: &Path, request: OperatorRequest) -> Result<SignedRequest> {
    let text: String = load_json(secret, MAX_CONFIG_BYTES)?;
    let seed: [u8; KEY_BYTES] = URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| "invalid private key encoding")?
        .try_into()
        .map_err(|_| "invalid private key width")?;
    let key = SigningKey::from_bytes(&seed);
    Ok(SignedRequest {
        signature: URL_SAFE_NO_PAD.encode(key.sign(&signed_bytes(&request)?).to_bytes()),
        public_key: URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes()),
        request,
    })
}
pub fn prepare_request(
    profile: &PilotProfile,
    trigger: kiln_provider_api::TriggerRequest,
    issued_at_unix: u64,
    expires_at_unix: u64,
) -> Result<OperatorRequest> {
    let runtime = profile.admit()?;
    let ingress = build_ingress(&runtime, IngressSource::Operator, trigger.clone())
        .map_err(|error| format!("ingress: {error:?}"))?;
    Ok(OperatorRequest {
        schema: REQUEST_SCHEMA.into(),
        issued_at_unix,
        expires_at_unix,
        cohort_identity: profile.cohort_identity.clone(),
        source_uri: profile.source_uri.clone(),
        source_identity: profile.source_identity.clone(),
        job_identity: profile.job_identity.clone(),
        pilot_profile_identity: identity(profile)?,
        runtime_profile_identity: runtime.profile_identity,
        run_id: hosted_run_id(&ingress.trigger_identity)
            .map_err(|error| format!("run: {error:?}"))?,
        attempt_id: profile.build_plan.attempt_id.clone(),
        trigger,
    })
}

// r[impl kiln.onixos_pilot.trigger]
// r[impl kiln.onixos_pilot.failure]
pub fn authenticate(profile: &PilotProfile, signed: &SignedRequest, now: u64) -> Result<()> {
    profile.admit()?;
    if serde_json::to_vec(signed)
        .map_err(|error| error.to_string())?
        .len()
        > profile.max_request_bytes
    {
        return Err("signed request exceeds bound".into());
    }
    if !profile.allowed_operator_keys.contains(&signed.public_key) {
        return Err("operator key is not explicitly allowlisted".into());
    }
    let public: [u8; KEY_BYTES] = URL_SAFE_NO_PAD
        .decode(&signed.public_key)
        .map_err(|_| "invalid public key encoding")?
        .try_into()
        .map_err(|_| "invalid public key width")?;
    let signature = Signature::from_slice(
        &URL_SAFE_NO_PAD
            .decode(&signed.signature)
            .map_err(|_| "invalid signature encoding")?,
    )
    .map_err(|_| "invalid signature width")?;
    VerifyingKey::from_bytes(&public)
        .map_err(|_| "invalid public key")?
        .verify_strict(&signed_bytes(&signed.request)?, &signature)
        .map_err(|_| "operator signature denied")?;
    let request = &signed.request;
    if request.schema != REQUEST_SCHEMA
        || request.issued_at_unix > now
        || request.expires_at_unix < now
        || request.expires_at_unix <= request.issued_at_unix
        || request.expires_at_unix - request.issued_at_unix > profile.max_request_age_seconds
        || now - request.issued_at_unix > profile.max_request_age_seconds
    {
        return Err("operator request is stale or has an invalid freshness window".into());
    }
    let expected = prepare_request(
        profile,
        request.trigger.clone(),
        request.issued_at_unix,
        request.expires_at_unix,
    )?;
    if &expected != request {
        return Err("operator source, cohort, plan, runtime or run identity drift".into());
    }
    Ok(())
}
