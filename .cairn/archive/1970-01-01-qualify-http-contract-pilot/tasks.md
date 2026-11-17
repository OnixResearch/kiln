# Tasks

## Phase 1: Served shell and client bounds

- [x] [serial] Run the lockstep client against the real HTTPS `serve` process with success, 400, 401, 403, 404 and pagination cases, keeping authentication, admission, transport and schema outcomes distinct. r[kiln.web.http_contract.served_matrix]
- [x] [serial] Bound received success and error bytes and enforce connect, total and body-idle deadlines without retries; test mutated, oversized, mislabeled, slow and stalled bodies through a loopback TLS stub. r[kiln.web.http_contract.client_bounds]

## Phase 2: Tooling

- [x] [serial] Add the pinned Drift tool, reviewed baseline, classification policy, fixture pairs and flake check. r[kiln.web.http_contract.drift]
- [x] [serial] Measure Progenitor generated-code volume, dependencies and cold and warm build time against the handwritten client in a scratch workspace, and retain the measurements with the decision. r[kiln.web.http_contract.generation_evaluation]

## Phase 3: Evidence

- [x] [serial] Parent runs the focused tests, the flake check and the Cairn gates and retains verification evidence. r[kiln.web.http_contract.served_matrix] r[kiln.web.http_contract.client_bounds] r[kiln.web.http_contract.drift] r[kiln.web.http_contract.generation_evaluation]
