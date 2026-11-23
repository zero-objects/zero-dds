// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DCPS side of a mapping: one DCPS data type per topic description,
//! and the conversion between a [`Row`] and a DCPS sample of that type
//! (DDS 1.2 §8.1.4.2: "a DCPS topic is a table, an instance a row, a
//! field a column"; §8.1.4.6: the DCPS description).
//!
//! The type of a topic is a structure named after the topic's type name
//! with one member per field, key fields first and marked `@key`. A
//! field name with '.' (the default names of §8.1.4.3 for the leaves of
//! a structure, `attribute.subfield`, and for the parts of a reference,
//! `relation.class`, `relation.oid`) designates a member of a nested
//! structure, as a field path does in a DCPS content filter: the field
//! `pos.x` is the member `x` of the structure member `pos`. A nested
//! structure is named `<type>_<path>` with '.' as '_'; it is `@key` when
//! one of its leaves is a key field.
//!
//! A type name keeps its `::` scopes; a character IDL does not admit in
//! an identifier becomes '_' (the default type name of a collection
//! topic is `Class.attribute`). Every segment of a field name has to be
//! an IDL identifier; a field name that is not is refused.
//!
//! The cells map onto IDL types as §8.1.4.2.3 lists them: `long`,
//! `short`, `char`, `octet`, `float`, `double`, `string` (unbounded),
//! `sequence<item>` (unbounded), and `DLRLOid` as the structure
//! `DDS::DLRLOid { long value0; long value1; long value2; }`: the three
//! values of the PSM's `DLRLOid { long value[3]; }` (§8.2.1.2.1) as three
//! members, because an oid field is a key field, and the key holder of
//! `zerodds-wire-codec` refuses a key member of array type (DDS-XTypes
//! admits it). §8.2.3.4 gives no DCPS type for an oid field. The
//! structures are `@final`.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_types::dynamic::{
    DynamicSample, DynamicType, DynamicTypeBuilderFactory, ExtensibilityKind, LENGTH_UNLIMITED,
    MEMBER_ID_INVALID, MemberDescriptor, TryConstructKind, TypeDescriptor, TypeKind,
};

use crate::mapping::{CellType, TopicDescription};
use crate::model::{BasicItem, MAX_STRUCT_NESTING, SequenceValue};
use crate::rows::{Cell, Row};
use crate::types::DlrlOid;

/// The scoped name of the oid structure (§8.2.1.2.1).
pub const DLRL_OID_TYPE: &str = "DDS::DLRLOid";

/// The most segments a field name has: a default name of §8.1.4.3 is an
/// attribute and one member per nested structure of its type (at most
/// [`MAX_STRUCT_NESTING`]), and the field of a reference puts the
/// reference's name in front of a key field of its target, one segment
/// more. [`topic_type`], [`row_to_sample`] and [`sample_to_row`] refuse a
/// longer name; the functions that follow a field name go down one call
/// per segment.
pub const MAX_FIELD_SEGMENTS: usize = MAX_STRUCT_NESTING + 2;

/// The error for a field name of more than [`MAX_FIELD_SEGMENTS`]
/// segments.
fn too_many_segments(name: &str) -> String {
    format!("field {name}: more than {MAX_FIELD_SEGMENTS} segments")
}

enum Node {
    Leaf { cell: CellType, key: bool },
    Group(Vec<(String, Node)>),
}

