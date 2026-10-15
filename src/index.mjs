// Kiln's Worker uses the SQLite API supplied by the pinned celld Durable Object
// runtime. This module contains no fleet, operator, or deployment credentials.
const text = new TextEncoder();
const MAX_REQUEST_BYTES = 1_048_576;
const MAX_REPLAY_RECORDS = 1_024;
const MAX_EVENTS_PER_APPEND = 32;
const MAX_OUTBOX_RECORDS = 64;
const MAX_CLOCK_SKEW_SECONDS = 300;
const CELLD_REVISION = "ae8fac053d79f971bfcb996054bb43eb2f9b05da";
const SCHEMA_ZERO_IDENTITY = "99a279c7fb99170797ff51ed0841ed9bee7957c9fb82926d9d80c87399e210c8";
const SCHEMA_ONE_IDENTITY = "9a41ba000f167e6a189136cbda0b62aef8efee9e5533cc4f6cafa8e4923a055b";
const LEASE_STATES = new Set(["Unclaimed", "Leased", "Expired", "Completed", "Ambiguous"]);

function storedLease(value) {
  const lease = value?.lease;
  if (!value || !lease || Object.keys(value).length !== 4 ||
      Object.keys(lease).length !== 6 || typeof lease.effect_id !== "string" ||
      !lease.effect_id || lease.effect_id.length > 512 ||
      typeof lease.provider_profile !== "string" || !lease.provider_profile ||
      lease.provider_profile.length > 512 || !LEASE_STATES.has(lease.state) ||
      (lease.owner !== null && (lease.owner === undefined || !identity(lease.owner))) ||
      (lease.lease_until !== null &&
       (lease.lease_until === undefined || !admittedInteger(lease.lease_until, Number.MAX_SAFE_INTEGER))) ||
      !admittedInteger(lease.attempts, 0xffff_ffff) ||
      (value.completion !== null && (value.completion === undefined || !identity(value.completion))) ||
      (value.reconciliation !== null &&
       (value.reconciliation === undefined || !identity(value.reconciliation)))) {
    throw new WorkerError("celld_lease_bound", 400);
  }
  identity(value.idempotency_key);
  return value;
}

function sameRef(left, right) {
  return left.provider_namespace === right.provider_namespace &&
    left.native_kind === right.native_kind &&
    left.native_id === right.native_id &&
    left.contract_version === right.contract_version;
}

function sameLease(left, right) {
  return left.lease.effect_id === right.lease.effect_id &&
    left.lease.provider_profile === right.lease.provider_profile &&
    left.lease.state === right.lease.state &&
    left.lease.attempts === right.lease.attempts &&
    left.lease.lease_until === right.lease.lease_until &&
    (left.lease.owner === null ? right.lease.owner === null :
      right.lease.owner !== null && sameRef(left.lease.owner, right.lease.owner)) &&
    sameRef(left.idempotency_key, right.idempotency_key) &&
    (left.completion === null ? right.completion === null :
      right.completion !== null && sameRef(left.completion, right.completion)) &&
    (left.reconciliation === null ? right.reconciliation === null :
      right.reconciliation !== null &&
      sameRef(left.reconciliation, right.reconciliation));
}

function admittedInteger(value, upper) {
  return Number.isSafeInteger(value) && value >= 0 && value <= upper;
}

function identity(value) {
  if (!value || typeof value !== "object" ||
      typeof value.provider_namespace !== "string" ||
      typeof value.native_kind !== "string" ||
      typeof value.native_id !== "string" ||
      !value.provider_namespace || !value.native_kind || !value.native_id) {
    throw new WorkerError("celld_identity", 400);
  }
  return `${value.provider_namespace}:${value.native_kind}:${value.native_id}`;
}

function byteArray(value, maxBytes) {
  if (!Array.isArray(value) || value.length > maxBytes ||
      value.some((byte) => !admittedInteger(byte, 255))) {
    throw new WorkerError("celld_envelope_bound", 400);
  }
  return Uint8Array.from(value);
}

