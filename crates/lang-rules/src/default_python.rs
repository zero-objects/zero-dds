// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Python member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Python. [`crate::default_c`] is the
//! same table as C, [`crate::default_java`] as Java, [`crate::default_cpp`]
//! as C++, [`crate::default_typescript`] as TypeScript, and
//! `crates/wire-probe/src/default.rs` is the reflective column's; that
//! module carries the clause quotations and the reasoning for the three
//! kinds Table 9 does not list. Deliberately a sixth derivation and not
//! a call into any of them: the columns are compared against each other,
//! and a shared helper would make them agree about a mistake.
//!
//! ## Why it is a generated function and not a literal
//!
//! A literal cannot carry an aggregate. "A structure without any of the
//! optional members and with other members set to their default values"
//! is a constructor call with one argument per required member, and
//! writing that at every position a member can go missing would put the
//! same answer in the generated file as many times as the type is
//! reached — where two copies can differ. One function per type is one
//! answer per type, which is what the C and Java columns do and for the
//! same reason.
//!
//! ## Why it answers with the value instead of filling one
//!
//! C's defaulter is `void zd_def_T(T *v)`: it writes through a pointer.
//! Python has no pointer and no out-parameter — rebinding a parameter
//! rebinds the local and nothing else — so the function **returns**
//! Table 9's value and the caller assigns it, which is the same decision
//! one step later. It is also what makes the reader's own shape work:
//! [`crate::emit::python::Python`]'s decoder opens with
//! `v = zd_def_T()`, so a position the stream does not carry is already
//! what Table 9 says before a single octet is read.
//!
//! ## What Python forces that TypeScript does not
//!
//! **A body is one `return`.** Every other column fills a value member
//! by member, because it has something to fill: C writes through a
//! pointer, Java and TypeScript `new` an empty object first. A generated
//! Python aggregate is a `@dataclasses.dataclass(kw_only=True)` whose
//! required fields have **no default**, so `S()` is a `TypeError` and
//! there is no empty object to fill — the whole value is one call. That
//! is what `adapter/python.rs` meant by leaving the fields required, and
//! `emit/python.rs`'s `reading_body` says the same thing from the other
//! side.
//!
//! **Every argument is named.** Two independent reasons, either of which
//! decides it. `kw_only=True` on the dataclass makes a positional
//! argument a `TypeError` outright; and `adapter/python.rs` builds the
//! field list from `all_members_of`, which flattens a base's members in
//! front of the derived ones — so a positional list would silently
//! transpose two same-typed members if that order ever changed. Named,
//! a field that moves or is renamed is a `TypeError` at the call naming
//! the parameter.
//!
//! **An `@optional` member is left out of the call.** §7.2.2.4.4.4.8's
//! "Implementations shall not provide a default value" is the absence
//! here, and `adapter/python.rs` already declares such a field
//! `typing.Optional[T] = None`. Passing `field=None` would say the same
//! thing in a way that stops saying it the day the declared default
//! moves; leaving the argument out takes the declaration's answer.
//!
//! **A union is one dataclass and not one arm per case.**
//! `adapter/python.rs` declares `_d` beside one `typing.Optional` field
//! per case, so the value is `U(_d=…, one=…)` — or `U(_d=…)` alone where
//! the discriminator's own default selects no branch, which Table 9
//! allows and which a Python dataclass can hold as it stands.
//!
//! **An enumeration's default is the enumerator, written from the module
//! root.** `a.b.Colour.RED` and not the number it happens to be: a
//! generated codec sits at the module root and `adapter/python.rs` nests
//! the `enum.IntEnum` in a class per IDL module, so `RED` there names
//! nothing at all. Through [`crate::emit::CodecTarget::enumerator_constant`]
//! rather than a second derivation, so the string is the one
//! `emit/python.rs` writes at every other site.
//!
//! **A `long double` is a `Float128` and never a `float`.** A Python
//! `float` is a binary64 with 53 significand bits and a `long double` is
//! a binary128 with 113, so a zero written as `0.0` would be a member of
//! the wrong type before any octet is read.
//! `runtime/python/zerodds_wire/float128.py` carries the sixteen octets
//! and `Float128.zero()` is its zero bit pattern.
//!
//! **A `fixed<P,S>` is a decimal string.** Table 9's zero for it is the
//! one spelling `crates/wire-probe/src/canonical.rs` accepts for that
//! scale — `"0.00"` for a `fixed<5,2>` — and not the empty string, which
//! is not a decimal at all.
//!
//! **A `char` and a `wchar` are a `str` of one.** Table 9's zero for a
//! character is the octet zero, which the C column writes as `0` and
//! this one as `"\x00"`: Python has no character type, the member holds
//! a one-character `str`, and an integer there would be a different type
//! rather than a different spelling of one value.
//!
//! **A fixed array is that many elements, spelled out, as a tuple.**
//! Python has no `Array.from(…, () => …)` and this tree has no
//! comprehension, and `(zd_def_T(),) * n` is the one form that must not
//! be used: it
//! evaluates the element **once** and repeats the reference, so writing
//! through element 0 of an array of aggregates would change all of them.
//! One call per element evaluates once per element. The cost is the
//! adapter's already — `adapter/python.rs` writes `tuple[T, …]` with one
//! argument per element for the same array — and it is stated in both
//! places rather than hidden by a threshold at which the spelling
//! changes.
//!
//! **A `typedef` names the same defaulter as its target.**
//! `adapter::python::type_tag` follows an alias through, so
//! [`defaulter_name`] answers `zd_def_Point` for `Position` as well —
//! and an alias arm that answered `return zd_def_Point()` would be a
//! function whose body calls itself. So a typedef is resolved *before*
//! the table is read, exactly as `emit/python.rs`'s `value_type_of`
//! resolves it, and the one function that results is the target's.

