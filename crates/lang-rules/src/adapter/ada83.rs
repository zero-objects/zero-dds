// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada 83 CST as the other end of a rule.
//!
//! `adapter::ada` is the template this file follows wherever RM83 admits
//! the same construct RM 2012 does — a discriminated record with a
//! variant part (RM83 3.7.3), an access to an unconstrained array (RM83
//! 3.8), an incomplete type declaration (RM83 3.10.1) are all original
//! Ada 83, unchanged by any later revision. Where this file departs, it
//! is one of six differences, and each is ANSI/MIL-STD-1815A's and not
//! a stylistic one:
//!
//! ## 1. No `Interfaces`, no modular type, no `Ada.Strings.Unbounded`
//!
//! Annex B (`Interfaces`) and `Ada.Strings.Unbounded` are both Ada 95;
//! modular types are Ada 95 too (RM95 3.5.4). Every integer-holding
//! scalar this column names is `Long_Integer`; a `string`, a `wstring`,
//! a `fixed<P,S>` and a `long double` are all `Zd_Wire83.Text` — an
//! access to an unconstrained `String`, the same shape a `sequence`
//! already needs, since there is no `Unbounded_String` to reach for.
//! `unsigned long long`'s top half needs 64 unsigned bits no
//! `Long_Integer` GNAT builds reaches, so a member of that type holds
//! the two's-complement *pattern* — the negative `Long_Integer` whose
//! bits it is, exactly as a `uint64_t` sits in a signed register.
//! `runtime/ada-83`'s `Write_U64` writes that pattern, `Read_U64`
//! answers it, and `Json_U64` renders it unsigned again, so the range is
//! carried whole and nothing about it is a *type* this materialiser has
//! no name for.
//!
//! ## 2. An enumerator is a constant, never a literal
//!
//! `adapter::ada` needs `'Enum_Val`/`'Enum_Rep` to cross between an
//! enumeration literal and the representation clause's code, and both
//! are GNAT-implemented attributes documented against Ada 2012 and Ada
//! 2022 — nothing in this repository's reach confirms either answers
//! under `-gnat83`. Rather than gate a whole construct family on an
//! unverified compiler behaviour, an IDL `enum` is materialised exactly
//! as a `bitmask`'s bits already are: one named `Long_Integer` constant
//! per enumerator, holding its wire value directly. No enumeration type,
//! no representation clause — [`zerodds_lang_cst::ada83`] has no node
//! for either. A union's discriminant is `Long_Integer` (or `Boolean`)
//! for the identical reason, and its default is the first enumerator's
//! *value*, not a literal name.
//!
//! ## 3. No interface, no tagged type, no `abstract`
//!
//! RM83 has no chapter on tagged types. An IDL `interface` is refused
//! outright, by [`ada83_items`], before any per-construct rule would be
//! consulted for it. A `struct` that inherits is not this problem —
//! `all_members_of` already flattens a base's members into the derived
//! record here exactly as it does in `adapter::ada`, and needs nothing
//! new.
//!
//! ## 4. The incomplete-type cut, ported unchanged
//!
//! `adapter::ada`'s cut of RM 3.10.1 — an incomplete type declaration,
//! completed once the cycle's other half is written — is unchanged
//! machinery in RM83 3.10.1, and `ordered_types`, `pending`,
//! `predeclare` and `reference_type` below are that same machinery,
//! not a re-derivation of it: the clause number has not moved since
//! 1983. What is genuinely Ada 83's is `declare_type`'s `Array` arm,
//! which still refuses a cycle an array closes — RM 3.10.1 admits an
//! incomplete type only as the designated subtype of an *access* type,
//! never as an array element, in every Ada revision — and this is also
//! IDL 4.2 §7.4.1.4.4.4.4's own refusal: an array's element count is
//! fixed and non-zero, so nothing about a value can stop the descent,
//! in any language. `tools/wireprobe/wireprobe-zerodds-ada83-gen`
//! answers that one case before this materialiser ever runs, through
//! the same `zerodds_ast_layout` recursion check every column's codec
//! generation already goes through, so the verdict is `refused` (a
//! specification question) and not this file's own `no-form`.
//!
//! ## 5. No `raise … with "message"`
//!
//! RM83 11.3 admits only a bare exception name. Nothing in this file
//! raises — that is `runtime/ada-83`'s business — but it is why that
//! runtime's exception set does not match `runtime/ada`'s one
//! `Wire_Error`, and it is recorded here because a reader who diffs the
//! two runtimes will otherwise take the difference for an oversight.
//!
//! ## 6. No `(null record)` and no `<>`
//!
//! `zerodds_lang_cst::ada83::Expr` has no `BoxDefault`. A record with no
//! components and no discriminant has no aggregate at all in this
//! column — RM83 4.3.1 requires at least one `component_association`,
//! and the literal `(null record)` that later Ada gives the empty case
//! is an Ada 2005 addition (AI95-287). The *type* declaration is still
//! written — `is null record;` is original RM83 3.8 — so this reaches
//! only a filled sample, which is `tools/wireprobe/wireprobe-zerodds-ada83-gen`'s
//! concern and not this file's.

use crate::adapter::cst::MaterialiseError;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, Bound, CaseLabel, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::ada83;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Ada 83";

/// The package a generated spec declares.
pub const PACKAGE: &str = "Zd83_Types";

/// The file name a generated spec has to be written under. GNAT derives
/// the unit name from the file name.
pub const FILE_NAME: &str = "zd83_types.ads";

/// The prefix of the array type behind a `sequence<T>`.
const SEQ_PREFIX: &str = "Zd_Seq_";
/// The prefix of an array type declared for one IDL array dimension.
const ARR_PREFIX: &str = "Zd_Arr_";
/// The prefix of the access type behind a `map<K,V>`.
const MAP_PREFIX: &str = "Zd_Map_";
/// The prefix of the entry record behind a `map<K,V>`.
const MAP_ENTRY_PREFIX: &str = "Zd_Map_Entry_";
/// The prefix of the discriminated record behind an `@optional` member.
const OPT_PREFIX: &str = "Zd_Opt_";
/// The prefix of the access type behind a member held by reference.
const REF_PREFIX: &str = "Zd_Ref_";
/// The suffix of the array type an access type designates.
const ARRAY_SUFFIX: &str = "_Array";

/// The discriminant name of a generated union record. `D`, as in
/// `adapter::ada`: RM 2.3 requires an identifier to begin with a
/// letter, and that has not moved since RM83 2.3.
pub const DISCRIMINANT: &str = "D";

/// The discriminant name of a generated optional record.
pub const PRESENT: &str = "Present";

/// The component name a generated wrapper record carries its value in.
pub const VALUE: &str = "Value";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep the walk follows the type graph.
const MAX_DEPTH: usize = 64;

/// One name the generated package declares.
#[derive(Debug, Clone)]
struct Declared {
    /// The folded key two Ada identifiers are the same identifier under.
    key: String,
    /// The IDL name behind it.
    declared: String,
}

/// What the emitter has to remember while it writes a cut cycle.
///
/// Ported unchanged from `adapter::ada`'s identical struct: RM83
/// 3.10.1 is RM 3.10.1, and all four lists are empty for a model whose
/// type graph is acyclic, which is every corpus case but seven — and
/// six of those seven, after this port.
#[derive(Debug, Clone, Default)]
struct Cycle {
    /// The types the order names before it declares them (RM83 3.10.1).
    cut: Vec<Id<TypeNode>>,
    /// The types whose declaration has already been written.
    completed: Vec<Id<TypeNode>>,
    /// The types an incomplete declaration has been written for. Every
    /// one of these has to be completed before the package ends.
    predeclared: Vec<Id<TypeNode>>,
    /// The declarations held back until every type they name is
    /// complete.
    deferred: Vec<ada83::Decl>,
}

/// The declaration a use of `id` waits for, if it has not been written.
fn pending(ast: &Ast, id: Id<TypeNode>, cycle: &Cycle) -> Option<Id<TypeNode>> {
    let target = declared_target(ast, id)?;
    if cycle.cut.contains(&target) && !cycle.completed.contains(&target) {
        return Some(target);
    }
    // A **named** typedef of a type in the cut waits for the same thing
    // its target waits for; see `adapter::ada::pending`'s identical
    // reasoning (`recursive-typedef-sequence` is the corpus case this
    // answers).
    let end = alias_end(ast, target);
    if end != target && cycle.cut.contains(&end) && !cycle.completed.contains(&end) {
        return Some(end);
    }
    None
}

