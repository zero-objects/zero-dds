// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The preprocessor. Spec §3.4.
//!
//! Directives reach here as trivia — the lexer recognises a `#` line and
//! keeps it, without interpreting it. This module reads them, resolves
//! includes, expands macros, and produces a token stream in which every
//! token knows where it textually came from **and** which macros
//! produced it.
//!
//! An include cycle is diagnosed, not followed.
//!
//! IDL 4.2 §7.3: "IDL shall be preprocessed according to the
//! specification of the preprocessor in ISO/IEC 14882:2003." That is
//! object-like and function-like macros (§16.3) with `#` and `##`
//! (§16.3.2, §16.3.3), the rescan and its hide sets (§16.3.4), `#if`
//! over a full integral constant expression (§16.1), `#line` (§16.4),
//! `#error` (§16.5), `__LINE__` and `__FILE__` (§16.8), and lines
//! continued with a backslash (§2.1). `__DATE__` and `__TIME__` are left
//! to the caller's defines: a front end that stamps the clock into what
//! it reads gives a different unit on every run.

mod condition;
pub mod macro_;
mod pptoken;

use crate::diagnostic::{Diagnostic, DiagnosticCode};
use alloc::borrow::ToOwned;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use macro_::{MacroTable, MacroUse};
use zerodds_idl_cst::{Span, Token, Trivia, TriviaKind, lex};

/// Which file a piece of text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub u32);

/// A file the preprocessor read.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// How it was named.
    pub path: String,
    /// What it contained.
    pub text: String,
}

/// Where one expanded token came from.
///
/// Both halves matter: the range says where a reader should look, and
/// the chain says which macro put it there. A diagnostic inside an
/// expansion needs to name both. Spec §3.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The file the text is in.
    pub file: FileId,
    /// Where in that file.
    pub span: Span,
    /// The macro expansions that produced it, outermost first. Empty for
    /// a token written literally.
    pub macro_chain: Vec<MacroUse>,
}

impl Origin {
    /// A token written literally in a file.
    #[must_use]
    pub const fn literal(file: FileId, span: Span) -> Self {
        Self {
            file,
            span,
            macro_chain: Vec::new(),
        }
    }

    /// Whether this token came out of a macro rather than being written.
    #[must_use]
    pub fn is_expanded(&self) -> bool {
        !self.macro_chain.is_empty()
    }
}

/// Where every token of the expanded stream came from.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
    origins: Vec<Origin>,
    /// Every stretch of the expanded text with the place it was taken
    /// from — each token, and each pragma or `//@` comment carried as
    /// trivia — in the order the text was written, which is ascending.
    ranges: Vec<(Span, Origin)>,
}

impl SourceMap {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            files: Vec::new(),
            origins: Vec::new(),
            ranges: Vec::new(),
        }
    }

    /// The file a unit is: the first one read.
    pub const ROOT: FileId = FileId(0);

    /// Record a file, returning its id.
    pub fn add_file(&mut self, path: &str, text: &str) -> FileId {
        let id = u32::try_from(self.files.len()).unwrap_or(u32::MAX);
        self.files.push(SourceFile {
            path: path.to_owned(),
            text: text.to_owned(),
        });
        FileId(id)
    }

    /// A file's recorded path.
    #[must_use]
    pub fn path(&self, file: FileId) -> Option<&str> {
        self.files.get(file.0 as usize).map(|f| f.path.as_str())
    }

    /// A file's text.
    #[must_use]
    pub fn text(&self, file: FileId) -> Option<&str> {
        self.files.get(file.0 as usize).map(|f| f.text.as_str())
    }

    /// Where the token at `index` in the expanded stream came from.
    #[must_use]
    pub fn origin(&self, index: usize) -> Option<&Origin> {
        self.origins.get(index)
    }

    /// How many tokens the map covers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.origins.len()
    }

    /// Whether the map covers nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    fn push(&mut self, origin: Origin) {
        self.origins.push(origin);
    }

    fn push_range(&mut self, expanded: Span, origin: Origin) {
        self.ranges.push((expanded, origin));
    }

    /// Where a range of the expanded text was written: the file and the
    /// range in that file's own text.
    ///
    /// A range that starts inside a token written as it stands maps
    /// octet for octet; one that starts inside a macro's expansion maps
    /// to the invocation, which is the text a reader can find. A range
    /// over several tokens of one file runs from the first to the last;
    /// one that leaves its file — a declaration whose body an include
    /// supplied — keeps the file it starts in. `None` before the first
    /// token, which a diagnostic of an empty unit points at.
    #[must_use]
    pub fn original(&self, expanded: Span) -> Option<(FileId, Span)> {
        let containing = |at: u32| {
            self.ranges
                .partition_point(|(range, _)| range.start <= at)
                .checked_sub(1)
        };
        let first = containing(expanded.start)?;
        let last = containing(expanded.end.saturating_sub(1).max(expanded.start))
            .unwrap_or(first)
            .max(first);
        let (from, start) = &self.ranges[first];
        let (to, end) = &self.ranges[last];
        let start_at = Self::offset_in(*from, start, expanded.start, false);
        if start.file != end.file {
            return Some((start.file, Span::new(start_at, start.span.end)));
        }
        let end_at = Self::offset_in(*to, end, expanded.end, true).max(start_at);
        Some((start.file, Span::new(start_at, end_at)))
    }

    /// The origin of the range of the expanded text that holds `at`:
    /// the file, where in it, and the macro expansions that made the
    /// text, outermost first. `None` before the first token.
    #[must_use]
    pub fn origin_at(&self, at: u32) -> Option<&Origin> {
        let index = self
            .ranges
            .partition_point(|(range, _)| range.start <= at)
            .checked_sub(1)?;
        self.ranges.get(index).map(|(_, origin)| origin)
    }

    /// `at`, an offset of the expanded text inside `range`, as an offset
    /// of the file `origin` names: shifted by the same distance where the
    /// text is the file's own, the invocation's edge where a macro made
    /// it.
    fn offset_in(range: Span, origin: &Origin, at: u32, end: bool) -> u32 {
        let written =
            !origin.is_expanded() && range.end - range.start == origin.span.end - origin.span.start;
        if written {
            let within = at.clamp(range.start, range.end) - range.start;
            origin.span.start + within
        } else if end {
            origin.span.end
        } else {
            origin.span.start
        }
    }
}

/// Where a node of the model was written ([`Preprocessed::provenance`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The file's path, as the preprocessor recorded it.
    pub path: String,
    /// The range in that file's text.
    pub span: Span,
    /// The macro expansions that produced the text, outermost first.
    pub macro_chain: Vec<ProvenanceMacro>,
}

