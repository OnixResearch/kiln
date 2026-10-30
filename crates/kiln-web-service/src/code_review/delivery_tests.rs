use super::*;
use serde_json::json;
use std::io::{BufRead, Read, Write};
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn principals() -> Vec<String> {
    ["alice", "bob", "carol"].map(String::from).to_vec()
}
fn anchor() -> domain::Anchor {
    domain::Anchor {
        revision: "r1".into(),
        base_commit: "base".into(),
        file: "src/lib.rs".into(),
        side: domain::Side::Revision,
        start_line: 1,
        end_line: 1,
    }
}
fn review() -> domain::ReviewState {
    let mut state = domain::ReviewState::default();
    state.changes.insert(
        "change".into(),
        domain::Change {
            id: "change".into(),
            project: "fixture".into(),
            repository: "repo".into(),
            target_ref: "refs/heads/main".into(),
            owner: "alice".into(),
            title: "Review".into(),
            description: "Public description".into(),
            state: domain::ChangeStatus::Open,
            wip: false,
            revisions: vec![domain::Revision {
                id: "r1".into(),
                number: 1,
                commit: "commit".into(),
                base: "base".into(),
                files: vec!["src/lib.rs".into()],
                author: "alice".into(),
                created_ms: 1,
                authorship: domain::Authorship::Undeclared,
            }],
            reviewers: BTreeMap::new(),
            votes: Vec::new(),
            threads: Vec::new(),
            reviewed: Vec::new(),
            attention: BTreeSet::new(),
            dependencies: Vec::new(),
            candidate: None,
            submission: None,
            maintenance: Vec::new(),
        },
    );
    state
}
fn profile(port: u16) -> Result<Profile, serde_json::Error> {
    serde_json::from_value(json!({
        "schema":"kiln.review-delivery-profile.v1", "max_records":100,
        "max_attempts":2, "retry_delay_ms":1000, "max_tick_effects":1,
        "destinations":[{"id":"review","project_id":"fixture","enabled":true,
            "on_success":false,"on_failure":false,"host":"localhost","port":port,"path":"/review",
            "addresses":["127.0.0.1"],"bearer_secret_file":null,"ca_certificate_file":null,
            "allow_loopback":true,"allow_private":false,"timeout_ms":2000,
            "max_request_bytes":8192,"max_response_bytes":128,"max_resolver_candidates":1}],
        "recipients":[{"principal":"bob","destination":"review","address":"bob@example.test","channel":"email_adapter"},
                      {"principal":"carol","destination":"review","address":"carol-hook","channel":"webhook"}]
    }))
}
fn published(before: &domain::ReviewState, text: &str, id: &str) -> domain::ReviewState {
    let mut after = before.clone();
    if let Some(change) = after.changes.get_mut("change") {
        change.threads.push(domain::Thread {
            id: id.into(),
            anchor: anchor(),
            blocking: false,
            resolved: false,
            comments: vec![domain::Comment {
                id: id.into(),
                author: "alice".into(),
                body: text.into(),
                created_ms: 10,
            }],
        });
    }
    after
}
fn plan(state: &mut State, profile: &Profile, text: &str) -> Result<(), String> {
    let before = review();
    after_transition(
        &before,
        &published(&before, text, "comment"),
        state,
        Some(profile),
        &principals(),
        "alice",
        &domain::Facts {
            event_id: "publication".into(),
            now_ms: 10,
            ..domain::Facts::default()
        },
    )
}

