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

pub use crate::key_hash::{KEY_HASH_LEN, PlainCdr2BeKeyHolder, compute_key_hash, keyhash_cdr2_be};

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

/// Octet length of a DHEADER — the `uint32` that XCDR2 puts in front of a
/// DELIMITED_CDR (`@appendable`) or PL_CDR2 (`@mutable`) object and states the
/// length of its content in. XTypes 1.3 §7.4.3.4.2.
pub const DHEADER_LEN: usize = 4;

/// A type that can be published/subscribed via DDS.
pub trait DdsType: Sized {
    /// Fully-qualified topic type name (e.g. `"std_msgs::String"`).
    /// Must match the peer type name exactly (strict matching).
    const TYPE_NAME: &'static str;

    /// The topic type name this endpoint announces.
    ///
    /// Defaults to [`Self::TYPE_NAME`], which is what a compiled-in type
    /// has: the generator knows the name and writes it into the `impl`.
    ///
    /// A type that is not compiled in does not. XTypes 1.3 §7.5 has a
    /// participant construct a type at run time — from a `TypeObject` a
    /// peer announced (§7.6.3.2), from an XML type description (§7.3.2),
    /// from IDL a tool read after the binary was built — and register it
    /// as a topic type under the name that description carries. That name
    /// is not a constant, so an implementation whose only spelling of it
    /// is an associated `const` cannot express such a type at all. The
    /// five run-time accessors here — this one, [`Self::type_identifier`],
    /// [`Self::extensibility`], [`Self::has_key`] and
    /// [`Self::key_holder_max_size`] — are what the DCPS entities read, so
    /// a type whose description arrives at run time answers them from what
    /// it read rather than from what a generator wrote.
    ///
    /// Every existing implementation keeps the constant it already has:
    /// the defaults forward to it.
    #[must_use]
    fn type_name() -> &'static str {
        Self::TYPE_NAME
    }

    /// The `TypeIdentifier` this endpoint announces in SEDP
    /// (`PID_TYPE_INFORMATION`, XTypes 1.3 §7.6.3.2). Defaults to
    /// [`Self::TYPE_IDENTIFIER`]. See [`Self::type_name`] for why this is
    /// a function.
    #[must_use]
    fn type_identifier() -> zerodds_types::TypeIdentifier {
        Self::TYPE_IDENTIFIER.clone()
    }

    /// The extensibility this endpoint frames its samples with and
    /// declares in the encapsulation header (XTypes 1.3 §7.6.3.1.2).
    /// Defaults to [`Self::EXTENSIBILITY`]. See [`Self::type_name`].
    #[must_use]
    fn extensibility() -> Extensibility {
        Self::EXTENSIBILITY
    }

    /// Whether the topic type is keyed (XTypes 1.3 §7.6.8). Defaults to
    /// [`Self::HAS_KEY`]. See [`Self::type_name`].
    #[must_use]
    fn has_key() -> bool {
        Self::HAS_KEY
    }

    /// Largest serialized sample of the type. Defaults to
    /// [`Self::MAX_SERIALIZED_SIZE`].
    #[must_use]
    fn max_serialized_size() -> Option<usize> {
        Self::MAX_SERIALIZED_SIZE
    }

    /// Maximum size of the PLAIN_CDR2-BE KeyHolder stream (XTypes 1.3
    /// §7.6.8.4 step 5). Defaults to [`Self::KEY_HOLDER_MAX_SIZE`]. See
    /// [`Self::type_name`].
    #[must_use]
    fn key_holder_max_size() -> Option<usize> {
        Self::KEY_HOLDER_MAX_SIZE
    }

    /// XTypes 1.3 §7.4.5 struct extensibility kind. Default `Final`
    /// for backwards compat with pre-`EXTENSIBILITY` codegen outputs.
    /// Spec: zerodds-xcdr2-rust §2.3.
    const EXTENSIBILITY: Extensibility = Extensibility::Final;

    /// Asserts that this type's **member** octets are framed the same way under
    /// XCDR1 (classic CDR) and XCDR2, so the [`Self::encode_xcdr1`] /
    /// [`Self::decode_xcdr1`] defaults may build the XCDR1 stream from the XCDR2
    /// codec instead of refusing.
    ///
    /// The claim is about everything *inside* the top-level object; the
    /// extensibility framing around it is not part of it, because the trait knows
    /// [`Self::EXTENSIBILITY`] itself and reframes accordingly (see
    /// [`Self::xcdr1_delegation_is_sound`] and
    /// [`Self::xcdr1_reframes_dheader`]). The member octets coincide only when
    /// none of the constructs that XTypes 1.3 frames differently between the
    /// versions occurs in the type:
    ///
    /// * an 8-octet primitive (`int64`/`uint64`/`double`) or a member containing
    ///   one — XCDR1's MAXALIGN is 8, XCDR2 caps alignment at 4 (§7.4.1.1.1), so
    ///   the padding differs.
    /// * a sequence or array whose element is not a primitive — XCDR2 prefixes a
    ///   DHEADER (§7.4.3.5.3).
    /// * a nested `@appendable`/`@mutable` member — its own DHEADER resp.
    ///   EMHEADERs sit inside the member octets, and only the top-level framing
    ///   is reframed here.
    /// * `wstring` / `wchar` — the unit width and framing differ between the
    ///   versions.
    ///
    /// Default `false`. A type that neither overrides the XCDR1 codec nor asserts
    /// the equivalence has no XCDR1 representation, and the write path refuses
    /// rather than shipping XCDR2 octets under a `CDR_LE` encapsulation.
    const XCDR1_MATCHES_XCDR2: bool = false;

    /// Whether the [`Self::encode_xcdr1`] / [`Self::decode_xcdr1`] defaults may
    /// delegate to the XCDR2 codec **unchanged**.
    ///
    /// True for `@final`, whose XCDR1 form (PLAIN_CDR) and XCDR2 form
    /// (PLAIN_CDR2) both carry the bare members, XTypes 1.3 Table 60.
    #[must_use]
    fn xcdr1_delegation_is_sound() -> bool {
        Self::XCDR1_MATCHES_XCDR2 && matches!(Self::EXTENSIBILITY, Extensibility::Final)
    }

    /// Whether the XCDR1 defaults reach the XCDR2 codec by removing resp. adding
    /// the top-level DHEADER.
    ///
    /// True for `@appendable`. XTypes 1.3 Table 60 gives that extensibility
    /// PLAIN_CDR under XCDR1 and DELIMITED_CDR under XCDR2: the member octets are
    /// the same, and the only difference is the 4-octet DHEADER XCDR2 puts in
    /// front of them. Because [`Self::XCDR1_MATCHES_XCDR2`] rules out any member
    /// that needs an alignment wider than 4, shifting the whole body by those 4
    /// octets changes no padding, so the two forms convert into each other
    /// exactly.
    ///
    /// `@mutable` is not included: XCDR1 frames it as PL_CDR with 16-bit PIDs and
    /// XCDR2 as PL_CDR2 with EMHEADERs, which is a different member framing and
    /// not a missing prefix.
    #[must_use]
    fn xcdr1_reframes_dheader() -> bool {
        Self::XCDR1_MATCHES_XCDR2 && matches!(Self::EXTENSIBILITY, Extensibility::Appendable)
    }

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

    /// Largest serialized sample of the type, in octets, without the
    /// 4-octet encapsulation header; `None` for a type of unbounded or
    /// unknown size. A writer announces it, with the header added, as
    /// `dataMaxSizeSerialized` (DDSI-RTPS 2.5 Table 8.53, `PID_TYPE_MAX_SIZE_SERIALIZED`).
    const MAX_SERIALIZED_SIZE: Option<usize> = None;

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
    /// idl-rust codegen (`type_identifier.rs`) emits a codegen-time-computed
    /// `EquivalenceHashComplete` of the struct's `CompleteStructType` here
    /// for essentially every struct shape (#24 fix — previously only
    /// primitive/string-only-member structs got a non-`None` id; a single
    /// typedef/enum/sequence/map/nested-struct/array member nulled the
    /// *whole* struct's identity, since the resolution was a self-contained
    /// per-struct subset that could not look up other named types). Member
    /// types are now resolved against the full `Specification`'s named-type
    /// registry (`zerodds_idl::semantics::build_type_registry`, the same
    /// mapper the AST → `TypeObject` pipeline uses), so the emitted id is
    /// non-`None` for typedef/enum/sequence/map/nested-struct/array members
    /// too. It only stays `None` for a struct that (transitively) contains a
    /// still-genuinely-unmappable `TypeSpec` (`fixed<>`/`any`).
    ///
    /// Once both sides (writer + reader) provide a TypeIdentifier, the
    /// subscriber match path calls
    /// [`zerodds_types::type_matcher::TypeMatcher::match_types`]
    /// (XTypes §7.6.3.4 + DDS 1.4 §2.2.3 TypeConsistencyEnforcement). As of
    /// the Gap 2 (#24) fix the runtime match resolves `EquivalenceHash` ids
    /// against the runtime's POPULATED [`zerodds_types::resolve::TypeRegistry`]
    /// (owned by the TypeLookup server, filled by
    /// [`DcpsRuntime::register_type_object`](crate::runtime::DcpsRuntime::register_type_object)
    /// and by TypeLookup `getTypes` replies): identical hashes match by
    /// identity (short-circuit, before any lookup); differing hashes are
    /// compared STRUCTURALLY once both `TypeObject`s are in the registry, so
    /// provably-compatible evolutions (added/reordered/widening-safe members
    /// under `@appendable`) now match. A remote id ABSENT from the registry
    /// triggers a `getTypes` fetch: under `force_type_validation` the match
    /// DEFERS until the reply lands (no optimistic match); otherwise it
    /// matches optimistically and confirms asynchronously. See
    /// `wire_reader_to_remote_writer` / `wire_writer_to_remote_reader`.
    const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier = zerodds_types::TypeIdentifier::None;

    /// XTypes 1.3 §7.3.4 — the full `TypeObject` for this type, if the
    /// codegen (or a hand impl) can provide it. Gap 2 (#24) auto-registration
    /// hook: `Publisher::create_datawriter` / `Subscriber::create_datareader`
    /// register the returned object with the runtime's TypeLookup server
    /// (`DcpsRuntime::register_type_object`) at endpoint creation, so
    ///   * remote peers can fetch it via `getTypes`, and
    ///   * the local runtime match sites can resolve this endpoint's
    ///     `TYPE_IDENTIFIER` structurally against the registry.
    ///
    /// It also decides what a `DataWriter<Self>::dispose`/
    /// `unregister_instance` puts on the wire (`DcpsRuntime::write_user_lifecycle`):
    /// with a registered `CompleteTypeObject`, it sends the instance's
    /// `serializedKey` (RTPS 2.5 §9.4.2.11) alongside `PID_KEY_HASH`, the
    /// form both RTI Connext 7.7.0 and CoreDX 6.15.3 act on. Without one
    /// (the `None` default below), it falls back to naming the instance
    /// by `PID_KEY_HASH` alone — which Eclipse Cyclone 11.0.1, Fast DDS
    /// 3.6.1 and OpenDDS 3.35 do not read as a dispose/unregister at all
    /// (`Test_FinalInstanceState_1`, measured 0/4 disposed on each,
    /// against 4/4 for RTI and CoreDX): a peer running one of those three
    /// silently never learns the instance is gone. `DcpsRuntime` logs this
    /// fallback once per writer (`writer.lifecycle.serialized_key_unavailable`,
    /// Warn) rather than only on the wire.
    ///
    /// Default `None` = "no object emitted": the endpoint still carries its
    /// `TYPE_IDENTIFIER`, and matching falls back to identity / TypeLookup
    /// fetch from the peer. `zerodds-build`'s generated `DdsType` impl
    /// provides this method (`crates/zerodds-build/src/dds_type.rs`
    /// emits it from the IDL model's `CompleteTypeObject`, omitted only
    /// if that object fails its own encode round trip); a hand-written
    /// `DdsType`, or one generated by an older `zerodds-build`, may still
    /// leave it at the default.
    #[must_use]
    fn type_object() -> Option<zerodds_types::type_object::TypeObject> {
        None
    }

    /// The `TypeObject`s that [`Self::type_object`] names by hash,
    /// transitively: a structure's base type, the aggregate, enumeration,
    /// bitmask, union and alias types its members reach. Registered
    /// beside the type's own object at endpoint creation, for the same
    /// two reasons (XTypes 1.3 §7.3.4.6, §7.6.3.3.4): a peer's
    /// `getTypeDependencies` has to be answerable, and the local match
    /// site has to be able to resolve this endpoint's own closure. An
    /// object registered without its closure resolves to nothing, and
    /// the runtime then cannot decide §7.6.3.4.2 for it at all.
    ///
    /// Default empty: a type that names no other type by hash has no
    /// dependencies. Codegen fills this in from the model.
    #[must_use]
    fn type_object_dependencies() -> Vec<zerodds_types::type_object::TypeObject> {
        Vec::new()
    }

    /// The strongly connected component this type belongs to, in both
    /// forms, when [`Self::TYPE_IDENTIFIER`] is a
    /// `TI_STRONGLY_CONNECTED_COMPONENT` identifier (XTypes 1.3 §7.3.4.9).
    ///
    /// A recursive type has no hash of its own: the hash belongs to the
    /// component, and a member is named by that hash and its position.
    /// [`Self::type_object`] cannot register such a member under a hash
    /// of its object, so the component is handed over whole and registered
    /// as one — the TypeLookup server serves it atomically and the
    /// TypeInformation announces the whole component (§7.6.3.2.1).
    ///
    /// Default `None`: a type outside every component.
    #[must_use]
    fn type_component() -> Option<zerodds_types::resolve::TypeComponent> {
        None
    }

    /// Serializes `self` into the XCDR2 payload sent as the
    /// `serialized_payload` of a DATA submessage. Default endianness:
    /// little-endian (RTPS 2.5 §10.5
    /// `RepresentationIdentifier = CDR2_LE = 0x0010`).
    ///
    /// # Errors
    /// CDR encoder error (buffer overflow, etc.).
    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError>;

    /// Whether this type's little-endian stream is also its big-endian one:
    /// a stream that holds no multi-octet value of the type's own, such as
    /// opaque bytes the application serialized itself.
    ///
    /// The big-endian defaults below delegate to the little-endian codec
    /// only under this assertion and refuse otherwise. They delegated
    /// unconditionally, so a type with no big-endian codec wrote
    /// little-endian octets under a `CDR2_BE`/`D_CDR2_BE`/`PL_CDR2_BE`
    /// encapsulation and read a big-endian peer's sample as byte-swapped
    /// field values, with no error on either side — the fault the XCDR1
    /// pair below already refuses. Generated types (`zerodds-build`)
    /// override both and are not affected.
    const BYTE_ORDER_AGNOSTIC: bool = false;

    /// Big-endian variant of [`Self::encode`]. Spec: zerodds-xcdr2-rust
    /// §2.4. The default delegates to [`Self::encode`] only when
    /// [`Self::BYTE_ORDER_AGNOSTIC`] is asserted: a big-endian re-encode
    /// needs the type's members, which only the type's own codec knows.
    ///
    /// # Errors
    /// `EncodeError::Invalid` when the type has no big-endian encoder and is
    /// not byte-order agnostic; otherwise the CDR encoder error.
    fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        if Self::BYTE_ORDER_AGNOSTIC {
            return self.encode(out);
        }
        Err(EncodeError::Invalid {
            what: "big-endian XCDR2 encoding not implemented for this type",
        })
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
    /// `D_CDR2_BE = 0x0008`, `PL_CDR2_BE = 0x000a`). Symmetric to
    /// [`Self::encode_be`]: the default reads through [`Self::decode`] only
    /// when [`Self::BYTE_ORDER_AGNOSTIC`] is asserted, because otherwise
    /// every multi-octet field would come back byte-swapped without an
    /// error. Spec: zerodds-xcdr2-rust §2.4; RTPS 2.5 §10.5.
    ///
    /// # Errors
    /// `DecodeError::Invalid` when the type has no big-endian decoder and is
    /// not byte-order agnostic; otherwise the CDR decoder error.
    fn decode_be(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        if Self::BYTE_ORDER_AGNOSTIC {
            return Self::decode(bytes);
        }
        Err(DecodeError::Invalid {
            what: "big-endian XCDR2 decoding not implemented for this type",
        })
    }

    /// Serializes as **classic CDR / XCDR1** little-endian (representation
    /// `CDR_LE = 0x0001`, max-alignment 8, no DHEADER on @final/@appendable,
    /// PL_CDR1 for @mutable). This is the encoding Cyclone DDS carries in an
    /// iceoryx PSMX chunk for a serialized sample, so the iceoryx-cyclone
    /// bridge uses it for cross-vendor same-host interop; it is also what a
    /// writer emits whenever the effective `DataRepresentation` offer starts
    /// with XCDR1 (the DDS/XTypes default, `data_representation::XCDR`).
    ///
    /// The default implementation reaches [`Self::encode`] only when
    /// [`Self::XCDR1_MATCHES_XCDR2`] is asserted: unchanged for `@final`
    /// ([`Self::xcdr1_delegation_is_sound`]), and with the top-level DHEADER
    /// removed for `@appendable` ([`Self::xcdr1_reframes_dheader`], XTypes 1.3
    /// Table 60 — PLAIN_CDR carries no DHEADER). Otherwise it refuses.
    /// Delegating unconditionally would put XCDR2 octets on the wire under a
    /// `CDR_LE` encapsulation header — a peer would parse them with the wrong
    /// alignment or take the DHEADER for a member value, and neither side would
    /// report anything. A type that cannot be written as XCDR1 must say so; it
    /// must not be silently written as something else.
    ///
    /// # Errors
    /// `EncodeError::Invalid` when the type declares no XCDR1 encoding, or when
    /// an `@appendable` type's XCDR2 stream is too short to hold the DHEADER it
    /// must begin with; otherwise the underlying CDR encoder error.
    fn encode_xcdr1(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        if Self::xcdr1_delegation_is_sound() {
            return self.encode(out);
        }
        if Self::xcdr1_reframes_dheader() {
            let mut delimited = Vec::new();
            self.encode(&mut delimited)?;
            let Some(members) = delimited.get(DHEADER_LEN..) else {
                // An @appendable XCDR2 stream begins with the DHEADER; a shorter
                // one is not one, and cropping it would emit arbitrary octets.
                return Err(EncodeError::Invalid {
                    what: "XCDR2 stream of an @appendable type is shorter than its DHEADER",
                });
            };
            out.extend_from_slice(members);
            return Ok(());
        }
        Err(EncodeError::Invalid {
            what: "XCDR1 encoding not implemented for this type",
        })
    }

    /// Serializes `self` as the continuation of a sample whose first octets
    /// are `prefix`, little-endian, XCDR1 (`xcdr1`) or XCDR2: the value is
    /// appended behind `prefix` and alignment counts from the start of
    /// `prefix`, because a value that is a member of a larger object aligns
    /// to the object. DDS-RPC 1.0 §7.5.1.1.6 puts the `Call` union behind a
    /// `RequestHeader` of variable length; under XCDR1 an 8-octet member of
    /// the union aligns to the sample.
    ///
    /// The default encodes the value on its own and appends it. That is the
    /// same stream when the type's widest alignment divides `prefix.len()`,
    /// and the default refuses (`EncodeError::Invalid`) when `prefix.len()`
    /// is not a multiple of the stream's alignment cap (8 for XCDR1, 4 for
    /// XCDR2) — it cannot know whether the type has a member that cares. A
    /// type whose codec can start at an offset (generated types) overrides
    /// it and has no such case.
    ///
    /// # Errors
    /// As [`Self::encode`] / [`Self::encode_xcdr1`], and the refusal above.
    fn encode_continuing(
        &self,
        prefix: &mut Vec<u8>,
        xcdr1: bool,
    ) -> core::result::Result<(), EncodeError> {
        let cap = if xcdr1 { 8 } else { 4 };
        if !prefix.len().is_multiple_of(cap) {
            return Err(EncodeError::Invalid {
                what: "this type cannot continue a sample at an offset that is not a multiple of the alignment cap",
            });
        }
        let mut body = Vec::new();
        if xcdr1 {
            self.encode_xcdr1(&mut body)?;
        } else {
            self.encode(&mut body)?;
        }
        prefix.extend_from_slice(&body);
        Ok(())
    }

    /// The counterpart of [`Self::encode_continuing`]: reads the value that
    /// begins at `offset` of `bytes`, alignment counted from the start of
    /// `bytes`. The default refuses an `offset` that is not a multiple of
    /// the alignment cap, for the reason given there.
    ///
    /// # Errors
    /// As [`Self::decode`] / [`Self::decode_xcdr1`], and the refusal above.
    fn decode_continuing(
        bytes: &[u8],
        offset: usize,
        xcdr1: bool,
    ) -> core::result::Result<Self, DecodeError> {
        let cap = if xcdr1 { 8 } else { 4 };
        if !offset.is_multiple_of(cap) {
            return Err(DecodeError::Invalid {
                what: "this type cannot continue a sample at an offset that is not a multiple of the alignment cap",
            });
        }
        let body = bytes.get(offset..).ok_or(DecodeError::Invalid {
            what: "offset past the end of the sample",
        })?;
        if xcdr1 {
            Self::decode_xcdr1(body)
        } else {
            Self::decode(body)
        }
    }

    /// Serializes as **classic CDR / XCDR1 big-endian** (`CDR_BE = 0x0000` /
    /// `PL_CDR_BE = 0x0002`). Symmetric to [`Self::decode_xcdr1_be`]. There is no
    /// safe generic default (a BE re-encode needs per-member byte swapping), so
    /// the default **errors explicitly** rather than silently shipping
    /// little-endian or XCDR2 bytes under a big-endian XCDR1 encapsulation —
    /// which a strict peer would mis-decode. Types that go on the wire as
    /// big-endian XCDR1 (idl-rust codegen, `ShapeType`) override this.
    ///
    /// # Errors
    /// Always `EncodeError::Invalid` unless overridden.
    fn encode_xcdr1_be(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        Err(EncodeError::Invalid {
            what: "XCDR1 big-endian encoding not implemented for this type",
        })
    }

    /// Deserializes a classic-CDR / XCDR1 little-endian payload. Symmetric to
    /// [`Self::encode_xcdr1`]: the default reaches [`Self::decode`] under the
    /// same [`Self::XCDR1_MATCHES_XCDR2`] assertion — unchanged for `@final`,
    /// and for `@appendable` by putting the DHEADER back that XCDR1 does not
    /// carry — and otherwise refuses. A peer's XCDR1 sample read through an
    /// XCDR2 decoder yields a sample the application cannot distinguish from a
    /// correct one.
    ///
    /// # Errors
    /// `DecodeError::Invalid` when the type declares no XCDR1 decoding, or when
    /// the payload is longer than a DHEADER can describe; otherwise the
    /// underlying CDR decoder error.
    fn decode_xcdr1(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        if Self::xcdr1_delegation_is_sound() {
            return Self::decode(bytes);
        }
        if Self::xcdr1_reframes_dheader() {
            // The whole XCDR1 body is the @appendable object's content, so the
            // DHEADER the XCDR2 decoder expects is its length (XTypes 1.3
            // §7.4.3.4.2). `Self::decode` is little-endian, hence a
            // little-endian DHEADER.
            let Ok(len) = u32::try_from(bytes.len()) else {
                return Err(DecodeError::Invalid {
                    what: "XCDR1 payload longer than a DHEADER can describe",
                });
            };
            let mut delimited = Vec::with_capacity(DHEADER_LEN + bytes.len());
            delimited.extend_from_slice(&len.to_le_bytes());
            delimited.extend_from_slice(bytes);
            return Self::decode(&delimited);
        }
        Err(DecodeError::Invalid {
            what: "XCDR1 decoding not implemented for this type",
        })
    }

    /// Deserializes a classic-CDR / XCDR1 **big-endian** payload (encapsulation
    /// `CDR_BE = 0x0000` / `PL_CDR_BE = 0x0002`). Symmetric to
    /// [`Self::encode_xcdr1_be`], and refusing for the same reason: a generic
    /// big-endian read needs per-member byte swapping, so delegating to the
    /// little-endian reader would hand the application byte-swapped field
    /// values without an error. Types that receive big-endian XCDR1
    /// (idl-rust codegen, `ShapeType`) override this.
    ///
    /// # Errors
    /// Always `DecodeError::Invalid` unless overridden.
    fn decode_xcdr1_be(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Err(DecodeError::Invalid {
            what: "XCDR1 big-endian decoding not implemented for this type",
        })
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

    /// Deserializes a `FooKeyHolder` written by [`Self::encode_key_holder_be`]
    /// and returns an instance whose `@key` members carry those values.
    ///
    /// DDS 1.4 §2.2.2.5.1.13: on a sample with `valid_data == false` — a
    /// dispose or unregister marker — "only the fields that form the key" are
    /// meaningful. The marker carries no body, so the reader has nothing but
    /// the instance's key holder to build the sample's `data` from, and that
    /// holder is not the type's own stream: XTypes 1.3 §7.6.8 step 4 writes it
    /// PLAIN_CDR2, **big-endian**, `MAXALIGN` four, with only the key members,
    /// no encapsulation header, no DHEADER and no member headers. Feeding it
    /// to [`Self::decode`] therefore reads the wrong bytes — for a
    /// string-keyed type it reads the holder's big-endian length as a DHEADER
    /// and fails outright.
    ///
    /// The members outside the holder are not in the stream and take their
    /// declared defaults (XTypes 1.3 §7.3.1.2.1.10). Reading them back is not
    /// possible and not asked for: the clause above declares them meaningless
    /// on such a sample.
    ///
    /// **Default implementation**: refuses. It refuses rather than guessing
    /// for the same reason [`Self::decode_xcdr1_be`] does — a wrong instance
    /// key silently merges or splits instances the application keeps apart,
    /// and no caller can see that it happened. Keyed types generated by
    /// `zerodds-build` override it; a hand-written keyed type that reaches the
    /// lifecycle path must too.
    ///
    /// # Errors
    /// `DecodeError::Invalid` unless overridden, and whatever the generated
    /// reader reports for a holder that does not match the type.
    fn decode_key_holder_be(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Err(DecodeError::Invalid {
            what: "this type cannot read its own key holder back",
        })
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

/// What went wrong encoding a sample.
///
/// A narrower type than [`zerodds_wire::WireError`] on purpose: a
/// `DdsType` caller decides between "the sample went out" and "it did
/// not", and the octet the byte runtime stopped at is not a decision it
/// can act on. [`EncodeError::from`] keeps which of the five kinds it
/// was, so the reason survives into a log even though the position does
/// not.
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

impl From<zerodds_wire::WireError> for EncodeError {
    fn from(e: zerodds_wire::WireError) -> Self {
        Self::Invalid {
            what: wire_error_kind(&e),
        }
    }
}

/// Which of [`zerodds_wire::WireError`]'s kinds this is, as a static
/// string.
///
/// The whole error carries a position and an expectation, which a
/// `DdsType` caller has no coordinate system for — but *which* kind it
/// was is the difference between a peer that sent a short message and a
/// value this type cannot represent, and dropping that made every
/// encode and decode failure in this crate read the same. The match is
/// exhaustive over a closed enum, so a sixth kind in `zerodds-wire`
/// fails to compile here rather than arriving as one of these five.
fn wire_error_kind(e: &zerodds_wire::WireError) -> &'static str {
    use zerodds_wire::WireError as W;
    match e {
        W::Truncated { .. } => "wire: the stream ended inside a value",
        W::NotUtf8 { .. } => "wire: a string holding bytes that are not UTF-8",
        W::ImpossibleLength { .. } => "wire: a declared length the stream cannot hold",
        W::OutOfRange { .. } => "wire: a value outside what this type admits",
        W::Discarded { .. } => "wire: a value the receiving type discards",
        W::WrongFraming { .. } => "wire: the framing is not what this encoding asks for",
    }
}

/// What went wrong decoding a sample. The read-side counterpart of
/// [`EncodeError`], and narrowed for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// Truncation or value out-of-range.
    Invalid {
        /// Static description.
        what: &'static str,
    },
    /// The receiving type cannot hold a value the sample carries, and
    /// `@try_construct` at that position is DISCARD.
    ///
    /// **Not the same thing as [`Self::Invalid`], and the reader path
    /// has to keep them apart.** XTypes 1.3 §7.2.4.1: "the default
    /// behavior is to discard the O2 object to avoid
    /// misinterpretation", and §7.2.3 says the sample "can be safely
    /// ignored (filtered out) by the receiver". §7.3.1.2.1.12 gives
    /// DISCARD as what an absent `@try_construct` means, so this is the
    /// *default* outcome of a narrowing and not an exceptional one.
    ///
    /// So it is one sample missing, not a broken stream: the two types
    /// are compatible, this sample does not fit, and the next one may.
    /// A reader that let it abort the whole `take()` would lose the
    /// samples around it; a reader that delivered it truncated would
    /// hand the application a value the writer never sent, which is the
    /// misinterpretation the clause names.
    Discarded {
        /// Static description.
        what: &'static str,
    },
}

