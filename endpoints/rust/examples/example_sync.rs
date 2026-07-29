// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
//! five typed `Reading { id, value, label }` samples and delivers them; the
//! subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.
//!
//! Run: `cargo run -p zerodds-endpoint-rust --example example_sync`

// A runnable example: printing to stdout and `expect` on the in-memory path are
// intentional here.
#![allow(clippy::print_stdout, clippy::expect_used)]

use zerodds_cdr::Endianness;
use zerodds_endpoint_rust::{Client, MemTransport, Reading};

fn main() {
    let total: u32 = 5;
    let transport = MemTransport::new();
    let mut client = Client::new(transport.clone());
    for i in 0..total {
        let r = Reading {
            id: 0x1000 + i,
            value: 20.0 + (i as f32) * 0.5,
            label: format!("bay-{i:02}"),
        };
        client.write(&r.marshal(Endianness::Little));
    }

    let mut got: u32 = 0;
    while got < total {
        let Some(body) = client.poll() else { break };
        let r = Reading::decode(&body);
        println!(
            "sync reading {got}: id=0x{:x} value={:.1} label=\"{}\"",
            r.id, r.value, r.label
        );
        got += 1;
    }

    assert!(got == total, "incomplete: got {got} of {total}");
    println!("ALL OK");
}
