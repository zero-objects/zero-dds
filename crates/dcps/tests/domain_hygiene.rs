//! No test may bind a DDS domain the kernel can take away from it.
//!
//! # The defect this exists to catch
//!
//! A DDS domain number *is* a UDP port number: RTPS §9.6.1.4.1 puts a
//! participant's SPDP multicast socket on `7400 + 250*domain` and its
//! SPDP unicast socket on `7400 + 250*domain + 10 + 2*index`. Domains
//! from roughly 102 upwards therefore land inside the range the Linux
//! kernel hands out to sockets that asked for "any port"
//! (`net.ipv4.ip_local_port_range`, `32768 60999` on the bench host).
//!
//! A socket the kernel picked from that range carries neither
//! `SO_REUSEADDR` nor `SO_REUSEPORT`, so a participant binding its SPDP
//! port afterwards gets `EADDRINUSE` and
//! `TransportError { label: "spdp multicast bind" }`. Measured: a plain
//! `bind(0.0.0.0, 0)` took port 51417, after which the SPDP recipe
//! failed on that port. The other party in the collision is usually not
//! another DDS stack — it is the ephemeral user-data socket of one of
//! ZeroDDS' own participants in another test binary of the same run.
//! That is why the whole class looked like "only a reduced number of
//! tests work at the same time": more binaries, more ephemeral sockets,
//! more collisions, and none of it flaky in the usual sense.
//!
//! # The two rules
//!
//! 1. A test that binds sockets on a **literal** domain may only use one
//!    of [`common::RESERVED_DOMAINS`], so that
//!    [`common::unique_domain`]'s lease band can never hand the same
//!    number to a second test.
//! 2. Every reserved domain's whole port window must end below `32768`,
//!    the lower bound of the ephemeral range on both platforms this
//!    workspace is tested on (Linux `32768..60999`, macOS
//!    `49152..65535`). Domains above the range exist and the lease band
//!    uses them, but a *fixed* domain has to be safe everywhere.
//!
//! Anything that needs neither is what [`common::unique_domain`] is for.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used,
    missing_docs
)]

use std::path::{Path, PathBuf};

/// The lowest ephemeral port any platform in this workspace's test
/// matrix hands out. Linux starts at `32768`, macOS at `49152`.
const LOWEST_EPHEMERAL_PORT: u32 = 32768;

/// The calls that make a participant bind real UDP sockets.
///
/// `create_participant_offline` is deliberately absent: it opens no
/// socket, so its domain argument is a label and not a port.
const BINDING_CALLS: &[&str] = &[
    "create_participant(",
    "create_participant_with_config(",
    "DcpsRuntime::start(",
    // The C entry points go through the same online factory.
    "zerodds_runtime_create(",
    "zerodds_dpf_create_participant(",
];

/// What a domain must never be computed from.
///
/// `base + std::process::id() % N` was in twelve places and is the same
/// defect in a different shape: it spreads the domain over a window
/// nobody checked, and above `101` every value in that window is a port
/// the kernel also hands out. Measured: 11 of 90 parallel runs of the
/// `zerodds-c-api` test binary died on domains 124, 128 and 166 with
/// `TransportError { label: "spdp multicast bind" }`, once as a SIGSEGV
/// because the NULL handle went unchecked. A domain is either a reserved
/// number or a lease; it is never arithmetic.
const FORBIDDEN_IN_A_DOMAIN: &[&str] = &["process::id()", "fetch_add", "atomic"];

/// Parameter names a test helper gives the domain it forwards.
///
/// Most of the domains this test found on its first run were not at a
/// binding call at all — they were literals handed to a local helper
/// (`two_runtimes(141)`, `runtime_with_random_port(101, 1)`) that passes
/// the value on. A rule that only reads the binding calls misses exactly
/// the files that factored theirs out.
const DOMAIN_PARAMETER_NAMES: &[&str] = &["domain", "domain_id", "dom"];

