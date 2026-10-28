// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The result tree: case, then system, then encoding and byte order.
//!
//! ```text
//! tests/interop-results/<case>/<system>/<encoding>.<endian>.bin
//! tests/interop-results/<case>/<system>/read.tsv
//! tests/interop-results/<case>/<system>/STATUS
//! ```
//!
//! The `.bin` holds the octets the system produced, so that a reader
//! can hand one straight back to a probe with `--in`. The hex of the
//! same octets is in `ledger.tsv` and in the source TSV the system's
//! own harness wrote, which is what keeps the tree diffable without
//! making it text.
//!
//! `read.tsv` holds one line per (encoding, byte order) of what came
//! back when that system read those bytes, and `STATUS` says why a
//! system produced nothing at all for a case. **Neither is ever
//! absent-meaning-fine**: a directory with no file in it would read as
//! agreement.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::ledger::{Entry, Ledger, Outcome};

/// What one write of the tree changed.
#[derive(Debug, Default)]
pub struct Written {
    /// How many files the tree holds after this run.
    pub files: usize,
    /// How many bytes of content those files hold.
    pub bytes: u64,
    /// Paths that were there and are not in this run's tree, which is
    /// what a case or a system being removed looks like.
    pub stale: Vec<PathBuf>,
}

/// Where a cell's bytes live, relative to the results root.
#[must_use]
pub fn bin_path(entry: &Entry) -> String {
    format!(
        "{}/{}/{}.{}.bin",
        entry.case, entry.system, entry.encoding, entry.endian
    )
}

/// Where a cell's read record lives, relative to the results root.
#[must_use]
pub fn read_path(case: &str, system: &str) -> String {
    format!("{case}/{system}/read.tsv")
}

/// Where a (case, system) pair's explanation lives.
#[must_use]
pub fn status_path(case: &str, system: &str) -> String {
    format!("{case}/{system}/STATUS")
}

/// Where what a (case, system) pair's front end *said* lives.
///
/// Beside `STATUS` and not inside it: `STATUS` answers why a slot
/// carries no bytes, and a warning is most often attached to a slot
/// that carries them.
#[must_use]
pub fn warnings_path(case: &str, system: &str) -> String {
    format!("{case}/{system}/WARNINGS")
}

fn unhex(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err(format!("`{hex}` has an odd number of hex digits"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|error| format!("`{hex}`: {error}"))
        })
        .collect()
}

/// Render the whole tree under `root`.
///
/// Every file is written; nothing is left from an earlier run that
/// this one did not produce. A stale `.bin` is a byte string no
/// harness stands behind any more, and it would still link from a
/// table that no longer mentions it.
///
/// # Errors
///
/// When a file cannot be written, or a recorded hex string is not hex.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `std::fs::write`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so the call reads as a self-call.
pub fn write(root: &Path, ledger: &Ledger) -> Result<Written, String> {
    let mut written = Written::default();
    let wanted = files(root, ledger)?;

    for (path, bytes) in &wanted {
        let parent = path
            .parent()
            .ok_or_else(|| format!("{}: no directory to write into", path.display()))?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
        let current = std::fs::read(path).ok();
        if current.as_deref() != Some(bytes.as_slice()) {
            std::fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))?;
        }
        written.files += 1;
        written.bytes += bytes.len() as u64;
    }

    written.stale = stale(root, &wanted)?;
    for path in &written.stale {
        std::fs::remove_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    prune_empty(root)?;
    Ok(written)
}

