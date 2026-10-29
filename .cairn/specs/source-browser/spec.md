# Source Browser Specification

## Purpose

Defines the `source-browser` capability.

## Requirements

### Requirement: Bounded read-only source observation
r[kiln.dashboard.source.read]
Kiln MUST expose read-only log, tree and file reads for explicitly admitted local Git, Jujutsu and Pijul sources under named bounds, resolving Git and Jujutsu content from an exact commit in the repository's Git object store and reporting Pijul's current-channel and working-copy scope explicitly.

#### Scenario: Exact revision content
r[kiln.dashboard.source.read.scenario.git]
- GIVEN an admitted Git or Jujutsu source with at least two commits
- WHEN an admitted reader lists revisions, lists a tree and reads a file at an exact commit
- THEN each read MUST reflect that commit, not the head, and MUST preserve exact object ids, byte counts and text
- AND a missing path, invalid revision, traversal attempt or unadmitted source MUST be rejected explicitly

#### Scenario: Exact revision changes
r[kiln.dashboard.source.read.scenario.diff]
- GIVEN an admitted Git or Jujutsu source and an exact commit
- WHEN an admitted reader requests that revision's changes
- THEN the response MUST be the unified diff against its first parent, or an empty parent for a root commit, with file, insertion and deletion counts and explicit truncation
- AND a symbolic revision MUST be rejected and a Pijul request MUST be refused rather than approximated

#### Scenario: Pijul scope
r[kiln.dashboard.source.read.scenario.pijul]
- GIVEN an admitted Pijul source with a configured channel and executable
- WHEN the executable is unavailable or a revision is selected
- THEN the read MUST report explicit unavailability or an unsupported revision rather than an empty success or substituted content

### Requirement: Separate source policy authority
r[kiln.dashboard.source.authority]
Kiln MUST admit source repositories and executables through a separate typed runtime policy, authorize only authenticated viewer/operator reads, and keep absolute paths, executables and object-store locations out of browser responses.

#### Scenario: Unadmitted authority is requested
r[kiln.dashboard.source.authority.scenario.policy]
- GIVEN a relative path or executable, an unknown kind, a duplicate identity, a channel on a Git source or a worker/write/anonymous request
- WHEN admission or a read is attempted
- THEN it MUST be rejected before any repository or process effect

### Requirement: Integrated source view
r[kiln.dashboard.source.view]
The live workspace MUST provide a Source view with source selection, bounded revision paging, a breadcrumb directory browser, bounded file rendering, line-numbered text, revision changes with addition and deletion highlighting, client-side entry filtering, up-level and newer/older revision navigation, and URL-restored selection.

#### Scenario: Source selection is restored
r[kiln.dashboard.source.view.scenario.restore]
- GIVEN source, revision, directory and file selections in the page URL
- WHEN the reader opens, reloads or navigates history
- THEN the page MUST restore that exact selection without substituting another repository, revision or file