function signedBytes(envelope, body) {
  identity(envelope.key_reference);
  identity(envelope.operation_id);
  const version = new Uint8Array(2);
  new DataView(version.buffer).setUint16(0, envelope.version, true);
  const timestamp = new Uint8Array(8);
  new DataView(timestamp.buffer).setBigUint64(0, BigInt(envelope.timestamp), true);
  const fields = [
    text.encode(envelope.path), version,
    text.encode(envelope.key_reference.provider_namespace),
    text.encode(envelope.key_reference.native_kind),
    text.encode(envelope.key_reference.native_id),
    timestamp, text.encode(envelope.nonce),
    text.encode(envelope.operation_id.provider_namespace),
    text.encode(envelope.operation_id.native_kind),
    text.encode(envelope.operation_id.native_id),
    text.encode(envelope.payload_identity), body,
  ];
  const domain = text.encode("kiln-celld-envelope-v1\0");
  const bytes = new Uint8Array(domain.length +
    fields.reduce((size, part) => size + 8 + part.length, 0));
  bytes.set(domain);
  const frame = new DataView(bytes.buffer);
  let offset = domain.length;
  for (const part of fields) {
    frame.setBigUint64(offset, BigInt(part.length), true);
    offset += 8;
    bytes.set(part, offset);
    offset += part.length;
  }
  return bytes;
}

class WorkerError extends Error {
  constructor(code, status) {
    super(code);
    this.code = code;
    this.status = status;
  }
}

