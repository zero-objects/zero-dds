// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Boundary tests for every C API call that writes a caller buffer bounded
//! by a capacity parameter: a list `cap`, a byte buffer `inout_len`, or the
//! locator list's `capacity`. Each call is exercised with an empty result, a
//! buffer that is exactly full, and a result one entry/byte larger than the
//! buffer holds.
//!
//! The list calls follow `docs/specs/zerodds-c-api-1.0.md` §2.8: the total is
//! reported in `*out_count` and the call returns `ZERODDS_OUT_OF_RESOURCES`
//! when the buffer is too small. `zerodds_reader_take_into` and the two
//! `*_get_key_value` calls report the needed length the same way; the two
//! `*_unicast_locators` calls return their total count, which may exceed the
//! capacity. `tests/buffer_bounds_gate.rs` keeps this set complete.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use zerodds::ZeroDdsStatus;
use zerodds::builtin_ffi::zerodds_dp_get_discovered_topics;
use zerodds::entities::ZeroDdsDomainParticipant;
use zerodds::extra_ffi::{
    zerodds_dr_get_matched_publications, zerodds_dw_get_matched_subscriptions,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{
    zerodds_dp_create_publisher, zerodds_dp_create_subscriber, zerodds_dp_create_topic,
    zerodds_dp_delete_contained_entities, zerodds_dp_get_discovered_participants,
};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter;
use zerodds::subscriber_ffi::{zerodds_sub_create_datareader, zerodds_sub_get_datareaders};
use zerodds::{
    ZeroDdsLocator, zerodds_reader_create, zerodds_reader_destroy, zerodds_reader_take_into,
    zerodds_reader_unicast_locators, zerodds_runtime_create, zerodds_runtime_destroy,
    zerodds_runtime_wait_for_peers, zerodds_writer_create, zerodds_writer_destroy,
    zerodds_writer_unicast_locators, zerodds_writer_wait_for_matched, zerodds_writer_write,
};

const OK: i32 = ZeroDdsStatus::Ok as i32;
const OOR: i32 = ZeroDdsStatus::OutOfResources as i32;

/// Polls `f` until it returns true or `timeout` elapses. Discovery between two
/// processes-in-one-participant-set is asynchronous; the boundary itself is
/// not, so the wait is only for the list to be non-empty.
fn wait_until(mut f: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if f() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// # Safety
/// The factory is a process singleton.
unsafe fn new_participant(domain: u32) -> *mut ZeroDdsDomainParticipant {
    let factory = zerodds_dpf_get_instance();
    // SAFETY: the factory is the singleton; the participant is deleted once
    // by `drop_participant`.
    unsafe { zerodds_dpf_create_participant(factory, domain, ptr::null()) }
}

/// # Safety
/// `p` comes from `new_participant` and is not used afterwards.
unsafe fn drop_participant(p: *mut ZeroDdsDomainParticipant) {
    let factory = zerodds_dpf_get_instance();
    // SAFETY: p is a live participant; every contained entity is dropped first.
    unsafe {
        zerodds_dp_delete_contained_entities(p);
        let _ = zerodds_dpf_delete_participant(factory, p);
    }
}

/// `zerodds_dp_get_discovered_participants`: empty on a lone participant,
/// exactly full, and one handle more than the buffer holds.
#[test]
fn dp_get_discovered_participants_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    // SAFETY: every handle comes from a create call on `domain` and is
    // deleted exactly once.
    unsafe {
        let local = new_participant(domain);
        let mut count = 0usize;
        // Empty: a lone participant discovers no remote participant.
        assert_eq!(
            zerodds_dp_get_discovered_participants(local, ptr::null_mut(), &mut count, 0),
            OK
        );
        assert_eq!(count, 0);

        let _peer_a = new_participant(domain);
        let _peer_b = new_participant(domain);

        let mut n = 0usize;
        assert!(
            wait_until(
                || {
                    let mut c = 0usize;
                    let rc =
                        zerodds_dp_get_discovered_participants(local, ptr::null_mut(), &mut c, 0);
                    n = c;
                    (rc == OK || rc == OOR) && c >= 2
                },
                Duration::from_secs(8)
            ),
            "the two peers were not discovered within the deadline"
        );

        // Exactly full: `cap == n`.
        let mut exact = vec![0u64; n];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dp_get_discovered_participants(local, exact.as_mut_ptr(), &mut got, n),
            OK
        );
        assert_eq!(got, n);
        assert!(exact.iter().all(|h| *h != 0), "every handle is filled");

        // One handle more than the buffer holds: `cap == n - 1`.
        let mut small = vec![0u64; n - 1];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dp_get_discovered_participants(local, small.as_mut_ptr(), &mut got, n - 1),
            OOR
        );
        assert_eq!(got, n, "the total is reported when the buffer is too small");
        assert_eq!(small.as_slice(), &exact[..n - 1]);

        drop_participant(local);
        drop_participant(_peer_a);
        drop_participant(_peer_b);
    }
}

