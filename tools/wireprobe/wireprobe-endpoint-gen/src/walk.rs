// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One IDL type and one JSON sample, into the calls a binding makes.
//!
//! The walk asks `zerodds-ast-layout` every question that is a property
//! of the *type* — where a DHEADER goes, what a member's id is, how a
//! member is framed, how wide an enumeration's holder is — and asks the
//! sample only for values. What it never does is decide a byte: no
//! padding is emitted, no length is computed, no order is chosen. Those
//! come out of the binding when the generated program runs, which is
//! the only reason running it says anything.
//!
//! ## Why the plan and not a second reading of §7.4
//!
//! Because the placement of a header is a property the two sides read
//! from the same IDL. A hand-written endpoint program frames an
//! `@appendable` struct with a DHEADER because the struct is
//! `@appendable`; this walk does the same, from the same declaration.
//! What is *inside* the DHEADER — the four octets, their order, the
//! body length they carry — is the binding's, and that is what the
//! comparison against `tests/interop-results/<case>/zerodds-rust/`
//! measures.

use crate::ops::{Int, Lc, Op, OutOfSurface, Program, Real, Version};
use crate::surface::Surface;
use serde_json::Value;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_ast_layout::key::{
    FixedDigitCount, WcharWidth, WstringCodeUnits, WstringLengthCounts, WstringLengthPolicy,
    WstringTerminator, WstringUnitOrder, WstringUnitWidth,
};
use zerodds_ast_layout::plan::{ScalarWrite, scalar_write};
use zerodds_ast_layout::{
    DepCollector, LayoutKey, MemberHeader, TypeLayout, WcharPosition, enumerator_wire_value, plan,
};

/// How one member's header has to be closed once its value is written.
///
/// Carried rather than recomputed, because the closing call takes an
/// argument the opening one decided: a member header closes with the
/// length code it opened with, and a parameter closes against the
/// alignment origin its own header moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    /// Nothing to close.
    None,
    /// A PL_CDR2 member header, opened with this length code.
    Em(Lc),
    /// A PL_CDR1 parameter header, either form.
    Pid,
}

/// What the walk can end in.
pub enum Refusal {
    /// The binding exports no operation for something in this case.
    /// Exit code 3.
    OutOfSurface(OutOfSurface),
    /// The model, the sample or the plan could not be read. Exit code
    /// 7: the generator, not the binding, is what went wrong.
    Generator(String),
}

impl From<OutOfSurface> for Refusal {
    fn from(reason: OutOfSurface) -> Self {
        Self::OutOfSurface(reason)
    }
}

impl From<String> for Refusal {
    fn from(message: String) -> Self {
        Self::Generator(message)
    }
}

/// Build the call sequence for one case.
///
/// # Errors
/// [`Refusal::OutOfSurface`] when the binding has no operation for a
/// construct in the type, and [`Refusal::Generator`] for anything else.
pub fn program(
    ast: &Ast,
    root: Id<TypeNode>,
    values: &Value,
    key: LayoutKey,
    surface: Surface,
) -> Result<Program, Refusal> {
    let mut walk = Walk {
        ast,
        key,
        surface,
        ops: Vec::new(),
    };
    // XTypes 1.3 §7.4.1: MAXALIGN is eight under XCDR1 and four under
    // XCDR2. It is not a property of the sample and not something the
    // walk can decide late — it is the encoding, and it reaches the
    // binding's constructor, so it is carried on the program.
    let version = if key.encoding.is_v1() {
        Version::Xcdr1
    } else {
        Version::Xcdr2
    };
    walk.value(root, values, "sample")?;
    Ok(Program {
        ops: walk.ops,
        version,
    })
}

struct Walk<'a> {
    ast: &'a Ast,
    key: LayoutKey,
    surface: Surface,
    ops: Vec<Op>,
}

