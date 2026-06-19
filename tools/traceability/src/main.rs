// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-traceability` — requirements-to-code matrix generator.
//!
//! Parses the spec-coverage docs under `docs/spec-coverage/` (the house format:
//! a `## §…` heading per requirement, with `**Status:**` / `**Repo:**` /
//! `**Tests:**` / `**Spec:**` fields) and emits a traceability matrix mapping
//! each requirement to its implementation status, code, and tests.
//!
//! ```text
//! zerodds-traceability [--format text|md|json] [--status <filter>] <path>...
//!   <path>            a coverage .md file or a directory of them (`.en.md`
//!                     mirrors are skipped). Defaults to docs/spec-coverage.
//!   --format          text (default) | md (GitHub table) | json
//!   --status <s>      only rows whose normalized status starts with <s>
//!                     (e.g. open, partial, done, n/a)
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI-Tool: stdout/stderr sind die Ausgabe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_DIR: &str = "docs/spec-coverage";

const USAGE: &str = "\
zerodds-traceability — requirements-to-code matrix generator

USAGE:
    zerodds-traceability [--format text|md|json] [--status <filter>] [<path>...]

ARGS:
    <path>          coverage .md file or directory (default: docs/spec-coverage);
                    `.en.md` mirrors are skipped when scanning a directory

OPTIONS:
    --format <fmt>  text (default) | md | json
    --status <s>    keep only rows whose normalized status starts with <s>
    -h, --help      show this usage

EXIT CODES:
    0  matrix emitted      2  bad invocation / unreadable path";

/// One requirement row in the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    source: String,
    section: String,
    status: String,
    repo: String,
    tests: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Md,
    Json,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("zerodds-traceability: {msg}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut format = Format::Text;
    let mut status_filter: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--format" => {
                i += 1;
                format = match args.get(i).map(String::as_str) {
                    Some("text") => Format::Text,
                    Some("md") => Format::Md,
                    Some("json") => Format::Json,
                    other => return Err(format!("--format expects text|md|json, got {other:?}")),
                };
            }
            "--status" => {
                i += 1;
                status_filter = Some(
                    args.get(i)
                        .ok_or("--status needs a value")?
                        .to_ascii_lowercase(),
                );
            }
            flag if flag.starts_with('-') => return Err(format!("unknown flag `{flag}`")),
            other => paths.push(other.to_string()),
        }
        i += 1;
    }

    if paths.is_empty() {
        paths.push(DEFAULT_DIR.to_string());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for p in &paths {
        collect_md_files(Path::new(p), &mut files)?;
    }
    files.sort();

    let mut items: Vec<Item> = Vec::new();
    for f in &files {
        let md = std::fs::read_to_string(f)
            .map_err(|e| format!("cannot read `{}`: {e}", f.display()))?;
        let source = f
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.display().to_string());
        items.extend(parse_items(&md, &source));
    }

    if let Some(filter) = &status_filter {
        items.retain(|it| normalize_status(&it.status).starts_with(filter));
    }

    match format {
        Format::Text => render_text(&items),
        Format::Md => render_md(&items),
        Format::Json => render_json(&items),
    }
    Ok(())
}

/// Collect `.md` coverage files from a path (file or directory). Directory scan
/// skips `.en.md` mirrors (duplicated English content).
fn collect_md_files(p: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if p.is_file() {
        out.push(p.to_path_buf());
        return Ok(());
    }
    if p.is_dir() {
        let rd =
            std::fs::read_dir(p).map_err(|e| format!("cannot read dir `{}`: {e}", p.display()))?;
        for entry in rd {
            let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
            let path = entry.path();
            let name = path.file_name().map(|s| s.to_string_lossy().into_owned());
            let is_md = name.as_deref().is_some_and(|n| n.ends_with(".md"));
            let is_en = name.as_deref().is_some_and(|n| n.ends_with(".en.md"));
            if path.is_file() && is_md && !is_en {
                out.push(path);
            }
        }
        return Ok(());
    }
    Err(format!("path not found: `{}`", p.display()))
}

/// Split a coverage doc into heading blocks (level >= 2) and emit one [`Item`]
/// per block that carries a `**Status:**` field.
fn parse_items(md: &str, source: &str) -> Vec<Item> {
    let mut blocks: Vec<(String, Vec<&str>)> = Vec::new();
    for line in md.lines() {
        if let Some(h) = heading(line) {
            blocks.push((h, Vec::new()));
        } else if let Some(last) = blocks.last_mut() {
            last.1.push(line);
        }
    }

    let mut items = Vec::new();
    for (section, body) in &blocks {
        let Some(status) = field(body, "Status") else {
            continue;
        };
        items.push(Item {
            source: source.to_string(),
            section: section.clone(),
            status,
            repo: field(body, "Repo").unwrap_or_default(),
            tests: field(body, "Tests").unwrap_or_default(),
        });
    }
    items
}

