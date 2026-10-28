// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A type registry that resolves nested-message references across a whole
//! [`FileDescriptorSet`], so a message field of another message type maps to a
//! fully-populated nested XTypes struct (attached via `add_member_resolved`, the
//! same mechanism the TypeObject bridge uses).
//!
//! Messages are indexed by their fully-qualified name (`package.Outer.Inner`).
//! Leaf fields (scalars, `string`, `bytes`, `repeated` of those) reuse
//! `crate::mapper::field_descriptor`. Singular message and enum fields recurse
//! into resolved nested structs / enums; `repeated` message and enum fields
//! become `sequence<struct>` / `sequence<enum>`; a proto `map<K,V>` (encoded by
//! `protoc` as a `repeated` synthetic `*Entry` message) becomes an XTypes
//! `map<K,V>`. All composite members are attached via `add_member_resolved` so
//! the reflective codec keeps the full element type graph.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};

use alloc::sync::Arc;

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_types::dynamic::{
    DynamicType, DynamicTypeBuilder, ExtensibilityKind, NewMember, TypeKind,
};

use crate::ProtoFieldType;
use crate::descriptor::{Enum, Field, FileDescriptorSet, Label, Message};
use crate::mapper::{MapError, build_error, element_type, field_type};

/// Resolves message and enum types by fully-qualified name into XTypes
/// types.
///
/// Every type goes into **one** model, which is what lets a message field
/// name another message: a `DynamicType` is a view on a
/// [`zerodds_ast::Ast`], and two types built into two models cannot refer
/// to each other. So [`Registry::build`] answers a handle, and
/// [`Registry::finish`] hands over the finished model at the end —
/// [`Registry::into_type`] does both for a caller that wants one type.
pub struct Registry {
    messages: BTreeMap<String, Message>,
    enums: BTreeMap<String, Enum>,
    extensibility: ExtensibilityKind,
    builder: DynamicTypeBuilder,
    cache: BTreeMap<String, Id<TypeNode>>,
}

fn join_fqn(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        return name.into();
    }
    let mut s = String::with_capacity(prefix.len() + 1 + name.len());
    s.push_str(prefix);
    s.push('.');
    s.push_str(name);
    s
}

/// zerodds-lint: recursion-depth 64 (message nesting; bounded by .proto depth)
fn index_message(
    prefix: &str,
    msg: &Message,
    msgs: &mut BTreeMap<String, Message>,
    enums: &mut BTreeMap<String, Enum>,
) {
    let fqn = join_fqn(prefix, &msg.name);
    for nested in &msg.nested {
        index_message(&fqn, nested, msgs, enums);
    }
    for e in &msg.enums {
        enums.insert(join_fqn(&fqn, &e.name), e.clone());
    }
    msgs.insert(fqn, msg.clone());
}

impl Registry {
    /// Index every message and enum in the set (top-level and nested) by
    /// fully-qualified name, ready for [`Registry::build`].
    #[must_use]
    pub fn from_set(set: &FileDescriptorSet, extensibility: ExtensibilityKind) -> Self {
        let mut messages = BTreeMap::new();
        let mut enums = BTreeMap::new();
        for file in &set.files {
            for msg in &file.messages {
                index_message(&file.package, msg, &mut messages, &mut enums);
            }
            for e in &file.enums {
                enums.insert(join_fqn(&file.package, &e.name), e.clone());
            }
        }
        Self {
            messages,
            enums,
            extensibility,
            builder: DynamicTypeBuilder::new(),
            cache: BTreeMap::new(),
        }
    }

    /// The finished model, holding every type built so far.
    #[must_use]
    pub fn finish(self) -> Arc<Ast> {
        self.builder.finish()
    }

    /// The finished model and a view on one of its types.
    ///
    /// # Errors
    /// [`MapError::Build`] where the handle is not this registry's.
    pub fn into_type(self, type_: Id<TypeNode>) -> Result<DynamicType, MapError> {
        self.builder.finish_as(type_).map_err(|e| build_error(&e))
    }

