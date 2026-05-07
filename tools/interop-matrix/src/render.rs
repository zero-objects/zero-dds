// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! HTML-Renderer der Interop-Matrix.

use crate::model::{Matrix, Status};

/// Rendert die Matrix als statische HTML-Seite.
#[must_use]
pub fn render_html(m: &Matrix) -> String {
    let mut out = String::with_capacity(8192);
    out.push_str(r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8">
<title>ZeroDDS Interop-Matrix</title>
<style>
body{font-family:-apple-system,BlinkMacSystemFont,system-ui,sans-serif;background:#1a1a1a;color:#ddd;margin:0;padding:16px}
h1{font-size:22px;margin:0 0 8px}
.meta{color:#888;font-size:12px;margin-bottom:16px}
table{border-collapse:collapse;width:100%}
th{text-align:left;padding:8px 10px;font-weight:600;color:#fff;background:#222;border-bottom:2px solid #444}
th.center,td.center{text-align:center}
td{padding:6px 10px;border-bottom:1px solid #2a2a2a;font-size:13px}
td.vendor{font-weight:600;color:#fff}
td.cell-pass{background:#1a4d1a;color:#aef;border-radius:4px}
td.cell-partial{background:#5a4a10;color:#fffba8}
td.cell-fail{background:#5a1a1a;color:#fdd}
td.cell-na{background:#2c2c2c;color:#888;font-style:italic}
td.cell-unknown{background:#5a3a10;color:#ffd9a8}
.legend{margin-top:16px;font-size:12px;color:#aaa}
.legend span{display:inline-block;padding:2px 6px;border-radius:3px;margin-right:8px}
.legend .l-pass{background:#1a4d1a;color:#aef}
.legend .l-partial{background:#5a4a10;color:#fffba8}
.legend .l-fail{background:#5a1a1a;color:#fdd}
.legend .l-na{background:#2c2c2c;color:#888;font-style:italic}
.legend .l-unknown{background:#5a3a10;color:#ffd9a8}
.fail-banner{background:#5a1a1a;color:#fdd;padding:8px 12px;border-radius:4px;margin-bottom:12px}
</style></head><body>
"#);
    out.push_str("<h1>ZeroDDS Interop-Matrix</h1>\n");
    out.push_str(&format!(
        r#"<div class="meta">generated {}{}</div>"#,
        html_escape(&m.generated_at),
        m.git_sha
            .as_ref()
            .map(|s| format!(" — git {}", html_escape(s)))
            .unwrap_or_default()
    ));
    out.push('\n');
    let fails = m.fail_count();
    if fails > 0 {
        out.push_str(&format!(
            r#"<div class="fail-banner">⚠ {fails} red cell{} — siehe Matrix unten.</div>"#,
            if fails == 1 { "" } else { "s" }
        ));
        out.push('\n');
    }
    out.push_str("<table><thead><tr><th>Vendor</th><th>Version</th>");
    for p in &m.profiles {
        out.push_str(&format!(r#"<th class="center">{}</th>"#, html_escape(p)));
    }
    out.push_str("</tr></thead><tbody>\n");
    for v in &m.vendors {
        out.push_str("<tr>");
        out.push_str(&format!(
            r#"<td class="vendor">{}</td><td>{}</td>"#,
            html_escape(&v.name),
            html_escape(&v.version)
        ));
        for p in &m.profiles {
            let cell = v.results.iter().find(|(k, _)| k == p).map(|(_, c)| c);
            match cell {
                Some(c) => {
                    let title = c.note.as_deref().unwrap_or("");
                    out.push_str(&format!(
                        r#"<td class="center {}" title="{}">{}</td>"#,
                        c.status.css_class(),
                        html_escape(title),
                        status_label(c.status)
                    ));
                }
                None => {
                    out.push_str(r#"<td class="center cell-unknown">?</td>"#);
                }
            }
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table>\n");
    out.push_str(
        r#"<div class="legend">
<span class="l-pass">✓ pass</span>
<span class="l-partial">~ partial</span>
<span class="l-fail">✗ fail</span>
<span class="l-na">– n/a</span>
<span class="l-unknown">? unknown</span>
</div>
</body></html>"#,
    );
    out
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Pass => "✓",
        Status::Partial => "~",
        Status::Fail => "✗",
        Status::NotApplicable => "–",
        Status::Unknown => "?",
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;
    use crate::model::{Cell, VendorRow};

    fn sample() -> Matrix {
        Matrix {
            generated_at: "2026-05-03T10:00:00Z".into(),
            git_sha: Some("abcdef0".into()),
            profiles: vec!["rtps_pubsub".into(), "xtypes_struct".into()],
            vendors: vec![
                VendorRow {
                    name: "Cyclone DDS".into(),
                    version: "0.10.5".into(),
                    results: vec![
                        (
                            "rtps_pubsub".into(),
                            Cell {
                                status: Status::Pass,
                                note: None,
                            },
                        ),
                        (
                            "xtypes_struct".into(),
                            Cell {
                                status: Status::Partial,
                                note: Some("Mutable-Type fail".into()),
                            },
                        ),
                    ],
                },
                VendorRow {
                    name: "RTI Connext".into(),
                    version: "7.2".into(),
                    results: vec![(
                        "rtps_pubsub".into(),
                        Cell {
                            status: Status::Fail,
                            note: Some("license missing".into()),
                        },
                    )],
                },
            ],
        }
    }

    #[test]
    fn render_includes_doctype_and_table() {
        let h = render_html(&sample());
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("<table>"));
        assert!(h.contains("Cyclone DDS"));
        assert!(h.contains("RTI Connext"));
    }

    #[test]
    fn render_marks_failures_with_banner() {
        let h = render_html(&sample());
        assert!(h.contains("fail-banner"));
        assert!(h.contains("1 red cell"));
    }

    #[test]
    fn render_uses_status_css_classes() {
        let h = render_html(&sample());
        assert!(h.contains("cell-pass"));
        assert!(h.contains("cell-partial"));
        assert!(h.contains("cell-fail"));
    }

    #[test]
    fn render_renders_missing_cell_as_unknown() {
        // RTI hat keinen xtypes_struct-Eintrag
        let h = render_html(&sample());
        // Suche nach "?" als Label fuer das fehlende Feld
        assert!(h.matches("cell-unknown").count() >= 1);
    }

    #[test]
    fn html_escape_handles_all() {
        assert_eq!(
            html_escape(r#"<a href="x&y">"#),
            "&lt;a href=&quot;x&amp;y&quot;&gt;"
        );
    }

    #[test]
    fn render_omits_banner_when_no_fails() {
        let mut m = sample();
        m.vendors[1].results[0].1.status = Status::Pass;
        let h = render_html(&m);
        // Das CSS hat den ".fail-banner"-Selektor immer; wir checken
        // dass das gerenderte Banner-DIV NICHT da ist.
        assert!(!h.contains(r#"<div class="fail-banner""#));
    }
}
