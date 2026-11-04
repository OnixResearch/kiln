//! Bounded host observation of the Firecracker VMM process.
//!
//! The pure module parses the selected `/proc/<pid>/status` fields and admits
//! the reviewed executable path. The hosted shell owns cgroup lookup, process
//! reads and PID selection.

/// Maximum retained `/proc/<pid>/status` bytes.
pub const MAX_STATUS_BYTES: usize = 65_536;
/// Maximum retained status lines.
pub const MAX_STATUS_LINES: usize = 256;
/// Maximum selected field value bytes.
pub const MAX_VALUE_BYTES: usize = 256;
/// Maximum executable path bytes.
pub const MAX_EXE_BYTES: usize = 4_096;

/// Expected service-user facts and the reviewed VMM executable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedProcess<'a> {
    pub uid: u32,
    pub gid: u32,
    pub exe: &'a str,
}

/// Admitted host-side process facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfinementFacts {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub cap_eff: u64,
    pub no_new_privs: bool,
    pub seccomp: u8,
    pub exe: String,
}

#[derive(Default)]
struct Selected {
    name: Option<String>,
    uid: Option<String>,
    gid: Option<String>,
    cap_eff: Option<String>,
    no_new_privs: Option<String>,
    seccomp: Option<String>,
}

fn reject() -> &'static str {
    "VMM process evidence is missing, malformed or outside its bound"
}

fn selected(field: &mut Option<String>, value: &str) -> Result<(), &'static str> {
    if field.is_some() || value.is_empty() || value.len() > MAX_VALUE_BYTES {
        return Err(reject());
    }
    *field = Some(value.to_owned());
    Ok(())
}

fn parse_u32(value: &str) -> Result<u32, &'static str> {
    let mut values = value.split_ascii_whitespace();
    let first = values.next().ok_or_else(reject)?;
    let parsed = first.parse::<u32>().map_err(|_| reject())?;
    if values.next().is_none() {
        return Err(reject());
    }
    for value in values {
        value.parse::<u32>().map_err(|_| reject())?;
    }
    Ok(parsed)
}

fn parse_cap_eff(value: &str) -> Result<u64, &'static str> {
    if value.is_empty() || value.len() > 16 {
        return Err(reject());
    }
    u64::from_str_radix(value, 16).map_err(|_| reject())
}

fn parse_u8(value: &str) -> Result<u8, &'static str> {
    value.parse::<u8>().map_err(|_| reject())
}

