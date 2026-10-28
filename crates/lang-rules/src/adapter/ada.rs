// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::c`] and [`crate::adapter::go`].
//! Reading the three side by side is the point: all three flatten a
//! module onto the name, two of the three need a declaration order, and
//! everything else they do differently is their language's.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The six things Ada decides that Go does not
//!
//! 1. **Declaration order matters.** Ada 2012 RM 3.11 makes a
//!    declaration visible from its own end onwards, so a type named
//!    before it is declared does not compile. The walk here is a
//!    post-order over the type graph, as in the C column and for the
//!    same reason. Where C can name an incomplete `struct` through a
//!    pointer anywhere it likes, Ada's incomplete type declaration
//!    (RM 3.10.1) may only be the **designated subtype of an access
//!    type** — never the component of a record and never the element of
//!    an array — so a cycle in the type graph is cut where an access
//!    type is what stands on the other side of the cut. See
//!    [`ordered_types`] and
//!    [the section below](#a-cycle-is-cut-at-an-access-type).
//! 2. **Identifiers are case-insensitive.** RM 2.3(5/2) folds before
//!    comparing, so `Value` and `value` are one declaration twice.
//!    Every collision check in this file compares on
//!    [`ada::fold`], which is what makes the check catch a pair a
//!    case-sensitive one would emit.
//! 3. **The escape cannot be a trailing underscore.** RM 2.3(4/3)
//!    forbids an identifier ending in one, so this column appends
//!    [`ada::ESCAPE_SUFFIX`] where every other column appends `_`. It is
//!    the only column in this crate whose escape is a letter, and the
//!    reason is a lexical rule and not a preference.
//! 4. **An enumeration carries no codes.** RM 3.5.1 numbers the literals
//!    0, 1, 2 … in declaration order; the representation clause of RM
//!    13.4 is what states them. Without it an IDL `@value(4)` is gone
//!    from the generated spec with nothing to say so — see
//!    [the section below](#an-enumeration-is-two-declarations).
//! 5. **A collection needs a declared name.** Ada has no anonymous array
//!    type in a component position, so `sequence<long>` becomes two
//!    declarations of its own, exactly as in the C column.
//! 6. **A typedef is a subtype.** RM 3.2.2 against RM 3.4.
//!
//! ## The memory shapes
//!
//! | IDL | Ada |
//! |---|---|
//! | `boolean` | `Boolean` |
//! | `octet`, `uint8`, `char` | `Interfaces.Unsigned_8` |
//! | `int8` | `Interfaces.Integer_8` |
//! | `short` | `Interfaces.Integer_16` |
//! | `unsigned short`, `wchar` | `Interfaces.Unsigned_16` |
//! | `long` | `Interfaces.Integer_32` |
//! | `unsigned long` | `Interfaces.Unsigned_32` |
//! | `long long` | `Interfaces.Integer_64` |
//! | `unsigned long long` | `Interfaces.Unsigned_64` |
//! | `float` | `Interfaces.IEEE_Float_32` |
//! | `double` | `Interfaces.IEEE_Float_64` |
//! | `long double` | `Unbounded_String`, holding the decimal |
//! | `string`, `wstring` | `Ada.Strings.Unbounded.Unbounded_String` |
//! | `fixed<P,S>` | `Unbounded_String`, holding the decimal |
//! | `sequence<T>` | `access array (Natural range <>) of T` |
//! | `T[N][M]` | one array type per dimension, outermost outermost |
//! | `map<K,V>` | a sequence of a declared entry record |
//! | `union` | one record with a discriminant and a variant part |
//! | `@optional T m` | a record discriminated on `Present : Boolean` |
//! | `@optional T m`, `T` on a cycle | `access T`, `null` when absent |
//! | a type that reaches itself | `type T;` first, the type after |
//! | `enum` | an enumeration type **and** a representation clause |
//! | `bitmask` | `new Interfaces.Unsigned_N` and one constant per bit |
//! | `typedef` | `subtype` |
//! | `native` | `new System.Address` |
//! | `interface` | `type … is interface` and abstract operations |
//!
//! ### Never `Integer` and never `Float`
//!
//! RM 3.5.4 requires `Integer` to hold at least −2**15 + 1 .. 2**15 − 1
//! and says nothing more; RM 3.5.7 leaves the format of `Float` to the
//! implementation. A member declared with either would have a different
//! width under a different compiler, for a wire format that fixes every
//! width. `Interfaces` (RM B.2) declares `Integer_8/16/32/64`,
//! `Unsigned_8/16/32/64`, `IEEE_Float_32` and `IEEE_Float_64`, and every
//! numeric member in a generated spec is one of those.
//!
//! ### `string` is `Unbounded_String` and never `String`
//!
//! Ada's `String` is `array (Positive range <>) of Character` (RM A.1):
//! an unconstrained array, whose length is part of the *subtype* of a
//! value rather than of the value. A record component's subtype must be
//! definite (RM 3.8), so `Name : String;` is not a legal component at
//! all, and `Name : String (1 .. 32);` would fix the length in the type
//! for a wire form that carries it per sample.
//! `Ada.Strings.Unbounded.Unbounded_String` is a definite private type
//! whose length is part of the value, which is the shape the wire has.
//!
//! A `wstring` is held as UTF-8 in the same type, which puts the
//! DDS-XTypes 1.3 §7.4.1.1.2 surrogate rule inside the Ada runtime as a
//! second implementation of the clause rather than a copy of the Rust
//! column's answer.
//!
//! ### `long double` is text and never `Long_Long_Float`
//!
//! Ada names three predefined floating types and fixes the format of
//! none of them; `Long_Long_Float` is 80-bit x87 extended on x86-64
//! GNAT and 64-bit on targets with no wider hardware type. An IDL
//! `long double` is a binary128 with 113 bits of significand, so a
//! member declared `Long_Long_Float` would lose bits at the assignment
//! on some targets and not on others, from one generated spec. The
//! member holds the decimal text instead — as the Go and Python columns
//! do, and for the same reason.
//!
//! ### `sequence<T>` is an access to an unconstrained array
//!
//! Ada has no built-in dynamic array. Two shapes were available:
//!
//! - `Ada.Containers.Vectors`, which needs a generic instantiation per
//!   element type — a `package X_Vectors is new Ada.Containers.Vectors
//!   (Natural, X);` for each — and drags the containers hierarchy into
//!   every generated spec.
//! - `type T_Array is array (Natural range <>) of T;` beside
//!   `type T_Seq is access T_Array;`, which is two ordinary type
//!   declarations and needs nothing instantiated.
//!
//! The second is what this column writes. It compiles with the
//! predefined environment alone, and the length is `X.all'Length` rather
//! than a container call, which is what a codec reads. Either way the
//! element needs a declared name, exactly as in the C column.
//!
//! ### `map<K,V>` is a sequence of entries and never a hashed container
//!
//! DDS-XTypes 1.3 §7.4.1.1.4 defines a map on the wire as a sequence of
//! key/value pairs, which is ordered. `Ada.Containers.Hashed_Maps`
//! iterates in an order the implementation chooses, so a column holding
//! a map in one would produce a different octet stream for one value —
//! and the KeyHash of a keyed type would differ with it.
//!
//! ### `@optional` is a discriminated record
//!
//! DDS-XTypes 1.3 §7.2.2.4.4.4.7. Two shapes were available: the C
//! column's `Has_M : Boolean;` beside `M : T;`, and a record
//! discriminated on `Present : Boolean` whose `False` arm has no
//! component at all. This column writes the second. In the first, the
//! value component exists whether or not the flag says so, and reading
//! it when the flag is `False` is a defined operation returning
//! whatever was there; in the second, `Payload.Value` when
//! `Payload.Present` is `False` raises `Constraint_Error` (RM 3.8.1) —
//! the absent state is not readable as a value. The cost is one
//! declared type per element type, which this column already pays for
//! every sequence and every array.
//!
//! ### An enumeration is two declarations
//!
//! This is the one thing this column does that no other does, so it is
//! written out. An Ada enumeration type declaration carries only the
//! literals:
//!
//! ```ada
//! type Mode is (IDLE, ACTIVE, FAULT);
//! ```
//!
//! and RM 3.5.1 gives those the positions 0, 1, 2 — which are also their
//! internal codes unless something says otherwise. An IDL
//! `@value(4) ACTIVE` would therefore arrive in the generated spec as
//! the number 1, and every value written from it would be off the wire
//! numbering with nothing in the file to say it happened. The
//! representation clause of RM 13.4 is what states the codes:
//!
//! ```ada
//! for Mode use (IDLE => 0, ACTIVE => 4, FAULT => 5);
//! ```
//!
//! RM 13.4 requires the codes to be static, distinct, and to satisfy the
//! type's own ordering — that is, to ascend in declaration order. IDL
//! puts no such constraint on `@value`, so an enumeration whose values
//! descend has no Ada enumeration form and is refused by name here
//! rather than emitted with a clause the compiler rejects or, worse,
//! without one.
//!
//! ### Enumeration literals carry no module prefix
//!
//! The Go and C columns prefix every enumerator with its flattened
//! module path, because in both a constant is one name in one namespace.
//! An Ada enumeration literal is an overloadable declaration —
//! a parameterless function returning its own type (RM 3.5.1) — so two
//! enumerations in one package may both declare `IDLE` and every use
//! resolves by the type the context expects. The literals here are
//! therefore the IDL names, unprefixed. Bitmask bit constants *are*
//! prefixed: they are objects, which do not overload.
//!
//! ### Modules flatten rather than nest
//!
//! Ada packages nest, and nesting would be the better Ada form. What
//! rules it out is the interaction with the declaration order above: a
//! package's declarative part is contiguous, an IDL module may be
//! reopened (IDL 4.2 §7.4.2), and a dependency order over the whole
//! model can require a declaration from module `a`, then one from
//! module `b`, then another from `a`. Nested packages cannot write that
//! order; a flat one always can. `::common::util::Location` is
//! `Common_Util_Location`.
//!
//! ### A cycle is cut at an access type
//!
//! Five corpus cases have a type that reaches itself: through a
//! `sequence`, through a `map` value, through a union branch, through
//! two types that name each other, and through an `@optional @external`
//! member with no collection at all. DDS-XTypes 1.3 §7.2.2.4.3 makes
//! each of them legal, because something terminates the recursion in
//! every sample.
//!
//! Ada writes a cycle with an incomplete type declaration (RM 3.10.1):
//!
//! ```ada
//! type Node;                                    --  incomplete
//! type Zd_Seq_Node_Array;                       --  incomplete
//! type Zd_Seq_Node is access Zd_Seq_Node_Array; --  legal: 3.10.1(9)
//! type Node is record
//!    Children : Zd_Seq_Node;
//! end record;                                   --  completion
//! type Zd_Seq_Node_Array is array (Natural range <>) of Node;
//! ```
//!
//! What makes that the only shape is the narrowness of RM 3.10.1(9): an
//! incomplete type may be the designated subtype of an access type, and
//! not a record component, an array element or a subtype mark. So:
//!
//! - The **array** behind a sequence or a map is declared incomplete
//!   too, because its element is the type on the cycle. The access type
//!   the member holds is complete straight away, which is what lets the
//!   record be written at all.
//! - A **map entry record** is declared incomplete for the same reason:
//!   its `Value` component is the type on the cycle.
//! - An **`@optional` or `@external` member** whose type is on the cycle
//!   becomes an access type — `Zd_Ref_Node` — rather than the `Present`
//!   wrapper, because that wrapper holds the value inline and would be
//!   as unbounded as the record it sits in. This is the one construct
//!   whose Ada shape depends on whether it is on a cycle, and the
//!   annotations agree with it: `null` is the absent state `@optional`
//!   needs, and indirection is what `@external` asks for (Table 21).
//! - An IDL **array** of a type on the cycle, a **union branch** of one,
//!   and a **typedef** of one are refused by name. The first two would
//!   have no bound, and RM 3.10.1 gives a subtype mark no incomplete
//!   form at all.
//!
//! The cut is chosen at an aggregate, because an aggregate is what an
//! incomplete declaration can name and complete. Every incomplete
//! declaration written is checked against the completions before the
//! package is handed back: an incomplete type with no completion is not
//! Ada, and it is not something a later reader should have to find.
//!
//! ### What GNAT 14.2.0 settled
//!
//! Everything above rests on the reference manual; the first run of the
//! whole corpus through `gcc-14 -c -gnat12` on Linux settled four things
//! the manual left this column guessing at, and each is written where it
//! is used:
//!
//! - A **negative case label** applies the `"-"` of the discriminant's
//!   type, which is not directly visible for a type reached as a
//!   qualified name (RM 8.4). Eleven cases failed; `operator_types` is
//!   the fix.
//! - An **identifier beginning with an underscore** is not one (RM
//!   2.3(2)), and IDL 4.2 §7.2.3.1 writes exactly that whenever a source
//!   escapes one of its own keywords. One case failed;
//!   [`ada::legal_name`] is the fix.
//! - A **variant choice naming an enumeration literal that a component
//!   of the same record is also named after** — `when A => A : …;` — is
//!   accepted. RM 8.3 hides a component from direct visibility, so the
//!   choice resolves to the literal, and the corpus's
//!   `union-enum-covered` shapes compiled.
//! - A **`when others => null;` arm** is accepted wherever this column
//!   writes one. It is written only where the earlier choices leave
//!   values over, so the question of an empty `others` does not arise in
//!   the generated text.
//!
//! ### Where the IDL spelling of a declaration is kept
//!
//! In the comment above it, and only where it moved. Unlike Go, this
//! column does not rename members for style: Ada folds case (RM
//! 2.3(5/2)), so [`ada::ada_case`] changes how a name is written and not
//! which name it is, and the IDL spelling reads back from the Ada one
//! unchanged. Three things do move a name — the reserved-word escape,
//! the lexical repair of [`ada::legal_name`], and the module flattening
//! — and none of them inverts: `Type_Idl` could have come from `type` or
//! from `type_idl`, `Idl_Long` from `_long` or from `Idl_long`. Every
//! one of them is recorded in a comment above the declaration or beside
//! the component, because a member's ID under `@autoid(HASH)` and under
//! `@hashid` is the first four octets of the MD5 of its **IDL** name
//! (DDS-XTypes 1.3 §7.3.1.2.1.1), and a spec that only knows the Ada
//! spelling cannot compute the ID for the member it is writing.