/// One macro expansion of a [`Provenance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceMacro {
    /// The macro.
    pub name: String,
    /// The file its invocation is written in.
    pub path: Option<String>,
    /// Where in that file.
    pub at: Span,
}

/// How the preprocessor finds an included file.
///
/// A trait rather than direct file access, so the whole preprocessor is
/// testable without a filesystem and so a build system can supply its
/// own include path.
pub trait FileLoader {
    /// Resolve and read `path`, as requested from `from`.
    ///
    /// Returns the resolved name and the contents. `None` if it cannot
    /// be found — the caller diagnoses that rather than the loader.
    fn load(&self, path: &str, from: Option<&str>) -> Option<(String, String)>;
}

/// A loader over an in-memory set of files.
///
/// What the tests use, and enough for a build system that already has
/// its sources in hand.
#[derive(Debug, Clone, Default)]
pub struct MemoryLoader {
    files: Vec<(String, String)>,
}

impl MemoryLoader {
    /// An empty loader.
    #[must_use]
    pub const fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Make `path` resolvable to `text`.
    #[must_use]
    pub fn with(mut self, path: &str, text: &str) -> Self {
        self.files.push((path.to_owned(), text.to_owned()));
        self
    }
}

impl FileLoader for MemoryLoader {
    fn load(&self, path: &str, _from: Option<&str>) -> Option<(String, String)> {
        self.files
            .iter()
            .find(|(name, _)| name == path)
            .map(|(name, text)| (name.clone(), text.clone()))
    }
}

/// The expanded token stream and where it came from.
#[derive(Debug, Clone)]
pub struct Preprocessed {
    /// The tokens the parser will see.
    pub tokens: Vec<Token>,
    /// The text those tokens index into.
    ///
    /// Concatenated from every file that contributed, so a token's span
    /// still slices something. The map says which file a range belongs
    /// to.
    pub text: String,
    /// Where each token came from.
    pub map: SourceMap,
    /// Everything wrong with the directives.
    pub diagnostics: Vec<Diagnostic>,
}

impl Preprocessed {
    /// Where a range of the expanded text was written, for a node of the
    /// model rather than a diagnostic: the file's path, the range in its
    /// text ([`SourceMap::original`]), and the macro expansions that
    /// produced the text at its start, outermost first, each with the
    /// path and range of its invocation. `None` before the first token.
    #[must_use]
    pub fn provenance(&self, expanded: Span) -> Option<Provenance> {
        let (file, span) = self.map.original(expanded)?;
        let path = self.map.path(file)?.to_string();
        let macro_chain = self
            .map
            .origin_at(expanded.start)
            .map(|origin| {
                origin
                    .macro_chain
                    .iter()
                    .map(|used| ProvenanceMacro {
                        name: used.name.clone(),
                        path: self.map.path(FileId(used.file)).map(ToString::to_string),
                        at: used.at,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Provenance {
            path,
            span,
            macro_chain,
        })
    }

    /// Moves `diagnostic` out of the expanded text into the file it
    /// points at.
    ///
    /// The parser and everything after it see only the expanded text,
    /// in which comments are gone, directives are gone, includes are
    /// inlined and every token stands one space from the next — so an
    /// offset into it is an offset into no file a reader has. Read
    /// against the unit, the first lines of comment were enough to put
    /// `file.idl:3:7` where `file.idl:6:10` was meant. This maps
    /// `primary` and every label through [`SourceMap::original`], and
    /// sets [`Diagnostic::file`] when the place is an included file. A
    /// label in a file other than the primary's names that file in its
    /// note.
    ///
    /// For diagnostics raised on the expanded text only: the
    /// preprocessor's own are in file offsets already.
    pub fn locate(&self, diagnostic: &mut Diagnostic) {
        let path = |file: FileId| {
            (file != SourceMap::ROOT)
                .then(|| self.map.path(file).map(ToString::to_string))
                .flatten()
        };
        let primary_file = match self.map.original(diagnostic.primary) {
            Some((file, span)) => {
                diagnostic.primary = span;
                diagnostic.file = path(file);
                Some(file)
            }
            None => None,
        };
        for (span, note) in &mut diagnostic.labels {
            if let Some((file, mapped)) = self.map.original(*span) {
                *span = mapped;
                if Some(file) != primary_file {
                    let name = self.map.path(file).unwrap_or("?");
                    *note = format!("{name}: {note}");
                }
            }
        }
    }
}

/// How deep includes may nest.
///
/// A bound rather than trust: a cycle is diagnosed, but a chain that is
/// merely very long should stop too.
const MAX_INCLUDE_DEPTH: usize = 64;

/// How deep argument pre-expansion may nest: `F(F(F(…)))` expands the
/// innermost argument first, one level per parenthesis.
const MAX_EXPANSION_DEPTH: usize = 256;

/// How many tokens the macro expansions of one unit may produce in all.
///
/// The hide sets of §16.3.4 make every expansion terminate, but not
/// every expansion small: a few lines of mutually doubling macros reach
/// billions of tokens. A bound, reported by name, rather than a host out
/// of memory.
const MAX_EXPANDED_TOKENS: usize = 1 << 22;

/// Run the preprocessor over `source`, named `path`.
#[must_use]
pub fn preprocess(path: &str, source: &str, loader: &dyn FileLoader) -> Preprocessed {
    preprocess_with(path, source, loader, &[])
}

/// As [`preprocess`], with macros the caller defined.
///
/// `defines` is what a build system passes as `-D`: each entry is a name
/// and its body, and an empty body is `-DNAME` — defined, expanding to
/// nothing, which is what `#ifdef` asks about. They are in place before
/// the first line is read, so a `#ifndef` guard at the top of the unit
/// sees them. A name written `NAME(a, b)` defines a function-like macro.
#[must_use]
pub fn preprocess_with(
    path: &str,
    source: &str,
    loader: &dyn FileLoader,
    defines: &[(String, String)],
) -> Preprocessed {
    let mut state = Preprocessor {
        loader,
        map: SourceMap::new(),
        macros: MacroTable::new(),
        diagnostics: Vec::new(),
        text: String::new(),
        tokens: Vec::new(),
        pending_pragmas: Vec::new(),
        open: Vec::new(),
        once: Vec::new(),
        presumed: Vec::new(),
        expansion_depth: 0,
        produced: 0,
    };
    for (name, body) in defines {
        let line = format!("{name} {body}");
        state.define(&line, Span::default());
    }
    state.include(path, source, 0);
    // A pragma at the end of the unit has no following token to attach
    // to, and `#pragma keylist Sample id` written under the struct it
    // keys is the common spelling — Cyclone's own examples put it there.
    // It goes onto the last token instead, which is the same declaration.
    state.flush_trailing_pragmas();
    Preprocessed {
        tokens: state.tokens,
        text: state.text,
        map: state.map,
        diagnostics: state.diagnostics,
    }
}

struct Preprocessor<'a> {
    loader: &'a dyn FileLoader,
    map: SourceMap,
    macros: MacroTable,
    diagnostics: Vec<Diagnostic>,
    text: String,
    tokens: Vec<Token>,
    /// `#pragma` lines waiting for the next token to carry them.
    ///
    /// The preprocessor consumes every directive and emits its tokens
    /// with no leading trivia, so a pragma written in the source did not
    /// survive into the stream the parser sees — and the lowering looks
    /// for exactly that trivia. `#pragma keylist Sample id` therefore
    /// reached the model only when nothing preprocessed the file, which
    /// meant a `build.rs` produced a type with no key at all.
    ///
    /// A pragma carries declaration metadata about what *follows* it, so
    /// it is attached to the next token rather than the previous one.
    pending_pragmas: Vec<(String, Trivia, FileId)>,
    /// The include chain, so a cycle is recognised rather than followed.
    open: Vec<String>,
    /// The files that asked to be read once, by resolved path.
    ///
    /// `#pragma once` reached the pragma arm below and was held as
    /// trivia like every other pragma, which is to say it was carried
    /// to the model and read by nobody. A header that named it was
    /// expanded once per `#include` all the same, and the second
    /// expansion redeclared every name in it — the same
    /// `DuplicateName` a header with no guard at all produces.
    once: Vec<String>,
    /// What `#line` (§16.4) said about a file: the difference between
    /// the line `__LINE__` reports and the physical one, and the name
    /// `__FILE__` reports instead of the path.
    presumed: Vec<(FileId, i64, Option<String>)>,
    /// How deep argument pre-expansion is nested now.
    expansion_depth: usize,
    /// How many tokens expansions have produced in this unit.
    produced: usize,
}

/// A preprocessing token on its way through expansion.
#[derive(Debug, Clone)]
struct PpToken {
    /// Its spelling.
    text: String,
    /// White space stood before it; `#` (§16.3.2) needs to know.
    space_before: bool,
    /// Where it came from.
    origin: Origin,
    /// The token the IDL lexer read, where this one was written in a
    /// file and is emitted as written. `None` for a token an expansion
    /// made, which the IDL lexer reads when it is emitted.
    written: Option<Token>,
    /// The macros whose expansion produced it, §16.3.4: none of them is
    /// expanded again inside it.
    hide: Vec<String>,
    /// A macro name §16.3.4/2 has taken out of further replacement for
    /// good.
    painted: bool,
}

impl PpToken {
    fn made(text: String, space_before: bool, origin: Origin) -> Self {
        Self {
            text,
            space_before,
            origin,
            written: None,
            hide: Vec::new(),
            painted: false,
        }
    }

    fn is_identifier(&self) -> bool {
        pptoken::is_identifier(&self.text)
    }
}

/// One file being read: its tokens, how far, and the conditionals open
/// in it.
struct FileCursor<'s> {
    tokens: Vec<Token>,
    at: usize,
    source: &'s str,
    file: FileId,
    conditions: Vec<Condition>,
    depth: usize,
}

impl FileCursor<'_> {
    fn keeping(&self) -> bool {
        self.conditions.iter().all(Condition::keeping)
    }
}

