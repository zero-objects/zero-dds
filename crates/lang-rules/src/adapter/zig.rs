// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Zig CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::go`] and [`crate::adapter::c`].
//! Reading the three side by side is the point: all three flatten a
//! module onto the name, and everything else they do differently is
//! their language's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The memory shapes
//!
//! | IDL | Zig |
//! |---|---|
//! | `boolean` | `bool` |
//! | `octet`, `uint8`, `char` | `u8` |
//! | `int8` | `i8` |
//! | `short` | `i16` |
//! | `unsigned short`, `wchar` | `u16` |
//! | `long` | `i32` |
//! | `unsigned long` | `u32` |
//! | `long long` | `i64` |
//! | `unsigned long long` | `u64` |
//! | `float` | `f32` |
//! | `double` | `f64` |
//! | **`long double`** | **`f128`** |
//! | `string` | `[]const u8` |
//! | `wstring` | `[]const u8`, holding UTF-8 |
//! | `fixed<P,S>` | `[]const u8`, holding the decimal |
//! | `sequence<T>` | `[]T` |
//! | `T[N][M]` | `[N][M]T`, outer dimension outermost |
//! | `map<K,V>` | `[]ZdMapEntry_K_V`, **not** `std.AutoHashMap` |
//! | `@optional T m` | `?T`, `null` when absent |
//! | `@external T m` | `*T` — the pointer is the indirection |
//! | `@optional @external T m` | `?*T`, the pointer inside the optional |
//! | `struct`, `valuetype`, `exception`, `bitset` | `struct { … }` |
//! | `union` | `struct { _d: D, … }`, one field per branch |
//! | `enum` | `enum(i32) { … }`, every value written out |
//! | `bitmask` | an unsigned integer and one constant per bit |
//! | `typedef` | `pub const Distance = i32;` |
//! | `native` | `*anyopaque`, or the facet's mapping |
//! | `interface` | `struct` of function pointers |
//!
//! ### `long double` is an `f128`, and this is the only column where it
//! ### can be
//!
//! Zig has a real binary128. `f128` is an IEEE 754 quadruple, 113 bits
//! of significand, which is exactly what an IDL `long double` is
//! (IDL 4.2 §7.4.1.4.4.2.2 takes the IEEE 754 double-extended/quadruple
//! form). Zig's own `comptime_float` is evaluated at that width, so a
//! decimal literal in the generated source is not rounded on its way in
//! either.
//!
//! Of the ten language columns this is the one whose built-in set
//! reaches it. The Go column holds the decimal in a `string` and says so
//! ("Go has no binary128 and no extended type at all"); the Python
//! column does the same. Both are honest workarounds for a language that
//! cannot hold the number; Zig does not need one, and a column that
//! copied the workaround would be throwing away the language's own
//! answer.
//!
//! ### A union is a struct with a discriminator, not a `union(enum)`
//!
//! Zig has a tagged union and it is the better Zig form for a sum type,
//! so this is the decision that has to be argued rather than assumed.
//!
//! It is not used because a Zig tagged union's tag *is* its active
//! field: `@as(Tag, value)` is defined for every value of the union, and
//! there is no state in which the tag holds something and no field is
//! live. An IDL union's discriminator is a separate value with its own
//! type, and IDL 4.2 §7.4.1.4.4.2.3 lets it take a value no `case` label
//! selects. When it does and the union declares no `default`, the union
//! carries **no** branch value at all and only the discriminator goes on
//! the wire (DDS-XTypes 1.3 §7.4.1.1.5.2). A `union(enum)` cannot be
//! constructed in that state, so a decoder reading such a value off the
//! wire would have nowhere to put it — and the shapes that would make it
//! fit, an extra `_unset` tag or an `?T` payload, are both a different
//! type from the one the IDL declared.
//!
//! The struct form holds it exactly: `_d` carries the discriminator's
//! own value, whatever it is, and the branch fields carry whatever the
//! last decode left. Only the selected branch is written
//! (DDS-XTypes 1.3 §7.4.1.1.5.2), which is the codec's business and not
//! the type's.
//!
//! ### `map<K,V>` is a slice of entries and never a hash map
//!
//! A `map<K,V>` goes on the wire as a sequence of key/value pairs
//! (DDS-XTypes 1.3 §7.4.1.1.4), which is ordered. `std.AutoHashMap` and
//! `std.StringHashMap` iterate in bucket order, which follows the hash
//! and the insertion history and is not the order the pairs arrived in;
//! a column holding a map that way would produce a different octet
//! stream for one value depending on how it was built, and the KeyHash
//! of a keyed type would move with it. The entry struct is declared
//! rather than written anonymously at each use so that a codec has a
//! name to take.
//!
//! ### A bitmask is an integer and one constant per bit, not a
//! ### `packed struct`
//!
//! Zig's `packed struct` of `bool` fields is the tempting form and it is
//! wrong here. Its fields are laid out in declaration order from the
//! least significant bit of its backing integer, which is a property of
//! the *declaration*; a bitmask's bits carry `@position`, which is a
//! property of the **wire** (DDS-XTypes 1.3 §7.4.1.1.3.2) and admits
//! gaps and any order. A `packed struct` would therefore need padding
//! fields to reproduce a sparse bitmask, and the mapping from a name to
//! its bit would be implicit in the field order rather than written
//! down. The integer plus `1 << position` says the position and nothing
//! else.
//!
//! ### Where the IDL spelling of a name is kept
//!
//! Nothing is renamed for style here — Zig has no export-by-case rule
//! and this column changes no letter's case. The only rename is the
//! escape: [`zig::escape_field`] appends an underscore to a Zig
//! **keyword** used as a field, and [`zig::escape_decl`] does the same
//! for a keyword or a **primitive name** used as a declaration, because
//! Zig refuses `pub const u8 = …` outright with
//! `name shadows primitive 'u8'`.
//!
//! Neither inverts: `type_` could have come from `type` or from `type_`.
//! DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's ID under
//! `@autoid(HASH)` and under `@hashid` from the first four octets of the
//! MD5 of its **IDL** name, so a file that only knows the Zig spelling
//! cannot compute the ID for the member it is writing. Every name the
//! escape moved therefore carries `IDL: \`…\`.` in the documentation
//! comment above it. Zig has no `reflect.StructTag`, so the comment is
//! where it goes — as in the C column and unlike Go's.
//!
//! ### Why every declaration is `pub`
//!
//! A Zig file is a struct and `pub` is what lets an importer name a
//! declaration in it (Zig language reference, "import"). A generated
//! types file exists to be imported, so a private declaration in one
//! would have no consumer at all.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::zig;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Zig";

