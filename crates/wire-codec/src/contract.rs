// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a value file means, in one place.
//!
//! Three backends read the same value file: [`crate::encode()`] turns it
//! into bytes, `zerodds_wire_probe::fill` into C for Cyclone and RTI,
//! and `zerodds_wire_probe::fill_cpp` into C++ for Fast DDS. Every
//! question about the file — is this key a member, may it be `null`, is
//! this integer expressible, which members does this aggregate have —
//! has to be answered the same way by all three, because a difference is
//! a difference in *the sample* and the matrix would read it as a
//! difference in the serialisers.
//!
//! The two backends that are not here still call these functions: the
//! rules are generic over [`Source`] and every one of them is
//! monomorphised at the same `serde_json::Value` the encoder uses. A
//! backend that lived in a different crate and re-derived the rules
//! would be back where defect 11 was.
//!
//! They agreed once by inspection and diverged twice by edit. Defect 11
//! of `docs/silent-drop-sweep.md` is one half of that divergence: the C
//! emitter walked `members_of` where the other two walked
//! `all_members_of`, so a derived type's base members were never filled
//! and the vendor serialised a structure of the right length with zeros
//! in it. Nothing said so, because each backend held its own copy of the
//! rule.
//!
//! So the rules live here and the backends call them. A rule that is
//! only in one backend is a rule the other two can contradict.
//!
//! ## What is recorded
//!
//! Sharing the decisions is half of it; the other half is checking that
//! the backends actually reach the same parts of the value. Each backend
//! threads a [`Consumed`] and pushes the path of every **scalar** it
//! reads out of the document. Two backends that accept the same file and
//! consumed different paths disagree about what the sample is, whatever
//! their exit codes said. `tests/probe_agreement.rs` is what compares
//! them.

use crate::encode::ProbeError;
use crate::source::{Fields, Source};
use std::string::String;
use std::vec::Vec;
use zerodds_ast::{Ast, Bound, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_float128::Float128;

/// Where each backend read a scalar out of the value document.
///
/// Paths only, in visit order. The comparison sorts, because the
/// backends visit a union's discriminator at different points and that
/// is not a disagreement about the sample.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Consumed(Vec<String>);

impl Consumed {
    /// A fresh recorder.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Record that a scalar at `path` was read.
    pub fn scalar(&mut self, path: &str) {
        self.0.push(path.to_string());
    }

    /// The paths, sorted, for comparison between backends.
    #[must_use]
    pub fn sorted(&self) -> Vec<String> {
        let mut out = self.0.clone();
        out.sort();
        out
    }

    /// The paths in visit order.
    #[must_use]
    pub fn paths(&self) -> &[String] {
        &self.0
    }

    /// Take another recorder's paths into this one.
    ///
    /// One caller: the union storage a suppressed case label makes the
    /// writer synthesise. It writes the **named** member into a scratch
    /// sink to get its octets, and the scratch sink has a recorder of
    /// its own — so the read of that member was happening and no
    /// recorder said so. `probe_agreement.rs` found it the day
    /// `Dialect::Cyclone` took the suppressed label: the Rust backend
    /// reported reading `picked.fraction`, which is the branch it wrote,
    /// and the two fill backends reported `picked.whole`, which is the
    /// member the sample names and the one all three actually read.
    pub fn absorb(&mut self, other: &Self) {
        self.0.extend(other.0.iter().cloned());
    }
}

/// One member of an aggregate, as all three backends see it.
#[derive(Debug, Clone)]
pub struct Member {
    /// The member's stable identity, which is how the layout plan names
    /// it. Carried here so the serialiser can match a planned member to
    /// its JSON key without a second walk of the model.
    pub node: zerodds_ast_core::NodeId,
    /// The member's simple name, which is also its JSON key.
    pub name: String,
    /// The member's type, resolved.
    pub type_: Id<TypeNode>,
    /// Whether the key may be left out.
    pub optional: bool,
}

/// The path the root of a value document is named by.
///
/// The three backends report a refusal by path, and a path that differs
/// between them cannot be compared. Taken from the model rather than
/// from the caller's spelling, so `::common::Robot` and `common::Robot`
/// name the same place.
#[must_use]
pub fn root_path(ast: &Ast, type_: Id<TypeNode>) -> String {
    ast.any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .map_or_else(String::new, |scoped| scoped.segments().join("::"))
}

/// Every member an aggregate has, a base's before its own.
///
/// `Ast::members_of` answers with a type's **own** members. A backend
/// that walks it and treats the answer as the whole one writes half a
/// sample: XTypes 1.3 §7.4.1.1.5.1 puts the base's members on the wire
/// first, "as if all of the members had been defined, in the same
/// order, in the most-derived type". Defect 11 of the sweep.
#[must_use]
pub fn members(ast: &Ast, type_: Id<TypeNode>) -> Vec<Member> {
    ast.all_members_of(type_)
        .into_iter()
        .filter_map(|id| {
            let member = ast.member(id).ok()?;
            Some(Member {
                node: member.node_id,
                name: member.name.clone(),
                type_: member.type_.resolved()?,
                optional: member.facets.is_optional(),
            })
        })
        .collect()
}

/// A union's members, by name, in case order.
///
/// A union's members hang off its cases; `members_of` does not answer
/// for one.
#[must_use]
pub fn union_members(ast: &Ast, type_: Id<TypeNode>) -> Vec<(String, Id<TypeNode>)> {
    union_cases(ast, type_)
        .into_iter()
        .map(|case| (case.name, case.type_))
        .collect()
}

/// One case member of a union.
///
/// The identity as well as the name, because a `@mutable` union's plan
/// names its members the way an aggregate's does — by node — and a walk
/// that had only the name would have to match on a string.
#[derive(Debug, Clone)]
pub struct Case {
    /// The member's stable identity.
    pub node: zerodds_ast_core::NodeId,
    /// Its simple name, which is also its JSON key.
    pub name: String,
    /// Its type, resolved.
    pub type_: Id<TypeNode>,
    /// `@try_construct` as the case member wrote it. §7.2.2.7 lists
    /// union members alongside structure members.
    pub try_construct: Option<zerodds_ast::TryConstruct>,
    /// The labels selecting this case, in declaration order.
    ///
    /// Carried because [`union_selection`] compares them: the probe
    /// evaluates labels only where a setting says it must, and it needs
    /// them in the same walk that gives it the member.
    pub labels: Vec<zerodds_ast::CaseLabel>,
}

impl Case {
    /// Whether one of this case's labels is `default:`.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.labels.iter().any(|label| label.is_default())
    }
}

