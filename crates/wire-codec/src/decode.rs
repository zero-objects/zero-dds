// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes back into the value.
//!
//! `encode.rs` walks the layout plan and writes; this walks the same
//! plan and reads. Not a second reading of the model, for the reason the
//! encoder gives: two walks that decided the framing separately would
//! agree with each other only until one of them changed.
//!
//! ## What the return direction adds
//!
//! Encoding is measured by comparing bytes, and a byte comparison passes
//! a decoder that is wrong in the same way its own encoder is wrong. A
//! round trip through one implementation passes such a defect every
//! time; a round trip through two does not. That is the whole reason
//! this file exists — `docs/specs/zerodds-interop-suite-1.0.md` §4, P2.
//!
//! ## Where reading needs a decision writing did not
//!
//! Two places, each of which the writer never had to face:
//!
//! - **A union's member.** The writer takes the member the value names;
//!   the reader has only the discriminator, so it evaluates the case
//!   labels against it, falls back to `default:`, and reads no member at
//!   all where neither matches.
//! - **The end of the stream.** A decoder that stopped early would leave
//!   bytes unread and report success, which is the failure class this
//!   suite exists to find. Anything left over is an error.

use crate::canonical;
use crate::encode::{ProbeError, sequence_element_carries_length};
use crate::sink::{Aggregate, Sink};
use crate::source::Source;
use crate::value::{Emit, MapLengthPolicy, ProbeRequest};

use std::string::String;
use std::vec::Vec;
use zerodds_ast::{
    Ast, Bound, CharWidth, Extensibility, PrimitiveKind, TryConstruct, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref, ScopedName};
use zerodds_ast_layout::{
    DefaultedDiscriminatorMember, DepCollector, ElementPadding, Endianness, Extent,
    FixedDigitCount, LayoutKey, MemberHeader, Purpose, WirePosition, WstringCodeUnits,
    WstringLengthCounts, WstringLengthPolicy, WstringTerminator, WstringUnitOrder,
    array_is_delimited, collection_is_delimited, enumeration_bit_bound, enumerator_wire_value,
    plan, union_negative_matches_nothing,
};
use zerodds_wire::{ParameterRole, Reader, WcharPosition, WireError};

/// What a walk carries down, the counterpart of the encoder's.
#[derive(Debug, Clone, Copy)]
struct Frame {
    key: LayoutKey,
    map_length: MapLengthPolicy,
    /// What to do if the value about to be read will not fit this
    /// type. DDS-XTypes 1.3 §7.2.2.7.
    ///
    /// Set by whichever site is about to read — a member, a collection
    /// element, a map key or value, a union's discriminator — from the
    /// annotation that governs *that* position. A site that does not set
    /// it leaves `DISCARD`, which is the clause's own default for an
    /// annotation that was not written, so a position nobody has taught
    /// this walk about behaves as the standard says rather than as
    /// whatever the enclosing member asked for.
    try_construct: TryConstruct,
    /// What the site *wrote*, before the alias chain of the type it uses
    /// was consulted.
    ///
    /// Only an array needs the distinction. There is no syntax for an
    /// array's element slot on a member declarator, so
    /// `@try_construct(TRIM) String5 m[3]` writes the annotation on the
    /// member and Table 17 spends it on the elements — and a use site's
    /// own annotation has to beat a default the element's `typedef`
    /// carries, which the already-resolved value cannot tell apart from
    /// an inherited one.
    written_try_construct: Option<TryConstruct>,
    /// The member whose value is being read, when the position is one.
    ///
    /// `USE_DEFAULT` substitutes "the default value of the
    /// member" (§7.2.2.7), and §7.2.2.4.4.4.9 puts the member's explicit
    /// `@default` ahead of Table 9's zero: "An explicit definition
    /// associates a default value with the member at the time where the
    /// aggregated type is defined." Substituting Table 9's zero for a
    /// member the designer gave a `@default` would be a silently wrong
    /// value, which is the failure class this project ranks above every
    /// other. `None` at a collection element or a discriminator, which
    /// are not members and have only their type's default.
    default_from: Option<Id<zerodds_ast::MemberNode>>,
    /// Whether the type about to be read is reached as a member or as a
    /// collection element.
    ///
    /// The encoder's counterpart, and it has to be threaded on this side
    /// too: under TAO's framing a `wchar` member carries an octet count
    /// and an element does not, so a reader that assumed the member's
    /// form for an element would take the first half of a code unit for
    /// a count and refuse. See `zerodds_ast_layout::WcharPosition`.
    wchar: WcharPosition,
    /// How many aggregates of the *value* the walk is inside.
    ///
    /// The encoder's counterpart, and the reason it exists on this side
    /// too: nothing in a byte stream bounds how deeply it nests a
    /// recursive type, and a reader that followed it would run out of
    /// stack rather than say what was wrong. See
    /// [`crate::MAX_VALUE_DEPTH`].
    depth: usize,
    /// Whether the value about to be read is the discriminator of a union.
    ///
    /// A bit that a bitmask discriminator does not declare is refused
    /// there and dropped elsewhere (`read_bitmask`): the discriminator is
    /// what selects the member, and a set with the bit left out selects
    /// another one or none.
    is_discriminator: bool,
}

impl Frame {
    /// The same frame, reading a position that wrote `behaviour` and
    /// uses `used`.
    fn constructing(
        self,
        ast: &Ast,
        behaviour: Option<TryConstruct>,
        used: Option<Id<TypeNode>>,
    ) -> Self {
        Self {
            try_construct: ast.effective_try_construct(behaviour, used),
            written_try_construct: behaviour,
            ..self
        }
    }

    /// The same frame, reading the value of `member`.
    fn for_member(
        self,
        ast: &Ast,
        behaviour: Option<TryConstruct>,
        used: Option<Id<TypeNode>>,
        member: Id<zerodds_ast::MemberNode>,
    ) -> Self {
        Self {
            default_from: Some(member),
            ..self.constructing(ast, behaviour, used)
        }
    }

    /// The same frame, reading a position whose behaviour is already
    /// resolved — a collection's element, whose slot the query below
    /// has already read.
    const fn resolved(self, behaviour: TryConstruct) -> Self {
        Self {
            try_construct: behaviour,
            written_try_construct: Some(behaviour),
            // An element, a key or a discriminator is not a member and
            // carries no `@default` of its own.
            default_from: None,
            ..self
        }
    }

    /// The frame an element of a collection is read under.
    ///
    /// Separate from [`Self::resolved`] because that one also serves a
    /// union's discriminator, which is a member position: the two
    /// questions overlap in their call sites and not in their answers.
    const fn in_collection(self) -> Self {
        Self {
            wchar: WcharPosition::Element,
            ..self
        }
    }

    /// The frame an aggregate's own members are read under.
    ///
    /// The one place the value's depth grows on this side: every
    /// descent into an aggregate's members goes through here, so no arm
    /// of the walk can descend without counting.
    const fn in_aggregate(self) -> Self {
        Self {
            wchar: WcharPosition::Member,
            depth: self.depth.saturating_add(1),
            ..self
        }
    }

    /// The refusal for a stream that nests past
    /// [`crate::MAX_VALUE_DEPTH`], or `Ok(())` while it does not.
    ///
    /// # Errors
    /// [`ProbeError::ValueTooDeep`].
    fn within_depth(self, path: &str) -> Result<(), ProbeError> {
        if self.depth >= crate::MAX_VALUE_DEPTH {
            return Err(ProbeError::ValueTooDeep {
                path: String::from(path),
                limit: crate::MAX_VALUE_DEPTH,
            });
        }
        Ok(())
    }

    /// Table 9's default for `type_`, or the member's `@default` where
    /// the position is a member that declares one.
    ///
    /// One call, so that every `USE_DEFAULT` in this walk goes through
    /// the same derivation the evolution path uses — `default.rs`, whose
    /// documentation carries Table 9 row by row — rather than each site
    /// spelling a zero of its own.
    ///
    /// # Errors
    /// Whatever the defaulter says about a type it cannot name.
    fn default_value<V: Source + Sink>(
        self,
        ast: &Ast,
        type_: Id<TypeNode>,
        path: &str,
    ) -> Result<V, ProbeError> {
        match self.default_from.and_then(|id| ast.member(id).ok()) {
            Some(node) => crate::default::member(ast, node, type_, path),
            None => crate::default::of_type(ast, type_, path),
        }
    }
}

/// What a receiving type does with a value it cannot hold.
///
/// One place, so the four behaviours cannot be spelled differently at
/// the string site and the sequence site. `trimmed` is what `TRIM`
/// produces where Table 11 defines it — a string, wstring, sequence or
/// map cut to the receiver's bound — and `None` everywhere else;
/// `default` is Table 9's value for the receiving type, computed only
/// when it is wanted because a structure's default is a walk of its own.
///
/// # Errors
/// [`ProbeError::Discarded`] under `DISCARD`, whatever `default`
/// returns under `USE_DEFAULT`, and [`ProbeError::Unsupported`] for a
/// `TRIM` the frontend should have refused.
fn narrowed<V, D>(
    behaviour: TryConstruct,
    trimmed: Option<V>,
    default: D,
    path: &str,
    expected: &str,
    found: &str,
) -> Result<V, ProbeError>
where
    D: FnOnce() -> Result<V, ProbeError>,
{
    match behaviour {
        TryConstruct::Discard => Err(ProbeError::Discarded {
            path: path.to_string(),
            expected: expected.to_string(),
            found: found.to_string(),
        }),
        TryConstruct::UseDefault => default(),
        // Table 11: TRIM "only applies to elements or members of type
        // string, wide string, sequence, or map. The behavior when
        // applied to other element/member types is unspecified and may
        // be treated as an error." `idl-lower`'s validation refuses such
        // a declaration outright, so reaching here means a model that
        // did not come through the frontend, and answering it with
        // DISCARD would hide that.
        TryConstruct::Trim => trimmed.ok_or(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "`@try_construct(TRIM)` on something that cannot be trimmed",
        }),
    }
}

/// The number of padding octets an encapsulated payload of `payload_len`
/// octets (encapsulation header included) carries at its end, given the
/// low byte of its options word.
///
/// The low two bits of the options are the count of padding octets the
/// writer appended (XTypes 1.3 §7.6.3.1.2), and it appends them to bring
/// the payload to a multiple of four; a payload whose length is not one
/// has none, whatever the options say. OpenDDS 3.34 writes a count of two
/// on a 70-octet payload that ends right after its last data octet: cutting
/// two octets off it cut two octets of the sample, and every such sample
/// failed to decode. Cyclone DDS, Fast DDS and RTI Connext pad what they
/// count, and there the count is honoured.
///
/// A count larger than the body after the header is no padding a writer
/// can have added: it is answered as is, and every caller checks that the
/// body holds it and refuses the stream when it does not (XTypes 1.3
/// §7.6.3.1.2: the count "shall be interpreted by the receiver to
/// determine where the serialized data exactly ended"). Reading such a
/// stream as if the count were 0 accepted a malformed payload in silence.
#[must_use]
pub fn padding_present(payload_len: usize, options_low: u8) -> usize {
    let declared = usize::from(options_low & 0x3);
    let body = payload_len.saturating_sub(4);
    if declared == 0 {
        0
    } else if declared > body {
        declared
    } else if payload_len % 4 != 0 {
        0
    } else {
        declared
    }
}

/// Read `bytes` as `request.type_name`, in canonical form.
///
/// The value that comes back is already canonical —
/// [`crate::canonical::text`] renders it, and
/// [`crate::canonical::difference`] compares it against the sample that
/// went in.
///
/// # Errors
/// [`ProbeError`], naming the path in the value or the offset in the
/// stream.
pub fn decode<V: Source + Sink>(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
) -> Result<V, ProbeError> {
    decode_continuing(ast, request, bytes, 0)
}

