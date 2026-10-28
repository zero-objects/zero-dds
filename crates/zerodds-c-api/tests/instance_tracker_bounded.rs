//! Regression test: the reader-side `InstanceTracker` stays bounded.
//!
//! # Background
//!
//! An old, never-pushed fix (`origin/tmp/p5-c-api-instance-leak`, commit
//! `1614f1acfcd`) documented a leak in the untyped C-API take path:
//! `resolve_instances_and_ownership` used to register a full-key instance
//! for EVERY Alive sample on a **keyless** topic with a **SHARED**-ownership
//! reader (the default), deriving the "key" from the whole payload. Measured
//! there: 258 MB in 545 265 allocations after 613 s on a keyless topic with
//! OWNERSHIP=SHARED.
//!
//! HEAD fixes this differently (see
//! `git log -S "Keyless SHARED readers retain no identities" -- \
//! crates/zerodds-c-api/src/subscriber_ffi.rs`, commits `c7c78181479` and
//! `9c044cfc8ea`): in
//! `crates/zerodds-c-api/src/subscriber_ffi.rs::resolve_instances_and_ownership`,
//! a keyless (`!keyed`) SHARED (`!exclusive`) reader takes the `else { 0 }`
//! branch of
//! ```ignore
//! let handle = if keyed || exclusive {
//!     drr.instances.ensure_registered_limited(kh, limit)...
//! } else {
//!     0
//! };
//! ```
//! and never touches `drr.instances` at all. A keyless EXCLUSIVE reader
//! still needs ONE instance for ownership arbitration (Spec §2.2.3.23), so
//! it registers exactly one (the constant `KEYLESS_KEY_HASH`). A keyed
//! reader's identities are bounded by `ResourceLimits.max_instances` via the
//! same `ensure_registered_limited`, which returns `None` once the cap is
//! hit; that sample is refused on its own and counted in SAMPLE_REJECTED
//! (REJECTED_BY_INSTANCES_LIMIT) on a RELIABLE reader — never a silent
//! truncation, and never at the cost of the admissible samples beside it.
//!
//! There was no test pinning this down. This file adds one. A keyless
//! reader of either ownership now tracks the topic's one instance (DDS 1.4
//! §2.2.1.2.2) under `KEYLESS_KEY_HASH`; what must stay bounded is that it
//! is one entry and not one per payload.
//!
//! # Observation method
//!
//! `ZeroDdsDataReader` (`crates/zerodds-c-api/src/entities.rs`) is a `pub`
//! struct in the `pub mod entities`, and its `instances` field
//! (`zerodds_dcps::instance_tracker::InstanceTracker`) is `pub`. That
//! tracker has a `pub fn len(&self) -> usize`
//! (`crates/dcps/src/instance_tracker.rs`). All of that is already public
//! Rust API of the `zerodds` (crate `zerodds-c-api`) library — not test-only
//! — so `instance_count()` below reads the tracker size directly off the
//! `*mut ZeroDdsDataReader` the C-ABI itself hands back from
//! `zerodds_sub_create_datareader_kind`, with **no new production code**.
//!
//! This is deliberately NOT `zerodds_dr_get_matched_publications` /
//! `zerodds_dr_get_matched_publication_data` (`crates/zerodds-c-api/include/zerodds.h`):
//! those report matched DataWriters, not registered per-instance
//! identities, and would not have caught the leak (the leak scales with
//! samples/keys, not with matched writers). No dedicated "instance count"
//! C-ABI function exists in `zerodds.h`, and adding a
//! `#[doc(hidden)]` FFI helper to `subscriber_ffi.rs` was therefore
//! considered but rejected as unnecessary: the direct field/method access
//! already satisfies "public API, minimal, no new production code".

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

