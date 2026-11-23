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
//! where an allocation fails or where an existing member is not large
//! enough to hold the copy (Table 45, "Deep Copy | Reuse"); an object that
//! was only partly copied is still one `zd_fini_<T>` can release, because
//! every pointer of it is either NULL or owned.
//!
//! ## Table 45
//!
//! | depth | action | a destination member that is not NULL |
//! |---|---|---|
//! | `ZD_COPY_SHALLOW` | any | released, then it points at the source's object |
//! | `ZD_COPY_DEEP` | `ZD_COPY_REUSE` | copied into; `false` if it is not large enough |
//! | `ZD_COPY_DEEP` | `ZD_COPY_REPLACE` | left to the application, a new object is allocated |
//! | `ZD_COPY_DEEP` | `ZD_COPY_RELEASE` | released, a new object is allocated |
//!
//! The table has no row "Shallow Copy | Reuse"; the shallow copy does what
//! its two rows say. A destination member that is NULL takes the source's
//! pointer (shallow) or a new object (deep). A source member that is NULL
//! makes the destination's NULL; what it pointed to is released, except by
//! a deep `ZD_COPY_REPLACE`. A destination member that already points at
//! the source's object (the state a shallow copy leaves) is the source's:
//! no copy releases it, a deep `ZD_COPY_REUSE` leaves it shared, and a deep
//! `ZD_COPY_REPLACE` or `ZD_COPY_RELEASE` allocates an object of its own.
//! A copy of an object onto itself (`dst == src`) changes nothing and
//! answers `true`, whatever the depth and the action.
//!
//! ## What the pointers own
//!
//! `malloc` made them, and `free` unmakes them: an application that
//! points an external member at storage of its own sets the pointer to
//! NULL before `zd_fini_<T>` (the clause says so). The generated decoder
//! fills a sample with pointers that are not `malloc`'s:
//!
//! - what it reads points into the arena the reader was given
//!   (`zdw_reader_alloc`): the pointee of an external member, the buffer of
//!   a collection (`_release` is false), the characters of a string;
//! - a member the stream does not carry gets the default of its type, and
//!   for a `string`, a `wstring` and a `fixed` that is a string literal
//!   (`""`, the decimal zero of the scale).
//!
//! Neither is `free`'s, and a literal cannot be written into. A decoded
//! sample is therefore not passed to `zd_fini_<T>`, and it is not the
//! destination of a copy: a deep `ZD_COPY_REUSE` writes into the
//! characters and the objects that are there, and the other rows release
//! them. It is a source for any copy. To use its storage as a destination
//! all the same, set its external pointers to NULL first.
//!
//! A `string`, a `wstring` and a `fixed` are `const char *` already, and
//! the clause leaves that mapping as it is. One that is `@external` is an
//! external member pointer like the others: NULL after the initialiser,
//! its characters freed by the destructor and copied by a deep copy
//! (`strlen` + 1 octets from `malloc`). An existing one is large enough
//! for a `ZD_COPY_REUSE` if it is at least as long as the source: that
//! many octets are known to be there, and the copy writes into them. An
//! application that assigned a literal or other storage that is not
//! writable heap storage of that length sets the member to NULL before
//! the copy, as before the destructor. One that is only `@optional` is
//! NULL after the initialiser and assigned by the copy, and its characters
//! stay the caller's; §7.5.1.2.4.1 names no owner.
//!
//! ## The collections
//!
//! A sequence or a map an external pointer references is
//! `{ _maximum, _length, _buffer, _release }`, and the object the pointer
//! references is the collection with its elements, at every depth of a
//! collection of collections. The pointer is that of an `@external`
//! member and of an `@optional` one alike (§7.5.1.2.4.1 makes the optional
//! member a pointer too, and both take the same path here). A header that
//! exists holds a valid `_maximum`, `_length` and `_release`: the copy
//! reads them, it does not overwrite the header.
//!
//! - The elements `[0, _length)` are objects; the slots behind them are
//!   storage.
//! - The destructor finalises the elements whose type has functions and
//!   frees `_buffer` where `_release` says the collection owns it.
//! - A deep copy copies the elements, each with the functions of its type;
//!   the key of a map entry is assigned. An element or a key that is a
//!   `string` is its pointer, as in every collection of this binding: the
//!   characters are the caller's.
//! - A collection the copy allocates gets a buffer of the source's length:
//!   `_maximum` is the source's `_length`, and `_release` is set.
//! - A collection that exists is reused. It is not large enough where its
//!   `_maximum` is below the source's `_length`, and the copy answers
//!   `false` before it changes anything of that collection — what
//!   `copy(map, other, autogrow = false)` of Table 47 does. A reuse
//!   finalises the elements it drops and initialises the ones it adds.
//!   What the copy reached before that collection holds the copy by then:
//!   the members in front of it, the elements in front of it in a
//!   collection or an array around it, and the key of the map entry whose
//!   value it is. The destructor releases that result.
//!
//! The entries of a map are copied here and not by `<MAP>_copy` of
//! `zerodds_map.h`: that operation assigns the entries, which shares what
//! an element with external members points at, and a map whose key the
//! clause gives no name has no operations.
//!
//! A sequence, a map and a `zd_arr_*` array an aggregate holds inline keep
//! the storage they have: such a `_buffer` is no external member, so none
//! of these functions frees or copies what it references, and the
//! application calls the functions of the element type.
//!
//! ## The union
//!
//! The live branch of a union is the one `_d` selects. `zd_init_<T>` of a
//! union sets `_d` to the default value of the discriminator type (Table
//! 9: zero, `false`, the `@default_literal` of an enumeration or else its
//! first literal) and clears the storage of the branches (`memset`), so
//! that whichever branch that value selects is NULL or zero; the other
//! three read `_d`. A copy whose destination holds another branch than the
//! source releases the destination's branch first (unless it is a deep
//! copy with `ZD_COPY_REPLACE`, which never releases).
//!
//! ## The rule for the includes
//!
//! `<stdlib.h>`, `<string.h>`, `<stdint.h>` and `<stdbool.h>` come with the first
//! function, behind an include guard together with the two enumerations,
//! so that two generated headers can share a translation unit. A model
//! without an external member gets none of this.

