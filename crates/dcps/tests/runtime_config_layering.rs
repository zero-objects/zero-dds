// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Layered files are validated and reach the runtime without environment overrides.
//! Website walkthrough: `website/why-zerodds-for-ros/config-complexity.html`.
//! Its base/drop-in/include examples are backed by the cases in this module.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig, SETTINGS, setting_path};

fn workdir(leaf: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("runtime-config")
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the work directory");
    dir
}

fn source_from(dir: &Path, base: &str, drop_in: Option<(&str, &str)>) -> ConfigSource {
    let base_path = dir.join("zerodds.yaml");
    std::fs::write(&base_path, base).expect("the base file");
    let mut loader = zerodds_config::Loader::new().base(&base_path);
    if let Some((name, text)) = drop_in {
        let d = dir.join("zerodds.d");
        std::fs::create_dir_all(&d).expect("the drop-in directory");
        std::fs::write(d.join(name), text).expect("the drop-in");
        loader = loader.drop_in_dir(&d);
    }
    ConfigSource::with_files(loader.load().expect("the configuration loads"))
}

#[test]
fn a_file_sets_a_value_the_built_in_default_would_otherwise_hold() {
    let dir = workdir("file");
    let source = source_from(&dir, "reader:\n  max_samples_per_proxy: 4096\n", None);
    let config = RuntimeConfig::from_source(&source).expect("valid settings");
    assert_eq!(config.max_samples_per_proxy, 4096);

    // Without the file the compiled-in value stands, so the assertion above
    // is about the file and not about a coincidence.
    let bare = RuntimeConfig::default();
    assert_ne!(bare.max_samples_per_proxy, 4096);
}

#[test]
fn a_drop_in_overrides_the_base_file() {
    let dir = workdir("dropin");
    let source = source_from(
        &dir,
        "transport:\n  recv_batch: 8\n",
        Some(("50-site.yaml", "transport:\n  recv_batch: 64\n")),
    );
    assert_eq!(
        RuntimeConfig::from_source(&source).unwrap().recv_batch_size,
        64
    );
}

#[test]
fn a_nested_directory_include_reaches_the_runtime_and_retains_its_origin() {
    let dir = workdir("included-dirs");
    std::fs::create_dir_all(dir.join("site.d")).expect("site directory");
    std::fs::create_dir_all(dir.join("shared.d")).expect("shared directory");
    std::fs::write(dir.join("zerodds.yaml"), "include: site.d\n").expect("base");
    std::fs::write(dir.join("site.d/10-shared.yaml"), "include: ../shared.d\n").expect("include");
    std::fs::write(
        dir.join("shared.d/10-default.yaml"),
        "reader:\n  max_samples_per_proxy: 4096\n",
    )
    .expect("default");
    let final_path = dir.join("site.d/90-project.yaml");
    std::fs::write(&final_path, "reader:\n  max_samples_per_proxy: 8192\n")
        .expect("project override");
    let files = zerodds_config::Loader::new()
        .base(dir.join("zerodds.yaml"))
        .load()
        .expect("nested directories load");
    assert_eq!(
        files.origin_of("reader.max_samples_per_proxy"),
        Some(&zerodds_config::Origin::File {
            path: final_path.display().to_string(),
            line: 2
        })
    );
    let source = ConfigSource::with_files(files);
    assert_eq!(
        RuntimeConfig::from_source(&source)
            .unwrap()
            .max_samples_per_proxy,
        8192
    );
}

fn cli(list: &[&str]) -> zerodds_config::CommandLine {
    zerodds_config::CommandLine::extract(list.iter().map(|s| (*s).to_string()))
        .expect("valid flags")
        .0
}

fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| (*v).to_string())
    }
}

#[test]
fn the_environment_sets_what_no_file_sets_and_yields_to_every_file() {
    let dir = workdir("env");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "transport:\n  recv_batch: 8\n").unwrap();
    let env = env_of(&[
        ("ZERODDS_RECV_BATCH", "128"),
        ("ZERODDS_TICK_PERIOD_MS", "7"),
    ]);
    let source =
        ConfigSource::from_command_line_with_env(&cli(&["--config", base.to_str().unwrap()]), env)
            .unwrap();
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(
        config.recv_batch_size, 8,
        "the file wins over the environment"
    );
    assert_eq!(
        config.tick_period,
        std::time::Duration::from_millis(7),
        "what no file sets, the environment sets over the default"
    );
    assert!(format!("{}", source.origin_of("ZERODDS_RECV_BATCH")).contains("zerodds.yaml"));
    assert_eq!(
        source.origin_of("runtime.tick_period_ms"),
        zerodds_config::Origin::Env("ZERODDS_TICK_PERIOD_MS".to_string())
    );
}

#[test]
fn a_flag_wins_over_the_file_and_the_environment() {
    let dir = workdir("flag");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "transport:\n  recv_batch: 8\n").unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&[
            "--config",
            base.to_str().unwrap(),
            "--set",
            "transport.recv_batch=64",
        ]),
        env_of(&[("ZERODDS_RECV_BATCH", "128")]),
    )
    .unwrap();
    assert_eq!(
        RuntimeConfig::from_source(&source).unwrap().recv_batch_size,
        64
    );
    assert_eq!(
        source.origin_of("transport.recv_batch").to_string(),
        "command line `--set transport.recv_batch=64`"
    );
}

#[test]
fn an_invalid_value_names_its_layer() {
    let dir = workdir("bad-layers");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "\n").unwrap();
    let from_env = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap()]),
        env_of(&[("ZERODDS_RECV_BATCH", "zero")]),
    )
    .unwrap();
    let error = RuntimeConfig::from_source(&from_env)
        .unwrap_err()
        .to_string();
    assert!(error.contains("$ZERODDS_RECV_BATCH"), "{error}");

    let from_flag = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap(), "--set", "tsn.pcp=9"]),
        env_of(&[]),
    )
    .unwrap();
    let error = RuntimeConfig::from_source(&from_flag)
        .unwrap_err()
        .to_string();
    assert!(error.contains("--set tsn.pcp=9"), "{error}");
}

