// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-config`. Safety classification: **SAFE**.
//!
//! Layered configuration: a base file, sorted drop-in directories, nested
//! includes, and — for every value that survives the merge — the file and
//! line it came from.
//!
//! ## Why this exists
//!
//! Three daemons carried the same YAML-subset parser in three copies
//! (`coap-bridge::daemon::yaml`, `mqtt-bridge::daemon::yaml`,
//! `websocket-bridge::daemon::config`), each saying in its own header that it
//! was identical to the others, and the runtime read 28 `ZERODDS_*`
//! environment variables straight out of `RuntimeConfig::default()`. A
//! deployment could not answer the one question an operator asks first: where
//! does this value come from. Nor could it ship a package default and let a
//! site override one field without copying the whole file.
//!
//! ## The model
//!
//! A configuration is one tree of [`Node`]s, built by merging sources in a
//! defined order. Later sources win. Every scalar remembers its [`Origin`],
//! and [`Config::origin_of`] answers the operator's question by path.
//!
//! The order is fixed and dull on purpose:
//!
//! 1. the base file, if there is one,
//! 2. each drop-in directory in the order it was added, and inside one
//!    directory its files sorted by name — `10-net.yaml` before `20-net.yaml`,
//!    the convention every systemd user already knows,
//!
//! Each file first loads its includes depth first, in the listed order, then
//! merges its own keys. The textual position of `include:` does not change
//! this precedence. An include may name a file or an explicit directory.
//! Included directories use the same filename sort and extension filter as
//! top-level drop-ins. Subdirectories are read only when explicitly included.
//! Paths are relative to the file naming them. Explicit includes must exist,
//! unlike optional top-level drop-in directories. Cycles and excessive depth
//! are refused for both files and directories.
//!
//! ```yaml
//! include:
//!   - defaults.yaml
//!   - project.d
//! transport:
//!   mtu: 1400
//! ```
//!
//! ## Merging
//!
//! Maps merge key by key, recursively. A scalar or sequence replaces what was
//! there. A key written `+name` **appends** to the sequence already at `name`
//! rather than replacing it, which is what a drop-in usually wants:
//!
//! ```yaml
//! # 00-base.yaml
//! peers:
//!   - 10.0.0.1
//! # 10-site.yaml
//! +peers:
//!   - 10.0.0.2            # both, in this order
//! # 20-override.yaml
//! peers:
//!   - 10.0.0.9            # only this one
//! ```
//!
//! ## What this is not
//!
//! Not a YAML implementation. It reads the subset the daemons already used —
//! two-space indentation, `key: value`, `- item` (a `- key: value` item is a
//! map block that nests as written), flat flow lists of scalars
//! (`[a, "b"]`), `#` comments, `${VAR}` and `${VAR:-default}` — and says so
//! when it meets anything else, rather than guessing. Anchors, flow
//! mappings, nested flow lists, multi-document streams and block scalars are
//! refused by name.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub mod cli;
mod layers;
mod load;
mod parse;
pub mod read;

pub use layers::{
    CLI_SOURCE, CommandLine, ENV_SOURCE, SystemLayout, env_config, env_layer, is_secret_path,
    loader_for, path_node, unknown_keys,
};
pub use load::{Loader, MAX_INCLUDE_DEPTH, origins};

/// How deep a configuration tree may nest.
///
/// A parser that recurses per indentation level and a walker that recurses
/// per map level both turn a file into stack, and a configuration file is
/// something an operator downloads, a package ships and a container mounts.
/// Thirty-two is far past any hand-written layout — the deepest thing in this
/// tree is four — and far short of what a stack minds.
///
/// Reaching it is a refusal by name ([`ConfigError::TooDeep`]), not a
/// truncation: a file half-read is a configuration nobody wrote.
pub const MAX_NESTING_DEPTH: usize = 32;
pub use parse::parse_str;

/// Where a value came from.
///
/// Carried per scalar rather than per file, because the answer an operator
/// needs is about one value: after a base file and four drop-ins, *which* of
/// them set `transport.mtu`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A compiled-in default. No file said anything.
    Default,
    /// The process environment, under this variable name.
    Env(String),
    /// A file, at this line (1-based).
    File {
        /// Path as it was given to the loader, not canonicalised — an
        /// operator recognises the path they wrote.
        path: String,
        /// 1-based line of the `key:` that carried the value.
        line: usize,
    },
    /// Set programmatically by a caller.
    Caller,
    /// The command line, as the flag that carried it: `--set a.b=1`.
    Cli(String),
}

