// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The command line of a ZeroDDS daemon, read one flag at a time.
//!
//! Each bridge daemon spelled out the same loop: split `--flag=VALUE`, take
//! the value from the next argument when there is no `=`, name the flag a
//! value is missing for. They differ only in their flags, so the loop is
//! here and the flags stay with each daemon:
//!
//! ```
//! use zerodds_config::cli::{CliError, Flags};
//!
//! let args = ["--domain=7".to_string(), "--topic".to_string(), "T".to_string()];
//! let (mut domain, mut topics) = (None::<i32>, Vec::new());
//! let mut flags = Flags::new(&args);
//! while let Some(flag) = flags.next_flag() {
//!     match flag.name() {
//!         "--domain" => domain = Some(flags.parsed(&flag)?),
//!         "--topic" => topics.push(flags.value(&flag)?),
//!         other => return Err(CliError::UnknownFlag(other.into())),
//!     }
//! }
//! assert_eq!((domain, topics), (Some(7), vec!["T".to_string()]));
//! # Ok::<(), CliError>(())
//! ```

use alloc::string::{String, ToString};
use core::str::FromStr;

/// What is wrong with a daemon's command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// A flag the daemon does not know.
    UnknownFlag(String),
    /// A flag that takes a value, at the end of the command line.
    MissingValue(String),
    /// A value its flag cannot use.
    InvalidValue {
        /// The flag.
        flag: String,
        /// The value given.
        value: String,
    },
    /// One flag of a pair without the other.
    Unpaired {
        /// The flag given.
        given: String,
        /// The flag it needs.
        needs: String,
    },
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownFlag(n) => write!(f, "unknown flag: {n}"),
            Self::MissingValue(n) => write!(f, "flag {n} needs a value"),
            Self::InvalidValue { flag, value } => write!(f, "bad value for {flag}: {value}"),
            Self::Unpaired { given, needs } => write!(f, "{given} needs {needs}"),
        }
    }
}

impl std::error::Error for CliError {}

/// One flag as given: its name, and the value after `=` if there was one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    name: String,
    inline: Option<String>,
}

impl Flag {
    /// The flag without an `=VALUE`, e.g. `--domain`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Walks a command line flag by flag.
#[derive(Debug)]
pub struct Flags<'a> {
    args: &'a [String],
    next: usize,
}

impl<'a> Flags<'a> {
    /// A walk over `args`, the program name not among them.
    #[must_use]
    pub fn new(args: &'a [String]) -> Self {
        Self { args, next: 0 }
    }

    /// The next flag, `None` past the last argument.
    pub fn next_flag(&mut self) -> Option<Flag> {
        let raw = self.args.get(self.next)?;
        self.next += 1;
        Some(match raw.split_once('=') {
            Some((name, value)) => Flag {
                name: name.to_string(),
                inline: Some(value.to_string()),
            },
            None => Flag {
                name: raw.clone(),
                inline: None,
            },
        })
    }

    /// The value of `flag`: what followed its `=`, or else the next
    /// argument, which the walk then passes over.
    ///
    /// # Errors
    /// [`CliError::MissingValue`] when `flag` is the last argument.
    pub fn value(&mut self, flag: &Flag) -> Result<String, CliError> {
        if let Some(v) = &flag.inline {
            return Ok(v.clone());
        }
        let v = self
            .args
            .get(self.next)
            .cloned()
            .ok_or_else(|| CliError::MissingValue(flag.name.clone()))?;
        self.next += 1;
        Ok(v)
    }

    /// The value of `flag`, parsed.
    ///
    /// # Errors
    /// [`CliError::MissingValue`] as [`Self::value`];
    /// [`CliError::InvalidValue`] when the value does not parse.
    pub fn parsed<T: FromStr>(&mut self, flag: &Flag) -> Result<T, CliError> {
        let v = self.value(flag)?;
        v.parse().map_err(|_| CliError::InvalidValue {
            flag: flag.name.clone(),
            value: v,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_value_comes_inline_or_from_the_next_argument() {
        let a = args(&["--a=1", "--b", "2", "--switch"]);
        let mut flags = Flags::new(&a);
        let f = flags.next_flag().unwrap();
        assert_eq!((f.name(), flags.value(&f).unwrap().as_str()), ("--a", "1"));
        let f = flags.next_flag().unwrap();
        assert_eq!(flags.parsed::<u8>(&f).unwrap(), 2);
        assert_eq!(flags.next_flag().unwrap().name(), "--switch");
        assert!(flags.next_flag().is_none());
    }

    #[test]
    fn a_switch_given_a_value_inline_keeps_its_name() {
        let a = args(&["--help=yes"]);
        let mut flags = Flags::new(&a);
        assert_eq!(flags.next_flag().unwrap().name(), "--help");
        assert!(flags.next_flag().is_none());
    }

    #[test]
    fn a_missing_and_an_unparseable_value_name_their_flag() {
        let a = args(&["--n"]);
        let mut flags = Flags::new(&a);
        let f = flags.next_flag().unwrap();
        assert_eq!(
            flags.value(&f).unwrap_err(),
            CliError::MissingValue("--n".into())
        );
        let a = args(&["--n", "x"]);
        let mut flags = Flags::new(&a);
        let f = flags.next_flag().unwrap();
        assert_eq!(
            flags.parsed::<i32>(&f).unwrap_err(),
            CliError::InvalidValue {
                flag: "--n".into(),
                value: "x".into()
            }
        );
    }

    #[test]
    fn the_messages_are_the_daemons_messages() {
        let cases = vec![
            (CliError::UnknownFlag("--x".into()), "unknown flag: --x"),
            (
                CliError::MissingValue("--x".into()),
                "flag --x needs a value",
            ),
            (
                CliError::InvalidValue {
                    flag: "--x".into(),
                    value: "v".into(),
                },
                "bad value for --x: v",
            ),
            (
                CliError::Unpaired {
                    given: "--a".into(),
                    needs: "--b".into(),
                },
                "--a needs --b",
            ),
        ];
        for (e, text) in cases {
            assert_eq!(e.to_string(), text);
        }
    }
}
