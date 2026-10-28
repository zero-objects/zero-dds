// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The D CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::c`] and [`crate::adapter::go`].
//! Reading the three side by side is the point: all three flatten a
//! module onto the name, D and C both use overlaid storage for a union
//! where Go cannot, and everything else D does differently is D's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The five things D decides that C does not
//!
//! 1. **Declaration order does not matter.** A name declared at module
//!    scope is in scope for the whole module however far down it is
//!    written (D language specification, "Modules"), so a struct may
//!    name a type declared below it. The C column has to walk the type
//!    graph post-order for exactly that reason; this one walks the model
//!    in declaration order, so the generated file tracks the source.
//! 2. **A collection needs no declared type.** `T[]` and `T[N]` are
//!    written where the member is, so `sequence<long>` produces no
//!    declaration at all. Only a `map<K,V>` does, because its entry is a
//!    struct and an anonymous one repeated per use would be a shape a
//!    reader cannot name.
//! 3. **An enumeration is one declaration.** A D `enum` is a named
//!    scope, so `Mode.IDLE` needs no prefix and the enumerators need no
//!    constants of their own. C and Go both need a type plus one
//!    constant per enumerator, and both have to prefix those constants
//!    with the module path to keep two enumerations apart.
//! 4. **There is no visibility question.** D's default protection is
//!    `public` (D language specification, "Attributes", "Protection
//!    Attributes"), so nothing is written for it — where the Go column
//!    has to move the first letter of every declared name because
//!    export is the case of one letter.
//! 5. **The annotation is a UDA and not a comment.** `@("idl:x")` is
//!    read back with `__traits(getAttributes, S.x)` at compile time. The
//!    C column has to put this in a macro beside the declaration; here
//!    it is part of the field.
//!
//! ## The memory shapes
//!
//! | IDL | D |
//! |---|---|
//! | `boolean` | `bool` |
//! | `octet`, `uint8`, `char` | `ubyte` |
//! | `int8` | `byte` |
//! | `short` / `unsigned short` | `short` / `ushort` |
//! | `wchar` | `ushort` |
//! | `long` / `unsigned long` | **`int` / `uint`** |
//! | `long long` / `unsigned long long` | `long` / `ulong` |
//! | `float` / `double` | `float` / `double` |
//! | `long double` | `string`, holding the decimal |
//! | `string`, `wstring` | `string` |
//! | `fixed<P,S>` | `string`, holding the decimal |
//! | `sequence<T>` | `T[]` |
//! | `T[N][M]` | **`T[M][N]`**, dimensions reversed |
//! | `map<K,V>` | `ZdMapEntry_K_V[]`, **not** an associative array |
//! | `struct`, `valuetype`, `exception`, `bitset` | `struct` |
//! | `union` | `struct { D _d; union { … } }` |
//! | `@optional T m` | `Nullable!(T)` |
//! | `@external T m` | `T*` |
//! | `@optional @external T m` | `T*`, `null` when absent |
//! | `enum` | `enum Name : int { … }` |
//! | `bitmask` | `alias Name = uintN;` and manifest constants |
//! | `typedef` | `alias Name = T;` |
//! | `native` | `alias Name = void*;` |
//! | `const` | `enum T NAME = value;` |
//! | `interface` | `interface Name { … }` |
//!
//! ### An IDL `long` is a D `int`, and an IDL `long long` is a D `long`
//!
//! The most likely defect in this column. An IDL `long` is 32 bits and
//! an IDL `long long` 64 (IDL 4.2 §7.4.1.4.4.2); a D `int` is 32 bits
//! and a D `long` is 64 (D language specification, "Types"). The two
//! languages use the word `long` for different widths, so a mapping that
//! followed the spelling would double the width of every 32-bit member
//! and halve nothing — a struct that compiles, encodes four extra octets
//! per member, and is read by no peer.
//!
//! ### An array's dimensions are written in the reverse order
//!
//! The second most likely defect. In D `int[3][2]` is **two arrays of
//! three** — the suffixes read right to left (D language specification,
//! "Arrays", "Rectangular Arrays") — where C's `int a[3][2]` is three
//! arrays of two. IDL `long grid[2][3]` is two rows of three
//! (IDL 4.2 §7.4.1.4.4.4), so the D declaration is `int[3][2] grid`.
//!
//! The tree holds the outermost dimension outermost, exactly as the Go
//! tree does; `zerodds_lang_render::d` writes the element before the
//! length, and that is where the reversal happens. Building the tree the
//! other way round would transpose every matrix in the corpus with
//! nothing in the output to say so.
//!
//! ### `struct` and never `class`
//!
//! A D `class` is a reference type with a vtable pointer and a monitor
//! field, allocated on the GC heap (D language specification,
//! "Classes"). The wire form of an IDL aggregate is a value: a sequence
//! of members with no header of its own. A column that made it a `class`
//! would put two machine words in front of the first member, add a null
//! state no IDL construct asked for, and make assignment share storage
//! rather than copy it.
//!
//! ### `map<K,V>` is a slice of entries and never `V[K]`
//!
//! A D associative array has no defined iteration order — `.byKeyValue`
//! walks the hash table's buckets — and a `map<K,V>` is written to the
//! wire as a sequence of key/value pairs (DDS-XTypes 1.3 §7.4.1.1.4). A
//! column holding it in a `V[K]` would produce a different octet stream
//! for one value depending on the insertion history of the table, and
//! the KeyHash of a keyed type would follow it.
//!
//! ### `long double` is a `string` and never a `real`
//!
//! D's `real` is the largest floating-point type the target implements:
//! 80-bit x87 extended on x86 and x86-64, and the same as `double` — 64
//! bits, 53 of significand — on AArch64, PowerPC and every other target
//! without an x87 (D language specification, "Types",
//! "Floating Point"). An IDL `long double` is a binary128 with 113 bits
//! of significand. `real` would therefore lose 49 bits on x86 and 60 on
//! AArch64, and lose a different number on each, which is the reason the
//! C column refuses the compiler's `long double` as well. The member
//! holds the decimal text instead, as the Go and Python columns do.
//!
//! ### `@optional T m` is `Nullable!(T)` and not `T*`
//!
//! Both express absence (DDS-XTypes 1.3 §7.2.2.4.4.4.7); they differ in
//! what they do to the member.
//!
//! `T*` adds an indirection and a heap allocation to a member that had
//! neither, and it is not uniform: `T` here is sometimes a value
//! (`int`), sometimes already a reference (`string`, `T[]`), and a
//! `string*` is a pointer to a slice — two indirections to reach the
//! first octet. `Nullable!(T)` from `std.typecons` is a struct holding a
//! `T` and a `bool`, so the member keeps its own storage whatever `T`
//! is, and the absent state is `isNull` rather than a dereference that
//! is undefined when it is wrong.
//!
//! ### `@external T m` is `T*`, and it is the pointer that makes the
//! recursive case finite
//!
//! `@external` says the member is stored **outside** the containing
//! object (DDS-XTypes 1.3 Table 21). It changes no octet on the wire;
//! it is a statement about where the language binding keeps the value,
//! and in D a pointer is the whole of it — no box type has to be
//! invented for it.
//!
//! Honoured for every `@external` member and not only for the ones that
//! would not otherwise compile. Six corpus cases carry the annotation —
//! on a primitive, on a `sequence`, on a `string`, on an aggregate, and
//! on a self-recursive member — and only the last of those needs the
//! indirection to have a size at all. A mapping that produced a pointer
//! only where a cycle forced one would be answering the cycle rather
//! than the annotation, and would put the same IDL member in two
//! different shapes depending on what else the file declared.
//!
//! **Measured.** `tests/interop-corpus/recursive-optional-external`
//! declares `@optional @external Node next` inside `Node`. Held inline
//! as `Nullable!(m_Node)` the struct contains itself, and LDC 1.40.0
//! refuses it: *unable to determine fields of `m_Node` because of
//! forward references*, from `core.internal.traits` instantiating
//! `Nullable!(m_Node)`. `Nullable!T` stores its `T` by value, so the
//! optional alone does not break the cycle — the pointer does. The Zig
//! column met the same case for the same reason.
//!
//! ### `@optional @external T m` is one pointer and not `Nullable!(T*)`
//!
//! A pointer already carries a state no written value can take, which
//! is the argument the Go column makes for its own optional: `null`
//! means absent. `Nullable!(T*)` would carry two — `isNull`, and
//! present-but-`null` — and the second is a state the wire form has no
//! encoding for. A value in it would be written as a present member
//! whose contents cannot be read, which is the silent-substitution class
//! this project is arranged against.
//!
//! So `@external` alone and `@optional @external` reach the same D type,
//! and the difference between them is in the member's documentation and
//! in the model the codec is generated from, not in the storage. That is
//! the same answer the Go column gives, where `@optional T` is `*T` and
//! there is nothing else for `@external` to add.
//!
//! ### The union is a real D `union`
//!
//! `struct Payload { Mode _d; union { int idle; double active; } }` —
//! the branches overlay, which is the shape the wire form has: only the
//! branch the discriminator selects is written
//! (DDS-XTypes 1.3 §7.4.1.1.5.2). This is the C column's answer. The Go
//! column puts the branches side by side only because Go has no overlaid
//! storage at all, and pays for it with a value as large as the sum of
//! its branches.
//!
//! **What `@safe` means for a reader.** D forbids, in `@safe` code,
//! reading a union field that overlaps a pointer or a reference
//! (D language specification, "Memory Safety"). A branch of
//! type `string` or `T[]` is a pointer and a length, so any union with
//! such a branch can only be read from `@system` or `@trusted` code. The
//! correctness argument is the discriminator: a reader that switches on
//! `_d` and touches only the branch it names is `@trusted`, and one that
//! does not is undefined however it is annotated. A union all of whose
//! branches are scalars stays readable from `@safe` code.
//!
//! ### Where the IDL spelling of a member is kept
//!
//! [`d::escape_name`] moves a name that collides with a D keyword or
//! with one of the module-scope names a generated file relies on, and it
//! does not invert: `version_` could have come from `version` or from
//! `version_`. DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's ID under
//! `@autoid(HASH)` and under `@hashid` from the first four octets of the
//! MD5 of its **IDL** name, so a file that only knows the D spelling
//! cannot compute the ID for the member it is writing.
//!
//! Every field therefore carries `@("idl:…")`, and every declaration
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
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::d;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "D";

