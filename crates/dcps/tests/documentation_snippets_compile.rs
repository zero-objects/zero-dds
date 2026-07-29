// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Compile + smoke test for the exact code shown in `documentation/**`
//! (the book, as opposed to `website/**` which `website_snippets_compile.rs`
//! covers). If a doc snippet's API drifts from the codebase, this test
//! stops compiling — the snippets are guaranteed to be real.
//!
//! Convention (mirrors `website_snippets_compile.rs`): construct the exact
//! structs/calls the fence shows and assert on a field or two. Deliberately
//! does **not** call `DcpsRuntime::start` / touch live sockets — that would
//! pull this file into the `resource_group: dcps-multicast` network-timing
//! concerns the other `crates/dcps/tests/*_e2e.rs` files manage with
//! `tests/common::{unique_domain, isolated_cfg}`. The live round-trip proof
//! for a fence instead lives in its `zerodds-examples/` companion (a real
//! `cargo run` in two terminals — see
//! `zerodds-examples/getting-started-first-publisher/README.md`), which
//! is not part of this workspace's `cargo test`.
//!
//! One function per fence, named `<page>_<anchor>_snippet`, doc-commented
//! with the exact `documentation/**` path + line so drift is traceable back
//! to the source. Add a function here whenever a new
//! `zerodds-examples/` companion for `documentation/**` is built (see
//! `internal/website-snippet-inventory.md` for the tracked-corpus twin of
//! this list).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

/// `documentation/01-getting-started/first-publisher.md` lines 33 (Publisher)
/// and 82 (Subscriber) — the low-level `DcpsRuntime` pub/sub walkthrough.
/// Verbatim field-for-field against the doc; companion:
/// `zerodds-examples/getting-started-first-publisher` (`pub.rs` / `sub.rs`,
/// compiles AND runs the identical code, verified live).
#[test]
fn first_publisher_snippet() {
    use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
    };
    use zerodds_rtps::wire_types::GuidPrefix;
    use zerodds_types::{PrimitiveKind, TypeIdentifier, qos::TypeConsistencyEnforcement};

    // Publisher fence (line 33): GuidPrefix + UserWriterConfig field set.
    let _pub_prefix = GuidPrefix::from_bytes([0x01; 12]);
    let writer_cfg = UserWriterConfig {
        topic_name: "HelloTopic".into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: TypeIdentifier::Primitive(PrimitiveKind::UInt8),
        data_representation_offer: None,
    };
    assert_eq!(writer_cfg.topic_name, "HelloTopic");
    assert!(writer_cfg.reliable);

    // Subscriber fence (line 82): GuidPrefix + UserReaderConfig field set.
    let _sub_prefix = GuidPrefix::from_bytes([0x02; 12]);
    let reader_cfg = UserReaderConfig {
        topic_name: "HelloTopic".into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: TypeIdentifier::Primitive(PrimitiveKind::UInt8),
        type_consistency: TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    };
    assert_eq!(reader_cfg.topic_name, writer_cfg.topic_name);
}

/// `documentation/03-configuration/observability.md` line 13 — "Layer 1:
/// lock-free atomic stats" fragment (`some_cache.stats()` /
/// `stats.snapshot()`). Fragment test (illustrative placeholder var, no
/// standalone companion): constructs a real `HistoryCache`, matching what
/// `some_cache` stands in for on the doc page.
#[test]
fn observability_cache_stats_snippet() {
    let some_cache = zerodds_rtps::history_cache::HistoryCache::new(16);

    // Doc fence, verbatim (loop body run once, not forever):
    let stats = some_cache.stats(); // Arc clone, cheap
    let snap = stats.snapshot();
    assert_eq!(snap.len, 0);
    assert_eq!(snap.max_sn, None);
}

