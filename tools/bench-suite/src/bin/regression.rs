// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Internal cross-process DCPS roundtrip regression gate. CI runs it as
//! `speed-regression` of the speed pillar (`ci/jobs/speed-pillar.yml`,
//! `tests/perf/speed/pillar.sh regression`) on the exclusive speed host;
//! it needs no second host. On the shared CI runner its SHM and SameHost
//! cells measured above the tolerance while other jobs ran beside it
//! (pipeline 8962).
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
//! **p50**: the absolute `min` is a 1-of-N lucky sample and varies
//! run-to-run ~3x — unusable as a gate metric. `min`/`p90` are REPORTED as
//! info columns (floor/tail trend visible), but NOT gated.
//!
//! ## Against the release, in the same window (A/B)
//! The gate does not compare with stored numbers. It measures two builds of
//! `roundtrip-1us` in turn, in the same reservation of the speed host: A, the
//! last release tag (`--reference`, built by `pillar.sh build-reference`
//! from the tag with this tree's `roundtrip_1us.rs`, so the two differ by
//! the library), and B, the checked tree (the `roundtrip-1us` beside this
//! bin). Each cell runs `REPS` repetitions of the pair, the order swapped
//! every repetition (A B, B A, A B, ...); the gate is the median p50 of B
//! against the median p50 of A. The speed host is exclusive on the board,
//! its virtualization host is not: runs of one binary spread by up to 31 %
//! (UDPv4/64, internal/release/rc8-test-suite.md "Speed"), and a stored
//! baseline took that load as a regression or hid one. Load in the window
//! now meets both sides.
//!
//! The tolerance (`TOLERANCE_PCT`) lies above the spread of the A/B
//! difference of one binary against itself (A/A) measured on the speed
//! host; the numbers are at the constant.
//!
//! Without `--reference` the gate measures nothing and fails: it has no
//! stored numbers to fall back to. A cell the reference tag cannot measure
//! at all is declared in `REFERENCE_GAPS` with the reason (`--reference-tag`
//! names the tag); there the checked tree is measured alone against the
//! ceiling, and the log says so.
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
//! `c996fd7a076` the older per-pair same-host path carried user DATA for
//! every user transport, so all three cells measured that path; measured
//! on the Linux test host at `8f94dbd7d6e`, TCPv4/64 p50 17.5 µs with it and 50.8 µs
//! without (`ZERODDS_SAME_HOST_SHM=0`).
//!
//! So the transport cells run with the channel off
//! (`ZERODDS_SAME_HOST_SHM=0` in both processes) and measure the transport
//! they name, and `SameHost` runs UDPv4 with the channel on and measures
//! the channel.
//!
//! ## Domains
//! A DDS domain is a range of UDP ports of the whole host (RTPS §9.6.1.4.1),
//! so the gate does not pick domain numbers itself: it runs under
//! `tests/integration/with_domains.py`, which leases them through the
//! protocol of `crates/test-domains` and hands them over in
//! `ZERODDS_TEST_DOMAINS`. The `k`-th measurement of a cell runs on the
//! `k`-th leased domain (modulo their number), so two measurements in a row
//! never share a domain:
//!
//! ```text
//! python3 tests/integration/with_domains.py 3 -- \
//!     target/release/regression --reference <rc.7 build>/roundtrip-1us
//! ```
//!
//! `--cells UDPv4,TCPv4` measures those cells only (the pillar splits the
//! gate over two jobs of at most 5 minutes); `--reps N` changes the
//! repetitions (the A/A measurement of the tolerance).

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
    /// How much the median p50 of B may lie above that of A, in percent, in
    /// every cell. From the spread of the A/B difference of one binary
    /// against itself (A/A), measured on the exclusive speed host with this
    /// gate (five repetitions per side): 24 cell-runs (the eight cells, three
    /// gate runs), differences of the medians from -10.4 to +17.0 %, mean
    /// -0.1 %, standard deviation 5.6 %; the build of rc.7 against `main` in
    /// the same window (12 cell-runs of the six cells rc.7 measures) -8.9 to
    /// +9.1 %, mean -1.7 %, standard deviation 6.1 %. 25 % lies 8 points
    /// above the largest A/A difference and 4.4 standard deviations above
    /// no difference; a threshold of 15 % would have turned one of the 24 A/A
    /// cell-runs red (SameHost/64, +17.0 %).
    const TOLERANCE_PCT: f64 = 25.0;
    /// Cells the reference cannot measure, per reference tag (`--reference-tag`),
    /// with the reason: there the gate measures the checked tree alone,
    /// against `CEILING_US`, and says so. A cell of another tag, or a
    /// reference that fails in a cell not named here, is a `SETUP_FAIL`.
    const REFERENCE_GAPS: &[(&str, &str, &str)] = &[(
        "v1.0.0-rc.7",
        "SHM",
        "the SHM user transport of rc.7 opens the ring it reads a peer from at its \
         own first send to that peer, and the pong sends nothing before it has \
         received (fixed in 9ba7af8cf3a): 0 of 30 000 echoes in each of 10 \
         repetitions on the Linux test host",
    )];
    /// Absolute sanity ceiling (us) on the p50: catches gross breakage even
    /// when the release is as slow. Cross-process loopback p50 is really
    /// ~20-50us; 5ms is generous (only ~100x regressions trip it).
    const CEILING_US: f64 = 5_000.0;
    /// Wallclock-Notbremse pro `roundtrip-1us`-Prozess (CI-Safety-Net).
    const MAX_RUNTIME_SECS: u64 = 30;
    /// Repetitions of the A/B pair per cell; the MEDIAN p50 of each side is
    /// gated, so one disturbed run of either side does not decide.
    const REPS: usize = 5;

    /// The variable `tests/integration/with_domains.py` hands the leased
    /// domains over in, comma-separated.
    const DOMAINS_VARIABLE: &str = "ZERODDS_TEST_DOMAINS";

    /// The leased domains, or why there are none.
    fn leased_domains() -> Result<Vec<u32>, String> {
        let raw = std::env::var(DOMAINS_VARIABLE).unwrap_or_default();
        let domains = raw
            .split(',')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(|d| {
                d.parse::<u32>()
                    .map_err(|_| format!("{DOMAINS_VARIABLE}={raw}: `{d}` is no domain"))
            })
            .collect::<Result<Vec<u32>, String>>()?;
        if domains.is_empty() {
            return Err(format!(
                "{DOMAINS_VARIABLE} is not set: run the gate under \
                 `python3 tests/integration/with_domains.py 3 -- <regression>`, \
                 which leases the domains (crates/test-domains)"
            ));
        }
        Ok(domains)
    }

    /// The command line: `--reference <bin>`, `--cells a,b`, `--reps n`.
    struct Args {
        reference: Option<PathBuf>,
        reference_tag: Option<String>,
        cells: Option<Vec<String>>,
        reps: usize,
    }

    fn parse_args() -> Result<Args, String> {
        let mut args = Args {
            reference: None,
            reference_tag: None,
            cells: None,
            reps: REPS,
        };
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
            match a.as_str() {
                "--reference" => args.reference = Some(PathBuf::from(value()?)),
                "--reference-tag" => args.reference_tag = Some(value()?),
                "--cells" => {
                    args.cells = Some(value()?.split(',').map(str::to_string).collect());
                }
                "--reps" => {
                    let v = value()?;
                    args.reps = v
                        .parse()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or_else(|| format!("--reps {v}: a positive number"))?;
                }
                other => return Err(format!("unknown argument {other}")),
            }
        }
        if let Some(cells) = &args.cells {
            for c in cells {
                if !CELLS.iter().any(|(name, ..)| name == c) {
                    return Err(format!(
                        "--cells: no cell {c} (cells: {})",
                        CELLS.iter().map(|(n, ..)| *n).collect::<Vec<_>>().join(",")
                    ));
                }
            }
        }
        Ok(args)
    }

    /// Path to the `roundtrip-1us` bin next to our own bin (same target dir).
    fn candidate_bin() -> PathBuf {
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

    /// One measurement of one build: (min_us, p50_us, p90_us).
    #[derive(Clone, Copy)]
    struct Sample {
        min_us: f64,
        p50_us: f64,
        p90_us: f64,
    }

    /// Measure one cell once: spawn pong echo process, run ping measurer,
    /// parse its `min`/`p50`/`p90` (ns). `None` on any error (no-match, bin
    /// missing, parse fail), with the reason on stderr.
    fn measure(
        bin: &Path,
        transport: &str,
        same_host: bool,
        payload: usize,
        domain: u32,
    ) -> Option<Sample> {
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
            .output()
            .ok();

        let _ = pong.kill();
        let _ = pong.wait();

        let out = out?;
        if !out.status.success() {
            eprintln!(
                "regression: {transport}/{payload} on domain {domain}: ping {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return None;
        }
        let txt = String::from_utf8_lossy(&out.stdout);
        // A ping that matched nothing or got no echo still exits 0 at its
        // max runtime, with an empty histogram whose quantiles read 0: the
        // SHM cells reported 0.00 us as OK (pipeline 8827, job 229115;
        // 55.97 us in job 223421). Fewer than half the samples is no
        // measurement.
        let received = parse_field(&txt, "n")?;
        if received < (SAMPLES / 2) as f64 {
            eprintln!(
                "regression: {transport}/{payload} on domain {domain}: {received} of {SAMPLES} \
                 samples came back: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return None;
        }
        Some(Sample {
            min_us: parse_field(&txt, "min")? / 1000.0,
            p50_us: parse_field(&txt, "p50")? / 1000.0,
            p90_us: parse_field(&txt, "p90")? / 1000.0,
        })
    }

    /// Median of a non-empty f64 slice (sorted internally; for an even
    /// length the lower of the two means — conservative).
    fn median(v: &[f64]) -> f64 {
        let mut v = v.to_vec();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        v[(v.len() - 1) / 2]
    }

    /// The repetitions of one side of a cell, aggregated: MEDIAN p50
    /// (spike-robust), MIN of the mins (best floor), MEDIAN p90; `None` when
    /// the majority of its repetitions failed (setup broken).
    fn aggregate(samples: &[Sample], reps: usize) -> Option<Sample> {
        if samples.len() <= reps / 2 {
            return None;
        }
        let p50s: Vec<f64> = samples.iter().map(|s| s.p50_us).collect();
        let p90s: Vec<f64> = samples.iter().map(|s| s.p90_us).collect();
        Some(Sample {
            min_us: samples
                .iter()
                .map(|s| s.min_us)
                .fold(f64::INFINITY, f64::min),
            p50_us: median(&p50s),
            p90_us: median(&p90s),
        })
    }

    fn list(samples: &[Sample]) -> String {
        samples
            .iter()
            .map(|s| format!("{:.2}", s.p50_us))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn run() -> i32 {
        let args = match parse_args() {
            Ok(a) => a,
            Err(why) => {
                eprintln!("regression: {why}");
                return 2;
            }
        };
        let Some(reference) = args.reference.clone() else {
            eprintln!(
                "regression: no --reference: the gate measures the checked tree against a \
                 build of the last release tag in the same window and has no stored numbers \
                 (pillar.sh build-reference builds it)"
            );
            return 2;
        };
        let candidate = candidate_bin();
        let domains = match leased_domains() {
            Ok(domains) => domains,
            Err(why) => {
                eprintln!("regression: {why}");
                return 2;
            }
        };
        let reps = args.reps;

        println!(
            "== internal bench regression (cross-process RTT): B against A, median p50 over \
             {reps} alternating repetitions per side =="
        );
        println!(
            "   A (reference): {}{}",
            reference.display(),
            args.reference_tag
                .as_deref()
                .map(|t| format!(" ({t})"))
                .unwrap_or_default()
        );
        let gap_of = |cell: &str| {
            REFERENCE_GAPS
                .iter()
                .find(|(tag, c, _)| Some(*tag) == args.reference_tag.as_deref() && *c == cell)
                .map(|(_, _, why)| *why)
        };
        for (_, cell, why) in REFERENCE_GAPS
            .iter()
            .filter(|(tag, ..)| Some(*tag) == args.reference_tag.as_deref())
        {
            println!("   (A not measured in {cell}: {why})");
        }
        println!("   B (candidate): {}", candidate.display());
        for (t, why) in EXCLUDED {
            println!("   (skipped: {t} — {why})");
        }
        println!("   (domains: {domains:?}, leased)");
        println!(
            "{:<16} {:>9} {:>9} {:>9} {:>9} {:>8} {:>6} {:>14}",
            "cell", "A_p50", "B_p50", "B_min", "B_p90", "B/A", "tol", "status"
        );

        let mut failed: Vec<String> = Vec::new();
        let mut measured = 0usize;
        let mut slot = 0usize;
        for &(cell, t, same_host) in CELLS {
            if let Some(only) = &args.cells
                && !only.iter().any(|c| c == cell)
            {
                continue;
            }
            for &pl in PAYLOADS {
                let key = format!("{cell}/{pl}");
                let mut a: Vec<Sample> = Vec::new();
                let mut b: Vec<Sample> = Vec::new();
                let b_only = gap_of(cell).is_some();
                for rep in 0..reps {
                    // A B in even repetitions, B A in odd ones: a host that
                    // drifts over the cell meets both sides as often.
                    let order: [bool; 2] = if rep % 2 == 0 {
                        [false, true]
                    } else {
                        [true, false]
                    };
                    for is_b in order {
                        if b_only && !is_b {
                            continue;
                        }
                        let bin = if is_b { &candidate } else { &reference };
                        let domain = domains[slot % domains.len()];
                        slot += 1;
                        if let Some(s) = measure(bin, t, same_host, pl, domain) {
                            if is_b { b.push(s) } else { a.push(s) }
                        }
                        thread::sleep(Duration::from_millis(300));
                    }
                }
                let tol = TOLERANCE_PCT;
                println!("ab: cell={key} a_p50=[{}] b_p50=[{}]", list(&a), list(&b));
                let (line, ok) = match (aggregate(&a, reps), aggregate(&b, reps)) {
                    (_, Some(mb)) if b_only => {
                        let status = if mb.p50_us > CEILING_US {
                            format!("CEILING(>{CEILING_US:.0})")
                        } else {
                            "OK(B only)".to_string()
                        };
                        let ok = status == "OK(B only)";
                        (
                            format!(
                                "{key:<16} {:>9} {:>9.2} {:>9.2} {:>9.2} {:>8} {:>6} {status:>14}",
                                "-", mb.p50_us, mb.min_us, mb.p90_us, "-", "-"
                            ),
                            ok,
                        )
                    }
                    (Some(ma), Some(mb)) => {
                        let ratio = mb.p50_us / ma.p50_us;
                        let status = if mb.p50_us > CEILING_US {
                            format!("CEILING(>{CEILING_US:.0})")
                        } else if ratio > 1.0 + tol / 100.0 {
                            format!("REGRESSED(+{:.0}%)", (ratio - 1.0) * 100.0)
                        } else {
                            "OK".to_string()
                        };
                        let ok = status == "OK";
                        (
                            format!(
                                "{key:<16} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>+7.1}% {:>5.0}% {status:>14}",
                                ma.p50_us,
                                mb.p50_us,
                                mb.min_us,
                                mb.p90_us,
                                (ratio - 1.0) * 100.0,
                                tol
                            ),
                            ok,
                        )
                    }
                    (ma, mb) => {
                        let side = match (ma.is_none() && !b_only, mb.is_none()) {
                            (true, true) => "A and B",
                            (true, false) => "A",
                            _ => "B",
                        };
                        (
                            format!("{key:<16} {:>66}", format!("SETUP_FAIL({side})")),
                            false,
                        )
                    }
                };
                println!("{line}");
                measured += 1;
                if !ok {
                    failed.push(key);
                }
                // Settle (TIME_WAIT / discovery cleanup between cells).
                thread::sleep(Duration::from_millis(500));
            }
        }

        if failed.is_empty() {
            println!("\n== all {measured} cells OK ==");
            0
        } else {
            println!("\n== {} of {measured} cell(s) RED ==", failed.len());
            for c in &failed {
                println!("   {c}");
            }
            1
        }
    }
}