/// The module a generated file declares.
///
/// One name for every generated file, because D's correspondence between
/// a module and a file is one-to-one and the whole model lands in one
/// file. `zdtypes` and not `types`: a module called `types` would
/// collide with any other `types` module on the import path, and D
/// resolves a top-level module name globally.
pub const MODULE: &str = "zdtypes";

/// The prefix a name this column invented carries.
///
/// Only a map entry needs one — every other D shape is written where the
/// member is.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// The D field name of the union discriminator.
///
/// `_d`, as in every column but Go's — Go cannot export a name beginning
/// with an underscore and D has no such rule, so the IDL-facing spelling
/// is kept. IDL 4.2 §7.4.1.4.4.1 gives the discriminator no member name
/// of its own, which is why this is stated here rather than assumed.
pub const DISCRIMINATOR: &str = "_d";

/// Build the D declarations a rule application produced.
///
/// The order is the model's own declaration order. D makes every
/// module-scope declaration visible throughout the module regardless of
/// where it appears, so unlike the C column there is no dependency walk
/// and the output tracks the source.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no D shape, or two declarations reach one D
/// spelling — never a silently dropped declaration.
pub fn d_items(ast: &Ast, applied: &Applied) -> Result<Vec<d::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, node) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::D),
            &declared_name(ast, &node.name, node.module),
            start,
            &mut out,
        );
    }

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "DConst").is_none() {
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
            Some(target) => d_value_type(ast, target)?,
            None => d::Type::Primitive(d::Primitive::Int),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(d::Decl::Const(d::ConstDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            type_: Some(type_.clone()),
            value: d_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "DError").is_none() {
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
        let mut members = Vec::new();
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
            members.push(d::Member::Field(member_field(ast, member_node, target)?));
        }
        out.push(d::Decl::Struct(d::StructDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An exception, as the structure its members make. A struct and not a",
                    "class deriving Throwable: what goes on the wire is the member list,",
                    "and the throwing is the RPC arc's business.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            members,
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "DInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut methods = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. `d::InterfaceDecl`
        // has no base list at all, so a derived interface has nothing to
        // reach OMG IDL 4.2 §7.4.3's inherited operations and attributes
        // through and they are written into it.
        // `interface::all_operations_flattened` gives both — the
        // get/set pair DDS-RPC 1.0 §7.5.1.1.3 implies for each
        // attribute, besides the declared operations — which
        // `Ast::all_operations_of` alone never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            // An attribute's own name for the getter, `set` plus it for
            // the setter — the same pair Go's, Swift's and Nim's
            // bindings take for the same reason — rather than
            // `all_operations_flattened`'s `get_`/`set_` prefix, which
            // spells out DDS-RPC 1.0's own implied-operation name and
            // not a D one. D reads a `T name()`/`void name(T)` pair as
            // a property through UFCS without an explicit `@property`.
            let field_ident = match op.origin {
                interface::Origin::AttributeGetter => field_name(op.attribute_name()),
                interface::Origin::AttributeSetter => {
                    let attr = op.attribute_name();
                    format!("set{}", capitalised(&field_name(attr)))
                }
                interface::Origin::Declared => field_name(&op.name),
            };
            signatures.push((field_ident.clone(), op.name.clone()));
            let mut params = Vec::new();
            for param in &op.params {
                params.push(d::Param {
                    name: Ident::new(&d::escape_name(&param.name)),
                    type_: d_member_type(ast, param.type_, &param.name)?,
                });
            }
            let returns = match op.returns {
                Some(target) => Some(d_member_type(ast, target, &op.name)?),
                None => None,
            };
            let doc = op
                .raises_line()
                .map_or_else(Doc::none, |line| Doc::line(&line));
            methods.push(d::Method {
                doc,
                name: Ident::new(&field_ident),
                returns,
                params,
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(d::Decl::Interface(d::InterfaceDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface, as the method set its operations and attributes make",
                    "— an attribute a `name`/`setName` pair, `readonly` only the first.",
                    "A D interface declares signatures and nothing else, which is what",
                    "an IDL interface declares; the transport is the RPC arc's",
                    "business. `raises`/`getraises`/`setraises` are documented on the",
                    "method they belong to.",
                    "",
                    "A derived interface carries its bases' operations and attributes",
                    "too: this declaration has no base list, so IDL 4.2 7.4.3's",
                    "inherited members are written into it rather than reached through",
                    "one.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            methods,
        }));
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::D, out))
}

/// `@verbatim` in the D column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close the struct's body; an enum, an alias
/// and an interface take them adjacent.
impl verbatim::Carrier for d::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Struct(node) = self else {
            return false;
        };
        if node.name.as_str() != name {
            return false;
        }
        let mut members: Vec<d::Member> = begin.drain(..).map(d::Member::Verbatim).collect();
        members.append(&mut node.members);
        members.extend(end.drain(..).map(d::Member::Verbatim));
        node.members = members;
        true
    }
}

