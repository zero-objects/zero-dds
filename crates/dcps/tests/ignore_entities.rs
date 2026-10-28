//! DDS 1.4 §2.2.2.2.1.14–17 `ignore_participant`, `ignore_topic`,
//! `ignore_publication`, `ignore_subscription`, with the handle the
//! built-in readers' SampleInfo carries, and what the call does to matches
//! made before it.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

#[cfg(target_os = "linux")]
mod linux {
    use std::time::{Duration, Instant};

    use zerodds_dcps::interop::ShapeType;
    use zerodds_dcps::{
        DataReader, DataReaderQos, DataWriter, DataWriterQos, DomainParticipantFactory,
        DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
    };

    use super::common::{ParticipantGuard, isolated_cfg, match_timeout, unique_domain};

    struct Pair {
        pub_p: ParticipantGuard,
        sub_p: ParticipantGuard,
        writer: DataWriter<ShapeType>,
        reader: DataReader<ShapeType>,
        // Last: fields drop in declaration order, and the domain goes back to
        // the pool only after the participants on it are gone.
        _lease: Option<super::common::DomainLease>,
    }

    /// How the two participants find each other.
    #[derive(Clone, Copy)]
    enum Route {
        /// The same multicast group: both are in one process, so discovery
        /// takes the in-process path.
        InProcess,
        /// Each its own multicast group, no multicast send, and the other's
        /// unicast SPDP port as the initial peer: the endpoints are known
        /// through SEDP over UDP only.
        Wire,
    }

    fn pair_via(topic: &str, route: Route) -> Pair {
        let factory = DomainParticipantFactory::instance();
        let (domain, lease) = match route {
            Route::InProcess => (unique_domain(), None),
            Route::Wire => {
                let lease = super::common::lease_domain();
                (lease.id(), Some(lease))
            }
        };
        let (cfg_pub, cfg_sub) = match route {
            Route::InProcess => {
                let cfg = isolated_cfg();
                (cfg.clone(), cfg)
            }
            Route::Wire => {
                let peer = zerodds_rtps::wire_types::Locator::udp_v4(
                    [127, 0, 0, 1],
                    zerodds_rtps::wire_types::spdp_unicast_port(u32::try_from(domain).unwrap(), 0),
                );
                let wire = |cfg: zerodds_dcps::runtime::RuntimeConfig| {
                    zerodds_dcps::runtime::RuntimeConfig {
                        spdp_multicast_send: false,
                        initial_peers: vec![peer],
                        ..cfg
                    }
                };
                (wire(isolated_cfg()), wire(isolated_cfg()))
            }
        };
        let pub_p = ParticipantGuard::new(
            factory
                .create_participant_with_config(domain, DomainParticipantQos::default(), cfg_pub)
                .expect("pub participant"),
        );
        let sub_p = ParticipantGuard::new(
            factory
                .create_participant_with_config(domain, DomainParticipantQos::default(), cfg_sub)
                .expect("sub participant"),
        );
        let pub_topic = pub_p
            .create_topic::<ShapeType>(topic, TopicQos::default())
            .expect("pub topic");
        let writer = pub_p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<ShapeType>(&pub_topic, DataWriterQos::default())
            .expect("writer");
        let sub_topic = sub_p
            .create_topic::<ShapeType>(topic, TopicQos::default())
            .expect("sub topic");
        let reader = sub_p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<ShapeType>(&sub_topic, DataReaderQos::default())
            .expect("reader");
        writer
            .wait_for_matched_subscription(1, match_timeout())
            .expect("writer match");
        reader
            .wait_for_matched_publication(1, match_timeout())
            .expect("reader match");
        Pair {
            _lease: lease,
            pub_p,
            sub_p,
            writer,
            reader,
        }
    }

    /// Polls `cond` for up to five seconds.
    fn eventually(cond: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if cond() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        cond()
    }

