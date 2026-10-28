// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value-filling code the Fast DDS probe needs.
//!
//! `fill.rs` writes C, for the two generators whose output is a plain
//! `struct`. Fast DDS generates a C++ class: every member is a pair of
//! accessors, so nothing can be assigned through a dotted path and the
//! statements have a shape of their own.
//!
//! ```text
//! sample.x = 1;                      // Cyclone, RTI
//! sample.x(1);                       // Fast DDS
//! sample.location.x = 1.0;           // Cyclone, RTI
//! sample.location().x(1.0);          // Fast DDS
//! ```
//!
//! ## Why this is not a self-oracle
//!
//! The same reason `fill.rs` is not: the statements place values into
//! the *vendor's* structures and the bytes still come from the vendor's
//! own serialiser. A misplaced value shows up as a disagreement between
//! the three probes on that case, never as a false agreement.
//!
//! ## Accessors, not temporaries
//!
//! `fastddsgen` emits a non-`const` reference getter beside every
//! setter, so a collection or a nested aggregate is reachable in place:
//! `sample.items()[0].a(1)`. That keeps the recursion the same shape as
//! the C one — an expression that names the destination — instead of
//! building a temporary per collection and handing it to the setter.
//! Only the terminal assignment differs, and only in two forms:
//!
//! | reached through | assignment |
//! |---|---|
//! | a member of an aggregate | `object.name(value);` |
//! | an element of an array or a sequence | `expression = value;` |
//! | a subscript of a map | `expression = value;` |
//!
//! ## Two places a statement has to reach before it can write
//!
//! The last row above writes like the middle one and does not *exist*
//! like it: `std::map::operator[]` inserts the entry, so a value that
//! emits no statement leaves the entry out of the sample the peer
//! serialises. And a union branch is reachable only after its setter
//! has selected it — `fastddsgen`'s getter throws until then. Both are
//! handled at one place each, `emit` and the `Union` arm, because
//! both were once handled in the arm that happened to hit them and both
//! were recorded as Fast DDS divergences.

use crate::contract::{self, Consumed};
use crate::encode::ProbeError;
use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// Where one value goes, in the shapes Fast DDS's generated code
/// offers.
///
/// Two of the three merely *reach* a place that already exists. The
/// third **creates** it, and that difference is what
/// [`emit`] enforces rather than each arm remembering it.
enum Place {
    /// A member of an aggregate: `object.name(value)` to write it,
    /// `object.name()` to descend into it.
    Accessor {
        /// The expression naming the aggregate.
        object: String,
        /// The member's name, which is also the accessor's name.
        name: String,
    },
    /// An element of a `std::array` or a `std::vector`, which is a plain
    /// lvalue in both directions. The element exists before the
    /// statement runs — the array declares it and `resize` creates it.
    Element(String),
    /// A subscript of a `std::map`, which is a plain lvalue in both
    /// directions and exists **only once a statement has named it**:
    /// `std::map::operator[]` is what inserts the entry.
    MapEntry(String),
}

impl Place {
    /// The statement that writes `literal` here.
    fn assign(&self, literal: &str) -> String {
        match self {
            Self::Accessor { object, name } => format!("{object}.{name}({literal});"),
            Self::Element(expression) | Self::MapEntry(expression) => {
                format!("{expression} = {literal};")
            }
        }
    }

    /// The expression that names what is here, for descending into it.
    fn reference(&self) -> String {
        match self {
            Self::Accessor { object, name } => format!("{object}.{name}()"),
            Self::Element(expression) | Self::MapEntry(expression) => expression.clone(),
        }
    }
}

/// The statements, and a note of every scalar the walk took out of the
/// value document.
///
/// The recorder rides with the output buffer for the same reason the
/// serialiser's does: one mutable thing to thread, so a new arm cannot
/// forget it.
#[derive(Debug, Default)]
struct Cpp {
    source: String,
    seen: Consumed,
}

