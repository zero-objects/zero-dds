// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Nim CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::go`], which does the same for
//! Go, and of [`crate::adapter::c`], which does it for C. Reading the
//! three side by side is the point: all of them flatten a module onto
//! the name, and everything else they do differently is the target
//! language's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The five things Nim decides that Go does not
//!
//! 1. **Visibility renames nothing.** Nim's export marker is a `*` after
//!    the name (Nim manual, "Export marker"), so an IDL `pose` is a Nim
//!    `pose*` and not a Go `Pose`. The Go column has to record what its
//!    capitalisation lost — a struct tag on every field and a comment
//!    over every declaration — because `Value` could have come from
//!    `value` or from `Value` and the rename does not invert. Here the
//!    IDL spelling survives into the file, so the only comment a
//!    declaration carries is one for the flattened module path and one
//!    for a name the escape had to repair.
//! 2. **Declarations are grouped into sections.** One `type` section
//!    holds every declared type, one `const` section holds every
//!    constant. Within a single `type` section the declarations may name
//!    each other in any order, including mutually (Nim manual, "Type
//!    sections"), so — as in Go and unlike `adapter/c.rs`, which walks
//!    the type graph post-order — the output tracks the declaration
//!    order of the source, and unlike the OCaml column there is no `and`
//!    to thread between the declarations that recurse.
//! 3. **An enumeration is one declaration.** Nim's enum fields take
//!    explicit ordinals, which is exactly what `@value` needs, so the C
//!    and Go answer — a defined integer type plus one constant per
//!    enumerator — is not required. The price is a constraint: the
//!    ordinals must ascend, and an IDL enumeration whose `@value`s do
//!    not is refused by name.
//! 4. **A union is a plain object.** Nim has an object variant and this
//!    column does not use one; the reason is below.
//! 5. **Two IDL member names can be one Nim field.** Nim compares
//!    identifiers after normalisation — first character's case
//!    significant, the rest lower-cased with the underscores removed —
//!    so `foo_bar` and `fooBar` are one identifier. IDL 4.2 §7.2.3.1
//!    forbids only a pair that differs in case alone, so the pair is
//!    legal IDL. [`zerodds_lang_cst::nim::normalise`] computes the key
//!    and every scope is checked against it.
//!
//! ## The memory shapes
//!
//! | IDL | Nim |
//! |---|---|
//! | `boolean` | `bool` |
//! | `octet`, `uint8`, `char` | `uint8` |
//! | `wchar`, `unsigned short` | `uint16` |
//! | `long` | `int32` |
//! | `long long` | `int64` |
//! | `float`, `double` | `float32`, `float64` |
//! | `long double` | `string`, holding the decimal |
//! | `string`, `wstring` | `string`, the second holding UTF-8 |
//! | `fixed<P,S>` | `string`, holding the decimal |
//! | `sequence<T>` | `seq[T]` |
//! | `T[N][M]` | `array[N, array[M, T]]`, outer dimension outermost |
//! | `map<K,V>` | `seq[ZdMapEntry_K_V]`, **not** a `Table` |
//! | `union` | `object` with a `d` field and one `Option[T]` per branch |
//! | `@optional T m` | `Option[T]`, `none(T)` when absent |
//! | `@external T m` | `ref T` |
//! | `@optional @external T m` | `Option[ref T]` |
//! | `enum` | `enum`, every ordinal written out |
//! | `bitmask` | an unsigned alias and one `const` per bit |
//! | `native` | `pointer`, or the facet's `native_mapping` |
//! | `interface` | `object` of procedural fields |
//!
//! ### A union is a plain object and never an object variant
//!
//! Nim's object variant — a `case` inside an object — is the shape an
//! IDL union looks like, and it is the wrong one here for two reasons
//! that both bite a codec rather than a reader.
//!
//! A variant object's branch fields are guarded by the discriminator at
//! run time: reading one while the discriminator selects another branch
//! raises `FieldDefect`. To write one, the discriminator has to be set
//! first, and it can only be set at object construction. A decoder reads
//! the discriminator **off the wire**, so the value it constructs with
//! is not a compile-time constant — and Nim cannot then prove which
//! branch the construction is filling. The way out is either a `case`
//! over the discriminator in every decoder, which restates the branch
//! table the type already has, or the `{.cast(uncheckedAssign).}`
//! pragma, which switches off exactly the checking that made the variant
//! attractive.
//!
//! The second reason is the discriminator's range. IDL 4.2 §7.4.1.4.4.1
//! lets a discriminator hold a value that matches no case label and no
//! `default`, and then no member is selected at all. A variant object
//! can only say that with an `else: discard` branch, and the resulting
//! type still cannot distinguish "no branch selected" from "a branch
//! whose fields were never written".
//!
//! An object with a `d` field and one `Option[T]` per branch says both
//! plainly: `d` is the discriminator as it came off the wire, and the
//! branch that was written is the one that is `isSome`. Only the
//! selected branch goes on the wire (DDS-XTypes 1.3 §7.4.1.1.5.2), so
//! every other branch is genuinely absent and not merely unset.
//!
//! The discriminator is called `d` and not `_d`, which is what every
//! other column spells it. A Nim identifier starts at a letter (Nim
//! manual, Lexical Analysis: `IDENTIFIER ::= letter ( ['_'] (letter |
//! digit) )*`), so `_d` is not a name in this language. `d` is in the
//! collision check with the branches, because IDL 4.2 §7.4.1.4.4.1
//! gives the discriminator no member name of its own and a branch called
//! `d` would take the same field.
//!
//! ### `@external` is a `ref` everywhere, and not only where it has to be
//!
//! DDS-XTypes 1.3 Table 21 says an `@external` member is held outside
//! the containing object; the indirection is the language mapping's and
//! changes no octet. Every `@external` member takes a `ref` here,
//! whether or not anything would break without it, because a mapping
//! that reached for a reference only when a cycle forced it would be a
//! repair and not a rule — `@external long a` would then be held inline
//! and the generated file would say nothing about what the IDL declared.
//!
//! Where it has to be is `tests/interop-corpus/recursive-optional-external`:
//! `@optional @external Node next` inside `Node`. Nim's `Option[T]`
//! stores its `T` inline, so without the reference the object contains
//! itself and Nim 2.2.10 refuses it — `illegal recursion in type
//! 'm_Node'`, measured on the Linux host. No Rust test would have found
//! it: the tree is finite either way, and the Go column does not meet it
//! at all, because a Go pointer is already the shape its optional takes.
//!
//! ### `map<K,V>` is a sequence of entries and never a `Table`
//!
//! `std/tables` is a hash table: its iteration order is the order of the
//! buckets, which depends on the hash of the keys and on the table's
//! capacity, and it is not the insertion order. A `map<K,V>` is written
//! to the wire as a sequence of key/value pairs (DDS-XTypes 1.3
//! §7.4.1.1.4), so a column holding it in a `Table` would produce a
//! different octet stream for two values that differ only in the order
//! they were built — and the KeyHash of a keyed type with them.
//!
//! ### `long double` is a `string` and never a `float64`
//!
//! Nim has `float32`, `float64` and no binary128. `float64` has 53 bits
//! of significand where an IDL `long double` has 113, so a member
//! declared `float64` would lose sixty bits at the assignment, before
//! any codec runs. The member holds the decimal text instead — exactly
//! as `fixed<P,S>` does, and as the Go and Python columns do for the
//! same reason.
//!
//! ### An enumeration's ordinals must ascend
//!
//! Nim: "the ordinal values have to be in ascending order" (Nim manual,
//! "Enumeration types"). `@value` does not have to, so an IDL
//! enumeration that numbers its enumerators downwards, or gives two of
//! them one value, has no Nim enum at all and is refused by name rather
//! than renumbered. An enumeration with *gaps* is fine and is what
//! `@value` usually produces: Nim calls it a *holey* enum, accepts it,
//! and only withdraws the conveniences that assume contiguity — it
//! cannot be iterated with `..` and cannot index an `array`. A generated
//! type needs neither.
//!
//! ### A bitmask is an integer and never a `set[T]`
//!
//! Nim has `set[T]`, which is a bit set over an ordinal type and would
//! read well. It is bounded: the base type's range must be under 2^16
//! (Nim manual, "Set type"), where DDS-XTypes 1.3 §7.4.1.1.3.2 admits a
//! `@bit_bound(64)`. And the bit a member occupies inside a `set` is the
//! compiler's layout, not the `@position` the IDL wrote, which is the
//! number the wire form uses. The plain unsigned integer keeps both.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::nim;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Nim";

