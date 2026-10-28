// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Built-in Types Set. XTypes 1.3 §7.6.5 and Annex E.
//!
//! Four types every conformant implementation pre-registers, so that a
//! demo, a tutorial or a command-line publisher has a topic type without
//! anyone writing IDL:
//!
//! ```idl
//! @appendable struct DDS::String      { string value; };
//! @appendable struct DDS::KeyedString { @key string key; string value; };
//! @appendable struct DDS::Bytes       { sequence<octet> value; };
//! @appendable struct DDS::KeyedBytes  { @key string key; sequence<octet> value; };
//! ```
//!
//! Cyclone DDS, Fast DDS and RTI Connext all register them by default;
//! without them `zerodds-admin pub <topic> "Hello"` has nothing to
//! publish.
//!
//! # The representation, and why it is the AST
//!
//! This was `zerodds_types::dynamic`, whose `DynamicType` was a third
//! type representation beside the IDL AST and the XTypes `TypeObject`.
//! The rebuild's answer is that a declared type **is**
//! [`zerodds_ast::Ast`] — `docs/OPEN-ITEMS.md` states the same shape for
//! the `DynamicType` that has still to come back: a view on the model
//! rather than a copy of it.
//!
//! So the four are declared into one `Ast` here, and [`BuiltinType`] and
//! [`BuiltinMember`] are borrowed views onto it. What that buys beyond
//! not maintaining a second model: the handle a caller gets out is the
//! one `zerodds-ast-layout` plans from and `zerodds-wire-codec`
//! executes, so a built-in type reaches the byte runtime by the same
//! path every generated type does — see [`BuiltinType::ast`] and
//! [`BuiltinType::id`].
//!
//! # What is not here
//!
//! `DynamicData` and the reflective codec (XTypes §7.5.2, §7.6.3). This
//! module answers "what is `DDS::KeyedString`" and not "here is an
//! instance of it"; the second is the open item, and a half of it
//! invented here would be the third representation again.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_ast::{
    Ast, AstError, Bound, CharWidth, Extensibility, MemberNode, MemberSpec, PrimitiveKind,
    TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// The spec type name `"DDS::String"`. XTypes 1.3 §7.6.5.
pub const NAME_DDS_STRING: &str = "DDS::String";

/// The spec type name `"DDS::KeyedString"`.
pub const NAME_DDS_KEYED_STRING: &str = "DDS::KeyedString";

/// The spec type name `"DDS::Bytes"`.
pub const NAME_DDS_BYTES: &str = "DDS::Bytes";

/// The spec type name `"DDS::KeyedBytes"`.
pub const NAME_DDS_KEYED_BYTES: &str = "DDS::KeyedBytes";

/// The four names, in the order §7.6.5 lists them.
pub const BUILTIN_TYPE_NAMES: [&str; 4] = [
    NAME_DDS_STRING,
    NAME_DDS_KEYED_STRING,
    NAME_DDS_BYTES,
    NAME_DDS_KEYED_BYTES,
];

/// Whether `name` is one of the four §7.6.5 names.
///
/// Exact, case-sensitive equality: `"dds::string"` is not `"DDS::String"`
/// and a participant that accepted it would match a remote topic type
/// name no vendor writes.
#[must_use]
pub fn is_builtin_type_name(name: &str) -> bool {
    BUILTIN_TYPE_NAMES.contains(&name)
}

/// The four types of §7.6.5, declared into one model.
pub struct BuiltinTypeSet {
    ast: Arc<Ast>,
    by_name: BTreeMap<String, Id<TypeNode>>,
}

