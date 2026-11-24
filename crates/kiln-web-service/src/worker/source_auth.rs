//! Shared source-authority enforcement helper for worker materialization and writeback.

use kiln_core::implementation::authority::AuthorityProfile;
use kiln_core::projection::authority::{
    EffectiveSourceScope, SourceAuthority, SourceAuthorityRequest, SourceCurrentness, check_request,
};

/// How a caller supplies an [`AuthorityProfile`] for each [`SourceAuthorityRequest`].
pub enum RequestProfile<'a> {
    /// Fixed profile (for example `LocalOperator`, `Missing`, or a captured/forged credential).
    Static(&'a AuthorityProfile),
    /// Per-request signer (for example `BasaltSourceAuthority` + `SourceSigner`).
    Signer(&'a dyn Fn(&SourceAuthorityRequest) -> Result<AuthorityProfile, String>),
}

impl std::fmt::Debug for RequestProfile<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static(profile) => formatter.debug_tuple("Static").field(profile).finish(),
            Self::Signer(_) => formatter.write_str("Signer(<fn>)"),
        }
    }
}

/// Authority context required at materialization, writeback, and result boundaries.
pub struct SourceAuthorization<'a> {
    pub authority: &'a mut dyn SourceAuthority,
    pub profile: RequestProfile<'a>,
    pub current: &'a SourceCurrentness,
    pub now_unix_s: u64,
}

impl std::fmt::Debug for SourceAuthorization<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceAuthorization")
            .field("profile", &self.profile)
            .field("current", &self.current)
            .field("now_unix_s", &self.now_unix_s)
            .finish_non_exhaustive()
    }
}

impl SourceAuthorization<'_> {
    /// Admit, revalidate, and `check_request` one exact [`SourceAuthorityRequest`],
    /// returning its confirmed [`EffectiveSourceScope`] and closing the transient
    /// session afterwards so large manifests never exhaust `MAX_SOURCE_SESSIONS`.
    ///
    /// # Errors
    /// Returns a redacted diagnostic message on any admission, revalidation, or
    /// pure `check_request` refusal.
    pub fn authorize(
        &mut self,
        request: &SourceAuthorityRequest,
    ) -> Result<EffectiveSourceScope, String> {
        let owned_profile;
        let profile = match &self.profile {
            RequestProfile::Static(profile) => *profile,
            RequestProfile::Signer(sign) => {
                owned_profile = sign(request)?;
                &owned_profile
            }
        };
        let decision = self
            .authority
            .admit(request, profile, self.current, self.now_unix_s)
            .map_err(|denial| {
                format!(
                    "source authority denied ({}): {}",
                    denial.ability,
                    denial.reason.as_str()
                )
            })?;
        let session = decision
            .session
            .ok_or_else(|| "source authority decision returned no session".to_string())?;
        if let Err(denial) =
            self.authority
                .revalidate(session, request, self.current, self.now_unix_s)
        {
            return Err(format!(
                "source authority revalidation denied ({}): {}",
                denial.ability,
                denial.reason.as_str()
            ));
        }
        let scope = self
            .authority
            .effective_scope(session)
            .cloned()
            .ok_or_else(|| "source authority session vanished after revalidation".to_string())?;
        let check = check_request(&scope, request, self.current, self.now_unix_s);
        self.authority.close(session);
        check.map_err(|reason| format!("source authority check denied: {}", reason.as_str()))?;
        Ok(scope)
    }
}
