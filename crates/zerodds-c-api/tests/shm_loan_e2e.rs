//! Stage 4 e2e: zero-copy same-host SHM loan through the public C ABI.
//!
//! Proves the full `flatdata-loan` path end to end without any RTPS wire or
//! discovery in the loop:
//!   1. A DataWriter enables the SHM loan (`zerodds_dw_enable_shm_loan`) →
//!      a POSIX shm segment is created at a flink path.
//!   2. `zerodds_dw_loan_message` hands back a pointer **into a shm slot**;
//!      the caller writes the payload straight into shared memory.
//!   3. `zerodds_dw_commit_loan` finalizes the slot in place — no staging copy.
//!   4. A DataReader maps the SAME named segment (`zerodds_dr_enable_shm`) and
//!      `zerodds_dr_take_shm` returns a read pointer into the reader's mapping
//!      of that segment — the bytes match what the writer wrote, although the
//!      sample never traversed the network. That is the zero-copy proof.
//!
//! Runs on any POSIX host (macOS + Linux); no multicast needed.

#![cfg(feature = "flatdata-loan")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::ffi::{CString, c_int};
use std::ptr;

use zerodds::ZeroDdsStatus;
use zerodds::extra_ffi::{
    zerodds_dw_commit_loan, zerodds_dw_discard_loan, zerodds_dw_loan_message,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_contained_entities,
};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;
use zerodds::shm_loan_ffi::{
    zerodds_dr_enable_shm, zerodds_dr_release_shm, zerodds_dr_take_shm, zerodds_dw_enable_shm_loan,
    zerodds_dw_set_delivery_mode,
};
use zerodds::subscriber_ffi::zerodds_sub_create_datareader;
use zerodds::{
    zerodds_reader_create, zerodds_reader_destroy, zerodds_reader_enable_shm,
    zerodds_reader_release_shm, zerodds_reader_take_shm, zerodds_runtime_create,
    zerodds_runtime_destroy, zerodds_writer_commit_loan, zerodds_writer_create,
    zerodds_writer_destroy, zerodds_writer_enable_shm_loan, zerodds_writer_loan_message,
};

/// The bytes at `p`, read one by one through volatile loads, so the read
/// happens after any write through another pointer to the same memory.
///
/// # Safety
/// `p` is valid for `n` bytes.
unsafe fn bytes_at(p: *const u8, n: usize) -> Vec<u8> {
    (0..n)
        // SAFETY: `i < n` and the caller guarantees that `p` is valid for
        // `n` bytes, so `p.add(i)` stays in that range and is readable; a
        // `u8` has no alignment to keep.
        .map(|i| unsafe { ptr::read_volatile(p.add(i)) })
        .collect()
}