/// `zerodds_dp_get_discovered_topics`: empty on a lone participant, exactly
/// full, and one handle more than the buffer holds.
#[test]
fn dp_get_discovered_topics_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    // SAFETY: every handle comes from a create call on `domain` and is
    // deleted exactly once.
    unsafe {
        let local = new_participant(domain);
        let mut count = 0usize;
        // Empty: no peer announces a topic yet.
        assert_eq!(
            zerodds_dp_get_discovered_topics(local, ptr::null_mut(), &mut count, 0),
            OK
        );
        assert_eq!(count, 0);

        // Two peers, each with a writer on its own topic; the topic is seen
        // through the peer's SEDP publication.
        let peer_a = new_participant(domain);
        let peer_b = new_participant(domain);
        for (peer, topic) in [(peer_a, c"CFfiBoundsTopicA"), (peer_b, c"CFfiBoundsTopicB")] {
            let t =
                zerodds_dp_create_topic(peer, topic.as_ptr(), c"RawBytes".as_ptr(), ptr::null());
            let pubh = zerodds_dp_create_publisher(peer, ptr::null());
            assert!(!zerodds_pub_create_datawriter(pubh, t, ptr::null()).is_null());
        }

        let mut n = 0usize;
        assert!(
            wait_until(
                || {
                    let mut c = 0usize;
                    let rc = zerodds_dp_get_discovered_topics(local, ptr::null_mut(), &mut c, 0);
                    n = c;
                    (rc == OK || rc == OOR) && c >= 2
                },
                Duration::from_secs(8)
            ),
            "the two peer topics were not discovered within the deadline"
        );

        // Exactly full.
        let mut exact = vec![0u64; n];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dp_get_discovered_topics(local, exact.as_mut_ptr(), &mut got, n),
            OK
        );
        assert_eq!(got, n);
        assert!(exact.iter().all(|h| *h != 0));

        // One handle more than the buffer holds.
        let mut small = vec![0u64; n - 1];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dp_get_discovered_topics(local, small.as_mut_ptr(), &mut got, n - 1),
            OOR
        );
        assert_eq!(got, n, "the total is reported when the buffer is too small");
        assert_eq!(small.as_slice(), &exact[..n - 1]);

        drop_participant(local);
        drop_participant(peer_a);
        drop_participant(peer_b);
    }
}

