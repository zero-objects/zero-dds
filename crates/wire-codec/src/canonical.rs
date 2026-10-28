// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The one form two implementations' values can be compared in.
//!
//! A pairwise run compares what came out of B against what went into A.
//! Both sides are written in the same representation, and a
//! representation usually has more than one way to write the same value:
//! in JSON, `2.0` and `2.0e0`, `{"a":1,"b":2}` and `{"b":2,"a":1}`,
//! `12345678901234567890` as a number and as a string. Comparing the two
//! texts would report a difference in formatting as a difference in
//! wire behaviour.
//!
//! ## Why the normalisation is type-directed
//!
//! A normaliser that only knew JSON could sort keys and reprint numbers,
//! and would still be wrong twice:
//!
//! - A `float` member carries a value the JSON was written with in full
//!   `double` precision — `0.1` in the file, `0.10000000149011612`
//!   coming back out of thirty-two bits. Neither side is wrong and the
//!   texts differ. Only the type says which of the two is the value.
//! - A `long long` is a string, because a JSON number is a double and
//!   loses the low bits past 2^53. A type-blind normaliser cannot tell
//!   a 64-bit integer that must stay a string from a `long` that must
//!   not become one.
//!
//! So [`canonicalise`] walks the type beside the value, the same walk
//! `encode` and `decode` make, and rebuilds the value in the one form
//! the table in `crate` documents. Running it over the case's own value
//! file and over a probe's output puts both in that form before
//! anything is compared.
//!
//! ## What it also does
//!
//! It validates. A key that names no member, a missing member that is
//! not `@optional`, an explicit `null` — all are errors here, with the
//! path, for the same reason they are errors in `encode`.

use crate::contract;
use crate::encode::ProbeError;
use crate::sink::{Aggregate, Sink};
use crate::source::Source;
use std::string::String;
use std::vec::Vec;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// Put one value into canonical form against its type.
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
pub fn canonicalise<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<V, ProbeError> {
    // Before the first hop: the walk below follows a typedef by
    // recursing on its target with the same value, so a chain that
    // returns to itself has no end. See
    // [`contract::reject_alias_cycle`].
    contract::reject_alias_cycle(ast)?;
    canonicalise_at(ast, type_, value, path)
}