impl core::fmt::Display for Origin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Default => f.write_str("built-in default"),
            Self::Env(name) => write!(f, "environment ${name}"),
            Self::File { path, line } => write!(f, "{path}:{line}"),
            Self::Caller => f.write_str("set by the caller"),
            Self::Cli(flag) => write!(f, "command line `{flag}`"),
        }
    }
}

/// One node of a configuration tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A scalar, with the place it came from.
    Scalar(String, Origin),
    /// An ordered sequence.
    Seq(Vec<Node>),
    /// A mapping, ordered by key so that a rendered tree is stable.
    Map(BTreeMap<String, Node>),
}

impl Node {
    /// The scalar's text, or `None` for a sequence or a map.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Scalar(s, _) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The origin of a scalar; `None` for a sequence or a map, which have no
    /// single one.
    #[must_use]
    pub fn origin(&self) -> Option<&Origin> {
        match self {
            Self::Scalar(_, o) => Some(o),
            _ => None,
        }
    }

    /// Follows a dotted path — `transport.mtu` — through maps.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&Self> {
        let mut here = self;
        for segment in path.split('.') {
            match here {
                Self::Map(m) => here = m.get(segment)?,
                _ => return None,
            }
        }
        Some(here)
    }

    /// Merges `other` into `self`, with `other` winning.
    ///
    /// Maps merge key by key. Anything else replaces. A key spelled `+name`
    /// in `other` appends to the sequence at `name` instead of replacing it,
    /// and appending to something that is not a sequence is an error rather
    /// than a silent replacement — a drop-in that meant to extend a list and
    /// found a scalar has a mistake in it, and saying so is the whole point.
    ///
    /// # Errors
    /// [`ConfigError::AppendToNonSeq`] when `+name` meets a non-sequence,
    /// [`ConfigError::TooDeep`] past [`MAX_NESTING_DEPTH`].
    ///
    /// zerodds-lint: recursion-depth 32
    pub fn merge(&mut self, other: Self) -> Result<(), ConfigError> {
        self.merge_at(other, 0)
    }

    /// [`Self::merge`] with the depth it is at.
    ///
    /// zerodds-lint: recursion-depth 32
    fn merge_at(&mut self, other: Self, depth: usize) -> Result<(), ConfigError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(ConfigError::TooDeep(MAX_NESTING_DEPTH));
        }
        // Even the first map is a layer: its +keys need interpretation, also
        // in newly introduced nested maps. Installing it verbatim would leave
        // literal +keys in the effective configuration.
        if matches!(other, Self::Map(_)) && !matches!(self, Self::Map(_)) {
            *self = Self::Map(BTreeMap::new());
        }
        let (mine, theirs) = match (&mut *self, other) {
            (Self::Map(mine), Self::Map(theirs)) => (mine, theirs),
            // Not two maps: the later source stands in place of the earlier
            // one. A scalar does not merge with a sequence, and pretending it
            // does would leave a value nobody wrote.
            (slot, value) => {
                *slot = value;
                return Ok(());
            }
        };
        for (key, value) in theirs {
            if let Some(name) = key.strip_prefix('+') {
                append(mine, name, value)?;
                continue;
            }
            if matches!(value, Self::Map(_)) {
                mine.entry(key)
                    .or_insert_with(|| Self::Map(BTreeMap::new()))
                    .merge_at(value, depth + 1)?;
            } else {
                mine.insert(key, value);
            }
        }
        Ok(())
    }
}

/// `+name` in a later source appends to the sequence at `name`.
///
/// Appending to something that is not a sequence is an error and not a silent
/// replacement: a drop-in that meant to extend a list and found a scalar has a
/// mistake in it, and saying so is the whole point of the spelling.
fn append(into: &mut BTreeMap<String, Node>, name: &str, value: Node) -> Result<(), ConfigError> {
    match into.get_mut(name) {
        Some(Node::Seq(existing)) => {
            match value {
                Node::Seq(added) => existing.extend(added),
                single => existing.push(single),
            }
            Ok(())
        }
        Some(_) => Err(ConfigError::AppendToNonSeq(name.to_string())),
        None => {
            // Nothing to append to yet: the drop-in creates the sequence, so
            // the order of two drop-ins that both append does not depend on
            // whether a base file happened to declare an empty list.
            into.insert(
                name.to_string(),
                match value {
                    Node::Seq(v) => Node::Seq(v),
                    single => Node::Seq(alloc::vec![single]),
                },
            );
            Ok(())
        }
    }
}

