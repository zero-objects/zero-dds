// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The layers around the files: environment beneath, `--set` above, and the
//! flag parser every program shares.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use zerodds_config::{
    CommandLine, ConfigError, Loader, Origin, SystemLayout, env_config, loader_for, unknown_keys,
};

fn workdir(leaf: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("command_line")
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the work directory");
    dir
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

const TABLE: &[(&str, &str)] = &[
    ("T_BATCH", "transport.recv_batch"),
    ("T_PEERS", "discovery.peers"),
    ("T_MTU", "transport.mtu"),
];

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    let map: BTreeMap<&str, &str> = pairs.iter().copied().collect();
    move |name| map.get(name).map(|v| (*v).to_string())
}

fn is_list(path: &str) -> bool {
    path == "discovery.peers"
}

#[test]
fn the_flags_come_out_and_the_rest_stays_in_order() {
    let (cl, rest) = CommandLine::extract(args(&[
        "subscribe",
        "--config",
        "/x/a.yaml",
        "--topic",
        "T",
        "--set",
        "transport.recv_batch=8",
        "--set=discovery.peers=10.0.0.1",
        "--check-config",
        "--",
        "--set",
        "not.mine=1",
    ]))
    .unwrap();
    assert_eq!(cl.config_path(), Some(Path::new("/x/a.yaml")));
    assert!(cl.check());
    assert_eq!(
        cl.sets(),
        &[
            ("transport.recv_batch".to_string(), "8".to_string()),
            ("discovery.peers".to_string(), "10.0.0.1".to_string()),
        ]
    );
    assert_eq!(
        rest,
        args(&["subscribe", "--topic", "T", "--", "--set", "not.mine=1"])
    );
    let (inline, _) = CommandLine::extract(args(&["--config=/y.yaml"])).unwrap();
    assert_eq!(inline.config_path(), Some(Path::new("/y.yaml")));
}

#[test]
fn malformed_flags_are_refused_by_name() {
    for bad in [
        &["--config"][..],
        &["--config="][..],
        &["--set"][..],
        &["--set", "novalue"][..],
        &["--set", "=1"][..],
        &["--set", "a..b=1"][..],
        &["--set", "a.=1"][..],
        &["--set", "a.+b.c=1"][..],
        &["--set", "+=1"][..],
        &["--config", "a", "--config", "b"][..],
    ] {
        match CommandLine::extract(args(bad)) {
            Err(ConfigError::Usage(_)) => {}
            other => panic!("{bad:?} must be a usage error, got {other:?}"),
        }
    }
}

#[test]
fn a_file_wins_over_the_environment_and_a_flag_over_both() {
    let dir = workdir("precedence");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "transport:\n  recv_batch: 16\n").unwrap();

    let e = env(&[("T_BATCH", "4"), ("T_MTU", "1400")]);
    let lower = env_config(TABLE, &e, is_list).unwrap();
    let (cl, _) = CommandLine::extract(args(&["--config", base.to_str().unwrap()])).unwrap();
    let files = loader_for(&cl, "zerodds").load_onto(lower.clone()).unwrap();
    assert_eq!(files.get_str("transport.recv_batch"), Some("16"));
    assert_eq!(
        files.get_str("transport.mtu"),
        Some("1400"),
        "what no file sets, the environment still does"
    );
    assert_eq!(
        files.origin_of("transport.mtu"),
        Some(&Origin::Env("T_MTU".to_string()))
    );

    let (cl, _) = CommandLine::extract(args(&[
        "--config",
        base.to_str().unwrap(),
        "--set",
        "transport.recv_batch=32",
    ]))
    .unwrap();
    let mut top = loader_for(&cl, "zerodds").load_onto(lower).unwrap();
    cl.apply(&mut top).unwrap();
    assert_eq!(top.get_str("transport.recv_batch"), Some("32"));
    assert_eq!(
        top.origin_of("transport.recv_batch"),
        Some(&Origin::Cli("--set transport.recv_batch=32".to_string()))
    );
    assert_eq!(
        top.sources().first().map(String::as_str),
        Some("environment")
    );
    assert_eq!(
        top.sources().last().map(String::as_str),
        Some("command line")
    );
}

