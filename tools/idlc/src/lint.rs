// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The rules a file can break while still being valid IDL.
//!
//! `check` answers "is this IDL". These answer "is this IDL you want to
//! put on a wire", and every one of them is a defect somebody has
//! actually shipped. They are separate from the front end's diagnostics
//! on purpose: a rule here is a judgement about a choice, and the
//! specification permits every one of the shapes below.
//!
//! Each rule states what it costs, because a warning that does not say
//! what it saves gets suppressed and never read again.
//!
//! ## What is deliberately not here
//!
//! Two rules were written and removed, because `check` already reports
//! both as **errors**, with a span, from the front end: two members that
//! resolve to one id (`IDL0016`) and a member that is both `@key` and
//! `@optional` (`IDL0013`). A lint rule that repeats a diagnostic the
//! stage before it already made is worse than none — it fires second,
//! without a position, and teaches a reader that the two tools disagree
//! about severity when they do not.

use std::collections::HashSet;

use zerodds_ast::{AnyId, Ast, Bound, Extensibility, TypeKind};
use zerodds_ast_core::Id;
use zerodds_idl_cst::Span;
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, Severity};

/// A `@mutable` aggregate above this many members is reported.
///
/// Each member of a `@mutable` type carries an EMHEADER on the wire —
/// four octets for the short form, twelve where the length code needs
/// the long one (XTypes 1.3 §7.4.3.4.2). At sixteen members that is at
/// least sixty-four octets of framing before any value, which is the
/// point where the extensibility is worth a second thought rather than a
/// default.
const MUTABLE_MEMBER_BUDGET: usize = 16;

/// Run every rule over `ast`.
#[must_use]
pub fn run(ast: &Ast) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        mutable_framing_cost(ast, id, node, &mut out);
        naming(node, &mut out);
        topic_missing_key(ast, id, node, &mut out);
        unbounded_collections(ast, id, node, &mut out);
        unbounded_keys(ast, id, node, &mut out);
    }
    unused_typedefs(ast, &mut out);
    out
}

/// How deep a type is followed through aliases, arrays and collections.
///
/// A chain this long is not written by hand, and an actual cycle is
/// invalid IDL that `check` reports before `lint` runs — the rules here
/// only see a model whose parse and resolve were clean.
const MAX_TYPE_DEPTH: usize = 64;

/// Where a rule points when the model has no span for the thing it is
/// about.
///
/// The model is built from a CST that has spans, but a node the lowering
/// synthesised has none of its own. Pointing at the start of the unit is
/// honest — the alternative is pointing at an unrelated declaration,
/// which sends a reader to the wrong line.
const NO_SPAN: Span = Span { start: 0, end: 0 };

/// Where a node was written: the file its provenance names and the
/// range in that file, or [`NO_SPAN`] in the unit where it has none. The
/// composer gives every node its file's offsets, an included file's as
/// well, so the finding names the place without being moved afterwards.
fn span_of(facets: &zerodds_ast::Facets) -> (Span, Option<String>) {
    let file = match &facets.source.origin {
        zerodds_ast::SourceOrigin::File { path } => Some(path.clone()),
        _ => None,
    };
    let span = facets
        .source
        .span
        .map_or(NO_SPAN, |s| Span::new(s.start, s.end));
    (span, file)
}

fn warn(
    code: DiagnosticCode,
    (at, file): (Span, Option<String>),
    message: String,
    spec: &'static str,
) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        code,
        message,
        primary: at,
        labels: Vec::new(),
        file,
        spec_reference: spec,
    }
}

/// `@mutable` on a wide aggregate: every member pays for the
/// extensibility, whether or not it will ever change.
fn mutable_framing_cost(
    ast: &Ast,
    id: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    node: &zerodds_ast::TypeNode,
    out: &mut Vec<Diagnostic>,
) {
    if !matches!(node.kind, TypeKind::Structure { .. }) {
        return;
    }
    if ast.effective_extensibility(id) != Extensibility::Mutable {
        return;
    }
    let count = ast.all_members_of(id).len();
    if count <= MUTABLE_MEMBER_BUDGET {
        return;
    }
    out.push(warn(
        DiagnosticCode::PolicyAdvice,
        span_of(&node.facets),
        format!(
            "`{}` is @mutable with {count} members; every one of them carries an EMHEADER on the \
             wire, at least four octets each and twelve where the length code needs the long \
             form. @appendable adds members at the end for one delimiter over the whole sample",
            node.name
        ),
        "XTypes 1.3 §7.4.3.4.2",
    ));
}

