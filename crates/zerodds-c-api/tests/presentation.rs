//! PRESENTATION through the C API (DDS 1.4 §2.2.3.6).
//!
//! Coherent sets (§2.2.2.4.1.10, §2.2.2.4.1.9; DDSI-RTPS 2.5 §8.7.6):
//! `zerodds_pub_begin_coherent_changes` and
//! `zerodds_pub_end_coherent_changes` open and close a set on every writer
//! of the publisher, and a reader under a subscriber with `coherent_access`
//! takes the set whole once it ended. Both calls did nothing, and the
//! reader handed out the members of an open set. Under GROUP access scope
//! the set spans the writers, the writers do not change while it is open
//! (§8.7.6), and a take needs `begin_access` (§2.2.2.5.2.8).
//!
//! The order of a read or take (§2.2.2.5.3.8): the samples of an instance
//! are consecutive, in write order under TOPIC scope with `ordered_access`,
//! one per take under GROUP scope with `ordered_access`. The C reader
//! returned them interleaved in arrival order.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::entities::{
    ZeroDdsDataReader, ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher,
    ZeroDdsSubscriber, ZeroDdsTopic,
};
use zerodds::extra_ffi::{
    zerodds_dp_get_default_publisher_qos, zerodds_dp_get_default_subscriber_qos,
    zerodds_sub_get_default_datareader_qos,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_publisher, zerodds_dp_delete_subscriber, zerodds_dp_delete_topic,
};
use zerodds::publisher_ffi::{
    zerodds_dw_wait_for_matched, zerodds_dw_write, zerodds_pub_begin_coherent_changes,
    zerodds_pub_create_datawriter, zerodds_pub_delete_datawriter, zerodds_pub_end_coherent_changes,
};
use zerodds::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsPresentationQosPolicy, ZeroDdsPublisherQos};
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, zerodds_dr_read, zerodds_dr_return_loan, zerodds_dr_take,
    zerodds_sub_begin_access, zerodds_sub_create_datareader, zerodds_sub_delete_datareader,
    zerodds_sub_end_access,
};

const OK: i32 = 0;
const PRECONDITION_NOT_MET: i32 = zerodds::ZeroDdsStatus::PreconditionNotMet as i32;

const KEEP_ALL: u32 = 1;

const INSTANCE_SCOPE: u32 = 0;
const TOPIC_SCOPE: u32 = 1;
const GROUP_SCOPE: u32 = 2;

/// How long a held set must stay invisible.
const HELD: Duration = Duration::from_millis(300);
/// How long a released set may take to arrive.
const ARRIVAL: Duration = Duration::from_secs(5);

struct Fixture {
    dp: *mut ZeroDdsDomainParticipant,
    publisher: *mut ZeroDdsPublisher,
    subscriber: *mut ZeroDdsSubscriber,
    topics: Vec<*mut ZeroDdsTopic>,
    writers: Vec<*mut ZeroDdsDataWriter>,
    readers: Vec<*mut ZeroDdsDataReader>,
    _lease: zerodds_test_domains::DomainLease,
}

impl Fixture {
    /// A publisher and a subscriber with `coherent_access` at `scope`, and
    /// a writer and a reader on each of `topics`, matched.
    fn new(scope: u32, topics: &[&str]) -> Self {
        Self::with(
            ZeroDdsPresentationQosPolicy {
                access_scope: scope,
                coherent_access: true,
                ordered_access: false,
            },
            topics,
        )
    }