/// The case a discriminator value selects, under
/// [`zerodds_ast_layout::UnionLabelMatch`].
///
/// `suppress_negative` is
/// [`zerodds_ast_layout::union_negative_matches_nothing`] for this
/// union's discriminator — the crossing, asked once by the caller and
/// passed in, so that a walk cannot ask it of the wrong type.
///
/// IDL 4.2 §7.4.1.4.4.2.4: the `default:` member is the one selected by
/// any value no label names. A union with no `default:` and no matching
/// label selects nothing, which is a case the corpus carries
/// (`union-default-unselected`) and not an error.
///
/// # Errors
/// [`UnevaluatedLabel`] for a label whose constant expression the model
/// cannot evaluate. Skipping it would turn a label that should have
/// matched into one that matches nothing, and the walk would select a
/// different branch — or none — and say nothing.
pub fn union_selection(
    ast: &Ast,
    cases: &[Case],
    discriminator: i128,
    suppress_negative: bool,
) -> Result<Option<usize>, UnevaluatedLabel> {
    let mut fallback = None;
    for (index, case) in cases.iter().enumerate() {
        for label in &case.labels {
            match label.expr() {
                None => fallback = Some(index),
                Some(expr) => {
                    // Reported and not skipped. A skipped label is a
                    // label that matches nothing, so the walk goes on to
                    // `default:` or off the end and the writer emits a
                    // branch the sample did not select — or emits none —
                    // and returns success. The Python column refuses the
                    // same construct in `emit.rs::label_value`; this is
                    // the same refusal on the probe's side.
                    let value = ast
                        .eval_integral(expr)
                        .map_err(|_| UnevaluatedLabel { case: index })?;
                    if suppress_negative && value < 0 {
                        continue;
                    }
                    if value == discriminator {
                        return Ok(Some(index));
                    }
                }
            }
        }
    }
    Ok(fallback)
}

