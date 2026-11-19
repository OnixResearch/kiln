//! Pure binding admission, not token verification or execution authority.

/// Explicit identity and time facts for one observation of existing authority.
#[derive(Clone, Copy)]
pub struct Input {
    pub authorized_token: [u8; super::HOLDER_INVOCATION_DIGEST_BYTES],
    pub candidate_token: [u8; super::HOLDER_INVOCATION_DIGEST_BYTES],
    pub authorized_at_seconds: u64,
    pub observed_at_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    TokenMismatch,
    TimeRegression,
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::TokenMismatch => "recheck token differs from admitted token",
            Self::TimeRegression => "recheck time precedes initial verification",
        })
    }
}

impl core::error::Error for Error {}

/// Admit the same token and a time not earlier than initial verification.
///
/// # Errors
/// Rejects token substitution or time regression. Success grants no authority.
// r[impl ucan.holder_recheck.binding]
pub fn admit(input: Input) -> Result<(), Error> {
    if input.authorized_token != input.candidate_token {
        return Err(Error::TokenMismatch);
    }
    if input.observed_at_seconds < input.authorized_at_seconds {
        return Err(Error::TimeRegression);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn input(observed_at_seconds: u64) -> super::Input {
        super::Input {
            authorized_token: [7; 32],
            candidate_token: [7; 32],
            authorized_at_seconds: 100,
            observed_at_seconds,
        }
    }

    #[test]
    fn same_token_accepts_initial_and_later_time() {
        assert_eq!(super::admit(input(100)), Ok(()));
        assert_eq!(super::admit(input(u64::MAX)), Ok(()));
    }

    #[test]
    fn substitution_and_time_regression_are_separate_denials() {
        let mut changed = input(101);
        changed.candidate_token[0] ^= 1;
        assert_eq!(super::admit(changed), Err(super::Error::TokenMismatch));
        assert_eq!(super::admit(input(99)), Err(super::Error::TimeRegression));
    }
}
