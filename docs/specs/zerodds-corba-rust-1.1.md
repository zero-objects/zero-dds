# zerodds-corba-rust 1.1 — IDL → Rust CORBA service mapping

> **Vendor spec.** OMG CORBA 3.3 Annex A defines IDL mapping tables for
> C++/Java/etc., but no official Rust PSM. This spec defines how
> `zerodds-corba-rust` maps IDL CORBA constructs to Rust — analogous to the
> OMG IDL CPP mapping.
>
> **Conformance basis:** OMG CORBA 3.3 (formal/2012-11-14), OMG IDL4
> (formal/2018-07-01), CORBA Messaging (§22), Bidirectional GIOP (§15.8),
> Valuetype (§15.3.4), Portable Interceptors (§16). This spec is a mapping ON
> TOP, not a replacement.
>
> **Wire status.** The generated stubs marshal real GIOP requests over
> IIOP and interoperate live with JacORB, omniORB and TAO.

## §1 Scope

`zerodds-corba-rust` is the build-time codegen for **CORBA service constructs**
in Rust, complementary to `zerodds-idl-rust` (data type codegen):

- IDL `interface` → Rust trait + client stub + server skeleton dispatch (fully
  wired to GIOP)
- IDL `valuetype` → trait + `<Name>Value` state carrier with wire marshalling (§4)
- IDL `attribute` → trait getter + setter (if writable)
- IDL `oneway op` → trait method without reply
- IDL `raises (E)` → the method return is `Result<T, IError>` with a typed
  exception enum per interface
- IDL `@ami` → AMI stubs (callback + polling, §5)
- IDL `module` → `pub mod`

**Runtime additions** (runtime API, §9): AMH (§6), Bidirectional GIOP (§7),
Portable Interceptors (§8).

## §2 Type mapping

| IDL | Rust | Source |
|---|---|---|
| primitive / `string` / `wstring` / `sequence` / arrays | see `zerodds-idl-rust-1.0` | DataType |
| `Object` | `zerodds_corba_rust::ObjectReference` | Service |
| `ValueBase` | `dyn zerodds_corba_rust::ValueBase` / `<Name>Value` | Service |
| `interface I` | `dyn I` (as param type) or `IStub` (client) | Service |

## §3 Interface mapping

For each `interface I { ... };` a trait, an `IStub` (client) and
`dispatch_i` (server skeleton) are emitted. The stub marshals in-args into the
GIOP request body, calls via `CorbaConnection::invoke`, and decodes return +
out/inout from the reply (same order).

### §3.1 Operation mapping

| IDL | Rust |
|---|---|
| `void op();` | `fn op(&self) -> Result<(), CorbaException>` |
| `T op(in U x);` | `fn op(&self, x: U) -> Result<T, CorbaException>` |
| `op(out T x);` / `op(inout T x);` | `fn op(&self, x: &mut T) -> ...` |
| `oneway op(...);` | stub fire-and-forget (`invoke_oneway`) |
| `raises (E1, E2)` | the return is `Result<T, IError>`; `IError` is a per-interface enum with `System(CorbaException)` + one typed variant per raised exception |

The stub resolves a `UserException` by its RepositoryId into the correct
`IError` variant (scope-correct RepoId, §10).

### §3.2 Attribute mapping

`readonly attribute T a;` → `fn a(&self) -> Result<T, CorbaException>`;
`attribute T b;` additionally `fn set_b(&self, value: T) -> ...`.

## §4 Valuetype mapping (§15.3.4)

For each `valuetype V` the following are emitted: the polymorphic trait `V: ValueBase`, a
concrete, **wire-marshallable** `VValue` struct, and a factory registration.

### §4.1 State carrier

```rust
pub struct VValue { pub x: i32, pub y: i32, /* all state members, base first */ }
impl ValueBase for VValue { fn repository_id(&self) -> &str { V_REPOSITORY_ID } }
impl ValueMarshal for VValue {
    fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError> { /* declaration order */ }
}
pub fn register_v_value(reg: &mut ValueRegistry); // RepositoryId → state reader factory
```

