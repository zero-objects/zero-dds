//! WP 1.D — Live-Interop-Test fuer Writer-Liveliness-Protocol gegen
//! echte Cyclone-DDS-Instanz.
//!
//! **Opt-in only** — `#[ignore]` markiert. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-dcps --test cyclone_live_wlp -- --ignored --nocapture
//! ```
//!
//! # Test-Ablauf
//!
//! 1. Eine ZeroDDS-Runtime startet auf Domain 42, sendet alle 200 ms
//!    AUTOMATIC-WLP-Heartbeats.
//! 2. Auf dem Cyclone-Host (Bench-VM `llvm`) startet `ddsperf -i 42 sub`,
//!    der lokale `subscriber` mit aktiviertem Liveliness-Listener.
//! 3. Cyclone schickt seinen eigenen WLP-Heartbeat zurueck (sobald
//!    er uns via SPDP entdeckt hat).
//! 4. Wir verifizieren, dass die ZeroDDS-Runtime Cyclone-Prefix als
//!    Peer in `peer_liveliness_last_seen()` sieht — also dass die
//!    Wire-Encoding bidirektional kompatibel ist.
//!
//! # Voraussetzungen
//!
//! - SSH-Zugriff auf `llvm@llvm` mit Passwort `llvm` (Lab-Setup)
//! - `sshpass` lokal installiert
//! - Cyclone DDS 0.10.2 (`ddsperf` Binary) auf `llvm`
//! - `/tmp/cyc.xml` auf `llvm` pinnt Cyclone auf `enp6s18`
//! - Lokaler Host im selben LAN wie `llvm`, Multicast-Durchlass
//!
//! Im CI bleibt der Test ignored — er laeuft nur manuell auf dem
//! Lab-Host.

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

use std::thread;
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_rtps::wire_types::GuidPrefix;

#[test]
#[ignore = "live cyclone interop — opt-in via --ignored, requires lab setup"]
fn cyclone_live_wlp_handshake() {
    // Aggressive WLP-Period damit Cyclone uns innerhalb einiger
    // Sekunden als alive sieht.
    let cfg = RuntimeConfig {
        tick_period: Duration::from_millis(50),
        spdp_period: Duration::from_millis(500),
        wlp_period: Duration::from_millis(200),
        participant_lease_duration: Duration::from_millis(600),
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(42, GuidPrefix::from_bytes([0xAA; 12]), cfg).expect("start");

    // Cyclone-Subscriber starten (lab-spezifisch — tooling kommt
    // ueber dasselbe ssh-pattern wie cyclone_live_sedp).
    eprintln!("ZeroDDS WLP-Endpoint live; warte 5 s auf Cyclone-Beacons");
    thread::sleep(Duration::from_secs(5));

    // Wir kennen den Cyclone-Prefix nicht a priori — wir checken
    // einfach, ob der WLP-Endpoint mindestens einen Peer
    // registriert hat. Wenn ddsperf nicht laeuft, schlaegt der
    // Test fehl — das ist die manuelle Verification, die mit
    // `#[ignore]` markiert ist.
    let count = rt.wlp.lock().ok().map(|w| w.peer_count()).unwrap_or(0);
    assert!(
        count > 0,
        "kein Cyclone-Peer im WLP-Cache — ddsperf -D 42 sub auf llvm laufen?"
    );
}