/// `zerodds_dw_get_matched_subscriptions`: empty with no matched reader,
/// exactly full, and one handle more than the buffer holds.
#[test]
fn dw_get_matched_subscriptions_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    // SAFETY: every handle comes from a create call on `domain` and is
    // deleted exactly once.
    unsafe {
        let p = new_participant(domain);

        // Empty: a writer whose topic has no reader.
        let t0 = zerodds_dp_create_topic(
            p,
            c"CFfiBoundsSubEmpty".as_ptr(),
            c"RawBytes".as_ptr(),
            ptr::null(),
        );
        let pub0 = zerodds_dp_create_publisher(p, ptr::null());
        let dw0 = zerodds_pub_create_datawriter(pub0, t0, ptr::null());
        let mut count = 0usize;
        assert_eq!(
            zerodds_dw_get_matched_subscriptions(dw0, ptr::null_mut(), &mut count, 0),
            OK
        );
        assert_eq!(count, 0);

        // Matched: one writer, two readers of this participant (matching
        // within one participant is synchronous).
        let t = zerodds_dp_create_topic(
            p,
            c"CFfiBoundsSub".as_ptr(),
            c"RawBytes".as_ptr(),
            ptr::null(),
        );
        let pubh = zerodds_dp_create_publisher(p, ptr::null());
        let sub = zerodds_dp_create_subscriber(p, ptr::null());
        let dw = zerodds_pub_create_datawriter(pubh, t, ptr::null());
        let _r1 = zerodds_sub_create_datareader(sub, t, ptr::null());
        let _r2 = zerodds_sub_create_datareader(sub, t, ptr::null());

        let mut n = 0usize;
        assert_eq!(
            zerodds_dw_get_matched_subscriptions(dw, ptr::null_mut(), &mut n, 0),
            OOR
        );
        assert_eq!(n, 2, "both readers of the participant matched");

        let mut exact = vec![0u64; n];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dw_get_matched_subscriptions(dw, exact.as_mut_ptr(), &mut got, n),
            OK
        );
        assert_eq!(got, n);

        let mut small = vec![0u64; n - 1];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dw_get_matched_subscriptions(dw, small.as_mut_ptr(), &mut got, n - 1),
            OOR
        );
        assert_eq!(got, n, "the total is reported when the buffer is too small");
        assert_eq!(small.as_slice(), &exact[..n - 1]);

        drop_participant(p);
    }
}

/// `zerodds_dr_get_matched_publications`: empty with no matched writer,
/// exactly full, and one handle more than the buffer holds.
#[test]
fn dr_get_matched_publications_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    // SAFETY: every handle comes from a create call on `domain` and is
    // deleted exactly once.
    unsafe {
        let p = new_participant(domain);

        // Empty: a reader whose topic has no writer.
        let t0 = zerodds_dp_create_topic(
            p,
            c"CFfiBoundsPubEmpty".as_ptr(),
            c"RawBytes".as_ptr(),
            ptr::null(),
        );
        let sub0 = zerodds_dp_create_subscriber(p, ptr::null());
        let dr0 = zerodds_sub_create_datareader(sub0, t0, ptr::null());
        let mut count = 0usize;
        assert_eq!(
            zerodds_dr_get_matched_publications(dr0, ptr::null_mut(), &mut count, 0),
            OK
        );
        assert_eq!(count, 0);

        // Matched: two writers, one reader of this participant.
        let t = zerodds_dp_create_topic(
            p,
            c"CFfiBoundsPub".as_ptr(),
            c"RawBytes".as_ptr(),
            ptr::null(),
        );
        let pubh = zerodds_dp_create_publisher(p, ptr::null());
        let sub = zerodds_dp_create_subscriber(p, ptr::null());
        let dr = zerodds_sub_create_datareader(sub, t, ptr::null());
        let _w1 = zerodds_pub_create_datawriter(pubh, t, ptr::null());
        let _w2 = zerodds_pub_create_datawriter(pubh, t, ptr::null());

        let mut n = 0usize;
        assert_eq!(
            zerodds_dr_get_matched_publications(dr, ptr::null_mut(), &mut n, 0),
            OOR
        );
        assert_eq!(n, 2, "both writers of the participant matched");

        let mut exact = vec![0u64; n];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dr_get_matched_publications(dr, exact.as_mut_ptr(), &mut got, n),
            OK
        );
        assert_eq!(got, n);

        let mut small = vec![0u64; n - 1];
        let mut got = 0usize;
        assert_eq!(
            zerodds_dr_get_matched_publications(dr, small.as_mut_ptr(), &mut got, n - 1),
            OOR
        );
        assert_eq!(got, n, "the total is reported when the buffer is too small");
        assert_eq!(small.as_slice(), &exact[..n - 1]);

        drop_participant(p);
    }
}

