import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const window = {};
vm.runInNewContext(readFileSync(new URL("../crates/kiln-web-service/src/cairn.js", import.meta.url), "utf8"), { window });
const { admitAuthoringSnapshot, authoringRequest } = window.KilnCairn;
const repository = "a".repeat(64);
const revision = "b".repeat(64);
const plain = (value) => JSON.parse(JSON.stringify(value));

function nativeFixture() {
  return {
    schema: "kiln.cairn-authoring.v1", repository,
    snapshot: { revision, change: null, graph: {
      command: "change graph", layout: "cairn", valid: true, active_change_count: 3,
      nodes: [
        { name: "alpha", dependencies: ["beta"] },
        { name: "beta", dependencies: ["gamma"] },
        { name: "gamma", dependencies: [] },
      ],
      archived_changes: ["released"],
    } },
  };
}

test("Cairn authoring sends exactly the selected root identity, displayed revision and named action", () => {
  const snapshot = admitAuthoringSnapshot(nativeFixture(), repository);
  assert.deepEqual(plain(authoringRequest(repository, snapshot, "create", "write-docs")), {
    repository, expected_revision: revision, action: { type: "create", change: "write-docs" },
  });
  assert.deepEqual(plain(authoringRequest(repository, snapshot, "add_dependency", "gamma", "released")), {
    repository, expected_revision: revision, action: { type: "add_dependency", change: "gamma", dependency: "released" },
  });
  assert.deepEqual(plain(authoringRequest(repository, snapshot, "remove_dependency", "alpha", "beta")), {
    repository, expected_revision: revision, action: { type: "remove_dependency", change: "alpha", dependency: "beta" },
  });
});

test("Cairn controls refuse unbound readback, invalid names, cycles, no-ops and unsupported artifact writes", () => {
  const fixture = nativeFixture();
  assert.throws(() => admitAuthoringSnapshot(fixture, "c".repeat(64)), /Invalid Cairn authoring readback/);
  const snapshot = admitAuthoringSnapshot(fixture, repository);
  assert.equal(authoringRequest(repository, snapshot, "add_dependency", "gamma", "alpha"), null);
  assert.equal(authoringRequest(repository, snapshot, "add_dependency", "alpha", "alpha"), null);
  assert.equal(authoringRequest(repository, snapshot, "add_dependency", "alpha", "beta"), null);
  assert.equal(authoringRequest(repository, snapshot, "add_dependency", "alpha", "missing"), null);
  assert.equal(authoringRequest(repository, snapshot, "remove_dependency", "alpha", "gamma"), null);
  assert.equal(authoringRequest(repository, snapshot, "edit_artifact", "alpha"), null);
  for (const name of ["alpha", "released", "../alpha", "archive", "-execute", "two--hyphens", "x".repeat(129)]) {
    assert.equal(authoringRequest(repository, snapshot, "create", name), null);
  }
  fixture.snapshot.revision = "some other revision";
  assert.throws(() => admitAuthoringSnapshot(fixture, repository), /Invalid Cairn authoring readback/);
});
