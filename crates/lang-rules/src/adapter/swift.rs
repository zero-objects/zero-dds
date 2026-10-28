// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Swift CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::go`], which does the same for
//! Go. Reading the two side by side is the point: both flatten a module
//! onto the name, and everything else they do differently is Swift's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The memory shapes
//!
//! | IDL | Swift |
//! |---|---|
//! | `boolean` | `Bool` |
//! | `octet`, `uint8`, `char` | `UInt8` |
//! | `int8` | `Int8` |
//! | `short` | `Int16` |
//! | `wchar`, `unsigned short` | `UInt16` |
//! | `long` | `Int32` |
//! | `unsigned long` | `UInt32` |
//! | `long long` | `Int64` |
//! | `unsigned long long` | `UInt64` |
//! | `float` | `Float` |
//! | `double` | `Double` |
//! | `long double` | `String`, holding the decimal |
//! | `string`, `wstring` | `String` |
//! | `fixed<P,S>` | `String`, holding the decimal |
//! | `sequence<T>` | `[T]` |
//! | `T[N][M]` | `[[T]]`, outer dimension outermost |
//! | `map<K,V>` | `[ZdMapEntry_K_V]`, **not** a `Dictionary` |
//! | `@optional T m` | `T?`, `nil` when absent |
//! | `@external T m` | `ZdExternal<T>` |
//! | `enum` | `enum Name: Int32`, raw values written out |
//! | `bitmask` | `struct Name: OptionSet` |
//! | `union` | `struct { _d; one optional per branch }` |
//! | `interface` | `protocol` |
//!
//! ## Why a module flattens rather than nesting
//!
//! Swift's unit of namespacing is the module, which is a build-system
//! target and not something a source file can declare. The in-file
//! equivalent is a nested type — the caseless-`enum`-as-namespace idiom,
//! `enum common { enum util { struct Location {} } }` — and it is
//! rejected here for a measured reason.
//!
//! Unqualified lookup inside a nested type finds the enclosing type's
//! members before the module's. Measured, on Swift 6.3: a file declaring
//! a top-level `struct Baz` and a nested `Foo.Baz`, with `Foo.Bar`
//! holding a member written `Baz`, resolves that member to `Foo.Baz` —
//! and the file compiles. Two IDL scopes each declaring a type of one
//! name is ordinary IDL, so a nesting column would need every member type
//! written from the module root to be safe, and there is no root
//! qualifier a single generated file can spell for itself. Flattening
//! puts every declaration in one scope where a collision is a collision,
//! and `remember` refuses the pair by IDL name before the compiler sees
//! the file.
//!
//! Declaration *order* is not a reason either way: a Swift declaration
//! may be named before it is written, so unlike the C column there is no
//! dependency walk here, and the output tracks the source.
//!
//! ## Why a `union` is a struct and not an `enum` with associated values
//!
//! This is the load-bearing decision of the column. Swift's
//! enum-with-associated-values is the shape Go has no form for and looks
//! like the natural fit — and it cannot hold what an IDL union holds.
//!
//! A union's discriminator may take a value that selects no branch. IDL
//! 4.2 §7.4.1.4.4 admits a `default:` label and also admits a union with
//! neither a `default:` nor a case for every value of the discriminator
//! type; DDS-XTypes 1.3 §7.4.1.1.5.2 then writes the discriminator and
//! nothing else. `union U switch (long) { case 0: long a; }` receiving a
//! discriminator of `7` is a legal sample. An
//! `enum U { case a(Int32) }` has no state for it: the decoder would
//! have to invent a branch, refuse a legal sample, or lose the
//! discriminator's value. A struct with `_d` and one optional property
//! per branch has all three states, and `_d` is a legal Swift
//! identifier — no escape needed.
//!
//! ## Why `map<K,V>` is a sequence of entries and never a `Dictionary`
//!
//! Two reasons and either is enough. A `map<K,V>` is written to the wire
//! as a sequence of key/value pairs (DDS-XTypes 1.3 §7.4.1.1.4), and
//! Swift's `Dictionary` has no order: "the order of key-value pairs" in
//! a dictionary is not defined, and the standard library seeds its hash
//! per process, so one program over one value would produce a different
//! octet stream on each run — and the KeyHash of a keyed type with it.
//! Second, `Dictionary` requires `Key: Hashable`, which a generated
//! struct holding a `Double` is not naturally: `Double` has two
//! representations of zero and a NaN that is not equal to itself.
//!
//! ## Why `long double` is a `String` and never a `Double`
//!
//! Swift's extended-precision type is `Float80`, and `Float80` exists
//! only on x86. Measured, on `arm64-apple-macosx`: `'Float80' is
//! unavailable: Float80 is not available on target platform.` A column
//! that used it would not compile on Apple silicon at all. `Double` is
//! not the fallback either: it has 53 bits of significand where an IDL
//! `long double` is a binary128 with 113, so a member declared `Double`
//! would lose sixty bits at the assignment, before any codec runs. The
//! member holds the decimal text instead — as `fixed<P,S>` does, and as
//! the Go and Python columns do for the same reason.
//!
//! ## Why an array is `[T]` and not a tuple
//!
//! Swift has no fixed-size array type. The two candidates are a tuple of
//! N elements, which puts the length in the type, and `[T]`, which does
//! not.
//!
//! The tuple is rejected. Swift's standard library declares `==` for
//! tuples of arity two to six only, so a seven-element array would
//! produce a type that cannot be compared; a tuple cannot be subscripted
//! by a value computed at run time, so a codec would have to write
//! `.0`, `.1`, … out in full; and a tuple cannot be iterated at all.
//! `long a[1000]` under that scheme is a type whose written form is
//! several thousand characters and a codec of a thousand statements.
//! Under this one it is `[Int32]`, with `1000` in the comment above the
//! member.
//!
//! What is given up is stated rather than hidden: the compiler no longer
//! rejects an array of the wrong length, so the length is checked by the
//! codec against the declared bound at encode and decode time. The
//! Python column makes the other choice — `tuple[T, T, T]` — and the
//! difference between the two columns is the difference between a
//! language whose tuples are usable at any arity and one whose are not.
//!
//! ## Why `@external` is a class
//!
//! `@external` says the member is stored out of line
//! (DDS-XTypes 1.3 Table 21). Swift's only stored form that does not sit
//! inside its owner is a reference, and a value type whose stored
//! property recursively contains it is rejected outright — measured, on
//! Swift 6.3: `value type 'N' cannot have a stored property that
//! recursively contains it`, for `struct N { var next: N? }`. The corpus
//! case `recursive-optional-external` is exactly that IDL. A
//! `ZdExternal<T>` box is emitted, once per file and only when a member
//! reaches one.
//!
//! The cost is stated: a class is a reference, so two copies of the
//! enclosing struct share the box until one of them replaces it. That is
//! what "stored out of line" means, and it is the only Swift form that
//! is.
//!
//! ## Where the IDL spelling of a member is kept
//!
//! Nowhere separate, because nothing moves it. [`swift::escape_name`]
//! puts back quotes round a keyword and back quotes do not rename:
//! `` `default` `` *is* `default`. A member's ID under `@autoid(HASH)`
//! and under `@hashid` is the first four octets of the MD5 of its IDL
//! name (DDS-XTypes 1.3 §7.3.1.2.1.1), and the Swift spelling is that
//! name. The Go column carries an `idl` struct tag on every field
//! precisely because `type` → `Type_` cannot be read back; this column
//! has nothing to record.
//!
//! Two spellings do move, and both are recorded in the comment above the
//! declaration: a declared type whose flattened name collides with a
//! standard-library type takes a trailing underscore
//! ([`swift::escape_type_name`]), and a declaration inside a module
//! takes the module path as a prefix.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Visibility};
use zerodds_lang_cst::swift;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Swift";