impl DecodeError {
    /// Whether this is the DISCARD outcome of a narrowing rather than a
    /// decode failure. See [`Self::Discarded`].
    #[must_use]
    pub const fn is_discarded(&self) -> bool {
        matches!(self, Self::Discarded { .. })
    }
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid { what } => write!(f, "decode error: {what}"),
            Self::Discarded { what } => write!(
                f,
                "the sample is discarded, `@try_construct` there is DISCARD: {what}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

impl From<zerodds_wire::WireError> for DecodeError {
    fn from(e: zerodds_wire::WireError) -> Self {
        let what = wire_error_kind(&e);
        // `WireError::Discarded` is what `wire::try_construct::fit_string`
        // and `fit_sequence` raise under `TryConstruct::Discard`. It has
        // to stay distinguishable all the way to the reader — see
        // `DecodeError::Discarded`.
        if matches!(e, zerodds_wire::WireError::Discarded { .. }) {
            Self::Discarded { what }
        } else {
            Self::Invalid { what }
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

impl DdsAny {
    /// Writes the erased value: the type name as a `string`, then the
    /// payload behind its own four-octet length.
    ///
    /// Inherent rather than a trait `impl`. `zerodds-cdr` had a
    /// `CdrEncode`/`CdrDecode` pair that this type implemented and that
    /// nothing in the tree called it through; `zerodds-wire` has no such
    /// pair, because a trait over one writer is a plan for one encoding
    /// and the plans live in `zerodds-ast-layout` now.
    ///
    /// # Errors
    /// [`zerodds_wire::WireError::OutOfRange`] for a payload longer than
    /// `u32::MAX`, which is refused and not truncated: a length that does
    /// not fit its field would put a shorter value on the wire than the
    /// caller handed over.
    pub fn encode_into(&self, w: &mut zerodds_wire::Writer) -> Result<(), zerodds_wire::WireError> {
        w.write_string(&self.type_name);
        let payload_len =
            u32::try_from(self.payload.len()).map_err(|_| zerodds_wire::WireError::OutOfRange {
                at: w.position(),
                expected: alloc::format!(
                    "a DdsAny payload of at most {} octets, and this one has {}",
                    u32::MAX,
                    self.payload.len()
                ),
            })?;
        w.write_u32(payload_len);
        w.write_bytes(&self.payload);
        Ok(())
    }

    /// Reads back what [`DdsAny::encode_into`] wrote.
    ///
    /// # Errors
    /// Whatever the reader reports — a truncated stream, or a payload
    /// length the stream cannot hold.
    pub fn decode_from(r: &mut zerodds_wire::Reader<'_>) -> Result<Self, zerodds_wire::WireError> {
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
    /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
    const XCDR1_MATCHES_XCDR2: bool = true;
    /// Opaque: the octets are whatever the application serialized, in the
    /// byte order it chose. Nothing here has a field to swap.
    const BYTE_ORDER_AGNOSTIC: bool = true;

    fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
        out.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
        Ok(Self {
            data: bytes.to_vec(),
        })
    }

    /// Opaque octets have no alignment of their own: they continue a
    /// sample at any offset.
    fn encode_continuing(
        &self,
        prefix: &mut Vec<u8>,
        _xcdr1: bool,
    ) -> core::result::Result<(), EncodeError> {
        prefix.extend_from_slice(&self.data);
        Ok(())
    }

    fn decode_continuing(
        bytes: &[u8],
        offset: usize,
        _xcdr1: bool,
    ) -> core::result::Result<Self, DecodeError> {
        let body = bytes.get(offset..).ok_or(DecodeError::Invalid {
            what: "offset past the end of the sample",
        })?;
        Ok(Self {
            data: body.to_vec(),
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

    /// A type with no codec of its own for an offset continues a sample only
    /// where its alignment is the stream's (`encode_continuing`'s default).
    #[test]
    fn the_default_continuing_codec_refuses_an_unaligned_offset() {
        let small = SmallKeyed { id: 7 };
        let mut aligned = vec![0u8; 8];
        small.encode_continuing(&mut aligned, true).unwrap();
        assert_eq!(aligned.len(), 12);
        let mut unaligned = vec![0u8; 4];
        assert!(small.encode_continuing(&mut unaligned, true).is_err());
        // XCDR2 caps alignment at 4.
        let mut xcdr2 = vec![0u8; 4];
        small.encode_continuing(&mut xcdr2, false).unwrap();
        assert!(SmallKeyed::decode_continuing(&aligned, 4, true).is_err());
    }

    #[test]
    fn raw_bytes_continue_a_sample_at_any_offset() {
        let raw = RawBytes::new(vec![1, 2, 3]);
        let mut prefix = vec![9u8; 5];
        raw.encode_continuing(&mut prefix, true).unwrap();
        assert_eq!(prefix, [9, 9, 9, 9, 9, 1, 2, 3]);
        assert_eq!(RawBytes::decode_continuing(&prefix, 5, true).unwrap(), raw);
    }

    // ---- .B: keyed types + KeyHash ----

    /// Test fixture: keyed topic with @key u32 id (max 4 byte → zero-pad).
    struct SmallKeyed {
        id: u32,
    }

    impl DdsType for SmallKeyed {
        const TYPE_NAME: &'static str = "test::SmallKeyed";
        /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
        const XCDR1_MATCHES_XCDR2: bool = true;
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
        /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
        const XCDR1_MATCHES_XCDR2: bool = true;
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
    fn a_byte_order_agnostic_type_shares_one_stream() {
        let r = RawBytes::new(vec![1, 2, 3]);
        let mut le = Vec::new();
        let mut be = Vec::new();
        r.encode(&mut le).unwrap();
        r.encode_be(&mut be).unwrap();
        assert_eq!(le, be);
        assert_eq!(RawBytes::decode_be(&be).unwrap(), r);
    }

    /// A type with no big-endian codec is refused big-endian, both ways.
    ///
    /// The default used to delegate to the little-endian codec: the octets
    /// went out little-endian under a big-endian encapsulation, and a
    /// big-endian peer's `u64` came back with its octets reversed. Neither
    /// side said anything, so the assertion is on the refusal.
    #[test]
    fn a_type_without_a_big_endian_codec_refuses_big_endian_both_ways() {
        struct LittleOnly(u64);
        impl DdsType for LittleOnly {
            const TYPE_NAME: &'static str = "test::LittleOnly";
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                let mut a = [0u8; 8];
                a.copy_from_slice(&bytes[..8]);
                Ok(Self(u64::from_le_bytes(a)))
            }
        }
        let mut out = Vec::new();
        assert!(matches!(
            LittleOnly(0x0102_0304_0506_0708).encode_be(&mut out),
            Err(EncodeError::Invalid { .. })
        ));
        assert!(out.is_empty(), "a refused encode wrote {out:?}");
        assert!(matches!(
            LittleOnly::decode_be(&0x0102_0304_0506_0708_u64.to_be_bytes()),
            Err(DecodeError::Invalid { .. })
        ));
    }

    /// A type that declares no XCDR1 codec is refused under XCDR1, both ways.
    ///
    /// The alternative the default used to take was to write the XCDR2 body and
    /// let the runtime stamp a `CDR_LE` header over it. That produces a sample
    /// no peer can be told from a correct one, so the assertion here is on the
    /// refusal and not on any octets.
    #[test]
    fn undeclared_type_refuses_xcdr1_both_directions() {
        /// No `XCDR1_MATCHES_XCDR2`, no XCDR1 override: exactly the shape a
        /// hand-written type has before anyone thinks about representations.
        struct Undeclared(u64);
        impl DdsType for Undeclared {
            const TYPE_NAME: &'static str = "test::Undeclared";
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                let mut a = [0u8; 8];
                a.copy_from_slice(&bytes[..8]);
                Ok(Self(u64::from_le_bytes(a)))
            }
        }

        let mut out = Vec::new();
        assert!(
            Undeclared(7).encode_xcdr1(&mut out).is_err(),
            "an undeclared type must refuse the XCDR1 write, not emit XCDR2 octets"
        );
        assert!(
            out.is_empty(),
            "a refused write must leave the buffer alone"
        );
        assert!(
            Undeclared::decode_xcdr1(&7u64.to_le_bytes()).is_err(),
            "an undeclared type must refuse the XCDR1 read"
        );
        // The XCDR2 path is untouched by the declaration.
        let mut xcdr2 = Vec::new();
        Undeclared(7).encode(&mut xcdr2).unwrap();
        assert_eq!(xcdr2, 7u64.to_le_bytes());
    }

    /// The declaration lets `@final` through unchanged and `@appendable` through
    /// without its DHEADER.
    ///
    /// XTypes 1.3 Table 60 gives `@appendable` PLAIN_CDR under XCDR1 and
    /// DELIMITED_CDR under XCDR2, so the XCDR1 wire is the XCDR2 wire minus the
    /// 4-octet prefix — the interop case a Cyclone `@final` writer on XCDR1
    /// produces against a ZeroDDS `@appendable` reader.
    #[test]
    fn equivalence_claim_reframes_appendable() {
        struct FinalEquivalent(u32);
        impl DdsType for FinalEquivalent {
            const TYPE_NAME: &'static str = "test::FinalEquivalent";
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Self(0))
            }
        }

        /// The same claim on an `@appendable` type: a DHEADER-delimited XCDR2
        /// body over one `uint32` member.
        struct AppendableEquivalent(u32);
        impl DdsType for AppendableEquivalent {
            const TYPE_NAME: &'static str = "test::AppendableEquivalent";
            const EXTENSIBILITY: Extensibility = Extensibility::Appendable;
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.extend_from_slice(&4u32.to_le_bytes());
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                let mut dheader = [0u8; 4];
                let mut value = [0u8; 4];
                if bytes.len() < 8 {
                    return Err(DecodeError::Invalid { what: "truncated" });
                }
                dheader.copy_from_slice(&bytes[..4]);
                value.copy_from_slice(&bytes[4..8]);
                if u32::from_le_bytes(dheader) as usize != bytes.len() - 4 {
                    return Err(DecodeError::Invalid { what: "dheader" });
                }
                Ok(Self(u32::from_le_bytes(value)))
            }
        }

        assert!(FinalEquivalent::xcdr1_delegation_is_sound());
        let mut out = Vec::new();
        FinalEquivalent(9).encode_xcdr1(&mut out).unwrap();
        assert_eq!(out, 9u32.to_le_bytes());

        assert!(!AppendableEquivalent::xcdr1_delegation_is_sound());
        assert!(AppendableEquivalent::xcdr1_reframes_dheader());
        let mut out = Vec::new();
        AppendableEquivalent(9).encode_xcdr1(&mut out).unwrap();
        assert_eq!(
            out,
            9u32.to_le_bytes(),
            "the XCDR1 wire of an @appendable type is its XCDR2 wire without the DHEADER"
        );
        assert_eq!(
            AppendableEquivalent::decode_xcdr1(&out).unwrap().0,
            9,
            "the XCDR1 read puts the DHEADER back before the XCDR2 decoder sees it"
        );
    }

    /// Neither big-endian XCDR1 direction has a generic default.
    ///
    /// A big-endian re-encode needs per-member swapping, and a big-endian read
    /// of a little-endian decoder returns swapped field values without an
    /// error — the reason the write direction already refused, and the read
    /// direction now does too.
    #[test]
    fn xcdr1_big_endian_refuses_in_both_directions() {
        let r = RawBytes::new(vec![1, 2, 3]);
        let mut out = Vec::new();
        assert!(r.encode_xcdr1_be(&mut out).is_err());
        assert!(RawBytes::decode_xcdr1_be(&[1, 2, 3]).is_err());
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
            /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
            const XCDR1_MATCHES_XCDR2: bool = true;
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
            /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
            const XCDR1_MATCHES_XCDR2: bool = true;
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

    // ---- DDSI-RTPS 2.5 §9.6.4.8: the three examples of the KeyHash ----

    /// The member that the three examples have in common: its holder is
    /// written by `holder`, its maximum size is stated by the type.
    struct SpecExample<const N: u8>;

    impl<const N: u8> DdsType for SpecExample<N> {
        const TYPE_NAME: &'static str = "test::SpecExample";
        const HAS_KEY: bool = true;
        const KEY_HOLDER_MAX_SIZE: Option<usize> = match N {
            1 => Some(4),
            2 => Some(28),
            _ => Some(21),
        };

        fn encode(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            Ok(())
        }
        fn decode(_bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
            Ok(Self)
        }
        fn encode_key_holder_be(&self, holder: &mut PlainCdr2BeKeyHolder) {
            match N {
                // @final struct Foo { @key long id; long x; long y; } with
                // id 0x12345678: the holder is the id.
                1 => holder.write_u32(0x1234_5678),
                // struct Foo { @key string<12> label; @key long long id; ... }
                // with label "BLUE" and id 0x123456789abcdef0.
                2 => {
                    holder.write_string("BLUE");
                    holder.write_u64(0x1234_5678_9abc_def0);
                }
                // Foo with @id(40) @key label and @id(30) @key m_nested: the
                // holder is ordered by memberId, m_nested's key first.
                _ => {
                    holder.write_u32(0x1234_5678);
                    holder.write_string("BLUE");
                }
            }
        }
    }

    fn holder_of<const N: u8>() -> Vec<u8> {
        let mut holder = PlainCdr2BeKeyHolder::new();
        SpecExample::<N>.encode_key_holder_be(&mut holder);
        holder.into_bytes()
    }

    /// Example 1: the 4-octet holder `{12 34 56 78}` is at most 16 octets,
    /// so the KeyHash is the holder extended with zeros to 16 octets.
    #[test]
    fn key_hash_example_1_of_the_spec_is_the_holder_padded_with_zeros() {
        assert_eq!(holder_of::<1>(), [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(
            SpecExample::<1>.compute_key_hash(),
            Some([
                0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00
            ])
        );
    }

    /// Example 2: the 20-octet holder (string, 3 octets of zero padding,
    /// the long long on a 4-octet boundary) has a maximum of 28 octets, so
    /// the KeyHash is its MD5.
    #[test]
    fn key_hash_example_2_of_the_spec_is_the_md5_of_the_holder() {
        assert_eq!(
            holder_of::<2>(),
            [
                0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34,
                0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0
            ]
        );
        assert_eq!(
            SpecExample::<2>.compute_key_hash(),
            Some([
                0xf9, 0x1a, 0x59, 0xe3, 0x2e, 0x45, 0x35, 0xd9, 0xa6, 0x9c, 0xd5, 0xd9, 0xf5, 0xb6,
                0xe3, 0x6e
            ])
        );
    }

    /// Example 3: the 13-octet holder, ordered by memberId (the nested key
    /// first), has a maximum of 21 octets, so the KeyHash is its MD5.
    #[test]
    fn key_hash_example_3_of_the_spec_is_the_md5_of_the_reordered_holder() {
        assert_eq!(
            holder_of::<3>(),
            [
                0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00
            ]
        );
        assert_eq!(
            SpecExample::<3>.compute_key_hash(),
            Some([
                0x37, 0x4b, 0x96, 0xe2, 0xe7, 0x27, 0x23, 0x7f, 0x01, 0x6c, 0xc4, 0xce, 0xbb, 0x6e,
                0xb7, 0x1e
            ])
        );
    }
}
