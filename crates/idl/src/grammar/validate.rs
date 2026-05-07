// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar-Validation — statische Konsistenz-Checks ueber eine [`Grammar`].
//!
//! Findet Konstruktions-Fehler und problematische Muster, bevor die
//! Parse-Engine gestartet wird:
//!
//! - **Invalid-Start**: `Grammar::start` zeigt auf eine nicht existierende
//!   Production.
//! - **Dangling-Reference**: eine [`Symbol::Nonterminal(id)`] verweist auf
//!   eine ProductionId, die nicht in `Grammar::productions` enthalten ist.
//! - **Left-Recursion**: eine Production erreicht sich selbst ueber die
//!   leftmost Nonterminal-Kette. Earley handelt Linksrekursion korrekt,
//!   aber wir flaggen sie als Warnung fuer Grammar-Review. Pfade werden
//!   als Sequenz von [`PathStep`] gemeldet, damit die betroffene
//!   Alternative pro Schritt identifizierbar ist.
//! - **Unused-Production**: eine Production ist vom Start aus nicht
//!   erreichbar. Solche Eintraege sind entweder toter BNF-Code oder ein
//!   fehlender Verweis.
//! - **First/First-Conflict**: zwei Alternativen derselben Production haben
//!   ueberlappende FIRST-Mengen. Die Berechnung nutzt die klassische
//!   transitive-Closure-Methode (Dragon Book) ueber alle Productions mit
//!   Epsilon-Propagation durch `Repeat`/`Choice`/leere Alternativen.
//!
//! Siehe RFC 0001 §5.1 und §9 Q-1. Scope-Deviation in
//! `.planning/wp-0.3-idl-parser/PLAN.md` (Task 1.2b).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

use super::compile::CompiledGrammar;
use super::{AltRef, Grammar, Production, ProductionId, RepeatKind, Symbol, TokenKind};

/// Schweregrad eines Validation-Befunds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Grammar ist nicht verwendbar — Engine wuerde fehlschlagen oder
    /// definitiv falsches Ergebnis liefern.
    Error,
    /// Grammar ist verwendbar, aber ein Muster deutet auf einen Bug oder
    /// auf Wartungs-Risiken hin (unused productions, left recursion,
    /// FIRST/FIRST-Konflikte).
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

/// Ein Schritt im Rekursions-Pfad einer [`ValidationIssue::LeftRecursion`].
///
/// Traegt sowohl die Production als auch die konkrete Alternative, ueber
/// die der Pfad weiterlaeuft. Bei direkter Rekursion (`A ::= A ...`) hat
/// `path` die Form `[step(A, alt0), step(A, alt0)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathStep {
    /// Production an dieser Stelle des Zyklus.
    pub production: ProductionId,
    /// Die Alternative, ueber die der naechste Schritt erreicht wurde.
    pub alternative: AltRef,
}

/// Ein konkreter Validation-Befund.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    /// `Grammar::start` zeigt auf eine nicht existierende ProductionId.
    InvalidStart {
        /// Die ungueltige ID.
        requested: ProductionId,
        /// Anzahl tatsaechlich vorhandener Productions.
        production_count: usize,
    },
    /// Ein `Symbol::Nonterminal(to)` verweist auf eine ProductionId, die
    /// nicht existiert.
    DanglingReference {
        /// Production, in der der Verweis steht.
        from: ProductionId,
        /// Die Alternative innerhalb der `from`-Production.
        from_alt: AltRef,
        /// Production, auf die verwiesen wurde.
        to: ProductionId,
    },
    /// Direkte oder indirekte Linksrekursion.
    LeftRecursion {
        /// Rekursions-Pfad, beginnend und endend bei derselben ProductionId.
        /// Jeder Schritt enthaelt die durchlaufene Alternative.
        path: Vec<PathStep>,
    },
    /// Production ist vom Start aus nicht erreichbar.
    UnusedProduction {
        /// ID der unerreichbaren Production.
        id: ProductionId,
        /// Name der Production (aus `Production::name`).
        name: &'static str,
    },
    /// Zwei Alternativen derselben Production haben ueberlappende
    /// FIRST-Mengen. Meldet pro Konflikt-Paar einen Report.
    FirstFirstConflict {
        /// Die betroffene Production.
        production: ProductionId,
        /// Die erste der beiden konfligierenden Alternativen (kleinerer Index).
        left: AltRef,
        /// Die zweite der beiden konfligierenden Alternativen.
        right: AltRef,
        /// Terminals, die in beiden FIRST-Mengen vorkommen. Sortiert nach
        /// interner Ordnung (deterministisch ueber Laeufe hinweg).
        shared_terminals: Vec<TokenKind>,
    },
}

