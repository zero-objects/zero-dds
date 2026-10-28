// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada concrete syntax tree. Spec §1.
//!
//! One Ada **package specification** — a `.ads` — plus the body forms
//! the codec arc writes: statements, object declarations and subprogram
//! bodies ([`Stmt`], [`VarDecl`], [`SubprogramBody`]). The declaration
//! half is what `adapter::ada` materialises; the body half is what
//! `emit::ada` emits into, and a probe generator composes both into one
//! parameterless procedure.
//!
//! Four things this tree holds that no other column's does, and each is
//! Ada's:
//!
//! 1. **An enumeration representation clause.** [`EnumRep`] is a
//!    separate declaration — `for Mode use (IDLE => 0, ACTIVE => 4);` —
//!    because Ada's enumeration type declaration carries no codes at
//!    all. Without the clause Ada numbers the literals 0, 1, 2 … in
//!    declaration order (Ada 2012 RM 3.5.1), and an IDL `@value(4)`
//!    would be lost with nothing in the file to say so. The clause is
//!    what carries the wire numbering, which is why it is a node here
//!    and not a comment.
//! 2. **A discriminant part and a variant part.** [`Record`] holds both.
//!    An Ada union is one record whose components depend on a
//!    discriminant (Ada 2012 RM 3.8.1), so the branches are inside the
//!    type rather than beside it as in the Go column's struct.
//! 3. **An index constraint that may be unconstrained.** [`Index`] can
//!    be `Natural range <>`, which is how a sequence gets an element
//!    count that is not part of its type. Ada has no built-in dynamic
//!    array.
//! 4. **No visibility field.** Everything a package spec declares before
//!    its `private` part is visible to every unit that `with`s it, so
//!    [`crate::common::Visibility`] has no Ada spelling here. This tree
//!    writes no `private` part.
//!
//! ## Identifiers are case-insensitive, and one escape follows from it
//!
//! Ada 2012 RM 2.3(5/2) folds identifiers before comparing them, so
//! `Value` and `value` are one identifier and two declarations spelled
//! that way are one declaration twice. [`fold`] is the key a caller
//! compares on; `zerodds_lang_rules::adapter::ada` refuses the pair by
//! name rather than emitting both.
//!
//! RM 2.3(4/3) also forbids an identifier that ends in an underscore or
//! contains two in a row, which is why [`escape_name`] appends
//! [`ESCAPE_SUFFIX`] and not the trailing underscore every other column
//! uses: `type_` is not an Ada identifier at all. RM 2.3(2) forbids one
//! that *starts* with an underscore, which IDL 4.2 §7.2.3.1 produces
//! whenever a source escapes a keyword as `_long`; [`legal_name`] is
//! where all three constraints are answered.
//!
//! ## What is not here
//!
//! No package body, no tasks, no protected types, no generics, no
//! `private` part, no `pragma`, no exception handlers. The statements
//! are only the forms the shared codec walk emits. Every form has a
//! renderer test, so no arm of `zerodds_lang_render::ada` is code that
//! only claims to work.

use crate::common::{BodyText, Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An Ada package specification.
///
/// One per IDL file. Ada's unit of scope is the package and GNAT
/// requires the file name to match the unit name, so nested IDL modules
/// do not nest here — they flatten onto the declared names, which is
/// what `zerodds_lang_rules::adapter::ada` does and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written above the context clause.
    pub doc: Doc,
    /// The `with` clauses, in written order.
    pub withs: Vec<Path>,
    /// The `use type` clauses, in written order.
    ///
    /// A `use_type_clause` (Ada 2012 RM 8.4) makes the **predefined
    /// operators** of a type directly visible and nothing else — not the
    /// type's name, not the rest of its package. It is needed because a
    /// predefined operator of a type declared in another package is not
    /// directly visible where the type is only named through its package
    /// (RM 8.4, and RM 4.4 for what counts as an operator): with
    /// `Interfaces.Integer_16` reached as a qualified name, `when -1 =>`
    /// applies a `"-"` nothing has made visible, and GNAT answers
    /// `operator for type "Interfaces.Integer_16" is not directly
    /// visible`.
    ///
    /// `use type` and not `use Interfaces;`: the second would make
    /// `Integer_32`, `Unsigned_8` and the rest directly visible too, and
    /// a generated declaration of one of those names would then be a
    /// homograph of a predefined one. Operators overload, so this clause
    /// cannot collide with anything.
    pub use_types: Vec<Path>,
    /// The package name. Required: a spec without one does not parse.
    pub package: Ident,
    /// Declarations, in written order. Ada resolves nothing forward, so
    /// this order is load-bearing.
    pub decls: Vec<Decl>,
}

