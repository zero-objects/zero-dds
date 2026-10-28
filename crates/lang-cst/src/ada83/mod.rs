// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada 83 concrete syntax tree. Spec §1.
//!
//! `crates/lang-cst/src/ada/mod.rs` is the closer relative than any of
//! this crate's other trees, and most of its shapes are literally the
//! same shapes: RM83 already has discriminated records with a variant
//! part (§3.7.3), unconstrained array types reached through an access
//! type (§3.8), incomplete type declarations (§3.10.1), qualified
//! expressions and allocators (§4.7, §4.8). Where this tree differs, it
//! is because ANSI/MIL-STD-1815A does not have something Ada 2012 does,
//! and each difference below is that, not a stylistic choice.
//!
//! # What ANSI/MIL-STD-1815A does not admit, and how this column answers
//!
//! **No tagged types, no interfaces, no `abstract`.** RM83 has no
//! Chapter on tagged types at all — `tagged`, `interface` and
//! `abstract` are Ada 95 and Ada 2005 words. `zerodds_lang_rules::adapter::ada83`
//! therefore has no `TypeDef::Interface` to produce: an IDL
//! `interface` is refused by name before this tree is ever built for
//! it, which is why this enum has no such variant and [`Subprogram`]
//! carries no `is_abstract` flag. A `struct` that inherits is not this
//! problem — `all_members_of` already flattens a base's members into
//! the derived record in the Ada 2012 column too, and this tree needs
//! nothing new for it.
//!
//! **No enumeration type at all, for the codec's purposes.** RM83 3.5.1
//! gives an enumeration type real literals, and this tree could
//! declare one. What Ada 2012 needs beside it — a representation
//! clause, plus `'Enum_Val` and `'Enum_Rep` to cross between the
//! literal and its wire code — is where the difference lands:
//! `'Enum_Rep`/`'Enum_Val` are GNAT-implemented attributes documented
//! against Ada 2012 and Ada 2022, and nothing in this repository's
//! reach confirms they answer under `-gnat83` rather than refusing the
//! attribute outright. Rather than gate a whole construct family on an
//! unverified compiler behaviour, `adapter::ada83` materialises an IDL
//! `enum` exactly as it would a set of named integer constants — the
//! same answer C gives — and this tree carries [`Decl::Object`] for
//! each enumerator and nothing else. No `TypeDef::Enumeration`, no
//! `EnumRep` node.
//!
//! **No `Ada.Strings.Unbounded`, no `Ada.Containers`.** Both are
//! library units this tree cannot name at all: Ada 83's I/O and string
//! packages are not children of `Ada` — that reorganisation is Ada 95's
//! (`with Text_IO;`, never `with Ada.Text_IO;`) — and `Ada.Containers`
//! does not exist before Ada 2005. A `string` member becomes, like a
//! `sequence`, an access to an unconstrained array — here of
//! `Character` — through [`TypeDef::Access`] over [`Index::Unconstrained`],
//! exactly the shape a `sequence<octet>` already takes. A `map<K, V>`
//! is what DDS-XTypes 1.3 §7.4.1.1.4 already says it is on the wire — an
//! ordered sequence of entries — so it costs this tree nothing beyond
//! what a sequence already needs.
//!
//! **No modular type.** RM83 3.5.4 gives every integer type a signed
//! range; there is no `mod` type definition until Ada 95. Every
//! integer-holding scalar this tree names is `Long_Integer`, and the
//! wire's unsigned reading of it — the two's-complement wrap `Long_Integer`
//! itself cannot express, and the truncation to a member's own width
//! — is `runtime/ada-83`'s arithmetic, the same `mod`/`/` construction
//! `endpoints/ada-83/src/zerodds_ada83_wire.adb` already carries. One
//! width has no full-range single-scalar form at all: an IDL
//! `unsigned long long`'s top half needs 64 unsigned bits and no
//! `Long_Integer` GNAT builds under `-gnat83` reaches past `2**63 - 1`.
//! This tree names the member `Long_Integer` regardless — the type has
//! a form — and `runtime/ada-83` refuses a *value* past the range at
//! the wire boundary, which is `docs/specs/zerodds-verdicts-1.0.md`'s
//! exit 1 and not this tree's exit 3.
//!
//! **No `Interfaces`.** Annex B, and everything in it, is Ada 95. There
//! is consequently no `Shift_Left`/`Shift_Right`/`and`/`or` on an
//! integer type either — those come from `Interfaces` or from a modular
//! type's own operators, and this tree has neither. Every packing
//! `runtime/ada-83` does — a bitset's fields into their anchor, a
//! union overlay's pattern, a `wstring` unit's byte order — is written
//! in `+`, `*`, `/` and `mod`, RM83 4.5's own operators, over
//! `Long_Integer`. No CST difference follows from this: it is entirely
//! `runtime/ada-83`'s arithmetic and never a shape this tree has to
//! carry.
//!
//! **No `raise … with "message"`.** RM83 11.3 admits only a bare
//! `raise` of an exception's name — the string-carrying form is an Ada
//! 2005 addition. `runtime/ada-83` therefore raises one of several
//! purpose-named exceptions rather than one `Wire_Error` with formatted
//! text, exactly as `endpoints/ada-83/src/zerodds_ada83_wire.ads`
//! already does. Nothing in this tree names an exception at all — a
//! `raise` is not a form the shared codec walk emits — so this is a
//! runtime-only difference and not a CST one; it is recorded here
//! because it explains why `runtime/ada-83`'s exception set does not
//! match `runtime/ada`'s one `Wire_Error`.
//!
//! **No `<>` inside an aggregate.** The box notation for "the default
//! value here" is an Ada 2005 addition (AI95-287); RM83 4.3.1's
//! aggregate needs an explicit association for every component that
//! exists. This tree therefore has no `BoxDefault` expression, unlike
//! `zerodds_lang_cst::ada::Expr`. A union's selected arm with zero
//! components needs no association at all — the discriminant alone is
//! already a non-empty aggregate, and RM83's rule is satisfied by
//! association count, not by a token that says "nothing more". A
//! record with zero components *and* no discriminant is the one shape
//! this loses outright: RM83 4.3.1 requires at least one
//! `component_association`, and `(null record)` — the literal spelling
//! Ada 2005 (AI95-287) added for exactly this case — is not available
//! either. `tools/wireprobe/wireprobe-zerodds-ada83-gen` answers by not
//! writing a value for a sample of such a type at all: a type with no
//! components has exactly one value, and an uninitialised declaration
//! of it already denotes it.
//!
//! **No `Ada.Command_Line`, no `Ada.Exceptions`, no
//! `Ada.Streams.Stream_IO`, no `Ada.Text_IO.Text_Streams`.** All four
//! are child units and none of this tree's declarations need any of
//! them — they matter only to the probe's harness, assembled by
//! `tools/wireprobe/wireprobe-zerodds-ada83-gen` and not by this crate,
//! through `Text_IO`, `Sequential_IO` and a `pragma Import (C, …)` of
//! the C library's `exit`. Recorded here because it is the reason this
//! tree, unlike `zerodds_lang_cst::ada`, is never asked to render a
//! full compilation unit's exception handler.

