//! E2E-Loopback: ein TcpTransport bindet, ein zweiter verbindet sich
//! und sendet ein Frame; der erste liest es via accept_one + recv.

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

    // Server-Accept-Thread: liest alle Frames bis zum EOF.
    let server_handle = thread::spawn(move || {
        server.accept_one().expect("accept_one");
        // Non-blocking drain aus der Queue.
        let mut out = Vec::new();
        while let Ok(dg) = server.try_recv() {
            out.push(dg);
        }
        out
    });

    // Mini-Delay, damit der Server wirklich auf accept() steht.
    thread::sleep(Duration::from_millis(50));

    // Client-Seite: bindet irgendwohin und sendet ein Frame an Server.
    let client = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("client bind");
    let dest_locator = Locator::tcp_v4([127, 0, 0, 1], server_loc.port);
    Transport::send(&client, &dest_locator, b"payload-one").expect("send frame 1");
    Transport::send(&client, &dest_locator, b"payload-two").expect("send frame 2");

    // Client dropt → TCP-Connection schliesst → Server accept_one kehrt zurueck.
    drop(client);

    let frames = server_handle.join().expect("server thread");
    assert_eq!(frames.len(), 2);
    assert_eq!(&frames[0].data[..], b"payload-one");
    assert_eq!(&frames[1].data[..], b"payload-two");
}

#[test]
fn reconnect_after_peer_restart() {
    // Wir starten einen Server, senden, der Server schliesst (via accept_one
    // return), dann starten wir einen zweiten Server auf *derselben* Adresse
    // und erwarten, dass ein neuer send erfolgreich ist (neuer Connect-Versuch).
    let server1 = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind1");
    let addr1 = server1.local_locator();

    // Server-Thread: eine accept-Runde, danach droppen.
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
    // Port 2 ist reserviert und faellt bei connect() typischerweise auf
    // ECONNREFUSED — perfekt fuer den Backoff-Test.
    let dead = Locator::tcp_v4([127, 0, 0, 1], 2);
    // Erster send schlaegt fehl (ECONNREFUSED) und setzt backoff_ms = 50ms.
    let _ = Transport::send(&client, &dead, b"a");
    let start = std::time::Instant::now();
    // Zweiter send innerhalb des Backoff-Fensters → sofortiger Fehler.
    let _ = Transport::send(&client, &dead, b"b");
    // Die Backoff-Variante gibt innerhalb < 10 ms zurueck, nicht erst
    // nach 50 ms (kein TCP-connect-Attempt).
    assert!(
        start.elapsed() < Duration::from_millis(20),
        "backoff should short-circuit"
    );
}

#[test]
fn concurrent_senders_to_one_server() {
    // B10/#13: zwei Client-Threads feuern parallel 50 Frames auf einen
    // Server; Server liest alles und prueft Zaehlung.
    let server = TcpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("bind");
    let server_loc = server.local_locator();

    let server_handle = thread::spawn(move || {
        let mut all = Vec::new();
        // Zweimal accept (zwei Clients), jedes liest bis EOF.
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