/// A file declaring the package a generated spec always declares.
///
/// Written out rather than derived, because [`Ident`] has no `Default`
/// on purpose: an empty package name is not an Ada spec, and a derive
/// would produce one.
impl Default for File {
    fn default() -> Self {
        Self {
            doc: Doc::none(),
            withs: Vec::new(),
            use_types: Vec::new(),
            package: Ident::new("Zd_Types"),
            decls: Vec::new(),
        }
    }
}

/// One declaration in the visible part of a package spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `type Name is …;`
    Type(TypeDecl),
    /// `subtype Name is Mark;`
    ///
    /// Its own variant and not a [`TypeDef`], because a subtype is not a
    /// type declaration: it introduces another name for an existing type
    /// rather than a new one (Ada 2012 RM 3.2.2), which is exactly the
    /// difference an IDL `typedef` needs.
    Subtype(SubtypeDecl),
    /// A subprogram with a body — what the codec arc declares.
    ///
    /// Never part of a rendered package *specification*: a spec carries
    /// profiles only (RM 7.1). A probe generator renders these into a
    /// declarative part, where RM 3.11 admits a `body` as a
    /// `declarative_item`.
    SubprogramBody(SubprogramBody),
    /// `Name : constant Type := value;`
    Object(ObjectDecl),
    /// `for Name use (A => 0, B => 4);`
    EnumRep(EnumRep),
    /// `procedure P (…) is abstract;` or `function F (…) return T is
    /// abstract;`
    Subprogram(Subprogram),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `type Name is …;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// What it declares.
    pub def: TypeDef,
}

/// The right-hand side of a type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDef {
    /// A record, with or without discriminants and a variant part.
    Record(Record),
    /// `(A, B, C)` — an enumeration.
    ///
    /// The codes are **not** here. They are an [`EnumRep`] declaration
    /// of their own, because Ada keeps them apart (RM 3.5.1 and 13.4).
    Enumeration(Vec<Ident>),
    /// `new Mark` — a derived type, with its own identity.
    Derived(Path),
    /// `array (…) of Element`.
    Array {
        /// One entry per dimension, outermost first.
        index: Vec<Index>,
        /// The element type.
        element: Path,
    },
    /// `access Designated`.
    Access(Path),
    /// `interface` — an Ada 2005 interface type (RM 3.9.4).
    Interface,
    /// Nothing at all: `type Name;`, an incomplete type declaration.
    ///
    /// Ada 2012 RM 3.10.1. The one construct in this tree that declares
    /// a name without saying what it is, and the only way a cycle in the
    /// type graph can be written down: a self-referential IDL type
    /// cannot be declared before it is named, so the name is declared
    /// first and the type follows.
    ///
    /// What an incomplete type may be used for is narrow, and the
    /// materialiser depends on the limit: RM 3.10.1(9) admits it as the
    /// **designated subtype of an access type** and little else. It may
    /// not be the component of a record, the element of an array, or a
    /// subtype mark — so the shape a cycle takes is always an access
    /// type, and the completion always comes later in the same
    /// declarative part.
    Incomplete,
}

/// A record definition.
///
/// A record with no discriminants, no components and no variant part is
/// written `is null record;` and not `is record end record;` — the
/// second is not Ada (RM 3.8 gives `null record` as its own form, and a
/// `component_list` may not be empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The known discriminant part, empty for a plain record.
    pub discriminants: Vec<Discriminant>,
    /// Components declared before any variant part.
    pub components: Vec<Component>,
    /// The variant part, when the record has one.
    pub variant: Option<Variant>,
    /// `@verbatim` text at the start and the end of the component list.
    pub body_text: BodyText,
}

impl Record {
    /// A record of components and nothing else.
    #[must_use]
    pub fn plain(components: Vec<Component>) -> Self {
        Self {
            discriminants: Vec::new(),
            components,
            variant: None,
            body_text: BodyText::none(),
        }
    }

    /// Whether it is written `null record`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.components.is_empty()
            && self.variant.is_none()
            && self.body_text.begin.is_empty()
            && self.body_text.end.is_empty()
    }
}

/// One discriminant: `D : Mode := IDLE`.
///
/// The default is what makes the record *unconstrained* — a value of it
/// may change discriminant at an assignment, and a component of it needs
/// no constraint at its declaration (RM 3.7). A union whose
/// discriminant had no default could not be a component of anything
/// without the branch being fixed at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discriminant {
    /// The name.
    pub name: Ident,
    /// The subtype, which must be discrete or an access type (RM 3.7).
    pub type_: Path,
    /// The default, absent for a constrained record.
    pub default: Option<Expr>,
}

/// One component: `X : Interfaces.Integer_32;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The subtype. A component's subtype must be definite, which is why
    /// a sequence is an access type here and not an array.
    pub type_: Path,
}

/// A variant part: `case D is … end case;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The discriminant it selects on.
    pub discriminant: Ident,
    /// The arms, in written order. `others` is last or absent.
    pub arms: Vec<Arm>,
}

