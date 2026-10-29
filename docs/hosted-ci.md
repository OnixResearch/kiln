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

The uppercase placeholder intentionally does not satisfy the peer contract.
Replace it, do not weaken the contract. Export with private permissions:

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
The [2026-10-29 security regression and real transport exercise](../.cairn/changes/add-iroh-hosted-ci/evidence/replay-capacity-20261029.json)
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

## Qualification and activation checklist

The [2026-10-28 local runtime observations](../.cairn/changes/add-iroh-hosted-ci/evidence/local-runtime.json)
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

The [2026-10-29 publication and desktop deployment receipt](../.cairn/changes/add-iroh-hosted-ci/evidence/deployment-20261029.json)
records source `9e837966fe9927c5b27b72ebc0f2a1753d65de07` on canonical
Radicle and its GitHub projection, and activation of the rebuilt existing
desktop web service. TLS-verified health/assets, actual desktop/mobile browser
navigation, retained logs and unchanged state/credentials bound that deployment.
This did not activate the separate hosted listener or grant writable review
provider capabilities: neither has an installed, separately admitted profile.
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