/// Where an expansion's tokens go: straight into the stream, or into a
/// list, for an argument expanded before substitution (§16.3.1) and for
/// an `#if` line.
enum Sink<'v> {
    Emit,
    Collect(&'v mut Vec<PpToken>),
}

impl Preprocessor<'_> {
    fn include(&mut self, path: &str, source: &str, depth: usize) {
        if self.once.iter().any(|seen| seen == path) {
            return;
        }
        if depth > MAX_INCLUDE_DEPTH {
            self.report(Diagnostic::error(
                DiagnosticCode::UnterminatedDeclaration,
                Span::default(),
                format!("include nesting past {MAX_INCLUDE_DEPTH} levels at `{path}`"),
                "OMG IDL 4.2 §7.3",
            ));
            return;
        }
        if self.open.iter().any(|open| open == path) {
            self.report(Diagnostic::error(
                DiagnosticCode::UnexpectedToken,
                Span::default(),
                format!(
                    "`{path}` includes itself, directly or through {}",
                    self.open.join(" -> ")
                ),
                "OMG IDL 4.2 §7.3",
            ));
            return;
        }

        let file = self.map.add_file(path, source);
        self.open.push(path.to_owned());
        let mut cursor = FileCursor {
            tokens: lex(source),
            at: 0,
            source,
            file,
            conditions: Vec::new(),
            depth,
        };
        let mut queue = VecDeque::new();
        self.run(&mut queue, Some(&mut cursor), &mut Sink::Emit);

        if !cursor.conditions.is_empty() {
            self.report(Diagnostic::error(
                DiagnosticCode::UnterminatedDeclaration,
                Span::default(),
                format!(
                    "{} conditional(s) left open in `{path}`",
                    cursor.conditions.len()
                ),
                "OMG IDL 4.2 §7.3",
            ));
        }
        let _ = self.open.pop();
    }

    /// The next token: the front of the replacement queue, or else the
    /// next token of the file that its conditionals keep, after the
    /// directives written before it have been carried out.
    fn pull(
        &mut self,
        queue: &mut VecDeque<PpToken>,
        cursor: Option<&mut FileCursor<'_>>,
    ) -> Option<PpToken> {
        if let Some(token) = queue.pop_front() {
            return Some(token);
        }
        let cursor = cursor?;
        while cursor.at < cursor.tokens.len() {
            let index = cursor.at;
            cursor.at += 1;
            let leading = cursor.tokens[index].leading.clone();
            for trivia in &leading {
                match trivia.kind {
                    TriviaKind::Directive => {
                        let line = trivia.span.slice(cursor.source).unwrap_or("");
                        self.directive(line, trivia.span, cursor);
                    }
                    // A `//@` comment is an annotation — DDS-XTypes 1.3
                    // §7.3.1.2.5, Table 24 — and the preprocessor emits
                    // no comments, so it would reach the parser as
                    // nothing at all. `//@key` written in a file read
                    // through `preprocess` left the member with no key
                    // and said nothing, which is the same defect the
                    // pragma below was fixed for. It is held and
                    // attached to the next token, like a pragma.
                    TriviaKind::LineComment
                        if crate::suffix::is_suffix_comment(
                            trivia.span.slice(cursor.source).unwrap_or(""),
                        ) && cursor.keeping() =>
                    {
                        self.pending_pragmas.push((
                            trivia.span.slice(cursor.source).unwrap_or("").to_string(),
                            Trivia {
                                kind: TriviaKind::LineComment,
                                span: trivia.span,
                            },
                            cursor.file,
                        ));
                    }
                    _ => {}
                }
            }
            let token = &cursor.tokens[index];
            if token.kind == zerodds_idl_cst::TokenKind::Eof || !cursor.keeping() {
                continue;
            }
            return Some(PpToken {
                text: token.span.slice(cursor.source).unwrap_or("").to_string(),
                space_before: !token.leading.is_empty(),
                origin: Origin::literal(cursor.file, token.span),
                written: Some(token.clone()),
                hide: Vec::new(),
                painted: false,
            });
        }
        None
    }

    /// Expand everything `queue` and `cursor` hold, ISO/IEC 14882:2003
    /// §16.3, and deliver each token that is not replaced to `sink`.
    ///
    /// A replacement goes back to the front of the queue, so it is
    /// rescanned together with the rest of the source (§16.3.4/1) —
    /// which is how `#define F G` and a function-like `G` written `F(1)`
    /// expand at all. Every token carries the macros that produced it,
    /// and none of those is replaced inside it again: the hide sets that
    /// make the rescan terminate.
    fn run(
        &mut self,
        queue: &mut VecDeque<PpToken>,
        mut cursor: Option<&mut FileCursor<'_>>,
        sink: &mut Sink<'_>,
    ) {
        while let Some(mut token) = self.pull(queue, cursor.as_deref_mut()) {
            if token.painted || !token.is_identifier() {
                self.deliver(token, sink);
                continue;
            }
            if token.hide.contains(&token.text) {
                // §16.3.4/2: met again inside its own expansion, the name
                // is not replaced — now or on any later rescan.
                token.painted = true;
                self.deliver(token, sink);
                continue;
            }
            if let Some(answer) = self.predefined(&token) {
                self.deliver(answer, sink);
                continue;
            }
            let Some(found) = self.macros.get(&token.text).cloned() else {
                self.deliver(token, sink);
                continue;
            };
            let replacement = match &found.params {
                None => {
                    let mut hide = token.hide.clone();
                    hide.push(token.text.clone());
                    self.substitute(&found, &[], &token, &hide)
                }
                Some(params) => {
                    // A function-like name without `(` after it is not an
                    // invocation (§16.3/10) and stays as written.
                    let Some(next) = self.pull(queue, cursor.as_deref_mut()) else {
                        self.deliver(token, sink);
                        continue;
                    };
                    if next.text != "(" {
                        queue.push_front(next);
                        self.deliver(token, sink);
                        continue;
                    }
                    let Some((mut args, closing)) =
                        self.arguments(queue, cursor.as_deref_mut(), &token)
                    else {
                        continue;
                    };
                    // `F()` is one empty argument, and that is no
                    // argument for a macro of no parameter.
                    if params.is_empty() && args.len() == 1 && args[0].is_empty() {
                        args.clear();
                    }
                    if args.len() != params.len() {
                        self.preprocessing_error(
                            token.origin.span,
                            format!(
                                "`{}` takes {} argument(s) and is given {}",
                                token.text,
                                params.len(),
                                args.len()
                            ),
                            "ISO/IEC 14882:2003 §16.3/4",
                        );
                        continue;
                    }
                    // §16.3.4 as Prosser's algorithm states it: the
                    // names hidden in both the macro name and the `)`
                    // that closes its invocation, and the macro.
                    let mut hide: Vec<String> = token
                        .hide
                        .iter()
                        .filter(|name| closing.hide.contains(name))
                        .cloned()
                        .collect();
                    hide.push(token.text.clone());
                    self.substitute(&found, &args, &token, &hide)
                }
            };
            self.produced = self.produced.saturating_add(replacement.len());
            if self.produced > MAX_EXPANDED_TOKENS {
                self.preprocessing_error(
                    token.origin.span,
                    format!(
                        "macro expansion produced more than {MAX_EXPANDED_TOKENS} tokens; `{}` is \
                         not expanded further",
                        token.text
                    ),
                    "zerodds-idl-frontend-1.0 §3.4",
                );
                return;
            }
            for replaced in replacement.into_iter().rev() {
                queue.push_front(replaced);
            }
        }
    }

    /// The arguments of an invocation whose `(` was just read, split at
    /// the commas no parenthesis encloses (§16.3/10), and the `)` that
    /// ends them. `None`, reported, where the source ends first.
    fn arguments(
        &mut self,
        queue: &mut VecDeque<PpToken>,
        mut cursor: Option<&mut FileCursor<'_>>,
        name: &PpToken,
    ) -> Option<(Vec<Vec<PpToken>>, PpToken)> {
        let mut args: Vec<Vec<PpToken>> = alloc::vec![Vec::new()];
        let mut nesting = 0_usize;
        while let Some(token) = self.pull(queue, cursor.as_deref_mut()) {
            match token.text.as_str() {
                ")" if nesting == 0 => return Some((args, token)),
                "," if nesting == 0 => args.push(Vec::new()),
                "(" | ")" => {
                    if token.text == "(" {
                        nesting += 1;
                    } else {
                        nesting -= 1;
                    }
                    if let Some(last) = args.last_mut() {
                        last.push(token);
                    }
                }
                _ => {
                    if let Some(last) = args.last_mut() {
                        last.push(token);
                    }
                }
            }
        }
        self.preprocessing_error(
            name.origin.span,
            format!(
                "the invocation of `{}` is not closed before the end of the input",
                name.text
            ),
            "ISO/IEC 14882:2003 §16.3/10",
        );
        None
    }

    /// The replacement list of `found` for the invocation `invocation`,
    /// with its parameters substituted (§16.3.1), `#` applied (§16.3.2)
    /// and `##` applied (§16.3.3), every token hiding `hide`.
    fn substitute(
        &mut self,
        found: &macro_::Macro,
        args: &[Vec<PpToken>],
        invocation: &PpToken,
        hide: &[String],
    ) -> Vec<PpToken> {
        enum Item {
            Token(PpToken),
            /// An argument with no tokens, which still takes part in a
            /// paste. §16.3.3/2's placemarker.
            Placemarker,
            Paste,
        }
        let mut chain = invocation.origin.macro_chain.clone();
        chain.push(MacroUse {
            name: invocation.text.clone(),
            at: invocation.origin.span,
            file: invocation.origin.file.0,
        });
        let origin = Origin {
            file: invocation.origin.file,
            span: invocation.origin.span,
            macro_chain: chain.clone(),
        };
        let params: &[String] = found.params.as_deref().unwrap_or(&[]);
        let function_like = found.params.is_some();
        let body = &found.body;
        let mut expanded: Vec<Option<Vec<PpToken>>> = alloc::vec![None; args.len()];
        let mut items: Vec<Item> = Vec::new();
        let mut index = 0;
        while index < body.len() {
            let piece = &body[index];
            let parameter = |text: &str| params.iter().position(|param| param == text);
            if function_like && piece.text == "#" {
                if let Some(slot) = body.get(index + 1).and_then(|next| parameter(&next.text)) {
                    items.push(Item::Token(PpToken::made(
                        stringize(&args[slot]),
                        piece.space_before,
                        origin.clone(),
                    )));
                    index += 2;
                    continue;
                }
            }
            if piece.text == "##" {
                items.push(Item::Paste);
                index += 1;
                continue;
            }
            if let Some(slot) = parameter(&piece.text).filter(|_| function_like) {
                // §16.3.1: an argument next to `#` or `##` is taken as
                // written; anywhere else it is "completely macro
                // replaced as if they formed the rest of the
                // preprocessing file".
                let pasted = body.get(index + 1).is_some_and(|next| next.text == "##")
                    || (index > 0 && body[index - 1].text == "##");
                let tokens = if pasted {
                    args[slot].clone()
                } else {
                    if expanded[slot].is_none() {
                        expanded[slot] = Some(self.expand_isolated(args[slot].clone()));
                    }
                    expanded[slot].clone().unwrap_or_default()
                };
                if tokens.is_empty() {
                    items.push(Item::Placemarker);
                }
                for (position, mut token) in tokens.into_iter().enumerate() {
                    if position == 0 {
                        token.space_before = piece.space_before;
                    }
                    let mut inner = chain.clone();
                    inner.extend(token.origin.macro_chain);
                    token.origin.macro_chain = inner;
                    items.push(Item::Token(token));
                }
                index += 1;
                continue;
            }
            items.push(Item::Token(PpToken::made(
                piece.text.clone(),
                piece.space_before,
                origin.clone(),
            )));
            index += 1;
        }

        // §16.3.3/3: each `##` joins the token before it and the one
        // after it, left to right, and the result has to be one token.
        let mut joined: Vec<Item> = Vec::new();
        let mut items = items.into_iter();
        while let Some(item) = items.next() {
            if !matches!(item, Item::Paste) {
                joined.push(item);
                continue;
            }
            let left = joined.pop();
            let right = items.next();
            match (left, right) {
                (Some(Item::Placemarker), Some(right)) => joined.push(right),
                (Some(left), Some(Item::Placemarker)) => joined.push(left),
                (Some(Item::Token(left)), Some(Item::Token(right))) => {
                    let text = format!("{}{}", left.text, right.text);
                    match pptoken::tokenize(&text) {
                        Ok(pieces) if pieces.len() == 1 => {
                            joined.push(Item::Token(PpToken::made(
                                text,
                                left.space_before,
                                origin.clone(),
                            )));
                        }
                        _ => {
                            self.preprocessing_error(
                                invocation.origin.span,
                                format!(
                                    "pasting `{}` and `{}` gives `{text}`, which is not one \
                                     preprocessing token",
                                    left.text, right.text
                                ),
                                "ISO/IEC 14882:2003 §16.3.3/3",
                            );
                            joined.push(Item::Token(left));
                            joined.push(Item::Token(right));
                        }
                    }
                }
                // A `##` at either end of a list is refused at `#define`
                // (§16.3.3/1), so an operand is always there.
                (left, right) => {
                    joined.extend(left);
                    joined.extend(right);
                }
            }
        }
        joined
            .into_iter()
            .filter_map(|item| match item {
                Item::Token(mut token) => {
                    for name in hide {
                        if !token.hide.contains(name) {
                            token.hide.push(name.clone());
                        }
                    }
                    Some(token)
                }
                Item::Placemarker | Item::Paste => None,
            })
            .collect()
    }

    /// An argument expanded on its own, as §16.3.1 asks: as if it were
    /// the rest of the file, with nothing after it to complete an
    /// invocation.
    ///
    /// zerodds-lint: recursion-depth 256 (`MAX_EXPANSION_DEPTH`; one
    /// level per argument nested in an argument, reported past it).
    fn expand_isolated(&mut self, tokens: Vec<PpToken>) -> Vec<PpToken> {
        if self.expansion_depth >= MAX_EXPANSION_DEPTH {
            if let Some(first) = tokens.first() {
                self.preprocessing_error(
                    first.origin.span,
                    format!(
                        "macro arguments nest past {MAX_EXPANSION_DEPTH} levels; this one is not \
                         expanded"
                    ),
                    "zerodds-idl-frontend-1.0 §3.4",
                );
            }
            return tokens;
        }
        self.expansion_depth += 1;
        let mut queue: VecDeque<PpToken> = tokens.into();
        let mut out = Vec::new();
        self.run(&mut queue, None, &mut Sink::Collect(&mut out));
        self.expansion_depth -= 1;
        out
    }

    /// `__LINE__` and `__FILE__`, §16.8/1, for the invocation `token`.
    fn predefined(&self, token: &PpToken) -> Option<PpToken> {
        let file = token.origin.file;
        let presumed = self.presumed.iter().rev().find(|(of, _, _)| *of == file);
        let text = match token.text.as_str() {
            "__LINE__" => {
                let physical = self
                    .map
                    .text(file)
                    .map_or(1, |source| line_of(source, token.origin.span.start));
                let delta = presumed.map_or(0, |(_, delta, _)| *delta);
                format!("{}", i64::from(physical) + delta)
            }
            "__FILE__" => {
                let name = presumed
                    .and_then(|(_, _, name)| name.clone())
                    .or_else(|| self.map.path(file).map(ToOwned::to_owned))
                    .unwrap_or_default();
                quote(&name)
            }
            _ => return None,
        };
        let mut answer = PpToken::made(text, token.space_before, token.origin.clone());
        answer.hide.clone_from(&token.hide);
        Some(answer)
    }

    /// Hand a token on: into the stream, or into the list being built.
    fn deliver(&mut self, token: PpToken, sink: &mut Sink<'_>) {
        match sink {
            Sink::Collect(list) => list.push(token),
            Sink::Emit => self.emit(token),
        }
    }

    /// Put a final token into the stream the parser reads.
    ///
    /// A token written in a file goes as the IDL lexer read it. One an
    /// expansion made is lexed as IDL now, and has to be one IDL token:
    /// `&&` is a preprocessing token and no IDL one, and is reported
    /// rather than passed on as something else.
    fn emit(&mut self, token: PpToken) {
        if let Some(written) = &token.written {
            let written = written.clone();
            self.push_text(written.kind, &token.text, &written, token.origin);
            return;
        }
        let lexed = lex(&token.text);
        let mut significant = lexed
            .iter()
            .filter(|lexed| lexed.kind != zerodds_idl_cst::TokenKind::Eof);
        match (significant.next(), significant.next()) {
            (Some(only), None) if only.leading.is_empty() => {
                let only = only.clone();
                self.push_text(only.kind, &token.text, &only, token.origin);
            }
            _ => {
                self.preprocessing_error(
                    token.origin.span,
                    format!(
                        "macro expansion produced `{}`, which is not an IDL token",
                        token.text
                    ),
                    "OMG IDL 4.2 §7.2",
                );
                let error = Token {
                    kind: zerodds_idl_cst::TokenKind::Error,
                    span: Span::default(),
                    leading: Vec::new(),
                    value: zerodds_idl_cst::TokenValue::None,
                };
                self.push_text(error.kind, &token.text, &error, token.origin);
            }
        }
    }

    /// Records a diagnostic of the file being read. Its spans are that
    /// file's own, so one raised inside an included file names it.
    fn report(&mut self, mut diagnostic: Diagnostic) {
        if self.open.len() > 1 {
            diagnostic.file = self.open.last().cloned();
        }
        self.diagnostics.push(diagnostic);
    }

    fn preprocessing_error(&mut self, span: Span, message: String, rule: &'static str) {
        self.report(Diagnostic::error(
            DiagnosticCode::Preprocessing,
            span,
            message,
            rule,
        ));
    }

    /// Append a token to the expanded stream, keeping its trivia so the
    /// round trip survives preprocessing.
    fn push_text(
        &mut self,
        kind: zerodds_idl_cst::TokenKind,
        text: &str,
        original: &Token,
        origin: Origin,
    ) {
        // A separator between tokens, so the expanded text is itself
        // lexable: without it `struct S` becomes `structS`, and a
        // consumer reading the text rather than the token stream reads
        // one identifier where there were two.
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        #[allow(clippy::cast_possible_truncation)]
        let start = self.text.len() as u32;
        self.text.push_str(text);
        #[allow(clippy::cast_possible_truncation)]
        let end = self.text.len() as u32;
        // Every pragma seen since the last token goes in front of this
        // one. The text they name is appended too, because the lowering
        // reads the directive's own bytes out of the span.
        self.map.push_range(Span::new(start, end), origin.clone());
        let leading = core::mem::take(&mut self.pending_pragmas)
            .into_iter()
            .map(|(text, held, file)| {
                self.text.push(' ');
                #[allow(clippy::cast_possible_truncation)]
                let from = self.text.len() as u32;
                self.text.push_str(&text);
                #[allow(clippy::cast_possible_truncation)]
                let to = self.text.len() as u32;
                self.map
                    .push_range(Span::new(from, to), Origin::literal(file, held.span));
                Trivia {
                    kind: held.kind,
                    span: Span::new(from, to),
                }
            })
            .collect();
        self.tokens.push(Token {
            kind,
            span: Span::new(start, end),
            leading,
            value: original.value.clone(),
        });
        self.map.push(origin);
    }

    /// Attach any pragma left over at the end of the unit to the last
    /// token, so a directive written after the declaration it names is
    /// not lost with the end of the file.
    fn flush_trailing_pragmas(&mut self) {
        if self.pending_pragmas.is_empty() {
            return;
        }
        let pending = core::mem::take(&mut self.pending_pragmas);
        let mut on_last = Vec::new();
        let mut on_eof = Vec::new();
        let mut eof_file = None;
        for (text, held, file) in pending {
            self.text.push(' ');
            #[allow(clippy::cast_possible_truncation)]
            let from = self.text.len() as u32;
            self.text.push_str(&text);
            #[allow(clippy::cast_possible_truncation)]
            let to = self.text.len() as u32;
            self.map
                .push_range(Span::new(from, to), Origin::literal(file, held.span));
            let moved = Trivia {
                kind: held.kind,
                span: Span::new(from, to),
            };
            // A pragma names the declaration it follows, so the last
            // token is the same declaration and carrying it there is
            // right. An annotation is read from the leading trivia of
            // the token *after* the declaration, so the last token is
            // exactly where it must not go: it would be reported as a
            // `//@` in a position that admits none. It gets an
            // end-of-file token to lead instead, which is the position
            // the unpreprocessed parse gives it.
            if held.kind == TriviaKind::LineComment {
                on_eof.push(moved);
                eof_file = eof_file.or(Some(file));
            } else {
                on_last.push(moved);
            }
        }
        if let Some(last) = self.tokens.last_mut() {
            last.leading.extend(on_last);
        }
        let (Some(file), false) = (eof_file, on_eof.is_empty()) else {
            return;
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = self.text.len() as u32;
        self.tokens.push(Token {
            kind: zerodds_idl_cst::TokenKind::Eof,
            span: Span::new(at, at),
            leading: on_eof,
            value: zerodds_idl_cst::TokenValue::None,
        });
        self.map.push(Origin {
            file,
            span: Span::new(at, at),
            macro_chain: Vec::new(),
        });
    }

    /// `#define`, the text after the word: a name, a parameter list
    /// where `(` follows the name with no space (§16.3/3 — `#define F
    /// (x)` is object-like), and the replacement list.
    fn define(&mut self, rest: &str, span: Span) {
        let text = rest.trim_start();
        let name_end = text
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || !c.is_ascii()))
            .unwrap_or(text.len());
        let name = &text[..name_end];
        if name.is_empty() {
            self.report(Diagnostic::error(
                DiagnosticCode::MissingToken,
                span,
                "`#define` without a name".to_string(),
                "OMG IDL 4.2 §7.3",
            ));
            return;
        }
        let after = &text[name_end..];
        let defined = if let Some(list) = after.strip_prefix('(') {
            let Some(close) = list.find(')') else {
                self.preprocessing_error(
                    span,
                    format!("the parameter list of `{name}` is not closed"),
                    "ISO/IEC 14882:2003 §16.3",
                );
                return;
            };
            let params: Vec<String> = if list[..close].trim().is_empty() {
                Vec::new()
            } else {
                list[..close]
                    .split(',')
                    .map(|param| param.trim().to_string())
                    .collect()
            };
            self.macros
                .define_function(name, params, &list[close + 1..])
        } else {
            self.macros.define(name, after)
        };
        if let Err(why) = defined {
            self.preprocessing_error(
                span,
                format!("`#define {name}`: {why}"),
                "ISO/IEC 14882:2003 §16.3",
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn directive(&mut self, raw: &str, span: Span, cursor: &mut FileCursor<'_>) {
        // §2.1/1 phase 2: a backslash at the end of a line splices it to
        // the next. The lexer keeps a continued directive one trivia; the
        // splice is made here, before the directive is read.
        let line = splice(raw);
        let body = line.trim_start().trim_start_matches('#').trim();
        let (word, rest) = split_word(body);
        let keeping = cursor.keeping();
        let file = cursor.file;

        match word {
            "define" if keeping => self.define(rest, span),
            // ISO/IEC 14882:2003 §16.5, which IDL 4.2 §7.3 adopts
            // wholesale: "#error causes the implementation to produce a
            // diagnostic message that includes the specified sequence of
            // preprocessing tokens." The directive was read and dropped,
            // so a file whose whole purpose was to refuse an unsupported
            // configuration compiled clean and emitted an empty
            // translation unit.
            //
            // `if keeping` is the whole of the rule's subtlety: an
            // `#error` inside a branch the conditionals cut out is text
            // that was never reached, and reporting it would refuse every
            // file that guards one — which is nearly every file that has
            // one at all.
            "error" if keeping => {
                let text = rest.trim();
                let message = if text.is_empty() {
                    String::from("`#error`")
                } else {
                    format!("`#error {text}`")
                };
                self.report(Diagnostic::error(
                    DiagnosticCode::ErrorDirective,
                    span,
                    message,
                    "OMG IDL 4.2 §7.3, ISO/IEC 14882:2003 §16.5",
                ));
            }
            "undef" if keeping => {
                let (name, _) = split_word(rest);
                if macro_::RESERVED.contains(&name) {
                    self.preprocessing_error(
                        span,
                        format!("`{name}` cannot be undefined"),
                        "ISO/IEC 14882:2003 §16.8/4",
                    );
                } else {
                    self.macros.undefine(name);
                }
            }
            "include" if keeping => {
                let Some(target) = include_target(rest) else {
                    self.report(Diagnostic::error(
                        DiagnosticCode::UnexpectedToken,
                        span,
                        "`#include` expects \"path\" or <path>".to_string(),
                        "OMG IDL 4.2 §7.3",
                    ));
                    return;
                };
                let from = self.map.path(file).map(ToOwned::to_owned);
                match self.loader.load(&target, from.as_deref()) {
                    Some((resolved, text)) => self.include(&resolved, &text, cursor.depth + 1),
                    None => self.report(Diagnostic::error(
                        DiagnosticCode::UnexpectedToken,
                        span,
                        format!("cannot find `{target}`"),
                        "OMG IDL 4.2 §7.3",
                    )),
                }
            }
            "line" if keeping => self.line_control(rest, span, cursor),
            "ifdef" => {
                let (name, _) = split_word(rest);
                cursor
                    .conditions
                    .push(Condition::new(keeping && self.macros.is_defined(name)));
            }
            "ifndef" => {
                let (name, _) = split_word(rest);
                cursor
                    .conditions
                    .push(Condition::new(keeping && !self.macros.is_defined(name)));
            }
            "if" => {
                // A condition inside a branch already cut out is not
                // evaluated: it may name what that branch's own guard
                // says is missing, and §16.1/6 skips the group whole.
                let taken = keeping && self.evaluate_condition(rest, span, file);
                cursor.conditions.push(Condition::new(taken));
            }
            "elif" => {
                let outer = cursor.conditions.len().saturating_sub(1);
                let enclosing_keeps = cursor.conditions[..outer].iter().all(Condition::keeping);
                let open = cursor
                    .conditions
                    .last()
                    .map(|condition| condition.satisfied);
                match open {
                    Some(satisfied) => {
                        let taken = enclosing_keeps
                            && !satisfied
                            && self.evaluate_condition(rest, span, file);
                        if let Some(condition) = cursor.conditions.last_mut() {
                            condition.elif(taken);
                        }
                    }
                    None => self.unmatched(span, "#elif"),
                }
            }
            "else" => match cursor.conditions.last_mut() {
                Some(condition) => condition.otherwise(),
                None => self.unmatched(span, "#else"),
            },
            "endif" => {
                if cursor.conditions.pop().is_none() {
                    self.unmatched(span, "#endif");
                }
            }
            // A pragma is not the preprocessor's business — but it has to
            // survive it. It carries declaration metadata the lowering
            // reads out of the trivia, so it is held here and attached to
            // the next token emitted rather than dropped with the rest of
            // the directive.
            //
            // With one exception: `#pragma once` is the one pragma this
            // stage owns. It says something about inclusion and nothing
            // about the declaration that follows, so it is answered
            // here and never handed on — a directive this stage has
            // acted on is not one the lowering has to report as
            // uninterpreted.
            "pragma" if keeping && rest.trim() == "once" => {
                if let Some(current) = self.open.last().cloned()
                    && !self.once.contains(&current)
                {
                    self.once.push(current);
                }
            }
            "pragma" => {
                if keeping {
                    self.pending_pragmas.push((
                        line.trim().to_string(),
                        Trivia {
                            kind: TriviaKind::Directive,
                            span,
                        },
                        file,
                    ));
                }
            }
            _ => {}
        }
    }

    /// `#line digit-sequence ["s-char-sequence"]`, §16.4: the line after
    /// the directive is numbered `digit-sequence`, and the file is called
    /// what the string says. Macros in the line are replaced first
    /// (§16.4/5).
    fn line_control(&mut self, rest: &str, span: Span, cursor: &FileCursor<'_>) {
        let tokens = self.expand_line(rest, span, cursor.file);
        let mut texts = tokens.iter().map(|token| token.text.as_str());
        let number = texts.next().and_then(|digits| {
            if digits.bytes().all(|b| b.is_ascii_digit()) {
                digits.parse::<i64>().ok()
            } else {
                None
            }
        });
        let name = match texts.next() {
            None => None,
            Some(literal) => match unquote(literal) {
                Some(name) => Some(name),
                None => {
                    self.preprocessing_error(
                        span,
                        format!("`#line` names its file with a string literal, not `{literal}`"),
                        "ISO/IEC 14882:2003 §16.4/4",
                    );
                    return;
                }
            },
        };
        let Some(number) = number.filter(|n| (1..=2_147_483_647).contains(n)) else {
            self.preprocessing_error(
                span,
                "`#line` takes a line number from 1 to 2147483647".to_string(),
                "ISO/IEC 14882:2003 §16.4/3",
            );
            return;
        };
        // The physical line the directive's successor starts on: the
        // directive's own, plus any line it was continued over, plus one.
        let next = line_of(cursor.source, span.end) + 1;
        let delta = number - i64::from(next);
        let name = name.or_else(|| {
            self.presumed
                .iter()
                .rev()
                .find(|(of, _, _)| *of == cursor.file)
                .and_then(|(_, _, name)| name.clone())
        });
        self.presumed.push((cursor.file, delta, name));
    }

    /// The tokens of a directive's operands with every macro replaced,
    /// for `#line` and `#if`.
    fn expand_line(&mut self, rest: &str, span: Span, file: FileId) -> Vec<PpToken> {
        match pptoken::tokenize(rest) {
            Ok(pieces) => self.expand_isolated(
                pieces
                    .into_iter()
                    .map(|piece| {
                        PpToken::made(piece.text, piece.space_before, Origin::literal(file, span))
                    })
                    .collect(),
            ),
            Err(why) => {
                self.preprocessing_error(span, why, "ISO/IEC 14882:2003 §2.4");
                Vec::new()
            }
        }
    }

    /// The truth of an `#if` or `#elif` line, §16.1.
    fn evaluate_condition(&mut self, rest: &str, span: Span, file: FileId) -> bool {
        let pieces = match pptoken::tokenize(rest) {
            Ok(pieces) => pieces,
            Err(why) => {
                self.preprocessing_error(span, why, "ISO/IEC 14882:2003 §2.4");
                return false;
            }
        };
        // `defined X` and `defined ( X )` are answered before any macro
        // is replaced (§16.1/1): replacing first would ask whether the
        // *body* of `X` is defined.
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < pieces.len() {
            let piece = &pieces[index];
            if piece.text != "defined" {
                tokens.push(PpToken::made(
                    piece.text.clone(),
                    piece.space_before,
                    Origin::literal(file, span),
                ));
                index += 1;
                continue;
            }
            let (name, next) = if pieces.get(index + 1).is_some_and(|p| p.text == "(") {
                let closed = pieces.get(index + 3).is_some_and(|p| p.text == ")");
                (pieces.get(index + 2).filter(|_| closed), index + 4)
            } else {
                (pieces.get(index + 1), index + 2)
            };
            let Some(name) = name.filter(|name| pptoken::is_identifier(&name.text)) else {
                self.preprocessing_error(
                    span,
                    "`defined` is followed by a macro name, alone or in parentheses".to_string(),
                    "ISO/IEC 14882:2003 §16.1/1",
                );
                return false;
            };
            let answer = if self.macros.is_defined(&name.text)
                || macro_::RESERVED[1..].contains(&name.text.as_str())
            {
                "1"
            } else {
                "0"
            };
            tokens.push(PpToken::made(
                answer.to_string(),
                piece.space_before,
                Origin::literal(file, span),
            ));
            index = next;
        }
        let expanded = self.expand_isolated(tokens);
        if expanded.iter().any(|token| token.text == "defined") {
            // §16.1/4: "If the token defined is generated as a result of
            // this replacement process … the behavior is undefined."
            self.preprocessing_error(
                span,
                "a macro in this condition expands to `defined`".to_string(),
                "ISO/IEC 14882:2003 §16.1/4",
            );
            return false;
        }
        let texts: Vec<String> = expanded.into_iter().map(|token| token.text).collect();
        match condition::evaluate(&texts) {
            Ok(outcome) => {
                for name in outcome.zeroed {
                    self.report(Diagnostic::warning(
                        DiagnosticCode::Preprocessing,
                        span,
                        format!("`{name}` in this condition names no macro and is taken as 0"),
                        "ISO/IEC 14882:2003 §16.1/4",
                    ));
                }
                outcome.value
            }
            Err(why) => {
                self.preprocessing_error(
                    span,
                    format!("`#if {}`: {why}", rest.trim()),
                    "ISO/IEC 14882:2003 §16.1",
                );
                false
            }
        }
    }

    fn unmatched(&mut self, span: Span, what: &str) {
        self.report(Diagnostic::error(
            DiagnosticCode::UnexpectedToken,
            span,
            format!("`{what}` without a matching `#if`"),
            "OMG IDL 4.2 §7.3",
        ));
    }
}

/// A directive line with every backslash-newline removed, §2.1/1
/// phase 2.
fn splice(line: &str) -> String {
    line.replace("\\\r\n", "").replace("\\\n", "")
}

/// The 1-based line `at` lies on in `source`.
fn line_of(source: &str, at: u32) -> u32 {
    let end = usize::try_from(at).unwrap_or(usize::MAX).min(source.len());
    let newlines = source.as_bytes()[..end]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    u32::try_from(newlines)
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

/// `text` as a string literal, `\` and `"` escaped.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

/// The contents of a string literal, escapes undone, or `None` for
/// anything else.
fn unquote(literal: &str) -> Option<String> {
    let inner = literal.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(chars.next()?);
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

/// `#parameter`, §16.3.2/2: the argument's spelling as a string
/// literal — one space where white space stood between two tokens, none
/// at either end, and `\` and `"` escaped inside a string or character
/// literal of the argument.
fn stringize(argument: &[PpToken]) -> String {
    let mut spelled = String::new();
    for (index, token) in argument.iter().enumerate() {
        if index > 0 && token.space_before {
            spelled.push(' ');
        }
        let literal = token.text.starts_with(['"', '\''])
            || token.text.starts_with("L\"")
            || token.text.starts_with("L'");
        if literal {
            for ch in token.text.chars() {
                if ch == '\\' || ch == '"' {
                    spelled.push('\\');
                }
                spelled.push(ch);
            }
        } else {
            spelled.push_str(&token.text);
        }
    }
    format!("\"{spelled}\"")
}

/// One level of conditional nesting.
#[derive(Debug, Clone, Copy)]
struct Condition {
    /// Whether the branch now open is being kept.
    active: bool,
    /// Whether any branch of this conditional has been taken, so a later
    /// `#elif` or `#else` knows to stay closed.
    satisfied: bool,
}

impl Condition {
    const fn new(taken: bool) -> Self {
        Self {
            active: taken,
            satisfied: taken,
        }
    }

    const fn keeping(&self) -> bool {
        self.active
    }

    const fn elif(&mut self, taken: bool) {
        self.active = taken && !self.satisfied;
        if self.active {
            self.satisfied = true;
        }
    }

    const fn otherwise(&mut self) {
        self.active = !self.satisfied;
        self.satisfied = true;
    }
}

/// Split off the first whitespace-delimited word.
fn split_word(text: &str) -> (&str, &str) {
    let trimmed = text.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(at) => (&trimmed[..at], &trimmed[at..]),
        None => (trimmed, ""),
    }
}

/// The path out of `"name"` or `<name>`.
fn include_target(rest: &str) -> Option<String> {
    let text = rest.trim();
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('<').and_then(|s| s.strip_suffix('>')))?;
    Some(inner.to_owned())
}
