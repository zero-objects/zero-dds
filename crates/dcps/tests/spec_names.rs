//! The operations and constants under the names DDS 1.4 gives them (§2.2.2,
//! §2.3.3): `get_topic`, `get_matched_*`, `get_enabled_statuses`,
//! `get_expression_parameters`, and the
//! `*_STATUS` and `*_QOS_POLICY_ID` constants.
#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use zerodds_dcps::Entity as _;
use zerodds_dcps::psm_constants::{qos_policy_id, status};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};

#[test]
fn the_operations_of_the_specification_answer_like_the_ones_of_this_crate() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(195, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("SpecNames", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader(&topic, DataReaderQos::default())
        .unwrap();
    assert_eq!(writer.get_topic().name(), "SpecNames");
    assert_eq!(
        writer.get_matched_subscriptions().unwrap(),
        writer.matched_subscriptions().unwrap()
    );
    assert_eq!(
        reader.get_matched_publications().unwrap(),
        reader.matched_publications().unwrap()
    );
    let condition = p.get_status_condition();
    condition.set_enabled_statuses(0b0101);
    assert_eq!(condition.get_enabled_statuses(), 0b0101);
    assert_eq!(
        condition.get_enabled_statuses(),
        condition.enabled_statuses()
    );
}

#[test]
fn the_constants_have_the_names_of_the_idl() {
    assert_eq!(
        status::INCONSISTENT_TOPIC_STATUS,
        status::INCONSISTENT_TOPIC
    );
    assert_eq!(
        status::SUBSCRIPTION_MATCHED_STATUS,
        status::SUBSCRIPTION_MATCHED
    );
    assert_eq!(qos_policy_id::USERDATA_QOS_POLICY_ID, 1);
    assert_eq!(qos_policy_id::LATENCYBUDGET_QOS_POLICY_ID, 5);
    assert_eq!(qos_policy_id::TIMEBASEDFILTER_QOS_POLICY_ID, 9);
    assert_eq!(qos_policy_id::DURABILITYSERVICE_QOS_POLICY_ID, 22);
}
