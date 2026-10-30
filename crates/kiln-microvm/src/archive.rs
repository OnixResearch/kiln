//! Deterministic bounded ustar layout for the read-only source snapshot drive.
//!
//! The shell walks the workspace and streams file bytes; this module decides
//! entry admission, order and exact header bytes. Ownership, times and modes
//! are normalized so equal trees produce equal images.

pub const BLOCK_BYTES: u64 = 512;
pub const MAX_SOURCE_ENTRIES: usize = 16_384;
/// ustar `prefix` (155) + `/` + `name` (100).
pub const MAX_PATH_BYTES: usize = 256;
pub const MAX_LINK_BYTES: usize = 100;
/// Two zero blocks end a tar stream.
pub const TRAILER_BYTES: u64 = 2 * BLOCK_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File { size: u64, executable: bool },
    Symlink { target: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEntry {
    /// Workspace-relative, `/`-separated, no `.`/`..`/empty components.
    pub path: String,
    pub kind: EntryKind,
}

/// Bytes occupied by one entry: header plus content padded to whole blocks.
pub fn entry_bytes(entry: &SourceEntry) -> u64 {
    match entry.kind {
        EntryKind::File { size, .. } => BLOCK_BYTES + size.div_ceil(BLOCK_BYTES) * BLOCK_BYTES,
        _ => BLOCK_BYTES,
    }
}

fn valid_component(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".."
}

// r[impl kiln.hosted.microvm.plan]
/// Admit and order entries for an image of exactly `capacity_bytes`.
/// Returns the sorted entries and the archive length before zero padding.
pub fn plan_archive(
    mut entries: Vec<SourceEntry>,
    capacity_bytes: u64,
) -> Result<(Vec<SourceEntry>, u64), String> {
    if entries.len() > MAX_SOURCE_ENTRIES {
        return Err("source snapshot entry count exceeds bound".into());
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let mut total = TRAILER_BYTES;
    let mut previous: Option<&str> = None;
    for entry in &entries {
        if entry.path.len() > MAX_PATH_BYTES
            || entry.path.bytes().any(|b| b == 0 || b == b'\\')
            || !entry.path.split('/').all(valid_component)
            || split_path(&entry.path).is_none()
        {
            return Err("source snapshot path rejected".into());
        }
        if previous == Some(entry.path.as_str()) {
            return Err("duplicate source snapshot path".into());
        }
        previous = Some(&entry.path);
        if let EntryKind::Symlink { target } = &entry.kind {
            if target.is_empty() || target.len() > MAX_LINK_BYTES || target.contains('\0') {
                return Err("source snapshot symlink target rejected".into());
            }
        }
        total = total
            .checked_add(entry_bytes(entry))
            .ok_or("source snapshot size overflow")?;
        if total > capacity_bytes {
            return Err("source snapshot exceeds the fixed image size".into());
        }
    }
    Ok((entries, total))
}

/// Split into ustar (prefix, name) at a `/`, preferring the whole path in `name`.
fn split_path(path: &str) -> Option<(&str, &str)> {
    if path.len() <= 100 {
        return Some(("", path));
    }
    path.match_indices('/')
        .map(|(index, _)| index)
        .filter(|index| *index <= 155)
        .filter_map(|index| Some((path.get(..index)?, path.get(index + 1..)?)))
        .find(|(_, name)| !name.is_empty() && name.len() <= 100)
}

fn put(header: &mut [u8; 512], offset: usize, value: &[u8]) {
    for (index, byte) in value.iter().enumerate() {
        if let Some(slot) = header.get_mut(offset + index) {
            *slot = *byte;
        }
    }
}

fn octal(value: u64, width: usize) -> Vec<u8> {
    let mut text = format!("{value:0>digits$o}", digits = width - 1).into_bytes();
    text.push(0);
    text
}

/// Exact 512-byte ustar header for an admitted entry.
pub fn header(entry: &SourceEntry) -> Result<[u8; 512], String> {
    let (prefix, name) = split_path(&entry.path).ok_or("source snapshot path rejected")?;
    let (mode, size, flag, link) = match &entry.kind {
        EntryKind::Directory => (0o755, 0, b'5', ""),
        EntryKind::File { size, executable } => {
            (if *executable { 0o755 } else { 0o644 }, *size, b'0', "")
        }
        EntryKind::Symlink { target } => (0o777, 0, b'2', target.as_str()),
    };
    if size >= 1 << 33 {
        return Err("source snapshot file exceeds ustar size field".into());
    }
    let mut block = [0_u8; 512];
    put(&mut block, 0, name.as_bytes());
    put(&mut block, 100, &octal(mode, 8));
    put(&mut block, 108, &octal(0, 8));
    put(&mut block, 116, &octal(0, 8));
    put(&mut block, 124, &octal(size, 12));
    put(&mut block, 136, &octal(0, 12));
    put(&mut block, 148, b"        ");
    put(&mut block, 156, &[flag]);
    put(&mut block, 157, link.as_bytes());
    put(&mut block, 257, b"ustar\0");
    put(&mut block, 263, b"00");
    put(&mut block, 345, prefix.as_bytes());
    let checksum: u64 = block.iter().map(|b| u64::from(*b)).sum();
    let mut field = format!("{checksum:06o}").into_bytes();
    field.extend([0, b' ']);
    put(&mut block, 148, &field);
    Ok(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, size: u64) -> SourceEntry {
        SourceEntry {
            path: path.into(),
            kind: EntryKind::File {
                size,
                executable: false,
            },
        }
    }

    fn field(block: &[u8; 512], start: usize, len: usize) -> String {
        let bytes = block.get(start..start + len).unwrap_or_default();
        String::from_utf8_lossy(bytes)
            .trim_end_matches(['\0', ' '])
            .to_owned()
    }

    // r[verify kiln.hosted.microvm.plan]
    #[test]
    fn archive_order_and_headers_are_deterministic_and_checksummed() {
        let entries = vec![
            file("b.txt", 3),
            SourceEntry {
                path: "a".into(),
                kind: EntryKind::Directory,
            },
            SourceEntry {
                path: "a/link".into(),
                kind: EntryKind::Symlink {
                    target: "../b.txt".into(),
                },
            },
        ];
        let mut reversed = entries.clone();
        reversed.reverse();
        let first = plan_archive(entries, 1 << 20);
        assert_eq!(first, plan_archive(reversed, 1 << 20));
        let (ordered, total) = first.unwrap_or_else(|e| unreachable!("{e}"));
        let paths: Vec<_> = ordered.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "a/link", "b.txt"]);
        assert_eq!(total, 512 + 512 + 1024 + TRAILER_BYTES);
        for entry in &ordered {
            let block = header(entry).unwrap_or_else(|e| unreachable!("{e}"));
            let stored = u64::from_str_radix(&field(&block, 148, 6), 8).unwrap_or(u64::MAX);
            let mut blank = block;
            blank.get_mut(148..156).unwrap_or_default().fill(b' ');
            assert_eq!(stored, blank.iter().map(|b| u64::from(*b)).sum::<u64>());
            assert_eq!(field(&block, 257, 5), "ustar");
            assert_eq!(field(&block, 136, 12), "00000000000");
        }
        let link = header(ordered.get(1).unwrap_or_else(|| unreachable!()))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(field(&link, 157, 100), "../b.txt");
        assert_eq!(link.get(156), Some(&b'2'));
    }

    #[test]
    fn long_paths_split_into_prefix_and_name() {
        let long = format!("{}/{}", "d".repeat(150), "f".repeat(90));
        let block = header(&file(&long, 0)).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(field(&block, 0, 100), "f".repeat(90));
        assert_eq!(field(&block, 345, 155), "d".repeat(150));
        let unsplittable = "x".repeat(101);
        assert!(plan_archive(vec![file(&unsplittable, 0)], 1 << 20).is_err());
    }

    #[test]
    fn unsafe_duplicate_or_oversized_sources_are_rejected() {
        for path in ["", "/abs", "a/../b", "a//b", "./a", "a\\b", "a\0b"] {
            assert!(
                plan_archive(vec![file(path, 1)], 1 << 20).is_err(),
                "{path:?}"
            );
        }
        assert!(plan_archive(vec![file("a", 1), file("a", 1)], 1 << 20).is_err());
        let exact = 512 + 512 + TRAILER_BYTES;
        assert!(plan_archive(vec![file("a", 512)], exact).is_ok());
        assert!(plan_archive(vec![file("a", 513)], exact).is_err());
        let long_link = SourceEntry {
            path: "l".into(),
            kind: EntryKind::Symlink {
                target: "t".repeat(MAX_LINK_BYTES + 1),
            },
        };
        assert!(plan_archive(vec![long_link], 1 << 20).is_err());
        let many = (0..=MAX_SOURCE_ENTRIES)
            .map(|i| file(&format!("f{i}"), 0))
            .collect();
        assert!(plan_archive(many, u64::MAX).is_err());
    }
}