/// The prefix a name this column invented carries.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// The box a member declared `@external` is held in.
const EXTERNAL_BOX: &str = "ZdExternal";

/// The generic parameter of [`EXTERNAL_BOX`].
const EXTERNAL_BOX_PARAM: &str = "Value";

/// The Swift property name of the union discriminator.
///
/// `_d`, which every other column but Go also uses: it is a legal Swift
/// identifier, and Swift has no rule — as Go does — that ties visibility
/// to the case of the first letter.
pub const DISCRIMINATOR: &str = "_d";

/// The label an initialiser parameter takes for a member named `self`.
const SELF_LABEL: &str = "`self`";

/// The body-side name that parameter takes instead.
///
/// A parameter labelled `` `self` `` shadows the implicit `self` inside
/// the initialiser, and every assignment then reads as a member of the
/// parameter. Measured, on Swift 6.3: `value of type 'Int32' has no
/// member 'default'`, from an initialiser whose first parameter was
/// labelled `` `self` ``. Swift lets a parameter carry a second name for
/// the body to use (*The Swift Programming Language*, "Functions"), and
/// this is it. The call site is unaffected: the label is still the IDL
/// name.
const SELF_PARAM: &str = "zdSelf";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Swift spelling may nest before the walk refuses
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

/// Build the Swift declarations a rule application produced.
///
/// The order is the model's own declaration order. A Swift declaration
/// may be named before it is written, so unlike the C column there is no
/// dependency walk and the output tracks the source.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Swift shape, or two declarations reach one
/// Swift spelling — never a silently dropped declaration.
pub fn swift_items(ast: &Ast, applied: &Applied) -> Result<Vec<swift::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::SWIFT),
            &swift_type_name(ast, id),
            start,
            &mut out,
        );
    }

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "SwiftLet").is_none() {
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
            Some(target) => swift_value_type(ast, target)?,
            None => swift::Type::Primitive(swift::Primitive::Int32),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(swift::Decl::Constant(swift::ConstDecl {
            doc: idl_note(&name, &node.name),
            visibility: Visibility::Public,
            name: Ident::new(&name),
            // The type is written out rather than left to inference: an
            // integer literal with no annotation is `Int`, whose width is
            // the platform's, and `let LIMIT = 8` would be a constant of
            // a different type than the member it is compared with.
            type_: Some(type_.clone()),
            value: swift_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "SwiftError").is_none() {
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
                continue;
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            properties.push(swift::Property {
                doc: array_note(ast, target),
                visibility: Visibility::Public,
                mutable: true,
                name: Ident::new(&property_name(&member_node.name)),
                type_: member_shape(ast, member_node, target)?,
            });
        }
        out.push(aggregate_with_conformances(
            with_idl_note(
                Doc::line(
                    "An exception, as the structure its members make — and `Swift.Error`, \
                     so an operation whose `raises` names it can literally `throw` one.",
                ),
                &name,
                &node.name,
            ),
            &name,
            properties,
            vec![Ident::new("Error")],
        ));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "SwiftProtocol").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut methods = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. Swift would carry
        // the inherited requirements through an inherited protocol, and
        // `swift::ProtocolDecl` has no place for one: a requirement left
        // out here is an operation the protocol does not have, and OMG
        // IDL 4.2 §7.4.3 gives a derived interface every one of its
        // bases' operations *and attributes*.
        // `interface::all_operations_flattened` carries both — the
        // get/set pair DDS-RPC 1.0 §7.5.1.1.3 implies for each
        // attribute, besides the declared operations — which
        // `Ast::all_operations_of` alone never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            // An attribute's own name for its getter — Swift's own
            // convention for a computed property's accessor pair is a
            // bare name and a `set` prefix, the same shape Go's binding
            // takes for the same reason — rather than
            // `all_operations_flattened`'s `get_`/`set_` prefix, which
            // spells out DDS-RPC 1.0's own implied-operation name and
            // not a Swift one.
            let (exported, idl_spelling) = match op.origin {
                interface::Origin::AttributeGetter => {
                    let attr = op.attribute_name();
                    (property_name(attr), op.name.clone())
                }
                interface::Origin::AttributeSetter => {
                    let attr = op.attribute_name();
                    (
                        format!("set{}", capitalise(&property_name(attr))),
                        op.name.clone(),
                    )
                }
                interface::Origin::Declared => (property_name(&op.name), op.name.clone()),
            };
            signatures.push((exported.clone(), idl_spelling));
            let mut params = Vec::new();
            for param in &op.params {
                params.push(swift::Param {
                    // The IDL name is the argument label, which is what a
                    // caller writes; a protocol requirement has no body,
                    // so there is no second name to give.
                    label: Ident::new(&property_name(&param.name)),
                    internal_name: None,
                    type_: swift_member_type(ast, param.type_, &param.name)?,
                });
            }
            let returns = match op.returns {
                Some(target) => Some(swift_member_type(ast, target, &op.name)?),
                None => None,
            };
            let doc = op
                .raises_line()
                .map_or_else(Doc::none, |line| Doc::line(&line));
            methods.push(swift::Method {
                doc,
                name: Ident::new(&exported),
                params,
                // A real `throws` — not only documented — since every
                // exception `raises` names now conforms to `Swift.Error`
                // (see `exceptions` above) and Swift has the keyword to
                // carry it.
                throws: !op.raises.is_empty(),
                returns,
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(swift::Decl::Protocol(swift::ProtocolDecl {
            doc: with_idl_note(
                Doc::line(
                    "An interface, as the requirements its operations and attributes \
                     make — an attribute a `name()`/`setName(_:)` pair, `readonly` only \
                     the first. A derived interface's bases are flattened in rather than \
                     inherited, so these are all of them; the transport is the RPC arc's \
                     business, and the shape is the IDL's. A `raises` clause is a real \
                     `throws`, over the `Swift.Error`-conforming struct each exception \
                     becomes.",
                ),
                &name,
                &node.name,
            ),
            visibility: Visibility::Public,
            name: Ident::new(&name),
            methods,
        }));
    }

    // The box goes in only where a member reached it. An unused public
    // class is a name in the output that nothing put there.
    if out.iter().any(decl_uses_external_box) {
        out.insert(0, external_box());
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::SWIFT, out))
}

/// `@verbatim` in the Swift column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a struct's or class's body; an enum,
/// a type alias and a protocol take them adjacent.
impl verbatim::Carrier for swift::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Aggregate(node) = self else {
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

/// The Swift declarations, in a file.
///
/// A caller that only wants the declarations uses [`swift_items`]; this
/// is what a generator writes to disk, and it exists here so that the
/// header is decided in one place.
///
/// # Errors
/// As [`swift_items`].
pub fn swift_file(ast: &Ast, applied: &Applied) -> Result<swift::File, MaterialiseError> {
    Ok(swift::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "Every declaration is `public`: Swift's default access level is",
            "`internal`, which is the defining module only, and a generated type",
            "no consumer can name has no consumer. The memberwise initialiser is",
            "written out for the same reason — the synthesised one is `internal`",
            "even when every property is `public`.",
            "",
            "A member whose IDL name is a Swift keyword is written in back quotes,",
            "which do not rename it: the IDL spelling is the Swift spelling, which",
            "is what @autoid(HASH) hashes (DDS-XTypes 1.3 7.3.1.2.1.1).",
        ]),
        imports: Vec::new(),
        decls: swift_items(ast, applied)?,
    })
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<swift::Decl>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence and an array are all
        // written where the member is: Swift spells each of them inline,
        // so none of them is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // The one collection that does need a name. Its two halves are
        // written with no framing between them, so an entry is a struct
        // and a struct needs a declaration.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: swift_type_name(ast, id),
                });
            };
            let name = map_entry_name_at(ast, id, 0)?;
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            out.push(aggregate(
                Doc::line(
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them.",
                ),
                &name,
                vec![
                    swift::Property {
                        doc: Doc::none(),
                        visibility: Visibility::Public,
                        mutable: true,
                        name: Ident::new("key"),
                        type_: swift_member_type(ast, key, &name)?,
                    },
                    swift::Property {
                        doc: Doc::none(),
                        visibility: Visibility::Public,
                        mutable: true,
                        name: Ident::new("value"),
                        type_: swift_member_type(ast, value, &name)?,
                    },
                ],
            ));
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "SwiftTypealias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = swift_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // `typealias` and not a wrapper struct. IDL 4.2 §7.4.1.4.3:
            // an alias "does not define a new type", and a Swift
            // `typealias` "introduces a named alias of an existing type"
            // with the same identity — a struct would be a second type
            // that needed a conversion the IDL does not ask for.
            out.push(swift::Decl::TypeAlias(swift::TypeAliasDecl {
                doc: idl_note(&name, &node.name),
                visibility: Visibility::Public,
                name: Ident::new(&name),
                type_: swift_member_type(ast, target, &node.name)?,
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "SwiftEnum").is_none() {
                return Ok(());
            }
            let name = swift_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut cases = Vec::new();
            let mut pairs = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "SwiftCase").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                pairs.push((property_name(&enum_node.name), enum_node.name.clone()));
                // The raw value is written out on every case. Swift
                // numbers an unassigned case one above the previous one,
                // and `@value` admits gaps, so a case left to the
                // language would renumber the wire.
                cases.push(swift::EnumCase {
                    doc: Doc::none(),
                    name: Ident::new(&property_name(&enum_node.name)),
                    raw_value: Some(swift::Expr::int(value)),
                });
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            out.push(swift::Decl::Enum(swift::EnumDecl {
                doc: with_idl_note(
                    Doc::line(
                        "An enumeration. The raw type is signed because DDS-XTypes 1.3 \
                         7.4.3.5.3 makes it so; the width on the wire is the \
                         @bit_bound's and is applied by the runtime.",
                    ),
                    &name,
                    &node.name,
                ),
                visibility: Visibility::Public,
                name: Ident::new(&name),
                // No raw type where there are no cases: Swift rejects the
                // pair outright — `an enum with no cases cannot declare a
                // raw type`.
                raw_type: if cases.is_empty() {
                    None
                } else {
                    Some(swift::Type::Primitive(swift::Primitive::Int32))
                },
                cases,
            }));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "SwiftOptionSet").is_none() {
                return Ok(());
            }
            let name = swift_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            let holder = swift::Type::Primitive(swift_holder(bound));
            let mut statics = Vec::new();
            let mut pairs = Vec::new();
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "SwiftFlag").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                pairs.push((property_name(&bit_node.name), bit_node.name.clone()));
                statics.push(swift::StaticLet {
                    doc: Doc::none(),
                    visibility: Visibility::Public,
                    name: Ident::new(&property_name(&bit_node.name)),
                    type_: None,
                    value: swift::Expr::Call {
                        callee: Ident::new(&name),
                        args: vec![swift::Argument {
                            label: Some(Ident::new("rawValue")),
                            value: swift::Expr::Binary {
                                op: "<<",
                                lhs: Box::new(swift::Expr::int(1)),
                                rhs: Box::new(swift::Expr::int(i128::from(position))),
                            },
                        }],
                    },
                });
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            // `OptionSet` and not a bare unsigned integer, which is what
            // the Go column has to produce. `OptionSet` is the standard
            // library's own answer to a set of bit flags: it gives the
            // type set algebra — `insert`, `contains`, `union` — over the
            // bound the IDL declared, and it keeps the flags inside the
            // type rather than as free constants that could be applied to
            // the wrong bitmask.
            out.push(swift::Decl::Aggregate(swift::AggregateDecl {
                doc: with_idl_note(
                    Doc::line(
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3 \
                         7.4.1.1.3.2 gives for its bound.",
                    ),
                    &name,
                    &node.name,
                ),
                visibility: Visibility::Public,
                kind: swift::Aggregate::Struct,
                name: Ident::new(&name),
                generics: Vec::new(),
                conformances: vec![Ident::new("OptionSet")],
                properties: vec![swift::Property {
                    doc: Doc::none(),
                    visibility: Visibility::Public,
                    // `let` and not `var`: `OptionSet`'s own operations
                    // build a new value rather than mutating the raw one.
                    mutable: false,
                    name: Ident::new("rawValue"),
                    type_: holder.clone(),
                }],
                statics,
                body_text: zerodds_lang_cst::common::BodyText::none(),
                inits: vec![swift::Init {
                    doc: Doc::line(
                        "`RawRepresentable`'s requirement, written out because the \
                         synthesised memberwise initialiser is internal.",
                    ),
                    visibility: Visibility::Public,
                    params: vec![swift::Param {
                        label: Ident::new("rawValue"),
                        internal_name: None,
                        type_: holder,
                    }],
                    body: vec![swift::Stmt::AssignProperty {
                        property: Ident::new("rawValue"),
                        value: swift::Expr::Name(Ident::new("rawValue")),
                    }],
                }],
            }));
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "SwiftBitset").is_none() {
                return Ok(());
            }
            let name = swift_type_name(ast, id);
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
                pairs.push((property_name(field_ident), field_ident.clone()));
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut properties = Vec::new();
            let mut index = 0;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                if field_node.name.is_none() {
                    continue;
                }
                properties.push(swift::Property {
                    doc: Doc::none(),
                    visibility: Visibility::Public,
                    mutable: true,
                    name: Ident::new(&pairs[index].0),
                    type_: swift::Type::Primitive(swift_holder(field_node.bits)),
                });
                index += 1;
            }
            out.push(aggregate(
                with_idl_note(
                    Doc::line(
                        "A bitset. The fields are separate integers here; the packing \
                         into the anchor is the codec's, because the order on the wire \
                         is the declaration's.",
                    ),
                    &name,
                    &node.name,
                ),
                &name,
                properties,
            ));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "SwiftValue"
            } else {
                "SwiftStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = swift_type_name(ast, id);
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
                properties.push(swift::Property {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    visibility: Visibility::Public,
                    mutable: true,
                    name: Ident::new(&property_name("parent")),
                    type_: swift_member_type(ast, base, "parent")?,
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
                let mut doc = array_note(ast, target);
                if member_node.facets.is_optional() {
                    // An `Optional` and not a value beside a flag: a
                    // Swift value has no "unset" state of its own, so a
                    // `long` member that arrived as 0 and one that did
                    // not arrive at all would be the same value. `nil` is
                    // the one state no written value can take.
                    doc.push(
                        "Optional. `nil` when the member is absent — DDS-XTypes 1.3 \
                         7.2.2.4.4.4.7. In a @mutable type an absent member is not \
                         written at all; outside one its absence is written.",
                    );
                }
                if super::held_externally(ast, member_node) {
                    doc.push(
                        "Stored out of line (`@external`, DDS-XTypes 1.3 Table 21). The \
                         box is a reference, so two copies of this value share it until \
                         one of them replaces it.",
                    );
                }
                properties.push(swift::Property {
                    doc,
                    visibility: Visibility::Public,
                    mutable: true,
                    name: Ident::new(&property_name(&member_node.name)),
                    type_: member_shape(ast, member_node, target)?,
                });
            }
            out.push(aggregate(idl_note(&name, &node.name), &name, properties));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "SwiftUnion").is_none() {
                return Ok(());
            }
            let name = swift_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let cases = ast.cases_of(id);
            // The discriminator's property is in the collision check with
            // the branches: IDL 4.2 §7.4.1.4.4.1 gives it no member name
            // of its own, so `_d` is this column's, and a branch called
            // `_d` would take the same Swift name and make one of the two
            // unreachable.
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case)
                    && let Ok(member) = ast.member(case_node.member)
                {
                    pairs.push((property_name(&member.name), member.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut properties = vec![swift::Property {
                doc: Doc::line("Which branch carries the value."),
                visibility: Visibility::Public,
                mutable: true,
                name: Ident::new(DISCRIMINATOR),
                type_: swift_member_type(ast, disc, &name)?,
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "SwiftBranch").is_none() {
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
                let mut doc = Doc::line("`nil` unless `_d` selects this branch.");
                if super::held_externally(ast, member) {
                    doc.push(
                        "Stored out of line (`@external`, DDS-XTypes 1.3 Table 21). The \
                         box is a reference, so two copies of this value share it until \
                         one of them replaces it.",
                    );
                }
                properties.push(swift::Property {
                    doc,
                    visibility: Visibility::Public,
                    mutable: true,
                    name: Ident::new(&property_name(&member.name)),
                    // Every branch is an `Optional`, including the one
                    // `_d` selects, because exactly one of them holds a
                    // value at a time and the other states have to be
                    // representable. `member_shape` and not the bare
                    // type, so an `@external` branch keeps its box the
                    // way a structure member does.
                    type_: swift::Type::optional(member_shape(ast, member, target)?),
                });
            }
            out.push(aggregate(
                with_idl_note(
                    Doc::lines(&[
                        "A union. Swift's enum with associated values is the shape this",
                        "looks like and cannot be: an IDL discriminator may take a value",
                        "that selects no branch (IDL 4.2 7.4.1.4.4), and DDS-XTypes 1.3",
                        "7.4.1.1.5.2 then writes the discriminator and nothing else. A",
                        "struct has that state; an enum has no case for it.",
                    ]),
                    &name,
                    &node.name,
                ),
                &name,
                properties,
            ));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "SwiftNative").is_none() {
                return Ok(());
            }
            let name = swift_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `Any` when there is none.
            //
            // `Any` here and refused in member position, which is not a
            // contradiction: a `native` declaration says the IDL author
            // has no wire form for it and the language mapping supplies
            // one, whereas an `any` member claims a wire form this column
            // cannot produce.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(swift::Decl::TypeAlias(swift::TypeAliasDecl {
                doc: with_idl_note(
                    Doc::line("A native type. Opaque unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                visibility: Visibility::Public,
                name: Ident::new(&name),
                type_: match mapping {
                    Some(spelling) => swift::Type::named(&spelling),
                    None => swift::Type::Primitive(swift::Primitive::Any),
                },
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = swift_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            out.push(swift::Decl::TypeAlias(swift::TypeAliasDecl {
                doc: with_idl_note(
                    Doc::line("A boxed value: an `Optional`, `nil` when absent."),
                    &name,
                    &node.name,
                ),
                visibility: Visibility::Public,
                name: Ident::new(&name),
                type_: swift::Type::optional(swift_member_type(ast, target, &name)?),
            }));
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `swift_member_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// A `public struct` with a written memberwise initialiser.
///
/// Every aggregate this column emits goes through here, so the
/// initialiser is decided once. Swift synthesises a memberwise
/// initialiser and gives it `internal` access even when every property is
/// `public` (*The Swift Programming Language*, "Access Control",
/// "Default Memberwise Initializers for Structure Types"), so a
/// `public struct` with no written `init` is a type another module can
/// hold and cannot construct.
fn aggregate(doc: Doc, name: &str, properties: Vec<swift::Property>) -> swift::Decl {
    aggregate_with_conformances(doc, name, properties, Vec::new())
}

/// [`aggregate`], additionally conforming to `conformances` — an IDL
/// `exception`'s own struct conforms to `Swift.Error`, which has no
/// required members, so this is the only difference an exception's
/// shape needs from an ordinary aggregate's to be `throw`able and
/// `catch`able.
fn aggregate_with_conformances(
    doc: Doc,
    name: &str,
    properties: Vec<swift::Property>,
    conformances: Vec<Ident>,
) -> swift::Decl {
    let init = memberwise_init(&properties);
    swift::Decl::Aggregate(swift::AggregateDecl {
        doc,
        visibility: Visibility::Public,
        kind: swift::Aggregate::Struct,
        name: Ident::new(name),
        generics: Vec::new(),
        conformances,
        properties,
        statics: Vec::new(),
        body_text: zerodds_lang_cst::common::BodyText::none(),
        inits: vec![init],
    })
}

/// The memberwise initialiser, written out.
fn memberwise_init(properties: &[swift::Property]) -> swift::Init {
    let mut params = Vec::new();
    let mut body = Vec::new();
    for property in properties {
        let internal = if property.name.as_str() == SELF_LABEL {
            Some(Ident::new(SELF_PARAM))
        } else {
            None
        };
        let value = Ident::new(
            internal
                .as_ref()
                .map_or_else(|| property.name.as_str(), Ident::as_str),
        );
        params.push(swift::Param {
            label: property.name.clone(),
            internal_name: internal,
            type_: property.type_.clone(),
        });
        body.push(swift::Stmt::AssignProperty {
            property: property.name.clone(),
            value: swift::Expr::Name(value),
        });
    }
    swift::Init {
        doc: Doc::line(
            "The memberwise initialiser, written out: the synthesised one is \
             internal even when every property is public, so a consumer in \
             another module could hold this type and not construct one.",
        ),
        visibility: Visibility::Public,
        params,
        body,
    }
}

/// The box a member declared `@external` is held in.
fn external_box() -> swift::Decl {
    swift::Decl::Aggregate(swift::AggregateDecl {
        doc: Doc::lines(&[
            "A member the IDL declared @external — stored out of line",
            "(DDS-XTypes 1.3 Table 21).",
            "",
            "A class and not a struct, because Swift's only stored form that does",
            "not sit inside its owner is a reference: `struct N { var next: N? }`",
            "is rejected with `value type 'N' cannot have a stored property that",
            "recursively contains it`, and a self-recursive type made finite by an",
            "absent @optional member is exactly that shape.",
            "",
            "The consequence is that two copies of the enclosing value share this",
            "box until one of them replaces it, which is what storing a member out",
            "of line means.",
        ]),
        visibility: Visibility::Public,
        kind: swift::Aggregate::Class,
        name: Ident::new(EXTERNAL_BOX),
        generics: vec![Ident::new(EXTERNAL_BOX_PARAM)],
        conformances: Vec::new(),
        properties: vec![swift::Property {
            doc: Doc::none(),
            visibility: Visibility::Public,
            mutable: true,
            name: Ident::new("value"),
            type_: swift::Type::named(EXTERNAL_BOX_PARAM),
        }],
        statics: Vec::new(),
        body_text: zerodds_lang_cst::common::BodyText::none(),
        inits: vec![swift::Init {
            doc: Doc::none(),
            visibility: Visibility::Public,
            params: vec![swift::Param {
                label: Ident::new("_"),
                internal_name: Some(Ident::new("value")),
                type_: swift::Type::named(EXTERNAL_BOX_PARAM),
            }],
            body: vec![swift::Stmt::AssignProperty {
                property: Ident::new("value"),
                value: swift::Expr::Name(Ident::new("value")),
            }],
        }],
    })
}

