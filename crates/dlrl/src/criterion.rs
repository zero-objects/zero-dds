// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DLRL queries and filters (DDS 1.2 Annex B).
//!
//! Annex B is the syntax of the query of a `QueryCriterion`
//! (§8.1.6.3.12) and of the `content_filter` of an `ObjectHome`
//! (§8.1.6.3.7). A [`QueryExpression`] is such an expression with its
//! parameters, parsed with the grammar of Annex B B.1 (through
//! [`zerodds_sql_filter::Grammar::Dlrl12`]) and checked against the
//! class it applies to:
//!
//! * every FIELDNAME designates a value of the class: `.` navigates into
//!   a simple structure and across a relation, `[n]` into a `List`, an
//!   `IntMap` or a `sequence`, `['key']` into a `StrMap` (B.2); the
//!   value at the end is of a basic type or an enumeration;
//! * a constant fits the field it is compared with: an INTEGERVALUE or
//!   FLOATVALUE a number, a CHARVALUE a `char` (one character between
//!   quotes), a STRING a `string`, an ENUMERATEDVALUE an enumerator of
//!   the field's enumeration;
//! * a PARAMETER `%n` has `n` smaller than 100, in both grammars
//!   (B.2; DDS 1.4 Annex B says the same); the parameters are as many
//!   as the expression requires (one more than its highest `%n`), each
//!   one Parameter token that fits its field the same way.
//!
//! Anything else is `SQLError` ("bad syntax, addresses non-existing
//! fields, or is not consistent with its parameters", §8.1.6.2).
//!
//! A value that is absent (an attribute without value, a relation that
//! designates no object or an object not available, an index or key
//! not in the collection) makes its predicate unknown; `NOT` of unknown
//! is unknown, and an object passes only when the condition is TRUE (the
//! three-valued logic of SQL).
//!
//! The DDS 1.4 filter grammar (with `LIKE`) is available as the named
//! option [`QueryGrammar::Dcps14`].

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;

use zerodds_sql_filter::{CmpOp, Expr, Grammar, Operand, Value as SqlValue, parse_with};

use crate::error::DlrlException;
use crate::model::{
    AttributeType, AttributeValue, BasicItem, BasicType, CollectionBasis, EnumerationType, Member,
    Model, Multiplicity, ObjectRef, SequenceValue, SimpleType, Value,
};
use crate::rows::ObjectData;

/// The deepest nesting of conditions a [`QueryExpression`] accepts.
pub const MAX_CONDITION_DEPTH: usize = 64;

/// The grammar of a [`QueryExpression`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryGrammar {
    /// DDS 1.2 Annex B, the syntax the specification gives for DLRL.
    #[default]
    AnnexB,
    /// The filter grammar of DDS 1.4 Annex B with `LIKE` and `!=`; its
    /// boolean literals and `null` have no DLRL type and are refused.
    Dcps14,
}

impl QueryGrammar {
    fn sql(self) -> Grammar {
        match self {
            Self::AnnexB => Grammar::Dlrl12,
            Self::Dcps14 => Grammar::Dcps14,
        }
    }
}

/// The type of the value a FIELDNAME designates.
#[derive(Debug, Clone, PartialEq)]
enum Leaf {
    /// `long`, `short`, `octet`.
    Integer,
    /// `real`, `double`.
    Float,
    /// `char`.
    Char,
    /// `string`.
    Text,
    /// An enumeration, compared by the position of its enumerators.
    Enum(EnumerationType),
}

impl Leaf {
    fn describe(&self) -> String {
        match self {
            Self::Integer => "an integer".into(),
            Self::Float => "a floating-point number".into(),
            Self::Char => "a char".into(),
            Self::Text => "a string".into(),
            Self::Enum(e) => format!("the enumeration {}", e.name),
        }
    }

    fn numeric(&self) -> bool {
        matches!(self, Self::Integer | Self::Float)
    }

