## Phase 1: Feed and OPML fuzzing

- [x] [serial] Add shrunk `fuzzing` bounds, XML control escaping, and the regression test. r[kiln.fuzz.feeds]
- [x] [serial] Add the feed/OPML/round-trip/URL fuzz crate, targets and seed corpus. r[kiln.fuzz.feeds]
- [x] [serial] Run the feed and OPML smoke targets and record the results. r[kiln.fuzz.feeds.smoke]

## Phase 2: Highlight fuzzing

- [x] [serial] Add the highlight/diff/structural fuzz crate, targets and seed corpus. r[kiln.fuzz.highlight]
- [x] [serial] Run the highlight, diff and structural smoke targets with ASan/UBSan settings and record the results. r[kiln.fuzz.highlight.smoke]

## Phase 3: Verification

- [x] [serial] Document the fuzz procedure, run focused tests, formatting, Clippy and Cairn validation, then record evidence. r[kiln.fuzz.nonclaims]
