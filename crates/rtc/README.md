# `zerodds-rtc`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-rtc/badge.svg)](https://docs.rs/zerodds-rtc)

OMG RTC 1.0 (`formal/2008-04-04`) — Robotic Technology Component.
Lightweight RTC + ExecutionContext + Lifecycle-State-Machine +
Periodic/Stimulus/Mode-Profile + Resource-Introspection. Local PSM
(§6.3) konform; `no_std + alloc`, `forbid(unsafe_code)`. Safety
classification: **STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
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

## Was ist drin

- **`return_code`** — `ReturnCode` mit allen 6 Status-Codes plus
  `is_ok()` / `into_result()` Helper.
- **`lifecycle`** — `LifeCycleState`, `ExecutionKind`,
  `ComponentAction`-Trait + State-Machine-Enforcement.
- **`object`** — `LightweightRtObject`, `ExecutionContextHandle`.
- **`execution`** — `ExecutionContext`, `ExecutionContextOperations`-Trait.
- **`semantics`** — `DataFlowComponentAction` (Periodic),
  `FsmComponentAction` (Stimulus), `MultiModeComponentAction` +
  `ModeOfOperation` (Modes).
- **`resource`** — `Introspection`, `ComponentProfile`, `PortProfile`,
  `ConnectorProfile`, `PortDirection`, `ProfileId`.

## Was nicht abgedeckt ist

- **CORBA PSM** (§6.5) — verlangt CORBA-ORB; ZeroDDS hat keinen.
- **Lightweight CCM PSM** (§6.4) — verlangt LwCCM-Container; siehe
  `crates/ccm/` welche die IDL-Equivalent-Transformation liefert,
  aber keinen Container bereitstellt.
- **§5.4 Discovery-/Wire-Aspekt** — partial: Resource-Daten-Modell
  ist im Crate, der Discovery-Wire-Aspekt nicht.

## Beispiel

```rust
use zerodds_rtc::ReturnCode;

// Spec §5.2.1.1: ReturnCode::Ok ist der einzige OK-Code.
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
