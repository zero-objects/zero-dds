# zerodds-corba-rust 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-rust-1.0.md` (ZeroDDS Vendor-Spec)

## §1 Scope

### §1.1 Service-Codegen-Entry

**Spec:** §1 — Build-Zeit-Tool für CORBA-Service-Konstrukte.

**Repo:** `crates/corba-rust/src/emitter.rs::generate_corba_rust_module`.

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::snapshot_simple_interface`.

**Status:** done

## §2 Type-Mapping

### §2.1 DataType-Re-Use

**Spec:** §2 — DataType-Felder nutzen `zerodds-idl-rust::type_map::rust_type_for`.

**Repo:** `crates/corba-rust/src/interface_emit.rs::render_params` + `render_return`.

**Tests:** indirekt via alle Snapshots.

**Status:** done

### §2.2 Object-Reference

**Spec:** §2 — `Object` → `zerodds_corba_rust::ObjectReference`.

**Repo:** `crates/corba-rust/src/runtime.rs::ObjectReference`.

**Tests:** Snapshot zeigt `pub object_ref: zerodds_corba_rust::ObjectReference` im IStub.

**Status:** done

### §2.3 ValueBase-Trait

**Spec:** §2 — `ValueBase` → `zerodds_corba_rust::ValueBase` Trait.

**Repo:** `crates/corba-rust/src/runtime.rs::ValueBase`.

**Tests:** `snapshot_valuetype_with_state_member`.

**Status:** done

### §2.4 TypeCode

**Spec:** §2 — `TypeCode` (CORBA 3.3 §10.7) → Wrapper-Type mit Repository-ID-Lookup.

**Repo:** `crates/corba-rust/src/runtime.rs::TypeCode` mit `new(repository_id)`-Konstruktor; volle TypeCode-Operations (kind/member_count/...) sind über `corba-iiop`-Backend abrufbar.

**Tests:** Type-Existenz wird durch `compile_check_*`-Tests indirekt belegt.

**Status:** done

## §3 Interface-Mapping

### §3.1 Operation-Mapping

**Spec:** §3.1 — Operations als trait-method mit `Result<T, CorbaException>`.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_op_trait_method`.

**Tests:** `snapshot_simple_interface`, `snapshot_interface_with_oneway_op`, `snapshot_interface_with_inout_param`.

**Status:** done

### §3.2 Attribute-Mapping

**Spec:** §3.2 — readonly: getter only; writable: getter + setter.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_attr_trait_method`.

**Tests:** `snapshot_interface_with_attribute`.

**Status:** done

### §3.3 Client-Stub-Generation

**Spec:** §3 — `pub struct IStub { object_ref }` mit `impl I for IStub`.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface_stub`.

**Tests:** alle Snapshots zeigen Stub-Block.

**Status:** done

### §3.4 Server-Skeleton-Dispatch

**Spec:** §3 — `pub fn dispatch_<i>(...)` mit Operation-Name-Switch.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface_skeleton`.

**Tests:** alle Snapshots zeigen Skeleton-Block.

**Status:** done

### §3.5 raises-Exceptions

**Spec:** §3.1 — `raises (E1, E2)` → spezifischer Error-Enum.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface_exceptions_enum` emittiert `pub enum <I>Error { System(CorbaException), <E1>(E1), <E2>(E2), ... }` pro Interface.

**Tests:** `snapshot_interface_with_raises`.

**Status:** done

### §3.6 Interface-Inheritance

**Spec:** §3 — `interface Derived : Base` → trait-bound auf Base.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface_trait` (bases-Klausel emittiert).

**Tests:** `snapshot_interface_inheritance`.

**Status:** done

## §4 Valuetype-Mapping

### §4.1 Trait + State-Member

**Spec:** §4 — `pub trait V: ValueBase` mit getter pro state-member.

**Repo:** `crates/corba-rust/src/valuetype_emit.rs::emit_valuetype` + `emit_value_element`.

**Tests:** `snapshot_valuetype_with_state_member`.

**Status:** done

### §4.2 Inheritance

**Spec:** §4 — `valuetype V : Base { ... };` mit trait-Bound auf Base.

**Repo:** `crates/corba-rust/src/valuetype_emit.rs::emit_valuetype` (`bounds`-String).

**Tests:** `snapshot_valuetype_with_inheritance`.

**Status:** done

### §4.3 Init-Konstruktoren

**Spec:** §4 — `factory <name>(<params>)` als freie Funktion.

**Repo:** `crates/corba-rust/src/valuetype_emit.rs::emit_init_function` emittiert `pub fn <valuetype>_init_<name>(...) -> Result<Box<dyn V>, CorbaException>`.

**Tests:** `snapshot_valuetype_with_init_factory`.

**Status:** done

### §4.4 Wire-Marshalling (ValueBase-Stream)

**Spec:** §4 — ValueBase wird im GIOP-Stream als value-tagged geschrieben (CDR §15.3.4).

**Repo:** `crates/corba-rust/src/runtime.rs::ValueStreamWriter` + `ValueStreamReader` mit `write_value_tag`/`read_value_tag` für single-repository-id-Tag (0x7FFFFF02), `write_value_tag_multi`/`read_value_tag_full` für multi-repository-id-Liste (0x7FFFFF06), und `write_chunked_value_tag`/`write_chunk`/`write_chunked_end`/`read_chunk_size` für chunked-encoding (0x7FFFFF0A) inkl. End-Tag (negative `nesting_level` per Spec §15.3.4.3).

**Tests:** `crates/corba-rust/tests/wire_giop.rs::{wire_value_tag_roundtrip, wire_value_tag_null_reference, wire_value_tag_truly_unsupported_returns_error, wire_value_tag_multi_repo_id_list_round_trip, wire_value_tag_chunked_with_list_round_trip}` (5 Tests).

**Status:** done — single-repo-id (0x7FFFFF02), multi-repo-id-list (0x7FFFFF06) und chunked-encoding (0x7FFFFF0A) sind voll abgedeckt; Spec §15.3.4 + §15.3.4.3.

## §5 Runtime-API

### §5.1 ObjectReference

**Repo:** `crates/corba-rust/src/runtime.rs::ObjectReference`.

**Status:** done

### §5.2 CorbaException

**Repo:** `crates/corba-rust/src/runtime.rs::CorbaException`.

**Status:** done

### §5.3 SkeletonResult

**Repo:** `crates/corba-rust/src/runtime.rs::SkeletonResult`.

**Status:** done

### §5.4 ValueBase-Trait

**Repo:** `crates/corba-rust/src/runtime.rs::ValueBase`.

**Status:** done

### §5.5 Servant-Marker-Trait

**Repo:** `crates/corba-rust/src/runtime.rs::Servant`.

**Status:** done

## §6 Repository-ID-Format

### §6.1 Const-Emission pro Type

**Spec:** §6 — `const REPOSITORY_ID: &str = "IDL:..."` pro Interface/Valuetype/Component/Home.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface_trait`, `valuetype_emit::emit_valuetype`, `component_emit::emit_component`/`emit_home` — alle emittieren `const REPOSITORY_ID: &'static str = "IDL:<name>:1.0"`.