#[test]
fn a_flag_for_a_setting_nobody_reads_is_refused() {
    let error = ConfigSource::from_command_line_with_env(
        &cli(&["--set", "transport.recv_bacth=8"]),
        env_of(&[]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("recv_bacth"), "{error}");
    assert!(
        ConfigSource::from_command_line_with_env(
            &cli(&["--set", "+discovery.peers=10.0.0.1"]),
            env_of(&[]),
        )
        .is_ok(),
        "an append to a known list is known"
    );
}

#[test]
fn the_check_report_lists_origins_and_fails_on_unknown_keys_and_bad_values() {
    let dir = workdir("check");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "transport:\n  recv_batch: 8\n").unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap(), "--set", "tsn.vlan=12"]),
        env_of(&[("ZERODDS_TICK_PERIOD_MS", "7")]),
    )
    .unwrap();
    let report = source.check_report(&|_| false).expect("valid");
    assert!(report.contains("transport.recv_batch = 8  ("), "{report}");
    assert!(report.contains("zerodds.yaml:2)"), "{report}");
    assert!(
        report.contains("runtime.tick_period_ms = 7  (environment $ZERODDS_TICK_PERIOD_MS)"),
        "{report}"
    );
    assert!(
        report.contains("tsn.vlan = 12  (command line `--set tsn.vlan=12`)"),
        "{report}"
    );
    assert!(
        report.contains("tsn.pcp = 0 [default]  (built-in default)"),
        "{report}"
    );

    std::fs::write(
        &base,
        "transport:\n  recv_batch: 0\ndiscovery:\n  peer:\n    - 10.0.0.1\n",
    )
    .unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap()]),
        env_of(&[]),
    )
    .unwrap();
    let report = source.check_report(&|_| false).expect_err("invalid");
    assert!(report.contains("invalid transport.recv_batch"), "{report}");
    assert!(
        report.contains("unknown setting discovery.peer"),
        "{report}"
    );
    assert!(
        source
            .check_report(&|path| path == "discovery.peer")
            .is_err(),
        "an application key does not excuse an invalid value"
    );
}

/// One `zerodds.yaml` serves every program on a host; the RMW's `ros2.*`
/// section does not make a bridge's check fail. The RMW checks it.
#[test]
fn the_rmw_section_is_not_an_unknown_key_for_other_programs() {
    let dir = workdir("rmw-section");
    let source = source_from(
        &dir,
        "ros2:\n  topic_mangling: none\n  qos_profiles:\n    fast:\n      depth: 1\n",
        None,
    );
    assert!(source.unknown_keys(&|_| false).is_empty());
    assert!(source.check_report(&|_| false).is_ok());
    let source = source_from(&dir, "ros3:\n  topic_mangling: none\n", None);
    assert_eq!(
        source.unknown_keys(&|_| false).len(),
        1,
        "only ros2.* is set aside"
    );
}

#[test]
fn the_origin_of_a_value_names_the_file_and_line_that_set_it() {
    let dir = workdir("origin");
    let source = source_from(
        &dir,
        "transport:\n  recv_batch: 8\n",
        Some(("50-site.yaml", "transport:\n  recv_batch: 64\n")),
    );
    let origin = format!("{}", source.origin_of("ZERODDS_RECV_BATCH"));
    assert!(
        origin.contains("50-site.yaml") && origin.ends_with(":2"),
        "the drop-in set it on line 2, said {origin}"
    );
    // A setting nobody wrote reports the built-in default rather than
    // inventing a source.
    let untouched = format!("{}", source.origin_of("ZERODDS_TSN_VLAN"));
    assert_eq!(untouched, "built-in default");
}

#[test]
fn file_only_settings_and_appended_lists_reach_the_typed_runtime() {
    let dir = workdir("typed");
    let source = source_from(
        &dir,
        "transport:\n  user_transport: UDPv6\nwire:\n  rtps_dialect: cyclone\n  data_representation_offer:\n    - XCDR2\ndiscovery:\n  no_multicast: false\n  server: false\n  peers:\n    - 127.0.0.1\ndiagnostics:\n  phase_timing: true\n",
        Some((
            "20-site.yaml",
            "discovery:\n  +peers:\n    - 127.0.0.2:7410\n",
        )),
    );
    let config = RuntimeConfig::from_source(&source).expect("typed file values");
    assert!(matches!(
        config.user_transport,
        Some(zerodds_dcps::runtime::UserTransportKind::UdpV6)
    ));
    assert_eq!(config.data_representation_offer, vec![2]);
    assert!(config.rtps_dialect_unknown.is_none());
    assert_eq!(
        config.rtps_behaviour.default_behaviour(),
        zerodds_rtps::dialect::RtpsDialect::Cyclone.behaviour()
    );
    assert!(config.spdp_multicast_send);
    assert!(!config.discovery_server);
    assert!(config.phase_timing);
    assert_eq!(
        config.initial_peer_specs,
        vec!["127.0.0.1", "127.0.0.2:7410"]
    );
    let origins = source.origins();
    assert!(
        origins
            .iter()
            .any(|(path, origin)| path == "discovery.peers[0]"
                && origin.to_string().contains("zerodds.yaml"))
    );
    assert!(
        origins
            .iter()
            .any(|(path, origin)| path == "discovery.peers[1]"
                && origin.to_string().contains("20-site.yaml"))
    );
}

/// `wire.codec_dialect` from a file reaches the runtime config and, through
/// it, the reflective codecs; unset it is the specification's.
#[test]
fn the_codec_dialect_reaches_the_typed_runtime() {
    let dir = workdir("codec-dialect");
    let config = RuntimeConfig::from_source(&source_from(
        &dir,
        "wire:\n  codec_dialect: fastdds\n",
        None,
    ))
    .expect("typed file values");
    assert_eq!(config.codec_dialect, zerodds_ast_layout::Dialect::FastDds);
    assert!(config.codec_dialect_unknown.is_none());
    let config =
        RuntimeConfig::from_source(&source_from(&dir, "wire:\n  big_endian: false\n", None))
            .expect("typed file values");
    assert_eq!(
        config.codec_dialect,
        zerodds_ast_layout::Dialect::Specification
    );
}

