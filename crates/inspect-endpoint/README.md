# `zerodds-inspect-endpoint`

ZeroDDS Inspect-Endpoint — feature-gated Tap-Hooks fuer den externen
PDE Reality Inspector (zerodds-pde).

Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Status

Debug-Tool-Crate, vollstaendig `#[cfg(feature = "inspect")]`-gated und
`#![forbid(unsafe_code)]`. Im Production-Build (Feature aus) faellt
der gesamte Tap-Mechanismus weg — kein Hot-Path-Branch (R-099, C-021).

## Architektur

* `tap` — Trait + Registry fuer Tap-Hooks an DCPS/RTPS/Transport.
* `frame` — Wire-Frame fuer den Side-Channel.
* `auth` — `cert.d`-Loader fuer X.509-PEM-Certs (R-100..R-104).
* `server` (feature `inspect`) — Broadcast-Hook + Inspect-Server.

## Sicherheits-Invarianten

* **Ghost-Inject** (R-110): Inject-Funktionen sind separate API-Pfade
  und publishen direkt in den DDS-Production-Datenpfad, **ohne** durch
  die Tap-Hooks zu laufen. Production-Taps sehen den Inject nicht.
* **Idle-Branch-Elision**: Tap-Hook-Aufrufe in dcps/rtps/transport
  sind hinter `#[cfg(feature = "inspect")]` versteckt. Ohne Feature
  kein Branch im Hot-Path.

## Verwendung

```toml
[dependencies]
zerodds-inspect-endpoint = { version = "1", features = ["inspect"] }
```

Default ist OFF — `inspect` muss explizit aktiviert werden, damit
die Tap-Hooks und der Server-Pfad uebersetzt werden.

## Tests

```bash
cargo test -p zerodds-inspect-endpoint --features inspect
```

## Siehe auch

* [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) —
  Safety-Klassifikation.
* [`crates/dcps/Cargo.toml`](../dcps/Cargo.toml) `inspect`-Feature —
  optionaler Konsument der Tap-Hooks.