    /// Build a message and hand back the finished model with a view on it.
    ///
    /// The one-type convenience over [`Registry::build`] and
    /// [`Registry::into_type`]; a caller that needs several types calls
    /// those two.
    ///
    /// # Errors
    /// As [`Registry::build`].
    pub fn build_type(mut self, fqn: &str) -> Result<DynamicType, MapError> {
        let id = self.build(fqn)?;
        self.into_type(id)
    }

    /// Build the `DynamicType` for the message with the given fully-qualified
    /// name (no leading dot).
    ///
    /// # Errors
    /// [`MapError`] if the name is unknown, the message graph is recursive, a
    /// field type is unsupported, or the builder rejects a member.
    pub fn build(&mut self, fqn: &str) -> Result<Id<TypeNode>, MapError> {
        let mut building = BTreeSet::new();
        self.build_in(fqn, &mut building)
    }

    /// zerodds-lint: recursion-depth 64 (message nesting; bounded by .proto graph)
    fn build_in(
        &mut self,
        fqn: &str,
        building: &mut BTreeSet<String>,
    ) -> Result<Id<TypeNode>, MapError> {
        if let Some(id) = self.cache.get(fqn) {
            return Ok(*id);
        }
        if !building.insert(fqn.to_string()) {
            return Err(MapError::Cyclic(fqn.to_string()));
        }
        let msg = self
            .messages
            .get(fqn)
            .ok_or_else(|| MapError::UnknownType(fqn.to_string()))?
            .clone();

        let structure = self
            .builder
            .structure(&msg.name, self.extensibility)
            .map_err(|e| build_error(&e))?;

        let mut emitted_oneofs: BTreeSet<usize> = BTreeSet::new();
        for field in &msg.fields {
            // A proto `oneof` becomes a single XTypes union member, emitted at
            // the position of the group's first field; later members of the same
            // group are folded into that union and skipped here.
            if let Some(oi) = field.oneof_index {
                let oi_key = usize::try_from(oi).unwrap_or(usize::MAX);
                if !emitted_oneofs.insert(oi_key) {
                    continue;
                }
                let union_ty = self.build_oneof(&msg, oi, building)?;
                let name = msg
                    .oneofs
                    .get(oi_key)
                    .cloned()
                    .unwrap_or_else(|| format!("oneof_{oi_key}"));
                let member_id = msg
                    .fields
                    .iter()
                    .filter(|f| f.oneof_index == Some(oi))
                    .filter_map(|f| u32::try_from(f.number).ok())
                    .min()
                    .ok_or_else(|| MapError::UnsupportedField {
                        field: name.clone(),
                        reason: "oneof group has no representable field number",
                    })?;
                self.builder
                    .add_member(
                        structure,
                        &NewMember::new(&name, union_ty).with_id(member_id),
                    )
                    .map_err(|e| build_error(&e))?;
                continue;
            }

            let id = u32::try_from(field.number).map_err(|_| MapError::UnsupportedField {
                field: field.name.clone(),
                reason: "field number out of range",
            })?;

            let singular = field.label != Label::Repeated;
            let member_type = match (field.ty, singular) {
                (Some(ProtoFieldType::Message | ProtoFieldType::Group), true) => {
                    let target = field.type_name.trim_start_matches('.');
                    self.build_in(target, building)?
                }
                (Some(ProtoFieldType::Enum), true) => {
                    let target = field.type_name.trim_start_matches('.');
                    self.build_enum(target)?
                }
                (Some(ProtoFieldType::Message | ProtoFieldType::Group), false) => {
                    // A `repeated` message is either a real `sequence<struct>` or
                    // the synthetic `*Entry` of a proto `map<K,V>`.
                    let target = field.type_name.trim_start_matches('.');
                    let entry = self.messages.get(target).filter(|m| m.map_entry).cloned();
                    if let Some(entry) = entry {
                        self.build_map(&entry, building)?
                    } else {
                        let elem = self.build_in(target, building)?;
                        self.builder
                            .sequence(elem, None)
                            .map_err(|e| build_error(&e))?
                    }
                }
                (Some(ProtoFieldType::Enum), false) => {
                    let target = field.type_name.trim_start_matches('.');
                    let elem = self.build_enum(target)?;
                    self.builder
                        .sequence(elem, None)
                        .map_err(|e| build_error(&e))?
                }
                _ => field_type(&mut self.builder, field)?,
            };
            self.builder
                .add_member(
                    structure,
                    &NewMember::new(&field.name, member_type).with_id(id),
                )
                .map_err(|e| build_error(&e))?;
        }

        building.remove(fqn);
        self.cache.insert(fqn.to_string(), structure);
        Ok(structure)
    }