use zerodds::entities::ZeroDdsDataReader;
use zerodds::extra_ffi::{zerodds_dr_get_qos, zerodds_pub_get_default_datawriter_qos};
use zerodds::extra_ffi::{zerodds_dw_get_qos, zerodds_dw_set_qos};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_publisher, zerodds_dp_delete_subscriber, zerodds_dp_delete_topic,
};
use zerodds::publisher_ffi::{
    zerodds_dw_wait_for_matched, zerodds_dw_write, zerodds_dw_write_keyed,
    zerodds_pub_create_datawriter, zerodds_pub_delete_datawriter,
};
use zerodds::qos_ffi::{ZeroDdsDataReaderQos, ZeroDdsDataWriterQos};
use zerodds::subscriber_ffi::{
    ZeroDdsSampleArray, ZeroDdsSampleRejectedStatus, zerodds_dr_get_sample_rejected_status,
    zerodds_dr_return_loan, zerodds_dr_take, zerodds_sub_create_datareader_kind,
    zerodds_sub_delete_datareader,
};

// `ZeroDdsStatus` discriminants (crates/zerodds-c-api/src/lib.rs) — repeated
// here as plain constants because the enum's variants are not part of this
// crate's re-exported names used above (kept minimal, no extra import).
/// HISTORY kind KEEP_ALL in the C QoS.
const KEEP_ALL: u32 = 1;
const OK: i32 = 0;
const NO_DATA: i32 = -7;
/// `ZeroDdsReliabilityQosPolicy::kind` for RELIABLE.
const RELIABLE: u32 = 2;

/// Reads the reader-side `InstanceTracker` size directly. See the module
/// doc comment ("Observation method") for why this is the chosen probe.
///
/// # Safety
/// `dr` must be a live pointer from `zerodds_sub_create_datareader_kind`.
unsafe fn instance_count(dr: *mut ZeroDdsDataReader) -> usize {
    // SAFETY: caller pledge (see fn doc).
    unsafe { (*dr).instances.len() }
}

/// Drains `want` samples via `zerodds_dr_take` (unbounded batches), freeing
/// each loan. Expects every batch to succeed (`OK`) — no resource errors.
/// Returns the total number of samples actually received.
///
/// # Safety
/// `dr` valid and enabled.
unsafe fn take_all_ok(dr: *mut ZeroDdsDataReader, want: usize, budget: Duration) -> usize {
    let deadline = Instant::now() + budget;
    let mut received = 0usize;
    while received < want && Instant::now() < deadline {
        // SAFETY: ZeroDdsSampleArray is repr(C) with only raw pointers and a
        // usize count; all-zero is null pointers and a count of 0, valid bits.
        let mut arr: ZeroDdsSampleArray = unsafe { core::mem::zeroed() };
        // SAFETY: dr valid (caller pledge); arr is a stack-local, freshly
        // zeroed out-param exactly as `zerodds_dr_take`'s `# Safety` expects.
        let rc = unsafe { zerodds_dr_take(dr, &mut arr, 0, 0, 0, 0) };
        match rc {
            OK => {
                received += arr.count;
                // SAFETY: arr came from the zerodds_dr_take call above.
                unsafe {
                    zerodds_dr_return_loan(dr, &mut arr);
                }
            }
            NO_DATA => std::thread::sleep(Duration::from_millis(5)),
            other => panic!("zerodds_dr_take: unexpected status {other}"),
        }
    }
    received
}