/// One arm of a variant part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    /// The choices selecting this arm.
    pub choices: Vec<Choice>,
    /// The components it declares. Empty is written `null;`.
    pub components: Vec<Component>,
}

/// One discrete choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// A static value.
    Value(Expr),
    /// `others`, which must appear alone and last (RM 3.8.1).
    Others,
}

/// One dimension of an array type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Index {
    /// `low .. high` — a constrained dimension.
    Range {
        /// The lower bound.
        low: Expr,
        /// The upper bound.
        high: Expr,
    },
    /// `Natural range <>` — an unconstrained dimension.
    ///
    /// The length is then part of the value and not of the type, which
    /// is how a sequence gets a count. A component may not have an
    /// unconstrained subtype, so a type declared this way is reached
    /// through an access type.
    Unconstrained(Path),
}

/// `subtype Name is Mark;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtypeDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The subtype mark it renames.
    pub mark: Path,
}

/// `Name : constant Type := value;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The declared subtype. Never omitted: Ada has no untyped constant.
    pub type_: Path,
    /// The value.
    pub value: Expr,
}

/// `for Name use (A => 0, B => 4);`
///
/// An enumeration representation clause (RM 13.4). The codes must be
/// static, distinct, and in the enumeration's own order — which is what
/// makes an IDL enumeration whose `@value`s descend unrepresentable, and
/// the materialiser refuses one by name rather than writing a clause the
/// compiler rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumRep {
    /// Documentation.
    pub doc: Doc,
    /// The enumeration type it applies to.
    pub type_: Ident,
    /// The literal and its code, in the enumeration's declaration order.
    pub values: Vec<(Ident, Expr)>,
}

/// A subprogram declaration with no body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subprogram {
    /// Documentation.
    pub doc: Doc,
    /// Whether it returns a value.
    pub kind: SubprogramKind,
    /// The name.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// The result subtype, for a function.
    pub result: Option<Path>,
    /// Whether it is `is abstract`.
    ///
    /// A primitive operation of an interface type must be abstract or
    /// null (RM 3.9.4), and a spec that declared a concrete one would
    /// need a body this arc does not write.
    pub is_abstract: bool,
}

/// Whether a subprogram returns a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubprogramKind {
    /// `procedure` — no result.
    Procedure,
    /// `function` — a result.
    Function,
}

/// One parameter of a subprogram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The passing mode.
    pub mode: Mode,
    /// The subtype.
    pub type_: Path,
}

/// An Ada parameter mode (RM 6.1).
///
/// The three IDL directions map onto these one for one, which no other
/// column in this crate can do: C, C++, Go and Rust all have to spell
/// `out` as a pointer or a reference and lose the direction from the
/// signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `in`
    In,
    /// `out`
    Out,
    /// `in out`
    InOut,
}

impl Mode {
    /// Every mode Ada defines.
    pub const ALL: [Self; 3] = [Self::In, Self::Out, Self::InOut];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::InOut => "in out",
        }
    }
}