impl Walk<'_> {
    fn layout(&self, type_: Id<TypeNode>) -> Result<TypeLayout, Refusal> {
        let mut deps = DepCollector::default();
        plan(self.ast, type_, self.key, &mut deps)
            .map_err(|error| Refusal::Generator(format!("layout: {error}")))
    }

    /// Follow typedefs to the type that decides the wire form.
    fn behind_aliases(&self, type_: Id<TypeNode>) -> Result<Id<TypeNode>, Refusal> {
        let mut current = type_;
        for _ in 0..32 {
            let node = self
                .ast
                .type_(current)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            match &node.kind {
                TypeKind::Alias { aliased } => {
                    current = aliased.resolved().ok_or_else(|| {
                        Refusal::Generator(format!("`{}`: unresolved typedef", node.name))
                    })?;
                }
                _ => return Ok(current),
            }
        }
        Err(Refusal::Generator("typedef chain too deep".into()))
    }

    /// Write one value of one type.
    fn value(&mut self, type_: Id<TypeNode>, json: &Value, path: &str) -> Result<(), Refusal> {
        // A scalar first: `scalar_write` sees through typedefs and
        // answers for every leaf, so nothing below has to.
        if let Some(write) = scalar_write(self.ast, type_, self.key, WcharPosition::Member) {
            return self.scalar(type_, write, json, path);
        }
        let resolved = self.behind_aliases(type_)?;
        let node = self
            .ast
            .type_(resolved)
            .map_err(|error| Refusal::Generator(format!("{error}")))?;
        match &node.kind {
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                self.aggregate(resolved, json, path)
            }
            TypeKind::Union { .. } => self.union(resolved, json, path),
            TypeKind::Sequence { element, .. } => {
                let element = element.resolved().ok_or_else(|| {
                    Refusal::Generator(format!("{path}: unresolved sequence element"))
                })?;
                self.collection(resolved, element, json, path, true)
            }
            TypeKind::Array { element, dims } => {
                let element = element.resolved().ok_or_else(|| {
                    Refusal::Generator(format!("{path}: unresolved array element"))
                })?;
                let mut expected: u64 = 1;
                for dim in dims {
                    let value = self
                        .ast
                        .eval_integral(*dim)
                        .map_err(|error| Refusal::Generator(format!("{path}: {error:?}")))?;
                    expected = expected.saturating_mul(u64::try_from(value).unwrap_or(0));
                }
                self.array(resolved, element, json, path, expected, dims.len())
            }
            TypeKind::Map { key, value, .. } => {
                let k = key
                    .resolved()
                    .ok_or_else(|| Refusal::Generator(format!("{path}: unresolved map key")))?;
                let v = value
                    .resolved()
                    .ok_or_else(|| Refusal::Generator(format!("{path}: unresolved map value")))?;
                self.map(resolved, k, v, json, path)
            }
            TypeKind::Bitset { .. } => self.bitset(resolved, json, path),
            other => Err(OutOfSurface(format!(
                "{path}: `{}` has no operation in this binding",
                kind_name(other)
            ))
            .into()),
        }
    }

    /// One scalar, as the one call its wire form takes.
    fn scalar(
        &mut self,
        type_: Id<TypeNode>,
        write: ScalarWrite,
        json: &Value,
        path: &str,
    ) -> Result<(), Refusal> {
        let op = match write {
            ScalarWrite::Bool => Op::Bool(json.as_bool().ok_or_else(|| bad(path, "a boolean"))?),
            ScalarWrite::U8 | ScalarWrite::I8 => Op::U8(widened(json, 8, path)?),
            // §7.4.1.1.1 Table 30 gives `char` one octet, so a code
            // point that does not fit one has no `char` form at all and
            // is refused rather than truncated. The Rust column refuses
            // `primitive-char-above-latin1` in the same words.
            ScalarWrite::Char => {
                let code = code_point(json, path)?;
                Op::Char(Int::new(u64::from(octet_char(code, path)?), code))
            }
            ScalarWrite::I16 | ScalarWrite::U16 => Op::U16(widened(json, 16, path)?),
            ScalarWrite::I32 | ScalarWrite::U32 => Op::U32(widened(json, 32, path)?),
            ScalarWrite::I64 | ScalarWrite::U64 => Op::U64(widened(json, 64, path)?),
            ScalarWrite::F32 => {
                let number = json.as_f64().ok_or_else(|| bad(path, "a number"))?;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "an IDL `float` is binary32; the sample carries it as JSON, \
                              which has one number type"
                )]
                let value = number as f32;
                // The text is the sample's own, unparsed: sixteen of the
                // seventeen renderers hand the decimal over as written so
                // that the narrowing to `binary32` is the target
                // language's and not this program's. `Real` says which.
                Op::F32(Real {
                    value,
                    text: json.to_string(),
                })
            }
            ScalarWrite::F64 => {
                if !self.surface.f64 {
                    return Err(OutOfSurface(format!(
                        "{path}: `double` needs a 64-bit float writer and this binding \
                         exports none"
                    ))
                    .into());
                }
                Op::F64(json.as_f64().ok_or_else(|| bad(path, "a number"))?)
            }
            ScalarWrite::F128 => {
                return Err(OutOfSurface(format!(
                    "{path}: `long double` is sixteen octets (XTypes 1.3 §7.4.1.1) and \
                     no binding exports a writer for it"
                ))
                .into());
            }
            ScalarWrite::Str => Op::Str(
                json.as_str()
                    .ok_or_else(|| bad(path, "a string"))?
                    .as_bytes()
                    .to_vec(),
            ),
            // The four axes a `wchar` carries are the binding's answer
            // now — two-octet units, the stream's order, no GIOP length
            // octet — so what is left here is to check that the plan
            // asked for the ones the binding gives. The corpus's key is
            // `Dialect::Specification` and resolves to exactly these;
            // another dialect would be a different measurement and is
            // refused rather than written as if it were this one.
            ScalarWrite::Wchar {
                width,
                length_octet,
                order,
            } => {
                self.wide_axes(width, length_octet, order, path)?;
                let code = code_point(json, path)?;
                Op::Wchar(Int::new(u64::from(bmp_unit(code, path)?), code))
            }
            ScalarWrite::Wstr {
                policy,
                order,
                code_units,
            } => {
                self.wstring_axes(policy, order, code_units, path)?;
                Op::Wstr(
                    json.as_str()
                        .ok_or_else(|| bad(path, "a string"))?
                        .encode_utf16()
                        .collect(),
                )
            }
            ScalarWrite::Fixed {
                digits,
                scale,
                count,
            } => {
                if count != FixedDigitCount::Declared {
                    return Err(OutOfSurface(format!(
                        "{path}: this key writes a `fixed`'s own significant digits and \
                         the binding writes the declared count (§9.3.2.8 gives the \
                         packing and not the count)"
                    ))
                    .into());
                }
                // The decimal unparsed: the padding to `digits`, the
                // nibble order, the leading zero and the sign nibble are
                // §9.3.2.8's, and handing over a digit string would be
                // this generator answering them.
                Op::Fixed {
                    value: json
                        .as_str()
                        .ok_or_else(|| bad(path, "a decimal, as a string"))?
                        .to_string(),
                    digits,
                    scale,
                }
            }
            ScalarWrite::Enum { bits } => {
                let value = self.enumerator(type_, json, path)?;
                holder(bits, value, path)?
            }
            ScalarWrite::Bitmask { bits } => {
                let value = self.bitmask(type_, json, path)?;
                holder(bits, value, path)?
            }
        };
        self.ops.push(op);
        Ok(())
    }

    /// The integer one enumerator serialises as.
    fn enumerator(&self, type_: Id<TypeNode>, json: &Value, path: &str) -> Result<i128, Refusal> {
        let resolved = self.behind_aliases(type_)?;
        let name = json
            .as_str()
            .ok_or_else(|| bad(path, "an enumerator name"))?;
        for id in self.ast.enumerators_of(resolved) {
            let node = self
                .ast
                .enumerator(*id)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            if node.name == name {
                return enumerator_wire_value(self.ast, *id, self.key.enumerator_value)
                    .ok_or_else(|| Refusal::Generator(format!("{path}: `{name}` has no value")));
            }
        }
        Err(Refusal::Generator(format!(
            "{path}: `{name}` is not an enumerator of this type"
        )))
    }

    /// Whether the key's wide-character axes are the ones the bindings
    /// answer.
    ///
    /// Not a guess and not a policy read: the resolved answer arrives on
    /// [`ScalarWrite::Wchar`], and this checks it against what
    /// `put_wchar` does. A key that asked for anything else — GIOP's
    /// length octet, four-octet units, units pinned to big-endian — is a
    /// different measurement, and writing it through this operation
    /// would report one dialect's octets under another's name.
    fn wide_axes(
        &self,
        width: WcharWidth,
        length_octet: bool,
        order: WstringUnitOrder,
        path: &str,
    ) -> Result<(), Refusal> {
        if width != WcharWidth::Two {
            return Err(OutOfSurface(format!(
                "{path}: this key gives `wchar` four octets and the binding's writer \
                 gives it two"
            ))
            .into());
        }
        if length_octet {
            return Err(OutOfSurface(format!(
                "{path}: this key puts GIOP's length octet before a `wchar` \
                 (§7.4.1.1.2) and the binding's writer emits none"
            ))
            .into());
        }
        if order != WstringUnitOrder::Stream {
            return Err(OutOfSurface(format!(
                "{path}: this key puts a `wchar`'s code units in an order other than \
                 the stream's and the binding's writer uses the stream's"
            ))
            .into());
        }
        Ok(())
    }

    /// The same, for the three axes a `wstring`'s length and units
    /// carry.
    fn wstring_axes(
        &self,
        policy: WstringLengthPolicy,
        order: WstringUnitOrder,
        code_units: WstringCodeUnits,
        path: &str,
    ) -> Result<(), Refusal> {
        if code_units != WstringCodeUnits::Utf16 {
            return Err(OutOfSurface(format!(
                "{path}: this key writes a `wstring` as its own UTF-8 octets and the \
                 binding's writer writes UTF-16 code units"
            ))
            .into());
        }
        if policy.counts != WstringLengthCounts::Octets {
            return Err(OutOfSurface(format!(
                "{path}: this key counts a `wstring`'s length in characters and the \
                 binding's writer counts octets"
            ))
            .into());
        }
        if policy.terminator != WstringTerminator::Absent {
            return Err(OutOfSurface(format!(
                "{path}: this key terminates a `wstring` and the binding's writer does \
                 not"
            ))
            .into());
        }
        if policy.unit != WstringUnitWidth::Two {
            return Err(OutOfSurface(format!(
                "{path}: this key gives a `wstring`'s units four octets and the \
                 binding's writer gives them two"
            ))
            .into());
        }
        if order != WstringUnitOrder::Stream {
            return Err(OutOfSurface(format!(
                "{path}: this key puts a `wstring`'s units in an order other than the \
                 stream's, or a byte-order mark before them, and the binding's writer \
                 does neither"
            ))
            .into());
        }
        Ok(())
    }

    /// A `bitset`: the anchor word its fields pack into, and the width
    /// of that anchor.
    ///
    /// The packing is read from the model — field *k* occupies the
    /// running sum of every preceding field's width, anonymous padding
    /// fields included, bases first — for the same reason a `bitmask`'s
    /// bit positions are: it is a property of the declaration, which a
    /// hand-written program reads from the same IDL. The anchor's width
    /// comes off the plan. What the binding then answers is which write
    /// the word goes out through and what that write aligns to.
    fn bitset(&mut self, type_: Id<TypeNode>, json: &Value, path: &str) -> Result<(), Refusal> {
        let layout = self.layout(type_)?;
        let anchor = match layout.extent {
            zerodds_ast_layout::Extent::Sized(size) => u8::try_from(size).map_err(|_| {
                Refusal::Generator(format!("{path}: a bitset anchor of {size} octets"))
            })?,
            zerodds_ast_layout::Extent::Unbounded => {
                return Err(Refusal::Generator(format!(
                    "{path}: a bitset with no fixed anchor"
                )));
            }
        };
        let object = json
            .as_object()
            .ok_or_else(|| bad(path, "an object of bitfields"))?;
        let mut bits: u64 = 0;
        let mut position: u32 = 0;
        for id in self.ast.all_bitfields_of(type_) {
            let field = self
                .ast
                .bitfield(id)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            let width = u32::from(field.bits);
            if position.saturating_add(width) > 64 {
                return Err(OutOfSurface(format!(
                    "{path}: a bitset wider than the sixty-four bits an anchor holds"
                ))
                .into());
            }
            // An anonymous field is padding: it moves the next field
            // along and carries no value of its own.
            if let Some(name) = field.name.as_ref() {
                let value = object.get(name).ok_or_else(|| {
                    Refusal::Generator(format!("{path}.{name}: the sample leaves it out"))
                })?;
                let raw = integer(value, &format!("{path}.{name}"))?;
                // Refused and never masked: a field value wider than its
                // declared width would go out as a different number and
                // the comparison would then report the truncation as
                // agreement.
                if raw < 0 || raw >= (1i128 << width) {
                    return Err(OutOfSurface(format!(
                        "{path}.{name}: {raw} does not fit {width} bits"
                    ))
                    .into());
                }
                #[expect(
                    clippy::cast_sign_loss,
                    reason = "the range check above leaves a non-negative value"
                )]
                let raw = raw as u64;
                bits |= raw << position;
            }
            position += width;
        }
        self.ops.push(Op::Bitset { bits, anchor });
        Ok(())
    }

    /// The holder word one bitmask sample sets.
    fn bitmask(&self, type_: Id<TypeNode>, json: &Value, path: &str) -> Result<i128, Refusal> {
        let resolved = self.behind_aliases(type_)?;
        let names = json.as_array().ok_or_else(|| bad(path, "a list of bits"))?;
        let mut word: u64 = 0;
        for entry in names {
            let name = entry.as_str().ok_or_else(|| bad(path, "a bit name"))?;
            let mut found = false;
            for id in self.ast.bits_of(resolved) {
                let node = self
                    .ast
                    .bit_value(*id)
                    .map_err(|error| Refusal::Generator(format!("{error}")))?;
                if node.name == name {
                    let position = self.ast.effective_bit_position(*id).ok_or_else(|| {
                        Refusal::Generator(format!("{path}: `{name}` has no position"))
                    })?;
                    word |= 1u64 << position;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(Refusal::Generator(format!(
                    "{path}: `{name}` is not a bit of this type"
                )));
            }
        }
        Ok(i128::from(word))
    }

    /// A struct or a valuetype: its members, in wire order, each framed
    /// the way the plan frames it.
    fn aggregate(&mut self, type_: Id<TypeNode>, json: &Value, path: &str) -> Result<(), Refusal> {
        let layout = self.layout(type_)?;
        let object = json
            .as_object()
            .ok_or_else(|| bad(path, "an object of members"))?;
        let by_node = self.members_by_node(type_)?;

        self.open_dheader(&layout, path)?;
        for member in &layout.members {
            let id = *by_node.get(&member.member).ok_or_else(|| {
                Refusal::Generator(format!(
                    "{path}: a member the plan named is not in the model"
                ))
            })?;
            let node = self
                .ast
                .member(id)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            let member_type = node.type_.resolved().ok_or_else(|| {
                Refusal::Generator(format!("{path}.{}: unresolved member type", node.name))
            })?;
            let child = format!("{path}.{}", node.name);
            let value = object.get(&node.name);
            match member.header {
                MemberHeader::NoHeader => {
                    let value = value.ok_or_else(|| {
                        Refusal::Generator(format!("{child}: the sample leaves it out"))
                    })?;
                    self.value(member_type, value, &child)?;
                }
                // §7.4.3.5.2 rule (20): one BOOLEAN `<is_present>`, then
                // the value when there is one.
                MemberHeader::OptionalFlag => {
                    self.ops.push(Op::Bool(value.is_some()));
                    if let Some(value) = value {
                        self.value(member_type, value, &child)?;
                    }
                }
                // An absent `@mutable` member leaves no trace at all —
                // neither under XCDR2's member header nor under XCDR1's
                // parameter list. §7.4.3.5.1.
                MemberHeader::EmHeader { .. } | MemberHeader::PidCdr1 { .. } => {
                    if let Some(value) = value {
                        let framing = self.open_member(&member.header, member.member_id, &child)?;
                        self.value(member_type, value, &child)?;
                        self.close_member(framing);
                    }
                }
                // An `@optional` member of a non-mutable XCDR1 type is
                // the other way about: rule (19) writes the header
                // whether the value is there or not, and declares zero
                // when it is not. Dropping it would leave the sample
                // four octets short of what every peer counts on and put
                // every following member at the wrong offset.
                MemberHeader::OptionalCdr1 { .. } => {
                    let framing = self.open_member(&member.header, member.member_id, &child)?;
                    if let Some(value) = value {
                        self.value(member_type, value, &child)?;
                    }
                    self.close_member(framing);
                }
            }
        }
        // §7.4.1.2: a PL_CDR1 parameter list ends in PID_LIST_END.
        if layout.needs_sentinel {
            self.ops.push(Op::PidListEnd);
        }
        self.close_dheader(&layout);
        Ok(())
    }

    /// Open one member's header, and say what closes it.
    fn open_member(
        &mut self,
        header: &MemberHeader,
        member_id: u32,
        path: &str,
    ) -> Result<Framing, Refusal> {
        match *header {
            MemberHeader::EmHeader {
                lc,
                must_understand,
            } => {
                let lc = Lc(lc.bits());
                self.ops.push(Op::EmHeaderBegin {
                    id: member_id,
                    must_understand,
                    lc,
                });
                Ok(Framing::Em(lc))
            }
            MemberHeader::PidCdr1 {
                pid,
                extended,
                must_understand,
            }
            | MemberHeader::OptionalCdr1 {
                pid,
                extended,
                must_understand,
            } => {
                if extended {
                    self.ops.push(Op::PidExtendedBegin {
                        member_id,
                        must_understand,
                    });
                } else {
                    self.ops.push(Op::PidBegin {
                        pid,
                        must_understand,
                    });
                }
                Ok(Framing::Pid)
            }
            MemberHeader::NoHeader | MemberHeader::OptionalFlag => Err(Refusal::Generator(
                format!("{path}: this header does not open a framed member"),
            )),
        }
    }

    fn close_member(&mut self, framing: Framing) {
        match framing {
            Framing::None => {}
            Framing::Em(lc) => self.ops.push(Op::EmHeaderEnd { lc }),
            Framing::Pid => self.ops.push(Op::PidEnd),
        }
    }

    /// A union: the discriminator, then the arm it selects.
    fn union(&mut self, type_: Id<TypeNode>, json: &Value, path: &str) -> Result<(), Refusal> {
        let layout = self.layout(type_)?;
        let object = json
            .as_object()
            .ok_or_else(|| bad(path, "an object with `_d`"))?;
        let selector = object
            .get("_d")
            .ok_or_else(|| Refusal::Generator(format!("{path}: the sample has no `_d`")))?;

        let node = self
            .ast
            .type_(type_)
            .map_err(|error| Refusal::Generator(format!("{error}")))?;
        let TypeKind::Union {
            discriminator,
            cases,
            default_case,
        } = &node.kind
        else {
            return Err(Refusal::Generator(format!("{path}: not a union")));
        };
        let disc_type = discriminator
            .resolved()
            .ok_or_else(|| Refusal::Generator(format!("{path}: unresolved discriminator")))?;

        self.open_dheader(&layout, path)?;
        // Rules (27) and (28) frame a `@mutable` union's discriminator
        // exactly like a member — at member id 0, with the header its
        // own type earns. `TypeLayout::discriminator` carries that
        // header and is `None` for every other extensibility, where
        // rules (26) and (30) write the discriminator bare.
        let disc_framing = match &layout.discriminator {
            None => Framing::None,
            Some(header) => self.open_member(header, 0, &format!("{path}._d"))?,
        };
        self.value(disc_type, selector, &format!("{path}._d"))?;
        self.close_member(disc_framing);

        let wanted = self.label_value(disc_type, selector, path)?;
        let mut selected = None;
        for case in cases {
            let case_node = self
                .ast
                .case(*case)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            for label in &case_node.labels {
                if let Some(expr) = label.expr() {
                    let value = self
                        .ast
                        .eval_integral(expr)
                        .map_err(|error| Refusal::Generator(format!("{path}: {error:?}")))?;
                    if value == wanted {
                        selected = Some(case_node.member);
                    }
                }
            }
            if selected.is_some() {
                break;
            }
        }
        // No label matched, so the `default:` arm is the one written —
        // and where there is none, the sample is the discriminator
        // alone. XTypes 1.3 §7.4.3.5.4.
        if selected.is_none() {
            if let Some(default) = default_case {
                let case_node = self
                    .ast
                    .case(*default)
                    .map_err(|error| Refusal::Generator(format!("{error}")))?;
                selected = Some(case_node.member);
            }
        }
        if let Some(member) = selected {
            let member_node = self
                .ast
                .member(member)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            let member_type = member_node.type_.resolved().ok_or_else(|| {
                Refusal::Generator(format!("{path}.{}: unresolved arm", member_node.name))
            })?;
            let child = format!("{path}.{}", member_node.name);
            let value = object.get(&member_node.name).ok_or_else(|| {
                Refusal::Generator(format!("{child}: the selected arm is not in the sample"))
            })?;
            // A `@mutable` union frames the selected branch as a member
            // too, and `TypeLayout::members` holds one plan per case for
            // exactly that. Every other extensibility writes the branch
            // bare, and `members` is then empty.
            let plan = layout
                .members
                .iter()
                .find(|plan| plan.member == member_node.node_id);
            let framing = match plan {
                None => Framing::None,
                Some(plan) => self.open_member(&plan.header, plan.member_id, &child)?,
            };
            self.value(member_type, value, &child)?;
            self.close_member(framing);
        }
        if layout.needs_sentinel {
            self.ops.push(Op::PidListEnd);
        }
        self.close_dheader(&layout);
        Ok(())
    }

    /// The integer a discriminator value compares as.
    fn label_value(&self, disc: Id<TypeNode>, json: &Value, path: &str) -> Result<i128, Refusal> {
        match scalar_write(self.ast, disc, self.key, WcharPosition::Member) {
            Some(ScalarWrite::Bool) => Ok(i128::from(
                json.as_bool().ok_or_else(|| bad(path, "a boolean"))?,
            )),
            // A character discriminator arrives as the character, not as
            // a number, and that is true of the wide one too. `wchar`
            // fell through to the integer arm and every
            // `union-discriminator-wchar` cell came back
            // `generator-defect: `ä`: invalid digit found in string` —
            // the case labels never got compared and no arm was
            // selected.
            Some(ScalarWrite::Char | ScalarWrite::Wchar { .. }) => code_point(json, path),
            Some(ScalarWrite::Enum { .. }) => self.enumerator(disc, json, path),
            Some(_) => integer(json, path),
            None => Err(Refusal::Generator(format!(
                "{path}: the discriminator is not a scalar"
            ))),
        }
    }

    /// A sequence: the element count, then the elements.
    fn collection(
        &mut self,
        type_: Id<TypeNode>,
        element: Id<TypeNode>,
        json: &Value,
        path: &str,
        counted: bool,
    ) -> Result<(), Refusal> {
        let layout = self.layout(type_)?;
        let items = json.as_array().ok_or_else(|| bad(path, "an array"))?;

        // A run of one-octet elements is exactly what `put_seq_u8`
        // writes — the count and the octets — so it goes out through
        // that call rather than through a hand-rolled loop, which is
        // the only way the operation gets exercised at all.
        if counted && !layout.needs_dheader && self.octet_wide(element) {
            let mut bytes = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                let child = format!("{path}[{index}]");
                bytes.push(self.one_octet(element, item, &child)?.octet());
            }
            self.ops.push(Op::SeqU8(bytes));
            return Ok(());
        }

        self.open_dheader(&layout, path)?;
        if counted {
            self.ops.push(Op::Count(count(items.len()).ok_or_else(|| {
                Refusal::Generator(format!("{path}: {} elements", items.len()))
            })?));
        }
        for (index, item) in items.iter().enumerate() {
            self.value(element, item, &format!("{path}[{index}]"))?;
        }
        self.close_dheader(&layout);
        Ok(())
    }

    /// An array: the elements, and no count — the dimensions are in the
    /// type.
    fn array(
        &mut self,
        type_: Id<TypeNode>,
        element: Id<TypeNode>,
        json: &Value,
        path: &str,
        expected: u64,
        dims: usize,
    ) -> Result<(), Refusal> {
        // A multi-dimensional array is one flat run on the wire
        // (§7.4.3.5), and the sample nests one list per dimension, so
        // exactly `dims - 1` levels of nesting come out here. Not one
        // level more: an array *of* sequences, of maps, of bitmasks or
        // of a typedef'd array nests in the sample too, and flattening
        // by shape rather than by dimension count would pull those
        // apart and lose the element boundaries with them.
        let mut flat = Vec::new();
        flatten(json, dims.saturating_sub(1), &mut flat, path)?;
        if flat.len() as u64 != expected {
            return Err(Refusal::Generator(format!(
                "{path}: the type has {expected} elements and the sample has {}",
                flat.len()
            )));
        }
        let layout = self.layout(type_)?;
        if !layout.needs_dheader && self.octet_wide(element) {
            let mut bytes = Vec::with_capacity(flat.len());
            for (index, item) in flat.iter().enumerate() {
                bytes.push(self.one_octet(element, item, &format!("{path}[{index}]"))?);
            }
            // Not `put_seq_u8`: an array carries no count. Each octet
            // goes out through the octet writer.
            for byte in bytes {
                self.ops.push(Op::U8(byte));
            }
            return Ok(());
        }
        self.open_dheader(&layout, path)?;
        for (index, item) in flat.iter().enumerate() {
            self.value(element, item, &format!("{path}[{index}]"))?;
        }
        self.close_dheader(&layout);
        Ok(())
    }

    /// A map: the entry count, then each key and value.
    fn map(
        &mut self,
        type_: Id<TypeNode>,
        k: Id<TypeNode>,
        v: Id<TypeNode>,
        json: &Value,
        path: &str,
    ) -> Result<(), Refusal> {
        let layout = self.layout(type_)?;
        let entries = json
            .as_array()
            .ok_or_else(|| bad(path, "an array of two-element arrays"))?;
        self.open_dheader(&layout, path)?;
        self.ops.push(Op::Count(count(entries.len()).ok_or_else(|| {
            Refusal::Generator(format!("{path}: {} entries", entries.len()))
        })?));
        for (index, entry) in entries.iter().enumerate() {
            let pair = entry
                .as_array()
                .ok_or_else(|| bad(&format!("{path}[{index}]"), "a two-element array"))?;
            if pair.len() != 2 {
                return Err(bad(&format!("{path}[{index}]"), "a two-element array"));
            }
            self.value(k, &pair[0], &format!("{path}[{index}].key"))?;
            self.value(v, &pair[1], &format!("{path}[{index}].value"))?;
        }
        self.close_dheader(&layout);
        Ok(())
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the signature is the call sites', which are `?`-chained through \
                  the walk; it returned a refusal while the delimiter was outside \
                  fourteen of the fifteen surfaces and the shape is kept so a \
                  binding that loses it again fails here and not silently"
    )]
    fn open_dheader(&mut self, layout: &TypeLayout, _path: &str) -> Result<(), Refusal> {
        if layout.needs_dheader {
            self.ops.push(Op::DHeaderBegin);
        }
        Ok(())
    }

    fn close_dheader(&mut self, layout: &TypeLayout) {
        if layout.needs_dheader {
            self.ops.push(Op::DHeaderEnd);
        }
    }

    /// Whether one element of this type is exactly one octet.
    fn octet_wide(&self, element: Id<TypeNode>) -> bool {
        matches!(
            scalar_write(
                self.ast,
                element,
                self.key,
                WcharPosition::inside_collection()
            ),
            Some(ScalarWrite::U8 | ScalarWrite::I8 | ScalarWrite::Char | ScalarWrite::Bool)
        )
    }

    fn one_octet(&self, element: Id<TypeNode>, json: &Value, path: &str) -> Result<Int, Refusal> {
        match scalar_write(
            self.ast,
            element,
            self.key,
            WcharPosition::inside_collection(),
        ) {
            Some(ScalarWrite::Bool) => {
                let flag = json.as_bool().ok_or_else(|| bad(path, "a boolean"))?;
                Ok(Int::new(u64::from(u8::from(flag)), i128::from(flag)))
            }
            Some(ScalarWrite::Char) => {
                let code = code_point(json, path)?;
                Ok(Int::new(u64::from(octet_char(code, path)?), code))
            }
            Some(ScalarWrite::U8 | ScalarWrite::I8) => widened(json, 8, path),
            _ => Err(Refusal::Generator(format!("{path}: not a one-octet type"))),
        }
    }

    fn members_by_node(
        &self,
        type_: Id<TypeNode>,
    ) -> Result<std::collections::HashMap<NodeId, Id<MemberNode>>, Refusal> {
        let mut map = std::collections::HashMap::new();
        for id in self.ast.all_members_of(type_) {
            let node = self
                .ast
                .member(id)
                .map_err(|error| Refusal::Generator(format!("{error}")))?;
            map.insert(node.node_id, id);
        }
        Ok(map)
    }
}

