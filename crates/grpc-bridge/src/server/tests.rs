// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Tests of the server side of a gRPC connection (spec §3, §4.1, §4.3).

use super::*;

/// One frame: type, flags, stream id, payload.
type Frame = (u8, u8, u32, Vec<u8>);

/// The frames of a byte stream.
fn frames(mut bytes: &[u8]) -> Vec<Frame> {
    let mut out = Vec::new();
    while bytes.len() >= 9 {
        let len =
            (usize::from(bytes[0]) << 16) | (usize::from(bytes[1]) << 8) | usize::from(bytes[2]);
        let sid = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) & 0x7fff_ffff;
        out.push((bytes[3], bytes[4], sid, bytes[9..9 + len].to_vec()));
        bytes = &bytes[9 + len..];
    }
    out
}

fn of_type(f: &[Frame], t: FrameType) -> Vec<Frame> {
    f.iter().filter(|f| f.0 == t as u8).cloned().collect()
}

fn request_fields(path: &str, extra: &[(&str, &str)]) -> Vec<HeaderField> {
    let mut v = alloc::vec![field(":method", "POST"), field(":path", path)];
    v.extend(extra.iter().map(|(n, val)| field(n, val)));
    v
}

fn headers_frame(enc: &mut HpackEncoder, sid: u32, end_stream: bool) -> Vec<u8> {
    let block = enc.encode(&request_fields(
        "/zerodds.t.v1.TStream/Publish",
        &[("content-type", "application/grpc")],
    ));
    let flags = Flags::END_HEADERS | if end_stream { Flags::END_STREAM } else { 0 };
    encode_raw(FrameType::Headers, flags, sid, &block)
}

fn data_frame(sid: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
    encode_raw(FrameType::Data, flags, sid, payload)
}

fn lpm(msg: &[u8]) -> Vec<u8> {
    let mut v = alloc::vec![0];
    v.extend_from_slice(&(msg.len() as u32).to_be_bytes());
    v.extend_from_slice(msg);
    v
}

/// Feeds a whole byte string, frame by frame.
fn feed(s: &mut GrpcServer, mut bytes: &[u8]) -> Result<(), ConnectionError> {
    while !bytes.is_empty() {
        let n = s.process_frame(bytes)?;
        assert!(n > 0, "a whole frame is consumed");
        bytes = &bytes[n..];
    }
    Ok(())
}

fn messages(events: &[CallEvent]) -> Vec<Vec<u8>> {
    events
        .iter()
        .filter_map(|e| match e {
            CallEvent::Message { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn server_starts_with_no_streams() {
    let s = GrpcServer::new();
    assert!(s.streams.is_empty());
    assert_eq!(s.active_streams(), 0);
}

/// A unary answer: response HEADERS, the message, the trailers with
/// END_STREAM, `:status 200` in the first block.
#[test]
fn grpcbridge_12_1_encode_response_includes_status_and_trailers() {
    let mut s = GrpcServer::new();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, true),
    )
    .unwrap();
    s.respond(&GrpcResponse {
        stream_id: 1,
        status: Status::Ok,
        message: None,
        body: alloc::vec![1, 2, 3],
    });
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 3, "{f:?}");
    assert_eq!(f[1].3, [0, 0, 0, 0, 3, 1, 2, 3]);
    assert_eq!(f[2].1, Flags::END_HEADERS | Flags::END_STREAM);
    let mut dec = HpackDecoder::default();
    let first = dec.decode(&f[0].3).unwrap();
    assert!(
        first
            .iter()
            .any(|h| h.name == ":status" && h.value == "200")
    );
    let trailers = dec.decode(&f[2].3).unwrap();
    assert!(
        trailers
            .iter()
            .any(|h| h.name == "grpc-status" && h.value == "0")
    );
    assert_eq!(s.active_streams(), 0);
}

#[test]
fn rst_stream_clears_state() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    assert_eq!(s.active_streams(), 1);
    feed(
        &mut s,
        &encode_one(FrameType::RstStream, 0, 1, &[0, 0, 0, 8]),
    )
    .unwrap();
    assert!(s.streams.is_empty());
    assert!(matches!(
        s.take_events().last(),
        Some(CallEvent::Reset {
            stream_id: 1,
            code: 8
        })
    ));
}

/// Spec §3, §4.1: the SETTINGS of the server carry the three values of
/// the limits, and a WINDOW_UPDATE opens the connection window.
#[test]
fn grpcbridge_3_the_handshake_announces_the_limits() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        max_concurrent_streams: 7,
        initial_window_size: 200_000,
        max_frame_size: 32_768,
        ..ServerLimits::default()
    });
    let f = frames(&s.handshake());
    assert_eq!(f[0].0, FrameType::Settings as u8);
    let settings = decode_settings(&f[0].3).unwrap();
    let value = |id| settings.iter().find(|s| s.id == id).map(|s| s.value);
    assert_eq!(value(SettingId::MaxConcurrentStreams), Some(7));
    assert_eq!(value(SettingId::InitialWindowSize), Some(200_000));
    assert_eq!(value(SettingId::MaxFrameSize), Some(32_768));
    assert_eq!(
        value(SettingId::MaxHeaderListSize),
        Some(MAX_HEADER_LIST_SIZE)
    );
    assert_eq!(f[1].0, FrameType::WindowUpdate as u8);
    assert_eq!(f[1].2, 0);
    assert_eq!(
        u32::from_be_bytes(f[1].3[..4].try_into().unwrap()),
        200_000 - 65_535
    );
}

/// Spec §3 `max_concurrent_streams`: the stream above it is refused
/// with RST_STREAM(REFUSED_STREAM); one below it after a stream ended
/// is taken.
#[test]
fn grpcbridge_3_a_stream_above_max_concurrent_streams_is_refused() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        max_concurrent_streams: 2,
        ..ServerLimits::default()
    });
    let mut enc = HpackEncoder::default();
    for sid in [1, 3] {
        feed(&mut s, &headers_frame(&mut enc, sid, false)).unwrap();
    }
    assert!(s.take_output().is_empty());
    feed(&mut s, &headers_frame(&mut enc, 5, false)).unwrap();
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0, f[0].2), (FrameType::RstStream as u8, 5));
    assert_eq!(f[0].3, (ErrorCode::RefusedStream as u32).to_be_bytes());
    assert_eq!(s.active_streams(), 2);
    // DATA of the refused stream is dropped, not a connection error.
    feed(&mut s, &data_frame(5, Flags::END_STREAM, &lpm(&[]))).unwrap();
    s.take_events();
    feed(&mut s, &data_frame(1, Flags::END_STREAM, &lpm(&[]))).unwrap();
    s.respond(&GrpcResponse {
        stream_id: 1,
        status: Status::Ok,
        message: None,
        body: Vec::new(),
    });
    let _ = s.take_output();
    assert_eq!(s.active_streams(), 1);
    feed(&mut s, &headers_frame(&mut enc, 7, false)).unwrap();
    assert!(s.take_output().is_empty());
    assert_eq!(s.active_streams(), 2);
}

