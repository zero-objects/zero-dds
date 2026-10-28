// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@key` on a union discriminator, on both values of the axis that
//! decides whether a front end carries it.
//!
//! DDS-XTypes 1.3 Table 21 lists `@key` against "Structure Members,
//! Union discriminator" and §7.2.2.4.4.4.8's own example is `union
//! MyCommand switch (@key CommandKind)`, so the clause permits it.
//! Measured 2026-08-10, `n11-keyunion/`: Cyclone's `idlc` refuses
//! the file, `rtiddsgen` refuses it at the discriminator, and
//! `fastddsgen` generates no `PubSubType` for a union at all. Clause and
//! implementations disagree, so the answer is an axis:
//! [`KeyOnUnionDiscriminator::Carried`] is the default and records the
//! declaration, [`KeyOnUnionDiscriminator::Refused`] reports it.
//!
//! Behind `Carried` the whole path has to hold, which is what this file
//! pins: the declaration loads, the discriminator's `TypeObject` flags
//! carry `IS_KEY`, the two `TypeIdentifier`s differ, and
//! §7.2.4.4.4.5's first condition — "either the discriminators of both
//! T1 and T2 are keys or neither are keys" — refuses the pair. That
//! refusal is the `INCONSISTENT_TOPIC` the OMG `dds-xtypes` suite's
//! `union_uint32_one_key` expects on both sides.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::TypeNode;
use zerodds_ast_core::Id;
use zerodds_types::assignability::{AssignabilityConfig, is_assignable};
use zerodds_types::from_ast::{Form, type_identifier};
use zerodds_types::type_object::flags::UnionDiscriminatorFlag;
use zerodds_xml_compose::{ComposeOptions, Composed, KeyOnUnionDiscriminator, compose_source_with};

/// `types/xml/unions.xml`'s `union_uint32` and
/// `types/xml/unions_key_discriminator.xml`'s `union_uint32_key`, the
/// pair the suite's `union_uint32_one_key` names. They differ in
/// `key="true"` and in nothing else.
const SOURCE: &str = r#"<dds>
  <types>
    <module name="Test">
      <union name="union_uint32" ><discriminator type="uint32"/>
      <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
      <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
      </union>
      <union name="union_uint32_key" ><discriminator type="uint32" key="true"/>
      <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
      <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
      </union>
    </module>
  </types>
</dds>"#;

fn load(axis: KeyOnUnionDiscriminator) -> Composed {
    compose_source_with(
        "unions_key_discriminator.xml",
        SOURCE,
        ComposeOptions {
            key_on_union_discriminator: axis,
            ..ComposeOptions::default()
        },
    )
}

fn type_of(composed: &Composed, name: &str) -> Id<TypeNode> {
    let segments = ["Test", name].map(str::to_owned).to_vec();
    composed
        .ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("the document declares {name}"))
}

/// `Refused` reports the declaration and builds no union from it.
///
/// This is what makes `union_uint32_one_key` end in `TOPIC_NOT_CREATED`
/// rather than in the `INCONSISTENT_TOPIC` the suite asks for, and it is
/// the behaviour the measured front ends have.
#[test]
fn refused_reports_the_declaration() {
    let composed = load(KeyOnUnionDiscriminator::Refused);
    assert!(composed.failed(), "Refused carries no key discriminator");
    let reported = composed
        .diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    assert!(
        reported.contains("union discriminator"),
        "the diagnostic names the position: {reported}"
    );
    assert!(
        reported.contains("7.2.2.4.4.4.8"),
        "the diagnostic names the clause that permits it: {reported}"
    );
}

/// The default carries the declaration (Table 21 permits `key` on a
/// discriminator): the document loads and the key is in the model.
#[test]
fn the_default_carries_the_declaration() {
    let composed = load(KeyOnUnionDiscriminator::default());
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    assert!(
        composed
            .ast
            .discriminator_is_key(type_of(&composed, "union_uint32_key"))
    );
}

/// Behind the axis the declaration loads, and `IS_KEY` reaches the
/// discriminator's `TypeObject` flags.
#[test]
fn the_axis_carries_the_key_into_the_type_object() {
    let composed = load(KeyOnUnionDiscriminator::Carried);
    assert!(!composed.failed(), "{:?}", composed.diagnostics);

    let plain = type_of(&composed, "union_uint32");
    let keyed = type_of(&composed, "union_uint32_key");
    assert!(!composed.ast.discriminator_is_key(plain));
    assert!(composed.ast.discriminator_is_key(keyed));

    let flags = |id| {
        zerodds_types::from_ast::minimal_type_object(&composed.ast, id)
            .expect("a minimal TypeObject")
    };
    let object = flags(keyed);
    let zerodds_types::type_object::minimal::MinimalTypeObject::Union(union_) = object else {
        panic!("union_uint32_key is a union");
    };
    assert!(
        union_
            .discriminator
            .common
            .member_flags
            .has(UnionDiscriminatorFlag::IS_KEY),
        "the discriminator carries IS_KEY"
    );

    let object = flags(plain);
    let zerodds_types::type_object::minimal::MinimalTypeObject::Union(union_) = object else {
        panic!("union_uint32 is a union");
    };
    assert!(
        !union_
            .discriminator
            .common
            .member_flags
            .has(UnionDiscriminatorFlag::IS_KEY),
        "the unkeyed discriminator does not"
    );
}

/// §7.2.4.4.4.5: the two are not assignable in either direction, and
/// their `TypeIdentifier`s differ.
///
/// The verdict the suite's `union_uint32_one_key` expects on both sides.
/// Both directions, because the case runs `union_uint32` as the
/// publisher and `union_uint32_key` as the subscriber and expects
/// `INCONSISTENT_TOPIC` from each.
#[test]
fn a_key_discriminator_is_not_assignable_to_a_plain_one() {
    let composed = load(KeyOnUnionDiscriminator::Carried);
    assert!(!composed.failed(), "{:?}", composed.diagnostics);

    let identifier = |name: &str| {
        type_identifier(&composed.ast, type_of(&composed, name), Form::Complete)
            .unwrap_or_else(|e| panic!("{name} has an identifier: {e:?}"))
    };
    let plain = identifier("union_uint32");
    let keyed = identifier("union_uint32_key");
    assert_ne!(
        plain, keyed,
        "the key bit is part of the equivalence hash, or the two could not be told apart"
    );

    let registry = &composed.registry;
    let cfg = AssignabilityConfig::default();
    assert!(!is_assignable(&plain, &keyed, registry, &cfg).is_yes());
    assert!(!is_assignable(&keyed, &plain, registry, &cfg).is_yes());

    // The control: the axis does not make every union unassignable to
    // itself.
    assert!(is_assignable(&keyed, &keyed, registry, &cfg).is_yes());
    assert!(is_assignable(&plain, &plain, registry, &cfg).is_yes());
}
