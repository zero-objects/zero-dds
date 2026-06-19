// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 1 §11 DII + §12 DSI + §13 DynAny — dynamic API.
//!
//! Spec source:
//! - §11 Dynamic Invocation Interface — `Request`, `NVList`, `NamedValue`.
//! - §12 Dynamic Skeleton Interface — `ServerRequest`.
//! - §13 Dynamic Management of Any Values — `DynAny`.
//!
//! ## Wire-up against GIOP / TypeCode (Layer-8 cleanup)
//!
//! The DII/DSI/DynAny layers were historically pure data models
//! without a wire path. As of the Layer-8 cleanup they are
//! productively wired:
//!
//! * §11 DII — [`Request::encode_giop_request`] converts a
//!   DII `Request` (NVList of in/inout args) into a
//!   `corba_giop::Request` wire frame.
//! * §12 DSI — the [`DsiServant`] trait gives servants a generic
//!   server-side dispatch path without a compiled skeleton.
//! * §13 DynAny — [`DynAny::from_type_code`] / [`DynAny::to_cdr`]
//!   walk a `corba_ir::TypeCode` over CDR `any` bytes.

use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// §11 Dynamic Invocation Interface (DII)
// ---------------------------------------------------------------------------

/// Spec §11.1.2 — `NamedValue` structure (name + value + flags).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedValue {
    /// Argument name (may be empty for positional arguments).
    pub name: String,
    /// Value as CDR-encoded bytes (the spec uses Any; we take
    /// the serialized form).
    pub value: Vec<u8>,
    /// Spec §11.1.2 — argument flags: `ARG_IN`/`ARG_OUT`/`ARG_INOUT`.
    pub flags: ArgFlag,
}

/// Spec §11.1.2 — argument direction flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgFlag {
    /// `ARG_IN` (1).
    In,
    /// `ARG_OUT` (2).
    Out,
    /// `ARG_INOUT` (3).
    InOut,
}

impl ArgFlag {
    /// Wire value per spec §11.1.2.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::In => 1,
            Self::Out => 2,
            Self::InOut => 3,
        }
    }

    /// Conversion from the wire value.
    ///
    /// # Errors
    /// `()` if the value is not 1/2/3.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            1 => Ok(Self::In),
            2 => Ok(Self::Out),
            3 => Ok(Self::InOut),
            _ => Err(()),
        }
    }

    /// `true` if the argument is transmitted to the server (in/inout).
    #[must_use]
    pub const fn is_input(self) -> bool {
        matches!(self, Self::In | Self::InOut)
    }
}

/// Spec §11.1.3 — `NVList` (sequence of NamedValue).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NvList {
    /// Entries.
    pub entries: Vec<NamedValue>,
}

impl NvList {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §11.1.3 — `add_value(name, value, flags)`.
    pub fn add_value(&mut self, name: impl Into<String>, value: Vec<u8>, flags: ArgFlag) {
        self.entries.push(NamedValue {
            name: name.into(),
            value,
            flags,
        });
    }