/// A case label whose constant expression the model cannot evaluate.
///
/// Carries the case's index and not a message: the caller holds the
/// path and the case's name, which is what makes the refusal
/// actionable.
#[derive(Debug, Clone, Copy)]
pub struct UnevaluatedLabel {
    /// Which case of the union the label belongs to.
    pub case: usize,
}

/// A union's case members, in case order.
///
/// A union's members hang off its cases; `members_of` does not answer
/// for one.
#[must_use]
pub fn union_cases(ast: &Ast, type_: Id<TypeNode>) -> Vec<Case> {
    ast.cases_of(type_)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| {
            let member = ast.member(case.member).ok()?;
            Some(Case {
                node: member.node_id,
                name: member.name.clone(),
                type_: member.type_.resolved()?,
                try_construct: member.facets.xtypes.as_ref().and_then(|x| x.try_construct),
                labels: case.labels.clone(),
            })
        })
        .collect()
}

/// The names of a bitset's own bitfields, anonymous padding excluded.
#[must_use]
pub fn bitfield_names(ast: &Ast, fields: &[Id<zerodds_ast::BitfieldNode>]) -> Vec<String> {
    fields
        .iter()
        .filter_map(|id| ast.bitfield(*id).ok())
        .filter_map(|field| field.name.clone())
        .collect()
}

/// Refuse a key that names nothing the type declares.
///
/// An error and not a warning: a misspelled member name would otherwise
/// silently test the default value, and the two emitters that skipped it
/// produced a sample the third one refused.
///
/// # Errors
/// [`ProbeError::Unknown`], naming the offending key's path.
pub fn reject_unknown_keys<V: Source>(
    object: Fields<'_, V>,
    declared: &[String],
    path: &str,
) -> Result<(), ProbeError> {
    for name in object.names() {
        if !declared.iter().any(|known| known == name) {
            return Err(ProbeError::Unknown {
                path: join(path, name),
                declared: declared.to_vec(),
            });
        }
    }
    Ok(())
}

/// The value for one member, or `None` where an `@optional` is absent.
///
/// The two rules the corpus README states, in the one place all three
/// backends ask:
///
/// - a member with no key is absent, which only an `@optional` may be;
/// - `null` is never a value and never an absence. An absent optional
///   member is a key that is **left out**. `null` would read as "present
///   and empty" to one backend and as "absent" to another, and the
///   vendors write nothing at all for an absent optional — measured
///   across all three, `vendor_bytes.md` §"Everything else all three
///   agree on".
///
/// # Errors
/// [`ProbeError::Missing`] for a required member with no key,
/// [`ProbeError::Mismatch`] for `null`.
pub fn member_value<'a, V: Source>(
    object: Fields<'a, V>,
    member: &Member,
    path: &str,
) -> Result<Option<&'a V>, ProbeError> {
    let member_path = join(path, &member.name);
    match object.get(&member.name) {
        Some(value) if value.is_null() => Err(ProbeError::Mismatch {
            path: member_path,
            expected: "a value, or the key left out for an absent @optional".to_string(),
            found: "null".to_string(),
        }),
        Some(value) => Ok(Some(value)),
        None if member.optional => Ok(None),
        None => Err(ProbeError::Missing { path: member_path }),
    }
}

/// Refuse `null` wherever a value belongs.
///
/// The counterpart of [`member_value`] for a place that has no
/// optionality to consider — a union's selected member, an element of a
/// collection. Same rule, same message, so the three backends report
/// the same refusal.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn reject_null<V: Source>(value: &V, path: &str) -> Result<(), ProbeError> {
    if value.is_null() {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "a value, or the key left out for an absent @optional".to_string(),
            found: "null".to_string(),
        });
    }
    Ok(())
}

/// A union's discriminator value, which is never absent.
///
/// # Errors
/// [`ProbeError::Missing`] when `_d` is not in the object.
pub fn discriminator<'a, V: Source>(
    object: Fields<'a, V>,
    path: &str,
) -> Result<&'a V, ProbeError> {
    object.get("_d").ok_or_else(|| ProbeError::Missing {
        path: join(path, "_d"),
    })
}

/// `<path>.<name>`, or `<name>` at the root of a nameless walk.
#[must_use]
pub fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{path}.{name}")
    }
}

// -- scalars ---------------------------------------------------------

