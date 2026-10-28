// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Client stubs — the operation signature in each PSM's argument-passing
//! form.
//!
//! A stub is the client-side proxy: it takes the arguments in the
//! language's own calling convention, marshals them into a GIOP request
//! and demarshals the reply. The **marshalling** belongs to the ORB
//! runtime. The **signature** belongs here, and it is not the mechanical
//! one: IDL to C++ §1.22 gives `in`, `inout`, `out` and the return
//! position four different spellings of the same IDL type, and IDL to
//! Java §1.4 routes `out` and `inout` through holder classes.
//!
//! Getting this wrong does not fail to compile in the obvious place. It
//! produces a stub whose `out string` takes a `char*&` where the ORB
//! passes a `CORBA::String_out`, which is a different type with a
//! different ownership rule.
//!
//! ## What decides the C++ spelling
//!
//! Not the type's name — its [`TypeShape`]. §1.9 splits every IDL type
//! into fixed-length and variable-length, and the `out` and return rows
//! differ between them. That property is computed from the model by
//! [`shape_of`], not guessed from a name, because a `struct` holding one
//! `string` is variable-length and looks like any other struct.
//!
//! ## Exceptions
//!
//! [`StubOp::raises`] carries what the operation declares. It is
//! rendered as documentation and not as a `throws` clause in every
//! column: C++ has no checked exceptions, Java's mapping does put them
//! in `throws`, and Rust has no CORBA error type in this tree at all —
//! the ORB crate that would define one is among the packages this
//! rebuild has not reached. Inventing one here would put a name in
//! generated code that resolves to nothing.

use alloc::boxed::Box as AllocBox;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, OperationNode, ParamDirection, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::{Doc, Ident, Path, Visibility, cpp, csharp, java, rust};

use crate::special_types::{SpecialType, TargetLanguage, special_type_of};

/// How deep [`shape_of`] follows aggregate members before giving up.
///
/// A cycle in IDL has to pass through a `sequence` or an object
/// reference, both of which answer variable-length without recursing, so
/// this bound is not reached by well-formed IDL. It is here so a model
/// built by hand with a cycle in it stops rather than overflowing the
/// stack, and the answer it stops with is variable-length — the
/// conservative one, which uses a `_out` where a plain reference would
/// have done.
const MAX_SHAPE_DEPTH: u32 = 64;

/// What the argument-passing rules turn on. IDL to C++ §1.9 and §1.22.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeShape {
    /// A primitive or an enumeration: passed and returned by value.
    Scalar,
    /// An aggregate whose size is known at compile time.
    FixedAggregate,
    /// An aggregate holding a string, a sequence, an `any` or a
    /// reference, so its size is not.
    VariableAggregate,
    /// A `sequence` or a `map`.
    ///
    /// Passed like a [`TypeShape::VariableAggregate`] in C++ and split
    /// from it for one reason, measured: Java gives a `typedef
    /// sequence<long> Longs` its own `LongsHolder` where every other
    /// alias resolves through to the aliased type's holder — a sequence
    /// has no Java declaration of its own to hang one on
    /// (`tests/orb-corpus/recorded/jacorb.txt`, row 15).
    Collection,
    /// An array. It has `_slice`, `_forany` and `_out` forms of its own.
    Array {
        /// Whether the element type is variable-length.
        variable: bool,
    },
    /// A reference to a declared interface.
    ObjectReference,
    /// A valuetype.
    Valuetype,
}

/// The type of a parameter or a return, as far as the stub mapping
/// cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StubType {
    /// One of the thirteen Annex A.1 rows.
    Special(SpecialType),
    /// Any other IDL type: its scoped name, and the shape that decides
    /// how it is passed.
    Other {
        /// The type's scoped name, in the source's `::` form.
        name: String,
        /// What the argument-passing rules turn on.
        shape: TypeShape,
        /// What a `typedef` names, for a language that has no typedef.
        ///
        /// `None` when the declaration is not an alias. C++ has
        /// typedefs and writes `name`; Java has none and writes what
        /// this resolves to — `typedef WithString W` is `probe.W` in
        /// C++ and `probe.WithString` in Java, both measured
        /// (`tests/orb-corpus/recorded/*`, row 15).
        aliased: Option<AllocBox<StubType>>,
    },
    /// `void` — a return position and nothing else.
    Void,
}

impl StubType {
    /// A named type of a given shape, declared rather than aliased.
    #[must_use]
    pub fn other(name: &str, shape: TypeShape) -> Self {
        Self::Other {
            name: name.to_string(),
            shape,
            aliased: None,
        }
    }

    /// A `typedef` of `aliased`, written by `name`.
    #[must_use]
    pub fn alias(name: &str, aliased: Self) -> Self {
        Self::Other {
            name: name.to_string(),
            shape: aliased.shape(),
            aliased: Some(AllocBox::new(aliased)),
        }
    }

    /// What this resolves to for a language with no typedef.
    #[must_use]
    pub fn resolved(&self) -> &Self {
        match self {
            Self::Other {
                aliased: Some(inner),
                ..
            } => inner.resolved(),
            other => other,
        }
    }

    /// Whether Java gives this its own generated holder rather than
    /// resolving through to the aliased type's.
    ///
    /// True for a `sequence` and for a `fixed`: neither has a Java
    /// declaration of its own, so the `typedef` that names it is where
    /// the holder is generated. Measured — `probe.LongsAliasHolder` and
    /// `probe.MoneyHolder` against `probe.WithStringHolder` for a struct
    /// alias (`tests/orb-corpus/recorded/jacorb.txt`, rows 11 and 15).
    #[must_use]
    pub fn names_its_own_java_holder(&self) -> bool {
        matches!(
            self.resolved(),
            Self::Special(SpecialType::Fixed | SpecialType::SequenceOfAny)
                | Self::Other {
                    shape: TypeShape::Collection,
                    ..
                }
        )
    }

