// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value-filling code a foreign probe needs.
//!
//! A vendor's generated code gives a probe the structure but not a
//! sample to put in it. This emits the statements that fill one, from
//! the same IDL and the same JSON the ZeroDDS probe reads — so all four
//! probes serialise the same sample and a difference in their output is
//! a difference in their serialisers.
//!
//! ## Why this is not a self-oracle
//!
//! The emitter places values into the *vendor's* structures. The bytes
//! still come from the vendor's own serialiser. An emitter that placed
//! a value wrongly shows up as a disagreement between the vendors on
//! that case, not as a false agreement.
//!
//! ## What differs between the vendors
//!
//! Scalars and nested aggregates are the same C in Cyclone and RTI, and
//! setter calls in Fast DDS. Sequences differ in all three:
//!
//! ```text
//! sample.ids._length = 3; sample.ids._buffer = ids;   // Cyclone
//! DDS_LongSeq_set_length(&sample.ids, 3);             // RTI
//! sample.ids(std::vector<int32_t>{1, 2, 3});          // Fast DDS
//! ```
//!
//! ## What has no C shape to emit into
//!
//! XTypes 1.3 §7.5.1 defines the plain language binding by reusing the
//! OMG IDL-to-C mapping and then listing the extensions: primitive
//! types, annotations, bitmask types, external members. It lists no
//! bitset, no map and no fixed-point type, and the mapping it reuses
//! has none of the three either. Those three are refused here by name
//! rather than emitted from a guessed field shape — a guess would
//! compile against one vendor's header and put a wrong sample into the
//! other's, which is the failure this file exists to avoid. The ZeroDDS
//! probe serialises all three, so the ZeroDDS column of the matrix is
//! populated and the vendor columns are visibly empty.
//!
//! ## What the two generators do with an identifier C reserves
//!
//! They disagree, so the emitter has to as well. Measured 2026-08-11
//! over `tests/interop-corpus/identifier-target-keywords/type.idl`:
//! `rtiddsgen -language C` (Connext 7.7) generates
//! `DDS_Long _c_static;` for the member `static`, and Cyclone's `idlc`
//! (11.0.1) generates `int32_t static;` — verbatim, keyword and all, for
//! that member and for `new` beside it. So [`Vendor::Rti`] runs its
//! identifiers through [`crate::target_keywords::c`] and
//! [`Vendor::Cyclone`] runs them through nothing.
//!
//! The measurement covers member names. Enumerator and bitmask flag
//! names, and the module segments flattened onto them, take the same
//! rule here by extension rather than by measurement; no corpus case has
//! a C keyword in one of those positions, so nothing observable turns on
//! it yet.

use crate::contract::{self, Consumed};
use crate::encode::ProbeError;
use crate::target_keywords::c;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// Which implementation the statements are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    /// Eclipse Cyclone DDS, C.
    Cyclone,
    /// RTI Connext DDS, C.
    Rti,
}

impl Vendor {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Cyclone, Self::Rti];

    /// The name the command line uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cyclone => "cyclone",
            Self::Rti => "rti",
        }
    }

    /// Parse a command-line value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == value)
    }
}

/// The statements, ready to `#include` into a probe template.
#[derive(Debug, Default)]
pub struct Fill {
    /// Declarations that have to precede the assignments — the backing
    /// arrays a Cyclone sequence points at.
    declarations: String,
    /// The assignments themselves.
    statements: String,
    /// A counter, so two sequences in one sample get distinct names.
    next: usize,
    /// Which scalars of the value document the walk read.
    seen: Consumed,
}

