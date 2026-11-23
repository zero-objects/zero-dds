// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `StreamLengthLeg`: a transport leg whose messages state their own
//! length.
//!
//! DDSI-RTPS 2.5 §8.3.3: "a stream-oriented transport (like TCP) would
//! need to include the length in the RTPS HeaderExtension in order to
//! identify the boundary of the RTPS message". The TCP transport in its
//! dialect `rtps` (`zerodds-tcp-transport-1.0` §2.5) delimits messages so
//! and adds nothing to a message itself: it takes only one that carries
//! the messageLength. This leg is where the runtime puts it in. A
//! message sent through it gets a HeaderExtension with the messageLength
//! directly behind the RTPS header, or the field in the HeaderExtension
//! it has (`HeaderExtension::with_message_length`), and then goes to the
//! transport. Everything else is the transport's.
//!
//! In front of a TCP transport
//! ([`StreamLengthLeg::for_tcp`](crate::stream_length_leg::StreamLengthLeg::for_tcp))
//! the leg decides per destination: a connection is used in both directions, and
//! one the transport accepted has the dialect it was found in (§4.1,
//! §4.2), whatever the runtime's own. The length goes into a message
//! whose destination the transport writes to in the dialect `rtps`
//! (`TcpTransport::dialect_towards`); a message to any other destination
//! goes to the transport as it is.
//!
//! The message is valid over a datagram transport as well: a receiver
//! holds the messageLength against the length it measured (§8.3.7.5).

use alloc::sync::Arc;
use alloc::vec::Vec;

use zerodds_rtps::header_extension::HeaderExtension;
use zerodds_rtps::wire_types::Locator;
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};
use zerodds_transport_tcp::{TcpDialect, TcpTransport};

/// The octets a HeaderExtension with only the messageLength adds to a
/// message: its submessage header (4) and the field (4).
const LENGTH_EXTENSION_SIZE: usize = 8;

/// A leg that states the length of each message it sends in the message's
/// HeaderExtension. See the module docs.
pub struct StreamLengthLeg {
    transport: Arc<dyn Transport + Send + Sync>,
    /// The TCP transport that tells the dialect per destination; `None`
    /// states the length of every message.
    tcp: Option<Arc<TcpTransport>>,
}

impl StreamLengthLeg {
    /// The leg in front of `transport`: every message gets its length.
    #[must_use]
    pub fn new(transport: Arc<dyn Transport + Send + Sync>) -> Self {
        Self {
            transport,
            tcp: None,
        }
    }

    /// The leg in front of the TCP transport `tcp`: a message gets its
    /// length when `tcp` writes to its destination in the dialect `rtps`
    /// ([`TcpTransport::dialect_towards`]), and goes as it is otherwise.
    #[must_use]
    pub fn for_tcp(tcp: Arc<TcpTransport>) -> Self {
        let transport: Arc<dyn Transport + Send + Sync> = tcp.clone();
        Self {
            transport,
            tcp: Some(tcp),
        }
    }

    /// Whether a message to `dest` has to state its length.
    fn states_length_towards(&self, dest: &Locator) -> bool {
        self.tcp
            .as_ref()
            .is_none_or(|tcp| tcp.dialect_towards(dest) == TcpDialect::Rtps)
    }

    /// `data` as it goes to `dest`: with its length stated where the
    /// destination wants it ([`Self::stated`]), as it is otherwise.
    fn for_destination<'a>(
        &self,
        dest: &Locator,
        data: &'a [u8],
    ) -> Result<alloc::borrow::Cow<'a, [u8]>, SendError> {
        if self.states_length_towards(dest) {
            Self::stated(data).map(alloc::borrow::Cow::Owned)
        } else {
            Ok(alloc::borrow::Cow::Borrowed(data))
        }
    }

    /// `data` with its length stated; an error for what is no RTPS 2.5
    /// message, which cannot carry the field and is not sent, and for a
    /// protected message whose tag covers a HeaderExtension it does not
    /// have ([`authenticated_without_extension`]).
    fn stated(data: &[u8]) -> Result<Vec<u8>, SendError> {
        if authenticated_without_extension(data) {
            return Err(SendError::Io {
                message: "message with an authenticated header has no HeaderExtension for its length",
            });
        }
        HeaderExtension::with_message_length(data).map_err(|_| SendError::Io {
            message: "message cannot carry a HeaderExtension messageLength",
        })
    }
}

/// Whether `data` is an RTPS message protected as a whole whose tag covers
/// the RTPS Header and the HeaderExtension (the SRTPS_PREFIX, submessage
/// id `0x33`, directly behind the header with its flag A, `0x02`;
/// DDS-Security 1.2 §7.4.6.6, §10.5.3.3.1 Table 80, 2) and that has no
/// HeaderExtension. One added now would not be the one the sender
/// authenticated, and the receiver's check of the tag would fail: such a
/// message gets its HeaderExtension before it is protected, and the leg
/// refuses one that did not.
fn authenticated_without_extension(data: &[u8]) -> bool {
    const HEADER: usize = 20;
    const SRTPS_PREFIX: u8 = 0x33;
    const FLAG_A: u8 = 0x02;
    data.get(HEADER) == Some(&SRTPS_PREFIX)
        && data
            .get(HEADER + 1)
            .is_some_and(|flags| flags & FLAG_A != 0)
}

