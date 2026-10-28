// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One value, one type, the bytes between them.
//!
//! The walk follows the layout plan, not a second reading of the
//! model: whether a structure carries a delimiter, what header each
//! member takes and which length code it uses are all the plan's
//! decisions. A probe that decided them again would agree with the
//! plan only until one of the two changed.

use crate::contract::{self, Consumed};
use crate::source::{Fields, Source};
use crate::value::{
    Emit, MapLengthPolicy, ProbeRequest, encapsulation_header_in, encapsulation_padding,
    wire_wchar_framing_of, wire_wchar_width_of, wire_wstring_unit_order_of,
};
use std::string::String;
use std::vec::Vec;
use zerodds_ast::{Ast, Bound, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref, ScopedName};
use zerodds_ast_layout::{
    DepCollector, ElementPadding, Endianness, EnumeratorValue, Extent, FixedDigitCount,
    KeyHashForm, LayoutKey, LengthCode, MapKeyOrdering, MemberHeader, Purpose, TypeLayout,
    WirePosition, WstringCodeUnits, WstringLengthCounts, WstringLengthPolicy, WstringTerminator,
    WstringUnitOrder, WstringUnitWidth, array_is_delimited, collection_is_delimited,
    enumeration_bit_bound, enumerator_wire_value, key_members_with, map_key_ordering, member_key,
    plan, union_negative_matches_nothing,
};
use zerodds_wire::{ParameterRole, WcharPosition, Writer};

/// Why a probe could not produce bytes.
///
/// Every variant names where in the value the trouble is, because a
/// probe that said only "type mismatch" over a fifty-member sample is
/// a probe nobody uses twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    /// The requested type is not in the model.
    NoSuchType {
        /// The scoped name asked for.
        name: String,
    },
    /// The layout could not be planned.
    NoLayout {
        /// What the planner said.
        detail: String,
    },
    /// The value at `path` is not what the type calls for.
    Mismatch {
        /// Where, as a dotted path from the sample's root.
        path: String,
        /// What the type wanted.
        expected: String,
        /// What the value held.
        found: String,
    },
    /// A member the type declares has no value and is not optional.
    Missing {
        /// Where.
        path: String,
    },
    /// The stream is a well-formed sample of a compatible type, and no
    /// object of *this* type can be constructed from it.
    ///
    /// The `try_construct` outcome of DDS-XTypes 1.3 §7.2.2.7 —
    /// a `string<32>` holding twelve characters arriving where the
    /// receiver declares `string<5>`, a `sequence<long,32>` of eight
    /// arriving at a bound of four, an enumerator this type does not
    /// declare — where the governing behaviour is `DISCARD`.
    ///
    /// # Why this is not one of the other variants
    ///
    /// §7.2.4.1: "the default behavior is to discard the O2 object to
    /// avoid misinterpretation", and §7.2.3: the sample "can be safely
    /// ignored (filtered out) by the receiver". A discarded sample is an
    /// **absence**, not an error: a DDS subscriber's `take()` returns
    /// nothing and no listener fires. Measured — in Cyclone the
    /// normalize failure makes `ddsi_serdata_from_ser` return `NULL`,
    /// `remote_make_sample` passes the `NULL` on and delivery ends at
    /// `remote_on_delivery_failure_fastpath`, so the sample never
    /// becomes one.
    ///
    /// [`Self::Mismatch`] and the framing errors underneath it say the
    /// *stream* is wrong and the peer needs fixing. This says one sample
    /// does not fit a type that is otherwise perfectly compatible, and
    /// the next sample may well fit. Keeping them apart is the whole
    /// point: a subscriber that logged every narrowed sample as a
    /// protocol error would drown, and one that treated a malformed
    /// stream as a narrowing would never notice a broken peer.
    Discarded {
        /// Where, as a dotted path from the sample's root.
        path: String,
        /// What this type can hold.
        expected: String,
        /// What arrived.
        found: String,
    },
    /// The value holds a key the type does not declare.
    ///
    /// An error and not a warning: a misspelled member name would
    /// otherwise silently test the default value.
    Unknown {
        /// Where.
        path: String,
        /// The keys the type does declare, so the typo is visible.
        declared: Vec<String>,
    },
    /// A construct this target has no form for.
    ///
    /// `kind` carries the whole reason, in the target's terms — "a
    /// bitmask, which is XTypes 1.3 §7.3.1.2.1.4 and not CORBA IDL",
    /// "a map, which the IDL-to-C mapping §7.5.1 reuses does not
    /// have". The rendering used to append "is not serialised by this
    /// probe yet" to every one of them, which turned a limit of CORBA
    /// or of a vendor's front end into a backlog item of ours. It was
    /// read that way: a review of why nine toolchains produced nothing
    /// for so many corpus cases classified all 127 of TAO's as ours on
    /// the strength of that suffix, when every one of them names a
    /// construct CORBA IDL does not have.
    ///
    /// Where the limit really is this probe's, [`Self::Unfilled`] says
    /// so in its own words.
    Unsupported {
        /// Where.
        path: String,
        /// Which construct, and why this target has no form for it.
        kind: &'static str,
    },
    /// The target took the construct and its own generated code does
    /// not build.
    ///
    /// A measurement of the toolchain and not of its specification,
    /// which is why it is not [`Self::Unsupported`]: `rosidl`'s IDL
    /// takes `wchar[3]` without a word, and what
    /// `rosidl_typesupport_fastrtps_c` 2.2.2 emits calls an undeclared
    /// `callbacks`. Recorded as a missing form, it said ROS has no
    /// collection of wchar — the opposite of what was measured.
    GeneratorDefect {
        /// Where.
        path: String,
        /// What was measured, including the emitted line.
        kind: &'static str,
    },
    /// The target carries the construct and a documented mapping rule
    /// makes the octets differ.
    ///
    /// Also a measurement, and also not a missing form. `omniidl` and
    /// `tao_idl` both take `long double`; CORBA's C++ mapping makes it
    /// the platform's x87 extended precision and not §9.3.1.3's
    /// binary128.
    MappingDivergence {
        /// Where.
        path: String,
        /// The rule, with its clause.
        kind: &'static str,
    },
    /// A construct this probe has not implemented.
    ///
    /// Separate from [`Self::Unsupported`], which is a limit of the
    /// target. Twenty-one sites reach a catch-all arm and have only
    /// the construct's own name to give — `union`, `map` — and a bare
    /// noun as a whole message says neither what is wrong nor whose
    /// gap it is. This variant supplies the sentence around it.
    Unfilled {
        /// Where.
        path: String,
        /// The construct's name, as the model spells it.
        kind: &'static str,
    },
    /// The value nests deeper than [`crate::MAX_VALUE_DEPTH`]
    /// aggregates.
    ///
    /// A recursive type has finite values because each one terminates
    /// itself — a sequence of length zero, an absent `@optional`
    /// member. A value whose object graph has a cycle, `a.next = a`,
    /// terminates nowhere, and XTypes 1.3 §7.3.1.2.1.4 says of it "if
    /// the graph of objects has cycles, it is not serializable".
    ///
    /// The walk cannot see the cycle — it is handed one member, or one
    /// header, at a time — so it counts, and says this. Not a silent
    /// truncation and not a stack overflow: the caller is told which
    /// value, how deep it went and what the limit is.
    ValueTooDeep {
        /// Where, as a dotted path from the sample's root.
        path: String,
        /// The limit that was passed, [`crate::MAX_VALUE_DEPTH`].
        limit: usize,
    },
    /// A KeyHash was asked for on a type that declares no `@key`.
    ///
    /// Not "unsupported" and not a zero hash: a keyless topic has one
    /// instance, so there is no handle to compute, and Cyclone, RTI and
    /// Fast DDS all decide that before any hash is. See `key_hash`.
    NoKey {
        /// The type asked about.
        type_name: String,
    },
}

impl core::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoSuchType { name } => write!(f, "`{name}` is not in the model"),
            Self::NoLayout { detail } => write!(f, "the layout could not be planned: {detail}"),
            Self::Mismatch {
                path,
                expected,
                found,
            } => write!(f, "{path}: expected {expected}, found {found}"),
            Self::Missing { path } => write!(f, "{path}: no value, and the member is not optional"),
            Self::Discarded {
                path,
                expected,
                found,
            } => write!(
                f,
                "the sample is discarded: at {path} this type holds {expected} and the \
                 sample carries {found}, and `@try_construct` there is DISCARD"
            ),
            Self::Unknown { path, declared } => {
                write!(f, "{path}: not a member; the type declares {declared:?}")
            }
            Self::Unsupported { path, kind } => write!(f, "{path}: {kind}"),
            Self::GeneratorDefect { path, kind } => write!(f, "{path}: {kind}"),
            Self::MappingDivergence { path, kind } => write!(f, "{path}: {kind}"),
            Self::Unfilled { path, kind } => {
                // "an enum", not "a enum": the article is the only word
                // this variant adds and getting it wrong reads as a
                // machine talking.
                let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
                    "an"
                } else {
                    "a"
                };
                write!(
                    f,
                    "{path}: {article} {kind}, which this probe does not carry yet"
                )
            }
            Self::ValueTooDeep { path, limit } => write!(
                f,
                "{path}: the value nests more than {limit} aggregates deep; a value whose object \
                 graph has a cycle is not serializable — XTypes 1.3 §7.3.1.2.1.4"
            ),
            Self::NoKey { type_name } => write!(
                f,
                "`{type_name}` declares no `@key`, so it has one instance and no KeyHash"
            ),
        }
    }
}

impl core::error::Error for ProbeError {}

/// What a walk carries down: the key the plan is computed under, and
/// the one framing question no plan answers.
///
/// The layout key is the planner's own input type, so a walk that
/// consults the plan and a walk that writes the bytes cannot drift
/// apart. The map policy is separate because the contradiction it
/// resolves is in the serialisation clause, not in the layout.
#[derive(Debug, Clone, Copy)]
struct Frame {
    key: LayoutKey,
    map_length: MapLengthPolicy,
    /// Whether the type about to be written is reached as a member or
    /// as a collection element.
    ///
    /// It rides in the frame rather than as an argument of its own for
    /// the reason the recorder rides with the writer: there is one thing
    /// to thread and a new arm of [`write_type`] cannot forget half of
    /// it. Only `wchar` reads it, and only under TAO's framing — see
    /// `zerodds_ast_layout::WcharPosition`.
    wchar: WcharPosition,
    /// How many aggregates of the *value* the walk is inside.
    ///
    /// The type graph's depth is the planner's and is bounded by the
    /// model; this is the value's, and a recursive type puts no bound
    /// on it at all. See [`crate::MAX_VALUE_DEPTH`].
    depth: usize,
}

impl Frame {
    /// The frame an element of a collection is written in.
    fn in_collection(self) -> Self {
        Self {
            wchar: WcharPosition::Element,
            ..self
        }
    }

    /// The frame an aggregate's own members are written in.
    ///
    /// An aggregate resets the position: a `wchar` member of a struct
    /// that is itself an array element is reached through the struct's
    /// marshaller and takes the member's framing.
    ///
    /// It is also the one place the value's depth grows, which is why
    /// the counter rides in the frame rather than beside it: every
    /// descent into an aggregate goes through here, so no arm of the
    /// walk can descend without counting.
    fn in_aggregate(self) -> Self {
        Self {
            wchar: WcharPosition::Member,
            depth: self.depth.saturating_add(1),
            ..self
        }
    }

