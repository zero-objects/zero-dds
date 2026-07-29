// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST types for OMG IDL 4.2.
//!
//! All types are owned (`String`, `Vec`, `Box`) — no source borrowing.
//! This lets the AST outlive the source string and be passed on, e.g. in build
//! pipelines, where the source file is already
//! closed.
//!
//! # Span convention
//! Every node that is diagnostics-relevant carries a `span: Span` —
//! byte offset in the source text. Helper structures without independent
//! diagnostics (e.g. [`PrimitiveType`]) have no span.

#![allow(missing_docs)] // doc-comments follow with T5.7

use crate::errors::Span;

// ============================================================================
// Specification / definition
// ============================================================================

/// Root node — corresponds to `<specification>` (§7.4.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Specification {
    pub definitions: Vec<Definition>,
    pub span: Span,
}

/// Top-level definition. Corresponds to `<definition>` (§7.4.1.2) plus an
/// optional `VendorExtension` catch-all for delta-inserted
/// constructs (T6.5).
#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Module(ModuleDef),
    Type(TypeDecl),
    Const(ConstDecl),
    Except(ExceptDecl),
    Interface(InterfaceDcl),
    ValueBox(ValueBoxDecl),
    ValueForward(ValueForwardDecl),
    /// `valuetype <name> [: <inheritance>] [supports <ifaces>] { <elements> };`
    /// (§7.4.5.4 Rule 99).
    ValueDef(ValueDef),
    /// `typeid <scoped_name> "<repo-id>";` (§7.4.6.3 Rule 113).
    TypeId(TypeIdDcl),
    /// `typeprefix <scoped_name> "<prefix>";` (§7.4.6.3 Rule 114).
    TypePrefix(TypePrefixDcl),
    /// `import <imported_scope>;` (§7.4.6.3 Rule 115).
    Import(ImportDcl),
    /// `component <name> [: base] [supports …] { … };` (§7.4.8.3 Rule 134).
    Component(ComponentDcl),
    /// `home <name> [: base] [supports …] manages <comp> [primarykey K] { … };`
    /// (§7.4.9.3 Rule 145).
    Home(HomeDcl),
    /// `eventtype <name> { … };` / `abstract eventtype` / `eventtype X;`
    /// (§7.4.10.3 Rule 166).
    Event(EventDcl),
    /// `porttype <name> { … };` / `porttype X;` (§7.4.11.3 Rule 172).
    Porttype(PorttypeDcl),
    /// `connector <name> [: base] { <port_ref>+ };` (§7.4.11.3 Rule 180).
    Connector(ConnectorDcl),
    /// `module <name> < <formal_params> > { <tpl_definition>+ };`
    /// (§7.4.12.3 Rule 185).
    TemplateModule(TemplateModuleDcl),
    /// `module <scoped_name>< <actual_params> > <new_name>;`
    /// (§7.4.12.3 Rule 190).
    TemplateModuleInst(TemplateModuleInst),
    /// User-Defined `@annotation Foo { ... };` (§7.4.15 Rules 218-221).
    Annotation(AnnotationDcl),
    /// Top-level construct from a vendor delta extension
    /// (e.g. `keylist Type (fields);` from the RTI delta). The span covers
    /// the entire construct; `production_name` is the production
    /// name from the delta definition.
    VendorExtension(VendorExtension),
}

impl Definition {
    /// Gemeinsamer Span-Accessor.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Module(d) => d.span,
            Self::Type(d) => d.span(),
            Self::Const(d) => d.span,
            Self::Except(d) => d.span,
            Self::Interface(d) => d.span(),
            Self::ValueBox(d) => d.span,
            Self::ValueForward(d) => d.span,
            Self::ValueDef(d) => d.span,
            Self::TypeId(d) => d.span,
            Self::TypePrefix(d) => d.span,
            Self::Import(d) => d.span,
            Self::Component(d) => d.span(),
            Self::Home(d) => d.span(),
            Self::Event(d) => d.span(),
            Self::Porttype(d) => d.span(),
            Self::Connector(d) => d.span,
            Self::TemplateModule(d) => d.span,
            Self::TemplateModuleInst(d) => d.span,
            Self::Annotation(d) => d.span,
            Self::VendorExtension(v) => v.span,
        }
    }
}