/// The zero-copy proof of `zerodds-shm-transport-1.0` §7.1: the writer's
/// loan pointer `wptr` and the reader's take pointer `rptr` are two
/// addresses of the same bytes of the segment behind `path`, slot `slot`.
/// A third mapping of the segment, made here, sees the bytes the writer
/// wrote; a pattern written through `wptr` after the commit shows up through
/// `rptr` and through the third mapping; one written through the third
/// mapping shows up through both. A commit that copied a heap buffer into the
/// slot fails the second step, a take that copied the slot out fails both.
///
/// # Safety
/// `wptr` and `rptr` are valid for `len` bytes: the loaned and the taken
/// slot of a live writer and reader.
unsafe fn assert_one_set_of_pages(
    path: &str,
    slot: u32,
    wptr: *mut u8,
    rptr: *const u8,
    len: usize,
    sent: &[u8],
) {
    use zerodds_flatdata::{PosixSlotAllocator, SlotBackend, SlotHandle};
    let probe = PosixSlotAllocator::attach(path).expect("a third mapping of the segment");
    let (pptr, plen) = probe
        .slot_read_ptr(SlotHandle {
            segment_id: 0,
            slot_index: slot,
        })
        .expect("the taken slot in the third mapping");
    assert_eq!(plen, len, "the slot's sample size");
    // SAFETY: wptr, rptr (caller) and pptr (the probe's slot of `len`
    // bytes, mapped while `probe` lives) are valid for `len` bytes.
    unsafe {
        assert_eq!(
            bytes_at(pptr, len),
            sent,
            "the segment holds what the writer wrote"
        );
        let mut at = [wptr as usize, rptr as usize, pptr as usize];
        at.sort_unstable();
        assert!(
            at[0] != at[1] && at[1] != at[2],
            "three mappings, three addresses"
        );
        let through_writer: Vec<u8> = sent.iter().map(|b| !b).collect();
        ptr::copy_nonoverlapping(through_writer.as_ptr(), wptr, len);
        assert_eq!(
            bytes_at(rptr, len),
            through_writer,
            "a write through the loan pointer reaches the reader's pointer"
        );
        assert_eq!(bytes_at(pptr, len), through_writer);
        let through_probe: Vec<u8> = sent.iter().map(|b| b ^ 0x5A).collect();
        ptr::copy_nonoverlapping(through_probe.as_ptr(), pptr.cast_mut(), len);
        assert_eq!(
            bytes_at(rptr, len),
            through_probe,
            "the reader's pointer reads the segment, not a copy"
        );
        assert_eq!(bytes_at(wptr, len), through_probe);
    }
}

fn flink_path(tag: &str) -> String {
    let dir = std::env::temp_dir();
    let p = dir.join(format!("zddsloan_{}_{}", std::process::id(), tag));
    // A leftover from a crashed prior run would make shm create() fail.
    let _ = std::fs::remove_file(&p);
    p.to_string_lossy().into_owned()
}

