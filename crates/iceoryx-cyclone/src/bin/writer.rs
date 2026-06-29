// SPDX-License-Identifier: Apache-2.0
//! Proof (write direction): ZeroDDS publishes a sample over iceoryx (C++/POSH)
//! SHM that a Cyclone DDS reader consumes.
//!
//! Offers the iceoryx service `{DDS_CYCLONE, IoxOracle::Sample, .IoxOracleTopic}`,
//! then publishes a raw `IoxOracle::Sample { id=2, value=0xCAFEF00D, data=08..01 }`
//! with a Cyclone-shaped PSMX user header. A Cyclone subscriber (started with
//! ALLOW_NONDISC_WR) reads it. Requires a running `iox-roudi`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

// CycloneIoxWriter & co. are exported only on `all(target_os = "linux",
// feature = "posh")`. `required-features = ["posh"]` (Cargo.toml) handles the
// feature but not the target_os, so gate the body on Linux and provide a
// fallback main for other hosts (e.g. `--all-features` on macOS).
#[cfg(target_os = "linux")]
use zerodds_iceoryx_cyclone::{
    iox_event_name, CycloneIoxWriter, DdsPsmxMetadata, CYCLONE_IOX_SERVICE,
};

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("zerodds-cyclone-iox-writer requires Linux (Cyclone iceoryx / POSH SHM).");
}

#[cfg(target_os = "linux")]
fn main() {
    let event = iox_event_name("", "IoxOracleTopic"); // ".IoxOracleTopic"
    let writer = CycloneIoxWriter::new(
        "zerodds-cyclone-writer",
        CYCLONE_IOX_SERVICE,
        "IoxOracle::Sample",
        &event,
    );

    // Raw self-contained sample: id=2, value=0xCAFEF00D, data=8..1 (native LE).
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&2u32.to_le_bytes());
    payload.extend_from_slice(&0xCAFE_F00Du32.to_le_bytes());
    for k in 0..8u8 {
        payload.push(8 - k);
    }
    // A synthetic but well-formed 16-byte writer GUID.
    let guid = *b"ZERODDS-IOXWRITE";
    let meta = DdsPsmxMetadata::raw(payload.len() as u32, guid);

    println!(
        "==> ZeroDDS publishing IoxOracle::Sample id=2 value=0xCAFEF00D over iceoryx service {{{CYCLONE_IOX_SERVICE} | IoxOracle::Sample | {event}}}"
    );
    let mut sent = 0;
    for _ in 0..200 {
        if writer.write_raw(&meta, &payload) {
            sent += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    println!("published {sent} samples");
}