/// As [`decode`], for a value that begins at `start` in `bytes`: alignment
/// counts from the start of `bytes`, the sample.
///
/// The counterpart of [`crate::encode::encode_continuing`].
///
/// # Errors
/// As [`decode`]; `ProbeError::Unsupported` for an `emit` other than
/// `Emit::Payload` when `start` is not zero.
pub fn decode_continuing<V: Source + Sink>(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
    start: usize,
) -> Result<V, ProbeError> {
    if start != 0 && request.emit != Emit::Payload {
        return Err(ProbeError::Unsupported {
            path: request.type_name.clone(),
            kind: "a continuation behind an encapsulation header or as a KeyHash",
        });
    }
    if request.emit == Emit::KeyHash {
        return Err(ProbeError::Unsupported {
            path: request.type_name.clone(),
            kind: "a KeyHash, which is a digest and holds no value to read back",
        });
    }
    let type_ = find_type(ast, &request.type_name)?;

    // `--emit payload+encapsulation` is the same payload behind the
    // four-byte header. The header is not re-read for the encoding: the
    // caller named one, and a probe that took the stream's word for it
    // could not be pointed at a stream to test what it does with a
    // header it disagrees with.
    //
    // The **options** word is read, because §7.6.3.1.2 says it shall
    // be: its low two bits are the count of padding bytes at the end of
    // the serialized data, and they "shall be interpreted by the
    // receiver to determine where the serialized data exactly ended".
    // Cyclone, RTI and Fast DDS all write them — see
    // `value::encapsulation_header`. A reader that ignored them would
    // take up to three bytes of padding for data, which `trailing`
    // forgives only because it cannot tell the two apart; with the
    // count it can.
    let payload = if request.emit == Emit::PayloadWithEncapsulation {
        // Split rather than index: the header's fourth byte is read
        // below, and taking it with a fallback would turn a stream too
        // short to hold a header into a padding count of zero.
        let (header, body) = bytes
            .split_at_checked(4)
            .ok_or_else(|| ProbeError::Mismatch {
                path: request.type_name.clone(),
                expected: "at least the four-byte encapsulation header".to_string(),
                found: format!("{} bytes", bytes.len()),
            })?;
        // Byte 3 of the header, low two bits. The word is big-endian
        // like the identifier, so the count is in the second byte of
        // the pair whatever the payload's order. `padding_present` says
        // whether the octets are there at all.
        let declared = padding_present(bytes.len(), header[3]);
        let end = body
            .len()
            .checked_sub(declared)
            .ok_or_else(|| ProbeError::Mismatch {
                path: request.type_name.clone(),
                expected: format!(
                    "a payload long enough to hold the {declared} padding byte(s) the \
                     encapsulation options declared"
                ),
                found: format!("{} byte(s) behind the header", body.len()),
            })?;
        // §7.4.1.1 says the padding is zero. A non-zero byte there is
        // either a writer that miscounted or a reader pointed at the
        // wrong offset, and cutting it off in silence would hide both.
        if body[end..].iter().any(|byte| *byte != 0) {
            return Err(ProbeError::Mismatch {
                path: request.type_name.clone(),
                expected: format!("{declared} zero byte(s) of trailing padding, per §7.4.1.1"),
                found: format!("{:02x?} at offset {}", &body[end..], end + 4),
            });
        }
        &body[..end]
    } else {
        bytes
    };

    // The request's own key, assembled by the request: a reader planned
    // under one dialect and a writer under another disagree about the
    // framing before either touches a byte, and that reads as a wire
    // difference without being one.
    let frame = Frame {
        key: request
            .layout_key(Purpose::Data)
            .for_sample(ast, type_)
            .map_err(|e| ProbeError::NoLayout {
                detail: e.to_string(),
            })?,
        map_length: request.map_length,
        // The top-level type is not a member of anything, so no
        // annotation governs it. §7.2.2.7's default stands.
        try_construct: TryConstruct::ABSENT,
        written_try_construct: None,
        default_from: None,
        wchar: WcharPosition::Member,
        depth: 0,
        is_discriminator: false,
    };
    // The same framing the writer would have used, from the same
    // policy: the read direction takes every dialect setting the write
    // direction does. `--dialect rti` on a decode run means "read what
    // RTI writes". The rest of the dialect reaches this walk through
    // `frame.key`, which is the plan it follows.
    let mut r = Reader::with_max_align(
        payload,
        wire_endianness(request.endianness),
        usize::from(request.encoding.max_alignment()),
    )
    .with_pl_cdr1_framing(request.pl_cdr1_framing())
    .with_pl_cdr1_reading(request.wire_pl_cdr1_reading())
    .with_wchar_width(request.wchar_width())
    .with_wchar_framing(request.wire_wchar_framing())
    .with_wstring_unit_order(request.wire_wstring_unit_order());
    if start != 0 {
        r.skip_to(start).map_err(|e| ProbeError::Mismatch {
            path: request.type_name.clone(),
            expected: format!("a value at offset {start}"),
            found: e.to_string(),
        })?;
    }
    let key = frame.key;
    let value = read_type(&mut r, ast, type_, frame, &request.type_name)?;
    if r.remaining() != 0 && appended_members_end_the_sample(ast, type_, key) {
        // XTypes 1.3 §7.2.4.2: an `@appendable` type "may have
        // additional members appended at the end", and a reader with
        // fewer members ignores them. Under encoding version 2 the
        // DHEADER says where they end and `read_aggregate` skipped to
        // it; under version 1 there is no delimiter, and for the
        // top-level type the sample's own extent is the only one — the
        // octets after the reader's last member are the writer's
        // appended members and nothing else can be there. A nested
        // `@appendable` type has the enclosing type's bytes behind it
        // and keeps the refusal below (`evo-appendable-nested`).
        //
        // Measured on the OMG `dds-xtypes` suite, `ext_appendable_struct_3`
        // under `-x 1`: a two-member `@appendable` writer read by its
        // one-member version answered "did not decode" here.
        return Ok(value);
    }
    trailing(&r, &request.type_name)?;
    Ok(value)
}

/// Read one value of `request.type_name` from the start of `bytes` and say
/// how many octets it took.
///
/// [`decode`] reads a whole payload and refuses octets left behind it. A
/// transport that packs several serialized samples into one buffer without a
/// length for each (DDS-XRCE 1.0 Annex A `sequence<SampleData>`) has only the
/// type to tell where one sample ends and the next begins: this reads the
/// first and returns where it stopped. The octets after it are not looked at.
///
/// The payload is read without an encapsulation header (`Emit::Payload`),
/// from alignment origin 0.
///
/// # Errors
/// [`ProbeError`], as [`decode`]; `Unsupported` for any `emit` but
/// `Emit::Payload`.
pub fn decode_prefix<V: Source + Sink>(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
) -> Result<(V, usize), ProbeError> {
    if request.emit != Emit::Payload {
        return Err(ProbeError::Unsupported {
            path: request.type_name.clone(),
            kind: "a prefix read of anything but the bare payload",
        });
    }
    let type_ = find_type(ast, &request.type_name)?;
    let frame = Frame {
        key: request
            .layout_key(Purpose::Data)
            .for_sample(ast, type_)
            .map_err(|e| ProbeError::NoLayout {
                detail: e.to_string(),
            })?,
        map_length: request.map_length,
        try_construct: TryConstruct::ABSENT,
        written_try_construct: None,
        default_from: None,
        wchar: WcharPosition::Member,
        depth: 0,
        is_discriminator: false,
    };
    let mut r = Reader::with_max_align(
        bytes,
        wire_endianness(request.endianness),
        usize::from(request.encoding.max_alignment()),
    )
    .with_pl_cdr1_framing(request.pl_cdr1_framing())
    .with_pl_cdr1_reading(request.wire_pl_cdr1_reading())
    .with_wchar_width(request.wchar_width())
    .with_wchar_framing(request.wire_wchar_framing())
    .with_wstring_unit_order(request.wire_wstring_unit_order());
    let value = read_type(&mut r, ast, type_, frame, &request.type_name)?;
    Ok((value, bytes.len() - r.remaining()))
}

/// Whether octets left after the top-level type are appended members of
/// an evolved `@appendable` writer rather than an unread tail: encoding
/// version 1, where no DHEADER delimits the aggregate, and the type is
/// `@appendable`. §7.2.4.2, and see the caller.
fn appended_members_end_the_sample(ast: &Ast, type_: Id<TypeNode>, key: LayoutKey) -> bool {
    if !key.encoding.is_v1() {
        return false;
    }
    let Ok(node) = ast.type_(type_) else {
        return false;
    };
    matches!(
        node.kind,
        TypeKind::Structure { .. } | TypeKind::Union { .. }
    ) && zerodds_ast_layout::xcdr::extensibility_of(ast, node, key) == Extensibility::Appendable
}

/// Validate the currently supported inverse holder shape without reading data.
///
/// Primitive, string, enumeration, alias and nested-structure keys are supported.
/// Optional/external, collection and union key contexts are explicitly refused.
/// # Errors
/// Unsupported holder shape or invalid layout.
pub fn validate_key_holder(ast: &Ast, request: &ProbeRequest) -> Result<(), ProbeError> {
    holder_members(ast, request).map(|_| ())
}