impl BuiltinTypeSet {
    /// Declare the four types.
    ///
    /// # Errors
    /// [`AstError`] if the model refuses a declaration, which for these
    /// four fixed types means the model API itself changed.
    pub fn build() -> Result<Self, AstError> {
        let mut ast = Ast::new();
        let root = ast.root_module();

        // The element and member types the four share. Declared once:
        // two `string` members of two structs are one type.
        let string = ast.add_type(
            root,
            "string",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )?;
        let octet = ast.add_type(root, "octet", TypeKind::Primitive(PrimitiveKind::Octet))?;
        let octet_sequence = ast.add_type(
            root,
            "sequence<octet>",
            TypeKind::Sequence {
                element: Ref::Resolved(octet),
                bound: Bound::Unbounded,
            },
        )?;

        let mut by_name = BTreeMap::new();
        for (name, keyed, value_type) in [
            (NAME_DDS_STRING, false, string),
            (NAME_DDS_KEYED_STRING, true, string),
            (NAME_DDS_BYTES, false, octet_sequence),
            (NAME_DDS_KEYED_BYTES, true, octet_sequence),
        ] {
            let id = ast.add_type(
                root,
                name,
                TypeKind::Structure {
                    base: None,
                    members: Vec::new(),
                },
            )?;
            // Annex E: `@extensibility(APPENDABLE)`, and no `@nested`.
            // §7.6.5 calls them "non-nested types used with DDS": the
            // wrapper structure exists "to provide the Service
            // implementation with a non-nested type definition", so each is
            // a topic type of its own.
            let any = ast.any_of_type(id)?;
            if let Some(facets) = ast.facets_mut(any) {
                facets.xtypes_mut().extensibility = Some(Extensibility::Appendable);
            }
            if keyed {
                let member = ast.add_member(id, MemberSpec::new("key", Ref::Resolved(string)))?;
                let any = ast.any_of_member(member)?;
                if let Some(facets) = ast.facets_mut(any) {
                    facets.xtypes_mut().is_key = true;
                }
            }
            let _ = ast.add_member(id, MemberSpec::new("value", Ref::Resolved(value_type)))?;
            by_name.insert(name.to_string(), id);
        }

        Ok(Self {
            ast: Arc::new(ast),
            by_name,
        })
    }

    /// The type of that name, if it is one of the four.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<BuiltinType> {
        self.by_name.get(name).map(|id| BuiltinType {
            ast: Arc::clone(&self.ast),
            id: *id,
        })
    }

    /// Every type in the set, in §7.6.5's order.
    #[must_use]
    pub fn all(&self) -> Vec<(String, BuiltinType)> {
        BUILTIN_TYPE_NAMES
            .iter()
            .filter_map(|name| self.get(name).map(|t| ((*name).to_string(), t)))
            .collect()
    }
}

/// One built-in type: a handle on the model and the node in it.
#[derive(Clone)]
pub struct BuiltinType {
    ast: Arc<Ast>,
    id: Id<TypeNode>,
}

impl BuiltinType {
    /// The model this type is declared in.
    ///
    /// The reason the handle is given out rather than a copy of the
    /// type's shape: `zerodds_ast_layout::plan` takes exactly this pair,
    /// so a caller that has a `BuiltinType` can compute the type's byte
    /// layout without this crate having to re-describe it.
    #[must_use]
    pub fn ast(&self) -> &Ast {
        &self.ast
    }

    /// The node.
    #[must_use]
    pub fn id(&self) -> Id<TypeNode> {
        self.id
    }

    /// The declared name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.ast.type_(self.id).map_or("", |t| t.name.as_str())
    }

    /// What kind of type it is — a struct, for all four.
    #[must_use]
    pub fn kind(&self) -> Option<&TypeKind> {
        self.ast.type_(self.id).ok().map(|t| &t.kind)
    }

    /// Whether it is annotated `@nested`.
    #[must_use]
    pub fn is_nested(&self) -> bool {
        self.ast
            .any_of_type(self.id)
            .ok()
            .and_then(|any| self.ast.facets(any))
            .and_then(|f| f.xtypes.as_ref())
            .is_some_and(|x| x.is_nested)
    }

    /// Its declared extensibility (`APPENDABLE` for all four, Annex E).
    #[must_use]
    pub fn extensibility(&self) -> Option<Extensibility> {
        self.ast
            .any_of_type(self.id)
            .ok()
            .and_then(|any| self.ast.facets(any))
            .and_then(|f| f.xtypes.as_ref())
            .and_then(|x| x.extensibility)
    }

    /// Its members, in declaration order.
    #[must_use]
    pub fn members(&self) -> Vec<BuiltinMember> {
        self.ast
            .all_members_of(self.id)
            .into_iter()
            .map(|id| BuiltinMember {
                ast: Arc::clone(&self.ast),
                id,
            })
            .collect()
    }

    /// How many members it declares.
    #[must_use]
    pub fn member_count(&self) -> usize {
        self.ast.all_members_of(self.id).len()
    }

    /// The member of that name.
    #[must_use]
    pub fn member_by_name(&self, name: &str) -> Option<BuiltinMember> {
        self.members().into_iter().find(|m| m.name() == name)
    }
}

/// One member of a built-in type.
#[derive(Clone)]
pub struct BuiltinMember {
    ast: Arc<Ast>,
    id: Id<MemberNode>,
}

