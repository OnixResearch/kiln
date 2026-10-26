use std::fs::{self, File};
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use nix::sys::statvfs::{FsFlags, fstatvfs};
use nix::unistd::geteuid;

use super::profile::relative_file;

/// Traverse directory descriptors, never following a symlink, including ancestors.
pub fn directory(path: &Path) -> Result<OwnedFd, String> {
    if !path.is_absolute() {
        return Err("worker path must be absolute".into());
    }
    let mut fd = open(
        "/",
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| "worker root unavailable")?;
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                fd = openat(
                    &fd,
                    Path::new(name),
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| "worker directory traversal denied")?;
            }
            _ => return Err("worker directory path invalid".into()),
        }
    }
    Ok(fd)
}

pub fn owned_directory(path: &Path) -> Result<(), String> {
    let fd = directory(path)?;
    let metadata = File::from(fd)
        .metadata()
        .map_err(|_| "worker directory unavailable")?;
    if metadata.uid() != geteuid().as_raw() || metadata.mode() & 0o077 != 0 {
        return Err("worker directory must be private and owned".into());
    }
    Ok(())
}

pub fn regular(path: &Path, maximum: usize, private: bool) -> Result<Vec<u8>, String> {
    let parent = path.parent().ok_or("worker file parent missing")?;
    let fd = directory(parent)?;
    let name = path.file_name().ok_or("worker file name missing")?;
    let file = openat(
        &fd,
        Path::new(name),
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| "worker file unavailable")?;
    read_file(File::from(file), maximum, private)
}

fn read_file(mut file: File, maximum: usize, private: bool) -> Result<Vec<u8>, String> {
    let before = file
        .metadata()
        .map_err(|_| "worker file metadata unavailable")?;
    if !before.is_file()
        || before.nlink() != 1
        || before.len() > maximum as u64
        || (private && !private_access(&file, &before))
    {
        return Err("worker file type, ownership or bound denied".into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "worker file read failed")?;
    let after = file
        .metadata()
        .map_err(|_| "worker file metadata unavailable")?;
    if bytes.len() > maximum
        || bytes.len() as u64 != before.len()
        || after.len() != before.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err("worker file changed or exceeded bound".into());
    }
    Ok(bytes)
}

// Linux POSIX ACL xattr layout, not an application identity format.
const ACL_VERSION: u32 = 2;
const ACL_HEADER_BYTES: usize = 4;
const ACL_ENTRY_BYTES: usize = 8;
const PRIVATE_ACL_ENTRIES: usize = 5;
const PRIVATE_ACL_BYTES: usize = ACL_HEADER_BYTES + ACL_ENTRY_BYTES * PRIVATE_ACL_ENTRIES;
const ACL_USER_OBJ: u16 = 0x01;
const ACL_USER: u16 = 0x02;
const ACL_GROUP_OBJ: u16 = 0x04;
const ACL_MASK: u16 = 0x10;
const ACL_OTHER: u16 = 0x20;
const ACL_READ: u16 = 0x04;

fn private_access(file: &File, metadata: &fs::Metadata) -> bool {
    let uid = geteuid().as_raw();
    let mode = metadata.mode() & 0o7777;
    if metadata.uid() == uid && matches!(mode, 0o400 | 0o600) {
        return true;
    }
    if uid == 0
        || metadata.uid() != 0
        || mode != 0o440
        || !fstatvfs(file).is_ok_and(|facts| facts.flags().contains(FsFlags::ST_RDONLY))
    {
        return false;
    }
    let mut acl = [0_u8; PRIVATE_ACL_BYTES + 1];
    rustix::fs::fgetxattr(file, c"system.posix_acl_access", &mut acl).is_ok_and(|length| {
        length == PRIVATE_ACL_BYTES && private_acl(&acl[..PRIVATE_ACL_BYTES], uid)
    })
}