/// `wire.encapsulation_ids`, `wire.pl_cdr1_headers` and `wire.pl_cdr1_reading`
/// (DDS-XTypes 1.3 Table 39, §7.4.1.2, §7.4.1.2.1): the defaults are the
/// clauses' readings that the measured vendors write, a file and the
/// environment set each, and a misspelling is refused.
#[test]
fn the_encapsulation_and_pl_cdr1_settings_default_to_the_vendors_and_are_set_by_file_and_environment()
 {
    use zerodds_ast_layout::{PlCdr1HeaderPolicy, PlCdr1Reading};
    use zerodds_wire_codec::value::EncapsulationIds;
    let d = RuntimeConfig::default();
    assert_eq!(d.encapsulation_ids, EncapsulationIds::Table60);
    assert_eq!(d.pl_cdr1_headers, None);
    assert_eq!(d.pl_cdr1_reading, PlCdr1Reading::Strict);

    let dir = workdir("encapsulation-pl-cdr1");
    let source = source_from(
        &dir,
        "wire:\n  encapsulation_ids: table-39\n  pl_cdr1_headers: short-rounded-length\n  pl_cdr1_reading: tolerant\n",
        None,
    );
    let c = RuntimeConfig::from_source(&source).expect("typed file values");
    assert_eq!(c.encapsulation_ids, EncapsulationIds::Table39);
    assert_eq!(
        c.pl_cdr1_headers,
        Some(PlCdr1HeaderPolicy::ShortRoundedLength)
    );
    assert_eq!(c.pl_cdr1_reading, PlCdr1Reading::Tolerant);

    let source = ConfigSource::from_command_line_with_env(
        &cli(&[]),
        env_of(&[
            ("ZERODDS_WIRE_ENCAPSULATION_IDS", "table-39"),
            ("ZERODDS_WIRE_PL_CDR1_HEADERS", "always-extended"),
            ("ZERODDS_WIRE_PL_CDR1_READING", "tolerant"),
        ]),
    )
    .unwrap();
    let c = RuntimeConfig::from_source(&source).expect("typed environment values");
    assert_eq!(c.encapsulation_ids, EncapsulationIds::Table39);
    assert_eq!(c.pl_cdr1_headers, Some(PlCdr1HeaderPolicy::AlwaysExtended));
    assert_eq!(c.pl_cdr1_reading, PlCdr1Reading::Tolerant);

    let dialect = source_from(&dir, "wire:\n  pl_cdr1_headers: dialect\n", None);
    assert_eq!(
        RuntimeConfig::from_source(&dialect)
            .unwrap()
            .pl_cdr1_headers,
        None
    );
    for bad in [
        "wire:\n  encapsulation_ids: table-40\n",
        "wire:\n  pl_cdr1_headers: rounded\n",
        "wire:\n  pl_cdr1_reading: lenient\n",
    ] {
        assert!(
            RuntimeConfig::from_source(&source_from(&dir, bad, None)).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn invalid_values_fail_before_runtime_creation() {
    let dir = workdir("invalid");
    for text in [
        "transport:\n  recv_batch: 0\n",
        "transport:\n  recv_batch: abc\n",
        "transport:\n  recv_batch: true\n",
        "transport:\n  shm_max_datagram: 0\n",
        "transport:\n  user_transport: nonexistent\n",
        "wire:\n  rtps_dialect: typo\n",
        "wire:\n  codec_dialect: fastdd\n",
        "wire:\n  data_representation_offer: XCDR3\n",
        "discovery:\n  peers:\n    - invalid-address\n",
        "diagnostics:\n  phase_timing: yesplease\n",
        "reader:\n  max_samples_per_proxy:\n    - 12\n",
    ] {
        let source = source_from(&dir, text, None);
        let error = RuntimeConfig::from_source(&source).expect_err("invalid setting must fail");
        assert!(error.to_string().contains("zerodds.yaml"));
    }
    assert!(ConfigSource::from_path(dir.join("missing.yaml")).is_err());
}

#[test]
fn every_setting_the_runtime_reads_has_a_path_and_no_two_share_one() {
    use std::collections::BTreeSet;
    let mut paths = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (env, path) in SETTINGS {
        assert!(
            env.starts_with("ZERODDS_"),
            "{env} is not a ZERODDS_ variable"
        );
        assert!(path.contains('.'), "{path} has no section");
        assert!(names.insert(*env), "{env} is listed twice");
        assert!(paths.insert(*path), "{path} is claimed twice");
        assert_eq!(setting_path(env), Some(*path));
    }
    assert!(SETTINGS.len() >= 23, "the table lost entries");
    assert_eq!(setting_path("ZERODDS_NOT_A_SETTING"), None);
}

/// The table has to cover what the runtime actually reads. A variable that
/// `from_source` asks for and the table does not name has no file spelling,
/// and nothing else would notice.
#[test]
fn the_table_covers_every_variable_the_runtime_asks_for() {
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runtime.rs"))
            .expect("the runtime source");
    let body = source
        .split_once("fn from_validated_source(source: &ConfigSource) -> Self {")
        .expect("from_source is there")
        .1;
    let mut missing = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("source.var(\"") {
        rest = &rest[at + 12..];
        let Some(end) = rest.find('"') else { break };
        let name = &rest[..end];
        if setting_path(name).is_none() && !SETTINGS.iter().any(|(_, path)| *path == name) {
            missing.push(name.to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "read by the runtime and missing from SETTINGS: {missing:?}"
    );
}

#[test]
fn the_writer_byte_order_and_the_durability_directory_are_settings() {
    let dir = workdir("writer-settings");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "durability:\n  dir: /var/lib/zerodds/durability\n").unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap()]),
        env_of(&[("ZERODDS_WIRE_BIG_ENDIAN", "1")]),
    )
    .unwrap();
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert!(config.wire_big_endian);
    assert_eq!(
        config.durability_dir.as_deref(),
        Some(Path::new("/var/lib/zerodds/durability"))
    );

    let empty = dir.join("empty.yaml");
    std::fs::write(&empty, "\n").unwrap();
    for (name, value) in [
        ("ZERODDS_WIRE_BIG_ENDIAN", "yes"),
        ("ZERODDS_DURABILITY_DIR", " "),
    ] {
        let source = ConfigSource::from_command_line_with_env(
            &cli(&["--config", empty.to_str().unwrap()]),
            env_of(&[(name, value)]),
        )
        .unwrap();
        let error = RuntimeConfig::from_source(&source)
            .expect_err("an unreadable value must be refused, not read as off")
            .to_string();
        assert!(error.contains(name), "{error}");
    }
    let off = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap()]),
        env_of(&[("ZERODDS_WIRE_BIG_ENDIAN", "0")]),
    )
    .unwrap();
    assert!(
        !RuntimeConfig::from_source(&off).unwrap().wire_big_endian,
        "0 is off; any set value used to mean on"
    );
}

