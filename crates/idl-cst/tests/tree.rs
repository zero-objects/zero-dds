// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The green/red tree. Spec §2.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::Arc;

use zerodds_idl_cst::{
    CstKind, GreenChild, GreenNode, GreenToken, Span, SyntaxNode, SyntaxSpans, TokenKind,
    TreeBuilder, TriviaKind, lex,
};

/// Build a tree that covers the whole source, the way the parser will:
/// every token, trivia included, becomes a leaf. Nothing here parses —
/// this is the losslessness mechanism on its own.
fn flat_tree(source: &str) -> Arc<GreenNode> {
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    for token in lex(source) {
        for trivia in &token.leading {
            builder.token(
                TokenKind::Error, // trivia has no token kind of its own here
                trivia.span.slice(source).unwrap_or(""),
            );
        }
        builder.token(token.kind, token.span.slice(source).unwrap_or(""));
    }
    builder.build().expect("a complete tree")
}

// -- the property the tree exists for ------------------------------------

#[test]
fn a_flat_tree_reproduces_its_source_byte_for_byte() {
    let source = "module Robot { struct Pose { long x; }; };";
    assert_eq!(flat_tree(source).text(), source);
}

#[test]
fn a_tree_over_comments_and_spacing_reproduces_them_too() {
    let source = "// leading\nmodule  R {\n  /* inner */\n};\n// trailing\n";
    assert_eq!(flat_tree(source).text(), source);
}

#[test]
fn a_tree_over_a_file_that_would_not_parse_still_round_trips() {
    // Spec §2.2: recovery inserts nodes rather than dropping input, so a
    // broken file is still reconstructible.
    let source = "struct { ??? \"unterminated\n};;;";
    assert_eq!(flat_tree(source).text(), source);
}

#[test]
fn an_empty_source_yields_an_empty_tree() {
    let tree = flat_tree("");
    assert_eq!(tree.text(), "");
    assert_eq!(tree.width(), 0);
}

// -- widths --------------------------------------------------------------

#[test]
fn a_nodes_width_is_the_sum_of_its_children() {
    let inner = Arc::new(GreenNode::new(
        CstKind::NamedType,
        vec![GreenChild::Token(GreenToken::new(TokenKind::Ident, "Pose"))],
    ));
    let outer = GreenNode::new(
        CstKind::Member,
        vec![
            GreenChild::Node(inner),
            GreenChild::Token(GreenToken::new(TokenKind::Ident, " x")),
            GreenChild::Token(GreenToken::new(TokenKind::Semicolon, ";")),
        ],
    );
    assert_eq!(outer.width(), 4 + 2 + 1);
    assert_eq!(outer.text(), "Pose x;");
}

#[test]
fn width_matches_the_text_length_for_every_node_of_a_real_file() {
    let source = "module R { struct S { long x; }; };";
    let root = Arc::new(SyntaxNode::root(flat_tree(source)));
    for node in root.descendants() {
        assert_eq!(
            node.text().len(),
            node.span().len() as usize,
            "{:?} disagrees with its span",
            node.kind()
        );
    }
}

// -- sharing -------------------------------------------------------------

#[test]
fn identical_subtrees_can_be_one_allocation() {
    // A green node knows no position, which is exactly what allows this.
    let shared = Arc::new(GreenNode::new(
        CstKind::PrimitiveType,
        vec![GreenChild::Token(GreenToken::new(TokenKind::Long, "long"))],
    ));
    let parent = GreenNode::new(
        CstKind::File,
        vec![
            GreenChild::Node(shared.clone()),
            GreenChild::Token(GreenToken::new(TokenKind::Comma, ",")),
            GreenChild::Node(shared.clone()),
        ],
    );
    assert_eq!(parent.text(), "long,long");
    assert_eq!(Arc::strong_count(&shared), 3);
}

#[test]
fn two_separately_built_identical_subtrees_compare_equal() {
    let build = || {
        GreenNode::new(
            CstKind::PrimitiveType,
            vec![GreenChild::Token(GreenToken::new(TokenKind::Long, "long"))],
        )
    };
    assert_eq!(build(), build());
}

// -- red nodes -----------------------------------------------------------

