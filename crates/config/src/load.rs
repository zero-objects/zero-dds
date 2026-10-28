// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Loading order: a base file, sorted drop-in directories, nested includes.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::path::{Path, PathBuf};

use crate::{Config, ConfigError, Node};

/// How deep `include:` may nest before the loader stops.
///
/// A bound and not a guess: cycle detection already refuses a chain that
/// returns to a file it is reading, so this catches the other shape — a
/// generator that emits `a` including `b` including `c` for ever without
/// repeating a name. Sixteen is past any hand-written layout.
pub const MAX_INCLUDE_DEPTH: usize = 16;

/// Builds a [`Config`] out of a base file and drop-in directories.
///
/// ```no_run
/// # use zerodds_config::Loader;
/// let config = Loader::new()
///     .base("/etc/zerodds/zerodds.yaml")
///     .drop_in_dir("/etc/zerodds/zerodds.d")
///     .drop_in_dir("/run/zerodds/zerodds.d")
///     .load()?;
/// // Where did this come from?
/// println!("{:?}", config.origin_of("transport.mtu"));
/// # Ok::<(), zerodds_config::ConfigError>(())
/// ```
#[derive(Debug, Default, Clone)]
pub struct Loader {
    base: Option<PathBuf>,
    drop_in_dirs: Vec<PathBuf>,
    extension: Option<String>,
    missing_base_is_error: bool,
}

impl Loader {
    /// A loader with nothing in it yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extension: Some("yaml".to_string()),
            ..Self::default()
        }
    }

    /// The base file, read before every drop-in.
    ///
    /// Absent by default: a package can ship only drop-ins, and a site can
    /// run with only a base.
    #[must_use]
    pub fn base(mut self, path: impl AsRef<Path>) -> Self {
        self.base = Some(path.as_ref().to_path_buf());
        self
    }

    /// Fail rather than continue when the base file is not there.
    ///
    /// Off by default, because "no configuration" is a legitimate state and a
    /// daemon that starts on its built-in defaults is what an empty
    /// `/etc/zerodds` should produce. On for a caller who passed `--config`
    /// and means it.
    #[must_use]
    pub fn require_base(mut self) -> Self {
        self.missing_base_is_error = true;
        self
    }

    /// Adds a drop-in directory, after the ones already added.
    ///
    /// Directories are read in the order they are added — a package
    /// directory, then a site one, then a runtime one — and inside each
    /// directory the files are read sorted by name. `10-net.yaml` before
    /// `20-net.yaml` before `99-local.yaml`, which is the ordering every
    /// systemd drop-in already follows and therefore the one an operator will
    /// assume without being told.
    ///
    /// A directory that does not exist is not an error. Half the point of
    /// drop-ins is that `/run/...` exists only sometimes.
    #[must_use]
    pub fn drop_in_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.drop_in_dirs.push(path.as_ref().to_path_buf());
        self
    }

    /// Only files with this extension are read from drop-in directories.
    ///
    /// `yaml` by default. `None` reads every file, which is what a directory
    /// of `.conf` fragments needs. Either way `.rpmnew`, `.dpkg-dist` and
    /// editor backups stay out when the default is kept — a package upgrade
    /// leaving `zerodds.yaml.rpmnew` behind must not quietly change what runs.
    #[must_use]
    pub fn extension(mut self, ext: Option<&str>) -> Self {
        self.extension = ext.map(ToString::to_string);
        self
    }

    /// Reads everything and merges it.
    ///
    /// # Errors
    /// [`ConfigError::Io`] for an unreadable file (and for a missing base
    /// after [`Self::require_base`]), [`ConfigError::Syntax`] for text outside
    /// the subset, [`ConfigError::IncludeCycle`] and
    /// [`ConfigError::IncludeTooDeep`] for include chains that do not end.
    pub fn load(&self) -> Result<Config, ConfigError> {
        self.load_onto(Config::default())
    }

    /// Reads everything and merges it over `config`, which keeps its sources
    /// and loses every value a file sets again. The environment layer goes in
    /// here, so that the files win over it and a drop-in's `+name` appends to
    /// what it set.
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn load_onto(&self, mut config: Config) -> Result<Config, ConfigError> {
        if let Some(base) = &self.base {
            if base
                .try_exists()
                .map_err(|error| ConfigError::Io(base.display().to_string(), error.to_string()))?
            {
                self.read_into(&mut config, base, &mut Vec::new(), 0)?;
            } else if self.missing_base_is_error {
                return Err(ConfigError::Io(
                    base.display().to_string(),
                    "no such file".to_string(),
                ));
            }
        }
        for dir in &self.drop_in_dirs {
            for file in self.sorted_files(dir)? {
                self.read_into(&mut config, &file, &mut Vec::new(), 0)?;
            }
        }
        Ok(config)
    }

    /// The files of one drop-in directory, sorted by name.
    ///
    /// Sorted by the file name and not by what the filesystem happens to
    /// return: `read_dir` gives no order, and a configuration whose result
    /// depends on inode layout is not a configuration.
    fn sorted_files(&self, dir: &Path) -> Result<Vec<PathBuf>, ConfigError> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            // A drop-in directory that is not there contributes nothing.
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(ConfigError::Io(dir.display().to_string(), e.to_string())),
        };
        let mut files: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|e| ConfigError::Io(dir.display().to_string(), e.to_string()))?;
            let path = entry.path();
            if let Some(want) = &self.extension {
                if path.extension().and_then(|e| e.to_str()) != Some(want.as_str()) {
                    continue;
                }
            }
            let metadata = std::fs::metadata(&path)
                .map_err(|e| ConfigError::Io(path.display().to_string(), e.to_string()))?;
            if !metadata.is_file() {
                continue;
            }
            files.push(path);
        }
        files.sort();
        Ok(files)
    }

    /// Reads one file or explicitly included directory, resolving includes
    /// before the including file's own keys. Directory entries are not
    /// recursively discovered: further directories require another include.
    ///
    /// zerodds-lint: recursion-depth 16
    fn read_into(
        &self,
        config: &mut Config,
        path: &Path,
        chain: &mut Vec<String>,
        depth: usize,
    ) -> Result<(), ConfigError> {
        if depth > MAX_INCLUDE_DEPTH {
            return Err(ConfigError::IncludeTooDeep(MAX_INCLUDE_DEPTH));
        }
        let shown = path.display().to_string();
        // Canonicalise for the comparison only: two names for one file are
        // still a cycle, and the operator wants to read the names they wrote.
        let identity = std::fs::canonicalize(path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| shown.clone());
        if chain.contains(&identity) {
            let mut names = chain.clone();
            names.push(identity);
            return Err(ConfigError::IncludeCycle(names));
        }
        chain.push(identity);

        // Unlike optional top-level drop-ins, explicitly included paths must
        // exist. Resolve metadata before sorted_files, whose API deliberately
        // treats a missing top-level directory as empty.
        let metadata =
            std::fs::metadata(path).map_err(|e| ConfigError::Io(shown.clone(), e.to_string()))?;
        if metadata.is_dir() {
            for file in self.sorted_files(path)? {
                self.read_into(config, &file, chain, depth + 1)?;
            }
            chain.pop();
            return Ok(());
        }

        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(shown.clone(), e.to_string()))?;
        let mut node = crate::parse_str(&text, &shown)?;

        // Includes are defaults for this file, regardless of where the
        // include key appears in its text. Their sequence order is retained.
        let includes = take_includes(&mut node, &shown)?;
        for relative in includes {
            let resolved = resolve_include(path, &relative);
            self.read_into(config, &resolved, chain, depth + 1)?;
        }
        config.push(node, shown)?;
        chain.pop();
        Ok(())
    }
}

