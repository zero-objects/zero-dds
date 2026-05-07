// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Public Top-Level-Parser-API (T5.4).
//!
//! Wickelt die Pipeline `Tokenize → Recognize → CST → AST` zu einer
//! ergonomischen Funktion. Default-Konfig:
//!
//! ```
//! let ast = zerodds_idl::parse("module Empty {};", &Default::default())
//!     .expect("parse must succeed");
//! assert_eq!(ast.definitions.len(), 1);
//! ```
//!
//! Die Pipeline ist bewusst simpel — fuer Caching, Recovery oder
//! Inkremental-Parsing wird in Phase 1 ein dedizierter Session-Layer
//! aufgesetzt (RFC 0001 §6).

use crate::ast::{self, Specification};
use crate::config::ParserConfig;
use crate::engine::{Engine, EngineError, Recognizer};
use crate::errors::{ParseError, Span};
use crate::grammar::TokenKind;
use crate::grammar::compile::CompiledGrammar;
use crate::grammar::compose::compose;
use crate::grammar::deltas::GrammarDelta;
use crate::grammar::idl42::IDL_42;
use crate::grammar::validate::validate;
use crate::lexer::{Token, TokenRules, Tokenizer};

/// Maximale `{`-/`}`-Verschachtelungstiefe der Token-Sequenz.
///
/// Schuetzt CST- und AST-Builder vor Stack-Overflow bei adversarial
/// deeply nested IDL-Inputs (TS-1-Finding 1,
/// `docs/test-harness/plan.md`). Der Cap ist deutlich oberhalb
/// realistischer IDL-Bestaende (typische Modul-Hierarchien gehen
/// 4-6 tief; selbst CCM-Component-Stacks bleiben unter 16) und
/// unterhalb der Stack-Frame-Limite des rekursiven CST-Builders im
/// Debug-Build (ab ~128 nested module triggern Test-Threads
/// Stack-Overflow).
pub const MAX_NESTING_DEPTH: usize = 64;

/// Maximale Anzahl aufeinanderfolgender `@`-Annotations vor einer
/// Declaration.
///
/// Die Annotation-Grammar ist linksrekursiv (`seq -> seq appl |
/// empty`); im rekursiven CST-Builder fuehrt das zu quadratischem
/// Verhalten in `try_match_symbols` (TS-1-Finding 2). Realistische
/// IDL-Decls tragen 1-5 Annotations; >64 ist adversarial.
pub const MAX_CONSECUTIVE_ANNOTATIONS: usize = 64;