    /// The refusal for a value that nests past [`crate::MAX_VALUE_DEPTH`],
    /// or `Ok(())` while it does not.
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
}

/// The bytes, and a note of every scalar the walk took out of the value.
///
/// The recorder rides with the writer rather than beside it so that a
/// new arm of `write_type` cannot forget to thread it: there is one
/// mutable thing to pass and it carries both.
struct Sink {
    w: Writer,
    seen: Consumed,
}

/// Serialise `values` as `request.type_name`.
///
/// # Errors
/// [`ProbeError`], always naming the path in the value.
pub fn encode<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
) -> Result<Vec<u8>, ProbeError> {
    encode_recording(ast, request, values, &mut Consumed::new())
}

/// As [`encode`], recording which scalars of the value were read.
///
/// The three probes read one value file, and a file that one of them
/// reads whole and another reads in part is the defect this exists to
/// make visible. `tests/probe_agreement.rs` compares the recordings.
///
/// # Errors
/// As [`encode`].
pub fn encode_recording<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
    seen: &mut Consumed,
) -> Result<Vec<u8>, ProbeError> {
    encode_noting_padding(ast, request, values, seen, false, 0).map(|(bytes, _)| bytes)
}

/// As [`encode`], for a value that continues a sample whose first `start`
/// octets someone else wrote: alignment counts from the start of the
/// sample, and only the value's own octets are returned.
///
/// DDS-RPC 1.0 §7.5.1.1.6 puts the `Call` union behind a header of
/// variable length, and under XCDR1 an 8-octet member of it aligns to the
/// sample. The request must be `Emit::Payload`.
///
/// # Errors
/// As [`encode`]; `ProbeError::Unsupported` for another `emit`.
pub fn encode_continuing<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
    start: usize,
) -> Result<Vec<u8>, ProbeError> {
    if request.emit != Emit::Payload {
        return Err(ProbeError::Unsupported {
            path: request.type_name.clone(),
            kind: "a continuation behind an encapsulation header or as a KeyHash",
        });
    }
    encode_noting_padding(ast, request, values, &mut Consumed::new(), false, start)
        .map(|(bytes, _)| bytes)
}

/// As [`encode`], and the offsets of the octets that carry no value.
///
/// The second half of the answer is computed by the **layout**, not by
/// comparing the result with anything: [`Writer::align_to`] is the one
/// place the walk emits a byte that XTypes 1.3 §7.4.1's alignment rules
/// put there instead of a value, and the writer notes each span it
/// emits. The result is a mask over the returned byte string, one entry
/// per octet, true where the plan says padding.
///
/// The interop comparison uses it to check its own octet rule: a peer's
/// differing octet that lands outside this mask is a difference in a
/// value, whatever it looks like. See
/// `tools/interop-results/src/rust_column.rs`.
///
/// # Errors
/// As [`encode`]. `Emit::KeyHash` has no padding mask — the holder is a
/// separate stream — and yields an all-false one.
pub fn encode_with_padding<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
) -> Result<(Vec<u8>, Vec<bool>), ProbeError> {
    let (bytes, spans) =
        encode_noting_padding(ast, request, values, &mut Consumed::new(), true, 0)?;
    let mut mask = vec![false; bytes.len()];
    for (at, length) in spans {
        for octet in mask.iter_mut().skip(at).take(length) {
            *octet = true;
        }
    }
    Ok((bytes, mask))
}

/// A stream and the spans of it that carry no value, as
/// `(offset, length)` in the finished stream's coordinates.
type StreamWithPadding = (Vec<u8>, Vec<(usize, usize)>);

fn encode_noting_padding<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
    seen: &mut Consumed,
    note_padding: bool,
    start: usize,
) -> Result<StreamWithPadding, ProbeError> {
    let type_ = find_type(ast, &request.type_name)?;
    let root = contract::root_path(ast, type_);

    if request.emit == Emit::KeyHash {
        return key_hash(ast, type_, values, request, seen).map(|bytes| (bytes, Vec::new()));
    }

    // A dialect that frames the whole sample one way or the other
    // decides here, once, for the sample's type; every nested type is
    // framed by that choice.
    let frame = Frame {
        key: request
            .layout_key(Purpose::Data)
            .for_sample(ast, type_)
            .map_err(|e| ProbeError::NoLayout {
                detail: e.to_string(),
            })?,
        map_length: request.map_length,
        wchar: WcharPosition::Member,
        depth: 0,
    };
    let writer = Writer::continuing(
        wire_endianness(request.endianness),
        usize::from(request.encoding.max_alignment()),
        vec![0; start],
    )
    .with_pl_cdr1_framing(request.pl_cdr1_framing())
    .with_wchar_width(request.wchar_width())
    .with_wchar_framing(request.wire_wchar_framing())
    .with_wstring_unit_order(request.wire_wstring_unit_order())
    // The half of `ShortWhereItFits` a plan cannot answer: a
    // sequence whose count is in the value. See
    // `LengthCodePolicy` and `Writer::with_short_length_codes`.
    .with_short_length_codes(request.length_codes.short_code_where_it_fits());
    let mut sink = Sink {
        w: if note_padding {
            writer.recording_padding()
        } else {
            writer
        },
        seen: core::mem::take(seen),
    };
    let result = write_type(&mut sink, ast, type_, values, frame, &root);
    *seen = core::mem::take(&mut sink.seen);
    result?;

    let mut spans = sink.w.padding_spans().to_vec();
    let mut payload = sink.w.finish();
    // The octets a continuation was counted from are not the value's.
    payload.drain(..start);
    if request.emit == Emit::PayloadWithEncapsulation {
        // §7.4.1.1 pads the serialized data to the next four-byte
        // boundary with zeros, and §7.6.3.1.2 puts the count of those
        // bytes in the encapsulation options so a receiver can find
        // where the data ended. Both halves or neither: padding without
        // the count hands a conforming receiver up to three bytes of
        // zero as data, and the count without the padding tells it to
        // cut bytes that are data.
        //
        // Cyclone, RTI and Fast DDS all write both — measured on the
        // loopback wire, `encapsulation_header`'s documentation has the
        // capture. `Emit::Payload` stays unpadded: that is the
        // serialized data itself, and it is what `vendor_bytes.md`
        // records for all three.
        let padding = encapsulation_padding(payload.len());
        let mut out = Vec::with_capacity(payload.len() + 4 + usize::from(padding));
        out.extend_from_slice(&encapsulation_header_in(
            request.encapsulation_ids,
            request.encoding,
            request.endianness,
            request.endianness == Endianness::Little,
            padding,
        ));
        out.extend_from_slice(&payload);
        // The four-octet encapsulation header moves every span, and the
        // trailing fill is padding by the same clause.
        for span in &mut spans {
            span.0 += 4;
        }
        if padding != 0 {
            spans.push((out.len(), usize::from(padding)));
        }
        out.resize(out.len() + usize::from(padding), 0);
        return Ok((out, spans));
    }
    Ok((payload, spans))
}

/// The sixteen-byte KeyHash. DDSI-RTPS 2.5 §9.6.4.8, XTypes 1.3 §7.6.8.
///
/// Big-endian and member-id order whatever the data request asked for:
/// two peers that disagree about either compute different hashes for
/// the same instance.
///
/// # A type with no `@key` member has no KeyHash
///
/// This used to return sixteen bytes for one, and not the same sixteen:
/// zeroes when the empty holder planned `Extent::Sized` and
/// `d41d8cd98f00b204e9800998ecf8427e` — the MD5 of nothing — when it
/// planned `Extent::Unbounded`, which is to say whenever the type was
/// `@mutable`. Two answers to a question with no answer.
///
/// Read literally, Step 1 of §9.6.4.8 says a keyless holder keeps every
/// member — "If there are any key members, then remove the non-key
/// members from FooKeyHolder. Otherwise, do not remove any members." No
/// implementation does that, because the clause it serves does not
/// arise: a keyless topic has one instance, so there is no handle to
/// distinguish. Measured against the three: Cyclone, RTI and Fast DDS
/// all decide on `nkeys == 0` before any hash is computed and produce
/// none. Cyclone's counterfactual — what its own code would give if the
/// decision were removed — is MD5 of the empty string for `@final` and
/// `@mutable` alike, with no split of any kind. The keyed path agrees
/// with all of them: §9.6.4.8 Example 1 gives
/// `12345678 00000000 00000000 00000000`, and so do Cyclone, Fast DDS
/// and this probe.
///
/// So the answer is neither of the two this returned. Asking for the
/// KeyHash of a type that has no instances is a mistake made upstream,
/// and sixteen bytes of anything hides it. `ProbeError::NoKey` is what
/// comes back, and `crates/lang-rules/src/encode.rs::key_hash_fn` —
/// which has emitted no `key_hash` for a keyless type all along — is the
/// half of the project that was right.
fn key_hash<V: Source>(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &V,
    request: &ProbeRequest,
    seen: &mut Consumed,
) -> Result<Vec<u8>, ProbeError> {
    key_material(ast, type_, values, request, seen, true)
}

/// Encode the writer's own `serializedKey` (DDSI-RTPS 2.5 §9.4.2.11) — the
/// key-only payload a dispose/unregister DATA carries behind its K flag —
/// in the representation and byte order `request` names.
///
/// The exact inverse of [`crate::decode::serialized_key`]: the same
/// `Purpose::Key` plus `request.key_holder`/`key_holder_order`/
/// `key_holder_nesting` (`AsDeclared` + `Declaration`, `Recursive` nesting
/// already the default) select the type's own extensibility framing (a
/// DHEADER for `@appendable`/`@mutable` under XCDR2, a per-member EMHEADER
/// for `@mutable`, nothing for `@final` or under XCDR1) and declaration
/// order, in `request`'s own encoding and endianness — not the canonical
/// big-endian PLAIN_CDR2 form [`key_holder`] always writes.
///
/// `values` needs only the `@key` members populated: `write_aggregate`
/// walks [`plan()`]'s already-filtered member list (the same `Purpose::Key`
/// "holder" plan [`key_holder`]'s canonical form uses, just under a
/// different `key_holder`/`key_holder_order`), so a non-key member is
/// never looked up.
///
/// `Emit::PayloadWithEncapsulation` wraps the result behind the four-byte
/// encapsulation header and zero-pads it to a four-byte boundary, with the
/// pad count in the header's low two option bits (XTypes 1.3 §7.6.3.1.2) —
/// the same convention [`encode`] uses for a full sample.
///
/// # Errors
/// [`ProbeError`]: the type is not in the model, [`plan()`] cannot describe
/// it under this key, or a `@key` member's value is missing or refused.
pub fn serialized_key<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
) -> Result<Vec<u8>, ProbeError> {
    let type_ = find_type(ast, &request.type_name)?;
    let root = contract::root_path(ast, type_);
    let frame = Frame {
        key: request.layout_key(Purpose::Key),
        map_length: request.map_length,
        wchar: WcharPosition::Member,
        depth: 0,
    };
    let writer = Writer::with_max_align(
        wire_endianness(request.endianness),
        usize::from(request.encoding.max_alignment()),
    )
    .with_pl_cdr1_framing(request.pl_cdr1_framing())
    .with_wchar_width(request.wchar_width())
    .with_wchar_framing(request.wire_wchar_framing())
    .with_wstring_unit_order(request.wire_wstring_unit_order())
    .with_short_length_codes(request.length_codes.short_code_where_it_fits());
    let mut sink = Sink {
        w: writer,
        seen: Consumed::new(),
    };
    write_type(&mut sink, ast, type_, values, frame, &root)?;
    let payload = sink.w.finish();
    if request.emit != Emit::PayloadWithEncapsulation {
        return Ok(payload);
    }
    // §7.4.1.1 pads the serialized data to the next four-byte boundary
    // with zeros, and §7.6.3.1.2 puts the count of those bytes in the
    // encapsulation options — the same rule `encode` applies to a full
    // sample, measured against Cyclone, RTI and Fast DDS there and against
    // Cyclone's own `serializedKey` octets here (`decode::serialized_key`'s
    // documentation has the measured vectors).
    let padding = encapsulation_padding(payload.len());
    let mut out = Vec::with_capacity(payload.len() + 4 + usize::from(padding));
    out.extend_from_slice(&encapsulation_header_in(
        request.encapsulation_ids,
        request.encoding,
        request.endianness,
        request.endianness == Endianness::Little,
        padding,
    ));
    out.extend_from_slice(&payload);
    out.resize(out.len() + usize::from(padding), 0);
    Ok(out)
}

