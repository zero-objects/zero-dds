// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The F# CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::zig`] and [`crate::adapter::go`].
//! Reading the three side by side is the point: all three flatten a
//! module onto the name, and everything else they do differently is
//! their language's — [`crate::adapter::zig`]'s own documentation and
//! [`zerodds_lang_cst::fsharp`]'s carry why this column's aggregate is a
//! mutable class and not Zig's `struct` or F#'s own record.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow (spec §2.3).
//!
//! ## The memory shapes
//!
//! | IDL | F# |
//! |---|---|
//! | `boolean` | `bool` |
//! | `octet`, `uint8`, `char` | `byte` |
//! | `int8` | `sbyte` |
//! | `short` | `int16` |
//! | `unsigned short`, `wchar` | `uint16` |
//! | `long` | `int32` |
//! | `unsigned long` | `uint32` |
//! | `long long` | `int64` |
//! | `unsigned long long` | `uint64` |
//! | `float` | `float32` |
//! | `double` | `float` |
//! | **`long double`** | **`string`, the decimal — .NET has no wider float** |
//! | `string`, `wstring` | `string` |
//! | `fixed<P,S>` | `string`, holding the decimal |
//! | `sequence<T>` | `T []` |
//! | `T[N][M]` | `T [] []`, outer dimension outermost |
//! | `map<K,V>` | `ZdMapEntry_K_V []`, **not** `Dictionary` |
//! | `@optional T m` | `T option`, `None` when absent |
//! | `@external T m` | `T` — a class is a reference already |
//! | `@optional @external T m` | `T option`, exactly as plain `@optional` |
//! | `struct`, `valuetype`, `exception`, `bitset` | a mutable class |
//! | `union` | a mutable class: `_d` and one field per branch |
//! | `enum` | a CLI enum, every value written out |
//! | `bitmask` | an unsigned type abbreviation and one constant per bit |
//! | `typedef` | `type Distance = int32` |
//! | `native` | `obj`, or the facet's mapping |
//! | `interface` | a class of function-valued fields |
//!
//! ### `long double` holds text, but the wire is a real binary128
//!
//! .NET's floating-point set stops at `System.Double`; this column
//! holds the decimal as `string`, the same storage
//! [`crate::adapter::go`] and [`crate::adapter::python`] give, and not
//! [`crate::adapter::zig`]'s `f128` — Zig's own documentation says why
//! it is the one column with a native type wide enough. What goes on
//! the wire is not a workaround, though: `runtime/fsharp/ZdWire.fs`'s
//! `WriteF128`/`ReadF128` write and read the real sixteen-byte
//! IEEE 754 binary128 pattern, the same algorithm
//! `runtime/go/float128.go` carries, ported onto
//! `System.Numerics.BigInteger`.
//!
//! ### A union is a class with a discriminator, not a discriminated union
//!
//! [`zerodds_lang_cst::fsharp`]'s module documentation carries the
//! argument in full; the summary is the one [`crate::adapter::zig`]
//! gives for its own tagged union: an IDL discriminator may take a value
//! no `case` label selects (IDL 4.2 §7.4.1.4.4.2.3), in which state the
//! union carries no branch value at all
//! (DDS-XTypes 1.3 §7.4.1.1.5.2) and F#'s own sum type — whose case *is*
//! its tag — cannot be constructed that way.
//!
//! ### `map<K,V>` is an array of entries and never a `Dictionary`
//!
//! A `map<K,V>` goes on the wire as a sequence of key/value pairs
//! (DDS-XTypes 1.3 §7.4.1.1.4), ordered; `System.Collections.Generic.
//! Dictionary` iterates in an implementation-defined order that follows
//! the hash and the insertion history, not the order the pairs arrived
//! in. The entry class is declared once and named, exactly as Zig's
//! entry struct is, so a codec has a type to take.
//!
//! ### A bitmask is a type abbreviation and one constant per bit
//!
//! As in Zig: the bits carry `@position` (DDS-XTypes 1.3 §7.4.1.1.3.2),
//! a property of the wire, and a language aggregate whose layout follows
//! declaration order — an F# `[<Flags>]` enum included, whose values are
//! still just an integer with named constants and buys nothing a bare
//! type abbreviation does not already have — would still need the same
//! constants to state a position outside declaration order.
//!
//! ### `@external` needs nothing at all
//!
//! DDS-XTypes 1.3 Table 21 says a member is *stored* outside the
//! containing object; it changes no octet. An F# class is heap-allocated
//! reference storage already — `Node()` is exactly as much an
//! indirection as Zig's `*Node`, without a pointer type to write — so
//! `@external` reaches [`fsharp_member_type`] and changes nothing it
//! returns. [`crate::adapter::zig`]'s pointer and this column's absence
//! of one are the same finding read two ways: an indirection is a tree
//! concern only where the language's default aggregate is a value.
//!
//! ### Where the IDL spelling of a name is kept
//!
//! Nothing is renamed for style — F# has no export-by-case rule either.
//! The only rename is the escape, and it reaches further than Zig's:
//! [`fsharp::escape_field`] and [`fsharp::escape_decl`] check the same
//! keyword list in both positions, because unlike Zig's field namespace
//! F# refuses a keyword as a member name too — that module's
//! documentation carries the measurement. Every name the escape moved
//! carries `IDL: \`…\`.` in the documentation comment above it, for the
//! same `@hashid`/`@autoid(HASH)` reason [`crate::adapter::zig`] states.
//!
//! ### Why a class needs a default at every field, and why that costs no
//! ### generated defaulter function
//!
//! [`zerodds_lang_cst::fsharp`]'s module documentation carries the
//! reasoning; [`crate::default_fsharp`] is where DDS-XTypes 1.3 Table 9
//! is read into an expression for `ClassField::default` to hold.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::default_fsharp;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::fsharp;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "F#";