    /// Number of entries (spec `count()`).
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Spec §11.2 — DII `Request`.
#[derive(Debug, Clone)]
pub struct Request {
    /// Operation name (e.g. `"getStatus"`).
    pub operation: String,
    /// Argument list (in/out/inout).
    pub arguments: NvList,
    /// Return value (post-invoke).
    pub result: Option<NamedValue>,
    /// User exception IDs the server is allowed to raise.
    pub user_exceptions: Vec<String>,
}

impl Request {
    /// Constructor.
    #[must_use]
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            arguments: NvList::new(),
            result: None,
            user_exceptions: Vec::new(),
        }
    }

    /// `add_in_arg(name, value)`.
    pub fn add_in_arg(&mut self, name: impl Into<String>, value: Vec<u8>) {
        self.arguments.add_value(name, value, ArgFlag::In);
    }

    /// `add_out_arg(name)`.
    pub fn add_out_arg(&mut self, name: impl Into<String>) {
        self.arguments.add_value(name, Vec::new(), ArgFlag::Out);
    }

    /// Encodes the DII request as a GIOP 1.2 `Request` body.
    ///
    /// Spec §11.2 (DII) + §15.4.2 (GIOP wire). NVList values with
    /// `ArgFlag::In`/`ArgFlag::InOut` are concatenated as already
    /// CDR-encoded bytes (the caller supplies the individual argument
    /// bytes pre-encoded per spec — analogous to the `body` contract of
    /// `corba_giop::Request`).
    ///
    /// # Errors
    /// [`GiopRequestError::ObjectKeyTooLong`] if `object_key` exceeds
    /// the `u32::MAX` limit.
    pub fn encode_giop_request(
        &self,
        request_id: u32,
        object_key: &[u8],
    ) -> Result<zerodds_corba_giop::Request, GiopRequestError> {
        if u32::try_from(object_key.len()).is_err() {
            return Err(GiopRequestError::ObjectKeyTooLong);
        }
        // Concatenate all input args. The caller pre-encoded them.
        let mut body: Vec<u8> = Vec::new();
        for nv in &self.arguments.entries {
            if nv.flags.is_input() {
                body.extend_from_slice(&nv.value);
            }
        }
        let target = zerodds_corba_giop::TargetAddress::Key(object_key.to_vec());
        let mut req = zerodds_corba_giop::Request::new(
            request_id,
            // SYNC_WITH_TARGET = 0x03 (response_expected = true).
            zerodds_corba_giop::ResponseFlags::SYNC_WITH_TARGET,
            target,
            self.operation.clone(),
        );
        req.body = body;
        Ok(req)
    }

    /// Spec §7.2 — DII **live invoke**: sends the in/inout args as a real
    /// GIOP request through the [`zerodds_corba_rust::CorbaConnection`]
    /// abstraction (transport-agnostic: IIOP/SSLIOP/UIOP) and stores the
    /// NoException reply body as the return value in [`Request::result`].
    /// Server-side `UserException`/`SystemException` come back as `Err`.
    ///
    /// Out/inout args with their own reply position require a
    /// TypeCode-driven split of the reply body (DynAny walk) — see
    /// `docs/corba-extra-mile-plan-2026-06-07.md` (DII refinement); the
    /// canonical §7 case (in-args + return value) is fully wire-capable here.
    ///
    /// # Errors
    /// Wire error or server-side exception (`CorbaException`).
    pub fn invoke(
        &mut self,
        conn: &dyn zerodds_corba_rust::CorbaConnection,
        target: &zerodds_corba_rust::ObjectReference,
        endianness: zerodds_cdr::Endianness,
    ) -> Result<(), zerodds_corba_rust::CorbaException> {
        // Concatenate input args (the caller pre-encoded them, same logic
        // as encode_giop_request).
        let mut payload: Vec<u8> = Vec::new();
        for nv in &self.arguments.entries {
            if nv.flags.is_input() {
                payload.extend_from_slice(&nv.value);
            }
        }
        let (reply_body, _reply_e) = conn.invoke(target, &self.operation, endianness, &payload)?;
        self.result = Some(NamedValue {
            name: String::from("return"),
            value: reply_body,
            flags: ArgFlag::Out,
        });
        Ok(())
    }
}

/// Error while encoding a DII request into a GIOP frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiopRequestError {
    /// `object_key` exceeds the `u32::MAX` limit.
    ObjectKeyTooLong,
}