/// The type an alias chain ends at, named links included.
fn alias_end(ast: &Ast, id: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => current = next,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// Whether [`ada83_items`] declares the member of `aggregate` whose type
/// is `member_type` as an access type: an `@optional` or `@external`
/// member whose type is still incomplete where `aggregate` is declared
/// (the `pending` arm of the record declaration, RM83 3.10.1).
///
/// The same question and the same answer as
/// `crate::adapter::ada::held_by_reference`, over this column's copy of
/// `ordered_types`: the codec, the Table 9 defaulter and the probe reach
/// that member as the access it is — `/= null`, `.all`, `new T'(…)` — and
/// not as the `Present`/`Value` record every other `@optional` member is.
#[must_use]
pub fn held_by_reference(
    ast: &Ast,
    aggregate: Id<TypeNode>,
    member_type: Id<TypeNode>,
    optional: bool,
    external: bool,
) -> bool {
    if !(optional || external) {
        return false;
    }
    let Ok(plan) = ordered_types(ast) else {
        return false;
    };
    let position = |id: Id<TypeNode>| plan.order.iter().position(|other| *other == id);
    let Some(at) = position(aggregate) else {
        return false;
    };
    let waiting =
        |id: Id<TypeNode>| plan.cut.contains(&id) && position(id).is_none_or(|place| place >= at);
    let Some(target) = declared_target(ast, member_type) else {
        return false;
    };
    if waiting(target) {
        return true;
    }
    let end = alias_end(ast, target);
    end != target && waiting(end)
}

/// The type an access [`held_by_reference`] answers for designates: the
/// declared target, or the end of its alias chain where that is what
/// the cycle waits on — the subtype mark a `new T'(…)` allocator names.
#[must_use]
pub fn reference_designated(ast: &Ast, member_type: Id<TypeNode>) -> Option<Id<TypeNode>> {
    let target = declared_target(ast, member_type)?;
    let Ok(plan) = ordered_types(ast) else {
        return Some(target);
    };
    if plan.cut.contains(&target) {
        return Some(target);
    }
    Some(alias_end(ast, target))
}

/// The type whose declaration a use of `id` resolves to.
fn declared_target(ast: &Ast, id: Id<TypeNode>) -> Option<Id<TypeNode>> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        let node = ast.type_(current).ok()?;
        match &node.kind {
            TypeKind::Primitive(_)
            | TypeKind::String { .. }
            | TypeKind::Fixed { .. }
            | TypeKind::Any
            | TypeKind::ValueBase
            | TypeKind::ObjectRef { .. }
            | TypeKind::AnnotationType { .. } => return None,
            TypeKind::Alias { aliased } if node.name.is_empty() => {
                current = aliased.resolved()?;
            }
            _ => return Some(current),
        }
    }
    None
}

/// Write `type X;` for a type named before it is declared, once.
fn predeclare(ast: &Ast, id: Id<TypeNode>, out: &mut Vec<ada83::Decl>, cycle: &mut Cycle) {
    if cycle.predeclared.contains(&id) {
        return;
    }
    cycle.predeclared.push(id);
    out.push(ada83::Decl::Type(ada83::TypeDecl {
        doc: Doc::lines(&[
            "Declared here and completed below (RM83 3.10.1). The IDL type reaches",
            "itself, so the name is needed before the type can be written; between",
            "the two an incomplete type may only be the designated subtype of an",
            "access type, which is what every use of it here is.",
        ]),
        name: Ident::new(&ada83_type_name(ast, id)),
        def: ada83::TypeDef::Incomplete,
    }));
}

/// `type X;` for a name this column invented, with a reason.
fn incomplete(name: &str, why: &str) -> ada83::Decl {
    ada83::Decl::Type(ada83::TypeDecl {
        doc: Doc::line(why),
        name: Ident::new(name),
        def: ada83::TypeDef::Incomplete,
    })
}

/// The access type a member held by reference takes, declared if new.
/// One per designated type, named structurally so two members of one
/// type collapse onto one declaration.
fn reference_type(
    ast: &Ast,
    target: Id<TypeNode>,
    out: &mut Vec<ada83::Decl>,
    emitted: &mut Vec<Declared>,
    cycle: &mut Cycle,
) -> Result<String, MaterialiseError> {
    let name = format!("{REF_PREFIX}{}", type_tag(ast, target));
    if already(emitted, &name) {
        return Ok(name);
    }
    remember(emitted, &name, &name)?;
    predeclare(ast, target, out, cycle);
    out.push(ada83::Decl::Type(ada83::TypeDecl {
        doc: Doc::lines(&[
            "A reference to a type that reaches this one back. `null` is the absent",
            "state; RM83 3.10.1(9) admits an incomplete designated subtype, which is",
            "why this declaration can stand before the type it designates.",
        ]),
        name: Ident::new(&name),
        def: ada83::TypeDef::Access(Path::single(&ada83_type_name(ast, target))),
    }));
    Ok(name)
}

/// Build the Ada 83 declarations a rule application produced.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Ada 83 form — an IDL `interface`, a type
/// that reaches itself, `any`, `ValueBase`, an object reference — or two
/// declarations reach one Ada identifier.
pub fn ada83_items(ast: &Ast, applied: &Applied) -> Result<Vec<ada83::Decl>, MaterialiseError> {
    // RM83 has no interface type, no tagged type, no `abstract`. Refused
    // here, once, rather than per-operation: a partial interface with
    // some operations refused and some not would be a spec that
    // compiles into something the IDL never declared.
    if let Some((_, node)) = ast.interfaces().next() {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!(
                "`{}`, an IDL interface — Ada 83 has no interface type, no tagged \
                 type and no `abstract` primitive operation; all three are Ada 95 \
                 or Ada 2005 additions (RM95 3.9.4 has no RM83 counterpart)",
                node.name
            ),
            language: LANGUAGE,
            at: node.name.clone(),
        });
    }

    let mut out = Vec::new();
    let mut emitted: Vec<Declared> = Vec::new();

    let plan = ordered_types(ast)?;
    let mut cycle = Cycle {
        cut: plan.cut,
        completed: Vec::new(),
        predeclared: Vec::new(),
        deferred: Vec::new(),
    };
    for id in plan.order {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted, &mut cycle)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::ADA83),
            &ada83_type_name(ast, id),
            start,
            &mut out,
        );
    }
    // The completions of the incomplete declarations, after every type
    // they name. RM83 3.10.1 requires the completion to be in the same
    // declarative part, and by here every aggregate in it is written.
    out.append(&mut cycle.deferred);
    for id in &cycle.predeclared {
        if !cycle.completed.contains(id) {
            let name = ada83_type_name(ast, *id);
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "`{name}`, declared incomplete to break a cycle and never completed \
                     — RM83 3.10.1 requires the completion in the same declarative part"
                ),
                language: LANGUAGE,
                at: name,
            });
        }
    }

    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "Ada83Const").is_none() {
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
            Some(target) => ada83_value_path(ast, target)?,
            None => ada83::Primitive::LongInteger.path(),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(ada83::Decl::Object(ada83::ObjectDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            type_: type_.clone(),
            value: ada83_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "Ada83Error").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut pairs = Vec::new();
        for member in &node.members {
            if let Ok(member_node) = ast.member(*member) {
                pairs.push((component_name(&member_node.name), member_node.name.clone()));
            }
        }
        no_folded_collision(&pairs, &name)?;
        let mut components = Vec::new();
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
            let spelled = component_name(&member_node.name);
            components.push(ada83::Component {
                doc: idl_note(&spelled, &member_node.name),
                name: Ident::new(&spelled),
                type_: ada83_member_path(ast, target, &member_node.name)?,
            });
        }
        out.push(ada83::Decl::Type(ada83::TypeDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An exception, as the record its members make. Not an Ada",
                    "`exception`: RM83 11.1 gives an exception no components at all,",
                    "and RM83 11.3 admits only a bare `raise` of one's name, no",
                    "message string travels with it either. Raising is the RPC arc's.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            def: ada83::TypeDef::Record(ada83::Record::plain(components)),
        }));
    }

    Ok(out)
}