/// The D declarations, in a file that names its module.
///
/// A caller that only wants the declarations uses [`d_items`]; this is
/// what a generator writes to disk, and it exists here so that the
/// module name and the import list are decided in one place.
///
/// # Errors
/// As [`d_items`].
pub fn d_file(ast: &Ast, applied: &Applied) -> Result<d::File, MaterialiseError> {
    let decls = d_items(ast, applied)?;
    // The import is written only where something in the file needs it.
    // `import std.typecons : Nullable;` in a file with no optional
    // member would make every consumer of the generated module link
    // Phobos for a name it does not use.
    let mut imports = Vec::new();
    if decls.iter().any(decl_uses_nullable) {
        imports.push(d::Import::selective(&["std", "typecons"], "Nullable"));
    }
    Ok(d::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "The IDL spelling of every renamed declaration is in the comment",
            "above it; the IDL spelling of every member is in its @(\"idl:...\")",
            "attribute, because a member's ID under @autoid(HASH) is the MD5 of",
            "the IDL name (DDS-XTypes 1.3 7.3.1.2.1.1) and the D spelling does",
            "not invert.",
            "",
            "An IDL `long` is 32 bits and is written `int` here; a D `long` is",
            "64 bits and is what an IDL `long long` becomes.",
        ]),
        module: Path::new(&[MODULE]),
        imports,
        decls,
    })
}