impl Fill {
    /// The whole block, declarations first.
    #[must_use]
    pub fn into_source(self) -> String {
        let mut out = self.declarations;
        out.push_str(&self.statements);
        out
    }

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the statements that fill `sample` with `values`.
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
pub fn fill(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    vendor: Vendor,
) -> Result<String, ProbeError> {
    fill_recording(ast, type_, values, vendor, &mut Consumed::new())
}

/// As [`fill`], recording which scalars of the value were read.
///
/// # Errors
/// As [`fill`].
pub fn fill_recording(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    vendor: Vendor,
    seen: &mut Consumed,
) -> Result<String, ProbeError> {
    // Before the first hop. Every walk in this crate follows a typedef
    // by recursing on its target with nothing else changing, so a chain
    // that returns to itself has no end; the gate is what makes the
    // typedef arms below finite. See `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Fill::default();
    let root = contract::root_path(ast, type_);
    let result = emit(ast, type_, values, vendor, "sample", &mut out, &root);
    *seen = core::mem::take(&mut out.seen);
    result?;
    Ok(out.into_source())
}

/// The C name a type takes in the generated code.
///
/// Both C generators flatten the module path with underscores, so
/// `::common::util::Location` is `common_util_Location`.
///
/// The **flattened** name goes through `identifier`, not each segment
/// of it. Flattening is what decides the question: `::package::static`
/// is `package_static`, which C does not reserve and which
/// `rtiddsgen -language C` leaves alone, while a type declared at the
/// outermost scope keeps its own spelling and a keyword there is a
/// keyword. Measured 2026-08-11 on the Linux test host over
/// `tests/interop-corpus/identifier-target-keyword-typename`, whose
/// `@final struct static` rtiddsgen renders `_c_static`: the compiler
/// named it itself, `implicit declaration of function
/// 'staticPlugin_get_serialized_sample_max_size'; did you mean
/// '_c_staticPlugin_get_serialized_sample_max_size'?`.
#[must_use]
pub fn c_name(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor) -> String {
    ast.any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .map_or_else(
            || String::from("UNKNOWN"),
            |scoped| identifier(vendor, &scoped.segments().join("_")).into_owned(),
        )
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
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    vendor: Vendor,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float128) => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: match vendor {
                // Measured, Cyclone DDS 11.0.1: `idlc` stops at
                // "Long doubles are currently unsupported"
                // (`src/tools/idlc/src/libidlc/libidlc__descriptor.c`),
                // so there is no generated structure to fill.
                Vendor::Cyclone => "a long double, which Cyclone's idlc refuses to generate",
                // Measured, RTI Connext 7.7 on Linux: `DDS_LongDouble`
                // is `struct { char bytes[16]; }`, so no scalar literal
                // assigns to it and RTI names no representation for the
                // sixteen octets. Filling them would make this harness
                // the one choosing the wire format for a vendor.
                Vendor::Rti => "a long double, which RTI leaves as sixteen opaque octets",
            },
        }),
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(out.statements, "{lvalue} = {literal};");
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            let literal = c_string(text);
            match vendor {
                // An unbounded string is a plain `char *` in Cyclone's
                // generated struct and the probe never frees the sample,
                // so a literal is safe. A bounded one is an in-struct
                // array — `string<16>` becomes `char field[17];` — so
                // the literal is copied, never assigned.
                Vendor::Cyclone => match bound_count(ast, *bound, path)? {
                    Some(n) => {
                        let _ = writeln!(
                            out.statements,
                            "strncpy({lvalue}, {literal}, {n}); {lvalue}[{n}] = '\\0';"
                        );
                    }
                    None => {
                        let _ = writeln!(out.statements, "{lvalue} = (char *){literal};");
                    }
                },
                // RTI's `_initialize` has already allocated; `DDS_String_dup`
                // is what its own examples assign with.
                Vendor::Rti => {
                    let _ = writeln!(out.statements, "{lvalue} = DDS_String_dup({literal});");
                }
            }
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            // Cyclone maps a bounded `wstring<N>` to `wchar_t field[N+1]`
            // in the struct, so the code units are copied into it; only
            // the unbounded field is a pointer the buffer can be assigned
            // to.
            let bounded = vendor == Vendor::Cyclone && bound_count(ast, *bound, path)?.is_some();
            emit_wstring(text, lvalue, bounded, out);
            Ok(())
        }
        TypeKind::Bitmask { .. } => {
            let set = contract::bit_names(ast, type_, value, path)?;
            emit_bitmask(ast, type_, &set, vendor, lvalue, out, path);
            Ok(())
        }
        TypeKind::Enumeration { .. } => {
            // The name is checked against the declared set before it is
            // pasted into an identifier: a typo used to become either a
            // different enumerator or a compile error in the harness,
            // and the matrix reads both as a vendor difference.
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            out.seen.scalar(path);
            // Cyclone flattens the module path onto the enumerator;
            // rtiddsgen does not — its gen.h for a module-scoped enum
            // declares the enumerator bare (`virtual ,` in
            // `identifier-target-keywords`), and the top-level cases
            // measured the `_c_` keyword escape separately.
            let prefix = match vendor {
                Vendor::Cyclone => module_prefix(ast, type_, vendor),
                Vendor::Rti => String::new(),
            };
            let _ = writeln!(
                out.statements,
                "{lvalue} = {prefix}{};",
                identifier(vendor, &name)
            );
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            emit(ast, target, value, vendor, lvalue, out, path)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = contract::object(value, path)?;
            // The members with their C access expressions —
            // `crate::dump::c_members` carries the measured shape both
            // generators share: a base's members live in a nested
            // `parent` field while the wire order stays flat, and an
            // `@external` flag rides along (XTypes 1.3 §7.5.1.2.5).
            let c_fields = crate::dump::c_members(ast, type_, lvalue, vendor);
            let declared: Vec<String> = c_fields
                .iter()
                .map(|(member, _, _)| member.name.clone())
                .collect();
            contract::reject_unknown_keys(object, &declared, path)?;
            for (member, field, external) in &c_fields {
                // An absent `@optional` has nothing to fill; a required
                // member with no key is refused here and not left to
                // whichever backend happens to check.
                let Some(member_value) = contract::member_value(object, member, path)? else {
                    continue;
                };
                let member_path = contract::join(path, &member.name);
                // XTypes 1.3 §7.5.1.2.4 maps a present `@optional`
                // member to a pointer, §7.5.1.2.5 an `@external` one —
                // both generators declare `int32_t *a` — so the value
                // needs an object to point at. A string member is a
                // pointer already (`char *text` is both the carrier and
                // the value; NULL is the absence), so it fills as usual.
                // The string exemption is exact to the measured C:
                // an optional string is the plain pointer for both
                // generators, and Cyclone's external string is too —
                // only rtiddsgen wraps an @external string in a second
                // pointer (`DDS_Char **text`, annotation-external-sequence).
                let plain_pointer_string =
                    is_string_type(ast, member.type_) && (vendor == Vendor::Cyclone || !*external);
                if (member.optional || *external) && !plain_pointer_string {
                    let backing = out.temporary("opt");
                    let _ = writeln!(out.declarations, "static __typeof__(*{field}) {backing};");
                    emit(
                        ast,
                        member.type_,
                        member_value,
                        vendor,
                        &backing,
                        out,
                        &member_path,
                    )?;
                    let _ = writeln!(out.statements, "{field} = &{backing};");
                    continue;
                }
                emit(
                    ast,
                    member.type_,
                    member_value,
                    vendor,
                    field,
                    out,
                    &member_path,
                )?;
            }
            Ok(())
        }
        TypeKind::Union { .. } => {
            let object = contract::object(value, path)?;
            let discriminant = contract::discriminator(object, path)?;
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let Some((member_type, _)) = union_member(ast, type_, name) else {
                    return Err(ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: union_member_names(ast, type_),
                    });
                };
                contract::reject_null(member_value, &contract::join(path, name))?;
                // Both C generators put the members in a `_u` union.
                emit(
                    ast,
                    member_type,
                    member_value,
                    vendor,
                    &format!("{lvalue}._u.{}", identifier(vendor, name)),
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
                vendor,
                &format!("{lvalue}._d"),
                out,
                &contract::join(path, "_d"),
            )
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            emit_array(ast, element, value, vendor, lvalue, out, path, &counts)
        }
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            emit_sequence(ast, element, items, vendor, lvalue, out, path)
        }
        TypeKind::Bitset { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitset, for which XTypes 1.3 §7.5.1 defines no C binding",
        }),
        TypeKind::Map { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a map, which the IDL-to-C mapping §7.5.1 reuses does not have",
        }),
        TypeKind::Fixed { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a fixed, which the IDL-to-C mapping §7.5.1 reuses does not have",
        }),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// A `wstring` field, as a static array of code units.