/// A28: `interop.rti_group_coherent` was retired with the RTI GROUP-coherent
/// writer emulation it gated. A file or `--set` that still names it is
/// refused, not silently ignored — the same generic unknown-setting path a
/// typo takes (`is_runtime_path` no longer lists it).
#[test]
fn the_retired_rti_group_coherent_setting_is_refused_by_name() {
    let dir = workdir("retired-rti-group-coherent");
    let source = source_from(&dir, "interop:\n  rti_group_coherent: true\n", None);
    let report = source
        .check_report(&|_| false)
        .expect_err("a retired key must fail the check");
    assert!(
        report.contains("unknown setting interop.rti_group_coherent"),
        "{report}"
    );
    let err = ConfigSource::from_command_line_with_env(
        &cli(&["--set", "interop.rti_group_coherent=true"]),
        env_of(&[]),
    )
    .expect_err("--set on a retired key must be refused");
    assert!(
        err.to_string().contains("interop.rti_group_coherent"),
        "{err}"
    );
}

#[test]
fn delivery_mode_and_time_based_filter_are_typed_settings() {
    use zerodds_dcps::runtime::DeliveryMode;
    let dir = workdir("ros2-settings");
    let base = dir.join("zerodds.yaml");
    std::fs::write(
        &base,
        "delivery:\n  mode: raw-same-host\nros2:\n  time_based_filter:\n    - 0.1\n    - /scan=0.5\n",
    )
    .unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&[
            "--config",
            base.to_str().unwrap(),
            "--set",
            "+ros2.time_based_filter=/image=0",
        ]),
        env_of(&[]),
    )
    .unwrap();
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(config.delivery_mode, DeliveryMode::RawSameHost);
    let ms = |m| Some(std::time::Duration::from_millis(m));
    assert_eq!(
        config.time_based_filter_for("/scan"),
        ms(500),
        "per topic wins"
    );
    assert_eq!(
        config.time_based_filter_for("/odom"),
        ms(100),
        "the default"
    );
    assert_eq!(config.time_based_filter_for("/image"), None, "0 disables");
    assert_eq!(
        RuntimeConfig::default().time_based_filter_for("/scan"),
        None
    );

    let empty = dir.join("empty.yaml");
    std::fs::write(&empty, "\n").unwrap();
    for (name, value) in [
        ("ZERODDS_DELIVERY_MODE", "raw_same_host"),
        ("ZERODDS_DELIVERY_MODE", "shm"),
        ("ZERODDS_TIME_BASED_FILTER", "nonsense"),
        ("ZERODDS_TIME_BASED_FILTER", "-1"),
        ("ZERODDS_TIME_BASED_FILTER", "/scan=fast"),
        ("ZERODDS_TIME_BASED_FILTER", "=0.1"),
    ] {
        let source = ConfigSource::from_command_line_with_env(
            &cli(&["--config", empty.to_str().unwrap()]),
            env_of(&[(name, value)]),
        )
        .unwrap();
        let error = RuntimeConfig::from_source(&source)
            .expect_err("a value that does not parse must be refused")
            .to_string();
        assert!(error.contains(name), "{name}={value}: {error}");
    }
}

#[test]
fn discovery_periods_threads_and_transports_are_settings() {
    use std::time::Duration;
    let dir = workdir("struct-fields");
    let base = dir.join("zerodds.yaml");
    std::fs::write(
        &base,
        "discovery:\n  spdp_period_ms: 250\n  participant_lease_duration_ms: 30000\n  wlp_period_ms: 1000\n\
         transport:\n  user_transports:\n    - UDPv4\n    - TCPv4\n\
         threads:\n  recv_priority: 80\n  recv_cpus:\n    - 2\n    - 3\n  extra_recv: 2\n\
         wire:\n  data_representation_match: strict\n",
    )
    .unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&[
            "--config",
            base.to_str().unwrap(),
            "--set",
            "threads.tick_cpus=1",
        ]),
        env_of(&[("ZERODDS_TICK_THREAD_PRIORITY", "70")]),
    )
    .unwrap();
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(config.spdp_period, Duration::from_millis(250));
    assert_eq!(config.participant_lease_duration, Duration::from_secs(30));
    assert_eq!(config.wlp_period, Duration::from_secs(1));
    assert_eq!(config.user_transports.len(), 2);
    assert_eq!(config.recv_thread_priority, Some(80));
    assert_eq!(config.tick_thread_priority, Some(70));
    assert_eq!(config.recv_thread_cpus, Some(vec![2, 3]));
    assert_eq!(config.tick_thread_cpus, Some(vec![1]));
    assert_eq!(config.extra_recv_threads, 2);
    assert_eq!(
        config.data_rep_match_mode,
        zerodds_rtps::publication_data::data_representation::DataRepMatchMode::Strict
    );

    let defaults = RuntimeConfig::default();
    assert_eq!(defaults.spdp_period, Duration::from_secs(30));
    assert_eq!(
        defaults.participant_lease_duration,
        Duration::from_secs(100)
    );
    assert_eq!(defaults.recv_thread_cpus, None);

    let empty = dir.join("empty.yaml");
    std::fs::write(&empty, "\n").unwrap();
    for (name, value) in [
        ("ZERODDS_SPDP_PERIOD_MS", "0"),
        ("ZERODDS_PARTICIPANT_LEASE_MS", "soon"),
        ("ZERODDS_USER_TRANSPORTS", "UDPv4,carrier-pigeon"),
        ("ZERODDS_RECV_THREAD_PRIORITY", "100"),
        ("ZERODDS_RECV_THREAD_CPUS", "2,x"),
        ("ZERODDS_EXTRA_RECV_THREADS", "1000"),
        ("ZERODDS_DATA_REPR_MATCH", "loose"),
        ("ZERODDS_TYPE_CONSISTENCY_ABSENT", "maybe"),
    ] {
        let source = ConfigSource::from_command_line_with_env(
            &cli(&["--config", empty.to_str().unwrap()]),
            env_of(&[(name, value)]),
        )
        .unwrap();
        let error = RuntimeConfig::from_source(&source)
            .expect_err("a value that does not parse must be refused")
            .to_string();
        assert!(error.contains(name), "{name}={value}: {error}");
    }
}

