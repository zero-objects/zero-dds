//! E2E wire-roundtrip tests for DDS-RPC.
//!
//! Verifies the full request-reply cycle between in-process
//! requester and replier through pure wire bytes (without RTPS/network):
//!
//! 1. The requester encodes RequestHeader + payload.
//! 2. The "wire" delivers the bytes.
//! 3. The replier decodes the frame, extracts RequestHeader + payload.
//! 4. The replier builds ReplyHeader (with related_request_id = request.SampleIdentity).
//! 5. The replier encodes ReplyHeader + reply payload.
//! 6. The requester decodes the reply, correlates via SampleIdentity.
//!
//! Spec-Anker: DDS-RPC 1.0 §7.5 (Wire), §7.6 (Correlation).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_rpc::common_types::{RemoteExceptionCode, ReplyHeader, RequestHeader, SampleIdentity};
use zerodds_rpc::wire_codec::{
    decode_reply_frame, decode_request_frame, encode_reply_frame, encode_request_frame,
};

/// Helper struct: simulates an in-process wire queue.
#[derive(Default)]
struct InProcWire {
    request_queue: Vec<Vec<u8>>,
    reply_queue: Vec<Vec<u8>>,
}

impl InProcWire {
    fn put_request(&mut self, frame: Vec<u8>) {
        self.request_queue.push(frame);
    }
    fn take_request(&mut self) -> Option<Vec<u8>> {
        if self.request_queue.is_empty() {
            None
        } else {
            Some(self.request_queue.remove(0))
        }
    }
    fn put_reply(&mut self, frame: Vec<u8>) {
        self.reply_queue.push(frame);
    }
    fn take_reply(&mut self) -> Option<Vec<u8>> {
        if self.reply_queue.is_empty() {
            None
        } else {
            Some(self.reply_queue.remove(0))
        }
    }
}

#[test]
fn single_request_reply_roundtrip() {
    let mut wire = InProcWire::default();

    // Step 1: the requester builds a request with a correlation-unique
    // SampleIdentity (writer GUID + sequence number).
    let req_id = SampleIdentity::new([0xAA; 16], 1);
    let req_hdr = RequestHeader::new(req_id, "Calculator::add");
    let req_payload = vec![0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00]; // a=1, b=2
    let req_frame = encode_request_frame(&req_hdr, &req_payload);
    wire.put_request(req_frame);

    // Step 2-3: the replier fetches the frame, decodes.
    let frame = wire.take_request().unwrap();
    let (decoded_hdr, decoded_payload) = decode_request_frame(&frame).unwrap();
    assert_eq!(decoded_hdr, req_hdr);
    assert_eq!(decoded_payload, &req_payload[..]);

    // Step 4-5: Replier baut Reply (related_request_id = decoded_hdr.request_id).
    let reply_id = SampleIdentity::new([0xBB; 16], 1);
    let _ = reply_id;
    let reply_hdr = ReplyHeader {
        related_request_id: decoded_hdr.request_id,
        remote_ex: RemoteExceptionCode::Ok,
    };
    let reply_payload = vec![0x03, 0x00, 0x00, 0x00]; // result=3
    let reply_frame = encode_reply_frame(&reply_hdr, &reply_payload);
    wire.put_reply(reply_frame);

    // Step 6: Requester holt Reply, verifiziert Korrelation.
    let frame = wire.take_reply().unwrap();
    let (back_hdr, back_payload) = decode_reply_frame(&frame).unwrap();
    assert_eq!(
        back_hdr.related_request_id, req_id,
        "related_request_id must match original SampleIdentity"
    );
    assert_eq!(back_hdr.remote_ex, RemoteExceptionCode::Ok);
    assert_eq!(back_payload, &reply_payload[..]);
}

#[test]
fn many_concurrent_request_reply_pairs() {
    // 100 requests with unique SampleIdentities, then 100 replies in
    // reverse order — correlation must still work out.
    let mut wire = InProcWire::default();
    let mut sent_ids = Vec::new();

    for i in 0u64..100 {
        let id = SampleIdentity::new([i as u8; 16], i);
        let hdr = RequestHeader::new(id, "Calculator::compute");
        let frame = encode_request_frame(&hdr, &[i as u8]);
        wire.put_request(frame);
        sent_ids.push(id);
    }

    // The replier processes in reverse order.
    let mut frames: Vec<Vec<u8>> = Vec::new();
    while let Some(frame) = wire.take_request() {
        frames.push(frame);
    }
    frames.reverse();
    for (idx, frame) in frames.into_iter().enumerate() {
        let _ = idx;
        let (req_hdr, _) = decode_request_frame(&frame).unwrap();
        let reply_hdr = ReplyHeader {
            related_request_id: req_hdr.request_id,
            remote_ex: RemoteExceptionCode::Ok,
        };
        wire.put_reply(encode_reply_frame(&reply_hdr, &[]));
    }

    // The requester collects all replies and matches each against sent_ids.
    let mut matched = 0;
    while let Some(frame) = wire.take_reply() {
        let (reply_hdr, _) = decode_reply_frame(&frame).unwrap();
        assert!(
            sent_ids.contains(&reply_hdr.related_request_id),
            "reply correlation-id {:?} not in sent set",
            reply_hdr.related_request_id
        );
        matched += 1;
    }
    assert_eq!(matched, 100, "all 100 replies must correlate");
}

#[test]
fn reply_with_remote_exception_propagates_correctly() {
    let req_id = SampleIdentity::new([0x42; 16], 7);
    let req_hdr = RequestHeader::new(req_id, "Calculator::divide");
    let req_frame = encode_request_frame(&req_hdr, &[10, 0, 0, 0, 0, 0, 0, 0]);

    let (decoded_req, _) = decode_request_frame(&req_frame).unwrap();
    let reply_hdr = ReplyHeader {
        related_request_id: decoded_req.request_id,
        remote_ex: RemoteExceptionCode::InvalidArgument,
    };
    let reply_frame = encode_reply_frame(&reply_hdr, &[]);

    let (back_hdr, _) = decode_reply_frame(&reply_frame).unwrap();
    assert_eq!(back_hdr.remote_ex, RemoteExceptionCode::InvalidArgument);
    assert_eq!(back_hdr.related_request_id, req_id);
}

#[test]
fn request_with_long_operation_name_roundtrips() {
    let req_id = SampleIdentity::new([0x99; 16], 42);
    let long_op = "VeryLongServiceName::very_long_operation_with_descriptive_name";
    let req_hdr = RequestHeader::new(req_id, long_op);
    let frame = encode_request_frame(&req_hdr, &[]);

    let (back_hdr, _) = decode_request_frame(&frame).unwrap();
    assert_eq!(back_hdr.instance_name, long_op);
    assert_eq!(back_hdr.request_id, req_id);
}

#[test]
fn large_payload_roundtrip() {
    let req_id = SampleIdentity::new([0xCC; 16], 100);
    let req_hdr = RequestHeader::new(req_id, "Stream::write");
    let large_payload: Vec<u8> = (0..65_000u32).map(|i| (i & 0xFF) as u8).collect();
    let frame = encode_request_frame(&req_hdr, &large_payload);

    let (back_hdr, back_payload) = decode_request_frame(&frame).unwrap();
    assert_eq!(back_hdr, req_hdr);
    assert_eq!(back_payload, large_payload.as_slice());
}
