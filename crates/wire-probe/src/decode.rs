// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The bytes back into the value — at `serde_json::Value`.
//!
//! The walk is `zerodds_wire_codec::decode`'s and follows the same
//! layout plan the encoder does. What is here is the value file's end of
//! it: the entry point pinned to the representation the probe writes,
//! and the shorthand that parses the IDL first.
//!
//! [`decode`] carries a recursion marker for the reason `contract.rs`'s
//! module note gives: `dds_bounded_recursion` reads a path call by its
//! last segment, so forwarding to the function of the same name in the
//! other crate looks like recursion. Depth 1.

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::encode::ProbeError;
use zerodds_wire_codec::value::ProbeRequest;

/// Read `bytes` as `request.type_name`, in canonical form. See
/// [`zerodds_wire_codec::decode::decode`].
///
/// # Errors
/// [`ProbeError`], naming the path in the value or the offset in the
/// stream.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The one call
/// it makes is `zerodds_wire_codec::decode::decode`, and the lint reads a
/// path call by its last segment. See the module note.
#[inline]
pub fn decode(ast: &Ast, request: &ProbeRequest, bytes: &[u8]) -> Result<Value, ProbeError> {
    zerodds_wire_codec::decode::decode(ast, request, bytes)
}

/// Shorthand for the common case: one IDL source, one type, one payload.
///
/// # Errors
/// As [`decode`], plus the frontend's own diagnostics.
pub fn unprobe(
    idl: &str,
    type_name: &str,
    bytes: &[u8],
    encoding: Encoding,
    endianness: Endianness,
) -> Result<Value, ProbeError> {
    let ast = crate::encode::model_from_idl(idl, "probe.idl")?;
    decode(
        &ast,
        &ProbeRequest::new(type_name, encoding, endianness),
        bytes,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::canonical::text;
    use crate::encode::{encode, model_from_idl};
    use serde_json::json;
    use zerodds_ast::TypeNode;
    use zerodds_ast_core::{Id, ScopedName};

    fn find_type(ast: &Ast, name: &str) -> Id<TypeNode> {
        let segments: Vec<String> = name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect();
        ast.find_by_scoped_name(&ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model")
    }

    /// One value out, the bytes, and the value back — through this
    /// project alone, which is what the vendor pairs are then compared
    /// against.
    fn round_trip(idl: &str, type_name: &str, value: &Value, encoding: Encoding) -> String {
        let ast = model_from_idl(idl, "decode.idl").expect("the fixture parses");
        for endianness in [Endianness::Little, Endianness::Big] {
            let request = ProbeRequest::new(type_name, encoding, endianness);
            let bytes = encode(&ast, &request, value).expect("the sample serialises");
            let back = decode(&ast, &request, &bytes).expect("the bytes read back");
            let there =
                crate::canonical::canonicalise(&ast, find_type(&ast, type_name), value, type_name)
                    .expect("the sample is well formed");
            assert_eq!(
                text(&there),
                text(&back),
                "{type_name} under {encoding:?}/{endianness:?}"
            );
        }
        let ast = model_from_idl(idl, "decode.idl").expect("the fixture parses");
        let request = ProbeRequest::new(type_name, encoding, Endianness::Little);
        let bytes = encode(&ast, &request, value).expect("the sample serialises");
        text(&decode(&ast, &request, &bytes).expect("the bytes read back"))
    }

    #[test]
    fn xtypes_7_4_3_5_3_a_union_with_no_default_and_no_matching_label_carries_no_member() {
        let idl = "union Choice switch (long) { case 1: long whole; case 2: double fraction; };
                   @final struct S { Choice picked; long tail; };";
        assert_eq!(
            round_trip(
                idl,
                "S",
                &json!({ "picked": { "_d": 99 }, "tail": -1 }),
                Encoding::PlainCdr2
            ),
            r#"{"picked":{"_d":99},"tail":-1}"#
        );
    }

    #[test]
    fn a_truncated_payload_is_an_error_and_not_a_shorter_sample() {
        let ast = model_from_idl("@final struct S { long a; long b; };", "decode.idl")
            .expect("the fixture parses");
        let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
        let bytes = encode(&ast, &request, &json!({ "a": 1, "b": 2 })).expect("it serialises");
        let refused = decode(&ast, &request, &bytes[..4]);
        assert!(refused.is_err(), "four bytes are not two longs");
    }

    #[test]
    fn bytes_left_over_are_an_error_and_not_ignored() {
        let ast = model_from_idl("@final struct S { long a; };", "decode.idl")
            .expect("the fixture parses");
        let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
        let mut bytes = encode(&ast, &request, &json!({ "a": 1 })).expect("it serialises");
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let refused = decode(&ast, &request, &bytes);
        assert!(refused.is_err(), "a tail this long is not padding");
    }
}
