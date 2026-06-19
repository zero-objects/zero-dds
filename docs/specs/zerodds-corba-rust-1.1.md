# zerodds-corba-rust 1.1 — IDL → Rust CORBA-Service Mapping

> **Vendor-Spec.** OMG CORBA 3.3 Annex-A definiert IDL-Mapping-Tabellen für
> C++/Java/etc., aber kein offizielles Rust-PSM. Diese Spec definiert, wie
> `zerodds-corba-rust` IDL-CORBA-Konstrukte auf Rust abbildet — analog zum
> OMG-IDL-CPP-Mapping.
>
> **Konformitäts-Träger:** OMG CORBA 3.3 (formal/2012-11-14), OMG IDL4
> (formal/2018-07-01), CORBA Messaging (§22), Bidirectional GIOP (§15.8),
> Valuetype (§15.3.4), Portable Interceptors (§16). Diese Spec ist ein Mapping ON
> TOP, kein Ersatz.
>
> **Wire-Status.** Die generierten Stubs marshallen echte GIOP-Requests über
> IIOP und interoperieren live mit JacORB, omniORB und TAO.

## §1 Scope

`zerodds-corba-rust` ist der Build-Zeit-Codegen für **CORBA-Service-Konstrukte**
in Rust, komplementär zu `zerodds-idl-rust` (DataType-Codegen):

- IDL `interface` → Rust trait + Client-Stub + Server-Skeleton-Dispatch (voll
  GIOP-verdrahtet)
- IDL `valuetype` → trait + `<Name>Value`-State-Carrier mit Wire-Marshalling (§4)
- IDL `attribute` → trait-getter + setter (wenn writable)
- IDL `oneway op` → trait-method ohne Reply
- IDL `raises (E)` → method-Return ist `Result<T, IError>` mit typisiertem
  Exception-Enum pro Interface
- IDL `@ami` → AMI-Stubs (Callback + Polling, §5)
- IDL `module` → `pub mod`

**Laufzeit-Ergänzungen** (Runtime-API, §9): AMH (§6), Bidirectional GIOP (§7),
Portable Interceptors (§8).

## §2 Type-Mapping

| IDL | Rust | Quelle |
|---|---|---|
| primitive / `string` / `wstring` / `sequence` / arrays | siehe `zerodds-idl-rust-1.0` | DataType |
| `Object` | `zerodds_corba_rust::ObjectReference` | Service |
| `ValueBase` | `dyn zerodds_corba_rust::ValueBase` / `<Name>Value` | Service |
| `interface I` | `dyn I` (als Param-Type) bzw. `IStub` (Client) | Service |

## §3 Interface-Mapping

Pro `interface I { ... };` werden ein Trait, ein `IStub` (Client) und
`dispatch_i` (Server-Skeleton) emittiert. Der Stub marshallt In-Args in den
GIOP-Request-Body, ruft über `CorbaConnection::invoke`, und dekodiert Return +
out/inout aus dem Reply (gleiche Reihenfolge).

### §3.1 Operation-Mapping

| IDL | Rust |
|---|---|
| `void op();` | `fn op(&self) -> Result<(), CorbaException>` |
| `T op(in U x);` | `fn op(&self, x: U) -> Result<T, CorbaException>` |
| `op(out T x);` / `op(inout T x);` | `fn op(&self, x: &mut T) -> ...` |
| `oneway op(...);` | Stub fire-and-forget (`invoke_oneway`) |
| `raises (E1, E2)` | Return ist `Result<T, IError>`; `IError` ist ein pro-Interface-Enum mit `System(CorbaException)` + je raised Exception einer typisierten Variante |

Der Stub löst eine `UserException` anhand der RepositoryId in die richtige
`IError`-Variante auf (scope-korrekte RepoId, §10).

### §3.2 Attribute-Mapping

`readonly attribute T a;` → `fn a(&self) -> Result<T, CorbaException>`;
`attribute T b;` zusätzlich `fn set_b(&self, value: T) -> ...`.

## §4 Valuetype-Mapping (§15.3.4)

Pro `valuetype V` werden emittiert: das polymorphe Trait `V: ValueBase`, eine
konkrete, **wire-marshallbare** `VValue`-Struct, und eine Factory-Registrierung.

### §4.1 State-Carrier

