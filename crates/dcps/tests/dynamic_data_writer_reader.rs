//! DDS-XTypes 1.3 §7.6.6.2: `DynamicDataWriter` and `DynamicDataReader`
//! are the `FooDataWriter` and `FooDataReader` of "Foo" = `DynamicData`,
//! and the XML Data Representation (§7.4.4, Table 60) carries the same
//! samples between them.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::Arc;

use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_types::dynamic::{DynamicSample, DynamicType};
use zerodds_types::type_object::TypeObject;

#[path = "common/mod.rs"]
mod common;

const IDL: &str = "
    @final struct Reading {
        @key long id;
        string<16> label;
        double value;
        sequence<long, 8> history;
    };";

fn support() -> (Arc<SerializedTypeSupport>, DynamicType) {
    let ast = Arc::new(zerodds_wire_probe::encode::model_from_idl(IDL, "reading.idl").unwrap());
    let type_ = DynamicType::find(&ast, "Reading").unwrap();
    let object = TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, type_.id()).unwrap(),
    )
    .to_bytes_le()
    .unwrap();
    let support = SerializedTypeSupport::from_type_object("Reading", &object, true, 0).unwrap();
    let type_ = support.get_type().unwrap();
    (support, type_)
}

fn reading(type_: &DynamicType, id: i32, label: &str, value: f64) -> DynamicSample {
    let mut sample = DynamicSample::new(type_).unwrap();
    let ids = (
        sample.get_member_id_by_name("id").unwrap(),
        sample.get_member_id_by_name("label").unwrap(),
        sample.get_member_id_by_name("value").unwrap(),
        sample.get_member_id_by_name("history").unwrap(),
    );
    sample.set_int32_value(ids.0, id).unwrap();
    sample.set_string_value(ids.1, label).unwrap();
    sample.set_float64_value(ids.2, value).unwrap();
    sample.set_int32_values(ids.3, &[1, 2, 3]).unwrap();
    sample
}

#[test]
fn a_dynamic_topic_takes_writers_and_readers_of_dynamic_data() {
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };
    let (support, type_) = support();
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(181, DomainParticipantQos::default());
    let topic = p
        .create_dynamic_topic("Readings", TopicQos::default(), Arc::clone(&support))
        .unwrap();
    assert_eq!(topic.type_name(), "Reading");
    let writer: zerodds_dcps::serialized_type::DynamicDataWriter = p
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    let reader: zerodds_dcps::serialized_type::DynamicDataReader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader(&topic, DataReaderQos::default())
        .unwrap();
    assert_eq!(reader.topic().name(), "Readings");
    let one = reading(&type_, 1, "a", 0.5);
    let handle = writer.register_instance(&one).unwrap();
    assert_eq!(writer.lookup_instance(&one), handle);
    assert_ne!(
        writer.lookup_instance(&reading(&type_, 2, "b", 0.5)),
        handle,
        "another key is another instance"
    );
    // A DynamicData of another type is refused.
    let other_ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl("@final struct Other { long v; };", "o.idl")
            .unwrap(),
    );
    let other = DynamicSample::new(&DynamicType::find(&other_ast, "Other").unwrap()).unwrap();
    assert!(writer.register_instance(&other).is_err());
}

#[cfg(target_os = "linux")]
mod linux_e2e {
    use std::time::Duration;

    use zerodds_dcps::serialized_type::{DynamicDataReader, DynamicDataWriter};
    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        SubscriberQos, TopicQos,
    };

    use super::{common, reading, support};

    fn exchange(representation: Option<Vec<i16>>) {
        let (support, type_) = support();
        let cfg = common::isolated_cfg();
        let factory = DomainParticipantFactory::instance();
        let domain = common::unique_domain();
        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");
        let pub_topic = pub_p
            .create_dynamic_topic("Readings", TopicQos::default(), support.clone())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_dynamic_topic("Readings", TopicQos::default(), support)
            .expect("sub topic");
        let writer: DynamicDataWriter = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter(
                &pub_topic,
                DataWriterQos {
                    data_representation: representation.clone(),
                    ..DataWriterQos::default()
                },
            )
            .expect("writer");
        let reader: DynamicDataReader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader(
                &sub_topic,
                DataReaderQos {
                    data_representation: representation,
                    ..DataReaderQos::default()
                },
            )
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, common::match_timeout())
            .expect("writer sees reader");
        reader
            .wait_for_matched_publication(1, common::match_timeout())
            .expect("reader sees writer");

        let sent = reading(&type_, 7, "tick <1> & co", 2.5);
        writer.write(&sent).expect("write");
        reader
            .wait_for_data(Duration::from_secs(3))
            .expect("sample arrives");
        let taken = reader.take().expect("take");
        assert_eq!(taken.len(), 1);
        assert!(taken[0].equals(&sent), "the sample arrives as it was sent");
    }

    #[test]
    fn dynamic_data_goes_over_xcdr() {
        exchange(None);
    }

    /// The writer sends the XML document of the sample under
    /// `{0x00, 0x04}`; the reader reads it against its type.
    #[test]
    fn dynamic_data_goes_over_xml() {
        use zerodds_rtps::publication_data::data_representation as dr;
        exchange(Some(vec![dr::XML, dr::XCDR2]));
    }
}
