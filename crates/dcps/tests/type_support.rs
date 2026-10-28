// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `register_type` and the TypeSupport (DDS 1.4 §2.2.2.3.6.1, §2.2.2.3.6.2,
//! §2.3.3) and `create_topic` for a registered type (§2.2.2.2.1.5).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_dcps::dds_type::{DecodeError, EncodeError};
use zerodds_dcps::type_support::TypeSupport;
use zerodds_dcps::{
    DdsError, DdsType, DomainParticipantFactory, DomainParticipantQos, RawBytes, TopicDescription,
    TopicQos,
};

macro_rules! plain_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        struct $name {
            value: u32,
        }
        impl DdsType for $name {
            const TYPE_NAME: &'static str = concat!("test::", stringify!($name));
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
                out.extend_from_slice(&self.value.to_be_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
                let v: [u8; 4] = bytes
                    .get(..4)
                    .and_then(|b| b.try_into().ok())
                    .ok_or(DecodeError::Invalid { what: "value" })?;
                Ok(Self {
                    value: u32::from_be_bytes(v),
                })
            }
        }
    };
}

plain_type!(First);
plain_type!(Second);

fn participant() -> zerodds_dcps::DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default())
}

/// §2.2.2.3.6.2: the default name of the TypeSupport is the type's own.
#[test]
fn the_type_support_names_its_type() {
    assert_eq!(TypeSupport::<First>::new().get_type_name(), "test::First");
    assert_eq!(
        TypeSupport::<RawBytes>::new().get_type_name(),
        RawBytes::type_name()
    );
}

/// §2.2.2.3.6.1: "it is allowed to register the same TypeSupport multiple
/// times with a DomainParticipant using the same or different values for the
/// type_name … the second (and subsequent) registrations are ignored but the
/// operation returns OK. The application may pass nil as the value for the
/// type_name."
#[test]
fn the_same_type_support_registers_again_under_the_same_or_another_name() {
    let p = participant();
    let support = TypeSupport::<First>::new();
    support.register_type(&p, None).unwrap();
    support.register_type(&p, None).unwrap();
    support.register_type(&p, Some("test::First")).unwrap();
    support.register_type(&p, Some("OtherName")).unwrap();
    support.register_type(&p, Some("OtherName")).unwrap();
    assert!(matches!(
        support.register_type(&p, Some("")),
        Err(DdsError::BadParameter { .. })
    ));
}

/// §2.2.2.3.6.1: "It is a pre-condition error to use the same type_name to
/// register two different TypeSupport with the same DomainParticipant … the
/// operation will fail and return PRECONDITION_NOT_MET."
#[test]
fn one_type_name_is_not_two_type_supports() {
    let p = participant();
    TypeSupport::<First>::new()
        .register_type(&p, Some("Shared"))
        .unwrap();
    assert!(matches!(
        TypeSupport::<Second>::new().register_type(&p, Some("Shared")),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    // The default name of the second is still free, and so is the name in
    // another participant.
    TypeSupport::<Second>::new()
        .register_type(&p, None)
        .unwrap();
    let q = participant();
    TypeSupport::<Second>::new()
        .register_type(&q, Some("Shared"))
        .unwrap();
}

/// §2.2.2.2.1.5: "Prior to creating a Topic the type must have been
/// registered with the Service"; the Topic is bound to the type under the
/// name it was registered with.
#[test]
fn a_topic_is_bound_to_the_registered_type() {
    let p = participant();
    assert!(matches!(
        p.create_topic_of_registered_type::<First>("Bound", "Alias", TopicQos::default()),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    TypeSupport::<First>::new()
        .register_type(&p, Some("Alias"))
        .unwrap();
    // A name registered for another type does not bind this one.
    assert!(matches!(
        p.create_topic_of_registered_type::<Second>("Bound", "Alias", TopicQos::default()),
        Err(DdsError::PreconditionNotMet { .. })
    ));
    let topic = p
        .create_topic_of_registered_type::<First>("Bound", "Alias", TopicQos::default())
        .unwrap();
    assert_eq!(topic.type_name(), "Alias");
    let found = p.lookup_topicdescription("Bound").unwrap();
    assert_eq!(found.get_type_name(), "Alias");
}
