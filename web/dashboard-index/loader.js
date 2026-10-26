import initialize, { mount_index, select_history } from "./bindgen/kiln_dashboard_index.js";

// Supplying bytes avoids wasm-bindgen's URL/fetch initialization branch entirely.
async function load_index(bytes) {
  if (!(bytes instanceof Uint8Array)) {
    throw new TypeError("Kiln index initialization requires embedded Uint8Array bytes");
  }
  await initialize({ module_or_path: bytes });
  return { mount_index, select_history };
}

// Stable property names survive identifier minification in the embedded module.
window.kilnRatsLoader = { load_index };