/// Whether a declaration names the external box anywhere in it.
fn decl_uses_external_box(decl: &swift::Decl) -> bool {
    match decl {
        swift::Decl::Aggregate(node) => {
            node.name.as_str() != EXTERNAL_BOX
                && node
                    .properties
                    .iter()
                    .any(|property| type_uses_external_box(&property.type_))
        }
        swift::Decl::TypeAlias(node) => type_uses_external_box(&node.type_),
        swift::Decl::Protocol(node) => node.methods.iter().any(|method| {
            method
                .params
                .iter()
                .any(|param| type_uses_external_box(&param.type_))
                || method.returns.as_ref().is_some_and(type_uses_external_box)
        }),
        swift::Decl::Enum(_)
        | swift::Decl::Constant(_)
        | swift::Decl::Func(_)
        | swift::Decl::Comment(_)
        | swift::Decl::Verbatim(_) => false,
    }
}

/// Whether a type names the external box.
///
/// zerodds-lint: recursion-depth 64 (element nesting of an owned
/// `swift::Type`). The tree is the one the materialiser built and
/// owns, so it holds no cycle; the depth is the declared type's
/// height, which `value_type_at` already bounded at `MAX_TYPE_DEPTH`.
fn type_uses_external_box(type_: &swift::Type) -> bool {
    match type_ {
        swift::Type::Generic { base, arguments } => {
            base.as_str() == EXTERNAL_BOX || arguments.iter().any(type_uses_external_box)
        }
        swift::Type::Array(inner) | swift::Type::Optional(inner) => type_uses_external_box(inner),
        swift::Type::Primitive(_) | swift::Type::Named(_) => false,
    }
}

