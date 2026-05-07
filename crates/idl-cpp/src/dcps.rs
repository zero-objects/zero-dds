// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Block-H: DCPS-Entity-Header-Stubs (Spec idl4-cpp-1.0 §7.6 + dds-psm-cxx-1.0 §8.1).
//!
//! Emittiert Class-Declarations und Method-Signaturen fuer die DCPS-Kern-
//! Entitaeten:
//!
//! - `dds::domain::DomainParticipant`
//! - `dds::pub::Publisher` + `dds::pub::DataWriter<T>`
//! - `dds::sub::Subscriber` + `dds::sub::DataReader<T>`
//! - `dds::topic::Topic<T>`
//! - `dds::core::Entity` (Basisklasse)
//!
//! Die volle Implementation liegt nicht im Codegen — das ist Aufgabe der
//! handgeschriebenen DDS-PSM-CXX-Runtime (Cluster C5.2 Templates).
//! Der Codegen liefert hier nur das Header-Skeleton, gegen das Anwender-
//! Code kompiliert.

use core::fmt::Write;

use crate::error::CppGenError;

/// Schreibt den vollstaendigen DCPS-Header.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben in den
/// `String`-Buffer scheitert.
pub fn emit_dcps_header(out: &mut String) -> Result<(), CppGenError> {
    writeln!(
        out,
        "// Block-H: DCPS-Entity-Header-Stubs (dds-psm-cxx-1.0 §8.1)."
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    emit_entity_base(out)?;
    emit_domain_participant(out)?;
    emit_topic(out)?;
    emit_publisher(out)?;
    emit_subscriber(out)?;
    emit_data_writer(out)?;
    emit_data_reader(out)?;

    Ok(())
}

/// Liefert die Liste der vom Block-H emittierten Klassen-FQN.
#[must_use]
pub fn dcps_class_names() -> Vec<&'static str> {
    vec![
        "dds::core::Entity",
        "dds::domain::DomainParticipant",
        "dds::topic::Topic",
        "dds::pub::Publisher",
        "dds::sub::Subscriber",
        "dds::pub::DataWriter",
        "dds::sub::DataReader",
    ]
}

