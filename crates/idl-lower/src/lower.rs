// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The CST-to-model walk. Spec §4.
//!
//! Builds through the model's own edit surface, so a file that fails
//! validation leaves nothing behind. Module reopening across files
//! converges on one node through `Ast::ensure_module` — the frontend
//! needs no dedup pass, which is what building the middle first bought.

use crate::annotation::{Applied, UseSite};

/// One argument of a parameterised type, with the annotations written
/// immediately in front of it.
///
/// `sequence<@try_construct(TRIM) String5, 10>` puts both in the same
/// child list, and the pairing is what tells the element apart from the
/// bound.
type TemplateArgument = (
    alloc::vec::Vec<alloc::sync::Arc<SyntaxNode>>,
    alloc::sync::Arc<SyntaxNode>,
);
use crate::scope::ScopeStack;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use zerodds_ast::{
    AnyId, Ast, BinaryOp, Bound, CharWidth, Completeness, ConstExpr, ConstExprNode, Facets,
    InterfaceKind, Literal, MemberSpec, ParamDirection, PrimitiveKind, SourceFacet, SourceOrigin,
    TypeKind, TypeNode, UnaryOp, ValuetypeKind,
};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};
use zerodds_idl_cst::{CstKind, Span, SyntaxNode, SyntaxSpans, TokenKind};
use zerodds_idl_parse::{Diagnostic, DiagnosticCode};

/// What lowering produced.
pub struct Lowered {
    /// The model, built as far as the file allowed.
    pub ast: Ast,
    /// Everything wrong that the model itself did not already reject.
    pub diagnostics: Vec<Diagnostic>,
}

/// What the caller may decide that the source does not.
///
/// Empty by default: everything the IDL did not say stays unsaid in the
/// model, and whoever asks a question the source left open applies the
/// rule then. An option here is the caller overruling that, which the
/// model records as though the source had said it.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// What an aggregate of unspecified extensibility is taken to be.
    ///
    /// `None` leaves the facet absent, which keeps "the source said
    /// appendable" apart from "the source said nothing" and lets
    /// `Ast::effective_extensibility` and the layout key apply their
    /// own default. `Some` writes it into the model, so every consumer
    /// — the wire plan, the TypeObject, the generated code — sees one
    /// answer.
    ///
    /// XTypes 1.3 §7.3.1.2.1.8 provides for exactly this:
    ///
    /// > In the event that the representation of a given type does not
    /// > indicate the type's extensibility kind, the type shall be
    /// > considered appendable. Implementations may provide a mechanism
    /// > to override this default behavior; for example, IDL compilers
    /// > may provide configuration options to allow users to specify
    /// > whether types of unspecified extensibility are to be considered
    /// > final, appendable, or mutable.
    ///
    /// What the source *did* say still wins: an `@final` annotation, an
    /// `@extensibility(...)`, or a `#pragma` are all the source
    /// speaking, and this option only fills the silence.
    pub default_extensibility: Option<zerodds_ast::Extensibility>,
    /// The `@default_nested` of the top-level modules that carry no
    /// annotation of their own, and of types declared outside any module.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.7: "If a top-level module is not
    /// annotated, the default is FALSE. […] IDL compilers shall provide
    /// the means to change the default value for non-annotated top-level
    /// modules." `None` leaves the specification's FALSE; `Some` writes
    /// the answer onto the root module, where `Ast::effective_nested`
    /// finds it for every type below that no module annotated.
    pub default_nested: Option<bool>,
    /// Whether a `@hashid` member advances the sequential counter the
    /// next unannotated member is numbered from.
    ///
    /// `true` by default, and the default is a measurement rather than a
    /// preference. DDS-XTypes 1.3 §7.3.1.2.1.1 states the rule twice and
    /// not the same way: one sentence numbers a member from "the
    /// most-recently specified ID (using the @id annotation …)", which
    /// leaves a hashed id out of the count, and the next from "the
    /// member ID of the previous member of the same Aggregated Type plus
    /// one", which does not. Cyclone and Fast DDS take the second, RTI
    /// the first — `crates/ast-layout/tests/fixtures/vendor_bytes.md`
    /// §7. Three of the four against one.
    ///
    /// It is here and not in the layout key because member ids are fixed
    /// in [`crate::complete`], before any plan exists.
    /// `zerodds_ast_layout::Dialect::hashed_id_advances_the_counter` is
    /// where the vendor fact lives; this is the switch it sets.
    pub hashed_id_advances_the_counter: HashedIdCounter,
    /// Which spelling of an escaped name a member-id hash reads.
    ///
    /// IDL 4.2 §7.2.3.2 makes the escape underscore "a purely lexical
    /// convention" that is not part of the identifier, so the
    /// specification's hash input for `_long` is `long` — the default.
    /// RTI Connext hashes the spelling as written, keyword or not:
    /// measured with rtiddsgen 7.7.0, whose TypeCode carries member id
    /// 79659193 = hash("_long") and 230782087 = hash("_foo") where the
    /// clause gives 56906255 and 186170796. Like the counter above it
    /// is settled in [`crate::complete`], before any layout key exists;
    /// `zerodds_ast_layout::Dialect` carries the vendor fact.
    pub hashed_name_spelling: HashedNameSpelling,
    /// Whether `@key` on a union discriminator is carried into the
    /// model.
    ///
    /// A frontend axis in the shape [`HashedIdCounter`] has, and here
    /// for the reason that one is here: the answer is settled while the
    /// file is read, long before any plan exists, and two load paths
    /// that answer it differently produce two different
    /// `TypeIdentifier`s for one declaration.
    ///
    /// [`KeyOnUnionDiscriminator::Refused`] is the default and is what
    /// every measured front end does — see that type for the runs.
    pub key_on_union_discriminator: KeyOnUnionDiscriminator,
    /// Whether a declaration may carry the name of the scope it sits in.
    ///
    /// A dialect axis, measured and not chosen: the corpus case
    /// `resolve-module-named-as-type` — `module common { struct common
    /// { long x; }; ... }` — is refused by Cyclone DDS and OpenDDS and
    /// taken by Fast DDS and RTI Connext, and the ledger records four
    /// octet rows on each side of that split. A reader that refuses the
    /// file can never reproduce the octets the other two wrote, so the
    /// refusal cannot live in the front end alone.
    pub name_of_the_enclosing_scope: NameOfTheEnclosingScope,
    /// Which vendors' extensions this lowering admits. Spec §3.5.
    ///
    /// Two `#pragma` families change the model rather than decorate it:
    /// Cyclone's `#pragma keylist` and OpenDDS's `#pragma DCPS_DATA_TYPE`
    /// / `DCPS_DATA_KEY` both make members keys. They are in no building
    /// block of OMG IDL 4.2, so a file that needs one is a file another
    /// vendor's front end reads differently — and under
    /// `Dialect::STRICT` that is an error rather than a warning.
    ///
    /// Not part of the derived `Default`: see the `impl Default` below.
    pub dialect: zerodds_idl_parse::Dialect,
}

/// The literal form a constant expression settles to.
///
/// Only what the lowering can decide from the expression itself. It is
/// deliberately coarser than a type: the question here is IDL 4.2
/// §7.4.1.4.3's "consistent with the type declared for the constant",
/// not what the value's exact type is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueForm {
    /// `TRUE` / `FALSE`.
    Boolean,
    /// An integer literal.
    Integer,
    /// A floating-point literal.
    Float,
    /// A fixed-point literal.
    Fixed,
    /// A narrow character literal.
    Char,
    /// A wide character literal.
    WChar,
    /// A narrow string literal.
    String,
    /// A wide string literal.
    WString,
}

impl ValueForm {
    /// Whether arithmetic applies to it.
    const fn is_numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Float | Self::Fixed)
    }

    /// How the diagnostic names it.
    const fn describe(self) -> &'static str {
        match self {
            Self::Boolean => "a boolean",
            Self::Integer => "an integer literal",
            Self::Float => "a floating-point literal",
            Self::Fixed => "a fixed-point literal",
            Self::Char => "a character literal",
            Self::WChar => "a wide character literal",
            Self::String => "a string literal",
            Self::WString => "a wide string literal",
        }
    }
}

/// `Options::default()` admits every vendor extension.
///
/// Written out rather than derived, because `Dialect::default()` is
/// `STRICT` and the two defaults are deliberately opposite. A parser
/// pointed at a file somebody else wrote has to read it before it judges
/// it, so the lenient dialect is what a caller who did not think about
/// the question gets; `--strict` is the caller who did.
impl Default for Options {
    fn default() -> Self {
        Self {
            default_extensibility: None,
            default_nested: None,
            hashed_id_advances_the_counter: HashedIdCounter::default(),
            hashed_name_spelling: HashedNameSpelling::default(),
            key_on_union_discriminator: KeyOnUnionDiscriminator::default(),
            name_of_the_enclosing_scope: NameOfTheEnclosingScope::default(),
            dialect: zerodds_idl_parse::Dialect::ALL_VENDORS,
        }
    }
}

/// Whether a declaration may reuse the name of its immediate scope.
///
/// A named type for the reason [`HashedNameSpelling`] is one: the
/// default has to be the specification's reading rather than whatever
/// `false` happens to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NameOfTheEnclosingScope {
    /// Refused. IDL 4.2 §7.5.2: "The name of a module, structure,
    /// union, map, interface, value type, event type, exception or home
    /// may not be redefined within the immediate scope of" that same
    /// construct. Cyclone DDS and OpenDDS agree, and this is what a
    /// caller who did not ask for a vendor gets.
    #[default]
    Refused,
    /// Taken. Fast DDS and RTI Connext accept the declaration and write
    /// octets for it; a probe that reproduces either of them has to
    /// reach the same model they did.
    Admitted,
}

/// Which spelling of an escaped name a member-id hash reads.
///
/// A named type for the reason [`HashedIdCounter`] is one: `Options` is
/// `Default`, and the default has to be the specification's reading, not
/// whatever `false` happens to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashedNameSpelling {
    /// The identifier without its escape underscore — `long` for
    /// `_long`. IDL 4.2 §7.2.3.2's reading.
    #[default]
    Stripped,
    /// The spelling in the file — `_long`, `_foo`. RTI Connext's
    /// measured reading.
    AsWritten,
}

/// Whether a `@hashid` member advances the sequential counter.
///
/// A named type and not a bare `bool`, because `Options` is `Default`
/// and a boolean field defaulting to `false` would silently take RTI's
/// reading for every caller that did not think about it. This defaults
/// to the reading three of the four implementations take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashedIdCounter {
    /// The next unannotated member is the hashed id plus one. Cyclone,
    /// Fast DDS and the second of §7.3.1.2.1.1's two sentences.
    #[default]
    Advances,
    /// The hashed id is skipped: the next unannotated member is numbered
    /// from the last *declared* id, or from zero when there is none.
    /// RTI Connext, and the first of the two sentences.
    Skipped,
}

/// Whether `@key` on a union discriminator is carried into the model.
///
/// # The clause permits it
///
/// DDS-XTypes 1.3 Table 21 lists `@key` against "Structure Members,
/// Union discriminator", and §7.2.2.4.4.4.8's own example is
///
/// ```idl
/// union MyCommand switch (@key CommandKind) { /* … */ };
/// ```
///
/// so the default is to carry it: a front end that refuses the
/// declaration, or tells the author the annotation "has no meaning"
/// there, states the opposite of the specification.
///
/// # What the implementations do
///
/// Measured on the four toolchains the Linux test host carries:
///
/// | | `union U switch (@key uint32)` |
/// |---|---|
/// | Cyclone `idlc` | refuses the file — "Using union type as part of the key is currently unsupported" |
/// | `rtiddsgen` | refuses it at the discriminator — "@key is not supported" |
/// | `fastddsgen` | generates no `PubSubType` for a union at all, so it has no keyed topic to compare against |
/// | `opendds_idl` | not recorded in that run |
///
/// So under [`Self::Carried`] this project computes a KeyHash over a
/// union discriminator that no measured peer computes. [`Self::Refused`]
/// is the setting for a tool that has to stay within what those
/// toolchains accept; it names the clause that permits the declaration
/// in its diagnostic.
///
/// # Why the OMG suite needs it
///
/// `union_uint32_one_key` pairs `unions.xml`'s `union_uint32` against
/// `unions_key_discriminator.xml`'s `union_uint32_key`, whose
/// discriminator carries `key="true"`, and expects `INCONSISTENT_TOPIC`
/// on both sides: §7.2.4.4.4.5 requires that "either the discriminators
/// of both T1 and T2 are keys or neither are keys". Under
/// [`Self::Refused`] the subscriber never creates the topic and the case
/// reports `TOPIC_NOT_CREATED`; under [`Self::Carried`] the declaration
/// loads, the discriminator's `IS_KEY` reaches its `TypeObject`, and the
/// two types are found not assignable — which is the verdict the suite
/// asks for.
///
/// A named type and not a `bool` for the reason [`HashedIdCounter`] is
/// one: `Options` is `Default`, and a boolean defaulting to `false`
/// would carry no statement about which reading that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyOnUnionDiscriminator {
    /// The annotation is reported and the declaration does not carry
    /// it. What the measured toolchains do.
    Refused,
    /// The annotation is carried: the model records it, the
    /// discriminator's `UnionDiscriminatorFlag` gets `IS_KEY`, the
    /// union is a keyed topic type and its KeyHash is computed over the
    /// discriminator alone. **The default**, as Table 21 and
    /// §7.2.2.4.4.4.8 have it.
    #[default]
    Carried,
}

impl KeyOnUnionDiscriminator {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Refused, Self::Carried];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Carried)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Carried => "carried",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo here changes a type's `TypeIdentifier`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "refused" => Some(Self::Refused),
            "carried" => Some(Self::Carried),
            _ => None,
        }
    }
}

/// Lower a parsed file into a fresh model.
#[must_use]
pub fn lower(root: &Arc<SyntaxNode>, path: &str) -> Lowered {
    lower_with(root, path, Options::default())
}

/// Lower a parsed file into a fresh model, deciding what the source left
/// open.
#[must_use]
pub fn lower_with(root: &Arc<SyntaxNode>, path: &str, options: Options) -> Lowered {
    let mut ast = Ast::new();
    let diagnostics = lower_into_with(&mut ast, root, path, options);
    Lowered { ast, diagnostics }
}

/// Lower a parsed file into an existing model.
///
/// The second file of a two-file build goes here, which is where module
/// reopening has to converge rather than duplicate.
pub fn lower_into(ast: &mut Ast, root: &Arc<SyntaxNode>, path: &str) -> Vec<Diagnostic> {
    lower_into_with(ast, root, path, Options::default())
}

/// Lower a parsed file into an existing model, deciding what the source
/// left open.
pub fn lower_into_with(
    ast: &mut Ast,
    root: &Arc<SyntaxNode>,
    path: &str,
    options: Options,
) -> Vec<Diagnostic> {
    let mut ctx = Lowering {
        ast,
        scopes: ScopeStack::new_placeholder(),
        diagnostics: Vec::new(),
        path: path.to_string(),
        syntax_spans: SyntaxSpans::of(root),
        options,
        pending_annotations: Vec::new(),
        pragma_extensibility: None,
        repository_directives: Vec::new(),
        module_ranges: Vec::new(),
        keylists: Vec::new(),
        prefixes: Vec::new(),
        pragma_prefixes: Vec::new(),
        pragma_versions: Vec::new(),
        open_dds_types: Vec::new(),
        open_dds_keys: Vec::new(),
        unrecognised_pragmas: Vec::new(),
        xtopics_regions: Vec::new(),
        templates: Vec::new(),
    };
    ctx.scopes = ScopeStack::new(ctx.ast.root_module());
    ctx.file(root);
    // Completion runs over the finished model: a rule that reads the
    // previous member's id needs every member to be there first.
    ctx.complete();
    // Validation last: a rule about member ids needs the derived ones,
    // and a rule about forward declarations needs the whole file.
    ctx.validate();
    // A diagnostic points at text, not at the whitespace before it. The
    // ranges that came from a node still carry that node's leading
    // trivia, so each one — the primary and every label — is narrowed
    // here, at the one place every diagnostic of the walk passes
    // through. A range holding no syntax at all keeps what it had.
    let mut diagnostics = core::mem::take(&mut ctx.diagnostics);
    for diagnostic in &mut diagnostics {
        diagnostic.primary = ctx.syntax_spans.trim(diagnostic.primary);
        for (span, _) in &mut diagnostic.labels {
            *span = ctx.syntax_spans.trim(*span);
        }
    }
    // One fact, one diagnostic: a collision the model refused and the
    // validation pass found again is the same collision.
    dedup(diagnostics)
}

/// The walk's state.
///
/// Shared with the sibling modules — expressions, annotations,
/// completion and validation are separate files over one walk, not
/// separate passes over the tree.
pub(crate) struct Lowering<'a> {
    pub(crate) ast: &'a mut Ast,
    pub(crate) scopes: ScopeStack,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) path: String,
    /// Where the file's syntax is, with the trivia left out.
    ///
    /// A range taken from the tree begins at the node's leading trivia,
    /// because that is where the lexer attaches it — right for the round
    /// trip, wrong for anything a person reads. Every range this walk
    /// records on a node and every range it puts in a diagnostic is
    /// narrowed through this index first.
    pub(crate) syntax_spans: SyntaxSpans,
    /// What the caller decided that the source does not say.
    pub(crate) options: Options,
    /// The extensibility an enclosing `#pragma` asked for, which a type
    /// without its own annotation inherits. Spec §5.
    pub(crate) pragma_extensibility: Option<zerodds_ast::Extensibility>,
    /// Annotations read from an enclosing wrapper node, waiting for the
    /// declaration they modify to exist.
    ///
    /// A stack rather than one slot: an annotated declaration may
    /// contain another one, and the inner annotations must not be
    /// applied to the outer node.
    pub(crate) pending_annotations: Vec<Vec<crate::annotation::Applied>>,
    /// `typeid` and `typeprefix` directives, applied once the whole file
    /// is walked: both may name a declaration that comes later.
    ///
    /// The name is kept as every scope it could denote, innermost first
    /// — §7.5's lookup — because a directive inside a module may name a
    /// sibling, its own module, or anything further out.
    pub(crate) repository_directives: Vec<(Vec<ScopedName>, String, bool)>,
    /// Every range a module's body was written in, with the module's
    /// scoped name — one entry per opening, since a module may be reopened.
    /// `#pragma prefix` and the type name of a `#pragma keylist` are read
    /// in the module around them, which only these ranges know: a module
    /// node carries no source range of its own.
    pub(crate) module_ranges: Vec<(Span, ScopedName)>,
    /// `keylist` directives, applied for the same reason: the scopes the
    /// type name could denote (innermost first), the member names or
    /// paths, and where the directive stands.
    pub(crate) keylists: Vec<(Vec<ScopedName>, Vec<String>, Span)>,
    /// `typeprefix` scopes, longest match first when a repository id is
    /// derived.
    pub(crate) prefixes: Vec<(ScopedName, String)>,
    /// `#pragma prefix` scopes, which do the same thing in the older
    /// spelling.
    pub(crate) pragma_prefixes: Vec<(ScopedName, String)>,
    /// `#pragma version X 1.2`, which replaces the `1.0` in a derived
    /// repository id.
    pub(crate) pragma_versions: Vec<(ScopedName, String)>,
    /// `#pragma DCPS_DATA_TYPE "X"`.
    pub(crate) open_dds_types: Vec<String>,
    /// `#pragma DCPS_DATA_KEY X f`.
    pub(crate) open_dds_keys: Vec<(String, Vec<String>, Span)>,
    /// Pragmas with no typed meaning, kept verbatim on the root module.
    pub(crate) unrecognised_pragmas: Vec<(String, String)>,
    /// The `#pragma dds_xtopics begin` declarations still open, with the
    /// version each named, innermost last. DDS-XTypes 1.3 §7.3.1.1.2.
    ///
    /// A stack because the clause nests them: "In the event that such
    /// pragma declarations are nested within one another, the innermost
    /// version number specified, if any, shall be in effect." Nothing
    /// closes it at the end of the file, because the same clause says a
    /// `begin` without a matching `end` "shall be considered to continue
    /// until the end of the IDL input" — which is a complete region and
    /// not a defect.
    pub(crate) xtopics_regions: Vec<(Span, String)>,
    /// The body of every template module declared so far, by the scoped
    /// name it was declared under.
    ///
    /// A template module is not a scope with types in it — §7.4.12: its
    /// contents are definitions *of a template*, and a definition whose
    /// type is a formal parameter has no type until an actual one is
    /// given. Lowering the body in place is what left `module
    /// M<typename T> { struct S { T a; }; };` with `S.a` pending for
    /// ever, and the block reporting success on a model that was not
    /// one. The body is kept here and lowered once per instantiation.
    pub(crate) templates: Vec<(ScopedName, Arc<SyntaxNode>)>,
}