    /// Build the enum type for the given fully-qualified name.
    ///
    /// Each protobuf enumerator becomes an XTypes enumerator carrying an
    /// explicit `@value(n)` — a proto enum states its numbers and they are
    /// not always consecutive, so a positional enumerator would renumber
    /// them. The value-0 enumerator is `@default_literal`, which proto3
    /// requires to be present and treats as the zero value.
    ///
    /// The `@bit_bound` is 32: `descriptor.proto` numbers enumerators with
    /// an `int32`.
    fn build_enum(&mut self, fqn: &str) -> Result<Id<TypeNode>, MapError> {
        if let Some(id) = self.cache.get(fqn) {
            return Ok(*id);
        }
        let en = self
            .enums
            .get(fqn)
            .ok_or_else(|| MapError::UnknownType(fqn.to_string()))?
            .clone();

        let enumeration = self
            .builder
            .enumeration(&en.name, Some(32))
            .map_err(|e| build_error(&e))?;

        for (lit_name, value) in &en.values {
            self.builder
                .add_enumerator(enumeration, lit_name, Some(*value), *value == 0)
                .map_err(|e| build_error(&e))?;
        }

        self.cache.insert(fqn.to_string(), enumeration);
        Ok(enumeration)
    }

    /// Build an XTypes `map<K,V>` from a synthetic protobuf `*Entry` message
    /// (key = field 1, value = field 2). Both are resolved through
    /// [`Registry::resolve_element_type`], so a message- or enum-valued map
    /// keeps its full element type.
    ///
    /// zerodds-lint: recursion-depth 64 (via `resolve_element_type` -> message graph)
    fn build_map(
        &mut self,
        entry: &Message,
        building: &mut BTreeSet<String>,
    ) -> Result<Id<TypeNode>, MapError> {
        let key_field = entry.fields.iter().find(|f| f.number == 1).ok_or_else(|| {
            MapError::UnsupportedField {
                field: entry.name.clone(),
                reason: "map entry without a key field (number 1)",
            }
        })?;
        let val_field = entry.fields.iter().find(|f| f.number == 2).ok_or_else(|| {
            MapError::UnsupportedField {
                field: entry.name.clone(),
                reason: "map entry without a value field (number 2)",
            }
        })?;
        let key_ty = self.resolve_element_type(key_field, building)?;
        let val_ty = self.resolve_element_type(val_field, building)?;
        self.builder
            .map(key_ty, val_ty, None)
            .map_err(|e| build_error(&e))
    }

    /// Resolve a field's **element** type (its `repeated`/`map` wrapping
    /// ignored) to a fully-resolved `DynamicType`: message/group -> nested
    /// struct, enum -> XTypes enum, everything else -> the scalar/`string`/
    /// `bytes` type from [`crate::mapper::element_descriptor`].
    ///
    /// zerodds-lint: recursion-depth 64 (via `build_in` -> message graph)
    fn resolve_element_type(
        &mut self,
        field: &Field,
        building: &mut BTreeSet<String>,
    ) -> Result<Id<TypeNode>, MapError> {
        match field.ty {
            Some(ProtoFieldType::Message | ProtoFieldType::Group) => {
                self.build_in(field.type_name.trim_start_matches('.'), building)
            }
            Some(ProtoFieldType::Enum) => self.build_enum(field.type_name.trim_start_matches('.')),
            _ => element_type(&mut self.builder, field),
        }
    }