/// Actual serialized holder, before zero padding or hashing.
///
/// # Errors
/// The same layout and value refusals as the KeyHash encoder.
pub fn key_holder<V: Source>(
    ast: &Ast,
    request: &ProbeRequest,
    values: &V,
) -> Result<Vec<u8>, ProbeError> {
    let type_ = find_type(ast, &request.type_name)?;
    key_material(ast, type_, values, request, &mut Consumed::new(), false)
}

fn key_material<V: Source>(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &V,
    request: &ProbeRequest,
    seen: &mut Consumed,
    hash: bool,
) -> Result<Vec<u8>, ProbeError> {
    let holder_key = request.key_holder_key();
    let form = zerodds_ast_layout::keyhash::key_hash_form_for(ast, type_, holder_key).map_err(
        |error| ProbeError::NoLayout {
            detail: format!("{error}"),
        },
    )?;
    let frame = Frame {
        key: holder_key,
        map_length: request.map_length,
        wchar: WcharPosition::Member,
        depth: 0,
    };

    // The holder is PLAIN_CDR2 big-endian whatever the data encoding
    // is, so its alignment cap is the one PLAIN_CDR2 has (§7.6.8) — unless
    // the dialect aligns the holder to its data encoding or packs it
    // (`KeyHolderAlignment`: Cyclone and RTI under XCDR1, OpenSplice).
    let mut sink = Sink {
        w: Writer::with_max_align(
            zerodds_wire::Endianness::Big,
            usize::from(
                holder_key
                    .key_holder_alignment
                    .max_alignment(holder_key.encoding),
            ),
        )
        // From the holder's own key, so that the width the writer uses
        // is the width `primitive_layout` sized the member at. Which
        // width a key holder should use for a `wchar` is unmeasured —
        // no vendor row has one in a key — and a writer that disagreed
        // with its own plan would be wrong whatever the answer is.
        .with_wchar_width(wire_wchar_width_of(holder_key.wchar_width))
        // As the width: from the holder's own key, so that the framing
        // the writer uses is the framing `primitive_layout` sized the
        // member at.
        .with_wchar_framing(wire_wchar_framing_of(holder_key.wchar_framing))
        .with_wstring_unit_order(wire_wstring_unit_order_of(holder_key.wstring_unit_order)),
        seen: core::mem::take(seen),
    };
    let root = contract::root_path(ast, type_);
    // A union keyed by its discriminator has no `@key` member at all —
    // Table 21 puts the annotation on the discriminator, which is not a
    // `MemberNode` — so `key_members_with` answers the empty list and
    // would report the type keyless. §7.6.8's holder for it is the
    // discriminator, and the planner above sized it as exactly that.
    if let Some(discriminator) = zerodds_ast_layout::union_key_discriminator(ast, type_) {
        let path = contract::join(&root, "_d");
        let value = values
            .field("_d")
            .ok_or_else(|| ProbeError::Missing { path: path.clone() })?;
        write_type(&mut sink, ast, discriminator, value, frame, &path)?;
        *seen = core::mem::take(&mut sink.seen);
        if !hash {
            return Ok(sink.w.finish());
        }
        return Ok(match form {
            KeyHashForm::ZeroPadded => sink.w.zero_padded().to_vec(),
            KeyHashForm::Md5 => sink.w.md5().to_vec(),
        });
    }
    let keys = key_members_with(
        ast,
        type_,
        holder_key.key_holder_nesting,
        holder_key.key_holder_order,
    )
    .map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    if keys.is_empty() {
        return Err(ProbeError::NoKey {
            type_name: request.type_name.clone(),
        });
    }
    let mut result = Ok(());
    for node in &keys {
        let Some((name, member_type, _)) = member_by_node(ast, type_, *node) else {
            continue;
        };
        let path = contract::join(&root, &name);
        let Some(value) = values.field(&name) else {
            result = Err(ProbeError::Missing { path });
            break;
        };
        result = write_type(&mut sink, ast, member_type, value, frame, &path);
        if result.is_err() {
            break;
        }
    }
    *seen = core::mem::take(&mut sink.seen);
    result?;

    if !hash {
        return Ok(sink.w.finish());
    }
    Ok(match form {
        KeyHashForm::ZeroPadded => sink.w.zero_padded().to_vec(),
        KeyHashForm::Md5 => sink.w.md5().to_vec(),
    })
}

/// The two crates name byte order separately — the layout planner does
/// not depend on the runtime, and the runtime does not depend on the
/// planner. This is the one place they meet.
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

/// The member with this identity, as the contract sees it.
///
/// `contract::members` walks `all_members_of`, so a member the
/// aggregate inherited is found here and not skipped as if the plan had
/// never named it.
fn member_by_node(
    ast: &Ast,
    aggregate: Id<TypeNode>,
    node: zerodds_ast_core::NodeId,
) -> Option<(String, Id<TypeNode>, bool)> {
    contract::members(ast, aggregate)
        .into_iter()
        .find(|member| member.node == node)
        .map(|member| (member.name, member.type_, member.optional))
}

/// Write one value of one type.
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`. Every
/// descent into an aggregate goes through `Frame::in_aggregate`, and
/// `write_aggregate` and `write_union` refuse a value past the count with
/// `ProbeError::ValueTooDeep`. The collection arms descend the value; the
/// typedef arm does not, and is finite because `write_aggregate` plans the
/// type first and `zerodds_ast_layout::plan` refuses a graph deeper than
/// its own `MAX_DEPTH` (`crates/ast-layout/src/plan.rs:388`).
#[allow(clippy::too_many_lines)]
fn write_type<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => write_primitive(s, *kind, value, frame.wchar, path),
        TypeKind::String { width, bound } => {
            write_string(s, ast, *width, *bound, value, frame, path)
        }
        TypeKind::Fixed { digits, scale } => {
            write_fixed(s, *digits, *scale, frame.key.fixed_digits, value, path)
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            write_type(s, ast, target, value, frame, path)
        }
        TypeKind::Enumeration { .. } => write_enumeration(s, ast, type_, value, frame, path),
        TypeKind::Bitmask { .. } => write_bitmask(s, ast, type_, value, path),
        TypeKind::Bitset { .. } => write_bitset(s, ast, type_, value, frame, path),
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            let delimited = collection_is_delimited(ast, element, frame.key);
            let dheader = delimited.then(|| s.w.write_dheader());
            #[allow(clippy::cast_possible_truncation)]
            s.w.write_u32(items.len() as u32);
            // `SequenceElementDelimiter::BeforeEachNonFinalElement` writes
            // an element's own octet count in front of it when the
            // element's type is not `@final`, under both encoding
            // versions. Measured on HDDS's `sequence-struct-appendable`
            // and `empty-struct-in-sequence` — see the axis.
            let carries_length = sequence_element_carries_length(ast, element, frame.key)?;
            let packed = frame.key.element_padding == ElementPadding::Packed;
            for (index, item) in items.iter().enumerate() {
                let element_path = format!("{path}[{index}]");
                let saved = packed.then(|| s.w.begin_packed_element());
                let length = carries_length.then(|| s.w.write_dheader());
                write_type(s, ast, element, item, frame.in_collection(), &element_path)?;
                if let Some(length) = length {
                    s.w.try_finish_dheader(length)
                        .map_err(|error| refused(path, error))?;
                }
                if let Some(saved) = saved {
                    s.w.finish_packed_element(saved);
                }
            }
            if let Some(dheader) = dheader {
                s.w.try_finish_dheader(dheader)
                    .map_err(|error| refused(path, error))?;
            }
            Ok(())
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            let delimited = array_is_delimited(ast, element, frame.key);
            let dheader = delimited.then(|| s.w.write_dheader());
            write_array_dimension(s, ast, element, value, frame.in_collection(), path, &counts)?;
            if let Some(dheader) = dheader {
                s.w.try_finish_dheader(dheader)
                    .map_err(|error| refused(path, error))?;
            }
            Ok(())
        }
        TypeKind::Map {
            key,
            value: v,
            bound,
        } => write_map(s, ast, type_, key, v, *bound, value, frame, path),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            write_aggregate(s, ast, type_, value, frame, path)
        }
        TypeKind::Union { .. } => write_union(s, ast, type_, value, frame, path),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// One dimension of a possibly multi-dimensional array.
///
/// XTypes 1.3 §7.4.3.5 treats a multi-dimensional array as one flat run
/// with the first index varying most slowly, which is what a nested
/// JSON array in written order already is.
///
/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `write_type`, which is bounded by `crate::MAX_VALUE_DEPTH`.
fn write_array_dimension<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    element: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return write_type(s, ast, element, value, frame, path);
    };
    let items = contract::array(value, path)?;
    if items.len() != count {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("exactly {count} elements"),
            found: format!("{}", items.len()),
        });
    }
    for (index, item) in items.iter().enumerate() {
        // `ElementPadding::Packed`: the element begins where the
        // previous one ended and aligns from its own first octet. See
        // `Writer::begin_packed_element`.
        let saved = (frame.key.element_padding == ElementPadding::Packed)
            .then(|| s.w.begin_packed_element());
        write_array_dimension(
            s,
            ast,
            element,
            item,
            frame,
            &format!("{path}[{index}]"),
            rest,
        )?;
        if let Some(saved) = saved {
            s.w.finish_packed_element(saved);
        }
    }
    Ok(())
}

/// A structure or a valuetype, framed as its plan says.
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`, checked by
/// the `frame.within_depth` at the top of this function before any member
/// is written.
fn write_aggregate<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    // Before anything is written: a value that never terminates is a
    // cyclic object graph, which XTypes 1.3 §7.3.1.2.1.4 does not
    // serialise, and no prefix of one is a sample.
    frame.within_depth(path)?;

    // `ElementPadding::Packed`: an aggregate value is its own alignment
    // block. Its members count from its first octet and the aggregate is
    // not itself padded up to its own alignment before it starts —
    // measured on HDDS's `typedef-of-struct`, whose second `Cell` puts
    // its `long` at offset six where the clause puts it at eight.
    let packed = frame.key.element_padding == ElementPadding::Packed;
    let saved_origin = packed.then(|| s.w.begin_packed_element());

    let object = contract::object(value, path)?;

    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;

    // Every key in the value has to name a member. Checked before
    // anything is written, so a typo is reported instead of producing
    // a sample with a default in it.
    let members = contract::members(ast, type_);
    let declared: Vec<String> = members.iter().map(|member| member.name.clone()).collect();
    contract::reject_unknown_keys(object, &declared, path)?;

    let dheader = layout.needs_dheader.then(|| s.w.write_dheader());

    // Each nested aggregate is re-planned from the top by the call
    // below, so the parent's plan cannot hand its members anything: the
    // key they are planned under has to be recomputed here from the same
    // rule the planner used, or the two walks disagree about what a key
    // holder keeps. XTypes 1.3 §7.6.8 Step 3.
    let frame = Frame {
        key: member_key(ast, type_, frame.key),
        ..frame.in_aggregate()
    };

    // The plan names members by their stable identity and the value
    // file names them by their key; `Member` carries both, so the two
    // are matched without a second walk of the model.
    for planned in &layout.members {
        let Some(member) = members.iter().find(|member| member.node == planned.member) else {
            continue;
        };
        let member_path = contract::join(path, &member.name);
        let member_value = contract::member_value(object, member, path)?;

        match (planned.header, member_value) {
            // Rule (20), §7.4.3.5.2: a BOOLEAN `<is_present>`, then the
            // value when there is one. One byte, not an EMHEADER, and
            // written whether or not the member is there — which is what
            // makes the gap readable. Cyclone, RTI and Fast DDS all
            // write exactly this; `vendor_bytes.md` §1.
            (MemberHeader::OptionalFlag, present) => {
                s.w.write_bool(present.is_some());
                if let Some(value) = present {
                    write_type(s, ast, member.type_, value, frame, &member_path)?;
                }
            }
            // Rule (19) and §7.4.1.2: the parameter header either way,
            // and "if the member takes no value within that particular
            // object, the data length indicated by the header shall be
            // zero". `finish_pid` declares zero because nothing was
            // written between it and the header.
            (
                MemberHeader::OptionalCdr1 {
                    pid,
                    extended,
                    must_understand,
                },
                present,
            ) => {
                let header = if extended {
                    s.w.write_pid_extended(
                        planned.member_id,
                        ParameterRole::Optional,
                        must_understand,
                    )
                } else {
                    s.w.write_pid(pid, ParameterRole::Optional, must_understand)
                }
                .map_err(|error| refused(&member_path, error))?;
                if let Some(value) = present {
                    write_type(s, ast, member.type_, value, frame, &member_path)?;
                }
                s.w.finish_pid(header);
            }
            // An `@optional` member of a `@mutable` type that took no
            // value. Nothing goes out: the parameter list itself says
            // which members are there, so an absent one leaves no trace.
            // `contract::member_value` returns `None` for no other
            // member, so the three headers below never lose a value here.
            (
                MemberHeader::NoHeader
                | MemberHeader::PidCdr1 { .. }
                | MemberHeader::EmHeader { .. },
                None,
            ) => {}
            // Rule (18): the value and nothing else.
            (MemberHeader::NoHeader, Some(value)) => {
                write_type(s, ast, member.type_, value, frame, &member_path)?;
            }
            // Rules (24) and (25).
            (
                MemberHeader::PidCdr1 {
                    pid,
                    extended,
                    must_understand,
                },
                Some(value),
            ) => {
                let header = if extended {
                    s.w.write_pid_extended(
                        planned.member_id,
                        ParameterRole::Member,
                        must_understand,
                    )
                } else {
                    s.w.write_pid(pid, ParameterRole::Member, must_understand)
                }
                .map_err(|error| refused(&member_path, error))?;
                write_type(s, ast, member.type_, value, frame, &member_path)?;
                s.w.finish_pid(header);
            }
            // Rule (22).
            (
                MemberHeader::EmHeader {
                    lc,
                    must_understand,
                },
                Some(value),
            ) => {
                let header =
                    s.w.write_emheader(planned.member_id, length_code(lc), must_understand)
                        .map_err(|error| refused(&member_path, error))?;
                write_type(s, ast, member.type_, value, frame, &member_path)?;
                s.w.finish_emheader(header)
                    .map_err(|error| refused(&member_path, error))?;
            }
        }
    }

    if layout.needs_sentinel {
        s.w.write_sentinel();
    }
    if let Some(dheader) = dheader {
        s.w.try_finish_dheader(dheader)
            .map_err(|error| refused(path, error))?;
    }
    if let Some(saved) = saved_origin {
        s.w.finish_packed_element(saved);
    }
    Ok(())
}

