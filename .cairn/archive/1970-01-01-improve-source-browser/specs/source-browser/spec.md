## ADDED Requirements

### Requirement: Code-host source navigation

r[kiln.dashboard.source.navigation]
The Source view MUST provide repository and revision pickers, a breadcrumb, and Files, History and Changes views selected by the URL. The Files view MUST list directories before files, each with an icon and a humanized size. On wide screens it MUST show the selected file beside the tree.

The file viewer MUST show linkable line numbers. It MAY apply bounded, presentation-only syntax highlighting. The rendered text MUST equal the retained text exactly. Copy and download actions MUST use only the retained text. No file, revision or line MUST be selected unless the URL or the reader selects it.

#### Scenario: Line anchor

r[kiln.dashboard.source.navigation.scenario.anchor]
- GIVEN a URL naming a source, revision, file and line range
- WHEN the reader opens or reloads it
- THEN the same file MUST render with exactly that range highlighted and scrolled into view
- AND a line outside the file MUST be reported, not substituted

#### Scenario: Revision changes

r[kiln.dashboard.source.navigation.scenario.changes]
- GIVEN a pinned Git or Jujutsu revision
- WHEN the reader opens Changes
- THEN each changed file MUST be listed with its status and +/- counts
- AND its hunks MUST show old and new line numbers with add/delete markers that do not rely on colour
- AND truncation MUST be stated
- AND for a Pijul source, Changes MUST state that historical changes are not readable
