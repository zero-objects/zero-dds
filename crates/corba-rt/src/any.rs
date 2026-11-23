// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL `any` (CORBA 3.3 part 2 §15.3.7) and IDL `wstring` (§15.3.2.7) —
//! the self-describing value a CORBA service passes when the type is not
//! fixed by the operation signature.
//!
//! An `any` on the wire is a full [`TypeCode`]
//! followed by the value in its own CDR representation, which is what
//! makes it readable by omniORB, TAO and JacORB for structured content —
//! sequence, struct, enum, a nested `any` — and not only for scalars.
//!
//! ## Where this lives
//!
//! In `zerodds-corba-rt` and not in `zerodds-wire`, for the reason
//! [`crate::type_code`] gives: an `any` is a CORBA type and `zerodds-wire`
//! is a byte runtime.
//!
//! ## The `wstring` byte-order mark is a coordinate, not a global
//!
//! Whether a GIOP `wstring` carries a leading `0xFEFF` and in what order
//! its code units go out is underspecified by §15.3.1.6, and the ORBs
//! disagree. It used to be a process-global `AtomicBool`
//! (`set_corba_wstring_bom`) — a setting every call site assumed rather
//! than received, so two peers in one process could not be served at
//! once and no call site said which form it wanted.
//!
//! It is now [`zerodds_wire::WstringUnitOrder`], carried by the `Writer`
//! and `Reader` themselves and set from the layout key like every other
//! dialect-level answer. [`WString::encode`] asks the writer it was given;
//! nothing here reads a global.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, FixedDigits, Reader, WireError, Writer, WstringUnitOrder};

use crate::object::ObjectReference;
use crate::type_code::{TypeCode, UnionLabel, UnionMember, value_modifier};

/// The UTF-16 byte-order mark (§15.3.1.6). A reader with the reverse
/// order sees the mirrored `0xFFFE` and swaps.
const UTF16_BOM: u16 = 0xFEFF;

/// Whether wide code units go out big-endian under `order` in a stream of
/// `stream` order.
const fn units_are_big(order: WstringUnitOrder, stream: Endianness) -> bool {
    match order {
        WstringUnitOrder::Stream => matches!(stream, Endianness::Big),
        WstringUnitOrder::BigEndianAlways => true,
        // Both write the units little-endian; only the first prepends a
        // mark. See `zerodds_wire::WstringUnitOrder`, where the
        // measurement behind each variant is recorded.
        WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped => false,
    }
}

/// An IDL `wstring`.
///
/// Holds Unicode text as a Rust `String`, but the **wire form** differs
/// from `string`: GIOP 1.2 (§15.3.2.7) writes a `uint32` length **in
/// octets** — not characters, and not counting a terminator — followed by
/// the UTF-16 code units, with no NUL at the end. That is what makes
/// `wstring` distinct from the UTF-8 `string` and readable by an ORB
/// whose transmission code set is UTF-16, which is the default for
/// omniORB, TAO and JacORB.
#[derive(Debug, Clone, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
pub struct WString(pub String);

impl WString {
    /// Borrows the text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Writes the GIOP 1.2 `wstring`.
    ///
    /// The byte-order mark and the code units' order come from the
    /// writer's [`zerodds_wire::Writer::wstring_unit_order`], which is
    /// set from the layout key. An empty `wstring` is a length of zero
    /// under every setting, mark included — there is nothing to mark.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] when the octet length exceeds `u32`.
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        let order = w.wstring_unit_order();
        let units = self.0.encode_utf16().count();
        if units == 0 {
            w.write_u32(0);
            return Ok(());
        }
        let with_bom = matches!(order, WstringUnitOrder::ByteOrderMark);
        let total_units = units + usize::from(with_bom);
        let octets = total_units
            .checked_mul(2)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| WireError::OutOfRange {
                at: w.position(),
                expected: format!("a wstring of at most u32::MAX octets, not {total_units} units"),
            })?;
        w.write_u32(octets);
        let big = units_are_big(order, w.endianness());
        // The units go out raw: their order is this axis's answer and not
        // the stream's, which is the whole point of the setting.
        if with_bom {
            write_unit(w, UTF16_BOM, big);
        }
        for unit in self.0.encode_utf16() {
            write_unit(w, unit, big);
        }
        Ok(())
    }

    /// Reads a GIOP 1.2 `wstring`.
    ///
    /// Liberal on the way in: a leading byte-order mark settles the
    /// units' order whatever the reader is configured for, and when there
    /// is none the reader's own
    /// [`zerodds_wire::Reader::wstring_unit_order`] answers — which for
    /// the default `Stream` is the message's order, the rule §15.3.1.6
    /// gives. So omniORB's marked form and JacORB's unmarked form both
    /// read, and so does what [`WString::encode`] writes under any
    /// setting.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for an odd octet count or units that are
    /// not valid UTF-16, [`WireError::ImpossibleLength`] for a length
    /// past the end of the stream.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let at = r.position();
        let octets = r.read_u32()? as usize;
        if !octets.is_multiple_of(2) {
            return Err(WireError::OutOfRange {
                at,
                expected: format!("an even wstring octet count, not {octets}"),
            });
        }
        if octets > r.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: octets,
                available: r.remaining(),
            });
        }
        if octets == 0 {
            return Ok(Self(String::new()));
        }
        let fallback_big = units_are_big(r.wstring_unit_order(), r.endianness());
        let bytes = r.read_bytes(octets)?;
        let (start, big) = match (bytes[0], bytes[1]) {
            (0xFE, 0xFF) => (2, true),
            (0xFF, 0xFE) => (2, false),
            _ => (0, fallback_big),
        };
        let mut units = Vec::with_capacity((octets - start) / 2);
        let mut idx = start;
        while idx + 1 < octets {
            let pair = [bytes[idx], bytes[idx + 1]];
            units.push(if big {
                u16::from_be_bytes(pair)
            } else {
                u16::from_le_bytes(pair)
            });
            idx += 2;
        }
        let text = String::from_utf16(&units).map_err(|_| WireError::OutOfRange {
            at,
            expected: String::from("a wstring whose code units are valid UTF-16"),
        })?;
        Ok(Self(text))
    }
}

/// One wide code unit, raw, in the order this `wstring`'s axis names.
fn write_unit(w: &mut Writer, unit: u16, big: bool) {
    if big {
        w.write_bytes(&unit.to_be_bytes());
    } else {
        w.write_bytes(&unit.to_le_bytes());
    }
}

impl From<&str> for WString {
    fn from(s: &str) -> Self {
        Self(String::from(s))
    }
}

impl From<String> for WString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// What a [`CorbaAny`] can carry: a value of every `TCKind` of CORBA
/// 3.3 Part 2 Table 9.2 that has a value encoding.
///
/// Struct, exception, sequence and enum values carry enough type
/// information that their [`TypeCode`] is derived from the value; a
/// union, a reference, a value type and an abstract interface carry
/// their `TypeCode`, because the value alone does not name the other
/// cases, the interface or the state members. `tk_native` and
/// `tk_local_interface` have no value encoding (a local object is not
/// marshalled) and are refused.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AnyValue {
    /// `tk_null`.
    #[default]
    Null,
    /// `tk_boolean`.
    Boolean(bool),
    /// `tk_octet`.
    Octet(u8),
    /// `tk_char`.
    Char(u8),
    /// `tk_short`.
    Short(i16),
    /// `tk_ushort`.
    UShort(u16),
    /// `tk_long`.
    Long(i32),
    /// `tk_ulong`.
    ULong(u32),
    /// `tk_longlong`.
    LongLong(i64),
    /// `tk_ulonglong`.
    ULongLong(u64),
    /// `tk_float`.
    Float(f32),
    /// `tk_double`.
    Double(f64),
    /// `tk_longdouble`: the IEEE 754 binary128 bit pattern, most
    /// significant octet first.
    LongDouble([u8; 16]),
    /// `tk_wchar`, as the bare transmission code unit.
    WChar(u16),
    /// `tk_string`.
    Str(String),
    /// `tk_wstring`.
    WStr(WString),
    /// `tk_fixed`: digits, scale and the decimal text.
    Fixed {
        /// Total number of digits (1 to 31).
        digits: u16,
        /// Digits after the point.
        scale: i16,
        /// The value as decimal text, e.g. `-12.50`.
        value: String,
    },
    /// `tk_Principal`: `sequence<octet>` (§9.3.5.3).
    Principal(Vec<u8>),
    /// `tk_TypeCode`: a `TypeCode` as a value.
    TypeCodeValue(TypeCode),
    /// `tk_sequence`: the element `TypeCode` — needed even when there are
    /// no elements to read it off — and the items.
    Seq {
        /// The elements' `TypeCode`.
        element: TypeCode,
        /// The element values.
        items: Vec<AnyValue>,
    },
    /// `tk_array`: one dimension, its length the number of items; a
    /// multi-dimensional array nests arrays.
    Array {
        /// The elements' `TypeCode`.
        element: TypeCode,
        /// The element values.
        items: Vec<AnyValue>,
    },
    /// `tk_struct`: repository id, name, ordered `(member_name, value)`.
    Struct {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The struct's name.
        name: String,
        /// The members in declaration order.
        members: Vec<(String, AnyValue)>,
    },
    /// `tk_except`: as a struct; on the wire the repository id precedes
    /// the members (§9.3.5.5).
    Except {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The exception's name.
        name: String,
        /// The members in declaration order.
        members: Vec<(String, AnyValue)>,
    },
    /// `tk_union`: the union's `TypeCode`, the discriminator and the
    /// value of the case it selects (`None` when it selects no case).
    Union {
        /// The `tk_union` (or an alias of one).
        type_code: TypeCode,
        /// The discriminator value.
        discriminator: UnionLabel,
        /// The selected member's value.
        value: Option<Box<AnyValue>>,
    },
    /// `tk_enum`: repository id, name, the ordinal, the enumerator names.
    Enum {
        /// `IDL:…:1.0`.
        repo_id: String,
        /// The enum's name.
        name: String,
        /// The ordinal, an index into `members`.
        value: u32,
        /// The enumerator names.
        members: Vec<String>,
    },
    /// `tk_objref`, `tk_component`, `tk_home`: the `TypeCode` and the
    /// reference, `None` for nil.
    ObjRef {
        /// The `TypeCode` of the interface.
        type_code: TypeCode,
        /// The reference.
        reference: Option<ObjectReference>,
    },
    /// `tk_value`, `tk_event`, `tk_value_box`: the formal `TypeCode` and
    /// the value (§9.3.4).
    Value {
        /// The formal type.
        type_code: TypeCode,
        /// Null, a value, or a value shared with an earlier one.
        value: ValueRef,
    },
    /// `tk_abstract_interface`: an object reference or a value (§9.3.7).
    Abstract {
        /// The `tk_abstract_interface`.
        type_code: TypeCode,
        /// What the abstract interface holds.
        value: AbstractValue,
    },
    /// `tk_any`: a nested `any`.
    Any(Box<CorbaAny>),
}

/// A value-type slot (§9.3.4): null, a value, or a reference to a value
/// encoded earlier in the same `any`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ValueRef {
    /// The null value (`<null_tag>`).
    #[default]
    Null,
    /// A value with its state.
    Instance(Box<ValueInstance>),
    /// The `n`-th value instance of this `any` (0-based, in the order in
    /// which the instances start), written as an indirection; this is how
    /// a shared or cyclic value graph is represented.
    Shared(u32),
}

/// A value instance: its most derived type and its state.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueInstance {
    /// The most derived type the value is known as: a `tk_value`,
    /// `tk_event` or `tk_value_box`. After truncation (§9.3.4.1) the base
    /// type it was truncated to.
    pub type_code: TypeCode,
    /// The state: the members of the concrete bases first, then those of
    /// the type, each in declaration order; for a value box the boxed
    /// value with an empty name.
    pub members: Vec<(String, AnyValue)>,
    /// The codebase URL of the value tag (§9.3.4.1, tag bit `0x01`):
    /// where the receiver may find the implementation of the type.
    pub codebase: Option<String>,
}

/// What an abstract interface holds (§9.3.7).
#[derive(Debug, Clone, PartialEq)]
pub enum AbstractValue {
    /// An object reference, `None` for nil (discriminator TRUE).
    Reference(Option<ObjectReference>),
    /// A value (discriminator FALSE); a nil abstract interface whose
    /// kind is not known is a null value.
    Value(ValueRef),
}

/// The value types an `any` decoder knows besides the ones its
/// `TypeCode`s name, by repository id: what a value inside an abstract
/// interface, or a value of a type derived from the formal one, is read
/// as.
pub type KnownValueTypes = BTreeMap<String, TypeCode>;

/// Hard cap on how deep the parts of one value nest while it is decoded:
/// struct and exception members, union branches, sequence and array
/// elements, aliases, recursive members, value type state and the `any`s
/// inside it, each part one level.
///
/// [`crate::type_code::MAX_TYPECODE_DEPTH`] bounds one `TypeCode`, not
/// the value it describes: a recursive `TypeCode` (`struct Node {
/// sequence<Node> kids; }`) or an `any` inside an `any` lets a value from
/// any peer nest without end, and each level is a stack frame of the
/// decoder. A deeper value is refused with a [`WireError`] instead of
/// overflowing the stack of the thread that reads it.
pub const MAX_VALUE_DEPTH: u32 = 2 * crate::type_code::MAX_TYPECODE_DEPTH;

