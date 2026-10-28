// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value a Go member takes when the stream does not carry it.
//!
//! Table 9 of DDS-XTypes 1.3, as Go. [`crate::default_c`] is the same
//! table as C, [`crate::default_python`] as Python; this is deliberately
//! an eighth derivation and not a call into any of them, because the
//! columns are compared against each other and a shared helper would
//! make them agree about a mistake.
//!
//! ## The shape is Python's, with Go's spellings
//!
//! A defaulter **returns** Table 9's value and the caller assigns it —
//! Go passes by value and a `void`-shaped filler would need a pointer
//! nothing else in the generated code takes. The reader's own shape
//! depends on it: `emit/go.rs`'s decoder opens with `v := zd_def_T()`,
//! so a position the stream does not carry is already what Table 9 says
//! before an octet is read.
//!
//! What Go decides that Python did not have to:
//!
//! - **An aggregate is a composite literal**, `S{X: zd_def_p_long()}`,
//!   with every field keyed: a positional literal would silently
//!   transpose two same-typed members the day `all_members_of` reorders.
//! - **An `@optional` member is left out of the literal.** The field is
//!   a `*T` and its zero value is `nil`, which is the absence
//!   §7.2.2.4.4.4.8 asks for.
//! - **A union's unselected branches keep their Go zero values.** Only
//!   `D` and the selected member are written; the printer beside this
//!   prints only the selected branch, so the zeros are storage and
//!   never values.
//! - **A `long double` is the decimal text `"0.0"`** — the exact form
//!   `runtime/go`'s `ReadF128` answers for the zero pattern — and a
//!   `fixed<P,S>` is the one spelling of zero for its scale.
//! - **A fixed array is that many elements, spelled out**, one
//!   defaulter call per element, with the type written on the outermost
//!   literal and elided inside it as `gofmt -s` writes it.

use crate::adapter::go::{field_name, go_type_name, go_value_type, module_prefix};
use crate::default_common::{discriminator_default_value, resolve_alias, selected_case};
use crate::emit::go::type_tag;
use crate::encode::EncodeError;
use crate::fixed_literal::{fixed_text, zero_decimal};
use zerodds_ast::{
    Ast, ConstExpr, Literal as AstLiteral, MemberNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::go;

/// How deep a chain of typedefs is followed before the model is taken
/// to be malformed.
const MAX_ALIAS_DEPTH: usize = 64;

/// The defaulter function name for a type, keyed the same way the
/// decoder is: by the IDL kind for a primitive and by the codec tag for
/// everything else, so `double` and `long double` — `float64` and
/// `string` here — cannot share one.
#[must_use]
pub fn defaulter_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let name = match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => {
            format!("zd_def_p_{}", kind.as_str().replace(' ', "_"))
        }
        _ => format!("zd_def_{}", type_tag(ast, id, 0)),
    };
    // In the exported spelling, here and not at each call site.
    // `go_defaulters_called_by` matches the names it finds in the
    // emitted decoders against the names this function builds, and the
    // two forms did not match: nothing matched, no defaulter was
    // emitted at all, and `go build` answered `undefined: Zd_def_Max`.
    // One function that answers the exported name is what keeps the
    // index and the calls the same set.
    crate::emit::go::exported_name(&name)
}

/// The prefix every defaulter's name begins with, in the spelling Go
/// exports.
///
/// Derived rather than written down, so the prefix and the names cannot
/// disagree about their first letter.
fn defaulter_prefix() -> String {
    crate::emit::go::exported_name("zd_def_")
}

/// The defaulter that goes with a decoder, by name: `zd_dec_p_long`
/// against `zd_def_p_long`, both built from the same key.
#[must_use]
pub fn defaulter_from_decoder(decoder: &str) -> String {
    // The decoder arrives in the spelling `decoder_name` published it
    // under — `Zd_dec_T` and not `zd_dec_T` — so the prefix to strip is
    // built the same way rather than written out a second time.
    let prefix = crate::emit::go::exported_name("zd_dec_");
    let name = match decoder.strip_prefix(&prefix) {
        Some(rest) => format!("zd_def_{rest}"),
        None => format!("zd_def_{decoder}"),
    };
    crate::emit::go::exported_name(&name)
}