/// The Ada 83 declarations, in a package specification that `with`s
/// `Zd_Wire83` and `System` unconditionally.
///
/// Unlike `adapter::ada::ada_file`, this never has to decide which
/// context clauses to write: every scalar this column names is either
/// `Standard`'s, `Zd_Wire83`'s or `System.Address` (an IDL `native`
/// declaration's), and a spec that happens to need neither `Zd_Wire83`
/// nor `System` carries an unused `with`, which GNAT's default warning
/// set does not fail on.
///
/// # Errors
/// As [`ada83_items`].
pub fn ada83_file(ast: &Ast, applied: &Applied) -> Result<ada83::File, MaterialiseError> {
    let decls = ada83_items(ast, applied)?;
    Ok(ada83::File {
        doc: Doc::lines(&[
            "Generated by the ZeroDDS Ada 83 column. Do not edit.",
            "",
            "Strict Ada 83 (-gnat83): no Interfaces, no modular type, no",
            "Ada.Strings.Unbounded, no tagged type. See the module documentation",
            "of zerodds_lang_cst::ada83 and zerodds_lang_rules::adapter::ada83.",
        ]),
        // Both unconditionally, for the same reason `Zd_Wire83` alone
        // is: an unused `with` is a warning GNAT's default set does
        // not fail on, and computing which of the two a given file
        // needs is exactly the `context_clause`/`operator_types`
        // machinery `adapter::ada` needs and this column does not.
        withs: vec![Path::single("Zd_Wire83"), Path::single("System")],
        package: Ident::new(PACKAGE),
        decls,
    })
}

/// The types in an order every declaration can be written in, and the
/// types a cycle among them was cut at.
///
/// Ported unchanged from `adapter::ada::ordered_types`: a back edge
/// found while walking depth-first names a cycle; the first aggregate
/// on it (RM83 3.10.1 admits an incomplete type only as an access
/// type's designated subtype, so only an aggregate — a record — can be
/// split into a declaration and a later completion) is cut, and the
/// walk runs again with that type's edges satisfied by its incomplete
/// declaration instead of followed. At most `ast.types().count()`
/// cycles exist, so that many retries either finish clean or prove one
/// is not cuttable at an aggregate at all — the shape
/// `crates/lang-rules/tests/ada83_end_to_end.rs`'s
/// `a_recursive_type_is_refused_because_this_column_has_not_cut_a_cycle_yet`
/// no longer names, now that the cut runs: it is `recursive-array`'s
/// shape, cut at `Node` like any other, and then refused in the
/// `Array` arm of [`declare_type`] because an array is not an access
/// type — see the module documentation for where that refusal is
/// meant to be measured instead.
fn ordered_types(ast: &Ast) -> Result<Plan, MaterialiseError> {
    let mut cut: Vec<Id<TypeNode>> = Vec::new();
    let passes = ast.types().count() + 1;
    for _ in 0..passes {
        let mut walk = Walk {
            order: Vec::new(),
            seen: Vec::new(),
            stack: Vec::new(),
            found: None,
        };
        for (id, _) in ast.types() {
            visit(ast, id, &cut, &mut walk, 0);
            if walk.found.is_some() {
                break;
            }
        }
        match walk.found {
            None => {
                return Ok(Plan {
                    order: walk.order,
                    cut,
                });
            }
            Some(at) if !cut.contains(&at) => cut.push(at),
            Some(_) => break,
        }
    }
    Err(MaterialiseError::NoLanguageForm {
        construct: "a cycle in the type graph that cannot be cut at an aggregate — RM83 \
                    3.10.1 admits an incomplete type only as the designated subtype of \
                    an access type"
            .to_string(),
        language: LANGUAGE,
        at: "the generated package".to_string(),
    })
}

/// A declaration order, and the types a cycle in it was cut at.
struct Plan {
    /// The types, in the order their declarations have to stand in.
    order: Vec<Id<TypeNode>>,
    /// The types declared incomplete first and completed later.
    cut: Vec<Id<TypeNode>>,
}

/// The state of one pass of the ordering walk.
struct Walk {
    /// The order this pass reached, if it found no new cycle.
    order: Vec<Id<TypeNode>>,
    /// Every type this pass has already placed, so a diamond in the
    /// graph is not walked twice.
    seen: Vec<Id<TypeNode>>,
    /// The types on the path from the walk's root to here — a back
    /// edge into this is a cycle.
    stack: Vec<Id<TypeNode>>,
    /// The type to cut, once a cycle this pass cannot already satisfy
    /// is found.
    found: Option<Id<TypeNode>>,
}

/// zerodds-lint: recursion-depth 64 (aggregate edges of the type graph;
/// bounded by `MAX_DEPTH` and by `walk.seen`, which admits each type
/// once). A back edge is detected against `walk.stack` and answered
/// with a cut point rather than followed, so a cycle ends the walk.
fn visit(ast: &Ast, id: Id<TypeNode>, cut: &[Id<TypeNode>], walk: &mut Walk, depth: usize) {
    if walk.found.is_some() {
        return;
    }
    if let Some(position) = walk.stack.iter().position(|other| *other == id) {
        // A back edge. The type to declare incomplete is the first
        // aggregate on the cycle, because that is the only kind an
        // incomplete declaration can name and complete.
        walk.found = Some(cut_point(ast, &walk.stack[position..]).unwrap_or(id));
        return;
    }
    if depth > MAX_DEPTH || walk.seen.contains(&id) {
        return;
    }
    walk.seen.push(id);
    walk.stack.push(id);
    for dependency in dependencies(ast, id) {
        // An edge into a type already cut is satisfied by its
        // incomplete declaration, so the walk does not follow it.
        if cut.contains(&dependency) {
            continue;
        }
        visit(ast, dependency, cut, walk, depth + 1);
        if walk.found.is_some() {
            break;
        }
    }
    walk.stack.pop();
    if walk.found.is_none() {
        walk.order.push(id);
    }
}

/// The type on a cycle whose declaration can be split in two.
fn cut_point(ast: &Ast, cycle: &[Id<TypeNode>]) -> Option<Id<TypeNode>> {
    cycle.iter().copied().find(|id| is_aggregate(ast, *id))
}

/// Whether a type is one an incomplete declaration can name: a record
/// or a discriminated record. A sequence or a map is not on the list —
/// this column already writes each as two declarations, and the cut
/// has to land where one name is declared twice.
fn is_aggregate(ast: &Ast, id: Id<TypeNode>) -> bool {
    matches!(
        ast.type_(id).map(|node| &node.kind),
        Ok(TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. })
    )
}

/// The types a type's Ada 83 declaration has to be preceded by.
/// Identical to `adapter::ada`'s `dependencies`.
fn dependencies(ast: &Ast, id: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut out = Vec::new();
    let Ok(node) = ast.type_(id) else {
        return out;
    };
    match &node.kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            out.extend(element.resolved());
        }
        TypeKind::Map { key, value, .. } => {
            out.extend(key.resolved());
            out.extend(value.resolved());
        }
        TypeKind::Alias { aliased } => out.extend(aliased.resolved()),
        TypeKind::ValueBox { boxed } => out.extend(boxed.resolved()),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            // The base is held as the member `parent`, so it is a dependency of its own.
            out.extend(super::c::struct_base(ast, id));
            for member in ast.all_members_of(id) {
                if let Ok(member_node) = ast.member(member) {
                    out.extend(member_node.type_.resolved());
                }
            }
        }
        TypeKind::Union { discriminator, .. } => {
            out.extend(discriminator.resolved());
            for case in ast.cases_of(id) {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        out.extend(member.type_.resolved());
                    }
                }
            }
        }
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
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