    /// A publisher and a subscriber with `presentation`, and a writer and a
    /// reader on each of `topics`, matched.
    fn with(presentation: ZeroDdsPresentationQosPolicy, topics: &[&str]) -> Self {
        let lease = zerodds_test_domains::lease_domain();
        let domain = lease.id() as u32;
        let type_name = CString::new("RawBytes").unwrap();
        // SAFETY: every pointer below is NULL-checked before use, per each
        // callee's own `# Safety` contract; the CStrings live to the end of
        // the block.
        unsafe {
            let f = zerodds_dpf_get_instance();
            let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
            assert!(!dp.is_null(), "participant create failed");
            let mut pub_qos: ZeroDdsPublisherQos = std::mem::zeroed();
            assert_eq!(zerodds_dp_get_default_publisher_qos(dp, &mut pub_qos), OK);
            pub_qos.presentation = presentation;
            let mut sub_qos: ZeroDdsPublisherQos = std::mem::zeroed();
            assert_eq!(zerodds_dp_get_default_subscriber_qos(dp, &mut sub_qos), OK);
            sub_qos.presentation = presentation;
            let publisher = zerodds_dp_create_publisher(dp, &pub_qos);
            let subscriber = zerodds_dp_create_subscriber(dp, &sub_qos);
            assert!(!publisher.is_null() && !subscriber.is_null());
            let mut fixture = Self {
                dp,
                publisher,
                subscriber,
                topics: Vec::new(),
                writers: Vec::new(),
                readers: Vec::new(),
                _lease: lease,
            };
            for name in topics {
                let topic_name = CString::new(*name).unwrap();
                let topic = zerodds_dp_create_topic(
                    dp,
                    topic_name.as_ptr(),
                    type_name.as_ptr(),
                    ptr::null(),
                );
                assert!(!topic.is_null(), "topic create failed");
                let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
                // KEEP_ALL: what a test writes, the reader keeps.
                let mut dr_qos: ZeroDdsDataReaderQos = std::mem::zeroed();
                assert_eq!(
                    zerodds_sub_get_default_datareader_qos(subscriber, &mut dr_qos),
                    OK
                );
                dr_qos.history.kind = KEEP_ALL;
                let reader = zerodds_sub_create_datareader(subscriber, topic, &dr_qos);
                assert!(!writer.is_null() && !reader.is_null());
                assert_eq!(
                    zerodds_dw_wait_for_matched(writer, 1, 5_000),
                    OK,
                    "writer did not match the same-participant reader"
                );
                fixture.topics.push(topic);
                fixture.writers.push(writer);
                fixture.readers.push(reader);
            }
            fixture
        }
    }

    fn write(&self, writer: usize, payload: &[u8]) {
        let writer = self.writers[writer];
        // SAFETY: the writer is live; the payload outlives the call.
        let rc = unsafe { zerodds_dw_write(writer, payload.as_ptr(), payload.len(), 0) };
        assert_eq!(rc, OK, "write");
    }

    fn take(&self, reader: usize) -> (i32, Vec<Vec<u8>>) {
        take(self.readers[reader])
    }

    fn take_until(
        &self,
        reader: usize,
        want: usize,
        within: Duration,
        group: bool,
    ) -> Vec<Vec<u8>> {
        take_until(self.subscriber, self.readers[reader], want, within, group)
    }
}

/// The payloads of the samples one take of `dr` returns, with the return
/// code.
fn take(dr: *mut ZeroDdsDataReader) -> (i32, Vec<Vec<u8>>) {
    let mut arr = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: the reader is live; `arr` is a valid out-parameter, and its
    // arrays hold `count` entries until the loan is returned.
    unsafe {
        let rc = zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0);
        let mut out = Vec::new();
        for i in 0..arr.count {
            if (*arr.infos.add(i)).valid_data {
                let buf = *arr.buffers.add(i);
                let len = *arr.lengths.add(i);
                out.push(std::slice::from_raw_parts(buf, len).to_vec());
            }
        }
        if arr.count > 0 {
            assert_eq!(zerodds_dr_return_loan(dr, &mut arr), OK);
        }
        (rc, out)
    }
}

/// What takes of `dr` return until `want` samples arrived or `within`
/// passed, each inside `begin_access`/`end_access` of `sub` under GROUP
/// scope.
fn take_until(
    sub: *mut ZeroDdsSubscriber,
    dr: *mut ZeroDdsDataReader,
    want: usize,
    within: Duration,
    group: bool,
) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + within;
    let mut got = Vec::new();
    loop {
        if group {
            // SAFETY: the subscriber is live.
            assert_eq!(unsafe { zerodds_sub_begin_access(sub) }, OK);
        }
        got.extend(take(dr).1);
        if group {
            // SAFETY: the subscriber is live.
            assert_eq!(unsafe { zerodds_sub_end_access(sub) }, OK);
        }
        if got.len() >= want || Instant::now() >= deadline {
            return got;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: every handle is live and deleted once, children first.
        unsafe {
            for r in &self.readers {
                zerodds_sub_delete_datareader(self.subscriber, *r);
            }
            for w in &self.writers {
                zerodds_pub_delete_datawriter(self.publisher, *w);
            }
            zerodds_dp_delete_subscriber(self.dp, self.subscriber);
            zerodds_dp_delete_publisher(self.dp, self.publisher);
            for t in &self.topics {
                zerodds_dp_delete_topic(self.dp, *t);
            }
            zerodds_dpf_delete_participant(zerodds_dpf_get_instance(), self.dp);
        }
    }
}

