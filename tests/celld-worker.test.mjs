import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import worker, { KilnJournal } from "../src/index.mjs";

const encoder = new TextEncoder();
const secret = "\\".repeat(32);
const path = "/v1/append";
const stream = { provider_namespace: "kiln", native_kind: "stream", native_id: "builds" };
const operation = { provider_namespace: "kiln", native_kind: "operation", native_id: "op-1" };
const key = { provider_namespace: "celld", native_kind: "hmac-key", native_id: "kiln-primary" };
const streamKey = "kiln:stream:builds";
const schemaZero = "99a279c7fb99170797ff51ed0841ed9bee7957c9fb82926d9d80c87399e210c8";
const schemaOne = "9a41ba000f167e6a189136cbda0b62aef8efee9e5533cc4f6cafa8e4923a055b";

function operationFor(id) {
  return { ...operation, native_id: id };
}

function signedMode(mode, nonce, id, route) {
  return envelope({ stream, ...mode }, nonce, operationFor(id), route);
}

function blake3(bytes) {
  return execFileSync("b3sum", ["--no-names"], { input: Buffer.from(bytes), encoding: "utf8" }).trim();
}

function record(revision) {
  const event = {
    RunStarted: {
      command_id: `kiln-command-${revision}`,
      run_id: `kiln-run-${revision}`,
      attempt_id: `kiln-attempt-${revision}`,
      provider_profile: "nix",
    },
  };
  return {
    schema_version: 1,
    stream,
    expected_revision: revision,
    event_identity: {
      provider_namespace: "kiln",
      native_kind: "event-blake3",
      native_id: blake3(encoder.encode(JSON.stringify(event))),
    },
    event,
  };
}

function request(revision, withOutbox = false) {
  return {
    stream,
    expected_revision: revision,
    records: [record(revision)],
    outbox: withOutbox ? [{
      effect_id: `kiln-effect-${revision}`,
      provider_profile: "nix",
      idempotency_key: {
        provider_namespace: "kiln", native_kind: "outbox", native_id: `outbox-${revision}`,
      },
      lease: "Unclaimed",
    }] : [],
  };
}

function identity(value) {
  return `${value.provider_namespace}:${value.native_kind}:${value.native_id}`;
}

function signature(envelope) {
  const version = Buffer.alloc(2);
  version.writeUInt16LE(envelope.version);
  const time = Buffer.alloc(8);
  time.writeBigUInt64LE(BigInt(envelope.timestamp));
  const fields = [
    Buffer.from(envelope.path), version,
    Buffer.from(envelope.key_reference.provider_namespace),
    Buffer.from(envelope.key_reference.native_kind),
    Buffer.from(envelope.key_reference.native_id),
    time, Buffer.from(envelope.nonce),
    Buffer.from(envelope.operation_id.provider_namespace),
    Buffer.from(envelope.operation_id.native_kind),
    Buffer.from(envelope.operation_id.native_id),
    Buffer.from(envelope.payload_identity), Buffer.from(envelope.body),
  ];
  const framed = [Buffer.from("kiln-celld-envelope-v1\0")];
  for (const field of fields) {
    const length = Buffer.alloc(8);
    length.writeBigUInt64LE(BigInt(field.length));
    framed.push(length, field);
  }
  return createHmac("sha256", secret).update(Buffer.concat(framed)).digest("hex");
}

function envelope(append, nonce, operationId = operation, route = path) {
  const body = [...encoder.encode(JSON.stringify(append))];
  return {
    version: 1,
    path: route,
    key_reference: key,
    timestamp: Math.floor(Date.now() / 1000),
    nonce,
    operation_id: operationId,
    payload_identity: blake3(body),
    body,
  };
}

class SqlStorage {
  constructor(database) {
    this.database = database;
    this.sql = {
      exec: (statement, ...bindings) => {
        const prepared = database.prepare(statement);
        const rows = statement.trimStart().toUpperCase().startsWith("SELECT")
          ? prepared.all(...bindings)
          : (prepared.run(...bindings), []);
        return { toArray: () => rows, one: () => {
          assert.equal(rows.length, 1);
          return rows[0];
        } };
      },
    };
  }

