//! Secret-like input screening ported from Lattice (#28).
//!
//! Refuses secret-like `relative_path` parameter names, `LATTICE_PARAM_*`
//! binding suffixes, and supplied values without echoing the supplied value.

const SECRET_WORDS: [&str; 17] = [
    "token",
    "tokens",
    "secret",
    "secrets",
    "password",
    "passwords",
    "passwd",
    "passphrase",
    "credential",
    "credentials",
    "apikey",
    "privatekey",
    "auth",
    "bearer",
    "cookie",
    "pat",
    "otp",
];

const KEY_QUALIFIERS: [&str; 8] = [
    "api",
    "private",
    "access",
    "secret",
    "session",
    "signing",
    "client",
    "encryption",
];

const TOKEN_RUN_MIN_BYTES: usize = 20;
const WORD_RUN_MEAN_BYTES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenAlphabet {
    Alphanumeric,
    UpperAlphanumeric,
    AlphanumericUnderscore,
    AlphanumericDash,
    Base64Url,
    Base64UrlDot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuffixRequirement {
    None,
    MinDigits(u8),
    UppercaseOrDigit,
}

impl SuffixRequirement {
    fn is_met(self, suffix: &str) -> bool {
        match self {
            Self::None => true,
            Self::MinDigits(minimum) => {
                suffix.bytes().filter(u8::is_ascii_digit).count() >= usize::from(minimum)
            }
            Self::UppercaseOrDigit => suffix
                .bytes()
                .any(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TokenFormat {
    prefix: &'static str,
    min_suffix_bytes: usize,
    alphabet: TokenAlphabet,
    requirement: SuffixRequirement,
}

const fn format(
    prefix: &'static str,
    min_suffix_bytes: usize,
    alphabet: TokenAlphabet,
) -> TokenFormat {
    requiring(prefix, min_suffix_bytes, alphabet, SuffixRequirement::None)
}

const fn requiring(
    prefix: &'static str,
    min_suffix_bytes: usize,
    alphabet: TokenAlphabet,
    requirement: SuffixRequirement,
) -> TokenFormat {
    TokenFormat {
        prefix,
        min_suffix_bytes,
        alphabet,
        requirement,
    }
}

const SLACK_MIN_DIGITS: SuffixRequirement = SuffixRequirement::MinDigits(10);

const TOKEN_FORMATS: [TokenFormat; 27] = [
    format("ghp_", 30, TokenAlphabet::Alphanumeric),
    format("gho_", 30, TokenAlphabet::Alphanumeric),
    format("ghu_", 30, TokenAlphabet::Alphanumeric),
    format("ghs_", 30, TokenAlphabet::Alphanumeric),
    format("ghr_", 30, TokenAlphabet::Alphanumeric),
    format("github_pat_", 40, TokenAlphabet::AlphanumericUnderscore),
    requiring(
        "glpat-",
        20,
        TokenAlphabet::Base64UrlDot,
        SuffixRequirement::UppercaseOrDigit,
    ),
    requiring(
        "xapp-",
        20,
        TokenAlphabet::AlphanumericDash,
        SLACK_MIN_DIGITS,
    ),
    requiring(
        "xoxa-",
        20,
        TokenAlphabet::AlphanumericDash,
        SLACK_MIN_DIGITS,
    ),
    requiring(
        "xoxb-",
        20,
        TokenAlphabet::AlphanumericDash,
        SLACK_MIN_DIGITS,
    ),
    requiring(
        "xoxp-",
        20,
        TokenAlphabet::AlphanumericDash,
        SLACK_MIN_DIGITS,
    ),
    requiring(
        "xoxr-",
        20,
        TokenAlphabet::AlphanumericDash,
        SLACK_MIN_DIGITS,
    ),
    requiring(
        "xoxs-",
        20,
        TokenAlphabet::AlphanumericDash,
        SLACK_MIN_DIGITS,
    ),
    format("sk-proj-", 40, TokenAlphabet::Base64Url),
    format("sk-svcacct-", 40, TokenAlphabet::Base64Url),
    format("sk-", 32, TokenAlphabet::Alphanumeric),
    format("sk_live_", 20, TokenAlphabet::Alphanumeric),
    format("sk_test_", 20, TokenAlphabet::Alphanumeric),
    format("rk_live_", 20, TokenAlphabet::Alphanumeric),
    format("AKIA", 16, TokenAlphabet::UpperAlphanumeric),
    format("ASIA", 16, TokenAlphabet::UpperAlphanumeric),
    format("AIza", 30, TokenAlphabet::Base64Url),
    format("ya29.", 20, TokenAlphabet::Base64UrlDot),
    format("npm_", 30, TokenAlphabet::Alphanumeric),
    format("pypi-", 50, TokenAlphabet::Base64Url),
    format("hf_", 30, TokenAlphabet::Alphanumeric),
    format("eyJ", 30, TokenAlphabet::Base64UrlDot),
];

const fn in_alphabet(alphabet: TokenAlphabet, byte: u8) -> bool {
    match alphabet {
        TokenAlphabet::Alphanumeric => byte.is_ascii_alphanumeric(),
        TokenAlphabet::UpperAlphanumeric => byte.is_ascii_uppercase() || byte.is_ascii_digit(),
        TokenAlphabet::AlphanumericUnderscore => byte.is_ascii_alphanumeric() || byte == b'_',
        TokenAlphabet::AlphanumericDash => byte.is_ascii_alphanumeric() || byte == b'-',
        TokenAlphabet::Base64Url => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'),
        TokenAlphabet::Base64UrlDot => {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
        }
    }
}

fn is_provider_token(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    TOKEN_FORMATS.iter().any(|token| {
        let candidate = if in_alphabet(token.alphabet, b'.') {
            component
        } else {
            stem
        };
        candidate.strip_prefix(token.prefix).is_some_and(|suffix| {
            suffix.len() >= token.min_suffix_bytes
                && suffix.bytes().all(|byte| in_alphabet(token.alphabet, byte))
                && token.requirement.is_met(suffix)
        })
    })
}

pub(super) fn is_secret_identifier(identifier: &str) -> bool {
    let mut previous: Option<&str> = None;
    for word in identifier.split('_') {
        let is_secret_word = SECRET_WORDS
            .iter()
            .any(|secret| word.eq_ignore_ascii_case(secret));
        let is_qualified_key = word.eq_ignore_ascii_case("key")
            && previous.is_some_and(|qualifier| {
                KEY_QUALIFIERS
                    .iter()
                    .any(|known| qualifier.eq_ignore_ascii_case(known))
            });
        if is_secret_word || is_qualified_key {
            return true;
        }
        previous = Some(word);
    }
    false
}

pub(super) fn is_secret_value(value: &str) -> bool {
    value
        .split('/')
        .any(|component| is_provider_token(component) || has_token_shaped_run(component))
}

fn has_token_shaped_run(component: &str) -> bool {
    component.split('.').any(|piece| {
        let mut shape = RunShape::default();
        let is_alphanumeric = piece
            .bytes()
            .filter(|byte| !matches!(byte, b'-' | b'_'))
            .all(|byte| shape.push(byte));
        is_alphanumeric && shape.is_token_shaped()
    })
}

#[derive(Default)]
struct RunShape {
    length_bytes: usize,
    uppercase_bytes: usize,
    lowercase_bytes: usize,
    digit_bytes: usize,
    lowercase_runs: usize,
    is_after_lowercase: bool,
}

impl RunShape {
    fn push(&mut self, byte: u8) -> bool {
        let is_lowercase = byte.is_ascii_lowercase();
        if is_lowercase && !self.is_after_lowercase {
            self.lowercase_runs = self.lowercase_runs.saturating_add(1);
        }
        self.is_after_lowercase = is_lowercase;
        self.length_bytes = self.length_bytes.saturating_add(1);
        let class = if byte.is_ascii_uppercase() {
            &mut self.uppercase_bytes
        } else if is_lowercase {
            &mut self.lowercase_bytes
        } else if byte.is_ascii_digit() {
            &mut self.digit_bytes
        } else {
            return false;
        };
        *class = class.saturating_add(1);
        true
    }

    fn is_token_shaped(&self) -> bool {
        let has_mixed_classes =
            self.uppercase_bytes > 0 && self.lowercase_bytes > 0 && self.digit_bytes > 0;
        let is_word_free =
            self.lowercase_bytes < self.lowercase_runs.saturating_mul(WORD_RUN_MEAN_BYTES);
        self.length_bytes >= TOKEN_RUN_MIN_BYTES && has_mixed_classes && is_word_free
    }
}