/// An object.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn object<'a, V: Source>(value: &'a V, path: &str) -> Result<Fields<'a, V>, ProbeError> {
    if value.is_object() {
        return Ok(Fields::of(value));
    }
    Err(ProbeError::Mismatch {
        path: path.to_string(),
        expected: "an object".to_string(),
        found: kind_of(value).to_string(),
    })
}

/// An array.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn array<'a, V: Source>(value: &'a V, path: &str) -> Result<&'a [V], ProbeError> {
    value.as_items().ok_or_else(|| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "an array".to_string(),
        found: kind_of(value).to_string(),
    })
}

/// A string.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn text<'a, V: Source>(value: &'a V, path: &str) -> Result<&'a str, ProbeError> {
    value.as_text().ok_or_else(|| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "a string".to_string(),
        found: kind_of(value).to_string(),
    })
}

/// `true` or `false`.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn boolean<V: Source>(value: &V, path: &str) -> Result<bool, ProbeError> {
    value.as_bool().ok_or_else(|| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "true or false".to_string(),
        found: kind_of(value).to_string(),
    })
}

/// A floating-point value.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn float<V: Source>(value: &V, path: &str) -> Result<f64, ProbeError> {
    value.as_float().ok_or_else(|| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "a number".to_string(),
        found: kind_of(value).to_string(),
    })
}

/// A `long double`.
///
/// The spelling is the representation's rule and is stated by
/// [`Source::as_float128`]; this is where the type asks for one, so that a
/// caller does not reach past the contract to get it.
///
/// # Errors
/// [`ProbeError::Mismatch`] for a value the representation cannot read
/// as a `long double`.
pub fn float128<V: Source>(value: &V, path: &str) -> Result<Float128, ProbeError> {
    value.as_float128(path)
}

/// Exactly one character.
///
/// A `char` field is one character, and a longer string is refused
/// rather than cut down to its first: two backends took `.next()` and
/// put `'a'` on the wire for `"ab"`, which is a sample nobody wrote.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn character<V: Source>(value: &V, path: &str) -> Result<char, ProbeError> {
    let text = text(value, path)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ch),
        _ => Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "exactly one character".to_string(),
            found: format!("`{text}`"),
        }),
    }
}

/// Exactly one character, of a width that can carry it.
///
/// The width check belongs beside `integer_for`'s range check and for
/// the same reason: a `char` holding `š` is a value file that says
/// something the type cannot hold, and the two C emitters wrote
/// `'\u{161}'` into a one-byte field where the compiler narrows it
/// without a word. DDS-XTypes 1.3 §7.2.2.2.1.2.2 gives `CHAR_8_TYPE`
/// eight bits and no encoding; §7.2.2.2.1.2.5 restricts `CHAR_16_TYPE`
/// to the Basic Multilingual Plane, "Unicode codepoints from 0x0000 to
/// U+FFFF".
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn character_for<V: Source>(
    kind: PrimitiveKind,
    value: &V,
    path: &str,
) -> Result<char, ProbeError> {
    let found = character(value, path)?;
    let point = u32::from(found);
    let (limit, expected) = match kind {
        PrimitiveKind::Char => (0xff, "a character below U+0100"),
        PrimitiveKind::WChar => (0xffff, "a character in the Basic Multilingual Plane"),
        // Not characters. A caller that routed one here is asking the
        // wrong question, and answering it would be the silent part.
        PrimitiveKind::Boolean
        | PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64
        | PrimitiveKind::Float32
        | PrimitiveKind::Float64
        | PrimitiveKind::Float128 => {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "a character type".to_string(),
                found: format!("{kind:?}"),
            });
        }
    };
    if point > limit {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: expected.to_string(),
            found: format!("U+{point:04X}"),
        });
    }
    Ok(found)
}

/// An integer, in whatever spelling the representation admits.
///
/// Which spellings those are is the representation's own rule and is
/// stated by [`Source::as_integer`] — a JSON number is a double and stops
/// carrying a 64-bit integer exactly past 2^53, which is a fact about
/// JSON and not about IDL. This is where the *type* asks for an
/// integer, so that no caller reaches past the contract to get one and
/// no backend accepts a spelling the other two refuse.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn integer<V: Source>(value: &V, path: &str) -> Result<i128, ProbeError> {
    value.as_integer(path)
}