  transactionSync(action) {
    this.database.exec("BEGIN IMMEDIATE");
    try {
      const result = action(this);
      this.database.exec("COMMIT");
      return result;
    } catch (error) {
      this.database.exec("ROLLBACK");
      throw error;
    }
  }
}

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), "kiln-celld-worker-"));
  const file = join(directory, "journal.sqlite");
  let db = new DatabaseSync(file);
  let journal = new KilnJournal({ storage: new SqlStorage(db) });
  t.after(() => {
    db.close();
    rmSync(directory, { recursive: true, force: true });
  });
  return {
    get db() { return db; },
    get journal() { return journal; },
    reopen() {
      db.close();
      db = new DatabaseSync(file);
      journal = new KilnJournal({ storage: new SqlStorage(db) });
    },
    env: {
      KILN_PATH: path,
      KILN_READ_PATH: "/v1/read",
      KILN_RECONCILE_PATH: "/v1/reconcile",
      KILN_LEASE_PATH: "/v1/lease",
      KILN_MIGRATE_PATH: "/v1/migrate",
      KILN_SCHEMA_VERSION: "1",
      KILN_WORKER_PROTOCOL_VERSION: "1",
      KILN_CELLD_REVISION: "ae8fac053d79f971bfcb996054bb43eb2f9b05da",
      KILN_OUTPUT_GATE: "1",
      KILN_STORAGE_PROBE: "1",
      KILN_DEPLOYMENT_ID: "0123456789abcdef",
      KILN_MAX_RESPONSE_BYTES: "1048576",
      KILN_MAX_REQUEST_BYTES: "1048576",
      KILN_HMAC_SECRET: secret,
      KILN_JOURNAL: {
        idFromName(name) { return name; },
        get(name) {
          assert.equal(name, streamKey);
          return { fetch: (incoming) => journal.fetch(incoming) };
        },
      },
    },
  };
}

async function dispatch(env, signed, options = {}) {
  const url = options.url ?? `https://kiln.invalid${signed.path}`;
  const message = new Request(url, {
    method: "POST",
    headers: { "x-kiln-signature": options.signature ?? signature(signed) },
    body: JSON.stringify(signed),
  });
  const response = await worker.fetch(message, env);
  return { status: response.status, body: await response.json() };
}

// Real SQLite rows, not a copied in-memory Rust model.
// r[verify kiln.celld.transport.scenario.valid]
// r[verify kiln.celld.ambiguity.scenario.repeat]
test("signed Worker append persists event, outbox, revision and exact idempotency", async (t) => {
  const local = fixture(t);
  const append = request(0, true);
  assert.deepEqual(await dispatch(local.env, envelope(append, "nonce-1")), {
    status: 200, body: { status: "accepted", revision: 1 },
  });
  assert.deepEqual(local.journal.read(streamKey, 1), {
    records: append.records, outbox: append.outbox,
  });
  assert.equal(local.db.prepare("SELECT schema_version FROM journal_meta").get().schema_version, 1);
  assert.equal(local.db.prepare("SELECT protocol_version FROM journal_meta").get().protocol_version, 1);
  const schema = local.db.prepare("SELECT version, identity FROM journal_schema WHERE stream = ?").get(streamKey);
  assert.equal(schema.version, 1);
  assert.equal(schema.identity, schemaOne);
  local.reopen();
  assert.deepEqual(local.journal.read(streamKey, 1), {
    records: append.records, outbox: append.outbox,
  });
  assert.deepEqual(await dispatch(local.env, envelope(append, "nonce-2")), {
    status: 200, body: { status: "repeated", revision: 1 },
  });
  assert.deepEqual(await dispatch(local.env, envelope(request(1), "nonce-3")), {
    status: 409, body: { code: "celld_conflict" },
  });
  assert.deepEqual(local.journal.read(streamKey, 1), {
    records: append.records, outbox: append.outbox,
  });
});

