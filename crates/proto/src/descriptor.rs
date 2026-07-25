// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A lightweight `FileDescriptorSet` AST and its decoder — the subset of
//! `descriptor.proto` needed to build XTypes types: files, messages (incl.
//! nested), fields (name / number / label / type / type_name) and enums.
//!
//! Parsed with [`crate::reader`]; every decode returns `None` on a truncated
//! or malformed descriptor rather than panicking.

use alloc::string::String;
use alloc::vec::Vec;

use crate::ProtoFieldType;
use crate::reader::{Reader, WireType};

/// `FieldDescriptorProto.Label` — a field's cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// proto2 `optional` (or proto3 singular).
    Optional,
    /// `required` (proto2 only).
    Required,
    /// `repeated` -> maps to `sequence<T>`.
    Repeated,
}

impl Label {
    fn from_wire(v: i32) -> Self {
        match v {
            2 => Self::Required,
            3 => Self::Repeated,
            _ => Self::Optional,
        }
    }
}

/// One field of a message (`FieldDescriptorProto`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Field name.
    pub name: String,
    /// Field number -> XTypes member id.
    pub number: i32,
    /// Cardinality.
    pub label: Label,
    /// Field type; `None` if the descriptor carried an out-of-range type code.
    pub ty: Option<ProtoFieldType>,
    /// For message/enum/group fields: the fully-qualified type name (leading
    /// `.`), else empty.
    pub type_name: String,
    /// `FieldDescriptorProto.oneof_index` — the index into the message's
    /// `oneof_decl` list if this field is part of a `oneof`, else `None`.
    pub oneof_index: Option<i32>,
}

/// A message type (`DescriptorProto`), possibly with nested messages/enums.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    /// Message name (unqualified).
    pub name: String,
    /// Fields, in declaration order.
    pub fields: Vec<Field>,
    /// Nested message types.
    pub nested: Vec<Message>,
    /// Nested enum types.
    pub enums: Vec<Enum>,
    /// `MessageOptions.map_entry` — set on the synthetic `*Entry` message that
    /// `protoc` generates for a `map<K,V>` field (key = field 1, value = 2).
    pub map_entry: bool,
    /// `oneof_decl` names, in declaration order. A field's `oneof_index` is an
    /// index into this list.
    pub oneofs: Vec<String>,
}

/// An enum type (`EnumDescriptorProto`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Enum {
    /// Enum name.
    pub name: String,
    /// `(name, number)` pairs in declaration order.
    pub values: Vec<(String, i32)>,
}

/// A single `.proto` file (`FileDescriptorProto`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct File {
    /// The `package` declaration (may be empty).
    pub package: String,
    /// Top-level messages.
    pub messages: Vec<Message>,
    /// Top-level enums.
    pub enums: Vec<Enum>,
}

/// A `FileDescriptorSet` — the output of `protoc -o out.pb`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDescriptorSet {
    /// The described files.
    pub files: Vec<File>,
}

fn read_str(r: &mut Reader<'_>) -> Option<String> {
    core::str::from_utf8(r.read_len_delimited()?)
        .ok()
        .map(String::from)
}

/// zerodds-lint: recursion-depth 64 (message nesting; bounded by .proto depth)
fn parse_message(bytes: &[u8]) -> Option<Message> {
    let mut m = Message::default();
    let mut r = Reader::new(bytes);
    while !r.at_end() {
        let (field, wt) = r.read_tag()?;
        match (field, wt) {
            (1, WireType::Len) => m.name = read_str(&mut r)?, // name
            (2, WireType::Len) => m.fields.push(parse_field(r.read_len_delimited()?)?), // field
            (3, WireType::Len) => m.nested.push(parse_message(r.read_len_delimited()?)?), // nested_type
            (4, WireType::Len) => m.enums.push(parse_enum(r.read_len_delimited()?)?), // enum_type
            (7, WireType::Len) => m.map_entry = parse_message_options(r.read_len_delimited()?)?, // options
            (8, WireType::Len) => m.oneofs.push(parse_oneof(r.read_len_delimited()?)?), // oneof_decl
            (_, wt) => r.skip(wt)?,
        }
    }
    Some(m)
}