#[test]
fn a_child_knows_its_offset_and_its_parent() {
    let inner = Arc::new(GreenNode::new(
        CstKind::NamedType,
        vec![GreenChild::Token(GreenToken::new(TokenKind::Ident, "Pose"))],
    ));
    let root = Arc::new(SyntaxNode::root(Arc::new(GreenNode::new(
        CstKind::File,
        vec![
            GreenChild::Token(GreenToken::new(TokenKind::Struct, "struct ")),
            GreenChild::Node(inner),
        ],
    ))));

    let children = root.children();
    assert_eq!(children.len(), 1);
    let named = &children[0];
    // "struct " is seven bytes, so the named type starts at seven.
    assert_eq!(named.span().start, 7);
    assert_eq!(named.text(), "Pose");
    assert!(named.parent().is_some());
}

#[test]
fn a_node_knows_its_ancestors_up_to_the_root() {
    let leaf = Arc::new(GreenNode::new(
        CstKind::NamedType,
        vec![GreenChild::Token(GreenToken::new(TokenKind::Ident, "X"))],
    ));
    let middle = Arc::new(GreenNode::new(
        CstKind::Member,
        vec![GreenChild::Node(leaf)],
    ));
    let root = Arc::new(SyntaxNode::root(Arc::new(GreenNode::new(
        CstKind::File,
        vec![GreenChild::Node(middle)],
    ))));

    let member = root.children().first().cloned().expect("a member");
    let named = member.children().first().cloned().expect("a named type");
    let ancestors: Vec<CstKind> = named.ancestors().iter().map(|n| n.kind()).collect();
    assert_eq!(ancestors, vec![CstKind::Member, CstKind::File]);
}

#[test]
fn offsets_stay_consistent_under_nesting() {
    // Each nested level shifts by the tokens before it; the child's
    // offset has to reflect that without the green node knowing it.
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    builder.token(TokenKind::Ident, "aa");
    builder.start(CstKind::Member);
    builder.token(TokenKind::Ident, "bbb");
    builder.start(CstKind::NamedType);
    builder.token(TokenKind::Ident, "cccc");
    builder.finish();
    builder.finish();
    builder.finish();

    let root = Arc::new(SyntaxNode::root(builder.build().expect("tree")));
    let member = root.children().first().cloned().expect("member");
    assert_eq!(member.span().start, 2);
    let named = member.children().first().cloned().expect("named");
    assert_eq!(named.span().start, 5);
    assert_eq!(named.text(), "cccc");
}

#[test]
fn tokens_report_their_own_positions() {
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    builder.token(TokenKind::Struct, "struct");
    builder.token(TokenKind::Ident, " S");
    builder.finish();
    let root = SyntaxNode::root(builder.build().expect("tree"));

    let tokens = root.tokens();
    assert_eq!(tokens[0].0.start, 0);
    assert_eq!(tokens[1].0.start, 6);
    assert_eq!(tokens[1].1.text(), " S");
}

// -- error nodes ---------------------------------------------------------

#[test]
fn an_error_node_is_an_ordinary_node_that_still_carries_its_text() {
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    builder.start(CstKind::Error);
    builder.token(TokenKind::Error, "???");
    builder.finish();
    builder.finish();

    let root = Arc::new(SyntaxNode::root(builder.build().expect("tree")));
    assert!(root.has_errors());
    assert_eq!(root.text(), "???");
}

#[test]
fn a_tree_without_errors_reports_none() {
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    builder.token(TokenKind::Struct, "struct");
    builder.finish();
    let root = Arc::new(SyntaxNode::root(builder.build().expect("tree")));
    assert!(!root.has_errors());
}

// -- the builder ---------------------------------------------------------

#[test]
fn the_builder_closes_nodes_left_open_rather_than_losing_them() {
    // A parser defect should not cost the input.
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    builder.start(CstKind::Module);
    builder.token(TokenKind::Ident, "R");
    // Two `start`, one missing `finish`.
    let tree = builder.build().expect("a tree even so");
    assert_eq!(tree.text(), "R");
}

#[test]
fn the_builder_reports_its_depth() {
    let mut builder = TreeBuilder::new();
    assert_eq!(builder.depth(), 0);
    builder.start(CstKind::File);
    assert_eq!(builder.depth(), 1);
    builder.start(CstKind::Module);
    assert_eq!(builder.depth(), 2);
    builder.finish();
    assert_eq!(builder.depth(), 1);
}

// -- depth ---------------------------------------------------------------

