// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar validation — static consistency checks over a [`Grammar`].
//!
//! Finds construction errors and problematic patterns before the
//! parse engine is started:
//!
//! - **Invalid-start**: `Grammar::start` points to a non-existent
//!   production.
//! - **Dangling-reference**: a [`Symbol::Nonterminal(id)`] refers to
//!   a ProductionId not contained in `Grammar::productions`.
//! - **Left-recursion**: a production reaches itself via the
//!   leftmost nonterminal chain. Earley handles left recursion correctly,
//!   but we flag it as a warning for grammar review. Paths are
//!   reported as a sequence of [`PathStep`], so that the affected
//!   alternative is identifiable per step.
//! - **Unused-production**: a production is not reachable from the start.
//!   Such entries are either dead BNF code or a
//!   missing reference.
//! - **First/first-conflict**: two alternatives of the same production have
//!   overlapping FIRST sets. The computation uses the classic
//!   transitive-closure method (Dragon Book) over all productions with
//!   epsilon propagation through `Repeat`/`Choice`/empty alternatives.
//!
//! See RFC 0001 §5.1 and §9 Q-1. Scope deviation in
//! `.planning/wp-0.3-idl-parser/PLAN.md` (Task 1.2b).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

use super::compile::CompiledGrammar;
use super::{AltRef, Grammar, Production, ProductionId, RepeatKind, Symbol, TokenKind};

/// Severity of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// The grammar is unusable — the engine would fail or
    /// definitely return a wrong result.
    Error,
    /// The grammar is usable, but a pattern indicates a bug or
    /// maintenance risks (unused productions, left recursion,
    /// FIRST/FIRST conflicts).
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

/// A step in the recursion path of a [`ValidationIssue::LeftRecursion`].
///
/// Carries both the production and the concrete alternative via
/// which the path continues. For direct recursion (`A ::= A ...`),
/// `path` has the form `[step(A, alt0), step(A, alt0)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathStep {
    /// Production at this point of the cycle.
    pub production: ProductionId,
    /// The alternative via which the next step was reached.
    pub alternative: AltRef,
}

/// A concrete validation finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    /// `Grammar::start` points to a non-existent ProductionId.
    InvalidStart {
        /// The invalid ID.
        requested: ProductionId,
        /// Number of actually present productions.
        production_count: usize,
    },
    /// A `Symbol::Nonterminal(to)` refers to a ProductionId that
    /// does not exist.
    DanglingReference {
        /// Production in which the reference appears.
        from: ProductionId,
        /// The alternative within the `from` production.
        from_alt: AltRef,
        /// Production that was referenced.
        to: ProductionId,
    },
    /// Direct or indirect left recursion.
    LeftRecursion {
        /// Recursion path, beginning and ending at the same ProductionId.
        /// Each step contains the traversed alternative.
        path: Vec<PathStep>,
    },
    /// A production is not reachable from the start.
    UnusedProduction {
        /// ID of the unreachable production.
        id: ProductionId,
        /// Name of the production (from `Production::name`).
        name: &'static str,
    },
    /// Two alternatives of the same production have overlapping
    /// FIRST sets. Reports one entry per conflicting pair.
    FirstFirstConflict {
        /// The affected production.
        production: ProductionId,
        /// The first of the two conflicting alternatives (smaller index).
        left: AltRef,
        /// The second of the two conflicting alternatives.
        right: AltRef,
        /// Terminals appearing in both FIRST sets. Sorted by
        /// internal order (deterministic across runs).
        shared_terminals: Vec<TokenKind>,
    },
}

impl ValidationIssue {
    /// Severity of the finding.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        match self {
            Self::InvalidStart { .. } | Self::DanglingReference { .. } => Severity::Error,
            Self::LeftRecursion { .. }
            | Self::UnusedProduction { .. }
            | Self::FirstFirstConflict { .. } => Severity::Warning,
        }
    }
}

/// Collected findings of a validation run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// Neuer leerer Report.
    #[must_use]
    pub fn new() -> Self {
        Self { issues: Vec::new() }
    }

    /// Adds a finding.
    pub fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// All findings in insertion order.
    #[must_use]
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    /// Only the `Error` findings.
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity() == Severity::Error)
    }

    /// Only the `Warning` findings.
    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity() == Severity::Warning)
    }

    /// `true` if at least one `Error` is present — the grammar is then
    /// not engine-safe.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }

    /// Total number of findings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.issues.len()
    }

    /// `true` if the report is empty (grammar clean).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Validation sweep over a [`CompiledGrammar`] (T6.9).
