//! No test may bind a DDS domain another run can be on at the same time.
//!
//! # The defects this exists to catch
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
//! A domain below the ephemeral range is not safe either when it is
//! fixed. The same test runs at the same time in the other slot of the
//! CI runner host, on the Linux test host and on the Windows runner, and
//! SPDP multicast crosses the LAN between them (`crates/test-domains`,
//! `BAND_VARIABLE`): two runs of one test on one fixed domain, with the
//! fixed GUID prefixes most of these tests use, are one domain with two
//! participants of each GUID. A lease is taken from the host's own share
//! of the band, so no other host and no other job holds it.
//!
//! # The rules
//!
//! 1. Code that binds sockets leases its domain (`common::unique_domain`,
//!    `common::lease_domain`) — tests, test modules and library code
//!    alike. A literal domain is found at the binding call, in a `let`,
//!    `const` or `static` it reaches the call through, and at the call
//!    of a helper in the same file that forwards it.
//! 2. A program a person starts by hand ([`PROGRAM_DIRECTORIES`]) and a
//!    documentation comment show the documented domains: a literal there
//!    is one of [`common::RESERVED_DOMAINS`], which the lease band never
//!    hands out.
//! 3. Every reserved domain's whole port window must end below `32768`,
//!    the lower bound of the ephemeral range on both platforms this
//!    workspace is tested on (Linux `32768..60999`, macOS
//!    `49152..65535`). Domains above the range exist and the lease band
//!    uses them, but a *fixed* domain has to be safe everywhere.

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

/// The calls that make a participant bind real UDP sockets, with the
/// position of the domain among their arguments.
///
/// `create_participant_offline` is deliberately absent: it opens no
/// socket, so its domain argument is a label and not a port.
const BINDING_CALLS: &[(&str, usize)] = &[
    ("create_participant(", 0),
    ("create_participant_with_config(", 0),
    ("DcpsRuntime::start(", 0),
    // The builder: `.build()` goes through the online factory.
    ("DomainParticipantFactory::create(", 0),
    // The C entry points go through the same online factory.
    ("zerodds_runtime_create(", 0),
    ("zerodds_runtime_create_from_config(", 0),
    ("zerodds_runtime_create_ros_defaults(", 0),
    ("zerodds_runtime_create_secure(", 0),
    ("zerodds_runtime_create_secure_from_env(", 0),
    // The factory comes first: `(factory, domain, qos)`.
    ("zerodds_dpf_create_participant(", 1),
];

/// What every entry of [`BINDING_CALLS`] contains one of.
const PREFILTER: &[&str] = &[
    "create_participant",
    "DcpsRuntime::start",
    "DomainParticipantFactory::create",
    "zerodds_runtime_create",
];