use crate::common::{BodyText, Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An Ada 83 package specification.
///
/// One per IDL file, exactly as `zerodds_lang_cst::ada::File` is —
/// nested IDL modules flatten onto the declared names for the same
/// reason: GNAT derives the file name from the unit name, and there are
/// no child units here to hold a module in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written above the context clause.
    pub doc: Doc,
    /// The `with` clauses, in written order. Unlike the Ada 2012 tree,
    /// this never needs a `use type` clause: every scalar this tree
    /// names — `Boolean`, `Long_Integer`, `Float`, `Long_Float` — is
    /// declared in `Standard`, whose predefined operators are directly
    /// visible everywhere without one (RM83 8.2), and `Zd_Wire83.Text`'s
    /// operators are never applied outside a call this tree's own
    /// package supplies. RM 8.4's problem — the one
    /// `zerodds_lang_cst::ada` exists to answer — never arises.
    pub withs: Vec<Path>,
    /// The package name. Required: a spec without one does not parse.
    pub package: Ident,
    /// Declarations, in written order. RM83 resolves nothing forward,
    /// so this order is load-bearing exactly as it is in the Ada 2012
    /// tree.
    pub decls: Vec<Decl>,
}

impl Default for File {
    fn default() -> Self {
        Self {
            doc: Doc::none(),
            withs: Vec::new(),
            package: Ident::new("Zd83_Types"),
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
    Subtype(SubtypeDecl),
    /// A subprogram with a body — what the codec arc declares.
    SubprogramBody(SubprogramBody),
    /// `Name : constant Type := value;` — a constant, and also the
    /// spelling an IDL enumerator and a bitmask bit take here, since
    /// this tree has no enumeration type of its own. See the module
    /// documentation.
    Object(ObjectDecl),
    /// A forward subprogram declaration — a profile with no body, for
    /// the mutually recursive pair RM83 3.9(6)/3.11 needs both specs of
    /// ahead of both bodies.
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
    /// `new Mark` — a derived type, with its own identity. RM83 3.4.
    /// Ordinary here — a derived type needs no tagged-ness — and used
    /// only for an IDL `native` declaration, over `System.Address`.
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
    /// Nothing at all: `type Name;`, an incomplete type declaration
    /// (RM83 3.10.1) — the same clause number as the Ada 2012 tree
    /// relies on, unchanged since 1983.
    Incomplete,
}

/// A record definition.
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

/// One discriminant: `D : Long_Integer := 0`. RM83 3.7 already makes a
/// default the way a discriminated record becomes unconstrained; this
/// is unchanged from the Ada 2012 tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discriminant {
    /// The name.
    pub name: Ident,
    /// The subtype, which must be discrete or an access type (RM83 3.7).
    pub type_: Path,
    /// The default, absent for a constrained record.
    pub default: Option<Expr>,
}

/// One component: `X : Long_Integer;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The subtype.
    pub type_: Path,
}

