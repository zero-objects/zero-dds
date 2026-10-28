// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Internal cross-process DCPS roundtrip regression gate — runs in CI,
//! needs NO external hosts (a replacement for the old host-bound bench
//! and soak jobs, which ran on separate VMs without a stability gain over
//! the CI runner).
//!
//! ## Why two processes
//! Two `DomainParticipant`s in the SAME process deliver user data over the
//! synchronous in-process fast path (`inproc` + `handle_user_datagram`) to —
//! transport-AGNOSTIC. A single-process bench thus measures, for EVERY
//! selected user-transport value the same in-process path (~1us, all
//! transports identical) and is WORTHLESS as a transport regression gate.
//! So this gate orchestrates the proven `roundtrip-1us` bin as two
//! separate processes (pong=echo, ping=measurer): separate processes have
//! no `inproc` peers -> data goes over the REAL wire transport.
//!
//! ## What is measured + gated
//! Per cell (Transport × Payload) RTT roundtrips (listener variant = RTT
//! directly in the recv thread, without user-polling latency). Gating is on the
//! **p50**: on the shared, loaded CI hosts (the CI runner and the Linux test host, no
//! dedicated bench host) the absolute `min` is a 1-of-N lucky sample
//! and varies run-to-run ~3x — unusable as a gate metric. The p50 is
//! empirically very stable by contrast (±0.5% across runs at 50k samples), so
//! a +20% gate is practically free of false reds and catches every real
//! a ≥20% median regression. `min`/`p90` are REPORTED as info columns
//! (floor/tail trend visible), but NOT gated.
//!
//! Baselines are host-specific (p50 ~ host scheduling) — they are, on the
//! CI runner via `--update`, NOT on a dev machine.
//!
//! ## Transport axis
//! Only the genuinely distinct loopback transports REACHABLE via the c-api FFI
//! Transports: UDPv4, TCPv4, SHM (`same-host-shm` is c-api default). UDS is
//! NOT a c-api feature in this build and TSN needs a real interface — both deliberately omitted
//! (explicitly logged, no silent cap).
//!
//! ## The same-host channel is its own cell
//! Two ZeroDDS participants on one host open a same-host channel when SPDP
//! reports each other (`crates/dcps/src/same_host_channel.rs`, spec
//! `zerodds-zero-copy-1.0` §6), and it carries every datagram to a peer's
//! **UDP** locator; TCP and SHM user transports keep their own path
//! (`SameHostChannels::add_peer` routes UDP locators only). With the
//! channel on, the `UDPv4` cell measures the channel and not UDP. Until
//! `6bbed212a93` the older per-pair same-host path carried user DATA for
//! every user transport, so all three cells measured that path; measured
//! on the Linux test host at `39a5a0e1c5d`, TCPv4/64 p50 17.5 µs with it and 50.8 µs
//! without (`ZERODDS_SAME_HOST_SHM=0`).
//!
//! So the transport cells run with the channel off
//! (`ZERODDS_SAME_HOST_SHM=0` in both processes) and measure the transport
//! they name, and `SameHost` runs UDPv4 with the channel on and measures
//! the channel.
//!
//! ## Baselines
//! `tests/perf/internal-bench/baselines.txt` (committed, `<zelle> <min_us>
//! <p50_us>` per line). `--update` rewrites them (conscious re-baseline
//! on an accepted change). Without `--update`: exit≠0 (red) if a cell
//! exceeds the baseline floor by > TOLERANCE or the absolute sanity
//! Ceiling breaks.
//!
//! Stage 2 (hook): security profile axis (none/data-enc/all-enc via
//! `roundtrip-1us` security env) as an additional column — same orchestrator.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("regression: Linux-only (cross-process DCPS roundtrip)");
}

#[cfg(target_os = "linux")]
fn main() {
    std::process::exit(inner::run());
}

#[cfg(target_os = "linux")]
mod inner {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;