///
/// Checks only the critical errors (invalid-start +
/// dangling-reference). The warning-level checks (left-recursion,
/// unused-production, first/first-conflict) are specifically aimed at
/// [`Grammar`] and are generalized in phase 1.
///
/// Typically called after [`super::compose::compose`],
/// to ensure that a vendor delta does not introduce dangling refs.
#[must_use]
pub fn validate_compiled(compiled: &CompiledGrammar) -> ValidationReport {
    let mut report = ValidationReport::new();
    if compiled.production(compiled.start).is_none() {
        report.push(ValidationIssue::InvalidStart {
            requested: compiled.start,
            production_count: compiled.production_count(),
        });
        return report;
    }
    for production in compiled.productions_iter() {
        for (alt_idx, alt) in production.alternatives.iter().enumerate() {
            for to in collect_nonterminal_refs(alt.symbols) {
                if compiled.production(to).is_none() {
                    report.push(ValidationIssue::DanglingReference {
                        from: production.id,
                        from_alt: AltRef {
                            index: alt_idx,
                            name: alt.name,
                        },
                        to,
                    });
                }
            }
        }
    }
    report
}

/// Runs all validation checks on the grammar and returns a report.
///
/// The checks run in a fixed order; on invalid-start the
/// validation aborts, so that no further misleading warnings are
/// produced.
#[must_use]
pub fn validate(grammar: &Grammar) -> ValidationReport {
    let mut report = ValidationReport::new();

    if check_start(grammar, &mut report) {
        return report;
    }

    check_dangling_references(grammar, &mut report);
    check_left_recursion(grammar, &mut report);
    check_unused_productions(grammar, &mut report);
    check_first_first_conflicts(grammar, &mut report);

    report
}

// ---------------------------------------------------------------------------
// Start check
// ---------------------------------------------------------------------------

