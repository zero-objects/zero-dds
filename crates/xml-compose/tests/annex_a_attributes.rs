//! Attributes of Annex A of DDS-XTypes 1.3 the XML loader read wrongly.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::{AnyId, AutoId};
use zerodds_xml_compose::{Composed, compose_source};

fn load(body: &str) -> Composed {
    compose_source(
        "annex_a.xml",
        &format!("<dds><types><module name=\"T\">{body}</module></types></dds>"),
    )
}

fn struct_id(composed: &Composed, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    composed
        .ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            "T".to_string(),
            name.to_string(),
        ]))
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{name} is declared"))
}

fn member_facet(composed: &Composed, struct_: &str, member: &str) -> zerodds_ast::XTypesFacet {
    let id = struct_id(composed, struct_);
    let member = composed
        .ast
        .members_of(id)
        .iter()
        .filter_map(|m| composed.ast.member(*m).ok())
        .find(|m| m.name == member)
        .expect("the member");
    member.facets.xtypes.clone().unwrap_or_default()
}

/// §7.3.2.4: for a collection member `external` is the element's.
#[test]
fn external_on_a_collection_member_is_the_elements() {
    let composed = load(
        "<struct name=\"S\">\
         <member name=\"plain\" type=\"int32\" external=\"true\"/>\
         <member name=\"seq\" type=\"int32\" sequenceMaxLength=\"-1\" external=\"true\"/>\
         <member name=\"arr\" type=\"int32\" arrayDimensions=\"3\" external=\"true\"/>\
         </struct>",
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let plain = member_facet(&composed, "S", "plain");
    assert!(plain.is_external && !plain.element_is_external);
    for name in ["seq", "arr"] {
        let facet = member_facet(&composed, "S", name);
        assert!(!facet.is_external, "{name}: the member is not external");
        assert!(facet.element_is_external, "{name}: its elements are");
    }
}

/// §7.3.2.4: "no other negative value is permitted" than -1.
#[test]
fn a_negative_bound_other_than_minus_one_is_refused() {
    let unbounded = load(
        "<struct name=\"S\"><member name=\"a\" type=\"int32\" sequenceMaxLength=\"-1\"/></struct>",
    );
    assert!(!unbounded.failed(), "{:?}", unbounded.diagnostics);
    let refused = load(
        "<struct name=\"S\"><member name=\"a\" type=\"int32\" sequenceMaxLength=\"-5\"/></struct>",
    );
    assert!(refused.failed());
}

/// Annex A spells the enumeration `sequencial`.
#[test]
fn autoid_reads_annex_as_spelling() {
    let composed = load(
        "<struct name=\"S\" autoid=\"sequencial\"><member name=\"a\" type=\"int32\"/></struct>",
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let id = struct_id(&composed, "S");
    assert_eq!(composed.ast.effective_autoid(id), Some(AutoId::Sequential));
}