    /// The handle of the one sample `take` returns from a built-in reader
    /// within five seconds whose key is `wanted`.
    fn builtin_handle<T: zerodds_dcps::DdsType + Send + 'static>(
        reader: &DataReader<T>,
        wanted: impl Fn(&T) -> bool,
    ) -> zerodds_dcps::InstanceHandle {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(s) = reader
                .take_with_info()
                .expect("take")
                .into_iter()
                .find(|s| s.info.valid_data && wanted(&s.data))
            {
                return s.info.instance_handle;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("no built-in sample came");
    }

    /// The writes after the call do not reach the reader.
    fn nothing_arrives(p: &Pair) -> bool {
        for x in 0..5 {
            p.writer
                .write(&ShapeType::new("GREEN", x, x, 30))
                .expect("write");
        }
        std::thread::sleep(Duration::from_millis(300));
        p.reader.take().expect("take").is_empty()
    }

    fn unmatched(p: &Pair) -> bool {
        eventually(|| p.reader.matched_publication_count() == 0)
    }

    /// DDS 1.4 §2.2.2.2.1.14: the handle of `ignore_participant` "is the
    /// one that appears in the SampleInfo retrieved when reading the
    /// data-samples available for the built-in DataReader to the
    /// 'DCPSParticipant' topic", and from then on "the Service will locally
    /// behave as if the remote participant did not exist". The handle of
    /// the built-in sample is that of `get_discovered_participants`; the
    /// match made before the call goes. The built-in readers numbered their
    /// instances 1, 2, …, and the match stayed.
    fn ignore_participant_with_the_builtin_handle_unmatches_via(route: Route) {
        let p = pair_via("IgnoreParticipant", route);
        let remote = p.pub_p.participant_handle();
        let builtin = p.sub_p.get_builtin_subscriber().participant_reader();
        let handle = builtin_handle(&builtin, |d| {
            zerodds_dcps::InstanceHandle::from_guid(d.key) == remote
        });
        assert_eq!(handle, remote, "the SampleInfo handle of the participant");
        assert!(p.sub_p.get_discovered_participants().contains(&handle));
        p.sub_p.ignore_participant(handle).expect("ignore");
        assert!(
            unmatched(&p),
            "the ignored participant's writer stayed matched"
        );
        assert!(nothing_arrives(&p));
    }

    /// §2.2.2.2.1.16 `ignore_publication` with the handle of the
    /// `DCPSPublication` sample, which is that of `matched_publications`.
    fn ignore_publication_with_the_builtin_handle_unmatches_via(route: Route) {
        let p = pair_via("IgnorePublication", route);
        let matched = p.reader.matched_publications().expect("matched");
        assert_eq!(matched.len(), 1);
        let builtin = p.sub_p.get_builtin_subscriber().publication_reader();
        let handle = builtin_handle(&builtin, |d| d.topic_name == "IgnorePublication");
        assert_eq!(
            handle, matched[0],
            "the SampleInfo handle of the publication"
        );
        p.sub_p.ignore_publication(handle).expect("ignore");
        assert!(unmatched(&p), "the ignored publication stayed matched");
        assert!(nothing_arrives(&p));
    }

    /// §2.2.2.2.1.15 `ignore_topic`: "it will locally ignore any publication
    /// or subscription to the Topic". With the handle of the `DCPSTopic`
    /// sample, the matched writer on the topic goes. `ignore_topic` was
    /// asked at no match.
    fn ignore_topic_unmatches_the_endpoints_of_the_topic_via(route: Route) {
        let p = pair_via("IgnoreTopic", route);
        let builtin = p.sub_p.get_builtin_subscriber().topic_reader();
        let handle = builtin_handle(&builtin, |d| d.name == "IgnoreTopic");
        p.sub_p.ignore_topic(handle).expect("ignore");
        assert!(
            unmatched(&p),
            "the writer on the ignored topic stayed matched"
        );
        assert!(nothing_arrives(&p));
    }

    /// §2.2.2.2.1.17 `ignore_subscription` on the writer's side, with the
    /// handle of the `DCPSSubscription` sample, which is that of
    /// `matched_subscriptions` (§2.2.2.4.2.24).
    fn ignore_subscription_unmatches_via(route: Route) {
        let p = pair_via("IgnoreSubscription", route);
        let matched = p.writer.matched_subscriptions().expect("matched");
        assert_eq!(matched.len(), 1);
        let builtin = p.pub_p.get_builtin_subscriber().subscription_reader();
        let handle = builtin_handle(&builtin, |d| d.topic_name == "IgnoreSubscription");
        assert_eq!(
            handle, matched[0],
            "the SampleInfo handle of the subscription"
        );
        p.pub_p.ignore_subscription(handle).expect("ignore");
        assert!(
            eventually(|| p.writer.matched_subscription_count() == 0),
            "the ignored subscription stayed matched"
        );
        assert!(nothing_arrives(&p));
    }

    #[test]
    fn ignore_participant_with_the_builtin_handle_unmatches() {
        ignore_participant_with_the_builtin_handle_unmatches_via(Route::InProcess);
    }

    #[test]
    fn ignore_participant_with_the_builtin_handle_unmatches_over_the_wire() {
        ignore_participant_with_the_builtin_handle_unmatches_via(Route::Wire);
    }

    #[test]
    fn ignore_publication_with_the_builtin_handle_unmatches() {
        ignore_publication_with_the_builtin_handle_unmatches_via(Route::InProcess);
    }

    #[test]
    fn ignore_publication_with_the_builtin_handle_unmatches_over_the_wire() {
        ignore_publication_with_the_builtin_handle_unmatches_via(Route::Wire);
    }

    #[test]
    fn ignore_topic_unmatches_the_endpoints_of_the_topic() {
        ignore_topic_unmatches_the_endpoints_of_the_topic_via(Route::InProcess);
    }

    #[test]
    fn ignore_topic_unmatches_the_endpoints_of_the_topic_over_the_wire() {
        ignore_topic_unmatches_the_endpoints_of_the_topic_via(Route::Wire);
    }

    #[test]
    fn ignore_subscription_unmatches() {
        ignore_subscription_unmatches_via(Route::InProcess);
    }

    #[test]
    fn ignore_subscription_unmatches_over_the_wire() {
        ignore_subscription_unmatches_via(Route::Wire);
    }
}
