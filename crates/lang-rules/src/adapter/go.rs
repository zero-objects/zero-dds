// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Go CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::c`], which does the same for C.
//! Reading the two side by side is the point: both flatten a module onto
//! the name, and everything else they do differently is Go's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The four things Go decides that C does not
//!
//! 1. **Declaration order does not matter.** "The scope of an identifier
//!    denoting a constant, type, variable, or function ... declared at
//!    top level is the package block" (Go spec, "Declarations and
//!    scope"), so a type may be named before it is declared. The C
//!    column has to walk the type graph post-order for exactly that
//!    reason; this one walks the model in declaration order, so the
//!    generated file tracks the source.
//! 2. **A collection needs no declared type.** `[]T` and `[N]T` are
//!    written where the member is, so `sequence<long>` produces no
//!    declaration at all. Only a `map<K,V>` does, because its entry is a
//!    struct and an anonymous one repeated per use would be a shape a
//!    reader cannot name.
//! 3. **Visibility is the case of one letter.** A name is exported if
//!    and only if it begins with an upper-case letter (Go spec,
//!    "Exported identifiers"), so [`go::exported`] moves the first
//!    character of every declared name and every field name. A types
//!    column whose types no other package can name has no consumer.
//! 4. **A struct tag carries what the rename lost.** Go's own run-time
//!    readable annotation, `reflect.StructTag`. The C column needs a
//!    macro beside the declaration for this; here it is part of the
//!    field.
//!
//! ## The memory shapes
//!
//! | IDL | Go |
//! |---|---|
//! | `string` | `string` |
//! | `wstring` | `string`, holding UTF-8 |
//! | `fixed<P,S>` | `string`, holding the decimal |
//! | `long double` | `string`, holding the decimal |
//! | `sequence<T>` | `[]T` |
//! | `T[N][M]` | `[N][M]T`, outer dimension outermost |
//! | `map<K,V>` | `[]ZdMapEntry_K_V`, **not** a Go `map` |
//! | `union` | `struct { D <disc>; ... }`, one field per branch |
//! | `@optional T m` | `*T`, `nil` when absent |
//!
//! ### `map<K,V>` is a slice of entries and never a Go `map`
//!
//! Go randomises map iteration order on purpose — "The iteration order
//! over maps is not specified and is not guaranteed to be the same from
//! one iteration to the next" (Go spec, "For statements with range
//! clause") — and the runtime perturbs the start of every range. A
//! `map<K,V>` is written to the wire as a sequence of key/value pairs
//! (DDS-XTypes 1.3 §7.4.1.1.4), so a column holding it in a Go `map`
//! would produce a different octet stream on every run of one program
//! over one value. That is not a byte-order question a peer can recover
//! from: the KeyHash of a keyed type would differ per run.
//!
//! ### `long double` is a `string` and never a `float64`
//!
//! Go has no binary128 and no extended type at all. `float64` has 53
//! bits of significand where an IDL `long double` has 113, so a member
//! declared `float64` would lose sixty bits at the assignment, before
//! any codec runs. The member holds the decimal text instead — exactly
//! as `fixed<P,S>` does and as the Python column does for the same
//! reason.
//!
//! ### Where the IDL spelling of a member is kept
//!
//! [`go::exported`] moves the first letter of every member and
//! [`go::escape_name`] moves a name that collides with a Go keyword or
//! predeclared identifier. Neither inverts: `Value` could have come from
//! `value` or from `Value`, and `type_` from `type` or from `type_`.
//! DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's ID under
//! `@autoid(HASH)` and under `@hashid` from the first four octets of the
//! MD5 of its **IDL** name, so a file that only knows the Go spelling
//! cannot compute the ID for the member it is writing.
//!
//! Every field therefore carries `` `idl:"…"` ``, and every declaration
//! whose own name was moved carries the IDL spelling in the comment
//! above it.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::go;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Go";