///
/// Not a `L"..."` literal, and this is the reason. XTypes 1.3 §7.5.1.1.1
/// leaves the C spelling of `Char16` open: an implementation "may
/// consider `wchar_t` to be an equivalent C type if the platform
/// supports it and it is of sufficient size. Otherwise, they may map
/// Char16 to an equivalent integer type." A wide literal is `wchar_t`,
/// which is four bytes on Linux, so against a vendor that chose a
/// sixteen-bit holder it is the wrong array — and with a cast in front
/// of it, wrong without saying so.
///
/// `__typeof__` takes the element type from the vendor's own field, so
/// the emitter never names it. The initialiser is plain integers, which
/// convert to whichever integer type that turns out to be. GCC and
/// Clang both have the operator unconditionally, and the harness
/// compiles with one of them.
///
/// The trailing zero is the C string terminator, not a wire byte:
/// §7.4.1.1.2 keeps the terminator off the wire, but a `Char16 *` field
/// carries no length beside it, so the serialiser has no other way to
/// find the end.
fn emit_wstring(text: &str, lvalue: &str, bounded: bool, out: &mut Fill) {
    let buffer = out.temporary("wstr");
    let mut units = String::new();
    for unit in text.encode_utf16() {
        let _ = write!(units, "0x{unit:04x}, ");
    }
    let _ = writeln!(
        out.declarations,
        "static __typeof__(*{lvalue}) {buffer}[] = {{ {units}0x0000 }};"
    );
    if bounded {
        // The field is an in-struct array; assigning to it is a C
        // constraint violation, so the units are copied, terminator
        // included.
        let _ = writeln!(
            out.statements,
            "memcpy({lvalue}, {buffer}, sizeof({buffer}));"
        );
    } else {
        let _ = writeln!(out.statements, "{lvalue} = {buffer};");
    }
}

