// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The foreign programs a scenario talks to.
//!
//! A scenario never skips: a broker, client or compiler that is not
//! installed fails it, naming the program, because a pillar job that
//! passes without its foreign half has measured nothing
//! (`rc8-test-suite.md`, "Rules": a skip is a failure unless declared).
//! The jobs provide the programs (`ci/integration.yml`, `tests/integration/provision.sh`).

use std::path::{Path, PathBuf};

/// The path of `program`, looked up in `PATH` and then in `extra`
/// directories.
#[must_use]
pub fn find(program: &str, extra: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(extra.iter().map(PathBuf::from))
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

/// [`find`], or a failed scenario that names the program.
///
/// # Panics
/// When `program` is in neither `PATH` nor `extra`.
#[must_use]
pub fn require(program: &str, extra: &[&str]) -> PathBuf {
    // zerodds-lint: allow no_panic_in_safe — a missing peer fails the scenario.
    find(program, extra).unwrap_or_else(|| {
        panic!(
            "`{program}` is not installed (PATH and {extra:?}); this scenario needs it \
             and does not skip: the job installs it (tests/integration/provision.sh)"
        )
    })
}

/// The path of an environment variable that names a built program (the
/// jobs build the daemons before the scenarios and export their paths).
///
/// # Panics
/// When the variable is unset or does not name an executable file.
#[must_use]
pub fn built(variable: &str) -> PathBuf {
    // zerodds-lint: allow no_panic_in_safe — a missing daemon fails the scenario.
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "{variable} is not set: it names the daemon this scenario starts; \
             ci/run_integration.py builds it and sets it"
            )
        });
    // zerodds-lint: allow no_panic_in_safe — see above.
    assert!(
        is_executable(&path),
        "{variable}={} is not an executable file",
        path.display()
    );
    path
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