/// Fetches a real default `DataReaderQos` snapshot by creating a throwaway
/// reader with `qos = NULL`, reading it back via `zerodds_dr_get_qos`, and
/// deleting the throwaway reader again. Mirrors how a real C caller builds
/// "defaults with one field changed" and keeps every field HEAD actually
/// defaults to (reliability, durability, history, ...) instead of an
/// all-zero guess.
///
/// # Safety
/// `sub`, `topic` valid.
unsafe fn default_reader_qos(
    sub: *mut zerodds::entities::ZeroDdsSubscriber,
    topic: *mut zerodds::entities::ZeroDdsTopic,
    is_keyed: i32,
) -> ZeroDdsDataReaderQos {
    // SAFETY: sub+topic valid (caller pledge); qos=NULL is NULL-tolerant.
    let scratch = unsafe { zerodds_sub_create_datareader_kind(sub, topic, ptr::null(), is_keyed) };
    assert!(!scratch.is_null(), "scratch reader create failed");
    // SAFETY: ZeroDdsDataReaderQos is a repr(C) plain-data struct; the
    // zeroed value is only an out-param that zerodds_dr_get_qos overwrites.
    let mut qos: ZeroDdsDataReaderQos = unsafe { core::mem::zeroed() };
    // SAFETY: scratch valid; qos is a valid stack out-param.
    let rc = unsafe { zerodds_dr_get_qos(scratch, &mut qos) };
    assert_eq!(rc, OK, "zerodds_dr_get_qos failed with {rc}");
    // SAFETY: scratch from the create call above, same subscriber.
    unsafe {
        zerodds_sub_delete_datareader(sub, scratch);
    }
    qos
}

/// (a) Keyless topic, reader OWNERSHIP=SHARED (the default): 2 000 samples
/// with distinct payloads must leave the tracker with ONE instance. This is exactly the
/// leaked scenario in `1614f1acfcd` — the old code derived a per-payload
/// "key" for every Alive sample regardless of keyed-ness/ownership.
#[test]
fn keyless_shared_reader_tracks_one_instance() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_name = CString::new("InstTrackerKeylessShared").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: every pointer below is NULL-checked before use, per each
    // callee's own `# Safety` contract; CStrings live to the end of the block.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant create failed");

        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        assert!(!topic.is_null(), "topic create failed");

        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        assert!(!publisher.is_null() && !subscriber.is_null());

        let writer = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        // is_keyed = 0: the reader's endpoint is NO_KEY, so the take path
        // must treat every sample as the topic's single instance. KEEP_ALL:
        // the single instance keeps every sample (DDS 1.4 §2.2.3.18).
        let mut qos = default_reader_qos(subscriber, topic, 0);
        qos.history.kind = KEEP_ALL;
        let reader = zerodds_sub_create_datareader_kind(subscriber, topic, &qos, 0);
        assert!(!writer.is_null() && !reader.is_null());

        assert_eq!(
            zerodds_dw_wait_for_matched(writer, 1, 5_000),
            OK,
            "writer did not match the same-participant reader"
        );

        assert_eq!(instance_count(reader), 0, "tracker must start empty");

        const N: u32 = 2_000;
        for i in 0..N {
            // Distinct payloads: the leaked code hashed the WHOLE payload as
            // a fake key, so identical payloads would not have exercised it.
            let payload = i.to_le_bytes();
            let rc = zerodds_dw_write(writer, payload.as_ptr(), payload.len(), 0);
            assert_eq!(rc, OK, "write {i} failed with {rc}");
        }

        let received = take_all_ok(reader, N as usize, Duration::from_secs(30));
        assert_eq!(
            received, N as usize,
            "expected all {N} samples delivered before asserting the tracker size"
        );

        // DDS 1.4 §2.2.1.2.2: the topic has one instance, so the tracker
        // holds that one and no entry per payload (regression: 1614f1acfcd).
        assert_eq!(
            instance_count(reader),
            1,
            "keyless SHARED reader must track exactly the topic's one instance \
             after taking {N} distinct-payload samples"
        );

        zerodds_sub_delete_datareader(subscriber, reader);
        zerodds_pub_delete_datawriter(publisher, writer);
        zerodds_dp_delete_subscriber(dp, subscriber);
        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, topic);
        zerodds_dpf_delete_participant(f, dp);
    }
}