/// The Swift type one member declaration takes.
///
/// The box goes on first and the `Optional` outside it:
/// `@optional @external Node next` is `ZdExternal<Node>?`, which is the
/// shape that both breaks the recursion and can say "absent".
fn member_shape(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<swift::Type, MaterialiseError> {
    let mut type_ = swift_member_type(ast, target, &member.name)?;
    if super::held_externally(ast, member) {
        type_ = swift::Type::generic(EXTERNAL_BOX, vec![type_]);
    }
    if member.facets.is_optional() {
        type_ = swift::Type::optional(type_);
    }
    Ok(type_)
}

/// The comment an array member carries, because Swift's array type has
/// no length in it.
///
/// The bound is the only place the declared length survives into the
/// generated source. A codec checks a sample against it; the compiler
/// cannot.
fn array_note(ast: &Ast, target: Id<TypeNode>) -> Doc {
    let Some(dims) = array_dims(ast, target) else {
        return Doc::none();
    };
    let written: Vec<String> = dims.iter().map(dim_text).collect();
    Doc::line(&format!(
        "A fixed-size array of {} elements. Swift's `[T]` carries no length, \
         so the bound is checked by the codec and not by the compiler.",
        written.join(" x ")
    ))
}

/// One dimension, written.
fn dim_text(value: &i128) -> String {
    format!("{value}")
}

/// The declared dimensions of an array type, following an anonymous
/// typedef the frontend made for an inline type.
fn array_dims(ast: &Ast, target: Id<TypeNode>) -> Option<Vec<i128>> {
    // A counted loop and not a call to itself on the alias target: an
    // anonymous `typedef` chain that closed on itself read forever, and
    // a cycle among the frontend's inline types is a finding of the
    // model's invariant pass rather than a shape it rejects.
    let mut current = target;
    for _ in 0..MAX_TYPE_DEPTH {
        let node = ast.type_(current).ok()?;
        match &node.kind {
            TypeKind::Array { dims, .. } => {
                return Some(
                    dims.iter()
                        .map(|dim| ast.eval_integral(*dim).unwrap_or(0))
                        .collect(),
                );
            }
            TypeKind::Alias { aliased } if node.name.is_empty() => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// The Swift spelling of a value's type, refusing what Swift cannot
/// hold.
///
/// `any` is the first of those, and Swift is the column where refusing
/// matters most: `Any` is a real Swift type, so a member declared with it
/// would compile and there would be no wire form behind it. What is in
/// the model is Building Block Any, one of IDL 4.2's fifteen, whose
/// content is decided at run time; a generated file that named `Any` and
/// a codec that could not be written for it is the silent-substitution
/// class this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn swift_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<swift::Type, MaterialiseError> {
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
    swift_value_type(ast, target)
}

/// The Swift type a value of `id` is held in.
///
/// The one answer, for every emitter. A property of a generated struct, a
/// codec's parameter and a defaulter's destination all have to name the
/// same Swift type or the file does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a collection's element does not
/// resolve.
pub fn swift_value_type(ast: &Ast, id: Id<TypeNode>) -> Result<swift::Type, MaterialiseError> {
    value_type_at(ast, id, 0)
}

/// [`swift_value_type`], carrying how many connector steps the walk has
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
    depth: usize,
) -> Result<swift::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Swift spelling nests past {MAX_TYPE_DEPTH}"),
            at: swift_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(swift::Type::Primitive(swift::Primitive::Any));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => swift_primitive_type(*kind),
        // A `wstring` holds a Swift `String`, which is a sequence of
        // extended grapheme clusters over UTF-8 storage and carries no
        // UTF-16 unit count of its own. The §7.4.1.1.2 surrogate rule
        // therefore sits in the Swift runtime as a second implementation
        // of the clause and not a copy of the Rust column's answer.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            swift::Type::Primitive(swift::Primitive::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            swift::Type::array(value_type_at(ast, element, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // One `[]` per dimension, outermost outermost: `long
            // grid[2][3]` is two rows of three, which in Swift is
            // `[[Int32]]`. The lengths are not in the type — Swift has no
            // form that holds them — so the member's comment carries them
            // and the codec checks them.
            //
            // Each dimension is still evaluated here, although the type
            // does not need it: a bound that does not evaluate is a
            // failure to report, and the alternative is a comment that
            // states a length nobody wrote.
            let mut type_ = value_type_at(ast, element, depth + 1)?;
            for dim in dims {
                ast.eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: node.name.clone(),
                    })?;
                type_ = swift::Type::array(type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            swift::Type::array(swift::Type::named(&map_entry_name_at(ast, id, depth + 1)?))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the Swift type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_type_at(ast, target, depth + 1)?,
            None => swift::Type::Primitive(swift::Primitive::Any),
        },
        _ => swift::Type::named(&swift_type_name(ast, id)),
    })
}

