// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-async-1.0 "Ziele" / "Nicht-Ziele" / "Module-Layout" — shape of
//! the crate as a build artefact.
//!
//! Three clauses are properties of the manifest rather than of any code
//! path: the non-goal "Eigene async-Runtime einbauen" (the crate ships
//! none), the goal "Optional Tokio-Glue als feature-gated Convenience"
//! (tokio is optional, not a hard dependency) and the module layout's
//! `default = ["std"], optional "tokio-glue"`.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

const MANIFEST: &str = include_str!("../Cargo.toml");

fn default_features() -> String {
    MANIFEST
        .lines()
        .find(|l| l.trim_start().starts_with("default ="))
        .expect("[features] default entry")
        .to_string()
}

/// Module layout — `default = ["std"]`, and `tokio-glue` is not part of it.
#[test]
fn default_feature_set_is_std_only() {
    let defaults = default_features();
    assert!(defaults.contains("std"), "{defaults}");
    assert!(
        !defaults.contains("tokio"),
        "tokio-glue must not be a default feature: {defaults}"
    );
}

/// Non-goal "Eigene async-Runtime einbauen" plus the goal "Optional
/// Tokio-Glue": tokio is an optional dependency pulled in by the
/// `tokio-glue` feature, and no other runtime crate is a dependency at
/// all. `futures-core` is a trait crate, not a runtime.
#[test]
fn no_async_runtime_is_a_hard_dependency() {
    let deps = MANIFEST
        .split("[dependencies]")
        .nth(1)
        .expect("[dependencies] section")
        .split("\n[")
        .next()
        .expect("section body");
    assert!(
        deps.contains("tokio = { version = \"1\", optional = true"),
        "tokio must be optional: {deps}"
    );
    assert!(
        MANIFEST.contains("tokio-glue = [\"dep:tokio\"]"),
        "the tokio dependency must hang off the tokio-glue feature"
    );
    for runtime in ["async-std", "smol", "async-global-executor", "embassy"] {
        assert!(
            !deps.contains(runtime),
            "no runtime crate may be a dependency, found {runtime}"
        );
    }
}