use crate::adapter::constant;
use crate::adapter::cst::MaterialiseError;
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{
    Ast, Bound, CaseLabel, CharWidth, ParamDirection, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::ada;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Ada";

/// The package a generated spec declares.
///
/// One name for every generated file. GNAT requires the source file name
/// to match the unit name, so a spec declaring `Zd_Types` has to be
/// written to `zd_types.ads` and nowhere else.
pub const PACKAGE: &str = "Zd_Types";

/// The file name a generated spec has to be written under.
///
/// Not a suggestion: GNAT derives the unit name from the file name, and
/// a package `Zd_Types` in any other file is a compilation error before
/// a single declaration is read.
pub const FILE_NAME: &str = "zd_types.ads";

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

/// The discriminant name of a generated union record.
///
/// `D` and not `_d`: RM 2.3 requires an identifier to begin with a
/// letter, so the `_d` every other column spells is not an Ada
/// identifier. The name is in the collision check with the branches,
/// because IDL 4.2 §7.4.1.4.4.1 gives the discriminator no member name
/// of its own and a branch called `d` would fold onto this one.
pub const DISCRIMINANT: &str = "D";

/// The discriminant name of a generated optional record.
pub const PRESENT: &str = "Present";

/// The component name a generated wrapper record carries its value in.
pub const VALUE: &str = "Value";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep the walk follows the type graph, matching the layout
/// planner's own bound.
const MAX_DEPTH: usize = 64;

/// What the emitter has to remember while it writes a cut cycle.
///
/// All four lists are empty for a model whose type graph is acyclic,
/// which is every corpus case but five, and the emitter's behaviour is
/// then exactly what it was before the cut existed.
#[derive(Debug, Clone, Default)]
struct Cycle {
    /// The types the order names before it declares them (RM 3.10.1).
    cut: Vec<Id<TypeNode>>,
    /// The types whose declaration has already been written.
    completed: Vec<Id<TypeNode>>,
    /// The types an incomplete declaration has been written for.
    ///
    /// Every one of these has to be completed before the package ends,
    /// which [`ada_items`] checks rather than assumes.
    predeclared: Vec<Id<TypeNode>>,
    /// The declarations held back until every type they name is
    /// complete.
    deferred: Vec<ada::Decl>,
}

/// The declaration a use of `id` waits for, if it has not been written.
///
/// `Some` only for a type in the cut set: those are the ones whose
/// declaration the order deliberately moved after a use of them. A type
/// outside the cut is either already written or is not part of a cycle
/// at all, and treating it as pending would change what this column
/// emits for every acyclic model.
/// Whether [`ada_items`] declares the member of `aggregate` whose type
/// is `member_type` as an access type: an `@optional` or `@external`
/// member whose type is still incomplete where `aggregate` is declared
/// (the `pending` arm of the record declaration, RM 3.10.1).
///
/// The codec, the Table 9 defaulter and the probe have to reach that
/// member as the access it is — `/= null`, `.all`, `new T'(…)` — and
/// not as the `Present`/`Value` record every other `@optional` member
/// is. The answer depends on the order Ada declares in, which is
/// [`ordered_types`]'s: a type is complete once its position in
/// `order` has been declared, so a target in the cut that is not
/// declared before `aggregate` is still waiting.
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

fn pending(ast: &Ast, id: Id<TypeNode>, cycle: &Cycle) -> Option<Id<TypeNode>> {
    let target = declared_target(ast, id)?;
    if cycle.cut.contains(&target) && !cycle.completed.contains(&target) {
        return Some(target);
    }
    // A **named** typedef of a type in the cut waits for the same thing
    // its target waits for. `declared_target` stops at the typedef,
    // because the typedef is the declaration a use resolves to; the
    // cycle is carried by what the alias chain ends at.
    // `subtype NodeRef is Node;` cannot stand before `Node` is
    // complete, so `sequence<NodeRef>` inside `Node` is a use of an
    // incomplete type exactly as `sequence<Node>` is — which is what
    // `recursive-typedef-sequence` is in the corpus for, and what it
    // caught: the generated array named `M_NodeRef` three declarations
    // before the subtype that declares it.
    let end = alias_end(ast, target);
    if end != target && cycle.cut.contains(&end) && !cycle.completed.contains(&end) {
        return Some(end);
    }
    None
}

/// The type an alias chain ends at, named links included.
///
/// [`declared_target`] goes through the unnamed links only, because an
/// unnamed alias has no Ada declaration and a named one does. This goes
/// all the way, which is the question [`pending`] asks.
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

/// The type whose declaration a use of `id` resolves to.
///
/// `None` where the Ada spelling is a name the predefined environment
/// declares — a primitive, a string, a `fixed` — and for the constructs
/// this column emits nothing for. An unnamed alias is transparent, so
/// the walk goes through it.
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
///
/// RM 3.10.1. Emitted where it is first needed rather than in a block at
/// the top of the package: an incomplete declaration with no completion
/// is not legal Ada, and putting it beside the use that needs it is what
/// keeps the two together.
fn predeclare(ast: &Ast, id: Id<TypeNode>, out: &mut Vec<ada::Decl>, cycle: &mut Cycle) {
    if cycle.predeclared.contains(&id) {
        return;
    }
    cycle.predeclared.push(id);
    out.push(ada::Decl::Type(ada::TypeDecl {
        doc: Doc::lines(&[
            "Declared here and completed below (RM 3.10.1). The IDL type reaches",
            "itself, so the name is needed before the type can be written; between",
            "the two an incomplete type may only be the designated subtype of an",
            "access type, which is what every use of it here is.",
        ]),
        name: Ident::new(&ada_type_name(ast, id)),
        def: ada::TypeDef::Incomplete,
    }));
}

/// `type X;` for a name this column invented, with a reason.
fn incomplete(name: &str, why: &str) -> ada::Decl {
    ada::Decl::Type(ada::TypeDecl {
        doc: Doc::line(why),
        name: Ident::new(name),
        def: ada::TypeDef::Incomplete,
    })
}

/// The access type a member held by reference takes, declared if new.
///
/// One per designated type, named structurally so two members of one
/// type collapse onto one declaration.
fn reference_type(
    ast: &Ast,
    target: Id<TypeNode>,
    out: &mut Vec<ada::Decl>,
    emitted: &mut Vec<Declared>,
    cycle: &mut Cycle,
) -> Result<String, MaterialiseError> {
    let name = format!("{REF_PREFIX}{}", type_tag(ast, target));
    if already(emitted, &name) {
        return Ok(name);
    }
    remember(emitted, &name, &name, false)?;
    predeclare(ast, target, out, cycle);
    out.push(ada::Decl::Type(ada::TypeDecl {
        doc: Doc::lines(&[
            "A reference to a type that reaches this one back. `null` is the absent",
            "state; RM 3.10.1(9) admits an incomplete designated subtype, which is",
            "why this declaration can stand before the type it designates.",
        ]),
        name: Ident::new(&name),
        def: ada::TypeDef::Access(Path::single(&ada_type_name(ast, target))),
    }));
    Ok(name)
}

/// One name the generated package declares.
#[derive(Debug, Clone)]
struct Declared {
    /// The folded key two Ada identifiers are the same identifier under.
    key: String,
    /// The IDL name behind it.
    declared: String,
    /// Whether Ada permits a second declaration of the same name.
    ///
    /// Enumeration literals and subprograms overload (RM 8.3); types,
    /// subtypes and constants do not.
    overloadable: bool,
}

/// Build the Ada declarations a rule application produced.
///
/// The order is a post-order over the type graph, followed by the
/// constants, the exceptions and the interfaces. Ada resolves nothing
/// forward, so unlike the Go column the output order is not the model's.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Ada form, or two declarations reach one Ada
/// identifier — never a silently dropped declaration.
pub fn ada_items(ast: &Ast, applied: &Applied) -> Result<Vec<ada::Decl>, MaterialiseError> {
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
            verbatim::declaration_slots(ast, id, verbatim::aliases::ADA),
            &ada_type_name(ast, id),
            start,
            &mut out,
        );
    }
    // The completions of the incomplete declarations, after every type
    // they name. RM 3.10.1 requires the completion to be in the same
    // declarative part, and by here every aggregate in it is written.
    out.append(&mut cycle.deferred);
    for id in &cycle.predeclared {
        if !cycle.completed.contains(id) {
            let name = ada_type_name(ast, *id);
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "`{name}`, declared incomplete to break a cycle and never completed \
                     — Ada 2012 RM 3.10.1 requires the completion in the same \
                     declarative part"
                ),
                language: LANGUAGE,
                at: name,
            });
        }
    }

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same. They come after the
    // types because each of them can name one.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "AdaConst").is_none() {
            continue;
        }
        // `eval_const` and not `eval_integral`: `const string NAME` is a
        // constant that evaluates perfectly well and is not a number.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = node.type_.resolved();
        let type_ = match declared {
            Some(target) => ada_value_path(ast, target)?,
            None => ada::Primitive::Integer32.path(),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name, false)?;
        out.push(ada::Decl::Object(ada::ObjectDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            type_: type_.clone(),
            value: ada_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "AdaError").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name, false)?;
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
                continue;
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            let spelled = component_name(&member_node.name);
            components.push(ada::Component {
                doc: idl_note(&spelled, &member_node.name),
                name: Ident::new(&spelled),
                type_: ada_member_path(ast, target, &member_node.name)?,
            });
        }
        out.push(ada::Decl::Type(ada::TypeDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An exception, as the record its members make. Not an Ada",
                    "`exception`: RM 11.1 gives an exception no components at all and",
                    "only a message string travels with one, so an IDL exception's",
                    "members would have nowhere to sit. Raising is the RPC arc's.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            def: ada::TypeDef::Record(ada::Record::plain(components)),
        }));
    }

    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "AdaInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name, false)?;
        out.push(ada::Decl::Type(ada::TypeDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface, as the Ada 2005 interface type its operations and",
                    "attributes make (RM 3.9.4): an attribute a Get_/Set_ pair of",
                    "primitive operations, `readonly` only the first. The operations",
                    "below are its primitive operations and are abstract, which RM",
                    "3.9.4 requires; the transport is the RPC arc's business and the",
                    "shape is the IDL's.",
                    "",
                    "A derived interface carries its bases' operations and attributes",
                    "too: this type is written with no progenitor list, so IDL 4.2",
                    "7.4.3's inherited members are declared against it rather than",
                    "inherited.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            def: ada::TypeDef::Interface,
        }));
        // The whole answer and not the declared one. `TypeDef::Interface`
        // writes `type T is interface;` with no progenitor list, so a
        // derived interface has no ancestor to inherit its primitive
        // operations from and OMG IDL 4.2 §7.4.3's inherited operations
        // and attributes are declared against it directly.
        // `interface::all_operations_flattened` gives both — the
        // Get_/Set_ pair DDS-RPC 1.0 §7.5.1.1.3 implies for each
        // attribute, besides the declared operations, already spelled
        // the way this column's own `Get_`/`Set_` primitive-operation
        // convention would — which `Ast::all_operations_of` alone never
        // did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        let mut signatures = Vec::new();
        for op in &operations {
            signatures.push((component_name(&op.name), op.name.clone()));
        }
        // IDL 4.2 §7.4.3.5.1 makes operation names unique within an
        // interface, so two that fold together came from the escape and
        // one of them would be unreachable.
        no_folded_collision(&signatures, &name)?;
        for op in &operations {
            let spelled = component_name(&op.name);
            remember(&mut emitted, &spelled, &op.name, true)?;
            // `Self` is this column's name for the controlling operand
            // and is in the collision check with the parameters: IDL
            // gives an operation no such parameter, so a declared one
            // called `self` would fold onto it.
            let mut names = vec![(SELF.to_string(), "<controlling operand>".to_string())];
            let mut params = vec![ada::Param {
                name: Ident::new(SELF),
                mode: ada::Mode::In,
                type_: Path::single(&name),
            }];
            for param in &op.params {
                let spelled = component_name(&param.name);
                names.push((spelled.clone(), param.name.clone()));
                params.push(ada::Param {
                    name: Ident::new(&spelled),
                    // The one mapping in this crate that keeps the IDL
                    // direction in the signature. RM 6.1 has all three
                    // modes and Ada 2012 admits `out` and `in out` on a
                    // function as well as on a procedure.
                    mode: match param.direction {
                        ParamDirection::In => ada::Mode::In,
                        ParamDirection::Out => ada::Mode::Out,
                        ParamDirection::InOut => ada::Mode::InOut,
                    },
                    type_: ada_member_path(ast, param.type_, &param.name)?,
                });
            }
            no_folded_collision(&names, &spelled)?;
            let result = match op.returns {
                Some(returns) => Some(ada_member_path(ast, returns, &op.name)?),
                None => None,
            };
            let mut doc = idl_note(&spelled, &op.name);
            if let Some(line) = op.raises_line() {
                // `raises_line` writes "\u{a7}7.11.1.1.2", and GNAT's
                // default source representation is Latin-1 with only
                // graphic ASCII admitted in a comment (RM 2.6) — the
                // same reason `every_generated_character_is_one_gnat_
                // reads_the_same_way` (`ada_end_to_end.rs`) checks the
                // whole file for one. `section ` says the same thing in
                // characters GNAT reads the same way under any encoding.
                doc.push(&line.replace('\u{a7}', "section "));
            }
            out.push(ada::Decl::Subprogram(ada::Subprogram {
                doc,
                kind: if result.is_some() {
                    ada::SubprogramKind::Function
                } else {
                    ada::SubprogramKind::Procedure
                },
                name: Ident::new(&spelled),
                params,
                result,
                is_abstract: true,
            }));
        }
    }

    Ok(out)
}

