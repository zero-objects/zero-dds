// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! QoS Compatibility Matrix Tool.
//!
//! Laesst alle Kombinationen der zentralen Policy-Enums gegen
//! `zerodds_qos::check_compatibility` laufen und gibt eine Tabelle in
//! Markdown aus. Hilft bei:
//!
//! * Sanity-Check vs. OMG DDS 1.4 §2.2.3 Table "QoS compatibility".
//! * Dokumentations-Update bei Policy-Aenderungen (`--output docs/...`).
//! * Regression-Erkennung wenn eine `is_compatible_with`-Aenderung
//!   das Matrix-Muster stillschweigend modifiziert.
//!
//! Ausgabe-Formate:
//! * `markdown` (default) — Tabelle stdout.
//! * `csv` — `writer,reader,compatible,reasons` pro Zeile.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;

use zerodds_qos::CompatibilityResult;
use zerodds_qos::duration::Duration;
use zerodds_qos::policies::{
    DurabilityKind, DurabilityQosPolicy, OwnershipKind, OwnershipQosPolicy, ReaderQos,
    ReliabilityKind, ReliabilityQosPolicy, WriterQos, check_compatibility,
};

#[derive(Debug, Clone, Copy)]
enum Format {
    Markdown,
    Csv,
}

fn parse_args() -> Result<Format, String> {
    let mut fmt = Format::Markdown;
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--format" => {
                let v = args
                    .next()
                    .ok_or_else(|| "missing value after --format".to_owned())?;
                fmt = match v.as_str() {
                    "markdown" | "md" => Format::Markdown,
                    "csv" => Format::Csv,
                    other => return Err(format!("unbekanntes Format: {other}")),
                };
            }
            "-h" | "--help" => {
                println!("usage: qos-matrix [--format markdown|csv]");
                std::process::exit(0);
            }
            other => return Err(format!("unbekanntes Arg: {other}")),
        }
    }
    Ok(fmt)
}

/// Ein Cell-Eintrag der Matrix.
struct Cell {
    writer_label: String,
    reader_label: String,
    compatible: bool,
    reasons: Vec<String>,
}

fn build_writer(dur: DurabilityKind, rel: ReliabilityKind, own: OwnershipKind) -> WriterQos {
    WriterQos {
        durability: DurabilityQosPolicy { kind: dur },
        reliability: ReliabilityQosPolicy {
            kind: rel,
            max_blocking_time: Duration::from_millis(100),
        },
        ownership: OwnershipQosPolicy { kind: own },
        ..WriterQos::default()
    }
}

fn build_reader(dur: DurabilityKind, rel: ReliabilityKind, own: OwnershipKind) -> ReaderQos {
    ReaderQos {
        durability: DurabilityQosPolicy { kind: dur },
        reliability: ReliabilityQosPolicy {
            kind: rel,
            max_blocking_time: Duration::from_millis(100),
        },
        ownership: OwnershipQosPolicy { kind: own },
        ..ReaderQos::default()
    }
}

fn label(dur: DurabilityKind, rel: ReliabilityKind, own: OwnershipKind) -> String {
    format!("{dur:?}/{rel:?}/{own:?}")
}

fn compute_cells() -> Vec<Cell> {
    let durabilities = [DurabilityKind::Volatile, DurabilityKind::TransientLocal];
    let reliabilities = [ReliabilityKind::BestEffort, ReliabilityKind::Reliable];
    let ownerships = [OwnershipKind::Shared, OwnershipKind::Exclusive];

    let mut out = Vec::new();
    for wd in durabilities {
        for wr in reliabilities {
            for wo in ownerships {
                let w = build_writer(wd, wr, wo);
                for rd in durabilities {
                    for rr in reliabilities {
                        for ro in ownerships {
                            let r = build_reader(rd, rr, ro);
                            let res = check_compatibility(&w, &r);
                            let reasons: Vec<String> = match &res {
                                CompatibilityResult::Compatible => Vec::new(),
                                CompatibilityResult::Incompatible(rs) => {
                                    rs.iter().map(|r| format!("{r:?}")).collect()
                                }
                            };
                            out.push(Cell {
                                writer_label: label(wd, wr, wo),
                                reader_label: label(rd, rr, ro),
                                compatible: res.is_compatible(),
                                reasons,
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

fn print_markdown(cells: &[Cell]) {
    // Writer x Reader: Zeilen = Writer, Spalten = Reader.
    // Erst Set von unique Writer- + Reader-Labels.
    let mut writers: Vec<&str> = cells.iter().map(|c| c.writer_label.as_str()).collect();
    writers.dedup();
    let mut readers: Vec<&str> = cells.iter().map(|c| c.reader_label.as_str()).collect();
    readers.sort_unstable();
    readers.dedup();

    let total = cells.len();
    let ok = cells.iter().filter(|c| c.compatible).count();
    println!("# QoS Compatibility Matrix");
    println!();
    println!("**Erzeugt von `qos-matrix`** — Durability × Reliability × Ownership.");
    println!();
    println!("* Kompatible Kombinationen: `{ok}` / `{total}`");
    println!("* `✓` = `check_compatibility().is_compatible() == true`.");
    println!("* `✗` = inkompatibel; Details im Anhang.");
    println!();

    print!("| Writer \\ Reader |");
    for r in &readers {
        print!(" {r} |");
    }
    println!();
    print!("|---|");
    for _ in &readers {
        print!("---|");
    }
    println!();

    let mut seen_writers = std::collections::BTreeSet::new();
    for w in &writers {
        if !seen_writers.insert(*w) {
            continue;
        }
        print!("| {w} |");
        for r in &readers {
            let cell = cells
                .iter()
                .find(|c| c.writer_label == *w && c.reader_label == *r);
            match cell {
                Some(c) if c.compatible => print!(" ✓ |"),
                Some(_) => print!(" ✗ |"),
                None => print!(" ? |"),
            }
        }
        println!();
    }

    // Anhang: Inkompatible Kombos mit Reasons.
    println!();
    println!("## Inkompatibilitaeten (Details)");
    println!();
    for c in cells.iter().filter(|c| !c.compatible) {
        println!(
            "* `{} → {}`: {}",
            c.writer_label,
            c.reader_label,
            c.reasons.join(", "),
        );
    }
}

fn print_csv(cells: &[Cell]) {
    println!("writer,reader,compatible,reasons");
    for c in cells {
        println!(
            "\"{}\",\"{}\",{},\"{}\"",
            c.writer_label,
            c.reader_label,
            c.compatible,
            c.reasons.join(";"),
        );
    }
}

fn main() -> ExitCode {
    let fmt = match parse_args() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("qos-matrix: {e}");
            return ExitCode::from(2);
        }
    };
    let cells = compute_cells();
    match fmt {
        Format::Markdown => print_markdown(&cells),
        Format::Csv => print_csv(&cells),
    }
    ExitCode::SUCCESS
}