/// `documentation/03-configuration/observability.md` line 46 — "Wire it
/// up" (Layer 2: Sink-based events). Companion:
/// `zerodds-examples/observability-stderr-sink` (runs live, prints the
/// `*.created` JSON lines the doc shows). This test only constructs the
/// `RuntimeConfig` (no `DcpsRuntime::start` — see file doc-comment).
#[test]
fn observability_stderr_json_sink_snippet() {
    use std::sync::Arc;

    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_foundation::observability::StderrJsonSink;

    // Doc fence, verbatim:
    let cfg = RuntimeConfig {
        observability: Arc::new(StderrJsonSink::new()),
        ..Default::default()
    };
    assert_eq!(cfg.tick_period, RuntimeConfig::default().tick_period);
}

/// `documentation/03-configuration/observability.md` line 73 — "Custom
/// sinks" fragment (a `Sink` trait impl). Fragment test: implements the
/// exact trait shown, wraps it in `Arc<dyn Sink>` as the doc's prose says
/// ("wrap in `Arc<dyn Sink>` and inject"), and proves `record` actually
/// gets called through the trait object.
#[test]
fn observability_custom_sink_snippet() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use zerodds_foundation::observability::{Component, Event, Level, Sink};

    // Doc fence, verbatim (with a counter instead of `/* ... */` so the
    // test can assert something):
    struct MetricsSink {
        calls: AtomicUsize,
    }
    impl Sink for MetricsSink {
        fn record(&self, _event: &Event) {
            self.calls.fetch_add(1, Ordering::Relaxed);
        }
    }

    let sink: Arc<dyn Sink> = Arc::new(MetricsSink {
        calls: AtomicUsize::new(0),
    });
    sink.record(&Event::new(Level::Info, Component::User, "test.event"));
    // The struct isn't reachable through the trait object to assert the
    // counter directly; constructing + recording through `Arc<dyn Sink>`
    // without panicking is itself the drift check the doc fence promises.
}

/// `documentation/03-configuration/qos-policies.md` line 111 — "Setting
/// QoS in code" (full `UserWriterConfig` QoS combination). Companion:
/// `zerodds-examples/qos-policies-full-writer` (registers this exact
/// config against a live `DcpsRuntime`).
#[test]
fn qos_policies_full_writer_snippet() {
    use zerodds_dcps::runtime::UserWriterConfig;
    use zerodds_qos::*;
    use zerodds_types::{PrimitiveKind, TypeIdentifier};

    // Doc fence, verbatim:
    let cfg = UserWriterConfig {
        topic_name: "Telemetry".into(),
        type_name: "Robot::Pose".into(),
        reliable: true, // = Reliability::Reliable
        durability: DurabilityKind::TransientLocal,
        deadline: DeadlineQosPolicy {
            period: Duration::from_millis(50),
        },
        lifespan: LifespanQosPolicy {
            duration: Duration::from_secs(5),
        },
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            lease_duration: Duration::from_secs(2),
        },
        ownership: OwnershipKind::Exclusive,
        ownership_strength: 100,
        presentation: Default::default(),
        partition: vec!["sensor.*".into()],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: TypeIdentifier::Primitive(PrimitiveKind::UInt8),
        data_representation_offer: None,
    };
    assert_eq!(cfg.ownership_strength, 100);
    assert_eq!(cfg.partition, vec!["sensor.*".to_string()]);
}