use zerodds_ast::{Ast, CaseLabel, MemberNode, TypeKind};
use zerodds_ast_core::Id;
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident, Literal};

use crate::adapter::annotated_external;
use crate::adapter::c::{
    c_map_entry_name, c_type_name, c_value_type, pointer_annotated, pointer_member,
    value_is_pointer,
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
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
/* Whether `n` slots of `size` octets do not fit in a `size_t`: only a
 * platform whose `size_t` is no wider than the 32-bit length can say yes. */
#if SIZE_MAX <= 0xFFFFFFFFu
#define ZD_LIFECYCLE_TOO_LARGE(n, size) ((size_t)(n) > SIZE_MAX / (size))
#else
#define ZD_LIFECYCLE_TOO_LARGE(n, size) (0)
#endif
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
    /// `@external` and a pointer already (`string`, `wstring`, `fixed`):
    /// the characters are what the pointer references, so the destructor
    /// frees them and a deep copy copies them.
    Text,
    /// `@optional` alone and a pointer already: NULL after the
    /// initialiser, assigned by the copy, never released — the bytes are
    /// the caller's.
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
    } else if annotated && annotated_external(member) {
        Kind::Text
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

/// A sequence or a map: `{ _maximum, _length, _buffer, _release }`.
struct Collection {
    /// The C type of one slot of `_buffer`: the element of a sequence, the
    /// entry of a map.
    slot: c::Type,
    /// The element of a sequence, the value of a map.
    element: Id<zerodds_ast::TypeNode>,
    /// A map: a slot is `{ key, value }`, and this is the C type of `key`.
    key: Option<c::Type>,
}

impl Collection {
    /// The slot `index` of the buffer of the collection `base`.
    fn slot_at(base: &c::Expr, index: &str) -> c::Expr {
        c::Expr::Index {
            value: Box::new(dot(base.clone(), "_buffer")),
            index: Box::new(name(index)),
        }
    }

    /// The element in that slot: the slot of a sequence, the `value` of a
    /// map's entry.
    fn element_at(&self, base: &c::Expr, index: &str) -> c::Expr {
        let slot = Self::slot_at(base, index);
        if self.key.is_some() {
            dot(slot, "value")
        } else {
            slot
        }
    }
}

/// The sequence or the map `id` is, through aliases; `None` for any other
/// type.
fn collection(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Option<Collection> {
    let id = resolved(ast, id);
    match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Sequence { element, .. }) => {
            let element = element.resolved()?;
            Some(Collection {
                slot: type_of(ast, element),
                element,
                key: None,
            })
        }
        Ok(TypeKind::Map { key, value, .. }) => {
            let (key, value) = (key.resolved()?, value.resolved()?);
            Some(Collection {
                slot: c::Type::Named(Ident::new(&c_map_entry_name(ast, id))),
                element: value,
                key: Some(type_of(ast, key)),
            })
        }
        _ => None,
    }
}