```rust
pub struct VValue { pub x: i32, pub y: i32, /* alle State-Member, Basis-zuerst */ }
impl ValueBase for VValue { fn repository_id(&self) -> &str { V_REPOSITORY_ID } }
impl ValueMarshal for VValue {
    fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> { /* Decl-Reihenfolge */ }
}
pub fn register_v_value(reg: &mut ValueRegistry); // RepositoryId → State-Reader-Factory
```

Inheritance-State wird **abgeflacht**: Basis-State zuerst, dann abgeleiteter
(§15.3.4). Ist `valuetype D : truncatable B` oder `custom valuetype`, wird
zusätzlich `D_BASE_IDS: &[&str]` (transitive Basis-RepositoryIds) emittiert.

### §4.2 Value-Wire-Engine

`ValueWriter`/`ValueReader`/`ValueRegistry` implementieren §15.3.4:

- **value_tag** (§15.3.4.1, gegen JacORB `CDRInputStream` verifiziert): `0x00000000`
  null, `0xffffffff` Indirection (Offset rel. zum Offset-Feld), `0x7fffff00..ff`
  Value (Bit 0 codebase, Bit 3 chunked, Bits 1-2 RepositoryId-Type-Info).
- **Value-Sharing**: dieselbe `Rc`-Instanz beim zweiten Schreiben → Indirection;
  ein Indirection-Decode liefert dieselbe `Rc`-Instanz (DAG erhalten).
- **chunked encoding + Truncation** (§15.3.4.3): `write_chunked` schreibt
  value_tag `0x7fffff0e` (chunked + RepositoryId-Liste) + Chunk + End-Tag `-1`;
  der Decoder wählt die erste bekannte RepositoryId der Liste (most-derived
  zuerst) und überspringt bei Truncation den abgeleiteten State-Rest über die
  Chunk-Größen bis zum End-Tag. **Custom-Marshalling** läuft über denselben
  chunked-Pfad (`marshal_state` = beliebige Logik).

**Byte-Konformität (normativ).** `Point(42,-7)` mit `IDL:Point:1.0` MUSS Big-
Endian zu `7fffff02 0000000e "IDL:Point:1.0\0" <pad> 0000002a fffffff9`
encodieren — byte-identisch zu JacORB 3.9 `write_value`. Der truncatable
`Derived(42,"hi") : truncatable Base` MUSS chunked
(`7fffff0e …repo-list… 0000000b 0000002a 00000003 "hi\0" ffffffff`) encodieren.

## §5 Asynchronous Method Invocation (§22)

Für ein `@ami`-Interface emittiert der Codegen das implied-IDL-AMI-Mapping:

- `<Iface>AmiHandler`-Trait (Callback-Modell §22.5): pro Operation `<op>`
  (Return + out/inout-Args) + `<op>_excep(CorbaException)`.
- `sendc_<op>(channel, handler, in-args…)` auf dem Stub.
- `sendp_<op>(channel, in-args…) -> <Iface><Op>Poller` (Polling-Modell §22.6) +
  `Poller::get_reply(channel)` mit typisiertem Return.

Beide laufen über `AsyncCorbaChannel` (§9), das der Transport (`AmiClient`)
implementiert — gleiches Layering wie `CorbaConnection` für die synchrone Seite.

## §6 Asynchronous Method Handling (§22.9)

Server-seitig erlaubt `AmhEndpoint` einen **verzögerten** Reply: `accept_request`
liefert den Request samt `AmhResponseHandler`; der Servant antwortet später mit
`send_reply`/`send_exception`. Mehrere Requests dürfen gleichzeitig geparkt und
in beliebiger Reihenfolge beantwortet werden.

## §7 Bidirectional GIOP (§15.8)

`BiDirEndpoint` ist ein Peer über EINER Connection, der Requests sendet UND
eingehende Requests des Gegenübers bedient — der Server kann über die client-
geöffnete Connection zurückrufen. `request_id`-Parität (Originator gerade,
Acceptor ungerade) verhindert Kollision; Listen-Points werden im
`BiDirIIOPServiceContext` (Tag 5) annonciert. Server-seitig extrahiert der
Dispatch den Object-Key aus `KeyAddr`/`ProfileAddr`/`ReferenceAddr` (§15.4.2).

## §8 Portable Interceptors (§16)

`RequestInfo` trägt `request_id`/`operation` + Request-/Reply-ServiceContext-
Listen (`add/get_request_service_context`, `add/get_reply_service_context`) +
`forward_reference`. `ClientRequestInterceptor`/`ServerRequestInterceptor` haben
die benannten Points (`send_request`/`receive_reply`/… bzw.
`receive_request_service_contexts`/`send_reply`/…). `ServiceContextInjector` ist
die spec-saubere Art, OTS- (id 0), CSIv2- (id 15) oder Codeset-Kontexte
beizulegen — als registrierter Interceptor statt im Transport hartcodiert.

