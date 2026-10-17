use super::*;

pub(in super::super) struct HostChild {
    pub(super) child: Child,
    pub(super) stderr: std::fs::File,
}

impl HostChild {
    pub(super) fn diagnostics(&self) -> String {
        use std::os::unix::fs::FileExt;
        const MAXIMUM_DIAGNOSTIC_BYTES: usize = 16_384;
        let mut bytes = [0; MAXIMUM_DIAGNOSTIC_BYTES];
        let count = self
            .stderr
            .read_at(&mut bytes, 0)
            .unwrap_or_else(|_| unreachable!("read child diagnostics"));
        let retained = bytes
            .get(..count)
            .unwrap_or_else(|| unreachable!("child diagnostic read exceeded buffer"));
        String::from_utf8_lossy(retained).into_owned()
    }

    pub(in super::super) fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(CHILD_DEADLINE_SECONDS);
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .unwrap_or_else(|_| unreachable!("wait child status"))
            {
                println!(
                    "{}",
                    serde_json::json!({
                        "schema": "kiln.aspen-host.qualification-child-exit.v1",
                        "pid": self.child.id(),
                        "status": status.to_string(),
                        "code": status.code(),
                        "diagnostics": self.diagnostics(),
                    })
                );
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "child exit deadline: {}",
                self.diagnostics()
            );
            std::thread::park_timeout(Duration::from_millis(CHILD_POLL_MILLISECONDS));
        }
    }

    pub(in super::super) fn finish(mut self) {
        let status = self.wait();
        assert!(
            status.success(),
            "host exit {status}: {}",
            self.diagnostics()
        );
    }

    pub(in super::super) fn reject(mut self, expected_diagnostic: &str) {
        let status = self.wait();
        assert_eq!(
            status.code(),
            Some(HOST_REJECTION_EXIT_CODE),
            "host rejection: {}",
            self.diagnostics()
        );
        assert!(
            self.diagnostics().contains(expected_diagnostic),
            "expected {expected_diagnostic:?}: {}",
            self.diagnostics()
        );
    }

    pub(in super::super) fn crash(mut self) {
        use std::os::unix::process::ExitStatusExt;
        const SIGKILL: i32 = 9;
        self.child
            .kill()
            .unwrap_or_else(|_| unreachable!("kill owned host"));
        assert_eq!(self.wait().signal(), Some(SIGKILL));
    }
}

impl Drop for HostChild {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(()) | Err(_) => {}
        }
        match self.child.wait() {
            Ok(_) | Err(_) => {}
        }
    }
}
