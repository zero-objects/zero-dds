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
//! * §13 DynAny — re-exported from `zerodds_corba_rt::dyn_any`
//!   ([`DynAny`], [`DynAnyFactory`]).

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
    /// Return value (post-invoke): the reply body.
    pub result: Option<NamedValue>,
    /// Where the octets of [`Self::result`] begin in the reply message,
    /// counted from its first octet: CDR aligns the result from there
    /// (CORBA 3.3 Part 2 §9.4), and a GIOP 1.0/1.1 reply body follows the
    /// reply header without padding (§9.4.3). 0 before [`Self::invoke`].
    pub result_offset: usize,
    /// The byte order of [`Self::result`]: the reply message's, which
    /// the server chooses (§9.4.1). Big-endian before [`Self::invoke`].
    pub result_endianness: zerodds_wire::Endianness,
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
            result_offset: 0,
            result_endianness: zerodds_wire::Endianness::Big,
            user_exceptions: Vec::new(),
        }
    }

    /// The reply body at its offset in the reply message: read it with
    /// `result_body().reader(request.result_endianness)`, which aligns
    /// from the message start. `None` before [`Self::invoke`].
    #[must_use]
    pub fn result_body(&self) -> Option<zerodds_corba_giop::AlignedBody> {
        self.result
            .as_ref()
            .map(|nv| zerodds_corba_giop::AlignedBody::new(&nv.value, self.result_offset))
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
    /// NoException reply body as the return value in [`Request::result`],
    /// its offset in the reply message in [`Request::result_offset`] and
    /// the reply's byte order in [`Request::result_endianness`]
    /// ([`Request::result_body`] reads it there, Part 2 §9.4.3).
    /// Server-side `UserException`/`SystemException` come back as `Err`.
    ///
    /// Out/inout args with their own reply position require a
    /// TypeCode-driven split of the reply body (DynAny walk), which is not
    /// built (`internal/release/OPEN-ITEMS.md`); the canonical §7 case
    /// (in-args + return value) is fully wire-capable here.
    ///
    /// # Errors
    /// Wire error or server-side exception (`CorbaException`).
    #[cfg(feature = "std")]
    pub fn invoke(
        &mut self,
        conn: &dyn zerodds_corba_rust::CorbaConnection,
        target: &zerodds_corba_rust::ObjectReference,
        endianness: zerodds_wire::Endianness,
    ) -> Result<(), zerodds_corba_rust::CorbaException> {
        // Concatenate input args (the caller pre-encoded them, same logic
        // as encode_giop_request).
        let mut payload: Vec<u8> = Vec::new();
        for nv in &self.arguments.entries {
            if nv.flags.is_input() {
                payload.extend_from_slice(&nv.value);
            }
        }
        // The concatenated values were written from offset 0; they go
        // out only where the body starts at a multiple of 8.
        let (reply_body, reply_e) = conn.invoke_aligned(
            target,
            &self.operation,
            endianness,
            &zerodds_corba_rust::marshaled_body(&payload, 0),
        )?;
        self.result = Some(NamedValue {
            name: String::from("return"),
            value: reply_body.bytes().to_vec(),
            flags: ArgFlag::Out,
        });
        self.result_offset = reply_body.offset();
        self.result_endianness = reply_e;
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
    /// Where the octets of [`Self::input_body`] begin in the GIOP
    /// message, counted from its first octet: CDR aligns the arguments
    /// from there (CORBA 3.3 Part 2 §9.4), and a GIOP 1.0/1.1 request
    /// body follows the header without padding (§9.4.2.1). 0 for a
    /// request built by [`Self::new`].
    pub arguments_offset: usize,
    /// The byte order of the arguments: the request message's (§9.4.1).
    /// Big-endian for a request built by [`Self::new`].
    pub endianness: zerodds_wire::Endianness,
    /// Reply value (set by the servant before `set_result`).
    pub reply: Option<NamedValue>,
    /// Exception reply (alternative to `reply`).
    pub exception: Option<NamedValue>,
}

impl ServerRequest {
    /// Constructor (typically from the DSI stub): arguments written from
    /// offset 0, big-endian.
    #[must_use]
    pub fn new(operation: impl Into<String>, arguments: NvList) -> Self {
        Self {
            operation: operation.into(),
            arguments,
            arguments_offset: 0,
            endianness: zerodds_wire::Endianness::Big,
            reply: None,
            exception: None,
        }
    }

    /// The request a server received: the body as the single in-argument
    /// `body`, with its offset in the message and the message's byte
    /// order.
    #[must_use]
    pub fn received(
        operation: impl Into<String>,
        body: &zerodds_corba_giop::AlignedBody,
        endianness: zerodds_wire::Endianness,
    ) -> Self {
        let mut arguments = NvList::new();
        arguments.add_value("body", body.bytes().to_vec(), ArgFlag::In);
        Self {
            arguments_offset: body.offset(),
            endianness,
            ..Self::new(operation, arguments)
        }
    }

    /// The in and inout arguments at their offset in the message: read
    /// them with `input().reader(request.endianness)`, which aligns from
    /// the message start.
    #[must_use]
    pub fn input(&self) -> zerodds_corba_giop::AlignedBody {
        zerodds_corba_giop::AlignedBody::new(&self.input_body(), self.arguments_offset)
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
    /// classic `Servant::invoke(operation, body)` path. The octets stand
    /// at [`Self::arguments_offset`] in the message; [`Self::input`]
    /// reads them there.
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
    /// Never called for the `CORBA::Object` operations (`_is_a`,
    /// `_non_existent`, …): [`dispatch_dsi`] answers those from
    /// [`Self::primary_interface`] and [`Self::all_interfaces`].
    fn dynamic_invoke(&self, req: &mut ServerRequest);

    /// `PortableServer::DynamicImplementation::_primary_interface` — the
    /// repository id of the most-derived interface this servant
    /// implements. `_is_a` and `_repository_id` answer from it.
    fn primary_interface(&self) -> String;

    /// The repository ids of the interfaces `primary_interface` derives
    /// from, transitively (`_all_interfaces` without the first entry).
    /// None by default.
    fn all_interfaces(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Spec §12 — **DSI server bind**: adapts a [`DsiServant`] to the
/// generic `(operation, body, endianness) -> SkeletonResult` dispatcher
/// signature (identical to the generated `dispatch_<iface>`). Directly
/// registrable with a CorbaServer — the incoming GIOP request body is packed
/// as a single in-arg into a [`ServerRequest`], `dynamic_invoke` is called, and
/// `reply`/`exception` are mapped onto the `SkeletonResult`.
///
/// The `exception` value MUST be the complete UserException reply body
/// (`string repository_id` + members, continuous CDR) — the dynamic
/// servant encodes it itself.
#[cfg(feature = "std")]
#[must_use]
pub fn dispatch_dsi(
    servant: &dyn DsiServant,
    operation: &str,
    body: &zerodds_corba_rust::AlignedBody,
    endianness: zerodds_wire::Endianness,
) -> zerodds_corba_rust::SkeletonResult {
    use zerodds_corba_rust::{CorbaException, SkeletonResult};
    // CORBA 3.3 Part 2 §15.4.2: `_is_a`, `_non_existent`, `_interface`,
    // `_repository_id`, `_component`, `_domain_managers` are operations
    // of CORBA::Object, answered by the ORB for every servant — the
    // generated skeletons and the POA do so first; a DSI servant's
    // dynamic_invoke never sees them.
    let primary = servant.primary_interface();
    let bases = servant.all_interfaces();
    let base_ids: Vec<&str> = bases.iter().map(String::as_str).collect();
    if let Some(result) = zerodds_corba_rust::runtime::dispatch_object_operation(
        operation, body, endianness, &primary, &base_ids,
    ) {
        return result;
    }
    // CORBA 3.3 Part 2 §9.4.2.1: the servant reads the arguments at
    // their offset in the message.
    let mut req = ServerRequest::received(operation, body, endianness);
    servant.dynamic_invoke(&mut req);
    if let Some(exc) = req.exception {
        // The servant wrote the exception from its first octet.
        return SkeletonResult::Exception(CorbaException::UserException {
            repository_id: exc.name,
            body: zerodds_corba_rust::AlignedBody::new(&exc.value, 0),
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

// `DynAny` and `DynAnyFactory` live in `zerodds_corba_rt::dyn_any`, on the
// TypeCode and `any` of the CORBA runtime; the ORB hands the factory out
// as the initial reference `DynAnyFactory`.
pub use zerodds_corba_rt::dyn_any::{
    DynAny, DynAnyError, DynAnyFactory, DynResult, InconsistentTypeCode, equivalent,
};

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
            zerodds_wire::Endianness::Little,
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
            fn primary_interface(&self) -> String {
                "IDL:demo/Echo:1.0".into()
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

    /// CORBA 3.3 Part 2 §9.4.2.1: a GIOP 1.0/1.1 request body follows
    /// the header without padding, and CDR aligns an 8-octet argument
    /// from the message start. `dispatch_dsi` hands the servant the
    /// body's offset and byte order: an octet and a `long long` read
    /// with `input()` come out as written; read from the first octet of
    /// the arguments they do not.
    #[test]
    fn corba_p2_9_4_2_1_a_dsi_servant_reads_its_arguments_at_their_offset() {
        use zerodds_corba_rust::SkeletonResult;
        const VALUE: i64 = 0x0102_0304_0506_0708;
        struct AfterOctet;
        impl DsiServant for AfterOctet {
            fn dynamic_invoke(&self, req: &mut ServerRequest) {
                let input = req.input();
                let mut r = input.reader(req.endianness);
                let mut w = zerodds_wire::Writer::new(req.endianness);
                if let (Ok(tag), Ok(value)) = (r.read_u8(), r.read_i64()) {
                    w.write_u8(tag);
                    w.write_i64(value);
                }
                req.set_result(w.finish());
            }
            fn primary_interface(&self) -> String {
                "IDL:demo/Aligned:1.0".into()
            }
        }
        for e in [
            zerodds_wire::Endianness::Big,
            zerodds_wire::Endianness::Little,
        ] {
            for offset in [8usize, 12, 13, 15] {
                let mut w = zerodds_corba_giop::body_writer(offset, e);
                w.write_u8(0x5a);
                w.write_i64(VALUE);
                let bytes = zerodds_corba_giop::finish_body(w, offset);
                let body = zerodds_corba_rust::AlignedBody::new(&bytes, offset);
                let SkeletonResult::Reply(reply) =
                    dispatch_dsi(&AfterOctet, "after_octet", &body, e)
                else {
                    panic!("offset {offset}: no reply");
                };
                let mut r = zerodds_wire::Reader::new(&reply, e);
                assert_eq!(r.read_u8().expect("tag"), 0x5a, "offset {offset}");
                assert_eq!(r.read_i64().expect("value"), VALUE, "offset {offset}");
                // The same octets from the first octet of the arguments.
                let input = ServerRequest::received("after_octet", &body, e).input_body();
                let mut from_zero = zerodds_wire::Reader::new(&input, e);
                let misread =
                    from_zero.read_u8().is_ok() && from_zero.read_i64().is_ok_and(|v| v == VALUE);
                assert_eq!(misread, offset % 8 == 0, "offset {offset}");
            }
        }
    }

    /// `_is_a` and `_non_existent` reach a DSI servant through
    /// `dispatch_dsi` like any object: answered from its interfaces, never
    /// passed to `dynamic_invoke` (which here would echo the request body
    /// back as if it were the answer).
    #[test]
    fn dsi_dispatch_answers_the_object_operations() {
        use zerodds_corba_rust::SkeletonResult;
        struct EchoDsi;
        impl DsiServant for EchoDsi {
            fn dynamic_invoke(&self, req: &mut ServerRequest) {
                let body = req.input_body();
                req.set_result(body);
            }
            fn primary_interface(&self) -> String {
                "IDL:demo/Echo:1.0".into()
            }
            fn all_interfaces(&self) -> Vec<String> {
                alloc::vec!["IDL:demo/Base:1.0".into()]
            }
        }
        let e = zerodds_wire::Endianness::Little;
        let is_a = |id: &str| {
            let mut w = zerodds_wire::Writer::new(e);
            w.write_string(id);
            let body = zerodds_corba_rust::AlignedBody::new(&w.finish(), 0);
            match dispatch_dsi(&EchoDsi, "_is_a", &body, e) {
                SkeletonResult::Reply(b) => b,
                other => panic!("_is_a: {other:?}"),
            }
        };
        assert_eq!(is_a("IDL:demo/Echo:1.0"), alloc::vec![1]);
        assert_eq!(is_a("IDL:demo/Base:1.0"), alloc::vec![1]);
        assert_eq!(is_a("IDL:omg.org/CORBA/Object:1.0"), alloc::vec![1]);
        assert_eq!(is_a("IDL:demo/Other:1.0"), alloc::vec![0]);
        assert!(matches!(
            dispatch_dsi(
                &EchoDsi,
                "_non_existent",
                &zerodds_corba_rust::AlignedBody::new(&[], 0),
                e
            ),
            SkeletonResult::Reply(b) if b == alloc::vec![0]
        ));
        assert!(matches!(
            dispatch_dsi(
                &EchoDsi,
                "ping",
                &zerodds_corba_rust::AlignedBody::new(&[7, 8], 0),
                e
            ),
            SkeletonResult::Reply(b) if b == alloc::vec![7, 8]
        ));
    }
}