/// A declared name that will collide once it reaches a language binding.
///
/// IDL admits it; C, C++, Java, Rust and every other target this tree
/// generates for do not, and the collision surfaces as a compiler error
/// in generated code rather than here. The list is the union of what
/// those languages reserve, kept short: a name in it is worth renaming
/// whatever the target.
const RESERVED_ELSEWHERE: &[&str] = &[
    "auto",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "final",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "import",
    "inline",
    "int",
    "let",
    "match",
    "mut",
    "namespace",
    "new",
    "operator",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "switch",
    "template",
    "this",
    "throw",
    "trait",
    "true",
    "try",
    "type",
    "typeof",
    "union",
    "unsafe",
    "use",
    "virtual",
    "void",
    "where",
    "while",
    "yield",
];

fn naming(node: &zerodds_ast::TypeNode, out: &mut Vec<Diagnostic>) {
    // Only names somebody wrote. `ast.types()` also holds the built-in
    // types and the anonymous ones the lowering makes for a `sequence<T>`
    // or a bounded string, and `float` is one of those: reporting it told
    // the reader to rename a name they had not chosen, which is the shape
    // that gets a whole rule suppressed.
    if !matches!(
        node.kind,
        TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::Alias { .. }
    ) {
        return;
    }
    let lowered = node.name.to_lowercase();
    if RESERVED_ELSEWHERE.contains(&lowered.as_str()) {
        out.push(warn(
            DiagnosticCode::PolicyAdvice,
            span_of(&node.facets),
            format!(
                "`{}` is a keyword in at least one language this generates for; the collision \
                 shows up as a compiler error in generated code and not here",
                node.name
            ),
            "",
        ));
    }
}

/// `topic-missing-key`: a `@topic` type in which nothing is `@key`.
///
/// What the repository understands as a topic type is the `@topic`
/// annotation ([`zerodds_ast::TopicAnnotation`], DDS-XTypes 1.3
/// Annex B); what it understands as the key is `@key` on a member
/// ([`zerodds_ast::Facets::is_key`]). A topic type without any key is
/// legal DDS — it is a topic with exactly one instance. What it costs:
/// every sample updates that one instance, KEEP_LAST history keeps a
/// depth per *instance* and therefore one window for the whole topic,
/// and no reader can address entities apart. Where that is the intent
/// it costs nothing; where instances were intended, the identifying
/// members are missing their `@key`.
fn topic_missing_key(
    ast: &Ast,
    id: Id<zerodds_ast::TypeNode>,
    node: &zerodds_ast::TypeNode,
    out: &mut Vec<Diagnostic>,
) {
    if !matches!(node.kind, TypeKind::Structure { .. }) {
        return;
    }
    if node
        .facets
        .xtypes
        .as_ref()
        .is_none_or(|x| x.topic.is_none())
    {
        return;
    }
    let has_key = ast
        .all_members_of(id)
        .iter()
        .any(|m| ast.member(*m).is_ok_and(|n| n.facets.is_key()));
    if has_key {
        return;
    }
    out.push(warn(
        DiagnosticCode::PolicyAdvice,
        span_of(&node.facets),
        format!(
            "`{}` is declared @topic and none of its members is @key: the topic has a single \
             instance, every write updates it, and a KEEP_LAST history holds one window for the \
             whole topic instead of one per key. If instances are intended, the identifying \
             members need @key",
            node.name
        ),
        "DDS-XTypes 1.3 Annex B",
    ));
}

/// The type behind a chain of `typedef`s.
///
/// An alias is a name for its target, not a container around it, so the
/// rules judge what the name stands for.
fn strip_aliases(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Id<zerodds_ast::TypeNode> {
    let mut current = id;
    for _ in 0..MAX_TYPE_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return current;
        };
        let TypeKind::Alias { aliased } = &node.kind else {
            return current;
        };
        let Some(next) = aliased.resolved() else {
            return current;
        };
        current = next;
    }
    current
}