impl Lowering<'_> {
    fn file(&mut self, node: &Arc<SyntaxNode>) {
        self.identifiers_do_not_collide_with_keywords(node);
        for child in node.children() {
            self.definition(&child);
        }
        // Pragmas are read after the walk: `#pragma ID X` names a
        // declaration that may come after it, and the scope a `#pragma
        // prefix` applies to is the module whose range contains it.
        self.collect_pragmas(node);
    }

    /// An identifier that differs from a keyword only in case.
    ///
    /// OMG IDL 4.2 §7.2.4, p. 20: "Keywords must be written exactly as
    /// shown in the above list. Identifiers that collide with keywords
    /// (see 7.2.3, Identifiers) are illegal." §7.2.3 supplies the
    /// comparison — "upper- and lower-case letters are treated as the
    /// same letter" — and its worked example is `typedef boolean
    /// BOOLEAN;`.
    ///
    /// Two rules, and the lexer used to apply the first as though it
    /// were the second: it matched keywords without regard to case, so
    /// `enum E { CUSTOM };` parsed as though the author had written the
    /// keyword `custom` there. The lexer now matches exactly, which
    /// leaves `CUSTOM` an ordinary identifier and leaves this the place
    /// the clause is enforced.
    ///
    /// Over the token stream and not over the model, because the rule is
    /// about the spelling in the file: a name that collides is illegal
    /// wherever it is written, whether it declares something, names a
    /// type or stands for a value. Five of the seven foreign front ends
    /// measured on the Linux test host answer the same way, three of
    /// them naming the keyword — OpenDDS and TAO with "identifier
    /// spellings differ only in case", JacORB with "Identifier CUSTOM
    /// collides with keyword custom".
    ///
    /// The escaped form is exempt. §7.2.3.2: prefixing an underscore "is
    /// a purely lexical convention that ONLY turns off keyword
    /// checking", which is exactly the check this is.
    fn identifiers_do_not_collide_with_keywords(&mut self, root: &Arc<SyntaxNode>) {
        let mut found: Vec<(Span, String, &'static str)> = Vec::new();
        for node in root.descendants() {
            for (span, token) in node.tokens() {
                if token.kind() != TokenKind::Ident {
                    continue;
                }
                let text = token.text();
                // `_AnIdentifier` is the name `AnIdentifier` with the
                // keyword check turned off, so it is not asked at all.
                if text.starts_with('_') {
                    continue;
                }
                if let Some(keyword) = zerodds_idl_cst::keyword_collision(text) {
                    found.push((span, text.to_string(), keyword));
                }
            }
        }
        for (span, name, keyword) in found {
            let message = alloc::format!(
                "`{name}` differs from the keyword `{keyword}` only in case, and IDL compares \
                 identifiers without regard to case. Write the keyword's own spelling, or escape \
                 the identifier as `_{name}`"
            );
            self.diagnose_at(
                DiagnosticCode::KeywordCollision,
                span,
                &message,
                "OMG IDL 4.2 §7.2.3, §7.2.4",
            );
        }
    }

    /// Every `#pragma` in the file, with where it was written.
    ///
    /// The preprocessor leaves them alone — a pragma carries declaration
    /// metadata rather than affecting expansion — so they arrive here as
    /// the directive trivia they were, still in the tree.
    fn collect_pragmas(&mut self, root: &Arc<SyntaxNode>) {
        let mut found: Vec<(Span, String)> = Vec::new();
        for node in root.descendants() {
            for (span, token) in node.tokens() {
                if token.kind() != TokenKind::Directive {
                    continue;
                }
                let line = token.text().trim();
                let Some(rest) = line.strip_prefix('#') else {
                    continue;
                };
                let rest = rest.trim_start();
                let Some(body) = rest.strip_prefix("pragma") else {
                    continue;
                };
                found.push((span, body.trim().to_string()));
            }
        }
        // Source order, and not the order the walk happened to reach
        // the tokens in: `#pragma dds_xtopics begin` and its `end` are
        // a pair whose nesting DDS-XTypes 1.3 §7.3.1.1.2 reads off the
        // order they were written in, and a `keylist` naming a type
        // declared twice reaches the same reading only that way.
        found.sort_by_key(|(span, _)| span.start);
        for (span, body) in found {
            self.pragma(span, &body);
        }
    }

    /// One `#pragma`, routed by its first word.
    fn pragma(&mut self, span: Span, body: &str) {
        let mut words = body.split_whitespace();
        let Some(directive) = words.next() else {
            return;
        };
        let rest: Vec<&str> = words.collect();
        match directive {
            // CORBA 3.3 §10.7: the three that shape repository ids.
            "prefix" => {
                if let Some(text) = unquote(rest.first().copied().unwrap_or_default()) {
                    let scope = self.module_containing(span);
                    self.record_repository_directive("typeprefix", &text);
                    self.pragma_prefixes.push((scope, text));
                }
            }
            "version" => {
                if let (Some(name), Some(version)) = (rest.first(), rest.get(1)) {
                    self.pragma_versions
                        .push((pragma_name(name), (*version).to_string()));
                }
            }
            "ID" => {
                if let (Some(name), Some(id)) = (rest.first(), rest.get(1).copied()) {
                    if let Some(text) = unquote(id) {
                        let candidates = self.pragma_candidates(span, name);
                        self.record_repository_directive("typeid", &text);
                        self.repository_directives.push((candidates, text, false));
                    }
                }
            }
            // Cyclone DDS says keys the way RTI says `keylist`.
            "keylist" => {
                self.diagnose_vendor(
                    zerodds_idl_parse::VendorFlag::Cyclone,
                    span,
                    "`#pragma keylist` is a Cyclone DDS directive and is in no building block \
                     of OMG IDL 4.2; it makes members keys, so a front end that ignores it \
                     builds a different type from the same file",
                );
                // OpenSplice's `idlpp` and Cyclone's `idlc` both take
                // `a, b` as well as `a b` (measured, 11.0.1 and 6.9).
                if let Some((name, fields)) = rest.split_first() {
                    let fields = fields
                        .iter()
                        .flat_map(|f| f.split(','))
                        .filter(|f| !f.is_empty())
                        .map(ToString::to_string)
                        .collect();
                    let candidates = self.pragma_candidates(span, name);
                    self.keylists.push((candidates, fields, span));
                }
            }
            "DCPS_DATA_TYPE" => {
                self.diagnose_vendor(
                    zerodds_idl_parse::VendorFlag::OpenDds,
                    span,
                    "`#pragma DCPS_DATA_TYPE` is an OpenDDS directive and is in no building \
                     block of OMG IDL 4.2; it marks the type as a topic type, so a front end \
                     that ignores it builds a different type from the same file. The same \
                     spelling is also OMG DDS 1.2 §8's DLRL directive that marks a DLRL object \
                     type — this front end reads it as OpenDDS's, and `zerodds-idlc dlrl` \
                     reads it as DLRL's",
                );
                // Quoted as OpenDDS writes it, or bare; either names the type.
                match rest.first().copied() {
                    Some(word) => {
                        self.open_dds_types
                            .push(unquote(word).unwrap_or_else(|| word.to_string()));
                    }
                    None => self.diagnose_at(
                        DiagnosticCode::Unsupported,
                        span,
                        "`#pragma DCPS_DATA_TYPE` names no type",
                        "OMG DDS 1.4 §2.2.1.2.2",
                    ),
                }
            }
            "DCPS_DATA_KEY" => {
                self.diagnose_vendor(
                    zerodds_idl_parse::VendorFlag::OpenDds,
                    span,
                    "`#pragma DCPS_DATA_KEY` is an OpenDDS directive and is in no building \
                     block of OMG IDL 4.2; it makes a member a key, so a front end that \
                     ignores it builds a different type from the same file. The same spelling \
                     is also OMG DDS 1.2 §8's DLRL directive that marks a DLRL object key — \
                     this front end reads it as OpenDDS's, and `zerodds-idlc dlrl` reads it as \
                     DLRL's",
                );
                // OpenDDS writes the type and the field in one string,
                // `"Robot::Pose id"`; the unquoted form `Robot::Pose id` is
                // read the same.
                let joined = rest.join(" ");
                let words: Vec<String> = unquote(&joined)
                    .unwrap_or(joined)
                    .split_whitespace()
                    .map(String::from)
                    .collect();
                if let Some((name, fields)) = words.split_first() {
                    self.open_dds_keys
                        .push((name.clone(), fields.to_vec(), span));
                } else {
                    self.diagnose_at(
                        DiagnosticCode::Unsupported,
                        span,
                        "`#pragma DCPS_DATA_KEY` names neither a type nor a member; \
                         the directive keys nothing",
                        "OMG DDS 1.4 §2.2.1.2.2",
                    );
                }
            }
            // OMG DDS 1.2 §8.2.2 — the third DLRL pragma, and the one
            // OpenDDS never repurposed: `DCPS_DATA_TYPE`/`DCPS_DATA_KEY`
            // collide with OpenDDS's own two-pragma dialect above, this
            // one names nothing OpenDDS defines. Not routed through
            // `diagnose_vendor`: it is not a vendor extension admitted
            // under `ALL_VENDORS` and rejected under `--strict` — it is
            // an OMG-specification construct this front end simply does
            // not turn into a type-model facet, the same standing
            // `dds_xtopics` below has. Kept in the bag as well as
            // reported, so a round trip writes it back; `zerodds-idlc
            // dlrl` is what reads it, together with the other two, into
            // DLRL object and home classes.
            "DCPS_DLRL_RELATION" => {
                self.warn_at(
                    DiagnosticCode::Unsupported,
                    span,
                    "`#pragma DCPS_DLRL_RELATION` is OMG DDS 1.2 §8's DLRL relation \
                     directive; this front end does not turn it into a type-model facet — \
                     `zerodds-idlc dlrl` reads it, together with `DCPS_DATA_TYPE` and \
                     `DCPS_DATA_KEY`, into DLRL object and home classes",
                    "OMG DDS 1.2 §8.2.2",
                );
                self.unrecognised_pragmas
                    .push((directive.to_string(), rest.join(" ")));
            }
            // DDS-XTypes 1.3 §7.3.1.1.2. Kept in the bag below as
            // well as read: the model has no typed field for it, and a
            // round trip has to write back what was there.
            "dds_xtopics" => {
                self.dds_xtopics(span, &rest);
                self.unrecognised_pragmas
                    .push((directive.to_string(), rest.join(" ")));
            }
            // A pragma nobody recognises is kept rather than dropped: a
            // round trip back to that vendor's dialect needs it, and the
            // model has a place for exactly this.
            //
            // **And it is reported.** Keeping it was the whole of the
            // handling, so `#pragma this_pragma_does_not_exist_anywhere`
            // and `#pragma keylist Sample id` were indistinguishable
            // from the outside: nought errors, nought warnings, exit
            // zero, even under `--strict --deny-warnings`. A pragma is
            // the one place in IDL where a typo has no syntax to fail
            // against, so silence there is silence about a directive
            // that was meant to change the type.
            //
            // A warning and not an error, and the level is the
            // specification's rather than a choice. DDS-XTypes 1.3
            // §7.3.1.1.1 on constructs outside the building blocks it
            // uses: "Compilers that do not support them shall ignore
            // them or issue a warning rather than halting with an
            // error." Ignoring is what was measured and is what the
            // finding is about, so the other permitted level is the one
            // taken — and `--deny-warnings` is the caller who wants it
            // to fail.
            other => {
                self.warn_at(
                    DiagnosticCode::Unsupported,
                    span,
                    &alloc::format!(
                        "`#pragma {other}` is not a directive this front end interprets; it is \
                         kept verbatim on the model and changes nothing"
                    ),
                    "DDS-XTypes 1.3 §7.3.1.1.1",
                );
                self.unrecognised_pragmas
                    .push((other.to_string(), rest.join(" ")));
            }
        }
    }

    /// `#pragma dds_xtopics begin [<version>]` and its `end`.
    /// DDS-XTypes 1.3 §7.3.1.1.2.
    ///
    /// The clause defines the directive in exactly those two forms, as
    /// a marker "to indicate to their tools and to human readers that
    /// their IDL file (or a portion of it) makes use of constructs
    /// defined by this specification". It carries no meaning for a
    /// declaration: the same clause calls the declarations "informative
    /// only", which is why nothing here reaches a type. What it does
    /// carry is a version, and the clause names three things that can
    /// be wrong with a pair of them.
    ///
    /// Every one of them is a warning. The clause admits "halting with
    /// an error" as one behaviour a compiler may choose, and the
    /// requirement it states in its own words is a "should" — "those
    /// version numbers should be the same as those used with the
    /// matching 'begin' declarations". A marker the specification calls
    /// informative does not stop a build; `--deny-warnings` is the
    /// caller who wants it to.
    fn dds_xtopics(&mut self, span: Span, rest: &[&str]) {
        let spec = "DDS-XTypes 1.3 §7.3.1.1.2";
        let Some((keyword, version)) = rest.split_first() else {
            self.warn_at(
                DiagnosticCode::Unsupported,
                span,
                "`#pragma dds_xtopics` was written with neither `begin` nor `end`; \
                 §7.3.1.1.2 defines the directive in those two forms only",
                spec,
            );
            return;
        };
        let version = xtopics_version(version);
        match *keyword {
            "begin" => self.xtopics_regions.push((span, version)),
            "end" => {
                let Some((opened_at, opened)) = self.xtopics_regions.pop() else {
                    self.warn_at(
                        DiagnosticCode::Unsupported,
                        span,
                        "`#pragma dds_xtopics end` closes a region no `begin` opened",
                        spec,
                    );
                    return;
                };
                if opened.is_empty() || version.is_empty() || opened == version {
                    return;
                }
                let message = alloc::format!(
                    "`#pragma dds_xtopics end {version}` closes a region opened as \
                     `begin {opened}`; §7.3.1.1.2 has the two version numbers the same"
                );
                self.diagnostics.push(
                    Diagnostic::warning(DiagnosticCode::Unsupported, span, message, spec)
                        .with_label(opened_at, alloc::format!("opened as `begin {opened}`")),
                );
            }
            other => self.warn_at(
                DiagnosticCode::Unsupported,
                span,
                &alloc::format!(
                    "`#pragma dds_xtopics {other}` is neither `begin` nor `end`; \
                     §7.3.1.1.2 defines the directive in those two forms only"
                ),
                spec,
            ),
        }
    }

    /// The innermost module whose source range contains `span`.
    /// Every scope the type name a `#pragma` writes could denote, innermost
    /// first — [`Self::directive_candidates`] for a directive the walk no
    /// longer stands in, whose scope is the module around its position.
    ///
    /// `pragma_name` alone read every name from the root: `module M {
    /// struct T {…}; #pragma keylist T k };` named `::T`, which does not
    /// exist, and the directive keyed nothing — the placement OpenSplice's
    /// `idlpp` requires for it. A name written `::T` still means the root
    /// and nothing else.
    fn pragma_candidates(&self, span: Span, written: &str) -> Vec<ScopedName> {
        let name = pragma_name(written);
        if written.starts_with("::") {
            return alloc::vec![name];
        }
        let module = self.module_containing(span);
        let enclosing = module.segments();
        (0..=enclosing.len())
            .rev()
            .map(|depth| {
                let mut path = enclosing[..depth].to_vec();
                path.extend_from_slice(name.segments());
                ScopedName::absolute(path)
            })
            .collect()
    }

    fn module_containing(&self, span: Span) -> ScopedName {
        // The narrowest range around `span`: modules nest, and the
        // innermost is the one a directive stands in.
        self.module_ranges
            .iter()
            .filter(|(range, _)| range.start <= span.start && range.end >= span.end)
            .min_by_key(|(range, _)| range.end.saturating_sub(range.start))
            .map_or_else(
                || ScopedName::absolute(Vec::new()),
                |(_, scoped)| scoped.clone(),
            )
    }

    /// One declaration, with whatever annotations reach it.
    ///
    /// A prefix annotation sits in the wrapper node the parser puts
    /// around the declaration, and [`Self::apply_waiting_annotations`]
    /// reads it from there. The suffix form of DDS-XTypes 1.3
    /// §7.3.1.2.5, Table 24 — `}; //@my_type_annotation` — has no
    /// wrapper to sit in: it belongs to the declaration it follows, so
    /// the parser puts it inside the declaration's own node. Both reach
    /// the same frame here, which is what makes Table 24's claim that
    /// the two forms are equivalent true of a type as well as of a
    /// member.
    fn definition(&mut self, node: &Arc<SyntaxNode>) {
        let suffix = self.suffix_annotations_of(node);
        // DDS-XTypes 1.3 §7.3.1.2.4, Table 22: `struct <ann_identifier> {
        // <ann_members> }; //@annotation` "is actually an annotation type
        // containing the members <ann_members>". It is the declaration of
        // an annotation for compilers that do not know the `@annotation`
        // token, not a structure that the annotation `@annotation` is
        // applied to.
        if node.kind() == CstKind::StructDecl
            && suffix
                .iter()
                .any(|applied| applied.name == "annotation" && applied.arguments.is_empty())
        {
            self.annotation_from_struct(node);
            return;
        }
        if suffix.is_empty() {
            self.definition_inner(node);
            return;
        }
        let outer = self.pending_annotations.pop();
        let mut frame = outer.clone().unwrap_or_default();
        frame.extend(suffix);
        self.pending_annotations.push(frame);
        self.definition_inner(node);
        let _ = self.pending_annotations.pop();
        if let Some(outer) = outer {
            self.pending_annotations.push(outer);
        }
    }

    fn definition_inner(&mut self, node: &Arc<SyntaxNode>) {
        match node.kind() {
            CstKind::Module => self.module(node),
            CstKind::TemplateModuleInst => self.template_module_inst(node),
            CstKind::TemplateModuleRef => self.template_module_ref(node),
            CstKind::StructDecl => self.structure(node),
            CstKind::EnumDecl => self.enumeration(node),
            CstKind::BitmaskDecl => self.bitmask(node),
            CstKind::UnionDecl => self.union(node),
            CstKind::TypedefDecl => self.typedef(node),
            CstKind::ExceptionDecl => self.exception(node),
            CstKind::ForwardDecl => self.forward(node),
            CstKind::NativeDecl => self.native(node),
            // An annotated declaration wraps its annotations and the
            // declaration itself. The annotations are read here and
            // applied to whatever the inner declaration produced, which
            // is why `declare_type` reports the node it made.
            CstKind::Member => {
                let annotations = self.annotations_of(node);
                self.pending_annotations.push(annotations);
                for child in node.children() {
                    self.definition(&child);
                }
                let _ = self.pending_annotations.pop();
            }
            CstKind::InterfaceDecl => self.interface(node),
            CstKind::ValuetypeDecl => self.valuetype(node),
            CstKind::ValueBoxDecl => self.value_box(node),
            CstKind::ConstDecl => self.constant(node),
            CstKind::BitsetDecl => self.bitset(node),
            CstKind::AnnotationDecl => self.annotation_decl(node),
            CstKind::RepositoryDirective => self.repository_directive(node),
            CstKind::ImportDecl => self.import(node),
            CstKind::KeylistDecl => self.keylist(node),
            CstKind::Error => {}
            _ => {
                for child in node.children() {
                    self.definition(&child);
                }
            }
        }
    }

    fn module(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let Some(parent) = self.scopes.current_module() else {
            return;
        };
        // Reopening converges here rather than duplicating. Spec §4.
        let Ok(module) = self.ast.ensure_module(parent, &name) else {
            return;
        };
        if let Some(parameters) = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::TemplateParameters)
        {
            self.template_parameters(&parameters, module);
            // The body is the template's, not the model's. It is kept
            // whole and lowered once per instantiation, with the formal
            // parameters bound to what the instantiation supplied.
            let qualified = self.scopes.qualify(&name);
            self.templates.push((qualified, node.clone()));
            return;
        }
        // A module is an annotation target of its own. DDS-XTypes 1.3
        // §7.3.1.2.1.7 puts `@default_nested` on one, and Table 21 lists
        // "Module" for it; the annotations in front of the declaration
        // reached nothing until this call, so `@default_nested(TRUE)
        // module M { … }` was parsed and dropped.
        //
        // Before the body, because a reopened module keeps the facets of
        // its first appearance and the annotation belongs to the
        // declaration that carries it.
        if let Ok(node_id) = self.ast.module(module).map(|m| m.node_id) {
            self.apply_waiting_annotations(AnyId::Module(module, node_id));
        }
        let range = self.syntax_spans.trim(node.span());
        self.module_ranges.push((range, self.scopes.qualify(&name)));
        self.scopes.push(&name, Some(module));
        for child in node.children() {
            self.definition(&child);
        }
        self.scopes.pop();
    }

    /// `module M<long, 8> N;` — rules (190), (191) and (192).
    ///
    /// Instantiation is substitution: the template's body is lowered
    /// into a module of the instance's name, with each formal parameter
    /// bound to the actual the instantiation gave it. Binding rather
    /// than rewriting the tree, because every reference the body makes
    /// to a formal is a name, and a name is what a scope answers.
    fn template_module_inst(&mut self, node: &Arc<SyntaxNode>) {
        // The instance's own name is the last identifier, after the
        // template's scoped name and its actual parameters.
        let idents: Vec<String> = node
            .syntax_tokens()
            .into_iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            .collect();
        let Some(instance) = idents.last().cloned() else {
            return;
        };
        let Some(template_name) = self.scoped_name_of(node) else {
            return;
        };
        let Some(template) = self.template_body(node, &template_name) else {
            let message = alloc::format!(
                "`{}` is not a template module declared in this file",
                template_name.segments().join("::")
            );
            self.diagnose_at(
                DiagnosticCode::UnresolvedName,
                node.span(),
                &message,
                "OMG IDL 4.2 §7.4.12.3 rule (190)",
            );
            return;
        };

        let formals = template_formals(&template);
        let actuals = self.actual_parameters(node);
        if formals.len() != actuals.len() {
            let message = alloc::format!(
                "`{}` takes {} actual parameter(s) and was given {}",
                template_name.segments().join("::"),
                formals.len(),
                actuals.len()
            );
            self.diagnose_at(
                DiagnosticCode::UnexpectedToken,
                node.span(),
                &message,
                "OMG IDL 4.2 §7.4.12.3 rule (191)",
            );
            return;
        }

        let Some(parent) = self.scopes.current_module() else {
            return;
        };
        self.check_collision(&instance, node);
        let Ok(module) = self.ast.ensure_module(parent, &instance) else {
            return;
        };
        self.instantiate(&template, &formals, &actuals, module, &instance, node);
    }

    /// `alias M<T> R;` — rules (193) and (194).
    ///
    /// A template module body may name another template module,
    /// instantiated with its **own** formal parameters. Inside an
    /// instantiation those formals are bound to actuals already, so the
    /// reference hands the bindings on unchanged; outside one there is
    /// nothing to hand on, and (189) admits the form only as a
    /// `<tpl_definition>`.
    fn template_module_ref(&mut self, node: &Arc<SyntaxNode>) {
        let Some(instance) = node
            .syntax_tokens()
            .into_iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            .next_back()
        else {
            return;
        };
        let Some(template_name) = self.scoped_name_of(node) else {
            return;
        };
        let Some(template) = self.template_body(node, &template_name) else {
            let message = alloc::format!(
                "`alias` names `{}`, which is not a template module declared in this file",
                template_name.segments().join("::")
            );
            self.diagnose_at(
                DiagnosticCode::UnresolvedName,
                node.span(),
                &message,
                "OMG IDL 4.2 §7.4.12.3 rule (193)",
            );
            return;
        };

        let mut actuals = Vec::new();
        for written in self.formal_parameter_names(node) {
            let Some(any) = self.scopes.lookup(&written) else {
                let message = alloc::format!(
                    "`{written}` is not a formal parameter of the template this `alias` is \
                     written in; rule (193) instantiates with the enclosing template's own \
                     parameters"
                );
                self.diagnose_at(
                    DiagnosticCode::UnresolvedName,
                    node.span(),
                    &message,
                    "OMG IDL 4.2 §7.4.12.3 rule (194)",
                );
                return;
            };
            actuals.push(Actual::Bound(any));
        }

        let formals = template_formals(&template);
        if formals.len() != actuals.len() {
            let message = alloc::format!(
                "`{}` takes {} formal parameter(s) and the `alias` names {}",
                template_name.segments().join("::"),
                formals.len(),
                actuals.len()
            );
            self.diagnose_at(
                DiagnosticCode::UnexpectedToken,
                node.span(),
                &message,
                "OMG IDL 4.2 §7.4.12.3 rule (194)",
            );
            return;
        }

        let Some(parent) = self.scopes.current_module() else {
            return;
        };
        self.check_collision(&instance, node);
        let Ok(module) = self.ast.ensure_module(parent, &instance) else {
            return;
        };
        self.instantiate(&template, &formals, &actuals, module, &instance, node);
    }

    /// The names written between the angle brackets of an `alias`.
    fn formal_parameter_names(&self, node: &Arc<SyntaxNode>) -> Vec<String> {
        node.children()
            .into_iter()
            .find(|child| child.kind() == CstKind::ActualParameters)
            .map_or_else(Vec::new, |list| {
                list.children()
                    .into_iter()
                    .filter(|child| child.kind() == CstKind::ScopedName)
                    .filter_map(|scoped| first_ident(&scoped))
                    .collect()
            })
    }

    /// Lower a template body into one instance module.
    fn instantiate(
        &mut self,
        template: &Arc<SyntaxNode>,
        formals: &[(String, bool)],
        actuals: &[Actual],
        module: Id<zerodds_ast::ModuleNode>,
        instance: &str,
        at: &Arc<SyntaxNode>,
    ) {
        if let Ok(node_id) = self.ast.module(module).map(|m| m.node_id) {
            self.record_origin(AnyId::Module(module, node_id), at.span());
        }
        self.scopes.push(instance, Some(module));
        for (formal, actual) in formals.iter().zip(actuals) {
            self.bind_actual(formal, actual, module, at);
        }
        for child in template.children() {
            self.definition(&child);
        }
        self.scopes.pop();
    }

    /// The stored body of the template a name denotes.
    fn template_body(
        &self,
        node: &Arc<SyntaxNode>,
        written: &ScopedName,
    ) -> Option<Arc<SyntaxNode>> {
        for candidate in self.directive_candidates(node, written) {
            if let Some((_, body)) = self
                .templates
                .iter()
                .find(|(name, _)| name.segments() == candidate.segments())
            {
                return Some(body.clone());
            }
        }
        None
    }

    /// The actual parameters an instantiation wrote, in order.
    fn actual_parameters(&mut self, node: &Arc<SyntaxNode>) -> Vec<Actual> {
        node.children()
            .into_iter()
            .find(|child| child.kind() == CstKind::ActualParameters)
            .map_or_else(Vec::new, |list| {
                list.children()
                    .into_iter()
                    .filter(|child| {
                        matches!(
                            child.kind(),
                            CstKind::PrimitiveType
                                | CstKind::NamedType
                                | CstKind::SequenceType
                                | CstKind::StringType
                                | CstKind::WStringType
                                | CstKind::MapType
                                | CstKind::FixedType
                        ) || crate::expr::is_expr(child.kind())
                    })
                    .map(Actual::Written)
                    .collect()
            })
    }

    /// Bind one formal parameter to the actual the instantiation gave.
    ///
    /// A `const` formal becomes a constant of the instance module, so a
    /// bound written as `sequence<T, Capacity>` in the template body
    /// resolves the way any other named constant does. Every other kind
    /// binds the actual type under the formal's name.
    fn bind_actual(
        &mut self,
        formal: &(String, bool),
        actual: &Actual,
        module: Id<zerodds_ast::ModuleNode>,
        at: &Arc<SyntaxNode>,
    ) {
        let (name, is_const) = (formal.0.clone(), formal.1);
        // (193) hands on what the enclosing instantiation already bound,
        // so there is nothing to evaluate: the actual is a node of the
        // model and the formal takes the same one under its own name.
        let actual = match actual {
            Actual::Bound(any) => {
                self.scopes.bind(&name, *any);
                return;
            }
            Actual::Written(node) => node.clone(),
        };
        let actual = &actual;
        if is_const {
            let Some(expr) = self.expr(actual) else {
                self.diagnose_at(
                    DiagnosticCode::UnexpectedToken,
                    at.span(),
                    "a `const` formal parameter needs a constant expression as its actual",
                    "OMG IDL 4.2 §7.4.12.3 rule (192)",
                );
                return;
            };
            let type_ = Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(Vec::new())));
            if let Ok(id) = self.ast.add_const(module, &name, type_, expr) {
                if let Ok(node_id) = self.ast.constant(id).map(|c| c.node_id) {
                    self.record_origin(AnyId::Const(id, node_id), at.span());
                    self.record_declaration_scope(AnyId::Const(id, node_id), at.span());
                    self.scopes.bind(&name, AnyId::Const(id, node_id));
                }
            }
            return;
        }
        let type_ref = self.type_ref_of(actual);
        match type_ref.resolved() {
            Some(id) => self.bind(&name, id),
            None => self.diagnose_at(
                DiagnosticCode::UnresolvedName,
                at.span(),
                "an actual parameter names a type that is not declared in this file",
                "OMG IDL 4.2 §7.4.12.3 rule (192)",
            ),
        }
    }

    fn structure(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let base = node
            .children()
            .iter()
            .find(|child| child.kind() == CstKind::InheritanceClause)
            .and_then(|clause| self.type_ref_from_scoped_name(clause));

        let Some(id) = self.declare_type(
            &name,
            TypeKind::Structure {
                base,
                members: Vec::new(),
            },
            node,
        ) else {
            return;
        };

        self.one_namespace_per_scope(node);
        for child in node.children() {
            if child.kind() == CstKind::Member {
                self.member(&child, id);
            }
        }
    }

    fn exception(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        self.check_collision(&name, node);
        let Some(module) = self.scopes.current_module() else {
            return;
        };
        let Ok(id) = self.ast.add_exception(module, &name) else {
            return;
        };
        if let Ok(node_id) = self.ast.exception(id).map(|e| e.node_id) {
            self.record_origin(AnyId::Exception(id, node_id), node.span());
            self.record_declaration_scope(AnyId::Exception(id, node_id), node.span());
            self.scopes.bind(&name, AnyId::Exception(id, node_id));
            self.apply_waiting_annotations(AnyId::Exception(id, node_id));
        }
        self.one_namespace_per_scope(node);
        for child in node.children() {
            if child.kind() == CstKind::Member {
                let annotations = self.annotations_of(&child);
                for (member_name, type_ref, at) in self.declared(&child) {
                    let spec = self.member_spec(&member_name, type_ref, &at);
                    let result = self.ast.add_exception_member(id, spec);
                    let Some(member) = self.report_model(result, at.span()) else {
                        continue;
                    };
                    let Ok(node_id) = self.ast.member(member).map(|m| m.node_id) else {
                        continue;
                    };
                    self.apply_annotations(&annotations, AnyId::Member(member, node_id));
                }
            }
        }
    }

    fn member(&mut self, node: &Arc<SyntaxNode>, aggregate: Id<TypeNode>) {
        let annotations = self.annotations_of(node);
        // `public` and `private` are the only two ways to open a
        // valuetype state member — rule (106) — and the keyword was
        // consumed and thrown away. CCM 4.0 §6.7.2 forbids a private
        // state member in a primary-key valuetype, a rule nothing could
        // check once the keyword was gone.
        let visibility = state_visibility(node);
        for (name, type_ref, at) in self.declared(node) {
            let spec = self.member_spec(&name, type_ref, &at);
            let result = self.ast.add_member(aggregate, spec);
            let Some(id) = self.report_model(result, at.span()) else {
                continue;
            };
            let Ok(node_id) = self.ast.member(id).map(|m| m.node_id) else {
                continue;
            };
            if let Some(visibility) = visibility {
                if let Some(facets) = self.ast.facets_mut(AnyId::Member(id, node_id)) {
                    facets.corba_mut().state_visibility = Some(visibility);
                }
            }
            // The name is stored stripped (§7.2.3.2); whether the
            // declaration spelled it `_name` survives only here, for
            // the one dialect whose member-id hash reads the spelling.
            if declarator_is_escaped(&at) {
                if let Some(facets) = self.ast.facets_mut(AnyId::Member(id, node_id)) {
                    facets.xtypes_mut().escaped = true;
                }
            }
            self.apply_annotations(&annotations, AnyId::Member(id, node_id));
        }
    }

    /// Every name a declaration introduces, with the type each one has.
    ///
    /// One type specification, several declarators: `double x, y, z;`.
    /// An array declarator gives its own name an array type over the
    /// shared element type — `long a, b[4];` is one `long` and one
    /// `long[4]`, not two arrays. IDL 4.2 §7.4.1.4.4.
    fn declared(
        &mut self,
        node: &Arc<SyntaxNode>,
    ) -> Vec<(String, Ref<TypeNode>, Arc<SyntaxNode>)> {
        let Some(element) = self.type_ref(node) else {
            return Vec::new();
        };
        let declarators: Vec<Arc<SyntaxNode>> = node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Declarator)
            .collect();

        let mut declared = Vec::new();
        for declarator in declarators {
            let Some(name) = first_ident(&declarator) else {
                continue;
            };
            let type_ref = self.declarator_type(&element, &declarator);
            declared.push((name, type_ref, declarator));
        }
        declared
    }

    /// The type a declarator gives `element`: the element itself, or an
    /// array of it when the declarator writes dimensions.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.4 writes the annotations of an array's
    /// elements between the declarator's name and its dimensions — `Foo
    /// array_of_foo @external [42];` — so they are applied to the array
    /// as the annotations of its element, the way `sequence<@external
    /// Foo, 42>` applies them to a sequence's. Written on a declarator
    /// that has no dimensions they have no element to apply to, and are
    /// reported rather than dropped.
    fn declarator_type(
        &mut self,
        element: &Ref<TypeNode>,
        declarator: &Arc<SyntaxNode>,
    ) -> Ref<TypeNode> {
        let dims = self.array_dimensions(declarator);
        let written = self.annotations_of(declarator);
        if dims.is_empty() {
            if let Some(first) = written.first() {
                self.annotation_error(
                    first,
                    &alloc::format!(
                        "`@{}` stands between a declarator's name and its dimensions, and applies \
                         to the elements of an array; this declarator is not an array",
                        first.name
                    ),
                    "DDS-XTypes 1.3 §7.3.1.2.1.4",
                );
            }
            return element.clone();
        }
        let array = self.anonymous(
            TypeKind::Array {
                element: element.clone(),
                dims,
            },
            declarator.span(),
        );
        if !written.is_empty() {
            if let Some(id) = array.resolved() {
                let any = AnyId::Type(id, self.node_id_of(id));
                self.apply_use_site(&written, any, UseSite::Element);
            }
        }
        array
    }

    /// The `[expr]` dimensions of a declarator, outermost first.
    fn array_dimensions(&mut self, declarator: &Arc<SyntaxNode>) -> Vec<Id<ConstExprNode>> {
        declarator
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::ArrayDimension)
            .filter_map(|dim| self.first_expr_child(&dim))
            .collect()
    }

    fn member_spec(&self, name: &str, type_: Ref<TypeNode>, at: &Arc<SyntaxNode>) -> MemberSpec {
        MemberSpec {
            name: name.to_string(),
            type_,
            facets: self.facets_at(at.span()),
        }
    }

    fn enumeration(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let Some(id) = self.declare_type(
            &name,
            TypeKind::Enumeration {
                enumerators: Vec::new(),
            },
            node,
        ) else {
            return;
        };
        for child in node.children() {
            if child.kind() == CstKind::Enumerator {
                if let Some(name) = first_ident(&child) {
                    // No explicit value here: `@value(n)` is applied
                    // with the other annotations, and a positional one
                    // is filled in by the completion pass. Spec §5.
                    let result = self.ast.add_enumerator(id, &name, None);
                    let Some(enumerator) = self.report_model(result, child.span()) else {
                        continue;
                    };
                    let annotations = self.annotations_of(&child);
                    let Ok(node_id) = self.ast.enumerator(enumerator).map(|e| e.node_id) else {
                        continue;
                    };
                    // The enumerator goes into the scope enclosing the
                    // enumeration, not into one of its own. OMG IDL 4.2
                    // §7.4.1.4.2: `module M { enum Size { small, medium,
                    // large }; }; const M::Size MYSIZE = M::medium;`.
                    // This is what lets `case RED:` name an enumerator,
                    // and what makes two enumerations sharing a literal
                    // name the collision §7.2.3 says it is.
                    self.check_collision(&name, &child);
                    self.scopes
                        .bind(&name, AnyId::Enumerator(enumerator, node_id));
                    self.apply_annotations(&annotations, AnyId::Enumerator(enumerator, node_id));
                }
            }
        }
    }

    fn bitmask(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let Some(id) = self.declare_type(&name, TypeKind::Bitmask { bits: Vec::new() }, node)
        else {
            return;
        };
        for child in node.children() {
            if child.kind() == CstKind::BitValue {
                if let Some(name) = first_ident(&child) {
                    let result = self.ast.add_bit_value(id, &name, None);
                    let Some(bit) = self.report_model(result, child.span()) else {
                        continue;
                    };
                    let annotations = self.annotations_of(&child);
                    let Ok(node_id) = self.ast.bit_value(bit).map(|b| b.node_id) else {
                        continue;
                    };
                    self.apply_annotations(&annotations, AnyId::BitValue(bit, node_id));
                }
            }
        }
    }

    fn union(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let discriminator = self.type_ref(node).unwrap_or_else(|| {
            Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(Vec::new())))
        });
        let Some(id) = self.declare_type(
            &name,
            TypeKind::Union {
                discriminator,
                cases: Vec::new(),
                default_case: None,
            },
            node,
        ) else {
            return;
        };

        // `union U switch (@try_construct K)` — §7.3.1.2.1.12.5. An
        // `Annotation` child of the declaration itself can only be the
        // discriminator's: the union's own annotations sit on the
        // enclosing `Member` node and a case's on its `UnionCase`.
        let discriminator_annotations: Vec<Applied> = node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Annotation)
            .filter_map(|child| self.applied_annotation(&child))
            .collect();
        if !discriminator_annotations.is_empty() {
            let any = AnyId::Type(id, self.node_id_of(id));
            self.apply_use_site(&discriminator_annotations, any, UseSite::Discriminator);
        }
        self.one_namespace_per_scope(node);

        for child in node.children() {
            if child.kind() != CstKind::UnionCase {
                continue;
            }
            let label_nodes: Vec<Arc<SyntaxNode>> = child
                .children()
                .into_iter()
                .filter(|c| c.kind() == CstKind::CaseLabel)
                .collect();
            let mut labels: Vec<zerodds_ast::CaseLabel> = Vec::new();
            for label in label_nodes {
                let is_default = label
                    .syntax_tokens()
                    .iter()
                    .any(|(_, token)| token.kind() == TokenKind::Default);
                if is_default {
                    labels.push(zerodds_ast::CaseLabel::Default);
                    continue;
                }
                // `case <expr>:` keeps its expression: the label may name
                // a constant or an enumerator, and the value it stands
                // for is not known until those resolve.
                match self.first_expr_child(&label) {
                    Some(expr) => labels.push(zerodds_ast::CaseLabel::Value(expr)),
                    None => self.diagnose_at(
                        DiagnosticCode::MissingToken,
                        label.span(),
                        "a `case` label needs a constant expression",
                        "OMG IDL 4.2 §7.4.1.4.4.3",
                    ),
                }
            }
            // `element_spec ::= type_spec declarator` — one declarator,
            // where a struct member's `member ::= type_spec
            // declarators ";"` takes a list. `parse_union_case` shares
            // `parse_declarator_list` with the struct member and so
            // accepts `case 1: long a, b;` without a word, and
            // `member_parts` answers with the first declarator only:
            // every branch after the comma was lowered into nothing,
            // named in no diagnostic, and absent from the model the
            // codec and every generator read.
            let declared = self.declared(&child);
            for (_, _, extra) in declared.iter().skip(1) {
                self.diagnose_at(
                    DiagnosticCode::UnexpectedToken,
                    extra.span(),
                    "a union case declares one member; `element_spec` takes a single declarator",
                    "OMG IDL 4.2 §7.4.1.4.4.3",
                );
            }
            let Some((member_name, type_ref, declarator)) = declared.into_iter().next() else {
                continue;
            };
            // The branch and the member behind it are two ranges, and a
            // reader needs both: the case is the whole `case 1: long a`,
            // the member is its declarator — the same range a struct
            // member of the same declaration gets. Giving the member the
            // branch's range made every message about it point at the
            // `case` keyword.
            let spec = self.member_spec(&member_name, type_ref, &declarator);
            let annotations = self.annotations_of(&child);
            let result = self.ast.add_case(id, labels, spec);
            let Some(case) = self.report_model(result, child.span()) else {
                continue;
            };
            let Ok(node_id) = self.ast.case(case).map(|c| c.node_id) else {
                continue;
            };
            // `Ast::add_case` marks a case's facets synthetic, and
            // nothing overwrote them: every rule that reports a branch —
            // a repeated label, `@key` on a case member — pointed at
            // byte zero of the file.
            self.record_origin(AnyId::Case(case, node_id), child.span());
            // The same trace of the escape spelling a struct member
            // keeps, for the same one consumer.
            if declarator_is_escaped(&declarator) {
                if let Some(facets) = self.ast.facets_mut(AnyId::Case(case, node_id)) {
                    facets.xtypes_mut().escaped = true;
                }
            }
            self.apply_annotations(&annotations, AnyId::Case(case, node_id));
            // Mirror the case's XTypes facet onto the member behind it.
            // The source writes a branch's annotations on the case, and
            // that is where `apply_annotations` puts them — but the
            // layout planner (`effective_member_id`, `must_understand`),
            // the codec walks (`is_external`, `try_construct`) and every
            // language adapter read `MemberNode::facets`. Left unmirrored,
            // `case 1: @external Inner i;` lowered without error and no
            // consumer ever saw the annotation — a silent drop.
            // `validate.rs` already reads both sides for its union rules,
            // so the mirror changes no diagnostic; it is applied once,
            // after every annotation of the branch, so no diagnostic is
            // emitted twice either.
            let mirrored = self
                .ast
                .facets(AnyId::Case(case, node_id))
                .and_then(|facets| facets.xtypes.clone());
            if let Some(xtypes) = mirrored {
                let member_any = self
                    .ast
                    .case(case)
                    .ok()
                    .map(|c| c.member)
                    .and_then(|member| self.ast.any_of_member(member).ok());
                if let Some(facets) = member_any.and_then(|any| self.ast.facets_mut(any)) {
                    *facets.xtypes_mut() = xtypes;
                }
            }
        }
    }

    /// `typedef <type> <declarator>, ...;`
    ///
    /// A declarator here carries dimensions exactly as one on a struct
    /// member does: `typedef long Matrix[2][3];` aliases an array of
    /// `long`, not a `long`. IDL 4.2 §7.4.1.4.4 gives both forms the same
    /// `<declarator>` production, so both give their own name the array
    /// type over the shared element type.
    fn typedef(&mut self, node: &Arc<SyntaxNode>) {
        // Rule (64)'s third alternative: the type is defined inside the
        // typedef. It is declared first, in the typedef's own scope,
        // and the declarators alias it.
        let defined = node.children().into_iter().find(|child| {
            matches!(
                child.kind(),
                CstKind::StructDecl
                    | CstKind::UnionDecl
                    | CstKind::EnumDecl
                    | CstKind::BitsetDecl
                    | CstKind::BitmaskDecl
            )
        });
        let element = match defined {
            Some(definition) => {
                self.definition_inner(&definition);
                let Some(name) = first_ident(&definition) else {
                    return;
                };
                match self.scopes.lookup(&name) {
                    Some(AnyId::Type(id, _)) => Ref::Resolved(id),
                    _ => return,
                }
            }
            None => {
                let Some(element) = self.type_ref(node) else {
                    return;
                };
                element
            }
        };
        for declarator in node
            .children()
            .iter()
            .filter(|child| child.kind() == CstKind::Declarator)
        {
            let Some(name) = first_ident(declarator) else {
                continue;
            };
            let aliased = self.declarator_type(&element, declarator);
            let _ = self.declare_type(&name, TypeKind::Alias { aliased }, declarator);
        }
    }

    fn native(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let _ = self.declare_type(&name, TypeKind::Native, node);
    }

    fn forward(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let Some(module) = self.scopes.current_module() else {
            return;
        };
        // A forward declaration is a completeness state, not a kind: the
        // definition completes this node rather than replacing it, so
        // references taken now stay valid. Spec §3.2 of the AST.
        if self.scopes.lookup(&name).is_some() {
            return;
        }
        // *Which* node, though, is the keyword's decision. Every forward
        // used to bind a `Structure` placeholder whatever it said, so
        // `interface I; interface I { … };` collided with itself:
        // `DuplicateName` for the definition, which could not complete a
        // type it was not, and `UndefinedForward` for the placeholder
        // nothing ever finished. Rules (75) and (135), the same bug
        // twice. `struct S;` and `union U;` were right all along, which
        // is what made it invisible.
        if let Some(kind) = forward_interface_kind(node) {
            if let Ok(id) = self.ast.add_forward_interface(module, &name, kind) {
                if let Ok(node_id) = self.ast.interface(id).map(|node| node.node_id) {
                    self.record_origin(AnyId::Interface(id, node_id), node.span());
                    self.scopes.bind(&name, AnyId::Interface(id, node_id));
                }
            }
            return;
        }
        if let Ok(id) = self.ast.add_forward_type(
            module,
            &name,
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        ) {
            // The interface branch above records where the declaration
            // was and this one did not, so `UndefinedForward` for a
            // `struct S;` that nothing completes — the one message whose
            // whole content is *which* declaration was left open — named
            // byte zero instead of the declaration.
            self.record_origin(AnyId::Type(id, self.node_id_of(id)), node.span());
            self.record_declaration_scope(AnyId::Type(id, self.node_id_of(id)), node.span());
            self.bind(&name, id);
        }
    }

    fn constant(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let Some(expr) = self.first_expr_child(node) else {
            self.diagnose_at(
                DiagnosticCode::MissingToken,
                node.span(),
                "a constant needs a value expression",
                "OMG IDL 4.2 §7.4.1.4.2",
            );
            return;
        };
        // The declared type is what the constant has; the expression is
        // checked against it rather than used to derive it. Spec §5.
        //
        // Rule (43) is the one exception the language itself makes:
        // `<fixed_pt_const_type> ::= "fixed"` has no parameters, so a
        // `const fixed` takes its digits and scale from its value.
        let type_ = if bare_fixed(node) {
            self.fixed_const_type(expr, node)
        } else {
            self.type_ref(node)
                .unwrap_or_else(|| Ref::Broken(broken_cause()))
        };
        self.check_const_value_form(&type_, expr, node);
        self.check_collision(&name, node);
        let Some(module) = self.scopes.current_module() else {
            return;
        };
        if let Ok(id) = self.ast.add_const(module, &name, type_, expr) {
            let node_id = self.ast.constant(id).map(|c| c.node_id).ok();
            if let Some(node_id) = node_id {
                self.record_origin(AnyId::Const(id, node_id), node.span());
                self.record_declaration_scope(AnyId::Const(id, node_id), node.span());
                self.scopes.bind(&name, AnyId::Const(id, node_id));
                self.apply_waiting_annotations(AnyId::Const(id, node_id));
            }
        }
    }

    fn bitset(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let base = node
            .children()
            .iter()
            .find(|child| child.kind() == CstKind::InheritanceClause)
            .and_then(|clause| self.type_ref_from_scoped_name(clause));
        let Some(id) = self.declare_type(
            &name,
            TypeKind::Bitset {
                base,
                fields: Vec::new(),
            },
            node,
        ) else {
            return;
        };

        for field in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::BitField)
        {
            self.bitfield(&field, id);
        }
    }

    /// One `bitfield<n>` or `bitfield<n, T>` of a bitset.
    ///
    /// An unnamed bitfield is padding and keeps its width, so the layout
    /// of the ones that follow stays right. XTypes 1.3 §7.3.1.2.1.11.
    fn bitfield(&mut self, node: &Arc<SyntaxNode>, bitset: Id<TypeNode>) {
        let Some(expr) = self.first_expr_child(node) else {
            return;
        };
        let Ok(width) = self.ast.eval_integral(expr) else {
            self.diagnose_at(
                DiagnosticCode::UnexpectedToken,
                node.span(),
                "a bitfield width must be an integer constant",
                "DDS-XTypes 1.3 §7.3.1.2.1.11",
            );
            return;
        };
        let Ok(bits) = u16::try_from(width) else {
            self.diagnose_at(
                DiagnosticCode::UnexpectedToken,
                node.span(),
                "a bitfield width does not fit in 16 bits",
                "DDS-XTypes 1.3 §7.3.1.2.1.11",
            );
            return;
        };
        let holder = self
            .type_ref(node)
            .filter(|type_| !matches!(type_, Ref::Broken(_)));
        let name = first_ident(node);
        let result = self.ast.add_bitfield(bitset, name.as_deref(), bits, holder);
        let Some(id) = self.report_model(result, node.span()) else {
            return;
        };
        let annotations = self.annotations_of(node);
        if let Ok(node_id) = self.ast.bitfield(id).map(|f| f.node_id) {
            self.apply_annotations(&annotations, AnyId::Bitfield(id, node_id));
        }
    }

    fn interface(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let kind = interface_kind(node);
        let (bases, supports) = self.interface_bases(node);
        let Some(id) = self.declare_interface(&name, kind, bases, node) else {
            return;
        };
        let node_id = self.ast.interface(id).map(|i| i.node_id).ok();
        if let Some(node_id) = node_id {
            self.record_origin(AnyId::Interface(id, node_id), node.span());
            self.scopes.bind(&name, AnyId::Interface(id, node_id));
            self.apply_waiting_annotations(AnyId::Interface(id, node_id));
            if !supports.is_empty() {
                if let Some(facets) = self.ast.facets_mut(AnyId::Interface(id, node_id)) {
                    facets.corba_mut().supports = supports;
                }
                self.ast.reindex_pending(AnyId::Interface(id, node_id));
            }
        }

        // The body is its own scope: a type declared inside an interface
        // is named through the interface. IDL 4.2 §7.5.
        if let Some(clause) = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::ManagesClause)
        {
            self.manages_clause(&clause, id);
        }

        self.one_namespace_per_scope(node);
        self.scopes.push_container(&name);
        for child in node.children() {
            match child.kind() {
                CstKind::Operation => self.operation(&child, id),
                CstKind::AttributeDecl => self.attribute(&child, id),
                CstKind::PortDecl => self.port(&child, id),
                CstKind::InheritanceClause | CstKind::ManagesClause => {}
                _ => self.definition(&child),
            }
        }
        self.scopes.pop();
    }

    /// The base interfaces of an interface, and the ones it supports.
    ///
    /// `component C : B supports I1, I2` writes two inheritance clauses
    /// and they mean different things. Taking only the first dropped
    /// every `supports` clause a component, home or connector carried,
    /// with no diagnostic — CCM 4.0 §6.3.2 needs both lists and cannot
    /// tell them apart once they are merged. The valuetype path already
    /// walked every clause; this one stopped at the first.
    fn interface_bases(
        &mut self,
        node: &Arc<SyntaxNode>,
    ) -> (
        Vec<Ref<zerodds_ast::InterfaceNode>>,
        Vec<Ref<zerodds_ast::InterfaceNode>>,
    ) {
        let mut bases = Vec::new();
        let mut supports = Vec::new();
        for clause in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::InheritanceClause)
        {
            let is_supports = clause
                .syntax_tokens()
                .iter()
                .any(|(_, token)| token.kind() == TokenKind::Supports);
            let names: Vec<Ref<zerodds_ast::InterfaceNode>> = clause
                .children()
                .into_iter()
                .filter(|child| child.kind() == CstKind::ScopedName)
                .filter_map(|scoped| self.scoped_name_of(&scoped))
                .map(|name| self.interface_ref(&name))
                .collect();
            if is_supports {
                supports.extend(names);
            } else {
                bases.extend(names);
            }
        }
        (bases, supports)
    }

    /// One port of a component, home, porttype or connector.
    ///
    /// A port is a named access point with an interface or event type
    /// behind it, which is an attribute with a component role on it.
    /// Making each port kind its own node kind would widen the type
    /// space every consumer matches on for a dialect most of them never
    /// see. Spec §4.2 of the AST.
    fn port(&mut self, node: &Arc<SyntaxNode>, interface: Id<zerodds_ast::InterfaceNode>) {
        let keyword = node.syntax_tokens().first().map(|(_, token)| token.kind());
        // The port's own name is the last identifier; the one before it
        // names the interface or event type.
        let idents: Vec<String> = node
            .syntax_tokens()
            .into_iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            .collect();
        let Some(port_name) = idents.last().cloned() else {
            return;
        };
        let target = self
            .scoped_names_of(node)
            .into_iter()
            .next()
            .or_else(|| self.scoped_name_of(node));

        let type_ = target.as_ref().map_or_else(
            || Ref::Broken(broken_cause()),
            |name| self.named_type_ref(name),
        );
        let Ok(id) = self.ast.add_attribute(interface, &port_name, type_, false) else {
            return;
        };
        let Ok(node_id) = self.ast.attribute(id).map(|a| a.node_id) else {
            return;
        };
        self.record_origin(AnyId::Attribute(id, node_id), node.span());

        let multiple = node
            .syntax_tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::Multiple);
        let interface_ref = target.as_ref().map_or_else(
            || Ref::Broken(broken_cause()),
            |name| self.interface_ref(name),
        );
        let event_ref = type_ref_of_port(self.ast, id);
        let role = match keyword {
            Some(TokenKind::Provides) => Some(zerodds_ast::ComponentRole::Facet {
                name: port_name.clone(),
                interface: interface_ref,
            }),
            Some(TokenKind::Uses) => Some(zerodds_ast::ComponentRole::Receptacle {
                name: port_name.clone(),
                interface: interface_ref,
                multiple,
            }),
            Some(TokenKind::Publishes | TokenKind::Emits) => {
                Some(zerodds_ast::ComponentRole::EventSource {
                    name: port_name.clone(),
                    event: event_ref,
                    publisher: keyword == Some(TokenKind::Publishes),
                })
            }
            Some(TokenKind::Consumes) => Some(zerodds_ast::ComponentRole::EventSink {
                name: port_name.clone(),
                event: event_ref,
            }),
            Some(TokenKind::Port) => Some(zerodds_ast::ComponentRole::PortTypeRole),
            Some(TokenKind::Mirrorport) => Some(zerodds_ast::ComponentRole::ConnectorRole),
            _ => None,
        };
        if let Some(role) = role {
            if let Some(facets) = self.ast.facets_mut(AnyId::Attribute(id, node_id)) {
                facets.corba_mut().component_role = Some(role);
            }
        }
        self.ast.reindex_pending(AnyId::Attribute(id, node_id));
    }

    /// A home's `manages X primarykey K` clause.
    fn manages_clause(
        &mut self,
        node: &Arc<SyntaxNode>,
        interface: Id<zerodds_ast::InterfaceNode>,
    ) {
        let names = self.scoped_names_of(node);
        let Some(component) = names.first() else {
            return;
        };
        let component_ref = self.interface_ref(component);
        let primary_key = names.get(1).map(|name| self.named_type_ref(name));
        let Ok(node_id) = self.ast.interface(interface).map(|i| i.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Interface(interface, node_id)) {
            facets.corba_mut().manages = Some(zerodds_ast::HomeManages {
                component: component_ref,
                primary_key,
            });
        }
        // Both references may name a declaration that has not arrived
        // yet; without this the waiter index never learns of them and
        // `manages C primarykey K` stays pending for the life of the
        // model.
        self.ast
            .reindex_pending(AnyId::Interface(interface, node_id));
    }

    /// `typeid X "id";` and `typeprefix X "prefix";`
    ///
    /// Both name a declaration that may not exist yet, so they are kept
    /// until the whole file is walked and applied in the completion
    /// pass. Spec §5.
    fn repository_directive(&mut self, node: &Arc<SyntaxNode>) {
        let is_prefix = node
            .syntax_tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::Typeprefix);
        let Some(name) = self.scoped_name_of(node) else {
            return;
        };
        let Some(text) = joined_string_literals(node.syntax_tokens(), TokenKind::StringLiteral)
            .into_iter()
            .next()
        else {
            return;
        };
        self.record_repository_directive(if is_prefix { "typeprefix" } else { "typeid" }, &text);
        self.repository_directives
            .push((self.directive_candidates(node, &name), text, is_prefix));
    }

    /// Note on the root module that a `typeid`/`typeprefix` directive was
    /// used, so the RPC layer can see it.
    ///
    /// The directive itself writes onto its target and leaves no node
    /// behind; DDS-RPC 1.0 §7.3.1.1 removes the directive from the
    /// service-definition grammar, so a service definition that carries
    /// one is outside it and `lower_service` has to report it.
    fn record_repository_directive(&mut self, keyword: &str, text: &str) {
        let root = self.ast.root_module();
        let Ok(node_id) = self.ast.module(root).map(|m| m.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Module(root, node_id)) {
            facets
                .corba_mut()
                .repository_directives
                .push(alloc::format!("{keyword} {text}"));
        }
    }

    /// `import X;` — kept as written on the root module.
    fn import(&mut self, node: &Arc<SyntaxNode>) {
        let written = joined_string_literals(node.syntax_tokens(), TokenKind::StringLiteral)
            .into_iter()
            .next()
            .or_else(|| {
                self.scoped_name_of(node)
                    .map(|name| name.segments().join("::"))
            });
        let Some(written) = written else {
            return;
        };
        let root = self.ast.root_module();
        let Ok(node_id) = self.ast.module(root).map(|m| m.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Module(root, node_id)) {
            facets.corba_mut().imports.push(written);
        }
    }

    /// `keylist T (a, b);` — RTI's way of declaring a type's keys.
    fn keylist(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = self.scoped_name_of(node) else {
            return;
        };
        // The type's name is a `ScopedName` child; the identifiers
        // directly under the declaration are the key fields.
        let fields: Vec<String> = node
            .syntax_tokens()
            .into_iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            .collect();
        self.keylists
            .push((self.directive_candidates(node, &name), fields, node.span()));
    }

    /// Every scope a directive's target name could be in, innermost
    /// first.
    ///
    /// §7.5 resolves a name from the scope that writes it outwards, and
    /// a directive's argument is a name being used. Qualifying it with
    /// the current scope and stopping there made `module m { typeprefix
    /// m "…"; };` name `m::m` — a scope that does not exist — and the
    /// directive was reported as naming nothing while `::m` sat one step
    /// out. `typeid`, `typeprefix` and `keylist` all read their target
    /// this way.
    ///
    /// A name written root-anchored means the root and nothing else, so
    /// it gets one candidate. Rule (4) `<scoped_name> ::+ "::"
    /// <identifier>`.
    fn directive_candidates(&self, node: &Arc<SyntaxNode>, name: &ScopedName) -> Vec<ScopedName> {
        if is_root_anchored(node) {
            return alloc::vec![name.clone()];
        }
        self.scopes.lookup_candidates(name.segments())
    }

    /// A template module's formal parameters.
    fn template_parameters(&mut self, node: &Arc<SyntaxNode>, module: Id<zerodds_ast::ModuleNode>) {
        let mut params = Vec::new();
        for parameter in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::TemplateParameter)
        {
            let Some(name) = first_ident(&parameter) else {
                continue;
            };
            let kind = template_param_kind(&parameter);
            // Two of (188)'s eleven alternatives carry a type: `const
            // <const_type>` names the type an actual constant must have,
            // and `<sequence_type>` names the sequence the actual must
            // be. Both are the parameter's constraint.
            let constraint = if matches!(
                kind,
                zerodds_ast::TemplateParamKind::Const | zerodds_ast::TemplateParamKind::Sequence
            ) {
                self.type_ref(&parameter)
            } else {
                None
            };
            params.push(zerodds_ast::TemplateParam {
                name,
                kind,
                constraint,
            });
        }
        if params.is_empty() {
            return;
        }
        let Ok(node_id) = self.ast.module(module).map(|m| m.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Module(module, node_id)) {
            facets.corba_mut().template_params = params;
        }
        self.ast.reindex_pending(AnyId::Module(module, node_id));
    }

    /// Every scoped name written directly under a node.
    pub(crate) fn scoped_names_of(&self, node: &Arc<SyntaxNode>) -> Vec<ScopedName> {
        node.children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::ScopedName)
            .filter_map(|scoped| self.scoped_name_of(&scoped))
            .collect()
    }

    /// The string literals of a `context (...)` clause, with escapes
    /// resolved.
    fn context_strings(&self, node: &Arc<SyntaxNode>) -> Vec<String> {
        joined_string_literals(node.syntax_tokens(), TokenKind::StringLiteral)
    }

    /// A reference to an exception by name, resolved if already bound.
    fn exception_ref(&self, name: &ScopedName) -> Ref<zerodds_ast::ExceptionNode> {
        if let Some(segment) = name.segments().last() {
            if let Some(AnyId::Exception(id, _)) = self.scopes.lookup(segment) {
                return Ref::Resolved(id);
            }
        }
        Ref::Pending(PendingRef::ScopedName(name.clone()))
    }

    /// A reference to a named type, resolved if already bound.
    fn named_type_ref(&self, name: &ScopedName) -> Ref<TypeNode> {
        if let Some(segment) = name.segments().last() {
            if let Some(found) = self.scopes.lookup(segment).and_then(AnyId::as_type) {
                return Ref::Resolved(found);
            }
        }
        Ref::Pending(PendingRef::ScopedName(name.clone()))
    }

    /// A reference to an interface by name, resolved if already bound.
    fn interface_ref(&self, name: &ScopedName) -> Ref<zerodds_ast::InterfaceNode> {
        if let Some(segment) = name.segments().last() {
            if let Some(AnyId::Interface(id, _)) = self.scopes.lookup(segment) {
                return Ref::Resolved(id);
            }
        }
        Ref::Pending(PendingRef::ScopedName(name.clone()))
    }

    fn operation(&mut self, node: &Arc<SyntaxNode>, interface: Id<zerodds_ast::InterfaceNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        // `void` has no type specification; the model's own void is the
        // primitive of that name.
        // `void` writes no type specification at all; the model records
        // the absence rather than a stand-in type.
        let return_type = self.type_ref(node);
        let oneway = node
            .syntax_tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::Oneway);
        let Ok(id) = self
            .ast
            .add_operation(interface, &name, return_type, oneway)
        else {
            return;
        };
        let annotations = self.annotations_of(node);
        let node_id = self.ast.operation(id).map(|o| o.node_id).ok();
        if let Some(node_id) = node_id {
            self.record_origin(AnyId::Operation(id, node_id), node.span());
            self.apply_annotations(&annotations, AnyId::Operation(id, node_id));
        }

        self.one_namespace_per_scope(node);
        for parameter in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Parameter)
        {
            self.parameter(&parameter, id);
        }
        for raises in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::RaisesClause)
        {
            for name in self.scoped_names_of(&raises) {
                let reference = self.exception_ref(&name);
                let _ = self.ast.add_raises(id, reference);
            }
        }
        for context in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::ContextClause)
        {
            for context in self.context_strings(&context) {
                let _ = self.ast.add_context(id, &context);
            }
        }
    }

    fn parameter(&mut self, node: &Arc<SyntaxNode>, operation: Id<zerodds_ast::OperationNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let direction = node
            .syntax_tokens()
            .iter()
            .find_map(|(_, token)| match token.kind() {
                TokenKind::In => Some(ParamDirection::In),
                TokenKind::Out => Some(ParamDirection::Out),
                TokenKind::Inout => Some(ParamDirection::InOut),
                _ => None,
            })
            .unwrap_or(ParamDirection::In);
        let type_ = self
            .type_ref(node)
            .unwrap_or_else(|| Ref::Broken(broken_cause()));
        if let Ok(id) = self.ast.add_param(operation, &name, direction, type_) {
            let annotations = self.annotations_of(node);
            let node_id = self.ast.param(id).map(|p| p.node_id).ok();
            if let Some(node_id) = node_id {
                self.record_origin(AnyId::Param(id, node_id), node.span());
                self.apply_annotations(&annotations, AnyId::Param(id, node_id));
            }
        }
    }

    fn attribute(&mut self, node: &Arc<SyntaxNode>, interface: Id<zerodds_ast::InterfaceNode>) {
        let readonly = node
            .syntax_tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::Readonly);
        // `attribute long a, b;` declares two attributes of one type.
        for (name, type_, at) in self.declared(node) {
            let Ok(id) = self.ast.add_attribute(interface, &name, type_, readonly) else {
                continue;
            };
            let annotations = self.annotations_of(node);
            let node_id = self.ast.attribute(id).map(|a| a.node_id).ok();
            if let Some(node_id) = node_id {
                self.record_origin(AnyId::Attribute(id, node_id), at.span());
                self.apply_annotations(&annotations, AnyId::Attribute(id, node_id));
            }
            for raises in node
                .children()
                .into_iter()
                .filter(|child| child.kind() == CstKind::RaisesClause)
            {
                let setter = raises
                    .syntax_tokens()
                    .iter()
                    .any(|(_, token)| token.kind() == TokenKind::Setraises);
                for name in self.scoped_names_of(&raises) {
                    let reference = self.exception_ref(&name);
                    let _ = self.ast.add_attribute_raises(id, reference, setter);
                }
            }
        }
    }

    fn valuetype(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let kind = valuetype_kind(node);
        let clauses: Vec<Arc<SyntaxNode>> = node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::InheritanceClause)
            .collect();
        // The first clause inherits valuetypes, a `supports` clause
        // names interfaces; both are written the same way.
        let mut bases = Vec::new();
        let mut supported = Vec::new();
        for clause in clauses {
            let supports = clause
                .syntax_tokens()
                .iter()
                .any(|(_, token)| token.kind() == TokenKind::Supports);
            for scoped in self.scoped_names_of(&clause) {
                if supports {
                    supported.push(self.interface_ref(&scoped));
                } else {
                    bases.push(self.named_type_ref(&scoped));
                }
            }
        }

        let declared = TypeKind::Valuetype {
            kind,
            bases,
            state: Vec::new(),
            initializers: Vec::new(),
            supported,
        };

        // `valuetype V;` and `abstract eventtype E;` are forward
        // declarations — rule (105) — and a forward declaration is a
        // completeness state. Declaring one complete made the definition
        // that follows a `DuplicateName` instead of the body it is.
        if !has_body(node) {
            if self.scopes.lookup(&name).is_some() {
                return;
            }
            let Some(module) = self.scopes.current_module() else {
                return;
            };
            if let Ok(id) = self.ast.add_forward_type(module, &name, declared) {
                self.record_origin(AnyId::Type(id, self.node_id_of(id)), node.span());
                self.record_declaration_scope(AnyId::Type(id, self.node_id_of(id)), node.span());
                if is_event_type(node) {
                    if let Some(facets) = self.ast.facets_mut(AnyId::Type(id, self.node_id_of(id)))
                    {
                        facets.corba_mut().is_event_type = true;
                    }
                }
                self.bind(&name, id);
            }
            return;
        }

        let Some(id) = self.declare_type(&name, declared, node) else {
            return;
        };
        // `eventtype` and `valuetype` lower to the same kind of node, so
        // the keyword is the only thing left that tells them apart, and
        // CCM 4.0 §6.6.1.1 maps the two differently.
        if is_event_type(node) {
            if let Some(facets) = self.ast.facets_mut(AnyId::Type(id, self.node_id_of(id))) {
                facets.corba_mut().is_event_type = true;
            }
        }
        self.one_namespace_per_scope(node);
        // The body's own scope, so a `raises` clause and a parameter
        // type inside an initialiser resolve the way an interface's do.
        self.scopes.push_container(&name);
        for child in node.children() {
            match child.kind() {
                CstKind::Member => self.member(&child, id),
                // Rule (105) puts `factory` initialisers and exported
                // operations in a valuetype body beside its state. This
                // loop used to take `Member` and nothing else, so every
                // one of them was parsed, kept in the CST and dropped
                // here — `TypeKind::Valuetype::initializers` came back
                // empty for every input in the tree, and the CORBA Rust
                // generator's `factory` output disappeared with no
                // diagnostic.
                CstKind::Operation => self.valuetype_operation(&child, id),
                // Rule (105)'s `<export>` also admits an attribute.
                // `AttributeNode::parent` is an `Id<InterfaceNode>`, so
                // the model has no place to put one declared here — and
                // an arm that did nothing dropped the declaration with
                // no diagnostic. Named instead: a gap that reports is a
                // gap, a gap that is silent is an acceptance claim.
                CstKind::AttributeDecl => self.diagnose_at(
                    DiagnosticCode::Unsupported,
                    child.span(),
                    "an attribute declared in a valuetype body has no model form yet; \
                     the model attaches an attribute to an interface",
                    "OMG IDL 4.2 §7.4.5",
                ),
                // The clause was read into `bases` and `supported`
                // above, before the body's scope was pushed.
                CstKind::InheritanceClause => {}
                // Rule (105)'s remaining `<export>` forms — a nested
                // `const`, `typedef`, `struct`, `union`, `enum`,
                // `bitset`, `bitmask`, `native` or `exception`. They
                // lower into the scope this body pushed, exactly as an
                // interface's do, and taking `Member` and `Operation`
                // and nothing else dropped every one of them:
                // `crates/idl-parse/tests/fixtures/spec_features/corba_value_repository.idl`
                // declares `exception Bad {};` inside `valuetype V` and
                // names it in `factory create(..) raises (Bad)`, so the
                // `raises` reference had no declaration to resolve to.
                _ => self.definition(&child),
            }
        }
        self.scopes.pop();
    }

    /// An operation inside a `valuetype` body — a `factory` initialiser
    /// or an exported operation. Rule (105) and rule (107).
    ///
    /// The two are told apart by the `factory` keyword, which the
    /// parser keeps as a token of the operation's own node.
    fn valuetype_operation(&mut self, node: &Arc<SyntaxNode>, valuetype: Id<TypeNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let initializer = node
            .syntax_tokens()
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::Factory);
        // An initialiser has no return type — rule (107) — and an
        // exported operation reads one the same way an interface's
        // does.
        let return_type = if initializer {
            None
        } else {
            self.type_ref(node)
        };
        let Ok(id) = self
            .ast
            .add_valuetype_operation(valuetype, &name, return_type, initializer)
        else {
            return;
        };
        let annotations = self.annotations_of(node);
        let node_id = self.ast.operation(id).map(|o| o.node_id).ok();
        if let Some(node_id) = node_id {
            self.record_origin(AnyId::Operation(id, node_id), node.span());
            self.apply_annotations(&annotations, AnyId::Operation(id, node_id));
        }
        for parameter in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Parameter)
        {
            self.parameter(&parameter, id);
        }
        for raises in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::RaisesClause)
        {
            for scoped in self.scoped_names_of(&raises) {
                let reference = self.exception_ref(&scoped);
                let _ = self.ast.add_raises(id, reference);
            }
        }
    }

    /// `valuetype VB long;` — a boxed value. Rule (126).
    ///
    /// `TypeKind::ValueBox` was in `crates/ast` from the start and
    /// unreachable from IDL: the parser committed to a body at the name
    /// and answered "expected `{` opening the valuetype body".
    fn value_box(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        let boxed = self
            .type_ref(node)
            .unwrap_or_else(|| Ref::Broken(broken_cause()));
        let _ = self.declare_type(&name, TypeKind::ValueBox { boxed }, node);
    }

    /// The `struct … //@annotation` form of an annotation declaration
    /// (DDS-XTypes 1.3 §7.3.1.2.4, Table 22).
    ///
    /// The members are the structure's. A default value is written as the
    /// member's `//@default(value)`, the one place a legacy compiler lets
    /// a member carry one.
    fn annotation_from_struct(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        self.check_collision(&name, node);
        let Some(module) = self.scopes.current_module() else {
            return;
        };
        let Ok(id) = self.ast.add_annotation_def(module, &name) else {
            return;
        };
        if let Ok(node_id) = self.ast.annotation_def(id).map(|a| a.node_id) {
            self.record_origin(AnyId::AnnotationDef(id, node_id), node.span());
            self.scopes.bind(&name, AnyId::AnnotationDef(id, node_id));
        }
        self.scopes.push(&name, None);
        for child in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Member)
        {
            let default = self
                .annotations_of(&child)
                .into_iter()
                .chain(self.suffix_annotations_of(&child))
                .find(|applied| applied.name == "default")
                .and_then(|applied| applied.arguments.first().and_then(|a| a.value));
            for (member_name, type_ref, at) in self.declared(&child) {
                let result = self
                    .ast
                    .add_annotation_member(id, &member_name, type_ref, default);
                self.report_model(result, at.span());
            }
        }
        self.scopes.pop();
    }

    /// An `@annotation` declaration.
    fn annotation_decl(&mut self, node: &Arc<SyntaxNode>) {
        let Some(name) = first_ident(node) else {
            return;
        };
        self.check_collision(&name, node);
        let Some(module) = self.scopes.current_module() else {
            return;
        };
        let Ok(id) = self.ast.add_annotation_def(module, &name) else {
            return;
        };
        let node_id = self.ast.annotation_def(id).map(|a| a.node_id).ok();
        if let Some(node_id) = node_id {
            self.record_origin(AnyId::AnnotationDef(id, node_id), node.span());
            // Bound in the scope so a use of `@Name` finds its
            // declaration rather than waiting on a name forever.
            self.scopes.bind(&name, AnyId::AnnotationDef(id, node_id));
        }
        // The body is its own scope, the way an interface body is. Rule
        // (221) admits an `enum`, a `const` and a `typedef` beside the
        // members — `@annotation Mode { enum Kind { READ, WRITE }; Kind
        // kind default READ; };` — and a member's type names one of
        // them. They were parsed and never lowered, which nothing
        // noticed while the members themselves were being dropped; with
        // the members read, `Kind` and `READ` had nowhere to resolve.
        self.scopes.push(&name, None);
        for child in node.children() {
            if matches!(
                child.kind(),
                CstKind::EnumDecl | CstKind::ConstDecl | CstKind::TypedefDecl
            ) {
                self.definition(&child);
            }
        }

        for child in node
            .children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Member)
        {
            // Read directly rather than through `Self::declared`.
            //
            // `declared` walks `CstKind::Declarator` children, and rule
            // (222) `<annotation_member> ::= <annotation_member_type>
            // <simple_declarator> [ "default" <const_expr> ] ";"` admits
            // exactly one name, which `parse_annotation_decl` reads with
            // a bare `expect(Ident)` and wraps in no `Declarator` node.
            // So `declared` found none, returned an empty list, and
            // every annotation's members were dropped without a word:
            // `@annotation Aa { long v; };` reached the model as a
            // declaration with zero members. Nothing downstream could
            // check an application against it, which is why all four of
            // IDL 4.2 §7.4.15.4.2's rules had nothing to hold.
            //
            // `first_ident` reads the node's own tokens and not its
            // descendants', so the name of `Ea v;` is `v` and not `Ea`.
            let Some(member_name) = first_ident(&child) else {
                self.diagnose_at(
                    DiagnosticCode::MissingToken,
                    child.span(),
                    "an annotation member needs a name",
                    "OMG IDL 4.2 §7.4.15.3 rule (222)",
                );
                continue;
            };
            let type_ = self
                .type_ref(&child)
                .unwrap_or_else(|| Ref::Broken(broken_cause()));
            let default = self.first_expr_child(&child);
            let result = self
                .ast
                .add_annotation_member(id, &member_name, type_, default);
            self.report_model(result, child.span());
        }
        self.scopes.pop();
    }

    // -- helpers ---------------------------------------------------------

    /// The first type specification among a node's children.
    fn type_ref(&mut self, node: &Arc<SyntaxNode>) -> Option<Ref<TypeNode>> {
        let type_node = node.children().into_iter().find(|child| {
            matches!(
                child.kind(),
                CstKind::PrimitiveType
                    | CstKind::NamedType
                    | CstKind::SequenceType
                    | CstKind::StringType
                    | CstKind::WStringType
                    | CstKind::MapType
                    | CstKind::FixedType
            )
        })?;
        Some(self.type_ref_of(&type_node))
    }

    fn type_ref_of(&mut self, node: &Arc<SyntaxNode>) -> Ref<TypeNode> {
        match node.kind() {
            CstKind::PrimitiveType => self.primitive_ref(node),
            CstKind::StringType | CstKind::WStringType => {
                let width = if node.kind() == CstKind::StringType {
                    CharWidth::Char
                } else {
                    CharWidth::WChar
                };
                // `string<N>` writes its bound in the argument list.
                let bound = self.bound_argument(node, 0);
                self.anonymous(TypeKind::String { width, bound }, node.span())
            }
            CstKind::SequenceType => {
                let element = self.type_argument(node, 0);
                let bound = self.bound_argument(node, 1);
                let sequence = self.anonymous(TypeKind::Sequence { element, bound }, node.span());
                self.use_site_annotations(node, 0, &sequence, UseSite::Element);
                sequence
            }
            CstKind::MapType => {
                let key = self.type_argument(node, 0);
                let value = self.type_argument(node, 1);
                let bound = self.bound_argument(node, 2);
                let map = self.anonymous(TypeKind::Map { key, value, bound }, node.span());
                self.use_site_annotations(node, 0, &map, UseSite::MapKey);
                self.use_site_annotations(node, 1, &map, UseSite::Element);
                map
            }
            CstKind::FixedType => self.fixed_ref(node),
            CstKind::NamedType => self.type_ref_from_scoped_name(node).unwrap_or_else(|| {
                Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(Vec::new())))
            }),
            _ => Ref::Broken(broken_cause()),
        }
    }

    /// The type at one position of a parameterised type's argument list.
    fn type_argument(&mut self, node: &Arc<SyntaxNode>, position: usize) -> Ref<TypeNode> {
        let Some(argument) = self.argument_at(node, position) else {
            return Ref::Broken(broken_cause());
        };
        self.type_ref_of(&argument)
    }

    /// The bound at one position of a parameterised type's argument list.
    ///
    /// Absent means unbounded; a malformed expression is not silently
    /// taken for one, it becomes a diagnostic in the expression lowering.
    fn bound_argument(&mut self, node: &Arc<SyntaxNode>, position: usize) -> Bound {
        let Some(argument) = self.argument_at(node, position) else {
            return Bound::Unbounded;
        };
        self.expr(&argument).map_or(Bound::Unbounded, Bound::Const)
    }

    fn argument_at(&self, node: &Arc<SyntaxNode>, position: usize) -> Option<Arc<SyntaxNode>> {
        self.arguments_of(node)?
            .into_iter()
            .nth(position)
            .map(|(_, argument)| argument)
    }

    /// A parameterised type's arguments, each with the annotations
    /// written immediately in front of it.
    ///
    /// `sequence<@try_construct(TRIM) String5, 10>` puts the annotation
    /// in the same child list as the arguments, so indexing by child
    /// position would count it as an argument and read the bound out of
    /// the element slot. Grouping is by adjacency: annotations attach to
    /// the argument that follows them, which is the order the source
    /// writes and the only grouping the grammar admits.
    fn arguments_of(&self, node: &Arc<SyntaxNode>) -> Option<Vec<TemplateArgument>> {
        let list = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::TypeArguments)?;
        let mut arguments: Vec<TemplateArgument> = Vec::new();
        let mut pending: Vec<Arc<SyntaxNode>> = Vec::new();
        for child in list.children() {
            if child.kind() == CstKind::Annotation {
                pending.push(child);
            } else {
                arguments.push((core::mem::take(&mut pending), child));
            }
        }
        // Annotations with no argument after them are a malformed
        // template — `sequence<@try_construct(TRIM)>`. They are dropped
        // here rather than attached to the previous argument, and the
        // parser has already reported the missing type.
        Some(arguments)
    }

    /// Read the annotations in front of one template argument and apply
    /// them to the slot they belong to on the collection itself.
    ///
    /// The collection and not the element type: the element type is
    /// shared — `String5` in §7.3.1.2.1.12.4 is a `typedef` that any
    /// number of members may use — and the behaviour belongs to this one
    /// use of it.
    fn use_site_annotations(
        &mut self,
        node: &Arc<SyntaxNode>,
        position: usize,
        collection: &Ref<TypeNode>,
        site: UseSite,
    ) {
        let written = self.argument_annotations(node, position);
        if written.is_empty() {
            return;
        }
        let Some(id) = collection.resolved() else {
            return;
        };
        let any = AnyId::Type(id, self.node_id_of(id));
        self.apply_use_site(&written, any, site);
    }

    /// The annotations written in front of one template argument.
    fn argument_annotations(
        &mut self,
        node: &Arc<SyntaxNode>,
        position: usize,
    ) -> Vec<crate::annotation::Applied> {
        let Some(arguments) = self.arguments_of(node) else {
            return Vec::new();
        };
        let Some((written, _)) = arguments.into_iter().nth(position) else {
            return Vec::new();
        };
        written
            .iter()
            .filter_map(|annotation| self.applied_annotation(annotation))
            .collect()
    }

    /// `fixed<d, s>`, whose parameters are digits and scale rather than
    /// types. IDL 4.2 §7.4.1.4.4.
    fn fixed_ref(&mut self, node: &Arc<SyntaxNode>) -> Ref<TypeNode> {
        let digits = self.fixed_parameter(node, 0);
        let scale = self.fixed_parameter(node, 1);
        match (digits, scale) {
            (Some(digits), Some(scale)) => {
                self.anonymous(TypeKind::Fixed { digits, scale }, node.span())
            }
            // A `fixed` whose parameters do not evaluate is broken rather
            // than defaulted: a silently chosen precision would change
            // the wire representation.
            _ => {
                // Two rules and two messages. (42) `<fixed_pt_type> ::=
                // "fixed" "<" <positive_int_const> ","
                // <positive_int_const> ">"` is a member's; (43)
                // `<fixed_pt_const_type> ::= "fixed"` is a constant's
                // and takes none. Naming (42) for a `const fixed` was
                // what made rule (43) look refused for a reason.
                let message = if has_type_arguments(node) {
                    "the digits and scale of `fixed` must both be integer constants"
                } else {
                    "a `fixed` used as a type needs its digits and scale — `fixed<d,s>`. Only a \
                     constant's `fixed` is written without them, and takes them from its value"
                };
                self.diagnose_at(
                    DiagnosticCode::UnexpectedToken,
                    node.span(),
                    message,
                    "OMG IDL 4.2 §7.4.1.4.4 rule (42)",
                );
                Ref::Broken(broken_cause())
            }
        }
    }

    /// The `fixed<d,s>` a `const fixed` takes from its own value.
    ///
    /// Rule (43) writes the type without parameters, so the value
    /// decides. §7.4.1.4.3, p. 31 says how, and Table 7-11 gives the
    /// arithmetic: "A fixed-point literal has the apparent number of
    /// total and fractional digits. For example, 0123.450d is considered
    /// to be fixed<7,3> and 3000.00d is fixed<6,2>. Prefix operators do
    /// not affect the precision".
    fn fixed_const_type(
        &mut self,
        expr: Id<ConstExprNode>,
        node: &Arc<SyntaxNode>,
    ) -> Ref<TypeNode> {
        match self.fixed_type_of(expr, 0) {
            Ok((digits, scale)) => self.anonymous(TypeKind::Fixed { digits, scale }, node.span()),
            Err(reason) => {
                self.diagnose_at(
                    DiagnosticCode::UnexpectedToken,
                    node.span(),
                    reason,
                    "OMG IDL 4.2 §7.4.1.4.3, Table 7-11",
                );
                Ref::Broken(broken_cause())
            }
        }
    }

    /// The digits and scale one expression of a fixed constant has.
    ///
    /// Table 7-11, p. 31, for `fixed<d1,s1> op fixed<d2,s2>`:
    ///
    /// | op | result |
    /// |---|---|
    /// | `+` | `fixed<max(d1-s1,d2-s2)+max(s1,s2)+1, max(s1,s2)>` |
    /// | `-` | the same as `+` |
    /// | `*` | `fixed<d1+d2, s1+s2>` |
    /// | `/` | `fixed<(d1-s1+s2)+sinf, sinf>` |
    ///
    /// `sinf` is the clause's own name for a precision it leaves to the
    /// implementation. The broadest reading is taken and written down
    /// here: `sinf` fills the result to the 31 digits §7.4.1.4.3 caps
    /// every fixed at, so a quotient keeps as many fractional digits as
    /// the type can hold. A narrower choice would discard digits the
    /// type has room for, and the clause gives no reason to.
    ///
    /// An integer operand is refused rather than promoted. Measured on
    /// the Linux test host, all four front ends that take `const fixed`
    /// at all refuse `3.14d + 1`: omniORB with "Cannot interpret integer
    /// literal as fixed point", JacORB with "Expecting fixed point
    /// constant", OpenDDS and TAO with "incompatible types in constant
    /// assignment".
    fn fixed_type_of(
        &self,
        expr: Id<ConstExprNode>,
        depth: usize,
    ) -> Result<(u8, u8), &'static str> {
        if depth > MAX_FIXED_EXPR_DEPTH {
            return Err("this fixed-point constant expression nests too deeply to evaluate");
        }
        let Ok(node) = self.ast.expr(expr) else {
            return Err("this fixed-point constant expression has no value");
        };
        match &node.expr {
            ConstExpr::Literal(zerodds_ast::Literal::Fixed { digits, scale }) => {
                let total = u8::try_from(digits.len()).map_err(
                    |_| "this fixed-point literal has more digits than `fixed` can hold",
                )?;
                Ok(cap_to_31_digits(total, *scale))
            }
            ConstExpr::Literal(zerodds_ast::Literal::Integer(_)) => Err(
                "an integer literal is not a fixed-point value; write it with a `d` suffix, as \
                 `1.0d`",
            ),
            ConstExpr::Literal(_) => {
                Err("a `const fixed` takes a fixed-point value, written with a `d` suffix")
            }
            // A name standing for another `const fixed` carries that
            // constant's own digits and scale.
            ConstExpr::NameRef(reference) => self.fixed_type_of_name(reference),
            ConstExpr::Unary { op, operand } => match op {
                // "Prefix operators do not affect the precision."
                UnaryOp::Plus | UnaryOp::Minus => self.fixed_type_of(*operand, depth + 1),
                UnaryOp::Not => Err(
                    "`~` is not applicable to a fixed-point expression: §7.4.1.4.3 admits unary \
                     `+` and `-` and binary `*`, `/`, `+` and `-`",
                ),
            },
            ConstExpr::Binary { op, lhs, rhs } => {
                let (d1, s1) = self.fixed_type_of(*lhs, depth + 1)?;
                let (d2, s2) = self.fixed_type_of(*rhs, depth + 1)?;
                fixed_binary(*op, d1, s1, d2, s2)
            }
        }
    }

    /// The digits and scale of the constant a name stands for.
    fn fixed_type_of_name(
        &self,
        reference: &Ref<zerodds_ast::ConstNode>,
    ) -> Result<(u8, u8), &'static str> {
        let named = match reference {
            Ref::Resolved(id) => Some(*id),
            Ref::Pending(PendingRef::ScopedName(name)) => name
                .segments()
                .last()
                .and_then(|segment| self.scopes.lookup(segment))
                .and_then(|any| match any {
                    AnyId::Const(id, _) => Some(id),
                    _ => None,
                }),
            _ => None,
        };
        let Some(named) = named else {
            return Err("this name does not stand for a constant declared before it");
        };
        let kind = self
            .ast
            .constant(named)
            .ok()
            .and_then(|constant| constant.type_.resolved())
            .and_then(|type_| self.ast.type_(type_).ok())
            .map(|node| node.kind.clone());
        match kind {
            Some(TypeKind::Fixed { digits, scale }) => Ok((digits, scale)),
            _ => Err("a fixed-point constant expression may only name another `const fixed`"),
        }
    }

    /// One parameter of `fixed<d, s>`, evaluated.
    fn fixed_parameter(&mut self, node: &Arc<SyntaxNode>, position: usize) -> Option<u8> {
        let argument = self.argument_at(node, position)?;
        let expr = self.expr(&argument)?;
        let value = self.ast.eval_integral(expr).ok()?;
        u8::try_from(value).ok()
    }

    /// A reference to the built-in a type keyword names.
    ///
    /// The fifteen primitives, and the built-in type names IDL provides
    /// that are not primitives. `any` is the first of those:
    /// `is_primitive_keyword` in `zerodds_idl_parse` admitted it, this
    /// function had no arm for it, and the member's type became
    /// `Ref::Broken` with **neither the parser nor the lowering saying
    /// anything**. The first complaint came from the layout planner
    /// three layers later, in a vocabulary about node ids.
    fn primitive_ref(&mut self, node: &Arc<SyntaxNode>) -> Ref<TypeNode> {
        let kinds: Vec<TokenKind> = node
            .syntax_tokens()
            .iter()
            .map(|(_, token)| token.kind())
            .filter(|kind| *kind != TokenKind::Error)
            .collect();
        if let Some((name, kind)) = builtin_of(&kinds) {
            return self.builtin_ref(name, kind);
        }
        let Some(primitive) = primitive_of(&kinds) else {
            return Ref::Broken(broken_cause());
        };
        self.builtin_ref(primitive.as_str(), TypeKind::Primitive(primitive))
    }

    /// One node per built-in per model, reused rather than repeated.
    ///
    /// A file with a hundred `long` members should not hold a hundred
    /// primitive types, and the same holds for `any`.
    fn builtin_ref(&mut self, name: &str, kind: TypeKind) -> Ref<TypeNode> {
        if let Some(existing) = self.scopes.lookup(name).and_then(AnyId::as_type) {
            return Ref::Resolved(existing);
        }
        let root = self.ast.root_module();
        match self.ast.add_type(root, name, kind) {
            Ok(id) => {
                self.scopes.bind(name, AnyId::Type(id, self.node_id_of(id)));
                Ref::Resolved(id)
            }
            Err(_) => Ref::Broken(broken_cause()),
        }
    }

    /// An anonymous type, which IDL allows inline in a member.
    fn anonymous(&mut self, kind: TypeKind, at: Span) -> Ref<TypeNode> {
        let root = self.ast.root_module();
        let result = self.ast.add_type(root, "", kind);
        self.report_model(result, at)
            .map_or_else(|| Ref::Broken(broken_cause()), Ref::Resolved)
    }

    /// A reference by name, resolved if the name is already bound and
    /// pending otherwise.
    ///
    /// Pending rather than an error: forward references are legal and
    /// the model is built to carry them. Spec §4.
    fn type_ref_from_scoped_name(&mut self, node: &Arc<SyntaxNode>) -> Option<Ref<TypeNode>> {
        let scoped = node
            .descendants()
            .into_iter()
            .find(|child| child.kind() == CstKind::ScopedName)?;
        let segments: Vec<String> = scoped
            .syntax_tokens()
            .iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            .collect();
        if segments.is_empty() {
            return None;
        }
        if segments.len() == 1 {
            if let Some(found) = self.scopes.lookup(&segments[0]) {
                if let Some(as_type) = found.as_type() {
                    return Some(Ref::Resolved(as_type));
                }
                // Found, and not a type. An interface named where a
                // type is expected is an object reference — §7.4.6 —
                // and `Ast::resolve_all_by_name` has the rule for that;
                // what it needs is a key it can look the declaration up
                // by. The written segment is *relative* to the scope
                // that wrote it, so recording it as an absolute name
                // asks the index for `::I` when the declaration is
                // `m::I`, and the reference breaks with `NotFound`.
                //
                // Measured: `module m { interface I {}; struct S { I r; };
                // };` broke, `interface I {}; struct S { I r; };` did
                // not, and `m::I` written out did not — the failure was
                // exactly the unqualified form inside a module, which is
                // how interface-typed members are normally written.
                if let Some(scoped) = self.ast.scoped_name_of(found) {
                    return Some(Ref::Pending(PendingRef::ScopedName(scoped)));
                }
            }
        }
        Some(Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(
            segments,
        ))))
    }

    /// The scoped name written under a node, absolute or relative as
    /// written.
    pub(crate) fn scoped_name_of(&self, node: &Arc<SyntaxNode>) -> Option<ScopedName> {
        let scoped = if node.kind() == CstKind::ScopedName {
            node.clone()
        } else {
            node.descendants()
                .into_iter()
                .find(|child| child.kind() == CstKind::ScopedName)?
        };
        let segments: Vec<String> = scoped
            .syntax_tokens()
            .iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            .collect();
        if segments.is_empty() {
            return None;
        }
        Some(ScopedName::absolute(segments))
    }

    /// Record a diagnostic at a byte range.
    pub(crate) fn diagnose_at(
        &mut self,
        code: DiagnosticCode,
        span: Span,
        message: &str,
        spec: &'static str,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code, span, message.to_string(), spec));
    }

    /// Record a warning at a byte range.
    ///
    /// For what the file may keep and the reader still has to hear
    /// about. The counterpart of [`Self::diagnose_at`], and separate
    /// from [`Self::diagnose_vendor`] because that one's severity is
    /// the dialect's answer, not the rule's.
    pub(crate) fn warn_at(
        &mut self,
        code: DiagnosticCode,
        span: Span,
        message: &str,
        spec: &'static str,
    ) {
        self.diagnostics
            .push(Diagnostic::warning(code, span, message.to_string(), spec));
    }

    /// Record that a construct outside OMG IDL 4.2 was written here.
    ///
    /// Always recorded; the dialect decides only the severity — an error
    /// when the flag is off, a warning when it is on. The parser's
    /// `diagnose_vendor` is the same rule one stage earlier, for the
    /// vendor constructs that are grammar rather than pragmas.
    ///
    /// Spec §3.5.
    pub(crate) fn diagnose_vendor(
        &mut self,
        flag: zerodds_idl_parse::VendorFlag,
        span: Span,
        message: &str,
    ) {
        let spec = "zerodds-idl-frontend-1.0 §3.5";
        let diagnostic = if self.options.dialect.admits(flag) {
            Diagnostic::warning(
                DiagnosticCode::VendorExtension,
                span,
                message.to_string(),
                spec,
            )
        } else {
            Diagnostic::error(
                DiagnosticCode::VendorExtension,
                span,
                message.to_string(),
                spec,
            )
        };
        self.diagnostics.push(diagnostic);
    }

    /// The declared type of a constant, checked against its value.
    ///
    /// IDL 4.2 §7.4.1.4.3: a constant declaration carries "a value
    /// expression (`<const_expr>`), which shall be consistent with the
    /// type declared for the constant". §7.2.6.3 states two of the cases
    /// as a requirement in so many words:
    ///
    /// > Attempts to assign a wide string literal to a non-wide string
    /// > constant or to assign a non-wide string literal to a wide string
    /// > constant result in a compile-time diagnostic.
    ///
    /// Without this the declared type was discarded and the literal's own
    /// type took its place: `const long K = "x";` reached the Rust
    /// backend as `pub const K: &'static str = "x";`, and the author
    /// heard about it from the Rust compiler at the use site rather than
    /// here.
    ///
    /// **Silent where it is not certain.** The check fires only when the
    /// declared type resolves here to a primitive, a `string`/`wstring`
    /// or a `fixed`, *and* the expression settles to one literal form. A
    /// named type, an alias, an enumeration, a reference to another
    /// constant and an unresolved reference all pass without a word: a
    /// rejection that reaches too far breaks valid IDL, which costs more
    /// than the defect it removes.
    fn check_const_value_form(
        &mut self,
        declared: &Ref<TypeNode>,
        expr: Id<ConstExprNode>,
        node: &Arc<SyntaxNode>,
    ) {
        let Some(complaint) = self.value_disagrees_with(declared, expr) else {
            return;
        };
        // §7.2.6.3 states the two string cases outright; the rest are
        // §7.4.1.4.3's "consistent with the type declared".
        let spec = if complaint.contains("string literal") {
            "OMG IDL 4.2 §7.2.6.3"
        } else {
            "OMG IDL 4.2 §7.4.1.4.3"
        };
        self.diagnose_at(
            DiagnosticCode::TypeMismatch,
            node.span(),
            &alloc::format!("the constant {complaint}"),
            spec,
        );
    }

    /// Whether `expr` is a value the type `declared` can hold, and if
    /// not, how to say so.
    ///
    /// `None` means "no complaint", which includes every case the answer
    /// is not settled in: an unresolved reference, a named type, an
    /// alias, an enumeration, a reference to another constant, or an
    /// expression that does not reduce to one literal form. A rejection
    /// that reaches too far breaks valid IDL, which costs more than the
    /// defect it removes.
    ///
    /// Two clauses ask this question and get the same answer here: IDL
    /// 4.2 §7.4.1.4.3 for a constant's value, and §7.4.15.4.2 for an
    /// annotation argument — "a constant expression, whose type must be
    /// compatible with the member's declaration". One reading, so the
    /// two cannot drift.
    pub(crate) fn value_disagrees_with(
        &self,
        declared: &Ref<TypeNode>,
        expr: Id<ConstExprNode>,
    ) -> Option<alloc::string::String> {
        let Ref::Resolved(id) = declared else {
            return None;
        };
        let kind = self.ast.type_(*id).ok()?.kind.clone();
        let form = self.value_form(expr, 0)?;
        let (admitted, wanted) = match &kind {
            TypeKind::Primitive(PrimitiveKind::Boolean) => {
                (form == ValueForm::Boolean, "a boolean")
            }
            TypeKind::Primitive(
                PrimitiveKind::Octet
                | PrimitiveKind::Int8
                | PrimitiveKind::UInt8
                | PrimitiveKind::Int16
                | PrimitiveKind::UInt16
                | PrimitiveKind::Int32
                | PrimitiveKind::UInt32
                | PrimitiveKind::Int64
                | PrimitiveKind::UInt64,
            ) => (form == ValueForm::Integer, "an integer"),
            TypeKind::Primitive(
                PrimitiveKind::Float32 | PrimitiveKind::Float64 | PrimitiveKind::Float128,
            ) => (
                matches!(form, ValueForm::Integer | ValueForm::Float),
                "a number",
            ),
            TypeKind::Fixed { .. } => (
                matches!(
                    form,
                    ValueForm::Integer | ValueForm::Float | ValueForm::Fixed
                ),
                "a number",
            ),
            TypeKind::Primitive(PrimitiveKind::Char) => (form == ValueForm::Char, "a character"),
            // A narrow character literal is a wide one written narrowly;
            // the reverse is not, because the code point may not fit.
            TypeKind::Primitive(PrimitiveKind::WChar) => (
                matches!(form, ValueForm::Char | ValueForm::WChar),
                "a character",
            ),
            TypeKind::String {
                width: CharWidth::Char,
                ..
            } => (form == ValueForm::String, "a string literal"),
            TypeKind::String {
                width: CharWidth::WChar,
                ..
            } => (form == ValueForm::WString, "a wide string literal"),
            // Everything else — an enumeration, a sequence, an aggregate
            // — is not settled here.
            _ => return None,
        };
        if admitted {
            return None;
        }
        Some(alloc::format!(
            "is declared as a type that takes {wanted}, and the value is {}; the declared \
             type is what it has, so the two have to agree",
            form.describe()
        ))
    }

    /// Which literal form a constant expression settles to, if one.
    ///
    /// `None` wherever the answer is not settled here: a reference to
    /// another constant, an enumerator, a mixed arithmetic expression, or
    /// a tree deeper than evaluation follows. `None` means "say nothing",
    /// never "assume".
    fn value_form(&self, expr: Id<ConstExprNode>, depth: usize) -> Option<ValueForm> {
        if depth > zerodds_ast::node::constant::MAX_EXPR_DEPTH {
            return None;
        }
        let node = self.ast.expr(expr).ok()?;
        match &node.expr {
            ConstExpr::Literal(literal) => match literal {
                Literal::Boolean(_) => Some(ValueForm::Boolean),
                Literal::Integer(_) => Some(ValueForm::Integer),
                Literal::Float { .. } => Some(ValueForm::Float),
                Literal::Fixed { .. } => Some(ValueForm::Fixed),
                Literal::Char(_) => Some(ValueForm::Char),
                Literal::WChar(_) => Some(ValueForm::WChar),
                Literal::String(_) => Some(ValueForm::String),
                Literal::WString(_) => Some(ValueForm::WString),
                // An enumerator names a type this does not resolve here.
                Literal::Enumerator(_) => None,
            },
            // The operand carries the form: `-1` is an integer, `~1` is
            // an integer, and neither operator changes that.
            ConstExpr::Unary { operand, .. } => self.value_form(*operand, depth + 1),
            // Only when both sides agree. `1 + K` where `K` is another
            // constant answers `None`, which is the honest answer at this
            // stage.
            ConstExpr::Binary { lhs, rhs, .. } => {
                let left = self.value_form(*lhs, depth + 1)?;
                let right = self.value_form(*rhs, depth + 1)?;
                if left == right {
                    Some(left)
                } else if left.is_numeric() && right.is_numeric() {
                    // `1 + 1.5` is floating; the wider form wins.
                    Some(if left == ValueForm::Fixed || right == ValueForm::Fixed {
                        ValueForm::Fixed
                    } else {
                        ValueForm::Float
                    })
                } else {
                    None
                }
            }
            ConstExpr::NameRef(_) => None,
        }
    }

    /// Give an expression node its source facet.
    pub(crate) fn record_expr_origin(&mut self, id: Id<zerodds_ast::ConstExprNode>, span: Span) {
        let node_id = self.ast.expr(id).map(|node| node.node_id).ok();
        if let Some(node_id) = node_id {
            self.record_origin(AnyId::ConstExpr(id, node_id), span);
        }
    }

    fn bind(&mut self, name: &str, id: Id<TypeNode>) {
        let node_id = self.node_id_of(id);
        self.scopes.bind(name, AnyId::Type(id, node_id));
    }

    fn node_id_of(&self, id: Id<TypeNode>) -> zerodds_ast_core::NodeId {
        self.ast.type_(id).map_or(
            zerodds_ast_core::NodeId::from_parts(zerodds_ast_core::IdSpace::SYNTHETIC, 0),
            |node| node.node_id,
        )
    }

    /// Facets naming where a node came from.
    fn facets_at(&self, span: Span) -> Facets {
        Facets {
            source: self.source_at(span),
            ..Facets::default()
        }
    }

    /// The source facet for a byte range of the file being lowered.
    ///
    /// The range is narrowed to the syntax inside it: a node's own span
    /// starts at its leading trivia, and a facet that kept it would put
    /// every consumer — the diagnostics of `complete.rs` and
    /// `validate.rs` among them — one blank line above the declaration.
    fn source_at(&self, span: Span) -> SourceFacet {
        let span = self.syntax_spans.trim(span);
        SourceFacet {
            origin: SourceOrigin::File {
                path: self.path.clone(),
            },
            span: Some(zerodds_ast::ByteSpan {
                start: span.start,
                end: span.end,
            }),
            macro_chain: Vec::new(),
            doc_comments: Vec::new(),
        }
    }

    fn record_origin(&mut self, any: AnyId, span: Span) {
        let source = self.source_at(span);
        if let Some(facets) = self.ast.facets_mut(any) {
            facets.source = source;
        }
    }

    /// Declare a named type in the current scope, or complete the
    /// forward declaration already standing there.
    ///
    /// The one place a named type enters the model, so the forward rule
    /// holds for every declaration form rather than for whichever ones
    /// remembered it. A definition following `struct S;` completes that
    /// node: references taken in between stay valid, which is why
    /// forwardness is a state and not a kind. Spec §3.2 of the AST.
    ///
    /// A genuine duplicate is reported and still declared, so the rest
    /// of the file is checked rather than abandoned at the first clash.
    /// Records the interface or value type a declaration sits in, when it
    /// sits in one: its scoped name and its repository id go through it
    /// (`ScopeStack::push_container`).
    fn record_declaration_scope(&mut self, any: AnyId, at: zerodds_idl_cst::Span) {
        let Some(scope) = self.scopes.enclosing_container().map(String::from) else {
            return;
        };
        let result = self.ast.set_declaration_scope(any, &scope);
        self.report_model(result, at);
    }

    fn declare_type(
        &mut self,
        name: &str,
        kind: TypeKind,
        at: &Arc<SyntaxNode>,
    ) -> Option<Id<TypeNode>> {
        if let Some(existing) = self.scopes.lookup_here(name).and_then(AnyId::as_type) {
            let forward = self
                .ast
                .type_(existing)
                .is_ok_and(|node| node.completeness == Completeness::Forward);
            if forward {
                return match self.ast.complete_type(existing, kind) {
                    Ok(()) => {
                        let any = AnyId::Type(existing, self.node_id_of(existing));
                        self.record_origin(any, at.span());
                        // The definition's own annotations. The fresh
                        // branch below applies them; this branch did
                        // not, so `@mutable struct Later { ... };`
                        // behind a `struct Later;` lost its
                        // extensibility and serialised under the
                        // default — invisibly wherever the default was
                        // `appendable` and it said `@appendable`,
                        // measured under Cyclone's `final` on
                        // `resolve-forward-declared`.
                        self.apply_waiting_annotations(any);
                        Some(existing)
                    }
                    Err(_) => None,
                };
            }
        }
        self.check_collision(name, at);
        let module = self.scopes.current_module()?;
        let result = self.ast.add_type(module, name, kind);
        let id = self.report_model(result, at.span())?;
        self.record_origin(AnyId::Type(id, self.node_id_of(id)), at.span());
        self.record_declaration_scope(AnyId::Type(id, self.node_id_of(id)), at.span());
        self.bind(name, id);
        self.apply_waiting_annotations(AnyId::Type(id, self.node_id_of(id)));
        Some(id)
    }

    /// Declare an interface in the current scope, or complete the
    /// forward declaration already standing there.
    ///
    /// The counterpart of [`Self::declare_type`], and it exists for the
    /// same reason: the forward rule has to hold for every declaration
    /// form rather than for whichever ones remembered it. `struct S;`
    /// and `union U;` had it and `interface I;` did not.
    fn declare_interface(
        &mut self,
        name: &str,
        kind: InterfaceKind,
        bases: Vec<Ref<zerodds_ast::InterfaceNode>>,
        at: &Arc<SyntaxNode>,
    ) -> Option<Id<zerodds_ast::InterfaceNode>> {
        if let Some(AnyId::Interface(existing, _)) = self.scopes.lookup_here(name) {
            let forward = self
                .ast
                .interface(existing)
                .is_ok_and(|node| node.completeness == Completeness::Forward);
            if forward {
                return match self.ast.complete_interface(existing, kind, bases) {
                    Ok(()) => {
                        // The definition's annotations, exactly as
                        // `declare_type`'s forward branch applies them.
                        if let Ok(node) = self.ast.interface(existing) {
                            let node_id = node.node_id;
                            self.apply_waiting_annotations(AnyId::Interface(existing, node_id));
                        }
                        Some(existing)
                    }
                    Err(_) => None,
                };
            }
        }
        self.check_collision(name, at);
        let module = self.scopes.current_module()?;
        let result = self.ast.add_interface(module, name, kind, bases);
        self.report_model(result, at.span())
    }

    /// Apply the annotations of the wrapper node this declaration sits
    /// in, and any written directly on the declaration itself.
    pub(crate) fn apply_waiting_annotations(&mut self, target: AnyId) {
        let Some(annotations) = self.pending_annotations.pop() else {
            return;
        };
        self.apply_annotations(&annotations, target);
        // Put them back: one wrapper may hold several declarators, and
        // each of them carries the same annotations.
        self.pending_annotations.push(annotations);
    }

    /// One namespace per scope, compared without regard to case.
    ///
    /// OMG IDL 4.2 §7.2.3.1: "There is only one namespace for IDL
    /// identifiers in each scope", and when two identifiers are compared
    /// to see whether they collide, "upper- and lower-case letters are
    /// treated as the same letter". The clause's own example is this
    /// shape —
    ///
    /// ```text
    /// module M {
    ///     typedef long Foo;
    ///     const long thing = 1;
    ///     interface thing { void doit (in Foo foo); };
    /// };
    /// ```
    ///
    /// — and the errors the specification lists for it include "`Foo`
    /// and `foo` collide", although `Foo` is declared in `M` and `foo`
    /// in the scope of `doit`. What binds the two is that the *simple*
    /// name `Foo` is **used** in the scope `foo` is **declared** in. An
    /// aggregate is that scope for its members, so `struct S { Inner
    /// inner; };` is the same collision.
    ///
    /// A type reached through a scoped name — `::common::First first;`
    /// — puts no simple identifier in the member's scope and does not
    /// collide. `tao_idl` and `opendds_idl`, the two front ends that
    /// answer this in the clause's own words, draw the line in the same
    /// place.
    ///
    /// Reported here rather than through `check_collision` because a
    /// member is not bound in `ScopeStack`: an aggregate is a scope for
    /// its member names and for nothing else, and binding members there
    /// would put them in the way of every later type lookup.
    ///
    /// Four scopes have this shape and all four are checked: an
    /// aggregate over its members, a union additionally over its
    /// discriminator, an interface over its attributes and operation
    /// names, and an operation over its parameters — which is the
    /// example's own scope.
    fn one_namespace_per_scope(&mut self, node: &Arc<SyntaxNode>) {
        // Every simple name the scope uses as a type, and every name it
        // declares, in one list — the clause has one namespace and so
        // has this. A name is kept with the span to report it at, which
        // is the declaration's: the use is legal on its own and only the
        // declaration can move.
        //
        // Track which body child (declaration) each name comes from, so
        // that under the ROS 2 dialect we can admit a collision between a
        // member/parameter name and the simple type name used *in that
        // same declaration* (e.g., `Pose pose;`), while still rejecting
        // cross-declaration collisions and declared/declared collisions.
        let mut used: Vec<UsedName> = Vec::new();
        let mut declared: Vec<DeclaredName> = Vec::new();

        // Which children hold the declarations of this scope, and which
        // an inner scope of their own. An operation's parameters are the
        // operation's scope and not the interface's, so an interface
        // stops at the operation's name and its return type.
        let (bodies, stop_at_inner_scope) = match node.kind() {
            CstKind::InterfaceDecl => (
                [CstKind::AttributeDecl, CstKind::Operation].as_slice(),
                true,
            ),
            CstKind::Operation => ([CstKind::Parameter].as_slice(), false),
            _ => ([CstKind::Member, CstKind::UnionCase].as_slice(), false),
        };

        let mut child_index = 0;
        for child in node.children() {
            let is_body = bodies.contains(&child.kind());
            // A type written directly on the declaration is used in this
            // scope: a union's discriminator and an operation's return
            // type are both written there.
            if !is_body && child.kind() != CstKind::NamedType {
                continue;
            }
            let inner_scope = stop_at_inner_scope && child.kind() == CstKind::Operation;
            if inner_scope {
                // Only the operation's own name and its return type
                // belong to the interface; the parameters do not.
                if let Some(name) = first_ident(&child) {
                    declared.push(DeclaredName {
                        name,
                        span: child.span(),
                        child_index,
                    });
                }
                for grandchild in child.children() {
                    if grandchild.kind() == CstKind::NamedType {
                        collect_simple_type_names_with_index(&grandchild, &mut used, child_index);
                    }
                }
                child_index += 1;
                continue;
            }
            collect_simple_type_names_with_index(&child, &mut used, child_index);
            if !is_body {
                child_index += 1;
                continue;
            }
            for declarator in child
                .descendants()
                .into_iter()
                .filter(|inner| inner.kind() == CstKind::Declarator)
            {
                if let Some(name) = first_ident(&declarator) {
                    declared.push(DeclaredName {
                        name,
                        span: declarator.span(),
                        child_index,
                    });
                }
            }
            // A parameter writes its name as a plain identifier rather
            // than through a declarator.
            if child.kind() == CstKind::Parameter
                && !child
                    .children()
                    .iter()
                    .any(|inner| inner.kind() == CstKind::Declarator)
            {
                if let Some(name) = first_ident(&child) {
                    declared.push(DeclaredName {
                        name,
                        span: child.span(),
                        child_index,
                    });
                }
            }
            child_index += 1;
        }

        for index in 0..declared.len() {
            let current = &declared[index];
            let name = &current.name;
            let span = current.span;
            let child_idx = current.child_index;

            // Check against used type names from other declarations
            let clash_used = used
                .iter()
                .filter(|u| u.child_index != child_idx)
                .map(|u| u.name.as_str())
                .find(|other| *other != name && other.eq_ignore_ascii_case(name));

            // Check against previously declared names (always an error)
            let clash_declared = declared[..index]
                .iter()
                .map(|d| d.name.as_str())
                .find(|other| *other != name && other.eq_ignore_ascii_case(name));

            // Check against used type names from the same declaration
            // Only relevant under ROS 2 dialect: admit this specific case
            let clash_same_decl = used
                .iter()
                .filter(|u| u.child_index == child_idx)
                .map(|u| u.name.as_str())
                .find(|other| *other != name && other.eq_ignore_ascii_case(name));

            if let Some(clash) = clash_declared {
                // Two declared names in the same scope differing only in case — always an error
                let message = alloc::format!(
                    "`{name}` and `{clash}` are in one scope and differ only in case; IDL has a \
                     single namespace per scope and compares identifiers without regard to case"
                );
                self.diagnose_at(
                    DiagnosticCode::DuplicateName,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.2.3.1",
                );
            } else if let Some(clash) = clash_used {
                // Declared name collides with a used type name from a different declaration — error
                let message = alloc::format!(
                    "`{name}` and `{clash}` are in one scope and differ only in case; IDL has a \
                     single namespace per scope and compares identifiers without regard to case. \
                     Rename the declaration, or write the type's scoped name"
                );
                self.diagnose_at(
                    DiagnosticCode::DuplicateName,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.2.3.1",
                );
            } else if let Some(clash) = clash_same_decl {
                // Declared name collides with a used type name from the same declaration
                // Admit under ROS 2 dialect, error otherwise
                if self
                    .options
                    .dialect
                    .admits(zerodds_idl_parse::VendorFlag::Ros2)
                {
                    // Record that this construct was admitted by the ROS 2 flag
                    // (no diagnostic emitted, but the construct is accepted)
                } else {
                    let message = alloc::format!(
                        "`{name}` and `{clash}` are in one scope and differ only in case; IDL has a \
                         single namespace per scope and compares identifiers without regard to case. \
                         Rename the declaration, or write the type's scoped name"
                    );
                    self.diagnose_at(
                        DiagnosticCode::DuplicateName,
                        span,
                        &message,
                        "OMG IDL 4.2 §7.2.3.1",
                    );
                }
            }
        }
    }

    /// Collision reporting, kept where the duplicate rule is stated.
    fn duplicate(&mut self, name: &str, at: &Arc<SyntaxNode>) {
        let message = alloc::format!(
            "`{name}` is already declared in this scope; IDL compares identifiers case-insensitively"
        );
        self.diagnose_at(
            DiagnosticCode::DuplicateName,
            at.span(),
            &message,
            "OMG IDL 4.2 §7.2.3",
        );
    }

    /// Turn a rejected model mutation into a diagnostic.
    ///
    /// The model refuses what would break a structural invariant, and
    /// the refusal is the only record of it: a lowering that discarded
    /// the `Err` would drop the declaration and say nothing, which is
    /// how `long a[0]` reached a model with no array in it and no
    /// complaint about one.
    pub(crate) fn report_model<T>(
        &mut self,
        result: Result<T, zerodds_ast::AstError>,
        at: Span,
    ) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                let (code, spec) = classify(&error);
                let message = self.explain(&error);
                self.diagnose_at(code, at, &message, spec);
                None
            }
        }
    }

    /// A rejected mutation, in the terms the source used.
    ///
    /// The model names nodes by identity, which is what it has; a
    /// diagnostic has to name them the way they were written.
    fn explain(&self, error: &zerodds_ast::AstError) -> String {
        use zerodds_ast::{AstError, InvariantViolation as V};
        let AstError::Invariant(violations) = error else {
            return alloc::format!("{error}");
        };
        match violations.first() {
            Some(V::DuplicateMemberId {
                id, first, second, ..
            }) => {
                let (first_name, first_origin) = self.member_id_source(*first);
                let (second_name, second_origin) = self.member_id_source(*second);
                member_id_collision(&second_name, second_origin, &first_name, first_origin, *id)
            }
            _ => alloc::format!("{error}"),
        }
    }

    /// A member's name and where its id came from.
    fn member_id_source(&self, node: zerodds_ast_core::NodeId) -> (String, &'static str) {
        let Some(any) = self.ast.resolve_node_id(node) else {
            return (String::from("a member"), "an unknown rule");
        };
        let name = match any {
            AnyId::Member(id, _) => self
                .ast
                .member(id)
                .map_or_else(|_| String::from("a member"), |m| m.name.clone()),
            _ => String::from("a member"),
        };
        let origin = self
            .ast
            .facets(any)
            .and_then(|facets| facets.xtypes.as_ref())
            .and_then(|x| x.member_id.as_ref())
            .map_or("an unknown rule", |assignment| assignment.origin.as_str());
        (name, origin)
    }

    /// Report a name that collides case-insensitively in the same scope.
    ///
    /// IDL is case-insensitive for collisions and case-preserving
    /// otherwise, so `Pose` and `pose` are the same name. Spec §6.
    fn check_collision(&mut self, name: &str, at: &Arc<SyntaxNode>) {
        if self.scopes.is_bound_here(name) {
            self.duplicate(name, at);
            return;
        }
        self.check_redefines_enclosing_scope(name, at);
    }

    /// A declaration may not reuse the name of the scope it is written
    /// in. IDL 4.2 §7.5.2.
    ///
    /// > The name of a module, structure, union, map, interface, value
    /// > type, event type, exception or home may not be redefined within
    /// > the immediate scope of the module, structure, union, map,
    /// > interface, value type, event type, exception or home.
    ///
    /// The specification gives it as an example with the comment
    /// `// Error`:
    ///
    /// ```text
    /// module M {
    ///   typedef short M;      // Error
    /// };
    /// ```
    ///
    /// **Immediate scope only**, which is what makes the rule safe to
    /// have: `module Ma { module Mb { typedef short Ma; }; };` is legal
    /// and stays legal, because `Ma` is not the name of the scope `Mb`.
    /// That distinction is the whole difference between this rule and
    /// one that would break valid IDL, and it is why only the innermost
    /// scope's name is compared.
    ///
    /// Case-insensitively, because §7.2.3 makes every identifier
    /// comparison in IDL case-insensitive and this is one.
    fn check_redefines_enclosing_scope(&mut self, name: &str, at: &Arc<SyntaxNode>) {
        let Some(scope) = self.scopes.innermost_name() else {
            return;
        };
        if !scope.eq_ignore_ascii_case(name) {
            return;
        }
        // The vendors split here, so the axis decides. Under
        // `NameOfTheEnclosingScope::Admitted` the declaration stands and
        // the model is the one Fast DDS and RTI Connext build — which is
        // the only way a probe reproduces the octets they wrote.
        if self.options.name_of_the_enclosing_scope == NameOfTheEnclosingScope::Admitted {
            return;
        }
        let message = alloc::format!(
            "`{name}` is the name of the scope it is declared in, and OMG IDL 4.2 §7.5.2 does \
             not admit redefining that name within its own immediate scope"
        );
        self.diagnose_at(
            DiagnosticCode::DuplicateName,
            at.span(),
            &message,
            "OMG IDL 4.2 §7.5.2",
        );
    }
}

