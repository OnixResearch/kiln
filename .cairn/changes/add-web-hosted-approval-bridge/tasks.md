## Phase 1: Supported bridge

- [x] [parallel] Carry the exact durable run/source/gate binding through Web dispatch and recovery; add explicit provider-composed server startup without a reverse dependency. r[kiln.web_hosted.binding]
- [x] [parallel] Implement dedicated-peer hosted approval/source admission, bounded exact source materialization and durable once-only attempt recovery. r[kiln.web_hosted.admission] r[kiln.web_hosted.replay]
- [x] [parallel] Implement real authenticated hosted WorkflowPort adapter, truthful outcome/log/cancellation mapping and runnable typed configuration. r[kiln.web_hosted.adapter]
- [x] [serial] Restore canonical Pending only from admitted exact live reconciliation, rejecting missing/substituted references during decision and replay without another dispatch. r[kiln.domain.attempts]

## Phase 2: Qualification

- [x] [serial] Exercise the actual local HTTPS-to-Iroh approval path and negative/stale/replay/restart cases; retain scoped evidence without waiving host safety. r[kiln.web_hosted.qualification]
- [x] [serial] Update operator documentation and run applicable compiled, configuration and lifecycle checks; retain any exact eligible-host prerequisite. r[kiln.web_hosted.qualification]
