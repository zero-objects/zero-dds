// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! GitHub #33: a type whose fields borrow goes through a DataWriter and a
//! DataReader. It is written with borrows of any lifetime
//! (`zerodds_codec::write`) and read lent out of the octets the reader
//! received (`DataReader::take_loan`, `LoanExt::read_as`), nothing copied.
//! The DataWriter and the DataReader are of its `'static` form, whose
//! `decode` refuses.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::borrow::Cow;
use std::time::{Duration, Instant};

use zerodds_codec::{DdsType, LoanExt};
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    SubscriberQos, TopicQos,
};

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "loan::Lent", extensibility = "final")]
struct Lent<'a> {
    #[dds(key)]
    id: i32,
    text: &'a str,
    data: &'a [u8],
    note: Cow<'a, str>,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "loan::Other", extensibility = "final")]
struct Other {
    id: i32,
}

#[test]
fn issue_33_a_borrowing_type_is_written_and_read_lent_through_dcps() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant(lease.id(), DomainParticipantQos::default())
        .expect("participant");
    let topic = participant
        .create_topic::<Lent<'static>>("LentTopic", TopicQos::default())
        .expect("topic");
    let reader = participant
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<Lent<'static>>(&topic, DataReaderQos::default())
        .expect("reader");
    let writer = participant
        .create_publisher(PublisherQos::default())
        .create_datawriter(&topic, DataWriterQos::default())
        .expect("writer");
    let deadline = Instant::now() + Duration::from_secs(5);
    while reader
        .get_subscription_matched_status()
        .expect("status")
        .current_count
        == 0
    {
        assert!(Instant::now() < deadline, "the writer matches");
        std::thread::sleep(Duration::from_millis(10));
    }

    // Borrows of a local, not 'static.
    let owned_text = String::from("lent text");
    let owned_data = vec![9u8, 8, 7];
    let value = Lent {
        id: 4,
        text: &owned_text,
        data: &owned_data,
        note: Cow::Borrowed("note"),
    };
    zerodds_codec::write(&writer, &value).expect("write");
    assert!(
        zerodds_codec::write(&writer, &Other { id: 1 }).is_err(),
        "a value of another IDL type is refused"
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let loan = loop {
        match reader.take_loan() {
            Ok(loan) => break loan,
            Err(_) => {
                assert!(Instant::now() < deadline, "the sample arrives");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    assert_eq!(loan.len(), 1);
    let back: Lent<'_> = loan.read_as(0).expect("a sample").expect("read");
    assert_eq!(back, value);
    let body = loan.get(0).and_then(|s| s.body()).expect("data");
    let range = body.as_ptr_range();
    assert!(
        range.contains(&back.text.as_ptr()),
        "the text is lent, not copied"
    );
    assert!(
        range.contains(&back.data.as_ptr()),
        "the octets are lent, not copied"
    );
    assert!(matches!(
        loan.read_as::<Other>(0),
        Some(Err(zerodds_codec::CodecError::OtherType { .. }))
    ));
    assert!(loan.read_as::<Lent<'_>>(1).is_none());

    // The 'static form names and writes the type; it is not read from
    // received octets.
    let mut bytes = Vec::new();
    let fixed = Lent {
        id: 1,
        text: "static",
        data: &[1],
        note: Cow::Borrowed("n"),
    };
    <Lent<'static> as zerodds_dcps::DdsType>::encode(&fixed, &mut bytes).expect("encode");
    assert!(<Lent<'static> as zerodds_dcps::DdsType>::decode(&bytes).is_err());
}
