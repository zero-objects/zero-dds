# `zerodds-monitor` v1.1 — Observability-Substrate — Spec-Coverage

**Quelle:** `docs/specs/zerodds-monitor-1.1.md` (ZeroDDS-Vendor-Spec).

**Kontext:** `crates/monitor` ist das Metric-/Trace-Substrat von ZeroDDS —
Counter/Gauge/Histogram + Registry, Prometheus-Text-Rendering, W3C-Trace-Context
(PID 0x0D00), Span-Schema und deklarative Topic-/GUID-Policies; 46 Tests
grün. Hook-Points (§7) liegen in den Layer-Crates
(transport/rtps/discovery/dcps/security).

Implementation:

- `crates/monitor/` — `counter.rs`, `gauge.rs`, `histogram.rs`, `labels.rs`,
  `registry.rs`, `metric_names.rs`, `prometheus.rs`, `trace_context.rs`,
  `span_names.rs`, `config.rs`, `server.rs`; 46 Tests grün.

---

## §1.1 Datenmodell — Schichten

**Spec:** §1.1 — drei Metric-Schichten (Counter/Gauge/Histogram) über einer
Registry; `no_std`-fähiger Kern.

**Repo:** `crates/monitor/src/lib.rs`.

**Tests:** `crates/monitor/src/registry.rs::tests::snapshot_captures_all_three_kinds`.

**Status:** done

## §1.2 Counter

**Spec:** §1.2 — monotoner, atomarer, thread-safer Counter.

**Repo:** `crates/monitor/src/counter.rs` (`Counter`).

**Tests:** `crates/monitor/src/counter.rs::tests::counter_starts_zero`,
`counter_inc_and_add`, `counter_concurrent_increments_are_consistent`.

**Status:** done

## §1.3 Gauge

**Spec:** §1.3 — auf-/absteigender Gauge.

**Repo:** `crates/monitor/src/gauge.rs` (`Gauge`).

**Tests:** `crates/monitor/src/gauge.rs::tests::gauge_set_and_get`,
`gauge_inc_dec`, `gauge_add_negative`.

**Status:** done

## §1.4 Histogram

**Spec:** §1.4 — log10-Buckets (11 endlich + `+Inf`); ns/s.

**Repo:** `crates/monitor/src/histogram.rs` (`Histogram`, `LabeledHistogram`).

**Tests:** `crates/monitor/src/histogram.rs::tests::histogram_records_ns`,
`histogram_records_seconds_converts`.

**Status:** done

## §1.5 Labels

**Spec:** §1.5 — Label-Set, key-sortiert/dedupliziert; Metric-Key aus Name+Labels.

**Repo:** `crates/monitor/src/labels.rs` (`Labels`, `MetricKey`).

**Tests:** `crates/monitor/src/labels.rs::tests::empty_labels`,
`labels_dedup_replaces_value`, `labels_sorted_by_key`,
`metric_key_eq_uses_name_and_labels`, `metric_key_hashable`.

**Status:** done

## §1.6 Registry

**Spec:** §1.6 — Registry mit idempotentem Lookup, `snapshot()`,
Default-Singleton.

**Repo:** `crates/monitor/src/registry.rs` (`Registry`, `default_registry`).

**Tests:** `crates/monitor/src/registry.rs::tests::default_registry_is_singleton`,
`registry_returns_same_counter_for_same_key`,
`registry_distinct_labels_distinct_counters`, `snapshot_captures_all_three_kinds`.

**Status:** done

## §2 Standard-Metric-Naming (31 Metriken: 25 Counter / 3 Gauge / 3 Histogram)

**Spec:** §2.1–§2.5 — 31 Metriken über 5 Domains (Transport 6, RTPS 6, DCPS 10,
Discovery 5, Security 4), `dds_`-Prefix, eindeutig.

**Repo:** `crates/monitor/src/metric_names.rs` (31 Konstanten).

**Tests:** `crates/monitor/src/metric_names.rs::tests::all_count_matches_spec`,
`all_have_dds_prefix`, `all_unique`.

**Status:** done

## §3 Prometheus-Text-Format-Encoding

**Spec:** §3.1–§3.3 — Bucket-Konvention (11 endlich + `+Inf`, `_sum`/`_count`),
Label-Escaping (`\\`, `"`, `\n`), sortierter Render; leere Registry → leerer
String.

**Repo:** `crates/monitor/src/prometheus.rs`.

**Tests:** `crates/monitor/src/prometheus.rs::tests::render_counter_with_labels`,
`render_gauge`, `render_histogram_has_buckets_sum_count`, `render_label_escaping`,
`render_sorted_by_metric_name`, `render_empty_registry_is_empty_string`.

**Status:** done

## §4 W3C-Trace-Context als RTPS-Vendor-PID (0x0D00)

**Spec:** §4.1–§4.4 — PID 0x0D00, `traceparent`-Wire-Format, Encode/Decode-
Roundtrip, Reject von ungültigem/kurzem/Zero-Trace-ID-Payload.

