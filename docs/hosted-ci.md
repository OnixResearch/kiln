# Iroh-connected hosted CI: bounded operator procedure

`kiln-hosted` is an optional composition, not a replacement CI core or a claim
that a paid public service is qualified. Customers keep their repositories on
their upstream source host. This procedure describes local commands and explicit
admission boundaries; it is **not approval** to activate providers, publish a
webhook, purchase capacity, transmit payment requests, or deploy a service.
Qualification is bounded to the retained local evidence below, not a live-provider outcome.

## 1. Local keys, typed profiles and connection

Use an already built `kiln-hosted` on `PATH` (workspace package `kiln-hosted`).
Alternatively run the packaged flake app with `nix run path:.#kiln-hosted -- ...`
from the repository root. Replace `...` with the same subcommand/options shown
below; for example `nix run path:.#kiln-hosted -- check --profile
"$HOME/kiln-hosted-local/server.json"`. Nix acquisition/build is separate from
profile admission and does not activate `serve` or any provider on its own.
Run from the repository root for the relative Nickel imports below. Prepare a
private operator directory on a filesystem with safe, non-symlink ancestors:

```sh
umask 077
mkdir -m 700 "$HOME/kiln-hosted-local"
kiln-hosted keygen --key "$HOME/kiln-hosted-local/server.key"
kiln-hosted keygen --key "$HOME/kiln-hosted-local/client.key"
```

Each command prints its public peer ID and creates a new mode-0600 file
containing 32 raw secret bytes. It refuses an existing target. Keep the server
and client keys separate. Record the **client** public ID as `bootstrap_peer`;
the server's transport identity is not automatically a customer membership.
Never put key bytes in Nickel, browser state, logs or request JSON.
Run only **one active endpoint process per private key**. In particular, stop
`connect` before a separate `rpc` process uses that same client key; competing
direct routes for one Iroh identity can time out. For concurrent browser bridge
and CLI access, generate distinct client keys and enroll both peers with the
required scopes. A timeout does not authorize a fresh effect or request ID:
preserve the identical request identity/input and inspect retained state before
retrying once the competing endpoint has stopped. No automatic retry or error
suppression is implied.

Create `config/hosted-service-local.ncl` locally with this shape, substituting
absolute paths and the actual client public ID. It is operator input, not a
committed credential fixture:

```nickel
(import "hosted-service.ncl") {
  service_id = "local-hosted",
  state_directory = "/home/OPERATOR/kiln-hosted-local/state",
  network = {
    key_file = "/home/OPERATOR/kiln-hosted-local/server.key",
    bind_address = "127.0.0.1:44771",
    relay_urls = [],
  },
  bootstrap_peer = "CLIENT_PUBLIC_ID_AS_64_LOWERCASE_HEX",
  bootstrap_organization = "acme",
  operator_peers = [],
  source_profiles = [],
  operations = null,
  secrets = null,
  execution = null,
  fleet = null,
}
```

`network.relay_urls` entries are canonical HTTPS base URLs: no spaces, query
strings, fragments, control characters, or out-of-range ports. Bracketed IPv6
and IPv4 socket addresses are accepted.

The uppercase placeholder intentionally does not satisfy the peer contract.
Replace it, do not weaken the contract. The profile rejects unknown fields at
the top level and in each nested section; remove stale keys instead of carrying
forward an older shape. Export with private permissions:

```sh
nickel export --format json config/hosted-service-local.ncl > "$HOME/kiln-hosted-local/server.json"
chmod 600 "$HOME/kiln-hosted-local/server.json"
kiln-hosted check --profile "$HOME/kiln-hosted-local/server.json"
kiln-hosted serve --profile "$HOME/kiln-hosted-local/server.json"
```

`check` admits the top-level structure; it does not open the service database,
resolve source credentials, check the actual execution environment, listen, or
qualify production. `serve` takes an exclusive service lock, opens
`state_directory/hosted.sqlite`, admits configured modules and binds Iroh. A
missing state directory may be created; an existing root must be owned mode
0700, with safe ancestors. Private JSON/key files must be owned, singly linked
regular files with no group/world permissions; symlink leaf files are denied.

`serve` prints its serialized endpoint address. Copy the **whole** endpoint
object into `remote` in `config/hosted-client-local.ncl`:

```nickel
(import "hosted-client.ncl") {
  network = {
    key_file = "/home/OPERATOR/kiln-hosted-local/client.key",
    bind_address = "127.0.0.1:0",
    relay_urls = [],
  },
  remote = {
    id = "SERVER_PUBLIC_ID_AS_64_LOWERCASE_HEX",
    addrs = [{ Ip = "127.0.0.1:44771" }],
  },
  organization = "acme",
  listen_address = "127.0.0.1:44881",
  webhook = null,
}
```

The shown `Ip` is a same-host direct-route example; replace it with the actual
endpoint output. The pinned Iroh 0.95.1 `EndpointAddr` is `{id, addrs}` with
externally tagged entries `{Ip = "IP:port"}` or
`{Relay = "https://operator-selected-relay.example"}`. It is not a string ticket
and is not the older `node_id`/`direct_addresses` shape. Discovery is cleared;
a peer ID alone supplies no usable route. Empty `relay_urls` disables relays;
there is no implicit public relay. Remote operation needs an explicitly
provisioned reachable direct address or selected HTTPS relay, firewall policy
and independent relay/TLS review. Reachability grants no application authority.

```sh
nickel export --format json config/hosted-client-local.ncl > "$HOME/kiln-hosted-local/client.json"
chmod 600 "$HOME/kiln-hosted-local/client.json"
kiln-hosted connect --profile "$HOME/kiln-hosted-local/client.json"
```

Open the exact `http://127.0.0.1:44881` URL printed by `connect`, not `localhost`
or a different Host alias. The browser talks only to the local bridge; the
bridge's local key authenticates the native Iroh connection. The page can be
opened before invitation acceptance; lack of `system.info` membership is not a
reason to bypass enrollment. Do not expose this bridge as a public customer
admin API or allow untrusted local processes to use it. It binds loopback,
checks Host/Origin and serves private no-store assets without a browser bearer
token. `/session` exposes public connection identities, not private key bytes.

Customer transport uses ALPN `kiln.customer.v1`, a single bounded JSON request
per bidirectional QUIC stream, four-byte big-endian frame length, EOF after the
frame, 1 MiB frames, at most 32 active exchanges and a 30-second exchange bound.
It is distinct from worker `kiln.workflow.v1` and observation protocols. GitHub
HTTPS signatures, checkout and commit statuses remain provider-native.

## 2. RPC, roles and project scope

Write a private `request.json` containing:

```json
{"schema":"kiln.hosted.v1","organization":"acme","request_id":"info-1","action":"system.info","input":{}}
```

```sh
kiln-hosted rpc --profile "$HOME/kiln-hosted-local/client.json" --request request.json
```

`rpc` prints the response envelope and exits nonzero for an application error.
`organization` must equal the client profile; it selects a scope, not authority.
The authenticated QUIC peer's **current** durable membership authorizes each
request, including retained replies. For effect-capable RPCs, reuse the identical
request identity and input to recover an interrupted exchange. A conflicting
input under the same ID fails. An uncertain retained request is not permission
to generate a new ID and repeat a possible external effect. New intentional
operations need new IDs. The server retains effect replay protection with finite
request capacity; it does not automatically prune it to regain space. Rejected
invitation acceptance retains no replay quota: membership, invitation consumption,
successful response and audit commit atomically. Issuer discovery/JWKS are current
authorized reads, not permanently retained effect requests; repeated metadata
GETs cannot consume that replay capacity.
The [2026-10-29 security regression and real transport exercise](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/replay-capacity-20261029.json)
records failing-before/passing-after capacity cases, injected audit failure,
same-request acceptance after rollback, restart replay and actual HTTP metadata
reads without permanent replay allocation. This is scoped local evidence, not
independent whole-system or hostile-isolation certification.

Browser-local scripted RPC, while `connect` runs, has this exact surface:

```sh
curl --fail-with-body http://127.0.0.1:44881/rpc \
  -H 'Origin: http://127.0.0.1:44881' \
  -H 'Content-Type: application/json' -H 'X-Kiln-Local-Client: 1' \
  --data-binary @request.json
```

HTTP 200 may contain an RPC `error`; inspect the envelope, not just HTTP status.
No caller-supplied role, principal, forwarding header or relay establishes
membership. Roles are `owner`, `admin`, `member`, `viewer`, `billing`. Owners and
admins have organization-wide project visibility; other roles need an explicit
sorted, unique project list. Owners may assign every role; admins may not
assign or alter owner/admin authority. Final-owner removal is denied.

Change `action` and `input` in the same envelope to use these account APIs:

| Action | Input and boundary |
| --- | --- |
| `accounts.list` | `{}` or `{"after":"PEER_ID"}`; owner/admin, bounded pages with returned cursor |
| `accounts.invite` | `{"peer":"PEER_ID","role":"member","projects":["app"],"expires_at_ms":FUTURE_MS,"nonce":"invite-1"}`; expiry must fit the admitted invitation lifetime |
| `accounts.accept` | `{"invitation":"RETURNED_INVITATION_ID"}`; run using the intended peer's client key and same organization, consume-once |
| `accounts.revoke_invitation` | `{"invitation":"ID"}`; owner/admin within assignable-role boundary |
| `accounts.set_membership` | `{"peer":"PEER_ID","expected_revision":CURRENT,"role":"viewer","projects":["app"],"active":true}`; stale revision denied; `active:false` revokes |

Invitations are bound to intended peer and inviter revision. Changed inviter
rights, expired/consumed invitations and revoked peers cannot restore old
authority. Bootstrap is once per database; restart does not re-enroll a revoked
peer or permit a different bootstrap identity.

## 3. Existing upstream repository connections and native routes

