// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.6.2 through the C API: the endpoints of a client and of a
//! service announce the discovery extension, read each other's, and the
//! service-matching (Figure 7.4) and client-matching (Figure 7.3)
//! algorithms decide on it.

#![cfg(feature = "rpc")]
#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::time::{Duration, Instant};

use zerodds::rpc_discovery_ffi::{
    ZeroDdsRpcPeer, zerodds_reader_matched_rpc_writers, zerodds_reader_set_rpc_discovery,
    zerodds_rpc_client_matching_complete, zerodds_rpc_service_matching_complete,
    zerodds_writer_matched_rpc_readers, zerodds_writer_set_rpc_discovery,
};
use zerodds::{
    ZeroDdsEndpointQos, ZeroDdsReader, ZeroDdsWriter, zerodds_endpoint_qos_default,
    zerodds_reader_create_qos, zerodds_reader_destroy, zerodds_reader_guid, zerodds_runtime_create,
    zerodds_runtime_destroy, zerodds_writer_create_qos, zerodds_writer_destroy,
    zerodds_writer_guid,
};

fn reliable() -> ZeroDdsEndpointQos {
    ZeroDdsEndpointQos {
        reliable: 1,
        history_keep_all: 1,
        ..zerodds_endpoint_qos_default()
    }
}