    /// Transport axis (explicit `--dcps-transport` arguments). Only FFI-
    /// reachable + genuinely distinct: UDPv4, TCPv4, SHM. See module doc.
    /// `(cell name, --dcps-transport, same-host channel on)`.
    const CELLS: &[(&str, &str, bool)] = &[
        ("UDPv4", "UDPv4", false),
        ("TCPv4", "TCPv4", false),
        ("SHM", "SHM", false),
        ("SameHost", "UDPv4", true),
    ];
    /// Deliberately NOT in the matrix (no silent cap — documented/logged here).
    const EXCLUDED: &[(&str, &str)] = &[
        ("UDS", "no c-api feature (same-host-uds)"),
        ("TSN", "needs a real interface (tsn.interface), no loopback"),
        (
            "UDPv6/TCPv6",
            "redundant with v4 for floor latency on loopback",
        ),
    ];
    /// Payload axis (bytes). Small = wakeup-dominated, medium = with copy fraction.
    const PAYLOADS: &[usize] = &[64, 4096];

    const WARMUP: u64 = 3_000;
    const SAMPLES: u64 = 30_000;
    /// +20% on the p50 = red.
    const TOLERANCE: f64 = 1.20;
    /// Absolute sanity ceiling (us) on the p50: catches gross breakage even
    /// without a baseline. Cross-process loopback p50 is really ~30-70us; 5ms
    /// is generous (only ~70-150x regressions trip it).
    const CEILING_US: f64 = 5_000.0;
    /// Wallclock-Notbremse pro `roundtrip-1us`-Prozess (CI-Safety-Net).
    const MAX_RUNTIME_SECS: u64 = 30;
    /// Repetitions per cell; the MEDIAN p50 is gated. On the shared
    /// CI hosts occasionally hit a ~3s contention spike on ONE cell (e.g.
    /// TCPv4/64 72->137us with otherwise stable ~80us). The median over 3 temporally
    /// distributed reps ignore a single spike -> robust +20% gate.
    const REPS: usize = 3;

    const BASELINE_PATH: &str = "tests/perf/internal-bench/baselines.txt";

    struct Cell {
        key: String,
        min_us: f64,
        p50_us: f64,
        p90_us: f64,
        ok: bool,
    }

    /// Path to the `roundtrip-1us` bin next to our own bin (same target dir).
    fn bench_bin() -> PathBuf {
        let exe = std::env::current_exe().expect("current_exe");
        exe.parent().expect("exe dir").join("roundtrip-1us")
    }