There is no repository upload/move or new forge requirement. This hosted source
composition currently admits the existing GitHub adapter's exact remote HTTPS
source profiles. Other existing core adapters do not automatically become
hosted connection providers. Follow [the GitHub procedure](operator.md#github-ci-adapter)
for actual credential references, reviewed source job and worker transport.
An installation credential is not synthesized by account enrollment.

Add an operator `source_profiles` entry to the service Nickel input:

```nickel
{
  path = "/absolute/reviewed/github-service.json",
  blake3 = "EXACT_LOWERCASE_BLAKE3_OF_THOSE_JSON_BYTES",
  organization = "acme",
  project = "app",
  network_activation = false,
  price_id = null,
  class = null,
  max_units = null,
}
```

Compute the digest of the exported JSON bytes with `b3sum
/absolute/reviewed/github-service.json`; do not use a source revision or
pipeline identity in its place. The referenced GitHub configuration includes
SecretSpec webhook/status references, native workflow socket/capabilities,
repository numeric ID and exact source-job binding. Startup rejects changed
bytes or profiles exceeding hosted bounds (128 run records, 2048 deliveries,
1 MiB body and record bounds). Its replay root and worker endpoint must exist
and be writable/reachable under the reviewed policy. Removing a startup profile
retires its current admission grant, not its historical replay evidence.

Owner/admin actions:

* `connections.profiles`, input `{"project":"app"}`, lists only the operator
  profiles admitted to this organization/project.
* `connections.connect`, input
  `{"project":"app","connection":"origin","repository_id":UPSTREAM_NUMERIC_ID,"owner":"UPSTREAM_OWNER","name":"UPSTREAM_REPO","profile_blake3":"EXACT_DIGEST","expected_revision":0}`,
  binds the existing upstream identity. Reconfiguration requires the current
  revision and creates a new generation, not a replacement repository identity.
* `connections.list`, input `{"project":"app"}`, returns generation/revision;
  continue with returned `next` as `after` when present.
* `connections.disconnect`, input
  `{"project":"app","connection":"origin","expected_revision":CURRENT}`,
  denies new admissions without deleting upstream source or replay records.

**Provision the exact route after every connection generation.** The connect
response returns `effective_adapter_identity`, `pipeline_identity` and
`workflow_policy_id`, with `route_admission_required:true`. The adapter rewrites
policy and native replay namespace with service/organization/project/connection/
generation/profile identity. A route for the unmodified profile is not enough.
The operator must admit that returned exact effective workflow policy/plan into
the Lattice worker handler and its private transport binding before dispatch.
There is no hosted RPC that automatically grants worker routes. Missing or stale
routes must fail; do not broaden a wildcard route to make a smoke succeed.

For a native webhook relay, set the client profile's optional binding:

```nickel
webhook = { project = "app", connection = "origin", generation = 1 }
```

`POST /github/webhook` on that loopback bridge relays the exact raw signed body
and GitHub delivery/event/signature headers over authenticated Iroh as
`connections.ingest`. It is not anonymous customer RPC. A separately approved
HTTPS reverse proxy/tunnel must route the intended public webhook URL to this
exact bridge, preserve body/signature/delivery headers and set Host to the exact
loopback address. Its Origin, if sent, must satisfy the bridge's same-origin
check. No TLS listener, DNS, public tunnel or GitHub webhook registration is
created by `connect`. A refused source request returns HTTP 400 rather than an
accepted acknowledgement. A wrong generation/repository/signature or missing
SecretSpec resolver fails without new work. Update the binding and public route
when a generation changes; do not derive it from an unauthenticated query.

Alternatively an authorized member may relay `connections.ingest` explicitly:
`{"project":"app","connection":"origin","generation":1,"headers":{"delivery":"GITHUB_DELIVERY_ID","event":"push","signature":"sha256=EXACT_NATIVE_SIGNATURE"},"body_base64":"EXACT_RAW_BODY_STANDARD_BASE64"}`.
The native signature still applies to the original body bytes. The entire
request must fit the frame bound; a maximal 1 MiB body will not fit after Base64
and envelope overhead. Duplicate delivery IDs retain the existing exact run;
conflicting body/generation identities fail.

Ingest reserves a source delivery; it is not dispatch authorization. To permit
external checkout/workflow/status effects, an operator must deliberately set
`network_activation = true` for the **exact** source profile and supply its
admitted `price_id`, matching `class` and finite `max_units`, then restart with
that reviewed configuration. Only after independent activation approval use
`connections.tick` with `{"project":"app","connection":"origin","generation":1}`.
Tick reserves finance before dispatch and drives native lifecycle/status
reconciliation; it is explicit, not an unattended background scheduler.
`connections.status` with the same input reads retained source/run observations.
An incident hold denies new source dispatch but permits already accepted work
to reconcile. Disconnect and generation retirement cannot erase accepted effects.

## 4. Prices, reservations, invoices and unknown obligations

`config/hosted-prices.ncl` validates operator price/account authoring, not merchant
approval. The service profile does not load that file or activate its sandbox
section. Owner/billing financial authority admits the selected values through
these RPCs (admin is not billing authority):

* `finance.account.configure`:
  `{"request_id":"account-1","expected_revision":0,"currency":"usd","cap_minor":10000,"request_max_minor":1000,"max_concurrent":1}`.
  These numbers illustrate integer minor-unit fields, **not approved prices or
  customer spend consent**. Use operator-approved limits and current revision.
* `finance.price.admit`:
  `{"class":"trusted-local","currency":"usd","unit_minor":1,"unit_ms":1,"max_units":1000}`;
  retain the returned exact `price_id`. Class/currency/maximum are immutable
  inputs to that price identity, not a free-form alias.
* `finance.account.get`: `{}`; `finance.reservation.get`:
  `{"attempt_id":"EXACT_ATTEMPT"}`; `finance.reconcile`: `{}`.
* `finance.usage.list`: `{"after":"","limit":100}`; only trusted settled
  usage appears here. Continue with `next_after`.
* `finance.invoice.draft`: `{"request_id":"invoice-1"}`; drafts consume only
  previously uninvoiced ledger entries. `finance.invoice.get` or
  `finance.invoice.export`: `{"invoice_id":"RETURNED_ID"}`.
* `finance.credit.issue`:
  `{"request_id":"credit-1","amount_minor":100,"reason":"reviewed correction","correction_of":"EXACT_INVOICE_ID"}`;
  omit `correction_of` for a non-invoice credit. It is a ledger credit, not a
  refund/payment, and never deletes a usage receipt or unknown reservation.

`finance.reserve` exists for explicit bounded reservation, with
`{attempt_id,price_id,class,max_units,binding}`; project bindings are scoped.
It is not execution permission. Do **not** manually pre-reserve an attempt for
`execution.run`: that path owns atomic pre-effect financial admission and denies
a preexisting financial attempt. Cap/concurrency checks include reserved and
unknown exposure across restart. Failure, cancellation or missing observations
are not proof of zero usage.

Local execution derives units from trusted host elapsed time and the admitted
price's `unit_ms`; do not accept customer-reported usage. Native source status
is **not trusted metering**: source tick reserves then marks exposure unknown,
and a completed GitHub/Lattice run does not settle that financial obligation.
An independently trusted, exact-attempt metering integration is required before
those source reservations can settle; the customer RPC offers no usage-injection
or force-release escape hatch. Recovery may clean a process slot while money
remains reserved. `finance.reconcile` checks ledger arithmetic; `matched` does
not mean payment collected or all unknowns resolved.

`finance.outbox.list` (`{"after":""}`) is observation, not payment dispatch.
The finance module has a pinned Stripe test-only protocol boundary requiring
real `sk_test_`/webhook secret/customer inputs and exact provider idempotency;
there is no `kiln-hosted` payment-sending CLI or customer payment-state mutation.
Do not invent a `finance.pay`/`usage.settle` RPC. Merchant eligibility, legal/tax
review, approved prices/terms, credentials, customer authorization and explicit
point-of-risk approval are independent prerequisites. Nothing here authorizes
any live charge, refund or even sandbox transmission.

## 5. Repository-owned pipelines and fail-closed migration

Keep pipeline source in the customer's upstream repository. Use
`config/hosted-pipeline.ncl` and `config/hosted-pipeline-example.ncl` for typed
schema `kiln.hosted.pipeline.v1`. For an operator-reviewed local authoring copy:

```sh
nickel export --format json config/hosted-pipeline-example.ncl > pipeline.json
kiln-hosted compile --input pipeline.json
kiln-hosted migrate --input /absolute/upstream-checkout/.github/workflows/checks.yml
```

`compile` prints a deterministic exact plan; `migrate` prints
`{pipeline,compiled,diagnostics}`. Neither executes or registers it. To export a
customer Nickel file, import the pipeline builder with the repository's actual
relative path and use the same `nickel export --format json FILE` command.
Templates, parameters, literal matrices, dependencies, literal environment maps,
explicit `bash`/`sh` and deadlines are bounded; cycles, unknown fields and
expansion overflow fail before execution.

RPC names are plural: `pipelines.compile` takes the pipeline object itself;
`pipelines.migrate` takes `{"yaml":"EXACT_YAML_TEXT"}`;
`pipelines.examples` takes `{}`. Owner/admin review uses `pipelines.review` with
`{"project":"app","name":"checks","expected_revision":0,"config":PIPELINE_OBJECT}`.
Retain returned `identity`/revision. `pipelines.list` takes `{"project":"app"}`;
`pipelines.get` takes `{"project":"app","identity":"EXACT_PLAN_ID"}`.
Replacing a named review revokes the prior current plan; a raw compile result
never authorizes dispatch. This local registry is distinct from the native
source connection's exact Lattice route; review does not install that route.

Actions migration is deliberately a subset: push/pull_request/workflow_dispatch
without filters/inputs, explicit shell `run` steps, all-success `needs`, literal
matrix axes with explicit `fail-fast:false`, simple `matrix.KEY` expressions and
literal environment precedence. Unknown `uses`, implicit shells, permissions,
conditions, services/containers, outputs, secret/context expressions,
include/exclude/max-parallel, job display names and GitHub runner/file-command
environment protocols are rejected, not silently omitted. Provide an explicit
native mapping and review changed semantics rather than deleting an erroring
field to pretend compatibility.

### Trigger filters and schedules

A `triggers` entry is either a bare name (`"manual"`, `"push"` or `"pull_request"`; existing plans keep identical bytes and identities) or an object:

- `{ event = "push", branches?, tags?, paths? }`
- `{ event = "pull_request", branches?, types?, paths? }`, where `branches` match the base branch and `types` are `opened`, `reopened`, `synchronize`, `closed` or `merged`
- `{ event = "schedule", schedule = "<id>" }`

Globs use `kiln_core::paths::matches`, and each list holds at most 32 patterns. Admission applies these rules:

- Each event kind appears once, and each schedule id once.
- At most 8 triggers.
- Unknown keys fail.

`execution.run` may carry `event = { kind, reference?, action?, base_branch?, base_revision?, revision? }`:

- **Changed files:** computed from the exact revisions in the workspace repository. Unknown changed files always run.
- **Match:** the first matching trigger is recorded as `trigger { index, event, reasons }` on the run result.
- **No match:** the attempt is stored as `skipped`, with every trigger's `excluded_by`. No finance or process is started.

`migrate` maps Actions filters onto native ones:

- `branches`, `tags`, `paths` and the `opened`, `reopened` and `synchronize` types map directly.
- `closed` maps to `closed` plus `merged` (recorded in diagnostics).
- `*-ignore` filters, `!` negation, and `?`, `+` and `[]` patterns are rejected.

`schedules.<id> = { cron, timezone = "UTC", min_interval_ms = 300000, reference = "refs/heads/main" }` is reviewed with the pipeline:

- **Review:** resolves `H` from BLAKE3 of organization, project, pipeline name and schedule id, and rejects occurrences closer than the minimum interval within 400 days.
- **Reservation:** `pipelines.schedule { project, name, schedule, reference, revision }` reserves occurrences durably:
  - The first call after a review only initializes the next occurrence.
  - Due occurrences fire once (`attempt_id` is derived from the occurrence).
  - Occurrences older than the minimum interval are `missed`, never caught up.
  - Occurrences too close to the last run are `too_close`.
  - The exact observed revision of the reviewed reference is bound.

Hosted has no default-branch source port: the caller supplies the revision it observed at the due time.

## 6. Explicit isolation, resource limits and execution recovery

A non-null service `execution` profile is required. `hosted-service.ncl` checks
all nested fields, including runner variants and workload grants. Each executable
and VM image is `{path,blake3}`: the actual file must resolve into immutable
`/nix/store`, satisfy file/size bounds and match exact BLAKE3 bytes. Customer RPC
cannot choose executable paths. Provision a functioning systemd user manager,
`manager_runtime_directory` `/run/user/UID`, cgroup resource delegation,
`systemd_run`, `systemctl`, `env`, a private `work_root` and finite concurrency,
memory, processes, CPU, deadline and output bounds before startup.
The user-manager cgroup-v2 boundary enforces `MemoryMax`, `MemorySwapMax`,
`TasksMax`, `CPUQuota` and `RuntimeMaxSec`; cleanup must stop the owned unit and
observe its authoritative `ControlGroup` empty, not merely a launcher exit.

Actual runner choices are:

* `kind = "namespaces"`: reviewed `unshare`, `python`, `mount`, `shell` and
  read-only `runtime_roots` selected from `/nix/store`, `/usr`, `/bin`, `/lib`,
  `/lib64`. User/mount/PID/network/IPC/UTS isolation and external systemd limits are
  required at runtime. This backend is **trusted jobs only**, not a hostile
  customer security boundary. Disabled user namespaces, mount permission or
  unavailable user-manager/cgroup enforcement are prerequisite failures, not
  reason to run unsandboxed.
* `kind = "qemu"`: reviewed `qemu`, `kernel`, `initramfs`, optional `root_image`,
  absolute `guest_shell`, explicit `acceleration` (`tcg` or `kvm`) and optional
  `qualification_identity`. The guest must implement the module's
  `QEMU_GUEST_INIT` input/result protocol, fw_cfg request input and mapped-xattr
  virtio-9p mapping of only the exact scoped customer workspace. Build the guest
  with Python 3, mount, stty and poweroff plus a kernel supporting virtio-9p,
  fw_cfg/sysfs and serial output. The optional raw root image is read-only;
  workspace mounts preserve changes between steps. An arbitrary distribution
  image is not sufficient. KVM requires actual device/capacity access. Hostile
  admission requires an exact independent qualification identity, but a
  syntactically valid hash is **not evidence** of independent review. Supplying
  images or a receipt field does not qualify a changed kernel, hypervisor,
  image, manager or host. No hostile-isolation qualification is claimed here.
  Receipts always report `hostile_isolation_claim:false`; `requested_trust` and
  `operator_qualification_references` retain requested policy/provided references,
  not independently verified qualification evidence.
* `kind = "firecracker"`: one Firecracker microVM per job. Fields:
  - `firecracker`, `kernel` and `initrd`: reviewed `{path,blake3}` identities.
  - `guest_shell`: absolute shell path inside the guest.
  - `vcpus` (1–8) and `memory_mib` (128–16384). Guest memory plus a 64 MiB VMM
    allowance must fit `memory_bytes`, and vCPUs plus 4 tasks must fit
    `processes`.
  - `boot_timeout_ms` (100–60000, below `maximum_timeout_ms`).
  - `source_image_mib` (1–1024): the fixed source drive size.
  - Optional `warm_pool = {directory, slots}`: at most 16 slots and at most
    `maximum_concurrent`. The directory must be an existing short, private
    0700 path outside `work_root` and `/nix/store`.
  - Optional `qualification_identity`.
  - Optional `debug_hold = {enabled, grace_seconds, max_concurrent}`: keep the
    microVM of a failed job alive for a bounded grace so a project member can
    debug it (see **Debug hold** below). `grace_seconds` is 1–900 and
    `max_concurrent` is 1–16 and at most `maximum_concurrent`. Omitted or
    `enabled = false` changes nothing: same transitions, unit limits,
    reservations, configuration identity and receipts.
  - Optional `untrusted_capable = true`: this runner may execute approved
    untrusted pull-request runs (see **Untrusted pull-request runs** below).
    Omitted keeps the configuration identity and refuses such runs.

  There is no network or jailer field. A cache policy naming a Firecracker
  runner enables restore through a read-only second drive (`/dev/vdb`), which
  the guest extracts over `/work`. After a successful job the host requests the
  declared output roots over the existing vsock session, validates and extracts
  the bounded ustar, and publishes after confirmed cleanup. The transfer bound
  includes ustar headers, padding and trailer separately from cached content;
  admitted bounds must fit the guest protocol frame. No writable host share is
  exposed. The optional object-storage tier is not implemented.

  The backend is admitted only on x86_64 hosts. Build the guest with
  `nix build path:.#microvm-guest` (x86_64-linux only). It contains the pinned
  nixpkgs NixOS kernel (stripped `vmlinux`), a `makeInitrdNG` initramfs
  (busybox, bash, the `kiln-microvm-guest` agent and the virtio/vsock modules)
  and the pinned `firecracker`. Its `manifest.json` is
  `kiln.microvm.guest-image.v2` and lists the exact paths and BLAKE3 values,
  including a `vmm` entry, to copy into Nickel. This is a Nix-built initramfs
  over the NixOS kernel, not a full NixOS system. `nix build
  path:.#microvm-guest-ch` builds the same agent over `virtio_pci` and the
  pinned Cloud Hypervisor; it is not executable by the hosted shell yet.

  For each job, the service:
  1. Archives the scoped workspace into a deterministic, bounded ustar image.
     Symlinks are archived, never followed; special files are rejected.
  2. Starts Firecracker in the attempt's systemd unit, with the same limits.
     The VM gets the image as its only, read-only virtio-blk drive, one vsock
     device, an entropy device, no network interface, and a CPU template that
     hides CET (see below).
  3. Runs the guest agent as PID 1. The agent connects to the single
     per-session host listener; the host removes that listener after the first
     connection. The agent extracts the source into guest tmpfs `/work` and runs
     each step as uid/gid 1000 with `no_new_privs`, the namespace backend's
     environment and shell flags, a deadline and output bounds.
  4. Writes the job result to `hosted_microvm_job` before it stops the unit, and
     records `destroyed` or `destroy_uncertain`. Uncertain teardown leaves the
     attempt `unknown` until `execution.recover`.

  Guest writes never reach the host workspace. With a warm pool, each slot
  holds an exclusive lock and a snapshot bound to the Firecracker, kernel,
  initrd, machine and slot identity. A changed identity or state hash rebuilds
  the snapshot. A restore gets fresh wall-clock time and a host entropy seed,
  and the VM is still destroyed after each job. When all slots are busy, the
  job cold boots.

  The guest is not offered CET (shadow stack/IBT). Firecracker 1.15 does not
  carry CET supervisor state through a snapshot on KVM hosts that virtualize
  CET, and the restored guest faults without the template.

  Run the KVM-gated host test with:

  ```sh
  KILN_MICROVM_GUEST=$(nix build --no-link --print-out-paths path:.#microvm-guest) \
    cargo test -p kiln-hosted --test microvm -- --nocapture
  ```

  The test runs the same pipeline on `namespaces` and `firecracker` and
  compares `canonical_outcome`, runs the guest-escape suite, and prints latency
  lines. Set `KILN_MICROVM_REQUIRED=1` to make missing KVM or guest artifacts a
  failure instead of a skip. On the 2026-10-30 reference host (AMD Zen 5, 1 vCPU,
  256 MiB, release build), the time from unit spawn to guest readiness was
  464–515 ms for a cold boot and 54–104 ms for a snapshot restore. This is a
  single-host measurement retained with the change evidence, not a capacity
  promise.

**Isolation claims of the Firecracker backend.** It upgrades exactly one claim.
A job no longer runs on the host kernel: it runs in a separate guest kernel
behind KVM and a seccomp-confined VMM.

As a result, a kernel exploit through unprivileged user namespaces or other
host syscalls first lands in the guest kernel, not the host. Inside the guest,
jobs run unprivileged, namespace creation and module loading are disabled, and
there is no network device (see [Guest networking (non-goal)](#guest-networking-non-goal)).
The retained escape suite denied namespace creation, host `/proc`/`/sys`
observation and IP or vsock egress.

<!-- r[impl kiln.microvm.confinement.nonclaims] -->

The shell observes that claim before the guest session. It adds
`NoNewPrivileges=yes` to the attempt unit, reads the unit's `cgroup.procs`, and
requires exactly one `firecracker` process with the service user's UID/GID,
`CapEff` zero, `NoNewPrivs` one, `Seccomp` two, and the reviewed immutable
executable path. For a warm pool the check follows `/snapshot/load`; for a cold
boot it follows spawn. A failed or unavailable observation journals failure and
never admits the guest. The `vm.confinement` receipt records the observed
fields and reviewed executable BLAKE3 identity; the lifecycle gate is
`ConfinementObserved`. This observes one process on one host; it is not a proof
that the reviewed binary has no vulnerability or that another host reports the
same facts.

The `vm.host` receipt records bounded host kernel release/package, the
issue-policy `cve_2026_98432` status, SMT control/active state, KSM run state,
kernel cmdline, CPU vulnerability entries, MPTCP sysctls/listener count, resctrl
mount state and L3 CAT/MBA capability files. Kernel/network fields are also
available directly on `vm`. Missing observations are `null`, empty maps or
explicit `unknown`, never inferred safe. Kernel package provenance is the
booted NixOS kernel store path, or an explicitly `operator-declared:`
`KILN_HOST_KERNEL_PACKAGE`; unsupported package discovery is `unknown` (not an
OS BUILD_ID masquerading as a kernel package). `ss -M -l -n -p -H` is bounded;
missing tooling, command failure or truncation yields unknown listener count.

The CVE name, advisory descriptions and upstream fixed-series thresholds are
**issue #28's supplied operator policy**, not an independently verified security
advisory. The checked-in table is in `kiln-microvm/src/host.rs`; only enumerated
release series are admitted. Exact reviewed vendor backport package entries can
override the upstream decision in the pure classifier, but **no vendor package
is currently asserted patched by the production shell**. Unknown/vulnerable
kernel status unconditionally refuses every dispatch on an `untrusted_capable`
runner. Only active SMT/running KSM may be admitted with a non-empty reviewed
`host_preflight_override`; `vm.host_admission` retains the reason and observed
refusal, in addition to the host facts. Other runners retain facts and continue.
MPTCP/tcp_migrate_req observations are partial exposure facts, not a fix. In
particular disabled MPTCP with nonzero or unknown listeners is not evidence of
reduced exposure; patching and rebooting remain the issue-required remedy.

Reference-host observation, **2026-11-05**: booted kernel **6.18.50**, package
`/nix/store/9xdswfnj5dv4al2cd51f3nlvsh78s5vv-linux-6.18.50/bzImage`,
`cve_2026_98432=vulnerable` **by the issue-supplied table**, SMT active, KSM off.
This host is therefore not admitted for untrusted-capable execution. The guest
kernel's separately pinned fixed-series gate does not patch the host.

**Guest kernel driver surface.** The guest uses the pinned NixOS-default
kernel. `nix build .#microvm-guest-surface` measures that image against the
reviewed class list in `config/microvm-guest-surface.ncl` and writes
`inventory.json`; the checked-in
[inventory](../fixtures/microvm/guest-driver-surface.json) pins the current
cohort (kernel version, configuration BLAKE3, guest nixpkgs revision, class
verdicts). The check fails when an enforced class is compiled in (`=y`) or the
guest module closure ships a loadable module for it.

Measured verdicts for the current cohort: Bluetooth, wireless, FireWire,
Thunderbolt/USB4, InfiniBand, sound and USB host-controller drivers are
**absent** from the image (the kernel builds them as modules, but the guest
module closure ships only the nine virtio/vsock modules and
`modules_disabled=1` denies loading). USB core and display/DRM/framebuffer
support are **builtin** and are measured only, not denied: the current
NixOS-default kernel compiles them in. Those are recorded misses that require a
separately reviewed slimmed-kernel redesign; the check does not pretend they
are absent.

The guest escape suite also requires the enforced classes' sysfs entries to be
absent. This measurement is about the guest image, not a hostile-isolation
claim: it does not prove the kernel is vulnerability-free, does not cover host
covert channels (issue #28), and keeps `hostile_isolation_claim: false`.
Refresh the inventory with the reviewed nixpkgs pin and rerun the guest build;
never update one surface alone.

Firecracker runners carry reviewed `trust_domain` and `host_identity` labels;
untrusted-capable runners require both. Static validation forbids combining
untrusted-capable and workload-bearing runners in one declared pool/host. At
runtime this executor launches locally: its durable reservation ledger uses the
**observed machine identity**, so different labels cannot evade whole-host
exclusion across attempts, retries or organizations. Untrusted provenance and
any job with a workload policy (even one with no delivered credential) conflict
on that host before financial/process effects. Unknown attempts keep their
reservation until reconciliation. Distinct physical host services must have
separately provisioned pools; separate labels on one local service are not
physical isolation. This ledger does not govern unrelated services/databases or
unmanaged host processes.
This machine ID is an internal local-executor bucket, not physical-hardware
attestation, and is not emitted in customer VM receipts. Independent service
databases, containers and OS instances require operator placement governance;
different machine IDs never prove different physical machines.

`vm.co_residency` retains actual overlapping managed microVM job identities
(organization, attempt, job), symmetrically across attempts, from observation
of a live confined VMM through owned cleanup. Sequential planned jobs are not
reported as simultaneous peers; completed peer records remain available.
Candidate peer PIDs are independently checked against recorded Linux process
start-time and non-dead state, so a stopped VMM awaiting final journaling or
`placement::finish` is not reported as an observed simultaneous peer.
Boot-only activity before live-process observation and unmanaged processes are
outside this evidence boundary. Admission reservations deliberately cover the
larger whole-attempt interval, independently of these observations.

Optional `cpu_affinity` (for example `0-3,8`) starts the VMM via reviewed
`taskset`; vCPU threads inherit its CPU mask. Whole-host trust exclusion remains
in force irrespective of affinity. Optional `cache_partition` names an
**existing operator-created resctrl CAT group**, not merely a label: before
guest job dispatch Kiln requires L3 schemata, assigns all observed VMM/vCPU
threads to the group's `tasks`, verifies membership and retains group, schemata
and thread IDs in `vm.cache_assignment`. Missing mount/permissions/CAT or failed
assignment refuses guest dispatch. Operators own exclusive masks and writable
delegation; Kiln does not manufacture or prove disjoint CAT masks. MBA only
throttles and does not fully partition DRAM bandwidth. Neither setting closes
all timing channels. A reviewed `version` is recorded as
`vm.backend_version`; the `vm.capabilities` block records the declared network,
snapshot, arch, confinement and per-job scope. Snapshot identity is now
`kiln.microvm.snapshot.v2` and binds the VMM kind, version and binary BLAKE3,
so an old warm snapshot is rebuilt rather than reused across a VMM change.

It does **not** claim:
- hostile multi-tenant or production qualification;
- resistance to KVM or Firecracker vulnerabilities on the host;
- protection from cache-eviction, CPU-contention or DRAM-bandwidth covert
  channels between co-resident microVMs, including pinned separate cores;
- complete DRAM bandwidth partitioning, or an inference that CAT/MBA or
  SMT/KSM disablement eliminates timing channels;
- the separation the jailer would add. The jailer is not used: there is no
  chroot, no dedicated uid or network namespace per VM, and the unit's cgroup
  replaces a per-VM jailer cgroup. The VMM runs as the service user.

It also does not provide:
- confidentiality of the source and secrets that the job is given;
- RNG independence beyond the host seed and VMGenID for restored snapshots.

Hostile admission still needs an operator `qualification_identity`, which is a
reference, not evidence. Receipts keep `hostile_isolation_claim:false`. Every
receipt carries `canonical_outcome`: the BLAKE3 identity of the pipeline,
per-job states and per-step name, state, exit code and stdout. It excludes
timing, cleanup, signals, stderr (the namespace launcher writes `systemd-run`
diagnostics there) and backend observations.

### Opt-in cooperative timing-channel measurements

`scripts/microvm-channels.py` builds a minimal static guest initramfs from
`tools/microvm-channel-probe.c`, runs two diskless/networkless Firecracker
guests, retains serial/raw receiver observations and decodes held-out symbols.
Sender-one symbols perform dependent cache-line chasing (cache eviction),
integer computation (CPU contention), or streaming memory reads/writes (DRAM
bandwidth); zero symbols sleep. The receiver counts corresponding work in the
middle 60% of fixed wall-clock slots. Only the sender receives random held-out
bits; receiver requests contain a symbol count, not the bits. Thirty-two
alternating training symbols select the median threshold and polarity; held-out
symbols determine bit error. Reported raw bitrate is the **offered symbol rate**,
not a reliable exfiltration capacity claim. High error is not channel-absence
proof. This cooperative suite is opt-in qualification evidence, never a CI gate.

Build with a static C toolchain, then run an explicit operator placement JSON:

```console
nix shell nixpkgs#pkgsStatic.stdenv.cc --command python3 scripts/microvm-channels.py build --cc x86_64-unknown-linux-musl-cc --output /tmp/channel-probe
python3 scripts/microvm-channels.py run --config /operator/placements.json --output /operator/observations.json --symbols 64 --symbol-ms 100
```

The JSON keys are `same_host`, `pinned_cores`, `separate_pools`, each containing
`sender`/`receiver` endpoints. Each endpoint supplies `command` (argv for the
worker), absolute `firecracker`, `kernel`, `initrd` paths, optional explicit
`cpus` integer list and a distinct `pool` label for separate pools. Example:

```json
{"command":["python3","/opt/kiln/scripts/microvm-channels.py","worker"],"firecracker":"/operator/firecracker","kernel":"/operator/vmlinux","initrd":"/tmp/channel-probe/probe-initrd.gz","cpus":[0],"pool":"llc-a"}
```

For a remote endpoint use `command:["ssh","pool-b","python3",
"/opt/kiln/scripts/microvm-channels.py","worker"]`; paths are interpreted on
that worker. Provision the same probe image and synchronized wall clocks before
running. The worker records a report-keyed OS-instance pseudonym and observed
core/LLC topology; raw machine IDs are not published. Pinned cores reject SMT
siblings. Separate pools require distinct declared pool names; within one
observed OS instance they also require disjoint observed LLC domains with
explicit CPU pinning and report
`distinct_llc_same_observed_os_instance_measurement_only`. Unequal OS-instance
pseudonyms report only
`distinct_observed_os_instances_physical_placement_unverified`: containers or
separate OS instances can still share physical hardware. This is not admitted
production untrusted/workload placement under Kiln's stronger whole-host rule.
Omitted placements are retained as unavailable, never fabricated. A real
cross-physical-host qualification needs independent operator/hardware evidence,
not just a second OS-instance ID. Worker/VMM timeouts derive from admitted
lead-time and symbol duration; timeout and cancellation own process-group cleanup.

The [2026-11-05 raw reference observations](../.cairn/archive/1970-01-01-complete-host-trust-placement/evidence/reference-channel-observations.json)
retain all nine real guest runs, binary/source BLAKE3 identities and serial
evidence. All used 20 raw symbols/s and 32 held-out bits per run:

| Placement | Cache bit error | CPU bit error | DRAM bit error |
| --- | ---: | ---: | ---: |
| Same host, scheduler placement | 0.40625 | 0.53125 | 0.125 |
| Pinned separate cores 0/1, same LLC | 0.53125 | 0.40625 | 0.46875 |
| Explicit pools llc-a/llc-b, pinned 0/8, distinct LLCs on same host | 0.5625 | 0.53125 | 0.4375 |

CPU0's LLC is `0-7,16-23`; CPU8's is `8-15,24-31`. The last row is genuine
distinct-LLC pool measurement, **not distinct-host evidence**; shared DRAM and
other host resources remain. These short samples are observations, not a
statistical detection conclusion. Host SMT was on, KSM off, and no host setting
was changed. The older trusted measurement guest used kernel 6.18.42, not the
new fixed-series guest pin. CAT/MBA programming was not qualified here: a mounted
resctrl filesystem, supported CAT hardware and a writable operator-created
exclusive CAT group are the exact external prerequisites. Every receipt and
report retains `hostile_isolation_claim:false`.

The historical nine rows were privacy-redacted after review: raw worker machine
IDs were replaced with per-evidence `REDACTED` OS-instance labels preserving only
equality. Raw samples, serial, rates and historical cohort hashes are unchanged;
the redaction and corrected non-hardware classifications are explicit metadata.

The separate [2026-11-06 aspen1 maintenance experiment](operator.md#readiness-reconciliation-and-qualification-2026-11-06)
booted host Linux 7.3.0, disabled SMT, and used exclusive operator-created CAT
groups with independent live membership/affinity sampling. Three pinned-core
rows at an offered 10 symbols/s had held-out errors of 3/64 (cache), 34/64 (CPU),
and 0/64 (memory). The maintained worker itself remains CAT-unaware; the retained
operator sidecar is separate evidence, not continuous enforcement or a hardware
isolation proof. No second physical host was allocated. Native space-padded
`L3:`/`L3CODE:` rows are accepted by hosted CAT admission; non-L3-only input is
still refused. This does not broaden the `hostile_isolation_claim:false` boundary.


### Cloud Hypervisor (per-job profile)

Cloud Hypervisor cold boot is wired through the hosted shell. A host run on the
reference host now matches the namespace/Firecracker canonical outcome, and the
retained escape suite passes: `/sys/class/net` is only `lo`, `/sys/block` is
only `vda`, namespace creation is denied and vsock egress is denied. Warm-pool
restore now passes on the same host, and the escape suite still passes. Cold
ready time was about 1.0 s and restored ready time about 0.8 s in this
single-host run. CET handling across a Cloud Hypervisor snapshot remains
unverified and is recorded as a non-claim.

Implemented per-job profile:

- cold boot with `--kernel`, `--initramfs`, `--cmdline`, `--cpus`, `--memory`,
  a read-only `--disk` source image, `--vsock`, `--seccomp true` and no
  `--net` or `--api-socket`;
- planned warm restore with `--restore source_url=file://…,resume=false`, then
  a host-side resume only after confinement observation;
- the same `<vsock>_10240` guest listener and `kiln.microvm.guest.v1` protocol.

Claims: no guest network device, rootless `systemd-run --user`, a read-only
source disk, the shared vsock protocol, `CapEff 0`, `NoNewPrivs 1`, and at least
one seccomp-filtered worker thread. The control thread itself reports
`Seccomp 0`; that is an explicit non-claim.

Non-claims: no retained or migratable session, no cross-host migration, no CET
snapshot handling, and no claim that its larger device model (PCI, hotplug,
VFIO) is as small as Firecracker's.

### Guest networking (non-goal)

<!-- r[impl kiln.hosted.microvm.network.nongoal] -->
A job in the Firecracker backend gets no guest network interface: no TAP device,
no virtio-net, no in-guest tunnel. This is a deliberate part of the isolation
claim, not a missing feature. The guest's only channels are the read-only source
drive, one vsock device and an entropy device.

If egress is ever needed (for example, fetching dependencies), the preferred
design is host-mediated: the guest asks over the existing vsock channel, a
host-side component enforces a reviewed allowlist and size and time bounds, and
every fetch is recorded in the job's receipt. The guest still never receives an
IP path. The debug hold already follows this rule: its commands travel over the
existing vsock session and it adds no network device.

Rejected alternatives:

- a guest NIC on a host bridge or NAT;
- an in-guest or host-terminated L3 tunnel, including a userspace WireGuard
  implementation such as sanstun.

Both give the guest general IP reachability, and the escape suite would have to
stop asserting no IP egress. Any change that adds guest networking must update
the isolation claim and the escape suite in the same change.

<!-- r[verify kiln.hosted.microvm.network.nongoal.scenario.escape] -->
The coupled check is the retained guest-escape suite: inside the guest it probes
TCP `1.1.1.1:443`, `10.0.2.2:80` and `169.254.169.254:80`, and vsock ports 10240,
1024 and 22, and every probe must report `denied`.

**Untrusted pull-request runs.** <!-- r[impl kiln.untrusted.execution] -->
An `execution.run` request may carry
`untrusted = {reason, source_commit, approver_id}`: the provenance of a fork or
outside-contributor pull request whose exact commit an interactive operator
approved (see the operator guide's untrusted pull request policy). Trusted
requests omit it and keep their request identity. Before any reservation or
effect, every job's runner must be `kind = "firecracker"` with
`untrusted_capable = true`, and the provenance must name an exact 40- or 64-hex
commit and a bounded approver; anything else is refused. Such an attempt, and
every retry attempt that carries the same provenance, gets no workload policy:
no secret environment and no `KILN_WORKLOAD_TOKEN`. Its cache is disabled with
outcome `disabled_untrusted`, so no restore or publication receipt exists.
There is no per-run override, and debug holds are never offered. Hosted GitHub
connections admit only `untrusted_pull_requests = "deny"`. Generic
`execution.run` checks caller-supplied provenance, not a Web approval. The
[explicit Web bridge](#explicit-web-approval-to-hosted-execution) separately
verifies the configured approval issuer and exact stored source/gate binding.

**Debug hold.** With `debug_hold.enabled`, a job is held only when all of the
following hold:
- its guest session completed and the job failed (a non-zero exit, timeout,
  truncated output or refused source);
- it had no secret grant and no workload-token audience;
- the attempt was trusted (hostile trust and approved untrusted pull-request
  runs are never held; there is no override);
- the runner has fewer than `max_concurrent` active holds.

The result is journaled first and never changes afterwards. The lifecycle then
goes `Journaled → Held → Destroying → Destroyed` (or `Abandoned` when teardown
cannot be proven). A hold ends at the first of:
- VMM exit;
- loss of the guest session;
- a storage failure;
- attempt cancellation;
- `execution.debug_release`;
- capacity pressure (an `execution.run` refused for capacity while holds are
  enabled);
- grace expiry.

A held VM keeps its attempt's `maximum_concurrent` slot (and its warm slot).
For jobs that could be held, the unit's `RuntimeMaxSec` gains the grace plus
2 s, and admission adds one grace to the attempt's maximum (hold time is billed
as attempt time).

Debugging goes through a host-side relay; there is no network device and no
new vsock listener:
1. `execution.debug_issue {project, attempt_id, job}` needs a write role
   (`owner`, `admin`, `member`) on the project. It returns a fresh 32-byte
   capability that is single-use, bound to the calling peer, and expires
   after 120 s or with the hold. Only its BLAKE3 digest is stored. At most 16
   capabilities are issued per hold.
2. `execution.debug_exec {project, attempt_id, job, capability, command}`
   re-checks write access, consumes the capability atomically and queues one
   session. The job thread that owns the held VM relays the command over the
   guest's existing vsock session. The guest accepts debug frames only after
   its run failed. It runs `bash -c COMMAND` in `/work` as uid 1000 with
   `no_new_privs`, the base environment (no step variables), a deadline of
   20 s or the rest of the hold, and 64 KiB per output stream. The call
   returns the report; a second use is refused.
3. `execution.debug_release` (write role) ends the hold early.
4. `execution.debug_list {project, attempt_id}` (any project role) lists
   holds and sessions.

Sessions record the peer, request, start and end times, duration, command
BLAKE3 and the bounded output. They grant no source, CI or review authority.

While a job is held, `execution.get` ends with `debug_holds[].connect`, the
exact connection instructions. The receipt's `microvm.debug_hold` records the
hold outcome (`held`, or `not_held` with the reason), end reason, held time,
session count and the instructions that applied.

Each command is one non-interactive shell invocation, not an interactive PTY,
so it fits the 30 s RPC exchange. A guest image built before this feature
closes its session after the run, so a debug command to it fails and ends the
hold as `guest_lost`.

Materialize only reviewed customer source under
`work_root/acme/app/RELATIVE_WORKSPACE` (no symlink escape). This local execution
API does not fetch upstream source by itself. Before approved local execution,
review the pipeline and admit its finance account/price. Explicit host
`execution.trusted_projects = [{organization = "acme", project = "app"}]`
is required for `trust:"trusted"`; the customer's assertion is not authority.
Also admit `execution.pricing` entries `{organization,runner,price_id,class,max_units}`
matching **every** compiled runner and the selected exact price/class. Both
lists default empty and fail closed before dispatch. A member with project
scope calls `execution.run` with
`{"project":"app","pipeline_id":"REVIEWED_PLAN_ID","attempt_id":"attempt-1","price_id":"EXACT_PRICE_ID","class":"EXACT_CLASS","workspace":"checkout-1","trust":"trusted"}`.
The executor selects only host-mapped runners, reserves before the first
process and executes ready jobs sequentially; it does not promise parallel
matrix scheduling. Its worst-case financial reservation is
`ceil((sum(job timeout_ms + 20000))/price.unit_ms)` and must fit every matching
operator policy's `max_units` and the admitted price maximum. Approve compatible
price/ceilings before calling. A deadline above the 30-second customer
exchange can outlive that exchange; retain the same request and attempt for
recovery, not a fresh speculative rerun.

`execution.get` input `{"project":"app","attempt_id":"attempt-1"}` reads
retained result/cleanup observations. Owner/admin `execution.recover` with the
same input reaps an uncertain systemd unit; it does not rerun it or settle its
unknown financial obligation. Missing teardown evidence keeps capacity
unavailable. `execution.cancel` takes the same `{project,attempt_id}` input and
requires owner/admin/member project scope. It signals the exact currently
running attempt in this service; its acknowledgement is a cancellation request,
not proof of teardown. Inspect `execution.get` for retained outcome and cleanup.
After restart use `execution.recover`, not a fresh speculative rerun. A transport
disconnect alone does not cancel a job.

### Optional tenant-scoped execution cache

`execution.cache` defaults to `null`. A host may admit an explicit
`{directory,maximum_entry_bytes,maximum_total_bytes,maximum_entries,maximum_age_seconds,maximum_files,policies}`
configuration. The directory must be an existing private root outside the
customer work tree, not an ancestor or descendant of `work_root`. Hosted bounds
are 1 MiB per entry, 1 GiB total, 4096 entries/files and 31536000 seconds maximum
age, all positive. Runtime also requires total bytes at least entry bytes and
nonempty unique exact organization/project/runner policies. Each policy has
`{organization,project,runner,worker}`, where `worker` is
`{producer,toolchain,platform,inputs,paths,maximum_entry_bytes,maximum_files}`.
Use source-relative input files and separate relative output roots; for example
`inputs = ["Cargo.lock"]`, `paths = ["build-cache"]`. Input/output overlap,
traversal and symlink/hardlink/special-file transfers are denied. Worker entry/
file limits must fit the host policy.
Firecracker policies additionally fit the hex-encoded cache frame **including
JSON and worst-case escaped nonce overhead**; namespace-only cache policies
are not restricted by this vsock limit. Firecracker admits at most 64 output
roots with at most 100 bytes per root. The guest return supports plain ustar
regular files/directories only. BusyBox GNU long-name records and PAX extended
headers are rejected explicitly (even when a path passes the generic
1024-byte `safe_path` rule); they do not silently truncate or select another
path. Such a return records `publish_error` and leaves successful job results
and existing cache entries unchanged. Keep returned relative paths short,
including directory separators and directory trailing slashes.

Start each attempt from freshly acquired source with every configured output
root absent: restoration refuses to overwrite an existing root. Keys bind the
organization/project tenant, exact execution configuration, compiled plan/job,
worker policy and BLAKE3 of source input files. An input/configuration/tenant
change cannot authorize reuse of a different key. Maintained descriptor-relative
transfer/store validation, finite pruning, content verification and serialized
cache ownership apply; cache bytes grant no execution or source authority.

The executor measures actual restore miss/hit and publication work with host
monotonic time, returning `cache.restore`/`cache.publish` observations. It
publishes only after successful execution and confirmed owned cleanup, and
rehashes source inputs before publication. Secret-bearing or workload-audience
granted jobs disable **both** restore and publication; do not treat redaction as
permission to cache secret-derived output. Absent/disabled cache work has null
duration, not fabricated zero; a measured zero duration can occur at millisecond
resolution and is not by itself hit evidence. Cache state is expendable,
excluded from finance/replay authority and not included in the recovery bundle.
No remote cache service, cross-tenant sharing permission or performance claim is
established by enabling this local bounded cache.

r[kiln.hosted.microvm.cache_integrity]

For any guest behaviour, at every point where the service, Firecracker unit,
or machine stops, existing cache entries must remain byte-identical to their
recorded BLAKE3, and a new entry must be absent or complete and verified: no
partial entry is visible under a key. This is a tested integrity requirement,
**not a proof**, power-loss qualification, confidentiality guarantee or hostile
isolation claim; receipts retain `hostile_isolation_claim: false`. Filesystem
atomic-rename/fsync semantics remain a deployment prerequisite.

The exclusive store owner writes under `kiln-cache-v1/staging`, outside the
64-hex key namespace, verifies the staged envelope and per-file bounds/BLAKE3,
fsyncs the staged file, uses one atomic `RENAME_NOREPLACE` into the key namespace,
then fsyncs both directories. It never updates an existing entry in place,
including when a second guest return conflicts with the first. Opening the
marked store under its owner lock removes orphan staging files; recovery never
promotes them. Corrupt entries are preserved but not restored or mounted;
`cache.restore.outcome = "corrupt_miss"` records a measured cold build without
changing the job result. The web cache endpoint preserves that same typed outcome
in its `{entry,outcome}` response, and workers retain it in step telemetry.
If accumulated retained data exceeds the bounded inventory scan, restore returns
`disabled_inventory_bound` rather than mounting an entry or failing the job.
Neither outcome authorizes publication: the host/worker skips publication for
that attempt, preserving the existing corrupt bytes without repair or deletion.
Other policy, source, transfer, ingress and publication errors remain strict;
this is not blanket suppression of cache security failures. Hosted receipts mark
the skipped publish as `skipped_restore_disabled` with null duration.
Verified hits and publications carry `entry_blake3` and `source_tier = "local"`;
misses carry a null entry hash. Remote object storage is not implemented, so no
remote latency/p99 or speedup is asserted.
If a remote tier is added, hash verification must precede use and its latency
distribution must be reported separately from local hits.

The guest sends bounded output bytes while it is alive, but host persistence
starts only after successful execution and recorded `Destroyed { journaled:
true }`. A `destroy_uncertain` or failed journal attempt returns no publishable
bytes. Warm snapshots contain an empty read-only cache device, never tenant
cache bytes. After snapshot load, the host checks the Firecracker PATCH API
response while binding a verified job image to `/drives/cache`; only then does
it send Run. The empty template remains unchanged, including across cache
misses and different source keys. Firecracker/kernel/initrd identities are
bound by the exact execution configuration in the key.

Regression commands (run in the reviewed development environment):

```console
cargo test --locked -p kiln-web-service cache::tests::
cargo test --locked -p kiln-hosted cache::tests::
cargo test --locked -p kiln-web-service worker::cache::tests::
cargo test --locked -p kiln-web-service telemetry::tests::
cargo test --locked -p kiln-web-service --test engine cache::
cargo test --locked -p kiln-microvm archive::tests::
cargo test --locked -p kiln-microvm protocol::tests::
cargo test --locked -p kiln-hosted microvm::tests::unconfirmed_or_unrecorded_destruction_cannot_publish_guest_bytes
KILN_MICROVM_REQUIRED=1 KILN_MICROVM_GUEST=/nix/store/REBUILT-kiln-microvm-guest \
  cargo test --locked -p kiln-hosted --features cache-fault-tests --test microvm firecracker_ -- --nocapture
KILN_MICROVM_REQUIRED=1 KILN_MICROVM_GUEST=/nix/store/REBUILT-kiln-microvm-guest \
  cargo test --locked -p kiln-hosted --test microvm namespace_corrupt_and_saturated_cache_preserve_successful_cold_jobs -- --nocapture
```

Use the actual output of `nix build --no-link --print-out-paths path:.#microvm-guest`
for `KILN_MICROVM_GUEST`, not the illustrative path above. The store fault
fixture starts real child publisher processes and sends SIGKILL at partial
receiving, verifying, before rename and after rename, then reopens the store
and checks old bytes, complete-or-absent new keys and orphan deletion. The
opt-in compile-time `cache-fault-tests` harness synchronizes a real systemd
Firecracker unit SIGKILL at the guest return boundary, positively observes the
callback/kill, and checks that already completed steps remain successful with
no cache publication. No production environment or configuration fault switch
exists. Persistent publish phases have **no live
Firecracker unit to kill**, because recorded destruction is a prerequisite.
The KVM journeys cover cold/warm restore, guest writes and later unchanged
restore, corruption cold builds and different-key warm-slot reuse. These
commands describe the verification procedure, not a claimed passing result.

## Web approval through Lattice to hosted execution

<!-- r[impl kiln.web_hosted.binding] r[impl kiln.web_hosted.admission] r[impl kiln.web_hosted.replay] r[impl kiln.web_hosted.adapter] -->

The ordinary `kiln-web-service` dispatches through Lattice. A genuine saved
workflow, served by `lattice workflow-exchange serve`, invokes the fixed
`kiln-hosted lattice-job` task. Lattice owns the outer DAG and workflow/node
lifecycle. One outer node delegates one reviewed hosted pipeline; its internal
jobs are not represented as additional Lattice nodes. There is no Web execution
provider selector or separate Web/hosted server. Existing default Web profiles
retain their serialized identities. Earlier direct-route receipts under
`add-web-hosted-approval-bridge/evidence/` are superseded historical evidence,
not proof of this topology.

Author the task with `config/hosted-lattice-job.ncl`. Its JSON pins the Web HTTPS
origin, CA and worker-token file, dedicated private Iroh network/key, hosted
`EndpointAddr`, organization, project/repository, node and backend policy. Pass
the BLAKE3 of the **original JSON bytes**, not reserialized content:

```sh
nickel export --format json task.ncl > task.json
b3sum task.json
kiln-hosted lattice-job --profile /absolute/path/task.json --identity EXACT_BLAKE3
```

The last command belongs in the reviewed saved native shell workflow, not a
parallel Web dispatcher. Pin its workflow revision, consumer graph and exact
node operation in the native handler's `route_exact`/`shell_environment` route.
Use a reviewed executable cohort and native process teardown grace above the
task's admitted `cleanup_timeout_ms` (default ten seconds; the smoke configures
forty-five seconds and sixty seconds of native grace). This needs
a cancellation-capable native executor, not just vendored transport or a
forwarding handler.

The unchanged native request injects only `LATTICE_SOURCE_PROVIDER`,
`LATTICE_SOURCE_REPOSITORY` and `LATTICE_SOURCE_REVISION`. Authenticated worker
lookup and Worker/Operator-only read-only
`GET /api/runs/{id}/execution-binding` recover the exact active persisted plan,
run, source, approval and native execution. Lookup and backend observation have
finite deadlines; native context gains no arbitrary approval payload.

The task's Web client uses rustls. Configure `tls_ca_path` as the issuing CA and
serve an end-entity server certificate (`basicConstraints CA:FALSE`,
`extendedKeyUsage serverAuth`) whose SAN covers the configured origin host. A CA
certificate served as the leaf is refused as `CaUsedAsEndEntity`.

Hosted `lattice_execution` policies in `config/hosted-service.ncl` pin the task
peer, Web project/repository, workflow policy, node/operation, enrolled Git
repository/snapshot limits, hosted project, pipeline, price/class and fixed Web
HTTPS authority. Hosted independently fetches that authoritative binding and
independently verifies the exact Git commit and canonical snapshot before
execution or financial effects. The task peer alone is **not** approval
authority. HTTPS authenticates the configured Web authority, not a human click
or an uncompromised Web process. Ordinary member `execution.run` provenance
remains caller-supplied and gains no verified Web claim.

`execution.lattice_run/get/cancel` bind the complete persisted request, native
execution, node and configuration to one durable attempt per Web run/node.
Changed replay conflicts. Recovery never reacquires, redispatches or recursively
calls Web. The `lattice-*` attempt/workspace and `.lattice-source-*` staging
namespaces remain reserved without a policy. Metadata reservation can precede
the callback; reservation alone remains `Unknown`, not verified execution or an
execution receipt. Original refusals are retained.

SIGINT/SIGTERM preserves cancellation intent through initial admission, targets
only the exact backend attempt and observes actual terminal cleanup. The
authoritative worker projection also exposes durable `cancel_requested` intent,
and hosted observes that persisted intent while its pipeline runs. This matters
because the native runner may destroy a cancelled task without a signal: Web
cancellation intent, not task liveness, is the authority that reaches the
backend. A pending admission acknowledgement is not completed cancellation.
Uncertain transport never resubmits a run; bounded get/cancel recover only the
original attempt. Native Lattice owns the outer outcome. Actual bounded
stdout/stderr use existing Web captured-log ingress with HTTP/base64-safe chunks
and declared byte/reference budgets; these are retained backend outputs, not
streaming partial guest output or synthetic node success.

All project, finance, resource, runner, kernel, SMT/KSM and CAT gates remain.
Approval is not a host-safety waiver. Source-verified kernel refusal is **not**
approved-untrusted VM execution. `hostile_isolation_claim` remains false;
eligible-host untrusted and distinct-physical-host qualification have separate
prerequisites.

With accessible KVM, the existing user manager and reviewed cohorts, use fresh
short private scratch **outside `/tmp`** so its `TMPDIR` fits the Unix-socket
bound and remains visible to the user manager:

```sh
python3 scripts/web-lattice-hosted-smoke.py --hosted /absolute/path/kiln-hosted --web /absolute/path/kiln-web-service --lattice /absolute/path/native-lattice --lattice-source /absolute/path/reviewed-native-source --guest /nix/store/REVIEWED-kiln-microvm-guest --root "$PWD/target/klh-proof" --evidence "$PWD/target/klh-evidence" --trusted-execution
```

The controlled **trusted** lane exercises real guest stdout, live HTTPS
cancellation, replay/restart and teardown without qualifying untrusted execution.
The untrusted lane requires the exact workstation kernel refusal, not a waived
gate. Retain only whitelisted evidence outside scratch; remove owned credentials
and raw databases after teardown. Kiln Web/hosted maintainers own approval,
source admission, task integration and this repeatable recipe; Lattice owns
native scheduling/lifecycle. Operators retain host, source and credential
authority.

## 7. Bounded fleet and timing observations

A service `fleet` profile supplies a reviewed immutable driver `{path,blake3}`,
an `organizations` allowlist, optional `pricing = {price_id,class,max_units}`,
`minimum`, `maximum`, `slots_per_instance`, `timeout_ms`. Provisioning requires
the organization grant and admitted finite pricing: finance reserves exposure
and marks it unknown before invoking the driver. A provider presence receipt
is not trusted usage metering and does not settle that obligation. No built-in
cloud credentials or free capacity are supplied. The executable must implement
`--kiln-fleet-v1` with bounded JSON stdin/stdout protocol
`kiln.hosted.fleet-driver.v1`; the host clears environment, bounds time/output
and owns the driver process group. Its receipt must match organization,
request and instance identities. Provision/reconcile behavior and credentials
inside that reviewed driver require independent provider/security review.

Owner/admin `fleet.configure` takes
`{"minimum":0,"maximum":1,"slots_per_instance":1,"expected_revision":0}`;
customer selection may narrow but not exceed operator bounds. `fleet.provision`,
`fleet.deprovision`, `fleet.reconcile` take
`{"instance_id":"worker-1","request_id":"fleet-1","expected_revision":CURRENT}`
(new instance revision zero). Reconcile inspects the retained provider request,
not a new provisioning retry. Global operator capacity and per-organization
capacity both apply, and deprovisioning preserves the active minimum floor.
Unknown instances still consume capacity and prevent unsafe profile changes.
`fleet.status` takes `{}` and reports retained
instances and bounded per-job queue/cache observations. `cache_ms` is nullable;
use `cache_outcome` and the execution restore/publish receipts to distinguish
disabled, miss and hit rather than inferring a hit from elapsed time. There is no
`fleet.scale` RPC to substitute for these exact actions. Fleet receipts are not
production supplier capacity or availability qualification. Do not provision
paid capacity without separate exact operator approval.

## 8. Secret references and interoperable workload JWTs

Service `secrets` imports `config/hosted-secrets.ncl`: HTTPS issuer without a
trailing slash, a distinct owned mode-0600 raw32 Ed25519 signing key,
nonempty approved audiences, lifetime at most 300 seconds and exact scoped
SecretSpec resolver references. `kiln-hosted keygen --key /absolute/issuer.key`
can create the raw seed format, but use a **separate** key from transport.
SQLite retains references, revocation and binding metadata, not resolved values.
The retained issuer policy is immutable per database; key rotation/policy change
requires a separately reviewed migration, not editing a live profile and hoping
restart overwrites authority.

Owner/admin `secrets.register`:
`{"project":"app","name":"deploy-token","resolver":"APPROVED_RESOLVER_ID","expected_revision":0,"expires_ms":FUTURE_MS}`.
`secrets.list` takes `{"project":"app"}`;
`secrets.revoke` takes `{project,name,expected_revision}`;
`secrets.revoke_job` takes `{"project":"app","job":"EXACT_BOUND_JOB_ID"}`.
Customer RPC cannot inject resolver executables, retrieve values or mint job
capabilities/JWTs. Disabled secret policy gives an explicit operator-policy
failure, not an empty secret fallback.

The trusted executor derives organization/project/job/admission identity,
expiry, audience and secret revisions from an admitted job and the host's exact
`execution.workload` policy for organization/project/runner. It resolves only
those named grants and inserts explicitly selected secret environment values;
its short-lived EdDSA JWT is delivered as `KILN_WORKLOAD_TOKEN`. Capability
exchange is consume-once and expiry/revocation is checked against retained
state. Job finish revokes retained capability authority. Output redaction
covers the exact delivered values/token, not arbitrary encodings, generated
files, guest disks or an assurance against malicious exfiltration. Never grant
platform/source/payment credentials to general customer jobs.

`secrets.discovery` and `secrets.jwks` take `{}` and return metadata/JWKS over
authorized Iroh RPC. The local bridge exposes their unwrapped JSON through
`GET /.well-known/openid-configuration` and `GET /jwks.json`; admission or
Iroh failures produce HTTP 503. These local HTTP routes are not a public HTTPS
issuer. Independently provision approved HTTPS DNS/TLS and route **only** these
metadata paths under the exact configured issuer, preserving the bridge's exact
loopback Host and permitted Origin. Discovery advertises `ISSUER/jwks.json`;
an issuer path prefix requires an explicitly reviewed proxy mapping. Do not
publish `/rpc`, `/session` or the administrative UI along with issuer metadata.
Establish the relying party's explicit issuer/audience/subject/scope policy and
review key custody/revocation behavior. No TLS endpoint or cloud role trust is
created by this executable. Standard
signature/expiry validation by a cloud provider does not automatically consult
Kiln's immediate local job revocation state; design that trust boundary
explicitly. Missing HTTPS routing, credentials, audience approval or cloud trust
is an operational prerequisite, not a passing workload-identity deployment.

## 9. Incident holds, content, support and database recovery

Configure service `operations` via `config/hosted-operations.ncl` using the same
`service_id`, three existing distinct mode-0700 non-symlink roots
`backup_root`, `restore_root`, `content_root`, and explicit database/content/
record/deadline bounds. For example create those local directories with
`mkdir -m 700 /absolute/private/backups /absolute/private/restores
/absolute/private/content` after their private parent exists. These roots and
limits are pinned in retained state; a changed profile is rejected.
Also supply `replay_roots`, the explicitly trusted absolute private mode-0700
directories containing every retained native source replay database. Use `[]`
only when no source stores exist. Preserve these exact paths on a fresh
recovery host; replay database and sidecar conflicts are denied, not overwritten.

* `operations.hold.set`:
  `{"held":true,"reason":"investigating uncertain effects","expected_revision":0}`;
  owner/admin, attributable revisioned hold. Clear only with current revision
  and independently approved reason. `operations.hold.get` takes `{}`.
* `operations.health` takes `{}`; it reports organization-local observations,
  not provider health, SLOs, uptime or customer support coverage.
* `operations.support.record` takes
  `{"subject":"attempt-1","observation":"operator observation without secrets"}`;
  this is an attributed local note, not a support ticket delivery, escalation
  policy or staffed response promise.
* `operations.audit.list` takes `{"after":0,"limit":100}`; owner/admin,
  maximum 200 rows per call, page by last returned sequence.
* `operations.content.put` takes
  `{"content_id":"log-1","content_base64":"STANDARD_BASE64","expires_ms":FUTURE_MS}`.
  Stay within both module bounds and the outer RPC frame. Owner/admin
  `operations.retention.sweep` takes `{"limit":100}` and deletes only expired
  owned content; it never prunes financial, account or replay protection.

For an online database bundle, `operations.backup` takes
`{"backup_id":"snapshot-1"}` and requires both current organization owner role
and an explicit service `operator_peers` host-wide peer grant (at most 16 actual
Iroh public IDs). The default empty list denies every backup RPC; bootstrap or
ordinary organization ownership does not grant host-wide snapshot access. Add
only independently approved service operators, since a bundle includes all
organizations. This field grants no other RPC host authority. The service
fences external effects and snapshots
the primary with a SQLite immediate-transaction boundary. The CLI alternative
requires the service's exclusive lock, so stop `serve` gracefully first
(SIGINT/SIGTERM). Run backup on the source host and restore on a **fresh recovery
host** after securely transferring the verified bundle/profile and independently
reestablishing the same trusted replay-root paths:

```sh
kiln-hosted backup --profile "$HOME/kiln-hosted-local/server.json" --id snapshot-1
kiln-hosted restore --profile "$HOME/kiln-hosted-local/server.json" \
  --id snapshot-1 --target restored-1 --identity MANIFEST_BLAKE3
```

Retain the returned manifest identity through an independent trusted channel;
substitute it for `MANIFEST_BLAKE3`. Backup IDs and target IDs are new single
components (letters/digits/underscore/hyphen), never reused or overwritten.
Restore verifies the trusted `kiln.hosted.backup.v2` manifest identity,
service/schema, every file's bytes and BLAKE3, SQLite integrity/foreign keys,
schema identity and table-row inventory. The bundle includes the **whole hosted
SQLite database plus every retained native source replay database** (at most
128, all sharing `maximum_database_bytes` as a total bundle bound), including
original native paths and exact inventory. Hosted account, finance/unknown
obligations, replay, review, incident and secret metadata are not filtered to
one customer. Restrict backup access accordingly.

Restore creates native replay stores only at their original explicitly admitted
paths, preserves native profile/run identities without rewriting configuration,
and publishes `restore_root/restored-1/hosted.sqlite` **last**, after every native
store is verified and durable. It prints that primary path. Corruption, wrong
service, unsafe path, oversized bundle, missing manifest, preexisting target or
existing native database/sidecars fail closed without overwriting. Partial
failures retain reserved targets; never erase them and retry blindly. An
incomplete backup is not restorable. Do not run this against existing source
stores on the active host and expect them to be replaced.

The bundle does **not** include private transport/issuer keys, resolved
SecretSpec values/provider stores, external Lattice journals, worker state,
customer workspaces, expendable execution cache, content blobs, VM images or an upstream repository backup.
The content catalog is preserved, not the corresponding files. Content
retention is not backup retention. Preserve those external dependencies with
their own reviewed backup consistency/identity and recovery procedures; a
database-bundle restore is not end-to-end CI recovery.

`restore` does **not** replace the original `state_directory/hosted.sqlite` or
start a listener. It does install the bundled native stores at their original
paths on the fresh recovery host. Separately approve a quiesced authority
cutover to the printed restored directory as the new service state root,
preserve pinned profile/keys/external stores, and prevent original/restored
services from both dispatching. Do not edit unknown reservations/replay rows or
drop current authorization to make a restore appear healthy. Reconcile external
effects and retained obligations before allowing new work.

## Unikernel tests: static PIE, no shell and no network

`kind: "unikernel-test"` is a separate trusted-only job class for **one prebuilt
static x86_64 little-endian Linux ELF64 PIE**. The ELF must be `ET_DYN`, have no
`PT_INTERP` and no `DT_NEEDED`; a dynamically linked PIE is not static. Default
Go `ET_EXEC` binaries are not supported by the selected loader. Jobs requiring a
shell or general Linux ABI keep the Linux microVM backend.

```json
{"kind":"unikernel-test","runner":"unikernel-reviewed",
 "unikernel":{"binary":{"path":"bin/tests","blake3":"<64 lowercase hex>"},
              "argv":["pass"],
              "fixtures":[{"path":"fixtures/input.json","blake3":"<64 lowercase hex>"}]}}
```

Paths are relative to the acquired exact source snapshot; regular-file identities
are verified before image construction, and symlinks/traversal are refused. The
binary becomes `/test`; fixtures retain their declared paths. There are no shell
`steps`. Multiple steps, a shell step, executable replacement, subprocess harness
or network fields are refused, rather than converted to a shell command. The
reviewed argv is fixed: no expansion, environment, matrix, reusable template,
whitespace/quoting or kernel-parameter token injection. No cache transfer,
credentials, warm pool, failed-job debug hold or guest writable output collection
is available for this backend. The native image is an immutable deterministic
newc initrd, not a disk collecting files after execution.
Fixtures are installed with read-only file permissions. The local Unikraft patch
restores persisted RAMFS permissions when recreating vnodes; upstream lookup's
unconditional all-permissions mode otherwise made a reopened fixture writable.
CPIO extraction uses temporary owner-write permissions before its final chmod.
The maintained Rust `fixture` case reads declared bytes, rejects a write-open,
then reads the unchanged bytes in both the raw and hosted native rails.
The native parser requires an explicit `KILN_ARGV_END` fence after the reviewed
arguments and discards VMM-added suffix tokens; customer use of that marker is
refused. Real fixture programs assert exact argv, so an appended VMM argument
cannot silently change the binary's invocation.

### Native backend and upstream identities

The bounded source/native-tool spike compared
[Nanos](https://github.com/nanovms/nanos/tree/ab823eb4bbcaf0874dc5e15568938bc02346d3d3)
(Apache-2.0) and [OPS](https://github.com/nanovms/ops/tree/a52795e38ba33a006f0ba740fee0f18fec58fbbb)
(MIT) against [Unikraft app-elfloader](https://github.com/unikraft/app-elfloader/tree/0cd7a2b2c4d5ece85647c0c9903b50426e849420).
Nanos built with native tools, but its default PVH image stopped before application
execution on Firecracker 1.16.1; the Linux-entry variant reached device probing
without completion within the bounded ten-second boot. This is an observed cohort
failure, not a claim that Nanos can never support Firecracker. OPS was researched
as Nanos's recommended image tool, not used as a substitute guest.

The selected working cohort is Unikraft core (BSD-3-Clause default, with per-file terms)
[`a4c494e80141bd402961c7d96150dd218c79db37`](https://github.com/unikraft/unikraft/tree/a4c494e80141bd402961c7d96150dd218c79db37),
ELF loader
[`0cd7a2b2c4d5ece85647c0c9903b50426e849420`](https://github.com/unikraft/app-elfloader/tree/0cd7a2b2c4d5ece85647c0c9903b50426e849420),
and libelf
[`c03d4ceb22a2b9e9799238f9c277a14284d313ad`](https://github.com/unikraft/lib-libelf/tree/c03d4ceb22a2b9e9799238f9c277a14284d313ad).
The lock retains exact upstream NAR identities; the native guest manifest also
records deterministic source-archive BLAKE3, defconfig and Kiln lifecycle-patch
identities. No OSv fallback was necessary after both real static language binaries
executed under this cohort. Upstream's [ELF loader instructions](https://github.com/unikraft/app-elfloader/blob/0cd7a2b2c4d5ece85647c0c9903b50426e849420/README.md)
and [process capability matrix](https://github.com/unikraft/unikraft/blob/a4c494e80141bd402961c7d96150dd218c79db37/lib/posix-process/README.md)
describe the compatibility boundary; Linux binary compatibility is not universal.

Rust fixtures use Rust 1.89's self-contained musl target and `-C
relocation-model=pic`; the ordinary static glibc Rust spike failed TLS allocation.
Go fixtures use `CGO_ENABLED=1 go build -buildmode=pie` with
`-ldflags='-linkmode=external -extldflags=-static-pie'` and the pinned static libc.
The flake fixes producer inputs and removes path/timestamp/build-ID variation.
The native kernel has no lwIP network stack. Multiprocessing is disabled;
elfloader's initial transfer is allowed, but subsequent guest `execve` is denied.
The real probe also attempts `fork`, IPv4 socket/connect, and vsock socket/connect
for CIDs 1 and 2: every attempt must fail. There is no NIC or vsock device in the
Firecracker configuration. This declaration does **not** replace the hosted
`ConfinementObserved` process/cgroup/seccomp gate before application dispatch.

Native startup requires the reviewed rootless systemd preboot policy
`kiln.native.preboot.v1`, authored in `config/unikernel-preboot-seccomp.ncl`.
Its explicit default-deny syscall allowlist is the reviewed expansion of
systemd 262's `@system-service`; unsupported syscalls return `EPERM`. The typed
source and embedded list are checked for exact agreement, and the receipt binds
both BLAKE3 identities. Before `InstanceStart`, Kiln verifies the unit's actual
expanded `SystemCallFilter` and errno against that exact list, records the
running user-manager version/features, and applies the **strict Firecracker**
observer: main and worker threads require `Seccomp: 2`. The Cloud Hypervisor
unfiltered-control-thread exception is never used for native Firecracker.
The inherited preboot filter is substantive confinement while Firecracker later
installs its own filters; it is not an allow-all filter used to manufacture a
status value. Missing/changed policy or seccomp 0/1 prevents test dispatch.
Refusal stops the owned transient unit before joining launcher capture workers;
service stdout cannot outlive its owner and obscure the policy failure.
The [real rootless preboot spike](../.cairn/archive/1970-01-01-add-unikernel-test-runner/evidence/strict-preboot/observed-policy.json)
retains the exact observed policy and manager cohort: without the preboot policy
the main thread reported seccomp 0 and was refused; with the explicit policy,
both threads reported seccomp 2 **before** application start and the actual
native Rust test passed. An independent real `ptrace(PTRACE_TRACEME)` process
under the same policy observed `-1/EPERM`, demonstrating enforced syscall
denial rather than a diagnostic-only property. This policy is native-only;
Linux microVM and Cloud Hypervisor unit properties are unchanged.

### Exit evidence and repeat procedure

The native kernel termination path emits one newline-terminated
`KILN_RESULT:{"schema":"kiln.unikernel.result.v1","exit_code":N}` frame after
process termination. Guest stdout/stderr share the bounded serial stream. A
missing, incomplete, malformed, truncated or duplicate status fails; VMM exit
zero alone never means test success. A guest that writes a counterfeit result
creates ambiguity with the native terminal frame and fails. Receipts record
`vm.backend: "firecracker"`, `guest_kind: "unikernel"`,
`unikernel: {project, version, image_blake3}`, and the native kernel/binary
identities; `hostile_isolation_claim` remains `false`.
Framing is parsed over bytes: non-UTF-8 test output outside the JSON frame is
permitted and displayed lossily, not mistaken for a failed status. Native capture
uses the remaining pipeline output budget; truncation or oversized serialized
evidence fails rather than succeeding after a saturating accounting update.
The original job deadline includes image preparation, VMM startup, API calls and
confinement observation; expiration prevents `InstanceStart` and never grants
the prepared image a fresh job timeout.

Build the actual native artifacts, then run the raw compatibility rail:

```console
nix build path:.#unikernel-guest -o result-unikernel-guest
nix build path:.#unikernel-rust-image -o result-unikernel-rust
nix build path:.#unikernel-go-image -o result-unikernel-go
nix build path:.#unikernel-probe-image -o result-unikernel-probe
python3 runtime/unikernel/smoke.py --guest result-unikernel-guest \
  --rust-image result-unikernel-rust --go-image result-unikernel-go \
  --probe-image result-unikernel-probe --output /tmp/kiln-unikernel-observations
# Rebuild native outputs independently, not merely retrieve the existing result:
nix build path:.#unikernel-guest path:.#unikernel-rust-image \
  path:.#unikernel-go-image path:.#unikernel-probe-image --rebuild --no-link
```

Use a fresh `--output` directory. The script verifies identities, rebuilds each
image twice from its declared binary, checks byte equality with the built image,
boots the real VMM, checks Rust/Go success and intentional exit 7, and checks the
real denial probe. The raw rail intentionally reports
`hosted_confinement_observed: false`; it is not the hosted pipeline's observer
proof. KVM access and the real reviewed Firecracker executable are required;
missing tools are failures, never mock/skip qualification.

The retained [single-host flake-built native compatibility receipt](../.cairn/archive/1970-01-01-add-unikernel-test-runner/evidence/permission-native/receipt.json)
and adjacent raw serial/config/VMM logs bind the actual Nix-built native kernel
(`b1debc88207360a26e54fae3e39a071b98a28af7131503ba71f1bd4600d4d4c5`),
Firecracker 1.16.1 (`4e4b972c0a8b73d7b598b178d5b40cb7b3989a0f692e82b8339ef58bad6e988c`)
and exact input images. On that Linux 6.18.50 x86_64 host, cold process spawn to
the selected application/probe marker was **129.400–134.884 ms** across six real
boots: Rust/Go pass/fail, read-only fixture consumption and network/process denial.
These are single-host observations, not an SLA, a cross-host benchmark or
qualification of a later rebuilt kernel. Rust/Go fixtures assert exact argc/argv;
native exit zero and intentional exit seven were observed. The canonical main
initrd BLAKE3 identities are Rust `0da44b6cc66d135b21248f975c87eadacc99964197961a2ff631b3bd6db6670a`,
Go `b444008c41ebdc427359c4065deb2c37bccfac2fde72b4bc5d73598f204579b8`
and probe `71416ad7b3ebcbb51c6190e24d93123125efbaf26617f7d825c17151a41c5e55`.
The image algorithm rebuilt each twice with matching bytes, and the integrated
Nix build plus `--rebuild` independently reproduced the kernel and all three
image outputs. These observations are distinct from the integrated
[hosted confinement receipt](../.cairn/archive/1970-01-01-add-unikernel-test-runner/evidence/hosted-observer.json),
which retains eight strict Seccomp 2 journeys. The paired fixture regression
records the failing-before writable reopen and passing-after refusal; actual
missing/changed preboot policies are separately refused before InstanceStart.

## Qualification and activation checklist

The [2026-10-28 local runtime observations](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/local-runtime.json)
retain actual server/bridge executable identities, scoped source identities,
browser role/deep-link inspection and direct Iroh requests. Real trusted
namespace jobs exercised success, failure and cancellation, excluded the
platform environment/home directory, and confirmed owned unit cleanup. Fresh
workspaces exercised a cache miss/publication, an exact 23-byte hit and a
changed-source miss. Existing timing rows migrated from invented zero durations
to explicitly unmeasured values. Cancellation preserved a stdout-worker teardown
timeout reason; confirmed cleanup, not that error alone, allowed the final
`cancelled` state. These measurements are one local cohort, not a latency SLA.

Kiln maintainers own the reusable cache-transfer helpers, hosted mechanisms and
the maintained `kiln-hosted` journey tests. Reproduce the local regression rail
with `nix develop path:. --command cargo test --locked --workspace`; enable the
maintained exact-source provider fixtures with `cargo test --locked -p
kiln-adapter-github --features protocol-fixtures --test local_ci_e2e` from the
same development shell. The provider rail uses isolated source/status fixtures,
not a new live GitHub run. The procedures above supply the typed inputs for a
separate operator-owned runtime cohort; temporary smoke keys and workspaces are
not deployment configuration.

The [2026-10-29 publication and desktop deployment receipt](../.cairn/archive/1970-01-01-add-iroh-hosted-ci/evidence/deployment-20261029.json)
records source `9e837966fe9927c5b27b72ebc0f2a1753d65de07` on canonical
Radicle and its GitHub projection, and activation of the rebuilt existing
desktop web service. TLS-verified health/assets, actual desktop/mobile browser
navigation, retained logs and unchanged state/credentials bound that deployment.
It did not activate the separate hosted listener, which has no installed,
separately admitted profile. Writable code review for the Kiln self-CI
repository was later admitted on explicit operator request; the receipt records
its exact principals, target and policy.
The hosted executable was built and published; commercial and hostile-isolation
qualification remain separate from this desktop deployment.

All nine areas above have bounded callable/local procedures, not a universal
production acceptance certificate. Keep observed native Iroh/browser,
remote-source run/status, stale/unauthorized denial, metering/restart,
isolation/cleanup, secret expiry/revocation and backup/restore receipts bound to
the exact source/tools/profile/host tested. Local checks, fixtures and planning
gates prove only their exercised boundaries. Missing prerequisites remain
explicit failures; do not replace a native worker, merchant, cloud trust,
credential resolver or isolator with a mock and label it qualified.

Before any real activation, obtain actual source credentials and exact route
admission, approved prices/caps and commercial/legal terms, independent metering
for native source attempts, provider capacity and reviewed fleet driver,
production firewall/relay/TLS and issuer routing, secret custody and customer
scope policy, independent security/hostile-isolation review where required,
complete dependency-aware backup/restore exercises and operational staffing.
No approval for live activation, publication, upstream mutation, purchases,
charges or production/hostile-isolation claims is supplied by this document.