fn broken_cause() -> zerodds_ast_core::BreakCause {
    zerodds_ast_core::BreakCause::NotFound {
        what: PendingRef::ScopedName(ScopedName::absolute(Vec::new())),
    }
}

/// A simple type name with the index of the body child it came from.
/// Used to distinguish same-declaration from cross-declaration collisions.
struct UsedName {
    name: String,
    child_index: usize,
}

/// A declared name with its span and the index of the body child it came from.
struct DeclaredName {
    name: String,
    span: Span,
    child_index: usize,
}

/// Like `collect_simple_type_names`, but records the child index.
fn collect_simple_type_names_with_index(
    node: &Arc<SyntaxNode>,
    into: &mut Vec<UsedName>,
    child_index: usize,
) {
    for named in node
        .descendants()
        .into_iter()
        .filter(|child| child.kind() == CstKind::NamedType)
    {
        let Some(scoped) = named
            .descendants()
            .into_iter()
            .find(|child| child.kind() == CstKind::ScopedName)
        else {
            continue;
        };
        let tokens = scoped.syntax_tokens();
        if tokens
            .iter()
            .any(|(_, token)| token.kind() == TokenKind::ColonColon)
        {
            continue;
        }
        let segments: Vec<&str> = tokens
            .iter()
            .filter(|(_, token)| token.kind() == TokenKind::Ident)
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()))
            .collect();
        if let [only] = segments.as_slice() {
            into.push(UsedName {
                name: (*only).to_string(),
                child_index,
            });
        }
    }
}