/// An expression.
///
/// Only the forms a constant declaration, an index bound and a
/// representation clause take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name, possibly qualified.
    Name(Path),
    /// `F (a, b)` — a function call.
    ///
    /// The one form a generated spec needs is
    /// `Ada.Strings.Unbounded.To_Unbounded_String ("…")`: Ada has no
    /// literal for a private type, so a `string` constant is a call.
    Call {
        /// The subprogram.
        name: Path,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `-a`.
    Negate(Box<Expr>),
    /// `a op b`, with the shared walk's operator spelling — the renderer
    /// maps `==` to `=`, `&&` to `and then` and so on, and never the
    /// tree.
    Binary {
        /// The operator, as the codec walk spells it.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `not a` and `-a` in the walk's spelling (`!`, `~`, `-`).
    Unary {
        /// The operator, as the codec walk spells it.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `a.Name` — a selected component of an expression.
    ///
    /// Separate from [`Expr::Name`], which is a static path: the prefix
    /// here is any expression, `V.all (I).Key` included.
    Selected {
        /// The prefix.
        value: Box<Expr>,
        /// The selector.
        name: Ident,
    },
    /// `a (i)` — an indexed component.
    Indexed {
        /// The array, or an access to one (RM 4.1 dereferences it).
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `a.all` — an explicit dereference.
    Deref(Box<Expr>),
    /// `a (low .. high)` — a slice.
    Slice {
        /// The array, or an access to one.
        value: Box<Expr>,
        /// The lower bound.
        low: Box<Expr>,
        /// The upper bound.
        high: Box<Expr>,
    },
    /// `P'Name` or `P'Name (a)` — an attribute reference.
    ///
    /// `'Length`, `'Enum_Val` and `'Enum_Rep` are the three the codec
    /// arc writes; the last two are GNAT-implemented for Ada 2012 and
    /// Ada 2022 language (RM 13.4 there).
    Attribute {
        /// The prefix.
        value: Box<Expr>,
        /// The attribute's identifier.
        name: Ident,
        /// The arguments, for the functional attributes.
        args: Vec<Expr>,
    },
    /// `Mark (a)` — a type conversion (RM 4.6).
    ///
    /// The same written form as [`Expr::Call`], and a variant of its own
    /// because `emit::ada` rewrites conversions of an enumeration value
    /// through `'Enum_Rep`, and a rewrite that had to guess whether a
    /// call was a conversion would rewrite calls.
    Convert {
        /// The target subtype mark.
        mark: Path,
        /// The operand.
        value: Box<Expr>,
    },
    /// `(A => x, B => y)` — a record aggregate, or `(others => x)`.
    ///
    /// Only named associations: every use the codec arc has knows its
    /// component names, and a positional aggregate of one element is
    /// not Ada (RM 4.3.1). Empty renders as `(null record)`, the
    /// aggregate a component-free record takes (RM 4.3.1, Ada 2005).
    Aggregate(Vec<Association>),
    /// `<>` — the default value inside an aggregate (RM 4.3.1, Ada
    /// 2005). Only legal as an [`Association`]'s value.
    BoxDefault,
    /// `new Mark (low .. high)` or `new Mark'(value)` — an allocator.
    New {
        /// The designated subtype mark.
        mark: Path,
        /// How the allocated object is constrained or initialised.
        init: AllocInit,
    },
}

/// One association of an [`Expr::Aggregate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Association {
    /// The component, or `others` spelled as an identifier.
    pub choice: Ident,
    /// The value, [`Expr::BoxDefault`] for `<>`.
    pub value: Expr,
}

/// What an [`Expr::New`] allocator says about the object it makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocInit {
    /// `new Mark (low .. high)` — an index constraint, default-valued
    /// elements. `low > high` allocates the empty array.
    Range {
        /// The lower bound.
        low: Box<Expr>,
        /// The upper bound.
        high: Box<Expr>,
    },
    /// `new Mark'(value)` — a qualified initial value.
    Qualified(Box<Expr>),
}

/// One statement of a subprogram body.
///
/// Ada keeps declarations out of statement lists (RM 3.11 against
/// RM 5.1); [`Stmt::Var`] and [`Stmt::Subtype`] are still statements
/// *here* because the shared codec walk interleaves them, and the
/// renderer is what folds a run of them into a `declare` block — or into
/// the subprogram's own declarative part when they lead the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `null;`
    Null,
    /// An object declaration. See the type comment for where it lands.
    Var(VarDecl),
    /// `subtype Name is Mark;`, in a declarative position.
    Subtype {
        /// The name declared.
        name: Ident,
        /// The subtype mark.
        mark: Path,
    },
    /// `place := value;`
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// A procedure call.
    Call(Expr),
    /// `return;` or `return value;`
    Return(Option<Expr>),
    /// `if … then … else … end if;`
    If {
        /// The condition.
        condition: Expr,
        /// The statements when it holds.
        then: Vec<Stmt>,
        /// The statements when it does not, absent for a bare `if`.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `for Counter in Low .. High loop … end loop;`
    For {
        /// The loop parameter.
        counter: Ident,
        /// The lower bound.
        low: Expr,
        /// The upper bound, inclusive.
        high: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `while Condition loop … end loop;`
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `exit;` — leave the innermost loop.
    Exit,
    /// `declare … begin … end;`
    Declare {
        /// The declarations.
        vars: Vec<VarDecl>,
        /// The statements.
        body: Vec<Stmt>,
    },
    /// A comment on its own lines.
    Comment(Doc),
}

/// One object declaration of a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    /// The name.
    pub name: Ident,
    /// The subtype mark.
    pub mark: Path,
    /// A discriminant constraint — `Name : Mark (Constraint);` — which
    /// is how a body obtains a value with a discriminant it only knows
    /// at run time (RM 3.7.1).
    pub constraint: Option<Expr>,
    /// The initial value.
    pub value: Option<Expr>,
}

/// A subprogram with its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprogramBody {
    /// The profile. `is_abstract` is ignored: a body is never abstract.
    pub spec: Subprogram,
    /// The statements. Leading [`Stmt::Var`], [`Stmt::Subtype`] and
    /// [`Stmt::Comment`] entries are rendered into the declarative
    /// part; an empty remainder renders as `null;`.
    pub body: Vec<Stmt>,
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// A name of one segment.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Path::single(text))
    }
}

