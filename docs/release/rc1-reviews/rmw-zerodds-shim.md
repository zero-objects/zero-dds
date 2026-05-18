# RC1 Review — `rmw-zerodds-shim`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 7 Profiles
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

## 1 Purpose

ROS 2 RMW shim — liefert `librmw_zerodds.{so,dylib,dll}` so dass rclcpp/rclpy ueber RMW_IMPLEMENTATION=rmw_zerodds_cpp ZeroDDS als RMW-Backend nutzen kann.

## 2 Public-Strategy

- **Marker:** `🌐 public`
- **Begründung:** Layer-7-Profile-Crate, public-fokussiert.

## 3 Content-Inventur

Siehe `crates/rmw-zerodds-shim/README.md` + `CHANGELOG.md` + `src/`.

## 4 Coherence-Audit

Alle Public-Items klassifiziert ✅; keine TEST-ONLY/DEAD-Items.

## 5 Spec-Coverage

REP-2007 §3-5 Pub-Sub-Pipeline; Services/Actions sind Stubs mit RMW_RET_UNSUPPORTED.

## 6 Forbidden-Token-Sweep

- §2.1 Hard-Forbidden: 0 hits ✅
- §2.1b Sprint/Project-Marker: 0 hits ✅
- §2.1c Datums-Marker: 0 hits ✅

## 7 §1.13 Spec-Conformance HARD-BLOCKER

- Inline-Deferral-Marker: 0 hits ✅
- Spec-Section-Coverage: alle relevanten §-Items auf done in den
  zugehörigen Spec-Coverage-Files
- Wire-Konformität: ueber zerodds-dcps + zerodds-rtps Wire-Bytes-Form
- Kohärenz: (a) wired, (b) Konsumenten in Cross-Crate-Tests, (c)
  getestet mit 14 tests

## 8 Tests + Lints + Doc-Build

- `cargo test -p rmw-zerodds-shim`: 14 tests
- `cargo clippy -p rmw-zerodds-shim --tests -- -D warnings`: clean
- License-Header §1.8: alle src/*.rs OK
- Cargo.toml §1.1: vollständig
- README.md §1.3: present
- CHANGELOG.md §1.4: present mit `[1.0.0-rc.1]`-Eintrag

## 9 Sign-off

✅ rc1-ready

## 10 CLI-Subcommand-Append

Folgende Items sind nach dem ersten Sign-off in den
`zerodds-ros2-shim`-CLI eingebracht worden (kein Major-Bump, alles
innerhalb 1.0.0-rc.1):

- `catalog`-Subcommand (Topic-Inventur).
- `metrics`-Subcommand (Prometheus-Snapshot).
- `selftest`-Subcommand.
- `zerodds-monitor` + `zerodds-observability-otlp`-Integration fuer
  JSON-Lines + OTLP-Spans-Diagnose; CLI ist exempt vom
  SIGTERM-Watcher (§9.2 gilt nur fuer Daemons).
- Tests gruen: 14 (kein Drift gegen pre-Append-Tally).
