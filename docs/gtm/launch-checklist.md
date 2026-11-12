# Launch checklist

Status values:

- `[x]` verified in the repository or observed directly on the stated date;
- `[ ]` open, not run, or an owner-only action;
- `[~]` partly verified; the note names what remains.

Observations are dated. Re-observe before launch: remote state and command
results change. Do not publish the release, the announcement or outreach until
every hard gate in [section 10](#10-gono-go-review) is `[x]`.

Unless a row says otherwise, local results were observed on 2026-11-12 on one
`x86_64-linux` host, inside `nix develop path:$PWD`, at source revision
`428d03fcfb02478ed79d38453fb44f82c6a2bdd5` plus the unpublished changes that
prepare this launch. The exact commands and outputs are retained in the
lifecycle packages `prepare-go-to-market`, `retain-heartwood-license-texts`,
`adopt-onix-package-prefix`, `resolve-launch-channel-blockers` and
`conform-aspen-host-to-octet`, and in the archived packages
`add-operator-cli-help` and `add-operator-cli-version`.

## 1. Release identity and legal

- [x] Version is `0.1.0` in `Cargo.toml` `[workspace.package]`.
- [x] `CHANGELOG.md` carries a `0.1.0` entry marked unreleased.
- [x] The license map `LICENSES.md` states the mixed-license tree, the
      `MIT OR Apache-2.0` default, the `GPL-2.0-only` Pijul adapter and the
      linked `AGPL-3.0-or-later` and `MPL-2.0` components.
- [x] `vendor/heartwood-canonical/LICENSE-MIT` and `LICENSE-APACHE` are now
      retained. The license map linked both, but neither had ever been
      imported; the importer now extracts and digest-verifies them from the
      pinned heartwood commit. Accepted requirement
      `kiln.radicle.canonical.provenance` says "Licenses MUST be retained."
- [x] Every relative link in `LICENSES.md` resolves (52 links).
- [ ] Release owner: review the license map against the exact tagged tree.
      The optional Onix discovery cohort still records `NOASSERTION` wrappers;
      default builds do not acquire it.
- [ ] Release owner: decide source-only or source plus binaries. No binary,
      container image or crates.io publication is prepared; 36 of the 46
      workspace crates set `publish = false`. Recommendation: source only.
- [x] The two packages that collided with crates.io are renamed: package
      `onix-kiln-core` in `crates/kiln-core` and package `onix-kiln-cli` in
      `crates/kiln-cli`. The library `kiln_core`, the binary `kiln` and both
      directories keep their names. On 2026-11-12 crates.io returned not
      found for all 54 first-party package names and still carried unrelated
      `kiln` 0.0.0, `kiln-core` 0.1.1 and `kiln-cli` 0.1.1.
- [ ] Owner: clear the product name and domain. The unrelated `kiln` crate
      remains, and the commercial plan lists name and domain clearance as an
      open gate. Launch copy must show no `cargo install` command.
- [ ] Owner: when Cairn, OnixOS or the Kiln Aspen host move their pinned
      Kiln revision past the rename, declare
      `kiln-core = { package = "onix-kiln-core", ... }`. Their current pins
      predate it and need no change.

## 2. Public channels

- [x] The canonical repository `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` is public.
      The `seed.radicle.garden` API reported project name `Kiln`, description
      `Provider-neutral CI control plane`, one delegate, threshold one and
      `main` at `428d03fcfb02478ed79d38453fb44f82c6a2bdd5`.
- [x] Anonymous `git ls-remote` returned the same `main`,
      `428d03fcfb02478ed79d38453fb44f82c6a2bdd5`, from the Garden HTTPS
      projection and from `https://github.com/OnixResearch/kiln.git`.
- [x] The GitHub projection is public with issues enabled and discussions
      disabled.
- [ ] Owner: publish the launch preparation through the canonical procedure
      in `docs/hosting-and-recovery.md`. Until then the public tree shows the
      previous README and no security policy.
- [ ] Owner: tag `v0.1.0` on the verified commit. No tag and no release exist
      on either remote.
- [x] `OnixResearch/kiln-public` no longer competes as an entry point. It
      held a parentless projection of an older candidate with the previous
      README. Its visibility was changed to private on 2026-11-12; an
      anonymous page request returns HTTP 404 and an anonymous clone asks for
      credentials. It had no stars, forks or watchers. See
      [the withdrawal record](../hosting-and-recovery.md#withdrawal-of-the-historyless-projection).
- [ ] Owner: decide whether to delete `kiln-public`. Deletion is irreversible
      and needs the `delete_repo` token scope; it was not attempted.
- [x] The workspace catalog names `https://github.com/OnixResearch/kiln` as
      Kiln's GitHub source. `docs/catalog.ncl` and the generated guide in the
      workspace repository were corrected in its local commit `6cd7a307`.
      The guide was regenerated with the workspace's own renderer and checked
      with asciidoctor 2.0.26; the rendered site under `public/` there was
      not refreshed.
- [x] The organization profile lists Kiln. One row was appended to the
      Projects table of `profile/README.md` in `OnixResearch/.github`, commit
      `a8a54313`, on 2026-11-12 at the operator's direction, and read back.
      The anonymous organization page shows the link. Until the launch
      preparation is published, the link reaches the previous README.
- [x] The GitHub description is `Provider-neutral CI control plane`, the
      Radicle project description, and the topics are `ci`,
      `continuous-integration`, `jujutsu`, `nix`, `radicle`, `rust` and
      `self-hosted`. Both were set on 2026-11-12 at the operator's direction
      and read back. No homepage is set.
- [x] Private vulnerability reporting is enabled on the GitHub projection;
      the API read back `enabled: true` on 2026-11-12. The advisory form
      redirects an anonymous visitor to sign in. `SECURITY.md` and
      `CODE_OF_CONDUCT.md` name that form.
- [x] Secret scanning is enabled on the projection. It listed no alert at
      2026-11-12T20:13Z, four hours after it was enabled. Read the alert list
      again before launch.
- [x] Secret-scanning push protection is enabled, set on 2026-11-12 at the
      operator's direction and read back. It rejects a push that carries a
      detected secret pattern; GitHub prints a bypass link for a reviewed
      false positive.
- [x] The projection's `main` has a branch rule. Repository ruleset
      `27157794` was created on 2026-11-12 at the operator's direction and
      read back as active. It blocks deletion and non-fast-forward pushes and
      has no bypass actor. It requires no review, status check or signature,
      so a fast-forward projection push passes. The rule was not provoked
      with a refused push. A projection rollback needs the ruleset disabled
      first; see [the hosting record](../hosting-and-recovery.md#projection-branch-rule).
- [x] The README states the mixed license in its first table. GitHub's
      detector displays `AGPL-3.0` for the repository; the license map is
      the authority.

## 3. Install and demo smoke

- [x] `nix develop path:$PWD` enters the shell with `rustc 1.88.0`.
- [x] `cargo run --locked -p onix-kiln-cli -- demo` printed the one-line state
      shown in the README.
- [x] `cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview`
      wrote `journal.json` and `dashboard.html`. The page rendered in
      headless Chromium with six runs and seven attempts.
- [x] `cargo test --locked --workspace` exited 0: 1612 passed, 0 failed,
      5 ignored across 161 result lines, in 1m16s with a warm build cache,
      at local revision `95668b0b`, which holds the package rename and both
      operator CLI changes.
- [x] `cargo test --locked -p kiln-adapter-github --features protocol-fixtures --test local_ci_e2e`
      passed 17 tests against isolated fixtures. It is not a live GitHub run.
- [x] The packaged flake output builds and runs. `nix build` of the `kiln`
      package at local revision `95668b0b` exited 0 in 10m15s. Its check phase
      ran the workspace tests in the sandbox: 1612 passed, 0 failed,
      5 ignored. The output holds 21 programs; `kiln demo`, `kiln --help` and
      `kiln --version` from it behave as documented.
- [ ] Release owner: repeat the packaged build on the release commit. `nix
      run` itself was not exercised.
- [ ] Release owner: repeat the quick start from a credential-isolated
      checkout of the public route, with an empty `HOME` and no Git or SSH
      credentials. The retained contributor check covers an earlier revision.
- [~] `aarch64-linux`: the 44 checks and 11 packages the flake declares for
      it evaluate to derivations at local revision `0495510a`. None was built
      or run. `microvm-guest-surface` is declared for `x86_64-linux` only.
- [x] `kiln --help`, `kiln -h` and `kiln help` print the usage text and exit
      0; a missing command, an unsupported command and a help request with a
      trailing argument are refused on standard error. Requirement
      `kiln.cli.help` and `crates/kiln-cli/tests/cli.rs` cover it.
- [x] `kiln --version` and `kiln -V` print `kiln 0.1.0`; a trailing argument
      is a usage error. Requirement `kiln.cli.version` covers it. The other
      shipped programs have no version flag; launch copy must not tell a
      reader to run one.
- [ ] Soft: an earlier `cargo test --locked --workspace` run left untracked
      `vendor/artifact/Cargo.lock` and `vendor/valence/Cargo.lock` in the
      working tree. Two later runs on 2026-11-12 did not; the cause is not
      established. Decide whether to ignore them if they reappear.

## 4. Repository gates

The documented list is in [the operator guide](../operator.md#local-reference-run).

- [x] `cargo clippy --locked --workspace --all-targets -- -D warnings`: pass.
- [x] `cargo fmt --all --check`: pass.
- [x] `scripts/check-nickel-fixtures.sh`: pass, 78 positive and 567 negative
      fixtures, after a repair. At the parent revision it failed its format
      check on `config/github-ci.ncl` and `config/microvm-guest-surface.ncl`
      before running any fixture. `nickel format` was applied to those two
      files; the change is whitespace only.
- [x] `cairn validate`: pass.
- [x] `cairn traceability coverage`: pass, 1748 requirements, all
      referenced, none missing, none dangling. This is declared linkage, not
      execution truth.
- [x] `cairn release-readiness --root .`: `valid: true`, `verdict: pass`,
      `onix_release_ready: false`. Skipped or absent inputs: release-evidence
      index freshness, semantic review ledger, stack provenance, function
      address and the workspace contract matrix.
- [x] The aggregate Octet check, `checks.x86_64-linux.octet`, and its four
      consumer checks pass at local revision `0495510a`, each with exit
      status 0. At published `main` the aggregate fails: the Aspen host check
      reported 14 findings in three library modules added on 2026-11-11 and
      2026-11-12, and a further 71 appeared in the binary and two live tests
      once the library compiled. The local change
      `conform-aspen-host-to-octet` resolves them by layout and code changes.
      It adds one reasoned clock allowance at the host's shell boundary; the
      two live tests carry the allowances the host's other integration tests
      already carry. The host's 46 tests pass before and after, and its 52
      requirement markers are unchanged.
- [x] `checks.x86_64-linux.kiln-aspen-host-package` passes at local revision
      `0495510a`: exit status 0 in 12m15s, with the host's 46 tests passing
      in the sandbox.
- [x] Nix checks run at local revision `95668b0b`, each with exit status 0:
      `no-std`, `radicle-status-boundaries`, `celld-boundaries`,
      `aspen-runtime-octet-deny-all`, `radicle-broker-wire-octet-deny-all` and
      `kiln-radicle-nix-provider-octet-deny-all`. The first five are the
      checks the package rename touched.
- [ ] `nix flake check path:$PWD --no-write-lock-file -L`: not run. It has 45
      checks. On 2026-11-12 it would have rebuilt 1224 derivations, including
      Nix from source, two Rust toolchains and a NixOS VM test, on a store
      with about 40 GiB free. Record its result on the release commit or
      state that it was not run.
- [ ] Operator: the Nix store on the preparation host needs attention. It
      was below its 100 GiB `min-free` setting, so every store write started
      an automatic garbage collection, and one collection held the collector
      lock for hours while about fifty daemon workers queued behind it. The
      Nix runs recorded here used a pinned `git+file` revision and
      `--option min-free 0` for those invocations only, under a watchdog that
      stops a build before free space falls below a floor.
- [ ] Open qualification items in the retained 2026-11-08 readiness record
      remain open: the combined Web approval handoff and distinct-physical-host
      measurements. They bound hosted and isolation claims, not the source
      release, and no launch copy may claim them.

## 5. Public surfaces

- [x] The README is the landing surface and follows the outline in
      [positioning](positioning.md#landing-page-outline). It references no
      product domain; none is recorded in this repository.
- [x] The quick start sits above the capability map and gives both public
      acquisition routes.
- [x] `docs/README.md` indexes the documentation and `docs/status.md` keeps
      the detailed ledger relocated from the README.
- [x] `SECURITY.md`, `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md` exist in the
      tree. The organization `.github` repository already supplies issue
      templates, a pull-request template and `SUPPORT.md`.
- [x] Three broken links that predate this preparation are repaired: the
      Choregraph reference in the README, one anchor in `docs/hosted-ci.md`
      and one archived-evidence path in `docs/operator.md`.
- [ ] Owner: confirm the contribution route in `CONTRIBUTING.md`. It accepts
      Radicle patches and GitHub pull requests, integrated on canonical
      `main`. Observed use: the Radicle seed lists no issues and no patches;
      GitHub has 12 open issues and one open pull request, all by the
      maintainer.
- [ ] Optional: social preview image. No logo or image asset exists.

## 6. Launch assets

- [x] [Positioning](positioning.md), [launch plan](launch-plan.md),
      [release notes draft](release-notes-0.1.0.md),
      [announcement draft](announcement.md), [outreach copy](outreach.md),
      [demo script](demo.md) and [design-partner guide](design-partners.md)
      are written.
- [ ] Release owner: replace the qualification section of the release notes
      with the tagged revision's results.
- [ ] Maintainer: review the release notes, announcement and outreach against
      the [message rules](positioning.md#message-rules) and name an approver.
      Subagent drafts were checked against their sources during preparation;
      that check is not independent review.
- [ ] Content owner: rehearse [the demo](demo.md) on the tagged revision.
      A recording is optional.
- [ ] All external copy discloses maintainer status.

## 7. Security and privacy

- [~] A bounded source-hygiene review is retained for an earlier public
      revision in the public-release evidence. It was not rerun. Published
      history cannot be assumed removable.
- [ ] Release owner: run a redacting secret scan over the final tree and its
      reachable history. Record scanner version, command, revision and
      result. A clean scan is not privacy clearance.
- [x] The launch kit contains no personal name, private contact, customer
      name, price or credential.
- [ ] Confirm that demo recordings and issue templates contain no credential
      or private host detail.
- [ ] Test the private reporting form as an outside reporter. Reporting is
      enabled; the form itself was not exercised.

## 8. Community and support

- [ ] Name a support owner for the first 30 days in a private tracker.
- [ ] Open and pin the launch-feedback issue from
      [outreach](outreach.md#launch-feedback-issue-prompt).
- [ ] Decide triage labels. The projection has GitHub's default label set.
- [ ] Decide whether to publish a response expectation. Current wording
      everywhere: no response time or support commitment is offered.
- [ ] A second person reproduces the quick start before broad outreach.

## 9. Hosted-CI track

This track is separate and stays private. Nothing in the `0.1.0` launch may
present hosted CI as available, in beta, priced or open for signup.

- [x] The [design-partner guide](design-partners.md) and the hosted-CI email
      in [outreach](outreach.md#hosted-ci-design-partner-email) are prepared.
      Both carry the status sentence: bounded local implementation, not a
      launched paid service.
- [ ] L0: three qualified discovery interviews with credible paid-pilot
      intent, retained in private business records, plus the operator's
      choices of seller and jurisdiction, region and currency, first runner
      class, first model and exposure budget.
- [ ] L1 through L7 in [the commercial plan](../commercial-launch.md#6-dependency-ordered-launch-backlog)
      are all open. No company formation, merchant approval, published price
      or live charge exists.

## 10. Go/no-go review

Hard gates:

- [ ] The launch preparation and the license-text repair are published, and
      both public routes resolve the release commit.
- [ ] The documented repository gates were run on the release commit and
      each result is recorded, including failures and gates not run.
- [ ] The quick start and the demo pass on the release commit from a
      credential-isolated checkout.
- [~] Private vulnerability reporting is enabled. Its form was not tested as
      an outside reporter.
- [x] `OnixResearch/kiln-public` no longer competes as an entry point.
- [ ] The release notes carry the tagged revision's results and have a named
      approver; the announcement has a named approver.
- [ ] A review confirms that no asset presents hosted CI as available or
      claims production or isolation qualification.
- [ ] A rollback owner is named.

Soft gates:

- [x] GitHub description and topics are set, and the organization profile
      lists Kiln.
- [ ] The contribution route is confirmed.
- [ ] `nix build path:$PWD#kiln` and the full flake check passed on the
      release commit.
- [x] `kiln --help` prints usage.
- [ ] A second person reproduced the quick start.
- [ ] A demo recording exists.

Record the decision, date, approver and evidence links in a private launch
tracker. Do not publish the tracker.

## 11. Owner-only publish sequence

Run these only after the go/no-go review. Replace `<REVISION>` with the
verified commit. Stop on the first failure. Publishing the commit and the tag
follows `docs/hosting-and-recovery.md`; this list does not replace it.

```console
# 1. Tag the verified commit locally.
jj tag set v0.1.0 -r <REVISION>

# 2. Publish commit and tag through the canonical procedure, then read back
#    both public routes. Each must name <REVISION>.
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
  git ls-remote https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git \
  refs/heads/main refs/tags/v0.1.0
git ls-remote https://github.com/OnixResearch/kiln.git \
  refs/heads/main refs/tags/v0.1.0

# 3. Read the projection settings back. Description, topics, private
#    vulnerability reporting, secret scanning, push protection and the main
#    ruleset are already set.
gh api repos/OnixResearch/kiln/private-vulnerability-reporting
gh api repos/OnixResearch/kiln --jq .security_and_analysis
gh repo view OnixResearch/kiln --json description,repositoryTopics
gh api repos/OnixResearch/kiln/rules/branches/main

# 4. Create the release from the reviewed notes. Replace relative links in
#    the notes first, as the notes describe.
gh release create v0.1.0 --repo OnixResearch/kiln --verify-tag \
  --title "Kiln 0.1.0" --notes-file docs/gtm/release-notes-0.1.0.md
```

On 2026-11-12, at the operator's direction, the projection's description,
topics, private vulnerability reporting, secret scanning, push protection and
`main` ruleset were set, Kiln was added to the organization profile, and
`kiln-public` was made private. No tag, publication or release command above
was run during preparation.