/// The unbounded sequence or map a type reaches without crossing a
/// named aggregate.
///
/// Aliases, array dimensions and the elements of bounded collections
/// are followed; a named struct is not, because its own members get
/// their own report where they are declared. Strings are deliberately
/// not this rule's business — see [`unbounded_collections`].
///
/// zerodds-lint: recursion-depth 64 (alias, array-element and bounded
/// collection-element nesting; bounded by [`MAX_TYPE_DEPTH`]). A named
/// aggregate is a leaf of this walk, so a recursive IDL type cannot
/// cycle here.
fn unbounded_collection_in(
    ast: &Ast,
    id: Id<zerodds_ast::TypeNode>,
    depth: usize,
) -> Option<&'static str> {
    if depth >= MAX_TYPE_DEPTH {
        return None;
    }
    let node = ast.type_(id).ok()?;
    let follow = |target: Option<Id<zerodds_ast::TypeNode>>| {
        target.and_then(|t| unbounded_collection_in(ast, t, depth + 1))
    };
    match &node.kind {
        TypeKind::Alias { aliased } => follow(aliased.resolved()),
        TypeKind::Array { element, .. } => follow(element.resolved()),
        TypeKind::Sequence { element, bound } => {
            if *bound == Bound::Unbounded {
                Some("sequence")
            } else {
                follow(element.resolved())
            }
        }
        TypeKind::Map { key, value, bound } => {
            if *bound == Bound::Unbounded {
                Some("map")
            } else {
                follow(key.resolved()).or_else(|| follow(value.resolved()))
            }
        }
        _ => None,
    }
}

/// `unbounded-collection`: a member whose wire size has no upper bound.
///
/// The specification permits `sequence<T>` and `map<K, V>` without a
/// bound. What it costs: the serialized size is whatever the writer
/// sends, the reader sizes its allocation from the received length, and
/// a profile that preallocates — every embedded and every zero-copy
/// path — cannot carry the type at all. `sequence<T, N>` and
/// `map<K, V, N>` make the worst case a number.
///
/// Unbounded *strings* are deliberately not reported here: `string`
/// without a bound is the idiomatic spelling in most IDL ever written,
/// and a rule that fires on most files teaches its readers to suppress
/// it — which is worse than not having the rule. A string inside a
/// *key* is different, and [`unbounded_keys`] does report it.
fn unbounded_collections(
    ast: &Ast,
    id: Id<zerodds_ast::TypeNode>,
    node: &zerodds_ast::TypeNode,
    out: &mut Vec<Diagnostic>,
) {
    let mut member_ids: Vec<Id<zerodds_ast::MemberNode>> = node.kind.members().to_vec();
    if matches!(node.kind, TypeKind::Union { .. }) {
        for case in ast.cases_of(id) {
            if let Ok(case_node) = ast.case(*case) {
                member_ids.push(case_node.member);
            }
        }
    }
    for member_id in member_ids {
        let Ok(member) = ast.member(member_id) else {
            continue;
        };
        let Some(target) = member.type_.resolved() else {
            continue;
        };
        let stripped = strip_aliases(ast, target);
        let direct = matches!(
            ast.type_(stripped).map(|n| &n.kind),
            Ok(TypeKind::Sequence {
                bound: Bound::Unbounded,
                ..
            } | TypeKind::Map {
                bound: Bound::Unbounded,
                ..
            })
        );
        let Some(what) = unbounded_collection_in(ast, stripped, 0) else {
            continue;
        };
        let verb = if direct { "is" } else { "contains" };
        out.push(warn(
            DiagnosticCode::PolicyAdvice,
            span_of(&member.facets),
            format!(
                "`{}.{}` {verb} an unbounded {what}: its wire size is whatever the writer sends, \
                 the reader allocates to match, and a preallocating profile cannot carry the \
                 type. A bound makes the worst case a number",
                node.name, member.name
            ),
            "",
        ));
    }
}