fn holder_members(
    ast: &Ast,
    request: &ProbeRequest,
) -> Result<(Id<TypeNode>, Vec<MemberSlot>), ProbeError> {
    let type_ = find_type(ast, &request.type_name)?;
    let bad = || ProbeError::Unsupported {
        path: request.type_name.clone(),
        kind: "inverse holder requires structure keys with supported scalar/alias/nested contexts",
    };
    let node = ast.type_(type_).map_err(|_| bad())?;
    if !matches!(node.kind, TypeKind::Structure { .. }) {
        return Err(bad());
    }
    let key = request.key_holder_key();
    // The holder is PLAIN_CDR2 (XTypes 1.3 §7.6.8) and holds no PL_CDR1
    // parameter, so how those are framed and read (`wire.pl_cdr1_headers`,
    // `wire.pl_cdr1_reading`) leaves it in the specification's form.
    let specification = LayoutKey::key(zerodds_ast_layout::Encoding::PlainCdr2);
    let holder_form = LayoutKey {
        pl_cdr1_headers: specification.pl_cdr1_headers,
        pl_cdr1_reading: specification.pl_cdr1_reading,
        ..key
    };
    if holder_form != specification {
        return Err(bad());
    }
    let layout = plan(
        ast,
        type_,
        request.layout_key(Purpose::Data),
        &mut DepCollector::default(),
    )
    .map_err(|e| ProbeError::NoLayout {
        detail: e.to_string(),
    })?;
    let members = member_table(ast, type_, &layout);
    let keys = zerodds_ast_layout::keyhash::key_members_with(
        ast,
        type_,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .map_err(|e| ProbeError::NoLayout {
        detail: e.to_string(),
    })?;
    if keys.is_empty() {
        return Err(ProbeError::NoKey {
            type_name: request.type_name.clone(),
        });
    }
    for id in keys {
        let Some(member) = members
            .iter()
            .find(|m| ast.member(m.member).is_ok_and(|n| n.node_id == id))
        else {
            continue;
        };
        let node = ast.member(member.member).map_err(|_| bad())?;
        if member.optional || node.facets.xtypes.as_ref().is_some_and(|x| x.is_external) {
            return Err(bad());
        }
        validate_holder_context(ast, member.type_, key, 0, &request.type_name)?;
    }
    Ok((type_, members))
}

/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH` is checked
/// before inspecting a type. Both alias and structure edges increment depth.
fn validate_holder_context(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    depth: usize,
    path: &str,
) -> Result<(), ProbeError> {
    let bad = || ProbeError::Unsupported {
        path: path.into(),
        kind: "unsupported or recursive inverse key context",
    };
    if depth >= crate::MAX_VALUE_DEPTH {
        return Err(bad());
    }
    let node = ast.type_(type_).map_err(|_| bad())?;
    match &node.kind {
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Enumeration { .. } => Ok(()),
        TypeKind::Alias { aliased } => validate_holder_context(
            ast,
            aliased.resolved().ok_or_else(bad)?,
            key,
            depth + 1,
            path,
        ),
        TypeKind::Structure { .. } => {
            let layout = plan(ast, type_, key, &mut DepCollector::default()).map_err(|e| {
                ProbeError::NoLayout {
                    detail: e.to_string(),
                }
            })?;
            let inside = zerodds_ast_layout::member_key(ast, type_, key);
            for slot in member_table(ast, type_, &layout) {
                let member = ast.member(slot.member).map_err(|_| bad())?;
                if slot.optional || member.facets.xtypes.as_ref().is_some_and(|x| x.is_external) {
                    return Err(bad());
                }
                validate_holder_context(ast, slot.type_, inside, depth + 1, path)?;
            }
            Ok(())
        }
        _ => Err(bad()),
    }
}

/// Decode the writer's own `serializedKey` (DDSI-RTPS 2.5 §9.4.2.11) — the
/// key-only payload behind a dispose/unregister DATA's K flag — in the
/// representation and byte order `request` names.
///
/// Unlike [`key_holder`], which always reads the canonical big-endian
/// PLAIN_CDR2 Key Holder XTypes 1.3 §7.6.8 fixes for computing the
/// KeyHash, this reads exactly what a writer put on the wire: only the
/// `@key` members — recursing into a nested keyed aggregate's own `@key`
/// members the same way — in declaration order, framed by the type's OWN
/// extensibility (a DHEADER for `@appendable`/`@mutable` under XCDR2, a
/// per-member EMHEADER for `@mutable`, nothing for `@final` or under
/// XCDR1), in `request`'s own encoding and endianness. Measured against
/// Eclipse Cyclone 11.0.1 (`ddsi_serdata_from_sample(sertype, SDK_KEY,
/// …)` + `ddsi_serdata_to_ser`): the key member kept its declared
/// position among the type's other members (a `@key` member need not be
/// the first, or even present, in what a truncated-sample reading of the
/// same bytes would decode), and a nested `@appendable` key member came
/// back with its own DHEADER inside the outer one.
///
/// `request.key_holder`/`key_holder_order`/`key_holder_nesting` select
/// this reading: `AsDeclared` combined with `Declaration` order (and
/// `Recursive` nesting, already the default) is the wire form; nothing
/// here touches the canonical hash-computation form (`ForcedFinal`
/// combined with `AscendingMemberId`) [`key_holder`] and the rest of
/// this module use.
///
/// Non-key members come back at their default value — the wire payload
/// never carried them — through the same `Purpose::Key` "holder" path
/// `read_aggregate` already takes for [`key_holder`]'s canonical form;
/// a caller after the KeyHash reads it off the decoded key values
/// through the existing canonical encoder, never from this value
/// directly.
///
/// Unlike [`decode`], nothing here tolerates leftover bytes as a
/// forward-evolved writer's appended members: a `serializedKey` carries
/// only the key members and nothing else can legitimately follow one, so
/// any byte `trailing` finds past the declared padding is refused.
///
/// # Errors
/// [`ProbeError`] naming the path or the offset: a context [`plan()`]
/// cannot describe, a payload the plan's shape does not fit, a value the
/// planned type refuses, or bytes left over the padding count does not
/// account for.
pub fn serialized_key<V: Source + Sink>(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
) -> Result<V, ProbeError> {
    let type_ = find_type(ast, &request.type_name)?;
    let payload = if request.emit == Emit::PayloadWithEncapsulation {
        // Same options-word padding rule as `decode` — §7.6.3.1.2's low
        // two bits, measured on Cyclone's own `serializedKey` octets
        // (e.g. `S` above: `0002` for two trailing zero bytes).
        let (header, body) = bytes
            .split_at_checked(4)
            .ok_or_else(|| ProbeError::Mismatch {
                path: request.type_name.clone(),
                expected: "at least the four-byte encapsulation header".to_string(),
                found: format!("{} bytes", bytes.len()),
            })?;
        let declared = padding_present(bytes.len(), header[3]);
        let end = body
            .len()
            .checked_sub(declared)
            .ok_or_else(|| ProbeError::Mismatch {
                path: request.type_name.clone(),
                expected: format!(
                    "a payload long enough to hold the {declared} padding byte(s) the \
                     encapsulation options declared"
                ),
                found: format!("{} byte(s) behind the header", body.len()),
            })?;
        if body[end..].iter().any(|byte| *byte != 0) {
            return Err(ProbeError::Mismatch {
                path: request.type_name.clone(),
                expected: format!("{declared} zero byte(s) of trailing padding, per §7.4.1.1"),
                found: format!("{:02x?} at offset {}", &body[end..], end + 4),
            });
        }
        &body[..end]
    } else {
        bytes
    };
    let frame = Frame {
        key: request.layout_key(Purpose::Key),
        map_length: request.map_length,
        try_construct: TryConstruct::ABSENT,
        written_try_construct: None,
        default_from: None,
        wchar: WcharPosition::Member,
        depth: 0,
        is_discriminator: false,
    };
    let mut r = Reader::with_max_align(
        payload,
        wire_endianness(request.endianness),
        usize::from(request.encoding.max_alignment()),
    )
    .with_pl_cdr1_framing(request.pl_cdr1_framing())
    .with_pl_cdr1_reading(request.wire_pl_cdr1_reading())
    .with_wchar_width(request.wchar_width())
    .with_wchar_framing(request.wire_wchar_framing())
    .with_wstring_unit_order(request.wire_wstring_unit_order());
    let value = read_type(&mut r, ast, type_, frame, &request.type_name)?;
    trailing(&r, &request.type_name)?;
    Ok(value)
}

/// Decode an actual PLAIN_CDR2-BE holder using the shared member plan and defaults.
/// # Errors
/// Unsupported key context, malformed holder or any trailing octets.
pub fn key_holder<V: Source + Sink>(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
) -> Result<V, ProbeError> {
    let (value, consumed) = key_holder_prefix(ast, request, bytes)?;
    if consumed != bytes.len() {
        return Err(ProbeError::Mismatch {
            path: request.type_name.clone(),
            expected: "fully consumed key holder".into(),
            found: format!("{} trailing bytes", bytes.len() - consumed),
        });
    }
    Ok(value)
}

/// Decode a PLAIN_CDR2-BE holder from the start of `bytes` and say how many
/// octets it took; what follows is the caller's (DDSI-RTPS 2.5 §9.6.4.8
/// Step 5.1: a KeyHash is the holder padded with zeros to 16 octets).
/// # Errors
/// Unsupported key context or a malformed holder.
pub fn key_holder_prefix<V: Source + Sink>(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
) -> Result<(V, usize), ProbeError> {
    let (type_, members) = holder_members(ast, request)?;
    let key = request.key_holder_key();
    let keys = zerodds_ast_layout::keyhash::key_members_with(
        ast,
        type_,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .map_err(|e| ProbeError::NoLayout {
        detail: e.to_string(),
    })?;
    let frame = Frame {
        key,
        map_length: request.map_length,
        try_construct: TryConstruct::ABSENT,
        written_try_construct: None,
        default_from: None,
        wchar: WcharPosition::Member,
        depth: 0,
        is_discriminator: false,
    };
    // The writer's cap (`encode::key_holder`): PLAIN_CDR2's four, unless
    // the dialect aligns the holder to its data encoding or packs it.
    let mut reader = Reader::with_max_align(
        bytes,
        zerodds_wire::Endianness::Big,
        usize::from(key.key_holder_alignment.max_alignment(key.encoding)),
    )
    .with_wchar_width(crate::value::wire_wchar_width_of(key.wchar_width))
    .with_wchar_framing(crate::value::wire_wchar_framing_of(key.wchar_framing))
    .with_wstring_unit_order(crate::value::wire_wstring_unit_order_of(
        key.wstring_unit_order,
    ));
    let mut out = Aggregate::new();
    for id in keys {
        let Some(slot) = members
            .iter()
            .find(|m| ast.member(m.member).is_ok_and(|n| n.node_id == id))
        else {
            continue;
        };
        out.insert(
            slot.name.clone(),
            read_type(
                &mut reader,
                ast,
                slot.type_,
                frame,
                &format!("{}.{}", request.type_name, slot.name),
            )?,
        );
    }
    let consumed = bytes.len() - reader.remaining();
    defaults_from(ast, &members, &mut out, &request.type_name)?;
    Ok((V::aggregate(out), consumed))
}

/// Nothing may be left over that is not padding.
///
/// A reader that stopped short and said nothing is the silent-drop shape
/// this suite is built to catch: the sample would come back looking
/// plausible with its last members missing. Padding is the one thing a
/// writer can legitimately leave behind — a `@mutable` sentinel is
/// aligned, and RTI leaves uninitialised bytes in the gap — so the
/// remainder is allowed to be padding and nothing else.
///
/// # Padding is what alignment asks for, not anything under the cap
///
/// This used to accept any remainder shorter than the alignment cap,
/// which is up to three bytes under XCDR2 and seven under XCDR1
/// whatever the position. That is more than alignment can ever require:
/// from an offset the padding to the cap is fixed, and a byte beyond it
/// is not padding, it is a member nobody read.
///
/// The two bytes a `wstring` read under the wrong
/// [`WstringLengthPolicy`] leaves behind are exactly that shape —
/// `04000000 61006200 ab` read as a character count returns `"a"`,
/// takes `62` for the trailing octet and leaves two bytes. The old
/// check let it through and the sample came back with a one-character
/// string and a wrong octet.
///
/// # And the padding at the end of a payload reaches four, not the cap
///
/// §7.4.1.1 fixes the amount: "[RTPS] states that following the
/// serialized data submessage element, padding bytes shall be added so
/// that the next submessage starts at a 4-byte offset … any padding
/// bytes added at the end of the serialized data shall be set to zero."
/// Four, whatever `MAXALIGN` is — so under XCDR1, where the cap is 8, a
/// remainder of five to seven bytes is not padding either.
///
/// The difference is a whole value. `evo-appendable-nested` under XCDR1:
/// a three-member `@appendable` nested in a `@final` type, read by a
/// two-member version. The reader takes the third member's bytes for the
/// outer type's `long tail` and hands back 65533 where 42 was sent, then
/// stops four bytes early. Against the old cap of 8 those four bytes sat
/// inside the allowance and the wrong value came back with no error;
/// against four they do not. Cyclone refuses the same payload. RTI reads
/// the same wrong 65533 this project used to — measured 2026-08-09.
fn trailing(r: &Reader<'_>, path: &str) -> Result<(), ProbeError> {
    let left = r.remaining();
    if left == 0 {
        return Ok(());
    }
    let cap = r.max_align().min(4);
    let padding = (cap - (r.position() % cap)) % cap;
    if left <= padding {
        return Ok(());
    }
    Err(ProbeError::Mismatch {
        path: path.to_string(),
        expected: format!("the whole payload consumed, bar at most {padding} byte(s) of padding"),
        found: format!("{left} byte(s) left at offset {}", r.position()),
    })
}

const fn wire_endianness(endianness: Endianness) -> zerodds_wire::Endianness {
    match endianness {
        Endianness::Little => zerodds_wire::Endianness::Little,
        Endianness::Big => zerodds_wire::Endianness::Big,
    }
}

fn find_type(ast: &Ast, name: &str) -> Result<Id<TypeNode>, ProbeError> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    ast.find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| ProbeError::NoSuchType {
            name: name.to_string(),
        })
}

/// Read one value of one type.
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`. Every
/// descent into an aggregate goes through `Frame::in_aggregate`, and
/// `read_aggregate` and `read_union` refuse a stream past the count with
/// `ProbeError::ValueTooDeep` before a byte is read. The collection arms
/// descend the value too; the typedef arm does not, and is finite because
/// `read_aggregate` plans the type first and `zerodds_ast_layout::plan`
/// refuses a graph deeper than its own `MAX_DEPTH`
/// (`crates/ast-layout/src/plan.rs:388`).
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`. Every
/// descent into an aggregate goes through `Frame::in_aggregate`, and
/// `read_aggregate` and `read_union` refuse a stream past the count with
/// `ProbeError::ValueTooDeep` before a byte is read. The collection arms
/// descend the value too; the typedef arm does not, and is finite because
/// `read_aggregate` plans the type first and `zerodds_ast_layout::plan`
/// refuses a graph deeper than its own `MAX_DEPTH`
/// (`crates/ast-layout/src/plan.rs:388`).
#[allow(clippy::too_many_lines)]
fn read_type<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => read_primitive(r, *kind, frame.wchar, path),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => {
            let text = wire(r.read_string(), path)?;
            fit_string(ast, type_, text, *bound, CharWidth::Char, frame, path)
        }
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => {
            let text = if frame.key.wstring_code_units == WstringCodeUnits::Utf8 {
                read_wstring_utf8(
                    r,
                    frame.key.wstring_length,
                    layout_position(frame.wchar),
                    path,
                )?
            } else {
                read_wstring(
                    r,
                    frame.key.wstring_length,
                    frame.key.wstring_unit_order,
                    // The position the `wstring` is reached from, which the
                    // frame already carries for the scalar `wchar`. Passed
                    // and not assumed: C1/C2/C3/C4 are measured to answer
                    // the two positions alike in nine implementations, and
                    // an answer reached by not asking is how that stops
                    // being a measurement.
                    layout_position(frame.wchar),
                    path,
                )?
            };
            fit_string(ast, type_, text, *bound, CharWidth::WChar, frame, path)
        }
        TypeKind::Fixed { digits, scale } => {
            read_fixed(r, *digits, *scale, frame.key.fixed_digits, path)
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            read_type(r, ast, target, frame, path)
        }
        TypeKind::Enumeration { .. } => read_enumeration(r, ast, type_, frame, path),
        TypeKind::Bitmask { .. } => read_bitmask(r, ast, type_, frame.is_discriminator, path),
        TypeKind::Bitset { .. } => read_bitset(r, ast, type_, frame, path),
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            if collection_is_delimited(ast, element, frame.key) {
                let _ = wire(r.read_dheader(), path)?;
            }
            let count = wire(r.read_u32(), path)? as usize;
            if count > r.remaining() {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: "an element count the stream can hold".to_string(),
                    found: format!("{count} with {} byte(s) left", r.remaining()),
                });
            }
            // Every element is read whatever the bound says, and the
            // trim happens afterwards. Stopping at the bound would leave
            // the rest of the sequence in the stream and put every
            // following member at the wrong offset — a decode that
            // succeeds and is wrong, which is worse than the overflow it
            // was avoiding. OpenDDS does the same and then skips to the
            // collection's end.
            let inside = frame
                .resolved(ast.element_try_construct(type_, None))
                .in_collection();
            let mut items = Vec::with_capacity(count);
            // The mirror of the writer: a `sequence` element of a
            // non-`@final` type is preceded by its own octet length under
            // `SequenceElementDelimiter::BeforeEachNonFinalElement`, and
            // `ElementPadding::Packed` counts every element's alignment
            // from the element's own first octet.
            let carries_length = sequence_element_carries_length(ast, element, frame.key)?;
            let packed = frame.key.element_padding == ElementPadding::Packed;
            for index in 0..count {
                let element_path = format!("{path}[{index}]");
                let saved = packed.then(|| r.begin_packed_element());
                if carries_length {
                    let declared = wire(r.read_u32(), &element_path)? as usize;
                    let before = r.remaining();
                    items.push(read_type(r, ast, element, inside, &element_path)?);
                    let consumed = before.saturating_sub(r.remaining());
                    if consumed != declared {
                        return Err(ProbeError::Mismatch {
                            path: element_path,
                            expected: format!("an element of exactly {declared} octet(s)"),
                            found: format!("{consumed}"),
                        });
                    }
                } else {
                    items.push(read_type(r, ast, element, inside, &element_path)?);
                }
                if let Some(saved) = saved {
                    r.finish_packed_element(saved);
                }
            }
            let limit = bound_value(ast, bound);
            fit_collection(ast, items, limit, type_, frame, path, "elements")
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = dimensions(ast, dims);
            if array_is_delimited(ast, element, frame.key) {
                let _ = wire(r.read_dheader(), path)?;
            }
            // An array's bounds must match the sender's — Table 17
            // requires `T1.bounds[] == T2.bounds[]` — so an array never
            // overflows and has no member-level behaviour of its own.
            // Only its elements can fail, and `element_try_construct`
            // lets a member's own annotation reach them, which is the
            // only syntax there is for annotating an array's element.
            let inside = frame
                .resolved(ast.element_try_construct(type_, frame.written_try_construct))
                .in_collection();
            read_array_dimension(r, ast, element, inside, path, &counts)
        }
        TypeKind::Map {
            key,
            value: entry,
            bound,
        } => read_map(r, ast, type_, key, entry, bound, frame, path),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            read_aggregate(r, ast, type_, frame, path)
        }
        TypeKind::Union { .. } => read_union(r, ast, type_, frame, path),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// A string that has been read, against the bound the receiver declares.
///
/// The bound is not on the wire — a `string<32>` and a `string<5>`
/// serialise identically — so the whole value is read first and the fit
/// is decided here. Table 16: "An object O2 of type T2 can-construct an
/// object of type T1 if and only if O2.length <= T1.length."
///
/// # What a bound counts
///
/// Octets for a narrow string and code units for a wide one, which is
/// what the bound means to every peer that has to hold the value:
/// `string<5>` is `char[6]` in the C mapping of all four DDS
/// implementations, and `fixed_string<5>` in Fast DDS's. Counting
/// Unicode scalar values instead would accept a five-character value of
/// ten octets that no C peer can hold. The trim still lands on a
/// character boundary, because a half-decoded UTF-8 sequence is not a
/// string at all.
fn fit_string<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    text: String,
    bound: Bound,
    width: CharWidth,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    let Some(limit) = bound_value(ast, &bound).and_then(|limit| usize::try_from(limit).ok()) else {
        return Ok(V::text(text));
    };
    let length = match width {
        CharWidth::Char => text.len(),
        CharWidth::WChar => text.chars().map(char::len_utf16).sum(),
    };
    if length <= limit {
        return Ok(V::text(text));
    }
    let trimmed = V::text(trim_text(&text, limit, width));
    let found = format!("{length} of them");
    let unit = match width {
        CharWidth::Char => "octets",
        CharWidth::WChar => "code units",
    };
    narrowed(
        frame.try_construct,
        Some(trimmed),
        || frame.default_value(ast, type_, path),
        path,
        &format!("at most {limit} {unit}"),
        &found,
    )
}

/// `text` cut to `limit` units, never inside a character.
///
/// Table 11: "The order of the characters in the string ... is
/// preserved", so this takes a prefix and never reorders or elides.
fn trim_text(text: &str, limit: usize, width: CharWidth) -> String {
    let mut used = 0usize;
    let mut out = String::new();
    for character in text.chars() {
        let size = match width {
            CharWidth::Char => character.len_utf8(),
            CharWidth::WChar => character.len_utf16(),
        };
        // A character that would straddle the bound is left out whole. A
        // C peer holding `char[limit + 1]` cannot store half of one
        // either, and half a UTF-8 sequence is not text.
        if used + size > limit {
            break;
        }
        used += size;
        out.push(character);
    }
    out
}

/// A collection that has been read, against the bound the receiver
/// declares.
///
/// Table 17, the `SEQUENCE_TYPE` and `MAP_TYPE` rows: "An object O2 of
/// type T2 can construct T1 if and only if O2.length <= T1.length."
/// §7.3.1.2.1.12.3 is the `TRIM` case, and it takes the **first**
/// elements: "a sequence of 4 longs containing the first four elements
/// of O2.member".
fn fit_collection<V: Source + Sink>(
    ast: &Ast,
    items: Vec<V>,
    limit: Option<i128>,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
    what: &str,
) -> Result<V, ProbeError> {
    let Some(limit) = limit.and_then(|limit| usize::try_from(limit).ok()) else {
        return Ok(V::collection(items));
    };
    if items.len() <= limit {
        return Ok(V::collection(items));
    }
    let found = format!("{}", items.len());
    let mut trimmed = items;
    trimmed.truncate(limit);
    narrowed(
        frame.try_construct,
        Some(V::collection(trimmed)),
        || frame.default_value(ast, type_, path),
        path,
        &format!("at most {limit} {what}"),
        &found,
    )
}