    /// The shape this type is passed by.
    #[must_use]
    pub const fn shape(&self) -> TypeShape {
        match self {
            Self::Special(t) => special_shape(*t),
            Self::Other { shape, .. } => *shape,
            // Never asked in an argument position; a `void` parameter
            // is not IDL.
            Self::Void => TypeShape::Scalar,
        }
    }
}

/// The shape each of the thirteen is passed by.
const fn special_shape(t: SpecialType) -> TypeShape {
    match t {
        // `void*`, `TimeBase::TimeT`, `CORBA::ULongLong`,
        // `CORBA::LongDouble` — all by value.
        SpecialType::NativeRef
        | SpecialType::Time
        | SpecialType::ULongLong
        | SpecialType::LongDouble => TypeShape::Scalar,
        SpecialType::Object | SpecialType::AbstractBase | SpecialType::TypeCode => {
            TypeShape::ObjectReference
        }
        SpecialType::ValueBase => TypeShape::Valuetype,
        // `CORBA::Fixed` is a fixed-length value type — the name is not
        // a coincidence: §7.4.1.7 bounds it at 31 digits.
        SpecialType::Fixed => TypeShape::FixedAggregate,
        // A string, a wstring and an `any` all carry a length the
        // declaration does not fix.
        SpecialType::String | SpecialType::WString | SpecialType::Any => {
            TypeShape::VariableAggregate
        }
        SpecialType::SequenceOfAny => TypeShape::Collection,
    }
}

/// One parameter of an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubParam {
    /// Parameter name as the IDL wrote it.
    pub name: String,
    /// Its type.
    pub type_: StubType,
    /// `in`, `out` or `inout`.
    pub direction: ParamDirection,
}

/// An operation, as a stub has to declare it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubOp {
    /// Operation name as the IDL wrote it, which is also the GIOP
    /// operation string.
    pub name: String,
    /// Return type; [`StubType::Void`] when the operation returns
    /// nothing.
    pub return_type: StubType,
    /// Parameters, in declaration order.
    pub params: Vec<StubParam>,
    /// `oneway` — no reply is expected. IDL 4.2 §7.4.3.5.
    pub oneway: bool,
    /// Scoped names of the exceptions the operation raises.
    pub raises: Vec<String>,
}

/// Build a [`StubOp`] from an operation in the model.
///
/// Returns `None` only for a stale handle.
#[must_use]
pub fn stub_op_of(ast: &Ast, id: Id<OperationNode>) -> Option<StubOp> {
    let node: &OperationNode = ast.operation(id).ok()?;
    let return_type = node
        .return_type
        .as_ref()
        .and_then(zerodds_ast_core::Ref::resolved)
        .map_or(StubType::Void, |target| stub_type_of(ast, target));
    let params = node
        .params
        .iter()
        .filter_map(|param| {
            let param = ast.param(*param).ok()?;
            Some(StubParam {
                name: param.name.clone(),
                type_: param
                    .type_
                    .resolved()
                    .map_or(StubType::Void, |target| stub_type_of(ast, target)),
                direction: param.direction,
            })
        })
        .collect();
    let raises = node
        .raises
        .iter()
        .filter_map(|raised| {
            let id = raised.resolved()?;
            let node = ast.exception(id).ok()?;
            Some(node.name.clone())
        })
        .collect();
    Some(StubOp {
        name: node.name.clone(),
        return_type,
        params,
        oneway: node.oneway,
        raises,
    })
}

/// The stub type a model type node is.
///
/// A **declared** type is written by its own name, even when what it
/// declares is one of the thirteen. `typedef fixed<10,2> Money` is
/// `probe::Money` in a signature and not `CORBA::Fixed`; `typedef
/// sequence<any> AnySeq` is `probe::AnySeq` and not `CORBA::AnySeq`;
/// `native Handle` is `probe::Handle` and not `void*`. All three ORBs
/// agree — `tests/orb-corpus/recorded/*`, rows 04, 07 and 11 — and an
/// earlier version of this function followed the alias and produced the
/// underlying name, which is a type the generated header never declares.
///
/// [`special_type_of`] still answers the row for the same node: what a
/// `Money` *is* and what a signature *writes* are different questions.
#[must_use]
pub fn stub_type_of(ast: &Ast, id: Id<TypeNode>) -> StubType {
    stub_type_at(ast, id, 0)
}

/// The body of [`stub_type_of`], carrying how many named aliases the
/// walk has already unwrapped.
///
/// zerodds-lint: recursion-depth 64 (named-alias chains; bounded by
/// [`MAX_SHAPE_DEPTH`], past which the alias is not unwrapped and the
/// node answers for itself)
fn stub_type_at(ast: &Ast, id: Id<TypeNode>, depth: u32) -> StubType {
    let Ok(node) = ast.type_(id) else {
        return StubType::Void;
    };
    if depth < MAX_SHAPE_DEPTH
        && !node.name.is_empty()
        && let TypeKind::Alias { aliased } = &node.kind
        && let Some(target) = aliased.resolved()
    {
        return StubType::alias(
            &scoped_name_of_type(ast, id),
            stub_type_at(ast, target, depth + 1),
        );
    }
    // A `native` declaration is a declaration and not an alias: every
    // ORB writes its name and none of them declares it, which is what
    // `native` means. It reaches `Other` with no aliased target, so no
    // column resolves it through to `void*`.
    if !node.name.is_empty() && matches!(node.kind, TypeKind::Native) {
        return StubType::other(&scoped_name_of_type(ast, id), TypeShape::Scalar);
    }
    if let Some(special) = special_type_of(ast, id) {
        return StubType::Special(special);
    }
    StubType::other(&scoped_name_of_type(ast, id), shape_of(ast, id))
}

/// The scoped name of a type node, in the source's `::` form.
fn scoped_name_of_type(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(node) = ast.type_(id) else {
        return "void".to_string();
    };
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map_or_else(|| node.name.clone(), |scoped| scoped.segments().join("::"))
}