/// Emit the C++ statements that fill `sample` with `values`.
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
pub fn fill_fastdds(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, ProbeError> {
    fill_fastdds_recording(ast, type_, values, &mut Consumed::new())
}

/// As [`fill_fastdds`], recording which scalars of the value were read.
///
/// # Errors
/// As [`fill_fastdds`].
pub fn fill_fastdds_recording(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    seen: &mut Consumed,
) -> Result<String, ProbeError> {
    // Before the first hop; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Cpp::default();
    let root = contract::root_path(ast, type_);
    let result = emit(
        ast,
        type_,
        values,
        &Place::Element(String::from("sample")),
        &mut out,
        &root,
    );
    *seen = core::mem::take(&mut out.seen);
    result?;
    Ok(out.source)
}

/// The C++ name a type takes in `fastddsgen`'s output.
///
/// The module path stays a namespace path, so `::common::util::Location`
/// is `common::util::Location`. This is what `--print-c-name` prints for
/// the Fast DDS probe, and it is also the enum's scope: `enum class`
/// means an enumerator is `Colour::RED`, never a bare `RED`.
#[must_use]
pub fn cpp_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    ast.any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .map_or_else(
            || String::from("UNKNOWN"),
            |scoped| scoped.segments().join("::"),
        )
}

/// One value into one place, and the guarantee that the place exists
/// afterwards.
///
/// Every arm below writes through [`Place::assign`] or
/// [`Place::reference`], so any statement at all names the place and, at
/// a [`Place::MapEntry`], calls `std::map::operator[]` and inserts the
/// entry. The converse is what this function is for: a value whose
/// filled form is **empty** reaches its arm and emits no statement, the
/// subscript is never written, the entry is never created, and the peer
/// serialises a map one entry short of the sample. Measured at
/// `b0bc71313`: `map-value-sequence`'s entry `(1, [])` and
/// `map-nested`'s entry `(2, {})` had no statement, and the 16 Fast DDS
/// rows that recorded were a defect in this emitter and not a
/// measurement of Fast DDS.
///
/// The class is wider than the one arm that was found. An empty
/// sequence, an empty inner map, a structure with no member to write —
/// an `@optional` absent from every member, or no members at all — a
/// bitset with no declared field, and an array whose every element is
/// one of those, all emit nothing. So the guard is here, at the one
/// place that knows which kind of place it is, rather than in each arm
/// that can be empty.
///
/// zerodds-lint: recursion-depth 128 — the sample's own nesting. This adds
/// the empty-place guard and hands the value straight to `emit_value`,
/// which carries the argument.
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    place: &Place,
    out: &mut Cpp,
    path: &str,
) -> Result<(), ProbeError> {
    let before = out.source.len();
    emit_value(ast, type_, value, place, out, path)?;
    if let Place::MapEntry(subscript) = place {
        if out.source.len() == before {
            // A discarded subscript, which is a call to `operator[]` and
            // therefore the insertion. The cast is what keeps
            // `-Wunused-value` quiet about the discarded reference.
            let _ = writeln!(out.source, "(void){subscript};");
        }
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 128 — the sample's own nesting. Every arm
/// but the typedef one descends into a strictly smaller part of the value,
/// so the walk ends because the sample is finite: a recursive type has
/// finite values, because the sequence that closes its cycle is eventually
/// empty. The typedef arm keeps the value and is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. A value file is `serde_json`'s to parse and its
/// `RECURSION_LIMIT` is 128.
#[allow(clippy::too_many_lines)]
fn emit_value(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    place: &Place,
    out: &mut Cpp,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        // Measured, Fast DDS with FastCDR `FASTCDR_SIZEOF_LONG_DOUBLE
        // 16` on Debian 13 x86-64: `Cdr::serialize(long double)` copies
        // the object's sixteen bytes, which are x87 80-bit extended plus
        // six bytes the compiler never wrote. The same source on aarch64
        // emits binary128, because there `long double` *is* binary128.
        // A literal here would pick one architecture's wire format and
        // record it as Fast DDS's; `docs/float128-column.md` §2 has the
        // bytes from both.
        TypeKind::Primitive(PrimitiveKind::Float128) => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a long double, whose Fast DDS bytes differ between x86-64 and aarch64",
        }),
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(out.source, "{}", place.assign(&literal));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(out.source, "{}", place.assign(&cpp_string(text)));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            emit_wstring(text, place, out);
            Ok(())
        }
        TypeKind::Bitmask { .. } => {
            let set = contract::bit_names(ast, type_, value, path)?;
            let literal = bitmask_literal(ast, type_, &set, out, path);
            let _ = writeln!(out.source, "{}", place.assign(&literal));
            Ok(())
        }
        TypeKind::Bitset { .. } => {
            let object = contract::object(value, path)?;
            // The base's bitfields too: `fastddsgen` flattens an
            // inherited bitset into a single struct, so
            // `sample.value().low` addresses an inherited field with no
            // change to the statement shape.
            let declared = contract::bitfield_names(ast, &ast.all_bitfields_of(type_));
            contract::reject_unknown_keys(object, &declared, path)?;
            let here = place.reference();
            // Every declared field, in declaration order, and not only
            // the keys the object happens to hold: a bitfield with no
            // value is a field left at whatever the C++ default gave it,
            // which is the sample the serialiser then writes.
            for name in &declared {
                let field_path = contract::join(path, name);
                let Some(field_value) = object.get(name) else {
                    return Err(ProbeError::Missing { path: field_path });
                };
                let raw = contract::integer(field_value, &field_path)?;
                out.seen.scalar(&field_path);
                // `fastddsgen` renders a bitset as a plain struct of C++
                // bit-fields with public members, not as an accessor
                // pair, so these are ordinary assignments.
                let _ = writeln!(out.source, "{here}.{name} = {raw};");
            }
            Ok(())
        }
        TypeKind::Map {
            key,
            value: entry,
            bound,
        } => {
            let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map key type is unresolved"),
            })?;
            let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map value type is unresolved"),
            })?;
            // An array of pairs and not an object, the same shape the
            // ZeroDDS probe reads: a JSON object keys on a string and
            // `map<long, float>` does not.
            let entries = contract::bounded_items(ast, value, *bound, "entries", path)?;
            let here = place.reference();
            for (index, pair) in entries.iter().enumerate() {
                let entry_path = format!("{path}[{index}]");
                let (entry_key, entry_value) = contract::map_entry(pair, &entry_path)?;
                let key_path = contract::join(&entry_path, "key");
                let key_literal = scalar_literal(ast, key_type, entry_key, &key_path)?;
                out.seen.scalar(&key_path);
                emit(
                    ast,
                    value_type,
                    entry_value,
                    &Place::MapEntry(format!("{here}[{key_literal}]")),
                    out,
                    &contract::join(&entry_path, "value"),
                )?;
            }
            Ok(())
        }
        TypeKind::Fixed { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a fixed, which fastddsgen rejects in the IDL before any \
                   statement could be written for it",
        }),
        TypeKind::Enumeration { .. } => {
            // Checked against the declared set before it is pasted into
            // an identifier, the same as the C emitter and the
            // serialiser.
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            out.seen.scalar(path);
            let literal = format!("{}::{name}", cpp_name(ast, type_));
            let _ = writeln!(out.source, "{}", place.assign(&literal));
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            emit(ast, target, value, place, out, path)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = contract::object(value, path)?;
            let here = place.reference();
            let members = contract::members(ast, type_);
            let declared: Vec<String> = members.iter().map(|member| member.name.clone()).collect();
            // A key that names no member is an error and not a silent
            // skip. An emitter that ignored it would drop part of the
            // sample and still produce bytes, which is the one failure
            // this whole file exists to make impossible.
            contract::reject_unknown_keys(object, &declared, path)?;
            for member in &members {
                // An absent `@optional` has nothing to fill, the same
                // as in the C emitter.
                let Some(member_value) = contract::member_value(object, member, path)? else {
                    continue;
                };
                // `fastddsgen` renders `@external` as
                // `eprosima::fastcdr::external<T>` and the generated
                // default constructor leaves it empty, so the pointee
                // is allocated first and the value written through the
                // dereferenced getter — the read side
                // (`dump_cpp::is_external`) mirrors this. `decltype`
                // names the pointee so no C++ type spelling is needed.
                // `@optional @external` stacks the two wrappers:
                // `optional<external<T>>`. Engaging the optional with a
                // freshly allocated external is one assignment through
                // `optional::operator=(T&&)`; the pointee is then
                // filled through `value()`. The `decltype` operand is
                // unevaluated, so naming `value()` on the still
                // disengaged optional is well-defined.
                if crate::dump_cpp::is_external(ast, type_, &member.name) && member.optional {
                    let getter = format!("{here}.{}()", member.name);
                    let _ = writeln!(
                        out.source,
                        "{{ using zd_t = std::remove_reference_t<decltype(*({getter}.value()))>; \
{getter} = eprosima::fastcdr::external<zd_t>(std::make_shared<zd_t>()); }}"
                    );
                    emit(
                        ast,
                        member.type_,
                        member_value,
                        &Place::Element(format!("(*{getter}.value())")),
                        out,
                        &contract::join(path, &member.name),
                    )?;
                    continue;
                }
                if crate::dump_cpp::is_external(ast, type_, &member.name) && !member.optional {
                    let getter = format!("{here}.{}()", member.name);
                    let _ = writeln!(
                        out.source,
                        "{{ using zd_t = std::remove_reference_t<decltype(*{getter})>; \
{getter} = eprosima::fastcdr::external<zd_t>(std::make_shared<zd_t>()); }}"
                    );
                    emit(
                        ast,
                        member.type_,
                        member_value,
                        &Place::Element(format!("(*{getter})")),
                        out,
                        &contract::join(path, &member.name),
                    )?;
                    continue;
                }
                // A present `@optional` string cannot go through the
                // setter: it takes `const optional<std::string>&`, and
                // `const char*` → `std::string` → `optional` is two
                // user conversions, which C++ refuses (measured:
                // `no matching function for call to 'Sample::note(const
                // char [6])'`, matrix fastdds annotation-must-understand
                // and annotation-optional-mixed). `optional::operator=
                // (const T&)` exists (fast-cdr
                // include/fastcdr/xcdr/optional.hpp:234), so the getter
                // assignment needs only the one conversion.
                if member.optional && crate::fill::is_string_type(ast, member.type_) {
                    emit(
                        ast,
                        member.type_,
                        member_value,
                        &Place::Element(format!("{here}.{}()", member.name)),
                        out,
                        &contract::join(path, &member.name),
                    )?;
                    continue;
                }
                emit(
                    ast,
                    member.type_,
                    member_value,
                    &Place::Accessor {
                        object: here.clone(),
                        name: member.name.clone(),
                    },
                    out,
                    &contract::join(path, &member.name),
                )?;
            }
            Ok(())
        }
        TypeKind::Union { .. } => {
            let object = contract::object(value, path)?;
            let discriminant = contract::discriminator(object, path)?;
            let here = place.reference();
            // The members go first and `_d` last. A Fast DDS member
            // setter moves the discriminator to that member's own label,
            // so an explicit `_d` written before it would be overwritten;
            // written after it, it pins the exact value — and the
            // generated `_d` setter refuses a value that would select a
            // different member, which is the error one wants.
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let Some(member_type) = union_member(ast, type_, name) else {
                    return Err(ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: union_member_names(ast, type_),
                    });
                };
                contract::reject_null(member_value, &contract::join(path, name))?;
                // A branch that is filled *through* its getter has to
                // be selected first, and only the setter selects it.
                //
                // Measured, `fastddsgen` 4.3.0 over
                // `union Choice switch (short) { case -1: sequence<long>
                // ids; default: string text; }`: the default
                // constructor sets `selected_member_` to the default
                // branch, `ids()` throws
                // `BadParamException("This member has not been
                // selected")` while it is not the selection, and
                // `ids(v)` both assigns and sets `m__d = -1`. So
                // `sample.picked().ids().resize(2)` — what this emitted
                // — threw before a value was written, and
                // `union-member-sequence`, `union-member-struct` and
                // their twins stood in the record as sixteen Fast DDS
                // refusals that were this emitter's.
                //
                // `{}` and not a typed literal: the setter's parameter
                // names the type, so the emitter does not have to. It
                // is overwritten immediately by the walk below, and the
                // `_d` written after the loop pins the discriminator.
                // An `@external` branch is `eprosima::fastcdr::external<T>`,
                // empty when default-constructed: the setter selects the
                // branch with a freshly allocated pointee, and the value
                // is written through it — what the structure-member arm
                // above does for an `@external` member. Through the
                // getter alone, `annotation-external-union-branch` and its
                // twin stood in the record as Fast DDS refusals that were
                // this emitter's: "'class eprosima::fastcdr::external<…>'
                // has no member named 'a'".
                if crate::dump_cpp::is_external(ast, type_, name) {
                    let getter = format!("{here}.{name}()");
                    let _ = writeln!(
                        out.source,
                        "{{ using zd_t = std::remove_reference_t<decltype(*{getter})>; \
{here}.{name}(eprosima::fastcdr::external<zd_t>(std::make_shared<zd_t>())); }}"
                    );
                    emit(
                        ast,
                        member_type,
                        member_value,
                        &Place::Element(format!("(*{getter})")),
                        out,
                        &contract::join(path, name),
                    )?;
                    continue;
                }
                if reached_through_the_getter(ast, member_type) {
                    let _ = writeln!(out.source, "{here}.{name}({{}});");
                }
                emit(
                    ast,
                    member_type,
                    member_value,
                    &Place::Accessor {
                        object: here.clone(),
                        name: name.to_string(),
                    },
                    out,
                    &contract::join(path, name),
                )?;
            }
            let TypeKind::Union { discriminator, .. } = &node.kind else {
                return Err(ProbeError::Unsupported {
                    path: path.to_string(),
                    kind: "a union whose kind changed under the emitter",
                });
            };
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the discriminator is unresolved"),
                })?;
            emit(
                ast,
                discriminator,
                discriminant,
                &Place::Accessor {
                    object: here,
                    name: String::from("_d"),
                },
                out,
                &contract::join(path, "_d"),
            )
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            emit_array(ast, element, value, &place.reference(), out, path, &counts)
        }
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            // `std::vector`, so the length is a resize. `resize(0)` on a
            // vector that is already empty is a no-op, and writing it
            // unconditionally is what keeps the statement count from
            // depending on the value: the omission this used to carry
            // was reasoned at a member — "the member starts empty" — and
            // was false at a map subscript, where the omitted statement
            // was also the entry's only creation. `emit` now guarantees
            // that place separately; the unconditional form is here so
            // that the two guards do not have to agree.
            let here = place.reference();
            let _ = writeln!(out.source, "{here}.resize({});", items.len());
            for (index, item) in items.iter().enumerate() {
                emit(
                    ast,
                    element,
                    item,
                    &Place::Element(format!("{here}[{index}]")),
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
            Ok(())
        }
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `emit`, which is bounded by the sample.
#[allow(clippy::too_many_arguments)]
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    here: &str,
    out: &mut Cpp,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(
            ast,
            element,
            value,
            &Place::Element(here.to_string()),
            out,
            path,
        );
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
        emit_array(
            ast,
            element,
            item,
            &format!("{here}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    Ok(())
}

/// A `wstring` field, as a `std::wstring` built from code points.
///
/// The C emitter cannot name the element type, because XTypes 1.3
/// §7.5.1.1.1 leaves the C spelling of `Char16` to the implementation.
/// The C++ mapping does not: §7.5.2 fixes `wstring` as `std::wstring`,
/// whose element is `wchar_t`. So the type is known and only its width
/// is not, and `wchar_t` is a code-point holder — four bytes on Linux,
/// two on Windows.
///
/// The initialiser is therefore code points, not UTF-16 code units. A
/// code point outside the BMP does not fit a sixteen-bit `wchar_t`, and
/// the `static_assert` says so at compile time rather than truncating.
///
/// Not an `L"..."` literal: that would put the text through the
/// compiler's source and execution character sets, and the point of the
/// probe is that the sample is exactly the one in the JSON.
fn emit_wstring(text: &str, place: &Place, out: &mut Cpp) {
    if text.chars().any(|ch| ch as u32 > 0xFFFF) {
        let _ = writeln!(
            out.source,
            "static_assert(sizeof(wchar_t) >= 4, \
             \"this sample has a code point outside the BMP\");"
        );
    }
    let mut units = String::new();
    for ch in text.chars() {
        let _ = write!(units, "static_cast<wchar_t>(0x{:04x}), ", ch as u32);
    }
    let _ = units.pop();
    let _ = units.pop();
    let _ = writeln!(
        out.source,
        "{}",
        place.assign(&format!("std::wstring{{{units}}}"))
    );
}

/// A bitmask value, as the flag names the binding declares, OR'ed.
///
/// XTypes 1.3 §7.5.2.2 gives the same shape the C binding has:
/// `fastddsgen` emits `enum <TypeName>Bits : <holder> { FLAG = 0x01, … }`
/// beside `typedef <holder> <TypeName>`, and the enumeration is not an
/// `enum class`, so a flag is named at the enclosing namespace.
///
/// Naming the flags rather than computing the integer keeps the
/// position-to-bit mapping on the vendor's side of the line.
fn bitmask_literal(
    ast: &Ast,
    type_: Id<TypeNode>,
    set: &[(String, u16)],
    out: &mut Cpp,
    path: &str,
) -> String {
    let prefix = namespace_prefix(ast, type_);
    let mut flags = Vec::with_capacity(set.len());
    for (index, (name, _)) in set.iter().enumerate() {
        out.seen.scalar(&format!("{path}[{index}]"));
        flags.push(format!("{prefix}{name}"));
    }

    // No flags set is the integer zero, not an empty expression.
    if flags.is_empty() {
        "0".to_string()
    } else {
        flags.join(" | ")
    }
}

/// The literal for a map key, which XTypes 1.3 §7.4.1.1.4 restricts to a
/// scalar. An aggregate key is refused rather than guessed at.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn scalar_literal(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<String, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive_literal(*kind, value, path),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => Ok(cpp_string(contract::text(value, path)?)),
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            Ok(format!("{}::{name}", cpp_name(ast, type_)))
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            scalar_literal(ast, target, value, path)
        }
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// The namespace a type's members are named through, with the trailing
/// `::` when there is one.
fn namespace_prefix(ast: &Ast, type_: Id<TypeNode>) -> String {
    let Some(scoped) = ast
        .any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
    else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = segments[..segments.len() - 1].join("::");
    prefix.push_str("::");
    prefix
}

/// Whether a value of this type is filled by descending through
/// [`Place::reference`] rather than by writing it with
/// [`Place::assign`].
///
/// The arms of [`emit_value`] that call `reference`: `Bitset`, `Map`,
/// `Structure`, `Valuetype`, `Union`, `Array` and `Sequence`. The ones
/// that call `assign`: `Primitive`, both `String` widths, `Bitmask` and
/// `Enumeration`. `Fixed` and `Float128` are refused before either.
///
/// It matters at one place — a union branch, where the getter throws
/// until the setter has selected the branch — and the list is here
/// rather than at that place so that an arm which changes how it writes
/// has one function to be checked against.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn reached_through_the_getter(ast: &Ast, type_: Id<TypeNode>) -> bool {
    match ast.type_(type_).map(|node| node.kind.clone()) {
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| reached_through_the_getter(ast, target)),
        Ok(
            TypeKind::Bitset { .. }
            | TypeKind::Map { .. }
            | TypeKind::Structure { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::Union { .. }
            | TypeKind::Array { .. }
            | TypeKind::Sequence { .. },
        ) => true,
        _ => false,
    }
}