/// The prefix a name this column invented carries.
///
/// Only a map entry needs one — every other Zig shape is written where
/// the member is. The prefix is also what keeps the invented name clear
/// of Zig's primitives, which a declaration may not shadow.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// The Zig field name of a union's discriminator.
///
/// `_d`, as in every column but Go's. Go had to spell it `D` because Go
/// cannot export a name beginning with an underscore; Zig's visibility
/// is the `pub` keyword and has nothing to do with the first character,
/// so the spelling the other columns use is kept. IDL 4.2 §7.4.1.4.4.1
/// gives the discriminator no member name of its own, which is why this
/// is stated here rather than assumed.
pub const DISCRIMINATOR: &str = "_d";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Zig spelling may nest before the walk refuses
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

/// Build the Zig declarations a rule application produced.
///
/// The order is the model's own declaration order. Zig resolves a
/// declaration anywhere in the file's container regardless of where it
/// is written (Zig language reference, "struct"), so — as in Go and
/// unlike C — there is no dependency walk and the output tracks the
/// source.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Zig shape, or two declarations reach one Zig
/// spelling — never a silently dropped declaration.
pub fn zig_items(ast: &Ast, applied: &Applied) -> Result<Vec<zig::Decl>, MaterialiseError> {
    Ok(verbatim::weave_file(
        ast,
        verbatim::aliases::ZIG,
        declarations(ast, applied)?,
    ))
}

