use super::*;
use kiln_web_service::engine::extend_projects;

fn appended(previous: &ServiceProfile) -> ServiceProfile {
    let mut next = previous.clone();
    let mut project = checked(next.projects.first().ok_or("project")).clone();
    project.id = "additional".into();
    project.label = "Additional project".into();
    next.projects.push(project);
    next.bounds.max_projects += 1;
    next
}

fn stored(profile: &ServiceProfile) -> (String, Vec<u8>) {
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    checked(connection.query_row(
        "SELECT profile,state FROM service_state WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ))
}

fn open_registry(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Engine {
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    for project in &profile.projects {
        ports.insert(
            project.id.clone(),
            Box::new(Port {
                profile: checked(project.lattice(profile.bounds.max_runs)).provider,
                backend: backend.clone(),
            }),
        );
    }
    checked(Engine::open_with_ports(profile, ports))
}

// r[verify kiln.web.projects.extension]
#[test]
fn project_extension_preserves_history_logs_replay_and_explicit_enrollment() {
    let (_root, previous, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(previous.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "retained"));
    let path = log_path(&run);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 0, b"kept", false),
            Role::Worker
        )
        .status,
        201
    );
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    poll(&mut engine);
    let run_path = format!(
        "/api/runs/{}",
        checked(field(&run, "id").as_str().ok_or("run"))
    );
    let before = value(&get(&mut engine, &run_path, Role::Viewer));
    let retained = stored_log(&previous);
    let retained_bytes = checked(std::fs::read(&retained));
    drop(engine);
    let (old_binding, old_state) = stored(&previous);
    let next = appended(&previous);
    checked(extend_projects(&previous, &next));
    let (new_binding, new_state) = stored(&next);
    assert_ne!(new_binding, old_binding);
    assert_eq!(new_state, old_state);
    assert_eq!(checked(std::fs::read(&retained)), retained_bytes);
    assert!(extend_projects(&previous, &next).is_err());
    assert_eq!(stored(&next), (new_binding, new_state));
    assert!(try_open(previous, backend.clone()).is_err());
    let mut engine = open_registry(next, backend.clone());
    assert_eq!(value(&get(&mut engine, &run_path, Role::Viewer)), before);
    assert_eq!(
        field(
            &value(&read_capture(&mut engine, &path, "stdout", 0, 4)),
            "text"
        ),
        "kept"
    );
    let projects = value(&get(&mut engine, "/api/projects", Role::Viewer));
    let available = checked(field(&projects, "available").as_array().ok_or("available"));
    assert!(available.iter().any(|p| field(p, "id") == "additional"));
    let enrolled = checked(field(&projects, "enrolled").as_array().ok_or("enrolled"));
    assert_eq!(
        enrolled.iter().map(|p| field(p, "id")).collect::<Vec<_>>(),
        vec![&json!("fixture")]
    );
    assert_eq!(backend.lock().dispatches, 1);
}

#[test]
fn project_extension_rejects_existing_authority_changes_and_invalid_suffix_without_rebinding() {
    let (_root, previous, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    drop(open(previous.clone(), backend));
    let original = stored(&previous);
    for case in 0..16 {
        let mut next = appended(&previous);
        let existing = checked(next.projects.first_mut().ok_or("existing project"));
        match case {
            0 => {
                checked(existing.workflow.nodes.first_mut().ok_or("existing node"))
                    .operation_identity = "b".repeat(64)
            }
            1 => existing.repository_path = previous.state_directory.join("other"),
            2 => existing.reference = "refs/heads/other".into(),
            3 => existing.label = "Changed existing meaning".into(),
            4 => next.operator_token_file = previous.state_directory.join("operator.token"),
            5 => next.worker_token_file = previous.state_directory.join("worker.token"),
            6 => next.tls.certificate_path = "/different/cert.pem".into(),
            7 => next.origin = "https://different.invalid".into(),
            8 => next.service_id = "different".into(),
            9 => next.state_directory = previous.state_directory.join("other"),
            10 => next.bounds.max_runs += 1,
            11 => next.bounds.max_projects = 1,
            12 => checked(next.projects.get_mut(1).ok_or("appended project")).id = "fixture".into(),
            13 => next.projects.swap(0, 1),
            14 => {
                next.projects.remove(0);
            }
            15 => {
                checked(
                    checked(next.projects.get_mut(1).ok_or("appended project"))
                        .workflow
                        .nodes
                        .first_mut()
                        .ok_or("appended node"),
                )
                .adapter_kind = "unsupported".into()
            }
            _ => unreachable!("case"),
        }
        assert!(extend_projects(&previous, &next).is_err(), "case {case}");
        assert_eq!(stored(&previous), original, "case {case}");
    }
}

#[test]
fn project_extension_rejects_live_owner_active_work_and_missing_state() {
    let (_root, previous, _) = fixture();
    let next = appended(&previous);
    assert!(extend_projects(&previous, &next).is_err());
    assert!(!previous.state_directory.exists());
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(previous.clone(), backend.clone());
    let initial = stored(&previous);
    assert!(extend_projects(&previous, &next).is_err());
    assert_eq!(stored(&previous), initial);
    enroll(&mut engine);
    assert_eq!(start(&mut engine, "active").status, 201);
    drop(engine);
    let active = stored(&previous);
    assert!(extend_projects(&previous, &next).is_err());
    assert_eq!(stored(&previous), active);
    let mut engine = open(previous.clone(), backend.clone());
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    poll(&mut engine);
    drop(engine);
    checked(extend_projects(&previous, &next));
    drop(open_registry(next, backend));
}

#[test]
fn project_extension_preserves_binding_on_corruption_and_transaction_failure() {
    for corrupt in [false, true] {
        let (_root, previous, _) = fixture();
        let backend = Arc::new(Mutex::new(Backend::default()));
        drop(open(previous.clone(), backend));
        let connection = checked(rusqlite::Connection::open(
            previous.state_directory.join("service.sqlite"),
        ));
        if corrupt {
            checked(connection.execute(
                "UPDATE service_state SET state=?1 WHERE id=1",
                [b"malformed state".as_slice()],
            ));
        } else {
            checked(connection.execute_batch("CREATE TRIGGER reject_extension BEFORE UPDATE OF profile ON service_state BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;"));
        }
        drop(connection);
        let original = stored(&previous);
        assert!(extend_projects(&previous, &appended(&previous)).is_err());
        assert_eq!(stored(&previous), original);
    }
}
