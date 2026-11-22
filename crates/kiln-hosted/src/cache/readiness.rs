//! Opt-in, thread-scoped benchmark observations. No runtime service switch.
use std::cell::RefCell;
use std::time::{Duration, Instant};

use serde::Serialize;

pub const PHASES: &[&str] = &[
    "queue_admission",
    "evaluation_preflight",
    "store_open_manifest_validation",
    "lookup_transfer_decode",
    "cache_save_publication",
    "manifest_validation",
    "materialization",
    "publication_collection",
    "setup",
    "execution",
    "publication",
    "restore",
];

#[derive(Clone, Debug, Serialize)]
pub struct Observation {
    pub phase: String,
    pub elapsed_ns: u64,
    pub user_cpu_ns: Option<u64>,
    pub system_cpu_ns: Option<u64>,
    pub injected_delay_ms: u64,
    pub cpu_scope: &'static str,
}

#[derive(Default)]
struct Scope {
    samples: Vec<Observation>,
    delay: Option<(String, u64)>,
}
thread_local! {
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

fn cpu() -> Option<(u64, u64)> {
    let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_THREAD).ok()?;
    let nanos = |value: nix::sys::time::TimeVal| {
        u64::try_from(value.tv_sec())
            .ok()?
            .checked_mul(1_000_000_000)?
            .checked_add(u64::try_from(value.tv_usec()).ok()?.checked_mul(1_000)?)
    };
    Some((nanos(usage.user_time())?, nanos(usage.system_time())?))
}

/// Times only actual synchronous operations on the calling thread. Nested aggregate
/// phases are explicitly inclusive and must not be summed with their children.
pub fn phase<T>(name: &'static str, operation: impl FnOnce() -> T) -> T {
    let active = SCOPE.with(|slot| slot.borrow().is_some());
    if !active {
        return operation();
    }
    let delay = SCOPE
        .with(|slot| {
            slot.borrow().as_ref().and_then(|scope| {
                scope
                    .delay
                    .as_ref()
                    .filter(|(phase, _)| phase == name)
                    .map(|(_, ms)| *ms)
            })
        })
        .unwrap_or(0);
    let started = Instant::now();
    let before = cpu();
    if delay > 0 {
        std::thread::sleep(Duration::from_millis(delay));
    }
    let result = operation();
    let after = cpu();
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let delta = before
        .zip(after)
        .map(|(a, b)| (b.0.saturating_sub(a.0), b.1.saturating_sub(a.1)));
    SCOPE.with(|slot| {
        if let Some(scope) = slot.borrow_mut().as_mut() {
            scope.samples.push(Observation {
                phase: name.into(),
                elapsed_ns,
                user_cpu_ns: delta.map(|value| value.0),
                system_cpu_ns: delta.map(|value| value.1),
                injected_delay_ms: delay,
                cpu_scope: "calling-thread",
            });
        }
    });
    result
}

/// A scope is exclusive and bounded by the caller's workload. Reset even on panic.
pub fn capture<T>(
    delay: Option<(String, u64)>,
    operation: impl FnOnce() -> T,
) -> Result<(T, Vec<Observation>), String> {
    if let Some((phase, ms)) = &delay
        && (!PHASES.contains(&phase.as_str()) || *ms > 1_000)
    {
        return Err("unknown phase or delay exceeds 1000 ms".into());
    }
    SCOPE.with(|slot| -> Result<(), String> {
        if slot.borrow().is_some() {
            return Err("nested readiness capture is unsupported".into());
        }
        *slot.borrow_mut() = Some(Scope {
            samples: Vec::new(),
            delay,
        });
        Ok(())
    })?;
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            SCOPE.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }
    let _reset = Reset;
    let result = operation();
    let samples = SCOPE
        .with(|slot| slot.borrow_mut().take().map(|scope| scope.samples))
        .ok_or("readiness scope disappeared")?;
    Ok((result, samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_materialization_is_not_execution_and_errors_are_preserved() {
        let (result, samples) = capture(Some(("materialization".into(), 20)), || {
            phase("execution", || 1);
            phase("materialization", || {
                Err::<(), _>("original provider failure")
            })
        })
        .unwrap_or_else(|error| unreachable!("capture: {error}"));
        assert_eq!(result, Err("original provider failure"));
        assert_eq!(samples.len(), 2);
        let first = samples
            .first()
            .unwrap_or_else(|| unreachable!("first sample"));
        let second = samples
            .get(1)
            .unwrap_or_else(|| unreachable!("second sample"));
        assert_eq!(first.injected_delay_ms, 0);
        assert_eq!(second.phase, "materialization");
        assert!(second.elapsed_ns >= 20_000_000);
        assert_eq!(second.injected_delay_ms, 20);
    }

    #[test]
    fn invalid_delays_and_nested_scopes_are_rejected_before_work() {
        assert!(
            capture(Some(("compilation".into(), 1)), || unreachable!(
                "not called"
            ))
            .is_err()
        );
        assert!(
            capture(Some(("materialization".into(), 1001)), || unreachable!(
                "not called"
            ))
            .is_err()
        );
        let (nested, _) = capture(None, || capture(None, || ()))
            .unwrap_or_else(|error| unreachable!("capture: {error}"));
        assert!(nested.is_err());
        assert!(capture(None, || ()).is_ok());
    }
}
