// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Initialisation, destruction and copy of the C aggregates that hold
//! `@external` members. DDS-XTypes 1.3 §7.5.1.2.3.1.
//!
//! An external member of C "shall be represented using pointers", and
//! then:
//!
//! - "The constructor/initializer of the enclosing object shall set the
//!   external member pointers to NULL."
//! - "The destructor of the enclosing object shall delete the objects
//!   referenced by non-NULL external member pointers. It is the
//!   responsibility of the application to set the external member
//!   pointers to NULL before destroying the enclosing object if they do
//!   not want to delete specific referenced objects."
//! - "The copy function of the enclosing object shall do a deep copy of
//!   the external members. If the destination external member is NULL it
//!   shall be allocated. If the destination external member is not NULL
//!   it shall be filled with a copy of the source member […] If the
//!   (recursive call to the) copy operation of the external member fails,
//!   then the copy function of the containing object shall fail as well."
//! - "There may be an additional copy function that takes in arguments
//!   which allow the user to control the behavior of the copy operation":
//!   shallow or deep, and for a destination that is not NULL the action of
//!   Table 45.
//!
//! The binding writes four functions for each aggregate that holds an
//! external member, directly or in an aggregate it holds inline:
//!
//! | function | |
//! |---|---|
//! | `zd_init_<T>(T *v)` | the external and optional pointers are NULL; an optional member is absent |
//! | `zd_fini_<T>(T *v)` | `free` of what a non-NULL external pointer references, and the pointer is NULL afterwards |
//! | `zd_copy_<T>(T *dst, const T *src)` | the deep copy with the default action, `ZD_COPY_REUSE` |
//! | `zd_copy_with_<T>(dst, src, depth, action)` | the copy of Table 45 |
//!
//! `T` is the C name of the aggregate. A destination is an object
//! `zd_init_<T>` has initialised, as any `*_init` and `*_copy` pair
//! requires: the copy reads its pointers to know what to reuse or
//! release. The copy functions answer `false`
//! where an allocation fails; an object that was only partly copied is
//! still one `zd_fini_<T>` can release, because every pointer of it is
//! either NULL or owned.
//!
//! ## What the pointers own
//!
//! `malloc` made them, and `free` unmakes them: an application that
//! points an external member at storage of its own sets the pointer to
//! NULL before `zd_fini_<T>` (the clause says so). A pointer a generated
//! reader made points into the arena the reader was given
//! (`zdw_reader_alloc`), which `free` must not see: such a sample is not
//! passed to `zd_fini_<T>`, or its pointers are NULLed first.
//!
//! A `string`, a `wstring` and a `fixed` are `const char *` already (the
//! clause leaves them as they are), and the sequence, the map and the
//! `zd_arr_*` array types keep the storage they have: none of those
//! pointers is an external member, so none of these functions frees or
//! copies what they reference. An element of a sequence is its own
//! object, and the application calls the functions of its type.
//!
//! ## The union
//!
//! The live branch of a union is the one `_d` selects. `zd_init_<T>` of a
//! union clears the storage of the branches (`memset`), because nothing
//! says yet which branch will be live; the other three read `_d`. A copy
//! whose destination holds another branch than the source releases the
//! destination's branch first (unless the action is `ZD_COPY_REPLACE`,
//! which never releases).
//!
//! ## The rule for the includes
//!
//! `<stdlib.h>`, `<string.h>` and `<stdbool.h>` come with the first
//! function, behind an include guard together with the two enumerations,
//! so that two generated headers can share a translation unit. A model
//! without an external member gets none of this.

use zerodds_ast::{Ast, CaseLabel, MemberNode, TypeKind};
use zerodds_ast_core::Id;
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident, Literal};

use crate::adapter::c::{
    c_type_name, c_value_type, pointer_annotated, pointer_member, value_is_pointer,
};
use crate::adapter::cst::MaterialiseError;

/// How far an alias or an array element is followed.
const MAX_DEPTH: usize = 64;

/// The text that opens the lifecycle section: the includes the functions
/// need and the two enumerations of Table 45.
const PRELUDE: &str = "\
#ifndef ZD_LIFECYCLE_DEFINED
#define ZD_LIFECYCLE_DEFINED
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
/* XTypes 1.3 Table 45: a copy is shallow or deep. */
typedef enum zd_copy_depth {
    ZD_COPY_DEEP = 0,
    ZD_COPY_SHALLOW = 1
} zd_copy_depth;
/* XTypes 1.3 Table 45: what a copy does with a destination member that is
 * not NULL. REUSE is the default of a deep copy. */