/// One declared type, recording that it was written.
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<ada83::Decl>,
    emitted: &mut Vec<Declared>,
    cycle: &mut Cycle,
) -> Result<(), MaterialiseError> {
    let before = out.len() + cycle.deferred.len();
    declare_type(ast, applied, id, out, emitted, cycle)?;
    if out.len() + cycle.deferred.len() > before && !cycle.completed.contains(&id) {
        cycle.completed.push(id);
    }
    Ok(())
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare_type(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<ada83::Decl>,
    emitted: &mut Vec<Declared>,
    cycle: &mut Cycle,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive and the two string shapes are written where the
        // member is: `Long_Integer` and `Zd_Wire83.Text` are names the
        // predefined environment and the runtime already declare.
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. } => {}

        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: ada83_type_name(ast, id),
                });
            };
            let access = ada83_type_name(ast, id);
            if already(emitted, &access) {
                return Ok(());
            }
            let array = format!("{access}{ARRAY_SUFFIX}");
            remember(emitted, &array, &array)?;
            remember(emitted, &access, &access)?;
            let elements = ada83::Decl::Type(ada83::TypeDecl {
                doc: Doc::lines(&[
                    "The elements of a sequence. `Natural range <>` leaves the length",
                    "out of the type, which is where the wire form keeps it (RM83 3.6).",
                    "A component's subtype must be definite (RM83 3.8), so this type is",
                    "reached through the access type below.",
                ]),
                name: Ident::new(&array),
                def: ada83::TypeDef::Array {
                    index: vec![ada83::Index::Unconstrained(Path::single("Natural"))],
                    element: ada83_member_path(ast, element, &access)?,
                },
            });
            // An element on a cycle is not declared yet, and RM83
            // 3.10.1 will not have an incomplete type as an array
            // element. The array type behind the sequence is therefore
            // itself declared incomplete, the access type designates
            // it — which RM83 3.10.1(9) does admit — and the array's
            // own declaration is the completion, after the element's.
            match pending(ast, element, cycle) {
                Some(target) => {
                    predeclare(ast, target, out, cycle);
                    out.push(incomplete(
                        &array,
                        "The elements of a sequence whose element type is not declared \
                         yet. Completed below, after the element.",
                    ));
                    cycle.deferred.push(elements);
                }
                None => out.push(elements),
            }
            out.push(ada83::Decl::Type(ada83::TypeDecl {
                doc: Doc::line(
                    "A sequence. `null` is an empty one that was never allocated; the \
                     storage is the caller's.",
                ),
                name: Ident::new(&access),
                def: ada83::TypeDef::Access(Path::single(&array)),
            }));
        }

        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: ada83_type_name(ast, id),
                });
            };
            // An IDL array of a type that reaches back to this one is a
            // value of unbounded size: RM83 3.10.1 admits an incomplete
            // type only as an access type's designated subtype, never
            // as an array element, and this is also IDL 4.2
            // §7.4.1.4.4.4.4's own refusal — an array's element count
            // is fixed and non-zero, so nothing about a value can stop
            // the descent, in any language. The measured verdict for
            // this shape is `wireprobe-zerodds-ada83-gen`'s, reached
            // through the shared layout planner before this
            // materialiser runs at all; this is the fallback for a
            // caller — this crate's own tests included — that reaches
            // the materialiser directly.
            if let Some(target) = pending(ast, element, cycle) {
                let name = ada83_type_name(ast, target);
                return Err(MaterialiseError::NoLanguageForm {
                    construct: format!(
                        "an array whose element `{name}` reaches the array itself — IDL \
                         4.2 §7.4.1.4.4.4.4 gives an array a fixed, non-zero element \
                         count, so nothing about a value can terminate the descent, and \
                         RM83 3.10.1 does not admit an incomplete type as an array \
                         element either"
                    ),
                    language: LANGUAGE,
                    at: ada83_type_name(ast, id),
                });
            }
            let tag = type_tag(ast, element);
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(ast.eval_integral(*dim).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: ada83_type_name(ast, id),
                    }
                })?);
            }
            let mut current = ada83_member_path(ast, element, &tag)?;
            for index in (0..lengths.len()).rev() {
                let name = array_type_name(&tag, &lengths[index..]);
                if !already(emitted, &name) {
                    remember(emitted, &name, &name)?;
                    out.push(ada83::Decl::Type(ada83::TypeDecl {
                        doc: Doc::line(
                            "One dimension of an array. The element count is in the type \
                             and not on the wire.",
                        ),
                        name: Ident::new(&name),
                        def: ada83::TypeDef::Array {
                            index: vec![ada83::Index::Range {
                                low: ada83::Expr::int(0),
                                high: ada83::Expr::int(lengths[index] - 1),
                            }],
                            element: current,
                        },
                    }));
                }
                current = Path::single(&name);
            }
        }

        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: ada83_type_name(ast, id),
                });
            };
            let access = ada83_type_name(ast, id);
            if already(emitted, &access) {
                return Ok(());
            }
            let entry = ada83_map_entry_name(ast, id);
            let array = format!("{access}{ARRAY_SUFFIX}");
            remember(emitted, &entry, &entry)?;
            remember(emitted, &array, &array)?;
            remember(emitted, &access, &access)?;
            let pair = ada83::Decl::Type(ada83::TypeDecl {
                doc: Doc::line(
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, with no framing between the two halves.",
                ),
                name: Ident::new(&entry),
                def: ada83::TypeDef::Record(ada83::Record::plain(vec![
                    ada83::Component {
                        doc: Doc::none(),
                        name: Ident::new("Key"),
                        type_: ada83_member_path(ast, key, &entry)?,
                    },
                    ada83::Component {
                        doc: Doc::none(),
                        name: Ident::new(VALUE),
                        type_: ada83_member_path(ast, value, &entry)?,
                    },
                ])),
            });
            let entries = ada83::Decl::Type(ada83::TypeDecl {
                doc: Doc::line("The entries of a map, in the order the wire carries them."),
                name: Ident::new(&array),
                def: ada83::TypeDef::Array {
                    index: vec![ada83::Index::Unconstrained(Path::single("Natural"))],
                    element: Path::single(&entry),
                },
            });
            // A key or a value on a cycle is not declared yet, and an
            // entry record may not have an incomplete component (RM83
            // 3.10.1). Both the entry and the array of entries are
            // therefore declared incomplete here and completed after
            // the key and the value; the access type, which is what
            // the member holds, is complete straight away.
            let waiting: Vec<Id<TypeNode>> = [key, value]
                .into_iter()
                .filter_map(|target| pending(ast, target, cycle))
                .collect();
            if waiting.is_empty() {
                out.push(pair);
                out.push(entries);
            } else {
                for target in waiting {
                    predeclare(ast, target, out, cycle);
                }
                out.push(incomplete(
                    &entry,
                    "One entry of a map whose key or value is not declared yet. \
                     Completed below.",
                ));
                out.push(incomplete(
                    &array,
                    "The entries of a map whose entry record is not complete yet. \
                     Completed below.",
                ));
                cycle.deferred.push(pair);
                cycle.deferred.push(entries);
            }
            out.push(ada83::Decl::Type(ada83::TypeDecl {
                doc: Doc::line("A map, held as the ordered sequence of pairs it is on the wire."),
                name: Ident::new(&access),
                def: ada83::TypeDef::Access(Path::single(&array)),
            }));
        }

        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "Ada83Subtype").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = ada83_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let decl = ada83::Decl::Subtype(ada83::SubtypeDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                mark: ada83_member_path(ast, target, &node.name)?,
            });
            // A subtype mark may not name an incomplete type (RM83
            // 3.10.1), and a subtype has no incomplete form of its own
            // to be cut at — but nothing forces it to stand at the
            // source's line either: `typedef Node NodeRef;` ahead of
            // `struct Node`'s completion is held back and declared
            // once the target is complete, the same deferral the array
            // and map instantiations on a cycle take
            // (`recursive-typedef-sequence` is the corpus case this
            // answers).
            if pending(ast, target, cycle).is_some() {
                cycle.deferred.push(decl);
            } else {
                out.push(decl);
            }
        }

        // No enumeration type, no representation clause: an enumerator
        // is a named `Long_Integer` constant, holding its wire value.
        // See the module documentation.
        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "Ada83Enum").is_none() {
                return Ok(());
            }
            let name = ada83_type_name(ast, id);
            let mut pairs = Vec::new();
            let mut values = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "Ada83EnumConst").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let spelled = component_name(&enum_node.name);
                pairs.push((spelled.clone(), enum_node.name.clone()));
                values.push((spelled, value));
            }
            if values.is_empty() {
                return Err(MaterialiseError::NoLanguageForm {
                    construct: format!(
                        "`{name}`, an enumeration with no enumerators — nothing to \
                         declare a constant for"
                    ),
                    language: LANGUAGE,
                    at: name,
                });
            }
            no_folded_collision(&pairs, &name)?;
            for (spelled, value) in values {
                remember(emitted, &spelled, &spelled)?;
                out.push(ada83::Decl::Object(ada83::ObjectDecl {
                    doc: Doc::lines(&[
                        "An enumerator of an IDL enumeration this column gives no type",
                        "of its own: see the module documentation.",
                    ]),
                    name: Ident::new(&spelled),
                    type_: ada83::Primitive::LongInteger.path(),
                    value: ada83::Expr::int(value),
                }));
            }
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "Ada83Bitmask").is_none() {
                return Ok(());
            }
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "Ada83BitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let bound = ast.effective_bit_bound(id);
                if position >= bound {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: format!(
                            "`{}`, a bit at position {position} of a bitmask bounded at \
                             {bound} — the constant has no value in the holder",
                            bit_node.name
                        ),
                        language: LANGUAGE,
                        at: bit_node.name.clone(),
                    });
                }
                let prefix = module_prefix(ast, id);
                let spelled = format!("{prefix}{}", component_name(&bit_node.name));
                remember(emitted, &spelled, &bit_node.name)?;
                out.push(ada83::Decl::Object(ada83::ObjectDecl {
                    doc: idl_note(&spelled, &bit_node.name),
                    name: Ident::new(&spelled),
                    type_: ada83::Primitive::LongInteger.path(),
                    // Bit 63 as the pattern: Table 33 admits a bound of
                    // 64 and `@position(63)` with it, and 2**63 is one
                    // past `Long_Integer'Last`. The constant is the
                    // negative number whose single bit that is, which is
                    // the same carriage the value it masks travels in.
                    value: ada83::Expr::int(ada83_pattern(1_i128 << u32::from(position))),
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "Ada83Bitset").is_none() {
                return Ok(());
            }
            let name = ada83_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                pairs.push((component_name(field_ident), field_ident.clone()));
            }
            no_folded_collision(&pairs, &name)?;
            let mut components = Vec::new();
            let mut index = 0;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                if field_node.name.is_none() {
                    continue;
                }
                components.push(ada83::Component {
                    doc: Doc::none(),
                    name: Ident::new(&pairs[index].0),
                    type_: ada83::Primitive::LongInteger.path(),
                });
                index += 1;
            }
            out.push(ada83::Decl::Type(ada83::TypeDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate Long_Integers here and not a",
                        "packed representation: the order on the wire is the",
                        "declaration's, and the packing into the anchor is the codec's,",
                        "in runtime/ada-83's arithmetic: Ada 83 has no Interfaces and",
                        "no bitwise operator on an integer type.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada83::TypeDef::Record(ada83::Record::plain(components)),
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "Ada83Value"
            } else {
                "Ada83Record"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = ada83_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`, as in
            // `adapter::ada`; the wire still carries the base's members first (§7.4.1.1.5.1).
            let members = ast.members_of(id).to_vec();
            let base = super::c::struct_base(ast, id);
            let mut pairs = Vec::new();
            if base.is_some() {
                pairs.push((component_name("parent"), "parent".to_string()));
            }
            for member in &members {
                if let Ok(member_node) = ast.member(*member) {
                    pairs.push((component_name(&member_node.name), member_node.name.clone()));
                }
            }
            no_folded_collision(&pairs, &name)?;
            let mut components = Vec::new();
            if let Some(base) = base {
                components.push(ada83::Component {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    name: Ident::new(&component_name("parent")),
                    type_: ada83_member_path(ast, base, "parent")?,
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
                let external = member_node
                    .facets
                    .xtypes
                    .as_ref()
                    .is_some_and(|xtypes| xtypes.is_external);
                let optional = member_node.facets.is_optional();
                let mut type_ = ada83_member_path(ast, target, &member_node.name)?;
                let mut doc = Doc::none();
                match pending(ast, target, cycle) {
                    // A member whose type reaches this one back. The
                    // record cannot hold it by value at all — the value
                    // would have no bound — and RM83 3.10.1 will not
                    // have an incomplete type as a component. An access
                    // type is the one shape the clause admits, and it is
                    // also what `@external` says the member is: held by
                    // reference (DDS-XTypes 1.3 Table 21). `null` is the
                    // absent state an `@optional` member needs, so the
                    // two annotations reach one declaration here.
                    Some(waiting) if optional || external => {
                        type_ = Path::single(&reference_type(ast, waiting, out, emitted, cycle)?);
                        doc = Doc::lines(&[
                            "Held by reference, because the type reaches this one back and",
                            "an Ada record cannot contain itself. `null` is the absent",
                            "state of an @optional member (DDS-XTypes 1.3 7.2.2.4.4.4.7)",
                            "and the indirection is what @external asks for (Table 21); RM83",
                            "3.10.1 admits an incomplete type only as the designated",
                            "subtype of an access type, so this is also the only spelling",
                            "available here.",
                        ]);
                    }
                    Some(waiting) => {
                        return Err(MaterialiseError::NoLanguageForm {
                            construct: format!(
                                "a member of `{}`, which reaches `{name}` back with no \
                                 @optional, no @external and no collection between them — \
                                 the value would have no bound, and RM83 3.10.1 does not \
                                 admit an incomplete type as a record component",
                                ada83_type_name(ast, waiting)
                            ),
                            language: LANGUAGE,
                            at: member_node.name.clone(),
                        });
                    }
                    None if optional => {
                        type_ = Path::single(&optional_type(
                            ast,
                            target,
                            &member_node.name,
                            out,
                            emitted,
                        )?);
                        doc = Doc::lines(&[
                            "Optional. `Present` is False when the member is absent:",
                            "DDS-XTypes 1.3 7.2.2.4.4.4.7. In a @mutable type an absent",
                            "member is not written at all; outside one its absence is",
                            "written.",
                        ]);
                    }
                    None => {}
                }
                let spelled = component_name(&member_node.name);
                components.push(ada83::Component {
                    doc: with_idl_note(doc, &spelled, &member_node.name),
                    name: Ident::new(&spelled),
                    type_,
                });
            }
            // `is null record;` is RM83 3.8, unchanged; only a *sample*
            // aggregate for it has no spelling here — see the module
            // documentation.
            out.push(ada83::Decl::Type(ada83::TypeDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                def: ada83::TypeDef::Record(ada83::Record::plain(components)),
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "Ada83Union").is_none() {
                return Ok(());
            }
            let name = ada83_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            out.push(union_decl(
                ast, applied, id, disc, &name, &node.name, cycle,
            )?);
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "Ada83Native").is_none() {
                return Ok(());
            }
            let name = ada83_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone())
                .filter(|spelling| is_ada_name(spelling));
            out.push(ada83::Decl::Type(ada83::TypeDecl {
                doc: with_idl_note(
                    Doc::line(
                        "A native type. `System.Address` unless a facet named a mapping \
                         that is an Ada subtype mark. RM83 13.7 already declares System.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada83::TypeDef::Derived(match mapping {
                    Some(spelling) => Path::new(&spelling.split('.').collect::<Vec<&str>>()),
                    None => ada83::Primitive::Address.path(),
                }),
            }));
        }

        TypeKind::ValueBox { boxed } => {
            // Refused and not skipped. `Ok(())` here declared nothing
            // for a boxed value whose target does not resolve and said
            // so nowhere, so `valuetype VB Missing;` came out of this
            // materialiser as an empty package — while every other
            // unresolved reference in this function (a sequence's
            // element, an array's, a map's key, a typedef's target, a
            // union's discriminator, a member's type) is
            // `Unevaluated`.
            let Some(target) = boxed.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "boxed value target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = ada83_type_name(ast, id);
            if already(emitted, &name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            // An access type is the one place RM83 3.10.1(9) does admit
            // an incomplete designated subtype, so a boxed value on a
            // cycle needs only the name declared ahead of it.
            if let Some(waiting) = pending(ast, target, cycle) {
                predeclare(ast, waiting, out, cycle);
            }
            out.push(ada83::Decl::Type(ada83::TypeDecl {
                doc: with_idl_note(
                    Doc::line("A boxed value: an access type, `null` when absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada83::TypeDef::Access(ada83_member_path(ast, target, &name)?),
            }));
        }

        // An interface never reaches this arm: `ada83_items` refuses
        // one before the type walk begins. An object reference belongs
        // to the RPC arc, an annotation type has no runtime form, and
        // `any`/`ValueBase` are decided at run time.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The record an `@optional` member is held in, declared if it is new.
fn optional_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
    out: &mut Vec<ada83::Decl>,
    emitted: &mut Vec<Declared>,
) -> Result<String, MaterialiseError> {
    let name = format!("{OPT_PREFIX}{}", type_tag(ast, target));
    if already(emitted, &name) {
        return Ok(name);
    }
    remember(emitted, &name, &name)?;
    out.push(ada83::Decl::Type(ada83::TypeDecl {
        doc: Doc::lines(&[
            "An optional value (DDS-XTypes 1.3 7.2.2.4.4.4.7). The absent state has",
            "no component at all, so reading `Value` when `Present` is False raises",
            "Constraint_Error (RM83 3.8.1) rather than returning whatever was there.",
        ]),
        name: Ident::new(&name),
        def: ada83::TypeDef::Record(ada83::Record {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            discriminants: vec![ada83::Discriminant {
                name: Ident::new(PRESENT),
                type_: ada83::Primitive::Boolean.path(),
                default: Some(ada83::Expr::Literal(Literal::Bool(false))),
            }],
            components: Vec::new(),
            variant: Some(ada83::Variant {
                discriminant: Ident::new(PRESENT),
                arms: vec![
                    ada83::Arm {
                        choices: vec![ada83::Choice::Value(ada83::Expr::Literal(Literal::Bool(
                            true,
                        )))],
                        components: vec![ada83::Component {
                            doc: Doc::none(),
                            name: Ident::new(VALUE),
                            type_: ada83_member_path(ast, target, at)?,
                        }],
                    },
                    ada83::Arm {
                        choices: vec![ada83::Choice::Value(ada83::Expr::Literal(Literal::Bool(
                            false,
                        )))],
                        components: Vec::new(),
                    },
                ],
            }),
        }),
    }));
    Ok(name)
}

/// The discriminated record an IDL union becomes (RM83 3.7.3).
#[allow(clippy::too_many_arguments)]
fn union_decl(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    disc: Id<TypeNode>,
    name: &str,
    declared: &str,
    cycle: &Cycle,
) -> Result<ada83::Decl, MaterialiseError> {
    let underlying = strip_aliases(ast, disc);
    let discrete = matches!(
        ast.type_(underlying).map(|node| &node.kind),
        Ok(TypeKind::Enumeration { .. })
            | Ok(TypeKind::Primitive(
                PrimitiveKind::Boolean
                    | PrimitiveKind::Char
                    | PrimitiveKind::WChar
                    | PrimitiveKind::Octet
                    | PrimitiveKind::Int8
                    | PrimitiveKind::UInt8
                    | PrimitiveKind::Int16
                    | PrimitiveKind::UInt16
                    | PrimitiveKind::Int32
                    | PrimitiveKind::UInt32
                    | PrimitiveKind::Int64
                    | PrimitiveKind::UInt64
            ))
    );
    if !discrete {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!(
                "`{name}`, a union whose discriminator is not a discrete type — RM83 \
                 3.7 admits only a discrete or an access type as a discriminant"
            ),
            language: LANGUAGE,
            at: name.to_string(),
        });
    }

    let cases = ast.cases_of(id);
    let mut pairs = vec![(DISCRIMINANT.to_string(), "<discriminator>".to_string())];
    for case in cases {
        if let Ok(case_node) = ast.case(*case) {
            if let Ok(member) = ast.member(case_node.member) {
                pairs.push((component_name(&member.name), member.name.clone()));
            }
        }
    }
    no_folded_collision(&pairs, name)?;

    let mut arms: Vec<ada83::Arm> = Vec::new();
    let mut default: Option<ada83::Arm> = None;
    let mut covered: Vec<i128> = Vec::new();
    for case in cases {
        let Ok(case_node) = ast.case(*case) else {
            return Err(MaterialiseError::NoNode {
                kind: "case",
                at: crate::adapter::cst::handle_of(*case),
            });
        };
        if ghost_created_for(applied, case_node.node_id, "Ada83Case").is_none() {
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
        // A branch whose type reaches this union back. IDL 4.2 gives a
        // union case no `@optional`, so there is no annotation to turn
        // the component into an access type as a struct member's would
        // be, and RM83 3.10.1 does not admit an incomplete type as a
        // component of a variant part either — a branch of `Node`
        // itself would have no bound. A branch of `sequence<Node>` (or
        // a map, `recursive-union-branch`'s and `recursive-through-map`'s
        // shape) is unaffected: the sequence's own access type is
        // never itself put on the cut (`is_aggregate` names only
        // records and unions), so `pending` answers `None` for it here.
        if let Some(waiting) = pending(ast, target, cycle) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a union branch of `{}`, which reaches `{name}` back with no \
                     collection between them — the value would have no bound",
                    ada83_type_name(ast, waiting)
                ),
                language: LANGUAGE,
                at: member.name.clone(),
            });
        }
        let spelled = component_name(&member.name);
        let component = ada83::Component {
            doc: idl_note(&spelled, &member.name),
            name: Ident::new(&spelled),
            type_: ada83_member_path(ast, target, &member.name)?,
        };
        let mut choices = Vec::new();
        let mut is_default = false;
        for label in &case_node.labels {
            match label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => {
                    let value =
                        ast.eval_integral(*expr)
                            .map_err(|_| MaterialiseError::Unevaluated {
                                construct: "union case label".to_string(),
                                at: member.name.clone(),
                            })?;
                    if covered.contains(&value) {
                        return Err(MaterialiseError::NoLanguageForm {
                            construct: format!(
                                "`{name}`, a union with two branches selected by {value} \
                                 — RM83 3.7.3 requires the choices of a variant part to \
                                 be distinct"
                            ),
                            language: LANGUAGE,
                            at: member.name.clone(),
                        });
                    }
                    covered.push(value);
                    choices.push(ada83::Choice::Value(choice_expr(ast, underlying, value)));
                }
            }
        }
        if is_default {
            default = Some(ada83::Arm {
                choices: vec![ada83::Choice::Others],
                components: vec![component],
            });
        } else if choices.is_empty() {
            continue;
        } else {
            arms.push(ada83::Arm {
                choices,
                components: vec![component],
            });
        }
    }
    if let Some(arm) = default {
        arms.push(arm);
    } else if !covers_everything(ast, underlying, &covered) {
        arms.push(ada83::Arm {
            choices: vec![ada83::Choice::Others],
            components: Vec::new(),
        });
    }

    Ok(ada83::Decl::Type(ada83::TypeDecl {
        doc: with_idl_note(
            Doc::lines(&[
                "A union, as the discriminated record of RM83 3.7.3. `D` is part of",
                "the value and not a convention beside it, so reading a branch the",
                "discriminant does not select raises Constraint_Error rather than",
                "returning whatever was there. Only the selected branch is written",
                "to the wire (DDS-XTypes 1.3 7.4.1.1.5.2).",
            ]),
            name,
            declared,
        ),
        name: Ident::new(name),
        def: ada83::TypeDef::Record(ada83::Record {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            discriminants: vec![ada83::Discriminant {
                name: Ident::new(DISCRIMINANT),
                type_: ada83_member_path(ast, disc, name)?,
                default: Some(discriminant_default(ast, underlying)),
            }],
            components: Vec::new(),
            variant: Some(ada83::Variant {
                discriminant: Ident::new(DISCRIMINANT),
                arms,
            }),
        }),
    }))
}

/// A 64-bit discriminant value as the `Long_Integer` whose bits it is.
///
/// The same fold `emit::ada83::pattern_literal` makes, and for the same
/// reason: a `union switch (unsigned long long)` may be labelled with a
/// value at or above 2**63, and this column's discriminant is
/// `Long_Integer`, which is signed. Written as the plain decimal GNAT
/// answers "choice given in variant part is not static" — RM83 3.7.3
/// requires a static choice and RM83 4.9(34) makes an expression that
/// raises `Constraint_Error` non-static.
pub fn ada83_pattern(value: i128) -> i128 {
    /// 2**64, the whole width, as the fold subtracts it.
    const WIDTH: i128 = 1 << 64;
    if value > i128::from(i64::MAX) && value <= i128::from(u64::MAX) {
        return value - WIDTH;
    }
    value
}

/// The Ada 83 choice a case label's value is written as.
///
/// An enumeration discriminant's choice is now the enumerator's
/// **value**, not a literal name: this column has no enumeration type
/// to name a literal of. See the module documentation.
fn choice_expr(ast: &Ast, underlying: Id<TypeNode>, value: i128) -> ada83::Expr {
    match ast.type_(underlying).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            ada83::Expr::Literal(Literal::Bool(value != 0))
        }
        _ => ada83::Expr::int(ada83_pattern(value)),
    }
}

/// Whether the labels already cover every value of the **discriminant's
/// Ada subtype** — which is not the same question as whether they cover
/// every value of the IDL discriminator.
///
/// `Boolean` is the one discriminator this column keeps as its own
/// type, so two labels exhaust it. An enumeration does not: this column
/// declares no enumeration type at all (see the module documentation),
/// its enumerators are named `Long_Integer` constants and the
/// discriminant is a `Long_Integer`. RM83 3.7.3 requires the choices of
/// a variant part to cover every value of the discriminant's subtype,
/// so covering RED, GREEN and BLUE leaves GNAT reporting "missing case
/// values: -16#8000_0000_0000_0000# .. -1" and "3 ..
/// 16#7FFF_FFFF_FFFF_FFFF#". An `others` arm is what closes it, and
/// answering `false` here is what adds one.
fn covers_everything(ast: &Ast, underlying: Id<TypeNode>, covered: &[i128]) -> bool {
    match ast.type_(underlying).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            covered.contains(&0) && covered.contains(&1)
        }
        _ => false,
    }
}

/// The default a union's discriminant takes: the first enumerator's
/// *value* (this column names no literal for it), `False`, or zero.
fn discriminant_default(ast: &Ast, underlying: Id<TypeNode>) -> ada83::Expr {
    match ast.type_(underlying).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { enumerators }) => {
            for enumerator in &enumerators {
                if let Some(value) = ast.effective_enumerator_value(*enumerator) {
                    return ada83::Expr::int(ada83_pattern(value));
                }
            }
            ada83::Expr::int(0)
        }
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            ada83::Expr::Literal(Literal::Bool(false))
        }
        _ => ada83::Expr::int(0),
    }
}

/// The Ada 83 spelling of a value's type, refusing what Ada cannot hold.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn ada83_member_path(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<Path, MaterialiseError> {
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
    ada83_value_path(ast, target)
}

/// The Ada 83 subtype mark a value of `id` is held under.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a collection's element does
/// not resolve.
pub fn ada83_value_path(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    value_path_at(ast, id, 0)
}

/// [`ada83_value_path`], carrying how many transparent typedefs the walk
/// has already stepped through.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the chain nests past
/// `MAX_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (unnamed-typedef chains; bounded by
/// `MAX_DEPTH`, which returns `MaterialiseError::Unevaluated`)
fn value_path_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<Path, MaterialiseError> {
    if depth > MAX_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("typedef chain, which nests past {MAX_DEPTH}"),
            at: type_name_at(ast, id, 0),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(ada83::Primitive::Address.path());
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => ada83_primitive(*kind).path(),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ada83::Primitive::Text.path(),
        // Neither declares a type here — an enumerator and a bitmask
        // bit are both named `Long_Integer` constants, per the module
        // documentation — so a *use* of either as a value's type, a
        // union's discriminant included, has to name the holder
        // directly rather than `ada83_type_name`'s flattened name,
        // which nothing declares a type under.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => {
            ada83::Primitive::LongInteger.path()
        }
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_path_at(ast, target, depth + 1)?,
            None => ada83::Primitive::Address.path(),
        },
        _ => Path::single(&type_name_at(ast, id, depth + 1)),
    })
}