impl ValidationIssue {
    /// Schweregrad des Befunds.
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

/// Gesammelte Befunde eines Validation-Laufs.
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

    /// Fuegt einen Befund hinzu.
    pub fn push(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// Alle Befunde in Hinzufuege-Reihenfolge.
    #[must_use]
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    /// Nur die `Error`-Befunde.
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity() == Severity::Error)
    }

    /// Nur die `Warning`-Befunde.
    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity() == Severity::Warning)
    }

    /// `true`, wenn mindestens ein `Error` vorliegt — Grammar ist dann
    /// nicht Engine-fest.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }

    /// Gesamt-Anzahl Befunde.
    #[must_use]
    pub fn len(&self) -> usize {
        self.issues.len()
    }

    /// `true`, wenn der Report leer ist (Grammar sauber).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Validation-Sweep ueber eine [`CompiledGrammar`] (T6.9).
///
/// prueft nur die kritischen Errors (Invalid-Start +
/// Dangling-Reference). Die warning-stufigen Checks (Left-Recursion,
/// Unused-Production, First/First-Conflict) sind speziell auf
/// [`Grammar`] gemuenzt und werden in Phase 1 generalisiert.
///
/// Wird typischerweise nach [`super::compose::compose`] aufgerufen,
/// um sicherzustellen, dass ein Vendor-Delta keine Dangling-Refs
/// einfuehrt.
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

/// Fuehrt alle Validation-Checks auf der Grammar aus und liefert einen Report.
///
/// Die Checks laufen in fester Reihenfolge; bei Invalid-Start bricht die
/// Validation ab, damit nicht weitere irrefuehrende Warnungen produziert
/// werden.
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
// Start-Check
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

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
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
// Left-Recursion mit PathStep
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

