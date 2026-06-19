//! C3 discovery probe — isolates the **participant-discovery (SPDP)** phase that
//! the initial-announcement burst targets, decoupled from the SEDP
//! (endpoint-discovery) reliable handshake that `latency_ping`'s pubs/subs
//! metric folds in.
//!
//! Creates a bare participant (no endpoints → no SEDP traffic) and prints the
//! wall-clock time until it discovers at least one peer participant
//! (`participants > 0`). Run two instances pointing at each other
//! (`ZERODDS_NO_MULTICAST=1 ZERODDS_PEERS=127.0.0.1`); under SPDP loss the burst
//! (default) reaches `participants>0` far faster / more reliably than the
//! legacy single-beacon path (`ZERODDS_INITIAL_ANNOUNCE_COUNT=0`).

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::{Duration, Instant};

use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(0, DomainParticipantQos::default())?;

    let deadline = Instant::now() + Duration::from_secs(35);
    let start = Instant::now();
    loop {
        let parts = participant.discovered_participants_count();
        if parts > 0 {
            println!(
                "PROBE: participant discovered in {} ms (participants={parts})",
                start.elapsed().as_millis()
            );
            break;
        }
        if Instant::now() >= deadline {
            println!("PROBE: TIMEOUT (>35s, participants=0)");
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Stay alive a moment so a peer probe can also discover us.
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}