test("pre-ledger schema-1 history cannot silently acquire identity through append", async (t) => {
  const local = fixture(t);
  const historical = record(0);
  local.db.prepare(
    "INSERT INTO journal_meta (stream, revision, schema_version, protocol_version) VALUES (?, 1, 1, 1)",
  ).run(streamKey);
  local.db.prepare(
    "INSERT INTO journal_event (stream, revision, identity, record) VALUES (?, 0, ?, ?)",
  ).run(streamKey, identity(historical.event_identity), JSON.stringify(historical));
  assert.deepEqual(await dispatch(local.env, envelope(
    request(1), "preledger-rejected", operationFor("preledger-rejected"),
  )), { status: 409, body: { code: "celld_schema_mismatch" } });
  local.reopen();
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_schema").get().count, 0);
  assert.equal(local.db.prepare("SELECT revision FROM journal_meta").get().revision, 1);
  assert.equal(local.db.prepare("SELECT record FROM journal_event").get().record,
    JSON.stringify(historical));
  assert.equal(local.db.prepare(
    "SELECT count(*) AS count FROM journal_nonce WHERE nonce = 'preledger-rejected'",
  ).get().count, 0);
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "read" }, "preledger-schema-read", "preledger-schema-read", "/v1/migrate",
  )), { status: 409, body: { code: "celld_schema_mismatch" } });
});

// r[verify kiln.celld.local_worker_evidence.scenario.rollback]
// r[verify kiln.celld.atomic_append.scenario.interrupt]
test("real SQLite abort after event insert rolls back linked outbox and operation", async (t) => {
  const local = fixture(t);
  local.db.exec("CREATE TRIGGER interrupt_outbox BEFORE INSERT ON journal_outbox BEGIN SELECT RAISE(ABORT, 'fixture write fault'); END");
  assert.deepEqual(await dispatch(local.env, envelope(request(0, true), "nonce-1")), {
    status: 503, body: { code: "celld_store_failure" },
  });
  local.reopen();
  assert.deepEqual(local.journal.read(streamKey, 1), { records: [], outbox: [] });
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_operation").get().count, 0);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_nonce").get().count, 0);
  local.db.exec("DROP TRIGGER interrupt_outbox");
  assert.deepEqual(await dispatch(local.env, envelope(request(0, true), "nonce-1")), {
    status: 200, body: { status: "accepted", revision: 1 },
  });
});

test("a nonce/operation boundary collision cannot reuse another signed envelope", async (t) => {
  const local = fixture(t);
  const first = envelope(request(0), "a", {
    provider_namespace: "bc", native_kind: "operation", native_id: "op-1",
  });
  const changed = { ...first, nonce: "ab", operation_id: {
    ...first.operation_id, provider_namespace: "c",
  } };
  assert.equal(first.nonce + first.operation_id.provider_namespace,
    changed.nonce + changed.operation_id.provider_namespace);
  assert.notEqual(signature(first), signature(changed));
  assert.deepEqual(await dispatch(local.env, changed, { signature: signature(first) }), {
    status: 401, body: { code: "celld_signature" },
  });
  assert.deepEqual(local.journal.read(streamKey, 1), { records: [], outbox: [] });
});

// r[verify kiln.celld.transport.scenario.rejected]
// r[verify kiln.celld.authority.scenario.operator_api]
// r[verify kiln.celld.conformance.scenario.wrong_boundary]
test("signature, nonce, stale revision and private route fail at correct boundaries", async (t) => {
  const local = fixture(t);
  const first = envelope(request(0), "nonce-1");
  const badSignature = await dispatch(local.env, first, { signature: "0".repeat(64) });
  assert.equal(badSignature.status, 401);
  assert.equal(badSignature.body.code, "celld_signature");
  assert.deepEqual(local.journal.read(streamKey, 1), { records: [], outbox: [] });
  assert.deepEqual(await dispatch(local.env, first), {
    status: 200, body: { status: "accepted", revision: 1 },
  });
  assert.deepEqual(await dispatch(local.env, first), {
    status: 409, body: { code: "celld_nonce_replay" },
  });
  assert.deepEqual(await dispatch(local.env, envelope(request(0), "nonce-2", {
    provider_namespace: "kiln", native_kind: "operation", native_id: "op-2",
  })), {
    status: 409, body: { code: "celld_stale_revision" },
  });
  const privateRoute = await dispatch(local.env, envelope(request(1), "nonce-3"), {
    url: "https://kiln.invalid/state",
  });
  assert.equal(privateRoute.status, 403);
  assert.equal(privateRoute.body.code, "celld_route");
  assert.equal(local.journal.read(streamKey, 2).records.length, 1);
});

