## Context

The live dashboard already shares a timestamp formatter and separate bounded native COB browser. The operated service has fifteen enrolled projects and nineteen retained runs; its optional --cobs policy is absent. The public Kiln Radicle store exists locally but contains no registered Onix refs.

## Decisions

### Decision: Preserve UTC and add browser-local presentation

Use a cached Intl.DateTimeFormat with the browser locale, full date, hours/minutes/seconds and timezone abbreviation. Include its resolved IANA timezone alongside the local label and retain the existing ISO UTC instant. Invalid, unsafe and unrecorded values remain unavailable; do not substitute zero or the current time. Reuse this formatter in run details, latest execution observations and other existing timestamp consumers; explicitly show last-run completion in overview project health.

### Decision: Read only an explicit existing public store

Use config/web-cobs.ncl to export a host-local policy for the public Kiln RID and its existing bare Git directory. Preserve service/access/automation profiles and native workflow bindings. Pass the policy to both installed preflight and serve commands. No service request discovers stores, enables private stores, synchronizes peers or writes objects; object creation is only the explicit offline operator import below. Empty inventory and exact missing-object results remain valid bounded observations, not evidence that no Onix objects exist elsewhere.

### Decision: Populate the selected store from real change artifacts

The six Onix types had no objects anywhere on the host: all sixty-nine local Radicle stores contained only `xyz.radicle.id`, `xyz.radicle.issue`, `xyz.radicle.patch` and `xyz.radworks.job` refs, and no Cairn collaboration store existed. The browser therefore reported a true empty observation, not a defect.

Under explicit operator authorization, import each active Cairn change with `cairn cob import --backend radicle-cob.v1` into the existing public Kiln store, signed by the operator's existing Radicle node identity (`~/.radicle/keys`) and recorded under `refs/namespaces/<operator>/refs/cobs/xyz.onixresearch.cairn-change/<oid>`. Each object binds the change's proposal, design, tasks, metadata and delta specs with their BLAKE3 identities. The import is local Git-only: it does not publish, synchronize, announce, mutate delegates or contact a peer, and it leaves store `HEAD`, `packed-refs`, config and every pre-existing ref unchanged.

### Decision: Exercise the real paths before delivery

Capture the baseline, run existing six-type positive/negative conformance, build the packaged binary, reload only the existing local service and dependent proxy, then exercise the authenticated browser/API. Prove UTC preservation and timezone conversion using actual recorded runs and controlled browser timezone changes, including a day-boundary conversion. Retain evidence and update the operator procedure without archive or publication.

## Risks / Trade-offs

- Browser-local output depends on the reader's locale/timezone and is not canonical stored state. Explicit labels preserve interpretability across hosts.
- Available local stores may contain no supported objects. State that fact rather than silently converting file-based lifecycle packages or claiming signature verification.
