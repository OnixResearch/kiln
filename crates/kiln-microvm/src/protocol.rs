//! Bounded framed host/guest protocol over vsock.
//!
//! A frame is a 4-byte big-endian length followed by that many bytes of JSON.
//! Every frame carries the versioned schema and is rejected otherwise.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::GUEST_WORKSPACE;

pub const SCHEMA: &str = "kiln.microvm.guest.v1";
pub const MAX_FRAME_BYTES: usize = 4 * 1_048_576;
pub const MAX_STEPS: usize = 128;
pub const ENTROPY_BYTES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostFrame {
    pub schema: String,
    pub message: HostMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostMessage {
    /// Readiness probe used before a warm snapshot is taken.
    Ping { nonce: String },
    /// The whole job. A guest accepts at most one run per boot.
    Run {
        nonce: String,
        /// Host wall clock, so restored snapshots do not keep a stale time.
        clock_unix_ms: u64,
        /// Fresh host seed mixed into the guest pool after boot or restore.
        entropy_hex: String,
        /// Job deadline relative to receipt of this frame.
        deadline_ms: u64,
        steps: Vec<GuestStep>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestStep {
    pub program: String,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub working_directory: String,
    pub output_limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestFrame {
    pub schema: String,
    pub message: GuestMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GuestMessage {
    Pong {
        nonce: String,
    },
    Step(StepReport),
    Done {
        nonce: String,
        /// Set when the source snapshot or request could not be admitted.
        admission_error: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepReport {
    pub index: usize,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub elapsed_ms: u64,
}

// r[impl kiln.hosted.microvm.protocol]
/// Length-prefixed encoding, rejected above the frame bound.
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(message).map_err(|e| e.to_string())?;
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err("microVM frame exceeds bound".into());
    }
    let length = u32::try_from(body.len()).map_err(|_| "microVM frame length overflow")?;
    let mut frame = length.to_be_bytes().to_vec();
    frame.extend(body);
    Ok(frame)
}

/// Validate a received length prefix before reading the body.
pub fn frame_length(prefix: [u8; 4]) -> Result<usize, String> {
    let length = usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| "frame length")?;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err("microVM frame length outside bound".into());
    }
    Ok(length)
}

fn decode<T: DeserializeOwned>(body: &[u8], schema: impl Fn(&T) -> &str) -> Result<T, String> {
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err("microVM frame body outside bound".into());
    }
    let frame: T = serde_json::from_slice(body).map_err(|_| "malformed microVM frame")?;
    if schema(&frame) != SCHEMA {
        return Err("microVM protocol schema mismatch".into());
    }
    Ok(frame)
}

pub fn decode_host(body: &[u8]) -> Result<HostFrame, String> {
    let frame = decode(body, |f: &HostFrame| f.schema.as_str())?;
    if let HostMessage::Run {
        steps, entropy_hex, ..
    } = &frame.message
    {
        if steps.is_empty() || steps.len() > MAX_STEPS {
            return Err("microVM run step count outside bound".into());
        }
        if entropy_hex.len() != ENTROPY_BYTES * 2 || hex_bytes(entropy_hex).is_none() {
            return Err("microVM entropy seed rejected".into());
        }
        if steps.iter().any(|s| {
            !inside_workspace(&s.working_directory)
                || !s.program.starts_with('/')
                || s.output_limit == 0
                || s.output_limit > MAX_FRAME_BYTES / 4
        }) {
            return Err("microVM step rejected".into());
        }
    }
    Ok(frame)
}

pub fn decode_guest(body: &[u8]) -> Result<GuestFrame, String> {
    decode(body, |f: &GuestFrame| f.schema.as_str())
}

pub fn host_frame(message: HostMessage) -> HostFrame {
    HostFrame {
        schema: SCHEMA.into(),
        message,
    }
}

pub fn guest_frame(message: GuestMessage) -> GuestFrame {
    GuestFrame {
        schema: SCHEMA.into(),
        message,
    }
}

/// Lowercase hex decoding for the entropy seed.
pub fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digit = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                _ => None,
            };
            Some((digit(*pair.first()?)? << 4) | digit(*pair.get(1)?)?)
        })
        .collect()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn inside_workspace(path: &str) -> bool {
    path == GUEST_WORKSPACE
        || path.strip_prefix(GUEST_WORKSPACE).is_some_and(|rest| {
            rest.starts_with('/') && rest.split('/').all(|p| p != ".." && p != ".")
        })
}

