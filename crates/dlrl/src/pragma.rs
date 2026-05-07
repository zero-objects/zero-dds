// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DLRL-Pragma-Parser — DDS 1.4 §B.4.1.
//!
//! Spec §B.4.1 (S. 211): drei Pragmas markieren IDL-Konstrukte fuer
//! den DLRL-Codegen:
//!
//! ```text
//! #pragma DCPS_DATA_TYPE  "<scoped-name>"
//! #pragma DCPS_DATA_KEY   "<scoped-name> <field>"
//! #pragma DCPS_DLRL_RELATION "<scoped-name> <relation> <target>"
//! ```

use alloc::string::String;

/// Geparste DLRL-Pragma-Variante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlrlPragma {
    /// `#pragma DCPS_DATA_TYPE "<scoped-name>"` — markiert ein
    /// `struct` als DLRL-Object-Type.
    DataType {
        /// Vollqualifizierter Type-Name.
        name: String,
    },
    /// `#pragma DCPS_DATA_KEY "<scoped-name> <field>"` — markiert
    /// ein Feld als DLRL-Object-Key.
    DataKey {
        /// Type-Name.
        type_name: String,
        /// Feld-Name.
        field: String,
    },
    /// `#pragma DCPS_DLRL_RELATION "<scoped-name> <relation> <target>"` —
    /// definiert eine Relationship.
    DlrlRelation {
        /// Source-Type-Name.
        type_name: String,
        /// Relation-Name.
        relation: String,
        /// Target-Type-Name.
        target: String,
    },
}

/// Parser-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsePragmaError {
    /// Line ist kein DLRL-Pragma.
    NotDlrlPragma,
    /// Tag ist unbekannt.
    UnknownTag(String),
    /// Quoted-String fehlt oder ist nicht abgeschlossen.
    MalformedQuotedString,
    /// Quote-Inhalt hat falsche Token-Anzahl (z.B. `DCPS_DATA_KEY` braucht 2 Tokens).
    WrongArity {
        /// Tag.
        tag: String,
        /// Erwartete Anzahl Tokens im Quote-String.
        expected: usize,
        /// Tatsaechliche Anzahl.
        actual: usize,
    },
}