fn union_member(ast: &Ast, union: Id<TypeNode>, name: &str) -> Option<Id<TypeNode>> {
    ast.cases_of(union)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .find(|member| member.name == name)
        .and_then(|member| member.type_.resolved())
}

fn union_member_names(ast: &Ast, union: Id<TypeNode>) -> Vec<String> {
    ast.cases_of(union)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .map(|member| member.name.clone())
        .collect()
}

/// The C++ literal for one primitive.
///
/// The same seventeen significant digits the C emitter writes, for the
/// same reason: a shorter literal would put different bytes on the wire
/// than the ZeroDDS probe's.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => {
            if contract::boolean(value, path)? {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        PrimitiveKind::Char => character(kind, value, path, 2)?,
        PrimitiveKind::WChar => character(kind, value, path, 4)?,
        PrimitiveKind::Float32 => {
            format!("{:.17e}f", contract::float(value, path)?)
        }
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            format!("{:.17e}", contract::float(value, path)?)
        }
        PrimitiveKind::Int64 => {
            let n = contract::integer_for(kind, value, path)?;
            // As in `fill.rs` and `fill_ros.rs`: C++ has no literal for
            // INT64_MIN either. `-9223372036854775808LL` is a positive
            // constant too large for `long long`, and what a compiler
            // does with it after the unary minus is an
            // implementation-defined conversion rather than the value
            // the case declares.
            if n == i128::from(i64::MIN) {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{n}LL")
            }
        }
        PrimitiveKind::UInt64 => format!("{}ULL", contract::integer_for(kind, value, path)?),
        // The range check is the contract's: a `300` written into a
        // `uint8_t` accessor is `44` on the wire and no compiler this
        // harness uses makes a noise about it.
        _ => format!("{}", contract::integer_for(kind, value, path)?),
    })
}