use crate::adapter::cst::MaterialiseError;
use crate::adapter::python::declared_path;
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::CodecTarget;
use crate::emit::python::Python;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::python::{self as py, escape_name};

/// How deep a chain of typedefs is followed before the model is taken to
/// be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type.
///
/// Keyed the same way [`crate::emit::decode::decoder_name`] is, so a
/// primitive's defaulter is named by its IDL kind and not by its Python
/// spelling. That distinction does more work in this column than in any
/// other: `octet`, `unsigned short`, `long` and `unsigned long long` are
/// one Python type and four wire widths, and a name built from the
/// Python type would give all four one defaulter.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", Python::type_name(ast, id)),
    }
}

/// The defaulter that goes with a decoder, by name.
///
/// The two schemes differ in one word — `zd_dec_p_long` against
/// `zd_def_p_long` — because both are built from the same key. Doing it
/// by text lets [`crate::emit::python::Python`] name the defaulter from
/// a routine that has only a signature; a name that does not resolve is
/// a `NameError` naming the function.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    }
}

/// `place = zd_def_<T>()`.
#[must_use]
pub fn default_into(ast: &Ast, id: Id<TypeNode>, place: py::Expr) -> py::Statement {
    set(place, call(&defaulter_name(ast, id), Vec::new()))
}

/// One member's default: its `@default`, or its type's.
///
/// §7.2.2.4.4.4.9 puts the explicit one first — "An explicit definition
/// associates a default value with the member at the time where the
/// aggregated type is defined" — and falls back to "the default value of
/// the member type. See Table 9."
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a Python expression. Refused rather than
/// silently replaced by Table 9's zero: a member the designer gave a
/// default and a member that got a zero instead are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: py::Expr,
) -> Result<Vec<py::Statement>, EncodeError> {
    Ok(vec![set(place, member_value(ast, node, type_)?)])
}

/// The defaulters `decls` call, and exactly those.
///
/// The set is read **off the emitted calls** rather than derived a second
/// time from the model, for the reason
/// [`crate::default_c::c_defaulters_called_by`] gives at length: a
/// derivation beside the decoder's is a second reading of §7.2.2.7, and
/// the two readings drift. The closure is taken by emitting and
/// re-reading until the batch calls nothing it does not also define.
///
/// # Errors
/// [`EncodeError`] for a construct with no Python default.
pub fn python_defaulters_called_by(
    ast: &Ast,
    decls: &[py::Statement],
) -> Result<Vec<py::Statement>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_statement(decl, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    // Bounded by the number of named defaulters, because every round but
    // the last adds at least one. Written down rather than left to the
    // loop, so a name that never resolves cannot spin.
    for _ in 0..=index.len() {
        let mut grew = false;
        for name in &called {
            if have.contains(name) {
                continue;
            }
            have.push(name.clone());
            // A name with no type behind it cannot happen for a call
            // this generator emitted — every one came from
            // `defaulter_name` over a type in this model — and if it
            // ever does, the omission is a `NameError` naming the
            // function at the first sample, which is louder than
            // anything this could return.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = python_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_statement(decl, &mut called);
        }
    }
    python_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Python default.
pub fn python_defaulters_over(
    ast: &Ast,
    types: &[Id<TypeNode>],
) -> Result<Vec<py::Statement>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let Some(value) = value_of(ast, id, &name)? else {
            continue;
        };
        emitted.push(name.clone());
        out.push(py::Statement::Function(py::Function {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            decorators: Vec::new(),
            name: Ident::new(&name),
            params: Vec::new(),
            returns: Some(Python::value_type(ast, id)),
            body: vec![py::Statement::Return(Some(value))],
        }));
    }
    Ok(out)
}

