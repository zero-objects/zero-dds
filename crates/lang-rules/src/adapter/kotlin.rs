// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::java`] and [`crate::adapter::zig`].
//! Kotlin shares the JVM's value model with Java — no unsigned types, a
//! `Char` is one UTF-16 code unit, every field is already a reference —
//! so the memory shapes below repeat Java's answer wherever Java's
//! answer is the JVM's and not merely Java's own syntax. Where this
//! column differs it is because Kotlin's own surface offers something
//! neither Java nor Zig has, and each difference is argued in
//! `crates/lang-cst/src/kotlin/mod.rs`, which this file is the producer
//! for.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule every sibling materialiser follows (spec §2.3).
//!
//! ## The memory shapes
//!
//! | IDL | Kotlin |
//! |---|---|
//! | `boolean` | `Boolean` |
//! | `octet`, `uint8`, `unsigned short`, `wchar` | `Int` |
//! | `char` | `Char` |
//! | `int8` | `Byte` |
//! | `short` | `Short` |
//! | `long` | `Int` |
//! | `unsigned long`, `long long`, `unsigned long long` | `Long` |
//! | `float` | `Float` |
//! | `double` | `Double` |
//! | **`long double`** | **`ZdWire.Float128`** |
//! | `string`, `wstring`, `fixed<P,S>` | `String` |
//! | `sequence<T>` | `Array<T>` (or `TArray` for a primitive `T`) |
//! | `T[N][M]` | `Array<Array<T>>`, outer dimension outermost |
//! | `map<K,V>` | `Array<ZdEntry_K_V>` |
//! | `@optional T m` | `T?`, `null` when absent |
//! | `@external T m` | `T` — every JVM field is already a reference |
//! | `struct`, `valuetype`, `exception`, `bitset` | `data class`, plain `class` when empty |
//! | `union` | `data class` with `_d`, one property per branch |
//! | `enum` | `object` of `const val Int` |
//! | `bitmask` | `typealias` to `Int`/`Long`, `const val` per bit |
//! | `typedef` | `typealias` |
//! | `native` | `typealias` to `Any`, or the facet's mapping |
//! | `interface` | a `class` of function-typed properties |
//!
//! Kotlin requires a data class to have at least one primary-constructor
//! property. An empty IDL aggregate therefore uses a constructible plain
//! class, with no invented marker property. It keeps the same constructor
//! and codec calls, but has ordinary reference equality, not generated data
//! class equality/copy methods. Inherited fields count: the decision follows
//! the complete materialized property list, not only locally declared members.
//!
//! ### An enumeration is an `object` of constants, not `enum class`
//!
//! Kotlin's `enum class` fixes one value per constant to the
//! declaration's own ordinal sequence (Kotlin language specification,
//! "Enum classes") — the constant's *position*, not a value the
//! declaration states. DDS-XTypes 1.3 §7.4.3.5.3 admits an
//! enumerator's `@value` out of order and with gaps, which an ordinal
//! cannot reproduce without renumbering the wire — the identical reason
//! `adapter/java.rs` gives for declaring `public static final int`
//! instead of `enum`. This column's `object` of `const val Int` is that
//! same answer, spelled as a singleton because that is what an
//! aggregate of constants with no instance is in Kotlin.
//!
//! A member holding an enumeration's value is therefore plain `Int` —
//! `Java::scalar`'s `Scalar::Int32` answer, unchanged — and casting a
//! decoded pattern into it costs nothing beyond the ordinary integer
//! narrowing every other integral member needs. Zig's tagged `enum(i32)`
//! needs `@enumFromInt`/`@intFromEnum` at every use for exactly the
//! reason this column does not: a Zig enum is a distinct type from its
//! backing integer and a Kotlin `Int` constant is not.
//!
//! ### `@external` needs no indirection of its own
//!
//! Every JVM object field is already a reference (JLS/Kotlin language
//! specification both, "the values of a reference type are references");
//! DDS-XTypes 1.3 Table 21 says `@external` changes where the value is
//! *stored* and no octet the wire carries, so there is nothing for this
//! column to add. `crates/lang-rules/src/emit/kotlin.rs` reaches
//! `CodecTarget::external_value`'s default for the same reason
//! `adapter/java.rs`'s emitter does.
//!
//! ### Where the IDL spelling of a name is kept
//!
//! As in the Zig and Java columns: nothing is renamed for style, and a
//! rename is always [`kotlin::escape_identifier`]'s keyword escape. The
//! IDL spelling is written into the comment above every name the escape
//! moved, because DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's
//! `@autoid(HASH)` id from the first four octets of the MD5 of its
//! **IDL** name and the escape does not invert.
//!
//! ### A module flattens onto the name
//!
//! Kotlin's package system nests exactly the way an IDL `module` does,
//! and this column does not use that: every generated declaration lives
//! in one file with one package line, because a probe compiles one file
//! and a second package per module would need a second file to put it
//! in. `::common::util::Location` is therefore `common_util_Location`,
//! the same flattening [`crate::adapter::zig`] and [`crate::adapter::c`]
//! give for the same reason.

