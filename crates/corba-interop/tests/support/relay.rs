// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A TCP relay between a client and a server that forwards GIOP and ZIOP
//! messages unchanged and records the magic and message type of each, per
//! direction: the observer of what went over the wire.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// The magic and message type of each message one direction carried.
pub type Seen = Arc<Mutex<Vec<([u8; 4], u8)>>>;

/// A relay on a port of 127.0.0.1 to one target.
pub struct Relay {
    /// The port clients connect to.
    pub port: u16,
    /// What the clients sent.
    pub to_server: Seen,
    /// What the server answered.
    pub to_client: Seen,
}

fn pump(mut from: TcpStream, mut to: TcpStream, seen: Seen) {
    loop {
        let mut header = [0u8; 12];
        if from.read_exact(&mut header).is_err() {
            break;
        }
        let size = if header[6] & 1 == 1 {
            u32::from_le_bytes([header[8], header[9], header[10], header[11]])
        } else {
            u32::from_be_bytes([header[8], header[9], header[10], header[11]])
        };
        let mut body = vec![0u8; size as usize];
        if from.read_exact(&mut body).is_err() {
            break;
        }
        seen.lock()
            .unwrap()
            .push(([header[0], header[1], header[2], header[3]], header[7]));
        if to.write_all(&header).is_err() || to.write_all(&body).is_err() {
            break;
        }
    }
    let _ = to.shutdown(Shutdown::Write);
}

/// Starts a relay to `host:port`.
pub fn relay(host: &str, port: u16) -> Relay {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let relay_port = listener.local_addr().unwrap().port();
    let to_server: Seen = Arc::default();
    let to_client: Seen = Arc::default();
    let (up, down) = (Arc::clone(&to_server), Arc::clone(&to_client));
    let host = host.to_string();
    std::thread::spawn(move || {
        for client in listener.incoming() {
            let Ok(client) = client else { break };
            let server = TcpStream::connect((host.as_str(), port)).unwrap();
            let (c2, s2) = (client.try_clone().unwrap(), server.try_clone().unwrap());
            let (up, down) = (Arc::clone(&up), Arc::clone(&down));
            std::thread::spawn(move || pump(client, server, up));
            std::thread::spawn(move || pump(s2, c2, down));
        }
    });
    Relay {
        port: relay_port,
        to_server,
        to_client,
    }
}

/// The magics one direction carried, in order.
pub fn magics(seen: &Seen) -> Vec<[u8; 4]> {
    seen.lock().unwrap().iter().map(|(m, _)| *m).collect()
}

/// The magics of the messages of type `message_type` one direction
/// carried, in order.
pub fn magics_of(seen: &Seen, message_type: u8) -> Vec<[u8; 4]> {
    seen.lock()
        .unwrap()
        .iter()
        .filter(|(_, t)| *t == message_type)
        .map(|(m, _)| *m)
        .collect()
}