/// Return the heading text for a level-2..4 ATX heading (`## `, `### `, `#### `).
fn heading(line: &str) -> Option<String> {
    for marker in ["#### ", "### ", "## "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some(rest.trim().replace('`', ""));
        }
    }
    None
}

/// First inline value of a `**Name:**` field within a block body.
fn field(body: &[&str], name: &str) -> Option<String> {
    let prefix = format!("**{name}:**");
    for line in body {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix(&prefix) {
            return Some(rest.trim().trim_matches('`').trim().to_string());
        }
    }
    None
}

/// Normalize a status string to its leading token, lowercased, stripped of
/// backticks/markup — e.g. "`n/a (informative)` — Glossar." -> "n/a".
fn normalize_status(s: &str) -> String {
    let cleaned = s.trim_start_matches('`').trim();
    cleaned
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('`')
        .to_ascii_lowercase()
}

fn status_counts(items: &[Item]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for it in items {
        *counts.entry(normalize_status(&it.status)).or_insert(0) += 1;
    }
    counts
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

fn render_text(items: &[Item]) {
    if items.is_empty() {
        println!("(no requirements matched)");
        return;
    }
    let mut current = "";
    for it in items {
        if it.source != current {
            println!("\n# {}", it.source);
            current = &it.source;
        }
        println!(
            "  [{:<8}] {}\n             repo:  {}\n             tests: {}",
            truncate(&normalize_status(&it.status), 8),
            it.section,
            truncate(&it.repo, 88),
            truncate(&it.tests, 88),
        );
    }
    println!("\nsummary: {} requirement(s)", items.len());
    for (status, n) in status_counts(items) {
        println!("  {status:<10} {n}");
    }
}

fn render_md(items: &[Item]) {
    println!("| Source | Requirement | Status | Repo | Tests |");
    println!("| --- | --- | --- | --- | --- |");
    for it in items {
        println!(
            "| {} | {} | {} | {} | {} |",
            md_cell(&it.source),
            md_cell(&it.section),
            md_cell(&normalize_status(&it.status)),
            md_cell(&it.repo),
            md_cell(&it.tests),
        );
    }
}

/// Escape a value for a Markdown table cell (pipes + newlines).
fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn render_json(items: &[Item]) {
    print!("[");
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!(
            "{{\"source\":{},\"section\":{},\"status\":{},\"repo\":{},\"tests\":{}}}",
            json_str(&it.source),
            json_str(&it.section),
            json_str(&it.status),
            json_str(&it.repo),
            json_str(&it.tests),
        );
    }
    println!("]");
}

/// Minimal JSON string encoder (quotes + escapes the control set we emit).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"# Some Spec — Coverage

Intro prose, no status here.

## §1 Scope

**Repo:** `crates/dcps/src/lib.rs`

**Tests:** Workspace-wide.

**Status:** done — fully implemented.

## §2 Glossary

**Status:** `n/a (informative)` — Glossar.

## §3 Backlog item

**Repo:** `crates/foo`

**Status:** open — pending.
"#;

    #[test]
    fn parses_status_bearing_sections_only() {
        let items = parse_items(DOC, "some-spec.md");
        // §1, §2, §3 carry Status; the title + intro do not.
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].section, "§1 Scope");
        assert_eq!(items[0].repo, "crates/dcps/src/lib.rs");
        assert_eq!(normalize_status(&items[0].status), "done");
        assert_eq!(normalize_status(&items[1].status), "n/a");
        assert_eq!(normalize_status(&items[2].status), "open");
    }

    #[test]
    fn status_counts_aggregate() {
        let items = parse_items(DOC, "s.md");
        let counts = status_counts(&items);
        assert_eq!(counts.get("done"), Some(&1));
        assert_eq!(counts.get("open"), Some(&1));
        assert_eq!(counts.get("n/a"), Some(&1));
    }

    #[test]
    fn status_filter_keeps_matching_rows() {
        let items: Vec<Item> = parse_items(DOC, "s.md")
            .into_iter()
            .filter(|it| normalize_status(&it.status).starts_with("open"))
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].section, "§3 Backlog item");
    }

    #[test]
    fn heading_only_matches_level_2_plus() {
        assert_eq!(heading("## §1 X"), Some("§1 X".to_string()));
        assert_eq!(heading("### §1.2 Y"), Some("§1.2 Y".to_string()));
        assert_eq!(heading("# Title"), None);
        assert_eq!(heading("plain"), None);
    }

    #[test]
    fn json_escapes_quotes() {
        assert_eq!(json_str("a\"b"), "\"a\\\"b\"");
    }
}
