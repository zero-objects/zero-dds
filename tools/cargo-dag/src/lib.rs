// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Workspace crate DAG sort for cargo publish.
//!
//! Crate `zerodds-cargo-dag`. Safety classification: **COMFORT**.
//!
//! # What does the tool do?
//!
//! Runs `cargo metadata --format-version 1 --no-deps` over the workspace and
//! builds the inter-crate dependency graph from its `packages` array.
//! Topological sorting gives a cycle-free order — that is the
//! `cargo publish` order.
//!
//! Dev-dependencies are ignored (irrelevant for publish).
//! Build-dependencies are counted in (e.g. `cbindgen` callers).
//!
//! # Why `cargo metadata` and not `workspace.members`?
//!
//! The member list in the root `Cargo.toml` is not the workspace. Cargo adds
//! every path dependency of a listed member as a member too, and the root
//! manifest's own comments say so ("Adding this one line also makes members
//! of its path dependencies: …"). Reading only the listed names dropped those
//! implicit members from the publish order — the transport, security,
//! recorder and SQL-filter crates among them — so a publish run would have
//! skipped dependencies and uploaded their dependents first. `cargo metadata`
//! reports the members Cargo actually resolves, so a newly added path
//! dependency enters the publish order without anyone editing a list.
//!
//! # What is this not?
//!
//! Not a full Cargo resolver — only workspace-internal `path` dependencies
//! become edges. Registry dependencies are irrelevant to the publish order
//! and are ignored.

#![warn(missing_docs)]

mod json;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use json::Value;

/// Describes a workspace crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrateInfo {
    /// Cargo package name (`name = "..."`).
    pub name: String,
    /// Member path relative to the workspace root.
    pub path: PathBuf,
    /// `publish` set to something other than crates.io?
    /// (default: publishable).
    pub publish_blocked: bool,
    /// Set of workspace-internal path deps (Cargo package names).
    pub deps: BTreeSet<String>,
}

/// Read error.
#[derive(Debug)]
pub enum DagError {
    /// `cargo metadata` could not be started.
    Spawn {
        /// The program that failed to start.
        program: OsString,
        /// Underlying error.
        err: std::io::Error,
    },
    /// `cargo metadata` ran but exited non-zero.
    Metadata {
        /// Exit code, if the process was not killed by a signal.
        status: Option<i32>,
        /// What the process wrote to stderr.
        stderr: String,
    },
    /// The metadata document could not be read as expected.
    Json(String),
    /// Cycle in the DAG.
    Cycle(Vec<String>),
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn { program, err } => {
                write!(f, "cannot run {}: {err}", program.to_string_lossy())
            }
            Self::Metadata { status, stderr } => match status {
                Some(code) => write!(f, "cargo metadata exited with {code}: {stderr}"),
                None => write!(f, "cargo metadata was terminated by a signal: {stderr}"),
            },
            Self::Json(msg) => write!(f, "cargo metadata output: {msg}"),
            Self::Cycle(c) => write!(f, "cycle detected: {}", c.join(" → ")),
        }
    }
}

impl std::error::Error for DagError {}

/// Collects all workspace crates with inter-crate deps.
///
/// The member set comes from `cargo metadata --no-deps`, so it contains the
/// implicit members (path dependencies of listed members) as well as the
/// ones named in `workspace.members`.
///
/// # Errors
/// Spawn/Metadata/Json error.
pub fn collect_crates(workspace_root: &Path) -> Result<Vec<CrateInfo>, DagError> {
    let document = run_cargo_metadata(workspace_root)?;
    parse_metadata(&document)
}