/// Whether a value an external pointer reaches holds storage these
/// functions manage: a collection (its buffer), an aggregate that holds an
/// external member, or an array of either.
fn owns(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> bool {
    let id = array_shape(ast, id).map_or_else(|| resolved(ast, id), |(_, element)| element);
    collection(ast, id).is_some() || needs(ast, id)
}

/// `for (<type> <index> = <from>; <index> < <below>; ++<index>) { body }`
fn counted(
    index: &str,
    type_: c::Primitive,
    from: c::Expr,
    below: c::Expr,
    body: Vec<c::Statement>,
) -> c::Statement {
    c::Statement::For {
        init: Box::new(c::Statement::Declare {
            type_: c::Type::Primitive(type_),
            name: Ident::new(index),
            value: Some(from),
        }),
        condition: binary("<", name(index), below),
        step: c::Expr::Unary {
            op: "++",
            value: Box::new(name(index)),
        },
        body: block(body),
    }
}

/// Statements that run `each` on every element of an array value `place`
/// of the dimensions `dims`, the element being the lvalue handed to it.
/// The loops take the index names `zd_i<depth>` onwards, one per
/// dimension.
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
    vec![counted(
        &index,
        c::Primitive::Size,
        int(0),
        int(*first),
        over_elements(rest, element, depth + 1, each),
    )]
}

/// [`over_elements`] for two arrays of one shape, walked by one index.
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
    vec![counted(
        &index,
        c::Primitive::Size,
        int(0),
        int(*first),
        over_elements_pair(rest, at(dst), at(src), depth + 1, each),
    )]
}

fn truth(value: bool) -> c::Expr {
    c::Expr::Literal(Literal::Bool(value))
}

/// The header of a collection without a buffer.
fn empty_collection(place: &c::Expr) -> Vec<c::Statement> {
    vec![
        assign(dot(place.clone(), "_maximum"), int(0)),
        assign(dot(place.clone(), "_length"), int(0)),
        assign(dot(place.clone(), "_buffer"), null()),
        assign(dot(place.clone(), "_release"), truth(false)),
    ]
}

/// The value `place` of type `ty`, initialised: an aggregate by its
/// initialiser, a collection as the empty one, an array element by
/// element. `depth` is the number of the next free loop index.
///
/// zerodds-lint: recursion-depth 64 (with [`fini_one`], [`copy_one`] and
/// [`copy_collection`]: the nesting of a collection of collections, which
/// [`check`] bounds by `MAX_DEPTH` before any of them runs).
fn init_value(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    place: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some((dims, element)) = array_shape(ast, ty) {
        if !owns(ast, element) {
            return Vec::new();
        }
        return over_elements(&dims, place, depth, &|at| init_one(ast, element, &at));
    }
    init_one(ast, resolved(ast, ty), &place)
}

fn init_one(ast: &Ast, ty: Id<zerodds_ast::TypeNode>, place: &c::Expr) -> Vec<c::Statement> {
    if collection(ast, ty).is_some() {
        return empty_collection(place);
    }
    if needs(ast, ty) {
        return vec![c::Statement::Expr(call(
            &init_name(ast, ty),
            vec![addr(place.clone())],
        ))];
    }
    Vec::new()
}

