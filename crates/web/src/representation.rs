// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XML Object-Representation Tags — Spec §8.3.4 Tab 6.

/// Spec §8.3.5 — Content-Type for all WebDDS REST bodies.
pub const CONTENT_TYPE_DDS_WEB_XML: &str = "application/zerodds-web+xml";

/// Spec §8.3.4 Tab 6 — object-representation categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepresentationKind {
    /// Single object (e.g., `<application>`).
    Object,
    /// List of objects (e.g., `<application_list>`).
    List,
}

/// Wire element name for an object/list Tab 6 (Spec §8.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    /// `qos_library` / `qos_library_list`.
    QosLibrary,
    /// `qos_profile` / `qos_profile_list`.
    QosProfile,
    /// `application` / `application_list`.
    Application,
    /// `domain_participant` / `domain_participant_list`.
    DomainParticipant,
    /// `types` (XML type definition from DDS-XTYPES).
    Types,
    /// `waitset` / `waitset_list`.
    Waitset,
    /// `topic` / `topic_list`.
    Topic,
    /// `publisher` / `publisher_list`.
    Publisher,
    /// `subscriber` / `subscriber_list`.
    Subscriber,
    /// `data_writer` / `data_writer_list`.
    DataWriter,
    /// `data_reader` / `data_reader_list`.
    DataReader,
    /// `read_sample_seq` (DataReader::read).
    ReadSampleSeq,
    /// `write_sample_seq` (DataWriter::write).
    WriteSampleSeq,
}

/// Returns the XML element name from Spec §8.3.4 Tab 6.
#[must_use]
pub const fn element_name(kind: ContentType, repr: RepresentationKind) -> &'static str {
    use RepresentationKind::*;
    match (kind, repr) {
        (ContentType::QosLibrary, Object) => "qos_library",
        (ContentType::QosLibrary, List) => "qos_library_list",
        (ContentType::QosProfile, Object) => "qos_profile",
        (ContentType::QosProfile, List) => "qos_profile_list",
        (ContentType::Application, Object) => "application",
        (ContentType::Application, List) => "application_list",
        (ContentType::DomainParticipant, Object) => "domain_participant",
        (ContentType::DomainParticipant, List) => "domain_participant_list",
        (ContentType::Types, _) => "types",
        (ContentType::Waitset, Object) => "waitset",
        (ContentType::Waitset, List) => "waitset_list",
        (ContentType::Topic, Object) => "topic",
        (ContentType::Topic, List) => "topic_list",
        (ContentType::Publisher, Object) => "publisher",
        (ContentType::Publisher, List) => "publisher_list",
        (ContentType::Subscriber, Object) => "subscriber",
        (ContentType::Subscriber, List) => "subscriber_list",
        (ContentType::DataWriter, Object) => "data_writer",
        (ContentType::DataWriter, List) => "data_writer_list",
        (ContentType::DataReader, Object) => "data_reader",
        (ContentType::DataReader, List) => "data_reader_list",
        (ContentType::ReadSampleSeq, _) => "read_sample_seq",
        (ContentType::WriteSampleSeq, _) => "write_sample_seq",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_constant() {
        // Spec §8.3.5 Tab 7+8.
        assert_eq!(CONTENT_TYPE_DDS_WEB_XML, "application/zerodds-web+xml");
    }

    #[test]
    fn element_names_match_spec_tab_6() {
        use ContentType as C;
        use RepresentationKind::{List, Object};
        // Spec §8.3.4 Tab 6 — Object + List Names.
        assert_eq!(element_name(C::QosLibrary, Object), "qos_library");
        assert_eq!(element_name(C::QosLibrary, List), "qos_library_list");
        assert_eq!(element_name(C::QosProfile, Object), "qos_profile");
        assert_eq!(element_name(C::QosProfile, List), "qos_profile_list");
        assert_eq!(element_name(C::Application, Object), "application");
        assert_eq!(element_name(C::Application, List), "application_list");
        assert_eq!(
            element_name(C::DomainParticipant, Object),
            "domain_participant"
        );
        assert_eq!(
            element_name(C::DomainParticipant, List),
            "domain_participant_list"
        );
        assert_eq!(element_name(C::Waitset, Object), "waitset");
        assert_eq!(element_name(C::Topic, Object), "topic");
        assert_eq!(element_name(C::Publisher, Object), "publisher");
        assert_eq!(element_name(C::Subscriber, Object), "subscriber");
        assert_eq!(element_name(C::DataWriter, Object), "data_writer");
        assert_eq!(element_name(C::DataReader, Object), "data_reader");
        assert_eq!(element_name(C::ReadSampleSeq, Object), "read_sample_seq");
        assert_eq!(element_name(C::WriteSampleSeq, Object), "write_sample_seq");
    }

    #[test]
    fn types_representation_uses_dds_xtypes_root() {
        // Spec — types XML is represented via the DDS-XTYPES root element.
        assert_eq!(
            element_name(ContentType::Types, RepresentationKind::Object),
            "types"
        );
        assert_eq!(
            element_name(ContentType::Types, RepresentationKind::List),
            "types"
        );
    }
}