/// The name of the controlling operand of an interface operation.
const SELF: &str = "Self";

/// The Ada declarations, in a package specification that `with`s what it
/// names.
///
/// A caller that only wants the declarations uses [`ada_items`]; this is
/// what a generator writes to [`FILE_NAME`], and it exists here so that
/// the package name and the context clause are decided in one place.
///
/// # Errors
/// As [`ada_items`].
pub fn ada_file(ast: &Ast, applied: &Applied) -> Result<ada::File, MaterialiseError> {
    let decls = ada_items(ast, applied)?;
    Ok(ada::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "Ada 2012. GNAT derives the unit name from the file name, so this",
            "spec has to be compiled as zd_types.ads.",
            "",
            "Identifiers are case-insensitive in Ada (RM 2.3(5/2)), so the IDL",
            "spelling of a member reads back from the Ada one unchanged. Where it",
            "does not -- an Ada reserved word, an identifier RM 2.3 will not have,",
            "a module flattened onto the name -- the IDL spelling is in the comment",
            "beside the declaration, because a member's ID under @autoid(HASH) is",
            "the MD5 of the IDL name (DDS-XTypes 1.3 7.3.1.2.1.1).",
        ]),
        withs: context_clause(&decls),
        use_types: operator_types(&decls),
        package: Ident::new(PACKAGE),
        decls,
    })
}