/// The declared extents of an array, in declaration order.
fn dimensions(ast: &Ast, dims: &[Id<zerodds_ast::ConstExprNode>]) -> Vec<usize> {
    let mut counts = Vec::with_capacity(dims.len());
    for dim in dims {
        // A dimension that does not evaluate is a zero-length one, which
        // is what the encoder also does with it — the two have to make
        // the same mistake or the round trip reports the frontend's gap
        // as a wire difference.
        let count = match ast.eval_integral(*dim) {
            Ok(value) => usize::try_from(value).unwrap_or(0),
            Err(_) => 0,
        };
        counts.push(count);
    }
    counts
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `read_type`, which is bounded by `crate::MAX_VALUE_DEPTH`.
fn read_array_dimension<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    element: Id<TypeNode>,
    frame: Frame,
    path: &str,
    counts: &[usize],
) -> Result<V, ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return read_type(r, ast, element, frame, path);
    };
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let saved =
            (frame.key.element_padding == ElementPadding::Packed).then(|| r.begin_packed_element());
        items.push(read_array_dimension(
            r,
            ast,
            element,
            frame,
            &format!("{path}[{index}]"),
            rest,
        )?);
        if let Some(saved) = saved {
            r.finish_packed_element(saved);
        }
    }
    Ok(V::collection(items))
}

/// A structure or a valuetype, framed as its plan says.
///
/// Three shapes, and which one applies is the extensibility, read the
/// same way the planner reads it rather than inferred back out of the
/// plan it produced.
fn read_aggregate<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    // Before a byte is read: a stream can nest a recursive type as
    // deeply as it likes, and following it is how a reader runs out of
    // stack instead of saying what is wrong.
    frame.within_depth(path)?;

    // `ElementPadding::Packed`: an aggregate is its own alignment block
    // on the read side too. See the writer's `write_aggregate`.
    let packed = frame.key.element_padding == ElementPadding::Packed;
    let saved_origin = packed.then(|| r.begin_packed_element());

    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;

    let end = if layout.needs_dheader {
        let length = wire(r.read_dheader(), path)?;
        Some(r.position().saturating_add(length))
    } else {
        None
    };

    let members = member_table(ast, type_, &layout);
    let node = ast.type_(type_).map_err(|e| ProbeError::NoLayout {
        detail: e.to_string(),
    })?;
    let extensibility = zerodds_ast_layout::xcdr::extensibility_of(ast, node, frame.key);
    let holder = frame.key.purpose == Purpose::Key;
    let original_key = frame.key;
    let frame = Frame {
        key: zerodds_ast_layout::member_key(ast, type_, frame.key),
        ..frame
    };
    let mutable = extensibility == Extensibility::Mutable
        && frame.key.aggregate_delimiter.frames_mutable_members();
    let mut out = if mutable && frame.key.encoding.is_v1() {
        read_parameter_list(r, ast, &members, frame, path)?
    } else if mutable {
        read_member_headers(r, ast, &members, frame, path, end)?
    } else {
        let tail = Tail {
            end,
            appendable: extensibility == Extensibility::Appendable,
        };
        read_in_plan_order(r, ast, &members, frame, path, tail)?
    };

    if let Some(end) = end {
        if r.position() > end {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: format!("the members to end at offset {end}, as the DHEADER said"),
                found: format!("offset {}", r.position()),
            });
        }
        // The delimiter is what a reader skips an unknown tail by. An
        // appendable type read by a peer that has fewer members than the
        // writer had leaves exactly such a tail, and the DHEADER is the
        // only thing that says where it ends.
        skip_to(r, end);
    }
    if holder {
        let data_key = LayoutKey {
            purpose: Purpose::Data,
            ..original_key
        };
        let full = plan(ast, type_, data_key, &mut DepCollector::default()).map_err(|e| {
            ProbeError::NoLayout {
                detail: e.to_string(),
            }
        })?;
        defaults_from(ast, &member_table(ast, type_, &full), &mut out, path)?;
    }
    if let Some(saved) = saved_origin {
        r.finish_packed_element(saved);
    }
    Ok(V::aggregate(out))
}

/// What the plan says about each member, with the model's own answer for
/// the name, the type and whether it may be left out.
struct MemberSlot {
    name: String,
    type_: Id<TypeNode>,
    optional: bool,
    member_id: u32,
    header: MemberHeader,
    /// The declaration, for the default value it may carry.
    member: Id<zerodds_ast::MemberNode>,
    /// `@try_construct` as the member wrote it, before the alias chain
    /// of its type is consulted. DDS-XTypes 1.3 §7.2.2.7.
    try_construct: Option<TryConstruct>,
}

impl MemberSlot {
    /// The frame this member's value is read under.
    fn frame(&self, ast: &Ast, frame: Frame) -> Frame {
        frame
            .for_member(ast, self.try_construct, Some(self.type_), self.member)
            .in_aggregate()
    }
}

fn member_table(
    ast: &Ast,
    type_: Id<TypeNode>,
    layout: &zerodds_ast_layout::TypeLayout,
) -> Vec<MemberSlot> {
    let all = ast.all_members_of(type_);
    let mut out = Vec::with_capacity(layout.members.len());
    for member in &layout.members {
        let mut found = None;
        for id in &all {
            let Ok(node) = ast.member(*id) else { continue };
            if node.node_id == member.member {
                found = Some((*id, node));
                break;
            }
        }
        let Some((id, node)) = found else { continue };
        let Some(member_type) = node.type_.resolved() else {
            continue;
        };
        out.push(MemberSlot {
            name: node.name.clone(),
            type_: member_type,
            optional: node.facets.is_optional(),
            member_id: member.member_id,
            header: member.header,
            member: id,
            try_construct: node.facets.xtypes.as_ref().and_then(|x| x.try_construct),
        });
    }
    out
}

/// Where a positional aggregate's members stop, which is not always
/// where this reader's plan stops.
///
/// The half of appendable evolution a deployed system meets first: a new
/// subscriber reading an old publisher. §7.2.4.4.8, Table 19 —
/// "Each non-optional member in a T1 object that is not present in the
/// T2 object takes its default value."
#[derive(Debug, Clone, Copy)]
struct Tail {
    /// The delimiter's extent, when the type has one.
    end: Option<usize>,
    /// Whether the aggregate is `@appendable`.
    appendable: bool,
}

impl Tail {
    /// Whether the writer's members have run out at this point.
    ///
    /// Two ways, and both are needed because §7.2.4.2 delimits an
    /// appendable type under encoding version 2 and not under version 1.
    ///
    /// - **The delimiter is used up.** `end` is `Some` for an
    ///   `@appendable` type under XCDR2, and the DHEADER is exactly the
    ///   thing that says how far the writer's members went.
    /// - **The payload is used up, in an `@appendable` type.** Under
    ///   XCDR1 there is no delimiter, and all three vendors default the
    ///   tail anyway when the bytes run out: measured 2026-08-09,
    ///   `evo-appendable-tail` under `xcdr1`, where Cyclone, RTI and Fast
    ///   DDS each hand back `c = 0` for a two-member writer read as a
    ///   three-member type. Refusing there would mean this project could
    ///   not receive from any of the three on an evolved XCDR1 topic.
    ///
    /// The second is deliberately `remaining() == 0` and not "fewer
    /// bytes than the member needs". An appendable type *nested* inside
    /// another still has the enclosing type's bytes behind it, so
    /// nothing here fires and the member is read out of them — which
    /// fails loudly on the next member, and is what Cyclone and RTI do
    /// with the same case (`evo-appendable-nested` under `xcdr1`, both
    /// refuse). Guessing there would take the outer type's bytes for a
    /// default and hand back a sample that looked right.
    ///
    /// `@final` gets neither: Table 19 requires the same set of member
    /// IDs on both sides, so a short stream is a type mismatch and not
    /// an evolution. Cyclone and RTI refuse it, Fast DDS defaults —
    /// `evo-final-tail` — and the clause is with the two.
    fn reached(self, r: &Reader<'_>) -> bool {
        match self.end {
            Some(end) => r.position() >= end,
            None => self.appendable && r.remaining() == 0,
        }
    }
}

/// A `@final` or `@appendable` aggregate: the members in plan order.
///
/// Every member is where the plan says it is, including an `@optional`
/// one: §7.4.3.5.2 rule (20) puts a BOOLEAN `<is_present>` in front of
/// it under XCDR2, and §7.4.1.2 gives it a parameter header with a
/// declared length of zero under XCDR1. Both mark the gap, so nothing
/// here is guessed and nothing rewinds.
///
/// This used to read the header into a clone of the reader and keep it
/// only when the member id matched, on the reading that nothing marks an
/// absent optional member. That reading was wrong — see the two clauses
/// above and `vendor_bytes.md` §1, where all three implementations write
/// the marker — and it was wrong in the direction that returns a value:
/// a vendor's `00000000` was taken for a header that did not match, the
/// member was called absent, and the four bytes that followed were read
/// as the *next* member.
fn read_in_plan_order<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    members: &[MemberSlot],
    frame: Frame,
    path: &str,
    tail: Tail,
) -> Result<Aggregate<V>, ProbeError> {
    let mut out = Aggregate::new();
    for (index, slot) in members.iter().enumerate() {
        let member_path = format!("{path}.{}", slot.name);
        if tail.reached(r) {
            defaults_from(ast, &members[index..], &mut out, path)?;
            break;
        }
        match slot.header {
            // Rule (18).
            MemberHeader::NoHeader => {
                out.insert(
                    slot.name.clone(),
                    read_type(r, ast, slot.type_, slot.frame(ast, frame), &member_path)?,
                );
            }
            // Rule (20). `read_bool` refuses anything but 0 and 1, so a
            // byte that is neither is a stream this type does not
            // describe rather than a presence flag to guess at.
            MemberHeader::OptionalFlag => {
                if wire(r.read_bool(), &member_path)? {
                    out.insert(
                        slot.name.clone(),
                        read_type(r, ast, slot.type_, slot.frame(ast, frame), &member_path)?,
                    );
                }
            }
            // Rule (19) and §7.4.1.2. The header is there either way and
            // a length of zero is what says the member is not.
            MemberHeader::OptionalCdr1 { .. } => {
                let header = wire(r.read_pid_as(ParameterRole::Optional), &member_path)?;
                if header.end || header.pid != slot.member_id {
                    return Err(ProbeError::Mismatch {
                        path: member_path,
                        expected: format!(
                            "a parameter header for member id {}, which §7.4.1.2 writes \
                             whether or not the member has a value",
                            slot.member_id
                        ),
                        found: if header.end {
                            format!("PID_LIST_END at offset {}", r.position())
                        } else {
                            format!("member id {} at offset {}", header.pid, r.position())
                        },
                    });
                }
                if header.length > 0 {
                    let at = r.position();
                    out.insert(
                        slot.name.clone(),
                        read_type(r, ast, slot.type_, slot.frame(ast, frame), &member_path)?,
                    );
                    overran(r, at, header.length, &member_path)?;
                }
                wire(r.finish_pid(header), &member_path)?;
            }
            // The planner gives these two only to a member of a
            // `@mutable` type, which is read by one of the two functions
            // below. Reaching them here would mean the plan and the walk
            // disagree about the extensibility, and reading on would put
            // every member after this one at the wrong offset.
            MemberHeader::EmHeader { .. } | MemberHeader::PidCdr1 { .. } => {
                return Err(ProbeError::NoLayout {
                    detail: format!(
                        "{member_path}: an MMEMBER header in a type that is not `@mutable`"
                    ),
                });
            }
        }
    }
    Ok(out)
}

/// A `@mutable` aggregate under XCDR2: EMHEADERs until the delimiter's
/// extent is used up.
///
/// Driven by the headers and not by the plan, because that is what the
/// headers are for. A writer may order the members as it likes and may
/// leave any `@optional` one out; a reader that walked its own plan
/// instead would misread both.
fn read_member_headers<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    members: &[MemberSlot],
    frame: Frame,
    path: &str,
    end: Option<usize>,
) -> Result<Aggregate<V>, ProbeError> {
    let Some(end) = end else {
        return Err(ProbeError::NoLayout {
            detail: format!("{path}: a mutable type under XCDR2 with no DHEADER planned"),
        });
    };
    let mut out = Aggregate::new();
    while r.position() < end {
        let at = r.position();
        let header = wire(r.read_emheader(), path)?;
        let mut slot = None;
        for candidate in members {
            if candidate.member_id == header.member_id {
                slot = Some(candidate);
                break;
            }
        }
        // Where the member ends. `extent` counts from where the reader
        // now is under every length code, including the three whose
        // NEXTINT is the member's own first word and was peeked rather
        // than consumed. §7.4.3.4.2.
        let ends_at = r.position().saturating_add(header.extent);
        let Some(slot) = slot else {
            // A member the model does not declare. `must_understand`
            // says which of the two things to do about it, and §7.4.3.5.3
            // says a reader that cannot skip it must not pretend it did.
            if header.must_understand {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: "a member the type declares".to_string(),
                    found: format!(
                        "member id {} with must_understand set, at offset {at}",
                        header.member_id
                    ),
                });
            }
            skip_to(r, ends_at);
            continue;
        };
        let member_path = format!("{path}.{}", slot.name);
        let value = read_type(r, ast, slot.type_, slot.frame(ast, frame), &member_path)?;
        if r.position() != ends_at {
            return Err(ProbeError::Mismatch {
                path: member_path,
                expected: format!(
                    "the member to end at offset {ends_at}, as length code {} said",
                    header.length_code
                ),
                found: format!("offset {}", r.position()),
            });
        }
        if out.insert(slot.name.clone(), value).is_some() {
            return Err(ProbeError::Mismatch {
                path: member_path,
                expected: "each member once".to_string(),
                found: "a second EMHEADER for the same member id".to_string(),
            });
        }
    }
    require_present(ast, members, &mut out, path)?;
    Ok(out)
}