#[test]
fn a_list_from_the_environment_is_appended_to_by_a_drop_in_and_by_a_flag() {
    let dir = workdir("append");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "transport:\n  recv_batch: 1\n").unwrap();
    std::fs::create_dir_all(dir.join("zerodds.d")).unwrap();
    std::fs::write(
        dir.join("zerodds.d/10-site.yaml"),
        "discovery:\n  +peers:\n    - 10.0.0.2\n",
    )
    .unwrap();
    let lower = env_config(TABLE, env(&[("T_PEERS", "10.0.0.1, ")]), is_list).unwrap();
    let (cl, _) = CommandLine::extract(args(&[
        "--config",
        base.to_str().unwrap(),
        "--set",
        "+discovery.peers=10.0.0.3",
    ]))
    .unwrap();
    let mut config = loader_for(&cl, "zerodds").load_onto(lower).unwrap();
    cl.apply(&mut config).unwrap();
    let peers: Vec<&str> = match config.get("discovery.peers") {
        Some(zerodds_config::Node::Seq(items)) => items.iter().filter_map(|n| n.as_str()).collect(),
        other => panic!("peers must be a sequence, got {other:?}"),
    };
    assert_eq!(peers, ["10.0.0.1", "10.0.0.2", "10.0.0.3"]);
}

#[test]
fn an_explicit_config_must_exist_and_its_includes_are_checked_for_cycles() {
    let dir = workdir("explicit");
    let (cl, _) = CommandLine::extract(args(&[
        "--config",
        dir.join("missing.yaml").to_str().unwrap(),
    ]))
    .unwrap();
    assert!(matches!(
        loader_for(&cl, "zerodds").load(),
        Err(ConfigError::Io(..))
    ));

    std::fs::write(dir.join("a.yaml"), "include: b.yaml\n").unwrap();
    std::fs::write(dir.join("b.yaml"), "include: a.yaml\n").unwrap();
    let (cl, _) =
        CommandLine::extract(args(&["--config", dir.join("a.yaml").to_str().unwrap()])).unwrap();
    assert!(matches!(
        loader_for(&cl, "zerodds").load(),
        Err(ConfigError::IncludeCycle(_))
    ));
}

#[test]
fn without_config_the_platform_layout_is_read() {
    let layout = SystemLayout::for_stem("zerodds");
    let dir = SystemLayout::dir();
    assert_eq!(layout.base, dir.join("zerodds.yaml"));
    assert_eq!(layout.drop_ins[0], dir.join("zerodds.d"));
    #[cfg(target_os = "linux")]
    {
        assert_eq!(dir, Path::new("/etc/zerodds"));
        assert_eq!(layout.drop_ins[1], Path::new("/run/zerodds/zerodds.d"));
    }
    #[cfg(target_os = "macos")]
    assert_eq!(dir, Path::new("/usr/local/etc/zerodds"));
    #[cfg(windows)]
    assert!(dir.ends_with("ZeroDDS"));
    assert_eq!(
        SystemLayout::for_stem("ws-bridged").base,
        dir.join("ws-bridged.yaml")
    );
}

#[test]
fn unknown_keys_name_their_file_and_line() {
    let dir = workdir("unknown");
    let base = dir.join("zerodds.yaml");
    std::fs::write(
        &base,
        "transport:\n  recv_batch: 8\ndiscovery:\n  peer:\n    - 10.0.0.1\n",
    )
    .unwrap();
    let config = Loader::new().base(&base).load().unwrap();
    let unknown = unknown_keys(&config, |path| path == "transport.recv_batch");
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, "discovery.peer");
    assert_eq!(unknown[0].1.to_string(), format!("{}:5", base.display()));
}