    /// Build an XTypes union from a proto `oneof` group: an `int32`
    /// discriminator and one case per member field, whose discriminator label
    /// and member id are the proto field number. A proto `oneof` has no field
    /// number of its own, so the union is attached to the struct by the caller.
    ///
    /// zerodds-lint: recursion-depth 64 (via `resolve_element_type` -> message graph)
    fn build_oneof(
        &mut self,
        msg: &Message,
        oneof_index: i32,
        building: &mut BTreeSet<String>,
    ) -> Result<Id<TypeNode>, MapError> {
        let name = msg
            .oneofs
            .get(usize::try_from(oneof_index).unwrap_or(usize::MAX))
            .cloned()
            .unwrap_or_else(|| format!("oneof_{oneof_index}"));
        let disc = self
            .builder
            .primitive(TypeKind::Int32)
            .map_err(|e| build_error(&e))?;
        // The union takes the message's extensibility, as the message
        // does: a `oneof` inside a `@mutable` message whose branches were
        // `@final` would stop evolving where the message it sits in does
        // not.
        let union = self
            .builder
            .union(&format!("{}.{name}", msg.name), disc, self.extensibility)
            .map_err(|e| build_error(&e))?;

        for field in msg
            .fields
            .iter()
            .filter(|f| f.oneof_index == Some(oneof_index))
            .cloned()
            .collect::<alloc::vec::Vec<_>>()
        {
            let id = u32::try_from(field.number).map_err(|_| MapError::UnsupportedField {
                field: field.name.clone(),
                reason: "oneof field number out of range",
            })?;
            let case_ty = self.resolve_element_type(&field, building)?;
            self.builder
                .add_case(
                    union,
                    &[i64::from(field.number)],
                    false,
                    &NewMember::new(&field.name, case_ty).with_id(id),
                )
                .map_err(|e| build_error(&e))?;
        }

        Ok(union)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::descriptor::{Field, File};
    use alloc::vec;

    fn scalar_field(name: &str, number: i32, ty: ProtoFieldType) -> Field {
        Field {
            name: name.to_string(),
            number,
            label: Label::Optional,
            ty: Some(ty),
            type_name: String::new(),
            oneof_index: None,
        }
    }

    fn msg_field(name: &str, number: i32, type_name: &str) -> Field {
        Field {
            name: name.to_string(),
            number,
            label: Label::Optional,
            ty: Some(ProtoFieldType::Message),
            type_name: type_name.to_string(),
            oneof_index: None,
        }
    }

    #[test]
    fn resolves_a_nested_message_member() {
        let inner = Message {
            name: "Inner".to_string(),
            fields: vec![scalar_field("x", 1, ProtoFieldType::Int32)],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let outer = Message {
            name: "Outer".to_string(),
            fields: vec![
                scalar_field("id", 1, ProtoFieldType::Int64),
                msg_field("inner", 2, ".pkg.Inner"),
            ],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![inner, outer],
                enums: vec![],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let outer_dt = reg.build_type("pkg.Outer").unwrap();
        assert_eq!(outer_dt.member_count(), 2);

        // the nested member is itself a struct carrying Inner's members
        let inner_member = outer_dt.member_by_name("inner").unwrap();
        assert_eq!(inner_member.id(), 2);
        let inner_ty = inner_member.dynamic_type().unwrap();
        assert_eq!(inner_ty.member_by_name("x").unwrap().id(), 1);
    }

    #[test]
    fn unknown_message_reference_is_reported() {
        let outer = Message {
            name: "Outer".to_string(),
            fields: vec![msg_field("inner", 1, ".pkg.Missing")],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![outer],
                enums: vec![],
            }],
        };
        let mut reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        assert!(matches!(
            reg.build("pkg.Outer"),
            Err(MapError::UnknownType(_))
        ));
    }

    fn repeated_msg_field(name: &str, number: i32, type_name: &str) -> Field {
        Field {
            name: name.to_string(),
            number,
            label: Label::Repeated,
            ty: Some(ProtoFieldType::Message),
            type_name: type_name.to_string(),
            oneof_index: None,
        }
    }

    /// A synthetic `protoc` map-entry message: `key`(1) + `value`(2),
    /// `map_entry = true`.
    fn map_entry(name: &str, key: Field, value: Field) -> Message {
        Message {
            name: name.to_string(),
            fields: vec![key, value],
            nested: vec![],
            enums: vec![],
            map_entry: true,
            oneofs: vec![],
        }
    }

    #[test]
    fn maps_a_scalar_valued_map_field() {
        let entry = map_entry(
            "PriceEntry",
            scalar_field("key", 1, ProtoFieldType::String),
            scalar_field("value", 2, ProtoFieldType::Int32),
        );
        let order = Message {
            name: "Order".to_string(),
            fields: vec![repeated_msg_field("prices", 3, ".pkg.Order.PriceEntry")],
            nested: vec![entry],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![order],
                enums: vec![],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let dt = reg.build_type("pkg.Order").unwrap();
        let prices = dt.member_by_name("prices").unwrap();
        assert_eq!(prices.id(), 3);
        let map_ty = prices.dynamic_type().unwrap();
        assert_eq!(map_ty.kind(), TypeKind::Map);
        assert_eq!(map_ty.key_element_type().unwrap().kind(), TypeKind::String8);
        assert_eq!(map_ty.element_type().unwrap().kind(), TypeKind::Int32);
    }

    #[test]
    fn maps_a_message_valued_map_field() {
        let point = Message {
            name: "Point".to_string(),
            fields: vec![
                scalar_field("x", 1, ProtoFieldType::Int32),
                scalar_field("y", 2, ProtoFieldType::Int32),
            ],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let entry = map_entry(
            "VertexEntry",
            scalar_field("key", 1, ProtoFieldType::String),
            msg_field("value", 2, ".pkg.Point"),
        );
        let mesh = Message {
            name: "Mesh".to_string(),
            fields: vec![repeated_msg_field("vertices", 1, ".pkg.Mesh.VertexEntry")],
            nested: vec![entry],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![point, mesh],
                enums: vec![],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let dt = reg.build_type("pkg.Mesh").unwrap();
        let map_ty = dt
            .member_by_name("vertices")
            .unwrap()
            .dynamic_type()
            .unwrap();
        assert_eq!(map_ty.kind(), TypeKind::Map);
        // value keeps the full nested struct (its members survive)
        let val = map_ty.element_type().unwrap();
        assert_eq!(val.kind(), TypeKind::Structure);
        assert_eq!(val.member_by_name("y").unwrap().id(), 2);
    }

    #[test]
    fn maps_a_repeated_message_to_sequence_of_struct() {
        let item = Message {
            name: "Item".to_string(),
            fields: vec![scalar_field("sku", 1, ProtoFieldType::Int64)],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let cart = Message {
            name: "Cart".to_string(),
            fields: vec![repeated_msg_field("items", 1, ".pkg.Item")],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![item, cart],
                enums: vec![],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let dt = reg.build_type("pkg.Cart").unwrap();
        let seq_ty = dt.member_by_name("items").unwrap().dynamic_type().unwrap();
        assert_eq!(seq_ty.kind(), TypeKind::Sequence);
        let elem = seq_ty.element_type().unwrap();
        assert_eq!(elem.kind(), TypeKind::Structure);
        assert_eq!(elem.member_by_name("sku").unwrap().id(), 1);
    }

    #[test]
    fn maps_a_repeated_enum_to_sequence_of_enum() {
        let color = Enum {
            name: "Color".to_string(),
            values: vec![("RED".to_string(), 0), ("GREEN".to_string(), 1)],
        };
        let palette = Message {
            name: "Palette".to_string(),
            fields: vec![Field {
                name: "colors".to_string(),
                number: 1,
                label: Label::Repeated,
                ty: Some(ProtoFieldType::Enum),
                type_name: ".pkg.Color".to_string(),
                oneof_index: None,
            }],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![palette],
                enums: vec![color],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let dt = reg.build_type("pkg.Palette").unwrap();
        let seq_ty = dt.member_by_name("colors").unwrap().dynamic_type().unwrap();
        assert_eq!(seq_ty.kind(), TypeKind::Sequence);
        assert_eq!(seq_ty.element_type().unwrap().kind(), TypeKind::Enumeration);
    }

    fn oneof_field(name: &str, number: i32, ty: ProtoFieldType, oneof: i32) -> Field {
        Field {
            name: name.to_string(),
            number,
            label: Label::Optional,
            ty: Some(ty),
            type_name: String::new(),
            oneof_index: Some(oneof),
        }
    }

    #[test]
    fn maps_a_oneof_to_a_union() {
        // message Value { int32 id = 1; oneof body { int32 i = 2; string s = 3; } }
        let value = Message {
            name: "Value".to_string(),
            fields: vec![
                scalar_field("id", 1, ProtoFieldType::Int32),
                oneof_field("i", 2, ProtoFieldType::Int32, 0),
                oneof_field("s", 3, ProtoFieldType::String, 0),
            ],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec!["body".to_string()],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![value],
                enums: vec![],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let dt = reg.build_type("pkg.Value").unwrap();
        // regular field + one union member for the whole oneof group
        assert_eq!(dt.member_count(), 2);
        assert_eq!(dt.member_by_name("id").unwrap().id(), 1);

        let union_member = dt.member_by_name("body").unwrap();
        // union member id = lowest field number in the group
        assert_eq!(union_member.id(), 2);
        let union_ty = union_member.dynamic_type().unwrap();
        assert_eq!(union_ty.kind(), TypeKind::Union);
        // one case per oneof field, keyed by field number
        assert_eq!(union_ty.member_by_name("i").unwrap().id(), 2);
        assert_eq!(union_ty.member_by_name("s").unwrap().id(), 3);
        assert_eq!(
            union_ty
                .member_by_name("s")
                .unwrap()
                .descriptor()
                .unwrap()
                .label,
            alloc::vec![3_i64]
        );
    }

    #[test]
    fn resolves_an_enum_member() {
        let color = Enum {
            name: "Color".to_string(),
            values: vec![
                ("RED".to_string(), 0),
                ("GREEN".to_string(), 1),
                ("BLUE".to_string(), 2),
            ],
        };
        let pixel = Message {
            name: "Pixel".to_string(),
            fields: vec![Field {
                name: "color".to_string(),
                number: 1,
                label: Label::Optional,
                ty: Some(ProtoFieldType::Enum),
                type_name: ".pkg.Color".to_string(),
                oneof_index: None,
            }],
            nested: vec![],
            enums: vec![],
            map_entry: false,
            oneofs: vec![],
        };
        let set = FileDescriptorSet {
            files: vec![File {
                package: "pkg".to_string(),
                messages: vec![pixel],
                enums: vec![color],
            }],
        };

        let reg = Registry::from_set(&set, ExtensibilityKind::Appendable);
        let dt = reg.build_type("pkg.Pixel").unwrap();
        let color_ty = dt.member_by_name("color").unwrap().dynamic_type().unwrap();
        assert_eq!(color_ty.kind(), TypeKind::Enumeration);
        // The enumerator value is the literal's value; an enumerator has
        // no member id (XTypes 1.3 §7.5.2.7.5).
        assert_eq!(
            color_ty.member_by_name("GREEN").unwrap().literal_value(),
            Some(1)
        );
        let blue = color_ty
            .members()
            .into_iter()
            .find(|member| member.literal_value() == Some(2))
            .unwrap();
        assert_eq!(blue.name(), "BLUE");
    }
}