/// A `@mutable` aggregate under XCDR1: parameters until `PID_LIST_END`.
fn read_parameter_list<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    members: &[MemberSlot],
    frame: Frame,
    path: &str,
) -> Result<Aggregate<V>, ProbeError> {
    let mut out = Aggregate::new();
    loop {
        if r.remaining() < 4 {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a PID_LIST_END sentinel".to_string(),
                found: format!("the stream ended at offset {}", r.position()),
            });
        }
        let header = wire(r.read_pid(), path)?;
        if header.end {
            break;
        }
        let at = r.position();
        // `PID_IGNORE` outranks everything below, including the
        // must-understand bit: §7.4.1.2.2 says "All consumers of this
        // Data Representation shall ignore parameters with this ID" and
        // states no exception. Until this ran, 0x3F03 was ignored only
        // by accident — by falling through the unknown-parameter skip —
        // and a peer that set the flag on one got the sample discarded.
        if header.ignore {
            skip_to(r, at.saturating_add(header.length));
            wire(r.finish_pid(header), path)?;
            continue;
        }
        let mut slot = None;
        // A vendor-private parameter (`FLAG_IMPL_EXTENSION`) and one
        // whose id is inside §7.4.1.2.2's reserved range are not members
        // of this type whatever their low bits say, so neither is looked
        // up. Reading them as members took a vendor's own payload for
        // the value of the member with the same fourteen low bits.
        if header.is_user_member() {
            for candidate in members {
                if candidate.member_id == header.pid {
                    slot = Some(candidate);
                    break;
                }
            }
        }
        // A parameter this type does not declare. PL_CDR1 carries a
        // length on every parameter, so skipping is always possible —
        // but §7.2.2.4.4.4.6 says a reader that does not understand a
        // must-understand member shall discard the whole sample, not
        // step over it. Skipping it silently was the XCDR1 half of M6;
        // the XCDR2 half in `read_member_headers` already refused.
        let Some(slot) = slot else {
            if header.must_understand {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: "a member the type declares".to_string(),
                    found: format!(
                        "member id {} with must_understand set, at offset {at}",
                        header.pid
                    ),
                });
            }
            skip_to(r, at.saturating_add(header.length));
            wire(r.finish_pid(header), path)?;
            continue;
        };
        let member_path = format!("{path}.{}", slot.name);
        let value = read_type(r, ast, slot.type_, slot.frame(ast, frame), &member_path)?;
        overran(r, at, header.length, &member_path)?;
        if out.insert(slot.name.clone(), value).is_some() {
            return Err(ProbeError::Mismatch {
                path: member_path,
                expected: "each member once".to_string(),
                found: "a second parameter for the same id".to_string(),
            });
        }
        wire(r.finish_pid(header), path)?;
    }
    require_present(ast, members, &mut out, path)?;
    Ok(out)
}

/// A parameter's value may not be longer than the header said it was.
///
/// The other way round is legitimate and is why `Reader::finish_pid`
/// finds the next header by alignment rather than by adding the length:
/// RTI rounds a member's declared length up to the alignment where Fast
/// DDS writes the exact one, and `vendor_bytes.md` §2 records both for
/// the same `short`. Reading *past* the declared length is neither, and
/// it means the reader and the writer disagree about the type.
fn overran(r: &Reader<'_>, at: usize, length: usize, path: &str) -> Result<(), ProbeError> {
    if r.position() > at.saturating_add(length) {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("a value within the {length} byte(s) the header declared"),
            found: format!("{} byte(s)", r.position().saturating_sub(at)),
        });
    }
    Ok(())
}

/// The members the stream did not carry, each taking its default.
///
/// §7.4.1.2.3: "any mutable structure member's value may be omitted. For
/// non-optional members this may be due to the sender having a
/// compatible type that does not have that member. In such a case, if
/// the receiver's member is not optional, it logically takes its default
/// value. If the member is optional, it takes no value at all."
///
/// This is what `@mutable` is for, and until it existed a new subscriber
/// meeting an old publisher got `ProbeError::Missing` — a hard failure
/// on the one exchange the extensibility kind was designed to allow.
///
/// # `@must_understand` does not except a member from this
///
/// It reads as though it should. §7.2.4.4.8, Table 19, makes it a
/// condition of assignability that members "for which both optional is
/// false and must_understand is true in either T1 or T2 appear … in both
/// T1 and T2", so a stream that omits one did not come from an
/// assignable type.
///
/// It is not the decoder's condition to enforce, and the measurement
/// says so plainly. `evo-mutable-mustunderstand` on 2026-08-09: Cyclone,
/// RTI **and** Fast DDS each hand back `c = 0` for a receiver whose
/// `@id(3) @must_understand long c` the writer's type did not have, in
/// both encoding versions. All three, both ways round.
///
/// They are right. §7.2.2.4.4.4.6 defines the attribute for one
/// direction only — "a data consumer, upon identifying a member it does
/// not recognize, must discard the entire data sample" — which is M6 and
/// is enforced where an *unknown* id arrives. The reverse has no such
/// rule, and §7.4.1.2.3 states the default with no exception in it:
/// "if the receiver's member is not optional, it logically takes its
/// default value". Table 19's condition is a static type-compatibility
/// check, which in DDS belongs to discovery and
/// `TypeConsistencyEnforcementQosPolicy` (§7.6.3.3) — a layer above this
/// one, which sees both TypeObjects and can refuse the match before a
/// sample is ever written. A decoder that refused here would break the
/// exchange all three vendors allow, on nothing the bytes say.
///
/// The same holds for `@key`, which §7.2.2.4.4.4.8 makes must-understand
/// by definition.
fn require_present<V: Source + Sink>(
    ast: &Ast,
    members: &[MemberSlot],
    out: &mut Aggregate<V>,
    path: &str,
) -> Result<(), ProbeError> {
    defaults_from(ast, members, out, path)
}

/// Fill in every member of `members` the stream left out.
///
/// Shared by the two `@mutable` readers, which know which ids arrived,
/// and by the `@appendable` reader, which knows where the writer's
/// members stopped and passes the tail from there.
fn defaults_from<V: Source + Sink>(
    ast: &Ast,
    members: &[MemberSlot],
    out: &mut Aggregate<V>,
    path: &str,
) -> Result<(), ProbeError> {
    for slot in members {
        if out.contains(&slot.name) {
            continue;
        }
        // "If the member is optional, it takes no value at all."
        // §7.2.2.4.4.4.7 is explicit that this is not a defaulted zero:
        // "Implementations shall not provide a default value in such a
        // case."
        if slot.optional {
            continue;
        }
        let member_path = format!("{path}.{}", slot.name);
        let node = ast
            .member(slot.member)
            .map_err(|error| ProbeError::NoLayout {
                detail: format!("{member_path}: {error}"),
            })?;
        out.insert(
            slot.name.clone(),
            crate::default::member(ast, node, slot.type_, &member_path)?,
        );
    }
    Ok(())
}

/// Whether a framed member's id has to be the one the plan names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberIdCheck {
    /// The stream's id must equal the plan's. Every read but one.
    Exact,
    /// Whatever id the stream carries is accepted, and the body behind
    /// it is read as the type the plan names.
    ///
    /// Reached from one place only:
    /// [`DefaultedDiscriminatorMember::StreamValue`] with a union whose
    /// discriminator was defaulted. The member the writer framed is the
    /// one the *writer's* discriminator selected, so its id is the
    /// writer's and there is nothing on this side to check it against —
    /// this reader's discriminator no longer names that case. Under
    /// [`DefaultedDiscriminatorMember::TypeDefault`] the member is never
    /// read at all, so the id never has to be believed.
    Streamed,
}

impl MemberIdCheck {
    /// Whether the id is checked.
    const fn is_exact(self) -> bool {
        matches!(self, Self::Exact)
    }
}

/// One `MMEMBER` of a `@mutable` type, read as its plan says.
///
/// The counterpart of `encode.rs::write_mmember`, and the same reason
/// for existing: rules (21) to (25) and rules (27) and (28) frame a
/// structure's members and a union's discriminator and selected member
/// identically, so one function reads both.
///
/// The id is checked rather than trusted, unless the caller hands in
/// [`MemberIdCheck::Streamed`]. A header for another member is not a
/// member this reader can skip past — the value behind it belongs to
/// something else, and reading on would take every following byte out
/// of the wrong place.
#[allow(clippy::too_many_arguments)]
fn read_mmember<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    header: MemberHeader,
    member_id: u32,
    id_check: MemberIdCheck,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    match header {
        MemberHeader::PidCdr1 { .. } => {
            let parameter = wire(r.read_pid(), path)?;
            if parameter.end || (id_check.is_exact() && parameter.pid != member_id) {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: format!("a parameter header for member id {member_id}"),
                    found: if parameter.end {
                        format!("PID_LIST_END at offset {}", r.position())
                    } else {
                        format!("member id {} at offset {}", parameter.pid, r.position())
                    },
                });
            }
            let at = r.position();
            // A discarded member still has to leave the reader where the
            // next parameter starts. The caller may be able to carry on
            // — a union whose discriminator took its default has to step
            // over the member the writer selected — and a reader parked
            // mid-parameter would read the next one out of the middle of
            // this one. The framing is closed first and the refusal is
            // re-raised after.
            let value = match read_type(r, ast, type_, frame, path) {
                Ok(value) => value,
                Err(discarded @ ProbeError::Discarded { .. }) => {
                    skip_to(r, at.saturating_add(parameter.length));
                    wire(r.finish_pid(parameter), path)?;
                    return Err(discarded);
                }
                Err(other) => return Err(other),
            };
            overran(r, at, parameter.length, path)?;
            wire(r.finish_pid(parameter), path)?;
            Ok(value)
        }
        MemberHeader::EmHeader { .. } => {
            let emheader = wire(r.read_emheader(), path)?;
            if id_check.is_exact() && emheader.member_id != member_id {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: format!("an EMHEADER for member id {member_id}"),
                    found: format!(
                        "member id {} at offset {}",
                        emheader.member_id,
                        r.position()
                    ),
                });
            }
            let ends_at = r.position().saturating_add(emheader.extent);
            // As above: the extent is known, so a discarded member is
            // stepped over before the refusal goes up.
            let value = match read_type(r, ast, type_, frame, path) {
                Ok(value) => value,
                Err(discarded @ ProbeError::Discarded { .. }) => {
                    skip_to(r, ends_at);
                    return Err(discarded);
                }
                Err(other) => return Err(other),
            };
            if r.position() != ends_at {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: format!("the member to end at offset {ends_at}"),
                    found: format!("offset {}", r.position()),
                });
            }
            Ok(value)
        }
        MemberHeader::NoHeader | MemberHeader::OptionalCdr1 { .. } | MemberHeader::OptionalFlag => {
            Err(ProbeError::NoLayout {
                detail: format!(
                    "{path}: a `@mutable` member planned with a header that is not one"
                ),
            })
        }
    }
}

/// Whether a union case takes a number of octets its own type fixes.
///
/// Read from one place: `DefaultedDiscriminatorMember::StreamValue` over
/// a union that states no extent of its own. A fixed-extent case is safe
/// to read out of octets the writer meant for a different member — it
/// takes what its type takes, and the reader ends at a position this
/// type decides — while an unbounded one would read its length out of
/// those octets and could run to the end of the buffer.
///
/// A plan that will not compute is not a bound either, so it answers
/// false and the member is stepped over.
fn case_extent_is_fixed(ast: &Ast, type_: Id<TypeNode>, frame: Frame) -> bool {
    let mut deps = DepCollector::new();
    plan(ast, type_, frame.key, &mut deps).is_ok_and(|layout| layout.extent.is_sized())
}

