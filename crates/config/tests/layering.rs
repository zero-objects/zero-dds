// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Layered configuration contracts, including explicit directory includes,
//! deterministic merging, source provenance and refusal paths.
//!
//! Each of these was named as missing before it existed: a typed base, sorted
//! drop-ins, controlled adding versus overriding, more than one drop-in
//! directory, nested includes, and cycle detection with a traceable origin
//! for every value.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use zerodds_config::{ConfigError, Loader, MAX_NESTING_DEPTH, Node, Origin, origins};

/// A directory of this test's own, under the target directory so a
/// `cargo clean` takes it along.
fn workdir(leaf: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("layering")
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the work directory");
    dir
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the parent directory");
    }
    std::fs::write(path, text).expect("the file is written");
}

#[test]
fn a_base_file_is_read_and_every_value_names_its_line() {
    let dir = workdir("base");
    let base = dir.join("zerodds.yaml");
    write(&base, "transport:\n  mtu: 1400\n  name: udp\n");

    let config = Loader::new().base(&base).load().expect("loads");
    assert_eq!(config.get_str("transport.mtu"), Some("1400"));
    assert_eq!(config.get_str("transport.name"), Some("udp"));
    match config.origin_of("transport.mtu") {
        Some(Origin::File { path, line }) => {
            assert!(path.ends_with("zerodds.yaml"), "path was {path}");
            assert_eq!(*line, 2, "mtu is on line 2 and has to say so");
        }
        other => panic!("expected a file origin, got {other:?}"),
    }
    assert_eq!(
        config.origin_of("transport.name"),
        Some(&Origin::File {
            path: base.display().to_string(),
            line: 3
        })
    );
}

