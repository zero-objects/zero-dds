// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The green/red tree. Spec §2.
//!
//! Green nodes are immutable, carry only kind and width, and are shared
//! through `Arc` — two identical subtrees can be one allocation. Red
//! nodes are built on demand and add what green cannot have without
//! losing sharing: an absolute offset and a parent.
//!
//! Trivia lives in the tree as ordinary tokens. That is what makes
//! `text()` reproduce the input rather than approximate it, and it is
//! why a green token carries its text instead of a span into some
//! particular file — a span would tie the tree to one source and
//! make sharing impossible.

use crate::span::Span;
use crate::token::TokenKind;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

/// What a syntax node is.
///
/// One flat enum over every form the grammar produces. Adding a form is
/// a variant, and every exhaustive match on it has to account for the
/// new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum CstKind {
    /// The whole file.
    File,

    // -- declarations ----------------------------------------------------
    /// `module X { ... };`
    Module,
    /// `const T N = expr;`
    ConstDecl,
    /// `typedef T N;`
    TypedefDecl,
    /// `struct S { ... };`
    StructDecl,
    /// `union U switch (T) { ... };`
    UnionDecl,
    /// `enum E { ... };`
    EnumDecl,
    /// `bitmask B { ... };`
    BitmaskDecl,
    /// `bitset B { ... };`
    BitsetDecl,
    /// `native N;`
    NativeDecl,
    /// `exception E { ... };`
    ExceptionDecl,
    /// `interface I { ... };`
    InterfaceDecl,
    /// `valuetype V { ... };`
    ValuetypeDecl,
    /// `valuetype VB long;` — a boxed value. IDL 4.2 §7.4.7 rule (126).
    ///
    /// Its own kind and not a `ValuetypeDecl` with a type beside it: a
    /// value box has no state members, no initialisers and no bases, and
    /// a consumer matching on the kind should not have to look for the
    /// absence of a body to tell the two apart.
    ValueBoxDecl,
    /// A forward declaration: `struct S;`
    ForwardDecl,
    /// `@annotation A { ... };`
    AnnotationDecl,
    /// `typeid X "id";` or `typeprefix X "prefix";`
    RepositoryDirective,
    /// `import X;`
    ImportDecl,
    /// `keylist T (a, b);` — an RTI Connext directive.
    KeylistDecl,
    /// The `<...>` formal parameter list of a template module.
    /// `module M<long> N;` — a template module instantiation.
    /// IDL 4.2 §7.4.12 rule (190).
    TemplateModuleInst,
    /// The actual parameters of an instantiation, rule (191). Each child
    /// is a type specification or a constant expression, rule (192).
    ActualParameters,
    /// `alias M<T> R;` inside a template module body, rule (193).
    TemplateModuleRef,
    /// The formal parameters of a template module, rule (186).
    TemplateParameters,
    /// One formal parameter of a template module.
    TemplateParameter,
    /// One port of a component: `provides`, `uses`, `emits`,
    /// `publishes`, `consumes`, `port` or `mirrorport`.
    PortDecl,
    /// A home's `manages X primarykey K` clause.
    ManagesClause,

    // -- members and parts -----------------------------------------------
    /// One member of a struct, exception or valuetype.
    Member,
    /// One `case` of a union, with its labels.
    UnionCase,
    /// A `case <expr>:` or `default:` label.
    CaseLabel,
    /// One enumerator.
    Enumerator,
    /// One bitmask bit value.
    BitValue,
    /// One bitset field.
    BitField,
    /// An operation.
    Operation,
    /// One parameter of an operation.
    Parameter,
    /// An attribute.
    AttributeDecl,
    /// A `raises (...)` clause.
    RaisesClause,
    /// A `context (...)` clause.
    ContextClause,
    /// An inheritance clause.
    InheritanceClause,

    // -- types -----------------------------------------------------------
    /// A named type reference.
    NamedType,
    /// A primitive keyword used as a type.
    PrimitiveType,
    /// `sequence<T>` or `sequence<T, N>`.
    SequenceType,
    /// `string` or `string<N>`.
    StringType,
    /// `wstring` or `wstring<N>`.
    WStringType,
    /// `map<K, V>` or `map<K, V, N>`.
    MapType,
    /// `fixed<d, s>`.
    FixedType,
    /// The `<...>` argument list of a parameterised type.
    TypeArguments,
    /// A declarator, simple or array.
    Declarator,
    /// One `[expr]` of an array declarator.
    ArrayDimension,

    // -- names and expressions -------------------------------------------
    /// A scoped name, possibly rooted at `::`.
    ScopedName,
    /// A literal used as an expression.
    LiteralExpr,
    /// A scoped name used as an expression.
    NameExpr,
    /// A unary application.
    UnaryExpr,
    /// A binary application.
    BinaryExpr,
    /// A parenthesised expression.
    ParenExpr,

    // -- annotations -----------------------------------------------------
    /// `@ann`, `@ann(v)` or `@ann(k = v, ...)`.
    Annotation,
    /// The parenthesised part of an annotation.
    AnnotationArguments,
    /// One `k = v` inside an annotation.
    AnnotationArgument,

    // -- recovery --------------------------------------------------------
    /// A range the parser could not make sense of.
    ///
    /// An ordinary node, not a separate channel: a file that fails to
    /// parse still round-trips because its bad ranges are in the tree
    /// rather than dropped. Spec §2.2, §3.2.
    Error,
}