impl Node {
    fn has_key(&self) -> bool {
        match self {
            Self::Leaf { key, .. } => *key,
            Self::Group(m) => m.iter().any(|(_, n)| n.has_key()),
        }
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The IDL type name of a mapping's type name: scopes kept, every other
/// character that an identifier cannot hold as '_'.
#[must_use]
pub fn dcps_type_name(type_name: &str) -> String {
    type_name
        .split("::")
        .map(|seg| {
            let mut out: String = seg
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
                out.insert(0, '_');
            }
            out
        })
        .collect::<Vec<_>>()
        .join("::")
}

fn struct_descriptor(name: &str) -> TypeDescriptor {
    TypeDescriptor {
        kind: TypeKind::Structure,
        name: name.into(),
        base_type: None,
        discriminator_type: None,
        bound: Vec::new(),
        element_type: None,
        key_element_type: None,
        extensibility_kind: Some(ExtensibilityKind::Final),
        is_nested: false,
        bit_bound: None,
    }
}

fn member(name: &str, type_: DynamicType, key: bool) -> MemberDescriptor {
    MemberDescriptor {
        name: name.into(),
        id: MEMBER_ID_INVALID,
        type_,
        index: u32::MAX,
        label: Vec::new(),
        is_default_label: false,
        try_construct_kind: TryConstructKind::Discard,
        is_key: key,
        is_optional: false,
        // DDS-XTypes 1.3 §7.2.2.4.4.4.8: a key member is must-understand.
        is_must_understand: key,
        is_shared: false,
        has_default_value: false,
        default_value: String::new(),
    }
}

fn err(e: impl core::fmt::Display) -> String {
    format!("{e}")
}

fn primitive(kind: TypeKind) -> Result<DynamicType, String> {
    DynamicTypeBuilderFactory::get_instance()
        .get_primitive_type(kind)
        .map_err(err)
}

fn item_kind(item: BasicItem) -> TypeKind {
    match item {
        BasicItem::Long => TypeKind::Int32,
        BasicItem::Short => TypeKind::Int16,
        BasicItem::Char => TypeKind::Char8,
        BasicItem::Octet => TypeKind::Byte,
        BasicItem::Real => TypeKind::Float32,
        BasicItem::Double => TypeKind::Float64,
        BasicItem::String => TypeKind::String8,
    }
}

fn string_type() -> Result<DynamicType, String> {
    DynamicTypeBuilderFactory::get_instance()
        .create_string_type(LENGTH_UNLIMITED)
        .map_err(err)?
        .build()
        .map_err(err)
}

fn item_type(item: BasicItem) -> Result<DynamicType, String> {
    match item {
        BasicItem::String => string_type(),
        other => primitive(item_kind(other)),
    }
}

/// The members of the oid structure, the three values in order.
pub const DLRL_OID_MEMBERS: [&str; 3] = ["value0", "value1", "value2"];

/// The type `DDS::DLRLOid { long value0; long value1; long value2; }`.
///
/// # Errors
/// The type builder refuses the type (does not happen for this type).
pub fn oid_type() -> Result<DynamicType, String> {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let mut b = factory
        .create_type(&struct_descriptor(DLRL_OID_TYPE))
        .map_err(err)?;
    for m in DLRL_OID_MEMBERS {
        b.add_member(&member(m, primitive(TypeKind::Int32)?, false))
            .map_err(err)?;
    }
    b.build().map_err(err)
}

fn cell_type(cell: CellType) -> Result<DynamicType, String> {
    match cell {
        CellType::Long => primitive(TypeKind::Int32),
        CellType::Short => primitive(TypeKind::Int16),
        CellType::Char => primitive(TypeKind::Char8),
        CellType::Octet => primitive(TypeKind::Byte),
        CellType::Real => primitive(TypeKind::Float32),
        CellType::Double => primitive(TypeKind::Float64),
        CellType::String => string_type(),
        CellType::Sequence(item) => DynamicTypeBuilderFactory::get_instance()
            .create_sequence_type(&item_type(item)?, LENGTH_UNLIMITED)
            .map_err(err)?
            .build()
            .map_err(err),
        CellType::Oid => oid_type(),
    }
}

/// zerodds-lint: recursion-depth 65 — one call per nested group, that is
/// per segment of a field name before its last; [`topic_type`] refuses a
/// name of more than [`MAX_FIELD_SEGMENTS`] (66) segments before it
/// builds the groups.
fn build_group(name: &str, members: &[(String, Node)]) -> Result<DynamicType, String> {
    let factory = DynamicTypeBuilderFactory::get_instance();
    let mut b = factory.create_type(&struct_descriptor(name)).map_err(err)?;
    for (m, node) in members {
        let (ty, key) = match node {
            Node::Leaf { cell, key } => (cell_type(*cell)?, *key),
            Node::Group(inner) => (build_group(&format!("{name}_{m}"), inner)?, node.has_key()),
        };
        b.add_member(&member(m, ty, key)).map_err(err)?;
    }
    b.build().map_err(err)
}

/// The DCPS data type of a topic of the mapping.
///
/// # Errors
/// A field name with a segment that is not an IDL identifier or with
/// more than [`MAX_FIELD_SEGMENTS`] segments, a field that is both a leaf
/// and a structure, or a type the builder refuses.
pub fn topic_type(desc: &TopicDescription) -> Result<DynamicType, String> {
    let mut root: Vec<(String, Node)> = Vec::new();
    for f in &desc.fields {
        let segments: Vec<&str> = f.name.split('.').collect();
        if segments.len() > MAX_FIELD_SEGMENTS {
            return Err(too_many_segments(&f.name));
        }
        if let Some(bad) = segments.iter().find(|s| !is_identifier(s)) {
            return Err(format!(
                "field {} of topic {}: `{bad}` is not an IDL identifier",
                f.name, desc.name
            ));
        }
        let mut level = &mut root;
        for (i, seg) in segments.iter().enumerate() {
            let last = i + 1 == segments.len();
            let pos = level.iter().position(|(n, _)| n == seg);
            match (pos, last) {
                (None, true) => {
                    level.push((
                        (*seg).into(),
                        Node::Leaf {
                            cell: f.cell,
                            key: f.key,
                        },
                    ));
                }
                (None, false) => {
                    level.push(((*seg).into(), Node::Group(Vec::new())));
                    let at = level.len() - 1;
                    let Node::Group(next) = &mut level[at].1 else {
                        return Err(format!("field {}: inner error", f.name));
                    };
                    level = next;
                }
                (Some(p), false) => {
                    let Node::Group(next) = &mut level[p].1 else {
                        return Err(format!(
                            "field {} of topic {}: `{seg}` is a leaf and a structure",
                            f.name, desc.name
                        ));
                    };
                    level = next;
                }
                (Some(_), true) => {
                    return Err(format!(
                        "field {} of topic {} is a leaf and a structure, or given twice",
                        f.name, desc.name
                    ));
                }
            }
        }
    }
    build_group(&dcps_type_name(&desc.type_name), &root)
}

fn set_leaf(s: &mut DynamicSample, name: &str, cell: &Cell) -> Result<(), String> {
    let id = s.get_member_id_by_name(name).map_err(err)?;
    match cell {
        Cell::Long(v) => s.set_int32_value(id, *v),
        Cell::Short(v) => s.set_int16_value(id, *v),
        Cell::Char(v) => s.set_char8_value(id, *v),
        Cell::Octet(v) => s.set_byte_value(id, *v),
        Cell::Real(v) => s.set_float32_value(id, *v),
        Cell::Double(v) => s.set_float64_value(id, *v),
        Cell::String(v) => s.set_string_value(id, v),
        Cell::Sequence(seq) => match seq {
            SequenceValue::Long(v) => s.set_int32_values(id, v),
            SequenceValue::Short(v) => s.set_int16_values(id, v),
            SequenceValue::Char(v) => s.set_char8_values(id, v),
            SequenceValue::Octet(v) => s.set_byte_values(id, v),
            SequenceValue::Real(v) => s.set_float32_values(id, v),
            SequenceValue::Double(v) => s.set_float64_values(id, v),
            SequenceValue::String(v) => s.set_string_values(id, v),
        },
        Cell::Oid(oid) => {
            let mut child = s.loan_value(id).map_err(err)?;
            let mut r = Ok(());
            for (m, v) in DLRL_OID_MEMBERS.iter().zip(oid.value) {
                r = child
                    .get_member_id_by_name(m)
                    .and_then(|vid| child.set_int32_value(vid, v));
                if r.is_err() {
                    break;
                }
            }
            s.return_loaned_value(child).map_err(err)?;
            r
        }
    }
    .map_err(err)
}

/// zerodds-lint: recursion-depth 65 — one call per segment of the field
/// name after the first; a name of more than [`MAX_FIELD_SEGMENTS`] (66)
/// segments is refused on entry.
fn set_path(s: &mut DynamicSample, path: &[&str], cell: &Cell) -> Result<(), String> {
    if path.len() > MAX_FIELD_SEGMENTS {
        return Err(too_many_segments(&path.join(".")));
    }
    match path {
        [leaf] => set_leaf(s, leaf, cell),
        [head, rest @ ..] => {
            let id = s.get_member_id_by_name(head).map_err(err)?;
            let mut child = s.loan_value(id).map_err(err)?;
            let r = set_path(&mut child, rest, cell);
            s.return_loaned_value(child).map_err(err)?;
            r
        }
        [] => Err("empty field name".into()),
    }
}

/// The sample of a row; fields of the topic the row does not carry keep
/// their default values.
///
/// # Errors
/// A cell for a field the topic does not have, or of another type; a
/// field name of more than [`MAX_FIELD_SEGMENTS`] segments.
pub fn row_to_sample(
    desc: &TopicDescription,
    type_: &DynamicType,
    row: &Row,
) -> Result<DynamicSample, String> {
    let mut s = DynamicSample::new(type_).map_err(err)?;
    for (name, cell) in &row.cells {
        let field = desc
            .field(name)
            .ok_or_else(|| format!("topic {} has no field {name}", desc.name))?;
        if !cell.fits(field.cell) {
            return Err(format!(
                "field {name} of topic {}: the cell is not of the field's type",
                desc.name
            ));
        }
        let path: Vec<&str> = name.split('.').collect();
        set_path(&mut s, &path, cell)?;
    }
    Ok(s)
}

fn get_leaf(s: &mut DynamicSample, name: &str, cell: CellType) -> Result<Cell, String> {
    let id = s.get_member_id_by_name(name).map_err(err)?;
    Ok(match cell {
        CellType::Long => Cell::Long(s.get_int32_value(id).map_err(err)?),
        CellType::Short => Cell::Short(s.get_int16_value(id).map_err(err)?),
        CellType::Char => Cell::Char(s.get_char8_value(id).map_err(err)?),
        CellType::Octet => Cell::Octet(s.get_byte_value(id).map_err(err)?),
        CellType::Real => Cell::Real(s.get_float32_value(id).map_err(err)?),
        CellType::Double => Cell::Double(s.get_float64_value(id).map_err(err)?),
        CellType::String => Cell::String(s.get_string_value(id).map_err(err)?),
        CellType::Sequence(item) => Cell::Sequence(match item {
            BasicItem::Long => SequenceValue::Long(s.get_int32_values(id).map_err(err)?),
            BasicItem::Short => SequenceValue::Short(s.get_int16_values(id).map_err(err)?),
            BasicItem::Char => SequenceValue::Char(s.get_char8_values(id).map_err(err)?),
            BasicItem::Octet => SequenceValue::Octet(s.get_byte_values(id).map_err(err)?),
            BasicItem::Real => SequenceValue::Real(s.get_float32_values(id).map_err(err)?),
            BasicItem::Double => SequenceValue::Double(s.get_float64_values(id).map_err(err)?),
            BasicItem::String => SequenceValue::String(s.get_string_values(id).map_err(err)?),
        }),
        CellType::Oid => {
            let mut child = s.get_complex_value(id).map_err(err)?;
            let mut v = [0i32; 3];
            for (slot, m) in v.iter_mut().zip(DLRL_OID_MEMBERS) {
                let vid = child.get_member_id_by_name(m).map_err(err)?;
                *slot = child.get_int32_value(vid).map_err(err)?;
            }
            Cell::Oid(DlrlOid { value: v })
        }
    })
}

/// zerodds-lint: recursion-depth 65 — one call per segment of the field
/// name after the first; a name of more than [`MAX_FIELD_SEGMENTS`] (66)
/// segments is refused on entry.
fn get_path(s: &mut DynamicSample, path: &[&str], cell: CellType) -> Result<Cell, String> {
    if path.len() > MAX_FIELD_SEGMENTS {
        return Err(too_many_segments(&path.join(".")));
    }
    match path {
        [leaf] => get_leaf(s, leaf, cell),
        [head, rest @ ..] => {
            let id = s.get_member_id_by_name(head).map_err(err)?;
            let mut child = s.get_complex_value(id).map_err(err)?;
            get_path(&mut child, rest, cell)
        }
        [] => Err("empty field name".into()),
    }
}

/// The row of a sample: every field of the topic, or only the key fields
/// (`key_only`, for the key-only sample that reports a disposed
/// instance).
///
/// # Errors
/// A field the sample does not hold with the field's type; a field name
/// of more than [`MAX_FIELD_SEGMENTS`] segments.
pub fn sample_to_row(
    desc: &TopicDescription,
    sample: &DynamicSample,
    key_only: bool,
) -> Result<Row, String> {
    let mut s = sample.clone();
    let mut cells = Vec::with_capacity(desc.fields.len());
    for f in desc.fields.iter().filter(|f| f.key || !key_only) {
        let path: Vec<&str> = f.name.split('.').collect();
        cells.push((f.name.clone(), get_path(&mut s, &path, f.cell)?));
    }
    Ok(Row {
        topic: desc.name.clone(),
        cells,
    })
}

/// A text that identifies the key value of a row of a topic (equal key
/// cells, equal text).
#[must_use]
pub fn row_key(desc: &TopicDescription, row: &Row) -> String {
    format!("{:?}", row.key(desc))
}

/// The IDL text of the DCPS description of a set of topics (§8.1.4.6:
/// "on demand, the corresponding DCPS description"): one structure per
/// topic type, as [`topic_type`] builds it.
///
/// # Errors
/// As [`topic_type`].
pub fn dcps_description(topics: &[TopicDescription]) -> Result<String, String> {
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut out = String::new();
    let mut oid_needed = false;
    let mut bodies = String::new();
    for t in topics {
        topic_type(t)?;
        let name = dcps_type_name(&t.type_name);
        if seen.insert(name.clone(), ()).is_some() {
            continue;
        }
        oid_needed |= t.fields.iter().any(|f| f.cell == CellType::Oid);
        bodies.push_str(&format!("// topic {}\n", t.name));
        idl_struct(&name, &t.fields_tree(), &mut bodies);
    }
    if oid_needed {
        out.push_str(
            "module DDS { @final struct DLRLOid { long value0; long value1; long value2; }; };\n",
        );
    }
    out.push_str(&bodies);
    Ok(out)
}

/// One member of the IDL text: a leaf or a nested structure.
pub(crate) enum IdlMember {
    Leaf(String, CellType, bool),
    Group(String, Vec<IdlMember>, bool),
}

impl TopicDescription {
    pub(crate) fn fields_tree(&self) -> Vec<IdlMember> {
        let mut root: Vec<IdlMember> = Vec::new();
        for f in &self.fields {
            let segments: Vec<&str> = f.name.split('.').collect();
            insert_member(&mut root, &segments, f.cell, f.key);
        }
        root
    }
}

/// zerodds-lint: recursion-depth 65 — one call per segment of the field
/// name after the first. [`TopicDescription::fields_tree`] runs only in
/// [`dcps_description`], after [`topic_type`] has refused every name of
/// more than [`MAX_FIELD_SEGMENTS`] (66) segments.
fn insert_member(level: &mut Vec<IdlMember>, path: &[&str], cell: CellType, key: bool) {
    match path {
        [leaf] => level.push(IdlMember::Leaf((*leaf).into(), cell, key)),
        [head, rest @ ..] => {
            let pos = level
                .iter()
                .position(|m| matches!(m, IdlMember::Group(n, _, _) if n == head));
            let at = if let Some(p) = pos {
                p
            } else {
                level.push(IdlMember::Group((*head).into(), Vec::new(), false));
                level.len() - 1
            };
            if let IdlMember::Group(_, inner, k) = &mut level[at] {
                *k |= key;
                insert_member(inner, rest, cell, key);
            }
        }
        [] => {}
    }
}

fn idl_cell(cell: CellType) -> String {
    let item = |i: BasicItem| match i {
        BasicItem::Long => "long",
        BasicItem::Short => "short",
        BasicItem::Char => "char",
        BasicItem::Octet => "octet",
        BasicItem::Real => "float",
        BasicItem::Double => "double",
        BasicItem::String => "string",
    };
    match cell {
        CellType::Long => "long".into(),
        CellType::Short => "short".into(),
        CellType::Char => "char".into(),
        CellType::Octet => "octet".into(),
        CellType::Real => "float".into(),
        CellType::Double => "double".into(),
        CellType::String => "string".into(),
        CellType::Sequence(i) => format!("sequence<{}>", item(i)),
        CellType::Oid => "DDS::DLRLOid".into(),
    }
}

/// zerodds-lint: recursion-depth 65 — one call per nested group, that is
/// per segment of a field name before its last; [`dcps_description`]
/// calls it after [`topic_type`] has refused every name of more than
/// [`MAX_FIELD_SEGMENTS`] (66) segments.
fn idl_struct(name: &str, members: &[IdlMember], out: &mut String) {
    // Nested structures first, then the structure itself; a scoped name
    // becomes modules.
    for m in members {
        if let IdlMember::Group(n, inner, _) = m {
            idl_struct(&format!("{name}_{n}"), inner, out);
        }
    }
    let (scopes, local) = match name.rsplit_once("::") {
        Some((s, l)) => (s.split("::").collect::<Vec<_>>(), l),
        None => (Vec::new(), name),
    };
    for s in &scopes {
        out.push_str(&format!("module {s} {{ "));
    }
    out.push_str(&format!("@final struct {local} {{"));
    let leaf_name = |n: &str| match name.rsplit_once("::") {
        Some((_, l)) => format!("{l}_{n}"),
        None => format!("{name}_{n}"),
    };
    for m in members {
        match m {
            IdlMember::Leaf(n, c, k) => {
                let key = if *k { "@key " } else { "" };
                out.push_str(&format!(" {key}{} {n};", idl_cell(*c)));
            }
            IdlMember::Group(n, _, k) => {
                let key = if *k { "@key " } else { "" };
                out.push_str(&format!(" {key}{} {n};", leaf_name(n)));
            }
        }
    }
    out.push_str(" };");
    for _ in &scopes {
        out.push_str(" };");
    }
    out.push('\n');
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::mapping::{FieldDescription, KeyContent};

    fn topic(field: &str) -> TopicDescription {
        TopicDescription {
            name: "T".into(),
            type_name: "T".into(),
            key_content: KeyContent::SimpleOid,
            fields: alloc::vec![FieldDescription {
                name: field.into(),
                cell: CellType::Long,
                key: false,
            }],
            classes: Vec::new(),
        }
    }

    /// A field name of more than `MAX_FIELD_SEGMENTS` segments is refused
    /// before the functions that go down one call per segment follow it.
    #[test]
    fn a_field_name_of_more_than_max_field_segments_is_refused() {
        let deep = alloc::vec!["a"; MAX_FIELD_SEGMENTS + 1].join(".");
        let refused = topic_type(&topic(&deep)).unwrap_err();
        assert!(refused.contains("segments"), "{refused}");
        assert!(dcps_description(&[topic(&deep)]).is_err());
        assert!(topic_type(&topic("a.b")).is_ok());
    }
}