/// One member's default: its `@default`, or its type's, assigned into
/// `place`. §7.2.2.4.4.4.9 puts the explicit one first.
///
/// An `@optional` member's place is a `*T`, so the value goes through
/// `zdwire.Ptr` — typed, because `Ptr` infers its parameter and an
/// untyped constant would infer `int` where the member is an `int32`.
///
/// # Errors
/// [`EncodeError::NoWriter`] where a `@default` names a value this
/// column cannot spell as a Go expression. Refused rather than silently
/// replaced by Table 9's zero: the two are different samples.
pub fn member_default(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
    place: go::Expr,
) -> Result<Vec<go::Stmt>, EncodeError> {
    let mut value = member_value(ast, node, type_)?;
    if node.facets.is_optional() {
        let member_type = match go_value_type(ast, type_) {
            Ok(member_type) => member_type,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: node.name.clone(),
                    kind: "optional member whose type has no Go form",
                });
            }
        };
        value = go::Expr::call(
            go::Expr::selector(go::Expr::name("zdwire"), "Ptr"),
            vec![go::Expr::Convert {
                type_: Box::new(member_type),
                value: Box::new(value),
            }],
        );
    }
    Ok(vec![go::Stmt::Assign { place, value }])
}

/// The defaulters `decls` call, and exactly those — read off the
/// emitted calls rather than derived a second time from the model, for
/// the reason [`crate::default_c`] gives at length: a derivation beside
/// the decoder's is a second reading of §7.2.2.7, and the two drift.
///
/// # Errors
/// [`EncodeError`] for a construct with no Go default.
pub fn go_defaulters_called_by(
    ast: &Ast,
    decls: &[go::Decl],
) -> Result<Vec<go::Decl>, EncodeError> {
    let index: Vec<(String, Id<TypeNode>)> = ast
        .types()
        .map(|(id, _)| (defaulter_name(ast, id), id))
        .collect();
    let mut called: Vec<String> = Vec::new();
    for decl in decls {
        calls_in_decl(decl, &mut called);
    }
    let mut wanted: Vec<Id<TypeNode>> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    // Bounded by the number of named defaulters, because every round
    // but the last adds at least one.
    for _ in 0..=index.len() {
        let mut grew = false;
        for name in &called {
            if have.contains(name) {
                continue;
            }
            have.push(name.clone());
            // A name with no type behind it cannot happen for a call
            // this generator emitted; if it ever does, the omission is
            // an undefined-name error from `go build` naming the
            // function, which is louder than anything this could
            // return.
            if let Some(id) = index.iter().find(|(n, _)| n == name).map(|(_, id)| *id) {
                wanted.push(id);
                grew = true;
            }
        }
        let produced = go_defaulters_over(ast, &wanted)?;
        if !grew {
            return Ok(produced);
        }
        for decl in &produced {
            calls_in_decl(decl, &mut called);
        }
    }
    go_defaulters_over(ast, &wanted)
}

/// Build the defaulter for each of `types`, once per name.
///
/// # Errors
/// [`EncodeError`] for a construct with no Go default.
pub fn go_defaulters_over(ast: &Ast, types: &[Id<TypeNode>]) -> Result<Vec<go::Decl>, EncodeError> {
    let mut out = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for id in types.iter().copied() {
        let name = defaulter_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let value = match value_of(ast, id, &name)? {
            Some(value) => value,
            None => continue,
        };
        let returns = match go_value_type(ast, resolve_alias(ast, id)) {
            Ok(returns) => returns,
            Err(_) => {
                return Err(EncodeError::NoWriter {
                    member: name,
                    kind: "type with no Go form to return",
                });
            }
        };
        emitted.push(name.clone());
        out.push(go::Decl::Func(go::FuncDecl {
            doc: Doc::line(
                "Table 9 of DDS-XTypes 1.3: what this type is where the stream did not \
                 carry it.",
            ),
            // `defaulter_name` already answers the exported spelling.
            name: Ident::new(&name),
            params: Vec::new(),
            results: vec![returns],
            body: vec![go::Stmt::Return(Some(value))],
        }));
    }
    Ok(out)
}

