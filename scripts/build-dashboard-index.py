#!/usr/bin/env python3
"""Build the locked, self-contained Ratzilla index assets; never install tools."""

from browser_wasm_assets import ROOT, Client, LocalSource, main

CLIENT = Client(
    package="kiln-dashboard-index",
    directory=ROOT / "web/dashboard-index",
    destination=ROOT / "crates/kiln-dashboard/src/render/generated",
    js="rats-index.js",
    wasm="rats-index.wasm.b64",
    notices="rats-index-notices.txt",
    build_label="kiln-dashboard-build",
    title="dashboard index",
    notice_header=(
        "Kiln offline repository/history index — dependency notices\n"
        "This is the locked wasm32 metadata dependency cohort (including build tools, proc macros and optional edges),\n"
        "not a claim that every package is linked into the final WASM. Original license/NOTICE texts follow.\n"
        "Registry packages come from the public crates.io registry; local package source is identified below.\n"
    ),
    local_sources={
        "kiln-dashboard-index": LocalSource("Kiln browser workspace", ROOT, ("LICENSE-MIT", "LICENSE-APACHE")),
        "rat-table": LocalSource(
            "Subwayrat selected source at 2e52b3150819a2365aaefd3dcf8bbd2a2fa2e901 (Kiln wrapper manifest)",
            ROOT / "vendor/subwayrat", ("LICENSE", "LICENSING.md")),
    },
)


# r[impl kiln.dashboard.rats.provenance.scenario.drift]
# r[verify kiln.dashboard.rats.provenance.scenario.drift]
if __name__ == "__main__":
    main(CLIENT, __doc__)
