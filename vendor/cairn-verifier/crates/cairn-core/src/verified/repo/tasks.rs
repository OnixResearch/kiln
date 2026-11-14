use super::TaskCounts;

pub fn count_tasks(markdown: &str) -> TaskCounts {
    let mut counts = TaskCounts {
        total: 0,
        done: 0,
        in_progress: 0,
        todo: 0,
        unmarked: 0,
    };
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let Some(state) = task_state(trimmed) else {
            continue;
        };
        counts.total += 1;
        increment_state(&mut counts, state);
        if !has_concurrency_marker(trimmed) {
            counts.unmarked += 1;
        }
    }
    counts
}

fn task_state(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
        return Some("done");
    }
    if trimmed.starts_with("- [~]") {
        return Some("in_progress");
    }
    if trimmed.starts_with("- [ ]") {
        return Some("todo");
    }
    None
}

fn increment_state(counts: &mut TaskCounts, state: &str) {
    match state {
        "done" => counts.done += 1,
        "in_progress" => counts.in_progress += 1,
        "todo" => counts.todo += 1,
        _ => debug_assert!(false, "unknown task state"),
    }
}

fn has_concurrency_marker(trimmed: &str) -> bool {
    trimmed.contains("[parallel]")
        || trimmed.contains("[serial]")
        || trimmed.contains("[depends:")
        || trimmed.contains("[after:")
}