    fn compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (a, b) if a.numeric() && b.numeric() => true,
            (Self::Char | Self::Text, Self::Char | Self::Text) => true,
            (Self::Enum(a), Self::Enum(b)) => a.name == b.name,
            _ => false,
        }
    }
}

/// One step of a FIELDNAME.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Member(String),
    Index(i64),
    Key(String),
}

/// A FIELDNAME resolved against the class.
#[derive(Debug, Clone, PartialEq)]
struct Field {
    text: String,
    steps: Vec<Step>,
    leaf: Leaf,
    /// Whether the path goes across a relation to another object.
    across_relation: bool,
}

/// A comparable value.
#[derive(Debug, Clone, PartialEq)]
enum Scalar {
    Int(i64),
    Float(f64),
    Text(String),
}

impl Scalar {
    fn compare(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Some(a.cmp(b)),
            #[allow(clippy::cast_precision_loss)]
            (Self::Int(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Self::Float(a), Self::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            (Self::Text(a), Self::Text(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

/// An operand of a predicate.
#[derive(Debug, Clone, PartialEq)]
enum Term {
    /// Index into the fields.
    Field(usize),
    /// A constant converted for the field it is compared with.
    Const(Scalar),
    /// A use of a parameter: index into the uses.
    Param(usize),
}

/// The checked condition.
#[derive(Debug, Clone, PartialEq)]
enum Cond {
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
    Cmp {
        lhs: Term,
        op: CmpOp,
        rhs: Term,
    },
    Between {
        field: usize,
        low: Term,
        high: Term,
        negated: bool,
    },
}

/// A use of `%n` and the field it is compared with.
#[derive(Debug, Clone, PartialEq)]
struct ParamUse {
    index: u32,
    leaf: Leaf,
}

/// A query or filter expression of Annex B with its parameters, checked
/// against one class of a model.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryExpression {
    class: String,
    expression: String,
    parameters: Vec<String>,
    grammar: QueryGrammar,
    fields: Vec<Field>,
    cond: Cond,
    uses: Vec<ParamUse>,
    bound: Vec<Scalar>,
    required: usize,
}

fn sql_error(message: impl Into<String>) -> DlrlException {
    DlrlException::sql(message)
}

impl QueryExpression {
    /// `expression` with `parameters` for the objects of `class`, in the
    /// grammar of Annex B.
    ///
    /// # Errors
    /// `SQLError` for bad syntax, a field the class does not have or
    /// that designates no basic or enumerated value, a constant or
    /// parameter that does not fit its field, or a parameter count that
    /// is not the one the expression requires.
    pub fn new(
        model: &Model,
        class: &str,
        expression: &str,
        parameters: &[String],
    ) -> Result<Self, DlrlException> {
        Self::with_grammar(model, class, expression, parameters, QueryGrammar::AnnexB)
    }

    /// [`QueryExpression::new`] in the given grammar.
    ///
    /// # Errors
    /// As [`QueryExpression::new`].
    pub fn with_grammar(
        model: &Model,
        class: &str,
        expression: &str,
        parameters: &[String],
        grammar: QueryGrammar,
    ) -> Result<Self, DlrlException> {
        if model.class(class).is_none() {
            return Err(sql_error(format!("the model has no class {class}")));
        }
        let expr = parse_with(expression, grammar.sql())
            .map_err(|e| sql_error(format!("{expression}: {}", e.0)))?;
        let mut b = Builder {
            model,
            class,
            fields: Vec::new(),
            uses: Vec::new(),
        };
        let cond = b.cond(&expr, 0)?;
        let required = b
            .uses
            .iter()
            .map(|u| u.index as usize + 1)
            .max()
            .unwrap_or(0);
        let mut q = Self {
            class: class.into(),
            expression: expression.into(),
            parameters: Vec::new(),
            grammar,
            fields: b.fields,
            cond,
            uses: b.uses,
            bound: Vec::new(),
            required,
        };
        q.set_parameters(parameters)?;
        Ok(q)
    }

    /// The class the expression applies to.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.class
    }

    /// `expression`.
    #[must_use]
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// `parameters`.
    #[must_use]
    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }

    /// The grammar the expression was parsed with.
    #[must_use]
    pub fn grammar(&self) -> QueryGrammar {
        self.grammar
    }

    /// The FIELDNAMEs that go across a relation to another object, as
    /// written in the expression (`a_radar.name`, `seen_by[1].name`).
    #[must_use]
    pub fn fields_across_relations(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.across_relation)
            .map(|f| f.text.as_str())
            .collect()
    }

    /// The number of parameters the expression requires: one more than
    /// its highest `%n`, 0 without parameters.
    #[must_use]
    pub fn required_parameters(&self) -> usize {
        self.required
    }

    /// `set_parameters`: new values of the parameters. On an error the
    /// expression keeps its parameters.
    ///
    /// # Errors
    /// `SQLError` when the count is not [`Self::required_parameters`],
    /// a parameter is not one INTEGERVALUE, CHARVALUE, FLOATVALUE,
    /// STRING or ENUMERATEDVALUE, or does not fit the field it is
    /// compared with.
    pub fn set_parameters(&mut self, parameters: &[String]) -> Result<(), DlrlException> {
        if parameters.len() != self.required {
            return Err(sql_error(format!(
                "{} requires {} parameter(s), {} given",
                self.expression,
                self.required,
                parameters.len()
            )));
        }
        // Every parameter is one Parameter token, also one whose `%n` the
        // expression does not use.
        let literals = parameters
            .iter()
            .enumerate()
            .map(|(n, text)| {
                parse_parameter(text, self.grammar)
                    .map_err(|m| sql_error(format!("parameter %{n}: {m}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut bound = Vec::with_capacity(self.uses.len());
        for u in &self.uses {
            let text = &parameters[u.index as usize];
            let literal = &literals[u.index as usize];
            let v = convert(literal, &u.leaf)
                .map_err(|m| sql_error(format!("parameter %{} ({text}): {m}", u.index)))?;
            bound.push(v);
        }
        self.parameters = parameters.to_vec();
        self.bound = bound;
        Ok(())
    }

    /// Whether `object` passes: the condition is TRUE for it. `resolve`
    /// gives the shared values of an object a relation designates, `None`
    /// when it is not available.
    #[must_use]
    pub fn check(
        &self,
        object: &ObjectData,
        resolve: &dyn Fn(&ObjectRef) -> Option<ObjectData>,
    ) -> bool {
        self.eval(&self.cond, object, resolve, 0) == Some(true)
    }

    /// zerodds-lint: recursion-depth 64 (`MAX_CONDITION_DEPTH`, the
    /// builder refuses deeper conditions)
    fn eval(
        &self,
        c: &Cond,
        object: &ObjectData,
        resolve: &dyn Fn(&ObjectRef) -> Option<ObjectData>,
        depth: usize,
    ) -> Option<bool> {
        if depth > MAX_CONDITION_DEPTH {
            return None;
        }
        match c {
            Cond::And(a, b) => {
                let (x, y) = (
                    self.eval(a, object, resolve, depth + 1),
                    self.eval(b, object, resolve, depth + 1),
                );
                match (x, y) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                }
            }
            Cond::Or(a, b) => {
                let (x, y) = (
                    self.eval(a, object, resolve, depth + 1),
                    self.eval(b, object, resolve, depth + 1),
                );
                match (x, y) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                }
            }
            Cond::Not(a) => self.eval(a, object, resolve, depth + 1).map(|v| !v),
            Cond::Cmp { lhs, op, rhs } => {
                let l = self.term(lhs, object, resolve)?;
                let r = self.term(rhs, object, resolve)?;
                if *op == CmpOp::Like {
                    return match (&l, &r) {
                        (Scalar::Text(s), Scalar::Text(p)) => Some(like(s, p)),
                        _ => None,
                    };
                }
                let o = l.compare(&r)?;
                Some(match op {
                    CmpOp::Eq => o == Ordering::Equal,
                    CmpOp::Neq => o != Ordering::Equal,
                    CmpOp::Lt => o == Ordering::Less,
                    CmpOp::Le => o != Ordering::Greater,
                    CmpOp::Gt => o == Ordering::Greater,
                    CmpOp::Ge => o != Ordering::Less,
                    CmpOp::Like => return None,
                })
            }
            Cond::Between {
                field,
                low,
                high,
                negated,
            } => {
                let v = self.term(&Term::Field(*field), object, resolve)?;
                let lo = self.term(low, object, resolve)?;
                let hi = self.term(high, object, resolve)?;
                let inside =
                    v.compare(&lo)? != Ordering::Less && v.compare(&hi)? != Ordering::Greater;
                Some(inside != *negated)
            }
        }
    }

    fn term(
        &self,
        t: &Term,
        object: &ObjectData,
        resolve: &dyn Fn(&ObjectRef) -> Option<ObjectData>,
    ) -> Option<Scalar> {
        match t {
            Term::Const(s) => Some(s.clone()),
            Term::Param(slot) => self.bound.get(*slot).cloned(),
            Term::Field(i) => {
                let f = self.fields.get(*i)?;
                read_field(f, object, resolve)
            }
        }
    }
}

/// Builds the checked condition.
struct Builder<'a> {
    model: &'a Model,
    class: &'a str,
    fields: Vec<Field>,
    uses: Vec<ParamUse>,
}

impl Builder<'_> {
    /// zerodds-lint: recursion-depth 64 (`MAX_CONDITION_DEPTH`, checked
    /// on entry)
    fn cond(&mut self, e: &Expr, depth: usize) -> Result<Cond, DlrlException> {
        if depth > MAX_CONDITION_DEPTH {
            return Err(sql_error(format!(
                "the condition is nested deeper than {MAX_CONDITION_DEPTH}"
            )));
        }
        Ok(match e {
            Expr::And(a, b) => Cond::And(
                Box::new(self.cond(a, depth + 1)?),
                Box::new(self.cond(b, depth + 1)?),
            ),
            Expr::Or(a, b) => Cond::Or(
                Box::new(self.cond(a, depth + 1)?),
                Box::new(self.cond(b, depth + 1)?),
            ),
            Expr::Not(a) => Cond::Not(Box::new(self.cond(a, depth + 1)?)),
            Expr::Cmp { lhs, op, rhs } => {
                let lf = self.field_of(lhs)?;
                let rf = self.field_of(rhs)?;
                let (lt, rt) = match (lf, rf) {
                    (Some(l), Some(r)) => {
                        let (a, b) = (&self.fields[l].leaf, &self.fields[r].leaf);
                        if !a.compatible(b) {
                            return Err(sql_error(format!(
                                "{} is {} and {} is {}",
                                self.fields[l].text,
                                a.describe(),
                                self.fields[r].text,
                                b.describe()
                            )));
                        }
                        (Term::Field(l), Term::Field(r))
                    }
                    (Some(l), None) => {
                        let leaf = self.fields[l].leaf.clone();
                        let text = self.fields[l].text.clone();
                        let other = self.operand(rhs, &leaf, &text)?;
                        (Term::Field(l), other)
                    }
                    (None, Some(r)) => {
                        let leaf = self.fields[r].leaf.clone();
                        let text = self.fields[r].text.clone();
                        let other = self.operand(lhs, &leaf, &text)?;
                        (other, Term::Field(r))
                    }
                    (None, None) => {
                        return Err(sql_error(
                            "a comparison needs a FIELDNAME on one side".to_string(),
                        ));
                    }
                };
                if *op == CmpOp::Like {
                    let text = lf.or(rf).map(|i| &self.fields[i]);
                    if let Some(f) = text
                        && !matches!(f.leaf, Leaf::Text | Leaf::Char)
                    {
                        return Err(sql_error(format!(
                            "LIKE needs a string, {} is {}",
                            f.text,
                            f.leaf.describe()
                        )));
                    }
                }
                Cond::Cmp {
                    lhs: lt,
                    op: *op,
                    rhs: rt,
                }
            }
            Expr::Between {
                field,
                low,
                high,
                negated,
            } => {
                let Some(f) = self.field_of(field)? else {
                    return Err(sql_error("BETWEEN needs a FIELDNAME".to_string()));
                };
                let leaf = self.fields[f].leaf.clone();
                let text = self.fields[f].text.clone();
                if self.field_of(low)?.is_some() || self.field_of(high)?.is_some() {
                    return Err(sql_error(
                        "the Range of BETWEEN is two Parameters".to_string(),
                    ));
                }
                let low = self.operand(low, &leaf, &text)?;
                let high = self.operand(high, &leaf, &text)?;
                Cond::Between {
                    field: f,
                    low,
                    high,
                    negated: *negated,
                }
            }
        })
    }

    /// The field an operand names, resolved and added; `None` for a
    /// constant or a parameter.
    fn field_of(&mut self, op: &Operand) -> Result<Option<usize>, DlrlException> {
        let Operand::Field(text) = op else {
            return Ok(None);
        };
        if let Some(i) = self.fields.iter().position(|f| &f.text == text) {
            return Ok(Some(i));
        }
        let steps =
            split_fieldname(text).ok_or_else(|| sql_error(format!("{text} is not a FIELDNAME")))?;
        let (leaf, across_relation) = resolve_leaf(self.model, self.class, text, &steps)?;
        self.fields.push(Field {
            text: text.clone(),
            steps,
            leaf,
            across_relation,
        });
        Ok(Some(self.fields.len() - 1))
    }

    /// A constant or parameter compared with a field of type `leaf`.
    fn operand(&mut self, op: &Operand, leaf: &Leaf, field: &str) -> Result<Term, DlrlException> {
        match op {
            Operand::Literal(v) => convert(v, leaf)
                .map(Term::Const)
                .map_err(|m| sql_error(format!("compared with {field}: {m}"))),
            // DDS 1.2 Annex B.2 and DDS 1.4 Annex B: "a natural number
            // (zero included) smaller than 100". The Annex B parser refuses
            // a larger one; the DDS 1.4 lexer reads any u32, whose `n + 1`
            // overflows the parameter count on a 32-bit target.
            Operand::Param(n) if *n >= 100 => Err(sql_error(format!(
                "PARAMETER %{n} compared with {field} is not smaller than 100"
            ))),
            Operand::Param(n) => {
                self.uses.push(ParamUse {
                    index: *n,
                    leaf: leaf.clone(),
                });
                Ok(Term::Param(self.uses.len() - 1))
            }
            Operand::Field(_) => Err(sql_error(format!(
                "{field} is compared with a second field where a Parameter is required"
            ))),
        }
    }
}

/// Splits the text of a FIELDNAME into its steps: names joined by `.`,
/// `[n]` with a signed decimal integer, `['key']` with `''` for a quote.
fn split_fieldname(text: &str) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let mut rest = text;
    let mut expect_name = true;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('[') {
            if steps.is_empty() {
                return None;
            }
            if let Some(r) = r.strip_prefix('\'') {
                let mut key = String::new();
                let mut chars = r.char_indices();
                let mut end = None;
                while let Some((i, ch)) = chars.next() {
                    if ch == '\'' {
                        if r[i + 1..].starts_with('\'') {
                            key.push('\'');
                            chars.next();
                        } else {
                            end = Some(i + 1);
                            break;
                        }
                    } else {
                        key.push(ch);
                    }
                }
                let r = r[end?..].strip_prefix(']')?;
                steps.push(Step::Key(key));
                rest = r;
            } else {
                let close = r.find(']')?;
                steps.push(Step::Index(r[..close].parse().ok()?));
                rest = &r[close + 1..];
            }
            expect_name = false;
            continue;
        }
        if !expect_name {
            rest = rest.strip_prefix('.')?;
        }
        let end = rest.find(['.', '[']).unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty() {
            return None;
        }
        steps.push(Step::Member(name.into()));
        rest = &rest[end..];
        expect_name = false;
    }
    (!steps.is_empty()).then_some(steps)
}

fn basic_of_item(i: BasicItem) -> BasicType {
    match i {
        BasicItem::Long => BasicType::Long,
        BasicItem::Short => BasicType::Short,
        BasicItem::Char => BasicType::Char,
        BasicItem::Octet => BasicType::Octet,
        BasicItem::Real => BasicType::Real,
        BasicItem::Double => BasicType::Double,
        BasicItem::String => BasicType::String,
    }
}

/// Where a step of a FIELDNAME stands while it is resolved.
enum At {
    Class(String),
    Simple(SimpleType),
    /// A collection that needs a subscript: its basis and what an item is.
    Collection(CollectionBasis, Box<At>),
}

/// Resolves the steps of a FIELDNAME against the model: the type of the
/// value it designates and whether the path goes across a relation.
fn resolve_leaf(
    model: &Model,
    class: &str,
    text: &str,
    steps: &[Step],
) -> Result<(Leaf, bool), DlrlException> {
    let unknown = |what: &str| sql_error(format!("{text}: {what}"));
    let mut at = At::Class(class.into());
    let mut across_relation = false;
    for step in steps {
        at = match (at, step) {
            (At::Class(c), Step::Member(name)) => match model.member(&c, name) {
                Some((_, Member::Attribute(a))) => match &a.ty {
                    AttributeType::Mono(t) => At::Simple(t.clone()),
                    AttributeType::Multi { basis, item } => {
                        At::Collection(*basis, Box::new(At::Simple(item.clone())))
                    }
                },
                Some((_, Member::Relation(r))) => {
                    across_relation = true;
                    match r.multiplicity {
                        Multiplicity::ToOne => At::Class(r.target.clone()),
                        Multiplicity::ToMany(basis) => {
                            At::Collection(basis, Box::new(At::Class(r.target.clone())))
                        }
                    }
                }
                Some((_, Member::Local)) => {
                    return Err(unknown(&format!(
                        "{name} is a local attribute of {c}, not known to the DLRL"
                    )));
                }
                None => return Err(unknown(&format!("class {c} has no attribute {name}"))),
            },
            (At::Simple(SimpleType::Struct(s)), Step::Member(name)) => {
                match s.fields.iter().find(|f| &f.name == name) {
                    Some(f) => At::Simple(f.ty.clone()),
                    None => {
                        return Err(unknown(&format!(
                            "structure {} has no member {name}",
                            s.name
                        )));
                    }
                }
            }
            (At::Simple(SimpleType::Basic(BasicType::Sequence(item))), Step::Index(_)) => {
                At::Simple(SimpleType::Basic(basic_of_item(item)))
            }
            (At::Collection(basis, item), step) => match (basis, step) {
                (CollectionBasis::List | CollectionBasis::IntMap, Step::Index(_))
                | (CollectionBasis::StrMap, Step::Key(_)) => *item,
                (CollectionBasis::Set, _) => {
                    return Err(unknown("a Set has no subscript"));
                }
                (CollectionBasis::StrMap, _) => {
                    return Err(unknown("a StrMap needs a ['key'] subscript"));
                }
                _ => return Err(unknown("a List or IntMap needs an [n] subscript")),
            },
            (_, Step::Member(name)) => {
                return Err(unknown(&format!("no member {name} to navigate to")));
            }
            (_, _) => return Err(unknown("a subscript where there is no collection")),
        };
    }
    let leaf = match at {
        At::Simple(SimpleType::Basic(b)) => match b {
            BasicType::Long | BasicType::Short | BasicType::Octet => Ok(Leaf::Integer),
            BasicType::Real | BasicType::Double => Ok(Leaf::Float),
            BasicType::Char => Ok(Leaf::Char),
            BasicType::String => Ok(Leaf::Text),
            BasicType::Sequence(_) => Err(unknown("a sequence is not a value to compare")),
        },
        At::Simple(SimpleType::Enumeration(e)) => Ok(Leaf::Enum(e)),
        At::Simple(SimpleType::Struct(s)) => Err(unknown(&format!(
            "structure {} is not a value to compare",
            s.name
        ))),
        At::Class(c) => Err(unknown(&format!(
            "an object of {c} is not a value to compare"
        ))),
        At::Collection(..) => Err(unknown("a collection is not a value to compare")),
    }?;
    Ok((leaf, across_relation))
}

/// A constant converted for a field of type `leaf`.
fn convert(v: &SqlValue, leaf: &Leaf) -> Result<Scalar, String> {
    match (v, leaf) {
        (SqlValue::Int(i), Leaf::Integer | Leaf::Float) => Ok(Scalar::Int(*i)),
        (SqlValue::Float(f), Leaf::Integer | Leaf::Float) => Ok(Scalar::Float(*f)),
        (SqlValue::String(s), Leaf::Char) => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(_), None) => Ok(Scalar::Text(s.clone())),
                _ => Err(format!(
                    "'{s}' is not a CHARVALUE (one character between quotes)"
                )),
            }
        }
        (SqlValue::String(s), Leaf::Text) => Ok(Scalar::Text(s.clone())),
        (SqlValue::String(s), Leaf::Enum(e)) => e
            .enumerators
            .iter()
            .position(|n| n == s)
            .map(|p| Scalar::Int(i64::try_from(p).unwrap_or(i64::MAX)))
            .ok_or_else(|| format!("'{s}' is not an enumerator of {}", e.name)),
        (SqlValue::Bool(_) | SqlValue::Null, _) => {
            Err("a boolean or null has no DLRL type".to_string())
        }
        (other, leaf) => Err(format!("{other:?} does not fit {}", leaf.describe())),
    }
}

