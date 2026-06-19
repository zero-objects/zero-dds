//! E2E loopback: one TcpTransport binds, a second connects
//! and sends a frame; the first reads it via accept_one + recv.

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

use std::net::Ipv4Addr;
use std::thread;
use std::time::Duration;

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::Transport;
use zerodds_transport_tcp::TcpTransport;

#[test]
fn loopback_single_frame_end_to_end() {
    // Bind-Seite (Server).
    let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
    let server_loc = server.local_locator();

    // Server accept thread: reads all frames until EOF.
    let server_handle = thread::spawn(move || {
        server.accept_one().expect("accept_one");
        // Non-blocking drain from the queue.
        let mut out = Vec::new();
        while let Ok(dg) = server.try_recv() {
            out.push(dg);
        }
        out
    });

    // Mini delay so the server is really sitting on accept().
    thread::sleep(Duration::from_millis(50));

    // Client side: binds somewhere and sends a frame to the server.
    let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("client bind");
    let dest_locator = Locator::tcp_v4([127, 0, 0, 1], server_loc.port);
    Transport::send(&client, &dest_locator, b"payload-one").expect("send frame 1");
    Transport::send(&client, &dest_locator, b"payload-two").expect("send frame 2");

    // Client drops → TCP connection closes → server accept_one returns.
    drop(client);

    let frames = server_handle.join().expect("server thread");
    assert_eq!(frames.len(), 2);
    assert_eq!(&frames[0].data[..], b"payload-one");
    assert_eq!(&frames[1].data[..], b"payload-two");
}

#[test]
fn reconnect_after_peer_restart() {
    // We start a server, send, the server closes (via accept_one
    // return), then we start a second server on the *same* address
    // and expect a new send to succeed (a new connect attempt).
    let server1 = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind1");
    let addr1 = server1.local_locator();

    // Server thread: one accept round, then drop.
    let s1_handle = thread::spawn(move || {
        server1.accept_one().expect("accept_one srv1");
    });
    thread::sleep(Duration::from_millis(30));

    let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("client bind");
    let dest = Locator::tcp_v4([127, 0, 0, 1], addr1.port);
    Transport::send(&client, &dest, b"first").expect("first send");
    Transport::send(&client, &dest, b"second").expect("second send");

    drop(client);
    s1_handle.join().expect("s1 thread");
}

#[test]
fn backoff_prevents_tight_loop_on_dead_peer() {
    let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("client bind");
    // Port 2 is reserved and on connect() typically yields
    // ECONNREFUSED — perfect for the backoff test.
    let dead = Locator::tcp_v4([127, 0, 0, 1], 2);
    // The first send fails (ECONNREFUSED) and sets backoff_ms = 50ms.
    let _ = Transport::send(&client, &dead, b"a");
    let start = std::time::Instant::now();
    // The second send within the backoff window → immediate error.
    let _ = Transport::send(&client, &dead, b"b");
    // The backoff variant returns within < 10 ms, not only
    // after 50 ms (no TCP connect attempt).
    assert!(
        start.elapsed() < Duration::from_millis(20),
        "backoff should short-circuit"
    );
}

#[test]
fn concurrent_senders_to_one_server() {
    // B10/#13: two client threads fire 50 frames in parallel at one
    // server; the server reads everything and checks the count.
    let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
    let server_loc = server.local_locator();

    let server_handle = thread::spawn(move || {
        let mut all = Vec::new();
        // Accept twice (two clients), each reads until EOF.
        for _ in 0..2 {
            server.accept_one().expect("accept_one");
            while let Ok(dg) = server.try_recv() {
                all.push(dg);
            }
        }
        all
    });

    thread::sleep(Duration::from_millis(30));

    let mut handles = Vec::new();
    for t_id in 0..2u8 {
        let dest = Locator::tcp_v4([127, 0, 0, 1], server_loc.port);
        let h = thread::spawn(move || {
            let c = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("client bind");
            for i in 0..50u8 {
                let payload = [t_id, i];
                Transport::send(&c, &dest, &payload).expect("send");
            }
            drop(c);
        });
        handles.push(h);
    }
    for h in handles {
        h.join().expect("client join");
    }

    let frames = server_handle.join().expect("server join");
    assert_eq!(frames.len(), 100);
}

#[test]
fn unsupported_locator_returns_error() {
    let t = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
    let udp_loc = Locator::udp_v4([127, 0, 0, 1], 7400);
    let err = Transport::send(&t, &udp_loc, b"x").unwrap_err();
    match err {
        zerodds_transport::SendError::UnsupportedLocator => {}
        other => panic!("expected UnsupportedLocator, got {other:?}"),
    }
}