/// (b) Keyless topic, reader OWNERSHIP=EXCLUSIVE: ownership arbitration
/// (Spec §2.2.3.23) needs exactly ONE instance slot (the constant
/// `KEYLESS_KEY_HASH`) — the tracker must never grow past that, regardless
/// of how many samples with distinct payloads are taken.
#[test]
fn keyless_exclusive_reader_tracks_exactly_one_instance() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_name = CString::new("InstTrackerKeylessExclusive").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: see the first test — identical contract.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant create failed");

        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        assert!(!topic.is_null(), "topic create failed");

        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        assert!(!publisher.is_null() && !subscriber.is_null());

        // OWNERSHIP is RxO and must be equal on both sides (DDS 1.4
        // §2.2.3.23): the writer is EXCLUSIVE as well.
        let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
        assert_eq!(
            zerodds_pub_get_default_datawriter_qos(publisher, &mut wqos),
            OK
        );
        wqos.ownership.kind = 1;
        let writer = zerodds_pub_create_datawriter(publisher, topic, &wqos);
        assert!(!writer.is_null());

        let mut qos = default_reader_qos(subscriber, topic, 0);
        qos.ownership.kind = 1; // Exclusive (0 = Shared, 1 = Exclusive).
        qos.history.kind = KEEP_ALL; // every sample of the one instance
        let reader = zerodds_sub_create_datareader_kind(subscriber, topic, &qos, 0);
        assert!(!reader.is_null());

        assert_eq!(
            zerodds_dw_wait_for_matched(writer, 1, 5_000),
            OK,
            "writer did not match the same-participant reader"
        );

        const N: u32 = 500;
        for i in 0..N {
            let payload = i.to_le_bytes();
            let rc = zerodds_dw_write(writer, payload.as_ptr(), payload.len(), 0);
            assert_eq!(rc, OK, "write {i} failed with {rc}");
        }

        let received = take_all_ok(reader, N as usize, Duration::from_secs(30));
        assert_eq!(received, N as usize, "expected all {N} samples delivered");

        assert_eq!(
            instance_count(reader),
            1,
            "keyless EXCLUSIVE reader must arbitrate exactly ONE instance \
             (all samples share the all-zero KEYLESS_KEY_HASH)"
        );

        zerodds_sub_delete_datareader(subscriber, reader);
        zerodds_pub_delete_datawriter(publisher, writer);
        zerodds_dp_delete_subscriber(dp, subscriber);
        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, topic);
        zerodds_dpf_delete_participant(f, dp);
    }
}