/// The types whose predefined operators the spec has to make visible.
///
/// A predefined operator of a type declared in another package is not
/// directly visible where the type is reached as a qualified name
/// (RM 8.4). Nothing in this column writes an operator on purpose — but
/// a negative literal is one: RM 4.4 makes `-13` a `simple_expression`
/// applying the `"-"` of the expected type, so
/// `when -13 =>` on a discriminant of `Interfaces.Integer_8` is an
/// operator call, and GNAT answers `operator for type
/// "Interfaces.Integer_8" is not directly visible`. Measured on GNAT
/// 14.2.0: eleven corpus cases, every one of them a union with a
/// negative case label.
///
/// Only the types that actually need it, and only where the expression
/// really carries an operator — a `use type` for a type nothing applies
/// an operator to is a clause a reader has to account for and cannot.
///
/// An enumeration representation clause is **not** in this walk. Its
/// codes are of `universal_integer` (RM 13.4), whose operators come from
/// `Standard` and are always directly visible; the corpus has
/// `for M_E use (LOW => -5, …)` and GNAT accepted it.
fn operator_types(decls: &[ada::Decl]) -> Vec<Path> {
    let mut marks: Vec<Path> = Vec::new();
    let note = |mark: &Path, marks: &mut Vec<Path>| {
        let resolved = resolve_mark(decls, mark);
        // A single segment is a name this package declares itself, and a
        // derived type's operators are declared with it (RM 3.4), so
        // they are already directly visible.
        if resolved.segments().len() > 1 && !marks.contains(&resolved) {
            marks.push(resolved);
        }
    };
    for decl in decls {
        match decl {
            ada::Decl::Object(node) if needs_operator(&node.value) => {
                note(&node.type_, &mut marks);
            }
            ada::Decl::Type(node) => {
                let ada::TypeDef::Record(record) = &node.def else {
                    continue;
                };
                for discriminant in &record.discriminants {
                    if discriminant.default.as_ref().is_some_and(needs_operator) {
                        note(&discriminant.type_, &mut marks);
                    }
                }
                let Some(variant) = &record.variant else {
                    continue;
                };
                let Some(selector) = record
                    .discriminants
                    .iter()
                    .find(|d| d.name == variant.discriminant)
                else {
                    continue;
                };
                for arm in &variant.arms {
                    for choice in &arm.choices {
                        if let ada::Choice::Value(value) = choice {
                            if needs_operator(value) {
                                note(&selector.type_, &mut marks);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    marks
}

/// Whether writing this expression applies an operator of its type.
///
/// A negative literal is the only one this column produces. RM 4.4:
/// a `simple_expression` may carry a leading `unary_adding_operator`,
/// and that operator belongs to the expected type — it is not part of
/// the literal.
fn needs_operator(expr: &ada::Expr) -> bool {
    match expr {
        ada::Expr::Negate(_) => true,
        ada::Expr::Literal(Literal::Int(text) | Literal::Float(text)) => text.starts_with('-'),
        _ => false,
    }
}

/// The subtype mark a local name stands for.
///
/// `subtype Kind is Interfaces.Integer_32;` shares `Integer_32`'s
/// operators, which are declared in `Interfaces` and are no more
/// directly visible than they were (RM 3.2.2 makes a subtype another
/// name for one type, not a new one). A `use_type_clause` in a context
/// clause can only name a library-level entity, so the chain is followed
/// to the qualified name at its end.
fn resolve_mark(decls: &[ada::Decl], mark: &Path) -> Path {
    let mut current = mark.clone();
    for _ in 0..MAX_DEPTH {
        if current.segments().len() > 1 {
            return current;
        }
        let Some(name) = current.segments().first().map(Ident::as_str) else {
            return current;
        };
        let next = decls.iter().find_map(|decl| match decl {
            ada::Decl::Subtype(node) if node.name.as_str() == name => Some(node.mark.clone()),
            _ => None,
        });
        match next {
            Some(mark) => current = mark,
            None => return current,
        }
    }
    current
}

/// The `with` clauses the declarations need.
///
/// Derived from what the declarations actually name rather than written
/// out: `with` is a dependency of the compilation unit (RM 10.1.2), and
/// a clause for a unit nothing names is a dependency the build has to
/// satisfy for no reason.
fn context_clause(decls: &[ada::Decl]) -> Vec<Path> {
    let mut roots: Vec<String> = Vec::new();
    for decl in decls {
        collect_roots(decl, &mut roots);
    }
    let mut out = Vec::new();
    // The order is the one a reader expects and not the order the
    // declarations happened to name them in, so two runs over one model
    // produce the same context clause.
    for unit in [
        Path::single("Interfaces"),
        Path::new(&["Ada", "Strings", "Unbounded"]),
        Path::single("System"),
    ] {
        let named = unit
            .segments()
            .first()
            .is_some_and(|root| roots.iter().any(|seen| seen == root.as_str()));
        if named {
            out.push(unit);
        }
    }
    out
}

/// The first segment of every qualified name a declaration writes.
///
/// A single-segment name is one this column declared itself, so only a
/// qualified one names a library unit.
fn push_root(path: &Path, out: &mut Vec<String>) {
    if path.segments().len() > 1 {
        if let Some(root) = path.segments().first() {
            out.push(root.as_str().to_string());
        }
    }
}

/// Every library unit the declarations reach, by its first segment.
fn collect_roots(decl: &ada::Decl, out: &mut Vec<String>) {
    let push = push_root;
    match decl {
        ada::Decl::Type(node) => match &node.def {
            ada::TypeDef::Record(record) => {
                for discriminant in &record.discriminants {
                    push(&discriminant.type_, out);
                }
                for component in &record.components {
                    push(&component.type_, out);
                }
                if let Some(variant) = &record.variant {
                    for arm in &variant.arms {
                        for component in &arm.components {
                            push(&component.type_, out);
                        }
                    }
                }
            }
            ada::TypeDef::Derived(mark) | ada::TypeDef::Access(mark) => push(mark, out),
            ada::TypeDef::Array { index, element } => {
                push(element, out);
                for dimension in index {
                    if let ada::Index::Unconstrained(mark) = dimension {
                        push(mark, out);
                    }
                }
            }
            ada::TypeDef::Enumeration(_) | ada::TypeDef::Interface | ada::TypeDef::Incomplete => {}
        },
        ada::Decl::Subtype(node) => push(&node.mark, out),
        ada::Decl::Object(node) => {
            push(&node.type_, out);
            if let ada::Expr::Call { name, .. } = &node.value {
                push(name, out);
            }
        }
        ada::Decl::Subprogram(node) => {
            for param in &node.params {
                push(&param.type_, out);
            }
            if let Some(result) = &node.result {
                push(result, out);
            }
        }
        // A body never appears in the materialised spec: `ada_items`
        // emits declarations only, and the codec arc's bodies are the
        // probe generator's to place.
        ada::Decl::EnumRep(_)
        | ada::Decl::Comment(_)
        | ada::Decl::Verbatim(_)
        | ada::Decl::SubprogramBody(_) => {}
    }
}

/// A declaration order, and the types whose declaration it moved.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Every type in the model, dependencies first.
    pub order: Vec<Id<TypeNode>>,
    /// The types the order names before it declares them.
    ///
    /// Each needs an incomplete type declaration (RM 3.10.1) ahead of
    /// its first use, and its own declaration is the completion. Empty
    /// for a model with no cycle in its type graph, which is every
    /// corpus case but five.
    pub cut: Vec<Id<TypeNode>>,
}

/// Every type in the model, dependencies first, with the cycles cut.
///
/// Post-order over the type graph, as in the C column. The difference
/// from C is what happens at a cycle. C names an incomplete `struct`
/// through a pointer anywhere it likes; Ada 2012 RM 3.10.1 admits an
/// incomplete type only as the **designated subtype of an access type**
/// — never as the component of a record, the element of an array, or a
/// subtype mark — so cutting a cycle is only possible where the shape on
/// the other side of the cut is an access type.
///
/// The walk therefore does two things. It finds a cycle, and it chooses
/// where to cut it: at an aggregate, because an aggregate is what an
/// incomplete type declaration can name and complete. A cycle with no
/// aggregate on it is left uncut and refused by name when the emitter
/// reaches it.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when the cycles cannot be cut in
/// as many passes as the model has types, which would mean the walk was
/// not making progress.
pub fn ordered_types(ast: &Ast) -> Result<Plan, MaterialiseError> {
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
        construct: "a cycle in the type graph that cannot be cut at an aggregate — Ada \
                    2012 RM 3.10.1 admits an incomplete type only as the designated \
                    subtype of an access type"
            .to_string(),
        language: LANGUAGE,
        at: "the generated package".to_string(),
    })
}

/// The state of one pass of the ordering walk.
struct Walk {
    order: Vec<Id<TypeNode>>,
    seen: Vec<Id<TypeNode>>,
    stack: Vec<Id<TypeNode>>,
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
        // A back edge. The cycle is the stack from here on; the type to
        // declare incomplete is the first aggregate on it, because that
        // is the only kind an incomplete declaration can name and
        // complete.
        walk.found = Some(cut_point(ast, &walk.stack[position..]).unwrap_or(id));
        return;
    }
    if depth > MAX_DEPTH || walk.seen.contains(&id) {
        return;
    }
    walk.seen.push(id);
    walk.stack.push(id);
    for dependency in dependencies(ast, id) {
        // An edge into a type already cut is satisfied by its incomplete
        // declaration, so the walk does not follow it.
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

/// Whether a type is one an incomplete declaration can name.
///
/// A record or a discriminated record: `type Node;` followed later by
/// `type Node is record … end record;` is a full type declaration
/// completing an incomplete one (RM 3.10.1). A sequence or a map is not
/// on the list because this column writes each as two declarations
/// already, and the cut has to land where a single name is declared
/// twice.
fn is_aggregate(ast: &Ast, id: Id<TypeNode>) -> bool {
    matches!(
        ast.type_(id).map(|node| &node.kind),
        Ok(TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. })
    )
}

/// The types a type's Ada declaration has to be preceded by.
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
    out: &mut Vec<ada::Decl>,
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
    out: &mut Vec<ada::Decl>,
    emitted: &mut Vec<Declared>,
    cycle: &mut Cycle,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive and the two string shapes are written where the
        // member is: `Interfaces.Integer_32` and
        // `Ada.Strings.Unbounded.Unbounded_String` are names the
        // predefined environment already declares.
        TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. } => {}

        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: ada_type_name(ast, id),
                });
            };
            let access = ada_type_name(ast, id);
            if already(emitted, &access) {
                cycle.completed.push(id);
                return Ok(());
            }
            let array = format!("{access}{ARRAY_SUFFIX}");
            remember(emitted, &array, &array, false)?;
            remember(emitted, &access, &access, false)?;
            let elements = ada::Decl::Type(ada::TypeDecl {
                doc: Doc::lines(&[
                    "The elements of a sequence. `Natural range <>` leaves the length",
                    "out of the type, which is where the wire form keeps it. A",
                    "component's subtype must be definite (RM 3.8), so this type is",
                    "reached through the access type below.",
                ]),
                name: Ident::new(&array),
                def: ada::TypeDef::Array {
                    index: vec![ada::Index::Unconstrained(Path::single("Natural"))],
                    element: ada_member_path(ast, element, &access)?,
                },
            });
            // An element on a cycle is not declared yet, and RM 3.10.1
            // will not have an incomplete type as an array element. The
            // array type is therefore itself declared incomplete, the
            // access type designates it — which RM 3.10.1(9) does admit
            // — and the array's own declaration is the completion, after
            // the element's.
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
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: Doc::line(
                    "A sequence. `null` is an empty one that was never allocated; the \
                     storage is the caller's.",
                ),
                name: Ident::new(&access),
                def: ada::TypeDef::Access(Path::single(&array)),
            }));
        }

        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: ada_type_name(ast, id),
                });
            };
            // An IDL array of a type that reaches back to this one is a
            // value of unbounded size, and Ada has no spelling for it:
            // an array element may not be an incomplete type (RM
            // 3.10.1), and there is no length to terminate the recursion
            // as a sequence's would (DDS-XTypes 1.3 §7.2.2.4.3).
            if let Some(target) = pending(ast, element, cycle) {
                let name = ada_type_name(ast, target);
                return Err(MaterialiseError::NoLanguageForm {
                    construct: format!(
                        "an array whose element `{name}` reaches the array itself — Ada \
                         2012 RM 3.10.1 does not admit an incomplete type as an array \
                         element, and the value would have no bound"
                    ),
                    language: LANGUAGE,
                    at: ada_type_name(ast, id),
                });
            }
            let tag = type_tag(ast, element);
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(ast.eval_integral(*dim).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: ada_type_name(ast, id),
                    }
                })?);
            }
            // One declared type per dimension, innermost first: Ada has
            // no anonymous array type in a component position, so
            // `array (0 .. 1) of array (0 .. 2) of T` cannot be written
            // as one declaration. The outermost dimension stays
            // outermost — `long grid[2][3]` is two rows of three, and
            // reversing it silently transposes the data.
            let mut current = ada_member_path(ast, element, &tag)?;
            for index in (0..lengths.len()).rev() {
                let name = array_type_name(&tag, &lengths[index..]);
                if !already(emitted, &name) {
                    remember(emitted, &name, &name, false)?;
                    out.push(ada::Decl::Type(ada::TypeDecl {
                        doc: Doc::line(
                            "One dimension of an array. The element count is in the type \
                             and not on the wire.",
                        ),
                        name: Ident::new(&name),
                        def: ada::TypeDef::Array {
                            index: vec![ada::Index::Range {
                                low: ada::Expr::int(0),
                                high: ada::Expr::int(lengths[index] - 1),
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
                    at: ada_type_name(ast, id),
                });
            };
            let access = ada_type_name(ast, id);
            if already(emitted, &access) {
                cycle.completed.push(id);
                return Ok(());
            }
            let entry = ada_map_entry_name(ast, id);
            let array = format!("{access}{ARRAY_SUFFIX}");
            remember(emitted, &entry, &entry, false)?;
            remember(emitted, &array, &array, false)?;
            remember(emitted, &access, &access, false)?;
            let pair = ada::Decl::Type(ada::TypeDecl {
                doc: Doc::line(
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them.",
                ),
                name: Ident::new(&entry),
                def: ada::TypeDef::Record(ada::Record::plain(vec![
                    ada::Component {
                        doc: Doc::none(),
                        name: Ident::new("Key"),
                        type_: ada_member_path(ast, key, &entry)?,
                    },
                    ada::Component {
                        doc: Doc::none(),
                        name: Ident::new(VALUE),
                        type_: ada_member_path(ast, value, &entry)?,
                    },
                ])),
            });
            let entries = ada::Decl::Type(ada::TypeDecl {
                doc: Doc::line("The entries of a map, in the order the wire carries them."),
                name: Ident::new(&array),
                def: ada::TypeDef::Array {
                    index: vec![ada::Index::Unconstrained(Path::single("Natural"))],
                    element: Path::single(&entry),
                },
            });
            // A key or a value on a cycle is not declared yet, and an
            // entry record may not have an incomplete component (RM
            // 3.10.1). Both the entry and the array of entries are
            // therefore declared incomplete here and completed after the
            // key and the value; the access type, which is what the
            // member holds, is complete straight away.
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
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: Doc::line("A map, held as the ordered sequence of pairs it is on the wire."),
                name: Ident::new(&access),
                def: ada::TypeDef::Access(Path::single(&array)),
            }));
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "AdaSubtype").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = ada_type_name(ast, id);
            remember(emitted, &name, &node.name, false)?;
            // A subtype and not `type X is new Y`. RM 3.2.2 makes a
            // subtype another name for one type; RM 3.4 makes a derived
            // type a new one with its own identity, so a value of
            // `Distance` would need an explicit conversion to reach an
            // `Interfaces.Integer_32`. IDL 4.2 §7.4.1.4.3: the alias
            // "does not define a new type".
            let decl = ada::Decl::Subtype(ada::SubtypeDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                mark: ada_member_path(ast, target, &node.name)?,
            });
            // A subtype mark may not name an incomplete type (RM
            // 3.10.1), and a subtype has no incomplete form of its own
            // to be cut at — but nothing forces the subtype to sit at
            // the source's line either: `typedef Node NodeRef;` ahead
            // of `struct Node`'s completion is held back and declared
            // once the target is complete, the same deferral the array
            // and map instantiations on a cycle take. A *use* of the
            // alias on the same cycle — `sequence<NodeRef>` inside
            // `Node` — is deferred by its own arm, and deferral keeps
            // declaration order, so the subtype still lands ahead of
            // it; a use off the cycle sits after the typedef in the
            // source (IDL declares before use) and its target is then
            // not pending either.
            if pending(ast, target, cycle).is_some() {
                cycle.deferred.push(decl);
            } else {
                out.push(decl);
            }
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "AdaEnum").is_none() {
                return Ok(());
            }
            let name = ada_type_name(ast, id);
            let mut literals = Vec::new();
            let mut codes = Vec::new();
            let mut pairs = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "AdaLiteral").is_none() {
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
                literals.push(Ident::new(&spelled));
                codes.push((Ident::new(&spelled), value));
            }
            // RM 3.5.1: an enumeration type definition needs at least
            // one literal. An IDL enumeration with none has no Ada form
            // and is refused by name rather than emitted as `is ()`.
            if literals.is_empty() {
                return Err(MaterialiseError::NoLanguageForm {
                    construct: format!(
                        "`{name}`, an enumeration with no enumerators — Ada 2012 RM \
                         3.5.1 requires at least one literal"
                    ),
                    language: LANGUAGE,
                    at: name,
                });
            }
            // RM 13.4: the codes shall be distinct and shall satisfy the
            // predefined ordering of the type — that is, ascend in
            // declaration order. IDL puts no such constraint on
            // `@value`. Refused by name: emitting the clause anyway
            // produces a spec GNAT rejects, and dropping the clause
            // silently renumbers every value on the wire.
            for pair in codes.windows(2) {
                if pair[0].1 >= pair[1].1 {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: format!(
                            "`{name}`, an enumeration whose @value codes do not ascend \
                             ({} => {}, then {} => {}) — Ada 2012 RM 13.4 requires an \
                             enumeration representation clause to follow the type's own \
                             ordering, and without the clause the wire numbering is lost",
                            pair[0].0.as_str(),
                            pair[0].1,
                            pair[1].0.as_str(),
                            pair[1].1
                        ),
                        language: LANGUAGE,
                        at: name,
                    });
                }
            }
            no_folded_collision(&pairs, &name)?;
            remember(emitted, &name, &node.name, false)?;
            for (spelled, declared) in &pairs {
                // Overloadable: an enumeration literal is a
                // parameterless function returning its own type (RM
                // 3.5.1), so two enumerations in one package may both
                // declare `IDLE` and every use resolves by context.
                remember(emitted, spelled, declared, true)?;
            }
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration. The literals carry no codes here: RM 3.5.1",
                        "numbers them 0, 1, 2 and so on in declaration order, so the clause",
                        "below is what states the wire numbering.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada::TypeDef::Enumeration(literals),
            }));
            out.push(ada::Decl::EnumRep(ada::EnumRep {
                doc: Doc::lines(&[
                    "The codes DDS-XTypes 1.3 7.4.3.5.3 puts on the wire, as the",
                    "representation clause of RM 13.4. Without it an @value gap would be",
                    "silently renumbered to the literal's position.",
                ]),
                type_: Ident::new(&name),
                values: codes
                    .into_iter()
                    .map(|(literal, value)| (literal, ada::Expr::int(value)))
                    .collect(),
            }));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "AdaBitmask").is_none() {
                return Ok(());
            }
            let name = ada_type_name(ast, id);
            remember(emitted, &name, &node.name, false)?;
            let bound = ast.effective_bit_bound(id);
            // A derived type and not a subtype: a bitmask is its own
            // type on the wire, and RM 3.4 derivation is what gives it
            // an identity of its own while keeping the modular
            // arithmetic of `Interfaces.Unsigned_N`.
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: with_idl_note(
                    Doc::line(
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3 \
                         7.4.1.1.3.2 gives for its bound.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada::TypeDef::Derived(ada_holder(bound).path()),
            }));
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "AdaBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                // A constant of the bitmask's own type has to be a value
                // of it. A position at or beyond the bound names a bit
                // the holder does not have, and the declaration would
                // not compile; refused by name rather than emitted with
                // the value wrapped by the modulus, which is what Ada
                // would otherwise do to it silently.
                if position >= bound {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: format!(
                            "`{}`, a bit at position {position} of a bitmask bounded at \
                             {bound} — the constant has no value in the holder type",
                            bit_node.name
                        ),
                        language: LANGUAGE,
                        at: name,
                    });
                }
                let spelled = format!("{prefix}{}", component_name(&bit_node.name));
                remember(emitted, &spelled, &bit_node.name, false)?;
                // The value and not a shift: Ada's `**` on a modular
                // type takes a `Natural` right operand and yields the
                // modular type, so `2 ** 4` would be a `Flags` already —
                // and the literal says what the bit is without the
                // reader working it out.
                out.push(ada::Decl::Object(ada::ObjectDecl {
                    doc: idl_note(&spelled, &bit_node.name),
                    name: Ident::new(&spelled),
                    type_: Path::single(&name),
                    value: ada::Expr::int(1_i128 << u32::from(position)),
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "AdaBitset").is_none() {
                return Ok(());
            }
            let name = ada_type_name(ast, id);
            remember(emitted, &name, &node.name, false)?;
            let mut pairs = Vec::new();
            // The base's bitfields are members of the derived record
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
                components.push(ada::Component {
                    doc: Doc::none(),
                    name: Ident::new(&pairs[index].0),
                    type_: ada_holder(field_node.bits).path(),
                });
                index += 1;
            }
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitset. The fields are separate integers here and not an Ada",
                        "record representation clause: the order on the wire is the",
                        "declaration's, and the packing into the anchor is the codec's,",
                        "where it can be seen.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada::TypeDef::Record(ada::Record::plain(components)),
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "AdaValue"
            } else {
                "AdaRecord"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = ada_type_name(ast, id);
            remember(emitted, &name, &node.name, false)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`; the wire
            // still carries the base's members first (§7.4.1.1.5.1).
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
                components.push(ada::Component {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    name: Ident::new(&component_name("parent")),
                    type_: ada_member_path(ast, base, "parent")?,
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
                let mut type_ = ada_member_path(ast, target, &member_node.name)?;
                let mut doc = Doc::none();
                match pending(ast, target, cycle) {
                    // A member whose type reaches this one back. The
                    // record cannot hold it by value at all — the value
                    // would have no bound — and RM 3.10.1 will not have
                    // an incomplete type as a component. An access type
                    // is the one shape the clause admits, and it is also
                    // what `@external` says the member is: held by
                    // reference (DDS-XTypes 1.3 Table 21). `null` is the
                    // absent state an `@optional` member needs, so the
                    // two annotations reach one declaration here.
                    Some(waiting) if optional || external => {
                        type_ = Path::single(&reference_type(ast, waiting, out, emitted, cycle)?);
                        doc = Doc::lines(&[
                            "Held by reference, because the type reaches this one back",
                            "and an Ada record cannot contain itself. `null` is the",
                            "absent state of an @optional member (DDS-XTypes 1.3",
                            "7.2.2.4.4.4.7) and the indirection is what @external asks",
                            "for (Table 21); RM 3.10.1 admits an incomplete type only",
                            "as the designated subtype of an access type, so this is",
                            "also the only spelling available here.",
                        ]);
                    }
                    Some(waiting) => {
                        return Err(MaterialiseError::NoLanguageForm {
                            construct: format!(
                                "a member of `{}`, which reaches `{name}` back with no \
                                 @optional, no @external and no collection between them \
                                 — the value would have no bound, and RM 3.10.1 does \
                                 not admit an incomplete type as a record component",
                                ada_type_name(ast, waiting)
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
                components.push(ada::Component {
                    doc: with_idl_note(doc, &spelled, &member_node.name),
                    name: Ident::new(&spelled),
                    type_,
                });
            }
            // An aggregate with no members is `is null record;` and not
            // `record end record;` (RM 3.8). Unlike the C column, which
            // has to refuse it — C23 §6.7.2.1p8 leaves a structure with
            // no named member undefined — Ada has a spelling for it and
            // the corpus contains the case.
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                def: ada::TypeDef::Record(ada::Record::plain(components)),
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "AdaUnion").is_none() {
                return Ok(());
            }
            let name = ada_type_name(ast, id);
            remember(emitted, &name, &node.name, false)?;
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
            if ghost_created_for(applied, node.node_id, "AdaNative").is_none() {
                return Ok(());
            }
            let name = ada_type_name(ast, id);
            remember(emitted, &name, &node.name, false)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `System.Address` when there is none.
            // A mapping that is not an Ada name is not used — a facet
            // written for the C column holds `void *`, which is not a
            // subtype mark and would produce a spec no compiler reads.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone())
                .filter(|spelling| is_ada_name(spelling));
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: with_idl_note(
                    Doc::line(
                        "A native type. `System.Address` unless a facet named a mapping \
                         that is an Ada subtype mark.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada::TypeDef::Derived(match mapping {
                    Some(spelling) => Path::new(&spelling.split('.').collect::<Vec<&str>>()),
                    None => ada::Primitive::Address.path(),
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
            let name = ada_type_name(ast, id);
            if already(emitted, &name) {
                cycle.completed.push(id);
                return Ok(());
            }
            remember(emitted, &name, &node.name, false)?;
            // An access type is the one place RM 3.10.1(9) does admit an
            // incomplete designated subtype, so a boxed value on a cycle
            // needs only the name declared ahead of it.
            if let Some(waiting) = pending(ast, target, cycle) {
                predeclare(ast, waiting, out, cycle);
            }
            out.push(ada::Decl::Type(ada::TypeDecl {
                doc: with_idl_note(
                    Doc::line("A boxed value: an access type, `null` when absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                def: ada::TypeDef::Access(ada_member_path(ast, target, &name)?),
            }));
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `ada_member_path`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The discriminated record an IDL union becomes.
///
/// One record whose branches are inside it (RM 3.8.1), not a struct with
/// every branch beside every other as in the Go column. The discriminant
/// has a default, which is what makes the record unconstrained: a value
/// may change branch at an assignment, and a component of the type needs
/// no constraint where it is declared (RM 3.7).
///
/// The `when others` arm is the honest answer to a discriminator taking
/// a value no branch selects. IDL admits that — a `long` discriminator
/// has 2**32 values and a union rarely lists them all — and DDS-XTypes
/// 1.3 §7.4.1.1.5.2 writes only the selected branch, so a sample whose
/// discriminator matches nothing carries the discriminator and no
/// branch. `when others => null;` is exactly that shape. It is written
/// only where values are left over: RM 3.8.1 makes `others` cover what
/// the earlier choices did not, and where they covered everything the
/// arm would be empty.
#[allow(clippy::too_many_arguments)]
fn union_decl(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    disc: Id<TypeNode>,
    name: &str,
    declared: &str,
    cycle: &Cycle,
) -> Result<ada::Decl, MaterialiseError> {
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
                "`{name}`, a union whose discriminator is not a discrete type — Ada \
                 2012 RM 3.7 admits only a discrete or an access type as a discriminant"
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

    let mut arms: Vec<ada::Arm> = Vec::new();
    let mut default: Option<ada::Arm> = None;
    let mut covered: Vec<i128> = Vec::new();
    for case in cases {
        let Ok(case_node) = ast.case(*case) else {
            continue;
        };
        if ghost_created_for(applied, case_node.node_id, "AdaCase").is_none() {
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
        // A branch whose type reaches this union back. IDL 4.2 gives a
        // union case no `@optional`, so there is no annotation to turn
        // the component into an access type as a struct member's would
        // be, and RM 3.10.1 does not admit an incomplete type as a
        // component of a variant part either.
        if let Some(waiting) = pending(ast, target, cycle) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a union branch of `{}`, which reaches `{name}` back with no \
                     collection between them — the value would have no bound",
                    ada_type_name(ast, waiting)
                ),
                language: LANGUAGE,
                at: member.name.clone(),
            });
        }
        let spelled = component_name(&member.name);
        let component = ada::Component {
            doc: idl_note(&spelled, &member.name),
            name: Ident::new(&spelled),
            type_: ada_member_path(ast, target, &member.name)?,
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
                                 — Ada 2012 RM 3.8.1 requires the choices of a variant \
                                 part to be distinct"
                            ),
                            language: LANGUAGE,
                            at: member.name.clone(),
                        });
                    }
                    covered.push(value);
                    choices.push(ada::Choice::Value(choice_expr(
                        ast, underlying, value, name,
                    )?));
                }
            }
        }
        if is_default {
            // The default branch is `when others`, which RM 3.8.1
            // requires to be alone and last. Any value labels it also
            // carried are covered by `others` too, because nothing after
            // it can claim them.
            default = Some(ada::Arm {
                choices: vec![ada::Choice::Others],
                components: vec![component],
            });
        } else if choices.is_empty() {
            // A case with no labels selects nothing; it would be an
            // Ada variant with an empty choice list, which does not
            // parse. Skipped rather than emitted with `others`, which
            // would give it every value no branch claims.
            continue;
        } else {
            arms.push(ada::Arm {
                choices,
                components: vec![component],
            });
        }
    }
    if let Some(arm) = default {
        arms.push(arm);
    } else if !covers_everything(ast, underlying, &covered) {
        arms.push(ada::Arm {
            choices: vec![ada::Choice::Others],
            components: Vec::new(),
        });
    }

    Ok(ada::Decl::Type(ada::TypeDecl {
        doc: with_idl_note(
            Doc::lines(&[
                "A union, as the discriminated record of RM 3.8.1. `D` is part of the",
                "value and not a convention beside it, so reading a branch the",
                "discriminant does not select raises Constraint_Error rather than",
                "returning whatever was there. Only the selected branch is written to",
                "the wire (DDS-XTypes 1.3 7.4.1.1.5.2).",
            ]),
            name,
            declared,
        ),
        name: Ident::new(name),
        def: ada::TypeDef::Record(ada::Record {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            discriminants: vec![ada::Discriminant {
                name: Ident::new(DISCRIMINANT),
                type_: ada_member_path(ast, disc, name)?,
                default: Some(discriminant_default(ast, underlying, name)?),
            }],
            components: Vec::new(),
            variant: Some(ada::Variant {
                discriminant: Ident::new(DISCRIMINANT),
                arms,
            }),
        }),
    }))
}

/// The Ada choice a case label's value is written as.
///
/// For an enumeration discriminant the literal and not the number: a
/// variant choice has to be a value of the discriminant's subtype, and
/// an integer is not one.
fn choice_expr(
    ast: &Ast,
    underlying: Id<TypeNode>,
    value: i128,
    at: &str,
) -> Result<ada::Expr, MaterialiseError> {
    match ast.type_(underlying).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { enumerators }) => {
            for enumerator in &enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ast.effective_enumerator_value(*enumerator) == Some(value) {
                    return Ok(ada::Expr::name(&component_name(&enum_node.name)));
                }
            }
            Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a union case label of {value}, which names no enumerator of the \
                     discriminator — an Ada variant choice has to be a value of the \
                     discriminant's subtype"
                ),
                language: LANGUAGE,
                at: at.to_string(),
            })
        }
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            Ok(ada::Expr::Literal(Literal::Bool(value != 0)))
        }
        _ => Ok(ada::Expr::int(value)),
    }
}

/// Whether the labels already cover every value of the discriminant.
///
/// Only two discriminant shapes can be covered at all: `Boolean`, which
/// has two values, and an enumeration, which has as many as it declares.
/// An integer discriminant is never covered — a `long` has 2**32 values
/// — so a union on one always keeps its `others` arm.
fn covers_everything(ast: &Ast, underlying: Id<TypeNode>, covered: &[i128]) -> bool {
    match ast.type_(underlying).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { enumerators }) => enumerators.iter().all(|enumerator| {
            ast.effective_enumerator_value(*enumerator)
                .is_some_and(|value| covered.contains(&value))
        }),
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            covered.contains(&0) && covered.contains(&1)
        }
        _ => false,
    }
}

/// The default a union's discriminant takes.
///
/// A discriminant with no default makes the record constrained, and a
/// constrained record cannot be a component of anything without the
/// branch being fixed where the component is declared (RM 3.7). The
/// value is the first enumerator, `False`, or zero — it is what a
/// default-constructed value has before anything is decoded into it, and
/// the codec overwrites it from the wire.
fn discriminant_default(
    ast: &Ast,
    underlying: Id<TypeNode>,
    at: &str,
) -> Result<ada::Expr, MaterialiseError> {
    match ast.type_(underlying).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { enumerators }) => {
            for enumerator in &enumerators {
                if let Ok(enum_node) = ast.enumerator(*enumerator) {
                    return Ok(ada::Expr::name(&component_name(&enum_node.name)));
                }
            }
            Err(MaterialiseError::NoLanguageForm {
                construct: "a union on an enumeration with no enumerators, which has no \
                            value its discriminant could default to"
                    .to_string(),
                language: LANGUAGE,
                at: at.to_string(),
            })
        }
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
            Ok(ada::Expr::Literal(Literal::Bool(false)))
        }
        _ => Ok(ada::Expr::int(0)),
    }
}

/// The record an `@optional` member is held in, declared if it is new.
///
/// One per element type, named structurally so two optional members of
/// one type collapse onto one declaration.
fn optional_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
    out: &mut Vec<ada::Decl>,
    emitted: &mut Vec<Declared>,
) -> Result<String, MaterialiseError> {
    let name = format!("{OPT_PREFIX}{}", type_tag(ast, target));
    if already(emitted, &name) {
        return Ok(name);
    }
    remember(emitted, &name, &name, false)?;
    out.push(ada::Decl::Type(ada::TypeDecl {
        doc: Doc::lines(&[
            "An optional value (DDS-XTypes 1.3 7.2.2.4.4.4.7). The absent state has",
            "no component at all, so reading `Value` when `Present` is False raises",
            "Constraint_Error (RM 3.8.1) rather than returning whatever was there,",
            "which is what a flag beside the value would do.",
        ]),
        name: Ident::new(&name),
        def: ada::TypeDef::Record(ada::Record {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            discriminants: vec![ada::Discriminant {
                name: Ident::new(PRESENT),
                type_: ada::Primitive::Boolean.path(),
                default: Some(ada::Expr::Literal(Literal::Bool(false))),
            }],
            components: Vec::new(),
            variant: Some(ada::Variant {
                discriminant: Ident::new(PRESENT),
                arms: vec![
                    ada::Arm {
                        choices: vec![ada::Choice::Value(ada::Expr::Literal(Literal::Bool(true)))],
                        components: vec![ada::Component {
                            doc: Doc::none(),
                            name: Ident::new(VALUE),
                            type_: ada_member_path(ast, target, at)?,
                        }],
                    },
                    ada::Arm {
                        choices: vec![ada::Choice::Value(ada::Expr::Literal(Literal::Bool(false)))],
                        components: Vec::new(),
                    },
                ],
            }),
        }),
    }));
    Ok(name)
}

/// The Ada spelling of a value's type, refusing what Ada cannot hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided at
/// run time — an Ada spec could declare a class-wide type for it and
/// there would be no wire form to generate a codec against, which is the
/// silent-substitution class this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn ada_member_path(
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
    ada_value_path(ast, target)
}

/// The Ada subtype mark a value of `id` is held under.
///
/// The one answer, for every emitter. A component of a generated record,
/// a codec's parameter and a defaulter's destination all have to name
/// the same Ada subtype or the spec does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a collection's element does
/// not resolve.
pub fn ada_value_path(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    value_path_at(ast, id, 0)
}

/// [`ada_value_path`], carrying how many transparent typedefs the walk
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
        return Ok(ada::Primitive::Address.path());
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => ada_primitive(*kind).path(),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ada::Primitive::UnboundedString.path(),
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the Ada mark is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_path_at(ast, target, depth + 1)?,
            None => ada::Primitive::Address.path(),
        },
        _ => Path::single(&type_name_at(ast, id, depth + 1)),
    })
}

/// The Ada type an IDL primitive is held in.
///
/// Exhaustive, so a primitive added to the model without an Ada spelling
/// is a compile error and not a member that silently takes another's.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open, and a fixed-width integer is the only choice that does not
/// depend on a platform type. Ada's `Character` is Latin-1 and its
/// `Wide_Character` is a 16-bit code point (RM A.1 and A.3.3), so
/// neither is the octet the wire carries.
///
/// `long double` is an `Unbounded_String` and never `Long_Long_Float`;
/// the module documentation carries why.
#[must_use]
pub const fn ada_primitive(kind: PrimitiveKind) -> ada::Primitive {
    match kind {
        PrimitiveKind::Boolean => ada::Primitive::Boolean,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => {
            ada::Primitive::Unsigned8
        }
        PrimitiveKind::Int8 => ada::Primitive::Integer8,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => ada::Primitive::Unsigned16,
        PrimitiveKind::Int16 => ada::Primitive::Integer16,
        PrimitiveKind::Int32 => ada::Primitive::Integer32,
        PrimitiveKind::UInt32 => ada::Primitive::Unsigned32,
        PrimitiveKind::Int64 => ada::Primitive::Integer64,
        PrimitiveKind::UInt64 => ada::Primitive::Unsigned64,
        PrimitiveKind::Float32 => ada::Primitive::Float32,
        PrimitiveKind::Float64 => ada::Primitive::Float64,
        PrimitiveKind::Float128 => ada::Primitive::UnboundedString,
    }
}

/// The unsigned integer a bit width calls for.
#[must_use]
pub const fn ada_holder(bits: u16) -> ada::Primitive {
    match bits {
        0..=8 => ada::Primitive::Unsigned8,
        9..=16 => ada::Primitive::Unsigned16,
        17..=32 => ada::Primitive::Unsigned32,
        _ => ada::Primitive::Unsigned64,
    }
}

/// `@verbatim` in the Ada column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a record's component list; an
/// enumeration, a derived type and an array take them adjacent. The file
/// slots are the caller's, because the file around these declarations
/// — the package, the context clauses before it — is.
impl verbatim::Carrier for ada::Decl {
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
        let ada::TypeDef::Record(record) = &mut node.def else {
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

/// A declared type's Ada identifier.
///
/// The scoped name with the module separators flattened onto it:
/// `::common::util::Location` is `Common_Util_Location`.
#[must_use]
pub fn ada_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    type_name_at(ast, id, 0)
}

/// [`ada_type_name`], carrying how many connector steps the walk has
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
/// one Ada identifier, and the second declaration would stand for
/// both.
fn type_name_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("Zd_Deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return "Zd_Unnamed".to_string();
    };
    match &node.kind {
        TypeKind::Primitive(kind) => ada_primitive(*kind).as_str().to_string(),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            ada::Primitive::UnboundedString.as_str().to_string()
        }
        TypeKind::Sequence { element, .. } => match element.resolved() {
            Some(element) => format!("{SEQ_PREFIX}{}", type_tag_at(ast, element, depth + 1)),
            None => format!("{SEQ_PREFIX}Unknown"),
        },
        TypeKind::Array { element, dims } => match element.resolved() {
            Some(element) => {
                let mut lengths = Vec::new();
                for dim in dims {
                    // A dimension that does not evaluate gets no number
                    // rather than a zero: the name has to differ from a
                    // `[0]` array's, and `declare` reports the failure
                    // by name.
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
pub fn ada_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
    map_entry_name_at(ast, id, 0)
}

/// [`ada_map_entry_name`], carrying the depth of the walk that reached
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

/// The name component an invented type takes from an element type.
///
/// The IDL kind and not the Ada spelling, and this is not cosmetic:
/// `double` and `long double` are `Interfaces.IEEE_Float_64` and
/// `Unbounded_String` here, and `octet`, `uint8` and `char` are all
/// `Interfaces.Unsigned_8`. A structural name built from the Ada type
/// would give `sequence<octet>` and `sequence<char>` one declaration —
/// harmless today and the same collapse that would be wrong if the two
/// ever differed on the wire.
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
        Ok(TypeKind::Primitive(kind)) => ada::ada_case(&kind.as_str().replace(' ', "_")),
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

/// The bound a `string`, a `sequence` or a `map` carries, as the tail
/// of its tag.
///
/// The tag keys the codec routine the generated program hoists a
/// collection into ([`crate::emit::ada::Ada::type_name`]), so a tag
/// that leaves the bound out gives `sequence<long, 4>` and
/// `sequence<long>` one routine and the first of the two decides what
/// the other's bound is: declared the other way round, the bounded
/// member is read by a routine with no length check at all. The
/// unevaluable case is `_X`, exactly as the `Array` arm above already
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

/// The Ada name a declaration outside the type arena takes.
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

/// Join scoped segments with `_`, escaping and casing each.
///
/// `_` and not concatenation: `CommonUtilLocation` cannot be read back
/// to its segments, and two different scoped names would reach one Ada
/// name with nothing to say so. The join is still not injective — an IDL
/// identifier may itself contain `_` — which is why [`remember`] checks
/// every declared name against every other one in the package.
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

/// The Ada spelling of a member, an enumerator, a parameter or an
/// operation.
///
/// Escaped, repaired, then cased, in that order.
///
/// The escape runs first for the same reason it does in the Go column,
/// and differently: `type` escapes to `type_Idl` and then cases to
/// `Type_Idl`, where casing first would give `Type`, which folds onto
/// the reserved word `type` (RM 2.3(5/2)) and would leave the escape
/// with nothing to do.
///
/// [`ada::legal_name`] runs second, on the escaped spelling rather than
/// the IDL one, because the escape can only append `_Idl` and never
/// introduces a leading underscore or a doubled one — so the repair sees
/// everything either of them could have produced. It is what answers the
/// two constraints of RM 2.3 the escape does not: an identifier starts
/// with a letter, and IDL 4.2 §7.2.3.1 writes `_long` for a member whose
/// name would otherwise be an IDL keyword.
#[must_use]
pub fn component_name(name: &str) -> String {
    ada::ada_case(&ada::legal_name(&ada::escape_name(name)))
}

/// Whether a spelling from a facet is an Ada subtype mark.
///
/// A `native_mapping` is whatever the source's `#pragma` said, and the
/// same facet feeds the C column, where `void *` is the answer. A
/// spelling that is not a name would produce a spec no compiler reads,
/// so it is not used and the declaration falls back to `System.Address`.
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
///
/// Compared on [`ada::fold`] and not on the written spelling, because
/// Ada folds case (RM 2.3(5/2)): `Plain` and `plain` are one identifier,
/// so [`ada::ada_case`] moved how the name is written and not which name
/// it is, and a note saying `IDL: \`plain\`.` above a component called
/// `Plain` records nothing a reader could not read off the declaration.
/// What does need recording is a spelling that will not invert — the
/// reserved-word escape, the lexical repair, the module flattening — and
/// each of those differs after folding too.
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
    ada::fold(spelled) != ada::fold(declared)
}