/// The elements a sequence or an array reserves room for before it reads
/// them; more grow the vector as they are read.
///
/// A count from the wire is checked against the octets that remain, but
/// an element of `tk_null`, `tk_void` or an empty struct takes no octets:
/// the count is not a promise of octets, and room for it is not reserved
/// in advance.
const RESERVED_ELEMENTS: usize = 256;

/// The first value tag (§9.3.4.1); tags run to `0x7fff_ffff`.
const VALUE_TAG_BASE: u32 = 0x7fff_ff00;
/// `<value_tag>` bit: a codebase URL follows.
const VT_CODEBASE: u32 = 0x01;
/// `<value_tag>` bits: a single repository id.
const VT_SINGLE_ID: u32 = 0x02;
/// `<value_tag>` bits: a list of repository ids.
const VT_ID_LIST: u32 = 0x06;
/// `<value_tag>` bit: the state is chunked.
const VT_CHUNKED: u32 = 0x08;
/// The last value tag.
const VALUE_TAG_LAST: u32 = 0x7fff_ffff;
/// `<indirection_tag>`.
const INDIRECTION_TAG: u32 = 0xffff_ffff;

/// Whether `t` is a `<value_tag>` (§9.3.4.1: `0x7fffff00` to
/// `0x7fffffff`); an end tag is negative and is not one.
const fn is_value_tag(t: u32) -> bool {
    t >= VALUE_TAG_BASE && t <= VALUE_TAG_LAST
}

fn refused(at: usize, expected: impl Into<String>) -> WireError {
    WireError::OutOfRange {
        at,
        expected: expected.into(),
    }
}

/// The kind of a value, to check it against a `TypeCode`; a recursive
/// marker matches a value whose type has its repository id.
fn conforms(v: &AnyValue, tc: &TypeCode) -> bool {
    let tc = tc.resolved();
    if let TypeCode::Recursive { repo_id } = tc {
        return v.type_code().resolved().repo_id() == Some(repo_id.as_str());
    }
    matches!(
        (v, tc),
        (AnyValue::Null, TypeCode::Null | TypeCode::Void)
            | (AnyValue::Boolean(_), TypeCode::Boolean)
            | (AnyValue::Octet(_), TypeCode::Octet)
            | (AnyValue::Char(_), TypeCode::Char)
            | (AnyValue::Short(_), TypeCode::Short)
            | (AnyValue::UShort(_), TypeCode::UShort)
            | (AnyValue::Long(_), TypeCode::Long)
            | (AnyValue::ULong(_), TypeCode::ULong)
            | (AnyValue::LongLong(_), TypeCode::LongLong)
            | (AnyValue::ULongLong(_), TypeCode::ULongLong)
            | (AnyValue::Float(_), TypeCode::Float)
            | (AnyValue::Double(_), TypeCode::Double)
            | (AnyValue::LongDouble(_), TypeCode::LongDouble)
            | (AnyValue::WChar(_), TypeCode::WChar)
            | (AnyValue::Str(_), TypeCode::String(_))
            | (AnyValue::WStr(_), TypeCode::WString(_))
            | (AnyValue::Fixed { .. }, TypeCode::Fixed { .. })
            | (AnyValue::Principal(_), TypeCode::Principal)
            | (AnyValue::TypeCodeValue(_), TypeCode::TypeCodeTc)
            | (AnyValue::Seq { .. }, TypeCode::Sequence { .. })
            | (AnyValue::Array { .. }, TypeCode::Array { .. })
            | (
                AnyValue::Struct { .. },
                TypeCode::Struct {
                    is_except: false,
                    ..
                }
            )
            | (
                AnyValue::Except { .. },
                TypeCode::Struct {
                    is_except: true,
                    ..
                }
            )
            | (AnyValue::Union { .. }, TypeCode::Union { .. })
            | (AnyValue::Enum { .. }, TypeCode::Enum { .. })
            | (
                AnyValue::ObjRef { .. },
                TypeCode::ObjRef { .. } | TypeCode::Component { .. } | TypeCode::Home { .. }
            )
            | (
                AnyValue::Value { .. },
                TypeCode::Value { .. } | TypeCode::ValueBox { .. }
            )
            | (
                AnyValue::Abstract { .. },
                TypeCode::AbstractInterface { .. }
            )
            | (AnyValue::Any(_), TypeCode::Any)
    )
}

/// The state members of a value type: those of its concrete bases first
/// (§9.3.4: "beginning with the highest possible base type").
fn value_state_members(tc: &TypeCode, at: usize) -> Result<Vec<(String, TypeCode)>, WireError> {
    let mut chain = Vec::new();
    let mut cur = tc.resolved();
    for _ in 0..crate::type_code::MAX_TYPECODE_DEPTH {
        match cur {
            TypeCode::Value {
                modifier,
                concrete_base,
                members,
                ..
            } => {
                if *modifier == value_modifier::VM_CUSTOM {
                    return Err(refused(
                        at,
                        "a value type whose state its TypeCode describes, not a custom-marshalled one",
                    ));
                }
                chain.push(members);
                cur = concrete_base.resolved();
            }
            TypeCode::ValueBox { content, .. } => {
                return Ok(vec![(String::new(), (**content).clone())]);
            }
            TypeCode::Null => {
                let mut out = Vec::new();
                for members in chain.into_iter().rev() {
                    out.extend(
                        members
                            .iter()
                            .map(|m| (m.name.clone(), m.member_type.clone())),
                    );
                }
                return Ok(out);
            }
            _ => return Err(refused(at, "a value type or value box TypeCode")),
        }
    }
    Err(refused(
        at,
        "a value type base chain within the TypeCode depth limit",
    ))
}

/// The repository ids a value of type `tc` can be truncated to, most
/// derived first: the type, then its concrete bases as long as each
/// derived type is truncatable (§9.3.4.1).
fn truncation_ids(tc: &TypeCode) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = tc.resolved();
    while let TypeCode::Value {
        repo_id,
        modifier,
        concrete_base,
        ..
    } = cur
    {
        out.push(repo_id.clone());
        if *modifier != value_modifier::VM_TRUNCATABLE {
            break;
        }
        cur = concrete_base.resolved();
    }
    out
}

/// The encoder's value state: where each value instance starts (for
/// [`ValueRef::Shared`]) and the open chunk of a chunked value.
#[derive(Default)]
struct Enc {
    values: Vec<usize>,
    depth: i32,
    chunk: Option<(zerodds_wire::writer::DHeader, usize)>,
}

impl Enc {
    /// Before non-value data inside a chunked value: open a chunk.
    fn data(&mut self, w: &mut Writer) {
        if self.depth > 0 && self.chunk.is_none() {
            let header = w.write_dheader();
            self.chunk = Some((header, w.position()));
        }
    }

    /// Ends the open chunk with its size.
    fn close(&mut self, w: &mut Writer) -> Result<(), WireError> {
        if let Some((header, start)) = self.chunk.take() {
            if w.position() - start >= VALUE_TAG_BASE as usize {
                return Err(refused(
                    w.position(),
                    "a value chunk below 0x7fffff00 octets",
                ));
            }
            w.try_finish_dheader(header)?;
        }
        Ok(())
    }
}

/// The decoder's state: the value types it knows, where each value
/// instance and each repository id started, and the chunk state of the
/// chunked values being read (§9.3.4.6).
struct Dec<'k, 'b> {
    known: &'k KnownValueTypes,
    /// The value instances read so far: the position of each value tag
    /// and the index of the value in the order they were read, which a
    /// shared value names.
    values: BTreeMap<usize, u32>,
    /// The values nested in the state a truncation skipped (§9.3.4.1),
    /// by the position of their value tag: a reader at the tag and the
    /// nesting depth of the chunked value around it. §9.3.4.6 lets a
    /// later indirection point back to one of them; it is read then.
    skipped: BTreeMap<usize, (Reader<'b>, i32)>,
    strings: BTreeMap<usize, String>,
    lists: BTreeMap<usize, Vec<String>>,
    /// The end of the current chunk while inside a chunked value; equal
    /// to the position when it is used up.
    chunk_end: Option<usize>,
    /// The nesting depth of chunked values.
    depth: i32,
    /// An end tag already read that ended the values down to this depth.
    ended: Option<i32>,
    /// The struct, union and value types being read around the current
    /// position, innermost last: what a recursive `TypeCode` resolves to.
    outer: Vec<TypeCode>,
    /// The parts of the value being read around the current position,
    /// capped by [`MAX_VALUE_DEPTH`].
    nesting: u32,
    /// The position the decode started at.
    start: usize,
    /// The parts read so far that took no octets ([`Dec::empty_part`]).
    empty_parts: usize,
}

impl Dec<'_, '_> {
    /// Counts a part of the value that took no octets: a value of
    /// `tk_null` or `tk_void`, of an empty struct, of an array of length
    /// 0, or of a struct or array made of such values only. A decode reads
    /// at most one such part per octet it has read since it started, and
    /// one more (a value of a type without octets on its own).
    ///
    /// The depth of a value is bounded by [`MAX_VALUE_DEPTH`], its breadth
    /// by the octets that remain for every count; parts without octets
    /// meet neither bound, and nested sequences or arrays of them would
    /// multiply their counts level by level: a few octets would allocate
    /// memory without end. A decode that reaches more is refused with a
    /// [`WireError`].
    fn empty_part(&mut self, r: &Reader<'_>) -> Result<(), WireError> {
        self.empty_parts += 1;
        let read = r.position().saturating_sub(self.start);
        if self.empty_parts > read.saturating_add(1) {
            return Err(refused(
                r.position(),
                "at most one value part without octets per octet read",
            ));
        }
        Ok(())
    }

    /// Before non-value data: inside a chunked value whose chunk is used
    /// up, the next chunk starts.
    fn data(&mut self, r: &mut Reader<'_>) -> Result<(), WireError> {
        let Some(end) = self.chunk_end else {
            return Ok(());
        };
        if r.position() < end {
            return Ok(());
        }
        if r.position() > end {
            return Err(refused(
                r.position(),
                "value data that does not run past its chunk",
            ));
        }
        r.align_to(4)?;
        let at = r.position();
        let size = r.read_u32()?;
        if size == 0 || size >= VALUE_TAG_BASE {
            return Err(refused(at, "a chunk size before more value data"));
        }
        self.chunk_end = Some(r.position() + size as usize);
        Ok(())
    }

    /// A string or an indirection to one read before (§9.3.4.3).
    fn string_or_indirection(&mut self, r: &mut Reader<'_>) -> Result<String, WireError> {
        r.align_to(4)?;
        let at = r.position();
        let first = r.read_u32()?;
        if first == INDIRECTION_TAG {
            let field = r.position();
            let target = indirection_target(field, r.read_i32()?)?;
            return self.strings.get(&target).cloned().ok_or_else(|| {
                refused(
                    field,
                    "an indirection to a repository id or URL read before",
                )
            });
        }
        // Not an indirection: re-read as a string from its length.
        let len = first as usize;
        if len == 0 || len > r.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: len,
                available: r.remaining(),
            });
        }
        let bytes = r.read_bytes(len)?;
        let text = core::str::from_utf8(&bytes[..len - 1])
            .map_err(|_| refused(at, "a repository id in UTF-8"))?;
        self.strings.insert(at, String::from(text));
        Ok(String::from(text))
    }
}

/// The target of an indirection whose offset field is at `field`.
fn indirection_target(field: usize, offset: i32) -> Result<usize, WireError> {
    if offset >= -4 {
        return Err(refused(field, "a negative indirection offset below -4"));
    }
    i64::try_from(field)
        .ok()
        .map(|f| f + i64::from(offset))
        .and_then(|t| usize::try_from(t).ok())
        .ok_or_else(|| refused(field, "an indirection inside the stream"))
}

impl AnyValue {
    /// The full [`TypeCode`] (§9.3.5) this value describes.
    #[must_use]
    pub fn type_code(&self) -> TypeCode {
        match self {
            Self::Null => TypeCode::Null,
            Self::Boolean(_) => TypeCode::Boolean,
            Self::Octet(_) => TypeCode::Octet,
            Self::Char(_) => TypeCode::Char,
            Self::Short(_) => TypeCode::Short,
            Self::UShort(_) => TypeCode::UShort,
            Self::Long(_) => TypeCode::Long,
            Self::ULong(_) => TypeCode::ULong,
            Self::LongLong(_) => TypeCode::LongLong,
            Self::ULongLong(_) => TypeCode::ULongLong,
            Self::Float(_) => TypeCode::Float,
            Self::Double(_) => TypeCode::Double,
            Self::LongDouble(_) => TypeCode::LongDouble,
            Self::WChar(_) => TypeCode::WChar,
            Self::Str(_) => TypeCode::String(0),
            Self::WStr(_) => TypeCode::WString(0),
            Self::Fixed { digits, scale, .. } => TypeCode::Fixed {
                digits: *digits,
                scale: *scale,
            },
            Self::Principal(_) => TypeCode::Principal,
            Self::TypeCodeValue(_) => TypeCode::TypeCodeTc,
            Self::Seq { element, .. } => TypeCode::Sequence {
                element: Box::new(element.clone()),
                bound: 0,
            },
            Self::Array { element, items } => TypeCode::Array {
                element: Box::new(element.clone()),
                length: u32::try_from(items.len()).unwrap_or(u32::MAX),
            },
            Self::Struct {
                repo_id,
                name,
                members,
            }
            | Self::Except {
                repo_id,
                name,
                members,
            } => TypeCode::Struct {
                repo_id: repo_id.clone(),
                name: name.clone(),
                members: members
                    .iter()
                    .map(|(n, v)| (n.clone(), v.type_code()))
                    .collect(),
                is_except: matches!(self, Self::Except { .. }),
            },
            Self::Enum {
                repo_id,
                name,
                members,
                ..
            } => TypeCode::Enum {
                repo_id: repo_id.clone(),
                name: name.clone(),
                members: members.clone(),
            },
            Self::Union { type_code, .. }
            | Self::ObjRef { type_code, .. }
            | Self::Value { type_code, .. }
            | Self::Abstract { type_code, .. } => type_code.clone(),
            Self::Any(_) => TypeCode::Any,
        }
    }

