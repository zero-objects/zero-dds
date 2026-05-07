// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Base+Delta-Komposition zu einer [`CompiledGrammar`] (T6.4).
//!
//! Pipeline:
//!
//! 1. Base-Grammar → [`compile`](super::compile::compile) (EBNF-Desugar)
//! 2. Fuer jeden Delta:
//!    - `additional_productions` an die Productions-Liste anhaengen
//!      (mit re-vergebenen IDs ab `current_count`)
//!    - `alternative_extensions` auf die Ziel-Productions anwenden
//!      (zusaetzliche Alternativen anhaengen)
//! 3. Kein erneuter Desugar-Pass — Deltas sollen pre-desugart sein
//!    (sie geben EBNF-frei vor;
//!    spaeter koennte ein Delta-Compile-Pass folgen).
//!
//! # Memory
//!
//! Wie [`compile`](super::compile::compile) nutzt diese Funktion
//! [`Box::leak`] fuer dynamische Production-Allokationen — siehe
//! Compile-Modul-Doc fuer Begruendung.

use super::compile::{CompiledGrammar, compile};
use super::deltas::GrammarDelta;
use super::{Alternative, Grammar, Production, ProductionId};

/// Komponiert eine Base-Grammar mit beliebig vielen Deltas.
///
/// Reihenfolge ist signifikant: spaetere Deltas koennen Productions
/// erweitern, die ein vorheriger Delta hinzugefuegt hat. Konflikte
/// (z.B. zwei Deltas, die dieselbe Alternative auf eine Production
/// anhaengen) entstehen nicht — Alternativen sind kommutativ-additiv.
#[must_use]
pub fn compose(base: &Grammar, deltas: &[&GrammarDelta]) -> CompiledGrammar {
    let mut compiled = compile(base);

    for delta in deltas {
        apply_delta(&mut compiled, delta);
    }

    compiled
}

fn apply_delta(compiled: &mut CompiledGrammar, delta: &GrammarDelta) {
    // 1. Bestehende Productions erweitern.
    for ext in delta.alternative_extensions {
        if let Some(idx) = production_index(compiled, ext.target) {
            extend_alternatives(&mut compiled.productions[idx], ext.extra_alternatives);
        }
    }

    // 2. Neue Productions anhaengen.
    for prod in delta.additional_productions {
        compiled.productions.push(*prod);
    }
}

fn production_index(compiled: &CompiledGrammar, id: ProductionId) -> Option<usize> {
    compiled.productions.iter().position(|p| p.id == id)
}

