## Context

The signed work workspace already supports issue capture, planning, triage, private attention and durable views. Its generic record-oriented presentation obscures those workflows. Existing URL keys and authenticated bridge remain the contract.

## Decisions

### Decision: Workflow-first navigation

**Choice:** Label the work page Issues, separate Work/CI/Administration navigation groups and retain the existing page keys. Give each work page a direct creation action. Planning has explicit category navigation; additional creation types remain in a disclosure.

**Rationale:** Users can identify their task without knowing native storage types. Existing deep links, saved views and keyboard interaction continue to work.

### Decision: Progressive disclosure

**Choice:** Show search and its submit action, with secondary filters/layout/group/sort under a native details disclosure. Show title/description/status/priority first in creation forms; move ownership, relationships, scheduling and evidence to labelled disclosures. Existing values and submit/confirmation logic remain intact.

**Rationale:** Basic capture has a small visible form without removing advanced capabilities or weakening exact-value confirmation. Empty states distinguish no results from unavailable data and offer a page-specific action only when writable.

## Risks / Trade-offs

Hidden advanced controls need clear labels and active-filter summaries. Preserve selected identities even outside the current list; do not auto-select a replacement. Saved views and explicit URL overrides must keep their existing precedence. Read-only sessions may browse but must not gain mutation authority. Browser overrides prove candidate behavior only, not deployed bytes.

## Verification Strategy

Exercise desktop/mobile issue capture, all planning categories, secondary filters/reset, saved boards, keyboard commands and session/read-only boundaries. Use the real native disposable exercise for writes; never create production test records. Run the focused web-service suite and repository-required gates, retaining failures and non-claims.