/// The Ada 83 type an IDL primitive is held in.
///
/// Exhaustive, so a primitive added to the model without an Ada 83
/// spelling is a compile error and not a member that silently takes
/// another's. `long double` is `Zd_Wire83.Text`, as `string` is: the
/// decimal text, never an implementation-defined float — the same
/// answer `adapter::ada` gives, for the same reason, and neither column
/// writes an octet for it: `runtime/ada-83`'s `Write_F128` refuses by
/// name, the same gap `runtime/ada` leaves for the f128 arc.
#[must_use]
pub const fn ada83_primitive(kind: PrimitiveKind) -> ada83::Primitive {
    match kind {
        PrimitiveKind::Boolean => ada83::Primitive::Boolean,
        PrimitiveKind::Float32 => ada83::Primitive::Float,
        PrimitiveKind::Float64 => ada83::Primitive::LongFloat,
        PrimitiveKind::Float128 => ada83::Primitive::Text,
        // Every integer-holding width, signed or unsigned, narrow or
        // wide, `char` and `wchar` included: see the module
        // documentation for why `Long_Integer` is the one answer.
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Char
        | PrimitiveKind::Int8
        | PrimitiveKind::WChar
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => ada83::Primitive::LongInteger,
    }
}

/// `@verbatim` in the Ada 83 column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a record's component list; an
/// enumeration, a derived type and an array take them adjacent. The file
/// slots are the caller's, as in the Ada column.
impl verbatim::Carrier for ada83::Decl {
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
        let ada83::TypeDef::Record(record) = &mut node.def else {
            return false;
        };
        if node.name.as_str() != name {
            return false;
        }
        record.body_text.begin.append(begin);
        record.body_text.end.append(end);
        true
    }
}