/// `zerodds_sub_get_datareaders`: empty subscriber, exactly full, and one
/// handle more than the buffer holds.
#[test]
fn sub_get_datareaders_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    // SAFETY: every handle comes from a create call on `domain` and is
    // deleted exactly once.
    unsafe {
        let p = new_participant(domain);
        let mut count = 0usize;
        // Empty: a subscriber with no reader.
        let sub_empty = zerodds_dp_create_subscriber(p, ptr::null());
        assert_eq!(
            zerodds_sub_get_datareaders(sub_empty, ptr::null_mut(), &mut count, 0, 0, 0, 0),
            OK
        );
        assert_eq!(count, 0);

        let t = zerodds_dp_create_topic(
            p,
            c"CFfiBoundsReaders".as_ptr(),
            c"RawBytes".as_ptr(),
            ptr::null(),
        );
        let sub = zerodds_dp_create_subscriber(p, ptr::null());
        let r1 = zerodds_sub_create_datareader(sub, t, ptr::null());
        let r2 = zerodds_sub_create_datareader(sub, t, ptr::null());
        // DDS 1.4 §2.2.2.5.2.10: a reader is listed when it holds samples.
        for r in [r1, r2] {
            assert!((*r).rt.test_inject_user_alive((*r).eid, vec![1u8]));
        }

        let mut n = 0usize;
        assert_eq!(
            zerodds_sub_get_datareaders(sub, ptr::null_mut(), &mut n, 0, 0, 0, 0),
            OOR
        );
        assert_eq!(n, 2);

        let mut exact = vec![ptr::null_mut(); n];
        let mut got = 0usize;
        assert_eq!(
            zerodds_sub_get_datareaders(sub, exact.as_mut_ptr(), &mut got, n, 0, 0, 0),
            OK
        );
        assert_eq!(got, n);
        assert!(exact.iter().all(|r| !r.is_null()));

        let mut small = vec![ptr::null_mut(); n - 1];
        let mut got = 0usize;
        assert_eq!(
            zerodds_sub_get_datareaders(sub, small.as_mut_ptr(), &mut got, n - 1, 0, 0, 0),
            OOR
        );
        assert_eq!(got, n, "the total is reported when the buffer is too small");
        assert_eq!(small[0], exact[0]);

        drop_participant(p);
    }
}

/// `zerodds_reader_take_into`: empty reader, exactly full, and one byte more
/// than the buffer holds. The refused sample is consumed and its payload
/// length is not silently truncated.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn reader_take_into_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic = CString::new("CFfiBoundsTakeInto").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    // SAFETY: runtimes, writer and reader come from the create calls right
    // above their use and are destroyed exactly once.
    unsafe {
        let rt_pub = zerodds_runtime_create(domain);
        let rt_sub = zerodds_runtime_create(domain);
        assert!(!rt_pub.is_null() && !rt_sub.is_null());
        assert_eq!(zerodds_runtime_wait_for_peers(rt_pub, 1, 5_000), OK);
        assert_eq!(zerodds_runtime_wait_for_peers(rt_sub, 1, 5_000), OK);
        let writer = zerodds_writer_create(rt_pub, topic.as_ptr(), typ.as_ptr(), 1);
        let reader = zerodds_reader_create(rt_sub, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null() && !reader.is_null());
        assert_eq!(zerodds_writer_wait_for_matched(writer, 1, 5_000), OK);

        let payload = [0xDEu8, 0xAD, 0xBE, 0xEF];
        let mut buf = [0u8; 8];

        // Empty: no sample pending.
        assert_eq!(
            zerodds_reader_take_into(reader, buf.as_mut_ptr(), buf.len()),
            0
        );

        // One byte more than the buffer holds: cap = 3 < 4 -> -8, the sample
        // is consumed and nothing is copied.
        assert_eq!(
            zerodds_writer_write(writer, payload.as_ptr(), payload.len()),
            OK
        );
        let mut one_more = 0isize;
        assert!(
            wait_until(
                || {
                    one_more = zerodds_reader_take_into(reader, buf.as_mut_ptr(), 3);
                    one_more == OOR as isize
                },
                Duration::from_secs(5)
            ),
            "no sample arrived for the too-small buffer"
        );
        assert_eq!(one_more, OOR as isize);
        assert_eq!(buf, [0u8; 8], "a refused sample must not truncate into buf");

        // Exactly full: cap == payload length.
        assert_eq!(
            zerodds_writer_write(writer, payload.as_ptr(), payload.len()),
            OK
        );
        let mut n = 0isize;
        assert!(
            wait_until(
                || {
                    n = zerodds_reader_take_into(reader, buf.as_mut_ptr(), payload.len());
                    n as usize == payload.len()
                },
                Duration::from_secs(5)
            ),
            "an exactly-fitting sample did not arrive"
        );
        assert_eq!(&buf[..payload.len()], &payload[..]);

        zerodds_writer_destroy(writer);
        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt_pub);
        zerodds_runtime_destroy(rt_sub);
    }
}