/// One `MMEMBER` of a `@mutable` type, framed as its plan says.
///
/// Shared by the aggregate walk and the union walk: rules (21) to (25)
/// frame a structure's members and rules (27) and (28) frame a union's
/// discriminator and selected member the same way, so one function
/// writes both.
#[allow(clippy::too_many_arguments)]
fn write_mmember<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    header: MemberHeader,
    member_id: u32,
    type_: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    match header {
        MemberHeader::PidCdr1 {
            pid,
            extended,
            must_understand,
        } => {
            let header = if extended {
                s.w.write_pid_extended(member_id, ParameterRole::Member, must_understand)
            } else {
                s.w.write_pid(pid, ParameterRole::Member, must_understand)
            }
            .map_err(|error| refused(path, error))?;
            write_type(s, ast, type_, value, frame, path)?;
            s.w.finish_pid(header);
            Ok(())
        }
        MemberHeader::EmHeader {
            lc,
            must_understand,
        } => {
            let header =
                s.w.write_emheader(member_id, length_code(lc), must_understand)
                    .map_err(|error| refused(path, error))?;
            write_type(s, ast, type_, value, frame, path)?;
            s.w.finish_emheader(header)
                .map_err(|error| refused(path, error))?;
            Ok(())
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

/// A union: the discriminator, then the selected member.
///
/// Under `@final` and `@appendable` that is rules (26) and (30) — the
/// discriminator as a `NOPT_FMEMBER`, then the branch, behind a DHEADER
/// where the extensibility calls for one.
///
/// Under `@mutable` it is rules (27) and (28): the discriminator and the
/// selected member each carry an `MMEMBER` header, and under XCDR1 a
/// `PID_LIST_END` closes the list. This project used to write neither.
/// `08000000 01000000 07000000` is a DHEADER over an unframed
/// discriminator and member; a peer parsing it as PL_CDR2 reads
/// `01000000` as an EMHEADER with LC0 and hands back a one-byte member
/// with no error. RTI and Fast DDS both write the clause's bytes —
/// `vendor_bytes.md` §8.
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`, checked by
/// the `frame.within_depth` at the top of this function.
fn write_union<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    // As in `write_aggregate`: a union on a cycle nests a value just as
    // a structure does.
    frame.within_depth(path)?;

    // As in `write_aggregate`: under `ElementPadding::Packed` a union is
    // its own alignment block, and `annotation-external-union-branch`
    // records one that starts where the previous member ended.
    let packed = frame.key.element_padding == ElementPadding::Packed;
    let saved_origin = packed.then(|| s.w.begin_packed_element());

    let object = contract::object(value, path)?;
    let discriminator_value = contract::discriminator(object, path)?;

    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let dheader = layout.needs_dheader.then(|| s.w.write_dheader());
    // A union's discriminator and its branch are members, whatever the
    // union itself was reached as.
    let frame = frame.in_aggregate();

    let TypeKind::Union { discriminator, .. } = &ast
        .type_(type_)
        .map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?
        .kind
    else {
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
    let discriminator_path = contract::join(path, "_d");
    match layout.discriminator {
        None => write_type(
            s,
            ast,
            discriminator,
            discriminator_value,
            frame,
            &discriminator_path,
        )?,
        Some(header) => write_mmember(
            s,
            ast,
            header,
            0,
            discriminator,
            discriminator_value,
            frame,
            &discriminator_path,
        )?,
    }

    // The member the value selects, by name. A union's members hang
    // off its cases, not off `members_of` — a case is the label list
    // and the member together.
    //
    // The probe does not evaluate the labels against the
    // discriminator: a value that names a member the discriminator
    // does not select is a test that wants exactly that, and refusing
    // it would make the divergence untestable.
    let cases = contract::union_cases(ast, type_);

    // Unless a setting says the labels decide. `union_negative_matches_nothing`
    // is that setting's one crossing; under the default it is false for
    // every discriminator and this walk is the one below, unchanged.
    let suppress_negative = union_negative_matches_nothing(ast, type_, frame.key.union_label);
    if suppress_negative {
        write_selected_branch(
            s,
            ast,
            &cases,
            object,
            discriminator_value,
            &layout,
            frame,
            path,
            suppress_negative,
        )?;
    } else {
        for (name, member_value) in object {
            if name == "_d" {
                continue;
            }
            let case =
                cases
                    .iter()
                    .find(|case| case.name == name)
                    .ok_or_else(|| ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: cases.iter().map(|case| case.name.clone()).collect(),
                    })?;
            let member_path = contract::join(path, name);
            contract::reject_null(member_value, &member_path)?;
            match layout.members.iter().find(|plan| plan.member == case.node) {
                None => write_type(s, ast, case.type_, member_value, frame, &member_path)?,
                Some(plan) => write_mmember(
                    s,
                    ast,
                    plan.header,
                    plan.member_id,
                    case.type_,
                    member_value,
                    frame,
                    &member_path,
                )?,
            }
        }
    }

    if layout.needs_sentinel {
        s.w.write_sentinel();
    }
    if let Some(dheader) = dheader {
        s.w.try_finish_dheader(dheader)
            .map_err(|error| refused(path, error))?;
    }
    if let Some(saved) = saved_origin {
        s.w.finish_packed_element(saved);
    }
    Ok(())
}

/// The branch the **labels** select, and the octets a tagless union has
/// where they select a member the sample did not name.
///
/// Reached only under [`zerodds_ast_layout::UnionLabelMatch::NegativeLabelUnmatchedBelow32`]
/// and only for a signed discriminator narrower than 32 bits — the
/// crossing at the call site decides both, so nothing here re-reads the
/// policy.
///
/// Three outcomes, all three measured on Cyclone DDS 11.0.1:
///
/// - the labels select the member the sample named — write it, which is
///   what the walk above does anyway;
/// - the labels select nothing and the union declares no `default:` —
///   write no branch at all. `union-discriminator-{int8,short}`, four
///   octets shorter than the clause;
/// - the labels select the `default:` branch — write the union's
///   **storage** read through it. `combined-everything`.
///
/// # The storage
///
/// A C union puts every branch at offset zero, so the storage holds the
/// named member's octets in the host's order with whatever was above
/// them — zeros, because `wireprobe-zerodds-c-gen` declares the sample
/// `static`. This column has a tagged representation and no such
/// storage, so it is synthesised: the named member written
/// little-endian, zero-extended to the selected branch's extent, and
/// emitted as one unsigned value of that width in the stream's order.
///
/// As an unsigned integer and not as the branch's own type: the octets
/// are a bit pattern and a `double` that went through
/// `f64::from_le_bytes` and back is a value, not a pattern. A signalling
/// NaN pattern is the case where the two differ and no test would show
/// it until one did.
///
/// # What it refuses, and why nothing reaches it
///
/// Every branch has to be a primitive of one, two, four or eight
/// octets, and
/// [`zerodds_ast_layout::union_negative_matches_nothing`] already
/// answers `false` for a union where one is not: a `string` branch has
/// no storage a probe can overlay, the peer reads a pointer out of it,
/// and `union-member-sequence` is recorded as a segmentation fault
/// rather than as octets. So the refusals below are unreachable through
/// the crossing and are kept loud rather than removed —
/// `write_selected_branch` is not the place to decide that a wire
/// question has no answer.
#[allow(clippy::too_many_arguments)]
fn write_selected_branch<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    cases: &[contract::Case],
    object: Fields<'_, V>,
    discriminator_value: &V,
    layout: &TypeLayout,
    frame: Frame,
    path: &str,
    suppress_negative: bool,
) -> Result<(), ProbeError> {
    // Through `contract`, which is where the value dialect lives: a
    // 64-bit integer arrives as a string and a narrow one as a number,
    // and a backend that read the document itself would accept one
    // spelling the other two refuse.
    let discriminator = contract::integer(discriminator_value, &contract::join(path, "_d"))?;
    let named = object.names().into_iter().find(|name| *name != "_d");

    // Every key the sample carries still has to name a declared case:
    // the suppression decides which branch goes out, not whether a typo
    // is a typo.
    for name in object.names() {
        if name != "_d" && !cases.iter().any(|case| case.name == name) {
            return Err(ProbeError::Unknown {
                path: contract::join(path, name),
                declared: cases.iter().map(|case| case.name.clone()).collect(),
            });
        }
    }

    let selection = contract::union_selection(ast, cases, discriminator, suppress_negative)
        .map_err(|unevaluated| ProbeError::Mismatch {
            path: path.to_string(),
            expected: String::from("a union whose case labels all evaluate"),
            found: format!(
                "the label of case `{}`, which the model cannot evaluate",
                cases
                    .get(unevaluated.case)
                    .map_or("<out of range>", |case| case.name.as_str())
            ),
        })?;
    let Some(index) = selection else {
        // No label matched and there is no `default:`. The discriminator
        // is already on the wire and nothing follows it.
        return Ok(());
    };
    let case = &cases[index];
    let member_path = contract::join(path, &case.name);
    let planned = layout.members.iter().find(|plan| plan.member == case.node);

    if named == Some(case.name.as_str()) {
        let member_value = object.get(&case.name).ok_or_else(|| ProbeError::Missing {
            path: member_path.clone(),
        })?;
        contract::reject_null(member_value, &member_path)?;
        return match planned {
            None => write_type(s, ast, case.type_, member_value, frame, &member_path),
            Some(plan) => write_mmember(
                s,
                ast,
                plan.header,
                plan.member_id,
                case.type_,
                member_value,
                frame,
                &member_path,
            ),
        };
    }

    let width = storage_width(ast, case.type_, frame.key).ok_or(ProbeError::Unsupported {
        path: member_path.clone(),
        kind: "a union branch with no fixed-width storage to overlay",
    })?;
    let octets =
        match named {
            None => vec![0_u8; width],
            Some(name) => {
                let source = cases.iter().find(|case| case.name == name).ok_or_else(|| {
                    ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: cases.iter().map(|case| case.name.clone()).collect(),
                    }
                })?;
                let source_width =
                    storage_width(ast, source.type_, frame.key).ok_or(ProbeError::Unsupported {
                        path: contract::join(path, name),
                        kind: "a union branch with no fixed-width storage to overlay",
                    })?;
                let mut host = Sink {
                    w: Writer::little_endian(),
                    seen: Consumed::new(),
                };
                let named_value = object.get(name).ok_or_else(|| ProbeError::Missing {
                    path: contract::join(path, name),
                })?;
                write_type(
                    &mut host,
                    ast,
                    source.type_,
                    named_value,
                    frame,
                    &contract::join(path, name),
                )?;
                // The scratch sink read the named member out of the value
                // document, and that read is the sample's whatever branch
                // the octets end up framed as. `Consumed::absorb` says why
                // this is not bookkeeping.
                s.seen.absorb(&host.seen);
                let mut octets = host.w.finish();
                if octets.len() != source_width {
                    return Err(ProbeError::Unsupported {
                        path: contract::join(path, name),
                        kind: "a union branch whose octets are not its storage",
                    });
                }
                octets.resize(width, 0);
                octets
            }
        };

    let mut pattern = [0_u8; 8];
    pattern[..width].copy_from_slice(&octets[..width]);
    let pattern = u64::from_le_bytes(pattern);
    #[allow(clippy::cast_possible_truncation)]
    fn write_pattern(s: &mut Sink, width: usize, pattern: u64) {
        match width {
            1 => s.w.write_u8(pattern as u8),
            2 => s.w.write_u16(pattern as u16),
            4 => s.w.write_u32(pattern as u32),
            _ => s.w.write_u64(pattern),
        }
    }
    // The branch that goes out, recorded as written and not as read:
    // where a member was named, `absorb` above has already recorded the
    // read, and where none was, `None => vec![0; width]` reads nothing
    // at all.
    if named.is_none() {
        s.seen.scalar(&member_path);
    }
    match planned {
        None => write_pattern(s, width, pattern),
        // Rules (27) and (28) still frame the branch a `@mutable` union
        // writes, whichever branch it is.
        Some(plan) => match plan.header {
            MemberHeader::PidCdr1 {
                pid,
                extended,
                must_understand,
            } => {
                let header = if extended {
                    s.w.write_pid_extended(plan.member_id, ParameterRole::Member, must_understand)
                } else {
                    s.w.write_pid(pid, ParameterRole::Member, must_understand)
                }
                .map_err(|error| refused(&member_path, error))?;
                write_pattern(s, width, pattern);
                s.w.finish_pid(header);
            }
            MemberHeader::EmHeader {
                lc,
                must_understand,
            } => {
                let header =
                    s.w.write_emheader(plan.member_id, length_code(lc), must_understand)
                        .map_err(|error| refused(&member_path, error))?;
                write_pattern(s, width, pattern);
                s.w.finish_emheader(header)
                    .map_err(|error| refused(&member_path, error))?;
            }
            MemberHeader::NoHeader
            | MemberHeader::OptionalCdr1 { .. }
            | MemberHeader::OptionalFlag => {
                return Err(ProbeError::NoLayout {
                    detail: format!(
                        "{member_path}: a `@mutable` member planned with a header that is not one"
                    ),
                });
            }
        },
    }
    Ok(())
}

/// The octets a primitive branch occupies in a C union's storage, or
/// `None` for a branch that has none.
///
/// The plan's extent and not a table of widths: a `wchar` is two octets
/// under one framing and one plus two under another, and a branch whose
/// extent is not a power-of-two count of octets has no overlay.
fn storage_width(ast: &Ast, type_: Id<TypeNode>, key: LayoutKey) -> Option<usize> {
    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, key, &mut deps).ok()?;
    let width = usize::try_from(layout.extent.sized()?).ok()?;
    matches!(width, 1 | 2 | 4 | 8).then_some(width)
}

/// An enumeration, written as the integer its bit bound calls for.
fn write_enumeration<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    let (name, declared) = contract::enumerator(ast, type_, value, path)?;
    s.seen.scalar(path);

    // Through the crossing function and not through
    // `Ast::effective_bit_bound`: the holder's width is an axis, and a
    // site that reads the declared bound straight answers it by
    // omission. `zerodds_ast_layout::enumeration_bit_bound` says why.
    let bits = enumeration_bit_bound(ast, type_, frame.key.enum_holder);
    // And which integer goes into that holder is the other axis, asked
    // through the sibling crossing. The contract resolved the name to
    // the enumerator the sample means; this decides what that enumerator
    // serialises as. `zerodds_ast_layout::enumerator_wire_value`.
    // Refused and not defaulted to `declared`: under `Ordinal` the
    // declared value is a different integer, and writing it would put
    // octets on the wire that no peer under this setting produces while
    // reporting success. `wire_value_of` says why.
    let value = match wire_value_of(ast, type_, &name, frame.key.enumerator_value) {
        Ok(Some(value)) => value,
        Ok(None) | Err(StaleEnumerator) => {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: format!(
                    "an enumerator whose wire value the `{}` setting can compute",
                    frame.key.enumerator_value.as_str()
                ),
                found: format!("{name}, declared {declared}, whose wire value does not evaluate"),
            });
        }
    };
    let pattern = enumerator_pattern(value, bits).ok_or_else(|| ProbeError::Mismatch {
        path: path.to_string(),
        expected: format!("an enumerator a {bits}-bit holder can hold"),
        found: format!("{name} = {value}"),
    })?;
    // The mask in `enumerator_pattern` already narrowed the value to the
    // holder's width, so each cast below is exact.
    #[allow(clippy::cast_possible_truncation)]
    match bits {
        1..=8 => s.w.write_u8(pattern as u8),
        9..=16 => s.w.write_u16(pattern as u16),
        _ => s.w.write_u32(pattern),
    }
    Ok(())
}

/// The integer a named enumerator of this type serialises as.
///
/// The name has already been checked against the declared set by
/// `contract::enumerator`; this walks the type once more to reach the
/// handle the crossing function takes.
///
/// # Why `None` is not answered with the declared value
///
/// It was: `wire_value_of(..).unwrap_or(declared)`. Under
/// [`EnumeratorValue::Declared`] that substitution is invisible because
/// the declared value *is* the answer; under
/// [`EnumeratorValue::Ordinal`] it writes a different integer, and the
/// probe reports success over octets no peer under that setting would
/// produce. `enumerator_wire_value` refuses to substitute a zero for
/// exactly this reason — substituting the declared value is the same
/// failure with a different substitute, and it is harder to see because
/// it is right half the time. The caller refuses instead.
///
/// A stale handle inside the walk is not skipped either: skipping it
/// would make the enumerator the sample names unreachable and hand the
/// walk's `None` back as though the name were undeclared, which
/// `contract::enumerator` has already ruled out.
fn wire_value_of(
    ast: &Ast,
    type_: Id<TypeNode>,
    name: &str,
    policy: EnumeratorValue,
) -> Result<Option<i128>, StaleEnumerator> {
    for id in ast.enumerators_of(type_) {
        let node = ast.enumerator(*id).map_err(|_| StaleEnumerator)?;
        if node.name == name {
            return Ok(enumerator_wire_value(ast, *id, policy));
        }
    }
    Ok(None)
}

/// An enumerator handle `enumerators_of` produced and the arena no
/// longer holds.
///
/// A unit type rather than a message, because the caller is the only
/// place that knows the path and the enumerator's name.
struct StaleEnumerator;

/// The bit pattern an enumerator's value has in its holder, or `None`
/// where the value does not fit the bound.
///
/// XTypes 1.3 §7.4.3.5.3 rule (5) is `XCDR << {O : ENUM_TYPE} = XCDR <<
/// { O.value : O.holder_type }`, and the ENUM_TYPE entry in the same
/// clause's table says "O.holder_type is either Int8, Int16 or Int32
/// depending on the value of the @bit_bound annotation". A **signed**
/// holder: `@value(-1)` is the two's complement of one, so at the
/// default bound it is `ffffffff`. Clamping it to zero, as
/// `u32::try_from(value).unwrap_or(0)` did, wrote a different
/// enumerator's value and reported success.
///
/// # What fits
///
/// The bound is a width, and the two implementations that accept these
/// constructs read it as one. Measured on 2026-08-09:
///
/// - RTI Connext 7.7 takes `@value(-1)` at the default bound and writes
///   `ffffffff`. Cyclone's `idlc` refuses the annotation outright
///   ("@value(-1) cannot be applied to 'enumerator' element"), so RTI is
///   the only oracle for a negative value.
/// - Cyclone takes `@value(200)` under `@bit_bound(8)` and writes `c8`.
///   RTI refuses any enum bound but 32, so Cyclone is the only oracle
///   for a narrow one.
///
/// Between them the accepted range is everything expressible in the
/// bound's bits, signed or unsigned: `-2^(n-1) ..= 2^n - 1`. A stricter
/// signed-only range would reject `@value(200)`, which Cyclone writes;
/// a wider one would accept values neither implementation can express.
///
/// §7.3.1.2.1.5 caps the bound itself: "the value member of the
/// annotation may take any value from 1 to 32, inclusive, when this
/// annotation is applied to an enumerated type." A bound outside that
/// has no holder to be written into, so it is refused rather than
/// widened to eight bytes.
fn enumerator_pattern(value: i128, bits: u16) -> Option<u32> {
    if bits == 0 || bits > 32 {
        return None;
    }
    let width = i128::from(bits);
    if value < -(1i128 << (width - 1)) || value > (1i128 << width) - 1 {
        return None;
    }
    let mask = (1u128 << width) - 1;
    u32::try_from((value as u128) & mask).ok()
}

/// A `string<N>` or `wstring<N>`, refusing a value the bound cannot hold.
///
/// The bytes are the same whether the string is bounded or not — the
/// bound is a promise about the source and not a difference on the wire
/// — so this is the only place it can be enforced. It was not enforced
/// anywhere: `let _ = bound;` sat here under a comment claiming the
/// front end caught an overrun, and the front end never sees the value.
/// Eight characters went into a `string<4>` and the probe said nothing,
/// which is the one thing a probe must not do.
///
/// # What the bound counts
///
/// Octets for `string<N>` and UTF-16 code units for `wstring<N>`, which
/// is what the receiving side has room for: §7.5.1.2.2 maps `string<5>`
/// to `char[6]` and `wstring<5>` to a five-unit buffer in every measured
/// implementation's C mapping, so a five-scalar value of ten octets is
/// one no peer can hold. `runtime/python/zerodds_wire/typesupport.py`
/// counts the same two ways — `utf8_octets` and `utf16_units` — and the
/// two columns have to agree on which values they refuse.
///
/// The NUL is not counted. §7.4.1.1.2 puts it on the wire for a narrow
/// string and not in the bound: `string<5>` holds five characters.
fn write_string<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    width: CharWidth,
    bound: Bound,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    let text = contract::text(value, path)?;
    s.seen.scalar(path);
    if let Some(limit) = bound.expr().and_then(|expr| ast.eval_integral(expr).ok()) {
        let (count, noun) = match width {
            CharWidth::Char => (text.len(), "octets"),
            CharWidth::WChar => (text.chars().map(utf16_units).sum(), "UTF-16 code units"),
        };
        if i128::try_from(count).unwrap_or(i128::MAX) > limit {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: format!("at most {limit} {noun}, which is what the bound holds"),
                found: format!("{count}"),
            });
        }
    }
    match width {
        CharWidth::Char => {
            s.w.write_string(text);
            Ok(())
        }
        CharWidth::WChar => {
            if frame.key.wstring_code_units == WstringCodeUnits::Utf8 {
                write_wstring_utf8(
                    &mut s.w,
                    text,
                    frame.key.wstring_length,
                    layout_position(frame.wchar),
                    path,
                )
            } else {
                write_wstring(
                    &mut s.w,
                    text,
                    frame.key.wstring_length,
                    frame.key.wstring_unit_order,
                    // The position the `wstring` is reached from. Passed and
                    // not assumed, for the reason `read_wstring` gives.
                    layout_position(frame.wchar),
                    path,
                )
            }
        }
    }
}