/// The prefix a name this column invented carries.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// The F# field name of a union's discriminator.
///
/// `_d`, as in every column but Go's, for the same reason
/// [`crate::adapter::zig`] keeps it: IDL 4.2 §7.4.1.4.4.1 gives the
/// discriminator no member name of its own, and F#'s visibility has
/// nothing to do with a leading underscore the way Go's export rule
/// does.
pub const DISCRIMINATOR: &str = "_d";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's F# spelling may nest before the walk refuses
/// it.
///
/// The same 64 every reference walk in the tree carries;
/// `zerodds-wire-codec`'s `default.rs` and `zerodds-types`'
/// `dynamic/type_.rs` both stop there. An aggregate is a leaf of these
/// walks — it is named, not descended into — so only a cycle among the
/// connectors (a `typedef`, a `sequence`, an `array`, a `map`) can come
/// back round, and the counter is what stops that rather than the
/// frontend happening to build inline types as a tree.
const MAX_TYPE_DEPTH: usize = 64;

/// The name every generated file's `module rec` takes.
///
/// One name and not one per IDL file's own name: the generator writes
/// exactly one file per probe run, and — as `Program.fs`'s own
/// `EntryPoint` does — the module a script is compiled as needs no
/// bearing on the IDL source's name.
pub const MODULE_NAME: &str = "ZdTypes";