/// The package a generated file declares.
///
/// One name for every generated file, because a Go package is a
/// directory and the whole model lands in one file. `zdtypes` and not
/// `types`: `types` is the name of a standard-library package
/// (`go/types`), and a generated package sharing it makes every import
/// of both need an alias.
pub const PACKAGE: &str = "zdtypes";

/// The prefix a name this column invented carries.
///
/// Only a map entry needs one — every other Go shape is written where
/// the member is. Exported, because the entry type appears in the
/// signature of a field a consumer reads.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// Build the Go declarations a rule application produced.
///
/// The order is the model's own declaration order. Go resolves a name
/// declared anywhere in the package block, so unlike the C column there
/// is no dependency walk and the output tracks the source.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Go shape, or two declarations reach one Go
/// spelling — never a silently dropped declaration.
pub fn go_items(ast: &Ast, applied: &Applied) -> Result<Vec<go::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::GO),
            &go_type_name(ast, id),
            start,
            &mut out,
        );
    }

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "GoConst").is_none() {
            continue;
        }
        // `eval_const` and not `eval_integral`: `const string NAME` is a
        // constant that evaluates perfectly well and is not a number,
        // and the integral question is simply the wrong one for four of
        // the eight forms IDL 4.2 admits.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = node.type_.resolved();
        let type_ = match declared {
            Some(target) => go_value_type(ast, target)?,
            None => go::Type::Primitive(go::Primitive::Int32),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(go::Decl::Const(go::ConstDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            type_: Some(type_.clone()),
            value: go_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "GoError").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut pairs = Vec::new();
        for member in &node.members {
            if let Ok(member_node) = ast.member(*member) {
                pairs.push((field_name(&member_node.name), member_node.name.clone()));
            }
        }
        no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
        let mut fields = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            fields.push(go::Field {
                doc: Doc::none(),
                name: Ident::new(&field_name(&member_node.name)),
                type_: go_member_type(ast, target, &member_node.name)?,
                tag: Some(go::Tag::one("idl", &member_node.name)),
            });
        }
        out.push(go::Decl::Type(go::TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line("An exception, as the structure its members make."),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            alias: false,
            type_: go::Type::Struct(fields),
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "GoInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut methods = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. Go would carry the
        // inherited operations through an embedded interface, and
        // `go::InterfaceDecl` has no place for one: the method set here
        // is the whole method set, so OMG IDL 4.2 §7.4.3's inherited
        // operations and attributes are written into it.
        // `interface::all_operations_flattened` gives both — the
        // declared operations and, per §7.5.1.1.3, the get/set pair
        // each attribute implies — bases before self, over
        // `adapter::interface::Service` rather than the bare
        // `Ast::all_operations_of`, whose own walk never picked up an
        // attribute at all.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            // An attribute's own name, not `get_`/`set_` plus it: Go's
            // idiom the coordinator named is `Memory()`/`SetMemory()`,
            // not `GetMemory()`/`SetMemory()` — the getter carries no
            // verb at all, matching a plain field access.
            let (exported, idl_spelling) = match op.origin {
                interface::Origin::AttributeGetter => {
                    let attr = op.attribute_name();
                    (go::exported(&go::escape_name(attr)), op.name.clone())
                }
                interface::Origin::AttributeSetter => {
                    let attr = op.attribute_name();
                    (
                        format!("Set{}", go::exported(&go::escape_name(attr))),
                        op.name.clone(),
                    )
                }
                interface::Origin::Declared => (field_name(&op.name), op.name.clone()),
            };
            signatures.push((exported.clone(), idl_spelling));
            let mut params = Vec::new();
            for param in &op.params {
                params.push(go::Param {
                    // A parameter name is local to the signature and is
                    // never reached from outside, so it is escaped and
                    // not exported: capitalising it would rename the
                    // argument for no reader.
                    name: Some(Ident::new(&go::escape_name(&param.name))),
                    type_: go_member_type(ast, param.type_, &param.name)?,
                });
            }
            let mut results = Vec::new();
            if let Some(returns) = op.returns {
                results.push(go_member_type(ast, returns, &op.name)?);
            }
            // Every method returns an `error` besides its own result —
            // Go's own idiom for "this call can fail", DDS-RPC 1.0's
            // `raises` (and every remote call's own possible failure)
            // included. `raises_line()` says which named exceptions the
            // IDL declared, in the doc, since a bare `error` carries no
            // type information a caller could switch on here.
            results.push(go::Type::named("error"));
            let doc = op
                .raises_line()
                .map_or_else(Doc::none, |line| Doc::line(&line));
            methods.push(go::Method {
                doc,
                name: Ident::new(&exported),
                params,
                results,
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(go::Decl::Interface(go::InterfaceDecl {
            doc: with_idl_note(
                Doc::line(
                    "An interface, as the method set its operations and attributes make \
                     — an attribute a `Name()`/`SetName()` pair, `readonly` only the \
                     first. A derived interface's bases are flattened in rather than \
                     embedded, so this is the whole method set; the transport is the RPC \
                     arc's business, and the shape is the IDL's. Every method returns an \
                     `error` besides its own result; `raises`/`getraises`/`setraises` \
                     name what it can carry, in the method's own doc.",
                ),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            methods,
        }));
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::GO, out))
}

/// `@verbatim` in the Go column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a struct's body; a defined scalar
/// type, an alias, a constant and an interface take them adjacent.
impl verbatim::Carrier for go::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Type(node) = self else {
            return false;
        };
        if node.alias || node.name.as_str() != name || !matches!(node.type_, go::Type::Struct(_)) {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// The Go declarations, in a file that names its package.
///
/// A caller that only wants the declarations uses [`go_items`]; this is
/// what a generator writes to disk, and it exists here so that the
/// package name is decided in one place.
///
/// # Errors
/// As [`go_items`].
pub fn go_file(ast: &Ast, applied: &Applied) -> Result<go::File, MaterialiseError> {
    Ok(go::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "The IDL spelling of every renamed declaration is in the comment",
            "above it; the IDL spelling of every member is in its `idl` struct",
            "tag, because a member's ID under @autoid(HASH) is the MD5 of the",
            "IDL name (DDS-XTypes 1.3 7.3.1.2.1.1) and the Go spelling does not",
            "invert.",
        ]),
        package: Ident::new(PACKAGE),
        imports: Vec::new(),
        decls: go_items(ast, applied)?,
    })
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<go::Decl>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence and an array are
        // all written where the member is: Go spells each of them
        // inline, so none of them is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // The one collection that does need a name. An anonymous
        // `struct { Key K; Value V }` repeated at every use is the same
        // Go type — Go's struct identity is structural — but it is a
        // shape a reader cannot name and a codec cannot take the address
        // of a field of without restating it.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: go_type_name(ast, id),
                });
            };
            let name = map_entry_name_at(ast, id, 0)?;
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::line(
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them.",
                ),
                name: Ident::new(&name),
                alias: false,
                type_: go::Type::Struct(vec![
                    go::Field {
                        doc: Doc::none(),
                        name: Ident::new("Key"),
                        type_: go_member_type(ast, key, &name)?,
                        tag: None,
                    },
                    go::Field {
                        doc: Doc::none(),
                        name: Ident::new("Value"),
                        type_: go_member_type(ast, value, &name)?,
                        tag: None,
                    },
                ]),
            }));
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "GoAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // `type A = B` and not `type A B`. The second declares a new
            // type with its own identity, so a value of `Distance` would
            // not be assignable to an `int32` without a conversion —
            // which is not what an IDL typedef says (IDL 4.2 §7.4.1.4.3:
            // the alias "does not define a new type").
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                alias: true,
                type_: go_member_type(ast, target, &node.name)?,
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "GoEnum").is_none() {
                return Ok(());
            }
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // A defined type and not an alias: an enumeration is its own
            // type, and `iota` is deliberately not used — it numbers
            // consecutively from zero and `@value` admits gaps, so an
            // `iota` block would renumber the wire.
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line(
                        "An enumeration. The holder is signed because DDS-XTypes 1.3 \
                         7.4.3.5.3 makes it so; the width on the wire is the \
                         @bit_bound's and is applied by the runtime.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                alias: false,
                type_: go::Type::Primitive(go::Primitive::Int32),
            }));
            let prefix = module_prefix(ast, id);
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "GoEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let constant = format!("{prefix}{}", field_name(&enum_node.name));
                remember(emitted, &constant, &enum_node.name)?;
                out.push(go::Decl::Const(go::ConstDecl {
                    doc: idl_note(&constant, &enum_node.name),
                    name: Ident::new(&constant),
                    type_: Some(go::Type::named(&name)),
                    value: go::Expr::int(value),
                }));
            }
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "GoBitmask").is_none() {
                return Ok(());
            }
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line(
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3 \
                         7.4.1.1.3.2 gives for its bound.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                alias: false,
                type_: go::Type::Primitive(go_holder(bound)),
            }));
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "GoBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = format!("{prefix}{}", field_name(&bit_node.name));
                remember(emitted, &constant, &bit_node.name)?;
                // The shift is bracketed and then converted, so that the
                // constant has the bitmask's own type: Go has no
                // implicit conversion between named integer types, and
                // an untyped `1 << 4` in `flags & MOVING` would take the
                // type of the other operand only by accident of context.
                out.push(go::Decl::Const(go::ConstDecl {
                    doc: idl_note(&constant, &bit_node.name),
                    name: Ident::new(&constant),
                    type_: Some(go::Type::named(&name)),
                    value: go::Expr::Binary {
                        op: "<<",
                        lhs: Box::new(go::Expr::int(1)),
                        rhs: Box::new(go::Expr::int(i128::from(position))),
                    },
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "GoBitset").is_none() {
                return Ok(());
            }
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            // The base's bitfields are members of the derived struct
            // too: an inherited bitset is one flat run.
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                pairs.push((field_name(field_ident), field_ident.clone()));
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            let mut index = 0;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                fields.push(go::Field {
                    doc: Doc::none(),
                    name: Ident::new(&pairs[index].0),
                    type_: go::Type::Primitive(go_holder(field_node.bits)),
                    tag: Some(go::Tag::one("idl", field_ident)),
                });
                index += 1;
            }
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line(
                        "A bitset. The fields are separate integers here; the packing \
                         into the anchor is the codec's, because the order on the wire \
                         is the declaration's.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                alias: false,
                type_: go::Type::Struct(fields),
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "GoValue"
            } else {
                "GoStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`.
            let members = ast.members_of(id).to_vec();
            let base = super::c::struct_base(ast, id);
            let mut pairs = Vec::new();
            if base.is_some() {
                pairs.push((field_name("parent"), "parent".to_string()));
            }
            for member in &members {
                if let Ok(member_node) = ast.member(*member) {
                    pairs.push((field_name(&member_node.name), member_node.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            if let Some(base) = base {
                fields.push(go::Field {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    name: Ident::new(&field_name("parent")),
                    type_: go_member_type(ast, base, "parent")?,
                    tag: Some(go::Tag::one("idl", "parent")),
                });
            }
            for member in &members {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                let mut type_ = go_member_type(ast, target, &member_node.name)?;
                let mut doc = Doc::none();
                if member_node.facets.is_optional() {
                    // A pointer and not a value beside a flag: Go's zero
                    // value is indistinguishable from a written zero, so
                    // a `long` member that arrived as 0 and one that did
                    // not arrive at all would be the same value. `nil`
                    // is the one state no written value can take.
                    type_ = go::Type::pointer(type_);
                    doc = Doc::line(
                        "Optional. `nil` when the member is absent — DDS-XTypes 1.3 \
                         7.2.2.4.4.4.7. In a @mutable type an absent member is not \
                         written at all; outside one its absence is written.",
                    );
                }
                fields.push(go::Field {
                    doc,
                    name: Ident::new(&field_name(&member_node.name)),
                    type_,
                    tag: Some(go::Tag::one("idl", &member_node.name)),
                });
            }
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                alias: false,
                type_: go::Type::Struct(fields),
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "GoUnion").is_none() {
                return Ok(());
            }
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let cases = ast.cases_of(id);
            // The discriminator's field is in the collision check with
            // the branches: IDL 4.2 §7.4.1.4.4.1 gives it no member name
            // of its own, so `D` is this column's, and a branch called
            // `d` would take the same Go spelling and make one of the two
            // unreachable.
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((field_name(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = vec![go::Field {
                doc: Doc::line("Which branch carries the value."),
                name: Ident::new(DISCRIMINATOR),
                type_: go_member_type(ast, disc, &name)?,
                tag: None,
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "GoCase").is_none() {
                    continue;
                }
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                fields.push(go::Field {
                    doc: Doc::none(),
                    name: Ident::new(&field_name(&member.name)),
                    type_: go_member_type(ast, target, &member.name)?,
                    tag: Some(go::Tag::one("idl", &member.name)),
                });
            }
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union. Go has no overlaid storage, so the branches sit side",
                        "by side and `D` says which one carries the value; only that",
                        "branch is written to the wire (DDS-XTypes 1.3 7.4.1.1.5.2).",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                alias: false,
                type_: go::Type::Struct(fields),
            }));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "GoNative").is_none() {
                return Ok(());
            }
            let name = go_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `any` when there is none.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line("A native type. Opaque unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                alias: true,
                type_: match mapping {
                    Some(spelling) => go::Type::named(&spelling),
                    None => go::Type::Primitive(go::Primitive::Any),
                },
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = go_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            out.push(go::Decl::Type(go::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line("A boxed value: a pointer, `nil` when absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                alias: true,
                type_: go::Type::pointer(go_member_type(ast, target, &name)?),
            }));
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `go_member_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The Go field name of the union discriminator.
///
/// `D` and not `_d`: Go cannot export a name beginning with an
/// underscore, and a discriminator no consumer can read is a union no
/// consumer can use. Every other column spells it `_d`, which is why
/// this is stated here rather than assumed.
pub const DISCRIMINATOR: &str = "D";

/// The Go spelling of a value's type, refusing what Go cannot hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided at
/// run time — Go's `any` would compile and there would be no wire form
/// to generate a codec against, which is the silent-substitution class
/// this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn go_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<go::Type, MaterialiseError> {
    let construct = match ast.type_(target).map(|node| &node.kind) {
        Ok(TypeKind::Any) => Some("any"),
        Ok(TypeKind::ValueBase) => Some("ValueBase"),
        Ok(TypeKind::ObjectRef { .. }) => Some("Object"),
        _ => None,
    };
    if let Some(construct) = construct {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: LANGUAGE,
            at: at.to_string(),
        });
    }
    go_value_type(ast, target)
}

/// The Go type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated struct, a
/// codec's parameter and a defaulter's destination all have to name the
/// same Go type or the file does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when an array length or a
/// collection's element does not resolve.
pub fn go_value_type(ast: &Ast, id: Id<TypeNode>) -> Result<go::Type, MaterialiseError> {
    value_type_at(ast, id, 0)
}

/// [`go_value_type`], carrying how many collection and alias steps the
/// walk has already taken.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and anonymous-alias
/// nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`). An aggregate is a leaf here — it is
/// named, not descended into — so only a cycle among the connectors can
/// come back round, and the counter is what stops that rather than the
/// frontend happening to build inline types as a tree.
fn value_type_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<go::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Go spelling nests past {MAX_TYPE_DEPTH}"),
            at: go_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(go::Type::Primitive(go::Primitive::Any));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => go_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units, which puts
        // the §7.4.1.1.2 surrogate rule inside the Go runtime where it
        // is a second implementation of the clause and not a copy of the
        // Rust column's answer. A Go `string` is a run of octets with no
        // encoding of its own, which is what makes that possible.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            go::Type::Primitive(go::Primitive::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            go::Type::slice(value_type_at(ast, element, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // Outermost dimension outermost: `long grid[2][3]` is two
            // rows of three, which in Go is `[2][3]int32`. Building it
            // from the inside out is what keeps that true.
            let mut type_ = value_type_at(ast, element, depth + 1)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: node.name.clone(),
                        })?;
                type_ = go::Type::array(length, type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            go::Type::slice(go::Type::named(&map_entry_name_at(ast, id, depth + 1)?))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the Go type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_type_at(ast, target, depth + 1)?,
            None => go::Type::Primitive(go::Primitive::Any),
        },
        _ => go::Type::named(&go_type_name(ast, id)),
    })
}