/// Whether a type is fixed-length or variable-length, and which
/// aggregate it is. IDL to C++ §1.9.
///
/// A `typedef` is followed; the alias has the shape of what it aliases.
#[must_use]
pub fn shape_of(ast: &Ast, id: Id<TypeNode>) -> TypeShape {
    shape_at(ast, id, 0)
}

/// The body of [`shape_of`], carrying how far the walk has descended.
///
/// This one does descend into an aggregate's members, so a recursive
/// IDL type reaches itself here.
///
/// zerodds-lint: recursion-depth 64 (alias, array-element, base and
/// member nesting; bounded by [`MAX_SHAPE_DEPTH`], past which the
/// answer is the conservative `TypeShape::VariableAggregate`)
fn shape_at(ast: &Ast, id: Id<TypeNode>, depth: u32) -> TypeShape {
    if depth >= MAX_SHAPE_DEPTH {
        return TypeShape::VariableAggregate;
    }
    let Ok(node) = ast.type_(id) else {
        return TypeShape::Scalar;
    };
    match &node.kind {
        TypeKind::Primitive(_) | TypeKind::Enumeration { .. } => TypeShape::Scalar,
        // A bitmask is an integer holder and a bitset is a fixed run of
        // bits; both are fixed-length and neither has a `_var`.
        TypeKind::Bitmask { .. } => TypeShape::Scalar,
        TypeKind::Bitset { .. } | TypeKind::Fixed { .. } => TypeShape::FixedAggregate,
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => TypeShape::Collection,
        TypeKind::String { .. } | TypeKind::Any | TypeKind::ValueBase => {
            TypeShape::VariableAggregate
        }
        TypeKind::Native => TypeShape::Scalar,
        TypeKind::ObjectRef { .. } => TypeShape::ObjectReference,
        TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => TypeShape::Valuetype,
        TypeKind::Array { element, .. } => TypeShape::Array {
            variable: element
                .resolved()
                .is_some_and(|element| is_variable(shape_at(ast, element, depth + 1))),
        },
        TypeKind::Alias { aliased } => aliased
            .resolved()
            .map_or(TypeShape::Scalar, |target| shape_at(ast, target, depth + 1)),
        TypeKind::Structure { base, members } => {
            let base_variable = base
                .as_ref()
                .and_then(zerodds_ast_core::Ref::resolved)
                .is_some_and(|base| is_variable(shape_at(ast, base, depth + 1)));
            let any_member = members.iter().any(|member| {
                ast.member(*member).ok().is_some_and(|member| {
                    member
                        .type_
                        .resolved()
                        .is_some_and(|target| is_variable(shape_at(ast, target, depth + 1)))
                })
            });
            if base_variable || any_member {
                TypeShape::VariableAggregate
            } else {
                TypeShape::FixedAggregate
            }
        }
        TypeKind::Union { cases, .. } => {
            let any_case = cases.iter().any(|case| {
                ast.case(*case)
                    .ok()
                    .and_then(|case| ast.member(case.member).ok())
                    .and_then(|member| member.type_.resolved())
                    .is_some_and(|target| is_variable(shape_at(ast, target, depth + 1)))
            });
            if any_case {
                TypeShape::VariableAggregate
            } else {
                TypeShape::FixedAggregate
            }
        }
        TypeKind::AnnotationType { .. } => TypeShape::Scalar,
    }
}

/// Whether a shape is variable-length in the §1.9 sense.
#[must_use]
pub const fn is_variable(shape: TypeShape) -> bool {
    match shape {
        TypeShape::Scalar | TypeShape::FixedAggregate => false,
        TypeShape::VariableAggregate
        | TypeShape::Collection
        | TypeShape::ObjectReference
        | TypeShape::Valuetype => true,
        TypeShape::Array { variable } => variable,
    }
}

/// A language has no CORBA PSM, or none this tree can render.
///
/// No column is in that state now. It named C#, whose tree did not exist
/// when this crate was written; `zerodds_lang_cst::csharp` holds one, so
/// [`render_stub_op`] and [`crate::render_skeleton_dispatch`] answer all
/// four. The type stays in the interface because it is what a column
/// added without a tree would be answered with, and because removing it
/// would change the signature of two published functions for no gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRenderer {
    /// The column that was asked for.
    pub language: TargetLanguage,
}

impl core::fmt::Display for NoRenderer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} has an OMG mapping but no CST in this tree, so nothing can be rendered for it",
            self.language.as_str()
        )
    }
}

/// Render an operation's stub declaration as text.
///
/// # Errors
/// [`NoRenderer`] for a column with an OMG mapping and no CST. There is
/// none: [`TargetLanguage::RENDERABLE`] is all four, and this returns a
/// `Result` because that set is a property of the tree rather than of
/// this function.
pub fn render_stub_op(op: &StubOp, language: TargetLanguage) -> Result<String, NoRenderer> {
    match language {
        TargetLanguage::Cpp => Ok(zerodds_lang_render::cpp::decl(&cpp::Decl::Function(
            cpp_stub_function(op),
        ))),
        TargetLanguage::Java => Ok(zerodds_lang_render::java::type_decl(
            &java::TypeDecl::Interface(java::Interface {
                doc: Doc::none(),
                annotations: Vec::new(),
                modifiers: alloc::vec![java::Modifier::Public],
                name: Ident::new("Stub"),
                generics: Vec::new(),
                extends: Vec::new(),
                members: alloc::vec![java::Member::Method(java_stub_method(op))],
            }),
        )),
        TargetLanguage::Rust => Ok(zerodds_lang_render::rust::item(&rust::Item::Function(
            rust_stub_function(op),
        ))),
        TargetLanguage::CSharp => Ok(zerodds_lang_render::csharp::type_decl(
            &csharp::TypeDecl::Interface(csharp::Interface {
                doc: Doc::none(),
                attributes: Vec::new(),
                modifiers: alloc::vec![csharp::Modifier::Public],
                name: Ident::new("Stub"),
                generics: Vec::new(),
                bases: Vec::new(),
                constraints: Vec::new(),
                members: alloc::vec![csharp::Member::Method(csharp_stub_method(op))],
            }),
        )),
    }
}