/// The one call an enumeration or bitmask holder of `bits` takes.
fn holder(bits: u16, value: i128, path: &str) -> Result<Op, Refusal> {
    #[expect(
        clippy::cast_sign_loss,
        reason = "a negative enumerator reaches the wire as its two's complement \
                  pattern in the holder's width, which is what the mask takes"
    )]
    let pattern = value as u128;
    let masked = |width: u32| {
        Int::new(
            u64::try_from(pattern & ((1u128 << width) - 1)).unwrap_or(0),
            value,
        )
    };
    Ok(match bits {
        0..=8 => Op::U8(masked(8)),
        9..=16 => Op::U16(masked(16)),
        17..=32 => Op::U32(masked(32)),
        33..=64 => Op::U64(masked(64)),
        other => {
            return Err(Refusal::Generator(format!(
                "{path}: a holder of {other} bits"
            )));
        }
    })
}

/// Take `levels` of nesting out of a sample list.
///
/// By count and not by shape: `levels` comes from the number of
/// dimensions the array declares, so an element that is itself a list —
/// a sequence, a map, a bitmask, a typedef'd array — arrives whole.
///
/// zerodds-lint: recursion-depth = the array's declared dimension count
/// (`levels` is one lower on every call and the descent stops at zero, so
/// the IDL declaration bounds it; the sample's own nesting does not)
fn flatten(json: &Value, levels: usize, out: &mut Vec<Value>, path: &str) -> Result<(), Refusal> {
    let items = json.as_array().ok_or_else(|| bad(path, "an array"))?;
    for item in items {
        if levels == 0 {
            out.push(item.clone());
        } else {
            flatten(item, levels - 1, out, path)?;
        }
    }
    Ok(())
}