/// `zerodds_writer_unicast_locators`: the length query copies nothing, an
/// exactly-sized buffer is filled, and the count is returned when the buffer
/// holds one entry fewer.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn writer_unicast_locators_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic = CString::new("CFfiBoundsWriterLoc").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    // SAFETY: the runtime and writer come from the create calls right above
    // their use and are destroyed exactly once.
    unsafe {
        let rt = zerodds_runtime_create(domain);
        assert!(!rt.is_null());
        let writer = zerodds_writer_create(rt, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!writer.is_null());

        // The empty buffer: out == NULL, capacity == 0 copies nothing and
        // returns the total.
        let total = zerodds_writer_unicast_locators(writer, ptr::null_mut(), 0);
        assert!(total >= 1, "a live runtime announces at least one locator");

        // Exactly full.
        let mut exact = vec![ZeroDdsLocator::default(); total];
        assert_eq!(
            zerodds_writer_unicast_locators(writer, exact.as_mut_ptr(), total),
            total
        );

        // One entry more than the buffer holds: the total is still returned.
        let mut small = vec![ZeroDdsLocator::default(); total - 1];
        assert_eq!(
            zerodds_writer_unicast_locators(writer, small.as_mut_ptr(), total - 1),
            total,
            "the total count is returned even when it exceeds the buffer"
        );
        // A host with one interface announces one locator; the short buffer
        // then holds none.
        if let (Some(first), Some(expected)) = (small.first(), exact.first()) {
            assert_eq!(first.port, expected.port);
            assert_eq!(first.address, expected.address);
        }

        // A NULL writer is the empty endpoint.
        assert_eq!(
            zerodds_writer_unicast_locators(ptr::null_mut(), ptr::null_mut(), 0),
            0
        );

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}

/// `zerodds_reader_unicast_locators`: as the writer's call.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn reader_unicast_locators_bounds() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic = CString::new("CFfiBoundsReaderLoc").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    // SAFETY: the runtime and reader come from the create calls right above
    // their use and are destroyed exactly once.
    unsafe {
        let rt = zerodds_runtime_create(domain);
        assert!(!rt.is_null());
        let reader = zerodds_reader_create(rt, topic.as_ptr(), typ.as_ptr(), 1);
        assert!(!reader.is_null());

        let total = zerodds_reader_unicast_locators(reader, ptr::null_mut(), 0);
        assert!(total >= 1, "a live runtime announces at least one locator");

        let mut exact = vec![ZeroDdsLocator::default(); total];
        assert_eq!(
            zerodds_reader_unicast_locators(reader, exact.as_mut_ptr(), total),
            total
        );

        let mut small = vec![ZeroDdsLocator::default(); total - 1];
        assert_eq!(
            zerodds_reader_unicast_locators(reader, small.as_mut_ptr(), total - 1),
            total,
            "the total count is returned even when it exceeds the buffer"
        );
        if let (Some(first), Some(expected)) = (small.first(), exact.first()) {
            assert_eq!(first.port, expected.port);
            assert_eq!(first.address, expected.address);
        }

        assert_eq!(
            zerodds_reader_unicast_locators(ptr::null_mut(), ptr::null_mut(), 0),
            0
        );

        zerodds_reader_destroy(reader);
        zerodds_runtime_destroy(rt);
    }
}
