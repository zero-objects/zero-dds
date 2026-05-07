# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialisation.

### Implementation

* Tauri 2.0 shell with `tauri-build` build script.
* Embeds `zerodds-dashboard` HTTP server on a background thread
  bound to `127.0.0.1:8089`; a `dist/index.html` splash redirects
  the WebView to the SPA once the server is ready.
* Eight Tauri commands: `inject_topics`, `inject_participants`,
  `inject_histograms`, `get_topics`, `get_participants`,
  `get_histograms`, `get_graph`, `get_recording`.

### Bundles

* macOS `.dmg` (universal x86_64 + aarch64).
* Linux `.AppImage`.
* Windows `.msi`.

### Architecture

* Layer: Tools.
* Dependencies (in): `tauri` 2.0, `zerodds-dashboard`.

### Stability

The Tauri command set is RC1-stable. Breaking changes require a
major version bump.
