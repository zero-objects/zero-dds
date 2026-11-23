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
`dispatch_i` (server skeleton) are emitted. The stub calls
`CorbaConnection::invoke` with a `MarshalArgs` closure that writes the in- and
inout-args into the GIOP request body, and decodes return + out/inout from the
reply (same order). The connection calls the closure once the request header
is known, with a writer whose alignment counts from the start of the message:
a GIOP 1.0/1.1 body follows the header without padding (CORBA 3.3 Part 2
§9.4.2.1), so the place of an 8-octet argument depends on the header. An
operation without in- or inout-args passes `no_arguments`.

The skeleton is `dispatch_i(servant: &dyn I, operation: &str, body:
&AlignedBody, endianness: Endianness) -> SkeletonResult`. `AlignedBody` holds
the request body with its offset in the message; the skeleton reads the
arguments with `AlignedBody::reader`, aligned from the message start.

A reply body is written from an eight-aligned origin (offset 0) and goes
on a multiple of 8 in the message. Under GIOP 1.2 and later the reply
header pads the body to 8 (CORBA 3.3 Part 2 §9.4.3). Under GIOP 1.0 and
1.1 the body follows the reply header without padding, at a multiple of 4
that the service contexts decide; when it would fall on 4 modulo 8,
`CorbaServer` adds the service context `PADDING_CONTEXT_ID` (0x5A445041,
ASCII `ZDPA`, a ZeroDDS vendor id after §7.7.1) with four zero octets,
which moves the body by twelve octets onto a multiple of 8
(`Reply::place_body_at_eight`, after the last other service context). A
receiver ignores a service context it does not know (§7.7.2). The stub
reads a reply body from offset 0: a reply of another ORB under GIOP 1.0
or 1.1 whose body does not stand on a multiple of 8 is read with its
8-octet values misaligned.

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

A `custom valuetype V` also gets the trait of its custom marshaling (CORBA
3.3 part 1 §9.5.1, OMG IDL 4.2 §7.4.7.4.4), which the application implements
on `VValue`; the generated `marshal_state` and the registered factory call it
instead of writing and reading the declared state:

```rust
pub trait VCustomMarshal: Sized {
    fn marshal(&self, os: &mut zerodds_wire::Writer) -> Result<(), zerodds_wire::WireError>;
    fn unmarshal(is: &mut zerodds_wire::Reader<'_>) -> Result<Self, zerodds_wire::WireError>;
}
```

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
  chunked path (`marshal_state` calls `VCustomMarshal::marshal`).

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
The reply reaches the handler and the poller as an `AsyncReply`, the body at
its offset in the reply message; the generated code reads it through
`AlignedBody::reader` (CORBA 3.3 Part 2 §9.4.3).

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

`zerodds_corba_ccm::portable_interceptor` (re-exported in `orb_extensions`).
`ClientRequestInterceptor` (`send_request`, `send_poll`, `receive_reply`,
`receive_exception`, `receive_other`) and `ServerRequestInterceptor`
(`receive_request_service_contexts`, `receive_request`, `send_reply`,
`send_exception`, `send_other`); every point returns
`Result<(), InterceptorError>` — a system exception or `ForwardRequest`.
`InterceptorRegistry` runs them with the flow-stack rules of CORBA 3.3 Part 1
§16.4: starting points in registration order, ending points in reverse order
for the interceptors whose starting point completed, a raised exception or
forward changes the ending point of the rest. `RequestInfo` carries
`request_id`, `operation`, `response_expected`, `sync_scope`, `reply_status`,
`forward_reference`, the request and reply service contexts
(`add_*_service_context(id, data, replace)`, BAD_INV_ORDER minor 15 for a
duplicate without `replace`; `get_*_service_context`, BAD_PARAM minor 26 for
an absent id) and the request scope slots; an attribute read outside its
interception points is BAD_INV_ORDER minor 14. `arguments`, `exceptions`,
`contexts`, `operation_context` and `result` raise NO_RESOURCES minor 1 at the
points Tables 16.1 and 16.2 admit them: this binding passes encoded CDR. On the
server, `InterceptorRegistry::server_start` runs
`receive_request_service_contexts` and `server_receive_request` runs
`receive_request`; between them the transport copies the request scope slots
into the thread scope of the servant (§16.5.4.5), and after the servant the
thread scope back into the request scope. `IorInfo` knows the profile ids of
the adapter's references (`add_ior_component_to_profile` with another id is
BAD_PARAM minor 29) and the policy types of the registered factories
(`get_effective_policy` is `None` for a known type without a policy,
INV_POLICY minor 3 for an unknown type). Interceptors are registered by an
`OrbInitializer` (`register_orb_initializer`); `orb_init` runs `pre_init` and
`post_init`, which return `Result<(), InitializerError>`: an error abandons
the ORB and `orb_init` raises the system exception, INITIALIZE for a user
exception. In `post_init` the `add_*_interceptor` operations and
`allocate_slot_id` raise BAD_INV_ORDER minor 26; while the initializers run,
`PiCurrent::get_slot`/`set_slot` raise BAD_INV_ORDER minor 14 (§16.9.2.14);
`OrbInitInfo::register_initial_reference` has the semantics of
`ORB::register_initial_reference`. `IiopCorbaConnection::with_orb` and
`CorbaServer::set_orb` run requests through the ORB's interceptors; a
`ForwardRequest` or a `LOCATION_FORWARD` reply sends the request to the new
target. `ServiceContextInjector` attaches OTS (id 0), CSIv2 (id 15) or codeset
contexts as a registered interceptor.

