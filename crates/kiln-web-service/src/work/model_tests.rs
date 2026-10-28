use super::*;

fn fields(title: &str) -> Fields {
    Fields {
        title: title.into(),
        ..Fields::default()
    }
}

fn create(
    kind: Kind,
    fields: Fields,
    records: &BTreeMap<String, Record>,
) -> Result<Record, String> {
    apply(
        None,
        &Operation {
            actor: "alice".into(),
            recorded_at_ms: 100,
            command: Command {
                request_id: "create-1".into(),
                object_id: None,
                expected_head: None,
                action: Action::Create { kind, fields },
            },
        },
        records,
    )
}

fn insert(records: &mut BTreeMap<String, Record>, id: &str, kind: Kind, fields: Fields) -> Record {
    let mut record = create(kind, fields, records)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    record.id = id.into();
    record.head = format!("head-{id}");
    records.insert(id.into(), record.clone());
    record
}

fn operation(record: &Record, actor: &str, request: &str, at: i64, action: Action) -> Operation {
    Operation {
        actor: actor.into(),
        recorded_at_ms: at,
        command: Command {
            request_id: request.into(),
            object_id: Some(record.id.clone()),
            expected_head: Some(record.head.clone()),
            action,
        },
    }
}

fn edit(
    record: &Record,
    replacement: Fields,
    records: &BTreeMap<String, Record>,
) -> Result<Record, String> {
    apply(
        Some(record),
        &operation(
            record,
            "alice",
            "edit-1",
            200,
            Action::Edit {
                fields: replacement,
            },
        ),
        records,
    )
}

fn view(shared: bool) -> View {
    View {
        filters: BTreeMap::new(),
        sort: "updated".into(),
        group_by: "status".into(),
        layout: "list".into(),
        shared,
    }
}