/// The value one defaulter answers with, or `None` where the type has no
/// Python data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<py::Expr>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name, so an arm answering with a call
    // would name this very function. See the module documentation.
    let id = resolve_alias(ast, id);
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    // A type the model does not hold. It used to answer `Ok(None)`,
    // which the caller reads as "nothing to emit for this one" and
    // skips — so a type whose handle no longer resolves quietly loses
    // its defaulter, and the first sign of it is a member left at
    // whatever the language zero-initialises to. Every other arm of
    // this function either answers a value or refuses by name; this
    // one now does too.
    let Ok(node) = ast.type_(id) else {
        return Err(refuse("a type the model does not hold"));
    };
    let value = match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),

        // `""`, for both widths: a Python `str` is a sequence of code
        // points already and the empty string is the same string in
        // either.
        TypeKind::String { .. } => text(""),

        // A `fixed<P,S>` is the decimal it was written as, and
        // `canonical::decimal` accepts exactly one spelling of zero for
        // a given scale.
        TypeKind::Fixed { scale, .. } => text(&zero_decimal(*scale)),

        // Only an alias `resolve_alias` could not follow reaches here.
        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10. By name and not as
        // the number it happens to be: `adapter/python.rs` declared the
        // member, and a generated file that spelled the value instead
        // would keep working after `@value` moved it.
        TypeKind::Enumeration { .. } => {
            let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
                return Err(refuse(
                    "enumeration with no enumerator to take as its default",
                ));
            };
            Python::enumerator_constant(ast, id, &enumerator)
        }

        // No bit set, which for an `enum.IntFlag` is the value zero.
        // `a.b.Flags(0)` and not a bare `0`: `IntFlag(0)` is a legal
        // member at every `@bit_bound` and is an `int`, so it satisfies
        // both the declared field — `adapter/python.rs` annotates the
        // member `int` — and this function's own return annotation,
        // which `emit/python.rs`'s `value_type_of` gives as the
        // declared class. A bare `0` satisfies only the first.
        TypeKind::Bitmask { .. } => match declared_path(ast, id) {
            Ok(path) => py::Expr::Call {
                func: path,
                args: vec![py::Arg::Positional(py::Expr::int(0))],
            },
            // The name is unreachable from the module root, and a
            // bitmask no annotation can name is still zero.
            Err(_) => py::Expr::int(0),
        },

        // A bitset's fields are separate integers in `adapter/python.rs`
        // — a dataclass with one field per bitfield, each stating its
        // own width in the metadata — so its default is that dataclass
        // with every field at zero. The packing into the anchor is the
        // codec's and does not appear here at all.
        TypeKind::Bitset { .. } => {
            let mut args = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding: it moves the next
                // field along and holds no value, so the declaration has
                // no field for it either.
                if let Some(field_name) = field_node.name.as_ref() {
                    args.push((escape_name(field_name), py::Expr::int(0)));
                }
            }
            construct(declared(ast, id, own_name)?, args)
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries. An empty `list` and not `None`:
        // `None` is what an `@optional` member's absence means here, and
        // one value cannot mean both. §7.2.2.4.4.4.7 asks for a
        // zero-length sequence, and §7.2.2.4.4.4.8's "Implementations
        // shall not provide a default value" for an `@optional` member
        // is the absence.
        //
        // A `list` and not a `tuple` for both: `emit/python.rs`'s
        // `value_type_of` answers `list[…]`, because the reader writes
        // elements into the slot it was handed.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => py::Expr::List(Vec::new()),

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding type."
        //
        // One call per element, one `tuple` per dimension — the
        // `tuple[T, …]` `adapter/python.rs` declares the member as. See
        // the module documentation for why `(zd_def_T(),) * n` is not
        // this: it evaluates the element once and repeats the reference.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(refuse("array with an unresolved element"));
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let length = ast
                    .eval_integral(*dim)
                    .map_err(|_| refuse("array whose length does not evaluate"))?;
                lengths.push(
                    usize::try_from(length)
                        .map_err(|_| refuse("array of a length this column cannot spell"))?,
                );
            }
            let mut out = call(&defaulter_name(ast, element), Vec::new());
            // Innermost dimension first, so the outer one ends up
            // outermost: `long a[2][3]` is two rows of three, which is
            // what the walk indexes `[i0][i1]`.
            for length in lengths.into_iter().rev() {
                out = py::Expr::Tuple(vec![out; length]);
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut args = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                args.push((
                    escape_name("parent"),
                    call(&defaulter_name(ast, base), Vec::new()),
                ));
            }
            for member_id in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member_id) else {
                    continue;
                };
                // Left out rather than passed as `None`: the field's
                // declared default is already the absence, and one
                // answer per question.
                if member_node.facets.is_optional() {
                    continue;
                }
                let Some(member_type) = member_node.type_.resolved() else {
                    return Err(EncodeError::NoWriter {
                        member: member_node.name.clone(),
                        kind: "member with an unresolved type",
                    });
                };
                args.push((
                    escape_name(&member_node.name),
                    member_value(ast, member_node, member_type)?,
                ));
            }
            construct(declared(ast, id, own_name)?, args)
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type."
        //
        // Where it selects none, `_d` alone is the whole value. That is
        // Table 9 read literally, and it is available here where it is
        // not in the TypeScript column: `adapter/typescript.rs`'s
        // `union_default` falls back to the *first* case because a `_d`
        // matching no arm does not inhabit a discriminated union type at
        // all. A Python union is one dataclass whose case fields all
        // default to `None`, so there is no such pressure and no case is
        // invented.
        TypeKind::Union { discriminator, .. } => {
            let Some(discriminator) = discriminator.resolved() else {
                return Err(refuse("union with an unresolved discriminator"));
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut args = vec![(
                "_d".to_string(),
                discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            )];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                args.push((field, call(&defaulter_name(ast, member_type), Vec::new())));
            }
            construct(declared(ast, id, own_name)?, args)
        }

        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Ok(None);
        }
        TypeKind::ValueBox { .. } => {
            return Err(refuse(
                "boxed value, whose presence flag has no settled wire form",
            ));
        }
        // Refused by name rather than passed over. `any` is in the model
        // because IDL 4.2 has it; DDS-XTypes has no `any` and no
        // encoding for one, and only CORBA CDR writes it — as a TypeCode
        // ahead of the value. Returning `Ok(None)` here would generate a
        // reader that silently omitted the member.
        TypeKind::Any => {
            return Err(refuse(
                "`any`, which DDS-XTypes has no encoding for; only CORBA CDR writes one, \
                 as a TypeCode ahead of the value",
            ));
        }
        TypeKind::ValueBase => {
            return Err(refuse(
                "`ValueBase`, which DDS-XTypes has no encoding for; only CORBA CDR writes \
                 one, as a chunked value with its repository id",
            ));
        }
    };
    Ok(Some(value))
}

