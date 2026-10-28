// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A [`zerodds_dcps::dds_type::DdsType`] for a DDS-RPC request/reply
//! payload whose structure is only known at run time (synthesized from
//! an IDL `interface` operation's parameters, DDS-RPC 1.0
//! §7.5.1.1.4/§7.5.1.1.5) — the same problem
//! `zerodds_dcps::serialized_type::SerializedSample` solves for a topic
//! sample, and the same reason it gives for why its own `DdsType::decode`
//! is a stub: "serialized samples require their topic support", which
//! `zerodds_rpc::requester::Requester<TIn, TOut>` /
//! `zerodds_rpc::replier::Replier<TIn, TOut>` have no room for — they
//! call `TOut::decode(bytes)` as a bare associated function, with no
//! instance to carry a `SerializedTypeSupport` reference through.
//!
//! [`DynamicRpcValue::encode`] needs no such help (`&self` already
//! carries the `Ast` + type name the value was decoded/built against).
//! [`DynamicRpcValue::decode`] does, and gets it from
//! [`with_decode_context`]: a thread-local slot the caller sets
//! immediately before the one `Requester`/`Replier` call that will
//! invoke `decode` and clears immediately after.
//!
//! **Why this is sound here and would not be in general**: every
//! `Requester`/`Replier` call that can reach `decode` —
//! `send_request_blocking` (`crate::requester`'s own doc: "internally
//! calls `tick()` in a polling loop") and `Replier::tick` ("the caller
//! orchestrates the tick itself — no dedicated thread") — runs
//! synchronously on the thread that calls it, never on a background
//! receive thread. `crate::rpc_dispatch` calls both exactly this way:
//! one blocking round trip per SOAP request, on the thread handling
//! that request, with the context set for exactly its duration. A
//! caller that instead used `send_request_async` (a different thread
//! polls the `mpsc::Receiver` and calls `tick()` there) would see the
//! wrong context if it raced this one — `rpc_dispatch` never does.

use std::cell::RefCell;
use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError};
use zerodds_types::dynamic::DynamicData;
use zerodds_wire_codec::ProbeRequest;

thread_local! {
    static DECODE_CONTEXT: RefCell<Option<(Arc<Ast>, String, Dialect)>> = const { RefCell::new(None) };
}

/// Sets the `(Ast, qualified type name, CDR dialect)`
/// [`DynamicRpcValue::decode`] reads for the duration of `f`, then restores
/// whatever context (if any) was set before — nested calls on the same
/// thread compose correctly, though `crate::rpc_dispatch` never nests them.
/// The dialect is the participant's `wire.codec_dialect`.
pub fn with_decode_context<R>(
    ast: &Arc<Ast>,
    type_name: &str,
    dialect: Dialect,
    f: impl FnOnce() -> R,
) -> R {
    let previous =
        DECODE_CONTEXT.with(|c| c.replace(Some((Arc::clone(ast), type_name.to_string(), dialect))));
    let result = f();
    DECODE_CONTEXT.with(|c| *c.borrow_mut() = previous);
    result
}

/// A DDS-RPC request/reply value: a [`DynamicData`] plus the `Ast`
/// (and qualified type name) it was built or decoded against, so
/// [`Self::encode`] and [`crate::dynxml`] can convert it without
/// needing any other context.
#[derive(Clone)]
pub struct DynamicRpcValue {
    /// The model this value's fields are described against.
    pub ast: Arc<Ast>,
    /// The value's qualified type name within `ast`.
    pub type_name: String,
    /// The value itself.
    pub data: DynamicData,
    /// The CDR dialect it is encoded in; the specification's unless
    /// [`Self::in_dialect`] names another.
    pub dialect: Dialect,
}

impl std::fmt::Debug for DynamicRpcValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicRpcValue")
            .field("type_name", &self.type_name)
            .field("data", &self.data)
            .finish_non_exhaustive()
    }
}

impl DynamicRpcValue {
    /// Constructor.
    #[must_use]
    pub fn new(ast: Arc<Ast>, type_name: String, data: DynamicData) -> Self {
        Self {
            ast,
            type_name,
            data,
            dialect: Dialect::default(),
        }
    }

    /// The same value, encoded in `dialect` — the participant's
    /// `wire.codec_dialect`.
    #[must_use]
    pub fn in_dialect(mut self, dialect: Dialect) -> Self {
        self.dialect = dialect;
        self
    }