    /// Writes **only the value**, without the `TypeCode` in front of it.
    fn encode_value(&self, w: &mut Writer, enc: &mut Enc) -> Result<(), WireError> {
        match self {
            Self::Value { type_code, value } => {
                return encode_value_ref(type_code, value, w, enc);
            }
            Self::Abstract { value, .. } => {
                enc.data(w);
                return match value {
                    AbstractValue::Reference(r) => {
                        w.write_bool(true);
                        encode_reference(r.as_ref(), w);
                        Ok(())
                    }
                    AbstractValue::Value(v) => {
                        w.write_bool(false);
                        // §9.3.7: a value in an abstract interface always
                        // carries its repository id; the formal type is
                        // the value's own.
                        let formal = match v {
                            ValueRef::Instance(i) => i.type_code.clone(),
                            _ => TypeCode::Null,
                        };
                        encode_value_ref(&formal, v, w, enc)
                    }
                };
            }
            Self::Any(inner) => {
                enc.data(w);
                inner.0.type_code().encode(w)?;
                return inner.0.encode_value(w, enc);
            }
            Self::Seq { items, .. } | Self::Array { items, .. } => {
                if matches!(self, Self::Seq { .. }) {
                    enc.data(w);
                    let len = u32::try_from(items.len()).map_err(|_| {
                        refused(w.position(), "a sequence of at most u32::MAX items")
                    })?;
                    w.write_u32(len);
                }
                if let Self::Seq { element, .. } | Self::Array { element, .. } = self
                    && let Some(bad) = items.iter().find(|i| !conforms(i, element))
                {
                    return Err(refused(
                        w.position(),
                        format!("an element of the element type, not {bad:?}"),
                    ));
                }
                for item in items {
                    item.encode_value(w, enc)?;
                }
                return Ok(());
            }
            Self::Struct { members, .. } => {
                for (_, v) in members {
                    v.encode_value(w, enc)?;
                }
                return Ok(());
            }
            Self::Except {
                repo_id, members, ..
            } => {
                enc.data(w);
                w.write_string(repo_id);
                for (_, v) in members {
                    v.encode_value(w, enc)?;
                }
                return Ok(());
            }
            Self::Union {
                type_code,
                discriminator,
                value,
            } => {
                let TypeCode::Union {
                    discriminator: dtc,
                    default_index,
                    members,
                    ..
                } = type_code.resolved()
                else {
                    return Err(refused(
                        w.position(),
                        "a union value with a tk_union TypeCode",
                    ));
                };
                if !discriminator.fits(dtc) {
                    return Err(refused(
                        w.position(),
                        "a union discriminator of the discriminant type",
                    ));
                }
                let selected = selected_member(members, *default_index, *discriminator);
                enc.data(w);
                discriminator.encode(w);
                return match (selected, value) {
                    (Some(m), Some(v)) if conforms(v, &m.member_type) => v.encode_value(w, enc),
                    (None, None) => Ok(()),
                    _ => Err(refused(
                        w.position(),
                        "a union value whose member is the case its discriminator selects",
                    )),
                };
            }
            // tk_null: no octets, so no chunk either (§9.3.5.2).
            Self::Null => return Ok(()),
            _ => {}
        }
        enc.data(w);
        self.encode_primitive(w)
    }

    /// Writes a value that is one piece of non-value data: a primitive,
    /// a string, a `fixed`, a `Principal`, a `TypeCode`, an enum or an
    /// object reference.
    fn encode_primitive(&self, w: &mut Writer) -> Result<(), WireError> {
        match self {
            Self::Boolean(v) => w.write_bool(*v),
            Self::Octet(v) | Self::Char(v) => w.write_u8(*v),
            Self::Short(v) => w.write_i16(*v),
            Self::UShort(v) | Self::WChar(v) => w.write_u16(*v),
            Self::Long(v) => w.write_i32(*v),
            Self::ULong(v) => w.write_u32(*v),
            Self::LongLong(v) => w.write_i64(*v),
            Self::ULongLong(v) => w.write_u64(*v),
            Self::Float(v) => w.write_f32(*v),
            Self::Double(v) => w.write_f64(*v),
            Self::LongDouble(be) => {
                // Part 2 Figure 9.2: the sixteen octets in the stream's
                // order, eight-aligned.
                w.align_to(8);
                let mut bytes = *be;
                if matches!(w.endianness(), Endianness::Little) {
                    bytes.reverse();
                }
                w.write_bytes(&bytes);
            }
            Self::Str(s) => w.write_string(s),
            Self::WStr(s) => s.encode(w)?,
            Self::Fixed {
                digits,
                scale,
                value,
            } => {
                let (d, sc) = fixed_params(*digits, *scale, w.position())?;
                w.write_fixed(value, d, sc, FixedDigits::Declared)?;
            }
            Self::Principal(bytes) => {
                let len = u32::try_from(bytes.len())
                    .map_err(|_| refused(w.position(), "a Principal of at most u32::MAX octets"))?;
                w.write_u32(len);
                w.write_bytes(bytes);
            }
            Self::TypeCodeValue(tc) => tc.encode(w)?,
            Self::Enum { value, .. } => w.write_u32(*value),
            Self::ObjRef { reference, .. } => encode_reference(reference.as_ref(), w),
            Self::Null
            | Self::Value { .. }
            | Self::Abstract { .. }
            | Self::Any(_)
            | Self::Seq { .. }
            | Self::Array { .. }
            | Self::Struct { .. }
            | Self::Except { .. }
            | Self::Union { .. } => {
                return Err(refused(
                    w.position(),
                    "a primitive value — a constructed one is written member by member",
                ));
            }
        }
        Ok(())
    }

    /// Reads **only the value**, guided by `tc`, one level below the
    /// part around it ([`MAX_VALUE_DEPTH`]); a value that took no octets
    /// counts against [`Dec::empty_part`].
    fn decode_value<'b>(
        tc: &TypeCode,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        if dec.nesting >= MAX_VALUE_DEPTH {
            return Err(refused(
                r.position(),
                format!("a value nested at most {MAX_VALUE_DEPTH} deep"),
            ));
        }
        let at = r.position();
        dec.nesting += 1;
        let v = Self::decode_part(tc, r, dec);
        dec.nesting -= 1;
        let v = v?;
        if r.position() == at {
            dec.empty_part(r)?;
        }
        Ok(v)
    }

    /// [`Self::decode_value`] at its level. Every kind whose value holds
    /// further parts is read by a function of its own, so that a level of
    /// a deep value costs the stack of that kind alone.
    fn decode_part<'b>(
        tc: &TypeCode,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        match tc {
            // A typedef resolves transparently to its content.
            TypeCode::Alias { content, .. } => Self::decode_value(content, r, dec),
            // tk_null and tk_void have no octets (§9.3.5.2).
            TypeCode::Null | TypeCode::Void => Ok(Self::Null),
            TypeCode::Recursive { repo_id } => Self::decode_recursive(repo_id, r, dec),
            TypeCode::Struct { .. } => Self::decode_struct(tc, r, dec),
            TypeCode::Array { element, length } => Self::decode_array(element, *length, r, dec),
            TypeCode::Value { .. } | TypeCode::ValueBox { .. } => Ok(Self::Value {
                type_code: tc.clone(),
                value: decode_value_ref(tc, r, dec)?,
            }),
            TypeCode::Sequence { element, .. } => {
                dec.data(r)?;
                Self::decode_sequence(element, r, dec)
            }
            TypeCode::Union { .. } => {
                dec.data(r)?;
                Self::decode_union(tc, r, dec)
            }
            TypeCode::AbstractInterface { .. } => {
                dec.data(r)?;
                Self::decode_abstract(tc, r, dec)
            }
            TypeCode::Any => {
                dec.data(r)?;
                Ok(Self::Any(Box::new(decode_any(r, dec)?)))
            }
            _ => {
                dec.data(r)?;
                Self::decode_simple(tc, r)
            }
        }
    }

    /// A recursive marker is the enclosing type with its id.
    fn decode_recursive<'b>(
        repo_id: &str,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        let target = dec
            .outer
            .iter()
            .rev()
            .find(|t| t.repo_id() == Some(repo_id))
            .cloned()
            .ok_or_else(|| {
                refused(
                    r.position(),
                    format!("a recursive TypeCode inside the type {repo_id}"),
                )
            })?;
        Self::decode_value(&target, r, dec)
    }

    /// A struct or an exception (§9.3.5.5: an exception starts with its
    /// repository id), member by member.
    fn decode_struct<'b>(
        tc: &TypeCode,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        let TypeCode::Struct {
            repo_id,
            name,
            members,
            is_except,
        } = tc
        else {
            return Err(refused(r.position(), "a struct TypeCode"));
        };
        if *is_except {
            dec.data(r)?;
            let at = r.position();
            let id = r.read_string()?;
            if !repo_id.is_empty() && id != *repo_id {
                return Err(refused(
                    at,
                    format!("the repository id {repo_id} of the exception, not {id}"),
                ));
            }
        }
        let mut out = Vec::with_capacity(members.len());
        dec.outer.push(tc.clone());
        for (member_name, member_type) in members {
            match Self::decode_value(member_type, r, dec) {
                Ok(v) => out.push((member_name.clone(), v)),
                Err(e) => {
                    dec.outer.pop();
                    return Err(e);
                }
            }
        }
        dec.outer.pop();
        Ok(if *is_except {
            Self::Except {
                repo_id: repo_id.clone(),
                name: name.clone(),
                members: out,
            }
        } else {
            Self::Struct {
                repo_id: repo_id.clone(),
                name: name.clone(),
                members: out,
            }
        })
    }

    /// The `length` elements of an array.
    fn decode_array<'b>(
        element: &TypeCode,
        length: u32,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        if length as usize > r.remaining() {
            return Err(WireError::ImpossibleLength {
                at: r.position(),
                claimed: length as usize,
                available: r.remaining(),
            });
        }
        let mut items = Vec::with_capacity((length as usize).min(RESERVED_ELEMENTS));
        for _ in 0..length {
            items.push(Self::decode_value(element, r, dec)?);
        }
        Ok(Self::Array {
            element: element.clone(),
            items,
        })
    }

    /// The count and the elements of a sequence.
    fn decode_sequence<'b>(
        element: &TypeCode,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        let at = r.position();
        let count = r.read_u32()? as usize;
        // A count is not a promise: a peer that claims four billion
        // elements must not become four billion allocations before the
        // first read fails.
        if count > r.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: count,
                available: r.remaining(),
            });
        }
        let mut items = Vec::with_capacity(count.min(RESERVED_ELEMENTS));
        for _ in 0..count {
            items.push(Self::decode_value(element, r, dec)?);
        }
        Ok(Self::Seq {
            element: element.clone(),
            items,
        })
    }

    /// The discriminator and the member it selects.
    fn decode_union<'b>(
        tc: &TypeCode,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        let TypeCode::Union {
            discriminator,
            default_index,
            members,
            ..
        } = tc
        else {
            return Err(refused(r.position(), "a union TypeCode"));
        };
        let label = UnionLabel::decode(discriminator, r)?;
        let value = match selected_member(members, *default_index, label) {
            Some(m) => {
                dec.outer.push(tc.clone());
                let v = Self::decode_value(&m.member_type, r, dec);
                dec.outer.pop();
                Some(Box::new(v?))
            }
            None => None,
        };
        Ok(Self::Union {
            type_code: tc.clone(),
            discriminator: label,
            value,
        })
    }

    /// An abstract interface: a reference or a value (§9.3.7).
    fn decode_abstract<'b>(
        tc: &TypeCode,
        r: &mut Reader<'b>,
        dec: &mut Dec<'_, 'b>,
    ) -> Result<Self, WireError> {
        let value = if r.read_bool()? {
            AbstractValue::Reference(decode_reference(r)?)
        } else {
            AbstractValue::Value(decode_value_ref(&TypeCode::Null, r, dec)?)
        };
        Ok(Self::Abstract {
            type_code: tc.clone(),
            value,
        })
    }

    /// Reads a value of a kind that holds no further parts.
    fn decode_simple(tc: &TypeCode, r: &mut Reader<'_>) -> Result<Self, WireError> {
        Ok(match tc {
            TypeCode::Boolean => Self::Boolean(r.read_bool()?),
            TypeCode::Octet => Self::Octet(r.read_u8()?),
            TypeCode::Char => Self::Char(r.read_u8()?),
            TypeCode::Short => Self::Short(r.read_i16()?),
            TypeCode::UShort => Self::UShort(r.read_u16()?),
            TypeCode::Long => Self::Long(r.read_i32()?),
            TypeCode::ULong => Self::ULong(r.read_u32()?),
            TypeCode::LongLong => Self::LongLong(r.read_i64()?),
            TypeCode::ULongLong => Self::ULongLong(r.read_u64()?),
            TypeCode::Float => Self::Float(r.read_f32()?),
            TypeCode::Double => Self::Double(r.read_f64()?),
            TypeCode::LongDouble => Self::LongDouble(r.read_f128()?.to_be_bytes()),
            TypeCode::WChar => Self::WChar(r.read_u16()?),
            TypeCode::String(_) => Self::Str(r.read_string()?),
            TypeCode::WString(_) => Self::WStr(WString::decode(r)?),
            TypeCode::Fixed { digits, scale } => {
                let (d, sc) = fixed_params(*digits, *scale, r.position())?;
                // Liberal: the field ends at its sign nibble, whatever
                // digit count the writer chose (omniORB the declared one,
                // JacORB the value's own).
                Self::Fixed {
                    digits: *digits,
                    scale: *scale,
                    value: r.read_fixed(d, sc, FixedDigits::ValuePrecision)?,
                }
            }
            TypeCode::Principal => {
                let at = r.position();
                let n = r.read_u32()? as usize;
                if n > r.remaining() {
                    return Err(WireError::ImpossibleLength {
                        at,
                        claimed: n,
                        available: r.remaining(),
                    });
                }
                Self::Principal(r.read_bytes(n)?.to_vec())
            }
            TypeCode::TypeCodeTc => Self::TypeCodeValue(TypeCode::decode(r)?),
            TypeCode::Enum {
                repo_id,
                name,
                members,
            } => {
                let at = r.position();
                let value = r.read_u32()?;
                if value as usize >= members.len() {
                    return Err(refused(
                        at,
                        format!("an enumerator below {}, not {value}", members.len()),
                    ));
                }
                Self::Enum {
                    repo_id: repo_id.clone(),
                    name: name.clone(),
                    value,
                    members: members.clone(),
                }
            }
            TypeCode::ObjRef { .. } | TypeCode::Component { .. } | TypeCode::Home { .. } => {
                Self::ObjRef {
                    type_code: tc.clone(),
                    reference: decode_reference(r)?,
                }
            }
            TypeCode::Native { .. } | TypeCode::LocalInterface { .. } => {
                return Err(refused(
                    r.position(),
                    "an any value of a marshallable type — tk_native and tk_local_interface \
                     have no value encoding",
                ));
            }
            TypeCode::Null
            | TypeCode::Void
            | TypeCode::Recursive { .. }
            | TypeCode::Alias { .. }
            | TypeCode::Struct { .. }
            | TypeCode::Array { .. }
            | TypeCode::Value { .. }
            | TypeCode::ValueBox { .. }
            | TypeCode::Sequence { .. }
            | TypeCode::Union { .. }
            | TypeCode::AbstractInterface { .. }
            | TypeCode::Any => {
                return Err(refused(
                    r.position(),
                    "a simple TypeCode — a constructed one is read part by part",
                ));
            }
        })
    }
}

