// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `zerodds-ros2-shim` — diagnostics CLI for the `rmw_zerodds_cpp` shim.
//
// Spec: `docs/specs/zerodds-ros2-bridge-1.0.md` §2 (CLI), §3 (config),
// §5 (topic mangling, REP-2007), §6 (QoS translation, REP-2009).
//
// Unlike the other bridges of the family, NO daemon is needed here
// — the shim is a `librmw_zerodds_cpp.so` that rclcpp/rclpy
// load directly. This binary is a pure diagnostics
// tool: topic-mangling preview, QoS-profile lookup, validate.

//! Diagnostics CLI for the ROS-2 RMW shim: topic-mangling preview,
//! QoS-profile lookup, validate. Not the actual RMW; in production
//! rclcpp/rclpy load `librmw_zerodds_cpp.so` directly.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;

use zerodds_ros2_rmw::qos_profiles::{Durability, History, QosProfile, Reliability, profiles};
use zerodds_ros2_rmw::topic_mangling::{RosKind, mangle_topic_name};

use zerodds_monitor::{Labels, Registry};

const VERSION: &str = "1.0.0";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut subcmd: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut config_path: Option<String> = None;
    let mut domain: Option<i32> = None;
    let mut enclave: Option<String> = None;
    let mut log_level = String::from("info");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = args.get(i).cloned();
            }
            "--domain" => {
                i += 1;
                domain = args.get(i).and_then(|s| s.parse().ok());
            }
            "--enclave" => {
                i += 1;
                enclave = args.get(i).cloned();
            }
            "--log-level" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    log_level = v.clone();
                }
            }
            "--version" | "-V" => {
                println!("zerodds-ros2-shim {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other if subcmd.is_none() && !other.starts_with('-') => {
                subcmd = Some(other.to_string());
            }
            other if !other.starts_with('-') => {
                positionals.push(other.to_string());
            }
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(1);
            }
        }
        i += 1;
    }

    let cfg = match config_path.as_deref() {
        Some(p) => match load_config(p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("config error: {e}");
                return ExitCode::from(1);
            }
        },
        None => ShimConfig::default(),
    };

    let _ = log_level;
    let _ = enclave;

    let Some(cmd) = subcmd else {
        print_usage();
        return ExitCode::from(1);
    };

    match cmd.as_str() {
        "info" => cmd_info(&cfg, domain),
        "topics" => cmd_topics(&cfg),
        "topic" => cmd_topic(&cfg, &positionals),
        "qos" => cmd_qos(&cfg, &positionals),
        "enclaves" => cmd_enclaves(&cfg),
        "validate" => cmd_validate(&positionals),
        "selftest" => cmd_selftest(),
        "catalog" => cmd_catalog(&cfg),
        "metrics" => cmd_metrics(),
        "doctor" => cmd_doctor(&cfg, domain),
        "graph" => cmd_graph(&cfg, domain),
        other => {
            eprintln!("unknown subcommand: {other}");
            print_usage();
            ExitCode::from(1)
        }
    }
}

fn print_usage() {
    eprintln!("zerodds-ros2-shim {VERSION}");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  zerodds-ros2-shim <SUBCOMMAND> [OPTIONS]");
    eprintln!();
    eprintln!("SUBCOMMANDS:");
    eprintln!("  info                  Show RMW-Compat-Level + active mangling");
    eprintln!("  topics                List configured topic overrides");
    eprintln!("  topic <ROS_TOPIC>     Show DDS-mangled topic for given ROS topic");
    eprintln!("  qos <PROFILE>        Show DDS-QoS for ROS profile");
    eprintln!("                        (default | sensor_data | services |");
    eprintln!("                         parameters | parameter_events)");
    eprintln!("  enclaves              List configured security enclaves");
    eprintln!("  validate <CONFIG>     Validate YAML config file");
    eprintln!("  selftest              Round-trip test (mangle + qos lookup)");
    eprintln!("  catalog               Print JSON catalog (Spec §5.2)");
    eprintln!("  metrics               Print Prometheus metric-name catalog (§8.2)");
    eprintln!("  doctor                Diagnose RMW/env/discovery/security setup (C8)");
    eprintln!("  graph                 Show local participant + topic graph (C8)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>       Path to YAML config (default $ZERODDS_CONFIG");
    eprintln!("                        or $HOME/.zerodds/ros2-shim.yaml)");
    eprintln!("  --domain <ID>         ROS_DOMAIN_ID override");
    eprintln!("  --enclave <PATH>      SROS2-Enclave path");
    eprintln!("  --log-level <LEVEL>   trace|debug|info|warn|error");
    eprintln!("  --version             Print version");
    eprintln!("  --help                Show this help");
}