/// An integer that fits the primitive it is destined for.
///
/// The range check belongs with the dialect and not with the
/// serialiser: an `octet` of `300` is a value file that says something
/// the type cannot hold, and a backend that emitted `300` into a C
/// `uint8_t` would put `44` on the wire without a word.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn integer_for<V: Source>(
    kind: PrimitiveKind,
    value: &V,
    path: &str,
) -> Result<i128, ProbeError> {
    let raw = integer(value, path)?;
    let (low, high, name): (i128, i128, &str) = match kind {
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => (0, 0xff, "an octet"),
        PrimitiveKind::Int8 => (-0x80, 0x7f, "an int8"),
        PrimitiveKind::Int16 => (-0x8000, 0x7fff, "a short"),
        PrimitiveKind::UInt16 => (0, 0xffff, "an unsigned short"),
        PrimitiveKind::Int32 => (-0x8000_0000, 0x7fff_ffff, "a long"),
        PrimitiveKind::UInt32 => (0, 0xffff_ffff, "an unsigned long"),
        PrimitiveKind::Int64 => (-0x8000_0000_0000_0000, 0x7fff_ffff_ffff_ffff, "a long long"),
        PrimitiveKind::UInt64 => (0, 0xffff_ffff_ffff_ffff, "an unsigned long long"),
        // Not integers. A caller that routed one here is asking the
        // wrong question, and answering it would be the silent part.
        PrimitiveKind::Boolean
        | PrimitiveKind::Char
        | PrimitiveKind::WChar
        | PrimitiveKind::Float32
        | PrimitiveKind::Float64
        | PrimitiveKind::Float128 => {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: "an integral type".to_string(),
                found: format!("{kind:?}"),
            });
        }
    };
    if raw < low || raw > high {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("{name} in range {low}..={high}"),
            found: format!("{raw}"),
        });
    }
    Ok(raw)
}

// -- named things ----------------------------------------------------

/// The enumerator a value names, and its effective value.
///
/// The name is checked against the declared set by all three backends.
/// Two of them used to paste the name into an identifier without
/// looking: a typo compiled into a different enumerator, or did not
/// compile at all in a place the matrix reads as a vendor difference.
///
/// # Errors
/// [`ProbeError::Mismatch`] for a value that is not a string,
/// [`ProbeError::Unknown`] for a name the enumeration does not declare.
pub fn enumerator<V: Source>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<(String, i128), ProbeError> {
    let name = text(value, path).map_err(|_| ProbeError::Mismatch {
        path: path.to_string(),
        expected: "an enumerator name, as a string".to_string(),
        found: kind_of(value).to_string(),
    })?;
    let mut declared = Vec::new();
    let mut found = None;
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        declared.push(node.name.clone());
        if node.name == name {
            found = ast.effective_enumerator_value(*id);
        }
    }
    let value = found.ok_or_else(|| ProbeError::Unknown {
        path: path.to_string(),
        declared,
    })?;
    Ok((name.to_string(), value))
}

/// The bit names a bitmask value sets, each with its position.
///
/// # Errors
/// [`ProbeError::Mismatch`] for a bound Table 33 does not tabulate, for
/// an entry that is not a string, or for a bit at or past the bound;
/// [`ProbeError::Unknown`] for a name the bitmask does not declare.
pub fn bit_names<V: Source>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<Vec<(String, u16)>, ProbeError> {
    let bound = ast.effective_bit_bound(type_);
    if bound == 0 || bound > 64 {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "an @bit_bound between 1 and 64, as Table 33 tabulates".to_string(),
            found: format!("{bound}"),
        });
    }
    let entries = array(value, path)?;

    let mut declared = Vec::new();
    let mut positions = Vec::new();
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        declared.push(node.name.clone());
        positions.push((node.name.clone(), ast.effective_bit_position(*id)));
    }

    let mut out = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let element = format!("{path}[{index}]");
        let name = text(entry, &element).map_err(|_| ProbeError::Mismatch {
            path: element.clone(),
            expected: "a bit name, as a string".to_string(),
            found: kind_of(entry).to_string(),
        })?;
        let position = positions
            .iter()
            .find(|(declared_name, _)| declared_name == name)
            .and_then(|(_, position)| *position)
            .ok_or_else(|| ProbeError::Unknown {
                path: element.clone(),
                declared: declared.clone(),
            })?;
        // §7.4.1.1.3.2: the bits above the bound "are not considered to
        // be part of the bitmask", so setting one puts a value on the
        // wire no peer is required to read back.
        if position >= bound {
            return Err(ProbeError::Mismatch {
                path: element,
                expected: format!("a bit below the @bit_bound({bound})"),
                found: format!("`{name}` at position {position}"),
            });
        }
        out.push((name.to_string(), position));
    }
    Ok(out)
}