#[test]
fn a_coherent_set_is_taken_whole_once_its_last_end_closes_it() {
    let fx = Fixture::new(TOPIC_SCOPE, &["CoherentTopicScope"]);
    // SAFETY: the publisher is live.
    unsafe {
        assert_eq!(zerodds_pub_begin_coherent_changes(fx.publisher), OK);
        // "These calls can be nested."
        assert_eq!(zerodds_pub_begin_coherent_changes(fx.publisher), OK);
    }
    for payload in [[1u8], [2], [3]] {
        fx.write(0, &payload);
    }
    // SAFETY: the publisher is live.
    assert_eq!(
        unsafe { zerodds_pub_end_coherent_changes(fx.publisher) },
        OK
    );
    assert!(
        fx.take_until(0, 1, HELD, false).is_empty(),
        "a member of an open set reached the application"
    );
    // SAFETY: the publisher is live.
    assert_eq!(
        unsafe { zerodds_pub_end_coherent_changes(fx.publisher) },
        OK
    );
    let set = fx.take_until(0, 3, ARRIVAL, false);
    assert_eq!(set, vec![vec![1u8], vec![2], vec![3]]);
    // SAFETY: the publisher is live.
    assert_eq!(
        unsafe { zerodds_pub_end_coherent_changes(fx.publisher) },
        PRECONDITION_NOT_MET,
        "an end without an open set"
    );
}

