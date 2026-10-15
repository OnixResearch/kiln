/// Named canonicalization profile. Callers must choose a profile explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalHashProfile {
    JsonValue,
    PreservesText,
}

impl CanonicalHashProfile {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::JsonValue => "json_value",
            Self::PreservesText => "preserves_text",
        }
    }
}
