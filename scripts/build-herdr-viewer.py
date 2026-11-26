#!/usr/bin/env python3
"""Build the locked, self-contained Rio Herdr pane viewer assets; never install tools."""

from browser_wasm_assets import ROOT, Client, LocalSource, main

RIO = ROOT / "vendor/rio"
RIO_LABEL = "Rio selected source at b13db0193c61c105b86e12b4ac6c6fc255cf0a36 (Kiln wrapper manifest)"

CLIENT = Client(
    package="kiln-herdr-viewer",
    directory=ROOT / "web/herdr-viewer",
    destination=ROOT / "crates/kiln-web-service/src/generated",
    js="herdr-viewer.js",
    wasm="herdr-viewer.wasm.b64",
    notices="herdr-viewer-notices.txt",
    build_label="kiln-herdr-viewer-build",
    title="Herdr viewer",
    notice_header=(
        "Kiln read-only Herdr pane viewer — dependency notices\n"
        "This is the locked wasm32 metadata dependency cohort (including build tools, proc macros and optional edges),\n"
        "not a claim that every package is linked into the final WASM. Original license/NOTICE texts follow.\n"
        "Registry packages come from the public crates.io registry; local package source is identified below.\n"
    ),
    local_sources={
        "kiln-herdr-viewer": LocalSource("Kiln browser workspace", ROOT, ("LICENSE-MIT", "LICENSE-APACHE")),
        **{name: LocalSource(RIO_LABEL, RIO, ("LICENSE",))
           for name in ("librio", "rio-fonts", "rio-graphics", "rio-unicode", "rio-vt")},
        # A WezTerm-derived crate: its own notice travels with Rio's.
        "rio-grapheme-width": LocalSource(RIO_LABEL, RIO, ("LICENSE", "rio-grapheme-width/LICENSE")),
    },
)


# r[impl kiln.sdlc.herdr_viewer]
if __name__ == "__main__":
    main(CLIENT, __doc__)