fn check_start(grammar: &Grammar, report: &mut ValidationReport) -> bool {
    if grammar.production(grammar.start).is_none() {
        report.push(ValidationIssue::InvalidStart {
            requested: grammar.start,
            production_count: grammar.production_count(),
        });
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Dangling-Reference
// ---------------------------------------------------------------------------

fn check_dangling_references(grammar: &Grammar, report: &mut ValidationReport) {
    for production in grammar.productions_iter() {
        for (alt_idx, alt) in production.alternatives.iter().enumerate() {
            for to in collect_nonterminal_refs(alt.symbols) {
                if grammar.production(to).is_none() {
                    report.push(ValidationIssue::DanglingReference {
                        from: production.id,
                        from_alt: AltRef {
                            index: alt_idx,
                            name: alt.name,
                        },
                        to,
                    });
                }
            }
        }
    }
}

/// zerodds-lint: recursion-depth 64 (parser/AST walk; bounded by IDL nesting)
fn collect_nonterminal_refs(symbols: &[Symbol]) -> Vec<ProductionId> {
    let mut out = Vec::new();
    for sym in symbols {
        match sym {
            Symbol::Nonterminal(id) => out.push(*id),
            Symbol::Repeat(_, inner) => out.extend(collect_nonterminal_refs(inner)),
            Symbol::Choice(branches) => {
                for branch in *branches {
                    out.extend(collect_nonterminal_refs(branch));
                }
            }
            Symbol::Terminal(_) => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Left recursion with PathStep
// ---------------------------------------------------------------------------

fn check_left_recursion(grammar: &Grammar, report: &mut ValidationReport) {
    let mut reported: HashSet<ProductionId> = HashSet::new();
    for production in grammar.productions_iter() {
        if reported.contains(&production.id) {
            continue;
        }
        if let Some(path) = find_left_recursion_path(grammar, production.id) {
            for step in &path {
                reported.insert(step.production);
            }
            report.push(ValidationIssue::LeftRecursion { path });
        }
    }
}

/// DFS along the leftmost nonterminal chain. Returns the path back to the
/// start node if a recursion is found.
///
/// Each `PathStep` contains the alternative via which the next node
/// was reached. The returned path begins and ends at `start`.
fn find_left_recursion_path(grammar: &Grammar, start: ProductionId) -> Option<Vec<PathStep>> {
    // Frame: (current production, iterator over (AltRef, leftmost-nonterminal-option) pairs)
    let start_prod = grammar.production(start)?;
    let mut visited: HashSet<ProductionId> = HashSet::new();
    visited.insert(start);
    let mut stack: Vec<(ProductionId, Vec<(AltRef, ProductionId)>)> =
        vec![(start, leftmost_moves(start_prod))];

    while let Some((_, moves)) = stack.last_mut() {
        let Some((alt_ref, next)) = moves.pop() else {
            // Frame exhausted. Pop the frame; the parent frame makes its
            // next move choice in the next loop iteration. We must not
            // consume the parent further here — otherwise untried
            // move alternatives are skipped (a bug that made LR via
            // non-leftmost alternatives invisible).
            stack.pop();
            continue;
        };

        if next == start {
            // Cycle found. Reconstruct the path along the stack prefix:
            // for each level the alternative leading to the next node.
            return Some(rebuild_left_recursion_path(grammar, start, &stack, alt_ref));
        }

        if visited.insert(next) {
            let Some(next_prod) = grammar.production(next) else {
                continue;
            };
            stack.push((next, leftmost_moves(next_prod)));
        }
    }
    None
}

/// Reconstructs the recursion path in `PathStep` form.
///
/// Approach: repeat the DFS without backtracking, only along the stack
/// prefix, and return for each level the actually chosen `AltRef`
/// (the last consumed move of the respective level).
fn rebuild_left_recursion_path(
    grammar: &Grammar,
    start: ProductionId,
    stack: &[(ProductionId, Vec<(AltRef, ProductionId)>)],
    final_alt: AltRef,
) -> Vec<PathStep> {
    // In the stack we have `moves_left` per level. The just-consumed move
    // was the top entry before the pop; since we called pop() after the
    // consume, the consumed move is no longer in the list.
    // We re-run from start along the productions in the stack and
    // each time choose the leftmost move to the next production node.
    let mut path: Vec<PathStep> = Vec::with_capacity(stack.len() + 1);
    for (i, (pid, _)) in stack.iter().enumerate() {
        let next_pid = if i + 1 < stack.len() {
            stack[i + 1].0
        } else {
            start // the last step leads back to the start node
        };
        let Some(production) = grammar.production(*pid) else {
            continue;
        };
        let alt_ref = find_alt_leading_to(production, next_pid).unwrap_or(final_alt);
        path.push(PathStep {
            production: *pid,
            alternative: alt_ref,
        });
    }
    // Conclusion: show the target node explicitly.
    path.push(PathStep {
        production: start,
        alternative: final_alt,
    });
    path
}

/// Finds the first alternative of a production whose leftmost chain leads to
/// `target`.
fn find_alt_leading_to(production: &Production, target: ProductionId) -> Option<AltRef> {
    for (idx, alt) in production.alternatives.iter().enumerate() {
        if leftmost_nonterminal_of(alt.symbols) == Some(target) {
            return Some(AltRef {
                index: idx,
                name: alt.name,
            });
        }
    }
    None
}

/// Per alternative of the production: `(AltRef, next leftmost nonterminal)`.
/// Alternatives that begin with a terminal or are empty are omitted.
fn leftmost_moves(production: &Production) -> Vec<(AltRef, ProductionId)> {
    let mut out = Vec::new();
    for (idx, alt) in production.alternatives.iter().enumerate() {
        if let Some(nt) = leftmost_nonterminal_of(alt.symbols) {
            out.push((
                AltRef {
                    index: idx,
                    name: alt.name,
                },
                nt,
            ));
        }
    }
    out
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn leftmost_nonterminal_of(symbols: &[Symbol]) -> Option<ProductionId> {
    match symbols.first()? {
        Symbol::Nonterminal(id) => Some(*id),
        Symbol::Repeat(_, inner) => leftmost_nonterminal_of(inner),
        Symbol::Choice(branches) => branches.iter().find_map(|b| leftmost_nonterminal_of(b)),
        Symbol::Terminal(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Unused-Production
// ---------------------------------------------------------------------------

fn check_unused_productions(grammar: &Grammar, report: &mut ValidationReport) {
    let reachable = reachable_from_start(grammar);
    for production in grammar.productions.iter() {
        // The ID comes from the production itself — not the slice index,
        // because productions are not necessarily in ID order in the slice
        // (insertion order vs. ID allocation).
        if !reachable.contains(&production.id) {
            report.push(ValidationIssue::UnusedProduction {
                id: production.id,
                name: production.name,
            });
        }
    }
}

fn reachable_from_start(grammar: &Grammar) -> BTreeSet<ProductionId> {
    let mut reachable = BTreeSet::new();
    let mut frontier: Vec<ProductionId> = vec![grammar.start];
    while let Some(pid) = frontier.pop() {
        if !reachable.insert(pid) {
            continue;
        }
        let Some(production) = grammar.production(pid) else {
            continue;
        };
        for alt in production.alternatives {
            for next in collect_nonterminal_refs(alt.symbols) {
                if !reachable.contains(&next) {
                    frontier.push(next);
                }
            }
        }
    }
    reachable
}

// ---------------------------------------------------------------------------
// FIRST-Set-Computation (transitive closure, klassisch Dragon Book)
// ---------------------------------------------------------------------------

/// FIRST set of a symbol sequence or production. Epsilon is tracked
/// separately, not as a synthetic terminal.
#[derive(Debug, Clone, Default)]
struct First {
    terminals: BTreeSet<TokenKind>,
    epsilon: bool,
}

impl First {
    fn union_terminals(&mut self, other: &BTreeSet<TokenKind>) -> bool {
        let before = self.terminals.len();
        self.terminals.extend(other.iter().copied());
        self.terminals.len() != before
    }
}

/// Computes FIRST sets for all productions of a grammar.
///
/// Classic fixpoint iteration: initially all sets empty, per iteration
/// the FIRST sets of each production are extended from FIRST of their
/// alternatives, until convergence.
fn compute_first_sets(grammar: &Grammar) -> BTreeMap<ProductionId, First> {
    let mut sets: BTreeMap<ProductionId, First> = grammar
        .productions
        .iter()
        .map(|p| (p.id, First::default()))
        .collect();

    loop {
        let mut changed = false;
        for production in grammar.productions_iter() {
            for alt in production.alternatives {
                let alt_first = first_of_sequence(alt.symbols, &sets);
                let entry = sets.entry(production.id).or_default();
                if entry.union_terminals(&alt_first.terminals) {
                    changed = true;
                }
                if alt_first.epsilon && !entry.epsilon {
                    entry.epsilon = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    sets
}

/// FIRST of a symbol sequence, based on the current production FIRST
/// sets.
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn first_of_sequence(symbols: &[Symbol], sets: &BTreeMap<ProductionId, First>) -> First {
    let mut result = First::default();
    if symbols.is_empty() {
        result.epsilon = true;
        return result;
    }
    let mut all_can_epsilon = true;
    for sym in symbols {
        let sym_first = first_of_symbol(sym, sets);
        result.terminals.extend(sym_first.terminals.iter().copied());
        if !sym_first.epsilon {
            all_can_epsilon = false;
            break;
        }
    }
    result.epsilon = all_can_epsilon;
    result
}

/// FIRST of a single symbol.
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn first_of_symbol(symbol: &Symbol, sets: &BTreeMap<ProductionId, First>) -> First {
    match symbol {
        Symbol::Terminal(kind) => {
            let mut first = First::default();
            first.terminals.insert(*kind);
            first
        }
        Symbol::Nonterminal(id) => sets.get(id).cloned().unwrap_or_default(),
        Symbol::Repeat(kind, inner) => {
            let mut first = first_of_sequence(inner, sets);
            // ZeroOrMore and Optional can produce epsilon, even if
            // the inner body cannot.
            if matches!(kind, RepeatKind::ZeroOrMore | RepeatKind::Optional) {
                first.epsilon = true;
            }
            first
        }
        Symbol::Choice(branches) => {
            let mut combined = First::default();
            for branch in *branches {
                let branch_first = first_of_sequence(branch, sets);
                combined
                    .terminals
                    .extend(branch_first.terminals.iter().copied());
                if branch_first.epsilon {
                    combined.epsilon = true;
                }
            }
            combined
        }
    }
}

// ---------------------------------------------------------------------------
// First/first-conflict (pairwise, via real FIRST)
// ---------------------------------------------------------------------------

fn check_first_first_conflicts(grammar: &Grammar, report: &mut ValidationReport) {
    let first_sets = compute_first_sets(grammar);

    for production in grammar.productions_iter() {
        if production.alternatives.len() < 2 {
            continue;
        }

        // FIRST pro Alternative einmal berechnen.
        let alt_firsts: Vec<First> = production
            .alternatives
            .iter()
            .map(|alt| first_of_sequence(alt.symbols, &first_sets))
            .collect();

        for i in 0..production.alternatives.len() {
            for j in (i + 1)..production.alternatives.len() {
                let shared: BTreeSet<TokenKind> = alt_firsts[i]
                    .terminals
                    .intersection(&alt_firsts[j].terminals)
                    .copied()
                    .collect();
                if !shared.is_empty() {
                    report.push(ValidationIssue::FirstFirstConflict {
                        production: production.id,
                        left: AltRef {
                            index: i,
                            name: production.alternatives[i].name,
                        },
                        right: AltRef {
                            index: j,
                            name: production.alternatives[j].name,
                        },
                        shared_terminals: shared.into_iter().collect(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        Alternative, Grammar, IdlVersion, Production, ProductionId, SpecRef, Symbol, TokenKind,
    };
    use super::*;

    const TEST_SPEC: SpecRef = SpecRef {
        doc: "TEST",
        section: "0.0",
    };

    const CLEAN_GRAMMAR: Grammar = Grammar {
        name: "clean",
        version: IdlVersion::V4_2,
        productions: &[Production {
            id: ProductionId(0),
            name: "root",
            spec_ref: TEST_SPEC,
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Terminal(TokenKind::Keyword("foo"))],
                note: None,
            }],
            ast_hint: None,
        }],
        start: ProductionId(0),
        token_rules: &[],
    };

    #[test]
    fn clean_grammar_produces_no_issues() {
        let report = validate(&CLEAN_GRAMMAR);
        assert!(
            report.is_empty(),
            "unerwartete Issues: {:?}",
            report.issues()
        );
    }

    #[test]
    fn invalid_start_is_reported_as_error() {
        const BROKEN: Grammar = Grammar {
            name: "broken",
            version: IdlVersion::V4_2,
            productions: &[],
            start: ProductionId(7),
            token_rules: &[],
        };
        let report = validate(&BROKEN);
        assert_eq!(report.issues().len(), 1);
        assert!(matches!(
            report.issues()[0],
            ValidationIssue::InvalidStart {
                requested: ProductionId(7),
                production_count: 0,
            }
        ));
        assert!(report.has_errors());
    }

    #[test]
    fn invalid_start_short_circuits_further_checks() {
        const BROKEN: Grammar = Grammar {
            name: "broken",
            version: IdlVersion::V4_2,
            productions: &[],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&BROKEN);
        assert_eq!(report.len(), 1);
    }

    #[test]
    fn dangling_nonterminal_reference_reports_altref() {
        const GR: Grammar = Grammar {
            name: "dangling",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "root",
                spec_ref: TEST_SPEC,
                alternatives: &[Alternative {
                    name: Some("only"),
                    symbols: &[Symbol::Nonterminal(ProductionId(42))],
                    note: None,
                }],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let danglings = report
            .issues()
            .iter()
            .filter(|i| {
                matches!(
                    i,
                    ValidationIssue::DanglingReference {
                        from: ProductionId(0),
                        from_alt: AltRef {
                            index: 0,
                            name: Some("only")
                        },
                        to: ProductionId(42),
                    }
                )
            })
            .count();
        assert_eq!(danglings, 1);
    }

    #[test]
    fn direct_left_recursion_reports_pathstep() {
        // A ::= A "x"
        const GR: Grammar = Grammar {
            name: "left_rec",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "a",
                spec_ref: TEST_SPEC,
                alternatives: &[Alternative {
                    name: Some("self"),
                    symbols: &[
                        Symbol::Nonterminal(ProductionId(0)),
                        Symbol::Terminal(TokenKind::Keyword("x")),
                    ],
                    note: None,
                }],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let recursions: Vec<Vec<PathStep>> = report
            .issues()
            .iter()
            .filter_map(|i| match i {
                ValidationIssue::LeftRecursion { path } => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(recursions.len(), 1);
        let path = &recursions[0];
        assert!(path.iter().any(|s| s.production == ProductionId(0)
            && s.alternative
                == AltRef {
                    index: 0,
                    name: Some("self")
                }));
    }

    #[test]
    fn indirect_left_recursion_reports_full_path() {
        // A ::= B "x"; B ::= A "y"
        const GR: Grammar = Grammar {
            name: "indirect",
            version: IdlVersion::V4_2,
            productions: &[
                Production {
                    id: ProductionId(0),
                    name: "a",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: Some("a_via_b"),
                        symbols: &[
                            Symbol::Nonterminal(ProductionId(1)),
                            Symbol::Terminal(TokenKind::Keyword("x")),
                        ],
                        note: None,
                    }],
                    ast_hint: None,
                },
                Production {
                    id: ProductionId(1),
                    name: "b",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: Some("b_via_a"),
                        symbols: &[
                            Symbol::Nonterminal(ProductionId(0)),
                            Symbol::Terminal(TokenKind::Keyword("y")),
                        ],
                        note: None,
                    }],
                    ast_hint: None,
                },
            ],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let any_leftrec = report
            .issues()
            .iter()
            .any(|i| matches!(i, ValidationIssue::LeftRecursion { .. }));
        assert!(any_leftrec);
    }

    #[test]
    fn unused_production_is_reported_as_warning() {
        const GR: Grammar = Grammar {
            name: "unused",
            version: IdlVersion::V4_2,
            productions: &[
                Production {
                    id: ProductionId(0),
                    name: "a",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: None,
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                        note: None,
                    }],
                    ast_hint: None,
                },
                Production {
                    id: ProductionId(1),
                    name: "b_unused",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: None,
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("y"))],
                        note: None,
                    }],
                    ast_hint: None,
                },
            ],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let unused_count = report
            .issues()
            .iter()
            .filter(|i| {
                matches!(
                    i,
                    ValidationIssue::UnusedProduction {
                        id: ProductionId(1),
                        name: "b_unused"
                    }
                )
            })
            .count();
        assert_eq!(unused_count, 1);
        assert!(!report.has_errors());
    }

    #[test]
    fn first_first_conflict_between_literal_alternatives() {
        // A ::= "x" | "x" "y"
        const GR: Grammar = Grammar {
            name: "literal_conflict",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "a",
                spec_ref: TEST_SPEC,
                alternatives: &[
                    Alternative {
                        name: Some("short"),
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                        note: None,
                    },
                    Alternative {
                        name: Some("long"),
                        symbols: &[
                            Symbol::Terminal(TokenKind::Keyword("x")),
                            Symbol::Terminal(TokenKind::Keyword("y")),
                        ],
                        note: None,
                    },
                ],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let matching = report
            .issues()
            .iter()
            .filter(|i| {
                matches!(
                    i,
                    ValidationIssue::FirstFirstConflict {
                        production: ProductionId(0),
                        left: AltRef { index: 0, name: Some("short") },
                        right: AltRef { index: 1, name: Some("long") },
                        shared_terminals,
                    } if shared_terminals == &vec![TokenKind::Keyword("x")]
                )
            })
            .count();
        assert_eq!(matching, 1);
    }

    #[test]
    fn first_first_conflict_through_nonterminal_is_detected() {
        // Regression against the old heuristic: one alt begins with a
        // nonterminal that in turn begins with the same terminal as the other alt.
        // The old heuristic did not find this — the FIRST set
        // via transitive closure must.
        //
        // A ::= B | "x"
        // B ::= "x" "y"
        //
        // FIRST(B) = {"x"}, FIRST(A's alt 0) via B = {"x"}, FIRST(alt 1) = {"x"}
        // → conflict on "x" expected.
        const GR: Grammar = Grammar {
            name: "transitive_conflict",
            version: IdlVersion::V4_2,
            productions: &[
                Production {
                    id: ProductionId(0),
                    name: "a",
                    spec_ref: TEST_SPEC,
                    alternatives: &[
                        Alternative {
                            name: Some("via_b"),
                            symbols: &[Symbol::Nonterminal(ProductionId(1))],
                            note: None,
                        },
                        Alternative {
                            name: Some("direct_x"),
                            symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                            note: None,
                        },
                    ],
                    ast_hint: None,
                },
                Production {
                    id: ProductionId(1),
                    name: "b",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: None,
                        symbols: &[
                            Symbol::Terminal(TokenKind::Keyword("x")),
                            Symbol::Terminal(TokenKind::Keyword("y")),
                        ],
                        note: None,
                    }],
                    ast_hint: None,
                },
            ],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let conflict_shared: Vec<Vec<TokenKind>> = report
            .issues()
            .iter()
            .filter_map(|i| match i {
                ValidationIssue::FirstFirstConflict {
                    production: ProductionId(0),
                    shared_terminals,
                    ..
                } => Some(shared_terminals.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            conflict_shared,
            vec![vec![TokenKind::Keyword("x")]],
            "FIRST-set closure must find the conflict through a nonterminal chain"
        );
    }

    #[test]
    fn first_set_handles_epsilon_via_optional_repeat() {
        // A ::= [ "x" ] "y" | "y"
        //
        // FIRST(alt 0): { "x", "y" } (Optional can be epsilon, then FIRST falls
        // through to "y"), FIRST(alt 1): { "y" } → conflict on "y".
        const GR: Grammar = Grammar {
            name: "optional_epsilon",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "a",
                spec_ref: TEST_SPEC,
                alternatives: &[
                    Alternative {
                        name: Some("optional_x_then_y"),
                        symbols: &[
                            Symbol::Repeat(
                                RepeatKind::Optional,
                                &[Symbol::Terminal(TokenKind::Keyword("x"))],
                            ),
                            Symbol::Terminal(TokenKind::Keyword("y")),
                        ],
                        note: None,
                    },
                    Alternative {
                        name: Some("plain_y"),
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("y"))],
                        note: None,
                    },
                ],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        let has_y_conflict = report.issues().iter().any(|i| {
            matches!(
                i,
                ValidationIssue::FirstFirstConflict {
                    shared_terminals,
                    ..
                } if shared_terminals.contains(&TokenKind::Keyword("y"))
            )
        });
        assert!(
            has_y_conflict,
            "epsilon propagation through Optional must recognize 'y' as shared"
        );
    }

    #[test]
    fn no_false_positive_on_disjoint_first_sets() {
        // A ::= "x" | "y" — disjoint, no conflict.
        const GR: Grammar = Grammar {
            name: "disjoint",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "a",
                spec_ref: TEST_SPEC,
                alternatives: &[
                    Alternative {
                        name: None,
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                        note: None,
                    },
                    Alternative {
                        name: None,
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("y"))],
                        note: None,
                    },
                ],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        assert!(
            !report
                .issues()
                .iter()
                .any(|i| matches!(i, ValidationIssue::FirstFirstConflict { .. })),
            "disjoint FIRST sets must not report a conflict"
        );
    }

    #[test]
    fn report_errors_and_warnings_are_separable() {
        const GR: Grammar = Grammar {
            name: "mixed",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "a",
                spec_ref: TEST_SPEC,
                alternatives: &[Alternative {
                    name: None,
                    symbols: &[
                        Symbol::Nonterminal(ProductionId(0)),
                        Symbol::Nonterminal(ProductionId(99)),
                    ],
                    note: None,
                }],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        assert!(report.has_errors());
        assert!(report.errors().count() >= 1);
        assert!(report.warnings().count() >= 1);
    }

    #[test]
    fn severity_display_renders_lowercase() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
    }

    #[test]
    fn empty_report_is_clean() {
        let report = ValidationReport::new();
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(!report.has_errors());
    }

    #[test]
    fn default_report_equivalent_to_new() {
        let default_report: ValidationReport = ValidationReport::default();
        let new_report = ValidationReport::new();
        assert_eq!(default_report, new_report);
    }

    #[test]
    fn nested_nonterminals_in_repeat_and_choice_are_resolved() {
        const GR: Grammar = Grammar {
            name: "nested",
            version: IdlVersion::V4_2,
            productions: &[
                Production {
                    id: ProductionId(0),
                    name: "a",
                    spec_ref: TEST_SPEC,
                    alternatives: &[
                        Alternative {
                            name: None,
                            symbols: &[Symbol::Repeat(
                                RepeatKind::ZeroOrMore,
                                &[Symbol::Nonterminal(ProductionId(1))],
                            )],
                            note: None,
                        },
                        Alternative {
                            name: None,
                            symbols: &[Symbol::Choice(&[
                                &[Symbol::Nonterminal(ProductionId(2))],
                                &[Symbol::Nonterminal(ProductionId(1))],
                            ])],
                            note: None,
                        },
                    ],
                    ast_hint: None,
                },
                Production {
                    id: ProductionId(1),
                    name: "b",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: None,
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("b"))],
                        note: None,
                    }],
                    ast_hint: None,
                },
                Production {
                    id: ProductionId(2),
                    name: "c",
                    spec_ref: TEST_SPEC,
                    alternatives: &[Alternative {
                        name: None,
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("c"))],
                        note: None,
                    }],
                    ast_hint: None,
                },
            ],
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate(&GR);
        // coverage: justified — assert! error message only on test failure.
        assert!(
            !report
                .issues()
                .iter()
                .any(|i| matches!(i, ValidationIssue::UnusedProduction { .. })),
            "B and C must be considered reachable, report: {:?}",
            report.issues()
        );
    }

    #[test]
    fn first_set_empty_alternative_is_epsilon() {
        // A ::= ε | "x"
        const GR: Grammar = Grammar {
            name: "epsilon_alt",
            version: IdlVersion::V4_2,
            productions: &[Production {
                id: ProductionId(0),
                name: "a",
                spec_ref: TEST_SPEC,
                alternatives: &[
                    Alternative {
                        name: Some("empty"),
                        symbols: &[],
                        note: None,
                    },
                    Alternative {
                        name: Some("x"),
                        symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                        note: None,
                    },
                ],
                ast_hint: None,
            }],
            start: ProductionId(0),
            token_rules: &[],
        };
        // No Unused / Dangling / InvalidStart — epsilon + "x" are disjoint,
        // no FirstFirstConflict.
        let report = validate(&GR);
        assert!(
            report.is_empty(),
            "unerwartete Issues: {:?}",
            report.issues()
        );
    }

    // -----------------------------------------------------------------
    // T6.9 — validate_compiled for CompiledGrammar / composition
    // -----------------------------------------------------------------

    #[test]
    fn validate_compiled_clean_for_idl_42() {
        use super::super::compose::compose;
        use super::super::idl42::IDL_42;
        let composed = compose(&IDL_42, &[]);
        let report = validate_compiled(&composed);
        assert!(!report.has_errors());
    }

    #[test]
    fn validate_compiled_detects_invalid_start() {
        use super::super::IdlVersion;
        use super::super::compile::CompiledGrammar;
        // Production IDs start at 0; start = ProductionId(99) is invalid.
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: TEST_SPEC,
            alternatives: &[],
            ast_hint: None,
        }];
        let compiled = CompiledGrammar {
            name: "test",
            version: IdlVersion::V4_2,
            productions: PRODS.to_vec(),
            start: ProductionId(99),
            token_rules: &[],
        };
        let report = validate_compiled(&compiled);
        assert!(report.has_errors());
        assert!(
            report
                .issues()
                .iter()
                .any(|i| matches!(i, ValidationIssue::InvalidStart { .. }))
        );
    }

    #[test]
    fn validate_compiled_detects_dangling_reference() {
        use super::super::IdlVersion;
        use super::super::compile::CompiledGrammar;
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: TEST_SPEC,
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Nonterminal(ProductionId(99))],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = CompiledGrammar {
            name: "test",
            version: IdlVersion::V4_2,
            productions: PRODS.to_vec(),
            start: ProductionId(0),
            token_rules: &[],
        };
        let report = validate_compiled(&compiled);
        assert!(report.has_errors());
        assert!(
            report
                .issues()
                .iter()
                .any(|i| matches!(i, ValidationIssue::DanglingReference { .. }))
        );
    }
}
