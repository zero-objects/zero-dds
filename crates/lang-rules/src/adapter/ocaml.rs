// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OCaml CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::go`] and [`crate::adapter::c`].
//! Reading the three side by side is the point: all three flatten a
//! module onto the name, and everything else this one does differently
//! is OCaml's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The five things OCaml decides that Go does not
//!
//! 1. **Declaration order is the file's meaning, and a cycle is one
//!    declaration.** A name resolves against what precedes it (OCaml
//!    manual, ch. "Module expressions"), so this column walks the type
//!    graph the way [`crate::adapter::c`] does — and then goes further,
//!    because C refuses a recursive type and OCaml can write one. The
//!    walk computes the strongly connected components of the type graph
//!    (`type_groups`); a component of more than one type, or one that
//!    reaches itself, is emitted as a single `type a = … and b = …`
//!    group. There is no other way to write it.
//! 2. **Case is grammar.** A type constructor and a record field are
//!    `lowercase-ident`, a variant constructor is `capitalized-ident`
//!    (OCaml manual, ch. "Lexical conventions"). `struct Pose` is
//!    `type pose`, and `zerodds_lang_cst::ocaml::lower_ident` folds the
//!    whole name so that `bitmask Flags { READY }` gives `let ready` and
//!    not `let rEADY`.
//! 3. **There are no unsigned integers.** Not in the language and not in
//!    the standard library. This is the column's central limitation and
//!    the table below says what each width costs.
//! 4. **An enumeration's numbering is not in the type.** An OCaml
//!    variant constructor carries no number, so `type mode = IDLE` loses
//!    what DDS-XTypes 1.3 §7.4.3.5.3 puts on the wire. Two conversions
//!    are emitted beside every enumeration and they are not a
//!    convenience: without them the numbering is gone.
//! 5. **A union cannot be a variant.** See below.
//!
//! ## Why a module is a name prefix and not a `module`
//!
//! OCaml has nested modules and `module Common = struct module Util =
//! struct … end end` is the better OCaml spelling of
//! `module common { module util { … } }`. It is not used, and the reason
//! is point 1 above: an `and`-joined group is one type declaration
//! inside one structure, so it cannot span two modules. IDL puts no such
//! restriction on where a cycle may close — `tests/interop-corpus/
//! recursive-mutual` closes one inside a module and nothing in IDL 4.2
//! stops one closing across two. A column that nested modules would
//! render most of the corpus and then have no form at all for the case
//! it is supposed to be best at. The scoped name is flattened with `_`
//! instead, as C and Go do, and every declared name in the file is
//! checked against every other by `remember`.
//!
//! ## The memory shapes
//!
//! | IDL | OCaml | what it costs |
//! |---|---|---|
//! | `boolean` | `bool` | — |
//! | `octet`, `uint8`, `char`, `int8` | `int` | — |
//! | `short`, `unsigned short`, `wchar` | `int` | — |
//! | `long` | `int32` | — |
//! | `unsigned long` | `int64` | a widening; every value is exact |
//! | `long long` | `int64` | — |
//! | `unsigned long long` | `string`, decimal | no integer type holds it |
//! | `float` | `float` | widened from 32 to 64 bits |
//! | `double` | `float` | — |
//! | `long double` | `string`, decimal | no binary128 |
//! | `string`, `wstring`, `fixed<P,S>` | `string` | — |
//! | `sequence<T>` | `t array` | — |
//! | `T[N][M]` | `t array array` | the bound leaves the type |
//! | `map<K,V>` | `zd_map_entry_k_v array` | — |
//! | `@optional T` | `t option` | — |
//!
//! ## What the generated file needs
//!
//! Nothing outside the standard library, and nothing added since OCaml
//! 4.02: `int32`, `int64`, `array`, `option`, `Obj.t`, `invalid_arg` and
//! `Printf.sprintf`. It compiles under `ocamlc -c` on 4.14 and on 5.x
//! with no `-package` and no `dune`.
//!
//! ### `int` is used only where 16 bits are enough
//!
//! OCaml's `int` is 63 bits on a 64-bit target and 31 on a 32-bit one
//! (OCaml manual, ch. "Integers"). Every IDL type mapped onto it needs
//! at most 16, so the difference between the two targets cannot be
//! reached by any value a member can hold — which is what makes `int`
//! usable at all for a wire format that fixes every width.
//!
//! ### `char` is an `int` and not an OCaml `char`
//!
//! An OCaml `char` really is exactly one octet, which an IDL `char` also
//! is, so the type looks like the right one. It carries no arithmetic:
//! there is no `+` on it, and every use goes through `Char.code` and
//! `Char.chr`, which are conversions to and from `int`. An IDL `octet`
//! is a number — a codec shifts it, masks it and compares it — and
//! `octet`, `uint8`, `int8` and `char` all take one octet on the wire.
//! Mapping three of them onto `int` and the fourth onto `char` would put
//! a conversion in every codec path for the fourth and give a member
//! whose IDL type is one octet a Rust-side neighbour it cannot be
//! compared with. All four are `int`.
//!
//! ### `unsigned long` is an `int64` and `unsigned long long` a `string`
//!
//! There is no unsigned integer type in OCaml. Three answers were
//! available for each width and only one of them is honest.
//!
//! For `unsigned long`: an `int32` would hold every bit pattern and read
//! them back as signed, so 4 000 000 000 arrives as −294 967 296 and
//! every comparison, every `Printf` and every bound check on it is
//! wrong, silently. An `int64` holds all 2³² values as themselves, and
//! costs four octets of memory that are not on the wire. `int64` is
//! used.
//!
//! For `unsigned long long`: `int64` holds every *bit pattern* and reads
//! the top half of the range back negative, and there is no wider
//! integer to widen into. The value is held as its decimal text, exactly
//! as `long double` and `fixed<P,S>` are, so that 18 446 744 073 709 551
//! 615 is that number in the member and not −1. This is the only
//! mapping in the ten columns where an integer becomes a string, and it
//! is the top bit that forces it.
//!
//! ### `float` and `double` are both `float`
//!
//! OCaml's `float` is an IEEE 754 binary64 — a C `double` (OCaml manual,
//! ch. "Floating-point numbers"). An IDL `float` is 32 bits, so a member
//! declared `float` here is *wider* than the wire form. Widening is
//! exact; the narrowing back to binary32 happens in the codec, at the
//! one place that knows the wire width. What the member cannot do is
//! distinguish a value the peer sent from the nearest binary32 to it,
//! which is the same limitation the Python column has and for the same
//! reason.
//!
//! ### `long double` is a `string`
//!
//! OCaml has no binary128 and no extended type. `float` has 53 bits of
//! significand where an IDL `long double` has 113, so a member declared
//! `float` would lose sixty bits at the assignment, before any codec
//! runs. The decimal text is held instead, as the Go and Python columns
//! do.
//!
//! ### `map<K,V>` is an array of entry records and never a `Hashtbl`
//!
//! A `map<K,V>` is written to the wire as an ordered sequence of
//! key/value pairs (DDS-XTypes 1.3 §7.4.1.1.4). `Hashtbl` has no order
//! at all — `Hashtbl.fold` visits bindings in an unspecified order and
//! `Hashtbl.add` keeps shadowed bindings — so a column holding a map in
//! one would produce a different octet stream from one value, and the
//! KeyHash of a keyed type would differ with it.
//!
//! An entry *record* rather than a `(k * v) array`: a tuple has no field
//! names, so a codec over it reads `fst` and `snd` and a reader of the
//! generated file cannot tell which half is the key from the type.
//!
//! ### `sequence<T>` is an array and never a list
//!
//! A `sequence<T>` goes on the wire as a count followed by the elements
//! (DDS-XTypes 1.3 §7.4.3.5.3). `List.length` is O(n), so an encoder
//! over a list would walk the whole structure once to write its first
//! four octets. `Array.length` is O(1).
//!
//! ### `T[N]` loses its bound
//!
//! OCaml has no fixed-size array type: `int32 array` is every length.
//! The bound is in the member's comment and enforced by the codec, which
//! is the same place the C column's `[N]` is checked on the way in.
//! Nothing in the type stops a program building a three-element array
//! for a `long[2]`.
//!
//! ### A union is a record and never a variant
//!
//! This is the load-bearing decision of the column. An OCaml variant
//! `type payload = Idle of int32 | Active of float` is the shape a
//! reader expects and it cannot represent an IDL union, because an IDL
//! discriminator may hold a value that selects no branch at all — a
//! union with no `default:` whose discriminator arrives as 7
//! (IDL 4.2 §7.4.1.4.4.1; DDS-XTypes 1.3 §7.4.1.1.5.2 says only the
//! selected branch is written, and none is selected). A variant has no
//! constructor for that state, so a decoder would have to invent a
//! branch, raise, or drop the discriminator. The record keeps the
//! discriminator as a member of its own and every branch as an `option`,
//! which represents all three states: this branch is selected and holds
//! a value; this branch is not selected; no branch is selected.
//!
//! ### An aggregate with no members is `unit`
//!
//! `type t = { }` is a syntax error — an OCaml record has at least one
//! field. The C column refuses the case by name for a related reason
//! (C23 §6.7.2.1p8). OCaml has an answer C does not: `unit` is the type
//! with exactly one value and no data, which is what an IDL aggregate
//! with no members is. `type m_s = unit` is emitted, and
//! `tests/interop-corpus/empty-struct` renders and compiles.
//!
//! ### What one flat structure costs
//!
//! Flattening puts every declaration in one structure, and OCaml's
//! namespaces are then what keeps them apart: a type name and a value
//! name never collide, so `typedef long limit;` and `const long limit`
//! both keep their spelling. Constructors and record fields are a
//! different matter — a second `type` declaring a field `x` shadows the
//! first for a record literal written without an annotation, and the
//! compiler resolves it by type where it can and warns where it cannot.
//! Nothing generated here writes a record literal, so the shadowing
//! reaches only a consumer's own code and is resolved there by
//! annotating the record. It is why every enumeration's two conversions
//! are emitted directly after the group that declares its constructors
//! and not in a block at the end of the file.
//!
//! ### Where the IDL spelling of a name is kept
//!
//! `lower_ident` folds case and neither it nor the flattening inverts:
//! `pose` could have come from `Pose` or from `pose`. DDS-XTypes 1.3
//! §7.3.1.2.1.1 computes a member's ID under `@autoid(HASH)` and under
//! `@hashid` from the first four octets of the MD5 of its **IDL** name,
//! so a file that only knew the OCaml spelling could not compute the ID
//! for the member it is writing. OCaml has no struct tag and no
//! attribute a program can read back at run time, so the IDL spelling
//! goes into the comment above the declaration and beside the field —
//! which is what `zerodds_lang_rules::adapter::c` has to do too, and for
//! the same lack.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::ocaml;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "OCaml";

