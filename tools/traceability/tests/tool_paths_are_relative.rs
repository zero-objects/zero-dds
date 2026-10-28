//! Repository tooling must locate the tree from its own file, never from an
//! absolute path baked into the source.
//!
//! A script that carries `Path("/Users/<somebody>/projects/zerodds")` reads
//! and writes the *main* checkout no matter which worktree invoked it. From an
//! agent worktree that is a silent cross-tree write: the run reports success,
//! the files change somewhere the caller is not looking, and the worktree it
//! was supposed to render stays untouched. The failure has no error message,
//! which is what makes it worth a gate rather than a convention.
//!
//! The correct form is `Path(__file__).resolve().parents[N]`, with `N` the
//! number of directories between the script and the tree it means.
//!
//! The same gate for the `website/_tools` scripts, and the check that they
//! name the right ancestor, is `website_tool_paths.rs`, which the public
//! mirror leaves out with `website/`.
//!
//! Paths are resolved from the repository root, two levels above this
//! crate's manifest (`tools/traceability`).

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

mod tool_paths;

use tool_paths::{absolute_paths, python_files, repo_root};

/// No script under `tools/` may name the checkout by an absolute developer
/// path.
///
/// The offence is a string literal that begins with a home-directory prefix
/// and names this repository — `"/Users/…/projects/zerodds"`.
#[test]
fn no_tool_hardcodes_an_absolute_repository_path() {
    let root = repo_root();
    let mut files = Vec::new();
    python_files(&root.join("tools"), &mut files);
    assert!(
        files.len() > 20,
        "expected tools/ to hold python scripts, found {}",
        files.len()
    );
    let offenders = absolute_paths(&root, &files);
    assert!(
        offenders.is_empty(),
        "tools must resolve the repository from `Path(__file__).resolve().parents[N]`, \
         not from an absolute path; found {} occurrence(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}
