// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The recursive-descent driver. Spec §3.
//!
//! The parser never returns `Err` for a syntax error. It records a
//! diagnostic, wraps the offending range in an error node, and
//! resynchronises — because a frontend that stops at the first mistake
//! makes a large file take as many runs as it has mistakes.

use crate::diagnostic::{Diagnostic, DiagnosticCode};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use zerodds_idl_cst::{
    Checkpoint, CstKind, GreenNode, Span, SyntaxNode, Token, TokenKind, TreeBuilder, lex,
};

/// What came out of parsing a file.
#[derive(Debug, Clone)]
pub struct Parse {
    root: Arc<GreenNode>,
    diagnostics: Vec<Diagnostic>,
    /// Where each piece of the tree's text came from, in tree order: the
    /// piece's offset in the tree and its range in the source the parser
    /// read. See [`Parse::source_span`].
    placed: Vec<(u32, Span)>,
}

impl Parse {
    /// The tree, always present — a file that fails to parse still has
    /// one, with its bad ranges as error nodes.
    #[must_use]
    pub fn syntax(&self) -> Arc<SyntaxNode> {
        Arc::new(SyntaxNode::root(self.root.clone()))
    }

    /// The green root.
    #[must_use]
    pub fn green(&self) -> &Arc<GreenNode> {
        &self.root
    }

    /// Everything wrong with the input, in source order.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether anything was wrong.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// Put diagnostics ahead of the ones already there.
    ///
    /// For the preprocessor's, which explain most of what the parser
    /// then makes of the result.
    pub fn prepend_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        let mut all = diagnostics;
        all.append(&mut self.diagnostics);
        self.diagnostics = all;
    }

    /// Reconstruct the source. Equals the input byte for byte, whether
    /// or not it parsed. Spec §2.2.
    #[must_use]
    pub fn text(&self) -> String {
        self.root.text()
    }

    /// A range of the tree as a range of the source the parser read.
    ///
    /// The same range for a file parsed with [`parse`], whose tree is the
    /// file byte for byte. Not for [`parse_preprocessed`]: the expanded
    /// text puts one space between two tokens and the tree holds the
    /// tokens without it, and a pragma's text stands after the token it
    /// leads there and before it here. A node's range in the tree — what
    /// lowering records — is an offset into neither file nor expanded
    /// text until it passes through this.
    ///
    /// Each end is moved by the piece of text that holds it; an end past
    /// the last piece stays at the end of that piece.
    #[must_use]
    pub fn source_span(&self, tree: Span) -> Span {
        let start = self.source_offset(tree.start, false);
        let end = if tree.end > tree.start {
            self.source_offset(tree.end, true).max(start)
        } else {
            start
        };
        Span::new(start, end)
    }

    /// One offset of the tree in the source. `end` reads the offset as
    /// the end of a range, so it belongs to the piece that holds the
    /// byte before it.
    fn source_offset(&self, offset: u32, end: bool) -> u32 {
        let probe = if end {
            offset.saturating_sub(1)
        } else {
            offset
        };
        let Some(index) = self
            .placed
            .partition_point(|(start, _)| *start <= probe)
            .checked_sub(1)
        else {
            return self
                .placed
                .first()
                .map_or(offset, |(_, source)| source.start);
        };
        let (start, source) = self.placed[index];
        source
            .start
            .saturating_add(offset.saturating_sub(start))
            .min(source.end)
    }
}

/// Parse a file.
#[must_use]
pub fn parse(source: &str) -> Parse {
    parse_with(source, crate::Dialect::ALL_VENDORS)
}

/// Parse under a chosen dialect.
///
/// [`parse`] admits every vendor extension, which is what a tool reading
/// files it did not write needs. `Dialect::STRICT` is OMG IDL 4.2 and
/// nothing else: a file that parses under it is one every vendor's
/// compiler also reads. Spec §3.5.
#[must_use]
pub fn parse_with(source: &str, dialect: crate::Dialect) -> Parse {
    let mut parser = Parser::new(source);
    parser.dialect = dialect;
    parser.parse_file()
}