/// The case a discriminator selects: the member with that label, else
/// the default case (§9.3.5.1.4).
fn selected_member(
    members: &[UnionMember],
    default_index: i32,
    label: UnionLabel,
) -> Option<&UnionMember> {
    let default = usize::try_from(default_index).ok();
    members
        .iter()
        .enumerate()
        .find(|(i, m)| Some(*i) != default && m.label == label)
        .map(|(_, m)| m)
        .or_else(|| default.and_then(|i| members.get(i)))
}

/// The digits and scale of a `tk_fixed` as the wire runtime takes them.
fn fixed_params(digits: u16, scale: i16, at: usize) -> Result<(u8, u8), WireError> {
    match (u8::try_from(digits), u8::try_from(scale)) {
        (Ok(d), Ok(s)) if (1..=31).contains(&d) && s <= d => Ok((d, s)),
        _ => Err(refused(
            at,
            format!(
                "a fixed of 1 to 31 digits with a scale from 0 to the digits, not <{digits},{scale}>"
            ),
        )),
    }
}

/// An object reference as an IOR; nil is the empty type id with no
/// profiles (§9.3.6).
fn encode_reference(reference: Option<&ObjectReference>, w: &mut Writer) {
    match reference {
        Some(r) => r.encode(w),
        None => {
            w.write_string("");
            w.write_u32(0);
        }
    }
}

/// Reads an object reference (§9.3.6). Nil is the empty type id with no
/// profiles; the type id `""` with a profile is a reference to the base
/// `Object` (§9.3.5.1.3). Every profile of the IOR is kept in its order
/// (Part 2 §7.6.3, Full IOR conformance). A type id without a profile is
/// neither nil nor a reference and is refused.
fn decode_reference(r: &mut Reader<'_>) -> Result<Option<ObjectReference>, WireError> {
    let at = r.position();
    let mut probe = r.clone();
    let type_id = probe.read_string()?;
    let count = probe.read_u32()?;
    match count {
        0 if type_id.is_empty() => {
            ObjectReference::decode(r)?;
            Ok(None)
        }
        0 => Err(refused(
            at,
            "a nil object reference or one with at least one profile",
        )),
        _ => ObjectReference::decode(r).map(Some),
    }
}

/// Writes a value-type slot (§9.3.4): null, an indirection to an earlier
/// instance, or a value header and its state. A value is written with
/// its single repository id and unchunked, unless its type is
/// truncatable or it sits inside a chunked value: then chunked, with the
/// list of the repository ids it can be truncated to.
fn encode_value_ref(
    formal: &TypeCode,
    value: &ValueRef,
    w: &mut Writer,
    enc: &mut Enc,
) -> Result<(), WireError> {
    match value {
        ValueRef::Null => {
            enc.data(w);
            w.write_u32(0);
            Ok(())
        }
        ValueRef::Shared(n) => {
            enc.data(w);
            let target = *enc
                .values
                .get(*n as usize)
                .ok_or_else(|| refused(w.position(), format!("an earlier value instance {n}")))?;
            w.write_u32(INDIRECTION_TAG);
            let field = w.position();
            let offset = i64::try_from(target)
                .ok()
                .zip(i64::try_from(field).ok())
                .and_then(|(t, f)| i32::try_from(t - f).ok())
                .ok_or_else(|| refused(field, "a value indirection that fits a long"))?;
            w.write_i32(offset);
            Ok(())
        }
        ValueRef::Instance(inst) => {
            let at = w.position();
            if !matches!(formal.resolved(), TypeCode::Null)
                && !matches!(
                    formal.resolved(),
                    TypeCode::Value { .. } | TypeCode::ValueBox { .. }
                )
            {
                return Err(refused(at, "a value of a value type"));
            }
            let state = value_state_members(&inst.type_code, at)?;
            if state.len() != inst.members.len()
                || state
                    .iter()
                    .zip(&inst.members)
                    .any(|((n, t), (vn, v))| n != vn || !conforms(v, t))
            {
                return Err(refused(
                    at,
                    "value state members that match its type's members, bases first",
                ));
            }
            let ids = truncation_ids(&inst.type_code);
            let repo_id = inst
                .type_code
                .repo_id()
                .ok_or_else(|| refused(at, "a value type with a repository id"))?
                .to_string();
            let chunked = enc.depth > 0 || ids.len() > 1;
            // A nested value starts outside the chunk of the value
            // around it (§9.3.4.6).
            enc.close(w)?;
            w.align_to(4);
            enc.values.push(w.position());
            let codebase_bit = if inst.codebase.is_some() {
                VT_CODEBASE
            } else {
                0
            };
            if chunked {
                w.write_u32(VALUE_TAG_BASE | codebase_bit | VT_ID_LIST | VT_CHUNKED);
                if let Some(url) = &inst.codebase {
                    w.write_string(url);
                }
                w.write_u32(u32::try_from(ids.len()).unwrap_or(u32::MAX));
                for id in &ids {
                    w.write_string(id);
                }
                enc.depth += 1;
            } else {
                w.write_u32(VALUE_TAG_BASE | codebase_bit | VT_SINGLE_ID);
                if let Some(url) = &inst.codebase {
                    w.write_string(url);
                }
                w.write_string(&repo_id);
            }
            for (_, v) in &inst.members {
                v.encode_value(w, enc)?;
            }
            if chunked {
                enc.close(w)?;
                w.write_i32(-enc.depth);
                enc.depth -= 1;
            }
            Ok(())
        }
    }
}

/// Reads a value-type slot (§9.3.4) of the formal type `formal`
/// ([`TypeCode::Null`] when the slot is an abstract interface's: then
/// the repository id names the type).
///
/// zerodds-lint: recursion-depth 256 — two calls per level of the value,
/// [`MAX_VALUE_DEPTH`] (128) levels. The state members are read through
/// `AnyValue::decode_value`, which counts `Dec::nesting` against
/// [`MAX_VALUE_DEPTH`] and refuses a deeper part; the slots inside them
/// come back here from `decode_part`. The one call without a level of its
/// own goes through `decode_skipped_value`, which starts this function at
/// a value tag (`skipped` holds value tag positions only), so that call
/// reads a value and never takes the indirection branch again.
fn decode_value_ref<'b>(
    formal: &TypeCode,
    r: &mut Reader<'b>,
    dec: &mut Dec<'_, 'b>,
) -> Result<ValueRef, WireError> {
    // Inside a chunked value a used-up chunk is followed by a chunk
    // (which may hold a null or an indirection) or by a nested value.
    let at_chunk_end = dec.chunk_end.is_some_and(|end| r.position() >= end);
    r.align_to(4)?;
    let mut tag_pos = r.position();
    let mut tag = r.read_u32()?;
    if at_chunk_end && tag > 0 && tag < VALUE_TAG_BASE {
        dec.chunk_end = Some(r.position() + tag as usize);
        r.align_to(4)?;
        tag_pos = r.position();
        tag = r.read_u32()?;
        if is_value_tag(tag) {
            return Err(refused(
                tag_pos,
                "a nested value that starts outside a chunk",
            ));
        }
    }
    match tag {
        0 => return Ok(ValueRef::Null),
        INDIRECTION_TAG => {
            let field = r.position();
            let target = indirection_target(field, r.read_i32()?)?;
            if let Some(n) = dec.values.get(&target) {
                return Ok(ValueRef::Shared(*n));
            }
            // A value inside state a truncation skipped (§9.3.4.6): read
            // now, at the first indirection to it; later indirections
            // share it.
            if let Some((at, level)) = dec.skipped.get(&target).cloned() {
                return decode_skipped_value(formal, at, level, dec);
            }
            return Err(refused(
                field,
                "an indirection to a value read before in this any",
            ));
        }
        t if is_value_tag(t) => {}
        _ => {
            return Err(refused(
                tag_pos,
                "a value tag, a null tag or an indirection",
            ));
        }
    }
    if !at_chunk_end && dec.chunk_end.is_some_and(|end| tag_pos < end) {
        return Err(refused(
            tag_pos,
            "a nested value that starts outside a chunk",
        ));
    }
    let index = u32::try_from(dec.values.len()).unwrap_or(u32::MAX);
    dec.values.insert(tag_pos, index);
    let flags = tag & 0xff;
    let codebase = if flags & VT_CODEBASE != 0 {
        Some(dec.string_or_indirection(r)?)
    } else {
        None
    };
    let ids: Vec<String> = match flags & 0x06 {
        0 => Vec::new(),
        VT_SINGLE_ID => vec![dec.string_or_indirection(r)?],
        VT_ID_LIST => {
            r.align_to(4)?;
            let list_pos = r.position();
            let n = r.read_u32()?;
            if n == INDIRECTION_TAG {
                let field = r.position();
                let target = indirection_target(field, r.read_i32()?)?;
                dec.lists.get(&target).cloned().ok_or_else(|| {
                    refused(field, "an indirection to a repository id list read before")
                })?
            } else {
                if n as usize > r.remaining() / 8 {
                    return Err(WireError::ImpossibleLength {
                        at: list_pos,
                        claimed: n as usize,
                        available: r.remaining(),
                    });
                }
                let mut list = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    list.push(dec.string_or_indirection(r)?);
                }
                dec.lists.insert(list_pos, list.clone());
                list
            }
        }
        _ => return Err(refused(tag_pos, "value tag type bits 0, 2 or 6")),
    };
    let chunked = flags & VT_CHUNKED != 0;
    if !chunked && dec.depth > 0 {
        return Err(refused(
            tag_pos,
            "a chunked value nested in a chunked value",
        ));
    }
    // The type the state is read as, and whether the value is truncated
    // to it.
    let resolve = |id: &str| -> Option<TypeCode> {
        if formal.resolved().repo_id() == Some(id) {
            Some(formal.resolved().clone())
        } else {
            dec.known.get(id).cloned()
        }
    };
    let (actual, truncated) = if ids.is_empty() {
        if matches!(formal.resolved(), TypeCode::Null) {
            return Err(refused(
                tag_pos,
                "a value in an abstract interface with its repository id",
            ));
        }
        (formal.resolved().clone(), false)
    } else if let Some(tc) = resolve(&ids[0]) {
        (tc, false)
    } else {
        match ids.iter().skip(1).find_map(|id| resolve(id)) {
            Some(tc) if chunked => (tc, true),
            _ => {
                return Err(refused(
                    tag_pos,
                    format!(
                        "a value of a known type or truncatable to one, not {}",
                        ids[0]
                    ),
                ));
            }
        }
    };
    let state = value_state_members(&actual, tag_pos)?;
    let saved_end = dec.chunk_end;
    if chunked {
        dec.depth += 1;
        dec.chunk_end = Some(r.position());
    }
    let my_depth = dec.depth;
    let mut members = Vec::with_capacity(state.len());
    let pushed = push_value_chain(&actual, &mut dec.outer);
    let mut failed = None;
    for (name, member_type) in &state {
        match AnyValue::decode_value(member_type, r, dec) {
            Ok(v) => members.push((name.clone(), v)),
            Err(e) => {
                failed = Some(e);
                break;
            }
        }
    }
    dec.outer.truncate(dec.outer.len() - pushed);
    if let Some(e) = failed {
        return Err(e);
    }
    if chunked {
        if truncated {
            skip_to_end(r, dec, my_depth)?;
        } else {
            end_chunked(r, dec, my_depth)?;
        }
        dec.depth = my_depth - 1;
        // The value around this one continues in a new chunk.
        dec.chunk_end = if dec.depth > 0 {
            Some(r.position())
        } else {
            saved_end
        };
    }
    Ok(ValueRef::Instance(Box::new(ValueInstance {
        type_code: actual,
        members,
        codebase,
    })))
}

