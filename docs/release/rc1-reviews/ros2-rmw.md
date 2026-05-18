# RC1 Review — `zerodds-ros2-rmw`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 7 Profiles
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

## 1 Purpose

ROS2-RMW-Mapping-Layer — Topic-Mangling, QoS-Profile, Identifier-Constraints per REP-2007/2008/2009.

## 2 Public-Strategy

- **Marker:** `🌐 public`
- **Begründung:** Layer-7-Profile-Crate, public-fokussiert.

## 3 Content-Inventur

Siehe `crates/ros2-rmw/README.md` + `CHANGELOG.md` + `src/`.

## 4 Coherence-Audit

Alle Public-Items klassifiziert ✅; keine TEST-ONLY/DEAD-Items.

## 5 Spec-Coverage

REP-2007/2008/2009: 13 done + 4 n/a (rejected/informative) in `docs/spec-coverage/ros2-rmw.md`.

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
  getestet mit 56 tests

## 8 Tests + Lints + Doc-Build

- `cargo test -p zerodds-ros2-rmw`: 56 tests
- `cargo clippy -p zerodds-ros2-rmw --tests -- -D warnings`: clean
- License-Header §1.8: alle src/*.rs OK
- Cargo.toml §1.1: vollständig
- README.md §1.3: present
- CHANGELOG.md §1.4: present mit `[1.0.0-rc.1]`-Eintrag

## 9 Sign-off

✅ rc1-ready

## 10 Service / Action / Cross-Vendor Append

Folgende Items sind nach dem ersten Sign-off eingebracht worden (kein
Major-Bump, alles innerhalb 1.0.0-rc.1):

- `service.rs` (REP-2008 Request-Reply ueber `zerodds-rpc` Topic-Naming).
- `action.rs` (REP-2009 Goal/Feedback/Result-Pattern).
- `msg_to_idl.rs` (ROS-2 `.msg`/`.srv`-Subset → IDL-AST-Mapping fuer
  Type-Hash, REP-2007).
- `json_log.rs` (strukturierte rmw-Diagnose-Sink).
- `cross_vendor.rs` (rclcpp/rclpy-Kompatibilitaet).
- Tests gruen: 56 (kein Drift gegen pre-Append-Tally).