#[test]
fn every_reserved_domain_stays_below_the_ephemeral_range() {
    let mut offenders = Vec::new();
    for &domain in common::RESERVED_DOMAINS {
        let (low, high) = common::domain_port_window(domain);
        if high >= LOWEST_EPHEMERAL_PORT {
            offenders.push(format!("domain {domain} occupies ports {low}..{high}"));
        }
    }
    assert!(
        offenders.is_empty(),
        "a fixed domain has to be safe on every platform, so its RTPS port \
         window must end below {LOWEST_EPHEMERAL_PORT} — the lowest port the \
         kernel hands out to a socket that asked for any port. These do not:\n  \
         {}\n\
         Either move the test that needs the domain to \
         `common::unique_domain()`, which leases from a band chosen against \
         the running kernel's range, or pick a number below \
         {}.",
        offenders.join("\n  "),
        (LOWEST_EPHEMERAL_PORT - 7400 - 248) / 250,
    );
}

#[test]
fn no_test_binds_a_literal_domain_the_lease_band_can_hand_out() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    let mut checked = 0usize;
    for file in rust_sources(&root.join("crates")) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (domain, line) in literal_domains(&text) {
            checked += 1;
            // Above `MAX_DOMAIN` the lease band cannot produce the number
            // at all, so there is nothing to collide with. The tests that
            // sit there are the ones about the u16 port ceiling itself
            // (`zerodds-c-api` on 232, 233 and 250).
            if domain > common::MAX_DOMAIN {
                continue;
            }
            if !common::RESERVED_DOMAINS.contains(&domain) {
                offenders.push(format!(
                    "{}:{line} binds domain {domain}",
                    file.strip_prefix(&root).unwrap_or(&file).display()
                ));
            }
        }
    }
    assert!(
        checked > 0,
        "the scan found no socket-binding call at all — the patterns in \
         BINDING_CALLS no longer match the code they are supposed to watch"
    );
    assert!(
        offenders.is_empty(),
        "these bind a literal DDS domain that `common::unique_domain()` can \
         lease to another test at the same time; two participants on one \
         domain share a port, and a socket bound to 0.0.0.0:port receives \
         every multicast group on that port, so they see each other's \
         discovery:\n  {}\n\
         Use `common::unique_domain()`, or add the number to \
         `common::RESERVED_DOMAINS` if the domain has to stay fixed.",
        offenders.join("\n  ")
    );
}