/// The first identifier token directly under a node.
/// The string literals in `tokens`, a run of adjacent pieces of one
/// width joined into one value.
///
/// §7.2.6.3: "Adjacent string literals are concatenated" (§7.2.6.4 for
/// wide ones). The parser keeps each piece as its own token; this joins
/// a run of them, each piece decoded on its own so no escape reaches
/// across a seam. Any other significant token between two literals —
/// a comma in a `context` clause — ends the run. A piece that does not
/// decode drops the whole run rather than leaving a shorter string.
pub(crate) fn joined_string_literals(
    tokens: impl IntoIterator<Item = (zerodds_idl_cst::Span, zerodds_idl_cst::GreenToken)>,
    kind: TokenKind,
) -> Vec<String> {
    let mut joined = Vec::new();
    let mut run: Option<Option<String>> = None;
    for (_, token) in tokens {
        if token.kind() == kind {
            let piece = match zerodds_idl_cst::literal_value(kind, token.text()) {
                zerodds_idl_cst::TokenValue::Text { value } => Some(value),
                _ => None,
            };
            run = Some(match (run.take(), piece) {
                (None, piece) => piece,
                (Some(Some(mut value)), Some(piece)) => {
                    value.push_str(&piece);
                    Some(value)
                }
                (Some(_), _) => None,
            });
        } else if let Some(finished) = run.take() {
            joined.extend(finished);
        }
    }
    if let Some(finished) = run {
        joined.extend(finished);
    }
    joined
}