impl CstKind {
    /// Whether this node is one the parser produced while recovering.
    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

/// A token in the green tree.
///
/// Carries its own text rather than a span, so an identical token in two
/// files is one value and identical subtrees can share an allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GreenToken {
    kind: TokenKind,
    text: Arc<str>,
}

impl GreenToken {
    /// A token of `kind` spelling `text`.
    #[must_use]
    pub fn new(kind: TokenKind, text: &str) -> Self {
        Self {
            kind,
            text: Arc::from(text),
        }
    }

    /// What it is.
    #[must_use]
    pub const fn kind(&self) -> TokenKind {
        self.kind
    }

    /// How it is spelled.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How many bytes it occupies.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn width(&self) -> u32 {
        self.text.len() as u32
    }
}

/// A child of a green node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GreenChild {
    /// A nested node.
    Node(Arc<GreenNode>),
    /// A token, which may be trivia.
    Token(GreenToken),
}

impl GreenChild {
    /// How many bytes it occupies.
    #[must_use]
    pub fn width(&self) -> u32 {
        match self {
            Self::Node(node) => node.width(),
            Self::Token(token) => token.width(),
        }
    }
}

/// An immutable node.
///
/// Knows its kind, its children and its total width — never its
/// position, because a node that knew where it was could not be shared
/// between two places.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GreenNode {
    kind: CstKind,
    width: u32,
    children: Vec<GreenChild>,
}

impl GreenNode {
    /// A node of `kind` over `children`, with the width they sum to.
    #[must_use]
    pub fn new(kind: CstKind, children: Vec<GreenChild>) -> Self {
        let width = children
            .iter()
            .fold(0u32, |sum, child| sum.saturating_add(child.width()));
        Self {
            kind,
            width,
            children,
        }
    }

    /// What it is.
    #[must_use]
    pub const fn kind(&self) -> CstKind {
        self.kind
    }

    /// Total bytes, trivia included.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Its children in order.
    #[must_use]
    pub fn children(&self) -> &[GreenChild] {
        &self.children
    }

    /// Reconstruct the source this node covers.
    ///
    /// The property the whole tree exists for: for the root, this equals
    /// the input byte for byte. Spec §2.2.
    ///
    /// Iterative rather than recursive. A deeply nested type — the
    /// grammar puts no bound on how far `sequence<sequence<...>>` can
    /// go — must not make reconstruction depend on stack depth, and this
    /// crate forbids panics, which a stack overflow would not respect.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::with_capacity(self.width as usize);
        let mut stack: Vec<&GreenChild> = self.children.iter().rev().collect();
        while let Some(child) = stack.pop() {
            match child {
                GreenChild::Token(token) => out.push_str(token.text()),
                GreenChild::Node(node) => stack.extend(node.children.iter().rev()),
            }
        }
        out
    }
}

/// Dropping a deeply nested tree must not depend on stack depth either.
///
/// `text()` is iterative, but the derived drop glue is not: releasing a
/// chain of fifty thousand nested `Arc<GreenNode>` recurses once per
/// level and aborts the process. Found by the depth test, which built
/// the tree successfully and then died tearing it down.
///
/// This unnests the children into a work list and releases them there,
/// so depth costs heap rather than stack. `Arc::into_inner` yields the
/// node only when this was the last reference, which is what keeps
/// shared subtrees alive.
impl Drop for GreenNode {
    fn drop(&mut self) {
        let mut stack: Vec<GreenChild> = core::mem::take(&mut self.children);
        while let Some(child) = stack.pop() {
            if let GreenChild::Node(node) = child {
                if let Some(mut inner) = Arc::into_inner(node) {
                    stack.append(&mut inner.children);
                }
            }
        }
    }
}

