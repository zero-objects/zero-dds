// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-admin` — the ZeroDDS operator CLI: load configs, look into running
//! systems, understand and analyze them.
//!
//! Subcommand groups (noun-verb):
//!
//! ```text
//! zerodds-admin domain inspect <id>        live: participants + their endpoints in a domain
//! zerodds-admin discovery snapshot <id>    live: raw SPDP/SEDP snapshot + counts
//! zerodds-admin config inspect <file.xml>  offline: load a DDS-XML deployment, domain-id topology
//! zerodds-admin qos validate <file.xml>    offline: DDS-XML well-formedness + library parse
//! zerodds-admin qos check <file.xml>       offline: writer/reader RxO compatibility (DDS 1.4 §2.2.3)
//! ```
//!
//! The `domain` / `discovery` groups join the domain over RTPS; the `config` /
//! `qos` groups are fully offline. Every data-bearing command accepts `--json`.

#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI-Tool: stdout/stderr sind die Ausgabe.

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zerodds_cli_common::{install_signal_handler, stable_prefix};
use zerodds_dcps::runtime::{DcpsRuntime, DiscoveredEndpointInfo, RuntimeConfig};
use zerodds_qos::{ReaderQos, WriterQos, check_compatibility};
use zerodds_xml::{
    QosProfileRegistry, XmlError, parse_domain_libraries, parse_domain_participant_libraries,
    parser::parse_xml_tree,
};

const MARKER_ADMIN: u8 = 0xAD;
const DEFAULT_PROBE_MS: u64 = 2000;

const USAGE: &str = "\
zerodds-admin — ZeroDDS operator CLI: load configs, inspect, understand, analyze

USAGE:
    zerodds-admin <group> <verb> [args] [--json]

LIVE (joins the domain over RTPS):
    domain inspect <id>        Participants + their writer/reader endpoints (topic,
                               type, reliability, durability) in a running domain
    discovery snapshot <id>    Raw SPDP/SEDP snapshot: participant list + pub/sub counts

OFFLINE (DDS-XML, no network):
    config inspect <file.xml>  Load a deployment, domain-id-centric topology
    qos validate <file.xml>    DDS-XML well-formedness + parse every library
    qos check <file.xml>       Writer/reader RxO compatibility; exit 1 on any mismatch

OPTIONS:
    --json                     Machine-readable output (data-bearing commands)
    --timeout-ms <T>           Live-probe observation window (default 2000)
    -h, --help                 Show this usage

EXIT CODES:
    0  ok / all compatible      1  validation failure / probe error      2  bad invocation";

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") || raw.is_empty() {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    // Split flags from positionals.
    let mut json = false;
    let mut timeout_ms = DEFAULT_PROBE_MS;
    let mut pos: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--json" => json = true,
            "--timeout-ms" => {
                i += 1;
                match raw.get(i).and_then(|v| v.parse().ok()) {
                    Some(v) => timeout_ms = v,
                    None => return bad("--timeout-ms needs a number"),
                }
            }
            flag if flag.starts_with('-') => return bad(&format!("unknown flag `{flag}`")),
            other => pos.push(other.to_string()),
        }
        i += 1;
    }

    let group = pos.first().map(String::as_str).unwrap_or("");
    let verb = pos.get(1).map(String::as_str).unwrap_or("");
    let arg = pos.get(2).map(String::as_str);

    match (group, verb) {
        ("domain", "inspect") => match arg {
            Some(a) => cmd_domain_inspect(a, timeout_ms, json),
            None => bad("`domain inspect` needs a <domain-id>"),
        },
        ("discovery", "snapshot") => match arg {
            Some(a) => cmd_discovery_snapshot(a, timeout_ms, json),
            None => bad("`discovery snapshot` needs a <domain-id>"),
        },
        ("config", "inspect") => {
            offline(arg, "config inspect", |xml| cmd_config_inspect(xml, json))
        }
        ("qos", "validate") => offline(arg, "qos validate", cmd_qos_validate),
        ("qos", "check") => offline(arg, "qos check", |xml| cmd_qos_check(xml, json)),
        _ => bad(&format!("unknown command `{group} {verb}`")),
    }
}