/// The Go type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a Go spelling
/// is a compile error and not a member that silently takes another's.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open, and a fixed-width integer is the only choice that does not
/// depend on a platform type. Go's `rune` is `int32` and its `byte` is
/// `uint8`, so neither adds anything over the integer itself.
///
/// `long double` is a `string` and never a `float64`; the module
/// documentation carries why.
#[must_use]
pub fn go_primitive_type(kind: PrimitiveKind) -> go::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => go::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => go::Primitive::UInt8,
        PrimitiveKind::Int8 => go::Primitive::Int8,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => go::Primitive::UInt16,
        PrimitiveKind::Int16 => go::Primitive::Int16,
        PrimitiveKind::Int32 => go::Primitive::Int32,
        PrimitiveKind::UInt32 => go::Primitive::UInt32,
        PrimitiveKind::Int64 => go::Primitive::Int64,
        PrimitiveKind::UInt64 => go::Primitive::UInt64,
        PrimitiveKind::Float32 => go::Primitive::Float32,
        PrimitiveKind::Float64 => go::Primitive::Float64,
        PrimitiveKind::Float128 => go::Primitive::String,
    };
    go::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for.
#[must_use]
pub const fn go_holder(bits: u16) -> go::Primitive {
    match bits {
        0..=8 => go::Primitive::UInt8,
        9..=16 => go::Primitive::UInt16,
        17..=32 => go::Primitive::UInt32,
        _ => go::Primitive::UInt64,
    }
}