/// A variant part: `case D is … end case;` (RM83 3.7.3).
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
    /// `others`, which must appear alone and last (RM83 3.7.3).
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
    /// `Natural range <>` — an unconstrained dimension (RM83 3.6). Not
    /// the aggregate box of the module documentation: this `<>` is an
    /// index subtype's own syntax, original to Ada 83.
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

/// A subprogram declaration with no body — always a forward declaration
/// here, never an abstract primitive operation: this tree has no
/// interface type to declare one for.
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
}

/// Whether a subprogram returns a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubprogramKind {
    /// `procedure` — no result.
    Procedure,
    /// `function` — a result.
    Function,
}

/// One parameter of a subprogram (RM83 6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The passing mode.
    pub mode: Mode,
    /// The subtype.
    pub type_: Path,
}

/// An Ada parameter mode, unchanged from RM83 6.1 through every later
/// revision.
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
    /// Every mode Ada 83 defines.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name, possibly qualified.
    Name(Path),
    /// `F (a, b)` — a function call.
    Call {
        /// The subprogram.
        name: Path,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `-a`.
    Negate(Box<Expr>),
    /// `a op b`, with the shared walk's operator spelling; the renderer
    /// maps it.
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
    Selected {
        /// The prefix.
        value: Box<Expr>,
        /// The selector.
        name: Ident,
    },
    /// `a (i)` — an indexed component.
    Indexed {
        /// The array, or an access to one (RM83 4.1 dereferences it).
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
    /// `P'Name` or `P'Name (a)` — an attribute reference. `'Length` is
    /// the only one the codec arc writes here: `'Enum_Val`/`'Enum_Rep`
    /// have no counterpart in this tree, per the module documentation.
    Attribute {
        /// The prefix.
        value: Box<Expr>,
        /// The attribute's identifier.
        name: Ident,
        /// The arguments, for the functional attributes.
        args: Vec<Expr>,
    },
    /// `Mark (a)` — a type conversion (RM83 4.6).
    Convert {
        /// The target subtype mark.
        mark: Path,
        /// The operand.
        value: Box<Expr>,
    },
    /// `(A => x, B => y)` — a record aggregate. Always at least one
    /// association: this tree has no `<>`, and a zero-component record
    /// with no discriminant has no aggregate at all — see the module
    /// documentation.
    Aggregate(Vec<Association>),
    /// `new Mark (low .. high)` or `new Mark'(value)` — an allocator
    /// (RM83 4.8).
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
    /// The value.
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
    /// `new Mark'(value)` — a qualified initial value (RM83 4.7).
    Qualified(Box<Expr>),
}

/// One statement of a subprogram body. As in the Ada 2012 tree,
/// [`Stmt::Var`] and [`Stmt::Subtype`] are statements *here* because the
/// shared codec walk interleaves them; the renderer folds a run of them
/// into a `declare` block or the subprogram's own declarative part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `null;`
    Null,
    /// An object declaration.
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
    /// A discriminant constraint — `Name : Mark (Constraint);` — the
    /// same RM83 3.7.1 mechanism the Ada 2012 tree uses for a
    /// discriminant a body only knows at run time.
    pub constraint: Option<Expr>,
    /// The initial value.
    pub value: Option<Expr>,
}

