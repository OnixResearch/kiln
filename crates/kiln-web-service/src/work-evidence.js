"use strict";

// r[impl kiln.work.change]
// Explicit identity joins only; provider observations never become work or release authority.
(() => {
  const MAX_LINKS = 64;
  const CONCURRENCY_LIMIT = 4;
  const component = (value) => encodeURIComponent(value);
  function abortError() {
    const error = new Error("Evidence read aborted");
    error.name = "AbortError";
    return error;
  }
  async function observation(kind, id, target, signal) {
    if (signal?.aborted) throw abortError();
    try { return { kind, id, available: true, value: await window.KilnWork.api(target, signal ? { signal } : undefined) }; }
    catch (error) {
      if (error.name === "AbortError" || signal?.aborted) throw error;
      return { kind, id, available: false, error: error.message || "Observation unavailable" };
    }
  }
  const unavailable = (kind, id, error) => ({ kind, id, available: false, error });
  async function read(record, options = {}) {
    const signal = options?.signal;
    if (signal?.aborted) throw abortError();
    const result = {
      source: [], reviews: [], runs: [], lifecycle: [], artifacts: [],
      non_claims: [
        "Links select exact independently admitted observations; a work status does not establish CI success.",
        "Raw collaboration history and producer review verdicts do not grant lifecycle, merge or release acceptance.",
        "Missing, inaccessible and retention-bounded observations remain unavailable; no alternate candidate is substituted.",
      ],
    };
    const links = Array.isArray(record?.fields?.links) ? record.fields.links : [];
    if (links.length > MAX_LINKS) throw new Error("Change detail link count exceeds its admitted bound.");
    const sourceLinks = links.filter((link) => link.kind === "source");
    const slots = new Array(links.length);
    let cursor = 0;
    const resolveLink = async (link) => {
      if (signal?.aborted) throw abortError();
      const { kind, id } = link;
      const out = { source: [], reviews: [], runs: [], lifecycle: [], artifacts: [] };
      if (kind === "run") {
        const path = `/api/runs/${component(id)}`;
        const run = await observation(kind, id, path, signal);
        if (run.available) {
          const [reports, artifacts, evidence] = await Promise.all([
            observation("reports", id, path + "/reports", signal),
            observation("artifacts", id, path + "/artifacts", signal),
            observation("evidence", id, path + "/evidence", signal),
          ]);
          out.runs.push({ id, available: true, run: run.value, reports, artifacts, evidence });
        } else out.runs.push(run);
      } else if (kind === "source") {
        const parts = id.split("@");
        if (parts.length !== 2 || !parts[0] || !parts[1]) {
          out.source.push(unavailable(kind, id, "Use the exact admitted source identity repository@revision."));
        } else {
          const [tree, diff] = await Promise.all([
            observation(kind, id, `/api/sources/${component(parts[0])}/tree?${new URLSearchParams({ revision: parts[1], path: "" })}`, signal),
            observation("diff", id, `/api/sources/${component(parts[0])}/diff?${new URLSearchParams({ revision: parts[1] })}`, signal),
          ]);
          out.source.push(tree, diff);
        }
      } else if (kind === "revision") {
        out.source.push(await observation(kind, id, `/api/sources/lookup?${new URLSearchParams({ revision: id })}`, signal));
        if (!sourceLinks.length) out.source.push(unavailable("diff", id, "Add an explicit repository@revision source link to select the exact source browser."));
      } else if (kind === "review") {
        out.reviews.push(await observation(kind, id, `/api/reviews/${component(id)}`, signal));
      } else if (kind === "patch" || kind === "cairn") {
        const parts = id.split("|");
        const target = kind === "cairn" ? out.lifecycle : out.reviews;
        if (parts.length !== 3 || parts.some((part) => !part)) {
          target.push(unavailable(kind, id, "Use repository|native-type|object-ID for an exact admitted COB history."));
        } else {
          const [repository, type, object] = parts;
          target.push(await observation(kind, id, `/api/cobs/object?${new URLSearchParams({ repository, type, object })}`, signal));
        }
      } else if (kind === "artifact") {
        const [runId, artifactId, extra] = id.split("|");
        if (!runId || !artifactId || extra !== undefined) {
          out.artifacts.push(unavailable(kind, id, "Use run-ID|artifact-ID to select exact retained artifact metadata."));
        } else {
          const inventory = await observation(kind, id, `/api/runs/${component(runId)}/artifacts`, signal);
          if (!inventory.available) out.artifacts.push(inventory);
          else {
            const artifact = inventory.value.artifacts?.find((row) => row.id === artifactId);
            out.artifacts.push(artifact ? { kind, id, available: true, value: artifact } : unavailable(kind, id, "Exact artifact is not retained for this run."));
          }
        }
      }
      return out;
    };
    const workers = Array.from({ length: Math.min(CONCURRENCY_LIMIT, links.length) }, async () => {
      while (cursor < links.length) {
        if (signal?.aborted) throw abortError();
        const index = cursor++;
        slots[index] = await resolveLink(links[index]);
      }
    });
    await Promise.all(workers);
    if (signal?.aborted) throw abortError();
    for (const slot of slots) {
      if (!slot) continue;
      result.source.push(...slot.source);
      result.reviews.push(...slot.reviews);
      result.runs.push(...slot.runs);
      result.lifecycle.push(...slot.lifecycle);
      result.artifacts.push(...slot.artifacts);
    }
    return result;
  }
  window.KilnWorkEvidence = Object.freeze({ read });
})();
