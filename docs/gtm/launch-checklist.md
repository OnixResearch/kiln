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
`428d03fcfb02478ed79d38453fb44f82c6a2bdd5` plus the uncommitted changes that
prepare this launch. The exact commands and outputs are retained in the
lifecycle packages `prepare-go-to-market` and `retain-heartwood-license-texts`.

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
- [ ] Owner: clear the product name. crates.io already carries unrelated
      crates named `kiln`, `kiln-core` and `kiln-cli`, so `cargo install`
      installs a different project, and the commercial plan lists name and
      domain clearance as an open gate. Launch copy must show no
      `cargo install` command.

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
- [ ] Owner: resolve `OnixResearch/kiln-public`. It is a second public
      repository holding a parentless projection of an older candidate, last
      pushed 2026-10-18, whose README is the previous status ledger. Archive
      it or point its README at the projection so that one entry point
      remains.
- [ ] Owner: add Kiln to the organization profile README. It is not listed.
- [ ] Owner: set the GitHub description and topics. Both are empty. The
      Radicle project description is a ready description. Set a homepage only
      if it is an owned URL.
- [ ] Owner: enable private vulnerability reporting. It is disabled, so the
      link in `SECURITY.md` and the conduct-report route in
      `CODE_OF_CONDUCT.md` are not yet working private channels.
- [ ] Owner: enable secret scanning and push protection. Both are disabled.
- [ ] Owner: decide branch protection for the projection's `main`. It is
      unprotected. A rule must still admit the maintainer's fast-forward
      projection push.
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
- [x] `cargo test --locked --workspace` exited 0: 1608 passed, 0 failed,
      5 ignored across 160 result lines, in 3m36s with a warm build cache.
- [x] `cargo test --locked -p kiln-adapter-github --features protocol-fixtures --test local_ci_e2e`
      passed 17 tests against isolated fixtures. It is not a live GitHub run.
- [ ] Release owner: build the packaged flake output with
      `nix build path:$PWD#kiln` and run `result/bin/kiln demo`. Not completed
      for this preparation; do not document `nix run` as exercised until it
      is.
- [ ] Release owner: repeat the quick start from a credential-isolated
      checkout of the public route, with an empty `HOME` and no Git or SSH
      credentials. The retained contributor check covers an earlier revision.
- [ ] `aarch64-linux` is declared by the flake and was not checked.
- [ ] Soft: `kiln --help` prints `unsupported command: --help` and exits 1,
      while the other shipped programs accept `--help`; `kiln` has no
      `--version`. Fixing it needs a specification-bearing change. Launch
      copy must not tell a reader to run either.
- [ ] Soft: `cargo test --locked --workspace` left untracked
      `vendor/artifact/Cargo.lock` and `vendor/valence/Cargo.lock` in the
      working tree. Decide whether to ignore, track or stop generating them.

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
- [x] `cairn traceability coverage`: pass, 1742 referenced, none missing,
      none dangling. This is declared linkage, not execution truth.
- [x] `cairn release-readiness --root .`: `valid: true`, `verdict: pass`,
      `onix_release_ready: false`. Skipped or absent inputs: release-evidence
      index freshness, semantic review ledger, stack provenance, function
      address and the workspace contract matrix.
- [ ] `nix build path:$PWD#checks.x86_64-linux.octet`: not run.
- [ ] `nix flake check path:$PWD --no-write-lock-file -L`: not run. It has 45
      checks. Record its result on the release commit or state that it was
      not run.
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
- [ ] Test the private reporting link after enabling it.

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
- [ ] Private vulnerability reporting is enabled and its link was tested.
- [ ] `OnixResearch/kiln-public` no longer competes as an entry point.
- [ ] The release notes carry the tagged revision's results and have a named
      approver; the announcement has a named approver.
- [ ] A review confirms that no asset presents hosted CI as available or
      claims production or isolation qualification.
- [ ] A rollback owner is named.

Soft gates:

- [ ] GitHub description and topics are set; the organization profile lists
      Kiln.
- [ ] The contribution route is confirmed.
- [ ] `nix build path:$PWD#kiln` and the full flake check passed on the
      release commit.
- [ ] `kiln --help` prints usage.
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

# 3. Set projection metadata and security settings.
gh repo edit OnixResearch/kiln \
  --description "Provider-neutral CI control plane" \
  --add-topic ci --add-topic continuous-integration --add-topic nix \
  --add-topic rust --add-topic radicle --add-topic jujutsu \
  --add-topic self-hosted
gh api --method PUT repos/OnixResearch/kiln/private-vulnerability-reporting
gh api --method PATCH repos/OnixResearch/kiln \
  -f 'security_and_analysis[secret_scanning][status]=enabled' \
  -f 'security_and_analysis[secret_scanning_push_protection][status]=enabled'

# 4. Read the settings back.
gh api repos/OnixResearch/kiln/private-vulnerability-reporting
gh repo view OnixResearch/kiln --json description,repositoryTopics

# 5. Create the release from the reviewed notes. Replace relative links in
#    the notes first, as the notes describe.
gh release create v0.1.0 --repo OnixResearch/kiln --verify-tag \
  --title "Kiln 0.1.0" --notes-file docs/gtm/release-notes-0.1.0.md
```

Configure branch protection in the repository settings. None of these
commands was run during preparation.