function reply(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

async function authenticate(request, env) {
  const destination = new URL(request.url);
  const path = destination.pathname;
  const action = path === env.KILN_PATH ? "append" :
    path === env.KILN_READ_PATH ? "read" :
    path === env.KILN_RECONCILE_PATH ? "reconcile" :
    path === env.KILN_LEASE_PATH ? "lease" :
    path === env.KILN_MIGRATE_PATH ? "migrate" : null;
  if (request.method !== "POST" || destination.protocol !== "https:" || !action ||
      path === "/state" || path === "/shutdown" ||
      path.startsWith("/do/") || path.startsWith("/evict/")) {
    throw new WorkerError("celld_route", 403);
  }
  const maxBytes = Number(env.KILN_MAX_REQUEST_BYTES ?? MAX_REQUEST_BYTES);
  if (!admittedInteger(maxBytes, MAX_REQUEST_BYTES) || maxBytes === 0) {
    throw new WorkerError("celld_request_bound", 400);
  }
  const declared = request.headers.get("content-length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > maxBytes)) {
    throw new WorkerError("celld_request_bound", 413);
  }
  const chunks = [];
  let received = 0;
  if (request.body) {
    const reader = request.body.getReader();
    try {
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        received += value.byteLength;
        if (received > maxBytes) {
          await reader.cancel();
          throw new WorkerError("celld_request_bound", 413);
        }
        chunks.push(value);
      }
    } finally {
      reader.releaseLock();
    }
  }
  const wire = new Uint8Array(received);
  let position = 0;
  for (const chunk of chunks) {
    wire.set(chunk, position);
    position += chunk.byteLength;
  }
  let envelope;
  try {
    envelope = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(wire));
  } catch {
    throw new WorkerError("celld_payload_decode", 400);
  }
  if (!envelope || envelope.version !== 1 || envelope.path !== path ||
      !admittedInteger(envelope.timestamp, Number.MAX_SAFE_INTEGER) ||
      Math.abs(Math.floor(Date.now() / 1000) - envelope.timestamp) > MAX_CLOCK_SKEW_SECONDS ||
      typeof envelope.nonce !== "string" || !envelope.nonce ||
      typeof envelope.payload_identity !== "string" ||
      !/^[0-9a-f]{64}$/.test(envelope.payload_identity)) {
    throw new WorkerError("celld_envelope", 400);
  }
  const body = byteArray(envelope.body, maxBytes);
  const secret = env.KILN_HMAC_SECRET;
  if (typeof secret !== "string" || !secret) {
    throw new WorkerError("celld_key_unavailable", 503);
  }
  const key = await crypto.subtle.importKey(
    "raw", text.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"],
  );
  const actual = request.headers.get("x-kiln-signature") ?? "";
  const expected = new Uint8Array(await crypto.subtle.sign("HMAC", key, signedBytes(envelope, body)));
  if (!/^[0-9a-f]{64}$/.test(actual)) throw new WorkerError("celld_signature", 401);
  let difference = 0;
  for (let index = 0; index < expected.length; index++) {
    difference |= expected[index] ^ Number.parseInt(actual.slice(index * 2, index * 2 + 2), 16);
  }
  if (difference !== 0) throw new WorkerError("celld_signature", 401);
  let operation;
  try {
    operation = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body));
  } catch {
    throw new WorkerError("celld_payload_decode", 400);
  }
  const stream = identity(operation?.stream);
  if (action === "append" &&
      (!admittedInteger(operation.expected_revision, Number.MAX_SAFE_INTEGER) ||
       !Array.isArray(operation.records) || operation.records.length === 0 ||
       operation.records.length > MAX_EVENTS_PER_APPEND ||
       !Array.isArray(operation.outbox) || operation.outbox.length > MAX_OUTBOX_RECORDS)) {
    throw new WorkerError("celld_append_bound", 400);
  }
  if (action === "read" &&
      (!admittedInteger(operation.max_records, MAX_REPLAY_RECORDS) ||
       operation.max_records === 0)) {
    throw new WorkerError("celld_replay_bound", 400);
  }
  if (action === "lease" &&
      (!["read", "apply"].includes(operation.mode) ||
       typeof operation.effect_id !== "string" || !operation.effect_id ||
       operation.effect_id.length > 512)) {
    throw new WorkerError("celld_lease_bound", 400);
  }
  if (action === "lease" && operation.mode === "apply") {
    storedLease(operation.expected);
    storedLease(operation.next);
    if (operation.expected.lease.effect_id !== operation.effect_id ||
        operation.next.lease.effect_id !== operation.effect_id ||
        !sameRef(operation.expected.idempotency_key, operation.next.idempotency_key) ||
        operation.expected.lease.provider_profile !== operation.next.lease.provider_profile) {
      throw new WorkerError("celld_lease_bound", 400);
    }
  }
  if (action === "migrate" &&
      !["read", "initialize", "apply"].includes(operation.mode)) {
    throw new WorkerError("celld_migration_bound", 400);
  }
  if (action === "migrate" && operation.mode === "apply" &&
      (!admittedInteger(operation.from_version, 0xffff) ||
       !admittedInteger(operation.to_version, 0xffff) ||
       typeof operation.input_identity !== "string" ||
       typeof operation.output_identity !== "string")) {
    throw new WorkerError("celld_migration_bound", 400);
  }
  // A valid signature does not admit an incompatible deployed Worker cohort.
  if (env.KILN_SCHEMA_VERSION !== "1" ||
      env.KILN_WORKER_PROTOCOL_VERSION !== "1" ||
      env.KILN_CELLD_REVISION !== CELLD_REVISION) {
    throw new WorkerError("celld_schema_mismatch", 409);
  }
  if (env.KILN_OUTPUT_GATE !== "1") {
    throw new WorkerError("celld_output_gate_disabled", 409);
  }
  if (env.KILN_STORAGE_PROBE !== "1") {
    throw new WorkerError("celld_storage_probe_failed", 409);
  }
  if (!/^[0-9a-f]{16}$/.test(env.KILN_DEPLOYMENT_ID ?? "")) {
    throw new WorkerError("celld_cohort_deployment", 409);
  }
  return { envelope, operation, stream, action };
}