fn extend_alternatives(prod: &mut Production, extra: &[Alternative]) {
    if extra.is_empty() {
        return;
    }
    let mut combined: Vec<Alternative> = prod.alternatives.to_vec();
    combined.extend_from_slice(extra);
    let leaked: &'static [Alternative] = Box::leak(combined.into_boxed_slice());
    prod.alternatives = leaked;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::grammar::deltas::{AlternativeExtension, GrammarDelta};
    use crate::grammar::{IdlVersion, Symbol, TokenKind};

    const fn alt_with(symbols: &'static [Symbol]) -> Alternative {
        Alternative {
            name: None,
            symbols,
            note: None,
        }
    }

    const fn prod(id: u32, name: &'static str, alts: &'static [Alternative]) -> Production {
        Production {
            id: ProductionId(id),
            name,
            spec_ref: super::super::SpecRef {
                doc: "TEST",
                section: "0",
            },
            alternatives: alts,
            ast_hint: None,
        }
    }

    static BASE_ALTS: &[Alternative] = &[alt_with(&[Symbol::Terminal(TokenKind::Keyword("a"))])];
    static BASE_PRODS: &[Production] = &[prod(0, "root", BASE_ALTS)];

    static BASE: Grammar = Grammar {
        name: "base",
        version: IdlVersion::V4_2,
        productions: BASE_PRODS,
        start: ProductionId(0),
        token_rules: &[],
    };

    static EXTRA_ALTS: &[Alternative] = &[alt_with(&[Symbol::Terminal(TokenKind::Keyword("b"))])];

    static EXTENSIONS: &[AlternativeExtension] = &[AlternativeExtension {
        target: ProductionId(0),
        extra_alternatives: EXTRA_ALTS,
    }];

    static EXTRA_PROD_ALTS: &[Alternative] =
        &[alt_with(&[Symbol::Terminal(TokenKind::Keyword("c"))])];
    static EXTRA_PRODS: &[Production] = &[prod(99, "extra", EXTRA_PROD_ALTS)];

    static DELTA: GrammarDelta = GrammarDelta {
        name: "test_delta",
        additional_productions: EXTRA_PRODS,
        alternative_extensions: EXTENSIONS,
    };

    #[test]
    fn compose_without_deltas_returns_compiled_base() {
        let composed = compose(&BASE, &[]);
        assert_eq!(composed.production_count(), 1);
        assert_eq!(composed.start, ProductionId(0));
    }

    #[test]
    fn compose_with_delta_adds_extra_production() {
        let composed = compose(&BASE, &[&DELTA]);
        assert_eq!(composed.production_count(), 2);
        let extra = composed.productions.iter().find(|p| p.name == "extra");
        assert!(extra.is_some(), "extra production must be present");
    }

    #[test]
    fn compose_with_delta_extends_existing_alternatives() {
        let composed = compose(&BASE, &[&DELTA]);
        let root = composed
            .productions
            .iter()
            .find(|p| p.name == "root")
            .expect("root present");
        assert_eq!(
            root.alternatives.len(),
            2,
            "root must have base + delta alternatives"
        );
    }

    #[test]
    fn compose_keeps_base_alternative_first() {
        let composed = compose(&BASE, &[&DELTA]);
        let root = composed
            .productions
            .iter()
            .find(|p| p.name == "root")
            .expect("root");
        // Erste Alternative ist die Base-`a`.
        match root.alternatives[0].symbols {
            [Symbol::Terminal(TokenKind::Keyword("a"))] => {}
            other => panic!("expected base 'a' alternative first, got {other:?}"),
        }
        // Zweite ist die Delta-`b`.
        match root.alternatives[1].symbols {
            [Symbol::Terminal(TokenKind::Keyword("b"))] => {}
            other => panic!("expected delta 'b' alternative second, got {other:?}"),
        }
    }

    #[test]
    fn compose_multiple_deltas_apply_in_sequence() {
        // Zwei Deltas: erste fuegt Alternative "b" hinzu, zweite "d".
        static EXTRA_2: &[Alternative] = &[alt_with(&[Symbol::Terminal(TokenKind::Keyword("d"))])];
        static EXT_2: &[AlternativeExtension] = &[AlternativeExtension {
            target: ProductionId(0),
            extra_alternatives: EXTRA_2,
        }];
        static DELTA_2: GrammarDelta = GrammarDelta {
            name: "second",
            additional_productions: &[],
            alternative_extensions: EXT_2,
        };
        let composed = compose(&BASE, &[&DELTA, &DELTA_2]);
        let root = composed
            .productions
            .iter()
            .find(|p| p.name == "root")
            .expect("root");
        assert_eq!(root.alternatives.len(), 3, "base + 2 deltas = 3 alts");
    }

    #[test]
    fn compose_preserves_start_production() {
        let composed = compose(&BASE, &[&DELTA]);
        assert_eq!(composed.start, BASE.start);
    }

    #[test]
    fn compose_extension_with_unknown_target_is_silently_skipped() {
        // Vendor-Delta mit Tippfehler in target — sollte nicht crashen.
        static EXTRA: &[Alternative] = &[alt_with(&[Symbol::Terminal(TokenKind::Keyword("x"))])];
        static EXT: &[AlternativeExtension] = &[AlternativeExtension {
            target: ProductionId(9999), // existiert nicht
            extra_alternatives: EXTRA,
        }];
        static BAD_DELTA: GrammarDelta = GrammarDelta {
            name: "bad",
            additional_productions: &[],
            alternative_extensions: EXT,
        };
        let composed = compose(&BASE, &[&BAD_DELTA]);
        assert_eq!(composed.production_count(), 1);
    }
}
