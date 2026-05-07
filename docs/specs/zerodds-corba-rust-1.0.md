# zerodds-corba-rust 1.0 — IDL → Rust CORBA-Service Mapping

> **Vendor-Spec.** OMG CORBA 3.3 Annex-A definiert IDL-Mapping-Tabellen für C++/Java/etc., aber keine offizielle Rust-PSM. Diese Spec definiert wie `zerodds-corba-rust` IDL-CORBA-Service-Konstrukte auf Rust abbildet — analog zu OMG-IDL-CPP-Mapping.
>
> **Konformitäts-Träger:** OMG CORBA 3.3 (formal/2012-11-14), OMG IDL4 (formal/2018-07-01). Diese Spec ist ein Mapping ON TOP, kein Ersatz.

## §1 Scope

`zerodds-corba-rust` ist der Build-Zeit-Codegen für **CORBA-Service-Konstrukte** in Rust. Komplementär zu `zerodds-idl-rust` (DataType-Codegen):

- IDL `interface` → Rust trait + Client-Stub + Server-Skeleton-Dispatch
- IDL `valuetype` → Rust trait + ValueBase-Inheritance
- IDL `attribute` → trait-getter + setter (wenn writable)
- IDL `oneway op` → trait-method ohne Reply
- IDL `raises` → method-Return-Type ist `Result<T, CorbaException>`
- IDL `module` → `pub mod`

**Out-of-Scope (Phase 2+):**
- IDL `component` / `home` (CCM) — geht in Phase-2 mit `corba-ccm-lib`-Binding
- POA-Configuration-Builder — Phase-2
- GIOP-Wire-Wiring im Stub/Skeleton — Phase-1 emittiert `NotYetWired`-Stubs
- User-Exception-Codegen pro Interface — Phase-2

## §2 Type-Mapping

CORBA-Service-Konstrukte nutzen die `zerodds-idl-rust`-Type-Map für DataType-Felder + die folgenden Service-spezifischen Mappings:

| IDL | Rust | Quelle |
|---|---|---|
| primitive types | siehe `zerodds-idl-rust-1.0` §2.1 | DataType |
| `string`, `wstring` | `String` | DataType |
| `sequence<T>`, `T[N]`, `Optional<T>` | siehe `zerodds-idl-rust-1.0` §2.3 | DataType |
| `Object` | `zerodds_corba_rust::ObjectReference` | Service |
| `ValueBase` | `dyn zerodds_corba_rust::ValueBase` | Service |
| `TypeCode` | (Phase-2: `zerodds_corba_rust::TypeCode`) | Service |
| `interface I` | `dyn I` (trait-object) wenn als Param-Type; sonst `IStub` | Service |

## §3 Interface-Mapping

Pro `interface I { ... };`:

```rust
// Trait — den Servants impl-en und Stubs gemeinsam nutzen.
pub trait I: Send + Sync {
    fn op1(&self, ...) -> Result<T, CorbaException>;
    // attribute getters/setters, oneway ohne Reply, etc.
}

// Client-Stub — sendet GIOP-Requests.
pub struct IStub { pub object_ref: ObjectReference }
impl IStub { pub fn new(object_ref: ObjectReference) -> Self { ... } }
impl I for IStub { /* delegiert an GIOP-Marshalling */ }

// Server-Skeleton-Dispatch — POA ruft das mit Op-Name + Payload.
pub fn dispatch_<i_lower>(
    servant: &dyn I,
    operation: &str,
    payload: &[u8],
) -> SkeletonResult { /* match operation { "op1" => ..., _ => BadOperation } */ }
```

### §3.1 Operation-Mapping

| IDL | Rust |
|---|---|
| `void op();` | `fn op(&self) -> Result<(), CorbaException>` |
| `T op();` | `fn op(&self) -> Result<T, CorbaException>` |
| `op(in T x);` | `fn op(&self, x: T) -> ...` |
| `op(out T x);` | `fn op(&mut self, x: &mut T) -> ...` |
| `op(inout T x);` | `fn op(&mut self, x: &mut T) -> ...` |
| `oneway op(...);` | `fn op(...) -> Result<(), CorbaException>` (Stub fire-and-forget) |
| `raises (E1, E2)` | (Phase 2: spezifischer Error-Enum statt `CorbaException`) |

`&self` vs `&mut self` richtet sich rein nach out/inout-Param-Anwesenheit. `in`-only-Operations sind `&self`.

### §3.2 Attribute-Mapping

```idl
interface I {
    readonly attribute long count;
    attribute string label;
};
```

→

```rust
pub trait I: Send + Sync {
    fn count(&self) -> Result<i32, CorbaException>;
    fn label(&self) -> Result<String, CorbaException>;
    fn set_label(&mut self, value: String) -> Result<(), CorbaException>;
}
```

`readonly` lässt den setter weg.

## §4 Valuetype-Mapping

Pro `valuetype V { ... };`:

```rust
pub trait V: ValueBase + Send + Sync {
    // public state-members als getter
    fn x(&self) -> i32;
    // private state-members mit `_priv_`-Prefix (nur für Wire-Marshalling-Helfer)
    fn _priv_y(&self) -> String;
    // operations + attributes wie bei interface
}
```

`ValueBase` ist Runtime-Trait mit `repository_id(&self) -> &str`. Concrete Implementations setzen `IDL:omg.org/V:1.0`-Format-IDs.

## §5 Runtime-API

```rust
// Object-Reference (IOR-encoded).
pub struct ObjectReference {
    pub type_id: String,           // z.B. "IDL:omg.org/MyInterface:1.0"
    pub iiop_profile: Vec<u8>,     // CDR-encoded IIOP-Profile (Phase-1 opaque)
}

// Exception-Familie.
pub enum CorbaException {
    SystemException { minor: u32, message: &'static str },  // Spec §3.17.1
    UserException { repository_id: String, payload: Vec<u8> },
}

// Skeleton-Dispatch-Result.
pub enum SkeletonResult {
    Reply(Vec<u8>),
    Exception(CorbaException),
    BadOperation,
    NotYetWired,
}

// ValueBase-Trait.
pub trait ValueBase {
    fn repository_id(&self) -> &str;
}

// POA-Servant-Marker.
pub trait Servant {
    fn target_repository_id(&self) -> &str;
}
```

## §6 Repository-ID-Format

CORBA 3.3 §10.7.3.1 standardisiert das Repository-ID-Format als `IDL:<scope>/<TypeName>:<major>.<minor>`. Der Codegen emittiert keine `const REPOSITORY_ID`-Konstante per Type in Phase-1 — Phase-2 fügt die hinzu (analog zu wie `zerodds_dcps::DdsType::TYPE_NAME` für DataTypes).

## §7 Konformitäts-Tests

Tests liegen in `crates/corba-rust/tests/`:

- `snapshot_codegen.rs` — Snapshot-Tests mit insta. Pro Test ein IDL-Snippet → Rust-Output committed unter `tests/snapshots/`.
- `compile_check.rs` (Phase 2) — emittierten Code real kompilieren gegen `zerodds-corba-rust` Pfad-Dep.
- `wire_giop.rs` (Phase 2) — Roundtrip-Tests mit echten GIOP-Bytes via `corba-giop`.