/// Runs `cargo metadata --format-version 1 --no-deps` in `workspace_root`.
fn run_cargo_metadata(workspace_root: &Path) -> Result<String, DagError> {
    // `CARGO` is set when we run under cargo; otherwise fall back to PATH.
    let program = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(&program)
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(workspace_root.join("Cargo.toml"))
        .output()
        .map_err(|err| DagError::Spawn {
            program: program.clone(),
            err,
        })?;
    if !output.status.success() {
        return Err(DagError::Metadata {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    String::from_utf8(output.stdout)
        .map_err(|e| DagError::Json(format!("stdout is not utf-8: {e}")))
}

/// Turns a `cargo metadata` document into the crate list.
fn parse_metadata(document: &str) -> Result<Vec<CrateInfo>, DagError> {
    let root = json::parse(document).map_err(DagError::Json)?;
    let workspace_root = root
        .get("workspace_root")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let packages = root
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| DagError::Json("no `packages` array".to_string()))?;
    let mut out = Vec::with_capacity(packages.len());
    for package in packages {
        out.push(crate_from_package(package, workspace_root.as_deref())?);
    }
    Ok(out)
}

/// Reads one `packages[]` entry.
fn crate_from_package(
    package: &Value,
    workspace_root: Option<&Path>,
) -> Result<CrateInfo, DagError> {
    let name = package
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| DagError::Json("package without `name`".to_string()))?
        .to_string();
    let manifest_path = package
        .get("manifest_path")
        .and_then(Value::as_str)
        .ok_or_else(|| DagError::Json(format!("package `{name}` without `manifest_path`")))?;
    Ok(CrateInfo {
        name,
        path: member_path(Path::new(manifest_path), workspace_root),
        publish_blocked: publish_blocked(package),
        deps: publish_relevant_path_deps(package),
    })
}

/// Directory of `manifest_path`, relative to the workspace root when it lies
/// beneath it (a path dependency outside the tree keeps its absolute path).
fn member_path(manifest_path: &Path, workspace_root: Option<&Path>) -> PathBuf {
    let dir = manifest_path.parent().unwrap_or(manifest_path);
    workspace_root
        .and_then(|root| dir.strip_prefix(root).ok())
        .map_or_else(|| dir.to_path_buf(), Path::to_path_buf)
}

/// Is this package kept out of a crates.io publish run?
///
/// `cargo metadata` reports the manifest's `publish` key as `null` (any
/// registry), or as the list of allowed registries — `publish = false`
/// serialises as the empty list. A package is in the publish run exactly
/// when its list of allowed registries is absent or names crates.io.
fn publish_blocked(package: &Value) -> bool {
    match package.get("publish") {
        None => false,
        Some(value) if value.is_null() => false,
        Some(value) => match value.as_array() {
            Some(registries) => !registries
                .iter()
                .any(|r| r.as_str() == Some(CRATES_IO_REGISTRY)),
            // Anything else is not a registry list we can honour.
            None => true,
        },
    }
}

/// The registry name Cargo uses for crates.io in a manifest `publish` list.
const CRATES_IO_REGISTRY: &str = "crates-io";

/// Workspace-internal dependencies that constrain the publish order:
/// every `dependencies[]` entry with a `path`, except dev-dependencies.
fn publish_relevant_path_deps(package: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(deps) = package.get("dependencies").and_then(Value::as_array) else {
        return out;
    };
    for dep in deps {
        // `kind` is null for a normal dep, "build" or "dev" otherwise.
        // Dev-deps are not part of the published manifest's resolution.
        if dep.get("kind").and_then(Value::as_str) == Some("dev") {
            continue;
        }
        // Only path deps are workspace-internal; a registry dep is already
        // published. `name` is the package name even under a rename.
        if dep.get("path").is_none_or(Value::is_null) {
            continue;
        }
        if let Some(name) = dep.get("name").and_then(Value::as_str) {
            out.insert(name.to_string());
        }
    }
    out
}

/// Topologically sorted — Kahn's algorithm with deterministic
/// Tie-break over crate names (alphabetical).
///
/// Returns an order such that each crate appears before its dependents
/// appears. `publish_blocked` crates are included but
/// marked; the caller filters them out if needed.
///
/// # Errors
/// `Cycle` if the graph has a cycle.
pub fn topo_sort(crates: &[CrateInfo]) -> Result<Vec<CrateInfo>, DagError> {
    // Map name → info index, so that we can work economically with u32
    // in HashSets.
    let by_name: BTreeMap<&str, usize> = crates
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.as_str(), i))
        .collect();
    // Per-crate indegree: how many workspace deps we have.
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
    // Kahn — priority queue over crate names, for determinism.
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
        // The rest are in the cycle.
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