/// The prefix of a name this column invented.
///
/// Only a map entry needs one. Lower case, because it names a type.
const INVENTED_PREFIX: &str = "zd_map_entry_";

/// The OCaml field name of a union's discriminator.
///
/// `_d`, which is what every other column calls it. A `lowercase-ident`
/// may begin with an underscore (OCaml manual, ch. "Lexical
/// conventions"), so unlike the Go column — which has to spell it `D`
/// because Go cannot export a name starting with one — nothing forces a
/// different name here.
pub const DISCRIMINATOR: &str = "_d";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep the walk follows a chain of anonymous aliases.
///
/// The same bound the layout planner uses. A chain that does not
/// terminate is a defect in the model and not a reason to hang.
const MAX_ALIAS_DEPTH: usize = 64;

/// Build the OCaml declarations a rule application produced.
///
/// The order is a topological one over the type graph, dependencies
/// first, with each strongly connected component emitted as one
/// `and`-joined group. Constants, exceptions and interfaces follow,
/// because each of them can name a type and no type can name any of
/// them.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no OCaml shape, or two declarations reach one
/// OCaml spelling — never a silently dropped declaration.
pub fn ocaml_items(ast: &Ast, applied: &Applied) -> Result<Vec<ocaml::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut names = Names::default();

    let declared = declared_types(ast, applied);
    for component in type_groups(ast, &declared) {
        let start = out.len();
        let mut items = Vec::new();
        let mut trailing = Vec::new();
        for index in &component {
            let Some(id) = declared.get(*index).copied() else {
                continue;
            };
            let built = build_type(ast, applied, id, &mut names)?;
            items.push(built.decl);
            trailing.extend(built.bindings);
        }
        if items.is_empty() {
            continue;
        }
        // An OCaml abbreviation may not take part in a cycle, however
        // the cycle is spelled: `type a = b array and b = a` is refused
        // with "The type abbreviation a is cyclic", because `b array`
        // contains `b`. A record or a variant breaks the chain — the
        // constructor is a definition and not an expansion — so a group
        // holding one is fine and a group holding none is not.
        let recursive = items.len() > 1 || self_referential(ast, &declared, &component);
        if recursive
            && !items.iter().any(|item| {
                matches!(
                    item.body,
                    ocaml::TypeBody::Record(_) | ocaml::TypeBody::Variant(_)
                )
            })
        {
            let spelled: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a cycle through type abbreviations alone ({}) — OCaml expands an \
                     abbreviation and reports the cycle rather than accepting it",
                    spelled.join(", ")
                ),
                language: LANGUAGE,
                at: spelled
                    .first()
                    .map_or_else(String::new, |name| (*name).to_string()),
            });
        }
        out.push(ocaml::Decl::Types(ocaml::TypeGroup {
            doc: if recursive {
                Doc::lines(&[
                    "A recursive group. OCaml resolves a type name against what precedes",
                    "it and against its own group, so a cycle in the IDL type graph is one",
                    "declaration joined by `and` and cannot be written any other way.",
                ])
            } else {
                Doc::none()
            },
            items,
        }));
        // The bindings an enumeration and a bitmask need go directly
        // after the group that declares them, and not in a block at the
        // end of the file. A variant constructor is bound in the
        // structure's own scope, so a second enumeration declaring the
        // same constructor name shadows the first from its declaration
        // onwards; emitting each conversion beside its type is what
        // keeps every arm referring to the constructor it was written
        // for.
        out.extend(trailing.into_iter().map(ocaml::Decl::Let));
        let ids: Vec<Id<TypeNode>> = component
            .iter()
            .filter_map(|index| declared.get(*index).copied())
            .collect();
        weave_group(ast, &ids, start, &mut out);
    }

    // Exceptions are not types, so they are walked separately — the
    // model keeps them in an arena of their own, and every sibling
    // materialiser does the same. They come after the types because an
    // exception member may name a type and no type may name an
    // exception.
    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "OCamlError").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut names.types, &name, &node.name)?;
        let mut pairs = Vec::new();
        for member in &node.members {
            if let Ok(member_node) = ast.member(*member) {
                pairs.push((
                    ocaml::lower_ident(&member_node.name),
                    member_node.name.clone(),
                ));
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
            fields.push(ocaml::Field {
                doc: member_doc(ast, target, &member_node.name, false),
                name: Ident::new(&ocaml::lower_ident(&member_node.name)),
                type_: ocaml_member_type(ast, target, &member_node.name)?,
            });
        }
        out.push(ocaml::Decl::Types(ocaml::TypeGroup::one(ocaml::TypeDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line("An exception, as the record its members make."),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            body: record_or_unit(fields),
        })));
    }

    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "OCamlConst").is_none() {
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
        let declared_type = node.type_.resolved();
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut names.values, &name, &node.name)?;
        let annotation = match declared_type {
            Some(target) => Some(ocaml_value_type(ast, target)?),
            None => None,
        };
        out.push(ocaml::Decl::Let(ocaml::LetDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            type_: annotation,
            body: ocaml_constant(ast, &value, declared_type, &node.name)?,
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "OCamlInterface").is_none() {
            continue;
        }
        // A class type declares a *type* name, so it shares the
        // namespace every `type` declaration is in.
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut names.types, &name, &node.name)?;
        let mut methods = Vec::new();
        let mut signatures = Vec::new();
        // The whole answer and not the declared one. OCaml would carry
        // the inherited methods through an `inherit` clause, and
        // `ocaml::ClassTypeDecl` has no place for one: a method left out
        // here is one the class type does not have, and OMG IDL 4.2
        // §7.4.3 gives a derived interface every one of its bases'
        // operations *and attributes*.
        // `interface::all_operations_flattened` gives both — the
        // get_/set_ pair DDS-RPC 1.0 §7.5.1.1.3 implies for each
        // attribute, besides the declared operations — which
        // `Ast::all_operations_of` alone never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            signatures.push((ocaml::lower_ident(&op.name), op.name.clone()));
            let mut params = Vec::new();
            for param in &op.params {
                params.push(ocaml_member_type(ast, param.type_, &param.name)?);
            }
            // `unit` for an operation returning `void`. A method with no
            // parameters is written `method m : t` and takes `unit`
            // nowhere: in OCaml a nullary method is a value of its
            // result type, and adding a `unit ->` would change the
            // signature for no gain.
            let result = match op.returns {
                Some(returns) => ocaml_member_type(ast, returns, &op.name)?,
                None => ocaml::Type::Primitive(ocaml::Primitive::Unit),
            };
            let doc = op
                .raises_line()
                .map_or_else(Doc::none, |line| Doc::line(&line));
            methods.push(ocaml::Method {
                doc,
                name: Ident::new(&ocaml::lower_ident(&op.name)),
                params,
                result,
            });
        }
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        out.push(ocaml::Decl::ClassType(ocaml::ClassTypeDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface, as the class type its operations and attributes",
                    "make — an attribute a get_/set_ pair of methods, `readonly` only",
                    "the first. A method type has no place for a direction, so an",
                    "`out` or `inout` parameter is an argument here like any other;",
                    "the direction is in the model and the RPC arc reads it there.",
                    "`raises`/`getraises`/`setraises` are documented on the method",
                    "they belong to.",
                    "",
                    "A derived interface carries its bases' operations and attributes",
                    "too: there is no `inherit` clause here, so IDL 4.2 7.4.3's",
                    "inherited members are written into the class type rather than",
                    "reached through one.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            methods,
        }));
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::OCAML, out))
}