/// Removes the `include:` key and returns what it named.
fn take_includes(node: &mut Node, path: &str) -> Result<Vec<String>, ConfigError> {
    let Node::Map(map) = node else {
        return Ok(Vec::new());
    };
    match map.remove("include") {
        None => Ok(Vec::new()),
        Some(Node::Seq(items)) => items
            .into_iter()
            .map(|item| include_path(item, path))
            .collect(),
        Some(one) => Ok(alloc::vec![include_path(one, path)?]),
    }
}

/// An invalid directive cannot silently remove a requested configuration
/// layer. Containers have no single stored source line, so diagnostics for
/// those point at the file, line 1; scalar diagnostics retain their line.
fn include_path(node: Node, path: &str) -> Result<String, ConfigError> {
    let line = match node.origin() {
        Some(crate::Origin::File { line, .. }) => *line,
        _ => 1,
    };
    if let Node::Scalar(value, _) = node {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }
    Err(ConfigError::Syntax {
        path: path.to_string(),
        line,
        message: "include expects a nonempty path or a sequence of nonempty paths".to_string(),
    })
}

/// An include is relative to the file that wrote it, absolute stays absolute.
fn resolve_include(from: &Path, relative: &str) -> PathBuf {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    from.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(candidate)
}

/// Every path in a tree, dotted, with the origin of its scalar.
///
/// The operator-facing view: one line per value, saying where it came from.
#[must_use]
pub fn origins(config: &Config) -> Vec<(String, crate::Origin)> {
    let mut out = Vec::new();
    if let Some(root) = config.root() {
        walk(root, &mut String::new(), &mut out);
    }
    out
}

/// zerodds-lint: recursion-depth 32
///
/// Bounded by construction: the tree it walks came out of `parse_str`, which
/// refuses past [`crate::MAX_NESTING_DEPTH`]. The guard is here anyway, since
/// a caller may hand in a tree it built itself.
fn walk(node: &Node, prefix: &mut String, out: &mut Vec<(String, crate::Origin)>) {
    walk_at(node, prefix, out, 0);
}

/// zerodds-lint: recursion-depth 32
fn walk_at(node: &Node, prefix: &mut String, out: &mut Vec<(String, crate::Origin)>, depth: usize) {
    if depth > crate::MAX_NESTING_DEPTH {
        return;
    }
    match node {
        Node::Scalar(_, origin) => out.push((prefix.clone(), origin.clone())),
        Node::Map(map) => {
            for (key, value) in map {
                let mark = prefix.len();
                if !prefix.is_empty() {
                    prefix.push('.');
                }
                prefix.push_str(key);
                walk_at(value, prefix, out, depth + 1);
                prefix.truncate(mark);
            }
        }
        Node::Seq(items) => {
            for (i, item) in items.iter().enumerate() {
                let mark = prefix.len();
                prefix.push_str(&alloc::format!("[{i}]"));
                walk_at(item, prefix, out, depth + 1);
                prefix.truncate(mark);
            }
        }
    }
}