fn eventually(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

/// # Safety
/// `w` comes from `zerodds_writer_create*` and is not destroyed.
unsafe fn writer_guid(w: *mut ZeroDdsWriter) -> [u8; 16] {
    // SAFETY: `w` is valid per the contract above; `g` has the 16 octets the
    // call writes.
    let mut g = [0u8; 16];
    assert_eq!(unsafe { zerodds_writer_guid(w, g.as_mut_ptr()) }, 0);
    g
}

/// # Safety
/// `r` comes from `zerodds_reader_create*` and is not destroyed.
unsafe fn reader_guid(r: *mut ZeroDdsReader) -> [u8; 16] {
    // SAFETY: `r` is valid per the contract above; `g` has the 16 octets the
    // call writes.
    let mut g = [0u8; 16];
    assert_eq!(unsafe { zerodds_reader_guid(r, g.as_mut_ptr()) }, 0);
    g
}

fn text(buf: &[core::ffi::c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn the_extension_is_announced_read_and_decides_the_matching() {
    let lease = zerodds_test_domains::lease_domain();
    let request = CString::new("RpcDiscReq").unwrap();
    let reply = CString::new("RpcDiscRep").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let name = CString::new("calc-A").unwrap();
    let alias = CString::new("Base_Service_Request").unwrap();
    // SAFETY: every pointer comes from the call above its use.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        let q = reliable();
        let client_writer = zerodds_writer_create_qos(rt, request.as_ptr(), typ.as_ptr(), 0, &q);
        let client_reader = zerodds_reader_create_qos(rt, reply.as_ptr(), typ.as_ptr(), 0, &q);
        let service_reader = zerodds_reader_create_qos(rt, request.as_ptr(), typ.as_ptr(), 0, &q);
        let service_writer = zerodds_writer_create_qos(rt, reply.as_ptr(), typ.as_ptr(), 0, &q);
        assert!(
            !client_writer.is_null()
                && !client_reader.is_null()
                && !service_reader.is_null()
                && !service_writer.is_null()
        );
        let (cw, cr) = (writer_guid(client_writer), reader_guid(client_reader));
        let (sr, sw) = (reader_guid(service_reader), writer_guid(service_writer));

        // Before the announcements the service cannot be paired.
        let mut complete = 9u8;
        assert_eq!(
            zerodds_rpc_service_matching_complete(
                client_writer,
                client_reader,
                ptr::null(),
                0,
                &mut complete
            ),
            0
        );
        assert_eq!(complete, 0);

        // §7.6.2.1: the client announces the related endpoint and no name;
        // the service announces the name and the related endpoint.
        assert_eq!(
            zerodds_writer_set_rpc_discovery(
                client_writer,
                ptr::null(),
                cr.as_ptr(),
                ptr::null(),
                0
            ),
            0
        );
        assert_eq!(
            zerodds_reader_set_rpc_discovery(
                client_reader,
                ptr::null(),
                cw.as_ptr(),
                ptr::null(),
                0
            ),
            0
        );
        assert_eq!(
            zerodds_writer_set_rpc_discovery(
                service_writer,
                name.as_ptr(),
                sr.as_ptr(),
                ptr::null(),
                0
            ),
            0
        );
        let aliases = [alias.as_ptr()];
        assert_eq!(
            zerodds_reader_set_rpc_discovery(
                service_reader,
                name.as_ptr(),
                sw.as_ptr(),
                aliases.as_ptr(),
                1
            ),
            0
        );

        // The client reads the service reader's extension.
        let mut peers: [ZeroDdsRpcPeer; 4] = std::mem::zeroed();
        let mut count = 0usize;
        assert!(eventually(|| {
            assert_eq!(
                zerodds_writer_matched_rpc_readers(
                    client_writer,
                    peers.as_mut_ptr(),
                    4,
                    &mut count
                ),
                0
            );
            (0..count.min(4)).any(|i| peers[i].key == sr && peers[i].has_instance_name == 1)
        }));
        let peer = peers.iter().take(count).find(|p| p.key == sr).unwrap();
        assert_eq!(text(&peer.instance_name), "calc-A");
        assert_eq!(peer.has_related, 1);
        assert_eq!(peer.related, sw);
        assert_eq!(peer.alias_count, 1);
        assert_eq!(text(&peer.aliases), "Base_Service_Request");

        // The service reads the client writer's: its reply reader, no name.
        assert!(eventually(|| {
            assert_eq!(
                zerodds_reader_matched_rpc_writers(
                    service_reader,
                    peers.as_mut_ptr(),
                    4,
                    &mut count
                ),
                0
            );
            (0..count.min(4)).any(|i| peers[i].key == cw && peers[i].has_related == 1)
        }));
        let client = peers.iter().take(count).find(|p| p.key == cw).unwrap();
        assert_eq!(client.related, cr);
        assert_eq!(client.has_instance_name, 0);

        // Figure 7.4: complete for the instance the service announced.
        let service_complete = |instance: Option<&CString>, accept: u8| {
            let mut out = 9u8;
            assert_eq!(
                zerodds_rpc_service_matching_complete(
                    client_writer,
                    client_reader,
                    instance.map_or(ptr::null(), |n| n.as_ptr()),
                    accept,
                    &mut out
                ),
                0
            );
            out == 1
        };
        assert!(eventually(|| service_complete(Some(&name), 0)));
        let other = CString::new("calc-B").unwrap();
        assert!(!service_complete(Some(&other), 0));
        assert!(
            !service_complete(None, 0),
            "the unnamed instance is not served"
        );

        // Figure 7.3: the per-client mask. The service has discovered both
        // endpoints of this client, and none of another.
        let client_complete = |guid: &[u8; 16], accept: u8| {
            let mut out = 9u8;
            assert_eq!(
                zerodds_rpc_client_matching_complete(
                    service_reader,
                    service_writer,
                    guid.as_ptr(),
                    accept,
                    &mut out
                ),
                0
            );
            out == 1
        };
        assert!(eventually(|| client_complete(&cw, 0)));
        assert!(!client_complete(&[0x77; 16], 0));
        assert!(client_complete(&[0x77; 16], 1), "the named alternative");

        // string<256> is the bound (§7.6.2.1).
        let long = CString::new("x".repeat(257)).unwrap();
        assert_eq!(
            zerodds_writer_set_rpc_discovery(
                service_writer,
                long.as_ptr(),
                sr.as_ptr(),
                ptr::null(),
                0
            ),
            zerodds::ZeroDdsStatus::BadParameter as i32
        );

        zerodds_reader_destroy(service_reader);
        zerodds_reader_destroy(client_reader);
        zerodds_writer_destroy(service_writer);
        zerodds_writer_destroy(client_writer);
        zerodds_runtime_destroy(rt);
    }
}

/// One matched pair on `topic`, announced, for the boundary tests.
///
/// # Safety
/// The returned handles are destroyed by the caller, endpoints first; the
/// domain lease outlives them.
unsafe fn matched_pair(
    domain: u32,
    topic: &str,
) -> (
    *mut zerodds::ZeroDdsRuntime,
    *mut ZeroDdsWriter,
    *mut ZeroDdsReader,
) {
    let topic = CString::new(topic).unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let q = reliable();
    // SAFETY: handles come from the calls above their use.
    unsafe {
        let rt = zerodds_runtime_create(domain);
        let w = zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &q);
        let r = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &q);
        assert!(!w.is_null() && !r.is_null());
        (rt, w, r)
    }
}