/// Spec §3 `max_recv_message_size`: a request whose prefix declares
/// more is refused with RESOURCE_EXHAUSTED before its octets arrive,
/// and its stream is reset after the answer.
#[test]
fn grpcbridge_3_a_message_above_max_recv_message_size_is_refused() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        max_recv_message_size: 100,
        ..ServerLimits::default()
    });
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    feed(&mut s, &data_frame(1, 0, &[0, 0, 0, 0, 101, 1, 2])).unwrap();
    let events = s.take_events();
    let Some(CallEvent::Refused(r)) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(r.status, Status::ResourceExhausted);
    assert_eq!(r.path, "/zerodds.t.v1.TStream/Publish");
    s.close_call(1, r.status, Some(&r.message));
    let f = frames(&s.take_output());
    assert_eq!(f.last().unwrap().0, FrameType::RstStream as u8);
    assert_eq!(f.last().unwrap().3, [0, 0, 0, 0]);
    // The rest of the message is dropped.
    feed(&mut s, &data_frame(1, Flags::END_STREAM, &[0; 99])).unwrap();
    assert!(s.take_events().is_empty());
    // A message of exactly the limit passes.
    feed(&mut s, &headers_frame(&mut enc, 3, false)).unwrap();
    feed(&mut s, &data_frame(3, Flags::END_STREAM, &lpm(&[7; 100]))).unwrap();
    assert_eq!(messages(&s.take_events()), [alloc::vec![7; 100]]);
}

/// Spec §3 `max_send_message_size`: a response message above it ends
/// the call with RESOURCE_EXHAUSTED; one of the limit goes out.
#[test]
fn grpcbridge_3_a_message_above_max_send_message_size_is_resource_exhausted() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        max_send_message_size: 10,
        ..ServerLimits::default()
    });
    let resp = |n| GrpcResponse {
        stream_id: 1,
        status: Status::Ok,
        message: None,
        body: alloc::vec![1; n],
    };
    assert_eq!(s.fit_response(resp(10)).status, Status::Ok);
    let refused = s.fit_response(resp(11));
    assert_eq!(refused.status, Status::ResourceExhausted);
    assert!(refused.body.is_empty());
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    assert_eq!(s.send_message(1, &[0; 11], false), Err(SendError::TooLarge));
    assert_eq!(s.send_message(1, &[0; 10], false), Ok(()));
    assert_eq!(s.send_message(9, &[0; 1], false), Err(SendError::Closed));
}

/// Spec §3 `initial_window_size`: the server returns what the client
/// sent with WINDOW_UPDATE on the connection and on the stream once
/// half the window is used; DATA beyond the connection window is a
/// connection error (RFC 9113 §6.9.1) with GOAWAY(FLOW_CONTROL_ERROR).
#[test]
fn grpcbridge_3_the_receive_windows_are_kept_at_initial_window_size() {
    let limits = ServerLimits {
        initial_window_size: 65_535,
        max_frame_size: 65_536,
        ..ServerLimits::default()
    };
    let mut s = GrpcServer::with_limits(limits);
    let _ = s.handshake();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    let mut msg = alloc::vec![0, 0, 0, 0x9c, 0x40];
    msg.extend_from_slice(&[1; 39_995]);
    feed(&mut s, &data_frame(1, 0, &msg)).unwrap();
    let updates: Vec<_> = of_type(&frames(&s.take_output()), FrameType::WindowUpdate)
        .iter()
        .map(|f| (f.2, u32::from_be_bytes(f.3[..4].try_into().unwrap())))
        .collect();
    assert_eq!(updates, alloc::vec![(0, 40_000), (1, 40_000)]);
    let mut t = GrpcServer::with_limits(limits);
    let _ = t.handshake();
    feed(
        &mut t,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    let e = feed(&mut t, &data_frame(1, 0, &[0; 65_536])).unwrap_err();
    assert_eq!(e.code, ErrorCode::FlowControlError);
    let goaway = of_type(&frames(&t.take_output()), FrameType::GoAway);
    assert_eq!(
        goaway[0].3[4..8],
        (ErrorCode::FlowControlError as u32).to_be_bytes()
    );
    assert!(t.is_closing());
}

/// A DATA frame of `len` octets that starts a message declared larger
/// than the frame, so the message stays incomplete.
fn partial_message(len: usize) -> Vec<u8> {
    let mut msg = alloc::vec![0, 0, 0, 0x13, 0x88];
    msg.resize(len, 1);
    msg
}

/// Spec §3 `initial_window_size`, RFC 9113 §6.9.1: DATA beyond a
/// stream's window is a stream error, RST_STREAM(FLOW_CONTROL_ERROR);
/// the connection serves the next stream.
#[test]
fn grpcbridge_3_data_beyond_a_stream_window_resets_the_stream() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        initial_window_size: 1_000,
        ..ServerLimits::default()
    });
    let _ = s.handshake();
    feed(&mut s, &encode_one(FrameType::Settings, Flags::ACK, 0, &[])).unwrap();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    feed(&mut s, &data_frame(1, 0, &partial_message(1_001))).unwrap();
    assert!(!s.is_closing());
    assert!(matches!(
        s.take_events().last(),
        Some(CallEvent::Reset { stream_id: 1, .. })
    ));
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!((f[0].0, f[0].2), (FrameType::RstStream as u8, 1));
    assert_eq!(f[0].3, (ErrorCode::FlowControlError as u32).to_be_bytes());
    // The rest of the stream is ignored, the next stream is served.
    feed(&mut s, &data_frame(1, Flags::END_STREAM, &[0; 10])).unwrap();
    assert!(s.take_output().is_empty());
    feed(&mut s, &headers_frame(&mut enc, 3, false)).unwrap();
    feed(
        &mut s,
        &data_frame(3, Flags::END_STREAM, &[0, 0, 0, 0, 2, 7, 7]),
    )
    .unwrap();
    assert_eq!(messages(&s.take_events()), [alloc::vec![7, 7]]);
    // A frame of the whole window is taken.
    feed(&mut s, &headers_frame(&mut enc, 5, false)).unwrap();
    feed(&mut s, &data_frame(5, 0, &partial_message(1_000))).unwrap();
    assert!(of_type(&frames(&s.take_output()), FrameType::RstStream).is_empty());
}

/// RFC 9113 §6.9.3: until the client acknowledged the SETTINGS, a
/// stream takes the 65 535 octets the client assumes, although
/// `initial_window_size` is smaller; the ACK then lowers the window of
/// the open stream by the difference (§6.9.2).
#[test]
fn grpcbridge_3_a_small_stream_window_applies_after_the_settings_ack() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        initial_window_size: 1_000,
        ..ServerLimits::default()
    });
    let _ = s.handshake();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    feed(&mut s, &data_frame(1, 0, &partial_message(1_001))).unwrap();
    let f = frames(&s.take_output());
    assert!(of_type(&f, FrameType::RstStream).is_empty(), "{f:?}");
    assert!(f.iter().any(|f| f.0 == FrameType::WindowUpdate as u8
        && f.2 == 1
        && f.3 == 1_001u32.to_be_bytes()));
    feed(&mut s, &encode_one(FrameType::Settings, Flags::ACK, 0, &[])).unwrap();
    feed(&mut s, &data_frame(1, 0, &[1; 1_001])).unwrap();
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!((f[0].0, f[0].2), (FrameType::RstStream as u8, 1));
    assert_eq!(f[0].3, (ErrorCode::FlowControlError as u32).to_be_bytes());
}