/// Reads a value nested in the state of a truncated value (§9.3.4.1)
/// from `at`, the reader at its value tag, as a chunked value at nesting
/// depth `level` of the formal type of the slot whose indirection points
/// to it; the chunk state of the decoder is restored afterwards.
///
/// zerodds-lint: recursion-depth 128 — one call per level of the value,
/// [`MAX_VALUE_DEPTH`]: it reads the value through `decode_value_ref`,
/// whose state members go down a level through `AnyValue::decode_value`
/// (capped by `Dec::nesting`) before an indirection inside them can come
/// back here.
fn decode_skipped_value<'b>(
    formal: &TypeCode,
    mut at: Reader<'b>,
    level: i32,
    dec: &mut Dec<'_, 'b>,
) -> Result<ValueRef, WireError> {
    let saved = (dec.chunk_end, dec.depth, dec.ended);
    dec.depth = level;
    // The value tag sits where a chunk of the value around it ended.
    dec.chunk_end = Some(at.position());
    dec.ended = None;
    let value = decode_value_ref(formal, &mut at, dec);
    (dec.chunk_end, dec.depth, dec.ended) = saved;
    value
}

/// Pushes a value type and its concrete bases onto `outer`; returns how
/// many.
fn push_value_chain(tc: &TypeCode, outer: &mut Vec<TypeCode>) -> usize {
    let mut n = 0;
    let mut cur = tc.resolved();
    loop {
        match cur {
            TypeCode::Value { concrete_base, .. } => {
                outer.push(cur.clone());
                n += 1;
                cur = concrete_base.resolved();
            }
            TypeCode::ValueBox { .. } => {
                outer.push(cur.clone());
                return n + 1;
            }
            _ => return n,
        }
    }
}

/// The end of a chunked value at `depth` after its state: an end tag
/// read before for this depth or below, or the end tag that follows the
/// used-up chunk.
fn end_chunked(r: &mut Reader<'_>, dec: &mut Dec<'_, '_>, depth: i32) -> Result<(), WireError> {
    if let Some(t) = dec.ended
        && t <= depth
    {
        if t == depth {
            dec.ended = None;
        }
        return Ok(());
    }
    if dec.chunk_end.is_some_and(|end| r.position() != end) {
        return Err(refused(
            r.position(),
            "a value chunk that ends with the value's state",
        ));
    }
    r.align_to(4)?;
    let at = r.position();
    let tag = r.read_i32()?;
    if tag >= 0 || -tag > depth {
        return Err(refused(
            at,
            format!("an end tag for nesting depth {depth} or less"),
        ));
    }
    if -tag < depth {
        dec.ended = Some(-tag);
    }
    Ok(())
}

/// Skips the rest of a value truncated at `depth` (§9.3.4.1): its chunks
/// and the chunked values nested in them, to its end tag.
fn skip_to_end<'b>(r: &mut Reader<'b>, dec: &mut Dec<'_, 'b>, depth: i32) -> Result<(), WireError> {
    if dec.ended.is_some_and(|t| t <= depth) {
        return end_chunked(r, dec, depth);
    }
    let mut level = depth;
    loop {
        if let Some(end) = dec.chunk_end
            && r.position() < end
        {
            r.skip_to(end)?;
        }
        r.align_to(4)?;
        let at = r.position();
        let before = r.clone();
        let tag = r.read_i32()?;
        #[allow(clippy::cast_sign_loss)]
        let utag = tag as u32;
        if tag > 0 && utag < VALUE_TAG_BASE {
            dec.chunk_end = Some(r.position() + utag as usize);
        } else if is_value_tag(utag) {
            // A nested value: its header, then its chunks one level down.
            // Recorded, so that a later indirection to it can read it.
            dec.skipped.entry(at).or_insert((before, level));
            let flags = utag & 0xff;
            if flags & VT_CHUNKED == 0 {
                return Err(refused(at, "a chunked value nested in a chunked value"));
            }
            if flags & VT_CODEBASE != 0 {
                dec.string_or_indirection(r)?;
            }
            match flags & 0x06 {
                0 => {}
                VT_SINGLE_ID => {
                    dec.string_or_indirection(r)?;
                }
                VT_ID_LIST => {
                    r.align_to(4)?;
                    let n = r.read_u32()?;
                    if n == INDIRECTION_TAG {
                        r.read_i32()?;
                    } else {
                        for _ in 0..n {
                            dec.string_or_indirection(r)?;
                        }
                    }
                }
                _ => return Err(refused(at, "value tag type bits 0, 2 or 6")),
            }
            level += 1;
            dec.chunk_end = Some(r.position());
        } else if tag < 0 {
            let ends = -tag;
            if ends <= depth {
                if ends < depth {
                    dec.ended = Some(ends);
                }
                return Ok(());
            }
            if ends > level {
                return Err(refused(
                    at,
                    format!("an end tag for nesting depth {level} or less"),
                ));
            }
            level = ends - 1;
            dec.chunk_end = Some(r.position());
        } else {
            return Err(refused(at, "a chunk size, a value tag or an end tag"));
        }
    }
}

/// An `any` inside a value being decoded: its `TypeCode` and its value
/// under the same chunk state.
fn decode_any<'b>(r: &mut Reader<'b>, dec: &mut Dec<'_, 'b>) -> Result<CorbaAny, WireError> {
    let tc = TypeCode::decode(r)?;
    Ok(CorbaAny(AnyValue::decode_value(&tc, r, dec)?))
}

/// IDL `any` (§9.3.5.2): self-describing, so a full [`TypeCode`]
/// (§9.3.5.1) and then the value in its own representation.
///
/// Wire-compatible with omniORB, TAO and JacORB, structured content
/// included.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CorbaAny(pub AnyValue);

impl CorbaAny {
    /// Writes the `TypeCode` and then the value.
    ///
    /// # Errors
    /// As [`TypeCode::encode`], [`WireError::OutOfRange`] for a sequence
    /// longer than `u32` can carry, a value that does not fit its
    /// `TypeCode` (a union member that is not the selected case, value
    /// state that does not match its type, a custom-marshalled value
    /// type) and a `fixed` outside 1 to 31 digits.
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        self.0.type_code().encode(w)?;
        let mut enc = Enc::default();
        self.0.encode_value(w, &mut enc)
    }

    /// Reads the `TypeCode` and then the value it describes.
    ///
    /// # Errors
    /// As [`TypeCode::decode`], plus whatever the value's own reader
    /// raises; a value type whose repository id names neither the
    /// `TypeCode`'s type nor a type it is truncatable to, and a value in
    /// an abstract interface, are refused — [`Self::decode_with`] names
    /// the value types to read them as.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Self::decode_with(r, &KnownValueTypes::new())
    }

    /// [`Self::decode`] for an `any` that is one of several values of a
    /// GIOP message: its `TypeCode` is a top-level `TypeCode` of the
    /// message, read with [`TypeCode::decode_top_level`], so that under
    /// GIOP 1.3 and above it may be an indirection to the `TypeCode` of an
    /// earlier `any` of the message (CORBA 3.3 Part 2 §9.3.5.1.6).
    ///
    /// # Errors
    /// As [`Self::decode`] and [`TypeCode::decode_top_level`].
    pub fn decode_in_message(
        r: &mut Reader<'_>,
        tops: &mut crate::type_code::TopLevelTypeCodes,
    ) -> Result<Self, WireError> {
        let start = r.position();
        let tc = TypeCode::decode_top_level(r, tops)?;
        let known = KnownValueTypes::new();
        let mut dec = Dec::new(&known, start);
        Ok(Self(AnyValue::decode_value(&tc, r, &mut dec)?))
    }

    /// [`Self::decode`] with the value types `known` besides those the
    /// `TypeCode` names.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_with(r: &mut Reader<'_>, known: &KnownValueTypes) -> Result<Self, WireError> {
        let mut dec = Dec::new(known, r.position());
        decode_any(r, &mut dec)
    }

    /// Writes the value alone, without its `TypeCode` — the
    /// `encode_value` of `IOP::Codec` (CORBA 3.3 Part 2 §7.8.1.2).
    ///
    /// # Errors
    /// As [`Self::encode`] for the value.
    pub fn encode_value(&self, w: &mut Writer) -> Result<(), WireError> {
        let mut enc = Enc::default();
        self.0.encode_value(w, &mut enc)
    }

    /// Reads a value of type `tc` that carries no `TypeCode` of its own
    /// — the `decode_value` of `IOP::Codec` (Part 2 §7.8.1.2).
    ///
    /// # Errors
    /// As [`Self::decode`] for the value.
    pub fn decode_value(tc: &TypeCode, r: &mut Reader<'_>) -> Result<Self, WireError> {
        let known = KnownValueTypes::new();
        let mut dec = Dec::new(&known, r.position());
        Ok(Self(AnyValue::decode_value(tc, r, &mut dec)?))
    }
}