/// Whether a declaration mentions `Nullable!(T)` anywhere.
fn decl_uses_nullable(decl: &d::Decl) -> bool {
    match decl {
        d::Decl::Struct(node) => node.members.iter().any(|member| match member {
            d::Member::Field(field) => type_uses_nullable(&field.type_),
            d::Member::Union(fields) => fields.iter().any(|f| type_uses_nullable(&f.type_)),
            d::Member::Verbatim(_) => false,
        }),
        d::Decl::Alias(node) => type_uses_nullable(&node.type_),
        d::Decl::Const(node) => node.type_.as_ref().is_some_and(type_uses_nullable),
        d::Decl::Interface(node) => node.methods.iter().any(|method| {
            method.returns.as_ref().is_some_and(type_uses_nullable)
                || method.params.iter().any(|p| type_uses_nullable(&p.type_))
        }),
        // A function's body holds statements and no member types; the
        // codec routines the probe appends never declare a Nullable of
        // their own.
        d::Decl::Func(_) | d::Decl::Enum(_) | d::Decl::Comment(_) | d::Decl::Verbatim(_) => false,
    }
}

/// zerodds-lint: recursion-depth 64 (element nesting of an owned
/// `d::Type`). The tree is the one the materialiser built and owns, so
/// it holds no cycle; the depth is the declared type's height, which
/// `value_type_at` already bounded at `MAX_TYPE_DEPTH`.
fn type_uses_nullable(type_: &d::Type) -> bool {
    match type_ {
        d::Type::Nullable(_) => true,
        d::Type::Slice(inner) | d::Type::Pointer(inner) => type_uses_nullable(inner),
        d::Type::Array { element, .. } => type_uses_nullable(element),
        d::Type::Primitive(_) | d::Type::Named(_) => false,
    }
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<d::Decl>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence and an array are
        // all written where the member is: D spells each of them inline,
        // so none of them is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // The one collection that does need a name. An anonymous
        // `struct { K key; V value; }` cannot be written in a member's
        // type position in D at all, and a repeated named one would be a
        // second declaration of the same shape.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: d_type_name(ast, id),
                });
            };
            let name = map_entry_name_at(ast, id, 0)?;
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            out.push(d::Decl::Struct(d::StructDecl {
                doc: Doc::lines(&[
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a",
                    "sequence of these, and the rules write the two halves with no",
                    "framing between them.",
                ]),
                name: Ident::new(&name),
                members: vec![
                    d::Member::Field(d::Field {
                        doc: Doc::none(),
                        udas: Vec::new(),
                        name: Ident::new("key"),
                        type_: d_member_type(ast, key, &name)?,
                    }),
                    d::Member::Field(d::Field {
                        doc: Doc::none(),
                        udas: Vec::new(),
                        name: Ident::new("value"),
                        type_: d_member_type(ast, value, &name)?,
                    }),
                ],
            }));
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "DAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = d_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // `alias Name = T;`, which is another name for one type and
            // not a new one — IDL 4.2 §7.4.1.4.3: the alias "does not
            // define a new type". D has no declaration that would make a
            // distinct type of it short of a wrapper struct, so there is
            // nothing here to get wrong; the note is for the reader
            // coming from the Go column, where `type A B` and
            // `type A = B` are two different declarations.
            out.push(d::Decl::Alias(d::AliasDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                type_: d_member_type(ast, target, &node.name)?,
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "DEnum").is_none() {
                return Ok(());
            }
            let name = d_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            let mut members = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "DEnumMember").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                pairs.push((field_name(&enum_node.name), enum_node.name.clone()));
                members.push(d::EnumMember {
                    doc: idl_note(&field_name(&enum_node.name), &enum_node.name),
                    name: Ident::new(&field_name(&enum_node.name)),
                    // Written out. D numbers an unvalued member one above
                    // its predecessor and `@value` admits gaps, so a
                    // member left unvalued would renumber the wire.
                    value: d::Expr::int(value),
                });
            }
            // The enumerators are in the enum's own scope in D —
            // `Mode.IDLE` — so the check is per enumeration and not
            // against the module, which is the difference from the C and
            // Go columns, where every enumerator is a module-scope
            // constant.
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let doc = with_idl_note(
                Doc::lines(&[
                    "An enumeration. The base type is signed because DDS-XTypes 1.3",
                    "7.4.3.5.3 makes it so; the width on the wire is the @bit_bound's",
                    "and is applied by the runtime.",
                ]),
                &name,
                &node.name,
            );
            if members.is_empty() {
                // D rejects an enum body with no members. IDL 4.2
                // §7.4.1.4.4.3 requires at least one enumerator, so this
                // is a model that lost one rather than a source that
                // wrote one; an alias to the holder is emitted so that
                // members naming the type still compile, and the
                // enumeration's absence is visible in the output rather
                // than as a failure four hundred lines further down.
                out.push(d::Decl::Alias(d::AliasDecl {
                    doc,
                    name: Ident::new(&name),
                    type_: d::Type::Primitive(d::Primitive::Int),
                }));
            } else {
                out.push(d::Decl::Enum(d::EnumDecl {
                    doc,
                    name: Ident::new(&name),
                    base: Some(d::Type::Primitive(d::Primitive::Int)),
                    members,
                }));
            }
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "DBitmask").is_none() {
                return Ok(());
            }
            let name = d_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            // An alias and not a D `enum`: an `enum` is a closed set of
            // named values, and a bitmask value is any combination of
            // its bits (DDS-XTypes 1.3 §7.4.1.1.3.2). A D enum would
            // make `READY | MOVING` a value of no enum member, which is
            // exactly the value a bitmask exists to carry.
            out.push(d::Decl::Alias(d::AliasDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3",
                        "7.4.1.1.3.2 gives for its bound. An alias and not an enum: a",
                        "bitmask value is any combination of its bits, and a D enum is a",
                        "closed set of named values.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                type_: d::Type::Primitive(d_holder(bound)),
            }));
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "DBitConst").is_none() {
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
                out.push(d::Decl::Const(d::ConstDecl {
                    doc: idl_note(&constant, &bit_node.name),
                    name: Ident::new(&constant),
                    type_: Some(d::Type::named(&name)),
                    value: d::Expr::Binary {
                        op: "<<",
                        lhs: Box::new(shift_base(position)),
                        rhs: Box::new(d::Expr::int(i128::from(position))),
                    },
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "DBitset").is_none() {
                return Ok(());
            }
            let name = d_type_name(ast, id);
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
            let mut members = Vec::new();
            let mut index = 0;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                members.push(d::Member::Field(d::Field {
                    doc: Doc::none(),
                    udas: vec![d::Uda::keyed("idl", field_ident)],
                    name: Ident::new(&pairs[index].0),
                    type_: d::Type::Primitive(d_holder(field_node.bits)),
                }));
                index += 1;
            }
            out.push(d::Decl::Struct(d::StructDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate integers here rather than D",
                        "bitfields: the order on the wire is the declaration's, and D's",
                        "own bit-field packing is the target ABI's.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                members,
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "DValue"
            } else {
                "DStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = d_type_name(ast, id);
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
                fields.push(d::Member::Field(parent_field(ast, base)?));
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
                fields.push(d::Member::Field(member_field(ast, member_node, target)?));
            }
            out.push(d::Decl::Struct(d::StructDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                members: fields,
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "DUnion").is_none() {
                return Ok(());
            }
            let name = d_type_name(ast, id);
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
            // `_d` would take the same D spelling and make one of the two
            // unreachable. The anonymous union puts its fields directly
            // on the enclosing struct, so the two share one scope.
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((field_name(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut branches = Vec::new();
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "DCase").is_none() {
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
                branches.push(member_field(ast, member, target)?);
            }
            let members = vec![
                d::Member::Field(d::Field {
                    doc: Doc::line("Which branch carries the value."),
                    udas: Vec::new(),
                    name: Ident::new(DISCRIMINATOR),
                    type_: d_member_type(ast, disc, &name)?,
                }),
                d::Member::Union(branches),
            ];
            out.push(d::Decl::Struct(d::StructDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union: the branches overlay, which is the shape the wire form",
                        "has — only the branch `_d` selects is written (DDS-XTypes 1.3",
                        "7.4.1.1.5.2). Reading a branch other than the one `_d` names is",
                        "undefined; where a branch holds a slice or a string the union",
                        "overlaps a reference, which D permits only from @system or",
                        "@trusted code.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                members,
            }));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "DNative").is_none() {
                return Ok(());
            }
            let name = d_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `void*` when there is none. `void*`
            // and not `Object`: an IDL `native` names a type the binding
            // supplies (IDL 4.2 §7.4.1.4.3.2), and D's `Object` would
            // add a vtable and a GC root to something whose contents the
            // IDL does not describe.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(d::Decl::Alias(d::AliasDecl {
                doc: with_idl_note(
                    Doc::line("A native type. Opaque unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                type_: match mapping {
                    Some(spelling) => d::Type::named(&spelling),
                    None => d::Type::pointer(d::Type::Primitive(d::Primitive::Void)),
                },
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = d_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            // `Nullable!(T)` and not `T*`, for the reason an `@optional`
            // member takes it: one representation of absence per column,
            // and one that does not change what the member's storage is.
            out.push(d::Decl::Alias(d::AliasDecl {
                doc: with_idl_note(
                    Doc::line("A boxed value: present or not, and `isNull` when it is not."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                type_: d::Type::nullable(d_member_type(ast, target, &name)?),
            }));
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `d_member_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The literal a bitmask's shift starts from.
///
/// `1` is an `int` in D, and a shift count outside `0 .. 31` on an `int`
/// is rejected at compile time (D language specification,
/// "Expressions"). A bit at position 31 or above therefore shifts
/// a `ulong`: at 31 the `int` result would already be the sign bit, and
/// above 63 the source is not a bitmask any `@bit_bound` admits.
fn shift_base(position: u16) -> d::Expr {
    if position >= 31 {
        d::Expr::suffixed_int(1, "UL")
    } else {
        d::Expr::int(1)
    }
}

/// Whether `type_` reaches `enclosing` through nothing but aliases and
/// array dimensions — the chain along which a D field holds its type
/// *by value*. A sequence is a D slice, a string owns its own storage,
/// a map is an associative array: each is an indirection and ends the
/// chain. Bounded, because a chain longer than this is a model defect
/// and not a spelling question.
fn reaches_enclosing_by_value(ast: &Ast, mut type_: Id<TypeNode>, enclosing: Id<TypeNode>) -> bool {
    for _ in 0..64 {
        if type_ == enclosing {
            return true;
        }
        let Ok(node) = ast.type_(type_) else {
            return false;
        };
        let next = match &node.kind {
            TypeKind::Alias { aliased } => aliased.resolved(),
            TypeKind::Array { element, .. } => element.resolved(),
            _ => return false,
        };
        match next {
            Some(next) => type_ = next,
            None => return false,
        }
    }
    false
}

/// The field one member declaration becomes.
///
/// One function for every position a member appears in — a struct, a
/// valuetype, an exception, a union branch — so that `@external` and
/// `@optional` cannot be honoured in one of them and forgotten in
/// another.
///
/// The two annotations interact, and the interaction is the whole of
/// this function:
///
/// - **`@external`** says the member is stored outside the containing
///   object (DDS-XTypes 1.3 Table 21). In D that is `T*` and nothing
///   else: no box type has to be invented, and the indirection is what
///   gives a self-recursive type a size.
/// - **`@optional`** says the member may be absent
///   (DDS-XTypes 1.3 §7.2.2.4.4.4.7). On its own that is `Nullable!(T)`,
///   which keeps the member's own storage whatever `T` is.
/// - **Both together** is one pointer, `null` when absent, and **not**
///   `Nullable!(T*)`. The pointer already carries a state no written
///   value can take. `Nullable!(T*)` would carry two — `isNull`, and
///   present-but-`null` — and the second is a state the wire form has no
///   encoding for, so a value in it would be written as a present member
///   whose contents cannot be read.
///
/// Measured: `tests/interop-corpus/recursive-optional-external` declares
/// `@optional @external Node next` inside `Node`. Held inline as
/// `Nullable!(m_Node)` the struct contains itself and LDC 1.40.0 refuses
/// it — *unable to determine fields of `m_Node` because of forward
/// references*, raised from `core.internal.traits` instantiating
/// `Nullable!(m_Node)`, because `Nullable!T` stores its `T` by value. The
/// optional alone does not break the cycle; the pointer does.
fn member_field(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<d::Field, MaterialiseError> {
    let mut type_ = d_member_type(ast, target, &member.name)?;
    let mut doc = Doc::none();
    let external = super::held_externally(ast, member);
    let optional = member.facets.is_optional();
    // A member whose type reaches the enclosing aggregate back through
    // nothing but aliases and array dimensions, and which is neither
    // `@optional` nor `@external`, is the only member on its own cycle
    // and nothing on that cycle can end a value — `m_Node[2]` inside
    // `m_Node`. IDL 4.2 §7.4.1.4.4.4.4 does not admit it: an array has
    // no length on the wire to terminate the cycle. Refused by name here
    // so the generated file cannot carry a declaration LDC would refuse;
    // `tests/interop-corpus/recursive-array` is the corpus form.
    //
    // An `@optional` member on the same chain is the legal kind — its
    // absence ends the value, XTypes 1.3 §7.3.1.2.1.3 — and
    // [`super::held_externally`] has already made it a pointer, which is
    // what gives the struct a size. This arm refused it as well until
    // that rule existed, citing a clause the type does not break.
    if !super::annotated_external(member)
        && !optional
        && let zerodds_ast::AnyId::Type(enclosing, _) = member.parent
        && reaches_enclosing_by_value(ast, target, enclosing)
    {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!(
                "a member whose type reaches `{}` back with no indirection — a D \
                 struct cannot contain itself by value, and IDL 4.2 §7.4.1.4.4.4.4 \
                 admits no cycle that nothing on it can terminate",
                d_type_name(ast, enclosing)
            ),
            language: LANGUAGE,
            at: member.name.clone(),
        });
    }
    if external {
        type_ = d::Type::pointer(type_);
        doc.push("Stored by reference — @external, DDS-XTypes 1.3 Table 21. The");
        doc.push("annotation changes no octet on the wire; it says where the binding");
        doc.push("keeps the value, and in D a pointer is the whole of it.");
    }
    if optional {
        if external {
            // No `Nullable` over the pointer: two absence states, one of
            // which the wire form cannot encode.
            doc.push("Optional. `null` when the member is absent — DDS-XTypes 1.3");
            doc.push("7.2.2.4.4.4.7. The pointer above carries that state already, so no");
            doc.push("Nullable is wrapped around it.");
        } else {
            type_ = d::Type::nullable(type_);
            doc.push("Optional. `isNull` when the member is absent — DDS-XTypes 1.3");
            doc.push("7.2.2.4.4.4.7. In a @mutable type an absent member is not written");
            doc.push("at all; outside one its absence is written.");
        }
    }
    Ok(d::Field {
        doc,
        udas: vec![d::Uda::keyed("idl", &member.name)],
        name: Ident::new(&field_name(&member.name)),
        type_,
    })
}

/// The field that holds the base of a derived structure: the first member `parent`
/// (XTypes 1.3 §7.5.1.4.1.3).
fn parent_field(ast: &Ast, base: Id<TypeNode>) -> Result<d::Field, MaterialiseError> {
    let mut doc = Doc::none();
    doc.push("The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).");
    Ok(d::Field {
        doc,
        udas: vec![d::Uda::keyed("idl", "parent")],
        name: Ident::new(&field_name("parent")),
        type_: d_member_type(ast, base, "parent")?,
    })
}

/// The D spelling of a value's type, refusing what D cannot hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided at
/// run time — D's `Variant` would compile and there would be no wire
/// form to generate a codec against, which is the silent-substitution
/// class this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn d_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<d::Type, MaterialiseError> {
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
    d_value_type(ast, target)
}

/// The D type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated struct, a
/// codec's parameter and a defaulter's destination all have to name the
/// same D type or the file does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when an array length or a
/// collection's element does not resolve.
pub fn d_value_type(ast: &Ast, id: Id<TypeNode>) -> Result<d::Type, MaterialiseError> {
    value_type_at(ast, id, 0)
}

/// [`d_value_type`], carrying how many connector steps the walk has
/// already taken.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the spelling nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array, map and
/// anonymous-alias nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn value_type_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<d::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose D spelling nests past {MAX_TYPE_DEPTH}"),
            at: d_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(d::Type::pointer(d::Type::Primitive(d::Primitive::Void)));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => d_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units, which puts
        // the §7.4.1.1.2 surrogate rule inside the D runtime where it is
        // a second implementation of the clause and not a copy of the
        // Rust column's answer. A D `string` is `immutable(char)[]` —
        // UTF-8 by definition — which is what makes that possible, and
        // is also why a `wstring` here is a D `string` and not a D
        // `wstring`: D's `wstring` is `immutable(wchar)[]`, UTF-16, and
        // holding UTF-16 would put the surrogate arithmetic in the
        // column instead of the runtime.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            d::Type::Primitive(d::Primitive::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            d::Type::slice(value_type_at(ast, element, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // Outermost dimension outermost in the tree; the renderer
            // writes the element first, so `long grid[2][3]` comes out
            // as `int[3][2]`, which D reads as two arrays of three.
            // Building it from the inside out is what keeps that true.
            let mut type_ = value_type_at(ast, element, depth + 1)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: node.name.clone(),
                        })?;
                type_ = d::Type::array(length, type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            d::Type::slice(d::Type::named(&map_entry_name_at(ast, id, depth + 1)?))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the D type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_type_at(ast, target, depth + 1)?,
            None => d::Type::pointer(d::Type::Primitive(d::Primitive::Void)),
        },
        _ => d::Type::named(&d_type_name(ast, id)),
    })
}

/// The D type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a D spelling is
/// a compile error and not a member that silently takes another's.
///
/// The two that have to be read carefully: **`long` is `int`** and
/// **`long long` is `long`**. IDL 4.2 §7.4.1.4.4.2 makes an IDL `long`
/// 32 bits and an IDL `long long` 64; the D language specification makes
/// a D `int` 32 and a D `long` 64. Following the spelling instead of the
/// width would double every 32-bit member.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open. D's `char` and `wchar` are UTF-8 and UTF-16 **code units**, so
/// neither is the IDL type of the same name: an arbitrary octet assigned
/// to a D `char` is accepted and then produces an invalid UTF-8 sequence
/// the moment anything in Phobos reads it. `ubyte` and `ushort` carry
/// the value with no encoding attached.
///
/// `long double` is a `string` and never a `real`; the module
/// documentation carries why.
#[must_use]
pub fn d_primitive_type(kind: PrimitiveKind) -> d::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => d::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => d::Primitive::UByte,
        PrimitiveKind::Int8 => d::Primitive::Byte,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => d::Primitive::UShort,
        PrimitiveKind::Int16 => d::Primitive::Short,
        PrimitiveKind::Int32 => d::Primitive::Int,
        PrimitiveKind::UInt32 => d::Primitive::UInt,
        PrimitiveKind::Int64 => d::Primitive::Long,
        PrimitiveKind::UInt64 => d::Primitive::ULong,
        PrimitiveKind::Float32 => d::Primitive::Float,
        PrimitiveKind::Float64 => d::Primitive::Double,
        PrimitiveKind::Float128 => d::Primitive::String,
    };
    d::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for.
#[must_use]
pub const fn d_holder(bits: u16) -> d::Primitive {
    match bits {
        0..=8 => d::Primitive::UByte,
        9..=16 => d::Primitive::UShort,
        17..=32 => d::Primitive::UInt,
        _ => d::Primitive::ULong,
    }
}

/// A declared type's D identifier.
///
/// The scoped name with the module separators flattened onto it, which
/// is what one module per file requires: `::common::util::Location` is
/// `Common_Util_Location`.
#[must_use]
pub fn d_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
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
/// built from the IDL kind and not the D spelling: `double` and
/// `long double` are `double` and `string` here, but `octet` and `uint8`
/// are both `ubyte`, and a name built from the D type would give
/// `map<octet,V>` and `map<uint8,V>` one declaration — harmless today
/// and the same collapse that would be wrong if the two ever differed on
/// the wire.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a key's or a value's tag
/// nests past `MAX_TYPE_DEPTH`.
pub fn d_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    map_entry_name_at(ast, id, 0)
}

/// [`d_map_entry_name`], carrying the depth of the walk that reached it.
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
            at: d_type_name(ast, id),
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
            at: d_type_name(ast, id),
        });
    }
    Ok(match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => capitalised(&kind.as_str().replace(' ', "_")),
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
        _ => d_type_name(ast, id),
    })
}

/// The first character upper-cased, for a name this column invented.
///
/// D attaches no meaning to the case of an identifier — it has nothing
/// like Go's export rule — so this changes nothing the compiler sees. It
/// is here so that `ZdMapEntry_Long_String` reads as segments and so
/// that the invented names of this column and the Go column can be
/// compared line for line.
fn capitalised(text: &str) -> String {
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

/// The flattened module path a module-scope constant carries.
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
    let mut prefix = flatten(&segments[..segments.len() - 1]);
    prefix.push('_');
    prefix
}

/// The D name a declaration outside the type arena takes.
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

/// How deep a type's D spelling may nest before the walk refuses
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

/// Join scoped segments with `_`, escaping each.
///
/// `_` and not concatenation: `CommonUtilLocation` cannot be read back
/// to its segments, and two different scoped names would reach one D
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
        .map(|segment| d::escape_name(segment))
        .collect::<Vec<String>>()
        .join("_")
}

/// The D spelling of a member, an enumerator or a method.
///
/// The escape and nothing else. D has no export rule and no naming
/// convention the compiler enforces, so unlike the Go column there is no
/// second step here that could reorder with the first.
#[must_use]
pub fn field_name(name: &str) -> String {
    d::escape_name(name)
}

/// The declaration's own IDL name, where the D spelling moved it.
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
/// the same D spelling.
///
/// One module scope holds every declaration in the file, so this is a
/// whole-file check and not a per-scope one — which is the difference
/// from [`no_spelling_collision`], used for the members of one
/// aggregate. Two IDL modules can flatten onto one D name (`a_b::C` and
/// `a::b_C` both give `a_b_C`) and D would report a redeclaration,
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
            at: "the generated module".to_string(),
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

/// The D expression a constant's value takes.
///
/// Written bare rather than cast. A manifest constant states its type,
/// and D converts an integer constant expression to a narrower type when
/// the value fits — so `enum ubyte LIMIT = 8;` is legal and a cast would
/// only hide a value that did not fit.
///
/// `long double` is refused by name. Its D holder is a `string` carrying
/// the decimal, and CORBA gives a `long double` literal no spelling that
/// survives a `double`: emitting one would put a different number in the
/// file than the IDL wrote.
fn d_constant(
    value: &zerodds_ast::ConstValue,
    type_: &d::Type,
    at: &str,
) -> Result<d::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_string = matches!(type_, d::Type::Primitive(d::Primitive::String));
    Ok(match value {
        ConstValue::Boolean(flag) => d::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => d::Expr::Literal(Literal::int(*number)),
        // The code point as a number: an IDL `char` is one octet here and
        // an IDL `wchar` two, and a D character literal is a UTF code
        // unit, which is neither.
        ConstValue::Char(character) => {
            d::Expr::Literal(Literal::int(i128::from(u32::from(*character))))
        }
        ConstValue::WChar(point) => d::Expr::Literal(Literal::int(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            if is_string {
                return Err(refuse("a `long double` constant, which has no D literal"));
            }
            d::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            d::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => d::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => d::Expr::Literal(Literal::Str(
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
