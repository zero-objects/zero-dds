// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Diagnostics. Spec §7.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use zerodds_idl_cst::Span;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// The file is not valid IDL. Parsing continues anyway.
    Error,
    /// Accepted, but worth saying.
    Warning,
}

/// A stable identifier for a diagnostic.
///
/// Stable across releases on purpose: a build that greps for a code has
/// to keep working. Codes are appended, never renumbered or reused.
/// Spec §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// A token appeared where the grammar did not allow it.
    UnexpectedToken,
    /// A token the grammar requires was not there.
    MissingToken,
    /// A declaration ran out before it was complete.
    UnterminatedDeclaration,
    /// The lexer could not make sense of a range.
    InvalidToken,
    /// A construct only a vendor dialect admits, under the strict
    /// dialect.
    VendorExtension,
    /// A construct the grammar has but this parser does not implement
    /// yet. Reported rather than skipped, so a gap cannot pass for
    /// acceptance.
    Unsupported,
    /// An annotation was written with an argument the specification does
    /// not define for it.
    InvalidAnnotation,
    /// A name is declared twice in one scope, case-insensitively.
    DuplicateName,
    /// A scoped name resolves to more than one declaration.
    AmbiguousName,
    /// A name resolves to nothing.
    UnresolvedName,
    /// A union discriminator is not of a type IDL allows.
    InvalidDiscriminator,
    /// A case label repeats, or lies outside the discriminator's range.
    InvalidCaseLabel,
    /// Two annotations on one node contradict each other.
    ConflictingAnnotations,
    /// A bound, dimension or precision is outside its legal range.
    InvalidBound,
    /// A forward declaration was never defined.
    UndefinedForward,
    /// Two members of one aggregate resolve to the same id.
    DuplicateMemberId,
    /// An identifier differs from a keyword only in case, which
    /// OMG IDL 4.2 §7.2.4 makes illegal.
    KeywordCollision,
    /// A choice the specification permits and something downstream pays
    /// for.
    ///
    /// Never produced by the parser or by lowering — both answer "is
    /// this IDL", and the answer here is always yes. It is the code
    /// `zerodds-idlc lint` reports under, so a rule about a *choice* is
    /// distinguishable from a rule about the language, by code and not
    /// by which tool printed it.
    PolicyAdvice,
    /// A value is not of the type it was declared with.
    ///
    /// IDL 4.2 §7.4.1.4.3 requires a constant's value expression to be
    /// "consistent with the type declared for the constant", and
    /// §7.2.6.3 states the wide/narrow string pair as a "compile-time
    /// diagnostic" in so many words.
    TypeMismatch,
    /// A `#error` directive was reached.
    ///
    /// ISO/IEC 14882:2003 §16.5, which IDL 4.2 §7.3 adopts: "#error
    /// causes the implementation to produce a diagnostic message that
    /// includes the specified sequence of preprocessing tokens." Its own
    /// code because it is the one diagnostic the *source* asked for.
    ErrorDirective,
    /// A derived type breaks a rule inheritance places on it.
    ///
    /// DDS-XTypes 1.3 §7.2.2.4.5: a derived structure has its base's
    /// extensibility or none of its own, and defines no key fields — its
    /// keys are its root base's. Duplicate names and member ids across the
    /// hierarchy keep their own codes, [`Self::DuplicateName`] and
    /// [`Self::DuplicateMemberId`].
    InvalidInheritance,
    /// A rule of the preprocessor ISO/IEC 14882:2003 §16 describes, which
    /// IDL 4.2 §7.3 adopts: a macro invoked with the wrong number of
    /// arguments, a `#` not followed by a parameter, a `##` at either
    /// end of a replacement list, a paste that does not form one token,
    /// an `#if` expression that does not evaluate. As a warning: an
    /// identifier in an `#if` that names no macro, which §16.1 evaluates
    /// as 0.
    Preprocessing,
}

impl DiagnosticCode {
    /// The greppable form, `IDL0001` and so on.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedToken => "IDL0001",
            Self::MissingToken => "IDL0002",
            Self::UnterminatedDeclaration => "IDL0003",
            Self::InvalidToken => "IDL0004",
            Self::VendorExtension => "IDL0005",
            Self::Unsupported => "IDL0006",
            Self::InvalidAnnotation => "IDL0007",
            Self::DuplicateName => "IDL0008",
            Self::AmbiguousName => "IDL0009",
            Self::UnresolvedName => "IDL0010",
            Self::InvalidDiscriminator => "IDL0011",
            Self::InvalidCaseLabel => "IDL0012",
            Self::ConflictingAnnotations => "IDL0013",
            Self::InvalidBound => "IDL0014",
            Self::UndefinedForward => "IDL0015",
            Self::DuplicateMemberId => "IDL0016",
            Self::KeywordCollision => "IDL0017",
            Self::PolicyAdvice => "IDL0018",
            Self::TypeMismatch => "IDL0019",
            Self::ErrorDirective => "IDL0020",
            Self::InvalidInheritance => "IDL0021",
            Self::Preprocessing => "IDL0022",
        }
    }

    /// Every code, so a test can assert the set is collision-free.
    pub const ALL: [Self; 22] = [
        Self::UnexpectedToken,
        Self::MissingToken,
        Self::UnterminatedDeclaration,
        Self::InvalidToken,
        Self::VendorExtension,
        Self::Unsupported,
        Self::InvalidAnnotation,
        Self::DuplicateName,
        Self::AmbiguousName,
        Self::UnresolvedName,
        Self::InvalidDiscriminator,
        Self::InvalidCaseLabel,
        Self::ConflictingAnnotations,
        Self::InvalidBound,
        Self::UndefinedForward,
        Self::DuplicateMemberId,
        Self::KeywordCollision,
        Self::PolicyAdvice,
        Self::TypeMismatch,
        Self::ErrorDirective,
        Self::InvalidInheritance,
        Self::Preprocessing,
    ];
}