/// RFC 9113 §6.1: the padding of a PADDED DATA frame is not part of
/// the message; a pad length beyond the frame is a connection error.
#[test]
fn grpcbridge_3_padded_data_is_taken_without_its_padding() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    let mut padded = alloc::vec![4];
    padded.extend_from_slice(&lpm(&[7, 8, 9]));
    padded.extend_from_slice(&[0xee; 4]);
    feed(
        &mut s,
        &data_frame(1, Flags::PADDED | Flags::END_STREAM, &padded),
    )
    .unwrap();
    assert_eq!(messages(&s.take_events()), [alloc::vec![7, 8, 9]]);
    feed(&mut s, &headers_frame(&mut enc, 3, false)).unwrap();
    let mut too_long = alloc::vec![9];
    too_long.extend_from_slice(&lpm(&[7, 8, 9]));
    assert_eq!(
        feed(
            &mut s,
            &data_frame(3, Flags::PADDED | Flags::END_STREAM, &too_long)
        )
        .unwrap_err()
        .code,
        ErrorCode::ProtocolError
    );
    let mut t = GrpcServer::new();
    feed(
        &mut t,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    assert!(feed(&mut t, &data_frame(1, Flags::PADDED, &[])).is_err());
}

/// RFC 9113 §8.1: a call answered before the client ended its side is
/// reset with RST_STREAM(NO_ERROR) after the trailers, and the DATA the
/// client had sent before it saw the reset is ignored (§5.1).
#[test]
fn grpcbridge_4_3_an_answer_before_the_clients_end_resets_the_stream() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    feed(&mut s, &data_frame(1, 0, &lpm(&[5]))).unwrap();
    assert_eq!(s.active_streams(), 1, "the call is open");
    s.respond(&GrpcResponse {
        stream_id: 1,
        status: Status::Ok,
        message: None,
        body: alloc::vec![1],
    });
    let f = frames(&s.take_output());
    let last = f.last().unwrap();
    assert_eq!((last.0, last.2), (FrameType::RstStream as u8, 1));
    assert_eq!(last.3, (ErrorCode::NoError as u32).to_be_bytes());
    assert_eq!(f[f.len() - 2].1, Flags::END_HEADERS | Flags::END_STREAM);
    assert_eq!(s.active_streams(), 0);
    s.take_events();
    feed(&mut s, &data_frame(1, 0, &lpm(&[6]))).unwrap();
    feed(&mut s, &data_frame(1, Flags::END_STREAM, &[])).unwrap();
    assert!(s.take_output().is_empty());
    assert!(s.take_events().is_empty());
    // A call whose client ended its side is not reset.
    feed(&mut s, &headers_frame(&mut enc, 3, false)).unwrap();
    feed(&mut s, &data_frame(3, Flags::END_STREAM, &lpm(&[]))).unwrap();
    s.respond(&GrpcResponse {
        stream_id: 3,
        status: Status::Ok,
        message: None,
        body: Vec::new(),
    });
    assert!(of_type(&frames(&s.take_output()), FrameType::RstStream).is_empty());
}

/// RFC 9113 §5.1: DATA after the client ended the stream is
/// RST_STREAM(STREAM_CLOSED) and the call ends; DATA on a stream never
/// opened is a connection error.
#[test]
fn grpcbridge_4_3_data_on_a_stream_the_client_closed_is_stream_closed() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    feed(&mut s, &data_frame(1, Flags::END_STREAM, &lpm(&[]))).unwrap();
    s.take_events();
    feed(&mut s, &data_frame(1, 0, &lpm(&[4]))).unwrap();
    assert!(matches!(
        s.take_events()[..],
        [CallEvent::Reset { stream_id: 1, code }] if code == ErrorCode::StreamClosed as u32
    ));
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0, f[0].2), (FrameType::RstStream as u8, 1));
    assert_eq!(f[0].3, (ErrorCode::StreamClosed as u32).to_be_bytes());
    assert_eq!(
        feed(&mut s, &data_frame(5, 0, &lpm(&[]))).unwrap_err().code,
        ErrorCode::ProtocolError
    );
}

/// Spec §3 `max_frame_size`: a frame above it is a connection error
/// (FRAME_SIZE_ERROR) as soon as its header is read; one of it is taken.
#[test]
fn grpcbridge_3_a_frame_above_max_frame_size_is_refused() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    let big = data_frame(1, 0, &[0; 16_385]);
    assert_eq!(
        s.process_frame(&big[..9]).unwrap_err().code,
        ErrorCode::FrameSizeError
    );
    let mut t = GrpcServer::with_limits(ServerLimits {
        max_frame_size: 32_768,
        ..ServerLimits::default()
    });
    feed(
        &mut t,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    assert_eq!(t.process_frame(&big[..9]), Ok(0), "waits for the payload");
    assert!(feed(&mut t, &big).is_ok());
}

/// Spec §3 `keepalive.permit_without_calls: false`: PINGs without a
/// call are answered until the third strike, then GOAWAY
/// ENHANCE_YOUR_CALM; with `true` every one is answered.
#[test]
fn grpcbridge_3_pings_without_a_call_follow_permit_without_calls() {
    let ping = encode_one(FrameType::Ping, 0, 0, &[9; 8]);
    let mut s = GrpcServer::new();
    for _ in 0..3 {
        feed(&mut s, &ping).unwrap();
        assert!(!s.is_closing());
    }
    feed(&mut s, &ping).unwrap();
    assert!(s.is_closing());
    let f = frames(&s.take_output());
    let goaway = f.last().unwrap();
    assert_eq!(goaway.0, FrameType::GoAway as u8);
    assert_eq!(
        goaway.3[4..8],
        (ErrorCode::EnhanceYourCalm as u32).to_be_bytes()
    );
    assert_eq!(&goaway.3[8..], b"too_many_pings");

    let mut permitted = GrpcServer::with_limits(ServerLimits {
        permit_pings_without_calls: true,
        ..ServerLimits::default()
    });
    for _ in 0..10 {
        feed(&mut permitted, &ping).unwrap();
    }
    assert!(!permitted.is_closing());
    // Server HEADERS in between reset the strikes.
    let mut reset = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    for i in 0..10u32 {
        feed(&mut reset, &ping).unwrap();
        let sid = 2 * i + 1;
        feed(&mut reset, &headers_frame(&mut enc, sid, true)).unwrap();
        reset.respond(&GrpcResponse {
            stream_id: sid,
            status: Status::Ok,
            message: None,
            body: Vec::new(),
        });
        let _ = reset.take_output();
    }
    assert!(!reset.is_closing());
}