fn private_acl(acl: &[u8], uid: u32) -> bool {
    let Some((header, entries)) = acl.split_first_chunk::<ACL_HEADER_BYTES>() else {
        return false;
    };
    if uid == 0 || acl.len() != PRIVATE_ACL_BYTES || *header != ACL_VERSION.to_le_bytes() {
        return false;
    }
    let expected = [
        (ACL_USER_OBJ, ACL_READ, u32::MAX),
        (ACL_USER, ACL_READ, uid),
        (ACL_GROUP_OBJ, 0, u32::MAX),
        (ACL_MASK, ACL_READ, u32::MAX),
        (ACL_OTHER, 0, u32::MAX),
    ];
    entries.chunks_exact(ACL_ENTRY_BYTES).zip(expected).all(
        |(entry, (tag, permissions, identity))| {
            tag.to_le_bytes()
                .into_iter()
                .chain(permissions.to_le_bytes())
                .chain(identity.to_le_bytes())
                .eq(entry.iter().copied())
        },
    )
}

pub fn artifact(root: &Path, relative: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    if !relative_file(relative) {
        return Err("worker artifact path denied".into());
    }
    let mut directory_fd = directory(root)?;
    let mut parts = relative.components().peekable();
    while let Some(part) = parts.next() {
        let Component::Normal(name) = part else {
            return Err("worker artifact path denied".into());
        };
        let final_part = parts.peek().is_none();
        let mut flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
        if !final_part {
            flags |= OFlag::O_DIRECTORY;
        }
        let fd = openat(&directory_fd, Path::new(name), flags, Mode::empty())
            .map_err(|_| "worker artifact traversal denied")?;
        if final_part {
            return read_file(File::from(fd), maximum, false);
        }
        directory_fd = fd;
    }
    Err("worker artifact path denied".into())
}

pub fn create_private(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|_| "worker checkout creation failed".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acl(uid: u32, group: u16, other: u16) -> Vec<u8> {
        let mut bytes = ACL_VERSION.to_le_bytes().to_vec();
        for (tag, permissions, identity) in [
            (ACL_USER_OBJ, ACL_READ, u32::MAX),
            (ACL_USER, ACL_READ, uid),
            (ACL_GROUP_OBJ, group, u32::MAX),
            (ACL_MASK, ACL_READ, u32::MAX),
            (ACL_OTHER, other, u32::MAX),
        ] {
            bytes.extend(tag.to_le_bytes());
            bytes.extend(permissions.to_le_bytes());
            bytes.extend(identity.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn private_acl_grants_only_root_and_the_exact_service_identity() {
        let service_uid = 41001;
        let valid = acl(service_uid, 0, 0);
        assert!(private_acl(&valid, service_uid));
        assert!(!private_acl(&valid, service_uid + 1));
        assert!(!private_acl(&acl(service_uid, ACL_READ, 0), service_uid));
        assert!(!private_acl(&acl(service_uid, 0, ACL_READ), service_uid));
        assert!(!private_acl(&acl(0, 0, 0), 0));
        let mut additional_user = valid.clone();
        additional_user.extend_from_slice(
            valid
                .get(ACL_HEADER_BYTES + ACL_ENTRY_BYTES..ACL_HEADER_BYTES + 2 * ACL_ENTRY_BYTES)
                .unwrap_or_else(|| unreachable!()),
        );
        assert!(!private_acl(&additional_user, service_uid));
        let mut truncated = valid.clone();
        truncated.pop();
        assert!(!private_acl(&truncated, service_uid));
        for index in [
            0,
            ACL_HEADER_BYTES,
            ACL_HEADER_BYTES + ACL_ENTRY_BYTES + 2,
            ACL_HEADER_BYTES + 3 * ACL_ENTRY_BYTES + 2,
        ] {
            let mut malformed = valid.clone();
            *malformed.get_mut(index).unwrap_or_else(|| unreachable!()) ^= 1;
            assert!(!private_acl(&malformed, service_uid));
        }
    }
}