/// An integer from the sample, in the dialect
/// `crates/wire-probe/src/lib.rs` documents: a 64-bit value arrives as a
/// string, because JSON has one number type and it is a double.
fn integer(json: &Value, path: &str) -> Result<i128, Refusal> {
    if let Some(value) = json.as_i64() {
        return Ok(i128::from(value));
    }
    if let Some(value) = json.as_u64() {
        return Ok(i128::from(value));
    }
    if let Some(text) = json.as_str() {
        return text
            .parse::<i128>()
            .map_err(|error| Refusal::Generator(format!("{path}: `{text}`: {error}")));
    }
    if let Some(value) = json.as_bool() {
        return Ok(i128::from(value));
    }
    Err(bad(path, "an integer"))
}

/// The code point of a one-character sample value.
fn code_point(json: &Value, path: &str) -> Result<i128, Refusal> {
    if let Some(value) = json.as_i64() {
        return Ok(i128::from(value));
    }
    let text = json.as_str().ok_or_else(|| bad(path, "a character"))?;
    let mut chars = text.chars();
    let first = chars
        .next()
        .ok_or_else(|| Refusal::Generator(format!("{path}: an empty character")))?;
    if chars.next().is_some() {
        return Err(Refusal::Generator(format!(
            "{path}: `{text}` is more than one character"
        )));
    }
    Ok(i128::from(u32::from(first)))
}

