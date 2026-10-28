// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! The five example documents the DDS-XML 1.0 specification ships with,
//! parsed by this crate.
//!
//! Nothing here was written by this tree. The files are the OMG's own
//! `dds-xml_*_example.xml`, distributed with the specification alongside
//! the XSDs in `crates/xml/schemas/`, and they are the only DDS-XML
//! documents in reach that no ZeroDDS code produced. They are not
//! committed — the specification's text and files are OMG copyright, the
//! same reason `internal/` keeps the OPC-UA NodeSets out of the tree — so
//! the suite points at them through `ZERODDS_OMG_DDSXML_DIR`.
//!
//! **The gate is loud in both directions.** With the variable unset each
//! test prints `[skip]` and returns; with it set, a missing or unreadable
//! file is a panic naming the path, never an early return. A test that
//! can silently not run is not a test, and this tree has paid for that
//! twice.
//!
//! ## What these files caught
//!
//! Every defect below made every conforming document of its building
//! block fail. None was visible to the crate's own tests, because those
//! tests were written from the same reading of the specification as the
//! parsers.
//!
//! 1. **Only `<dds>` was accepted as a document root.** §7.3.2.3,
//!    §7.3.4.3, §7.3.5.3 and §7.3.6.3 each define a root element for
//!    their own building block, and four of these five files use one —
//!    only `dds-xml_dds_system_example.xml` opens with `<dds>`.
//! 2. **`domain_ref` was required on a `<domain_participant>`.**
//!    `dds-xml_domainparticipant_definitions_nonamespace.xsd` marks only
//!    `name` as `use="required"`; `domain_ref`, `domain_id` and
//!    `base_name` are all optional, and the first participant of
//!    `dds-xml_domainparticipants_example.xml` carries `domain_id` and no
//!    `domain_ref`.
//! 3. **`domain_id` was not read at all**, so the one identifier that
//!    participant does carry was dropped.
//! 4. **An `<application>`'s `<domain_participant>` was read as a `ref`
//!    attribute.** `dds-xml_application_definitions_nonamespace.xsd`
//!    includes the DomainParticipant schema and types those children as
//!    `domainParticipant` — a full inline definition. There is no `ref`
//!    attribute in the schema, and neither of the two applications in
//!    `dds-xml_applications_example.xml` has one.

use std::fs;
use std::path::{Path, PathBuf};

use zerodds_xml::qos_parser::parse_qos_libraries;
use zerodds_xml::{
    parse_application_libraries, parse_dds_xml, parse_domain_libraries,
    parse_domain_participant_libraries,
};

/// `None` when the corpus is not configured; the tests say so and stop.
fn corpus() -> Option<PathBuf> {
    std::env::var("ZERODDS_OMG_DDSXML_DIR")
        .ok()
        .map(PathBuf::from)
}

/// Reads one example file. Panics with the path when the directory is
/// configured but the file is not in it — a corpus that is half there
/// must fail, not skip.
fn example(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "ZERODDS_OMG_DDSXML_DIR is set but {} is not readable: {e}",
            path.display()
        )
    })
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

/// §7.3.2.3 — `<qos_library>` is a document root of its own.
#[test]
fn qos_example() {
    let dir = corpus_or_skip!();
    let xml = example(&dir, "dds-xml_qos_example.xml");

    let libs = parse_qos_libraries(&xml).expect("the OMG's own QoS document must parse");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "DDSDefaultQoSLibrary");
    assert_eq!(libs[0].profiles.len(), 1);

    let p = &libs[0].profiles[0];
    assert_eq!(p.name, "DDSDefaultQosProfile");
    // The profile carries every one of the six QoS containers the
    // specification names; a parser that read the root but dropped the
    // bodies would still pass the assertions above.
    assert!(!p.datawriter_qos.is_empty(), "<datawriter_qos>");
    assert!(!p.datareader_qos.is_empty(), "<datareader_qos>");
    assert!(!p.topic_qos.is_empty(), "<topic_qos>");
    assert!(!p.publisher_qos.is_empty(), "<publisher_qos>");
    assert!(!p.subscriber_qos.is_empty(), "<subscriber_qos>");
    assert!(
        !p.domainparticipant_qos.is_empty(),
        "<domain_participant_qos>"
    );
}

/// §7.3.4.3 — `<domain_library>` is a document root of its own.
#[test]
fn domains_example() {
    let dir = corpus_or_skip!();
    let xml = example(&dir, "dds-xml_domains_example.xml");

    let libs = parse_domain_libraries(&xml).expect("the OMG's own domain document must parse");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "MyDomainLibrary");
    assert_eq!(libs[0].domains.len(), 2);

    let shapes = &libs[0].domains[0];
    assert_eq!(shapes.name, "ShapesDomain");
    assert_eq!(shapes.domain_id, Some(32));
    assert_eq!(shapes.register_types.len(), 1);
    // Pentagon through Dodecagon.
    assert_eq!(shapes.topics.len(), 8);
    assert_eq!(shapes.topics[0].name, "Pentagon");

    let other = &libs[0].domains[1];
    assert_eq!(other.name, "AnotherShapesDomain");
    assert_eq!(other.domain_id, Some(101));
    assert_eq!(other.topics.len(), 3);
}