typedef enum zd_copy_action {
    ZD_COPY_REUSE = 0,
    ZD_COPY_REPLACE = 1,
    ZD_COPY_RELEASE = 2
} zd_copy_action;
#endif";

/// How a member takes part.
#[derive(Clone)]
enum Kind {
    /// `@external` or `@optional`: a pointer to a value of this type.
    External(Id<zerodds_ast::TypeNode>),
    /// `@external` or `@optional` and a pointer already (`string`,
    /// `wstring`, `fixed`): NULL after the initialiser, assigned by the
    /// copy, never released — the bytes are the caller's.
    Borrowed(Id<zerodds_ast::TypeNode>),
    /// An aggregate (or an array of them) that holds external members,
    /// held inline.
    Inline(Id<zerodds_ast::TypeNode>),
    /// Anything else: assigned by the copy, nothing to initialise or
    /// release.
    Plain(Id<zerodds_ast::TypeNode>),
}

/// One member of a structure, or one branch of a union.
struct Slot {
    field: String,
    kind: Kind,
    /// For a branch: the labels that select it, `None` for `default:`.
    labels: Vec<Option<i128>>,
}

/// The type an alias chain ends at.
fn resolved(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Id<zerodds_ast::TypeNode> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| &node.kind) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => current = next,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// An array's dimensions, outermost first, and its element type — through
/// aliases and through an element that is itself an array.
fn array_shape(
    ast: &Ast,
    id: Id<zerodds_ast::TypeNode>,
) -> Option<(Vec<i128>, Id<zerodds_ast::TypeNode>)> {
    let mut dims = Vec::new();
    let mut current = resolved(ast, id);
    let mut found = false;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            break;
        };
        let TypeKind::Array {
            element,
            dims: lengths,
        } = &node.kind
        else {
            break;
        };
        found = true;
        for length in lengths {
            dims.push(ast.eval_integral(*length).ok()?);
        }
        current = resolved(ast, element.resolved()?);
    }
    found.then_some((dims, current))
}

/// Whether an aggregate holds an external member, directly or in an
/// aggregate it holds inline. The aggregates held inline are walked from a
/// worklist, each once.
fn needs(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> bool {
    let mut seen: Vec<Id<zerodds_ast::TypeNode>> = Vec::new();
    let mut pending = vec![id];
    while let Some(id) = pending.pop() {
        let id = resolved(ast, id);
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let members: Vec<&MemberNode> = match ast.type_(id).map(|node| node.kind.clone()) {
            // The base is a member `parent` the structure holds inline
            // (XTypes 1.3 §7.5.1.4.1.3), so what the base needs the
            // structure needs; the members are its own.
            Ok(TypeKind::Structure { .. } | TypeKind::Valuetype { .. }) => {
                if let Some(base) = super::c::struct_base(ast, id) {
                    pending.push(base);
                }
                ast.members_of(id)
                    .iter()
                    .filter_map(|member| ast.member(*member).ok())
                    .collect()
            }
            Ok(TypeKind::Union { .. }) => ast
                .cases_of(id)
                .iter()
                .filter_map(|case| ast.case(*case).ok())
                .filter_map(|case| ast.member(case.member).ok())
                .collect(),
            _ => continue,
        };
        for member in members {
            let Some(target) = member.type_.resolved() else {
                continue;
            };
            let annotated = pointer_annotated(member);
            // An `@external` or `@optional` member is a pointer (§7.5.1.2.3.1,
            // §7.5.1.2.4.1), a `string` one included, and the initialiser
            // is what makes it NULL.
            if annotated {
                return true;
            }
            // A member held behind a pointer for another reason (a cycle) is
            // the application's or the arena's, and is not walked.
            if pointer_member(ast, id, target, annotated) {
                continue;
            }
            pending.push(array_shape(ast, target).map_or(target, |(_, element)| element));
        }
    }
    false
}

fn slot(ast: &Ast, enclosing: Id<zerodds_ast::TypeNode>, member: &MemberNode) -> Option<Slot> {
    let target = member.type_.resolved()?;
    let annotated = pointer_annotated(member);
    let kind = if annotated && !value_is_pointer(ast, target) {
        Kind::External(target)
    } else if annotated {
        Kind::Borrowed(target)
    } else if pointer_member(ast, enclosing, target, annotated) {
        Kind::Plain(target)
    } else {
        let element = array_shape(ast, target).map_or(target, |(_, element)| element);
        if needs(ast, element) {
            Kind::Inline(target)
        } else {
            Kind::Plain(target)
        }
    };
    Some(Slot {
        field: c::escape_keyword(&member.name),
        kind,
        labels: Vec::new(),
    })
}

fn slots(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Vec<Slot> {
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Structure { .. } | TypeKind::Valuetype { .. }) => {
            let mut out = Vec::new();
            // The member `parent`: the base, held inline.
            if let Some(base) = super::c::struct_base(ast, id) {
                out.push(Slot {
                    field: super::c::PARENT.to_string(),
                    kind: if needs(ast, base) {
                        Kind::Inline(base)
                    } else {
                        Kind::Plain(base)
                    },
                    labels: Vec::new(),
                });
            }
            out.extend(
                ast.members_of(id)
                    .iter()
                    .filter_map(|member| ast.member(*member).ok())
                    .filter_map(|member| slot(ast, id, member)),
            );
            out
        }
        Ok(TypeKind::Union { .. }) => {
            let mut out = Vec::new();
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(mut slot) = slot(ast, id, member) else {
                    continue;
                };
                for label in &case_node.labels {
                    match label {
                        CaseLabel::Default => slot.labels.push(None),
                        CaseLabel::Value(expr) => {
                            if let Ok(value) = ast.eval_integral(*expr) {
                                slot.labels.push(Some(value));
                            }
                        }
                    }
                }
                out.push(slot);
            }
            out
        }
        _ => Vec::new(),
    }
}

