//! The seven bridge man5 pages checked against the seven example configs.
//!
//! §11 of each bridge spec ships two artefacts that describe the same
//! schema: `/etc/zerodds/<daemon>.yaml` documented in `man5`, and the
//! example config installed next to it. They drifted apart once already
//! — every man page described a `participant:` / `routes:` / `logging:`
//! / `observability:` schema that no daemon has ever read, while the
//! examples had been pulled onto the schema of §3. A page that names a
//! key the daemon ignores is worse than no page: it sends an operator
//! looking for a typo that is not there.
//!
//! The check runs in both directions on the key names themselves:
//! every key the example declares must appear in the man page, and
//! every key the man page sets in bold must appear in the example.
//! Commented-out keys in the example count — they are the optional part
//! of the schema, and the example is where they are declared.
//!
//! Paths are resolved from the repository root, three levels above this
//! crate's manifest (`tools/traceability`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The seven daemons of §1.1: (man page stem, example config stem).
const PAGES: [(&str, &str); 7] = [
    ("zerodds-ws-bridged.yaml", "ws-bridged.yaml"),
    ("zerodds-mqtt-bridged.yaml", "mqtt-bridged.yaml"),
    ("zerodds-coap-bridged.yaml", "coap-bridged.yaml"),
    ("zerodds-amqp-bridged.yaml", "amqp-bridged.yaml"),
    ("zerodds-grpc-bridged.yaml", "grpc-bridged.yaml"),
    ("zerodds-corba-bridged.yaml", "corba-bridged.yaml"),
    ("zerodds-ros2.yaml", "ros2.yaml"),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root above tools/traceability")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Every mapping key of a YAML document, at any depth, including the
/// commented-out ones. `- identity: x` contributes `identity`.
fn yaml_keys(doc: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in doc.lines() {
        let mut t = line.trim();
        // A commented line still declares a key: the optional part of
        // the schema lives behind `#` in every one of the examples.
        t = t.strip_prefix('#').map_or(t, str::trim_start);
        t = t.strip_prefix("- ").map_or(t, str::trim_start);
        let Some((key, _)) = t.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !is_key_token(key) {
            continue;
        }
        out.insert(key.to_string());
        // A flow map on the same line declares its keys too:
        // `history: { kind: keep_last, depth: 10 }`.
        if let Some(inner) = t
            .split_once('{')
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(inner, _)| inner)
        {
            for entry in inner.split(',') {
                if let Some((k, _)) = entry.split_once(':') {
                    let k = k.trim();
                    if is_key_token(k) {
                        out.insert(k.to_string());
                    }
                }
            }
        }
    }
    out
}