fn first_ident(node: &Arc<SyntaxNode>) -> Option<String> {
    node.syntax_tokens()
        .iter()
        .find(|(_, token)| token.kind() == TokenKind::Ident)
        .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
}

/// Whether the declarator's name token carries the escape underscore —
/// the spelling `identifier_name` strips and `XTypesFacet::escaped`
/// keeps.
fn declarator_is_escaped(node: &Arc<SyntaxNode>) -> bool {
    node.syntax_tokens()
        .iter()
        .find(|(_, token)| token.kind() == TokenKind::Ident)
        .is_some_and(|(_, token)| token.text().starts_with('_'))
}

/// How deep a fixed-point constant expression is followed.
///
/// The same reason [`zerodds_ast::MAX_EXPR_DEPTH`] exists: a tree read
/// from somewhere else can be hostile, and a walk without a bound hangs
/// rather than reporting.
const MAX_FIXED_EXPR_DEPTH: usize = 256;

/// The largest number of digits a `fixed` may have. §7.4.1.4.3.
const MAX_FIXED_DIGITS: u8 = 31;

/// Whether a declaration's type is `fixed` written without parameters.
///
/// Rule (43) `<fixed_pt_const_type> ::= "fixed"` — the constant form.
fn bare_fixed(node: &Arc<SyntaxNode>) -> bool {
    node.children()
        .into_iter()
        .find(|child| child.kind() == CstKind::FixedType)
        .is_some_and(|fixed| !has_type_arguments(&fixed))
}

