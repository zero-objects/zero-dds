// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The example of DDS-XTypes 1.3 §7.6.3.1.2 (S. 216-217): TypeA, TypeB and
//! TypeC in CDR big endian with the encapsulation header and the `options`
//! field, whose two least significant bits of the second octet count the
//! padding octets behind the payload. The expected octets are those of the
//! three figures of the spec; the figure of O3 names `O3.member2` twice, the
//! third member is `member3` (a typing slip of the spec).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast::{Ast, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::decode;
use zerodds_wire_codec::encode;
use zerodds_wire_codec::value::{Emit, ProbeRequest};

fn types() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let short = ast
        .add_type(root, "Short", TypeKind::Primitive(PrimitiveKind::Int16))
        .expect("short");
    let char_ = ast
        .add_type(root, "Char", TypeKind::Primitive(PrimitiveKind::Char))
        .expect("char");
    let mut structure = |name: &str, members: &[(&str, Id<TypeNode>)]| {
        let id = ast
            .add_type(
                root,
                name,
                TypeKind::Structure {
                    base: None,
                    members: Vec::new(),
                },
            )
            .expect("struct");
        let any = ast.any_of_type(id).expect("any");
        ast.facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .extensibility = Some(Extensibility::Final);
        for (member, type_) in members {
            ast.add_member(id, MemberSpec::new(member, Ref::Resolved(*type_)))
                .expect("member");
        }
    };
    structure("TypeA", &[("member1", short)]);
    structure("TypeB", &[("member1", short), ("member2", char_)]);
    structure(
        "TypeC",
        &[("member1", short), ("member2", char_), ("member3", char_)],
    );
    ast
}

fn request(type_name: &str) -> ProbeRequest {
    let mut request = ProbeRequest::new(type_name, Encoding::PlainCdr1, Endianness::Big);
    request.emit = Emit::PayloadWithEncapsulation;
    request
}

#[test]
fn the_options_count_the_padding_of_the_three_objects() {
    let ast = types();
    let cases = [
        (
            "TypeA",
            json!({"member1": 0x11}),
            vec![0x00, 0x00, 0x00, 0x02, 0x00, 0x11, 0x00, 0x00],
        ),
        (
            "TypeB",
            json!({"member1": 0x23, "member2": "b"}),
            vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x23, 0x62, 0x00],
        ),
        (
            "TypeC",
            json!({"member1": 0x23, "member2": "b", "member3": "c"}),
            vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x23, 0x62, 0x63],
        ),
    ];
    for (name, value, expected) in cases {
        let bytes = encode::encode(&ast, &request(name), &value).expect("encodes");
        assert_eq!(bytes, expected, "{name}");
        // The receiver reads the same octets back and finds where the data
        // ended from the two bits.
        let back: serde_json::Value =
            decode::decode(&ast, &request(name), &expected).expect("decodes");
        assert_eq!(back, value, "{name} read back");
    }
}
