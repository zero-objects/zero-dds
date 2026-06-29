// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DdsType` — the trait that user types must implement to be sent
//! over DDS.
//!
//! # Usage
//!
//! User types implement the trait either by hand or via the codegen
//! pipeline `zerodds-idl-rust` (IDL → Rust with a derived `DdsType`
//! impl). The encoder/decoder pairs follow the XCDR2 convention (see
//! `zerodds-cdr`); the trait stays transport- and QoS-agnostic.
//!
//! # Interop note
//!
//! For interop with Cyclone/Fast-DDS, the `TYPE_NAME` MUST match the
//! remote topic type name exactly (strict equality). IDL type
//! namespacing (e.g. `std_msgs::msg::String`) must be taken into
//! account.

extern crate alloc;
use alloc::vec::Vec;

pub use zerodds_cdr::{KEY_HASH_LEN, PlainCdr2BeKeyHolder, compute_key_hash};

/// XTypes 1.3 §7.4.5 struct extensibility kind. Wire-relevant
/// information for the sample encoder; mirrors the IDL annotations
/// `@final` / `@appendable` / `@mutable`.
///
/// Spec: `zerodds-xcdr2-rust` §2 references this as
/// `ExtensibilityKind`; the implementation name `Extensibility` and
/// the spec-aligned alias [`ExtensibilityKind`] are identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Extensibility {
    /// `@final`: tight-packed body, no header.
    Final = 0,
    /// `@appendable`: 4-byte DHEADER + body, forward-compatible.
    Appendable = 1,
    /// `@mutable`: one EMHEADER + body per member.
    Mutable = 2,
}

/// Spec-aligned alias: `zerodds-xcdr2-rust` §2 references the
/// Extensibility enum under the name `ExtensibilityKind`. We keep
/// `Extensibility` as the implementation name; both are identical via
/// the alias.
pub type ExtensibilityKind = Extensibility;

/// A type that can be published/subscribed via DDS.
pub trait DdsType: Sized {
    /// Fully-qualified topic type name (e.g. `"std_msgs::String"`).
    /// Must match the peer type name exactly (strict matching).
    const TYPE_NAME: &'static str;

    /// XTypes 1.3 §7.4.5 struct extensibility kind. Default `Final`
    /// for backwards compat with pre-`EXTENSIBILITY` codegen outputs.
    /// Spec: zerodds-xcdr2-rust §2.3.
    const EXTENSIBILITY: Extensibility = Extensibility::Final;

    /// `true` if the topic type is **keyed** (at least one member with
    /// a `@key` annotation). Default `false` — the caller (proc-macro)
    /// overrides this for keyed types and also implements
    /// [`Self::encode_key_holder_be`].
    ///
    /// Spec: XTypes 1.3 §7.6.8 (KeyHash requirement for keyed topics).
    ///
    /// Note (`zerodds-xcdr2-rust` §11 errata): the spec references this
    /// field as `IS_KEYED`. We keep `HAS_KEY` for source compat with
    /// pre-1.0 code; the spec-aligned alias [`Self::IS_KEYED`] always
    /// returns the same value.
    const HAS_KEY: bool = false;

    /// Spec-aligned alias for [`Self::HAS_KEY`].
    /// `zerodds-xcdr2-rust` §2 references this as `IS_KEYED`.
    const IS_KEYED: bool = Self::HAS_KEY;

    /// Maximum size of the PLAIN_CDR2-BE KeyHolder stream in bytes
    /// (XTypes 1.3 §7.6.8.4 step 5). `None` = not keyed or unbounded
    /// (MD5 path). `Some(n)` with `n <= 16` = zero-pad path.
    const KEY_HOLDER_MAX_SIZE: Option<usize> = None;

    /// `true` if the type is annotated with `@nested` (XTypes 1.3
    /// §7.4.6.3.5). Nested types are only intended as members of other
    /// types and MUST NOT be registered as a DDS topic type.
    /// `DomainParticipant::create_topic` rejects registration of
    /// nested types with `PreconditionNotMet`.
    const IS_NESTED: bool = false;