impl core::fmt::Debug for StreamLengthLeg {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StreamLengthLeg").finish_non_exhaustive()
    }
}

impl Transport for StreamLengthLeg {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        self.transport
            .send(dest, &self.for_destination(dest, data)?)
    }

    fn send_with_priority(
        &self,
        dest: &Locator,
        data: &[u8],
        priority: i32,
    ) -> Result<(), SendError> {
        self.transport
            .send_with_priority(dest, &self.for_destination(dest, data)?, priority)
    }

    /// What the transport carries, less the HeaderExtension this leg
    /// adds. In front of a TCP transport: what its own dialect carries,
    /// and at most what a connection of the dialect `rtps` carries less
    /// the HeaderExtension, for a message to an accepted connection of
    /// that dialect.
    fn max_message_size(&self) -> usize {
        let rtps = TcpDialect::Rtps
            .max_message_size()
            .saturating_sub(LENGTH_EXTENSION_SIZE);
        match &self.tcp {
            None => self
                .transport
                .max_message_size()
                .saturating_sub(LENGTH_EXTENSION_SIZE),
            Some(tcp) => tcp.dialect().max_message_size().min(rtps),
        }
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        self.transport.recv()
    }

    fn recv_batch(&self, out: &mut Vec<ReceivedDatagram>, max: usize) -> Result<usize, RecvError> {
        self.transport.recv_batch(out, max)
    }

    fn local_locator(&self) -> Locator {
        self.transport.local_locator()
    }

    fn local_locators(&self) -> Vec<Locator> {
        self.transport.local_locators()
    }

    fn add_destination(&self, dest: &Locator) {
        self.transport.add_destination(dest);
    }

    fn forget_destination(&self, dest: &Locator) {
        self.transport.forget_destination(dest);
    }

    fn close(&self) {
        self.transport.close();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A transport that keeps what it is given.
    #[derive(Default)]
    struct Recording {
        sent: Mutex<Vec<(Locator, Vec<u8>, i32)>>,
        added: Mutex<Vec<Locator>>,
        forgotten: Mutex<Vec<Locator>>,
        closed: Mutex<bool>,
    }

    impl Transport for Recording {
        fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
            self.sent.lock().unwrap().push((*dest, data.to_vec(), 0));
            Ok(())
        }
        fn send_with_priority(
            &self,
            dest: &Locator,
            data: &[u8],
            priority: i32,
        ) -> Result<(), SendError> {
            self.sent
                .lock()
                .unwrap()
                .push((*dest, data.to_vec(), priority));
            Ok(())
        }
        fn max_message_size(&self) -> usize {
            1000
        }
        fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
            Err(RecvError::Timeout)
        }
        fn local_locator(&self) -> Locator {
            Locator::tcp_v4([127, 0, 0, 1], 7410)
        }
        fn add_destination(&self, dest: &Locator) {
            self.added.lock().unwrap().push(*dest);
        }
        fn forget_destination(&self, dest: &Locator) {
            self.forgotten.lock().unwrap().push(*dest);
        }
        fn close(&self) {
            *self.closed.lock().unwrap() = true;
        }
    }

    /// An RTPS 2.5 header and an INFO_TS, 32 octets.
    fn plain_message() -> Vec<u8> {
        let mut m = vec![b'R', b'T', b'P', b'S', 2, 5, 0x01, 0xF0];
        m.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        m.extend_from_slice(&[0x09, 0x01, 8, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
        m
    }

    /// zerodds-tcp-transport-1.0 §2.5: a message sent through the leg
    /// reaches the transport with a HeaderExtension directly behind the
    /// RTPS header whose messageLength is the length of what the
    /// transport gets (DDSI-RTPS 2.5 §9.4.5.2: id 0x00, flags E|L, 4
    /// octets, the length), the submessages behind it unchanged; with and
    /// without a priority.
    #[test]
    fn tcp_2_5_a_message_sent_through_the_leg_states_its_length() {
        let recording = Arc::new(Recording::default());
        let leg = StreamLengthLeg::new(recording.clone());
        let dest = Locator::tcp_v4([127, 0, 0, 1], 7411);
        let plain = plain_message();
        leg.send(&dest, &plain).unwrap();
        leg.send_with_priority(&dest, &plain, 5).unwrap();
        let sent = recording.sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        for ((to, data, _), priority) in sent.iter().zip([0, 5]) {
            assert_eq!(*to, dest);
            assert_eq!(data.len(), 40);
            assert_eq!(data[..20], plain[..20]);
            assert_eq!(data[20..28], [0x00, 0x03, 4, 0, 40, 0, 0, 0]);
            assert_eq!(data[28..], plain[20..]);
            let _ = priority;
        }
        assert_eq!(sent[0].2, 0);
        assert_eq!(sent[1].2, 5);
    }

    /// What is no RTPS 2.5 message cannot state its length: the send
    /// fails and the transport gets nothing.
    #[test]
    fn tcp_2_5_the_leg_refuses_what_cannot_carry_the_length() {
        let recording = Arc::new(Recording::default());
        let leg = StreamLengthLeg::new(recording.clone());
        let dest = Locator::tcp_v4([127, 0, 0, 1], 7411);
        let mut old = plain_message();
        old[5] = 4;
        for bad in [&b"abc"[..], &old[..], &plain_message()[..19]] {
            assert_eq!(
                leg.send(&dest, bad),
                Err(SendError::Io {
                    message: "message cannot carry a HeaderExtension messageLength"
                })
            );
        }
        assert!(recording.sent.lock().unwrap().is_empty());
    }

    /// zerodds-tcp-transport-1.0 §2.5 with DDS-Security 1.2 §10.5.3.3.1
    /// Table 80: a protected message stands behind the HeaderExtension the
    /// leg adds when its tag does not cover the header (SRTPS_PREFIX
    /// without the flag A): `RTPS Header | HeaderExtension | SRTPS_PREFIX`.
    /// One whose tag covers the header and the HeaderExtension (flag A)
    /// and that has none is refused, and the transport gets nothing: a
    /// HeaderExtension added now is not the one under the tag. With its
    /// HeaderExtension in place, put there before the protection, it goes
    /// out, and octets other than the messageLength stay as they are.
    #[test]
    fn tcp_2_5_the_leg_and_a_protected_message() {
        let recording = Arc::new(Recording::default());
        let leg = StreamLengthLeg::new(recording.clone());
        let dest = Locator::tcp_v4([127, 0, 0, 1], 7411);
        let header = plain_message()[..20].to_vec();
        // SRTPS_PREFIX (0x33) with a CryptoHeader of 20 octets, then four
        // octets that stand for the rest.
        let protected = |flags: u8| {
            let mut m = vec![0x33, flags, 20, 0];
            m.extend_from_slice(&[0xC5; 20]);
            m.extend_from_slice(&[0x34, 0x01, 0, 0]);
            m
        };

        // Without the flag A: the HeaderExtension goes in front.
        let mut unauthenticated = header.clone();
        unauthenticated.extend_from_slice(&protected(0x01));
        leg.send(&dest, &unauthenticated).unwrap();
        {
            let sent = recording.sent.lock().unwrap();
            assert_eq!(sent.len(), 1);
            let data = &sent[0].1;
            assert_eq!(data[..20], header[..]);
            assert_eq!(data[20..28], [0x00, 0x03, 4, 0, 56, 0, 0, 0]);
            assert_eq!(data[28..], unauthenticated[20..]);
        }

        // With the flag A and no HeaderExtension: refused, with and
        // without the flag P.
        for flags in [0x03, 0x07] {
            let mut authenticated = header.clone();
            authenticated.extend_from_slice(&protected(flags));
            let refused = Err(SendError::Io {
                message: "message with an authenticated header has no HeaderExtension for its length",
            });
            assert_eq!(leg.send(&dest, &authenticated), refused);
            assert_eq!(leg.send_with_priority(&dest, &authenticated, 1), refused);
        }
        assert_eq!(recording.sent.lock().unwrap().len(), 1);

        // With the flag A behind its HeaderExtension: sent, the length
        // set, everything else as it was.
        let mut in_place = header.clone();
        in_place.extend_from_slice(&[0x00, 0x03, 4, 0, 0, 0, 0, 0]);
        in_place.extend_from_slice(&protected(0x07));
        leg.send(&dest, &in_place).unwrap();
        let sent = recording.sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        let data = &sent[1].1;
        assert_eq!(data.len(), in_place.len());
        assert_eq!(data[..24], in_place[..24]);
        assert_eq!(data[24..28], 56u32.to_le_bytes());
        assert_eq!(data[28..], in_place[28..]);
    }

    /// Everything but the message is the transport's: the size it
    /// carries less the eight octets the leg adds, its locators, its
    /// destinations, its close, its receive.
    #[test]
    fn tcp_2_5_the_leg_hands_everything_else_to_the_transport() {
        let recording = Arc::new(Recording::default());
        let leg = StreamLengthLeg::new(recording.clone());
        assert_eq!(leg.max_message_size(), 992);
        assert_eq!(leg.local_locator(), Locator::tcp_v4([127, 0, 0, 1], 7410));
        assert_eq!(
            leg.local_locators(),
            vec![Locator::tcp_v4([127, 0, 0, 1], 7410)]
        );
        assert_eq!(leg.recv(), Err(RecvError::Timeout));
        let mut out = Vec::new();
        assert_eq!(leg.recv_batch(&mut out, 4), Err(RecvError::Timeout));
        let dest = Locator::tcp_v4([127, 0, 0, 1], 7411);
        leg.add_destination(&dest);
        leg.forget_destination(&dest);
        leg.close();
        assert_eq!(*recording.added.lock().unwrap(), vec![dest]);
        assert_eq!(*recording.forgotten.lock().unwrap(), vec![dest]);
        assert!(*recording.closed.lock().unwrap());
    }
}