/// The prefix a name this column invented carries.
///
/// Only a map entry needs one — every other Nim shape is written where
/// the member is. The prefix also keeps an invented name clear of every
/// Nim keyword and of every predeclared name, so it never needs an
/// escape of its own.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// The Nim field name of the union discriminator.
///
/// `d` and not `_d`: Nim's identifier production starts at a letter, so
/// a leading underscore is a lexical error and not a name. The module
/// documentation carries the rest.
pub const DISCRIMINATOR: &str = "d";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Nim spelling may nest before the walk refuses
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

/// What the walk over the model has produced so far.
///
/// Two lists and not one, because Nim groups declarations into sections:
/// every type goes under one `type` and every constant under one
/// `const`. The `type` section is written first, so a constant may name
/// a type — Nim resolves a name at module scope only after its
/// declaration, and only a `type` section is exempt from that.
#[derive(Debug, Default)]
struct Build {
    /// The declarations of the `type` section, in model order.
    types: Vec<nim::TypeDecl>,
    /// The declarations of the `const` section, in model order.
    consts: Vec<nim::ConstDecl>,
    /// Every module-scope name declared so far: the normalised key, and
    /// the IDL name it came from.
    emitted: Vec<(String, String)>,
}

/// Build the Nim sections a rule application produced.
///
/// The order is the model's own declaration order within each section.
/// A `type` section resolves a name declared anywhere inside it, so —
/// as in Go and unlike the C column — there is no dependency walk and
/// the output tracks the source.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Nim form, or two declarations reach one Nim
/// identifier — never a silently dropped declaration.
pub fn nim_sections(ast: &Ast, applied: &Applied) -> Result<Vec<nim::Section>, MaterialiseError> {
    let mut build = Build::default();
    // Text for a type that declared nothing in the `type` section: it has
    // no declaration to ride on, and goes after the section.
    let mut unattached: Vec<String> = Vec::new();

    for (id, _) in ast.types() {
        let start = build.types.len();
        declare(ast, applied, id, &mut build)?;
        attach_verbatim(
            verbatim::declaration_slots(ast, id, verbatim::aliases::NIM),
            &nim_type_name(ast, id),
            build.types.get_mut(start..).unwrap_or_default(),
            &mut unattached,
        );
    }

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "NimConst").is_none() {
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
            Some(target) => nim_value_type(ast, target)?,
            None => nim::Type::Primitive(nim::Primitive::Int32),
        };
        let name = declared_name(ast, &node.name, node.module);
        declare_name(&mut build, &name, &node.name)?;
        let value = nim_constant(&value, &type_, &node.name)?;
        build.consts.push(nim::ConstDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            exported: true,
            type_: Some(type_),
            value,
        });
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "NimError").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        declare_name(&mut build, &name, &node.name)?;
        let mut pairs = Vec::new();
        for member in &node.members {
            if let Ok(member_node) = ast.member(*member) {
                pairs.push((field_key(&member_node.name), member_node.name.clone()));
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
            let (type_, doc) = member_shape(ast, member_node, target)?;
            fields.push(nim::Field {
                doc,
                name: Ident::new(&field_name(&member_node.name)),
                exported: true,
                type_,
            });
        }
        // A plain object and not one inheriting `CatchableError`: only a
        // `ref object` can be raised in Nim, and a `ref` would give the
        // type reference semantics that no other member of a generated
        // file has. Raising is the RPC arc's business; the shape is the
        // IDL's.
        build.types.push(nim::TypeDecl {
            text: zerodds_lang_cst::nim::TypeText::default(),
            doc: with_idl_note(
                Doc::line("An exception, as the object its members make."),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            exported: true,
            body: nim::TypeBody::Object(fields),
        });
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "NimInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        declare_name(&mut build, &name, &node.name)?;
        let mut fields = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. The object below is
        // a plain `object` and not one inheriting another, so a derived
        // interface has no base object to reach OMG IDL 4.2 §7.4.3's
        // inherited operations and attributes through: a field left out
        // here is one the generated file does not have.
        // `interface::all_operations_flattened` gives both — the
        // get/set pair DDS-RPC 1.0 §7.5.1.1.3 implies for each
        // attribute, besides the declared operations — which
        // `Ast::all_operations_of` alone never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            // An attribute's own name for its getter field, `set` plus
            // it for the setter's — the same bare-name/`set`-prefixed
            // pair the Go and Swift bindings take for the same reason —
            // rather than `all_operations_flattened`'s `get_`/`set_`
            // prefix, which spells out DDS-RPC 1.0's own implied-
            // operation name and not a Nim one.
            let field_ident = match op.origin {
                interface::Origin::AttributeGetter => field_name(op.attribute_name()),
                interface::Origin::AttributeSetter => {
                    let attr = op.attribute_name();
                    format!("set{}", capitalise(&field_name(attr)))
                }
                interface::Origin::Declared => field_name(&op.name),
            };
            signatures.push((nim::normalise(&field_ident), op.name.clone()));
            let mut params = Vec::new();
            let mut param_names = Vec::new();
            for param in &op.params {
                param_names.push((field_key(&param.name), param.name.clone()));
                params.push(nim::Param {
                    name: Ident::new(&field_name(&param.name)),
                    type_: nim_member_type(ast, param.type_, &param.name)?,
                });
            }
            // A parameter list is a scope of its own: two parameters
            // that normalise onto one name are a signature Nim refuses,
            // and the message would name a Nim identifier rather than
            // the two IDL parameters behind it.
            no_spelling_collision(as_pairs(&param_names), LANGUAGE, &op.name)?;
            let result = match op.returns {
                Some(returns) => Some(Box::new(nim_member_type(ast, returns, &op.name)?)),
                None => None,
            };
            let mut doc = repair_note(&op.name);
            if let Some(line) = op.raises_line() {
                doc.push(&line);
            }
            fields.push(nim::Field {
                doc,
                name: Ident::new(&field_ident),
                exported: true,
                type_: nim::Type::Proc { params, result },
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        // Nim has no interface construct. A `concept` is the closest —
        // and it is documented as experimental and its own manual says
        // its specification will change, so a generated file that
        // depended on one would be a file whose meaning is a compiler
        // release away from moving. An object of procedural fields is
        // ordinary Nim: the field holds the implementation, and a
        // consumer with several implementations holds several objects.
        build.types.push(nim::TypeDecl {
            text: zerodds_lang_cst::nim::TypeText::default(),
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface, as an object of procedural fields — one per",
                    "operation, and one (or, `readonly` aside, two) per attribute. Nim",
                    "has no interface construct; `concept` is experimental, so the",
                    "shape a consumer can rely on is a field per member. A derived",
                    "interface's bases are flattened in — the object inherits from",
                    "nothing — so this is every field the interface has, own and",
                    "inherited. The transport is the RPC arc's business; the shape is",
                    "the IDL's. `raises`/`getraises`/`setraises` are documented on the",
                    "field they belong to.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            exported: true,
            body: nim::TypeBody::Object(fields),
        });
    }

    let (begin_file, end_file) = verbatim::file_slots(ast, verbatim::aliases::NIM);
    let mut sections: Vec<nim::Section> =
        begin_file.into_iter().map(nim::Section::Verbatim).collect();
    if !build.types.is_empty() {
        sections.push(nim::Section::Type(build.types));
    }
    sections.extend(unattached.into_iter().map(nim::Section::Verbatim));
    if !build.consts.is_empty() {
        sections.push(nim::Section::Const(build.consts));
    }
    sections.extend(end_file.into_iter().map(nim::Section::Verbatim));
    Ok(sections)
}

/// Attach one type's `@verbatim` text to what it declared: `declared`,
/// the `type` section's entries for it.
///
/// `BEFORE_DECLARATION` rides before the first entry and
/// `AFTER_DECLARATION` after the last. `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close the body of the entry named `name`
/// when it is an object; for an enumeration or an alias they go
/// adjacent, `BEGIN` before and `END` after. A type that declared no
/// entry leaves all of it in `unattached`, which follows the section.
fn attach_verbatim(
    slots: verbatim::Slots,
    name: &str,
    declared: &mut [nim::TypeDecl],
    unattached: &mut Vec<String>,
) {
    if slots.is_empty() {
        return;
    }
    let mut begin = slots.begin;
    let mut end = slots.end;
    if let Some(object) = declared
        .iter_mut()
        .find(|decl| decl.name.as_str() == name && matches!(decl.body, nim::TypeBody::Object(_)))
    {
        object.text.body.begin.append(&mut begin);
        object.text.body.end.append(&mut end);
    }
    if declared.is_empty() {
        unattached.extend(slots.before);
        unattached.extend(begin);
        unattached.extend(end);
        unattached.extend(slots.after);
        return;
    }
    if let Some(first) = declared.first_mut() {
        let mut before = slots.before;
        before.append(&mut begin);
        before.append(&mut first.text.before);
        first.text.before = before;
    }
    if let Some(last) = declared.last_mut() {
        last.text.after.append(&mut end);
        last.text.after.extend(slots.after);
    }
}

/// The Nim sections, in a file that imports what they need.
///
/// A caller that only wants the sections uses [`nim_sections`]; this is
/// what a generator writes to disk, and it exists here so that the
/// imports are decided in one place.
///
/// # Errors
/// As [`nim_sections`].
pub fn nim_file(ast: &Ast, applied: &Applied) -> Result<nim::File, MaterialiseError> {
    let sections = nim_sections(ast, applied)?;
    // Read off the tree and never tracked beside it: a member that
    // reaches an `Option` through a sequence or an array asks for the
    // import exactly as a bare one does, and a flag set at the point the
    // `Option` was built would have to be set in five places.
    let mut imports = Vec::new();
    if sections.iter().any(section_needs_options) {
        imports.push(nim::Import::new("std/options"));
    }
    Ok(nim::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "Every declaration carries Nim's `*` export marker, which is what makes",
            "it reachable from another module. The marker renames nothing, so the",
            "IDL spelling of a declaration is the spelling in this file unless a",
            "comment above it says otherwise.",
        ]),
        imports,
        sections,
    })
}