    /// XTypes 1.3 §7.3.4.2 — TypeIdentifier of the type for
    /// XTypes-aware discovery + compatibility matching. Default
    /// `TypeIdentifier::None` signals "type-id not provided;
    /// reader-writer matching falls back to plain `type_name`
    /// comparison (DDS 1.4 §2.2.3 default path)".
    ///
    /// idl-rust codegen emits the appropriate TypeIdentifier here:
    /// - Primitive `int32` → `TypeIdentifier::Primitive(PrimitiveKind::Int32)`,
    /// - String `string<N>` → `TypeIdentifier::String8Small{ bound }`,
    /// - Composite struct → `TypeIdentifier::EquivalenceHash` (once the
    ///   TypeRegistry lookup is live).
    ///
    /// Once both sides (writer + reader) provide a TypeIdentifier, the
    /// subscriber match path calls
    /// [`zerodds_types::type_matcher::TypeMatcher::match_types`]
    /// (XTypes §7.6.3.7 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement).
    const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier = zerodds_types::TypeIdentifier::None;

    /// Serializes `self` into the XCDR2 payload sent as the
    /// `serialized_payload` of a DATA submessage. Default endianness:
    /// little-endian (RTPS 2.5 §10.5
    /// `RepresentationIdentifier = CDR2_LE = 0x0010`).
    ///
    /// # Errors
    /// CDR encoder error (buffer overflow, etc.).
    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError>;

    /// Big-endian variant of [`Self::encode`]. The default
    /// implementation delegates to [`Self::encode`] (no byte swap),
    /// since a generic BE re-encode is not possible without type
    /// reflection. Codegen overrides this for structures that should
    /// genuinely go on the wire as BE. Spec: zerodds-xcdr2-rust §2.4.
    ///
    /// # Errors
    /// CDR encoder error.
    fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode(out)
    }

    /// Deserializes a little-endian XCDR2 payload. The caller ensures that
    /// `bytes` contains the full sample payload (encapsulation header already
    /// stripped).
    ///
    /// # Errors
    /// CDR decoder error (truncation, unexpected bytes, etc.).
    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError>;

    /// Big-endian variant of [`Self::decode`] — for a payload whose
    /// encapsulation header declared a big-endian representation identifier
    /// (`CDR_BE = 0x0000`, `PL_CDR_BE = 0x0002`, `CDR2_BE = 0x0006`,
    /// `D_CDR2_BE = 0x0008`, `PL_CDR2_BE = 0x000a`). The default implementation
    /// delegates to [`Self::decode`] (little-endian) so pre-`decode_be` codegen
    /// keeps working on the canonical wire; idl-rust codegen overrides this to
    /// build a big-endian reader. Symmetric to [`Self::encode_be`]. Spec:
    /// zerodds-xcdr2-rust §2.4; RTPS 2.5 §10.5.
    ///
    /// # Errors
    /// CDR decoder error.
    fn decode_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode(bytes)
    }

    /// Serializes as **classic CDR / XCDR1** little-endian (representation
    /// `CDR_LE = 0x0001`, max-alignment 8, no DHEADER on @final/@appendable,
    /// PL_CDR1 for @mutable). This is the encoding Cyclone DDS carries in an
    /// iceoryx PSMX chunk for a serialized sample, so the iceoryx-cyclone
    /// bridge uses it for cross-vendor same-host interop. The default delegates
    /// to [`Self::encode`] (XCDR2) — correct only for types whose XCDR1 and
    /// XCDR2 byte streams coincide (no 8-byte members, @final, no @mutable);
    /// idl-rust codegen overrides it with a true XCDR1 writer.
    ///
    /// # Errors
    /// CDR encoder error.
    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        self.encode(out)
    }

    /// Deserializes a classic-CDR / XCDR1 little-endian payload. Symmetric to
    /// [`Self::encode_xcdr1`]; the default delegates to [`Self::decode`] and
    /// idl-rust codegen overrides it with an XCDR1 reader.
    ///
    /// # Errors
    /// CDR decoder error.
    fn decode_xcdr1(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode(bytes)
    }

    /// Deserializes a classic-CDR / XCDR1 **big-endian** payload (encapsulation
    /// `CDR_BE = 0x0000` / `PL_CDR_BE = 0x0002`). The default delegates to
    /// [`Self::decode_xcdr1`] (little-endian) — correct for byte-order-agnostic
    /// types and the common case (Cyclone/FastDDS/RTI emit XCDR1 little-endian);
    /// idl-rust codegen overrides it with a true big-endian XCDR1 reader.
    /// Symmetric to [`Self::decode_be`] for XCDR2.
    ///
    /// # Errors
    /// CDR decoder error.
    fn decode_xcdr1_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Self::decode_xcdr1(bytes)
    }

    /// Serializes the `@key` member values in **PLAIN_CDR2-BE** format
    /// into the given [`PlainCdr2BeKeyHolder`]. Order: ascending by
    /// `member_id` (XTypes 1.3 §7.6.8.3.1.b).
    ///
    /// **Default implementation**: empty write. Keyed types MUST
    /// override this.
    ///
    /// Called by the DcpsRuntime in the sample-encode path to write
    /// PID_KEY_HASH into the inline QoS.
    fn encode_key_holder_be(&self, _holder: &mut PlainCdr2BeKeyHolder) {
        // Default: no key. Keyed types override.
    }

    /// Returns the value of a field path (dotted, e.g. `"a.b"`) as a
    /// `zerodds_sql_filter::Value` for SQL filter evaluation in
    /// QueryCondition / ContentFilteredTopic. Default: `None` (no field
    /// reachable — the filter then denies every sample that contains a
    /// field access).
    ///
    /// Spec: DDS 1.4 §B.2.1 (Filter Expressions) together with
    /// §2.2.2.5.9 (QueryCondition) and §2.2.2.3.5
    /// (ContentFilteredTopic). Generated IDL stubs override this per
    /// field.
    #[must_use]
    fn field_value(&self, _path: &str) -> Option<zerodds_sql_filter::Value> {
        None
    }

    /// Computes the 16-byte KeyHash of this instance per XTypes 1.3
    /// §7.6.8.4. `None` if `HAS_KEY = false`.
    ///
    /// The default implementation uses [`Self::encode_key_holder_be`] +
    /// [`Self::KEY_HOLDER_MAX_SIZE`] and delegates to
    /// [`compute_key_hash`].
    #[must_use]
    fn compute_key_hash(&self) -> Option<[u8; KEY_HASH_LEN]> {
        if !Self::HAS_KEY {
            return None;
        }
        let mut holder = PlainCdr2BeKeyHolder::new();
        self.encode_key_holder_be(&mut holder);
        let max = Self::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
        Some(compute_key_hash(holder.as_bytes(), max))
    }

    /// Spec-aligned alias for [`Self::compute_key_hash`].
    /// `zerodds-xcdr2-rust` §2.5 uses the name `key_hash`; the
    /// implementation name keeps `compute_key_hash` for historical
    /// compat. Both return the same value.
    #[must_use]
    fn key_hash(&self) -> Option<[u8; KEY_HASH_LEN]> {
        self.compute_key_hash()
    }
}

