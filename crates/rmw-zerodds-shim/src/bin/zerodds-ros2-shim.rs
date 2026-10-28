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
//!
//! It reads what the RMW reads: the runtime files (`zerodds.yaml`, its
//! drop-ins and includes) over the environment, with their `ros2.*` section.
//! `--config` selects another base, to check a file before it is installed;
//! the RMW itself reads the system layout.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::process::ExitCode;

use rmw_zerodds::ros2_config::{self, Mangling, QosPatch, Ros2Config};
use zerodds_dcps::runtime::{ConfigSource, RuntimeConfig};
use zerodds_ros2_rmw::qos_profiles::{Reliability, profiles};
use zerodds_ros2_rmw::topic_mangling::{RosKind, mangle_topic_name};

use zerodds_monitor::{Labels, Registry};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Flags of the old shim file that the runtime layers replace.
const REFUSED_FLAGS: &[(&str, &str)] = &[
    (
        "--runtime-config",
        "the runtime files are the RMW's only configuration; select a base with --config",
    ),
    (
        "--log-level",
        "the RMW logs through rcutils; use --ros-args --log-level",
    ),
    (
        "--enclave",
        "the RMW reads its enclave from security.enclave_dir (env ZERODDS_SECURITY_DIR)",
    ),
];

/// What the RMW would start with: the runtime files over the environment,
/// their `ros2.*` section, and the runtime settings built from them.
struct Loaded {
    source: ConfigSource,
    ros2: Ros2Config,
    runtime: RuntimeConfig,
}

fn load(command_line: &zerodds_config::CommandLine) -> Result<Loaded, String> {
    let source =
        ConfigSource::for_program(command_line, "zerodds", &ros2_config::is_key, &[], |name| {
            env::var(name).ok()
        })
        .map_err(|e| e.to_string())?;
    let ros2 = Ros2Config::from_config(source.tree()).map_err(|e| e.to_string())?;
    let runtime = RuntimeConfig::from_source(&source).map_err(|e| e.to_string())?;
    Ok(Loaded {
        source,
        ros2,
        runtime,
    })
}