/// Weave the `@verbatim` text of one type group's members around and
/// into what it declared: `out[start..]`, the group and the bindings
/// after it.
///
/// `BEGIN_DECLARATION` and `END_DECLARATION` open and close the body of a
/// record. The rest surrounds the whole group: `type a = … and b = …` is
/// one declaration, and a line between its `and`s would split it. So a
/// recursive group's text is the members' text in declaration order,
/// before the `type` and after the last binding.
fn weave_group(ast: &Ast, ids: &[Id<TypeNode>], start: usize, out: &mut Vec<ocaml::Decl>) {
    let mut merged = verbatim::Slots::default();
    for id in ids {
        let mut slots = verbatim::declaration_slots(ast, *id, verbatim::aliases::OCAML);
        if slots.is_empty() {
            continue;
        }
        let name = ocaml_type_name(ast, *id);
        if let Some(ocaml::Decl::Types(group)) = out.get_mut(start) {
            if let Some(record) = group.items.iter_mut().find(|item| {
                item.name.as_str() == name && matches!(item.body, ocaml::TypeBody::Record(_))
            }) {
                record.body_text.begin.append(&mut slots.begin);
                record.body_text.end.append(&mut slots.end);
            }
        }
        merged.before.append(&mut slots.before);
        merged.begin.append(&mut slots.begin);
        merged.end.append(&mut slots.end);
        merged.after.append(&mut slots.after);
    }
    verbatim::weave_with(merged, start, out, ocaml::Decl::Verbatim, |_, _, _| {});
}

/// The file-level half; the body is `weave_group`'s, which knows the
/// group a type is in.
impl verbatim::Carrier for ocaml::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(&mut self, _: &str, _: &mut Vec<String>, _: &mut Vec<String>) -> bool {
        false
    }
}

/// The OCaml declarations, in a file.
///
/// A caller that only wants the declarations uses [`ocaml_items`]; this
/// is what a generator writes to disk.
///
/// # Errors
/// As [`ocaml_items`].
pub fn ocaml_file(ast: &Ast, applied: &Applied) -> Result<ocaml::File, MaterialiseError> {
    Ok(ocaml::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "OCaml has no attribute a program reads back at run time, so the IDL",
            "spelling of every renamed declaration and every renamed member is in",
            "the comment beside it. A member's ID under @autoid(HASH) is the MD5",
            "of the IDL name (DDS-XTypes 1.3 7.3.1.2.1.1), and neither the case",
            "fold nor the flattening of a scoped name inverts.",
        ]),
        decls: ocaml_items(ast, applied)?,
    })
}

// -- the declaration order ------------------------------------------------

/// The names already taken, one list per OCaml namespace.
///
/// OCaml keeps type names and value names apart — `type limit = int32`
/// and `let limit = 5` coexist and neither hides the other (OCaml
/// manual, ch. "Names") — so one flat list would refuse a pair the
/// compiler accepts. Constructors and record fields are two further
/// namespaces and are not tracked here: a later declaration of either
/// shadows an earlier one rather than colliding with it, which is why
/// every enumeration's conversions are emitted directly beside the type
/// that declares its constructors.
#[derive(Default)]
struct Names {
    /// Type constructors, which is every `type` and every `class type`.
    types: Vec<(String, String)>,
    /// Value names, which is every `let`.
    values: Vec<(String, String)>,
}

/// What one type declaration produced.
struct Built {
    /// The declaration itself, which joins its group.
    decl: ocaml::TypeDecl,
    /// The `let` bindings that follow the group it is in.
    bindings: Vec<ocaml::LetDecl>,
}