/// How many UTF-16 code units one scalar takes.
///
/// Two above the Basic Multilingual Plane, which is a surrogate pair —
/// §7.4.1.1.2's own rationale for counting bytes rather than characters.
const fn utf16_units(character: char) -> usize {
    if (character as u32) > 0xFFFF { 2 } else { 1 }
}

/// A `wstring`: a four-byte length, then UTF-16 code units.
///
/// XTypes 1.3 §7.4.3.5.3 rule (4) writes `O.ssize` as a `UInt32` and
/// then the `Char16` units, annotated "No NUL" against rule (3)'s
/// "includes NUL" for a narrow string. Two things follow, and only the
/// first is visible in the rule.
///
/// What `O.ssize` counts is settled in §7.4.1.1.2: "The serialized
/// length of an object of type String<Char16> shall be the number of
/// bytes in the CDR buffer taken by the String<Char16> characters. This
/// is twice the number of characters in the string". Code units, not
/// characters — the clause's own rationale gives the reason, that a
/// code point outside the Basic Multilingual Plane takes two UTF-16
/// units and a byte count still reaches the end of the string. So a
/// `"𝄞"` is a length of four and not of one or two.
///
/// The same clause forbids the two things a UTF-16 encoder would
/// otherwise emit: "The UTF-16 representation ... shall not include a
/// Byte Order Mark (BOM). The representation shall also not include any
/// terminating NUL character(s)." The order the units go out in is the
/// stream's, which is what the encapsulation identifier already
/// announced.
///
/// # The other two forms are written on request
///
/// Cyclone writes the clause above under both versions and RTI writes
/// it under XCDR2; RTI's XCDR1 and Fast DDS's output are two other
/// things, and [`WstringLengthPolicy`] carries the measurement and the
/// argument. Only the count and the character width change here; the
/// text is the same text.
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

