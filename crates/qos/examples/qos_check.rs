//! C2: static QoS compatibility check as a CLI — "QoS mismatch loud +
//! ahead-of-time instead of a silent runtime failure". Surfaces
//! [`zerodds_qos::compatibility::compute_compatibility`] for
//! launch/CI validation: checks whether a writer QoS can serve a reader
//! QoS BEFORE the nodes run — and names EVERY incompatible policy.
//!
//! ```text
//! # qos_check <writer rel,dur> <reader rel,dur>
//! cargo run -p zerodds-qos --example qos_check -- besteffort,volatile reliable,transientlocal
//! #   → INCOMPATIBLE: Reliability, Durability
//! cargo run -p zerodds-qos --example qos_check -- reliable,transientlocal reliable,volatile
//! #   → COMPATIBLE
//! ```
//! Exit code 0 = compatible, 1 = incompatible, 2 = usage error.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use zerodds_qos::compatibility::compute_compatibility;
use zerodds_qos::{DurabilityKind, ReaderQos, ReliabilityKind, ReliabilityQosPolicy, WriterQos};

fn parse_rel(s: &str) -> Option<ReliabilityKind> {
    match s.to_ascii_lowercase().as_str() {
        "besteffort" | "be" => Some(ReliabilityKind::BestEffort),
        "reliable" | "r" => Some(ReliabilityKind::Reliable),
        _ => None,
    }
}

fn parse_dur(s: &str) -> Option<DurabilityKind> {
    match s.to_ascii_lowercase().as_str() {
        "volatile" | "v" => Some(DurabilityKind::Volatile),
        "transientlocal" | "tl" => Some(DurabilityKind::TransientLocal),
        "transient" | "t" => Some(DurabilityKind::Transient),
        "persistent" | "p" => Some(DurabilityKind::Persistent),
        _ => None,
    }
}

fn parse_pair(arg: &str) -> Option<(ReliabilityKind, DurabilityKind)> {
    let (rel, dur) = arg.split_once(',')?;
    Some((parse_rel(rel.trim())?, parse_dur(dur.trim())?))
}

fn usage() -> ! {
    eprintln!("usage: qos_check <writer rel,dur> <reader rel,dur>");
    eprintln!("  rel = besteffort|reliable   dur = volatile|transientlocal|transient|persistent");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        usage();
    }
    let Some((w_rel, w_dur)) = parse_pair(&args[0]) else {
        usage()
    };
    let Some((r_rel, r_dur)) = parse_pair(&args[1]) else {
        usage()
    };

    let mut w = WriterQos::default();
    w.reliability = ReliabilityQosPolicy {
        kind: w_rel,
        ..w.reliability
    };
    w.durability.kind = w_dur;

    let mut r = ReaderQos::default();
    r.reliability = ReliabilityQosPolicy {
        kind: r_rel,
        ..r.reliability
    };
    r.durability.kind = r_dur;

    println!("Writer offered: {w_rel:?}/{w_dur:?}   Reader requested: {r_rel:?}/{r_dur:?}");
    let result = compute_compatibility(&w, &r);
    if result.is_compatible() {
        println!("== COMPATIBLE ==");
    } else {
        let reasons: Vec<String> = match &result {
            zerodds_qos::CompatibilityResult::Incompatible(rs) => {
                rs.iter().map(|r| format!("{r:?}")).collect()
            }
            zerodds_qos::CompatibilityResult::Compatible => Vec::new(),
        };
        println!("== INCOMPATIBLE: {} ==", reasons.join(", "));
        std::process::exit(1);
    }
}