/// Every type that produces a `type` declaration, in the model's order.
///
/// A primitive, a string, a `fixed`, a sequence and an array are written
/// where the member is and declare nothing; an anonymous alias is
/// transparent; a construct the rules created no node for is not this
/// column's. A `map<K,V>` declares its entry record once per distinct
/// entry name, so two identical maps collapse onto one declaration
/// rather than two that differ by a counter.
fn declared_types(ast: &Ast, applied: &Applied) -> Vec<Id<TypeNode>> {
    let mut out = Vec::new();
    let mut map_names: Vec<String> = Vec::new();
    for (id, node) in ast.types() {
        let created = match &node.kind {
            TypeKind::Map { .. } => {
                let name = ocaml_map_entry_name(ast, id);
                if map_names.contains(&name) {
                    continue;
                }
                map_names.push(name);
                out.push(id);
                continue;
            }
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            TypeKind::Alias { .. } if node.name.is_empty() => continue,
            TypeKind::Alias { .. } => "OCamlAlias",
            TypeKind::Enumeration { .. } => "OCamlVariant",
            TypeKind::Bitmask { .. } => "OCamlBitmask",
            TypeKind::Bitset { .. } => "OCamlBitset",
            TypeKind::Structure { .. } => "OCamlRecord",
            TypeKind::Valuetype { .. } => "OCamlValue",
            TypeKind::Union { .. } => "OCamlUnion",
            TypeKind::Native => "OCamlNative",
            // A boxed value is a declared name with no rule of its own,
            // as it is in the Go column: it is a `T option`, and the
            // declaration is what gives the member something to name.
            TypeKind::ValueBox { boxed } => {
                if boxed.resolved().is_some() {
                    out.push(id);
                }
                continue;
            }
            // An object reference belongs to the RPC arc, an annotation
            // type has no runtime form at all, and `any` and `ValueBase`
            // are decided at run time. Named here rather than passed
            // over, so the coverage table and this file say the same
            // thing; a member that reaches one is refused by name in
            // `ocaml_member_type`.
            TypeKind::Primitive(_)
            | TypeKind::String { .. }
            | TypeKind::Fixed { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Array { .. }
            | TypeKind::ObjectRef { .. }
            | TypeKind::AnnotationType { .. }
            | TypeKind::Any
            | TypeKind::ValueBase => continue,
        };
        if ghost_created_for(applied, node.node_id, created).is_some() {
            out.push(id);
        }
    }
    out
}

/// The declarations each strongly connected component of the type graph
/// holds, dependencies first.
///
/// Tarjan's algorithm, iteratively. It is used rather than the C
/// column's post-order walk because that walk answers only "what must
/// come first" and stops at a cycle; here a cycle is not an error, it is
/// a group, and the group's membership is exactly a strongly connected
/// component.
///
/// Tarjan emits a component only after every component it points to, so
/// the result is already in dependency order. Within a component the
/// indices are sorted, so two runs over one model produce the same file.
#[must_use]
fn type_groups(ast: &Ast, declared: &[Id<TypeNode>]) -> Vec<Vec<usize>> {
    let count = declared.len();
    let edges: Vec<Vec<usize>> = (0..count)
        .map(|index| {
            let mut targets = Vec::new();
            if let Some(id) = declared.get(index) {
                for reference in body_references(ast, *id) {
                    if let Some(other) = index_of(ast, declared, reference) {
                        if !targets.contains(&other) {
                            targets.push(other);
                        }
                    }
                }
            }
            targets
        })
        .collect();

    const UNVISITED: usize = usize::MAX;
    let mut order = vec![UNVISITED; count];
    let mut low = vec![0_usize; count];
    let mut on_stack = vec![false; count];
    let mut stack: Vec<usize> = Vec::new();
    let mut next = 0_usize;
    let mut out: Vec<Vec<usize>> = Vec::new();

    for root in 0..count {
        if order.get(root).copied() != Some(UNVISITED) {
            continue;
        }
        let mut calls: Vec<(usize, usize)> = Vec::new();
        set(&mut order, root, next);
        set(&mut low, root, next);
        next += 1;
        stack.push(root);
        set(&mut on_stack, root, true);
        calls.push((root, 0));

        while let Some((node, cursor)) = calls.last().copied() {
            let child = edges.get(node).and_then(|list| list.get(cursor)).copied();
            if let Some(child) = child {
                if let Some(last) = calls.last_mut() {
                    last.1 = cursor + 1;
                }
                if order.get(child).copied() == Some(UNVISITED) {
                    set(&mut order, child, next);
                    set(&mut low, child, next);
                    next += 1;
                    stack.push(child);
                    set(&mut on_stack, child, true);
                    calls.push((child, 0));
                } else if on_stack.get(child).copied() == Some(true) {
                    let candidate = order.get(child).copied().unwrap_or(UNVISITED);
                    let current = low.get(node).copied().unwrap_or(UNVISITED);
                    set(&mut low, node, current.min(candidate));
                }
                continue;
            }
            calls.pop();
            if low.get(node).copied() == order.get(node).copied() {
                let mut component = Vec::new();
                while let Some(top) = stack.pop() {
                    set(&mut on_stack, top, false);
                    component.push(top);
                    if top == node {
                        break;
                    }
                }
                // Sorted, so the group's written order is the model's
                // declaration order and not the traversal's.
                component.sort_unstable();
                out.push(component);
            }
            if let Some((parent, _)) = calls.last().copied() {
                let child_low = low.get(node).copied().unwrap_or(UNVISITED);
                let parent_low = low.get(parent).copied().unwrap_or(UNVISITED);
                set(&mut low, parent, parent_low.min(child_low));
            }
        }
    }
    out
}

/// Write into a slot, doing nothing where the index is out of range.
///
/// A helper rather than an index expression, because indexing would
/// panic and the workspace denies that in library code. Every index
/// here is in range by construction; this is what makes that not depend
/// on the reader believing it.
fn set<T>(slots: &mut [T], index: usize, value: T) {
    if let Some(slot) = slots.get_mut(index) {
        *slot = value;
    }
}

/// Whether a one-member component reaches itself.
///
/// Tarjan puts a type that names itself in a component of one, which is
/// indistinguishable from an ordinary type until the edge is looked at.
/// A self-reference still has to be written as a `type … = …` that
/// mentions its own name, which OCaml accepts for a record and refuses
/// for an abbreviation, so the distinction is what the guard above
/// needs.
fn self_referential(ast: &Ast, declared: &[Id<TypeNode>], component: &[usize]) -> bool {
    let [only] = component else {
        return false;
    };
    let Some(id) = declared.get(*only).copied() else {
        return false;
    };
    body_references(ast, id)
        .into_iter()
        .any(|reference| index_of(ast, declared, reference) == Some(*only))
}

/// The declared types a type's own declaration names.
///
/// Following through the shapes that declare nothing: a sequence, an
/// array and an anonymous alias are written where the member is, so
/// `sequence<Node>` in a member of `Node` is an edge from `Node` to
/// itself and the group has to hold it.
fn body_references(ast: &Ast, id: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut out = Vec::new();
    let Ok(node) = ast.type_(id) else {
        return out;
    };
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            // The base is held as the member `parent`, so the group has to hold it.
            if let Some(base) = super::c::struct_base(ast, id) {
                out.extend(value_references(ast, base, 0));
            }
            for member in ast.all_members_of(id) {
                if let Ok(member_node) = ast.member(member) {
                    if let Some(target) = member_node.type_.resolved() {
                        out.extend(value_references(ast, target, 0));
                    }
                }
            }
        }
        TypeKind::Union { discriminator, .. } => {
            if let Some(target) = discriminator.resolved() {
                out.extend(value_references(ast, target, 0));
            }
            for case in ast.cases_of(id) {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        if let Some(target) = member.type_.resolved() {
                            out.extend(value_references(ast, target, 0));
                        }
                    }
                }
            }
        }
        TypeKind::Alias { aliased } => {
            if let Some(target) = aliased.resolved() {
                out.extend(value_references(ast, target, 0));
            }
        }
        TypeKind::ValueBox { boxed } => {
            if let Some(target) = boxed.resolved() {
                out.extend(value_references(ast, target, 0));
            }
        }
        TypeKind::Map { key, value, .. } => {
            if let Some(target) = key.resolved() {
                out.extend(value_references(ast, target, 0));
            }
            if let Some(target) = value.resolved() {
                out.extend(value_references(ast, target, 0));
            }
        }
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Native
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {}
    }
    out
}

/// The declared types that appear in the OCaml spelling of one value's
/// type.
///
/// zerodds-lint: recursion-depth 64 (collection and unnamed-typedef
/// nesting; bounded by `MAX_ALIAS_DEPTH`, past which the walk
/// answers with no references)
fn value_references(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Vec<Id<TypeNode>> {
    if depth > MAX_ALIAS_DEPTH {
        return Vec::new();
    }
    let Ok(node) = ast.type_(id) else {
        return Vec::new();
    };
    match &node.kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => element
            .resolved()
            .map(|target| value_references(ast, target, depth + 1))
            .unwrap_or_default(),
        TypeKind::Alias { aliased } if node.name.is_empty() => aliased
            .resolved()
            .map(|target| value_references(ast, target, depth + 1))
            .unwrap_or_default(),
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => Vec::new(),
        _ => vec![id],
    }
}

/// Where a type sits in the declared list, following a map to whichever
/// map declared its entry record.
fn index_of(ast: &Ast, declared: &[Id<TypeNode>], id: Id<TypeNode>) -> Option<usize> {
    if let Some(index) = declared.iter().position(|other| *other == id) {
        return Some(index);
    }
    // Two `map<long,string>` written in two members are two type nodes
    // and one entry declaration, so the second reaches the first's.
    if matches!(
        ast.type_(id).map(|node| &node.kind),
        Ok(TypeKind::Map { .. })
    ) {
        let name = ocaml_map_entry_name(ast, id);
        return declared
            .iter()
            .position(|other| ocaml_map_entry_name(ast, *other) == name);
    }
    None
}

// -- one declaration ------------------------------------------------------