/// `RowAccess` adapter for a `DdsType` sample value. Used by the
/// DataReader in `read_w_condition`/`take_w_condition` and by the
/// `ContentFilteredTopic` filter.
pub struct DdsTypeRow<'a, T: DdsType> {
    /// Inner sample whose fields are queried via
    /// [`DdsType::field_value`].
    pub sample: &'a T,
}

impl<'a, T: DdsType> DdsTypeRow<'a, T> {
    /// Constructor.
    #[must_use]
    pub fn new(sample: &'a T) -> Self {
        Self { sample }
    }
}

impl<T: DdsType> zerodds_sql_filter::RowAccess for DdsTypeRow<'_, T> {
    fn get(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        self.sample.field_value(path)
    }
}

/// Placeholder error for DdsType::encode. In v1.3 this will be
/// re-exported as `zerodds_cdr::EncodeError` once the CDR layer is
/// stabilized from the DCPS perspective.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// Buffer overflow or field-specific value-range error.
    Invalid {
        /// Static description.
        what: &'static str,
    },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid { what } => write!(f, "encode error: {what}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

impl From<zerodds_cdr::EncodeError> for EncodeError {
    fn from(e: zerodds_cdr::EncodeError) -> Self {
        // zerodds-cdr errors are passed through as an opaque `Invalid`
        // wrap. That is sufficient for DdsType callers, who only need
        // the "encoding failed" information — the detailed error
        // structure lives in the cdr layer and is serialized via
        // Display when a caller logs the error message.
        let _ = e;
        Self::Invalid {
            what: "zerodds_cdr encode error",
        }
    }
}

/// Placeholder error for DdsType::decode.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// Truncation or value out-of-range.
    Invalid {
        /// Static description.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid { what } => write!(f, "decode error: {what}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

impl From<zerodds_cdr::DecodeError> for DecodeError {
    fn from(e: zerodds_cdr::DecodeError) -> Self {
        let _ = e;
        Self::Invalid {
            what: "zerodds_cdr decode error",
        }
    }
}

// ---------------------------------------------------------------------
// DdsAny — IDL `any` type erasure (XCDR2 §7.4.4.7)
//
// Wire format: TypeIdentifier header (CDR string) + payload bytes.
// Pure Rust with no external crate dep; full type erasure via a
// string tag.

/// IDL `any` as a type-erasure wrapper. Carries a type-identifier
/// string (e.g. `"std_msgs::Header"`) plus the payload bytes.
///
/// Consumer pattern: check `type_name`, then deserialize the `payload`
/// with the concrete DdsType.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DdsAny {
    /// Fully-qualified type name (matches `DdsType::TYPE_NAME`).
    pub type_name: alloc::string::String,
    /// XCDR2 payload bytes of the wrapped value.
    pub payload: Vec<u8>,
}

impl DdsAny {
    /// Constructs a `DdsAny` from a `DdsType` value.
    ///
    /// # Errors
    /// `EncodeError` on encode failure.
    pub fn pack<T: DdsType>(value: &T) -> Result<Self, EncodeError> {
        let mut payload = Vec::new();
        value.encode(&mut payload)?;
        Ok(Self {
            type_name: alloc::string::String::from(T::TYPE_NAME),
            payload,
        })
    }

    /// Attempts to unpack the wrapped value as `T`.
    ///
    /// # Errors
    /// `DecodeError::Invalid` if `T::TYPE_NAME != self.type_name` or on
    /// a decode error.
    pub fn unpack<T: DdsType>(&self) -> Result<T, DecodeError> {
        if self.type_name != T::TYPE_NAME {
            return Err(DecodeError::Invalid {
                what: "DdsAny: type-name mismatch",
            });
        }
        T::decode(&self.payload)
    }
}

impl zerodds_cdr::CdrEncode for DdsAny {
    fn encode(
        &self,
        w: &mut zerodds_cdr::BufferWriter,
    ) -> core::result::Result<(), zerodds_cdr::EncodeError> {
        // Type name as a CDR string + payload bytes with a u32 length prefix.
        w.write_string(&self.type_name)?;
        let payload_len = u32::try_from(self.payload.len()).map_err(|_| {
            zerodds_cdr::EncodeError::ValueOutOfRange {
                message: "DdsAny: payload > u32::MAX",
            }
        })?;
        w.write_u32(payload_len)?;
        w.write_bytes(&self.payload)?;
        Ok(())
    }
}

impl zerodds_cdr::CdrDecode for DdsAny {
    fn decode(
        r: &mut zerodds_cdr::BufferReader<'_>,
    ) -> core::result::Result<Self, zerodds_cdr::DecodeError> {
        let type_name = r.read_string()?;
        let payload_len = r.read_u32()? as usize;
        let payload = r.read_bytes(payload_len)?.to_vec();
        Ok(Self { type_name, payload })
    }
}

// ---------------------------------------------------------------------
// Built-in `DdsType` for &[u8]/Vec<u8> payloads
//
// Many ROS use cases and interop tests need to "pass through raw". A
// `BytesPayload` newtype with a fixed type name allows that.
// ---------------------------------------------------------------------

/// An opaque raw byte payload with a configurable type name (via an
/// `impl` of `BytesPayload<T>` or a newtype).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBytes {
    /// Payload bytes (placed on the wire as-is, no CDR framing).
    pub data: Vec<u8>,
}