/// A declared type's Go identifier.
///
/// The scoped name with the module separators flattened onto it and
/// every segment exported, which is what a package with no nesting
/// requires: `::common::util::Location` is `Common_Util_Location`.
#[must_use]
pub fn go_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "ZdUnnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => flatten(scoped.segments()),
        None => "ZdUnnamed".to_string(),
    }
}

/// The entry struct a `map<K,V>` needs a name for.
///
/// Structural, so two identical anonymous maps collapse onto one
/// declaration instead of two that differ only in a counter. The tag is
/// built from the IDL kind and not the Go spelling: `double` and
/// `long double` are `float64` and `string` here, but `octet` and
/// `uint8` are both `uint8`, and a name built from the Go type would
/// give `map<octet,V>` and `map<uint8,V>` one declaration — harmless
/// today and the same collapse that would be wrong if the two ever
/// differed on the wire.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the key's or the value's tag
/// nests past `MAX_TYPE_DEPTH`.
pub fn go_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    map_entry_name_at(ast, id, 0)
}

/// [`go_map_entry_name`], carrying the depth of the walk that reached
/// it.
///
/// The depth is threaded rather than reset: `type_tag` reaches a map's
/// entry name through this function and this function reaches `type_tag`
/// again, so a counter that started over here would leave the pair with
/// no bound at all.
///
/// zerodds-lint: recursion-depth 64 (key and value tags, via
/// `type_tag`; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn map_entry_name_at(
    ast: &Ast,
    id: Id<TypeNode>,
    depth: usize,
) -> Result<String, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("map entry name, which nests past {MAX_TYPE_DEPTH}"),
            at: go_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(format!("{INVENTED_PREFIX}Unknown"));
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return Ok(format!("{INVENTED_PREFIX}Unknown"));
    };
    let key = match key.resolved() {
        Some(k) => type_tag(ast, k, depth + 1)?,
        None => "Unknown".to_string(),
    };
    let value = match value.resolved() {
        Some(v) => type_tag(ast, v, depth + 1)?,
        None => "Unknown".to_string(),
    };
    Ok(format!("{INVENTED_PREFIX}{key}_{value}"))
}