/// One declared type, with whatever `let` bindings belong beside it.
#[allow(clippy::too_many_lines)]
fn build_type(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    names: &mut Names,
) -> Result<Built, MaterialiseError> {
    let name = ocaml_type_name(ast, id);
    let Ok(node) = ast.type_(id) else {
        remember(&mut names.types, &name, &name)?;
        return Ok(Built {
            decl: ocaml::TypeDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::none(),
                name: Ident::new(&name),
                body: ocaml::TypeBody::Alias(ocaml::Type::Primitive(ocaml::Primitive::Unit)),
            },
            bindings: Vec::new(),
        });
    };

    match &node.kind {
        TypeKind::Map { key, value, .. } => {
            let name = ocaml_map_entry_name(ast, id);
            remember(&mut names.types, &name, &name)?;
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: name,
                });
            };
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: Doc::lines(&[
                        "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a",
                        "sequence of these, and the rules write the two halves with no",
                        "framing between them.",
                    ]),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Record(vec![
                        ocaml::Field {
                            doc: Doc::none(),
                            name: Ident::new("key"),
                            type_: ocaml_member_type(ast, key, &name)?,
                        },
                        ocaml::Field {
                            doc: Doc::none(),
                            name: Ident::new("value"),
                            type_: ocaml_member_type(ast, value, &name)?,
                        },
                    ]),
                },
                bindings: Vec::new(),
            })
        }

        TypeKind::Alias { aliased } => {
            remember(&mut names.types, &name, &node.name)?;
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            // An abbreviation and not a new type. OCaml abbreviations
            // are transparent, which is what IDL 4.2 §7.4.1.4.3 says an
            // alias is: it "does not define a new type".
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: array_note(ast, target, idl_note(&name, &node.name)),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Alias(ocaml_member_type(ast, target, &node.name)?),
                },
                bindings: Vec::new(),
            })
        }

        TypeKind::Enumeration { enumerators } => {
            remember(&mut names.types, &name, &node.name)?;
            let prefix = module_prefix(ast, id);
            let mut constructors = Vec::new();
            let mut values = Vec::new();
            let mut pairs = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "OCamlConstructor").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let spelled = ocaml::upper_ident(&format!("{prefix}{}", enum_node.name));
                pairs.push((spelled.clone(), enum_node.name.clone()));
                constructors.push(ocaml::Constructor {
                    doc: idl_note(&spelled, &enum_node.name),
                    name: Ident::new(&spelled),
                    args: Vec::new(),
                });
                values.push((spelled, value));
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            // An enumeration with no enumerator has no value, and an
            // abstract type is the OCaml type with none. `type e =` on
            // its own does not parse and `unit` would give it a value
            // the IDL does not.
            if constructors.is_empty() {
                return Ok(Built {
                    decl: ocaml::TypeDecl {
                        body_text: zerodds_lang_cst::common::BodyText::none(),
                        doc: with_idl_note(
                            Doc::line(
                                "An enumeration with no enumerator, so no value: abstract is \
                                 the OCaml type with none.",
                            ),
                            &name,
                            &node.name,
                        ),
                        name: Ident::new(&name),
                        body: ocaml::TypeBody::Abstract,
                    },
                    bindings: Vec::new(),
                });
            }
            let bindings = enum_conversions(&name, &values);
            for binding in &bindings {
                remember(&mut names.values, binding.name.as_str(), &node.name)?;
            }
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: with_idl_note(
                        Doc::lines(&[
                            "An enumeration. A variant constructor carries no number, so the",
                            "numbering DDS-XTypes 1.3 7.4.3.5.3 puts on the wire is in the two",
                            "conversions below and nowhere else.",
                        ]),
                        &name,
                        &node.name,
                    ),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Variant(constructors),
                },
                bindings,
            })
        }

        TypeKind::Bitmask { bits } => {
            remember(&mut names.types, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            let holder = ocaml_bitmask_holder(bound);
            let prefix = module_prefix(ast, id);
            let mut bindings = Vec::new();
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "OCamlBitLet").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let spelled = ocaml::lower_ident(&format!("{prefix}{}", bit_node.name));
                remember(&mut names.values, &spelled, &bit_node.name)?;
                // The mask is computed here and written as a literal.
                // OCaml's `lsl` is an operator on `int` alone, so a
                // shift of an `int32` would have to be written
                // `Int32.shift_left 1l 4` — a function call, evaluated
                // when the module is loaded, where a literal is the same
                // number with nothing to run.
                let mask =
                    one_shifted(position, holder).ok_or_else(|| MaterialiseError::Unevaluated {
                        construct: format!(
                            "bit position {position}, which does not fit the @bit_bound({bound}) \
                             holder"
                        ),
                        at: bit_node.name.clone(),
                    })?;
                bindings.push(ocaml::LetDecl {
                    doc: with_idl_note(
                        Doc::line(&format!("Bit {position}.")),
                        &spelled,
                        &bit_node.name,
                    ),
                    name: Ident::new(&spelled),
                    type_: Some(ocaml::Type::named(&name)),
                    body: ocaml::Expr::Literal(holder_literal(mask, holder)),
                });
            }
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: with_idl_note(
                        Doc::lines(&[
                            "A bitmask. OCaml has no unsigned integer, so Table 33 of",
                            "DDS-XTypes 1.3 7.4.1.1.3.2 gives the width and the sign is lost:",
                            "a bound of 32 with bit 31 set reads back negative.",
                        ]),
                        &name,
                        &node.name,
                    ),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Alias(ocaml::Type::Primitive(holder)),
                },
                bindings,
            })
        }

        TypeKind::Bitset { .. } => {
            remember(&mut names.types, &name, &node.name)?;
            let mut pairs = Vec::new();
            // The base's bitfields are members of the derived type too:
            // an inherited bitset is one flat run.
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                pairs.push((ocaml::lower_ident(field_ident), field_ident.clone()));
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
                let spelled = pairs
                    .get(index)
                    .map_or_else(String::new, |pair| pair.0.clone());
                index += 1;
                fields.push(ocaml::Field {
                    doc: idl_note(&spelled, field_ident),
                    name: Ident::new(&spelled),
                    type_: ocaml::Type::Primitive(ocaml_holder(field_node.bits)),
                });
            }
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: with_idl_note(
                        Doc::lines(&[
                            "A bitset. The fields are separate integers here; the packing into",
                            "the anchor is the codec's, because the order on the wire is the",
                            "declaration's.",
                        ]),
                        &name,
                        &node.name,
                    ),
                    name: Ident::new(&name),
                    body: record_or_unit(fields),
                },
                bindings: Vec::new(),
            })
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            remember(&mut names.types, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`; the wire
            // still carries the base's members first (§7.4.1.1.5.1).
            let members = ast.members_of(id).to_vec();
            let base = super::c::struct_base(ast, id);
            let mut pairs = Vec::new();
            if base.is_some() {
                pairs.push((ocaml::lower_ident("parent"), "parent".to_string()));
            }
            for member in &members {
                if let Ok(member_node) = ast.member(*member) {
                    pairs.push((
                        ocaml::lower_ident(&member_node.name),
                        member_node.name.clone(),
                    ));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            if let Some(base) = base {
                fields.push(ocaml::Field {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    name: Ident::new(&ocaml::lower_ident("parent")),
                    type_: ocaml_member_type(ast, base, "parent")?,
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
                let optional = member_node.facets.is_optional();
                let mut type_ = ocaml_member_type(ast, target, &member_node.name)?;
                if optional {
                    type_ = ocaml::Type::option(type_);
                }
                fields.push(ocaml::Field {
                    doc: member_doc(ast, target, &member_node.name, optional),
                    name: Ident::new(&ocaml::lower_ident(&member_node.name)),
                    type_,
                });
            }
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: idl_note(&name, &node.name),
                    name: Ident::new(&name),
                    body: record_or_unit(fields),
                },
                bindings: Vec::new(),
            })
        }

        TypeKind::Union { discriminator, .. } => {
            remember(&mut names.types, &name, &node.name)?;
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
            // `_d` would take the same OCaml spelling and make one of the
            // two unreachable.
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((ocaml::lower_ident(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = vec![ocaml::Field {
                doc: Doc::line("Which branch carries the value."),
                name: Ident::new(DISCRIMINATOR),
                type_: ocaml_member_type(ast, disc, &name)?,
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "OCamlCase").is_none() {
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
                fields.push(ocaml::Field {
                    doc: with_idl_note(
                        Doc::line("`None` unless the discriminator selects this branch."),
                        &ocaml::lower_ident(&member.name),
                        &member.name,
                    ),
                    name: Ident::new(&ocaml::lower_ident(&member.name)),
                    type_: ocaml::Type::option(ocaml_member_type(ast, target, &member.name)?),
                });
            }
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: with_idl_note(
                        Doc::lines(&[
                            "A union, as a record and not a variant. An IDL discriminator may",
                            "hold a value that selects no branch at all (IDL 4.2 7.4.1.4.4.1),",
                            "and a variant has no constructor for that state; only the selected",
                            "branch is written to the wire (DDS-XTypes 1.3 7.4.1.1.5.2).",
                        ]),
                        &name,
                        &node.name,
                    ),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Record(fields),
                },
                bindings: Vec::new(),
            })
        }

        TypeKind::Native => {
            remember(&mut names.types, &name, &node.name)?;
            // `Obj.t` and never an abstract `type handle`. An abstract
            // type declared in a structure has no value at all, so a
            // record with a member of one could never be built — a
            // `native` member would make the whole aggregate
            // unconstructible. `Obj.t` is the standard library's own
            // opaque type: it says nothing about what is held, which is
            // what `native` says, and it leaves the mapping to whatever
            // a facet named.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: with_idl_note(
                        Doc::lines(&[
                            "A native type. Opaque unless a facet named its mapping: `Obj.t`",
                            "carries a value of any shape and nothing may be read out of it",
                            "except by code that knows what was put in.",
                        ]),
                        &name,
                        &node.name,
                    ),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Alias(match mapping {
                        Some(spelling) => ocaml::Type::named(&spelling),
                        None => ocaml::Type::qualified("Obj", "t"),
                    }),
                },
                bindings: Vec::new(),
            })
        }

        TypeKind::ValueBox { boxed } => {
            remember(&mut names.types, &name, &node.name)?;
            let Some(target) = boxed.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "boxed value target".to_string(),
                    at: node.name.clone(),
                });
            };
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: with_idl_note(
                        Doc::line("A boxed value: `None` when absent."),
                        &name,
                        &node.name,
                    ),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Alias(ocaml::Type::option(ocaml_member_type(
                        ast, target, &name,
                    )?)),
                },
                bindings: Vec::new(),
            })
        }

        // Not reachable: `declared_types` never puts one of these in the
        // list. Answered rather than passed over, so that a kind added
        // to the model without a decision here is a `unit` with a
        // comment on it and not a missing declaration.
        _ => {
            remember(&mut names.types, &name, &node.name)?;
            Ok(Built {
                decl: ocaml::TypeDecl {
                    body_text: zerodds_lang_cst::common::BodyText::none(),
                    doc: Doc::line("A construct with no OCaml declaration of its own."),
                    name: Ident::new(&name),
                    body: ocaml::TypeBody::Alias(ocaml::Type::Primitive(ocaml::Primitive::Unit)),
                },
                bindings: Vec::new(),
            })
        }
    }
}

/// A record, or `unit` where there are no fields.
///
/// `type t = { }` is a syntax error: an OCaml record has at least one
/// field. `unit` is the type with exactly one value and no data, which
/// is what an IDL aggregate with no members is.
fn record_or_unit(fields: Vec<ocaml::Field>) -> ocaml::TypeBody {
    if fields.is_empty() {
        return ocaml::TypeBody::Alias(ocaml::Type::Primitive(ocaml::Primitive::Unit));
    }
    ocaml::TypeBody::Record(fields)
}

/// The two functions an enumeration's numbering lives in.
///
/// Both, and not only the forward one. A binding that can turn an
/// enumerator into its wire value but not back has lost the mapping for
/// a decoder, and the decoder is the side that meets a number it did not
/// choose.
///
/// The inverse raises rather than returning an `option`. A value that
/// names no enumerator is a peer disagreeing with this IDL about the
/// type, which DDS-XTypes 1.3 §7.4.3.5.3 does not permit — and an
/// `option` here would be a second absence beside `@optional`'s, at the
/// same member, meaning something different.
fn enum_conversions(name: &str, values: &[(String, i128)]) -> Vec<ocaml::LetDecl> {
    let forward = values
        .iter()
        .map(|(constructor, value)| ocaml::Arm {
            pattern: ocaml::Pattern::Construct(Ident::new(constructor)),
            body: ocaml::Expr::Literal(ocaml::int32_literal(*value)),
        })
        .collect();
    let mut backward: Vec<ocaml::Arm> = values
        .iter()
        .map(|(constructor, value)| ocaml::Arm {
            pattern: ocaml::Pattern::Literal(ocaml::int32_literal(*value)),
            body: ocaml::Expr::construct(constructor),
        })
        .collect();
    backward.push(ocaml::Arm {
        pattern: ocaml::Pattern::Bind(Ident::new("value")),
        body: ocaml::Expr::apply(
            ocaml::Expr::name("invalid_arg"),
            vec![ocaml::Expr::Paren(Box::new(ocaml::Expr::apply(
                ocaml::Expr::name("Printf.sprintf"),
                vec![
                    ocaml::Expr::Literal(Literal::string(&format!("{name}_of_int32: %ld"))),
                    ocaml::Expr::name("value"),
                ],
            )))],
        ),
    });
    vec![
        ocaml::LetDecl {
            doc: Doc::line(&format!(
                "The wire value of a `{name}`, which the variant itself does not carry."
            )),
            name: Ident::new(&ocaml::lower_ident(&format!("int32_of_{name}"))),
            type_: None,
            body: ocaml::Expr::Function(forward),
        },
        ocaml::LetDecl {
            doc: Doc::lines(&[
                &format!("The `{name}` a wire value names."),
                "Raises `Invalid_argument` for a value that names no enumerator: a peer",
                "sending one disagrees with this IDL about the type, and returning `None`",
                "would be a second kind of absence beside `@optional`'s.",
            ]),
            name: Ident::new(&ocaml::lower_ident(&format!("{name}_of_int32"))),
            type_: None,
            body: ocaml::Expr::Function(backward),
        },
    ]
}

// -- types ----------------------------------------------------------------

/// The OCaml spelling of a value's type, refusing what OCaml cannot
/// hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided at
/// run time — OCaml's `Obj.t` would compile and there would be no wire
/// form to generate a codec against, which is the silent-substitution
/// class this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn ocaml_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<ocaml::Type, MaterialiseError> {
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
    ocaml_value_type(ast, target)
}

/// The OCaml type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated record, a
/// codec's parameter and a defaulter's destination all have to name the
/// same OCaml type or the file does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a collection's element does
/// not resolve.
pub fn ocaml_value_type(ast: &Ast, id: Id<TypeNode>) -> Result<ocaml::Type, MaterialiseError> {
    value_type_at(ast, id, 0)
}

/// [`ocaml_value_type`], carrying how many connector steps the walk has
/// already taken.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the spelling nests past
/// `MAX_ALIAS_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and
/// anonymous-alias nesting; bounded by `MAX_ALIAS_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`). An aggregate is a leaf here — it
/// is named, not descended into — so only a cycle among the connectors
/// can come back round, and the counter is what stops that rather than
/// the frontend happening to build inline types as a tree.
fn value_type_at(
    ast: &Ast,
    id: Id<TypeNode>,
    depth: usize,
) -> Result<ocaml::Type, MaterialiseError> {
    if depth > MAX_ALIAS_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose OCaml spelling nests past {MAX_ALIAS_DEPTH}"),
            at: ocaml_type_name(ast, id),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(ocaml::Type::Primitive(ocaml::Primitive::Unit));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => ocaml_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units. An OCaml
        // `string` is a run of octets with no encoding of its own
        // (OCaml manual, ch. "Strings"), which is what makes that
        // possible, and it puts the §7.4.1.1.2 surrogate rule inside the
        // runtime rather than in a second copy of the Rust column's
        // answer.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            ocaml::Type::Primitive(ocaml::Primitive::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            ocaml::Type::array(value_type_at(ast, element, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // One `array` per dimension and no length anywhere: OCaml
            // has no fixed-size array type. `long grid[2][3]` is two
            // rows of three, and `int32 array array` says only that it
            // is an array of arrays — the bound is in the member's
            // comment and enforced by the codec.
            let mut type_ = value_type_at(ast, element, depth + 1)?;
            for _ in dims {
                type_ = ocaml::Type::array(type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            ocaml::Type::array(ocaml::Type::named(&ocaml_map_entry_name(ast, id)))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the OCaml type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_type_at(ast, target, depth + 1)?,
            None => ocaml::Type::Primitive(ocaml::Primitive::Unit),
        },
        _ => ocaml::Type::named(&ocaml_type_name(ast, id)),
    })
}