/// The value `place` of type `ty`, finalised: what it owns is released.
///
/// zerodds-lint: recursion-depth 64 (as [`init_value`]; through [`fini_one`]).
fn fini_value(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    place: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some((dims, element)) = array_shape(ast, ty) {
        if !owns(ast, element) {
            return Vec::new();
        }
        let inner = depth + dims.len();
        return over_elements(&dims, place, depth, &|at| {
            fini_one(ast, element, &at, inner)
        });
    }
    fini_one(ast, resolved(ast, ty), &place, depth)
}

/// zerodds-lint: recursion-depth 64 (as [`init_value`]).
fn fini_one(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    place: &c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some(found) = collection(ast, ty) {
        let mut out = Vec::new();
        if owns(ast, found.element) {
            let index = format!("zd_i{depth}");
            out.push(counted(
                &index,
                c::Primitive::UInt32,
                int(0),
                dot(place.clone(), "_length"),
                fini_value(
                    ast,
                    found.element,
                    found.element_at(place, &index),
                    depth + 1,
                ),
            ));
        }
        // `_release` says whose the buffer is: the arena of a generated
        // reader and an array of the application are not `free`'s.
        out.push(when(
            dot(place.clone(), "_release"),
            vec![c::Statement::Expr(call(
                "free",
                vec![dot(place.clone(), "_buffer")],
            ))],
        ));
        out.extend(empty_collection(place));
        return out;
    }
    if needs(ast, ty) {
        return vec![c::Statement::Expr(call(
            &fini_name(ast, ty),
            vec![addr(place.clone())],
        ))];
    }
    Vec::new()
}

/// The storage a value the copy has just initialised needs to take a copy
/// of `src`: the buffer of a collection, of the source's length. The copy
/// fails where `malloc` does.
fn reserve_value(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some((dims, element)) = array_shape(ast, ty) {
        if collection(ast, element).is_none() {
            return Vec::new();
        }
        return over_elements_pair(&dims, dst, src, depth, &|d, s| {
            reserve_one(ast, element, &d, &s)
        });
    }
    reserve_one(ast, resolved(ast, ty), &dst, &src)
}

fn reserve_one(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: &c::Expr,
    src: &c::Expr,
) -> Vec<c::Statement> {
    let Some(found) = collection(ast, ty) else {
        // An aggregate allocates what its members need when it is copied
        // into, and a scalar needs nothing.
        return Vec::new();
    };
    let length = dot(src.clone(), "_length");
    let size = c::Expr::SizeOf(Box::new(found.slot.clone()));
    let buffer = dot(dst.clone(), "_buffer");
    vec![when(
        binary(">", length.clone(), int(0)),
        vec![
            when(
                call("ZD_LIFECYCLE_TOO_LARGE", vec![length.clone(), size.clone()]),
                vec![fail()],
            ),
            assign(
                buffer.clone(),
                c::Expr::Cast {
                    type_: Box::new(c::Type::Pointer(Box::new(found.slot))),
                    value: Box::new(call("malloc", vec![octets(length.clone(), size)])),
                },
            ),
            when(eq(buffer, null()), vec![fail()]),
            assign(dot(dst.clone(), "_maximum"), length),
            assign(dot(dst.clone(), "_release"), truth(true)),
        ],
    )]
}

/// `(size_t)count * size`
fn octets(count: c::Expr, size: c::Expr) -> c::Expr {
    binary(
        "*",
        c::Expr::Cast {
            type_: Box::new(c::Type::Primitive(c::Primitive::Size)),
            value: Box::new(count),
        },
        size,
    )
}

/// The copy of the value `src` of type `ty` into the object `dst`: an
/// aggregate that holds external members by its copy function, a
/// collection element by element, an array of either in a loop, anything
/// else assigned (an array of it by `memcpy`). The failure of a part is
/// the failure of the whole.
fn copy_value(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: c::Expr,
    src: c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some((dims, element)) = array_shape(ast, ty) {
        if !owns(ast, element) {
            return vec![c::Statement::Expr(call(
                "memcpy",
                vec![dst, src, c::Expr::SizeOf(Box::new(type_of(ast, ty)))],
            ))];
        }
        let inner = depth + dims.len();
        // The same index walks both arrays.
        return over_elements_pair(&dims, dst, src, depth, &|d, s| {
            copy_one(ast, element, &d, &s, inner)
        });
    }
    copy_one(ast, resolved(ast, ty), &dst, &src, depth)
}

