# Tasks: real repository integration

## Phase 1: Implementation

- [x] [parallel] G1 Add read-only replay-checked canonical GitHub observation export and positive/negative behavior coverage. r[kiln.dashboard.repositories.github_export]
- [x] [parallel] C1 Add bounded typed repository/history catalog admission, offline navigation and watch selection. r[kiln.dashboard.repositories.catalog]
- [x] [parallel] P1 Add bounded real-report generation and atomic Aspen3 file delivery shell. r[kiln.dashboard.repositories.delivery]
- [x] [serial] R1 Connect the actual Kiln repository/source route and clearly distinguish fresh execution from retained self-CI evidence. r[kiln.dashboard.repositories.real_ci]

## Phase 2: Verification

- [x] [serial] V1 Exercise success/failure, duplicate/restart recovery, catalog/export rejection, browser navigation/no-script/watch and actual delivery; record results and missing prerequisites. r[kiln.dashboard.repositories.real_ci.scenario.scope]
- [x] [serial] V2 Run focused and required repository gates, update operator documentation and retain exact evidence without publication or archival. r[kiln.dashboard.repositories.delivery.scenario.failure]