/// A string or sequence bound as a count, or a refusal that names the
/// bound — never a silent zero.
fn bound_count(
    ast: &Ast,
    bound: zerodds_ast::Bound,
    path: &str,
) -> Result<Option<i128>, ProbeError> {
    match bound.expr() {
        None => Ok(None),
        Some(expr) => ast
            .eval_integral(expr)
            .map(Some)
            .map_err(|_| ProbeError::NoLayout {
                detail: format!("{path}: a bound that does not evaluate to a count"),
            }),
    }
}

/// A bitmask field, as the flag names the binding declares, OR'ed.
///
/// XTypes 1.3 §7.5.1.2.2 defines the binding for a bitmask as if the
/// source had declared `@bit_bound(n) enum <TypeName>Bits` with
/// `@value(1 << <position>)` on each flag, plus a typedef of the type
/// name to the unsigned integer Table 44 gives for the bound. So the
/// flag names are enumerators, and both C generators put enumerators at
/// file scope with the module path flattened onto them — the same rule
/// the enumeration arm above uses.
///
/// Naming the flags rather than computing the integer keeps the
/// position-to-bit mapping on the vendor's side of the line. An emitter
/// that wrote `5` would be asserting the mapping that §7.4.1.1.3.2
/// specifies and the vendor is under test for.
#[allow(clippy::too_many_arguments)]
fn emit_bitmask(
    ast: &Ast,
    type_: Id<TypeNode>,
    set: &[(String, u16)],
    vendor: Vendor,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) {
    // The same vendor split as the enumerator emission: Cyclone
    // flattens the module path onto the flag, rtiddsgen leaves it bare.
    let prefix = match vendor {
        Vendor::Cyclone => module_prefix(ast, type_, vendor),
        Vendor::Rti => String::new(),
    };
    let mut flags = Vec::with_capacity(set.len());
    for (index, (name, _)) in set.iter().enumerate() {
        out.seen.scalar(&format!("{path}[{index}]"));
        flags.push(format!("{prefix}{}", identifier(vendor, name)));
    }

    // No flags set is the integer zero, not an empty expression.
    let value = if flags.is_empty() {
        "0".to_string()
    } else {
        flags.join(" | ")
    };
    let _ = writeln!(out.statements, "{lvalue} = {value};");
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `emit`, which is bounded by the sample.
#[allow(clippy::too_many_arguments)]
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    vendor: Vendor,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, value, vendor, lvalue, out, path);
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
            vendor,
            &format!("{lvalue}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    Ok(())
}