impl core::fmt::Display for ParsePragmaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotDlrlPragma => f.write_str("not a DLRL pragma line"),
            Self::UnknownTag(t) => write!(f, "unknown DLRL pragma tag `{t}`"),
            Self::MalformedQuotedString => f.write_str("malformed quoted string"),
            Self::WrongArity {
                tag,
                expected,
                actual,
            } => write!(f, "pragma `{tag}` expects {expected} tokens, got {actual}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParsePragmaError {}

/// Parst eine Source-Line in einen [`DlrlPragma`].
///
/// # Errors
/// Siehe [`ParsePragmaError`].
pub fn parse_pragma(line: &str) -> Result<DlrlPragma, ParsePragmaError> {
    let trimmed = line.trim();
    let after_hash = trimmed
        .strip_prefix('#')
        .ok_or(ParsePragmaError::NotDlrlPragma)?
        .trim_start();
    let after_pragma = after_hash
        .strip_prefix("pragma")
        .ok_or(ParsePragmaError::NotDlrlPragma)?
        .trim_start();
    // Tag — bis Whitespace.
    let tag_end = after_pragma
        .find(char::is_whitespace)
        .unwrap_or(after_pragma.len());
    let tag = &after_pragma[..tag_end];
    let rest = after_pragma[tag_end..].trim_start();
    let inner = parse_quoted(rest)?;
    let tokens: alloc::vec::Vec<&str> = inner.split_whitespace().collect();
    match tag {
        "DCPS_DATA_TYPE" => {
            if tokens.len() != 1 {
                return Err(ParsePragmaError::WrongArity {
                    tag: tag.into(),
                    expected: 1,
                    actual: tokens.len(),
                });
            }
            Ok(DlrlPragma::DataType {
                name: tokens[0].into(),
            })
        }
        "DCPS_DATA_KEY" => {
            if tokens.len() != 2 {
                return Err(ParsePragmaError::WrongArity {
                    tag: tag.into(),
                    expected: 2,
                    actual: tokens.len(),
                });
            }
            Ok(DlrlPragma::DataKey {
                type_name: tokens[0].into(),
                field: tokens[1].into(),
            })
        }
        "DCPS_DLRL_RELATION" => {
            if tokens.len() != 3 {
                return Err(ParsePragmaError::WrongArity {
                    tag: tag.into(),
                    expected: 3,
                    actual: tokens.len(),
                });
            }
            Ok(DlrlPragma::DlrlRelation {
                type_name: tokens[0].into(),
                relation: tokens[1].into(),
                target: tokens[2].into(),
            })
        }
        _ => Err(ParsePragmaError::UnknownTag(tag.into())),
    }
}

fn parse_quoted(s: &str) -> Result<&str, ParsePragmaError> {
    let stripped = s
        .strip_prefix('"')
        .ok_or(ParsePragmaError::MalformedQuotedString)?;
    let end = stripped
        .find('"')
        .ok_or(ParsePragmaError::MalformedQuotedString)?;
    Ok(&stripped[..end])
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_data_type_pragma() {
        let p = parse_pragma(r#"#pragma DCPS_DATA_TYPE "demo::Trade""#).unwrap();
        assert_eq!(
            p,
            DlrlPragma::DataType {
                name: "demo::Trade".into()
            }
        );
    }

    #[test]
    fn parses_data_key_pragma() {
        let p = parse_pragma(r#"#pragma DCPS_DATA_KEY "demo::Trade symbol""#).unwrap();
        assert_eq!(
            p,
            DlrlPragma::DataKey {
                type_name: "demo::Trade".into(),
                field: "symbol".into(),
            }
        );
    }

    #[test]
    fn parses_relation_pragma() {
        let p =
            parse_pragma(r#"#pragma DCPS_DLRL_RELATION "demo::Order trades demo::Trade""#).unwrap();
        assert_eq!(
            p,
            DlrlPragma::DlrlRelation {
                type_name: "demo::Order".into(),
                relation: "trades".into(),
                target: "demo::Trade".into(),
            }
        );
    }

    #[test]
    fn non_dlrl_line_rejected() {
        assert_eq!(
            parse_pragma("// just a comment"),
            Err(ParsePragmaError::NotDlrlPragma)
        );
    }

    #[test]
    fn unknown_tag_rejected() {
        let err = parse_pragma(r#"#pragma DCPS_NEWTAG "foo""#).unwrap_err();
        assert!(matches!(err, ParsePragmaError::UnknownTag(_)));
    }

    #[test]
    fn data_type_with_extra_tokens_rejected() {
        let err = parse_pragma(r#"#pragma DCPS_DATA_TYPE "a b""#).unwrap_err();
        assert!(matches!(
            err,
            ParsePragmaError::WrongArity {
                expected: 1,
                actual: 2,
                ..
            }
        ));
    }

    #[test]
    fn data_key_with_one_token_rejected() {
        let err = parse_pragma(r#"#pragma DCPS_DATA_KEY "demo::Trade""#).unwrap_err();
        assert!(matches!(
            err,
            ParsePragmaError::WrongArity {
                expected: 2,
                actual: 1,
                ..
            }
        ));
    }

    #[test]
    fn missing_quotes_rejected() {
        let err = parse_pragma("#pragma DCPS_DATA_TYPE foo").unwrap_err();
        assert_eq!(err, ParsePragmaError::MalformedQuotedString);
    }

    #[test]
    fn extra_whitespace_tolerated() {
        let p = parse_pragma(r#"   #   pragma   DCPS_DATA_TYPE   "demo::Trade"  "#).unwrap();
        assert_eq!(
            p,
            DlrlPragma::DataType {
                name: "demo::Trade".into()
            }
        );
    }
}
