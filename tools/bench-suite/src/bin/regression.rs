// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Internes Cross-Process-DCPS-Roundtrip-Regressions-Gate — laeuft in CI,
//! braucht KEINE externen Hosts (Ersatz fuer die alten host-gebundenen
//! bench-llvm/soak-pivot, die nur pve-VMs ohne Stabilitaetsgewinn ueber glr1
//! waren).
//!
//! ## Warum zwei Prozesse
//! Zwei `DomainParticipant`s im SELBEN Prozess liefern User-Daten ueber den
//! synchronen In-Process-Fastpath (`inproc` + `handle_user_datagram`) zu —
//! transport-AGNOSTISCH. Ein Single-Process-Bench misst also fuer JEDEN
//! `ZERODDS_USER_TRANSPORT`-Wert denselben In-Process-Pfad (~1us, alle
//! Transporte identisch) und ist als Transport-Regressionsgate WERTLOS.
//! Darum orchestriert dieses Gate die bewaehrte `roundtrip-1us`-Bin als zwei
//! getrennte Prozesse (pong=Echo, ping=Messer): getrennte Prozesse haben
//! keine `inproc`-Peers -> Daten laufen ueber den ECHTEN Wire-Transport.
//!
//! ## Was gemessen + gegated wird
//! Pro Zelle (Transport × Payload) RTT-Roundtrips (Listener-Variante = RTT
//! direkt im Recv-Thread, ohne User-Polling-Latenz). Gegated wird auf den
//! **p50**: auf den geteilten, belasteten CI-Hosts (glr1/codepit, kein
//! dedizierter Bench-Host) ist der absolute `min` ein 1-aus-N-Glueckssample
//! und schwankt run-to-run ~3x — als Gate-Metrik unbrauchbar. Der p50 ist
//! dagegen empirisch sehr stabil (±0.5% ueber Laeufe bei 50k Samples), also
//! ist ein +20%-Gate praktisch false-red-frei und faengt jede reale
//! ≥20%-Median-Regression. `min`/`p90` werden als Info-Spalten BERICHTET
//! (Floor-/Tail-Trend sichtbar), aber NICHT gegated.
//!
//! Baselines sind host-spezifisch (p50 ~ Host-Scheduling) — sie werden auf dem
//! CI-Runner per `--update` geseedet, NICHT auf einer Dev-Maschine.
//!
//! ## Transport-Achse
//! Nur die ueber die c-api-FFI ERREICHBAREN, genuin distinkten Loopback-
//! Transporte: UDPv4, TCPv4, SHM (`same-host-shm` ist c-api-default). UDS ist
//! KEIN c-api-Feature (`ZERODDS_USER_TRANSPORT=UDS` faellt still auf UDPv4
//! zurueck) und TSN braucht ein echtes Interface — beide bewusst ausgelassen
//! (explizit geloggt, kein stiller Cap).
//!
//! ## Baselines
//! `tests/perf/internal-bench/baselines.txt` (committed, `<zelle> <min_us>
//! <p50_us>` pro Zeile). `--update` schreibt sie neu (bewusstes Re-Baseline
//! bei akzeptierter Aenderung). Ohne `--update`: exit≠0 (rot) wenn eine Zelle
//! den Baseline-Floor um > TOLERANCE ueberschreitet oder das absolute Sanity-
//! Ceiling reisst.
//!
//! Stage 2 (Haken): Security-Profil-Achse (none/data-enc/all-enc via
//! `roundtrip-1us`-Security-Env) als zusaetzliche Spalte — selber Orchestrator.

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

    /// Transport-Achse (Werte fuer `ZERODDS_USER_TRANSPORT`). Nur FFI-
    /// erreichbar + genuin distinkt: UDPv4, TCPv4, SHM. Siehe Modul-Doc.
    const TRANSPORTS: &[&str] = &["UDPv4", "TCPv4", "SHM"];
    /// Bewusst NICHT in der Matrix (kein stiller Cap — hier dokumentiert/geloggt).
    const EXCLUDED: &[(&str, &str)] = &[
        (
            "UDS",
            "kein c-api-Feature (same-host-uds) -> stiller UDPv4-Fallback",
        ),
        (
            "TSN",
            "braucht echtes Interface (ZERODDS_TSN_IFACE), kein Loopback",
        ),
        (
            "UDPv6/TCPv6",
            "redundant zu v4 fuer Floor-Latenz auf Loopback",
        ),
    ];
    /// Payload-Achse (Bytes). Klein = wakeup-dominiert, mittel = mit Copy-Anteil.
    const PAYLOADS: &[usize] = &[64, 4096];

    const WARMUP: u64 = 3_000;
    const SAMPLES: u64 = 30_000;
    /// +20% auf den p50 = rot.
    const TOLERANCE: f64 = 1.20;
    /// Absolutes Sanity-Ceiling (us) auf den p50: faengt grobe Kaputtheit auch
    /// ohne Baseline. Cross-Process-Loopback-p50 liegt real bei ~30-70us; 5ms
    /// ist grosszuegig (nur ~70-150x-Regressionen reissen es).
    const CEILING_US: f64 = 5_000.0;
    /// Wallclock-Notbremse pro `roundtrip-1us`-Prozess (CI-Safety-Net).
    const MAX_RUNTIME_SECS: u64 = 30;
    /// Wiederholungen pro Zelle; gegated wird der MEDIAN-p50. Auf den geteilten
    /// CI-Hosts trifft ab und zu eine ~3s-Contention-Spitze EINE Zelle (z.B.
    /// TCPv4/64 72->137us bei sonst stabilen ~80us). Der Median ueber 3 zeitlich
    /// verteilte Reps ignoriert einen einzelnen Spike -> robustes +20%-Gate.
    const REPS: usize = 3;

    const BASELINE_PATH: &str = "tests/perf/internal-bench/baselines.txt";

    struct Cell {
        key: String,
        min_us: f64,
        p50_us: f64,
        p90_us: f64,
        ok: bool,
    }

    /// Pfad zur `roundtrip-1us`-Bin neben der eigenen Bin (gleiches target-Dir).
    fn bench_bin() -> PathBuf {
        let exe = std::env::current_exe().expect("current_exe");
        exe.parent().expect("exe dir").join("roundtrip-1us")
    }

    /// Parst eine `report()`-Zeile der Form `min        = 17920 ns`.
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

    /// Eine Zelle messen: pong-Echo-Prozess spawnen, ping-Messer laufen lassen,
    /// dessen `min`/`p50`/`p90` (ns) parsen -> (min_us, p50_us, p90_us). `None`
    /// bei jedem Fehler (no-match, Bin fehlt, Parse-Fail) -> SETUP_FAIL (laut).
    fn measure(
        bin: &Path,
        transport: &str,
        payload: usize,
        domain: u32,
    ) -> Option<(f64, f64, f64)> {
        if !bin.exists() {
            eprintln!(
                "regression: roundtrip-1us nicht gefunden: {}",
                bin.display()
            );
            return None;
        }
        let topic = format!("bench-reg-{transport}-{payload}");
        let dom = domain.to_string();
        let pl = payload.to_string();
        let mr = MAX_RUNTIME_SECS.to_string();

        // pong (Echo) als Kind-Prozess.
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
            .env("ZERODDS_USER_TRANSPORT", transport)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        // Pong muss erst Discovery/Endpoints hochfahren, sonst verpasst ping
        // die ersten Samples (Sekunden-RTT-Artefakt).
        thread::sleep(Duration::from_secs(2));

        // ping (Messer, Listener-RTT) — blockiert bis samples voll oder
        // max-runtime; sammelt stdout fuer den min/p50-Parse.
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
            .env("ZERODDS_USER_TRANSPORT", transport)
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

    /// Median eines nicht-leeren f64-Slices (sortiert intern; bei gerader
    /// Laenge der untere der beiden Mittelwerte — konservativ).
    fn median(v: &mut [f64]) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        v[(v.len() - 1) / 2]
    }

    /// Eine Zelle ueber `REPS` zeitlich verteilte Reps messen und aggregieren:
    /// MEDIAN-p50 (Spike-robust), MIN der mins (bester Floor), MEDIAN-p90.
    /// `base_domain` + Rep-Index liefern pro Rep eine disjunkte Domain (kein
    /// TIME_WAIT/Discovery-Bleed zwischen Back-to-Back-Reps). `None` wenn die
    /// Mehrheit der Reps scheitert (Setup kaputt) -> Zelle = SETUP_FAIL.
    fn measure_cell(
        bin: &Path,
        transport: &str,
        payload: usize,
        base_domain: u32,
    ) -> Option<(f64, f64, f64)> {
        let mut mins = Vec::new();
        let mut p50s = Vec::new();
        let mut p90s = Vec::new();
        for rep in 0..REPS {
            // Disjunkte Domain pro Rep (Cap < 232).
            let domain = 140 + ((base_domain - 140 + rep as u32 * 7) % 80);
            if let Some((mn, p50, p90)) = measure(bin, transport, payload, domain) {
                mins.push(mn);
                p50s.push(p50);
                p90s.push(p90);
            }
            thread::sleep(Duration::from_millis(300));
        }
        // Mehrheit muss erfolgreich sein, sonst ist das Setup kaputt.
        if p50s.len() <= REPS / 2 {
            return None;
        }
        let min_us = mins.iter().copied().fold(f64::INFINITY, f64::min);
        let p50_us = median(&mut p50s);
        let p90_us = median(&mut p90s);
        Some((min_us, p50_us, p90_us))
    }

    /// Baseline-Zeile: `<key> <p50_us> <min_us> <p90_us>`. Gegated wird p50
    /// (erste Spalte); min/p90 sind Info. Fehlende Info-Spalten = 0.0.
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
            println!("   (ausgelassen: {t} — {why})");
        }
        println!(
            "{:<16} {:>9} {:>9} {:>9} {:>10} {:>16}",
            "cell", "min_us", "p50_us", "p90_us", "base_p50", "status"
        );

        let mut cells: Vec<Cell> = Vec::new();
        let mut idx: u32 = 0;
        for &t in TRANSPORTS {
            for &pl in PAYLOADS {
                let key = format!("{t}/{pl}");
                // Disjunkte Basis-Domain pro Zelle (kein Discovery-Bleed). Die
                // Reps f*chern davon ab. Cap < 232.
                let domain = 140 + ((idx * REPS as u32) % 60);
                idx += 1;
                // Baseline = p50 (erste Spalte der Baseline-Zeile).
                let base_p50 = baselines.get(&key).map(|&(p50, ..)| p50);
                let (min_us, p50_us, p90_us, status, ok) = match measure_cell(&bin, t, pl, domain) {
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
                // Settle (TIME_WAIT / Discovery-Cleanup zwischen Zellen).
                thread::sleep(Duration::from_millis(500));
            }
        }

        if update {
            let mut out = String::from(
                "# Internal-Bench-Baselines (committed). Spalten: <transport/payload> <p50_us> <min_us> <p90_us>\n\
                 # Metrik: Cross-Process-DCPS-Roundtrip-RTT (Listener). Gegated wird p50.\n\
                 # HOST-SPEZIFISCH: auf dem CI-Runner per --update geseedet, nicht auf Dev-Maschinen.\n\
                 # Regeneriert via: cargo run -p zerodds-bench-suite --bin regression --release -- --update\n\
                 # Gate (ohne --update): p50 > base_p50 * 1.20 ODER > 5000us => rot.\n",
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
            println!(
                "\n== baselines aktualisiert: {} ==",
                baseline_path.display()
            );
            // Bei --update trotzdem rot, wenn eine Zelle gar nicht messbar war
            // (eine NAN-Baseline waere wertlos).
            return if cells.iter().any(|c| !c.p50_us.is_finite()) {
                eprintln!("WARN: Zelle(n) nicht messbar — Baseline unvollstaendig");
                1
            } else {
                0
            };
        }

        let failed: Vec<&Cell> = cells.iter().filter(|c| !c.ok).collect();
        if failed.is_empty() {
            println!("\n== alle {} Zellen OK ==", cells.len());
            0
        } else {
            println!("\n== {} Zelle(n) ROT ==", failed.len());
            for c in &failed {
                println!("   {}", c.key);
            }
            1
        }
    }
}