// ---------------------------------------------------------------------
// C++ — IDL to C++ `formal/2008-01-09` §1.22.
// ---------------------------------------------------------------------

/// The C++ stub declaration for an operation.
///
/// A namespace-scope function and not a class member, because the class
/// it belongs to is the generated interface's and this crate does not
/// build that: the caller puts the declaration where its own emitter
/// puts it.
#[must_use]
pub fn cpp_stub_function(op: &StubOp) -> cpp::Function {
    cpp::Function {
        doc: stub_doc(op),
        returns: cpp_return_type(&op.return_type),
        name: Ident::new(&op.name),
        params: op
            .params
            .iter()
            .map(|param| cpp::Param {
                type_: cpp_param_type(&param.type_, param.direction),
                name: Some(Ident::new(&param.name)),
                default: None,
            })
            .collect(),
        is_inline: false,
        is_static: false,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: false,
        body: cpp::FunctionBody::None,
    }
}

/// The C++ spelling of a parameter, by direction. §1.22, Table 1-3.
#[must_use]
pub fn cpp_param_type(type_: &StubType, direction: ParamDirection) -> cpp::Type {
    // A `typedef string Name` is written `const char*` and not
    // `const probe::Name&`: `string`'s C++ mapping is a pointer, not a
    // class, so there is nothing for the alias to name. Both ORBs
    // agree on the `in` and return positions
    // (`tests/orb-corpus/recorded/{omniorb,tao}.txt`, row 15); for the
    // `out` position they write two different typedefs of one type —
    // omniidl `probe::NameAlias_out`, tao_idl `CORBA::String_out` —
    // and this takes the second, which is the spec's own name.
    let type_ = match type_.resolved() {
        resolved @ StubType::Special(SpecialType::String | SpecialType::WString) => resolved,
        _ => type_,
    };
    // The two irregular rows. A `string` is not an aggregate and its
    // `out` form is a class with an assignment operator that frees the
    // old value, which is why it is not `char*&`.
    if let StubType::Special(SpecialType::String) = type_ {
        return match direction {
            ParamDirection::In => cpp_const_pointer_to_char(),
            ParamDirection::InOut => cpp::Type::Ref(AllocBox::new(cpp_pointer_to_char())),
            ParamDirection::Out => named_cpp("CORBA::String_out"),
        };
    }
    if let StubType::Special(SpecialType::WString) = type_ {
        return match direction {
            ParamDirection::In => cpp::Type::Pointer(AllocBox::new(cpp::Type::Const(
                AllocBox::new(named_cpp("CORBA::WChar")),
            ))),
            ParamDirection::InOut => cpp::Type::Ref(AllocBox::new(cpp::Type::Pointer(
                AllocBox::new(named_cpp("CORBA::WChar")),
            ))),
            ParamDirection::Out => named_cpp("CORBA::WString_out"),
        };
    }
    if let StubType::Special(SpecialType::NativeRef) = type_ {
        let pointer = cpp::Type::Pointer(AllocBox::new(cpp::Type::Void));
        return match direction {
            ParamDirection::In => pointer,
            ParamDirection::InOut | ParamDirection::Out => cpp::Type::Ref(AllocBox::new(pointer)),
        };
    }

    let base = cpp_base_name(type_);
    match (type_.shape(), direction) {
        (TypeShape::Scalar, ParamDirection::In) => named_cpp(&base),
        (TypeShape::Scalar, _) => cpp::Type::Ref(AllocBox::new(named_cpp(&base))),

        (TypeShape::FixedAggregate, ParamDirection::In) => cpp::Type::Ref(AllocBox::new(
            cpp::Type::Const(AllocBox::new(named_cpp(&base))),
        )),
        (TypeShape::FixedAggregate, _) => cpp::Type::Ref(AllocBox::new(named_cpp(&base))),

        // A sequence is passed exactly like any other variable-length
        // aggregate in C++; the two shapes differ only in Java.
        (TypeShape::VariableAggregate | TypeShape::Collection, ParamDirection::In) => {
            cpp::Type::Ref(AllocBox::new(cpp::Type::Const(AllocBox::new(named_cpp(
                &base,
            )))))
        }
        (TypeShape::VariableAggregate | TypeShape::Collection, ParamDirection::InOut) => {
            cpp::Type::Ref(AllocBox::new(named_cpp(&base)))
        }
        (TypeShape::VariableAggregate | TypeShape::Collection, ParamDirection::Out) => {
            named_cpp(&format!("{base}_out"))
        }

        // An array parameter is written as the array type itself; C++
        // adjusts it to a pointer to the first element, which is why
        // there is no reference here. §1.14.
        (TypeShape::Array { .. }, ParamDirection::In) => {
            cpp::Type::Const(AllocBox::new(named_cpp(&base)))
        }
        (TypeShape::Array { variable: false }, _) => named_cpp(&base),
        (TypeShape::Array { variable: true }, ParamDirection::InOut) => named_cpp(&base),
        (TypeShape::Array { variable: true }, ParamDirection::Out) => {
            named_cpp(&format!("{base}_out"))
        }

        (TypeShape::ObjectReference, ParamDirection::In) => named_cpp(&format!("{base}_ptr")),
        (TypeShape::ObjectReference, ParamDirection::InOut) => {
            cpp::Type::Ref(AllocBox::new(named_cpp(&format!("{base}_ptr"))))
        }
        (TypeShape::ObjectReference, ParamDirection::Out) => named_cpp(&format!("{base}_out")),

        (TypeShape::Valuetype, ParamDirection::In) => {
            cpp::Type::Pointer(AllocBox::new(named_cpp(&base)))
        }
        (TypeShape::Valuetype, ParamDirection::InOut) => cpp::Type::Ref(AllocBox::new(
            cpp::Type::Pointer(AllocBox::new(named_cpp(&base))),
        )),
        (TypeShape::Valuetype, ParamDirection::Out) => named_cpp(&format!("{base}_out")),
    }
}