    /// Parses a `report()` line of the form `min        = 17920 ns`.
    fn parse_field(txt: &str, key: &str) -> Option<f64> {
        for line in txt.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix(key) {
                let rest = rest.trim_start();
                if let Some(num) = rest.strip_prefix('=') {
                    let digits: String = num
                        .trim_start()
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(v) = digits.parse::<f64>() {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    /// Measure one cell: spawn pong echo process, run ping measurer,
    /// parse its `min`/`p50`/`p90` (ns) -> (min_us, p50_us, p90_us). `None`
    /// on any error (no-match, bin missing, parse fail) -> SETUP_FAIL (loud).
    fn measure(
        bin: &Path,
        transport: &str,
        same_host: bool,
        payload: usize,
        domain: u32,
    ) -> Option<(f64, f64, f64)> {
        if !bin.exists() {
            eprintln!("regression: roundtrip-1us not found: {}", bin.display());
            return None;
        }
        let topic = format!("bench-reg-{transport}-{payload}");
        let dom = domain.to_string();
        let pl = payload.to_string();
        let mr = MAX_RUNTIME_SECS.to_string();
        let same_host = if same_host { "1" } else { "0" };

        // pong (echo) as child process.
        let mut pong = Command::new(bin)
            .args([
                "--role",
                "pong",
                "--use-dcps",
                "--dcps-domain",
                &dom,
                "--payload",
                &pl,
                "--dcps-topic",
                &topic,
                "--max-runtime",
                &mr,
            ])
            .args(["--dcps-transport", transport])
            .env("ZERODDS_SAME_HOST_SHM", same_host)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        // Pong must bring up discovery/endpoints first, otherwise ping misses
        // the first samples (second-RTT artifact).
        thread::sleep(Duration::from_secs(2));

        // ping (measurer, listener RTT) — blocks until samples full or
        // max runtime; collects stdout for the min/p50 parse.
        let warmup = WARMUP.to_string();
        let samples = SAMPLES.to_string();
        let out = Command::new(bin)
            .args([
                "--role",
                "ping",
                "--use-dcps",
                "--dcps-domain",
                &dom,
                "--payload",
                &pl,
                "--dcps-topic",
                &topic,
                "--warmup",
                &warmup,
                "--samples",
                &samples,
                "--listener",
                "--max-runtime",
                &mr,
            ])
            .args(["--dcps-transport", transport])
            .env("ZERODDS_SAME_HOST_SHM", same_host)
            .stderr(Stdio::null())
            .output()
            .ok();

        let _ = pong.kill();
        let _ = pong.wait();

        let out = out?;
        if !out.status.success() {
            return None;
        }
        let txt = String::from_utf8_lossy(&out.stdout);
        let min_ns = parse_field(&txt, "min")?;
        let p50_ns = parse_field(&txt, "p50")?;
        let p90_ns = parse_field(&txt, "p90")?;
        Some((min_ns / 1000.0, p50_ns / 1000.0, p90_ns / 1000.0))
    }

    /// Median of a non-empty f64 slice (sorted internally; for an even
    /// length the lower of the two means — conservative).
    fn median(v: &mut [f64]) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        v[(v.len() - 1) / 2]
    }

    /// Measure a cell over `REPS` temporally spread reps and aggregate:
    /// MEDIAN p50 (spike-robust), MIN of the mins (best floor), MEDIAN p90.
    /// `base_domain` + rep index yield a disjoint domain per rep (no
    /// TIME_WAIT/discovery bleed between back-to-back reps). `None` if the
    /// majority of reps fails (setup broken) -> cell = SETUP_FAIL.
    fn measure_cell(
        bin: &Path,
        transport: &str,
        same_host: bool,
        payload: usize,
        base_domain: u32,
    ) -> Option<(f64, f64, f64)> {
        let mut mins = Vec::new();
        let mut p50s = Vec::new();
        let mut p90s = Vec::new();
        for rep in 0..REPS {
            // Disjoint domain per rep (cap < 232).
            let domain = 140 + ((base_domain - 140 + rep as u32 * 7) % 80);
            if let Some((mn, p50, p90)) = measure(bin, transport, same_host, payload, domain) {
                mins.push(mn);
                p50s.push(p50);
                p90s.push(p90);
            }
            thread::sleep(Duration::from_millis(300));
        }
        // The majority must succeed, otherwise the setup is broken.
        if p50s.len() <= REPS / 2 {
            return None;
        }
        let min_us = mins.iter().copied().fold(f64::INFINITY, f64::min);
        let p50_us = median(&mut p50s);
        let p90_us = median(&mut p90s);
        Some((min_us, p50_us, p90_us))
    }

    /// Baseline line: `<key> <p50_us> <min_us> <p90_us>`. p50 is gated
    /// (first column); min/p90 are info. Missing info columns = 0.0.
    fn load_baselines(path: &Path) -> BTreeMap<String, (f64, f64, f64)> {
        let mut m = BTreeMap::new();
        if let Ok(txt) = std::fs::read_to_string(path) {
            for line in txt.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let mut it = line.split_whitespace();
                if let (Some(k), Some(p50)) = (it.next(), it.next()) {
                    if let Ok(p50) = p50.parse() {
                        let min = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let p90 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        m.insert(k.to_string(), (p50, min, p90));
                    }
                }
            }
        }
        m
    }

    pub fn run() -> i32 {
        let update = std::env::args().any(|a| a == "--update");
        let baseline_path = Path::new(BASELINE_PATH);
        let baselines = load_baselines(baseline_path);
        let bin = bench_bin();

        println!(
            "== internal bench regression (cross-process RTT, median-p50 over {REPS} reps, gate +{}%) ==",
            ((TOLERANCE - 1.0) * 100.0) as i32
        );
        for (t, why) in EXCLUDED {
            println!("   (skipped: {t} — {why})");
        }
        println!(
            "{:<16} {:>9} {:>9} {:>9} {:>10} {:>16}",
            "cell", "min_us", "p50_us", "p90_us", "base_p50", "status"
        );

        let mut cells: Vec<Cell> = Vec::new();
        let mut idx: u32 = 0;
        for &(cell, t, same_host) in CELLS {
            for &pl in PAYLOADS {
                let key = format!("{cell}/{pl}");
                // Disjoint base domain per cell (no discovery bleed). The
                // Reps derive from it. Cap < 232.
                let domain = 140 + ((idx * REPS as u32) % 60);
                idx += 1;
                // Baseline = p50 (first column of the baseline line).
                let base_p50 = baselines.get(&key).map(|&(p50, ..)| p50);
                let (min_us, p50_us, p90_us, status, ok) =
                    match measure_cell(&bin, t, same_host, pl, domain) {
                        Some((min_us, p50_us, p90_us)) => {
                            if p50_us > CEILING_US {
                                (
                                    min_us,
                                    p50_us,
                                    p90_us,
                                    format!("CEILING(>{CEILING_US:.0})"),
                                    false,
                                )
                            } else if let Some(bp50) = base_p50 {
                                if p50_us > bp50 * TOLERANCE {
                                    let pct = (p50_us / bp50 - 1.0) * 100.0;
                                    (
                                        min_us,
                                        p50_us,
                                        p90_us,
                                        format!("REGRESSED(+{pct:.0}%)"),
                                        false,
                                    )
                                } else {
                                    (min_us, p50_us, p90_us, "OK".to_string(), true)
                                }
                            } else {
                                (min_us, p50_us, p90_us, "NEW".to_string(), true)
                            }
                        }
                        None => (
                            f64::NAN,
                            f64::NAN,
                            f64::NAN,
                            "SETUP_FAIL".to_string(),
                            false,
                        ),
                    };
                let base = base_p50
                    .map(|p| format!("{p:.2}"))
                    .unwrap_or_else(|| "-".into());
                println!(
                    "{key:<16} {min_us:>9.2} {p50_us:>9.2} {p90_us:>9.2} {base:>10} {status:>16}"
                );
                cells.push(Cell {
                    key,
                    min_us,
                    p50_us,
                    p90_us,
                    ok,
                });
                // Settle (TIME_WAIT / discovery cleanup between cells).
                thread::sleep(Duration::from_millis(500));
            }
        }

        if update {
            let mut out = String::from(
                "# Internal-Bench-Baselines (committed). Spalten: <transport/payload> <p50_us> <min_us> <p90_us>\n\
                 # Metric: cross-process DCPS roundtrip RTT (listener). p50 is gated.\n\
                 # HOST-SPECIFIC: seeded on the CI runner via --update, not on dev machines.\n\
                 # Regeneriert via: cargo run -p zerodds-bench-suite --bin regression --release -- --update\n\
                 # Gate (without --update): p50 > base_p50 * 1.20 OR > 5000us => red.\n",
            );
            for c in &cells {
                if c.p50_us.is_finite() {
                    out.push_str(&format!(
                        "{} {:.2} {:.2} {:.2}\n",
                        c.key, c.p50_us, c.min_us, c.p90_us
                    ));
                }
            }
            if let Some(parent) = baseline_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(baseline_path, out).expect("write baselines");
            println!("\n== baselines updated: {} ==", baseline_path.display());
            // Still red under --update if a cell was not measurable at all
            // (a NaN baseline would be worthless).
            return if cells.iter().any(|c| !c.p50_us.is_finite()) {
                eprintln!("WARN: cell(s) not measurable — baseline incomplete");
                1
            } else {
                0
            };
        }

        let failed: Vec<&Cell> = cells.iter().filter(|c| !c.ok).collect();
        if failed.is_empty() {
            println!("\n== all {} cells OK ==", cells.len());
            0
        } else {
            println!("\n== {} cell(s) RED ==", failed.len());
            for c in &failed {
                println!("   {}", c.key);
            }
            1
        }
    }
}
