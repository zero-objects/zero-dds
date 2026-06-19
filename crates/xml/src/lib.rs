// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML 1.0 Loader — well-formed Foundation (Cluster F).
//!
//! Crate `zerodds-xml`. Implements the shared well-formedness and
//! IDL-PSM mapping layer from DDS-XML 1.0 §7.1 and §7.2. Building-block-
//! specific decoders (QoS library, types, domains, participants,
//! applications, samples) build on this crate — see
//! `docs/spec-coverage/zerodds-xml-1.0.open.md` Cluster G/H/I/J.
//!
//! # Spec sources
//!
//! * OMG DDS-XML 1.0, formal/18-10-01, December 2018.
//! * §7.1 — XML Representation Syntax (general rules + schema +
//!   chameleon pattern).
//! * §7.2 — XML Representation of DDS IDL PSM (data type mapping).
//! * §7.2.2 — symbol constants (`LENGTH_UNLIMITED`, `DURATION_INFINITE_*`).
//! * §7.2.6 — `Duration_t` representation.
//!
//! # Public API (foundation)
//!
//! * [`parse_xml_tree`] — generic well-formedness loader, returns
//!   [`DdsXmlDocument`].
//! * [`parse_dds_xml`] — high-level building-block loader, returns
//!   [`DdsXml`] (Cluster G+H+I+J).
//! * [`DdsXmlDocument`] / [`XmlElement`] — in-memory tree.
//! * Data type helpers from [`types`] (boolean, long, ulong, Duration,
//!   enum whitelist, string).
//! * Inheritance resolution from [`inheritance::resolve_chain`] with
//!   cycle detection.
//! * Errors from [`XmlError`].
//!
//! Safety classification: **STANDARD**.
//!
//! See `docs/architecture/02_architecture.md §3` and
//! `docs/architecture/04_safety_by_architecture.md §2`.
//!
//! The crate is `#![forbid(unsafe_code)]`, `no_std + alloc`, and
//! uses exclusively `roxmltree` (Apache-2.0 / MIT, pure-Rust)
//! as the XML backend.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(test, allow(clippy::panic, clippy::unwrap_used, clippy::expect_used))]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod application;
pub mod conformance;
pub mod domain;
pub mod errors;
pub mod inheritance;
pub mod parser;
pub mod participant;
pub mod qos;
pub mod qos_inheritance;
pub mod qos_parser;
pub mod registry;
pub mod resolver;
pub mod sample;
pub mod schemas;
pub mod typeobject_bridge;
pub mod types;
pub mod xsd_loader;
pub mod xsd_schema;
pub mod xtypes_def;
pub mod xtypes_parser;
pub mod zerodds_xml;