/// The C++ spelling of a return type. §1.22, Table 1-3.
#[must_use]
pub fn cpp_return_type(type_: &StubType) -> cpp::Type {
    let type_ = match type_.resolved() {
        resolved @ StubType::Special(SpecialType::String | SpecialType::WString) => resolved,
        _ => type_,
    };
    match type_ {
        StubType::Void => return cpp::Type::Void,
        StubType::Special(SpecialType::String) => return cpp_pointer_to_char(),
        StubType::Special(SpecialType::WString) => {
            return cpp::Type::Pointer(AllocBox::new(named_cpp("CORBA::WChar")));
        }
        StubType::Special(SpecialType::NativeRef) => {
            return cpp::Type::Pointer(AllocBox::new(cpp::Type::Void));
        }
        _ => {}
    }
    let base = cpp_base_name(type_);
    match type_.shape() {
        TypeShape::Scalar | TypeShape::FixedAggregate => named_cpp(&base),
        // A variable-length result is heap-allocated and the caller owns
        // it, so it comes back as a pointer.
        TypeShape::VariableAggregate | TypeShape::Collection | TypeShape::Valuetype => {
            cpp::Type::Pointer(AllocBox::new(named_cpp(&base)))
        }
        // An array cannot be returned by value in C++; §1.14 returns a
        // pointer to its slice type.
        TypeShape::Array { .. } => {
            cpp::Type::Pointer(AllocBox::new(named_cpp(&format!("{base}_slice"))))
        }
        TypeShape::ObjectReference => named_cpp(&format!("{base}_ptr")),
    }
}

/// The unsuffixed C++ name a type's `_ptr`, `_out` and `_slice` forms are
/// built from.
///
/// For a special type it is the table's entry with the `_var` suffix
/// removed: `CORBA::Object_var` is the managed *member* type and
/// `CORBA::Object_ptr` the argument type, and both are built from
/// `CORBA::Object`.
fn cpp_base_name(type_: &StubType) -> String {
    match type_ {
        StubType::Special(t) => {
            let written = crate::language_mapping(*t, TargetLanguage::Cpp);
            written.strip_suffix("_var").unwrap_or(written).to_string()
        }
        StubType::Other { name, .. } => name.clone(),
        StubType::Void => "void".to_string(),
    }
}

fn named_cpp(written: &str) -> cpp::Type {
    let segments: Vec<&str> = written.split("::").collect();
    cpp::Type::named(Path::new(&segments))
}

fn cpp_pointer_to_char() -> cpp::Type {
    cpp::Type::Pointer(AllocBox::new(cpp::Type::Primitive(cpp::Primitive::Char)))
}

fn cpp_const_pointer_to_char() -> cpp::Type {
    cpp::Type::Pointer(AllocBox::new(cpp::Type::Const(AllocBox::new(
        cpp::Type::Primitive(cpp::Primitive::Char),
    ))))
}

// ---------------------------------------------------------------------
// Java — IDL to Java `formal/2008-01-04` §1.4.
// ---------------------------------------------------------------------

/// The Java stub declaration for an operation.
///
/// A method with no body: this is the signature the generated operations
/// interface declares and the stub class implements.
#[must_use]
pub fn java_stub_method(op: &StubOp) -> java::Method {
    java::Method {
        doc: stub_doc(op),
        annotations: Vec::new(),
        modifiers: Vec::new(),
        generics: Vec::new(),
        returns: java_return_type(&op.return_type),
        name: Ident::new(&zerodds_lang_cst::java::escape_keyword(&op.name)),
        params: op
            .params
            .iter()
            .map(|param| java::Param {
                annotations: Vec::new(),
                final_: false,
                type_: java_param_type(&param.type_, param.direction),
                name: Ident::new(&zerodds_lang_cst::java::escape_keyword(&param.name)),
                varargs: false,
            })
            .collect(),
        // §1.4.5: a user exception maps onto a checked Java exception
        // and appears in `throws`. This is the one column where
        // `raises` is part of the signature.
        throws: op
            .raises
            .iter()
            .map(|raised| java::Type::named(Path::new(&[raised.as_str()])))
            .collect(),
        body: None,
    }
}

/// The Java spelling of a parameter, by direction. §1.4.
///
/// `in` takes the mapped type. `out` and `inout` take a **holder**: Java
/// passes by value and has no reference parameters, so the mapping wraps
/// the value in a class the callee assigns into.
#[must_use]
pub fn java_param_type(type_: &StubType, direction: ParamDirection) -> java::Type {
    match direction {
        ParamDirection::In => java_value_type(type_),
        ParamDirection::Out | ParamDirection::InOut => java_holder_type(type_),
    }
}

/// The Java spelling of a return type. §1.4 — always the mapped type.
#[must_use]
pub fn java_return_type(type_: &StubType) -> java::Type {
    match type_ {
        StubType::Void => java::Type::Void,
        _ => java_value_type(type_),
    }
}

/// Java has no `typedef`, so an alias is written as what it aliases.
///
/// Measured: `typedef WithString StructAlias` is `probe.WithString` in a
/// JacORB signature and `typedef sequence<long> LongsAlias` is `int[]`
/// (`tests/orb-corpus/recorded/jacorb.txt`, row 15). The C++ column
/// writes `probe::StructAlias` for the same declaration.
fn java_value_type(type_: &StubType) -> java::Type {
    match type_.resolved() {
        StubType::Special(t) => crate::special_types::java_type(*t),
        StubType::Other { name, shape, .. } => match shape {
            TypeShape::Array { .. } => {
                java::Type::Array(AllocBox::new(java::Type::named(java_path(name))))
            }
            _ => java::Type::named(java_path(name)),
        },
        StubType::Void => java::Type::Void,
    }
}

