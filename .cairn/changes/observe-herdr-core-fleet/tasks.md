## Phase 1: Implementation

- [x] [serial] Integrate the opt-in Core observer and machine view. r[kiln.fleet.herdr_core_observation]
- [x] [serial] Verify bounded reconnect and browser presentation. r[kiln.fleet.herdr_core_observation]

## Verification evidence

- Nickel 1.18 exported both the legacy profile and the opt-in profile with two distinct aliases; it refused the wrong version, duplicate aliases and an unenrolled host.
- Chromium rendered Core reporting and unknown program status separately from legacy sessions and exact assignment counts. Stale and unreachable rows showed no live counts; an unmapped peer showed “Not selected.” A synthetic roster HTTP 503 left Cairn and work-source sections visible.
- Eight focused Core tests, eight Fleet service tests and five web proxy tests passed. Fixtures exercise remote line loss, omission of an ambiguous empty remote result, handshake rejection, bounded frames, missed events and redaction.
- A disposable Herdr Core 0.1.19 on an isolated Unix socket initially reported zero live local terminals. Fleet refreshed to one OSC 7501 `blocked` terminal and one `unknown` terminal, kept an unmatched alias unreported, dropped counts when Core stopped, and recovered a fresh zero after reconnect. No real remote Core or live deployment was exercised.
