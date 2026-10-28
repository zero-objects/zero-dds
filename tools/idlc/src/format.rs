// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The canonical form of an IDL file.
//!
//! The formatter renders the token stream — every syntax token, every
//! comment, every preprocessor directive — under one fixed layout. It is
//! a printer over the lossless lexer output, not over the lowered model,
//! for one reason: the model does not carry comments or directives, and
//! a formatter that prints the model would drop both without a word.
//! The token stream carries them, so they are printed.
//!
//! Three properties, and each is enforced rather than assumed:
//!
//! - **Nothing is dropped.** After rendering, the output is lexed again
//!   and compared token for token — kinds and spellings — against the
//!   input. Comments and directives are part of that comparison. A
//!   mismatch is a named refusal of the file, never an output missing
//!   something.
//! - **Idempotent.** Rendering the rendered output must reproduce it
//!   byte for byte, and this too is checked on every run.
//! - **Round-trip safe.** The syntax tokens are unchanged, so reparsing
//!   the output builds the same tree modulo trivia, and lowering it
//!   builds the same model modulo source offsets. The corpus test in
//!   this module measures that over every IDL file the repository has.
//!
//! A file that is not valid IDL is refused. `zerodds-idlc format`
//! decides that the way `check` does, after the preprocessor
//! ([`canonical_of_checked`]): the call of a function-like macro is no
//! IDL syntax before its expansion and valid IDL after it, and the
//! layout, which is over tokens, needs no tree. The tests decide it on
//! the source as written (`canonical_under`), having no preprocessed
//! parse. Spellings are never normalised — `0x1F` stays `0x1F`,
//! because rewriting it as `31` would change the source, not format it
//! (the lexer keeps the radix for exactly this reason).

use zerodds_idl_cst::{Span, TokenKind, TriviaKind, lex};
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, Severity};

/// How many spaces one nesting level indents.
const INDENT: &str = "    ";

/// Format `source` canonically.
///
/// # Errors
/// The refusals, each named: the parse diagnostics when the file does
/// not parse cleanly, or the invariant that failed when the rendered
/// output would not have carried every token, comment and directive of
/// the input.
#[cfg(test)]
pub fn canonical(source: &str) -> Result<String, Vec<Diagnostic>> {
    canonical_under(source, zerodds_idl_parse::Dialect::ALL_VENDORS)
}

/// Format `source` canonically, under a chosen dialect.
///
/// [`canonical`] admits every vendor extension. `Dialect::STRICT` is what
/// `--strict` reaches: a vendor construct is then a parse error, and this
/// refuses the file rather than laying it out. A `format` that rewrites a
/// file the same run's `check` rejects hands the user a file their tool
/// discards on the next step.
///
/// # Errors
/// As [`canonical`].
#[cfg(test)]
pub fn canonical_under(
    source: &str,
    dialect: zerodds_idl_parse::Dialect,
) -> Result<String, Vec<Diagnostic>> {
    let parse = zerodds_idl_parse::parse_with(source, dialect);
    if parse.has_errors() {
        let mut diagnostics = parse.diagnostics().to_vec();
        diagnostics.push(refusal(
            Span::new(0, 0),
            String::from(
                "refusing to format: the file does not parse cleanly, and formatting a tree \
                 with error recovery in it would lay out the recovery rather than the source",
            ),
        ));
        return Err(diagnostics);
    }
    canonical_of_checked(source)
}

/// The canonical form of a `source` the caller has found valid after the
/// preprocessor — `zerodds-idlc format` runs `check`'s analysis first and
/// formats only a file with no error in it. The layout is over the
/// source's tokens, comments and directives, so a macro call stays the
/// tokens it was written as; the token comparison and the idempotence
/// check run as for every file.
///
/// # Errors
/// The invariant that failed when the rendered output would not have
/// carried every token, comment and directive of the input, or would not
/// render to itself.
pub fn canonical_of_checked(source: &str) -> Result<String, Vec<Diagnostic>> {
    let items = stream(source);
    let out = render(&items);
    verify(&items, &out)?;
    Ok(out)
}

/// One diagnostic that names why a file is not formatted.
fn refusal(span: Span, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: DiagnosticCode::Unsupported,
        message,
        primary: span,
        labels: Vec::new(),
        // ZeroDDS policy, and the message says so by saying "refusing".
        file: None,
        spec_reference: "",
    }
}