/// (c) Keyed topic, RELIABLE reader, `ResourceLimits.max_instances = 8`,
/// 50 distinct KeyHashes: the tracker never exceeds 8 entries, `take()`
/// hands out exactly the 8 admitted samples, and each of the other 42 is
/// counted in SAMPLE_REJECTED with REJECTED_BY_INSTANCES_LIMIT (DDS 1.4
/// §2.2.3.19, §2.2.4.2.6.3). `take()` used to fail with `OutOfResources`
/// for a batch holding a refused sample and drop the admissible ones in it.
///
/// Samples are taken in unbounded batches, so a batch mixes admitted and
/// refused samples whenever the same-participant delivery puts them into
/// the channel together — the case the old contract lost data in.
#[test]
fn keyed_reader_resource_limit_bounds_tracker() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_name = CString::new("InstTrackerKeyedBounded").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: see the first test — identical contract.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant create failed");

        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        assert!(!topic.is_null(), "topic create failed");

        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        assert!(!publisher.is_null() && !subscriber.is_null());

        // Default writer is WITH_KEY (zerodds_pub_create_datawriter ==
        // register_user_writer, "Default: WithKey" per crates/dcps/src/runtime.rs).
        //
        // `zerodds_dw_write_keyed` enforces its OWN writer-side limit via
        // the same `ensure_registered_limited`
        // (crates/zerodds-c-api/src/publisher_ffi.rs), independent of the
        // reader-side tracker under test here. Set to unlimited (the
        // §2.2.3.19 default) explicitly so the writer registers all 50 keys
        // whatever the default; the cap under test is the READER's
        // `resource_limits.max_instances = 8` set below.
        // RESOURCE_LIMITS and RELIABILITY are not changeable once the writer
        // exists (DDS 1.4 §2.2.3), so they go in at the creation; a
        // `set_qos` of them answers IMMUTABLE_POLICY.
        let writer = {
            let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            let rc = zerodds_pub_get_default_datawriter_qos(publisher, &mut wqos);
            assert_eq!(
                rc, OK,
                "zerodds_pub_get_default_datawriter_qos failed with {rc}"
            );
            wqos.resource_limits.max_instances = -1; // LENGTH_UNLIMITED
            wqos.reliability.kind = RELIABLE;
            zerodds_pub_create_datawriter(publisher, topic, &wqos)
        };
        assert!(!writer.is_null());
        {
            let mut wqos: ZeroDdsDataWriterQos = core::mem::zeroed();
            assert_eq!(zerodds_dw_get_qos(writer, &mut wqos), OK);
            assert_eq!(wqos.resource_limits.max_instances, -1);
            wqos.resource_limits.max_instances = 4;
            assert_ne!(
                zerodds_dw_set_qos(writer, &wqos),
                OK,
                "an immutable policy changed"
            );
        }

        let mut qos = default_reader_qos(subscriber, topic, 1);
        qos.resource_limits.max_instances = 8;
        qos.reliability.kind = RELIABLE;
        let reader = zerodds_sub_create_datareader_kind(subscriber, topic, &qos, 1);
        assert!(!reader.is_null());

        assert_eq!(
            zerodds_dw_wait_for_matched(writer, 1, 5_000),
            OK,
            "writer did not match the same-participant reader"
        );

        const KEYS: usize = 50;
        for k in 0..KEYS {
            let mut key = [0u8; 16];
            key[0..8].copy_from_slice(&(k as u64).to_le_bytes());
            let payload = [k as u8, 0xAA];
            let rc =
                zerodds_dw_write_keyed(writer, payload.as_ptr(), payload.len(), key.as_ptr(), 16);
            assert_eq!(rc, OK, "write_keyed {k} failed with {rc}");
        }

        let rejected = |reader| {
            let mut st: ZeroDdsSampleRejectedStatus = core::mem::zeroed();
            let rc = zerodds_dr_get_sample_rejected_status(reader, &mut st);
            assert_eq!(
                rc, OK,
                "zerodds_dr_get_sample_rejected_status failed with {rc}"
            );
            st
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut ok_count = 0usize;
        while (ok_count < 8 || (rejected(reader).total_count as usize) < KEYS - 8)
            && Instant::now() < deadline
        {
            let mut arr: ZeroDdsSampleArray = core::mem::zeroed();
            let rc = zerodds_dr_take(reader, &mut arr, 0, 0, 0, 0);
            match rc {
                OK => {
                    ok_count += arr.count;
                    zerodds_dr_return_loan(reader, &mut arr);
                }
                NO_DATA => std::thread::sleep(Duration::from_millis(5)),
                other => panic!("zerodds_dr_take: unexpected status {other}"),
            }
        }

        let status = rejected(reader);
        assert_eq!(
            ok_count, 8,
            "exactly the first 8 distinct keys must be delivered under max_instances=8"
        );
        assert_eq!(
            status.total_count as usize,
            KEYS - 8,
            "the remaining {} distinct keys must each be counted as rejected",
            KEYS - 8
        );
        assert_eq!(status.last_reason, 1, "REJECTED_BY_INSTANCES_LIMIT");
        assert_eq!(
            instance_count(reader),
            8,
            "tracker size must equal the admitted 8 instances, never exceeding max_instances"
        );

        zerodds_sub_delete_datareader(subscriber, reader);
        zerodds_pub_delete_datawriter(publisher, writer);
        zerodds_dp_delete_subscriber(dp, subscriber);
        zerodds_dp_delete_publisher(dp, publisher);
        zerodds_dp_delete_topic(dp, topic);
        zerodds_dpf_delete_participant(f, dp);
    }
}