#[test]
fn drop_ins_are_read_sorted_by_name_and_not_by_the_filesystem() {
    let dir = workdir("sorted");
    let d = dir.join("zerodds.d");
    // Written in an order that is not the sorted one, so a loader that just
    // takes `read_dir` order has a chance to be wrong.
    write(&d.join("99-last.yaml"), "value: last\n");
    write(&d.join("10-first.yaml"), "value: first\n");
    write(&d.join("50-middle.yaml"), "value: middle\n");

    let config = Loader::new().drop_in_dir(&d).load().expect("loads");
    assert_eq!(
        config.get_str("value"),
        Some("last"),
        "99- has to be read after 10- and 50-, so it wins"
    );
    let read: Vec<String> = config
        .sources()
        .iter()
        .map(|s| {
            Path::new(s)
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(read, ["10-first.yaml", "50-middle.yaml", "99-last.yaml"]);
}

#[test]
fn a_later_directory_wins_over_an_earlier_one() {
    let dir = workdir("dirs");
    let package = dir.join("package.d");
    let site = dir.join("site.d");
    let runtime = dir.join("run.d");
    write(
        &package.join("10-net.yaml"),
        "net:\n  mtu: 1400\n  ttl: 1\n",
    );
    write(&site.join("10-net.yaml"), "net:\n  mtu: 9000\n");
    write(&runtime.join("10-net.yaml"), "net:\n  ttl: 4\n");

    let config = Loader::new()
        .drop_in_dir(&package)
        .drop_in_dir(&site)
        .drop_in_dir(&runtime)
        .load()
        .expect("loads");
    assert_eq!(
        config.get_str("net.mtu"),
        Some("9000"),
        "site overrode the package"
    );
    assert_eq!(
        config.get_str("net.ttl"),
        Some("4"),
        "runtime overrode the package"
    );
    let origin = config.origin_of("net.mtu").expect("an origin");
    assert!(
        format!("{origin}").contains("site.d"),
        "the origin has to name the directory that actually set it, said {origin}"
    );
}

#[test]
fn plus_appends_where_a_bare_key_replaces() {
    let dir = workdir("append");
    let d = dir.join("d");
    write(
        &d.join("10-base.yaml"),
        "peers:\n  - 10.0.0.1\n  - 10.0.0.2\n",
    );
    write(&d.join("20-add.yaml"), "+peers:\n  - 10.0.0.3\n");

    let config = Loader::new().drop_in_dir(&d).load().expect("loads");
    let Some(Node::Seq(peers)) = config.get("peers") else {
        panic!("peers is a sequence, got {:?}", config.get("peers"));
    };
    let values: Vec<&str> = peers.iter().filter_map(Node::as_str).collect();
    assert_eq!(values, ["10.0.0.1", "10.0.0.2", "10.0.0.3"]);

    // The bare spelling replaces, in the same layout.
    let dir2 = workdir("replace");
    let d2 = dir2.join("d");
    write(
        &d2.join("10-base.yaml"),
        "peers:\n  - 10.0.0.1\n  - 10.0.0.2\n",
    );
    write(&d2.join("20-set.yaml"), "peers:\n  - 10.0.0.9\n");
    let config2 = Loader::new().drop_in_dir(&d2).load().expect("loads");
    let Some(Node::Seq(peers2)) = config2.get("peers") else {
        panic!("peers is a sequence");
    };
    let values2: Vec<&str> = peers2.iter().filter_map(Node::as_str).collect();
    assert_eq!(values2, ["10.0.0.9"], "a bare key replaces the whole list");
}

#[test]
fn append_keys_are_applied_even_in_the_first_or_a_new_nested_map() {
    let dir = workdir("first-append");
    write(
        &dir.join("base.yaml"),
        "+peers:\n  - first\nnet:\n  +peers:\n    - nested\n",
    );
    write(
        &dir.join("site.d/10-add.yaml"),
        "+peers:\n  - second\nnet:\n  +peers:\n    - next\nother:\n  +peers:\n    - new\n",
    );
    let config = Loader::new()
        .base(dir.join("base.yaml"))
        .drop_in_dir(dir.join("site.d"))
        .load()
        .expect("append keys are normalized");
    for (path, expected) in [
        ("peers", vec!["first", "second"]),
        ("net.peers", vec!["nested", "next"]),
        ("other.peers", vec!["new"]),
    ] {
        let Some(Node::Seq(values)) = config.get(path) else {
            panic!("{path} is a list")
        };
        assert_eq!(
            values.iter().filter_map(Node::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert!(config.get("+peers").is_none());
    assert!(config.get("net.+peers").is_none());
    assert!(config.get("other.+peers").is_none());
}

#[test]
fn a_refused_layer_changes_neither_values_nor_sources() {
    let mut config = zerodds_config::Config::default();
    config
        .push(
            zerodds_config::parse_str("a: original\nz:\n  child: scalar\n", "base.yaml").unwrap(),
            "base.yaml".into(),
        )
        .unwrap();
    let before = config.root().cloned();
    let sources = config.sources().to_vec();
    let rejected =
        zerodds_config::parse_str("a: changed\nz:\n  +child:\n    - invalid\n", "bad.yaml")
            .unwrap();
    assert!(matches!(
        config.push(rejected, "bad.yaml".into()),
        Err(ConfigError::AppendToNonSeq(_))
    ));
    assert_eq!(config.root(), before.as_ref());
    assert_eq!(config.sources(), sources);
}

#[cfg(unix)]
#[test]
fn a_broken_matching_fragment_symlink_is_not_silently_skipped() {
    let dir = workdir("broken-fragment");
    std::os::unix::fs::symlink(dir.join("missing.yaml"), dir.join("10-broken.yaml"))
        .expect("broken symlink");
    assert!(
        matches!(Loader::new().drop_in_dir(&dir).load(), Err(ConfigError::Io(path, _)) if path.ends_with("10-broken.yaml"))
    );
}

#[test]
fn appending_to_something_that_is_not_a_list_is_refused_by_name() {
    let dir = workdir("append-scalar");
    let d = dir.join("d");
    write(&d.join("10-base.yaml"), "peers: one\n");
    write(&d.join("20-add.yaml"), "+peers:\n  - two\n");

    match Loader::new().drop_in_dir(&d).load() {
        Err(ConfigError::AppendToNonSeq(key)) => assert_eq!(key, "peers"),
        other => panic!("expected AppendToNonSeq, got {other:?}"),
    }
}

#[test]
fn includes_nest_and_resolve_relative_to_the_file_that_wrote_them() {
    let dir = workdir("include");
    write(
        &dir.join("zerodds.yaml"),
        "include: fragments/net.yaml\nname: root\n",
    );
    write(
        &dir.join("fragments/net.yaml"),
        "include: deeper.yaml\nnet:\n  mtu: 1400\n",
    );
    write(&dir.join("fragments/deeper.yaml"), "net:\n  ttl: 7\n");

    let config = Loader::new()
        .base(dir.join("zerodds.yaml"))
        .load()
        .expect("loads");
    assert_eq!(
        config.get_str("net.ttl"),
        Some("7"),
        "the deepest include was read"
    );
    assert_eq!(config.get_str("net.mtu"), Some("1400"));
    assert_eq!(config.get_str("name"), Some("root"));
    // Depth first, and the including file merged after what it included.
    let names: Vec<String> = config
        .sources()
        .iter()
        .map(|s| {
            Path::new(s)
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["deeper.yaml", "net.yaml", "zerodds.yaml"]);
}

#[test]
fn included_directories_are_sorted_nested_filtered_and_traceable() {
    let dir = workdir("included-directories");
    write(&dir.join("base.yaml"), "include: site.d\nname: root\n");
    write(
        &dir.join("site.d/90-last.yaml"),
        "net:\n  mtu: 9000\n+peers:\n  - last\n",
    );
    write(
        &dir.join("site.d/10-first.yaml"),
        "include: ../shared.d\nnet:\n  ttl: 7\n+peers:\n  - first\n",
    );
    write(
        &dir.join("shared.d/20-defaults.yaml"),
        "net:\n  mtu: 1400\n  ttl: 1\npeers:\n  - shared\n",
    );
    write(
        &dir.join("site.d/99-backup.yaml.bak"),
        "net:\n  mtu: wrong\n",
    );
    write(
        &dir.join("site.d/not-included.d/99-hidden.yaml"),
        "name: wrong\n",
    );
    let config = Loader::new()
        .base(dir.join("base.yaml"))
        .load()
        .expect("directory includes load");
    assert_eq!(config.get_str("net.mtu"), Some("9000"));
    assert_eq!(config.get_str("net.ttl"), Some("7"));
    assert_eq!(config.get_str("name"), Some("root"));
    let Some(Node::Seq(peers)) = config.get("peers") else {
        panic!("peers is a sequence")
    };
    assert_eq!(
        peers.iter().map(Node::as_str).collect::<Vec<_>>(),
        [Some("shared"), Some("first"), Some("last")]
    );
    assert_eq!(config.sources().len(), 4);
    assert_eq!(
        config.origin_of("net.mtu"),
        Some(&Origin::File {
            path: dir.join("site.d/90-last.yaml").display().to_string(),
            line: 2
        })
    );
}

#[test]
fn an_explicit_include_is_required_but_an_empty_directory_is_valid() {
    let dir = workdir("required-include");
    write(&dir.join("base.yaml"), "include: absent.d\n");
    assert!(
        matches!(Loader::new().base(dir.join("base.yaml")).load(), Err(ConfigError::Io(path, _)) if path.ends_with("absent.d"))
    );
    std::fs::create_dir(dir.join("absent.d")).expect("empty directory");
    let config = Loader::new()
        .base(dir.join("base.yaml"))
        .load()
        .expect("empty directory is valid");
    assert_eq!(config.sources().len(), 1);
}

#[test]
fn malformed_include_directives_are_refused_instead_of_dropping_layers() {
    let dir = workdir("malformed-include");
    write(&dir.join("valid.yaml"), "value: included\n");
    for text in [
        "include:\n  directory: site.d\n",
        "include:\n  - valid.yaml\n  - directory: site.d\n",
        "include: ''\n",
        "include:\n  - ''\n",
    ] {
        let path = dir.join("base.yaml");
        write(&path, text);
        assert!(
            matches!(Loader::new().base(&path).load(), Err(ConfigError::Syntax { path: source, message, .. }) if source == path.display().to_string() && message.contains("include expects"))
        );
    }
}

#[test]
fn directory_cycles_are_refused_and_repeated_noncyclic_includes_remain_ordered() {
    let dir = workdir("directory-cycle");
    write(&dir.join("base.yaml"), "include: site.d\n");
    write(&dir.join("site.d/10-cycle.yaml"), "include: ../site.d\n");
    assert!(
        matches!(Loader::new().base(dir.join("base.yaml")).load(), Err(ConfigError::IncludeCycle(chain)) if chain.last().is_some_and(|p| p.ends_with("site.d")))
    );
    write(&dir.join("site.d/10-cycle.yaml"), "+peers:\n  - same\n");
    write(&dir.join("base.yaml"), "include:\n  - site.d\n  - site.d\n");
    let config = Loader::new()
        .base(dir.join("base.yaml"))
        .load()
        .expect("not an active cycle");
    let Some(Node::Seq(peers)) = config.get("peers") else {
        panic!("peers is a sequence")
    };
    assert_eq!(peers.len(), 2);
}

#[test]
fn included_directories_use_the_selected_extension() {
    let dir = workdir("include-extension");
    write(&dir.join("base.conf"), "include: site.d\n");
    write(&dir.join("site.d/10-good.conf"), "value: good\n");
    write(&dir.join("site.d/99-other.yaml"), "value: wrong\n");
    let config = Loader::new()
        .extension(Some("conf"))
        .base(dir.join("base.conf"))
        .load()
        .expect("loads conf fragments");
    assert_eq!(config.get_str("value"), Some("good"));
}

#[test]
fn including_file_keys_win_regardless_of_the_directives_text_position() {
    let dir = workdir("include-precedence");
    write(&dir.join("site.d/10-values.yaml"), "value: included\n");
    write(&dir.join("before.yaml"), "value: own\ninclude: site.d\n");
    write(&dir.join("after.yaml"), "include: site.d\nvalue: own\n");
    for name in ["before.yaml", "after.yaml"] {
        let config = Loader::new().base(dir.join(name)).load().expect("loads");
        assert_eq!(config.get_str("value"), Some("own"));
    }
}

#[cfg(unix)]
#[test]
fn directory_cycles_through_a_symlink_are_refused() {
    let dir = workdir("symlink-directory-cycle");
    write(&dir.join("base.yaml"), "include: site.d\n");
    write(&dir.join("site.d/10-values.yaml"), "include: ../alias.d\n");
    std::os::unix::fs::symlink(dir.join("site.d"), dir.join("alias.d")).expect("directory alias");
    assert!(matches!(
        Loader::new().base(dir.join("base.yaml")).load(),
        Err(ConfigError::IncludeCycle(_))
    ));
}

#[test]
fn directory_include_depth_is_bounded() {
    let dir = workdir("directory-depth");
    write(&dir.join("base.yaml"), "include: level0.d\n");
    for level in 0..=zerodds_config::MAX_INCLUDE_DEPTH {
        write(
            &dir.join(format!("level{level}.d/10-next.yaml")),
            &format!("include: ../level{}.d\n", level + 1),
        );
    }
    assert!(
        matches!(Loader::new().base(dir.join("base.yaml")).load(), Err(ConfigError::IncludeTooDeep(bound)) if bound == zerodds_config::MAX_INCLUDE_DEPTH)
    );
}

#[test]
fn an_include_cycle_is_named_rather_than_looped() {
    let dir = workdir("cycle");
    write(&dir.join("a.yaml"), "include: b.yaml\n");
    write(&dir.join("b.yaml"), "include: a.yaml\n");

    match Loader::new().base(dir.join("a.yaml")).load() {
        Err(ConfigError::IncludeCycle(chain)) => {
            assert!(
                chain.len() >= 2,
                "the chain has to show the way in: {chain:?}"
            );
            assert!(
                chain.last().is_some_and(|last| last.ends_with("a.yaml")),
                "the cycle closes on a.yaml, chain was {chain:?}"
            );
        }
        other => panic!("expected an IncludeCycle, got {other:?}"),
    }
}

#[test]
fn a_missing_drop_in_directory_contributes_nothing_and_a_required_base_does_not() {
    let dir = workdir("missing");
    write(&dir.join("zerodds.yaml"), "a: 1\n");

    let config = Loader::new()
        .base(dir.join("zerodds.yaml"))
        .drop_in_dir(dir.join("not-there.d"))
        .load()
        .expect("a missing drop-in directory is normal");
    assert_eq!(config.get_str("a"), Some("1"));

    // A base that was asked for by name and is not there is an error.
    match Loader::new()
        .base(dir.join("absent.yaml"))
        .require_base()
        .load()
    {
        Err(ConfigError::Io(path, _)) => assert!(path.ends_with("absent.yaml")),
        other => panic!("expected an Io error, got {other:?}"),
    }
    // Without `require_base` the same absence is simply empty.
    let empty = Loader::new()
        .base(dir.join("absent.yaml"))
        .load()
        .expect("loads");
    assert!(empty.root().is_none());
}

#[test]
fn only_the_named_extension_is_read_so_a_package_leftover_cannot_change_what_runs() {
    let dir = workdir("extension");
    let d = dir.join("d");
    write(&d.join("10-net.yaml"), "mtu: 1400\n");
    write(&d.join("10-net.yaml.rpmnew"), "mtu: 9999\n");
    write(&d.join("10-net.yaml.bak"), "mtu: 1\n");

    let config = Loader::new().drop_in_dir(&d).load().expect("loads");
    assert_eq!(
        config.get_str("mtu"),
        Some("1400"),
        "an upgrade leftover must not be read"
    );
    assert_eq!(config.sources().len(), 1);
}

#[test]
fn every_value_can_be_traced_in_one_listing() {
    let dir = workdir("origins");
    let base = dir.join("zerodds.yaml");
    let d = dir.join("d");
    write(&base, "net:\n  mtu: 1400\n  ttl: 1\n");
    write(&d.join("50-site.yaml"), "net:\n  mtu: 9000\n");

    let config = Loader::new()
        .base(&base)
        .drop_in_dir(&d)
        .load()
        .expect("loads");
    let listing = origins(&config);
    let mtu = listing
        .iter()
        .find(|(path, _)| path == "net.mtu")
        .expect("net.mtu is listed");
    let ttl = listing
        .iter()
        .find(|(path, _)| path == "net.ttl")
        .expect("net.ttl is listed");
    assert!(
        format!("{}", mtu.1).contains("50-site.yaml"),
        "mtu came from the drop-in, said {}",
        mtu.1
    );
    assert!(
        format!("{}", ttl.1).contains("zerodds.yaml"),
        "ttl came from the base, said {}",
        ttl.1
    );
}

#[test]
fn a_construct_outside_the_subset_is_refused_rather_than_misread() {
    let dir = workdir("subset");
    for (name, text, expect) in [
        (
            "flowmapnest.yaml",
            "qos: {history: {depth: 1}}\n",
            "flat map of scalars",
        ),
        (
            "flowmapdup.yaml",
            "qos: {depth: 1, depth: 2}\n",
            "distinct keys",
        ),
        ("flowmapkey.yaml", "qos: {depth}\n", "flat map of scalars"),
        ("flownest.yaml", "peers: [a, [b]]\n", "flat list of scalars"),
        ("flowopen.yaml", "peers: [a, b\n", "flat list of scalars"),
        (
            "flowempty.yaml",
            "peers: [a, , b]\n",
            "flat list of scalars",
        ),
        (
            "seqseq.yaml",
            "peers:\n  - - a\n",
            "sequence directly inside",
        ),
        (
            "scalarcont.yaml",
            "peers:\n  - a\n    b: 1\n",
            "does not continue",
        ),
        ("anchor.yaml", "base: &x\n", "anchors"),
        ("alias.yaml", "other: *x\n", "aliases"),
        ("block.yaml", "text: |\n", "block scalars"),
        ("tab.yaml", "net:\n\tmtu: 1\n", "tab"),
    ] {
        let path = dir.join(name);
        write(&path, text);
        match Loader::new().base(&path).load() {
            Err(ConfigError::Syntax { message, .. }) => assert!(
                message.contains(expect),
                "{name}: expected a refusal naming `{expect}`, said `{message}`"
            ),
            other => panic!("{name}: expected a syntax refusal, got {other:?}"),
        }
    }
}

#[test]
fn a_file_nested_past_the_bound_is_refused_rather_than_taking_the_stack() {
    let dir = workdir("deep");
    // One key per level, two spaces deeper each time. A configuration file is
    // something an operator downloads, a package ships and a container mounts;
    // it does not get to decide how much stack it is given.
    let mut text = String::new();
    for level in 0..(MAX_NESTING_DEPTH + 5) {
        text.push_str(&" ".repeat(level * 2));
        text.push_str(&format!("k{level}:\n"));
    }
    text.push_str(&" ".repeat((MAX_NESTING_DEPTH + 5) * 2));
    text.push_str("leaf: 1\n");
    let path = dir.join("deep.yaml");
    write(&path, &text);

    match Loader::new().base(&path).load() {
        Err(ConfigError::TooDeep(bound)) => assert_eq!(bound, MAX_NESTING_DEPTH),
        other => panic!("expected TooDeep, got {other:?}"),
    }

    // And a file inside the bound still loads, so the guard is a bound and
    // not a wall in front of ordinary files.
    let mut ok_text = String::new();
    for level in 0..8 {
        ok_text.push_str(&" ".repeat(level * 2));
        ok_text.push_str(&format!("k{level}:\n"));
    }
    ok_text.push_str(&" ".repeat(8 * 2));
    ok_text.push_str("leaf: 1\n");
    let ok_path = dir.join("fine.yaml");
    write(&ok_path, &ok_text);
    let config = Loader::new()
        .base(&ok_path)
        .load()
        .expect("eight levels load");
    assert_eq!(config.get_str("k0.k1.k2.k3.k4.k5.k6.k7.leaf"), Some("1"));
}

#[test]
fn a_flat_flow_list_is_a_sequence_and_sequence_items_nest_as_written() {
    let dir = workdir("nesting");
    let path = dir.join("bridge.yaml");
    write(
        &path,
        concat!(
            "acl: [\"alice\", 'bob, jr', carol]\n",
            "empty: []\n",
            "topics:\n",
            "  - name: Chat::Message\n",
            "    qos:\n",
            "      reliability: reliable\n",
            "      history:\n",
            "        depth: 5\n",
            "    acl:\n",
            "      read: [\"*\"]\n",
            "      write:\n",
            "        - alice\n",
            "  - name: Sensor\n",
            "    type: S\n",
        ),
    );
    let config = Loader::new().base(&path).load().expect("loads");
    let scalars = |path: &str| -> Vec<String> {
        match config.get(path) {
            Some(Node::Seq(items)) => items
                .iter()
                .map(|n| n.as_str().unwrap_or("<not a scalar>").to_string())
                .collect(),
            other => panic!("{path}: expected a sequence, got {other:?}"),
        }
    };
    assert_eq!(scalars("acl"), ["alice", "bob, jr", "carol"]);
    assert!(scalars("empty").is_empty());
    let Some(Node::Seq(topics)) = config.get("topics") else {
        panic!("topics must be a sequence");
    };
    assert_eq!(topics.len(), 2);
    assert_eq!(
        topics[0].get("qos.history.depth").and_then(Node::as_str),
        Some("5")
    );
    assert_eq!(
        topics[0].get("qos.reliability").and_then(Node::as_str),
        Some("reliable")
    );
    assert!(
        topics[0].get("reliability").is_none(),
        "a nested key must not surface one level up"
    );
    assert!(matches!(topics[0].get("acl.read"), Some(Node::Seq(r)) if r.len() == 1));
    assert!(matches!(topics[0].get("acl.write"), Some(Node::Seq(w)) if w.len() == 1));
    assert_eq!(topics[1].get("type").and_then(Node::as_str), Some("S"));
    match topics[0].get("qos.history.depth").and_then(Node::origin) {
        Some(Origin::File { line, .. }) => assert_eq!(*line, 8),
        other => panic!("depth must keep its line, got {other:?}"),
    }
}

/// `#` inside quotes is part of the value; outside, it starts a comment.
#[test]
fn a_hash_inside_quotes_is_part_of_the_value() {
    let node = zerodds_config::parse_str(
        "a:\n  double: \"x#y\"  # comment\n  single: 'p#q'\n  bare: v # comment\n",
        "<string>",
    )
    .expect("parses");
    assert_eq!(node.get("a.double").and_then(Node::as_str), Some("x#y"));
    assert_eq!(node.get("a.single").and_then(Node::as_str), Some("p#q"));
    assert_eq!(node.get("a.bare").and_then(Node::as_str), Some("v"));
}

/// `${VAR}` takes the environment, `${VAR:-default}` the default when the
/// variable is unset, and an unset variable without a default is empty.
#[test]
fn environment_references_expand_before_the_parse() {
    const UNSET: &str = "ZERODDS_CONFIG_TEST_UNSET_7d2e";
    assert!(std::env::var(UNSET).is_err(), "{UNSET} must be unset");
    let path = std::env::var("PATH").expect("PATH is set");
    let node = zerodds_config::parse_str(
        &format!("e:\n  set: \"${{PATH}}\"\n  fallback: \"${{{UNSET}:-/from-default}}\"\n  empty: \"x${{{UNSET}}}y\"\n"),
        "<string>",
    )
    .expect("parses");
    assert_eq!(
        node.get("e.set").and_then(Node::as_str),
        Some(path.as_str())
    );
    assert_eq!(
        node.get("e.fallback").and_then(Node::as_str),
        Some("/from-default")
    );
    assert_eq!(node.get("e.empty").and_then(Node::as_str), Some("xy"));
}

/// A flat flow map is a map (`history: { kind: keep_last, depth: 10 }` in
/// the bridge specs), with quotes and `${VAR}` as in block form.
#[test]
fn a_flat_flow_map_is_a_map() {
    let dir = workdir("flowmap");
    let path = dir.join("bridge.yaml");
    write(
        &path,
        "qos:\n  history: { kind: \"keep_last\", depth: 10 }\n  empty: {}\n",
    );
    let config = Loader::new().base(&path).load().expect("loads");
    assert_eq!(
        config.get("qos.history.kind").and_then(Node::as_str),
        Some("keep_last")
    );
    assert_eq!(
        config.get("qos.history.depth").and_then(Node::as_str),
        Some("10")
    );
    assert!(matches!(config.get("qos.empty"), Some(Node::Map(m)) if m.is_empty()));
    // Every value names its line, as a block map's would.
    assert!(
        origins(&config)
            .iter()
            .any(|(p, o)| p == "qos.history.depth" && o.to_string().contains("bridge.yaml:2")),
        "{:?}",
        origins(&config)
    );
}