/// A subprogram with its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprogramBody {
    /// The profile.
    pub spec: Subprogram,
    /// The statements. Leading [`Stmt::Var`], [`Stmt::Subtype`] and
    /// [`Stmt::Comment`] entries are rendered into the declarative part;
    /// an empty remainder renders as `null;`.
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

/// A type this column names in a generated spec — every one of them
/// declared in `Standard` or in `Zd_Wire83`, and none of them
/// implementation-defined in width: `Long_Integer` is not `Integer`
/// (RM83 3.5.4 fixes only a lower bound on either), it is the one this
/// column always names, and `runtime/ada-83` is written to whatever
/// width GNAT gives it as long as it reaches `2**31 - 1`, which every
/// GNAT target does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `Boolean` — from `Standard` (RM83 A.1).
    Boolean,
    /// `Long_Integer` — every integer-holding scalar this column
    /// declares, signed or unsigned alike; see the module
    /// documentation.
    LongInteger,
    /// `Float` — the IEEE 754 binary32 `runtime/ada-83` writes.
    Float,
    /// `Long_Float` — the IEEE 754 binary64 `runtime/ada-83` writes.
    /// RM83 3.5.7 already predefines it; nothing here is Ada 95's
    /// `Interfaces.IEEE_Float_64`.
    LongFloat,
    /// `Zd_Wire83.Text` — an access to an unconstrained `String`, the
    /// answer a `string`, a `wstring`, a `fixed<P,S>` and a `long
    /// double` all take, in the absence of `Ada.Strings.Unbounded`.
    Text,
    /// `System.Address` — an IDL `native` declaration's target. RM83
    /// 13.7 already declares `System`; it is not a child unit and needs
    /// none of Ada 95's reorganisation.
    Address,
}

impl Primitive {
    /// Every type the tree names.
    pub const ALL: [Self; 6] = [
        Self::Boolean,
        Self::LongInteger,
        Self::Float,
        Self::LongFloat,
        Self::Text,
        Self::Address,
    ];

    /// How it is written, fully qualified.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::LongInteger => "Long_Integer",
            Self::Float => "Float",
            Self::LongFloat => "Long_Float",
            Self::Text => "Zd_Wire83.Text",
            Self::Address => "System.Address",
        }
    }

    /// The same, as a path the tree can hold.
    #[must_use]
    pub fn path(self) -> Path {
        Path::new(&self.as_str().split('.').collect::<Vec<&str>>())
    }

    /// The library unit a `with` clause has to name for it. `None` for
    /// the three `Standard` types, which need none (RM83 8.2).
    #[must_use]
    pub fn unit(self) -> Option<Path> {
        match self {
            Self::Boolean | Self::LongInteger | Self::Float | Self::LongFloat => None,
            Self::Text => Some(Path::single("Zd_Wire83")),
            Self::Address => Some(Path::single("System")),
        }
    }
}

/// The letters this column repairs an identifier with — the same marker
/// the Ada 2012 tree uses, and for the same three RM 2.3 constraints,
/// unchanged since RM83 2.3.
pub const ESCAPE_MARKER: &str = "Idl";

/// The suffix a name that cannot be written as it stands takes. A
/// letter suffix and not a trailing underscore, for the identical
/// reason `zerodds_lang_cst::ada::ESCAPE_SUFFIX` gives: RM83 2.3
/// forbids one.
pub const ESCAPE_SUFFIX: &str = "_Idl";

/// The same name, spelled so that Ada 83 will read it as an identifier.
/// Identical in every particular to `zerodds_lang_cst::ada::legal_name`:
/// the three constraints of RM 2.3 have not moved between 1983 and 2012.
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

/// The name an IDL identifier takes in Ada 83. [`ESCAPE_SUFFIX`] where
/// the identifier is a reserved word of RM83 2.9 or one of the names a
/// generated spec relies on.
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

/// The conventional Ada spelling of an identifier — identical to
/// `zerodds_lang_cst::ada::ada_case`, since RM 2.3(5/2)'s case folding
/// is unchanged from RM83 2.3.
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
#[must_use]
pub fn fold(name: &str) -> String {
    name.to_lowercase()
}