impl<'k> Dec<'k, '_> {
    /// A decoder of an `any` or a value that starts at `start`.
    fn new(known: &'k KnownValueTypes, start: usize) -> Self {
        Dec {
            known,
            values: BTreeMap::new(),
            skipped: BTreeMap::new(),
            strings: BTreeMap::new(),
            lists: BTreeMap::new(),
            chunk_end: None,
            depth: 0,
            ended: None,
            outer: Vec::new(),
            nesting: 0,
            start,
            empty_parts: 0,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;
    use zerodds_wire::{WcharFraming, WcharPosition, WcharWidth};

    fn rt(v: &AnyValue, e: Endianness) {
        let any = CorbaAny(v.clone());
        let mut w = Writer::new(e);
        any.encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, e);
        assert_eq!(&CorbaAny::decode(&mut r).unwrap().0, v, "{v:?} / {e:?}");
        assert_eq!(r.remaining(), 0, "trailing bytes for {v:?}");
    }

    /// Part 2 §9.3.5.1.6 under GIOP 1.3 and above: the `TypeCode` of the
    /// second `any` of a message is an indirection to the `TypeCode` of
    /// the first; the second value is read with it. Below 1.3 the
    /// indirection is refused.
    #[test]
    fn corba_p2_9_3_5_a_second_any_indirects_to_the_first_typecode_under_1_3() {
        use crate::type_code::TopLevelTypeCodes;
        let first = AnyValue::Struct {
            repo_id: String::from("IDL:demo/P:1.0"),
            name: String::from("P"),
            members: vec![(String::from("x"), AnyValue::Long(1))],
        };
        let mut w = Writer::new(Endianness::Little);
        CorbaAny(first.clone()).encode(&mut w).unwrap();
        w.align_to(4);
        w.write_u32(0xffff_ffff);
        let field = w.position();
        #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
        w.write_u32((-(field as i64)) as i32 as u32);
        w.write_i32(2); // the value of the second `any`: P { x: 2 }
        let bytes = w.finish();
        let read = |allow: bool| {
            let mut r = Reader::new(&bytes, Endianness::Little);
            let mut tops = TopLevelTypeCodes::new(allow);
            let a = CorbaAny::decode_in_message(&mut r, &mut tops)?;
            let b = CorbaAny::decode_in_message(&mut r, &mut tops)?;
            Ok::<_, WireError>((a.0, b.0, r.remaining()))
        };
        let (a, b, rest) = read(true).unwrap();
        assert_eq!(a, first);
        assert_eq!(
            b,
            AnyValue::Struct {
                repo_id: String::from("IDL:demo/P:1.0"),
                name: String::from("P"),
                members: vec![(String::from("x"), AnyValue::Long(2))],
            }
        );
        assert_eq!(rest, 0);
        assert!(read(false).is_err());
    }

    #[test]
    fn scalars_roundtrip() {
        for v in [
            AnyValue::Null,
            AnyValue::Boolean(true),
            AnyValue::Octet(0xab),
            AnyValue::Char(b'Q'),
            AnyValue::Short(-2),
            AnyValue::UShort(7),
            AnyValue::Long(-70_000),
            AnyValue::ULong(70_000),
            AnyValue::LongLong(-5_000_000_000),
            AnyValue::ULongLong(5_000_000_000),
            AnyValue::Float(1.5),
            AnyValue::Double(-2.25),
            AnyValue::WChar(0x00e4),
            AnyValue::Str("hello".into()),
        ] {
            rt(&v, Endianness::Big);
            rt(&v, Endianness::Little);
        }
    }

    #[test]
    fn structured_roundtrip() {
        let seq = AnyValue::Seq {
            element: TypeCode::Long,
            items: vec![AnyValue::Long(1), AnyValue::Long(-1)],
        };
        rt(&seq, Endianness::Big);
        rt(&seq, Endianness::Little);

        // An *empty* sequence: the element TypeCode has no other source,
        // which is why the variant carries it.
        let empty = AnyValue::Seq {
            element: TypeCode::String(0),
            items: vec![],
        };
        rt(&empty, Endianness::Big);

        let st = AnyValue::Struct {
            repo_id: "IDL:Point:1.0".into(),
            name: "Point".into(),
            members: vec![
                ("x".into(), AnyValue::Long(3)),
                ("label".into(), AnyValue::Str("p".into())),
            ],
        };
        rt(&st, Endianness::Big);
        rt(&st, Endianness::Little);

        let en = AnyValue::Enum {
            repo_id: "IDL:Color:1.0".into(),
            name: "Color".into(),
            value: 1,
            members: vec!["RED".into(), "GREEN".into()],
        };
        rt(&en, Endianness::Little);

        let nested = AnyValue::Any(Box::new(CorbaAny(AnyValue::ULong(9))));
        rt(&nested, Endianness::Big);
    }

    #[test]
    fn any_opens_with_its_typecode() {
        let mut w = Writer::new(Endianness::Big);
        CorbaAny(AnyValue::ULong(1)).encode(&mut w).unwrap();
        let bytes = w.finish();
        assert_eq!(&bytes[0..4], &[0, 0, 0, 5], "TCKind tk_ulong");
    }

    // -- wstring ---------------------------------------------------------

    fn wstring_bytes(text: &str, e: Endianness, order: WstringUnitOrder) -> Vec<u8> {
        let mut w = Writer::new(e).with_wstring_unit_order(order);
        WString::from(text).encode(&mut w).unwrap();
        w.finish()
    }

    #[test]
    fn wstring_byte_order_mark_is_omni_orb_form() {
        // The mark and the units, both little-endian, whatever the
        // stream's own order is. Four octets: the mark and one unit.
        let bytes = wstring_bytes("A", Endianness::Big, WstringUnitOrder::ByteOrderMark);
        assert_eq!(bytes, [0, 0, 0, 4, 0xFF, 0xFE, 0x41, 0x00]);
    }

    #[test]
    fn wstring_stream_order_is_the_jacorb_form() {
        // No mark; the units take the message's order.
        assert_eq!(
            wstring_bytes("A", Endianness::Big, WstringUnitOrder::Stream),
            [0, 0, 0, 2, 0x00, 0x41]
        );
        assert_eq!(
            wstring_bytes("A", Endianness::Little, WstringUnitOrder::Stream),
            [2, 0, 0, 0, 0x41, 0x00]
        );
    }

    #[test]
    fn wstring_big_endian_always_ignores_the_stream() {
        assert_eq!(
            wstring_bytes("A", Endianness::Little, WstringUnitOrder::BigEndianAlways),
            [2, 0, 0, 0, 0x00, 0x41]
        );
    }

    #[test]
    fn empty_wstring_is_a_zero_length_under_every_order() {
        for order in [
            WstringUnitOrder::Stream,
            WstringUnitOrder::BigEndianAlways,
            WstringUnitOrder::ByteOrderMark,
            WstringUnitOrder::HostUnswapped,
        ] {
            assert_eq!(
                wstring_bytes("", Endianness::Big, order),
                [0, 0, 0, 0],
                "{order:?}"
            );
        }
    }

    #[test]
    fn wstring_roundtrips_under_every_order_and_stream() {
        for order in [
            WstringUnitOrder::Stream,
            WstringUnitOrder::BigEndianAlways,
            WstringUnitOrder::ByteOrderMark,
            WstringUnitOrder::HostUnswapped,
        ] {
            for e in [Endianness::Big, Endianness::Little] {
                for text in ["", "A", "grüß", "日本語"] {
                    let bytes = wstring_bytes(text, e, order);
                    let mut r = Reader::new(&bytes, e).with_wstring_unit_order(order);
                    assert_eq!(
                        WString::decode(&mut r).unwrap().as_str(),
                        text,
                        "{text:?} / {order:?} / {e:?}"
                    );
                }
            }
        }
    }

    /// A marked stream reads under a reader configured for the unmarked
    /// form, and the other way round — the decoder is liberal on this
    /// axis on purpose, because the ORBs disagree about it.
    #[test]
    fn wstring_decode_accepts_the_other_orb_s_form() {
        let marked = wstring_bytes("äb", Endianness::Big, WstringUnitOrder::ByteOrderMark);
        let mut r = Reader::new(&marked, Endianness::Big); // default Stream
        assert_eq!(WString::decode(&mut r).unwrap().as_str(), "äb");

        let unmarked = wstring_bytes("äb", Endianness::Big, WstringUnitOrder::Stream);
        let mut r = Reader::new(&unmarked, Endianness::Big)
            .with_wstring_unit_order(WstringUnitOrder::Stream);
        assert_eq!(WString::decode(&mut r).unwrap().as_str(), "äb");
    }

    #[test]
    fn odd_wstring_octet_count_is_refused() {
        let bytes = [0u8, 0, 0, 3, 0x00, 0x41, 0x00];
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(matches!(
            WString::decode(&mut r),
            Err(WireError::OutOfRange { .. })
        ));
    }

    #[test]
    fn impossible_wstring_length_is_refused_before_allocating() {
        let bytes = [0xFFu8, 0xFF, 0xFF, 0xFE, 0x00, 0x41];
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(matches!(
            WString::decode(&mut r),
            Err(WireError::ImpossibleLength { .. })
        ));
    }

    /// The CORBA `wchar` — one octet of length, then big-endian units —
    /// is `zerodds-wire`'s own, reached by naming the framing. It is not
    /// re-implemented here, and this pins the bytes so that the
    /// capability is not silently lost with the type that used to hold
    /// it: omniORB 4.3 and JacORB 3.9 both write `02 0041` for `'A'`.
    #[test]
    fn corba_wchar_is_the_wire_runtime_s_giop_framing() {
        for e in [Endianness::Big, Endianness::Little] {
            let mut w = Writer::new(e)
                .with_wchar_framing(WcharFraming::GiopLengthPrefixed)
                .with_wchar_width(WcharWidth::Two)
                .with_wstring_unit_order(WstringUnitOrder::BigEndianAlways);
            w.write_wchar('A', WcharPosition::Member).unwrap();
            assert_eq!(w.finish(), [0x02, 0x00, 0x41], "{e:?}");
        }
    }

    // -- §9.3: the constructed and pseudo-object kinds -------------------

    use crate::type_code::{ValueMember, visibility};

    fn union_tc() -> TypeCode {
        TypeCode::Union {
            repo_id: "IDL:U:1.0".into(),
            name: "U".into(),
            discriminator: Box::new(TypeCode::Long),
            default_index: 1,
            members: vec![
                UnionMember {
                    label: UnionLabel::Long(7),
                    name: "s".into(),
                    member_type: TypeCode::String(0),
                },
                UnionMember {
                    label: UnionLabel::Long(0),
                    name: "d".into(),
                    member_type: TypeCode::Double,
                },
            ],
        }
    }

    fn reference() -> ObjectReference {
        ObjectReference {
            type_id: "IDL:Echo:1.0".into(),
            iiop_profile: vec![0, 1, 2, 0, 0, 0, 0, 1, 9],
            policy_overrides: Vec::new(),
            profiles: Vec::new(),
        }
    }

    #[test]
    fn corba_p2_9_3_5_2_any_with_every_constructed_type_round_trips() {
        let values = [
            AnyValue::LongDouble([0x3f, 0xff, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            AnyValue::Fixed {
                digits: 6,
                scale: 2,
                value: "-1234.50".into(),
            },
            AnyValue::Principal(vec![1, 2, 3]),
            AnyValue::TypeCodeValue(union_tc()),
            AnyValue::Array {
                element: TypeCode::Short,
                items: vec![AnyValue::Short(1), AnyValue::Short(-2), AnyValue::Short(3)],
            },
            AnyValue::Array {
                element: TypeCode::Array {
                    element: Box::new(TypeCode::Octet),
                    length: 2,
                },
                items: vec![
                    AnyValue::Array {
                        element: TypeCode::Octet,
                        items: vec![AnyValue::Octet(1), AnyValue::Octet(2)],
                    },
                    AnyValue::Array {
                        element: TypeCode::Octet,
                        items: vec![AnyValue::Octet(3), AnyValue::Octet(4)],
                    },
                ],
            },
            AnyValue::Union {
                type_code: union_tc(),
                discriminator: UnionLabel::Long(7),
                value: Some(Box::new(AnyValue::Str("seven".into()))),
            },
            // Any other label selects the default case.
            AnyValue::Union {
                type_code: union_tc(),
                discriminator: UnionLabel::Long(42),
                value: Some(Box::new(AnyValue::Double(0.5))),
            },
            AnyValue::Except {
                repo_id: "IDL:Oops:1.0".into(),
                name: "Oops".into(),
                members: vec![("code".into(), AnyValue::Long(5))],
            },
            AnyValue::ObjRef {
                type_code: TypeCode::ObjRef {
                    repo_id: "IDL:Echo:1.0".into(),
                    name: "Echo".into(),
                },
                reference: Some(reference()),
            },
            AnyValue::ObjRef {
                type_code: TypeCode::Component {
                    repo_id: "IDL:C:1.0".into(),
                    name: "C".into(),
                },
                reference: None,
            },
            AnyValue::Abstract {
                type_code: TypeCode::AbstractInterface {
                    repo_id: "IDL:Ab:1.0".into(),
                    name: "Ab".into(),
                },
                value: AbstractValue::Reference(Some(reference())),
            },
        ];
        for v in &values {
            rt(v, Endianness::Big);
            rt(v, Endianness::Little);
        }
    }

    #[test]
    fn corba_p2_9_3_5_5_an_exception_in_an_any_starts_with_its_repository_id() {
        let mut w = Writer::new(Endianness::Big);
        AnyValue::Except {
            repo_id: "IDL:E:1.0".into(),
            name: "E".into(),
            members: vec![],
        }
        .encode_value(&mut w, &mut Enc::default())
        .unwrap();
        let mut expected = Writer::new(Endianness::Big);
        expected.write_string("IDL:E:1.0");
        assert_eq!(w.finish(), expected.finish());
    }

    #[test]
    fn corba_p2_9_3_7_a_nil_reference_and_the_two_abstract_branches_on_the_wire() {
        // Nil: the empty type id and no profiles (§9.3.6).
        let mut w = Writer::new(Endianness::Big);
        encode_reference(None, &mut w);
        assert_eq!(w.finish(), [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
        // An abstract interface holding a null value: discriminator FALSE
        // and the null tag.
        let v = AnyValue::Abstract {
            type_code: TypeCode::AbstractInterface {
                repo_id: "IDL:Ab:1.0".into(),
                name: "Ab".into(),
            },
            value: AbstractValue::Value(ValueRef::Null),
        };
        let mut w = Writer::new(Endianness::Big);
        v.encode_value(&mut w, &mut Enc::default()).unwrap();
        assert_eq!(w.finish(), [0, 0, 0, 0, 0, 0, 0, 0]);
        rt(&v, Endianness::Little);
    }

    #[test]
    fn corba_p2_9_3_union_member_must_be_the_selected_case() {
        let wrong = CorbaAny(AnyValue::Union {
            type_code: union_tc(),
            discriminator: UnionLabel::Long(7),
            value: Some(Box::new(AnyValue::Double(1.0))),
        });
        let mut w = Writer::new(Endianness::Big);
        assert!(matches!(
            wrong.encode(&mut w),
            Err(WireError::OutOfRange { .. })
        ));
        // No default case and an unmatched label: no member, no value.
        let no_default = TypeCode::Union {
            repo_id: "IDL:N:1.0".into(),
            name: "N".into(),
            discriminator: Box::new(TypeCode::Boolean),
            default_index: -1,
            members: vec![UnionMember {
                label: UnionLabel::Boolean(true),
                name: "t".into(),
                member_type: TypeCode::Long,
            }],
        };
        rt(
            &AnyValue::Union {
                type_code: no_default,
                discriminator: UnionLabel::Boolean(false),
                value: None,
            },
            Endianness::Big,
        );
    }

    #[test]
    fn corba_p2_9_3_local_and_native_values_are_refused() {
        for tc in [
            TypeCode::Native {
                repo_id: "IDL:N:1.0".into(),
                name: "N".into(),
            },
            TypeCode::LocalInterface {
                repo_id: "IDL:L:1.0".into(),
                name: "L".into(),
            },
        ] {
            let mut w = Writer::new(Endianness::Big);
            tc.encode(&mut w).unwrap();
            let bytes = w.finish();
            let mut r = Reader::new(&bytes, Endianness::Big);
            assert!(matches!(
                CorbaAny::decode(&mut r),
                Err(WireError::OutOfRange { .. })
            ));
        }
    }

    fn animal() -> TypeCode {
        TypeCode::Value {
            repo_id: "IDL:Animal:1.0".into(),
            name: "Animal".into(),
            modifier: value_modifier::VM_NONE,
            concrete_base: Box::new(TypeCode::Null),
            members: vec![ValueMember {
                name: "legs".into(),
                member_type: TypeCode::Short,
                visibility: visibility::PUBLIC_MEMBER,
            }],
            is_event: false,
        }
    }

    fn horse() -> TypeCode {
        TypeCode::Value {
            repo_id: "IDL:Horse:1.0".into(),
            name: "Horse".into(),
            modifier: value_modifier::VM_TRUNCATABLE,
            concrete_base: Box::new(animal()),
            members: vec![ValueMember {
                name: "rider".into(),
                member_type: animal(),
                visibility: visibility::PUBLIC_MEMBER,
            }],
            is_event: false,
        }
    }

    fn an_animal(legs: i16) -> ValueRef {
        ValueRef::Instance(Box::new(ValueInstance {
            type_code: animal(),
            members: vec![("legs".into(), AnyValue::Short(legs))],
            codebase: None,
        }))
    }

    #[test]
    fn corba_p2_9_3_4_a_value_is_tag_repository_id_and_state() {
        let v = AnyValue::Value {
            type_code: animal(),
            value: an_animal(4),
        };
        let mut w = Writer::new(Endianness::Big);
        v.encode_value(&mut w, &mut Enc::default()).unwrap();
        let mut expected = Writer::new(Endianness::Big);
        expected.write_u32(0x7fff_ff02);
        expected.write_string("IDL:Animal:1.0");
        expected.write_i16(4);
        assert_eq!(w.finish(), expected.finish());
        for e in [Endianness::Big, Endianness::Little] {
            rt(&v, e);
            rt(
                &AnyValue::Value {
                    type_code: animal(),
                    value: ValueRef::Null,
                },
                e,
            );
        }
    }

    #[test]
    fn corba_p2_9_3_4_3_a_shared_and_a_cyclic_value_are_indirections() {
        // valuetype Pair { public Animal a; public Animal b; } with b the
        // same instance as a.
        let pair = TypeCode::Value {
            repo_id: "IDL:Pair:1.0".into(),
            name: "Pair".into(),
            modifier: value_modifier::VM_NONE,
            concrete_base: Box::new(TypeCode::Null),
            members: vec![
                ValueMember {
                    name: "a".into(),
                    member_type: animal(),
                    visibility: visibility::PUBLIC_MEMBER,
                },
                ValueMember {
                    name: "b".into(),
                    member_type: animal(),
                    visibility: visibility::PUBLIC_MEMBER,
                },
            ],
            is_event: false,
        };
        let shared = AnyValue::Value {
            type_code: pair.clone(),
            value: ValueRef::Instance(Box::new(ValueInstance {
                type_code: pair,
                members: vec![
                    (
                        "a".into(),
                        AnyValue::Value {
                            type_code: animal(),
                            value: an_animal(3),
                        },
                    ),
                    (
                        "b".into(),
                        AnyValue::Value {
                            type_code: animal(),
                            value: ValueRef::Shared(1),
                        },
                    ),
                ],
                codebase: None,
            })),
        };
        rt(&shared, Endianness::Big);
        // valuetype List { public List next; } pointing at itself.
        let list = TypeCode::Value {
            repo_id: "IDL:List:1.0".into(),
            name: "List".into(),
            modifier: value_modifier::VM_NONE,
            concrete_base: Box::new(TypeCode::Null),
            members: vec![ValueMember {
                name: "next".into(),
                member_type: TypeCode::Recursive {
                    repo_id: "IDL:List:1.0".into(),
                },
                visibility: visibility::PUBLIC_MEMBER,
            }],
            is_event: false,
        };
        let cyclic = AnyValue::Value {
            type_code: list.clone(),
            value: ValueRef::Instance(Box::new(ValueInstance {
                type_code: list.clone(),
                members: vec![(
                    "next".into(),
                    AnyValue::Value {
                        type_code: list,
                        value: ValueRef::Shared(0),
                    },
                )],
                codebase: None,
            })),
        };
        rt(&cyclic, Endianness::Little);
        let mut w = Writer::new(Endianness::Big);
        cyclic.encode_value(&mut w, &mut Enc::default()).unwrap();
        let bytes = w.finish();
        // Tag, "IDL:List:1.0", then the indirection back to the tag at 0.
        let n = bytes.len();
        assert_eq!(&bytes[n - 8..n - 4], &[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(
            &bytes[n - 4..],
            &(-i32::try_from(n - 4).unwrap()).to_be_bytes()
        );
    }

    #[test]
    fn corba_p2_9_3_4_6_a_truncatable_value_is_chunked_with_its_id_list() {
        let v = AnyValue::Value {
            type_code: horse(),
            value: ValueRef::Instance(Box::new(ValueInstance {
                type_code: horse(),
                members: vec![
                    ("legs".into(), AnyValue::Short(4)),
                    (
                        "rider".into(),
                        AnyValue::Value {
                            type_code: animal(),
                            value: an_animal(2),
                        },
                    ),
                ],
                codebase: None,
            })),
        };
        let mut w = Writer::new(Endianness::Big);
        v.encode_value(&mut w, &mut Enc::default()).unwrap();
        let bytes = w.finish();
        let mut x = Writer::new(Endianness::Big);
        x.write_u32(0x7fff_ff0e); // chunked, list of ids
        x.write_u32(2);
        x.write_string("IDL:Horse:1.0");
        x.write_string("IDL:Animal:1.0");
        x.write_u32(2); // chunk: legs
        x.write_i16(4);
        x.write_u32(0x7fff_ff0a); // nested value, chunked, one id
        x.write_u32(1);
        x.write_string("IDL:Animal:1.0");
        x.write_u32(2); // its chunk
        x.write_i16(2);
        x.write_i32(-2); // nested value ends
        x.write_i32(-1); // outer value ends
        let expected = x.finish();
        // The nested value is written with the list form too (one id):
        // compare the header words that are fixed by the rules.
        assert_eq!(&bytes[..4], &expected[..4]);
        assert_eq!(&bytes[bytes.len() - 8..], &expected[expected.len() - 8..]);
        for e in [Endianness::Big, Endianness::Little] {
            rt(&v, e);
        }
    }

    fn knight() -> TypeCode {
        TypeCode::Value {
            repo_id: "IDL:Knight:1.0".into(),
            name: "Knight".into(),
            modifier: value_modifier::VM_TRUNCATABLE,
            concrete_base: Box::new(animal()),
            members: vec![
                ValueMember {
                    name: "pet".into(),
                    member_type: animal(),
                    visibility: visibility::PUBLIC_MEMBER,
                },
                ValueMember {
                    name: "age".into(),
                    member_type: TypeCode::Long,
                    visibility: visibility::PUBLIC_MEMBER,
                },
            ],
            is_event: false,
        }
    }

    /// `tc` and then `state`, as an any on the wire.
    fn any_bytes(tc: &TypeCode, state: &[u8]) -> Vec<u8> {
        let mut w = Writer::new(Endianness::Big);
        tc.encode(&mut w).unwrap();
        let mut b = w.finish();
        // The value starts at the next four-aligned position.
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        b.extend_from_slice(state);
        b
    }

    #[test]
    fn corba_p2_9_3_4_6_chunks_continuations_and_a_shared_end_tag_are_read() {
        // Knight (legs from Animal; pet: Animal; age: long) as another ORB
        // may write it: a chunk, the nested value outside it with an
        // end tag -2 of its own, a continuation chunk, the end tag -1.
        let mut x = Writer::new(Endianness::Big);
        x.write_u32(0x7fff_ff0e);
        x.write_u32(2);
        x.write_string("IDL:Knight:1.0");
        x.write_string("IDL:Animal:1.0");
        x.write_u32(2);
        x.write_i16(4);
        x.write_u32(0x7fff_ff08); // nested, chunked, no type information
        x.write_u32(2);
        x.write_i16(9);
        x.write_i32(-2);
        x.write_u32(4); // continuation chunk
        x.write_i32(30);
        x.write_i32(-1);
        let bytes = any_bytes(&knight(), &x.finish());
        let mut r = Reader::new(&bytes, Endianness::Big);
        let any = CorbaAny::decode(&mut r).unwrap();
        let AnyValue::Value {
            value: ValueRef::Instance(inst),
            ..
        } = any.0
        else {
            panic!("a value");
        };
        assert_eq!(
            inst.members,
            vec![
                ("legs".into(), AnyValue::Short(4)),
                (
                    "pet".into(),
                    AnyValue::Value {
                        type_code: animal(),
                        value: an_animal(9),
                    }
                ),
                ("age".into(), AnyValue::Long(30)),
            ]
        );
        assert_eq!(r.remaining(), 0);

        // Horse (rider last): one end tag -1 ends the nested and the
        // outer value together.
        let mut x = Writer::new(Endianness::Big);
        x.write_u32(0x7fff_ff0e);
        x.write_u32(2);
        x.write_string("IDL:Horse:1.0");
        x.write_string("IDL:Animal:1.0");
        x.write_u32(2);
        x.write_i16(4);
        x.write_u32(0x7fff_ff08);
        x.write_u32(2);
        x.write_i16(9);
        x.write_i32(-1);
        x.write_u32(0xdead_beef); // what follows the any
        let bytes = any_bytes(&horse(), &x.finish());
        let mut r = Reader::new(&bytes, Endianness::Big);
        let any = CorbaAny::decode(&mut r).unwrap();
        let AnyValue::Value {
            value: ValueRef::Instance(inst),
            ..
        } = any.0
        else {
            panic!("a value");
        };
        assert_eq!(inst.members.len(), 2);
        assert_eq!(r.read_u32().unwrap(), 0xdead_beef);

        // A primitive past the end of its chunk is refused.
        let mut x = Writer::new(Endianness::Big);
        x.write_u32(0x7fff_ff08);
        x.write_u32(2);
        x.write_i16(4);
        x.write_i32(-1);
        let ok = any_bytes(&animal(), &x.finish());
        let mut r = Reader::new(&ok, Endianness::Big);
        assert!(CorbaAny::decode(&mut r).is_ok());
        let mut x = Writer::new(Endianness::Big);
        x.write_u32(0x7fff_ff08);
        x.write_i32(-1); // an end tag where the state's chunk belongs
        let bad = any_bytes(&animal(), &x.finish());
        let mut r = Reader::new(&bad, Endianness::Big);
        assert!(CorbaAny::decode(&mut r).is_err());
    }

    #[test]
    fn corba_p2_9_3_4_1_a_value_of_an_unknown_derived_type_is_truncated() {
        // A Horse sent where the receiver knows only Animal: the `rider`
        // state is skipped to the end tag.
        let h = AnyValue::Value {
            type_code: horse(),
            value: ValueRef::Instance(Box::new(ValueInstance {
                type_code: horse(),
                members: vec![
                    ("legs".into(), AnyValue::Short(4)),
                    (
                        "rider".into(),
                        AnyValue::Value {
                            type_code: animal(),
                            value: an_animal(2),
                        },
                    ),
                ],
                codebase: None,
            })),
        };
        let mut w = Writer::new(Endianness::Little);
        // The formal type in the TypeCode is Animal; the value is a Horse.
        animal().encode(&mut w).unwrap();
        let mut enc = Enc::default();
        let AnyValue::Value { value, .. } = &h else {
            unreachable_value()
        };
        encode_value_ref(&animal(), value, &mut w, &mut enc).unwrap();
        w.write_u32(0xdead_beef); // what follows the any
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        let any = CorbaAny::decode(&mut r).unwrap();
        assert_eq!(
            any.0,
            AnyValue::Value {
                type_code: animal(),
                value: an_animal(4),
            }
        );
        assert_eq!(r.read_u32().unwrap(), 0xdead_beef);
        // With Horse known, the whole value is read.
        let known: KnownValueTypes = [("IDL:Horse:1.0".to_string(), horse())]
            .into_iter()
            .collect();
        let mut r = Reader::new(&bytes, Endianness::Little);
        let any = CorbaAny::decode_with(&mut r, &known).unwrap();
        let AnyValue::Value {
            value: ValueRef::Instance(inst),
            ..
        } = any.0
        else {
            panic!("a value");
        };
        assert_eq!(inst.type_code, horse());
        // A non-truncatable value of an unknown type is refused.
        let mut w = Writer::new(Endianness::Big);
        w.write_u32(0x7fff_ff02);
        w.write_string("IDL:Zebra:1.0");
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let mut dec = Dec {
            known: &KnownValueTypes::new(),
            values: BTreeMap::new(),
            skipped: BTreeMap::new(),
            strings: BTreeMap::new(),
            lists: BTreeMap::new(),
            chunk_end: None,
            depth: 0,
            ended: None,
            outer: Vec::new(),
            nesting: 0,
            start: 0,
            empty_parts: 0,
        };
        assert!(matches!(
            decode_value_ref(&animal(), &mut r, &mut dec),
            Err(WireError::OutOfRange { .. })
        ));
    }

    /// §9.3.4.6: an indirection may point back to a value nested in the
    /// state a truncation skipped; that value is read at the first such
    /// indirection, later ones share it.
    #[test]
    fn corba_p2_9_3_4_6_an_indirection_into_truncated_state_reads_the_skipped_value() {
        // valuetype Trio { public Animal a; public Animal b; public Animal c; }
        let member = |name: &str| ValueMember {
            name: name.into(),
            member_type: animal(),
            visibility: visibility::PUBLIC_MEMBER,
        };
        let trio = TypeCode::Value {
            repo_id: "IDL:Trio:1.0".into(),
            name: "Trio".into(),
            modifier: value_modifier::VM_NONE,
            concrete_base: Box::new(TypeCode::Null),
            members: vec![member("a"), member("b"), member("c")],
            is_event: false,
        };
        let slot = |value: ValueRef| AnyValue::Value {
            type_code: animal(),
            value,
        };
        // a is a Horse whose rider is instance 2; b and c point to it.
        let horse_with_rider = ValueRef::Instance(Box::new(ValueInstance {
            type_code: horse(),
            members: vec![
                ("legs".into(), AnyValue::Short(4)),
                ("rider".into(), slot(an_animal(2))),
            ],
            codebase: None,
        }));
        let sent = ValueRef::Instance(Box::new(ValueInstance {
            type_code: trio.clone(),
            members: vec![
                ("a".into(), slot(horse_with_rider)),
                ("b".into(), slot(ValueRef::Shared(2))),
                ("c".into(), slot(ValueRef::Shared(2))),
            ],
            codebase: None,
        }));
        let mut w = Writer::new(Endianness::Big);
        trio.encode(&mut w).unwrap();
        encode_value_ref(&trio, &sent, &mut w, &mut Enc::default()).unwrap();
        w.write_u32(0xdead_beef);
        let bytes = w.finish();
        // Horse is unknown: a is truncated to Animal, the rider skipped.
        let mut r = Reader::new(&bytes, Endianness::Big);
        let any = CorbaAny::decode(&mut r).unwrap();
        assert_eq!(r.read_u32().unwrap(), 0xdead_beef);
        let expected = AnyValue::Value {
            type_code: trio.clone(),
            value: ValueRef::Instance(Box::new(ValueInstance {
                type_code: trio,
                members: vec![
                    ("a".into(), slot(an_animal(4))),
                    ("b".into(), slot(an_animal(2))),
                    ("c".into(), slot(ValueRef::Shared(2))),
                ],
                codebase: None,
            })),
        };
        assert_eq!(any.0, expected);
        // What was read writes and reads back as itself.
        rt(&expected, Endianness::Little);
    }

    /// §9.3.4.1: the codebase URL of a value tag (bit 0x01) is kept and
    /// written back, before the repository id, for a single id and for an
    /// id list.
    #[test]
    fn corba_p2_9_3_4_1_a_codebase_url_is_kept_and_written_back() {
        let with_codebase = |type_code: TypeCode, members| {
            ValueRef::Instance(Box::new(ValueInstance {
                type_code,
                members,
                codebase: Some("http://repo.example/classes".into()),
            }))
        };
        let single = AnyValue::Value {
            type_code: animal(),
            value: with_codebase(animal(), vec![("legs".into(), AnyValue::Short(4))]),
        };
        let mut w = Writer::new(Endianness::Big);
        single.encode_value(&mut w, &mut Enc::default()).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert_eq!(r.read_u32().unwrap(), 0x7fff_ff03, "codebase and one id");
        assert_eq!(r.read_string().unwrap(), "http://repo.example/classes");
        assert_eq!(r.read_string().unwrap(), "IDL:Animal:1.0");
        assert_eq!(r.read_i16().unwrap(), 4);
        rt(&single, Endianness::Big);
        let chunked = AnyValue::Value {
            type_code: horse(),
            value: with_codebase(
                horse(),
                vec![
                    ("legs".into(), AnyValue::Short(4)),
                    (
                        "rider".into(),
                        AnyValue::Value {
                            type_code: animal(),
                            value: an_animal(2),
                        },
                    ),
                ],
            ),
        };
        let mut w = Writer::new(Endianness::Little);
        chunked.encode_value(&mut w, &mut Enc::default()).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Little);
        assert_eq!(
            r.read_u32().unwrap(),
            0x7fff_ff0f,
            "codebase, id list, chunked"
        );
        assert_eq!(r.read_string().unwrap(), "http://repo.example/classes");
        assert_eq!(r.read_u32().unwrap(), 2, "Horse and Animal");
        let known: KnownValueTypes = [("IDL:Horse:1.0".to_string(), horse())]
            .into_iter()
            .collect();
        let mut r = Reader::new(&bytes, Endianness::Little);
        let mut dec = Dec {
            known: &known,
            values: BTreeMap::new(),
            skipped: BTreeMap::new(),
            strings: BTreeMap::new(),
            lists: BTreeMap::new(),
            chunk_end: None,
            depth: 0,
            ended: None,
            outer: Vec::new(),
            nesting: 0,
            start: 0,
            empty_parts: 0,
        };
        let AnyValue::Value { value, .. } = &chunked else {
            unreachable_value()
        };
        assert_eq!(
            &decode_value_ref(&horse(), &mut r, &mut dec).unwrap(),
            value
        );
    }

    /// §9.3.6, §9.3.5.1.3: nil is the empty type id with no profiles; the
    /// empty type id with an IIOP profile is a reference to `Object`; an
    /// IOR with other or further profiles keeps all of them and is
    /// written back unchanged (Part 2 §7.6.3); a type id without a
    /// profile is refused.
    #[test]
    fn corba_p2_9_3_6_object_references_in_an_any_keep_their_profiles_or_are_refused() {
        let ior = |type_id: &str, profiles: &[(u32, &[u8])]| {
            let mut w = Writer::new(Endianness::Big);
            w.write_string(type_id);
            w.write_u32(u32::try_from(profiles.len()).unwrap());
            for (tag, data) in profiles {
                w.write_u32(*tag);
                w.write_u32(u32::try_from(data.len()).unwrap());
                w.write_bytes(data);
            }
            w.finish()
        };
        let read = |bytes: &[u8]| decode_reference(&mut Reader::new(bytes, Endianness::Big));
        assert_eq!(read(&ior("", &[])).unwrap(), None);
        let base_object = read(&ior("", &[(0, &[0, 1, 2])])).unwrap().unwrap();
        assert_eq!(base_object.type_id, "");
        assert_eq!(base_object.iiop_profile, vec![0, 1, 2]);
        assert_eq!(
            read(&ior("IDL:Echo:1.0", &[(0, &[9])])).unwrap(),
            Some(ObjectReference {
                type_id: "IDL:Echo:1.0".into(),
                iiop_profile: vec![9],
                policy_overrides: Vec::new(),
                profiles: Vec::new(),
            })
        );
        // TAG_MULTIPLE_COMPONENTS only, TAG_UIPMC only, two profiles: kept
        // and written back byte for byte.
        for bytes in [
            ior("", &[(1, &[0, 0, 0, 0])]),
            ior("IDL:Echo:1.0", &[(3, &[0])]),
            ior("IDL:Echo:1.0", &[(0, &[9]), (1, &[0, 0, 0, 0])]),
        ] {
            let reference = read(&bytes).unwrap();
            let mut w = Writer::new(Endianness::Big);
            encode_reference(reference.as_ref(), &mut w);
            assert_eq!(w.finish(), bytes);
        }
        assert!(matches!(
            read(&ior("IDL:Echo:1.0", &[])),
            Err(WireError::OutOfRange { .. })
        ));
    }

    fn unreachable_value() -> ! {
        panic!("the test value is a value type")
    }

    #[test]
    fn corba_p2_9_3_4_a_value_box_and_an_abstract_value_round_trip() {
        let boxed = TypeCode::ValueBox {
            repo_id: "IDL:Name:1.0".into(),
            name: "Name".into(),
            content: Box::new(TypeCode::String(0)),
        };
        let v = AnyValue::Value {
            type_code: boxed.clone(),
            value: ValueRef::Instance(Box::new(ValueInstance {
                type_code: boxed,
                members: vec![(String::new(), AnyValue::Str("Bucephalus".into()))],
                codebase: None,
            })),
        };
        rt(&v, Endianness::Big);
        // An abstract interface holding a value: its repository id names
        // the type, which the decoder must know.
        let ab = AnyValue::Abstract {
            type_code: TypeCode::AbstractInterface {
                repo_id: "IDL:Ab:1.0".into(),
                name: "Ab".into(),
            },
            value: AbstractValue::Value(an_animal(6)),
        };
        let mut w = Writer::new(Endianness::Big);
        CorbaAny(ab.clone()).encode(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(CorbaAny::decode(&mut r).is_err());
        let known: KnownValueTypes = [("IDL:Animal:1.0".to_string(), animal())]
            .into_iter()
            .collect();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert_eq!(CorbaAny::decode_with(&mut r, &known).unwrap().0, ab);
    }
    /// `struct Node { sequence<Node> kids; }`: the member's element is
    /// the struct itself.
    #[cfg(feature = "std")]
    fn node() -> TypeCode {
        TypeCode::Struct {
            repo_id: "IDL:Node:1.0".into(),
            name: "Node".into(),
            members: vec![(
                "kids".into(),
                TypeCode::Sequence {
                    element: Box::new(TypeCode::Recursive {
                        repo_id: "IDL:Node:1.0".into(),
                    }),
                    bound: 0,
                },
            )],
            is_except: false,
        }
    }

    /// `valuetype VNode { public VNode next; };`.
    #[cfg(feature = "std")]
    fn vnode() -> TypeCode {
        TypeCode::Value {
            repo_id: "IDL:VNode:1.0".into(),
            name: "VNode".into(),
            modifier: value_modifier::VM_NONE,
            concrete_base: Box::new(TypeCode::Null),
            members: vec![ValueMember {
                name: "next".into(),
                member_type: TypeCode::Recursive {
                    repo_id: "IDL:VNode:1.0".into(),
                },
                visibility: visibility::PUBLIC_MEMBER,
            }],
            is_event: false,
        }
    }

    /// `count` times the `u32` `each`, then the `u32`s of `end`.
    #[cfg(feature = "std")]
    fn words(each: u32, count: usize, end: &[u32]) -> Vec<u8> {
        let mut w = Writer::new(Endianness::Big);
        for _ in 0..count {
            w.write_u32(each);
        }
        for e in end {
            w.write_u32(*e);
        }
        w.finish()
    }

    /// How deep the parts of a decoded value nest.
    #[cfg(feature = "std")]
    fn levels(v: &AnyValue) -> usize {
        match v {
            AnyValue::Struct { members, .. } => {
                1 + members.iter().map(|(_, m)| levels(m)).max().unwrap_or(0)
            }
            AnyValue::Seq { items, .. } => 1 + items.iter().map(levels).max().unwrap_or(0),
            AnyValue::Any(inner) => 1 + levels(&inner.0),
            AnyValue::Value {
                value: ValueRef::Instance(i),
                ..
            } => 1 + i.members.iter().map(|(_, m)| levels(m)).max().unwrap_or(0),
            _ => 1,
        }
    }

    /// A recursive `TypeCode` and an `any` inside an `any` let a value
    /// nest without end, and each level is a stack frame of the decoder:
    /// a value deeper than `MAX_VALUE_DEPTH` is a `WireError`, not a stack
    /// overflow that aborts the process. Measured on a thread with the
    /// stack the standard library gives a thread it spawns (2 MiB), in
    /// the unoptimised build of the tests: the deepest value that is
    /// read, and values one level deeper and 20000 levels deep.
    #[cfg(feature = "std")]
    #[test]
    fn corba_p2_9_3_a_value_nested_beyond_max_value_depth_is_refused_not_a_stack_overflow() {
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(|| {
                let max = MAX_VALUE_DEPTH as usize;
                let deeper = |e: WireError| {
                    matches!(&e, WireError::OutOfRange { expected, .. }
                        if expected.contains("nested at most"))
                };
                // Node, kids, then kid and kids again per level: two
                // parts first and three per level.
                let tc = node();
                let fits = (max - 2) / 3;
                let value = CorbaAny::decode_value(
                    &tc,
                    &mut Reader::new(&words(1, fits, &[0]), Endianness::Big),
                )
                .unwrap();
                assert_eq!(levels(&value.0), 2 * fits + 2);
                for n in [fits + 1, 20_000] {
                    let e = CorbaAny::decode_value(
                        &tc,
                        &mut Reader::new(&words(1, n, &[0]), Endianness::Big),
                    )
                    .unwrap_err();
                    assert!(deeper(e), "{n} levels");
                }
                // An any of an any: one part per any.
                let long = |n: usize| {
                    use crate::type_code::tckind::{TK_ANY, TK_LONG};
                    words(TK_ANY, n, &[TK_LONG, 42])
                };
                let any =
                    CorbaAny::decode(&mut Reader::new(&long(max - 1), Endianness::Big)).unwrap();
                assert_eq!(levels(&any.0), max);
                for n in [max, 20_000] {
                    let e =
                        CorbaAny::decode(&mut Reader::new(&long(n), Endianness::Big)).unwrap_err();
                    assert!(deeper(e), "{n} anys");
                }
                // A value whose state is a value of its own type: the
                // value and its recursive member per level, then the
                // null value at the end.
                let tc = vnode();
                let chain = |n: usize| words(VALUE_TAG_BASE, n, &[0]);
                let fits = (max - 1) / 2;
                let value =
                    CorbaAny::decode_value(&tc, &mut Reader::new(&chain(fits), Endianness::Big))
                        .unwrap();
                assert_eq!(levels(&value.0), fits + 1);
                for n in [fits + 1, 20_000] {
                    let e =
                        CorbaAny::decode_value(&tc, &mut Reader::new(&chain(n), Endianness::Big))
                            .unwrap_err();
                    assert!(deeper(e), "{n} values");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
