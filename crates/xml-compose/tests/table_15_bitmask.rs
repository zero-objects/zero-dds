// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 §7.2.4.4.4, Table 15, rows two to five, over the types the
//! OMG `dds-xtypes` suite declares rather than over types built for the
//! test.
//!
//! The suite's `types/xml/unions_bitmask.xml` pairs a union whose
//! discriminator is a 16- and a 32-bit bitmask against
//! `unions.xml`'s `union_uint32`, and its two cases
//! (`union_uint32_bitmask32`, `union_uint32_bitmask16`) expect the first
//! pair to communicate and the second to be refused. Reproduced here on
//! the model rather than over the wire, so that the verdict is pinned
//! independently of discovery.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_types::assignability::{AssignabilityConfig, is_assignable};
use zerodds_types::from_ast::{Form, type_identifier};
use zerodds_xml_compose::compose_source;

/// The three unions of the suite, verbatim from
/// `types/xml/unions_bitmask.xml` plus `unions.xml`'s `union_uint32`,
/// which the suite pairs them with.
const SOURCE: &str = r#"<dds>
    <types>
      <module name="Test">
        <bitmask name="B_16" bitBound="16">
            <flag name="B_FLAG_1" value="0"/>
            <flag name="B_FLAG_2" value="1"/>
        </bitmask>
        <bitmask name="B_32" bitBound="32">
            <flag name="B_FLAG_1" value="0"/>
            <flag name="B_FLAG_2" value="1"/>
            <flag name="B_FLAG_3" value="2"/>
        </bitmask>
        <union name="union_uint32" ><discriminator type="uint32" />
            <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
            <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
        </union>
        <union name="union_bitmask16" ><discriminator type="nonBasic" nonBasicTypeName="B_16"/>
            <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
            <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
        </union>
        <union name="union_bitmask32" ><discriminator type="nonBasic" nonBasicTypeName="B_32"/>
            <case><caseDiscriminator value="2"/><member name="x1" type="int16"/></case>
            <case><caseDiscriminator value="1"/><member name="x2" type="int32"/></case>
        </union>
      </module>
    </types>
</dds>"#;

/// A `uint32` discriminator and a 32-bit bitmask discriminator are
/// assignable; a 16-bit one is not.
///
/// Table 15 pairs a bitmask with the one unsigned integer whose range
/// its bit bound falls in — [17, 32] is `uint32`, [9, 16] is `uint16` —
/// and the ranges neither overlap nor nest, so 16 bits fitting into 32
/// changes nothing. Both directions, because the row is symmetric.
#[test]
fn a_bitmask_discriminator_matches_the_uint_of_its_bound() {
    let composed = compose_source("unions_bitmask.xml", SOURCE);
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let ast = &composed.ast;

    let ti = |name: &str| {
        let segments = ["Test", name].map(str::to_owned).to_vec();
        let id = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .unwrap_or_else(|| panic!("the document declares {name}"));
        type_identifier(ast, id, Form::Complete)
            .unwrap_or_else(|e| panic!("{name} has an identifier: {e:?}"))
    };

    let uint32 = ti("union_uint32");
    let bitmask32 = ti("union_bitmask32");
    let bitmask16 = ti("union_bitmask16");
    let registry = &composed.registry;
    let cfg = AssignabilityConfig::default();

    assert!(is_assignable(&uint32, &bitmask32, registry, &cfg).is_yes());
    assert!(is_assignable(&bitmask32, &uint32, registry, &cfg).is_yes());
    assert!(!is_assignable(&uint32, &bitmask16, registry, &cfg).is_yes());
    assert!(!is_assignable(&bitmask16, &uint32, registry, &cfg).is_yes());
}
