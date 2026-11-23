// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DynamicAny` (CORBA 3.3 Part 1 §13.2): a value of a type known only
//! by its [`TypeCode`] at run time, built, read and walked component by
//! component.
//!
//! [`DynAnyFactory`] creates a [`DynAny`] from a [`CorbaAny`] or, with
//! the default values of §13.2.1, from a [`TypeCode`]. One [`DynAny`]
//! type carries the operations of `DynAny` and of the derived interfaces
//! `DynEnum`, `DynStruct` (structs and exceptions) and `DynSequence`;
//! an operation of a derived interface on a value of another kind raises
//! `TypeMismatch`.
//!
//! The kinds are those a [`CorbaAny`] can carry: the basic types,
//! string, wstring, `any`, enum, struct, exception, sequence and alias.
//! `tk_union`, `tk_array`, `tk_fixed`, `tk_value`, `tk_value_box`,
//! `tk_objref`, `tk_TypeCode` and `tk_longdouble` have no [`AnyValue`]
//! yet; the factory refuses them with `InconsistentTypeCode`.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::any::{AnyValue, CorbaAny, WString};
use crate::type_code::TypeCode;

/// `DynAny::TypeMismatch` and `DynAny::InvalidValue` (§13.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynAnyError {
    /// `TypeMismatch`: the operation does not fit the type.
    TypeMismatch,
    /// `InvalidValue`: the value or the current position does not fit.
    InvalidValue,
}

/// `DynAnyFactory::InconsistentTypeCode` (§13.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InconsistentTypeCode;

/// Result of a `DynAny` operation.
pub type DynResult<T> = Result<T, DynAnyError>;

/// `DynamicAny::DynAnyFactory` (§13.2.1); the ORB hands it out as the
/// initial reference `DynAnyFactory`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DynAnyFactory;

impl DynAnyFactory {
    /// `create_dyn_any`: a copy of the value; current position 0 when it
    /// has components, else -1.
    ///
    /// # Errors
    /// [`InconsistentTypeCode`] for a kind this module does not carry.
    pub fn create_dyn_any(&self, value: &CorbaAny) -> Result<DynAny, InconsistentTypeCode> {
        let tc = value.0.type_code();
        let mut d = self.create_dyn_any_from_type_code(&tc)?;
        d.from_any(value).map_err(|_| InconsistentTypeCode)?;
        Ok(d)
    }

    /// `create_dyn_any_without_truncation`: as `create_dyn_any`; this
    /// module carries no valuetypes, so nothing is truncated.
    ///
    /// # Errors
    /// As [`create_dyn_any`](Self::create_dyn_any).
    pub fn create_dyn_any_without_truncation(
        &self,
        value: &CorbaAny,
    ) -> Result<DynAny, InconsistentTypeCode> {
        self.create_dyn_any(value)
    }

    /// `create_dyn_any_from_type_code`: the default value of §13.2.1.
    ///
    /// # Errors
    /// [`InconsistentTypeCode`] for a kind this module does not carry.
    pub fn create_dyn_any_from_type_code(
        &self,
        tc: &TypeCode,
    ) -> Result<DynAny, InconsistentTypeCode> {
        DynAny::default_for(tc, &mut Vec::new())
    }

    /// `create_multiple_dyn_anys`.
    ///
    /// # Errors
    /// [`InconsistentTypeCode`] as `create_dyn_any`.
    pub fn create_multiple_dyn_anys(
        &self,
        values: &[CorbaAny],
    ) -> Result<Vec<DynAny>, InconsistentTypeCode> {
        values.iter().map(|v| self.create_dyn_any(v)).collect()
    }

    /// `create_multiple_anys`.
    #[must_use]
    pub fn create_multiple_anys(&self, values: &[DynAny]) -> Vec<CorbaAny> {
        values.iter().map(DynAny::to_any).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    /// tk_null / tk_void: no value, no components.
    Empty,
    /// A basic value (scalar, string, wstring, any).
    Basic(AnyValue),
    /// DynEnum: the ordinal.
    Enum(u32),
    /// DynStruct: one component per member.
    Struct(Vec<DynAny>),
    /// DynSequence: the element type (recursion resolved) and the
    /// elements.
    Sequence(Box<TypeCode>, Vec<DynAny>),
}

/// A `DynAny` (§13.2.2) with the operations of `DynEnum` (§13.2.4),
/// `DynStruct` (§13.2.5) and `DynSequence` (§13.2.7).
#[derive(Debug, Clone, PartialEq)]
pub struct DynAny {
    tc: TypeCode,
    node: Node,
    current: i32,
}

/// The type an alias names, through every alias.
fn unalias(tc: &TypeCode) -> &TypeCode {
    let mut t = tc;
    while let TypeCode::Alias { content, .. } = t {
        t = content;
    }
    t
}

/// `TypeCode::equivalent` (§8.11.1): aliases removed, names and member
/// names ignored, repository ids compared where both have one.
///
/// The member and element pairs still to compare wait in a list on the
/// heap, not in nested calls: the stack this takes does not grow with
/// how deep the two `TypeCode`s nest.
#[must_use]
pub fn equivalent(a: &TypeCode, b: &TypeCode) -> bool {
    let mut pending: Vec<(&TypeCode, &TypeCode)> = alloc::vec![(a, b)];
    while let Some((a, b)) = pending.pop() {
        if !equivalent_here(a, b, &mut pending) {
            return false;
        }
    }
    true
}

/// [`equivalent`] for the pair itself; the pairs of its members or
/// elements, which are equivalent as well when the pair is, go to
/// `pending`.
fn equivalent_here<'t>(
    a: &'t TypeCode,
    b: &'t TypeCode,
    pending: &mut Vec<(&'t TypeCode, &'t TypeCode)>,
) -> bool {
    let (a, b) = (unalias(a), unalias(b));
    match (a, b) {
        (
            TypeCode::Struct {
                repo_id: ra,
                members: ma,
                is_except: ea,
                ..
            },
            TypeCode::Struct {
                repo_id: rb,
                members: mb,
                is_except: eb,
                ..
            },
        ) => {
            if !ra.is_empty() && !rb.is_empty() {
                return ra == rb && ea == eb;
            }
            if ea != eb || ma.len() != mb.len() {
                return false;
            }
            pending.extend(ma.iter().zip(mb).map(|((_, x), (_, y))| (x, y)));
            true
        }
        (
            TypeCode::Enum {
                repo_id: ra,
                members: ma,
                ..
            },
            TypeCode::Enum {
                repo_id: rb,
                members: mb,
                ..
            },
        ) => {
            if !ra.is_empty() && !rb.is_empty() {
                return ra == rb;
            }
            ma.len() == mb.len()
        }
        (
            TypeCode::Sequence {
                element: xa,
                bound: ba,
            },
            TypeCode::Sequence {
                element: xb,
                bound: bb,
            },
        ) => {
            if ba != bb {
                return false;
            }
            pending.push((xa, xb));
            true
        }
        (TypeCode::Recursive { repo_id: ra }, TypeCode::Recursive { repo_id: rb }) => ra == rb,
        (TypeCode::Recursive { repo_id: r }, TypeCode::Struct { repo_id: s, .. })
        | (TypeCode::Struct { repo_id: s, .. }, TypeCode::Recursive { repo_id: r }) => r == s,
        _ => a == b,
    }
}