/// The value one defaulter answers with, or `None` where the type has
/// no Go data form at all.
#[allow(clippy::too_many_lines)]
fn value_of(ast: &Ast, id: Id<TypeNode>, own_name: &str) -> Result<Option<go::Expr>, EncodeError> {
    // Before the table and not as one of its rows: a typedef and its
    // target share a defaulter name, so an arm answering with a call
    // would name this very function.
    let id = resolve_alias(ast, id);
    let node = match ast.type_(id) {
        Ok(node) => node,
        // Refused by name, not answered as `Ok(None)`. The caller reads
        // `None` as "nothing to emit for this one" and skips it, so a
        // type whose handle no longer resolves quietly loses its
        // defaulter and the first sign of it is a member left at
        // whatever the language zero-initialises to.
        Err(_) => {
            return Err(EncodeError::NoWriter {
                member: own_name.to_string(),
                kind: "a type the model does not hold",
            });
        }
    };
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: own_name.to_string(),
        kind,
    };
    let value = match &node.kind {
        TypeKind::Primitive(kind) => primitive_default(*kind),

        // `""`, for both widths: the member holds UTF-8 either way.
        TypeKind::String { .. } => text(""),

        // The one decimal spelling of zero for the scale, which is what
        // the canonical form accepts.
        TypeKind::Fixed { scale, .. } => text(&zero_decimal(*scale)),

        // Only an alias `resolve_alias` could not follow reaches here.
        TypeKind::Alias { .. } => {
            return Err(refuse("typedef with an unresolved target"));
        }

        // "The first literal in the enumeration", or the one
        // `@default_literal` names — §7.3.1.2.1.10. By name, in the
        // materialiser's own spelling, so the file keeps working after
        // `@value` moves the number.
        TypeKind::Enumeration { .. } => {
            let enumerator = match crate::emit::default_enumerator_name(ast, id) {
                Some(enumerator) => enumerator,
                None => {
                    return Err(refuse(
                        "enumeration with no enumerator to take as its default",
                    ));
                }
            };
            enumerator_expr(ast, id, &enumerator)
        }

        // No bit set. `Flags(0)` and not a bare `0`: the function's
        // result type is the defined type, and a defined type's zero is
        // written with its name on it.
        TypeKind::Bitmask { .. } => go::Expr::Convert {
            type_: Box::new(go::Type::named(&go_type_name(ast, id))),
            value: Box::new(go::Expr::int(0)),
        },

        // A bitset's fields are separate integers in `adapter/go.rs`;
        // the packing into the anchor is the codec's and does not
        // appear here.
        TypeKind::Bitset { .. } => {
            let mut elems = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let field_node = match ast.bitfield(*field) {
                    Ok(field_node) => field_node,
                    Err(_) => continue,
                };
                // An anonymous bitfield is padding: it moves the next
                // field along and holds no value, so the declaration
                // has no field for it either.
                if let Some(name) = field_node.name.as_ref() {
                    elems.push(go::CompositeElem {
                        key: Some(Ident::new(&field_name(name))),
                        value: go::Expr::int(0),
                    });
                }
            }
            composite(ast, id, elems)
        }

        // "A zero-length sequence of the same element type", and the
        // same for a map's entries: the typed empty literal, not `nil`
        // — the two are the same zero elements to the codec, and the
        // typed form is what a `return` under this result type reads
        // as.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => {
            let type_ = match go_value_type(ast, id) {
                Ok(type_) => type_,
                Err(_) => return Err(refuse("collection with no Go form")),
            };
            go::Expr::Composite {
                type_: Some(Box::new(type_)),
                elems: Vec::new(),
            }
        }

        // "An array of the same dimensions and same element type whose
        // elements take the default value for their corresponding
        // type." One call per element, the type on the outermost
        // literal, elided inside.
        TypeKind::Array { element, dims } => {
            let element = match element.resolved() {
                Some(element) => element,
                None => return Err(refuse("array with an unresolved element")),
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let length = match ast.eval_integral(*dim) {
                    Ok(length) => length,
                    Err(_) => return Err(refuse("array whose length does not evaluate")),
                };
                let length = match usize::try_from(length) {
                    Ok(length) => length,
                    Err(_) => return Err(refuse("array of a length this column cannot spell")),
                };
                lengths.push(length);
            }
            let type_ = match go_value_type(ast, id) {
                Ok(type_) => type_,
                Err(_) => return Err(refuse("array with no Go form")),
            };
            let mut out = go::Expr::call(go::Expr::name(&defaulter_name(ast, element)), Vec::new());
            // Innermost dimension first, so the outer one ends up
            // outermost: `long a[2][3]` is two rows of three.
            let mut level = lengths.len();
            for length in lengths.into_iter().rev() {
                level -= 1;
                let elems = core::iter::repeat_with(|| go::CompositeElem {
                    key: None,
                    value: out.clone(),
                })
                .take(length)
                .collect();
                out = go::Expr::Composite {
                    type_: if level == 0 {
                        Some(Box::new(type_.clone()))
                    } else {
                        None
                    },
                    elems,
                };
            }
            out
        }

        // "A structure without any of the optional members and with
        // other members set to their default values."
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut elems = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                elems.push(go::CompositeElem {
                    key: Some(Ident::new(&field_name("parent"))),
                    value: go::Expr::call(go::Expr::name(&defaulter_name(ast, base)), Vec::new()),
                });
            }
            for member_id in ast.members_of(id) {
                let member_node = match ast.member(*member_id) {
                    Ok(member_node) => member_node,
                    Err(_) => continue,
                };
                // Left out rather than written as `nil`: the field's
                // zero value is already the absence.
                if member_node.facets.is_optional() {
                    continue;
                }
                let member_type = match member_node.type_.resolved() {
                    Some(member_type) => member_type,
                    None => {
                        return Err(EncodeError::NoWriter {
                            member: member_node.name.clone(),
                            kind: "member with an unresolved type",
                        });
                    }
                };
                elems.push(go::CompositeElem {
                    key: Some(Ident::new(&field_name(&member_node.name))),
                    value: member_value(ast, member_node, member_type)?,
                });
            }
            composite(ast, id, elems)
        }

        // "A union with the discriminator set to the default value for
        // the discriminator type. If this selects a branch, then the
        // selected member is also set to the default value for the
        // member type." Where it selects none, `D` alone is the whole
        // value; the other fields are Go zero values the printer never
        // reads.
        TypeKind::Union { discriminator, .. } => {
            let discriminator = match discriminator.resolved() {
                Some(discriminator) => discriminator,
                None => return Err(refuse("union with an unresolved discriminator")),
            };
            let selector = discriminator_default_value(ast, discriminator);
            let mut elems = vec![go::CompositeElem {
                key: Some(Ident::new("D")),
                value: discriminant_literal(ast, discriminator, selector.unwrap_or(0)),
            }];
            if let Some((member_type, field)) = selected_by_default(ast, id, selector)? {
                elems.push(go::CompositeElem {
                    key: Some(Ident::new(&field)),
                    value: go::Expr::call(
                        go::Expr::name(&defaulter_name(ast, member_type)),
                        Vec::new(),
                    ),
                });
            }
            composite(ast, id, elems)
        }

        TypeKind::Native | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Ok(None);
        }
        TypeKind::ValueBox { .. } => {
            return Err(refuse(
                "boxed value, whose presence flag has no settled wire form",
            ));
        }
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
fn member_value(
    ast: &Ast,
    node: &MemberNode,
    type_: Id<TypeNode>,
) -> Result<go::Expr, EncodeError> {
    match node.facets.xtypes.as_ref().and_then(|x| x.default_value) {
        Some(expr) => explicit(ast, expr, type_, &node.name),
        None => Ok(go::Expr::call(
            go::Expr::name(&defaulter_name(ast, type_)),
            Vec::new(),
        )),
    }
}