/// The name component a map entry takes from its key or value.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the tag nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and map nesting;
/// bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<String, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type tag, which nests past {MAX_TYPE_DEPTH}"),
            at: go_type_name(ast, id),
        });
    }
    Ok(match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => go::exported(&kind.as_str().replace(' ', "_")),
        Ok(TypeKind::String {
            width: CharWidth::Char,
            ..
        }) => "String".to_string(),
        Ok(TypeKind::String {
            width: CharWidth::WChar,
            ..
        }) => "WString".to_string(),
        Ok(TypeKind::Sequence { element, .. }) => match element.resolved() {
            Some(element) => format!("SeqOf{}", type_tag(ast, element, depth + 1)?),
            None => "SeqOfUnknown".to_string(),
        },
        Ok(TypeKind::Array { element, dims }) => match element.resolved() {
            Some(element) => {
                let mut name = format!("ArrOf{}", type_tag(ast, element, depth + 1)?);
                for dim in &dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => name.push_str(&format!("_{length}")),
                        // A dimension that does not evaluate gets no
                        // number rather than a zero: the name has to
                        // differ from a `[0]` array's, and the caller
                        // reports the failure by name.
                        Err(_) => name.push_str("_x"),
                    }
                }
                name
            }
            None => "ArrOfUnknown".to_string(),
        },
        Ok(TypeKind::Map { .. }) => map_entry_name_at(ast, id, depth + 1)?,
        _ => go_type_name(ast, id),
    })
}