fn parse_field(bytes: &[u8]) -> Option<Field> {
    let mut name = String::new();
    let mut number = 0i32;
    let mut label = Label::Optional;
    let mut ty = None;
    let mut type_name = String::new();
    let mut oneof_index = None;
    let mut r = Reader::new(bytes);
    while !r.at_end() {
        let (field, wt) = r.read_tag()?;
        match (field, wt) {
            (1, WireType::Len) => name = read_str(&mut r)?, // name
            (3, WireType::Varint) => number = i32::try_from(r.read_varint()?).ok()?, // number
            (4, WireType::Varint) => {
                label = Label::from_wire(i32::try_from(r.read_varint()?).ok()?)
            } // label
            (5, WireType::Varint) => {
                ty = ProtoFieldType::from_wire(i32::try_from(r.read_varint()?).ok()?)
            } // type
            (6, WireType::Len) => type_name = read_str(&mut r)?, // type_name
            (9, WireType::Varint) => oneof_index = Some(i32::try_from(r.read_varint()?).ok()?), // oneof_index
            (_, wt) => r.skip(wt)?,
        }
    }
    Some(Field {
        name,
        number,
        label,
        ty,
        type_name,
        oneof_index,
    })
}

/// Parse an `OneofDescriptorProto`, returning its `name` (field 1).
fn parse_oneof(bytes: &[u8]) -> Option<String> {
    let mut name = String::new();
    let mut r = Reader::new(bytes);
    while !r.at_end() {
        let (field, wt) = r.read_tag()?;
        match (field, wt) {
            (1, WireType::Len) => name = read_str(&mut r)?, // name
            (_, wt) => r.skip(wt)?,
        }
    }
    Some(name)
}

/// Parse `MessageOptions`, returning `map_entry` (field 7, bool). Other options
/// are skipped.
fn parse_message_options(bytes: &[u8]) -> Option<bool> {
    let mut map_entry = false;
    let mut r = Reader::new(bytes);
    while !r.at_end() {
        let (field, wt) = r.read_tag()?;
        match (field, wt) {
            (7, WireType::Varint) => map_entry = r.read_varint()? != 0, // map_entry
            (_, wt) => r.skip(wt)?,
        }
    }
    Some(map_entry)
}

fn parse_enum(bytes: &[u8]) -> Option<Enum> {
    let mut e = Enum::default();
    let mut r = Reader::new(bytes);
    while !r.at_end() {
        let (field, wt) = r.read_tag()?;
        match (field, wt) {
            (1, WireType::Len) => e.name = read_str(&mut r)?, // name
            (2, WireType::Len) => {
                // EnumValueDescriptorProto { name(1), number(2) }
                let mut vr = Reader::new(r.read_len_delimited()?);
                let mut vname = String::new();
                let mut vnum = 0i32;
                while !vr.at_end() {
                    let (vf, vwt) = vr.read_tag()?;
                    match (vf, vwt) {
                        (1, WireType::Len) => vname = read_str(&mut vr)?,
                        (2, WireType::Varint) => vnum = i32::try_from(vr.read_varint()?).ok()?,
                        (_, vwt) => vr.skip(vwt)?,
                    }
                }
                e.values.push((vname, vnum));
            }
            (_, wt) => r.skip(wt)?,
        }
    }
    Some(e)
}

fn parse_file(bytes: &[u8]) -> Option<File> {
    let mut f = File::default();
    let mut r = Reader::new(bytes);
    while !r.at_end() {
        let (field, wt) = r.read_tag()?;
        match (field, wt) {
            (2, WireType::Len) => f.package = read_str(&mut r)?, // package
            (4, WireType::Len) => f.messages.push(parse_message(r.read_len_delimited()?)?), // message_type
            (5, WireType::Len) => f.enums.push(parse_enum(r.read_len_delimited()?)?), // enum_type
            (_, wt) => r.skip(wt)?,
        }
    }
    Some(f)
}

