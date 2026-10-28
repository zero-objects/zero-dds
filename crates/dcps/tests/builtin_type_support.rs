//! DDS-XTypes 1.3 Annex E: `StringTypeSupport`, `KeyedStringTypeSupport`,
//! `BytesTypeSupport` and `KeyedBytesTypeSupport` — "This interface shall
//! instantiate the type FooTypeSupport defined by the DDS specification".

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::builtin_types::{
    BytesTypeSupport, DdsBytes, DdsKeyedBytes, DdsKeyedString, DdsString, KeyedBytesTypeSupport,
    KeyedStringTypeSupport, StringTypeSupport, TypeSupport,
};
use zerodds_dcps::{DdsError, DdsType, DomainParticipantFactory, DomainParticipantQos, TopicQos};

fn participant(domain: i32) -> zerodds_dcps::DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(domain, DomainParticipantQos::default())
}

/// DDS 1.4 §2.2.2.3.6 `get_type_name`: the names of Annex E, which are the
/// type names of the data types.
#[test]
fn each_type_support_names_its_type() {
    assert_eq!(StringTypeSupport.get_type_name(), "DDS::String");
    assert_eq!(KeyedStringTypeSupport.get_type_name(), "DDS::KeyedString");
    assert_eq!(BytesTypeSupport.get_type_name(), "DDS::Bytes");
    assert_eq!(KeyedBytesTypeSupport.get_type_name(), "DDS::KeyedBytes");
    assert_eq!(StringTypeSupport.get_type_name(), DdsString::TYPE_NAME);
    assert_eq!(
        KeyedStringTypeSupport.get_type_name(),
        DdsKeyedString::TYPE_NAME
    );
    assert_eq!(BytesTypeSupport.get_type_name(), DdsBytes::TYPE_NAME);
    assert_eq!(
        KeyedBytesTypeSupport.get_type_name(),
        DdsKeyedBytes::TYPE_NAME
    );
}

/// DDS 1.4 §2.2.2.3.6 `register_type`: the type is registered under the
/// name given or, with none, under `get_type_name`; a topic of the type is
/// created from the registered name; registering again is not an error;
/// another type under a taken name is.
#[test]
fn register_type_registers_each_type_under_a_name() {
    let p = participant(181);
    let supports: [&dyn TypeSupport; 4] = [
        &StringTypeSupport,
        &KeyedStringTypeSupport,
        &BytesTypeSupport,
        &KeyedBytesTypeSupport,
    ];
    for (index, support) in supports.into_iter().enumerate() {
        let name = support.get_type_name().to_string();
        // Not registered yet.
        assert!(
            p.create_registered_topic(&format!("before{index}"), &name, TopicQos::default())
                .is_err(),
            "{name}"
        );
        support.register_type(&p, None).unwrap();
        support.register_type(&p, Some(&name)).unwrap();
        let topic = p
            .create_registered_topic(&format!("t{index}"), &name, TopicQos::default())
            .unwrap();
        assert_eq!(topic.type_name(), name);
        // Under another name of the caller's.
        let alias = format!("Alias{index}");
        support.register_type(&p, Some(&alias)).unwrap();
        let aliased = p
            .create_registered_topic(&format!("u{index}"), &alias, TopicQos::default())
            .unwrap();
        assert_eq!(aliased.type_name(), alias);
    }
}

/// Another type under a taken name, and a name that is no name.
#[test]
fn register_type_refuses_a_taken_name_and_an_empty_one() {
    let p = participant(182);
    StringTypeSupport.register_type(&p, Some("Shared")).unwrap();
    let error = KeyedStringTypeSupport
        .register_type(&p, Some("Shared"))
        .unwrap_err();
    assert!(
        matches!(error, DdsError::PreconditionNotMet { .. }),
        "{error:?}"
    );
    let error = BytesTypeSupport.register_type(&p, Some("")).unwrap_err();
    assert!(matches!(error, DdsError::BadParameter { .. }), "{error:?}");
    // The first registration is untouched.
    StringTypeSupport.register_type(&p, Some("Shared")).unwrap();
}

/// A typed topic is created under the registered name too.
#[test]
fn a_typed_topic_takes_the_registered_name() {
    let p = participant(183);
    KeyedStringTypeSupport
        .register_type(&p, Some("Pairs"))
        .unwrap();
    let topic = p
        .create_topic_with_type_name::<DdsKeyedString>("typed", "Pairs", TopicQos::default())
        .unwrap();
    assert_eq!(topic.type_name(), "Pairs");
}