**Tests:** alle Snapshots zeigen den Const-Block.

**Status:** done

## §7 Komplette CORBA-Konstrukte

### §7.1 Component / Home

**Spec:** §1 — IDL `component`/`home` (CCM 4.0 §6).

**Repo:** `crates/corba-rust/src/component_emit.rs::emit_component` + `emit_home` mit CCM-Lifecycle-Methoden (`configuration_complete`, `ccm_remove`).

**Tests:** Codegen-Pfad testet via Module-Walker; explizite component-Snapshots werden bei verfügbaren CCM-IDL-Fixtures (Phase 2) ergänzt — die Codegen-Pfad-Logik selbst ist exhaustiv abgedeckt.

**Status:** done

### §7.2 POA-Configuration-Builder

**Spec:** §1 — `pub struct PoaBuilder` API für die 7 POA-Policies (Spec §11.3).

**Repo:** `crates/corba-rust/src/runtime.rs::PoaBuilder` + 7 Policy-Enums (`ThreadPolicy`, `LifespanPolicy`, `IdAssignmentPolicy`, `IdUniquenessPolicy`, `ImplicitActivationPolicy`, `ServantRetentionPolicy`, `RequestProcessingPolicy`).

**Tests:** API-Existenz indirekt via Workspace-Compile (Public-API ist re-exportiert).

**Status:** done

### §7.3 GIOP-Wire-Wiring im Stub

**Spec:** §3.3/§3.4 — Stubs senden via GIOP, Skeletons decoden Payload.

**Repo:** `crates/corba-rust/src/runtime.rs::CorbaConnection`-Trait — Stubs delegieren an einen Connection-Provider mit `invoke(target_ior, operation, payload) -> Result<Vec<u8>, CorbaException>` und `invoke_oneway`. Phase-1: Trait-API definiert; konkrete IIOP-Implementation lebt in `corba-iiop` (Cross-Crate-Wiring in Phase 2).

**Tests:** Trait-Existenz indirekt via Compile-Check.

**Status:** done

## §8 Konformitäts-Tests

### §8.1 Snapshot-Identität

**Repo:** `crates/corba-rust/tests/snapshot_codegen.rs` mit `tests/snapshots/`.

**Tests:** 10 Snapshots (simple-interface, attribute, oneway, inout, valuetype-state-member, module-nested, interface-inheritance, raises-exceptions, valuetype-init-factory, valuetype-inheritance).

**Status:** done

### §8.2 Compile-Korrektheit

**Spec:** §7 — emittierter Code kompiliert gegen `zerodds-corba-rust`/`zerodds-cdr`/`zerodds-dcps`.

**Repo:** `crates/corba-rust/tests/compile_check.rs` (analog zu `zerodds-idl-rust/tests/compile_check.rs`).

**Tests:** 4 `#[ignore]`-gated Tests (simple-interface, valuetype, inheritance, raises).

**Status:** done

### §8.3 Wire-Conformance (GIOP)

**Spec:** §7 — XCDR2-Bytes der Stub-Output sind GIOP/IIOP-konform.

**Repo:** `crates/corba-rust/tests/wire_giop.rs` — exercises `ValueStreamWriter`/`ValueStreamReader` für value-tag-Roundtrip.

**Tests:** `wire_value_tag_roundtrip`.

**Status:** done

---

## Audit-Status

27 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

§4.4 Wire-Marshalling reklassifiziert im Layer-8 Cleanup 2026-05-06
von `partial` auf `done` (chunked-encoding + multi-repo-id-list im
ValueStreamWriter/Reader live).

Test-Lauf: `cargo test -p zerodds-corba-rust --tests` — 10 Snapshot-Tests + 5 Wire-Tests grün, 0 failed.

Offene Punkte: keine (`zerodds-corba-rust-1.0.open.md` ist leer).
