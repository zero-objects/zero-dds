// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! DDS-XML documents measured against the XSD schemas, by `xmllint`.
//!
//! Everything else in this crate's suite asks whether *our* parser
//! accepts a document. That is a closed loop: the parsers and the tests
//! were written from the same reading of the specification, so a
//! misreading passes both. This file hands the question to a second
//! implementation — `xmllint` from libxml2 — and asks the schemas.
//!
//! Two directions, and the second is the one that catches the damage.
//!
//! 1. **What we write validates.** The crate has exactly one DDS-XML
//!    writer, [`serialize_sample`] (§7.3.7), and seventeen XSD files under
//!    `crates/xml/schemas/` that it publishes as `zerodds_xml::schemas`.
//!    Both are output: a document we emit and a schema someone else
//!    validates against. Every schema must compile, and the writer's
//!    output must validate.
//! 2. **What the schema rejects, we reject.** A parser that accepts more
//!    than the schema takes in documents no other DDS stack reads, and
//!    says nothing where an error belongs. [`SCHEMA_INVALID`] holds
//!    documents that `xmllint` rejects; each one records whether this
//!    crate rejects it too, and where it does not, *why*.
//!
//! Tolerance is fine. Unnoticed tolerance is not — the `Verdict::Accept`
//! rows below are the full list of places where this crate reads a
//! document its own schema turns down, each with the reason.
//!
//! # The gates are loud
//!
//! * `xmllint` is found by running it, not by testing a name against
//!   `PATH`. Absent, every test here prints `[skip]` and returns.
//!   Present, [`xmllint_is_a_working_oracle`] proves it says *no* to a
//!   document that violates the schema before any other test trusts its
//!   *yes*. A validator that cannot fail validates nothing.
//! * Exit status is read from the process, never through a pipe. A
//!   pipeline reports the status of its last stage, and this tree has
//!   twice shipped a check that passed because a helper swallowed one.
//! * The OMG's five example documents are copyright and not in the tree;
//!   `ZERODDS_OMG_DDSXML_DIR` points at them. Unset, those tests print
//!   `[skip]`. Set but incomplete, they panic with the path.

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_xml::{
    PrimitiveType, SampleValue, StructMember, StructType, TypeDef, TypeRef,
    parse_application_libraries, parse_dds_xml, parse_domain_libraries,
    parse_domain_participant_libraries, parse_qos_libraries, parse_sample, parse_type_libraries,
    serialize_sample,
};

// ---------------------------------------------------------------------
// The validator
// ---------------------------------------------------------------------

/// Locates `xmllint`, or `None`.
///
/// `ZERODDS_XMLLINT` overrides the program. The probe **runs** the
/// candidate: a check that looks for a name on `PATH` reports success
/// for a program that is not there under a different name, and reports
/// failure for one that is there under a path it did not search.
fn xmllint() -> Option<String> {
    let prog = std::env::var("ZERODDS_XMLLINT").unwrap_or_else(|_| "xmllint".into());
    match Command::new(&prog).arg("--version").output() {
        Ok(out) if out.status.success() => Some(prog),
        _ => None,
    }
}

macro_rules! xmllint_or_skip {
    () => {
        match xmllint() {
            Some(p) => p,
            None => {
                eprintln!("[skip] xmllint not runnable (set ZERODDS_XMLLINT to override)");
                return;
            }
        }
    };
}

/// Verdict of the validator on one document.
#[derive(Debug, PartialEq, Eq)]
enum Lint {
    Valid,
    Invalid(String),
}

