# `zerodds-monitor` v1.1 — Observability substrate — Spec coverage

**Source:** `docs/specs/zerodds-monitor-1.1.md` (ZeroDDS vendor spec).

**Context:** `crates/monitor` is the ZeroDDS metric/trace substrate —
counter/gauge/histogram + registry, Prometheus text rendering, W3C Trace-Context
(PID 0x0D00), span schema and declarative topic/GUID policies; 46 tests
green. Hook points (§7) live in the layer crates
(transport/rtps/discovery/dcps/security).

Implementation:

- `crates/monitor/` — `counter.rs`, `gauge.rs`, `histogram.rs`, `labels.rs`,
  `registry.rs`, `metric_names.rs`, `prometheus.rs`, `trace_context.rs`,
  `span_names.rs`, `config.rs`, `server.rs`; 46 tests green.

---

## §1.1 Data model — layers

**Spec:** §1.1 — three metric layers (counter/gauge/histogram) over a registry;
`no_std`-capable core.

**Repo:** `crates/monitor/src/lib.rs`.

**Tests:** `crates/monitor/src/registry.rs::tests::snapshot_captures_all_three_kinds`.

**Status:** done

## §1.2 Counter

**Spec:** §1.2 — monotonic, atomic, thread-safe counter.

**Repo:** `crates/monitor/src/counter.rs` (`Counter`).

**Tests:** `crates/monitor/src/counter.rs::tests::counter_starts_zero`,
`counter_inc_and_add`, `counter_concurrent_increments_are_consistent`.

**Status:** done

## §1.3 Gauge

**Spec:** §1.3 — up/down gauge.

**Repo:** `crates/monitor/src/gauge.rs` (`Gauge`).

**Tests:** `crates/monitor/src/gauge.rs::tests::gauge_set_and_get`,
`gauge_inc_dec`, `gauge_add_negative`.

**Status:** done

## §1.4 Histogram

**Spec:** §1.4 — log10 buckets (11 finite + `+Inf`); ns/s.

**Repo:** `crates/monitor/src/histogram.rs` (`Histogram`, `LabeledHistogram`).

**Tests:** `crates/monitor/src/histogram.rs::tests::histogram_records_ns`,
`histogram_records_seconds_converts`.

**Status:** done

## §1.5 Labels

**Spec:** §1.5 — label set, key-sorted/deduplicated; metric key from name+labels.

**Repo:** `crates/monitor/src/labels.rs` (`Labels`, `MetricKey`).

**Tests:** `crates/monitor/src/labels.rs::tests::empty_labels`,
`labels_dedup_replaces_value`, `labels_sorted_by_key`,
`metric_key_eq_uses_name_and_labels`, `metric_key_hashable`.

**Status:** done

## §1.6 Registry

**Spec:** §1.6 — registry with idempotent lookup, `snapshot()`, default singleton.

**Repo:** `crates/monitor/src/registry.rs` (`Registry`, `default_registry`).

**Tests:** `crates/monitor/src/registry.rs::tests::default_registry_is_singleton`,
`registry_returns_same_counter_for_same_key`,
`registry_distinct_labels_distinct_counters`, `snapshot_captures_all_three_kinds`.

**Status:** done

## §2 Standard metric naming (31 metrics: 25 counter / 3 gauge / 3 histogram)

**Spec:** §2.1–§2.5 — 31 metrics across 5 domains (transport 6, RTPS 6, DCPS 10,
discovery 5, security 4), `dds_` prefix, unique.

**Repo:** `crates/monitor/src/metric_names.rs` (31 constants).

**Tests:** `crates/monitor/src/metric_names.rs::tests::all_count_matches_spec`,
`all_have_dds_prefix`, `all_unique`.

**Status:** done

## §3 Prometheus text-format encoding

**Spec:** §3.1–§3.3 — bucket convention (11 finite + `+Inf`, `_sum`/`_count`),
label escaping (`\\`, `"`, `\n`), sorted render; empty registry → empty string.

**Repo:** `crates/monitor/src/prometheus.rs`.

**Tests:** `crates/monitor/src/prometheus.rs::tests::render_counter_with_labels`,
`render_gauge`, `render_histogram_has_buckets_sum_count`, `render_label_escaping`,
`render_sorted_by_metric_name`, `render_empty_registry_is_empty_string`.

**Status:** done

## §4 W3C Trace-Context as an RTPS vendor PID (0x0D00)

