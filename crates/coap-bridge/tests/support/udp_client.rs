// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The UDP client side of the daemon e2e tests (`#[path]` include).

use std::net::UdpSocket;
use std::time::Duration;

use zerodds_coap_bridge::codec::decode;
use zerodds_coap_bridge::message::CoapMessage;

/// A client socket on a port of its own, reads time out after 2 s.
pub fn client_socket() -> UdpSocket {
    let s = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    s.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    s
}

/// The next datagram on `socket`, decoded.
pub fn recv_msg(socket: &UdpSocket) -> CoapMessage {
    let mut buf = [0u8; 65535];
    let (n, _peer) = socket.recv_from(&mut buf).expect("recv");
    decode(&buf[..n]).expect("decode")
}