/// One thing wrong with the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// How serious.
    pub severity: Severity,
    /// The stable code.
    pub code: DiagnosticCode,
    /// What is wrong, in a sentence.
    pub message: String,
    /// Where it is.
    pub primary: Span,
    /// Further places worth pointing at, with a note each.
    pub labels: Vec<(Span, String)>,
    /// The file `primary` and `labels` point into, when that is not the
    /// unit itself — an included file. `None` is the unit.
    ///
    /// The spans index the file's own text once a caller has mapped
    /// them out of the preprocessed stream; see
    /// `Preprocessed::locate`.
    pub file: Option<String>,
    /// The paragraph the rule comes from.
    ///
    /// A diagnostic without one is ZeroDDS policy and says so in the
    /// string rather than leaving it empty. Spec §7.
    pub spec_reference: &'static str,
}

impl Diagnostic {
    /// An error at `primary`.
    #[must_use]
    pub fn error(
        code: DiagnosticCode,
        primary: Span,
        message: String,
        spec_reference: &'static str,
    ) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message,
            primary,
            labels: Vec::new(),
            file: None,
            spec_reference,
        }
    }

    /// Whether the rule is a clause of a specification rather than
    /// ZeroDDS policy.
    ///
    /// A clause reference carries the section sign and names the
    /// specification (`OMG IDL 4.2 §7.5.2`, `DDS-XTypes 1.3 §7.2.2.4.5`);
    /// a policy diagnostic names the ZeroDDS document that states the
    /// policy (`zerodds-ast-1.0 §8.1`), as [`Diagnostic::spec_reference`]
    /// asks of it.
    #[must_use]
    pub fn cites_a_clause(&self) -> bool {
        self.spec_reference.contains('§') && !self.spec_reference.starts_with("zerodds")
    }

    /// A warning at `primary`.
    ///
    /// For what is accepted and is still worth a caller's attention: a
    /// construct admitted only because the dialect was asked to admit
    /// it, and which no other vendor's compiler will read.
    #[must_use]
    pub fn warning(
        code: DiagnosticCode,
        primary: Span,
        message: String,
        spec_reference: &'static str,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message,
            primary,
            labels: Vec::new(),
            file: None,
            spec_reference,
        }
    }

    /// Add a secondary place worth pointing at.
    #[must_use]
    pub fn with_label(mut self, span: Span, note: String) -> Self {
        self.labels.push((span, note));
        self
    }

    /// Whether this is an error rather than a warning.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] at {}: {} ({})",
            match self.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            self.code.as_str(),
            self.primary,
            self.message,
            self.spec_reference
        )
    }
}

impl core::error::Error for Diagnostic {}

/// Whether `diagnostics` are the specification refusing the input: at
/// least one error, and every error cites a clause
/// ([`Diagnostic::cites_a_clause`]). Warnings count neither way.
///
/// What the probe generators answer with: a refusal by a clause is the
/// verdict `refused` (exit 1), and anything else stopping the front end
/// is a gap of this tree (exit 6), `docs/specs/zerodds-verdicts-1.0.md`.
#[must_use]
pub fn refused_by_a_clause(diagnostics: &[Diagnostic]) -> bool {
    let mut errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .peekable();
    errors.peek().is_some() && errors.all(Diagnostic::cites_a_clause)
}

#[cfg(test)]
mod clause_tests {
    use super::*;

    fn at(severity: Severity, spec_reference: &'static str) -> Diagnostic {
        Diagnostic {
            severity,
            code: DiagnosticCode::InvalidInheritance,
            message: String::new(),
            primary: Span::new(0, 0),
            labels: Vec::new(),
            file: None,
            spec_reference,
        }
    }

    #[test]
    fn a_clause_of_an_omg_specification_refuses_and_zerodds_policy_does_not() {
        let clause = at(Severity::Error, "DDS-XTypes 1.3 §7.2.2.4.5");
        let idl = at(Severity::Error, "OMG IDL 4.2 §7.5.2");
        let policy = at(Severity::Error, "zerodds-ast-1.0 §8.1");
        let warning = at(Severity::Warning, "zerodds-idl-frontend-1.0 §3");
        assert!(clause.cites_a_clause() && idl.cites_a_clause());
        assert!(!policy.cites_a_clause());
        assert!(refused_by_a_clause(&[clause.clone(), idl.clone()]));
        assert!(refused_by_a_clause(&[clause.clone(), warning.clone()]));
        assert!(!refused_by_a_clause(&[clause, policy]));
        assert!(!refused_by_a_clause(&[warning]));
        assert!(!refused_by_a_clause(&[]));
    }
}