/// Whether a name has already been declared in the package.
fn already(emitted: &[Declared], spelled: &str) -> bool {
    let key = ada::fold(spelled);
    emitted.iter().any(|other| other.key == key)
}

/// Record a declared name, refusing a second declaration that reaches
/// the same Ada identifier.
///
/// The whole package is one declarative region, so this is a
/// whole-package check and not a per-scope one. The comparison is on
/// [`ada::fold`] because Ada's is (RM 2.3(5/2)): `Value` and `value` are
/// one identifier, and a case-sensitive check would pass a pair GNAT
/// rejects with `"Value" conflicts with declaration at line …`, naming
/// an Ada identifier and neither of the two IDL declarations behind it.
///
/// `overloadable` is what keeps two enumeration literals of different
/// types from being refused: an enumeration literal is a parameterless
/// function (RM 3.5.1) and RM 8.3 permits two overloadable homographs.
/// A type and a literal of one name are still refused, because a type
/// declaration does not overload.
fn remember(
    emitted: &mut Vec<Declared>,
    spelled: &str,
    declared: &str,
    overloadable: bool,
) -> Result<(), MaterialiseError> {
    let key = ada::fold(spelled);
    if let Some(first) = emitted
        .iter()
        .find(|other| other.key == key && !(overloadable && other.overloadable))
    {
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
        overloadable,
    });
    Ok(())
}