/// Whether anything in a section is an `Option`.
fn section_needs_options(section: &nim::Section) -> bool {
    let nim::Section::Type(decls) = section else {
        return false;
    };
    decls.iter().any(|decl| match &decl.body {
        nim::TypeBody::Object(fields) => fields.iter().any(|field| field.type_.needs_options()),
        nim::TypeBody::Alias(type_) => type_.needs_options(),
        nim::TypeBody::Enum(_) => false,
    })
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    build: &mut Build,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence and an array are
        // all written where the member is: Nim spells each of them
        // inline, so none of them is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // The one collection that does need a name. Nim's structural
        // `tuple[key: K, value: V]` would do without a declaration, but
        // it is a shape a reader cannot name and a `seq` of it prints as
        // its structure rather than as the map it is.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: nim_type_name(ast, id),
                });
            };
            let name = map_entry_name_at(ast, id, 0)?;
            if build
                .emitted
                .iter()
                .any(|(key, _)| *key == nim::normalise(&name))
            {
                return Ok(());
            }
            declare_name(build, &name, &name)?;
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: Doc::lines(&[
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a",
                    "sequence of these, and the rules write the two halves with no",
                    "framing between them.",
                ]),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Object(vec![
                    nim::Field {
                        doc: Doc::none(),
                        name: Ident::new("key"),
                        exported: true,
                        type_: nim_member_type(ast, key, &name)?,
                    },
                    nim::Field {
                        doc: Doc::none(),
                        name: Ident::new("value"),
                        exported: true,
                        type_: nim_member_type(ast, value, &name)?,
                    },
                ]),
            });
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "NimAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            // A plain alias and never `distinct`. A `distinct` type has
            // its own identity and needs an explicit conversion to reach
            // its base, which is not what an IDL typedef says
            // (IDL 4.2 §7.4.1.4.3: the alias "does not define a new
            // type").
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Alias(nim_member_type(ast, target, &node.name)?),
            });
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "NimEnum").is_none() {
                return Ok(());
            }
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            let prefix = module_prefix(ast, id);
            let mut fields = Vec::new();
            let mut previous: Option<i128> = None;
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "NimEnumField").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                // "The ordinal values have to be in ascending order"
                // (Nim manual, "Enumeration types"). `@value` does not
                // have to, so a descending or repeated ordinal has no
                // Nim enum. Refused rather than renumbered: an
                // enumerator whose number changed would be a different
                // value on the wire.
                if previous.is_some_and(|last| value <= last) {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: "an enumerator whose value does not exceed its predecessor's, \
                                    which a Nim enum cannot order"
                            .to_string(),
                        language: LANGUAGE,
                        at: enum_node.name.clone(),
                    });
                }
                previous = Some(value);
                // The module path goes on the field, because a
                // non-`{.pure.}` enum's fields are declared in the
                // module's scope and not the enumeration's — two IDL
                // modules each with an `IDLE` would otherwise be one
                // name here.
                let field = escape_joined(&[prefix.as_str(), enum_node.name.as_str()]);
                declare_name(build, &field, &enum_node.name)?;
                fields.push(nim::EnumField {
                    doc: idl_note(&field, &enum_node.name),
                    name: Ident::new(&field),
                    value: Some(nim::Expr::int(value)),
                });
            }
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration. Every ordinal is written out, because @value admits",
                        "gaps and a field with none takes its predecessor's plus one. A Nim",
                        "enum with gaps is a `holey` enum: it holds and compares exactly as a",
                        "contiguous one does, and only `..` iteration and array indexing are",
                        "withdrawn. The width on the wire is the @bit_bound's and is applied",
                        "by the runtime.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Enum(fields),
            });
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "NimBitmask").is_none() {
                return Ok(());
            }
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3",
                        "7.4.1.1.3.2 gives for its bound.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Alias(nim::Type::Primitive(nim_holder(bound))),
            });
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "NimBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = escape_joined(&[prefix.as_str(), bit_node.name.as_str()]);
                declare_name(build, &constant, &bit_node.name)?;
                // The `1` is converted to the mask's own type before the
                // shift, not after. An unconverted `1` is an `int`, and
                // `1 shl 63` overflows an `int` into a negative number
                // before anything could convert it — which is exactly
                // the bit a `@bit_bound(64)` mask needs.
                build.consts.push(nim::ConstDecl {
                    doc: idl_note(&constant, &bit_node.name),
                    name: Ident::new(&constant),
                    exported: true,
                    type_: None,
                    value: nim::Expr::Binary {
                        op: "shl",
                        lhs: Box::new(nim::Expr::Convert {
                            type_: Box::new(nim::Type::named(&name)),
                            value: Box::new(nim::Expr::int(1)),
                        }),
                        rhs: Box::new(nim::Expr::int(i128::from(position))),
                    },
                });
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "NimBitset").is_none() {
                return Ok(());
            }
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            let mut pairs = Vec::new();
            // The base's bitfields are members of the derived object
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
                pairs.push((field_key(field_ident), field_ident.clone()));
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                fields.push(nim::Field {
                    doc: repair_note(field_ident),
                    name: Ident::new(&field_name(field_ident)),
                    exported: true,
                    type_: nim::Type::Primitive(nim_holder(field_node.bits)),
                });
            }
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate integers here; the packing",
                        "into the anchor is the codec's, because the order on the wire",
                        "is the declaration's.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Object(fields),
            });
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "NimValue"
            } else {
                "NimObject"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`.
            let members = ast.members_of(id).to_vec();
            let base = super::c::struct_base(ast, id);
            let mut pairs = Vec::new();
            if base.is_some() {
                pairs.push((field_key("parent"), "parent".to_string()));
            }
            for member in &members {
                if let Ok(member_node) = ast.member(*member) {
                    pairs.push((field_key(&member_node.name), member_node.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            if let Some(base) = base {
                let mut doc = Doc::none();
                doc.push("The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).");
                fields.push(nim::Field {
                    doc,
                    name: Ident::new(&field_name("parent")),
                    exported: true,
                    type_: nim_member_type(ast, base, "parent")?,
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
                let (type_, doc) = member_shape(ast, member_node, target)?;
                fields.push(nim::Field {
                    doc,
                    name: Ident::new(&field_name(&member_node.name)),
                    exported: true,
                    type_,
                });
            }
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Object(fields),
            });
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "NimUnion").is_none() {
                return Ok(());
            }
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let cases = ast.cases_of(id);
            // The discriminator's field is in the collision check with
            // the branches: IDL 4.2 §7.4.1.4.4.1 gives it no member name
            // of its own, so `d` is this column's, and a branch called
            // `d` — or `D_`, which normalises onto it — would take the
            // same field and make one of the two unreachable.
            let mut pairs = vec![(nim::normalise(DISCRIMINATOR), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((field_key(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = vec![nim::Field {
                doc: Doc::line("The discriminator, as it came off the wire."),
                name: Ident::new(DISCRIMINATOR),
                exported: true,
                type_: nim_member_type(ast, disc, &name)?,
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "NimCase").is_none() {
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
                // `held_shape` and not `member_shape`: the `Option` on a
                // branch is this column's marker for "this is the branch
                // that was written" and not an `@optional`, which
                // IDL 4.2 §7.4.1.4.4 does not admit on a union member.
                // An `@external` branch is still held by reference, so
                // the two compose the same way they do in a struct.
                let (type_, doc) = held_shape(ast, member, target)?;
                fields.push(nim::Field {
                    doc,
                    name: Ident::new(&field_name(&member.name)),
                    exported: true,
                    type_: nim::Type::option(type_),
                });
            }
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union. The branch that was written is the one that is `isSome`;",
                        "`d` is the discriminator, which may hold a value that selects no",
                        "branch at all (IDL 4.2 7.4.1.4.4.1). Only the selected branch goes",
                        "on the wire (DDS-XTypes 1.3 7.4.1.1.5.2). Not a Nim object variant:",
                        "a decoder constructs one from a discriminator it read at run time,",
                        "which Nim cannot check at compile time.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Object(fields),
            });
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "NimNative").is_none() {
                return Ok(());
            }
            let name = nim_type_name(ast, id);
            declare_name(build, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and an untraced `pointer` when there is
            // none. `pointer` and not `ref`: a `ref` is traced, and an
            // IDL `native` says nothing that would let this runtime take
            // ownership of what it names.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: with_idl_note(
                    Doc::line("A native type. Opaque unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Alias(match mapping {
                    Some(spelling) => nim::Type::named(&spelling),
                    None => nim::Type::Primitive(nim::Primitive::Pointer),
                }),
            });
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = nim_type_name(ast, id);
            if build
                .emitted
                .iter()
                .any(|(key, _)| *key == nim::normalise(&name))
            {
                return Ok(());
            }
            declare_name(build, &name, &node.name)?;
            // `Option[T]` and not a `ref`, for the reason an `@optional`
            // member takes one: a boxed value is a value that may be
            // absent, and a `ref` would add sharing the IDL did not ask
            // for.
            build.types.push(nim::TypeDecl {
                text: zerodds_lang_cst::nim::TypeText::default(),
                doc: with_idl_note(
                    Doc::line("A boxed value: a value that may be absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                exported: true,
                body: nim::TypeBody::Alias(nim::Type::option(nim_member_type(ast, target, &name)?)),
            });
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `nim_member_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The Nim spelling of a value's type, refusing what Nim cannot hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided at
/// run time — Nim's `RootObj` or a `pointer` would compile and there
/// would be no wire form to generate a codec against, which is the
/// silent-substitution class this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn nim_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<nim::Type, MaterialiseError> {
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
    nim_value_type(ast, target)
}