impl BuiltinMember {
    /// The declared name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.ast.member(self.id).map_or("", |m| m.name.as_str())
    }

    /// Whether it is annotated `@key`.
    #[must_use]
    pub fn is_key(&self) -> bool {
        self.ast.member(self.id).is_ok_and(|m| m.facets.is_key())
    }

    /// Its declared type, when the reference resolves.
    ///
    /// `None` for a reference that does not, which cannot happen for
    /// these four — every one of them is declared in the same
    /// transaction as its members — but is not asserted away, because a
    /// caller reading a member of a type it did not build has to be able
    /// to tell.
    #[must_use]
    pub fn type_(&self) -> Option<BuiltinType> {
        let member = self.ast.member(self.id).ok()?;
        match member.type_ {
            Ref::Resolved(id) => Some(BuiltinType {
                ast: Arc::clone(&self.ast),
                id,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn set() -> BuiltinTypeSet {
        BuiltinTypeSet::build().expect("the four built-in types declare")
    }

    #[test]
    fn all_four_are_declared() {
        let s = set();
        assert_eq!(s.all().len(), 4);
        for name in BUILTIN_TYPE_NAMES {
            assert!(s.get(name).is_some(), "{name} is missing");
        }
    }

    #[test]
    fn each_one_is_an_appendable_non_nested_struct_under_its_spec_name() {
        let s = set();
        for name in BUILTIN_TYPE_NAMES {
            let t = s.get(name).unwrap();
            assert_eq!(t.name(), name);
            assert!(
                matches!(t.kind(), Some(TypeKind::Structure { .. })),
                "{name}"
            );
            assert!(!t.is_nested(), "{name} is @nested (§7.6.5: non-nested)");
            assert_eq!(
                t.extensibility(),
                Some(Extensibility::Appendable),
                "{name} (Annex E)"
            );
        }
    }

    #[test]
    fn the_two_keyed_ones_carry_a_key_and_the_other_two_do_not() {
        let s = set();
        for name in [NAME_DDS_KEYED_STRING, NAME_DDS_KEYED_BYTES] {
            let t = s.get(name).unwrap();
            assert_eq!(t.member_count(), 2, "{name}");
            let key = t.member_by_name("key").expect("key member");
            assert!(key.is_key(), "{name}: key is not @key");
            assert!(matches!(
                key.type_().and_then(|t| t.kind().cloned()),
                Some(TypeKind::String {
                    width: CharWidth::Char,
                    bound: Bound::Unbounded
                })
            ));
        }
        for name in [NAME_DDS_STRING, NAME_DDS_BYTES] {
            let t = s.get(name).unwrap();
            assert_eq!(t.member_count(), 1, "{name}");
            assert!(t.member_by_name("key").is_none(), "{name}");
            assert!(!t.members()[0].is_key(), "{name}: value must not be @key");
        }
    }

    #[test]
    fn the_string_types_hold_an_unbounded_string() {
        let s = set();
        for name in [NAME_DDS_STRING, NAME_DDS_KEYED_STRING] {
            let value = s.get(name).unwrap().member_by_name("value").unwrap();
            assert!(matches!(
                value.type_().and_then(|t| t.kind().cloned()),
                Some(TypeKind::String {
                    width: CharWidth::Char,
                    bound: Bound::Unbounded
                })
            ));
        }
    }

    #[test]
    fn the_bytes_types_hold_an_unbounded_octet_sequence() {
        let s = set();
        for name in [NAME_DDS_BYTES, NAME_DDS_KEYED_BYTES] {
            let t = s.get(name).unwrap();
            let value = t.member_by_name("value").unwrap();
            let element = match value.type_().and_then(|t| t.kind().cloned()) {
                Some(TypeKind::Sequence {
                    element,
                    bound: Bound::Unbounded,
                }) => element,
                other => panic!("{name}: value is {other:?}"),
            };
            let Ref::Resolved(element) = element else {
                panic!("{name}: the element type does not resolve");
            };
            assert!(matches!(
                t.ast().type_(element).map(|t| t.kind.clone()),
                Ok(TypeKind::Primitive(PrimitiveKind::Octet))
            ));
        }
    }

    #[test]
    fn a_name_that_is_not_one_of_the_four() {
        let s = set();
        assert!(s.get("My::Custom").is_none());
        assert!(s.get("dds::string").is_none());
        assert!(!is_builtin_type_name("DDS::String_"));
        assert!(!is_builtin_type_name("dds::string"));
        for name in BUILTIN_TYPE_NAMES {
            assert!(is_builtin_type_name(name));
        }
    }

    /// The set names four types and `is_builtin_type_name` answers for
    /// four names, and a fifth added to one and not the other would let
    /// a participant report a type it cannot produce.
    #[test]
    fn the_predicate_and_the_set_agree() {
        let s = set();
        for (name, _) in s.all() {
            assert!(is_builtin_type_name(&name), "{name}");
        }
        assert_eq!(s.all().len(), BUILTIN_TYPE_NAMES.len());
    }
}