/// A sequence, in whichever shape the vendor's generated code has.
///
/// zerodds-lint: recursion-depth 128 — the sample's own nesting, below
/// `emit`, which carries the argument. Each element is a strictly smaller
/// part of the array this was handed.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    items: &[Value],
    vendor: Vendor,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) -> Result<(), ProbeError> {
    // Both shapes below need the element type, and §7.5.1.1.1 leaves the
    // C spelling of `Char16` to the vendor. It is still the vendor's own
    // field that carries it: Cyclone's `sequence<wstring>` is a
    // `dds_sequence_wstring` whose `_buffer` is a `wchar_t **`, so
    // `__typeof__(*{lvalue}._buffer)` is the element type without the
    // emitter naming it, and RTI generates `DDS_WstringSeq` — measured
    // 2026-08-10 with `idlc` and `rtiddsgen -language C` over
    // `@final struct Wide { sequence<wstring> texts; octet tail; };`.
    // This used to be refused on the reasoning that "there is no field
    // to take the type from until the sequence has one", and the
    // sequence *is* the field. AL-12.
    let count = items.len();
    match vendor {
        Vendor::Cyclone => {
            // A backing array the sequence points at. Declared ahead of
            // every assignment so a later member cannot reference it
            // before it exists.
            let buffer = out.temporary("seq");
            // Named where §7.5.1's reused mapping fixes the name, and
            // taken from the vendor's own `_buffer` where it does not.
            let element_c = if is_wide_string(ast, element) {
                format!("__typeof__(*{lvalue}._buffer)")
            } else {
                c_type_name(ast, element, vendor)
            };
            if count == 0 {
                let _ = writeln!(
                    out.statements,
                    "{lvalue}._length = 0; {lvalue}._maximum = 0; \
                     {lvalue}._buffer = NULL; {lvalue}._release = false;"
                );
                return Ok(());
            }
            let _ = writeln!(out.declarations, "static {element_c} {buffer}[{count}];");
            for (index, item) in items.iter().enumerate() {
                emit(
                    ast,
                    element,
                    item,
                    vendor,
                    &format!("{buffer}[{index}]"),
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
            let _ = writeln!(
                out.statements,
                "{lvalue}._length = {count}; {lvalue}._maximum = {count}; \
                 {lvalue}._buffer = {buffer}; {lvalue}._release = false;"
            );
            Ok(())
        }
        Vendor::Rti => {
            // RTI's sequences are opaque; the generated `<T>Seq` API is
            // the only way in.
            let seq = rti_sequence_name(ast, element, vendor);
            let _ = writeln!(
                out.statements,
                "{seq}_set_maximum(&{lvalue}, {count});\n\
                 {seq}_set_length(&{lvalue}, {count});"
            );
            for (index, item) in items.iter().enumerate() {
                // `get_reference` returns a pointer whatever the element
                // is, and `(*p)` is an lvalue of the element type, so
                // one expression serves every kind: `(*p) = 1` for a
                // scalar, `(*p).x` for a structure, `(*p)._u.a` for a
                // union, `(*p)._length` for a nested sequence.
                //
                // The `p->` form this used to build could only be
                // continued by a structure walk, and a union element was
                // handed to that walk: `_d` matched no member of a
                // union's own member list and the whole case was refused
                // with a path nobody could act on. One form, one
                // recursion, and every element kind reaches its own arm.
                let target = format!("(*{seq}_get_reference(&{lvalue}, {index}))");
                emit(
                    ast,
                    element,
                    item,
                    vendor,
                    &target,
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
            Ok(())
        }
    }
}

/// Whether `type_` is a `wstring` once typedefs are seen through.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn is_wide_string(ast: &Ast, type_: Id<TypeNode>) -> bool {
    match ast.type_(type_).map(|node| node.kind.clone()) {
        Ok(TypeKind::String { width, .. }) => width == CharWidth::WChar,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| is_wide_string(ast, target)),
        _ => false,
    }
}

/// The C type name of a sequence element, for the backing array.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one
/// and it stops at a collection rather than resolving past it; a chain
/// that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn c_type_name(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor) -> String {
    match ast.type_(type_).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => c_primitive(kind).to_string(),
        Ok(TypeKind::String { .. }) => "char *".to_string(),
        // A typedef whose target is a collection keeps its own name for
        // Cyclone: `idlc` emits the typedef (`typedef int32_t Triple[3];`,
        // `dds_sequence_Triple`), and resolving past it would leave the
        // backing array without a spellable type.
        Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
            None => "void".to_string(),
            Some(target) => {
                let collection = matches!(
                    ast.type_(target).map(|node| node.kind.clone()),
                    Ok(TypeKind::Sequence { .. } | TypeKind::Array { .. })
                );
                if collection && vendor == Vendor::Cyclone {
                    c_name(ast, type_, vendor)
                } else {
                    c_type_name(ast, target, vendor)
                }
            }
        },
        // Cyclone names an anonymous sequence after its element:
        // `sequence<long>` is `dds_sequence_long`, a nested
        // `sequence<sequence<long>>` is
        // `dds_sequence_sequence_dds_sequence_long`, `sequence<Cell>` is
        // `dds_sequence_Cell` — measured in the cells' own `gen.h`.
        Ok(TypeKind::Sequence { element, .. }) if vendor == Vendor::Cyclone => element
            .resolved()
            .map_or_else(|| "void".to_string(), |e| cyclone_sequence_name(ast, e)),
        _ => c_name(ast, type_, vendor),
    }
}