/// The flattened, exported module path a package-scope constant carries.
///
/// `pub` because the codec backend has to spell an enumerator the same
/// way the declaration above spelled its constant, and a second
/// derivation of the prefix would be a second chance to disagree.
pub fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return String::new();
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = flatten(&segments[..segments.len() - 1]);
    prefix.push('_');
    prefix
}

/// The Go name a declaration outside the type arena takes.
///
/// Constants, exceptions and interfaces are not types, so their scope
/// comes from the module they were declared in rather than from
/// [`Ast::scoped_name_of`].
fn declared_name(ast: &Ast, name: &str, module: Id<zerodds_ast::ModuleNode>) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    // Outermost first, so the walk up is reversed at the end. The bound
    // matches the layout planner's own: a parent chain that does not
    // terminate is a defect in the model and not a reason to hang.
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            break;
        }
        depth += 1;
        let Ok(node) = ast.module(id) else { break };
        if node.name.is_empty() {
            break;
        }
        segments.push(node.name.clone());
        current = node.parent;
    }
    segments.reverse();
    segments.push(name.to_string());
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Go spelling may nest before the walk refuses it.
///
/// The same 64 every reference walk in the tree carries —
/// `zerodds-wire-codec`'s `default.rs` and `zerodds-types`'
/// `dynamic/type_.rs` both stop there. A cycle in the type graph is a
/// finding of the model's own rules, not a shape this materialiser can
/// spell, so it is refused by name instead of followed.
const MAX_TYPE_DEPTH: usize = 64;

