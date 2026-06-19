# `zerodds-rtc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rtc/badge.svg)](https://docs.rs/zerodds-rtc)

OMG RTC 1.0 (`formal/2008-04-04`) — Robotic Technology Component.
Lightweight RTC + ExecutionContext + lifecycle state machine +
Periodic/Stimulus/Mode profiles + resource introspection. Local PSM
(§6.3) compliant; `no_std + alloc`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG RTC 1.0 | §5.2.1 ReturnCode_t |
| OMG RTC 1.0 | §5.2.2.2 LightweightRTObject |
| OMG RTC 1.0 | §5.2.2.3 LifeCycle |
| OMG RTC 1.0 | §5.2.2.4 Component-Action-Trait |
| OMG RTC 1.0 | §5.2.2.5 / §5.2.2.6 ExecutionContext |
| OMG RTC 1.0 | §5.2.2.7 ExecutionKind |
| OMG RTC 1.0 | §5.3 Execution-Semantics (Periodic/Stimulus/Modes) |
| OMG RTC 1.0 | §5.4 Resource Data Model |
| OMG RTC 1.0 | §6.3 Local PSM |

## What's included

- **`return_code`** — `ReturnCode` with all 6 status codes plus
  `is_ok()` / `into_result()` helpers.
- **`lifecycle`** — `LifeCycleState`, `ExecutionKind`,
  `ComponentAction` trait + state-machine enforcement.
- **`object`** — `LightweightRtObject`, `ExecutionContextHandle`.
- **`execution`** — `ExecutionContext`, `ExecutionContextOperations` trait.
- **`semantics`** — `DataFlowComponentAction` (Periodic),
  `FsmComponentAction` (Stimulus), `MultiModeComponentAction` +
  `ModeOfOperation` (Modes).
- **`resource`** — `Introspection`, `ComponentProfile`, `PortProfile`,
  `ConnectorProfile`, `PortDirection`, `ProfileId`.

## What's not covered

- **CORBA PSM** (§6.5) — requires a CORBA ORB; ZeroDDS has none.
- **Lightweight CCM PSM** (§6.4) — requires an LwCCM container; see
  `crates/ccm/`, which provides the IDL-equivalent transformation
  but no container.
- **§5.4 discovery/wire aspect** — partial: the resource data model
  is in the crate, the discovery wire aspect is not.

## Example

```rust
use zerodds_rtc::ReturnCode;

// Spec §5.2.1.1: ReturnCode::Ok is the only OK code.
assert!(ReturnCode::Ok.is_ok());
assert!(!ReturnCode::PreconditionNotMet.is_ok());
assert_eq!(ReturnCode::Ok.into_result(), Ok(()));
```

## Tests

```bash
cargo test -p zerodds-rtc
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [Components](../../documentation/02-architecture/components.md)
- [Spec-Coverage Audit](../../docs/spec-coverage/omg-rtc-1.0.md)