## §9 Runtime-API

```rust
pub struct ObjectReference { pub type_id: String, pub iiop_profile: Vec<u8> }

pub enum CorbaException {
    SystemException { minor: u32, message: &'static str },
    UserException { repository_id: String, body: Vec<u8>, endianness: Endianness },
}

pub enum SkeletonResult { Reply(Vec<u8>), Exception(CorbaException), BadOperation }

pub trait CorbaConnection {            // synchron
    fn invoke(&self, target, op, e, payload) -> Result<(Vec<u8>, Endianness), CorbaException>;
    fn invoke_oneway(&self, target, op, e, payload) -> Result<(), CorbaException>;
}

pub trait AsyncCorbaChannel {          // asynchron (AMI, §5)
    fn send(&mut self, op, payload, cb) -> Result<u32, CorbaException>;
    fn send_poll(&mut self, op, payload) -> Result<u32, CorbaException>;
    fn get_reply(&mut self, request_id) -> Result<AsyncReply, CorbaException>;
    fn perform_work(&mut self) -> Result<u32, CorbaException>;
    fn perform_all(&mut self) -> Result<(), CorbaException>;
}

pub trait ValueBase { fn repository_id(&self) -> &str; }
pub trait ValueMarshal: ValueBase { fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError>; }
```

Es gibt keinen `SkeletonResult::NotYetWired`-Stub — der Wire ist voll verdrahtet.
Transport-Implementierungen (`IiopCorbaConnection`, `AmiClient`, `AmhEndpoint`,
`BiDirEndpoint`) liegen in `zerodds-corba-interop`.

## §10 Repository-ID-Format

CORBA 3.3 §10.7.3.1: `IDL:<scope>/<TypeName>:<major>.<minor>`. Der Codegen
emittiert pro Interface/Valuetype eine `*_REPOSITORY_ID`-Konstante,
**scope-korrekt** (Modul-Pfad durchgereicht, inkl. `typeprefix`).

## §11 Konformität

Ein **konformer** Codegen + Runtime:

1. emittiert Stubs, die echte GIOP-Requests marshallen (kein `NotYetWired`),
2. mappt `raises` auf typisierte `IError`-Enums (§3.1),
3. emittiert wire-marshallbare Valuetype-State-Carrier + die value_wire-Engine
   mit Sharing/Chunking/Truncation byte-konform zu §4.2,
4. emittiert AMI-Stubs für `@ami` (§5),
5. liefert AMH, Bidirectional GIOP und Portable Interceptors gemäß §6–§8,
6. encodiert RepositoryIds scope-korrekt (§10).

## §12 Implementierungs-Mapping

| Spec | Code |
|---|---|
| §3 Interface | `corba-rust/src/interface_emit.rs` |
| §4 Valuetype-Engine | `corba-rust/src/value_wire.rs`; Codegen `corba-rust/src/valuetype_emit.rs` |
| §5 AMI-Codegen | `corba-rust/src/ami_emit.rs`; `AsyncCorbaChannel` in `corba-rust/src/runtime.rs` |
| §6 AMH | `corba-interop/src/runtime.rs` — `AmhEndpoint`, `AmhResponseHandler` |
| §7 Bidirectional | `corba-interop/src/runtime.rs` — `BiDirEndpoint` |
| §8 Interceptors | `corba-ccm/src/orb_extensions.rs` — `RequestInfo`, `InterceptorRegistry` |
| §9 Runtime | `corba-rust/src/runtime.rs`; Transport `corba-interop/src/runtime.rs` |

## §13 Konformitäts-Tests

- `snapshot_codegen.rs` — insta-Snapshots pro IDL-Snippet (Interface/Valuetype/AMI).
- `compile_check.rs` — emittierter Code real kompiliert gegen Pfad-Deps
  (Interface, Valuetype + Inheritance, AMI).
- `value_wire`-Unit — byte-exakte Tests inkl. JacORB-Capture (single/chunked/
  truncated, BE+LE).
- `corba-interop/tests/` — GIOP-e2e über echtes IIOP: valuetype, AMI, AMH,
  bidir; **cross-ORB live** gegen JacORB (valuetype-echo, AMI, CSIv2, BiDir) und
  byte-konform gegen omniORB (BiDir-SC).
