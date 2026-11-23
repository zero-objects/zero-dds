// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS 1.2 §8.2.2 the generation process: the XML model tags under the
//! DTD of §8.2.2.3.1, and the generator that applies them to the model
//! description of IDL valuetypes (§8.2.2.1, §8.2.2.2), with the example
//! of §8.2.3.

#![cfg(feature = "model-tags")]
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, missing_docs)]

mod common;

use common::{example_model, example_tags};
use zerodds_dlrl::generation::{
    GenerationError, MemberDecl, ModelDescription, TypeRef, ValuetypeDecl, generate,
};
use zerodds_dlrl::mapping::{KeyContent, Mapping};
use zerodds_dlrl::model::{
    BasicType, CollectionBasis, EnumCoding, EnumerationType, Member, Multiplicity, SimpleType,
};
use zerodds_dlrl::model_tags::{ModelTags, RelationRef, parse_model_tags};

/// The full model tags of §8.2.3.3.
const FULL: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<!DOCTYPE Dlrl SYSTEM "dlrl.dtd">
<Dlrl name="example">
  <templateDef name="StringStrMap" pattern="StrMap" itemType="string"/>
  <templateDef name="TrackList" pattern="List" itemType="Track"/>
  <classMapping name="Track">
    <mainTopic name="TRACK-TOPIC">
      <keyDescription content="FullOid">
        <keyField>CLASS</keyField>
        <keyField>OID</keyField>
      </keyDescription>
    </mainTopic>
    <monoAttribute name="x">
      <valueField>X</valueField>
    </monoAttribute>
    <monoAttribute name="y">
      <placeTopic name="Y_TOPIC">
        <keyDescription content="FullOid">
          <keyField>CLASS</keyField>
          <keyField>OID</keyField>
        </keyDescription>
      </placeTopic>
      <valueField>Y</valueField>
    </monoAttribute>
    <multiAttribute name="comments">
      <multiPlaceTopic name="COMMENTS-TOPIC" indexField="INDEX">
        <keyDescription content="FullOid">
          <keyField>CLASS</keyField>
          <keyField>OID</keyField>
        </keyDescription>
      </multiPlaceTopic>
      <valueField>COMMENT</valueField>
    </multiAttribute>
    <monoRelation name="a_radar">
      <keyDescription content="SimpleOid">
        <keyField>RADAR_OID</keyField>
      </keyDescription>
    </monoRelation>
    <local name="w"/>
  </classMapping>
  <classMapping name="Track3D">
    <mainTopic name="TRACK-TOPIC">
      <keyDescription content="FullOid">
        <keyField>CLASS</keyField>
        <keyField>OID</keyField>
      </keyDescription>
    </mainTopic>
    <extensionTopic name="TRACK3D-TOPIC">
      <keyDescription content="FullOid">
        <keyField>CLASS</keyField>
        <keyField>OID</keyField>
      </keyDescription>
    </extensionTopic>
    <monoAttribute name="z">
      <valueField>Z</valueField>
    </monoAttribute>
  </classMapping>
  <classMapping name="Radar">
    <mainTopic name="RADAR-TOPIC">
      <keyDescription content="SimpleOid">
        <keyField>OID</keyField>
      </keyDescription>
    </mainTopic>
    <multiRelation name="tracks">
      <multiPlaceTopic name="RADARTRACKS-TOPIC" indexField="INDEX">
        <keyDescription content="SimpleOid">
          <keyField>RADAR-OID</keyField>
        </keyDescription>
      </multiPlaceTopic>
      <keyDescription content="FullOid">
        <keyField>TRACK-CLASS</keyField>
        <keyField>TRACK-OID</keyField>
      </keyDescription>
    </multiRelation>
  </classMapping>
  <associationDef>
    <relation class="Track" attribute="a_radar"/>
    <relation class="Radar" attribute="tracks"/>
  </associationDef>
</Dlrl>
"#;

/// The minimal model tags of §8.2.3.3.
const MINIMAL: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<!DOCTYPE Dlrl SYSTEM "dlrl.dtd">
<Dlrl name="Example">
  <templateDef name="stringStrMap" pattern="StrMap" itemType="string"/>
  <templateDef name="TrackList" pattern="List" itemType="Track"/>
  <classMapping name="Track">
<local name="w"/>
  </classMapping>
  <associationDef>
    <relation class="Track" attribute="a_radar"/>
    <relation class="Radar" attribute="tracks"/>
                                 </associationDef>