#[test]
fn interface_bindings_come_from_a_file_and_are_checked_entry_by_entry() {
    let dir = workdir("bindings");
    let base = dir.join("zerodds.yaml");
    std::fs::write(
        &base,
        concat!(
            "transport:\n",
            "  interface_bindings:\n",
            "    - name: lo\n",
            "      bind: 127.0.0.1\n",
            "      kind: loopback\n",
            "      subnet: 127.0.0.0/8\n",
            "    - name: wan\n",
            "      bind: 0.0.0.0:7410\n",
            "      kind: wan\n",
            "      subnet: 0.0.0.0/0\n",
            "      default: true\n",
        ),
    )
    .unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&["--config", base.to_str().unwrap()]),
        env_of(&[]),
    )
    .unwrap();
    let report = source.check_report(&|_| false);
    #[cfg(feature = "security")]
    {
        let config = RuntimeConfig::from_source(&source).unwrap();
        assert_eq!(config.interface_bindings.len(), 2);
        assert_eq!(config.interface_bindings[1].bind_port, 7410);
        assert!(config.interface_bindings[1].default);
        let report = report.expect("valid");
        assert!(
            report.contains("transport.interface_bindings = 2 entries"),
            "{report}"
        );

        std::fs::write(
            &base,
            "transport:\n  interface_bindings:\n    - name: lo\n      bind: 127.0.0.1\n      kind: loopbak\n      subnet: 127.0.0.0/8\n",
        )
        .unwrap();
        let source = ConfigSource::from_command_line_with_env(
            &cli(&["--config", base.to_str().unwrap()]),
            env_of(&[]),
        )
        .unwrap();
        let error = RuntimeConfig::from_source(&source).unwrap_err().to_string();
        assert!(error.contains("entry 0: kind `loopbak`"), "{error}");
        assert!(error.contains("zerodds.yaml:"), "{error}");
    }
    #[cfg(not(feature = "security"))]
    {
        let error = RuntimeConfig::from_source(&source).unwrap_err().to_string();
        assert!(error.contains("`security` feature"), "{error}");
        assert!(report.is_err());
    }
}

#[test]
fn the_udp_socket_options_come_from_every_layer() {
    let dir = workdir("udp");
    let base = dir.join("zerodds.yaml");
    std::fs::write(
        &base,
        "transport:\n  udp:\n    connected_cache: false\n    recv_buffer: 0\n",
    )
    .unwrap();
    let source = ConfigSource::from_command_line_with_env(
        &cli(&[
            "--config",
            base.to_str().unwrap(),
            "--set",
            "transport.udp.cache_max_entries=4",
        ]),
        env_of(&[
            ("ZERODDS_UDP_CACHE_SNDBUF", "65536"),
            ("ZERODDS_UDP_DONTROUTE", "1"),
        ]),
    )
    .unwrap();
    let udp = RuntimeConfig::from_source(&source).unwrap().udp;
    assert!(!udp.connected_cache_enabled);
    assert_eq!(udp.recv_buffer_size, None, "0 leaves the kernel default");
    assert_eq!(udp.connected_cache_max_entries, 4);
    assert_eq!(udp.connected_cache_sndbuf, Some(65536));
    assert!(udp.dont_route);

    let bare = RuntimeConfig::default().udp;
    assert!(bare.connected_cache_enabled);
    assert_eq!(bare.connected_cache_max_entries, 16);
    assert_eq!(bare.recv_buffer_size, Some(1024 * 1024));
}

#[test]
fn a_bad_udp_value_is_refused_instead_of_falling_back() {
    for (name, value) in [
        ("ZERODDS_UDP_CACHE_MAX", "0"),
        ("ZERODDS_UDP_CACHE_MAX", "many"),
        ("ZERODDS_UDP_RCVBUF", "1M"),
        ("ZERODDS_UDP_CACHE_ENABLE", "yes"),
    ] {
        let source =
            ConfigSource::from_command_line_with_env(&cli(&[]), env_of(&[(name, value)])).unwrap();
        let error = RuntimeConfig::from_source(&source).unwrap_err().to_string();
        assert!(error.contains(name), "{name}={value}: {error}");
    }
}

/// C2(6) — `time.source`: `system` (default) always validates; `tai`
/// validates only where `CLOCK_TAI` is actually reachable (Linux + `std`) —
/// on any other build/host it is refused, loudly, rather than silently
/// running on the system clock while a config file claims otherwise. An
/// unknown spelling is refused too.
#[test]
fn time_source_tai_is_refused_off_linux_system_always_validates() {
    let source = ConfigSource::from_command_line_with_env(
        &cli(&[]),
        env_of(&[("ZERODDS_TIME_SOURCE", "system")]),
    )
    .unwrap();
    RuntimeConfig::from_source(&source).expect("system always validates");

    for value in ["tai", "bogus"] {
        let source = ConfigSource::from_command_line_with_env(
            &cli(&[]),
            env_of(&[("ZERODDS_TIME_SOURCE", value)]),
        )
        .unwrap();
        let result = RuntimeConfig::from_source(&source);
        if value == "tai" && cfg!(all(feature = "std", target_os = "linux")) {
            result.expect("tai validates on Linux+std");
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("time.source"), "{value}: {error}");
        }
    }
}