/// Parse a preprocessed stream.
///
/// The tree covers the expanded text, not the file as written: an
/// `#include` brought other files into it and a conditional cut parts of
/// it out. [`parse`] is the byte-exact path; this is the one that sees
/// what a compiler would.
///
/// The preprocessor's own diagnostics come first, because a file that
/// failed to include something explains most of what follows.
#[must_use]
pub fn parse_preprocessed(preprocessed: &crate::preprocess::Preprocessed) -> Parse {
    parse_preprocessed_with(preprocessed, crate::Dialect::ALL_VENDORS)
}

/// Parse a preprocessed stream under a chosen dialect.
///
/// [`parse_preprocessed`] admits every vendor extension. This is the
/// entry `--strict` reaches: `Dialect::STRICT` turns every vendor
/// construct from a warning into an error, so a file that gets through
/// is one every other vendor's front end reads too. Spec §3.5.
#[must_use]
pub fn parse_preprocessed_with(
    preprocessed: &crate::preprocess::Preprocessed,
    dialect: crate::Dialect,
) -> Parse {
    let mut parser = Parser::over(&preprocessed.text, preprocessed.tokens.clone());
    parser.dialect = dialect;
    let mut parse = parser.parse_file();
    parse.prepend_diagnostics(preprocessed.diagnostics.clone());
    parse
}

/// The parser state.
pub struct Parser<'a> {
    pub(crate) source: &'a str,
    pub(crate) tokens: Vec<Token>,
    pub(crate) at: usize,
    pub(crate) builder: TreeBuilder,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Which dialect this parse accepts. Spec §3.5.
    pub(crate) dialect: crate::Dialect,
    /// Guards against a rule that consumes nothing being retried
    /// forever. Bumped whenever the cursor moves.
    steps: usize,
    /// How long the tree's text is so far.
    tree_len: u32,
    /// Where each piece of the tree's text came from.
    /// [`Parse::source_span`].
    placed: Vec<(u32, Span)>,
}

/// How many parser steps one file may take.
///
/// A bound rather than trust: a rule that fails to consume is a defect,
/// and without this it hangs instead of being reported.
const MAX_STEPS: usize = 4_000_000;

