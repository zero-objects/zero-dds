// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Workspace-Crate-DAG-Sort fuer cargo publish.
//!
//! Crate `zerodds-cargo-dag`. Safety classification: **COMFORT**.
//!
//! # Was tut das Tool?
//!
//! Liest die Workspace-`Cargo.toml`, dann pro Member-Crate den Block
//! `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]` und
//! baut den Inter-Crate-Dependency-Graph. Topologische Sortierung gibt
//! eine zykel-freie Reihenfolge — das ist die `cargo publish`-Order.
//!
//! Dev-Dependencies werden ignoriert (sind fuer Publish irrelevant).
//! Build-Dependencies werden mitgezaehlt (z.B. `cbindgen`-Aufrufer).
//!
//! # Was ist das nicht?
//!
//! Kein voller Cargo-Resolver — wir parsen nur die Workspace-internen
//! `path = "..."`-Refs. Externe Crates (crates.io-Deps) ignorieren wir.
//! Reicht fuer den Publish-Order-Use-Case.

#![warn(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Beschreibt eine Workspace-Crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateInfo {
    /// Cargo-Package-Name (`name = "..."`).
    pub name: String,
    /// Member-Pfad relativ zum Workspace-Root.
    pub path: PathBuf,
    /// `publish = false` gesetzt? (default: false → publishable).
    pub publish_blocked: bool,
    /// Set von Workspace-internen Path-Deps (Cargo-Package-Names).
    pub deps: BTreeSet<String>,
}

/// Lese-Fehler.
#[derive(Debug)]
pub enum DagError {
    /// IO-Fehler beim Lesen einer Cargo.toml.
    Io {
        /// Welche Datei.
        path: PathBuf,
        /// Underlying error.
        err: std::io::Error,
    },
    /// Workspace-`members`-Block nicht gefunden.
    MissingMembers,
    /// Crate-Name nicht extrahierbar.
    MissingPackageName(PathBuf),
    /// Zyklus im DAG.
    Cycle(Vec<String>),
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, err } => write!(f, "io {}: {err}", path.display()),
            Self::MissingMembers => write!(f, "workspace.members not found in root Cargo.toml"),
            Self::MissingPackageName(p) => write!(f, "missing [package] name in {}", p.display()),
            Self::Cycle(c) => write!(f, "cycle detected: {}", c.join(" → ")),
        }
    }
}

impl std::error::Error for DagError {}

/// Sammelt alle Workspace-Crates mit Inter-Crate-Deps.
///
/// # Errors
/// IO/Parse/Cycle-Fehler.
pub fn collect_crates(workspace_root: &Path) -> Result<Vec<CrateInfo>, DagError> {
    let root_toml_path = workspace_root.join("Cargo.toml");
    let root_toml = fs::read_to_string(&root_toml_path).map_err(|e| DagError::Io {
        path: root_toml_path.clone(),
        err: e,
    })?;
    let members = parse_members(&root_toml).ok_or(DagError::MissingMembers)?;
    let mut out = Vec::with_capacity(members.len());
    for m in members {
        let crate_dir = workspace_root.join(&m);
        let toml_path = crate_dir.join("Cargo.toml");
        let toml = fs::read_to_string(&toml_path).map_err(|e| DagError::Io {
            path: toml_path.clone(),
            err: e,
        })?;
        let name = parse_package_name(&toml)
            .ok_or_else(|| DagError::MissingPackageName(toml_path.clone()))?;
        let publish_blocked = parse_publish_blocked(&toml);
        let deps = parse_path_deps(&toml);
        out.push(CrateInfo {
            name,
            path: PathBuf::from(m),
            publish_blocked,
            deps,
        });
    }
    Ok(out)
}

/// Topologisch sortiert — Kahn's Algorithmus mit deterministischem
/// Tie-Break ueber Crate-Namen (alphabetisch).
///
/// Returnt eine Reihenfolge so dass jeder Crate vor seinen Dependents
/// erscheint. `publish_blocked` Crates werden mit-aufgenommen aber
/// markiert; der Caller filtert ggf. heraus.
///
/// # Errors
/// `Cycle` wenn der Graph einen Zyklus hat.
pub fn topo_sort(crates: &[CrateInfo]) -> Result<Vec<CrateInfo>, DagError> {
    // Map Name → Info-Index, sodass wir mit u32 in HashSets sparsam
    // arbeiten koennen.
    let by_name: BTreeMap<&str, usize> = crates
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.as_str(), i))
        .collect();
    // Pro-Crate-Indegree: wie viele Workspace-Deps haben wir.
    let mut indegree: Vec<usize> = vec![0; crates.len()];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); crates.len()]; // dep → dependent
    for (i, c) in crates.iter().enumerate() {
        for d in &c.deps {
            if let Some(&j) = by_name.get(d.as_str()) {
                adj[j].push(i);
                indegree[i] += 1;
            }
        }
    }
    // Kahn — Priority-Queue ueber Crate-Namen, damit deterministisch.
    let mut ready: BTreeSet<(String, usize)> = crates
        .iter()
        .enumerate()
        .filter(|(i, _)| indegree[*i] == 0)
        .map(|(i, c)| (c.name.clone(), i))
        .collect();
    let mut order = Vec::with_capacity(crates.len());
    while let Some((name, i)) = ready.pop_first() {
        let _ = name;
        order.push(crates[i].clone());
        for &j in &adj[i] {
            indegree[j] -= 1;
            if indegree[j] == 0 {
                ready.insert((crates[j].name.clone(), j));
            }
        }
    }
    if order.len() < crates.len() {
        // Restliche sind im Zyklus.
        let cycle: Vec<String> = crates
            .iter()
            .enumerate()
            .filter(|(i, _)| indegree[*i] > 0)
            .map(|(_, c)| c.name.clone())
            .collect();
        return Err(DagError::Cycle(cycle));
    }
    Ok(order)
}