/// Whether a declaration wrote a body at all.
///
/// A `valuetype` with no `{` is rule (105)'s forward declaration.
fn has_body(node: &Arc<SyntaxNode>) -> bool {
    node.syntax_tokens()
        .iter()
        .any(|(_, token)| token.kind() == TokenKind::LBrace)
}

/// Whether a parameterised type wrote its `<...>` at all.
fn has_type_arguments(node: &Arc<SyntaxNode>) -> bool {
    node.children()
        .iter()
        .any(|child| child.kind() == CstKind::TypeArguments)
}

/// Table 7-11's result type for one binary operator.
///
/// In `u16` throughout, and every intermediate is bounded: both operands
/// have already been through [`cap_to_31_digits`], so the widest result
/// any row can reach is `31 + 31 + 1`. The conversion back reports
/// rather than clamps all the same — a number this arithmetic cannot
/// have produced would otherwise become a precision nobody wrote.
fn fixed_binary(op: BinaryOp, d1: u8, s1: u8, d2: u8, s2: u8) -> Result<(u8, u8), &'static str> {
    // §7.4.1.4.3's own reading of a fixed type: the digits include the
    // fractional ones, so a scale above them is not a `fixed` at all and
    // the row's `d - s` would have nothing to subtract from.
    if s1 > d1 || s2 > d2 {
        return Err("a `fixed` scale cannot exceed its digits");
    }
    let (d1, s1, d2, s2) = (u16::from(d1), u16::from(s1), u16::from(d2), u16::from(s2));
    let (digits, scale) = match op {
        // fixed<max(d1-s1,d2-s2)+max(s1,s2)+1, max(s1,s2)>
        BinaryOp::Add | BinaryOp::Sub => {
            let scale = s1.max(s2);
            ((d1 - s1).max(d2 - s2) + scale + 1, scale)
        }
        // fixed<d1+d2, s1+s2>
        BinaryOp::Mul => (d1 + d2, s1 + s2),
        // fixed<(d1-s1+s2)+sinf, sinf>, with `sinf` filling the result
        // to the 31 digits the type holds.
        BinaryOp::Div => {
            let whole = d1 - s1 + s2;
            let sinf = u16::from(MAX_FIXED_DIGITS).saturating_sub(whole);
            (whole + sinf, sinf)
        }
        BinaryOp::Or
        | BinaryOp::Xor
        | BinaryOp::And
        | BinaryOp::Shl
        | BinaryOp::Shr
        | BinaryOp::Mod => {
            return Err(
                "§7.4.1.4.3 admits unary `+` and `-` and binary `*`, `/`, `+` and `-` in a \
                 fixed-point expression, and nothing else",
            );
        }
    };
    let too_wide = "this fixed-point expression reaches a precision `fixed` cannot state";
    Ok(cap_to_31_digits(
        u8::try_from(digits).map_err(|_| too_wide)?,
        u8::try_from(scale).map_err(|_| too_wide)?,
    ))
}

