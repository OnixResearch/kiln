#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessIdentity {
    pub(super) pid: u32,
    pub(super) group: u32,
    pub(super) start_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessObservation {
    pub(super) identity: ProcessIdentity,
    pub(super) state: char,
}

pub(super) fn observe_process(pid: u32) -> super::TestResult<Option<ProcessObservation>> {
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    // comm may itself contain spaces or parentheses; the last ')' ends it.
    let (_, fields) = stat.rsplit_once(')').ok_or("malformed proc stat")?;
    let mut fields = fields.split_whitespace();
    let state = fields
        .next()
        .ok_or("missing process state")?
        .chars()
        .next()
        .ok_or("empty process state")?;
    let group = fields.nth(1).ok_or("missing process group")?.parse()?;
    let start_ticks = fields
        .nth(16)
        .ok_or("missing process start time")?
        .parse()?;
    Ok(Some(ProcessObservation {
        identity: ProcessIdentity {
            pid,
            group,
            start_ticks,
        },
        state,
    }))
}

pub(super) fn read_identity(path: &std::path::Path) -> super::TestResult<Option<ProcessIdentity>> {
    let text = std::fs::read_to_string(path)?;
    let Ok(pid) = text.trim().parse() else {
        // A freshly created PID file may not have received its bytes yet.
        return Ok(None);
    };
    Ok(observe_process(pid)?.map(|observation| observation.identity))
}

pub(super) fn signal(pid: u32, signal: rustix::process::Signal) -> super::TestResult {
    let raw_pid = i32::try_from(pid)?;
    let pid =
        rustix::process::Pid::from_raw(raw_pid).ok_or("provider process ID must be nonzero")?;
    rustix::process::kill_process(pid, signal)?;
    Ok(())
}
