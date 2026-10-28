// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Constants and expression trees. Spec §3.5.

use crate::facet::Facets;
use crate::node::member::EnumeratorNode;
use crate::node::module::ModuleNode;
use crate::node::type_::TypeNode;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use zerodds_ast_core::{Id, NodeId, Ref};

/// A named constant. Spec §3.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Enclosing module.
    pub module: Id<ModuleNode>,
    /// The interface or value type this is declared in, when it is not
    /// declared directly in a module (OMG IDL 4.2 §7.4.3.4: an interface
    /// body holds type, constant and exception declarations). Its name is
    /// a segment of the scoped name and of the repository id:
    /// `CosNaming::NamingContext::NotFound`.
    pub scope: Option<String>,
    /// Declared type.
    pub type_: Ref<TypeNode>,
    /// The defining expression, retained as a tree.
    pub expr: Id<ConstExprNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A literal value.
///
/// Floating-point and fixed-point literals retain their written form:
/// `1.` and `1.0` are the same number but not the same source text, and
/// the formatter has to render back what was written. Spec §3.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// `TRUE` / `FALSE`.
    Boolean(bool),
    /// Integer literal, in its evaluated form; the written radix lives
    /// in the source facet.
    Integer(i128),
    /// Floating-point literal, as written.
    Float {
        /// Mantissa exactly as written, including any trailing point.
        mantissa: String,
        /// Exponent as written, without the `e`.
        exponent: Option<String>,
    },
    /// Fixed-point literal, as written.
    Fixed {
        /// Digits as written, without the point.
        digits: String,
        /// Digits after the point.
        scale: u8,
    },
    /// Narrow character literal.
    Char(char),
    /// Wide character literal, as a code point.
    WChar(u32),
    /// Narrow string literal, with escapes resolved.
    String(String),
    /// Wide string literal, as code points.
    WString(Vec<u32>),
    /// An enumerator used as a value.
    Enumerator(Ref<EnumeratorNode>),
}

/// Unary operator. Spec §3.5, IDL 4.2 §7.4.1.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `~`
    Not,
}

impl UnaryOp {
    /// Every unary operator IDL 4.2 defines.
    pub const ALL: [Self; 3] = [Self::Plus, Self::Minus, Self::Not];

    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Not => "~",
        }
    }
}

/// Binary operator. Spec §3.5, IDL 4.2 §7.4.1.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// `|`
    Or,
    /// `^`
    Xor,
    /// `&`
    And,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
}

impl BinaryOp {
    /// Every binary operator IDL 4.2 defines, in precedence order from
    /// loosest to tightest.
    pub const ALL: [Self; 10] = [
        Self::Or,
        Self::Xor,
        Self::And,
        Self::Shl,
        Self::Shr,
        Self::Add,
        Self::Sub,
        Self::Mul,
        Self::Div,
        Self::Mod,
    ];

    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Or => "|",
            Self::Xor => "^",
            Self::And => "&",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
        }
    }
}

/// One node of a constant expression tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstExpr {
    /// A literal value.
    Literal(Literal),
    /// A reference to another constant by name.
    NameRef(Ref<ConstNode>),
    /// Unary application.
    Unary {
        /// The operator.
        op: UnaryOp,
        /// Its operand.
        operand: Id<ConstExprNode>,
    },
    /// Binary application.
    Binary {
        /// The operator.
        op: BinaryOp,
        /// Left operand.
        lhs: Id<ConstExprNode>,
        /// Right operand.
        rhs: Id<ConstExprNode>,
    },
}

/// An expression node.
///
/// Expressions live in their own arena, so an expression referenced from
/// a bound, an array dimension, a case label, a `@default` or an
/// enumerator value is a first-class node with its own identity and its
/// own source facet. Spec §3.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstExprNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// The expression at this node.
    pub expr: ConstExpr,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// Why an expression could not be evaluated.