/// `os.path.normpath('/work/' + relative)` as used by the namespace backend.
pub fn guest_working_directory(relative: &str) -> Result<String, String> {
    let joined = format!("{GUEST_WORKSPACE}/{relative}");
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let path = format!("/{}", parts.join("/"));
    if inside_workspace(&path) {
        Ok(path)
    } else {
        Err("working directory escapes source".into())
    }
}

/// Shell flags shared with the namespace and QEMU backends.
pub fn shell_arguments(bash: bool, run: &str) -> Vec<String> {
    let mut arguments: Vec<String> = if bash {
        ["--noprofile", "--norc", "-e", "-o", "pipefail"]
            .map(String::from)
            .to_vec()
    } else {
        vec!["-e".into()]
    };
    arguments.extend(["-c".into(), run.into()]);
    arguments
}

/// The namespace backend's exact environment: base variables, then step values.
pub fn guest_environment(shell: &str, step: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let directory = shell.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut environment = BTreeMap::from([
        ("PATH".to_owned(), format!("{directory}:/bin")),
        ("HOME".to_owned(), "/tmp".to_owned()),
        ("TMPDIR".to_owned(), "/tmp".to_owned()),
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
    ]);
    environment.extend(step.iter().map(|(k, v)| (k.clone(), v.clone())));
    environment
}