#[test]
fn private_draft_never_routes_then_publication_routes_only_admitted_mentions() -> TestResult {
    let mut public = review();
    let before_draft = public.clone();
    let mut facts = domain::Facts {
        event_id: "save".into(),
        now_ms: 2,
        anchors: vec![anchor()],
        ..domain::Facts::default()
    };
    domain::apply(
        &mut public,
        "alice",
        domain::Command::SaveDraft {
            change: "change".into(),
            expected_revision: "r1".into(),
            draft: domain::Draft {
                id: "draft".into(),
                change: "change".into(),
                author: "alice".into(),
                anchor: anchor(),
                thread: None,
                body: "SECRET-DRAFT @bob @mallory @carol-extra".into(),
                blocking: false,
            },
        },
        &facts,
    )
    .map_err(|error| error.to_string())?;
    let mut delivery = State::default();
    let profile = profile(443)?;
    after_transition(
        &before_draft,
        &public,
        &mut delivery,
        Some(&profile),
        &principals(),
        "alice",
        &facts,
    )?;
    assert!(delivery.inbox.is_empty());
    assert!(delivery.outbox.is_empty());
    let before_publish = public.clone();
    facts.event_id = "publish".into();
    facts.now_ms = 3;
    domain::apply(
        &mut public,
        "alice",
        domain::Command::Publish {
            change: "change".into(),
            expected_revision: "r1".into(),
            draft_ids: vec!["draft".into()],
            vote: None,
        },
        &facts,
    )
    .map_err(|error| error.to_string())?;
    after_transition(
        &before_publish,
        &public,
        &mut delivery,
        Some(&profile),
        &principals(),
        "alice",
        &facts,
    )?;
    assert_eq!(
        inbox(&delivery, "bob")
            .first()
            .ok_or("missing bob inbox")?
            .kind,
        "mentioned"
    );
    assert!(inbox(&delivery, "carol").is_empty());
    assert!(inbox(&delivery, "mallory").is_empty());
    assert_eq!(delivery.outbox.keys().count(), 1);
    let body = String::from_utf8(
        delivery
            .outbox
            .values()
            .next()
            .ok_or("missing delivery")?
            .body
            .clone(),
    )?;
    assert!(!body.contains("SECRET-DRAFT"));
    assert!(!body.contains("@bob"));
    Ok(())
}

#[test]
fn owned_subscription_unsubscribe_dedup_and_read_are_independent() -> TestResult {
    let before = review();
    let mut state = State::default();
    subscribe(&mut state, &before, "carol", "change", true, &principals())?;
    assert!(
        subscribe(
            &mut state,
            &before,
            "mallory",
            "change",
            true,
            &principals()
        )
        .is_err()
    );
    assert!(subscribe(&mut state, &before, "bob", "missing", true, &principals()).is_err());
    let after = published(&before, "@bob please review", "one");
    let profile = profile(443)?;
    let facts = domain::Facts {
        event_id: "one".into(),
        now_ms: 10,
        ..domain::Facts::default()
    };
    after_transition(
        &before,
        &after,
        &mut state,
        Some(&profile),
        &principals(),
        "alice",
        &facts,
    )?;
    after_transition(
        &before,
        &after,
        &mut state,
        Some(&profile),
        &principals(),
        "alice",
        &facts,
    )?;
    assert_eq!(
        inbox(&state, "bob")
            .first()
            .ok_or("missing bob inbox")?
            .kind,
        "mentioned"
    );
    assert_eq!(
        inbox(&state, "carol")
            .first()
            .ok_or("missing carol inbox")?
            .kind,
        "subscribed_update"
    );
    assert_eq!(state.outbox.keys().count(), 2);
    let bob_outcomes = outcomes(&state, "bob", "change");
    assert_eq!(
        bob_outcomes
            .iter()
            .map(|row| row
                .pointer("/notification/kind")
                .and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("mentioned")]
    );
    let encoded = serde_json::to_string(&bob_outcomes)?;
    assert!(
        !encoded.contains("bob@example.test")
            && !encoded.contains("carol-hook")
            && !encoded.contains("destination_identity")
    );
    assert!(outcomes(&state, "mallory", "change").is_empty());
    assert!(outcomes(&state, "bob", "other-change").is_empty());
    assert!(set_read(&mut state, "carol", "one", true));
    assert!(
        !inbox(&state, "bob")
            .first()
            .ok_or("missing bob inbox")?
            .read
    );
    subscribe(&mut state, &after, "carol", "change", false, &principals())?;
    let next = published(&after, "No mentions", "two");
    after_transition(
        &after,
        &next,
        &mut state,
        Some(&profile),
        &principals(),
        "alice",
        &domain::Facts {
            event_id: "two".into(),
            now_ms: 20,
            ..domain::Facts::default()
        },
    )?;
    assert_eq!(
        inbox(&state, "carol")
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        vec!["one"]
    );
    assert!(!subscribed(&state, "carol", "change"));
    Ok(())
}