/// The OCaml type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without an OCaml
/// spelling is a compile error and not a member that silently takes
/// another's. The module documentation carries the argument for each
/// row; the two that surprise are `unsigned long` (an `int64`, so that
/// 4 000 000 000 is that number and not −294 967 296) and
/// `unsigned long long` (a decimal `string`, because no OCaml integer
/// holds 2⁶⁴−1).
#[must_use]
pub fn ocaml_primitive_type(kind: PrimitiveKind) -> ocaml::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => ocaml::Primitive::Bool,
        // Every one of these needs at most 16 bits, so it fits an OCaml
        // `int` on a 32-bit target as well as on a 64-bit one.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Char
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::WChar => ocaml::Primitive::Int,
        PrimitiveKind::Int32 => ocaml::Primitive::Int32,
        PrimitiveKind::UInt32 | PrimitiveKind::Int64 => ocaml::Primitive::Int64,
        PrimitiveKind::UInt64 | PrimitiveKind::Float128 => ocaml::Primitive::String,
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => ocaml::Primitive::Float,
    };
    ocaml::Type::Primitive(primitive)
}

/// The integer a bitfield of `bits` is held in.
///
/// `int` up to 30 bits and not up to 31: OCaml's `int` is 31 bits on a
/// 32-bit target, one of which is the sign, so a 31-bit unsigned
/// bitfield does not fit one there. 31 and 32 take an `int32`, which
/// holds every bit pattern and reads the top one back as a sign.
#[must_use]
pub const fn ocaml_holder(bits: u16) -> ocaml::Primitive {
    match bits {
        0..=30 => ocaml::Primitive::Int,
        31..=32 => ocaml::Primitive::Int32,
        _ => ocaml::Primitive::Int64,
    }
}