impl FileDescriptorSet {
    /// Decode a `FileDescriptorSet` from `protoc -o out.pb` bytes. `None` on a
    /// truncated or malformed descriptor.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut set = Self::default();
        let mut r = Reader::new(bytes);
        while !r.at_end() {
            let (field, wt) = r.read_tag()?;
            match (field, wt) {
                (1, WireType::Len) => set.files.push(parse_file(r.read_len_delimited()?)?), // file
                (_, wt) => r.skip(wt)?,
            }
        }
        Some(set)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use alloc::vec;

    #[test]
    fn parses_a_field_descriptor() {
        // FieldDescriptorProto { name="x"(1), number=1(3), type=INT32=5(5) }
        let bytes = [0x0a, 0x01, b'x', 0x18, 0x01, 0x28, 0x05];
        let f = parse_field(&bytes).unwrap();
        assert_eq!(f.name, "x");
        assert_eq!(f.number, 1);
        assert_eq!(f.ty, Some(ProtoFieldType::Int32));
        assert_eq!(f.label, Label::Optional);
    }

    #[test]
    fn parses_map_entry_option() {
        // DescriptorProto { name="E"(1), options(7) = { map_entry=true(7) } }
        // MessageOptions bytes: field 7 varint -> 0x38 0x01
        let bytes = [0x0a, 0x01, b'E', 0x3a, 0x02, 0x38, 0x01];
        let m = parse_message(&bytes).unwrap();
        assert_eq!(m.name, "E");
        assert!(m.map_entry);
    }

    #[test]
    fn parses_oneof_index_and_decl() {
        // field x: name="x"(1) number=1(3) type=INT32=5(5) oneof_index=0(9)
        let fx = [0x0a, 0x01, b'x', 0x18, 0x01, 0x28, 0x05, 0x48, 0x00];
        // OneofDescriptorProto { name="body"(1) }
        let od = [0x0a, 0x04, b'b', b'o', b'd', b'y'];
        // DescriptorProto { name="M"(1), field=fx(2), oneof_decl=od(8) }
        let mut msg = vec![0x0a, 0x01, b'M'];
        msg.push(0x12);
        msg.push(fx.len() as u8);
        msg.extend_from_slice(&fx);
        msg.push(0x42);
        msg.push(od.len() as u8);
        msg.extend_from_slice(&od);

        let m = parse_message(&msg).unwrap();
        assert_eq!(m.fields[0].oneof_index, Some(0));
        assert_eq!(m.oneofs, vec!["body".to_string()]);
    }

    #[test]
    fn parses_a_message_with_two_fields() {
        // field a: name="a"(1) number=1(3) type=STRING=9(5)
        let fa = [0x0a, 0x01, b'a', 0x18, 0x01, 0x28, 0x09];
        // field b: name="b"(1) number=2(3) type=BOOL=8(5) label=REPEATED=3(4)
        let fb = [0x0a, 0x01, b'b', 0x18, 0x02, 0x20, 0x03, 0x28, 0x08];
        // DescriptorProto { name="M"(1), field=fa(2), field=fb(2) }
        let mut msg = vec![0x0a, 0x01, b'M'];
        msg.push(0x12);
        msg.push(fa.len() as u8);
        msg.extend_from_slice(&fa);
        msg.push(0x12);
        msg.push(fb.len() as u8);
        msg.extend_from_slice(&fb);

        let m = parse_message(&msg).unwrap();
        assert_eq!(m.name, "M");
        assert_eq!(m.fields.len(), 2);
        assert_eq!(m.fields[0].name, "a");
        assert_eq!(m.fields[0].ty, Some(ProtoFieldType::String));
        assert_eq!(m.fields[1].name, "b");
        assert_eq!(m.fields[1].number, 2);
        assert_eq!(m.fields[1].label, Label::Repeated);
        assert_eq!(m.fields[1].ty, Some(ProtoFieldType::Bool));
    }
}