/// Join scoped segments with `_`, exporting each.
///
/// `_` and not concatenation: `CommonUtilLocation` cannot be read back
/// to its segments, and two different scoped names would reach one Go
/// name with nothing to say so. The join is still not injective — an IDL
/// identifier may itself contain `_` — which is why [`remember`] checks
/// every declared name against every other one in the file.
fn flatten(segments: &[String]) -> String {
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

fn flatten_str(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| go::exported(&go::escape_name(segment)))
        .collect::<Vec<String>>()
        .join("_")
}

/// The Go spelling of a member, an enumerator or a method.
///
/// Escaped first and exported second, in that order: `type` escapes to
/// `type_` and then exports to `Type_`, where exporting first would give
/// `Type`, which is not a Go keyword at all and would leave the escape
/// with nothing to do — two IDL names, `type` and `Type`, reaching one
/// Go name through two different routes.
#[must_use]
pub fn field_name(name: &str) -> String {
    go::exported(&go::escape_name(name))
}

/// The declaration's own IDL name, where the Go spelling moved it.
fn idl_note(spelled: &str, declared: &str) -> Doc {
    if spelled == declared {
        return Doc::none();
    }
    Doc::line(&format!("IDL: `{declared}`."))
}

/// The same, appended to a documentation comment the caller wrote.
fn with_idl_note(mut doc: Doc, spelled: &str, declared: &str) -> Doc {
    if spelled != declared {
        doc.push(&format!("IDL: `{declared}`."));
    }
    doc
}