/// Every name an escape has to move out of the way of: RM83 2.9's
/// reserved words — 63 of them, twelve fewer than Ada 2012's 73, since
/// `abstract`, `aliased`, `interface`, `overriding`, `protected`,
/// `requeue`, `some`, `synchronized`, `tagged`, `until` and
/// `terminate`'s newer partners do not exist yet — then the names this
/// column's own generated spec depends on: the library units it
/// `with`s, the `Standard` types it writes, and `Zd83_Types` itself
/// (RM83 8.1 puts a package's own name in the region it encloses).
pub const RESERVED: &[&str] = &[
    // Reserved words, RM83 2.9.
    "abort",
    "abs",
    "accept",
    "access",
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
    "package",
    "pragma",
    "private",
    "procedure",
    "raise",
    "range",
    "record",
    "rem",
    "renames",
    "return",
    "reverse",
    "select",
    "separate",
    "subtype",
    "task",
    "then",
    "type",
    "use",
    "when",
    "while",
    "with",
    "xor",
    // Library units a generated spec names, and the packages above them.
    "system",
    "standard",
    "zd_wire83",
    // `Standard` names a generated spec writes or relies on.
    "boolean",
    "character",
    "duration",
    "float",
    "integer",
    "long_integer",
    "long_float",
    "natural",
    "positive",
    "string",
    // The package this column declares. RM83 8.1.
    "zd83_types",
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
    fn no_named_type_needs_a_use_clause() {
        // Every scalar is either Standard's, whose operators are
        // always directly visible (RM83 8.2), or reached only through a
        // call: this tree carries no `use_type` construct at all.
        for primitive in Primitive::ALL {
            assert!(
                primitive.unit().is_none()
                    || primitive == Primitive::Text
                    || primitive == Primitive::Address,
            );
        }
    }

    #[test]
    fn no_child_unit_is_named() {
        // No `Ada.` prefix reaches this tree at all: RM83's I/O and
        // string packages are not children of `Ada`.
        for primitive in Primitive::ALL {
            if let Some(unit) = primitive.unit() {
                assert!(
                    !unit.segments().iter().any(|s| s.as_str() == "Ada"),
                    "{} names a child unit",
                    primitive.as_str()
                );
            }
        }
    }

    #[test]
    fn a_reserved_word_escapes_with_a_letter_and_never_an_underscore() {
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
        for spelling in ["type", "Type", "TYPE", "TyPe"] {
            assert!(is_reserved(spelling), "{spelling} was not recognised");
            assert_eq!(escape_name(spelling), alloc::format!("{spelling}_Idl"));
        }
    }

    #[test]
    fn ada_95_words_are_not_reserved_here() {
        // The twelve words RM83 2.9 does not have. A member named one
        // of these needs no escape under `-gnat83`, unlike under the
        // Ada 2012 column.
        for word in [
            "abstract",
            "aliased",
            "interface",
            "overriding",
            "protected",
            "requeue",
            "some",
            "synchronized",
            "tagged",
            "terminate",
            "until",
        ] {
            assert!(!is_reserved(word), "{word} is not an Ada 83 reserved word");
        }
    }

    #[test]
    fn an_identifier_may_not_start_with_an_underscore() {
        assert_eq!(legal_name("_long"), "Idl_long");
        assert_eq!(legal_name("_d"), "Idl_d");
        assert_eq!(legal_name("plain"), "plain");
    }

    #[test]
    fn an_identifier_may_not_hold_two_underscores_in_a_row() {
        assert_eq!(legal_name("a__b"), "a_Idl_b");
        assert_eq!(legal_name("a_b"), "a_b");
    }

    #[test]
    fn an_identifier_may_not_end_in_an_underscore() {
        assert_eq!(legal_name("foo_"), "foo_Idl");
    }

    #[test]
    fn ada_casing_moves_the_first_letter_of_each_word_and_nothing_else() {
        assert_eq!(ada_case("value"), "Value");
        assert_eq!(ada_case("my_field"), "My_Field");
        assert_eq!(ada_case("IDLE"), "IDLE");
    }

    #[test]
    fn two_spellings_that_differ_only_in_case_have_one_fold_key() {
        assert_eq!(fold("Value"), fold("value"));
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
    fn a_record_with_nothing_in_it_is_a_null_record() {
        assert!(Record::plain(Vec::new()).is_null());
        assert!(
            !Record::plain(alloc::vec![Component {
                doc: Doc::none(),
                name: Ident::new("X"),
                type_: Primitive::LongInteger.path(),
            }])
            .is_null()
        );
    }

    #[test]
    fn every_parameter_mode_writes_its_ada_spelling() {
        assert_eq!(Mode::In.as_str(), "in");
        assert_eq!(Mode::Out.as_str(), "out");
        assert_eq!(Mode::InOut.as_str(), "in out");
        assert_eq!(Mode::ALL.len(), 3);
    }
}
