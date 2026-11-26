## 1. Capture

- [ ] [serial] [task:ansi-capture] Add bounded `format=ansi` visible capture to the Fleet route, adapter, and web bridge, with positive and negative tests. r[kiln.sdlc.herdr_viewer]

## 2. Viewer

- [ ] [serial] [task:viewer-module] [after:ansi-capture] Add the `web/herdr-viewer` Ratzilla workspace and build script with generated assets, notices and a flake freshness check. r[kiln.sdlc.herdr_viewer]
- [ ] [serial] [task:console] [after:viewer-module] Serve the assets and add the Fleet console viewer with bounded visible refresh and plain-text fallback. r[kiln.sdlc.herdr_viewer.scenario.view]

## 3. Verification

- [ ] [serial] [task:verify] [after:console] Verify in a browser against a disposable Herdr pane that shows colored output, and check escape and markup injection stay inert. r[kiln.sdlc.herdr_viewer.scenario.inert]
