"use strict";

// r[impl kiln.work.change]
// Explicit identity joins only; provider observations never become work or release authority.
(() => {
  const MAX_LINKS = 64;
  const component = (value) => encodeURIComponent(value);
  async function observation(kind, id, target) {
    try { return { kind, id, available: true, value: await window.KilnWork.api(target) }; }
    catch (error) {
      if (error.name === "AbortError") throw error;
      return { kind, id, available: false, error: error.message || "Observation unavailable" };
    }
  }
  const unavailable = (kind, id, error) => ({ kind, id, available: false, error });
  async function read(record) {
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
    for (const link of links) {
      const { kind, id } = link;
      if (kind === "run") {
        const path = `/api/runs/${component(id)}`;
        const run = await observation(kind, id, path);
        if (run.available) {
          const [reports, artifacts, evidence] = await Promise.all([
            observation("reports", id, path + "/reports"),
            observation("artifacts", id, path + "/artifacts"),
            observation("evidence", id, path + "/evidence"),
          ]);
          result.runs.push({ id, available: true, run: run.value, reports, artifacts, evidence });
        } else result.runs.push(run);
      } else if (kind === "source") {
        const parts = id.split("@");
        if (parts.length !== 2 || !parts[0] || !parts[1]) {
          result.source.push(unavailable(kind, id, "Use the exact admitted source identity repository@revision."));
          continue;
        }
        result.source.push(await observation(kind, id, `/api/sources/${component(parts[0])}/tree?${new URLSearchParams({ revision: parts[1], path: "" })}`));
        result.source.push(await observation("diff", id, `/api/sources/${component(parts[0])}/diff?${new URLSearchParams({ revision: parts[1] })}`));
      } else if (kind === "revision") {
        result.source.push(await observation(kind, id, `/api/sources/lookup?${new URLSearchParams({ revision: id })}`));
        if (!sourceLinks.length) result.source.push(unavailable("diff", id, "Add an explicit repository@revision source link to select the exact source browser."));
      } else if (kind === "review") {
        result.reviews.push(await observation(kind, id, `/api/reviews/${component(id)}`));
      } else if (kind === "patch" || kind === "cairn") {
        const parts = id.split("|");
        const target = kind === "cairn" ? result.lifecycle : result.reviews;
        if (parts.length !== 3 || parts.some((part) => !part)) {
          target.push(unavailable(kind, id, "Use repository|native-type|object-ID for an exact admitted COB history."));
          continue;
        }
        const [repository, type, object] = parts;
        target.push(await observation(kind, id, `/api/cobs/object?${new URLSearchParams({ repository, type, object })}`));
      } else if (kind === "artifact") {
        const [runId, artifactId, extra] = id.split("|");
        if (!runId || !artifactId || extra !== undefined) {
          result.artifacts.push(unavailable(kind, id, "Use run-ID|artifact-ID to select exact retained artifact metadata."));
          continue;
        }
        const inventory = await observation(kind, id, `/api/runs/${component(runId)}/artifacts`);
        if (!inventory.available) result.artifacts.push(inventory);
        else {
          const artifact = inventory.value.artifacts?.find((row) => row.id === artifactId);
          result.artifacts.push(artifact ? { kind, id, available: true, value: artifact } : unavailable(kind, id, "Exact artifact is not retained for this run."));
        }
      }
    }
    return result;
  }
  window.KilnWorkEvidence = Object.freeze({ read });
})();