#[test]
fn capi_2_6_shm_loan_writer_to_reader_zero_copy() {
    // The domain is leased: it is a UDP port (RTPS 9.6.1.4.1), and a fixed
    // one is shared with every other run of this test on the LAN
    // (`crates/test-domains`).
    let lease = zerodds_test_domains::lease_domain();
    let domain: u32 = lease.id() as u32;
    let topic_name = CString::new("ShmLoanE2E").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    let path = flink_path("zc");
    let c_path = CString::new(path.clone()).unwrap();

    // SAFETY: all FFI calls use valid, NULL-checked handles + C strings that
    // outlive the calls, per each extern fn's documented contract.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant create");

        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let dw = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        let dr = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!dw.is_null() && !dr.is_null(), "endpoint create");

        // 1) Enable the SHM loan on the writer (creates the segment) and map
        //    the same segment on the reader.
        assert_eq!(
            zerodds_dw_enable_shm_loan(dw, c_path.as_ptr(), 8, 256),
            ZeroDdsStatus::Ok as c_int,
            "enable_shm_loan"
        );
        assert_eq!(
            zerodds_dr_enable_shm(dr, c_path.as_ptr(), 0),
            ZeroDdsStatus::Ok as c_int,
            "dr_enable_shm"
        );

        // 2) Loan a slot → a pointer into shared memory; write the payload in.
        let payload: [u8; 16] = [
            0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4, 5, 6, 7, 8, 0xCA, 0xFE, 0xBA, 0xBE,
        ];
        let mut wptr: *mut u8 = ptr::null_mut();
        let mut wlen: usize = 0;
        assert_eq!(
            zerodds_dw_loan_message(dw, payload.len(), &mut wptr, &mut wlen),
            ZeroDdsStatus::Ok as c_int,
            "loan_message"
        );
        assert!(!wptr.is_null() && wlen == payload.len(), "loan ptr/len");
        ptr::copy_nonoverlapping(payload.as_ptr(), wptr, payload.len());

        // 3) Commit in place — no staging copy.
        assert_eq!(
            zerodds_dw_commit_loan(dw, wptr, payload.len()),
            ZeroDdsStatus::Ok as c_int,
            "commit_loan"
        );

        // 4) Reader takes the slot zero-copy from the shared segment.
        let mut rptr: *const u8 = ptr::null();
        let mut rlen: usize = 0;
        let mut slot: u32 = u32::MAX;
        assert_eq!(
            zerodds_dr_take_shm(dr, &mut rptr, &mut rlen, &mut slot),
            ZeroDdsStatus::Ok as c_int,
            "take_shm"
        );
        assert!(!rptr.is_null(), "read ptr");
        assert_eq!(rlen, payload.len(), "read len");
        let read_back = std::slice::from_raw_parts(rptr, rlen);
        assert_eq!(
            read_back,
            &payload[..],
            "bytes match across the SHM mapping"
        );

        // Zero-copy proof: the reader's pointer is in its own mapping of the
        // segment, NOT the writer's loan pointer (separate mmap, shared pages),
        // and both address the segment's bytes, not a copy of them.
        assert_ne!(
            rptr as usize, wptr as usize,
            "reader reads from its own mapping, not the writer's address"
        );
        assert_one_set_of_pages(&path, slot, wptr, rptr, rlen, &payload);

        // Release the slot; with one reader and no fresh sample, the next take
        // reports NoData.
        assert_eq!(
            zerodds_dr_release_shm(dr, slot),
            ZeroDdsStatus::Ok as c_int,
            "release_shm"
        );
        let mut r2: *const u8 = ptr::null();
        let mut l2: usize = 0;
        let mut s2: u32 = 0;
        assert_eq!(
            zerodds_dr_take_shm(dr, &mut r2, &mut l2, &mut s2),
            ZeroDdsStatus::NoData as c_int,
            "no second sample pending"
        );

        // A second sample flows through the same ring, observed zero-copy.
        let payload2: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
        let mut w2: *mut u8 = ptr::null_mut();
        let mut wl2: usize = 0;
        assert_eq!(
            zerodds_dw_loan_message(dw, payload2.len(), &mut w2, &mut wl2),
            ZeroDdsStatus::Ok as c_int
        );
        ptr::copy_nonoverlapping(payload2.as_ptr(), w2, payload2.len());
        assert_eq!(
            zerodds_dw_commit_loan(dw, w2, payload2.len()),
            ZeroDdsStatus::Ok as c_int
        );
        let mut r3: *const u8 = ptr::null();
        let mut l3: usize = 0;
        let mut s3: u32 = 0;
        assert_eq!(
            zerodds_dr_take_shm(dr, &mut r3, &mut l3, &mut s3),
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(std::slice::from_raw_parts(r3, l3), &payload2[..]);
        assert_eq!(zerodds_dr_release_shm(dr, s3), ZeroDdsStatus::Ok as c_int);

        zerodds_dp_delete_contained_entities(dp);
        zerodds_dpf_delete_participant(f, dp);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn capi_13_2_delivery_mode_setter_validation() {
    // The domain is leased: it is a UDP port (RTPS 9.6.1.4.1), and a fixed
    // one is shared with every other run of this test on the LAN
    // (`crates/test-domains`).
    let lease = zerodds_test_domains::lease_domain();
    let domain: u32 = lease.id() as u32;
    let topic_name = CString::new("DeliveryModeSet").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: valid handles + C strings outliving the calls per each contract.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let dw = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        assert!(!dw.is_null());

        assert_eq!(
            zerodds_dw_set_delivery_mode(dw, 0),
            ZeroDdsStatus::Ok as c_int,
            "Portable"
        );
        assert_eq!(
            zerodds_dw_set_delivery_mode(dw, 1),
            ZeroDdsStatus::Ok as c_int,
            "RawSameHost"
        );
        // Iceoryx (2): Unsupported unless the iceoryx backend is compiled in.
        #[cfg(not(feature = "delivery-iceoryx"))]
        assert_eq!(
            zerodds_dw_set_delivery_mode(dw, 2),
            ZeroDdsStatus::Unsupported as c_int,
            "Iceoryx not wired without the feature"
        );
        #[cfg(feature = "delivery-iceoryx")]
        assert_eq!(
            zerodds_dw_set_delivery_mode(dw, 2),
            ZeroDdsStatus::Ok as c_int,
            "Iceoryx accepted with the feature"
        );
        assert_eq!(
            zerodds_dw_set_delivery_mode(dw, 99),
            ZeroDdsStatus::BadParameter as c_int,
            "junk value"
        );
        assert_eq!(
            zerodds_dw_set_delivery_mode(ptr::null_mut(), 0),
            ZeroDdsStatus::BadParameter as c_int,
            "null writer"
        );

        zerodds_dp_delete_contained_entities(dp);
        zerodds_dpf_delete_participant(f, dp);
    }
}

