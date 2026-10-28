//! Shared by `tool_paths_are_relative.rs` and `website_tool_paths.rs`.

use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root above tools/traceability")
        .to_path_buf()
}

/// The home-directory prefixes that identify an absolute developer path.
/// A repository path under any of these is machine-specific by construction.
const ABSOLUTE_HOME_PREFIXES: [&str; 2] = ["/Users/", "/home/"];

/// Collects `*.py` under `dir`, one level deep plus subdirectories.
pub fn python_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            python_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "py") {
            out.push(p);
        }
    }
}

/// Every quoted literal in `files` that begins with a home-directory
/// prefix, as `path:line  code`. Comment lines are skipped: a docstring may
/// quote such a path as an example, and it is the executable constant that
/// does the damage.
pub fn absolute_paths(root: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut offenders: Vec<String> = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with('#') {
                continue;
            }
            for prefix in ABSOLUTE_HOME_PREFIXES {
                // Only a quoted literal counts; the prefix must be inside a
                // string, which is how the path reaches `Path(...)`.
                for quote in ['"', '\''] {
                    let needle = format!("{quote}{prefix}");
                    if code.contains(&needle) {
                        offenders.push(format!(
                            "{}:{}  {}",
                            f.strip_prefix(root).unwrap_or(f).display(),
                            i + 1,
                            code.trim()
                        ));
                    }
                }
            }
        }
    }
    offenders
}