/// Record a declared name, refusing a second declaration that reaches
/// the same Go spelling.
///
/// One package block holds every declaration in the file, so this is a
/// whole-file check and not a per-scope one — which is the difference
/// from [`no_spelling_collision`], used for the members of one
/// aggregate. Two IDL modules can flatten onto one Go name (`a_b::C` and
/// `a::b_C` both give `A_B_C`) and Go would report `A_B_C redeclared in
/// this block`, naming neither IDL declaration.
fn remember(
    emitted: &mut Vec<(String, String)>,
    spelled: &str,
    declared: &str,
) -> Result<(), MaterialiseError> {
    if let Some((_, first)) = emitted.iter().find(|(other, _)| other == spelled) {
        return Err(MaterialiseError::SpellingCollision {
            first: first.clone(),
            second: declared.to_string(),
            spelled: spelled.to_string(),
            language: LANGUAGE,
            at: "the generated package".to_string(),
        });
    }
    emitted.push((spelled.to_string(), declared.to_string()));
    Ok(())
}

/// Borrow a list of owned name pairs for [`no_spelling_collision`].
fn as_pairs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(spelled, declared)| (spelled.as_str(), declared.as_str()))
        .collect()
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk every sibling materialiser does: a
/// correspondence is an ordinary node with a link to the anchor and a
/// link to what was created.
fn ghost_created_for(applied: &Applied, node: NodeId, type_name: &str) -> Option<GhostId> {
    let anchor = applied
        .origin
        .iter()
        .find(|(_, model)| **model == node)
        .map(|(ghost, _)| *ghost)?;
    let typ = applied.graph.types.lookup(type_name)?;
    applied
        .graph
        .nodes_of_type(typ)
        .map(|created| created.id)
        .find(|created| {
            applied.graph.connected(&anchor, created)
                || applied
                    .graph
                    .parts(&anchor)
                    .any(|part| applied.graph.connected(&part.other, created))
        })
}

/// The Go expression a constant's value takes.
///
/// Converted to the declared type rather than written bare: Go has no
/// implicit conversion between named types, so `const LIMIT Flags = 8`
/// is only legal because an untyped constant converts at the
/// declaration, and a value written where the type is a defined one has
/// to say which type it is.
///
/// `long double` is refused by name. Its Go holder is a `string`
/// carrying the decimal, and CORBA gives a `long double` literal no
/// spelling that survives a `float64`: emitting one would put a
/// different number in the file than the IDL wrote.
fn go_constant(
    value: &zerodds_ast::ConstValue,
    type_: &go::Type,
    at: &str,
) -> Result<go::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_string = matches!(type_, go::Type::Primitive(go::Primitive::String));
    Ok(match value {
        ConstValue::Boolean(flag) => go::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => go::Expr::Literal(Literal::int(*number)),
        // The code point as a number: an IDL `char` is one octet here and
        // an IDL `wchar` two, and a Go rune literal is neither.
        ConstValue::Char(character) => {
            go::Expr::Literal(Literal::int(i128::from(u32::from(*character))))
        }
        ConstValue::WChar(point) => go::Expr::Literal(Literal::int(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            if is_string {
                return Err(refuse("a `long double` constant, which has no Go literal"));
            }
            go::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            go::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => go::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => go::Expr::Literal(Literal::Str(
            constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a file that disagrees with the IDL and compiles.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