// ---------------------------------------------------------------------
// The stream: what the layout is computed over.
// ---------------------------------------------------------------------

/// What one item of the stream is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece {
    /// A syntax token.
    Code(TokenKind),
    /// `// ...`
    Line,
    /// `/* ... */`
    Block,
    /// A preprocessor line, kept verbatim.
    Directive,
}

/// One token, comment or directive, with the line structure that
/// preceded it in its source.
#[derive(Debug, Clone)]
struct Item {
    piece: Piece,
    text: String,
    /// Where it was in the source it came from, for naming a refusal.
    span: Span,
    /// Whether at least one line break separated it from the previous
    /// item.
    newline_before: bool,
    /// Whether at least one *blank line* did — two or more breaks.
    blank_before: bool,
}

fn slice(source: &str, span: Span) -> String {
    source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("")
        .to_string()
}

/// Everything the layout is computed over, in source order: syntax
/// tokens, comments and directives, with whitespace reduced to the two
/// facts the renderer keeps (a break, a blank line).
fn stream(source: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut newlines = 0usize;
    for token in lex(source) {
        for trivia in &token.leading {
            match trivia.kind {
                TriviaKind::Newline => newlines += 1,
                TriviaKind::Whitespace => {}
                TriviaKind::LineComment | TriviaKind::BlockComment | TriviaKind::Directive => {
                    items.push(Item {
                        piece: match trivia.kind {
                            TriviaKind::LineComment => Piece::Line,
                            TriviaKind::BlockComment => Piece::Block,
                            _ => Piece::Directive,
                        },
                        text: slice(source, trivia.span),
                        span: trivia.span,
                        newline_before: newlines > 0,
                        blank_before: newlines > 1,
                    });
                    newlines = 0;
                }
            }
        }
        if token.kind == TokenKind::Eof {
            break;
        }
        items.push(Item {
            piece: Piece::Code(token.kind),
            text: slice(source, token.span),
            span: token.span,
            newline_before: newlines > 0,
            blank_before: newlines > 1,
        });
        newlines = 0;
    }
    items
}

// ---------------------------------------------------------------------
// The renderer.
// ---------------------------------------------------------------------

/// What a `{` opens, decided by the declaration keyword before it.
///
/// An `enum` or `bitmask` body is a comma-separated list and each entry
/// takes a line of its own. Every other body is `;`-separated
/// declarations, where a comma separates declarators of *one*
/// declaration (`long a, b;`) and stays on the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BraceKind {
    List,
    Decls,
}

/// What bracket the renderer is currently inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Paren,
    Angle,
    Brace(BraceKind),
}

/// How two adjacent items are separated.
enum Sep {
    None,
    Space,
    Newline { blank: bool },
}

#[derive(Debug, Default)]
struct Renderer {
    out: String,
    indent: usize,
    ctx: Vec<Ctx>,
    /// What the next `{` will open.
    pending_brace: Option<BraceKind>,
    /// Between `case`/`default` and its `:` — where a colon is a label's
    /// and hugs its expression, unlike an inheritance colon.
    label_pending: bool,
    /// The previous syntax token, comments skipped.
    last_code: Option<TokenKind>,
    /// Whether that token was a unary `+`, `-` or `~`.
    last_unary: bool,
}

fn render(items: &[Item]) -> String {
    let mut renderer = Renderer::default();
    for (index, item) in items.iter().enumerate() {
        let prev = if index == 0 {
            None
        } else {
            items.get(index - 1)
        };
        renderer.emit(prev, item);
    }
    if !renderer.out.is_empty() && !renderer.out.ends_with('\n') {
        renderer.out.push('\n');
    }
    renderer.out
}

impl Renderer {
    fn emit(&mut self, prev: Option<&Item>, item: &Item) {
        let sep = self.separator(prev, item);
        if item.piece == Piece::Code(TokenKind::RBrace) {
            self.indent = self.indent.saturating_sub(1);
            // Pop back to the brace; an unbalanced paren or angle left
            // open inside would otherwise misclassify every later comma.
            while let Some(top) = self.ctx.pop() {
                if matches!(top, Ctx::Brace(_)) {
                    break;
                }
            }
        }
        match sep {
            Sep::Newline { blank } => {
                self.out.push('\n');
                if blank {
                    self.out.push('\n');
                }
                // A directive goes to column zero: the lexer only reads
                // `#` as a directive at the start of its line.
                if item.piece != Piece::Directive {
                    for _ in 0..self.indent {
                        self.out.push_str(INDENT);
                    }
                }
            }
            Sep::Space => self.out.push(' '),
            Sep::None => {}
        }
        self.out.push_str(&item.text);
        self.update(item);
    }