/// [`zig_items`] without the file-level `@verbatim` text, which
/// [`zig_file`] places after its analysis block: `END_FILE` is the end of
/// the file.
fn declarations(ast: &Ast, applied: &Applied) -> Result<Vec<zig::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::ZIG),
            &zig_type_name(ast, id),
            start,
            &mut out,
        );
    }

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "ZigConst").is_none() {
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
            Some(target) => zig_value_type(ast, target, &node.name)?,
            None => zig::Type::Primitive(zig::Primitive::I32),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(zig::Decl::Const(zig::ConstDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: idl_note(&name, &node.name),
            is_pub: true,
            name: Ident::new(&name),
            type_: Some(type_.clone()),
            value: zig_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "ZigError").is_none() {
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
            fields.push(member_field(ast, member_node, target)?);
        }
        out.push(zig::Decl::Const(zig::ConstDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line("An exception, as the structure its members make."),
                &name,
                &node.name,
            ),
            is_pub: true,
            name: Ident::new(&name),
            type_: None,
            value: zig::Expr::type_(zig::Type::Struct(fields)),
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "ZigInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut fields = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. Zig has no
        // inheritance at all, so a derived interface's struct has no base
        // struct to reach OMG IDL 4.2 §7.4.3's inherited operations and
        // attributes through: a field left out here is one the
        // generated file does not have.
        // `interface::all_operations_flattened` gives both — the
        // get/set pair DDS-RPC 1.0 §7.5.1.1.3 implies for each
        // attribute, besides the declared operations — which
        // `Ast::all_operations_of` alone never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            // An attribute's own name for the getter field, `set_` plus
            // it for the setter's — snake_case, Zig's own field
            // convention, rather than `all_operations_flattened`'s
            // `get_`/`set_` prefix repeating DDS-RPC 1.0's implied-
            // operation name (`get_memory`, not `memory`).
            let field_ident = match op.origin {
                interface::Origin::AttributeGetter => field_name(op.attribute_name()),
                interface::Origin::AttributeSetter => {
                    format!("set_{}", field_name(op.attribute_name()))
                }
                interface::Origin::Declared => field_name(&op.name),
            };
            signatures.push((field_ident.clone(), op.name.clone()));
            let mut params = Vec::new();
            for param in &op.params {
                params.push(zig::Param {
                    // A parameter name in a Zig function *type* is
                    // ignored by the compiler and read by a person, so
                    // it keeps the IDL spelling with only the keyword
                    // escape applied.
                    name: Some(Ident::new(&zig::escape_field(&param.name))),
                    type_: zig_member_type(ast, param.type_, &param.name)?,
                });
            }
            let returns = match op.returns {
                Some(target) => zig_member_type(ast, target, &op.name)?,
                None => zig::Type::Primitive(zig::Primitive::Void),
            };
            let mut doc = idl_note(&field_ident, &op.name);
            if let Some(line) = op.raises_line() {
                doc.push(&line);
            }
            fields.push(zig::Field {
                doc,
                name: Ident::new(&field_ident),
                // `*const fn` and not `fn`: a bare function type in Zig
                // has an unknown size and cannot be a struct field, so
                // the field is a pointer to one.
                type_: zig::Type::const_pointer(zig::Type::Fn {
                    params,
                    returns: Box::new(returns),
                }),
                default: None,
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(zig::Decl::Const(zig::ConstDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface, as the struct of function pointers its operations",
                    "and attributes make — an attribute a `name`/`set_name` pair,",
                    "`readonly` only the first. No `callconv` is written: measured on",
                    "Zig 0.14.1, a `callconv(.C)` function refuses a parameter of a",
                    "generated aggregate — \"only extern structs and ABI sized packed",
                    "structs are extern compatible\" — and every aggregate here holds",
                    "slices and optionals, which no extern struct may. A derived",
                    "interface's bases are flattened in — Zig has no inheritance — so",
                    "this is every field the interface has, own and inherited. The",
                    "transport is the RPC arc's business; the shape is the IDL's.",
                    "`raises`/`getraises`/`setraises` are documented on the field they",
                    "belong to — Zig's own error unions carry no exception data, only",
                    "a tag, and DDS-RPC 1.0's `raises` names a struct.",
                ]),
                &name,
                &node.name,
            ),
            is_pub: true,
            name: Ident::new(&name),
            type_: None,
            value: zig::Expr::type_(zig::Type::Struct(fields)),
        }));
    }

    Ok(out)
}