/// The shipped `zerodds.yaml.example` is valid, holds only the built-in
/// defaults, and it and `zerodds.yaml(5)` name every runtime setting with
/// its environment variable, so neither falls behind `SETTINGS`.
#[test]
fn the_shipped_example_and_the_man_page_name_every_setting() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("packaging/linux/configs/zerodds.yaml.example");
    let files = zerodds_config::Loader::new()
        .base(&example)
        .load()
        .expect("the example loads");
    let source = ConfigSource::with_files(files);
    let report = source.check_report(&|_| false);
    assert!(report.is_ok(), "{report:?}");
    let config = RuntimeConfig::from_source(&source).expect("valid settings");
    assert_eq!(
        format!("{config:?}"),
        format!("{:?}", RuntimeConfig::default()),
        "the example holds the defaults"
    );

    let example_text = std::fs::read_to_string(&example).expect("the example");
    let man = std::fs::read_to_string(root.join("man/man5/zerodds.yaml.5")).expect("the page");
    for (env, path) in SETTINGS {
        let leaf = path.rsplit('.').next().expect("a leaf");
        assert!(man.contains(&format!(".B {path}\n")), "man page: {path}");
        assert!(man.contains(&format!("[ {env} ]")), "man page: {env}");
        if !path.starts_with("ros2.") {
            assert!(example_text.contains(leaf), "example: {path}");
            assert!(example_text.contains(env), "example: {env}");
        }
    }
}

/// DDS-XTypes 1.3 §7.6.3.1.1 and §7.6.3.4.1: both defaults are the clause's,
/// and a peer that needs the older behaviour has a named setting for each.
#[test]
fn the_xtypes_defaults_are_the_clauses_and_each_has_a_named_way_back() {
    use zerodds_dcps::runtime::AbsentTypeConsistency;
    use zerodds_rtps::publication_data::data_representation::DataRepMatchMode;
    let defaults = RuntimeConfig::default();
    assert_eq!(defaults.data_rep_match_mode, DataRepMatchMode::Strict);
    assert_eq!(
        defaults.absent_type_consistency,
        AbsentTypeConsistency::Disallow
    );
    let source = ConfigSource::from_command_line_with_env(
        &cli(&[]),
        env_of(&[
            ("ZERODDS_DATA_REPR_MATCH", "tolerant"),
            ("ZERODDS_TYPE_CONSISTENCY_ABSENT", "allow"),
        ]),
    )
    .unwrap();
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(config.data_rep_match_mode, DataRepMatchMode::Tolerant);
    assert_eq!(config.absent_type_consistency, AbsentTypeConsistency::Allow);
}