/// Build the F# declarations a rule application produced.
///
/// The order is the model's own declaration order. `module rec` gives
/// every declaration mutual visibility regardless of where it is
/// written (see [`zerodds_lang_cst::fsharp`]'s module documentation), so
/// — as in Zig and Go — there is no dependency walk here either.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no F# shape, or two declarations reach one F#
/// spelling.
pub fn fsharp_items(ast: &Ast, applied: &Applied) -> Result<Vec<fsharp::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::FSHARP),
            &fsharp_type_name(ast, id),
            start,
            &mut out,
        );
    }

    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "FsConst").is_none() {
            continue;
        }
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = node.type_.resolved();
        let type_ = match declared {
            Some(target) => fsharp_value_type(ast, target, &node.name)?,
            None => fsharp::Type::Primitive(fsharp::Primitive::Int32),
        };
        // `long double` and `string`/`wstring`/`fixed<P,S>` all
        // materialise to the same `Type::Primitive(String)`, and only
        // the first of the four takes a floating-point `ConstValue` —
        // `fsharp_constant` needs to tell them apart to answer a
        // `const long double` with the decimal text rather than
        // refusing it as "a float into a text-shaped type".
        let is_long_double = declared.is_some_and(|target| {
            matches!(
                ast.type_(target).map(|node| node.kind.clone()),
                Ok(TypeKind::Primitive(PrimitiveKind::Float128))
            )
        });
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(fsharp::Decl::Let(fsharp::LetDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            params: Vec::new(),
            type_: Some(type_.clone()),
            body: vec![fsharp::Stmt::Return(fsharp_constant(
                &value,
                &type_,
                is_long_double,
                &node.name,
            )?)],
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "FsError").is_none() {
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
                return Err(MaterialiseError::NoNode {
                    kind: "member",
                    at: crate::adapter::cst::handle_of(*member),
                });
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            fields.push(member_field(ast, member_node, target)?);
        }
        out.push(fsharp::Decl::Type(fsharp::TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line("An exception, as the class its members make."),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            body: fsharp::TypeBody::Class(fields),
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "FsInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut fields = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. `operations_of`
        // stops at what the interface writes down; §7.4.3 says a client
        // holding a `Derived` may call every operation of `Base` (and
        // every attribute) too, and this column's class has no base to
        // inherit them through — it is a flat class of function-valued
        // fields. Walking the declared list left `interface Derived :
        // Base` with a class carrying only its own operations, the
        // base's gone and no diagnostic anywhere.
        // `interface::all_operations_flattened` gives the flattened
        // operations and, per §7.5.1.1.3, the get_/set_ pair each
        // attribute implies — which `Ast::all_operations_of` alone
        // never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            // The field of an attribute accessor is `get_<attribute>` or
            // `set_<attribute>`: the attribute's own name, not the
            // `get_attribute_`/`set_attribute_` DDS-RPC 1.0 §7.5.1.1.3 gives
            // the implied operation, and two names so that the pair does not
            // collide.
            let field_ident = match op.origin {
                interface::Origin::AttributeGetter => {
                    field_name(&format!("get_{}", op.attribute_name()))
                }
                interface::Origin::AttributeSetter => {
                    field_name(&format!("set_{}", op.attribute_name()))
                }
                interface::Origin::Declared => field_name(&op.name),
            };
            signatures.push((field_ident.clone(), op.name.clone()));
            let mut param_types = Vec::new();
            for param in &op.params {
                param_types.push(fsharp_member_type(ast, param.type_, &param.name)?);
            }
            let returns = match op.returns {
                Some(target) => fsharp_member_type(ast, target, &op.name)?,
                None => fsharp::Type::Primitive(fsharp::Primitive::Unit),
            };
            let fn_type = fsharp::Type::Fn {
                params: param_types,
                returns: Box::new(returns.clone()),
            };
            let mut doc = idl_note(&field_ident, &op.name);
            if let Some(line) = op.raises_line() {
                doc.push(&line);
            }
            fields.push(fsharp::ClassField {
                doc,
                name: Ident::new(&field_ident),
                type_: fn_type,
                default: interface_default(&returns),
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(fsharp::Decl::Type(fsharp::TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface, as the class of function-valued fields its",
                    "operations and attributes make — an attribute a get_/set_ pair of",
                    "fields, `readonly` only the first. The transport is the RPC arc's",
                    "business; the shape is the IDL's.",
                    "`raises`/`getraises`/`setraises` are documented on the field they",
                    "belong to.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            body: fsharp::TypeBody::Class(fields),
        }));
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::FSHARP, out))
}

/// `@verbatim` in the F# column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a class's body; an enumeration and an
/// abbreviation take them adjacent.
impl verbatim::Carrier for fsharp::Decl {
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
        if node.name.as_str() != name || !matches!(node.body, fsharp::TypeBody::Class(_)) {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// The default an interface operation's function-valued field carries:
/// a lambda that raises rather than a plausible answer, because there
/// is no Table 9 row for an operation and a silently-wrong return value
/// is the class of defect this project refuses to build in.
fn interface_default(returns: &fsharp::Type) -> fsharp::Expr {
    let _ = returns;
    fsharp::Expr::call(
        "fun _ -> failwith",
        vec![fsharp::Expr::Literal(Literal::string(
            "an interface field with no implementation bound to it",
        ))],
    )
}

/// The F# declarations, in a file that names its module.
///
/// A caller that only wants the declarations uses [`fsharp_items`]; this
/// is what a generator writes to disk.
///
/// # Errors
/// As [`fsharp_items`].
pub fn fsharp_file(ast: &Ast, applied: &Applied) -> Result<fsharp::File, MaterialiseError> {
    let decls = fsharp_items(ast, applied)?;
    Ok(fsharp::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "The IDL spelling of every renamed name is in the comment above it.",
            "Nothing is renamed for style; a rename here is always an escape: an F#",
            "keyword used as a member or a declaration. A member's ID under",
            "@autoid(HASH) is the MD5 of its IDL name (DDS-XTypes 1.3 7.3.1.2.1.1)",
            "and the escape does not invert, which is why the comment is there.",
        ]),
        module: Ident::new(MODULE_NAME),
        decls,
    })
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<fsharp::Decl>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    // `id` came from `ast.types()` in `fsharp_items`, so an arena that
    // will not answer for it contradicts the iterator that produced it.
    // `Ok(())` here meant the whole declaration left the file with no
    // word said, and the two walks below — `NoNode` for an exception
    // member, an operation, a parameter, an enumerator and a bit — are
    // this file's own contract for the same handle.
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::NoNode {
            kind: "type",
            at: crate::adapter::cst::handle_of(id),
        });
    };

    match &node.kind {
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: fsharp_type_name(ast, id),
                });
            };
            let name = map_entry_name_at(ast, id, 0)?;
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            let key_default =
                default_fsharp::default_expr(ast, key).map_err(|error| no_form(&error, &name))?;
            let value_default =
                default_fsharp::default_expr(ast, value).map_err(|error| no_form(&error, &name))?;
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::lines(&[
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a",
                    "sequence of these, and the rules write the two halves with no",
                    "framing between them.",
                ]),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Class(vec![
                    fsharp::ClassField {
                        doc: Doc::none(),
                        name: Ident::new("Key"),
                        type_: fsharp_member_type(ast, key, &name)?,
                        default: key_default,
                    },
                    fsharp::ClassField {
                        doc: Doc::none(),
                        name: Ident::new("Value"),
                        type_: fsharp_member_type(ast, value, &name)?,
                        default: value_default,
                    },
                ]),
            }));
        }

        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "FsAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = fsharp_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // `type Distance = int32` — a type abbreviation binds a name
            // to a type and creates no new one, which IDL 4.2 §7.4.1.4.3
            // says an alias does.
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Alias(fsharp_member_type(ast, target, &node.name)?),
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "FsEnum").is_none() {
                return Ok(());
            }
            let name = fsharp_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            for enumerator in enumerators {
                if let Ok(enum_node) = ast.enumerator(*enumerator) {
                    pairs.push((field_name(&enum_node.name), enum_node.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut variants = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "enumerator",
                        at: crate::adapter::cst::handle_of(*enumerator),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "FsVariant").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let spelled = field_name(&enum_node.name);
                variants.push(fsharp::Variant {
                    doc: idl_note(&spelled, &enum_node.name),
                    name: Ident::new(&spelled),
                    value: fsharp::Expr::int(value),
                });
            }
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration. The backing type is signed because DDS-XTypes",
                        "1.3 7.4.3.5.3 makes it so; the width on the wire is the",
                        "@bit_bound's and is applied by the runtime.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Enum {
                    backing: fsharp::Type::Primitive(fsharp::Primitive::Int32),
                    variants,
                },
            }));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "FsBitmask").is_none() {
                return Ok(());
            }
            let name = fsharp_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitmask, as the unsigned type abbreviation Table 33 of",
                        "DDS-XTypes 1.3 7.4.1.1.3.2 gives for its bound. Not an F#",
                        "[<Flags>] enum: a bit's place is its @position, which admits",
                        "gaps and any order, and an enum's own layout would say nothing",
                        "about it that these constants do not already say.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Alias(fsharp::Type::Primitive(fsharp_holder(bound))),
            }));
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "bit",
                        at: crate::adapter::cst::handle_of(*bit),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "FsBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = fsharp::escape_decl(&format!("{prefix}{}", bit_node.name));
                remember(emitted, &constant, &bit_node.name)?;
                // `uint16(1 <<< 4)` and not a bare `1 <<< 4`: F# gives
                // `1` no width until something asks, `<<<`'s result is
                // then the same `int32` its left operand was, and the
                // `let`'s own ascription to `Flags` does not coerce it
                // — `typed_int`'s documentation carries the measurement.
                // `Flags` is `uint16`'s own abbreviation, so wrapping in
                // the holder's conversion function types the whole
                // expression exactly the ascription needs.
                let shifted = fsharp::Expr::Binary {
                    op: "<<<",
                    lhs: Box::new(fsharp::Expr::int(1)),
                    rhs: Box::new(fsharp::Expr::int(i128::from(position))),
                };
                out.push(fsharp::Decl::Let(fsharp::LetDecl {
                    doc: idl_note(&constant, &bit_node.name),
                    name: Ident::new(&constant),
                    params: Vec::new(),
                    type_: Some(fsharp::Type::named(&name)),
                    body: vec![fsharp::Stmt::Return(typed_int(
                        fsharp_holder(bound),
                        shifted,
                    ))],
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "FsBitset").is_none() {
                return Ok(());
            }
            let name = fsharp_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                // Loud, as the bitmask's `bit_value` walk above already
                // is: a skipped bitfield left the class one field short
                // and the collision check one name short, and the
                // generated file still compiled — a bitset that lost a
                // declared field with nothing said.
                let Ok(field_node) = ast.bitfield(*field) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "bitfield",
                        at: crate::adapter::cst::handle_of(*field),
                    });
                };
                // An anonymous bitfield is padding and declares no
                // field; the codec still advances by its width.
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
                    return Err(MaterialiseError::NoNode {
                        kind: "bitfield",
                        at: crate::adapter::cst::handle_of(*field),
                    });
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                let _ = field_ident;
                fields.push(fsharp::ClassField {
                    doc: idl_note(
                        &pairs[index].0,
                        &field_node.name.clone().unwrap_or_default(),
                    ),
                    name: Ident::new(&pairs[index].0),
                    type_: fsharp::Type::Primitive(fsharp_holder(field_node.bits)),
                    default: typed_int(fsharp_holder(field_node.bits), fsharp::Expr::int(0)),
                });
                index += 1;
            }
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate integers here and not packed",
                        "into one holder, because the order on the wire is the",
                        "declaration's and the packing into the anchor is the codec's.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Class(fields),
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "FsValue"
            } else {
                "FsClass"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = fsharp_type_name(ast, id);
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
                let mut doc = Doc::none();
                doc.push("The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).");
                fields.push(fsharp::ClassField {
                    doc,
                    name: Ident::new(&field_name("parent")),
                    type_: fsharp_member_type(ast, base, "parent")?,
                    default: default_fsharp::default_expr(ast, base)
                        .map_err(|error| no_form(&error, "parent"))?,
                });
            }
            for member in &members {
                // Loud, as the exception's member walk in `fsharp_items`
                // already is: skipping one left the class a field short
                // — a member the IDL declares, absent from the generated
                // type and from every diagnostic — while the file still
                // compiled, which is the defect class this tree refuses.
                let Ok(member_node) = ast.member(*member) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "member",
                        at: crate::adapter::cst::handle_of(*member),
                    });
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                fields.push(member_field(ast, member_node, target)?);
            }
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Class(fields),
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "FsUnion").is_none() {
                return Ok(());
            }
            let name = fsharp_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let cases = ast.cases_of(id);
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((field_name(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let disc_default = default_fsharp::discriminator_default(ast, disc)
                .map_err(|error| no_form(&error, &name))?;
            let mut fields = vec![fsharp::ClassField {
                doc: Doc::line("Which branch carries the value."),
                name: Ident::new(DISCRIMINATOR),
                type_: fsharp_member_type(ast, disc, &name)?,
                default: disc_default,
            }];
            for case in cases {
                // Loud on both handles. A skipped case is a branch the
                // union declares and the class does not carry: the
                // discriminator still reaches that label, and the
                // generated file has no field to read it into — a
                // branch that vanishes between the IDL and the type.
                // The `ghost_created_for` skip between them is a
                // different thing: the rule did not fire for that case,
                // which the overlay says on purpose.
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "case",
                        at: crate::adapter::cst::handle_of(*case),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "FsCase").is_none() {
                    continue;
                }
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "member",
                        at: crate::adapter::cst::handle_of(case_node.member),
                    });
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                fields.push(member_field(ast, member, target)?);
            }
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union, as a class carrying the discriminator beside every",
                        "branch. Not an F# discriminated union: a case's data *is* its",
                        "tag, and an IDL discriminator may take a value no case label",
                        "selects (IDL 4.2 7.4.1.4.4.2.3), in which state no branch is",
                        "live and only the discriminator goes on the wire",
                        "(DDS-XTypes 1.3 7.4.1.1.5.2). A discriminated union cannot be",
                        "constructed that way; this class holds it exactly. Every",
                        "branch field defaults to its own type's Table 9 value, whether",
                        "or not the default discriminator selects it — the wire never",
                        "reads an unselected branch, so any value there is as good as",
                        "any other, and a real one costs nothing an undefined one saves.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Class(fields),
            }));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "FsNative").is_none() {
                return Ok(());
            }
            let name = fsharp_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line("A native type. `obj` unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Alias(match mapping {
                    Some(spelling) => fsharp::Type::named(&spelling),
                    None => fsharp::Type::named("obj"),
                }),
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = fsharp_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            // `T option`: an F# class reference already carries the
            // indirection a boxed value needs, so only the absence has
            // to be added and `option` is exactly that.
            out.push(fsharp::Decl::Type(fsharp::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line("A boxed value: `None` when absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                body: fsharp::TypeBody::Alias(fsharp::Type::option(fsharp_member_type(
                    ast, target, &name,
                )?)),
            }));
        }

        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// Turn a [`crate::encode::EncodeError`] arising from a Table 9 lookup
