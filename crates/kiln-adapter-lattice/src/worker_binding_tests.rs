use super::*;

fn id(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn binding(state: &Path) -> TransportBinding {
    TransportBinding::new(
        &id('a'),
        3,
        Path::new("/nix/store/00000000000000000000000000000000-lattice/bin/lattice"),
        &id('b'),
        state,
    )
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn recovery_uses_the_root_bound_at_the_first_launch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root_a = directory.path().join("root-a");
    let root_b = directory.path().join("root-b");
    std::fs::create_dir(&root_a).unwrap_or_else(|_| unreachable!());
    std::fs::create_dir(&root_b).unwrap_or_else(|_| unreachable!());
    let path = TransportBinding::path(directory.path(), &id('a'), 3);
    retain(&path, &binding(&root_a)).unwrap_or_else(|error| unreachable!("{error}"));
    // Repeating the same first launch is idempotent.
    retain(&path, &binding(&root_a)).unwrap_or_else(|error| unreachable!("{error}"));
    // A later profile naming root B can neither rebind nor redirect recovery.
    assert!(retain(&path, &binding(&root_b)).is_err());
    let retained = load(&path).unwrap_or_else(|error| unreachable!("{error}"));
    let launch = recovery_launch(retained, &id('a'), 3, is_directory(&root_a))
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(launch.state_directory, root_a);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .unwrap_or_else(|_| unreachable!())
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o400);
    }
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn a_missing_binding_or_bound_root_never_recovers_in_a_fresh_root() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path().join("root");
    let path = TransportBinding::path(directory.path(), &id('a'), 3);
    // An intent persisted before bindings existed has no binding.
    let absent = load(&path).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(absent, None);
    assert!(recovery_launch(absent, &id('a'), 3, true).is_err());
    retain(&path, &binding(&root)).unwrap_or_else(|error| unreachable!("{error}"));
    // The bound root was never created or was lost: refused, not recreated.
    assert!(!is_directory(&root));
    let retained = load(&path).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(recovery_launch(retained.clone(), &id('a'), 3, is_directory(&root)).is_err());
    assert!(!root.exists());
    // Another attempt or fence never borrows this binding.
    assert!(recovery_launch(retained.clone(), &id('c'), 3, true).is_err());
    assert!(recovery_launch(retained, &id('a'), 4, true).is_err());
    // A symbolic link is not the bound directory.
    #[cfg(unix)]
    {
        let target = directory.path().join("target");
        std::fs::create_dir(&target).unwrap_or_else(|_| unreachable!());
        std::os::unix::fs::symlink(&target, &root).unwrap_or_else(|_| unreachable!());
        assert!(!is_directory(&root));
    }
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn malformed_bindings_are_refused() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("binding.json");
    std::fs::write(
        &path,
        br#"{"schema":"kiln.lattice-transport-binding.v1","attempt":"a","fence":3,"lattice_program":"/p","executable_blake3":"b","state_directory":"/s","extra":1}"#,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(load(&path).is_err());
    std::fs::write(
        &path,
        br#"{"schema":"kiln.lattice-transport-binding.v1","attempt":"a","attempt":"c","fence":3,"lattice_program":"/p","executable_blake3":"b","state_directory":"/s"}"#,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(load(&path).is_err());
    let other = TransportBinding {
        schema: "kiln.other.v1".into(),
        ..binding(directory.path())
    };
    assert!(recovery_launch(Some(other), &id('a'), 3, true).is_err());
}
