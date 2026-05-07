// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar-Datenmodell fuer den grammatik-getriebenen IDL-Parser.
//!
//! Grammatiken sind als `&'static`-Daten definiert (Compile-Zeit-Konstanten),
//! nicht als Code. Die Parse-Engine (`crate::engine`) traversiert die
//! Grammar-Daten, um Tokens in einen Concrete Syntax Tree zu ueberfuehren.
//!
//! Siehe RFC 0001 §5.1 fuer das Entwurfs-Rationale.
//!
//! Ein statischer Validator fuer Grammar-Daten lebt im Submodul
//! [`validate`] — siehe dort fuer die Liste der erkannten Probleme.
//!
//! ## Entwurfs-Invarianten
//!
//! - Keine Heap-Allokation fuer Grammar-Daten zur Laufzeit. Produktionen,
//!   Alternativen und Symbole sind `&'static [...]`-Slices im Binary-Segment.
//! - Jede [`Production`] traegt einen [`SpecRef`] — die exakte Section-Nummer
//!   in der zugrundeliegenden Spec (OMG IDL 4.2 §7.x). Dieser Anker wird von
//!   `tools/traceability` konsumiert und gehoert zur Audit-Evidenz
//!   (`docs/architecture/04_safety_by_architecture.md §4`).
//! - [`ProductionId`] ist ein newtype-wrapper um `u32` und wird zum Verweis
//!   zwischen Productions benutzt (Nonterminal-Referenzen).

use core::fmt;

pub mod compile;
pub mod compose;
pub mod deltas;
pub mod idl42;
pub mod toy;
pub mod validate;

/// Version der IDL-Spec, an die sich eine Grammar haelt.
///
/// Version-Deltas (Task 6.4, `grammar::deltas`) komponieren eine Basis-Grammar
/// mit Versions-spezifischen Abweichungen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdlVersion {
    /// Pre-OMG-4.0. Historisch relevant fuer Migration aus aelteren Codebasen.
    V3_5,
    /// Erste OMG-4.x-Fassung.
    V4_0,
    /// Zwischenrevision.
    V4_1,
    /// Aktueller Ziel-Standard fuer ZeroDDS. Default.
    V4_2,
}

impl Default for IdlVersion {
    fn default() -> Self {
        Self::V4_2
    }
}

/// Eindeutiger Identifier fuer eine Production innerhalb einer Grammar.
///
/// Newtype um `u32`. Indizes sind stabil innerhalb einer Grammar-Konstante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProductionId(pub u32);

impl ProductionId {
    /// Der rohe Index als `usize` — fuer Array-Lookups.
    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Eindeutiger Identifier fuer eine Token-Regel innerhalb einer Grammar.
///
/// Newtype um `u32`. Wird beim Extrahieren der Token-Regeln aus Terminals
/// vergeben (Task 2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenRuleId(pub u32);

/// Referenz auf eine Spec-Section.
///
/// Beispiel: `SpecRef { doc: "OMG IDL 4.2", section: "7.4.1.4.4.2" }` verweist
/// auf die `<struct_def>`-Production im IDL-4.2-Dokument.
///
/// `Display` rendert als `"OMG IDL 4.2 §7.4.1.4.4.2"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpecRef {
    /// Menschenlesbarer Dokument-Name (z.B. `"OMG IDL 4.2"`).
    pub doc: &'static str,
    /// Section-Pfad innerhalb des Dokuments (z.B. `"7.4.1.4.4.2"`).
    pub section: &'static str,
}

impl fmt::Display for SpecRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} §{}", self.doc, self.section)
    }
}

