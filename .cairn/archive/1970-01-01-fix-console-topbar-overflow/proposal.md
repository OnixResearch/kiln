## Why

The pinned Bombadil rail confirmed a real topbar regression: Raw collaboration history renders a 1071px document at a 1024px viewport. Its long breadcrumb and non-shrinking account controls force intrinsic grid tracks beyond the available header width. The operator requested fixing this defect, verifying the four target widths, rerunning the unchanged owned/deployed browser rail, and diagnosing the build host's unexpected automatic-GC request before another build.

## What Changes

- Make topbar tracks and account controls shrink without losing visible location, account access, search or running-state controls; keep the existing mobile navigation behavior.
- Diagnose configured GC thresholds and confirm capacity using read-only observations. Keep automatic GC disabled on every Nix invocation used by this work. Do not change host policy, run shared cleanup, deploy the fix or restart managed services without separate authorization.
- Rebuild the selected example, exercise 390/768/1024/1440px layouts and the original long-location scenario, then retain unchanged Bombadil owned/deployed outcomes and process-preservation evidence.

## Impact

- **Consumer and outcome:** operators/contributors can navigate the example console without the confirmed page-level overflow.
- **Durable capability and owner:** a regression-covered responsive header, owned by Kiln web-service/dashboard maintainers; host GC policy remains operator-owned.
- **Files:** `crates/kiln-web-service/src/web.css`, relevant operator/changelog documentation, active coverage enrollment and this native lifecycle package. The Bombadil property specification stays unchanged.
- **Testing:** actual browser geometry and reachable controls at all four widths, unchanged bounded Bombadil campaigns, owned teardown, protected service identities, native lifecycle gates and declared active traceability.
- **Non-claims:** no exhaustive UI coverage, CI/provider correctness, hostile isolation, production/release acceptance or automatic deployment.
