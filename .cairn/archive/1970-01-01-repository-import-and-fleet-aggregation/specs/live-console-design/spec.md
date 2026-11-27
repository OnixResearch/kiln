## ADDED Requirements

### Requirement: Repositories and Fleet Copies workbench with guarded multi-forge import UI

r[kiln.web.repository_import_ui]
The live console MUST provide a **Repositories & Fleet Copies** view inside `3.2 System & Source` (linked from `1.1 Factory`, `?inspect=repo:<id>` Slide-Over Inspector, and `Ctrl+K` `> import repo`) displaying each repository's canonical Kiln identity, GitHub/Gitea/Radicle remotes and mirrors (`canonical` vs `mirror`), cross-fleet local working copies per machine (`vcs`, `head_commit`, `branch_or_bookmark`, `dirty`/`diverged`/`unavailable` badges, and active Cairn counts), and admission status (`Observed` vs `CI Configured` vs `Fleet Enrolled`). The **Import Repository / Add Mirror** flow MUST support GitHub, Gitea/Forgejo, and Radicle inputs, surface an explicit read-only Radicle non-mutation notice, and route all catalog mutations through `#control-dialog` Tier-2/3 confirmations.

#### Scenario: Operator imports a remote/mirror and inspects cross-fleet local copies

r[kiln.web.repository_import_ui.scenario.workbench]
- GIVEN an authenticated Operator viewing `3.2 System & Source` -> `Repositories`
- WHEN the Operator imports a GitHub or Gitea mirror onto an existing Radicle repository and opens `?inspect=repo:<id>` in the Slide-Over Inspector
- THEN the UI MUST require `#control-dialog` confirmation before issuing `POST /api/repositories/{id}/mirrors`
- AND the Slide-Over MUST display all remotes/mirrors alongside every machine's local copy status, explicitly flagging diverged, dirty, or offline hosts