#[test]
fn every_entity_has_a_real_typed_create_and_invalid_payload_is_rejected() {
    let records = BTreeMap::new();
    for kind in [
        Kind::Issue,
        Kind::Project,
        Kind::Milestone,
        Kind::Cycle,
        Kind::Initiative,
        Kind::Team,
        Kind::Request,
    ] {
        let valid = create(kind, fields("Work"), &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert_eq!(valid.kind, kind);
        assert_eq!(valid.created_by, "alice");
        assert_eq!(valid.fields.status, "backlog");
        let mut invalid = fields("Work");
        invalid.personal = Some(Personal::default());
        assert!(create(kind, invalid, &records).is_err());
    }
    let mut personal = fields("Attention");
    personal.personal = Some(Personal::default());
    assert_eq!(
        create(Kind::Personal, personal, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .owner,
        "alice"
    );
    assert!(create(Kind::Personal, fields("Attention"), &records).is_err());
    let mut saved = fields("My view");
    saved.view = Some(view(false));
    assert_eq!(
        create(Kind::View, saved, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .owner,
        "alice"
    );
    assert!(create(Kind::View, fields("Missing view"), &records).is_err());
}

#[test]
fn text_and_collection_boundaries_are_byte_bounded_and_unique() {
    let records = BTreeMap::new();
    let mut valid = fields(&"x".repeat(MAX_TITLE));
    valid.description = "d".repeat(MAX_DESCRIPTION);
    valid.labels = (0..MAX_LIST).map(|i| format!("label-{i}")).collect();
    valid.priority = 4;
    assert_eq!(
        create(Kind::Issue, valid.clone(), &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields,
        valid
    );
    for invalid in [
        Fields {
            title: "x".repeat(MAX_TITLE + 1),
            ..valid.clone()
        },
        Fields {
            title: " ".into(),
            ..valid.clone()
        },
        Fields {
            title: "bad\0title".into(),
            ..valid.clone()
        },
        Fields {
            description: "d".repeat(MAX_DESCRIPTION + 1),
            ..valid.clone()
        },
        Fields {
            labels: vec!["same".into(), "same".into()],
            ..valid.clone()
        },
        Fields {
            labels: (0..=MAX_LIST).map(|i| i.to_string()).collect(),
            ..valid.clone()
        },
        Fields {
            assignees: vec!["alice".into(), "alice".into()],
            ..valid.clone()
        },
        Fields {
            priority: 5,
            ..valid
        },
    ] {
        assert!(create(Kind::Issue, invalid, &records).is_err());
    }
    // A byte bound remains meaningful for non-ASCII titles.
    assert!(create(Kind::Issue, fields(&"é".repeat(MAX_TITLE)), &records).is_err());
}

#[test]
fn command_identity_head_and_timestamp_admission_preserve_native_identity() {
    let mut records = BTreeMap::new();
    let record = insert(&mut records, "issue-a", Kind::Issue, fields("Original"));
    let op = operation(
        &record,
        "alice",
        "change",
        200,
        Action::Edit {
            fields: fields("Changed"),
        },
    );
    let changed = apply(Some(&record), &op, &records)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(changed.id, record.id);
    assert_eq!(changed.head, record.head);
    assert_eq!(changed.created_at_ms, 100);
    assert_eq!(changed.updated_at_ms, 200);
    for bad in [
        Operation {
            actor: " ".into(),
            ..op.clone()
        },
        Operation {
            recorded_at_ms: 99,
            ..op.clone()
        },
        Operation {
            recorded_at_ms: MAX_TIMESTAMP + 1,
            ..op.clone()
        },
        Operation {
            command: Command {
                expected_head: None,
                ..op.command.clone()
            },
            ..op.clone()
        },
        Operation {
            command: Command {
                expected_head: Some("stale".into()),
                ..op.command.clone()
            },
            ..op.clone()
        },
        Operation {
            command: Command {
                object_id: Some("other".into()),
                ..op.command.clone()
            },
            ..op.clone()
        },
        Operation {
            command: Command {
                request_id: "bad id".into(),
                ..op.command.clone()
            },
            ..op.clone()
        },
    ] {
        assert!(apply(Some(&record), &bad, &records).is_err());
    }
    assert!(apply(None, &op, &records).is_err());
    let fresh = create(Kind::Issue, fields("Fresh"), &records)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!((fresh.id.as_str(), fresh.head.as_str()), ("", ""));
    let mut invalid_create = op;
    invalid_create.command.action = Action::Create {
        kind: Kind::Issue,
        fields: fields("Bad"),
    };
    assert!(apply(None, &invalid_create, &records).is_err());
    assert!(apply(Some(&record), &invalid_create, &records).is_err());
}

#[test]
fn planning_references_are_typed_and_scope_changes_cannot_break_consumers() {
    let mut records = BTreeMap::new();
    insert(&mut records, "team", Kind::Team, fields("Team"));
    insert(
        &mut records,
        "initiative",
        Kind::Initiative,
        fields("Outcome"),
    );
    let project_fields = Fields {
        team: Some("team".into()),
        initiative: Some("initiative".into()),
        ..fields("Project")
    };
    insert(&mut records, "project", Kind::Project, project_fields);
    insert(
        &mut records,
        "other-project",
        Kind::Project,
        fields("Other"),
    );
    let milestone = insert(
        &mut records,
        "milestone",
        Kind::Milestone,
        Fields {
            project: Some("project".into()),
            team: Some("team".into()),
            ..fields("Milestone")
        },
    );
    insert(
        &mut records,
        "cycle",
        Kind::Cycle,
        Fields {
            project: Some("project".into()),
            team: Some("team".into()),
            ..fields("Cycle")
        },
    );
    let issue_fields = Fields {
        team: Some("team".into()),
        project: Some("project".into()),
        milestone: Some("milestone".into()),
        cycle: Some("cycle".into()),
        initiative: Some("initiative".into()),
        start_at_ms: Some(100),
        due_at_ms: Some(500),
        ..fields("Scoped issue")
    };
    let issue = insert(&mut records, "issue", Kind::Issue, issue_fields.clone());
    assert_eq!(issue.fields, issue_fields);
    for invalid in [
        Fields {
            project: Some("team".into()),
            ..issue_fields.clone()
        },
        Fields {
            team: Some("missing".into()),
            ..issue_fields.clone()
        },
        Fields {
            project: Some("other-project".into()),
            ..issue_fields.clone()
        },
        Fields {
            start_at_ms: Some(501),
            ..issue_fields.clone()
        },
        Fields {
            due_at_ms: Some(-1),
            ..issue_fields.clone()
        },
    ] {
        assert!(edit(&issue, invalid, &records).is_err());
    }
    let mut moved = milestone.fields.clone();
    moved.project = Some("other-project".into());
    assert!(edit(&milestone, moved, &records).is_err());
    let mut renamed = milestone.fields.clone();
    renamed.title = "Renamed milestone".into();
    assert_eq!(
        edit(&milestone, renamed, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .title,
        "Renamed milestone"
    );
}

#[test]
fn parent_blocker_and_duplicate_graphs_accept_dags_and_reject_cycles() {
    let mut records = BTreeMap::new();
    let root = insert(&mut records, "root", Kind::Issue, fields("Root"));
    let child = insert(
        &mut records,
        "child",
        Kind::Issue,
        Fields {
            parent: Some("root".into()),
            ..fields("Child")
        },
    );
    let leaf = insert(
        &mut records,
        "leaf",
        Kind::Issue,
        Fields {
            parent: Some("child".into()),
            ..fields("Leaf")
        },
    );
    assert_eq!(leaf.fields.parent.as_deref(), Some("child"));
    assert!(
        edit(
            &root,
            Fields {
                parent: Some("leaf".into()),
                ..root.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        edit(
            &child,
            Fields {
                parent: Some("child".into()),
                ..child.fields.clone()
            },
            &records
        )
        .is_err()
    );
    insert(&mut records, "project", Kind::Project, fields("Project"));
    assert!(
        edit(
            &child,
            Fields {
                parent: Some("project".into()),
                ..child.fields.clone()
            },
            &records
        )
        .is_err()
    );

    let blocked = edit(
        &child,
        Fields {
            blocked_by: vec!["root".into()],
            ..child.fields.clone()
        },
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    records.insert(blocked.id.clone(), blocked);
    assert!(
        edit(
            &root,
            Fields {
                blocked_by: vec!["child".into()],
                ..root.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        edit(
            &root,
            Fields {
                blocked_by: vec!["root".into()],
                ..root.fields.clone()
            },
            &records
        )
        .is_err()
    );

    let duplicate = edit(
        &child,
        Fields {
            status: "duplicate".into(),
            duplicate_of: Some("root".into()),
            ..child.fields.clone()
        },
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    records.insert(duplicate.id.clone(), duplicate);
    assert!(
        edit(
            &root,
            Fields {
                status: "duplicate".into(),
                duplicate_of: Some("child".into()),
                ..root.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        edit(
            &root,
            Fields {
                duplicate_of: Some("child".into()),
                ..root.fields.clone()
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn workflow_extensions_are_explicit_and_cannot_be_removed_while_used() {
    let mut records = BTreeMap::new();
    let team = insert(
        &mut records,
        "team",
        Kind::Team,
        Fields {
            workflow: vec!["reviewing".into()],
            ..fields("Team")
        },
    );
    let issue_fields = Fields {
        team: Some("team".into()),
        status: "reviewing".into(),
        ..fields("Review")
    };
    insert(&mut records, "issue", Kind::Issue, issue_fields.clone());
    assert!(
        create(
            Kind::Issue,
            Fields {
                team: None,
                ..issue_fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        create(
            Kind::Issue,
            Fields {
                status: "unknown".into(),
                ..issue_fields
            },
            &records
        )
        .is_err()
    );
    assert!(
        create(
            Kind::Issue,
            Fields {
                workflow: vec!["reviewing".into()],
                ..fields("Bad workflow")
            },
            &records
        )
        .is_err()
    );
    assert!(
        edit(
            &team,
            Fields {
                workflow: Vec::new(),
                ..team.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert_eq!(
        edit(
            &team,
            Fields {
                workflow: vec!["reviewing".into(), "ready".into()],
                ..team.fields.clone()
            },
            &records
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .fields
        .workflow,
        vec!["reviewing", "ready"]
    );
}

#[test]
fn requests_retain_customer_source_and_require_real_snooze_and_duplicate_targets() {
    let mut records = BTreeMap::new();
    insert(&mut records, "canonical", Kind::Issue, fields("Canonical"));
    let request = insert(
        &mut records,
        "request",
        Kind::Request,
        Fields {
            status: "triage".into(),
            customer: Some("Customer".into()),
            source_url: Some("https://example.test/request/1".into()),
            ..fields("Intake")
        },
    );
    let snoozed = edit(
        &request,
        Fields {
            status: "snoozed".into(),
            due_at_ms: Some(500),
            ..request.fields.clone()
        },
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(snoozed.fields.customer, request.fields.customer);
    assert_eq!(snoozed.fields.source_url, request.fields.source_url);
    for changed_intake in [
        Fields {
            customer: None,
            ..request.fields.clone()
        },
        Fields {
            customer: Some("Other customer".into()),
            ..request.fields.clone()
        },
        Fields {
            source_url: None,
            ..request.fields.clone()
        },
        Fields {
            source_url: Some("https://example.test/replaced".into()),
            ..request.fields.clone()
        },
    ] {
        assert!(edit(&request, changed_intake, &records).is_err());
    }
    assert!(
        edit(
            &request,
            Fields {
                status: "snoozed".into(),
                due_at_ms: None,
                ..request.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        edit(
            &request,
            Fields {
                status: "snoozed".into(),
                due_at_ms: Some(200),
                ..request.fields.clone()
            },
            &records
        )
        .is_err()
    );
    // An elapsed reminder/snooze does not prevent unrelated later discussion.
    let later = operation(
        &snoozed,
        "alice",
        "after-due",
        600,
        Action::Comment {
            body: "Still waiting".into(),
            mentions: vec![],
        },
    );
    assert_eq!(
        apply(Some(&snoozed), &later, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .body,
        "Still waiting"
    );
    let duplicate = edit(
        &request,
        Fields {
            status: "duplicate".into(),
            duplicate_of: Some("canonical".into()),
            ..request.fields.clone()
        },
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(duplicate.fields.duplicate_of.as_deref(), Some("canonical"));
    assert!(
        edit(
            &request,
            Fields {
                status: "duplicate".into(),
                ..request.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        create(
            Kind::Request,
            Fields {
                source_url: Some("javascript:alert(1)".into()),
                ..fields("Bad source")
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn discussion_preserves_authorship_mentions_and_bounded_reaction_sets() {
    let mut records = BTreeMap::new();
    let record = insert(&mut records, "team", Kind::Team, fields("Team"));
    let op = operation(
        &record,
        "alice",
        "comment-1",
        200,
        Action::Comment {
            body: "Hello\nworld".into(),
            mentions: vec!["bob".into()],
        },
    );
    let commented = apply(Some(&record), &op, &records)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let message = &commented
        .comments
        .first()
        .unwrap_or_else(|| unreachable!("fixture member absent"));
    assert_eq!(message.id, "message:comment-1");
    assert_eq!(message.author, "alice");
    assert_eq!(message.mentions, vec!["bob"]);
    assert_eq!(message.recorded_at_ms, 200);
    let edit_action = Action::EditComment {
        comment: message.id.clone(),
        body: "Corrected".into(),
        mentions: vec![],
    };
    let denied = operation(&commented, "bob", "edit-comment", 300, edit_action.clone());
    assert!(apply(Some(&commented), &denied, &records).is_err());
    let allowed = operation(&commented, "alice", "edit-comment", 300, edit_action);
    let corrected = apply(Some(&commented), &allowed, &records)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        corrected
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .body,
        "Corrected"
    );
    assert_eq!(
        corrected
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .edited_at_ms,
        Some(300)
    );
    assert_eq!(
        corrected
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .recorded_at_ms,
        200
    );
    let reaction = Action::React {
        comment: message.id.clone(),
        emoji: "👍".into(),
        active: true,
    };
    let reacted = apply(
        Some(&corrected),
        &operation(&corrected, "bob", "react", 400, reaction.clone()),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repeated = apply(
        Some(&reacted),
        &operation(&reacted, "bob", "react-again", 500, reaction),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        repeated
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .reactions
            .get("👍")
            .unwrap_or_else(|| unreachable!("fixture reaction absent")),
        &vec!["bob"]
    );
    let removed = apply(
        Some(&repeated),
        &operation(
            &repeated,
            "bob",
            "unreact",
            600,
            Action::React {
                comment: message.id.clone(),
                emoji: "👍".into(),
                active: false,
            },
        ),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(
        !removed
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .reactions
            .contains_key("👍")
    );
    assert!(
        apply(
            Some(&record),
            &operation(
                &record,
                "alice",
                "bad",
                200,
                Action::Comment {
                    body: " ".into(),
                    mentions: vec![],
                }
            ),
            &records
        )
        .is_err()
    );
    assert!(
        apply(
            Some(&record),
            &operation(
                &record,
                "alice",
                "bad",
                200,
                Action::Comment {
                    body: "Message".into(),
                    mentions: vec!["bob".into(), "bob".into()],
                }
            ),
            &records
        )
        .is_err()
    );
    assert!(
        apply(
            Some(&corrected),
            &operation(
                &corrected,
                "alice",
                "bad",
                400,
                Action::React {
                    comment: "missing".into(),
                    emoji: "👍".into(),
                    active: true,
                }
            ),
            &records
        )
        .is_err()
    );
}

#[test]
fn full_field_edit_preserves_discussion_and_update_messages_support_authorship() {
    let mut records = BTreeMap::new();
    let record = insert(&mut records, "project", Kind::Project, fields("Project"));
    let updated = apply(
        Some(&record),
        &operation(
            &record,
            "alice",
            "update",
            150,
            Action::Update {
                body: "Weekly update".into(),
                mentions: vec!["bob".into()],
            },
        ),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let changed = edit(
        &updated,
        Fields {
            description: "Revised description".into(),
            ..updated.fields.clone()
        },
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(changed.updates, updated.updates);
    assert_eq!(changed.created_by, record.created_by);
    let edited = apply(
        Some(&changed),
        &operation(
            &changed,
            "alice",
            "edit-update",
            300,
            Action::EditComment {
                comment: updated
                    .updates
                    .first()
                    .unwrap_or_else(|| unreachable!("fixture member absent"))
                    .id
                    .clone(),
                body: "Corrected update".into(),
                mentions: vec![],
            },
        ),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        edited
            .updates
            .first()
            .unwrap_or_else(|| unreachable!("fixture update absent"))
            .body,
        "Corrected update"
    );
    assert!(
        edit(
            &changed,
            Fields {
                title: String::new(),
                ..changed.fields.clone()
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn private_ownership_cannot_be_erased_transferred_or_published_by_full_field_edit() {
    let mut records = BTreeMap::new();
    let private = insert(
        &mut records,
        "private",
        Kind::View,
        Fields {
            view: Some(view(false)),
            ..fields("Private view")
        },
    );
    assert!(visible(&private, "alice"));
    assert!(!visible(&private, "bob"));
    assert!(!visible(&private, ""));
    for replacement in [
        Fields {
            owner: String::new(),
            ..private.fields.clone()
        },
        Fields {
            owner: "bob".into(),
            ..private.fields.clone()
        },
        Fields {
            view: Some(view(true)),
            ..private.fields.clone()
        },
    ] {
        assert!(edit(&private, replacement, &records).is_err());
    }
    let alice = operation(
        &private,
        "alice",
        "rename",
        200,
        Action::Edit {
            fields: Fields {
                title: "Renamed private".into(),
                ..private.fields.clone()
            },
        },
    );
    assert_eq!(
        apply(Some(&private), &alice, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .title,
        "Renamed private"
    );
    assert!(
        apply(
            Some(&private),
            &Operation {
                actor: "bob".into(),
                ..alice
            },
            &records
        )
        .is_err()
    );
    assert!(
        create(
            Kind::View,
            Fields {
                owner: "bob".into(),
                view: Some(view(false)),
                ..fields("Forged")
            },
            &records
        )
        .is_err()
    );

    let shared = insert(
        &mut records,
        "shared",
        Kind::View,
        Fields {
            view: Some(view(true)),
            ..fields("Shared")
        },
    );
    assert!(visible(&shared, "bob"));
    let op = operation(
        &shared,
        "bob",
        "shared-edit",
        200,
        Action::Edit {
            fields: Fields {
                title: "Collaborative edit".into(),
                ..shared.fields.clone()
            },
        },
    );
    assert_eq!(
        apply(Some(&shared), &op, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .title,
        "Collaborative edit"
    );
    assert!(
        apply(
            Some(&shared),
            &Operation {
                command: Command {
                    action: Action::Edit {
                        fields: Fields {
                            owner: "bob".into(),
                            ..shared.fields.clone()
                        }
                    },
                    ..op.command.clone()
                },
                ..op
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn personal_state_is_owner_scoped_and_read_markers_bind_observed_heads() {
    let mut records = BTreeMap::new();
    let issue = insert(&mut records, "issue", Kind::Issue, fields("Issue"));
    let personal_payload = Personal {
        watched: vec!["issue".into()],
        favorites: vec!["issue".into()],
        read: BTreeMap::from([("issue".into(), issue.head.clone())]),
        reminders: BTreeMap::from([("issue".into(), 500)]),
    };
    let personal = insert(
        &mut records,
        "personal",
        Kind::Personal,
        Fields {
            personal: Some(personal_payload.clone()),
            ..fields("My attention")
        },
    );
    assert_eq!(personal.fields.personal, Some(personal_payload.clone()));
    assert!(visible(&personal, "alice"));
    assert!(!visible(&personal, "bob"));
    let forbidden = operation(
        &personal,
        "bob",
        "snoop",
        200,
        Action::Comment {
            body: "Snoop".into(),
            mentions: vec![],
        },
    );
    assert!(apply(Some(&personal), &forbidden, &records).is_err());
    let mut bad_head = personal_payload.clone();
    bad_head.read.insert("issue".into(), "not-observed".into());
    assert!(
        edit(
            &personal,
            Fields {
                personal: Some(bad_head),
                ..personal.fields.clone()
            },
            &records
        )
        .is_err()
    );
    let mut bad_reminder = personal_payload.clone();
    bad_reminder.reminders.insert("issue".into(), 200);
    assert!(
        edit(
            &personal,
            Fields {
                personal: Some(bad_reminder),
                ..personal.fields.clone()
            },
            &records
        )
        .is_err()
    );
    let mut bad_reference = personal_payload;
    bad_reference.favorites.push("missing".into());
    assert!(
        edit(
            &personal,
            Fields {
                personal: Some(bad_reference),
                ..personal.fields.clone()
            },
            &records
        )
        .is_err()
    );
    let mut new_issue = issue;
    new_issue.head = "new-native-head".into();
    records.insert("issue".into(), new_issue);
    // An old read marker is meaningful unread state, not a stale write error;
    // elapsed reminders remain visible until the owner clears them.
    let op = operation(
        &personal,
        "alice",
        "rename-after-due",
        600,
        Action::Edit {
            fields: Fields {
                title: "My renamed attention".into(),
                ..personal.fields.clone()
            },
        },
    );
    assert_eq!(
        apply(Some(&personal), &op, &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .personal,
        personal.fields.personal
    );
    assert!(
        edit(
            &personal,
            Fields {
                owner: String::new(),
                ..personal.fields.clone()
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn personal_references_cannot_cross_principal_privacy() {
    let mut records = BTreeMap::new();
    let private = insert(
        &mut records,
        "private",
        Kind::View,
        Fields {
            view: Some(view(false)),
            ..fields("Alice only")
        },
    );
    let personal = Fields {
        personal: Some(Personal {
            favorites: vec![private.id],
            ..Personal::default()
        }),
        ..fields("Attention")
    };
    assert_eq!(
        create(Kind::Personal, personal.clone(), &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .owner,
        "alice"
    );
    let bob = Operation {
        actor: "bob".into(),
        recorded_at_ms: 100,
        command: Command {
            request_id: "bob-create".into(),
            object_id: None,
            expected_head: None,
            action: Action::Create {
                kind: Kind::Personal,
                fields: personal,
            },
        },
    };
    assert!(apply(None, &bob, &records).is_err());
    assert!(
        create(
            Kind::Issue,
            Fields {
                blocked_by: vec!["private".into()],
                ..fields("Public leak")
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn view_filters_layout_sort_and_grouping_are_validated_as_a_whole() {
    let mut records = BTreeMap::new();
    insert(
        &mut records,
        "team",
        Kind::Team,
        Fields {
            workflow: vec!["reviewing".into()],
            ..fields("Team")
        },
    );
    insert(&mut records, "project", Kind::Project, fields("Project"));
    let valid = View {
        filters: BTreeMap::from([
            ("kind".into(), "issue".into()),
            ("team".into(), "team".into()),
            ("project".into(), "project".into()),
            ("status".into(), "reviewing".into()),
            ("assignee".into(), "alice".into()),
            ("priority".into(), "4".into()),
            ("label".into(), "customer".into()),
            ("q".into(), "billing".into()),
        ]),
        layout: "board".into(),
        sort: "priority".into(),
        group_by: "assignee".into(),
        shared: true,
    };
    assert_eq!(
        create(
            Kind::View,
            Fields {
                view: Some(valid.clone()),
                ..fields("Saved")
            },
            &records
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .fields
        .view,
        Some(valid.clone())
    );
    for invalid in [
        View {
            layout: "calendar".into(),
            ..valid.clone()
        },
        View {
            sort: "random".into(),
            ..valid.clone()
        },
        View {
            group_by: "secret".into(),
            ..valid.clone()
        },
        View {
            filters: BTreeMap::from([("unknown".into(), "x".into())]),
            ..valid.clone()
        },
        View {
            filters: BTreeMap::from([("priority".into(), "5".into())]),
            ..valid.clone()
        },
        View {
            filters: BTreeMap::from([("kind".into(), "run".into())]),
            ..valid.clone()
        },
        View {
            filters: BTreeMap::from([("status".into(), "reviewing".into())]),
            ..valid.clone()
        },
        View {
            filters: BTreeMap::from([("team".into(), "project".into())]),
            ..valid.clone()
        },
        View {
            filters: BTreeMap::from([("project".into(), "missing".into())]),
            ..valid
        },
    ] {
        assert!(
            create(
                Kind::View,
                Fields {
                    view: Some(invalid),
                    ..fields("Invalid saved")
                },
                &records
            )
            .is_err()
        );
    }
}

#[test]
fn saved_view_workflow_dependency_prevents_invalidating_a_custom_status_filter() {
    let mut records = BTreeMap::new();
    let team = insert(
        &mut records,
        "team",
        Kind::Team,
        Fields {
            workflow: vec!["reviewing".into()],
            ..fields("Team")
        },
    );
    insert(
        &mut records,
        "view",
        Kind::View,
        Fields {
            view: Some(View {
                filters: BTreeMap::from([
                    ("team".into(), "team".into()),
                    ("status".into(), "reviewing".into()),
                ]),
                ..view(false)
            }),
            ..fields("Review queue")
        },
    );
    assert!(
        edit(
            &team,
            Fields {
                workflow: vec![],
                ..team.fields.clone()
            },
            &records
        )
        .is_err()
    );
    assert_eq!(
        edit(
            &team,
            Fields {
                title: "Renamed".into(),
                ..team.fields.clone()
            },
            &records
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .fields
        .title,
        "Renamed"
    );
}

#[test]
fn inert_identity_links_accept_supported_kinds_without_granting_authority() {
    let records = BTreeMap::new();
    let links: Vec<_> = [
        "source", "revision", "patch", "review", "run", "artifact", "cairn",
    ]
    .into_iter()
    .map(|kind| Link {
        kind: kind.into(),
        id: format!("{kind}:opaque"),
    })
    .collect();
    let valid = Fields {
        links: links.clone(),
        ..fields("Linked change")
    };
    assert_eq!(
        create(Kind::Issue, valid.clone(), &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .links,
        links
    );
    assert!(
        create(
            Kind::Issue,
            Fields {
                links: vec![Link {
                    kind: "publish".into(),
                    id: "anything".into()
                }],
                ..valid.clone()
            },
            &records
        )
        .is_err()
    );
    assert!(
        create(
            Kind::Issue,
            Fields {
                links: vec![
                    valid
                        .links
                        .first()
                        .unwrap_or_else(|| unreachable!("fixture link absent"))
                        .clone(),
                    valid
                        .links
                        .first()
                        .unwrap_or_else(|| unreachable!("fixture link absent"))
                        .clone()
                ],
                ..valid
            },
            &records
        )
        .is_err()
    );
}

#[test]
fn history_reduction_is_deterministic_and_shell_controls_real_head() {
    let mut records = BTreeMap::new();
    let initial = insert(
        &mut records,
        "native-object",
        Kind::Issue,
        fields("Initial"),
    );
    let operations = [
        operation(
            &initial,
            "alice",
            "comment-request",
            200,
            Action::Comment {
                body: "Persisted".into(),
                mentions: vec!["bob".into()],
            },
        ),
        Operation {
            actor: "bob".into(),
            recorded_at_ms: 300,
            command: Command {
                request_id: "reaction-request".into(),
                object_id: Some(initial.id.clone()),
                expected_head: Some("native-head-2".into()),
                action: Action::React {
                    comment: "message:comment-request".into(),
                    emoji: "👍".into(),
                    active: true,
                },
            },
        },
    ];
    let replay = || {
        let mut state = initial.clone();
        for (index, operation) in operations.iter().enumerate() {
            state = apply(Some(&state), operation, &records)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            state.head = format!("native-head-{}", index + 2);
        }
        state
    };
    let first = replay();
    let reopened = replay();
    assert_eq!(first, reopened);
    assert_eq!(first.head, "native-head-3");
    assert_eq!(
        first
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .reactions
            .get("👍")
            .unwrap_or_else(|| unreachable!("fixture reaction absent")),
        &vec!["bob"]
    );
    assert!(
        apply(
            Some(&first),
            operations
                .first()
                .unwrap_or_else(|| unreachable!("fixture operation absent")),
            &records
        )
        .is_err()
    );
    // Exact request replay is served by the shell's signed request index;
    // bypassing it must not append the same message twice even with a new head.
    let duplicate = Operation {
        command: Command {
            expected_head: Some(first.head.clone()),
            ..operations
                .first()
                .unwrap_or_else(|| unreachable!("fixture member absent"))
                .command
                .clone()
        },
        recorded_at_ms: 400,
        ..operations
            .first()
            .unwrap_or_else(|| unreachable!("fixture member absent"))
            .clone()
    };
    assert!(apply(Some(&first), &duplicate, &records).is_err());
}

#[test]
fn wire_rejects_unknown_fields_actions_and_invalid_numeric_ranges() {
    let valid = r#"{"request_id":"r","object_id":null,"expected_head":null,"action":{"type":"create","kind":"issue","fields":{"title":"Issue"}}}"#;
    let command: Command = serde_json::from_str(valid)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        command.action,
        Action::Create {
            kind: Kind::Issue,
            fields: fields("Issue")
        }
    );
    for invalid in [
        r#"{"request_id":"r","object_id":null,"expected_head":null,"admin":true,"action":{"type":"create","kind":"issue","fields":{"title":"Issue"}}}"#,
        r#"{"request_id":"r","action":{"type":"delete"}}"#,
        r#"{"request_id":"r","action":{"type":"create","kind":"issue","fields":{"title":"Issue","priority":-1}}}"#,
        r#"{"request_id":"r","action":{"type":"create","kind":"issue","fields":{"title":"Issue","secret":true}}}"#,
        r#"{"request_id":"r","action":{"type":"comment","body":"Text","mentions":[],"author":"bob"}}"#,
    ] {
        assert!(serde_json::from_str::<Command>(invalid).is_err());
    }
}

#[test]
fn discussion_body_and_stream_limits_accept_the_boundary_and_reject_overflow() {
    let mut records = BTreeMap::new();
    let record = insert(&mut records, "issue", Kind::Issue, fields("Issue"));
    let boundary = operation(
        &record,
        "alice",
        "boundary",
        200,
        Action::Comment {
            body: "x".repeat(MAX_BODY),
            mentions: (0..MAX_LIST).map(|i| format!("user-{i}")).collect(),
        },
    );
    let admitted = apply(Some(&record), &boundary, &records)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        admitted
            .comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .body,
        "x".repeat(MAX_BODY)
    );
    let overflow = Operation {
        command: Command {
            action: Action::Comment {
                body: "x".repeat(MAX_BODY + 1),
                mentions: vec![],
            },
            ..boundary.command.clone()
        },
        ..boundary.clone()
    };
    assert!(apply(Some(&record), &overflow, &records).is_err());
    let excess_mentions = Operation {
        command: Command {
            action: Action::Comment {
                body: "Mention everyone".into(),
                mentions: (0..=MAX_LIST).map(|i| format!("user-{i}")).collect(),
            },
            ..boundary.command.clone()
        },
        ..boundary
    };
    assert!(apply(Some(&record), &excess_mentions, &records).is_err());
    let mut almost_full = record;
    almost_full.comments = (0..MAX_MESSAGES - 1)
        .map(|i| Message {
            id: format!("message:old-{i}"),
            author: "alice".into(),
            body: format!("Message {i}"),
            mentions: vec![],
            reactions: BTreeMap::new(),
            recorded_at_ms: 100,
            edited_at_ms: None,
        })
        .collect();
    let full = apply(
        Some(&almost_full),
        &operation(
            &almost_full,
            "alice",
            "last",
            200,
            Action::Comment {
                body: "Last admitted".into(),
                mentions: vec![],
            },
        ),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        full.comments
            .last()
            .unwrap_or_else(|| unreachable!("last admitted message"))
            .body,
        "Last admitted"
    );
    assert!(
        apply(
            Some(&full),
            &operation(
                &full,
                "alice",
                "overflow",
                300,
                Action::Comment {
                    body: "Rejected".into(),
                    mentions: vec![]
                }
            ),
            &records
        )
        .is_err()
    );
}

#[test]
fn personal_collections_and_reaction_actors_enforce_their_independent_bounds() {
    let mut records = BTreeMap::new();
    for i in 0..=MAX_PERSONAL {
        insert(
            &mut records,
            &format!("issue-{i}"),
            Kind::Issue,
            fields("Issue"),
        );
    }
    let personal = Personal {
        watched: (0..MAX_PERSONAL).map(|i| format!("issue-{i}")).collect(),
        ..Personal::default()
    };
    let valid = Fields {
        personal: Some(personal.clone()),
        ..fields("Attention")
    };
    assert_eq!(
        create(Kind::Personal, valid.clone(), &records)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .fields
            .personal,
        Some(personal.clone())
    );
    let mut excess = personal;
    excess.watched.push(format!("issue-{MAX_PERSONAL}"));
    assert!(
        create(
            Kind::Personal,
            Fields {
                personal: Some(excess),
                ..valid
            },
            &records
        )
        .is_err()
    );

    let record = records
        .get("issue-0")
        .unwrap_or_else(|| unreachable!("fixture issue absent"));
    let mut almost_full = record.clone();
    almost_full.comments.push(Message {
        id: "message:reactions".into(),
        author: "alice".into(),
        body: "React here".into(),
        mentions: vec![],
        reactions: BTreeMap::from([(
            "👍".into(),
            (0..MAX_LIST - 1).map(|i| format!("actor-{i}")).collect(),
        )]),
        recorded_at_ms: 100,
        edited_at_ms: None,
    });
    let action = Action::React {
        comment: "message:reactions".into(),
        emoji: "👍".into(),
        active: true,
    };
    let full = apply(
        Some(&almost_full),
        &operation(&almost_full, "last-actor", "last", 200, action.clone()),
        &records,
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(
        full.comments
            .first()
            .unwrap_or_else(|| unreachable!("fixture comment absent"))
            .reactions
            .get("👍")
            .unwrap_or_else(|| unreachable!("fixture reaction absent"))
            .iter()
            .any(|actor| actor == "last-actor")
    );
    assert!(
        apply(
            Some(&full),
            &operation(&full, "overflow-actor", "excess", 300, action),
            &records
        )
        .is_err()
    );
}

#[test]
fn blocker_diamonds_are_not_cycles_but_a_back_edge_is_rejected() {
    let mut records = BTreeMap::new();
    let root = insert(&mut records, "root", Kind::Issue, fields("Root"));
    for id in ["left", "right"] {
        insert(
            &mut records,
            id,
            Kind::Issue,
            Fields {
                blocked_by: vec!["root".into()],
                ..fields(id)
            },
        );
    }
    let top = insert(
        &mut records,
        "top",
        Kind::Issue,
        Fields {
            blocked_by: vec!["left".into(), "right".into()],
            ..fields("Diamond")
        },
    );
    assert_eq!(top.fields.blocked_by, vec!["left", "right"]);
    assert!(
        edit(
            &root,
            Fields {
                blocked_by: vec!["top".into()],
                ..root.fields.clone()
            },
            &records
        )
        .is_err()
    );
}
