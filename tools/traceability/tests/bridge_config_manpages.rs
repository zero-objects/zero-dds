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
const PAGES: [(&str, &str); 8] = [
    ("zerodds-ws-bridged.yaml", "ws-bridged.yaml"),
    ("zerodds-mqtt-bridged.yaml", "mqtt-bridged.yaml"),
    ("zerodds-coap-bridged.yaml", "coap-bridged.yaml"),
    ("zerodds-amqp-bridged.yaml", "amqp-bridged.yaml"),
    ("zerodds-grpc-bridged.yaml", "grpc-bridged.yaml"),
    ("zerodds-corba-bridged.yaml", "corba-bridged.yaml"),
    ("zerodds-opcua-bridged.yaml", "opcua-bridged.yaml"),
    ("zerodds-ros2.yaml", "ros2.yaml"),
];

/// The parser of each daemon of [`PAGES`]: the source files whose
/// `.only(&[...])` lists and `KEYS` table name every key it reads.
const PARSERS: [(&str, &[&str]); 8] = [
    (
        "zerodds-ws-bridged.yaml",
        &["crates/websocket-bridge/src/daemon/config.rs"],
    ),
    (
        "zerodds-mqtt-bridged.yaml",
        &["crates/mqtt-bridge/src/daemon/config.rs"],
    ),
    (
        "zerodds-coap-bridged.yaml",
        &["crates/coap-bridge/src/daemon/config.rs"],
    ),
    (
        "zerodds-amqp-bridged.yaml",
        &["crates/amqp-endpoint/src/bridge_daemon/config.rs"],
    ),
    (
        "zerodds-grpc-bridged.yaml",
        &["crates/grpc-bridge/src/bridge_daemon/config.rs"],
    ),
    (
        "zerodds-corba-bridged.yaml",
        &["crates/corba-dds-bridge/src/bridge_daemon/config.rs"],
    ),
    (
        "zerodds-opcua-bridged.yaml",
        &["crates/opcua-bridge/src/daemon/config.rs"],
    ),
    (
        "zerodds-ros2.yaml",
        &["crates/rmw-zerodds-shim/src/ros2_config.rs"],
    ),
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

/// The string literals of every `<open>...]` list in `src`, such as
/// `.only(&[` or `pub const KEYS: &[&str] = &[`; a list may span lines.
fn string_lists(src: &str, open: &str) -> Vec<Vec<String>> {
    let mut lists = Vec::new();
    let mut rest = src;
    while let Some(at) = rest.find(open) {
        let after = &rest[at + open.len()..];
        let end = after.find(']').expect("a list literal ends with ]");
        let mut items = Vec::new();
        let mut body = &after[..end];
        while let Some(q) = body.find('"') {
            let tail = &body[q + 1..];
            let close = tail.find('"').expect("a string literal ends with a quote");
            items.push(tail[..close].to_string());
            body = &tail[close + 1..];
        }
        lists.push(items);
        rest = &after[end..];
    }
    lists
}

/// The `.SH KEYS` part of a man page split at its `.SS` headings: the
/// heading's section path (`amqp`, `opcua.server`, `topics` for
/// `topics (list)`, the empty path for `Top level` and the text before
/// the first heading) and the bold tokens under it.
fn man_subsections(page: &str) -> std::collections::BTreeMap<String, BTreeSet<String>> {
    let mut parts: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut current: Option<String> = None;
    for line in page.lines() {
        if line.starts_with(".SH ") {
            current = (line.trim_end() == ".SH KEYS").then(String::new);
            if let Some(c) = &current {
                parts.entry(c.clone()).or_default();
            }
            continue;
        }
        if let Some(heading) = line.strip_prefix(".SS ")
            && current.is_some()
        {
            let first = heading.split_whitespace().next().unwrap_or_default();
            let path = if first == "Top" {
                String::new()
            } else {
                first.replace("[]", "")
            };
            parts.entry(path.clone()).or_default();
            current = Some(path);
            continue;
        }
        if let Some(c) = &current {
            let text = parts.entry(c.clone()).or_default();
            text.push_str(line);
            text.push('\n');
        }
    }
    parts
        .into_iter()
        .map(|(k, text)| (k, man_bold_keys(&text)))
        .collect()
}

/// The identifier at the start of `s`.
fn ident(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    &s[..end]
}

/// The first string literal of `s`, or `None` when the argument is not a
/// literal (a key computed at run time, such as a profile name).
fn first_literal(s: &str) -> Option<&str> {
    let s = s.trim_start();
    let s = s.strip_prefix('"')?;
    s.find('"').map(|e| &s[..e])
}

/// The name and line of the `fn` that contains line `i`.
fn enclosing_fn(lines: &[&str], i: usize) -> Option<(String, usize)> {
    (0..=i).rev().find_map(|j| {
        let t = lines[j].trim_start();
        let t = t
            .strip_prefix("pub(crate) ")
            .or_else(|| t.strip_prefix("pub "))
            .unwrap_or(t);
        t.strip_prefix("fn ")
            .map(|rest| (ident(rest).to_string(), j))
    })
}

/// `parent` joined with the key `name`; a computed key adds nothing.
fn join(parents: Vec<String>, name: Option<&str>) -> Vec<String> {
    parents
        .into_iter()
        .map(|p| match name {
            Some(n) if p.is_empty() => n.to_string(),
            Some(n) => format!("{p}.{n}"),
            None => p,
        })
        .collect()
}

/// The section paths the `Section` bound to `var` at line `i` stands
/// for, traced back through the parser's own bindings:
/// `let v = p.section("n")?` (path of `p` plus `n`), `Section::root`
/// (the empty path), `Section::of(Some(item), ..)` (the list `item`
/// comes from), and a `&Section` parameter (the arguments at the call
/// sites). `None` when the binding is in no such form.
fn section_paths(lines: &[&str], var: &str, i: usize, depth: usize) -> Option<Vec<String>> {
    if depth > 12 {
        return None;
    }
    let (fname, fstart) = enclosing_fn(lines, i)?;
    for j in (fstart..i).rev() {
        let t = lines[j].trim_start();
        let rhs = [format!("let {var} = "), format!("let Some({var}) = ")]
            .iter()
            .find_map(|p| t.strip_prefix(p.as_str()))
            .map(str::to_string);
        let Some(rhs) = rhs else { continue };
        if rhs.starts_with("Section::root") {
            return Some(vec![String::new()]);
        }
        if rhs.starts_with("Section::of(") {
            let item = (j..lines.len().min(j + 12)).find_map(|k| {
                let at = lines[k].find("Some(")?;
                Some(ident(&lines[k][at + 5..]).to_string())
            })?;
            return item_paths(lines, &item, j, depth + 1);
        }
        let parent = ident(&rhs);
        let call = rhs[parent.len()..].strip_prefix(".section(")?;
        let up = section_paths(lines, parent, j, depth + 1)?;
        return Some(join(up, first_literal(call)));
    }
    let signature: String = lines[fstart..lines.len().min(fstart + 6)].concat();
    if signature.contains(&format!("{var}: &Section")) {
        return call_site_paths(lines, &fname, fstart, false, depth + 1);
    }
    None
}

/// The section paths of the list entry `item` bound at line `i`: the
/// loop `for (path, item) in p.items("n")` above it, or, for a function
/// parameter, the loops around its call sites.
fn item_paths(lines: &[&str], item: &str, i: usize, depth: usize) -> Option<Vec<String>> {
    let (fname, fstart) = enclosing_fn(lines, i)?;
    for j in (fstart..i).rev() {
        if let Some(paths) = items_loop(lines, j, Some(item), depth) {
            return paths;
        }
    }
    call_site_paths(lines, &fname, fstart, true, depth + 1)
}

/// The loop `for (_, item) in p.items("n")` at line `j` (any item name
/// when `item` is `None`): `Some(paths)` with the path of `p` plus `n`.
fn items_loop(
    lines: &[&str],
    j: usize,
    item: Option<&str>,
    depth: usize,
) -> Option<Option<Vec<String>>> {
    let t = lines[j].trim_start();
    let rest = t.strip_prefix("for (")?;
    let (_, rest) = rest.split_once(", ")?;
    let bound = ident(rest);
    if item.is_some_and(|it| it != bound) {
        return None;
    }
    let (_, iter) = rest.split_once(") in ")?;
    let parent = ident(iter);
    let name = iter[parent.len()..].strip_prefix(".items(")?;
    Some(section_paths(lines, parent, j, depth + 1).map(|up| join(up, first_literal(name))))
}

/// The paths at every call site of `fname` (other than its definition at
/// `fstart`): for a list entry the `items` loop just above the call, for
/// a `&Section` the first argument (`&entry` or `&entry.section("qos")?`).
fn call_site_paths(
    lines: &[&str],
    fname: &str,
    fstart: usize,
    is_item: bool,
    depth: usize,
) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let needle = format!("{fname}(");
    for (j, line) in lines.iter().enumerate() {
        if j == fstart || line.contains(&format!("fn {needle}")) {
            continue;
        }
        let Some(at) = line.find(&needle) else {
            continue;
        };
        let before = line[..at].chars().last();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let args = &line[at + needle.len()..];
        if is_item {
            let found = (j.saturating_sub(8)..j)
                .rev()
                .find_map(|k| items_loop(lines, k, None, depth))?;
            out.extend(found?);
        } else {
            let arg = args.trim_start().trim_start_matches('&');
            let var = ident(arg);
            let up = section_paths(lines, var, j, depth + 1)?;
            let name = arg[var.len()..].strip_prefix(".section(");
            out.extend(match name {
                Some(n) => join(up, first_literal(n)),
                None => up,
            });
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The heading a key at `path` is documented under: the longest prefix
/// of `path` the page has a `.SS` for, else the top level.
fn home<'a>(
    path: &str,
    subs: &'a std::collections::BTreeMap<String, BTreeSet<String>>,
) -> (&'a str, &'a BTreeSet<String>) {
    let parts: Vec<&str> = path.split('.').filter(|p| !p.is_empty()).collect();
    for n in (1..=parts.len()).rev() {
        let h = parts[..n].join(".");
        if let Some((k, v)) = subs.get_key_value(&h) {
            return (k, v);
        }
    }
    subs.get_key_value("")
        .map(|(k, v)| (k.as_str(), v))
        .expect("a man page with a KEYS section")
}

/// Whether `key` at `path` is documented under its own heading: in bold
/// there, or as a heading of its own (`opcua.server`).
fn documented_at(
    path: &str,
    key: &str,
    subs: &std::collections::BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let full = if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    };
    home(path, subs).1.contains(key) || subs.contains_key(&full)
}

/// The keys of the parser `sources` that `page` does not document under
/// the heading of their own section, as `section.key`, and how many keys
/// were checked. The keys come from each `.only(&[...])` list, placed at
/// the section its receiver is bound to, and from the dotted `KEYS`
/// table. A list whose section cannot be traced is an error: the check
/// must not pass by not looking.
fn undocumented_keys(page: &str, sources: &[String]) -> Result<(Vec<String>, usize), String> {
    let subs = man_subsections(page);
    let page_bold = man_bold_keys(page);
    let mut missing = BTreeSet::new();
    let mut checked = 0;
    for (name, src) in sources.iter().enumerate() {
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(at) = line.find(".only(&[") else {
                continue;
            };
            let receiver = line[..at]
                .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap_or_default();
            let list = string_lists(&lines[i..].join("\n"), ".only(&[")
                .into_iter()
                .next()
                .unwrap_or_default();
            let paths = section_paths(&lines, receiver, i, 0).ok_or_else(|| {
                format!(
                    "source {name}, line {}: the section of `{receiver}` cannot be traced",
                    i + 1
                )
            })?;
            for key in list {
                checked += 1;
                // A reader shared by two lists (a tenant's topics read
                // like the top-level ones) is documented once.
                if !paths.iter().any(|p| documented_at(p, &key, &subs)) {
                    missing.insert(match paths[0].as_str() {
                        "" => key,
                        p => format!("{p}.{key}"),
                    });
                }
            }
        }
        for list in string_lists(src, "pub const KEYS: &[&str] = &[") {
            for key in list {
                let parts: Vec<&str> = key.split('.').collect();
                for n in 0..parts.len() {
                    checked += 1;
                    let path = parts[..n].join(".");
                    let leaf = parts[n];
                    let is_section = n + 1 < parts.len();
                    // A page without a heading for a section names it in
                    // its description (the ROS 2 page's `ros2`).
                    let ok = documented_at(&path, leaf, &subs)
                        || (is_section
                            && home(&path, &subs).0.is_empty()
                            && page_bold.contains(leaf));
                    if !ok {
                        missing.insert(parts[..=n].join("."));
                    }
                }
            }
        }
    }
    Ok((missing.into_iter().collect(), checked))
}