Received service contexts reach the server interceptors and
`CorbaServer::on_request_contexts` whatever their id and the GIOP version of
the request; none raises an exception (CORBA 3.3 Part 2 §7.7.2). The contexts
`IiopCorbaConnection` originates follow Table 7.2 for the GIOP version of the
request: the codeset context from 1.1 on and first in the list, the SAS
context and `INVOCATION_POLICIES` from 1.2 on; a request with CSIv2
credentials below GIOP 1.2 is refused with NO_PERMISSION. A context an
interceptor adds with the id of one of these takes its place in the list.
`CorbaServer` sends `INVOCATION_POLICIES` only in GIOP 1.2 replies.
`IiopCorbaConnection::invoke_passing_contexts` sends the contexts of a bridged
request and returns the reply contexts; the MIOP gateway forwards object
operations with it. `OrbInitInfo::codec_factory` and the initial reference
`CodecFactory` give `zerodds_corba_ccm::iop_codec::CodecFactory` (Part 2
§7.8): `ENCODING_CDR_ENCAPS` 1.0 to 1.4, big-endian encapsulations read in
both byte orders, `wchar` and `wstring` in their GIOP 1.2 form, and a type
Table 7.2 ties to a later GIOP version raises `InvalidTypeForEncoding`.

## §9 Runtime API

