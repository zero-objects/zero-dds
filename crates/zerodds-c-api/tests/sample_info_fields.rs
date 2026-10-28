//! DDS 1.4 §2.2.2.5 `SampleInfo` field correctness over `zerodds_dr_take`.
//!
//! # Background
//!
//! `crates/zerodds-c-api/src/subscriber_ffi.rs`'s `LoanMemory::from_state`
//! used to fill most `ZeroDdsSampleInfo` fields with constants — `view_state`
//! always NEW, `disposed_generation_count`/`no_writers_generation_count`/
//! `sample_rank`/`generation_rank`/`absolute_generation_rank` always `0`,
//! lifecycle markers' `instance_state` derived only from the wire `kind` and
//! never from the reader's own `InstanceTracker`. Fixed: `resolve_instances_
//! and_ownership` now bakes each `SampleInfo` from the SAME per-instance
//! `InstanceTracker` state machine the typed `DataReader` uses
//! (`crates/dcps/src/subscriber.rs`), and `LoanMemory::from_state` computes
//! `sample_rank`/`generation_rank`/`absolute_generation_rank` over the
//! returned batch via `zerodds_dcps::sample_info::compute_ranks`.
//!
//! Each test here is a same-participant writer+reader pair on a keyed
//! topic, using `zerodds_dw_write_keyed` so the C-FFI's untyped take path
//! resolves a real per-instance identity (Spec §2.2.2.5.4).
//!
//! Not covered here (see `resolve_instances_and_ownership`'s and
//! `apply_lost_writers`'s doc comments in `subscriber_ffi.rs` for why):
//! * `publication_handle` on a `Lifecycle` marker — `UserSample::Lifecycle`
//!   carries no writer GUID at all (neither does the typed `DataReader`'s
//!   own lifecycle path), so it stays `HANDLE_NIL` for both.
//! * `NOT_ALIVE_NO_WRITERS` via a matched writer actually disconnecting
//!   (`apply_lost_writers`) — needs a real SEDP unmatch/liveliness-lease
//!   event; `zerodds_pub_delete_datawriter` does not currently send one.
//!   The explicit `unregister_instance` path below exercises the same
//!   `NOT_ALIVE_NO_WRITERS` reporting through the other route.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::extra_ffi::{
    zerodds_dw_register_instance, zerodds_dw_unregister_instance,
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
    zerodds_dw_dispose, zerodds_dw_wait_for_matched, zerodds_dw_write_keyed,
    zerodds_pub_create_datawriter, zerodds_pub_delete_datawriter,
};
use zerodds::qos_ffi::ZeroDdsDataReaderQos;
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, ZeroDdsSampleInfo, zerodds_dr_return_loan, zerodds_dr_take,
    zerodds_sub_create_datareader_kind, zerodds_sub_delete_datareader,
};

const OK: i32 = 0;
const NO_DATA: i32 = -7;

// `ZeroDdsSampleInfo` bit values (crates/zerodds-c-api/src/subscriber_ffi.rs).
const VIEW_NEW: u32 = 1;
const VIEW_NOT_NEW: u32 = 2;
const INSTANCE_ALIVE: u32 = 1;
const INSTANCE_DISPOSED: u32 = 2;
const INSTANCE_NO_WRITERS: u32 = 4;

struct Fixture {
    // Held for the fixture's lifetime: `DomainLease::drop` releases the
    // domain back to the pool, so it must outlive every participant/entity
    // created on it below, not be dropped (or forgotten) right after
    // reading its id.
    _lease: zerodds_test_domains::DomainLease,
    p: *mut zerodds::entities::ZeroDdsDomainParticipant,
    publisher: *mut zerodds::entities::ZeroDdsPublisher,
    subscriber: *mut zerodds::entities::ZeroDdsSubscriber,
    topic: *mut zerodds::entities::ZeroDdsTopic,
    writer: *mut zerodds::entities::ZeroDdsDataWriter,
    reader: *mut zerodds::entities::ZeroDdsDataReader,
}