/// into the materialiser's own error type, for the two positions —
/// a map entry's fields, a union discriminator's default — that need
/// one before any codec walk exists to report it through `EncodeError`
/// itself.
fn no_form(error: &crate::encode::EncodeError, at: &str) -> MaterialiseError {
    MaterialiseError::NoLanguageForm {
        construct: error.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    }
}

/// The F# spelling of a value's type, refusing what F# cannot hold.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn fsharp_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<fsharp::Type, MaterialiseError> {
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
    fsharp_value_type(ast, target, at)
}

/// The F# type a value of `id` is held in. The one answer, for every
/// emitter: a class field, a codec's parameter and a default expression
/// all have to name the same F# type or the file does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the type, an array length or a
/// collection's element does not resolve.
pub fn fsharp_value_type(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
) -> Result<fsharp::Type, MaterialiseError> {
    value_type_at(ast, id, at, 0)
}

/// [`fsharp_value_type`], carrying how many connector steps the walk has
/// already taken.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the spelling nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array, map and
/// anonymous-alias nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn value_type_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<fsharp::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose F# spelling nests past {MAX_TYPE_DEPTH}"),
            at: fsharp_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "type behind a member".to_string(),
            at: at.to_string(),
        });
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => fsharp_primitive_type(*kind),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => fsharp::Type::text(),
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            fsharp::Type::array(value_type_at(ast, element, at, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            let mut type_ = value_type_at(ast, element, at, depth + 1)?;
            for dim in dims.iter().rev() {
                let _ = ast
                    .eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: node.name.clone(),
                    })?;
                type_ = fsharp::Type::array(type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            fsharp::Type::array(fsharp::Type::named(&map_entry_name_at(ast, id, depth + 1)?))
        }
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "inline typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            value_type_at(ast, target, at, depth + 1)?
        }
        _ => fsharp::Type::named(&fsharp_type_name(ast, id)),
    })
}

