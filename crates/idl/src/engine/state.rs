// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Earley-engine data types: [`EarleyItem`] and [`StateSet`].
//!
//! An **Earley item** is the triple `[A → α · β, i]`:
//!
//! - `A` is the left-hand side of a production,
//! - `α · β` is the right-hand side with a **dot** that marks the current
//!   parser position within the alternative,
//! - `i` is the **origin** — the position in the input token stream at which
//!   parsing of this production was started.
//!
//! A **state set** `Sₖ` is the set of all Earley items that are active at
//! token position `k`. The engine constructs one state set per token position
//! and runs the three Earley operations on it
//! (scan, predict, complete) — implemented in Task 1.4.
//!
//! See RFC 0001 §5.2 and Aycock/Horspool 2002 "Practical Earley Parsing".
//!
//! The data structures are deliberately simple:
//!
//! - [`EarleyItem`] is `Copy` + `Hash` + `Eq` — can be used in sets and
//!   maps without clone overhead.
//! - [`StateSet`] holds items in insertion order (for deterministic
//!   iteration) plus a dedup set. Duplicate inserts are no-ops and
//!   are signaled by the return value `false`.

use std::collections::HashSet;

use crate::grammar::{Grammar, GrammarLike, ProductionId, Symbol};

/// An Earley item — parser state within a grammar alternative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EarleyItem {
    /// The production whose alternative is currently being matched.
    pub production: ProductionId,
    /// Index of the alternative within `Production::alternatives`.
    pub alternative_index: usize,
    /// Position of the dot — number of already matched symbols of the
    /// alternative.
    pub dot: usize,
    /// Token-stream position at which parsing of this item began.
    pub origin: usize,
}

impl EarleyItem {
    /// Constructs a fresh item with the dot before the first symbol.
    #[must_use]
    pub const fn new(production: ProductionId, alternative_index: usize, origin: usize) -> Self {
        Self {
            production,
            alternative_index,
            dot: 0,
            origin,
        }
    }

    /// Returns a new item with the dot advanced by one, without modifying
    /// the original (`[A → α X · β]` from `[A → α · X β]`).
    #[must_use]
    pub const fn advance(self) -> Self {
        Self {
            dot: self.dot + 1,
            ..self
        }
    }

    /// Accesses the alternative's symbol sequence in the grammar.
    ///
    /// Returns `None` if `production` or `alternative_index` do not
    /// exist (indicates a grammar construction error, detected by the
    /// validator, see `crate::grammar::validate`).
    #[must_use]
    pub fn symbols<'g, G: GrammarLike + ?Sized>(&self, grammar: &'g G) -> Option<&'g [Symbol]> {
        grammar
            .production(self.production)?
            .alternatives
            .get(self.alternative_index)
            .map(|alt| alt.symbols)
    }

    /// `true` if the dot is at the end of the alternative (the production is
    /// fully matched — consumed by the complete operation).
    #[must_use]
    pub fn is_complete<G: GrammarLike + ?Sized>(&self, grammar: &G) -> bool {
        self.symbols(grammar)
            .is_some_and(|syms| self.dot >= syms.len())
    }

    /// The symbol immediately after the dot, or `None` if the item
    /// is complete.
    #[must_use]
    pub fn next_symbol<'g, G: GrammarLike + ?Sized>(&self, grammar: &'g G) -> Option<&'g Symbol> {
        self.symbols(grammar)?.get(self.dot)
    }
}

#[allow(dead_code)]
const _GRAMMAR_TYPE: Option<Grammar> = None;

/// Set of all Earley items of a token position.
///
/// Items are iterated in insertion order (deterministic), but
/// internally a `HashSet` is kept for duplicate detection. That is
/// important because the Earley operations (especially predict and complete)
/// often produce identical items.
#[derive(Debug, Clone, Default)]
pub struct StateSet {
    items: Vec<EarleyItem>,
    seen: HashSet<EarleyItem>,
}

impl StateSet {
    /// New empty state set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an item, provided it is not already in the set.
    ///
    /// Returns `true` on an actual add, `false` if a duplicate.
    /// The return value drives the work-queue progress in the engine
    /// (no repeated predict/complete for duplicates).
    pub fn push(&mut self, item: EarleyItem) -> bool {
        if self.seen.insert(item) {
            self.items.push(item);
            true
        } else {
            false
        }
    }

    /// All items in insertion order.
    #[must_use]
    pub fn items(&self) -> &[EarleyItem] {
        &self.items
    }

    /// Number of unique items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` if the set contains no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Checks whether an item is already in the set.
    #[must_use]
    pub fn contains(&self, item: &EarleyItem) -> bool {
        self.seen.contains(item)
    }

    /// Iterates over items in insertion order.
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
    // AltRef is re-exported from the grammar module via `pub use`; aliased
    // to silence the import warning.
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
        // Regression: advance() and the original must have a different
        // hash identity, so that the state set takes in both.
        let mut set = StateSet::new();
        let item = EarleyItem::new(ProductionId(0), 0, 0);
        let advanced = item.advance();
        assert!(set.push(item));
        assert!(set.push(advanced));
        assert_eq!(set.len(), 2);
    }
}
