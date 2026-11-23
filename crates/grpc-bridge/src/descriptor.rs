// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `google.protobuf.FileDescriptorProto` in proto2 wire format, for the
//! reflection service (`zerodds-grpc-bridge-1.0` §4.6).
//!
//! The descriptor messages of `google/protobuf/descriptor.proto` the bridge
//! needs: `FileDescriptorProto` (name 1, package 2, message_type 4,
//! service 6, syntax 12), `DescriptorProto` (name 1, field 2, oneof_decl 8),
//! `FieldDescriptorProto` (name 1, number 3, label 4, type 5, type_name 6,
//! oneof_index 9, json_name 10), `OneofDescriptorProto` (name 1),
//! `ServiceDescriptorProto` (name 1, method 2) and `MethodDescriptorProto`
//! (name 1, input_type 2, output_type 3, client_streaming 5,
//! server_streaming 6). The fields are proto2 `optional`: a value is
//! written when it is set, a default value included (`number`, `label`).

use alloc::string::String;
use alloc::vec::Vec;

use crate::messages::{put_bytes, put_varint};

/// `FieldDescriptorProto.Type` (descriptor.proto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// `TYPE_UINT64` (4).
    Uint64 = 4,
    /// `TYPE_INT32` (5).
    Int32 = 5,
    /// `TYPE_BOOL` (8).
    Bool = 8,
    /// `TYPE_STRING` (9).
    String = 9,
    /// `TYPE_MESSAGE` (11).
    Message = 11,
    /// `TYPE_BYTES` (12).
    Bytes = 12,
    /// `TYPE_UINT32` (13).
    Uint32 = 13,
}

/// One field of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDesc {
    /// The field name.
    pub name: &'static str,
    /// The field number.
    pub number: u32,
    /// `repeated` (`LABEL_REPEATED`), else `LABEL_OPTIONAL`.
    pub repeated: bool,
    /// The type.
    pub ty: FieldType,
    /// For [`FieldType::Message`]: the message name in the same package.
    pub message: Option<&'static str>,
    /// The index of the oneof the field belongs to.
    pub oneof: Option<u32>,
}

impl FieldDesc {
    /// A singular field.
    #[must_use]
    pub const fn one(name: &'static str, number: u32, ty: FieldType) -> Self {
        Self {
            name,
            number,
            repeated: false,
            ty,
            message: None,
            oneof: None,
        }
    }

    /// A singular message field of type `message`.
    #[must_use]
    pub const fn msg(name: &'static str, number: u32, message: &'static str) -> Self {
        Self {
            name,
            number,
            repeated: false,
            ty: FieldType::Message,
            message: Some(message),
            oneof: None,
        }
    }

    /// The same field, `repeated`.
    #[must_use]
    pub const fn repeated(mut self) -> Self {
        self.repeated = true;
        self
    }

    /// The same field, a member of the oneof `index`.
    #[must_use]
    pub const fn in_oneof(mut self, index: u32) -> Self {
        self.oneof = Some(index);
        self
    }
}

/// One message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDesc {
    /// The message name.
    pub name: &'static str,
    /// Its fields.
    pub fields: Vec<FieldDesc>,
    /// The names of its oneofs (the index is the position).
    pub oneofs: Vec<&'static str>,
}

/// One method of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDesc {
    /// The method name.
    pub name: &'static str,
    /// The request message (a name in the same package).
    pub input: &'static str,
    /// The response message (a name in the same package).
    pub output: &'static str,
    /// A stream of requests.
    pub client_streaming: bool,
    /// A stream of responses.
    pub server_streaming: bool,
}

/// One service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDesc {
    /// The service name.
    pub name: String,
    /// Its methods.
    pub methods: Vec<MethodDesc>,
}

/// One `.proto` file: a package with messages and services, no imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDesc {
    /// The file name (`zerodds/chat/v1/bridge.proto`).
    pub name: String,
    /// The package.
    pub package: String,
    /// Its messages.
    pub messages: Vec<MessageDesc>,
    /// Its services.
    pub services: Vec<ServiceDesc>,
}