/// Filtert nicht-publishable (publish=false) raus.
#[must_use]
pub fn only_publishable(crates: &[CrateInfo]) -> Vec<CrateInfo> {
    crates
        .iter()
        .filter(|c| !c.publish_blocked)
        .cloned()
        .collect()
}

// ----- TOML-Parsing — primitive Linien-Heuristik, reicht fuer
// unsere Crates (alle nutzen kanonisches Format). Kein toml-Crate-Dep. -----

fn parse_members(toml: &str) -> Option<Vec<String>> {
    // Erste `members = [` finden, dann bis `]`.
    let mut iter = toml.lines();
    let mut in_block = false;
    let mut buf = String::new();
    for line in &mut iter {
        let stripped = strip_comment(line);
        if !in_block {
            if stripped.trim_start().starts_with("members") {
                if let Some(eq) = stripped.find('=') {
                    let after = &stripped[eq + 1..];
                    if let Some(start) = after.find('[') {
                        in_block = true;
                        buf.push_str(&after[start + 1..]);
                        if let Some(end) = buf.find(']') {
                            buf.truncate(end);
                            break;
                        }
                    }
                }
            }
        } else if let Some(end) = stripped.find(']') {
            buf.push_str(&stripped[..end]);
            break;
        } else {
            buf.push_str(stripped);
            buf.push('\n');
        }
    }
    if !in_block {
        return None;
    }
    // Strings extrahieren.
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in buf.chars() {
        if c == '"' {
            if quoted && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            quoted = !quoted;
        } else if quoted {
            cur.push(c);
        }
    }
    Some(out)
}

fn parse_package_name(toml: &str) -> Option<String> {
    // Ersten `[package]`-Block + `name = "..."`-Zeile.
    let mut in_pkg = false;
    for line in toml.lines() {
        let s = strip_comment(line);
        let t = s.trim();
        if t.starts_with('[') {
            in_pkg = t == "[package]";
        } else if in_pkg && t.starts_with("name") {
            if let Some(eq) = t.find('=') {
                let v = t[eq + 1..].trim();
                if let Some(stripped) = v.strip_prefix('"') {
                    if let Some(end) = stripped.find('"') {
                        return Some(stripped[..end].to_string());
                    }
                }
            }
        }
    }
    None
}

fn parse_publish_blocked(toml: &str) -> bool {
    let mut in_pkg = false;
    for line in toml.lines() {
        let s = strip_comment(line);
        let t = s.trim();
        if t.starts_with('[') {
            in_pkg = t == "[package]";
        } else if in_pkg && t.starts_with("publish") {
            // `publish = false` oder `publish = []`
            if t.contains("false") || t.contains("[]") {
                return true;
            }
        }
    }
    false
}

fn parse_path_deps(toml: &str) -> BTreeSet<String> {
    // Wir sammeln alle `name = { path = "..." ... }`-Eintraege aus
    // [dependencies] und [build-dependencies]. Inline-Tabellen
    // erkennen wir per `path = "`-Substring.
    let mut out = BTreeSet::new();
    let mut current_section = String::new();
    for line in toml.lines() {
        let s = strip_comment(line);
        let t = s.trim();
        if t.starts_with('[') {
            current_section = t.to_string();
            continue;
        }
        if !is_dep_section(&current_section) {
            continue;
        }
        // Extract crate name = LHS up to `=`.
        let Some(eq) = t.find('=') else {
            continue;
        };
        let lhs = t[..eq].trim();
        if lhs.is_empty() || lhs.starts_with('#') {
            continue;
        }
        let rhs = t[eq + 1..].trim();
        // Forms:
        //   name = "1.0"             (registry dep — ignoriere)
        //   name = { path = "..." }  (workspace path)
        //   name = { workspace = true }  (workspace inheritance — ignoriere)
        if rhs.contains("path =") || rhs.contains("path=") {
            // Cargo-Package-Name = LHS (kein Rename-Support hier;
            // unsere Crates haben keine Renames).
            // Falls LHS quoted (selten in Cargo): strippen.
            let name = lhs.trim_matches('"').to_string();
            // Ggf. `package = "..."`-Override im inline-Table.
            let actual = extract_package_override(rhs).unwrap_or(name);
            out.insert(actual);
        }
    }
    out
}