/// The two's complement pattern of `value` in a holder of `bits`, or a
/// refusal when it does not fit one.
///
/// Refused and not masked. A sample whose value is wider than the
/// member carrying it would go out as a different number, and the
/// comparison against the Rust column would then report the truncation
/// as agreement -- the one outcome a measurement must never produce.
/// The bound is the union of the signed and the unsigned range, because
/// a holder of `bits` carries either and the pattern is the same.
fn narrow(value: i128, bits: u32, path: &str) -> Result<u128, Refusal> {
    let unsigned_max = (1i128 << bits) - 1;
    let signed_min = -(1i128 << (bits - 1));
    if value > unsigned_max || value < signed_min {
        // `no-form` and not a defect of this generator: a value wider
        // than the member declared has no form in *any* column, and
        // every one of them refuses it rather than writing a different
        // number.
        return Err(OutOfSurface(format!("{path}: {value} does not fit {bits} bits")).into());
    }
    #[expect(
        clippy::cast_sign_loss,
        reason = "a negative value inside the signed range reaches the wire as its \
                  two's complement pattern, which the mask below takes"
    )]
    let pattern = value as u128;
    Ok(pattern & ((1u128 << bits) - 1))
}

/// A length word, or nothing when the collection has more elements than
/// the four-octet count XTypes 1.3 §7.4.3.5.3 gives it.
fn count(len: usize) -> Option<Int> {
    let count = u32::try_from(len).ok()?;
    Some(Int::new(u64::from(count), i128::from(count)))
}