/// A union: the discriminator, then the member its labels select.
///
/// A `@mutable` union frames both as `MMEMBER`s — rules (27) and (28) —
/// and the discriminator comes first, because a reader cannot know which
/// member is selected until it has read it. RTI and Fast DDS both write
/// it first; a peer that reordered the two would be unreadable by any of
/// the three, so the order is required here rather than searched for.
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`, checked by
/// the `frame.within_depth` at the top of this function before any header
/// is read.
fn read_union<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    // As in `read_aggregate`: a union on a cycle nests a value just as
    // a structure does.
    frame.within_depth(path)?;

    // As in `read_aggregate`: under `ElementPadding::Packed` a union is
    // its own alignment block.
    let packed = frame.key.element_padding == ElementPadding::Packed;
    let saved_origin = packed.then(|| r.begin_packed_element());

    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let end = if layout.needs_dheader {
        let length = wire(r.read_dheader(), path)?;
        Some(r.position().saturating_add(length))
    } else {
        None
    };

    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a union whose kind changed under the probe",
        });
    };
    let discriminator = discriminator
        .resolved()
        .ok_or_else(|| ProbeError::NoLayout {
            detail: format!("{path}: the discriminator is unresolved"),
        })?;

    let discriminant_path = format!("{path}._d");
    // §7.3.1.2.1.12.5 puts `@try_construct` on the discriminator itself:
    // `union U switch (@try_construct K)`. Table 19 admits only DEFAULT
    // there, and the frontend refuses TRIM, so the only two outcomes are
    // a discarded sample and the discriminator's Table 9 default.
    //
    // Read under DISCARD first, so that a discriminator this type cannot
    // construct comes back as an error rather than as a value: the
    // difference decides not only what the discriminator is but whether
    // the *member* the stream carries can be believed. §7.2.4.4.8's
    // UNION_TYPE row defaults the discriminator and then sets the member
    // T1 now selects to *its* default; it does not take the writer's
    // member, and it cannot — §7.2.4.1 rules out "any inspection of a
    // data producer's type definition", and without one there is no way
    // to know what type the writer's member had. Measured twice, on two
    // vendors: OpenDDS hands back `d=0 v=0` for §7.3.1.2.1.12.5's own
    // example, not the 42 the writer put in `e3_value`; and RTI Connext
    // 7.7.0, given a writer that sends VAL3 and a reader whose
    // discriminator defaults to VAL1, hands back `{"x1":0}` — it does not
    // read the writer's four octets as `x1`. RTI reaches that outcome
    // without the annotation at all: `rtiddsgen` reports `try_construct`
    // as ignored, and the shipped Extensible Types guide lists it among
    // the annotations the middleware ignores. Which makes it a witness
    // for the *default* behaviour §7.2.4.1 describes, not for a reading
    // of the annotation.
    //
    // The dds-xtypes suite expects the other answer — `{"x1":3}`, the
    // member kept. That disagreement is an axis rather than a verdict:
    // `DefaultedDiscriminatorMember::StreamValue` reads the writer's
    // octets as the member this type now selects, and
    // `DefaultedDiscriminatorMember::TypeDefault` — the default, which
    // is what RTI and OpenDDS hand back — steps over them. Of the four
    // implementations that can be made to run the case, three default
    // the member and only CoreDX 6.14 keeps it, that one on the strength
    // of its exclusion list rather than a measurement. Fast DDS 3.6.1
    // cannot express the case: it refuses annotations on union
    // discriminators outright, and ties enum assignability to the enum's
    // identity, so the suite's whole E1/E2 family never matches there.
    // A discriminator is a member position, not an element one.
    let discarding = Frame {
        is_discriminator: true,
        ..frame.resolved(TryConstruct::Discard).in_aggregate()
    };
    let raw = match layout.discriminator {
        None => read_type(r, ast, discriminator, discarding, &discriminant_path),
        Some(header) => read_mmember(
            r,
            ast,
            header,
            0,
            MemberIdCheck::Exact,
            discriminator,
            discarding,
            &discriminant_path,
        ),
    };
    let mut discriminator_defaulted = false;
    let discriminant = match raw {
        Ok(value) => value,
        Err(discarded @ ProbeError::Discarded { .. }) => {
            if ast.discriminator_try_construct(type_) != TryConstruct::UseDefault {
                return Err(discarded);
            }
            discriminator_defaulted = true;
            crate::default::of_type(ast, discriminator, &discriminant_path)?
        }
        Err(other) => return Err(other),
    };
    let selector = selector_of(ast, discriminator, &discriminant, &discriminant_path)?;

    let mut out = Aggregate::new();
    out.insert("_d".to_string(), discriminant);
    // Whether the sentinel the parameter list ends with has been eaten
    // already, which happens where the writer's union selected no member
    // and this one's does.
    let mut sentinel_seen = false;
    let suppress_negative = union_negative_matches_nothing(ast, type_, frame.key.union_label);
    let selected =
        select_case(ast, type_, selector, suppress_negative).map_err(|_| ProbeError::Mismatch {
            path: path.to_string(),
            expected: String::from("a union whose case labels all evaluate"),
            found: String::from("a case label the model cannot evaluate"),
        })?;
    // The axis, and the one place it is read. Two conditions beyond the
    // axis value itself, and neither is a restriction of convenience:
    //
    // - a case to read the octets into. Where the defaulted
    //   discriminator selects none, `StreamValue` has nothing to say and
    //   the member is stepped over as under the default.
    // - a bound on how far the read can run. The octets are read as
    //   *this* type's case, which is not the type the writer wrote, so
    //   the length prefix of a `string`, `sequence` or `map` would be
    //   taken out of octets that were never a length. A delimiter and a
    //   sentinel each state where the union ends and every read inside
    //   them is checked against it; where the union states neither — an
    //   undelimited `@final` or `@appendable` union under XCDR1 — a
    //   fixed-extent case is the bound, because it takes exactly as many
    //   octets as its own type has and no number in the stream decides
    //   how many. An unbounded case there has no bound at all, and is
    //   stepped over instead, which is the refusal below.
    let keep_streamed_member = discriminator_defaulted
        && frame.key.defaulted_discriminator_member == DefaultedDiscriminatorMember::StreamValue
        && selected.as_ref().is_some_and(|case| {
            end.is_some() || layout.needs_sentinel || case_extent_is_fixed(ast, case.type_, frame)
        });
    if discriminator_defaulted && !keep_streamed_member {
        // The stream's member belongs to a case this type does not
        // have, and the reader has to get past it to whatever follows.
        // Two framings say where that is and one does not: a delimiter
        // states the extent, a PL_CDR1 parameter list ends at its
        // sentinel, and an undelimited `@final` or `@appendable` union
        // under XCDR1 states nothing at all. Guessing the member's
        // length in the third case would hand back a sample assembled
        // out of the wrong offsets.
        match (end, layout.needs_sentinel) {
            (Some(end), _) => skip_to(r, end),
            (None, true) => skip_parameter_list(r, path)?,
            (None, false) => {
                return Err(ProbeError::Mismatch {
                    path: discriminant_path,
                    expected: "a delimiter or a sentinel, so that a defaulted \
                               discriminator can step over the member the writer \
                               selected — or read it, under \
                               `DefaultedDiscriminatorMember::StreamValue`"
                        .to_string(),
                    found: "an undelimited union, whose member extent nothing states".to_string(),
                });
            }
        }
    }

    if let Some(case) = selected {
        let member_path = format!("{path}.{}", case.name);
        // The discriminator was defaulted, the reader is already past
        // the writer's member, and the case this type now selects had no
        // value in the stream: Table 19's "m1 is set to its default
        // value according to its type".
        if discriminator_defaulted && !keep_streamed_member {
            let value = crate::default::of_type(ast, case.type_, &member_path)?;
            out.insert(case.name, value);
            return Ok(V::aggregate(out));
        }
        // Under `StreamValue` the framed member carries the writer's
        // member id, which this reader has nothing to check against.
        let id_check = if keep_streamed_member {
            MemberIdCheck::Streamed
        } else {
            MemberIdCheck::Exact
        };
        let case_frame = frame
            .constructing(ast, case.try_construct, Some(case.type_))
            .in_aggregate();
        let value = match layout.members.iter().find(|plan| plan.member == case.node) {
            None => read_type(r, ast, case.type_, case_frame, &member_path)?,
            Some(plan) => {
                // §7.2.4.4.8, Table 19, the UNION_TYPE row: "If the
                // discriminator value does not select any member in O2,
                // then: If the discriminator value selects a member m1
                // in O1, then m1 is set to its default value according
                // to its type." Two versions of a union whose newer one
                // gave an existing discriminator value a case is exactly
                // that, and RTI is the witness — `evo-union-unselected`
                // on 2026-08-09 hands back `{"_d":2,"b":0}` in both
                // encoding versions.
                //
                // Which of the two the stream is saying is read off the
                // framing and never guessed: under XCDR2 the DHEADER
                // says the union is over, and under XCDR1 the sentinel
                // does.
                match union_member_frame(
                    r,
                    plan.header,
                    plan.member_id,
                    id_check,
                    end,
                    &member_path,
                )? {
                    UnionMember::AbsentAtSentinel => {
                        sentinel_seen = true;
                        crate::default::of_type(ast, case.type_, &member_path)?
                    }
                    UnionMember::AbsentAtDelimiter => {
                        crate::default::of_type(ast, case.type_, &member_path)?
                    }
                    UnionMember::Parameter(parameter) => {
                        let at = r.position();
                        let value = read_type(r, ast, case.type_, case_frame, &member_path)?;
                        overran(r, at, parameter.length, &member_path)?;
                        wire(r.finish_pid(parameter), &member_path)?;
                        value
                    }
                    UnionMember::Framed => read_mmember(
                        r,
                        ast,
                        plan.header,
                        plan.member_id,
                        id_check,
                        case.type_,
                        case_frame,
                        &member_path,
                    )?,
                }
            }
        };
        out.insert(case.name, value);
    }

    // Rule (28): the parameter list ends at its sentinel, whether or not
    // a member was selected. A defaulted discriminator that stepped over
    // the writer's member is already at the delimiter's end, sentinel and
    // all; one that read it is not, and closes the list the ordinary way.
    if discriminator_defaulted && !keep_streamed_member {
        if let Some(saved) = saved_origin {
            r.finish_packed_element(saved);
        }
        return Ok(V::aggregate(out));
    }
    if layout.needs_sentinel && !sentinel_seen {
        let sentinel = wire(r.read_pid(), path)?;
        if !sentinel.end {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "PID_LIST_END closing the union's parameter list".to_string(),
                found: format!("member id {} at offset {}", sentinel.pid, r.position()),
            });
        }
    }

    if let Some(end) = end {
        if r.position() > end {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: format!("the union to end at offset {end}, as the DHEADER said"),
                found: format!("offset {}", r.position()),
            });
        }
        skip_to(r, end);
    }
    if let Some(saved) = saved_origin {
        r.finish_packed_element(saved);
    }
    Ok(V::aggregate(out))
}

/// Step over the rest of a PL_CDR1 parameter list, to its sentinel.
///
/// Each parameter states its own length, so this walks and never
/// guesses. A stream that runs out before the sentinel is malformed and
/// says so rather than stopping quietly at the end of the buffer.
///
/// # Errors
/// [`ProbeError::Mismatch`] where no sentinel arrives.
fn skip_parameter_list(r: &mut Reader<'_>, path: &str) -> Result<(), ProbeError> {
    while r.remaining() >= 4 {
        let header = wire(r.read_pid(), path)?;
        if header.end {
            return Ok(());
        }
        let at = r.position();
        skip_to(r, at.saturating_add(header.length));
        wire(r.finish_pid(header), path)?;
    }
    Err(ProbeError::Mismatch {
        path: path.to_string(),
        expected: "PID_LIST_END closing the union's parameter list".to_string(),
        found: format!("the stream ended at offset {}", r.position()),
    })
}

/// What the stream says about the member this union's discriminator
/// selects.
#[derive(Debug)]
enum UnionMember {
    /// A `PID_LIST_END` stood where the member's parameter would be, and
    /// has been read.
    AbsentAtSentinel,
    /// The delimiter's extent was used up before the member's EMHEADER.
    AbsentAtDelimiter,
    /// The member's parameter header, already read.
    Parameter(zerodds_wire::reader::PidRead),
    /// The member is framed as the plan says and nothing has been read.
    Framed,
}

/// Read as far as it takes to tell whether the member is there.
///
/// Only ever answered from the framing, never from a guess. A `@final`
/// union has neither a delimiter nor a sentinel, so nothing here fires —
/// and Table 19 requires two final unions to have an identical set of
/// labels anyway, which makes a stream missing the member a type
/// mismatch rather than an evolution.
fn union_member_frame(
    r: &mut Reader<'_>,
    header: MemberHeader,
    member_id: u32,
    id_check: MemberIdCheck,
    end: Option<usize>,
    path: &str,
) -> Result<UnionMember, ProbeError> {
    match header {
        // Rule (28): the list runs to its sentinel, and a sentinel where
        // the member's parameter would be is the writer saying it wrote
        // none. The header is consumed either way, because a reader that
        // could put one back would need a mark this crate does not
        // have — and the two outcomes each use what was read.
        MemberHeader::PidCdr1 { .. } => {
            let parameter = wire(r.read_pid(), path)?;
            if parameter.end {
                return Ok(UnionMember::AbsentAtSentinel);
            }
            if id_check.is_exact() && parameter.pid != member_id {
                return Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: format!("a parameter header for member id {member_id}"),
                    found: format!("member id {} at offset {}", parameter.pid, r.position()),
                });
            }
            Ok(UnionMember::Parameter(parameter))
        }
        // Rule (27) under a DHEADER: the extent is what says the union is
        // over.
        MemberHeader::EmHeader { .. } if end.is_some_and(|end| r.position() >= end) => {
            Ok(UnionMember::AbsentAtDelimiter)
        }
        _ => Ok(UnionMember::Framed),
    }
}

/// The integer a discriminator selects a case by.
///
/// IDL 4.2 §7.4.1.4.4.4.2 allows an integer, a `char`, a `boolean` and
/// an enumeration; all four reduce to an integer, and for the
/// enumeration it is the enumerator's value rather than its ordinal.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one;
/// a chain that returns to itself never reaches here, because every caller
/// has planned the type first and `zerodds_ast_layout::plan` refuses a
/// graph deeper than its own `MAX_DEPTH`.
pub(crate) fn selector_of<V: Source>(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<i128, ProbeError> {
    let node = ast
        .type_(discriminator)
        .map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
    match &node.kind {
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            selector_of(ast, target, value, path)
        }
        TypeKind::Enumeration { .. } => {
            let name = value.as_text().ok_or_else(|| ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an enumerator name".to_string(),
                found: "something else".to_string(),
            })?;
            for id in ast.enumerators_of(discriminator) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                if enumerator.name == name {
                    if let Some(number) = ast.effective_enumerator_value(*id) {
                        return Ok(number);
                    }
                }
            }
            Err(ProbeError::Unknown {
                path: path.to_string(),
                declared: Vec::new(),
            })
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            let flag = value.as_bool().ok_or_else(|| ProbeError::Mismatch {
                path: path.to_string(),
                expected: "true or false".to_string(),
                found: "something else".to_string(),
            })?;
            Ok(i128::from(flag))
        }
        // A bitmask discriminator, which DDS-XTypes 1.3 §7.5.2.6 admits
        // in XML (`<discriminator type="nonBasic" nonBasicTypeName=
        // "B_32"/>`) and IDL 4.2 rule (50) does not admit at all. Its
        // value is the set of bit names `read_bitmask` handed back, and
        // the label it is compared against is an integer, so the set is
        // packed back into the anchor the same way `write_bitmask` packs
        // it. Every position is below the bound — `bit_names` refuses
        // one that is not, per §7.4.1.1.3.2 — so the shift is in range
        // and the result is non-negative in `i128`, which keeps an
        // unsigned label of a 64-bit bitmask comparable.
        TypeKind::Bitmask { .. } => {
            let mut bits: u64 = 0;
            for (_, position) in crate::contract::bit_names(ast, discriminator, value, path)? {
                bits |= 1u64 << position;
            }
            Ok(i128::from(bits))
        }
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            let text = value.as_text().ok_or_else(|| ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a one-character string".to_string(),
                found: "something else".to_string(),
            })?;
            match text.chars().next() {
                Some(ch) => Ok(i128::from(u32::from(ch))),
                None => Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: "exactly one character".to_string(),
                    found: "an empty string".to_string(),
                }),
            }
        }
        _ => integer_of(value, path),
    }
}

/// The case the discriminator selects, or `default:`, or nothing.
///
/// Nothing is a legal outcome: a union with no `default:` and a
/// discriminator matching no label carries the discriminator and no
/// member. `union-default-unselected` in the corpus is that sample.
///
/// # Errors
/// [`contract::UnevaluatedLabel`] for a label the model cannot
/// evaluate. A skipped label matches nothing, so the reader would fall
/// to `default:` or to no member at all and hand back a branch the
/// stream does not carry — the shape the negative-label guard below
/// already names, arrived at by a different route.
///
/// Spelled out rather than naming the guard's identifier:
/// `union_label_match.rs::the_sites_go_through_the_crossing` counts the
/// occurrences of that identifier per file to check that every
/// selection site takes the axis, and a mention in prose would inflate
/// the count and let a removed *use* hide behind a comment.
pub(crate) fn select_case(
    ast: &Ast,
    type_: Id<TypeNode>,
    selector: i128,
    suppress_negative: bool,
) -> Result<Option<crate::contract::Case>, crate::contract::UnevaluatedLabel> {
    let mut fallback = None;
    for id in ast.cases_of(type_) {
        let Ok(node) = ast.case(*id) else { continue };
        let Ok(member) = ast.member(node.member) else {
            continue;
        };
        let Some(member_type) = member.type_.resolved() else {
            continue;
        };
        let case = crate::contract::Case {
            node: member.node_id,
            name: member.name.clone(),
            type_: member_type,
            try_construct: member.facets.xtypes.as_ref().and_then(|x| x.try_construct),
            labels: node.labels.clone(),
        };
        for (index, label) in node.labels.iter().enumerate() {
            match label.expr() {
                Some(expr) => {
                    // Reported and not skipped, for the reason the
                    // negative-label guard below gives: a label
                    // that is passed over matches nothing, and the read
                    // hands back `default:` or no member at all while
                    // the stream carries this branch's octets. The
                    // writer refuses the same construct in
                    // `contract::union_selection`.
                    let value = ast
                        .eval_integral(expr)
                        .map_err(|_| crate::contract::UnevaluatedLabel { case: index })?;
                    // The setting reaches the reader as well as the
                    // writer. A reader that matched a label its own
                    // writer suppresses would hand back the branch the
                    // stream does not carry, which is the shape this
                    // project calls a silent wrong value.
                    if suppress_negative && value < 0 {
                        continue;
                    }
                    if value == selector {
                        return Ok(Some(case));
                    }
                }
                None => fallback = Some(case.clone()),
            }
        }
    }
    Ok(fallback)
}