/// Whether the type, aliases resolved, is a `string` or `wstring` — the
/// kinds both C generators map to a pointer of their own.
pub(crate) fn is_string_type(ast: &Ast, type_: Id<TypeNode>) -> bool {
    let resolved = contract::resolve_alias(ast, type_);
    matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::String { .. })
    )
}

/// Cyclone's C name for an anonymous `sequence<element>`.
fn cyclone_sequence_name(ast: &Ast, element: Id<TypeNode>) -> String {
    format!("dds_sequence_{}", cyclone_sequence_tag(ast, element))
}

/// The element part of Cyclone's anonymous-sequence name.
fn cyclone_sequence_tag(ast: &Ast, element: Id<TypeNode>) -> String {
    match ast.type_(element).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => cyclone_primitive_tag(kind).to_string(),
        // A nested anonymous sequence contributes `sequence_` plus its
        // own generated name (gen.h of `sequence-nested`).
        Ok(TypeKind::Sequence { element: inner, .. }) => inner.resolved().map_or_else(
            || "void".to_string(),
            |e| format!("sequence_{}", cyclone_sequence_name(ast, e)),
        ),
        // Everything named — aliases, structs, unions, enums — appears
        // under its own flattened name.
        _ => c_name(ast, element, Vendor::Cyclone),
    }
}

/// The IDL keyword spelling Cyclone uses inside generated sequence names.
const fn cyclone_primitive_tag(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "bool",
        PrimitiveKind::Octet => "octet",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar",
        PrimitiveKind::Int8 => "int8",
        PrimitiveKind::UInt8 => "uint8",
        PrimitiveKind::Int16 => "short",
        PrimitiveKind::UInt16 => "unsigned_short",
        PrimitiveKind::Int32 => "long",
        PrimitiveKind::UInt32 => "unsigned_long",
        PrimitiveKind::Int64 => "long_long",
        PrimitiveKind::UInt64 => "unsigned_long_long",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => "long_double",
    }
}

