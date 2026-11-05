//! Pure host preflight facts for the Firecracker runner.
//!
//! Shells read kernel, SMT, KSM, MPTCP and resctrl inputs. This module only
//! classifies those observations and decides whether an `untrusted_capable`
//! runner may admit work. It performs no I/O and grants no isolation claim.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// CVE-2026-98432 status derived from a checked-in fixed-release table plus
/// explicitly reviewed vendor package backports.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CveStatus {
    Fixed,
    Vulnerable,
    Unknown,
}

/// Upstream fixed releases: (major, minor, patch) for the first fixed patch of
/// each supported series. A later release in the same series is fixed.
const FIXED_RELEASES: &[(u64, u64, u64)] = &[
    (7, 3, 0),
    (7, 2, 10),
    (6, 18, 56),
    (6, 12, 113),
    (6, 6, 159),
    (6, 1, 190),
    (5, 15, 223),
    (5, 10, 272),
];

/// Host facts recorded before a Firecracker attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostFacts {
    pub host_kernel_release: String,
    pub host_kernel_package: String,
    pub cve_2026_98432: CveStatus,
    pub smt_active: bool,
    pub ksm_running: bool,
    pub net_mptcp_enabled: Option<bool>,
    pub tcp_migrate_req: Option<bool>,
    pub mptcp_listeners: Option<u32>,
    pub resctrl_mounted: bool,
}

impl HostFacts {
    /// Derive the CVE status and retain the remaining observed host facts.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn observed(
        host_kernel_release: impl Into<String>,
        host_kernel_package: impl Into<String>,
        backports: &BTreeMap<String, CveStatus>,
        smt_active: bool,
        ksm_running: bool,
        net_mptcp_enabled: Option<bool>,
        tcp_migrate_req: Option<bool>,
        mptcp_listeners: Option<u32>,
        resctrl_mounted: bool,
    ) -> Self {
        let host_kernel_release = host_kernel_release.into();
        let host_kernel_package = host_kernel_package.into();
        let cve_2026_98432 = classify_kernel(&host_kernel_release, &host_kernel_package, backports);
        Self {
            host_kernel_release,
            host_kernel_package,
            cve_2026_98432,
            smt_active,
            ksm_running,
            net_mptcp_enabled,
            tcp_migrate_req,
            mptcp_listeners,
            resctrl_mounted,
        }
    }

    /// `Some(reason)` means an `untrusted_capable` runner must refuse unless an
    /// explicit operator override is recorded.
    #[must_use]
    pub fn untrusted_refusal(&self) -> Option<String> {
        if self.cve_2026_98432 != CveStatus::Fixed {
            return Some(format!(
                "host kernel CVE-2026-98432 is {} (release {})",
                match self.cve_2026_98432 {
                    CveStatus::Fixed => "fixed",
                    CveStatus::Vulnerable => "vulnerable",
                    CveStatus::Unknown => "unknown",
                },
                self.host_kernel_release
            ));
        }
        if self.smt_active {
            return Some("host SMT is active".into());
        }
        if self.ksm_running {
            return Some("host KSM is running".into());
        }
        if self.net_mptcp_enabled == Some(false) && self.mptcp_listeners.unwrap_or(0) > 0 {
            return Some("MPTCP listeners remain after disabling the sysctl".into());
        }
        None
    }

    /// Admit an untrusted runner. A non-empty operator override is retained by
    /// the caller's receipt; this function only decides admission.
    pub fn admit_untrusted(&self, override_reason: Option<&str>) -> Result<(), String> {
        match self.untrusted_refusal() {
            None => Ok(()),
            Some(_) if override_reason.is_some_and(|reason| !reason.trim().is_empty()) => Ok(()),
            Some(reason) => Err(reason),
        }
    }
}

/// Classify a kernel release and package against CVE-2026-98432.
#[must_use]
pub fn classify_kernel(
    release: &str,
    package: &str,
    backports: &BTreeMap<String, CveStatus>,
) -> CveStatus {
    if !package.is_empty()
        && let Some(status) = backports.get(package)
    {
        return *status;
    }
    let Some((major, minor, patch)) = parse_release(release) else {
        return CveStatus::Unknown;
    };
    if (major, minor, patch) >= (7, 3, 0) {
        return CveStatus::Fixed;
    }
    for &(series_major, series_minor, fixed_patch) in FIXED_RELEASES {
        if (major, minor) == (series_major, series_minor) {
            return if patch >= fixed_patch {
                CveStatus::Fixed
            } else {
                CveStatus::Vulnerable
            };
        }
    }
    CveStatus::Unknown
}

fn parse_release(release: &str) -> Option<(u64, u64, u64)> {
    let mut parts = release.split(|character: char| !character.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backports() -> BTreeMap<String, CveStatus> {
        BTreeMap::from([("6.18.42-1vendor1".into(), CveStatus::Fixed)])
    }

    fn facts(release: &str, package: &str) -> HostFacts {
        HostFacts::observed(
            release,
            package,
            &backports(),
            false,
            false,
            Some(true),
            Some(false),
            Some(0),
            true,
        )
    }

    #[test]
    fn fixed_vulnerable_vendor_and_unparseable_releases_are_distinct() {
        assert_eq!(
            classify_kernel("6.18.56", "", &backports()),
            CveStatus::Fixed
        );
        assert_eq!(
            classify_kernel("6.18.42", "", &backports()),
            CveStatus::Vulnerable
        );
        assert_eq!(
            classify_kernel("6.18.42", "6.18.42-1vendor1", &backports()),
            CveStatus::Fixed
        );
        assert_eq!(
            classify_kernel("unparseable", "", &backports()),
            CveStatus::Unknown
        );
    }

    #[test]
    fn untrusted_admission_refuses_smt_ksm_and_unfixed_kernel_with_override() {
        let mut fixed = facts("6.18.56", "");
        assert!(fixed.admit_untrusted(None).is_ok());
        fixed.smt_active = true;
        assert!(fixed.untrusted_refusal().is_some());
        assert!(fixed.admit_untrusted(Some("operator accepted")).is_ok());
        fixed.smt_active = false;
        fixed.ksm_running = true;
        assert!(fixed.admit_untrusted(None).is_err());
        fixed.ksm_running = false;
        fixed.cve_2026_98432 = CveStatus::Vulnerable;
        assert!(fixed.admit_untrusted(None).is_err());
    }

    #[test]
    fn mptcp_listeners_with_disabled_sysctl_are_not_mitigated() {
        let mut facts = facts("6.18.56", "");
        facts.net_mptcp_enabled = Some(false);
        facts.mptcp_listeners = Some(1);
        assert!(facts.untrusted_refusal().is_some());
        facts.mptcp_listeners = Some(0);
        assert!(facts.untrusted_refusal().is_none());
    }
}