use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::kotlin;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Kotlin";

/// The prefix a name this column invented carries — a map's entry class.
const INVENTED_PREFIX: &str = "ZdEntry_";

/// The Kotlin property name of a union's discriminator. IDL 4.2
/// §7.4.1.4.4.1 gives it no member name of its own.
pub const DISCRIMINATOR: &str = "_d";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// How far along a chain of base interfaces the walk follows. The bound
/// [`Ast::all_members_of`] keeps for a structure's base chain.
const MAX_INTERFACE_DEPTH: usize = 64;

/// Build the Kotlin declarations a rule application produced.
///
/// The order is the model's own declaration order — Kotlin resolves a
/// top-level declaration anywhere in the file regardless of where it is
/// written (Kotlin language specification, "Declarations"), so — as in
/// Zig and Go and unlike C — there is no dependency walk.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Kotlin shape, or two declarations reach one
/// Kotlin spelling.
pub fn kotlin_items(ast: &Ast, applied: &Applied) -> Result<Vec<kotlin::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::KOTLIN),
            &kotlin_type_name(ast, id),
            start,
            &mut out,
        );
    }

    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "KotlinConst").is_none() {
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
            Some(target) => kotlin_value_type(ast, target, &node.name)?,
            None => kotlin::Type::Primitive(kotlin::Primitive::Int),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(kotlin::Decl::Const(kotlin::ConstDecl {
            doc: idl_note(&name, &node.name),
            // `const val` accepts only a primitive or `String` (Kotlin
            // language specification, "Const properties"); a `long
            // double` constant's Kotlin form is `Float128`, an
            // aggregate, and the only IDL constant kind that is not one
            // of the two — every other kind IDL 4.2 7.4.1.9 admits for a
            // constant already is.
            is_const: is_const_eligible(&type_),
            name: Ident::new(&name),
            type_: type_.clone(),
            value: kotlin_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "KotlinError").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut pairs = Vec::new();
        for member in &node.members {
            if let Ok(member_node) = ast.member(*member) {
                pairs.push((property_name(&member_node.name), member_node.name.clone()));
            }
        }
        no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
        let mut properties = Vec::new();
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
            properties.push(member_property(ast, member_node, target)?);
        }
        out.push(kotlin::Decl::Class(kotlin::Class {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line("An exception, as the class its members make."),
                &name,
                &node.name,
            ),
            is_data: !properties.is_empty(),
            name: Ident::new(&name),
            properties,
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "KotlinInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut properties = Vec::new();
        let mut signatures = Vec::new();
        for operation in &operations_with_bases(ast, id, &node.name)? {
            let Ok(op) = ast.operation(*operation) else {
                return Err(MaterialiseError::NoNode {
                    kind: "operation",
                    at: crate::adapter::cst::handle_of(*operation),
                });
            };
            signatures.push((property_name(&op.name), op.name.clone()));
            let mut params = Vec::new();
            for param in &op.params {
                let Ok(param_node) = ast.param(*param) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "parameter",
                        at: crate::adapter::cst::handle_of(*param),
                    });
                };
                let Some(target) = param_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "parameter type".to_string(),
                        at: param_node.name.clone(),
                    });
                };
                params.push(kotlin_member_type(ast, target, &param_node.name)?);
            }
            let returns = match op
                .return_type
                .as_ref()
                .and_then(zerodds_ast_core::Ref::resolved)
            {
                Some(target) => kotlin_member_type(ast, target, &op.name)?,
                None => kotlin::Type::Unit,
            };
            properties.push(kotlin::Property {
                doc: idl_note(&property_name(&op.name), &op.name),
                mutable: false,
                name: Ident::new(&property_name(&op.name)),
                type_: kotlin::Type::Function {
                    params,
                    returns: Box::new(returns),
                },
                default: None,
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(kotlin::Decl::Interface(kotlin::Class {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line(
                    "An interface, as the class of function-typed properties its operations make.",
                ),
                &name,
                &node.name,
            ),
            is_data: false,
            name: Ident::new(&name),
            properties,
        }));
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::KOTLIN, out))
}