/// Filters out the crates a publish run does not upload.
#[must_use]
pub fn only_publishable(crates: &[CrateInfo]) -> Vec<CrateInfo> {
    crates
        .iter()
        .filter(|c| !c.publish_blocked)
        .cloned()
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    /// One `packages[]` entry in the spelling `cargo metadata` emits.
    fn package(body: &str) -> Value {
        json::parse(body).unwrap()
    }

    #[test]
    fn reads_name_and_relative_member_path() {
        let p = package(
            r#"{"name":"zerodds-transport",
                "manifest_path":"/ws/crates/transport/Cargo.toml",
                "publish":null,"dependencies":[]}"#,
        );
        let c = crate_from_package(&p, Some(Path::new("/ws"))).unwrap();
        assert_eq!(c.name, "zerodds-transport");
        assert_eq!(c.path, PathBuf::from("crates/transport"));
        assert!(!c.publish_blocked);
    }

    #[test]
    fn keeps_absolute_path_for_a_member_outside_the_workspace_root() {
        let p = package(
            r#"{"name":"x","manifest_path":"/elsewhere/x/Cargo.toml",
                "publish":null,"dependencies":[]}"#,
        );
        let c = crate_from_package(&p, Some(Path::new("/ws"))).unwrap();
        assert_eq!(c.path, PathBuf::from("/elsewhere/x"));
    }

    #[test]
    fn publish_null_is_publishable_and_empty_list_is_blocked() {
        assert!(!publish_blocked(&package(r#"{"publish":null}"#)));
        assert!(!publish_blocked(&package("{}")));
        assert!(publish_blocked(&package(r#"{"publish":[]}"#)));
    }

    #[test]
    fn publish_list_is_honoured_per_registry() {
        assert!(!publish_blocked(&package(r#"{"publish":["crates-io"]}"#)));
        assert!(publish_blocked(&package(
            r#"{"publish":["house-registry"]}"#
        )));
        assert!(!publish_blocked(&package(
            r#"{"publish":["house-registry","crates-io"]}"#
        )));
    }

    #[test]
    fn path_deps_are_edges_and_registry_deps_are_not() {
        let p = package(
            r#"{"dependencies":[
                {"name":"zerodds-foundation","kind":null,"path":"/ws/crates/foundation"},
                {"name":"serde","kind":null,"path":null},
                {"name":"cbindgen","kind":"build","path":null},
                {"name":"zerodds-build","kind":"build","path":"/ws/crates/zerodds-build"}]}"#,
        );
        let d = publish_relevant_path_deps(&p);
        assert_eq!(
            d,
            [
                "zerodds-foundation".to_string(),
                "zerodds-build".to_string()
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn dev_dependencies_are_not_edges() {
        let p = package(
            r#"{"dependencies":[
                {"name":"zerodds-test-domains","kind":"dev","path":"/ws/crates/test-domains"}]}"#,
        );
        assert!(publish_relevant_path_deps(&p).is_empty());
    }

    #[test]
    fn a_renamed_path_dep_is_an_edge_under_its_package_name() {
        let p = package(
            r#"{"dependencies":[
                {"name":"zerodds-wire","rename":"wire","kind":null,"path":"/ws/crates/wire"}]}"#,
        );
        let d = publish_relevant_path_deps(&p);
        assert!(d.contains("zerodds-wire"));
        assert!(!d.contains("wire"));
    }

    #[test]
    fn parse_metadata_reads_every_package_in_the_document() {
        let doc = r#"{"workspace_root":"/ws","packages":[
            {"name":"a","manifest_path":"/ws/a/Cargo.toml","publish":null,"dependencies":[]},
            {"name":"b","manifest_path":"/ws/b/Cargo.toml","publish":[],
             "dependencies":[{"name":"a","kind":null,"path":"/ws/a"}]}]}"#;
        let crates = parse_metadata(doc).unwrap();
        assert_eq!(crates.len(), 2);
        assert_eq!(crates[1].name, "b");
        assert!(crates[1].publish_blocked);
        assert!(crates[1].deps.contains("a"));
    }

    #[test]
    fn parse_metadata_rejects_a_document_without_packages() {
        assert!(matches!(
            parse_metadata(r#"{"workspace_root":"/ws"}"#),
            Err(DagError::Json(_))
        ));
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
    fn collect_crates_e2e() {
        // A real `cargo metadata` run over a throwaway workspace whose second
        // member is reachable only as a path dependency of the first — the
        // implicit membership that the old `workspace.members` reader missed.
        let tmp = tempdir();
        std::fs::write(
            tmp.join("Cargo.toml"),
            r#"
[workspace]
resolver = "2"
members = ["crates/b"]
"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.join("crates/a/src")).unwrap();
        std::fs::create_dir_all(tmp.join("crates/b/src")).unwrap();
        std::fs::write(tmp.join("crates/a/src/lib.rs"), "").unwrap();
        std::fs::write(tmp.join("crates/b/src/lib.rs"), "").unwrap();
        std::fs::write(
            tmp.join("crates/a/Cargo.toml"),
            r#"
[package]
name = "a"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        )
        .unwrap();
        std::fs::write(
            tmp.join("crates/b/Cargo.toml"),
            r#"
[package]
name = "b"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
a = { path = "../a" }
"#,
        )
        .unwrap();
        let crates = collect_crates(&tmp).unwrap();
        let order = topo_sort(&crates).unwrap();
        let names: Vec<_> = order.iter().map(|c| c.name.clone()).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(
            only_publishable(&order)
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zd-cargo-dag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