</Dlrl>
"#;

fn member(name: &str, ty: TypeRef) -> MemberDecl {
    MemberDecl {
        name: name.into(),
        ty,
    }
}

fn double() -> TypeRef {
    TypeRef::Basic(BasicType::Double)
}

/// The IDL model description of §8.2.3.2, as `zerodds-idlc` reads it.
fn example_description() -> ModelDescription {
    ModelDescription {
        valuetypes: vec![
            ValuetypeDecl {
                name: "Track".into(),
                bases: vec!["DLRL::ObjectRoot".into()],
                members: vec![
                    member("x", double()),
                    member("y", double()),
                    member("comments", TypeRef::Valuetype("stringStrMap".into())),
                    member("w", TypeRef::Basic(BasicType::Long)),
                    member("a_radar", TypeRef::Valuetype("Radar".into())),
                ],
            },
            ValuetypeDecl {
                name: "Track3D".into(),
                bases: vec!["Track".into()],
                members: vec![member("z", double())],
            },
            ValuetypeDecl {
                name: "Radar".into(),
                bases: vec!["DLRL::ObjectRoot".into()],
                members: vec![member("tracks", TypeRef::Valuetype("TrackList".into()))],
            },
        ],
        types: vec![],
    }
}

fn tags(xml: &str) -> ModelTags {
    parse_model_tags(xml.as_bytes()).unwrap_or_else(|e| panic!("{e}"))
}

/// §8.2.2.3, §8.2.3.3: the full model tags of the example parse into the
/// class mappings, templates, association and local of the example.
#[test]
fn dlrl_8_2_2_3_the_full_tags_of_8_2_3_3_parse() {
    let t = tags(FULL);
    assert_eq!(t.name.as_deref(), Some("example"));
    assert_eq!(t.class_mappings, example_tags());
    assert_eq!(t.templates.len(), 2);
    assert_eq!(t.templates[0].name, "StringStrMap");
    assert_eq!(t.templates[0].pattern, CollectionBasis::StrMap);
    assert_eq!(t.templates[0].item_type, "string");
    assert_eq!(t.templates[1].pattern, CollectionBasis::List);
    assert_eq!(t.templates[1].item_type, "Track");
    assert_eq!(
        t.associations,
        vec![[
            RelationRef {
                class: "Track".into(),
                attribute: "a_radar".into()
            },
            RelationRef {
                class: "Radar".into(),
                attribute: "tracks".into()
            },
        ]]
    );
    assert_eq!(t.locals.len(), 1);
    assert_eq!(
        (t.locals[0].class.as_str(), t.locals[0].attribute.as_str()),
        ("Track", "w")
    );
    // The same document in UTF-8 reads the same.
    let utf8 = FULL.replace("ISO-8859-1", "UTF-8");
    assert_eq!(tags(&utf8), t);
}

/// §8.2.2.1: the description of §8.2.3.2 with the full tags gives the
/// model of §8.2.3.2 and the mapping the tags describe.
#[test]
fn dlrl_8_2_2_1_the_example_generates_model_and_mapping() {
    let m = generate(&example_description(), &tags(FULL)).unwrap();
    let model = m.model();
    // The member types: comments a StrMap of strings, a_radar a to-1
    // relation, tracks a to-many List relation, w local.
    match model.member("Track", "comments") {
        Some((_, Member::Attribute(a))) => assert_eq!(
            a.ty,
            zerodds_dlrl::model::AttributeType::Multi {
                basis: CollectionBasis::StrMap,
                item: SimpleType::Basic(BasicType::String)
            }
        ),
        other => panic!("comments: {other:?}"),
    }
    match model.member("Track", "a_radar") {
        Some((_, Member::Relation(r))) => {
            assert_eq!(r.target, "Radar");
            assert_eq!(r.multiplicity, Multiplicity::ToOne);
            assert_eq!(r.inverse.as_deref(), Some("tracks"));
        }
        other => panic!("a_radar: {other:?}"),
    }
    match model.member("Radar", "tracks") {
        Some((_, Member::Relation(r))) => {
            assert_eq!(r.target, "Track");
            assert_eq!(r.multiplicity, Multiplicity::ToMany(CollectionBasis::List));
            assert_eq!(r.inverse.as_deref(), Some("a_radar"));
        }
        other => panic!("tracks: {other:?}"),
    }
    assert_eq!(model.member("Track", "w").map(|x| x.1), Some(Member::Local));
    assert!(model.is_kind_of("Track3D", "Track"));
    // The topics are those of the mapping the fixture gives directly.
    let expected = Mapping::new(example_model(), &example_tags()).unwrap();
    assert_eq!(m.topics(), expected.topics());
    assert_eq!(m.classes(), expected.classes());
}