/// [`canonicalise`], below the gate the entry point has already passed.
///
/// zerodds-lint: recursion-depth 128 — the value's own nesting. Every
/// arm but the typedef one descends into a strictly smaller part of
/// `value`, so the walk ends because the value is finite; the typedef
/// arm keeps the value and is finite because
/// [`contract::reject_alias_cycle`] has refused a chain that returns to
/// itself. A value file is `serde_json`'s to parse and its
/// `RECURSION_LIMIT` is 128; a value that came out of
/// [`crate::decode`] is capped at [`crate::MAX_VALUE_DEPTH`] instead.
fn canonicalise_at<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<V, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive(*kind, value, path),
        TypeKind::String { width, .. } => {
            let text = contract::text(value, path)?;
            match width {
                // A `wstring` on the wire is UTF-16 code units, and a
                // code point outside the Basic Multilingual Plane comes
                // back as the surrogate pair it was written as. Rust
                // strings are UTF-8, so the pair has already been joined
                // by the time it is here and nothing more is needed.
                CharWidth::Char | CharWidth::WChar => Ok(V::text(text.to_string())),
            }
        }
        TypeKind::Fixed { digits, scale } => {
            let text = contract::text(value, path)?;
            Ok(V::fixed(decimal(text, *digits, *scale, path)?))
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            canonicalise_at(ast, target, value, path)
        }
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            Ok(V::enumerator(name))
        }
        TypeKind::Bitmask { .. } => bitmask(ast, type_, value, path),
        TypeKind::Bitset { .. } => {
            let object = contract::object(value, path)?;
            // `all_bitfields_of` and not the node's own `fields`: a
            // `bitset` that extends another carries the base's
            // bitfields, and `encode::write_bitset` and
            // `decode::read_bitset` both write and read them. Taking
            // only the derived type's own list here refused
            // `bitset-inheritance`'s sample — `Sample.value.low: not a
            // member; the type declares ["high"]` — for a member the
            // encoder had just written. Nothing caught it because the
            // canonical form is only reached by the read direction and
            // by `--values-out`, and neither was run over the corpus.
            let declared = contract::bitfield_names(ast, &ast.all_bitfields_of(type_));
            contract::reject_unknown_keys(object, &declared, path)?;
            let mut out = Vec::with_capacity(declared.len());
            for name in &declared {
                let field_path = contract::join(path, name);
                let Some(field) = object.get(name) else {
                    return Err(ProbeError::Missing { path: field_path });
                };
                out.push((name.clone(), contract::integer(field, &field_path)?));
            }
            V::bitfields(out, path)
        }
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the element type is unresolved"),
            })?;
            let dims = match &node.kind {
                TypeKind::Array { dims, .. } => dims.len(),
                _ => 1,
            };
            collection(ast, element, value, path, dims)
        }
        TypeKind::Map {
            key, value: entry, ..
        } => {
            let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map key type is unresolved"),
            })?;
            let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map value type is unresolved"),
            })?;
            let entries = contract::array(value, path)?;
            let mut out = Vec::with_capacity(entries.len());
            for (index, pair) in entries.iter().enumerate() {
                let entry_path = format!("{path}[{index}]");
                let (entry_key, entry_value) = contract::map_entry(pair, &entry_path)?;
                out.push(V::map_entry(
                    canonicalise_at(ast, key_type, entry_key, &format!("{entry_path}.key"))?,
                    canonicalise_at(ast, value_type, entry_value, &format!("{entry_path}.value"))?,
                ));
            }
            // Sorted by key, because a map is a mapping and not a list.
            // XTypes 1.3 §7.4.1.1.4 defines the serialisation as a
            // sequence of entries and says nothing about their order, and
            // the bindings differ: `fastddsgen` maps a `map` to
            // `std::map`, which is ordered by key, so Fast DDS writes and
            // reads entries in a different order than a sample that was
            // written in another one. Measured on 2026-08-09 —
            // `map-key-long` came back with 1 where the sample had
            // 2147483647 in the same slot, on Fast DDS reading its own
            // bytes. Comparing the order would report a container choice
            // as a wire disagreement; a key that arrived with the wrong
            // value still fails, which is the part that is one.
            out.sort_by_key(|entry| {
                entry
                    .as_items()
                    .and_then(<[V]>::first)
                    .map_or_else(String::new, Source::render)
            });
            Ok(V::collection(out))
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            aggregate(ast, type_, value, path)
        }
        TypeKind::Union { .. } => union(ast, type_, value, path),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// A structure or a valuetype, member by member in declaration order.
///
/// `all_members_of` and not `members_of`: a derived type's base
/// contributes members, and a normaliser that walked only the type's own
/// would report every inherited member as an undeclared key.
///
/// zerodds-lint: recursion-depth 128 — one frame per aggregate of the
/// value, below `canonicalise_at`, which carries the argument. Each member
/// is a strictly smaller part of the object this was handed.
fn aggregate<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<V, ProbeError> {
    let object = contract::object(value, path)?;
    let members = contract::members(ast, type_);
    let declared: Vec<String> = members.iter().map(|member| member.name.clone()).collect();
    contract::reject_unknown_keys(object, &declared, path)?;

    let mut out = Aggregate::new();
    for member in members {
        // `contract::member_value` is the one place the two rules live:
        // a key left out is an absent `@optional`, and `null` is neither
        // a value nor an absence. All three probes ask it, so the input
        // sample and every probe's output mean the same thing.
        let Some(present) = contract::member_value(object, &member, path)? else {
            continue;
        };
        let member_path = contract::join(path, &member.name);
        out.insert(
            member.name.clone(),
            canonicalise_at(ast, member.type_, present, &member_path)?,
        );
    }
    Ok(V::aggregate(out))
}

/// A union: `_d`, and the member the value names.
///
/// The label is not evaluated here. A sample whose `_d` selects no case
/// carries no member at all — `union-default-unselected` in the corpus
/// is exactly that — and a sample that names a member the discriminator
/// does not select is a case somebody wrote on purpose. Deciding either
/// way here would change what the comparison is testing.
///
/// zerodds-lint: recursion-depth 128 — one frame per aggregate of the
/// value, below `canonicalise_at`, which carries the argument. The
/// discriminant and each named member are strictly smaller parts of the
/// object this was handed.
fn union<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<V, ProbeError> {
    let object = contract::object(value, path)?;
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

    let cases = contract::union_members(ast, type_);
    let mut out = Aggregate::new();
    let discriminant = contract::discriminator(object, path)?;
    out.insert(
        "_d".to_string(),
        canonicalise_at(
            ast,
            discriminator,
            discriminant,
            &contract::join(path, "_d"),
        )?,
    );

    for (name, member_value) in object {
        if name == "_d" {
            continue;
        }
        let member_type = cases
            .iter()
            .find(|(case_name, _)| case_name == name)
            .map(|(_, member_type)| *member_type)
            .ok_or_else(|| ProbeError::Unknown {
                path: format!("{path}.{name}"),
                declared: cases.iter().map(|(name, _)| name.clone()).collect(),
            })?;
        out.insert(
            name.to_string(),
            canonicalise_at(ast, member_type, member_value, &format!("{path}.{name}"))?,
        );
    }
    Ok(V::aggregate(out))
}

