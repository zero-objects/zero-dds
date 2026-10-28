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
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, WireError, Writer, WstringUnitOrder};

use crate::type_code::TypeCode;

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
        if octets % 2 != 0 {
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

/// What a [`CorbaAny`] can carry: every scalar IDL type, string and
/// wstring, **and** the structured types — sequence, struct, enum, a
/// nested `any`.
///
/// The structured variants carry enough type information that the full
/// [`TypeCode`] can be derived from the value alone, including the
/// element `TypeCode` of an empty sequence, which has no other source.
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
    /// `tk_wchar`, as the bare transmission code unit.
    WChar(u16),
    /// `tk_string`.
    Str(String),
    /// `tk_wstring`.
    WStr(WString),
    /// `tk_sequence`: the element `TypeCode` — needed even when there are
    /// no elements to read it off — and the items.
    Seq {
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
    /// `tk_any`: a nested `any`.
    Any(Box<CorbaAny>),
}

impl AnyValue {
    /// The full [`TypeCode`] (§15.3.5) this value describes.
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
            Self::WChar(_) => TypeCode::WChar,
            Self::Str(_) => TypeCode::String(0),
            Self::WStr(_) => TypeCode::WString(0),
            Self::Seq { element, .. } => TypeCode::Sequence {
                element: Box::new(element.clone()),
                bound: 0,
            },
            Self::Struct {
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
                is_except: false,
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
            Self::Any(_) => TypeCode::Any,
        }
    }

    /// Writes **only the value**, without the `TypeCode` in front of it.
    fn encode_value(&self, w: &mut Writer) -> Result<(), WireError> {
        match self {
            Self::Null => Ok(()),
            Self::Boolean(v) => {
                w.write_bool(*v);
                Ok(())
            }
            Self::Octet(v) | Self::Char(v) => {
                w.write_u8(*v);
                Ok(())
            }
            Self::Short(v) => {
                w.write_i16(*v);
                Ok(())
            }
            Self::UShort(v) | Self::WChar(v) => {
                w.write_u16(*v);
                Ok(())
            }
            Self::Long(v) => {
                w.write_i32(*v);
                Ok(())
            }
            Self::ULong(v) => {
                w.write_u32(*v);
                Ok(())
            }
            Self::LongLong(v) => {
                w.write_i64(*v);
                Ok(())
            }
            Self::ULongLong(v) => {
                w.write_u64(*v);
                Ok(())
            }
            Self::Float(v) => {
                w.write_f32(*v);
                Ok(())
            }
            Self::Double(v) => {
                w.write_f64(*v);
                Ok(())
            }
            Self::Str(s) => {
                w.write_string(s);
                Ok(())
            }
            Self::WStr(s) => s.encode(w),
            Self::Seq { items, .. } => {
                let len = u32::try_from(items.len()).map_err(|_| WireError::OutOfRange {
                    at: w.position(),
                    expected: format!("a sequence of at most u32::MAX items, not {}", items.len()),
                })?;
                w.write_u32(len);
                for item in items {
                    item.encode_value(w)?;
                }
                Ok(())
            }
            Self::Struct { members, .. } => {
                for (_, v) in members {
                    v.encode_value(w)?;
                }
                Ok(())
            }
            Self::Enum { value, .. } => {
                w.write_u32(*value);
                Ok(())
            }
            Self::Any(inner) => inner.encode(w),
        }
    }

    /// Reads **only the value**, guided by `tc`.
    fn decode_value(tc: &TypeCode, r: &mut Reader<'_>) -> Result<Self, WireError> {
        match tc {
            TypeCode::Null | TypeCode::Void => Ok(Self::Null),
            TypeCode::Boolean => Ok(Self::Boolean(r.read_bool()?)),
            TypeCode::Octet => Ok(Self::Octet(r.read_u8()?)),
            TypeCode::Char => Ok(Self::Char(r.read_u8()?)),
            TypeCode::Short => Ok(Self::Short(r.read_i16()?)),
            TypeCode::UShort => Ok(Self::UShort(r.read_u16()?)),
            TypeCode::Long => Ok(Self::Long(r.read_i32()?)),
            TypeCode::ULong => Ok(Self::ULong(r.read_u32()?)),
            TypeCode::LongLong => Ok(Self::LongLong(r.read_i64()?)),
            TypeCode::ULongLong => Ok(Self::ULongLong(r.read_u64()?)),
            TypeCode::Float => Ok(Self::Float(r.read_f32()?)),
            TypeCode::Double => Ok(Self::Double(r.read_f64()?)),
            TypeCode::WChar => Ok(Self::WChar(r.read_u16()?)),
            TypeCode::String(_) => Ok(Self::Str(r.read_string()?)),
            TypeCode::WString(_) => Ok(Self::WStr(WString::decode(r)?)),
            TypeCode::Sequence { element, .. } => {
                let at = r.position();
                let count = r.read_u32()? as usize;
                // A count is not a promise: a peer that claims four
                // billion elements must not become four billion
                // allocations before the first read fails.
                if count > r.remaining() {
                    return Err(WireError::ImpossibleLength {
                        at,
                        claimed: count,
                        available: r.remaining(),
                    });
                }
                let mut items = Vec::with_capacity(count.min(4096));
                for _ in 0..count {
                    items.push(Self::decode_value(element, r)?);
                }
                Ok(Self::Seq {
                    element: (**element).clone(),
                    items,
                })
            }
            TypeCode::Struct {
                repo_id,
                name,
                members,
                ..
            } => {
                let mut out = Vec::with_capacity(members.len());
                for (member_name, member_type) in members {
                    out.push((member_name.clone(), Self::decode_value(member_type, r)?));
                }
                Ok(Self::Struct {
                    repo_id: repo_id.clone(),
                    name: name.clone(),
                    members: out,
                })
            }
            TypeCode::Enum {
                repo_id,
                name,
                members,
            } => Ok(Self::Enum {
                repo_id: repo_id.clone(),
                name: name.clone(),
                value: r.read_u32()?,
                members: members.clone(),
            }),
            // A typedef resolves transparently to its content.
            TypeCode::Alias { content, .. } => Self::decode_value(content, r),
            TypeCode::Any => Ok(Self::Any(Box::new(CorbaAny::decode(r)?))),
            TypeCode::ObjRef { .. } | TypeCode::TypeCodeTc => Err(WireError::OutOfRange {
                at: r.position(),
                expected: String::from(
                    "an any whose value this decoder reads — an objref or a TypeCode value \
                     inside an any is not one of them",
                ),
            }),
            // A value can only be read against a *resolved* type; the
            // marker names a type that is still being built.
            TypeCode::Recursive { .. } => Err(WireError::OutOfRange {
                at: r.position(),
                expected: String::from("an any whose TypeCode is resolved, not a recursive marker"),
            }),
        }
    }
}

/// IDL `any` (§15.3.7): self-describing, so a full [`TypeCode`] (§15.3.5)
/// and then the value in its own representation.
///
/// Wire-compatible with omniORB, TAO and JacORB, structured content
/// included.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CorbaAny(pub AnyValue);

impl CorbaAny {
    /// Writes the `TypeCode` and then the value.
    ///
    /// # Errors
    /// As [`TypeCode::encode`], and [`WireError::OutOfRange`] for a
    /// sequence longer than `u32` can carry.
    pub fn encode(&self, w: &mut Writer) -> Result<(), WireError> {
        self.0.type_code().encode(w)?;
        self.0.encode_value(w)
    }

    /// Reads the `TypeCode` and then the value it describes.
    ///
    /// # Errors
    /// As [`TypeCode::decode`], plus whatever the value's own reader
    /// raises.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let tc = TypeCode::decode(r)?;
        Ok(Self(AnyValue::decode_value(&tc, r)?))
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
}