/// Runs `xmllint --schema`, reading the process exit status directly.
///
/// `--nonet` keeps the run off the network: `xsi:schemaLocation` in the
/// OMG's documents points at `www.omg.org`, and a validation that
/// quietly fetched a schema from the internet would be measuring
/// something other than the file under test.
fn validate(prog: &str, schema: &Path, doc: &Path, catalog: Option<&Path>) -> Lint {
    let mut cmd = Command::new(prog);
    cmd.arg("--nonet")
        .arg("--noout")
        .arg("--schema")
        .arg(schema)
        .arg(doc);
    // The schemas of the specification include each other by absolute
    // `http://www.omg.org/spec/DDS-XML/20180501/` URL. A catalog maps that
    // prefix onto a directory, so the run stays offline: the in-tree
    // `schemas/` unless the caller names another one.
    let default_catalog;
    let catalog = match catalog {
        Some(c) => c,
        None => {
            default_catalog = omg_catalog(&schemas());
            &default_catalog
        }
    };
    cmd.env("XML_CATALOG_FILES", catalog);
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("running {prog} on {}: {e}", doc.display()));
    if out.status.success() {
        Lint::Valid
    } else {
        Lint::Invalid(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Writes `xml` into a scratch file and validates it.
fn validate_str(prog: &str, schema: &Path, name: &str, xml: &str) -> Lint {
    let dir = scratch();
    let path = dir.join(name);
    std::fs::write(&path, xml).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
    validate(prog, schema, &path, None)
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join("zerodds-xml-schema-validation");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn schemas() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas")
}

/// In-tree schema for a building block.
fn schema(file: &str) -> PathBuf {
    let p = schemas().join(file);
    assert!(p.is_file(), "schema missing: {}", p.display());
    p
}

// ---------------------------------------------------------------------
// Building blocks: schema, root element, parser entry point
// ---------------------------------------------------------------------

/// Which parser a document is fed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Qos,
    Types,
    Domains,
    Participants,
    Applications,
    DdsSystem,
}

impl Block {
    /// The in-tree XSD that carries this block's root element.
    fn in_tree_schema(self) -> PathBuf {
        schema(match self {
            Block::Qos => "dds-xml_qos_definitions.xsd",
            Block::Types => "dds-xml_type_definitions.xsd",
            Block::Domains => "dds-xml_domain_definitions.xsd",
            Block::Participants => "dds-xml_domainparticipant_definitions.xsd",
            Block::Applications => "dds-xml_application_definitions.xsd",
            Block::DdsSystem => "dds-xml_dds_system_definitions.xsd",
        })
    }

    /// The OMG's own XSD file name, as it sits next to the example
    /// documents in `ZERODDS_OMG_DDSXML_DIR`. Same spelling as
    /// [`Self::in_tree_schema`]: the in-tree files are named after the
    /// specification (§7.3.2.3, §7.3.4.3, §7.3.5.3, §7.3.6.3), and the
    /// content differs — see the header of each chameleon file.
    fn omg_schema(self) -> &'static str {
        match self {
            Block::Qos => "dds-xml_qos_definitions.xsd",
            Block::Types => "dds-xml_type_definitions.xsd",
            Block::Domains => "dds-xml_domain_definitions.xsd",
            Block::Participants => "dds-xml_domainparticipant_definitions.xsd",
            Block::Applications => "dds-xml_application_definitions.xsd",
            Block::DdsSystem => "dds-xml_dds_system_definitions.xsd",
        }
    }

    /// Feeds `xml` to this block's parser.
    fn parse(self, xml: &str) -> Result<(), String> {
        let r = match self {
            Block::Qos => parse_qos_libraries(xml).map(|_| ()),
            Block::Types => parse_type_libraries(xml).map(|_| ()),
            Block::Domains => parse_domain_libraries(xml).map(|_| ()),
            Block::Participants => parse_domain_participant_libraries(xml).map(|_| ()),
            Block::Applications => parse_application_libraries(xml).map(|_| ()),
            Block::DdsSystem => parse_dds_xml(xml).map(|_| ()),
        };
        r.map_err(|e| format!("{e:?}"))
    }
}

// ---------------------------------------------------------------------
// Direction 1 — what this crate writes
// ---------------------------------------------------------------------

/// The validator must be able to say no. Runs before anything trusts a
/// yes.
#[test]
fn xmllint_is_a_working_oracle() {
    let prog = xmllint_or_skip!();
    let s = schema("dds-xml_qos_definitions.xsd");

    let good = r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L">
        <qos_profile name="P"/>
    </qos_library>"#;
    assert_eq!(
        validate_str(&prog, &s, "oracle_good.xml", good),
        Lint::Valid,
        "xmllint rejects a document that satisfies the schema — the \
         validator or the schema is broken, and every other verdict in \
         this file is worthless until that is settled"
    );

    // `name` is use="required" on qosLibrary.
    let bad = r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML">
        <qos_profile name="P"/>
    </qos_library>"#;
    match validate_str(&prog, &s, "oracle_bad.xml", bad) {
        Lint::Invalid(_) => {}
        Lint::Valid => panic!(
            "xmllint validated a document with no name attribute on \
             <qos_library>. It is not validating: a run that cannot fail \
             turns every test below into a no-op"
        ),
    }

    // Not well-formed at all — the floor below schema validity.
    match validate_str(
        &prog,
        &s,
        "oracle_malformed.xml",
        "<qos_library name=\"L\">",
    ) {
        Lint::Invalid(_) => {}
        Lint::Valid => panic!("xmllint validated a document that is not well-formed XML"),
    }
}

/// Every published schema compiles. A schema that libxml2 cannot load is
/// not a schema anyone can validate against, and nothing in this tree
/// tried to load one until now.
#[test]
fn every_in_tree_schema_compiles() {
    let prog = xmllint_or_skip!();
    let mut checked = 0;
    for entry in std::fs::read_dir(schemas()).expect("schemas dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("xsd") {
            continue;
        }
        // Well-formedness of the schema document itself.
        let out = Command::new(&prog)
            .arg("--nonet")
            .arg("--noout")
            .arg(&path)
            .output()
            .expect("xmllint");
        assert!(
            out.status.success(),
            "{} is not well-formed XML:\n{}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        checked += 1;
    }
    assert_eq!(checked, 17, "expected seventeen XSD files under schemas/");

    // Compiling the schema is a separate step from parsing it: an
    // unresolved include or a duplicate type only shows up here. Every
    // namespaced schema is compiled by validating a document against it;
    // the chameleons are compiled along the way through xs:include.
    for block in [
        Block::Qos,
        Block::Types,
        Block::Domains,
        Block::Participants,
        Block::Applications,
        Block::DdsSystem,
    ] {
        let s = block.in_tree_schema();
        let lint = validate_str(
            &prog,
            &s,
            "compile_probe.xml",
            "<not_a_dds_xml_root xmlns=\"http://www.omg.org/spec/DDS-XML\"/>",
        );
        let Lint::Invalid(msg) = lint else {
            panic!(
                "{} accepts <not_a_dds_xml_root> as a document root",
                s.display()
            )
        };
        assert!(
            !msg.contains("Schemas parser error") && !msg.contains("failed to compile"),
            "{} does not compile:\n{msg}",
            s.display()
        );
    }
}

