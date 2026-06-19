// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Prometheus-Text-Format-Exporter (Spec §3).

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::registry::RegistrySnapshot;

/// Histogram-Bucket-Bounds in Sekunden (Spec §3.1).
const PROM_BUCKET_BOUNDS_SECONDS: &[f64] = &[
    1e-09, 1e-08, 1e-07, 1e-06, 1e-05, 1e-04, 1e-03, 1e-02, 1e-01, 1.0, 10.0,
];

/// Renders a snapshot in Prometheus exposition format v0.0.4.
#[must_use]
pub fn render_prometheus(snap: &RegistrySnapshot) -> String {
    let helps: BTreeMap<&'static str, &'static str> = snap.helps.iter().copied().collect();

    let mut by_name: BTreeMap<&'static str, MetricGroup> = BTreeMap::new();
    for (key, value) in &snap.counters {
        by_name
            .entry(key.name)
            .or_insert(MetricGroup::Counter(Vec::new()))
            .push_counter(key, *value);
    }
    for (key, value) in &snap.gauges {
        by_name
            .entry(key.name)
            .or_insert(MetricGroup::Gauge(Vec::new()))
            .push_gauge(key, *value);
    }
    for (key, hist) in &snap.histograms {
        by_name
            .entry(key.name)
            .or_insert(MetricGroup::Histogram(Vec::new()))
            .push_histogram(key, hist);
    }

    let mut out = String::new();
    let mut first = true;
    for (name, group) in &by_name {
        if !first {
            out.push('\n');
        }
        first = false;
        let help = helps.get(name).copied().unwrap_or("");
        write_help_and_type(&mut out, name, group.kind(), help);
        match group {
            MetricGroup::Counter(items) => render_counter_lines(&mut out, name, items),
            MetricGroup::Gauge(items) => render_gauge_lines(&mut out, name, items),
            MetricGroup::Histogram(items) => render_histogram_lines(&mut out, name, items),
        }
    }
    out
}

enum MetricGroup<'a> {
    Counter(Vec<(&'a crate::Labels, u64)>),
    Gauge(Vec<(&'a crate::Labels, i64)>),
    Histogram(
        Vec<(
            &'a crate::Labels,
            &'a zerodds_foundation::tracing::Histogram,
        )>,
    ),
}

impl<'a> MetricGroup<'a> {
    fn push_counter(&mut self, key: &'a crate::MetricKey, v: u64) {
        if let MetricGroup::Counter(items) = self {
            items.push((&key.labels, v));
        }
    }
    fn push_gauge(&mut self, key: &'a crate::MetricKey, v: i64) {
        if let MetricGroup::Gauge(items) = self {
            items.push((&key.labels, v));
        }
    }
    fn push_histogram(
        &mut self,
        key: &'a crate::MetricKey,
        h: &'a zerodds_foundation::tracing::Histogram,
    ) {
        if let MetricGroup::Histogram(items) = self {
            items.push((&key.labels, h));
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            MetricGroup::Counter(_) => "counter",
            MetricGroup::Gauge(_) => "gauge",
            MetricGroup::Histogram(_) => "histogram",
        }
    }
}

fn write_help_and_type(out: &mut String, name: &str, kind: &str, help: &str) {
    if !help.is_empty() {
        let _ = writeln!(out, "# HELP {name} {help}");
    }
    let _ = writeln!(out, "# TYPE {name} {kind}");
}

fn render_counter_lines(out: &mut String, name: &str, items: &[(&crate::Labels, u64)]) {
    let mut sorted: Vec<&(&crate::Labels, u64)> = items.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (labels, v) in sorted {
        let lbl = render_labels(labels);
        let _ = writeln!(out, "{name}{lbl} {v}");
    }
}

fn render_gauge_lines(out: &mut String, name: &str, items: &[(&crate::Labels, i64)]) {
    let mut sorted: Vec<&(&crate::Labels, i64)> = items.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (labels, v) in sorted {
        let lbl = render_labels(labels);
        let _ = writeln!(out, "{name}{lbl} {v}");
    }
}

fn render_histogram_lines(
    out: &mut String,
    name: &str,
    items: &[(&crate::Labels, &zerodds_foundation::tracing::Histogram)],
) {
    let mut sorted: Vec<&(&crate::Labels, &zerodds_foundation::tracing::Histogram)> =
        items.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (labels, hist) in sorted {
        let mut cumulative: u64 = 0;
        for (idx, bound) in PROM_BUCKET_BOUNDS_SECONDS.iter().enumerate() {
            cumulative = cumulative.saturating_add(hist.buckets[idx]);
            let lbl = render_labels_with_extra(labels, "le", &format_seconds(*bound));
            let _ = writeln!(out, "{name}_bucket{lbl} {cumulative}");
        }
        // the last +Inf bucket reflects the total number of records
        // (catches overflow above 10s).
        let lbl_inf = render_labels_with_extra(labels, "le", "+Inf");
        let _ = writeln!(out, "{name}_bucket{lbl_inf} {}", hist.count);

        let lbl = render_labels(labels);
        let sum_seconds = (hist.sum_ns as f64) * 1e-9;
        let _ = writeln!(out, "{name}_sum{lbl} {}", format_float(sum_seconds));
        let _ = writeln!(out, "{name}_count{lbl} {}", hist.count);
    }
}

fn render_labels(labels: &crate::Labels) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in labels.iter() {
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "{k}=\"{}\"", escape_label_value(v));
    }
    out.push('}');
    out
}

fn render_labels_with_extra(labels: &crate::Labels, extra_key: &str, extra_value: &str) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in labels.iter() {
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "{k}=\"{}\"", escape_label_value(v));
    }
    if !first {
        out.push(',');
    }
    let _ = write!(out, "{extra_key}=\"{extra_value}\"");
    out.push('}');
    out
}

