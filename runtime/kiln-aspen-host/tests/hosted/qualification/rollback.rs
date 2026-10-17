use super::*;

const INCOMPATIBLE_CALLBACK_TIMEOUT_MILLISECONDS: u64 = 30_001;

// r[verify kiln.aspen_host.production_evidence]
#[test]
#[allow(
    tigerstyle::function_length,
    reason = "the rollback drill keeps both incompatible cohorts and exact prior restoration visible"
)]
fn child_host_explicit_rollback_rejects_mixed_cohort_then_restores_prior_exact_cohort() {
    let mut fixture = ProcessFixture::new(std::time::Instant::now);
    let lattice = FixtureLattice::start(&fixture, false);
    let ingress = fixture.ingress(0);
    let prior_host = fixture.host_executable.clone();
    let prior_host_identity = blake3::hash(
        &std::fs::read(&prior_host).unwrap_or_else(|_| unreachable!("prior host executable bytes")),
    );
    let prior_extension = fixture.paths.extension_executable.clone();
    let prior_extension_bytes =
        std::fs::read(&prior_extension).unwrap_or_else(|_| unreachable!("prior executable cohort"));
    let prior_profile_bytes = std::fs::read(&fixture.paths.runtime_profile)
        .unwrap_or_else(|_| unreachable!("prior runtime profile"));
    let child = fixture.start(1);
    let ack = completed(fixture.client().submit(&fixture.profile, &ingress));
    child.finish();
    let prior_state = state_snapshot(&fixture.paths.workspace);

    // This is an admitted but incompatible profile, not malformed JSON. State
    // identity must reject it before native recovery or provider effects.
    let mut incompatible = profile_document_fixture();
    incompatible.bounds.callback_timeout_millis = INCOMPATIBLE_CALLBACK_TIMEOUT_MILLISECONDS;
    let admitted = incompatible
        .clone()
        .into_runtime_profile()
        .and_then(|profile| admit_profile(&profile))
        .unwrap_or_else(|_| unreachable!("incompatible profile remains individually admissible"));
    assert_ne!(admitted.profile_identity, fixture.profile.profile_identity);
    write_json(&fixture.paths.runtime_profile, &incompatible);
    fixture
        .spawn(1)
        .reject("host state marker differs from the admitted cohort");
    assert_eq!(state_snapshot(&fixture.paths.workspace), prior_state);
    assert!(!fixture.host_socket.exists());

    // Independently reject executable mix-and-match against the old profile and
    // old state; altered bytes must not silently inherit the old cohort identity.
    std::fs::write(&fixture.paths.runtime_profile, &prior_profile_bytes)
        .unwrap_or_else(|_| unreachable!("restore exact prior profile"));
    let candidate_extension = fixture.directory.path().join("candidate-extension");
    std::fs::copy(&prior_extension, &candidate_extension)
        .unwrap_or_else(|_| unreachable!("candidate executable copy"));
    let mut candidate_bytes = prior_extension_bytes.clone();
    candidate_bytes.extend_from_slice(b"qualification-incompatible-cohort");
    std::fs::write(&candidate_extension, candidate_bytes)
        .unwrap_or_else(|_| unreachable!("candidate executable bytes"));
    fixture.paths.extension_executable = candidate_extension;
    fixture
        .spawn(1)
        .reject("host state marker differs from the admitted cohort");
    assert_eq!(state_snapshot(&fixture.paths.workspace), prior_state);
    assert!(!fixture.host_socket.exists());

    // Run the separately admitted candidate with its own fresh state before
    // rolling back. Reject an old-profile downgrade against that candidate state.
    let prior_workspace = fixture.paths.workspace.clone();
    let prior_profile = fixture.profile.clone();
    let candidate_state =
        tempfile::tempdir().unwrap_or_else(|_| unreachable!("candidate state root"));
    fixture.paths.workspace = candidate_state.path().to_path_buf();
    fixture.profile = admitted;
    write_json(&fixture.paths.runtime_profile, &incompatible);
    let candidate_ingress = fixture.ingress(1);
    let candidate_host = fixture.start(1);
    let candidate_ack = completed(
        fixture
            .client()
            .submit(&fixture.profile, &candidate_ingress),
    );
    assert_ne!(candidate_ack.operation_id, ack.operation_id);
    candidate_host.finish();
    let candidate_snapshot = state_snapshot(&fixture.paths.workspace);
    fixture.paths.extension_executable = prior_extension.clone();
    fixture.profile = prior_profile;
    std::fs::write(&fixture.paths.runtime_profile, &prior_profile_bytes)
        .unwrap_or_else(|_| unreachable!("select exact prior profile"));
    fixture
        .spawn(1)
        .reject("host state marker differs from the admitted cohort");
    assert_eq!(state_snapshot(&fixture.paths.workspace), candidate_snapshot);
    assert!(!fixture.host_socket.exists());
    fixture.paths.workspace = prior_workspace;

    // Explicit rollback selects all prior inputs, including the quiescent state
    // cohort; recovery then must replay the original full acknowledgement.
    fixture.host_executable = prior_host;
    assert_eq!(
        blake3::hash(
            &std::fs::read(&fixture.host_executable)
                .unwrap_or_else(|_| unreachable!("restored exact host bytes"))
        ),
        prior_host_identity
    );
    fixture.paths.extension_executable = prior_extension;
    restore_state(&fixture.paths.workspace, &prior_state);
    assert_eq!(
        std::fs::read(&fixture.paths.extension_executable)
            .unwrap_or_else(|_| unreachable!("restored executable bytes")),
        prior_extension_bytes
    );
    assert_eq!(
        std::fs::read(&fixture.paths.runtime_profile)
            .unwrap_or_else(|_| unreachable!("restored profile bytes")),
        prior_profile_bytes
    );
    let rolled_back = fixture.start(RESTART_REQUESTS_PER_OPERATION);
    let mut client = fixture.client();
    assert_eq!(completed(client.submit(&fixture.profile, &ingress)), ack);
    assert_eq!(
        completed(client.reconcile(&fixture.profile, &ingress.operation_id)),
        ack
    );
    rolled_back.finish();
    assert_eq!(
        lattice.finish(),
        ExchangeCounts {
            dispatch: 2,
            recover: 0,
            observe: 2
        }
    );
}