fn extract_package_override(rhs: &str) -> Option<String> {
    let key = "package =";
    let alt = "package=";
    let needle_pos = rhs.find(key).or_else(|| rhs.find(alt))?;
    let after = &rhs[needle_pos..];
    let q = after.find('"')?;
    let rest = &after[q + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn is_dep_section(section: &str) -> bool {
    section == "[dependencies]" || section == "[build-dependencies]"
}

fn strip_comment(line: &str) -> &str {
    // Naive: erste `#` ausserhalb von Strings.
    let mut in_string = false;
    for (i, c) in line.char_indices() {
        if c == '"' {
            in_string = !in_string;
        }
        if c == '#' && !in_string {
            return &line[..i];
        }
    }
    line
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;

    #[test]
    fn parses_members_block() {
        let toml = r#"
[workspace]
members = [
    "crates/a",
    "crates/b",
]
"#;
        let m = parse_members(toml).unwrap();
        assert_eq!(m, vec!["crates/a", "crates/b"]);
    }

    #[test]
    fn parses_members_inline() {
        let toml = r#"members = ["a", "b", "c"]"#;
        let m = parse_members(toml).unwrap();
        assert_eq!(m, vec!["a", "b", "c"]);
    }

    #[test]
    fn parses_package_name() {
        let toml = r#"
[package]
name = "dds-foo"
version = "1"
"#;
        assert_eq!(parse_package_name(toml).as_deref(), Some("dds-foo"));
    }

    #[test]
    fn detects_publish_false() {
        let toml = r#"
[package]
name = "x"
publish = false
"#;
        assert!(parse_publish_blocked(toml));
    }

    #[test]
    fn parses_path_deps_detects_workspace_paths() {
        let toml = r#"
[package]
name = "x"

[dependencies]
zerodds-foundation = { path = "../foundation" }
serde = "1.0"
zerodds-cdr = { path = "../cdr", default-features = false }

[build-dependencies]
cbindgen = "0.29"
"#;
        let d = parse_path_deps(toml);
        assert!(d.contains("zerodds-foundation"));
        assert!(d.contains("zerodds-cdr"));
        assert!(!d.contains("serde"));
        assert!(!d.contains("cbindgen"));
    }

    #[test]
    fn topo_sort_produces_dep_first_order() {
        let crates = vec![
            CrateInfo {
                name: "a".into(),
                path: "a".into(),
                publish_blocked: false,
                deps: BTreeSet::new(),
            },
            CrateInfo {
                name: "b".into(),
                path: "b".into(),
                publish_blocked: false,
                deps: ["a".into()].into_iter().collect(),
            },
            CrateInfo {
                name: "c".into(),
                path: "c".into(),
                publish_blocked: false,
                deps: ["a".into(), "b".into()].into_iter().collect(),
            },
        ];
        let order = topo_sort(&crates).unwrap();
        let names: Vec<_> = order.iter().map(|c| c.name.clone()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn topo_sort_detects_cycle() {
        let crates = vec![
            CrateInfo {
                name: "a".into(),
                path: "a".into(),
                publish_blocked: false,
                deps: ["b".into()].into_iter().collect(),
            },
            CrateInfo {
                name: "b".into(),
                path: "b".into(),
                publish_blocked: false,
                deps: ["a".into()].into_iter().collect(),
            },
        ];
        assert!(matches!(topo_sort(&crates), Err(DagError::Cycle(_))));
    }

    #[test]
    fn only_publishable_filters_out_blocked() {
        let crates = vec![
            CrateInfo {
                name: "a".into(),
                path: "a".into(),
                publish_blocked: false,
                deps: BTreeSet::new(),
            },
            CrateInfo {
                name: "b".into(),
                path: "b".into(),
                publish_blocked: true,
                deps: BTreeSet::new(),
            },
        ];
        let pub_ = only_publishable(&crates);
        assert_eq!(pub_.len(), 1);
        assert_eq!(pub_[0].name, "a");
    }

    #[test]
    fn extract_package_override_works() {
        assert_eq!(
            extract_package_override(r#"{ path = "../x", package = "foo-pkg" }"#),
            Some("foo-pkg".into())
        );
        assert_eq!(extract_package_override(r#"{ path = "../x" }"#), None);
    }

    #[test]
    fn collect_crates_e2e() {
        let tmp = tempdir();
        std::fs::write(
            tmp.join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/a", "crates/b"]
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.join("crates/a")).unwrap();
        std::fs::create_dir_all(tmp.join("crates/b")).unwrap();
        std::fs::write(
            tmp.join("crates/a/Cargo.toml"),
            r#"
[package]
name = "a"
[dependencies]
"#,
        )
        .unwrap();
        std::fs::write(
            tmp.join("crates/b/Cargo.toml"),
            r#"
[package]
name = "b"
[dependencies]
a = { path = "../a" }
"#,
        )
        .unwrap();
        let crates = collect_crates(&tmp).unwrap();
        let order = topo_sort(&crates).unwrap();
        let names: Vec<_> = order.iter().map(|c| c.name.clone()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zd-cargo-dag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
