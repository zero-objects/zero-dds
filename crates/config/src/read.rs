// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed reads out of a merged tree, for a program's own schema.
//!
//! Every refusal is a [`ConfigError::Invalid`] that names the dotted setting,
//! the origin of the value (file and line, environment variable, flag) and
//! what the setting accepts. A key a schema does not name is refused the same
//! way: `tls.cert_fiel` is valid text and a value nobody reads.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{ConfigError, Node, Origin};

/// One map of the tree, read under a dotted prefix.
#[derive(Debug, Clone)]
pub struct Section<'a> {
    map: Option<&'a BTreeMap<String, Node>>,
    prefix: String,
}

static EMPTY: BTreeMap<String, Node> = BTreeMap::new();

/// The origin of the first scalar under `node`, for refusals about a
/// sequence or a map, which carry none of their own.
///
/// zerodds-lint: recursion-depth 32
#[must_use]
pub fn first_origin(node: &Node) -> Origin {
    /// zerodds-lint: recursion-depth 32
    fn walk(node: &Node, depth: usize) -> Option<&Origin> {
        if depth > crate::MAX_NESTING_DEPTH {
            return None;
        }
        match node {
            Node::Scalar(_, origin) => Some(origin),
            Node::Seq(items) => items.iter().find_map(|n| walk(n, depth + 1)),
            Node::Map(map) => map.values().find_map(|n| walk(n, depth + 1)),
        }
    }
    walk(node, 0).cloned().unwrap_or(Origin::Default)
}

fn invalid(setting: String, origin: Origin, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        setting,
        origin,
        message: message.into(),
    }
}