/// The Swift type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a Swift
/// spelling is a compile error and not a member that silently takes
/// another's.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open, and a fixed-width integer is the only choice that does not
/// depend on a platform type. Swift's `Character` is an extended grapheme
/// cluster and its `Unicode.Scalar` is a code point, so neither is one
/// octet or one UTF-16 unit and neither can stand in.
///
/// `long double` is a `String` and never a `Double` or a `Float80`; the
/// module documentation carries why.
#[must_use]
pub fn swift_primitive_type(kind: PrimitiveKind) -> swift::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => swift::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => {
            swift::Primitive::UInt8
        }
        PrimitiveKind::Int8 => swift::Primitive::Int8,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => swift::Primitive::UInt16,
        PrimitiveKind::Int16 => swift::Primitive::Int16,
        PrimitiveKind::Int32 => swift::Primitive::Int32,
        PrimitiveKind::UInt32 => swift::Primitive::UInt32,
        PrimitiveKind::Int64 => swift::Primitive::Int64,
        PrimitiveKind::UInt64 => swift::Primitive::UInt64,
        PrimitiveKind::Float32 => swift::Primitive::Float,
        PrimitiveKind::Float64 => swift::Primitive::Double,
        PrimitiveKind::Float128 => swift::Primitive::String,
    };
    swift::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for.