/// Build one guest step from a reviewed compiled step.
pub fn guest_step(
    shell: &str,
    bash: bool,
    run: &str,
    working_directory: &str,
    environment: &BTreeMap<String, String>,
    output_limit: usize,
) -> Result<GuestStep, String> {
    Ok(GuestStep {
        program: shell.into(),
        arguments: shell_arguments(bash, run),
        environment: guest_environment(shell, environment),
        working_directory: guest_working_directory(working_directory)?,
        output_limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run() -> HostMessage {
        HostMessage::Run {
            nonce: "n".into(),
            clock_unix_ms: 1,
            entropy_hex: "ab".repeat(ENTROPY_BYTES),
            deadline_ms: 1000,
            steps: vec![
                guest_step("/bin/bash", true, "echo hi", ".", &BTreeMap::new(), 1024)
                    .unwrap_or_else(|e| unreachable!("{e}")),
            ],
        }
    }

    fn body(frame: &[u8]) -> &[u8] {
        frame.get(4..).unwrap_or_default()
    }

    // r[verify kiln.hosted.microvm.protocol]
    #[test]
    fn frames_round_trip_and_reject_wrong_schema_unknown_fields_and_bounds() {
        let frame = encode(&host_frame(run())).unwrap_or_else(|e| unreachable!("{e}"));
        let prefix: [u8; 4] = frame
            .get(..4)
            .and_then(|p| p.try_into().ok())
            .unwrap_or_default();
        assert_eq!(frame_length(prefix), Ok(frame.len() - 4));
        assert_eq!(decode_host(body(&frame)), Ok(host_frame(run())));
        assert!(frame_length(0_u32.to_be_bytes()).is_err());
        assert!(
            frame_length(
                u32::try_from(MAX_FRAME_BYTES + 1)
                    .unwrap_or(u32::MAX)
                    .to_be_bytes()
            )
            .is_err()
        );

        let mut wrong = host_frame(run());
        wrong.schema = "kiln.microvm.guest.v0".into();
        let encoded = encode(&wrong).unwrap_or_default();
        assert!(decode_host(body(&encoded)).is_err());
        let extra =
            json!({"schema": SCHEMA, "message": {"kind": "ping", "nonce": "n", "extra": 1}});
        assert!(decode_host(extra.to_string().as_bytes()).is_err());
        assert!(decode_guest(b"not json").is_err());

        let report = guest_frame(GuestMessage::Done {
            nonce: "n".into(),
            admission_error: None,
        });
        let encoded = encode(&report).unwrap_or_default();
        assert_eq!(decode_guest(body(&encoded)), Ok(report));
        let oversized = guest_frame(GuestMessage::Pong {
            nonce: "x".repeat(MAX_FRAME_BYTES),
        });
        assert!(encode(&oversized).is_err());
    }

    // r[verify kiln.hosted.microvm.protocol]
    #[test]
    fn run_requests_reject_escaping_or_unbounded_steps() {
        let mut steps = match run() {
            HostMessage::Run { steps, .. } => steps,
            HostMessage::Ping { .. } => unreachable!(),
        };
        let valid = steps.pop().unwrap_or_else(|| unreachable!());
        for (step, entropy) in [
            (
                GuestStep {
                    working_directory: "/etc".into(),
                    ..valid.clone()
                },
                "ab".repeat(32),
            ),
            (
                GuestStep {
                    working_directory: "/work/../etc".into(),
                    ..valid.clone()
                },
                "ab".repeat(32),
            ),
            (
                GuestStep {
                    program: "bash".into(),
                    ..valid.clone()
                },
                "ab".repeat(32),
            ),
            (
                GuestStep {
                    output_limit: 0,
                    ..valid.clone()
                },
                "ab".repeat(32),
            ),
            (valid.clone(), "AB".repeat(32)),
            (valid.clone(), "ab".repeat(31)),
        ] {
            let frame = host_frame(HostMessage::Run {
                nonce: "n".into(),
                clock_unix_ms: 0,
                entropy_hex: entropy,
                deadline_ms: 1,
                steps: vec![step],
            });
            assert!(decode_host(&serde_json::to_vec(&frame).unwrap_or_default()).is_err());
        }
        let empty = host_frame(HostMessage::Run {
            nonce: "n".into(),
            clock_unix_ms: 0,
            entropy_hex: "00".repeat(32),
            deadline_ms: 1,
            steps: vec![],
        });
        assert!(decode_host(&serde_json::to_vec(&empty).unwrap_or_default()).is_err());
    }

    // r[verify kiln.hosted.microvm.protocol]
    #[test]
    fn environment_arguments_and_directory_match_the_namespace_backend() {
        let step_env = BTreeMap::from([
            ("CI".to_owned(), "true".to_owned()),
            ("HOME".to_owned(), "/custom".to_owned()),
        ]);
        let environment = guest_environment("/bin/bash", &step_env);
        assert_eq!(
            environment,
            BTreeMap::from([
                ("CI".to_owned(), "true".to_owned()),
                ("HOME".to_owned(), "/custom".to_owned()),
                ("LANG".to_owned(), "C.UTF-8".to_owned()),
                ("PATH".to_owned(), "/bin:/bin".to_owned()),
                ("TMPDIR".to_owned(), "/tmp".to_owned()),
            ])
        );
        assert_eq!(
            shell_arguments(true, "x"),
            ["--noprofile", "--norc", "-e", "-o", "pipefail", "-c", "x"]
        );
        assert_eq!(shell_arguments(false, "x"), ["-e", "-c", "x"]);
        assert_eq!(guest_working_directory("."), Ok("/work".into()));
        assert_eq!(guest_working_directory("a/./b/"), Ok("/work/a/b".into()));
        assert_eq!(guest_working_directory("a/../b"), Ok("/work/b".into()));
        assert!(guest_working_directory("..").is_err());
        assert!(guest_working_directory("a/../../etc").is_err());
        assert_eq!(hex_bytes("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
        assert_eq!(hex_bytes("0"), None);
    }
}