**Spec:** §4.1–§4.4 — PID 0x0D00, `traceparent` wire format, encode/decode
roundtrip, reject of invalid/short/zero-trace-id payload.

**Repo:** `crates/monitor/src/trace_context.rs`.

**Tests:** `crates/monitor/src/trace_context.rs::tests::pid_constant_is_0x0d00`,
`traceparent_format_matches_w3c`, `traceparent_sampled_bit`,
`pid_roundtrip_with_state`, `pid_roundtrip_without_state`,
`from_to_span_context_roundtrip`, `pid_decode_rejects_invalid_traceparent_format`,
`pid_decode_rejects_short_payload`, `pid_decode_rejects_zero_trace_id`.

**Status:** done

## §5 Span schema

**Spec:** §5.1–§5.2 — span types + hierarchy; GUID-bearing attributes
(`dds.writer_guid` etc.) are subject to `GuidLabelPolicy` (§9).

**Repo:** `crates/monitor/src/span_names.rs` (span constants + GUID attr keys).

**Tests:** `crates/monitor/src/trace_context.rs::tests::from_to_span_context_roundtrip`.

**Status:** done

## §6 Lifecycle and configuration

**Spec:** §6.1–§6.3 — default registry, `MonitorConfig` (incl. the 1.1 policy
fields), optional Prometheus server.

**Repo:** `crates/monitor/src/config.rs` (`MonitorConfig`, `set_config`,
`active_config`), `crates/monitor/src/server.rs`, `crates/monitor/src/registry.rs`.

**Tests:** `crates/monitor/src/config.rs::tests::defaults_match_spec`,
`crates/monitor/src/registry.rs::tests::default_registry_is_singleton`,
`crates/monitor/src/server.rs::tests::server_404s_unknown_path`,
`server_serves_metrics_on_request`.

**Status:** done

## §7 Hook-point table (normative, cross-layer wire-up)

**Spec:** §7 — normative table of metric/span emission per layer crate.

**Repo:** `crates/rtps/src/metrics.rs`, `crates/dcps/src/metrics.rs` +
`crates/dcps/src/runtime.rs`, `crates/discovery/src/`,
`crates/transport-udp/`/`-tcp/`, `crates/security/src/token.rs`.

**Tests:** `crates/monitor/src/metric_names.rs::tests::all_count_matches_spec`
plus the §10 cross-layer test
`crates/dcps/src/metrics.rs::tests::write_increments_samples_written_and_renders`.

**Status:** done

## §8 Stability

**Spec:** §8 — stability policy; the 1.1 policy fields are additive.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §9 Security (declarative policies)

**Spec:** §9 — (a) payload redaction; (b) `GuidLabelPolicy` for GUID span attrs;
(c) `TopicLabelPolicy` for topic-label cardinality. Both dyn-free.

**Repo:** `crates/monitor/src/config.rs` (`TopicLabelPolicy {Full,Truncate,
Hashed,Drop}`, `GuidLabelPolicy {Full,Hashed,Omit}`, `MonitorConfig` fields,
helpers `topic_label()`/`guid_label()`), applied in
`crates/dcps/src/metrics.rs` (`topic_labels()` on all topic hook points).
(a) satisfied by the numbers-only data model.

**Tests:** `crates/monitor/src/config.rs::tests::topic_policy_full_is_identity`,
`topic_policy_truncate_caps_length`, `topic_policy_hashed_is_stable_and_hex`,
`topic_policy_drop_omits_label`, `guid_policy_variants`,
`active_config_defaults_when_unset_helpers_apply`, `defaults_match_spec`.

**Status:** done

## §10 Test obligation

**Spec:** §10 — counter/gauge consistency, histogram re-export, Prometheus text,
label escaping, PID roundtrip, **cross-layer** (dcps-write → render), **policy**.

**Repo:** `crates/monitor/src/` (§1–§4 + §9 policy tests),
`crates/dcps/src/metrics.rs` (cross-layer test).

**Tests:** the §1–§4 obligations +
`crates/dcps/src/metrics.rs::tests::write_increments_samples_written_and_renders`
(cross-layer) + the §9 policy tests.

**Status:** done

## §11 Coverage doc

**Spec:** §11 — this document; acceptance "all §-sections done".

**Repo:** `docs/spec-coverage/zerodds-monitor-1.1.md`.

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

10 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-monitor` — 46 tests green, 0 failed; plus
`cargo test -p zerodds-dcps` (§10 cross-layer test green, metrics feature default).

Open items: none. Decision records: none.
