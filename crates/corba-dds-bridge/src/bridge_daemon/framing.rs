// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Reply bodies, outgoing fragments and the reassembly of incoming ones (spec §4.2, §4.6).

#[allow(unused_imports)]
use super::prelude::*;

/// The completion status of a system exception (CORBA §4.12.3).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Completion {
    No = 1,
    Maybe = 2,
}

/// A `SystemExceptionReplyBody` (GIOP §15.4.3.3) in `endianness`, as the
/// body of a GIOP 1.2 reply (8-aligned from the message start, so the
/// body's own offsets are the alignment).
pub(crate) fn system_exception_body(
    name: &str,
    minor: u32,
    completed: Completion,
    endianness: Endianness,
) -> Vec<u8> {
    let id = format!("IDL:omg.org/CORBA/{name}:1.0");
    let u32b = |v: u32| match endianness {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    };
    let mut b = Vec::new();
    b.extend_from_slice(&u32b(u32::try_from(id.len() + 1).unwrap_or(0)));
    b.extend_from_slice(id.as_bytes());
    b.push(0);
    while b.len() % 4 != 0 {
        b.push(0);
    }
    b.extend_from_slice(&u32b(minor));
    b.extend_from_slice(&u32b(completed as u32));
    b
}

pub(crate) fn encode_reply_with_context(
    version: Version,
    endianness: Endianness,
    request_id: u32,
    status: ReplyStatusType,
    body: Vec<u8>,
    service_context: ServiceContextList,
) -> io::Result<Vec<u8>> {
    let reply = zerodds_corba_giop::Reply {
        request_id,
        reply_status: status,
        service_context,
        body,
    };
    encode_message(version, endianness, false, &Message::Reply(reply))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("encode reply: {e:?}")))
}

/// The reply's service-context list carrying the SAS message this
/// request's CSIv2 evaluation produced, if any (§7.2: attached to every
/// reply the request gets, success or refusal, once a SAS decision was
/// made for it).
pub(crate) fn sas_service_context(
    sas_reply: Option<&zerodds_corba_csiv2::sas::SasMessage>,
    endianness: Endianness,
) -> ServiceContextList {
    let mut list = ServiceContextList::default();
    if let Some(m) = sas_reply
        && let Some(sc) = crate::csiv2_wire::service_context_for(m, endianness)
    {
        list.0.push(sc);
    }
    list
}

/// Writes a GIOP message, split into fragments above `fragment_size`
/// (GIOP §15.4.9: the first message with the fragment bit, then Fragment
/// messages; in GIOP 1.2 each carries the request id, in GIOP 1.1 none
/// does). GIOP 1.0 has no fragments; its messages go out whole.
pub(crate) fn write_giop(
    conn: &ConnWriter,
    message: &[u8],
    fragment_size: usize,
    request_id: u32,
    endianness: Endianness,
) -> io::Result<()> {
    let mut w = conn
        .lock()
        .map_err(|_| io::Error::other("connection writer poisoned"))?;
    for frame in fragments(message, fragment_size, request_id, endianness) {
        w.write_all(&frame)?;
    }
    w.flush()
}

/// The frames [`write_giop`] writes for `message`.
///
/// Every fragment but the last ends on an 8-octet boundary of the message,
/// header included, so no primitive of 8 octets or less is split (GIOP
/// §15.4.9). The first frame carries 12 header octets, so its data is 4
/// short of a multiple of 8; each later frame's data is a multiple of 8,
/// after a 16-octet header with the request id in GIOP 1.2 and a 12-octet
/// header in GIOP 1.1.
pub(crate) fn fragments(
    message: &[u8],
    fragment_size: usize,
    request_id: u32,
    endianness: Endianness,
) -> Vec<Vec<u8>> {
    let minor = match message.get(4..6) {
        Some([1, m]) if *m >= 1 => *m,
        _ => 0,
    };
    if message.len() <= fragment_size.max(64) + 12 || minor == 0 {
        return vec![message.to_vec()];
    }
    let body = &message[12..];
    let u32b = |v: u32| match endianness {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    };
    let flags = |more: bool| {
        u8::from(matches!(endianness, Endianness::Little)) | if more { 0x02 } else { 0 }
    };
    let step = (fragment_size.max(64) / 8) * 8;
    let first_len = step - 4;
    let (head, mut rest) = body.split_at(first_len.min(body.len()));
    let mut out = Vec::new();
    let mut frame = Vec::with_capacity(head.len() + 12);
    frame.extend_from_slice(b"GIOP");
    frame.extend_from_slice(&[1, minor]);
    frame.push(flags(!rest.is_empty()));
    frame.push(message[7]);
    frame.extend_from_slice(&u32b(u32::try_from(head.len()).unwrap_or(u32::MAX)));
    frame.extend_from_slice(head);
    out.push(frame);
    while !rest.is_empty() {
        let (chunk, tail) = rest.split_at(step.min(rest.len()));
        rest = tail;
        let with_id = minor >= 2;
        let mut frame = Vec::with_capacity(chunk.len() + 16);
        frame.extend_from_slice(b"GIOP");
        frame.extend_from_slice(&[1, minor]);
        frame.push(flags(!rest.is_empty()));
        frame.push(7); // Fragment
        let size = chunk.len() + if with_id { 4 } else { 0 };
        frame.extend_from_slice(&u32b(u32::try_from(size).unwrap_or(u32::MAX)));
        if with_id {
            frame.extend_from_slice(&u32b(request_id));
        }
        frame.extend_from_slice(chunk);
        out.push(frame);
    }
    out
}