    /// What goes between the previous item and this one.
    fn separator(&self, prev: Option<&Item>, item: &Item) -> Sep {
        let Some(prev) = prev else {
            return Sep::None;
        };
        // `{}` and `};` stay together.
        if prev.piece == Piece::Code(TokenKind::LBrace)
            && item.piece == Piece::Code(TokenKind::RBrace)
        {
            return Sep::None;
        }

        let mut break_line = match prev.piece {
            Piece::Line | Piece::Directive => true,
            // A block comment that stood on its own line keeps its line.
            Piece::Block => item.newline_before,
            Piece::Code(kind) => match kind {
                TokenKind::Semicolon | TokenKind::LBrace => true,
                TokenKind::RBrace => item.piece != Piece::Code(TokenKind::Semicolon),
                TokenKind::Comma => {
                    matches!(self.ctx.last(), Some(Ctx::Brace(BraceKind::List)))
                }
                _ => false,
            },
        };
        break_line |= match item.piece {
            Piece::Directive | Piece::Code(TokenKind::RBrace) => true,
            Piece::Line | Piece::Block => item.newline_before,
            Piece::Code(_) => false,
        };
        // A comment the source attached to the line stays attached — it
        // annotates what stands before it, and moving it to the next
        // line would re-address it.
        if matches!(item.piece, Piece::Line | Piece::Block)
            && !item.newline_before
            && !matches!(prev.piece, Piece::Line | Piece::Directive)
        {
            break_line = false;
        }
        if break_line {
            return Sep::Newline {
                blank: item.blank_before,
            };
        }
        if self.gap(prev, item) {
            Sep::Space
        } else {
            Sep::None
        }
    }

    /// Whether two same-line items get a space between them.
    fn gap(&self, prev: &Item, item: &Item) -> bool {
        let next_kind = match item.piece {
            // A comment attached to the line gets a space before it.
            Piece::Line | Piece::Block | Piece::Directive => return true,
            Piece::Code(kind) => kind,
        };
        let prev_kind = match prev.piece {
            // After an inline block comment, closers still hug.
            Piece::Block => {
                return !matches!(
                    next_kind,
                    TokenKind::Semicolon
                        | TokenKind::Comma
                        | TokenKind::RParen
                        | TokenKind::RBracket
                );
            }
            Piece::Line | Piece::Directive => return true,
            Piece::Code(kind) => kind,
        };
        self.code_gap(prev_kind, next_kind)
    }

    #[allow(clippy::too_many_lines)]
    fn code_gap(&self, prev: TokenKind, next: TokenKind) -> bool {
        use TokenKind::{
            At, Colon, ColonColon, Comma, Context, Getraises, Greater, GreaterGreater, Ident,
            LBracket, LParen, Less, Minus, Plus, RBracket, RParen, Raises, Semicolon, Setraises,
            Switch, Tilde,
        };
        // Closers and separators hug what they close.
        if matches!(next, Semicolon | Comma) {
            return false;
        }
        if prev == LParen || next == RParen {
            return false;
        }
        if prev == LBracket || next == RBracket {
            return false;
        }
        // `@name`, `a::b`.
        if prev == At || prev == ColonColon {
            return false;
        }
        if next == ColonColon {
            // `m::T` continues a name; `in ::T` starts a rooted one.
            return prev != Ident;
        }
        // Type arguments: `sequence<long, 4>`.
        if next == Less {
            return !(prev == Ident || prev.is_keyword());
        }
        if prev == Less {
            return false;
        }
        // `> >` must not fuse: the lexer reads `>>` as one token, so
        // gluing two closers would change the token stream — which the
        // verifier would refuse. The source's own `>>` stays `>>`.
        if prev == Greater && matches!(next, Greater | GreaterGreater) {
            return true;
        }
        if matches!(next, Greater | GreaterGreater) && self.ctx.last() == Some(&Ctx::Angle) {
            return false;
        }
        // `f(x)` and `@id(1)` against `switch (long)` and `raises (E)`.
        if next == LParen {
            if matches!(prev, Switch | Raises | Getraises | Setraises | Context) {
                return true;
            }
            return !(prev == Ident || prev.is_keyword());
        }
        // Array declarators: `long x[3][4]`.
        if next == LBracket {
            return !(prev == Ident || prev == RBracket);
        }
        // `case 1:` hugs its label; `struct D : Base` does not.
        if next == Colon {
            return !self.label_pending;
        }
        if prev == Colon {
            return true;
        }
        // `-1` after `=`, `(`, a label colon or another operator.
        if matches!(prev, Plus | Minus | Tilde) && self.last_unary {
            return false;
        }
        true
    }