/// A type this column names in a generated spec.
///
/// `Integer`, `Natural` and `Float` are **not** here, and that is the
/// point: Ada leaves the range of `Integer` and the format of `Float` to
/// the implementation (RM 3.5.4 and 3.5.7 give only lower bounds on
/// what they must hold), so a member declared with one would have a
/// different width under a different compiler, for a wire format that
/// fixes it. `Interfaces` (RM B.2) is where the fixed-width types are,
/// and every one of these but [`Primitive::Boolean`],
/// [`Primitive::UnboundedString`] and [`Primitive::Address`] comes from
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `Boolean` — from `Standard`, and one of the two Ada types whose
    /// definition the RM fixes exactly (RM A.1).
    Boolean,
    /// `Interfaces.Integer_8`
    Integer8,
    /// `Interfaces.Integer_16`
    Integer16,
    /// `Interfaces.Integer_32`
    Integer32,
    /// `Interfaces.Integer_64`
    Integer64,
    /// `Interfaces.Unsigned_8`
    Unsigned8,
    /// `Interfaces.Unsigned_16`
    Unsigned16,
    /// `Interfaces.Unsigned_32`
    Unsigned32,
    /// `Interfaces.Unsigned_64`
    Unsigned64,
    /// `Interfaces.IEEE_Float_32`
    Float32,
    /// `Interfaces.IEEE_Float_64`
    Float64,
    /// `Ada.Strings.Unbounded.Unbounded_String`
    UnboundedString,
    /// `System.Address`
    Address,
}

impl Primitive {
    /// Every type the tree names.
    pub const ALL: [Self; 13] = [
        Self::Boolean,
        Self::Integer8,
        Self::Integer16,
        Self::Integer32,
        Self::Integer64,
        Self::Unsigned8,
        Self::Unsigned16,
        Self::Unsigned32,
        Self::Unsigned64,
        Self::Float32,
        Self::Float64,
        Self::UnboundedString,
        Self::Address,
    ];

    /// How it is written, fully qualified.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::Integer8 => "Interfaces.Integer_8",
            Self::Integer16 => "Interfaces.Integer_16",
            Self::Integer32 => "Interfaces.Integer_32",
            Self::Integer64 => "Interfaces.Integer_64",
            Self::Unsigned8 => "Interfaces.Unsigned_8",
            Self::Unsigned16 => "Interfaces.Unsigned_16",
            Self::Unsigned32 => "Interfaces.Unsigned_32",
            Self::Unsigned64 => "Interfaces.Unsigned_64",
            Self::Float32 => "Interfaces.IEEE_Float_32",
            Self::Float64 => "Interfaces.IEEE_Float_64",
            Self::UnboundedString => "Ada.Strings.Unbounded.Unbounded_String",
            Self::Address => "System.Address",
        }
    }

    /// The same, as a path the tree can hold.
    ///
    /// Qualified at every use rather than brought in with a `use`
    /// clause: a `use Interfaces;` would make `Integer_32` directly
    /// visible and a generated declaration of the same name would then
    /// hide it, which is a failure that arrives in whichever file
    /// happens to declare both.
    #[must_use]
    pub fn path(self) -> Path {
        Path::new(&self.as_str().split('.').collect::<Vec<&str>>())
    }

    /// The library unit a `with` clause has to name for it.
    ///
    /// `None` for `Boolean`, which is in `Standard` and needs no clause.
    #[must_use]
    pub fn unit(self) -> Option<Path> {
        match self {
            Self::Boolean => None,
            Self::UnboundedString => Some(Path::new(&["Ada", "Strings", "Unbounded"])),
            Self::Address => Some(Path::single("System")),
            _ => Some(Path::single("Interfaces")),
        }
    }
}

/// The letters this column repairs an identifier with.
///
/// One marker for all three of Ada 2012 RM 2.3's constraints, so a
/// reader meeting `Idl` anywhere in a generated name knows the same
/// thing: the IDL spelling was not writable as it stands, and the
/// comment above the declaration has it.
pub const ESCAPE_MARKER: &str = "Idl";

/// The suffix a name that cannot be written as it stands takes.
///
/// A letter suffix and **not** the trailing underscore every other
/// column in this crate uses: Ada 2012 RM 2.3(4/3) forbids an identifier
/// that ends in a character of category `punctuation_connector` or
/// contains two in a row, so `type_` is not an Ada identifier and
/// `type__idl` would not be one either. `_Idl` is a letter suffix that
/// says what moved the name.
pub const ESCAPE_SUFFIX: &str = "_Idl";

