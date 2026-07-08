//! Endpoint-delete RECEIVE side — the unmatch primitive end-to-end.
//!
//! Proves that deleting a remote endpoint (dropping a `DataWriter` /
//! `DataReader`) does not just tear down the local side and send an SEDP
//! dispose (that is covered by the send-side tests), but that the **peer**
//! reacts to the dispose: it removes the now-stale proxy from its own matched
//! endpoints and evicts the endpoint from its discovery cache — immediately,
//! not after the liveliness lease.
//!
//! Linux-only, like the other real-SPDP-multicast e2e tests: it needs a live
//! multicast bus between two participants.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux_e2e {
    use std::time::{Duration, Instant};

    use zerodds_dcps::{
        DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
        RawBytes, SubscriberQos, TopicQos,
    };

    use super::common::{isolated_cfg, match_timeout, unique_domain};

    /// Polls `f` until it returns `true` or `timeout` elapses. Returns whether
    /// the condition was met. Used as the unmatch synchronization point — there
    /// is (deliberately) no `wait_for_unmatched` waker on the public API, so a
    /// bounded poll against the live status count is the test's join condition.
    fn poll_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if f() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // Mirror of `dropping_a_reader_unmatches_the_remote_writer`. This direction
    // exercises the SEDP **publication** dispose; it used to be flaky because
    // `ReliableWriter::write_lifecycle` only direct-sent the dispose when the
    // SEDP writer's reader-proxy cursor sat exactly at `dispose_sn - 1` (a race
    // against the background SEDP send loop). `write_lifecycle` now drains the
    // proxy in-order up to the marker, so the dispose is delivered
    // cursor-independently. See `internal/dcps/endpoint-delete-and-unmatch-followup.md`.
    #[test]
    fn dropping_a_writer_unmatches_the_remote_reader() {
        let cfg = isolated_cfg();
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(11);

        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<RawBytes>("DeleteMe", TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<RawBytes>("DeleteMe", TopicQos::default())
            .expect("sub topic");

        let publisher = pub_p.create_publisher(PublisherQos::default());
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());

        let writer = publisher
            .create_datawriter::<RawBytes>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let reader = subscriber
            .create_datareader::<RawBytes>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        // Establish the match in both directions first.
        writer
            .wait_for_matched_subscription(1, match_timeout())
            .expect("writer matches reader");
        reader
            .wait_for_matched_publication(1, match_timeout())
            .expect("reader matches writer");
        assert_eq!(reader.matched_publication_count(), 1);
        assert_eq!(sub_p.discovered_publications_count(), 1);

        // Delete the writer. Drop runs the local teardown + SEDP dispose send.
        drop(writer);

        // The subscriber must drop the match — both the live matched-status
        // count on the reader and the SEDP discovery cache on the participant.
        assert!(
            poll_until(match_timeout(), || reader.matched_publication_count() == 0),
            "reader still matched to the deleted writer after the dispose"
        );
        assert!(
            poll_until(match_timeout(), || sub_p.discovered_publications_count()
                == 0),
            "deleted publication still in the subscriber discovery cache"
        );
    }

    #[test]
    fn dropping_a_reader_unmatches_the_remote_writer() {
        let cfg = isolated_cfg();
        let factory = DomainParticipantFactory::instance();
        let domain = unique_domain(12);

        let pub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
            .expect("pub participant");
        let sub_p = factory
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .expect("sub participant");

        let pub_topic = pub_p
            .create_topic::<RawBytes>("DeleteMe2", TopicQos::default())
            .expect("pub topic");
        let sub_topic = sub_p
            .create_topic::<RawBytes>("DeleteMe2", TopicQos::default())
            .expect("sub topic");

        let publisher = pub_p.create_publisher(PublisherQos::default());
        let subscriber = sub_p.create_subscriber(SubscriberQos::default());

        let writer = publisher
            .create_datawriter::<RawBytes>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let reader = subscriber
            .create_datareader::<RawBytes>(&sub_topic, DataReaderQos::default())
            .expect("reader");

        writer
            .wait_for_matched_subscription(1, match_timeout())
            .expect("writer matches reader");
        reader
            .wait_for_matched_publication(1, match_timeout())
            .expect("reader matches writer");
        assert_eq!(writer.matched_subscription_count(), 1);
        assert_eq!(pub_p.discovered_subscriptions_count(), 1);

        // Delete the reader. The writer-side peer must unmatch.
        drop(reader);

        assert!(
            poll_until(match_timeout(), || writer.matched_subscription_count() == 0),
            "writer still matched to the deleted reader after the dispose"
        );
        assert!(
            poll_until(match_timeout(), || pub_p.discovered_subscriptions_count()
                == 0),
            "deleted subscription still in the publisher discovery cache"
        );
    }
}