fn main() -> ExitCode {
    let (command_line, args) = match zerodds_config::CommandLine::extract(env::args().skip(1)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let mut subcmd: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut domain: Option<i32> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if let Some((flag, why)) = REFUSED_FLAGS.iter().find(|(flag, _)| *flag == arg) {
            eprintln!("{flag}: {why}");
            return ExitCode::from(1);
        }
        match arg {
            "--domain" => {
                i += 1;
                match args.get(i).map(|s| s.parse::<i32>()) {
                    Some(Ok(d)) => domain = Some(d),
                    _ => {
                        eprintln!("--domain requires a ROS_DOMAIN_ID");
                        return ExitCode::from(1);
                    }
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

    if command_line.check() {
        return check_config(&command_line);
    }
    let Some(cmd) = subcmd else {
        print_usage();
        return ExitCode::from(1);
    };
    // Commands that read no configuration.
    match cmd.as_str() {
        "validate" => return cmd_validate(&positionals),
        "selftest" => return cmd_selftest(),
        "metrics" => return cmd_metrics(),
        _ => {}
    }
    let loaded = match load(&command_line) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("runtime configuration: {e}");
            return ExitCode::from(1);
        }
    };
    for (path, origin) in loaded.source.unknown_keys(&ros2_config::is_key) {
        eprintln!("warning: {origin}: unknown setting {path} is not read");
    }
    let domain = domain.unwrap_or_else(ros_domain_id);

    match cmd.as_str() {
        "info" => cmd_info(&loaded, domain),
        "topics" => cmd_topics(&loaded.ros2),
        "topic" => cmd_topic(&loaded.ros2, &positionals),
        "qos" => cmd_qos(&loaded.ros2, &positionals),
        "enclaves" => cmd_enclaves(&loaded.runtime),
        "catalog" => cmd_catalog(&loaded.ros2, domain),
        "doctor" => cmd_doctor(&loaded, domain),
        "graph" => cmd_graph(&loaded, domain),
        other => {
            eprintln!("unknown subcommand: {other}");
            print_usage();
            ExitCode::from(1)
        }
    }
}

/// The domain rcl passes to the RMW: `ROS_DOMAIN_ID`, else 0.
fn ros_domain_id() -> i32 {
    env::var("ROS_DOMAIN_ID")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
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
    eprintln!("  topic <ROS_TOPIC>     Show the DDS topic of a ROS topic");
    eprintln!("  qos <PROFILE>         Show a QoS profile of ros2.qos_profiles");
    eprintln!("                        (default | sensor_data | services |");
    eprintln!("                         parameters | parameter_events | configured)");
    eprintln!("  enclaves              List configured security enclaves");
    eprintln!("  validate <FILE>       Validate a runtime file with its FILE.d/");
    eprintln!("  selftest              Round-trip test (mangle + qos lookup)");
    eprintln!("  catalog               Print JSON catalog (Spec §5.2)");
    eprintln!("  metrics               Print Prometheus metric-name catalog (§8.2)");
    eprintln!("  doctor                Diagnose RMW/env/discovery/security setup (C8)");
    eprintln!("  graph                 Show local participant + topic graph (C8)");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --config <FILE>       runtime base with FILE.d/ (default: zerodds.yaml in the");
    eprintln!("                        system configuration directory, which the RMW reads)");
    eprintln!("  --set <PATH=VALUE>    one setting above every file, repeatable");
    eprintln!("  --check-config        validate the runtime files and their ros2 section,");
    eprintln!("                        print every value with its origin");
    eprintln!("  --domain <ID>         ROS_DOMAIN_ID override");
    eprintln!("  --version             Print version");
    eprintln!("  --help                Show this help");
    eprintln!();
    eprintln!("REFUSED, with where the value belongs:");
    for (flag, instead) in REFUSED_FLAGS {
        eprintln!("  {flag:<20}  {instead}");
    }
}

// ============================================================================
// Subcommands
// ============================================================================

fn mangling_name(m: &Mangling) -> String {
    match m {
        Mangling::Rmw => "rmw".to_string(),
        Mangling::None => "none".to_string(),
        Mangling::Custom {
            topic,
            request,
            response,
        } => format!(
            "custom (topic_prefix {topic}, request_prefix {request}, response_prefix {response})"
        ),
    }
}

fn cmd_info(loaded: &Loaded, domain: i32) -> ExitCode {
    let dist = env::var("ROS_DISTRO").unwrap_or_else(|_| "<unset>".into());
    let runtime = &loaded.runtime;
    println!("zerodds-ros2-shim       {VERSION}");
    println!("rmw_implementation      rmw_zerodds_cpp");
    println!("rmw_compat              REP-2007 (Humble/Iron/Jazzy/Rolling)");
    println!("ros_distro              {dist}");
    println!("ros_domain_id           {domain}");
    println!(
        "topic_mangling          {}",
        mangling_name(&loaded.ros2.mangling)
    );
    println!("qos_profiles            {}", loaded.ros2.profiles.len());
    println!("topic_overrides         {}", loaded.ros2.overrides.len());
    println!("multicast               {}", runtime.spdp_multicast_send);
    println!(
        "discovery.peers         {}",
        runtime.initial_peers.len() + runtime.initial_peer_specs.len()
    );
    println!(
        "enclave                 {}",
        runtime
            .security_enclave_dir
            .as_ref()
            .map_or_else(|| "(none)".to_string(), |d| d.display().to_string())
    );
    ExitCode::SUCCESS
}

/// The profile an override names, or `qos` for an inline map.
fn override_label(o: &ros2_config::TopicOverride) -> &str {
    o.profile.as_deref().unwrap_or("qos")
}

fn cmd_topics(cfg: &Ros2Config) -> ExitCode {
    if cfg.overrides.is_empty() {
        println!("(no topic overrides configured)");
        return ExitCode::SUCCESS;
    }
    println!("{:<32}  {:<32}  qos", "ros_topic", "dds_topic");
    for o in &cfg.overrides {
        println!(
            "{:<32}  {:<32}  {}",
            o.ros_topic,
            cfg.dds_topic(&o.ros_topic, false),
            override_label(o)
        );
    }
    ExitCode::SUCCESS
}

fn cmd_topic(cfg: &Ros2Config, args: &[String]) -> ExitCode {
    let Some(name) = args.first() else {
        eprintln!("usage: zerodds-ros2-shim topic <ROS_TOPIC>");
        return ExitCode::from(1);
    };
    if let Err(e) = mangle_topic_name(name, RosKind::Topic) {
        eprintln!("mangle error: {e}");
        return ExitCode::from(1);
    }
    println!("ros_topic               {name}");
    println!("dds_topic               {}", cfg.dds_topic(name, false));
    println!("topic_mangling          {}", mangling_name(&cfg.mangling));
    if let Some(o) = cfg.overrides.iter().find(|o| o.ros_topic == *name) {
        println!("override                {}", override_label(o));
        print_patch(&o.qos);
    }
    ExitCode::SUCCESS
}

fn cmd_qos(cfg: &Ros2Config, args: &[String]) -> ExitCode {
    let Some(profile_name) = args.first() else {
        eprintln!("usage: zerodds-ros2-shim qos <PROFILE>");
        eprintln!(
            "profiles: {}",
            cfg.profiles.keys().cloned().collect::<Vec<_>>().join(" | ")
        );
        return ExitCode::from(1);
    };
    let Some(p) = cfg.profiles.get(profile_name) else {
        eprintln!("unknown profile: {profile_name}");
        return ExitCode::from(1);
    };
    println!("profile                 {profile_name}");
    print_patch(p);
    ExitCode::SUCCESS
}

/// The fields of a profile or override; a field it leaves out keeps what
/// the application requested.
fn print_patch(p: &QosPatch) {
    const AS_REQUESTED: &str = "(as requested)";
    let name = |v: Option<u32>, names: &[(u32, &'static str)]| -> String {
        v.map_or_else(
            || AS_REQUESTED.to_string(),
            |v| {
                names
                    .iter()
                    .find(|(n, _)| *n == v)
                    .map_or_else(|| format!("UNKNOWN({v})"), |(_, s)| (*s).to_string())
            },
        )
    };
    let duration = |v: Option<u64>| -> String {
        match v {
            None => AS_REQUESTED.to_string(),
            Some(0) => "INFINITE".to_string(),
            Some(ns) => format!("{}ms", ns / 1_000_000),
        }
    };
    const SYSTEM_DEFAULT: (u32, &str) = (0, "SYSTEM_DEFAULT");
    println!(
        "reliability             {}",
        name(
            p.reliability,
            &[SYSTEM_DEFAULT, (1, "RELIABLE"), (2, "BEST_EFFORT")]
        )
    );
    println!(
        "durability              {}",
        name(
            p.durability,
            &[SYSTEM_DEFAULT, (1, "TRANSIENT_LOCAL"), (2, "VOLATILE")]
        )
    );
    let history = match (p.history, p.depth) {
        (Some(1), Some(d)) => format!("KEEP_LAST({d})"),
        (Some(1), None) => format!("KEEP_LAST(depth {AS_REQUESTED})"),
        (h, Some(d)) if h != Some(2) => format!(
            "{} depth {d}",
            name(h, &[SYSTEM_DEFAULT, (1, "KEEP_LAST"), (2, "KEEP_ALL")])
        ),
        (h, _) => name(h, &[SYSTEM_DEFAULT, (1, "KEEP_LAST"), (2, "KEEP_ALL")]),
    };
    println!("history                 {history}");
    println!("deadline                {}", duration(p.deadline_ns));
    println!("lifespan                {}", duration(p.lifespan_ns));
    println!(
        "liveliness              {}",
        name(
            p.liveliness,
            &[SYSTEM_DEFAULT, (1, "AUTOMATIC"), (3, "MANUAL_BY_TOPIC")]
        )
    );
    println!(
        "liveliness_lease        {}",
        duration(p.liveliness_lease_ns)
    );
}

fn cmd_enclaves(runtime: &RuntimeConfig) -> ExitCode {
    match &runtime.security_enclave_dir {
        None => println!("(no enclave configured; ROS_SECURITY_ENABLE=false)"),
        Some(dir) => {
            println!("active enclave          {}", dir.display());
            println!("source                  security.enclave_dir");
        }
    }
    ExitCode::SUCCESS
}

/// `validate <FILE>`: FILE as the runtime base, with its `FILE.d/`, as the
/// RMW would read it from the system directory.
fn cmd_validate(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!("usage: zerodds-ros2-shim validate <FILE>");
        return ExitCode::from(1);
    };
    let command_line = zerodds_config::CommandLine::default().with_config(path);
    match load(&command_line) {
        Ok(loaded) => {
            let unknown = loaded.source.unknown_keys(&ros2_config::is_key);
            for (key, origin) in &unknown {
                eprintln!("config invalid: {origin}: unknown setting {key}");
            }
            if !unknown.is_empty() {
                return ExitCode::from(1);
            }
            println!(
                "config OK ({} topic overrides)",
                loaded.ros2.overrides.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("config invalid: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_catalog(cfg: &Ros2Config, domain: i32) -> ExitCode {
    let mut out = String::from("{\"service\":\"zerodds-ros2-shim\",\"version\":\"");
    out.push_str(VERSION);
    out.push_str("\",\"rmw_implementation\":\"rmw_zerodds_cpp\",\"ros_domain_id\":");
    out.push_str(&domain.to_string());
    out.push_str(",\"topic_mangling\":\"");
    push_json_str(&mut out, &mangling_name(&cfg.mangling));
    out.push_str("\",\"topic_overrides\":[");
    for (i, t) in cfg.overrides.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"ros_topic\":\"");
        push_json_str(&mut out, &t.ros_topic);
        out.push_str("\",\"dds_topic\":\"");
        push_json_str(&mut out, &cfg.dds_topic(&t.ros_topic, false));
        out.push_str("\",\"qos_profile\":\"");
        push_json_str(&mut out, override_label(t));
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
fn cmd_doctor(loaded: &Loaded, domain: i32) -> ExitCode {
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
    match env::var("ROS_DOMAIN_ID").ok() {
        Some(s) if s.trim().parse::<i32>() == Ok(domain) => {
            line("ok", "ROS_DOMAIN_ID", domain.to_string())
        }
        Some(s) => line(
            "warn",
            "ROS_DOMAIN_ID",
            format!("env={s} but effective={domain} (--domain override)"),
        ),
        None => line("ok", "ROS_DOMAIN_ID", format!("{domain} (default)")),
    }

    // 4. Discovery, from the runtime settings the RMW starts with.
    let runtime = &loaded.runtime;
    let peers = runtime.initial_peers.len() + runtime.initial_peer_specs.len();
    match (runtime.spdp_multicast_send, peers) {
        (false, 0) => line(
            "fail",
            "discovery",
            "multicast-free but no discovery.peers in runtime configuration".into(),
        ),
        (false, n_peers) => line(
            "ok",
            "discovery",
            format!("multicast-free, {n_peers} configured unicast peer(s)"),
        ),
        (true, _) => line("ok", "discovery", "multicast enabled".into()),
    }

    // 5. Security: `security.enclave_dir` (env ZERODDS_SECURITY_DIR) / SROS2
    // enclave, from the same layers the RMW reads.
    match &runtime.security_enclave_dir {
        Some(p) => {
            let d = p.display().to_string();
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
        None => {
            let sros2 = env::var("ROS_SECURITY_ENABLE").ok().as_deref() == Some("true");
            if sros2 {
                line(
                    "warn",
                    "security",
                    "ROS_SECURITY_ENABLE=true but security.enclave_dir (ZERODDS_SECURITY_DIR) unset"
                        .into(),
                );
            } else {
                line("ok", "security", "disabled (no enclave)".into());
            }
        }
    }

    // 6. The mangling the RMW applies.
    line(
        "ok",
        "mangling",
        format!(
            "{}: /chatter -> {}",
            mangling_name(&loaded.ros2.mangling),
            loaded.ros2.dds_topic("/chatter", false)
        ),
    );

    println!();
    println!("summary: {warns} warning(s), {fails} failure(s)");
    if fails > 0 {
        ExitCode::from(5)
    } else {
        ExitCode::SUCCESS
    }
}

/// C8 — print the local configured graph, not the discovered remote graph.
fn cmd_graph(loaded: &Loaded, domain: i32) -> ExitCode {
    let cfg = &loaded.ros2;
    println!("participant");
    println!("  rmw_implementation    rmw_zerodds_cpp");
    println!("  domain_id             {domain}");
    let disc = if loaded.runtime.spdp_multicast_send {
        "multicast"
    } else {
        "unicast"
    };
    println!("  discovery             {disc}");
    println!("  topic_mangling        {}", mangling_name(&cfg.mangling));
    println!();
    if cfg.overrides.is_empty() {
        println!("topics                  (none configured)");
    } else {
        println!("topics");
        println!("  {:<28}  {:<28}  qos", "ros_topic", "dds_topic");
        for o in &cfg.overrides {
            println!(
                "  {:<28}  {:<28}  {}",
                o.ros_topic,
                cfg.dds_topic(&o.ros_topic, false),
                override_label(o)
            );
        }
    }
    ExitCode::SUCCESS
}

/// `--check-config`: every runtime value with its origin, the `ros2.*`
/// section as the RMW reads it; 0 when both are valid.
fn check_config(command_line: &zerodds_config::CommandLine) -> ExitCode {
    let source = match ConfigSource::for_program(
        command_line,
        "zerodds",
        &ros2_config::is_key,
        &[],
        |name| env::var(name).ok(),
    ) {
        Ok(source) => source,
        Err(e) => {
            // Deployment §2.1: a file that does not load is exit 2.
            println!("error: {e}");
            println!("configuration invalid");
            return ExitCode::from(2);
        }
    };
    let mut valid = match source.check_report(&ros2_config::is_key) {
        Ok(report) => {
            print!("{report}");
            true
        }
        Err(report) => {
            print!("{report}");
            false
        }
    };
    println!("ros2:");
    match Ros2Config::from_config(source.tree()) {
        Ok(cfg) => {
            println!("  topic_mangling = {}", mangling_name(&cfg.mangling));
            println!(
                "  qos_profiles = {}",
                cfg.profiles.keys().cloned().collect::<Vec<_>>().join(", ")
            );
            for o in &cfg.overrides {
                println!(
                    "  topic_override {} -> {} ({})",
                    o.ros_topic,
                    cfg.dds_topic(&o.ros_topic, false),
                    override_label(o)
                );
            }
        }
        Err(e) => {
            valid = false;
            println!("error: {e}");
        }
    }
    if valid {
        println!("configuration valid");
        ExitCode::SUCCESS
    } else {
        println!("configuration invalid");
        ExitCode::from(1)
    }
}