impl<'a> Parser<'a> {
    /// A parser over `source`.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            tokens: lex(source),
            at: 0,
            builder: TreeBuilder::new(),
            diagnostics: Vec::new(),
            dialect: crate::Dialect::ALL_VENDORS,
            steps: 0,
            tree_len: 0,
            placed: Vec::new(),
        }
    }

    /// A parser over an already-lexed stream.
    ///
    /// The preprocessor produces one: its tokens index into text it
    /// assembled from every file that contributed, so the parser lexes
    /// nothing and reads spans into that text instead.
    #[must_use]
    pub fn over(source: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            at: 0,
            builder: TreeBuilder::new(),
            diagnostics: Vec::new(),
            dialect: crate::Dialect::ALL_VENDORS,
            steps: 0,
            tree_len: 0,
            placed: Vec::new(),
        }
    }

    /// Parse the whole file.
    #[must_use]
    pub fn parse_file(mut self) -> Parse {
        self.builder.start(CstKind::File);
        while !self.at_eof() {
            let before = self.at;
            self.parse_definition();
            if self.at == before {
                // A rule that consumed nothing would loop; take the
                // token as an error and move on.
                self.error_token("expected a declaration");
            }
            if self.steps > MAX_STEPS {
                break;
            }
        }
        self.eat_eof();
        self.builder.finish();

        let root = self
            .builder
            .build()
            .unwrap_or_else(|| Arc::new(GreenNode::new(CstKind::File, Vec::new())));
        Parse {
            root,
            diagnostics: self.diagnostics,
            placed: self.placed,
        }
    }

    // -- cursor ----------------------------------------------------------

    /// What the current token is.
    #[must_use]
    pub fn current(&self) -> TokenKind {
        self.tokens.get(self.at).map_or(TokenKind::Eof, |t| t.kind)
    }

    /// What the token `ahead` positions on is.
    #[must_use]
    pub fn peek(&self, ahead: usize) -> TokenKind {
        self.tokens
            .get(self.at + ahead)
            .map_or(TokenKind::Eof, |t| t.kind)
    }

    /// Whether the cursor is at the end.
    #[must_use]
    pub fn at_eof(&self) -> bool {
        self.current() == TokenKind::Eof
    }

    /// Where the current token is.
    #[must_use]
    pub fn current_span(&self) -> Span {
        self.tokens
            .get(self.at)
            .map_or_else(|| Span::empty_at(self.source_end()), |t| t.span)
    }

    /// The text of the current token.
    #[must_use]
    pub fn current_text(&self) -> &str {
        self.tokens
            .get(self.at)
            .and_then(|t| t.span.slice(self.source))
            .unwrap_or("")
    }

    #[allow(clippy::cast_possible_truncation)]
    fn source_end(&self) -> u32 {
        self.source.len() as u32
    }

    /// Consume the current token into the tree, trivia first.
    ///
    /// Trivia reaches the tree through `Parser::emit_leading_trivia`
    /// and nowhere else, which is what keeps the round trip exact
    /// without every rule having to think about it.
    pub fn bump(&mut self) {
        self.steps += 1;
        let Some(token) = self.tokens.get(self.at) else {
            return;
        };
        let text = token.span.slice(self.source).unwrap_or("");
        let kind = token.kind;
        let span = token.span;
        self.emit_leading_trivia();
        self.emit(kind, text, span);
        self.at += 1;
    }

    /// Put one piece of text into the tree, and note where in the source
    /// it came from ([`Parse::source_span`]).
    pub(crate) fn emit(&mut self, kind: TokenKind, text: &str, source: Span) {
        if !text.is_empty() {
            self.placed.push((self.tree_len, source));
        }
        self.builder.token(kind, text);
        #[allow(clippy::cast_possible_truncation)]
        let len = text.len() as u32;
        self.tree_len = self.tree_len.saturating_add(len);
    }

    /// Put the current token's leading trivia into the tree.
    ///
    /// Trivia goes into the tree here and nowhere else. A `//@` comment
    /// that reaches this point is one no declaration claimed as a suffix
    /// annotation, and it is reported where it stands: DDS-XTypes 1.3
    /// §7.3.1.2.5 admits the form only after the declaration it applies
    /// to, and a `//@key` that is read as a comment leaves the model
    /// saying "not a key" about a file that says "key".
    fn emit_leading_trivia(&mut self) {
        let Some(token) = self.tokens.get(self.at) else {
            return;
        };
        let leading: Vec<(zerodds_idl_cst::TriviaKind, Span)> = token
            .leading
            .iter()
            .map(|trivia| (trivia.kind, trivia.span))
            .collect();
        for (kind, span) in leading {
            let text = span.slice(self.source).unwrap_or("");
            if kind == zerodds_idl_cst::TriviaKind::LineComment
                && crate::suffix::is_suffix_comment(text)
            {
                self.diagnose_stray_suffix(span);
            }
            self.emit(trivia_token(kind), text, span);
        }
    }

    /// Consume the end-of-file token, so its trailing trivia lands in
    /// the tree.
    pub(crate) fn eat_eof(&mut self) {
        self.emit_leading_trivia();
    }

    /// The rest of a string literal whose first piece `kind` was just
    /// consumed: every adjacent literal of the same width.
    ///
    /// §7.2.6.3: "Adjacent string literals are concatenated", and
    /// §7.2.6.4 says the same of wide ones. A lexical rule, so it holds
    /// wherever a string literal stands — a constant, an annotation
    /// argument, a `typeid`, a `typeprefix`, an `import`, a `context`
    /// clause. The pieces stay separate tokens, so the round trip keeps
    /// them as written; the lowering joins their values, each decoded on
    /// its own, which keeps the characters of the pieces "distinct".
    /// A literal of the other width next to it is diagnosed: neither
    /// clause concatenates a narrow and a wide literal.
    pub(crate) fn eat_adjacent_string_literals(&mut self, kind: TokenKind) {
        loop {
            let next = self.current();
            if next == kind {
                self.bump();
            } else if matches!(next, TokenKind::StringLiteral | TokenKind::WStringLiteral) {
                self.diagnose(
                    crate::diagnostic::DiagnosticCode::UnexpectedToken,
                    "a string literal and a wide string literal are not concatenated; \
                     adjacent literals have to be of one width",
                    "OMG IDL 4.2 §7.2.6.3, §7.2.6.4",
                );
                self.bump();
            } else {
                return;
            }
        }
    }

    /// Consume the current token if it is `kind`.
    pub fn eat(&mut self, kind: TokenKind) -> bool {
        if self.current() == kind {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consume `kind`, or diagnose that it was missing.
    ///
    /// Does not consume anything on failure: the caller's recovery
    /// decides what to skip.
    pub fn expect(&mut self, kind: TokenKind, what: &str) -> bool {
        if self.eat(kind) {
            return true;
        }
        let span = Span::empty_at(self.current_span().start);
        self.diagnostics.push(Diagnostic::error(
            DiagnosticCode::MissingToken,
            span,
            format!("expected {what}"),
            "OMG IDL 4.2 §7.4",
        ));
        false
    }

    // -- tree ------------------------------------------------------------

    /// Begin a node.
    pub fn start(&mut self, kind: CstKind) {
        self.builder.start(kind);
    }

    /// End the node in progress.
    pub fn finish(&mut self) {
        self.builder.finish();
    }

    /// Mark the current position, for wrapping already-emitted children.
    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        self.builder.checkpoint()
    }

    /// Wrap everything since `checkpoint` in a node.
    pub fn start_at(&mut self, checkpoint: Checkpoint, kind: CstKind) {
        self.builder.start_at(checkpoint, kind);
    }

    /// How many tokens are left.
    #[must_use]
    pub fn tokens_left(&self) -> usize {
        self.tokens.len().saturating_sub(self.at)
    }

    /// Consume the first half of a `>>` token, leaving the second.
    ///
    /// `sequence<sequence<long>>` ends in one token that closes two
    /// argument lists. The tree records two `>` covering one source
    /// token, so `text()` still reproduces the input. Spec §2.3.
    pub fn split_shift_right(&mut self) {
        self.steps += 1;
        let Some(token) = self.tokens.get(self.at).cloned() else {
            return;
        };
        if token.kind != TokenKind::GreaterGreater {
            return;
        }
        // The leading trivia belongs to the first half only.
        self.emit_leading_trivia();
        self.emit(
            TokenKind::Greater,
            ">",
            Span::new(token.span.start, token.span.start.saturating_add(1)),
        );

        // Replace the token with a bare `>` covering the second byte, so
        // the outer list closes on real input rather than on nothing.
        let rest = Token {
            kind: TokenKind::Greater,
            span: Span::new(token.span.start.saturating_add(1), token.span.end),
            leading: Vec::new(),
            value: token.value,
        };
        if let Some(slot) = self.tokens.get_mut(self.at) {
            *slot = rest;
        }
    }

    // -- diagnostics and recovery ---------------------------------------

    /// Record a diagnostic without consuming anything.
    pub fn diagnose(&mut self, code: DiagnosticCode, message: &str, spec_reference: &'static str) {
        self.diagnostics.push(Diagnostic::error(
            code,
            self.current_span(),
            message.to_string(),
            spec_reference,
        ));
    }

    /// Record that a construct outside OMG IDL 4.2 was written here.
    ///
    /// **Always recorded, whether or not the dialect admits it.** The
    /// severity is what the dialect decides: an error when the flag is
    /// off, a warning when it is on.
    ///
    /// Admitting a construct and saying nothing about it are two
    /// different things, and only the first is what `Dialect` is for.
    /// The gate here was written the other way round — silent when
    /// admitted — and `parse` passes `Dialect::ALL_VENDORS` while every
    /// caller in this tree calls `parse`, so the gate never fired for
    /// anybody and `keylist` reached every one of them with nothing said
    /// about it. A file that is not IDL 4.2 is an interoperability fact
    /// of the same kind as a byte difference, and a caller cannot act on
    /// a fact it is never given.
    ///
    /// Spec §3.5.
    pub(crate) fn diagnose_vendor(
        &mut self,
        flag: crate::dialect::VendorFlag,
        message: &str,
        spec_reference: &'static str,
    ) {
        let span = self.current_span();
        let diagnostic = if self.dialect.admits(flag) {
            Diagnostic::warning(
                DiagnosticCode::VendorExtension,
                span,
                message.to_string(),
                spec_reference,
            )
        } else {
            Diagnostic::error(
                DiagnosticCode::VendorExtension,
                span,
                message.to_string(),
                spec_reference,
            )
        };
        self.diagnostics.push(diagnostic);
    }

    /// Wrap the current token in an error node and move past it.
    pub fn error_token(&mut self, message: &str) {
        self.diagnose(DiagnosticCode::UnexpectedToken, message, "OMG IDL 4.2 §7.4");
        self.start(CstKind::Error);
        self.bump();
        self.finish();
    }

    /// Skip to the next place a declaration can begin.
    ///
    /// Recovery points are `;`, `}` and the top-level keywords. The
    /// skipped range becomes an error node so the round trip holds.
    /// Spec §3.2.
    pub fn recover_to_declaration(&mut self) {
        if self.at_eof() || self.at_recovery_point() {
            return;
        }
        self.start(CstKind::Error);
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.current() {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                TokenKind::Semicolon if depth == 0 => {
                    self.bump();
                    break;
                }
                _ if depth == 0 && self.at_recovery_point() => break,
                _ => {}
            }
            self.bump();
            if self.steps > MAX_STEPS {
                break;
            }
        }
        self.finish();
    }

    /// Whether the current token can begin a declaration.
    #[must_use]
    pub fn at_recovery_point(&self) -> bool {
        matches!(
            self.current(),
            TokenKind::Module
                | TokenKind::Const
                | TokenKind::Typedef
                | TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
                | TokenKind::Bitmask
                | TokenKind::Bitset
                | TokenKind::Native
                | TokenKind::Exception
                | TokenKind::Interface
                | TokenKind::Valuetype
                | TokenKind::Eventtype
                | TokenKind::Component
                | TokenKind::Home
                | TokenKind::Connector
                | TokenKind::Porttype
                | TokenKind::Local
                | TokenKind::Abstract
                | TokenKind::Custom
                | TokenKind::Typeid
                | TokenKind::Typeprefix
                | TokenKind::Import
                | TokenKind::At
        )
    }

    /// The diagnostics gathered so far.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// The token kind one piece of trivia takes in the tree.
///
/// `Newline` and `Whitespace` are one kind here: the tree keeps the text
/// itself, so nothing about the round trip needs them apart.
pub(crate) const fn trivia_token(kind: zerodds_idl_cst::TriviaKind) -> TokenKind {
    match kind {
        zerodds_idl_cst::TriviaKind::Whitespace | zerodds_idl_cst::TriviaKind::Newline => {
            TokenKind::Whitespace
        }
        zerodds_idl_cst::TriviaKind::LineComment => TokenKind::LineComment,
        zerodds_idl_cst::TriviaKind::BlockComment => TokenKind::BlockComment,
        zerodds_idl_cst::TriviaKind::Directive => TokenKind::Directive,
    }
}
