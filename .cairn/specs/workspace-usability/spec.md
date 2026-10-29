# Workspace Usability Specification

## Purpose

Defines the `workspace-usability` capability.

## Requirements

### Requirement: Workflow-first work navigation

r[kiln.work.usability]
The live workspace MUST distinguish issues, planning and CI navigation, offer page-specific creation, and keep secondary filters and advanced metadata available through labelled disclosures. It MUST preserve existing URL restoration, saved-view precedence, selected identities, exact-head confirmation and authentication boundaries. All supported work types MUST remain reachable.

#### Scenario: Capture and browse

- GIVEN an admitted writable disposable work store
- WHEN a user opens Issues, creates an issue, browses planning categories or applies a saved board
- THEN the visible workflow MUST match its destination and the persisted signed state MUST match confirmed fields

#### Scenario: Unavailable or read-only work

- GIVEN unavailable storage, an empty or filtered snapshot, or a read-only session
- WHEN the user opens the workspace
- THEN unavailable storage MUST remain an explicit failure, empty results MUST offer contextual guidance, and no mutation authority or replacement selection MUST be invented

### Requirement: Navigable live workspace
r[kiln.dashboard.usability]

The live workspace MUST expose readable summaries and discoverable common actions without requiring users to copy opaque identities for ordinary run selection. Exact identities MUST remain inspectable. Responsive navigation MUST support keyboard dismissal and visible focus. Loading, missing selections, failed reads and absent reports MUST stay explicit, without replacing selected facts or granting authority.
