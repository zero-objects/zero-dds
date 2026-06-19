# `zerodds-observability-otlp` v1.0 — OTLP/HTTP/JSON-Exporter — Spec-Coverage

**Quelle:** `docs/specs/zerodds-observability-otlp-1.0.md` (ZeroDDS-Vendor-Spec,
OTLP/HTTP/JSON-Exporter, an OpenTelemetry-Proto v1.4 angelehnt).

**Kontext:** `crates/observability-otlp` puffert Traces/Histogramme/Logs und
pusht sie als OTLP/HTTP/JSON an einen OTel-Collector. Eigene `Span`/`Histogram`/
`Event`-Typen (kein Dep-Zyklus zu `zerodds-monitor`); 8 Tests grün.

Implementation:

- `crates/observability-otlp/` — `OtlpExporter`, `OtlpConfig`, `ExportError`,
  `add_span`/`add_histogram`/`add_event`/`flush`; 8 Tests grün.

---

## §1 Architektur — Puffer + Flush

**Spec:** §1, „Architektur" — Exporter puffert Telemetrie und pusht sie
periodisch/per `flush()` an einen OTLP/HTTP/JSON-Collector; kein Collector =
kein Panic.

**Repo:** `crates/observability-otlp/src/lib.rs` (`OtlpExporter`,
`ExporterBuffers`).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::flush_drains_buffers_even_with_no_collector`.

**Status:** done

## §2.1 Endpoint `/v1/traces`

**Spec:** §2.1 — Spans/Events werden als OTLP-Trace-JSON an `/v1/traces`
gesendet; String-Escaping spec-konform.

**Repo:** `crates/observability-otlp/src/lib.rs` (`add_span`, `add_event`,
Trace-JSON-Builder).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::traces_json_roundtrip_shape`,
`json_escape_handles_quotes_and_newlines`.

**Status:** done

## §2.2 Endpoint `/v1/metrics`

**Spec:** §2.2 — Histogramme werden als OTLP-Metric-JSON an `/v1/metrics`
gesendet.

**Repo:** `crates/observability-otlp/src/lib.rs` (`add_histogram`,
Metric-JSON-Builder).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::metrics_json_roundtrip_shape`.

**Status:** done

## §2.3 Endpoint `/v1/logs`

**Spec:** §2.3 — strukturierte Logs werden als OTLP-Log-JSON an `/v1/logs`
gesendet.

**Repo:** `crates/observability-otlp/src/lib.rs` (Log-JSON-Builder).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::logs_json_roundtrip_shape`.

**Status:** done

## §3 Konfiguration

**Spec:** §3, „Konfiguration" — `OtlpConfig` mit Collector-Endpoint
(Default localhost) und Exporter-Parametern.

**Repo:** `crates/observability-otlp/src/lib.rs` (`OtlpConfig`).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::config_default_points_to_localhost`.

**Status:** done

## §4 Lifecycle + HTTP-Fehlerbehandlung

**Spec:** §4, „Lifecycle" — `flush()` leert die Puffer; HTTP-Fehlerstatus wird
als `ExportError::HttpStatus` gemeldet, Connect-Refused als `ExportError::Io` —
kein Panic.

**Repo:** `crates/observability-otlp/src/lib.rs` (`flush`, `ExportError`,
HTTP-Status-Parser).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::flush_drains_buffers_even_with_no_collector`,
`parse_http_status_extracts_code`, `parse_http_status_handles_500`.

**Status:** done

## §5 Bridge zu `zerodds-monitor::Registry`

**Spec:** §5 — der Caller exportiert via `Registry::snapshot()`; der Exporter
nimmt `Histogram`-Instanzen direkt. Counter/Gauge laufen **per
Designentscheidung** nur über Prometheus (kein OTLP-Doppel-Export).

**Repo:** `crates/observability-otlp/src/lib.rs` (`add_histogram` = Ingestions-
Punkt für den caller-seitigen Snapshot-Bridge; bewusst kein Dep auf
`zerodds-monitor`, um einen Zyklus zu vermeiden).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::metrics_json_roundtrip_shape`.

**Status:** done (Histogram-Export + caller-seitiger Snapshot-Bridge wie in der
Spec spezifiziert; Counter/Gauge-Weglassung ist die dokumentierte Spec-Design-
entscheidung, kein Gap)

## §6 Sicherheit

**Spec:** §6 — TLS ist out-of-scope für den built-in HTTP/1.1-Stub (Caller setzt
einen lokalen OTel-Collector-Sidecar mit TLS-Frontend); Authentication-Header
sind eine Major-additive Erweiterung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — die Spec scoped TLS/Auth bewusst aus
(Sidecar-Delegation); keine in-crate normative Anforderung.

## §7 Stabilität

**Spec:** §7 — Public-API stabil; Wire-Format an OTel-v1.4 angelehnt,
Schema-Änderung = Major-Bump.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Stabilitäts-Policy, keine Implementierungs-
Anforderung.

## §8 Test-Pflicht

**Spec:** §8 — verpflichtend: Config→JSON-Roundtrip-Smoketest,
HTTP-500-Fehlerbehandlung (`ExportError::HttpStatus`), Connect-Refused ohne
Panic.

**Repo:** `crates/observability-otlp/src/lib.rs` (Test-Modul).

**Tests:** `crates/observability-otlp/src/lib.rs::tests::config_default_points_to_localhost`,
`parse_http_status_handles_500`, `flush_drains_buffers_even_with_no_collector`,
`traces_json_roundtrip_shape`, `metrics_json_roundtrip_shape`,
`logs_json_roundtrip_shape`.

**Status:** done

---

## Audit-Status

8 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-observability-otlp` — 8 Tests grün, 0 failed.

Offene Punkte: keine. Decision-Records: keine.