pub use application::{ApplicationEntry, ApplicationLibrary, parse_application_libraries};
pub use conformance::{IDL_TO_XML_MAPPING, SUPPORTED_BUILDING_BLOCKS};
pub use domain::{DomainEntry, DomainLibrary, RegisterType, TopicEntry, parse_domain_libraries};
pub use errors::XmlError;
pub use inheritance::{MAX_INHERITANCE_DEPTH, resolve_chain};
pub use parser::{
    DDS_XML_NS, DdsXmlDocument, MAX_LIST_ELEMENTS, MAX_TOTAL_ELEMENTS, XmlElement, parse_xml_tree,
};
pub use participant::{
    DataReaderEntry, DataWriterEntry, DomainParticipantEntry, DomainParticipantLibrary,
    PublisherEntry, SubscriberEntry, parse_domain_participant_libraries,
};
pub use qos::{EntityQos, QosLibrary, QosProfile, topic_filter_matches};
pub use qos_inheritance::{ResolvedQos, resolve_profile};
pub use qos_parser::{
    parse_bool_strict, parse_entity_qos_public, parse_qos_libraries, parse_qos_library,
    parse_qos_library_element_public,
};
pub use registry::QosProfileRegistry;
pub use resolver::{LibraryRef, parse_library_ref};
pub use sample::{
    PrimitiveValue, SampleValue, parse_sample, parse_sample_element, serialize_sample,
};
pub use schemas::{
    ALL_SCHEMAS, APPLICATIONS_NAMESPACED_XSD, APPLICATIONS_NONAMESPACE_XSD, COMMON_XSD,
    DATA_SAMPLES_NAMESPACED_XSD, DATA_SAMPLES_NONAMESPACE_XSD, DDS_SYSTEM_NAMESPACED_XSD,
    DDS_SYSTEM_NONAMESPACE_XSD, DOMAIN_PARTICIPANTS_NAMESPACED_XSD,
    DOMAIN_PARTICIPANTS_NONAMESPACE_XSD, DOMAINS_NAMESPACED_XSD, DOMAINS_NONAMESPACE_XSD,
    QOS_NAMESPACED_XSD, QOS_NONAMESPACE_XSD, TYPES_NAMESPACED_XSD, TYPES_NONAMESPACE_XSD,
    embedded_block_names,
};
pub use typeobject_bridge::{
    BridgeError, bridge_library, xml_type_to_minimal_typeobject, xml_type_to_typeobject,
};
pub use types::{
    DURATION_INFINITE_NSEC, DURATION_INFINITE_SEC, DURATION_ZERO_NSEC, DURATION_ZERO_SEC, Duration,
    LENGTH_UNLIMITED, MAX_STRING_BYTES, TIME_INVALID_NSEC, TIME_INVALID_SEC, parse_bool,
    parse_duration_nsec, parse_duration_sec, parse_enum, parse_long, parse_octet_sequence,
    parse_positive_long_unlimited, parse_string, parse_ulong,
};
pub use xtypes_def::{
    BitField, BitValue, BitmaskType, BitsetType, EnumLiteral, EnumType, Extensibility, ModuleEntry,
    PrimitiveType, StructMember, StructType, TypeDef, TypeLibrary, TypeRef, TypedefType, UnionCase,
    UnionDiscriminator, UnionType,
};
pub use xtypes_parser::{parse_type_libraries, parse_types_element};
pub use zerodds_xml::{
    DdsXml, ParticipantFactoryAdapter, ResolvedDataReader, ResolvedDataWriter, ResolvedParticipant,
    ResolvedPublisher, ResolvedSubscriber, ResolvedTopic, apply_to_factory, parse_dds_xml,
};