/// Every file the tree under `root` holds for `ledger`, with its content:
/// the payloads, `read.tsv`, `STATUS` and `WARNINGS`.
///
/// One function for [`write`] and [`verify`], so the check compares
/// exactly what a regeneration would leave on disk. It compared the
/// payloads alone, and a `WARNINGS` file whose line had left
/// `sources/toolchain-warnings.tsv`, or a `STATUS` the planner no longer
/// stood behind, passed `--verify` and was deleted or rewritten by the
/// next `--regenerate`.
///
/// # Errors
///
/// When a recorded hex string is not hex.
fn files(root: &Path, ledger: &Ledger) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mut wanted: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();

    // read.tsv and STATUS are per (case, system); the payloads are per
    // cell. Collected first so that one pass writes each file once.
    let mut reads: BTreeMap<(String, &'static str), String> = BTreeMap::new();
    let mut statuses: BTreeMap<(String, &'static str), Vec<String>> = BTreeMap::new();
    let mut wrote_bytes: BTreeMap<(String, &'static str), bool> = BTreeMap::new();

    for entry in &ledger.entries {
        let pair = (entry.case.clone(), entry.system);
        if let Outcome::Bytes(hex) = &entry.write {
            let bytes = unhex(hex)?;
            wanted.insert(root.join(bin_path(entry)), bytes);
            wrote_bytes.insert(pair.clone(), true);
        } else {
            wrote_bytes.entry(pair.clone()).or_insert(false);
            statuses.entry(pair.clone()).or_default().push(format!(
                "{}\t{}\t{}\t{}",
                entry.encoding,
                entry.endian,
                entry.write.token(),
                entry.write.detail()
            ));
        }
        let line = format!(
            "{}\t{}\t{}\t{}\n",
            entry.encoding,
            entry.endian,
            entry.read.token(),
            entry.read.detail()
        );
        reads.entry(pair).or_default().push_str(&line);
    }

    for ((case, system), body) in reads {
        let header = format!(
            "# What {system} got back when it read the bytes of {case}, held against\n\
             # tests/interop-corpus/{case}/values.json and never against another\n\
             # column's reading of the same bytes.\n\
             # encoding\tendian\tverdict\tdetail\n"
        );
        wanted.insert(
            root.join(read_path(&case, system)),
            format!("{header}{body}").into_bytes(),
        );
    }

    for ((case, system), lines) in statuses {
        let produced = wrote_bytes
            .get(&(case.clone(), system))
            .copied()
            .unwrap_or(false);
        let mut body = format!(
            "# The (encoding, byte order) slots of {case} that {system} produced no\n\
             # bytes for, and why. {}\n\
             # encoding\tendian\tstatus\tdetail\n",
            if produced {
                "The rest are the .bin files beside this one."
            } else {
                "There are no bytes from this system for this case at all."
            }
        );
        for line in lines {
            body.push_str(&line);
            body.push('\n');
        }
        wanted.insert(root.join(status_path(&case, system)), body.into_bytes());
    }

    for (pair, lines) in &ledger.warnings {
        let (case, system) = match pair.split_once('/') {
            Some(split) => split,
            None => continue,
        };
        let mut body = format!(
            "# What {system}'s front end said about {case}. Not why a slot carries no\n\
             # bytes — that is STATUS beside this file — but what the front end printed\n\
             # while it did take the case. tests/interop-results/WARNINGS.md says what\n\
             # each of these means and which of them changes a verdict.\n\
             # program\twarning\n"
        );
        for line in lines {
            body.push_str(line);
            body.push('\n');
        }
        wanted.insert(root.join(warnings_path(case, system)), body.into_bytes());
    }
    Ok(wanted)
}

/// Every file under a case directory that this run did not produce.
fn stale(root: &Path, wanted: &BTreeMap<PathBuf, Vec<u8>>) -> Result<Vec<PathBuf>, String> {
    let mut stale = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(stale),
        Err(error) => return Err(format!("{}: {error}", root.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", root.display()))?;
        let path = entry.path();
        // The generated documents and the sources live beside the case
        // directories and are not part of the sweep.
        if !path.is_dir() || path.file_name().is_some_and(|name| name == "sources") {
            continue;
        }
        walk(&path, wanted, &mut stale, 0)?;
    }
    stale.sort();
    Ok(stale)
}

/// How far below a case directory the sweep descends.
///
/// The tree is `<case>/<system>/<file>`, so the sweep starting at a case
/// directory reaches depth one and nothing legitimate is near the cap.
/// It is here because the descent tests `Path::is_dir`, which follows
/// symbolic links: a link under the result tree pointing at one of its
/// own ancestors is an endless descent, and the tree is written by
/// scripts that run under `tests/interop-results/` on hosts whose disk
/// this program does not own.
const MAX_TREE_DEPTH: usize = 32;

/// Everything under `directory` that this run did not produce.
///
/// zerodds-lint: recursion-depth 32 (directory nesting; bounded by
/// [`MAX_TREE_DEPTH`], past which the descent stops with the directory
/// named rather than reporting its files as stale)
fn walk(
    directory: &Path,
    wanted: &BTreeMap<PathBuf, Vec<u8>>,
    stale: &mut Vec<PathBuf>,
    depth: usize,
) -> Result<(), String> {
    // Loud, and not a truncated sweep: everything this function did not
    // reach would be reported as *not stale*, and the caller deletes
    // from the complement of that list. A quiet return here is a tree
    // that keeps files no run stands behind.
    if depth > MAX_TREE_DEPTH {
        return Err(format!(
            "{}: directory nesting exceeds {MAX_TREE_DEPTH} — a symbolic link into an \
             ancestor of the result tree would descend forever",
            directory.display()
        ));
    }
    for entry in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        if path.is_dir() {
            walk(&path, wanted, stale, depth + 1)?;
        } else if !wanted.contains_key(&path) {
            stale.push(path);
        }
    }
    Ok(())
}

/// A system directory left empty by a case that no longer exists.
fn prune_empty(root: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    let mut directories = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.file_name().is_some_and(|name| name != "sources") {
            directories.push(path);
        }
    }
    for case in directories {
        for entry in std::fs::read_dir(&case).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir()
                && std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .next()
                    .is_none()
            {
                let _ = std::fs::remove_dir(&path);
            }
        }
        if std::fs::read_dir(&case)
            .into_iter()
            .flatten()
            .next()
            .is_none()
        {
            let _ = std::fs::remove_dir(&case);
        }
    }
    Ok(())
}