/// Directories whose programs a person starts by hand, on the domains the
/// documentation names: `examples/` of a crate, and the `proofs/` programs
/// that show one exchange with another vendor on its default domain.
const PROGRAM_DIRECTORIES: &[&str] = &["examples", "proofs"];

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
fn code_leases_the_domain_it_binds_and_an_example_names_a_reserved_one() {
    let root = workspace_root();
    let mut literal = Vec::new();
    let mut unreserved = Vec::new();
    let mut checked = 0usize;
    for (call, _) in BINDING_CALLS {
        assert!(
            PREFILTER.iter().any(|stem| call.contains(stem)),
            "`{call}` contains none of PREFILTER, so the files that use only \
             it would be read past"
        );
    }
    for (file, findings) in scan_in_parallel(rust_sources(&root.join("crates"))) {
        let relative = file.strip_prefix(&root).unwrap_or(&file);
        let program = relative.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| PROGRAM_DIRECTORIES.contains(&name))
        });
        for found in findings {
            checked += 1;
            // Above `MAX_DOMAIN` the port mapping refuses the number, so no
            // socket is bound. The tests that sit there are the ones about
            // the u16 port ceiling itself (`zerodds-c-api` on 232, 233 and
            // 250).
            if found.domain > common::MAX_DOMAIN {
                continue;
            }
            let at = format!(
                "{}:{} binds domain {}",
                relative.display(),
                found.line,
                found.domain
            );
            if program || found.in_comment {
                if !common::RESERVED_DOMAINS.contains(&found.domain) {
                    unreserved.push(at);
                }
            } else {
                literal.push(at);
            }
        }
    }
    assert!(
        checked > 0,
        "the scan found no socket-binding call at all — the patterns in \
         BINDING_CALLS no longer match the code they are supposed to watch"
    );
    assert!(
        literal.is_empty(),
        "these bind a literal DDS domain. The same test runs at the same \
         time in the other slot of the CI runner host, on the Linux test host \
         and on the Windows runner, SPDP multicast crosses the LAN between \
         them, and two runs on one fixed domain see each other's \
         participants:\n  {}\n\
         Lease the domain: `common::unique_domain()` in an integration test, \
         `zerodds_test_domains::lease_domain()` held for as long as the \
         participants live in a unit test.",
        literal.join("\n  ")
    );
    assert!(
        unreserved.is_empty(),
        "these programs or documentation comments bind a literal DDS domain \
         that `common::unique_domain()` can lease to a test at the same time; \
         two participants on one domain share a port, and a socket bound to \
         0.0.0.0:port receives every multicast group on that port, so they \
         see each other's discovery:\n  {}\n\
         Name one of `common::RESERVED_DOMAINS`.",
        unreserved.join("\n  ")
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

/// [`literal_domains`] of every file but this one, which names the calls it
/// looks for, on a thread per CPU (at most eight): the test is built without
/// optimisation, and one thread through the 47 MB of the workspace's Rust
/// took 3.2 s on the CI runner host (pipeline 9087).
fn scan_in_parallel(files: Vec<PathBuf>) -> Vec<(PathBuf, Vec<LiteralDomain>)> {
    let files: Vec<PathBuf> = files
        .into_iter()
        .filter(|file| !file.ends_with("domain_hygiene.rs"))
        .collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZero::get)
        .clamp(1, 8);
    let share = files.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = files
            .chunks(share)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|file| {
                            let text = std::fs::read_to_string(file).ok()?;
                            Some((file.clone(), literal_domains(&text)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a scan thread"))
            .collect()
    })
}

/// One literal domain at a socket-binding call.
struct LiteralDomain {
    domain: i32,
    /// The line of the call, from 1.
    line: usize,
    /// The call is in a comment: documentation, not code.
    in_comment: bool,
}

/// Every domain passed as a decimal literal to a socket-binding call or
/// to a helper in the same file that forwards one, with its line: the
/// literal at the call, or the literal a `let` of the same function or a
/// `const` or `static` of the file binds the argument's name to.
///
/// The argument may sit on the call's line or on the lines after it —
/// `rustfmt` breaks the call as soon as it grows — so they are read.
///
/// What this does **not** see: a domain that reaches a binding call
/// through two hops, through a struct field or a function's return value,
/// or through a helper whose parameter is named something else. The rule
/// it enforces is a floor, not a proof.
fn literal_domains(text: &str) -> Vec<LiteralDomain> {
    // A file that names no binding call has no helper that forwards to one
    // either: most of the workspace, read past here. Four searches instead
    // of one per call: every call contains one of these.
    if !PREFILTER.iter().any(|stem| text.contains(stem)) {
        return Vec::new();
    }
    let mut calls: Vec<(String, usize)> = BINDING_CALLS
        .iter()
        .map(|(call, position)| ((*call).to_string(), *position))
        .collect();
    calls.extend(domain_forwarding_helpers(text));
    let lines: Vec<&str> = text.lines().collect();
    let items = item_bindings(&lines);
    let mut found = Vec::new();
    for index in 0..lines.len() {
        for (call, position) in &calls {
            // `create_participant(` is a substring of
            // `create_participant_with_config(`; the longer one is
            // matched on its own pass, so skip the short match here.
            if call == "create_participant(" {
                let line = lines[index];
                if let Some(at) = line.find(call.as_str())
                    && line[..at + call.len()].ends_with("create_participant_with_config(")
                {
                    continue;
                }
            }
            if let Some(domain) = literal_at_position(&lines, &items, index, call, *position) {
                found.push(LiteralDomain {
                    domain,
                    line: index + 1,
                    in_comment: lines[index].trim_start().starts_with("//"),
                });
            }
        }
    }
    found
}

/// The argument at `position` of `call`, if it is a decimal literal or a
/// name bound to one (see [`bound_literal`]).
///
/// Arguments are read from the call's own line and from the lines after
/// it, because `rustfmt` puts each one on its own line as soon as the
/// call grows. Nesting is tracked so that a comma inside a nested call
/// does not count as an argument separator.
fn literal_at_position(
    lines: &[&str],
    items: &std::collections::HashMap<&str, Option<i32>>,
    index: usize,
    call: &str,
    position: usize,
) -> Option<i32> {
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
    // `domain as u32` is the domain.
    let argument = argument
        .trim()
        .split_once(" as ")
        .map_or(argument.trim(), |(value, _)| value.trim());
    if let Some(domain) = decimal(argument) {
        return Some(domain);
    }
    if !argument.is_empty()
        && argument
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return bound_literal(lines, items, index, argument);
    }
    None
}