/// Spec §6 Liveliness: the keepalive PING goes out and its ACK is seen.
#[test]
fn grpcbridge_3_a_keepalive_ping_and_its_ack() {
    let mut s = GrpcServer::new();
    s.keepalive_ping([1, 2, 3, 4, 5, 6, 7, 8]);
    let f = frames(&s.take_output());
    assert_eq!((f[0].0, f[0].1), (FrameType::Ping as u8, 0));
    assert!(!s.ping_acked([1, 2, 3, 4, 5, 6, 7, 8]));
    feed(
        &mut s,
        &encode_one(FrameType::Ping, Flags::ACK, 0, &[1, 2, 3, 4, 5, 6, 7, 8]),
    )
    .unwrap();
    assert!(s.ping_acked([1, 2, 3, 4, 5, 6, 7, 8]));
}

/// §4.1: a message larger than the client's MAX_FRAME_SIZE goes out in
/// several DATA frames (RFC 9113 §4.2), within the client's window.
#[test]
fn grpcbridge_4_1_a_message_above_max_frame_size_is_split() {
    let mut s = GrpcServer::new();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, true),
    )
    .unwrap();
    // 40 005 octets fit the client's 65 535.
    s.send_message(1, &[5; 40_000], false).unwrap();
    let data = of_type(&frames(&s.take_output()), FrameType::Data);
    assert_eq!(data.len(), 3);
    assert!(data.iter().all(|f| f.3.len() <= 16_384));
    assert_eq!(data.iter().map(|f| f.3.len()).sum::<usize>(), 40_005);
    // The client's larger MAX_FRAME_SIZE is used.
    let mut t = GrpcServer::new();
    let settings = encode_settings(&[Setting {
        id: SettingId::MaxFrameSize,
        value: 32_768,
    }]);
    feed(&mut t, &encode_one(FrameType::Settings, 0, 0, &settings)).unwrap();
    feed(
        &mut t,
        &headers_frame(&mut HpackEncoder::default(), 1, true),
    )
    .unwrap();
    t.send_message(1, &[5; 40_000], false).unwrap();
    let data = of_type(&frames(&t.take_output()), FrameType::Data);
    assert_eq!(
        data.iter().map(|f| f.3.len()).collect::<Vec<_>>(),
        [32_768, 7_237]
    );
}

/// §4.1, RFC 9113 §6.9: the server's DATA waits for the client's window —
/// of the stream and of the connection — and goes out after its
/// WINDOW_UPDATE; the trailers wait behind the DATA. A SETTINGS
/// `INITIAL_WINDOW_SIZE` change applies to the open stream (§6.9.2).
#[test]
fn grpcbridge_4_1_data_waits_for_the_clients_window() {
    let mut s = GrpcServer::new();
    let settings = encode_settings(&[Setting {
        id: SettingId::InitialWindowSize,
        value: 100,
    }]);
    feed(&mut s, &encode_one(FrameType::Settings, 0, 0, &settings)).unwrap();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, true),
    )
    .unwrap();
    s.send_message(1, &[1; 295], false).unwrap();
    s.close_call(1, Status::Ok, None);
    let f = frames(&s.take_output());
    let data = of_type(&f, FrameType::Data);
    assert_eq!(data.iter().map(|f| f.3.len()).sum::<usize>(), 100);
    assert_eq!(of_type(&f, FrameType::Headers).len(), 1, "trailers wait");
    assert_eq!(s.queued(1), 200);
    assert!(frames(&s.take_output()).is_empty());
    // A stream WINDOW_UPDATE of 150 lets 150 more go.
    feed(
        &mut s,
        &encode_one(FrameType::WindowUpdate, 0, 1, &150u32.to_be_bytes()),
    )
    .unwrap();
    let data = of_type(&frames(&s.take_output()), FrameType::Data);
    assert_eq!(data.iter().map(|f| f.3.len()).sum::<usize>(), 150);
    // SETTINGS raises the window of the open stream by 1 000 - 100.
    let settings = encode_settings(&[Setting {
        id: SettingId::InitialWindowSize,
        value: 1_000,
    }]);
    feed(&mut s, &encode_one(FrameType::Settings, 0, 0, &settings)).unwrap();
    let f = frames(&s.take_output());
    assert_eq!(
        of_type(&f, FrameType::Data)
            .iter()
            .map(|f| f.3.len())
            .sum::<usize>(),
        50
    );
    let trailers = of_type(&f, FrameType::Headers);
    assert_eq!(trailers[0].1, Flags::END_HEADERS | Flags::END_STREAM);
    assert_eq!(s.active_streams(), 0);

    // The connection window: 65 535 for all streams together.
    let mut c = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    for sid in [1, 3] {
        feed(&mut c, &headers_frame(&mut enc, sid, true)).unwrap();
        c.send_message(sid, &[2; 40_000], false).unwrap();
    }
    let sent = of_type(&frames(&c.take_output()), FrameType::Data)
        .iter()
        .map(|f| f.3.len())
        .sum::<usize>();
    assert_eq!(sent, 65_535);
    assert_eq!(c.send_windows(1).0, 0);
    feed(
        &mut c,
        &encode_one(FrameType::WindowUpdate, 0, 0, &20_000u32.to_be_bytes()),
    )
    .unwrap();
    let sent = of_type(&frames(&c.take_output()), FrameType::Data)
        .iter()
        .map(|f| f.3.len())
        .sum::<usize>();
    assert_eq!(sent, 80_010 - 65_535);
    // A window above 2^31-1 is a connection error.
    let e = feed(
        &mut c,
        &encode_one(FrameType::WindowUpdate, 0, 0, &0x7fff_ffffu32.to_be_bytes()),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::FlowControlError);
}

/// §4.1, RFC 9113 §6.10: a header block in HEADERS and CONTINUATION
/// frames is one block; another frame inside it is a connection error.
#[test]
fn grpcbridge_4_1_a_header_block_in_continuation_frames_is_read() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    let block = enc.encode(&request_fields(
        "/zerodds.t.v1.TStream/Publish",
        &[
            ("content-type", "application/grpc"),
            ("x-long", &"v".repeat(3_000)),
        ],
    ));
    let (a, rest) = block.split_at(10);
    let (b, c) = rest.split_at(1_000);
    feed(&mut s, &encode_raw(FrameType::Headers, 0, 1, a)).unwrap();
    assert!(s.take_events().is_empty());
    feed(&mut s, &encode_raw(FrameType::Continuation, 0, 1, b)).unwrap();
    feed(
        &mut s,
        &encode_raw(FrameType::Continuation, Flags::END_HEADERS, 1, c),
    )
    .unwrap();
    let events = s.take_events();
    let Some(CallEvent::Open(call)) = events.first() else {
        panic!("{events:?}");
    };
    assert_eq!(call.method, "Publish");
    assert!(
        call.metadata
            .iter()
            .any(|(n, v)| n == "x-long" && v.len() == 3_000)
    );
    // A DATA frame inside a header block.
    let mut t = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    let block = enc.encode(&request_fields("/a.B/C", &[]));
    feed(&mut t, &encode_raw(FrameType::Headers, 0, 1, &block)).unwrap();
    let e = feed(&mut t, &data_frame(1, 0, &lpm(&[]))).unwrap_err();
    assert_eq!(e.code, ErrorCode::ProtocolError);
    // A CONTINUATION without a header block.
    let mut u = GrpcServer::new();
    let e = feed(
        &mut u,
        &encode_raw(FrameType::Continuation, Flags::END_HEADERS, 1, &[]),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::ProtocolError);
}