test("declared batch and request bounds reject before SQLite mutation", async (t) => {
  const local = fixture(t);
  const oversizedBatch = request(0);
  oversizedBatch.records = Array.from({ length: 33 }, (_, index) => record(index));
  assert.deepEqual(await dispatch(local.env, envelope(oversizedBatch, "nonce-1")), {
    status: 400, body: { code: "celld_append_bound" },
  });
  const oversizedOutbox = request(0, true);
  oversizedOutbox.outbox = Array.from({ length: 65 }, (_, index) => ({
    ...oversizedOutbox.outbox[0], effect_id: `kiln-effect-${index}`,
  }));
  assert.deepEqual(await dispatch(local.env, envelope(oversizedOutbox, "nonce-2")), {
    status: 400, body: { code: "celld_append_bound" },
  });
  const huge = new Request(`https://kiln.invalid${path}`, {
    method: "POST",
    body: "x".repeat(1_048_577),
  });
  assert.equal((await worker.fetch(huge, local.env)).status, 413);
  assert.deepEqual(local.journal.read(streamKey, 1), { records: [], outbox: [] });
});

test("stored schema mismatch blocks new mutations despite a valid signature", async (t) => {
  const local = fixture(t);
  const append = request(0, true);
  assert.equal((await dispatch(local.env, envelope(append, "nonce-1"))).status, 200);
  local.db.exec("UPDATE journal_meta SET schema_version = 2");
  assert.deepEqual(await dispatch(local.env, envelope(request(1), "nonce-2", {
    provider_namespace: "kiln", native_kind: "operation", native_id: "op-2",
  })), {
    status: 409, body: { code: "celld_schema_mismatch" },
  });
  local.db.exec("UPDATE journal_meta SET schema_version = 1");
  local.db.exec("UPDATE journal_schema SET identity = '0'");
  assert.deepEqual(await dispatch(local.env, envelope(request(1), "nonce-3",
    operationFor("op-3"))), {
    status: 409, body: { code: "celld_schema_mismatch" },
  });
  assert.equal(local.db.prepare("SELECT revision FROM journal_meta").get().revision, 1);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_event").get().count, 1);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_outbox").get().count, 1);
});

test("signed read and exact operation lookup use durable SQL, not a fabricated snapshot", async (t) => {
  const local = fixture(t);
  const append = request(0, true);
  const read = { stream, max_records: 1 };
  assert.deepEqual(await dispatch(local.env, envelope(read, "lookup-1", operation, "/v1/read")), {
    status: 200, body: { status: "read", records: [], outbox: [] },
  });
  assert.equal((await dispatch(local.env, envelope(append, "nonce-1"))).status, 200);
  local.reopen();
  assert.deepEqual(await dispatch(local.env, envelope(read, "lookup-2", operation, "/v1/read")), {
    status: 200, body: { status: "read", records: append.records, outbox: append.outbox },
  });
  assert.deepEqual(await dispatch(local.env, envelope({ stream }, "lookup-3", operation, "/v1/reconcile")), {
    status: 200, body: {
      status: "committed", revision: 1,
      payload_identity: blake3(encoder.encode(JSON.stringify(append))),
    },
  });
  const other = { ...operation, native_id: "unknown-operation" };
  assert.deepEqual(await dispatch(local.env, envelope({ stream }, "lookup-4", other, "/v1/reconcile")), {
    status: 200, body: { status: "absent" },
  });
  local.db.exec("UPDATE journal_event SET identity = 'kiln:event-blake3:wrong'");
  assert.deepEqual(await dispatch(local.env, envelope(read, "lookup-5", operation, "/v1/read")), {
    status: 503, body: { code: "celld_store_damage" },
  });
});

test("bounded replay reads ordered committed SQLite rows", async (t) => {
  const local = fixture(t);
  const first = request(0);
  const second = request(1);
  assert.equal((await dispatch(local.env, envelope(first, "nonce-1"))).status, 200);
  assert.equal((await dispatch(local.env, envelope(second, "nonce-2", {
    provider_namespace: "kiln", native_kind: "operation", native_id: "op-2",
  }))).status, 200);
  assert.throws(() => local.journal.read(streamKey, 1), { code: "celld_replay_bound" });
  assert.deepEqual(local.journal.read(streamKey, 2).records, [first.records[0], second.records[0]]);
});