/// Klassifikation eines Terminal-Tokens.
///
/// Token-Ebene der Grammar: was der Lexer aus dem Source-Text erkennt.
/// Konkrete Lexer-Regeln werden aus den Terminals einer Grammar extrahiert
/// (Task 2.2). Der Lexer-Satz wird in Woche 2 ausgebaut; fuer Task 1.1 reicht
/// die Grundkategorisierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TokenKind {
    /// Reserviertes Schluesselwort (`struct`, `module`, `interface`, ...).
    Keyword(&'static str),
    /// Interpunktion oder Operator (`{`, `;`, `::`, `<`, ...).
    Punct(&'static str),
    /// Bezeichner (Identifier).
    Ident,
    /// Ganzzahlen-Literal.
    IntegerLiteral,
    /// Gleitkomma-Literal.
    FloatLiteral,
    /// String-Literal.
    StringLiteral,
    /// Char-Literal.
    CharLiteral,
    /// Boolean-Literal (`TRUE`, `FALSE`).
    BoolLiteral,
    /// Wide-Char-Literal (`L'x'`, IDL 4.2 §7.2.6.3).
    WideCharLiteral,
    /// Wide-String-Literal (`L"..."`, IDL 4.2 §7.2.6.5).
    WideStringLiteral,
    /// Fixed-Point-Literal (z.B. `1.234d`, IDL 4.2 §7.2.6.6).
    FixedPtLiteral,
    /// Anfang oder Ende der Eingabe (synthetisch vom Engine verwendet).
    EndOfInput,
}

/// Wiederholung innerhalb einer Production-Alternative.
///
/// Entspricht den EBNF-Metasymbolen:
/// - [`RepeatKind::ZeroOrMore`] — `{ X }*`
/// - [`RepeatKind::OneOrMore`] — `{ X }+`
/// - [`RepeatKind::Optional`] — `[ X ]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepeatKind {
    /// Null oder mehr Wiederholungen.
    ZeroOrMore,
    /// Eine oder mehr Wiederholungen.
    OneOrMore,
    /// Optional — null oder eine Wiederholung.
    Optional,
}

/// Element einer Alternative.
///
/// Rekursives Enum: Terminals (Tokens), Nonterminals (Verweise auf andere
/// Productions), Wiederholungen und Inline-Alternativen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    /// Terminal — vom Lexer erzeugter Token.
    Terminal(TokenKind),
    /// Nonterminal — Referenz auf eine andere Production.
    Nonterminal(ProductionId),
    /// Wiederholung einer Teilsequenz.
    Repeat(RepeatKind, &'static [Symbol]),
    /// Inline-Alternativen — mehrere Zweige an Ort und Stelle.
    Choice(&'static [&'static [Symbol]]),
}

impl Symbol {
    /// `true` wenn das Symbol ein Terminal ist (d.h. ein Lexer-Token).
    #[inline]
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Terminal(_))
    }

    /// `true` wenn das Symbol ein Nonterminal ist (d.h. ein Verweis auf eine
    /// andere Production).
    #[inline]
    #[must_use]
    pub const fn is_nonterminal(self) -> bool {
        matches!(self, Self::Nonterminal(_))
    }
}

/// Eine Alternative innerhalb einer Production.
///
/// Entspricht einem Zweig einer EBNF-Rechte-Seite, z.B. in
/// `<type_spec> ::= <simple_type_spec> | <template_type_spec>` sind die
/// beiden Nonterminals jeweils eine Alternative.
#[derive(Debug, Clone, Copy)]
pub struct Alternative {
    /// Optionaler Name der Alternative (z.B. `"prefixed"`, `"unqualified"`).
    /// Nützlich fuer AST-Builder-Dispatch (Task 5.2) und als Diagnostik-
    /// Anker in Validation-Reports.
    pub name: Option<&'static str>,
    /// Die Sequenz von Symbolen, die diese Alternative bilden.
    pub symbols: &'static [Symbol],
    /// Optionale Review-Notiz (z.B. Hinweis auf Vendor-Spezifika oder
    /// Unklarheiten in der Spec). Erscheint im Grammar-Validation-Report.
    pub note: Option<&'static str>,
}

/// Referenz auf eine spezifische Alternative einer Production.
///
/// Wird in Validation-Reports verwendet, um Issues exakt zu verorten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltRef {
    /// Index der Alternative innerhalb von `Production::alternatives`.
    pub index: usize,
    /// Kopie des optionalen Namens (siehe `Alternative::name`).
    pub name: Option<&'static str>,
}