/// The F# type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without an F# spelling
/// is a compile error. `char` is one octet and `wchar` two, the wire
/// widths DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding free to choose
/// for `Char16`; `long double` is `string`, the module documentation's
/// answer to .NET having nothing 113 bits wide.
#[must_use]
pub fn fsharp_primitive_type(kind: PrimitiveKind) -> fsharp::Type {
    use fsharp::Primitive as P;
    let primitive = match kind {
        PrimitiveKind::Boolean => P::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => P::Byte,
        PrimitiveKind::Int8 => P::SByte,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => P::UInt16,
        PrimitiveKind::Int16 => P::Int16,
        PrimitiveKind::Int32 => P::Int32,
        PrimitiveKind::UInt32 => P::UInt32,
        PrimitiveKind::Int64 => P::Int64,
        PrimitiveKind::UInt64 => P::UInt64,
        PrimitiveKind::Float32 => P::Float32,
        PrimitiveKind::Float64 => P::Double,
        PrimitiveKind::Float128 => return fsharp::Type::text(),
    };
    fsharp::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for: the smallest standard
/// width that holds it, exactly [`crate::adapter::zig::zig_holder`]'s
/// reasoning read onto F#'s type names.
#[must_use]
pub const fn fsharp_holder(bits: u16) -> fsharp::Primitive {
    match bits {
        0..=8 => fsharp::Primitive::Byte,
        9..=16 => fsharp::Primitive::UInt16,
        17..=32 => fsharp::Primitive::UInt32,
        _ => fsharp::Primitive::UInt64,
    }
}

/// Wraps `value` in the conversion function a literal of `primitive`
/// needs to compile as one.
///
/// F# gives `int32` and `float` (`System.Double`) bare literal syntax
/// and every other numeric primitive none: measured (`fsi`, .NET SDK
/// 9.0.106), `let b : byte = 0` is `error FS0001` — `0` is `int` and
/// does not become `byte` under the ascription, a function argument
/// position refuses the same way, and neither does an operator
/// expression like `1 <<< 4` assigned to a narrower-typed `let`. The
/// conversion function is what every one of those positions accepts
/// unchanged: `byte 0`, `uint16 (1 <<< 4)`, `byte(1)` all compile
/// (parentheses or a space both work; this column always uses the
/// former, through [`fsharp::Expr::call`]).
///
/// ## Why a literal gets the suffix and not the conversion
///
/// The conversion function takes an *expression*, and an integer
/// literal that is not ascribed anything is `int` before the conversion
/// ever sees it. `uint32(3735928559)` and `uint64(16045690984833335998)`
/// are therefore both `error FS1147`, "this number is outside the
/// allowable range for a 32-bit signed integer" — measured (`dotnet
/// fsi`, .NET SDK 9.0.106) on `matrix/cases/array-1d-primitives`, whose
/// `unsigned long` sample carries 0xDEADBEEF. The literal suffixes of
/// the F# specification §4.3 give the constant its width in the lexer
/// instead, where no `int` range applies: `3735928559u`,
/// `16045690984833335998UL`, and the signed minima `-128y`, `-32768s`
/// and `-9223372036854775808L` that the conversion form cannot express
/// either.
#[must_use]
pub fn typed_int(primitive: fsharp::Primitive, value: fsharp::Expr) -> fsharp::Expr {
    use fsharp::Primitive as P;
    let (conversion, suffix) = match primitive {
        P::SByte => ("sbyte", "y"),
        P::Byte => ("byte", "uy"),
        P::Int16 => ("int16", "s"),
        P::UInt16 => ("uint16", "us"),
        // `int32` takes a bare literal; wrapping it would compile too,
        // but every other column's constant tables read as decimals
        // and not as calls, and this one is no different where it does
        // not have to be.
        P::Int32 => return value,
        P::UInt32 => ("uint32", "u"),
        P::Int64 => ("int64", "L"),
        P::UInt64 => ("uint64", "UL"),
        P::Bool | P::Float32 | P::Double | P::String | P::Unit => return value,
    };
    if let fsharp::Expr::Literal(Literal::Int(digits)) = &value {
        return fsharp::Expr::Literal(Literal::Int(format!("{digits}{suffix}")));
    }
    fsharp::Expr::call(conversion, vec![value])
}

/// A declared type's F# identifier: the scoped name flattened onto the
/// module, exactly as [`crate::adapter::zig::zig_type_name`] flattens
/// its own — `::common::util::Location` is `common_util_Location`, and
/// no case is changed.
#[must_use]
pub fn fsharp_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "ZdUnnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => flatten(scoped.segments()),
        None => "ZdUnnamed".to_string(),
    }
}