test("signed lease claim retains planned facts and evidence across SQLite reopen", async (t) => {
  const local = fixture(t);
  const append = request(0, true);
  assert.equal((await dispatch(local.env, envelope(append, "lease-append"))).status, 200);
  const route = "/v1/lease";
  const effect_id = append.outbox[0].effect_id;
  const initial = {
    lease: { effect_id, provider_profile: "nix", state: "Unclaimed",
      owner: null, lease_until: null, attempts: 0 },
    idempotency_key: append.outbox[0].idempotency_key,
    completion: null, reconciliation: null,
  };
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "read", effect_id }, "lease-read-1", "lease-read-1", route,
  )), { status: 200, body: { status: "lease", lease: initial } });
  // The shell/core provides the planned claim; the Worker only conditionally applies it.
  const claimed = {
    ...initial,
    lease: {
      ...initial.lease, state: "Leased", attempts: 1,
      lease_until: 120, owner: { provider_namespace: "kiln", native_kind: "worker", native_id: "a" },
    },
  };
  const claim = signedMode({ mode: "apply", effect_id, expected: initial, next: claimed },
    "lease-claim-1", "lease-claim-1", route);
  assert.deepEqual(await dispatch(local.env, claim),
    { status: 200, body: { status: "updated", lease: claimed } });
  local.reopen();
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "read", effect_id }, "lease-read-2", "lease-read-2", route,
  )), { status: 200, body: { status: "lease", lease: claimed } });
  assert.equal(local.journal.read(streamKey, 1).outbox[0].lease, "Leased");
  const staleNext = {
    ...claimed,
    lease: { ...claimed.lease, owner: {
      provider_namespace: "kiln", native_kind: "worker", native_id: "other",
    }, lease_until: 140 },
  };
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "apply", effect_id, expected: { ...claimed, unexpected: "ignored" },
      next: staleNext },
    "lease-malformed", "lease-malformed", route,
  )), { status: 400, body: { code: "celld_lease_bound" } });
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "apply", effect_id, expected: initial, next: staleNext },
    "lease-stale", "lease-stale", route,
  )), { status: 409, body: { code: "celld_conflict" } });
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_mutation").get().count, 1);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_nonce WHERE nonce = 'lease-stale'").get().count, 0);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_nonce WHERE nonce = 'lease-malformed'").get().count, 0);
  assert.deepEqual((await dispatch(local.env, signedMode(
    { mode: "read", effect_id }, "lease-after-stale", "lease-after-stale", route,
  ))).body.lease, claimed);
  const reconciled = {
    ...claimed, lease: { ...claimed.lease, state: "Completed" },
    completion: { provider_namespace: "kiln", native_kind: "completion", native_id: "done" },
    reconciliation: { provider_namespace: "kiln", native_kind: "reconciliation", native_id: "observed" },
  };
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "apply", effect_id, expected: claimed, next: reconciled },
    "lease-completed", "lease-completed", route,
  )), { status: 200, body: { status: "updated", lease: reconciled } });
  local.reopen();
  assert.deepEqual((await dispatch(local.env, signedMode(
    { mode: "read", effect_id }, "lease-read-3", "lease-read-3", route,
  ))).body.lease, reconciled);
  assert.equal(local.journal.read(streamKey, 1).outbox[0].lease, "Completed");
  assert.deepEqual(await dispatch(local.env, signedMode(
    { stream }, "lease-reconcile", "lease-completed", "/v1/reconcile",
  )), {
    status: 200,
    body: { status: "committed", revision: 1,
      payload_identity: blake3(encoder.encode(JSON.stringify({
        stream, mode: "apply", effect_id, expected: claimed, next: reconciled,
      }))) },
  });
});

