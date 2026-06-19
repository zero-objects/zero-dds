// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `#pragma ami4ccm` parser — Spec §7.7.
//!
//! Spec §7.7 (p. 12) defines two pragma forms:
//! ```text
//! #pragma ami4ccm interface "<fully qualified interface name>"
//! #pragma ami4ccm receptacle "<fully qualified receptacle name>"
//! ```
//!
//! Both pragmas are markers for the AMI4CCM-aware IDL compiler:
//! * `interface` — marks an interface definition as AMI4CCM-enabled,
//!   i.e. the implied IDL interfaces (`AMI4CCM_<Iface>` +
//!   `AMI4CCM_<Iface>ReplyHandler`) are generated for it.
//! * `receptacle` — marks a component receptacle as AMI4CCM-enabled,
//!   i.e. the component context gains an additional method
//!   `get_connection_sendc_<recep>()` (Spec §7.7, p. 13).
//!
//! These pragmas are **textual** in the IDL file and appear before the
//! actual `interface`/`component` definition. We parse them from a
//! source line.

use alloc::string::String;
use core::fmt;

/// Parsed AMI4CCM pragma variant (Spec §7.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ami4CcmPragma {
    /// `#pragma ami4ccm interface "<fully qualified interface name>"`.
    Interface {
        /// Fully-qualified name (e.g. `Stock::StockManager` or
        /// `StockManager`).
        name: String,
    },
    /// `#pragma ami4ccm receptacle "<fully qualified receptacle name>"`.
    Receptacle {
        /// Fully-qualified name `Component::receptacle` (e.g.
        /// `Client::manager`).
        name: String,
    },
}

/// Parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsePragmaError {
    /// Line is not a `#pragma ami4ccm`.
    NotAmi4ccmPragma,
    /// Tag is neither `interface` nor `receptacle`.
    UnknownTag(String),
    /// Quoted string is missing or unterminated.
    MalformedQuotedName,
    /// Empty name inside the quotes.
    EmptyName,
}

impl fmt::Display for ParsePragmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAmi4ccmPragma => f.write_str("not a `#pragma ami4ccm` line"),
            Self::UnknownTag(tag) => write!(f, "unknown ami4ccm tag `{tag}`"),
            Self::MalformedQuotedName => f.write_str("malformed quoted name"),
            Self::EmptyName => f.write_str("empty name in quotes"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParsePragmaError {}

/// Parses a source line into an [`Ami4CcmPragma`].
///
/// Accepts whitespace variations and ignores trailing whitespace.
/// Lines that do not start with `#pragma ami4ccm` return
/// [`ParsePragmaError::NotAmi4ccmPragma`] (so the caller can simply
/// skip non-AMI4CCM pragmas).
///
/// # Errors
/// See [`ParsePragmaError`].
pub fn parse_pragma(line: &str) -> Result<Ami4CcmPragma, ParsePragmaError> {
    let trimmed = line.trim();
    let after_hash = trimmed
        .strip_prefix('#')
        .ok_or(ParsePragmaError::NotAmi4ccmPragma)?
        .trim_start();
    let after_pragma = after_hash
        .strip_prefix("pragma")
        .ok_or(ParsePragmaError::NotAmi4ccmPragma)?
        .trim_start();
    let after_ami = after_pragma
        .strip_prefix("ami4ccm")
        .ok_or(ParsePragmaError::NotAmi4ccmPragma)?
        .trim_start();
    // Tag (interface / receptacle) — alphabetic, up to whitespace.
    let tag_end = after_ami
        .find(char::is_whitespace)
        .unwrap_or(after_ami.len());
    let tag = &after_ami[..tag_end];
    let rest = after_ami[tag_end..].trim_start();
    let name = parse_quoted(rest)?;
    match tag {
        "interface" => Ok(Ami4CcmPragma::Interface {
            name: String::from(name),
        }),
        "receptacle" => Ok(Ami4CcmPragma::Receptacle {
            name: String::from(name),
        }),
        other => Err(ParsePragmaError::UnknownTag(String::from(other))),
    }
}

fn parse_quoted(s: &str) -> Result<&str, ParsePragmaError> {
    let s = s.trim_end();
    let inner = s
        .strip_prefix('"')
        .ok_or(ParsePragmaError::MalformedQuotedName)?
        .strip_suffix('"')
        .ok_or(ParsePragmaError::MalformedQuotedName)?;
    if inner.is_empty() {
        return Err(ParsePragmaError::EmptyName);
    }
    Ok(inner)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_interface_pragma() {
        // Spec §7.7 example.
        let p = parse_pragma(r#"#pragma ami4ccm interface "StockManager""#).expect("valid pragma");
        assert_eq!(
            p,
            Ami4CcmPragma::Interface {
                name: String::from("StockManager")
            }
        );
    }

    #[test]
    fn parses_receptacle_pragma() {
        // Spec §7.7 example — receptacle with component scope.
        let p =
            parse_pragma(r#"#pragma ami4ccm receptacle "Client::manager""#).expect("valid pragma");
        assert_eq!(
            p,
            Ami4CcmPragma::Receptacle {
                name: String::from("Client::manager")
            }
        );
    }

    #[test]
    fn parses_pragma_with_extra_whitespace() {
        let p = parse_pragma("  #  pragma  ami4ccm  interface  \"Foo::Bar\"  ").expect("valid");
        assert_eq!(
            p,
            Ami4CcmPragma::Interface {
                name: String::from("Foo::Bar")
            }
        );
    }

    #[test]
    fn rejects_non_ami_pragma() {
        let err = parse_pragma("#pragma prefix Foo").unwrap_err();
        assert_eq!(err, ParsePragmaError::NotAmi4ccmPragma);
    }

    #[test]
    fn rejects_non_pragma_line() {
        let err = parse_pragma("interface Foo {};").unwrap_err();
        assert_eq!(err, ParsePragmaError::NotAmi4ccmPragma);
    }

    #[test]
    fn rejects_unknown_tag() {
        let err = parse_pragma(r#"#pragma ami4ccm component "Foo""#).unwrap_err();
        assert_eq!(err, ParsePragmaError::UnknownTag(String::from("component")));
    }

    #[test]
    fn rejects_unquoted_name() {
        let err = parse_pragma("#pragma ami4ccm interface Foo").unwrap_err();
        assert_eq!(err, ParsePragmaError::MalformedQuotedName);
    }

    #[test]
    fn rejects_empty_quoted_name() {
        let err = parse_pragma(r#"#pragma ami4ccm interface """#).unwrap_err();
        assert_eq!(err, ParsePragmaError::EmptyName);
    }

    #[test]
    fn display_error_describes_each_variant() {
        assert!(alloc::format!("{}", ParsePragmaError::NotAmi4ccmPragma).contains("ami4ccm"));
        assert!(
            alloc::format!("{}", ParsePragmaError::UnknownTag(String::from("x"))).contains('x')
        );
        assert!(alloc::format!("{}", ParsePragmaError::MalformedQuotedName).contains("malformed"));
        assert!(alloc::format!("{}", ParsePragmaError::EmptyName).contains("empty"));
    }
}