#[must_use]
pub const fn swift_holder(bits: u16) -> swift::Primitive {
    match bits {
        0..=8 => swift::Primitive::UInt8,
        9..=16 => swift::Primitive::UInt16,
        17..=32 => swift::Primitive::UInt32,
        _ => swift::Primitive::UInt64,
    }
}

/// A declared type's Swift identifier.
///
/// The scoped name with the module separators flattened onto it:
/// `::common::util::Location` is `common_util_Location`. The segments are
/// **not** recased. Swift has no rule tying visibility to the case of a
/// letter — that is Go's — so recasing would change the IDL spelling for
/// no gain, and the IDL spelling is what `@autoid(HASH)` hashes.
#[must_use]
pub fn swift_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
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
/// built from the IDL kind and not the Swift spelling: `double` and
/// `long double` are `Double` and `String` here, but `octet` and `uint8`
/// are both `UInt8`, and a name built from the Swift type would give
/// `map<octet,V>` and `map<uint8,V>` one declaration — harmless today and
/// the same collapse that would be wrong if the two ever differed on the
/// wire.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a key's or a value's tag
/// nests past `MAX_TYPE_DEPTH`.
pub fn swift_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    map_entry_name_at(ast, id, 0)
}

/// [`swift_map_entry_name`], carrying the depth of the walk that reached it.
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
            at: swift_type_name(ast, id),
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
            at: swift_type_name(ast, id),
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
        _ => swift_type_name(ast, id),
    })
}