/// An array or a sequence, one dimension at a time.
///
/// A multi-dimensional array is nested JSON arrays, so the recursion
/// counts dimensions down rather than flattening: `long grid[2][3]` is
/// two arrays of three, and a normaliser that produced six would compare
/// equal against a probe that got the dimensions the wrong way round.
///
/// zerodds-lint: recursion-depth 8 — one frame per array dimension. `dims`
/// counts down by one at each frame and the last dimension's elements go
/// to `canonicalise_at`, which is bounded separately.
fn collection<V: Source + Sink>(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &V,
    path: &str,
    dims: usize,
) -> Result<V, ProbeError> {
    let items = contract::array(value, path)?;
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        out.push(if dims > 1 {
            collection(ast, element, item, &item_path, dims - 1)?
        } else {
            canonicalise_at(ast, element, item, &item_path)?
        });
    }
    Ok(V::collection(out))
}

/// A bitmask, as the set bits in declaration order.
///
/// Order and duplicates are both normalised away because neither
/// survives the wire: the value is an integer with bits in it, so
/// `["B", "A", "A"]` and `["A", "B"]` are the same sample and only one
/// of the two can come back out.
fn bitmask<V: Source + Sink>(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &V,
    path: &str,
) -> Result<V, ProbeError> {
    let mut set = Vec::new();
    for (name, _) in contract::bit_names(ast, type_, value, path)? {
        set.push(name);
    }
    // Declaration order and not the order the names were written in:
    // the value is an integer with bits in it, so `["B", "A", "A"]` and
    // `["A", "B"]` are the same sample and only one can come back out.
    let mut out = Vec::new();
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        if set.contains(&node.name) {
            out.push(node.name.clone());
        }
    }
    Ok(V::bit_names(out))
}

/// One primitive, in the shape its width calls for.
///
/// Which shape that is belongs to the representation and is
/// [`Sink`]'s answer; what is decided here is the *type* half of it,
/// which is the same in every representation: an `octet` is checked
/// against an octet's range, a `float` is narrowed through thirty-two
/// bits before anything is stored, and a `wchar` is checked against the
/// Basic Multilingual Plane.
fn primitive<V: Source + Sink>(
    kind: PrimitiveKind,
    value: &V,
    path: &str,
) -> Result<V, ProbeError> {
    match kind {
        PrimitiveKind::Boolean => Ok(V::boolean(contract::boolean(value, path)?)),
        PrimitiveKind::Char | PrimitiveKind::WChar => Ok(V::character(
            kind,
            contract::character_for(kind, value, path)?,
        )),
        // Thirty-two bits of float hold fewer digits than the JSON that
        // was written for them. Rounding through `f32` here is what puts
        // the file's `0.1` and the wire's `0.10000000149011612` into the
        // same value.
        PrimitiveKind::Float32 => {
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(contract::float(value, path)? as f32);
            V::float(kind, narrowed, path)
        }
        PrimitiveKind::Float64 => V::float(kind, contract::float(value, path)?, path),
        // A `long double` canonicalises to its exact value, whatever
        // form the value file used, and never back through a double —
        // which would undo the point of accepting the string on the way
        // in. See `decode.rs`.
        PrimitiveKind::Float128 => Ok(V::float128(contract::float128(value, path)?)),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => {
            V::integer(kind, contract::integer_for(kind, value, path)?, path)
        }
    }
}