/// The Nim type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated object, a
/// codec's parameter and a defaulter's destination all have to name the
/// same Nim type or the file does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when an array length or a
/// collection's element does not resolve.
pub fn nim_value_type(ast: &Ast, id: Id<TypeNode>) -> Result<nim::Type, MaterialiseError> {
    value_type_at(ast, id, 0)
}

/// [`nim_value_type`], carrying how many connector steps the walk has
/// already taken.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the spelling nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array, map and
/// anonymous-alias nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn value_type_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<nim::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Nim spelling nests past {MAX_TYPE_DEPTH}"),
            at: nim_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(nim::Type::Primitive(nim::Primitive::Pointer));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => nim_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units, which puts
        // the §7.4.1.1.2 surrogate rule inside the Nim runtime where it
        // is a second implementation of the clause and not a copy of the
        // Rust column's answer. A Nim `string` is a run of octets with
        // no encoding of its own, which is what makes that possible.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            nim::Type::Primitive(nim::Primitive::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            nim::Type::seq(value_type_at(ast, element, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // Outermost dimension outermost: `long grid[2][3]` is two
            // rows of three, which in Nim is
            // `array[2, array[3, int32]]`. Building it from the inside
            // out is what keeps that true.
            let mut type_ = value_type_at(ast, element, depth + 1)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: node.name.clone(),
                        })?;
                type_ = nim::Type::array(length, type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            nim::Type::seq(nim::Type::named(&map_entry_name_at(ast, id, depth + 1)?))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the Nim type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_type_at(ast, target, depth + 1)?,
            None => nim::Type::Primitive(nim::Primitive::Pointer),
        },
        _ => nim::Type::named(&nim_type_name(ast, id)),
    })
}

