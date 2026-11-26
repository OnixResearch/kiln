import initialize, { mount } from "./bindgen/kiln_herdr_viewer.js";

const MODULE = "/herdr-viewer.wasm.b64";
let ready = null;

// Fetch the base64 module from Kiln's own origin and supply its bytes, which
// avoids wasm-bindgen's URL/fetch initialization branch entirely.
async function initialized() {
  const response = await fetch(MODULE, { credentials: "same-origin", cache: "no-cache" });
  if (!response.ok) throw new Error(`Herdr viewer module unavailable (${response.status})`);
  const binary = atob((await response.text()).trim());
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  await initialize({ module_or_path: bytes });
  return Object.freeze({
    // The viewer is read-only: render(bytes, columns, rows) and dispose() only.
    mount(element) {
      const viewer = mount(element);
      return Object.freeze({
        render: (snapshot, columns, rows) => viewer.render(snapshot, columns, rows),
        dispose: () => viewer.dispose(),
      });
    },
  });
}

async function load() {
  ready ??= initialized();
  try {
    return await ready;
  } catch (error) {
    ready = null;
    throw error;
  }
}

// Stable property names survive identifier minification in the bundle.
window.KilnHerdrViewer = Object.freeze({ load });