```rust
pub struct ObjectReference {
    pub type_id: String,
    // The first TAG_INTERNET_IOP profile body, encapsulated; empty when
    // the reference has none.
    pub iiop_profile: Vec<u8>,
    // Client-side policy overrides (CORBA 3.3 Part 1 §8.3.9): the policy
    // type and the value as an encapsulated `any`. Not part of the IOR:
    // `encode` does not write them, `decode` returns none. The IIOP
    // connection takes the ZIOP policies of an invocation from them before
    // the thread and the ORB overrides.
    pub policy_overrides: Vec<PolicyOverride>,
    // Every profile of the IOR in its order when it holds more or other
    // profiles than the one in `iiop_profile`, or that one profile with
    // empty data (CORBA 3.3 Part 2 §7.6.3, Full IOR conformance); empty
    // otherwise. `encode` writes them back unchanged; a reference without
    // profiles is written with none (nil).
    pub profiles: Vec<RawProfile>,
}
pub struct RawProfile { pub tag: u32, pub data: Vec<u8> }
pub struct PolicyOverride { pub policy_type: u32, pub value: Vec<u8> }

pub enum CorbaException {
    System(SystemException),   // zerodds_corba_rt::SystemException
    UserException { repository_id: String, body: AlignedBody, endianness: Endianness },
}
// `body` is the whole user exception (repository id, then the members) at
// its offset in the reply message (CORBA 3.3 Part 2 §9.4.3): a GIOP 1.0/1.1
// reply of another ORB may put it on 4 modulo 8, and the members align from
// the message start. Stubs read it through `AlignedBody::reader`; a body a
// servant writes from its first octet has offset 0. A server writes the body
// of a user exception it passes on at the same offset modulo 8; a GIOP 1.2
// reply cannot carry a body written for 4 modulo 8 and goes out as MARSHAL
// COMPLETED_YES.

pub struct SystemException {   // CORBA 3.3 Part 1 §8.12
    pub kind: SystemExceptionKind,   // the 39 standard system exceptions
    pub minor: u32,                  // VMCID (high 20 bits) + code
    pub completed: CompletionStatus, // COMPLETED_YES / _NO / _MAYBE
}

pub enum SkeletonResult { Reply(Vec<u8>), Exception(CorbaException), BadOperation }

// A system exception goes on the wire with its own repository id
// (`IDL:omg.org/CORBA/<NAME>:1.0`), minor code and completion status, and
// is read back with the same identity. A repository id that names no
// standard system exception is read as UNKNOWN minor 0x4f4d0002 with the
// completion status kept (§8.12.2). Generated stubs and skeletons raise
// MARSHAL with COMPLETED_NO before the operation ran (request marshalling,
// argument unmarshalling) and COMPLETED_YES after it (reply marshalling,
// reply unmarshalling); an undeclared user exception from a servant goes
// out as UNKNOWN minor 0x4f4d0001 COMPLETED_YES; BadOperation goes out as
// BAD_OPERATION minor 0x4f4d0002.

// The arguments of a request: writes them into the body writer the
// connection hands over, at the body's offset in the message. Called again
// for a request that goes out again with another header (a forward, another
// addressing disposition); an error is the invocation's exception.
pub type MarshalArgs<'a> = dyn Fn(&mut Writer) -> Result<(), CorbaException> + 'a;
pub fn no_arguments(w: &mut Writer) -> Result<(), CorbaException>;
// A body already marshaled at `offset` (a bridge passes a received body on):
// written as it is where the new body starts at the same offset modulo 8,
// anywhere when empty, elsewhere MARSHAL COMPLETED_NO.
pub fn marshaled_body(body: &[u8], offset: usize) -> impl Fn(&mut Writer) -> Result<(), CorbaException>;

pub trait CorbaConnection {            // synchronous
    fn invoke(&self, target, op, e, args: &MarshalArgs) -> Result<(Vec<u8>, Endianness), CorbaException>;
    fn invoke_oneway(&self, target, op, e, args: &MarshalArgs) -> Result<(), CorbaException>;
    // The reply body at its offset in the reply message; the generated
    // stubs read the reply through it. The default answers the body of
    // `invoke` at offset 0.
    fn invoke_aligned(&self, target, op, e, args: &MarshalArgs) -> Result<(AlignedBody, Endianness), CorbaException>;
}

// The outcome of an asynchronous invocation: the reply body at its offset
// in the reply message and the reply's byte order, or the exception.
pub type AsyncReply = Result<(AlignedBody, Endianness), CorbaException>;

pub trait AsyncCorbaChannel {          // asynchronous (AMI, §5)
    fn send(&mut self, op, args: &MarshalArgs, cb) -> Result<u32, CorbaException>;
    fn send_poll(&mut self, op, args: &MarshalArgs) -> Result<u32, CorbaException>;
    fn get_reply(&mut self, request_id) -> Result<AsyncReply, CorbaException>;
    fn perform_work(&mut self) -> Result<u32, CorbaException>;
    fn perform_all(&mut self) -> Result<(), CorbaException>;
}

pub trait ValueBase { fn repository_id(&self) -> &str; }
pub trait ValueMarshal: ValueBase { fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), EncodeError>; }
```

There is no `SkeletonResult::NotYetWired` stub — the wire is fully wired.
Transport implementations (`IiopCorbaConnection`, `AmiClient`, `AmhEndpoint`,
`BiDirEndpoint`) live in `zerodds-corba-interop`. `AmiClient` calls the
`MarshalArgs` of a request when it sends it, once for a body offset of 0 and
once for 4 modulo 8 (the header of a request it sends ends on a multiple of
4), and keeps both for a resend after `LOCATION_FORWARD` or
`NEEDS_ADDRESSING_MODE`; an exception of the arguments at one of the two
offsets is raised by the request that goes out at that offset. `BiDirEndpoint::invoke` takes a body written from
offset 0: Bi-Directional GIOP is GIOP 1.2, whose body starts on a multiple of
8. The server dispatchers of `CorbaServer`, `BiDirEndpoint` and the MIOP
`Goa`, and `AmhRequest::body`, carry the body as `AlignedBody`. So do the
replies on the client side: `CorbaConnection::invoke_aligned`, the
`AsyncReply` of `AmiClient` (the generated `sendc_` callbacks and pollers read
it with `AlignedBody::reader`), `BiDirEndpoint::collect_reply` and
`BiDirEndpoint::invoke`; the DII `Request::invoke` keeps the offset and the
byte order of its result (`Request::result_body`, `result_endianness`). A
GIOP 1.0/1.1 reply body follows the reply header without padding (CORBA 3.3
Part 2 §9.4.3), so another ORB may put it on four modulo eight. A user
exception a servant passes on goes out in a reply of the exception's own byte
order and on its own offset modulo 8.