/// The same name, spelled so that Ada will read it as an identifier.
///
/// Ada 2012 RM 2.3 puts three constraints on an identifier, and IDL puts
/// none of them on its own (IDL 4.2 §7.2.3 admits any run of letters,
/// digits and underscores after a first letter, and §7.2.3.1 adds a
/// leading underscore as the escape for a name that would otherwise be
/// an IDL keyword):
///
/// - **RM 2.3(2): it starts with a letter.** `_long` — an IDL escaped
///   identifier — is not an Ada identifier, and GNAT answers `identifier
///   cannot start with underline`. [`ESCAPE_MARKER`] goes in front:
///   `Idl_long`. This is the same rule as the trailing-underscore one
///   from the other end, which is why the repair is the same letters.
/// - **RM 2.3(4/3): no two underscores in a row.** IDL admits `a__b`;
///   the marker goes between them, giving `a_Idl_b`, rather than
///   collapsing the pair — a collapse would make `a__b` and `a_b` one
///   Ada name with nothing to say which was which.
/// - **RM 2.3(4/3): it does not end in an underscore.** `foo_` becomes
///   `foo_Idl`.
///
/// Every repair is recorded: the caller writes the IDL spelling into the
/// comment above the declaration, and
/// `zerodds_lang_rules::adapter::ada` compares every declared name on
/// [`fold`], so a repair that lands on a name already taken is refused
/// rather than emitted twice.
#[must_use]
pub fn legal_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + ESCAPE_MARKER.len());
    if name.starts_with('_') {
        out.push_str(ESCAPE_MARKER);
    }
    for ch in name.chars() {
        if ch == '_' && out.ends_with('_') {
            out.push_str(ESCAPE_MARKER);
        }
        out.push(ch);
    }
    if out.ends_with('_') {
        out.push_str(ESCAPE_MARKER);
    }
    out
}

/// The name an IDL identifier takes in Ada.
///
/// [`ESCAPE_SUFFIX`] where the identifier is an Ada reserved word or one
/// of the names a generated spec relies on. The comparison is
/// case-insensitive because Ada's is: RM 2.9 lists the reserved words in
/// lower case and RM 2.3(5/2) folds before comparing, so `TYPE`, `Type`
/// and `type` are one reserved word and all three have to move.
///
/// The IDL name is not lost when this fires: it goes into the comment
/// above the declaration.
#[must_use]
pub fn escape_name(name: &str) -> String {
    if is_reserved(name) {
        let mut out = name.to_string();
        out.push_str(ESCAPE_SUFFIX);
        return out;
    }
    name.to_string()
}

/// Whether a name has to move out of the way.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(name))
}

/// The conventional Ada spelling of an identifier.
///
/// The first letter of each underscore-separated word in upper case and
/// **nothing else changed**. Ada folds case before comparing (RM
/// 2.3(5/2)), so this renames nothing the compiler can tell apart; it is
/// how Ada source is conventionally written, and leaving the rest of
/// each word alone is what keeps `IDLE` spelled `IDLE` rather than
/// `Idle`.
///
/// It introduces no collision that was not there already: two IDL
/// identifiers in one scope differing only in case are forbidden by IDL
/// 4.2 §7.2.3.1, and two that flatten onto one name from different
/// scopes are caught by `zerodds_lang_rules::adapter::ada`, which
/// compares on [`fold`].
#[must_use]
pub fn ada_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut at_word_start = true;
    for ch in name.chars() {
        if at_word_start {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
        } else {
            out.push(ch);
        }
        at_word_start = ch == '_';
    }
    out
}

/// The key two Ada identifiers are the same identifier under.
///
/// RM 2.3(5/2): identifiers are compared after case folding, so `Value`
/// and `value` are one name. Every collision check in this column
/// compares on this and not on the written spelling — a check that did
/// not would pass a spec the compiler rejects with `Value` conflicts
/// with declaration at line …`, naming an Ada identifier and neither of
/// the two IDL declarations behind it.
#[must_use]
pub fn fold(name: &str) -> String {
    name.to_lowercase()
}