/// The holder class an `out` or `inout` parameter takes.
///
/// The standard holders live in `org.omg.CORBA` and cover the special
/// types and the primitives. Everything else takes the `<Name>Holder`
/// class the IDL compiler generates beside the type — which is why an
/// `out` of a user type names a class that only exists once the IDL has
/// been compiled.
#[must_use]
pub fn java_holder_type(type_: &StubType) -> java::Type {
    // A `typedef` resolves through to the aliased type's holder, except
    // where the aliased type has no Java declaration of its own — a
    // `sequence` or a `fixed` — in which case the alias is where the
    // holder is generated. Both halves are measured
    // (`tests/orb-corpus/recorded/jacorb.txt`, rows 11 and 15).
    if let StubType::Other { name, .. } = type_
        && type_.names_its_own_java_holder()
    {
        return java::Type::named(java_path(&format!("{name}Holder")));
    }
    let type_ = type_.resolved();
    let written = match type_ {
        StubType::Special(t) => match t {
            SpecialType::Object => "org.omg.CORBA.ObjectHolder",
            SpecialType::ValueBase | SpecialType::AbstractBase => "org.omg.CORBA.ValueBaseHolder",
            SpecialType::TypeCode => "org.omg.CORBA.TypeCodeHolder",
            SpecialType::Any => "org.omg.CORBA.AnyHolder",
            SpecialType::SequenceOfAny => "org.omg.CORBA.AnySeqHolder",
            // Both take `StringHolder`, because both map to
            // `java.lang.String` and a holder is named after the type
            // it holds. Measured: JacORB writes
            // `void out_v(org.omg.CORBA.StringHolder v)` for
            // `out wstring v` (`tests/orb-corpus/recorded/jacorb.txt`,
            // row 09), not the `WStringHolder` the earlier table named.
            SpecialType::String | SpecialType::WString => "org.omg.CORBA.StringHolder",
            // `TimeT` and `unsigned long long` are both a Java `long`.
            SpecialType::Time | SpecialType::ULongLong => "org.omg.CORBA.LongHolder",
            SpecialType::Fixed => "org.omg.CORBA.FixedHolder",
            // `long double` is a Java `double` (§1.3.1), so its holder
            // is `DoubleHolder` and the low eight bytes are lost with
            // it. The type table records the same loss.
            SpecialType::LongDouble => "org.omg.CORBA.DoubleHolder",
            // A `native` has no standard holder; the compiler generates
            // one beside the declaration.
            SpecialType::NativeRef => "org.omg.CORBA.AnyHolder",
        },
        StubType::Other { name, .. } => {
            return java::Type::named(java_path(&format!("{name}Holder")));
        }
        StubType::Void => return java::Type::Void,
    };
    java::Type::named(java_path(written))
}

/// A Java path from a name written with either separator.
fn java_path(name: &str) -> Path {
    let dotted = name.replace("::", ".");
    let segments: Vec<&str> = dotted.split('.').collect();
    Path::new(&segments)
}

// ---------------------------------------------------------------------
// C# — IDL to C# `formal/2003-09-04` §1.5 and Annex A.
// ---------------------------------------------------------------------

/// The C# stub declaration for an operation.
///
/// An interface method with no body: the signature the generated
/// operations interface declares and the stub class implements, which is
/// the Java column's shape and not the C++ one — C# has no free
/// functions.
///
/// `raises` is documentation and not a `throws` clause. C# has no
/// checked exceptions at all, so there is nothing on the signature for a
/// user exception to reach; the C++ column is in the same position and
/// takes the same answer. Java is the one column where `raises` is part
/// of the signature.
#[must_use]
pub fn csharp_stub_method(op: &StubOp) -> csharp::Method {
    csharp::Method {
        doc: stub_doc(op),
        attributes: Vec::new(),
        modifiers: Vec::new(),
        returns: csharp_return_type(&op.return_type),
        name: Ident::new(&csharp::escape_keyword(&op.name)),
        generics: Vec::new(),
        params: op
            .params
            .iter()
            .map(|param| csharp::Param {
                attributes: Vec::new(),
                modifier: csharp_param_modifier(param.direction),
                type_: csharp_param_type(&param.type_, param.direction),
                name: Ident::new(&csharp::escape_keyword(&param.name)),
                default: None,
            })
            .collect(),
        constraints: Vec::new(),
        body: None,
        expression: None,
    }
}

/// The C# passing mode of a direction. §1.5.
///
/// `in` is C#'s default and takes no modifier — deliberately not
/// [`csharp::ParamModifier::In`], which is C# 7.2's read-only reference
/// and means something else: it forbids assignment inside the callee and
/// binds an lvalue at the call site. `out` and `ref` are the two the
/// mapping names, and they are the reason the C# column needs no holder
/// classes the way Java does.
#[must_use]
pub const fn csharp_param_modifier(direction: ParamDirection) -> Option<csharp::ParamModifier> {
    match direction {
        ParamDirection::In => None,
        ParamDirection::Out => Some(csharp::ParamModifier::Out),
        ParamDirection::InOut => Some(csharp::ParamModifier::Ref),
    }
}

/// The C# spelling of a parameter. §1.5.
///
/// One type for all three directions: the direction is carried by the
/// modifier [`csharp_param_modifier`] returns, not by the type. That is
/// the whole difference from the other three columns — C++ spells the
/// direction into the type (`const T&`, `T_out`), Java routes `out` and
/// `inout` through a holder class, Rust through `&mut`.
#[must_use]
pub fn csharp_param_type(type_: &StubType, direction: ParamDirection) -> csharp::Type {
    let _ = direction;
    csharp_value_type(type_)
}

/// The C# spelling of a return type. §1.5 — always the mapped type.
#[must_use]
pub fn csharp_return_type(type_: &StubType) -> csharp::Type {
    match type_ {
        StubType::Void => csharp::Type::Predefined(csharp::Predefined::Void),
        _ => csharp_value_type(type_),
    }
}