/// zerodds-lint: recursion-depth 64 (as [`init_value`]).
fn copy_one(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: &c::Expr,
    src: &c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    if let Some(found) = collection(ast, ty) {
        return copy_collection(ast, &found, dst, src, depth);
    }
    if needs(ast, ty) {
        return vec![when(
            not(call(
                &copy_with_name(ast, ty),
                vec![
                    addr(dst.clone()),
                    addr(src.clone()),
                    name("depth"),
                    name("action"),
                ],
            )),
            vec![fail()],
        )];
    }
    vec![assign(dst.clone(), src.clone())]
}

/// The copy of the collection `src` into the collection `dst`, which is
/// one that exists or one [`reserve_value`] gave a buffer.
///
/// Table 45, "Deep Copy | Reuse": "If the existing member is not large
/// enough, this operation shall fail" — the capacity is asked before
/// anything of the destination is changed.
///
/// zerodds-lint: recursion-depth 64 (as [`init_value`]).
fn copy_collection(
    ast: &Ast,
    found: &Collection,
    dst: &c::Expr,
    src: &c::Expr,
    depth: usize,
) -> Vec<c::Statement> {
    let index = format!("zd_i{depth}");
    let dst_length = dot(dst.clone(), "_length");
    let src_length = dot(src.clone(), "_length");
    let mut out = vec![when(
        binary(">", src_length.clone(), dot(dst.clone(), "_maximum")),
        vec![fail()],
    )];
    if !owns(ast, found.element) {
        // Slots that own nothing: the octets. `memmove`, because two
        // headers an application assigned to each other share a buffer.
        out.push(when(
            binary(">", src_length.clone(), int(0)),
            vec![c::Statement::Expr(call(
                "memmove",
                vec![
                    dot(dst.clone(), "_buffer"),
                    dot(src.clone(), "_buffer"),
                    octets(
                        src_length.clone(),
                        c::Expr::SizeOf(Box::new(found.slot.clone())),
                    ),
                ],
            ))],
        ));
        out.push(assign(dst_length, src_length));
        return out;
    }
    // The elements the copy drops are finalised, the ones it adds are
    // initialised, and `_length` follows each step, so that a copy that
    // fails half way leaves a collection the destructor can release.
    out.push(counted(
        &index,
        c::Primitive::UInt32,
        src_length.clone(),
        dst_length.clone(),
        fini_value(ast, found.element, found.element_at(dst, &index), depth + 1),
    ));
    out.push(when(
        binary("<", src_length.clone(), dst_length.clone()),
        vec![assign(dst_length.clone(), src_length.clone())],
    ));
    let mut added = init_value(ast, found.element, found.element_at(dst, &index), depth + 1);
    added.push(assign(
        dst_length.clone(),
        binary("+", name(&index), int(1)),
    ));
    added.extend(reserve_value(
        ast,
        found.element,
        found.element_at(dst, &index),
        found.element_at(src, &index),
        depth + 1,
    ));
    out.push(counted(
        &index,
        c::Primitive::UInt32,
        dst_length,
        src_length.clone(),
        added,
    ));
    let mut each = Vec::new();
    if let Some(key) = &found.key {
        each.push(c::Statement::Expr(call(
            "memmove",
            vec![
                addr(dot(Collection::slot_at(dst, &index), "key")),
                addr(dot(Collection::slot_at(src, &index), "key")),
                c::Expr::SizeOf(Box::new(key.clone())),
            ],
        )));
    }
    each.extend(copy_value(
        ast,
        found.element,
        found.element_at(dst, &index),
        found.element_at(src, &index),
        depth + 1,
    ));
    out.push(counted(
        &index,
        c::Primitive::UInt32,
        int(0),
        src_length,
        each,
    ));
    out
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

fn action_is(which: &str) -> c::Expr {
    eq(name("action"), name(which))
}

fn action_is_not(which: &str) -> c::Expr {
    ne(name("action"), name(which))
}

/// What the statements that allocate a NULL destination leave.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Made {
    /// An initialised object the fill then copies into.
    Empty,
    /// The copy itself.
    Filled,
}