#[test]
fn capi_13_2_raw_same_host_mode_writer_to_reader() {
    // The domain is leased: it is a UDP port (RTPS 9.6.1.4.1), and a fixed
    // one is shared with every other run of this test on the LAN
    // (`crates/test-domains`).
    let lease = zerodds_test_domains::lease_domain();
    let domain: u32 = lease.id() as u32;
    let topic_name = CString::new("RawModeE2E").unwrap();
    let type_name = CString::new("RawBytes").unwrap();
    let path = flink_path("raw");
    let c_path = CString::new(path.clone()).unwrap();

    // SAFETY: valid handles + C strings outliving the calls per each contract.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let dw = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        let dr = zerodds_sub_create_datareader(subscriber, topic, ptr::null());
        assert!(!dw.is_null() && !dr.is_null());

        assert_eq!(
            zerodds_dw_set_delivery_mode(dw, 1),
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            zerodds_dw_enable_shm_loan(dw, c_path.as_ptr(), 8, 128),
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            zerodds_dr_enable_shm(dr, c_path.as_ptr(), 0),
            ZeroDdsStatus::Ok as c_int
        );

        // The producer writes the in-memory form (here a small fixed pattern
        // standing in for a #[repr(C)] struct).
        let payload: [u8; 8] = [0x2a, 0, 0, 0, 0xFF, 0xEE, 0xDD, 0xCC];
        let mut wptr: *mut u8 = ptr::null_mut();
        let mut wlen: usize = 0;
        assert_eq!(
            zerodds_dw_loan_message(dw, payload.len(), &mut wptr, &mut wlen),
            ZeroDdsStatus::Ok as c_int
        );
        ptr::copy_nonoverlapping(payload.as_ptr(), wptr, payload.len());
        assert_eq!(
            zerodds_dw_commit_loan(dw, wptr, payload.len()),
            ZeroDdsStatus::Ok as c_int
        );

        let mut rptr: *const u8 = ptr::null();
        let mut rlen: usize = 0;
        let mut slot: u32 = u32::MAX;
        assert_eq!(
            zerodds_dr_take_shm(dr, &mut rptr, &mut rlen, &mut slot),
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(rlen, payload.len());
        assert_eq!(std::slice::from_raw_parts(rptr, rlen), &payload[..]);
        assert_eq!(zerodds_dr_release_shm(dr, slot), ZeroDdsStatus::Ok as c_int);

        zerodds_dp_delete_contained_entities(dp);
        zerodds_dpf_delete_participant(f, dp);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn capi_2_6_loan_without_shm_falls_back_to_heap() {
    // The domain is leased: it is a UDP port (RTPS 9.6.1.4.1), and a fixed
    // one is shared with every other run of this test on the LAN
    // (`crates/test-domains`).
    let lease = zerodds_test_domains::lease_domain();
    let domain: u32 = lease.id() as u32;
    let topic_name = CString::new("ShmLoanFallback").unwrap();
    let type_name = CString::new("RawBytes").unwrap();

    // SAFETY: valid handles + C strings outliving the calls per each contract.
    unsafe {
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        assert!(!dp.is_null(), "participant");
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name.as_ptr(), ptr::null());
        assert!(!topic.is_null(), "topic");
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        assert!(!publisher.is_null(), "publisher");
        let dw = zerodds_pub_create_datawriter(publisher, topic, ptr::null());
        assert!(!dw.is_null(), "datawriter");

        let mut ptr_out: *mut u8 = ptr::null_mut();
        let mut len_out: usize = 0;
        assert_eq!(
            zerodds_dw_loan_message(dw, 8, &mut ptr_out, &mut len_out),
            ZeroDdsStatus::Ok as c_int
        );
        assert!(!ptr_out.is_null() && len_out == 8);
        for i in 0..8usize {
            *ptr_out.add(i) = i as u8;
        }
        // Discard (heap path) also works through the transparent hook.
        assert_eq!(
            zerodds_dw_discard_loan(dw, ptr_out, 8),
            ZeroDdsStatus::Ok as c_int
        );

        zerodds_dp_delete_contained_entities(dp);
        zerodds_dpf_delete_participant(f, dp);
    }
}

/// The runtime-path handles (`ZeroDdsWriter`/`ZeroDdsReader`) are what the
/// ROS-2 RMW bridge uses. This proves the same `(runtime, eid)`-keyed SHM loan
/// works there too: `zerodds_writer_enable_shm_loan` + the transparent
/// `zerodds_writer_loan_message`/`commit_loan`, and `zerodds_reader_enable_shm`
/// + `zerodds_reader_take_shm` read the slot zero-copy.
#[test]
fn capi_2_6_shm_loan_runtime_path_writer_to_reader_zero_copy() {
    // The domain is leased: it is a UDP port (RTPS 9.6.1.4.1), and a fixed
    // one is shared with every other run of this test on the LAN
    // (`crates/test-domains`).
    let lease = zerodds_test_domains::lease_domain();
    let domain: u32 = lease.id() as u32;
    let topic = CString::new("ShmLoanRtE2E").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let path = flink_path("rt");
    let c_path = CString::new(path.clone()).unwrap();

    // SAFETY: valid handles + C strings outliving the calls per each contract.
    unsafe {
        let rt = zerodds_runtime_create(domain);
        assert!(!rt.is_null(), "runtime");
        let writer = zerodds_writer_create(rt, topic.as_ptr(), typ.as_ptr(), 1);
        let reader = zerodds_reader_create(rt, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null(), "endpoints");

        assert_eq!(
            zerodds_writer_enable_shm_loan(writer, c_path.as_ptr(), 8, 256),
            ZeroDdsStatus::Ok as c_int,
            "writer_enable_shm_loan"
        );
        assert_eq!(
            zerodds_reader_enable_shm(reader, c_path.as_ptr(), 0),
            ZeroDdsStatus::Ok as c_int,
            "reader_enable_shm"
        );

        // Loan a slot through the runtime path → pointer into shared memory.
        let payload: [u8; 12] = [9, 8, 7, 6, 5, 4, 3, 2, 1, 0xAA, 0xBB, 0xCC];
        let mut wptr: *mut u8 = ptr::null_mut();
        let mut wlen: usize = 0;
        assert_eq!(
            zerodds_writer_loan_message(writer, payload.len(), &mut wptr, &mut wlen),
            ZeroDdsStatus::Ok as c_int,
            "writer_loan_message"
        );
        assert!(!wptr.is_null() && wlen == payload.len());
        ptr::copy_nonoverlapping(payload.as_ptr(), wptr, payload.len());
        assert_eq!(
            zerodds_writer_commit_loan(writer, wptr, payload.len()),
            ZeroDdsStatus::Ok as c_int,
            "writer_commit_loan"
        );

        // Reader takes it zero-copy from the shared segment.
        let mut rptr: *const u8 = ptr::null();
        let mut rlen: usize = 0;
        let mut slot: u32 = u32::MAX;
        assert_eq!(
            zerodds_reader_take_shm(reader, &mut rptr, &mut rlen, &mut slot),
            ZeroDdsStatus::Ok as c_int,
            "reader_take_shm"
        );
        assert!(!rptr.is_null() && rlen == payload.len());
        assert_eq!(
            std::slice::from_raw_parts(rptr, rlen),
            &payload[..],
            "runtime-path bytes match across the SHM mapping"
        );
        assert_one_set_of_pages(&path, slot, wptr, rptr, rlen, &payload);
        assert_eq!(
            zerodds_reader_release_shm(reader, slot),
            ZeroDdsStatus::Ok as c_int
        );

        zerodds_writer_destroy(writer);
        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt);
    }
    let _ = std::fs::remove_file(&path);
}