/// The Nim type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a Nim spelling
/// is a compile error and not a member that silently takes another's.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open, and a fixed-width integer is the only choice that does not
/// depend on a platform type. Nim's own `char` is exactly one byte and
/// is *not* used: it is a distinct ordinal type with no arithmetic, so
/// `c + 1` does not compile and a codec adding an offset to an octet
/// would need a conversion at every use.
///
/// `long double` is a `string` and never a `float64`; the module
/// documentation carries why.
#[must_use]
pub fn nim_primitive_type(kind: PrimitiveKind) -> nim::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => nim::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => nim::Primitive::UInt8,
        PrimitiveKind::Int8 => nim::Primitive::Int8,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => nim::Primitive::UInt16,
        PrimitiveKind::Int16 => nim::Primitive::Int16,
        PrimitiveKind::Int32 => nim::Primitive::Int32,
        PrimitiveKind::UInt32 => nim::Primitive::UInt32,
        PrimitiveKind::Int64 => nim::Primitive::Int64,
        PrimitiveKind::UInt64 => nim::Primitive::UInt64,
        PrimitiveKind::Float32 => nim::Primitive::Float32,
        PrimitiveKind::Float64 => nim::Primitive::Float64,
        PrimitiveKind::Float128 => nim::Primitive::String,
    };
    nim::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for.