// r[verify kiln.celld.migrations.scenario.valid]
test("signed 0-to-1 empty schema migration retains exact identity and ledger", async (t) => {
  const local = fixture(t);
  const route = "/v1/migrate";
  const readSchema = (nonce) => dispatch(local.env,
    signedMode({ mode: "read" }, nonce, nonce, route));
  assert.deepEqual(await readSchema("schema-absent"),
    { status: 404, body: { code: "celld_schema_absent" } });
  const init = signedMode({ mode: "initialize" }, "schema-init", "schema-init", route);
  assert.deepEqual(await dispatch(local.env, init), {
    status: 200, body: { status: "initialized", version: 0, identity: schemaZero, revision: 0 },
  });
  assert.deepEqual(await readSchema("schema-zero"), {
    status: 200, body: { status: "schema", version: 0, identity: schemaZero, revision: 0 },
  });
  const migration = signedMode({
    mode: "apply", from_version: 0, to_version: 1,
    input_identity: schemaZero, output_identity: schemaOne,
  }, "schema-migrate", "schema-migrate", route);
  assert.deepEqual(await dispatch(local.env, migration), {
    status: 200, body: { status: "migrated", version: 1, identity: schemaOne, revision: 0 },
  });
  local.reopen();
  assert.deepEqual(await readSchema("schema-one"), {
    status: 200, body: { status: "schema", version: 1, identity: schemaOne, revision: 0 },
  });
  assert.deepEqual({ ...local.db.prepare(
    "SELECT from_version, to_version, input_identity, output_identity FROM journal_migration",
  ).get() }, { from_version: 0, to_version: 1,
    input_identity: schemaZero, output_identity: schemaOne });
  assert.deepEqual(await dispatch(local.env, signedMode(
    { stream }, "schema-lookup", "schema-migrate", "/v1/reconcile",
  )), { status: 200, body: { status: "committed", revision: 0,
    payload_identity: migration.payload_identity } });
  assert.deepEqual(await dispatch(local.env, {
    ...migration, nonce: "schema-migrate-again", timestamp: Math.floor(Date.now() / 1000),
  }), { status: 200, body: { status: "migrated", version: 1, identity: schemaOne, revision: 0 } });
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_migration").get().count, 1);
  assert.equal((await dispatch(local.env, envelope(request(0), "schema-append",
    operationFor("schema-append")))).status, 200);
});

test("migration identity, edge and injected ledger failure cannot partially change schema", async (t) => {
  const local = fixture(t);
  const route = "/v1/migrate";
  assert.equal((await dispatch(local.env, signedMode(
    { mode: "initialize" }, "init", "init", route,
  ))).status, 200);
  const apply = { mode: "apply", from_version: 0, to_version: 1,
    input_identity: schemaZero, output_identity: schemaOne };
  for (const [id, patch, code] of [
    ["bad-input", { input_identity: "0".repeat(64) }, "celld_migration_edge"],
    ["bad-output", { output_identity: "0".repeat(64) }, "celld_migration_edge"],
    ["unsupported", { to_version: 2 }, "celld_migration_edge"],
  ]) {
    assert.deepEqual(await dispatch(local.env, signedMode(
      { ...apply, ...patch }, id, id, route,
    )), { status: 409, body: { code } });
  }
  local.db.exec("CREATE TRIGGER reject_ledger BEFORE INSERT ON journal_migration BEGIN SELECT RAISE(ABORT, 'fixture ledger fault'); END");
  assert.deepEqual(await dispatch(local.env, signedMode(
    apply, "fault", "fault", route,
  )), { status: 503, body: { code: "celld_store_failure" } });
  local.reopen();
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "read" }, "after-fault", "after-fault", route,
  )), { status: 200, body: { status: "schema", version: 0,
    identity: schemaZero, revision: 0 } });
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_migration").get().count, 0);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_nonce WHERE nonce = 'fault'").get().count, 0);
  local.db.exec("DROP TRIGGER reject_ledger");
  assert.equal((await dispatch(local.env, signedMode(apply, "fault", "fault", route))).status, 200);
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "initialize" }, "init-again", "init-again", route,
  )), { status: 409, body: { code: "celld_migration_conflict" } });
});

test("signed requests reject incompatible, missing or disabled declared cohorts before object resolution", async (t) => {
  const local = fixture(t);
  const original = local.env.KILN_JOURNAL;
  local.env.KILN_JOURNAL = { idFromName() { throw new Error("object must not resolve"); } };
  const append = envelope(request(0, true), "cohort-negative");
  for (const [setting, value, code] of [
    ["KILN_SCHEMA_VERSION", "2", "celld_schema_mismatch"],
    ["KILN_WORKER_PROTOCOL_VERSION", "2", "celld_schema_mismatch"],
    ["KILN_CELLD_REVISION", "wrong", "celld_schema_mismatch"],
    ["KILN_OUTPUT_GATE", "0", "celld_output_gate_disabled"],
    ["KILN_STORAGE_PROBE", "0", "celld_storage_probe_failed"],
    ["KILN_OUTPUT_GATE", undefined, "celld_output_gate_disabled"],
    ["KILN_STORAGE_PROBE", undefined, "celld_storage_probe_failed"],
    ["KILN_DEPLOYMENT_ID", undefined, "celld_cohort_deployment"],
    ["KILN_DEPLOYMENT_ID", "different", "celld_cohort_deployment"],
  ]) {
    const env = { ...local.env };
    if (value === undefined) delete env[setting];
    else env[setting] = value;
    assert.deepEqual(await dispatch(env, append), { status: 409, body: { code } });
  }
  local.env.KILN_JOURNAL = original;
  assert.deepEqual(local.journal.read(streamKey, 1), { records: [], outbox: [] });
});