/// §7.4.1.4.3's cap, applied after every computation.
///
/// "If an individual computation between a pair of fixed-point literals
/// actually generates more than 31 significant digits, then a 31-digit
/// result is retained as follows: `fixed<d,s> => fixed<31, 31-d+s>`."
///
/// `31 - d + s` goes negative when the integral part alone exceeds 31
/// digits. The clause does not say what then, and the only reading the
/// type admits is a scale of zero — a negative scale is not
/// representable and rounding is expressly forbidden, so the fractional
/// digits are the ones that go.
fn cap_to_31_digits(digits: u8, scale: u8) -> (u8, u8) {
    if digits <= MAX_FIXED_DIGITS {
        return (digits, scale);
    }
    let capped = i32::from(MAX_FIXED_DIGITS) - i32::from(digits) + i32::from(scale);
    (MAX_FIXED_DIGITS, u8::try_from(capped.max(0)).unwrap_or(0))
}

/// Whether a declaration was written `eventtype`. Rule (166).
fn is_event_type(node: &Arc<SyntaxNode>) -> bool {
    node.syntax_tokens()
        .iter()
        .any(|(_, token)| token.kind() == TokenKind::Eventtype)
}

/// Whether a member declaration opened with `public` or `private`.
///
/// Only a valuetype state member carries one; a struct member has no
/// visibility keyword and the answer is `None`. IDL 4.2 §7.4.4,
/// rule (106).
fn state_visibility(node: &Arc<SyntaxNode>) -> Option<zerodds_ast::StateVisibility> {
    node.syntax_tokens()
        .iter()
        .find_map(|(_, token)| match token.kind() {
            TokenKind::Public => Some(zerodds_ast::StateVisibility::Public),
            TokenKind::Private => Some(zerodds_ast::StateVisibility::Private),
            _ => None,
        })
}