#[must_use]
pub const fn nim_holder(bits: u16) -> nim::Primitive {
    match bits {
        0..=8 => nim::Primitive::UInt8,
        9..=16 => nim::Primitive::UInt16,
        17..=32 => nim::Primitive::UInt32,
        _ => nim::Primitive::UInt64,
    }
}

/// A declared type's Nim identifier.
///
/// The scoped name with the module separators flattened onto it, which
/// is what a language whose module is a file requires:
/// `::common::util::Location` is `common_util_Location`. The case of
/// every segment is the IDL's — the export marker does the work Go's
/// capitalisation does, so nothing is renamed.
#[must_use]
pub fn nim_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let segments = scoped_segments(ast, id);
    if segments.is_empty() {
        return "ZdUnnamed".to_string();
    }
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    escape_joined(&borrowed)
}

/// The same, without the escape.
///
/// For a name this column is going to embed in a longer one. Stropping
/// puts backticks around the whole identifier, so a stropped segment
/// inside a longer name would produce `` a_`type`_b ``, which is not an
/// identifier at all — the escape has to run once, on the finished name.
fn raw_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let segments = scoped_segments(ast, id);
    if segments.is_empty() {
        return "ZdUnnamed".to_string();
    }
    segments.join("_")
}

/// A declared type's scoped name, outermost segment first.
fn scoped_segments(ast: &Ast, id: Id<TypeNode>) -> Vec<String> {
    let Ok(any) = ast.any_of_type(id) else {
        return Vec::new();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => scoped.segments().to_vec(),
        None => Vec::new(),
    }
}

/// The entry object a `map<K,V>` needs a name for.
///
/// Structural, so two identical anonymous maps collapse onto one
/// declaration instead of two that differ only in a counter. The tag is
/// built from the IDL kind and not the Nim spelling: `double` and
/// `long double` are `float64` and `string` here, but `octet` and
/// `uint8` are both `uint8`, and a name built from the Nim type would
/// give `map<octet,V>` and `map<uint8,V>` one declaration — harmless
/// today and the same collapse that would be wrong if the two ever
/// differed on the wire.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a key's or a value's tag
/// nests past `MAX_TYPE_DEPTH`.
pub fn nim_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    map_entry_name_at(ast, id, 0)
}