/// The entry class a `map<K,V>` needs a name for. Structural, exactly as
/// [`crate::adapter::zig::zig_map_entry_name`] is, and built from the
/// IDL kind rather than the F# spelling for the reason that function's
/// documentation gives.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a key's or a value's tag
/// nests past `MAX_TYPE_DEPTH`.
pub fn fsharp_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    map_entry_name_at(ast, id, 0)
}

/// [`fsharp_map_entry_name`], carrying the depth of the walk that reached it.
///
/// The depth is threaded rather than reset: `type_tag` reaches a
/// map's entry name through this function and this function reaches
/// `type_tag` again, so a counter that started over here would leave
/// the pair with no bound at all.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a key's or a value's tag
/// nests past `MAX_TYPE_DEPTH`.
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
            at: fsharp_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(format!("{INVENTED_PREFIX}unknown"));
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return Ok(format!("{INVENTED_PREFIX}unknown"));
    };
    let key = match key.resolved() {
        Some(k) => type_tag(ast, k, depth + 1)?,
        None => "unknown".to_string(),
    };
    let value = match value.resolved() {
        Some(v) => type_tag(ast, v, depth + 1)?,
        None => "unknown".to_string(),
    };
    Ok(format!("{INVENTED_PREFIX}{key}_{value}"))
}

