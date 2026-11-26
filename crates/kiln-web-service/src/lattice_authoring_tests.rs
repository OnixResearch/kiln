use super::*;
use std::fs;
use std::os::unix::fs::symlink;

struct Fixture {
    dir: tempfile::TempDir,
    first: String,
    second: String,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temporary owner catalog");
        let first = Self::export(
            dir.path(),
            "first.author.json",
            'a',
            b"{ enabled = false }\n",
        );
        let second = Self::export(
            dir.path(),
            "second.author.json",
            'b',
            b"{ enabled = true }\n",
        );
        let index = json!({"schema": INDEX_SCHEMA, "exports": [
            {"workflow": "local-demo", "archive": "first.author.json", "blake3": first},
            {"workflow": "local-demo", "archive": "second.author.json", "blake3": second},
        ]});
        fs::write(dir.path().join("index.json"), index.to_string()).expect("write index");
        Self { dir, first, second }
    }

    fn export(root: &Path, filename: &str, revision: char, source: &[u8]) -> String {
        // Native export wire shape, using disposable source. This fixture does
        // not claim to exercise Lattice's evaluator or attest its receipts.
        let artifact = json!({
            "schema": EXPORT_SCHEMA,
            "association": {
                "state": "CapturedModules",
                "source_revision": revision.to_string().repeat(64),
                "expanded_artifact": "c".repeat(64),
                "program": {
                    "kind": "Workflow", "parameters": {}, "entry": "workflow.ncl",
                    "files": {"workflow.ncl": source, "assets/raw.bin": [255]}
                },
                "observation": {"private_opaque_fact": "not returned by the viewer"}
            }
        })
        .to_string();
        fs::write(root.join(filename), &artifact).expect("write export");
        blake3::hash(artifact.as_bytes()).to_hex().to_string()
    }

    fn load(&self) -> Result<SourceCatalog, String> {
        SourceCatalog::load(&self.dir.path().join("index.json"))
    }
}

#[test]
fn exact_export_revision_and_members_are_stable_without_live_head() {
    let fixture = Fixture::new();
    let catalog = fixture.load().expect("pinned archives admitted");
    let listing = response(Some(&catalog), "GET", ROUTE, b"", Role::Operator);
    assert_eq!(listing.status, 200);
    assert!(
        listing
            .headers
            .contains(&("Cache-Control", "no-store".into()))
    );
    let json: serde_json::Value = serde_json::from_slice(&listing.body).expect("source listing");
    assert_eq!(json["revisions"][0]["archive_blake3"], fixture.first);
    assert_eq!(json["revisions"][0]["source_revision"], "a".repeat(64));
    assert_eq!(json["revisions"][1]["archive_blake3"], fixture.second);
    assert_eq!(json["revisions"][1]["source_revision"], "b".repeat(64));
    assert_eq!(
        json["revisions"][0]["members"],
        json!(["assets/raw.bin", "workflow.ncl"])
    );
    assert!(
        !listing
            .body
            .windows(19)
            .any(|bytes| bytes == b"private_opaque_fact")
    );

    for (archive, expected) in [
        (&fixture.first, b"{ enabled = false }\n".as_slice()),
        (&fixture.second, b"{ enabled = true }\n".as_slice()),
    ] {
        let target = format!("{ROUTE}?archive={archive}&member=1");
        let member = response(Some(&catalog), "GET", &target, b"", Role::Operator);
        assert_eq!(member.status, 200);
        assert_eq!(member.content_type, "text/plain; charset=utf-8");
        assert_eq!(member.body, expected);
    }
}

#[test]
fn unavailable_forbidden_mutating_and_inexact_requests_fail_closed() {
    let fixture = Fixture::new();
    let catalog = fixture.load().expect("valid source catalog");
    assert_eq!(
        response(None, "GET", ROUTE, b"", Role::Operator).status,
        503
    );
    for role in [Role::Viewer, Role::Worker] {
        assert_eq!(
            response(Some(&catalog), "GET", ROUTE, b"", role).status,
            403
        );
    }
    let mutation = response(
        Some(&catalog),
        "POST",
        ROUTE,
        br#"{"expected_revision":"stale"}"#,
        Role::Operator,
    );
    assert_eq!(mutation.status, 405);
    assert!(mutation.headers.contains(&("Allow", "GET".into())));
    let get = |target: &str| response(Some(&catalog), "GET", target, b"", Role::Operator).status;
    assert_eq!(
        get(&format!("{ROUTE}?archive={}&member=0", fixture.first)),
        415
    );
    assert_eq!(
        get(&format!("{ROUTE}?archive={}&member=2", fixture.first)),
        404
    );
    assert_eq!(
        get(&format!("{ROUTE}?archive={}&member=1", "f".repeat(64))),
        404
    );
    for member in ["", "01", "64", "1&member=0", "../../workflow.ncl", "%31"] {
        assert_eq!(
            get(&format!(
                "{ROUTE}?archive={}&member={member}",
                fixture.first
            )),
            400
        );
    }
    assert_eq!(
        response(Some(&catalog), "GET", ROUTE, b"{}", Role::Operator).status,
        400
    );
}

#[test]
fn drift_symlinks_and_oversize_inputs_fail_at_admission() {
    let fixture = Fixture::new();
    let first = fixture.dir.path().join("first.author.json");
    let replacement = fixture.dir.path().join("replacement.author.json");
    fs::rename(&first, &replacement).expect("move original archive");
    symlink(&replacement, &first).expect("create archive symlink");
    assert!(fixture.load().is_err());
    fs::remove_file(&first).expect("remove symlink");
    fs::write(&first, "changed bytes").expect("replace pinned archive bytes");
    assert_eq!(
        fixture.load().err().as_deref(),
        Some("Lattice author-source archive pin mismatch")
    );
    let oversized = OpenOptions::new()
        .write(true)
        .open(first)
        .expect("open archive fixture");
    oversized
        .set_len((MAX_ARCHIVE_BYTES + 1) as u64)
        .expect("make oversized sparse archive");
    assert!(fixture.load().is_err());
}
