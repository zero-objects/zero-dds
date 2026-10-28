// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust column's union, in one place for the declaration
//! (`adapter::cst`), the codec (`encode`) and the default
//! (`default_rust`).
//!
//! A union is an enum with one variant per case. The value of a union is
//! its discriminator and the member it selects (IDL 4.2 §7.4.1.4.4.4), and
//! the enum keeps both:
//!
//! - a case with one label fixes the discriminator, so its variant holds
//!   the member alone: `M(i32)`;
//! - a case with several labels, and the `default:` case, do not, so their
//!   variant holds the discriminator first: `M(i32, i32)`. Every other
//!   column keeps the discriminator in its own field; this column dropped
//!   it there, and a value read under the second label of a case, or under
//!   any value the `default:` case takes, went out again under the first
//!   label or under a value of the column's choosing;
//! - a union without `default:` whose labels do not cover its
//!   discriminator's values can hold a discriminator that selects no
//!   member at all. The variant [`NO_MEMBER`] holds that discriminator.
//!   Its name starts with `_`, which no IDL identifier does, so it meets
//!   no case's variant.

use zerodds_ast::{Ast, CaseLabel, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Literal, Path};
use zerodds_lang_cst::rust;

/// The variant of a union whose discriminator selects no member.
pub(crate) const NO_MEMBER: &str = "_NoMember";

/// What a discriminator's labels are written as in Rust source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiscriminatorKind {
    /// `bool`: `true`, `false`.
    Boolean,
    /// `char` for `char` and `wchar`: a character literal.
    Character,
    /// An enumeration: the path to the enumerator.
    Enumeration(Id<TypeNode>),
    /// Every integer type: an integer literal.
    Integer,
}

/// One case of a union, as the enum holds it.
#[derive(Debug)]
pub(crate) struct RustCase<'a> {
    /// The case's member.
    pub(crate) member: &'a MemberNode,
    /// Its variant's name.
    pub(crate) variant: String,
    /// The values of its explicit labels, in declaration order.
    pub(crate) labels: Vec<i128>,
    /// Whether it is the `default:` case.
    pub(crate) is_default: bool,
}

impl RustCase<'_> {
    /// Whether the variant holds the discriminator ahead of the member.
    pub(crate) fn carries_discriminator(&self) -> bool {
        self.is_default || self.labels.len() > 1
    }
}

/// A union as the Rust column materialises it.
#[derive(Debug)]
pub(crate) struct RustUnion<'a> {
    /// The discriminator's type, aliases unwrapped.
    pub(crate) discriminator: Id<TypeNode>,
    /// How its labels are written.
    pub(crate) kind: DiscriminatorKind,
    /// The cases, in declaration order.
    pub(crate) cases: Vec<RustCase<'a>>,
    /// Whether a discriminator may select no member ([`NO_MEMBER`]).
    pub(crate) no_member: bool,
}

impl RustUnion<'_> {
    /// The value of every explicit label, of every case.
    pub(crate) fn all_labels(&self) -> Vec<i128> {
        self.cases
            .iter()
            .flat_map(|case| case.labels.iter().copied())
            .collect()
    }
}

/// `union_type`'s shape, or why it has none in this column.
///
/// # Errors
/// A label that does not evaluate, a case with no label, or an unresolved
/// discriminator or member type.
pub(crate) fn shape(ast: &Ast, union_type: Id<TypeNode>) -> Result<RustUnion<'_>, &'static str> {
    let node = ast
        .type_(union_type)
        .map_err(|_| "a union the model does not hold")?;
    let TypeKind::Union {
        discriminator,
        cases,
        default_case,
    } = &node.kind
    else {
        return Err("not a union");
    };
    let discriminator = unalias(
        ast,
        discriminator
            .resolved()
            .ok_or("a union with an unresolved discriminator")?,
    )?;
    let kind = match ast.type_(discriminator).map(|t| &t.kind) {
        Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => DiscriminatorKind::Boolean,
        Ok(TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar)) => {
            DiscriminatorKind::Character
        }
        Ok(TypeKind::Enumeration { .. }) => DiscriminatorKind::Enumeration(discriminator),
        Ok(TypeKind::Primitive(_)) => DiscriminatorKind::Integer,
        _ => {
            return Err(
                "a union whose discriminator is no integer, character, boolean or enumeration",
            );
        }
    };
    let mut rust_cases = Vec::new();
    for case in cases {
        let case_node = ast
            .case(*case)
            .map_err(|_| "a union case the model does not hold")?;
        let member = ast
            .member(case_node.member)
            .map_err(|_| "a union case without its member")?;
        let mut labels = Vec::new();
        let mut is_default = *default_case == Some(*case);
        for label in &case_node.labels {
            match label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => labels.push(
                    ast.eval_integral(*expr)
                        .map_err(|_| "a union case whose label does not evaluate")?,
                ),
            }
        }
        if !is_default && labels.is_empty() {
            return Err("a union case carrying no case label");
        }
        rust_cases.push(RustCase {
            member,
            variant: crate::adapter::cst::variant_name(&member.name),
            labels,
            is_default,
        });
    }
    let has_default = rust_cases.iter().any(|case| case.is_default);
    let mut distinct: Vec<i128> = rust_cases
        .iter()
        .flat_map(|case| case.labels.iter().copied())
        .collect();
    distinct.sort_unstable();
    distinct.dedup();
    let covered = domain_size(ast, discriminator, kind)
        .is_some_and(|size| u128::try_from(distinct.len()).is_ok_and(|n| n >= size));
    Ok(RustUnion {
        discriminator,
        kind,
        cases: rust_cases,
        no_member: !has_default && !covered,
    })
}