// -- expression and statement helpers --------------------------------------

fn name(text: &str) -> c::Expr {
    c::Expr::Name(Ident::new(text))
}

fn int(value: i128) -> c::Expr {
    c::Expr::Literal(Literal::int(value))
}

fn arrow(value: c::Expr, field: &str) -> c::Expr {
    c::Expr::Arrow {
        value: Box::new(value),
        name: Ident::new(field),
    }
}

fn dot(value: c::Expr, field: &str) -> c::Expr {
    c::Expr::Member {
        value: Box::new(value),
        name: Ident::new(field),
    }
}

fn addr(value: c::Expr) -> c::Expr {
    c::Expr::AddressOf(Box::new(value))
}

fn deref(value: c::Expr) -> c::Expr {
    c::Expr::Paren(Box::new(c::Expr::Deref(Box::new(value))))
}

fn call(function: &str, args: Vec<c::Expr>) -> c::Expr {
    c::Expr::Call {
        callee: Box::new(name(function)),
        args,
    }
}

fn binary(op: &'static str, lhs: c::Expr, rhs: c::Expr) -> c::Expr {
    c::Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn assign(place: c::Expr, value: c::Expr) -> c::Statement {
    c::Statement::Expr(binary("=", place, value))
}

fn eq(lhs: c::Expr, rhs: c::Expr) -> c::Expr {
    binary("==", lhs, rhs)
}

fn ne(lhs: c::Expr, rhs: c::Expr) -> c::Expr {
    binary("!=", lhs, rhs)
}

fn null() -> c::Expr {
    name("NULL")
}

fn and(lhs: c::Expr, rhs: c::Expr) -> c::Expr {
    binary("&&", lhs, rhs)
}

fn or(lhs: c::Expr, rhs: c::Expr) -> c::Expr {
    binary("||", lhs, rhs)
}

fn not(value: c::Expr) -> c::Expr {
    c::Expr::Unary {
        op: "!",
        value: Box::new(c::Expr::Paren(Box::new(value))),
    }
}

fn block(statements: Vec<c::Statement>) -> c::Block {
    c::Block { statements }
}

fn when(condition: c::Expr, then: Vec<c::Statement>) -> c::Statement {
    c::Statement::If {
        condition,
        then: block(then),
        otherwise: None,
    }
}

fn either(
    condition: c::Expr,
    then: Vec<c::Statement>,
    otherwise: Vec<c::Statement>,
) -> c::Statement {
    c::Statement::If {
        condition,
        then: block(then),
        otherwise: Some(block(otherwise)),
    }
}

fn fail() -> c::Statement {
    c::Statement::Return(Some(name("false")))
}

fn type_of(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Type {
    c_value_type(ast, id)
}

fn init_name(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> String {
    format!("zd_init_{}", c_type_name(ast, id))
}

fn fini_name(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> String {
    format!("zd_fini_{}", c_type_name(ast, id))
}

fn copy_name(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> String {
    format!("zd_copy_{}", c_type_name(ast, id))
}

fn copy_with_name(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> String {
    format!("zd_copy_with_{}", c_type_name(ast, id))
}

// -- one value ---------------------------------------------------------------

/// Statements that run `each` on every element of an array value `place`
/// of the dimensions `dims`, the element being the lvalue handed to it.
fn over_elements<F: Fn(c::Expr) -> Vec<c::Statement>>(
    dims: &[i128],
    place: c::Expr,
    depth: usize,
    each: &F,
) -> Vec<c::Statement> {
    let Some((first, rest)) = dims.split_first() else {
        return each(place);
    };
    let index = format!("zd_i{depth}");
    let element = c::Expr::Index {
        value: Box::new(place),
        index: Box::new(name(&index)),
    };
    vec![c::Statement::For {
        init: Box::new(c::Statement::Declare {
            type_: c::Type::Primitive(c::Primitive::Size),
            name: Ident::new(&index),
            value: Some(int(0)),
        }),
        condition: binary("<", name(&index), int(*first)),
        step: c::Expr::Unary {
            op: "++",
            value: Box::new(name(&index)),
        },
        body: block(over_elements(rest, element, depth + 1, each)),
    }]
}

/// The statements that apply `per_aggregate` to the aggregates inside the
/// value `place` of type `ty`: the value itself, or each element of an
/// array of them.
fn walk_value<F: Fn(Id<zerodds_ast::TypeNode>, c::Expr) -> Vec<c::Statement>>(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    place: c::Expr,
    depth: usize,
    per_aggregate: &F,
) -> Vec<c::Statement> {
    if let Some((dims, element)) = array_shape(ast, ty) {
        return over_elements(&dims, place, depth, &|element_place| {
            per_aggregate(resolved(ast, element), element_place)
        });
    }
    per_aggregate(resolved(ast, ty), place)
}

fn init_value(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    place: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    walk_value(ast, ty, place, depth, &|aggregate, lvalue| {
        if !needs(ast, aggregate) {
            return Vec::new();
        }
        vec![c::Statement::Expr(call(
            &init_name(ast, aggregate),
            vec![addr(lvalue)],
        ))]
    })
}

fn fini_value(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    place: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    walk_value(ast, ty, place, depth, &|aggregate, lvalue| {
        if !needs(ast, aggregate) {
            return Vec::new();
        }
        vec![c::Statement::Expr(call(
            &fini_name(ast, aggregate),
            vec![addr(lvalue)],
        ))]
    })
}

/// The copy of one value of an aggregate or an array of them: the
/// recursive copy, and the failure of it is the failure of this one.
fn copy_aggregates(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some((dims, element)) = array_shape(ast, ty) {
        let element = resolved(ast, element);
        // The same index walks both arrays, so the source is rebuilt with
        // the loop's index names, which `over_elements` derives from the
        // depth alone.
        return over_elements_pair(&dims, dst, src, depth, &|d, s| copy_one(ast, element, d, s));
    }
    copy_one(ast, resolved(ast, ty), dst, src)
}

fn copy_one(
    ast: &Ast,
    aggregate: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
) -> Vec<c::Statement> {
    vec![when(
        not(call(
            &copy_with_name(ast, aggregate),
            vec![addr(dst), addr(src), name("depth"), name("action")],
        )),
        vec![fail()],
    )]
}

fn over_elements_pair<F: Fn(c::Expr, c::Expr) -> Vec<c::Statement>>(
    dims: &[i128],
    dst: c::Expr,
    src: c::Expr,
    depth: usize,
    each: &F,
) -> Vec<c::Statement> {
    let Some((first, rest)) = dims.split_first() else {
        return each(dst, src);
    };
    let index = format!("zd_i{depth}");
    let at = |value: c::Expr| c::Expr::Index {
        value: Box::new(value),
        index: Box::new(name(&index)),
    };
    vec![c::Statement::For {
        init: Box::new(c::Statement::Declare {
            type_: c::Type::Primitive(c::Primitive::Size),
            name: Ident::new(&index),
            value: Some(int(0)),
        }),
        condition: binary("<", name(&index), int(*first)),
        step: c::Expr::Unary {
            op: "++",
            value: Box::new(name(&index)),
        },
        body: block(over_elements_pair(rest, at(dst), at(src), depth + 1, each)),
    }]
}

/// A member that is neither external nor an aggregate that holds one:
/// assigned, or `memcpy`'d if it is an array.
fn copy_plain(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
) -> c::Statement {
    if array_shape(ast, ty).is_some() {
        return c::Statement::Expr(call(
            "memcpy",
            vec![dst, src, c::Expr::SizeOf(Box::new(type_of(ast, ty)))],
        ));
    }
    assign(dst, src)
}

/// The pointer `place` to a value of type `ty`, released: what it
/// references is finalised, then freed.
fn release(ast: &Ast, ty: Id<zerodds_ast::TypeNode>, pointer: c::Expr) -> Vec<c::Statement> {
    let mut out = fini_value(ast, ty, deref(pointer.clone()), 0);
    out.push(c::Statement::Expr(call("free", vec![pointer])));
    out
}

/// The copy of one external member: Table 45.
fn copy_external(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
) -> Vec<c::Statement> {
    let held = type_of(ast, ty);
    let pointer_type = c::Type::Pointer(Box::new(held.clone()));
    let deep = {
        let mut statements = vec![
            when(
                and(
                    ne(dst.clone(), null()),
                    eq(name("action"), name("ZD_COPY_RELEASE")),
                ),
                {
                    let mut gone = release(ast, ty, dst.clone());
                    gone.push(assign(dst.clone(), null()));
                    gone
                },
            ),
            // REPLACE: the existing storage is the application's to release.
            when(
                and(
                    ne(dst.clone(), null()),
                    eq(name("action"), name("ZD_COPY_REPLACE")),
                ),
                vec![assign(dst.clone(), null())],
            ),
        ];
        let mut made = vec![
            assign(
                dst.clone(),
                c::Expr::Cast {
                    type_: Box::new(pointer_type),
                    value: Box::new(call("malloc", vec![c::Expr::SizeOf(Box::new(held))])),
                },
            ),
            when(eq(dst.clone(), null()), vec![fail()]),
        ];
        made.extend(init_value(ast, ty, deref(dst.clone()), 0));
        statements.push(when(eq(dst.clone(), null()), made));
        statements.extend(copy_value_through(
            ast,
            ty,
            deref(dst.clone()),
            deref(src.clone()),
        ));
        statements
    };
    vec![either(
        eq(src.clone(), null()),
        // An absent source: the destination becomes absent. A REPLACE
        // never releases.
        vec![
            when(
                and(
                    ne(dst.clone(), null()),
                    ne(name("action"), name("ZD_COPY_REPLACE")),
                ),
                release(ast, ty, dst.clone()),
            ),
            assign(dst.clone(), null()),
        ],
        vec![either(
            eq(name("depth"), name("ZD_COPY_SHALLOW")),
            // Table 45, both shallow rows: the destination points where the
            // source does, and what it pointed to before is released.
            vec![
                when(
                    and(ne(dst.clone(), null()), ne(dst.clone(), src.clone())),
                    release(ast, ty, dst.clone()),
                ),
                assign(dst.clone(), src.clone()),
            ],
            deep,
        )],
    )]
}

/// The copy of the value an external pointer references.
fn copy_value_through(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
) -> Vec<c::Statement> {
    let element = array_shape(ast, ty).map_or(ty, |(_, element)| element);
    if needs(ast, element) {
        copy_aggregates(ast, ty, dst, src, 0)
    } else {
        vec![copy_plain(ast, ty, dst, src)]
    }
}

// -- the four functions ------------------------------------------------------

fn function(
    doc: Doc,
    returns: c::Type,
    name: &str,
    params: Vec<c::Param>,
    body: Vec<c::Statement>,
) -> c::Decl {
    c::Decl::Function(c::Function {
        doc,
        is_static: true,
        is_inline: false,
        // A model that never calls them is not an error: these are the
        // application's.
        is_maybe_unused: true,
        returns,
        name: Ident::new(name),
        params,
        body: Some(block(body)),
    })
}

fn prototype(returns: c::Type, name: &str, params: Vec<c::Param>) -> c::Decl {
    c::Decl::Function(c::Function {
        doc: Doc::none(),
        is_static: true,
        is_inline: false,
        is_maybe_unused: true,
        returns,
        name: Ident::new(name),
        params,
        body: None,
    })
}

fn param(type_: c::Type, name: &str) -> c::Param {
    c::Param {
        type_,
        name: Some(Ident::new(name)),
    }
}

fn pointer_to(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Type {
    c::Type::Pointer(Box::new(type_of(ast, id)))
}

fn const_pointer_to(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Type {
    c::Type::Pointer(Box::new(c::Type::Const(Box::new(type_of(ast, id)))))
}

fn is_union(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> bool {
    matches!(
        ast.type_(id).map(|node| &node.kind),
        Ok(TypeKind::Union { .. })
    )
}

/// The discriminator's C type, for the cast of a label.
fn discriminator_type(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Type {
    match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Union { discriminator, .. }) => discriminator
            .resolved()
            .map_or(c::Type::Primitive(c::Primitive::Int32), |disc| {
                type_of(ast, disc)
            }),
        _ => c::Type::Primitive(c::Primitive::Int32),
    }
}

/// Whether the branch `slot` is the live one, given `_d` of `base`.
fn selects(
    ast: &Ast,
    aggregate: Id<zerodds_ast::TypeNode>,
    all: &[Slot],
    slot: &Slot,
    base: &c::Expr,
) -> c::Expr {
    let selector = arrow(base.clone(), "_d");
    let disc = discriminator_type(ast, aggregate);
    let label = |value: i128| {
        eq(
            selector.clone(),
            c::Expr::Cast {
                type_: Box::new(disc.clone()),
                value: Box::new(int(value)),
            },
        )
    };
    let mut condition: Option<c::Expr> = None;
    let mut push = |expr: c::Expr| {
        condition = Some(match condition.take() {
            Some(previous) => or(previous, expr),
            None => expr,
        });
    };
    for entry in &slot.labels {
        match entry {
            Some(value) => push(label(*value)),
            None => {
                // `default:` selects the branch when no explicit label does.
                let mut none: Option<c::Expr> = None;
                for other in all {
                    for explicit in other.labels.iter().flatten() {
                        none = Some(match none.take() {
                            Some(previous) => or(previous, label(*explicit)),
                            None => label(*explicit),
                        });
                    }
                }
                push(match none {
                    Some(any) => not(any),
                    None => int(1),
                });
            }
        }
    }
    condition.unwrap_or_else(|| int(0))
}

/// The place of the member `slot` in `base` (`v` or `dst`/`src`).
fn place(aggregate_is_union: bool, base: &c::Expr, slot: &Slot) -> c::Expr {
    if aggregate_is_union {
        dot(arrow(base.clone(), "_u"), &slot.field)
    } else {
        arrow(base.clone(), &slot.field)
    }
}

fn init_function(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Decl {
    let v = name("v");
    let mut body = Vec::new();
    if is_union(ast, id) {
        body.push(c::Statement::Expr(call(
            "memset",
            vec![
                addr(arrow(v.clone(), "_u")),
                int(0),
                c::Expr::SizeOf(Box::new(c::Type::UnionTag(Ident::new(&format!(
                    "{}_u",
                    c_type_name(ast, id)
                ))))),
            ],
        )));
    } else {
        for slot in slots(ast, id) {
            let member = place(false, &v, &slot);
            match &slot.kind {
                Kind::External(_) | Kind::Borrowed(_) => body.push(assign(member, null())),
                Kind::Inline(ty) => body.extend(init_value(ast, *ty, member, 0)),
                Kind::Plain(_) => {}
            }
        }
    }
    if body.is_empty() {
        body.push(c::Statement::Expr(c::Expr::Cast {
            type_: Box::new(c::Type::Void),
            value: Box::new(v),
        }));
    }
    function(
        Doc::lines(&[
            "Sets the external members to NULL and the optional members absent.",
            "XTypes 1.3 7.5.1.2.3.1.",
        ]),
        c::Type::Void,
        &init_name(ast, id),
        vec![param(pointer_to(ast, id), "v")],
        body,
    )
}

fn fini_function(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Decl {
    let v = name("v");
    let union = is_union(ast, id);
    let all = slots(ast, id);
    let mut body = Vec::new();
    for slot in &all {
        let member = place(union, &v, slot);
        let mut then = match &slot.kind {
            Kind::External(ty) => {
                vec![when(ne(member.clone(), null()), {
                    let mut gone = release(ast, *ty, member.clone());
                    gone.push(assign(member.clone(), null()));
                    gone
                })]
            }
            Kind::Inline(ty) => fini_value(ast, *ty, member.clone(), 0),
            Kind::Plain(_) | Kind::Borrowed(_) => continue,
        };
        if union {
            then = vec![when(selects(ast, id, &all, slot, &v), then)];
        }
        body.extend(then);
    }
    if body.is_empty() {
        body.push(c::Statement::Expr(c::Expr::Cast {
            type_: Box::new(c::Type::Void),
            value: Box::new(v),
        }));
    }
    function(
        Doc::lines(&[
            "Deletes what the non-NULL external members reference, and sets them to NULL.",
            "An application that does not want an object deleted sets its pointer to",
            "NULL first. XTypes 1.3 7.5.1.2.3.1.",
        ]),
        c::Type::Void,
        &fini_name(ast, id),
        vec![param(pointer_to(ast, id), "v")],
        body,
    )
}

fn copy_with_function(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Decl {
    let dst = name("dst");
    let src = name("src");
    let union = is_union(ast, id);
    let all = slots(ast, id);
    let mut body = Vec::new();
    if union {
        // Another live branch in the destination than in the source: the
        // destination's is released first (a REPLACE never releases), and
        // the storage starts clean.
        body.push(when(
            ne(arrow(dst.clone(), "_d"), arrow(src.clone(), "_d")),
            vec![
                when(
                    ne(name("action"), name("ZD_COPY_REPLACE")),
                    vec![c::Statement::Expr(call(
                        &fini_name(ast, id),
                        vec![dst.clone()],
                    ))],
                ),
                c::Statement::Expr(call(
                    "memset",
                    vec![
                        addr(arrow(dst.clone(), "_u")),
                        int(0),
                        c::Expr::SizeOf(Box::new(c::Type::UnionTag(Ident::new(&format!(
                            "{}_u",
                            c_type_name(ast, id)
                        ))))),
                    ],
                )),
            ],
        ));
        body.push(assign(arrow(dst.clone(), "_d"), arrow(src.clone(), "_d")));
    }
    for slot in &all {
        let dst_member = place(union, &dst, slot);
        let src_member = place(union, &src, slot);
        let mut statements = Vec::new();
        match &slot.kind {
            Kind::External(ty) => {
                statements.extend(copy_external(ast, *ty, dst_member, src_member));
            }
            Kind::Inline(ty) => {
                statements.extend(copy_aggregates(ast, *ty, dst_member, src_member, 0));
            }
            Kind::Plain(ty) | Kind::Borrowed(ty) => {
                statements.push(copy_plain(ast, *ty, dst_member, src_member));
            }
        }
        if union {
            body.push(when(selects(ast, id, &all, slot, &src), statements));
        } else {
            body.extend(statements);
        }
    }
    // `depth` and `action` are used by the members that point at an object;
    // an aggregate with only borrowed pointers (a `string`) uses neither.
    for unused in ["depth", "action"] {
        body.push(c::Statement::Expr(c::Expr::Cast {
            type_: Box::new(c::Type::Void),
            value: Box::new(name(unused)),
        }));
    }
    body.push(c::Statement::Return(Some(name("true"))));
    function(
        Doc::lines(&[
            "Copies `src` into `dst`. XTypes 1.3 7.5.1.2.3.1 and Table 45: a deep copy",
            "allocates a NULL destination member and fills a non-NULL one, as `action`",
            "says (REUSE, REPLACE, RELEASE); a shallow copy points the destination at",
            "the source's object. Answers false where an allocation fails.",
        ]),
        c::Type::Primitive(c::Primitive::Bool),
        &copy_with_name(ast, id),
        vec![
            param(pointer_to(ast, id), "dst"),
            param(const_pointer_to(ast, id), "src"),
            param(c::Type::Named(Ident::new("zd_copy_depth")), "depth"),
            param(c::Type::Named(Ident::new("zd_copy_action")), "action"),
        ],
        body,
    )
}

fn copy_function(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Decl {
    function(
        Doc::lines(&[
            "The deep copy with the default action, which reuses a non-NULL destination.",
            "XTypes 1.3 7.5.1.2.3.1.",
        ]),
        c::Type::Primitive(c::Primitive::Bool),
        &copy_name(ast, id),
        vec![
            param(pointer_to(ast, id), "dst"),
            param(const_pointer_to(ast, id), "src"),
        ],
        vec![c::Statement::Return(Some(call(
            &copy_with_name(ast, id),
            vec![
                name("dst"),
                name("src"),
                name("ZD_COPY_DEEP"),
                name("ZD_COPY_REUSE"),
            ],
        )))],
    )
}

fn prototypes(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Vec<c::Decl> {
    vec![
        prototype(
            c::Type::Void,
            &init_name(ast, id),
            vec![param(pointer_to(ast, id), "v")],
        ),
        prototype(
            c::Type::Void,
            &fini_name(ast, id),
            vec![param(pointer_to(ast, id), "v")],
        ),
        prototype(
            c::Type::Primitive(c::Primitive::Bool),
            &copy_with_name(ast, id),
            vec![
                param(pointer_to(ast, id), "dst"),
                param(const_pointer_to(ast, id), "src"),
                param(c::Type::Named(Ident::new("zd_copy_depth")), "depth"),
                param(c::Type::Named(Ident::new("zd_copy_action")), "action"),
            ],
        ),
    ]
}

/// Refuses, by name, what the walks above would otherwise step over: a
/// member or a branch the model does not hold, a type that does not
/// resolve, an array dimension or a case label that does not evaluate.
/// Run first, so that every lookup after it finds what it looks for and
/// a function is never written with a member missing from it.
fn check(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Result<(), MaterialiseError> {
    let name = c_type_name(ast, id);
    let unevaluated = |construct: &str, at: &str| MaterialiseError::Unevaluated {
        construct: construct.to_string(),
        at: format!("{name}.{at}"),
    };
    let mut members: Vec<(Id<zerodds_ast::MemberNode>, Vec<CaseLabel>)> = Vec::new();
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Structure { .. } | TypeKind::Valuetype { .. }) => {
            members.extend(ast.all_members_of(id).into_iter().map(|m| (m, Vec::new())));
        }
        Ok(TypeKind::Union { .. }) => {
            for case in ast.cases_of(id) {
                let case = ast
                    .case(*case)
                    .map_err(|_| unevaluated("a union case the model does not hold", ""))?;
                members.push((case.member, case.labels.clone()));
            }
        }
        _ => return Ok(()),
    }
    for (member, labels) in members {
        let member = ast
            .member(member)
            .map_err(|_| unevaluated("a member the model does not hold", ""))?;
        let target = member
            .type_
            .resolved()
            .ok_or_else(|| unevaluated("member type", &member.name))?;
        // Walks the array shape once; it answers `None` for a dimension
        // that does not evaluate, which `array_shape` cannot tell apart
        // from "not an array", so the dimensions are asked here.
        let mut current = resolved(ast, target);
        for _ in 0..MAX_DEPTH {
            let Ok(node) = ast.type_(current) else {
                return Err(unevaluated("a type the model does not hold", &member.name));
            };
            let TypeKind::Array { element, dims } = &node.kind else {
                break;
            };
            for dim in dims {
                ast.eval_integral(*dim)
                    .map_err(|_| unevaluated("array dimension", &member.name))?;
            }
            let Some(next) = element.resolved() else {
                return Err(unevaluated("array element type", &member.name));
            };
            current = resolved(ast, next);
        }
        for label in labels {
            if let CaseLabel::Value(expr) = label {
                ast.eval_integral(expr)
                    .map_err(|_| unevaluated("case label", &member.name))?;
            }
        }
    }
    Ok(())
}

/// The lifecycle functions of every aggregate in `order` that holds an
/// external member, behind the guarded prelude; nothing where none does.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] naming the member, branch, array
/// dimension or case label the model cannot give.
pub fn lifecycle(
    ast: &Ast,
    order: &[Id<zerodds_ast::TypeNode>],
) -> Result<Vec<c::Decl>, MaterialiseError> {
    for id in order {
        check(ast, *id)?;
    }
    let eligible: Vec<Id<zerodds_ast::TypeNode>> = order
        .iter()
        .copied()
        .filter(|id| {
            matches!(
                ast.type_(*id).map(|node| &node.kind),
                Ok(TypeKind::Structure { .. }
                    | TypeKind::Valuetype { .. }
                    | TypeKind::Union { .. })
            ) && needs(ast, *id)
        })
        .collect();
    if eligible.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = vec![c::Decl::Verbatim(PRELUDE.to_string())];
    // Prototypes first: the aggregates call each other, and one that holds
    // itself through an external member calls itself.
    for id in &eligible {
        out.extend(prototypes(ast, *id));
    }
    for id in &eligible {
        out.push(init_function(ast, *id));
        out.push(fini_function(ast, *id));
        out.push(copy_with_function(ast, *id));
        out.push(copy_function(ast, *id));
    }
    Ok(out)
}