/// §7.3.7 — the one writer in this crate, measured against the schema
/// this crate publishes for it.
#[test]
fn serialize_sample_output_validates() {
    let prog = xmllint_or_skip!();

    let type_def = TypeDef::Struct(StructType {
        name: "Reading".into(),
        members: vec![
            StructMember {
                name: "id".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                ..Default::default()
            },
            StructMember {
                name: "label".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::String),
                ..Default::default()
            },
            StructMember {
                name: "samples".into(),
                type_ref: TypeRef::Primitive(PrimitiveType::Long),
                sequence_max_length: Some(8),
                ..Default::default()
            },
        ],
        ..Default::default()
    });

    let xml = parse_sample(
        r#"<sample type_ref="Reading">
             <id>7</id>
             <label>a &amp; b</label>
             <samples><item>1</item><item>2</item></samples>
           </sample>"#,
        &type_def,
        &[],
    )
    .expect("fixture parses");
    let written = serialize_sample(&xml, &type_def, &[]);

    // §7.3.7.3.2: sequence and array elements are named <item>.
    assert!(
        written.contains("<samples><item>1</item><item>2</item></samples>"),
        "sequence not written with <item>: {written}"
    );

    // The writer emits a document without the DDS-XML namespace; in the
    // namespace it validates against the namespaced schema, which holds
    // the roots (§7.3.1: a chameleon file has none).
    let namespaced = written.replacen(
        "<sample ",
        "<sample xmlns=\"http://www.omg.org/spec/DDS-XML\" ",
        1,
    );
    assert_eq!(
        validate_str(
            &prog,
            &schema("dds-xml_data_samples_definitions.xsd"),
            "written_sample_ns.xml",
            &namespaced,
        ),
        Lint::Valid,
        "namespaced serialize_sample output does not validate: {namespaced}"
    );

    // And it reads back as the value it was written from.
    let reparsed = parse_sample(&written, &type_def, &[]).expect("written sample re-parses");
    assert_eq!(reparsed, xml, "serialize_sample is not round-trip stable");
    assert!(matches!(reparsed, SampleValue::Struct(_)));
}

// ---------------------------------------------------------------------
// Direction 2 — what the schema rejects
// ---------------------------------------------------------------------

/// What this crate's parser does with a document the schema turns down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// The parser rejects it too.
    Reject,
    /// The parser reads it. The reason is deliberate and stated.
    Accept(&'static str),
}

/// A document the in-tree schema rejects.
struct Invalid {
    /// File name in the scratch dir; also the case label.
    name: &'static str,
    block: Block,
    /// The rule being violated, spelled out.
    rule: &'static str,
    xml: &'static str,
    verdict: Verdict,
}

