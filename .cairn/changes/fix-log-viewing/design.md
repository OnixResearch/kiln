## Context

Live reproduction on retained Nickel Eval stdout: the initial read showed 6,829 characters at byte cursor 6,832; reading again erased the output at EOF. Changing the literal search cleared text without fetching. Selecting diagnostic with a non-empty search sent an unsupported query. The retained Kiln stdout spans multiple 16 KiB UI reads.

## Decisions

### Decision: Commit displayed state only after admission

Read continuation is a pure comparison of exact selected key and requested/previous-next cursor. Append admitted contiguous chunks, including empty EOF reads. Explicit seeks replace only after success. Keep generation fencing, exact cursor validation, source/stream identity checks and existing display bounds. Failed reads keep previously admitted text with an explicit stale/error status.

### Decision: Keep the existing bounded API

No backend schema or provider changes. Search changes start a fresh read; diagnostic streams omit `q` and disable search while preserving the typed worker-stream search value. Add a read-from-start button rather than requiring manual cursor editing. Following scrolls only while the viewport is already at the tail; selection changes still invalidate pending reads and stop following.

## Risks / Trade-offs

- Display limits can discard old visible text; keep the warning and exact download path. Text character counts never become byte/native cursors.
- Empty reads do not imply complete output or success. Search skips are intentional selection, not fabricated source gaps.
- Browser verification uses candidate asset overrides and real GET responses only. It does not activate a deployment, start jobs or establish general backend correctness.

## Verification Strategy

Validate proposal/design/tasks gates before implementation. Verify exact concatenated multi-chunk output, repeated EOF, explicit rewind/seek, failed continuation retention, search changes, diagnostic reads with prior search, following and scroll position, stale stream responses, mobile controls and session retirement. Run syntax and focused service checks after integration; retain observed evidence here without adding source-text or mock-echo tests.
