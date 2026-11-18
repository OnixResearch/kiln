## Context

Kiln `0.1.0` is published at `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE`. The GitHub repository `OnixResearch/kiln` is a public read-only projection with issues enabled. No tag or release exists on either remote. The crates set `publish = false`; distribution is source plus the Nix flake. `docs/commercial-launch.md` owns the hosted-compute plan and its open gates L0–L7. Sibling repositories Octet, Cairn and Lattice already carry launch kits, security policies, contribution guides and changelogs; Kiln follows their shape.

Accepted requirement `kiln.public_release.claims` governs public documentation: it must distinguish package-specific licenses, historical publication facts, current local verification and unperformed publication effects.

## Goals and Success Contract

A reader reaches a working first command and an honest capability map from the README without reading the evidence ledger. External copy has one reviewed source for messages, proof points and forbidden claims. Every action that needs operator authority is named, not performed. Completion means checked documents and retained observations, not a tag, announcement, enabled repository setting, launched service or closed commercial gate.

## Decisions

### Decision: The README is the landing surface

**Choice:** Lead `README.md` with positioning, the exercised quick start, a capability map and the claim boundary. Reference no product domain: none is recorded in this repository.

**Rationale:** The GitHub projection has no description, topics or homepage, so the README is the only public entry point. An unowned domain in launch copy is a dead link.

### Decision: Relocate the ledger verbatim

**Choice:** Move the implemented-area list, integration procedures and publication record from the README to `docs/status.md` without rewording. Change only relative link prefixes. Keep the `Quick start` heading that `docs/reference-pins.md` links to.

**Rationale:** That text carries reviewed claim boundaries and receipt links. Rewriting it risks claim drift; deleting it loses two publication records that `docs/hosting-and-recovery.md` does not repeat.

### Decision: Two launch tracks with separate gates

**Choice:** Treat the open-source `0.1.0` release and the hosted-CI pilot as independent tracks. The kit prepares the release and design-partner discovery only. `docs/commercial-launch.md` stays the single source for hosted gates; the kit links it and marks every hosted gate open.

**Rationale:** Local mechanisms do not close company, isolation, metering or collection gates. Copy that implies an available paid service would violate the commercial plan and the claim requirement.

### Decision: Proof points carry their bounds

**Choice:** Every proof point cites a repository document or retained receipt with its stated scope. Counts appear only with their scope sentence. The kit lists what the launch must not claim: production or hostile-isolation qualification, an available hosted service, an SLA, permissive-only licensing of combined artifacts, live Tangled or Gitea interoperability, and current validity of historical receipts.

**Rationale:** Retained receipts bind recorded cohorts and dates. Unbounded restatement turns them into false claims.

### Decision: Radicle stays canonical; GitHub carries issues and private reports

**Choice:** `CONTRIBUTING.md` names Radicle as canonical and accepts changes as Radicle patches or GitHub pull requests that maintainers integrate on canonical `main`. `SECURITY.md` uses GitHub private vulnerability reporting with a no-details public fallback, because that setting was observed disabled. The checklist asks the operator to confirm the route and enable the setting.

**Rationale:** The repository states the canonical host and the projection rule but no external contribution procedure. The chosen text follows those two facts and leaves the decision visible.

### Decision: Observe, do not mutate

**Choice:** The checklist records dated read-only observations of tags, GitHub metadata and security settings, and the exact commands exercised locally. Unchecked rows name owner-only actions with commands. This change performs no push, tag, release, setting change, announcement or payment action.

**Rationale:** Repository rules reserve remote mutation for explicit operator requests.

### Decision: No runtime change in this package

**Choice:** Keep CLI behavior unchanged. `kiln --help` printing `unsupported command: --help` is recorded as an open first-run item for a separate specification-bearing change.

**Rationale:** A code change needs its own requirement and traceability markers; an unsynchronized requirement would leave dangling references in the accepted-spec coverage gate.

### Decision: Repair only gate-blocking and link defects found on the way

**Choice:** Fix three broken links in existing documents and apply the canonical Nickel formatter to two configuration files that failed the fixture gate's format check. Record other findings in the checklist without changing them.

**Rationale:** A launch surface with broken links or a red documented gate contradicts the launch checklist. Both repairs are mechanical and change no meaning. The missing vendored license texts need an importer change and have their own package.

### Decision: No-spec planning classification

**Choice:** Use `no-spec-delta` with change-local acceptance and an explicitly agentic self-review of the hash-bound planning inputs.

**Rationale:** Documentation and launch copy do not amend accepted software behavior. The self-review is not independent, legal or release approval.

## Risks / Trade-offs

- A concise README hides detail one click deeper; the ledger remains complete in `docs/status.md` and the README links each area.
- Launch copy ages faster than code. The kit names its canonical sources and states that the source wins on conflict.
- Observed GitHub state can change after the recorded date; rows carry their observation date and must be re-observed before launch.
- The contribution and reporting routes are proposals until the operator confirms them.
- Drafting used delegated model output. Every claim was checked against its cited source before acceptance; model output is not evidence.

## Verification Strategy

Check every relative link and heading anchor in the repository documents. Confirm each link target of the previous README is still reachable from the README or `docs/status.md`. Run the documented quick-start and demo commands and record their results. Review positive cases (a reader can find install, demo, license map, security route, hosted-CI status) and negative cases (no production, isolation, SLA, paid-availability, permissive-only or live-interoperability claim; no historical receipt presented as current). Run the three planning gates, the classification review and `cairn validate`, retaining native outputs. Run the documented local gates and record each result, including failures and gates not run, as observations of the preparation tree; they are not release qualification.