/// One member's default as a value: its `@default`, or a call to its
/// type's defaulter.
///
/// The value and not the assignment, because a Python aggregate is
/// constructed in one call and there is nothing to assign into yet.
/// [`member_default`] is the same answer one step later.
fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<py::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(call(&defaulter_name(ast, type_), Vec::new())),
    }
}

/// Table 9's primitive rows, as the Python holder each takes.
///
/// Ten of the fifteen are the same `int` zero, because a Python `int`
/// holds every IDL integer width exactly — see `emit/python.rs`'s
/// [`CodecTarget::scalar`]. The five that differ are the five where
/// Python has a type of its own.
fn primitive_default(kind: PrimitiveKind) -> py::Expr {
    match kind {
        PrimitiveKind::Boolean => py::Expr::Literal(Literal::Bool(false)),
        // `"\x00"`, a one-character `str`: Python has no character type,
        // so an integer here would be a different type and not a
        // different spelling of the same value. The octet is the C
        // column's `0`.
        PrimitiveKind::Char | PrimitiveKind::WChar => py::Expr::Literal(Literal::Char('\0')),
        // `0.0` and not `0`: the member is annotated `float`, and the
        // spelling is what says which of Python's two numeric types the
        // value is.
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            py::Expr::Literal(Literal::Float("0.0".to_string()))
        }
        // A binary128 zero is its zero bit pattern.
        // `runtime/python/zerodds_wire/float128.py` names it
        // `Float128.zero()`, a classmethod whose one parameter has a
        // default, so the spelling does not depend on the sign or on the
        // class's internal layout.
        PrimitiveKind::Float128 => py::Expr::Call {
            func: Path::new(&["Float128", "zero"]),
            args: Vec::new(),
        },
        PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => py::Expr::int(0),
    }
}

/// The path a generated declaration of `id` is reached by, from the
/// module root.
///
/// `adapter/python.rs` nests an IDL module in a class and a defaulter
/// sits at the module root, so `Point` there names nothing at all — or,
/// worse, names whatever else happened to be bound.
fn declared(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Path, EncodeError> {
    declared_path(ast, id).map_err(|_| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind: "aggregate whose declared name the module chain will not answer for",
    })
}

/// `a.b.S(x=…, y=…)` — every argument named, for the two reasons the
/// module documentation gives.
fn construct(path: Path, args: Vec<(String, py::Expr)>) -> py::Expr {
    py::Expr::Call {
        func: path,
        args: args
            .into_iter()
            .map(|(name, value)| {
                py::Arg::Keyword(py::Keyword {
                    name: Ident::new(&name),
                    value,
                })
            })
            .collect(),
    }
}

/// The case a defaulted discriminator selects, if any.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: Option<i128>,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, escape_name(&member.name))))
}

/// A discriminator value in the Python form its type gives it.
///
/// The four shapes are not interchangeable and the codec says so:
/// `emit/python.rs`'s `cast` rewrites every case label into the
/// discriminator's own form before comparing it, so a `_d` written as a
/// number where the labels are characters would select no case at all —
/// `"a" == 97` is `False` for every sample.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> py::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        // The enumerator and not the number, for the reason the
        // enumeration row gives. An `enum.IntEnum` member *is* an `int`,
        // so it compares equal to the label the codec writes.
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return Python::enumerator_constant(ast, resolved, &enumerator);
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => py::Expr::Literal(Literal::Bool(value != 0)),
        // `value` here is Table 9's and never a designer's: a `char` or
        // `wchar` discriminator's default is zero — the arm above says
        // so — and the only other source is an enumerator, which the
        // enumeration branch has already answered. So the fallback is
        // unreachable rather than a substitution, and `'\0'` is the
        // value it would be answering with anyway.
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            char_literal(u32::try_from(value).unwrap_or(0))
        }
        _ => py::Expr::int(value),
    }
}

