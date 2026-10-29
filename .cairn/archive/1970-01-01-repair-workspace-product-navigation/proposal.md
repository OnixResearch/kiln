## Why

The deployed workspace presents implementation types, a generic creation selector and eight filter controls ahead of useful content. The operator reports that the UI makes no sense. Issues, planning and CI need distinct entry points and obvious next actions.

## What Changes

- Make Issues the work entry point, group sidebar destinations into Work, CI and Administration, and use human-readable titles without breaking existing URLs.
- Provide page-specific creation actions, planning categories and contextual empty states. Keep every supported work type accessible.
- Keep search visible; disclose secondary filters and advanced creation fields on demand. Preserve saved-view precedence, exact-head confirmation, session erasure and read-only admission.

## Impact

Current consumers are Kiln's authenticated internal developers and CI operators. Kiln web-service/dashboard maintainers own this frontend capability. The durable contribution is a repeatable issue-first workflow over the existing signed store, not a new storage or authority model. Files: web.html, web.js, web.css, work.js, work.css, command palette labels and operator documentation. Verification: actual desktop/mobile browser flows, disposable native work exercise, focused and repository-required gates.

## Out of Scope

No production data writes, deployment, publication, API/schema changes, CI dispatch, lifecycle acceptance or permission changes.
