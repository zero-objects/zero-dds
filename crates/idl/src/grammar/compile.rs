// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar-Compile-Pass: desugart `Symbol::Repeat` und `Symbol::Choice`
//! zu rekursiven Hilfs-Productions.
//!
//! Die Earley-Engine arbeitet nur auf reinen kontextfreien Grammatiken
//! (Terminal + Nonterminal). EBNF-Konstrukte wie `<X>+`, `<X>*`,
//! `[<X>]` und Inline-`(<A> | <B>)` werden hier in Hilfs-Productions
//! umgeformt:
//!
//! - `Symbol::Repeat(OneOrMore, inner)` →
//!   `_rep_N ::= inner | _rep_N inner`
//! - `Symbol::Repeat(ZeroOrMore, inner)` →
//!   `_rep_N ::= ε | _rep_N inner`
//! - `Symbol::Repeat(Optional, inner)` →
//!   `_rep_N ::= ε | inner`
//! - `Symbol::Choice(branches)` →
//!   `_chc_N ::= branch_0 | branch_1 | ...`
//!
//! Das Ergebnis [`CompiledGrammar`] besteht aus den Original-Productions
//! plus den synthetisierten. Die ID-Reihenfolge ist `[original, ...,
//! synthesized, ...]`. Original-Productions behalten ihre IDs, synthesized
//! Productions bekommen neue IDs ab `original.len()`.
//!
//! ## Implementierungs-Detail: `Box::leak`
//!
//! `Production` und `Alternative` haben `&'static [Symbol]`-Slices. Fuer
//! synthesized Productions reichen diese in dynamisch allokierte Vecs. Wir
//! nutzen [`Box::leak`], um sie zu `&'static`-Slices zu machen. Das
//! bedeutet einen kleinen, einmaligen Memory-Leak pro
//! [`compile`]-Aufruf — bei realistischen IDL-Grammars im niedrigen
//! kB-Bereich. Fuer ein CLI-Tool oder Build-Pipeline-Step ist das
//! akzeptabel; in Library-Settings sollte `compile()` einmalig pro
//! Programm-Lebensdauer aufgerufen werden.

use super::{
    AltRef as _AltRef, Alternative, Grammar, IdlVersion, Production, ProductionId, RepeatKind,
    SpecRef, Symbol, TokenKind, TokenRule,
};

/// Compiliert eine [`Grammar`] zu einer EBNF-freien Form.
///
/// Wenn die Grammar keine `Symbol::Repeat`/`Symbol::Choice`-Symbole
/// enthaelt, ist das Ergebnis strukturell identisch zur Input-Grammar
/// (nur in einer owned [`CompiledGrammar`]-Form).
#[must_use]
pub fn compile(grammar: &Grammar) -> CompiledGrammar {
    CompileState::new(grammar).compile()
}

/// EBNF-freie Form einer [`Grammar`]. Wird von [`compile`] erzeugt; die
/// Engine arbeitet primaer auf dieser Form.
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    /// Menschenlesbarer Name (uebernommen aus der Quelle).
    pub name: &'static str,
    /// IDL-Version (uebernommen).
    pub version: IdlVersion,
    /// Productions in der Reihenfolge `[original_0, ..., original_n,
    /// synthesized_0, ..., synthesized_m]`. Original behalten ihre IDs.
    pub productions: Vec<Production>,
    /// Start-Production (uebernommen).
    pub start: ProductionId,
    /// Token-Regeln (uebernommen).
    pub token_rules: &'static [TokenRule],
}

impl CompiledGrammar {
    /// Lookup einer Production anhand ihrer ID.
    ///
    /// Versucht erst den schnellen positional-Lookup (gilt fuer
    /// kompilierte Base-Grammars, deren Productions in ID-Reihenfolge
    /// liegen). Fallback: lineare Suche, falls die positional-Annahme
    /// nicht haelt — das ist nach Delta-Komposition (T6.5) der Fall,
    /// weil Vendor-Productions mit hohen IDs (>= 1000) ab ihrem
    /// Anhaenge-Index liegen.
    #[must_use]
    pub fn production(&self, id: ProductionId) -> Option<&Production> {
        if let Some(p) = self.productions.get(id.0 as usize) {
            if p.id == id {
                return Some(p);
            }
        }
        self.productions.iter().find(|p| p.id == id)
    }

    /// Start-Production-Lookup.
    #[must_use]
    pub fn start_production(&self) -> Option<&Production> {
        self.production(self.start)
    }

    /// Anzahl Productions (original + synthesized).
    #[must_use]
    pub fn production_count(&self) -> usize {
        self.productions.len()
    }