/// §4.1, RFC 9113 §10.5.1: a header block of HEADERS and CONTINUATION
/// frames above 1 MiB is a connection error ENHANCE_YOUR_CALM, which goes
/// out in GOAWAY; a block of exactly 1 MiB is still taken.
#[test]
fn grpcbridge_4_1_a_header_block_above_1_mib_is_enhance_your_calm() {
    let mut s = GrpcServer::new();
    let chunk = alloc::vec![0u8; 16_384];
    feed(&mut s, &encode_raw(FrameType::Headers, 0, 1, &chunk)).unwrap();
    // 64 frames of 16 KiB: 1 MiB, no END_HEADERS yet.
    for _ in 1..64 {
        feed(&mut s, &encode_raw(FrameType::Continuation, 0, 1, &chunk)).unwrap();
    }
    assert_eq!(MAX_HEADER_BLOCK, 64 * chunk.len());
    assert!(s.take_events().is_empty());
    let e = feed(&mut s, &encode_raw(FrameType::Continuation, 0, 1, &[0])).unwrap_err();
    assert_eq!(e.code, ErrorCode::EnhanceYourCalm);
    let goaway = of_type(&frames(&s.take_output()), FrameType::GoAway);
    assert_eq!(goaway.len(), 1);
    assert_eq!(
        goaway[0].3[4..8],
        (ErrorCode::EnhanceYourCalm as u32).to_be_bytes()
    );
}

/// §4.1: a single HEADERS frame (with a `max_frame_size` above 1 MiB) whose
/// header block is above 1 MiB is ENHANCE_YOUR_CALM as well, with
/// END_HEADERS and without; a block of exactly 1 MiB is taken.
#[test]
fn grpcbridge_4_1_a_headers_frame_above_1_mib_is_enhance_your_calm() {
    let limits = ServerLimits {
        max_frame_size: 2 * 1024 * 1024,
        ..ServerLimits::default()
    };
    for flags in [0, Flags::END_HEADERS] {
        let mut s = GrpcServer::with_limits(limits);
        let block = alloc::vec![0u8; MAX_HEADER_BLOCK + 1];
        let e = feed(&mut s, &encode_raw(FrameType::Headers, flags, 1, &block)).unwrap_err();
        assert_eq!(e.code, ErrorCode::EnhanceYourCalm, "flags {flags}");
        let goaway = of_type(&frames(&s.take_output()), FrameType::GoAway);
        assert_eq!(
            goaway[0].3[4..8],
            (ErrorCode::EnhanceYourCalm as u32).to_be_bytes()
        );
    }
    // Padding is not part of the block: 1 MiB of block and 200 of padding.
    let mut s = GrpcServer::with_limits(limits);
    let mut payload = alloc::vec![200u8];
    payload.extend_from_slice(&alloc::vec![0u8; MAX_HEADER_BLOCK + 200]);
    feed(
        &mut s,
        &encode_raw(FrameType::Headers, Flags::PADDED, 1, &payload),
    )
    .unwrap();
    assert!(s.take_events().is_empty());
}

/// The block of the review: `:method POST`, `:path`, a literal of 4 000
/// octets with incremental indexing (name `x`), then `n` references to it
/// (0xbe, the first dynamic entry).
fn amplifying_block(n: usize) -> Vec<u8> {
    let mut b = alloc::vec![0x83, 0x04];
    b.extend_from_slice(&zerodds_hpack::encode_string("/a.B/C", false));
    b.push(0x40);
    b.extend_from_slice(&zerodds_hpack::encode_string("x", false));
    b.extend_from_slice(&zerodds_hpack::encode_string(&"v".repeat(4000), false));
    b.extend(core::iter::repeat_n(0xbe, n));
    b
}

/// §4.1, RFC 9113 §6.5.2, §10.5.1: a request whose header list decodes to
/// more than MAX_HEADER_LIST_SIZE (a block of 64 015 octets, as in the
/// review) is refused with RESOURCE_EXHAUSTED without a connection error;
/// the block was decoded to its end, so the next request may refer to its
/// literal. A list of exactly the limit is a call, one octet more is
/// refused.
#[test]
fn grpcbridge_4_1_a_header_list_above_the_limit_is_resource_exhausted() {
    let mut s = GrpcServer::new();
    let block = amplifying_block(60_000);
    assert_eq!(block.len(), 64_015);
    let (a, rest) = block.split_at(16_384);
    feed(
        &mut s,
        &encode_raw(FrameType::Headers, Flags::END_STREAM, 1, a),
    )
    .unwrap();
    let mut chunks = rest.chunks(16_384).peekable();
    while let Some(c) = chunks.next() {
        let flags = if chunks.peek().is_none() {
            Flags::END_HEADERS
        } else {
            0
        };
        feed(&mut s, &encode_raw(FrameType::Continuation, flags, 1, c)).unwrap();
    }
    let events = s.take_events();
    let [CallEvent::Refused(r)] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!((r.stream_id, r.status), (1, Status::ResourceExhausted));
    assert!(r.message.contains("16384"), "{}", r.message);
    let (status, message) = (r.status, r.message.clone());
    s.close_call(1, status, Some(&message));
    let f = frames(&s.take_output());
    assert_eq!(of_type(&f, FrameType::Headers).len(), 2);
    assert!(of_type(&f, FrameType::GoAway).is_empty());
    // The next request refers to the literal of the refused block:
    // :method POST (43), :path /a.B/C (43), three times x (4 033 each).
    let mut next = alloc::vec![0x83, 0x04];
    next.extend_from_slice(&zerodds_hpack::encode_string("/a.B/C", false));
    next.extend_from_slice(&[0xbe; 3]);
    feed(
        &mut s,
        &encode_raw(FrameType::Headers, Flags::END_HEADERS, 3, &next),
    )
    .unwrap();
    let events = s.take_events();
    let Some(CallEvent::Open(call)) = events.first() else {
        panic!("{events:?}");
    };
    assert_eq!(call.metadata.len(), 3);
    assert!(
        call.metadata
            .iter()
            .all(|(n, v)| n == "x" && v.len() == 4000)
    );
    // A list of exactly the limit is a call, one octet more is refused:
    // 43 + 43 + 4 033 + (32 + 4 + 12 229) = 16 384.
    for (pad, refused) in [(12_229, false), (12_230, true)] {
        let mut t = GrpcServer::new();
        let block = HpackEncoder::default().encode(&request_fields(
            "/a.B/C",
            &[("x", &"v".repeat(4000)), ("pad1", &"p".repeat(pad))],
        ));
        feed(
            &mut t,
            &encode_raw(FrameType::Headers, Flags::END_HEADERS, 1, &block),
        )
        .unwrap();
        let events = t.take_events();
        assert_eq!(
            matches!(events.first(), Some(CallEvent::Refused(_))),
            refused,
            "{pad}: {events:?}"
        );
        assert_eq!(
            matches!(events.first(), Some(CallEvent::Open(_))),
            !refused,
            "{pad}"
        );
    }
}

