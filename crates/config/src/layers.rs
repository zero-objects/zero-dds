// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two layers around the files: the environment beneath them and the
//! command line above them.
//!
//! The precedence every ZeroDDS program uses, lowest first:
//!
//! 1. compiled-in defaults,
//! 2. the environment, under the names a consumer's table lists,
//! 3. the files — base, drop-in directories, includes, in [`crate::Loader`]
//!    order,
//! 4. `--set path=value` on the command line, in the order written.
//!
//! `--config PATH` does not add a layer. It selects which base file layer 3
//! starts from; without it, layer 3 is the [`SystemLayout`] of the platform.
//! `--check-config` loads all four exactly as a start would, and the program
//! reports instead of running.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::path::PathBuf;

use crate::{Config, ConfigError, Node, Origin};

/// The source name the environment layer carries in [`Config::sources`].
pub const ENV_SOURCE: &str = "environment";

/// The source name the command-line layer carries in [`Config::sources`].
pub const CLI_SOURCE: &str = "command line";

/// A tree holding one value at a dotted path.
///
/// A last segment written `+name` appends when the tree is merged, the same
/// spelling a drop-in file uses.
///
/// # Errors
/// [`ConfigError::Usage`] for an empty path or an empty segment — `a..b`,
/// `.a`, `a.` name nothing, and reading them as something would be a guess.
pub fn path_node(path: &str, value: &str, origin: Origin) -> Result<Node, ConfigError> {
    at_path(path, Node::Scalar(value.to_string(), origin))
}

/// `leaf` at `path`, under the rules of [`path_node`].
fn at_path(path: &str, leaf: Node) -> Result<Node, ConfigError> {
    let segments: Vec<&str> = path.split('.').collect();
    if segments.iter().any(|s| s.is_empty() || *s == "+") {
        return Err(ConfigError::Usage(alloc::format!(
            "`{path}` is not a dotted setting path"
        )));
    }
    if segments[..segments.len() - 1]
        .iter()
        .any(|s| s.starts_with('+'))
    {
        return Err(ConfigError::Usage(alloc::format!(
            "`{path}`: only the last segment may be written `+name`"
        )));
    }
    let mut node = leaf;
    for segment in segments.iter().rev() {
        let mut map = BTreeMap::new();
        map.insert((*segment).to_string(), node);
        node = Node::Map(map);
    }
    Ok(node)
}

/// A `--set` path as the tree spells it: `+discovery.peers`, the spelling
/// an operator reaches for, is `discovery.+peers` there.
fn tree_path(path: &str) -> String {
    let path = path.trim();
    match path.strip_prefix('+') {
        Some(rest) => match rest.rsplit_once('.') {
            Some((head, last)) => alloc::format!("{head}.+{last}"),
            None => alloc::format!("+{rest}"),
        },
        None => path.to_string(),
    }
}

/// The environment layer: one value per variable of `table` that `lookup`
/// finds, at the path the table gives it.
///
/// `table` pairs a variable name with a dotted path, the way the runtime's
/// `SETTINGS` does. `lookup` is the environment — `std::env::var` in a
/// program, a map in a test. An empty value counts as set: `FOO=` is how an
/// operator clears a list.
///
/// A path `is_list` accepts becomes a sequence, split at commas, so that a
/// drop-in's `+name` appends to what the environment set instead of meeting
/// a scalar. An empty value is the empty sequence.
///
/// # Errors
/// [`ConfigError::Usage`] when a table path is not a dotted path, which is a
/// defect in the table rather than in the environment.
pub fn env_layer<F, L>(table: &[(&str, &str)], lookup: F, is_list: L) -> Result<Node, ConfigError>
where
    F: Fn(&str) -> Option<String>,
    L: Fn(&str) -> bool,
{
    let mut root = Node::Map(BTreeMap::new());
    for &(name, path) in table {
        let Some(value) = lookup(name) else {
            continue;
        };
        let origin = Origin::Env(name.to_string());
        let mut node = path_node(path, &value, origin.clone())?;
        if is_list(path) {
            let items = value
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| Node::Scalar(v.to_string(), origin.clone()))
                .collect();
            set_leaf(&mut node, Node::Seq(items));
        }
        root.merge(node)?;
    }
    Ok(root)
}

