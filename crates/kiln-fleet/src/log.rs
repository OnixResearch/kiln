//! Structured log: one JSON object per line on stderr.

use std::io::Write;

use serde_json::{Map, Value};

/// Wall-clock milliseconds since the Unix epoch; zero if the clock is before it.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// Write `{"ts_ms", "level", "event", ...fields}` as one line.
pub fn event(level: &str, event: &str, fields: Value) {
    let mut line = Map::new();
    line.insert("ts_ms".into(), Value::from(now_ms()));
    line.insert("level".into(), Value::from(level));
    line.insert("event".into(), Value::from(event));
    if let Value::Object(fields) = fields {
        line.extend(fields);
    }
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{}", Value::Object(line));
}

pub fn info(name: &str, fields: Value) {
    event("info", name, fields);
}

pub fn warn(name: &str, fields: Value) {
    event("warn", name, fields);
}
