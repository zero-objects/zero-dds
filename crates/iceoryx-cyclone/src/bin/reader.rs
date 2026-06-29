// SPDX-License-Identifier: Apache-2.0
//! Proof: ZeroDDS itself reads a Cyclone DDS sample over iceoryx (C++/POSH) SHM.
//!
//! Discovers Cyclone's iceoryx service for the IDL type, subscribes, and decodes
//! the raw self-contained `IoxOracle::Sample { u32 id; u32 value; octet[8] }`
//! from the chunk — asserting the live wire matches what the C oracle saw.
//! Requires a running `iox-roudi` and a Cyclone iox publisher.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

// CycloneIoxReader is exported only on `all(target_os = "linux", feature =
// "posh")`. `required-features = ["posh"]` (Cargo.toml) handles the feature,
// but cannot express the target_os, so gate the body on Linux and provide a
// fallback main for other hosts (e.g. `--all-features` on macOS).
#[cfg(target_os = "linux")]
use zerodds_iceoryx_cyclone::CycloneIoxReader;

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("zerodds-cyclone-iox-reader requires Linux (Cyclone iceoryx / POSH SHM).");
}

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    let instance = "IoxOracle::Sample";
    let Some((service, inst, event)) =
        CycloneIoxReader::discover_by_instance("zerodds-cyclone-reader", instance, 50)
    else {
        eprintln!("READER FAIL: no Cyclone iox service for {instance}");
        return std::process::ExitCode::FAILURE;
    };
    println!("==> ZeroDDS reader found Cyclone service {{{service} | {inst} | {event}}}");
    let reader = CycloneIoxReader::new("zerodds-cyclone-reader", &service, &inst, &event);

    for _ in 0..100 {
        if let Some((meta, payload)) = reader.take() {
            println!(
                "METADATA: sample_size={} state={} cdr_id=0x{:04x} statusinfo={} raw={}",
                meta.sample_size,
                meta.sample_state,
                meta.cdr_identifier,
                meta.statusinfo,
                meta.is_raw()
            );
            print!("PAYLOAD[{}]:", payload.len());
            for b in &payload {
                print!(" {b:02x}");
            }
            println!();
            let id = u32::from_le_bytes(payload[0..4].try_into().unwrap());
            let value = u32::from_le_bytes(payload[4..8].try_into().unwrap());
            let data_ok = (0..8).all(|k| payload[8 + k] == (k as u8) + 1);
            if meta.is_raw() && meta.sample_size == 16 && id == 1 && value == 0x1234_5678 && data_ok
            {
                println!(
                    "READER OK: ZeroDDS read the Cyclone sample over iceoryx SHM (id=1 value=0x12345678 data=01..08)"
                );
                return std::process::ExitCode::SUCCESS;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!("READER FAIL: no matching sample");
    std::process::ExitCode::FAILURE
}