/// `@verbatim` in the Kotlin column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` go into a class body written after the primary
/// constructor; an enum, an alias and an object take them adjacent.
impl verbatim::Carrier for kotlin::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Class(node) = self else {
            return false;
        };
        if node.name.as_str() != name {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// The Kotlin declarations, in a file.
///
/// # Errors
/// As [`kotlin_items`].
pub fn kotlin_file(ast: &Ast, applied: &Applied) -> Result<kotlin::File, MaterialiseError> {
    let decls = kotlin_items(ast, applied)?;
    Ok(kotlin::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "The IDL spelling of every renamed name is in the comment above it.",
            "Nothing is renamed for style — a rename here is always the keyword",
            "escape. A member's ID under @autoid(HASH) is the MD5 of its IDL name",
            "(DDS-XTypes 1.3 7.3.1.2.1.1) and the escape does not invert, which is",
            "why the comment is there.",
        ]),
        package: None,
        imports: Vec::new(),
        decls,
    })
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<kotlin::Decl>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // Written where the member is; none of these is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // A map's entry needs a name: Kotlin class identity is nominal,
        // so two anonymous entry shapes could not share one codec.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: kotlin_type_name(ast, id),
                });
            };
            let name = kotlin_map_entry_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            out.push(kotlin::Decl::Class(kotlin::Class {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::lines(&[
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a",
                    "sequence of these, and the rules write the two halves with no",
                    "framing between them.",
                ]),
                is_data: true,
                name: Ident::new(&name),
                properties: vec![
                    kotlin::Property {
                        doc: Doc::none(),
                        mutable: true,
                        name: Ident::new("key"),
                        type_: kotlin_member_type(ast, key, &name)?,
                        default: Some(zero_of(ast, key)?),
                    },
                    kotlin::Property {
                        doc: Doc::none(),
                        mutable: true,
                        name: Ident::new("value"),
                        type_: kotlin_member_type(ast, value, &name)?,
                        default: Some(zero_of(ast, value)?),
                    },
                ],
            }));
        }

        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "KotlinAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = kotlin_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            out.push(kotlin::Decl::Alias(kotlin::AliasDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                target: kotlin_member_type(ast, target, &node.name)?,
            }));
            // A typedef of a plain value gets a function of its own name
            // that answers its zero: `typealias m_LL = Int` has no
            // constructor, and a codec allocating an `Array<m_LL>` writes
            // `Array(n) { m_LL() }` (`resolve-typedef-chain-three`). A
            // typealias and a function live in separate namespaces.
            if let Some(zero) = crate::default_kotlin::alias_zero(ast, target) {
                out.push(kotlin::Decl::Func(kotlin::FuncDecl {
                    doc: Doc::none(),
                    name: Ident::new(&name),
                    params: Vec::new(),
                    returns: kotlin::Type::named(&name),
                    body: vec![kotlin::Stmt::Return(Some(zero))],
                }));
            }
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "KotlinEnum").is_none() {
                return Ok(());
            }
            let name = kotlin_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            for enumerator in enumerators {
                if let Ok(enum_node) = ast.enumerator(*enumerator) {
                    pairs.push((property_name(&enum_node.name), enum_node.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut members = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "enumerator",
                        at: crate::adapter::cst::handle_of(*enumerator),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "KotlinConstant").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let spelled = property_name(&enum_node.name);
                members.push(kotlin::ConstDecl {
                    doc: idl_note(&spelled, &enum_node.name),
                    is_const: true,
                    name: Ident::new(&spelled),
                    type_: kotlin::Type::Primitive(kotlin::Primitive::Int),
                    value: kotlin::Expr::int(value),
                });
            }
            out.push(kotlin::Decl::Object(kotlin::Object {
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration's constants. Not `enum class`: its ordinal is the",
                        "constant's position and DDS-XTypes 1.3 7.4.3.5.3 admits an",
                        "@value out of order or with a gap, which an ordinal cannot hold",
                        "without renumbering the wire.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                members,
            }));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "KotlinBitmask").is_none() {
                return Ok(());
            }
            // No declaration of its own: a bitmask's Kotlin form is the
            // signed integer wide enough for Table 33 of DDS-XTypes 1.3
            // 7.4.1.1.3.2's bound, held plainly — `kotlin_value_type`
            // answers `Int` or `Long` directly. A Kotlin `typealias`
            // would add nothing to the type system (it is transparent
            // to the compiler; `typealias Flags = Int` and `Int` are one
            // type) and would only give `crate::emit::kotlin::Kotlin`'s
            // `cast` a name it could not tell apart from a real
            // aggregate — which numeric conversion an alias wants is a
            // question only its *underlying* type answers, and the
            // alias itself carries none.
            let bound = ast.effective_bit_bound(id);
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "bit",
                        at: crate::adapter::cst::handle_of(*bit),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "KotlinBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = kotlin::escape_identifier(&format!("{prefix}{}", bit_node.name));
                remember(emitted, &constant, &bit_node.name)?;
                let holder = unsigned_holder(bound);
                // `1L shl n` where the holder is `Long`: Kotlin's `shl`
                // answers in its receiver's own width and does not
                // widen an `Int` result into the `Long` the constant is
                // declared, so the `1` carries the width itself.
                let one = if holder == kotlin::Primitive::Long {
                    kotlin::Expr::Literal(Literal::Int("1L".to_string()))
                } else {
                    kotlin::Expr::int(1)
                };
                out.push(kotlin::Decl::Const(kotlin::ConstDecl {
                    doc: idl_note(&constant, &bit_node.name),
                    is_const: true,
                    name: Ident::new(&constant),
                    type_: kotlin::Type::Primitive(holder),
                    value: kotlin::Expr::Binary {
                        op: "shl",
                        lhs: Box::new(one),
                        rhs: Box::new(kotlin::Expr::int(i128::from(position))),
                    },
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "KotlinBitset").is_none() {
                return Ok(());
            }
            let name = kotlin_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
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
                pairs.push((property_name(field_ident), field_ident.clone()));
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut properties = Vec::new();
            let mut index = 0;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "bitfield",
                        at: crate::adapter::cst::handle_of(*field),
                    });
                };
                if field_node.name.is_none() {
                    continue;
                }
                properties.push(kotlin::Property {
                    doc: idl_note(&pairs[index].0, &pairs[index].1),
                    mutable: true,
                    name: Ident::new(&pairs[index].0),
                    type_: kotlin::Type::Primitive(unsigned_holder(field_node.bits)),
                    default: Some(kotlin::Expr::int(0)),
                });
                index += 1;
            }
            out.push(kotlin::Decl::Class(kotlin::Class {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate properties here and not a",
                        "single packed word, because the order on the wire is the",
                        "declaration's and the packing into the anchor is the codec's.",
                    ]),
                    &name,
                    &node.name,
                ),
                is_data: !properties.is_empty(),
                name: Ident::new(&name),
                properties,
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "KotlinValue"
            } else {
                "KotlinStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = kotlin_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`.
            let members = ast.members_of(id).to_vec();
            let base = super::c::struct_base(ast, id);
            let mut pairs = Vec::new();
            if base.is_some() {
                pairs.push((property_name("parent"), "parent".to_string()));
            }
            for member in &members {
                if let Ok(member_node) = ast.member(*member) {
                    pairs.push((property_name(&member_node.name), member_node.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut properties = Vec::new();
            if let Some(base) = base {
                properties.push(kotlin::Property {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    mutable: true,
                    name: Ident::new(&property_name("parent")),
                    type_: kotlin_member_type(ast, base, "parent")?,
                    default: Some(zero_of(ast, base)?),
                });
            }
            for member in &members {
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
                properties.push(member_property(ast, member_node, target)?);
            }
            out.push(kotlin::Decl::Class(kotlin::Class {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                is_data: !properties.is_empty(),
                name: Ident::new(&name),
                properties,
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "KotlinUnion").is_none() {
                return Ok(());
            }
            let name = kotlin_type_name(ast, id);
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
                        pairs.push((property_name(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut properties = vec![kotlin::Property {
                doc: Doc::line("Which branch carries the value."),
                mutable: true,
                name: Ident::new(DISCRIMINATOR),
                type_: kotlin_member_type(ast, disc, &name)?,
                default: Some(zero_of(ast, disc)?),
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "case",
                        at: crate::adapter::cst::handle_of(*case),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "KotlinCase").is_none() {
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
                properties.push(member_property(ast, member, target)?);
            }
            out.push(kotlin::Decl::Class(kotlin::Class {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union, as a data class carrying the discriminator beside every",
                        "branch. Only the branch the discriminator selects is written",
                        "(DDS-XTypes 1.3 7.4.1.1.5.2); the codec's business, not the",
                        "type's.",
                    ]),
                    &name,
                    &node.name,
                ),
                is_data: true,
                name: Ident::new(&name),
                properties,
            }));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "KotlinNative").is_none() {
                return Ok(());
            }
            let name = kotlin_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(kotlin::Decl::Alias(kotlin::AliasDecl {
                doc: with_idl_note(
                    Doc::line("A native type. `Any` unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                target: match mapping {
                    Some(spelling) => kotlin::Type::named(&spelling),
                    None => kotlin::Type::named("Any"),
                },
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = kotlin_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            out.push(kotlin::Decl::Alias(kotlin::AliasDecl {
                doc: with_idl_note(
                    Doc::line("A boxed value: nullable, `null` when absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                target: kotlin::Type::nullable(kotlin_member_type(ast, target, &name)?),
            }));
        }

        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// Every operation an interface has: its bases' first, in declaration
/// order, then its own.
///
/// [`Ast::operations_of`] answers what an interface *declares*, and this
/// column writes no supertype — the class it emits carries one property
/// per operation and nothing else — so an operation not written here is
/// an operation the generated file does not have. `declare`'s structure
/// arm already takes [`Ast::all_members_of`], the inherited members
/// included, for that same reason; an interface is flattened the same
/// way. CORBA resolves an operation along the base list in declaration
/// order (IDL 4.2 §7.4.3), which is the order they are collected in.
///
/// An unresolved base is refused rather than skipped: skipping it drops
/// every operation the base declares and says nothing.
///
/// zerodds-lint: recursion-depth 64 (base chains; bounded by `MAX_INTERFACE_DEPTH`)
fn operations_with_bases(
    ast: &Ast,
    id: Id<zerodds_ast::InterfaceNode>,
    at: &str,
) -> Result<Vec<Id<zerodds_ast::OperationNode>>, MaterialiseError> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    collect_operations(ast, id, at, &mut seen, &mut out)?;
    Ok(out)
}

/// The walk behind [`operations_with_bases`]. A base already collected
/// is not walked again: an inheritance cycle is invalid IDL and looping
/// here would hang instead of reporting it, and the diamond CORBA
/// multiple inheritance admits would otherwise declare one operation
/// twice — which [`no_spelling_collision`] would then refuse.
///
/// zerodds-lint: recursion-depth 64 (interface inheritance; bounded by
/// `MAX_INTERFACE_DEPTH` and by `seen`, which admits each interface
/// once — an inheritance cycle is invalid IDL and stops here on the
/// second visit rather than hanging)
fn collect_operations(
    ast: &Ast,
    id: Id<zerodds_ast::InterfaceNode>,
    at: &str,
    seen: &mut Vec<Id<zerodds_ast::InterfaceNode>>,
    out: &mut Vec<Id<zerodds_ast::OperationNode>>,
) -> Result<(), MaterialiseError> {
    if seen.contains(&id) || seen.len() >= MAX_INTERFACE_DEPTH {
        return Ok(());
    }
    seen.push(id);
    let Ok(node) = ast.interface(id) else {
        return Err(MaterialiseError::NoNode {
            kind: "interface",
            at: crate::adapter::cst::handle_of(id),
        });
    };
    for base in &node.bases {
        let Some(base_id) = base.resolved() else {
            return Err(MaterialiseError::Unevaluated {
                construct: "base interface".to_string(),
                at: at.to_string(),
            });
        };
        collect_operations(ast, base_id, at, seen, out)?;
    }
    out.extend(ast.operations_of(id).iter().copied());
    Ok(())
}

/// The Kotlin spelling of a value's type, refusing what Kotlin has no
/// runtime form for: `any`, `ValueBase` and an object reference — the
/// same three [`crate::adapter::zig::zig_member_type`] refuses, and for
/// the same reason (that module's documentation gives it in full).
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn kotlin_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<kotlin::Type, MaterialiseError> {
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
    kotlin_value_type(ast, target, at)
}

/// The Kotlin type a value of `id` is held in.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the type, an array length or a
/// collection's element does not resolve.
pub fn kotlin_value_type(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
) -> Result<kotlin::Type, MaterialiseError> {
    value_type_at(ast, id, at, 0)
}

/// [`kotlin_value_type`], carrying how many connector steps the walk has
/// already taken.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the spelling nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and
/// anonymous-alias nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`). An aggregate is a leaf here — it
/// is named, not descended into — so only a cycle among the connectors
/// can come back round, and the counter is what stops that rather than
/// the frontend happening to build inline types as a tree.
fn value_type_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<kotlin::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Kotlin spelling nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "type behind a member".to_string(),
            at: at.to_string(),
        });
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => kotlin_primitive_type(*kind),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => kotlin::Type::named("String"),
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            kotlin::Type::array(value_type_at(ast, element, at, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            let mut type_ = value_type_at(ast, element, at, depth + 1)?;
            for _ in dims.iter() {
                type_ = kotlin::Type::array(type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            kotlin::Type::array(kotlin::Type::named(&kotlin_map_entry_name(ast, id)))
        }
        // No declaration backs a bitmask's name — see `declare`'s
        // Bitmask arm — so its codec-visible type is the holder itself.
        TypeKind::Bitmask { .. } => {
            kotlin::Type::Primitive(unsigned_holder(ast.effective_bit_bound(id)))
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
        // An enumeration's value is a plain Int: the module documentation
        // says why the type is not a Kotlin `enum class`.
        TypeKind::Enumeration { .. } => kotlin::Type::Primitive(kotlin::Primitive::Int),
        _ => kotlin::Type::named(&kotlin_type_name(ast, id)),
    })
}

/// The Kotlin type an IDL primitive becomes. Exhaustive: a primitive
/// added to the model without a Kotlin spelling is a compile error.
#[must_use]
pub fn kotlin_primitive_type(kind: PrimitiveKind) -> kotlin::Type {
    use kotlin::{Primitive, Type};
    match kind {
        PrimitiveKind::Boolean => Type::Primitive(Primitive::Boolean),
        PrimitiveKind::Char => Type::Primitive(Primitive::Char),
        PrimitiveKind::Int8 => Type::Primitive(Primitive::Byte),
        PrimitiveKind::Int16 => Type::Primitive(Primitive::Short),
        // No unsigned type: the next signed width up, exactly as
        // `adapter/java.rs::scalar` decides for the JVM value model.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::WChar
        | PrimitiveKind::UInt16 => Type::Primitive(Primitive::Int),
        PrimitiveKind::Int32 => Type::Primitive(Primitive::Int),
        PrimitiveKind::UInt32 | PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
            Type::Primitive(Primitive::Long)
        }
        PrimitiveKind::Float32 => Type::Primitive(Primitive::Float),
        PrimitiveKind::Float64 => Type::Primitive(Primitive::Double),
        PrimitiveKind::Float128 => Type::named("Float128"),
    }
}

/// The signed integer a bit width calls for: `Int` up to 16 bits, `Long`
/// past it — the same answer `adapter/java.rs::unsigned_holder` gives,
/// because the JVM has neither an unsigned type nor one narrower than
/// `Int` that this column's codec assembles a value in.
///
/// The boundary is at 16 and not at 32 because the value is unsigned and
/// the holder is not. A `@bit_bound(32)` bitmask with bit 31 set —
/// DDS-XTypes 1.3 Table 33 gives that bound a 32-bit unsigned value —
/// sits in an `Int` as a negative pattern, and the widening to the
/// `Long` the codec computes in sign-extends it into a value the
/// runtime's own range check refuses. A `Long` holder carries the same
/// thirty-two bits with no sign to extend.
#[must_use]
pub const fn unsigned_holder(bits: u16) -> kotlin::Primitive {
    if bits <= 16 {
        kotlin::Primitive::Int
    } else {
        kotlin::Primitive::Long
    }
}

/// A declared type's Kotlin identifier: the scoped name flattened with
/// `_`, escaped. No case is changed — this column has no export-by-case
/// rule and a rename here would be a rename with no cause.
#[must_use]
pub fn kotlin_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "ZdUnnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => flatten(scoped.segments()),
        None => "ZdUnnamed".to_string(),
    }
}

/// The entry class a `map<K,V>` needs a name for. Structural, built from
/// the IDL kind and not the Kotlin spelling, for the reason
/// [`crate::adapter::zig::zig_map_entry_name`] gives.
#[must_use]
pub fn kotlin_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(node) = ast.type_(id) else {
        return format!("{INVENTED_PREFIX}unknown");
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return format!("{INVENTED_PREFIX}unknown");
    };
    let key = key
        .resolved()
        .map_or_else(|| "unknown".to_string(), |k| type_tag(ast, k, 0));
    let value = value
        .resolved()
        .map_or_else(|| "unknown".to_string(), |v| type_tag(ast, v, 0));
    format!("{INVENTED_PREFIX}{key}_{value}")
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag also names the routine a hoisted collection's codec is
/// emitted under, and one body is emitted per name. With the bound left
/// out, `sequence<long>` and `sequence<long, 4>` are one name and
/// whichever was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Kotlin held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both got
/// the bounded body, so `b` refused — and truncated — a length the IDL
/// puts no limit on. `string<4>` beside `string` and `wstring<4>`
/// beside `wstring` collapse the same way, onto the trimming call.
/// Either way the reader answered with a value the IDL does not
/// describe and said nothing, which is why the bound is part of the
/// name. The same suffix the Zig column carries.
///
/// An `array` needs nothing here: its `dims` are already in the tag,
/// under the `Array` arm below.
fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `seq_long_4` is what the `Array`
        // arm answers for an array of four `long`, and the two are
        // different wire forms.
        Some(expr) => match ast.eval_integral(expr) {
            Ok(limit) => format!("_b{limit}"),
            // As the array's `_x`: a bound that does not evaluate is
            // still not the unbounded form, and the materialiser
            // reports the expression that would not evaluate.
            Err(_) => "_bx".to_string(),
        },
    }
}

/// The name component a map entry or a codec function takes from its
/// key, value or element: the IDL kind, structurally, so that two
/// identical anonymous collections collapse onto one declaration.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
#[must_use]
pub fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "unresolved".to_string();
    }
    let node = match ast.type_(id) {
        Ok(node) => node,
        Err(_) => return "unresolved".to_string(),
    };
    match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => format!("string{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => format!("wstring{}", bound_tag(ast, bound)),
        TypeKind::Fixed { digits, scale } => format!("fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "seq{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, element, depth + 1)
            ),
            None => format!("seq_unresolved{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", type_tag(ast, element, depth + 1)),
                None => "arr_unresolved".to_string(),
            };
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => name.push_str(&format!("_{length}")),
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        // The entry class's own name carries no bound — two maps that
        // differ only in bound hold the same pairs — but the tag does,
        // for the routine-name reason in [`bound_tag`].
        TypeKind::Map { key, value, bound } => {
            let half = |half: Option<Id<TypeNode>>| match half {
                Some(target) => type_tag(ast, target, depth + 1),
                None => "unresolved".to_string(),
            };
            format!(
                "map{}_{}_{}",
                bound_tag(ast, bound),
                half(key.resolved()),
                half(value.resolved())
            )
        }
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => type_tag(ast, target, depth + 1),
            None => "unresolved".to_string(),
        },
        TypeKind::ValueBox { boxed } => match boxed.resolved() {
            Some(target) => format!("box_{}", type_tag(ast, target, depth + 1)),
            None => "box_unresolved".to_string(),
        },
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => kotlin_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
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

/// The Kotlin name a declaration outside the type arena takes.
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
    kotlin::escape_identifier(&segments.join("_"))
}

/// Join scoped segments with `_` and escape the result. Not injective —
/// an IDL identifier may itself hold `_` — which is why [`remember`]
/// checks every declared name against every other one in the file.
fn flatten(segments: &[String]) -> String {
    kotlin::escape_identifier(&segments.join("_"))
}

/// The Kotlin spelling of a member, an enumerator, a bitfield or an
/// operation. Only the keyword escape: Kotlin's soft keywords parse as
/// identifiers everywhere the grammar does not specifically want one, so
/// a member named `data` needs none.
#[must_use]
pub fn property_name(name: &str) -> String {
    kotlin::escape_identifier(name)
}

/// The property one member declaration becomes.
///
/// `@external` needs no indirection — the module documentation gives the
/// reason — so only `@optional`'s nullability changes the type here.
fn member_property(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<kotlin::Property, MaterialiseError> {
    let spelled = property_name(&member.name);
    let mut doc = idl_note(&spelled, &member.name);
    let mut type_ = kotlin_member_type(ast, target, &member.name)?;
    if is_external(member) {
        doc.push("Stored by reference — @external, DDS-XTypes 1.3 Table 21. Every");
        doc.push("JVM object field already is one, so nothing further is needed.");
    }
    let default = if member.facets.is_optional() {
        type_ = kotlin::Type::nullable(type_);
        doc.push("Optional. `null` when the member is absent — DDS-XTypes 1.3");
        doc.push("7.2.2.4.4.4.7.");
        kotlin::Expr::Literal(Literal::Null)
    } else {
        zero_of(ast, target)?
    };
    Ok(kotlin::Property {
        doc,
        mutable: true,
        name: Ident::new(&spelled),
        type_,
        default: Some(default),
    })
}

/// Whether a member is annotated `@external`.
fn is_external(member: &zerodds_ast::MemberNode) -> bool {
    member
        .facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_external)
}

/// A resting value for a primary-constructor property, so that
/// `crate::emit::kotlin`'s `storage` can call every generated class's
/// zero-argument constructor.
///
/// This is *not* Table 9's default — that is
/// `crate::default_kotlin::member_default`'s answer, computed once per
/// decoder and written by the generated codec. This is a placeholder any
/// value of the type accepts, chosen only so the constructor compiles;
/// every field a codec cares about is assigned before it is read.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn zero_of(ast: &Ast, id: Id<TypeNode>) -> Result<kotlin::Expr, MaterialiseError> {
    zero_of_depth(ast, id, 0)
}

/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_TYPE_DEPTH`, past which the resting value is a
/// plain `0` — nothing a codec reads, as the doc on [`zero_of`] says)
fn zero_of_depth(
    ast: &Ast,
    id: Id<TypeNode>,
    depth: usize,
) -> Result<kotlin::Expr, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Ok(kotlin::Expr::int(0));
    }
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "the type a resting value is built for".to_string(),
            at: crate::adapter::cst::handle_of(id),
        });
    };
    Ok(match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Boolean) => kotlin::Expr::Literal(Literal::Bool(false)),
        TypeKind::Primitive(PrimitiveKind::Char) => kotlin::Expr::Literal(Literal::Char('\u{0}')),
        TypeKind::Primitive(PrimitiveKind::Float32) => {
            kotlin::Expr::Literal(Literal::Float("0.0f".to_string()))
        }
        TypeKind::Primitive(PrimitiveKind::Float64) => {
            kotlin::Expr::Literal(Literal::Float("0.0".to_string()))
        }
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            kotlin::Expr::method_call(kotlin::Expr::name("Float128"), "zero", Vec::new())
        }
        TypeKind::Primitive(
            PrimitiveKind::UInt32 | PrimitiveKind::Int64 | PrimitiveKind::UInt64,
        ) => kotlin::Expr::Literal(Literal::Int("0L".to_string())),
        TypeKind::Primitive(_) | TypeKind::Enumeration { .. } => kotlin::Expr::int(0),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            kotlin::Expr::Literal(Literal::Str(String::new()))
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element behind a resting value".to_string(),
                    at: named_at(node, id),
                });
            };
            // An empty sequence has no element to initialize. NewArray
            // requires an element constructor even at length zero, which
            // is invalid for aliases, nested arrays and Float128.
            kotlin::Expr::ArrayOf {
                element: Box::new(kotlin_value_type(ast, element, "sequence default")?),
                elements: Vec::new(),
            }
        }
        // Every dimension is evaluated and refused where it does not,
        // rather than folded to a zero. A resting value is what an
        // encoder reads out of a sample nobody assigned to, and an
        // `arrayOf()` where `T[5]` belongs writes five elements fewer
        // than the declaration states — the empty array is a legal
        // Kotlin value of the declared `Array<T>` type, so nothing
        // downstream can tell it from a length the source asked for.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element behind a resting value".to_string(),
                    at: named_at(node, id),
                });
            };
            let mut expr = zero_of_depth(ast, element, depth + 1)?;
            let mut type_ = kotlin_value_type(ast, element, "array default")?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: named_at(node, id),
                        })?;
                let count = usize::try_from(length).map_err(|_| MaterialiseError::Unevaluated {
                    construct: "array length this column cannot spell".to_string(),
                    at: named_at(node, id),
                })?;
                expr = kotlin::Expr::ArrayOf {
                    element: Box::new(type_.clone()),
                    elements: vec![expr; count],
                };
                type_ = kotlin::Type::array(type_);
            }
            expr
        }
        TypeKind::Map { .. } => kotlin::Expr::ArrayOf {
            element: Box::new(kotlin::Type::named(&kotlin_map_entry_name(ast, id))),
            elements: Vec::new(),
        },
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => zero_of_depth(ast, target, depth + 1)?,
            None => kotlin::Expr::int(0),
        },
        TypeKind::ValueBox { .. } => kotlin::Expr::Literal(Literal::Null),
        TypeKind::Bitmask { .. } => kotlin::Expr::int(0),
        _ => kotlin::Expr::New {
            type_: Box::new(kotlin::Type::named(&kotlin_type_name(ast, id))),
            args: Vec::new(),
        },
    })
}

/// A type's own name where it has one, and its arena handle where it
/// does not. An anonymous array or sequence carries the empty string,
/// and a diagnostic that points at nothing names nothing.
fn named_at(node: &TypeNode, id: Id<TypeNode>) -> String {
    if node.name.is_empty() {
        return crate::adapter::cst::handle_of(id);
    }
    node.name.clone()
}

/// The declaration's own IDL name, where the Kotlin spelling moved it.
fn idl_note(spelled: &str, declared: &str) -> Doc {
    if spelled == declared {
        return Doc::none();
    }
    Doc::line(&format!("IDL: `{declared}`."))
}

fn with_idl_note(mut doc: Doc, spelled: &str, declared: &str) -> Doc {
    if spelled != declared {
        doc.push(&format!("IDL: `{declared}`."));
    }
    doc
}

/// Record a declared name, refusing a second declaration that reaches
/// the same Kotlin spelling.
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

/// Whether Kotlin accepts `type_` as a `const val` — a primitive or
/// `String` (Kotlin language specification, "Const properties"). Only
/// `Float128` (an IDL `long double`) among the shapes a constant may
/// take is neither; `kotlin_items` writes `val` there instead.
fn is_const_eligible(type_: &kotlin::Type) -> bool {
    match type_ {
        kotlin::Type::Primitive(_) => true,
        kotlin::Type::Named { path, args } => {
            args.is_empty() && path.segments().len() == 1 && path.segments()[0].as_str() == "String"
        }
        _ => false,
    }
}

/// The Kotlin expression a constant's value takes.
fn kotlin_constant(
    value: &zerodds_ast::ConstValue,
    type_: &kotlin::Type,
    at: &str,
) -> Result<kotlin::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_wide = matches!(type_, kotlin::Type::Primitive(kotlin::Primitive::Long));
    let is_float32 = matches!(type_, kotlin::Type::Primitive(kotlin::Primitive::Float));
    let is_float128 = *type_ == kotlin::Type::named("Float128");
    Ok(match value {
        ConstValue::Boolean(flag) => kotlin::Expr::Literal(Literal::Bool(*flag)),
        // Through `long_text` and not a bare `{number}L`: Kotlin has no
        // negative integer literal, so `-9223372036854775808L` asks for
        // a literal one past `Long.MAX_VALUE` and kotlinc 2.4.0 answers
        // "value out of range" — measured, and an error rather than a
        // warning, in a `const val` initialiser as well as in an
        // assignment. `zerodds_lang_render::kotlin::long_text` writes
        // that one value as `Long.MIN_VALUE`, which is itself a
        // `const val` and so stands in a `const val` initialiser.
        ConstValue::Integer(number) => {
            if is_wide {
                let text = if *number == i128::from(i64::MIN) {
                    zerodds_lang_render::kotlin::long_text(i64::MIN)
                } else {
                    format!("{number}L")
                };
                kotlin::Expr::Literal(Literal::Int(text))
            } else {
                kotlin::Expr::Literal(Literal::int(*number))
            }
        }
        ConstValue::Char(character) => kotlin::Expr::Literal(Literal::Char(*character)),
        ConstValue::WChar(point) => kotlin::Expr::int(i128::from(*point)),
        // Kotlin's integer-literal inference does not extend to
        // floating-point ones (`val x: Float = 0.5` refuses —
        // "the floating-point literal does not conform to the expected
        // type Float"), so a `float` constant carries its own `f` suffix
        // rather than relying on the declared type to narrow it. A
        // `long double` constant is `Float128.parse(".."`)`: `const val`
        // admits only a primitive or `String` (Kotlin language
        // specification, "Const properties"), and `Float128` is
        // neither, so this is a plain `val` evaluated once at load —
        // `is_const_eligible` is what tells [`kotlin_items`] to declare
        // it that way.
        ConstValue::Float { mantissa, exponent } => {
            let text = crate::adapter::constant::float_text(mantissa, exponent.as_deref());
            if is_float128 {
                kotlin::Expr::method_call(
                    kotlin::Expr::name("Float128"),
                    "parse",
                    vec![kotlin::Expr::Literal(Literal::Str(text))],
                )
            } else if is_float32 {
                kotlin::Expr::Literal(Literal::Float(format!("{text}f")))
            } else {
                kotlin::Expr::Literal(Literal::Float(text))
            }
        }
        ConstValue::Fixed { digits, scale } => kotlin::Expr::Literal(Literal::Str(
            crate::adapter::constant::fixed_decimal(digits, *scale),
        )),
        ConstValue::String(text) => kotlin::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => kotlin::Expr::Literal(Literal::Str(
            crate::adapter::constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )),
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