/// Admit one exact VMM process. `/proc/<pid>/exe` is read by the caller and
/// passed as `exe`; the parser performs no I/O.
pub fn admit_status(
    status: &[u8],
    exe: &str,
    expected: ExpectedProcess<'_>,
) -> Result<ConfinementFacts, &'static str> {
    if status.is_empty()
        || status.len() > MAX_STATUS_BYTES
        || exe.is_empty()
        || exe.len() > MAX_EXE_BYTES
        || exe.contains('\0')
        || exe != expected.exe
    {
        return Err(reject());
    }
    let text = std::str::from_utf8(status).map_err(|_| reject())?;
    let mut fields = Selected::default();
    let mut lines = 0usize;
    for line in text.lines() {
        lines += 1;
        if lines > MAX_STATUS_LINES || line.is_empty() {
            return Err(reject());
        }
        let (key, value) = line.split_once(':').ok_or_else(reject)?;
        let value = value.trim_matches([' ', '\t']);
        match key {
            "Name" => selected(&mut fields.name, value)?,
            "Uid" => selected(&mut fields.uid, value)?,
            "Gid" => selected(&mut fields.gid, value)?,
            "CapEff" => selected(&mut fields.cap_eff, value)?,
            "NoNewPrivs" => selected(&mut fields.no_new_privs, value)?,
            "Seccomp" => selected(&mut fields.seccomp, value)?,
            _ => {}
        }
    }
    let name = fields.name.ok_or_else(reject)?;
    let uid = parse_u32(fields.uid.as_deref().ok_or_else(reject)?)?;
    let gid = parse_u32(fields.gid.as_deref().ok_or_else(reject)?)?;
    let cap_eff = parse_cap_eff(fields.cap_eff.as_deref().ok_or_else(reject)?)?;
    let no_new_privs = parse_u8(fields.no_new_privs.as_deref().ok_or_else(reject)?)?;
    let seccomp = parse_u8(fields.seccomp.as_deref().ok_or_else(reject)?)?;
    if name != "firecracker"
        || uid != expected.uid
        || gid != expected.gid
        || cap_eff != 0
        || no_new_privs != 1
        || seccomp != 2
    {
        return Err(reject());
    }
    Ok(ConfinementFacts {
        name,
        uid,
        gid,
        cap_eff,
        no_new_privs: true,
        seccomp,
        exe: exe.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-firecracker/bin/firecracker";

    fn status() -> Vec<u8> {
        b"Name:\tfirecracker\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\nCapEff:\t0000000000000000\nNoNewPrivs:\t1\nSeccomp:\t2\n".to_vec()
    }

    fn expected() -> ExpectedProcess<'static> {
        ExpectedProcess {
            uid: 1000,
            gid: 1000,
            exe: EXE,
        }
    }

    #[test]
    fn confined_process_is_admitted() {
        let facts = admit_status(&status(), EXE, expected()).unwrap_or_else(|error| {
            unreachable!("valid confinement fixture was rejected: {error}")
        });
        assert_eq!(facts.name, "firecracker");
        assert_eq!(facts.uid, 1000);
        assert_eq!(facts.gid, 1000);
        assert_eq!(facts.cap_eff, 0);
        assert!(facts.no_new_privs);
        assert_eq!(facts.seccomp, 2);
        assert_eq!(facts.exe, EXE);
    }

    #[test]
    fn selected_field_changes_fail_closed() {
        for (from, to) in [
            ("Name:\tfirecracker", "Name:\tother"),
            (
                "Uid:\t1000\t1000\t1000\t1000",
                "Uid:\t1001\t1001\t1001\t1001",
            ),
            (
                "Gid:\t1000\t1000\t1000\t1000",
                "Gid:\t1001\t1001\t1001\t1001",
            ),
            ("CapEff:\t0000000000000000", "CapEff:\t0000000000000001"),
            ("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
            ("Seccomp:\t2", "Seccomp:\t1"),
        ] {
            let mut body = String::from_utf8(status()).unwrap_or_default();
            body = body.replace(from, to);
            assert!(
                admit_status(body.as_bytes(), EXE, expected()).is_err(),
                "changed field was admitted: {to}"
            );
        }
    }

    #[test]
    fn duplicate_missing_and_malformed_fields_fail_closed() {
        let mut duplicate = String::from_utf8(status()).unwrap_or_default();
        duplicate.push_str("Seccomp:\t2\n");
        assert!(admit_status(duplicate.as_bytes(), EXE, expected()).is_err());

        let missing = b"Name:\tfirecracker\nUid:\t1000\t1000\t1000\t1000\n";
        assert!(admit_status(missing, EXE, expected()).is_err());

        for body in [
            b"not a status\n".as_slice(),
            b"Name: firecracker\nUid: nope\nGid: 1000\nCapEff: 0\nNoNewPrivs: 1\nSeccomp: 2\n",
            b"Name: firecracker\nUid: 1000\nGid: 1000\nCapEff: 0\nNoNewPrivs: 1\nSeccomp: 2\n",
        ] {
            assert!(admit_status(body, EXE, expected()).is_err());
        }
    }

    #[test]
    fn executable_and_bounds_are_admitted_exactly() {
        assert!(admit_status(&status(), "/other/firecracker", expected()).is_err());
        assert!(admit_status(&status(), "", expected()).is_err());
        let oversized = vec![b'a'; MAX_STATUS_BYTES + 1];
        assert!(admit_status(&oversized, EXE, expected()).is_err());
        assert!(admit_status(&[0xff, 0xfe], EXE, expected()).is_err());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let mut body = String::from_utf8(status()).unwrap_or_default();
        body.push_str("VmRSS:\t1024 kB\nThreads:\t3\n");
        assert!(admit_status(body.as_bytes(), EXE, expected()).is_ok());
    }
}