Inheritance state is **flattened**: base state first, then the derived one
(§15.3.4). If it is `valuetype D : truncatable B` or `custom valuetype`,
`D_BASE_IDS: &[&str]` (transitive base RepositoryIds) is emitted in addition.

### §4.2 Value wire engine

`ValueWriter`/`ValueReader`/`ValueRegistry` implement §15.3.4:

- **value_tag** (§15.3.4.1, verified against JacORB `CDRInputStream`): `0x00000000`
  null, `0xffffffff` indirection (offset relative to the offset field), `0x7fffff00..ff`
  value (bit 0 codebase, bit 3 chunked, bits 1-2 RepositoryId type info).
- **Value sharing**: the same `Rc` instance on the second write → indirection;
  an indirection decode returns the same `Rc` instance (DAG preserved).
- **chunked encoding + truncation** (§15.3.4.3): `write_chunked` writes
  value_tag `0x7fffff0e` (chunked + RepositoryId list) + chunk + end tag `-1`;
  the decoder picks the first known RepositoryId of the list (most-derived
  first) and on truncation skips the rest of the derived state via the
  chunk sizes up to the end tag. **Custom marshalling** runs over the same
  chunked path (`marshal_state` = arbitrary logic).

**Byte conformance (normative).** `Point(42,-7)` with `IDL:Point:1.0` MUST encode
big-endian to `7fffff02 0000000e "IDL:Point:1.0\0" <pad> 0000002a fffffff9`
— byte-identical to JacORB 3.9 `write_value`. The truncatable
`Derived(42,"hi") : truncatable Base` MUST encode chunked
(`7fffff0e …repo-list… 0000000b 0000002a 00000003 "hi\0" ffffffff`).

## §5 Asynchronous Method Invocation (§22)

For an `@ami` interface, the codegen emits the implied-IDL AMI mapping:

- `<Iface>AmiHandler` trait (callback model §22.5): per operation `<op>`
  (return + out/inout args) + `<op>_excep(CorbaException)`.
- `sendc_<op>(channel, handler, in-args…)` on the stub.
- `sendp_<op>(channel, in-args…) -> <Iface><Op>Poller` (polling model §22.6) +
  `Poller::get_reply(channel)` with a typed return.

Both run over `AsyncCorbaChannel` (§9), which the transport (`AmiClient`)
implements — the same layering as `CorbaConnection` for the synchronous side.

## §6 Asynchronous Method Handling (§22.9)

On the server side, `AmhEndpoint` allows a **deferred** reply: `accept_request`
returns the request together with an `AmhResponseHandler`; the servant answers later with
`send_reply`/`send_exception`. Several requests may be parked at the same time and
answered in any order.

## §7 Bidirectional GIOP (§15.8)

`BiDirEndpoint` is a peer over ONE connection that sends requests AND
serves incoming requests of the counterpart — the server can call back over the
connection opened by the client. `request_id` parity (originator even,
acceptor odd) prevents collisions; listen points are announced in the
`BiDirIIOPServiceContext` (tag 5). On the server side, the
dispatch extracts the object key from `KeyAddr`/`ProfileAddr`/`ReferenceAddr` (§15.4.2).

## §8 Portable Interceptors (§16)

`RequestInfo` carries `request_id`/`operation` + request/reply ServiceContext
lists (`add/get_request_service_context`, `add/get_reply_service_context`) +
`forward_reference`. `ClientRequestInterceptor`/`ServerRequestInterceptor` have
the named points (`send_request`/`receive_reply`/… or
`receive_request_service_contexts`/`send_reply`/…). `ServiceContextInjector` is
the spec-clean way to attach OTS (id 0), CSIv2 (id 15) or codeset contexts
— as a registered interceptor instead of hard-coded in the transport.