/// The lowerCamelCase JSON name of a field (protoc's `ToJsonName`): an
/// underscore is dropped and the letter after it upper-cased.
#[must_use]
pub fn json_name(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut up = false;
    for c in field.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// A proto2 `optional` varint field: written even with the value 0.
fn put_varint_field(out: &mut Vec<u8>, number: u32, v: u64) {
    put_varint(out, u64::from(number) << 3);
    put_varint(out, v);
}

/// A proto2 `optional` string field: written even when empty.
fn put_str(out: &mut Vec<u8>, number: u32, v: &str) {
    put_varint(out, (u64::from(number) << 3) | 2);
    put_varint(out, v.len() as u64);
    out.extend_from_slice(v.as_bytes());
}

impl FileDesc {
    /// The fully-qualified name of `message` in this package (`.pkg.Name`).
    fn qualified(&self, message: &str) -> String {
        format!(".{}.{message}", self.package)
    }

    /// The `FileDescriptorProto` of the file, serialized.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_str(&mut out, 1, &self.name);
        put_str(&mut out, 2, &self.package);
        for m in &self.messages {
            let mut d = Vec::new();
            put_str(&mut d, 1, m.name);
            for f in &m.fields {
                let mut fd = Vec::new();
                put_str(&mut fd, 1, f.name);
                put_varint_field(&mut fd, 3, u64::from(f.number));
                put_varint_field(&mut fd, 4, if f.repeated { 3 } else { 1 });
                put_varint_field(&mut fd, 5, f.ty as u64);
                if let Some(t) = f.message {
                    put_str(&mut fd, 6, &self.qualified(t));
                }
                if let Some(i) = f.oneof {
                    put_varint_field(&mut fd, 9, u64::from(i));
                }
                put_str(&mut fd, 10, &json_name(f.name));
                put_bytes(&mut d, 2, &fd);
            }
            for o in &m.oneofs {
                let mut od = Vec::new();
                put_str(&mut od, 1, o);
                put_bytes(&mut d, 8, &od);
            }
            put_bytes(&mut out, 4, &d);
        }
        for s in &self.services {
            let mut sd = Vec::new();
            put_str(&mut sd, 1, &s.name);
            for m in &s.methods {
                let mut md = Vec::new();
                put_str(&mut md, 1, m.name);
                put_str(&mut md, 2, &self.qualified(m.input));
                put_str(&mut md, 3, &self.qualified(m.output));
                if m.client_streaming {
                    put_varint_field(&mut md, 5, 1);
                }
                if m.server_streaming {
                    put_varint_field(&mut md, 6, 1);
                }
                put_bytes(&mut sd, 2, &md);
            }
            put_bytes(&mut out, 6, &sd);
        }
        put_str(&mut out, 12, "proto3");
        out
    }

    /// Whether `symbol` (fully qualified, without a leading `.`) is
    /// declared in this file: a message, a field of one, a service or a
    /// method of one.
    #[must_use]
    pub fn declares(&self, symbol: &str) -> bool {
        let Some(local) = symbol
            .strip_prefix(self.package.as_str())
            .and_then(|r| r.strip_prefix('.'))
        else {
            return false;
        };
        let (head, member) = match local.split_once('.') {
            Some((h, m)) => (h, Some(m)),
            None => (local, None),
        };
        if let Some(m) = self.messages.iter().find(|m| m.name == head) {
            return match member {
                None => true,
                Some(f) => m.fields.iter().any(|x| x.name == f) || m.oneofs.contains(&f),
            };
        }
        if let Some(s) = self.services.iter().find(|s| s.name == head) {
            return match member {
                None => true,
                Some(meth) => s.methods.iter().any(|x| x.name == meth),
            };
        }
        false
    }

    /// Whether `name` (fully qualified, without a leading `.`) is a message
    /// of this file.
    #[must_use]
    pub fn has_message(&self, name: &str) -> bool {
        name.strip_prefix(self.package.as_str())
            .and_then(|r| r.strip_prefix('.'))
            .is_some_and(|local| self.messages.iter().any(|m| m.name == local))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::messages::{Field, fields};

    fn file() -> FileDesc {
        FileDesc {
            name: "a/b.proto".into(),
            package: "a.b".into(),
            messages: alloc::vec![MessageDesc {
                name: "M",
                fields: alloc::vec![
                    FieldDesc::one("first_name", 1, FieldType::String),
                    FieldDesc::msg("child", 2, "M").repeated().in_oneof(0),
                ],
                oneofs: alloc::vec!["kind"],
            }],
            services: alloc::vec![ServiceDesc {
                name: "S".into(),
                methods: alloc::vec![MethodDesc {
                    name: "Go",
                    input: "M",
                    output: "M",
                    client_streaming: true,
                    server_streaming: false,
                }],
            }],
        }
    }

    #[test]
    fn grpcbridge_4_6_json_name_is_lower_camel_case() {
        assert_eq!(json_name("cdr_payload"), "cdrPayload");
        assert_eq!(json_name("source_ts_ns"), "sourceTsNs");
        assert_eq!(json_name("flags"), "flags");
    }

    /// The file descriptor carries name, package, the message with its
    /// fields (number, label, type, type_name, oneof_index) and the
    /// service with its method, read back with the decoder of messages.rs.
    #[test]
    fn grpcbridge_4_6_the_file_descriptor_is_read_back() {
        let wire = file().encode();
        let top: Vec<(u32, Vec<u8>)> = fields(&wire)
            .map(|f| match f.unwrap() {
                (n, Field::Len(v)) => (n, v.to_vec()),
                (n, _) => panic!("field {n} is no LEN"),
            })
            .collect();
        assert_eq!(top[0], (1, b"a/b.proto".to_vec()));
        assert_eq!(top[1], (2, b"a.b".to_vec()));
        assert_eq!(top[2].0, 4);
        assert_eq!(top[3].0, 6);
        assert_eq!(top[4], (12, b"proto3".to_vec()));
        // The second field of M: repeated (3), TYPE_MESSAGE (11), `.a.b.M`,
        // oneof 0.
        let msg: Vec<_> = fields(&top[2].1).map(Result::unwrap).collect();
        let Field::Len(child) = msg[2].1 else {
            panic!()
        };
        let mut seen = Vec::new();
        for f in fields(child) {
            match f.unwrap() {
                (n, Field::Varint(v)) => seen.push((n, v, Vec::new())),
                (n, Field::Len(b)) => seen.push((n, 0, b.to_vec())),
                (n, Field::Fixed) => panic!("{n}"),
            }
        }
        assert!(seen.contains(&(3, 2, Vec::new())));
        assert!(seen.contains(&(4, 3, Vec::new())));
        assert!(seen.contains(&(5, 11, Vec::new())));
        assert!(seen.contains(&(6, 0, b".a.b.M".to_vec())));
        assert!(seen.contains(&(9, 0, Vec::new())));
        assert!(seen.contains(&(10, 0, b"child".to_vec())));
        // The method: input/output `.a.b.M`, client_streaming 1, no
        // server_streaming.
        let svc: Vec<_> = fields(&top[3].1).map(Result::unwrap).collect();
        let Field::Len(method) = svc[1].1 else {
            panic!()
        };
        let m: Vec<_> = fields(method).map(Result::unwrap).collect();
        assert!(matches!(m[1], (2, Field::Len(b".a.b.M"))));
        assert!(matches!(m[3], (5, Field::Varint(1))));
        assert_eq!(m.len(), 4);
    }

    #[test]
    fn grpcbridge_4_6_declares_finds_messages_fields_services_and_methods() {
        let f = file();
        for s in [
            "a.b.M",
            "a.b.M.first_name",
            "a.b.M.kind",
            "a.b.S",
            "a.b.S.Go",
        ] {
            assert!(f.declares(s), "{s}");
        }
        for s in ["a.b", "a.b.X", "a.b.M.x", "a.b.S.Stop", "a.bb.M", "M"] {
            assert!(!f.declares(s), "{s}");
        }
        assert!(f.has_message("a.b.M"));
        assert!(!f.has_message("a.b.S"));
    }
}