/// RFC 7541 §6.3, RFC 9113 §4.3: a dynamic table size update above the
/// 4 096 octets of SETTINGS_HEADER_TABLE_SIZE (the server announces no other
/// value) is a connection error COMPRESSION_ERROR; one to 4 096 is taken.
#[test]
fn grpcbridge_4_1_a_table_size_update_above_4096_is_compression_error() {
    let mut s = GrpcServer::new();
    let request = HpackEncoder::default().encode(&request_fields("/a.B/C", &[]));
    let mut block = zerodds_hpack::encode_integer(4096, 5, 0x20);
    block.extend_from_slice(&request);
    feed(
        &mut s,
        &encode_raw(FrameType::Headers, Flags::END_HEADERS, 1, &block),
    )
    .unwrap();
    assert!(matches!(s.take_events().first(), Some(CallEvent::Open(_))));
    let mut block = zerodds_hpack::encode_integer(1 << 20, 5, 0x20);
    block.extend_from_slice(&request);
    let e = feed(
        &mut s,
        &encode_raw(FrameType::Headers, Flags::END_HEADERS, 3, &block),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::CompressionError);
}

/// §4.6, RFC 9113 §6.9: a held stream gets no WINDOW_UPDATE however much
/// of its window the client uses; its messages are still events, and the
/// connection window is returned. Released, the window used meanwhile goes
/// back in one WINDOW_UPDATE.
#[test]
fn grpcbridge_4_6_a_held_stream_returns_no_window_until_released() {
    let mut s = GrpcServer::with_limits(ServerLimits {
        initial_window_size: 65_535,
        ..ServerLimits::default()
    });
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    s.hold_window(1, true);
    let _ = s.take_output();
    // 40 messages of 1 000 octets with their prefix: above half the window.
    for _ in 0..40 {
        feed(&mut s, &data_frame(1, 0, &lpm(&[1; 995]))).unwrap();
    }
    assert_eq!(messages(&s.take_events()).len(), 40);
    let f = frames(&s.take_output());
    let updates = |f: &[Frame], sid: u32| -> Vec<u32> {
        f.iter()
            .filter(|f| f.0 == FrameType::WindowUpdate as u8 && f.2 == sid)
            .map(|f| u32::from_be_bytes(f.3[..4].try_into().unwrap()))
            .collect()
    };
    assert!(updates(&f, 1).is_empty(), "{:?}", updates(&f, 1));
    assert_eq!(updates(&f, 0).len(), 1, "the connection window goes back");
    s.hold_window(1, false);
    let f = frames(&s.take_output());
    assert_eq!(updates(&f, 1), [40_000]);
    // Not held, the window goes back at half of it as before.
    for _ in 0..33 {
        feed(&mut s, &data_frame(1, 0, &lpm(&[1; 995]))).unwrap();
    }
    assert_eq!(updates(&frames(&s.take_output()), 1), [33_000]);
}

/// §4.1, RFC 9113 §6.2: the padding and the priority fields of HEADERS
/// are not part of the header block.
#[test]
fn grpcbridge_4_1_padded_headers_and_data_are_read() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    let block = enc.encode(&request_fields("/zerodds.t.v1.TStream/PublishOne", &[]));
    let mut payload = alloc::vec![3]; // pad length
    payload.extend_from_slice(&[0x80, 0, 0, 0, 16]); // priority: exclusive, weight
    payload.extend_from_slice(&block);
    payload.extend_from_slice(&[0; 3]);
    feed(
        &mut s,
        &encode_raw(
            FrameType::Headers,
            Flags::END_HEADERS | Flags::PADDED | Flags::PRIORITY,
            1,
            &payload,
        ),
    )
    .unwrap();
    let mut padded = alloc::vec![2];
    padded.extend_from_slice(&lpm(&[1, 2]));
    padded.extend_from_slice(&[0; 2]);
    feed(
        &mut s,
        &data_frame(1, Flags::PADDED | Flags::END_STREAM, &padded),
    )
    .unwrap();
    let events = s.take_events();
    assert!(matches!(&events[0], CallEvent::Open(c) if c.method == "PublishOne"));
    assert_eq!(messages(&events), [alloc::vec![1, 2]]);
    assert!(matches!(
        events.last(),
        Some(CallEvent::HalfClose { stream_id: 1 })
    ));
    // Padding longer than the frame.
    let mut t = GrpcServer::new();
    let e = feed(
        &mut t,
        &encode_raw(
            FrameType::Headers,
            Flags::END_HEADERS | Flags::PADDED,
            1,
            &[9, 0x82],
        ),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::ProtocolError);
}

/// §4.1, RFC 9113 §4.1, §5.5, §6.3, §8.4: a frame of an unknown type and
/// PRIORITY are ignored; PUSH_PROMISE from a client is a connection error.
#[test]
fn grpcbridge_4_1_unknown_frames_are_ignored_and_push_promise_is_refused() {
    let mut s = GrpcServer::new();
    let mut unknown = encode_raw(FrameType::Data, 0, 0, &[1, 2, 3]);
    unknown[3] = 0x42;
    feed(&mut s, &unknown).unwrap();
    feed(
        &mut s,
        &encode_raw(FrameType::Priority, 0, 1, &[0, 0, 0, 0, 7]),
    )
    .unwrap();
    assert!(s.take_output().is_empty());
    assert!(s.take_events().is_empty());
    let e = feed(
        &mut s,
        &encode_raw(FrameType::PushPromise, 0, 1, &[0, 0, 0, 2]),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::ProtocolError);
    let goaway = of_type(&frames(&s.take_output()), FrameType::GoAway);
    assert_eq!(goaway.len(), 1);
    assert_eq!(
        goaway[0].3[4..8],
        (ErrorCode::ProtocolError as u32).to_be_bytes()
    );
}