/// # Errors
/// [`MaterialiseError::Unevaluated`] when the tag nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and map
/// nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<String, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type tag, which nests past {MAX_TYPE_DEPTH}"),
            at: fsharp_type_name(ast, id),
        });
    }
    Ok(match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => kind.as_str().replace(' ', "_"),
        Ok(TypeKind::String {
            width: CharWidth::Char,
            ..
        }) => "string".to_string(),
        Ok(TypeKind::String {
            width: CharWidth::WChar,
            ..
        }) => "wstring".to_string(),
        Ok(TypeKind::Sequence { element, .. }) => match element.resolved() {
            Some(element) => format!("seq_of_{}", type_tag(ast, element, depth + 1)?),
            None => "seq_of_unknown".to_string(),
        },
        Ok(TypeKind::Array { element, dims }) => match element.resolved() {
            Some(element) => {
                let mut name = format!("arr_of_{}", type_tag(ast, element, depth + 1)?);
                for dim in &dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => name.push_str(&format!("_{length}")),
                        Err(_) => name.push_str("_x"),
                    }
                }
                name
            }
            None => "arr_of_unknown".to_string(),
        },
        Ok(TypeKind::Map { .. }) => map_entry_name_at(ast, id, depth + 1)?,
        _ => fsharp_type_name(ast, id),
    })
}

/// The flattened module path a file-scope constant carries.
fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
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
    let mut prefix = segments[..segments.len() - 1].join("_");
    prefix.push('_');
    prefix
}

/// The F# name a declaration outside the type arena takes.
fn declared_name(ast: &Ast, name: &str, module: Id<zerodds_ast::ModuleNode>) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
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
    fsharp::escape_decl(&segments.join("_"))
}

/// Join scoped segments with `_` and escape the result — the same
/// non-per-segment escape [`crate::adapter::zig`]'s `flatten` applies,
/// for the reason its documentation gives.
fn flatten(segments: &[String]) -> String {
    fsharp::escape_decl(&segments.join("_"))
}

