# `zerodds-inspect-endpoint`

ZeroDDS inspect endpoint — feature-gated tap hooks for the external
PDE Reality Inspector (zerodds-pde).

Part of [**ZeroDDS**](../../README.md). Safety classification: **STANDARD**.

## Status

Debug-tool crate, fully `#[cfg(feature = "inspect")]`-gated and
`#![forbid(unsafe_code)]`. In the production build (feature off) the
entire tap mechanism is dropped — no hot-path branch (R-099, C-021).

## Architecture

* `tap` — trait + registry for tap hooks on DCPS/RTPS/transport.
* `frame` — wire frame for the side channel.
* `auth` — `cert.d` loader for X.509 PEM certs (R-100..R-104).
* `server` (feature `inspect`) — broadcast hook + inspect server.

## Security invariants

* **Ghost inject** (R-110): inject functions are separate API paths
  and publish directly into the DDS production data path, **without** going
  through the tap hooks. Production taps do not see the inject.
* **Idle-branch elision**: tap-hook calls in dcps/rtps/transport
  are hidden behind `#[cfg(feature = "inspect")]`. Without the feature
  there is no branch in the hot path.

## Usage

```toml
[dependencies]
zerodds-inspect-endpoint = { version = "1", features = ["inspect"] }
```

The default is OFF — `inspect` must be enabled explicitly so that
the tap hooks and the server path are compiled.

## Tests

```bash
cargo test -p zerodds-inspect-endpoint --features inspect
```

## See also

* [`docs/architecture/04_safety_by_architecture.md`](../../docs/architecture/04_safety_by_architecture.md) —
  safety classification.
* [`crates/dcps/Cargo.toml`](../dcps/Cargo.toml) `inspect` feature —
  optional consumer of the tap hooks.