/// DDSI-RTPS 2.5 §8.4.7.1 heartbeatPeriod, nackResponseDelay, §8.4.10.1
/// heartbeatResponseDelay and the SPDP period are settings, in a file, by
/// environment variable, and the built-in values hold without them.
#[test]
fn the_protocol_timing_parameters_are_settings() {
    use std::time::Duration;
    let defaults = RuntimeConfig::default();
    // The spec's values are the defaults (DDSI-RTPS 2.5 §8.4.7.1, §8.4.10.1,
    // §9.6.2.4).
    assert_eq!(defaults.nack_response_delay, Duration::from_millis(200));
    assert_eq!(defaults.nack_suppression_duration, Duration::ZERO);
    assert_eq!(
        defaults.reader_heartbeat_response_delay,
        Duration::from_millis(500)
    );
    assert_eq!(
        defaults.reader_heartbeat_suppression_duration,
        Duration::ZERO
    );
    assert_eq!(defaults.spdp_period, Duration::from_secs(30));
    assert_eq!(
        defaults.writer_heartbeat_period,
        zerodds_rtps::reliable_writer::DEFAULT_HEARTBEAT_PERIOD
    );
    assert_eq!(
        defaults.reader_heartbeat_response_delay,
        zerodds_rtps::reliable_reader::DEFAULT_HEARTBEAT_RESPONSE_DELAY
    );
    let dir = workdir("timing");
    let source = source_from(
        &dir,
        "writer:\n  heartbeat_period_ms: 250\n  nack_response_delay_ms: 200\n\
         reader:\n  heartbeat_response_delay_ms: 500\n  heartbeat_suppression_duration_ms: 40\n\
         discovery:\n  spdp_period_ms: 30000\n",
        None,
    );
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(config.writer_heartbeat_period, Duration::from_millis(250));
    assert_eq!(config.nack_response_delay, Duration::from_millis(200));
    assert_eq!(
        config.reader_heartbeat_response_delay,
        Duration::from_millis(500)
    );
    assert_eq!(
        config.reader_heartbeat_suppression_duration,
        Duration::from_millis(40)
    );
    assert_eq!(config.spdp_period, Duration::from_secs(30));
    // A heartbeat period of 0 is no period.
    let bad = source_from(&dir, "writer:\n  heartbeat_period_ms: 0\n", None);
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// DDSI-RTPS 2.5 §8.4.14.1: the message size of the UDP transport is a
/// setting, in a file and by environment variable; a size a datagram cannot
/// have is refused.
#[test]
fn the_udp_message_size_is_a_setting() {
    assert_eq!(RuntimeConfig::default().udp.max_message_size, 1472);
    let dir = workdir("udp-message-size");
    let source = source_from(
        &dir,
        "transport:\n  udp:\n    max_message_size: 9000\n",
        None,
    );
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(config.udp.max_message_size, 9000);
    for bad in ["10", "65508", "many"] {
        let text = format!("transport:\n  udp:\n    max_message_size: {bad}\n");
        let source = source_from(&dir, &text, None);
        assert!(RuntimeConfig::from_source(&source).is_err(), "{bad}");
    }
}

/// DDSI-RTPS 2.5 §9.3.2.4: the TCP locator kinds are ZeroDDS's own by
/// default; `transport.tcp_locator_kinds: fast-dds` selects 4 and 8.
#[test]
fn the_tcp_locator_kinds_are_a_setting() {
    assert!(!RuntimeConfig::default().tcp_locator_kinds_fast_dds);
    let dir = workdir("tcp-kinds");
    let source = source_from(&dir, "transport:\n  tcp_locator_kinds: fast-dds\n", None);
    assert!(
        RuntimeConfig::from_source(&source)
            .unwrap()
            .tcp_locator_kinds_fast_dds
    );
    let source = source_from(&dir, "transport:\n  tcp_locator_kinds: vendor\n", None);
    assert!(
        !RuntimeConfig::from_source(&source)
            .unwrap()
            .tcp_locator_kinds_fast_dds
    );
    let bad = source_from(&dir, "transport:\n  tcp_locator_kinds: other\n", None);
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// DDSI-RTPS 2.5 §9.6.2.3: PB, DG, PG and d0 to d3 are settings, in a file
/// and by environment variable; the defaults are the spec's values.
#[test]
fn the_port_mapping_parameters_are_settings() {
    use zerodds_rtps::wire_types::PortMapping;
    assert_eq!(RuntimeConfig::default().port_mapping, PortMapping::SPEC);
    let dir = workdir("ports");
    let source = source_from(
        &dir,
        "ports:\n  port_base: 20000\n  domain_gain: 100\n  participant_gain: 4\n  d0: 5\n  d1: 6\n  d2: 7\n  d3: 8\n",
        None,
    );
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(
        config.port_mapping,
        PortMapping {
            port_base: 20000,
            domain_gain: 100,
            participant_gain: 4,
            d0: 5,
            d1: 6,
            d2: 7,
            d3: 8,
        }
    );
    // A value above 65535 is no port.
    let bad = source_from(&dir, "ports:\n  port_base: 70000\n", None);
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// DDSI-RTPS 2.5 §8.3.7.4 gives no threshold for CLOCK_SKEW_DETECTED: the
/// detection is a setting and off by default.
#[test]
fn the_clock_skew_threshold_is_a_setting_and_off_by_default() {
    assert_eq!(RuntimeConfig::default().clock_skew_threshold_s, 0);
    let dir = workdir("skew");
    let source = source_from(&dir, "wire:\n  clock_skew_threshold_s: 5\n", None);
    assert_eq!(
        RuntimeConfig::from_source(&source)
            .unwrap()
            .clock_skew_threshold_s,
        5
    );
    let bad = source_from(&dir, "wire:\n  clock_skew_threshold_s: -1\n", None);
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// `dcps.presentation_instance_coherence`: `per_instance` (the default) and
/// `none` are taken from a file, anything else is refused.
#[test]
fn the_presentation_instance_coherence_setting_takes_both_values_and_refuses_others() {
    use zerodds_dcps::runtime::PresentationInstanceCoherence;

    let dir = workdir("presentation-instance-coherence");
    assert_eq!(
        RuntimeConfig::default().presentation_instance_coherence,
        PresentationInstanceCoherence::PerInstance
    );
    for (value, expected) in [
        ("per_instance", PresentationInstanceCoherence::PerInstance),
        ("none", PresentationInstanceCoherence::None),
    ] {
        let source = source_from(
            &dir,
            &format!("dcps:\n  presentation_instance_coherence: {value}\n"),
            None,
        );
        assert_eq!(
            RuntimeConfig::from_source(&source)
                .unwrap()
                .presentation_instance_coherence,
            expected
        );
    }
    let source = source_from(
        &dir,
        "dcps:\n  presentation_instance_coherence: sometimes\n",
        None,
    );
    assert!(RuntimeConfig::from_source(&source).is_err());
}

#[test]
fn the_type_object_v1_announcement_is_off_unless_the_file_or_the_variable_turns_it_on() {
    // DDS-XTypes 1.3 Annex D: `type_id` and `type` are optional members of
    // the built-in topic data; this project announces them on request.
    assert!(!RuntimeConfig::default().type_object_v1);
    let dir = workdir("type-object-v1");
    let source = source_from(&dir, "discovery:\n  type_object_v1: true\n", None);
    assert!(RuntimeConfig::from_source(&source).unwrap().type_object_v1);
    let source = source_from(&dir, "discovery:\n  type_object_v1: false\n", None);
    assert!(!RuntimeConfig::from_source(&source).unwrap().type_object_v1);

    let empty = dir.join("zerodds.yaml");
    std::fs::write(&empty, "\n").unwrap();
    let on = ConfigSource::from_command_line_with_env(
        &cli(&["--config", empty.to_str().unwrap()]),
        env_of(&[("ZERODDS_TYPE_OBJECT_V1", "1")]),
    )
    .unwrap();
    assert!(RuntimeConfig::from_source(&on).unwrap().type_object_v1);
}

#[test]
fn the_default_offer_is_the_spec_default_and_the_readers_request_xcdr2_and_xcdr() {
    // DDS-XTypes 1.3 §7.6.3.1.1: an empty list is XCDR, the default of the writers.
    let default = RuntimeConfig::default();
    assert_eq!(default.data_representation_offer, vec![0_i16]);
    // The ROS profiles offer both.
    let ros = RuntimeConfig::ros_defaults();
    assert_eq!(ros.data_representation_offer, vec![0_i16, 2]);

    let dir = workdir("representation");
    let source = source_from(
        &dir,
        "wire:\n  data_representation_offer: XCDR2,XCDR1\n",
        None,
    );
    let config = RuntimeConfig::from_source(&source).unwrap();
    assert_eq!(config.data_representation_offer, vec![2_i16, 0]);
}

/// `wire.omit_heartbeat_group_digests` and `writer.keyless_lifecycle_on_wire`
/// are off by default, taken from a file, and a non-boolean is refused.
#[test]
fn the_heartbeat_digest_and_keyless_lifecycle_settings_are_read_from_yaml() {
    let d = RuntimeConfig::default();
    assert!(!d.omit_heartbeat_group_digests);
    assert!(!d.keyless_lifecycle_on_wire);
    assert!(
        d.user_multicast,
        "the default locators of 9.6.2.3 include d2"
    );
    let dir = workdir("digests");
    let source = source_from(
        &dir,
        "wire:\n  omit_heartbeat_group_digests: true\nwriter:\n  keyless_lifecycle_on_wire: true\ntransport:\n  user_multicast: false\n",
        None,
    );
    let c = RuntimeConfig::from_source(&source).unwrap();
    assert!(c.omit_heartbeat_group_digests);
    assert!(c.keyless_lifecycle_on_wire);
    assert!(!c.user_multicast);
    let bad = source_from(&dir, "wire:\n  omit_heartbeat_group_digests: maybe\n", None);
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// `reader.honor_data_max_size_serialized` holds a payload against the
/// `dataMaxSizeSerialized` its writer announced (DDSI-RTPS 2.5 §8.4.10.4)
/// unless it is off; a value that is no boolean is refused.
#[test]
fn the_data_max_size_setting_is_on_by_default_and_read_from_yaml() {
    assert!(RuntimeConfig::default().reader_honor_data_max_size_serialized);
    let dir = workdir("data-max-size");
    let source = source_from(
        &dir,
        "reader:\n  honor_data_max_size_serialized: false\n",
        None,
    );
    let c = RuntimeConfig::from_source(&source).unwrap();
    assert!(!c.reader_honor_data_max_size_serialized);
    let bad = source_from(
        &dir,
        "reader:\n  honor_data_max_size_serialized: maybe\n",
        None,
    );
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// `reader.dispose_with_unregister` takes a marker with the DisposedFlag
/// and the UnregisteredFlag as both (DDSI-RTPS 2.5 §9.6.3.9) unless it is
/// off; a value that is no boolean is refused.
#[test]
fn the_dispose_with_unregister_setting_is_on_by_default_and_read_from_yaml() {
    assert!(RuntimeConfig::default().reader_dispose_with_unregister);
    let dir = workdir("dispose-with-unregister");
    let source = source_from(&dir, "reader:\n  dispose_with_unregister: false\n", None);
    let c = RuntimeConfig::from_source(&source).unwrap();
    assert!(!c.reader_dispose_with_unregister);
    let bad = source_from(&dir, "reader:\n  dispose_with_unregister: maybe\n", None);
    assert!(RuntimeConfig::from_source(&bad).is_err());
}

/// The vendor forms of the security plugins have a key, an environment
/// variable and the same values; the defaults are the specification's.
#[test]
fn the_security_vendor_forms_are_settings_with_the_specifications_defaults() {
    let defaults = RuntimeConfig::default().security_vendor_settings;
    assert_eq!(
        [
            defaults.kx_key_fallback.as_str(),
            defaults.compatibility_check.as_str(),
            defaults.algo_name_nul.as_str(),
            defaults.algorithm_names.as_str(),
            defaults.subject_name_form.as_str(),
            defaults.origin_authentication_fallback.as_str(),
        ],
        ["spec", "spec", "spec", "1.2", "rfc4514", "spec"]
    );

    let dir = workdir("security-forms");
    let source = source_from(
        &dir,
        "security:\n  kx_key_fallback: accept\n  compatibility_check: ignore\n  \
         algo_name_nul: mirror\n  algorithm_names: 1.1\n  subject_name_form: x509_name_oneline\n  \
         origin_authentication_fallback: accept\n",
        None,
    );
    let from_file = RuntimeConfig::from_source(&source)
        .expect("valid settings")
        .security_vendor_settings;
    assert_eq!(from_file.kx_key_fallback, "accept");
    assert_eq!(from_file.compatibility_check, "ignore");
    assert_eq!(from_file.algo_name_nul, "mirror");
    assert_eq!(from_file.algorithm_names, "1.1");
    assert_eq!(from_file.subject_name_form, "x509_name_oneline");
    assert_eq!(from_file.origin_authentication_fallback, "accept");
}

/// Layering as every runtime setting: the environment sets what no file sets,
/// and yields to the file.
#[test]
fn the_security_vendor_forms_follow_the_layers_and_refuse_an_unknown_value() {
    let dir = workdir("security-forms-layers");
    let base = dir.join("zerodds.yaml");
    std::fs::write(&base, "security:\n  subject_name_form: xn_flag_oneline\n").unwrap();
    let env = env_of(&[
        ("ZERODDS_SEC_SUBJECT_NAME_FORM", "x509_name_oneline"),
        ("ZERODDS_SEC_ALGO_NAME_NUL", "omit"),
    ]);
    let source =
        ConfigSource::from_command_line_with_env(&cli(&["--config", base.to_str().unwrap()]), env)
            .unwrap();
    let settings = RuntimeConfig::from_source(&source)
        .unwrap()
        .security_vendor_settings;
    assert_eq!(
        settings.subject_name_form, "xn_flag_oneline",
        "the file wins over the environment"
    );
    assert_eq!(
        settings.algo_name_nul, "omit",
        "what no file sets, the environment sets"
    );

    for (variable, bad) in [
        ("ZERODDS_SEC_KX_KEY_FALLBACK", "yes"),
        ("ZERODDS_SEC_COMPATIBILITY_CHECK", "off"),
        ("ZERODDS_SEC_ALGO_NAME_NUL", "nul"),
        ("ZERODDS_SEC_ALGORITHM_NAMES", "1.0"),
        ("ZERODDS_SEC_SUBJECT_NAME_FORM", "dn"),
        ("ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK", "yes"),
    ] {
        let empty = dir.join("empty.yaml");
        std::fs::write(&empty, "\n").unwrap();
        let from_env = ConfigSource::from_command_line_with_env(
            &cli(&["--config", empty.to_str().unwrap()]),
            env_of(&[(variable, bad)]),
        )
        .unwrap();
        let error = RuntimeConfig::from_source(&from_env)
            .expect_err(variable)
            .to_string();
        assert!(error.contains(&format!("${variable}")), "{error}");
    }
    let bad_file = source_from(&dir, "security:\n  algorithm_names: 1.0\n", None);
    assert!(RuntimeConfig::from_source(&bad_file).is_err());
}

/// The values reach the security crates, which parse them.
#[cfg(feature = "security")]
#[test]
fn the_security_vendor_forms_reach_the_security_profile() {
    use zerodds_security_runtime::{AlgoNameNul, AlgorithmNames, SubjectNameForm};
    let dir = workdir("security-forms-reach");
    let source = source_from(
        &dir,
        "security:\n  kx_key_fallback: accept\n  algo_name_nul: omit\n  algorithm_names: auto\n  \
         subject_name_form: xn_flag_oneline\n  origin_authentication_fallback: accept\n",
        None,
    );
    let settings = RuntimeConfig::from_source(&source)
        .unwrap()
        .security_vendor_settings;
    let forms =
        zerodds_security_runtime::vendor_form_settings_from_env(&|name| settings.lookup(name))
            .expect("the security crates take the values");
    assert!(forms.kx_key_fallback);
    assert!(forms.compatibility_checks);
    assert_eq!(forms.algo_name_nul, AlgoNameNul::Omit);
    assert_eq!(forms.algorithm_names, AlgorithmNames::Auto);
    assert_eq!(forms.subject_name_form, SubjectNameForm::XnFlagOneline);
    assert!(forms.origin_authentication_fallback);
}
