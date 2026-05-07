// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Earley-Engine-Datentypen: [`EarleyItem`] und [`StateSet`].
//!
//! Ein **Earley-Item** ist die Triple `[A → α · β, i]`:
//!
//! - `A` ist die linke Seite einer Production,
//! - `α · β` ist die rechte Seite mit einem **Dot**, der die aktuelle
//!   Parser-Position innerhalb der Alternative markiert,
//! - `i` ist die **Origin** — die Position im Input-Token-Stream, an der
//!   das Parsing dieser Production gestartet wurde.
//!
//! Ein **State-Set** `Sₖ` ist die Menge aller Earley-Items, die zur
//! Token-Position `k` aktiv sind. Die Engine konstruiert pro Token-Position
//! ein State-Set und fuehrt darauf die drei Earley-Operationen aus
//! (Scan, Predict, Complete) — implementiert in Task 1.4.
//!
//! Siehe RFC 0001 §5.2 und Aycock/Horspool 2002 „Practical Earley Parsing".
//!
//! Datenstrukturen sind bewusst einfach:
//!
//! - [`EarleyItem`] ist `Copy` + `Hash` + `Eq` — laesst sich in Sets und
//!   Maps verwenden, ohne Clone-Overhead.
//! - [`StateSet`] haelt Items in Einfuege-Reihenfolge (fuer deterministische
//!   Iteration) plus ein Dedup-Set. Duplicate-Inserts sind no-ops und
//!   werden vom Rueckgabewert `false` signalisiert.

use std::collections::HashSet;

use crate::grammar::{Grammar, GrammarLike, ProductionId, Symbol};

/// Ein Earley-Item — Parser-Zustand innerhalb einer Grammar-Alternative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EarleyItem {
    /// Die Production, deren Alternative gerade gematcht wird.
    pub production: ProductionId,
    /// Index der Alternative innerhalb `Production::alternatives`.
    pub alternative_index: usize,
    /// Position des Dots — Anzahl der bereits gematchten Symbole der
    /// Alternative.
    pub dot: usize,
    /// Token-Stream-Position, an der das Parsing dieses Items begonnen hat.
    pub origin: usize,
}

impl EarleyItem {
    /// Konstruiert ein frisches Item mit Dot vor dem ersten Symbol.
    #[must_use]
    pub const fn new(production: ProductionId, alternative_index: usize, origin: usize) -> Self {
        Self {
            production,
            alternative_index,
            dot: 0,
            origin,
        }
    }

    /// Liefert ein neues Item mit Dot um eins weiter, ohne das Original zu
    /// modifizieren (`[A → α X · β]` aus `[A → α · X β]`).
    #[must_use]
    pub const fn advance(self) -> Self {
        Self {
            dot: self.dot + 1,
            ..self
        }
    }

    /// Greift auf die Alternative-Symbol-Sequenz in der Grammar zu.
    ///
    /// Liefert `None`, wenn `production` oder `alternative_index` nicht
    /// existieren (deutet auf einen Grammar-Konstruktionsfehler, der vom
    /// Validator erkannt wird, siehe `crate::grammar::validate`).
    #[must_use]
    pub fn symbols<'g, G: GrammarLike + ?Sized>(&self, grammar: &'g G) -> Option<&'g [Symbol]> {
        grammar
            .production(self.production)?
            .alternatives
            .get(self.alternative_index)
            .map(|alt| alt.symbols)
    }

    /// `true`, wenn der Dot am Ende der Alternative steht (Production ist
    /// vollstaendig gematcht — wird von der Complete-Operation konsumiert).
    #[must_use]
    pub fn is_complete<G: GrammarLike + ?Sized>(&self, grammar: &G) -> bool {
        self.symbols(grammar)
            .is_some_and(|syms| self.dot >= syms.len())
    }

    /// Das Symbol unmittelbar nach dem Dot, oder `None` wenn das Item
    /// komplett ist.
    #[must_use]
    pub fn next_symbol<'g, G: GrammarLike + ?Sized>(&self, grammar: &'g G) -> Option<&'g Symbol> {
        self.symbols(grammar)?.get(self.dot)
    }
}

