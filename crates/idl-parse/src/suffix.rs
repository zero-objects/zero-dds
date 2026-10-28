// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The suffix form of an annotation. DDS-XTypes 1.3 §7.3.1.2.5.
//!
//! Table 24 of §7.3.1.2.5, p. 91, gives two productions for applying an
//! annotation and calls them equivalent:
//!
//! ```text
//! { "@"   <annotation_type_name> [ "(" <arguments> ")" ] }*
//! { "//@" <annotation_type_name> [ "(" <arguments> ")" ] }*
//! ```
//!
//! > "Alternately and equivalently, apply an annotation to a type or
//! > type member by suffixing it with an annotation type name using
//! > slash-slash-at ("//@") instead of the at sign by itself."
//!
//! §7.3.1.2.3 says where the second one may stand and how it is
//! recognised: "As an alternative to prefixing a declaration with an
//! annotation, it is legal to follow the declaration with a single-line
//! comment containing the annotation string. To distinguish such
//! comments from regular comments, there must be no space in between
//! the double slash ("//") and the at-sign ('@')." Its example applies
//! one to a member and one to the type:
//!
//! ```idl
//! struct Gadget {
//!     long my_integer; //@my_member_annotation("Hello")
//! }; //@my_type_annotation
//! ```
//!
//! and its second example applies two to one member, the second on the
//! line below: "If multiple annotations are to be applied to the same
//! element, the at-sign of each shall be preceded by a double slash and
//! no white space."
//!
//! ## How it is read
//!
//! The lexer does not know about this form — a `//@` comment is trivia
//! there, like every other comment, so nothing about lexing an IDL file
//! changes. The parser claims those comments at the positions where the
//! specification allows an annotation, immediately after it has parsed
//! the declaration they suffix, and turns each one into the same
//! [`CstKind::Annotation`] node the prefix form produces. The marker
//! token is [`TokenKind::SlashSlashAt`] instead of [`TokenKind::At`];
//! everything after it is parsed by [`Parser::parse_annotation_tail`],
//! the prefix form's own body. That is what makes the two forms
//! equivalent by construction rather than by two rules kept in step.
//!
//! A `//@` comment the parser does not claim is **not** dropped: it is
//! reported where it stands, by [`Parser::bump`], as a `//@` written
//! where no annotation may be applied. A file that says "this member is
//! the key" and a model that says it is not would be the same class of
//! defect as a dropped byte on the wire.

use crate::diagnostic::{Diagnostic, DiagnosticCode};
use crate::parser::Parser;
use alloc::string::ToString;
use alloc::vec::Vec;
use zerodds_idl_cst::{CstKind, Span, TokenKind, Trivia, TriviaKind, lex};

/// The marker that opens a suffix annotation. §7.3.1.2.3.
pub(crate) const MARKER: &str = "//@";

/// Whether a line comment's text opens a suffix annotation.
///
/// Exactly `//@`: `/// @key` and `// @key` are ordinary comments,
/// because §7.3.1.2.3 admits nothing between the double slash and the
/// at-sign, and `///@key` has a third slash there.
#[must_use]
pub(crate) fn is_suffix_comment(text: &str) -> bool {
    text.starts_with(MARKER)
}

