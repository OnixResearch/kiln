use super::*;

/// `nickel export --format json config/fixtures/tangled-spindle-positive.ncl`.
const EXPORTED_PROFILE: &str = include_str!("../../../../fixtures/tangled/spindle-profile.json");

// r[verify kiln.tangled.profile]
#[test]
fn exported_nickel_profile_is_admitted() {
    let value: TangledProfile = ok(serde_json::from_str(EXPORTED_PROFILE));
    let admitted = ok(admit_profile(value));
    assert_eq!(admitted.profile.limits.maximum_token_lifetime_seconds, 300);
    assert!(admitted.repository(REPO).is_some());
}

/// Expected refusal code and the profile change that causes it.
type Mutation = (&'static str, fn(&mut TangledProfile));

// r[verify kiln.tangled.profile] r[verify kiln.tangled.profile.scenario.invalid]
#[test]
fn reviewed_profile_is_admitted_and_bad_profiles_fail_closed() {
    let admitted = profile();
    assert!(admitted.repository(REPO).is_some());
    assert!(admitted.repository(OTHER_REPO).is_none());
    assert_eq!(
        ok(admit_profile(profile_value())).identity,
        admitted.identity
    );

    let mutations: Vec<Mutation> = vec![
        ("tangled_profile_schema", |p| {
            p.schema = "kiln.tangled-spindle.v0".into()
        }),
        ("tangled_profile_spindle", |p| {
            p.spindle_did = "did:plc:abcdefghijklmnopqrstuvwx".into()
        }),
        ("tangled_profile_plc", |p| {
            p.plc_directory_url = "http://plc.directory".into()
        }),
        ("tangled_profile_path", |p| p.git_program = "git".into()),
        ("tangled_profile_clone_url", |p| {
            if let Some(pin) = p.repositories.first_mut() {
                pin.clone_url = "http://knot.example.com/x".into();
            }
        }),
        ("tangled_profile_knot", |p| {
            if let Some(pin) = p.repositories.first_mut() {
                pin.knot_did = "did:plc:abcdefghijklmnopqrstuvwx".into();
            }
        }),
        ("tangled_profile_actors", |p| {
            if let Some(pin) = p.repositories.first_mut() {
                pin.approvers.clear();
            }
        }),
        ("tangled_profile_repository", |p| {
            let copy = p.repositories.clone();
            p.repositories.extend(copy);
        }),
        ("tangled_profile_repositories", |p| p.repositories.clear()),
        ("tangled_profile_engines", |p| p.engines.clear()),
        ("tangled_profile_workflow", |p| {
            p.workflow.untrusted_policy_id = p.workflow.policy_id.clone()
        }),
        ("tangled_profile_status", |p| {
            p.status.endpoint = "http://status.example.com".into()
        }),
        ("tangled_profile_status", |p| p.status.maximum_attempts = 0),
        ("tangled_profile_limits", |p| p.limits.maximum_pipelines = 0),
        ("tangled_profile_limits", |p| {
            p.limits.maximum_token_lifetime_seconds = 0
        }),
        ("tangled_profile_non_claims", |p| p.non_claims.clear()),
    ];
    for (code, mutate) in mutations {
        let mut value = profile_value();
        mutate(&mut value);
        assert_eq!(err(admit_profile(value)).code, code);
    }
}
