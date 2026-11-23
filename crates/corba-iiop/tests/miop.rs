// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MIOP over real multicast sockets (CORBA 3.3 Part 2 §11.1.5, §11.1.7):
//! a sender's packet collection reaches every receiver that joined the
//! group, IPv4 and IPv6, and only those.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

use zerodds_corba_iiop::miop::{
    MiopReceiver, MiopSender, MulticastInterface, Received, join_group,
};

/// A port for one test: the process id spreads parallel runs.
fn port(offset: u16) -> u16 {
    20_000 + (std::process::id() % 20_000) as u16 + offset
}

fn receive_one(r: &mut MiopReceiver) -> (Vec<u8>, SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match r.receive(Duration::from_millis(100)).unwrap() {
            Received::Collection(bytes, from) => return (bytes, from),
            Received::Refused(e, _) => panic!("refused: {e}"),
            Received::Idle => {}
        }
    }
    panic!("no collection within 5 s");
}

fn message(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 249) as u8).collect()
}

#[test]
fn corba_p2_11_1_5_every_receiver_of_the_ipv4_group_gets_the_collection() {
    let group = IpAddr::V4(Ipv4Addr::new(239, 255, 41, 7));
    let p = port(1);
    let timeout = Duration::from_secs(2);
    let mut a = MiopReceiver::join(group, p, MulticastInterface::Default, timeout).unwrap();
    let mut b = MiopReceiver::join(group, p, MulticastInterface::Default, timeout).unwrap();
    let sender = MiopSender::new(group, 1).unwrap().with_max_datagram(1000);
    let msg = message(10_000);
    let packets = sender.send(SocketAddr::new(group, p), &msg).unwrap();
    assert!(packets > 10);
    assert_eq!(receive_one(&mut a).0, msg);
    assert_eq!(receive_one(&mut b).0, msg);
    assert_eq!(a.stats().delivered, 1);
    assert_eq!(
        a.stats().malformed + a.stats().incomplete + a.stats().timed_out,
        0
    );
}

#[test]
fn corba_p2_11_1_5_a_receiver_of_another_group_on_the_same_port_gets_nothing() {
    let group = IpAddr::V4(Ipv4Addr::new(239, 255, 41, 8));
    let other = IpAddr::V4(Ipv4Addr::new(239, 255, 41, 9));
    let p = port(2);
    let mut joined = MiopReceiver::join(
        group,
        p,
        MulticastInterface::Default,
        Duration::from_secs(2),
    )
    .unwrap();
    let mut elsewhere = MiopReceiver::join(
        other,
        p,
        MulticastInterface::Default,
        Duration::from_secs(2),
    )
    .unwrap();
    let sender = MiopSender::new(group, 1).unwrap();
    sender
        .send(SocketAddr::new(group, p), &message(100))
        .unwrap();
    assert_eq!(receive_one(&mut joined).0, message(100));
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        assert!(matches!(
            elsewhere.receive(Duration::from_millis(100)).unwrap(),
            Received::Idle
        ));
    }
}

#[test]
fn corba_p2_11_1_5_the_ipv6_group_gets_the_collection() {
    let group = IpAddr::V6(Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0x4d49, 0x4f50));
    let p = port(3);
    let mut r = MiopReceiver::join(
        group,
        p,
        MulticastInterface::Default,
        Duration::from_secs(2),
    )
    .unwrap();
    let sender = MiopSender::new(group, 1).unwrap().with_max_datagram(1200);
    let msg = message(5000);
    sender.send(SocketAddr::new(group, p), &msg).unwrap();
    assert_eq!(receive_one(&mut r).0, msg);
}

#[test]
fn corba_p2_11_1_5_joining_needs_a_multicast_address_and_a_matching_interface() {
    assert!(
        join_group(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            port(4),
            MulticastInterface::Default
        )
        .is_err()
    );
    assert!(
        join_group(
            IpAddr::V4(Ipv4Addr::new(239, 255, 41, 10)),
            port(4),
            MulticastInterface::V6Index(1)
        )
        .is_err()
    );
}

#[test]
fn corba_p2_11_1_6_2_a_foreign_datagram_on_the_group_port_is_counted() {
    let group = IpAddr::V4(Ipv4Addr::new(239, 255, 41, 11));
    let p = port(5);
    let mut r = MiopReceiver::join(
        group,
        p,
        MulticastInterface::Default,
        Duration::from_secs(2),
    )
    .unwrap();
    let raw = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
    raw.send_to(b"GIOP and not MIOP at all", SocketAddr::new(group, p))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "nothing received");
        match r.receive(Duration::from_millis(100)).unwrap() {
            Received::Refused(e, _) => {
                assert!(e.to_string().contains("InvalidMagic"), "{e}");
                break;
            }
            Received::Collection(..) => panic!("a foreign datagram was delivered"),
            Received::Idle => {}
        }
    }
    assert_eq!(r.stats().malformed, 1);
}