/// A node with a position.
///
/// Built on demand from a green node plus the offset it sits at and the
/// parent it hangs from. Cheap to make and cheap to drop; nothing caches
/// them.
#[derive(Debug, Clone)]
pub struct SyntaxNode {
    green: Arc<GreenNode>,
    offset: u32,
    parent: Option<Arc<SyntaxNode>>,
}

impl SyntaxNode {
    /// The root of a tree.
    #[must_use]
    pub const fn root(green: Arc<GreenNode>) -> Self {
        Self {
            green,
            offset: 0,
            parent: None,
        }
    }

    /// What it is.
    #[must_use]
    pub fn kind(&self) -> CstKind {
        self.green.kind()
    }

    /// Where it sits in the file, trivia included.
    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(self.offset, self.offset.saturating_add(self.green.width()))
    }

    /// The green node behind it.
    #[must_use]
    pub fn green(&self) -> &Arc<GreenNode> {
        &self.green
    }

    /// Its parent, if it is not the root.
    #[must_use]
    pub fn parent(&self) -> Option<&Arc<Self>> {
        self.parent.as_ref()
    }

    /// The chain from this node up to the root, this node first.
    #[must_use]
    pub fn ancestors(&self) -> Vec<Arc<Self>> {
        let mut out = Vec::new();
        let mut current = self.parent.clone();
        while let Some(node) = current {
            current = node.parent.clone();
            out.push(node);
        }
        out
    }

    /// Its child nodes, with their positions filled in.
    #[must_use]
    pub fn children(self: &Arc<Self>) -> Vec<Arc<Self>> {
        let mut out = Vec::new();
        let mut at = self.offset;
        for child in self.green.children() {
            match child {
                GreenChild::Node(green) => {
                    out.push(Arc::new(Self {
                        green: green.clone(),
                        offset: at,
                        parent: Some(self.clone()),
                    }));
                    at = at.saturating_add(green.width());
                }
                GreenChild::Token(token) => at = at.saturating_add(token.width()),
            }
        }
        out
    }

    /// Its direct tokens, with their positions.
    #[must_use]
    pub fn tokens(&self) -> Vec<(Span, GreenToken)> {
        let mut out = Vec::new();
        let mut at = self.offset;
        for child in self.green.children() {
            let width = child.width();
            if let GreenChild::Token(token) = child {
                out.push((Span::new(at, at.saturating_add(width)), token.clone()));
            }
            at = at.saturating_add(width);
        }
        out
    }

    /// The node's own tokens with the trivia left out.
    ///
    /// [`SyntaxNode::tokens`] keeps everything, because the round trip
    /// needs it. A rule asking what a node says wants this one: the
    /// first token of `@ key` is `@`, and the one after it is `key`,
    /// not the space between them.
    #[must_use]
    pub fn syntax_tokens(&self) -> Vec<(Span, GreenToken)> {
        self.tokens()
            .into_iter()
            .filter(|(_, token)| !token.kind().is_trivia())
            .collect()
    }

    /// Reconstruct the source this node covers.
    #[must_use]
    pub fn text(&self) -> String {
        self.green.text()
    }

    /// Every node in the subtree, this one first, in source order.
    #[must_use]
    pub fn descendants(self: &Arc<Self>) -> Vec<Arc<Self>> {
        let mut out = alloc::vec![self.clone()];
        let mut index = 0;
        while index < out.len() {
            let node = out[index].clone();
            out.extend(node.children());
            index += 1;
        }
        out
    }

    /// Whether this node or any below it came from recovery.
    #[must_use]
    pub fn has_errors(self: &Arc<Self>) -> bool {
        self.descendants().iter().any(|node| node.kind().is_error())
    }
}

impl fmt::Display for SyntaxNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text())
    }
}

/// A marked position among an open node's children.
///
/// Only meaningful to the builder that produced it and only while that
/// builder is at the same depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    depth: usize,
    children: usize,
}

/// Builds a green tree.
///
/// Nodes are opened and closed around the tokens they contain, so a
/// parser writes what it is doing rather than assembling children by
/// hand.
#[derive(Debug, Default)]
pub struct TreeBuilder {
    stack: Vec<(CstKind, Vec<GreenChild>)>,
    finished: Option<Arc<GreenNode>>,
}