    /// Fold one emitted item into the state the next decisions read.
    fn update(&mut self, item: &Item) {
        let Piece::Code(kind) = item.piece else {
            return;
        };
        match kind {
            TokenKind::LParen => self.ctx.push(Ctx::Paren),
            TokenKind::RParen => {
                if self.ctx.last() == Some(&Ctx::Paren) {
                    self.ctx.pop();
                }
            }
            TokenKind::Less => self.ctx.push(Ctx::Angle),
            TokenKind::Greater => {
                if self.ctx.last() == Some(&Ctx::Angle) {
                    self.ctx.pop();
                }
            }
            // `>>` closes two type-argument lists at once when it is
            // inside them, and is a shift operator when it is not.
            TokenKind::GreaterGreater => {
                for _ in 0..2 {
                    if self.ctx.last() == Some(&Ctx::Angle) {
                        self.ctx.pop();
                    }
                }
            }
            TokenKind::LBrace => {
                self.ctx
                    .push(Ctx::Brace(self.pending_brace.unwrap_or(BraceKind::Decls)));
                self.pending_brace = None;
                self.indent += 1;
                self.label_pending = false;
            }
            // The pop and dedent for `}` happen in `emit`, before the
            // indentation of its own line is written.
            TokenKind::RBrace => self.label_pending = false,
            TokenKind::Enum | TokenKind::Bitmask => {
                self.pending_brace = Some(BraceKind::List);
            }
            TokenKind::Module
            | TokenKind::Struct
            | TokenKind::Union
            | TokenKind::Interface
            | TokenKind::Exception
            | TokenKind::Valuetype
            | TokenKind::Eventtype
            | TokenKind::Component
            | TokenKind::Home
            | TokenKind::Porttype
            | TokenKind::Connector
            | TokenKind::Bitset
            | TokenKind::Annotation => {
                self.pending_brace = Some(BraceKind::Decls);
            }
            TokenKind::Case | TokenKind::Default => self.label_pending = true,
            TokenKind::Colon | TokenKind::Semicolon => self.label_pending = false,
            _ => {}
        }
        self.last_unary = matches!(kind, TokenKind::Plus | TokenKind::Minus | TokenKind::Tilde)
            && unary_position(self.last_code);
        self.last_code = Some(kind);
    }
}

/// Whether an operator after `before` is unary rather than binary.
fn unary_position(before: Option<TokenKind>) -> bool {
    let Some(kind) = before else {
        return true;
    };
    matches!(
        kind,
        TokenKind::Equals
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::LBrace
            | TokenKind::Less
            | TokenKind::LessLess
            | TokenKind::Greater
            | TokenKind::GreaterGreater
            | TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::Case
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Tilde
            | TokenKind::Pipe
            | TokenKind::Caret
            | TokenKind::Ampersand
    )
}

// ---------------------------------------------------------------------
// The guarantee: what would have been lost refuses the file.
// ---------------------------------------------------------------------