/// A declared type's Ada 83 identifier — the scoped name with the
/// module separators flattened onto it, exactly as `adapter::ada`'s.
#[must_use]
pub fn ada83_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    type_name_at(ast, id, 0)
}

/// [`ada83_type_name`], carrying how many connector steps the walk has
/// already taken.
///
/// The depth is threaded through [`type_tag_at`] and [`map_entry_name_at`]
/// and back rather than started over at any of the three doors: they
/// reach each other, so a counter reset at one would leave the group
/// with no bound at all.
///
/// zerodds-lint: recursion-depth 64 (sequence, array, map and
/// unnamed-typedef nesting; bounded by `MAX_DEPTH`, which names the type
/// by its arena slot rather than descending). The slot and not a fixed
/// word: two types that both nest past the bound would otherwise take
/// one Ada 83 identifier, and the second declaration would stand for
/// both.
fn type_name_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("Zd_Deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return "Zd_Unnamed".to_string();
    };
    match &node.kind {
        TypeKind::Primitive(kind) => ada83_primitive(*kind).as_str().to_string(),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            ada83::Primitive::Text.as_str().to_string()
        }
        TypeKind::Sequence { element, .. } => match element.resolved() {
            Some(element) => format!("{SEQ_PREFIX}{}", type_tag_at(ast, element, depth + 1)),
            None => format!("{SEQ_PREFIX}Unknown"),
        },
        TypeKind::Array { element, dims } => match element.resolved() {
            Some(element) => {
                let mut lengths = Vec::new();
                for dim in dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => lengths.push(length),
                        Err(_) => {
                            return format!(
                                "{ARR_PREFIX}{}_X",
                                type_tag_at(ast, element, depth + 1)
                            );
                        }
                    }
                }
                array_type_name(&type_tag_at(ast, element, depth + 1), &lengths)
            }
            None => format!("{ARR_PREFIX}Unknown"),
        },
        TypeKind::Map { key, value, .. } => match (key.resolved(), value.resolved()) {
            (Some(key), Some(value)) => format!(
                "{MAP_PREFIX}{}_{}",
                type_tag_at(ast, key, depth + 1),
                type_tag_at(ast, value, depth + 1)
            ),
            _ => format!("{MAP_PREFIX}Unknown"),
        },
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => type_name_at(ast, target, depth + 1),
            None => "Zd_Unnamed".to_string(),
        },
        _ => {
            let Ok(any) = ast.any_of_type(id) else {
                return "Zd_Unnamed".to_string();
            };
            match ast.scoped_name_of(any) {
                Some(scoped) => flatten(scoped.segments()),
                None => "Zd_Unnamed".to_string(),
            }
        }
    }
}