impl TreeBuilder {
    /// An empty builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stack: Vec::new(),
            finished: None,
        }
    }

    /// Begin a node.
    pub fn start(&mut self, kind: CstKind) {
        self.stack.push((kind, Vec::new()));
    }

    /// Add a token to the node in progress.
    pub fn token(&mut self, kind: TokenKind, text: &str) {
        if let Some((_, children)) = self.stack.last_mut() {
            children.push(GreenChild::Token(GreenToken::new(kind, text)));
        }
    }

    /// End the node in progress, attaching it to its parent.
    pub fn finish(&mut self) {
        let Some((kind, children)) = self.stack.pop() else {
            return;
        };
        let node = Arc::new(GreenNode::new(kind, children));
        match self.stack.last_mut() {
            Some((_, siblings)) => siblings.push(GreenChild::Node(node)),
            None => self.finished = Some(node),
        }
    }

    /// The completed tree.
    ///
    /// `None` if a node was left open, which is a parser defect rather
    /// than a malformed input — every `start` has to have its `finish`.
    #[must_use]
    pub fn build(mut self) -> Option<Arc<GreenNode>> {
        while !self.stack.is_empty() {
            self.finish();
        }
        self.finished
    }

    /// How deep the builder currently is.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Mark the current position among the open node's children.
    ///
    /// Precedence climbing does not know it is building a binary
    /// expression until it has already parsed the left operand, so it
    /// needs to wrap children it has emitted. A checkpoint plus
    /// [`TreeBuilder::start_at`] is that, and it is why the builder
    /// exists rather than callers assembling children by hand.
    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            depth: self.stack.len(),
            children: self.stack.last().map_or(0, |(_, children)| children.len()),
        }
    }

    /// Wrap everything emitted since `checkpoint` in a new node.
    ///
    /// A checkpoint from a different nesting depth is ignored rather
    /// than acted on: acting on it would silently reparent a subtree.
    pub fn start_at(&mut self, checkpoint: Checkpoint, kind: CstKind) {
        if checkpoint.depth != self.stack.len() {
            self.start(kind);
            return;
        }
        let Some((_, children)) = self.stack.last_mut() else {
            self.start(kind);
            return;
        };
        if checkpoint.children > children.len() {
            self.start(kind);
            return;
        }
        let wrapped: Vec<GreenChild> = children.split_off(checkpoint.children);
        self.stack.push((kind, wrapped));
    }
}

/// Where a file's syntax is, with the trivia left out.
///
/// A node's [`span`](SyntaxNode::span) covers the trivia too, because
/// the round trip needs it: the lexer attaches whitespace and comments
/// leading, so the blank line and the indentation before a declaration
/// belong to the node that follows them. That is right for
/// reconstruction and wrong for a diagnostic — the span of the second
/// declarator of `long a,\n         b;` begins at the newline after the
/// comma, so a message about `b` names the line before it.
///
/// This is the index that turns such a span back into the text it names:
/// the first significant token at or after the start, and the last one
/// ending at or before the end. A span that holds no significant token
/// at all — an empty one, or one covering only trivia — is returned
/// unchanged rather than moved somewhere it did not point.
#[derive(Debug, Clone, Default)]
pub struct SyntaxSpans {
    /// Every non-trivia token's range, ascending. Tokens do not overlap,
    /// so this is sorted by `start` and by `end` at once.
    spans: Vec<Span>,
}

impl SyntaxSpans {
    /// Index the subtree under `root`.
    ///
    /// Iterative for the same reason [`GreenNode::text`] is: the grammar
    /// puts no bound on nesting, and this crate forbids panics, which a
    /// stack overflow would not respect.
    #[must_use]
    pub fn of(root: &Arc<SyntaxNode>) -> Self {
        let mut spans = Vec::new();
        let mut stack: Vec<(&GreenNode, u32)> = alloc::vec![(root.green(), root.span().start)];
        while let Some((green, offset)) = stack.pop() {
            let mut at = offset;
            for child in green.children() {
                let width = child.width();
                match child {
                    GreenChild::Token(token) => {
                        if !token.kind().is_trivia() {
                            spans.push(Span::new(at, at.saturating_add(width)));
                        }
                    }
                    GreenChild::Node(node) => stack.push((node, at)),
                }
                at = at.saturating_add(width);
            }
        }
        spans.sort_unstable_by_key(|span| span.start);
        Self { spans }
    }

    /// `span` narrowed to the text it covers.
    #[must_use]
    pub fn trim(&self, span: Span) -> Span {
        if span.is_empty() {
            return span;
        }
        let first = self.spans.partition_point(|token| token.start < span.start);
        let Some(head) = self.spans.get(first) else {
            return span;
        };
        if head.start >= span.end {
            return span;
        }
        let last = self.spans.partition_point(|token| token.end <= span.end);
        let end = match last.checked_sub(1).and_then(|index| self.spans.get(index)) {
            Some(tail) if tail.end > head.start => tail.end,
            _ => span.end,
        };
        Span::new(head.start, end)
    }
}
