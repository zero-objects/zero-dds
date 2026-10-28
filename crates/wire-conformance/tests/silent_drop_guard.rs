//! The silent-drop guard runs under `cargo test`.
//!
//! # Why this test exists
//!
//! `tools/silent-drop-guard.sh` was red for thirty commits and nothing
//! acted on it. The reason was not that it ran nowhere. It was wired
//! into three places and one of them really ran it: the GitLab
//! `silent-drop` job failed on every pipeline from `f863ceb46` onwards.
//! It failed inside a pipeline where forty of sixty jobs were failing,
//! so one more red job carried no signal. The other two wirings were
//! nominal — `scripts/pre-commit.sh` is not installed as a git hook and
//! nothing in the tree installs it, and `.github/workflows/ci.yml` is
//! not the file GitHub runs, because the root repo's only remote is
//! GitLab and the public mirror is a separate checkout whose workflow
//! has no such job.
//!
//! A guard is only a guard where it is read. `cargo test` is the chain
//! a developer runs before anything else and the one every CI job that
//! tests this workspace already runs, so a drift shows up next to the
//! change that caused it rather than in a pipeline nobody opens.
//!
//! The allowlist is what makes this cheap: a new site fails here with
//! the site printed, and `bash tools/silent-drop-guard.sh --update`
//! plus one written sentence is the whole answer.
//!
//! The workspace's no-panic contract is relaxed here as it is in
//! `bytes.rs`: a test that swallowed a failure to run the guard would
//! report success having measured nothing, which is the exact shape the
//! guard exists to catch.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::Command;

/// The workspace root: this crate is `crates/wire-conformance`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("crates/wire-conformance sits two levels below the workspace root")
        .to_path_buf()
}

/// Every silent-drop site is in the allowlist with a reason.
///
/// Unix only: the guard is a bash script and the Windows job builds
/// rather than lints. It is skipped loudly rather than silently — a
/// test that reports success on a platform where it measured nothing is
/// the shape this guard exists to catch.
#[test]
fn every_silent_drop_site_is_accounted_for() {
    let root = repo_root();
    let script = root.join("tools/silent-drop-guard.sh");
    assert!(
        script.is_file(),
        "tools/silent-drop-guard.sh is missing at {}",
        script.display()
    );

    if !cfg!(unix) {
        eprintln!(
            "SKIPPED: the silent-drop guard is a bash script and this is not a unix host. \
             It runs in scripts/pre-commit.sh and the GitLab `silent-drop` job."
        );
        return;
    }

    let output = Command::new("bash")
        .arg(&script)
        .current_dir(&root)
        .output()
        .expect("running bash tools/silent-drop-guard.sh");

    if !output.status.success() {
        panic!(
            "tools/silent-drop-guard.sh failed.\n\
             A NEW line is a site nobody judged: read the function it sits in and decide \
             whether a value can go missing. Fix it, or run \
             `bash tools/silent-drop-guard.sh --update` and write down why it cannot.\n\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
