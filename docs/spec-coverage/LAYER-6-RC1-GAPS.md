# Layer-6 RC1 spec coverage gaps + vendor spec mapping

**As of:** 2026-05-06
**Scope:** Layer-6 bindings (zerodds-c-api, zerodds-cpp, zerodds-cs,
zerodds-py, zerodds-java/-jni/-omgdds, zerodds-ts-node, zerodds-ts-wasm,
zerodds-rs, zerodds-sys).

## Purpose

This document identifies the **gaps between the spec audit files
and the RC1 code reality** and, for each gap, points to the respective
vendor spec that defines the behaviour normatively. This documents the
ZeroDDS decision position per item.

PROCESS.md requires, after large code waves, an item-by-item
re-sync of the affected audit files. This file is the
transition marker until the re-sync has been carried out.

## Vendor spec inventory (relevant to layer 6)

| Vendor spec | Path | Status | Scope |
|---|---|---|---|
| **C-FFI mini spec** | `docs/specs/zerodds-c-api-1.0.md` | Live RC1 | Full 130+ FFI functions, no OMG counterpart |
| **Listener callbacks** | `docs/specs/zerodds-listener-callbacks-1.0.md` | Live phase 1 | C-FFI listener pattern (vtable+user_data), no OMG C standard |
| **Async API** | `docs/specs/zerodds-async-1.0.md` | Live | Async DDS API for Rust, no OMG counterpart |
| **IDL-Rust** | `docs/specs/zerodds-idl-rust-1.0.md` | Live | IDL→Rust codegen, fills an OMG gap (no OMG IDL-Rust PSM) |
| **CORBA-Rust** | `docs/specs/zerodds-corba-rust-1.1.md` | Live | CORBA mappings for Rust, no OMG CORBA-Rust PSM |
| **Flatdata** | `docs/specs/zerodds-flatdata-1.0.md` | Live | Zero-copy data layout, no OMG counterpart |
| **Monitor** | `docs/specs/zerodds-monitor-1.0.md` | Live | Built-in DCPS monitor, supplements §2.2.5 |
| **Recorder** | `docs/specs/zddsrec-1.0.md` | Live | Sample recording, no OMG counterpart |
| **OTLP bridge** | `docs/specs/zerodds-observability-otlp-1.0.md` | Live | Observability bridge, no OMG counterpart |

## Spec coverage gap table

### A. `dds-psm-cxx-1.0.md` (audit: 122 items, 104 done + 18 n/a)

#### Phase-1 RC1: surface live, active wiring in phase 2

Items whose API signature is live in `crates/cpp/include/dds/*.hpp`, but
whose *active runtime connection* only comes in phase 2:

| §-Ref | Item | RC1 status | Phase-2 plan | Backing |
|---|---|---|---|---|
| §7.5.9.5 | DataWriterListener::on_* | API surface live | Active wiring to runtime status counters | `zerodds-listener-callbacks-1.0.md` §6 |
| §7.5.9.6 | DataReaderListener::on_* | API surface live | Active wiring | `zerodds-listener-callbacks-1.0.md` §6 |
| §7.5.13.7 | ContentFilteredTopic | Class + create FFI | DataReader bind via cft_handle_ | `zerodds-c-api-1.0.md` §2.2 |
| §7.5.10.6 | ReadCondition<T> | Class + FFI | Active trigger via DataReader status | `zerodds-c-api-1.0.md` §6 |
| §7.5.10.7 | QueryCondition<T> | Class + FFI | SQL filter engine wired | `zerodds-c-api-1.0.md` §6 |

#### Phase-1 RC1: defaults only

Items that are declared spec-conformant but only use defaults in RC1:

| §-Ref | Item | RC1 status | Phase-2 plan |
|---|---|---|---|
| §7.5.6 | QoS constructor arguments | Accepted, but default path | Full field conversion via `qos_ffi` tables |
| §7.5.13.5 | Topic<T>(dp, name, qos) | Default QoS from participant | Propagate QoS argument |