/// [`nim_map_entry_name`], carrying the depth of the walk that reached it.
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
            at: nim_type_name(ast, id),
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
    // Escaped once, at the end: the prefix keeps the finished name clear
    // of every keyword, so all the escape does here is repair whatever
    // underscores the tags brought with them.
    Ok(nim::escape_name(&format!("{INVENTED_PREFIX}{key}_{value}")))
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
            at: nim_type_name(ast, id),
        });
    }
    Ok(match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => capitalise(&kind.as_str().replace(' ', "_")),
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
        _ => raw_type_name(ast, id),
    })
}

/// `text` with its first character in upper case.
///
/// For the invented name of a map entry only. Nothing an IDL source
/// declared goes through it: Nim's export marker means this column never
/// has to change the case of a name it was given.
fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    for upper in first.to_uppercase() {
        out.push(upper);
    }
    out.push_str(chars.as_str());
    out
}

/// The type's name with the repair applied and no stropping: what a
/// generated codec function is named after.
///
/// A codec name is `zd_enc_` plus this, and the prefix keeps it clear of
/// every keyword — but a stropped segment inside it would produce
/// `` zd_enc_`method` ``, which is not an identifier, and a leading or
/// doubled underscore from the IDL would not lex. So the repair runs and
/// the backticks never do.
#[must_use]
pub fn nim_bare_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let escaped = nim::escape_name(&raw_type_name(ast, id));
    escaped.trim_matches('`').to_string()
}

/// The constant one enumerator is named by: the flattened module path,
/// then the enumerator, exactly as `declare`'s enumeration arm spells
/// the field — a non-`{.pure.}` enum's fields live in the module's
/// scope, so the use site and the declaration have to agree on the
/// prefix.
#[must_use]
pub fn nim_enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> String {
    let prefix = module_prefix(ast, id);
    escape_joined(&[prefix.as_str(), enumerator])
}

/// The flattened module path a module-scope name carries, with a
/// trailing separator, or nothing at the top level.
pub(crate) fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
    let segments = scoped_segments(ast, id);
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = segments[..segments.len() - 1].join("_");
    prefix.push('_');
    prefix
}

/// The Nim name a declaration outside the type arena takes.
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
    escape_joined(&borrowed)
}

/// Join scoped segments with `_` and escape the result once.
///
/// `_` and not concatenation: `commonUtilLocation` cannot be read back
/// to its segments, and two different scoped names would reach one Nim
/// name with nothing to say so. The join is still not injective — an IDL
/// identifier may itself contain `_`, and Nim ignores an interior
/// underscore anyway — which is why [`declare_name`] checks every
/// declared name against every other one in the file.
///
/// Escaped once and at the end, never per segment: a stropped segment
/// inside a longer name would produce `` a_`type`_b ``, which is not an
/// identifier.
fn escape_joined(segments: &[&str]) -> String {
    let joined: Vec<&str> = segments
        .iter()
        .copied()
        .filter(|segment| !segment.is_empty())
        .collect();
    nim::escape_name(&joined.join("_"))
}

/// The Nim spelling of a member, an enumerator, a parameter or a field.
#[must_use]
pub fn field_name(name: &str) -> String {
    nim::escape_name(name)
}

