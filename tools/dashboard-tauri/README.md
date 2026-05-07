# `zerodds-dashboard-tauri` — Native App Wrapper

> Tauri 2.0 native shell that embeds the `zerodds-dashboard`
> backend in-process and renders the same SPA in a native window.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

## Build targets

* macOS: `.dmg` (universal: x86_64 + aarch64).
* Linux: `.AppImage`.
* Windows: `.msi`.

## Prerequisites

* `tauri-cli` ≥ 2.0:
  ```bash
  cargo install tauri-cli --version "^2.0"
  ```
* Platform system dependencies:
  - macOS: Xcode Command-Line-Tools.
  - Linux: `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libgtk-3-dev`,
    `libayatana-appindicator3-dev`, `librsvg2-dev`.
  - Windows: WebView2 runtime + MSVC build tools.

## Build

```bash
cd tools/dashboard-tauri
cargo tauri dev   # development with live reload
cargo tauri build # native bundle in src-tauri/target/release/bundle/
```

The app starts a `zerodds-dashboard` HTTP server on
`127.0.0.1:8089` at launch; the embedded splash
(`dist/index.html`) redirects to the SPA once the server is ready.

## Architecture

```
+--------------- Native window ---------------+
|  src-tauri/main.rs                          |
|    └── thread::spawn(run_blocking(:8089))   |
|         └── zerodds-dashboard HTTP server   |
|              └── DashboardState             |
|                                             |
|  WebView  (loads http://127.0.0.1:8089/)    |
|    └── d3 force graph + histograms          |
+---------------------------------------------+
```

## Inject state from outside

An external provider (live `DcpsRuntime` hook, test harness) can
stream data via the HTTP API:

```bash
curl -X POST http://127.0.0.1:8089/api/inject/topics \
  -H "Content-Type: application/json" \
  -d '[{"name":"/cmd_vel","type_name":"geometry_msgs::msg::Twist","publishers":2,"subscribers":1,"sample_rate_hz":50.0}]'
```

Endpoints:
* `POST /api/inject/topics`
* `POST /api/inject/participants`
* `POST /api/inject/histograms`

## IPC bridge

The frontend can talk to the back-end directly through
`window.__TAURI__.invoke()` — no HTTP fetch, no CORS:

```js
import { invoke } from '@tauri-apps/api/core';
await invoke('inject_topics', { json: JSON.stringify([{ /* ... */ }]) });
const topics = JSON.parse(await invoke('get_topics'));
```

| Tauri command | Arguments | Returns |
| --- | --- | --- |
| `inject_topics` | `{ json }` | `usize` (count) |
| `inject_participants` | `{ json }` | `usize` |
| `inject_histograms` | `{ json }` | `usize` |
| `get_topics` | — | `String` (JSON body) |
| `get_participants` | — | `String` |
| `get_histograms` | — | `String` |
| `get_graph` | — | `String` |
| `get_recording` | — | `String` |

## Stability

`1.0.0-rc.1` — public command set is stable. Breaking changes
require a major version bump.

## Icons

`src-tauri/icons/` ships the default ZeroDDS marks. Replace with
your branding before vendor builds.

## Licence

Apache-2.0. See [`LICENSE`](../../LICENSE).
