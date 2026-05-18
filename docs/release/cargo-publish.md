# `cargo publish` Workflow

Pfad fuer ZeroDDS-Crates auf crates.io. Stand fuer Release `1.0.0-rc.1`.

## Public-Surface-Crates

Per RC1-Review-Audit (`docs/release/rc1-reviews/<crate>.md` Markierung
`Public-Strategy: 🌐 public`) ist die Mehrheit der Workspace-Crates
publishable. Aktueller Stand: ~96 Crates `publish = true`,
9 Crates `publish = false` (siehe unten).

Die 🌐-public-Surface umfasst:

* **Foundation/Protocol/QoS/Transport** — `zerodds-foundation`,
  `zerodds-cdr`, `zerodds-cdr-derive`, `zerodds-types`, `zerodds-qos`,
  `zerodds-rtps`, `zerodds-discovery`, `zerodds-transport*`.
* **DCPS-Runtime** — `zerodds-dcps`, `zerodds-dcps-async`,
  `zerodds-rpc`, `zerodds-rs`, `zerodds-c-api` und die Sprach-
  Bindings (`zerodds-cs`, `zerodds-py`, `zerodds-sys`,
  `zerodds-ts-wasm`, `zerodds-java-omgdds`).
* **Bridges + Endpoints** — `zerodds-amqp-bridge` + `-endpoint`,
  `zerodds-coap-bridge`, `zerodds-mqtt-bridge`,
  `zerodds-websocket-bridge`, `zerodds-grpc-bridge`,
  `zerodds-http2`, `zerodds-hpack`, `zerodds-zenoh-bridge`,
  `zerodds-corba-dds-bridge`, `zerodds-bridge-security`.
* **CORBA/CCM-Stack** — `zerodds-corba-{giop,iiop,ior,csiv2,poa,
  cosnaming,ir,codegen,cos-event,ccm,ccm-ejb,ccm-lib,dnc}`,
  `zerodds-corba-rust`, `zerodds-ami4ccm`, `zerodds-ccm`.
* **Optionale Spec-Profile** — `zerodds-dlrl`, `zerodds-dlrl-codegen`,
  `zerodds-soap`, `zerodds-xml-wire`, `zerodds-web`,
  `zerodds-opcua-gateway`, `zerodds-xrce`, `zerodds-xrce-agent`,
  `zerodds-xrce-client`.
* **ROS-2-Pfad** — `zerodds-ros2-rmw`, `rmw-zerodds-shim`.
* **Observability/Recorder** — `zerodds-recorder`, `zerodds-monitor`,
  `zerodds-observability-otlp`, `zerodds-conformance`,
  `zerodds-rt-linux`, `zerodds-rtc`, `zerodds-time-service`,
  `zerodds-flatdata`, `zerodds-flatdata-derive`,
  `zerodds-sql-filter`, `zerodds-transport-tsn`,
  `zerodds-inspect-endpoint`.
* **Security-Stack** — `zerodds-security` + alle `zerodds-security-*`
  Plugins.
* **IDL-Compiler-Layer** — `zerodds-idl`, `zerodds-idlc`,
  `zerodds-idl-{cpp,csharp,java,rust,ts}`, `zerodds-xmlc`,
  `zerodds-xml`.
* **CLI-Tools** — `zerodds-admin`, `zerodds-amqp-dds-endpoint`,
  `zerodds-bench-suite`, `zerodds-chaos`, `zerodds-chaos-clock-skew`,
  `zerodds-dashboard`, `zerodds-recorder-bridge`, `zerodds-replay`.

`publish = false` (Internal-only, **bleiben geblockt**):

| Crate | Begruendung |
| ----- | ----------- |
| `zerodds-cargo-dag` | Konsumiert in-tree Workspace-Graph |
| `zerodds-interop-matrix` | Konsumiert ZeroDDS-CI-Artefakte |
| `zerodds-isolation-smoke` | Internal Smoke-Test-Runner |
| `zerodds-perf` | Internal Benchmark-Suite |
| `zerodds-qos-matrix` | QoS-Compat-Matrix-Renderer |
| `zerodds-traceability` | Requirements-Matrix-Generator |
| `dds-roundtrip-codegen` | Internal Build-Helper |
| `zerodds-lint` | Custom Project-Lints (`dds_no_panic_in_safe`, ...) |
| `zerodds-cpp` | Distribution via Header-Tarball aus `include/`, nicht crates.io (siehe Mainline-Doku §08-Release) |

