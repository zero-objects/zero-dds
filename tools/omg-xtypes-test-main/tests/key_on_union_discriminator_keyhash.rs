// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The KeyHash of a union keyed by its discriminator.
//!
//! `crates/xml-compose/tests/key_on_union_discriminator.rs` pins the
//! model and the assignability verdict; this is the rest of the path.
//! XTypes 1.3 §7.6.8 Step 1 removes everything that is not a key, and
//! for a union keyed by its discriminator that leaves the discriminator:
//! the branches are not keys, and which branch a sample carries is the
//! discriminator's own answer. So the holder is four big-endian octets
//! and the hash is those octets zero-padded to sixteen.
//!
//! No measured implementation computes this — `idlc`, `rtiddsgen` and
//! `fastddsgen` all refuse the declaration, measured 2026-08-10 — which
//! is exactly why it sits behind
//! `KeyOnUnionDiscriminator::Carried` and reaches no wire unless
//! somebody asks for it.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::TypeNode;
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness, union_key_discriminator};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{Emit, encode};
use zerodds_xml_compose::{ComposeOptions, Composed, KeyOnUnionDiscriminator, compose_source_with};

/// `types/xml/unions_key_discriminator.xml` verbatim.
const SOURCE: &str = r#"<dds>
  <types>
    <module name="Test">
      <union name="union_uint32_key" ><discriminator type="uint32" key="true"/>
      <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
      <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
      </union>
    </module>
  </types>
</dds>"#;

fn model() -> Composed {
    let composed = compose_source_with(
        "unions_key_discriminator.xml",
        SOURCE,
        ComposeOptions {
            key_on_union_discriminator: KeyOnUnionDiscriminator::Carried,
            ..ComposeOptions::default()
        },
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    composed
}

fn union_type(composed: &Composed) -> Id<TypeNode> {
    let segments = ["Test", "union_uint32_key"].map(str::to_owned).to_vec();
    composed
        .ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the document declares union_uint32_key")
}

fn key_hash(composed: &Composed, sample: &serde_json::Value) -> Vec<u8> {
    let request = ProbeRequest::new(
        "Test::union_uint32_key",
        Encoding::DelimitedCdr,
        Endianness::Big,
    )
    .emitting(Emit::KeyHash);
    encode(&composed.ast, &request, sample).expect("the keyed union has a KeyHash")
}

/// A union keyed by its discriminator is a keyed type, and the walk over
/// members alone does not see it.
///
/// `zerodds_ast_layout::key_members` answers the empty list for it —
/// Table 21 puts the annotation on the discriminator, which is not a
/// `MemberNode` — so anything asking "is this keyed" has to ask
/// [`union_key_discriminator`] as well. A reader that asked only the
/// member walk would announce a keyless topic for a keyed type.
#[test]
fn the_discriminator_is_what_makes_the_union_keyed() {
    let composed = model();
    let type_ = union_type(&composed);
    assert!(
        zerodds_ast_layout::key_members(&composed.ast, type_)
            .expect("a key member list")
            .is_empty(),
        "no member of this union carries @key"
    );
    assert!(
        union_key_discriminator(&composed.ast, type_).is_some(),
        "and yet the type is keyed"
    );
}

/// The holder is the discriminator, big-endian, zero-padded to sixteen.
#[test]
fn the_key_hash_is_the_discriminator_alone() {
    let composed = model();
    let hash = key_hash(&composed, &serde_json::json!({"_d": 2, "x1": 128}));
    assert_eq!(
        hash,
        vec![0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        "four big-endian octets of the discriminator, zero-padded"
    );
}

/// Two samples of one instance hash alike; two instances do not.
///
/// The branch value is not part of the key — Step 1 removed it — so a
/// change to `x1` leaves the instance where it was, and a change to the
/// discriminator moves it. Without both halves the hash would be either
/// a constant or a digest of the whole sample, and each of those is a
/// keyed topic that behaves like a keyless one.
#[test]
fn the_branch_value_is_not_part_of_the_instance() {
    let composed = model();
    let one = key_hash(&composed, &serde_json::json!({"_d": 2, "x1": 128}));
    let same_instance = key_hash(&composed, &serde_json::json!({"_d": 2, "x1": 7}));
    let other_instance = key_hash(&composed, &serde_json::json!({"_d": 1, "x2": 128}));
    assert_eq!(one, same_instance);
    assert_ne!(one, other_instance);
}

/// The sample itself is unchanged by the annotation.
///
/// `@key` on the discriminator decides the instance and the type's
/// identity; it writes nothing extra into a sample. The octets below are
/// the same ones `union_uint32` produces, which is what makes the
/// refusal at discovery the only difference between the two types on the
/// wire.
#[test]
fn the_payload_is_the_same_octets_as_the_unkeyed_union() {
    let composed = model();
    let request = ProbeRequest::new(
        "Test::union_uint32_key",
        Encoding::DelimitedCdr,
        Endianness::Little,
    );
    let bytes = encode(
        &composed.ast,
        &request,
        &serde_json::json!({"_d": 2, "x1": 128}),
    )
    .expect("the keyed union encodes");
    // DHEADER(6) | discriminator 2 as uint32 | x1 = 128 as int16.
    assert_eq!(bytes, [0x06, 0, 0, 0, 0x02, 0, 0, 0, 0x80, 0x00]);
}