/// Optionaler Hinweis fuer den AST-Builder, welche Builder-Funktion fuer
/// diese Production aufgerufen werden soll. Details werden in Woche 5
/// (Task 5.2) spezifiziert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AstHint {
    /// Trigger fuer den AST-Builder unter diesem symbolischen Namen.
    /// Der Builder dispatcht auf diesen Namen, nicht auf die Production-ID.
    Named(&'static str),
}

/// Eine Production — die linke Seite einer EBNF-Regel.
///
/// Beispiel:
///
/// ```rust,ignore
/// const PROD_MODULE: Production = Production {
///     id: ProductionId(1),
///     name: "module",
///     spec_ref: SpecRef { doc: "OMG IDL 4.2", section: "7.4.1.3" },
///     alternatives: &[/* ... */],
///     ast_hint: Some(AstHint::Named("Module")),
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Production {
    /// Eindeutige ID innerhalb der Grammar.
    pub id: ProductionId,
    /// Menschenlesbarer Name (entspricht dem EBNF-Nonterminal-Namen).
    pub name: &'static str,
    /// Verweis auf die Spec-Section, aus der diese Production stammt.
    pub spec_ref: SpecRef,
    /// Die Zweige der rechten Seite.
    pub alternatives: &'static [Alternative],
    /// Optionaler Builder-Hinweis.
    pub ast_hint: Option<AstHint>,
}

/// Token-Match-Regel fuer den Lexer.
///
/// Vorerst nur Struktur. Match-Logik wird in Woche 2 (Task 2.3) implementiert.
#[derive(Debug, Clone, Copy)]
pub struct TokenRule {
    /// ID der Regel innerhalb der Grammar.
    pub id: TokenRuleId,
    /// Welcher TokenKind wird erzeugt.
    pub kind: TokenKind,
    /// Match-Literal (fuer `Keyword` und `Punct`) oder Pattern-Name (fuer
    /// regex-artige Tokens wie Identifier). Pattern-Namen werden vom Lexer
    /// auf eine handgeschriebene Match-Funktion gemappt (Task 2.3).
    pub pattern: &'static str,
}

/// Eine Grammar — die komplette Beschreibung einer Sprach-Syntax.
///
/// Zusammengesetzt aus Productions (Nonterminals) und einer Menge von
/// Token-Regeln (Terminals). Start-Production wird per [`Grammar::start`]
/// referenziert.
#[derive(Debug, Clone, Copy)]
pub struct Grammar {
    /// Menschenlesbarer Name (z.B. `"IDL 4.2"`).
    pub name: &'static str,
    /// IDL-Version, an der sich die Grammar orientiert.
    pub version: IdlVersion,
    /// Die Produktions-Menge. Index `i` entspricht `ProductionId(i as u32)`.
    pub productions: &'static [Production],
    /// Die Start-Production — typischerweise `<specification>` bei IDL.
    pub start: ProductionId,
    /// Token-Regeln fuer den Lexer.
    pub token_rules: &'static [TokenRule],
}

/// Abstraktion ueber [`Grammar`] und [`compile::CompiledGrammar`] —
/// einheitlicher Lookup-Trait fuer den Recognizer.
pub trait GrammarLike {
    /// Sucht eine Production anhand ihrer ID.
    fn production(&self, id: ProductionId) -> Option<&Production>;
    /// Start-Production-ID.
    fn start(&self) -> ProductionId;
    /// Slice ueber alle Productions (in ID-Reihenfolge).
    fn productions_slice(&self) -> &[Production];
}

impl GrammarLike for Grammar {
    fn production(&self, id: ProductionId) -> Option<&Production> {
        // Productions sind nicht garantiert in ID-Reihenfolge im Slice
        // (Eintragungs-Reihenfolge in IDL_42.productions kann von der
        // numerischen ID-Reihenfolge abweichen, z.B. ID 100 wird nach
        // ID 116 eingetragen). Linearer Scan nach `id`.
        self.productions.iter().find(|p| p.id == id)
    }
    fn start(&self) -> ProductionId {
        self.start
    }
    fn productions_slice(&self) -> &[Production] {
        self.productions
    }
}

impl Grammar {
    /// Sucht eine Production anhand ihrer ID.
    ///
    /// Gibt `None` zurueck, wenn die ID nicht vorhanden ist.
    #[must_use]
    pub fn production(&self, id: ProductionId) -> Option<&Production> {
        self.productions.iter().find(|p| p.id == id)
    }