/// Table 9's primitive rows, as the Go holder each takes. The integers
/// and both character widths are integer zeros — the member holds the
/// code unit — and the two floats are `0.0`, spelled as a float so the
/// literal reads as the type it converts to. A binary128 zero is the
/// decimal `"0.0"`, which is what `ReadF128` answers for the zero
/// pattern.
fn primitive_default(kind: PrimitiveKind) -> go::Expr {
    match kind {
        PrimitiveKind::Boolean => go::Expr::Literal(Literal::Bool(false)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
            go::Expr::Literal(Literal::Float("0.0".to_string()))
        }
        PrimitiveKind::Float128 => text("0.0"),
        PrimitiveKind::Char
        | PrimitiveKind::WChar
        | PrimitiveKind::Octet
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => go::Expr::int(0),
    }
}

/// `Name{key: value, ...}` — a keyed composite literal of the declared
/// type.
fn composite(ast: &Ast, id: Id<TypeNode>, elems: Vec<go::CompositeElem>) -> go::Expr {
    go::Expr::Composite {
        type_: Some(Box::new(go::Type::named(&go_type_name(ast, id)))),
        elems,
    }
}

/// An enumerator constant, in the materialiser's spelling: the
/// flattened module prefix and the exported name, the same two
/// functions `adapter/go.rs` builds the declaration from.
fn enumerator_expr(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> go::Expr {
    go::Expr::name(&format!(
        "{}{}",
        module_prefix(ast, id),
        field_name(enumerator)
    ))
}

/// The case a defaulted discriminator selects, if any.
fn selected_by_default(
    ast: &Ast,
    id: Id<TypeNode>,
    selector: Option<i128>,
) -> Result<Option<(Id<TypeNode>, String)>, EncodeError> {
    let selector = match selector {
        Some(selector) => selector,
        None => return Ok(None),
    };
    Ok(selected_case(ast, id, selector)?
        .map(|(member, member_type)| (member_type, field_name(&member.name))))
}

/// A discriminator value in the Go form its type gives it: the
/// enumerator for an enumeration, `true`/`false` for a `boolean`, and
/// the number for everything else — a `char` and a `wchar`
/// discriminator hold their code unit as an integer here, so the
/// number *is* the form.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> go::Expr {
    let resolved = resolve_alias(ast, discriminator);
    if matches!(
        ast.type_(resolved).map(|node| node.kind.clone()),
        Ok(TypeKind::Enumeration { .. })
    ) {
        if let Some(enumerator) = crate::emit::default_enumerator_name(ast, resolved) {
            return enumerator_expr(ast, resolved, &enumerator);
        }
    }
    match resolved_primitive(ast, resolved) {
        Some(PrimitiveKind::Boolean) => go::Expr::Literal(Literal::Bool(value != 0)),
        _ => go::Expr::int(value),
    }
}