#[test]
fn a_writer_created_inside_an_open_set_joins_it() {
    let fx = Fixture::new(TOPIC_SCOPE, &["CoherentJoinA"]);
    // SAFETY: the publisher is live.
    assert_eq!(
        unsafe { zerodds_pub_begin_coherent_changes(fx.publisher) },
        OK
    );
    let topic_name = CString::new("CoherentJoinB").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    // SAFETY: the participant, publisher and subscriber are live; the
    // handles are deleted below.
    unsafe {
        let topic =
            zerodds_dp_create_topic(fx.dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let writer = zerodds_pub_create_datawriter(fx.publisher, topic, ptr::null());
        let reader = zerodds_sub_create_datareader(fx.subscriber, topic, ptr::null());
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(zerodds_dw_wait_for_matched(writer, 1, 5_000), OK);
        let payload = [7u8];
        assert_eq!(zerodds_dw_write(writer, payload.as_ptr(), 1, 0), OK);
        assert!(
            take_until(fx.subscriber, reader, 1, HELD, false).is_empty(),
            "the new writer's sample is not in the open set"
        );
        assert_eq!(zerodds_pub_end_coherent_changes(fx.publisher), OK);
        assert_eq!(
            take_until(fx.subscriber, reader, 1, ARRIVAL, false),
            vec![vec![7u8]]
        );
        zerodds_sub_delete_datareader(fx.subscriber, reader);
        zerodds_pub_delete_datawriter(fx.publisher, writer);
        zerodds_dp_delete_topic(fx.dp, topic);
    }
}

#[test]
fn a_group_coherent_set_spans_the_writers_and_holds_them() {
    let fx = Fixture::new(GROUP_SCOPE, &["CoherentGroupA", "CoherentGroupB"]);
    // §2.2.2.5.2.8: a take under GROUP access scope needs begin_access.
    assert_eq!(fx.take(0).0, PRECONDITION_NOT_MET);
    // SAFETY: the subscriber is live.
    assert_eq!(
        unsafe { zerodds_sub_end_access(fx.subscriber) },
        PRECONDITION_NOT_MET,
        "an end_access without begin_access"
    );

    // SAFETY: the publisher is live.
    assert_eq!(
        unsafe { zerodds_pub_begin_coherent_changes(fx.publisher) },
        OK
    );
    fx.write(0, &[1]);
    fx.write(1, &[2]);
    fx.write(0, &[3]);
    assert!(
        fx.take_until(0, 1, HELD, true).is_empty(),
        "reader A saw an open set"
    );
    assert!(
        fx.take_until(1, 1, Duration::ZERO, true).is_empty(),
        "reader B saw an open set"
    );

    // DDSI-RTPS 2.5 §8.7.6: "Writers are not allowed to be added or
    // removed from a Publisher from the time that a coherent set begins
    // until after it ends."
    // SAFETY: the publisher and the topic are live.
    unsafe {
        assert!(
            zerodds_pub_create_datawriter(fx.publisher, fx.topics[0], ptr::null()).is_null(),
            "a writer was added inside a group coherent set"
        );
        assert_eq!(
            zerodds_pub_delete_datawriter(fx.publisher, fx.writers[1]),
            PRECONDITION_NOT_MET,
            "a writer was removed inside a group coherent set"
        );
        assert_eq!(zerodds_pub_end_coherent_changes(fx.publisher), OK);
    }
    assert_eq!(fx.take_until(0, 2, ARRIVAL, true), vec![vec![1u8], vec![3]]);
    assert_eq!(fx.take_until(1, 1, ARRIVAL, true), vec![vec![2u8]]);
}

/// Samples of the instances `[1]` and `[2]`, written 1, 2, 1, 2, 1, and
/// taken in one take of `fx`'s reader after all of them arrived.
fn write_two_instances_and_take(fx: &Fixture) -> Vec<Vec<u8>> {
    for payload in TWO_INSTANCES {
        fx.write(0, &payload);
    }
    // Arrived: a read does not take them away.
    let deadline = Instant::now() + ARRIVAL;
    loop {
        let n = read_count(fx.readers[0]);
        if n == TWO_INSTANCES.len() {
            break;
        }
        assert!(Instant::now() < deadline, "{n} of 5 samples arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
    fx.take(0).1
}

const TWO_INSTANCES: [[u8; 1]; 5] = [[1], [2], [1], [2], [1]];

/// How many samples one read of `dr` returns; they stay in the reader.
fn read_count(dr: *mut ZeroDdsDataReader) -> usize {
    let mut arr = ZeroDdsSampleArray {
        buffers: ptr::null_mut(),
        lengths: ptr::null_mut(),
        infos: ptr::null_mut(),
        count: 0,
        loan_token: ptr::null_mut(),
    };
    // SAFETY: the reader is live; `arr` is a valid out-parameter.
    unsafe {
        let _ = zerodds_dr_read(dr, &mut arr, 0, 0, 0, 0);
        let n = arr.count;
        if n > 0 {
            assert_eq!(zerodds_dr_return_loan(dr, &mut arr), OK);
        }
        n
    }
}

#[test]
fn the_samples_of_an_instance_are_consecutive() {
    let fx = Fixture::with(
        ZeroDdsPresentationQosPolicy {
            access_scope: INSTANCE_SCOPE,
            coherent_access: false,
            ordered_access: false,
        },
        &["PresentationGrouped"],
    );
    let batch = write_two_instances_and_take(&fx);
    assert_eq!(batch.len(), 5);
    let runs = batch.windows(2).filter(|w| w[0] != w[1]).count() + 1;
    assert_eq!(runs, 2, "the instances are interleaved: {batch:?}");
    // Within an instance the samples stay in write order; all of `[1]` are
    // equal, so the count of each tells it.
    assert_eq!(batch.iter().filter(|p| p.as_slice() == [1]).count(), 3);
}

#[test]
fn topic_scope_with_ordered_access_keeps_the_write_order() {
    let fx = Fixture::with(
        ZeroDdsPresentationQosPolicy {
            access_scope: TOPIC_SCOPE,
            coherent_access: false,
            ordered_access: true,
        },
        &["PresentationWriteOrder"],
    );
    assert_eq!(
        write_two_instances_and_take(&fx),
        vec![vec![1u8], vec![2], vec![1], vec![2], vec![1]]
    );
}

#[test]
fn group_scope_with_ordered_access_takes_one_sample_at_a_time() {
    let fx = Fixture::with(
        ZeroDdsPresentationQosPolicy {
            access_scope: GROUP_SCOPE,
            coherent_access: false,
            ordered_access: true,
        },
        &["PresentationOnePerRead"],
    );
    for payload in TWO_INSTANCES {
        fx.write(0, &payload);
    }
    let deadline = Instant::now() + ARRIVAL;
    let mut taken = Vec::new();
    while taken.len() < TWO_INSTANCES.len() && Instant::now() < deadline {
        // SAFETY: the subscriber is live.
        assert_eq!(unsafe { zerodds_sub_begin_access(fx.subscriber) }, OK);
        let batch = fx.take(0).1;
        // SAFETY: the subscriber is live.
        assert_eq!(unsafe { zerodds_sub_end_access(fx.subscriber) }, OK);
        assert!(batch.len() <= 1, "a take returned {} samples", batch.len());
        taken.extend(batch);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(taken, vec![vec![1u8], vec![2], vec![1], vec![2], vec![1]]);
}