/// The `<T>Seq` prefix RTI generates for a sequence of this element.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one
/// and it stops at a collection rather than resolving past it; a chain
/// that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn rti_sequence_name(ast: &Ast, element: Id<TypeNode>, vendor: Vendor) -> String {
    match ast.type_(element).map(|node| node.kind.clone()) {
        // The sequence names keep int8/uint8 distinct even though the
        // scalar C types fold onto Char/Octet: rtiddsgen 7.7's own
        // header for `sequence<uint8, 4>` declares
        // `struct DDS_UInt8Seq` beside `DDS_OctetSeq` for `octet`
        // (measured in the cells' gen.h).
        Ok(TypeKind::Primitive(PrimitiveKind::UInt8)) => "DDS_UInt8Seq".to_string(),
        Ok(TypeKind::Primitive(PrimitiveKind::Int8)) => "DDS_Int8Seq".to_string(),
        Ok(TypeKind::Primitive(kind)) => format!("DDS_{}Seq", rti_primitive(kind)),
        // `DDS_WstringSeq` for a wide one, measured 2026-08-10 in
        // `rtiddsgen -language C`'s own header for
        // `sequence<wstring>`: `struct DDS_WstringSeq texts;`.
        Ok(TypeKind::String {
            width: CharWidth::WChar,
            ..
        }) => "DDS_WstringSeq".to_string(),
        Ok(TypeKind::String { .. }) => "DDS_StringSeq".to_string(),
        // A typedef of a collection keeps its own name: rtiddsgen
        // names the sequence after the alias (`TripleSeq` for
        // `sequence<Triple>` with `typedef long Triple[3]`), and
        // resolving past it leaves nothing spellable.
        Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
            None => "DDS_LongSeq".to_string(),
            Some(target) => {
                let collection = matches!(
                    ast.type_(target).map(|node| node.kind.clone()),
                    Ok(TypeKind::Sequence { .. } | TypeKind::Array { .. })
                );
                if collection {
                    format!("{}Seq", c_name(ast, element, vendor))
                } else {
                    rti_sequence_name(ast, target, vendor)
                }
            }
        },
        _ => format!("{}Seq", c_name(ast, element, vendor)),
    }
}

const fn c_primitive(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "bool",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "uint8_t",
        PrimitiveKind::Int8 => "int8_t",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar_t",
        PrimitiveKind::Int16 => "int16_t",
        PrimitiveKind::UInt16 => "uint16_t",
        PrimitiveKind::Int32 => "int32_t",
        PrimitiveKind::UInt32 => "uint32_t",
        PrimitiveKind::Int64 => "int64_t",
        PrimitiveKind::UInt64 => "uint64_t",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => "double",
    }
}

/// The name RTI puts in `DDS_<name>Seq`.
const fn rti_primitive(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "Boolean",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "Octet",
        PrimitiveKind::Int8 => "Char",
        PrimitiveKind::Char => "Char",
        PrimitiveKind::WChar => "Wchar",
        PrimitiveKind::Int16 => "Short",
        PrimitiveKind::UInt16 => "UnsignedShort",
        PrimitiveKind::Int32 => "Long",
        PrimitiveKind::UInt32 => "UnsignedLong",
        PrimitiveKind::Int64 => "LongLong",
        PrimitiveKind::UInt64 => "UnsignedLongLong",
        PrimitiveKind::Float32 => "Float",
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => "Double",
    }
}

/// The flattened module path an enumerator carries, with a trailing
/// underscore when there is one.
///
/// Each segment is an identifier the generator had to render, so each
/// goes through [`identifier`]. Extended from the member measurement
/// rather than measured: the corpus's one module called after a keyword
/// is `package`, which C does not reserve.
fn module_prefix(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor) -> String {
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
    let mut prefix = segments[..segments.len() - 1]
        .iter()
        .map(|segment| identifier(vendor, segment).into_owned())
        .collect::<Vec<String>>()
        .join("_");
    prefix.push('_');
    prefix
}

/// One identifier, as the vendor's own generator spells it.
///
/// The whole of the difference between the two C columns of the matrix
/// on this question. Measured 2026-08-11 over
/// `tests/interop-corpus/identifier-target-keywords/type.idl`: RTI
/// renames what C reserves and Cyclone renames nothing, so the C keyword
/// list is applied to one and not to the other. An emitter that renamed
/// for both would stop agreeing with Cyclone's header, which spells the
/// member `static`.
pub(crate) fn identifier(vendor: Vendor, name: &str) -> Cow<'_, str> {
    // A member whose name is an IDL keyword can only have been written
    // in the escaped spelling (`_long`), and rtiddsgen keeps that
    // spelling in its C field name — measured over
    // `identifier-escaped-keyword`: the generated struct carries
    // `_long`, and the `_c_` rename of the C list is not applied
    // (`did you mean '_long'?` was the compiler's answer to guessing
    // it). The model's name is the identifier, so the escape is put
    // back from the keyword table.
    if zerodds_idl_cst::keyword_collision(name).is_some()
        || zerodds_idl_cst::KEYWORDS
            .iter()
            .any(|(keyword, _)| *keyword == name)
    {
        return match vendor {
            Vendor::Cyclone => Cow::Borrowed(name),
            Vendor::Rti => Cow::Owned(format!("_{name}")),
        };
    }
    match vendor {
        Vendor::Cyclone => Cow::Borrowed(name),
        Vendor::Rti => c(name),
    }
}