impl RawBytes {
    /// Constructor.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl DdsType for RawBytes {
    const TYPE_NAME: &'static str = "zerodds::RawBytes";

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            data: bytes.to_vec(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn raw_bytes_roundtrip() {
        let orig = RawBytes::new(vec![1, 2, 3, 4, 5]);
        let mut buf = Vec::new();
        orig.encode(&mut buf).unwrap();
        let back = RawBytes::decode(&buf).unwrap();
        assert_eq!(back, orig);
    }

    #[test]
    fn raw_bytes_type_name_is_namespaced() {
        assert_eq!(RawBytes::TYPE_NAME, "zerodds::RawBytes");
    }

    // ---- .B: keyed types + KeyHash ----

    /// Test fixture: keyed topic with @key u32 id (max 4 byte → zero-pad).
    struct SmallKeyed {
        id: u32,
    }

    impl DdsType for SmallKeyed {
        const TYPE_NAME: &'static str = "test::SmallKeyed";
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);

        fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            out.extend_from_slice(&self.id.to_le_bytes());
            Ok(())
        }
        fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            if bytes.len() < 4 {
                return Err(DecodeError::Invalid {
                    what: "truncated SmallKeyed",
                });
            }
            let id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(Self { id })
        }
        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            holder.write_u32(self.id);
        }
    }

    #[test]
    fn small_keyed_produces_zero_padded_keyhash() {
        let s = SmallKeyed { id: 0x1122_3344 };
        let key = s.compute_key_hash().expect("keyed");
        assert_eq!(&key[0..4], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&key[4..16], &[0u8; 12]);
    }

    #[test]
    fn non_keyed_returns_none_for_keyhash() {
        let r = RawBytes::new(vec![1, 2, 3]);
        assert_eq!(r.compute_key_hash(), None);
    }

    #[test]
    fn keyed_two_instances_have_distinct_hashes() {
        let a = SmallKeyed { id: 1 };
        let b = SmallKeyed { id: 2 };
        assert_ne!(a.compute_key_hash(), b.compute_key_hash());
    }

    /// Test fixture: keyed topic with an unbounded @key string (MD5 path).
    struct LargeKeyed {
        topic: alloc::string::String,
    }

    impl DdsType for LargeKeyed {
        const TYPE_NAME: &'static str = "test::LargeKeyed";
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = None; // unbounded → MD5

        fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            out.extend_from_slice(self.topic.as_bytes());
            Ok(())
        }
        fn decode(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            Err(DecodeError::Invalid {
                what: "test fixture",
            })
        }
        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            holder.write_string(&self.topic);
        }
    }

    #[test]
    fn large_keyed_produces_md5_hashed_keyhash() {
        let s = LargeKeyed {
            topic: alloc::string::String::from("hello"),
        };
        let key = s.compute_key_hash().expect("keyed");
        // 16-byte deterministic hash, non-zero
        assert_ne!(key, [0u8; 16]);
        // Idempotent
        let key2 = s.compute_key_hash().expect("keyed");
        assert_eq!(key, key2);
    }

    #[test]
    fn spec_aligned_aliases_match_implementation_names() {
        // zerodds-xcdr2-rust §11 Errata.
        assert_eq!(
            <RawBytes as DdsType>::IS_KEYED,
            <RawBytes as DdsType>::HAS_KEY
        );
        fn is_keyed<T: DdsType>() -> bool {
            T::IS_KEYED
        }
        assert!(is_keyed::<SmallKeyed>());
        let s = SmallKeyed { id: 0xABCD };
        assert_eq!(s.key_hash(), s.compute_key_hash());
    }

    #[test]
    fn extensibility_default_is_final() {
        assert_eq!(<RawBytes as DdsType>::EXTENSIBILITY, Extensibility::Final);
        // ExtensibilityKind alias is the same type.
        let _: ExtensibilityKind = Extensibility::Mutable;
    }

    #[test]
    fn encode_be_default_delegates_to_encode() {
        let r = RawBytes::new(vec![1, 2, 3]);
        let mut le = Vec::new();
        let mut be = Vec::new();
        r.encode(&mut le).unwrap();
        r.encode_be(&mut be).unwrap();
        assert_eq!(le, be);
    }

    #[test]
    fn keyed_member_order_matters() {
        // Hypothetically: two members in a different order would yield
        // different hashes. We verify this with a mock type that writes
        // two fields in reverse order.
        struct A {
            x: u32,
            y: u32,
        }
        impl DdsType for A {
            const TYPE_NAME: &'static str = "test::A";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(8);
            fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> Result<Self, DecodeError> {
                Err(DecodeError::Invalid { what: "stub" })
            }
            fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
                holder.write_u32(self.x);
                holder.write_u32(self.y);
            }
        }
        struct B {
            x: u32,
            y: u32,
        }
        impl DdsType for B {
            const TYPE_NAME: &'static str = "test::B";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(8);
            fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> Result<Self, DecodeError> {
                Err(DecodeError::Invalid { what: "stub" })
            }
            fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
                holder.write_u32(self.y);
                holder.write_u32(self.x);
            }
        }
        let a = A { x: 1, y: 2 };
        let b = B { x: 1, y: 2 };
        assert_ne!(a.compute_key_hash(), b.compute_key_hash());
    }
}