/// An enumeration, read as the integer its bit bound calls for.
///
/// Matched on the bit pattern and not on the value, because the pattern
/// is what a holder narrower than the value's range leaves: Cyclone
/// takes `@value(200)` under `@bit_bound(8)` and writes `c8`, which read
/// as a signed `Int8` is `-56`. Both readings name the same enumerator,
/// and only the pattern says so. `vendor_bytes.md`, `enum_bb8_200`.
fn read_enumeration<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    // The reader takes the same axis the writer took: this project's
    // rule is that a reader is given the settings rather than left to
    // guess the form. `zerodds_ast_layout::enumeration_bit_bound`.
    let bits = enumeration_bit_bound(ast, type_, frame.key.enum_holder);
    let pattern = match bits {
        0 => {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an @bit_bound between 1 and 32".to_string(),
                found: "0".to_string(),
            });
        }
        1..=8 => u32::from(wire(r.read_u8(), path)?),
        9..=16 => u32::from(wire(r.read_u16(), path)?),
        17..=32 => wire(r.read_u32(), path)?,
        other => {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an @bit_bound between 1 and 32, per §7.3.1.2.1.5".to_string(),
                found: format!("{other}"),
            });
        }
    };

    let mut declared = Vec::new();
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        declared.push(node.name.clone());
        // The reader takes the same axis the writer took, through the
        // same crossing: a peer that numbers its enumerators by position
        // puts the position on the wire, and a reader comparing declared
        // values would name a different enumerator or none.
        // `zerodds_ast_layout::enumerator_wire_value`.
        let Some(value) = enumerator_wire_value(ast, *id, frame.key.enumerator_value) else {
            continue;
        };
        if enumerator_pattern(value, bits) == Some(pattern) {
            return Ok(V::enumerator(node.name.clone()));
        }
    }
    // A pattern no enumerator has cannot construct an object of this
    // type — Table 18: "If the name or value of the T2 object does not
    // exist in T1, the object cannot construct any object of type T1" —
    // so it is the site's `try_construct` that decides, not this
    // function. Handing the number back was never an option: the
    // canonical form of an enumeration is a name and there is no name
    // for this.
    //
    // TRIM has no meaning on an enumeration (Table 11), and the frontend
    // refuses to write it there, so `narrowed` reports it rather than
    // inventing a nearest enumerator.
    let found = format!("{pattern:#x}, which names no enumerator; the type declares {declared:?}");
    narrowed(
        frame.try_construct,
        None,
        || frame.default_value(ast, type_, path),
        path,
        "an enumerator this type declares",
        &found,
    )
}

/// The bit pattern an enumerator's value has in its holder.
///
/// The same rule `encode.rs` writes by, and deliberately a second copy
/// of it rather than a call into the writer: the two directions are
/// compared against each other, and a shared helper would make them
/// agree about a mistake.
fn enumerator_pattern(value: i128, bits: u16) -> Option<u32> {
    if bits == 0 || bits > 32 {
        return None;
    }
    let width = i128::from(bits);
    if value < -(1i128 << (width - 1)) || value > (1i128 << width) - 1 {
        return None;
    }
    let mask = (1u128 << width) - 1;
    #[allow(clippy::cast_sign_loss)]
    u32::try_from((value as u128) & mask).ok()
}

/// A bitmask, read as the set bits' names in declaration order.
fn read_bitmask<V: Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    refuse_undeclared: bool,
    path: &str,
) -> Result<V, ProbeError> {
    let bound = ast.effective_bit_bound(type_);
    let bits = match bound {
        1..=8 => u64::from(wire(r.read_u8(), path)?),
        9..=16 => u64::from(wire(r.read_u16(), path)?),
        17..=32 => u64::from(wire(r.read_u32(), path)?),
        33..=64 => wire(r.read_u64(), path)?,
        other => {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an @bit_bound between 1 and 64, as Table 33 tabulates".to_string(),
                found: format!("{other}"),
            });
        }
    };

    let mut names = Vec::new();
    let mut declared: u64 = 0;
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        let Some(position) = ast.effective_bit_position(*id) else {
            continue;
        };
        if position >= bound {
            continue;
        }
        let mask = 1u64 << position;
        declared |= mask;
        if bits & mask != 0 {
            names.push(node.name.clone());
        }
    }
    // A union is selected by its discriminator, and a discriminator with a
    // bit left out is another value: it would select another case, or
    // none, with nothing saying so. The bit is named, with the type.
    if refuse_undeclared {
        let width_mask = if bound >= 64 {
            u64::MAX
        } else {
            (1u64 << bound) - 1
        };
        let undeclared = bits & width_mask & !declared;
        if undeclared != 0 {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: format!(
                    "only the bits `{}` declares as a union discriminator",
                    ast.type_(type_)
                        .map_or("<bitmask>", |node| node.name.as_str())
                ),
                found: format!(
                    "bit {} set, which no flag of the bitmask names",
                    undeclared.trailing_zeros()
                ),
            });
        }
    }
    // A bit the type does not name is not read back. §7.4.1.1.3.2: "the
    // states of the remaining serialized bits are not specified, and
    // those bits are not considered to be part of the bitmask", and
    // Table 18: "Preserve bit values by index for all bits identified in
    // both T1 and T2". A writer's bit that this type has no name for is
    // a bit the types do not share.
    Ok(V::bit_names(names))
}

/// A bitset, unpacked out of the anchor the planner sized.
///
/// An inherited bitset reads the whole chain, base-first, because that
/// is where the writer put it; see `write_bitset` for the measurement.
fn read_bitset<V: Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    let fields = ast.all_bitfields_of(type_);
    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let Extent::Sized(anchor) = layout.extent else {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitset whose anchor the planner does not size",
        });
    };
    let bits = match anchor {
        1 => u64::from(wire(r.read_u8(), path)?),
        2 => u64::from(wire(r.read_u16(), path)?),
        4 => u64::from(wire(r.read_u32(), path)?),
        8 => wire(r.read_u64(), path)?,
        other => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: match other {
                    0 => "a bitset with no anchor",
                    _ => "a bitset whose anchor is not one, two, four or eight bytes",
                },
            });
        }
    };

    let mut out = Vec::new();
    let mut position: u32 = 0;
    for id in &fields {
        let Ok(field) = ast.bitfield(*id) else {
            continue;
        };
        let width = u32::from(field.bits);
        if position.saturating_add(width) > 64 {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a bitset wider than the sixty-four bits an anchor holds",
            });
        }
        if let Some(name) = field.name.as_ref() {
            let mask = if width >= 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            let raw = (bits >> position) & mask;
            out.push((name.clone(), i128::from(raw)));
        }
        position += width;
    }
    V::bitfields(out, path)
}