/// What a member is *held* in, before any marker for its absence.
///
/// `@external` says the member is stored outside the containing object
/// (DDS-XTypes 1.3 Table 21). In Nim that is `ref T`: a traced
/// reference, so the runtime owns the cell and frees it with the object.
/// `ref` and not `ptr` — a `ptr` is untraced and would make every
/// `@external` member an ownership question the IDL does not answer.
///
/// The annotation changes no octet: Table 21 says the indirection is the
/// language mapping's. It is applied to every `@external` member and not
/// only to the ones that recurse, because a mapping that reached for a
/// reference when a cycle forced it would be a repair and not a rule —
/// `@external Inner nested` and `@external long a` would then be held
/// inline and say nothing about what the IDL declared.
///
/// It also happens to be what makes a self-recursive declaration finite,
/// and that is where it was measured:
/// `tests/interop-corpus/recursive-optional-external` declares
/// `@optional @external Node next` inside `Node`, and Nim's `Option[T]`
/// stores its `T` inline, so `Option[m_Node]` is an object containing
/// itself — `illegal recursion in type 'm_Node'`, from Nim 2.2.10 on the
/// Linux host. The Zig and D columns refused the same case for the same
/// reason in the same run.
///
/// # Errors
/// As [`nim_member_type`].
fn held_shape(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<(nim::Type, Doc), MaterialiseError> {
    let mut doc = repair_note(&member.name);
    let mut type_ = nim_member_type(ast, target, &member.name)?;
    if super::held_externally(ast, member) {
        type_ = nim::Type::reference(type_);
        // Two short lines rather than one long one, here and below: a
        // note this long repeated over every field of an aggregate is
        // what a reader skips, and `Doc` holds lines so the renderer can
        // keep them at the field's own indentation.
        doc.push("Held by reference - @external, DDS-XTypes 1.3 Table 21. The");
        doc.push("indirection is the language mapping's and changes no octet.");
    }
    Ok((type_, doc))
}

/// The full Nim type one member declaration takes.
///
/// The reference goes on first and the `Option` outside it, so
/// `@optional @external Node next` is `Option[ref Node]`. That order is
/// the whole of it: the two annotations answer different questions —
/// Table 21 says where the member is held, §7.2.2.4.4.4.7 says whether
/// it is there at all — and the outer form has to be the one that can
/// say "absent". The other order, `ref Option[Node]`, is a reference to
/// an optional, which is neither.
///
/// A bare `ref T` with `nil` for the absent state would compile and is
/// refused for the reason this project refuses every silent
/// substitution: it is the same Nim type an `@external` member with no
/// `@optional` takes, so the generated file would no longer say which of
/// the two annotations the IDL wrote.
///
/// # Errors
/// As [`nim_member_type`].
fn member_shape(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<(nim::Type, Doc), MaterialiseError> {
    let (mut type_, mut doc) = held_shape(ast, member, target)?;
    if member.facets.is_optional() {
        // `Option[T]` and not a bare `ref T`. A `ref` on its own would
        // give the member reference semantics the rest of the object
        // does not have — two copies of a decoded value would share one
        // heap cell — and it would cost an allocation for every
        // absent-capable `boolean`. `Option[T]` keeps the value
        // semantics, and `isNone` is the one state no written value can
        // take: Nim's zero value is indistinguishable from a written
        // zero, so a `long` that arrived as 0 and one that did not
        // arrive would otherwise be equal.
        type_ = nim::Type::option(type_);
        // ASCII only, here and in every other string this column writes
        // into a comment: the renderer escapes a code point above 0x7F
        // to `\xHH` per octet, which is what a Nim string literal needs
        // and what a reader of a comment cannot read.
        doc.push("Optional. `none(T)` when the member is absent - DDS-XTypes 1.3");
        doc.push("7.2.2.4.4.4.7. In a @mutable type an absent member is not written");
        doc.push("at all; outside one its absence is written.");
    }
    Ok((type_, doc))
}

/// The key Nim would compare a member's Nim spelling by.
///
/// The collision check runs on this and not on the spelling, because
/// `foo_bar` and `fooBar` are two spellings and one identifier. See
/// [`zerodds_lang_cst::nim::normalise`].
fn field_key(name: &str) -> String {
    nim::normalise(&field_name(name))
}

/// The declaration's own IDL name, where the Nim spelling moved it.
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

/// The member's own IDL name, where the underscore repair moved it.
///
/// Only fires when it did. Nim's export marker renames nothing, so most
/// members carry no note at all — which is the difference from the Go
/// column, where every field needs an `idl` struct tag because the
/// capitalisation does not invert.
fn repair_note(declared: &str) -> Doc {
    idl_note(&field_name(declared), declared)
}

/// Record a module-scope name, refusing what Nim would take twice or
/// take out from under its own declarations.
///
/// Two checks, and neither is what [`no_spelling_collision`] does for
/// the members of one aggregate:
///
/// - **A second declaration reaching the same identifier.** One module
///   scope holds every declaration in the file, so this is a whole-file
///   check. Two IDL modules can flatten onto one Nim name (`a_b::C` and
///   `a::b_C` both give `a_b_C`), and so can two names Nim normalises
///   together (`a_b::C` and `ab::C`). Nim would report `redefinition of
///   'a_b_C'`, naming neither IDL declaration.
/// - **A declaration shadowing a name the file writes.** `seq`,
///   `string`, `Option` and the fixed-width integers are ordinary
///   module-scope symbols in Nim, not keywords, so a declaration may
///   shadow one and the file still parses — it stops compiling at the
///   next member declared with it. Stropping does not help, because
///   `` `seq` `` *is* `seq`, and a trailing-underscore escape does not
///   lex in this language, so the answer is to refuse by name.
fn declare_name(build: &mut Build, spelled: &str, declared: &str) -> Result<(), MaterialiseError> {
    let key = nim::normalise(spelled);
    if nim::PREDECLARED.contains(&key.as_str()) {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!(
                "a declaration named `{declared}`, which is the Nim name this file's own \
                 declarations are written in terms of"
            ),
            language: LANGUAGE,
            at: "the generated module".to_string(),
        });
    }
    if let Some((_, first)) = build.emitted.iter().find(|(other, _)| *other == key) {
        return Err(MaterialiseError::SpellingCollision {
            first: first.clone(),
            second: declared.to_string(),
            spelled: key,
            language: LANGUAGE,
            at: "the generated module".to_string(),
        });
    }
    build.emitted.push((key, declared.to_string()));
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

/// The Nim expression a constant's value takes.
///
/// Written bare rather than converted: unlike Go, Nim's aliases are
/// transparent — `type Distance = int32` makes the two one type — so a
/// value of the declared type needs no conversion at the declaration,
/// and the constant's own `: T` annotation says which type it has.
///
/// `long double` is refused by name. Its Nim holder is a `string`
/// carrying the decimal, and CORBA gives a `long double` literal no
/// spelling that survives a `float64`: emitting one would put a
/// different number in the file than the IDL wrote.
fn nim_constant(
    value: &zerodds_ast::ConstValue,
    type_: &nim::Type,
    at: &str,
) -> Result<nim::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_string = matches!(type_, nim::Type::Primitive(nim::Primitive::String));
    Ok(match value {
        ConstValue::Boolean(flag) => nim::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => nim::Expr::Literal(Literal::int(*number)),
        // The code point as a number: an IDL `char` is one octet here
        // and an IDL `wchar` two, and Nim's `char` is neither — it is
        // one byte with no arithmetic, which is why the member's type is
        // a `uint8` and the literal has to be one too.
        ConstValue::Char(character) => {
            nim::Expr::Literal(Literal::int(i128::from(u32::from(*character))))
        }
        ConstValue::WChar(point) => nim::Expr::Literal(Literal::int(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            if is_string {
                return Err(refuse("a `long double` constant, which has no Nim literal"));
            }
            nim::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            nim::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => nim::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => nim::Expr::Literal(Literal::Str(
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