/// User-Defined Annotation Declaration (§7.4.15).
///
/// `@annotation <name> { <member>* };` with members per Rule 220:
/// `annotation_member` (CONST_TYPE simple_declarator [default expr])
/// or embedded `enum_dcl`/`const_dcl`/`typedef_dcl`.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationDcl {
    pub name: Identifier,
    pub members: Vec<AnnotationMember>,
    pub embedded_types: Vec<TypeDecl>,
    pub embedded_consts: Vec<ConstDecl>,
    pub span: Span,
}

/// Member-Slot (§7.4.15 Rule 221).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationMember {
    pub name: Identifier,
    pub type_spec: ConstType,
    pub default: Option<ConstExpr>,
    pub span: Span,
}

// ============================================================================
// CORBA-specific top-level decls (§7.4.6.3)
// ============================================================================

/// `typeid <scoped_name> "<repo-id>";` (§7.4.6.3 Rule 113).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeIdDcl {
    pub target: ScopedName,
    pub repository_id: String,
    pub span: Span,
}

/// `typeprefix <scoped_name> "<prefix>";` (§7.4.6.3 Rule 114).
#[derive(Debug, Clone, PartialEq)]
pub struct TypePrefixDcl {
    pub target: ScopedName,
    pub prefix: String,
    pub span: Span,
}

/// `import <imported_scope>;` (§7.4.6.3 Rule 115).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDcl {
    pub imported: ImportedScope,
    pub span: Span,
}

/// `<imported_scope>` — scoped name or string repository ID.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportedScope {
    Scoped(ScopedName),
    Repository(String),
}

// ============================================================================
// Value-Type Full Definition (§7.4.5.4)
// ============================================================================

/// `valuetype <name> [: <inheritance>] [supports <ifaces>] { <elements> };`
/// (§7.4.5.4 Rule 99 + §7.4.7.3 Custom/Abstract-Variants).
#[derive(Debug, Clone, PartialEq)]
pub struct ValueDef {
    pub name: Identifier,
    pub kind: ValueKind,
    pub inheritance: Option<ValueInheritanceSpec>,
    pub elements: Vec<ValueElement>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// `valuetype <name> { … };`.
    Concrete,
    /// `custom valuetype <name> { … };` (§7.4.7.3 Rule 128).
    Custom,
    /// `abstract valuetype <name> { … };` (§7.4.7.3 Rule 127).
    Abstract,
}

/// `: [truncatable] <value_name>{,…} [supports <iface>{,…}]` (Rules 102-104,130).
#[derive(Debug, Clone, PartialEq)]
pub struct ValueInheritanceSpec {
    pub truncatable: bool,
    pub bases: Vec<ScopedName>,
    pub supports: Vec<ScopedName>,
    pub span: Span,
}