fn write_wstring(
    w: &mut Writer,
    text: &str,
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
    position: WirePosition,
    path: &str,
) -> Result<(), ProbeError> {
    // The coordinates are asked separately because they are separate:
    // C1 says what the length counts, C2 whether a terminator joins the
    // run, C3 how wide a unit is, and C4 — on its own axis now — in
    // what order each one lands. Folding them together is what produced
    // `UnswappedUnits`, a byte order wearing a length policy's name,
    // and it is what left the cell both ROS type supports write with no
    // name at all.
    let mut units: Vec<u16> =
        if matches!(policy.counts_at(position), WstringLengthCounts::Characters)
            && policy.unit_at(position) == WstringUnitWidth::Two
        {
            // Fast CDR counts `wcslen` characters and then writes each
            // four-byte `wchar_t` as two bytes. A code point outside the
            // BMP loses its high bits there: `U+1D11E` goes out as `d11e`,
            // which is `U+D11E`, a Hangul syllable and a well-formed code
            // point. The peer reads a different character and has nothing
            // to object to. Reproducing a vendor's length convention is a
            // policy; reproducing a data loss no reader can see is not, so
            // this refuses.
            //
            // The refusal is on the pair (characters, two octets) and not
            // on a named form: it is the narrow unit that loses the high
            // bits, and a character count over four-octet units — the ROS
            // cell — carries the surrogate pair intact and is not refused.
            if text.chars().any(|c| c as u32 > 0xFFFF) {
                return Err(ProbeError::Unsupported {
                    path: path.to_string(),
                    kind: concat!(
                        "a code point outside the BMP under a character count over two-octet ",
                        "units, which has no way to carry it — Fast CDR truncates it to ",
                        "sixteen bits and sends a different character"
                    ),
                });
            }
            #[allow(clippy::cast_possible_truncation)]
            text.chars().map(|c| c as u32 as u16).collect()
        } else {
            text.encode_utf16().collect()
        };
    if matches!(policy.terminator_at(position), WstringTerminator::Present) {
        // CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form. The NUL
        // is a unit of the run and is inside whatever the length counts.
        units.push(0);
    }

    // The byte-order mark is a code unit like any other and its octets
    // are counted in the length: omniORB writes `0a000000 fffe 6100
    // 34d8 1edd 6200` for `"a𝄞b"`, ten octets over five units.
    // §7.4.1.1.2 forbids it outright — "shall not include a Byte Order
    // Mark" — so it is here only under `ByteOrderMark`, which no DDS
    // dialect selects.
    // An **empty** `wstring` carries no mark: omniORB writes `00000000`
    // for one, measured 2026-08-10 as `wstring-unbounded-empty`, which
    // is byte-identical to every other implementation. A mark on an
    // empty string would be a two-octet string that decodes to nothing,
    // and no peer writes it.
    if order.has_mark() && !units.is_empty() {
        units.insert(0, BYTE_ORDER_MARK);
    }

    let width = policy.character_width(position);
    #[allow(clippy::cast_possible_truncation)]
    // C1, over units that already include the terminator and the mark.
    // An octet count multiplies by C3; a character count is the run's
    // length. One expression rather than an arm per named form, so a
    // cell nobody has measured computes its length the same way.
    let declared = match policy.counts_at(position) {
        WstringLengthCounts::Octets => (units.len() * width) as u32,
        WstringLengthCounts::Characters => units.len() as u32,
    };
    w.write_u32(declared);

    for unit in units {
        write_wstring_unit(w, unit, width, policy, order);
    }
    Ok(())
}

/// A `wstring` whose characters go out as their own UTF-8 octets.
///
/// [`WstringCodeUnits::Utf8`] is the measured departure of one product's
/// generated serialiser. XTypes 1.3 Table 31 gives a `wstring`'s code
/// unit as `Char16` and §7.4.1.1.2 makes the length "twice the number
/// of characters", so the clause has no UTF-8 form at all; the axis
/// carries the measurement and the recorded rows.
///
/// The length is the UTF-8 octet count, not a count of anything a
/// two-octet unit could be multiplied by — under this value C1 and C3
/// have nothing to say (see [`WstringCodeUnits::counts_octets`]) and
/// only the terminator still reaches the wire. `wstring-member` is
/// `04 00 00 00 61 62 63 00` for `"abc"`: three octets, a NUL, and the
/// four declares all of it.
fn write_wstring_utf8(
    w: &mut Writer,
    text: &str,
    policy: WstringLengthPolicy,
    position: WirePosition,
    path: &str,
) -> Result<(), ProbeError> {
    let bytes = text.as_bytes();
    let terminator = matches!(policy.terminator_at(position), WstringTerminator::Present);
    let content = bytes.len() + usize::from(terminator);
    let declared = u32::try_from(content).map_err(|_| ProbeError::Unsupported {
        path: path.to_string(),
        kind: "a wstring whose UTF-8 octet count fits the UInt32 length field",
    })?;
    w.write_u32(declared);
    for byte in bytes {
        w.write_u8(*byte);
    }
    if terminator {
        w.write_u8(0);
    }
    Ok(())
}

/// Whether an element of a `sequence` is preceded by its own octet
/// length under this key.
///
/// [`SequenceElementDelimiter::BeforeEachNonFinalElement`] is the
/// measured departure: a four-octet `UInt32` in front of every element
/// whose type is not `@final`, under both encoding versions. A type
/// with no extensibility of its own — a primitive, a string, an
/// enumeration, a `typedef` — is `@final` by construction and takes
/// none; only the four aggregate kinds can be `@appendable` or
/// `@mutable`.
///
/// The alias is **not** seen through, and that is the measurement.
/// `recursive-typedef-sequence` is `sequence<NodeRef>` with
/// `typedef Node NodeRef` over an `@appendable Node`, and HDDS writes
/// no element length: its generated sequence encoder decides on the
/// declared element type, and a `typedef` carries no `@final`/
/// `@appendable` of its own. A `sequence<Later>` of a forward-declared
/// structure, and a `sequence<Node>` of the structure itself, both carry
/// it.
///
/// # Errors
/// [`ProbeError::NoLayout`] for an element id the model does not hold:
/// whether the element carries a length cannot be told, and guessing
/// either way frames every element of the sequence wrong.
pub(crate) fn sequence_element_carries_length(
    ast: &Ast,
    element: Id<TypeNode>,
    key: LayoutKey,
) -> Result<bool, ProbeError> {
    zerodds_ast_layout::xcdr::sequence_element_carries_length(ast, element, key).ok_or_else(|| {
        ProbeError::NoLayout {
            detail: format!(
                "the sequence element {element:?} is not a type the model holds, so whether it \
                 carries its own length cannot be told"
            ),
        }
    })
}