/// The Zig declarations, in a file that carries its own analysis block.
///
/// A caller that only wants the declarations uses [`zig_items`]; this is
/// what a generator writes to disk.
///
/// The file ends in [`zig::AnalyseAll`], and that is not decoration.
/// Zig analyses a container's declarations only when something reaches
/// them: measured on Zig 0.14.1, a file holding nothing but
/// `pub const X: u8 = 300;` passes `zig build-obj` with status 0. A
/// generated types file that no compilation references is otherwise
/// checked for syntax and for nothing else, so the block is what makes
/// `zig test` over it a semantic check.
///
/// # Errors
/// As [`zig_items`].
pub fn zig_file(ast: &Ast, applied: &Applied) -> Result<zig::File, MaterialiseError> {
    let mut decls = declarations(ast, applied)?;
    decls.push(zig::Decl::AnalyseAll(zig::AnalyseAll {
        doc: Doc::lines(&[
            "Zig analyses a container's declarations only when something reaches",
            "them, so a generated types file that nothing imports is checked for",
            "syntax and for nothing else. This block reaches all of them, which is",
            "what makes `zig test` over this file a semantic check. It costs a",
            "normal build nothing: a test block is not compiled outside a test",
            "build.",
        ]),
    }));
    Ok(zig::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "The IDL spelling of every renamed name is in the comment above it.",
            "Nothing is renamed for style — Zig has no export-by-case rule — so a",
            "rename here is always an escape: a Zig keyword used as a field, or a",
            "keyword or primitive name used as a declaration. A member's ID under",
            "@autoid(HASH) is the MD5 of its IDL name (DDS-XTypes 1.3 7.3.1.2.1.1)",
            "and the escape does not invert, which is why the comment is there.",
        ]),
        decls: verbatim::weave_file(ast, verbatim::aliases::ZIG, decls),
    })
}