/// DFS entlang der leftmost Nonterminal-Kette. Liefert den Pfad zurueck zum
/// Startknoten, wenn eine Rekursion gefunden wird.
///
/// Jeder `PathStep` enthaelt die Alternative, ueber die der naechste Knoten
/// erreicht wurde. Der zurueckgelieferte Pfad beginnt und endet auf `start`.
fn find_left_recursion_path(grammar: &Grammar, start: ProductionId) -> Option<Vec<PathStep>> {
    // Frame: (aktuelle Production, Iterator ueber (AltRef, leftmost-Nonterminal-Option)-Paare)
    let start_prod = grammar.production(start)?;
    let mut visited: HashSet<ProductionId> = HashSet::new();
    visited.insert(start);
    let mut stack: Vec<(ProductionId, Vec<(AltRef, ProductionId)>)> =
        vec![(start, leftmost_moves(start_prod))];

    while let Some((_, moves)) = stack.last_mut() {
        let Some((alt_ref, next)) = moves.pop() else {
            // Frame erschoepft. Pop den Frame; der Parent-Frame trifft seine
            // naechste Move-Wahl in der naechsten Loop-Iteration. Wir duerfen
            // hier den Parent nicht weiter konsumieren — sonst werden ungeprobte
            // Move-Alternativen uebersprungen (Bug, der LR durch
            // nicht-leftmost Alternativen unsichtbar machte).
            stack.pop();
            continue;
        };

        if next == start {
            // Zyklus gefunden. Rekonstruiere Pfad entlang der Stack-Prefix:
            // fuer jede Ebene die Alternative, die zum naechsten Knoten fuehrt.
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

/// Rekonstruiert den Rekursions-Pfad in `PathStep`-Form.
///
/// Vorgehen: wiederhole die DFS ohne Backtracking, nur entlang der Stack-
/// Prefix, und gib fuer jede Ebene den tatsaechlich gewaehlten `AltRef`
/// zurueck (den letzten konsumierten Move der jeweiligen Ebene).
fn rebuild_left_recursion_path(
    grammar: &Grammar,
    start: ProductionId,
    stack: &[(ProductionId, Vec<(AltRef, ProductionId)>)],
    final_alt: AltRef,
) -> Vec<PathStep> {
    // Wir haben im Stack pro Ebene `moves_left`. Der gerade konsumierte Move
    // war der oberste Eintrag vor dem pop; da wir pop() nach dem Konsum
    // gemacht haben, liegt der konsumierte Move nicht mehr in der Liste.
    // Wir re-laufen von start aus entlang der Productions im Stack und
    // waehlen jeweils den leftmost-Move zum naechsten Production-Knoten.
    let mut path: Vec<PathStep> = Vec::with_capacity(stack.len() + 1);
    for (i, (pid, _)) in stack.iter().enumerate() {
        let next_pid = if i + 1 < stack.len() {
            stack[i + 1].0
        } else {
            start // letzter Schritt fuehrt zurueck zum Startknoten
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
    // Abschluss: Zielknoten explizit anzeigen.
    path.push(PathStep {
        production: start,
        alternative: final_alt,
    });
    path
}

/// Findet die erste Alternative einer Production, deren leftmost-Kette auf
/// `target` fuehrt.
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

/// Pro Alternative der Production: `(AltRef, naechstes-leftmost-Nonterminal)`.
/// Alternativen, die terminal beginnen oder leer sind, werden weggelassen.
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
        // ID kommt aus der Production selbst — nicht der Slice-Index,
        // weil Productions nicht zwingend in ID-Reihenfolge im Slice
        // sind (Eintragungs-Reihenfolge vs. ID-Allokation).
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

/// FIRST-Menge einer Symbol-Sequenz oder Production. Epsilon wird separat
/// getrackt, nicht als synthetisches Terminal.
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

/// Berechnet FIRST-Mengen fuer alle Productions einer Grammar.
///
/// Klassische Fixpoint-Iteration: initial alle Mengen leer, pro Iteration
/// werden die FIRST-Mengen jeder Production aus FIRST ihrer Alternativen
/// erweitert, bis Konvergenz.
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

/// FIRST einer Symbol-Sequenz, basierend auf aktuellen Production-FIRST-
/// Mengen.
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

/// FIRST eines einzelnen Symbols.
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
            // ZeroOrMore und Optional koennen epsilon produzieren, auch wenn
            // der innere Body das nicht kann.
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
// First/First-Conflict (pairwise, ueber echtes FIRST)
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
        // Regression gegen die alte Heuristik: eine Alt beginnt mit einem
        // Nonterminal, das wiederum mit demselben Terminal wie die andere Alt
        // anfaengt. Die alte Heuristik hat das nicht gefunden — FIRST-Set
        // ueber transitive Closure muss.
        //
        // A ::= B | "x"
        // B ::= "x" "y"
        //
        // FIRST(B) = {"x"}, FIRST(A's alt 0) via B = {"x"}, FIRST(alt 1) = {"x"}
        // → Konflikt auf "x" erwartet.
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
            "FIRST-Set-Closure muss Konflikt durch Nonterminal-Kette finden"
        );
    }

    #[test]
    fn first_set_handles_epsilon_via_optional_repeat() {
        // A ::= [ "x" ] "y" | "y"
        //
        // FIRST(alt 0): { "x", "y" } (Optional kann epsilon, dann faellt FIRST
        // durch zu "y"), FIRST(alt 1): { "y" } → Konflikt auf "y".
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
            "Epsilon-Propagation durch Optional muss 'y' als shared erkennen"
        );
    }

    #[test]
    fn no_false_positive_on_disjoint_first_sets() {
        // A ::= "x" | "y" — disjunkt, kein Konflikt.
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
            "disjunkte FIRST-Mengen duerfen keinen Konflikt melden"
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
        // coverage: justified — assert!-Fehlermeldung nur bei Test-Failure.
        assert!(
            !report
                .issues()
                .iter()
                .any(|i| matches!(i, ValidationIssue::UnusedProduction { .. })),
            "B und C muessen als erreichbar gelten, Report: {:?}",
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
        // Keine Unused / Dangling / InvalidStart — epsilon + "x" sind disjunkt,
        // kein FirstFirstConflict.
        let report = validate(&GR);
        assert!(
            report.is_empty(),
            "unerwartete Issues: {:?}",
            report.issues()
        );
    }

    // -----------------------------------------------------------------
    // T6.9 — validate_compiled fuer CompiledGrammar / Composition
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
        // Production-IDs starten bei 0; start = ProductionId(99) ist invalid.
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