#[test]
fn no_domain_is_computed_from_a_process_id_or_a_counter() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    for file in rust_sources(&root.join("crates")) {
        // This file names the patterns it looks for.
        if file.ends_with("domain_hygiene.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            // Case folded: a counter behind a helper is spelled
            // `static NEXT_DOMAIN: AtomicI32` and `NEXT_DOMAIN.fetch_add(..)`,
            // which a lower-case match read past.
            let folded = line.to_lowercase();
            let Some(rest) = folded.split_once("domain") else {
                continue;
            };
            // `let domain: u32 = ...`, `let dom = ...`, `domain_id: ...`
            if !rest.1.contains('=') && !rest.1.contains(':') {
                continue;
            }
            if FORBIDDEN_IN_A_DOMAIN
                .iter()
                .any(|pattern| rest.1.contains(pattern))
            {
                offenders.push(format!(
                    "{}:{} {}",
                    file.strip_prefix(&root).unwrap_or(&file).display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a domain computed from a PID or a counter walks over a window \
         nobody checked, and above domain 101 every value in that window is \
         a port the kernel hands out to any socket that asked for one:\n  {}\n\
         Take a reserved number, or `common::unique_domain()`.",
        offenders.join("\n  ")
    );
}

/// Every domain passed as a decimal literal to a socket-binding call or
/// to a helper in the same file that forwards one, with its line.
///
/// The argument may sit on the call's line or on the next one —
/// `rustfmt` breaks the call as soon as it grows — so both are read.
///
/// What this does **not** see: a domain that reaches a binding call
/// through two hops, through a `const`, or through a helper whose
/// parameter is named something else. The rule it enforces is a floor,
/// not a proof.
fn literal_domains(text: &str) -> Vec<(i32, usize)> {
    let mut calls: Vec<(String, usize)> = BINDING_CALLS
        .iter()
        .map(|call| ((*call).to_string(), 0))
        .collect();
    calls.extend(domain_forwarding_helpers(text));
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for index in 0..lines.len() {
        for (call, position) in &calls {
            // `create_participant(` is a substring of
            // `create_participant_with_config(`; the longer one is
            // matched on its own pass, so skip the short match here.
            if call == "create_participant(" {
                let line = lines[index];
                if let Some(at) = line.find(call.as_str()) {
                    if line[..at + call.len()].ends_with("create_participant_with_config(") {
                        continue;
                    }
                }
            }
            if let Some(domain) = literal_at_position(&lines, index, call, *position) {
                found.push((domain, index + 1));
            }
        }
    }
    found
}

/// The argument at `position` of `call`, if it is a decimal literal.
///
/// Arguments are read from the call's own line and from the lines after
/// it, because `rustfmt` puts each one on its own line as soon as the
/// call grows. Nesting is tracked so that a comma inside a nested call
/// does not count as an argument separator.
fn literal_at_position(lines: &[&str], index: usize, call: &str, position: usize) -> Option<i32> {
    let line = lines[index];
    let at = line.find(call)?;
    // A call is only a call if what precedes the name is not itself part
    // of an identifier (`start(` inside `restart(`).
    if line[..at]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
        && !call.contains("::")
    {
        return None;
    }
    let mut rest = String::from(&line[at + call.len()..]);
    for follower in lines.iter().skip(index + 1).take(12) {
        if rest.chars().filter(|c| *c == ')').count() > rest.chars().filter(|c| *c == '(').count() {
            break;
        }
        rest.push('\n');
        rest.push_str(follower);
    }
    let argument = split_arguments(&rest).into_iter().nth(position)?;
    let argument = argument.trim();
    let digits: String = argument.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() != argument.len() {
        // A number with anything after it (`0xFF`, `1_000`, `12u8`) is
        // not the plain domain literal this rule is about.
        return None;
    }
    digits.parse::<i32>().ok()
}

/// Splits an argument list at top-level commas, up to its closing paren.
fn split_arguments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' if depth == 0 => {
                out.push(current);
                return out;
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    out.push(current);
    out
}

/// Helpers defined in this file that take a domain and pass it on, as
/// `("name(", argument index)`.
fn domain_forwarding_helpers(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("fn ") else {
            continue;
        };
        let Some(paren) = rest.find('(') else {
            continue;
        };
        let name = rest[..paren].trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        // The signature may wrap; take enough lines to hold it, and
        // enough more to see whether the body binds anything. A helper
        // that ends in `create_participant_offline` forwards a label,
        // not a port, and is not this rule's business.
        let window: String = text
            .lines()
            .skip(index)
            .take(60)
            .collect::<Vec<_>>()
            .join("\n");
        if !BINDING_CALLS.iter().any(|call| {
            window.match_indices(call).any(|(at, _)| {
                *call != "create_participant("
                    || !window[..at + call.len()].ends_with("create_participant_with_config(")
            })
        }) {
            continue;
        }
        let signature: String = text
            .lines()
            .skip(index)
            .take(12)
            .collect::<Vec<_>>()
            .join("\n");
        let Some(after) = signature.find('(') else {
            continue;
        };
        for (position, parameter) in split_arguments(&signature[after + 1..]).iter().enumerate() {
            let Some((parameter_name, kind)) = parameter.split_once(':') else {
                continue;
            };
            if DOMAIN_PARAMETER_NAMES.contains(&parameter_name.trim())
                && matches!(kind.trim(), "i32" | "u32")
            {
                out.push((format!("{name}("), position));
            }
        }
    }
    out
}

fn rust_sources(directory: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        // `target` holds build output, `vendor` holds other people's code.
        if name == "target" || name == "vendor" || name == ".git" {
            continue;
        }
        if path.is_dir() {
            out.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    out
}

fn workspace_root() -> PathBuf {
    // crates/dcps -> crates -> workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/dcps has two ancestors")
        .to_path_buf()
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