/// Compare a freshly built ledger against the tree on disk.
///
/// Returns one line per difference: a file the run produces and the
/// tree lacks or holds differently, and a file the tree holds that the
/// run does not produce. An empty answer is the whole claim of the CI
/// job: the tree committed is what `--regenerate` would leave.
///
/// # Errors
///
/// When a recorded hex string is not hex, or a file cannot be read.
pub fn verify(root: &Path, ledger: &Ledger) -> Result<Vec<String>, String> {
    let mut differences = Vec::new();
    let wanted = files(root, ledger)?;
    let shown = |path: &Path| {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    for (path, expected) in &wanted {
        match std::fs::read(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                differences.push(format!(
                    "{}: the run produces it and the tree has no file",
                    shown(path)
                ));
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
            Ok(found) if &found != expected => {
                differences.push(format!(
                    "{}: the tree holds {} bytes and the run produces {}",
                    shown(path),
                    found.len(),
                    expected.len()
                ));
            }
            Ok(_) => {}
        }
    }
    for path in stale(root, &wanted)? {
        differences.push(format!(
            "{}: the tree holds it and no run produces it",
            shown(&path)
        ));
    }
    Ok(differences)
}

/// One line per system, saying how much of the tree it accounts for.
#[must_use]
pub fn tally(ledger: &Ledger) -> String {
    let mut counts: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for entry in &ledger.entries {
        *counts
            .entry(entry.system)
            .or_default()
            .entry(entry.write.token())
            .or_default() += 1;
    }
    let mut out = String::new();
    for system in &crate::systems::SYSTEMS {
        let row = counts.get(system.id).cloned().unwrap_or_default();
        let bytes = row.get("bytes").copied().unwrap_or(0);
        let total: usize = row.values().sum();
        let _ = write!(
            out,
            "{:<14} {bytes:>5} of {total:>5} rows carry bytes",
            system.id
        );
        let mut rest: Vec<(&&str, &usize)> =
            row.iter().filter(|(token, _)| **token != "bytes").collect();
        rest.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        for (token, count) in rest {
            let _ = write!(out, "; {count} {token}");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// A tree nested past [`MAX_TREE_DEPTH`] is refused by name, not
    /// swept as far as the cap and reported as complete.
    ///
    /// The descent tests `Path::is_dir`, which follows symbolic links,
    /// so an unbounded descent is reachable from a link and not only
    /// from a genuinely deep tree. Plain directories are what the test
    /// builds, because they need no privilege on any host.
    #[test]
    fn refuses_a_tree_nested_past_the_cap() {
        let root = std::env::temp_dir().join("zerodds-interop-results-walk-depth");
        let _ = std::fs::remove_dir_all(&root);
        let mut deep = root.clone();
        for level in 0..=MAX_TREE_DEPTH + 1 {
            deep = deep.join(format!("level-{level}"));
        }
        std::fs::create_dir_all(&deep).expect("build the nested directories");

        let wanted = BTreeMap::new();
        let mut stale = Vec::new();
        let outcome = walk(&root, &wanted, &mut stale, 0);

        let _ = std::fs::remove_dir_all(&root);
        let message = outcome.expect_err("a tree past the cap is refused");
        assert!(
            message.contains(&format!("exceeds {MAX_TREE_DEPTH}")),
            "the refusal names the cap: {message}"
        );
        assert!(
            message.contains("level-"),
            "the refusal names the directory it stopped at: {message}"
        );
    }

    /// The shape the result tree actually has — `<case>/<system>/` — is
    /// nowhere near the cap and sweeps to the end.
    #[test]
    fn sweeps_the_real_tree_shape() {
        let root = std::env::temp_dir().join("zerodds-interop-results-walk-shape");
        let _ = std::fs::remove_dir_all(&root);
        let cell = root.join("case").join("system");
        std::fs::create_dir_all(&cell).expect("build the cell directory");
        let left_over = cell.join("xcdr2.le.bin");
        std::fs::write(&left_over, b"\x00").expect("write the stale file");

        let wanted = BTreeMap::new();
        let mut stale = Vec::new();
        walk(&root, &wanted, &mut stale, 0).expect("the real shape sweeps");

        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(stale, vec![left_over]);
    }
}