/// A map: the entries, in the shape §7.4.1.1.4 says a map has.
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`. The
/// entries go back to `read_type`, whose aggregate arms count and refuse.
#[allow(clippy::too_many_arguments)]
fn read_map<V: Source + Sink>(
    r: &mut Reader<'_>,
    ast: &Ast,
    type_: Id<TypeNode>,
    key_type: &Ref<TypeNode>,
    value_type: &Ref<TypeNode>,
    bound: &Bound,
    frame: Frame,
    path: &str,
) -> Result<V, ProbeError> {
    let key_type = key_type.resolved().ok_or_else(|| ProbeError::NoLayout {
        detail: format!("{path}: the map key type is unresolved"),
    })?;
    let value_type = value_type.resolved().ok_or_else(|| ProbeError::NoLayout {
        detail: format!("{path}: the map value type is unresolved"),
    })?;

    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let end = if layout.needs_dheader {
        let length = wire(r.read_dheader(), path)?;
        Some(r.position().saturating_add(length))
    } else {
        None
    };
    // Under `Rule15Omits` a delimited map states no count, so the extent
    // is the only thing that says where the entries stop. Under the
    // default the count is there whether or not a delimiter is.
    let counted = end.is_none() || frame.map_length == MapLengthPolicy::SequenceEquivalent;
    let declared = if counted {
        Some(wire(r.read_u32(), path)? as usize)
    } else {
        None
    };

    let mut entries = Vec::new();
    let mut index = 0usize;
    loop {
        match (declared, end) {
            (Some(count), _) if index >= count => break,
            (None, Some(end)) if r.position() >= end => break,
            (None, None) => break,
            _ => {}
        }
        let entry_path = format!("{path}[{index}]");
        // The key's behaviour and the value's are separate slots, and
        // Table 17 gives them different powers: a key that cannot be
        // constructed "fails the whole map" with no escape, while a
        // value takes the value slot's behaviour. `idl-lower` refuses
        // anything but DISCARD on a key, so `key_try_construct` is the
        // default here and the query is what says so.
        let key = read_type(
            r,
            ast,
            key_type,
            frame.resolved(ast.key_try_construct(type_)).in_collection(),
            &format!("{entry_path}.key"),
        )?;
        let value = read_type(
            r,
            ast,
            value_type,
            frame
                .resolved(ast.element_try_construct(type_, None))
                .in_collection(),
            &format!("{entry_path}.value"),
        )?;
        entries.push(V::map_entry(key, value));
        index += 1;
    }
    // The entries are all read before the bound is applied, for the
    // reason a sequence's are: stopping early would leave the rest in
    // the stream. The delimiter is then skipped to whether or not this
    // map fits, so a discarded map still leaves the reader where the
    // next member starts — the caller may be reading it under
    // USE_DEFAULT.
    let limit = bound_value(ast, bound);
    let over_bound = limit.is_some_and(|limit| entries.len() as i128 > limit);
    if let Some(end) = end {
        skip_to(r, end);
    }
    if over_bound {
        // Sorted first: a trim keeps "the order of the ... elements in
        // the sequence or map", and a map's order is the canonical one
        // this probe puts it in, not the wire's.
        entries.sort_by_key(canonical::text);
        return fit_collection(ast, entries, limit, type_, frame, path, "entries");
    }
    // Sorted by key, for the reason `canonical::canonicalise` sorts:
    // §7.4.1.1.4 says nothing about entry order, `fastddsgen` maps a
    // `map` to `std::map` and so reorders one, and the canonical form a
    // pairwise run compares in has to be the same on both sides of the
    // comparison or every map case fails on the container choice.
    entries.sort_by_key(canonical::text);
    Ok(V::collection(entries))
}

/// A `wstring`, in the form [`WstringLengthPolicy`] names.
///
/// The default is XTypes 1.3 §7.4.1.1.2 — a byte count, "twice the
/// number of characters in the string", no terminator and no byte-order
/// mark to strip. Two of the three measured implementations write
/// something else, and the policy's own documentation carries the bytes
/// and the argument for reading one form rather than sniffing.
///
/// # Nothing here looks at the length to decide which form it is
///
/// The length is the field a hostile peer controls, and the forms are
/// not separable anyway — a Fast DDS `"ab"` followed by a `long` is
/// byte-identical to a specification `"a"` followed by the same `long`.
/// So the configured form is read, and a length that form cannot have
/// produced is refused with the form it *does* fit named in the error,
/// which is a diagnosis after the refusal rather than a parse decided
/// by the stream.
/// The layout crate's name for a byte-layer position.
///
/// The two enumerations are deliberately separate — `zerodds-wire`
/// knows nothing about a dialect — and this is the one place the walk
/// crosses between them. A `match` and not a cast: a position added to
/// either side has to be given a meaning here rather than silently
/// becoming `Member`, which is precisely how the framing defect
/// survived five emitters.
const fn layout_position(position: WcharPosition) -> WirePosition {
    match position {
        WcharPosition::Member => WirePosition::Member,
        WcharPosition::Element => WirePosition::Element,
        WcharPosition::Sentinel => WirePosition::Sentinel,
        WcharPosition::ExtendedMarker => WirePosition::ExtendedMarker,
    }
}

fn read_wstring(
    r: &mut Reader<'_>,
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
    position: WirePosition,
    path: &str,
) -> Result<String, ProbeError> {
    let declared = wire(r.read_u32(), path)? as usize;
    let width = policy.character_width(position);
    // The three coordinates, not a list of named forms. C1 says whether
    // the count is of units or of the octets they occupy, C3 says what
    // an octet count divides by, and C2 is handled after the run
    // because a terminator occupies a unit on the wire whichever way it
    // is counted.
    let mut units = if matches!(policy.counts_at(position), WstringLengthCounts::Characters) {
        read_wstring_units(r, declared, width, order, path)?
    } else {
        // A count the width does not divide is a form mismatch and not
        // a short read: rounding it down would leave the remainder in
        // the stream and put every later member at the wrong offset.
        if declared % width != 0 {
            return Err(wstring_form_mismatch(policy, declared, path));
        }
        read_wstring_units(r, declared / width, width, order, path)?
    };
    if matches!(policy.terminator_at(position), WstringTerminator::Present) {
        // The count includes the NUL, so a count of zero is a frame
        // that does not even hold its own terminator.
        if declared < 1 {
            return Err(wstring_form_mismatch(policy, declared, path));
        }
        let terminator = units.pop();
        if terminator != Some(0) {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a NUL character ending the wstring, as its count includes one"
                    .to_string(),
                found: format!("{:04x}", terminator.unwrap_or_default()),
            });
        }
    }
    // A NUL inside the text is the narrow string's problem in wide
    // form: every C peer stops at it, so the value this reader returns
    // and the value the peer returns differ for bytes both accepted.
    // Only where a terminator is expected — a cell without one is
    // entitled to carry U+0000 as a character.
    if matches!(policy.terminator_at(position), WstringTerminator::Present) {
        if let Some(offset) = units.iter().position(|unit| *unit == 0) {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "no NUL inside a wstring a C peer would stop at".to_string(),
                found: format!("a NUL at character {offset} of {}", units.len()),
            });
        }
    }

    // The byte-order mark is not a character of the string, and under
    // `ByteOrderMark` it also decides the order of everything behind it
    // — which `read_wstring_units` could not know until it had read it.
    // CORBA §9.3.1.6: `FEFF` in the stream's order means the units
    // follow in the stream's order, `FFFE` means they are the other way
    // round, and a `wstring` with no mark at all is big-endian. That
    // last rule is what makes this variant read a JacORB peer as well as
    // an omniORB one.
    if order.has_mark() {
        strip_byte_order_mark(&mut units);
    }

    // An unpaired surrogate is refused rather than replaced: U+FFFD in
    // its place would be a character the sender never had.
    //
    // It catches less than it looks. Fast CDR's truncation of `U+1D11E`
    // is `U+D11E`, a Hangul syllable and well-formed UTF-16, so the
    // wrong character arrives with nothing to object to. Only a code
    // point whose low sixteen bits land in `D800..=DFFF` fails here.
    String::from_utf16(&units).map_err(|_| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "well-formed UTF-16".to_string(),
        found: "an unpaired surrogate".to_string(),
    })
}

/// A `wstring` whose characters arrive as their own UTF-8 octets.
///
/// The mirror of `encode::write_wstring_utf8` and of
/// [`WstringCodeUnits::Utf8`]: the declared length is the UTF-8 octet
/// count, and the terminator, when the cell carries one, is the last of
/// those octets. A count the stream cannot hold is refused before a byte
/// is read, for the reason `read_wstring` gives.
fn read_wstring_utf8(
    r: &mut Reader<'_>,
    policy: WstringLengthPolicy,
    position: WirePosition,
    path: &str,
) -> Result<String, ProbeError> {
    let declared = wire(r.read_u32(), path)? as usize;
    if declared > r.remaining() {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "a wstring length the stream can hold".to_string(),
            found: format!("{declared} byte(s) with {} left", r.remaining()),
        });
    }
    let mut bytes = Vec::with_capacity(declared);
    for _ in 0..declared {
        bytes.push(wire(r.read_u8(), path)?);
    }
    if matches!(policy.terminator_at(position), WstringTerminator::Present) {
        if bytes.pop() != Some(0) {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a NUL octet ending the wstring, as its count includes one".to_string(),
                found: "a byte that is not zero".to_string(),
            });
        }
        if bytes.contains(&0) {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "no NUL inside a wstring a C peer would stop at".to_string(),
                found: "a NUL before the terminator".to_string(),
            });
        }
    }
    String::from_utf8(bytes).map_err(|_| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "well-formed UTF-8".to_string(),
        found: "an invalid sequence".to_string(),
    })
}

/// `count` characters of `width` bytes each, as UTF-16 code units.
///
/// The four-byte form is RTI's XCDR1 one, where a character is a code
/// unit zero-extended; a value above `0xFFFF` there is not a form this
/// reader knows how to narrow, so it is refused rather than truncated.
fn read_wstring_units(
    r: &mut Reader<'_>,
    count: usize,
    width: usize,
    order: WstringUnitOrder,
    path: &str,
) -> Result<Vec<u16>, ProbeError> {
    let bytes = count.checked_mul(width).unwrap_or(usize::MAX);
    if bytes > r.remaining() {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "a length the stream can hold".to_string(),
            found: format!("{bytes} byte(s) with {} left", r.remaining()),
        });
    }
    let mut units = Vec::with_capacity(count);
    for _ in 0..count {
        if width == 4 {
            // `ByteOrderMark` reads little-endian here and the mark is
            // re-examined once the whole run is in: a mark that says
            // `FFFE` means every unit has to be swapped back, which is
            // `strip_byte_order_mark`'s second job.
            let wide = match order {
                WstringUnitOrder::Stream => wire(r.read_u32(), path)?,
                WstringUnitOrder::BigEndianAlways => wire(r.read_u32_big(), path)?,
                WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped => {
                    wire(r.read_u32_little(), path)?
                }
            };
            let unit = u16::try_from(wide).map_err(|_| ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a four-byte wstring character holding one UTF-16 code unit".to_string(),
                found: format!("{wide:08x}"),
            })?;
            units.push(unit);
        } else {
            units.push(match order {
                WstringUnitOrder::Stream => wire(r.read_u16(), path)?,
                WstringUnitOrder::BigEndianAlways => wire(r.read_u16_big(), path)?,
                WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped => {
                    wire(r.read_u16_little(), path)?
                }
            });
        }
    }
    Ok(units)
}

/// The UTF-16 byte-order mark, `U+FEFF`, and its byte-swapped form.
const BYTE_ORDER_MARK: u16 = 0xFEFF;
/// `U+FFFE`, which is not a character: a mark read this way says the
/// units behind it are the other way round.
const SWAPPED_BYTE_ORDER_MARK: u16 = 0xFFFE;

/// Take the byte-order mark off the front and apply what it said.
///
/// The units arrived little-endian, because that is what omniORB writes
/// on this host and what [`WstringUnitOrder::ByteOrderMark`] therefore
/// reads. Three cases, and the third is the one that matters:
///
/// - `FEFF` — the mark is the right way up, the units are as read.
/// - `FFFE` — the mark is byte-swapped, so every unit behind it is too,
///   and each one is swapped back. This is what a conforming big-endian
///   omniORB peer produces and what a reader that ignored the mark would
///   return as different characters, well formed and the right length.
/// - no mark — CORBA's rule for a GIOP 1.2 `wstring` without one is
///   UTF-16 big-endian, so every unit read little-endian is swapped.
///
/// The mark is never a character of the returned string in any of the
/// three, which is the half §7.4.1.1.2 cares about.
///
/// The stream's own byte order is not an argument and does not need to
/// be: reading the units little-endian and then obeying the mark is
/// correct in all four combinations of writer host and stream order,
/// because the mark is written in the same order as the content it
/// introduces. That is what a byte-order mark is for.
fn strip_byte_order_mark(units: &mut Vec<u16>) {
    match units.first().copied() {
        Some(BYTE_ORDER_MARK) => {
            units.remove(0);
        }
        Some(SWAPPED_BYTE_ORDER_MARK) => {
            units.remove(0);
            for unit in units.iter_mut() {
                *unit = unit.swap_bytes();
            }
        }
        _ => {
            for unit in units.iter_mut() {
                *unit = unit.swap_bytes();
            }
        }
    }
}

/// A length the configured form cannot have produced.
///
/// The error names the form the length does fit, and the setting that
/// selects it. Reading it that way is not offered — the reader has
/// already refused — but a peer misconfiguration that reports "an odd
/// byte count" and nothing else costs an afternoon to place.
fn wstring_form_mismatch(policy: WstringLengthPolicy, declared: usize, path: &str) -> ProbeError {
    // Sweeps the whole product and not the measured five: a cell left
    // out of the sweep is a cell this diagnostic can never name, and
    // naming the cell the writer used is the whole point of it.
    let candidates: Vec<&str> = WstringLengthPolicy::ALL_CELLS
        .into_iter()
        .filter(|other| *other != policy && wstring_length_is_possible(*other, declared))
        .map(WstringLengthPolicy::as_str)
        .collect();
    let fits = if candidates.is_empty() {
        String::from("no other wstring length policy either")
    } else {
        format!("--wstring-length {}", candidates.join(" or "))
    };
    ProbeError::Mismatch {
        path: path.to_string(),
        expected: format!(
            "a wstring length the {} policy can produce",
            policy.as_str()
        ),
        found: format!("{declared}, which fits {fits}"),
    }
}

/// Whether `declared` is a length `policy` could have written.
///
/// Only the shape of the number, which is all a refused read has to go
/// on: an odd count is not a byte count, and a zero count cannot
/// include a terminator.
const fn wstring_length_is_possible(policy: WstringLengthPolicy, declared: usize) -> bool {
    // The two rules the coordinates give, and no list of named forms:
    // an octet count has to divide by the unit width, and a count that
    // includes a terminator cannot be zero. A cell nobody has measured
    // is checked by the same two.
    //
    // A byte count and `HostUnswapped`'s byte count are the same number
    // — that point of C4 differs in the *content*'s byte order, which
    // no length can show — so a diagnostic that names one names both.
    // That is a property of C4 now and not a special case here.
    if matches!(policy.counts, WstringLengthCounts::Octets)
        && declared % policy.unit.octets() as usize != 0
    {
        return false;
    }
    if matches!(policy.terminator, WstringTerminator::Present) && declared < 1 {
        return false;
    }
    true
}

/// `fixed<P, S>`: packed BCD, [CDR] §9.3.2.8.
///
/// Every rule about the field — the declaration's bound, how its end is
/// found under each digit count, which nibble is a sign, where the point
/// sits, and the one spelling a sample comes back as — is in
/// `zerodds_wire::Reader::read_fixed`, which is where the twenty-first
/// language column reaches it too. What stays here is the path: `Reader`
/// says what it expected and at which byte, and a refusal over a
/// fifty-member document that cannot say which member it was is one nobody
/// can act on.
fn read_fixed<V: Sink>(
    r: &mut Reader<'_>,
    digits: u8,
    scale: u8,
    policy: FixedDigitCount,
    path: &str,
) -> Result<V, ProbeError> {
    let text = r
        .read_fixed(digits, scale, crate::encode::fixed_digits(policy))
        .map_err(|error| ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("a fixed<{digits}, {scale}> the stream can carry"),
            found: format!("{error}"),
        })?;
    Ok(V::fixed(text))
}

fn read_primitive<V: Sink>(
    r: &mut Reader<'_>,
    kind: PrimitiveKind,
    position: WcharPosition,
    path: &str,
) -> Result<V, ProbeError> {
    match kind {
        PrimitiveKind::Boolean => Ok(V::boolean(wire(r.read_bool(), path)?)),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => {
            V::integer(kind, i128::from(wire(r.read_u8(), path)?), path)
        }
        PrimitiveKind::Int8 => V::integer(kind, i128::from(wire(r.read_i8(), path)?), path),
        PrimitiveKind::Char => Ok(V::character(kind, wire(r.read_char(), path)?)),
        PrimitiveKind::WChar => Ok(V::character(kind, wire(r.read_wchar(position), path)?)),
        PrimitiveKind::Int16 => V::integer(kind, i128::from(wire(r.read_i16(), path)?), path),
        PrimitiveKind::UInt16 => V::integer(kind, i128::from(wire(r.read_u16(), path)?), path),
        PrimitiveKind::Int32 => V::integer(kind, i128::from(wire(r.read_i32(), path)?), path),
        PrimitiveKind::UInt32 => V::integer(kind, i128::from(wire(r.read_u32(), path)?), path),
        // Which spelling a sixty-four-bit width takes is the
        // representation's, and it is told which width it has: a JSON
        // number is a double and loses the low bits past 2^53.
        PrimitiveKind::Int64 => V::integer(kind, i128::from(wire(r.read_i64(), path)?), path),
        PrimitiveKind::UInt64 => V::integer(kind, i128::from(wire(r.read_u64(), path)?), path),
        PrimitiveKind::Float32 => V::float(kind, f64::from(wire(r.read_f32(), path)?), path),
        PrimitiveKind::Float64 => V::float(kind, wire(r.read_f64(), path)?, path),
        // A string, like the 64-bit widths above and for the same
        // reason: a JSON number is a double, and 60 of a long double's
        // 113 significand bits do not fit one.
        //
        // The *exact* expansion and not the shortest one. Every binary
        // value has finitely many decimal digits and every column can
        // print them with nothing but long multiplication; the shortest
        // form needs a parser and a search, and four columns that each
        // implement that search will disagree about which of two equally
        // short spellings to emit long before they disagree about the
        // value. `crates/float128` still has `to_shortest_decimal` for
        // callers that want it — this is the canonical form, not the
        // readable one.
        PrimitiveKind::Float128 => Ok(V::float128(wire(r.read_f128(), path)?)),
    }
}

/// Move the cursor to `target`, which is never backwards.
///
/// A framing word that pointed behind the cursor would mean the reader
/// had already read past the end of what it framed, and that is caught
/// where the word is read rather than papered over by seeking back.
fn skip_to(r: &mut Reader<'_>, target: usize) {
    while r.position() < target && r.remaining() > 0 {
        // `read_u8` is the one read that never aligns, so it advances by
        // exactly one whatever the offset is.
        if r.read_u8().is_err() {
            return;
        }
    }
}

fn bound_value(ast: &Ast, bound: &Bound) -> Option<i128> {
    match bound.expr() {
        Some(expr) => ast.eval_integral(expr).ok(),
        None => None,
    }
}

/// The integer a **stream-side** value carries.
///
/// The same [`Source::as_integer`] a value file goes through, and it can
/// be, because the value here is one this crate has just built: a
/// discriminator is an integer, a `char`, a `boolean` or an enumeration
/// (§7.4.1.4.4.4.2), the first three are handled above and the fourth
/// is read out of a field no wider than sixty-four bits — which
/// [`Sink::integer`] has already spelled in whatever form the
/// representation reserves for that width. So no spelling reaches here
/// that the representation would refuse.
///
/// It used to be a second reading with the range rule left out, on the
/// argument that the spelling was this project's own. That argument is
/// what makes the shared one safe; it is not a reason to have two.
fn integer_of<V: Source>(value: &V, path: &str) -> Result<i128, ProbeError> {
    value.as_integer(path)
}

/// A wire error, with the path the value walk is at put back on it.
///
/// `WireError` names the offset, which is what a reader knows; the path
/// is what a caller reading a fifty-member sample needs.
fn wire<T>(result: Result<T, WireError>, path: &str) -> Result<T, ProbeError> {
    result.map_err(|error| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "a readable value".to_string(),
        found: format!("{error}"),
    })
}

#[cfg(test)]
mod padding_tests {
    use super::padding_present;

    /// XTypes 1.3 §7.6.3.1.2: the count is honoured where the octets make
    /// the payload a multiple of four, and ignored where they cannot be
    /// there — OpenDDS 3.34's 70-octet payload counting two. A count the
    /// body cannot hold comes back whole, for the caller to refuse.
    #[test]
    fn padding_is_counted_only_where_it_can_be_present() {
        assert_eq!(padding_present(8, 0x02), 2);
        assert_eq!(padding_present(68, 0x02), 2);
        assert_eq!(padding_present(70, 0x02), 0);
        assert_eq!(padding_present(9, 0x03), 0);
        assert_eq!(padding_present(8, 0x00), 0);
        assert_eq!(padding_present(4, 0x01), 1);
        assert_eq!(padding_present(5, 0x03), 3);
    }
}