/// §11 packaging, from the parser's side: every key a daemon reads is
/// documented in its man page, under the heading of its own section. The
/// two tests above compare the page with the example, so a key both of
/// them leave out (the AMQP daemon's `catalog` section and `type_file`
/// were such keys) went unnoticed; and a key whose name another section
/// also uses (`enabled`) would pass a check against the whole page.
#[test]
fn every_key_the_daemon_reads_is_in_the_man_page() {
    let mut checked = 0;
    for (man_stem, sources) in PARSERS {
        let page = read(&format!("man/man5/{man_stem}.5"));
        let srcs: Vec<String> = sources.iter().map(|s| read(s)).collect();
        let (missing, n) =
            undocumented_keys(&page, &srcs).unwrap_or_else(|e| panic!("{sources:?}: {e}"));
        assert!(
            n > 0,
            "{sources:?} has no `.only(&[...])` list and no KEYS table"
        );
        assert!(
            missing.is_empty(),
            "man/man5/{man_stem}.5 does not document {missing:?} under its section, which {sources:?} reads"
        );
        checked += n;
    }
    assert!(checked > 300, "only {checked} keys found in the parsers");
}

/// The check can fail: the AMQP page without the `enabled` entry of its
/// `catalog` section is refused, although `enabled` stays in bold under
/// `metrics` and in the prose of `amqp.tls`.
#[test]
fn a_key_missing_from_its_own_section_is_found() {
    let page = read("man/man5/zerodds-amqp-bridged.yaml.5");
    let src = [read("crates/amqp-endpoint/src/bridge_daemon/config.rs")];
    let lines: Vec<&str> = page.lines().collect();
    let at = lines
        .windows(3)
        .position(|w| w[0] == ".SS catalog" && w[1] == ".TP" && w[2] == ".B enabled")
        .expect("the catalog section starts with its enabled entry");
    let mut cut = lines.clone();
    cut.drain(at + 1..at + 3);
    let cut = cut.join("\n");
    assert!(man_bold_keys(&cut).contains("enabled"));
    let (missing, _) = undocumented_keys(&cut, &src).expect("traceable");
    assert_eq!(missing, vec!["catalog.enabled".to_string()]);
    let (missing, _) = undocumented_keys(&page, &src).expect("traceable");
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn section_paths_follow_sections_lists_and_call_sites() {
    let src = "fn from_config() {\n    let root = Section::root(x)?;\n    let amqp = root.section(\"amqp\")?;\n    let tls = amqp.section(\"tls\")?;\n    tls.only(&[\"enabled\"])?;\n    for (path, item) in root.items(\"topics\")? {\n        out.push(read_topic(&path, item)?);\n    }\n}\nfn read_topic(path: &str, item: &Node) {\n    let entry = Section::of(Some(item), path)?;\n    let qos = entry.section(\"qos\")?;\n    qos.only(&[\"depth\"])?;\n}";
    let lines: Vec<&str> = src.lines().collect();
    assert_eq!(
        section_paths(&lines, "tls", 4, 0),
        Some(vec!["amqp.tls".to_string()])
    );
    assert_eq!(
        section_paths(&lines, "qos", 12, 0),
        Some(vec!["topics.qos".to_string()])
    );
    assert_eq!(section_paths(&lines, "nothing", 12, 0), None);
}

#[test]
fn string_lists_reads_a_list_over_several_lines() {
    let src = "a.only(&[\n    \"x\",\n    \"y_z\",\n])?;\nb.only(&[\"w\"])?;";
    assert_eq!(
        string_lists(src, ".only(&["),
        vec![
            vec!["x".to_string(), "y_z".to_string()],
            vec!["w".to_string()]
        ]
    );
}