/// §8.2.3.3: with the minimal tags every class takes the default mapping
/// of §8.1.4.3.
#[test]
fn dlrl_8_2_2_3_the_minimal_tags_of_8_2_3_3_take_the_default_mapping() {
    let t = tags(MINIMAL);
    assert_eq!(t.class_mappings.len(), 1);
    assert!(t.class_mappings[0].members.is_empty());
    assert!(t.class_mappings[0].main_topic.is_none());
    let m = generate(&example_description(), &t).unwrap();
    let expected = Mapping::default_for(example_model()).unwrap();
    assert_eq!(m.topics(), expected.topics());
    assert_eq!(m.classes(), expected.classes());
}

fn refused(xml: &str, path: &str, what: &str) {
    match parse_model_tags(xml.as_bytes()) {
        Ok(t) => panic!("accepted: {xml}\n{t:?}"),
        Err(e) => {
            assert!(
                e.path.contains(path),
                "{xml}: path {:?} does not contain {path:?} ({})",
                e.path,
                e.message
            );
            assert!(
                e.message.contains(what),
                "{xml}: message {:?} does not contain {what:?}",
                e.message
            );
        }
    }
}

fn doc(body: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="ISO-8859-1"?><Dlrl name="d">{body}</Dlrl>"#)
}

fn class(body: &str) -> String {
    doc(&format!(r#"<classMapping name="C">{body}</classMapping>"#))
}

const SIMPLE: &str =
    r#"<keyDescription content="SimpleOid"><keyField>OID</keyField></keyDescription>"#;

/// §8.2.2.3.1, §8.2.2.3.2.1 to .14: each construct read with well-formed
/// input, each deviation from the DTD refused with the path of the
/// element.
#[test]
fn dlrl_8_2_2_3_1_each_violation_of_the_dtd_is_refused_with_its_path() {
    // Root (§8.2.2.3.2.1).
    refused("<Other/>", "Other", "root element is not Dlrl");
    refused(&doc("<value>x</value>"), "value", "not an element of Dlrl");
    refused(&doc("text"), "Dlrl", "holds no text");
    refused(
        r#"<Dlrl name="d" kind="x"/>"#,
        "Dlrl",
        "has no attribute kind",
    );
    refused("<Dlrl><unclosed></Dlrl>", "", "");
    // enumDef (.2).
    let t = tags(&doc(
        r#"<enumDef name="WeekDays"><value>Monday</value><value>Tuesday</value></enumDef>"#,
    ));
    assert_eq!(t.enums[0].name, "WeekDays");
    assert_eq!(t.enums[0].values, ["Monday", "Tuesday"]);
    refused(
        &doc(r#"<enumDef><value>a</value></enumDef>"#),
        "enumDef",
        "required attribute name",
    );
    refused(
        &doc(r#"<enumDef name="E"><item>a</item></enumDef>"#),
        "item",
        "value elements only",
    );
    refused(
        &doc(r#"<enumDef name="E"><value/></enumDef>"#),
        "value",
        "is empty",
    );
    refused(
        &doc(r#"<enumDef name="E"><value><b/></value></enumDef>"#),
        "b",
        "text only",
    );
    // templateDef (.3).
    refused(
        &doc(r#"<templateDef name="T" pattern="Bag" itemType="long"/>"#),
        "templateDef",
        "pattern Bag",
    );
    refused(
        &doc(r#"<templateDef name="T" pattern="List"/>"#),
        "templateDef",
        "itemType",
    );
    refused(
        &doc(r#"<templateDef name="T" pattern="List" itemType="long">x</templateDef>"#),
        "templateDef",
        "holds no text",
    );
    refused(
        &doc(
            r#"<templateDef name="T" pattern="List" itemType="long"><value>a</value></templateDef>"#,
        ),
        "value",
        "EMPTY",
    );
    // associationDef (.4).
    refused(
        &doc(r#"<associationDef><relation class="A" attribute="b"/></associationDef>"#),
        "associationDef",
        "exactly two",
    );
    refused(
        &doc(
            r#"<associationDef><relation class="A"/><relation class="B" attribute="a"/></associationDef>"#,
        ),
        "relation",
        "attribute",
    );
    refused(
        &doc(
            r#"<associationDef><end class="A" attribute="b"/><relation class="B" attribute="a"/></associationDef>"#,
        ),
        "end",
        "relation elements only",
    );
    // compoRelationDef (.5).
    let t = tags(&doc(
        r#"<compoRelationDef class="Radar" attribute="tracks"/>"#,
    ));
    assert_eq!(t.compositions[0].attribute, "tracks");
    refused(
        &doc(r#"<compoRelationDef attribute="tracks"/>"#),
        "compoRelationDef",
        "class",
    );
    // classMapping (.6), order mainTopic?, extensionTopic?, members.
    refused(
        &doc("<classMapping/>"),
        "classMapping",
        "required attribute name",
    );
    refused(
        &class(&format!(
            r#"<monoAttribute name="x"><valueField>X</valueField></monoAttribute><mainTopic name="T">{SIMPLE}</mainTopic>"#
        )),
        "mainTopic",
        "comes before the attributes",
    );
    refused(
        &class(&format!(
            r#"<extensionTopic name="E">{SIMPLE}</extensionTopic><mainTopic name="T">{SIMPLE}</mainTopic>"#
        )),
        "mainTopic",
        "mainTopic before extensionTopic",
    );
    refused(
        &class("<attribute name=\"x\"/>"),
        "attribute",
        "not an element of classMapping",
    );
    // mainTopic (.7) with typename, keyDescription (.8).
    let t = tags(&class(&format!(
        r#"<mainTopic name="TRACK-TOPIC" typename="TrackType">{SIMPLE}</mainTopic>"#
    )));
    let main = t.class_mappings[0].main_topic.as_ref().unwrap();
    assert_eq!(main.type_name.as_deref(), Some("TrackType"));
    assert_eq!(main.key.content, KeyContent::SimpleOid);
    refused(
        &class(r#"<mainTopic name="T"/>"#),
        "mainTopic",
        "needs a keyDescription",
    );
    refused(
        &class(&format!(
            r#"<mainTopic name="T">{SIMPLE}{SIMPLE}</mainTopic>"#
        )),
        "keyDescription",
        "holds one keyDescription",
    );
    refused(
        &class(&format!(r#"<mainTopic>{SIMPLE}</mainTopic>"#)),
        "mainTopic",
        "required attribute name",
    );
    refused(
        &class(&format!(
            r#"<mainTopic name="T" indexField="I">{SIMPLE}</mainTopic>"#
        )),
        "mainTopic",
        "no attribute indexField",
    );
    refused(
        &class(
            r#"<mainTopic name="T"><keyDescription content="SimpleOID"><keyField>OID</keyField></keyDescription></mainTopic>"#,
        ),
        "keyDescription",
        "content SimpleOID",
    );
    refused(
        &class(
            r#"<mainTopic name="T"><keyDescription content="FullOid"><keyField>OID</keyField></keyDescription></mainTopic>"#,
        ),
        "keyDescription",
        "needs two keyField",
    );
    refused(
        &class(
            r#"<mainTopic name="T"><keyDescription content="SimpleOid"><keyField>A</keyField><keyField>B</keyField></keyDescription></mainTopic>"#,
        ),
        "keyDescription",
        "needs one keyField",
    );
    refused(
        &class(
            r#"<mainTopic name="T"><keyDescription content="NoOid"></keyDescription></mainTopic>"#,
        ),
        "keyDescription",
        "at least one keyField",
    );
    refused(
        &class(
            r#"<mainTopic name="T"><keyDescription content="NoOid"><field>A</field></keyDescription></mainTopic>"#,
        ),
        "field",
        "keyField elements only",
    );
    let t = tags(&class(
        r#"<mainTopic name="T"><keyDescription content="NoOid"><keyField>A</keyField><keyField>B</keyField><keyField>C</keyField></keyDescription></mainTopic>"#,
    ));
    assert_eq!(
        t.class_mappings[0].main_topic.as_ref().unwrap().key.fields,
        ["A", "B", "C"]
    );
    // extensionTopic (.9).
    refused(
        &class(r#"<extensionTopic name="E"></extensionTopic>"#),
        "extensionTopic",
        "needs a keyDescription",
    );
    // monoAttribute (.10).
    refused(
        &class(r#"<monoAttribute name="x"/>"#),
        "monoAttribute",
        "at least one valueField",
    );
    refused(
        &class(&format!(
            r#"<monoAttribute name="x"><placeTopic name="P">{SIMPLE}</placeTopic></monoAttribute>"#
        )),
        "monoAttribute",
        "at least one valueField",
    );
    refused(
        &class(&format!(
            r#"<monoAttribute name="x"><valueField>X</valueField><placeTopic name="P">{SIMPLE}</placeTopic></monoAttribute>"#
        )),
        "placeTopic",
        "where valueField is required",
    );
    refused(
        &class(r#"<monoAttribute><valueField>X</valueField></monoAttribute>"#),
        "monoAttribute",
        "required attribute name",
    );
    // multiAttribute (.11).
    refused(
        &class(r#"<multiAttribute name="m"><valueField>V</valueField></multiAttribute>"#),
        "multiAttribute",
        "multiPlaceTopic first",
    );
    refused(
        &class(&format!(
            r#"<multiAttribute name="m"><multiPlaceTopic name="M" indexField="I">{SIMPLE}</multiPlaceTopic></multiAttribute>"#
        )),
        "multiAttribute",
        "at least one valueField",
    );
    let t = tags(&class(&format!(
        r#"<multiAttribute name="m"><multiPlaceTopic name="M" typename="MT" indexField="I">{SIMPLE}</multiPlaceTopic><valueField>V1</valueField><valueField>V2</valueField></multiAttribute>"#
    )));
    match &t.class_mappings[0].members[0] {
        zerodds_dlrl::mapping::MemberMappingSpec::MultiAttribute {
            topic,
            value_fields,
            ..
        } => {
            assert_eq!(topic.index_field.as_deref(), Some("I"));
            assert_eq!(topic.type_name.as_deref(), Some("MT"));
            assert_eq!(value_fields, &["V1", "V2"]);
        }
        other => panic!("{other:?}"),
    }
    // monoRelation (.12).
    refused(
        &class(r#"<monoRelation name="r"/>"#),
        "monoRelation",
        "needs a keyDescription",
    );
    refused(
        &class(&format!(
            r#"<monoRelation name="r">{SIMPLE}{SIMPLE}</monoRelation>"#
        )),
        "keyDescription",
        "ends with one keyDescription",
    );
    refused(
        &class(r#"<monoRelation name="r"><valueField>V</valueField></monoRelation>"#),
        "valueField",
        "where keyDescription is required",
    );
    // multiRelation (.13).
    refused(
        &class(&format!(
            r#"<multiRelation name="r">{SIMPLE}</multiRelation>"#
        )),
        "multiRelation",
        "a multiPlaceTopic and a keyDescription",
    );
    refused(
        &class(&format!(
            r#"<multiRelation name="r"><placeTopic name="P">{SIMPLE}</placeTopic>{SIMPLE}</multiRelation>"#
        )),
        "placeTopic",
        "multiPlaceTopic first",
    );
    // local (.14).
    refused(
        &class(r#"<local name="w"><valueField>W</valueField></local>"#),
        "valueField",
        "EMPTY",
    );
    refused(&class(r#"<local/>"#), "local", "required attribute name");
    // Another encoding.
    refused(
        r#"<?xml version="1.0" encoding="UTF-16"?><Dlrl/>"#,
        "",
        "encoding",
    );
}

/// §8.2.2.3.2.2, §8.2.2.3.2.5, §8.2.2.1: enumDef codes the enumeration by
/// its strings, compoRelationDef makes a composition, and what does not
/// resolve against the description is refused.
#[test]
fn dlrl_8_2_2_1_tags_apply_to_the_description_and_unresolved_names_are_refused() {
    let week = EnumerationType::new("Day", &["MON", "TUE"]);
    let desc = ModelDescription {
        valuetypes: vec![
            ValuetypeDecl {
                name: "Plan".into(),
                bases: vec!["DDS::ObjectRoot".into()],
                members: vec![
                    member("day", TypeRef::Enumeration(week.clone())),
                    member("steps", TypeRef::Valuetype("StepSet".into())),
                    member("days", TypeRef::Valuetype("DayList".into())),
                ],
            },
            ValuetypeDecl {
                name: "Step".into(),
                bases: vec!["DDS::ObjectRoot".into()],
                members: vec![member("n", TypeRef::Basic(BasicType::Long))],
            },
        ],
        types: vec![SimpleType::Enumeration(week)],
    };
    let xml = doc(
        r#"<enumDef name="Day"><value>Monday</value><value>Tuesday</value></enumDef>
           <templateDef name="StepSet" pattern="Set" itemType="Step"/>
           <templateDef name="DayList" pattern="List" itemType="Day"/>
           <compoRelationDef class="Plan" attribute="steps"/>"#,
    );
    let m = generate(&desc, &tags(&xml)).unwrap();
    match m.model().member("Plan", "day") {
        Some((_, Member::Attribute(a))) => match &a.ty {
            zerodds_dlrl::model::AttributeType::Mono(SimpleType::Enumeration(e)) => {
                assert_eq!(
                    e.coding,
                    EnumCoding::Names(vec!["Monday".into(), "Tuesday".into()])
                );
            }
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
    match m.model().member("Plan", "days") {
        Some((_, Member::Attribute(a))) => match &a.ty {
            zerodds_dlrl::model::AttributeType::Multi {
                basis: CollectionBasis::List,
                item: SimpleType::Enumeration(e),
            } => assert!(matches!(e.coding, EnumCoding::Names(_))),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
    match m.model().member("Plan", "steps") {
        Some((_, Member::Relation(r))) => {
            assert!(r.is_composition);
            assert_eq!(r.multiplicity, Multiplicity::ToMany(CollectionBasis::Set));
        }
        other => panic!("{other:?}"),
    }

    let unresolved = |xml: &str, what: &str| match generate(&desc, &tags(xml)) {
        Err(GenerationError::Unresolved(m)) => assert!(m.contains(what), "{m}"),
        other => panic!("{xml}: {other:?}"),
    };
    let both = r#"<templateDef name="StepSet" pattern="Set" itemType="Step"/>
                  <templateDef name="DayList" pattern="List" itemType="Day"/>"#;
    // A forward valuetype without templateDef.
    unresolved(
        &doc(r#"<templateDef name="DayList" pattern="List" itemType="Day"/>"#),
        "StepSet",
    );
    // An itemType that is nothing of the description.
    unresolved(
        &doc(
            r#"<templateDef name="StepSet" pattern="Set" itemType="Walk"/><templateDef name="DayList" pattern="List" itemType="Day"/>"#,
        ),
        "itemType Walk",
    );
    // A templateDef no member uses: its itemType is resolved all the
    // same, and the unused templateDef is refused like an unused enumDef.
    unresolved(
        &doc(&format!(
            r#"{both}<templateDef name="NopeList" pattern="List" itemType="Nope"/>"#
        )),
        "itemType Nope",
    );
    unresolved(
        &doc(&format!(
            r#"{both}<templateDef name="StepList" pattern="List" itemType="Step"/>"#
        )),
        "templateDef StepList: no member",
    );
    // An enumDef with another number of values, or for no enumeration.
    unresolved(
        &doc(&format!(
            r#"{both}<enumDef name="Day"><value>Monday</value></enumDef>"#
        )),
        "gives 1 values",
    );
    unresolved(
        &doc(&format!(
            r#"{both}<enumDef name="Month"><value>Jan</value></enumDef>"#
        )),
        "enumDef Month",
    );
    // local, compoRelationDef and associationDef that name no member.
    unresolved(
        &doc(&format!(
            r#"{both}<classMapping name="Plan"><local name="zz"/></classMapping>"#
        )),
        "local zz",
    );
    unresolved(
        &doc(&format!(
            r#"{both}<compoRelationDef class="Plan" attribute="day"/>"#
        )),
        "day is no relation",
    );
    unresolved(
        &doc(&format!(
            r#"{both}<associationDef><relation class="Plan" attribute="steps"/><relation class="Nope" attribute="x"/></associationDef>"#
        )),
        "no class Nope",
    );
    // A templateDef for a valuetype with a body.
    unresolved(
        &doc(&format!(
            r#"{both}<templateDef name="Step" pattern="List" itemType="long"/>"#
        )),
        "templateDef Step",
    );
    // A classMapping for a class the description does not have.
    assert!(matches!(
        generate(
            &desc,
            &tags(&doc(&format!(r#"{both}<classMapping name="Ghost"/>"#)))
        ),
        Err(GenerationError::Mapping(_))
    ));
}