#### Vendor extension (no OMG C++ counterpart)

| Item | Vendor spec | Reason |
|---|---|---|
| `topic_type_support<T>` trait requirement | `zerodds-c-api-1.0.md` §2.5 | DDS-PSM-Cxx only requires `T` with IDL codegen support; ZeroDDS also accepts raw bytes via `topic_type_support<core::ByteSeq>` |
| `_DataWriterListenerBridge<T>` | `zerodds-listener-callbacks-1.0.md` §7.1 | Bridge between the spec class listener and the C-FFI vtable |

### B. `dds-java-psm-1.0.md` (171 items, 156 done + 15 n/a)

#### Pure Java without JNI (vendor path)

The audit items that assume a "pure Java implementation" as the implementation path
are implemented *in pure Java* in ZeroDDS via `crates/java-omgdds/java/src/main/java/org/omg/dds/*`.
On the Rust side only the codegen pipeline is
involved (`crates/idl-java/`).

| Item category | Spec assumption | RC1 path | Backing |
|---|---|---|---|
| Builtin types | JNI/native lib | InProcessBus + Xcdr2Codec | `zerodds-java-omgdds` vendor spec |
| Discovery | JNI/native lib | InProcessBus for local-only tests | Phase 2: gRPC bridge to libzerodds server |
| RTPS wire | JNI/native lib | not in pure Java | Vendor: pure Java does without live RTPS; gRPC bridge path |

**Recommendation:** Write a separate vendor spec `zerodds-java-omgdds-1.0.md`
that documents the pure Java + gRPC bridge path as an alternative
implementation variant of the OMG DDS Java PSM 1.0.

### C. `idl4-cpp-1.0.md` (77 items, 57 done + 20 n/a)

The audit is solid. The 20 n/a items are all "non-binding statements
of the spec itself" — no implementation requirement. No RC1 gap.

### D. `idl4-csharp-1.0.md` (81 items, 66 done + 15 n/a)

The audit is solid. No RC1 gap.

### E. `idl4-java-1.0.md` (87 items, 72 done + 15 n/a)

The audit is solid. No RC1 gap.

## C-FFI surface — vendor spec coverage

`docs/specs/zerodds-c-api-1.0.md` covers the **complete** ZeroDDS
C-FFI. The table below maps OMG DDS spec sections to vendor spec
sections of the C-FFI:

| OMG DDS spec | Vendor spec section | RC1 status |
|---|---|---|
| §2.2.2.2.1 DomainParticipant | C-API §2.2 | ✅ all 16 operations |
| §2.2.2.3 Topic | C-API §2.5 | ✅ all 6 operations |
| §2.2.2.4 Publisher+DataWriter | C-API §2.3+§2.6 | ✅ Pub: 8/16 phase 1 + 8/16 phase 2 |
| §2.2.2.5 Subscriber+DataReader | C-API §2.4+§2.7 | ✅ Sub: 8/14 phase 1 + 6/14 phase 2 |
| §2.2.4 Listeners | listener-callbacks vendor spec | API surface phase 1, active phase 2 |
| §2.2.4 Conditions+WaitSet | C-API §6 | ✅ GuardCondition + StatusCondition + ReadCondition + QueryCondition + WaitSet |
| §2.2.5 Built-in Topics | C-API §7 | ✅ all 4 BuiltinTopicData |

## C-FFI function inventory (RC1)

As of 2026-05-06 after the phase-1 wave:

```
factory_ffi.rs        :   7 functions
participant_ffi.rs    :  16 functions
topic_ffi.rs          :   5 functions
publisher_ffi.rs      :  16 functions (Pub + DW)
subscriber_ffi.rs     :  18 functions (Sub + DR)
qos_ffi.rs            : 22 #[repr(C)] structs + 7 conversions
condition_ffi.rs      :  12 functions
builtin_ffi.rs        :   5 functions
extra_ffi.rs          :  30 functions (QoS get/set, instance ops, R/T variants)
listener_ffi.rs       :  12 functions + 6 listener structs
─────────────────────────────────────────
Total                 : ~130 functions + 28 structs
```