/// Reassembles fragmented GIOP 1.1/1.2 messages (§4.6): the first message
/// carries the fragment bit, Fragment messages (type 7) follow until one
/// without it.
#[derive(Default)]
pub(crate) struct Reassembly {
    pub(crate) first: Option<Vec<u8>>,
}

impl Reassembly {
    /// Feeds one raw message; returns a whole message when complete.
    pub(crate) fn feed(&mut self, raw: Vec<u8>) -> io::Result<Option<Vec<u8>>> {
        let more = raw[6] & 0x02 != 0;
        let is_fragment = raw[7] == 7;
        let le = raw[6] & 0x01 != 0;
        match (&mut self.first, is_fragment) {
            (None, false) if !more => Ok(Some(raw)),
            (None, false) => {
                self.first = Some(raw);
                Ok(None)
            }
            (None, true) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a Fragment without a fragmented message",
            )),
            (Some(_), false) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a new message inside a fragmented one",
            )),
            (Some(first), true) => {
                // GIOP 1.2 fragments start with the request id.
                let skip = if raw[4..6] == [1, 2] { 16 } else { 12 };
                if raw.len() < skip {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "a short Fragment",
                    ));
                }
                first.extend_from_slice(&raw[skip..]);
                if more {
                    return Ok(None);
                }
                let mut whole = self.first.take().unwrap_or_default();
                whole[6] &= !0x02;
                let size = u32::try_from(whole.len() - 12).unwrap_or(u32::MAX);
                let b = if le {
                    size.to_le_bytes()
                } else {
                    size.to_be_bytes()
                };
                whole[8..12].copy_from_slice(&b);
                Ok(Some(whole))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::bridge_daemon::config::tests::load;

    #[test]
    fn corbabridge_4_3_a_system_exception_body_is_the_giop_layout() {
        let b = system_exception_body("TIMEOUT", 7, Completion::Maybe, Endianness::Little);
        let id = b"IDL:omg.org/CORBA/TIMEOUT:1.0";
        assert_eq!(
            u32::from_le_bytes(b[..4].try_into().unwrap()) as usize,
            id.len() + 1
        );
        assert_eq!(&b[4..4 + id.len()], id);
        let off = (4 + id.len() + 1).div_ceil(4) * 4;
        assert_eq!(u32::from_le_bytes(b[off..off + 4].try_into().unwrap()), 7);
        assert_eq!(
            u32::from_le_bytes(b[off + 4..off + 8].try_into().unwrap()),
            2
        );
    }

    /// Every fragment but the last ends on an 8-octet boundary of the
    /// message stream, header included (GIOP 1.2 §15.4.9), and the pieces
    /// put back together are the message.
    #[test]
    fn corbabridge_4_6_outbound_fragments_end_on_8_octet_boundaries() {
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl Write for Sink {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let body: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let msg = encode_reply_with_context(
            Version::V1_2,
            Endianness::Little,
            9,
            ReplyStatusType::NoException,
            body,
            ServiceContextList::default(),
        )
        .unwrap();
        let out = Arc::new(Mutex::new(Vec::new()));
        let conn: ConnWriter = Arc::new(Mutex::new(Box::new(Sink(Arc::clone(&out)))));
        write_giop(&conn, &msg, 128, 9, Endianness::Little).unwrap();
        let bytes = out.lock().unwrap().clone();
        let mut r = Reassembly::default();
        let (mut at, mut whole) = (0, None);
        while at < bytes.len() {
            let size = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize;
            let frame = bytes[at..at + 12 + size].to_vec();
            let more = frame[6] & 0x02 != 0;
            if more {
                assert_eq!(
                    frame.len() % 8,
                    0,
                    "a fragment ending off an 8-octet boundary"
                );
            }
            at += frame.len();
            whole = r.feed(frame).unwrap();
        }
        assert_eq!(whole.unwrap(), msg);
    }

    /// GIOP 1.1 fragments a reply too: its Fragment messages carry no
    /// request id, every cut lies on an 8-octet boundary of the message, and
    /// the pieces put back together are the message. GIOP 1.0 has no
    /// fragments.
    #[test]
    fn corbabridge_4_6_a_giop_1_1_reply_is_fragmented_without_request_ids() {
        let body: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let msg = encode_reply_with_context(
            Version::V1_1,
            Endianness::Big,
            9,
            ReplyStatusType::NoException,
            body.clone(),
            ServiceContextList::default(),
        )
        .unwrap();
        let frames = fragments(&msg, 128, 9, Endianness::Big);
        assert!(frames.len() > 1);
        let mut at_in_message = 12;
        let mut r = Reassembly::default();
        let mut whole = None;
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(&frame[4..6], &[1, 1]);
            let size = u32::from_be_bytes(frame[8..12].try_into().unwrap()) as usize;
            assert_eq!(frame.len(), 12 + size, "no FragmentHeader in GIOP 1.1");
            if i > 0 {
                assert_eq!(frame[7], 7, "a Fragment");
            }
            at_in_message += size;
            if frame[6] & 0x02 != 0 {
                assert_eq!(at_in_message % 8, 0, "a cut off an 8-octet boundary");
            }
            whole = r.feed(frame.clone()).unwrap();
        }
        assert_eq!(whole.unwrap(), msg);
        let old = encode_reply_with_context(
            Version::V1_0,
            Endianness::Big,
            9,
            ReplyStatusType::NoException,
            body,
            ServiceContextList::default(),
        )
        .unwrap();
        assert_eq!(fragments(&old, 128, 9, Endianness::Big), vec![old]);
    }

    /// A message within `fragment_size` goes out as it is.
    #[test]
    fn corbabridge_4_6_a_message_within_the_fragment_size_goes_out_whole() {
        let msg = encode_reply_with_context(
            Version::V1_2,
            Endianness::Little,
            1,
            ReplyStatusType::NoException,
            vec![7; 100],
            ServiceContextList::default(),
        )
        .unwrap();
        assert_eq!(fragments(&msg, 4096, 1, Endianness::Little), vec![msg]);
    }

    /// A Fragment without the message it continues, and a new message
    /// inside a fragmented one, are refused.
    #[test]
    fn corbabridge_4_6_a_fragment_out_of_place_is_refused() {
        let mut frag = b"GIOP\x01\x02\x01\x07".to_vec();
        frag.extend_from_slice(&4u32.to_le_bytes());
        frag.extend_from_slice(&9u32.to_le_bytes());
        assert!(Reassembly::default().feed(frag).is_err());
        let mut first = b"GIOP\x01\x02\x03\x00".to_vec();
        first.extend_from_slice(&8u32.to_le_bytes());
        first.extend_from_slice(&[0; 8]);
        let mut r = Reassembly::default();
        assert!(r.feed(first.clone()).unwrap().is_none());
        assert!(r.feed(first).is_err());
    }

    #[test]
    fn corbabridge_4_6_a_fragmented_message_is_put_back_together() {
        // GIOP 1.2 LE Request split into a first message and one Fragment.
        let body: Vec<u8> = (0..40u8).collect();
        let mut first = b"GIOP\x01\x02\x03\x00".to_vec();
        first.extend_from_slice(&24u32.to_le_bytes());
        first.extend_from_slice(&body[..24]);
        let mut frag = b"GIOP\x01\x02\x01\x07".to_vec();
        frag.extend_from_slice(&(16u32 + 4).to_le_bytes());
        frag.extend_from_slice(&9u32.to_le_bytes());
        frag.extend_from_slice(&body[24..]);
        let mut r = Reassembly::default();
        assert!(r.feed(first).unwrap().is_none());
        let whole = r.feed(frag).unwrap().unwrap();
        assert_eq!(whole[6] & 0x02, 0, "fragment bit cleared");
        assert_eq!(u32::from_le_bytes(whole[8..12].try_into().unwrap()), 40);
        assert_eq!(&whole[12..], &body[..]);
    }
}