/// The first letter raised, for a name this column invented.
///
/// Only the invented names go through here — a map entry's tag — because
/// they are not IDL names and nothing has to read them back.
fn capitalise(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// The Swift name a declaration outside the type arena takes.
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

/// Join scoped segments with `_`.
///
/// `_` and not concatenation: `commonutilLocation` cannot be read back to
/// its segments, and two different scoped names would reach one Swift
/// name with nothing to say so. The join is still not injective — an IDL
/// identifier may itself contain `_` — which is why [`remember`] checks
/// every declared name against every other one in the file.
///
/// The escape runs on the **joined** name and not on each segment,
/// because a back quote wraps a whole identifier: escaping segments would
/// give `` `default`_Location ``, which is not one identifier at all.
fn flatten(segments: &[String]) -> String {
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

fn flatten_str(segments: &[&str]) -> String {
    swift::escape_type_name(&segments.join("_"))
}

/// The Swift spelling of a member, an enumerator, a bit or a method.
///
/// Back quotes where the IDL name is a Swift keyword, and the IDL name
/// unchanged otherwise. Nothing is renamed, so nothing has to be recorded
/// elsewhere and no pair of IDL names can collide here that did not
/// already collide in the IDL.
#[must_use]
pub fn property_name(name: &str) -> String {
    swift::escape_name(name)
}

/// The declaration's own IDL name, where the Swift spelling moved it.
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

/// Record a declared name, refusing a second declaration that reaches the
/// same Swift spelling.
///
/// One module scope holds every declaration in the file, so this is a
/// whole-file check and not a per-scope one — which is the difference
/// from [`no_spelling_collision`], used for the members of one aggregate.
/// Two IDL modules can flatten onto one Swift name (`a_b::C` and
/// `a::b_C` both give `a_b_C`) and Swift would report `invalid
/// redeclaration of 'a_b_C'`, naming neither IDL declaration.
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

/// The Swift expression a constant's value takes.
///
/// `long double` is refused by name. Its Swift holder is a `String`
/// carrying the decimal, and CORBA gives a `long double` literal no
/// spelling that survives a `Double`: emitting one would put a different
/// number in the file than the IDL wrote.
fn swift_constant(
    value: &zerodds_ast::ConstValue,
    type_: &swift::Type,
    at: &str,
) -> Result<swift::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_string = matches!(type_, swift::Type::Primitive(swift::Primitive::String));
    Ok(match value {
        ConstValue::Boolean(flag) => swift::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => swift::Expr::Literal(Literal::int(*number)),
        // The code point as a number: an IDL `char` is one octet here and
        // an IDL `wchar` two, and a Swift `Character` literal is neither.
        ConstValue::Char(character) => {
            swift::Expr::Literal(Literal::int(i128::from(u32::from(*character))))
        }
        ConstValue::WChar(point) => swift::Expr::Literal(Literal::int(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            if is_string {
                return Err(refuse(
                    "a `long double` constant, which has no Swift literal",
                ));
            }
            swift::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            swift::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => swift::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => swift::Expr::Literal(Literal::Str(
            constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is a
        // file that disagrees with the IDL and compiles.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