Object location (CORBA 3.3 Part 2 §9.6, §9.4.3.2, §9.4.6). The client
transports resend a request transparently: after `LOCATION_FORWARD` or
`LOCATION_FORWARD_PERM` to the IOR of the reply body (at most 16 forwards per
invocation, then TRANSIENT `COMPLETED_NO`), after `NEEDS_ADDRESSING_MODE` to
the same target addressed as `ProfileAddr` or `ReferenceAddr` (an agent
asking for the disposition it just got is TRANSIENT `COMPLETED_NO`).
`IiopCorbaConnection` records a `_PERM` forward per original reference
(`forwarded_reference`) and starts later invocations there; when the recorded
target cannot be reached, or answers TRANSIENT, COMM_FAILURE or
OBJECT_NOT_EXIST with `COMPLETED_NO`, the record is dropped and the invocation
restarts at the original reference.

Profiles and addresses (CORBA 3.3 Part 2 §7.6.3, §7.6.6.2). `IiopCorbaConnection`
(requests and `locate`) and `AmiClient::connect` try the `TAG_INTERNET_IOP`
profiles of a reference in the order of the IOR and, in each, the address of
the profile body and then its `TAG_ALTERNATE_IIOP_ADDRESS` addresses, until a
connection opens; an SSLIOP profile is reached at its host and SSL port, a
UIOP profile at its socket path. A reference without a readable IIOP profile
raises IMP_LIMIT minor 1, one whose addresses cannot be reached TRANSIENT
minor 2, both `COMPLETED_NO`. A `ReferenceAddr` target carries every profile
of the IOR and the index of the profile the request went over.
`IiopCorbaConnection::locate` sends a
`LocateRequest` and answers `LocateOutcome::Here(reference)` or
`LocateOutcome::Unknown`, following `OBJECT_FORWARD(_PERM)` and
`LOC_NEEDS_ADDRESSING_MODE` and raising the exception of
`LOC_SYSTEM_EXCEPTION`. `CorbaServer::forward(key, reference, permanent)`
makes the server a location agent for `key` (`LOCATION_FORWARD(_PERM)` to
requests, `OBJECT_FORWARD(_PERM)` to `LocateRequest`s, `_PERM` only in GIOP
1.2); `stop_forwarding(key)` ends it. Servers take the object key from all
three addressing dispositions and answer a target without a readable IIOP
profile with `NEEDS_ADDRESSING_MODE` (`LOC_NEEDS_ADDRESSING_MODE`) asking for
`KeyAddr`.

GIOP versions (CORBA 3.3 Part 2 §7.9, §9.4.1, §9.7): the server reads and
writes GIOP 1.0 to 1.4 (`Version::MAX`; 1.3 and 1.4 use the 1.2 formats)
and answers in the version of the request; a message of a higher minor
version is answered with a `MessageError` of version 1.4, then the
connection closes. The client sends the version of the target's IIOP
profile, at most 1.4. References carry IIOP 1.2 profiles
(`DEFAULT_PROFILE_VERSION`) unless `CorbaServer::set_profile_version`
sets 1.1 to 1.4 (BAD_PARAM otherwise); a reference whose host is an IPv6
address is written as IIOP 1.4 (Table 7.2), its `host` field holding the
address without the brackets of the URL form (`[::1]` is written `::1`,
`IiopProfileBody::for_endpoint`). The default is 1.2 because
JacORB 3.9 does not call through a 1.4 profile and omniORB 4.3.3 refuses
one in a reply (measured on the Linux test host).
`CorbaServer::set_component_reference(key, component)` makes `_component`
on `key` answer with the component's reference (Part 3 §6.4.3.1); without
it the dispatcher answers the nil reference.

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
| §8 Interceptors | `corba-ccm/src/portable_interceptor.rs` — `RequestInfo`, `InterceptorRegistry`, `OrbInitializer` |
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