/// A config key is lowercase ASCII with digits and underscores. The
/// filter keeps prose, `\-\-flags`, `#` and IANA names out of the sets
/// on both sides.
fn is_key_token(t: &str) -> bool {
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Splits a roff macro line into its arguments, honouring the `"..."`
/// quoting the `.B`/`.BR`/`.BI` macros use to carry spaces.
fn roff_args(rest: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut started = false;
    for c in rest.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                started = true;
            }
            ' ' | '\t' if !in_quote => {
                if started {
                    args.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            _ => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(cur);
    }
    args
}

/// Every key a man page sets in **bold**. `.B` bolds every argument;
/// `.BR` and `.BI` alternate fonts, so only the even-indexed arguments
/// are bold. Getting that alternation wrong makes an extractor read the
/// *values* of a page as if they were keys.
fn man_bold_keys(page: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in page.lines() {
        let (rest, alternating) = match () {
            () if line.starts_with(".B ") => (&line[3..], false),
            () if line.starts_with(".BR ") => (&line[4..], true),
            () if line.starts_with(".BI ") => (&line[4..], true),
            () => continue,
        };
        let args = roff_args(rest);
        for (i, arg) in args.iter().enumerate() {
            if alternating && i % 2 == 1 {
                continue; // the roman/italic half of each pair
            }
            let tok = arg.trim_matches([',', '.', ';', ':', '(', ')'].as_slice());
            // `ros2.ros_domain_id` style references contribute their leaf.
            let leaf = tok.rsplit('.').next().unwrap_or(tok);
            if is_key_token(leaf) {
                out.insert(leaf.to_string());
            }
        }
    }
    out
}

/// §11 packaging: every key the shipped example declares is described
/// by the man page for the same daemon. A key that reaches the daemon
/// but is undocumented is a key an operator has to read the parser to
/// find.
#[test]
fn every_example_key_is_documented_in_the_man_page() {
    for (man_stem, example_stem) in PAGES {
        let example = read(&format!("packaging/linux/configs/{example_stem}.example"));
        let page = read(&format!("man/man5/{man_stem}.5"));
        let documented = man_bold_keys(&page);
        let missing: Vec<String> = yaml_keys(&example)
            .into_iter()
            .filter(|k| !documented.contains(k))
            .collect();
        assert!(
            missing.is_empty(),
            "man/man5/{man_stem}.5 does not document {missing:?}, \
             declared by {example_stem}.example"
        );
    }
}

/// The other direction: the man page names no key the example does not
/// declare. This is the check that caught the old pages, which
/// described `participant`, `routes`, `observability` and
/// `iiop_listen` — none of which any parser reads.
#[test]
fn the_man_page_names_no_key_the_example_does_not_declare() {
    // `.BR name (1)` cross-references in SEE ALSO are page names, not
    // keys, and the daemon names are not config keys either.
    let cross_refs: BTreeSet<String> = ["zerodds", "1"].iter().map(|s| (*s).to_string()).collect();
    for (man_stem, example_stem) in PAGES {
        let example = read(&format!("packaging/linux/configs/{example_stem}.example"));
        let page = read(&format!("man/man5/{man_stem}.5"));
        let declared = yaml_keys(&example);
        let invented: Vec<String> = man_bold_keys(&page)
            .into_iter()
            .filter(|k| !declared.contains(k) && !cross_refs.contains(k))
            .collect();
        assert!(
            invented.is_empty(),
            "man/man5/{man_stem}.5 documents {invented:?}, which \
             {example_stem}.example does not declare — the daemon most \
             likely does not read them either"
        );
    }
}

/// The schema that never existed, pinned by name so it cannot come
/// back: no man page and no example may reintroduce the
/// `participant:` / `routes:` / `observability:` vocabulary.
#[test]
fn the_pre_spec_vocabulary_is_gone_from_pages_and_examples() {
    // `logging:` is not on the list: it is the real schema of the ROS 2
    // shim, which has no `log_level:` scalar.
    const GHOSTS: [&str; 5] = [
        "participant",
        "routes",
        "observability",
        "iiop_listen",
        "metrics_bind",
    ];
    for (man_stem, example_stem) in PAGES {
        let example = read(&format!("packaging/linux/configs/{example_stem}.example"));
        let page = read(&format!("man/man5/{man_stem}.5"));
        for ghost in GHOSTS {
            assert!(
                !yaml_keys(&example).contains(ghost),
                "{example_stem}.example reintroduces the key {ghost:?}"
            );
            // A page may *mention* a ghost in prose — the CORBA page
            // says `bind`, not `iiop_listen` — so only bold keys count.
            assert!(
                !man_bold_keys(&page).contains(ghost),
                "man/man5/{man_stem}.5 documents the key {ghost:?}"
            );
        }
    }
}

/// Every man page must point at the parser that is the source of truth
/// and must say what an unknown key does: a daemon logs a WARN and starts,
/// the RMW fails `rmw_init`. Both are operational facts an operator cannot
/// guess.
#[test]
fn every_man_page_names_its_parser_and_what_an_unknown_key_does() {
    for (man_stem, _) in PAGES {
        let page = read(&format!("man/man5/{man_stem}.5"));
        assert!(
            page.contains("crates/"),
            "man/man5/{man_stem}.5 does not name the parser it documents"
        );
        let unknown = if man_stem == "zerodds-ros2.yaml" {
            "an unknown key or an invalid value fails initialisation"
        } else {
            "WARN"
        };
        assert!(
            page.contains(unknown),
            "man/man5/{man_stem}.5 does not say what an unknown key does ({unknown})"
        );
        assert!(
            page.contains("${VAR}"),
            "man/man5/{man_stem}.5 does not document the environment substitution"
        );
        assert!(
            page.contains(".SH SYNOPSIS"),
            "man/man5/{man_stem}.5 has no SYNOPSIS naming the config path"
        );
    }
}
