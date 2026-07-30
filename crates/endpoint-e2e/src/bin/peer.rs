// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standalone ping-pong / reliable **peer** for the native-frontends examples.
//!
//! A language endpoint app (the star) runs as a separate process and talks to
//! this peer over UDP. The peer is the real ZeroDDS side: it decodes the app's
//! XRCE frames with the genuine `zerodds-cdr` / `zerodds-xrce` codecs.
//!
//! Usage:
//!   peer pingpong <port>                  # one Ping -> Pong exchange
//!   peer reliable <port> <drop_every> [n] # reliable receiver, injects loss,
//!                                          # asserts n gap-free (default 12)

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::expect_used)]

use std::time::Duration;

use zerodds_endpoint_e2e::{
    bind_peer_on, bind_reliable_peer_on, reliable_collect, serve_ping_pong,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: peer pingpong <port> | peer reliable <port> <drop_every> [n]";
    match args.get(1).map(String::as_str) {
        Some("pingpong") => {
            let port: u16 = args.get(2).and_then(|s| s.parse().ok()).expect(usage);
            let peer = bind_peer_on(port).expect("bind ping-pong peer");
            eprintln!("peer: ping-pong listening on 127.0.0.1:{port}");
            match serve_ping_pong(&peer, Duration::from_secs(30)) {
                Ok((seq, msg)) => {
                    println!("PEER OK: ping-pong (Ping seq={seq} msg={msg:?} -> Pong)");
                }
                Err(e) => {
                    eprintln!("peer: ping-pong failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("reliable") => {
            let port: u16 = args.get(2).and_then(|s| s.parse().ok()).expect(usage);
            let drop_every: usize = args.get(3).and_then(|s| s.parse().ok()).expect(usage);
            let count: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(12);
            let de = (drop_every != 0).then_some(drop_every);
            let peer = bind_reliable_peer_on(port, de).expect("bind reliable peer");
            eprintln!(
                "peer: reliable listening on 127.0.0.1:{port} (drop_every={drop_every}, expect {count})"
            );
            let got = reliable_collect(&peer, "peer", count);
            println!(
                "PEER OK: reliable {}/{count} delivered gap-free (loss recovered via ACKNACK)",
                got.len()
            );
        }
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    }
}