**Repo:** `crates/monitor/src/trace_context.rs`.

**Tests:** `crates/monitor/src/trace_context.rs::tests::pid_constant_is_0x0d00`,
`traceparent_format_matches_w3c`, `traceparent_sampled_bit`,
`pid_roundtrip_with_state`, `pid_roundtrip_without_state`,
`from_to_span_context_roundtrip`, `pid_decode_rejects_invalid_traceparent_format`,
`pid_decode_rejects_short_payload`, `pid_decode_rejects_zero_trace_id`.

**Status:** done

## §5 Span-Schema

**Spec:** §5.1–§5.2 — Span-Typen + Hierarchie; GUID-tragende Attribute
(`dds.writer_guid` etc.) unterliegen `GuidLabelPolicy` (§9).

**Repo:** `crates/monitor/src/span_names.rs` (Span-Konstanten + GUID-Attr-Keys).

**Tests:** `crates/monitor/src/trace_context.rs::tests::from_to_span_context_roundtrip`.

**Status:** done

## §6 Lifecycle und Konfiguration

**Spec:** §6.1–§6.3 — Default-Registry, `MonitorConfig` (inkl. der 1.1-Policy-
Felder), optionaler Prometheus-Server.

**Repo:** `crates/monitor/src/config.rs` (`MonitorConfig`, `set_config`,
`active_config`), `crates/monitor/src/server.rs`, `crates/monitor/src/registry.rs`.

**Tests:** `crates/monitor/src/config.rs::tests::defaults_match_spec`,
`crates/monitor/src/registry.rs::tests::default_registry_is_singleton`,
`crates/monitor/src/server.rs::tests::server_404s_unknown_path`,
`server_serves_metrics_on_request`.

**Status:** done

## §7 Hook-Point-Tabelle (normativ, Cross-Layer-Wireup)

**Spec:** §7 — normative Tabelle der Metric-/Span-Emission pro Layer-Crate.

**Repo:** `crates/rtps/src/metrics.rs`, `crates/dcps/src/metrics.rs` +
`crates/dcps/src/runtime.rs`, `crates/discovery/src/`,
`crates/transport-udp/`/`-tcp/`, `crates/security/src/token.rs`.

**Tests:** `crates/monitor/src/metric_names.rs::tests::all_count_matches_spec` +
der §10-Cross-Layer-Test
`crates/dcps/src/metrics.rs::tests::write_increments_samples_written_and_renders`.

**Status:** done

## §8 Stabilität

**Spec:** §8 — Stabilitäts-Policy; die 1.1-Policy-Felder sind additiv.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §9 Sicherheit (deklarative Policies)

**Spec:** §9 — (a) Payload-Redaction; (b) `GuidLabelPolicy` für GUID-Span-Attrs;
(c) `TopicLabelPolicy` für Topic-Label-Kardinalität. Beide dyn-frei.

**Repo:** `crates/monitor/src/config.rs` (`TopicLabelPolicy {Full,Truncate,
Hashed,Drop}`, `GuidLabelPolicy {Full,Hashed,Omit}`, `MonitorConfig`-Felder,
Helper `topic_label()`/`guid_label()`), angewandt in
`crates/dcps/src/metrics.rs` (`topic_labels()` auf alle Topic-Hook-Points).
(a) erfüllt durch das zahlen-only Datenmodell.

**Tests:** `crates/monitor/src/config.rs::tests::topic_policy_full_is_identity`,
`topic_policy_truncate_caps_length`, `topic_policy_hashed_is_stable_and_hex`,
`topic_policy_drop_omits_label`, `guid_policy_variants`,
`active_config_defaults_when_unset_helpers_apply`, `defaults_match_spec`.

**Status:** done

## §10 Test-Pflicht

**Spec:** §10 — Counter/Gauge-Konsistenz, Histogram-Re-Export, Prometheus-Text,
Label-Escaping, PID-Roundtrip, **Cross-Layer** (dcps-write → render), **Policy**.

**Repo:** `crates/monitor/src/` (§1–§4 + §9-Policy-Tests),
`crates/dcps/src/metrics.rs` (Cross-Layer-Test).

**Tests:** die §1–§4-Pflichttests +
`crates/dcps/src/metrics.rs::tests::write_increments_samples_written_and_renders`
(Cross-Layer) + die §9-Policy-Tests.

**Status:** done

## §11 Coverage-Doc

**Spec:** §11 — dieses Dokument; Akzeptanz „alle §-Sektionen done".

**Repo:** `docs/spec-coverage/zerodds-monitor-1.1.md`.

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

10 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-monitor` — 46 Tests grün, 0 failed; plus
`cargo test -p zerodds-dcps` (§10-Cross-Layer-Test grün, metrics-Feature default).

Offene Punkte: keine. Decision-Records: keine.