/// One integer argument of a `bits`-wide write: the sample's value, and
/// the two's complement pattern [`narrow`] resolved for it.
///
/// Both, because the two spellings of an argument in
/// [`crate::ops::Form`] are the two ends of exactly this conversion, and
/// which end a binding takes is what `endpoint-number-surface` measures.
/// The refusal is [`narrow`]'s and happens before either is kept, so a
/// value outside the holder has no argument in either form.
fn widened(json: &Value, bits: u32, path: &str) -> Result<Int, Refusal> {
    let value = integer(json, path)?;
    let pattern = narrow(value, bits, path)?;
    Ok(Int::new(u64::try_from(pattern).unwrap_or(0), value))
}

/// One octet of `char`, or a refusal.
///
/// XTypes 1.3 §7.4.1.1.1 Table 30 gives `char` one octet. A code point
/// that needs more has no `char` form, and truncating it to its low
/// octet would write a different character and call it the same one.
fn octet_char(code: i128, path: &str) -> Result<u8, Refusal> {
    u8::try_from(code).map_err(|_| {
        OutOfSurface(format!(
            "{path}: expected a character below U+0100, found U+{code:04X}"
        ))
        .into()
    })
}

/// One two-octet code unit of `wchar`, or a refusal.
///
/// The refusal is the code point's and not the width's: a four-octet
/// `wchar` zero-extends a code *unit*, not a code point, so U+10000 has
/// no `wchar` form in either width. The Rust column refuses
/// `primitive-wchar-above-bmp` in the same words and records no octets
/// for it at all.
fn bmp_unit(code: i128, path: &str) -> Result<u16, Refusal> {
    u16::try_from(code).map_err(|_| {
        OutOfSurface(format!(
            "{path}: expected a BMP character, found U+{code:04X}"
        ))
        .into()
    })
}

fn bad(path: &str, wanted: &str) -> Refusal {
    Refusal::Generator(format!("{path}: the sample is not {wanted}"))
}

const fn kind_name(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Native => "native",
        TypeKind::Any => "any",
        TypeKind::ValueBase => "ValueBase",
        TypeKind::ValueBox { .. } => "a boxed value",
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => "wstring",
        _ => "this construct",
    }
}