/// The built-in type names IDL provides that are not primitives.
///
/// `<base_type_spec>` reaches these through their own rules rather than
/// through `<primitive_type>`, and each is one keyword: `any` is
/// §7.4.2's rules (69) and (70), `Object` §7.4.6's (117) and (118),
/// `ValueBase` §7.4.7's (131) and (132).
/// They are named types in the model exactly as `long` is, so a member
/// declared with one carries a resolved reference and not a broken one.
fn builtin_of(kinds: &[TokenKind]) -> Option<(&'static str, TypeKind)> {
    match kinds {
        [TokenKind::Any] => Some(("any", TypeKind::Any)),
        // Rule (117) `<object_type> ::= "Object"` names no interface,
        // which is what the absent reference says.
        [TokenKind::Object] => Some(("Object", TypeKind::ObjectRef { interface: None })),
        // (131) `<value_base_type> ::= "ValueBase"`, the abstract base
        // every valuetype has, written as a type.
        [TokenKind::Valuebase] => Some(("ValueBase", TypeKind::ValueBase)),
        _ => None,
    }
}

/// Which primitive a run of type keywords spells.
fn primitive_of(kinds: &[TokenKind]) -> Option<PrimitiveKind> {
    Some(match kinds {
        [TokenKind::Boolean] => PrimitiveKind::Boolean,
        [TokenKind::Octet] => PrimitiveKind::Octet,
        [TokenKind::Char] => PrimitiveKind::Char,
        [TokenKind::Wchar] => PrimitiveKind::WChar,
        [TokenKind::Short] => PrimitiveKind::Int16,
        [TokenKind::Long] => PrimitiveKind::Int32,
        [TokenKind::Long, TokenKind::Long] => PrimitiveKind::Int64,
        [TokenKind::Long, TokenKind::Double] => PrimitiveKind::Float128,
        [TokenKind::Unsigned, TokenKind::Short] => PrimitiveKind::UInt16,
        [TokenKind::Unsigned, TokenKind::Long] => PrimitiveKind::UInt32,
        [TokenKind::Unsigned, TokenKind::Long, TokenKind::Long] => PrimitiveKind::UInt64,
        [TokenKind::Float] => PrimitiveKind::Float32,
        [TokenKind::Double] => PrimitiveKind::Float64,
        [TokenKind::Int8] => PrimitiveKind::Int8,
        [TokenKind::Uint8] => PrimitiveKind::UInt8,
        [TokenKind::Int16] => PrimitiveKind::Int16,
        [TokenKind::Uint16] => PrimitiveKind::UInt16,
        [TokenKind::Int32] => PrimitiveKind::Int32,
        [TokenKind::Uint32] => PrimitiveKind::UInt32,
        [TokenKind::Int64] => PrimitiveKind::Int64,
        [TokenKind::Uint64] => PrimitiveKind::UInt64,
        _ => return None,
    })
}

/// Whether a scoped name was written anchored at the global scope.
///
/// Rule (4) `<scoped_name> ::+ "::" <identifier>`: the leading `::` says
/// the root and nothing else, so a name that carries it is not looked up
/// outwards from where it was written.
fn is_root_anchored(node: &Arc<SyntaxNode>) -> bool {
    let scoped = if node.kind() == CstKind::ScopedName {
        Some(node.clone())
    } else {
        node.descendants()
            .into_iter()
            .find(|child| child.kind() == CstKind::ScopedName)
    };
    scoped.is_some_and(|scoped| {
        scoped
            .syntax_tokens()
            .first()
            .is_some_and(|(_, token)| token.kind() == TokenKind::ColonColon)
    })
}

/// Which interface a forward declaration forwards, if it is one.
///
/// `None` for `struct S;` and `union U;`, whose forward declaration is a
/// type. Rule (75) `<interface_forward_dcl> ::= <interface_header>` —
/// and the header carries the `local` or `abstract` modifier, so the
/// forward and its definition agree on the form before the body arrives.
///
/// (135) `<component_forward_dcl> ::= "component" <identifier>` is the
/// same rule one building block on, and the four component-model
/// declarations reach the model as interfaces of their own kind. They
/// are listed here rather than left to the type path for the reason
/// (75) is: a `component` forward completed by a `component` definition
/// has to be one node.
fn forward_interface_kind(node: &Arc<SyntaxNode>) -> Option<InterfaceKind> {
    let mut modifier = None;
    for (_, token) in node.syntax_tokens() {
        match token.kind() {
            TokenKind::Local => modifier = Some(InterfaceKind::Local),
            TokenKind::Abstract => modifier = Some(InterfaceKind::Abstract),
            TokenKind::Interface => {
                return Some(modifier.unwrap_or(InterfaceKind::Interface));
            }
            TokenKind::Component => return Some(InterfaceKind::Component),
            TokenKind::Home => return Some(InterfaceKind::Home),
            TokenKind::Connector => return Some(InterfaceKind::Connector),
            TokenKind::Porttype => return Some(InterfaceKind::PortType),
            // The name ends the header; a modifier with no keyword after
            // it is not an interface forward.
            TokenKind::Ident => return None,
            _ => {}
        }
    }
    None
}

/// Which interface form a declaration wrote. IDL 4.2 §7.4.3.
fn interface_kind(node: &Arc<SyntaxNode>) -> InterfaceKind {
    for (_, token) in node.syntax_tokens() {
        match token.kind() {
            TokenKind::Local => return InterfaceKind::Local,
            TokenKind::Abstract => return InterfaceKind::Abstract,
            TokenKind::Component => return InterfaceKind::Component,
            TokenKind::Home => return InterfaceKind::Home,
            TokenKind::Connector => return InterfaceKind::Connector,
            TokenKind::Porttype => return InterfaceKind::PortType,
            // The modifier precedes `interface`, so the keyword itself
            // ends the search rather than continuing into the body.
            TokenKind::Interface => break,
            _ => {}
        }
    }
    InterfaceKind::Interface
}

/// Which valuetype form a declaration wrote. IDL 4.2 §7.4.4.
fn valuetype_kind(node: &Arc<SyntaxNode>) -> ValuetypeKind {
    let mut kind = ValuetypeKind::Regular;
    for (_, token) in node.syntax_tokens() {
        match token.kind() {
            TokenKind::Custom => kind = ValuetypeKind::Custom,
            TokenKind::Abstract => kind = ValuetypeKind::Abstract,
            TokenKind::LBrace => break,
            _ => {}
        }
    }
    if kind != ValuetypeKind::Regular {
        return kind;
    }
    // `truncatable` is written on the **inheritance** clause — rule
    // (109) `<value_inheritance_spec> ::= [ ":" [ "truncatable" ]
    // <value_name> …` — and not beside the `valuetype` keyword. The
    // loop above reads the declaration's own tokens, and the keyword is
    // a token of the clause's node, so it was never seen: every
    // `valuetype D : truncatable B` came back `Regular`, and the
    // §15.3.4.3 base-identifier list a peer needs in order to truncate
    // was not emitted. The three are mutually exclusive in valid IDL —
    // CORBA 3.3 part 1 §5.2.5 forbids a truncatable `custom` valuetype
    // and an `abstract` one has no state to truncate — so one field
    // holds the answer.
    let truncatable = node
        .children()
        .into_iter()
        .filter(|child| child.kind() == CstKind::InheritanceClause)
        .any(|clause| {
            clause
                .syntax_tokens()
                .iter()
                .any(|(_, token)| token.kind() == TokenKind::Truncatable)
        });
    if truncatable {
        ValuetypeKind::Truncatable
    } else {
        ValuetypeKind::Regular
    }
}

/// The diagnostic a rejected mutation gets.
///
/// One arm per violation the model can raise, with no catch-all: a new
/// invariant is a compile error here rather than a diagnostic that says
/// only "unexpected token".
fn classify(error: &zerodds_ast::AstError) -> (DiagnosticCode, &'static str) {
    use zerodds_ast::{AstError, InvariantViolation as V};
    let AstError::Invariant(violations) = error else {
        return (DiagnosticCode::UnexpectedToken, "zerodds-ast-1.0 §8.1");
    };
    match violations.first() {
        Some(V::NonPositiveArrayDimension { .. }) => {
            (DiagnosticCode::InvalidBound, "OMG IDL 4.2 §7.4.1.4.4")
        }
        Some(V::IncompatibleCaseLabel { .. }) => {
            (DiagnosticCode::InvalidCaseLabel, "OMG IDL 4.2 §7.4.1.4.4.3")
        }
        Some(
            V::DuplicateMemberName { .. }
            | V::DuplicateEnumeratorName { .. }
            | V::NameCollidesOnlyInCase { .. },
        ) => (DiagnosticCode::DuplicateName, "OMG IDL 4.2 §7.2.3"),
        Some(V::DuplicateMemberId { .. }) => (
            DiagnosticCode::DuplicateMemberId,
            "DDS-XTypes 1.3 §7.3.1.2.1.1",
        ),
        Some(V::MultipleDefaultCases { .. }) => {
            (DiagnosticCode::InvalidCaseLabel, "OMG IDL 4.2 §7.4.1.4.4.3")
        }
        Some(V::DanglingReference { .. }) | None => {
            (DiagnosticCode::UnexpectedToken, "zerodds-ast-1.0 §8.1")
        }
    }
}

/// Drop diagnostics that repeat one already reported.
///
/// Compared by code and message rather than by byte range: the same
/// fact reported from two places points at two different parts of it.
fn dedup(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen: Vec<(DiagnosticCode, String)> = Vec::new();
    let mut out = Vec::new();
    for diagnostic in diagnostics {
        let key = (diagnostic.code, diagnostic.message.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        out.push(diagnostic);
    }
    out
}

/// How a member-id collision reads.
///
/// One wording, whether the collision surfaced as a rejected commit or
/// as a validation pass over the finished model: two spellings of one
/// fact would read as two problems.
pub(crate) fn member_id_collision(
    name: &str,
    origin: &str,
    other: &str,
    other_origin: &str,
    id: u32,
) -> String {
    alloc::format!(
        "`{name}` and `{other}` both have member id {id}: {name} from {origin}, {other} from {other_origin}"
    )
}

/// Which kind of entity a template parameter stands for.
/// What an instantiation supplies for one formal parameter.
enum Actual {
    /// A type specification or constant expression written at the
    /// instantiation — rule (192).
    Written(Arc<SyntaxNode>),
    /// A binding handed on by an enclosing instantiation, which is what
    /// rule (193)'s `alias` does.
    Bound(AnyId),
}

/// The formal parameters of a stored template body, in order.
///
/// Each is its name and whether it is the `const` alternative, which is
/// the only one whose actual is a value rather than a type.
fn template_formals(body: &Arc<SyntaxNode>) -> Vec<(String, bool)> {
    body.children()
        .into_iter()
        .find(|child| child.kind() == CstKind::TemplateParameters)
        .map_or_else(Vec::new, |list| {
            list.children()
                .into_iter()
                .filter(|child| child.kind() == CstKind::TemplateParameter)
                .filter_map(|parameter| {
                    let name = first_ident(&parameter)?;
                    let is_const =
                        template_param_kind(&parameter) == zerodds_ast::TemplateParamKind::Const;
                    Some((name, is_const))
                })
                .collect()
        })
}

fn template_param_kind(node: &Arc<SyntaxNode>) -> zerodds_ast::TemplateParamKind {
    use zerodds_ast::TemplateParamKind as K;
    // (188)'s `<sequence_type>` alternative writes the keyword inside a
    // type specification of its own, so it is not one of this node's own
    // tokens the way the ten classifiers are.
    if node
        .children()
        .iter()
        .any(|child| child.kind() == CstKind::SequenceType)
    {
        return K::Sequence;
    }
    for (_, token) in node.syntax_tokens() {
        match token.kind() {
            TokenKind::Typename => return K::TypeName,
            TokenKind::Const => return K::Const,
            TokenKind::Sequence => return K::Sequence,
            TokenKind::Interface => return K::Interface,
            TokenKind::Valuetype => return K::ValueType,
            TokenKind::Eventtype => return K::EventType,
            TokenKind::Struct => return K::Struct,
            TokenKind::Union => return K::Union,
            TokenKind::Exception => return K::Exception,
            TokenKind::Enum => return K::Enum,
            _ => {}
        }
    }
    K::TypeName
}

/// The type an event port carries, as a reference.
fn type_ref_of_port(ast: &Ast, attribute: Id<zerodds_ast::AttributeNode>) -> Ref<TypeNode> {
    ast.attribute(attribute)
        .map_or_else(|_| Ref::Broken(broken_cause()), |node| node.type_.clone())
}

/// The content of a quoted pragma argument, or the word itself.
fn unquote(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or(trimmed)
            .to_string(),
    )
}

/// The version number a `#pragma dds_xtopics` declaration named, in the
/// one spelling DDS-XTypes 1.3 §7.3.1.1.2 compares.
///
/// "It shall be interpreted without respect to case, and any spaces (for
/// example, in `1.0 Beta 1`) shall be replaced with underscores." Both
/// halves are needed to compare a pair: the words arrive here already
/// split on the spaces the clause replaces, so joining them with
/// underscores is that replacement, and the case is folded so that
/// `1.0_beta_1` and `1.0_Beta_1` are the one version they name.
///
/// An absent version number is the empty string, which the clause makes
/// a legal declaration — the number is optional in both forms — and
/// which therefore compares against nothing.
fn xtopics_version(words: &[&str]) -> String {
    words.join("_").to_ascii_lowercase()
}

/// The scoped name a pragma argument names.
fn pragma_name(written: &str) -> ScopedName {
    ScopedName::absolute(
        written
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(ToString::to_string)
            .collect(),
    )
}