/// The names one scope declares, checked for a collision after folding.
///
/// The counterpart of
/// [`no_spelling_collision`](crate::adapter::cst::no_spelling_collision)
/// for a language whose identifiers are case-insensitive. That one
/// compares the written spellings, which is right for the seven columns
/// whose languages do; here `Value` and `value` are one component name
/// and the shared function would let both through.
fn no_folded_collision(pairs: &[(String, String)], at: &str) -> Result<(), MaterialiseError> {
    let mut seen: Vec<(String, &str)> = Vec::new();
    for (spelled, declared) in pairs {
        let key = ada::fold(spelled);
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

/// The Ada expression a constant's value takes.
///
/// A `string` constant is a **call** and not a literal:
/// `Unbounded_String` is a private type (RM A.4.5) and Ada has no
/// literal for one, so the value is
/// `Ada.Strings.Unbounded.To_Unbounded_String ("…")`, evaluated when the
/// package is elaborated.
///
/// `long double` is refused by name. Its Ada holder is an
/// `Unbounded_String` carrying the decimal, and CORBA gives a
/// `long double` literal no spelling that survives a 64-bit float:
/// emitting one would put a different number in the spec than the IDL
/// wrote.
fn ada_constant(
    value: &zerodds_ast::ConstValue,
    type_: &Path,
    at: &str,
) -> Result<ada::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_text = type_ == &ada::Primitive::UnboundedString.path();
    let text = |value: &str| -> Result<ada::Expr, MaterialiseError> {
        if !value.chars().all(is_graphic_ascii) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "a string constant holding a character an Ada string literal \
                            cannot carry — RM 2.6 admits only graphic characters, and \
                            GNAT reads a source file as Latin-1 unless it is told \
                            otherwise, so a non-ASCII literal would not be the value \
                            the IDL wrote"
                    .to_string(),
                language: LANGUAGE,
                at: at.to_string(),
            });
        }
        Ok(ada::Expr::Call {
            name: Path::new(&["Ada", "Strings", "Unbounded", "To_Unbounded_String"]),
            args: vec![ada::Expr::Literal(Literal::Str(value.to_string()))],
        })
    };
    Ok(match value {
        ConstValue::Boolean(flag) => ada::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => ada::Expr::int(*number),
        // The code point as a number: an IDL `char` is one octet here
        // and an IDL `wchar` two, and an Ada character literal is a
        // value of `Standard.Character`, which is neither.
        ConstValue::Char(character) => ada::Expr::int(i128::from(u32::from(*character))),
        ConstValue::WChar(point) => ada::Expr::int(i128::from(*point)),
        ConstValue::Float { mantissa, exponent } => {
            if is_text {
                return Err(refuse("a `long double` constant, which has no Ada literal"));
            }
            ada::Expr::Literal(Literal::Float(ada_float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => text(&constant::fixed_decimal(digits, *scale))?,
        ConstValue::String(value) => text(value)?,
        ConstValue::WString(points) => text(
            &constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )?,
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a spec that disagrees with the IDL and compiles.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}

/// Whether a character is one an Ada string literal can carry.
fn is_graphic_ascii(ch: char) -> bool {
    ('\u{20}'..='\u{7e}').contains(&ch)
}

/// A floating-point literal Ada reads as a real number.
///
/// [`constant::float_text`] is written for the C family, where `1e5` is
/// a floating-point literal. Ada 2012 RM 2.4.1 makes a numeric literal
/// with no point an *integer* literal, exponent or not, and an integer
/// literal is not a value of a floating-point type — so `X : constant
/// Interfaces.IEEE_Float_64 := 1e5;` does not compile. The point is put
/// back in front of the exponent, which does not change the value.
fn ada_float_text(mantissa: &str, exponent: Option<&str>) -> String {
    let text = constant::float_text(mantissa, exponent);
    if text.contains('.') {
        return text;
    }
    match text.split_once(['e', 'E']) {
        Some((head, tail)) => format!("{head}.0e{tail}"),
        None => format!("{text}.0"),
    }
}