impl<'a> Section<'a> {
    /// The root of a tree (`None` = nothing was loaded: every read is absent).
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] when the root is not a map.
    pub fn root(node: Option<&'a Node>) -> Result<Self, ConfigError> {
        Self::of(node, "")
    }

    /// `node` as a section named `prefix`.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] when `node` is not a map.
    pub fn of(node: Option<&'a Node>, prefix: impl Into<String>) -> Result<Self, ConfigError> {
        let prefix = prefix.into();
        match node {
            None => Ok(Self { map: None, prefix }),
            Some(Node::Map(map)) => Ok(Self {
                map: Some(map),
                prefix,
            }),
            Some(other) => Err(invalid(
                prefix,
                first_origin(other),
                "expected a map of settings",
            )),
        }
    }

    fn path(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            alloc::format!("{}.{key}", self.prefix)
        }
    }

    /// The dotted path of `key` in this section, for a caller's own refusal.
    #[must_use]
    pub fn path_of(&self, key: &str) -> String {
        self.path(key)
    }

    /// The raw node at `key`.
    #[must_use]
    pub fn node(&self, key: &str) -> Option<&'a Node> {
        self.map.unwrap_or(&EMPTY).get(key)
    }

    /// Refuses every key that `allowed` does not name.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] for the first unknown key, with its origin.
    pub fn only(&self, allowed: &[&str]) -> Result<(), ConfigError> {
        for (key, node) in self.map.unwrap_or(&EMPTY) {
            if !allowed.contains(&key.as_str()) {
                return Err(invalid(
                    self.path(key),
                    first_origin(node),
                    alloc::format!("unknown setting; known here: {}", allowed.join(", ")),
                ));
            }
        }
        Ok(())
    }

    /// The nested section at `key`.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] when it is not a map.
    pub fn section(&self, key: &str) -> Result<Section<'a>, ConfigError> {
        Section::of(self.node(key), self.path(key))
    }

    /// The scalar at `key` and where it came from.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] when `key` holds a list or a map.
    pub fn scalar(&self, key: &str) -> Result<Option<(&'a str, &'a Origin)>, ConfigError> {
        match self.node(key) {
            None => Ok(None),
            Some(Node::Scalar(value, origin)) => Ok(Some((value.as_str(), origin))),
            Some(other) => Err(invalid(
                self.path(key),
                first_origin(other),
                "expected a single value",
            )),
        }
    }

    /// The text at `key`.
    ///
    /// # Errors
    /// As [`Self::scalar`].
    pub fn string(&self, key: &str) -> Result<Option<String>, ConfigError> {
        Ok(self.scalar(key)?.map(|(v, _)| v.to_string()))
    }

    /// The value at `key`, parsed; `expect` says what the setting accepts.
    ///
    /// # Errors
    /// As [`Self::scalar`], and [`ConfigError::Invalid`] when it does not
    /// parse.
    pub fn parse<T: core::str::FromStr>(
        &self,
        key: &str,
        expect: &str,
    ) -> Result<Option<T>, ConfigError> {
        match self.scalar(key)? {
            None => Ok(None),
            Some((value, origin)) => value.trim().parse::<T>().map(Some).map_err(|_| {
                invalid(
                    self.path(key),
                    origin.clone(),
                    alloc::format!("got `{value}`, expected {expect}"),
                )
            }),
        }
    }

    /// `true`/`false` (or `1`/`0`) at `key`.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] for any other spelling.
    pub fn boolean(&self, key: &str) -> Result<Option<bool>, ConfigError> {
        match self.scalar(key)? {
            None => Ok(None),
            Some((value, origin)) => match value.trim() {
                "true" | "1" => Ok(Some(true)),
                "false" | "0" => Ok(Some(false)),
                other => Err(invalid(
                    self.path(key),
                    origin.clone(),
                    alloc::format!("got `{other}`, expected true/false"),
                )),
            },
        }
    }

    /// One of `choices` at `key`.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] for anything else.
    pub fn choice(&self, key: &str, choices: &[&str]) -> Result<Option<String>, ConfigError> {
        match self.scalar(key)? {
            None => Ok(None),
            Some((value, origin)) => {
                let value = value.trim();
                if choices.contains(&value) {
                    Ok(Some(value.to_string()))
                } else {
                    Err(invalid(
                        self.path(key),
                        origin.clone(),
                        alloc::format!("got `{value}`, expected one of {}", choices.join(", ")),
                    ))
                }
            }
        }
    }

    /// A list of texts at `key`: a sequence of scalars, or one scalar as a
    /// list of one.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] for a map or a sequence holding one.
    pub fn strings(&self, key: &str) -> Result<Option<Vec<String>>, ConfigError> {
        match self.node(key) {
            None => Ok(None),
            Some(Node::Scalar(value, _)) => Ok(Some(alloc::vec![value.clone()])),
            Some(Node::Seq(items)) => items
                .iter()
                .map(|item| match item {
                    Node::Scalar(value, _) => Ok(value.clone()),
                    other => Err(invalid(
                        self.path(key),
                        first_origin(other),
                        "expected a list of values",
                    )),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            Some(other) => Err(invalid(
                self.path(key),
                first_origin(other),
                "expected a list of values",
            )),
        }
    }

    /// The items of the sequence at `key`, each with its dotted path.
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] when `key` is not a sequence.
    pub fn items(&self, key: &str) -> Result<Vec<(String, &'a Node)>, ConfigError> {
        match self.node(key) {
            None => Ok(Vec::new()),
            Some(Node::Seq(items)) => Ok(items
                .iter()
                .enumerate()
                .map(|(i, item)| (alloc::format!("{}[{i}]", self.path(key)), item))
                .collect()),
            Some(other) => Err(invalid(
                self.path(key),
                first_origin(other),
                "expected a list",
            )),
        }
    }
}

/// Splits a bridge's `--topic DDS:OTHER` shorthand at the single `:` that is
/// not part of an IDL `::`: `Chat::Message:chat/msg` is the DDS topic
/// `Chat::Message` with `chat/msg` on the other side; without such a `:` the
/// whole text is the DDS topic.
#[must_use]
pub fn split_topic_shorthand(text: &str) -> (String, Option<String>) {
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        let lone =
            *b == b':' && bytes.get(i + 1) != Some(&b':') && (i == 0 || bytes[i - 1] != b':');
        if lone {
            return (
                text[..i].trim().to_string(),
                Some(text[i + 1..].trim().to_string()),
            );
        }
    }
    (text.trim().to_string(), None)
}