#[test]
fn bounds_fail_atomically_and_destination_changes_fail_closed() -> TestResult {
    let mut config = profile(443)?;
    let mut state = State::default();
    config.max_records = 1;
    assert!(plan(&mut state, &config, "@bob @carol").is_err());
    assert!(state.inbox.is_empty() && state.outbox.is_empty());
    plan(&mut state, &config, "@bob")?;
    config
        .destinations
        .first_mut()
        .ok_or("missing destination")?
        .path = "/different".into();
    assert!(reserve(&config, &mut state, 10)?.is_empty());
    assert_eq!(
        state.outbox.values().next().ok_or("missing row")?.status,
        Status::DestinationDenied
    );
    config
        .destinations
        .first_mut()
        .ok_or("missing destination")?
        .allow_loopback = false;
    assert!(config.admit(&principals()).is_err());
    config
        .destinations
        .first_mut()
        .ok_or("missing destination")?
        .allow_loopback = true;
    config
        .destinations
        .first_mut()
        .ok_or("missing destination")?
        .host = "localhost@evil.example".into();
    assert!(config.admit(&principals()).is_err());
    Ok(())
}

#[test]
fn restart_retains_identity_marks_lost_reservation_uncertain_and_rejects_stale_result() -> TestResult
{
    let config = profile(443)?;
    let mut state = State::default();
    plan(&mut state, &config, "@bob")?;
    let work = reserve(&config, &mut state, 10)?
        .pop()
        .ok_or("missing reservation")?;
    let mut restarted: State = serde_json::from_slice(&serde_json::to_vec(&state)?)?;
    let identity = restarted
        .outbox
        .keys()
        .next()
        .ok_or("missing identity")?
        .clone();
    assert!(reserve(&config, &mut restarted, 20)?.is_empty());
    let row = restarted.outbox.get(&identity).ok_or("missing row")?;
    assert_eq!(row.status, Status::Uncertain);
    assert_eq!(
        row.attempts.first().ok_or("missing first attempt")?.id,
        work.attempt
    );
    let stale = Completion {
        id: work.id,
        attempt: work.attempt,
        destination_identity: work.destination_identity,
        status: Status::Delivered,
        http_status: Some(202),
        completed_ms: 21,
    };
    assert!(complete(&mut restarted, stale).is_err());
    assert_eq!(
        restarted.outbox.get(&identity).ok_or("missing row")?.status,
        Status::Uncertain
    );
    Ok(())
}

#[test]
fn only_proven_presend_failure_retries_with_same_recipient_identity_and_bounded_attempts()
-> TestResult {
    let config = profile(443)?;
    let mut state = State::default();
    plan(&mut state, &config, "@bob")?;
    for now in [10, 1010] {
        let work = reserve(&config, &mut state, now)?
            .pop()
            .ok_or("missing work")?;
        assert_eq!(work.request.idempotency_key.native_id.as_str(), work.id);
        complete(
            &mut state,
            Completion {
                id: work.id,
                attempt: work.attempt,
                destination_identity: work.destination_identity,
                status: Status::Retryable,
                http_status: None,
                completed_ms: now + 1,
            },
        )?;
        assert!(reserve(&config, &mut state, now + 1)?.is_empty());
    }
    assert!(reserve(&config, &mut state, 2010)?.is_empty());
    let row = state.outbox.values().next().ok_or("missing row")?;
    assert_eq!(row.status, Status::Exhausted);
    assert_ne!(
        row.attempts.first().ok_or("missing first attempt")?.id,
        row.attempts.get(1).ok_or("missing second attempt")?.id
    );
    assert_eq!(
        transport_status(&TransportResult::TimedOut {
            progress: TransportProgress::RequestSent
        }),
        Status::Uncertain
    );
    assert_eq!(
        transport_status(&TransportResult::Disconnected {
            progress: TransportProgress::Connected
        }),
        Status::Uncertain
    );
    assert_eq!(
        transport_status(&TransportResult::TimedOut {
            progress: TransportProgress::BeforeConnect
        }),
        Status::Retryable
    );
    Ok(())
}

