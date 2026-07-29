// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Deep example (async): the same sensor-telemetry flow, but the subscriber does
//! not own the run-loop. `AsyncReader` spawns a std thread that forwards decoded
//! bodies over an `mpsc` channel; the consumer blocks on `recv` — the idiomatic
//! std concurrency model (no async runtime). Every field is decoded.
//!
//! Run: `cargo run -p zerodds-endpoint-rust --example example_async`

#![allow(clippy::print_stdout, clippy::expect_used)]

use zerodds_cdr::Endianness;
use zerodds_endpoint_rust::{AsyncReader, Client, MemTransport, Reading};

fn main() {
    let total: u32 = 5;
    let transport = MemTransport::new();
    let mut client = Client::new(transport.clone());
    for i in 0..total {
        let r = Reading {
            id: 0x2000 + i,
            value: 100.0 - (i as f32),
            label: format!("sensor-{i:02}"),
        };
        client.write(&r.marshal(Endianness::Little));
    }

    let reader = AsyncReader::start(transport);
    for got in 0..total {
        let body = reader.recv();
        let r = Reading::decode(&body);
        println!(
            "async reading {got}: id=0x{:x} value={:.1} label=\"{}\"",
            r.id, r.value, r.label
        );
    }
    reader.stop();
    println!("ALL OK");
}