    fn probe_request_in(&self, order: Endianness, encoding: Encoding) -> ProbeRequest {
        ProbeRequest::in_dialect(&self.type_name, encoding, order, self.dialect)
    }

    /// The value written in `order` and `encoding` at `start`, behind the
    /// first `start` octets of a sample (`0` for a value on its own), which
    /// the reflective codec does for either byte order.
    fn encode_in(
        &self,
        order: Endianness,
        encoding: Encoding,
        start: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), EncodeError> {
        let bytes = zerodds_wire_codec::encode_continuing(
            &self.ast,
            &self.probe_request_in(order, encoding),
            &self.data,
            start,
        )
        .map_err(|_| EncodeError::Invalid {
            what: "dynamic RPC payload refused by the wire codec",
        })?;
        out.extend(bytes);
        Ok(())
    }

    /// A value read in `order` and `encoding` from `start` of `bytes`,
    /// against the decode context the caller set.
    fn decode_in(
        order: Endianness,
        encoding: Encoding,
        bytes: &[u8],
        start: usize,
    ) -> Result<Self, DecodeError> {
        let (ast, type_name, dialect) =
            DECODE_CONTEXT
                .with(|c| c.borrow().clone())
                .ok_or(DecodeError::Invalid {
                    what: "no decode context set — see crate::dynamic_rpc::with_decode_context",
                })?;
        let request = ProbeRequest::in_dialect(&type_name, encoding, order, dialect);
        let data =
            zerodds_wire_codec::decode_continuing::<DynamicData>(&ast, &request, bytes, start)
                .map_err(|_| DecodeError::Invalid {
                    what: "dynamic RPC payload refused by the wire codec",
                })?;
        Ok(Self {
            ast,
            type_name,
            data,
            dialect,
        })
    }
}