/// A program flag whose value is a list entry of several settings
/// (`zerodds-webdds-bridged --type NAME=FILE`) appends one map; a `--set`
/// value is one scalar, and the reader refused `+types={name: …}` with
/// "expected a map of settings" (measured on the Linux test host).
#[test]
fn a_flag_appends_a_map_to_a_list_from_a_file_or_to_none() {
    let dir = workdir("entry");
    let base = dir.join("webdds.yaml");
    std::fs::write(&base, "types:\n  - name: A\n    idl_file: /a.idl\n").unwrap();
    let (mut cl, _) = CommandLine::extract(args(&["--config", base.to_str().unwrap()])).unwrap();
    cl.push_entry(
        "+types",
        &[("name", "M::Sensor"), ("idl_file", "/x.idl")],
        "--type M::Sensor=/x.idl",
    )
    .unwrap();
    let mut config = loader_for(&cl, "webdds").load().unwrap();
    cl.apply(&mut config).unwrap();
    let names = |config: &zerodds_config::Config| -> Vec<(String, String)> {
        match config.get("types") {
            Some(zerodds_config::Node::Seq(items)) => items
                .iter()
                .map(|item| match item {
                    zerodds_config::Node::Map(map) => (
                        map["name"].as_str().unwrap().to_string(),
                        map["idl_file"].as_str().unwrap().to_string(),
                    ),
                    other => panic!("a type entry must be a map, got {other:?}"),
                })
                .collect(),
            other => panic!("types must be a sequence, got {other:?}"),
        }
    };
    assert_eq!(
        names(&config),
        [
            ("A".to_string(), "/a.idl".to_string()),
            ("M::Sensor".to_string(), "/x.idl".to_string()),
        ]
    );
    match config.get("types") {
        Some(zerodds_config::Node::Seq(items)) => match &items[1] {
            zerodds_config::Node::Map(map) => assert_eq!(
                map["name"].origin().unwrap().to_string(),
                "command line `--type M::Sensor=/x.idl`"
            ),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }

    let mut alone = zerodds_config::Config::default();
    cl.apply(&mut alone).unwrap();
    assert_eq!(
        names(&alone),
        [("M::Sensor".to_string(), "/x.idl".to_string())]
    );
}

#[test]
fn an_entry_needs_an_append_path_and_plain_field_names() {
    let mut cl = CommandLine::default();
    assert!(matches!(
        cl.push_entry("types", &[("name", "A")], "--type A"),
        Err(ConfigError::Usage(_))
    ));
    assert!(matches!(
        cl.push_entry("+types", &[("a.b", "A")], "--type A"),
        Err(ConfigError::Usage(_))
    ));
    assert!(matches!(
        cl.push_entry("+a..types", &[("name", "A")], "--type A"),
        Err(ConfigError::Usage(_))
    ));
    assert!(
        cl.push_entry("+bridge.types", &[("name", "A")], "--type A")
            .is_ok()
    );
}

#[test]
fn a_secret_given_on_the_command_line_does_not_show_in_its_origin() {
    let (cl, _) = CommandLine::extract(args(&[
        "--set",
        "auth.bearer_token=hunter2",
        "--set",
        "tls.key_file=/k.pem",
    ]))
    .unwrap();
    let mut config = zerodds_config::Config::default();
    cl.apply(&mut config).unwrap();
    assert_eq!(
        config.get_str("auth.bearer_token"),
        Some("hunter2"),
        "the value is kept"
    );
    assert_eq!(
        config.origin_of("auth.bearer_token").unwrap().to_string(),
        "command line `--set auth.bearer_token=***`"
    );
    assert!(
        config
            .origin_of("tls.key_file")
            .unwrap()
            .to_string()
            .contains("/k.pem"),
        "a path to a key is not the key"
    );
    assert!(zerodds_config::is_secret_path("mqtt.password"));
    assert!(zerodds_config::is_secret_path("topics[0].psk"));
    assert!(zerodds_config::is_secret_path("auth.sasl_users.alice"));
    assert!(!zerodds_config::is_secret_path("tls.key_file"));
}

/// `--opt=value` in the rest comes back as `--opt value`; `--set=a=b` stays
/// a set; after `--` nothing is split.
#[test]
fn a_long_option_with_equals_is_split_in_the_rest() {
    let args = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    let (cl, rest) = zerodds_config::CommandLine::extract(args(&[
        "--domain=3",
        "--set=domain=4",
        "--bind",
        "0.0.0.0:1",
        "--topic=A=B",
        "--",
        "--x=y",
    ]))
    .unwrap();
    assert_eq!(cl.sets(), &[("domain".to_string(), "4".to_string())]);
    assert_eq!(
        rest,
        args(&[
            "--domain",
            "3",
            "--bind",
            "0.0.0.0:1",
            "--topic",
            "A=B",
            "--",
            "--x=y"
        ])
    );
}