test("authenticated Worker response binds the observed deployment, revision and schema", async (t) => {
  const local = fixture(t);
  const signed = envelope(request(0), "cohort-header");
  const response = await worker.fetch(new Request(`https://kiln.invalid${signed.path}`, {
    method: "POST",
    headers: { "x-kiln-signature": signature(signed) },
    body: JSON.stringify(signed),
  }), local.env);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("x-kiln-worker-cohort"),
    "ae8fac053d79f971bfcb996054bb43eb2f9b05da:1:1:0123456789abcdef");
  assert.equal((await response.json()).revision, 1);
});

test("lease ledger write failure rolls back conditional lease and nonce together", async (t) => {
  const local = fixture(t);
  const append = request(0, true);
  assert.equal((await dispatch(local.env, envelope(append, "lease-fault-append"))).status, 200);
  const effect_id = append.outbox[0].effect_id;
  const initial = {
    lease: { effect_id, provider_profile: "nix", state: "Unclaimed",
      owner: null, lease_until: null, attempts: 0 },
    idempotency_key: append.outbox[0].idempotency_key,
    completion: null, reconciliation: null,
  };
  const next = { ...initial, lease: { ...initial.lease,
    state: "Leased", attempts: 1, lease_until: 102,
    owner: { provider_namespace: "kiln", native_kind: "worker", native_id: "claimant" },
  } };
  const claim = signedMode(
    { mode: "apply", effect_id, expected: initial, next },
    "lease-fault-claim", "lease-fault-claim", "/v1/lease",
  );
  local.db.exec("CREATE TRIGGER reject_lease_ledger BEFORE INSERT ON journal_mutation BEGIN SELECT RAISE(ABORT, 'fixture lease ledger fault'); END");
  assert.deepEqual(await dispatch(local.env, claim),
    { status: 503, body: { code: "celld_store_failure" } });
  local.reopen();
  assert.deepEqual((await dispatch(local.env, signedMode(
    { mode: "read", effect_id }, "lease-fault-read", "lease-fault-read", "/v1/lease",
  ))).body.lease, initial);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_mutation").get().count, 0);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_nonce WHERE nonce = 'lease-fault-claim'").get().count, 0);
  local.db.exec("DROP TRIGGER reject_lease_ledger");
  assert.deepEqual(await dispatch(local.env, claim),
    { status: 200, body: { status: "updated", lease: next } });
});

test("invalid signatures and nonempty prior schema reject before admitted migration", async (t) => {
  const local = fixture(t);
  const migration = signedMode(
    { mode: "initialize" }, "unsigned-init", "unsigned-init", "/v1/migrate",
  );
  const lease = signedMode(
    { mode: "read", effect_id: "unknown" }, "unsigned-lease", "unsigned-lease", "/v1/lease",
  );
  for (const signed of [migration, lease]) {
    assert.deepEqual(await dispatch(local.env, signed, { signature: "0".repeat(64) }),
      { status: 401, body: { code: "celld_signature" } });
  }
  assert.equal((await dispatch(local.env, migration)).status, 200);
  local.db.prepare("INSERT INTO journal_event (stream, revision, identity, record) VALUES (?, ?, ?, ?)").run(
    streamKey, 0, "kiln:event-blake3:dirty", JSON.stringify(record(0)),
  );
  assert.deepEqual(await dispatch(local.env, signedMode(
    { mode: "apply", from_version: 0, to_version: 1,
      input_identity: schemaZero, output_identity: schemaOne },
    "dirty-migrate", "dirty-migrate", "/v1/migrate",
  )), { status: 409, body: { code: "celld_migration_conflict" } });
  assert.equal(local.db.prepare("SELECT schema_version FROM journal_meta").get().schema_version, 0);
  assert.equal(local.db.prepare("SELECT count(*) AS count FROM journal_migration").get().count, 0);
});