/// `Iceoryx` delivery mode end-to-end through the runtime FFI: the writer
/// publishes over an iceoryx2 service, a subscriber on the same service receives
/// it. Proves the cross-stack same-host path (delivery-modes-1.0 §3.3).
#[cfg(feature = "delivery-iceoryx")]
#[test]
fn iceoryx_mode_writer_to_reader() {
    use zerodds::{zerodds_reader_enable_iceoryx, zerodds_writer_enable_iceoryx};

    // The domain is leased: it is a UDP port (RTPS 9.6.1.4.1), and a fixed
    // one is shared with every other run of this test on the LAN
    // (`crates/test-domains`).
    let lease = zerodds_test_domains::lease_domain();
    let domain: u32 = lease.id() as u32;
    let topic = CString::new("IceoryxModeE2E").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    // iceoryx2 service names are plain identifiers — keep it unique per run.
    let service = CString::new(format!("zerodds_capi_ice_{}", std::process::id())).unwrap();

    // SAFETY: valid handles + C strings outliving the calls per each contract.
    unsafe {
        let rt = zerodds_runtime_create(domain);
        assert!(!rt.is_null(), "runtime");
        let writer = zerodds_writer_create(rt, topic.as_ptr(), typ.as_ptr(), 1);
        let reader = zerodds_reader_create(rt, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null());

        assert_eq!(
            zerodds_writer_enable_iceoryx(writer, service.as_ptr(), 64),
            ZeroDdsStatus::Ok as c_int,
            "writer_enable_iceoryx"
        );
        assert_eq!(
            zerodds_reader_enable_iceoryx(reader, service.as_ptr()),
            ZeroDdsStatus::Ok as c_int,
            "reader_enable_iceoryx"
        );

        let payload: [u8; 6] = [1, 2, 3, 4, 5, 6];
        let mut wptr: *mut u8 = ptr::null_mut();
        let mut wlen: usize = 0;
        assert_eq!(
            zerodds_writer_loan_message(writer, payload.len(), &mut wptr, &mut wlen),
            ZeroDdsStatus::Ok as c_int
        );
        ptr::copy_nonoverlapping(payload.as_ptr(), wptr, payload.len());
        assert_eq!(
            zerodds_writer_commit_loan(writer, wptr, payload.len()),
            ZeroDdsStatus::Ok as c_int
        );

        // iceoryx2 is synchronous on the same-process path.
        let mut rptr: *const u8 = ptr::null();
        let mut rlen: usize = 0;
        let mut slot: u32 = u32::MAX;
        assert_eq!(
            zerodds_reader_take_shm(reader, &mut rptr, &mut rlen, &mut slot),
            ZeroDdsStatus::Ok as c_int,
            "reader_take_shm (iceoryx)"
        );
        assert_eq!(rlen, payload.len());
        assert_eq!(std::slice::from_raw_parts(rptr, rlen), &payload[..]);
        assert_eq!(
            zerodds_reader_release_shm(reader, slot),
            ZeroDdsStatus::Ok as c_int
        );

        zerodds_writer_destroy(writer);
        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt);
    }
}