/// One parameter string as the Parameter token it must be.
fn parse_parameter(text: &str, grammar: QueryGrammar) -> Result<SqlValue, String> {
    match parse_with(&format!("x = {text}"), grammar.sql()) {
        Ok(Expr::Cmp {
            rhs: Operand::Literal(v),
            ..
        }) => Ok(v),
        _ => Err(format!(
            "'{text}' is not one INTEGERVALUE, CHARVALUE, FLOATVALUE, STRING or ENUMERATEDVALUE"
        )),
    }
}

/// Where a step stands while a field of an object is read.
enum Here {
    Object(ObjectData),
    Attribute(AttributeValue),
    Value(Value),
}

fn read_field(
    f: &Field,
    object: &ObjectData,
    resolve: &dyn Fn(&ObjectRef) -> Option<ObjectData>,
) -> Option<Scalar> {
    let mut here = Here::Object(object.clone());
    for step in &f.steps {
        // A reference with steps after it: the object it designates.
        if let Here::Value(Value::Ref(r)) = &here {
            here = Here::Object(resolve(r.as_ref()?)?);
        }
        here = match (here, step) {
            (Here::Object(o), Step::Member(name)) => match o.values.get(name)? {
                AttributeValue::Mono(v) => Here::Value(v.clone()),
                coll => Here::Attribute(coll.clone()),
            },
            (Here::Attribute(AttributeValue::List(items)), Step::Index(i)) => {
                Here::Value(items.get(usize::try_from(*i).ok()?)?.clone())
            }
            (Here::Attribute(AttributeValue::IntMap(items)), Step::Index(i)) => {
                Here::Value(items.get(&i32::try_from(*i).ok()?)?.clone())
            }
            (Here::Attribute(AttributeValue::StrMap(items)), Step::Key(k)) => {
                Here::Value(items.get(k)?.clone())
            }
            (Here::Value(Value::Struct { fields, .. }), Step::Member(name)) => {
                Here::Value(fields.iter().find(|(n, _)| n == name)?.1.clone())
            }
            (Here::Value(Value::Sequence(seq)), Step::Index(i)) => {
                Here::Value(sequence_item(&seq, usize::try_from(*i).ok()?)?)
            }
            _ => return None,
        };
    }
    let Here::Value(v) = here else {
        return None;
    };
    match (v, &f.leaf) {
        (Value::Long(x), _) => Some(Scalar::Int(i64::from(x))),
        (Value::Short(x), _) => Some(Scalar::Int(i64::from(x))),
        (Value::Octet(x), _) => Some(Scalar::Int(i64::from(x))),
        (Value::Real(x), _) => Some(Scalar::Float(f64::from(x))),
        (Value::Double(x), _) => Some(Scalar::Float(x)),
        (Value::Char(c), _) => Some(Scalar::Text(c.to_string())),
        (Value::String(s), _) => Some(Scalar::Text(s)),
        (Value::Enum { enumerator, .. }, Leaf::Enum(e)) => e
            .enumerators
            .iter()
            .position(|n| *n == enumerator)
            .and_then(|p| i64::try_from(p).ok())
            .map(Scalar::Int),
        _ => None,
    }
}