/// The copy of one external member, given how its pointer is released,
/// what a deep copy allocates for a NULL destination (`made`) and how it
/// fills a destination that is another object than the source's
/// (`filled`): the paragraphs of the clause on a NULL destination, and
/// Table 45.
fn copy_pointer(
    dst: &c::Expr,
    src: &c::Expr,
    released: &[c::Statement],
    made: Vec<c::Statement>,
    filled: Vec<c::Statement>,
    leaves: Made,
) -> Vec<c::Statement> {
    let gone = || {
        let mut gone = released.to_vec();
        gone.push(assign(dst.clone(), null()));
        gone
    };
    let mut deep = vec![
        // The state a shallow copy leaves: the object is the source's, so
        // it is not the destination's to release, and a copy of its own
        // is what REPLACE and RELEASE allocate.
        when(
            and(eq(dst.clone(), src.clone()), action_is_not("ZD_COPY_REUSE")),
            vec![assign(dst.clone(), null())],
        ),
        when(
            and(ne(dst.clone(), null()), action_is("ZD_COPY_RELEASE")),
            gone(),
        ),
        // REPLACE: the existing storage is the application's to release.
        when(
            and(ne(dst.clone(), null()), action_is("ZD_COPY_REPLACE")),
            vec![assign(dst.clone(), null())],
        ),
    ];
    // A REUSE of the source's own object has nothing to copy.
    let fill = when(ne(dst.clone(), src.clone()), filled);
    match leaves {
        Made::Empty => {
            deep.push(when(eq(dst.clone(), null()), made));
            deep.push(fill);
        }
        Made::Filled => deep.push(either(eq(dst.clone(), null()), made, vec![fill])),
    }
    vec![either(
        eq(name("depth"), name("ZD_COPY_SHALLOW")),
        // Table 45, both shallow rows: the destination points where the
        // source does, and what it pointed to before is released.
        vec![
            when(
                and(ne(dst.clone(), null()), ne(dst.clone(), src.clone())),
                released.to_vec(),
            ),
            assign(dst.clone(), src.clone()),
        ],
        vec![either(
            eq(src.clone(), null()),
            // An absent source: the destination becomes absent. A REPLACE
            // never releases.
            vec![
                when(
                    and(ne(dst.clone(), null()), action_is_not("ZD_COPY_REPLACE")),
                    released.to_vec(),
                ),
                assign(dst.clone(), null()),
            ],
            deep,
        )],
    )]
}

/// The copy of one external member that points at a value of type `ty`.
fn copy_external(
    ast: &Ast,
    ty: Id<zerodds_ast::TypeNode>,
    dst: &c::Expr,
    src: &c::Expr,
) -> Vec<c::Statement> {
    let held = type_of(ast, ty);
    let mut made = vec![
        assign(
            dst.clone(),
            c::Expr::Cast {
                type_: Box::new(c::Type::Pointer(Box::new(held.clone()))),
                value: Box::new(call("malloc", vec![c::Expr::SizeOf(Box::new(held))])),
            },
        ),
        when(eq(dst.clone(), null()), vec![fail()]),
    ];
    made.extend(init_value(ast, ty, deref(dst.clone()), 0));
    made.extend(reserve_value(
        ast,
        ty,
        deref(dst.clone()),
        deref(src.clone()),
        0,
    ));
    copy_pointer(
        dst,
        src,
        &release(ast, ty, dst.clone()),
        made,
        copy_value(ast, ty, deref(dst.clone()), deref(src.clone()), 0),
        Made::Empty,
    )
}

/// `(char *)pointer`: the characters of a `const char *` these functions
/// own.
fn characters(pointer: c::Expr) -> c::Expr {
    c::Expr::Cast {
        type_: Box::new(c::Type::Pointer(Box::new(c::Type::Primitive(
            c::Primitive::Char,
        )))),
        value: Box::new(pointer),
    }
}