fn union_member(ast: &Ast, union: Id<TypeNode>, name: &str) -> Option<(Id<TypeNode>, String)> {
    ast.cases_of(union)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .find(|member| member.name == name)
        .and_then(|member| Some((member.type_.resolved()?, member.name.clone())))
}

fn union_member_names(ast: &Ast, union: Id<TypeNode>) -> Vec<String> {
    ast.cases_of(union)
        .iter()
        .filter_map(|id| ast.case(*id).ok())
        .filter_map(|case| ast.member(case.member).ok())
        .map(|member| member.name.clone())
        .collect()
}

/// The C literal for one primitive.
///
/// Floating point goes out with seventeen significant digits, which is
/// what a `double` round-trips through — a literal written shorter
/// would put different bytes on the wire than the ZeroDDS probe's.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => {
            if contract::boolean(value, path)? {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // An integer and not a character constant. `'x'` in C is an
        // `int` of the *narrow* execution character set, so a code point
        // above U+007F becomes a multi-character constant: measured on
        // the recording host, gcc 14.2.0 compiles `'\u{e4}'` to `0xC3A4`
        // — the two UTF-8 octets packed — and `'\u{1234}'` to `0x88B4`,
        // the last two of three, with nothing but `-Wmultichar` to say
        // so. Those are the octets this tree recorded as Cyclone's and
        // RTI's `wchar`; every other emitter here already writes an
        // integer, and `corba_cpp::primitive_literal` carries the same
        // argument for `CORBA::WChar`.
        //
        // Plain integers rather than a cast, for `emit_wstring`'s
        // reason: XTypes §7.5.1.1.1 leaves the C spelling of `Char16`
        // open, so the width is the vendor's field's and an integer
        // converts to whichever it turns out to be.
        //
        // `character_for` and not `character`: a code point the width
        // cannot carry is refused here rather than narrowed in silence,
        // which is what the serialiser does with the same value.
        PrimitiveKind::Char => {
            format!(
                "0x{:02x}",
                contract::character_for(kind, value, path)? as u32
            )
        }
        PrimitiveKind::WChar => {
            format!(
                "0x{:04x}",
                contract::character_for(kind, value, path)? as u32
            )
        }
        PrimitiveKind::Float32 => {
            format!("{:.17e}f", contract::float(value, path)?)
        }
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            format!("{:.17e}", contract::float(value, path)?)
        }
        PrimitiveKind::Int64 => {
            let n = contract::integer_for(kind, value, path)?;
            // C has no literal for INT64_MIN: `-9223372036854775808LL`
            // is a positive constant too large for `long long`, parsed
            // unsigned and then negated, and gcc says so. The same
            // subtraction `<stdint.h>` uses spells the value exactly.
            if n == i128::from(i64::MIN) {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{n}LL")
            }
        }
        PrimitiveKind::UInt64 => {
            format!("{}ULL", contract::integer_for(kind, value, path)?)
        }
        // The range check is the contract's, not `try_from`'s: a `300`
        // written into a `uint8_t` field is `44` on the wire and no
        // compiler this harness uses makes a noise about it.
        _ => format!("{}", contract::integer_for(kind, value, path)?),
    })
}

/// A C string literal.
///
/// Every byte outside printable ASCII goes out as a three-digit octal
/// escape, which is `corba_cpp::cpp_string`'s rule and for its reason:
/// `\xHH` is greedy, so `"\xc3a"` is one character with an out-of-range
/// value and a UTF-8 string followed by a hex digit changes meaning.
/// Three-digit octal cannot run on.
///
/// Not the raw UTF-8 octets this used to emit. Those compile only where
/// the compiler's narrow execution character set is UTF-8; the ORB
/// emitter never relied on that and this one no longer does either.
fn c_string(text: &str) -> String {
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