/// Real configured TLS receiver, using the same pinned CA/hostname/IP transport
/// as production. Only the test profile explicitly admits loopback.
#[test]
fn configured_https_receiver_accepts_published_mention_and_subscription_after_durable_restart()
-> TestResult {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs = rustls_pemfile::certs(&mut std::io::Cursor::new(include_bytes!(
        "../../../../fixtures/http-observation/tls/cert.pem"
    )))
    .collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(include_bytes!(
        "../../../../fixtures/http-observation/tls/key.pem"
    )))?
    .ok_or("missing TLS key")?;
    let tls = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?,
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let receiver = std::thread::spawn(move || -> Result<Vec<serde_json::Value>, String> {
        let mut received = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while received.len() < 2 {
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(error) => return Err(error.to_string()),
                }
            };
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .map_err(|e| e.to_string())?;
            socket
                .set_write_timeout(Some(std::time::Duration::from_secs(3)))
                .map_err(|e| e.to_string())?;
            let mut stream = rustls::StreamOwned::new(
                rustls::ServerConnection::new(tls.clone()).map_err(|e| e.to_string())?,
                socket,
            );
            let mut reader = std::io::BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).map_err(|e| e.to_string())?;
            if line != "POST /review HTTP/1.1\r\n" {
                return Err("unexpected target".into());
            }
            let mut length = None;
            let mut key = None;
            loop {
                line.clear();
                if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                    return Err("truncated HTTP".into());
                }
                if line == "\r\n" {
                    break;
                }
                let (name, value) = line.split_once(':').ok_or("bad header")?;
                if name.eq_ignore_ascii_case("content-length") {
                    length = Some(value.trim().parse::<usize>().map_err(|e| e.to_string())?);
                }
                if name.eq_ignore_ascii_case("idempotency-key") {
                    key = Some(value.trim().to_owned());
                }
            }
            let length = length.ok_or("missing length")?;
            if length > 8192 {
                return Err("body exceeded bound".into());
            }
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
            let body: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if body.get("id").and_then(serde_json::Value::as_str) != key.as_deref() {
                return Err("idempotency mismatch".into());
            }
            drop(reader);
            stream
                .write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .map_err(|e| e.to_string())?;
            stream.flush().map_err(|e| e.to_string())?;
            received.push(body);
        }
        Ok(received)
    });
    let dir = tempfile::tempdir()?;
    let ca = dir.path().join("ca.pem");
    std::fs::write(
        &ca,
        include_bytes!("../../../../fixtures/http-observation/tls/ca-cert.pem"),
    )?;
    let mut config = profile(port)?;
    config
        .destinations
        .first_mut()
        .ok_or("missing destination")?
        .ca_certificate_file = Some(ca);
    let mut state = State::default();
    subscribe(
        &mut state,
        &review(),
        "carol",
        "change",
        true,
        &principals(),
    )?;
    plan(&mut state, &config, "PUBLIC-COMMENT @bob")?;
    let database = dir.path().join("outbox.sqlite");
    let connection = rusqlite::Connection::open(&database)?;
    connection.execute_batch("CREATE TABLE state (id INTEGER PRIMARY KEY, body BLOB NOT NULL)")?;
    connection.execute(
        "INSERT INTO state VALUES (1, ?1)",
        [serde_json::to_vec(&state)?],
    )?;
    drop(connection);
    let connection = rusqlite::Connection::open(&database)?;
    let bytes: Vec<u8> =
        connection.query_row("SELECT body FROM state WHERE id = 1", [], |r| r.get(0))?;
    let mut state: State = serde_json::from_slice(&bytes)?;
    for now in [10, 11] {
        let work = reserve(&config, &mut state, now)?
            .pop()
            .ok_or("missing external work")?;
        connection.execute(
            "UPDATE state SET body = ?1 WHERE id = 1",
            [serde_json::to_vec(&state)?],
        )?;
        complete(&mut state, work.deliver()?)?;
        connection.execute(
            "UPDATE state SET body = ?1 WHERE id = 1",
            [serde_json::to_vec(&state)?],
        )?;
    }
    let received = receiver.join().map_err(|_| "receiver panicked")??;
    let by_principal: BTreeMap<_, _> = received
        .iter()
        .map(|r| (r["principal"].as_str().unwrap_or_default(), r))
        .collect();
    let bob = by_principal.get("bob").ok_or("missing bob delivery")?;
    let carol = by_principal.get("carol").ok_or("missing carol delivery")?;
    assert_eq!(
        bob.pointer("/notification/kind")
            .and_then(serde_json::Value::as_str),
        Some("mentioned")
    );
    assert_eq!(bob["channel"], "email_adapter");
    assert_eq!(bob["address"], "bob@example.test");
    assert_eq!(
        carol
            .pointer("/notification/kind")
            .and_then(serde_json::Value::as_str),
        Some("subscribed_update")
    );
    assert!(!serde_json::to_string(&received)?.contains("PUBLIC-COMMENT"));
    assert!(state.outbox.values().all(|r| {
        r.status == Status::Delivered
            && r.attempts
                .first()
                .is_some_and(|a| a.http_status == Some(202))
    }));
    assert!(reserve(&config, &mut state, 12)?.is_empty());
    Ok(())
}