/// The primitive `id` is once typedefs are seen through, or `None` where
/// it is not one.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_ALIAS_DEPTH`)
fn resolved_primitive(ast: &Ast, id: Id<TypeNode>) -> Option<PrimitiveKind> {
    let mut current = id;
    for _ in 0..MAX_ALIAS_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Primitive(kind)) => return Some(kind),
            Ok(TypeKind::Alias { aliased }) => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// An explicit `@default` as a Python expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<py::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = ast
        .expr(expr)
        .map_err(|_| refuse("a `@default` this frontend cannot follow"))?;
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    let is_float128 = matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128)));
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(py::Expr::Literal(Literal::Bool(*flag)))
        }
        // A one-character string, which is what a `char` member is here.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(char_literal(u32::from(*ch))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(char_literal(*code)),
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(text(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let Some(ch) = char::from_u32(*code) else {
                    return Err(refuse("a `@default` wide string that is not code points"));
                };
                out.push(ch);
            }
            Ok(text(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = exponent
                .as_ref()
                .map_or(String::new(), |text| format!("e{text}"));
            let written = format!("{mantissa}{exponent}");
            if is_float128 {
                // Through the decimal and never through a Python
                // `float`: `Float128.from_decimal` is exact, and a
                // literal would round the source's digits to binary64
                // sixty bits before the value reached the member.
                return Ok(py::Expr::Call {
                    func: Path::new(&["Float128", "from_decimal"]),
                    args: vec![py::Arg::Positional(text(&written))],
                });
            }
            // No suffix and no cast: a `float` member and a `double`
            // member are the same Python `float`, and the rounding to
            // binary32 is the writer's, at the octet.
            Ok(py::Expr::Literal(Literal::Float(written)))
        }
        // A `fixed<P,S>` is the decimal text here, as the member's type
        // is a `str`.
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(text(&fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let target = reference
                .resolved()
                .ok_or_else(|| refuse("a `@default` enumerator that is unresolved"))?;
            let enumerator = ast
                .enumerator(target)
                .map_err(|_| refuse("a `@default` enumerator this frontend cannot follow"))?;
            Ok(Python::enumerator_constant(ast, resolved, &enumerator.name))
        }
        _ => {
            // An integer literal, a named constant or arithmetic over
            // them, folded the same way a bound or a case label is.
            let value = ast
                .eval_integral(expr)
                .map_err(|_| refuse("a `@default` this frontend cannot evaluate"))?;
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    py::Expr::Literal(Literal::Bool(value != 0))
                }
                // Refused rather than folded to NUL where the value has
                // no code unit at all. A `@default` the designer wrote
                // and a member that got `"\x00"` instead are different
                // samples, and §7.2.2.4.4.4.9 associates a value with
                // the member — a value this column cannot spell is a
                // fact about the column and belongs in the exit code.
                //
                // The bound is `0xFFFF` and not "a Unicode scalar
                // value": §7.4.1.1.1 writes a `wchar` as one UTF-16 code
                // unit, and a lone surrogate is one. Refusing the
                // surrogates would refuse what `chr` spells and the wire
                // carries, which is the same substitution one door over.
                Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
                    let point = u32::try_from(value)
                        .ok()
                        .filter(|code| *code <= u32::from(u16::MAX))
                        .ok_or_else(|| {
                            refuse("a `@default` character outside the range of a UTF-16 code unit")
                        })?;
                    char_literal(point)
                }
                // `Float128.from_int` and not a literal: the member is a
                // binary128 and an `int` there is a different type.
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => py::Expr::Call {
                    func: Path::new(&["Float128", "from_int"]),
                    args: vec![py::Arg::Positional(py::Expr::int(value))],
                },
                // Every integer width, with no widening: a Python `int`
                // is unbounded, so 18446744073709551615 is itself here
                // where the Java column has to carry it as `-1L`.
                _ => py::Expr::int(value),
            })
        }
    }
}