/// C# has no `typedef`, so an alias is written as what it aliases.
///
/// The Java treatment and not the C++ one, for the same reason: a `using`
/// alias in C# is file-scoped and is not a type, so a generated
/// declaration has nothing to name. Unlike Java this is **cited and not
/// measured** — the C# column has an OMG mapping and no compiler on the
/// measurement host, which the module documentation says of the type
/// table too.
fn csharp_value_type(type_: &StubType) -> csharp::Type {
    match type_.resolved() {
        StubType::Special(t) => crate::special_types::csharp_type(*t),
        StubType::Other { name, shape, .. } => match shape {
            TypeShape::Array { .. } => csharp::Type::array(csharp::Type::named(csharp_path(name))),
            _ => csharp::Type::named(csharp_path(name)),
        },
        StubType::Void => csharp::Type::Predefined(csharp::Predefined::Void),
    }
}

/// A C# path from a name written with either separator.
fn csharp_path(name: &str) -> Path {
    let dotted = name.replace("::", ".");
    let segments: Vec<&str> = dotted.split('.').collect();
    Path::new(&segments)
}

// ---------------------------------------------------------------------
// Rust — this project's mapping. The OMG has none.
// ---------------------------------------------------------------------

/// The Rust stub declaration for an operation.
///
/// `in` takes a value for a scalar and a shared reference otherwise;
/// `out` and `inout` take a unique reference, which is Rust's holder and
/// needs no class. The signature has no body: the marshalling is the ORB
/// crate's.
#[must_use]
pub fn rust_stub_function(op: &StubOp) -> rust::Function {
    rust::Function {
        doc: stub_doc(op),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&rust::escape_keyword(&op.name)),
        generics: Vec::new(),
        params: core::iter::once(rust::Param {
            name: None,
            type_: rust::Type::unit(),
            receiver: Some(rust::Receiver::Ref),
        })
        .chain(op.params.iter().map(|param| rust::Param {
            name: Some(Ident::new(&rust::escape_keyword(&param.name))),
            type_: rust_param_type(&param.type_, param.direction),
            receiver: None,
        }))
        .collect(),
        returns: match &op.return_type {
            StubType::Void => None,
            other => Some(rust_value_type(other)),
        },
        body: None,
    }
}

/// The Rust spelling of a parameter, by direction.
#[must_use]
pub fn rust_param_type(type_: &StubType, direction: ParamDirection) -> rust::Type {
    let value = rust_value_type(type_);
    match direction {
        ParamDirection::In if matches!(type_.shape(), TypeShape::Scalar) => value,
        ParamDirection::In => rust::Type::Ref {
            target: AllocBox::new(value),
            mutable: false,
            lifetime: None,
        },
        ParamDirection::Out | ParamDirection::InOut => rust::Type::Ref {
            target: AllocBox::new(value),
            mutable: true,
            lifetime: None,
        },
    }
}

fn rust_value_type(type_: &StubType) -> rust::Type {
    // Rust *has* a type alias, so the declaration's own name is what a
    // generated module brings and what a signature writes — the C++
    // treatment and not the Java one.
    match type_ {
        StubType::Special(t) => crate::special_types::rust_type(*t),
        StubType::Other { name, .. } => {
            let last = name.rsplit("::").next().unwrap_or(name);
            rust::Type::named(Path::single(&rust::escape_keyword(last)))
        }
        StubType::Void => rust::Type::unit(),
    }
}