// r[impl kiln.celld.local_worker_evidence]
// The same Durable Object SQL is exercised locally and under the pinned celld runtime.
export class KilnJournal {
  constructor(state) {
    this.storage = state.storage;
    this.storage.transactionSync((tx) => {
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_meta (stream TEXT PRIMARY KEY, revision INTEGER NOT NULL, schema_version INTEGER NOT NULL, protocol_version INTEGER NOT NULL)");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_event (stream TEXT NOT NULL, revision INTEGER NOT NULL, identity TEXT NOT NULL UNIQUE, record TEXT NOT NULL, PRIMARY KEY(stream, revision))");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_outbox (stream TEXT NOT NULL, effect TEXT NOT NULL, record TEXT NOT NULL, PRIMARY KEY(stream, effect))");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_operation (stream TEXT NOT NULL, operation TEXT NOT NULL, payload TEXT NOT NULL, revision INTEGER NOT NULL, PRIMARY KEY(stream, operation))");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_nonce (key TEXT NOT NULL, nonce TEXT NOT NULL, PRIMARY KEY(key, nonce))");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_lease (stream TEXT NOT NULL, effect TEXT NOT NULL, facts TEXT NOT NULL, PRIMARY KEY(stream, effect))");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_schema (stream TEXT PRIMARY KEY, version INTEGER NOT NULL, identity TEXT NOT NULL)");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_migration (stream TEXT NOT NULL, operation TEXT NOT NULL, payload TEXT NOT NULL, from_version INTEGER NOT NULL, to_version INTEGER NOT NULL, input_identity TEXT NOT NULL, output_identity TEXT NOT NULL, PRIMARY KEY(stream, operation))");
      tx.sql.exec("CREATE TABLE IF NOT EXISTS journal_mutation (stream TEXT NOT NULL, operation TEXT NOT NULL, payload TEXT NOT NULL, revision INTEGER NOT NULL, result TEXT NOT NULL, PRIMARY KEY(stream, operation))");
    });
  }

  async fetch(request) {
    if (request.method !== "POST") return reply({ code: "celld_route" }, 403);
    let packet;
    try {
      packet = await request.json();
    } catch {
      return reply({ code: "celld_payload_decode" }, 400);
    }
    try {
      let result;
      if (packet.action === "append") {
        result = this.append({ envelope: packet.envelope, append: packet.operation, stream: packet.stream });
      } else if (packet.action === "reconcile") {
        result = this.reconcile(packet.stream, packet.envelope);
      } else if (packet.action === "read") {
        result = { status: "read", ...this.read(packet.stream, packet.operation.max_records) };
      } else if (packet.action === "lease") {
        result = this.lease(packet.stream, packet.envelope, packet.operation);
      } else if (packet.action === "migrate") {
        result = this.migrate(packet.stream, packet.envelope, packet.operation);
      } else {
        throw new WorkerError("celld_route", 403);
      }
      const encoded = JSON.stringify(result);
      if (text.encode(encoded).byteLength > MAX_REQUEST_BYTES) {
        throw new WorkerError("celld_response_bound", 413);
      }
      return new Response(encoded, {
        headers: { "content-type": "application/json" },
      });
    } catch (error) {
      if (error instanceof WorkerError) return reply({ code: error.code }, error.status);
      return reply({ code: "celld_store_failure" }, 503);
    }
  }

  append({ envelope, append, stream }) {
    const operation = identity(envelope.operation_id);
    const key = identity(envelope.key_reference);
    const prepared = append.records.map((record, offset) => {
      if (identity(record?.stream) !== stream ||
          record.expected_revision !== append.expected_revision + offset) {
        throw new WorkerError("celld_event_sequence", 400);
      }
      return [identity(record.event_identity), JSON.stringify(record)];
    });
    const outbox = append.outbox.map((record) => {
      if (typeof record?.effect_id !== "string" || !record.effect_id ||
          typeof record.provider_profile !== "string" || !record.provider_profile ||
          record.lease !== "Unclaimed") {
        throw new WorkerError("celld_outbox", 400);
      }
      identity(record.idempotency_key);
      const facts = {
        lease: {
          effect_id: record.effect_id, provider_profile: record.provider_profile,
          state: record.lease, owner: null, lease_until: null, attempts: 0,
        },
        idempotency_key: record.idempotency_key,
        completion: null, reconciliation: null,
      };
      storedLease(facts);
      return [JSON.stringify(record.effect_id), JSON.stringify(record), JSON.stringify(facts)];
    });
    return this.storage.transactionSync((tx) => {
      const sql = tx.sql;
      const nonce = sql.exec(
        "SELECT 1 AS found FROM journal_nonce WHERE key = ? AND nonce = ?", key, envelope.nonce,
      ).toArray();
      if (nonce.length) throw new WorkerError("celld_nonce_replay", 409);
      sql.exec("INSERT INTO journal_nonce (key, nonce) VALUES (?, ?)", key, envelope.nonce);
      // Only a virgin, empty object can acquire schema-1 identity here. Existing
      // pre-ledger schema-1 history needs an admitted migration, not silent repair.
      const existing = this.schema(stream, sql);
      if (existing && existing.version !== 1) {
        throw new WorkerError("celld_schema_mismatch", 409);
      }
      if (!existing) {
        if (["journal_event", "journal_outbox", "journal_lease",
          "journal_operation", "journal_mutation", "journal_migration"].some((table) =>
          sql.exec(`SELECT 1 AS found FROM ${table} WHERE stream = ? LIMIT 1`,
            stream).toArray().length)) {
          throw new WorkerError("celld_schema_mismatch", 409);
        }
        sql.exec(
          "INSERT INTO journal_meta (stream, revision, schema_version, protocol_version) VALUES (?, 0, 1, 1)",
          stream,
        );
        sql.exec("INSERT INTO journal_schema (stream, version, identity) VALUES (?, 1, ?)",
          stream, SCHEMA_ONE_IDENTITY);
      }
      const current = existing?.revision ?? 0;
      if (sql.exec(
        "SELECT 1 AS found FROM journal_mutation WHERE stream = ? AND operation = ?",
        stream, operation,
      ).toArray().length) {
        throw new WorkerError("celld_conflict", 409);
      }
      const prior = sql.exec(
        "SELECT payload, revision FROM journal_operation WHERE stream = ? AND operation = ?",
        stream, operation,
      ).toArray();
      if (prior.length) {
        if (prior[0].payload !== envelope.payload_identity) {
          throw new WorkerError("celld_conflict", 409);
        }
        return { status: "repeated", revision: prior[0].revision };
      }
      if (current !== append.expected_revision ||
          !admittedInteger(current + prepared.length, Number.MAX_SAFE_INTEGER)) {
        throw new WorkerError("celld_stale_revision", 409);
      }
      for (let offset = 0; offset < prepared.length; offset++) {
        const [eventIdentity, record] = prepared[offset];
        sql.exec(
          "INSERT INTO journal_event (stream, revision, identity, record) VALUES (?, ?, ?, ?)",
          stream, current + offset, eventIdentity, record,
        );
      }
      for (const [effect, record, facts] of outbox) {
        sql.exec(
          "INSERT INTO journal_outbox (stream, effect, record) VALUES (?, ?, ?)",
          stream, effect, record,
        );
        sql.exec(
          "INSERT INTO journal_lease (stream, effect, facts) VALUES (?, ?, ?)",
          stream, effect, facts,
        );
      }
      const revision = current + prepared.length;
      sql.exec("UPDATE journal_meta SET revision = ? WHERE stream = ?", revision, stream);
      sql.exec(
        "INSERT INTO journal_operation (stream, operation, payload, revision) VALUES (?, ?, ?, ?)",
        stream, operation, envelope.payload_identity, revision,
      );
      return { status: "accepted", revision };
    });
  }

  schema(stream, sql = this.storage.sql) {
    const meta = sql.exec(
      "SELECT revision, schema_version, protocol_version FROM journal_meta WHERE stream = ?", stream,
    ).toArray();
    const schema = sql.exec(
      "SELECT version, identity FROM journal_schema WHERE stream = ?", stream,
    ).toArray();
    if (!meta.length && !schema.length) return null;
    if (meta.length !== 1 || schema.length !== 1 ||
        meta[0].protocol_version !== 1 ||
        meta[0].schema_version !== schema[0].version ||
        !((schema[0].version === 0 && schema[0].identity === SCHEMA_ZERO_IDENTITY) ||
          (schema[0].version === 1 && schema[0].identity === SCHEMA_ONE_IDENTITY))) {
      throw new WorkerError("celld_schema_mismatch", 409);
    }
    return { status: "schema", version: schema[0].version,
      identity: schema[0].identity, revision: meta[0].revision };
  }

  mutation(sql, stream, envelope) {
    const key = identity(envelope.key_reference);
    if (sql.exec("SELECT 1 AS found FROM journal_nonce WHERE key = ? AND nonce = ?",
      key, envelope.nonce).toArray().length) {
      throw new WorkerError("celld_nonce_replay", 409);
    }
    const operation = identity(envelope.operation_id);
    if (sql.exec("SELECT 1 AS found FROM journal_operation WHERE stream = ? AND operation = ?",
      stream, operation).toArray().length) {
      throw new WorkerError("celld_conflict", 409);
    }
    const prior = sql.exec(
      "SELECT payload, result FROM journal_mutation WHERE stream = ? AND operation = ?",
      stream, operation,
    ).toArray();
    if (prior.length && prior[0].payload !== envelope.payload_identity) {
      throw new WorkerError("celld_conflict", 409);
    }
    sql.exec("INSERT INTO journal_nonce (key, nonce) VALUES (?, ?)", key, envelope.nonce);
    return prior.length ? JSON.parse(prior[0].result) : null;
  }

  remember(sql, stream, envelope, revision, result) {
    sql.exec(
      "INSERT INTO journal_mutation (stream, operation, payload, revision, result) VALUES (?, ?, ?, ?, ?)",
      stream, identity(envelope.operation_id), envelope.payload_identity,
      revision, JSON.stringify(result),
    );
  }

  lease(stream, envelope, request) {
    const effect = JSON.stringify(request.effect_id);
    if (request.mode === "read") {
      const schema = this.schema(stream);
      if (schema && schema.version !== 1) throw new WorkerError("celld_schema_mismatch", 409);
      const rows = this.storage.sql.exec(
        "SELECT facts FROM journal_lease WHERE stream = ? AND effect = ?", stream, effect,
      ).toArray();
      if (!rows.length) throw new WorkerError("celld_lease_absent", 404);
      const lease = storedLease(JSON.parse(rows[0].facts));
      if (lease.lease.effect_id !== request.effect_id) {
        throw new WorkerError("celld_store_damage", 503);
      }
      return { status: "lease", lease };
    }
    return this.storage.transactionSync((tx) => {
      const sql = tx.sql;
      const prior = this.mutation(sql, stream, envelope);
      if (prior) return prior;
      const schema = this.schema(stream, sql);
      if (!schema || schema.version !== 1) {
        throw new WorkerError("celld_schema_mismatch", 409);
      }
      const rows = sql.exec(
        "SELECT facts FROM journal_lease WHERE stream = ? AND effect = ?", stream, effect,
      ).toArray();
      if (!rows.length) throw new WorkerError("celld_lease_absent", 404);
      if (!sameLease(storedLease(JSON.parse(rows[0].facts)), request.expected)) {
        throw new WorkerError("celld_conflict", 409);
      }
      sql.exec("UPDATE journal_lease SET facts = ? WHERE stream = ? AND effect = ? AND facts = ?",
        JSON.stringify(request.next), stream, effect, rows[0].facts);
      const postflight = sql.exec(
        "SELECT facts FROM journal_lease WHERE stream = ? AND effect = ?", stream, effect,
      ).one();
      if (!sameLease(storedLease(JSON.parse(postflight.facts)), request.next)) {
        throw new WorkerError("celld_conflict", 409);
      }
      const result = { status: "updated", lease: request.next };
      this.remember(sql, stream, envelope, schema.revision, result);
      return result;
    });
  }

  migrate(stream, envelope, request) {
    if (request.mode === "read") {
      const schema = this.schema(stream);
      if (!schema) throw new WorkerError("celld_schema_absent", 404);
      return schema;
    }
    return this.storage.transactionSync((tx) => {
      const sql = tx.sql;
      const prior = this.mutation(sql, stream, envelope);
      if (prior) return prior;
      const schema = this.schema(stream, sql);
      if (request.mode === "initialize") {
        if (schema || ["journal_event", "journal_outbox", "journal_lease",
          "journal_operation", "journal_mutation", "journal_migration"].some((table) =>
          sql.exec(`SELECT 1 AS found FROM ${table} WHERE stream = ? LIMIT 1`,
            stream).toArray().length)) {
          throw new WorkerError("celld_migration_conflict", 409);
        }
        sql.exec(
          "INSERT INTO journal_meta (stream, revision, schema_version, protocol_version) VALUES (?, 0, 0, 1)", stream,
        );
        sql.exec(
          "INSERT INTO journal_schema (stream, version, identity) VALUES (?, 0, ?)",
          stream, SCHEMA_ZERO_IDENTITY,
        );
        const result = { status: "initialized", version: 0,
          identity: SCHEMA_ZERO_IDENTITY, revision: 0 };
        this.remember(sql, stream, envelope, 0, result);
        return result;
      }
      if (request.from_version !== 0 || request.to_version !== 1 ||
          request.input_identity !== SCHEMA_ZERO_IDENTITY ||
          request.output_identity !== SCHEMA_ONE_IDENTITY) {
        throw new WorkerError("celld_migration_edge", 409);
      }
      if (!schema || schema.version !== 0 || schema.identity !== request.input_identity ||
          schema.revision !== 0 ||
          ["journal_event", "journal_outbox", "journal_lease"].some((table) =>
            sql.exec(`SELECT 1 AS found FROM ${table} WHERE stream = ? LIMIT 1`,
              stream).toArray().length)) {
        throw new WorkerError("celld_migration_conflict", 409);
      }
      sql.exec("UPDATE journal_meta SET schema_version = 1 WHERE stream = ?", stream);
      sql.exec("UPDATE journal_schema SET version = 1, identity = ? WHERE stream = ?",
        SCHEMA_ONE_IDENTITY, stream);
      sql.exec(
        "INSERT INTO journal_migration (stream, operation, payload, from_version, to_version, input_identity, output_identity) VALUES (?, ?, ?, 0, 1, ?, ?)",
        stream, identity(envelope.operation_id), envelope.payload_identity,
        SCHEMA_ZERO_IDENTITY, SCHEMA_ONE_IDENTITY,
      );
      const result = { status: "migrated", version: 1,
        identity: SCHEMA_ONE_IDENTITY, revision: 0 };
      this.remember(sql, stream, envelope, 0, result);
      if (this.schema(stream, sql).identity !== SCHEMA_ONE_IDENTITY) {
        throw new WorkerError("celld_schema_mismatch", 409);
      }
      return result;
    });
  }

  reconcile(stream, envelope) {
    const operation = identity(envelope.operation_id);
    const prior = this.storage.sql.exec(
      "SELECT payload, revision FROM journal_operation WHERE stream = ? AND operation = ?",
      stream, operation,
    ).toArray();
    const mutation = this.storage.sql.exec(
      "SELECT payload, revision FROM journal_mutation WHERE stream = ? AND operation = ?",
      stream, operation,
    ).toArray();
    if (!prior.length && !mutation.length) return { status: "absent" };
    if (prior.length && mutation.length) throw new WorkerError("celld_store_damage", 503);
    const recorded = prior[0] ?? mutation[0];
    return {
      status: "committed", revision: recorded.revision, payload_identity: recorded.payload,
    };
  }

  // A signed read returns only contiguous, identity-matched committed rows.
  read(stream, maxRecords) {
    if (!admittedInteger(maxRecords, MAX_REPLAY_RECORDS) || maxRecords === 0) {
      throw new WorkerError("celld_replay_bound", 400);
    }
    const meta = this.storage.sql.exec(
      "SELECT revision, schema_version, protocol_version FROM journal_meta WHERE stream = ?", stream,
    ).toArray();
    if (meta.length && this.schema(stream).version !== 1) {
      throw new WorkerError("celld_schema_mismatch", 409);
    }
    const rows = this.storage.sql.exec(
      "SELECT revision, identity, record FROM journal_event WHERE stream = ? ORDER BY revision LIMIT ?",
      stream, maxRecords + 1,
    ).toArray();
    if (rows.length > maxRecords) throw new WorkerError("celld_replay_bound", 400);
    const records = rows.map((row, index) => {
      const record = JSON.parse(row.record);
      if (row.revision !== index || record.expected_revision !== index ||
          identity(record.stream) !== stream ||
          identity(record.event_identity) !== row.identity) {
        throw new WorkerError("celld_store_damage", 503);
      }
      return record;
    });
    if ((meta[0]?.revision ?? 0) !== rows.length) {
      throw new WorkerError("celld_store_damage", 503);
    }
    const outbox = this.storage.sql.exec(
      "SELECT o.effect, o.record, l.facts FROM journal_outbox o LEFT JOIN journal_lease l ON l.stream = o.stream AND l.effect = o.effect WHERE o.stream = ? ORDER BY o.effect LIMIT ?",
      stream, MAX_OUTBOX_RECORDS + 1,
    ).toArray();
    if (outbox.length > MAX_OUTBOX_RECORDS) {
      throw new WorkerError("celld_replay_bound", 400);
    }
    return { records, outbox: outbox.map(({ effect, record, facts }) => {
      const original = JSON.parse(record);
      if (facts === null || facts === undefined) throw new WorkerError("celld_store_damage", 503);
      const current = storedLease(JSON.parse(facts));
      if (JSON.stringify(current.lease.effect_id) !== effect ||
          original.provider_profile !== current.lease.provider_profile ||
          !sameRef(original.idempotency_key, current.idempotency_key)) {
        throw new WorkerError("celld_store_damage", 503);
      }
      return { ...original, lease: current.lease.state };
    }) };
  }
}

export default {
  async fetch(request, env) {
    try {
      const packet = await authenticate(request, env);
      if (!env.KILN_JOURNAL || typeof env.KILN_JOURNAL.idFromName !== "function") {
        throw new WorkerError("celld_binding_unavailable", 503);
      }
      const id = env.KILN_JOURNAL.idFromName(packet.stream);
      const result = await env.KILN_JOURNAL.get(id).fetch(new Request(request.url, {
        method: "POST",
        body: JSON.stringify(packet),
      }));
      const headers = new Headers(result.headers);
      headers.set("x-kiln-worker-cohort", [
        env.KILN_CELLD_REVISION, env.KILN_WORKER_PROTOCOL_VERSION,
        env.KILN_SCHEMA_VERSION, env.KILN_DEPLOYMENT_ID,
      ].join(":"));
      return new Response(result.body, { status: result.status, headers });
    } catch (error) {
      if (error instanceof WorkerError) return reply({ code: error.code }, error.status);
      return reply({ code: "celld_store_failure" }, 503);
    }
  },
};
