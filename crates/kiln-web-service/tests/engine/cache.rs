use super::*;
use kiln_web_service::cache::{
    CacheEntry, CacheEnvelope, CacheFile, CacheKey, CacheProfile, CacheProject,
};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn install(engine: &mut Engine, root: &std::path::Path) {
    let directory = root.join("shared-cache");
    checked(std::fs::create_dir(&directory));
    checked(std::fs::set_permissions(
        &directory,
        std::fs::Permissions::from_mode(0o700),
    ));
    checked(engine.install_cache(CacheProfile {
        schema: "kiln.web-cache.v1".into(),
        directory,
        maximum_entry_bytes: 4096,
        maximum_total_bytes: 16384,
        maximum_entries: 8,
        maximum_age_seconds: 3600,
        maximum_files: 16,
        projects: vec![CacheProject {
            project_id: "fixture".into(),
            producers: vec!["producer-v1".into()],
            toolchains: vec!["toolchain-v1".into()],
            platforms: vec!["linux-x86_64".into()],
            paths: vec!["cache-output".into()],
        }],
    }));
}
fn cache_key() -> CacheKey {
    CacheKey {
        project_id: "fixture".into(),
        node_id: "job".into(),
        producer: "producer-v1".into(),
        toolchain: "toolchain-v1".into(),
        platform: "linux-x86_64".into(),
        inputs: BTreeMap::from([(
            PathBuf::from("source.txt"),
            blake3::hash(b"source").to_hex().to_string(),
        )]),
        paths: vec!["cache-output".into()],
    }
}
fn request(run: &Value) -> Value {
    json!({"attempt_id":field(run,"attempt_id"),"execution":field(run,"execution"),"source_revision":field(run,"source_revision"),"node_id":"job","key":cache_key(),"entry":null,"observed_success":false})
}
#[test]
fn authenticated_exact_worker_cache_is_cold_then_warm_and_rejects_stale_or_failed_producers() {
    let (root, mut profile, _) = fixture();
    profile.bounds.max_request_bytes = 1_048_576;
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    install(&mut engine, root.path());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "cache-start"));
    let id = checked(field(&run, "id").as_str().ok_or("run id"));
    let restore = format!("/api/runs/{id}/cache/restore");
    let publish = format!("/api/runs/{id}/cache/publish");
    let body = request(&run);
    for role in [Role::Viewer, Role::Operator] {
        assert_eq!(
            call(&mut engine, "POST", &restore, body.clone(), role).status,
            403
        );
        assert_eq!(
            call(&mut engine, "POST", &publish, body.clone(), role).status,
            403
        );
    }
    let cold = call(&mut engine, "POST", &restore, body.clone(), Role::Worker);
    assert_eq!(cold.status, 200);
    assert!(field(&value(&cold), "entry").is_null());
    let bytes = b"real shared cached data";
    let entry = checked(CacheEnvelope::new(CacheEntry {
        key: cache_key(),
        files: vec![CacheFile {
            path: "cache-output/data".into(),
            blake3: blake3::hash(bytes).to_hex().to_string(),
            body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        }],
    }));
    let mut upload = body.clone();
    set(
        &mut upload,
        "entry",
        checked(serde_json::to_value(entry.clone())),
    );
    assert_eq!(
        call(&mut engine, "POST", &publish, upload.clone(), Role::Worker).status,
        409
    );
    set(&mut upload, "observed_success", json!(true));
    assert_eq!(
        call(&mut engine, "POST", &publish, upload.clone(), Role::Worker).status,
        201
    );
    let warm = call(&mut engine, "POST", &restore, body.clone(), Role::Worker);
    assert_eq!(warm.status, 200);
    assert_eq!(
        field(field(&value(&warm), "entry"), "blake3"),
        &json!(entry.blake3)
    );
    let mut stale = body.clone();
    set(&mut stale, "source_revision", json!("wrong"));
    assert_eq!(
        call(&mut engine, "POST", &restore, stale, Role::Worker).status,
        409
    );
    let mut wrong_scope = body.clone();
    *checked(
        wrong_scope
            .pointer_mut("/key/project_id")
            .ok_or("cache project"),
    ) = json!("another-project");
    assert_eq!(
        call(&mut engine, "POST", &restore, wrong_scope, Role::Worker).status,
        403
    );
    backend.lock().cancelled = true;
    assert_eq!(
        call(&mut engine, "POST", &publish, upload, Role::Worker).status,
        409
    );
}
