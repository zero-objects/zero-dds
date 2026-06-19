# `zerodds-observability-otlp` v1.0 — OTLP/HTTP/JSON exporter — Spec coverage

**Source:** `docs/specs/zerodds-observability-otlp-1.0.md` (ZeroDDS vendor spec,
OTLP/HTTP/JSON exporter, modelled on OpenTelemetry proto v1.4).

**Context:** `crates/observability-otlp` buffers traces/histograms/logs and
pushes them as OTLP/HTTP/JSON to an OTel collector. Own `Span`/`Histogram`/
`Event` types (no dependency cycle to `zerodds-monitor`); 8 tests green.

Implementation:

- `crates/observability-otlp/` — `OtlpExporter`, `OtlpConfig`, `ExportError`,
  `add_span`/`add_histogram`/`add_event`/`flush`; 8 tests green.

---

## §1 Architecture — buffer + flush

**Spec:** §1, "Architektur" — the exporter buffers telemetry and pushes it
periodically / on `flush()` to an OTLP/HTTP/JSON collector; no collector = no
panic.

**Repo:** `crates/observability-otlp/src/lib.rs` (`OtlpExporter`,
`ExporterBuffers`).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::flush_drains_buffers_even_with_no_collector`.

**Status:** done

## §2.1 Endpoint `/v1/traces`

**Spec:** §2.1 — spans/events are sent as OTLP trace JSON to `/v1/traces`;
spec-conformant string escaping.

**Repo:** `crates/observability-otlp/src/lib.rs` (`add_span`, `add_event`, trace
JSON builder).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::traces_json_roundtrip_shape`,
`json_escape_handles_quotes_and_newlines`.

**Status:** done

## §2.2 Endpoint `/v1/metrics`

**Spec:** §2.2 — histograms are sent as OTLP metric JSON to `/v1/metrics`.

**Repo:** `crates/observability-otlp/src/lib.rs` (`add_histogram`, metric JSON
builder).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::metrics_json_roundtrip_shape`.

**Status:** done

## §2.3 Endpoint `/v1/logs`

**Spec:** §2.3 — structured logs are sent as OTLP log JSON to `/v1/logs`.

**Repo:** `crates/observability-otlp/src/lib.rs` (log JSON builder).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::logs_json_roundtrip_shape`.

**Status:** done

## §3 Configuration

**Spec:** §3, "Konfiguration" — `OtlpConfig` with a collector endpoint
(default localhost) and exporter parameters.

**Repo:** `crates/observability-otlp/src/lib.rs` (`OtlpConfig`).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::config_default_points_to_localhost`.

**Status:** done

## §4 Lifecycle + HTTP error handling

**Spec:** §4, "Lifecycle" — `flush()` drains the buffers; an HTTP error status is
reported as `ExportError::HttpStatus`, connect-refused as `ExportError::Io` — no
panic.

**Repo:** `crates/observability-otlp/src/lib.rs` (`flush`, `ExportError`, HTTP
status parser).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::flush_drains_buffers_even_with_no_collector`,
`parse_http_status_extracts_code`, `parse_http_status_handles_500`.

**Status:** done

## §5 Bridge to `zerodds-monitor::Registry`

**Spec:** §5 — the caller exports via `Registry::snapshot()`; the exporter takes
`Histogram` instances directly. Counter/gauge go through Prometheus only, **by
design** (no OTLP double-export).

**Repo:** `crates/observability-otlp/src/lib.rs` (`add_histogram` = ingestion
point for the caller-side snapshot bridge; deliberately no `zerodds-monitor`
dependency, to avoid a cycle).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::metrics_json_roundtrip_shape`.

**Status:** done (histogram export + caller-side snapshot bridge as specified;
the counter/gauge omission is the documented spec design decision, not a gap)

## §6 Security

**Spec:** §6 — TLS is out-of-scope for the built-in HTTP/1.1 stub (the caller
runs a local OTel collector sidecar with a TLS frontend); authentication headers
are a major-additive extension.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — the spec deliberately scopes TLS/auth out
(sidecar delegation); no in-crate normative requirement.

## §7 Stability

**Spec:** §7 — public API stable; wire format modelled on OTel v1.4, schema
change = major bump.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — stability policy, not an implementation
requirement.

## §8 Test obligation

**Spec:** §8 — mandatory: config→JSON roundtrip smoke test, HTTP 500 error
handling (`ExportError::HttpStatus`), connect-refused without panic.

**Repo:** `crates/observability-otlp/src/lib.rs` (test module).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::config_default_points_to_localhost`,
`parse_http_status_handles_500`, `flush_drains_buffers_even_with_no_collector`,
`traces_json_roundtrip_shape`, `metrics_json_roundtrip_shape`,
`logs_json_roundtrip_shape`.

**Status:** done

---

## Audit status

8 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-observability-otlp` — 8 tests green, 0 failed.

Open items: none. Decision records: none.