/// The primitive `id` is once typedefs are seen through, or `None`.
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

/// An explicit `@default` as a Go expression for `type_`.
#[allow(clippy::too_many_lines)]
fn explicit(
    ast: &Ast,
    expr: Id<zerodds_ast::ConstExprNode>,
    type_: Id<TypeNode>,
    member_name: &str,
) -> Result<go::Expr, EncodeError> {
    let refuse = |kind: &'static str| EncodeError::NoWriter {
        member: member_name.to_string(),
        kind,
    };
    let node = match ast.expr(expr) {
        Ok(node) => node,
        Err(_) => return Err(refuse("a `@default` this frontend cannot follow")),
    };
    let resolved = resolve_alias(ast, type_);
    let kind = ast.type_(resolved).map(|node| node.kind.clone());
    let is_float128 = matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Float128)));
    match &node.expr {
        ConstExpr::Literal(AstLiteral::Boolean(flag)) => {
            Ok(go::Expr::Literal(Literal::Bool(*flag)))
        }
        // The code unit as the number the member holds: `uint8` for a
        // `char`, `uint16` for a `wchar`, and a surrogate is a legal
        // `wchar` value here exactly because it is a number.
        ConstExpr::Literal(AstLiteral::Char(ch)) => Ok(go::Expr::int(i128::from(u32::from(*ch)))),
        ConstExpr::Literal(AstLiteral::WChar(code)) => Ok(go::Expr::int(i128::from(*code))),
        ConstExpr::Literal(AstLiteral::String(value)) => Ok(text(value)),
        ConstExpr::Literal(AstLiteral::WString(codes)) => {
            let mut out = String::new();
            for code in codes {
                let ch = match char::from_u32(*code) {
                    Some(ch) => ch,
                    None => {
                        return Err(refuse("a `@default` wide string that is not code points"));
                    }
                };
                out.push(ch);
            }
            Ok(text(&out))
        }
        ConstExpr::Literal(AstLiteral::Float { mantissa, exponent }) => {
            let exponent = match exponent.as_ref() {
                Some(exponent) => format!("e{exponent}"),
                None => String::new(),
            };
            let written = format!("{mantissa}{exponent}");
            if is_float128 {
                // The member holds decimal text and the runtime parses
                // it exactly; a Go float literal would round the
                // source's digits to binary64 sixty bits early.
                return Ok(text(&written));
            }
            Ok(go::Expr::Literal(Literal::Float(written)))
        }
        // A `fixed<P,S>` member is a string here.
        ConstExpr::Literal(AstLiteral::Fixed { digits, scale }) => {
            Ok(text(&fixed_text(digits, *scale)))
        }
        ConstExpr::Literal(AstLiteral::Enumerator(reference)) => {
            let target = match reference.resolved() {
                Some(target) => target,
                None => return Err(refuse("a `@default` enumerator that is unresolved")),
            };
            let enumerator = match ast.enumerator(target) {
                Ok(enumerator) => enumerator,
                Err(_) => {
                    return Err(refuse(
                        "a `@default` enumerator this frontend cannot follow",
                    ));
                }
            };
            Ok(enumerator_expr(ast, resolved, &enumerator.name))
        }
        _ => {
            // An integer literal, a named constant or arithmetic over
            // them, folded the same way a bound or a case label is.
            let value = match ast.eval_integral(expr) {
                Ok(value) => value,
                Err(_) => return Err(refuse("a `@default` this frontend cannot evaluate")),
            };
            Ok(match kind {
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    go::Expr::Literal(Literal::Bool(value != 0))
                }
                // The member is a `uint8` or a `uint16` and the value
                // is its code unit; one outside the width is refused
                // rather than folded, because a `@default` the designer
                // wrote and a member that got a zero instead are
                // different samples.
                Ok(TypeKind::Primitive(PrimitiveKind::Char)) => {
                    if !(0..=0xFF).contains(&value) {
                        return Err(refuse("a `@default` character outside one octet"));
                    }
                    go::Expr::int(value)
                }
                Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => {
                    if !(0..=0xFFFF).contains(&value) {
                        return Err(refuse(
                            "a `@default` character outside the range of a UTF-16 code unit",
                        ));
                    }
                    go::Expr::int(value)
                }
                // A binary128 integer default is its decimal text.
                Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => text(&value.to_string()),
                _ => go::Expr::int(value),
            })
        }
    }
}

