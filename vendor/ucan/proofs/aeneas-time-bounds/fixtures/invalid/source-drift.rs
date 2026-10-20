// Negative fixture only: this is not production source and must never be proof-selected.
#[derive(Clone, Copy)]
pub struct TimeBounds {
    not_before_seconds: u64,
    expires_at_seconds: u64,
}

impl TimeBounds {
    pub fn new(not_before_seconds: u64, expires_at_seconds: u64) -> Option<Self> {
        (not_before_seconds <= expires_at_seconds).then_some(Self {
            not_before_seconds,
            expires_at_seconds,
        })
    }
}