/// A code point as the one-character `str` a `char` member holds.
///
/// `chr(<code>)` and not a character literal, because `Literal::Char`
/// carries a Rust `char` and a Rust `char` is a Unicode scalar value. A
/// Python `str` is a sequence of code points and `chr(0xD800)` is a
/// one-character string; the two types disagree exactly on
/// `0xD800..=0xDFFF`, and `char::from_u32(code).unwrap_or('\0')` answered
/// that disagreement by substituting NUL with no diagnostic.
///
/// The same fold was measured in the C# and TypeScript columns on
/// `@default(0xD800) wchar c`, where they wrote `'\0'` while the C column
/// wrote `55296` and the Java column `(char) 55296`. The numeric path in
/// [`explicit`] already refused rather than folded; this is the literal
/// path, which nothing had reached.
///
/// Above `0xFFFF` there is no single UTF-16 code unit, and §7.4.1.1.1
/// writes a `wchar` as one; the callers bound the value before it gets
/// here.
fn char_literal(code: u32) -> py::Expr {
    match char::from_u32(code) {
        // The character itself where Rust can spell it, so a readable
        // `@default` stays readable in the generated file.
        Some(point) => py::Expr::Literal(Literal::Char(point)),
        None => py::Expr::Call {
            func: Path::new(&["chr"]),
            args: vec![py::Arg::Positional(py::Expr::int(i128::from(code)))],
        },
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one statement calls, appended to `out`.
///
/// The counterpart of the TypeScript column's `calls_in_decl`. Python
/// has no separate declaration node — a `def` and a `class` are
/// statements — so the walk starts one level down and covers every
/// compound form, because a defaulter call reaches this from inside a
/// loop body as readily as from a `return`.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_statement(statement: &py::Statement, out: &mut Vec<String>) {
    match statement {
        py::Statement::Class(class) => calls_in_body(&class.body, out),
        py::Statement::Function(function) => calls_in_body(&function.body, out),
        py::Statement::Annotated(binding) => {
            if let Some(value) = &binding.value {
                calls_in_expr(value, out);
            }
        }
        py::Statement::Assign(binding) => calls_in_expr(&binding.value, out),
        py::Statement::Set { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        py::Statement::Expr(value) => calls_in_expr(value, out),
        py::Statement::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            calls_in_body(then, out);
            if let Some(otherwise) = otherwise {
                calls_in_body(otherwise, out);
            }
        }
        py::Statement::For { bound, body, .. } => {
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        py::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        py::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        py::Statement::Break
        | py::Statement::Comment(_)
        | py::Statement::Pass
        | py::Statement::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[py::Statement], out: &mut Vec<String>) {
    for statement in body {
        calls_in_statement(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &py::Expr, out: &mut Vec<String>) {
    match expr {
        py::Expr::Call { func, args } => {
            // Unqualified only: a defaulter is a module-level function
            // of the generated file, and this generator built every call
            // to one with `defaulter_name`. `a.b.S(…)` is a constructor
            // and never a defaulter.
            if func.segments().len() == 1 {
                let name = func
                    .segments()
                    .first()
                    .map(|segment| segment.as_str().to_string())
                    .unwrap_or_default();
                if name.starts_with("zd_def_") && !out.iter().any(|seen| seen == &name) {
                    out.push(name);
                }
            }
            for arg in args {
                match arg {
                    py::Arg::Positional(value) => calls_in_expr(value, out),
                    py::Arg::Keyword(keyword) => calls_in_expr(&keyword.value, out),
                }
            }
        }
        py::Expr::Tuple(values) | py::Expr::List(values) => {
            for value in values {
                calls_in_expr(value, out);
            }
        }
        py::Expr::Dict(entries) => {
            for (key, value) in entries {
                calls_in_expr(key, out);
                calls_in_expr(value, out);
            }
        }
        py::Expr::Attribute { value, .. } => calls_in_expr(value, out),
        py::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        py::Expr::Slice { start, end } => {
            for side in [start, end].into_iter().flatten() {
                calls_in_expr(side, out);
            }
        }
        py::Expr::Unary { value, .. } | py::Expr::Paren(value) => calls_in_expr(value, out),
        py::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        // An annotation holds no call: `Expr::Annotation` is a *type*
        // written where a value belongs, and a type carries no defaulter.
        py::Expr::Literal(_) | py::Expr::Path(_) | py::Expr::Annotation(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> py::Expr {
    py::Expr::Literal(Literal::Str(value.to_string()))
}

fn call(callee: &str, args: Vec<py::Expr>) -> py::Expr {
    py::Expr::Call {
        func: Path::single(callee),
        args: args.into_iter().map(py::Arg::Positional).collect(),
    }
}

fn set(place: py::Expr, value: py::Expr) -> py::Statement {
    py::Statement::Set { place, value }
}

/// Kept so the module's one dependency on the materialiser's error type
/// is stated rather than inferred.
const _: Option<MaterialiseError> = None;

#[cfg(test)]
mod tests {
    // `print_stderr` for the one skip below: a test that silently
    // checks nothing has to say so, and there is no other channel a
    // `cargo test` run shows.
    #![allow(
        clippy::expect_used,
        clippy::panic,
        clippy::print_stderr,
        clippy::unwrap_used
    )]
    use super::*;

    /// IDL in, one model out.
    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().is_empty(),
            "the source does not parse: {:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "the source does not lower: {:?}",
            lowered.diagnostics
        );
        let mut ast: Ast = lowered.ast;
        ast.resolve_all_by_name().expect("the names resolve");
        ast
    }

    fn root_of(ast: &Ast, type_name: &str) -> Id<TypeNode> {
        ast.types()
            .find(|(_, node)| node.name.as_str() == type_name)
            .map(|(id, _)| id)
            .expect("the named type")
    }

    fn rendered(decls: &[py::Statement]) -> String {
        let text = decls
            .iter()
            .map(zerodds_lang_render::python::statement)
            .collect::<Vec<_>>()
            .join("\n");
        parses(&text);
        text
    }

    /// The defaulter for one named type.
    fn defaulter(idl: &str, type_name: &str) -> String {
        let ast = model(idl);
        let root = root_of(&ast, type_name);
        rendered(&python_defaulters_over(&ast, &[root]).expect("Table 9 answers for the type"))
    }

    /// The defaulter for one named type and every defaulter it reaches.
    ///
    /// The only way to a collection's own row: a `sequence<long>` and a
    /// `long[3]` have no IDL name to look up, and the batch is where
    /// their functions are.
    fn closure(idl: &str, type_name: &str) -> String {
        let ast = model(idl);
        let root = root_of(&ast, type_name);
        let seed = python_defaulters_over(&ast, &[root]).expect("the root's own defaulter");
        rendered(&python_defaulters_called_by(&ast, &seed).expect("the closure"))
    }

    /// Hand the text to CPython and ask whether it is a program.
    ///
    /// The same check `tests/python_codec.rs` makes, and for the same
    /// reason: in Python an indent that is off by four is a different
    /// program, and no reader in this crate would see it.
    ///
    /// Skipped, loudly, where no interpreter is on the path — a test
    /// that silently checks nothing is worse than one that says it did
    /// not run.
    fn parses(text: &str) {
        let Ok(output) = std::process::Command::new("python3")
            .arg("-c")
            .arg("import sys; compile(sys.stdin.read(), '<t>', 'exec')")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write as _;
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait_with_output()
            })
        else {
            eprintln!("no python3 on the path: the defaulter's syntax was not checked");
            return;
        };
        assert!(
            output.status.success(),
            "the defaulter is not Python:\n{}\n--- source ---\n{text}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn a_structure_of_primitives_names_every_required_field() {
        // `kw_only=True` makes a positional argument a TypeError, and a
        // required field left out is one too — which is what
        // `adapter/python.rs` meant by giving it no default.
        let text = defaulter("@final struct S { long x; double y; boolean z; };", "S");
        assert!(
            text.contains("def zd_def_S() -> S:"),
            "the signature is not the Python one:\n{text}"
        );
        assert!(
            text.contains("return S(x=zd_def_p_long(), y=zd_def_p_double(), z=zd_def_p_boolean())"),
            "the fields are not named:\n{text}"
        );
    }

    #[test]
    fn a_nested_structure_reaches_its_members_defaulter_and_not_a_literal() {
        let text = defaulter(
            "@final struct Inner { long a; }; @final struct S { Inner i; long b; };",
            "S",
        );
        assert!(
            text.contains("return S(i=zd_def_Inner(), b=zd_def_p_long())"),
            "the nested default is not a call:\n{text}"
        );
    }

    #[test]
    fn a_nested_module_is_written_from_the_module_root() {
        // A generated codec sits at the module root, where `S` names
        // nothing: `adapter/python.rs` nests each IDL module in a class.
        let text = defaulter(
            "module a { module b { @final struct S { long x; }; }; };",
            "S",
        );
        assert!(
            text.contains("return a.b.S(x=zd_def_p_long())"),
            "the constructor is not qualified:\n{text}"
        );
    }

    #[test]
    fn an_optional_member_is_absent_from_the_constructor_call() {
        // 7.2.2.4.4.4.8: "Implementations shall not provide a default
        // value in such a case". The declared field is
        // `typing.Optional[T] = None` and the absence is the value.
        let text = defaulter("@final struct S { @optional long a; long b; };", "S");
        assert!(
            text.contains("return S(b=zd_def_p_long())"),
            "the optional member is not left out:\n{text}"
        );
        assert!(!text.contains("a="), "the optional member is set:\n{text}");
    }

    #[test]
    fn a_union_whose_discriminators_default_selects_a_case_sets_both() {
        // Table 9: the discriminator's own default, and the member it
        // selects. A `long` discriminator defaults to zero.
        let text = defaulter(
            "@final union U switch (long) { case 0: long one; case 1: string other; };",
            "U",
        );
        assert!(
            text.contains("return U(_d=0, one=zd_def_p_long())"),
            "the selected case is not set:\n{text}"
        );
    }

    #[test]
    fn a_union_whose_discriminators_default_selects_nothing_is_the_discriminator_alone() {
        // No label is zero and there is no `default:`, so Table 9's "if
        // this selects a branch" does not fire. A Python union is one
        // dataclass whose case fields default to None, so no case is
        // invented to fill the value out.
        let text = defaulter(
            "@final union U switch (long) { case 1: long one; case 2: long two; };",
            "U",
        );
        assert!(
            text.contains("return U(_d=0)"),
            "a case was invented:\n{text}"
        );
    }

    #[test]
    fn a_unions_default_case_is_what_an_unlisted_discriminator_selects() {
        let text = defaulter(
            "@final union U switch (long) { case 1: long one; default: string other; };",
            "U",
        );
        assert!(
            text.contains("return U(_d=0, other=zd_def_string())"),
            "the default case is not selected:\n{text}"
        );
    }

    #[test]
    fn an_enumeration_takes_its_first_literal_by_name() {
        // 7.3.1.2.1.10. By name and not as the number: a file that
        // spelled the value would keep working after `@value` moved it.
        let text = defaulter("enum Colour { RED, GREEN, BLUE };", "Colour");
        assert!(
            text.contains("return Colour.RED"),
            "the first literal is not the default:\n{text}"
        );
    }

    #[test]
    fn an_enumeration_with_a_default_literal_takes_that_one() {
        let text = defaulter(
            "enum Colour { RED, @default_literal GREEN, BLUE };",
            "Colour",
        );
        assert!(
            text.contains("return Colour.GREEN"),
            "@default_literal is not honoured:\n{text}"
        );
    }

    #[test]
    fn a_sequence_is_an_empty_list_at_every_bound() {
        // 7.2.2.4.4.4.7 asks for a zero-length sequence, and the bound
        // does not change what zero elements are.
        // The two names differ because the bound is part of the tag —
        // one hoisted routine per IDL type, not per element type. See
        // `crates/lang-rules/tests/a_bound_belongs_to_the_hoisted_routine.rs`.
        for (idl, name) in [
            (
                "@final struct S { sequence<long> many; };",
                "zd_def_seq_long",
            ),
            (
                "@final struct S { sequence<long, 4> many; };",
                "zd_def_seq_b4_long",
            ),
        ] {
            let text = closure(idl, "S");
            assert!(
                text.contains(&format!("def {name}() -> list[int]:")) && text.contains("return []"),
                "the sequence default is not an empty list:\n{text}"
            );
            // Not `None`: that is what an `@optional` member's absence
            // means here, and one value cannot mean both.
            assert!(!text.contains("None"), "the sequence is absent:\n{text}");
        }
    }

    #[test]
    fn a_fixed_size_array_is_that_many_elements() {
        // One call per element and never `(zd_def_T(),) * 3`, which
        // evaluates the element once and repeats the reference; a tuple,
        // because `adapter/python.rs` declares the member `tuple[int, int,
        // int]` and a list in its place is not `==` to one.
        let text = closure("@final struct S { long a[3]; };", "S");
        assert!(
            text.contains("return (zd_def_p_long(), zd_def_p_long(), zd_def_p_long())"),
            "the elements are not spelled out as a tuple:\n{text}"
        );
        assert!(!text.contains(" * 3"), "the element is aliased:\n{text}");
    }

    #[test]
    fn a_two_dimensional_array_is_one_tuple_per_dimension() {
        let text = closure("@final struct S { long g[2][3]; };", "S");
        // Two rows of three, which is what the walk indexes [i0][i1].
        assert!(
            text.contains(
                "((zd_def_p_long(), zd_def_p_long(), zd_def_p_long()), \
                 (zd_def_p_long(), zd_def_p_long(), zd_def_p_long()))"
            ),
            "the dimensions are not nested outermost first:\n{text}"
        );
    }

    #[test]
    fn a_fixed_is_the_one_decimal_spelling_of_zero_for_its_scale() {
        // `crates/wire-probe/src/canonical.rs` accepts exactly one
        // spelling of zero at a given scale, and "" is not a decimal.
        let text = defaulter("@final struct S { fixed<5,2> f; };", "S");
        assert!(
            text.contains("f=zd_def_fixed_5_2()"),
            "the member does not reach the defaulter:\n{text}"
        );
        let text = defaulter(
            "typedef fixed<5,2> Money; @final struct S { Money f; };",
            "Money",
        );
        assert!(
            text.contains("return \"0.00\""),
            "the fixed default is not the decimal:\n{text}"
        );
    }

    #[test]
    fn a_long_double_is_a_float128_and_never_a_float() {
        // A Python `float` is a binary64 with 53 significand bits and a
        // `long double` is a binary128 with 113.
        let text = defaulter("typedef long double LD; @final struct S { LD d; };", "LD");
        assert!(
            text.contains("return Float128.zero()"),
            "the long double default is not the runtime's zero:\n{text}"
        );
        assert!(
            !text.contains("0.0"),
            "a float reached a binary128:\n{text}"
        );
    }

    #[test]
    fn a_char_is_a_one_character_string_and_not_an_integer() {
        // Python has no character type; the octet is the C column's 0.
        let text = closure("@final struct S { char c; };", "S");
        assert!(
            text.contains("return \"\\x00\""),
            "the char default is not a str:\n{text}"
        );
    }

    #[test]
    fn a_bitset_is_the_dataclass_with_every_field_at_zero() {
        // `adapter/python.rs` declares a dataclass of one field per
        // bitfield; the packing into the anchor is the codec's.
        let text = defaulter(
            "bitset Packed { bitfield<3> low; bitfield<5> high; };",
            "Packed",
        );
        assert!(
            text.contains("return Packed(low=0, high=0)"),
            "the bitset default is not the dataclass:\n{text}"
        );
    }

    #[test]
    fn a_typedef_and_its_target_are_one_function_and_it_does_not_call_itself() {
        // `type_tag` follows an alias through, so both name
        // `zd_def_Point`; an alias arm answering with a call would be a
        // function whose body calls itself.
        let text = defaulter(
            "@final struct Point { long x; }; typedef Point Position;",
            "Position",
        );
        assert!(
            text.contains("return Point(x=zd_def_p_long())"),
            "the typedef does not resolve to its target's table row:\n{text}"
        );
        assert!(
            !text.contains("return zd_def_Point()"),
            "the defaulter calls itself:\n{text}"
        );
    }

    #[test]
    fn the_closure_reaches_every_defaulter_a_batch_calls() {
        // The set is read off the emitted calls, so a member's member's
        // defaulter arrives without a second walk over the model.
        let idl = "@final struct Leaf { long a; };
                   @final struct Middle { Leaf l; };
                   @final struct Top { Middle m; sequence<double> s; };";
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let mut ast: Ast = lowered.ast;
        ast.resolve_all_by_name().expect("the names resolve");
        let top = ast
            .types()
            .find(|(_, node)| node.name.as_str() == "Top")
            .map(|(id, _)| id)
            .expect("the root type");
        let seed = python_defaulters_over(&ast, &[top]).expect("the root's own defaulter");
        let all = python_defaulters_called_by(&ast, &seed).expect("the closure");
        let text = all
            .iter()
            .map(zerodds_lang_render::python::statement)
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "def zd_def_Middle()",
            "def zd_def_Leaf()",
            "def zd_def_p_long()",
            "def zd_def_seq_double()",
        ] {
            assert!(text.contains(expected), "{expected} is missing:\n{text}");
        }
        parses(&text);
    }
}
