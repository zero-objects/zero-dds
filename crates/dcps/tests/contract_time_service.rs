// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-time-service`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-time-service` (features `std` only),
//! from `src/time_source.rs`:
//! - `time_base::{TICKS_PER_SECOND, UTC_EPOCH_TO_UNIX_TICKS}` to convert
//!   DCPS `Time_t` (seconds since 1970) to `TimeT` (100 ns since
//!   1582-10-15) and back (`time_to_ticks`, `ticks_to_time`).
//! - `time_base::local_tdf` for the tdf of the default
//!   `TimeSource::now_uto`; `UtcT` built from its public fields and with
//!   `UtcT::new` (an inaccuracy above 48 bits is an error), `UtcT::default`;
//!   `Uto::from_utc`, `Uto::{time, inaccuracy, tdf}`.
//! - `TimeService::with_source` (a source returning 0 is unavailable),
//!   `with_inaccuracy`, `with_tdf`, `inaccuracy`, `universal_time` for
//!   `TimeServiceClock`: `universal_time` carries the source's tick, the
//!   service's inaccuracy and the tdf, and fails with `TimeUnavailable`
//!   when the source gives 0 (`TimeServiceClock::now` reports
//!   `Time::INVALID` then).
//!
//! Oracle: OMG Time Service 1.1 §1.3.2.1 (`TimeT`: 100 ns units since
//! 15 October 1582), §1.3.2.2 (inaccuracy: 48 bits), §1.3.2.4 (the tdf of
//! the local time zone in every absolute UTO), §2.1.1 (`universal_time`,
//! `TimeUnavailable`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_time_service::time_base::{TICKS_PER_SECOND, UTC_EPOCH_TO_UNIX_TICKS, local_tdf};
use zerodds_time_service::{MAX_INACCURACY, TdfT, TimeService, TimeUnavailable, UtcT, Uto};

/// 2026-07-15T12:00:00Z as `TimeT`.
const JULY: u64 = UTC_EPOCH_TO_UNIX_TICKS + 1_784_116_800 * TICKS_PER_SECOND;

#[test]
fn time_1_3_2_1_the_epoch_offset_and_tick_are_those_of_the_gregorian_change() {
    // 141 427 days from 1582-10-15 to 1970-01-01, 10^7 ticks a second.
    assert_eq!(TICKS_PER_SECOND, 10_000_000);
    assert_eq!(UTC_EPOCH_TO_UNIX_TICKS, 141_427 * 86_400 * TICKS_PER_SECOND);
}

#[test]
fn time_2_1_1_universal_time_carries_tick_inaccuracy_and_stated_tdf() {
    let service = TimeService::with_source(|| JULY)
        .with_inaccuracy(5_000)
        .unwrap()
        .with_tdf(-300);
    assert_eq!(service.inaccuracy(), 5_000);
    let uto = service.universal_time().unwrap();
    assert_eq!(uto.time(), JULY);
    assert_eq!(uto.inaccuracy(), 5_000);
    assert_eq!(uto.tdf(), -300);
}

#[test]
fn time_2_1_1_a_source_giving_zero_is_time_unavailable() {
    let service = TimeService::with_source(|| 0);
    assert!(matches!(service.universal_time(), Err(TimeUnavailable)));
}

#[test]
fn time_1_3_2_2_inaccuracy_above_48_bits_is_refused() {
    assert!(
        TimeService::with_source(|| JULY)
            .with_inaccuracy(MAX_INACCURACY)
            .is_ok()
    );
    assert!(
        TimeService::with_source(|| JULY)
            .with_inaccuracy(MAX_INACCURACY + 1)
            .is_err()
    );
    assert!(UtcT::new(0, MAX_INACCURACY + 1, 0).is_err());
    let utc = UtcT::new(0, MAX_INACCURACY, 0).unwrap();
    assert_eq!(Uto::from_utc(utc).inaccuracy(), MAX_INACCURACY);
    assert_eq!(UtcT::default(), UtcT::new(0, 0, 0).unwrap());
}

#[test]
fn uto_from_utc_keeps_the_fields() {
    let uto = Uto::from_utc(UtcT {
        time: JULY,
        inacclo: 7,
        inacchi: 1,
        tdf: 330,
    });
    assert_eq!(uto.time(), JULY);
    assert_eq!(uto.inaccuracy(), (1 << 32) | 7);
    assert_eq!(uto.tdf(), 330);
}

/// The environment variable that turns [`local_tdf_probe_child`] on.
const LOCAL_TDF_PROBE_ENV: &str = "ZERODDS_DCPS_CONTRACT_TDF_PROBE";

/// Runs only as the child process [`probe`] starts with `TZ` set; in a
/// normal run the variable is absent and it returns at once. Prints
/// `local_tdf` and the tdf of `universal_time` of a service without a
/// stated tdf, both at [`JULY`].
#[test]
#[allow(clippy::print_stdout)]
fn local_tdf_probe_child() {
    if std::env::var_os(LOCAL_TDF_PROBE_ENV).is_none() {
        return;
    }
    let service = TimeService::with_source(|| JULY);
    let uto = service.universal_time().unwrap();
    println!("ZERODDS-CONTRACT-TDF {} {}", local_tdf(JULY), uto.tdf());
}

/// Starts this test binary as a child with `TZ=tz` and returns what
/// [`local_tdf_probe_child`] printed.
fn probe(tz: &str) -> (TdfT, TdfT) {
    let exe = std::env::current_exe().expect("test binary");
    let out = std::process::Command::new(exe)
        .args([
            "--exact",
            "local_tdf_probe_child",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env("TZ", tz)
        .env(LOCAL_TDF_PROBE_ENV, "1")
        .output()
        .expect("start the probe");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|l| {
            l.split_once("ZERODDS-CONTRACT-TDF ")
                .map(|(_, rest)| rest.trim())
        })
        .unwrap_or_else(|| panic!("TZ={tz}: no probe line in {stdout:?}"));
    let (local, universal) = line.split_once(' ').expect("two values");
    (local.parse().expect("tdf"), universal.parse().expect("tdf"))
}

#[test]
fn time_1_3_2_4_local_tdf_is_the_zone_of_the_process_at_the_time() {
    // July: daylight saving time in both zones.
    assert_eq!(probe("CET-1CEST,M3.5.0,M10.5.0/3"), (120, 120));
    assert_eq!(probe("EST5EDT,M3.2.0,M11.1.0"), (-240, -240));
    assert_eq!(probe("IST-5:30"), (330, 330));
}