/// `@verbatim` in the Zig column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close the body of
/// `const Name = struct { … }`; an enum, a constant and an alias take
/// them adjacent.
impl verbatim::Carrier for zig::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Const(node) = self else {
            return false;
        };
        let is_struct = matches!(&node.value, zig::Expr::Type(type_) if matches!(**type_, zig::Type::Struct(_)));
        if node.name.as_str() != name || !is_struct {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<zig::Decl>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence and an array are
        // all written where the member is: Zig spells each of them
        // inline, so none of them is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // The one collection that does need a name. Zig's struct
        // identity is nominal, not structural: two anonymous
        // `struct { key: K, value: V }` written at two uses are two
        // different types, so a codec taking one could not be handed
        // the other.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: zig_type_name(ast, id),
                });
            };
            let name = map_entry_name_at(ast, id, 0)?;
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::lines(&[
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a",
                    "sequence of these, and the rules write the two halves with no",
                    "framing between them.",
                ]),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::Struct(vec![
                    zig::Field {
                        doc: Doc::none(),
                        name: Ident::new("key"),
                        type_: zig_member_type(ast, key, &name)?,
                        default: None,
                    },
                    zig::Field {
                        doc: Doc::none(),
                        name: Ident::new("value"),
                        type_: zig_member_type(ast, value, &name)?,
                        default: None,
                    },
                ])),
            }));
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "ZigAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = zig_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // `pub const Distance = i32;` is the whole of it. A Zig
            // declaration binds a name to a type value and creates no
            // new type (Zig language reference, "comptime"), which is
            // exactly what IDL 4.2 §7.4.1.4.3 says an alias does: it
            // "does not define a new type". Go needs the `= ` form of
            // `type` to get the same effect; Zig has only this one.
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig_member_type(ast, target, &node.name)?),
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "ZigEnum").is_none() {
                return Ok(());
            }
            let name = zig_type_name(ast, id);
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
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "ZigVariant").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let spelled = field_name(&enum_node.name);
                variants.push(zig::Variant {
                    doc: idl_note(&spelled, &enum_node.name),
                    name: Ident::new(&spelled),
                    // Written out, always. Zig would number an
                    // unvalued variant consecutively from zero and
                    // `@value` admits gaps, so leaving it out would
                    // renumber the wire.
                    value: Some(zig::Expr::int(value)),
                });
            }
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration. The tag type is signed because DDS-XTypes 1.3",
                        "7.4.3.5.3 makes it so; the width on the wire is the",
                        "@bit_bound's and is applied by the runtime.",
                    ]),
                    &name,
                    &node.name,
                ),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::Enum {
                    tag: Box::new(zig::Type::Primitive(zig::Primitive::I32)),
                    variants,
                }),
            }));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "ZigBitmask").is_none() {
                return Ok(());
            }
            let name = zig_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3",
                        "7.4.1.1.3.2 gives for its bound. Not a `packed struct` of",
                        "bools: that lays its fields out in declaration order from the",
                        "backing integer's least significant bit, and a bit's place is",
                        "its @position, which admits gaps and any order.",
                    ]),
                    &name,
                    &node.name,
                ),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::Primitive(zig_holder(bound))),
            }));
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "ZigBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = zig::escape_decl(&format!("{prefix}{}", bit_node.name));
                remember(emitted, &constant, &bit_node.name)?;
                out.push(zig::Decl::Const(zig::ConstDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: idl_note(&constant, &bit_node.name),
                    is_pub: true,
                    name: Ident::new(&constant),
                    // Ascribed with the bitmask's own name so that
                    // `flags & READY` has one type throughout: a Zig
                    // `comptime_int` would coerce to whatever the other
                    // operand happened to be.
                    type_: Some(zig::Type::named(&name)),
                    value: zig::Expr::Binary {
                        op: "<<",
                        lhs: Box::new(zig::Expr::int(1)),
                        rhs: Box::new(zig::Expr::int(i128::from(position))),
                    },
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "ZigBitset").is_none() {
                return Ok(());
            }
            let name = zig_type_name(ast, id);
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
                fields.push(zig::Field {
                    doc: idl_note(&pairs[index].0, field_ident),
                    name: Ident::new(&pairs[index].0),
                    type_: zig::Type::Primitive(zig_holder(field_node.bits)),
                    default: None,
                });
                index += 1;
            }
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate integers here and not a",
                        "`packed struct`'s bit fields, because the order on the wire is",
                        "the declaration's and the packing into the anchor is the",
                        "codec's.",
                    ]),
                    &name,
                    &node.name,
                ),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::Struct(fields)),
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "ZigValue"
            } else {
                "ZigStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = zig_type_name(ast, id);
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
                fields.push(parent_field(ast, base)?);
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
                fields.push(member_field(ast, member_node, target)?);
            }
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: idl_note(&name, &node.name),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::Struct(fields)),
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "ZigUnion").is_none() {
                return Ok(());
            }
            let name = zig_type_name(ast, id);
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
            // of its own, so `_d` is this column's, and a branch called
            // `_d` would take the same Zig spelling and make one of the
            // two unreachable.
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((field_name(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = vec![zig::Field {
                doc: Doc::line("Which branch carries the value."),
                name: Ident::new(DISCRIMINATOR),
                type_: zig_member_type(ast, disc, &name)?,
                default: None,
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "ZigCase").is_none() {
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
                fields.push(member_field(ast, member, target)?);
            }
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union, as a struct carrying the discriminator beside every",
                        "branch. Not a Zig `union(enum)`: a tagged union's tag is its",
                        "active field, and an IDL discriminator may take a value no",
                        "case label selects (IDL 4.2 7.4.1.4.4.2.3), in which state no",
                        "branch is live and only the discriminator goes on the wire",
                        "(DDS-XTypes 1.3 7.4.1.1.5.2). A tagged union cannot be",
                        "constructed that way; this struct holds it exactly.",
                    ]),
                    &name,
                    &node.name,
                ),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::Struct(fields)),
            }));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "ZigNative").is_none() {
                return Ok(());
            }
            let name = zig_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `*anyopaque` when there is none.
            // `anyopaque` itself cannot be a member's type — Zig gives
            // it no size — so the pointer is the declaration.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line("A native type. Opaque unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(match mapping {
                    Some(spelling) => zig::Type::named(&spelling),
                    None => zig::Type::pointer(zig::Type::Primitive(zig::Primitive::AnyOpaque)),
                }),
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = zig_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            // `?*T` and not `*T`: a Zig pointer cannot be null, so the
            // optional is what carries the absence a boxed value has.
            // The pointer and not `?T` is what breaks a cycle, since a
            // boxed value may hold the aggregate that holds it.
            out.push(zig::Decl::Const(zig::ConstDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line("A boxed value: a pointer, `null` when absent."),
                    &name,
                    &node.name,
                ),
                is_pub: true,
                name: Ident::new(&name),
                type_: None,
                value: zig::Expr::type_(zig::Type::optional(zig::Type::pointer(zig_member_type(
                    ast, target, &name,
                )?))),
            }));
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `zig_member_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The Zig spelling of a value's type, refusing what Zig cannot hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided
/// at run time — Zig's `anytype` is a comptime parameter form and not a
/// storage type at all, and `*anyopaque` would compile with no wire form
/// behind it, which is the silent-substitution class this project is
/// arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn zig_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<zig::Type, MaterialiseError> {
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
    zig_value_type(ast, target, at)
}

/// The Zig type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated struct, a
/// codec's parameter and a defaulter's destination all have to name the
/// same Zig type or the file does not compile.
///
/// A type the arena does not answer for is an error and not a
/// substitution. Go's column returns `any` there because Go has one that
/// compiles; Zig's nearest equivalent would be `*anyopaque`, which would
/// put a pointer where a value belongs and take the wire form with it.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the type, an array length or a
/// collection's element does not resolve.
pub fn zig_value_type(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
) -> Result<zig::Type, MaterialiseError> {
    value_type_at(ast, id, at, 0)
}

/// [`zig_value_type`], carrying how many connector steps the walk has
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
) -> Result<zig::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Zig spelling nests past {MAX_TYPE_DEPTH}"),
            at: zig_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "type behind a member".to_string(),
            at: at.to_string(),
        });
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => zig_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units, which puts
        // the §7.4.1.1.2 surrogate rule inside the Zig runtime where it
        // is a second implementation of the clause and not a copy of the
        // Rust column's answer. A Zig `[]const u8` is a run of octets
        // with no encoding of its own, which is what makes that
        // possible; a `fixed<P,S>` takes the same shape and holds its
        // decimal, for the reason the C column gives.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => zig::Type::text(),
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            zig::Type::slice(value_type_at(ast, element, at, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // Outermost dimension outermost: `long grid[2][3]` is two
            // rows of three, which in Zig is `[2][3]i32`. Building it
            // from the inside out is what keeps that true.
            let mut type_ = value_type_at(ast, element, at, depth + 1)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: node.name.clone(),
                        })?;
                type_ = zig::Type::array(length, type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            zig::Type::slice(zig::Type::named(&map_entry_name_at(ast, id, depth + 1)?))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the Zig type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "inline typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            value_type_at(ast, target, at, depth + 1)?
        }
        _ => zig::Type::named(&zig_type_name(ast, id)),
    })
}

/// The Zig type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a Zig spelling
/// is a compile error and not a member that silently takes another's.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open, and a fixed-width integer is the only choice that does not
/// depend on a platform type.
///
/// `long double` is an `f128` and not a decimal in a string: Zig has a
/// real binary128, and the module documentation says what that is worth.
#[must_use]
pub fn zig_primitive_type(kind: PrimitiveKind) -> zig::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => zig::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => zig::Primitive::U8,
        PrimitiveKind::Int8 => zig::Primitive::I8,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => zig::Primitive::U16,
        PrimitiveKind::Int16 => zig::Primitive::I16,
        PrimitiveKind::Int32 => zig::Primitive::I32,
        PrimitiveKind::UInt32 => zig::Primitive::U32,
        PrimitiveKind::Int64 => zig::Primitive::I64,
        PrimitiveKind::UInt64 => zig::Primitive::U64,
        PrimitiveKind::Float32 => zig::Primitive::F32,
        PrimitiveKind::Float64 => zig::Primitive::F64,
        PrimitiveKind::Float128 => zig::Primitive::F128,
    };
    zig::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for.
///
/// The smallest standard width that holds it, and not Zig's exact
/// `u3`: a bitset's field is a value here and the packing into the
/// anchor is the codec's, so a type narrower than the value it is given
/// would make an assignment a truncation the compiler performs silently
/// where a codec should have refused.
#[must_use]
pub const fn zig_holder(bits: u16) -> zig::Primitive {
    match bits {
        0..=8 => zig::Primitive::U8,
        9..=16 => zig::Primitive::U16,
        17..=32 => zig::Primitive::U32,
        _ => zig::Primitive::U64,
    }
}

/// A declared type's Zig identifier.
///
/// The scoped name with the module separators flattened onto it, which
/// is what a file that cannot nest requires: `::common::util::Location`
/// is `common_util_Location`. No case is changed — Zig's visibility is
/// the `pub` keyword and not the first letter, so a rename here would be
/// a rename with no cause.
#[must_use]
pub fn zig_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
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
/// built from the IDL kind and not the Zig spelling: `double` and
/// `long double` are `f64` and `f128` here, but `octet` and `uint8` are
/// both `u8`, and a name built from the Zig type would give
/// `map<octet,V>` and `map<uint8,V>` one declaration — harmless today
/// and the same collapse that would be wrong if the two ever differed on
/// the wire.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a key's or a value's tag
/// nests past `MAX_TYPE_DEPTH`.
pub fn zig_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    map_entry_name_at(ast, id, 0)
}

/// [`zig_map_entry_name`], carrying the depth of the walk that reached it.
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
            at: zig_type_name(ast, id),
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

/// The name component a map entry takes from its key or value.
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
            at: zig_type_name(ast, id),
        });
    }
    Ok(match ast.type_(id).map(|node| node.kind.clone()) {
        // The IDL spelling with its spaces closed up: `unsigned long`
        // is `unsigned_long`. The whole name is prefixed, so nothing
        // here can collide with a Zig primitive.
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
                        // A dimension that does not evaluate gets no
                        // number rather than a zero: the name has to
                        // differ from a `[0]` array's, and the caller
                        // reports the failure by name.
                        Err(_) => name.push_str("_x"),
                    }
                }
                name
            }
            None => "arr_of_unknown".to_string(),
        },
        Ok(TypeKind::Map { .. }) => map_entry_name_at(ast, id, depth + 1)?,
        _ => zig_type_name(ast, id),
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

/// The Zig name a declaration outside the type arena takes.
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
    zig::escape_decl(&segments.join("_"))
}

/// Join scoped segments with `_` and escape the result.
///
/// `_` and not concatenation: `commonutilLocation` cannot be read back
/// to its segments, and two different scoped names would reach one Zig
/// name with nothing to say so. The join is still not injective — an IDL
/// identifier may itself contain `_` — which is why [`remember`] checks
/// every declared name against every other one in the file.
///
/// The escape is applied to the **joined** name and not to each segment,
/// which is the difference from the Go column. A Zig keyword or
/// primitive name is only a problem when it is the whole identifier:
/// `align::Pose` flattens to `align_Pose`, which shadows nothing and
/// parses, while escaping per segment would give `align__Pose` and a
/// second underscore that means nothing.
fn flatten(segments: &[String]) -> String {
    zig::escape_decl(&segments.join("_"))
}

/// The Zig spelling of a member, an enumerator, a bitfield or a method.
///
/// Only the keyword escape: measured on Zig 0.14.1,
/// `struct { type: u8, u8: i32 }` compiles, because a field name lives
/// in the container's field namespace and the primitive types are not in
/// it. A member spelled `type` therefore keeps its IDL spelling, where
/// the same name as a declaration would become `type_`.
#[must_use]
pub fn field_name(name: &str) -> String {
    zig::escape_field(name)
}

/// The field one member declaration becomes.
///
/// Two indirections, and the order between them is the whole of it.
/// `@external` says the member is stored outside the containing object
/// (DDS-XTypes 1.3 Table 21), which in Zig is a pointer and needs no
/// invented box type. `@optional` says it may be absent
/// (DDS-XTypes 1.3 §7.2.2.4.4.4.7), which is `?`. The pointer goes on
/// first and the optional outside it — `?*Node` — which is the shape
/// that both breaks a recursion and can say "absent". The other order,
/// `*?Node`, is a pointer to an optional and is neither.
///
/// Measured: `tests/interop-corpus/recursive-optional-external` declares
/// `@optional @external Node next` inside `Node`. Without the pointer,
/// `next: ?m_Node` is a struct containing itself and Zig refuses it —
/// `struct 'm_Node' depends on itself`. No Rust test would have found
/// that: the tree is finite either way, and `@optional` alone is what
/// the sibling Go column relies on, because a Go pointer is already the
/// shape its optional takes.
/// The field that holds the base of a derived structure: the first member `parent`
/// (XTypes 1.3 §7.5.1.4.1.3).
fn parent_field(ast: &Ast, base: Id<TypeNode>) -> Result<zig::Field, MaterialiseError> {
    let spelled = field_name("parent");
    let mut doc = idl_note(&spelled, "parent");
    doc.push("The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).");
    Ok(zig::Field {
        doc,
        name: Ident::new(&spelled),
        type_: zig_member_type(ast, base, "parent")?,
        default: None,
    })
}

fn member_field(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<zig::Field, MaterialiseError> {
    let spelled = field_name(&member.name);
    let mut doc = idl_note(&spelled, &member.name);
    let mut type_ = zig_member_type(ast, target, &member.name)?;
    if super::held_externally(ast, member) {
        type_ = zig::Type::pointer(type_);
        doc.push("Stored by reference — @external, DDS-XTypes 1.3 Table 21. A Zig");
        doc.push("pointer is the whole of it, and the indirection is what makes a");
        doc.push("self-recursive type finite.");
    }
    let mut default = None;
    if member.facets.is_optional() {
        // `?T` and not a value beside a flag: Zig's optional has a
        // `null` that no value of `T` can equal, where a zero-initialised
        // `i32` member that arrived as 0 and one that did not arrive at
        // all would be the same value.
        type_ = zig::Type::optional(type_);
        default = Some(zig::Expr::Literal(Literal::Null));
        doc.push("Optional. `null` when the member is absent — DDS-XTypes 1.3");
        doc.push("7.2.2.4.4.4.7. In a @mutable type an absent member is not written");
        doc.push("at all; outside one its absence is written.");
    }
    Ok(zig::Field {
        doc,
        name: Ident::new(&spelled),
        type_,
        default,
    })
}

/// The declaration's own IDL name, where the Zig spelling moved it.
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
/// the same Zig spelling.
///
/// One file container holds every declaration, so this is a whole-file
/// check and not a per-scope one — which is the difference from
/// [`no_spelling_collision`], used for the fields of one aggregate. Two
/// IDL modules can flatten onto one Zig name (`a_b::C` and `a::b_C` both
/// give `a_b_C`) and Zig would report `duplicate struct member name`,
/// naming neither IDL declaration.
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

/// The Zig expression a constant's value takes.
///
/// Zig's `comptime_int` and `comptime_float` are arbitrary-precision and
/// f128-wide respectively, and coerce to the ascribed type at the
/// declaration; a value that does not fit is a compile error rather than
/// a wrap. That is why the ascription is always written: an unascribed
/// `pub const LIMIT = 8;` would be a `comptime_int` and would take
/// whatever width its first use wanted.
///
/// Unlike the Go and Python columns, a `long double` constant needs no
/// refusal here. Its Zig holder is `f128` and a decimal literal is
/// parsed at that width, so the number in the file is the number the IDL
/// wrote.
fn zig_constant(
    value: &zerodds_ast::ConstValue,
    type_: &zig::Type,
    at: &str,
) -> Result<zig::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_text = matches!(type_, zig::Type::Slice { .. });
    Ok(match value {
        ConstValue::Boolean(flag) => zig::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => zig::Expr::Literal(Literal::int(*number)),
        // The code point as a number: an IDL `char` is one octet here
        // and an IDL `wchar` two, while a Zig character literal is a
        // `comptime_int` holding a Unicode code point. Writing `'x'`
        // would compile and would put a different width's value in the
        // file for any character the two forms disagree on.
        ConstValue::Char(character) => {
            zig::Expr::Literal(Literal::int(i128::from(u32::from(*character))))
        }
        ConstValue::WChar(point) => zig::Expr::Literal(Literal::int(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            if is_text {
                return Err(refuse("a floating-point constant of a text-shaped type"));
            }
            zig::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            zig::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => zig::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => zig::Expr::Literal(Literal::Str(
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
