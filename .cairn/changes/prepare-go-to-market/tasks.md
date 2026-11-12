## Phase 1: Landing surface

- [x] [serial] Rework the README into the landing surface, relocate the status ledger, integration procedures and publication record verbatim to `docs/status.md`, and add the documentation index. a[prepare-go-to-market.landing]

## Phase 2: Community files

- [x] [serial] Add the security policy, contribution guide, code of conduct and changelog from repository facts. a[prepare-go-to-market.community]

## Phase 3: Launch kit

- [x] [serial] Write positioning, launch plan, release notes, announcement, outreach, demo script and the design-partner guide with cited bounds and must-not-claim rules. a[prepare-go-to-market.kit]
- [x] [serial] Record observed public-channel state, exercised commands and owner-only actions in the launch checklist. a[prepare-go-to-market.observed]

## Phase 4: Document verification

- [x] [serial] Check links and anchors, run the documented quick-start and demo commands, review positive and negative claim cases, validate lifecycle gates and retain bounded evidence. a[prepare-go-to-market.verification]

The packaged `nix build path:$PWD#kiln` output, the Octet check and the full
flake check were not exercised; `evidence/verification.json` lists them under
`not_exercised`. Publication, the tag, repository settings and outreach stay
open owner actions in `docs/gtm/launch-checklist.md`. This package is not
synchronized or archived.