/// The elements of a collection, with its declared bound enforced.
///
/// The bound is part of the type, so a sample that overruns it is a
/// sample no peer's generated code can hold. One backend checked it and
/// two did not, which made an over-long sequence a ZeroDDS-only refusal.
///
/// # Errors
/// [`ProbeError::Mismatch`].
pub fn bounded_items<'a, V: Source>(
    ast: &Ast,
    value: &'a V,
    bound: Bound,
    noun: &str,
    path: &str,
) -> Result<&'a [V], ProbeError> {
    let items = array(value, path)?;
    if let Some(limit) = bound.expr().and_then(|expr| ast.eval_integral(expr).ok()) {
        if i128::try_from(items.len()).unwrap_or(i128::MAX) > limit {
            return Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: format!("at most {limit} {noun}"),
                found: format!("{}", items.len()),
            });
        }
    }
    Ok(items)
}

/// The key and the value of one map entry.
///
/// A map entry is a two-element array, `[key, value]`, and not a JSON
/// object: a JSON object keys on a string and `map<long, float>` does
/// not. §7.4.1.1.4 restricts a key to a scalar, which is what makes the
/// pair form sufficient.
///
/// # Errors
/// [`ProbeError::Mismatch`] for anything that is not a pair.
pub fn map_entry<'a, V: Source>(entry: &'a V, path: &str) -> Result<(&'a V, &'a V), ProbeError> {
    let pair = array(entry, path)?;
    match pair {
        [key, value] => Ok((key, value)),
        other => Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: "a two-element array, `[key, value]`".to_string(),
            found: format!("{} elements", other.len()),
        }),
    }
}

/// The element counts of an array's dimensions.
///
/// # Errors
/// [`ProbeError::Unsupported`] where a dimension does not evaluate. A
/// dimension that substituted zero wrote no elements at all and said it
/// had worked — defect 7 of the sweep, in its probe-side form.
pub fn array_dimensions(
    ast: &Ast,
    dims: &[Id<zerodds_ast::ConstExprNode>],
    path: &str,
) -> Result<Vec<usize>, ProbeError> {
    dims.iter()
        .map(|dim| {
            ast.eval_integral(*dim)
                .ok()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or(ProbeError::Unsupported {
                    path: path.to_string(),
                    kind: "an array dimension that does not evaluate to a count",
                })
        })
        .collect()
}

/// How a union's `switch` and its `case` labels have to be spelled in C
/// and C++.
///
/// `switch ((long long) …)` with `case <v>LL:` is right for every
/// signed discriminator and wrong for one case that is in the corpus:
/// `union Choice switch (unsigned long long) { case 16045690984833335998: … }`
/// does not fit a `long long`, so the label is a constant the compiler
/// refuses — `integer constant is so large that it is unsigned`, and
/// then an error. The generated probe does not build at all.
///
/// It is a real gap and not a hypothetical: `union-discriminator-ulonglong`
/// is a corpus case, and every emitter that writes a `switch` over a
/// discriminator had this. Found through the CORBA column's decode
/// probe, which is the first one to compile that case.
///
/// Returns `("unsigned long long", "ULL")` for an unsigned
/// discriminator and `("long long", "LL")` for everything else. An
/// enumeration is signed — E1 — and so is `char`.
#[must_use]
pub fn discriminator_c_width(
    ast: &Ast,
    discriminator: Id<TypeNode>,
) -> (&'static str, &'static str) {
    let resolved = resolve_alias(ast, discriminator);
    let unsigned = matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Primitive(
            PrimitiveKind::UInt8
                | PrimitiveKind::UInt16
                | PrimitiveKind::UInt32
                | PrimitiveKind::UInt64
                | PrimitiveKind::Octet
        ))
    );
    if unsigned {
        ("unsigned long long", "ULL")
    } else {
        ("long long", "LL")
    }
}