/// The array type declared for one run of dimensions.
fn array_type_name(tag: &str, lengths: &[i128]) -> String {
    let mut name = format!("{ARR_PREFIX}{tag}");
    for length in lengths {
        name.push('_');
        name.push_str(&length.to_string());
    }
    name
}

/// The entry record a `map<K,V>` needs a name for.
#[must_use]
pub fn ada83_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
    map_entry_name_at(ast, id, 0)
}

/// [`ada83_map_entry_name`], carrying the depth of the walk that reached
/// it.
///
/// zerodds-lint: recursion-depth 64 (key and value tags, via
/// `type_tag_at`; bounded by `MAX_DEPTH`, which names the type by its
/// arena slot rather than descending)
fn map_entry_name_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("{MAP_ENTRY_PREFIX}Deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return format!("{MAP_ENTRY_PREFIX}Unknown");
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return format!("{MAP_ENTRY_PREFIX}Unknown");
    };
    let key = key
        .resolved()
        .map_or_else(|| "Unknown".to_string(), |k| type_tag_at(ast, k, depth + 1));
    let value = value
        .resolved()
        .map_or_else(|| "Unknown".to_string(), |v| type_tag_at(ast, v, depth + 1));
    format!("{MAP_ENTRY_PREFIX}{key}_{value}")
}

/// The bound a `string`, a `sequence` or a `map` carries, as the tail
/// of its tag.
///
/// The tag keys the codec routine the generated program hoists a
/// collection into ([`crate::emit::ada83::Ada83::type_name`]), so a tag
/// that leaves the bound out gives `sequence<long, 4>` and
/// `sequence<long>` one routine and the first of the two decides what
/// the other's bound is: declared the other way round, the bounded
/// member is read by a routine with no length check at all. The
/// unevaluable case is `_X`, exactly as the `Array` arm below already
/// writes it for a dimension that does not fold.
fn bound_tag(ast: &Ast, bound: Bound) -> String {
    match bound.expr() {
        None => String::new(),
        Some(expr) => match ast.eval_integral(expr) {
            Ok(length) => format!("_{length}"),
            Err(_) => "_X".to_string(),
        },
    }
}

/// The name component an invented type takes from an element type.
#[must_use]
pub fn type_tag(ast: &Ast, id: Id<TypeNode>) -> String {
    type_tag_at(ast, id, 0)
}

