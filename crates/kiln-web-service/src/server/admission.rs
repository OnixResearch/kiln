use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;

use crate::profile::ServiceProfile;
use crate::worker::files::directory;

// r[impl kiln.web.deploy.runtime]
/// Filesystem admission only: never create a state root or contact an external worker.
pub(super) fn runtime_inputs(profile: &ServiceProfile) -> Result<(), String> {
    state_directory(&profile.state_directory, true)?;
    if profile.state_directory.exists() {
        for name in ["artifacts", "logs"] {
            state_directory(&profile.state_directory.join(name), false)?;
        }
        for name in [
            "service.lock",
            "service.sqlite",
            "service.sqlite-wal",
            "service.sqlite-shm",
        ] {
            state_file(&profile.state_directory.join(name))?;
        }
    }
    for project in &profile.projects {
        directory(&project.repository_path)
            .map_err(|_| "repository directory unavailable or unsafe")?;
        let repository = git2::Repository::open_ext(
            &project.repository_path,
            git2::RepositoryOpenFlags::NO_SEARCH,
            std::iter::empty::<&Path>(),
        )
        .map_err(|_| "allowlisted repository cannot be opened")?;
        repository
            .revparse_single(&project.reference)
            .map_err(|_| "allowlisted repository reference unavailable")?;
        let socket = &project.workflow.socket_path;
        directory(socket.parent().ok_or("workflow socket parent missing")?)
            .map_err(|_| "workflow socket parent unavailable or unsafe")?;
        match fs::symlink_metadata(socket) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket()
                    || !project.workflow.allowed_user_ids.contains(&metadata.uid())
                {
                    return Err("workflow socket type or owner denied".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("workflow socket metadata unavailable".into()),
        }
    }
    Ok(())
}

fn state_directory(path: &Path, root: bool) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            directory(path).map_err(|_| "state directory unavailable or unsafe")?;
            if !metadata.is_dir()
                || metadata.uid() != nix::unistd::geteuid().as_raw()
                || metadata.mode() & 0o7777 != 0o700
            {
                return Err("state root must be an owned private directory".into());
            }
            safe_ancestors(path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if root {
                let parent = path.parent().ok_or("state parent missing")?;
                directory(parent).map_err(|_| "state parent unavailable or unsafe")?;
                safe_ancestors(parent)?;
                let metadata =
                    fs::symlink_metadata(parent).map_err(|_| "state parent unavailable")?;
                if metadata.uid() != nix::unistd::geteuid().as_raw()
                    || metadata.mode() & 0o300 != 0o300
                {
                    return Err("state parent must permit owned private state creation".into());
                }
            }
        }
        Err(_) => return Err("state directory metadata unavailable".into()),
    }
    Ok(())
}

fn safe_ancestors(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| "state ancestor unavailable")?;
        if metadata.file_type().is_symlink()
            || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
        {
            return Err(
                "state ancestors may not permit untrusted replacement or symlink traversal".into(),
            );
        }
    }
    Ok(())
}

fn state_file(path: &Path) -> Result<(), String> {
    let file: File = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("private state file unavailable or unsafe".into()),
    };
    let metadata = file
        .metadata()
        .map_err(|_| "state file metadata unavailable")?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err("state file must be an owned private regular file".into());
    }
    Ok(())
}