/// The canonical decimal for a `fixed<P, S>`.
///
/// The wire form is `P` digits and a sign, so `1.5` and `0001.500` are
/// the same sample under `fixed<7, 3>` and only one string can come back
/// out of it. That one is the digits the type has: no leading zeros
/// beyond a single one before the point, exactly `S` after it, and a
/// leading `-` only where a non-zero value is negative — [CDR] §9.3.2.8
/// has one zero and gives it the positive sign nibble.
pub(crate) fn decimal(text: &str, digits: u8, scale: u8, path: &str) -> Result<String, ProbeError> {
    // Every rule about what is a legal value for `fixed<P, S>` is in
    // `zerodds_wire::parse_fixed`, and it was written out here as well until
    // the two disagreed about nothing only because nobody had changed either:
    // a canonical form that accepted a value the writer refuses would hand a
    // sample back that cannot be written again.
    let parts =
        zerodds_wire::parse_fixed(text, digits, scale, 0).map_err(|_| ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("a decimal that fits fixed<{digits}, {scale}>"),
            found: format!("`{text}`"),
        })?;

    let mut out = String::new();
    let all_zero = parts.whole.is_empty() && parts.fraction.bytes().all(|byte| byte == b'0');
    // [CDR] §9.3.2.8 has one zero and gives it the positive sign nibble, so
    // the canonical text of a negative zero carries no `-` either.
    if parts.negative && !all_zero {
        out.push('-');
    }
    out.push_str(if parts.whole.is_empty() {
        "0"
    } else {
        parts.whole
    });
    if scale > 0 {
        out.push('.');
        out.push_str(parts.fraction);
        for _ in parts.fraction.len()..usize::from(scale) {
            out.push('0');
        }
    }
    Ok(out)
}

/// The canonical text of a canonical value.
///
/// Keys ordered, no whitespace, one spelling per number. What that
/// spelling is belongs to the representation — [`Source::render`] — for
/// the reason it belongs there and not here: a form that depended on the
/// map implementation would change under a Cargo feature somewhere else
/// in the workspace, and only the representation knows what its own maps
/// and numbers are. This is what a fixture records and what a comparison
/// hashes.
#[must_use]
pub fn text<V: Source>(value: &V) -> String {
    value.render()
}

/// Where two canonical values first differ, as a path and the two forms.
///
/// The first difference and not all of them: a decoder that lost its
/// place reports every member after the one it lost, and a report of
/// forty differences says less than a report of the first.
///
/// Two aggregates are walked member by member and two collections
/// element by element; anything else — including an aggregate against a
/// collection — is compared as text, which is what says the two are
/// different shapes rather than different values.
///
/// zerodds-lint: recursion-depth 128 — the shallower of the two values'
/// nesting. Both arms descend into a strictly smaller part of *both*
/// values, and neither is a type walk at all, so the walk ends because the
/// values are finite. A value file is `serde_json`'s to parse and its
/// `RECURSION_LIMIT` is 128.
#[must_use]
pub fn difference<V: Source>(expected: &V, found: &V, path: &str) -> Option<Difference> {
    if expected.is_object() && found.is_object() {
        let mut keys: Vec<&str> = expected
            .entries()
            .into_iter()
            .map(|(name, _)| name)
            .chain(found.entries().into_iter().map(|(name, _)| name))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            let member = format!("{path}.{key}");
            match (expected.field(key), found.field(key)) {
                (Some(a), Some(b)) => {
                    if let Some(inner) = difference(a, b, &member) {
                        return Some(inner);
                    }
                }
                (a, b) => {
                    return Some(Difference {
                        path: member,
                        expected: a.map(text),
                        found: b.map(text),
                    });
                }
            }
        }
        return None;
    }
    if let (Some(left), Some(right)) = (expected.as_items(), found.as_items()) {
        for index in 0..left.len().max(right.len()) {
            let element = format!("{path}[{index}]");
            match (left.get(index), right.get(index)) {
                (Some(a), Some(b)) => {
                    if let Some(inner) = difference(a, b, &element) {
                        return Some(inner);
                    }
                }
                (a, b) => {
                    return Some(Difference {
                        path: element,
                        expected: a.map(text),
                        found: b.map(text),
                    });
                }
            }
        }
        return None;
    }
    if text(expected) == text(found) {
        return None;
    }
    Some(Difference {
        path: path.to_string(),
        expected: Some(text(expected)),
        found: Some(text(found)),
    })
}

/// One place two values disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// Where, as a dotted path from the sample's root.
    pub path: String,
    /// What went in. `None` where the key was not there at all.
    pub expected: Option<String>,
    /// What came back.
    pub found: Option<String>,
}

impl core::fmt::Display for Difference {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let expected = self.expected.as_deref().unwrap_or("<absent>");
        let found = self.found.as_deref().unwrap_or("<absent>");
        write!(f, "{}: wrote {expected}, read {found}", self.path)
    }
}