/// §7.3.5.3 — `<domain_participant_library>` is a document root of its
/// own, and its first participant names its domain numerically.
#[test]
fn domain_participants_example() {
    let dir = corpus_or_skip!();
    let xml = example(&dir, "dds-xml_domainparticipants_example.xml");

    let libs = parse_domain_participant_libraries(&xml)
        .expect("the OMG's own participant document must parse");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "MyDomainLibrary");
    assert_eq!(libs[0].participants.len(), 2);

    // Defects 2 and 3: `domain_id` and no `domain_ref` at all.
    let hello = &libs[0].participants[0];
    assert_eq!(hello.name, "HelloParticipant");
    assert_eq!(hello.domain_ref, None);
    assert_eq!(hello.domain_id, Some(32));
    // §7.3.5.4.2 plus the Domain building block's types: the
    // participant's <register_type> children are definitions, not
    // references into a domain it does not name.
    assert_eq!(hello.register_types.len(), 1);
    assert_eq!(hello.register_types[0].name, "HelloWorld");
    assert_eq!(hello.publishers.len(), 1);
    assert_eq!(hello.publishers[0].data_writers[0].name, "HelloWriter");

    // The other names both, and the two must not be confused.
    let shapes = &libs[0].participants[1];
    assert_eq!(shapes.name, "ShapesParticipant");
    assert_eq!(
        shapes.domain_ref.as_deref(),
        Some("ADomainLibrary::ShapeDomain")
    );
    assert_eq!(shapes.domain_id, Some(127));
    assert_eq!(shapes.publishers[0].data_writers.len(), 3);
    assert_eq!(shapes.subscribers[0].data_readers.len(), 3);
}

/// §7.3.6.3 — `<application_library>` is a document root of its own, and
/// its participants are written out in full rather than referenced.
#[test]
fn applications_example() {
    let dir = corpus_or_skip!();
    let xml = example(&dir, "dds-xml_applications_example.xml");

    let libs =
        parse_application_libraries(&xml).expect("the OMG's own application document must parse");
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "MyApplicationLibrary");
    assert_eq!(libs[0].applications.len(), 2);

    // Defect 4: both of these are inline definitions, not references.
    let first = &libs[0].applications[0];
    assert_eq!(first.name, "AnApplication");
    assert_eq!(first.domain_participants.len(), 2);
    assert_eq!(first.domain_participants[0].name, "HelloParticipant");
    assert_eq!(first.domain_participants[0].domain_id, Some(32));
    assert_eq!(
        first.domain_participants[0].publishers[0].data_writers[0].name,
        "HelloWriter"
    );
    // §7.3.6.4.2's inheritance form: base_name and no local domain.
    let goodbye = &first.domain_participants[1];
    assert_eq!(goodbye.name, "GoodbyeParticipant");
    assert_eq!(
        goodbye.base_name.as_deref(),
        Some("MyParticipantLibrary::GoodbyeParticipant")
    );
    assert_eq!(goodbye.domain_ref, None);
    assert_eq!(goodbye.domain_id, Some(64));

    let second = &libs[0].applications[1];
    assert_eq!(second.name, "AnotherApplication");
    let sp = &second.domain_participants[0];
    assert_eq!(
        sp.domain_ref.as_deref(),
        Some("ADomainLibrary::ShapeDomain")
    );
    assert_eq!(sp.publishers[0].data_writers.len(), 3);
    assert_eq!(sp.subscribers[0].data_readers.len(), 3);
}

/// §8.1 — the composed root, which carries several building blocks at
/// once. This is the one file of the five that always parsed.
#[test]
fn dds_system_example() {
    let dir = corpus_or_skip!();
    let xml = example(&dir, "dds-xml_dds_system_example.xml");

    let doc = parse_dds_xml(&xml).expect("the OMG's own composed document must parse");
    assert_eq!(doc.type_libraries.len(), 1);
    assert_eq!(doc.application_libraries.len(), 1);
    assert_eq!(doc.application_libraries[0].name, "MyApplications");

    let app = &doc.application_libraries[0].applications[0];
    assert_eq!(app.name, "SimpleShapesDemoApplication");
    let dp = &app.domain_participants[0];
    assert_eq!(dp.name, "MyParticipant");
    assert_eq!(
        dp.domain_ref.as_deref(),
        Some("ShapesDomainLibrary::ShapesDomain")
    );
    // The participant's own <topic> children are definitions typed by
    // the Domain building block's topicEntry, not references.
    let topic_names: Vec<&str> = dp.topics.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(topic_names, ["Square", "Circle", "Triangle"]);
    assert_eq!(dp.publishers[0].data_writers.len(), 3);
    assert_eq!(dp.subscribers[0].data_readers.len(), 3);

    // The `<types>` block of the same document: the ShapeType the
    // participant registers.
    assert!(
        doc.resolve_type("ShapeType").is_some(),
        "the composed document's <types> block must reach the type registry"
    );
}
