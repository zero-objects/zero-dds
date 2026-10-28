// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The publish DAG must cover the whole workspace.
//!
//! These tests run against this repository, not a fixture. They exist because
//! the DAG was once built from the `workspace.members` list in the root
//! `Cargo.toml`, which is not the workspace: Cargo makes every path
//! dependency of a listed member a member too. Eighteen publishable crates —
//! the transport, security, recorder and SQL-filter families among them —
//! were therefore absent from the publish order that
//! `scripts/publish-crates.sh` drives, so a publish run would have skipped
//! dependencies and uploaded their dependents first.
//!
//! The oracle here is deliberately not the library's own reader: the member
//! set comes from the flat `workspace_members` id list of `cargo metadata`,
//! and the publish flag from the member's `Cargo.toml` text. If the DAG ever
//! loses a package again — by going back to a hand-written list, or by a
//! filter that drops one — the first test fails and names it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_cargo_dag::{CrateInfo, collect_crates, only_publishable, topo_sort};

/// The repository root: two levels above `tools/cargo-dag`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// The publish order the release script consumes.
fn publish_order() -> Vec<CrateInfo> {
    let crates = collect_crates(&workspace_root())
        .unwrap_or_else(|e| panic!("collect_crates on the workspace: {e}"));
    let ordered = topo_sort(&crates).unwrap_or_else(|e| panic!("topo_sort on the workspace: {e}"));
    only_publishable(&ordered)
}

/// Every workspace crate, in dependency-first order.
fn full_order() -> Vec<CrateInfo> {
    let crates = collect_crates(&workspace_root())
        .unwrap_or_else(|e| panic!("collect_crates on the workspace: {e}"));
    topo_sort(&crates).unwrap_or_else(|e| panic!("topo_sort on the workspace: {e}"))
}

/// Independent oracle: the manifest directory of every workspace member,
/// read out of the `workspace_members` id list of `cargo metadata`.
///
/// A package id spells the manifest directory between `file://` and `#` in
/// every id format Cargo has emitted, which is all this needs.
fn member_manifest_dirs() -> Vec<PathBuf> {
    let root = workspace_root();
    let out = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).expect("metadata is utf-8");
    let start = text
        .find("\"workspace_members\":")
        .expect("metadata has workspace_members");
    let list_start = text[start..]
        .find('[')
        .expect("workspace_members is a list")
        + start;
    let list_end = text[list_start..]
        .find(']')
        .expect("workspace_members list ends")
        + list_start;
    let ids: Vec<&str> = text[list_start + 1..list_end]
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    assert!(!ids.is_empty(), "workspace_members is empty");
    ids.iter()
        .map(|id| {
            let scheme = id.find("file://").expect("package id names a file:// path");
            let rest = &id[scheme + "file://".len()..];
            let dir = rest.split('#').next().unwrap_or(rest);
            PathBuf::from(dir)
        })
        .collect()
}

/// Independent oracle: package name and publish flag, read from the
/// member's `Cargo.toml` text.
fn manifest_name_and_publish(dir: &Path) -> (String, bool) {
    let manifest = dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
    let mut in_package = false;
    let mut name = None;
    let mut blocked = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or(raw).trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && name.is_none() && line.starts_with("name") {
            name = line
                .split('=')
                .nth(1)
                .map(|v| v.trim().trim_matches('"').to_string());
        } else if in_package && line.starts_with("publish") {
            let value = line.split('=').nth(1).unwrap_or("").trim();
            blocked = value == "false" || value == "[]";
        }
    }
    (
        name.unwrap_or_else(|| panic!("no [package] name in {}", manifest.display())),
        blocked,
    )
}

#[test]
fn publish_dag_contains_every_publishable_workspace_package() {
    let expected: BTreeSet<String> = member_manifest_dirs()
        .iter()
        .map(|d| manifest_name_and_publish(d))
        .filter(|(_, blocked)| !blocked)
        .map(|(name, _)| name)
        .collect();
    let actual: BTreeSet<String> = publish_order().iter().map(|c| c.name.clone()).collect();

    let missing: Vec<&String> = expected.difference(&actual).collect();
    let extra: Vec<&String> = actual.difference(&expected).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "publish DAG does not match the publishable workspace packages\n\
         missing from the DAG ({}): {missing:?}\n\
         in the DAG but not publishable ({}): {extra:?}",
        missing.len(),
        extra.len()
    );
    assert_eq!(expected.len(), actual.len());
}

/// Every member Cargo would add because a listed member depends on it by
/// path is listed itself (`ci/test_dependency_graph.py` holds every tracked
/// manifest named). Before, the publish DAG was checked here to contain such
/// implicit members; with none left, the check is that none comes back
/// unlisted, and `publish_dag_contains_every_publishable_workspace_package` covers the
/// DAG for all of them.
#[test]
fn every_member_cargo_adds_by_path_is_also_listed() {
    let root = workspace_root();
    let listed: BTreeSet<PathBuf> = listed_member_paths(&root);
    let implicit: Vec<String> = member_manifest_dirs()
        .iter()
        .filter(|dir| {
            dir.strip_prefix(&root)
                .map(|rel| !listed.contains(rel))
                .unwrap_or(false)
        })
        .map(|dir| dir.display().to_string())
        .collect();
    assert!(
        implicit.is_empty(),
        "workspace members not named in `members`: {implicit:?}"
    );
}

/// The paths spelled out in `workspace.members` of the root manifest.
fn listed_member_paths(root: &Path) -> BTreeSet<PathBuf> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).expect("read root Cargo.toml");
    let start = text.find("members").expect("root manifest lists members");
    let open = text[start..].find('[').expect("members is a list") + start;
    let close = text[open..].find(']').expect("members list ends") + open;
    let mut out = BTreeSet::new();
    let mut rest = &text[open + 1..close];
    while let Some(q) = rest.find('"') {
        rest = &rest[q + 1..];
        let Some(end) = rest.find('"') else { break };
        let candidate = PathBuf::from(&rest[..end]);
        // The block is read as text, so quoted words inside its comments end
        // up here too. Keeping only entries that name a real manifest drops
        // them; a listed member misread as unlisted would only widen the
        // implicit set, never narrow it.
        if root.join(&candidate).join("Cargo.toml").is_file() {
            out.insert(candidate);
        }
        rest = &rest[end + 1..];
    }
    assert!(!out.is_empty(), "no members parsed from the root manifest");
    out
}

#[test]
fn every_crate_comes_after_its_dependencies() {
    let order = full_order();
    let names: Vec<&str> = order.iter().map(|c| c.name.as_str()).collect();
    let position = |name: &str| names.iter().position(|n| *n == name);
    for (i, c) in order.iter().enumerate() {
        for dep in &c.deps {
            // Deps outside the workspace impose no order.
            let Some(j) = position(dep) else { continue };
            assert!(
                j < i,
                "`{}` is at {i} but its dependency `{dep}` is at {j}",
                c.name
            );
        }
    }
    // The same must hold after the non-publishable crates are filtered out,
    // because that filtered list is what the publish loop walks.
    let published = only_publishable(&order);
    let pub_names: Vec<&str> = published.iter().map(|c| c.name.as_str()).collect();
    for (i, c) in published.iter().enumerate() {
        for dep in &c.deps {
            let Some(j) = pub_names.iter().position(|n| *n == dep) else {
                continue;
            };
            assert!(
                j < i,
                "publish order: `{}` is at {i} but its dependency `{dep}` is at {j}",
                c.name
            );
        }
    }
}