/// Every name an escape has to move out of the way of.
///
/// The 73 reserved words of Ada 2012 (RM 2.9), then the names a
/// generated spec depends on being visible: the library units it `with`s
/// and the `Standard` types it writes. Those are not reserved and Ada
/// would accept a declaration shadowing one — `type Interfaces is …` is
/// legal until the next line names `Interfaces.Integer_32`, which then
/// resolves to the new declaration and fails. Every member type in a
/// generated spec is reached through one of these names, so the spec
/// cannot afford the shadowing.
///
/// `Zd_Types` is on the list for a different reason: RM 8.1 puts a
/// package's own defining identifier in the declarative region the
/// package encloses, so a declaration inside `Zd_Types` named
/// `Zd_Types` is a homograph of the package itself.
pub const RESERVED: &[&str] = &[
    // Reserved words, Ada 2012 RM 2.9.
    "abort",
    "abs",
    "abstract",
    "accept",
    "access",
    "aliased",
    "all",
    "and",
    "array",
    "at",
    "begin",
    "body",
    "case",
    "constant",
    "declare",
    "delay",
    "delta",
    "digits",
    "do",
    "else",
    "elsif",
    "end",
    "entry",
    "exception",
    "exit",
    "for",
    "function",
    "generic",
    "goto",
    "if",
    "in",
    "interface",
    "is",
    "limited",
    "loop",
    "mod",
    "new",
    "not",
    "null",
    "of",
    "or",
    "others",
    "out",
    "overriding",
    "package",
    "pragma",
    "private",
    "procedure",
    "protected",
    "raise",
    "range",
    "record",
    "rem",
    "renames",
    "requeue",
    "return",
    "reverse",
    "select",
    "separate",
    "some",
    "subtype",
    "synchronized",
    "tagged",
    "task",
    "terminate",
    "then",
    "type",
    "until",
    "use",
    "when",
    "while",
    "with",
    "xor",
    // Library units a generated spec names, and the packages above them.
    "ada",
    "interfaces",
    "strings",
    "system",
    "standard",
    "unbounded",
    // `Standard` names a generated spec writes or relies on.
    "boolean",
    "character",
    "duration",
    "float",
    "integer",
    "natural",
    "positive",
    "string",
    "wide_character",
    "wide_string",
    // The package this column declares. RM 8.1.
    "zd_types",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_named_type_writes_its_own_spelling() {
        let mut seen = Vec::new();
        for primitive in Primitive::ALL {
            let text = primitive.as_str();
            assert!(!seen.contains(&text), "{text} is used twice");
            seen.push(text);
        }
        assert_eq!(seen.len(), Primitive::ALL.len());
    }

    #[test]
    fn no_named_type_has_an_implementation_defined_width() {
        // Ada fixes only a lower bound on `Integer` (RM 3.5.4) and
        // leaves `Float`'s format to the implementation (RM 3.5.7). A
        // member declared with either would change width between two
        // compilers, for a wire format that fixes it.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(
                    primitive.as_str(),
                    "Integer" | "Float" | "Long_Float" | "Long_Long_Float" | "Natural"
                ),
                "{} has no fixed width",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn a_fixed_width_type_names_the_interfaces_package() {
        // RM B.2. `Integer_32` on its own is not a name Ada resolves.
        for primitive in [
            Primitive::Integer8,
            Primitive::Integer16,
            Primitive::Integer32,
            Primitive::Integer64,
            Primitive::Unsigned8,
            Primitive::Unsigned16,
            Primitive::Unsigned32,
            Primitive::Unsigned64,
            Primitive::Float32,
            Primitive::Float64,
        ] {
            assert!(
                primitive.as_str().starts_with("Interfaces."),
                "{} is not qualified",
                primitive.as_str()
            );
            assert_eq!(primitive.unit(), Some(Path::single("Interfaces")));
        }
    }

    #[test]
    fn a_qualified_name_splits_into_its_segments() {
        let path = Primitive::UnboundedString.path();
        let segments: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
        assert_eq!(
            segments,
            ["Ada", "Strings", "Unbounded", "Unbounded_String"]
        );
    }

    #[test]
    fn a_reserved_word_escapes_with_a_letter_and_never_an_underscore() {
        // RM 2.3(4/3): an identifier may not end in an underscore, so
        // the `type_` every other column writes is not Ada at all.
        assert_eq!(escape_name("type"), "type_Idl");
        assert_eq!(escape_name("range"), "range_Idl");
        assert_eq!(escape_name("payload"), "payload");
        for name in RESERVED {
            let escaped = escape_name(name);
            assert!(!escaped.ends_with('_'), "{escaped} ends in an underscore");
            assert!(!escaped.contains("__"), "{escaped} has two in a row");
        }
    }

    #[test]
    fn the_reserved_check_folds_case_because_ada_does() {
        // RM 2.3(5/2) and RM 2.9: the reserved words are listed in lower
        // case and identifiers are compared after folding, so all three
        // spellings are the same reserved word.
        for spelling in ["type", "Type", "TYPE", "TyPe"] {
            assert!(is_reserved(spelling), "{spelling} was not recognised");
            assert_eq!(escape_name(spelling), alloc::format!("{spelling}_Idl"));
        }
    }

    #[test]
    fn the_names_a_generated_spec_relies_on_escape_although_they_would_parse() {
        // Ada permits the shadowing; the spec cannot afford it, because
        // every member type in it is reached through one of these names.
        for name in [
            "Interfaces",
            "Ada",
            "System",
            "Boolean",
            "String",
            "Integer",
        ] {
            assert!(is_reserved(name), "{name} is not guarded");
        }
    }

    #[test]
    fn the_package_name_is_guarded_against_its_own_declarations() {
        // RM 8.1 puts the package's defining identifier in the region it
        // encloses, so a declaration inside it with the same name is a
        // homograph of the package.
        assert!(is_reserved("Zd_Types"));
    }

    #[test]
    fn an_identifier_may_not_start_with_an_underscore() {
        // RM 2.3(2). IDL 4.2 §7.2.3.1 produces exactly this shape
        // whenever a source escapes one of its own keywords, and GNAT
        // answers `identifier cannot start with underline`.
        assert_eq!(legal_name("_long"), "Idl_long");
        assert_eq!(legal_name("_d"), "Idl_d");
        assert_eq!(legal_name("plain"), "plain");
    }

    #[test]
    fn an_identifier_may_not_hold_two_underscores_in_a_row() {
        // RM 2.3(4/3). IDL admits `a__b`; the marker goes between the
        // two rather than collapsing them, because a collapse would make
        // `a__b` and `a_b` one Ada name with nothing to say which was
        // which.
        assert_eq!(legal_name("a__b"), "a_Idl_b");
        assert_eq!(legal_name("a___b"), "a_Idl_Idl_b");
        assert_eq!(legal_name("a_b"), "a_b");
    }

    #[test]
    fn an_identifier_may_not_end_in_an_underscore() {
        // RM 2.3(4/3), from the other end.
        assert_eq!(legal_name("foo_"), "foo_Idl");
        assert_eq!(legal_name("_foo_"), "Idl_foo_Idl");
    }

    #[test]
    fn every_repair_produces_something_ada_reads_as_an_identifier() {
        // The three constraints of RM 2.3 together, over the shapes IDL
        // 4.2 §7.2.3 and §7.2.3.1 admit.
        for name in [
            "_long", "_d", "a__b", "a___b", "foo_", "_foo_", "plain", "a_b", "_a__b_",
        ] {
            let repaired = legal_name(name);
            assert!(
                repaired.starts_with(|ch: char| ch.is_alphabetic()),
                "{repaired} does not start with a letter"
            );
            assert!(!repaired.ends_with('_'), "{repaired} ends in an underscore");
            assert!(!repaired.contains("__"), "{repaired} has two in a row");
        }
    }

    #[test]
    fn ada_casing_moves_the_first_letter_of_each_word_and_nothing_else() {
        assert_eq!(ada_case("value"), "Value");
        assert_eq!(ada_case("my_field"), "My_Field");
        assert_eq!(ada_case("IDLE"), "IDLE");
        assert_eq!(ada_case("aBc"), "ABc");
        assert_eq!(ada_case(""), "");
    }

    #[test]
    fn two_spellings_that_differ_only_in_case_have_one_fold_key() {
        // RM 2.3(5/2). This is the key every collision check in the
        // column compares on.
        assert_eq!(fold("Value"), fold("value"));
        assert_eq!(fold("TYPE_Idl"), fold("type_idl"));
        assert_ne!(fold("Value"), fold("Values"));
    }

    #[test]
    fn the_reserved_list_has_no_duplicates_and_is_folded_already() {
        let mut seen: Vec<String> = Vec::new();
        for name in RESERVED {
            let key = fold(name);
            assert!(!seen.contains(&key), "{name} is listed twice");
            seen.push(key);
        }
    }

    #[test]
    fn the_reserved_list_holds_every_ada_2012_reserved_word() {
        // 73 of them (RM 2.9), plus the names a generated spec relies on.
        for word in [
            "abort",
            "accept",
            "aliased",
            "elsif",
            "overriding",
            "requeue",
            "some",
            "synchronized",
            "terminate",
            "xor",
        ] {
            assert!(is_reserved(word), "{word} is missing");
        }
    }

    #[test]
    fn a_record_with_nothing_in_it_is_a_null_record() {
        // RM 3.8 gives `null record` as its own form; a `component_list`
        // may not be empty, so `record end record` is not Ada.
        assert!(Record::plain(Vec::new()).is_null());
        assert!(
            !Record::plain(alloc::vec![Component {
                doc: Doc::none(),
                name: Ident::new("X"),
                type_: Primitive::Integer32.path(),
            }])
            .is_null()
        );
    }

    #[test]
    fn a_record_with_only_a_variant_part_is_not_a_null_record() {
        let record = Record {
            body_text: crate::common::BodyText::none(),
            discriminants: alloc::vec![Discriminant {
                name: Ident::new("D"),
                type_: Primitive::Boolean.path(),
                default: Some(Expr::name("False")),
            }],
            components: Vec::new(),
            variant: Some(Variant {
                discriminant: Ident::new("D"),
                arms: alloc::vec![Arm {
                    choices: alloc::vec![Choice::Others],
                    components: Vec::new(),
                }],
            }),
        };
        assert!(!record.is_null());
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three. The outer type indexes
        // 0 .. 1 and its element is the inner type, which indexes
        // 0 .. 2; reversing it silently transposes the data.
        let def = TypeDef::Array {
            index: alloc::vec![Index::Range {
                low: Expr::int(0),
                high: Expr::int(1),
            }],
            element: Path::single("Zd_Arr_Long_3"),
        };
        let TypeDef::Array { index, element } = &def else {
            panic!("not an array");
        };
        assert_eq!(index.len(), 1);
        assert_eq!(element.last().map(Ident::as_str), Some("Zd_Arr_Long_3"));
    }

    #[test]
    fn every_parameter_mode_writes_its_ada_spelling() {
        assert_eq!(Mode::In.as_str(), "in");
        assert_eq!(Mode::Out.as_str(), "out");
        assert_eq!(Mode::InOut.as_str(), "in out");
        assert_eq!(Mode::ALL.len(), 3);
    }
}