/// [`type_tag`], carrying how many connector steps the walk has already
/// taken.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and map nesting,
/// via `type_name_at` and `map_entry_name_at`; bounded by `MAX_DEPTH`,
/// which names the type by its arena slot rather than descending)
fn type_tag_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("Deep_{}", id.slot());
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => ada83::ada_case(&kind.as_str().replace(' ', "_")),
        // Both widths are held in `Zd_Wire83.Text` — see the module
        // documentation — but they are *not* one wire form: a `string`
        // is UTF-8 with the terminating NUL counted in its length and a
        // `wstring` is UTF-16 code units with the NUL nowhere
        // (DDS-XTypes 1.3 7.4.1.1.2). The tag has to tell them apart or
        // the hoisted codec routine of whichever came first writes both.
        Ok(TypeKind::String { width, bound }) => format!(
            "{}{}",
            match width {
                CharWidth::Char => "String",
                CharWidth::WChar => "Wstring",
            },
            bound_tag(ast, bound)
        ),
        Ok(TypeKind::Fixed { digits, scale }) => format!("Fixed_{digits}_{scale}"),
        Ok(TypeKind::Sequence { element, bound }) => match element.resolved() {
            Some(element) => format!(
                "Seq_Of{}_{}",
                bound_tag(ast, bound),
                type_tag_at(ast, element, depth + 1)
            ),
            None => "Seq_Of_Unknown".to_string(),
        },
        Ok(TypeKind::Array { element, dims }) => match element.resolved() {
            Some(element) => {
                let mut name = format!("Arr_Of_{}", type_tag_at(ast, element, depth + 1));
                for dim in &dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => name.push_str(&format!("_{length}")),
                        Err(_) => name.push_str("_X"),
                    }
                }
                name
            }
            None => "Arr_Of_Unknown".to_string(),
        },
        // The entry record's own name carries no bound — two maps that
        // differ only in bound hold the same pairs — but the tag does,
        // for the routine-name reason in [`bound_tag`].
        Ok(TypeKind::Map { bound, .. }) => {
            {
                // The collection's bound goes in front of the entry name:
                // after it, a bound on the map and one on its value would
                // end in the same suffix.
                let entry = map_entry_name_at(ast, id, depth + 1);
                match bound_tag(ast, bound) {
                    bound if bound.is_empty() => entry,
                    bound => format!("Map{bound}_{entry}"),
                }
            }
        }
        _ => type_name_at(ast, id, depth + 1),
    }
}

/// The type an alias chain ends at.
fn strip_aliases(ast: &Ast, id: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// The flattened module path a package-level constant carries.
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

/// The Ada 83 name a declaration outside the type arena takes.
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
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

/// Join scoped segments with `_`, escaping and casing each.
fn flatten(segments: &[String]) -> String {
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

fn flatten_str(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| component_name(segment))
        .collect::<Vec<String>>()
        .join("_")
}

/// The Ada 83 spelling of a member, an enumerator, a parameter or a
/// bit — escaped, repaired, then cased, exactly as `adapter::ada`'s
/// [`component_name`](crate::adapter::ada::component_name) is, and for
/// the identical reason: the escape has to run before the casing folds
/// a reserved word's case away.
#[must_use]
pub fn component_name(name: &str) -> String {
    ada83::ada_case(&ada83::legal_name(&ada83::escape_name(name)))
}

/// Whether a spelling from a facet is an Ada subtype mark.
fn is_ada_name(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.split('.').all(|segment| {
        !segment.is_empty()
            && segment.starts_with(|ch: char| ch.is_ascii_alphabetic())
            && segment
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !segment.ends_with('_')
            && !segment.contains("__")
    })
}

/// The declaration's own IDL name, where the Ada spelling lost it.
fn idl_note(spelled: &str, declared: &str) -> Doc {
    if !moved(spelled, declared) {
        return Doc::none();
    }
    Doc::line(&format!("IDL: `{declared}`."))
}

/// The same, appended to a documentation comment the caller wrote.
fn with_idl_note(mut doc: Doc, spelled: &str, declared: &str) -> Doc {
    if moved(spelled, declared) {
        doc.push(&format!("IDL: `{declared}`."));
    }
    doc
}

/// Whether the Ada spelling is a name the IDL one does not read back
/// from.
fn moved(spelled: &str, declared: &str) -> bool {
    ada83::fold(spelled) != ada83::fold(declared)
}

/// Whether a name has already been declared in the package.
fn already(emitted: &[Declared], spelled: &str) -> bool {
    let key = ada83::fold(spelled);
    emitted.iter().any(|other| other.key == key)
}

/// Record a declared name, refusing a second declaration that reaches
/// the same Ada identifier.
///
/// Unlike `adapter::ada`'s `remember`, nothing here is overloadable:
/// this column declares no enumeration literal, so there is no
/// homograph RM 8.3 would otherwise admit.
fn remember(
    emitted: &mut Vec<Declared>,
    spelled: &str,
    declared: &str,
) -> Result<(), MaterialiseError> {
    let key = ada83::fold(spelled);
    if let Some(first) = emitted.iter().find(|other| other.key == key) {
        return Err(MaterialiseError::SpellingCollision {
            first: first.declared.clone(),
            second: declared.to_string(),
            spelled: spelled.to_string(),
            language: LANGUAGE,
            at: "the generated package".to_string(),
        });
    }
    emitted.push(Declared {
        key,
        declared: declared.to_string(),
    });
    Ok(())
}

/// The names one scope declares, checked for a collision after folding.
fn no_folded_collision(pairs: &[(String, String)], at: &str) -> Result<(), MaterialiseError> {
    let mut seen: Vec<(String, &str)> = Vec::new();
    for (spelled, declared) in pairs {
        let key = ada83::fold(spelled);
        if let Some((_, first)) = seen.iter().find(|(other, _)| *other == key) {
            return Err(MaterialiseError::SpellingCollision {
                first: (*first).to_string(),
                second: declared.clone(),
                spelled: spelled.clone(),
                language: LANGUAGE,
                at: at.to_string(),
            });
        }
        seen.push((key, declared));
    }
    Ok(())
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

/// The Ada 83 expression a constant's value takes.
///
/// A `string` constant is `Zd_Wire83.Text_Of ("…")` and not a literal —
/// `Text` is an access type, and Ada admits no literal for one — and
/// `long double` is refused by name, as it is in `adapter::ada`, for
/// the identical reason: CORBA gives a `long double` literal no
/// spelling that survives a 64-bit float.
fn ada83_constant(
    value: &zerodds_ast::ConstValue,
    type_: &Path,
    at: &str,
) -> Result<ada83::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_text = type_ == &ada83::Primitive::Text.path();
    let text = |value: &str| -> Result<ada83::Expr, MaterialiseError> {
        if !value.chars().all(is_graphic_ascii) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "a string constant holding a character an Ada string literal \
                            cannot carry — RM83 2.6 admits only graphic characters, and \
                            GNAT reads a source file as Latin-1 unless told otherwise"
                    .to_string(),
                language: LANGUAGE,
                at: at.to_string(),
            });
        }
        Ok(ada83::Expr::Call {
            name: Path::new(&["Zd_Wire83", "Text_Of"]),
            args: vec![ada83::Expr::Literal(Literal::Str(value.to_string()))],
        })
    };
    Ok(match value {
        ConstValue::Boolean(flag) => ada83::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => ada83::Expr::int(*number),
        ConstValue::Char(character) => ada83::Expr::int(i128::from(u32::from(*character))),
        ConstValue::WChar(point) => ada83::Expr::int(i128::from(*point)),
        ConstValue::Float { mantissa, exponent } => {
            if is_text {
                return Err(refuse("a `long double` constant, which has no Ada literal"));
            }
            ada83::Expr::Literal(Literal::Float(ada83_float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            text(&crate::adapter::constant::fixed_decimal(digits, *scale))?
        }
        ConstValue::String(value) => text(value)?,
        ConstValue::WString(points) => text(
            &crate::adapter::constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )?,
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}

/// Whether a character is one an Ada string literal can carry.
fn is_graphic_ascii(ch: char) -> bool {
    ('\u{20}'..='\u{7e}').contains(&ch)
}

/// A floating-point literal Ada 83 reads as a real number (RM83 2.4.1
/// makes a pointless numeric literal an integer one, unchanged since).
fn ada83_float_text(mantissa: &str, exponent: Option<&str>) -> String {
    let text = crate::adapter::constant::float_text(mantissa, exponent);
    if text.contains('.') {
        return text;
    }
    match text.split_once(['e', 'E']) {
        Some((head, tail)) => format!("{head}.0e{tail}"),
        None => format!("{text}.0"),
    }
}