/// What can go wrong while reading a configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The file could not be read. Carries the path and the OS message.
    Io(String, String),
    /// The text is not in the subset this reader accepts.
    Syntax {
        /// Path of the file the line is in.
        path: String,
        /// 1-based line.
        line: usize,
        /// What was wrong, in the reader's own words.
        message: String,
    },
    /// An `include:` chain came back to a file or directory already being read.
    IncludeCycle(Vec<String>),
    /// An `include:` chain went deeper than [`MAX_INCLUDE_DEPTH`].
    IncludeTooDeep(usize),
    /// `+name` was written where `name` is not a sequence.
    AppendToNonSeq(String),
    /// The tree nests deeper than [`MAX_NESTING_DEPTH`].
    TooDeep(usize),
    /// A command-line flag or a setting path that cannot be read.
    Usage(String),
    /// A value that parsed but is not one its setting accepts. Carries the
    /// setting path, where the value came from, and what was expected — the
    /// origin may be a file line, an environment variable or a flag.
    Invalid {
        /// Dotted setting path.
        setting: String,
        /// Where the refused value came from.
        origin: Origin,
        /// What the setting accepts.
        message: String,
    },
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(path, why) => write!(f, "{path}: {why}"),
            Self::Syntax {
                path,
                line,
                message,
            } => write!(f, "{path}:{line}: {message}"),
            Self::IncludeCycle(chain) => {
                write!(f, "include cycle: {}", chain.join(" -> "))
            }
            Self::IncludeTooDeep(depth) => {
                write!(f, "includes nested deeper than {depth}")
            }
            Self::AppendToNonSeq(key) => {
                write!(f, "`+{key}` appends, but `{key}` is not a sequence")
            }
            Self::TooDeep(depth) => {
                write!(f, "nested deeper than {depth} levels")
            }
            Self::Usage(message) => f.write_str(message),
            Self::Invalid {
                setting,
                origin,
                message,
            } => write!(f, "{origin}: invalid {setting}: {message}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// A merged configuration, and where each of its values came from.
#[derive(Debug, Clone, Default)]
pub struct Config {
    root: Option<Node>,
    /// Files read, in the order they were read. The audit trail an operator
    /// gets when the value is not where they expected it.
    sources: Vec<String>,
}

impl Config {
    /// The merged tree, or `None` when nothing was loaded.
    #[must_use]
    pub fn root(&self) -> Option<&Node> {
        self.root.as_ref()
    }

    /// Every file that contributed, in the order it was read.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// The node at a dotted path.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&Node> {
        self.root.as_ref()?.get(path)
    }

    /// The scalar at a dotted path.
    #[must_use]
    pub fn get_str(&self, path: &str) -> Option<&str> {
        self.get(path)?.as_str()
    }

    /// Where the value at a dotted path came from.
    ///
    /// This is the question a layered configuration exists to answer.
    #[must_use]
    pub fn origin_of(&self, path: &str) -> Option<&Origin> {
        self.get(path)?.origin()
    }

    /// Merges one more source in, after everything already there.
    /// A refused layer leaves both the effective tree and source list unchanged.
    ///
    /// # Errors
    /// Whatever [`Node::merge`] reports.
    pub fn push(&mut self, node: Node, source: String) -> Result<(), ConfigError> {
        self.push_each(alloc::vec![node], source)
    }

    /// Merges several trees in, one after another, as one source. Each
    /// tree's `+name` appends to what the ones before it left, which one
    /// pre-merged tree could not express. All or nothing, like [`Self::push`].
    ///
    /// # Errors
    /// Whatever [`Node::merge`] reports.
    pub fn push_each(&mut self, nodes: Vec<Node>, source: String) -> Result<(), ConfigError> {
        let mut root = self
            .root
            .clone()
            .unwrap_or_else(|| Node::Map(BTreeMap::new()));
        for node in nodes {
            root.merge(node)?;
        }
        self.root = Some(root);
        self.sources.push(source);
        Ok(())
    }
}