impl core::fmt::Display for GiopRequestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ObjectKeyTooLong => f.write_str("object_key exceeds u32::MAX"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GiopRequestError {}

// ---------------------------------------------------------------------------
// §12 Dynamic Skeleton Interface (DSI)
// ---------------------------------------------------------------------------

/// Spec §12.1 — `ServerRequest` (server-side counterpart of DII Request).
#[derive(Debug, Clone)]
pub struct ServerRequest {
    /// Operation name (chosen by the client).
    pub operation: String,
    /// Received arguments.
    pub arguments: NvList,
    /// Reply value (set by the servant before `set_result`).
    pub reply: Option<NamedValue>,
    /// Exception reply (alternative to `reply`).
    pub exception: Option<NamedValue>,
}

impl ServerRequest {
    /// Constructor (typically from the DSI stub).
    #[must_use]
    pub fn new(operation: impl Into<String>, arguments: NvList) -> Self {
        Self {
            operation: operation.into(),
            arguments,
            reply: None,
            exception: None,
        }
    }

    /// Spec §12.1 — `set_result(value)`.
    pub fn set_result(&mut self, value: Vec<u8>) {
        self.reply = Some(NamedValue {
            name: String::new(),
            value,
            flags: ArgFlag::Out,
        });
    }

    /// Spec §12.1 — `set_exception(exception)`.
    pub fn set_exception(&mut self, exception_id: impl Into<String>, value: Vec<u8>) {
        self.exception = Some(NamedValue {
            name: exception_id.into(),
            value,
            flags: ArgFlag::Out,
        });
    }

    /// Concatenates in/inout bytes into a flat body — used by the
    /// default `DsiServant` adapter to map the DSI path onto the
    /// classic `Servant::invoke(operation, body)` path.
    #[must_use]
    pub fn input_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        for nv in &self.arguments.entries {
            if nv.flags.is_input() {
                body.extend_from_slice(&nv.value);
            }
        }
        body
    }
}

/// Spec §12 — DSI servant trait.
///
/// The classic servant trait (in `crates/corba-poa/src/servant.rs`)
/// works operation-bytes-driven. DSI additionally requires a
/// generic path via `ServerRequest`. Implementing crates
/// can implement this trait alongside `Servant`; the
/// default impl gathers the in/inout args via [`ServerRequest::input_body`]
/// and delegates to a caller-provided body handler.
///
/// The trait is deliberately kept _orthogonal_ to `corba-poa::Servant`
/// so that `corba-ccm` does not need a dependency on `corba-poa` (which
/// would create a layer cycle — `corba-poa` is layer 8.16,
/// `corba-ccm` is layer 8.3).
pub trait DsiServant {
    /// Spec §12 — generic dispatch. Implementers decide based on
    /// `req.operation` + `req.arguments` what happens; reply via
    /// [`ServerRequest::set_result`] or [`ServerRequest::set_exception`].
    fn dynamic_invoke(&self, req: &mut ServerRequest);
}

/// Spec §12 — **DSI server bind**: adapts a [`DsiServant`] to the
/// generic `(operation, body, endianness) -> SkeletonResult` dispatcher
/// signature (identical to the generated `dispatch_<iface>`). Directly
/// registrable with a CorbaServer — the incoming GIOP request body is packed
/// as a single in-arg into a [`ServerRequest`], `dynamic_invoke` is called, and
/// `reply`/`exception` are mapped onto the [`SkeletonResult`].
///
/// The `exception` value MUST be the complete UserException reply body
/// (`string repository_id` + members, continuous CDR) — the dynamic
/// servant encodes it itself.
#[must_use]
pub fn dispatch_dsi(
    servant: &dyn DsiServant,
    operation: &str,
    body: &[u8],
    endianness: zerodds_cdr::Endianness,
) -> zerodds_corba_rust::SkeletonResult {
    use zerodds_corba_rust::{CorbaException, SkeletonResult};
    let mut args = NvList::new();
    args.add_value("body", body.to_vec(), ArgFlag::In);
    let mut req = ServerRequest::new(operation, args);
    servant.dynamic_invoke(&mut req);
    if let Some(exc) = req.exception {
        return SkeletonResult::Exception(CorbaException::UserException {
            repository_id: exc.name,
            body: exc.value,
            endianness,
        });
    }
    match req.reply {
        Some(r) => SkeletonResult::Reply(r.value),
        None => SkeletonResult::Reply(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// §13 Dynamic Management of Any Values (DynAny)
// ---------------------------------------------------------------------------

/// Spec §13.1 — `DynAny` type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynAnyKind {
    /// `DynBoolean` etc.
    Primitive,
    /// `DynStruct`.
    Struct,
    /// `DynUnion`.
    Union,
    /// `DynEnum`.
    Enum,
    /// `DynSequence`.
    Sequence,
    /// `DynArray`.
    Array,
    /// `DynFixed`.
    Fixed,
    /// `DynValue`.
    Value,
    /// `DynValueBox`.
    ValueBox,
}

/// Spec §13.2 — `DynAny`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynAny {
    /// Type kind (spec §13.1 discriminator).
    pub kind: DynAnyKind,
    /// Repository ID of the type (e.g. `IDL:demo/MyStruct:1.0`).
    pub repository_id: String,
    /// CDR-encoded value.
    pub value: Vec<u8>,
}

/// Error during the TypeCode-driven DynAny walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynAnyError {
    /// CDR decode error while walking a primitive value.
    Decode(zerodds_cdr::DecodeError),
    /// `tk_kind` is not yet covered by the DynAny walker
    /// (e.g. custom-marshal valuetypes).
    UnsupportedKind(zerodds_corba_ir::TcKind),
}