/// Replaces the single leaf of a tree [`path_node`] built.
fn set_leaf(node: &mut Node, leaf: Node) {
    let mut here = node;
    loop {
        match here {
            Node::Map(map) => match map.values_mut().next() {
                Some(next) => here = next,
                None => return,
            },
            slot => {
                *slot = leaf;
                return;
            }
        }
    }
}

/// What the configuration flags on a command line said.
///
/// Every ZeroDDS program reads the same two flags, so they are parsed once
/// here and the program sees only the arguments that are its own:
///
/// - `--config PATH` / `--config=PATH` selects the base file. Given twice, it
///   is refused: which of the two was meant is not the parser's to decide.
/// - `--set PATH=VALUE` / `--set=PATH=VALUE` sets one value above every file.
///   Repeatable; a later one wins, `--set +discovery.peers=…` appends.
///
/// - `--check-config` asks the program to validate and report the effective
///   configuration and exit, without starting.
///
/// Parsing stops at `--`; what follows it belongs to the program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandLine {
    config: Option<PathBuf>,
    sets: Vec<(String, String)>,
    entries: Vec<Entry>,
    check: bool,
}

/// One map a program's own flag appends to a list, see
/// [`CommandLine::push_entry`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    path: String,
    fields: Vec<(String, String)>,
    flag: String,
}