/// The documentation every column's declaration carries.
///
/// `oneway` and `raises` are stated here in every column, and in Java
/// they are stated *and* in the `throws` clause. A reader of generated
/// C++ has no other place to learn that an operation raises anything.
fn stub_doc(op: &StubOp) -> Doc {
    let mut doc = Doc::line(&if op.oneway {
        format!("`oneway {}` — no reply is expected.", op.name)
    } else {
        format!("`{}`", op.name)
    });
    if !op.raises.is_empty() {
        doc.push(&format!("Raises: {}.", op.raises.join(", ")));
    }
    doc
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn cpp_text(type_: &StubType, direction: ParamDirection) -> String {
        zerodds_lang_render::cpp::type_(&cpp_param_type(type_, direction))
    }

    fn cpp_return_text(type_: &StubType) -> String {
        zerodds_lang_render::cpp::type_(&cpp_return_type(type_))
    }

    fn java_text(type_: &StubType, direction: ParamDirection) -> String {
        zerodds_lang_render::java::type_(&java_param_type(type_, direction))
    }

    #[test]
    fn cpp_string_takes_four_different_spellings() {
        let string = StubType::Special(SpecialType::String);
        assert_eq!(
            cpp_text(&string, ParamDirection::In).replace(' ', ""),
            "constchar*"
        );
        assert_eq!(
            cpp_text(&string, ParamDirection::InOut).replace(' ', ""),
            "char*&"
        );
        assert_eq!(cpp_text(&string, ParamDirection::Out), "CORBA::String_out");
        assert_eq!(cpp_return_text(&string).replace(' ', ""), "char*");
    }

    #[test]
    fn cpp_wstring_takes_the_wchar_forms_and_not_the_char_ones() {
        let wstring = StubType::Special(SpecialType::WString);
        assert_eq!(
            cpp_text(&wstring, ParamDirection::In).replace(' ', ""),
            "constCORBA::WChar*"
        );
        assert_eq!(
            cpp_text(&wstring, ParamDirection::Out),
            "CORBA::WString_out"
        );
    }

    #[test]
    fn a_variable_length_struct_returns_a_pointer_and_a_fixed_one_a_value() {
        let variable = StubType::other("demo::WithString", TypeShape::VariableAggregate);
        let fixed = StubType::other("demo::TwoLongs", TypeShape::FixedAggregate);
        assert_eq!(
            cpp_return_text(&variable).replace(' ', ""),
            "demo::WithString*"
        );
        assert_eq!(cpp_return_text(&fixed), "demo::TwoLongs");
        // And their `out` forms differ for the same reason.
        assert_eq!(
            cpp_text(&variable, ParamDirection::Out),
            "demo::WithString_out"
        );
        assert_eq!(
            cpp_text(&fixed, ParamDirection::Out).replace(' ', ""),
            "demo::TwoLongs&"
        );
    }

    #[test]
    fn an_object_reference_argument_is_a_ptr_and_its_out_is_an_out() {
        let reference = StubType::other("demo::Echo", TypeShape::ObjectReference);
        assert_eq!(cpp_text(&reference, ParamDirection::In), "demo::Echo_ptr");
        assert_eq!(
            cpp_text(&reference, ParamDirection::InOut).replace(' ', ""),
            "demo::Echo_ptr&"
        );
        assert_eq!(cpp_text(&reference, ParamDirection::Out), "demo::Echo_out");
        assert_eq!(cpp_return_text(&reference), "demo::Echo_ptr");
    }

    #[test]
    fn the_generic_object_uses_the_corba_names_without_the_var_suffix() {
        let object = StubType::Special(SpecialType::Object);
        assert_eq!(cpp_text(&object, ParamDirection::In), "CORBA::Object_ptr");
        assert_eq!(cpp_text(&object, ParamDirection::Out), "CORBA::Object_out");
    }

    #[test]
    fn java_out_and_inout_take_a_holder_and_in_does_not() {
        let string = StubType::Special(SpecialType::String);
        assert_eq!(java_text(&string, ParamDirection::In), "java.lang.String");
        assert_eq!(
            java_text(&string, ParamDirection::Out),
            "org.omg.CORBA.StringHolder"
        );
        assert_eq!(
            java_text(&string, ParamDirection::InOut),
            "org.omg.CORBA.StringHolder"
        );
    }

    #[test]
    fn a_user_type_takes_the_generated_holder_beside_it() {
        let user = StubType::other("demo::Order", TypeShape::VariableAggregate);
        assert_eq!(java_text(&user, ParamDirection::Out), "demo.OrderHolder");
    }

    #[test]
    fn java_long_double_loses_its_low_bytes_in_both_the_type_and_the_holder() {
        let long_double = StubType::Special(SpecialType::LongDouble);
        assert_eq!(java_text(&long_double, ParamDirection::In), "double");
        assert_eq!(
            java_text(&long_double, ParamDirection::Out),
            "org.omg.CORBA.DoubleHolder"
        );
    }

    #[test]
    fn rust_in_is_by_value_for_a_scalar_and_by_reference_otherwise() {
        let scalar = StubType::Special(SpecialType::ULongLong);
        let aggregate = StubType::Special(SpecialType::Any);
        assert_eq!(
            zerodds_lang_render::rust::type_(&rust_param_type(&scalar, ParamDirection::In)),
            "u64"
        );
        assert_eq!(
            zerodds_lang_render::rust::type_(&rust_param_type(&aggregate, ParamDirection::In)),
            "&zerodds_corba_rt::CorbaAny"
        );
        assert_eq!(
            zerodds_lang_render::rust::type_(&rust_param_type(&aggregate, ParamDirection::Out)),
            "&mut zerodds_corba_rt::CorbaAny"
        );
    }

    #[test]
    fn every_column_renders_and_none_is_refused() {
        let op = StubOp {
            name: "echo".to_string(),
            return_type: StubType::Special(SpecialType::String),
            params: Vec::new(),
            oneway: false,
            raises: Vec::new(),
        };
        for language in TargetLanguage::ALL {
            assert!(render_stub_op(&op, language).is_ok(), "{language:?}");
        }
        let csharp = render_stub_op(&op, TargetLanguage::CSharp).expect("csharp");
        assert!(csharp.contains("string echo()"), "{csharp}");
    }

    #[test]
    fn csharp_carries_the_direction_on_the_modifier_and_not_on_the_type() {
        let op = StubOp {
            name: "swap".to_string(),
            return_type: StubType::Void,
            params: alloc::vec![
                StubParam {
                    name: "a".to_string(),
                    type_: StubType::other("probe::Point", TypeShape::FixedAggregate),
                    direction: ParamDirection::Out,
                },
                StubParam {
                    name: "b".to_string(),
                    type_: StubType::other("probe::Point", TypeShape::FixedAggregate),
                    direction: ParamDirection::InOut,
                },
                StubParam {
                    name: "c".to_string(),
                    type_: StubType::other("probe::Point", TypeShape::FixedAggregate),
                    direction: ParamDirection::In,
                },
            ],
            oneway: false,
            raises: Vec::new(),
        };
        let text = render_stub_op(&op, TargetLanguage::CSharp).expect("csharp");
        assert!(text.contains("out probe.Point a"), "{text}");
        assert!(text.contains("ref probe.Point b"), "{text}");
        assert!(text.contains("probe.Point c"), "{text}");
        // The `in` direction takes no modifier: C#'s `in` is a
        // read-only reference and is a different thing.
        assert!(!text.contains("in probe.Point"), "{text}");
    }

    #[test]
    fn csharp_resolves_a_typedef_the_way_java_does() {
        let aliased = StubType::alias("probe::Name", StubType::Special(SpecialType::String));
        assert_eq!(
            zerodds_lang_render::csharp::type_(&csharp_return_type(&aliased)),
            "string"
        );
    }

    #[test]
    fn a_oneway_operation_says_so_in_every_column_that_renders() {
        let op = StubOp {
            name: "notify".to_string(),
            return_type: StubType::Void,
            params: alloc::vec![StubParam {
                name: "msg".to_string(),
                type_: StubType::Special(SpecialType::String),
                direction: ParamDirection::In,
            }],
            oneway: true,
            raises: Vec::new(),
        };
        for language in TargetLanguage::RENDERABLE {
            let text = render_stub_op(&op, language).expect("renderable");
            assert!(text.contains("oneway"), "{language:?}: {text}");
        }
    }

    #[test]
    fn raises_reaches_the_java_throws_clause() {
        let op = StubOp {
            name: "withdraw".to_string(),
            return_type: StubType::Void,
            params: Vec::new(),
            oneway: false,
            raises: alloc::vec!["InsufficientFunds".to_string()],
        };
        let text = render_stub_op(&op, TargetLanguage::Java).expect("renderable");
        assert!(text.contains("throws InsufficientFunds"), "{text}");
    }
}