/// The characters an external `string`, `wstring` or `fixed` points at,
/// freed.
fn release_text(pointer: &c::Expr) -> Vec<c::Statement> {
    vec![c::Statement::Expr(call(
        "free",
        vec![characters(pointer.clone())],
    ))]
}

/// The copy of one external member that is a `const char *` already: the
/// characters up to and with the terminator.
fn copy_text(dst: &c::Expr, src: &c::Expr) -> Vec<c::Statement> {
    let length = |of: &c::Expr| call("strlen", vec![of.clone()]);
    let size = binary("+", length(src), int(1));
    let fresh = name("zd_text");
    let made = vec![
        c::Statement::Declare {
            type_: c::Type::Pointer(Box::new(c::Type::Primitive(c::Primitive::Char))),
            name: Ident::new("zd_text"),
            value: Some(characters(call("malloc", vec![size.clone()]))),
        },
        when(eq(fresh.clone(), null()), vec![fail()]),
        c::Statement::Expr(call(
            "memcpy",
            vec![fresh.clone(), src.clone(), size.clone()],
        )),
        assign(dst.clone(), fresh),
    ];
    // An existing string holds at least its own length and the
    // terminator, and that is all that is known of the block it is in.
    let filled = vec![
        when(binary(">", length(src), length(dst)), vec![fail()]),
        c::Statement::Expr(call(
            "memmove",
            vec![characters(dst.clone()), src.clone(), size],
        )),
    ];
    copy_pointer(dst, src, &release_text(dst), made, filled, Made::Filled)
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

/// The value a union's discriminator has by default (XTypes 1.3 Table 9):
/// zero, `false`, or the `@default_literal` of an enumeration and else its
/// first literal — the value the defaulter of the type gives `_d`.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] naming the union whose discriminator
/// does not resolve or has an enumerator that does not evaluate.
fn discriminator_default(
    ast: &Ast,
    id: Id<zerodds_ast::TypeNode>,
) -> Result<i128, MaterialiseError> {
    let refused = || MaterialiseError::Unevaluated {
        construct: "the default value of the discriminator type".to_string(),
        at: format!("{}._d", c_type_name(ast, id)),
    };
    let discriminator = match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Union { discriminator, .. }) => discriminator.resolved(),
        _ => return Err(refused()),
    };
    discriminator
        .and_then(|found| crate::default_common::discriminator_default_value(ast, found))
        .ok_or_else(refused)
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