/// The UTF-16 byte-order mark, `U+FEFF`.
///
/// Named because it appears on both sides of the wire and a bare
/// `0xFEFF` in a reader would read as a character of the string.
const BYTE_ORDER_MARK: u16 = 0xFEFF;

/// One `wstring` code unit, at its width and in its order.
///
/// The order and the length convention are separate settings and this is
/// where the two meet, once, so that a form cannot be written one way in
/// the length field and another in the content.
fn write_wstring_unit(
    w: &mut Writer,
    unit: u16,
    width: usize,
    policy: WstringLengthPolicy,
    order: WstringUnitOrder,
) {
    // C3 picks the writer and C4 picks its endianness. There is no
    // longer an arm for a length policy that decides a byte order:
    // `HostUnswapped` is a point of the order axis, so Cyclone's form
    // and `rosidl_typesupport_fastrtps`'s are the same answer at two
    // different widths rather than two unrelated special cases.
    let _ = policy;
    match (width, order) {
        (2, WstringUnitOrder::Stream) => w.write_u16(unit),
        (2, WstringUnitOrder::BigEndianAlways) => w.write_u16_big(unit),
        (2, WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped) => {
            w.write_u16_little(unit);
        }
        (_, WstringUnitOrder::Stream) => w.write_u32(u32::from(unit)),
        (_, WstringUnitOrder::BigEndianAlways) => w.write_u32_big(u32::from(unit)),
        (_, WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped) => {
            w.write_u32_little(u32::from(unit));
        }
    }
}

/// A bitmask, written as the unsigned integer its bit bound calls for.
///
/// XTypes 1.3 §7.4.3.5.3 rule (6) writes `O.value` as
/// `O.holder_type`, and Table 33 of §7.4.1.1.3.2 gives the holder:
/// `UInt8` for a bound of 1 to 8, `UInt16` to 16, `UInt32` to 32 and
/// `UInt64` to 64. The same clause fixes the direction — "Bit indexes
/// are counted from zero starting at the least-significant bit of the
/// full byte size of the bitmask" — so bit `n` is `1 << n` in the
/// holder whatever the holder's width.
///
/// A position at or past the bound is refused. §7.4.1.1.3.2 says the
/// bits above the bound "are not considered to be part of the
/// bitmask", so setting one would put a value on the wire that no peer
/// is required to read back.
fn write_bitmask<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<(), ProbeError> {
    let set = contract::bit_names(ast, type_, value, path)?;
    let bound = ast.effective_bit_bound(type_);

    let mut bits: u64 = 0;
    for (index, (_, position)) in set.iter().enumerate() {
        s.seen.scalar(&format!("{path}[{index}]"));
        bits |= 1u64 << position;
    }

    // `contract::bit_names` refused a bound outside 1..=64, so each
    // narrowing below is exact for the bits that survived it.
    match bound {
        1..=8 => s.w.write_u8(u8::try_from(bits).unwrap_or(0)),
        9..=16 => s.w.write_u16(u16::try_from(bits).unwrap_or(0)),
        17..=32 => s.w.write_u32(u32::try_from(bits).unwrap_or(0)),
        _ => s.w.write_u64(bits),
    }
    Ok(())
}

/// A bitset, written as the anchor integer its bitfields pack into.
///
/// # The specification does not say
///
/// DDS-XTypes 1.3 has no serialisation rule for a bitset. §7.4.3.5.3
/// runs from rule (1) to rule (30) and no left-hand side matches one;
/// §7.4.1.1 covers strings, enumerations, bitmasks, maps and structures
/// and stops. The only normative text about bitsets at all is the
/// TypeObject IDL's `CommonBitfield { unsigned short position;
/// BitsetMemberFlag flags; octet bitcount; TypeKind holder_type; }`
/// with its `// Ordered by Bitfield.position`.
///
/// **There is no clause number to cite for the wire form, and this
/// comment used to carry a wrong one.** It said §7.3.4.6, which is
/// "Classification of TypeIdentifiers" and says nothing about bitsets.
/// A wrong citation is worse than none: the next reader follows it,
/// finds text that does not support the claim, and has to work out
/// whether the claim or the pointer is broken.
/// `docs/refused-constructs.md`, "bitset", walks the whole document and
/// records where every rule this file follows actually comes from.
///
/// Two things are therefore taken from elsewhere and are marked as such
/// in the tests:
///
/// - **The positions.** `CommonBitfield.position` has to come from
///   somewhere, and the only reading under which an IDL bitset
///   round-trips through a TypeObject is that positions run
///   cumulatively from zero in declaration order, anonymous padding
///   fields included. That is the rule
///   `crates/xml/src/typeobject_bridge.rs` already fills the field by.
/// - **The anchor.** How wide the integer holding those bits is comes
///   from the layout planner, `zerodds_ast_layout::plan`, and is not
///   decided a second time here — the same reason the rest of this
///   module reads the plan rather than the model.
///
/// - **Inheritance.** A bitset that inherits takes the base's bitfields
///   at the low positions, in declaration order, before its own — the
///   same rule §7.4.1.1.5.1 states for an inherited structure. Fast DDS
///   is the only implementation that parses `bitset Derived : Base`
///   (Cyclone's `idlc` and RTI's `rtiddsgen` reject the keyword), and it
///   flattens the chain into one `std::bitset<N>` filled base-first. The
///   measurement is in `docs/refused-constructs.md`; the whole-chain
///   width comes from the planner through [`Ast::all_bitfields_of`].
fn write_bitset<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    let fields = ast.all_bitfields_of(type_);
    let object = contract::object(value, path)?;

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

    let declared = contract::bitfield_names(ast, &fields);
    contract::reject_unknown_keys(object, &declared, path)?;

    let mut bits: u64 = 0;
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
        // An anonymous field is padding: it moves the next field along
        // and carries no value of its own.
        if let Some(name) = field.name.as_ref() {
            let member_path = contract::join(path, name);
            let Some(member_value) = object.get(name.as_str()) else {
                return Err(ProbeError::Missing { path: member_path });
            };
            let raw = contract::integer(member_value, &member_path)?;
            s.seen.scalar(&member_path);
            // A value wider than its bitfield would overwrite the field
            // above it. Refused rather than masked: a mask puts a
            // different number on the wire than the sample asked for.
            let limit = 1i128 << width;
            if raw < 0 || raw >= limit {
                return Err(ProbeError::Mismatch {
                    path: member_path,
                    expected: format!("an unsigned value below {limit}, for bitfield<{width}>"),
                    found: format!("{raw}"),
                });
            }
            #[allow(clippy::cast_sign_loss)]
            let raw = raw as u64;
            bits |= raw << position;
        }
        position += width;
    }

    match anchor {
        1 => s.w.write_u8(u8::try_from(bits).unwrap_or(0)),
        2 => s.w.write_u16(u16::try_from(bits).unwrap_or(0)),
        4 => s.w.write_u32(u32::try_from(bits).unwrap_or(0)),
        8 => s.w.write_u64(bits),
        _ => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a bitset whose anchor is not one, two, four or eight bytes",
            });
        }
    }
    Ok(())
}

/// A map: the entries, in the shape §7.4.1.1.4 says a map has.
///
/// §7.4.1.1.4 defines a map as equivalent to a
/// `sequence<MapEntry_<key_type>_<value_type>>`, and §7.4.3.5.3 rules
/// (14), (15) and (16) write the key and the value of each entry
/// consecutively with no framing of their own. The rules are followed
/// rather than the equivalence on that point: read literally, the
/// `MapEntry` struct in §7.4.1.1.4 carries no extensibility annotation
/// and would default to appendable under §7.2.4.4, which would put a
/// DHEADER in front of every entry. No rule writes one.
///
/// Whether the map itself is delimited is the planner's decision and is
/// read off the plan, not recomputed: `zerodds_ast_layout` sets
/// `needs_dheader` from rule (14), which exempts a map only when both
/// halves are primitive, and rule (15), which delimits the rest under
/// XCDR2. All of Fast DDS, OpenDDS and this project write `map<long,
/// long>` as a bare count and the pairs, so the exemption is measured.
/// Where Fast DDS goes further and exempts `map<string, long>` as well
/// — its criterion is the value type alone — the broader reading is
/// taken here: a string key defeats the clause's own rationale for the
/// exemption, that the size can be determined "without iterating over
/// each element". OpenDDS reads it the same way.
///
/// Whether a delimited map also carries an entry count is the one thing
/// the specification says twice and differently, and the two
/// implementations that can be asked took one reading each; see
/// [`MapLengthPolicy`].
///
/// zerodds-lint: recursion-depth 64 — `crate::MAX_VALUE_DEPTH`. The
/// entries go back to `write_type`, whose aggregate arms count and refuse.
#[allow(clippy::too_many_arguments)]
fn write_map<V: Source>(
    s: &mut Sink,
    ast: &Ast,
    type_: Id<TypeNode>,
    key_type: &Ref<TypeNode>,
    value_type: &Ref<TypeNode>,
    bound: Bound,
    value: &V,
    frame: Frame,
    path: &str,
) -> Result<(), ProbeError> {
    let key_type = key_type.resolved().ok_or_else(|| ProbeError::NoLayout {
        detail: format!("{path}: the map key type is unresolved"),
    })?;
    let value_type = value_type.resolved().ok_or_else(|| ProbeError::NoLayout {
        detail: format!("{path}: the map value type is unresolved"),
    })?;

    // An array of pairs and not an object: a JSON object keys on a
    // string, and `map<long, float>` does not.
    let entries = contract::bounded_items(ast, value, bound, "entries", path)?;

    let mut deps = DepCollector::new();
    let layout = plan(ast, type_, frame.key, &mut deps).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let dheader = layout.needs_dheader.then(|| s.w.write_dheader());
    if dheader.is_none() || frame.map_length == MapLengthPolicy::SequenceEquivalent {
        #[allow(clippy::cast_possible_truncation)]
        s.w.write_u32(entries.len() as u32);
    }

    for index in map_entry_order(ast, type_, key_type, entries, frame, path)? {
        let entry = &entries[index];
        let entry_path = format!("{path}[{index}]");
        let (entry_key, entry_value) = contract::map_entry(entry, &entry_path)?;
        write_type(
            s,
            ast,
            key_type,
            entry_key,
            frame.in_collection(),
            &contract::join(&entry_path, "key"),
        )?;
        write_type(
            s,
            ast,
            value_type,
            entry_value,
            frame.in_collection(),
            &contract::join(&entry_path, "value"),
        )?;
    }

    if let Some(dheader) = dheader {
        s.w.try_finish_dheader(dheader)
            .map_err(|error| refused(path, error))?;
    }
    Ok(())
}