/// §4.1: a response header block larger than the client's
/// MAX_FRAME_SIZE continues in CONTINUATION frames; a smaller HPACK
/// table of the client is announced at the start of the next block
/// (RFC 7541 §4.2), which the client's decoder reads.
#[test]
fn grpcbridge_4_1_a_large_header_block_goes_out_in_continuation_frames() {
    let mut s = GrpcServer::new();
    let settings = encode_settings(&[Setting {
        id: SettingId::HeaderTableSize,
        value: 0,
    }]);
    feed(&mut s, &encode_one(FrameType::Settings, 0, 0, &settings)).unwrap();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, true),
    )
    .unwrap();
    let long = "x".repeat(40_000);
    s.close_call(1, Status::Internal, Some(&long));
    let f = frames(&s.take_output());
    let blocks: Vec<_> = f
        .iter()
        .filter(|f| f.0 == FrameType::Headers as u8 || f.0 == FrameType::Continuation as u8)
        .collect();
    // Response HEADERS, then trailers in HEADERS + CONTINUATION frames.
    assert!(blocks.len() >= 3, "{}", blocks.len());
    assert!(blocks.iter().all(|f| f.3.len() <= 16_384));
    assert_eq!(blocks[0].3[0], 0x20, "table size update to 0");
    let mut dec = HpackDecoder::with_max_size(0);
    dec.decode(&blocks[0].3).unwrap();
    let mut trailer_block = Vec::new();
    for b in &blocks[1..] {
        trailer_block.extend_from_slice(&b.3);
    }
    assert_eq!(
        blocks.last().unwrap().1 & Flags::END_HEADERS,
        Flags::END_HEADERS
    );
    assert_eq!(blocks[1].1 & Flags::END_HEADERS, 0);
    let trailers = dec.decode(&trailer_block).unwrap();
    assert!(
        trailers
            .iter()
            .any(|h| h.name == "grpc-message" && h.value == long)
    );
    assert_eq!(dec.table().len(), 0, "nothing indexed with a table of 0");
}

/// §4.3: a client stream delivers each message as it completes and the
/// half-close after the last; the call stays open for the answer.
#[test]
fn grpcbridge_4_3_a_client_stream_delivers_each_message_and_the_half_close() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    for i in 0..5u8 {
        feed(&mut s, &data_frame(1, 0, &lpm(&[i; 3]))).unwrap();
    }
    feed(&mut s, &data_frame(1, Flags::END_STREAM, &[])).unwrap();
    let events = s.take_events();
    assert!(matches!(&events[0], CallEvent::Open(c) if !c.end_stream));
    assert_eq!(
        messages(&events),
        (0..5u8).map(|i| alloc::vec![i; 3]).collect::<Vec<_>>()
    );
    assert!(matches!(
        events.last(),
        Some(CallEvent::HalfClose { stream_id: 1 })
    ));
    assert!(s.is_open(1));
    assert!(s.client_done(1));
    s.respond(&GrpcResponse {
        stream_id: 1,
        status: Status::Ok,
        message: None,
        body: alloc::vec![8],
    });
    let f = frames(&s.take_output());
    assert!(of_type(&f, FrameType::RstStream).is_empty());
    assert_eq!(s.active_streams(), 0);
}

/// §4.3: two messages in one DATA frame are two messages; the
/// Compressed-Flag of each is passed on.
#[test]
fn grpcbridge_4_3_two_messages_in_one_data_frame_are_two_messages() {
    let mut s = GrpcServer::new();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    let mut two = lpm(&[1]);
    let mut second = lpm(&[2, 2]);
    second[0] = 1;
    two.extend_from_slice(&second);
    feed(&mut s, &data_frame(1, 0, &two)).unwrap();
    let events = s.take_events();
    let flags: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            CallEvent::Message {
                compressed,
                message,
                ..
            } => Some((*compressed, message.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(flags, [(false, alloc::vec![1]), (true, alloc::vec![2, 2])]);
}

/// §4.3: one message across three DATA frames is one message.
#[test]
fn grpcbridge_4_3_a_message_across_data_frames_is_one_message() {
    let mut s = GrpcServer::new();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    let whole = lpm(&[3; 30_000]);
    for part in whole.chunks(12_000) {
        feed(&mut s, &data_frame(1, 0, part)).unwrap();
    }
    let events = s.take_events();
    assert_eq!(messages(&events), [alloc::vec![3; 30_000]]);
}

/// §4.3: a stream that ends inside a message is refused with INTERNAL.
#[test]
fn grpcbridge_4_3_a_stream_that_ends_inside_a_message_is_internal() {
    let mut s = GrpcServer::new();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, false),
    )
    .unwrap();
    feed(
        &mut s,
        &data_frame(1, Flags::END_STREAM, &[0, 0, 0, 0, 9, 1]),
    )
    .unwrap();
    let events = s.take_events();
    assert!(matches!(
        events.last(),
        Some(CallEvent::Refused(r)) if r.status == Status::Internal
    ));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CallEvent::HalfClose { .. }))
    );
}

/// §4.3: the call carries its request headers: `:method`,
/// `content-type`, `grpc-encoding`, `grpc-accept-encoding`,
/// `grpc-timeout` and the metadata; a malformed `:path` is UNIMPLEMENTED,
/// a request without `:path` is malformed (RST_STREAM(PROTOCOL_ERROR)).
#[test]
fn grpcbridge_4_3_the_call_carries_its_request_headers() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    let block = enc.encode(&request_fields(
        "/zerodds.t.v1.TStream/Subscribe",
        &[
            ("content-type", "application/grpc+proto; charset=x"),
            ("grpc-encoding", "gzip"),
            ("grpc-accept-encoding", "gzip,identity"),
            ("grpc-timeout", "250m"),
            ("authorization", "Bearer t"),
        ],
    ));
    feed(
        &mut s,
        &encode_raw(FrameType::Headers, Flags::END_HEADERS, 1, &block),
    )
    .unwrap();
    let events = s.take_events();
    let Some(CallEvent::Open(call)) = events.first() else {
        panic!("{events:?}");
    };
    assert_eq!(call.http_method, "POST");
    assert_eq!(call.service, "zerodds.t.v1.TStream");
    assert_eq!(call.encoding.as_deref(), Some("gzip"));
    assert_eq!(call.accept_encoding.as_deref(), Some("gzip,identity"));
    assert_eq!(call.timeout.as_deref(), Some("250m"));
    assert!(
        call.metadata
            .contains(&("authorization".into(), "Bearer t".into()))
    );
    assert!(call.content_type_is_grpc_proto());
    let mut json = call.clone();
    json.content_type = Some("application/grpc+json".into());
    assert!(json.content_type_is_grpc() && !json.content_type_is_grpc_proto());
    json.content_type = Some("text/plain".into());
    assert!(!json.content_type_is_grpc());
    // A multi-byte UTF-8 character across the 16th octet: not gRPC, and
    // no panic.
    for ct in ["application/grp\u{e4}", "application/grpc\u{e4}", "\u{e4}"] {
        json.content_type = Some(ct.into());
        assert_eq!(
            json.content_type_is_grpc(),
            ct.starts_with("application/grpc")
        );
        assert!(!json.content_type_is_grpc_proto());
    }

    let block = enc.encode(&request_fields("no-slash", &[]));
    feed(
        &mut s,
        &encode_raw(
            FrameType::Headers,
            Flags::END_HEADERS | Flags::END_STREAM,
            3,
            &block,
        ),
    )
    .unwrap();
    assert!(matches!(
        s.take_events()[..],
        [CallEvent::Refused(ref r)] if r.status == Status::Unimplemented
    ));
    let block = enc.encode(&[field(":method", "POST")]);
    feed(
        &mut s,
        &encode_raw(
            FrameType::Headers,
            Flags::END_HEADERS | Flags::END_STREAM,
            5,
            &block,
        ),
    )
    .unwrap();
    assert!(s.take_events().is_empty());
    let rst = of_type(&frames(&s.take_output()), FrameType::RstStream);
    assert_eq!(rst[0].2, 5);
    assert_eq!(rst[0].3, (ErrorCode::ProtocolError as u32).to_be_bytes());
}