const OK: i32 = 0;
const OOR: i32 = zerodds::ZeroDdsStatus::OutOfResources as i32;

#[test]
fn writer_matched_rpc_readers_bounds() {
    // SAFETY: handles from `matched_pair`; buffers sized by the caps used.
    unsafe {
        let lease = zerodds_test_domains::lease_domain();
        let (rt, w, r) = matched_pair(lease.id() as u32, "RpcBoundsW");
        let mut peers: [ZeroDdsRpcPeer; 2] = std::mem::zeroed();
        let mut count = 99usize;
        // Empty: nothing has matched yet or, once it has, the total is 1;
        // wait for the match, then test the three boundaries on a total of 1.
        assert!(eventually(|| {
            zerodds_writer_matched_rpc_readers(w, peers.as_mut_ptr(), 2, &mut count) == OK
                && count == 1
        }));
        // Exactly full.
        assert_eq!(
            zerodds_writer_matched_rpc_readers(w, peers.as_mut_ptr(), 1, &mut count),
            OK
        );
        assert_eq!(count, 1);
        // One too small: the total is reported, nothing is written past cap.
        assert_eq!(
            zerodds_writer_matched_rpc_readers(w, ptr::null_mut(), 0, &mut count),
            OOR
        );
        assert_eq!(count, 1);
        // Empty result: a writer on a topic with no reader reports 0 and OK.
        let lone = CString::new("RpcBoundsWLone").unwrap();
        let typ = CString::new("RawBytes").unwrap();
        let q = reliable();
        let lw = zerodds_writer_create_qos(rt, lone.as_ptr(), typ.as_ptr(), 0, &q);
        count = 99;
        assert_eq!(
            zerodds_writer_matched_rpc_readers(lw, ptr::null_mut(), 0, &mut count),
            OK
        );
        assert_eq!(count, 0);
        zerodds_writer_destroy(lw);
        zerodds_reader_destroy(r);
        zerodds_writer_destroy(w);
        zerodds_runtime_destroy(rt);
    }
}

#[test]
fn reader_matched_rpc_writers_bounds() {
    // SAFETY: as above.
    unsafe {
        let lease = zerodds_test_domains::lease_domain();
        let (rt, w, r) = matched_pair(lease.id() as u32, "RpcBoundsR");
        let mut peers: [ZeroDdsRpcPeer; 2] = std::mem::zeroed();
        let mut count = 99usize;
        assert!(eventually(|| {
            zerodds_reader_matched_rpc_writers(r, peers.as_mut_ptr(), 2, &mut count) == OK
                && count == 1
        }));
        assert_eq!(
            zerodds_reader_matched_rpc_writers(r, peers.as_mut_ptr(), 1, &mut count),
            OK
        );
        assert_eq!(count, 1);
        assert_eq!(
            zerodds_reader_matched_rpc_writers(r, ptr::null_mut(), 0, &mut count),
            OOR
        );
        assert_eq!(count, 1);
        let lone = CString::new("RpcBoundsRLone").unwrap();
        let typ = CString::new("RawBytes").unwrap();
        let q = reliable();
        let lr = zerodds_reader_create_qos(rt, lone.as_ptr(), typ.as_ptr(), 0, &q);
        count = 99;
        assert_eq!(
            zerodds_reader_matched_rpc_writers(lr, ptr::null_mut(), 0, &mut count),
            OK
        );
        assert_eq!(count, 0);
        zerodds_reader_destroy(lr);
        zerodds_reader_destroy(r);
        zerodds_writer_destroy(w);
        zerodds_runtime_destroy(rt);
    }
}