    /// Iteriert ueber alle Productions.
    pub fn productions_iter(&self) -> impl Iterator<Item = &Production> {
        self.productions.iter()
    }
}

impl super::GrammarLike for CompiledGrammar {
    fn production(&self, id: ProductionId) -> Option<&Production> {
        self.production(id)
    }
    fn start(&self) -> ProductionId {
        self.start
    }
    fn productions_slice(&self) -> &[Production] {
        &self.productions
    }
}

// ---------------------------------------------------------------------------
// Compile-Implementierung
// ---------------------------------------------------------------------------

struct CompileState<'g> {
    base: &'g Grammar,
    productions: Vec<Production>,
    next_id: u32,
}

impl<'g> CompileState<'g> {
    fn new(grammar: &'g Grammar) -> Self {
        Self {
            base: grammar,
            productions: Vec::with_capacity(grammar.productions.len() * 2),
            next_id: grammar.productions.len() as u32,
        }
    }

    fn compile(mut self) -> CompiledGrammar {
        // Zuerst alle Original-Productions mit transformierten Symbolen.
        let mut transformed: Vec<Production> = Vec::with_capacity(self.base.productions.len());
        for prod in self.base.productions {
            let new_alts: Vec<Alternative> = prod
                .alternatives
                .iter()
                .map(|alt| Alternative {
                    name: alt.name,
                    symbols: leak_symbols(self.transform_symbols(alt.symbols)),
                    note: alt.note,
                })
                .collect();
            transformed.push(Production {
                id: prod.id,
                name: prod.name,
                spec_ref: prod.spec_ref,
                alternatives: leak_alternatives(new_alts),
                ast_hint: prod.ast_hint,
            });
        }

        // Original kommen zuerst, dann synthesized (ueber `productions` waehrend
        // transform_symbols befuellt).
        let mut all = transformed;
        all.append(&mut self.productions);

        CompiledGrammar {
            name: self.base.name,
            version: self.base.version,
            productions: all,
            start: self.base.start,
            token_rules: self.base.token_rules,
        }
    }

    fn transform_symbols(&mut self, symbols: &[Symbol]) -> Vec<Symbol> {
        let mut out = Vec::with_capacity(symbols.len());
        for sym in symbols {
            out.push(self.transform_symbol(sym));
        }
        out
    }

    fn transform_symbol(&mut self, sym: &Symbol) -> Symbol {
        match sym {
            Symbol::Terminal(_) | Symbol::Nonterminal(_) => *sym,
            Symbol::Repeat(kind, inner) => {
                let inner_transformed = self.transform_symbols(inner);
                let new_id = self.alloc_id();
                let synth = self.synth_repeat(new_id, *kind, inner_transformed);
                self.productions.push(synth);
                Symbol::Nonterminal(new_id)
            }
            Symbol::Choice(branches) => {
                let mut alts: Vec<Alternative> = Vec::with_capacity(branches.len());
                for branch in *branches {
                    let bt = self.transform_symbols(branch);
                    alts.push(Alternative {
                        name: None,
                        symbols: leak_symbols(bt),
                        note: None,
                    });
                }
                let new_id = self.alloc_id();
                self.productions.push(Production {
                    id: new_id,
                    name: leak_name(format!("_chc_{}", new_id.0)),
                    spec_ref: SYNTHESIZED_SPEC,
                    alternatives: leak_alternatives(alts),
                    ast_hint: None,
                });
                Symbol::Nonterminal(new_id)
            }
        }
    }

    fn alloc_id(&mut self) -> ProductionId {
        let id = ProductionId(self.next_id);
        self.next_id += 1;
        id
    }

    fn synth_repeat(
        &mut self,
        id: ProductionId,
        kind: RepeatKind,
        inner: Vec<Symbol>,
    ) -> Production {
        let inner_static: &'static [Symbol] = leak_symbols(inner);
        let alts: Vec<Alternative> = match kind {
            RepeatKind::ZeroOrMore => vec![
                Alternative {
                    name: Some("empty"),
                    symbols: &[],
                    note: None,
                },
                // _rep ::= _rep inner...
                Alternative {
                    name: Some("cons"),
                    symbols: leak_symbols({
                        let mut v = vec![Symbol::Nonterminal(id)];
                        v.extend_from_slice(inner_static);
                        v
                    }),
                    note: None,
                },
            ],
            RepeatKind::OneOrMore => vec![
                Alternative {
                    name: Some("single"),
                    symbols: inner_static,
                    note: None,
                },
                Alternative {
                    name: Some("cons"),
                    symbols: leak_symbols({
                        let mut v = vec![Symbol::Nonterminal(id)];
                        v.extend_from_slice(inner_static);
                        v
                    }),
                    note: None,
                },
            ],
            RepeatKind::Optional => vec![
                Alternative {
                    name: Some("empty"),
                    symbols: &[],
                    note: None,
                },
                Alternative {
                    name: Some("present"),
                    symbols: inner_static,
                    note: None,
                },
            ],
        };
        Production {
            id,
            name: leak_name(format!("_rep_{}", id.0)),
            spec_ref: SYNTHESIZED_SPEC,
            alternatives: leak_alternatives(alts),
            ast_hint: None,
        }
    }
}