#[allow(dead_code)]
const _GRAMMAR_TYPE: Option<Grammar> = None;

/// Menge aller Earley-Items einer Token-Position.
///
/// Items werden in Einfuege-Reihenfolge iteriert (deterministisch), aber
/// intern wird ein `HashSet` zur Duplicate-Erkennung gefuehrt. Das ist
/// wichtig, weil die Earley-Operationen (besonders Predict und Complete)
/// oft identische Items erzeugen.
#[derive(Debug, Clone, Default)]
pub struct StateSet {
    items: Vec<EarleyItem>,
    seen: HashSet<EarleyItem>,
}

impl StateSet {
    /// Neuer leerer State-Set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuegt ein Item ein, sofern es nicht bereits im Set liegt.
    ///
    /// Liefert `true` bei echtem Hinzufuegen, `false` wenn Duplikat.
    /// Der Rueckgabewert steuert in der Engine den Arbeits-Queue-Fortschritt
    /// (kein erneutes Predict/Complete fuer Duplikate).
    pub fn push(&mut self, item: EarleyItem) -> bool {
        if self.seen.insert(item) {
            self.items.push(item);
            true
        } else {
            false
        }
    }

    /// Alle Items in Einfuege-Reihenfolge.
    #[must_use]
    pub fn items(&self) -> &[EarleyItem] {
        &self.items
    }

    /// Anzahl eindeutiger Items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true`, wenn der Set keine Items enthaelt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Prueft, ob ein Item bereits im Set liegt.
    #[must_use]
    pub fn contains(&self, item: &EarleyItem) -> bool {
        self.seen.contains(item)
    }

