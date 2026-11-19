//! Stable libtest summary admission. This is an oracle grammar, not proof of test truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coverage {
    pub passed: bool,
    pub executed: u64,
}

/// Admit complete UTF-8 libtest summaries paired with their announced test counts.
/// Other output grants no coverage. Malformed, unmatched, or failing summaries fail closed.
pub fn coverage(bytes: &[u8]) -> Coverage {
    parse(bytes).unwrap_or(Coverage {
        passed: false,
        executed: 0,
    })
}

fn parse(bytes: &[u8]) -> Option<Coverage> {
    let text = std::str::from_utf8(bytes).ok()?;
    // A prefix ending inside the final summary is not a complete payload.
    if !text.ends_with('\n') {
        return None;
    }
    let mut pending = None;
    let mut executed = 0_u64;
    let mut summaries = 0_u64;
    let mut passed = true;
    for line in text.lines() {
        if let Some(count) = line.strip_prefix("running ") {
            let count = count
                .strip_suffix(" tests")
                .or_else(|| count.strip_suffix(" test"))?;
            if pending.replace(count.parse::<u64>().ok()?).is_some() {
                return None;
            }
        } else if let Some(summary) = line.strip_prefix("test result: ") {
            let (status, counts) = summary.split_once(". ")?;
            if !matches!(status, "ok" | "FAILED") {
                return None;
            }
            let fields: Vec<_> = counts.split("; ").collect();
            let [passed_field, failed_field, ignored_field, measured_field, filtered_field, elapsed_field] =
                fields.as_slice()
            else {
                return None;
            };
            let elapsed = elapsed_field
                .strip_prefix("finished in ")?
                .strip_suffix('s')?;
            let (seconds, fraction) = elapsed.split_once('.')?;
            if seconds.is_empty()
                || fraction.is_empty()
                || !seconds.bytes().all(|b| b.is_ascii_digit())
                || !fraction.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            let number =
                |field: &str, suffix: &str| field.strip_suffix(suffix)?.parse::<u64>().ok();
            let good = number(passed_field, " passed")?;
            let failed = number(failed_field, " failed")?;
            let ignored = number(ignored_field, " ignored")?;
            let measured = number(measured_field, " measured")?;
            number(filtered_field, " filtered out")?;
            if pending.take()?
                != good
                    .checked_add(failed)?
                    .checked_add(ignored)?
                    .checked_add(measured)?
            {
                return None;
            }
            executed = executed.checked_add(good)?.checked_add(failed)?;
            summaries = summaries.checked_add(1)?;
            passed &= status == "ok" && failed == 0;
        }
    }
    if pending.is_some() || summaries == 0 {
        return None;
    }
    Some(Coverage {
        passed: passed && executed > 0,
        executed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVALID_UTF8_BYTE: u8 = u8::MAX;

    #[test]
    fn complete_nonempty_summary_and_negative_grammar_controls() {
        let good = "running 1 test\n.\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        assert_eq!(
            coverage(good.as_bytes()),
            Coverage {
                passed: true,
                executed: 1
            }
        );
        for invalid in [
            String::new(),
            good.replace("running 1 test", "running 2 tests"),
            good.replace("1 passed", "0 passed"),
            good.replace("ok.", "FAILED."),
            good.replace("running 1 test\n", ""),
            format!("{good}running 2 tests\n"),
            good.replace("1 passed", "18446744073709551616 passed"),
            good.trim_end().into(),
            good.replace("0.00s", ""),
            good.replace("0.00s", "0."),
            good.replace("0.00s", "NaNs"),
            good.replace("0.00s", "0.00s unreviewed"),
            good.replace("0 ignored; 0 measured", "0 measured; 0 ignored"),
            good.replace("0 measured; ", ""),
            good.replace("0 measured; ", "0 measured; 0 retired; "),
        ] {
            assert!(!coverage(invalid.as_bytes()).passed);
        }
        let empty = good
            .replace("running 1 test", "running 0 tests")
            .replace("1 passed", "0 passed");
        assert!(!coverage(empty.as_bytes()).passed);
        assert!(!coverage(&[INVALID_UTF8_BYTE]).passed);
    }
}