// -- reading the calls back out ----------------------------------------

/// The `zd_def_*` functions one declaration calls, appended to `out`.
fn calls_in_decl(decl: &go::Decl, out: &mut Vec<String>) {
    match decl {
        go::Decl::Func(func) => calls_in_body(&func.body, out),
        go::Decl::Const(constant) => calls_in_expr(&constant.value, out),
        // A type declaration and an interface hold types and no calls,
        // a comment holds prose, and `@verbatim` text is the user's and
        // not this generator's to read.
        go::Decl::Type(_)
        | go::Decl::Interface(_)
        | go::Decl::Comment(_)
        | go::Decl::Verbatim(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_body(body: &[go::Stmt], out: &mut Vec<String>) {
    for statement in body {
        calls_in_stmt(statement, out);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn calls_in_stmt(statement: &go::Stmt, out: &mut Vec<String>) {
    match statement {
        go::Stmt::Var { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        go::Stmt::Assign { place, value } => {
            calls_in_expr(place, out);
            calls_in_expr(value, out);
        }
        go::Stmt::Expr(value) => calls_in_expr(value, out),
        go::Stmt::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        go::Stmt::If {
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
        go::Stmt::For {
            init, bound, body, ..
        } => {
            calls_in_expr(init, out);
            calls_in_expr(bound, out);
            calls_in_body(body, out);
        }
        go::Stmt::While { condition, body } => {
            calls_in_expr(condition, out);
            calls_in_body(body, out);
        }
        go::Stmt::Break | go::Stmt::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn calls_in_expr(expr: &go::Expr, out: &mut Vec<String>) {
    match expr {
        go::Expr::Call { func, args } => {
            // Unqualified only: a defaulter is a package-level function
            // of the generated file, and this generator built every
            // call to one with `defaulter_name`. `zdwire.Ptr(..)` is
            // the runtime's and never a defaulter.
            if let go::Expr::Name(name) = func.as_ref() {
                let name = name.as_str();
                // The prefix through `exported_name`, not spelled out:
                // a defaulter's name carries Go's visibility in its
                // first letter, and a literal `"zd_def_"` here stopped
                // matching the moment that letter was raised — with the
                // consequence that no defaulter was emitted at all and
                // `go build` answered `undefined: Zd_def_Max`.
                if name.starts_with(&defaulter_prefix()) && !out.iter().any(|seen| seen == name) {
                    out.push(name.to_string());
                }
            }
            calls_in_expr(func, out);
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        go::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        go::Expr::Convert { value, .. }
        | go::Expr::Paren(value)
        | go::Expr::Unary { value, .. } => calls_in_expr(value, out),
        go::Expr::Selector { value, .. } => calls_in_expr(value, out),
        go::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        go::Expr::SliceTo { value, high } => {
            calls_in_expr(value, out);
            calls_in_expr(high, out);
        }
        go::Expr::Composite { elems, .. } => {
            for elem in elems {
                calls_in_expr(&elem.value, out);
            }
        }
        go::Expr::Make { length, .. } => calls_in_expr(length, out),
        go::Expr::Literal(_) | go::Expr::Name(_) => {}
    }
}

// -- the small helpers --------------------------------------------------

fn text(value: &str) -> go::Expr {
    go::Expr::Literal(Literal::Str(value.to_string()))
}