impl Fixture {
    /// Same-participant keyed writer+reader pair on a fresh leased domain.
    ///
    /// # Safety
    /// Must be torn down with `Fixture::cleanup` exactly once.
    unsafe fn new(topic_name: &str) -> Self {
        let lease = zerodds_test_domains::lease_domain();
        let domain = lease.id() as u32;
        let topic_name = CString::new(topic_name).unwrap();
        let type_name = CString::new("SampleInfoFieldsTy").unwrap();
        // SAFETY: every pointer below is NULL-checked before use, per each
        // callee's own `# Safety` contract.
        unsafe {
            let f = zerodds_dpf_get_instance();
            let p = zerodds_dpf_create_participant(f, domain, ptr::null());
            assert!(!p.is_null(), "participant create failed");
            let topic =
                zerodds_dp_create_topic(p, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
            assert!(!topic.is_null(), "topic create failed");
            let publisher = zerodds_dp_create_publisher(p, ptr::null());
            let subscriber = zerodds_dp_create_subscriber(p, ptr::null());
            assert!(!publisher.is_null() && !subscriber.is_null());
            let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
            // KEEP_ALL: the tests write several samples of an instance and
            // read their SampleInfo (DDS 1.4 §2.2.3.18; the default
            // KEEP_LAST(1) keeps the newest).
            let mut dr_qos: ZeroDdsDataReaderQos = core::mem::zeroed();
            assert_eq!(
                zerodds_sub_get_default_datareader_qos(subscriber, &mut dr_qos),
                OK
            );
            dr_qos.history.kind = 1;
            let reader = zerodds_sub_create_datareader_kind(subscriber, topic, &dr_qos, 1);
            assert!(!writer.is_null() && !reader.is_null());
            assert_eq!(
                zerodds_dw_wait_for_matched(writer, 1, 5_000),
                OK,
                "writer did not match the same-participant reader"
            );
            Fixture {
                _lease: lease,
                p,
                publisher,
                subscriber,
                topic,
                writer,
                reader,
            }
        }
    }

    /// # Safety
    /// Called exactly once, after the last use of `self`'s handles.
    unsafe fn cleanup(self) {
        // SAFETY: every handle came from `Self::new` and is still live.
        unsafe {
            zerodds_sub_delete_datareader(self.subscriber, self.reader);
            zerodds_pub_delete_datawriter(self.publisher, self.writer);
            zerodds_dp_delete_subscriber(self.p, self.subscriber);
            zerodds_dp_delete_publisher(self.p, self.publisher);
            zerodds_dp_delete_topic(self.p, self.topic);
            zerodds_dpf_delete_participant(zerodds_dpf_get_instance(), self.p);
        }
    }
}

/// Takes exactly one batch (up to `max`), polling `zerodds_dr_take` until
/// data is available or `budget` elapses. Returns the decoded infos; the
/// loan is returned before this function does.
///
/// # Safety
/// `reader` valid and enabled.
unsafe fn take_batch(
    reader: *mut zerodds::entities::ZeroDdsDataReader,
    max: usize,
    budget: Duration,
) -> Vec<ZeroDdsSampleInfo> {
    let deadline = Instant::now() + budget;
    loop {
        // SAFETY: ZeroDdsSampleArray is repr(C), all-zero is a valid initial
        // out-param (null pointers, count 0).
        let mut arr: ZeroDdsSampleArray = unsafe { core::mem::zeroed() };
        // SAFETY: reader valid (caller pledge); arr a freshly zeroed
        // stack-local out-param.
        let rc = unsafe { zerodds_dr_take(reader, &mut arr, max, 0, 0, 0) };
        match rc {
            OK => {
                // SAFETY: arr.count entries of arr.infos were written by
                // `zerodds_dr_take` above.
                let infos = unsafe { core::slice::from_raw_parts(arr.infos, arr.count) }.to_vec();
                // SAFETY: arr came from the zerodds_dr_take call above.
                unsafe { zerodds_dr_return_loan(reader, &mut arr) };
                return infos;
            }
            NO_DATA => {
                assert!(
                    Instant::now() < deadline,
                    "no data before the test's own budget elapsed"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            other => panic!("zerodds_dr_take: unexpected status {other}"),
        }
    }
}

/// (a) View-state (Spec §2.2.2.5.1.2): a brand-new instance's first sample
/// reports NEW; a second sample of the SAME instance, taken in a SEPARATE
/// `zerodds_dr_take` call, reports NOT_NEW.
#[test]
fn view_state_new_then_not_new_across_separate_takes() {
    // SAFETY: fixture torn down exactly once at the end of this test.
    let fx = unsafe { Fixture::new("SIF_ViewState") };
    let key = [0xA1u8; 16];
    // SAFETY: fx's handles are live until the teardown below; each pointer
    // passed points at a local buffer of the length passed with it.
    unsafe {
        let payload = [1u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, payload.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let first = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].view_state, VIEW_NEW, "first sample must be NEW");
        assert_eq!(first[0].instance_state, INSTANCE_ALIVE);
        assert_ne!(first[0].instance_handle, 0);

        let payload2 = [2u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, payload2.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let second = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].view_state, VIEW_NOT_NEW,
            "same instance's second sample, taken separately, must be NOT_NEW"
        );
        assert_eq!(
            second[0].instance_handle, first[0].instance_handle,
            "same key must resolve to the same instance handle"
        );
    }
    // SAFETY: last use of every fixture handle was above.
    unsafe { fx.cleanup() };
}

/// (b) Instance-state DISPOSED (Spec §2.2.2.5.1.3 + §2.2.2.4.2.10): a
/// `dispose` lifecycle marker reports `NOT_ALIVE_DISPOSED`, `valid_data =
/// false`, and the SAME instance handle as the instance's own alive sample.
#[test]
fn instance_state_dispose_reports_not_alive_disposed() {
    // SAFETY: fixture torn down exactly once at the end of this test.
    let fx = unsafe { Fixture::new("SIF_Dispose") };
    let key = [0xB2u8; 16];
    // SAFETY: fx's handles are live until the teardown below; each pointer
    // passed points at a local buffer of the length passed with it.
    unsafe {
        let payload = [7u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, payload.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let alive = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(alive.len(), 1);
        assert!(alive[0].valid_data);

        assert_eq!(zerodds_dw_dispose(fx.writer, key.as_ptr(), 0), OK);
        let disposed = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(disposed.len(), 1);
        assert_eq!(disposed[0].instance_state, INSTANCE_DISPOSED);
        assert!(
            !disposed[0].valid_data,
            "a dispose marker carries no payload"
        );
        assert_eq!(
            disposed[0].instance_handle, alive[0].instance_handle,
            "the dispose marker must name the SAME instance as the alive sample"
        );
    }
    // SAFETY: last use of every fixture handle was above.
    unsafe { fx.cleanup() };
}

/// (c) Instance-state NOT_ALIVE_NO_WRITERS (Spec §2.2.2.5.1.3 +
/// §2.2.2.4.2.7): an explicit `unregister_instance` (no prior dispose)
/// reports `NOT_ALIVE_NO_WRITERS`, distinct from DISPOSED.
#[test]
fn instance_state_unregister_reports_not_alive_no_writers() {
    // SAFETY: fixture torn down exactly once at the end of this test.
    let fx = unsafe { Fixture::new("SIF_Unregister") };
    let key = [0xC3u8; 16];
    // SAFETY: fx's handles are live until the teardown below; each pointer
    // passed points at a local buffer of the length passed with it.
    unsafe {
        let payload = [9u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, payload.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let _alive = take_batch(fx.reader, 10, Duration::from_secs(15));

        let mut handle = 0u64;
        assert_eq!(
            zerodds_dw_register_instance(fx.writer, key.as_ptr(), 16, &mut handle),
            OK
        );
        assert_eq!(zerodds_dw_unregister_instance(fx.writer, handle), OK);
        let unregistered = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(unregistered.len(), 1);
        assert_eq!(
            unregistered[0].instance_state, INSTANCE_NO_WRITERS,
            "unregister_instance (no dispose) must report NOT_ALIVE_NO_WRITERS, not DISPOSED"
        );
        assert!(!unregistered[0].valid_data);
    }
    // SAFETY: last use of every fixture handle was above.
    unsafe { fx.cleanup() };
}

/// (d) Generation counts (Spec §2.2.2.5.1.4-5): `disposed_generation_count`
/// stays `0` across the dispose itself (dispose does not bump it — only a
/// REACTIVATION does), then increments to `1` on the write that reactivates
/// the instance, together with `view_state` going back to NEW and
/// `instance_state` back to ALIVE.
#[test]
fn disposed_generation_count_increments_on_reactivation() {
    // SAFETY: fixture torn down exactly once at the end of this test.
    let fx = unsafe { Fixture::new("SIF_Generation") };
    let key = [0xD4u8; 16];
    // SAFETY: fx's handles are live until the teardown below; each pointer
    // passed points at a local buffer of the length passed with it.
    unsafe {
        let payload = [1u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, payload.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let alive = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(alive[0].disposed_generation_count, 0);

        assert_eq!(zerodds_dw_dispose(fx.writer, key.as_ptr(), 0), OK);
        let disposed = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(
            disposed[0].disposed_generation_count, 0,
            "dispose itself must not bump the generation count"
        );

        let payload2 = [2u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, payload2.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let reactivated = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(reactivated.len(), 1);
        assert_eq!(
            reactivated[0].disposed_generation_count, 1,
            "the write that reactivates a disposed instance must bump the count"
        );
        assert_eq!(
            reactivated[0].view_state, VIEW_NEW,
            "reactivation is NEW again"
        );
        assert_eq!(reactivated[0].instance_state, INSTANCE_ALIVE);
        assert_eq!(
            reactivated[0].instance_handle, alive[0].instance_handle,
            "reactivation must keep the SAME instance handle"
        );
    }
    // SAFETY: last use of every fixture handle was above.
    unsafe { fx.cleanup() };
}

/// (e) `sample_rank`/`generation_rank` (Spec §2.2.2.5.1.6-7) over a batch of
/// three same-instance samples taken together in ONE `zerodds_dr_take`
/// call: `sample_rank` counts down (2, 1, 0); with no generation change in
/// the batch, `generation_rank` is `0` for every sample.
#[test]
fn sample_rank_counts_down_within_one_batch() {
    // SAFETY: fixture torn down exactly once at the end of this test.
    let fx = unsafe { Fixture::new("SIF_SampleRank") };
    let key = [0xE5u8; 16];
    // SAFETY: fx's handles are live until the teardown below; each pointer
    // passed points at a local buffer of the length passed with it.
    unsafe {
        for payload in [1u8, 2, 3] {
            let bytes = [payload];
            assert_eq!(
                zerodds_dw_write_keyed(fx.writer, bytes.as_ptr(), 1, key.as_ptr(), 16),
                OK
            );
        }
        let infos = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(infos.len(), 3, "all three samples must arrive in one batch");
        assert_eq!(infos[0].sample_rank, 2);
        assert_eq!(infos[1].sample_rank, 1);
        assert_eq!(
            infos[2].sample_rank, 0,
            "the last sample of the batch ranks 0"
        );
        for info in &infos {
            assert_eq!(
                info.generation_rank, 0,
                "no dispose/no-writers event occurred"
            );
            assert_eq!(info.absolute_generation_rank, 0);
        }
    }
    // SAFETY: last use of every fixture handle was above.
    unsafe { fx.cleanup() };
}

/// (f) `publication_handle` (Spec §2.2.2.5.1.11): an ALIVE sample's
/// `publication_handle` is non-zero and identical across two samples from
/// the SAME writer.
#[test]
fn publication_handle_is_nonzero_and_stable_per_writer() {
    // SAFETY: fixture torn down exactly once at the end of this test.
    let fx = unsafe { Fixture::new("SIF_PubHandle") };
    let key = [0xF6u8; 16];
    // SAFETY: fx's handles are live until the teardown below; each pointer
    // passed points at a local buffer of the length passed with it.
    unsafe {
        let p1 = [1u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, p1.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let first = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(first.len(), 1);
        assert_ne!(first[0].publication_handle, 0);

        let p2 = [2u8];
        assert_eq!(
            zerodds_dw_write_keyed(fx.writer, p2.as_ptr(), 1, key.as_ptr(), 16),
            OK
        );
        let second = take_batch(fx.reader, 10, Duration::from_secs(15));
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].publication_handle, first[0].publication_handle,
            "the same writer must report the same publication_handle"
        );
    }
    // SAFETY: last use of every fixture handle was above.
    unsafe { fx.cleanup() };
}