impl DdsType for DynamicRpcValue {
    const TYPE_NAME: &'static str =
        "zerodds::webdds_bridge::DynamicRpcValue (requires crate::dynamic_rpc's decode context)";

    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_in(Endianness::Little, Encoding::PlainCdr2, 0, out)
    }

    /// The reflective codec writes big-endian as readily as little-endian,
    /// so this answers instead of taking the trait's refusal.
    fn encode_be(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_in(Endianness::Big, Encoding::PlainCdr2, 0, out)
    }

    fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_in(Endianness::Little, Encoding::PlainCdr2, bytes, 0)
    }

    fn decode_be(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_in(Endianness::Big, Encoding::PlainCdr2, bytes, 0)
    }

    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode_in(Endianness::Little, Encoding::PlainCdr1, 0, out)
    }

    fn decode_xcdr1(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::decode_in(Endianness::Little, Encoding::PlainCdr1, bytes, 0)
    }

    /// The value behind the first octets of a DDS-RPC sample, alignment
    /// counted from the sample (DDS-RPC 1.0 §7.5.1.1.6).
    fn encode_continuing(&self, prefix: &mut Vec<u8>, xcdr1: bool) -> Result<(), EncodeError> {
        let encoding = if xcdr1 {
            Encoding::PlainCdr1
        } else {
            Encoding::PlainCdr2
        };
        self.encode_in(Endianness::Little, encoding, prefix.len(), prefix)
    }

    fn decode_continuing(bytes: &[u8], offset: usize, xcdr1: bool) -> Result<Self, DecodeError> {
        let encoding = if xcdr1 {
            Encoding::PlainCdr1
        } else {
            Encoding::PlainCdr2
        };
        Self::decode_in(Endianness::Little, encoding, bytes, offset)
    }

    fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue> {
        use zerodds_dcps::FilterValue as Value;
        let mut value = &self.data;
        for component in path.split('.') {
            value = value.member(component)?;
        }
        match value {
            DynamicData::Boolean(v) => Some(Value::Bool(*v)),
            DynamicData::Integer { value, .. } => i64::try_from(*value).ok().map(Value::Int),
            DynamicData::Float { value, .. } => Some(Value::Float(*value)),
            DynamicData::Text(v) | DynamicData::Character { value: v, .. } => {
                Some(Value::String(v.clone()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_types::dynamic::DynamicType;

    fn model(idl: &str, type_name: &str) -> (Arc<Ast>, DynamicType) {
        let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, "t.idl").unwrap());
        let dt = DynamicType::find(&ast, type_name).unwrap();
        (ast, dt)
    }

    #[test]
    fn encode_needs_no_external_context() {
        let (ast, _dt) = model(
            "module M { @final struct Params { long a; long b; }; };",
            "M::Params",
        );
        let data = DynamicData::Aggregate(vec![
            (
                "a".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 3),
            ),
            (
                "b".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 4),
            ),
        ]);
        let value = DynamicRpcValue::new(ast, "M::Params".to_string(), data);
        let mut out = Vec::new();
        value.encode(&mut out).expect("encode");
        assert!(!out.is_empty());
    }

    /// The participant's CDR dialect reaches the octets of a call: in the
    /// specification's a @key member of a @mutable type carries the
    /// must-understand flag in its EMHEADER, in Fast DDS's it does not
    /// (Fast DDS writes `00 00 00 20` for member 0 with LC 2). A value
    /// decodes back in the dialect it was written in.
    #[test]
    fn the_codec_dialect_reaches_the_octets_of_a_call() {
        let (ast, _dt) = model(
            "module M { @mutable struct Call { @key long id; long v; }; };",
            "M::Call",
        );
        let data = DynamicData::Aggregate(vec![
            (
                "id".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 7),
            ),
            (
                "v".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 9),
            ),
        ]);
        let spec = DynamicRpcValue::new(Arc::clone(&ast), "M::Call".to_string(), data);
        let fastdds = spec.clone().in_dialect(Dialect::FastDds);
        let (mut a, mut b) = (Vec::new(), Vec::new());
        spec.encode(&mut a).expect("encode");
        fastdds.encode(&mut b).expect("encode");
        // DHEADER, then the first EMHEADER.
        assert_eq!(a[4..8], [0x00, 0x00, 0x00, 0xa0], "{a:02x?}");
        assert_eq!(b[4..8], [0x00, 0x00, 0x00, 0x20], "{b:02x?}");
        let decoded = with_decode_context(&ast, "M::Call", Dialect::FastDds, || {
            DynamicRpcValue::decode(&b)
        })
        .expect("decode");
        assert_eq!(decoded.dialect, Dialect::FastDds);
        assert_eq!(decoded.data.member("id"), fastdds.data.member("id"));
        assert_eq!(decoded.data.member("v"), fastdds.data.member("v"));
    }

    #[test]
    fn decode_without_context_is_a_named_error_not_a_panic() {
        let err = DynamicRpcValue::decode(&[0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::Invalid { .. }));
    }

    #[test]
    fn round_trips_through_encode_and_decode_with_context_set() {
        let (ast, _dt) = model(
            "module M { @final struct Params { long a; long b; }; };",
            "M::Params",
        );
        let data = DynamicData::Aggregate(vec![
            (
                "a".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 7),
            ),
            (
                "b".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 9),
            ),
        ]);
        let value = DynamicRpcValue::new(Arc::clone(&ast), "M::Params".to_string(), data);
        let mut bytes = Vec::new();
        value.encode(&mut bytes).expect("encode");

        let decoded = with_decode_context(&ast, "M::Params", Dialect::default(), || {
            DynamicRpcValue::decode(&bytes)
        })
        .expect("decode");
        assert_eq!(decoded.data.member("a"), value.data.member("a"));
        assert_eq!(decoded.data.member("b"), value.data.member("b"));
    }

    #[test]
    fn context_is_restored_after_the_call_not_leaked() {
        let (ast, _dt) = model(
            "module M { @final struct Params { long a; }; };",
            "M::Params",
        );
        let data = DynamicData::Aggregate(vec![(
            "a".to_string(),
            DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, 1),
        )]);
        let value = DynamicRpcValue::new(Arc::clone(&ast), "M::Params".to_string(), data);
        let mut bytes = Vec::new();
        value.encode(&mut bytes).unwrap();
        let _ = with_decode_context(&ast, "M::Params", Dialect::default(), || {
            DynamicRpcValue::decode(&bytes)
        });
        // Outside the closure, no context is set — the next bare decode
        // (as the daemon's own request handler will see it between two
        // unrelated calls) fails loudly instead of reusing a stale one.
        let err = DynamicRpcValue::decode(&bytes).unwrap_err();
        assert!(matches!(err, DecodeError::Invalid { .. }));
    }
}