/// Check that `formatted` still carries the input, and only then let it
/// out.
///
/// The renderer is built never to drop an item, but a guarantee that is
/// only a reading of the code is a guarantee until the next edit. This
/// lexes the output and compares it item for item — syntax tokens,
/// comments and directives alike — and renders it once more to hold the
/// idempotence. Either failure refuses the file by name; neither ships
/// an output missing something.
fn verify(input: &[Item], formatted: &str) -> Result<(), Vec<Diagnostic>> {
    let output = stream(formatted);
    for (index, expected) in input.iter().enumerate() {
        let got = output.get(index);
        if got.is_none_or(|g| g.piece != expected.piece || g.text != expected.text) {
            return Err(vec![refusal(
                expected.span,
                format!(
                    "refusing to format: {} `{}` would not survive reformatting; the file keeps \
                     its original form",
                    describe(expected.piece),
                    expected.text
                ),
            )]);
        }
    }
    if let Some(extra) = output.get(input.len()) {
        return Err(vec![refusal(
            Span::new(0, 0),
            format!(
                "refusing to format: reformatting would introduce {} `{}` that the input does \
                 not have",
                describe(extra.piece),
                extra.text
            ),
        )]);
    }
    if render(&output) != formatted {
        return Err(vec![refusal(
            Span::new(0, 0),
            String::from(
                "refusing to format: the rendered form is not idempotent for this file; the \
                 file keeps its original form",
            ),
        )]);
    }
    Ok(())
}