/// `text` as a plain decimal literal. A number with anything after it
/// (`0xFF`, `1_000`, `12u8`) is not the domain literal this rule is
/// about.
fn decimal(text: &str) -> Option<i32> {
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse::<i32>().ok()
}

/// The literal `name` is bound to where it is used on line `index`: a
/// `let` above it in the same function, or else a `const` or `static` of
/// the file (`items`). `None` when the nearest binding is anything but a
/// literal, or there is none.
fn bound_literal(
    lines: &[&str],
    items: &std::collections::HashMap<&str, Option<i32>>,
    index: usize,
    name: &str,
) -> Option<i32> {
    for line in lines[..index].iter().rev() {
        if let Some(value) = binding_of(line, name, &["let"]) {
            return value;
        }
        if starts_a_function(line) {
            break;
        }
    }
    items.get(name).copied().flatten()
}

/// Every `const` and `static` of a file by name, with the literal it is
/// bound to (`None` for anything else); the first of a name counts.
fn item_bindings<'a>(lines: &[&'a str]) -> std::collections::HashMap<&'a str, Option<i32>> {
    let mut items = std::collections::HashMap::new();
    for &line in lines {
        let mut rest = line.trim_start();
        for visibility in ["pub(crate) ", "pub "] {
            rest = rest.strip_prefix(visibility).unwrap_or(rest);
        }
        let Some(rest) = rest
            .strip_prefix("const ")
            .or_else(|| rest.strip_prefix("static "))
        else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if end == 0 {
            continue;
        }
        let name = &rest[..end];
        if let Some(value) = binding_of(line, name, &["const", "static"]) {
            items.entry(name).or_insert(value);
        }
    }
    items
}

/// Whether `line` binds `name` with one of `keywords`: `Some(Some(n))`
/// for a decimal literal on the same line, `Some(None)` for anything
/// else, `None` when the line binds something else or nothing.
#[allow(clippy::option_option)]
fn binding_of(line: &str, name: &str, keywords: &[&str]) -> Option<Option<i32>> {
    let mut rest = line.trim_start();
    for visibility in ["pub(crate) ", "pub "] {
        rest = rest.strip_prefix(visibility).unwrap_or(rest);
    }
    let rest = keywords
        .iter()
        .find_map(|keyword| rest.strip_prefix(keyword)?.strip_prefix(' '))?
        .trim_start();
    let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
    let rest = rest.strip_prefix(name)?;
    if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    // `let domain: u32 = 27;`
    let (_, value) = rest.split_once('=')?;
    Some(
        value
            .trim()
            .strip_suffix(';')
            .and_then(|value| decimal(value.trim())),
    )
}

/// Whether `line` is the signature of a function: the boundary a `let`
/// does not reach across.
fn starts_a_function(line: &str) -> bool {
    let mut rest = line.trim_start();
    loop {
        let before = rest;
        for qualifier in [
            "pub(crate) ",
            "pub ",
            "async ",
            "unsafe ",
            "const ",
            "extern \"C\" ",
        ] {
            rest = rest.strip_prefix(qualifier).unwrap_or(rest);
        }
        if rest == before {
            break;
        }
    }
    rest.starts_with("fn ")
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
    // The lines once: a window skipped to from the top for every `fn` read
    // the file once per function. The test took 10.5 s on the Linux CI host
    // (job 228489) and ran past its 25 s on Windows (job 228611).
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
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
        let window: String = lines[index..lines.len().min(index + 60)].join("\n");
        if !BINDING_CALLS.iter().any(|(call, _)| {
            window.match_indices(call).any(|(at, _)| {
                *call != "create_participant("
                    || !window[..at + call.len()].ends_with("create_participant_with_config(")
            })
        }) {
            continue;
        }
        let signature: String = lines[index..lines.len().min(index + 12)].join("\n");
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