#[cfg(feature = "std")]
pub use xsd_loader::load_type_libraries_from_uri;
pub use xsd_loader::{
    DDS_XML_NAMESPACE, MAX_DATA_URI_BODY, MAX_FILE_BYTES, ValidationMode,
    load_type_libraries_from_string,
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod integration_tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use alloc::string::{String, ToString};

    /// End-to-end: parse a QoS-Library-shaped XML, walk the tree,
    /// extract values via the Cluster-F type helpers.
    #[test]
    fn qos_profile_shape_roundtrip() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-XML">
    <!-- root namespace per §7.3.x targetNamespace -->
    <qos_library name="Lib1">
        <qos_profile name="Base">
            <datawriter_qos>
                <reliability>
                    <kind>RELIABLE_RELIABILITY_QOS</kind>
                    <max_blocking_time>
                        <sec>DURATION_INFINITE_SEC</sec>
                        <nanosec>DURATION_INFINITE_NSEC</nanosec>
                    </max_blocking_time>
                </reliability>
                <history>
                    <kind>KEEP_LAST_HISTORY_QOS</kind>
                    <depth>10</depth>
                </history>
                <resource_limits>
                    <max_samples>LENGTH_UNLIMITED</max_samples>
                </resource_limits>
            </datawriter_qos>
        </qos_profile>
        <qos_profile name="Derived" base_name="Base">
        </qos_profile>
    </qos_library>
</dds>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        assert_eq!(doc.root.name, "dds");
        assert_eq!(doc.root.namespace.as_deref(), Some(DDS_XML_NS));

        let lib = doc.root.child("qos_library").expect("lib");
        assert_eq!(lib.attribute("name"), Some("Lib1"));

        let profiles: alloc::vec::Vec<_> = lib.children_named("qos_profile").collect();
        assert_eq!(profiles.len(), 2);

        let base = profiles[0];
        assert_eq!(base.attribute("name"), Some("Base"));
        assert_eq!(base.attribute("base_name"), None);

        let derived = profiles[1];
        assert_eq!(derived.attribute("name"), Some("Derived"));
        assert_eq!(derived.attribute("base_name"), Some("Base"));

        // Reach into <reliability><max_blocking_time> and parse Duration.
        let dw = base.child("datawriter_qos").expect("dw");
        let rel = dw.child("reliability").expect("rel");
        let kind_str = rel.child("kind").expect("kind").text.as_str();
        let kind = parse_enum(
            kind_str,
            &["BEST_EFFORT_RELIABILITY_QOS", "RELIABLE_RELIABILITY_QOS"],
        )
        .expect("enum");
        assert_eq!(kind, "RELIABLE_RELIABILITY_QOS");

        let mbt = rel.child("max_blocking_time").expect("mbt");
        let sec = parse_duration_sec(mbt.child("sec").expect("sec").text.as_str()).expect("sec");
        let nsec =
            parse_duration_nsec(mbt.child("nanosec").expect("nsec").text.as_str()).expect("nsec");
        let dur = Duration { sec, nanosec: nsec };
        assert!(dur.is_infinite(), "max_blocking_time should be INFINITE");

        // Resource-Limits: max_samples should resolve to LENGTH_UNLIMITED.
        let rl = dw.child("resource_limits").expect("rl");
        let ms = parse_long(rl.child("max_samples").expect("ms").text.as_str()).expect("long");
        assert_eq!(ms, LENGTH_UNLIMITED);

        // History.depth — a regular long.
        let hist = dw.child("history").expect("hist");
        let depth = parse_long(hist.child("depth").expect("depth").text.as_str()).expect("depth");
        assert_eq!(depth, 10);

        // Inheritance: Derived -> Base -> (none). Resolve via the
        // inheritance module.
        let mut by_name: BTreeMap<String, Option<String>> = BTreeMap::new();
        for p in lib.children_named("qos_profile") {
            let n = p.attribute("name").expect("named").to_string();
            let b = p.attribute("base_name").map(ToString::to_string);
            by_name.insert(n, b);
        }
        let chain = resolve_chain("Derived", |n| {
            by_name
                .get(n)
                .cloned()
                .ok_or_else(|| XmlError::MissingRequiredElement(n.to_string()))
        })
        .expect("chain");
        assert_eq!(
            chain,
            alloc::vec!["Base".to_string(), "Derived".to_string()]
        );
    }

    /// Detect a base_name cycle across two profiles.
    #[test]
    fn detect_cycle_between_profiles() {
        let xml = r#"<dds>
            <qos_library name="L">
                <qos_profile name="A" base_name="B"/>
                <qos_profile name="B" base_name="A"/>
            </qos_library>
        </dds>"#;
        let doc = parse_xml_tree(xml).expect("parse");
        let lib = doc.root.child("qos_library").expect("lib");
        let mut by_name: BTreeMap<String, Option<String>> = BTreeMap::new();
        for p in lib.children_named("qos_profile") {
            let n = p.attribute("name").expect("name").to_string();
            let b = p.attribute("base_name").map(ToString::to_string);
            by_name.insert(n, b);
        }
        let err = resolve_chain("A", |n| {
            by_name
                .get(n)
                .cloned()
                .ok_or_else(|| XmlError::MissingRequiredElement(n.to_string()))
        })
        .expect_err("cycle");
        assert!(matches!(err, XmlError::CircularInheritance(_)));
    }
}