fn describe(piece: Piece) -> &'static str {
    match piece {
        Piece::Code(_) => "the token",
        Piece::Line | Piece::Block => "the comment",
        Piece::Directive => "the preprocessor directive",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn format(source: &str) -> String {
        canonical(source).expect("formats")
    }

    #[test]
    fn the_layout_is_the_fixed_one() {
        let got = format("module  m{ @final   struct S{long a;sequence<long,3>b;};};");
        assert_eq!(
            got,
            "module m {\n    @final struct S {\n        long a;\n        sequence<long, 3> \
             b;\n    };\n};\n"
        );
    }

    #[test]
    fn an_enum_takes_one_line_per_enumerator_and_declarators_share_one() {
        assert_eq!(format("enum E { A, B };"), "enum E {\n    A,\n    B\n};\n");
        assert_eq!(
            format("struct S { long a, b; };"),
            "struct S {\n    long a, b;\n};\n"
        );
    }

    #[test]
    fn a_union_keeps_its_labels_on_the_member_line() {
        let got = format("union U switch (long) { case 1: long a; default: string b; };");
        assert_eq!(
            got,
            "union U switch (long) {\n    case 1: long a;\n    default: string b;\n};\n"
        );
    }

    /// The call of a function-like macro is no IDL syntax before its
    /// expansion; a file `check` accepts after the preprocessor is laid
    /// out with the call as the tokens it was written as.
    #[test]
    fn a_function_like_macro_call_keeps_its_tokens() {
        let source = "#define TWICE(x) ((x) * 2)\nconst long K = TWICE(3);\n\
                      #define CAT(a, b) a##b\nstruct CAT(Po, int) { long x; };\n";
        assert!(
            canonical(source).is_err(),
            "the source as written does not parse"
        );
        let got = canonical_of_checked(source).expect("formats");
        assert_eq!(
            got,
            "#define TWICE(x) ((x) * 2)\nconst long K = TWICE(3);\n#define CAT(a, b) a##b\n\
             struct CAT(Po, int) {\n    long x;\n};\n"
        );
    }

    #[test]
    fn spellings_are_never_normalised() {
        let got = format("const long C = 0x1F;");
        assert!(got.contains("0x1F"), "{got}");
    }

    /// The model does not carry comments; the stream does, so they are
    /// printed — a trailing one on its line, a leading one on its own.
    #[test]
    fn comments_are_printed_not_dropped() {
        let got = format("// leading\nstruct S {\n    long a; // trailing\n};\n");
        assert_eq!(got, "// leading\nstruct S {\n    long a; // trailing\n};\n");
    }

    #[test]
    fn a_preprocessor_directive_keeps_its_line_at_column_zero() {
        let got = format("#define DEPTH 3\nstruct S { long a; };");
        assert_eq!(got, "#define DEPTH 3\nstruct S {\n    long a;\n};\n");
    }

    #[test]
    fn one_blank_line_survives_and_more_collapse_to_one() {
        let got = format("struct A { long x; };\n\n\n\nstruct B { long y; };");
        assert_eq!(
            got,
            "struct A {\n    long x;\n};\n\nstruct B {\n    long y;\n};\n"
        );
    }

    #[test]
    fn a_negative_literal_hugs_its_sign_and_a_difference_does_not() {
        let got = format("const long A=-1;const long B=A-1;");
        assert_eq!(got, "const long A = -1;\nconst long B = A - 1;\n");
    }

    /// Two separate `>` tokens stay two tokens: fused they would lex
    /// back as one `>>`, and the token stream is the round trip. The
    /// source's own `>>` is one token already and stays glued.
    #[test]
    fn nested_closers_keep_the_space_that_keeps_them_two_tokens() {
        let got = format("typedef sequence<sequence<long> > Nested;");
        assert!(got.contains("<long> >"), "{got}");
        let got = format("typedef sequence<sequence<long>> Nested;");
        assert!(got.contains("<long>>"), "{got}");
    }

    #[test]
    fn a_file_with_parse_errors_is_refused_by_name() {
        let diagnostics = canonical("struct S { long }; ").expect_err("refused");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.is_error() && d.message.contains("refusing to format")),
            "{diagnostics:?}"
        );
    }

    /// The guarantee itself: an output missing a comment is caught and
    /// turned into a refusal that names the comment, not shipped.
    #[test]
    fn a_dropped_comment_is_a_named_refusal_never_a_silent_output() {
        let input = stream("// held\nstruct S {\n    long a;\n};\n");
        let without = "struct S {\n    long a;\n};\n";
        let diagnostics = verify(&input, without).expect_err("caught");
        assert!(
            diagnostics[0].message.contains("// held"),
            "{diagnostics:?}"
        );
        assert!(diagnostics[0].is_error());
    }

    #[test]
    fn formatting_twice_changes_nothing() {
        let sources = [
            "module m { struct S { @key long id; string<32> name; }; };",
            "enum Color { RED, GREEN, BLUE };",
            "// top\n#include \"other.idl\"\nstruct S { long a; /* mid */ long b; };",
            "union U switch (short) { case 1: case 2: long a; default: octet b; };",
            "interface I { readonly attribute long temp, humid; void f(in long x); };",
        ];
        for source in sources {
            let once = format(source);
            assert_eq!(format(&once), once, "{source}");
        }
    }

    // -----------------------------------------------------------------
    // The corpus: every IDL file the repository has.
    // -----------------------------------------------------------------

    fn idl_files_under(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                idl_files_under(&path, out);
            } else if path.extension().is_some_and(|e| e == "idl") {
                out.push(path);
            }
        }
    }

    /// Every `type.idl` of the interop corpus and every IDL fixture of
    /// the frontend formats idempotently and round-trips: same syntax
    /// tokens, same comments, same model. A refusal is only legitimate
    /// for a file that does not parse cleanly in the first place.
    #[test]
    fn the_corpus_formats_idempotently_and_round_trips() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        idl_files_under(&repo.join("tests/interop-corpus"), &mut files);
        idl_files_under(&repo.join("crates/idl-parse/tests/fixtures"), &mut files);
        assert!(
            files.len() >= 400,
            "the corpus is present ({} files found)",
            files.len()
        );

        let mut formatted = 0usize;
        let mut refused = 0usize;
        for path in files {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            match canonical(&text) {
                Ok(out) => {
                    let again = canonical(&out)
                        .unwrap_or_else(|e| panic!("{}: reformat refused: {e:?}", path.display()));
                    assert_eq!(again, out, "{}: not idempotent", path.display());

                    // `canonical` has already verified the token stream;
                    // hold the model too, span-free through the dump.
                    let before = zerodds_idl_lower::lower(
                        &zerodds_idl_parse::parse(&text).syntax(),
                        "corpus.idl",
                    )
                    .ast;
                    let after = zerodds_idl_lower::lower(
                        &zerodds_idl_parse::parse(&out).syntax(),
                        "corpus.idl",
                    )
                    .ast;
                    assert_eq!(
                        crate::dump::tree(&before),
                        crate::dump::tree(&after),
                        "{}: the reparsed model differs",
                        path.display()
                    );
                    assert_eq!(
                        crate::dump::dependencies(&before),
                        crate::dump::dependencies(&after),
                        "{}: the reparsed dependencies differ",
                        path.display()
                    );
                    formatted += 1;
                }
                Err(diagnostics) => {
                    assert!(
                        zerodds_idl_parse::parse(&text).has_errors(),
                        "{}: refused without parse errors: {diagnostics:?}",
                        path.display()
                    );
                    refused += 1;
                }
            }
        }
        assert!(
            formatted >= 400,
            "{formatted} formatted, {refused} refused — the corpus should format"
        );
    }
}