impl CommandLine {
    /// Takes the configuration flags out of `args` and returns the rest in
    /// their original order.
    ///
    /// A long option in the rest written `--opt=value` comes back as `--opt`
    /// and `value`, so every program accepts both spellings the man pages use
    /// (before, amqp-, grpc- and corba-bridged and `zerodds-ros2-shim`
    /// answered `--domain=3` with "unknown argument", measured). What follows
    /// a `--` is passed on untouched.
    ///
    /// `args` excludes the program name.
    ///
    /// # Errors
    /// [`ConfigError::Usage`] for a flag without its value, a `--set` without
    /// `=`, an empty path, or `--config` given twice.
    pub fn extract<I>(args: I) -> Result<(Self, Vec<String>), ConfigError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut out = Self::default();
        let mut rest = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--" {
                rest.push(arg);
                rest.extend(args);
                break;
            }
            let (flag, inline) = match arg.split_once('=') {
                Some((flag, value)) if flag == "--config" || flag == "--set" => {
                    (flag.to_string(), Some(value.to_string()))
                }
                _ => (arg.clone(), None),
            };
            match flag.as_str() {
                "--check-config" => out.check = true,
                "--config" => {
                    let value = match inline {
                        Some(v) => v,
                        None => args.next().ok_or_else(|| {
                            ConfigError::Usage("--config expects a path".to_string())
                        })?,
                    };
                    if value.is_empty() {
                        return Err(ConfigError::Usage("--config expects a path".to_string()));
                    }
                    if out.config.is_some() {
                        return Err(ConfigError::Usage(
                            "--config given twice; name one base file".to_string(),
                        ));
                    }
                    out.config = Some(PathBuf::from(value));
                }
                "--set" => {
                    let value = match inline {
                        Some(v) => v,
                        None => args.next().ok_or_else(|| {
                            ConfigError::Usage("--set expects PATH=VALUE".to_string())
                        })?,
                    };
                    out.push_set(&value)?;
                }
                _ => match arg.strip_prefix("--").and_then(|a| a.split_once('=')) {
                    Some((name, value)) if !name.is_empty() => {
                        rest.push(alloc::format!("--{name}"));
                        rest.push(value.to_string());
                    }
                    _ => rest.push(arg),
                },
            }
        }
        Ok((out, rest))
    }

    /// Adds one `PATH=VALUE`, as a `--set` after the ones already given.
    ///
    /// # Errors
    /// [`ConfigError::Usage`] without `=` or with an invalid path.
    pub fn push_set(&mut self, assignment: &str) -> Result<(), ConfigError> {
        let Some((path, value)) = assignment.split_once('=') else {
            return Err(ConfigError::Usage(alloc::format!(
                "--set {assignment}: expected PATH=VALUE"
            )));
        };
        let path = tree_path(path);
        path_node(&path, value, Origin::Default)?;
        self.sets.push((path, value.to_string()));
        Ok(())
    }

    /// Appends one map of scalars to the list at `path`, for a program flag
    /// whose value is a list entry with several settings — a `--set` value
    /// is always one scalar. `path` is spelled as for `--set`
    /// (`+types`), `flag` is the flag as the operator wrote it and becomes
    /// the origin of every field. Entries are merged after every `--set`,
    /// in the order pushed.
    ///
    /// # Errors
    /// [`ConfigError::Usage`] for an invalid path, a path not written
    /// `+name`, or a field name that is not one path segment.
    pub fn push_entry(
        &mut self,
        path: &str,
        fields: &[(&str, &str)],
        flag: &str,
    ) -> Result<(), ConfigError> {
        let path = tree_path(path);
        if !path
            .rsplit('.')
            .next()
            .is_some_and(|last| last.starts_with('+'))
        {
            return Err(ConfigError::Usage(alloc::format!(
                "{flag}: `{path}` appends only when written `+name`"
            )));
        }
        at_path(&path, Node::Map(BTreeMap::new()))?;
        if let Some((name, _)) = fields
            .iter()
            .find(|(name, _)| name.is_empty() || name.contains('.') || name.starts_with('+'))
        {
            return Err(ConfigError::Usage(alloc::format!(
                "{flag}: `{name}` is not a setting name"
            )));
        }
        self.entries.push(Entry {
            path,
            fields: fields
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect(),
            flag: flag.to_string(),
        });
        Ok(())
    }

    /// Whether `--check-config` was given.
    #[must_use]
    pub fn check(&self) -> bool {
        self.check
    }

    /// Selects a base file, as `--config` would.
    #[must_use]
    pub fn with_config(mut self, path: impl Into<PathBuf>) -> Self {
        self.config = Some(path.into());
        self
    }

    /// The base file `--config` named.
    #[must_use]
    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.config.as_deref()
    }

    /// Every `--set`, as `(path, value)`, in the order given. The path keeps
    /// a leading `+` on its last segment.
    #[must_use]
    pub fn sets(&self) -> &[(String, String)] {
        &self.sets
    }

    /// The `--set` values, one tree each, in the order given, each value's
    /// origin naming its flag. Kept apart so that `+name` appends to the
    /// layers below instead of to an empty tree.
    ///
    /// # Errors
    /// [`ConfigError::Usage`] for an invalid path.
    pub fn layer(&self) -> Result<Vec<Node>, ConfigError> {
        let sets = self.sets.iter().map(|(path, value)| {
            let shown = if is_secret_path(path) {
                "***"
            } else {
                value.as_str()
            };
            path_node(
                path,
                value,
                Origin::Cli(alloc::format!("--set {path}={shown}")),
            )
        });
        let entries = self.entries.iter().map(|entry| {
            let origin = Origin::Cli(entry.flag.clone());
            let map = entry
                .fields
                .iter()
                .map(|(name, value)| (name.clone(), Node::Scalar(value.clone(), origin.clone())))
                .collect();
            at_path(&entry.path, Node::Map(map))
        });
        sets.chain(entries).collect()
    }

    /// Merges [`Self::layer`] onto `config` as its topmost source. Nothing is
    /// pushed when no `--set` was given, so [`Config::sources`] stays the
    /// list of what actually contributed.
    ///
    /// # Errors
    /// Whatever [`Config::push_each`] reports.
    pub fn apply(&self, config: &mut Config) -> Result<(), ConfigError> {
        if self.sets.is_empty() && self.entries.is_empty() {
            return Ok(());
        }
        config.push_each(self.layer()?, CLI_SOURCE.to_string())
    }
}

/// A [`Config`] holding only the environment layer, ready for
/// [`crate::Loader::load_onto`]. Empty and without a source entry when no
/// variable of `table` is set.
///
/// # Errors
/// Whatever [`env_layer`] reports.
pub fn env_config<F, L>(
    table: &[(&str, &str)],
    lookup: F,
    is_list: L,
) -> Result<Config, ConfigError>
where
    F: Fn(&str) -> Option<String>,
    L: Fn(&str) -> bool,
{
    let mut config = Config::default();
    let node = env_layer(table, lookup, is_list)?;
    if matches!(&node, Node::Map(m) if !m.is_empty()) {
        config.push(node, ENV_SOURCE.to_string())?;
    }
    Ok(config)
}