impl Parser<'_> {
    /// Claim the `//@…` comments that suffix the declaration just
    /// parsed, as annotations of the node still open.
    ///
    /// Called where the specification allows an annotation and the
    /// declaration it applies to has just been consumed — after a
    /// member's `;`, after a declaration's closing `;`, after an
    /// enumerator's name. Everywhere else a `//@` is reported rather
    /// than read, which is the whole point of reading it here.
    ///
    /// Only white space may stand between the declaration and the run
    /// of annotations, and between two of them: §7.3.1.2.3's example
    /// separates the two annotations of one member by a line break and
    /// nothing else, and a comment in between would mean the annotation
    /// no longer suffixes the declaration.
    pub(crate) fn parse_suffix_annotations(&mut self) {
        while let Some(count) = self.suffix_run_length() {
            self.take_suffix_comment(count);
        }
    }

    /// How many leading trivia items of the current token end at the
    /// next suffix annotation, that one included.
    ///
    /// `None` when what follows the cursor is not one.
    fn suffix_run_length(&self) -> Option<usize> {
        let token = self.tokens.get(self.at)?;
        for (index, trivia) in token.leading.iter().enumerate() {
            match trivia.kind {
                TriviaKind::Whitespace | TriviaKind::Newline => {}
                TriviaKind::LineComment
                    if is_suffix_comment(trivia.span.slice(self.source).unwrap_or("")) =>
                {
                    return Some(index + 1);
                }
                _ => return None,
            }
        }
        None
    }

    /// Take `count` leading trivia items, the last of which is a suffix
    /// annotation comment, into the tree.
    ///
    /// The white space before it goes in as trivia and the comment goes
    /// in as one or more [`CstKind::Annotation`] nodes, so the bytes
    /// stay in the tree in the order they were written and the round
    /// trip holds. Taking them out of the token's leading list is what
    /// stops [`Parser::bump`] from emitting them a second time.
    fn take_suffix_comment(&mut self, count: usize) {
        let Some(token) = self.tokens.get_mut(self.at) else {
            return;
        };
        let taken: Vec<Trivia> = token.leading.drain(..count).collect();
        let Some((comment, before)) = taken.split_last() else {
            return;
        };
        for trivia in before {
            let text = trivia.span.slice(self.source).unwrap_or("");
            self.emit(crate::parser::trivia_token(trivia.kind), text, trivia.span);
        }
        for segment in self.suffix_segments(comment.span) {
            self.start(CstKind::Annotation);
            // The marker's own bytes stand right before the body.
            #[allow(clippy::cast_possible_truncation)]
            let marker = Span::new(
                segment.start.saturating_sub(MARKER.len() as u32),
                segment.start,
            );
            self.emit(TokenKind::SlashSlashAt, MARKER, marker);
            self.parse_suffix_body(segment);
            self.finish();
        }
    }

    /// The bodies of the annotations one comment carries, each without
    /// its `//@`.
    ///
    /// One comment usually carries one, but §7.3.1.2.3 requires no more
    /// of a second than that its at-sign be preceded by a double slash
    /// with no white space, which `//@a //@b` satisfies on one line as
    /// much as on two. A `//@` inside a string literal is part of the
    /// argument, not a second marker.
    fn suffix_segments(&self, comment: Span) -> Vec<Span> {
        let text = comment.slice(self.source).unwrap_or("");
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut start = MARKER.len();
        let mut at = start;
        let mut quote: Option<u8> = None;
        while at < bytes.len() {
            let byte = bytes[at];
            match quote {
                Some(closing) => {
                    if byte == b'\\' {
                        at += 2;
                        continue;
                    }
                    if byte == closing {
                        quote = None;
                    }
                    at += 1;
                }
                None if byte == b'"' || byte == b'\'' => {
                    quote = Some(byte);
                    at += 1;
                }
                None if bytes[at..].starts_with(MARKER.as_bytes()) => {
                    out.push(self.sub_span(comment, start, at));
                    at += MARKER.len();
                    start = at;
                }
                None => at += 1,
            }
        }
        out.push(self.sub_span(comment, start, bytes.len()));
        out
    }

    /// A byte range inside a comment, as a span into the file.
    #[allow(clippy::cast_possible_truncation)]
    fn sub_span(&self, comment: Span, from: usize, to: usize) -> Span {
        let base = comment.start;
        Span::new(
            base.saturating_add(from as u32),
            base.saturating_add(to as u32),
        )
    }

    /// Parse one annotation body out of the comment's own bytes.
    ///
    /// The body is lexed on its own and its spans are moved to where the
    /// bytes actually are, so every diagnostic and every span in the
    /// tree points at the file rather than at a detached fragment. The
    /// token stream is swapped in for the duration: the tree builder and
    /// the diagnostics are the parser's own, so the node this produces
    /// is the node the prefix form produces.
    fn parse_suffix_body(&mut self, body: Span) {
        let Some(text) = body.slice(self.source) else {
            return;
        };
        let mut tokens = lex(text);
        for token in &mut tokens {
            token.span = shift(token.span, body.start);
            for trivia in &mut token.leading {
                trivia.span = shift(trivia.span, body.start);
            }
        }

        let outer_tokens = core::mem::replace(&mut self.tokens, tokens);
        let outer_at = core::mem::replace(&mut self.at, 0);

        self.parse_annotation_tail();
        // Anything left over is text the annotation grammar has no place
        // for — `//@key and the rest of the sentence`. Reported rather
        // than kept as decoration: §7.3.1.2.3 makes the whole comment
        // the annotation, so a reader who wrote prose after one wrote
        // something this parser must not guess at.
        if !self.at_eof() {
            self.diagnose(
                DiagnosticCode::UnexpectedToken,
                "a `//@` annotation ends with the comment; nothing may follow it on the line",
                "DDS-XTypes 1.3 §7.3.1.2.3",
            );
            self.start(CstKind::Error);
            while !self.at_eof() {
                self.bump();
            }
            self.finish();
        }
        self.eat_eof();

        self.tokens = outer_tokens;
        self.at = outer_at;
    }

    /// Report a `//@` comment standing where no annotation may be
    /// applied.
    ///
    /// Called from [`Parser::bump`] for every comment that reaches the
    /// tree as trivia, which is every one the suffix rules did not
    /// claim. DDS-XTypes 1.3 §7.3.1.2.5 admits the form only as a suffix
    /// of a declaration; a `//@` anywhere else says something about a
    /// declaration that cannot be identified, and a frontend that
    /// dropped it would answer "not a key" to a file that said "key".
    pub(crate) fn diagnose_stray_suffix(&mut self, span: Span) {
        self.diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnexpectedToken,
            span,
            "a `//@` annotation may only follow the declaration it applies to".to_string(),
            "DDS-XTypes 1.3 §7.3.1.2.5",
        ));
    }
}

/// Move a span from a lexed fragment to where the fragment sits.
const fn shift(span: Span, by: u32) -> Span {
    Span::new(span.start.saturating_add(by), span.end.saturating_add(by))
}