/// How many values the discriminator's type has, where that is few
/// enough for labels to cover: `bool`, 8- and 16-bit types, and an
/// enumeration's enumerators.
fn domain_size(ast: &Ast, discriminator: Id<TypeNode>, kind: DiscriminatorKind) -> Option<u128> {
    match kind {
        DiscriminatorKind::Boolean => Some(2),
        DiscriminatorKind::Enumeration(enumeration) => {
            u128::try_from(ast.enumerators_of(enumeration).len()).ok()
        }
        DiscriminatorKind::Character | DiscriminatorKind::Integer => {
            match ast.type_(discriminator).map(|t| &t.kind) {
                Ok(TypeKind::Primitive(
                    PrimitiveKind::Char
                    | PrimitiveKind::Octet
                    | PrimitiveKind::Int8
                    | PrimitiveKind::UInt8,
                )) => Some(256),
                Ok(TypeKind::Primitive(
                    PrimitiveKind::WChar | PrimitiveKind::Int16 | PrimitiveKind::UInt16,
                )) => Some(65_536),
                _ => None,
            }
        }
    }
}

/// `type_` with its aliases unwrapped.
fn unalias(ast: &Ast, mut type_: Id<TypeNode>) -> Result<Id<TypeNode>, &'static str> {
    for _ in 0..64 {
        match ast.type_(type_).map(|t| &t.kind) {
            Ok(TypeKind::Alias { aliased }) => {
                type_ = aliased
                    .resolved()
                    .ok_or("a discriminator alias that does not resolve")?;
            }
            Ok(_) => return Ok(type_),
            Err(_) => return Err("a discriminator the model does not hold"),
        }
    }
    Err("a discriminator alias chain longer than 64")
}

/// The label `value` as Rust source, written from module `from`: an
/// integer, `true`/`false`, a character, or the enumerator's path.
///
/// # Errors
/// A character outside Unicode, or an enumeration with no enumerator of
/// that value.
pub(crate) fn label_literal(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    kind: DiscriminatorKind,
    value: i128,
) -> Result<LabelSource, &'static str> {
    Ok(match kind {
        DiscriminatorKind::Integer => LabelSource::Literal(Literal::int(value)),
        DiscriminatorKind::Boolean => LabelSource::Literal(Literal::Bool(value != 0)),
        DiscriminatorKind::Character => LabelSource::Literal(Literal::Char(
            u32::try_from(value)
                .ok()
                .and_then(char::from_u32)
                .ok_or("a character label outside Unicode")?,
        )),
        DiscriminatorKind::Enumeration(enumeration) => {
            LabelSource::Path(enumerator_path(ast, from, enumeration, value)?)
        }
    })
}

/// A label in Rust source: a literal, or a path to an enumerator.
#[derive(Debug, Clone)]
pub(crate) enum LabelSource {
    /// `3`, `true`, `'a'`.
    Literal(Literal),
    /// `Color::RED`.
    Path(Path),
}

impl LabelSource {
    /// As an expression.
    pub(crate) fn expr(&self) -> rust::Expr {
        match self {
            Self::Literal(literal) => rust::Expr::Literal(literal.clone()),
            Self::Path(path) => rust::Expr::Path(path.clone()),
        }
    }

    /// As a pattern.
    pub(crate) fn pattern(&self) -> rust::Pattern {
        match self {
            Self::Literal(literal) => rust::Pattern::Literal(literal.clone()),
            Self::Path(path) => rust::Pattern::Path(path.clone()),
        }
    }
}

/// The path, from module `from`, to the enumerator of `enumeration`
/// whose value is `value`.
pub(crate) fn enumerator_path(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    enumeration: Id<TypeNode>,
    value: i128,
) -> Result<Path, &'static str> {
    let enumerator = ast
        .enumerators_of(enumeration)
        .iter()
        .copied()
        .find(|e| ast.effective_enumerator_value(*e) == Some(value))
        .ok_or("an enumeration label with no enumerator of its value")?;
    enumerator_path_of(ast, from, enumeration, enumerator)
}

/// The path of one enumerator, from the enumerator itself.
///
/// A carried enumeration discriminator matches each enumerator against
/// the integer that crosses the wire, which is not the declared value
/// under every setting (`zerodds_ast_layout::EnumeratorValue`); the
/// variant is named from the enumerator and not looked up by a value.
pub(crate) fn enumerator_path_of(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    enumeration: Id<TypeNode>,
    enumerator: Id<zerodds_ast::EnumeratorNode>,
) -> Result<Path, &'static str> {
    let node = ast
        .type_(enumeration)
        .map_err(|_| "an enumeration the model does not hold")?;
    let enumerator = ast
        .enumerator(enumerator)
        .map_err(|_| "an enumerator the model does not hold")?;
    let path = crate::adapter::cst::relative_path(ast, from, node)
        .map_err(|_| "an enumeration the model has no module for")?;
    let mut segments: Vec<String> = path
        .segments()
        .iter()
        .map(|s| s.as_str().to_string())
        .collect();
    segments.push(zerodds_lang_cst::rust::escape_keyword(&enumerator.name));
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(Path::new(&borrowed))
}
