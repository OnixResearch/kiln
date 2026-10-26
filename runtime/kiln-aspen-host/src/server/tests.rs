use super::*;

const SHARED_DIRECTORY_MODES: [u32; 2] = [0o750, 0o707];
const PERMISSION_BITS: u32 = 0o777;

// r[verify kiln.aspen_host.transport.accepted]
#[test]
fn private_socket_grants_owner_access_only() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("private directory"));
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|_| unreachable!("private directory permissions"));
    let socket_path = directory.path().join("host.sock");
    let listener = bind_socket(&socket_path).unwrap_or_else(|_| unreachable!("private socket"));
    let metadata =
        std::fs::metadata(&socket_path).unwrap_or_else(|_| unreachable!("socket metadata"));
    assert_eq!(metadata.permissions().mode() & PERMISSION_BITS, SOCKET_MODE);
    let client = std::os::unix::net::UnixStream::connect(&socket_path)
        .unwrap_or_else(|_| unreachable!("owner connects"));
    let (accepted, _) = listener
        .accept()
        .unwrap_or_else(|_| unreachable!("owner accepted"));
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
    drop(accepted);
    drop(client);
}

// r[verify kiln.aspen_host.transport.rejected]
#[test]
fn shared_parent_is_denied_before_socket_creation() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let socket_path = directory.path().join("host.sock");
    for mode in SHARED_DIRECTORY_MODES {
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(mode))
            .unwrap_or_else(|_| unreachable!("shared parent permissions"));
        assert!(matches!(
            bind_socket(&socket_path),
            Err(HostBridgeError::Admission(_))
        ));
        assert!(!socket_path.exists());
    }
}

// r[verify kiln.aspen_host.transport.rejected]
#[test]
fn symlinked_parent_is_denied_before_socket_creation() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("private directory"));
    let linked_parent = directory.path().join("alias");
    std::os::unix::fs::symlink(directory.path(), &linked_parent)
        .unwrap_or_else(|_| unreachable!("parent symlink"));
    let socket_path = linked_parent.join("host.sock");
    assert!(matches!(
        bind_socket(&socket_path),
        Err(HostBridgeError::Admission(_))
    ));
    assert!(!directory.path().join("host.sock").exists());
}

// r[verify kiln.aspen_host.transport.rejected]
#[test]
fn private_parent_owned_by_another_user_is_denied() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("private directory"));
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|_| unreachable!("private directory permissions"));
    let metadata = std::fs::symlink_metadata(directory.path())
        .unwrap_or_else(|_| unreachable!("directory metadata"));
    let other_user = metadata.uid().wrapping_add(1);
    assert!(matches!(
        admit_socket_directory(&metadata, other_user),
        Err(HostBridgeError::Admission(_))
    ));
}
