// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Public top-level parser API (T5.4).
//!
//! Wraps the pipeline `Tokenize → Recognize → CST → AST` into an
//! ergonomic function. Default config:
//!
//! ```
//! let ast = zerodds_idl::parse("module Empty {};", &Default::default())
//!     .expect("parse must succeed");
//! assert_eq!(ast.definitions.len(), 1);
//! ```
//!
//! The pipeline is deliberately simple — for caching, recovery or
//! incremental parsing a dedicated session layer is set up in phase 1
//! (RFC 0001 §6).

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

/// Maximum `{`/`}` nesting depth of the token sequence.
///
/// Protects the CST and AST builders from stack overflow on adversarial
/// deeply nested IDL inputs (TS-1 finding 1,
/// `docs/test-harness/plan.md`). The cap is well above
/// realistic IDL bodies (typical module hierarchies go
/// 4-6 deep; even CCM component stacks stay under 16) and
/// below the stack-frame limit of the recursive CST builder in the
/// debug build (from ~128 nested modules, test threads trigger
/// stack overflow).
pub const MAX_NESTING_DEPTH: usize = 64;

/// Maximum number of consecutive `@` annotations before a
/// declaration.
///
/// The annotation grammar is left-recursive (`seq -> seq appl |
/// empty`); in the recursive CST builder this leads to quadratic
/// behavior in `try_match_symbols` (TS-1 finding 2). Realistic
/// IDL decls carry 1-5 annotations; >64 is adversarial.
pub const MAX_CONSECUTIVE_ANNOTATIONS: usize = 64;

/// Pre-Tokenization-Validation:
///
/// 1. Counts the maximum `{` depth — cap [`MAX_NESTING_DEPTH`]
///    protects the CST builder from stack overflow.
/// 2. Counts consecutive `@` annotations — cap
///    [`MAX_CONSECUTIVE_ANNOTATIONS`] protects from quadratic
///    behavior in the left-recursive `annotation_appl_seq`.
///
/// Both checks run before engine recognize, so adversarial
/// inputs are rejected without penalty.
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
            // A semicolon separates decl sequences — annotations
            // before it belong to the completed decl.
            consecutive_at = 0;
        }
    }
    Ok(())
}

/// High-level error of the top-level parser.
///
/// Unifies lexer, recognition and builder errors under one type, so that
/// consumers only have to handle one error variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Lexer or recognition error. Contains the existing
    /// [`ParseError`] variant for token/EOF diagnostics.
    Parse(ParseError),
    /// Preprocessor error (`#include`, `#define`, `#error`, …). Only produced by the
    /// bundled [`parse_source`] function.
    Preprocess(crate::preprocessor::PreprocessError),
    /// Grammar construction error. Occurs only on a corrupt build of the
    /// grammar constant — should never happen for `IDL_42`.
    InvalidGrammar(String),
    /// AST builder error. Indicates a bug in the builder or grammar drift.
    AstBuild(ast::BuilderError),
    /// A used construct requires a feature that is off in
    /// [`ParserConfig::features`]. List of all violations.
    FeaturesDisabled(Vec<crate::features::gate::FeatureGateError>),
    /// `{`/`}` nesting exceeds [`MAX_NESTING_DEPTH`].
    /// Protection against stack overflow in the recursive CST builder
    /// (TS-1 finding 1).
    DepthLimit {
        /// Current cap.
        limit: usize,
        /// Position of the `{` token that exceeded the cap.
        span: Span,
    },
    /// Consecutive `@` annotations exceed
    /// [`MAX_CONSECUTIVE_ANNOTATIONS`]. Protection against quadratic
    /// behavior in the left-recursive annotation-sequence CST build
    /// (TS-1 finding 2).
    AnnotationLimit {
        /// Current cap.
        limit: usize,
        /// Position of the `@` token that exceeded the cap.
        span: Span,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse error: {e:?}"),
            Self::Preprocess(e) => write!(f, "preprocess error: {e}"),
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

/// Parses IDL source into a typed [`Specification`].
///
/// Pipeline: Tokenize → Earley-Recognize → CST-Build → AST-Build.
///
/// In phase 0, `cfg.version`, `cfg.compat` and `cfg.vendor`
/// are not yet evaluated — the grammar is hard-coded [`IDL_42`].
/// With T6.x, version/compat/vendor deltas take effect (see
/// [`crate::config`]).
///
/// # Errors
/// - [`Error::Parse`]: lexer error, token mismatch, or the grammar
///   does not accept the token sequence.
/// - [`Error::InvalidGrammar`]: should only occur on a corrupt grammar
///   constant.
/// - [`Error::AstBuild`]: CST structure deviates from the grammar — a bug.
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
    // Feature-gate pass: reject constructs whose feature is off in
    // `cfg.features`. On violations: return all of them collected.
    let gate_errors = crate::features::gate::validate(&cst, &cfg.features);
    if !gate_errors.is_empty() {
        return Err(Error::FeaturesDisabled(gate_errors));
    }
    let ast = ast::build(&cst)?;
    Ok(ast)
}

/// Full front-end path for an IDL source: preprocessor + parser +
/// key-pragma application in a single call.
///
/// In contrast to [`parse`] (which expects pre-processed input and does not
/// see pragmas), here the preprocessor runs first — incl. `#include` via
/// the `resolver` —, then the parser, and finally
/// [`apply_key_pragmas`](crate::semantics::apply_key_pragmas), which
/// translates `#pragma keylist` / `#pragma DCPS_DATA_KEY` / `#pragma cats` and the
/// RTI `keylist` VendorExtension into synthetic `@key` annotations.
/// This makes pragma-keyed fields downstream identical to
/// inline `@key`.
///
/// For sources without `#include`, a
/// [`MemoryResolver`](crate::preprocessor::MemoryResolver) suffices.
///
/// # Errors
/// [`Error::Preprocess`] on preprocessor errors, otherwise as [`parse`].
pub fn parse_source<R: crate::preprocessor::Resolver>(
    file_name: &str,
    src: &str,
    resolver: R,
    cfg: &ParserConfig,
) -> Result<Specification, Error> {
    let processed = crate::preprocessor::Preprocessor::new(resolver)
        .process(file_name, src)
        .map_err(Error::Preprocess)?;
    let mut spec = parse(&processed.expanded, cfg)?;
    crate::semantics::apply_key_pragmas(&mut spec, &processed);
    Ok(spec)
}

/// Like [`parse`], but with additional vendor grammar deltas (T6.5).
///
/// Composition: base grammar `IDL_42` + deltas → [`CompiledGrammar`].
/// Tokenizer rules are derived from the composed grammar so that
/// vendor-specific keywords/punctuation are recognized.
///
/// # Example
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
/// As [`parse`].
pub fn parse_with_deltas(
    src: &str,
    cfg: &ParserConfig,
    deltas: &[&GrammarDelta],
) -> Result<Specification, Error> {
    let _ = cfg; // currently unused.
    let composed: CompiledGrammar = compose(&IDL_42, deltas);

    // Token rules from the composed grammar — so that vendor keywords are
    // recognized.
    let rules = TokenRules::from_productions(composed.productions_iter());
    let tokenizer = Tokenizer::new(rules);
    let stream = tokenizer.tokenize(src).map_err(Error::Parse)?;
    check_nesting_depth(stream.tokens())?;

    // Validation on the base grammar (delta validation comes with T6.9).
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
        // Vendor-pragmatic: empty member_list allowed.
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