fn emit_entity_base(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace core {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "/// Basisklasse aller DCPS-Entitaeten (dds-psm-cxx-1.0 §7.5.1)."
    )
    .map_err(fmt_err)?;
    writeln!(out, "class Entity {{").map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(out, "    virtual ~Entity() = default;").map_err(fmt_err)?;
    writeln!(out, "    void enable();").map_err(fmt_err)?;
    writeln!(out, "    void close();").map_err(fmt_err)?;
    writeln!(
        out,
        "    const ::dds::core::InstanceHandle& instance_handle() const;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::core").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_domain_participant(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace domain {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "/// DomainParticipant (dds-psm-cxx-1.0 §8.1.1).").map_err(fmt_err)?;
    writeln!(
        out,
        "class DomainParticipant : public ::dds::core::Entity {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(out, "    explicit DomainParticipant(int32_t domain_id);").map_err(fmt_err)?;
    writeln!(
        out,
        "    DomainParticipant(int32_t domain_id, const ::dds::domain::qos::DomainParticipantQos& qos);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ~DomainParticipant() override;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "    int32_t domain_id() const;").map_err(fmt_err)?;
    writeln!(
        out,
        "    const ::dds::domain::qos::DomainParticipantQos& qos() const;"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    void qos(const ::dds::domain::qos::DomainParticipantQos& q);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    void assert_liveliness();").map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::domain").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_topic(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace topic {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "/// Topic<T> (dds-psm-cxx-1.0 §8.1.2).").map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "class Topic : public ::dds::core::Entity {{").map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(
        out,
        "    Topic(::dds::domain::DomainParticipant& dp, const std::string& name);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    Topic(::dds::domain::DomainParticipant& dp, const std::string& name, const ::dds::topic::qos::TopicQos& qos);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ~Topic() override;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "    const std::string& name() const;").map_err(fmt_err)?;
    writeln!(out, "    const std::string& type_name() const;").map_err(fmt_err)?;
    writeln!(out, "    const ::dds::topic::qos::TopicQos& qos() const;").map_err(fmt_err)?;
    writeln!(out, "    void qos(const ::dds::topic::qos::TopicQos& q);").map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::topic").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_publisher(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace pub {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "/// Publisher (dds-psm-cxx-1.0 §8.1.3).").map_err(fmt_err)?;
    writeln!(out, "class Publisher : public ::dds::core::Entity {{").map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(
        out,
        "    explicit Publisher(::dds::domain::DomainParticipant& dp);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    Publisher(::dds::domain::DomainParticipant& dp, const ::dds::pub::qos::PublisherQos& qos);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ~Publisher() override;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "    const ::dds::pub::qos::PublisherQos& qos() const;").map_err(fmt_err)?;
    writeln!(out, "    void qos(const ::dds::pub::qos::PublisherQos& q);").map_err(fmt_err)?;
    writeln!(
        out,
        "    void wait_for_acknowledgments(const ::dds::core::Duration& timeout);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::pub").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_subscriber(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace sub {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "/// Subscriber (dds-psm-cxx-1.0 §8.1.4).").map_err(fmt_err)?;
    writeln!(out, "class Subscriber : public ::dds::core::Entity {{").map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(
        out,
        "    explicit Subscriber(::dds::domain::DomainParticipant& dp);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    Subscriber(::dds::domain::DomainParticipant& dp, const ::dds::sub::qos::SubscriberQos& qos);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ~Subscriber() override;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "    const ::dds::sub::qos::SubscriberQos& qos() const;"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    void qos(const ::dds::sub::qos::SubscriberQos& q);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    void notify_datareaders();").map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::sub").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_data_writer(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace pub {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "/// DataWriter<T> (dds-psm-cxx-1.0 §8.1.5).").map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "class DataWriter : public ::dds::core::Entity {{").map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(
        out,
        "    DataWriter(::dds::pub::Publisher& pub, ::dds::topic::Topic<T>& topic);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    DataWriter(::dds::pub::Publisher& pub, ::dds::topic::Topic<T>& topic, const ::dds::pub::qos::DataWriterQos& qos);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ~DataWriter() override;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "    void write(const T& sample);").map_err(fmt_err)?;
    writeln!(
        out,
        "    void write(const T& sample, const ::dds::core::Time& src_time);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    ::dds::core::InstanceHandle register_instance(const T& key);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    void unregister_instance(const ::dds::core::InstanceHandle& h);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    void dispose_instance(const ::dds::core::InstanceHandle& h);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    void wait_for_acknowledgments(const ::dds::core::Duration& timeout);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    ::dds::core::status::PublicationMatchedStatus publication_matched_status();"
    )
    .map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::pub").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_data_reader(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "namespace dds {{ namespace sub {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "/// DataReader<T> (dds-psm-cxx-1.0 §8.1.6).").map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "class DataReader : public ::dds::core::Entity {{").map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(
        out,
        "    DataReader(::dds::sub::Subscriber& sub, ::dds::topic::Topic<T>& topic);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    DataReader(::dds::sub::Subscriber& sub, ::dds::topic::Topic<T>& topic, const ::dds::sub::qos::DataReaderQos& qos);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ~DataReader() override;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "    std::vector<::dds::sub::Sample<T>> take();").map_err(fmt_err)?;
    writeln!(out, "    std::vector<::dds::sub::Sample<T>> read();").map_err(fmt_err)?;
    writeln!(
        out,
        "    ::dds::core::status::SubscriptionMatchedStatus subscription_matched_status();"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    ::dds::core::status::SampleLostStatus sample_lost_status();"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    ::dds::core::status::SampleRejectedStatus sample_rejected_status();"
    )
    .map_err(fmt_err)?;
    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} // namespace dds::sub").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn fmt_err(_: core::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    fn render() -> String {
        let mut s = String::new();
        emit_dcps_header(&mut s).expect("emit");
        s
    }

    #[test]
    fn entity_base_class_emitted_in_dds_core() {
        let s = render();
        assert!(s.contains("namespace dds { namespace core {"));
        assert!(s.contains("class Entity {"));
        assert!(s.contains("virtual ~Entity() = default;"));
    }

    #[test]
    fn domain_participant_class_declaration_is_generated() {
        let s = render();
        assert!(s.contains("namespace dds { namespace domain {"));
        assert!(s.contains("class DomainParticipant : public ::dds::core::Entity {"));
        assert!(s.contains("explicit DomainParticipant(int32_t domain_id);"));
        assert!(s.contains("int32_t domain_id() const;"));
    }

    #[test]
    fn publisher_and_subscriber_emitted() {
        let s = render();
        assert!(s.contains("namespace dds { namespace pub {"));
        assert!(s.contains("class Publisher : public ::dds::core::Entity {"));
        assert!(s.contains("namespace dds { namespace sub {"));
        assert!(s.contains("class Subscriber : public ::dds::core::Entity {"));
    }

    #[test]
    fn topic_template_with_t_parameter() {
        let s = render();
        assert!(s.contains("namespace dds { namespace topic {"));
        assert!(s.contains("template <typename T>"));
        assert!(s.contains("class Topic : public ::dds::core::Entity {"));
    }

    #[test]
    fn data_writer_and_data_reader_templates() {
        let s = render();
        assert!(s.contains("class DataWriter : public ::dds::core::Entity {"));
        assert!(s.contains("class DataReader : public ::dds::core::Entity {"));
        assert!(s.contains("void write(const T& sample);"));
        assert!(s.contains("std::vector<::dds::sub::Sample<T>> take();"));
        assert!(s.contains("std::vector<::dds::sub::Sample<T>> read();"));
    }

    #[test]
    fn data_writer_has_status_accessor_for_publication_matched() {
        let s = render();
        assert!(s.contains(
            "::dds::core::status::PublicationMatchedStatus publication_matched_status();"
        ));
    }

    #[test]
    fn dcps_class_names_count_seven() {
        let names = dcps_class_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"dds::domain::DomainParticipant"));
        assert!(names.contains(&"dds::topic::Topic"));
    }
}