/// How many typedefs one chain may hop through before it is taken for a
/// cycle.
///
/// The same number [`crate::MAX_VALUE_DEPTH`] and
/// `zerodds_ast_layout::MAX_DEPTH` carry, and for the same reason: every
/// hop is one frame of a recursive descent, and 64 sits far enough below
/// the thread's stack that the refusal arrives before the stack does.
pub const MAX_ALIAS_HOPS: usize = 64;

/// Refuse a model whose typedefs close a cycle, before any walk follows
/// one.
///
/// IDL 4.2 §7.4.1.4.4.4 has no declaration that closes a typedef cycle —
/// a `typedef` names a type that must already be declared, and the
/// forward declaration IDL does have is for structures, unions and
/// interfaces, not for aliases. So `typedef T T;` is not IDL. The
/// frontend admits it anyway, measured 2026-08-31 on
/// `typedef Tee Tee; @final struct Ess { Tee ex; };`, which
/// `model_from_idl` accepts with no diagnostic.
///
/// What that model then does is the reason this exists. Every walk in
/// this crate and in `zerodds-wire-probe` sees a typedef the same way —
/// resolve the target, recurse on it, with nothing else changing — so a
/// chain that returns to itself is an unbounded recursion in each of
/// them. Thirteen of them ran into the stack rather than answering:
/// `canonicalise`, the five fill emitters, the five dump emitters,
/// `ros_idl::ros_package` and `recursion::recursive_aggregates`. A
/// probe that aborts has measured nothing, and an empty cell reads as a
/// vendor difference.
///
/// Called once at the entry of each walk rather than at each alias arm:
/// the answer is a property of the model, not of the position in it, and
/// one scan up front leaves every arm below free to follow a chain that
/// is now known to be finite.
///
/// Every alias in the model and not only the ones below `root`: an
/// unreachable cyclic typedef is a declaration the frontend should have
/// refused, and a probe run over a file holding one is measuring a model
/// nobody can write down.
///
/// # Errors
/// [`ProbeError::NoLayout`], naming the typedef the chain returns to.
pub fn reject_alias_cycle(ast: &Ast) -> Result<(), ProbeError> {
    let aliases: Vec<Id<TypeNode>> = ast
        .types()
        .filter(|(_, node)| matches!(node.kind, TypeKind::Alias { .. }))
        .map(|(id, _)| id)
        .collect();
    for start in aliases {
        let mut chain: Vec<Id<TypeNode>> = vec![start];
        let mut current = start;
        loop {
            let Ok(TypeKind::Alias { aliased }) = ast.type_(current).map(|node| node.kind.clone())
            else {
                break;
            };
            let Some(next) = aliased.resolved() else {
                break;
            };
            if chain.contains(&next) || chain.len() > MAX_ALIAS_HOPS {
                return Err(ProbeError::NoLayout {
                    detail: format!(
                        "{}: a typedef chain that returns to itself, which IDL 4.2 §7.4.1.4.4.4 \
                         has no declaration for and no walk over the type graph can end",
                        alias_name(ast, start)
                    ),
                });
            }
            chain.push(next);
            current = next;
        }
    }
    Ok(())
}

/// The scoped name of a typedef, for the refusal above.
fn alias_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    ast.any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .map_or_else(
            || String::from("a typedef"),
            |scoped| scoped.segments().join("::"),
        )
}

/// Whether `type_` is one of the two aggregates a JSON object stands
/// for, once typedefs are seen through.
#[must_use]
pub fn resolve_alias(ast: &Ast, type_: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = type_;
    // A typedef chain is finite in a committed model; the bound is
    // belt and braces against a cycle a hand-built model could hold.
    // [`reject_alias_cycle`] is the loud half of the same guard, and
    // runs at the entry of every walk that reaches here.
    for _ in 0..MAX_ALIAS_HOPS {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => current = next,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// The kind a value has, for a refusal that says what was there.
///
/// The representation's own word for it — [`Source::kind`] — because
/// what shapes there are to be is the representation's question and not
/// the type system's.
#[must_use]
pub fn kind_of<V: Source>(value: &V) -> &'static str {
    value.kind()
}
