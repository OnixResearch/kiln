use super::*;
use kiln_core::types::ContentId;

fn launch(root: &Path) -> TransportLaunch {
    TransportLaunch {
        lattice_program: "/nix/store/00000000000000000000000000000000-lattice/bin/lattice".into(),
        executable_blake3: ContentId::from_blake3_hex("b".repeat(64))
            .unwrap_or_else(|_| unreachable!()),
        state_directory: root.display().to_string(),
    }
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn recovery_uses_only_the_recorded_launch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path().join("root");
    std::fs::create_dir(&root).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        recovery_launch(Some(launch(&root)), is_directory(&root), true),
        Ok(launch(&root))
    );
    // An intent persisted before launches were recorded has none.
    assert!(recovery_launch(None, true, true).is_err());
    // A changed program is refused.
    assert!(recovery_launch(Some(launch(&root)), true, false).is_err());
}

// r[verify kiln.implementation.lattice_cli.scenario.bound_root]
#[test]
fn a_missing_or_symlinked_root_is_never_recovered() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let root = directory.path().join("root");
    assert!(!is_directory(&root));
    assert!(recovery_launch(Some(launch(&root)), is_directory(&root), true).is_err());
    assert!(!root.exists());
    #[cfg(unix)]
    {
        let target = directory.path().join("target");
        std::fs::create_dir(&target).unwrap_or_else(|_| unreachable!());
        std::os::unix::fs::symlink(&target, &root).unwrap_or_else(|_| unreachable!());
        assert!(!is_directory(&root));
        assert!(recovery_launch(Some(launch(&root)), is_directory(&root), true).is_err());
    }
}