## §9 Runtime API

```rust
pub struct ObjectReference { pub type_id: String, pub iiop_profile: Vec<u8> }

pub enum CorbaException {
    SystemException { minor: u32, message: &'static str },
    UserException { repository_id: String, body: Vec<u8>, endianness: Endianness },
}

pub enum SkeletonResult { Reply(Vec<u8>), Exception(CorbaException), BadOperation }

pub trait CorbaConnection {            // synchronous
    fn invoke(&self, target, op, e, payload) -> Result<(Vec<u8>, Endianness), CorbaException>;
    fn invoke_oneway(&self, target, op, e, payload) -> Result<(), CorbaException>;
}

pub trait AsyncCorbaChannel {          // asynchronous (AMI, §5)
    fn send(&mut self, op, payload, cb) -> Result<u32, CorbaException>;
    fn send_poll(&mut self, op, payload) -> Result<u32, CorbaException>;
    fn get_reply(&mut self, request_id) -> Result<AsyncReply, CorbaException>;
    fn perform_work(&mut self) -> Result<u32, CorbaException>;
    fn perform_all(&mut self) -> Result<(), CorbaException>;
}

pub trait ValueBase { fn repository_id(&self) -> &str; }
pub trait ValueMarshal: ValueBase { fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError>; }
```

There is no `SkeletonResult::NotYetWired` stub — the wire is fully wired.
Transport implementations (`IiopCorbaConnection`, `AmiClient`, `AmhEndpoint`,
`BiDirEndpoint`) live in `zerodds-corba-interop`.

## §10 Repository ID format

CORBA 3.3 §10.7.3.1: `IDL:<scope>/<TypeName>:<major>.<minor>`. The codegen
emits a `*_REPOSITORY_ID` constant per interface/valuetype,
**scope-correct** (module path passed through, incl. `typeprefix`).

## §11 Conformance

A **conformant** codegen + runtime:

1. emits stubs that marshal real GIOP requests (no `NotYetWired`),
2. maps `raises` to typed `IError` enums (§3.1),
3. emits wire-marshallable valuetype state carriers + the value_wire engine
   with sharing/chunking/truncation byte-conformant to §4.2,
4. emits AMI stubs for `@ami` (§5),
5. delivers AMH, Bidirectional GIOP and Portable Interceptors according to §6–§8,
6. encodes RepositoryIds scope-correctly (§10).

## §12 Implementation mapping

| Spec | Code |
|---|---|
| §3 Interface | `corba-rust/src/interface_emit.rs` |
| §4 Valuetype engine | `corba-rust/src/value_wire.rs`; codegen `corba-rust/src/valuetype_emit.rs` |
| §5 AMI codegen | `corba-rust/src/ami_emit.rs`; `AsyncCorbaChannel` in `corba-rust/src/runtime.rs` |
| §6 AMH | `corba-interop/src/runtime.rs` — `AmhEndpoint`, `AmhResponseHandler` |
| §7 Bidirectional | `corba-interop/src/runtime.rs` — `BiDirEndpoint` |
| §8 Interceptors | `corba-ccm/src/orb_extensions.rs` — `RequestInfo`, `InterceptorRegistry` |
| §9 Runtime | `corba-rust/src/runtime.rs`; transport `corba-interop/src/runtime.rs` |

## §13 Conformance tests

- `snapshot_codegen.rs` — insta snapshots per IDL snippet (interface/valuetype/AMI).
- `compile_check.rs` — emitted code actually compiles against path deps
  (interface, valuetype + inheritance, AMI).
- `value_wire` unit — byte-exact tests incl. JacORB capture (single/chunked/
  truncated, BE+LE).
- `corba-interop/tests/` — GIOP e2e over real IIOP: valuetype, AMI, AMH,
  bidir; **cross-ORB live** against JacORB (valuetype-echo, AMI, CSIv2, BiDir) and
  byte-conformant against omniORB (BiDir-SC).
