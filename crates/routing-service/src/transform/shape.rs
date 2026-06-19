// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Flat type shapes for content filtering / transformation.
//!
//! A [`TypeShape`] describes a top-level struct as an ordered list of scalar /
//! string [`Member`]s plus its extensibility (whether an XCDR2 DHEADER frames
//! the body). This covers the practical routing-transformation case (flat
//! `@final` / `@appendable` structs of primitives and strings). Nested structs,
//! sequences, unions and `@mutable` member headers are a documented extension.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Scalar member kinds (CDR primitives + bounded/unbounded string).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarKind {
    /// `boolean` (1 byte).
    Bool,
    /// `octet` / `uint8`.
    U8,
    /// `int8`.
    I8,
    /// `uint16`.
    U16,
    /// `int16`.
    I16,
    /// `uint32`.
    U32,
    /// `int32`.
    I32,
    /// `uint64`.
    U64,
    /// `int64`.
    I64,
    /// `float`.
    F32,
    /// `double`.
    F64,
    /// `string` (UTF-8, length-prefixed + NUL).
    String,
}

impl ScalarKind {
    /// Natural alignment of the primitive (strings align on their u32 length).
    #[must_use]
    pub fn natural_align(self) -> usize {
        match self {
            ScalarKind::Bool | ScalarKind::U8 | ScalarKind::I8 => 1,
            ScalarKind::U16 | ScalarKind::I16 => 2,
            ScalarKind::U32 | ScalarKind::I32 | ScalarKind::F32 | ScalarKind::String => 4,
            ScalarKind::U64 | ScalarKind::I64 | ScalarKind::F64 => 8,
        }
    }

    /// XCDR2 alignment rule: `min(natural, 4)` (XTypes 1.3 §7.4.2.5.1).
    #[must_use]
    pub fn xcdr2_align(self) -> usize {
        self.natural_align().min(4)
    }
}

/// One struct member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Member name (matches the IDL member name).
    pub name: String,
    /// Member scalar kind.
    pub kind: ScalarKind,
}

/// A flat top-level struct shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeShape {
    /// Registered type name (matches `registered_type_name`).
    pub name: String,
    /// Ordered members.
    pub members: Vec<Member>,
    /// `true` if the struct is `@appendable` (an XCDR2 DHEADER frames the
    /// body); `false` for `@final`. Ignored for XCDR1 (never delimited).
    #[serde(default)]
    pub appendable: bool,
}

impl TypeShape {
    /// Looks up a member index by name.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.members.iter().position(|m| m.name == name)
    }

    /// Whether a member of the given name exists.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.index_of(name).is_some()
    }
}

/// A name→shape registry the router consults when a route needs decoding.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    shapes: BTreeMap<String, TypeShape>,
}

impl TypeRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers (or replaces) a shape under its `name`.
    pub fn insert(&mut self, shape: TypeShape) -> &mut Self {
        self.shapes.insert(shape.name.clone(), shape);
        self
    }

    /// Looks up a shape by type name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&TypeShape> {
        self.shapes.get(name)
    }

    /// Parses a registry from JSON (an array of [`TypeShape`]).
    ///
    /// # Errors
    /// [`crate::error::RoutingError::Config`] on malformed JSON.
    pub fn from_json(s: &str) -> crate::error::Result<Self> {
        let shapes: Vec<TypeShape> = serde_json::from_str(s)
            .map_err(|e| crate::error::RoutingError::Config(format!("type shapes json: {e}")))?;
        let mut reg = Self::new();
        for sh in shapes {
            reg.insert(sh);
        }
        Ok(reg)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn align_rules() {
        assert_eq!(ScalarKind::U64.natural_align(), 8);
        assert_eq!(ScalarKind::U64.xcdr2_align(), 4);
        assert_eq!(ScalarKind::U16.xcdr2_align(), 2);
        assert_eq!(ScalarKind::String.natural_align(), 4);
    }

    #[test]
    fn registry_json() {
        let js =
            r#"[{"name":"T","members":[{"name":"a","kind":"u32"},{"name":"b","kind":"string"}]}]"#;
        let reg = TypeRegistry::from_json(js).unwrap();
        let sh = reg.get("T").unwrap();
        assert_eq!(sh.members.len(), 2);
        assert_eq!(sh.index_of("b"), Some(1));
        assert!(!sh.appendable);
    }
}
