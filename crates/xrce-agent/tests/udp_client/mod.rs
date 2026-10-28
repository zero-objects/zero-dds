// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The client side the UDP tests of the agent share: the sample type they
//! write, a UDP `ClientTransport` and the wait that keeps the client ticking.
#![allow(dead_code)]

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use zerodds_xrce_client::{ClientError, ClientTransport, XrceClient};

/// `@final struct KeyedPoint { @key long id; sequence<octet> payload; };`
/// -- plain XCDR2 (no DHEADER, this type is `@final`): `id`(4 bytes LE) +
/// `payload.length`(4 bytes LE) + `payload` bytes, no trailing padding
/// (nothing follows within the sample).
pub fn encode_keyed_point(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// UDP `ClientTransport`: a connected, non-blocking socket. `try_recv`
/// treats `WouldBlock` as "nothing yet", per the trait's "does not
/// block" contract.
pub struct UdpTransport(pub UdpSocket);

impl ClientTransport for UdpTransport {
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
        self.0
            .send(payload)
            .map(|_| ())
            .map_err(|_| ClientError::Transport)
    }
    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        let mut buf = [0u8; 65_507];
        match self.0.recv(&mut buf) {
            Ok(n) => Ok(Some(buf[..n].to_vec())),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(None)
            }
            Err(_) => Err(ClientError::Transport),
        }
    }
}

/// Drives `step` until it returns something non-empty, or panics after
/// `budget`. Ticks `client` (§8.4.14.1 `HEARTBEAT` emission for any
/// reliable stream with unacknowledged in-flight messages) on every
/// iteration before calling `step` — without this, a datagram UDP
/// loopback occasionally drops (real packet loss, not a simulated one)
/// on a `reliable: true` request is never recovered, since the agent
/// never NACKs on its own (§8.4.14.2) and nothing else in the test
/// prompts a resend.
pub fn wait_until<T>(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    budget: Duration,
    mut step: impl FnMut(&mut XrceClient<UdpTransport>) -> Vec<T>,
    what: &str,
) -> Vec<T> {
    let wait_start = Instant::now();
    loop {
        client.tick(start.elapsed()).expect("tick");
        let got = step(client);
        if !got.is_empty() {
            return got;
        }
        if wait_start.elapsed() > budget {
            panic!("timed out after {budget:?} waiting for: {what}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