/// `true` when a value of `tc` has components (§13.2.2.9).
fn has_components(node: &Node) -> bool {
    match node {
        Node::Struct(m) => !m.is_empty(),
        Node::Sequence(_, e) => !e.is_empty(),
        _ => false,
    }
}

/// `true` for an exception TypeCode without members, the one kind of
/// struct that cannot have components (§13.2.1, §13.2.2.9).
fn is_empty_exception(tc: &TypeCode) -> bool {
    matches!(unalias(tc), TypeCode::Struct { members, is_except: true, .. } if members.is_empty())
}

fn resolve<'a>(tc: &'a TypeCode, outer: &'a [TypeCode]) -> Option<&'a TypeCode> {
    match tc {
        TypeCode::Recursive { repo_id } => outer
            .iter()
            .rev()
            .find(|t| matches!(unalias(t), TypeCode::Struct { repo_id: r, .. } if r == repo_id)),
        other => Some(other),
    }
}

impl DynAny {
    fn default_for(tc: &TypeCode, outer: &mut Vec<TypeCode>) -> Result<Self, InconsistentTypeCode> {
        let real = resolve(tc, outer).ok_or(InconsistentTypeCode)?.clone();
        let node = match unalias(&real) {
            TypeCode::Null | TypeCode::Void => Node::Empty,
            TypeCode::Boolean => Node::Basic(AnyValue::Boolean(false)),
            TypeCode::Octet => Node::Basic(AnyValue::Octet(0)),
            TypeCode::Char => Node::Basic(AnyValue::Char(0)),
            TypeCode::WChar => Node::Basic(AnyValue::WChar(0)),
            TypeCode::Short => Node::Basic(AnyValue::Short(0)),
            TypeCode::UShort => Node::Basic(AnyValue::UShort(0)),
            TypeCode::Long => Node::Basic(AnyValue::Long(0)),
            TypeCode::ULong => Node::Basic(AnyValue::ULong(0)),
            TypeCode::LongLong => Node::Basic(AnyValue::LongLong(0)),
            TypeCode::ULongLong => Node::Basic(AnyValue::ULongLong(0)),
            TypeCode::Float => Node::Basic(AnyValue::Float(0.0)),
            TypeCode::Double => Node::Basic(AnyValue::Double(0.0)),
            TypeCode::String(_) => Node::Basic(AnyValue::Str(String::new())),
            TypeCode::WString(_) => Node::Basic(AnyValue::WStr(WString(String::new()))),
            TypeCode::Any => Node::Basic(AnyValue::Any(Box::default())),
            TypeCode::Enum { members, .. } => {
                if members.is_empty() {
                    return Err(InconsistentTypeCode);
                }
                Node::Enum(0)
            }
            s @ TypeCode::Struct { members, .. } => {
                outer.push(s.clone());
                let mut items = Vec::with_capacity(members.len());
                for (_, m) in members {
                    let item = Self::default_for(m, outer);
                    match item {
                        Ok(i) => items.push(i),
                        Err(e) => {
                            outer.pop();
                            return Err(e);
                        }
                    }
                }
                outer.pop();
                Node::Struct(items)
            }
            TypeCode::Sequence { element, .. } => {
                let recursive = matches!(**element, TypeCode::Recursive { .. });
                let element = resolve(element, outer).ok_or(InconsistentTypeCode)?.clone();
                // An element of this type must be constructible; a
                // recursive element is the enclosing struct, which is
                // being built.
                if !recursive {
                    Self::default_for(&element, outer)?;
                }
                Node::Sequence(Box::new(element), Vec::new())
            }
            // §13.2.1: tk_Principal, tk_native and tk_local_interface
            // have no DynAny. The DynAny interfaces of the other kinds
            // (DynFixed, DynUnion, DynArray, DynValue, DynValueBox, and the
            // reference, TypeCode and long double components) are not
            // built here: refused rather than given a wrong shape.
            TypeCode::TypeCodeTc
            | TypeCode::ObjRef { .. }
            | TypeCode::Alias { .. }
            | TypeCode::Recursive { .. }
            | TypeCode::Principal
            | TypeCode::Native { .. }
            | TypeCode::LocalInterface { .. }
            | TypeCode::LongDouble
            | TypeCode::Fixed { .. }
            | TypeCode::Array { .. }
            | TypeCode::Union { .. }
            | TypeCode::Value { .. }
            | TypeCode::ValueBox { .. }
            | TypeCode::AbstractInterface { .. }
            | TypeCode::Component { .. }
            | TypeCode::Home { .. } => return Err(InconsistentTypeCode),
        };
        // §13.2.1: a DynStruct starts at -1 for an empty exception and at
        // 0 for every other struct TypeCode; the other kinds at 0 when the
        // default value has components.
        let current = match &node {
            Node::Struct(_) if is_empty_exception(&real) => -1,
            Node::Struct(_) => 0,
            other if has_components(other) => 0,
            _ => -1,
        };
        Ok(Self {
            tc: tc.clone(),
            node,
            current,
        })
    }

    /// §13.2.2.1 `type`: the TypeCode the `DynAny` was created with.
    #[must_use]
    pub fn type_code(&self) -> &TypeCode {
        &self.tc
    }

    /// §13.2.2.2 `assign`.
    ///
    /// # Errors
    /// `TypeMismatch` when the types are not equivalent.
    pub fn assign(&mut self, other: &DynAny) -> DynResult<()> {
        if !equivalent(&self.tc, &other.tc) {
            return Err(DynAnyError::TypeMismatch);
        }
        self.node = other.node.clone();
        self.current = if has_components(&self.node) { 0 } else { -1 };
        Ok(())
    }

    /// §13.2.2.3 `from_any`.
    ///
    /// # Errors
    /// `TypeMismatch` when the value's type is not equivalent; an any
    /// carries no string bound, so a string longer than the bound of the
    /// `DynAny`'s type is `InvalidValue`, as is an enum ordinal outside
    /// its enumerators.
    pub fn from_any(&mut self, value: &CorbaAny) -> DynResult<()> {
        let node = Self::node_from_value(&self.tc, &value.0, &mut Vec::new())?;
        self.node = node;
        self.current = if has_components(&self.node) { 0 } else { -1 };
        Ok(())
    }