    /// Iteriert ueber Items in Einfuege-Reihenfolge.
    pub fn iter(&self) -> impl Iterator<Item = &EarleyItem> {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{
        AltRef as _AltRefUnused, Alternative, Grammar, IdlVersion, Production, SpecRef, TokenKind,
    };
    // AltRef wird vom Grammar-Modul ueber `pub use` re-exportiert; zum
    // Import-Warning-Silencing alias.
    #[allow(unused_imports)]
    use _AltRefUnused as _;

    const TEST_SPEC: SpecRef = SpecRef {
        doc: "TEST",
        section: "0.0",
    };

    /// Grammar: A ::= "x" "y"   (start: A)
    const PROD_A: Production = Production {
        id: ProductionId(0),
        name: "a",
        spec_ref: TEST_SPEC,
        alternatives: &[Alternative {
            name: Some("xy"),
            symbols: &[
                Symbol::Terminal(TokenKind::Keyword("x")),
                Symbol::Terminal(TokenKind::Keyword("y")),
            ],
            note: None,
        }],
        ast_hint: None,
    };

    const GRAMMAR: Grammar = Grammar {
        name: "test",
        version: IdlVersion::V4_2,
        productions: &[PROD_A],
        start: ProductionId(0),
        token_rules: &[],
    };

    #[test]
    fn earley_item_new_has_dot_at_zero() {
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        assert_eq!(item.dot, 0);
        assert_eq!(item.origin, 0);
        assert_eq!(item.alternative_index, 0);
    }

    #[test]
    fn earley_item_advance_increments_dot() {
        let item = EarleyItem::new(ProductionId(0), 0, 5);
        let advanced = item.advance();
        assert_eq!(advanced.dot, 1);
        // Andere Felder unveraendert.
        assert_eq!(advanced.origin, 5);
        assert_eq!(advanced.production, ProductionId(0));
        // Original unveraendert.
        assert_eq!(item.dot, 0);
    }

    #[test]
    fn earley_item_symbols_reads_from_grammar() {
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        let expected: &[Symbol] = &[
            Symbol::Terminal(TokenKind::Keyword("x")),
            Symbol::Terminal(TokenKind::Keyword("y")),
        ];
        assert_eq!(item.symbols(&GRAMMAR), Some(expected));
    }

    #[test]
    fn earley_item_symbols_returns_none_for_invalid_production() {
        let item = EarleyItem::new(ProductionId(99), 0, 0);
        assert!(item.symbols(&GRAMMAR).is_none());
    }

    #[test]
    fn earley_item_symbols_returns_none_for_invalid_alt_index() {
        let item = EarleyItem::new(ProductionId(0), 99, 0);
        assert!(item.symbols(&GRAMMAR).is_none());
    }

    #[test]
    fn earley_item_is_complete_when_dot_at_end() {
        let item = EarleyItem {
            production: ProductionId(0),
            alternative_index: 0,
            dot: 2,
            origin: 0,
        };
        assert!(item.is_complete(&GRAMMAR));
    }

    #[test]
    fn earley_item_is_not_complete_when_dot_in_middle() {
        let item = EarleyItem {
            production: ProductionId(0),
            alternative_index: 0,
            dot: 1,
            origin: 0,
        };
        assert!(!item.is_complete(&GRAMMAR));
    }

    #[test]
    fn earley_item_next_symbol_is_first_symbol_at_start() {
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        let next = item.next_symbol(&GRAMMAR);
        assert_eq!(next, Some(&Symbol::Terminal(TokenKind::Keyword("x"))));
    }

    #[test]
    fn earley_item_next_symbol_after_advance_is_second_symbol() {
        let item = EarleyItem::new(ProductionId(0), 0, 0).advance();
        let next = item.next_symbol(&GRAMMAR);
        assert_eq!(next, Some(&Symbol::Terminal(TokenKind::Keyword("y"))));
    }

    #[test]
    fn earley_item_next_symbol_is_none_when_complete() {
        let item = EarleyItem {
            production: ProductionId(0),
            alternative_index: 0,
            dot: 2,
            origin: 0,
        };
        assert!(item.next_symbol(&GRAMMAR).is_none());
    }

    #[test]
    fn state_set_is_initially_empty() {
        let set = StateSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn state_set_push_adds_new_item_and_returns_true() {
        let mut set = StateSet::new();
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        assert!(set.push(item));
        assert_eq!(set.len(), 1);
        assert!(set.contains(&item));
        assert!(!set.is_empty());
    }

    #[test]
    fn state_set_push_duplicate_returns_false() {
        let mut set = StateSet::new();
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        assert!(set.push(item));
        assert!(
            !set.push(item),
            "second insert of same item must return false"
        );
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn state_set_push_different_items_keeps_both() {
        let mut set = StateSet::new();
        let a = EarleyItem::new(ProductionId(0), 0, 0);
        let b = EarleyItem::new(ProductionId(0), 0, 1); // andere Origin
        assert!(set.push(a));
        assert!(set.push(b));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn state_set_preserves_insertion_order() {
        let mut set = StateSet::new();
        let a = EarleyItem::new(ProductionId(0), 0, 0);
        let b = EarleyItem::new(ProductionId(0), 0, 1);
        let c = EarleyItem::new(ProductionId(0), 0, 2);
        set.push(a);
        set.push(b);
        set.push(c);
        let order: Vec<_> = set.iter().copied().collect();
        assert_eq!(order, vec![a, b, c]);
    }

    #[test]
    fn state_set_contains_reflects_inserts_exactly() {
        let mut set = StateSet::new();
        let a = EarleyItem::new(ProductionId(0), 0, 0);
        let b = EarleyItem::new(ProductionId(0), 0, 1);
        set.push(a);
        assert!(set.contains(&a));
        assert!(!set.contains(&b));
    }

    #[test]
    fn state_set_default_equals_new() {
        let default_set = StateSet::default();
        let new_set = StateSet::new();
        assert_eq!(default_set.len(), new_set.len());
        assert_eq!(default_set.is_empty(), new_set.is_empty());
    }

    #[test]
    fn advanced_item_is_distinct_from_original() {
        // Regression: advance() und original muessen eine unterschiedliche
        // Hash-Identitaet haben, damit der State-Set beide aufnimmt.
        let mut set = StateSet::new();
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        let advanced = item.advance();
        assert!(set.push(item));
        assert!(set.push(advanced));
        assert_eq!(set.len(), 2);
    }
}