/// One character as a C++ integer literal.
///
/// An integer and not a character constant, which is
/// `corba_cpp::primitive_literal`'s rule and `fill::primitive_literal`'s.
/// `'x'` is the narrow execution character set, so anything above
/// U+007F is a multi-character constant — `0xC3A4` for U+00E4 on the
/// recording host — and `L'x'` is `wchar_t`, four octets on Linux,
/// where a generated `char16_t` accessor takes two. The integer
/// converts to whichever the accessor declares and says what it is.
///
/// `digits` is how wide the literal is printed, which is presentation
/// only: it makes a `wchar` read as a code point beside the `wstring`
/// units the same file emits.
///
/// `character_for` and not `character`: a code point above the member's
/// width is refused here rather than narrowed in silence, and the
/// serialiser refuses the same value — the three backends have to read
/// one value file the same way.
fn character(
    kind: PrimitiveKind,
    value: &Value,
    path: &str,
    digits: usize,
) -> Result<String, ProbeError> {
    let code = contract::character_for(kind, value, path)? as u32;
    Ok(format!("0x{code:0digits$x}"))
}

/// A C++ string literal, escaped as `corba_cpp::cpp_string` escapes
/// one: three-digit octal for every byte outside printable ASCII,
/// because `\xHH` is greedy and three-digit octal cannot run on. Raw
/// UTF-8 octets, which this used to emit, compile only where the
/// compiler's narrow execution character set is UTF-8.
fn cpp_string(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.as_bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(char::from(*byte)),
            other => {
                let _ = write!(out, "\\{other:03o}");
            }
        }
    }
    out.push('"');
    out
}