// ============================================================================
// Subcommands
// ============================================================================

fn cmd_info(cfg: &ShimConfig, domain_override: Option<i32>) -> ExitCode {
    let dist = env::var("ROS_DISTRO").unwrap_or_else(|_| "<unset>".into());
    let domain = domain_override.unwrap_or(cfg.ros_domain_id);
    println!("zerodds-ros2-shim       {VERSION}");
    println!("rmw_implementation      rmw_zerodds_cpp");
    println!("rmw_compat              REP-2007 (Humble/Iron/Jazzy/Rolling)");
    println!("ros_distro              {dist}");
    println!("ros_domain_id           {domain}");
    println!("namespace               {}", cfg.namespace);
    println!("topic_mangling          {}", cfg.topic_mangling);
    if cfg.topic_mangling == "custom" {
        println!("  topic_prefix          {}", cfg.custom_topic_prefix);
        println!("  request_prefix        {}", cfg.custom_request_prefix);
        println!("  response_prefix       {}", cfg.custom_response_prefix);
    }
    println!("enclave                 {}", cfg.enclave);
    println!("topic_overrides         {}", cfg.topic_overrides.len());
    println!("multicast               {}", cfg.discovery_multicast);
    println!(
        "unicast_initial_peers   {}",
        cfg.unicast_initial_peers.len()
    );
    ExitCode::SUCCESS
}

fn cmd_topics(cfg: &ShimConfig) -> ExitCode {
    if cfg.topic_overrides.is_empty() {
        println!("(no topic overrides configured)");
        return ExitCode::SUCCESS;
    }
    println!("{:<32}  {:<32}  qos", "ros_topic", "dds_topic");
    for o in &cfg.topic_overrides {
        let dds =
            mangle_topic_name(&o.ros_topic, RosKind::Topic).unwrap_or_else(|_| o.ros_topic.clone());
        println!("{:<32}  {:<32}  {}", o.ros_topic, dds, o.qos_profile);
    }
    ExitCode::SUCCESS
}