/// The integer a bitmask of `bound` bits is held in.
///
/// `int32` or `int64` and never `int`, although a bound of 8 would fit
/// one: the constants beside a bitmask are masks, and a mask whose type
/// changed with the bound would make `flags land ready` compile for one
/// bitmask and not for the next. DDS-XTypes 1.3 §7.4.1.1.3.2 Table 33
/// gives the width; the sign is lost, because OCaml has no unsigned
/// integer.
#[must_use]
pub const fn ocaml_bitmask_holder(bound: u16) -> ocaml::Primitive {
    if bound <= 32 {
        ocaml::Primitive::Int32
    } else {
        ocaml::Primitive::Int64
    }
}

/// `1 << position`, in the holder's own width, or [`None`] where the
/// position does not fit it.
///
/// Wrapped rather than widened: bit 31 of an `int32` mask is
/// −2 147 483 648 and bit 63 of an `int64` mask is
/// −9 223 372 036 854 775 808, because OCaml has no unsigned integer to
/// write them in. The alternative is a literal outside the type's range,
/// which `ocamlc` refuses outright.
fn one_shifted(position: u16, holder: ocaml::Primitive) -> Option<i128> {
    let width = match holder {
        ocaml::Primitive::Int64 => 64_u32,
        _ => 32_u32,
    };
    if u32::from(position) >= width {
        return None;
    }
    let raw = 1_i128.checked_shl(u32::from(position))?;
    Some(match holder {
        ocaml::Primitive::Int64 => i128::from(raw as i64),
        _ => i128::from(raw as i32),
    })
}

/// A literal in the holder's own width, with the suffix it needs.
fn holder_literal(value: i128, holder: ocaml::Primitive) -> Literal {
    match holder {
        ocaml::Primitive::Int64 => ocaml::int64_literal(value),
        ocaml::Primitive::Int32 => ocaml::int32_literal(value),
        _ => Literal::int(value),
    }
}

// -- names ----------------------------------------------------------------

/// A declared type's OCaml identifier.
///
/// The scoped name with the module separators flattened onto it and
/// every segment folded to lower case: `::common::util::Location` is
/// `common_util_location`.
#[must_use]
pub fn ocaml_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "zd_unnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => flatten(scoped.segments()),
        None => "zd_unnamed".to_string(),
    }
}

/// The entry record a `map<K,V>` needs a name for.
///
/// Structural, so two identical anonymous maps collapse onto one
/// declaration instead of two that differ only in a counter. The tag is
/// built from the IDL kind and not the OCaml spelling: `double` and
/// `float` are both `float` here and `unsigned long` and `long long` are
/// both `int64`, so a name built from the OCaml type would give
/// `map<unsigned long,V>` and `map<long long,V>` one declaration — which
/// is the collapse that would be wrong the moment the two differ on the
/// wire, and they already do.
#[must_use]
pub fn ocaml_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
    map_entry_name_at(ast, id, 0)
}

/// [`ocaml_map_entry_name`], carrying the depth of the walk that reached
/// it.
///
/// The depth is threaded rather than started over: `type_tag` reaches a
/// map's entry name here and this function reaches `type_tag` again, so
/// a counter reset at this door would leave the pair unbounded — the
/// `MAX_ALIAS_DEPTH` check in `type_tag` would see 0 on every turn of
/// the cycle.
///
/// zerodds-lint: recursion-depth 64 (key and value tags, via
/// `type_tag`; bounded by `MAX_ALIAS_DEPTH`, which names the type by its
/// arena slot rather than descending)
fn map_entry_name_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_ALIAS_DEPTH {
        return format!("{INVENTED_PREFIX}deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return format!("{INVENTED_PREFIX}unknown");
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return format!("{INVENTED_PREFIX}unknown");
    };
    let key = key
        .resolved()
        .map_or_else(|| "unknown".to_string(), |k| type_tag(ast, k, depth + 1));
    let value = value
        .resolved()
        .map_or_else(|| "unknown".to_string(), |v| type_tag(ast, v, depth + 1));
    format!("{INVENTED_PREFIX}{key}_{value}")
}

/// The name component a map entry takes from its key or value.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and map nesting,
/// via `map_entry_name_at`; bounded by `MAX_ALIAS_DEPTH`, which names
/// the type by its arena slot rather than descending)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_ALIAS_DEPTH {
        // The slot and not a bare `deep`: two types that both nest past
        // the bound would otherwise take one tag, and the records they
        // name would merge onto one declaration with nothing said.
        return format!("deep_{}", id.slot());
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
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
            Some(element) => format!("seq_of_{}", type_tag(ast, element, depth + 1)),
            None => "seq_of_unknown".to_string(),
        },
        Ok(TypeKind::Array { element, dims }) => match element.resolved() {
            Some(element) => {
                let mut name = format!("arr_of_{}", type_tag(ast, element, depth + 1));
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
        Ok(TypeKind::Map { .. }) => map_entry_name_at(ast, id, depth + 1),
        _ => ocaml_type_name(ast, id),
    }
}

/// The OCaml constructor an enumerator of `id` is spelled as — the one
/// spelling, shared by this materialiser, the codec backend and the
/// probe generator: the flattened module prefix and the enumerator
/// name, capitalised as a `capitalized-ident` has to be.
#[must_use]
pub fn ocaml_enumerator_name(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> String {
    ocaml::upper_ident(&format!("{}{enumerator}", module_prefix(ast, id)))
}

/// The flattened, lower-cased module path a file-scope binding carries.
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
    let Some(head) = segments.get(..segments.len() - 1) else {
        return String::new();
    };
    let mut prefix = flatten(head);
    prefix.push('_');
    prefix
}

/// The OCaml name a declaration outside the type arena takes.
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

/// Join scoped segments with `_`, folding each to lower case.
///
/// `_` and not concatenation: `commonutillocation` cannot be read back
/// to its segments, and two different scoped names would reach one OCaml
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
        .map(|segment| ocaml::lower_ident(segment))
        .collect::<Vec<String>>()
        .join("_")
}

// -- comments -------------------------------------------------------------

/// The comment a member carries: its IDL spelling, its array bound and
/// what `None` means.
fn member_doc(ast: &Ast, target: Id<TypeNode>, declared: &str, optional: bool) -> Doc {
    let mut doc = idl_note(&ocaml::lower_ident(declared), declared);
    if optional {
        doc.push(
            "Optional: `None` when the member is absent — DDS-XTypes 1.3 7.2.2.4.4.4.7. In a \
             @mutable type an absent member is not written at all; outside one its absence is \
             written.",
        );
    }
    array_note(ast, target, doc)
}

/// The bound an `array` cannot carry, written into the comment instead.
fn array_note(ast: &Ast, target: Id<TypeNode>, mut doc: Doc) -> Doc {
    if let Some(dims) = array_dims(ast, target, 0) {
        doc.push(&format!(
            "Fixed size in the IDL: {dims}. OCaml has no fixed-size array type, so the bound is \
             not in the type and the codec is what enforces it."
        ));
    }
    doc
}

/// The dimensions of an array type, following anonymous aliases.
///
/// zerodds-lint: recursion-depth 64 (unnamed-typedef chains; bounded by
/// `MAX_ALIAS_DEPTH`, past which the walk answers `None`)
fn array_dims(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<String> {
    if depth > MAX_ALIAS_DEPTH {
        return None;
    }
    let node = ast.type_(id).ok()?;
    match &node.kind {
        TypeKind::Array { dims, .. } => {
            let mut text = String::new();
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => text.push_str(&format!("[{length}]")),
                    Err(_) => text.push_str("[?]"),
                }
            }
            Some(text)
        }
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            array_dims(ast, aliased.resolved()?, depth + 1)
        }
        _ => None,
    }
}