/// The indices of a map's entries, in the order they go on the wire.
///
/// `0..len` under [`MapEntryOrder::Declaration`], which is every
/// dialect but Fast DDS and OpenDDS; a permutation sorted by the key's
/// own value under [`MapEntryOrder::AscendingKey`], which is those two.
///
/// [`zerodds_ast_layout::map_key_ordering`] is the crossing and this is
/// its only caller in the Rust column. Where the policy asks for
/// ascending keys and the key type has no order, the answer is a
/// **refusal** and not the declaration order: a setting that silently
/// did nothing for one key type and something for another would be a
/// flag that reports success and changed no octet.
fn map_entry_order<V: Source>(
    ast: &Ast,
    type_: Id<TypeNode>,
    key_type: Id<TypeNode>,
    entries: &[V],
    frame: Frame,
    path: &str,
) -> Result<Vec<usize>, ProbeError> {
    if frame.key.map_entries.is_default() {
        return Ok((0..entries.len()).collect());
    }
    let Some(ordering) = map_key_ordering(ast, type_, frame.key.map_entries) else {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a map whose key type has no value order, under --map-entries ascending-key",
        });
    };
    let mut keyed = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let entry_path = format!("{path}[{index}]");
        let (entry_key, _) = contract::map_entry(entry, &entry_path)?;
        let sort_key = map_sort_key(
            ast,
            key_type,
            ordering,
            entry_key,
            &contract::join(&entry_path, "key"),
        )?;
        keyed.push((sort_key, index));
    }
    // By the key and then by the declared position, so that a sample
    // with two entries carrying the same key keeps their order rather
    // than taking whichever one the sort happened to move.
    keyed.sort();
    Ok(keyed.into_iter().map(|(_, index)| index).collect())
}

/// One map key reduced to something with an order.
///
/// Three shapes and not one: an integer order is not an octet order —
/// `-1234567` encodes as `ffed2979` and sorts first by value and last by
/// octet — and a string's order is over its octets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum MapSortKey {
    /// Every scalar key: the value, signed or unsigned as the key type
    /// says, widened to `i128` so that both fit one comparison.
    Integer(i128),
    /// A `string` or `wstring` key: the octets of its UTF-8, compared
    /// unsigned. UTF-8 is order preserving, so this is also the
    /// code-point order a `std::wstring` compares in.
    CodeUnits(Vec<u8>),
}

/// The sort key of one map key value.
fn map_sort_key<V: Source>(
    ast: &Ast,
    key_type: Id<TypeNode>,
    ordering: MapKeyOrdering,
    value: &V,
    path: &str,
) -> Result<MapSortKey, ProbeError> {
    let resolved = contract::resolve_alias(ast, key_type);
    let kind = ast.type_(resolved).map(|node| node.kind.clone()).ok();
    match ordering {
        MapKeyOrdering::EnumeratorValue => {
            let (_, declared) = contract::enumerator(ast, resolved, value, path)?;
            Ok(MapSortKey::Integer(declared))
        }
        MapKeyOrdering::CodeUnits => Ok(MapSortKey::CodeUnits(
            contract::text(value, path)?.as_bytes().to_vec(),
        )),
        MapKeyOrdering::SignedInteger | MapKeyOrdering::UnsignedInteger => {
            let raw = match kind {
                Some(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    i128::from(contract::boolean(value, path)?)
                }
                Some(TypeKind::Primitive(kind @ (PrimitiveKind::Char | PrimitiveKind::WChar))) => {
                    i128::from(u32::from(contract::character_for(kind, value, path)?))
                }
                Some(TypeKind::Primitive(kind)) => contract::integer_for(kind, value, path)?,
                _ => contract::integer(value, path)?,
            };
            // An IDL `char` is a C++ `char`, and `char` is signed on the
            // host the corpus was recorded on. Written out here rather
            // than left to the column's own compiler, so that the three
            // columns agree on a host whose `char` is unsigned.
            let signed = ordering == MapKeyOrdering::SignedInteger
                && matches!(kind, Some(TypeKind::Primitive(PrimitiveKind::Char)))
                && raw >= 0x80;
            Ok(MapSortKey::Integer(if signed { raw - 0x100 } else { raw }))
        }
    }
}

/// `fixed<P, S>`: the IDL fixed-point decimal, as packed BCD.
///
/// # The specification does not say
///
/// DDS-XTypes 1.3 does not carry the type. §7.2.2 has no fixed-point
/// kind, the IDL-to-type-system table of §7.3.1 does not map one, and
/// §7.4.3.5.3 has no rule whose left-hand side matches. Searching the
/// 1.3 text for "fixed" returns only prose about fixed-*size*
/// collections. A DDS peer is therefore not required to read this at
/// all, and the tests mark the bytes as unverified against any DDS
/// vendor.
///
/// It is nonetheless measured. `fixed<P,S>` came from CORBA and two
/// CORBA ORBs still write it: omniORB 4.3.3 produces `12345c` for a
/// `fixed<5,2>` holding `123.45`, byte for byte what this function
/// writes, over nine shapes covering the sign nibble, the leading zero
/// of an even digit count, a zero value, thirty-one digits and
/// alignment one — and the same bytes in both byte orders. JacORB 3.9
/// agrees on eight of the nine, differing only where Java's
/// `BigDecimal` has already lost the declared digit count. The run is
/// in `crates/ast-layout/tests/fixtures/vendor_bytes.md`, "Where the
/// three cannot answer".
///
/// The form written here is CORBA's, [CDR] §9.3.2.8, which is where an
/// IDL `fixed` came from and the only wire definition it has: `P` digit
/// nibbles most-significant first, then a sign nibble — `0xc` for
/// positive or zero, `0xd` for negative — with a leading zero nibble
/// prepended when `P + 1` is odd so the nibbles pair up. That is
/// `ceil((P + 1) / 2)` bytes, and it is the extent
/// `zerodds_ast_layout` plans for the type, at alignment one: the
/// digits are bytes and no padding precedes them.
///
/// The JSON is a decimal string, because a JSON number is a double and
/// a `fixed<31, 2>` is exactly the case a double cannot carry.
fn write_fixed<V: Source>(
    s: &mut Sink,
    digits: u8,
    scale: u8,
    policy: FixedDigitCount,
    value: &V,
    path: &str,
) -> Result<(), ProbeError> {
    if digits == 0 {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a fixed with no digits, which can hold no value",
        });
    }
    // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    // `fixed<31, S>` is the widest declaration IDL admits. A wider one
    // has no packing defined for it, so it is refused rather than packed
    // into octets no reader can be told the width of. The C runtime
    // makes the same check on both sides.
    if digits > 31 {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "a fixed of at most 31 digits".to_string(),
            found: format!("fixed<{digits}, {scale}>"),
        });
    }
    if scale > digits {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "a scale no larger than the digit count".to_string(),
            found: format!("fixed<{digits}, {scale}>"),
        });
    }
    let text = contract::text(value, path).map_err(|_| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "a decimal, as a string — `\"123.45\"`".to_string(),
        found: contract::kind_of(value).to_string(),
    })?;
    s.seen.scalar(path);
    // The value's rule and the packing both live in `zerodds-wire`, which is
    // where the twenty-first column reaches them too. What stays here is the
    // path: `Writer` says what it expected and at which byte, and a refusal
    // over a fifty-member document that cannot say which member it was is one
    // nobody can act on.
    s.w.write_fixed(text, digits, scale, fixed_digits(policy))
        .map_err(|error| refused(path, error))
}

/// The layout planner's digit-count axis as the byte layer's copy of it.
///
/// `zerodds-wire` does not depend on `zerodds-ast-layout` and must not — a
/// byte layer that needs the layout planner is not a byte layer — so the axis
/// exists twice and this is the crossing. `tests/fixed_digits_agree.rs` holds
/// the two halves against each other over every argument either can take.
pub(crate) const fn fixed_digits(policy: FixedDigitCount) -> zerodds_wire::FixedDigits {
    match policy {
        FixedDigitCount::Declared => zerodds_wire::FixedDigits::Declared,
        FixedDigitCount::ValuePrecision => zerodds_wire::FixedDigits::ValuePrecision,
        FixedDigitCount::ValueStringDigits => zerodds_wire::FixedDigits::ValueStringDigits,
    }
}

/// A value the runtime refused, as a probe error that names the path.
///
/// `Writer` says what it expected and at which byte; what it cannot know
/// is where in the sample the value came from, and a refusal without the
/// path is one nobody can act on over a fifty-member document.
fn refused(path: &str, error: zerodds_wire::WireError) -> ProbeError {
    let expected = match error {
        zerodds_wire::WireError::OutOfRange { expected, .. } => expected,
        other => format!("{other}"),
    };
    ProbeError::Mismatch {
        path: path.to_string(),
        expected,
        found: "the value the runtime refused".to_string(),
    }
}

/// One primitive, with the range check the dialect makes rather than
/// the one `try_from` happens to make.
///
/// Every narrowing below is exact: `contract::integer_for` has already
/// refused a value outside the primitive's range, so nothing here can
/// wrap.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn write_primitive<V: Source>(
    s: &mut Sink,
    kind: PrimitiveKind,
    value: &V,
    position: WcharPosition,
    path: &str,
) -> Result<(), ProbeError> {
    match kind {
        PrimitiveKind::Boolean => s.w.write_bool(contract::boolean(value, path)?),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => {
            s.w.write_u8(contract::integer_for(kind, value, path)? as u8);
        }
        PrimitiveKind::Int8 => {
            s.w.write_i8(contract::integer_for(kind, value, path)? as i8);
        }
        // `character_for` has already refused a code point the width
        // cannot carry, and it did so with the path in hand. The runtime
        // refuses the same values, so nothing reaches the `?` today; it
        // is there because the runtime is the last thing between a value
        // and the wire, and a day when the two stop agreeing has to end
        // in a refusal rather than in a byte nobody wrote.
        PrimitiveKind::Char => {
            let character = contract::character_for(kind, value, path)?;
            s.w.write_char(character)
                .map_err(|error| refused(path, error))?;
        }
        PrimitiveKind::WChar => {
            let character = contract::character_for(kind, value, path)?;
            s.w.write_wchar(character, position)
                .map_err(|error| refused(path, error))?;
        }
        PrimitiveKind::Int16 => {
            s.w.write_i16(contract::integer_for(kind, value, path)? as i16);
        }
        PrimitiveKind::UInt16 => {
            s.w.write_u16(contract::integer_for(kind, value, path)? as u16);
        }
        PrimitiveKind::Int32 => {
            s.w.write_i32(contract::integer_for(kind, value, path)? as i32);
        }
        PrimitiveKind::UInt32 => {
            s.w.write_u32(contract::integer_for(kind, value, path)? as u32);
        }
        PrimitiveKind::Int64 => {
            s.w.write_i64(contract::integer_for(kind, value, path)? as i64);
        }
        PrimitiveKind::UInt64 => {
            s.w.write_u64(contract::integer_for(kind, value, path)? as u64);
        }
        PrimitiveKind::Float32 => s.w.write_f32(contract::float(value, path)? as f32),
        PrimitiveKind::Float64 => s.w.write_f64(contract::float(value, path)?),
        PrimitiveKind::Float128 => s.w.write_f128(contract::float128(value, path)?),
    }
    s.seen.scalar(path);
    Ok(())
}

const fn length_code(lc: LengthCode) -> u8 {
    match lc {
        LengthCode::Lc0 => 0,
        LengthCode::Lc1 => 1,
        LengthCode::Lc2 => 2,
        LengthCode::Lc3 => 3,
        LengthCode::Lc4 => 4,
        LengthCode::Lc5 => 5,
        LengthCode::Lc6 => 6,
        LengthCode::Lc7 => 7,
    }
}