/// §4.3, gRPC over HTTP/2: a request that is not gRPC gets HTTP 415 with
/// `grpc-status` in one HEADERS frame that ends the stream.
#[test]
fn grpcbridge_4_3_reject_answers_with_the_http_status() {
    let mut s = GrpcServer::new();
    feed(
        &mut s,
        &headers_frame(&mut HpackEncoder::default(), 1, true),
    )
    .unwrap();
    s.reject(1, 415, Status::InvalidArgument, "content-type text/plain");
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].1, Flags::END_HEADERS | Flags::END_STREAM);
    let h = HpackDecoder::default().decode(&f[0].3).unwrap();
    assert!(h.iter().any(|h| h.name == ":status" && h.value == "415"));
    assert!(h.iter().any(|h| h.name == "grpc-status" && h.value == "3"));
    assert_eq!(s.active_streams(), 0);
}

/// gRPC over HTTP/2, "Responses": `grpc-message` is percent-encoded.
#[test]
fn grpcbridge_4_5_grpc_message_is_percent_encoded() {
    assert_eq!(percent_encode("a b"), "a b");
    assert_eq!(percent_encode("50% \u{e4}\n"), "50%25 %C3%A4%0A");
}

/// §5.2: a GET is passed on as a call with its path (also one that is no
/// `/<service>/<method>`) and its `:authority`; `respond_http` answers it
/// with `:status`, `content-type`, `content-length` and the body, the last
/// DATA frame with END_STREAM, and the stream is gone.
#[test]
fn grpcbridge_5_2_a_get_is_answered_with_a_plain_response() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    let block = enc.encode(&[
        field(":method", "GET"),
        field(":path", "/idl/rt/chatter"),
        field(":authority", "example:50051"),
    ]);
    feed(
        &mut s,
        &encode_raw(
            FrameType::Headers,
            Flags::END_HEADERS | Flags::END_STREAM,
            1,
            &block,
        ),
    )
    .unwrap();
    let events = s.take_events();
    let CallEvent::Open(call) = &events[0] else {
        panic!("{events:?}")
    };
    assert_eq!(call.http_method, "GET");
    assert_eq!(call.path, "/idl/rt/chatter");
    assert_eq!(call.authority.as_deref(), Some("example:50051"));
    let body = alloc::vec![b'x'; 20_000];
    s.respond_http(1, 200, "text/x-omg-idl", &body);
    let f = frames(&s.take_output());
    let headers = of_type(&f, FrameType::Headers);
    let fields = HpackDecoder::default().decode(&headers[0].3).unwrap();
    for (n, v) in [
        (":status", "200"),
        ("content-type", "text/x-omg-idl"),
        ("content-length", "20000"),
    ] {
        assert!(fields.iter().any(|h| h.name == n && h.value == v), "{n}");
    }
    assert_eq!(headers[0].1 & Flags::END_STREAM, 0);
    let data = of_type(&f, FrameType::Data);
    assert_eq!(data.len(), 2, "20 000 octets in frames of 16 384");
    assert_eq!(data[0].1 & Flags::END_STREAM, 0);
    assert_eq!(data[1].1, Flags::END_STREAM);
    assert_eq!(data.iter().map(|d| d.3.len()).sum::<usize>(), 20_000);
    assert_eq!(s.active_streams(), 0);
    // An empty body ends the stream with the HEADERS.
    let block = enc.encode(&[field(":method", "GET"), field(":path", "/idl/none")]);
    feed(
        &mut s,
        &encode_raw(
            FrameType::Headers,
            Flags::END_HEADERS | Flags::END_STREAM,
            3,
            &block,
        ),
    )
    .unwrap();
    s.respond_http(3, 404, "text/plain", b"");
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].1, Flags::END_HEADERS | Flags::END_STREAM);
}

/// Spec §9.2, RFC 9113 §6.8: the graceful GOAWAY names the last stream the
/// client opened and NO_ERROR, the connection stays open; the stream it
/// names keeps its call, a stream opened after it is refused with
/// REFUSED_STREAM and its DATA dropped; a second call sends nothing.
#[test]
fn grpcbridge_9_2_begin_drain_serves_the_open_streams_and_refuses_new_ones() {
    let mut s = GrpcServer::new();
    let mut enc = HpackEncoder::default();
    feed(&mut s, &headers_frame(&mut enc, 1, false)).unwrap();
    feed(&mut s, &headers_frame(&mut enc, 3, false)).unwrap();
    let _ = s.take_events();
    assert_eq!(s.draining(), None);
    s.begin_drain();
    assert_eq!(s.draining(), Some(3));
    assert!(
        !s.is_closing(),
        "the graceful GOAWAY does not end the connection"
    );
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0, f[0].2), (FrameType::GoAway as u8, 0));
    assert_eq!(f[0].3[..4], 3u32.to_be_bytes());
    assert_eq!(f[0].3[4..8], (ErrorCode::NoError as u32).to_be_bytes());
    s.begin_drain();
    assert!(s.take_output().is_empty(), "one GOAWAY");
    // A new stream after the GOAWAY: refused, its DATA dropped.
    feed(&mut s, &headers_frame(&mut enc, 5, false)).unwrap();
    let f = frames(&s.take_output());
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0, f[0].2), (FrameType::RstStream as u8, 5));
    assert_eq!(f[0].3, (ErrorCode::RefusedStream as u32).to_be_bytes());
    feed(&mut s, &data_frame(5, Flags::END_STREAM, &lpm(&[1]))).unwrap();
    assert!(s.take_events().is_empty());
    // The streams up to 3 are served on.
    feed(&mut s, &data_frame(3, Flags::END_STREAM, &lpm(&[7]))).unwrap();
    let events = s.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CallEvent::Message { stream_id: 3, .. })),
        "{events:?}"
    );
    s.respond(&GrpcResponse {
        stream_id: 3,
        status: Status::Ok,
        message: None,
        body: Vec::new(),
    });
    let f = frames(&s.take_output());
    assert!(
        f.iter()
            .any(|f| f.0 == FrameType::Headers as u8 && f.2 == 3)
    );
    assert_eq!(s.active_streams(), 1);
}