fn bad(msg: &str) -> ExitCode {
    eprintln!("zerodds-admin: {msg}\n\n{USAGE}");
    ExitCode::from(2)
}

/// Driver for offline `<file.xml>` commands. The closure returns the process
/// exit code (0 ok, 1 validation failure).
fn offline(arg: Option<&str>, cmd: &str, f: impl Fn(&str) -> Result<u8, XmlError>) -> ExitCode {
    let Some(path) = arg else {
        return bad(&format!("`{cmd}` needs a <file.xml> argument"));
    };
    let xml = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zerodds-admin: cannot read `{path}`: {e}");
            return ExitCode::FAILURE;
        }
    };
    match f(&xml) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("zerodds-admin: {cmd}: {e}");
            ExitCode::FAILURE
        }
    }
}

// ============================================================================
// Live probe (shared by `domain inspect` and `discovery snapshot`)
// ============================================================================

/// Start a runtime on `domain`, observe for `timeout_ms`, then run `f` over the
/// settled runtime. Responds to Ctrl-C promptly.
fn with_probe(
    domain_arg: &str,
    timeout_ms: u64,
    f: impl FnOnce(&DcpsRuntime) -> ExitCode,
) -> ExitCode {
    let domain: i32 = match domain_arg.parse() {
        Ok(v) => v,
        Err(_) => return bad(&format!("invalid domain-id `{domain_arg}`")),
    };
    let prefix = stable_prefix(MARKER_ADMIN);
    let runtime = match DcpsRuntime::start(domain, prefix, RuntimeConfig::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("zerodds-admin: DcpsRuntime::start failed: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(Arc::clone(&stop));
    eprintln!("zerodds-admin: probing domain {domain} for {timeout_ms}ms");
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    let code = f(&runtime);
    drop(runtime);
    code
}

fn hex_prefix(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `domain inspect <id>` — live participants grouped with their endpoints.
fn cmd_domain_inspect(domain_arg: &str, timeout_ms: u64, json: bool) -> ExitCode {
    with_probe(domain_arg, timeout_ms, |rt| {
        let participants = rt.discovered_participants();
        let pubs = rt.discovered_publication_endpoints();
        let subs = rt.discovered_subscription_endpoints();

        // Group endpoints by owning participant prefix (first 12 GUID bytes).
        let mut writers: BTreeMap<[u8; 12], Vec<&DiscoveredEndpointInfo>> = BTreeMap::new();
        let mut readers: BTreeMap<[u8; 12], Vec<&DiscoveredEndpointInfo>> = BTreeMap::new();
        for e in &pubs {
            writers.entry(prefix12(e)).or_default().push(e);
        }
        for e in &subs {
            readers.entry(prefix12(e)).or_default().push(e);
        }

        // Union of all participant prefixes seen.
        let mut prefixes: Vec<[u8; 12]> = participants.iter().map(|p| p.sender_prefix.0).collect();
        prefixes.extend(writers.keys().copied());
        prefixes.extend(readers.keys().copied());
        prefixes.sort_unstable();
        prefixes.dedup();

        if json {
            print!("{{\"domain\":{domain_arg},\"participants\":[");
            for (i, pfx) in prefixes.iter().enumerate() {
                if i > 0 {
                    print!(",");
                }
                print!("{{\"prefix\":\"{}\",\"writers\":[", hex_prefix(pfx));
                print_eps_json(writers.get(pfx).map(Vec::as_slice).unwrap_or_default());
                print!("],\"readers\":[");
                print_eps_json(readers.get(pfx).map(Vec::as_slice).unwrap_or_default());
                print!("]}}");
            }
            println!("]}}");
        } else {
            println!(
                "=== domain {domain_arg}: {} participant(s) ===",
                prefixes.len()
            );
            for pfx in &prefixes {
                println!("participant {}", hex_prefix(pfx));
                for e in writers.get(pfx).map(Vec::as_slice).unwrap_or_default() {
                    println!("  writer  {}", fmt_endpoint(e));
                }
                for e in readers.get(pfx).map(Vec::as_slice).unwrap_or_default() {
                    println!("  reader  {}", fmt_endpoint(e));
                }
            }
        }
        ExitCode::SUCCESS
    })
}

fn prefix12(e: &DiscoveredEndpointInfo) -> [u8; 12] {
    let mut p = [0u8; 12];
    p.copy_from_slice(&e.endpoint_guid[..12]);
    p
}

fn fmt_endpoint(e: &DiscoveredEndpointInfo) -> String {
    format!(
        "{} <{}> [{}, {}]",
        e.topic_name,
        e.type_name,
        if e.reliable {
            "RELIABLE"
        } else {
            "BEST_EFFORT"
        },
        if e.transient_local {
            "TRANSIENT_LOCAL"
        } else {
            "VOLATILE"
        },
    )
}

fn print_eps_json(eps: &[&DiscoveredEndpointInfo]) {
    for (i, e) in eps.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!(
            "{{\"topic\":{},\"type\":{},\"reliable\":{},\"transient_local\":{}}}",
            json_str(&e.topic_name),
            json_str(&e.type_name),
            e.reliable,
            e.transient_local,
        );
    }
}

/// `discovery snapshot <id>` — raw SPDP/SEDP participant list + counts.
fn cmd_discovery_snapshot(domain_arg: &str, timeout_ms: u64, json: bool) -> ExitCode {
    with_probe(domain_arg, timeout_ms, |rt| {
        let participants = rt.discovered_participants();
        let pubs = rt.discovered_publications_count();
        let subs = rt.discovered_subscriptions_count();
        if json {
            print!(
                "{{\"domain\":{domain_arg},\"participants\":{},\"publications\":{pubs},\"subscriptions\":{subs},\"prefixes\":[",
                participants.len()
            );
            for (i, p) in participants.iter().enumerate() {
                if i > 0 {
                    print!(",");
                }
                print!("{}", json_str(&hex_prefix(&p.sender_prefix.0)));
            }
            println!("]}}");
        } else {
            println!("=== discovery snapshot (domain {domain_arg}) ===");
            println!("participants:       {}", participants.len());
            println!("publications seen:  {pubs}");
            println!("subscriptions seen: {subs}");
            for p in &participants {
                println!(
                    "  · prefix={} vendor={:02x}{:02x}",
                    hex_prefix(&p.sender_prefix.0),
                    p.sender_vendor.0[0],
                    p.sender_vendor.0[1],
                );
            }
        }
        ExitCode::SUCCESS
    })
}

// ============================================================================
// config inspect — Domain-id-centric topology (offline)
// ============================================================================

struct Part {
    name: String,
    domain_ref: String,
    writers: Vec<(String, String)>, // (writer/reader name, topic)
    readers: Vec<(String, String)>,
}

fn cmd_config_inspect(xml: &str, json: bool) -> Result<u8, XmlError> {
    let mut by_ref: BTreeMap<String, u32> = BTreeMap::new();
    for lib in &parse_domain_libraries(xml)? {
        for d in &lib.domains {
            by_ref.insert(format!("{}::{}", lib.name, d.name), d.domain_id);
            by_ref.entry(d.name.clone()).or_insert(d.domain_id);
        }
    }

    // domain_id (None = unresolved) -> participant rows.
    let mut groups: BTreeMap<Option<u32>, Vec<Part>> = BTreeMap::new();
    for lib in &parse_domain_participant_libraries(xml)? {
        for p in &lib.participants {
            let id = by_ref.get(&p.domain_ref).copied();
            let writers = p
                .publishers
                .iter()
                .flat_map(|pubr| pubr.data_writers.iter())
                .map(|w| (w.name.clone(), w.topic_ref.clone()))
                .collect();
            let readers = p
                .subscribers
                .iter()
                .flat_map(|subr| subr.data_readers.iter())
                .map(|r| (r.name.clone(), r.topic_ref.clone()))
                .collect();
            groups.entry(id).or_default().push(Part {
                name: format!("{}::{}", lib.name, p.name),
                domain_ref: p.domain_ref.clone(),
                writers,
                readers,
            });
        }
    }

    if json {
        print!("[");
        let mut first = true;
        for (id, parts) in &groups {
            for p in parts {
                if !first {
                    print!(",");
                }
                first = false;
                print!(
                    "{{\"domain\":{},\"participant\":{},\"domain_ref\":{},\"writers\":[",
                    id.map_or_else(|| "null".to_string(), |n| n.to_string()),
                    json_str(&p.name),
                    json_str(&p.domain_ref),
                );
                print_pairs_json(&p.writers);
                print!("],\"readers\":[");
                print_pairs_json(&p.readers);
                print!("]}}");
            }
        }
        println!("]");
        return Ok(0);
    }

    if groups.is_empty() {
        println!("(no domain_participant definitions)");
        return Ok(0);
    }
    for (id, parts) in &groups {
        match id {
            Some(n) => println!("domain {n}"),
            None => println!("domain <unresolved domain_ref>"),
        }
        for p in parts {
            println!("  participant {} (domain_ref {})", p.name, p.domain_ref);
            for (w, t) in &p.writers {
                println!("    writer {w} -> topic {t}");
            }
            for (r, t) in &p.readers {
                println!("    reader {r} -> topic {t}");
            }
        }
    }
    Ok(0)
}

fn print_pairs_json(pairs: &[(String, String)]) {
    for (i, (name, topic)) in pairs.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!(
            "{{\"name\":{},\"topic\":{}}}",
            json_str(name),
            json_str(topic)
        );
    }
}

// ============================================================================
// qos validate — DDS-XML well-formedness + library parse (offline)
// ============================================================================

fn cmd_qos_validate(xml: &str) -> Result<u8, XmlError> {
    parse_xml_tree(xml)?;
    let domains = parse_domain_libraries(xml)?;
    let parts = parse_domain_participant_libraries(xml)?;
    let registry = QosProfileRegistry::from_xml(xml)?;
    let n_domains: usize = domains.iter().map(|l| l.domains.len()).sum();
    let n_parts: usize = parts.iter().map(|l| l.participants.len()).sum();
    println!(
        "valid: {} qos-lib, {} domain-lib ({n_domains} domains), {} participant-lib ({n_parts} participants)",
        registry.library_count(),
        domains.len(),
        parts.len(),
    );
    Ok(0)
}

// ============================================================================
// qos check — RxO compatibility (offline)
// ============================================================================

struct Mismatch {
    topic: String,
    writer: String,
    reader: String,
    reasons: String,
}

fn cmd_qos_check(xml: &str, json: bool) -> Result<u8, XmlError> {
    let registry = QosProfileRegistry::from_xml(xml)?;
    let mut writers: BTreeMap<String, Vec<(String, WriterQos)>> = BTreeMap::new();
    let mut readers: BTreeMap<String, Vec<(String, ReaderQos)>> = BTreeMap::new();

    for lib in &parse_domain_participant_libraries(xml)? {
        for p in &lib.participants {
            for pubr in &p.publishers {
                for w in &pubr.data_writers {
                    let q = match (&w.qos, &w.qos_profile_ref) {
                        (Some(q), _) => q.into_writer_qos(),
                        (None, Some(r)) => registry.writer_qos(r)?,
                        (None, None) => WriterQos::default(),
                    };
                    let label = format!("{}::{}::{}", p.name, pubr.name, w.name);
                    writers
                        .entry(w.topic_ref.clone())
                        .or_default()
                        .push((label, q));
                }
            }
            for subr in &p.subscribers {
                for r in &subr.data_readers {
                    let q = match (&r.qos, &r.qos_profile_ref) {
                        (Some(q), _) => q.into_reader_qos(),
                        (None, Some(pref)) => registry.reader_qos(pref)?,
                        (None, None) => ReaderQos::default(),
                    };
                    let label = format!("{}::{}::{}", p.name, subr.name, r.name);
                    readers
                        .entry(r.topic_ref.clone())
                        .or_default()
                        .push((label, q));
                }
            }
        }
    }

    let mut pairs = 0usize;
    let mut bad: Vec<Mismatch> = Vec::new();
    for (topic, ws) in &writers {
        let Some(rs) = readers.get(topic) else {
            continue;
        };
        for (wl, wq) in ws {
            for (rl, rq) in rs {
                pairs += 1;
                if let zerodds_qos::CompatibilityResult::Incompatible(reasons) =
                    check_compatibility(wq, rq)
                {
                    bad.push(Mismatch {
                        topic: topic.clone(),
                        writer: wl.clone(),
                        reader: rl.clone(),
                        reasons: reasons
                            .iter()
                            .map(|r| format!("{r:?}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    });
                }
            }
        }
    }

    if json {
        print!("{{\"pairs\":{pairs},\"incompatible\":[");
        for (i, m) in bad.iter().enumerate() {
            if i > 0 {
                print!(",");
            }
            print!(
                "{{\"topic\":{},\"writer\":{},\"reader\":{},\"reasons\":{}}}",
                json_str(&m.topic),
                json_str(&m.writer),
                json_str(&m.reader),
                json_str(&m.reasons),
            );
        }
        println!("]}}");
    } else {
        for m in &bad {
            println!(
                "INCOMPATIBLE topic {}: writer {} <-> reader {}: {}",
                m.topic, m.writer, m.reader, m.reasons
            );
        }
        if bad.is_empty() {
            println!("qos check: {pairs} writer/reader pair(s) checked, all compatible");
        } else {
            println!("qos check: {} of {pairs} pair(s) INCOMPATIBLE", bad.len());
        }
    }
    Ok(u8::from(!bad.is_empty()))
}

// ============================================================================
// Minimal JSON string encoder (no serde dependency, matches the house tools).
// ============================================================================

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Writer BestEffort vs Reader Reliable -> incompatible (Reliability).
    const QOS_BAD: &str = r#"<dds>
        <domain_participant_library name="DPL">
            <domain_participant name="P" domain_ref="d">
                <publisher name="Pub">
                    <data_writer name="W" topic_ref="T">
                        <datawriter_qos><reliability><kind>BEST_EFFORT</kind></reliability></datawriter_qos>
                    </data_writer>
                </publisher>
                <subscriber name="Sub">
                    <data_reader name="R" topic_ref="T">
                        <datareader_qos><reliability><kind>RELIABLE</kind></reliability></datareader_qos>
                    </data_reader>
                </subscriber>
            </domain_participant>
        </domain_participant_library>
    </dds>"#;

    const QOS_OK: &str = r#"<dds>
        <domain_participant_library name="DPL">
            <domain_participant name="P" domain_ref="d">
                <publisher name="Pub"><data_writer name="W" topic_ref="T"/></publisher>
                <subscriber name="Sub"><data_reader name="R" topic_ref="T"/></subscriber>
            </domain_participant>
        </domain_participant_library>
    </dds>"#;

    const DEPLOY: &str = r#"<dds>
        <domain_library name="DL">
            <domain name="D7" domain_id="7"/>
        </domain_library>
        <domain_participant_library name="DPL">
            <domain_participant name="P7" domain_ref="DL::D7">
                <publisher name="Pub"><data_writer name="W" topic_ref="T"/></publisher>
            </domain_participant>
        </domain_participant_library>
    </dds>"#;

    #[test]
    fn qos_check_flags_incompatible_reliability() {
        assert_eq!(cmd_qos_check(QOS_BAD, false).ok(), Some(1u8));
    }

    #[test]
    fn qos_check_passes_compatible_defaults() {
        assert_eq!(cmd_qos_check(QOS_OK, false).ok(), Some(0u8));
        assert_eq!(cmd_qos_check(QOS_OK, true).ok(), Some(0u8));
    }

    #[test]
    fn qos_validate_accepts_well_formed() {
        assert_eq!(cmd_qos_validate(DEPLOY).ok(), Some(0u8));
    }

    #[test]
    fn qos_validate_rejects_malformed() {
        assert!(cmd_qos_validate("<dds><domain_participant_library></dds>").is_err());
    }

    #[test]
    fn config_inspect_groups_by_domain_id() {
        assert_eq!(cmd_config_inspect(DEPLOY, false).ok(), Some(0u8));
        assert_eq!(cmd_config_inspect(DEPLOY, true).ok(), Some(0u8));
    }

    #[test]
    fn json_escapes_quotes() {
        assert_eq!(json_str("a\"b"), "\"a\\\"b\"");
    }
}