    /// Gibt die Start-Production zurueck.
    ///
    /// # Errors
    /// Liefert `None`, wenn `self.start` auf eine nicht existierende
    /// Production verweist — dann liegt ein Grammar-Konstruktionsfehler vor,
    /// der von [`crate::grammar::validate`] (Task 1.2) erkannt wird.
    #[must_use]
    pub fn start_production(&self) -> Option<&Production> {
        self.production(self.start)
    }

    /// Anzahl Productions.
    #[inline]
    #[must_use]
    pub fn production_count(&self) -> usize {
        self.productions.len()
    }

    /// Iteriert ueber alle Productions.
    pub fn productions_iter(&self) -> impl Iterator<Item = &Production> {
        self.productions.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal-Grammar fuer Tests: ein einziger nichtterminaler Zweig, der
    /// zwei Terminals akzeptiert (`module <Ident>`). Keine vollstaendige
    /// IDL-Grammar, nur Testdaten.
    const PROD_DUMMY_MODULE: Production = Production {
        id: ProductionId(0),
        name: "dummy_module",
        spec_ref: SpecRef {
            doc: "TEST",
            section: "0.0",
        },
        alternatives: &[Alternative {
            name: None,
            symbols: &[
                Symbol::Terminal(TokenKind::Keyword("module")),
                Symbol::Terminal(TokenKind::Ident),
            ],
            note: None,
        }],
        ast_hint: None,
    };

    const DUMMY_GRAMMAR: Grammar = Grammar {
        name: "dummy",
        version: IdlVersion::V4_2,
        productions: &[PROD_DUMMY_MODULE],
        start: ProductionId(0),
        token_rules: &[],
    };

    #[test]
    fn default_idl_version_is_v4_2() {
        assert_eq!(IdlVersion::default(), IdlVersion::V4_2);
    }

    #[test]
    fn production_id_converts_to_usize() {
        assert_eq!(ProductionId(42).as_usize(), 42);
    }

    #[test]
    fn spec_ref_displays_with_paragraph_sign() {
        let sref = SpecRef {
            doc: "OMG IDL 4.2",
            section: "7.4.1.4.4.2",
        };
        assert_eq!(format!("{sref}"), "OMG IDL 4.2 §7.4.1.4.4.2");
    }

    #[test]
    fn symbol_classifies_terminals_and_nonterminals() {
        let term = Symbol::Terminal(TokenKind::Ident);
        let nonterm = Symbol::Nonterminal(ProductionId(0));
        let rep = Symbol::Repeat(RepeatKind::ZeroOrMore, &[]);

        assert!(term.is_terminal());
        assert!(!term.is_nonterminal());

        assert!(nonterm.is_nonterminal());
        assert!(!nonterm.is_terminal());

        assert!(!rep.is_terminal());
        assert!(!rep.is_nonterminal());
    }

    #[test]
    fn grammar_looks_up_production_by_id() {
        let prod = DUMMY_GRAMMAR.production(ProductionId(0));
        assert!(prod.is_some());
        assert_eq!(prod.map(|p| p.name), Some("dummy_module"));
    }

    #[test]
    fn grammar_returns_none_for_out_of_range_production() {
        assert!(DUMMY_GRAMMAR.production(ProductionId(99)).is_none());
    }

    #[test]
    fn grammar_resolves_start_production() {
        let start = DUMMY_GRAMMAR.start_production();
        assert!(start.is_some());
        assert_eq!(start.map(|p| p.name), Some("dummy_module"));
    }

    #[test]
    fn grammar_with_invalid_start_returns_none() {
        const BROKEN: Grammar = Grammar {
            name: "broken",
            version: IdlVersion::V4_2,
            productions: &[],
            start: ProductionId(0),
            token_rules: &[],
        };
        assert!(BROKEN.start_production().is_none());
    }

    #[test]
    fn grammar_counts_and_iterates_productions() {
        assert_eq!(DUMMY_GRAMMAR.production_count(), 1);
        let names: Vec<&str> = DUMMY_GRAMMAR.productions_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["dummy_module"]);
    }
}