///
/// Evaluation of a partially discovered model fails loudly; it never
/// substitutes a default. Spec §3.5.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvalError {
    /// The tree reached a reference that is not resolved.
    Unresolved {
        /// The expression node that could not be followed.
        at: NodeId,
    },
    /// A handle in the tree did not address a live expression node.
    DanglingExpr,
    /// Division or remainder by zero.
    DivisionByZero {
        /// The expression node performing the division.
        at: NodeId,
    },
    /// The computation left the range of `i128`.
    Overflow {
        /// The expression node that overflowed.
        at: NodeId,
    },
    /// A shift distance was negative or wider than the value.
    InvalidShift {
        /// The expression node performing the shift.
        at: NodeId,
    },
    /// The literal is not integral, so integral evaluation cannot apply.
    NotIntegral {
        /// The expression node holding the literal.
        at: NodeId,
    },
    /// The expression nests deeper than evaluation will follow. Bounds
    /// the work a hostile snapshot can demand.
    TooDeep,
    /// An infix operator combines a floating-point operand with one that
    /// is not. IDL 4.2 §7.4.1.4.2: "An infix operator may combine two
    /// integer types, floating point types or fixed point types, but not
    /// mixtures of these."
    MixedOperands {
        /// The expression node with the mixed operands.
        at: NodeId,
    },
    /// An operator other than unary `+ -` and binary `* / + -` over
    /// floating-point operands. IDL 4.2 §7.4.1.4.2 admits only those.
    NotFloatingOperator {
        /// The expression node performing the operation.
        at: NodeId,
    },
    /// An operator other than unary `+ -` and binary `* / + -` over
    /// fixed-point operands. IDL 4.2 §7.4.1.4.2 admits only those.
    NotFixedOperator {
        /// The expression node performing the operation.
        at: NodeId,
    },
    /// The arithmetic would have to run in `long double`: the constant
    /// is one, an operand is one, or a literal is too large for `double`
    /// and so is one (IDL 4.2 §7.4.1.4.2). This build has no IEEE
    /// double-extended or binary128 arithmetic to run it in, and doing it
    /// in `double` instead would round where the clause says not to.
    LongDouble {
        /// The expression node that needs it.
        at: NodeId,
    },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolved { at } => write!(f, "expression {at:?} reaches an unresolved name"),
            Self::DanglingExpr => write!(f, "expression tree contains a dangling handle"),
            Self::DivisionByZero { at } => write!(f, "division by zero in expression {at:?}"),
            Self::Overflow { at } => write!(f, "arithmetic overflow in expression {at:?}"),
            Self::InvalidShift { at } => write!(f, "invalid shift distance in expression {at:?}"),
            Self::NotIntegral { at } => write!(f, "expression {at:?} is not integral"),
            Self::TooDeep => write!(f, "expression nests too deeply to evaluate"),
            Self::MixedOperands { at } => write!(
                f,
                "expression {at:?} combines a floating-point operand with one that is not"
            ),
            Self::NotFloatingOperator { at } => write!(
                f,
                "expression {at:?} applies an operator floating-point values do not take"
            ),
            Self::NotFixedOperator { at } => write!(
                f,
                "expression {at:?} applies an operator fixed-point values do not take"
            ),
            Self::LongDouble { at } => write!(
                f,
                "expression {at:?} needs long double arithmetic, which this build does not have"
            ),
        }
    }
}

impl core::error::Error for EvalError {}

/// How deep integral evaluation will follow an expression tree.
///
/// The limit exists because a snapshot can be hostile; without it, a
/// crafted tree would make evaluation run for an unbounded time.
pub const MAX_EXPR_DEPTH: usize = 256;

/// A constant expression's value, in the form the expression had.
///
/// [`Ast::eval_integral`] answers for the constants that are numbers a
/// wire format cares about, and returns [`EvalError::NotIntegral`] for the
/// rest. That is the right answer where an integer is required — an array
/// bound, a `@bit_bound`, a case label — and the wrong one where the
/// question is only "what does this constant say": `const string
/// MODULE_ID = "spatial.sensing.lidar/1.5";` is a perfectly evaluated
/// constant that no integer represents, and a backend that asked the
/// integral question could not emit it at all.
///
/// [`Ast::eval_integral`]: crate::Ast::eval_integral
/// [`EvalError::NotIntegral`]: crate::EvalError::NotIntegral
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ConstValue {
    /// `TRUE` / `FALSE`.
    Boolean(bool),
    /// An integer, including a character or an enumerator evaluated as
    /// one, and every arithmetic expression over them.
    Integer(i128),
    /// A floating-point literal, in the two halves it was written in.
    ///
    /// Kept as text: the decimal a source wrote and the nearest `f64` are
    /// two different values, and which one a backend needs is the
    /// backend's business.
    Float {
        /// Mantissa exactly as written, including any trailing point.
        mantissa: String,
        /// Exponent as written, without the `e`.
        exponent: Option<String>,
    },
    /// A fixed-point literal — a decimal, and never a binary float.
    Fixed {
        /// Digits as written, without the point.
        digits: String,
        /// Digits after the point.
        scale: u8,
    },
    /// A narrow character.
    Char(char),
    /// A wide character, as a code point.
    WChar(u32),
    /// A narrow string, with escapes resolved.
    String(String),
    /// A wide string, as code points.
    WString(Vec<u32>),
}