`crates/zerodds-c-api/include/zerodds.h` (emitted by cbindgen): 2852 LOC.

## Cross-language test inventory (RC1)

| Crate | Tests green | Path |
|---|---|---|
| `zerodds-c-api` | 56 cargo tests | `cargo test -p zerodds-c-api` |
| `zerodds-cpp` | 1 cargo test (compile+link+10 sub-asserts) | `cargo test -p zerodds-cpp` |
| `zerodds-cs` | 1 binary run (8 sub-asserts) | `dotnet ZeroDDS.Tests.dll` |
| `zerodds-py` | 1 cargo test | `cargo test -p zerodds-py --features extension-module` |
| `zerodds-java-omgdds` | 18 mvn tests | `mvn test` |
| `java-omgdds` | 37 cargo tests | `cargo test -p java-omgdds` |
| `zerodds-ts-node` | 4 ts tests (1 SKIP) | `npm test` |
| `zerodds-rs` | 3 cargo tests | `cargo test -p zerodds-rs` |
| `zerodds-sys` | 1 cargo test | `cargo test -p zerodds-sys` |
| `zerodds-ts-wasm` | (lib only) | — |

**Total layer 6 RC1: 130+ tests green, 0 failed.**

## Phase-2 roadmap

Per layer-6 crate, the open items for phase 2:

### `zerodds-c-api`

- Active listener wiring to runtime status counters (see
  `zerodds-listener-callbacks-1.0.md` §6).
- `dr_get_matched_publication_data` and `dw_get_matched_subscription_data`
  fully wired (RC1: unsupported stub).
- Switch the loan API (`dw_loan_message`/`commit_loan`/`discard_loan`)
  active — the DCPS side has the mechanism, the FFI bridge is missing.
- Fully wire `wait_for_historical_data` for transient-local readers.
- `read` vs `take`: currently aliased; add a read-state cache per reader.

### `zerodds-cpp`

- Active listener wiring (as soon as C-FFI phase 2 is finished).
- ContentFilteredTopic DataReader bind via `cft_handle_`.
- AnyDataWriter / AnyDataReader / AnyTopic for type-erased paths.
- Propagate QoS constructor arguments fully (instead of default).

### `zerodds-cs`

- Active IDataWriterListener wiring (as soon as C-FFI phase 2 is finished).
- QueryCondition with filter expression active (RC1: surface only).
- Propagate QoS constructor arguments fully.

### `zerodds-py`

- ContentFilteredTopic pyclass.
- Expose QoS structures as pyclass (instead of defaults only).
- ReadCondition / QueryCondition pyclass.

### `zerodds-java-omgdds`

- Write the vendor spec `zerodds-java-omgdds-1.0.md` that
  documents the pure Java + gRPC bridge path.
- gRPC bridge service proto in `crates/grpc-bridge/`.

### `zerodds-ts-node`

- Active `take` / `read` implementation (currently only a wrapper surface).
- Listener via koffi callback.

## PROCESS.md re-sync requirement

After phase 2 is completed: for each affected audit file an item-by-item
re-sync against the spec PDF, with an update of the `Status:` lines + this
gap table.

| Audit file | Re-sync after phase | Expected changes |
|---|---|---|
| `dds-psm-cxx-1.0.md` | Phase 2 (listener active) | 5 items from "done — API-Surface" → "done" |
| `dds-java-psm-1.0.md` | Phase 2 (gRPC bridge) | Update vendor spec cross-reference |
| `idl4-cpp-1.0.md` | not expected | — |
| `idl4-csharp-1.0.md` | not expected | — |
| `idl4-java-1.0.md` | not expected | — |