/// The unbounded string, sequence or map a *key* reaches.
///
/// Unlike [`unbounded_collection_in`] this follows named aggregates,
/// because a struct-typed `@key` member pulls that struct's key into
/// this type's key: its `@key` members where it has any, all of its
/// members where it has none (DDS-XTypes 1.3 §7.6.8). Strings count
/// here — the key is what gets serialized for every keyhash.
///
/// zerodds-lint: recursion-depth 64 (this one does follow aggregates,
/// so a recursive keyed type reaches itself; `seen` admits every type
/// once and [`MAX_TYPE_DEPTH`] caps how many it admits at all)
fn unbounded_in_key(
    ast: &Ast,
    id: Id<zerodds_ast::TypeNode>,
    seen: &mut Vec<Id<zerodds_ast::TypeNode>>,
) -> Option<&'static str> {
    if seen.contains(&id) || seen.len() >= MAX_TYPE_DEPTH {
        return None;
    }
    seen.push(id);
    let node = ast.type_(id).ok()?;
    let follow = |target: Option<Id<zerodds_ast::TypeNode>>,
                  seen: &mut Vec<Id<zerodds_ast::TypeNode>>| {
        target.and_then(|t| unbounded_in_key(ast, t, seen))
    };
    match &node.kind {
        TypeKind::String { bound, .. } => (*bound == Bound::Unbounded).then_some("string"),
        TypeKind::Sequence { element, bound } => {
            if *bound == Bound::Unbounded {
                Some("sequence")
            } else {
                follow(element.resolved(), seen)
            }
        }
        TypeKind::Map { key, value, bound } => {
            if *bound == Bound::Unbounded {
                Some("map")
            } else {
                follow(key.resolved(), seen).or_else(|| follow(value.resolved(), seen))
            }
        }
        TypeKind::Alias { aliased } => follow(aliased.resolved(), seen),
        TypeKind::Array { element, .. } => follow(element.resolved(), seen),
        TypeKind::Structure { .. } => {
            let members = ast.all_members_of(id);
            let keyed: Vec<_> = members
                .iter()
                .filter(|m| ast.member(**m).is_ok_and(|n| n.facets.is_key()))
                .copied()
                .collect();
            let closure = if keyed.is_empty() { members } else { keyed };
            for member_id in closure {
                let Ok(member) = ast.member(member_id) else {
                    continue;
                };
                if let Some(hit) = follow(member.type_.resolved(), seen) {
                    return Some(hit);
                }
            }
            None
        }
        // A union's key is its discriminator, which is integral, enum
        // or boolean and therefore bounded.
        _ => None,
    }
}

/// `unbounded-key`: a `@key` member whose serialized key has no maximum.
///
/// The specification permits it. What it costs: the key's serialized
/// form has no upper bound, so its keyhash always takes the digest path
/// instead of fitting the sixteen raw octets, every instance lookup
/// serializes an unbounded value, and a reader that budgets memory per
/// instance has no budget to compute. Keys are compared on every
/// sample; they are the one place a bound always pays for itself.
fn unbounded_keys(
    ast: &Ast,
    _id: Id<zerodds_ast::TypeNode>,
    node: &zerodds_ast::TypeNode,
    out: &mut Vec<Diagnostic>,
) {
    for member_id in node.kind.members() {
        let Ok(member) = ast.member(*member_id) else {
            continue;
        };
        if !member.facets.is_key() {
            continue;
        }
        let Some(target) = member.type_.resolved() else {
            continue;
        };
        let mut seen = Vec::new();
        let Some(what) = unbounded_in_key(ast, target, &mut seen) else {
            continue;
        };
        out.push(warn(
            DiagnosticCode::PolicyAdvice,
            span_of(&member.facets),
            format!(
                "`{}.{}` is @key and reaches an unbounded {what}: the serialized key has no \
                 maximum, so the keyhash always goes through the digest path and per-instance \
                 memory cannot be budgeted. A bound on the key member pays for itself on every \
                 sample",
                node.name, member.name
            ),
            "",
        ));
    }
}

/// Every type id something in the model points at.
fn used_type_ids(ast: &Ast) -> HashSet<Id<zerodds_ast::TypeNode>> {
    let mut used = HashSet::new();
    let mut mark = |target: Option<Id<zerodds_ast::TypeNode>>| {
        if let Some(id) = target {
            used.insert(id);
        }
    };
    for any in ast.all_nodes() {
        match any {
            AnyId::Type(id, _) => {
                if let Ok(node) = ast.type_(id) {
                    for reference in node.kind.type_refs() {
                        mark(reference.resolved());
                    }
                }
            }
            AnyId::Member(id, _) => {
                if let Ok(node) = ast.member(id) {
                    mark(node.type_.resolved());
                }
            }
            AnyId::Const(id, _) => {
                if let Ok(node) = ast.constant(id) {
                    mark(node.type_.resolved());
                }
            }
            AnyId::Param(id, _) => {
                if let Ok(node) = ast.param(id) {
                    mark(node.type_.resolved());
                }
            }
            AnyId::Attribute(id, _) => {
                if let Ok(node) = ast.attribute(id) {
                    mark(node.type_.resolved());
                }
            }
            AnyId::AnnotationMember(id, _) => {
                if let Ok(node) = ast.annotation_member(id) {
                    mark(node.type_.resolved());
                }
            }
            AnyId::Operation(id, _) => {
                if let Ok(node) = ast.operation(id) {
                    mark(node.return_type.as_ref().and_then(|r| r.resolved()));
                }
            }
            AnyId::Bitfield(id, _) => {
                if let Ok(node) = ast.bitfield(id) {
                    mark(node.holder_type.as_ref().and_then(|r| r.resolved()));
                }
            }
            _ => {}
        }
    }
    used
}