/// Where a platform keeps the system configuration of one ZeroDDS program.
///
/// The directories are those of `docs/specs/zerodds-deployment-1.0.md`:
/// `/etc/zerodds` on Linux and other Unix systems (§2.1),
/// `/usr/local/etc/zerodds` on macOS (§3.1), `%PROGRAMDATA%\ZeroDDS` on
/// Windows (§4.1). Linux adds the volatile `/run/zerodds/<stem>.d` after the
/// persistent drop-ins, for values a unit writes at boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLayout {
    /// The base file, `<dir>/<stem>.yaml`.
    pub base: PathBuf,
    /// Drop-in directories, read in this order after the base.
    pub drop_ins: Vec<PathBuf>,
}

impl SystemLayout {
    /// The platform's configuration directory.
    #[must_use]
    pub fn dir() -> PathBuf {
        #[cfg(windows)]
        {
            let root = std::env::var_os("PROGRAMDATA")
                .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
            root.join("ZeroDDS")
        }
        #[cfg(target_os = "macos")]
        {
            PathBuf::from("/usr/local/etc/zerodds")
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            PathBuf::from("/etc/zerodds")
        }
    }

    /// The layout for base name `stem`: `zerodds` for the DDS runtime,
    /// `ws-bridged` for the WebSocket bridge.
    #[must_use]
    pub fn for_stem(stem: &str) -> Self {
        let dir = Self::dir();
        #[allow(unused_mut)]
        let mut drop_ins = alloc::vec![dir.join(alloc::format!("{stem}.d"))];
        #[cfg(target_os = "linux")]
        drop_ins.push(PathBuf::from("/run/zerodds").join(alloc::format!("{stem}.d")));
        Self {
            base: dir.join(alloc::format!("{stem}.yaml")),
            drop_ins,
        }
    }

    /// A loader for this layout. The base is optional: an empty system
    /// directory means the built-in defaults.
    #[must_use]
    pub fn loader(&self) -> crate::Loader {
        self.drop_ins
            .iter()
            .fold(crate::Loader::new().base(&self.base), |l, d| {
                l.drop_in_dir(d)
            })
    }
}

/// The loader a command line selects: `--config PATH` is a required base
/// with its sibling `<stem>.d`, otherwise the system layout for `stem`.
#[must_use]
pub fn loader_for(command_line: &CommandLine, stem: &str) -> crate::Loader {
    match command_line.config_path() {
        Some(path) => crate::Loader::new()
            .base(path)
            .require_base()
            .drop_in_dir(path.with_extension("d")),
        None => SystemLayout::for_stem(stem).loader(),
    }
}

/// Every scalar path in `config` that `known` does not accept, with where it
/// came from. A sequence element is reported by its sequence's path.
///
/// This is the typo check: `discovery.peer` instead of `discovery.peers` is
/// valid text and a value no program reads.
#[must_use]
pub fn unknown_keys<K>(config: &Config, known: K) -> Vec<(String, Origin)>
where
    K: Fn(&str) -> bool,
{
    let mut out: Vec<(String, Origin)> = Vec::new();
    for (path, origin) in crate::origins(config) {
        let path = path.split('[').next().unwrap_or_default().to_string();
        if !known(&path) && !out.iter().any(|(p, _)| *p == path) {
            out.push((path, origin));
        }
    }
    out
}

/// Whether a setting holds a secret, judged by its path segments:
/// `auth.bearer_token`, `mqtt.password`, `oscore.master_secret`, and every
/// entry under `auth.sasl_users` (user name to password). A report or an
/// origin shows `***` in place of such a value. A path to a key file
/// (`tls.key_file`) is not a secret; its contents are.
#[must_use]
pub fn is_secret_path(path: &str) -> bool {
    path.split('.').any(|segment| {
        let segment = segment
            .split('[')
            .next()
            .unwrap_or_default()
            .trim_start_matches('+')
            .to_ascii_lowercase();
        [
            "token",
            "password",
            "secret",
            "passphrase",
            "psk",
            "sasl_users",
            "credentials",
        ]
        .iter()
        .any(|word| segment.contains(word))
    })
}