const SYNTHESIZED_SPEC: SpecRef = SpecRef {
    doc: "synthesized",
    section: "compile",
};

fn leak_symbols(syms: Vec<Symbol>) -> &'static [Symbol] {
    Box::leak(syms.into_boxed_slice())
}

fn leak_alternatives(alts: Vec<Alternative>) -> &'static [Alternative] {
    Box::leak(alts.into_boxed_slice())
}

fn leak_name(name: String) -> &'static str {
    Box::leak(name.into_boxed_str())
}

// `AltRef` re-export-stub (parallel zum Pattern in idl42.rs).
#[allow(dead_code)]
const _ALT_REF_TYPE: Option<_AltRef> = None;

// Unused but keeps Symbol::Repeat/Choice/Nonterminal/etc. matchable.
#[allow(dead_code)]
const _TOKEN_KIND_UNUSED: Option<TokenKind> = None;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::grammar::{Alternative, Grammar};

    fn dummy_grammar(productions: &'static [Production]) -> Grammar {
        Grammar {
            name: "test",
            version: IdlVersion::V4_2,
            productions,
            start: ProductionId(0),
            token_rules: &[],
        }
    }

    #[test]
    fn compile_grammar_without_repeat_is_structural_copy() {
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = compile(&dummy_grammar(PRODS));
        assert_eq!(compiled.production_count(), 1);
        assert_eq!(compiled.start, ProductionId(0));
    }

    #[test]
    fn compile_repeat_one_or_more_creates_synth_production() {
        // A ::= <X>+   →   A ::= _rep_1 ;  _rep_1 ::= X | _rep_1 X
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Repeat(
                    RepeatKind::OneOrMore,
                    &[Symbol::Terminal(TokenKind::Keyword("x"))],
                )],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = compile(&dummy_grammar(PRODS));
        assert_eq!(compiled.production_count(), 2);
        // Original wurde transformiert: erstes Symbol jetzt Nonterminal(1).
        let a = compiled.production(ProductionId(0)).expect("a");
        assert_eq!(a.alternatives[0].symbols.len(), 1);
        assert_eq!(
            a.alternatives[0].symbols[0],
            Symbol::Nonterminal(ProductionId(1))
        );
        // Synth-Production hat 2 Alternativen (single + cons).
        let rep = compiled.production(ProductionId(1)).expect("rep");
        assert_eq!(rep.alternatives.len(), 2);
        assert_eq!(rep.name, "_rep_1");
    }

    #[test]
    fn compile_repeat_zero_or_more_creates_epsilon_alt() {
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Repeat(
                    RepeatKind::ZeroOrMore,
                    &[Symbol::Terminal(TokenKind::Keyword("x"))],
                )],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = compile(&dummy_grammar(PRODS));
        let rep = compiled.production(ProductionId(1)).expect("rep");
        assert_eq!(rep.alternatives.len(), 2);
        // Erste Alt ist empty (epsilon).
        assert_eq!(rep.alternatives[0].symbols, &[] as &[Symbol]);
        assert_eq!(rep.alternatives[0].name, Some("empty"));
    }

    #[test]
    fn compile_repeat_optional_creates_two_alts() {
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Repeat(
                    RepeatKind::Optional,
                    &[Symbol::Terminal(TokenKind::Keyword("x"))],
                )],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = compile(&dummy_grammar(PRODS));
        let rep = compiled.production(ProductionId(1)).expect("rep");
        assert_eq!(rep.alternatives.len(), 2);
        assert_eq!(rep.alternatives[0].name, Some("empty"));
        assert_eq!(rep.alternatives[1].name, Some("present"));
    }

    #[test]
    fn compile_choice_creates_one_alt_per_branch() {
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Choice(&[
                    &[Symbol::Terminal(TokenKind::Keyword("x"))],
                    &[Symbol::Terminal(TokenKind::Keyword("y"))],
                ])],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = compile(&dummy_grammar(PRODS));
        let chc = compiled.production(ProductionId(1)).expect("chc");
        assert_eq!(chc.alternatives.len(), 2);
        assert_eq!(chc.name, "_chc_1");
    }

    #[test]
    fn engine_recognizes_grammar_with_one_or_more_repeat() {
        // A ::= "x"+    nimmt ein oder mehr "x".
        use crate::engine::Engine;
        use crate::lexer::Token;
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Repeat(
                    RepeatKind::OneOrMore,
                    &[Symbol::Terminal(TokenKind::Keyword("x"))],
                )],
                note: None,
            }],
            ast_hint: None,
        }];
        let g = dummy_grammar(PRODS);
        let engine = Engine::new(&g);

        // 1 x → ok
        let one = [Token::synthetic(TokenKind::Keyword("x"))];
        assert!(engine.recognize(&one).is_ok());

        // 5 x → ok
        let five = [Token::synthetic(TokenKind::Keyword("x")); 5];
        assert!(engine.recognize(&five).is_ok());

        // 0 x → reject (OneOrMore braucht mindestens eins)
        let zero: [Token<'static>; 0] = [];
        assert!(engine.recognize(&zero).is_err());
    }

    #[test]
    fn engine_recognizes_grammar_with_zero_or_more_repeat() {
        // A ::= "x"*    nimmt null oder mehr "x".
        use crate::engine::Engine;
        use crate::lexer::Token;
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Repeat(
                    RepeatKind::ZeroOrMore,
                    &[Symbol::Terminal(TokenKind::Keyword("x"))],
                )],
                note: None,
            }],
            ast_hint: None,
        }];
        let g = dummy_grammar(PRODS);
        let engine = Engine::new(&g);

        // 0 x → ok (Empty erlaubt)
        let zero: [Token<'static>; 0] = [];
        assert!(engine.recognize(&zero).is_ok());

        // 3 x → ok
        let three = [Token::synthetic(TokenKind::Keyword("x")); 3];
        assert!(engine.recognize(&three).is_ok());
    }

    #[test]
    fn engine_recognizes_grammar_with_optional_repeat() {
        // A ::= ["x"] "y"   optional x, dann y.
        use crate::engine::Engine;
        use crate::lexer::Token;
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[
                    Symbol::Repeat(
                        RepeatKind::Optional,
                        &[Symbol::Terminal(TokenKind::Keyword("x"))],
                    ),
                    Symbol::Terminal(TokenKind::Keyword("y")),
                ],
                note: None,
            }],
            ast_hint: None,
        }];
        let g = dummy_grammar(PRODS);
        let engine = Engine::new(&g);

        // "y" allein → ok (optional skip)
        let y_only = [Token::synthetic(TokenKind::Keyword("y"))];
        assert!(engine.recognize(&y_only).is_ok());

        // "x y" → ok
        let xy = [
            Token::synthetic(TokenKind::Keyword("x")),
            Token::synthetic(TokenKind::Keyword("y")),
        ];
        assert!(engine.recognize(&xy).is_ok());
    }

    #[test]
    fn engine_recognizes_grammar_with_choice() {
        // A ::= ("x" | "y") "z"
        use crate::engine::Engine;
        use crate::lexer::Token;
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[
                    Symbol::Choice(&[
                        &[Symbol::Terminal(TokenKind::Keyword("x"))],
                        &[Symbol::Terminal(TokenKind::Keyword("y"))],
                    ]),
                    Symbol::Terminal(TokenKind::Keyword("z")),
                ],
                note: None,
            }],
            ast_hint: None,
        }];
        let g = dummy_grammar(PRODS);
        let engine = Engine::new(&g);

        let xz = [
            Token::synthetic(TokenKind::Keyword("x")),
            Token::synthetic(TokenKind::Keyword("z")),
        ];
        assert!(engine.recognize(&xz).is_ok());

        let yz = [
            Token::synthetic(TokenKind::Keyword("y")),
            Token::synthetic(TokenKind::Keyword("z")),
        ];
        assert!(engine.recognize(&yz).is_ok());
    }

    #[test]
    fn compile_nested_repeat_creates_chained_synthetics() {
        // A ::= <(<X>+)>*   →   2 synth Productions
        const PRODS: &[Production] = &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: SpecRef {
                doc: "T",
                section: "0",
            },
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Repeat(
                    RepeatKind::ZeroOrMore,
                    &[Symbol::Repeat(
                        RepeatKind::OneOrMore,
                        &[Symbol::Terminal(TokenKind::Keyword("x"))],
                    )],
                )],
                note: None,
            }],
            ast_hint: None,
        }];
        let compiled = compile(&dummy_grammar(PRODS));
        // Original + 2 Synth.
        assert_eq!(compiled.production_count(), 3);
    }
}