/// The declaration's own IDL name, where the OCaml spelling moved it.
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
/// the same OCaml spelling.
///
/// One structure holds every declaration in the file, so this is a
/// whole-file check and not a per-scope one — which is the difference
/// from [`no_spelling_collision`], used for the members of one
/// aggregate.
///
/// Two things can bring two IDL declarations to one OCaml name and only
/// one of them is the case fold. Within a scope the fold cannot collide:
/// IDL 4.2 §7.2.3.1 puts every identifier in one namespace and compares
/// case-insensitively, so `Pose` beside `pose` is already an error the
/// frontend reports. What is left is the flattening — `a_b::C` and
/// `a::b_C` both give `a_b_c` — and OCaml would report
/// `Multiple definition of the type name a_b_c`, naming neither IDL
/// declaration.
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
            at: "the generated structure".to_string(),
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

// -- constants ------------------------------------------------------------

/// The OCaml expression a constant's value takes.
///
/// The literal's *form* is decided by the type the constant was declared
/// with and not by the value, because OCaml distinguishes `0`, `0l` and
/// `0L` and has no implicit conversion between them: `let x : int32 = 0`
/// does not type-check.
///
/// Two cases are not literals at all:
///
/// - A constant of an **enumeration** type is written as the
///   constructor. `let m : mode = 4l` does not type-check — a variant is
///   not an integer — and `mode_of_int32 4l` would be a function call
///   evaluated when the module loads, where the constructor is the same
///   value checked at compile time.
/// - A constant of a type held as a **decimal string** — an
///   `unsigned long long`, a `long double`, a `fixed<P,S>` — is written
///   as that decimal. It is the same text the member holds, so the
///   constant and a value assigned from it are the same thing.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for an enumeration constant
/// whose value names no enumerator, for a `wstring` holding a lone
/// surrogate, and for a `ConstValue` variant added to the model after
/// this was written.
fn ocaml_constant(
    ast: &Ast,
    value: &zerodds_ast::ConstValue,
    declared: Option<Id<TypeNode>>,
    at: &str,
) -> Result<ocaml::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: String| MaterialiseError::NoLanguageForm {
        construct,
        language: LANGUAGE,
        at: at.to_string(),
    };
    if let Some(target) = declared {
        if let Some(enumeration) = enumeration_behind(ast, target, 0) {
            let ConstValue::Integer(number) = value else {
                return Err(refuse(
                    "a constant of an enumeration type whose value is not a number".to_string(),
                ));
            };
            return enumerator_named(ast, enumeration, *number)
                .map(|spelled| ocaml::Expr::construct(&spelled))
                .ok_or_else(|| {
                    refuse(format!(
                        "a constant of an enumeration type whose value {number} names no \
                         enumerator, and an OCaml variant has no other spelling"
                    ))
                });
        }
    }
    let holder = declared
        .and_then(|target| literal_holder(ast, target, 0))
        .unwrap_or_else(|| holder_for(value));
    Ok(match value {
        ConstValue::Boolean(flag) => ocaml::Expr::Literal(Literal::Bool(*flag)),
        // The code point as a number: an IDL `char` is one octet in this
        // column and an IDL `wchar` two, and an OCaml `char` literal is
        // neither an `int` nor two octets.
        ConstValue::Char(character) => integer_expr(i128::from(u32::from(*character)), holder),
        ConstValue::WChar(point) => integer_expr(i128::from(*point), holder),
        ConstValue::Integer(number) => integer_expr(*number, holder),
        ConstValue::Float { mantissa, exponent } => {
            let text = constant::float_text(mantissa, exponent.as_deref());
            if matches!(holder, ocaml::Primitive::String) {
                // A `long double`. The member holds the decimal, so the
                // constant is that decimal and not a `float` that lost
                // sixty bits on the way to it.
                ocaml::Expr::Literal(Literal::Str(text))
            } else {
                ocaml::Expr::Literal(Literal::Float(text))
            }
        }
        ConstValue::Fixed { digits, scale } => {
            ocaml::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => ocaml::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => {
            ocaml::Expr::Literal(Literal::Str(constant::wstring_utf8(points).ok_or_else(
                || refuse("a `wstring` constant holding a lone surrogate".to_string()),
            )?))
        }
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a file that disagrees with the IDL and compiles.
        _ => {
            return Err(refuse(
                "a constant form this binding has no literal for".to_string(),
            ));
        }
    })
}

/// An integer value written in whatever form its holder takes.
fn integer_expr(number: i128, holder: ocaml::Primitive) -> ocaml::Expr {
    ocaml::Expr::Literal(match holder {
        ocaml::Primitive::Int32 => ocaml::int32_literal(number),
        ocaml::Primitive::Int64 => ocaml::int64_literal(number),
        // An `unsigned long long` is held as its decimal, so its
        // constant is that decimal too.
        ocaml::Primitive::String => Literal::Str(format!("{number}")),
        // `let x : float = 1` does not type-check in OCaml any more than
        // `let x : int32 = 1` does, so an integer written where a float
        // is declared gets the point it needs.
        ocaml::Primitive::Float => Literal::Float(format!("{number}.0")),
        _ => Literal::int(number),
    })
}

/// The holder a value takes when the constant declared no type.
fn holder_for(value: &zerodds_ast::ConstValue) -> ocaml::Primitive {
    use zerodds_ast::ConstValue;
    match value {
        ConstValue::Boolean(_) => ocaml::Primitive::Bool,
        ConstValue::Float { .. } => ocaml::Primitive::Float,
        ConstValue::String(_) | ConstValue::WString(_) | ConstValue::Fixed { .. } => {
            ocaml::Primitive::String
        }
        // An integer with no declared type is a `long` in IDL 4.2
        // §7.4.1.4.2 terms, and `int32` is what a `long` is here.
        _ => ocaml::Primitive::Int32,
    }
}

/// The primitive a declared constant's literal has to be written as,
/// following abbreviations.
///
/// zerodds-lint: recursion-depth 64 (typedef chains; bounded by
/// `MAX_ALIAS_DEPTH`, past which the walk answers `None`)
fn literal_holder(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<ocaml::Primitive> {
    if depth > MAX_ALIAS_DEPTH {
        return None;
    }
    let node = ast.type_(id).ok()?;
    match &node.kind {
        TypeKind::Primitive(kind) => match ocaml_primitive_type(*kind) {
            ocaml::Type::Primitive(primitive) => Some(primitive),
            _ => None,
        },
        TypeKind::String { .. } | TypeKind::Fixed { .. } => Some(ocaml::Primitive::String),
        TypeKind::Bitmask { .. } => Some(ocaml_bitmask_holder(ast.effective_bit_bound(id))),
        TypeKind::Alias { aliased } => literal_holder(ast, aliased.resolved()?, depth + 1),
        _ => None,
    }
}

/// The enumeration behind a declared type, following abbreviations.
///
/// zerodds-lint: recursion-depth 64 (typedef chains; bounded by
/// `MAX_ALIAS_DEPTH`, past which the walk answers `None`)
fn enumeration_behind(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<Id<TypeNode>> {
    if depth > MAX_ALIAS_DEPTH {
        return None;
    }
    let node = ast.type_(id).ok()?;
    match &node.kind {
        TypeKind::Enumeration { .. } => Some(id),
        TypeKind::Alias { aliased } => enumeration_behind(ast, aliased.resolved()?, depth + 1),
        _ => None,
    }
}

/// The constructor an enumeration's value names, as it is spelled here.
fn enumerator_named(ast: &Ast, enumeration: Id<TypeNode>, value: i128) -> Option<String> {
    let node = ast.type_(enumeration).ok()?;
    let TypeKind::Enumeration { enumerators } = &node.kind else {
        return None;
    };
    let prefix = module_prefix(ast, enumeration);
    for enumerator in enumerators {
        if ast.effective_enumerator_value(*enumerator) != Some(value) {
            continue;
        }
        if let Ok(enum_node) = ast.enumerator(*enumerator) {
            return Some(ocaml::upper_ident(&format!("{prefix}{}", enum_node.name)));
        }
    }
    None
}