fn escape_label_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

fn format_seconds(s: f64) -> String {
    if s == 0.0 {
        "0".to_string()
    } else {
        // Prometheus accepts both `1e-09` and `0.000000001`;
        // we use the compact exponential form.
        let exp = s.log10().round() as i32;
        let test = 10f64.powi(exp);
        if (test - s).abs() < s * 1e-6 {
            format!("1e{exp:+03}")
        } else {
            format_float(s)
        }
    }
}

fn format_float(f: f64) -> String {
    if f == 0.0 {
        "0".to_string()
    } else {
        format!("{f}")
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use crate::{Labels, Registry};

    #[test]
    fn render_counter_with_labels() {
        let r = Registry::new();
        r.set_help("dds_test_total", "Test counter");
        let c = r.counter("dds_test_total", Labels::new().with("topic", "A"));
        c.add(42);
        let txt = r.render_prometheus();
        assert!(txt.contains("# HELP dds_test_total Test counter"));
        assert!(txt.contains("# TYPE dds_test_total counter"));
        assert!(txt.contains(r#"dds_test_total{topic="A"} 42"#));
    }

    #[test]
    fn render_gauge() {
        let r = Registry::new();
        let g = r.gauge("dds_test_g", Labels::new());
        g.set(-7);
        let txt = r.render_prometheus();
        assert!(txt.contains("# TYPE dds_test_g gauge"));
        assert!(txt.contains("dds_test_g -7"));
    }

    #[test]
    fn render_histogram_has_buckets_sum_count() {
        let r = Registry::new();
        let h = r.histogram("dds_test_h", Labels::new());
        h.record_ns(500);
        h.record_ns(2_000_000);
        let txt = r.render_prometheus();
        assert!(txt.contains("# TYPE dds_test_h histogram"));
        assert!(txt.contains(r#"dds_test_h_bucket{le="1e-06"}"#));
        assert!(txt.contains(r#"dds_test_h_bucket{le="+Inf"} 2"#));
        assert!(txt.contains("dds_test_h_count 2"));
        assert!(txt.contains("dds_test_h_sum"));
    }

    #[test]
    fn render_label_escaping() {
        let r = Registry::new();
        let c = r.counter(
            "dds_test_esc_total",
            Labels::new().with("path", "a\"b\\c\n"),
        );
        c.inc();
        let txt = r.render_prometheus();
        assert!(txt.contains(r#"path="a\"b\\c\n""#));
    }

    #[test]
    fn render_sorted_by_metric_name() {
        let r = Registry::new();
        r.counter("dds_b_total", Labels::new()).inc();
        r.counter("dds_a_total", Labels::new()).inc();
        let txt = r.render_prometheus();
        let pa = txt.find("dds_a_total").expect("a present");
        let pb = txt.find("dds_b_total").expect("b present");
        assert!(pa < pb, "metrics must be sorted: {txt}");
    }

    #[test]
    fn render_empty_registry_is_empty_string() {
        let r = Registry::new();
        assert_eq!(r.render_prometheus(), "");
    }
}