#[test]
fn reconstruction_does_not_depend_on_stack_depth() {
    // The grammar puts no bound on sequence<sequence<...>>, so text()
    // must not either. A recursive implementation overflows here.
    let mut inner = Arc::new(GreenNode::new(
        CstKind::PrimitiveType,
        vec![GreenChild::Token(GreenToken::new(TokenKind::Long, "long"))],
    ));
    for _ in 0..50_000 {
        inner = Arc::new(GreenNode::new(
            CstKind::SequenceType,
            vec![GreenChild::Node(inner)],
        ));
    }
    assert_eq!(inner.text(), "long");
    assert_eq!(inner.width(), 4);
}

proptest::proptest! {
    /// However the tokens fall, a flat tree over them reproduces the
    /// source.
    #[test]
    fn any_source_round_trips_through_a_flat_tree(source in "\\PC{0,200}") {
        proptest::prop_assert_eq!(flat_tree(&source).text(), source);
    }
}

// -- the trivia-free view of a span --------------------------------------

/// The same flat tree, but with trivia carried under the token kinds the
/// lexer names it by. `flat_tree` files trivia under `TokenKind::Error`,
/// which `is_trivia` does not answer to; `SyntaxSpans` reads exactly
/// that answer, so it needs the real kinds.
fn trivia_tree(source: &str) -> Arc<SyntaxNode> {
    let mut builder = TreeBuilder::new();
    builder.start(CstKind::File);
    for token in lex(source) {
        for trivia in &token.leading {
            let kind = match trivia.kind {
                TriviaKind::Whitespace | TriviaKind::Newline => TokenKind::Whitespace,
                TriviaKind::LineComment => TokenKind::LineComment,
                TriviaKind::BlockComment => TokenKind::BlockComment,
                TriviaKind::Directive => TokenKind::Directive,
            };
            builder.token(kind, trivia.span.slice(source).unwrap_or(""));
        }
        builder.token(token.kind, token.span.slice(source).unwrap_or(""));
    }
    Arc::new(SyntaxNode::root(builder.build().expect("a complete tree")))
}

/// What `trim` makes of the range `text` occupies in `source`.
fn trimmed(source: &str, text: &str) -> String {
    let start = u32::try_from(source.find(text).expect("the fixture holds it")).expect("small");
    let span = Span::new(start, start + u32::try_from(text.len()).expect("small"));
    SyntaxSpans::of(&trivia_tree(source))
        .trim(span)
        .slice(source)
        .expect("in bounds")
        .to_string()
}

#[test]
fn a_leading_newline_and_indent_are_not_part_of_what_a_span_names() {
    let source = "struct S {\n    long a,\n         b;\n};\n";
    assert_eq!(trimmed(source, "\n         b"), "b");
}

#[test]
fn a_leading_comment_is_not_part_of_it_either() {
    let source = "struct S {\n    // why\n    long a;\n};\n";
    assert_eq!(trimmed(source, "\n    // why\n    long a"), "long a");
}

#[test]
fn a_span_that_already_starts_at_its_text_is_left_alone() {
    let source = "struct S {\n    long a;\n};\n";
    assert_eq!(trimmed(source, "long a"), "long a");
}

#[test]
fn trailing_trivia_is_dropped_from_the_end() {
    let source = "struct S {\n    long a;\n};\n";
    assert_eq!(trimmed(source, "long a;\n"), "long a;");
}

/// An empty span points at a place rather than at text, and there is no
/// text to narrow it to. Moving it to the next token would make it name
/// a declaration that has nothing to do with the message.
#[test]
fn an_empty_span_stays_where_it_is() {
    let source = "struct S {\n    long a;\n};\n";
    let index = SyntaxSpans::of(&trivia_tree(source));
    assert_eq!(index.trim(Span::new(0, 0)), Span::new(0, 0));
    assert_eq!(index.trim(Span::new(12, 12)), Span::new(12, 12));
}

/// A range over nothing but whitespace has no text to name, so it keeps
/// the range it was given rather than jumping to the next declaration.
#[test]
fn a_span_of_trivia_only_keeps_its_range() {
    let source = "struct S {\n    long a;\n};\n";
    let at = u32::try_from(source.find('\n').expect("present")).expect("small");
    let span = Span::new(at, at + 3);
    assert_eq!(SyntaxSpans::of(&trivia_tree(source)).trim(span), span);
}

#[test]
fn an_index_over_an_empty_file_narrows_nothing() {
    let index = SyntaxSpans::of(&trivia_tree(""));
    assert_eq!(index.trim(Span::new(0, 4)), Span::new(0, 4));
}