fn cmd_topic(_cfg: &ShimConfig, args: &[String]) -> ExitCode {
    let Some(name) = args.first() else {
        eprintln!("usage: zerodds-ros2-shim topic <ROS_TOPIC>");
        return ExitCode::from(1);
    };
    match mangle_topic_name(name, RosKind::Topic) {
        Ok(t) => {
            println!("ros_topic               {name}");
            println!("dds_topic               {t}");
            println!("kind                    Topic (rt/)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("mangle error: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_qos(_cfg: &ShimConfig, args: &[String]) -> ExitCode {
    let Some(profile_name) = args.first() else {
        eprintln!("usage: zerodds-ros2-shim qos <PROFILE>");
        eprintln!(
            "profiles: default | sensor_data | services | parameters | parameter_events | map"
        );
        return ExitCode::from(1);
    };
    let p: QosProfile = match profile_name.as_str() {
        "default" => profiles::DEFAULT,
        "sensor_data" => profiles::SENSOR_DATA,
        "map" => profiles::MAP,
        // Spec REP-2003 doesn't define `services`/`parameters`/
        // `parameter_events` in our crate yet — derive them per spec.
        "services" => QosProfile {
            reliability: Reliability::Reliable,
            durability: Durability::Volatile,
            history: History::KeepLast(10),
            liveliness_lease_secs: None,
            deadline_secs: None,
        },
        "parameters" => QosProfile {
            reliability: Reliability::Reliable,
            durability: Durability::Volatile,
            history: History::KeepLast(1000),
            liveliness_lease_secs: None,
            deadline_secs: None,
        },
        "parameter_events" => QosProfile {
            reliability: Reliability::Reliable,
            durability: Durability::Volatile,
            history: History::KeepAll,
            liveliness_lease_secs: None,
            deadline_secs: None,
        },
        other => {
            eprintln!("unknown profile: {other}");
            return ExitCode::from(1);
        }
    };
    print_qos(profile_name, &p);
    ExitCode::SUCCESS
}

fn cmd_enclaves(cfg: &ShimConfig) -> ExitCode {
    // FUTURE (L5): actual SROS2 enclave discovery via
    // `$ROS_SECURITY_KEYSTORE` Walkthrough. Stub.
    if cfg.enclave.is_empty() {
        println!("(no enclave configured; ROS_SECURITY_ENABLE=false)");
    } else {
        println!("active enclave          {}", cfg.enclave);
        println!("source                  config:enclave");
    }
    ExitCode::SUCCESS
}

fn cmd_validate(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!("usage: zerodds-ros2-shim validate <CONFIG>");
        return ExitCode::from(1);
    };
    match load_config(path) {
        Ok(cfg) => {
            println!("config OK ({} topic overrides)", cfg.topic_overrides.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("config invalid: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_catalog(cfg: &ShimConfig) -> ExitCode {
    let mut out = String::from("{\"service\":\"zerodds-ros2-shim\",\"version\":\"");
    out.push_str(VERSION);
    out.push_str("\",\"rmw_implementation\":\"rmw_zerodds_cpp\",\"ros_domain_id\":");
    out.push_str(&cfg.ros_domain_id.to_string());
    out.push_str(",\"namespace\":\"");
    push_json_str(&mut out, &cfg.namespace);
    out.push_str("\",\"topic_overrides\":[");
    for (i, t) in cfg.topic_overrides.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let dds =
            mangle_topic_name(&t.ros_topic, RosKind::Topic).unwrap_or_else(|_| t.ros_topic.clone());
        out.push_str("{\"ros_topic\":\"");
        push_json_str(&mut out, &t.ros_topic);
        out.push_str("\",\"dds_topic\":\"");
        push_json_str(&mut out, &dds);
        out.push_str("\",\"qos_profile\":\"");
        push_json_str(&mut out, &t.qos_profile);
        out.push_str("\"}");
    }
    out.push_str("]}");
    println!("{out}");
    ExitCode::SUCCESS
}

fn push_json_str(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

fn cmd_metrics() -> ExitCode {
    // Generate the Prometheus metric-name catalog. Since the shim is a
    // library in the rclcpp process (not a daemon), we provide only
    // the schema; the real rclcpp process should fetch the counters from
    // `zerodds_monitor::default_registry()` + export them via OTLP/Prometheus.
    let registry = Registry::new();
    let names = [
        (
            "zerodds_ros2_topics_published_total",
            "ROS2 topics published",
        ),
        (
            "zerodds_ros2_topics_subscribed_total",
            "ROS2 topics subscribed",
        ),
        ("zerodds_ros2_messages_in_total", "ROS2 inbound messages"),
        ("zerodds_ros2_messages_out_total", "ROS2 outbound messages"),
        ("zerodds_ros2_bytes_in_total", "ROS2 inbound bytes"),
        ("zerodds_ros2_bytes_out_total", "ROS2 outbound bytes"),
        ("zerodds_ros2_errors_total", "ROS2 wire/codec errors"),
    ];
    for (n, help) in names {
        registry.set_help(n, help);
        // Initialize counter with 0 so /metrics shows the schema.
        let _ = registry.counter(n, Labels::new());
    }
    print!("{}", registry.render_prometheus());
    ExitCode::SUCCESS
}

fn cmd_selftest() -> ExitCode {
    // §2 selftest: Mangle a known topic and look up a profile.
    let mangled = match mangle_topic_name("/chatter", RosKind::Topic) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("selftest FAIL (mangle): {e}");
            return ExitCode::from(4);
        }
    };
    if mangled != "rt/chatter" {
        eprintln!("selftest FAIL: expected `rt/chatter`, got `{mangled}`");
        return ExitCode::from(4);
    }
    let p = profiles::DEFAULT;
    if !matches!(p.reliability, Reliability::Reliable) {
        eprintln!("selftest FAIL: default profile is not RELIABLE");
        return ExitCode::from(4);
    }
    println!("selftest OK");
    println!("  /chatter            -> rt/chatter");
    println!("  default profile      -> RELIABLE/VOLATILE/KEEP_LAST(10)");
    ExitCode::SUCCESS
}

/// C8 — `doctor`: diagnose the ROS/ZeroDDS setup from the **local**
/// environment + config (no DDS-graph introspection needed). Reports each
/// check as `ok` / `warn` / `fail`; exits non-zero if any hard check fails.
fn cmd_doctor(cfg: &ShimConfig, domain_override: Option<i32>) -> ExitCode {
    let mut warns = 0u32;
    let mut fails = 0u32;
    let mut line = |status: &str, label: &str, detail: String| {
        match status {
            "warn" => warns += 1,
            "fail" => fails += 1,
            _ => {}
        }
        println!("[{status:>4}] {label:<26} {detail}");
    };

    // 1. RMW_IMPLEMENTATION consistency — must select this shim when set.
    match env::var("RMW_IMPLEMENTATION") {
        Ok(v) if v == "rmw_zerodds_cpp" => {
            line("ok", "RMW_IMPLEMENTATION", v);
        }
        Ok(v) => line(
            "fail",
            "RMW_IMPLEMENTATION",
            format!("{v} (expected rmw_zerodds_cpp — this shim won't be loaded)"),
        ),
        Err(_) => line(
            "warn",
            "RMW_IMPLEMENTATION",
            "unset (rcl picks the default rmw, not necessarily zerodds)".into(),
        ),
    }

    // 2. ROS_DISTRO — REP-2007 compatible distros.
    match env::var("ROS_DISTRO") {
        Ok(d) if matches!(d.as_str(), "humble" | "iron" | "jazzy" | "rolling") => {
            line("ok", "ROS_DISTRO", d)
        }
        Ok(d) => line(
            "warn",
            "ROS_DISTRO",
            format!("{d} (untested REP-2007 distro)"),
        ),
        Err(_) => line("warn", "ROS_DISTRO", "unset".into()),
    }

    // 3. Domain id.
    let domain = domain_override.unwrap_or(cfg.ros_domain_id);
    let env_domain = env::var("ROS_DOMAIN_ID").ok();
    match &env_domain {
        Some(s) if s.trim().parse::<i32>() == Ok(domain) => {
            line("ok", "ROS_DOMAIN_ID", domain.to_string())
        }
        Some(s) => line(
            "warn",
            "ROS_DOMAIN_ID",
            format!("env={s} but effective={domain} (override/config mismatch)"),
        ),
        None => line("ok", "ROS_DOMAIN_ID", format!("{domain} (default)")),
    }

    // 4. Discovery: multicast vs unicast peers.
    let no_mc = env::var_os("ZERODDS_NO_MULTICAST").is_some() || !cfg.discovery_multicast;
    let peers_env = env::var("ZERODDS_PEERS").unwrap_or_default();
    let n_peers = cfg.unicast_initial_peers.len()
        + peers_env
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .count();
    if no_mc {
        if n_peers == 0 {
            line(
                "fail",
                "discovery",
                "multicast-free but NO unicast peers (ZERODDS_PEERS / config) — peers will not find each other".into(),
            );
        } else {
            line(
                "ok",
                "discovery",
                format!("multicast-free, {n_peers} unicast peer(s)"),
            );
        }
    } else {
        line("ok", "discovery", "multicast enabled".into());
    }

    // 5. Security: ZERODDS_SECURITY_DIR / SROS2 enclave.
    match env::var("ZERODDS_SECURITY_DIR") {
        Ok(d) if !d.is_empty() => {
            let p = std::path::Path::new(&d);
            if p.join("cert.pem").is_file() && p.join("governance.p7s").is_file() {
                line("ok", "security", format!("enclave {d}"));
            } else {
                line(
                    "fail",
                    "security",
                    format!("{d} missing cert.pem/governance.p7s (not an SROS2 enclave)"),
                );
            }
        }
        _ => {
            let sros2 = env::var("ROS_SECURITY_ENABLE").ok().as_deref() == Some("true");
            if sros2 {
                line(
                    "warn",
                    "security",
                    "ROS_SECURITY_ENABLE=true but ZERODDS_SECURITY_DIR unset".into(),
                );
            } else {
                line("ok", "security", "disabled (no enclave)".into());
            }
        }
    }

    // 6. Self-test of the mangling path (reuses the selftest invariant).
    match mangle_topic_name("/chatter", RosKind::Topic) {
        Ok(m) if m == "rt/chatter" => line("ok", "mangling", "/chatter -> rt/chatter".into()),
        Ok(m) => line(
            "fail",
            "mangling",
            format!("/chatter -> {m} (expected rt/chatter)"),
        ),
        Err(e) => line("fail", "mangling", format!("error: {e}")),
    }

    println!();
    println!("summary: {warns} warning(s), {fails} failure(s)");
    if fails > 0 {
        ExitCode::from(5)
    } else {
        ExitCode::SUCCESS
    }
}

/// C8 — `graph`: print the **local** participant + topic graph derived from
/// config (discovery-independent — works even with multicast off / no peers
/// visible yet). Shows what this node WOULD publish/subscribe, with the
/// DDS-mangled topic names.
fn cmd_graph(cfg: &ShimConfig, domain_override: Option<i32>) -> ExitCode {
    let domain = domain_override.unwrap_or(cfg.ros_domain_id);
    println!("participant");
    println!("  rmw_implementation    rmw_zerodds_cpp");
    println!("  domain_id             {domain}");
    println!("  namespace             {}", cfg.namespace);
    let disc = if env::var_os("ZERODDS_NO_MULTICAST").is_some() || !cfg.discovery_multicast {
        "unicast"
    } else {
        "multicast"
    };
    println!("  discovery             {disc}");
    println!();
    if cfg.topic_overrides.is_empty() {
        println!("topics                  (none configured)");
    } else {
        println!("topics");
        println!("  {:<28}  {:<28}  qos", "ros_topic", "dds_topic");
        for o in &cfg.topic_overrides {
            let dds = mangle_topic_name(&o.ros_topic, RosKind::Topic)
                .unwrap_or_else(|_| o.ros_topic.clone());
            println!("  {:<28}  {:<28}  {}", o.ros_topic, dds, o.qos_profile);
        }
    }
    ExitCode::SUCCESS
}

fn print_qos(name: &str, p: &QosProfile) {
    println!("profile                 {name}");
    println!(
        "reliability             {}",
        match p.reliability {
            Reliability::SystemDefault => "SYSTEM_DEFAULT",
            Reliability::Reliable => "RELIABLE",
            Reliability::BestEffort => "BEST_EFFORT",
            Reliability::Unknown => "UNKNOWN",
        }
    );
    println!(
        "durability              {}",
        match p.durability {
            Durability::SystemDefault => "SYSTEM_DEFAULT",
            Durability::Volatile => "VOLATILE",
            Durability::TransientLocal => "TRANSIENT_LOCAL",
            Durability::Unknown => "UNKNOWN",
        }
    );
    match p.history {
        History::SystemDefault => println!("history                 SYSTEM_DEFAULT"),
        History::KeepLast(n) => println!("history                 KEEP_LAST({n})"),
        History::KeepAll => println!("history                 KEEP_ALL"),
        History::Unknown => println!("history                 UNKNOWN"),
    }
    println!(
        "deadline                {}",
        p.deadline_secs
            .map_or_else(|| "INFINITE".into(), |s| format!("{s}s"))
    );
    println!(
        "liveliness_lease        {}",
        p.liveliness_lease_secs
            .map_or_else(|| "INFINITE".into(), |s| format!("{s}s"))
    );
}

// ============================================================================
// Config (YAML-Subset Loader)
// ============================================================================

#[derive(Debug, Clone)]
struct ShimConfig {
    namespace: String,
    topic_mangling: String,
    custom_topic_prefix: String,
    custom_request_prefix: String,
    custom_response_prefix: String,
    enclave: String,
    ros_domain_id: i32,
    discovery_multicast: bool,
    unicast_initial_peers: Vec<String>,
    topic_overrides: Vec<TopicOverride>,
}

impl Default for ShimConfig {
    fn default() -> Self {
        Self {
            namespace: "/".into(),
            topic_mangling: "rmw".into(),
            custom_topic_prefix: "rt/".into(),
            custom_request_prefix: "rq/".into(),
            custom_response_prefix: "rr/".into(),
            enclave: String::new(),
            ros_domain_id: 0,
            discovery_multicast: true,
            unicast_initial_peers: Vec::new(),
            topic_overrides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct TopicOverride {
    ros_topic: String,
    qos_profile: String,
}

fn load_config(path: &str) -> Result<ShimConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    parse_yaml_subset(&raw)
}

/// Minimal YAML-subset parser for the shim config schema.
/// Supports only the fields from Spec §3 — no anchor/alias,
/// no multi-doc, no tags. Indent = 2 spaces.
fn parse_yaml_subset(s: &str) -> Result<ShimConfig, String> {
    let mut cfg = ShimConfig::default();
    let mut section: Vec<String> = Vec::new();
    let mut current_override: Option<TopicOverride> = None;

    for (lineno, raw) in s.lines().enumerate() {
        // Strip comments (yaml: `#` starts a comment unless quoted).
        let line = match raw.find('#') {
            Some(i) => &raw[..i],
            None => raw,
        };
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.bytes().take_while(|b| *b == b' ').count();
        let body = line.trim_end();
        let trimmed = body.trim_start();

        // Section depth from indent (assume 2-space indent steps).
        let depth = indent / 2;
        section.truncate(depth);

        // List item: `- key: value` or `- ros_topic: ...`.
        if let Some(rest) = trimmed.strip_prefix("- ") {
            // Flush current override if we're in topic_overrides.
            if section.last().map(String::as_str) == Some("topic_overrides") {
                if let Some(ov) = current_override.take() {
                    cfg.topic_overrides.push(ov);
                }
                let mut new_ov = TopicOverride {
                    ros_topic: String::new(),
                    qos_profile: "default".into(),
                };
                if let Some((k, v)) = split_kv(rest) {
                    if k == "ros_topic" {
                        new_ov.ros_topic = unquote(v).to_string();
                    }
                }
                current_override = Some(new_ov);
            } else if section.last().map(String::as_str) == Some("unicast_initial_peers") {
                cfg.unicast_initial_peers.push(rest.trim().to_string());
            }
            continue;
        }

        // Key: Value or Key:
        let Some((key, value)) = split_kv(trimmed) else {
            return Err(format!(
                "line {}: not a key:value: `{}`",
                lineno + 1,
                trimmed
            ));
        };

        if value.is_empty() {
            // Section-Header.
            section.push(key.to_string());
            continue;
        }

        // Path-aware setter.
        let path_str = section.join("/");
        match (path_str.as_str(), key) {
            ("ros2", "namespace") => cfg.namespace = unquote(value).into(),
            ("ros2", "topic_mangling") => cfg.topic_mangling = unquote(value).into(),
            ("ros2", "enclave") => cfg.enclave = unquote(value).into(),
            ("ros2", "ros_domain_id") => {
                cfg.ros_domain_id = value
                    .parse()
                    .map_err(|e| format!("line {}: bad ros_domain_id: {e}", lineno + 1))?;
            }
            ("ros2/custom_mangling", "topic_prefix") => {
                cfg.custom_topic_prefix = unquote(value).into();
            }
            ("ros2/custom_mangling", "request_prefix") => {
                cfg.custom_request_prefix = unquote(value).into();
            }
            ("ros2/custom_mangling", "response_prefix") => {
                cfg.custom_response_prefix = unquote(value).into();
            }
            ("discovery", "multicast") => {
                cfg.discovery_multicast = matches!(value, "true" | "yes" | "1");
            }
            (_, _) => {
                // Topic-override fields.
                if section.last().map(String::as_str) == Some("topic_overrides")
                    || (section.len() >= 2 && section[section.len() - 2] == "topic_overrides")
                {
                    if let Some(ov) = current_override.as_mut() {
                        match key {
                            "ros_topic" => ov.ros_topic = unquote(value).into(),
                            "qos_profile" => ov.qos_profile = unquote(value).into(),
                            _ => {}
                        }
                    }
                }
                // Unknown keys: ignore (forward-compat).
            }
        }
    }
    if let Some(ov) = current_override.take() {
        cfg.topic_overrides.push(ov);
    }

    Ok(cfg)
}

fn split_kv(s: &str) -> Option<(&str, &str)> {
    let i = s.find(':')?;
    let k = s[..i].trim();
    let v = s[i + 1..].trim();
    Some((k, v))
}

fn unquote(s: &str) -> &str {
    let quoted =
        (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''));
    if quoted && s.len() >= 2 {
        return &s[1..s.len() - 1];
    }
    s
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_yaml() {
        let yaml = "\
ros2:
  namespace: \"/zerodds\"
  topic_mangling: \"rmw\"
  ros_domain_id: 7
  enclave: \"/security/foo\"
discovery:
  multicast: false
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.namespace, "/zerodds");
        assert_eq!(cfg.topic_mangling, "rmw");
        assert_eq!(cfg.ros_domain_id, 7);
        assert_eq!(cfg.enclave, "/security/foo");
        assert!(!cfg.discovery_multicast);
    }

    #[test]
    fn parse_topic_overrides() {
        let yaml = "\
ros2:
  namespace: \"/\"
  topic_overrides:
    - ros_topic: \"/chatter\"
      qos_profile: \"sensor_data\"
    - ros_topic: \"/other\"
      qos_profile: \"default\"
";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.topic_overrides.len(), 2);
        assert_eq!(cfg.topic_overrides[0].ros_topic, "/chatter");
        assert_eq!(cfg.topic_overrides[0].qos_profile, "sensor_data");
        assert_eq!(cfg.topic_overrides[1].ros_topic, "/other");
    }

    #[test]
    fn parse_with_comments_and_blanks() {
        let yaml = "\
# top-level comment
ros2:
  # inner comment
  namespace: \"/x\"

";
        let cfg = parse_yaml_subset(yaml).expect("parse");
        assert_eq!(cfg.namespace, "/x");
    }

    #[test]
    fn parse_invalid_returns_err() {
        let yaml = "this is not yaml";
        let res = parse_yaml_subset(yaml);
        assert!(res.is_err());
    }

    #[test]
    fn unquote_handles_double_and_single() {
        assert_eq!(unquote("\"foo\""), "foo");
        assert_eq!(unquote("'bar'"), "bar");
        assert_eq!(unquote("baz"), "baz");
        assert_eq!(unquote(""), "");
    }
}