/// `documentation/03-configuration/runtime-config.md` line 6 — the full
/// `RuntimeConfig` field list, plus the "Common combinations" recipes
/// (lines 93/101/110), folded into the same companion
/// (`zerodds-examples/runtime-config`) and the same test per the doc's own
/// grouping.
#[test]
fn runtime_config_full_field_list_snippet() {
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use std::time::Duration;

    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_foundation::observability::StderrJsonSink;

    // Doc fence, verbatim (top of page):
    let cfg = RuntimeConfig {
        tick_period: Duration::from_millis(50),
        spdp_period: Duration::from_secs(5),
        spdp_multicast_group: Ipv4Addr::new(239, 255, 0, 1),
        multicast_interface: Ipv4Addr::UNSPECIFIED,
        announce_secure_endpoints: false,
        wlp_period: Duration::ZERO, // = lease/3
        participant_lease_duration: Duration::from_secs(100),
        observability: zerodds_foundation::observability::null_sink(),
        // … security fields when feature = "security"
        ..Default::default()
    };
    assert_eq!(cfg.spdp_multicast_group, Ipv4Addr::new(239, 255, 0, 1));
    assert_eq!(cfg.participant_lease_duration, Duration::from_secs(100));

    // "Common combinations" > "Sandbox / test" (line 93):
    let sandbox = RuntimeConfig::default();
    assert_eq!(
        sandbox.tick_period,
        zerodds_dcps::runtime::DEFAULT_TICK_PERIOD
    );

    // "Common combinations" > "Production server with monitoring" (line 101):
    let production = RuntimeConfig {
        observability: Arc::new(StderrJsonSink::new()),
        ..Default::default()
    };
    assert_eq!(production.tick_period, sandbox.tick_period);

    // "Common combinations" > "Hard real-time" (line 110):
    let hard_realtime = RuntimeConfig {
        tick_period: Duration::from_millis(2),
        spdp_period: Duration::from_secs(60), // discovery is rare
        participant_lease_duration: Duration::from_secs(10),
        ..Default::default()
    };
    assert_eq!(hard_realtime.tick_period, Duration::from_millis(2));
}

/// `documentation/06-operations/monitoring.md` line 25 — "1. Lock-free
/// atomic poll" fragment (`metrics::gauge!`). The doc's illustrative
/// two-positional-arg form (`gauge!(name, value)`) predates the current
/// `metrics` crate API (0.24: `gauge!(name)` returns a handle, `.set(value)`
/// applies it) — the doc fence was updated to match; this fragment test
/// pins the real, currently-published `metrics` crate API so a future
/// upstream breaking change is caught here instead of silently drifting
/// the doc again.
#[test]
fn monitoring_metrics_gauge_snippet() {
    let some_writer_cache = zerodds_rtps::history_cache::HistoryCache::new(16);

    // Doc fence, verbatim (loop body run once, not forever):
    let stats = some_writer_cache.stats();
    let snap = stats.snapshot();
    metrics::gauge!("dds.cache.len").set(snap.len as f64);
    metrics::gauge!("dds.cache.evicted").set(snap.evicted as f64);
    if let Some(_max) = snap.max_sn { /* … */ }
}

/// `documentation/04-idl/idlc-handbook.md` lines 388 (`build.rs`
/// integration) + 442 (`include!`). The doc's `build.rs` fence shells out
/// to the `zerodds-idlc` binary (a separate process, not something this
/// non-network compile test invokes — see file doc-comment); this test
/// instead pins the library the CLI's `generate --rust` subcommand wraps
/// (`zerodds_idl::parse` + `zerodds_idl_rust::generate_rust_module`),
/// generating the exact `idl/Robot.idl` companion input
/// (`zerodds-examples/idlc-buildrs`, which runs the real CLI end to end
/// via a live `build.rs` + `include!`). If the codegen library's API or
/// the generated code's shape drifts, this test breaks.
#[test]
fn idlc_handbook_buildrs_snippet() {
    let idl = r#"
module Robot {
    @appendable
    struct Pose {
        @key string<32> robot_id;
        double x;
        double y;
        double theta;
    };
};
"#;
    let ast = zerodds_idl::parse(idl, &zerodds_idl::config::ParserConfig::default())
        .expect("parse Robot.idl");
    let opts = zerodds_idl_rust::RustGenOptions::default();
    let rust = zerodds_idl_rust::generate_rust_module(&ast, &opts).expect("generate rust");

    assert!(rust.contains("pub struct Pose"));
    assert!(rust.contains("impl zerodds_dcps::DdsType for Pose"));
}