impl core::fmt::Display for DynAnyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "DynAny decode: {e}"),
            Self::UnsupportedKind(k) => write!(f, "DynAny unsupported kind: {k:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DynAnyError {}

impl From<zerodds_cdr::DecodeError> for DynAnyError {
    fn from(e: zerodds_cdr::DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl DynAny {
    /// Constructor.
    #[must_use]
    pub fn new(kind: DynAnyKind, repository_id: impl Into<String>, value: Vec<u8>) -> Self {
        Self {
            kind,
            repository_id: repository_id.into(),
            value,
        }
    }

    /// Spec §13.2 — `equal(other)` with type + value comparison.
    #[must_use]
    pub fn equal(&self, other: &Self) -> bool {
        self == other
    }

    /// Spec §13.2 — `from_any(any_bytes)`.
    #[must_use]
    pub fn from_any(
        kind: DynAnyKind,
        repository_id: impl Into<String>,
        any_bytes: Vec<u8>,
    ) -> Self {
        Self::new(kind, repository_id, any_bytes)
    }

    /// Spec §13.2 — `to_any()`.
    #[must_use]
    pub fn to_any(&self) -> Vec<u8> {
        self.value.clone()
    }

    /// Spec §13.1 — TypeCode-driven `any` inspection.
    ///
    /// Walks the `TypeCode` over the `raw` buffer (CDR bytes of the
    /// `any` payload, endianness marker in the stream). For primitive
    /// types (`Long` / `ULong` / `Boolean` / ...) the value is checked
    /// (decode errors are propagated); for complex types (`Struct`/
    /// `Sequence`/`Array`) the raw bytes are preserved as an opaque payload
    /// — the caller can evaluate the sub-DynAny via re-entry with
    /// the member TypeCode.
    ///
    /// # Errors
    /// [`DynAnyError::Decode`] on CDR inconsistencies,
    /// [`DynAnyError::UnsupportedKind`] for custom-marshaling paths.
    pub fn from_type_code(
        tc: &zerodds_corba_ir::TypeCode,
        raw: &[u8],
    ) -> Result<Self, DynAnyError> {
        let kind = map_kind(tc.kind);
        let repository_id = tc.id().unwrap_or("").to_string();
        // Sanity walk for primitive types (catch decoding errors early).
        if matches!(kind, DynAnyKind::Primitive) {
            let mut r = zerodds_cdr::BufferReader::new(raw, zerodds_cdr::Endianness::Little);
            walk_primitive(&mut r, tc.kind)?;
        }
        Ok(Self::new(kind, repository_id, raw.to_vec()))
    }

    /// Projects the DynAny back onto CDR `any` bytes (raw payload,
    /// no type-tag prefix — the caller is responsible for `tk_kind`
    /// encoding if an `Any` wrapper is needed).
    #[must_use]
    pub fn to_cdr(&self) -> Vec<u8> {
        self.value.clone()
    }
}

/// Mapping `TcKind` → `DynAnyKind` (spec §13.1 Table 13-1).
fn map_kind(k: zerodds_corba_ir::TcKind) -> DynAnyKind {
    use zerodds_corba_ir::TcKind as K;
    match k {
        K::Null
        | K::Void
        | K::Short
        | K::Long
        | K::UShort
        | K::ULong
        | K::Float
        | K::Double
        | K::Boolean
        | K::Char
        | K::Octet
        | K::LongLong
        | K::ULongLong
        | K::LongDouble
        | K::WChar
        | K::String
        | K::WString
        | K::TypeCode
        | K::Principal
        | K::Any => DynAnyKind::Primitive,
        K::Struct | K::Except => DynAnyKind::Struct,
        K::Union => DynAnyKind::Union,
        K::Enum => DynAnyKind::Enum,
        K::Sequence => DynAnyKind::Sequence,
        K::Array => DynAnyKind::Array,
        K::Fixed => DynAnyKind::Fixed,
        K::Value => DynAnyKind::Value,
        K::ValueBox => DynAnyKind::ValueBox,
        K::Alias | K::ObjRef | K::Native | K::AbstractInterface | K::LocalInterface => {
            DynAnyKind::Primitive
        }
    }
}

/// Walk function for primitive TypeCodes — verifies that the CDR
/// buffer can hold at least the primitive layout.
fn walk_primitive(
    r: &mut zerodds_cdr::BufferReader<'_>,
    k: zerodds_corba_ir::TcKind,
) -> Result<(), DynAnyError> {
    use zerodds_corba_ir::TcKind as K;
    match k {
        K::Null | K::Void => Ok(()),
        K::Boolean | K::Char | K::Octet => {
            let _ = r.read_u8()?;
            Ok(())
        }
        K::Short | K::UShort | K::WChar => {
            let _ = r.read_u16()?;
            Ok(())
        }
        K::Long | K::ULong | K::Float => {
            let _ = r.read_u32()?;
            Ok(())
        }
        K::LongLong | K::ULongLong | K::Double => {
            let _ = r.read_u64()?;
            Ok(())
        }
        K::String | K::WString => {
            // length-prefix + bytes; validate length only.
            let _ = r.read_u32()?;
            Ok(())
        }
        // LongDouble + TypeCode + Principal + Any: format-specific —
        // we accept the raw bytes without a deeper walk.
        _ => Ok(()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_corba_giop::{Message, Version, decode_message, encode_message};

    // §11 DII
    #[test]
    fn arg_flag_round_trip() {
        for f in [ArgFlag::In, ArgFlag::Out, ArgFlag::InOut] {
            assert_eq!(ArgFlag::from_u8(f.to_u8()).expect("ok"), f);
        }
    }

    #[test]
    fn arg_flag_unknown_value_rejected() {
        assert!(ArgFlag::from_u8(0).is_err());
        assert!(ArgFlag::from_u8(99).is_err());
    }

    #[test]
    fn nvlist_add_value_increments_count() {
        let mut l = NvList::new();
        l.add_value("a", alloc::vec![1, 2], ArgFlag::In);
        l.add_value("b", alloc::vec![3], ArgFlag::Out);
        assert_eq!(l.count(), 2);
    }

    #[test]
    fn dii_request_add_in_arg() {
        let mut r = Request::new("getStatus");
        r.add_in_arg("client_id", alloc::vec![1, 2, 3]);
        assert_eq!(r.arguments.count(), 1);
        assert_eq!(r.arguments.entries[0].flags, ArgFlag::In);
    }

    #[test]
    fn dii_request_add_out_arg() {
        let mut r = Request::new("getStatus");
        r.add_out_arg("status");
        assert_eq!(r.arguments.entries[0].flags, ArgFlag::Out);
    }

    // §11 DII — Wire-up

    #[test]
    fn dii_encode_giop_request_concatenates_input_args() {
        let mut r = Request::new("ping");
        r.add_in_arg("a", alloc::vec![0xde, 0xad]);
        r.add_in_arg("b", alloc::vec![0xbe, 0xef]);
        // The OUT arg must not go into the body.
        r.add_out_arg("c");
        let req = r.encode_giop_request(42, &[0x10, 0x20]).expect("encode ok");
        assert_eq!(req.request_id, 42);
        assert_eq!(req.operation, "ping");
        assert_eq!(req.body, alloc::vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(
            req.target,
            zerodds_corba_giop::TargetAddress::Key(alloc::vec![0x10, 0x20])
        );
    }

    #[test]
    fn dii_encode_giop_request_inout_treated_as_input() {
        let mut r = Request::new("op");
        r.arguments
            .add_value("x", alloc::vec![0xaa], ArgFlag::InOut);
        let req = r.encode_giop_request(1, b"k").expect("encode ok");
        assert_eq!(req.body, alloc::vec![0xaa]);
    }

    #[test]
    fn dii_encode_giop_request_round_trip_via_giop_codec() {
        // DII path → GIOP 1.2 wire frame → decode → fields identical.
        let mut r = Request::new("getStatus");
        r.add_in_arg("client_id", alloc::vec![0x01, 0x02, 0x03, 0x04]);
        let req = r.encode_giop_request(7, &[0xab, 0xcd]).expect("encode ok");
        let frame = encode_message(
            Version::V1_2,
            zerodds_cdr::Endianness::Little,
            false,
            &Message::Request(req),
        )
        .expect("frame ok");
        let (decoded, rest) = decode_message(&frame).expect("decode ok");
        assert!(rest.is_empty());
        match decoded {
            Message::Request(req2) => {
                assert_eq!(req2.request_id, 7);
                assert_eq!(req2.operation, "getStatus");
                assert_eq!(req2.body, alloc::vec![0x01, 0x02, 0x03, 0x04]);
            }
            _ => panic!("expected Request variant"),
        }
    }

    // §12 DSI
    #[test]
    fn dsi_server_request_set_result() {
        let mut sr = ServerRequest::new("getStatus", NvList::new());
        sr.set_result(alloc::vec![0x01, 0x02]);
        assert!(sr.reply.is_some());
        assert!(sr.exception.is_none());
    }

    #[test]
    fn dsi_server_request_set_exception() {
        let mut sr = ServerRequest::new("op", NvList::new());
        sr.set_exception("IDL:demo/Bad:1.0", alloc::vec![0xff]);
        assert!(sr.exception.is_some());
        let ex = sr.exception.expect("ok");
        assert_eq!(ex.name, "IDL:demo/Bad:1.0");
    }

    #[test]
    fn dsi_input_body_concatenates_in_and_inout() {
        let mut nv = NvList::new();
        nv.add_value("a", alloc::vec![0x01], ArgFlag::In);
        nv.add_value("b", alloc::vec![0x02], ArgFlag::Out); // skipped
        nv.add_value("c", alloc::vec![0x03], ArgFlag::InOut);
        let sr = ServerRequest::new("op", nv);
        assert_eq!(sr.input_body(), alloc::vec![0x01, 0x03]);
    }

    // §12 DSI — Wire-up

    #[test]
    fn dsi_servant_default_dispatch_via_input_body() {
        struct EchoDsi;
        impl DsiServant for EchoDsi {
            fn dynamic_invoke(&self, req: &mut ServerRequest) {
                let body = req.input_body();
                req.set_result(body);
            }
        }
        let mut nv = NvList::new();
        nv.add_value("a", alloc::vec![0xde, 0xad], ArgFlag::In);
        nv.add_value("b", alloc::vec![0xbe, 0xef], ArgFlag::InOut);
        let mut sr = ServerRequest::new("ping", nv);
        EchoDsi.dynamic_invoke(&mut sr);
        let reply = sr.reply.expect("reply set");
        assert_eq!(reply.value, alloc::vec![0xde, 0xad, 0xbe, 0xef]);
    }

    // §13 DynAny
    #[test]
    fn dyn_any_round_trip() {
        let d = DynAny::new(DynAnyKind::Struct, "IDL:demo/S:1.0", alloc::vec![1, 2, 3]);
        assert_eq!(d.to_any(), alloc::vec![1, 2, 3]);
    }

    #[test]
    fn dyn_any_equal_same_value() {
        let a = DynAny::new(DynAnyKind::Primitive, "long", alloc::vec![1]);
        let b = DynAny::new(DynAnyKind::Primitive, "long", alloc::vec![1]);
        assert!(a.equal(&b));
    }

    #[test]
    fn dyn_any_not_equal_different_kind() {
        let a = DynAny::new(DynAnyKind::Primitive, "long", alloc::vec![1]);
        let b = DynAny::new(DynAnyKind::Struct, "long", alloc::vec![1]);
        assert!(!a.equal(&b));
    }

    #[test]
    fn dyn_any_from_any_round_trip() {
        let d = DynAny::from_any(DynAnyKind::Sequence, "seq<long>", alloc::vec![0xde, 0xad]);
        assert_eq!(d.to_any(), alloc::vec![0xde, 0xad]);
    }

    #[test]
    fn dyn_any_kind_variants_are_distinct() {
        assert_ne!(DynAnyKind::Primitive, DynAnyKind::Struct);
        assert_ne!(DynAnyKind::Sequence, DynAnyKind::Array);
    }

    // §13 DynAny — Wire-up

    #[test]
    fn dyn_any_from_type_code_long_round_trip() {
        let tc = zerodds_corba_ir::TypeCode::primitive(zerodds_corba_ir::TcKind::Long);
        // Little-endian encoding of a `long` value 0x12345678.
        let raw = alloc::vec![0x78, 0x56, 0x34, 0x12];
        let dyn_any = DynAny::from_type_code(&tc, &raw).expect("walk ok");
        assert_eq!(dyn_any.kind, DynAnyKind::Primitive);
        assert_eq!(dyn_any.to_cdr(), raw);
    }

    #[test]
    fn dyn_any_from_type_code_long_rejects_truncated_buffer() {
        let tc = zerodds_corba_ir::TypeCode::primitive(zerodds_corba_ir::TcKind::Long);
        // Only 3 bytes — long requires 4.
        let raw = alloc::vec![0x01, 0x02, 0x03];
        let err = DynAny::from_type_code(&tc, &raw).expect_err("must fail");
        assert!(matches!(err, DynAnyError::Decode(_)));
    }

    #[test]
    fn dyn_any_from_type_code_sequence_preserves_bytes() {
        let tc = zerodds_corba_ir::TypeCode::sequence(
            zerodds_corba_ir::TypeCode::primitive(zerodds_corba_ir::TcKind::Long),
            10,
        );
        // Complex type → no primitive validation, raw bytes preserved.
        let raw = alloc::vec![0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa];
        let dyn_any = DynAny::from_type_code(&tc, &raw).expect("walk ok");
        assert_eq!(dyn_any.kind, DynAnyKind::Sequence);
        assert_eq!(dyn_any.to_cdr(), raw);
    }

    #[test]
    fn dyn_any_from_type_code_struct_preserves_bytes_and_id() {
        let tc = zerodds_corba_ir::TypeCode::r#struct(
            "IDL:demo/Pair:1.0".into(),
            "Pair".into(),
            alloc::vec![
                zerodds_corba_ir::type_code::StructMember {
                    name: "a".into(),
                    type_code: zerodds_corba_ir::TypeCode::primitive(
                        zerodds_corba_ir::TcKind::Long,
                    ),
                },
                zerodds_corba_ir::type_code::StructMember {
                    name: "b".into(),
                    type_code: zerodds_corba_ir::TypeCode::primitive(
                        zerodds_corba_ir::TcKind::Long,
                    ),
                },
            ],
        );
        let raw = alloc::vec![0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00];
        let d = DynAny::from_type_code(&tc, &raw).expect("walk ok");
        assert_eq!(d.kind, DynAnyKind::Struct);
        assert_eq!(d.repository_id, "IDL:demo/Pair:1.0");
        assert_eq!(d.to_cdr(), raw);
    }
}