/// `<value_element>` (§7.4.5.4.1.3 Rule 105).
#[derive(Debug, Clone, PartialEq)]
pub enum ValueElement {
    /// `<export>` — Op/Attr/Type/Const/Except (analogous to interface body).
    Export(Export),
    /// `<state_member>` with visibility (Rule 106).
    State(StateMember),
    /// `<init_dcl>` — `factory <name>(<params>) [<raises>];` (Rule 107).
    Init(InitDcl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateMember {
    pub visibility: StateVisibility,
    pub type_spec: TypeSpec,
    pub declarators: Vec<Declarator>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitDcl {
    pub name: Identifier,
    pub params: Vec<ParamDecl>,
    pub raises: Vec<ScopedName>,
    pub span: Span,
}

// ============================================================================
// Components – Basic + Homes + CCM-Specific (§7.4.8 - §7.4.10)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentDcl {
    Def(ComponentDef),
    Forward(Identifier, Span),
}

impl ComponentDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDef {
    pub name: Identifier,
    pub base: Option<ScopedName>,
    pub supports: Vec<ScopedName>,
    pub body: Vec<ComponentExport>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentExport {
    Provides {
        type_spec: ScopedName,
        name: Identifier,
        span: Span,
    },
    Uses {
        type_spec: ScopedName,
        name: Identifier,
        multiple: bool,
        span: Span,
    },
    Attribute(AttrDcl),
    /// `emits <T> <name>;` (§7.4.10.3 Rule 159).
    Emits {
        type_spec: ScopedName,
        name: Identifier,
        span: Span,
    },
    /// `publishes <T> <name>;` (Rule 160).
    Publishes {
        type_spec: ScopedName,
        name: Identifier,
        span: Span,
    },
    /// `consumes <T> <name>;` (Rule 161).
    Consumes {
        type_spec: ScopedName,
        name: Identifier,
        span: Span,
    },
    /// `port <T> <name>;` / `mirrorport <T> <name>;` (§7.4.11.3 Rule 178).
    Port {
        type_spec: ScopedName,
        name: Identifier,
        mirror: bool,
        span: Span,
    },
}

/// Attribute-decl stub for components/homes/values (simplified form;
/// the full attr-decl is in the interface pipeline).
#[derive(Debug, Clone, PartialEq)]
pub struct AttrDcl {
    pub readonly: bool,
    pub type_spec: TypeSpec,
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HomeDcl {
    Def(HomeDef),
    Forward(Identifier, Span),
}

impl HomeDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HomeDef {
    pub name: Identifier,
    pub base: Option<ScopedName>,
    pub supports: Vec<ScopedName>,
    pub manages: ScopedName,
    pub primary_key: Option<ScopedName>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventDcl {
    Def(EventDef),
    Forward(Identifier, Span),
}

impl EventDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventDef {
    pub name: Identifier,
    pub kind: ValueKind,
    pub inheritance: Option<ValueInheritanceSpec>,
    pub elements: Vec<ValueElement>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Ports + Connectors (§7.4.11)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PorttypeDcl {
    Def(PorttypeDef),
    Forward(Identifier, Span),
}

impl PorttypeDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PorttypeDef {
    pub name: Identifier,
    pub body: Vec<ComponentExport>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorDcl {
    pub name: Identifier,
    pub base: Option<ScopedName>,
    pub body: Vec<ComponentExport>,
    pub span: Span,
}

// ============================================================================
// Template Modules (§7.4.12)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateModuleDcl {
    pub name: Identifier,
    pub formal_params: Vec<FormalParam>,
    pub definitions: Vec<Definition>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormalParam {
    pub kind: FormalParamKind,
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormalParamKind {
    Typename,
    Interface,
    Valuetype,
    Eventtype,
    Struct,
    Union,
    Exception,
    Enum,
    Sequence,
    Const(ConstType),
    SequenceType(SequenceType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateModuleInst {
    pub template: ScopedName,
    pub actual_params: Vec<ConstExpr>,
    pub instance_name: Identifier,
    pub span: Span,
}

/// Catch-all for top-level constructs inserted by a vendor delta.
/// The AST builder leaves the inner structure unevaluated — the
/// per-vendor-delta AST builder should enrich it in a later phase.
#[derive(Debug, Clone, PartialEq)]
pub struct VendorExtension {
    /// Production name from the delta definition (e.g. `"rti_keylist"`).
    pub production_name: String,
    /// Raw source slice of the construct for diagnostics.
    pub raw: String,
    pub span: Span,
}

/// `module <ident> { <definition>* };`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDef {
    pub name: Identifier,
    pub definitions: Vec<Definition>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
    /// Name-identifier span of every source occurrence that was merged into
    /// this node — one entry for a plain module, N entries for a module
    /// reopened N times (§7.4.1.4). The AST builder folds reopened
    /// `module M { ... }` blocks into a single [`ModuleDef`] so backends
    /// emit one wrapper per module instead of double-emitting (see
    /// `ast::builder::merge_reopened_modules`); validators that care about
    /// *where* each reopen occurred (e.g. the §7.4.6.4.1.4 effect-5
    /// exposed-redefine check) inspect this instead of `name.span`, which
    /// only ever reflects the first occurrence.
    pub reopen_spans: Vec<Span>,
}

// ============================================================================
// Type-Decl
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TypeDecl {
    Constr(ConstrTypeDecl),
    Typedef(TypedefDecl),
    /// `native <simple_declarator>;` (§7.4.1.3 Rule 61). Declares a
    /// platform-/language-specific opaque type without IDL structure — a CORBA
    /// construct (e.g. `native Cookie;`), active by default in the parser.
    Native(NativeDecl),
}

impl TypeDecl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Constr(d) => d.span(),
            Self::Typedef(d) => d.span,
            Self::Native(d) => d.span,
        }
    }
}

/// `native <name>;` — opaque type without an IDL definition (§7.4.1.3).
#[derive(Debug, Clone, PartialEq)]
pub struct NativeDecl {
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstrTypeDecl {
    Struct(StructDcl),
    Union(UnionDcl),
    Enum(EnumDef),
    Bitset(BitsetDecl),
    Bitmask(BitmaskDecl),
}

impl ConstrTypeDecl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Struct(d) => d.span(),
            Self::Union(d) => d.span(),
            Self::Enum(d) => d.span,
            Self::Bitset(d) => d.span,
            Self::Bitmask(d) => d.span,
        }
    }
}

// ============================================================================
// Struct
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum StructDcl {
    Def(StructDef),
    Forward(StructForwardDecl),
}

impl StructDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(d) => d.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: Identifier,
    /// Optionaler Basis-Typ (Extended Data Types BB §7.4.13).
    pub base: Option<ScopedName>,
    pub members: Vec<Member>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructForwardDecl {
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub type_spec: TypeSpec,
    pub declarators: Vec<Declarator>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Union
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum UnionDcl {
    Def(UnionDef),
    Forward(UnionForwardDecl),
}

impl UnionDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(d) => d.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionDef {
    pub name: Identifier,
    pub switch_type: SwitchTypeSpec,
    pub cases: Vec<Case>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionForwardDecl {
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwitchTypeSpec {
    Integer(IntegerType),
    Char,
    Boolean,
    Octet,
    Scoped(ScopedName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub labels: Vec<CaseLabel>,
    pub element: ElementSpec,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaseLabel {
    /// `case <expr>:`.
    Value(ConstExpr),
    /// `default:`.
    Default,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementSpec {
    pub type_spec: TypeSpec,
    pub declarator: Declarator,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Enum
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: Identifier,
    pub enumerators: Vec<Enumerator>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Enumerator {
    pub name: Identifier,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Bitset / Bitmask
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct BitsetDecl {
    pub name: Identifier,
    pub base: Option<ScopedName>,
    pub bitfields: Vec<Bitfield>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bitfield {
    pub spec: BitfieldSpec,
    /// `None` for anonymous padding bitfields.
    pub name: Option<Identifier>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitfieldSpec {
    pub width: ConstExpr,
    pub dest_type: Option<PrimitiveType>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitmaskDecl {
    pub name: Identifier,
    pub values: Vec<BitValue>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitValue {
    pub name: Identifier,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Typedef + Declarators
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct TypedefDecl {
    pub type_spec: TypeSpec,
    pub declarators: Vec<Declarator>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Declarator {
    Simple(Identifier),
    Array(ArrayDeclarator),
}

impl Declarator {
    #[must_use]
    pub fn name(&self) -> &Identifier {
        match self {
            Self::Simple(n) => n,
            Self::Array(a) => &a.name,
        }
    }

    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Simple(n) => n.span,
            Self::Array(a) => a.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayDeclarator {
    pub name: Identifier,
    pub sizes: Vec<ConstExpr>,
    pub span: Span,
}

// ============================================================================
// Const Decl + ConstExpr
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: Identifier,
    pub type_: ConstType,
    pub value: ConstExpr,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

/// Restricted type-spec for `const` decls (§7.4.1.4.4.5).
#[derive(Debug, Clone, PartialEq)]
pub enum ConstType {
    Integer(IntegerType),
    Floating(FloatingType),
    Char,
    WideChar,
    Boolean,
    Octet,
    String { wide: bool },
    Fixed,
    Scoped(ScopedName),
}

/// Expression — corresponds to `<const_expr>` and sub-productions.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstExpr {
    Literal(Literal),
    Scoped(ScopedName),
    Unary {
        op: UnaryOp,
        operand: Box<ConstExpr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<ConstExpr>,
        rhs: Box<ConstExpr>,
        span: Span,
    },
}

impl ConstExpr {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span,
            Self::Scoped(s) => s.span,
            Self::Unary { span, .. } | Self::Binary { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    Xor,
    And,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// Literal value — content taken from the source text.
#[derive(Debug, Clone, PartialEq)]
pub struct Literal {
    pub kind: LiteralKind,
    /// Raw text from the source (incl. quotes/suffix). Semantic
    /// conversion happens only in the AST builder/validator.
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralKind {
    Integer,
    Floating,
    Fixed,
    Char,
    WideChar,
    String,
    WideString,
    Boolean,
}

// ============================================================================
// Exception
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ExceptDecl {
    pub name: Identifier,
    pub members: Vec<Member>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Interface
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum InterfaceDcl {
    Def(InterfaceDef),
    Forward(InterfaceForwardDecl),
}

impl InterfaceDcl {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Def(d) => d.span,
            Self::Forward(d) => d.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDef {
    pub kind: InterfaceKind,
    pub name: Identifier,
    pub bases: Vec<ScopedName>,
    pub exports: Vec<Export>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceForwardDecl {
    pub kind: InterfaceKind,
    pub name: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceKind {
    Plain,
    Abstract,
    Local,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Export {
    Op(OpDecl),
    Attr(AttrDecl),
    Type(TypeDecl),
    Const(ConstDecl),
    Except(ExceptDecl),
}

impl Export {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Op(d) => d.span,
            Self::Attr(d) => d.span,
            Self::Type(d) => d.span(),
            Self::Const(d) => d.span,
            Self::Except(d) => d.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpDecl {
    pub name: Identifier,
    pub oneway: bool,
    /// `None` for `void`, `Some` otherwise.
    pub return_type: Option<TypeSpec>,
    pub params: Vec<ParamDecl>,
    pub raises: Vec<ScopedName>,
    /// `context (...)` clause (§7.4.6.3 Rule 123/124): the property names that
    /// the operation implicitly carries over from the client context. Empty if no
    /// context clause is present. CORBA-specific, rarely used.
    pub context: Vec<String>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParamDecl {
    pub attribute: ParamAttribute,
    pub type_spec: TypeSpec,
    pub name: Identifier,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamAttribute {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttrDecl {
    pub name: Identifier,
    pub type_spec: TypeSpec,
    pub readonly: bool,
    /// `getraises` / readonly `raises` list (§7.4.3.1 Rules 91/95).
    /// For a readonly attribute this is the `raises_expr` list; for a
    /// writable attribute the `getraises` list.
    pub get_raises: Vec<ScopedName>,
    /// `setraises` list (§7.4.3.1 Rule 96). Non-empty only for a writable
    /// attribute.
    pub set_raises: Vec<ScopedName>,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

// ============================================================================
// Valuetype (minimal — see T-LIM-5)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ValueBoxDecl {
    pub name: Identifier,
    pub type_spec: TypeSpec,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValueForwardDecl {
    pub name: Identifier,
    pub span: Span,
}

// ============================================================================
// Type-Spec
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpec {
    Primitive(PrimitiveType),
    Scoped(ScopedName),
    Sequence(SequenceType),
    String(StringType),
    Fixed(FixedPtType),
    Map(MapType),
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Integer(IntegerType),
    Floating(FloatingType),
    Char,
    WideChar,
    Boolean,
    Octet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerType {
    Short,
    Long,
    LongLong,
    UShort,
    ULong,
    ULongLong,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatingType {
    Float,
    Double,
    LongDouble,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceType {
    pub elem: Box<TypeSpec>,
    /// `None` = unbounded.
    pub bound: Option<ConstExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringType {
    pub wide: bool,
    pub bound: Option<ConstExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FixedPtType {
    pub digits: ConstExpr,
    pub scale: ConstExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapType {
    pub key: Box<TypeSpec>,
    pub value: Box<TypeSpec>,
    pub bound: Option<ConstExpr>,
    pub span: Span,
}

// ============================================================================
// Annotations
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: ScopedName,
    pub params: AnnotationParams,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationParams {
    /// `@key` — no parentheses.
    None,
    /// `@id()` — empty parentheses.
    Empty,
    /// `@id(7)` — single positional expression.
    Single(ConstExpr),
    /// `@range(min=0, max=10)` — list of named parameters.
    Named(Vec<NamedParam>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedParam {
    pub name: Identifier,
    pub value: ConstExpr,
    pub span: Span,
}

// ============================================================================
// Identifier / ScopedName
// ============================================================================

/// Simple identifier with span.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub text: String,
    pub span: Span,
}

impl Identifier {
    #[must_use]
    pub fn new(text: impl Into<String>, span: Span) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }
}

/// Scoped-Name `[::] <ident> ( :: <ident> )*` (§7.4.1.4.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedName {
    /// `true` if the name begins with `::` (absolute path).
    pub absolute: bool,
    pub parts: Vec<Identifier>,
    pub span: Span,
}

impl ScopedName {
    #[must_use]
    pub fn single(ident: Identifier) -> Self {
        let span = ident.span;
        Self {
            absolute: false,
            parts: vec![ident],
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn s(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    #[test]
    fn identifier_new_stores_text_and_span() {
        let id = Identifier::new("Foo", s(0, 3));
        assert_eq!(id.text, "Foo");
        assert_eq!(id.span, s(0, 3));
    }

    #[test]
    fn scoped_name_single_wraps_identifier() {
        let id = Identifier::new("Foo", s(0, 3));
        let sn = ScopedName::single(id.clone());
        assert!(!sn.absolute);
        assert_eq!(sn.parts, vec![id]);
        assert_eq!(sn.span, s(0, 3));
    }

    #[test]
    fn definition_span_dispatches_per_variant() {
        let m = Definition::Module(ModuleDef {
            name: Identifier::new("M", s(7, 8)),
            definitions: vec![],
            annotations: vec![],
            span: s(0, 20),
            reopen_spans: vec![s(7, 8)],
        });
        assert_eq!(m.span(), s(0, 20));
    }

    #[test]
    fn const_expr_span_dispatches_per_variant() {
        let lit = ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw: "42".to_string(),
            span: s(0, 2),
        });
        assert_eq!(lit.span(), s(0, 2));
        let unary = ConstExpr::Unary {
            op: UnaryOp::Minus,
            operand: Box::new(lit.clone()),
            span: s(0, 3),
        };
        assert_eq!(unary.span(), s(0, 3));
    }

    #[test]
    fn declarator_name_returns_identifier() {
        let id = Identifier::new("buf", s(0, 3));
        let d = Declarator::Simple(id.clone());
        assert_eq!(d.name(), &id);
        assert_eq!(d.span(), s(0, 3));

        let arr = Declarator::Array(ArrayDeclarator {
            name: Identifier::new("matrix", s(5, 11)),
            sizes: vec![],
            span: s(5, 15),
        });
        assert_eq!(arr.name().text, "matrix");
        assert_eq!(arr.span(), s(5, 15));
    }

    #[test]
    fn export_span_dispatches_per_variant() {
        let op = Export::Op(OpDecl {
            name: Identifier::new("ping", s(0, 4)),
            oneway: false,
            return_type: None,
            params: vec![],
            raises: vec![],
            context: vec![],
            annotations: vec![],
            span: s(0, 10),
        });
        assert_eq!(op.span(), s(0, 10));
    }

    #[test]
    fn type_decl_span_dispatches() {
        let td = TypeDecl::Typedef(TypedefDecl {
            type_spec: TypeSpec::Primitive(PrimitiveType::Boolean),
            declarators: vec![],
            annotations: vec![],
            span: s(2, 10),
        });
        assert_eq!(td.span(), s(2, 10));
    }

    #[test]
    fn struct_dcl_span_dispatches() {
        let sd = StructDcl::Forward(StructForwardDecl {
            name: Identifier::new("Foo", s(7, 10)),
            span: s(0, 11),
        });
        assert_eq!(sd.span(), s(0, 11));
    }

    #[test]
    fn ast_types_are_clone_and_eq() {
        let id = Identifier::new("X", s(0, 1));
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }
}