fn init_function(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Result<c::Decl, MaterialiseError> {
    let v = name("v");
    let mut body = Vec::new();
    if is_union(ast, id) {
        // The destructor and the copy read `_d`, so it holds a value from
        // here on: Table 9's for the discriminator type.
        body.push(assign(
            arrow(v.clone(), "_d"),
            c::Expr::Cast {
                type_: Box::new(discriminator_type(ast, id)),
                value: Box::new(int(discriminator_default(ast, id)?)),
            },
        ));
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
                Kind::External(_) | Kind::Text | Kind::Borrowed(_) => {
                    body.push(assign(member, null()));
                }
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
    Ok(function(
        Doc::lines(&[
            "Sets the external members to NULL and the optional members absent; the",
            "discriminator of a union to the default value of its type.",
            "XTypes 1.3 7.5.1.2.3.1.",
        ]),
        c::Type::Void,
        &init_name(ast, id),
        vec![param(pointer_to(ast, id), "v")],
        body,
    ))
}

fn fini_function(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> c::Decl {
    let v = name("v");
    let union = is_union(ast, id);
    let all = slots(ast, id);
    let mut body = Vec::new();
    for slot in &all {
        let member = place(union, &v, slot);
        let mut then: Vec<c::Statement> = match &slot.kind {
            Kind::External(ty) => {
                vec![when(ne(member.clone(), null()), {
                    let mut gone = release(ast, *ty, member.clone());
                    gone.push(assign(member.clone(), null()));
                    gone
                })]
            }
            Kind::Text => {
                vec![when(ne(member.clone(), null()), {
                    let mut gone = release_text(&member);
                    gone.push(assign(member.clone(), null()));
                    gone
                })]
            }
            Kind::Inline(ty) => fini_value(ast, *ty, member.clone(), 0),
            // Nothing of theirs is these functions' to release.
            Kind::Plain(_) | Kind::Borrowed(_) => Vec::new(),
        };
        if then.is_empty() {
            continue;
        }
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
            "NULL first. A sample a generated decoder filled is not passed here: its",
            "pointers are the reader's arena and string literals. XTypes 1.3 7.5.1.2.3.1.",
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
    // One object for both: every destination member is the source member,
    // and a row that releases or replaces the destination's would take the
    // source's with it. Nothing is copied, as `<MAP>_copy` of
    // `zerodds_map.h` answers for `map == other`.
    let mut body = vec![when(
        eq(dst.clone(), src.clone()),
        vec![c::Statement::Return(Some(name("true")))],
    )];
    if union {
        // Another live branch in the destination than in the source: the
        // destination's is released first (a deep REPLACE never releases),
        // and the storage starts clean.
        body.push(when(
            ne(arrow(dst.clone(), "_d"), arrow(src.clone(), "_d")),
            vec![
                when(
                    or(
                        eq(name("depth"), name("ZD_COPY_SHALLOW")),
                        action_is_not("ZD_COPY_REPLACE"),
                    ),
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
                statements.extend(copy_external(ast, *ty, &dst_member, &src_member));
            }
            Kind::Text => statements.extend(copy_text(&dst_member, &src_member)),
            Kind::Inline(ty) => {
                statements.extend(copy_value(ast, *ty, dst_member, src_member, 0));
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
            "the source's object. Answers false where an allocation fails or an",
            "existing member is not large enough for a REUSE. A copy of an object",
            "onto itself changes nothing and answers true. `dst` is an object",
            "zd_init has initialised and whose members are NULL or heap storage: a",
            "sample a generated decoder filled is a source, not a destination.",
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
/// resolve, an array dimension or a case label that does not evaluate, a
/// union whose discriminator has no default value.
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
            // The initialiser writes it into `_d`.
            discriminator_default(ast, id)?;
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
        // from "not an array", so the dimensions are asked here. Behind an
        // external pointer the walk goes on through the collections: their
        // elements are copied and released with them, so each element type
        // has to resolve, and the nesting has to end. The walk looks at
        // `MAX_DEPTH` types and the last of them has to be the one that
        // ends the nesting: `MAX_DEPTH - 1` levels pass, `MAX_DEPTH` do not.
        let behind_pointer = pointer_annotated(member);
        let mut current = resolved(ast, target);
        let mut ended = false;
        for _ in 0..MAX_DEPTH {
            let Ok(node) = ast.type_(current) else {
                return Err(unevaluated("a type the model does not hold", &member.name));
            };
            let next = match &node.kind {
                TypeKind::Array { element, dims } => {
                    for dim in dims {
                        ast.eval_integral(*dim)
                            .map_err(|_| unevaluated("array dimension", &member.name))?;
                    }
                    element
                        .resolved()
                        .ok_or_else(|| unevaluated("array element type", &member.name))?
                }
                TypeKind::Sequence { element, .. } if behind_pointer => element
                    .resolved()
                    .ok_or_else(|| unevaluated("sequence element type", &member.name))?,
                TypeKind::Map { key, value, .. } if behind_pointer => {
                    key.resolved()
                        .ok_or_else(|| unevaluated("map key type", &member.name))?;
                    value
                        .resolved()
                        .ok_or_else(|| unevaluated("map value type", &member.name))?
                }
                _ => {
                    ended = true;
                    break;
                }
            };
            current = resolved(ast, next);
        }
        if !ended {
            return Err(unevaluated(
                &format!(
                    "arrays and collections nested {MAX_DEPTH} levels or deeper, \
                     which the functions do not follow"
                ),
                &member.name,
            ));
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
/// dimension, element type, case label or default value of a union's
/// discriminator the model cannot give, or the member whose arrays and
/// collections nest 64 levels or deeper.
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
        out.push(init_function(ast, *id)?);
        out.push(fini_function(ast, *id));
        out.push(copy_with_function(ast, *id));
        out.push(copy_function(ast, *id));
    }
    Ok(out)
}