/// The F# spelling of a member, an enumerator, a bitfield or an
/// operation. Unlike Zig's [`crate::adapter::zig::field_name`], this
/// checks the same keyword list [`fsharp::escape_decl`] does — the
/// module documentation of [`zerodds_lang_cst::fsharp`] carries the
/// measurement that a field position refuses a keyword too.
#[must_use]
pub fn field_name(name: &str) -> String {
    fsharp::escape_field(name)
}

/// Whether a member is annotated `@external`.
fn is_external(member: &zerodds_ast::MemberNode) -> bool {
    member
        .facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_external)
}

/// The field one member declaration becomes.
///
/// `@external` reaches nothing here — the module documentation says
/// why — and `@optional` becomes `T option`, `None` its default: F#'s
/// optional has a `None` no value of `T` can equal, where a
/// zero-initialised `int32` that arrived as 0 and one that did not
/// arrive at all would be the same value.
fn member_field(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<fsharp::ClassField, MaterialiseError> {
    let spelled = field_name(&member.name);
    let mut doc = idl_note(&spelled, &member.name);
    let type_ = fsharp_member_type(ast, target, &member.name)?;
    if is_external(member) {
        doc.push("Stored by reference — @external, DDS-XTypes 1.3 Table 21. An F# class");
        doc.push("is a reference already, and the indirection is what makes a");
        doc.push("self-recursive type finite.");
    }
    let (type_, default) = if member.facets.is_optional() {
        doc.push("Optional. `None` when the member is absent — DDS-XTypes 1.3");
        doc.push("7.2.2.4.4.4.7. In a @mutable type an absent member is not written");
        doc.push("at all; outside one its absence is written.");
        (fsharp::Type::option(type_), fsharp::Expr::None_)
    } else {
        let default = default_fsharp::default_expr(ast, target)
            .map_err(|error| no_form(&error, &member.name))?;
        (type_, default)
    };
    Ok(fsharp::ClassField {
        doc,
        name: Ident::new(&spelled),
        type_,
        default,
    })
}

/// The declaration's own IDL name, where the F# spelling moved it.
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
/// the same F# spelling — one `module rec` holds every declaration, so
/// this is a whole-file check, exactly as
/// [`crate::adapter::zig`]'s own `remember` is.
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
            at: "the generated file".to_string(),
        });
    }
    emitted.push((spelled.to_string(), declared.to_string()));
    Ok(())
}

fn as_pairs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(spelled, declared)| (spelled.as_str(), declared.as_str()))
        .collect()
}

/// Whether the rules created a node of `type_name` for `node`.
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

/// The F# expression a constant's value takes.
fn fsharp_constant(
    value: &zerodds_ast::ConstValue,
    type_: &fsharp::Type,
    is_long_double: bool,
    at: &str,
) -> Result<fsharp::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_text = matches!(type_, fsharp::Type::Primitive(fsharp::Primitive::String));
    // `typed_int` only where `type_` is a bare primitive: a `const`
    // whose type is a bitmask's abbreviation or an enumeration reaches
    // this through a different `TypeKind` arm entirely, and wrapping a
    // `Named` type here would call a conversion function that does not
    // exist.
    let wrap = |value: fsharp::Expr| match type_ {
        fsharp::Type::Primitive(primitive) => typed_int(*primitive, value),
        _ => value,
    };
    Ok(match value {
        ConstValue::Boolean(flag) => fsharp::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => wrap(fsharp::Expr::int(*number)),
        ConstValue::Char(character) => wrap(fsharp::Expr::int(i128::from(u32::from(*character)))),
        ConstValue::WChar(point) => wrap(fsharp::Expr::int(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            let decimal = constant::float_text(mantissa, exponent.as_deref());
            // `long double` and `string`/`wstring`/`fixed<P,S>` share
            // one F# type; a `long double` constant is legitimately a
            // floating-point value of a text-shaped type, and the
            // decimal text is exactly what its member holds — the
            // caller's `is_long_double` says which of the two this is.
            if is_text && !is_long_double {
                return Err(refuse("a floating-point constant of a text-shaped type"));
            }
            if is_long_double {
                return Ok(fsharp::Expr::Literal(Literal::Str(decimal)));
            }
            let literal = fsharp::Expr::Literal(Literal::Float(decimal));
            // `float32`'s bare decimal is `float` (F#'s default width);
            // `typed_int`'s documentation carries the measurement for
            // the integer case, and this is the same finding for the
            // one non-integer width it does not cover.
            if *type_ == fsharp::Type::Primitive(fsharp::Primitive::Float32) {
                fsharp::Expr::call("float32", vec![literal])
            } else {
                literal
            }
        }
        ConstValue::Fixed { digits, scale } => {
            fsharp::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => fsharp::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => fsharp::Expr::Literal(Literal::Str(
            constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )),
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