/// `unused-typedef`: a `typedef` nothing in the unit references.
///
/// Every generated binding still carries the alias, every reader
/// assumes the name matters, and a rename of the target later leaves it
/// behind pointing at the old spelling. A typedef kept as public
/// vocabulary for files that `#include` this one is a choice — this
/// lint runs over the unit as composed, so such a name is reported
/// against the file that carries it and the choice is made knowingly.
///
/// A chain `typedef A B;` where only `B` is unused reports `B` alone:
/// `A` is referenced — by `B`. Removing `B` makes the next run report
/// `A`, which is the honest order to dismantle a chain in.
fn unused_typedefs(ast: &Ast, out: &mut Vec<Diagnostic>) {
    let used = used_type_ids(ast);
    for (id, node) in ast.types() {
        if !matches!(node.kind, TypeKind::Alias { .. })
            || node.name.is_empty()
            || !node.facets.source.is_from_file()
            || used.contains(&id)
        {
            continue;
        }
        out.push(warn(
            DiagnosticCode::PolicyAdvice,
            span_of(&node.facets),
            format!(
                "typedef `{}` is referenced by nothing in this unit: every generated binding \
                 still carries it and a reader assumes it matters. Keep it knowingly as public \
                 vocabulary, or remove it",
                node.name
            ),
            "",
        ));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn model(source: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(source);
        assert!(
            !parsed.has_errors(),
            "the fixture does not parse: {:?}",
            parsed.diagnostics()
        );
        zerodds_idl_lower::lower(&parsed.syntax(), "test.idl").ast
    }

    fn messages(source: &str) -> Vec<String> {
        run(&model(source)).into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn a_clean_file_reports_nothing() {
        let got = messages("module m { @appendable struct S { @key long id; string name; }; };");
        assert!(got.is_empty(), "{got:?}");
    }

    /// Seventeen members is one over the budget, sixteen is not.
    #[test]
    fn a_wide_mutable_struct_is_reported_and_a_narrow_one_is_not() {
        let members = |n: usize| {
            (0..n)
                .map(|i| format!("long m{i};"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let wide = format!("module m {{ @mutable struct S {{ {} }}; }};", members(17));
        let got = messages(&wide);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("EMHEADER"), "{got:?}");

        let narrow = format!("module m {{ @mutable struct S {{ {} }}; }};", members(16));
        assert!(messages(&narrow).is_empty());
    }

    /// An `@appendable` type of the same width is not reported: it pays
    /// one delimiter for the whole sample, not one header per member.
    #[test]
    fn width_alone_is_not_the_rule() {
        let members = (0..20)
            .map(|i| format!("long m{i};"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!("module m {{ @appendable struct S {{ {members} }}; }};");
        assert!(messages(&source).is_empty());
    }

    #[test]
    fn a_name_that_is_a_keyword_elsewhere_is_reported() {
        let got = messages("module m { struct Class { long x; }; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("keyword"), "{got:?}");
    }

    /// Every rule is a warning. `check` decides whether a file is IDL;
    /// nothing here refuses one.
    #[test]
    fn no_rule_here_produces_an_error() {
        let ast = model("module m { struct Class { long x; }; };");
        assert!(
            run(&ast).iter().all(|d| d.severity == Severity::Warning),
            "a lint rule must not fail a build on its own"
        );
    }

    /// `topic-missing-key`: fires on a `@topic` struct with no `@key`
    /// member, stays silent once one member is the key — and stays
    /// silent on a keyless struct that is not a topic, because keyless
    /// non-topic types are the normal case.
    #[test]
    fn a_keyless_topic_is_reported_and_a_keyed_or_plain_one_is_not() {
        let got = messages("@topic struct Status { long value; string<32> unit; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            got[0].contains("Status") && got[0].contains("@key"),
            "{got:?}"
        );

        assert!(messages("@topic struct Status { @key long device; long value; };").is_empty());
        assert!(messages("struct Status { long value; };").is_empty());
    }

    /// The finding points at the declaration, not at offset zero.
    #[test]
    fn a_finding_carries_the_declaration_span() {
        let source = "module m { @topic struct Late { long value; }; };";
        let ast = model(source);
        let found = run(&ast);
        assert_eq!(found.len(), 1, "{found:?}");
        let at = found[0].primary;
        assert!(at.end > at.start, "{at:?}");
        let named = source
            .get(at.start as usize..at.end as usize)
            .expect("in range");
        assert!(named.contains("Late"), "{named}");
    }

    /// `unbounded-collection`: an unbounded sequence is reported, a
    /// bounded one is not — also when the sequence hides behind a
    /// typedef, and when an unbounded map is the collection.
    #[test]
    fn an_unbounded_sequence_or_map_is_reported_and_a_bounded_one_is_not() {
        let got = messages("struct S { sequence<long> samples; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            got[0].contains("samples") && got[0].contains("sequence"),
            "{got:?}"
        );

        let got = messages("typedef sequence<long> Samples; struct S { Samples samples; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("is an unbounded sequence"), "{got:?}");

        let got = messages("struct S { map<long, long> table; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("map"), "{got:?}");

        assert!(messages("struct S { sequence<long, 16> samples; };").is_empty());
        assert!(messages("struct S { map<long, long, 16> table; };").is_empty());
    }

    /// An unbounded *string* member is deliberately not this rule's
    /// finding: it is the idiomatic spelling, and a rule that fires on
    /// most files gets suppressed. The same string under `@key` is
    /// reported — by `unbounded-key`.
    #[test]
    fn an_unbounded_string_is_only_reported_when_it_is_the_key() {
        assert!(messages("struct S { string name; };").is_empty());

        let got = messages("struct S { @key string name; long v; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            got[0].contains("name") && got[0].contains("@key"),
            "{got:?}"
        );
    }

    /// `unbounded-key`: bounded keys are silent; an unbounded sequence
    /// reached through a `@key` struct member's own key is reported.
    #[test]
    fn a_bounded_key_is_silent_and_a_nested_unbounded_one_is_not() {
        assert!(messages("struct S { @key string<64> name; long v; };").is_empty());
        assert!(messages("struct S { @key long id; string name; };").is_empty());

        let got = messages(
            "@final struct Inner { @key sequence<octet> raw; long other; }; \
             @final struct Outer { @key Inner which; long v; };",
        );
        // Three findings: `Inner.raw` as an unbounded collection, the
        // same member as an unbounded key, and `Outer.which` whose key
        // closure reaches it. The overlap is intended — the two rules
        // name two different costs of the one declaration.
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(
            got.iter()
                .any(|m| m.contains("Inner.raw") && m.contains("@key")),
            "{got:?}"
        );
        assert!(got.iter().any(|m| m.contains("Outer.which")), "{got:?}");
    }

    /// `unused-typedef`: a typedef nothing references is reported; one
    /// that a member uses is not, and in a chain only the unused end is
    /// named — its target is referenced by the chain itself.
    #[test]
    fn an_unused_typedef_is_reported_and_a_used_one_is_not() {
        let got = messages("module m { typedef long Meters; struct S { long plain; }; };");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("Meters"), "{got:?}");

        assert!(
            messages("module m { typedef long Meters; struct S { Meters height; }; };").is_empty()
        );

        let got = messages(
            "module m { typedef long Meters; typedef Meters Altitude; struct S { long x; }; };",
        );
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("Altitude"), "{got:?}");
    }

    /// A typedef used only inside another type's shell — a sequence
    /// element, a union case, a constant — is used.
    #[test]
    fn every_reference_position_counts_as_use() {
        assert!(messages("typedef long T; struct S { sequence<T, 4> xs; };").is_empty());
        assert!(
            messages("typedef long T; union U switch (short) { case 1: T a; default: octet b; };")
                .is_empty()
        );
        assert!(messages("typedef long T; const T WIDTH = 3;").is_empty());
    }
}