/// Pre-Tokenization-Validation:
///
/// 1. Zaehlt die maximale `{`-Tiefe — Cap [`MAX_NESTING_DEPTH`]
///    schuetzt CST-Builder vor Stack-Overflow.
/// 2. Zaehlt aufeinanderfolgende `@`-Annotations — Cap
///    [`MAX_CONSECUTIVE_ANNOTATIONS`] schuetzt vor quadratischem
///    Verhalten im linksrekursiven `annotation_appl_seq`.
///
/// Beide Pruefungen laufen vor Engine-Recognize, sodass adversarial
/// Inputs ohne Penalty zurueckgewiesen werden.
fn check_nesting_depth(tokens: &[Token<'_>]) -> Result<(), Error> {
    let mut depth: usize = 0;
    let mut consecutive_at: usize = 0;
    for t in tokens {
        let TokenKind::Punct(p) = t.kind else {
            continue;
        };
        if p == "{" {
            depth += 1;
            consecutive_at = 0;
            if depth > MAX_NESTING_DEPTH {
                return Err(Error::DepthLimit {
                    limit: MAX_NESTING_DEPTH,
                    span: t.span,
                });
            }
        } else if p == "}" {
            depth = depth.saturating_sub(1);
            consecutive_at = 0;
        } else if p == "@" {
            consecutive_at += 1;
            if consecutive_at > MAX_CONSECUTIVE_ANNOTATIONS {
                return Err(Error::AnnotationLimit {
                    limit: MAX_CONSECUTIVE_ANNOTATIONS,
                    span: t.span,
                });
            }
        } else if p == ";" {
            // Ein Semicolon trennt Decl-Sequenzen — Annotations
            // davor gehoeren zur abgeschlossenen Decl.
            consecutive_at = 0;
        }
    }
    Ok(())
}

/// High-Level-Fehler des Top-Level-Parsers.
///
/// Vereint Lexer-, Recognition- und Builder-Fehler unter einem Typ, damit
/// Konsumenten nur eine Error-Variante behandeln muessen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Lexer- oder Recognition-Fehler. Enthaelt den existierenden
    /// [`ParseError`]-Variant fuer Token/EOF-Diagnostik.
    Parse(ParseError),
    /// Grammar-Konstruktions-Fehler. Tritt nur bei korruptem Build der
    /// Grammar-Konstante auf — sollte fuer `IDL_42` nie passieren.
    InvalidGrammar(String),
    /// AST-Builder-Fehler. Indiziert Bug im Builder oder Grammar-Drift.
    AstBuild(ast::BuilderError),
    /// Verwendetes Konstrukt benoetigt ein Feature das in
    /// [`ParserConfig::features`] aus ist. Liste aller Verletzungen.
    FeaturesDisabled(Vec<crate::features::gate::FeatureGateError>),
    /// `{`-/`}`-Verschachtelung ueberschreitet [`MAX_NESTING_DEPTH`].
    /// Schutz vor Stack-Overflow im rekursiven CST-Builder
    /// (TS-1-Finding 1).
    DepthLimit {
        /// Aktueller Cap.
        limit: usize,
        /// Position des `{`-Tokens, das den Cap ueberschritten hat.
        span: Span,
    },
    /// Aufeinanderfolgende `@`-Annotations ueberschreiten
    /// [`MAX_CONSECUTIVE_ANNOTATIONS`]. Schutz vor quadratischem
    /// Verhalten im linksrekursiven Annotation-Sequence-CST-Build
    /// (TS-1-Finding 2).
    AnnotationLimit {
        /// Aktueller Cap.
        limit: usize,
        /// Position des `@`-Tokens, das den Cap ueberschritten hat.
        span: Span,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse error: {e:?}"),
            Self::InvalidGrammar(msg) => write!(f, "invalid grammar: {msg}"),
            Self::AstBuild(e) => write!(f, "ast build error: {e}"),
            Self::FeaturesDisabled(errs) => {
                writeln!(f, "{} feature-gate violation(s):", errs.len())?;
                for e in errs {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
            Self::DepthLimit { limit, span } => {
                write!(
                    f,
                    "brace nesting exceeds {limit} at {span} — refusing to parse to protect \
                     against stack overflow"
                )
            }
            Self::AnnotationLimit { limit, span } => {
                write!(
                    f,
                    "more than {limit} consecutive annotations at {span} — refusing to parse \
                     to protect against quadratic CST-build cost"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

impl From<ast::BuilderError> for Error {
    fn from(e: ast::BuilderError) -> Self {
        Self::AstBuild(e)
    }
}

/// Parst IDL-Source zu einer typisierten [`Specification`].
///
/// Pipeline: Tokenize → Earley-Recognize → CST-Build → AST-Build.
///
/// In Phase 0 wird `cfg.version`, `cfg.compat` und `cfg.vendor`
/// noch nicht ausgewertet — die Grammar ist hardgecodet [`IDL_42`].
/// Mit T6.x werden Versions-/Compat-/Vendor-Deltas wirksam (siehe
/// [`crate::config`]).
///
/// # Errors
/// - [`Error::Parse`]: Lexer-Fehler, Token-Mismatch, oder die Grammar
///   akzeptiert die Token-Sequenz nicht.
/// - [`Error::InvalidGrammar`]: Sollte nur bei korrupter Grammar-
///   Konstante auftreten.
/// - [`Error::AstBuild`]: CST-Struktur weicht von Grammar ab — Bug.
pub fn parse(src: &str, cfg: &ParserConfig) -> Result<Specification, Error> {
    let tokenizer = Tokenizer::for_grammar(&IDL_42);
    let stream = tokenizer.tokenize(src).map_err(Error::Parse)?;
    check_nesting_depth(stream.tokens())?;
    let engine = Engine::new(&IDL_42);
    let result = match engine.recognize(stream.tokens()) {
        Ok(r) => r,
        Err(EngineError::InvalidGrammar(report)) => {
            return Err(Error::InvalidGrammar(format!(
                "{} validation issues",
                report.errors().count()
            )));
        }
        Err(EngineError::NotAccepted { last_consumed }) => {
            return Err(Error::Parse(ParseError::UnexpectedToken {
                found: stream
                    .tokens()
                    .get(last_consumed)
                    .map(|t| t.kind)
                    .unwrap_or(crate::grammar::TokenKind::Ident),
                expected: Vec::new(),
                span: stream
                    .tokens()
                    .get(last_consumed)
                    .map(|t| t.span)
                    .unwrap_or(Span::SYNTHETIC),
            }));
        }
    };
    let cst = crate::cst::build_cst(engine.compiled_grammar(), stream.tokens(), &result)
        .ok_or_else(|| {
            Error::AstBuild(ast::BuilderError {
                message: "CST reconstruction failed (recognition succeeded but tree invalid)"
                    .to_string(),
                span: Span::SYNTHETIC,
            })
        })?;
    // Feature-Gate-Pass: lehne Konstrukte ab deren Feature in
    // `cfg.features` aus ist. Bei Violations: alle gesammelt liefern.
    let gate_errors = crate::features::gate::validate(&cst, &cfg.features);
    if !gate_errors.is_empty() {
        return Err(Error::FeaturesDisabled(gate_errors));
    }
    let ast = ast::build(&cst)?;
    Ok(ast)
}

/// Wie [`parse`], aber mit zusaetzlichen Vendor-Grammar-Deltas (T6.5).
///
/// Komposition: Base-Grammar `IDL_42` + Deltas → [`CompiledGrammar`].
/// Tokenizer-Rules werden aus der composed Grammar abgeleitet, damit
/// Vendor-spezifische Keywords/Punctuation erkannt werden.
///
/// # Beispiel
/// ```
/// use zerodds_idl::config::ParserConfig;
/// use zerodds_idl::grammar::deltas::RTI_CONNEXT;
/// use zerodds_idl::parser::parse_with_deltas;
///
/// let src = r"
///     struct Sensor { long id; double value; };
///     keylist Sensor (id);
/// ";
/// let ast = parse_with_deltas(src, &ParserConfig::default(), &[&RTI_CONNEXT])
///     .expect("RTI delta accepts keylist");
/// assert_eq!(ast.definitions.len(), 2);
/// ```
///
/// # Errors
/// Wie [`parse`].
pub fn parse_with_deltas(
    src: &str,
    cfg: &ParserConfig,
    deltas: &[&GrammarDelta],
) -> Result<Specification, Error> {
    let _ = cfg; // aktuell ungenutzt.
    let composed: CompiledGrammar = compose(&IDL_42, deltas);

    // Token-Rules aus composed Grammar — damit Vendor-Keywords erkannt
    // werden.
    let rules = TokenRules::from_productions(composed.productions_iter());
    let tokenizer = Tokenizer::new(rules);
    let stream = tokenizer.tokenize(src).map_err(Error::Parse)?;
    check_nesting_depth(stream.tokens())?;

    // Validation auf der Base-Grammar (Delta-Validation kommt mit T6.9).
    let base_report = validate(&IDL_42);
    if base_report.has_errors() {
        return Err(Error::InvalidGrammar(format!(
            "{} validation issues",
            base_report.errors().count()
        )));
    }

    let result = Recognizer::new(&composed).recognize(stream.tokens());
    if !result.accepted {
        let last = stream.tokens().len();
        return Err(Error::Parse(ParseError::UnexpectedToken {
            found: stream
                .tokens()
                .get(last)
                .map(|t| t.kind)
                .unwrap_or(crate::grammar::TokenKind::Ident),
            expected: Vec::new(),
            span: stream
                .tokens()
                .get(last)
                .map(|t| t.span)
                .unwrap_or(Span::SYNTHETIC),
        }));
    }

    let cst = crate::cst::build_cst(&composed, stream.tokens(), &result).ok_or_else(|| {
        Error::AstBuild(ast::BuilderError {
            message: "CST reconstruction failed (composed grammar)".to_string(),
            span: Span::SYNTHETIC,
        })
    })?;
    ast::build(&cst).map_err(Error::AstBuild)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::*;
    use crate::ast::{Definition, TypeDecl};

    #[test]
    fn parse_empty_module_with_default_config() {
        let ast = parse("module Empty {};", &ParserConfig::default()).expect("parse");
        assert_eq!(ast.definitions.len(), 1);
        assert!(matches!(ast.definitions[0], Definition::Module(_)));
    }

    #[test]
    fn parse_struct_with_members() {
        let ast = parse(
            "struct Point { long x; long y; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        assert_eq!(ast.definitions.len(), 1);
        assert!(matches!(
            ast.definitions[0],
            Definition::Type(TypeDecl::Constr(_))
        ));
    }

    #[test]
    fn parse_returns_error_for_lex_failure() {
        let res = parse("\u{4711} not idl", &ParserConfig::default());
        assert!(matches!(res, Err(Error::Parse(_))));
    }

    #[test]
    fn parse_returns_error_for_grammar_rejection() {
        let res = parse("struct Foo {", &ParserConfig::default());
        assert!(matches!(res, Err(Error::Parse(_))));
    }

    #[test]
    fn parse_with_pragmatic_config_parses_empty_struct_member_list() {
        // Vendor-pragmatisch: leere member_list zugelassen.
        let ast = parse("struct Empty {};", &ParserConfig::pragmatic_4_2()).expect("parse");
        assert_eq!(ast.definitions.len(), 1);
    }

    #[test]
    fn parse_handles_complex_dds_topic_pattern() {
        let src = r#"
            @topic
            @appendable
            struct Sensor {
                @key long sensor_id;
                double value;
                @optional string label;
            };
        "#;
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        assert_eq!(ast.definitions.len(), 1);
    }

    #[test]
    fn parse_with_rti_delta_accepts_keylist() {
        use crate::grammar::deltas::RTI_CONNEXT;
        let src = r"
            struct Sensor { long id; double value; };
            keylist Sensor (id);
        ";
        let result = parse_with_deltas(src, &ParserConfig::default(), &[&RTI_CONNEXT]);
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn parse_without_rti_delta_rejects_keylist() {
        let src = r"
            struct Sensor { long id; double value; };
            keylist Sensor (id);
        ";
        let result = parse(src, &ParserConfig::default());
        assert!(
            matches!(result, Err(Error::Parse(_))),
            "expected parse error, got {result:?}"
        );
    }

    #[test]
    fn parse_with_rti_delta_accepts_multi_field_keylist() {
        use crate::grammar::deltas::RTI_CONNEXT;
        let src = r"
            struct Coord { long x; long y; long z; };
            keylist Coord (x, y, z);
        ";
        let result = parse_with_deltas(src, &ParserConfig::default(), &[&RTI_CONNEXT]);
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn error_display_does_not_panic() {
        let err = Error::AstBuild(ast::BuilderError {
            message: "x".to_string(),
            span: Span::new(0, 1),
        });
        let s = format!("{err}");
        assert!(s.contains("ast build error"));
    }
}