    fn node_from_value(tc: &TypeCode, v: &AnyValue, outer: &mut Vec<TypeCode>) -> DynResult<Node> {
        let real = resolve(tc, outer).ok_or(DynAnyError::TypeMismatch)?.clone();
        let mismatch = Err(DynAnyError::TypeMismatch);
        Ok(match (unalias(&real), v) {
            (TypeCode::Null | TypeCode::Void, AnyValue::Null) => Node::Empty,
            (TypeCode::String(bound), AnyValue::Str(s)) => {
                if *bound != 0 && s.chars().count() > *bound as usize {
                    return Err(DynAnyError::InvalidValue);
                }
                Node::Basic(v.clone())
            }
            (TypeCode::WString(bound), AnyValue::WStr(s)) => {
                if *bound != 0 && s.0.chars().count() > *bound as usize {
                    return Err(DynAnyError::InvalidValue);
                }
                Node::Basic(v.clone())
            }
            (
                TypeCode::Enum {
                    repo_id, members, ..
                },
                AnyValue::Enum {
                    repo_id: r, value, ..
                },
            ) => {
                if !repo_id.is_empty() && !r.is_empty() && repo_id != r {
                    return mismatch;
                }
                if *value as usize >= members.len() {
                    return Err(DynAnyError::InvalidValue);
                }
                Node::Enum(*value)
            }
            (
                s @ TypeCode::Struct {
                    repo_id,
                    members,
                    is_except,
                    ..
                },
                AnyValue::Struct {
                    repo_id: r,
                    members: vm,
                    ..
                }
                | AnyValue::Except {
                    repo_id: r,
                    members: vm,
                    ..
                },
            ) => {
                if *is_except != matches!(v, AnyValue::Except { .. })
                    || (!repo_id.is_empty() && !r.is_empty() && repo_id != r)
                    || members.len() != vm.len()
                {
                    return mismatch;
                }
                outer.push(s.clone());
                let mut items = Vec::with_capacity(members.len());
                for ((_, mtc), (_, mv)) in members.iter().zip(vm) {
                    let node = Self::node_from_value(mtc, mv, outer);
                    let node = match node {
                        Ok(n) => n,
                        Err(e) => {
                            outer.pop();
                            return Err(e);
                        }
                    };
                    let current = if has_components(&node) { 0 } else { -1 };
                    items.push(DynAny {
                        tc: mtc.clone(),
                        node,
                        current,
                    });
                }
                outer.pop();
                Node::Struct(items)
            }
            (TypeCode::Sequence { element, bound }, AnyValue::Seq { element: ve, items }) => {
                let element = resolve(element, outer)
                    .ok_or(DynAnyError::TypeMismatch)?
                    .clone();
                // Each item is checked against the element type below; an
                // empty sequence has only its declared element type.
                if items.is_empty() && !equivalent(&element, ve) {
                    return mismatch;
                }
                if *bound != 0 && items.len() > *bound as usize {
                    return Err(DynAnyError::InvalidValue);
                }
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let node = Self::node_from_value(&element, item, outer)?;
                    let current = if has_components(&node) { 0 } else { -1 };
                    out.push(DynAny {
                        tc: element.clone(),
                        node,
                        current,
                    });
                }
                Node::Sequence(Box::new(element), out)
            }
            (t, v)
                if !matches!(
                    v,
                    AnyValue::Seq { .. } | AnyValue::Struct { .. } | AnyValue::Except { .. }
                ) && equivalent(t, &v.type_code()) =>
            {
                Node::Basic(v.clone())
            }
            _ => return mismatch,
        })
    }

    /// §13.2.2.4 `to_any`.
    #[must_use]
    pub fn to_any(&self) -> CorbaAny {
        CorbaAny(self.value())
    }

    fn value(&self) -> AnyValue {
        match (&self.node, unalias(&self.tc)) {
            (Node::Empty, _) => AnyValue::Null,
            (Node::Basic(v), _) => v.clone(),
            (
                Node::Enum(value),
                TypeCode::Enum {
                    repo_id,
                    name,
                    members,
                },
            ) => AnyValue::Enum {
                repo_id: repo_id.clone(),
                name: name.clone(),
                value: *value,
                members: members.clone(),
            },
            (
                Node::Struct(items),
                TypeCode::Struct {
                    repo_id,
                    name,
                    members,
                    is_except,
                },
            ) => {
                let repo_id = repo_id.clone();
                let name = name.clone();
                let members = members
                    .iter()
                    .zip(items)
                    .map(|((n, _), d)| (n.clone(), d.value()))
                    .collect();
                if *is_except {
                    AnyValue::Except {
                        repo_id,
                        name,
                        members,
                    }
                } else {
                    AnyValue::Struct {
                        repo_id,
                        name,
                        members,
                    }
                }
            }
            (Node::Sequence(element, items), _) => AnyValue::Seq {
                element: (**element).clone(),
                items: items.iter().map(DynAny::value).collect(),
            },
            // An enum or struct node is made only for a TypeCode of that
            // kind, and the TypeCode does not change after creation.
            (Node::Enum(_) | Node::Struct(_), _) => AnyValue::Null,
        }
    }

    /// §13.2.2.5 `equal`: equivalent types and equal values; the current
    /// position does not count.
    #[must_use]
    pub fn equal(&self, other: &DynAny) -> bool {
        equivalent(&self.tc, &other.tc)
            && match (&self.node, &other.node) {
                (Node::Struct(a), Node::Struct(b))
                | (Node::Sequence(_, a), Node::Sequence(_, b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.equal(y))
                }
                (a, b) => a == b,
            }
    }

    /// §13.2.2.7 `copy`.
    #[must_use]
    pub fn copy(&self) -> DynAny {
        self.clone()
    }

    /// §13.2.2.6 `destroy`: the value goes with the Rust value; nothing
    /// else to release.
    pub fn destroy(self) {}

    // -- §13.2.2.9 iteration ------------------------------------------

    /// `component_count`.
    #[must_use]
    pub fn component_count(&self) -> u32 {
        let n = match &self.node {
            Node::Struct(m) => m.len(),
            Node::Sequence(_, e) => e.len(),
            _ => 0,
        };
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    /// `seek(index)`: `true` and the position set when `index` names a
    /// component; else the position becomes -1 and `false`.
    pub fn seek(&mut self, index: i32) -> bool {
        if u32::try_from(index).is_ok_and(|i| i < self.component_count()) {
            self.current = index;
            true
        } else {
            self.current = -1;
            false
        }
    }

    /// `rewind`: `seek(0)`.
    pub fn rewind(&mut self) {
        self.seek(0);
    }

    /// `next`: advances; `false` and position -1 past the last component.
    // The IDL name of the operation; it moves a position and is no
    // iterator.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        let n = self.current.saturating_add(1);
        self.seek(n)
    }

    /// `current_component`: `None` at position -1 (or past the last
    /// component).
    ///
    /// # Errors
    /// `TypeMismatch` for a value that cannot have components: a basic
    /// value, a `DynEnum`, an empty exception (§13.2.2.9).
    pub fn current_component(&mut self) -> DynResult<Option<&mut DynAny>> {
        if is_empty_exception(&self.tc) {
            return Err(DynAnyError::TypeMismatch);
        }
        let idx = usize::try_from(self.current).ok();
        match &mut self.node {
            Node::Struct(m) => Ok(idx.and_then(move |i| m.get_mut(i))),
            Node::Sequence(_, e) => Ok(idx.and_then(move |i| e.get_mut(i))),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// The value an insert or get works on: the value itself without
    /// components, else the current component, one level down only
    /// (§13.2.2.8).
    fn target(&mut self) -> DynResult<&mut DynAny> {
        match &self.node {
            Node::Struct(_) | Node::Sequence(..) => {
                let c = self.current_component()?.ok_or(DynAnyError::InvalidValue)?;
                // "Calling an insert or get operation on a DynAny whose
                // current component itself has components raises
                // TypeMismatch."
                if matches!(c.node, Node::Struct(_) | Node::Sequence(..)) {
                    return Err(DynAnyError::TypeMismatch);
                }
                Ok(c)
            }
            _ => Ok(self),
        }
    }

    fn insert(&mut self, v: AnyValue) -> DynResult<()> {
        let t = self.target()?;
        match (&t.node, unalias(&t.tc)) {
            (Node::Basic(old), tc)
                if core::mem::discriminant(old) == core::mem::discriminant(&v) =>
            {
                match (tc, &v) {
                    (TypeCode::String(b), AnyValue::Str(s))
                        if *b != 0 && s.chars().count() > *b as usize =>
                    {
                        return Err(DynAnyError::InvalidValue);
                    }
                    (TypeCode::WString(b), AnyValue::WStr(s))
                        if *b != 0 && s.0.chars().count() > *b as usize =>
                    {
                        return Err(DynAnyError::InvalidValue);
                    }
                    _ => {}
                }
                t.node = Node::Basic(v);
                Ok(())
            }
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    fn get(&mut self) -> DynResult<AnyValue> {
        let t = self.target()?;
        match &t.node {
            Node::Basic(v) => Ok(v.clone()),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }
}

macro_rules! basic_access {
    ($($insert:ident, $get:ident, $variant:ident, $ty:ty);+ $(;)?) => {
        impl DynAny {
            $(
                #[doc = concat!("§13.2.2.8 `", stringify!($insert), "`.\n\n# Errors\n`TypeMismatch` for another type; `InvalidValue` at position -1 of a constructed value.")]
                pub fn $insert(&mut self, value: $ty) -> DynResult<()> {
                    self.insert(AnyValue::$variant(value))
                }
                #[doc = concat!("§13.2.2.8 `", stringify!($get), "`.\n\n# Errors\nAs the insert operation.")]
                pub fn $get(&mut self) -> DynResult<$ty> {
                    match self.get()? {
                        AnyValue::$variant(v) => Ok(v),
                        _ => Err(DynAnyError::TypeMismatch),
                    }
                }
            )+
        }
    };
}

basic_access! {
    insert_boolean, get_boolean, Boolean, bool;
    insert_octet, get_octet, Octet, u8;
    insert_char, get_char, Char, u8;
    insert_short, get_short, Short, i16;
    insert_ushort, get_ushort, UShort, u16;
    insert_long, get_long, Long, i32;
    insert_ulong, get_ulong, ULong, u32;
    insert_longlong, get_longlong, LongLong, i64;
    insert_ulonglong, get_ulonglong, ULongLong, u64;
    insert_float, get_float, Float, f32;
    insert_double, get_double, Double, f64;
    insert_wchar, get_wchar, WChar, u16;
    insert_string, get_string, Str, String;
    insert_wstring, get_wstring, WStr, WString;
}

impl DynAny {
    /// §13.2.2.8 `insert_any`.
    ///
    /// # Errors
    /// As the other insert operations.
    pub fn insert_any(&mut self, value: CorbaAny) -> DynResult<()> {
        self.insert(AnyValue::Any(Box::new(value)))
    }

    /// §13.2.2.8 `get_any`.
    ///
    /// # Errors
    /// As the other get operations.
    pub fn get_any(&mut self) -> DynResult<CorbaAny> {
        match self.get()? {
            AnyValue::Any(a) => Ok(*a),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// §13.2.2.8 `insert_dyn_any`: the value of `value` as an `any`.
    ///
    /// # Errors
    /// As [`insert_any`](Self::insert_any).
    pub fn insert_dyn_any(&mut self, value: &DynAny) -> DynResult<()> {
        self.insert_any(value.to_any())
    }

    /// §13.2.2.8 `get_dyn_any`: the contained `any` as a `DynAny`.
    ///
    /// # Errors
    /// As [`get_any`](Self::get_any); `TypeMismatch` when the contained
    /// value's kind is not carried here.
    pub fn get_dyn_any(&mut self) -> DynResult<DynAny> {
        let a = self.get_any()?;
        DynAnyFactory
            .create_dyn_any(&a)
            .map_err(|_| DynAnyError::TypeMismatch)
    }

    // -- §13.2.4 DynEnum ----------------------------------------------

    fn enum_parts(&self) -> DynResult<(u32, &[String])> {
        match (&self.node, unalias(&self.tc)) {
            (Node::Enum(v), TypeCode::Enum { members, .. }) => Ok((*v, members)),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `DynEnum::get_as_ulong`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynEnum`.
    pub fn get_as_ulong(&self) -> DynResult<u32> {
        Ok(self.enum_parts()?.0)
    }

    /// `DynEnum::set_as_ulong`.
    ///
    /// # Errors
    /// `InvalidValue` for an ordinal outside the enumerators.
    pub fn set_as_ulong(&mut self, value: u32) -> DynResult<()> {
        let (_, members) = self.enum_parts()?;
        if value as usize >= members.len() {
            return Err(DynAnyError::InvalidValue);
        }
        self.node = Node::Enum(value);
        Ok(())
    }

    /// `DynEnum::get_as_string`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynEnum`.
    pub fn get_as_string(&self) -> DynResult<String> {
        let (v, members) = self.enum_parts()?;
        members
            .get(v as usize)
            .cloned()
            .ok_or(DynAnyError::InvalidValue)
    }

    /// `DynEnum::set_as_string`.
    ///
    /// # Errors
    /// `InvalidValue` for a name that is no enumerator.
    pub fn set_as_string(&mut self, name: &str) -> DynResult<()> {
        let (_, members) = self.enum_parts()?;
        let idx = members
            .iter()
            .position(|m| m == name)
            .ok_or(DynAnyError::InvalidValue)?;
        self.node = Node::Enum(u32::try_from(idx).map_err(|_| DynAnyError::InvalidValue)?);
        Ok(())
    }

    // -- §13.2.5 DynStruct --------------------------------------------

    fn struct_members(&self) -> DynResult<&[(String, TypeCode)]> {
        match (&self.node, unalias(&self.tc)) {
            (Node::Struct(_), TypeCode::Struct { members, .. }) => Ok(members),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `DynStruct::current_member_name`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynStruct` or has no members,
    /// `InvalidValue` at position -1.
    pub fn current_member_name(&self) -> DynResult<String> {
        let members = self.struct_members()?;
        if members.is_empty() {
            return Err(DynAnyError::TypeMismatch);
        }
        let i = usize::try_from(self.current).map_err(|_| DynAnyError::InvalidValue)?;
        Ok(members[i].0.clone())
    }

    /// `DynStruct::current_member_kind`: the `TCKind` of the current
    /// member.
    ///
    /// # Errors
    /// As [`current_member_name`](Self::current_member_name).
    pub fn current_member_kind(&self) -> DynResult<u32> {
        let members = self.struct_members()?;
        if members.is_empty() {
            return Err(DynAnyError::TypeMismatch);
        }
        let i = usize::try_from(self.current).map_err(|_| DynAnyError::InvalidValue)?;
        Ok(members[i].1.tckind())
    }

    /// `DynStruct::get_members`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynStruct`.
    pub fn get_members(&self) -> DynResult<Vec<(String, CorbaAny)>> {
        let members = self.struct_members()?;
        let Node::Struct(items) = &self.node else {
            return Err(DynAnyError::TypeMismatch);
        };
        Ok(members
            .iter()
            .zip(items)
            .map(|((n, _), d)| (n.clone(), d.to_any()))
            .collect())
    }

    /// `DynStruct::set_members`: names, where given, must match the
    /// members in order.
    ///
    /// # Errors
    /// `InvalidValue` for another number of members, `TypeMismatch` for
    /// a name or a value type that does not fit.
    pub fn set_members(&mut self, values: &[(String, CorbaAny)]) -> DynResult<()> {
        let members = self.struct_members()?.to_vec();
        if values.len() != members.len() {
            return Err(DynAnyError::InvalidValue);
        }
        let mut items = Vec::with_capacity(values.len());
        for ((name, tc), (vn, va)) in members.iter().zip(values) {
            if !vn.is_empty() && vn != name {
                return Err(DynAnyError::TypeMismatch);
            }
            let mut d = DynAny::default_for(tc, &mut alloc::vec![self.tc.clone()])
                .map_err(|_| DynAnyError::TypeMismatch)?;
            d.from_any(va)?;
            items.push(d);
        }
        self.node = Node::Struct(items);
        self.current = if members.is_empty() { -1 } else { 0 };
        Ok(())
    }

    /// `DynStruct::get_members_as_dyn_any`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynStruct`.
    pub fn get_members_as_dyn_any(&self) -> DynResult<Vec<(String, DynAny)>> {
        let members = self.struct_members()?;
        let Node::Struct(items) = &self.node else {
            return Err(DynAnyError::TypeMismatch);
        };
        Ok(members
            .iter()
            .zip(items)
            .map(|((n, _), d)| (n.clone(), d.clone()))
            .collect())
    }

    /// `DynStruct::set_members_as_dyn_any`.
    ///
    /// # Errors
    /// As [`set_members`](Self::set_members).
    pub fn set_members_as_dyn_any(&mut self, values: &[(String, DynAny)]) -> DynResult<()> {
        let anys: Vec<(String, CorbaAny)> = values
            .iter()
            .map(|(n, d)| (n.clone(), d.to_any()))
            .collect();
        let members = self.struct_members()?;
        for ((_, tc), (_, d)) in members.iter().zip(values) {
            if !equivalent(tc, &d.tc) {
                return Err(DynAnyError::TypeMismatch);
            }
        }
        self.set_members(&anys)
    }

    // -- §13.2.7 DynSequence ------------------------------------------

    fn sequence_bound(&self) -> DynResult<u32> {
        match (&self.node, unalias(&self.tc)) {
            (Node::Sequence(..), TypeCode::Sequence { bound, .. }) => Ok(*bound),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `DynSequence::get_length`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynSequence`.
    pub fn get_length(&self) -> DynResult<u32> {
        self.sequence_bound()?;
        Ok(self.component_count())
    }

    /// `DynSequence::set_length`: new elements get default values; a
    /// growing sequence at position -1 moves to the first new element, a
    /// shrinking one that loses the current element goes to -1.
    ///
    /// # Errors
    /// `InvalidValue` beyond the bound of a bounded sequence.
    pub fn set_length(&mut self, len: u32) -> DynResult<()> {
        let bound = self.sequence_bound()?;
        if bound != 0 && len > bound {
            return Err(DynAnyError::InvalidValue);
        }
        let Node::Sequence(element, items) = &mut self.node else {
            return Err(DynAnyError::TypeMismatch);
        };
        let old = items.len();
        let len = len as usize;
        if len > old {
            let proto = DynAny::default_for(element, &mut alloc::vec![self.tc.clone()])
                .map_err(|_| DynAnyError::TypeMismatch)?;
            items.resize(len, proto);
            if self.current == -1 {
                self.current = i32::try_from(old).map_err(|_| DynAnyError::InvalidValue)?;
            }
        } else {
            items.truncate(len);
            if usize::try_from(self.current).is_ok_and(|c| c >= len) {
                self.current = -1;
            }
        }
        Ok(())
    }

    /// `DynSequence::get_elements`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynSequence`.
    pub fn get_elements(&self) -> DynResult<Vec<CorbaAny>> {
        self.sequence_bound()?;
        let Node::Sequence(_, items) = &self.node else {
            return Err(DynAnyError::TypeMismatch);
        };
        Ok(items.iter().map(DynAny::to_any).collect())
    }

    /// `DynSequence::set_elements`.
    ///
    /// # Errors
    /// `InvalidValue` beyond the bound, `TypeMismatch` for an element of
    /// another type.
    pub fn set_elements(&mut self, values: &[CorbaAny]) -> DynResult<()> {
        let bound = self.sequence_bound()?;
        if bound != 0 && values.len() > bound as usize {
            return Err(DynAnyError::InvalidValue);
        }
        let Node::Sequence(element, _) = &self.node else {
            return Err(DynAnyError::TypeMismatch);
        };
        let element = (**element).clone();
        let mut items = Vec::with_capacity(values.len());
        for v in values {
            let mut d = DynAny::default_for(&element, &mut alloc::vec![self.tc.clone()])
                .map_err(|_| DynAnyError::TypeMismatch)?;
            d.from_any(v)?;
            items.push(d);
        }
        let current = if items.is_empty() { -1 } else { 0 };
        self.node = Node::Sequence(Box::new(element), items);
        self.current = current;
        Ok(())
    }

    /// `DynSequence::get_elements_as_dyn_any`.
    ///
    /// # Errors
    /// `TypeMismatch` when this is not a `DynSequence`.
    pub fn get_elements_as_dyn_any(&self) -> DynResult<Vec<DynAny>> {
        self.sequence_bound()?;
        let Node::Sequence(_, items) = &self.node else {
            return Err(DynAnyError::TypeMismatch);
        };
        Ok(items.clone())
    }

    /// `DynSequence::set_elements_as_dyn_any`.
    ///
    /// # Errors
    /// As [`set_elements`](Self::set_elements).
    pub fn set_elements_as_dyn_any(&mut self, values: &[DynAny]) -> DynResult<()> {
        if let Node::Sequence(element, _) = &self.node
            && values.iter().any(|d| !equivalent(element, &d.tc))
        {
            return Err(DynAnyError::TypeMismatch);
        }
        let anys: Vec<CorbaAny> = values.iter().map(DynAny::to_any).collect();
        self.set_elements(&anys)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn point_tc() -> TypeCode {
        TypeCode::Struct {
            repo_id: "IDL:demo/Point:1.0".into(),
            name: "Point".into(),
            members: vec![
                ("x".into(), TypeCode::Long),
                ("label".into(), TypeCode::String(4)),
                ("flag".into(), TypeCode::Boolean),
            ],
            is_except: false,
        }
    }

    fn color_tc() -> TypeCode {
        TypeCode::Enum {
            repo_id: "IDL:demo/Color:1.0".into(),
            name: "Color".into(),
            members: vec!["RED".into(), "GREEN".into(), "BLUE".into()],
        }
    }

    fn create(tc: &TypeCode) -> DynAny {
        DynAnyFactory.create_dyn_any_from_type_code(tc).unwrap()
    }

    #[test]
    fn corba_p1_13_2_1_default_values_and_current_positions() {
        let p = create(&point_tc());
        assert_eq!(p.component_count(), 3);
        assert_eq!(p.current, 0, "a struct with members starts at 0");
        let CorbaAny(AnyValue::Struct { members, .. }) = p.to_any() else {
            panic!("struct");
        };
        assert_eq!(
            members,
            vec![
                ("x".to_string(), AnyValue::Long(0)),
                ("label".to_string(), AnyValue::Str(String::new())),
                ("flag".to_string(), AnyValue::Boolean(false)),
            ]
        );
        let e = create(&color_tc());
        assert_eq!(e.get_as_string().unwrap(), "RED", "the first enumerator");
        assert_eq!(e.current, -1);
        let s = create(&TypeCode::Sequence {
            element: Box::new(TypeCode::Short),
            bound: 0,
        });
        assert_eq!((s.get_length().unwrap(), s.current), (0, -1));
        // tk_null and tk_void: legal, no value, no components.
        for tc in [TypeCode::Null, TypeCode::Void] {
            assert_eq!(create(&tc).component_count(), 0);
        }
        let any = create(&TypeCode::Any).to_any();
        assert_eq!(any, CorbaAny(AnyValue::Any(Box::default())));
        // An empty exception starts at -1.
        let ex = create(&TypeCode::Struct {
            repo_id: "IDL:demo/Oops:1.0".into(),
            name: "Oops".into(),
            members: vec![],
            is_except: true,
        });
        assert_eq!(ex.current, -1);
    }

    #[test]
    fn corba_p1_13_2_1_kinds_without_an_any_value_are_inconsistent() {
        for tc in [
            TypeCode::TypeCodeTc,
            TypeCode::ObjRef {
                repo_id: "IDL:X:1.0".into(),
                name: "X".into(),
            },
            TypeCode::Recursive {
                repo_id: "IDL:X:1.0".into(),
            },
        ] {
            assert_eq!(
                DynAnyFactory.create_dyn_any_from_type_code(&tc),
                Err(InconsistentTypeCode)
            );
        }
    }

    #[test]
    fn corba_p1_13_2_2_8_insert_and_get_per_basic_type() {
        let mut d = create(&TypeCode::Long);
        d.insert_long(-7).unwrap();
        assert_eq!(d.get_long().unwrap(), -7);
        assert_eq!(d.insert_short(1), Err(DynAnyError::TypeMismatch));
        assert_eq!(d.get_ulong(), Err(DynAnyError::TypeMismatch));
        macro_rules! roundtrip {
            ($tc:expr, $ins:ident, $get:ident, $v:expr) => {{
                let mut d = create(&$tc);
                d.$ins($v.clone()).unwrap();
                assert_eq!(d.$get().unwrap(), $v);
                assert_eq!(d.to_any().0.type_code(), $tc);
            }};
        }
        roundtrip!(TypeCode::Boolean, insert_boolean, get_boolean, true);
        roundtrip!(TypeCode::Octet, insert_octet, get_octet, 0xfe_u8);
        roundtrip!(TypeCode::Char, insert_char, get_char, b'z');
        roundtrip!(TypeCode::Short, insert_short, get_short, -3_i16);
        roundtrip!(TypeCode::UShort, insert_ushort, get_ushort, 65_000_u16);
        roundtrip!(TypeCode::ULong, insert_ulong, get_ulong, 4_000_000_000_u32);
        roundtrip!(TypeCode::LongLong, insert_longlong, get_longlong, -9_i64);
        roundtrip!(
            TypeCode::ULongLong,
            insert_ulonglong,
            get_ulonglong,
            u64::MAX
        );
        roundtrip!(TypeCode::Float, insert_float, get_float, 1.5_f32);
        roundtrip!(TypeCode::Double, insert_double, get_double, -2.25_f64);
        roundtrip!(TypeCode::WChar, insert_wchar, get_wchar, 0x263a_u16);
        roundtrip!(
            TypeCode::String(0),
            insert_string,
            get_string,
            "text".to_string()
        );
        roundtrip!(
            TypeCode::WString(0),
            insert_wstring,
            get_wstring,
            WString("wide".into())
        );
        let mut a = create(&TypeCode::Any);
        a.insert_any(CorbaAny(AnyValue::Double(3.0))).unwrap();
        assert_eq!(a.get_any().unwrap(), CorbaAny(AnyValue::Double(3.0)));
        let inner = a.get_dyn_any().unwrap();
        assert_eq!(inner.type_code(), &TypeCode::Double);
        // A bounded string refuses a longer value.
        let mut b = create(&TypeCode::String(3));
        assert_eq!(
            b.insert_string("four".into()),
            Err(DynAnyError::InvalidValue)
        );
    }

    #[test]
    fn corba_p1_13_2_2_9_iteration_over_the_members_of_a_struct() {
        let mut p = create(&point_tc());
        // Insert works on the current component.
        p.insert_long(42).unwrap();
        assert!(p.next());
        assert_eq!(p.current_member_name().unwrap(), "label");
        assert_eq!(
            p.current_member_kind().unwrap(),
            crate::type_code::tckind::TK_STRING
        );
        p.insert_string("abc".into()).unwrap();
        assert_eq!(p.insert_long(1), Err(DynAnyError::TypeMismatch));
        assert!(p.next());
        p.current_component()
            .unwrap()
            .unwrap()
            .insert_boolean(true)
            .unwrap();
        assert!(!p.next(), "past the last member");
        assert_eq!(p.current, -1);
        assert_eq!(p.insert_long(1), Err(DynAnyError::InvalidValue));
        assert!(p.current_component().unwrap().is_none());
        assert_eq!(p.current_member_name(), Err(DynAnyError::InvalidValue));
        p.rewind();
        assert_eq!(p.get_long().unwrap(), 42);
        assert!(!p.seek(3));
        assert!(p.seek(1));
        assert_eq!(p.get_string().unwrap(), "abc");
        // A basic value has no components.
        assert_eq!(
            create(&TypeCode::Long).current_component().err(),
            Some(DynAnyError::TypeMismatch)
        );
    }

    #[test]
    fn corba_p1_13_2_5_struct_members_get_and_set() {
        let mut p = create(&point_tc());
        let members = vec![
            ("x".to_string(), CorbaAny(AnyValue::Long(5))),
            ("label".to_string(), CorbaAny(AnyValue::Str("ok".into()))),
            ("flag".to_string(), CorbaAny(AnyValue::Boolean(true))),
        ];
        p.set_members(&members).unwrap();
        assert_eq!(p.get_members().unwrap(), members);
        let mut wrong_name = members.clone();
        wrong_name[0].0 = "y".into();
        assert_eq!(p.set_members(&wrong_name), Err(DynAnyError::TypeMismatch));
        assert_eq!(p.set_members(&members[..2]), Err(DynAnyError::InvalidValue));
        let mut wrong_type = members.clone();
        wrong_type[2].1 = CorbaAny(AnyValue::Long(1));
        assert_eq!(p.set_members(&wrong_type), Err(DynAnyError::TypeMismatch));
        let as_dyn = p.get_members_as_dyn_any().unwrap();
        let mut q = create(&point_tc());
        q.set_members_as_dyn_any(&as_dyn).unwrap();
        assert!(q.equal(&p));
        // Operations of another interface: TypeMismatch.
        assert_eq!(p.get_as_ulong(), Err(DynAnyError::TypeMismatch));
        assert_eq!(p.get_length(), Err(DynAnyError::TypeMismatch));
    }

    #[test]
    fn corba_p1_13_2_4_enum_as_string_and_ulong() {
        let mut e = create(&color_tc());
        e.set_as_string("BLUE").unwrap();
        assert_eq!(e.get_as_ulong().unwrap(), 2);
        e.set_as_ulong(1).unwrap();
        assert_eq!(e.get_as_string().unwrap(), "GREEN");
        assert_eq!(e.set_as_ulong(3), Err(DynAnyError::InvalidValue));
        assert_eq!(e.set_as_string("PINK"), Err(DynAnyError::InvalidValue));
        let CorbaAny(AnyValue::Enum { value, .. }) = e.to_any() else {
            panic!("enum");
        };
        assert_eq!(value, 1);
    }

    #[test]
    fn corba_p1_13_2_7_sequence_length_elements_and_bound() {
        let tc = TypeCode::Sequence {
            element: Box::new(TypeCode::Long),
            bound: 3,
        };
        let mut s = create(&tc);
        s.set_length(2).unwrap();
        assert_eq!(
            s.current, 0,
            "growing from -1 moves to the first new element"
        );
        assert_eq!(
            s.get_elements().unwrap(),
            vec![CorbaAny(AnyValue::Long(0)), CorbaAny(AnyValue::Long(0))]
        );
        s.seek(1);
        s.insert_long(9).unwrap();
        assert_eq!(
            s.set_length(4),
            Err(DynAnyError::InvalidValue),
            "beyond the bound"
        );
        s.set_length(1).unwrap();
        assert_eq!(s.current, -1, "the current element was cut off");
        s.set_elements(&[CorbaAny(AnyValue::Long(1)), CorbaAny(AnyValue::Long(2))])
            .unwrap();
        assert_eq!(s.get_length().unwrap(), 2);
        assert_eq!(
            s.set_elements(&[CorbaAny(AnyValue::Short(1))]),
            Err(DynAnyError::TypeMismatch)
        );
        let four = vec![CorbaAny(AnyValue::Long(0)); 4];
        assert_eq!(s.set_elements(&four), Err(DynAnyError::InvalidValue));
        let elems = s.get_elements_as_dyn_any().unwrap();
        let mut t = create(&tc);
        t.set_elements_as_dyn_any(&elems).unwrap();
        assert!(t.equal(&s));
        let CorbaAny(AnyValue::Seq { items, .. }) = s.to_any() else {
            panic!("seq");
        };
        assert_eq!(items, vec![AnyValue::Long(1), AnyValue::Long(2)]);
    }

    #[test]
    fn corba_p1_13_2_2_from_any_assign_equal_and_copy() {
        let value = CorbaAny(AnyValue::Struct {
            repo_id: "IDL:demo/Point:1.0".into(),
            name: "Point".into(),
            members: vec![
                ("x".into(), AnyValue::Long(3)),
                ("label".into(), AnyValue::Str("abcd".into())),
                ("flag".into(), AnyValue::Boolean(true)),
            ],
        });
        let mut p = create(&point_tc());
        p.from_any(&value).unwrap();
        assert_eq!(p.to_any(), value);
        assert_eq!(p.current, 0);
        // A label longer than string<4>: InvalidValue.
        let mut long = value.clone();
        if let AnyValue::Struct { members, .. } = &mut long.0 {
            members[1].1 = AnyValue::Str("abcde".into());
        }
        assert_eq!(p.from_any(&long), Err(DynAnyError::InvalidValue));
        assert_eq!(
            p.from_any(&CorbaAny(AnyValue::Long(1))),
            Err(DynAnyError::TypeMismatch)
        );
        // equal ignores the current position; copy is independent.
        let mut q = p.copy();
        q.seek(2);
        assert!(q.equal(&p));
        q.insert_boolean(false).unwrap();
        assert!(!q.equal(&p));
        q.assign(&p).unwrap();
        assert!(q.equal(&p));
        assert_eq!(
            q.assign(&create(&color_tc())),
            Err(DynAnyError::TypeMismatch)
        );
        // An alias is equivalent to its content type.
        let alias = TypeCode::Alias {
            repo_id: "IDL:demo/Meters:1.0".into(),
            name: "Meters".into(),
            content: Box::new(TypeCode::Long),
        };
        let mut m = create(&alias);
        m.insert_long(12).unwrap();
        assert_eq!(
            m.type_code(),
            &alias,
            "type() is the TypeCode it was made with"
        );
        let mut l = create(&TypeCode::Long);
        l.assign(&m).unwrap();
        assert_eq!(l.get_long().unwrap(), 12);
        // The factory from an any.
        let d = DynAnyFactory.create_dyn_any(&value).unwrap();
        assert_eq!(d.to_any(), value);
        let many = DynAnyFactory
            .create_multiple_dyn_anys(&[value.clone(), CorbaAny(AnyValue::Octet(1))])
            .unwrap();
        assert_eq!(
            DynAnyFactory.create_multiple_anys(&many),
            vec![value, CorbaAny(AnyValue::Octet(1))]
        );
    }

    #[test]
    fn corba_p1_13_2_7_a_recursive_sequence_member_builds_nested_values() {
        // struct Node { long id; sequence<Node> kids; };
        let node = TypeCode::Struct {
            repo_id: "IDL:demo/Node:1.0".into(),
            name: "Node".into(),
            members: vec![
                ("id".into(), TypeCode::Long),
                (
                    "kids".into(),
                    TypeCode::Sequence {
                        element: Box::new(TypeCode::Recursive {
                            repo_id: "IDL:demo/Node:1.0".into(),
                        }),
                        bound: 0,
                    },
                ),
            ],
            is_except: false,
        };
        let mut root = create(&node);
        root.insert_long(1).unwrap();
        root.next();
        let kids = root.current_component().unwrap().unwrap();
        kids.set_length(1).unwrap();
        let child = kids.current_component().unwrap().unwrap();
        child.insert_long(2).unwrap();
        let CorbaAny(AnyValue::Struct { members, .. }) = root.to_any() else {
            panic!("struct");
        };
        let AnyValue::Seq { items, .. } = &members[1].1 else {
            panic!("seq");
        };
        let AnyValue::Struct { members: child, .. } = &items[0] else {
            panic!("child struct");
        };
        assert_eq!(child[0].1, AnyValue::Long(2));
        // The any of the tree reads back into a DynAny of the same type.
        let mut again = create(&node);
        again.from_any(&root.to_any()).unwrap();
        assert!(again.equal(&root));
    }

    #[test]
    fn corba_p1_13_2_2_8_a_current_component_with_components_is_type_mismatch() {
        // struct Outer { Point p; long y; };
        let outer_tc = TypeCode::Struct {
            repo_id: "IDL:demo/Outer:1.0".into(),
            name: "Outer".into(),
            members: vec![("p".into(), point_tc()), ("y".into(), TypeCode::Long)],
            is_except: false,
        };
        let mut outer = create(&outer_tc);
        assert_eq!(outer.current, 0);
        // The current component p has components: the operation does not
        // reach into p.x.
        assert_eq!(outer.insert_long(5), Err(DynAnyError::TypeMismatch));
        assert_eq!(outer.get_long(), Err(DynAnyError::TypeMismatch));
        // Also when p's own position is -1.
        assert!(!outer.current_component().unwrap().unwrap().seek(-1));
        assert_eq!(outer.insert_long(5), Err(DynAnyError::TypeMismatch));
        let CorbaAny(AnyValue::Struct { members, .. }) = outer.to_any() else {
            panic!("struct");
        };
        let AnyValue::Struct { members: p, .. } = &members[0].1 else {
            panic!("p");
        };
        assert_eq!(p[0].1, AnyValue::Long(0), "p.x unchanged");
        // The basic member y takes the value.
        assert!(outer.next());
        outer.insert_long(9).unwrap();
        assert_eq!(outer.get_long().unwrap(), 9);
        // sequence<sequence<long>> with one element: the element has
        // components.
        let mut seq = create(&TypeCode::Sequence {
            element: Box::new(TypeCode::Sequence {
                element: Box::new(TypeCode::Long),
                bound: 0,
            }),
            bound: 0,
        });
        seq.set_length(1).unwrap();
        assert_eq!(seq.insert_long(1), Err(DynAnyError::TypeMismatch));
    }

    #[test]
    fn corba_p1_13_2_1_an_empty_struct_starts_at_0_an_empty_exception_has_no_components() {
        let empty_struct = create(&TypeCode::Struct {
            repo_id: "IDL:demo/Empty:1.0".into(),
            name: "Empty".into(),
            members: vec![],
            is_except: false,
        });
        assert_eq!(empty_struct.current, 0, "zero for all but empty exceptions");
        let mut ex = create(&TypeCode::Struct {
            repo_id: "IDL:demo/Oops:1.0".into(),
            name: "Oops".into(),
            members: vec![],
            is_except: true,
        });
        assert_eq!(ex.current, -1);
        assert_eq!(ex.component_count(), 0);
        // §13.2.2.9: an empty exception cannot have components.
        assert_eq!(
            ex.current_component().err(),
            Some(DynAnyError::TypeMismatch)
        );
        assert_eq!(ex.get_long(), Err(DynAnyError::TypeMismatch));
        assert_eq!(ex.current_member_name(), Err(DynAnyError::TypeMismatch));
        // An exception with members has components.
        let mut full = create(&TypeCode::Struct {
            repo_id: "IDL:demo/Range:1.0".into(),
            name: "Range".into(),
            members: vec![("limit".into(), TypeCode::Long)],
            is_except: true,
        });
        assert_eq!(full.current, 0);
        assert!(full.current_component().unwrap().is_some());
    }
}