fn sequence_item(seq: &SequenceValue, i: usize) -> Option<Value> {
    Some(match seq {
        SequenceValue::Long(v) => Value::Long(*v.get(i)?),
        SequenceValue::Short(v) => Value::Short(*v.get(i)?),
        SequenceValue::Char(v) => Value::Char(*v.get(i)?),
        SequenceValue::Octet(v) => Value::Octet(*v.get(i)?),
        SequenceValue::Real(v) => Value::Real(*v.get(i)?),
        SequenceValue::Double(v) => Value::Double(*v.get(i)?),
        SequenceValue::String(v) => Value::String(v.get(i)?.clone()),
    })
}

/// `LIKE` of the DDS 1.4 grammar: `%` any run of characters, `_` one.
fn like(s: &str, pattern: &str) -> bool {
    let s: Vec<char> = s.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    // Iterative matching with one backtrack point for the last `%`.
    let (mut i, mut j) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while i < s.len() {
        if j < p.len() && (p[j] == '_' || p[j] == s[i]) {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == '%' {
            star = Some((j, i));
            j += 1;
        } else if let Some((sj, si)) = star {
            j = sj + 1;
            i = si + 1;
            star = Some((sj, si + 1));
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == '%' {
        j += 1;
    }
    j == p.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn dlrl_b_2_like_of_the_dcps14_option_matches_percent_and_underscore() {
        assert!(like("TRACK-7", "TRACK-%"));
        assert!(like("TRACK-7", "TRACK-_"));
        assert!(!like("TRACK-17", "TRACK-_"));
        assert!(like("abc", "%"));
        assert!(like("", "%"));
        assert!(!like("abc", "a%d"));
        assert!(like("aXbXc", "a%c"));
    }

    #[test]
    fn dlrl_b_2_fieldname_text_splits_into_steps() {
        assert_eq!(
            split_fieldname("a.b[-3].c['k''q']").unwrap(),
            [
                Step::Member("a".into()),
                Step::Member("b".into()),
                Step::Index(-3),
                Step::Member("c".into()),
                Step::Key("k'q".into()),
            ]
        );
        assert!(split_fieldname("[1]").is_none());
        assert!(split_fieldname("a..b").is_none());
        assert!(split_fieldname("a[1]b").is_none());
    }
}