## Publish-Reihenfolge

Erzeugt durch `cargo-dag`:

```bash
cargo run --release -p zerodds-cargo-dag -- . --format verbose
```

`cargo-dag` macht eine topologische Sortierung der Workspace-Crates
nach internem `path = "..."`-Dep-Graph. Dev-Deps werden ignoriert,
Build-Deps mitgerechnet. Crates ohne Workspace-Deps kommen zuerst,
gefolgt von Crates die nur auf Level-0 deps nuetzen, etc.

Der Output ist deterministisch (alphabetischer Tie-Break), reproducible
ueber Runs.

## Versions-Strategie

* Workspace-Version `1.0.0-rc.1` (Release-Candidate). Pre-1.0-Sprints
  liefen unter `0.0.0` als Platzhalter; mit der Aufnahme der Publish-
  Reihenfolge wurde atomic auf `1.0.0-rc.1` gebumpt.
* Atomic-Bump der gesamten Workspace via `cargo workspaces version`
  (separat installierbar): `cargo install cargo-workspaces`.
* Patch-/Minor-Bumps zwischen Releases via
  `cargo workspaces version patch|minor`.
* Pflicht-Sync: bei Workspace-Bump muessen die Pfad-Dep-Constraints
  (`version = "X.Y.Z", path = "..."`) mitziehen, sonst lehnt
  `cargo publish` die Resolution gegen crates.io ab.

## Pflichtfelder pro Crate (vor Publish)

Jeder publishable Crate muss folgende Cargo.toml-Felder gesetzt haben:

```toml
[package]
name = "dds-..."
version.workspace = true
edition.workspace = true
license = "Apache-2.0"
repository = "https://github.com/zerodds/zerodds"  # echte URL ersetzen
description = "..."  # >= 10 Zeichen
readme = "README.md"
keywords = ["dds", "rtps", "..."]    # max 5
categories = ["..."]                  # crates.io taxonomy
publish = ["crates-io"]                # statt false
```

Plus per Crate eine `README.md` mit Spec-Mapping und Safety-
Klassifikation.

## License-Audit

```bash
cargo deny check licenses
```

Fuer 1.0.0-rc.1: alles `Apache-2.0`. Build-Deps (cbindgen MPL-2.0,
hdrhistogram MIT) sind ueber `deny.toml`-Exceptions zugelassen.

## Erste Publikation

Reihenfolge:

```bash
# 1) Authentifizieren
cargo login <crates.io-API-token>

# 2) Pre-Flight: Dry-Run
for c in $(./target/release/cargo-dag . --only-publishable); do
  echo "=== $c ==="
  cargo publish -p "$c" --dry-run --allow-dirty
done

# 3) Echte Publikation in DAG-Order
for c in $(./target/release/cargo-dag . --only-publishable); do
  cargo publish -p "$c"
  sleep 60   # crates.io rate-limit
done
```

Yank-Strategie: bei Fehler in Crate `X` direkt nach Publikation:

```bash
cargo yank --version 1.0.0-rc.1 zerodds-X
```

## RC1-Publish-Checkliste

1. Repository-URL `https://github.com/zero-objects/zero-dds`,
   Homepage `https://zerodds.org` (workspace-default). Pre-Publish:
   verifizieren dass beide live + lesbar sind.
2. Pflichtfelder pro public Crate gesetzt (`description`, `keywords`,
   `categories`, `repository`, `homepage`, `documentation`, `readme`,
   `authors`, `publish = true`). RC1-Reviews unter
   `docs/release/rc1-reviews/<crate>.md` halten den Review-Stand fest.
3. README pro public Crate (Template in
   `docs/release/crate-readme-template.md`).
4. License-Audit gruen (`cargo deny check`).
5. Workspace-Version steht auf `1.0.0-rc.1`; alle Pfad-Dep-Constraints
   tragen `version = "1.0.0-rc.1", path = "..."`.
6. `cargo-dag --only-publishable` Reihenfolge erzeugen, sequentiell
   `cargo publish` mit `sleep 60` zwischen Crates (crates.io
   Rate-Limit).