/// Documents that violate the schema. The `Accept` rows are the complete
/// list of this crate's tolerance beyond its own schema.
const SCHEMA_INVALID: &[Invalid] = &[
    // --- required attributes (§7.3.2.4, §7.3.4.4.1, §7.3.5.4.1) ------
    Invalid {
        name: "qos_library_without_name",
        block: Block::Qos,
        rule: "qosLibrary/@name use=\"required\"",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML"><qos_profile name="P"/></qos_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "qos_profile_without_name",
        block: Block::Qos,
        rule: "qosProfile/@name use=\"required\"",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile/></qos_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "domain_without_name",
        block: Block::Domains,
        rule: "domainEntry/@name use=\"required\"",
        xml: r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain domain_id="0"/></domain_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "topic_without_register_type_ref",
        block: Block::Domains,
        rule: "topicEntry/@register_type_ref use=\"required\" — §7.3.4.4.1: \
               \"Topics shall refer to a registered type using the \
               register_type_ref XML attribute\"",
        xml: r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain name="D" domain_id="0"><topic name="T"/></domain></domain_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "participant_without_name",
        block: Block::Participants,
        rule: "domainParticipantEntry/@name use=\"required\"",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant domain_id="0"/></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "publisher_without_name",
        block: Block::Participants,
        rule: "publisherEntry/@name use=\"required\"",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="0"><publisher><data_writer name="W" topic_ref="T"/></publisher></domain_participant></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "data_writer_without_topic_ref",
        block: Block::Participants,
        rule: "dataWriterEntry/@topic_ref use=\"required\"",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="0"><publisher name="Pub"><data_writer name="W"/></publisher></domain_participant></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "application_without_name",
        block: Block::Applications,
        rule: "applicationEntry/@name use=\"required\"",
        xml: r#"<application_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><application><domain_participant name="P" domain_id="0"/></application></application_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "reliability_short_spelling",
        block: Block::Qos,
        rule: "7.1.4 Tab.7.1: an enum's legal values are the DCPS IDL \
               literals — RELIABLE is not one of ReliabilityQosPolicyKind",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><datawriter_qos><reliability><kind>RELIABLE</kind></reliability></datawriter_qos></qos_profile></qos_library>"#,
        verdict: Verdict::Reject,
    },
    // --- document roots (§7.3.2.3, §7.3.4.3, §7.3.5.3, §7.3.6.3) -----
    Invalid {
        name: "root_element_misspelled",
        block: Block::Qos,
        rule: "only the declared global element is a document root",
        xml: r#"<qos_libraries xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"/></qos_libraries>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "domain_library_root_for_participants",
        block: Block::Participants,
        rule: "a building block's document root is its own, not another block's",
        xml: r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain name="D" domain_id="0"/></domain_library>"#,
        verdict: Verdict::Reject,
    },
    // --- attribute types ---------------------------------------------
    Invalid {
        name: "domain_id_negative",
        block: Block::Domains,
        rule: "domainEntry/@domain_id is xs:nonNegativeInteger",
        xml: r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain name="D" domain_id="-1"/></domain_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "domain_id_not_a_number",
        block: Block::Domains,
        rule: "domainEntry/@domain_id is xs:nonNegativeInteger",
        xml: r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain name="D" domain_id="ShapesDomain"/></domain_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "participant_domain_id_negative",
        block: Block::Participants,
        rule: "domainParticipantEntry/@domain_id is xs:nonNegativeInteger",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="-3"/></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    // --- content models ----------------------------------------------
    Invalid {
        name: "unknown_element_in_qos_profile",
        block: Block::Qos,
        rule: "qosProfile is a closed choice of the six QoS containers",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><bogus_qos/></qos_profile></qos_library>"#,
        // A vendor element or a misspelling used to vanish without a
        // word. Refused now.
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "unknown_element_under_dds",
        block: Block::DdsSystem,
        rule: "ddsSystem is a closed choice of the five libraries",
        xml: r#"<dds xmlns="http://www.omg.org/spec/DDS-XML"><bogus_library name="L"/></dds>"#,
        // Same one level up: a skipped block is a section of the
        // document that did not load.
        verdict: Verdict::Reject,
    },
    // --- attributes the grammar does not have ------------------------
    Invalid {
        name: "participant_by_ref_attribute",
        block: Block::Applications,
        rule: "there is no ref attribute in this grammar — §7.3.6 types an \
               <application>'s children as full domainParticipant definitions",
        xml: r#"<application_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><application name="A"><domain_participant ref="Lib::P"/></application></application_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "data_writer_by_ref_attribute",
        block: Block::Participants,
        rule: "dataWriterEntry has name and topic_ref, no ref",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="0"><publisher name="Pub"><data_writer ref="W" topic_ref="T"/></publisher></domain_participant></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "topic_by_ref_attribute",
        block: Block::Participants,
        rule: "topicEntry has name and register_type_ref, no ref",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="0"><topic ref="Square"/></domain_participant></domain_participant_library>"#,
        // A participant's <topic> is a definition (§7.3.5.4.2 plus the
        // Domain building block's topicEntry), so without `name` the
        // document is refused rather than read as a selection.
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "is_default_qos_attribute",
        block: Block::Qos,
        rule: "qosProfile has name and base_name; is_default_qos is in no OMG schema",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P" is_default_qos="true"/></qos_library>"#,
        // A vendor attribute (RTI). Unknown attributes are refused like
        // unknown elements; `ReadOptions::report_and_skip` leaves them out
        // and lists them.
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "topic_filter_on_profile",
        block: Block::Qos,
        rule: "§7.3.2.4.3 puts topic_filter on the QoS container, not on the profile",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P" topic_filter="A*"/></qos_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "data_reader_under_publisher",
        block: Block::Participants,
        rule: "publisher holds data_writer and publisher_qos; a data_reader belongs to a \
               subscriber (§7.3.5.4.1; the example of §7.3.6.4.1.1 has it under a publisher)",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="0"><publisher name="Pub"><data_reader name="R" topic_ref="T"/></publisher></domain_participant></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "data_reader_without_topic_ref",
        block: Block::Participants,
        rule: "dataReader/@topic_ref use=\"required\" (the example of §7.3.5.4.1.1 omits it)",
        xml: r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="0"><subscriber name="Sub"><data_reader name="R"/></subscriber></domain_participant></domain_participant_library>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "domain_with_base_name",
        block: Block::Domains,
        rule: "the schema's domain has name and domain_id; §7.3.4.4.2's base_name is not in it",
        xml: r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain name="B" domain_id="7"><topic name="T" register_type_ref="R"/></domain><domain name="D" base_name="B"><topic name="T2" register_type_ref="R"/></domain></domain_library>"#,
        verdict: Verdict::Accept(
            "§7.3.4.4.2 of the text defines domain inheritance through \
             base_name and writes `<domain name=\"MyDomain\" \
             base_name=\"BaseDomain\">`; the schema of the same \
             specification declares no such attribute. The text is read.",
        ),
    },
    Invalid {
        name: "topic_filter_as_child_element",
        block: Block::Qos,
        rule: "§7.3.2.4.3 spells the filter as an attribute; there is no \
               <topic_filter> element in the grammar",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><topic_filter>A*</topic_filter></qos_profile></qos_library>"#,
        verdict: Verdict::Reject,
    },
    // --- Types (XTypes 1.3 Annex A) ----------------------------------
    Invalid {
        name: "member_without_type",
        block: Block::Types,
        rule: "memberDecl/@type use=\"required\" (XTypes 1.3 Annex A)",
        xml: r#"<types xmlns="http://www.omg.org/spec/DDS-XML"><struct name="S"><member name="m"/></struct></types>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "idl_spelling_of_a_primitive",
        block: Block::Types,
        rule: "allTypeKind enumerates int32, not the IDL spelling long \
               (XTypes 1.3 Annex A)",
        xml: r#"<types xmlns="http://www.omg.org/spec/DDS-XML"><struct name="S"><member name="m" type="long"/></struct></types>"#,
        verdict: Verdict::Reject,
    },
    // --- name syntax (§7.1.4 / elementName) --------------------------
    Invalid {
        name: "library_name_with_a_space",
        block: Block::Qos,
        rule: "elementName pattern ((::)?[a-zA-Z0-9_.])+ — no whitespace, \
               so that every name survives a library::name reference",
        xml: r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="My Lib"><qos_profile name="P"/></qos_library>"#,
        verdict: Verdict::Accept(
            "Names are read as written. The pattern also excludes \"-\", \
             which this workspace itself uses (crates/rpc/src/qos_profile \
             .rs names a profile \"best-effort\"), so enforcing it would \
             reject documents this project writes. What a name may \
             contain is a question for whoever resolves it, and \
             resolver.rs splits on \"::\" only.",
        ),
    },
];

/// The rows above really are schema-invalid — asked of `xmllint`, not
/// asserted by us — and this crate's answer to each is the one recorded.
#[test]
fn schema_invalid_documents() {
    let prog = xmllint_or_skip!();
    let mut tolerated = 0;
    for case in SCHEMA_INVALID {
        let schema = case.block.in_tree_schema();
        let lint = validate_str(&prog, &schema, &format!("{}.xml", case.name), case.xml);
        assert!(
            matches!(lint, Lint::Invalid(_)),
            "case `{}` is meant to violate {} but {} accepts it — the case \
             or the schema is wrong, and a negative test that is not \
             negative proves nothing",
            case.name,
            case.rule,
            schema.display()
        );

        match (case.block.parse(case.xml), case.verdict) {
            (Err(_), Verdict::Reject) => {}
            (Ok(()), Verdict::Accept(_)) => tolerated += 1,
            (Ok(()), Verdict::Reject) => panic!(
                "case `{}`: the schema rejects this document ({}), this \
                 crate reads it, and no reason is recorded. Either reject \
                 it or write down why it is tolerated.",
                case.name, case.rule
            ),
            (Err(e), Verdict::Accept(reason)) => panic!(
                "case `{}` is recorded as tolerated — {reason} — but the \
                 parser rejects it: {e}",
                case.name
            ),
        }
    }
    // The count is pinned so that a new tolerance cannot slip in as an
    // edit to a table nobody counts.
    assert_eq!(
        tolerated, 2,
        "the number of documents this crate reads against its own schema \
         changed; every one of them needs a stated reason"
    );
}

/// Documents the normative schemas define. Each is valid against the
/// schema (`xmllint`) and the reader reads it; `tests/normative_forms.rs`
/// checks what it makes of them.
const XSD_VALID: &[(&str, Block, &str)] = &[
    (
        "resource_limits_initial",
        Block::Qos,
        r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><datawriter_qos topic_filter="B*"><resource_limits><max_samples>128</max_samples><max_samples_per_instance>128</max_samples_per_instance><initial_samples>128</initial_samples><max_instances>1</max_instances><initial_instances>1</initial_instances></resource_limits></datawriter_qos></qos_profile></qos_library>"#,
    ),
    (
        "empty_numeric_elements",
        Block::Qos,
        r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><datawriter_qos><deadline><period><sec></sec><nanosec/></period></deadline><history><depth>3</depth></history></datawriter_qos></qos_profile></qos_library>"#,
    ),
    (
        "user_data_base64_value",
        Block::Qos,
        r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><domain_participant_qos><user_data><value>aGk=</value></user_data></domain_participant_qos><publisher_qos><partition><name></name></partition><group_data><value></value></group_data></publisher_qos></qos_profile></qos_library>"#,
    ),
    (
        "unnamed_container_in_library",
        Block::Qos,
        r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><topic_qos><durability/></topic_qos></qos_library>"#,
    ),
    (
        "container_base_name_in_profile",
        Block::Qos,
        r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="B"/><qos_profile name="D"><datawriter_qos base_name="B" name="n"/></qos_profile></qos_library>"#,
    ),
    (
        "repeated_subscriber_qos",
        Block::Qos,
        r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><qos_profile name="P"><subscriber_qos/><subscriber_qos topic_filter="A*"/></qos_profile></qos_library>"#,
    ),
    (
        "registered_name_and_format",
        Block::Domains,
        r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="DL"><domain name="D" domain_id="1"><register_type name="R" type_ref="T" format="x"><registered_name>RT</registered_name></register_type><topic name="Tp" register_type_ref="R"><registered_name>A</registered_name><topic_qos base_name="Q"/></topic></domain></domain_library>"#,
    ),
    (
        "reader_filters_and_conditions",
        Block::Participants,
        r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_id="1"><subscriber name="S"><data_reader name="R" topic_ref="T"><content_filter name="f" kind="builtin.sql"><expression>x &gt; %0</expression><parameter_list><param>5</param></parameter_list></content_filter><status_condition><status>DATA_AVAILABLE</status></status_condition><read_condition name="rc"><sample_state>NOT_READ</sample_state></read_condition><query_condition name="q" kind="builtin.stringMatch"/></data_reader></subscriber></domain_participant></domain_participant_library>"#,
    ),
    (
        "participant_qos_after_entities",
        Block::Participants,
        r#"<domain_participant_library xmlns="http://www.omg.org/spec/DDS-XML" name="L"><domain_participant name="P" domain_ref="X::Y" domain_id="127"><publisher name="Pub"><data_writer name="W" topic_ref="T"/><publisher_qos><entity_factory/></publisher_qos></publisher><subscriber name="S"><data_reader name="R" topic_ref="T"/><subscriber_qos base_name="L::Q"/></subscriber><domain_participant_qos><entity_factory/></domain_participant_qos></domain_participant></domain_participant_library>"#,
    ),
    (
        "application_participant_inherits",
        Block::Applications,
        r#"<application_library xmlns="http://www.omg.org/spec/DDS-XML" name="A"><application name="X"><domain_participant name="G" base_name="Lib::G" domain_id="64"/></application></application_library>"#,
    ),
];

/// The reader reads what the schema defines.
#[test]
fn xsd_valid_documents_are_read() {
    let prog = xmllint_or_skip!();
    for (name, block, xml) in XSD_VALID {
        let lint = validate_str(&prog, &block.in_tree_schema(), &format!("{name}.xml"), xml);
        assert_eq!(
            lint,
            Lint::Valid,
            "`{name}` is meant to be valid against the schema"
        );
        if let Err(e) = block.parse(xml) {
            panic!("`{name}` is valid against the schema and the reader refuses it: {e}");
        }
    }
}

/// The other asymmetry: documents the schema accepts and this crate
/// turns down. Being stricter is safe, but it is not free, so it is
/// written down too.
#[test]
fn stricter_than_the_schema() {
    let prog = xmllint_or_skip!();

    // The Types schema types the bound attributes as xs:string (XTypes
    // 1.3 Annex A), because a bound may be a constant expression; whether
    // a name in it is declared is not a question XSD can ask. A bound
    // naming a constant that no <const> declares is valid against that
    // schema and an error here — dropping the bound would silently unbound
    // a bounded string.
    let unknown_bound = r#"<types xmlns="http://www.omg.org/spec/DDS-XML">
        <struct name="S"><member name="c" type="string" stringMaxLength="NO_SUCH_CONST"/></struct>
    </types>"#;
    assert_eq!(
        validate_str(
            &prog,
            &Block::Types.in_tree_schema(),
            "unknown_bound.xml",
            unknown_bound
        ),
        Lint::Valid,
        "the Types schema takes any string as a bound"
    );
    assert!(
        Block::Types.parse(unknown_bound).is_err(),
        "an unresolvable stringMaxLength must be an error, not a dropped bound"
    );

    // A bound that names a declared <const> is read — stringMaxLength is
    // typed xs:string in the schema precisely so that it may name one,
    // and reading it as an integer only would reject the OMG's own
    // dds-xml_dds_system_example.xml.
    let symbolic_bound = r#"<types xmlns="http://www.omg.org/spec/DDS-XML">
        <const name="MAX" type="int32" value="128"/>
        <struct name="S"><member name="c" type="string" stringMaxLength="MAX"/></struct>
    </types>"#;
    assert!(
        Block::Types.parse(symbolic_bound).is_ok(),
        "a symbolic bound naming a declared const must be read"
    );

    // §7.3.2.4.3 repeats a DataWriter/DataReader/Topic QoS container to
    // select by topic filter. The schema allows the repetition, and so
    // does the parser: both containers are kept, each with its own
    // `topic_filter`.
    let repeated = r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L">
        <qos_profile name="P">
            <datawriter_qos topic_filter="A*"><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>
            <datawriter_qos topic_filter="B*"><history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>1</depth></history></datawriter_qos>
        </qos_profile>
    </qos_library>"#;
    assert_eq!(
        validate_str(
            &prog,
            &Block::Qos.in_tree_schema(),
            "repeated_container.xml",
            repeated
        ),
        Lint::Valid,
        "the schema allows repeated QoS containers (§7.3.2.4.3)"
    );
    assert!(
        Block::Qos.parse(repeated).is_ok(),
        "repeated datawriter_qos containers with a topic_filter parse (§7.3.2.4.3)"
    );

    // The schema's container choice is unbounded for every kind, so a
    // second publisher_qos is valid, and the parser keeps both: containers
    // with the same (here: no) filter merge, the later policies overriding.
    let repeated_publisher = r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L">
        <qos_profile name="P">
            <publisher_qos><partition><name><element>a</element></name></partition></publisher_qos>
            <publisher_qos><entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory></publisher_qos>
        </qos_profile>
    </qos_library>"#;
    assert_eq!(
        validate_str(
            &prog,
            &Block::Qos.in_tree_schema(),
            "repeated_publisher.xml",
            repeated_publisher
        ),
        Lint::Valid,
        "the schema's container choice is unbounded"
    );
    let libs = parse_qos_libraries(repeated_publisher).expect("repeated publisher_qos");
    let publisher = &libs[0].profiles[0].publisher_qos;
    assert_eq!(publisher.len(), 1, "same filter: one merged container");
    assert!(publisher[0].qos.partition.is_some());
    assert!(publisher[0].qos.entity_factory.is_some());
}

/// §7.3.2.4.3 in its schema form: the filter as an attribute on the QoS
/// container. It used to fall through the lax branch and vanish.
#[test]
fn topic_filter_attribute_is_read() {
    let xml = r#"<qos_library xmlns="http://www.omg.org/spec/DDS-XML" name="L">
        <qos_profile name="P">
            <datawriter_qos topic_filter="Sensor*">
                <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
            </datawriter_qos>
        </qos_profile>
    </qos_library>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let p = &libs[0].profiles[0];
    // §7.3.2.4.3 writes the filter as an attribute on the QoS
    // container; it is the container's, not the profile's.
    assert!(p.topic_filter.is_none());
    let dw = p.datawriter_qos.first().expect("dw");
    assert_eq!(dw.topic_filter.as_deref(), Some("Sensor*"));
    assert!(dw.qos.reliability.is_some());
}

// ---------------------------------------------------------------------
// The OMG's own documents
// ---------------------------------------------------------------------

fn corpus() -> Option<PathBuf> {
    std::env::var("ZERODDS_OMG_DDSXML_DIR")
        .ok()
        .map(PathBuf::from)
}

macro_rules! corpus_or_skip {
    () => {
        match corpus() {
            Some(d) => d,
            None => {
                eprintln!("[skip] ZERODDS_OMG_DDSXML_DIR not set");
                return;
            }
        }
    };
}

/// The five example documents and the block each belongs to.
const OMG_EXAMPLES: &[(&str, Block)] = &[
    ("dds-xml_qos_example.xml", Block::Qos),
    ("dds-xml_domains_example.xml", Block::Domains),
    (
        "dds-xml_domainparticipants_example.xml",
        Block::Participants,
    ),
    ("dds-xml_applications_example.xml", Block::Applications),
    ("dds-xml_dds_system_example.xml", Block::DdsSystem),
];

/// Writes the XML catalog that maps the OMG's absolute
/// `http://www.omg.org/spec/DDS-XML/20180501/` schema URLs onto the
/// corpus directory.
///
/// Without it every OMG schema fails to compile under `--nonet`, and a
/// test that only asks "did xmllint say no?" would pass on a schema that
/// never loaded. Callers must therefore also check *why* xmllint said
/// no — see [`assert_document_invalid`].
fn omg_catalog(dir: &Path) -> PathBuf {
    let path = scratch().join("omg-catalog.xml");
    let catalog = format!(
        r#"<?xml version="1.0"?>
<catalog xmlns="urn:oasis:names:tc:entity:xmlns:xml:catalog">
  <rewriteURI uriStartString="http://www.omg.org/spec/DDS-XML/20180501/" rewritePrefix="{dir}/"/>
  <rewriteSystem systemIdStartString="http://www.omg.org/spec/DDS-XML/20180501/" rewritePrefix="{dir}/"/>
</catalog>
"#,
        dir = dir.display()
    );
    std::fs::write(&path, catalog).expect("catalog");
    path
}

/// Asserts that xmllint rejected the *document*, not the schema.
///
/// "Schema failed to compile" and "document is invalid" are both a
/// non-zero exit status, and treating them as the same thing is how a
/// negative test comes to pass without ever loading a grammar.
fn assert_document_invalid(lint: &Lint, what: &str) {
    match lint {
        Lint::Valid => panic!("{what}: accepted, expected a schema violation"),
        Lint::Invalid(msg) => assert!(
            !msg.contains("Schemas parser error")
                && !msg.contains("failed to compile")
                && !msg.contains("failed to load"),
            "{what}: xmllint failed on the schema, not on the document — \
             this verdict says nothing about the document:\n{msg}"
        ),
    }
}

/// A corpus that is half there fails with the path; it never skips.
fn example_path(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    assert!(
        p.is_file(),
        "ZERODDS_OMG_DDSXML_DIR is set but {} is not there",
        p.display()
    );
    p
}

/// The schemas this tree publishes must accept the specification's own
/// documents. They did not: every chameleon carried
/// `elementFormDefault="unqualified"`, so a document that puts its
/// elements in the DDS-XML namespace — which all five do, and which is
/// the point of the namespaced schema — failed on its first child
/// element.
#[test]
fn omg_examples_validate_against_in_tree_schemas() {
    let prog = xmllint_or_skip!();
    let dir = corpus_or_skip!();
    for (file, block) in OMG_EXAMPLES {
        let doc = example_path(&dir, file);
        let schema = block.in_tree_schema();
        match validate(&prog, &schema, &doc, None) {
            Lint::Valid => {}
            Lint::Invalid(msg) => panic!(
                "{file} does not validate against {}:\n{msg}",
                schema.display()
            ),
        }
    }
}

/// The same five documents against the OMG's own XSD files, which sit
/// next to them in the corpus directory. This is the outside check: not
/// our schema, not our parser.
///
/// Those files `xs:include` each other by absolute `http://www.omg.org`
/// URL. A catalog maps the URL prefix onto the corpus directory, so the
/// run stays offline (`--nonet`) and validates against the files on
/// disk.
#[test]
fn omg_examples_validate_against_omg_schemas() {
    let prog = xmllint_or_skip!();
    let dir = corpus_or_skip!();

    let catalog_path = omg_catalog(&dir);

    for (file, block) in OMG_EXAMPLES {
        let doc = example_path(&dir, file);
        let schema = example_path(&dir, block.omg_schema());
        match validate(&prog, &schema, &doc, Some(&catalog_path)) {
            Lint::Valid => {}
            Lint::Invalid(msg) => panic!(
                "{file} does not validate against the OMG's own {}:\n{msg}\n\
                 (if every file failed on an unresolved include, the \
                 catalog did not take and this test measured nothing)",
                schema.display()
            ),
        }
    }
}

/// The in-tree schemas and the OMG's must agree on the same documents.
/// Where they do not, the divergence is named here rather than left to
/// be discovered by whoever validates our output with their schema.
#[test]
fn in_tree_and_omg_schemas_agree_on_the_negative_cases() {
    let prog = xmllint_or_skip!();
    let dir = corpus_or_skip!();

    // The text of 7.3.4.4.2 defines domain inheritance through base_name
    // and writes `<domain name="MyDomain" base_name="BaseDomain">`; the
    // schema of the same specification declares no such attribute on
    // <domain>, in-tree or OMG alike. Both reject it, the parser reads it
    // (see the `domain_with_base_name` row).
    let inheriting_domain = r#"<domain_library xmlns="http://www.omg.org/spec/DDS-XML" name="L">
        <domain name="BaseDomain" domain_id="7"><topic name="T" register_type_ref="R"/></domain>
        <domain name="MyDomain" base_name="BaseDomain"><topic name="T2" register_type_ref="R"/></domain>
    </domain_library>"#;
    let catalog = omg_catalog(&dir);
    let path = scratch().join("inheriting_domain.xml");
    std::fs::write(&path, inheriting_domain).expect("case file");
    let omg = example_path(&dir, Block::Domains.omg_schema());
    assert_document_invalid(
        &validate(&prog, &omg, &path, Some(&catalog)),
        "the OMG's domain schema is expected to reject base_name on <domain>",
    );
    assert_document_invalid(
        &validate(&prog, &Block::Domains.in_tree_schema(), &path, None),
        "the in-tree domain schema is the OMG's and rejects base_name on <domain>",
    );
    // And the parser reads the inheriting domain, including the id it
    // takes from its base.
    let libs = parse_domain_libraries(inheriting_domain).expect("inheriting domain parses");
    let lib = &libs[0];
    assert_eq!(
        lib.effective_domain_id(lib.domain("MyDomain").expect("MyDomain"))
            .expect("chain"),
        Some(7)
    );

    // Everything else: the same verdict from both schemas.
    for case in SCHEMA_INVALID {
        let omg_schema = example_path(&dir, case.block.omg_schema());
        let doc = scratch().join(format!("{}.xml", case.name));
        std::fs::write(&doc, case.xml).expect("case file");
        assert_document_invalid(
            &validate(&prog, &omg_schema, &doc, Some(&catalog)),
            &format!(
                "case `{}` violates {} against the in-tree schema; the \
                 OMG's {} must say the same, or the in-tree schema is \
                 stricter than the specification's own",
                case.name,
                case.rule,
                case.block.omg_schema()
            ),
        );
    }
}
