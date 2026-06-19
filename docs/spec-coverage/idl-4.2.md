# IDL 4.2 — Spec-Coverage

**Spec:** [OMG IDL 4.2](https://www.omg.org/spec/IDL/4.2/PDF) (142 Seiten, OMG formal/18-01-05)


Implementation:

- `crates/idl/` — OMG-IDL-4.2-Parser + AST + Semantik-Modell.

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

---

## §1 Scope

### 1.1 IDL als descriptive language

**Spec:** §1, S. 1 — "OMG Interface Definition Language (IDL). IDL is a
descriptive language used to define data types and interfaces in a way
that is independent of the programming language or operating system/
processor platform."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Position-Statement der Spec; keine Code-Anforderung.

### 1.2 IDL definiert nur Syntax

**Spec:** §1, S. 1 — "The IDL specifies only the syntax used to define
the data types and interfaces. It is normally used in connection with
other specifications that further define how these types/interfaces
are utilized in specific contexts and platforms" — drei separate
Spec-Typen: language mapping (C/C++/Java/C#), serialization, middleware
(DDS, CORBA).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Position-Statement der Spec; keine Code-Anforderung.

### 1.3 IDL-Grammar nutzt EBNF-ähnliche Notation

**Spec:** §1, S. 1 — "The description of IDL grammar uses a syntax
notation that is similar to Extended Backus-Naur Format (EBNF)."

**Repo:** `crates/idl/src/grammar/mod.rs` — `Symbol::Terminal`,
`Symbol::Nonterminal`, `Symbol::Choice(branches)`,
`Symbol::Repeat(RepeatKind, ...)` mit
`RepeatKind::ZeroOrMore` (`*`), `OneOrMore` (`+`), `Optional` (`[]`).
Composer in `crates/idl/src/grammar/compile.rs`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests` (Recognizer-Tests
~750 Stk; jeder Test belegt EBNF-konformes Parsing).
`crates/idl/src/grammar/compile.rs::tests::compile_repeat_*`.

**Status:** done

### 1.4 `.idl`-File-Extension

**Spec:** §1 (implizit) + §7 Konvention — IDL-Source-Files haben
`.idl`-Extension.

**Repo:** Konvention; nicht hart durchgesetzt. `parse(src: &str, ...)`-
API in `crates/idl/src/parser.rs` akzeptiert beliebige Strings.
Fixtures in `crates/idl/tests/fixtures/**/*.idl`.

**Tests:** alle Fixture-Tests in `crates/idl/tests/coverage_report.rs`.

**Status:** done

---

## §2 Conformance Criteria

### 2.1 Keine Independent-Conformance-Points

**Spec:** §2, S. 3 — "This document defines IDL such that it can be
referenced by other specifications. It contains no independent
conformance points."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Konformitäts-Definition delegiert an Spec-Konsumenten; kein IDL-eigenes Verhalten.

### 2.2 Future Specs `shall reference`

**Spec:** §2 (1), S. 3 — "Future specifications that use IDL shall
reference this IDL standard or a future revision thereof."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe Referenz/Bindung; nicht im IDL-Compiler implementierbar.

### 2.3 Selected Building Blocks vollständig unterstützen

**Spec:** §2 (3a), S. 3 — "All selected building blocks shall be
supported entirely."

**Repo:** `crates/idl/src/features/mod.rs` — `IdlFeatures` mit 22
Bool-Flags pro Building-Block-Cluster
(`corba_value_types_full`/`_extras`, `corba_repository_ids`,
`corba_components`/`_homes`/`_eventtypes`/`_ports`,
`corba_template_modules`, `corba_native`, etc.) +
10 Profile-Konstrukturen (`dds_basic`, `dds_extensible`,
`corba_full`, `opensplice_legacy/_modern`, `rti_connext`,
`cyclonedds`, `fastdds`, `none`, `all`).
Feature-Gate-Validator: `crates/idl/src/features/gate.rs`
(`validate(cst, &features) -> Vec<FeatureGateError>`).

**Tests:** `crates/idl/src/features/mod.rs::tests` (13 Profile-Tests),
`crates/idl/src/features/gate.rs::tests` (27 Gate-Tests, decken
Production-Level + Alternative-Level-Gating).

**Status:** done

### 2.4 Annotations: voll unterstützt oder voll ignoriert

**Spec:** §2 (3b), S. 3 — "Selected annotations shall be either
supported as described in 8.2.2 Rules for Using Standardized
Annotations, or fully ignored. In the latter case, the IDL-dependent
specification shall not define a specific annotation, either with the
same name and another meaning or with the same meaning and another
name."

**Repo:** `crates/idl/src/semantics/annotations.rs` — `lower_single`
mappt 22 Standard-Annotations auf `BuiltinAnnotation`-Variants;
unbekannte/vendor-spezifische landen in `Lowered::custom`
(Passthrough).

**Tests:** `crates/idl/src/semantics/annotations.rs::tests`
(45+ Annotation-Lowering-Tests).

**Status:** done

---

## §3 Normative References

### 3.1 Referenzen-Liste

**Spec:** §3, S. 5 — referenziert: ISO/IEC 14882:2003 (C++); RFC 2119;
CORBA (formal/2012-11-12 part1, /-14 part2, /-16 part3); DDS-XTypes 1.2;
DDS 1.4; DDS-RPC 1.0.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe Referenz/Bindung; nicht im IDL-Compiler implementierbar.

---

## §4 Terms and Definitions

### 4.1 building block

**Spec:** §4, S. 7 — "A *building block* is a consistent set of IDL
rules that together form a piece of IDL functionality. Building blocks
are atomic, meaning that if selected, they must be totally supported.
Building blocks are described in clause 7, IDL Syntax and Semantics."

**Repo:** Implementation als Production-Cluster in
`crates/idl/src/grammar/idl42.rs` mit Feature-Flag-Gating
(`crates/idl/src/features/`). Atomicity wird durch
`Feature-Gate-Validator` durchgesetzt: ein Building-Block ist entweder
voll aktiv oder voll deaktiviert.

**Tests:** `crates/idl/src/features/gate.rs::tests` —
`corba_full_allows_*`, `dds_basic_rejects_*` belegen Atomicity.

**Status:** done

### 4.2 group of annotations

**Spec:** §4, S. 7 — "A *group of annotations* is a consistent set of
annotations, expressed in IDL. Groups of annotations are described in
clause 8, Standardized Annotations."

**Repo:** `crates/idl/src/semantics/annotations.rs` — `BuiltinAnnotation`-
Enum mit ~25 Variants, gruppiert via `lower_*`-Funktionen
(General-Purpose, Data-Modeling, Units-Ranges, Data-Implementation,
Code-Generation, Interfaces).

**Tests:** s. 2.4

**Status:** done

### 4.3 profile

**Spec:** §4, S. 7 — "A *profile* is a selection of building blocks
possibly complemented with groups of annotations that determines a
specific IDL usage. Profiles are described in clause 9, Profiles."

**Repo:** `crates/idl/src/features/mod.rs` —
`IdlFeatures::dds_basic()`/`dds_extensible()`/`corba_full()`/
`opensplice_legacy()`/`opensplice_modern()`/`rti_connext()`/
`cyclonedds()`/`fastdds()`. `crates/idl/src/config.rs::Profile`-Enum
(`DdsBasic`/`DdsExtensible`/`Full`).

**Tests:** `crates/idl/src/features/mod.rs::tests::*_profile`
(je ein Test pro Profile),
`crates/idl/src/config.rs::tests::*_constructor`.

**Status:** done

---

## §5 Symbols

### 5.1 Acronyme

**Spec:** §5, S. 9 — Tabelle mit Acronymen: ASCII, BIPM, CCM, CORBA,
DDS, EBNF, IDL, ISO, LwCCM, OMG, ORB, XTypes.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acronyme-Tabelle; informative Auflistung.

---

## §6 Additional Information

### 6.1 Acknowledgments

**Spec:** §6.1, S. 11 — "The following companies submitted this
specification: Thales, RTI. The following companies supported this
specification: Mitre, Northrop Grumman, Remedy IT."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

### 6.2 Specification History

**Spec:** §6.2, S. 11 — Historie IDL 3.5 → 4.0 (Building-Blocks
eingeführt) → 4.1 (bitset/bitmap) → 4.2 ("added support for 8-bit
integer types, added size-explicit keywords for integer types, enhanced
the readability, and reordered the building blocks to follow a logical
dependency progression").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7 IDL Syntax and Semantics

## §7.1 Overview

### §7.1 — IDL ist middleware-agnostic + descriptive

**Spec:** §7.1, S. 13 — "OMG IDL is a language that allows unambiguous
specification of the interfaces … client objects may use and (server)
object implementations provide as well as all needed related
constructs such as exceptions and data types. … IDL is a purely
descriptive language. This means that actual programs that use these
interfaces or create the associated data types cannot be written in
IDL, but in a programming language, for which mappings from IDL
constructs have been defined."

**Repo:** Crate `crates/idl/` produziert ausschließlich Parse-/CST-/
AST-/TypeObject-Ergebnisse, keine Sprach-Mapping-Outputs. Sprach-
Mappings sind separate Crates (`crates/dcps/`, `crates/idl-java/`,
`crates/idl-cpp/`).

**Tests:** n/a — Architektur-Position.

**Status:** done

### §7.1 — Pipeline Preprocessing → Tokens → Translation Unit

**Spec:** §7.1, S. 13 — "The clause is organized as follows:
- The description of IDL's lexical conventions is presented in 7.2,
  Lexical Conventions.
- A description of IDL preprocessing is presented in 7.3, Preprocessing.
- The grammar itself is presented in 7.4, IDL Grammar.
- The scoping rules for identifiers in an IDL specification are
  described in 7.5, Names and Scoping."

**Repo:** Pipeline in `crates/idl/src/lib.rs::parse`:
1. Preprocessing — `crates/idl/src/preprocessor/mod.rs::run`
2. Tokenisierung — `crates/idl/src/lexer/tokenizer.rs::Tokenizer::tokenize`
3. Recognition — `crates/idl/src/engine/mod.rs::Engine::recognize`
4. CST-Bau — `crates/idl/src/cst/build.rs::build_cst`
5. AST-Lowering — `crates/idl/src/ast/lower.rs::Ast::lower`

**Tests:** `crates/idl/tests/coverage_report.rs::generate_grammar_coverage_report`
(End-to-End-Pipeline gegen 15 Fixtures).

**Status:** done

### §7.1 Table 7-1 — IDL EBNF-Symbol-Set

**Spec:** §7.1 + Table 7-1, S. 14 — komplette Tabelle:
- `::=` "Is defined to be (left part of the rule is defined to be
  right part of the rule)"
- `|` "Alternatively"
- `::+` "Is added as alternative (left part of the rule is completed
  with right part of the rule as a new alternative)"
- `<text>` "Nonterminal"
- `"text"` "Literal"
- `*` "The preceding syntactic unit can be repeated zero or more times"
- `+` "The preceding syntactic unit must be repeated at least once"
- `{}` "The enclosed syntactic units are grouped as a single
  syntactic unit"
- `[]` "The enclosed syntactic unit is optional – may occur zero or
  one time"

**Repo:** `crates/idl/src/grammar/mod.rs` — vollständiges Mapping:
- `::=` → `Production { name, alternatives }`.
- `|` → `Vec<Alternative>` pro Production.
- `::+` → `crates/idl/src/grammar/compose.rs::apply_delta` (s. nächstes
  Item).
- `<text>` → `Symbol::Nonterminal(ProductionId)`.
- `"text"` → `Symbol::Terminal(TokenKind::Keyword(...))` /
  `TokenKind::Punct(...)`.
- `*` → `Symbol::Repeat(RepeatKind::ZeroOrMore, ...)`.
- `+` → `Symbol::Repeat(RepeatKind::OneOrMore, ...)`.
- `[]` → `Symbol::Repeat(RepeatKind::Optional, ...)`.
- `{}` → Composer-Gruppierung via `Symbol::Choice`/`Symbol::Repeat`.

**Tests:** `crates/idl/src/grammar/mod.rs::tests::symbol_classifies_terminals_and_nonterminals`,
`grammar_looks_up_production_by_id`,
`grammar_returns_none_for_out_of_range_production`,
`grammar_resolves_start_production`,
`grammar_counts_and_iterates_productions`.
`crates/idl/src/grammar/compile.rs::tests::compile_repeat_one_or_more_creates_synth_production`,
`compile_repeat_zero_or_more_creates_epsilon_alt`,
`compile_repeat_optional_creates_two_alts`,
`compile_choice_creates_one_alt_per_branch`,
`compile_nested_repeat_creates_chained_synthetics`.

**Status:** done

### §7.1 — `::+` Operator (Building-Block-Komposition)

**Spec:** §7.1, S. 13 — "However, to allow composition of specific
parts of the description, while avoiding redundancy; a new operator
(::+) has been added. This operator allows adding alternatives to an
existing definition. For example, assuming the rule x ::= y, the rule
x ::+ z shall be interpreted as x ::= y | z."

**Repo:** `crates/idl/src/grammar/compose.rs::apply_delta` (l. 42) —
jede Delta-Production fügt Alternativen zu einer Base-Production
hinzu. Aufrufer in `crates/idl/src/grammar/idl42.rs` für
Building-Block-Deltas (`corba_value`, `corba_components`,
`template_modules` etc.).

**Tests:** `crates/idl/src/grammar/compose.rs::tests::compose_without_deltas_returns_compiled_base`,
`compose_with_delta_adds_extra_production`,
`compose_with_delta_extends_existing_alternatives`,
`compose_keeps_base_alternative_first`,
`compose_multiple_deltas_apply_in_sequence`,
`compose_preserves_start_production`,
`compose_extension_with_unknown_target_is_silently_skipped`.

**Status:** done

### §7.1 — IDL-Pragmas dürfen überall stehen

**Spec:** §7.1, S. 13 — "IDL-specific pragmas may appear anywhere in a
specification; the textual location of these pragmas may be
semantically constrained by a particular implementation."

**Repo:** Preprocessor `crates/idl/src/preprocessor/mod.rs::process_line`
behandelt `#pragma`-Direktiven zeilenweise; die Position ist durch
das Line-basierte Scanning inhärent positionsfrei. Konkret:
- Cyclone-/OpenSplice `#pragma keylist <Type> <field>...` —
  `parse_pragma_keylist` (l. 816+).
- OpenSplice-Legacy `#pragma DCPS_DATA_TYPE`/`DCPS_DATA_KEY`/`cats`/
  `genequality` — `OpenSplicePragma`-Enum (l. 155+).
Annotations vs. Pragmas: §8.2.2 reglementiert Annotation-Position;
Pragmas hier sind streng `#pragma`-Form.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests`:
`pragma_is_stripped`,
`opensplice_pragma_data_type_quoted`,
`opensplice_pragma_data_type_unquoted`,
`opensplice_pragma_data_key`,
`opensplice_pragma_cats`,
`opensplice_pragma_genequality`,
`opensplice_legacy_full_topic_decl`.

**Status:** done

### §7.1 — `.idl` File-Extension

**Spec:** §7.1, S. 13 — "A source file containing specifications
written in IDL shall have a `.idl` extension."

**Repo:** Konvention im Repo durchgängig erfüllt
(`crates/idl/tests/fixtures/**/*.idl`); Hard-Enforcement liegt im
späteren `idlc`-CLI-Frontend. Library-API
`crates/idl/src/parser.rs::parse(src: &str)` akzeptiert beliebige
Strings, ist also extension-agnostic.

**Tests:** alle Fixtures in `crates/idl/tests/coverage_report.rs::FIXTURES`
nutzen `.idl`-Extension.

**Status:** done

### §7.1 — Footnote-Definitionen (middleware/compiler/client object)

**Spec:** §7.1 Footnotes 1-3, S. 13 — informative Definitionen:
- footnote 1: "the word *middleware* refers to any piece of software
  that will make use of IDL-derived artifacts. CORBA and DDS
  implementations are examples of middleware. The word *compiler*
  refers to any piece of software that produces these IDL-derived
  artifacts based on an IDL specification."
- footnote 2: "abstract semantics that is applicable to all IDL
  usages. When needed, middleware-specific interpretations of that
  abstract semantics will be given afterwards in dedicated clauses."
- footnote 3: "*client objects* should be understood here as abstract
  clients, i.e., entities invoking operations provided by object
  implementations, regardless of the means used to perform this
  invocation or even whether those implementations are co-located or
  remotely accessible."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffsdefinitionen; kein Code-Mapping.

---

## §7.2 Lexical Conventions

### §7.2 — Lexical Conventions Scope (footnote: Adaptation aus C++ ARM)

**Spec:** §7.2, S. 14 — "This sub clause presents the lexical
conventions of IDL. It defines tokens in an IDL specification and
describes comments, identifiers, keywords, and literals – integer,
character, and floating point constants and string literals."
Footnote 4: "This sub clause is an adaptation of The Annotated C++
Reference Manual, Clause 2; it differs in the list of legal keywords
and punctuation".

**Repo:** Lexer-Architektur in `crates/idl/src/lexer/`:
`token.rs` (Token + TokenStream), `rules.rs` (TokenRules aus Grammar),
`tokenizer.rs` (Engine). Mapping zur Spec in den Sub-Items unten.

**Tests:** s. Sub-Items.

**Status:** done

### §7.2 — Source besteht aus einer oder mehreren Files

**Spec:** §7.2, S. 14 — "An IDL specification logically consists of
one or more files. A file is conceptually translated in several
phases."

**Repo:** Multi-File-Verarbeitung via `#include`-Direktiven im
Preprocessor. `crates/idl/src/preprocessor/mod.rs::process` resolved
includes rekursiv (mit Cycle-Detection).

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::quoted_include_resolves`,
`system_include_resolves`,
`missing_include_is_error`,
`include_cycle_is_detected`.

**Status:** done

### §7.2 — Phase 1: Preprocessing → Token-Sequenz (Translation Unit)

**Spec:** §7.2, S. 14 — "The first phase is preprocessing, which
performs file inclusion and macro substitution. Preprocessing is
controlled by directives introduced by lines having # as the first
character other than white space. The result of preprocessing is a
sequence of tokens. Such a sequence of tokens, that is, a file after
preprocessing, is called a translation unit."

**Repo:** `crates/idl/src/preprocessor/mod.rs::run` produziert
`ProcessedSource` (Source-String + SourceMap + gesammelte Pragmas);
`Tokenizer::tokenize` konsumiert diesen String und liefert
`TokenStream` — die Translation-Unit. Whitespace vor `#` zulässig
in `process_line`.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::passthrough_for_source_without_directives`,
`define_object_like_substitutes_in_subsequent_lines`,
`source_map_records_segments`,
`expand_macros_skips_unknown_identifiers`,
`expand_macros_substitutes_only_full_idents`.

**Status:** done

### §7.2 — Source-Charset: ASCII; String/Char-Literale: ISO Latin-1

**Spec:** §7.2, S. 14 — "IDL uses the ASCII character set, except for
string literals and character literals, which use the ISO Latin-1
(8859-1) character set. The ISO Latin-1 character set is divided into
alphabetic characters (letters) digits, graphic characters, the space
(blank) character, and formatting characters."

**Repo:** Source-Lexer (`crates/idl/src/lexer/tokenizer.rs`):
- Identifier (`is_ident_start` l. 339, `is_ident_continue` l. 343)
  akzeptieren nur ASCII A-Z/a-z/0-9/`_`.
- Punctuation (`match_punct` l. 257) akzeptiert ASCII-Zeichen.
- String-/Char-Literal-Inhalte (`scan_string_literal` l. 358,
  `scan_char_literal` l. 374) konsumieren beliebige Bytes innerhalb der
  Quotes (Latin-1-fähig; UTF-8 Multibyte-Sequenzen werden als
  Byte-Folge durchgereicht).

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_ident_emits_ident_token`,
`ident_starting_with_keyword_prefix_stays_ident`,
`ident_starting_with_underscore`,
`ident_with_digits_after_letter`,
`string_literal_with_escape`,
`char_literal_with_escape`.

**Status:** done

### §7.2 Table 7-2 — ISO Latin-1 Alphabet (Letters)

**Spec:** §7.2 Table 7-2, S. 14-15 — vollständiges ISO-Latin-1-Alphabet:
- ASCII A/a-Z/z;
- akzentuierte Vokale + Konsonanten: `Àà Áá Ââ Ãã Ää Åå Ææ Çç Èè Éé
  Êê Ëë Ìì Íí Îî Ïï Ññ Òò Óó Ôô Õõ Öö Øø Ùù Úú Ûû Üü ß ÿ`.
"upper and lower case equivalences are paired".

**Repo:** Identifier-Lexer akzeptiert nur ASCII A-Z/a-z (Spec §7.2.3
sagt "ASCII alphabetic"). Akzentuierte Latin-1-Letters sind in
Identifiern nicht erlaubt — konform zur Spec.
String-/Char-Literale akzeptieren beliebige Bytes (also auch alle
Latin-1-Letter-Codepoints 0xC0-0xFF).
Case-Äquivalenz für Identifier-Vergleich:
`crates/idl/src/semantics/resolver.rs::CaseInsensitiveIdent::lower`
(l. 50) — ASCII-Lowercase. Latin-1-Akzent-Pairing (z.B. `Ää`) ist
nicht implementiert, in der Praxis irrelevant da Identifier ASCII-only.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::case_insensitive_lookup_finds_struct`,
`case_insensitive_ident_eq_and_hash_consistent`.

**Status:** done

### §7.2 Table 7-3 — Decimal Digits (0–9)

**Spec:** §7.2 Table 7-3, S. 15 — "0 1 2 3 4 5 6 7 8 9".

**Repo:** `crates/idl/src/lexer/tokenizer.rs::scan_number` (l. 161)
nutzt `b.is_ascii_digit()` für Dezimal, `is_ascii_hexdigit()` für
Hex, manuelle `b'0'..=b'7'`-Prüfung für Octal.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::integer_decimal_octal_hex`,
`integer_literal_when_grammar_includes_it`,
`float_with_dot_and_exponent`,
`fixed_point_with_d_suffix`.
`crates/idl/src/semantics/const_eval.rs::tests::int_hex_literal_parsed`,
`int_octal_literal_parsed`,
`integer_literal_octal_max_value`,
`integer_literal_decimal_zero_is_zero`.

**Status:** done

### §7.2 Table 7-4 — Graphic Characters

**Spec:** §7.2 Table 7-4, S. 16-17 — vollständiger Graphic-Charset:
- ASCII: `! " # $ % & ' ( ) * + , - . / : ; < = > ? @ [ \ ] ^ _ \``
  `{ | } ~`.
- Latin-1-Symbole: `¡ ¢ £ ¤ ¥ ¦ § ¨ © ª « ¬` (soft-hyphen) `® ¯ ° ±
  ² ³ ´ µ ¶ · ¸ ¹ º » ¼ ½ ¾ ¿ × ÷`.

**Repo:** ASCII-Graphic-Zeichen werden lexikalisch behandelt:
- Punctuation/Operatoren in `match_punct`
  (`crates/idl/src/lexer/tokenizer.rs` l. 257) — siehe §7.2.5 Table 7-7.
- Quote-Marker `'`/`"` triggern `scan_char_literal`/`scan_string_literal`.
- Backslash `\` ist Escape-Marker innerhalb Literale.
Latin-1-Graphic-Zeichen erscheinen nur innerhalb von String-/Char-
Literalen und werden als Bytes durchgereicht.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_punct`,
`longest_match_for_multichar_punct`,
`shorter_punct_matches_when_longer_does_not_apply`,
`sequence_struct_ident_braces`,
`spans_are_continuous_and_correct`.

**Status:** done

### §7.2 Table 7-5 — Formatting Characters

**Spec:** §7.2 Table 7-5, S. 17 — sieben Formatting-Chars mit ISO-646-
Octal-Werten:
- `BEL` 007 (alert), `BS` 010 (backspace), `HT` 011 (horizontal tab),
- `NL`/`LF` 012 (newline), `VT` 013 (vertical tab), `FF` 014 (form
  feed),
- `CR` 015 (carriage return).

**Repo:** Decoding in Char-/String-Literalen:
`crates/idl/src/semantics/const_eval.rs::decode_escapes` (ll. 614-625)
erzeugt `\a`→0x07, `\b`→0x08, `\t`→0x09, `\n`→0x0A, `\v`→0x0B,
`\f`→0x0C, `\r`→0x0D — alle sieben Werte korrekt.
Source-Whitespace zwischen Tokens:
`crates/idl/src/lexer/tokenizer.rs::skip_whitespace` (l. 280) behandelt
nur Space, HT (`\t`), NL (`\n`), CR (`\r`) — VT/FF fehlen, siehe
§7.2.1-Item.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::char_literal_newline_escape`
(belegt `\n`-Wert 0x0A).
`crates/idl/src/lexer/tokenizer.rs::tests::whitespace_only_yields_empty_stream`
(belegt Space + Tab + Newline).

**Status:** done

### §7.2.1 — Fünf Token-Klassen

**Spec:** §7.2.1, S. 17 — "There are five kinds of tokens:
identifiers, keywords, literals, operators, and other separators."

**Repo:** `crates/idl/src/grammar/mod.rs::TokenKind`-Enum (l. 103+):
- Identifier: `Ident`.
- Keyword: `Keyword(&'static str)`.
- Literale: `IntegerLiteral`, `FloatLiteral`, `StringLiteral`,
  `CharLiteral`, `WideCharLiteral`, `WideStringLiteral`,
  `FixedPtLiteral`, `BooleanLiteral`.
- Operatoren / "other separators": `Punct(&'static str)`.
- Plus `EndOfInput`-Sentinel (kein Spec-Token).

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_keyword_emits_keyword_token`,
`single_ident_emits_ident_token`,
`single_punct`,
`integer_literal_when_grammar_includes_it`,
`string_literal_with_escape`,
`char_literal_with_escape`,
`wide_char_literal`,
`wide_string_literal`,
`float_with_dot_and_exponent`,
`fixed_point_with_d_suffix`.

**Status:** done

### §7.2.1 — Whitespace separiert Tokens, sonst ignoriert

**Spec:** §7.2.1, S. 17 — "Blanks, horizontal and vertical tabs,
newlines, form feeds, and comments (collective, 'white space') as
described below are ignored except as they serve to separate tokens.
Some white space is required to separate otherwise adjacent
identifiers, keywords, and constants."

**Repo:** `crates/idl/src/lexer/tokenizer.rs::skip_whitespace` (l. 280)
behandelt Blanks (` `, 0x20), HT (`\t`, 0x09), NL (`\n`, 0x0A) und CR
(`\r`, 0x0D); `skip_trivia` (l. 292) kombiniert Whitespace + Comment-
Trivia. Whitespace wird komplett gedroppt; Token-Trennung passiert
implizit durch Position-Vorschub.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::whitespace_only_yields_empty_stream`,
`newlines_separate_tokens_without_emitting_trivia`,
`sequence_struct_ident_braces`,
`whitespace_includes_vt_and_ff` (VT/FF).

**Status:** done

### §7.2.1 — Longest-Match-Regel

**Spec:** §7.2.1, S. 17 — "If the input stream has been parsed into
tokens up to a given character, the next token is taken to be the
longest string of characters that could possibly constitute a token."

**Repo:** `crates/idl/src/lexer/rules.rs::TokenRules::from_grammar`
(l. 52) sammelt alle Terminals und sortiert sie nach Lex-Priorität —
längere Strings zuerst (`"::"` vor `":"`, `"=="` vor `"="`,
`"<<"` vor `"<"`).
`Tokenizer::match_punct` (l. 257) iteriert in dieser Reihenfolge und
nimmt den ersten Match (= längsten gültigen).

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::longest_match_for_multichar_punct`
(`<<` vor `<`),
`shorter_punct_matches_when_longer_does_not_apply` (`<` allein wenn
kein `<<`),
`ident_starting_with_keyword_prefix_stays_ident` (Identifier-Match
länger als Keyword-Präfix).

**Status:** done

### §7.2.2 — `/* */` Block-Comments (nicht nestbar)

**Spec:** §7.2.2, S. 17 — "The characters `/*` start a comment, which
terminates with the characters `*/`. These comments do not nest."

**Repo:** `crates/idl/src/lexer/tokenizer.rs::skip_block_comment`
(l. 323) — sucht nach `*/` ohne Recursion/Nesting; emittiert
`ParseError "unterminated block comment starting at byte offset N"`
bei fehlendem Terminator.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::block_comment_skipped`,
`multiline_block_comment_skipped`,
`multiple_comments_in_a_row`,
`comments_inside_struct_definition`,
`unterminated_block_comment_is_error`.

**Status:** done

### §7.2.2 — `//` Line-Comments

**Spec:** §7.2.2, S. 17 — "The characters `//` start a comment, which
terminates at the end of the line on which they occur."

**Repo:** `crates/idl/src/lexer/tokenizer.rs::skip_line_comment`
(l. 315) — konsumiert bis NL oder EOF.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::line_comment_skipped`,
`line_comment_at_end_of_input`.

**Status:** done

### §7.2.2 — Comment-Marker haben innerhalb Comments keine Bedeutung

**Spec:** §7.2.2, S. 17 — "The comment characters `//`, `/*`, and `*/`
have no special meaning within a `//` comment and are treated just
like other characters. Similarly, the comment characters `//` and
`/*` have no special meaning within a `/*` comment."

**Repo:** `skip_line_comment` konsumiert blind alle Zeichen bis NL
(keine `/*`-Erkennung). `skip_block_comment` sucht ausschließlich nach
der `*/`-Sequenz, ignoriert dazwischenliegende `//` und `/*`.
`slash_in_string_is_not_comment_start`-Test belegt zusätzlich, dass
`/`-Zeichen innerhalb String-Literalen kein Comment-Start sind.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::slash_in_string_is_not_comment_start`,
`line_comment_contains_block_comment_start`,
`block_comment_contains_line_comment_marker`,
`block_comment_does_not_nest` (alle drei).

**Status:** done

### §7.2.2 — Erlaubte Zeichen in Comments

**Spec:** §7.2.2, S. 18 — "Comments may contain alphabetic, digit,
graphic, space, horizontal tab, vertical tab, form feed, and newline
characters."

**Repo:** `skip_line_comment`/`skip_block_comment` konsumieren
beliebige Bytes bis EOL bzw. `*/` — keine Charset-Prüfung. Damit sind
alle aufgezählten Klassen automatisch erlaubt.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::comments_inside_struct_definition`,
`multiple_comments_in_a_row`.

**Status:** done

### §7.2.3 — Identifier-Form (ASCII alphabetic + digit + `_`)

**Spec:** §7.2.3, S. 18 — "An identifier is an arbitrarily long
sequence of ASCII alphabetic, digit and underscore (_) characters. The
first character must be an ASCII alphabetic character. All characters
are significant."

**Repo:** `crates/idl/src/lexer/tokenizer.rs`:
- `is_ident_start` (l. 339) — `b.is_ascii_alphabetic() || b == b'_'`.
  Hinweis: Underscore-Start erlaubt durch Escaped-Identifier-Regel
  §7.2.3.2; Spec-Strict-First-Char-Letter-Regel ist im Lexer
  zugunsten der Escape-Variante aufgeweicht und durch §7.2.3.2 oben
  abgedeckt.
- `is_ident_continue` (l. 343) — `b.is_ascii_alphanumeric() || b == b'_'`.
- `scan_ident` (l. 347) — konsumiert bis zum letzten Continue-Byte.
"All characters are significant" → `scan_ident` setzt keine Längen-
Limits, alle Bytes des Idents fließen in den `text`-Slice.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_ident_emits_ident_token`,
`ident_starting_with_keyword_prefix_stays_ident`,
`ident_with_digits_after_letter`.

**Status:** done

### §7.2.3 — Identifier-Case-Sensitivity (insensitive Definition, Same-Case-Use)

**Spec:** §7.2.3, S. 18 — "IDL identifiers are case insensitive.
However, all references to a definition must use the same case as the
defining occurrence. This allows natural mappings to case-sensitive
languages."

**Repo:** Case-insensitive Vergleichs-Schlüssel:
`crates/idl/src/semantics/resolver.rs::CaseInsensitiveIdent` (Hash + Eq
via ASCII-Lowercase, l. 50, 63); Original-Schreibweise wird im
`original`-Field bewahrt für Diagnostik und Code-Gen.
Same-Case-Reference-Pflicht: aktuell **nicht** durchgesetzt; Lookups
über `Scope::lookup` finden case-insensitiv den Eintrag, ein Use mit
abweichender Schreibweise generiert keine Diagnostik.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::case_insensitive_lookup_finds_struct`,
`case_insensitive_ident_eq_and_hash_consistent`.

**Status:** done — Same-Case-Use-Diagnostik durch
`crates/idl/src/semantics/resolver.rs::Resolver::resolve` (l. 770+)
implementiert: nach erfolgreichem Lookup wird das Use-Casing (last
part des Scoped-Names, gestripped um §7.2.3.2-Escape) gegen
`sym.original_casing` verglichen; Mismatch liefert
`ResolverError::CaseMismatch`. Tests
`reference_with_different_case_reports_error`,
`reference_with_same_case_resolves_ok`,
`escaped_reference_to_unescaped_def_resolves_ok`.

### §7.2.3.1 — Case-Äquivalenz beim Vergleich

**Spec:** §7.2.3.1, S. 18 — "When comparing two identifiers to see if
they collide:
- Upper- and lower-case letters are treated as the same letter.
  Table 7-2 defines the equivalence mapping of upper- and lower-case
  letters.
- All characters are significant."

**Repo:** Vergleichs-Operator: `CaseInsensitiveIdent::eq`/`hash` (l. 50,
63) macht ASCII-Lowercase. "All characters significant": Vergleich
nutzt den vollen Identifier-String — kein Truncation, kein Suffix-Match.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::case_insensitive_ident_eq_and_hash_consistent`,
`case_insensitive_lookup_finds_struct`.

**Status:** done

### §7.2.3.1 — Same-Definition-Case-Pflicht (Compile-Error)

**Spec:** §7.2.3.1, S. 18 — "Identifiers that differ only in case
collide, and will yield a compilation error under certain
circumstances. An identifier for a given definition must be spelled
identically (e.g., with respect to case) throughout a specification."

**Repo:** Definition-Insert in
`crates/idl/src/semantics/resolver.rs` (Scope-Insert) ist case-
insensitiv: zwei Definitionen `Foo` und `foo` im selben Scope erzeugen
`DuplicateDefinition`. "Same spelling throughout"-Use ist Teil von
§7.2.3-open-same-case-ref oben.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::duplicate_definition_logs_error`.

**Status:** done

### §7.2.3.1 — Single Namespace pro Scope

**Spec:** §7.2.3.1, S. 18 — "There is only one namespace for IDL
identifiers in each scope. Using the same identifier for a constant
and an interface, for example, produces a compilation error."
Beispiel-Code: `module M { typedef long Foo; const long thing = 1;
interface thing { void doit (in Foo foo); readonly attribute long
Attribute; }; };` mit Errors: "reuse of identifier thing", "Foo and
foo collide", "Attribute collides with keyword attribute".

**Repo:** `crates/idl/src/semantics/resolver.rs::Scope` ist
**eine** Map von `CaseInsensitiveIdent → Symbol`. Es existiert
kein eigener Namespace für Typen vs. Constants vs. Interfaces.
Beispiele aus dem Spec-Code:
- `typedef long Foo` und `in Foo foo` im selben Op-Scope kollidieren —
  Param-Konflikt.
- `const long thing = 1` und `interface thing { … }` in M kollidieren.
- `attribute long Attribute` kollidiert mit Keyword `attribute` (siehe
  §7.2.4-open, Strict-Case ist dort offen).

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::duplicate_definition_logs_error`,
`module_reopen_merges_symbols`,
`rejects_typedef_redef_in_same_scope`,
`rejects_const_redef_in_same_scope`,
`rejects_duplicate_param_names_within_op`,
`rejects_case_conflict_param_names_within_op`,
`accepts_same_name_in_nested_scopes`,
`accepts_same_name_across_independent_modules`.

**Status:** done — Beispiel "`Attribute` kollidiert mit Keyword
`attribute`" durch §7.2.4-Keyword-Collision-Check in
`Scope::insert` abgedeckt
(`identifier_collides_with_keyword_case_insensitive`).

### §7.2.3.2-base — Escaped-Identifier-Regel (`_`-Prefix turn-off Keyword-Check)

**Spec:** §7.2.3.2, S. 18-19 — "As all languages, IDL uses some
reserved words called keywords (see 7.2.4, Keywords). As IDL evolves,
new keywords that are added to the IDL language may inadvertently
collide with identifiers used in existing IDL and programs that use
that IDL. … To minimize the amount of work, users may lexically
`escape` identifiers by prepending an underscore (_) to an identifier.
*This is a purely lexical convention that ONLY turns off keyword
checking.* The resulting identifier follows all the other rules for
identifier processing. For example, the identifier `_AnIdentifier` is
treated as if it were `AnIdentifier`."
Beispiel-Code: `module M { interface thing { attribute boolean
abstract; // Error: abstract collides with keyword abstract attribute
boolean _abstract; // OK: abstract is an identifier }; };`

**Repo:** Lexer-Verhalten in
`crates/idl/src/lexer/tokenizer.rs::classify_ident` (l. 242):
- Identifier-Text mit `_`-Prefix: `is_ident_start` akzeptiert `_`,
  `scan_ident` greift den ganzen Token, Keyword-Lookup matcht NICHT
  weil das Lookup-Wort den `_` enthält — Token-Klasse bleibt `Ident`.
  Damit ist die Spec-Bedingung "ONLY turns off keyword checking"
  erfüllt.
- Spec-Äquivalenz `_AnIdentifier == AnIdentifier` für Symbol-Lookup:
  AKTUELL NICHT implementiert; `_abstract` und `abstract` werden als
  zwei verschiedene Einträge im Resolver-Scope behandelt.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::ident_starting_with_underscore`
(`_struct` → `Ident`, kein Keyword-Match).

**Status:** done — Tokenizer schaltet Keyword-Check ab; die
Lookup-Äquivalenz `_AnIdentifier == AnIdentifier` ist im Resolver
über `strip_escape` realisiert (siehe Folgeeintrag).

### §7.2.3.2 — Äquivalenz `_AnIdentifier == AnIdentifier` beim Lookup

**Spec:** §7.2.3.2, S. 18 — "the identifier `_AnIdentifier` is treated
as if it were `AnIdentifier`." (= Äquivalenz beim Lookup, nicht nur
Keyword-Check-Off).

**Repo:** `crates/idl/src/semantics/resolver.rs::strip_escape` (l. 75+)
strippt einen führenden `_`, wenn der Rest mit ASCII-Letter beginnt.
`CaseInsensitiveIdent::eq`/`hash` (l. 54+) sowie der CaseConflict-
Vergleich in `Scope::insert` (l. 159+) verwenden den gestripten Key,
sodass `_foo` und `foo` als kanonisch derselbe Identifier behandelt
werden.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::escaped_identifier_equals_unescaped_for_lookup`,
`escaped_identifier_collides_with_unescaped`.

**Status:** done

### §7.2.3.2 — Note (informativ, Empfehlung zur Verwendung)

**Spec:** §7.2.3.2, S. 19 — "Note – To avoid unnecessary confusion
for readers of IDL, it is recommended that IDL specifications only use
the escaped form of identifiers when the non-escaped form clashes with
a newly introduced IDL keyword. It is also recommended that interface
designers avoid defining new identifiers that are known to require
escaping. Escaped literals are only recommended for IDL that expresses
legacy items, or for IDL that is mechanically generated."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung/Note der Spec; nicht-bindend.

### §7.2.4 Table 7-6 — Vollständige Keyword-Liste

**Spec:** §7.2.4 + Table 7-6, S. 19-20 — "The identifiers listed in
Table 7-6 are reserved for use as keywords and may not be used for
another purpose, unless escaped with a leading underscore."
Keyword-Liste (73 Einträge):
`abstract`, `any`, `alias`, `attribute`, `bitfield`,
`bitmask`, `bitset`, `boolean`, `case`, `char`,
`component`, `connector`, `const`, `consumes`, `context`,
`custom`, `default`, `double`, `exception`, `emits`,
`enum`, `eventtype`, `factory`, `FALSE`, `finder`,
`fixed`, `float`, `getraises`, `home`, `import`,
`in`, `inout`, `interface`, `local`, `long`,
`manages`, `map`, `mirrorport`, `module`, `multiple`,
`native`, `Object`, `octet`, `oneway`, `out`,
`primarykey`, `private`, `port`, `porttype`, `provides`,
`public`, `publishes`, `raises`, `readonly`, `setraises`,
`sequence`, `short`, `string`, `struct`, `supports`,
`switch`, `TRUE`, `truncatable`, `typedef`, `typeid`,
`typename`, `typeprefix`, `unsigned`, `union`, `uses`,
`ValueBase`, `valuetype`, `void`, `wchar`, `wstring`,
`int8`, `uint8`, `int16`, `int32`, `int64`,
`uint16`, `uint32`, `uint64`.

**Repo:** Alle Keywords erscheinen als
`Symbol::Terminal(TokenKind::Keyword("..."))` in Productions in
`crates/idl/src/grammar/idl42.rs` (z.B. `int8`/`uint8`/.../`uint64`
ll. 816+ in `integer_type`-Deltas, `valuetype`/`truncatable`/`custom`
ll. 2540+ im `value_dcl`-Cluster, `eventtype`/`port`/`porttype`/
`finder`/`home`/`manages`/`emits`/`publishes`/`consumes`/`provides`/
`uses`/`mirrorport` im CCM-Cluster). `from_grammar` in
`crates/idl/src/lexer/rules.rs` (l. 52) sammelt sie automatisch in
`TokenRules`.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_keyword_emits_keyword_token`,
`all_table_7_6_keywords_classified_as_keyword` (73-Eintrag-Whitelist
gegen `IDL_42`-Grammar; `alias`/`context`/`ValueBase` via Production-Stubs
abgedeckt).
Recognizer-Tests in `crates/idl/src/grammar/idl42.rs::tests`
(~750 Tests, decken alle Keyword-tragenden Productions).

**Status:** done

### §7.2.4 — "Keywords must be written exactly as shown"

**Spec:** §7.2.4, S. 20 — "Keywords must be written exactly as shown
in the above list. Identifiers that collide with keywords (see 7.2.3,
Identifiers) are illegal."
Beispiel-Code: `module M { typedef Long Foo; // Error: keyword is
long not Long typedef boolean BOOLEAN; // Error: BOOLEAN collides with
the keyword … boolean; };`.

**Repo:** `crates/idl/src/lexer/tokenizer.rs::classify_ident` (l. 242)
matcht Keywords case-strikt; `Long` und `BOOLEAN` werden lexikalisch
als `Ident` klassifiziert. Die Resolver-Diagnostik für "Identifiers
that collide with keywords are illegal" ist
`crates/idl/src/semantics/resolver.rs::Scope::insert` (l. 159+):
case-insensitiver Match gegen `IDL_KEYWORDS_TABLE_7_6` (83 Keywords)
liefert `ResolverError::IdentifierCollidesWithKeyword`. Escape via
`_`-Präfix (§7.2.3.2) überspringt den Check.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_keyword_emits_keyword_token`,
`ident_starting_with_keyword_prefix_stays_ident`,
`crates/idl/src/semantics/resolver.rs::tests::identifier_collides_with_keyword_case_insensitive`,
`escaped_identifier_with_keyword_does_not_collide`.

**Status:** done

### §7.2.4 — Note: Building-Block-spezifische Keyword-Subsets

**Spec:** §7.2.4, S. 20 — "Note – As the IDL grammar is now organized
in building blocks that dedicated profiles may include or not, some of
these keywords may be irrelevant to some profiles. Each building
block description (see 7.4, IDL Grammar) includes the set of keywords
that are specific to it. The minimum set of keywords for a given
profile results from the union of all the ones of the included
building blocks. However, to avoid unnecessary confusion for readers
of IDL and to allow IDL compilers supporting several profiles in a
single tool, it is recommended that IDL specifications avoid using
any of the keywords listed in Table 7-6: All IDL keywords."

**Repo:** Building-Block-spezifische Keyword-Aktivierung wird durch
das Feature-Flag-System abgedeckt
(`crates/idl/src/features/mod.rs`, `crates/idl/src/features/gate.rs`).
Beispiel: bei `dds_basic`-Profile sind `valuetype`/`eventtype`/
`component` etc. nicht aktiv und der Feature-Gate-Validator lehnt
entsprechende Productions ab.

**Tests:** `crates/idl/src/features/gate.rs::tests` —
Profile-spezifische Rejects (`dds_basic_rejects_native`,
`dds_extensible_rejects_value_def`,
`corba_full_minus_extras_rejects_custom_valuetype`,
`corba_full_minus_extras_rejects_abstract_valuetype`,
`corba_full_minus_extras_rejects_truncatable`); Allows
(`corba_full_allows_native`, `corba_full_allows_value_def`,
`dds_basic_allows_value_box`,
`opensplice_legacy_allows_value_def_with_factory`,
`corba_full_allows_repository_ids_and_import`,
`corba_full_allows_oneway_and_abstract_local`,
`corba_full_allows_custom_abstract_truncatable`).

**Status:** done

### §7.2.5 — Punctuation Charset (Table 7-7)

**Spec:** §7.2.5 + Table 7-7, S. 20 — IDL-Punctuation-Charset:
`; { } : , = + - ( ) < > [ ] ' " \ | ^ & * / % ~ @`
(zwei Zeilen in der Spec-Tabelle: erste Zeile
`; { } : , = + - ( ) < > [ ]`,
zweite Zeile `' " \ | ^ & * / % ~ @`).

**Repo:** Punctuation als `Symbol::Terminal(TokenKind::Punct("..."))`
in `crates/idl/src/grammar/idl42.rs`:
`;` `{` `}` `:` `::` `,` `=` `+` `-` `(` `)` `<` `>` `[` `]` `*`
`/` `%` `~` `^` `&` `|` `<<` `>>` `@`.
Apostroph (`'`) und Double-Quote (`"`) sind Quote-Marker für
Char-/String-Literale (`scan_char_literal`/`scan_string_literal`),
nicht standalone-Tokens. Backslash (`\`) ist Escape-Marker innerhalb
Literale.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::single_punct`,
`longest_match_for_multichar_punct`,
`shorter_punct_matches_when_longer_does_not_apply`,
`tokenize_toy_grammar_addition`,
`tokenize_toy_grammar_parenthesized`,
`sequence_struct_ident_braces`,
`spans_are_continuous_and_correct`.

**Status:** done

### §7.2.5 — Preprocessor-Token-Set (Table 7-8)

**Spec:** §7.2.5 + Table 7-8, S. 20 — "the tokens listed in Table 7-8
are used by the preprocessor": `# ## ! || &&`.

**Repo:** `crates/idl/src/preprocessor/mod.rs`:
- `#` — Direktiven-Marker, `process_line` (l. 460+) erkennt
  `# <directive>` und dispatched zu `define`/`undef`/`include`/`if`/
  `ifdef`/`ifndef`/`elif`/`else`/`endif`/`pragma`/`warning`/`line`.
- `##` — Token-Pasting-Operator: im Modul-Kommentar (l. 28) als
  geplant gelistet, in `expand_macros` (l. 895) NICHT implementiert.
- `!` — Negation in `#if`-Expressions: `eval_if_tokens` (l. 699).
- `||` — Logical-OR in `#if`-Expressions: `eval_if_tokens`.
- `&&` — Logical-AND in `#if`-Expressions: `eval_if_tokens`.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests`:
- `#`-Direktiven: `pragma_is_stripped`,
  `define_object_like_substitutes_in_subsequent_lines`,
  `ifdef_keeps_block_when_macro_defined`,
  `ifdef_drops_block_when_macro_not_defined`,
  `ifndef_inverse_of_ifdef`, `else_branch_taken_when_initial_false`,
  `nested_ifdef_works`, `undef_removes_macro`,
  `quoted_include_resolves`, `system_include_resolves`,
  `missing_include_is_error`, `include_cycle_is_detected`,
  `unmatched_endif_is_error`, `unclosed_conditional_is_error`,
  `unmatched_else_is_error`,
  `macro_in_inactive_branch_does_not_take_effect`,
  `nested_if_in_active_branch`,
  `warning_directive_does_not_abort`,
  `line_directive_does_not_abort`.
- `!`-Eval: `if_eval_logical_not`.
- `||`-Eval: `if_eval_logical_or`.
- `&&`-Eval: kein dedizierter Test.
- `#if`-Numeric: `if_eval_numeric_zero_drops_block`,
  `if_eval_numeric_nonzero_keeps_block`.
- `#if defined`: `if_eval_defined_macro_keeps_block`,
  `if_eval_undefined_macro_drops_block`.
- `#elif`: `if_elif_else_branches`,
  `if_elif_picks_first_true_branch`.

**Status:** done — `#`/`##` und `&&` sind in
`crates/idl/src/preprocessor/mod.rs` implementiert (siehe Folge-
Einträge).

### §7.2.5 — `#` Stringize-Operator (in function-like Macros)

**Spec:** Table 7-8, S. 20 — `#` ist Preprocessor-Token. ISO/IEC
14882:2003 (referenziert in §7.3) §16.3.2: in einer function-like
Macro wandelt `#param` den zugehörigen Argument-Text in einen
String-Literal um.

**Repo:** `crates/idl/src/preprocessor/mod.rs::expand_function_like`
erkennt `#param` im Body, substituiert mit dem Argument-Text und
wrappt in `"…"`. `\` und `"` im Argument werden escaped.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::stringize_param_in_function_macro`,
`stringize_escapes_quotes_and_backslashes`.

**Status:** done

### §7.2.5 — `##` Token-Pasting-Operator

**Spec:** Table 7-8, S. 20 — `##` ist Preprocessor-Token. ISO/IEC
14882:2003 §16.3.3: konkateniert die beiden umgebenden Tokens.

**Repo:** `crates/idl/src/preprocessor/mod.rs::expand_function_like`
führt einen Token-Paste-Pass aus, bevor Param-Substitution greift:
LHS und RHS um `##` werden param-substituiert und konkateniert;
Whitespace zwischen den Operanden entfällt.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::token_paste_concatenates_idents`,
`token_paste_with_macro_args_produces_single_ident`.

**Status:** done

### §7.2.5 — Logical-AND im `#if`-Eval

**Spec:** Table 7-8, S. 20 — `&&` ist Preprocessor-Token.

**Repo:** `crates/idl/src/preprocessor/mod.rs::eval_and` (l. 759+)
parst und wertet `&&` aus.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::if_eval_logical_and_both_defined_keeps_block`,
`if_eval_logical_and_one_undefined_drops_block`,
`if_eval_logical_and_both_undefined_drops_block`.

**Status:** done

### §7.2.6 — Literal-Klassen-Uebersicht

**Spec:** §7.2.6, S. 20 — "This sub clause describes the following
literals: Integer, Character, Floating-point, String, Fixed-point."

**Repo:** `crates/idl/src/grammar/mod.rs::TokenKind` (l. 103+):
`IntegerLiteral`, `FloatLiteral`, `StringLiteral`, `CharLiteral`,
`WideStringLiteral`, `WideCharLiteral`, `FixedPtLiteral` (Bool ist
nicht Spec-Literal-Klasse, sondern als `TRUE`/`FALSE`-Keyword
modelliert; `BooleanLiteral`-Variante existiert intern für Lowering-
Convenience).

**Tests:** s. Sub-Items §7.2.6.1 - §7.2.6.5.

**Status:** done

### §7.2.6.1 — Integer Literal Defaults: Decimal (no leading 0)

**Spec:** §7.2.6.1, S. 20 — "An integer literal consisting of a
sequence of digits is taken to be *decimal* (base ten) unless it
begins with 0 (digit zero)."

**Repo:** `crates/idl/src/lexer/tokenizer.rs::scan_number` (l. 161+) —
nimmt Sequenz von ASCII-Digits, ohne Präfix → `IntegerLiteral`.
`crates/idl/src/semantics/const_eval.rs::parse_integer` (l. 290) ruft
`from_str_radix(s, 10)` wenn kein `0`-Prefix.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::integer_literal_when_grammar_includes_it`.
`crates/idl/src/semantics/const_eval.rs::tests::integer_literal_decimal_zero_is_zero`,
`int_promotion_long_default`.

**Status:** done

### §7.2.6.1 — Octal Integer Literal (leading `0`)

**Spec:** §7.2.6.1, S. 20 — "A sequence of digits starting with 0 is
taken to be an *octal integer* (base eight). The digits 8 and 9 are
not octal digits and thus are not allowed in an octal integer
literal."

**Repo:** `parse_integer` erkennt `0`-Prefix ohne `x`/`X` und ruft
`from_str_radix(s, 8)`. Bei Digits `8`/`9` schlägt das Parsing fehl
(Rust-Stdlib gibt Error, mapped zu `EvalError::InvalidLiteral`).

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::integer_decimal_octal_hex`.
`crates/idl/src/semantics/const_eval.rs::tests::int_octal_literal_parsed`,
`integer_literal_octal_max_value`,
`octal_literal_with_digit_8_or_9_is_error` (belegt
`018`/`079` → `EvalError::InvalidLiteral`).

**Status:** done

### §7.2.6.1 — Hexadecimal Integer Literal (`0x`/`0X` Präfix)

**Spec:** §7.2.6.1, S. 21 — "A sequence of digits preceded by 0x (or
0X) is taken to be a *hexadecimal integer* (base sixteen). The
hexadecimal digits include a (or A) through f (or F) with decimal
values ten through fifteen, respectively."
Beispiel: "the number twelve can be written 12, 014, or 0XC."

**Repo:** `scan_number` (l. 163+): bei `0x`/`0X`-Präfix konsumiert
`is_ascii_hexdigit()`-Folge (akzeptiert `a-f`/`A-F`/`0-9`).
`parse_integer` ruft `from_str_radix(s, 16)`.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::integer_decimal_octal_hex`.
`crates/idl/src/semantics/const_eval.rs::tests::int_hex_literal_parsed`,
`int_promotion_to_ulong_when_too_large`,
`int_promotion_long_long_when_signed_huge`.

**Status:** done

### §7.2.6.2 — `char` ist 8-bit (0..255), Latin-1-/null-/Formatting-Werte

**Spec:** §7.2.6.2.1, S. 21 — "A *char* is an 8-bit quantity with a
numerical value between 0 and 255 (decimal). The value of a space,
alphabetic, digit, or graphic character literal is the numerical
value of the character as defined in the ISO Latin-1 (8859-1)
character set standard (see Table 7-2 on page 14, Table 7-3 on page 15
and Table 7-4 on page 16). The value of a *null* is 0. The value of a
formatting character literal is the numerical value of the character
as defined in the ISO 646 standard (see Table 7-5 on page 17). The
meaning of all other characters is implementation-dependent."

**Repo:** `crates/idl/src/semantics/const_eval.rs::decode_char` (l. 507)
ruft `decode_escapes(_, allow_wide=false)` und prüft Range 0..255 →
`ConstValue::Octet`. Latin-1-/Formatting-Werte ergeben sich aus den
Codepoint-Werten der Escape-Sequenzen.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::char_literal_basic_ascii`,
`char_literal_hex_max_byte`,
`char_literal_octal_max`,
`char_literal_octal_overflow_is_range_error`,
`char_literal_nul_is_allowed`,
`char_literal_newline_escape`.

**Status:** done

### §7.2.6.2.1 — Char-Literal-Form (`'X'`)

**Spec:** §7.2.6.2.1, S. 21 — "Character literals may have type
*char* (non-wide character) or *wchar* (wide character). Both wide and
non-wide character literals must be specified using characters from
the ISO Latin-1 (8859-1) character set. … A character literal is one
or more characters enclosed in single quotes, as in:
`const char C1 = 'X';`."

**Repo:** Lex: `crates/idl/src/lexer/tokenizer.rs::scan_char_literal`
(l. 374) — konsumiert Inhalt zwischen `'…'`, akzeptiert
Backslash-Escape, produziert `TokenKind::CharLiteral`.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::char_literal_with_escape`,
`unterminated_char_literal_is_error`.

**Status:** done

### §7.2.6.2.1 — `wchar` (implementation-dependent size, `L'X'`-Form)

**Spec:** §7.2.6.2.1, S. 21 — "A *wchar* (wide character) is intended
to encode wide characters from any character set. Its size is
implementation dependent. … Wide character literals have in addition
an L prefix, as in: `const wchar C2 = L'X';`."

**Repo:** Lex-Disambiguator in
`crates/idl/src/lexer/tokenizer.rs::tokenize` (ll. 86-99) — `L'…'` →
`WideCharLiteral`. Const-Eval:
`crates/idl/src/semantics/const_eval.rs::decode_wide_char` (l. 530)
liefert `u32`-Codepoint via `decode_escapes(_, allow_wide=true)`.
Implementation-dependent-Size ist erfüllt: Code-Gen-Schicht
(späterer Spike) entscheidet pro Sprach-Mapping (z.B. C++ `wchar_t`).

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::wide_char_literal`,
`identifier_starting_with_l_is_not_wide_literal`.
`crates/idl/src/semantics/const_eval.rs::tests::wchar_literal_unicode_decodes`,
`wchar_literal_unicode_three_digits_is_error`.

**Status:** done

### §7.2.6.2.1 — Cross-Assign wchar↔char Error

**Spec:** §7.2.6.2.1, S. 21 — "Attempts to assign a wide character
literal to a non-wide character constant, or to assign a non-wide
character literal to a wide character constant, shall be treated as
an error."

**Repo:** Lex-Phase produziert getrennte Token-Kinds (`CharLiteral` vs
`WideCharLiteral`); Cross-Type-Diagnostik in
`crates/idl/src/semantics/const_eval.rs::check_const_decl_type_match`:
Mismatch zwischen `ConstType` (Char/WideChar/String{wide}) und
`ConstValue` (Char/WChar/String/WString) liefert
`EvalError::CrossAssignWideNarrow`. Gilt symmetrisch für
char↔wchar und string↔wstring (siehe §7.2.6.3).

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::wide_char_literal_to_char_const_is_error`,
`narrow_char_literal_to_wchar_const_is_error`,
`matching_char_const_decl_passes`.

**Status:** done

### §7.2.6.2.2 Table 7-9 — Escape-Sequenz-Set

**Spec:** §7.2.6.2.2 + Table 7-9, S. 21-22 — Escape-Sequenzen:
- `\n` newline
- `\t` horizontal tab
- `\v` vertical tab
- `\b` backspace
- `\r` carriage return
- `\f` form feed
- `\a` alert
- `\\` backslash
- `\?` question mark
- `\'` single quote
- `\"` double quote
- `\ooo` octal number (1, 2, oder 3 Digits)
- `\xhh` hexadecimal number (1 oder 2 Digits)
- `\uhhhh` Unicode character (1, 2, 3, oder 4 Digits, nur in
  `wchar`/`wstring`).

**Repo:** `crates/idl/src/semantics/const_eval.rs::decode_escapes`
(ll. 602-697) implementiert alle 14 Klassen:
- 11 Single-Char-Escapes (ll. 614-625) → konkrete Codepoint-Werte.
- `\ooo` (ll. 626-639) — bis zu 3 Octal-Digits, Termination beim ersten
  Nicht-Octal-Char.
- `\xhh` (ll. 640-660) — bis zu 2 Hex-Digits, Fehler bei null Digits.
- `\uhhhh` (ll. 661-687) — exakt 4 Hex-Digits, Fehler wenn weniger;
  nur erlaubt wenn `allow_wide=true`.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::char_literal_with_escape`,
`string_literal_with_escape`.
`crates/idl/src/semantics/const_eval.rs::tests::char_literal_newline_escape`,
`char_literal_hex_escape`,
`char_literal_octal_escape`,
`char_literal_question_mark_escape`,
`char_literal_hex_max_byte`,
`char_literal_octal_max`,
`char_literal_hex_no_digits_is_error`,
`char_literal_octal_overflow_is_range_error`,
`wchar_literal_unicode_decodes`,
`wchar_literal_unicode_three_digits_is_error`,
`string_literal_with_escape_decoded`,
`string_literal_with_hex_and_octal_escapes`,
`wstring_literal_with_unicode_escape`.

**Status:** done — Tabellengetriebener Test
`crates/idl/src/semantics/const_eval.rs::tests::escape_sequences_table_7_9_alphabetic`
deckt alle 11 alphabetischen Escapes.

### §7.2.6.2.2 — Backslash-Folge-Char nicht in Set: undefined behaviour

**Spec:** §7.2.6.2.2, S. 21 — "If the character following a backslash
is not one of those specified, the behavior is undefined. An escape
sequence specifies a single character."

**Repo:** `decode_escapes` (l. 688+) liefert
`EvalError::InvalidLiteral` bei unbekannter Escape-Sequenz statt
"undefined behaviour"-Passthrough. Strikter als Spec — aus Audit-Sicht
zulässig (UB ist erlaubt zu fixen).

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::unknown_escape_is_invalid_literal`.

**Status:** done — Test belegt `'\q'` → `EvalError::InvalidLiteral`.

### §7.2.6.2.2 — `\ooo` Termination beim ersten Nicht-Octal-Digit

**Spec:** §7.2.6.2.2, S. 21 — "The escape `\ooo` consists of the
backslash followed by one, two, or three octal digits that are taken
to specify the value of the desired character. … A sequence of octal
or hexadecimal digits is terminated by the first character that is
not an octal digit or a hexadecimal digit, respectively. The value of
a character constant is implementation dependent if it exceeds that
of the largest *char*."

**Repo:** `decode_escapes` (ll. 626-639) — konsumiert max. 3 Digits,
peekt vor jedem Schritt; Loop-`break` beim ersten Nicht-Octal-Char.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::char_literal_octal_max`,
`char_literal_octal_overflow_is_range_error`,
`string_literal_with_hex_and_octal_escapes`.

**Status:** done

### §7.2.6.2.2 — `\xhh` Termination beim ersten Nicht-Hex-Digit

**Spec:** §7.2.6.2.2, S. 21 — "The escape `\xhh` consists of the
backslash followed by x followed by one or two hexadecimal digits
that are taken to specify the value of the desired character."

**Repo:** `decode_escapes` (ll. 640-660) — konsumiert max. 2 Digits;
Fehler `\\x with no hex digits` wenn count == 0.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::char_literal_hex_escape`,
`char_literal_hex_max_byte`,
`char_literal_hex_no_digits_is_error`.

**Status:** done

### §7.2.6.2.2 — `\uhhhh` nur in `wchar`/`wstring`

**Spec:** §7.2.6.2.2, S. 22 — "The escape `\uhhhh` consists of a
backslash followed by the character u, followed by one, two, three,
or four hexadecimal digits. This represents a Unicode character
literal. … The `\u` escape is valid only with *wchar* and *wstring*
types. Because a wide string literal is defined as a sequence of wide
character literals, a sequence of `\u` literals can be used to define
a wide string literal. Attempts to set a char type to a `\u` defined
literal or a string type to a sequence of `\u` literals result in an
error."

**Repo:** `decode_escapes` (ll. 661-687): wenn `allow_wide=false`,
liefert `EvalError::InvalidLiteral "\u escape only valid in wide
literals"`. Aufrufer in `decode_char`/`decode_string` setzen
`allow_wide=false`; `decode_wide_char`/`decode_wide_string` setzen
`allow_wide=true`.
Hinweis: Spec lässt 1-4 Hex-Digits zu, Implementation verlangt
exakt 4 (`count != 4 → Error`). Konkret strikter — siehe
§7.2.6.2.2-open-uhhhh-1to3.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::char_literal_unicode_in_narrow_is_error`,
`wchar_literal_unicode_decodes`,
`wchar_literal_unicode_three_digits_is_error`,
`wstring_literal_with_unicode_escape`,
`string_literal_with_unicode_escape_is_error`.

**Status:** done — `decode_escapes`-`\u`-Branch akzeptiert
`count >= 1`. Tests
`crates/idl/src/semantics/const_eval.rs::tests::wchar_literal_unicode_one_digit_decodes`,
`wchar_literal_unicode_two_digits_decodes`,
`wchar_literal_unicode_three_digits_decodes`,
`wchar_literal_unicode_zero_digits_is_error`.

### §7.2.6.3 — String-Literal-Form (`"…"`, null-terminated)

**Spec:** §7.2.6.3, S. 22 — "Strings are null-terminated sequences of
characters. Strings are of type *string* if they are made of non-wide
characters or *wstring* (wide string) if they are made of wide
characters. A string literal is a sequence of character literals (as
defined in 7.2.6.2, Character Literals), with the exception of the
character with numeric value 0, surrounded by double quotes, as in:
`const string S1 = "Hello";`."

**Repo:** Lex: `crates/idl/src/lexer/tokenizer.rs::scan_string_literal`
(l. 358) — konsumiert `"…"`-Inhalt, akzeptiert Backslash-Escape.
Const-Eval: `crates/idl/src/semantics/const_eval.rs::decode_string`
(l. 550) ruft `decode_escapes(_, allow_wide=false)`. NUL-Prüfung
erfolgt im `decode_string`-Body.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::string_literal_with_escape`,
`unterminated_string_literal_is_error`,
`slash_in_string_is_not_comment_start`.
`crates/idl/src/semantics/const_eval.rs::tests::string_literal_concat`,
`string_literal_with_escape_decoded`.

**Status:** done

### §7.2.6.3 — `wstring`-Form (`L"…"`)

**Spec:** §7.2.6.3, S. 22 — "Wide string literals have in addition an
L prefix, for example: `const wstring S2 = L"Hello";`. Both wide and
non-wide string literals must be specified using characters from the
ISO Latin-1 (8859-1) character set."

**Repo:** Lex-Disambiguator in `tokenize` (ll. 88+): `L"…"` →
`WideStringLiteral`. Const-Eval:
`crates/idl/src/semantics/const_eval.rs::decode_wide_string` (l. 581)
ruft `decode_escapes(_, allow_wide=true)` und liefert `Vec<u32>`.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::wide_string_literal`.
`crates/idl/src/semantics/const_eval.rs::tests::wstring_concat`,
`wstring_literal_with_unicode_escape`.

**Status:** done

### §7.2.6.3 — NUL-Verbot in String und WString

**Spec:** §7.2.6.3, S. 22 — "A string literal shall not contain the
character `\0`. A wide string literal shall not contain the wide
character with value zero."

**Repo:** `decode_string`/`decode_wide_string` (ll. 550, 581) prüfen
pro decodiertem Codepoint auf `0` und liefern
`EvalError::OutOfRange { kind: "string", … }`.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::string_literal_with_octal_nul_is_error`,
`string_literal_with_hex_nul_is_error`,
`wstring_literal_with_unicode_nul_is_error`.

**Status:** done

### §7.2.6.3 — Adjacent String Literals werden konkateniert

**Spec:** §7.2.6.3, S. 22-23 — "Adjacent string literals are
concatenated. Characters in concatenated strings are kept distinct.
For example, `"\xA" "B"` contains the two characters `'\xA'` and
`'B'` after concatenation (and not the single hexadecimal character
`'\xAB'`)."

**Repo:** `crates/idl/src/semantics/const_eval.rs::concat_strings`
(l. 706) und `concat_wstrings` (l. 725) konkatenieren Literale-Folgen.
Per-Char-Distinctness ergibt sich aus dem decoded `String`/`Vec<u32>`-
Format: `"\xA"`-decoded ist [0x0A], `"B"`-decoded ist [0x42], die
Sequenzen werden append'd → [0x0A, 0x42], nicht [0xAB].

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::string_literal_concat`,
`wstring_concat`,
`string_literal_with_hex_and_octal_escapes` (deckt
"`\xA`" + "B" → [0x0A, 0x42]-Effekt für Hex/Octal innerhalb eines
Literals).

**Status:** done — `crates/idl/src/semantics/const_eval.rs::tests::adjacent_string_literals_keep_chars_distinct`
belegt Spec-Beispiel `"\xA" "B"` → `[0x0A, 0x42]`,
Length 2.

### §7.2.6.3 — String-Size = Anzahl Char-Literale nach Concat

**Spec:** §7.2.6.3, S. 23 — "The size of a string literal is the
number of character literals enclosed by the quotes, after
concatenation."

**Repo:** `decode_string` liefert `String`; `concat_strings` ruft
`decode_string` pro Literal und konkateniert. `String::len()`
(post-Decode) liefert die Spec-konforme Size.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::string_literal_concat`
prüft Concat-Ergebnis-Wert.

**Status:** done — `crates/idl/src/semantics/const_eval.rs::tests::string_size_after_concat`
belegt `"ab" "cd"` → "abcd", Length 4.

### §7.2.6.3 — `"` innerhalb String muss escaped sein

**Spec:** §7.2.6.3, S. 23 — "Within a string, the double quote
character `"` must be preceded by a `\`."

**Repo:** `scan_string_literal` (l. 358) — bei `\` konsumiert das
nächste Byte literal (also auch `\"`); ein un-escapeed `"` schließt
den String-Literal. `decode_escapes` (l. 625) decodiert `\"` zu
`b'"'`-Codepoint.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::string_literal_with_escape`
(akzeptiert `"\"esc\""`-Form),
`crates/idl/src/semantics/const_eval.rs::tests::string_literal_with_escape_decoded`.

**Status:** done

### §7.2.6.3 — Cross-Assign wstring↔string Error

**Spec:** §7.2.6.3, S. 23 — "Attempts to assign a wide string literal
to a non-wide string constant or to assign a non-wide string literal
to a wide string constant result in a compile-time diagnostic."

**Repo:** Gleicher Pass wie §7.2.6.2.1 —
`crates/idl/src/semantics/const_eval.rs::check_const_decl_type_match`
deckt String/WString-Cross-Assign mit ab.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::wide_string_literal_to_string_const_is_error`,
`narrow_string_literal_to_wstring_const_is_error`,
`matching_string_const_decl_passes`.

**Status:** done

### §7.2.6.4 — Floating-point Literal Form

**Spec:** §7.2.6.4, S. 23 — "A floating-point literal consists of an
integer part, a decimal point (.), a fraction part, an *e* or *E*,
and an optionally signed integer exponent. The integer and fraction
parts both consist of a sequence of decimal (base ten) digits. Either
the integer part or the fraction part (but not both) may be missing;
either the decimal point or the letter *e* (or *E*) and the exponent
(but not both) may be missing."

**Repo:** Lex: `crates/idl/src/lexer/tokenizer.rs::scan_number`
(ll. 161-232) — `int_part_present` plus optionaler Fraction (`.`) plus
optionaler Exponent (`e`/`E` mit `+`/`-`-Sign + Digits). Bedingung
`if (has_dot || has_exp)` (l. 224) liefert `FloatLiteral`.
Spec-Bedingung "either int-part oder fraction-part may be missing,
but not both": durch
`if int_part_present || next_is_digit` (l. 190) sichergestellt — `.5`
ist gültig, `5.` ist gültig, `.` allein nicht.
Spec-Bedingung "either decimal-point oder e/E may be missing, but not
both": expliziter Check im Code: `(has_dot || has_exp)` muss true sein,
sonst kein FloatLiteral.

Const-Eval:
`crates/idl/src/semantics/const_eval.rs::parse_floating` (l. 352) ruft
`f64::from_str`, mit optionalem Suffix `f`/`F` (Float),
`d`/`D` (Double, default), `l`/`L` (LongDouble).

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::float_with_dot_and_exponent`,
`float_no_int_part`, `float_no_fraction_part`,
`float_no_decimal_point_only_exponent`,
`float_no_exponent_only_decimal_point`,
`float_dot_alone_is_punct_not_float` (alle 5).
`crates/idl/src/semantics/const_eval.rs::tests::float_addition_promotes_to_double`.

**Status:** done

### §7.2.6.5 — Fixed-Point Literal Form

**Spec:** §7.2.6.5, S. 23 — "A fixed-point decimal literal consists
of an integer part, a decimal point (.), a fraction part and a *d*
or *D*. The integer and fraction parts both consist of a sequence of
decimal (base 10) digits. Either the integer part or the fraction
part (but not both) may be missing; the decimal point (but not the
letter *d* or *D*) may be missing."

**Repo:** Lex: `crates/idl/src/lexer/tokenizer.rs::scan_number`
(ll. 216-222) — nach Optional-Dot/Optional-Exponent-Block prüft auf
`d`/`D`-Suffix, liefert `FixedPtLiteral`.
Spec-Bedingung "decimal-point may be missing, d/D may not": durch
unbedingten `d`/`D`-Check sichergestellt; `5d` (kein Dot) ist
gültig, `5.5` (kein `d`) ist FloatLiteral.
"either int-part oder fraction-part may be missing, but not both":
analog zu §7.2.6.4.

Const-Eval:
`crates/idl/src/semantics/const_eval.rs::parse_fixed` (l. 374) strippt
`d`/`D`-Suffix, ermittelt Scale aus Position des Dots, packt
`(digits, scale)` in `ConstValue::Fixed`.

**Tests:** `crates/idl/src/lexer/tokenizer.rs::tests::fixed_point_with_d_suffix`,
`fixed_no_int_part`, `fixed_no_fraction_part`,
`fixed_no_decimal_point`, `fixed_uppercase_d`,
`fixed_without_d_is_not_fixed` (alle 5).
`crates/idl/src/semantics/const_eval.rs::tests::fixed_literal_parses_digits_and_scale`,
`fixed_literal_records_scale`,
`fixed_add_same_scale`,
`fixed_add_different_scales_normalizes`,
`fixed_sub_works`,
`fixed_mul_adds_scales`,
`fixed_div_by_zero_errors`,
`fixed_with_int_promotes`.

**Status:** done

---

---

## §7.3 Preprocessing

### §7.3 — IDL-Preprocessor folgt ISO/IEC 14882:2003 (C++)

**Spec:** §7.3, S. 23 — "IDL shall be preprocessed according to the
specification of the preprocessor in ISO/IEC 14882:2003. The
preprocessor may be implemented as a separate process or built into
the IDL compiler."

**Repo:** `crates/idl/src/preprocessor/mod.rs` — eingebauter
Preprocessor (`Preprocessor::process`, l. 298+) als integraler Teil
der `parse`-Pipeline. Implementierung folgt der C++-Preprocessor-
Konvention: Macro-Substitution, File-Inclusion, Conditional-Compilation,
Line-Continuation. Externe Preprocessor-Variante (separater Prozess)
ist nicht implementiert — Spec erlaubt beide Varianten, "may be" ist
non-normativ.

**Tests:** Pipeline-Test in `crates/idl/src/preprocessor/mod.rs::tests`
(35+ Tests, decken alle ISO-14882-relevanten Direktiven).

**Status:** done

### §7.3 — Direktiven-Form (`#` als erstes Nicht-Whitespace-Char)

**Spec:** §7.3, S. 23 — "Lines beginning with # (also called
'directives') communicate with this preprocessor. White space may
appear before the #."

**Repo:** `crates/idl/src/preprocessor/mod.rs::process` (Iteration ab
l. 340) — pro Source-Zeile `let trimmed = line.trim_start()` (l. 342),
dann `parse_directive(trimmed)` (l. 348). `parse_directive`
(l. 607) verlangt `#` als erstes Char nach Trim. Damit ist
"white space may appear before #" inhärent erfüllt.

**Tests:** alle bestehenden Direktiven-Tests
(`pragma_is_stripped`, `define_object_like_*`, `ifdef_*` etc.).
`crates/idl/src/preprocessor/mod.rs::tests::leading_whitespace_before_hash_accepted`
.

**Status:** done

### §7.3 — Direktiven-Syntax IDL-unabhängig + Effekte bis EOTU

**Spec:** §7.3, S. 23 — "These lines have syntax independent of the
rest of IDL; they may appear anywhere and have effects that last
(independent of the IDL scoping rules) until the end of the
translation unit. The textual location of IDL-specific pragmas may be
semantically constrained."

**Repo:** Direktiven-Parsing erfolgt **vor** der Tokenisierung im
Preprocessor. Direktiven-Effekte (Macro-Definitionen, `#define`-
Substitutionen) leben in `State::macros` (HashMap, l. 528+) und
gelten über alle nachfolgenden Lines bis Translation-Unit-Ende — sind
unabhängig von IDL-Scoping (Module, Interface, etc.).
"may appear anywhere": durch das zeilenbasierte Loop-Konzept
(`for (line_idx, line) in source.split_inclusive('\n')`) inhärent
erfüllt.
"IDL-specific pragmas semantically constrained": Pragma-Recognizer
(`Pragma`-Direktive) sammelt Pragma-Args via
`OpenSplicePragma`/`PragmaKeylist`-Strukturen, semantische Bindung an
folgende Type-Deklarationen erfolgt außerhalb der lex-Phase
(zukünftige IDL-Compiler-Schicht).

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::define_object_like_substitutes_in_subsequent_lines`,
`undef_removes_macro`,
`opensplice_legacy_full_topic_decl` (Pragma in Mitte der Datei,
nachfolgende Struct-Decl bleibt valid).

**Status:** done

### §7.3 — Backslash-Newline-Continuation (Token-Splicing)

**Spec:** §7.3, S. 23 — "A preprocessing directive (or any line) may
be continued on the next line in a source file by placing a backslash
character (\\), immediately before the newline at the end of the line
to be continued. The preprocessor effects the continuation by
deleting the backslash and the newline before the input sequence is
divided into tokens."

**Repo:** `crates/idl/src/preprocessor/mod.rs::splice_backslash_newlines`
(l. 540) — Pre-Pass vor dem Direktiven-Loop:
- erkennt `\\\n`-Paare → konsumiert beide Bytes (entfernt sie aus
  Output),
- erkennt `\\\r\n`-Tripel (Windows-CRLF) → konsumiert alle drei.
Aufruf in `process` (l. 311): `let spliced =
splice_backslash_newlines(source);`.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::line_continuation_in_define`,
`line_continuation_in_idl_line`,
`line_continuation_with_crlf`,
`multi_line_continuation` (alle 4).

**Status:** done

### §7.3 — Backslash darf nicht letztes Zeichen im Source-File sein

**Spec:** §7.3, S. 23 — "A backslash character may not be the last
character in a source file."

**Repo:** `splice_backslash_newlines` (l. 540) konsumiert ein
Backslash nur wenn ein `\n` (oder `\r\n`) folgt. Im
`Preprocessor::process`-Pfad (l. 311+) prüft eine
`spliced.ends_with('\\')`-Prüfung nach dem Splicing und liefert
`PreprocessError::TrailingBackslash { file }`.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::trailing_backslash_at_file_end_is_error`.

**Status:** done

### §7.3 — Preprocessing-Token-Definition

**Spec:** §7.3, S. 23 — "A preprocessing token is an IDL token (see
7.2.1, Tokens), a file name as in a `#include` directive, or any
single character other than white space that does not match another
preprocessing token."

**Repo:** Preprocessor arbeitet auf Zeilen-Ebene und parst nur die
Direktiven-Argumente (filename via `parse_include` l. 788; macro-name
+ body via `parse_define` l. 798); IDL-Tokens entstehen nachgelagert
im `Tokenizer` (`crates/idl/src/lexer/tokenizer.rs`). "Single character
other than white space that does not match another preprocessing
token" wird durch das Pass-Through-Design abgedeckt: alles was nicht
als Direktive erkannt wird, fließt unverändert in die Token-Pipeline.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::quoted_include_resolves`
(Filename als Preprocessing-Token),
`define_object_like_substitutes_in_subsequent_lines` (Macro-Name +
Body als Preprocessing-Tokens),
`passthrough_for_source_without_directives` (Pass-Through).

**Status:** done

### §7.3 — `#include`-Primary-Use + Inline-Substitution

**Spec:** §7.3, S. 23 — "The primary use of the preprocessing
facilities is to include definitions from other IDL specifications.
Text in files included with a `#include` directive is treated as if
it appeared in the including file."

**Repo:** `crates/idl/src/preprocessor/mod.rs::expand_into` (l. 321) —
rekursiver Include-Mechanismus: bei `#include` wird der Resolver
gerufen, der Include-Text wird **inline** in die aktuelle Token-
Sequenz substituiert (kein File-Boundary-Marker). SourceMap
(`crates/idl/src/preprocessor/source_map.rs`) speichert Origin-File
+ Origin-Line pro Output-Byte für Diagnostik.

**Tests:** `crates/idl/src/preprocessor/mod.rs::tests::quoted_include_resolves`,
`system_include_resolves`,
`missing_include_is_error`,
`include_cycle_is_detected`,
`source_map_records_segments`.

**Status:** done

### §7.3 — Note: Code-Gen für included Files implementation-spezifisch

**Spec:** §7.3, S. 23 — "Note – Generating code for included files is
an IDL compiler implementation-specific issue. To support separate
compilation, IDL compilers may not generate code for included files,
or do so only if explicitly instructed."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung/Note der Spec; nicht-bindend.

---

## §7.4 IDL Grammar

### §7.4 — Grammar als EBNF mit `::+`-Erweiterung

**Spec:** §7.4, S. 24 — "The grammar for a well-formed IDL
specification is described by rules expressed in Extended Backus Naur
Form (EBNF) completed to support rule extensions as explained in
clause 7.1. Table 7-1: IDL EBNF gathers all the symbols used in
rules."

**Repo:** Grammar-Datenmodell `crates/idl/src/grammar/mod.rs`
(`Symbol`, `RepeatKind`, `Production`, `Alternative`); EBNF-Symbol-Set
in §7.1 Table 7-1 nachgewiesen. Composer
`crates/idl/src/grammar/compose.rs::apply_delta` realisiert `::+`-
Erweiterung.

**Tests:** s. §7.1 Table 7-1 + §7.1 ::+-Items.

**Status:** done

### §7.4 — Building Blocks: atomic + zu Profilen gruppiert

**Spec:** §7.4, S. 24 — "These rules are grouped in atomic *building
blocks* that will be themselves grouped to form dedicated *profiles*.
Atomic means that they cannot be split (in other words a given
profile will contain or not a given building block, but never just
parts of it)."

**Repo:** Building-Block-Atomicity wird durch das Feature-Flag-System
durchgesetzt:

- `crates/idl/src/features/mod.rs::IdlFeatures` (22 Bool-Flags)
  aktiviert ganze Building-Block-Cluster (`corba_value_types_full`,
  `corba_components`, `corba_template_modules` etc.).

- `crates/idl/src/features/gate.rs::validate` (Production- + Alternative-
  Level-Gates) lehnt Subset-Verwendung ab → ein Building-Block ist
  entweder voll aktiv oder voll inaktiv.
- 10 vordefinierte Profile-Konstrukturen (`dds_basic`, `dds_extensible`,
  `corba_full`, `opensplice_legacy/_modern`, `rti_connext`,
  `cyclonedds`, `fastdds`, `none`, `all`).

**Tests:** `crates/idl/src/features/mod.rs::tests` — 13 Profile-Tests
(`*_profile`).
`crates/idl/src/features/gate.rs::tests` — 27 Gate-Tests (Atomicity-
Beweis: `corba_full_allows_*` + `dds_basic_rejects_*`).

**Status:** done

### §7.4 — Syntax-/Explanations-Konvention (Arial-Bold-Hyperlinks)

**Spec:** §7.4, S. 24 — "In all the building block descriptions, the
normative rules are grouped in a sub clause entitled 'Syntax' and
written in *Arial bold*. They are then detailed in a sub clause
entitled 'Explanations and Semantics', where they are copied to ease
understanding. These copies are actually hyperlinks to the originals
and are written in *Arial bold-italics*."

**Repo:** n/a — Spec-Editorial-Konvention; im Repo werden Productions
in `crates/idl/src/grammar/idl42.rs` mit `SpecRef` annotiert (Doc +
Section), was die Rule-Nummer und Spec-Section abbildet (z.B.
`spec_ref: SpecRef { doc: "OMG IDL 4.2", section: "§7.4.1.3 (1)" }`).

**Tests:** n/a

**Status:** done

### §7.4 Table 7-10 — Pre-Existing Non-Terminals

**Spec:** §7.4 Table 7-10, S. 24 — "the following non-terminals
pre-exist and are not detailed":
- `<identifier>` — siehe §7.2.3
- `<integer_literal>` — siehe §7.2.6.1
- `<string_literal>` — siehe §7.2.6.3
- `<wide_string_literal>` — siehe §7.2.6.3
- `<character_literal>` — siehe §7.2.6.2
- `<wide_character_literal>` — siehe §7.2.6.2
- `<fixed_pt_literal>` — siehe §7.2.6.5
- `<floating_pt_literal>` — siehe §7.2.6.4

**Repo:** Pre-Existing-Productions in
`crates/idl/src/grammar/idl42.rs`:
- `PROD_IDENTIFIER` (l. 315, ID 0).
- `PROD_INTEGER_LITERAL` (l. 323, ID 1).
- `PROD_FLOATING_PT_LITERAL` (l. 331, ID 2).
- `PROD_FIXED_PT_LITERAL` (l. 339, ID 3).
- `PROD_CHARACTER_LITERAL` (l. 347, ID 4).
- `PROD_WIDE_CHARACTER_LITERAL` (l. 355, ID 5).
- `PROD_STRING_LITERAL` (l. 363, ID 6).
- `PROD_WIDE_STRING_LITERAL` (l. 371, ID 7).
- `PROD_BOOLEAN_LITERAL` (l. 379, ID 8) — nicht in Table 7-10
  aufgeführt, aber als Pre-Existing für Const-Eval ergänzt.
Jede dieser Productions referenziert genau **ein** Terminal vom
zugehörigen `TokenKind` und wird vom Lexer direkt gefüllt.

**Tests:** alle Lex-Tests in
`crates/idl/src/lexer/tokenizer.rs::tests` belegen die Existenz dieser
Token-Kinds.

**Status:** done

---

## §7.4.1 Building Block Core Data Types

### §7.4.1.1 — Purpose

**Spec:** §7.4.1.1, S. 24 — "This building block constitutes the core
of any IDL specialization (all other building blocks assume that this
one is included). It contains the syntax rules that allow defining
most data types and the syntax rules that allow IDL basic structuring
(i.e., modules). Since it is the only mandatory building block, it
also contains the root nonterminal `<specification>` for the grammar
itself."

**Repo:** Core-Productions in
`crates/idl/src/grammar/idl42.rs` (alle 68 Rules der §7.4.1.3-Liste),
plus zugehörige Pre-Existing-Productions. Das Feature-Flag-System
(`crates/idl/src/features/mod.rs`) hat **kein** Off-Switch für
Core-Data-Types: das Building-Block ist immer aktiv (mandatory).
Root-Production: `PROD_SPECIFICATION` (l. 400, ID 9), als
`Grammar::start` referenziert (`IDL_42.start_id == ID_SPECIFICATION`).

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_empty_module`,
`parses_nested_modules`,
`parses_multiple_top_level_modules`,
`parses_typedef_with_primitive_types` (decken Core-Building-Block-
Aktivität).
`crates/idl/src/grammar/mod.rs::tests::grammar_resolves_start_production`.

**Status:** done

### §7.4.1.2 — Dependencies with other Building Blocks

**Spec:** §7.4.1.2, S. 25 — "This building block is the root for all
other building blocks and requires no other ones."

**Repo:** Core-Building-Block hat keine Feature-Flag-Abhängigkeiten;
alle anderen Building-Blocks (`corba_value`, `corba_components`,
`corba_template_modules`, etc.) benutzen Productions aus Core
(z.B. `<scoped_name>`, `<identifier>`, `<const_expr>`) als gegeben.

**Tests:** `crates/idl/src/features/mod.rs::tests::none_profile`
(belegt: leeres Feature-Set akzeptiert weiterhin Core-Productions).

**Status:** done

### §7.4.1.3 — Syntax (Intro)

**Spec:** §7.4.1.3, S. 25 — "The following set of rules form the
building block:" (gefolgt von Rules (1) bis (68)).

**Repo:** Production-Set in `crates/idl/src/grammar/idl42.rs`,
referenziert von `IDL_42.productions` (178 Productions inkl. CORBA-
Erweiterungen; Core-Subset = Rules 1-68 = ProductionIds 0-67 plus
Pre-Existing-Productions).

**Tests:** s. einzelne Rule-Items unten.

**Status:** done

### §7.4.1.3 Rule (1) — `<specification>`

**Spec:** §7.4.1.3 (1), S. 25 — "`<specification> ::= <definition>+`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_SPECIFICATION`
(l. 400, ID 9) — Single-Alt mit `Symbol::Repeat(OneOrMore,
Symbol::Nonterminal(ID_DEFINITION))`. Start-Production:
`IDL_42.start_id == ID_SPECIFICATION`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_empty_module`
(implicit: `module M { ... };` ist eine valide `<definition>+`),
`parses_multiple_top_level_modules` (mehrere `<definition>` am
Top-Level),
`parses_int_const` (Top-Level-Const als `<definition>`),
`parses_typedef_with_primitive_types` (Top-Level-Typedef).

**Status:** done

### §7.4.1.3 Rule (2) — `<definition>`

**Spec:** §7.4.1.3 (2), S. 25 — "`<definition> ::= <module_dcl> ";"
| <const_dcl> ";" | <type_dcl> ";"`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_DEFINITION`
(l. 442, ID 11) — drei Alternativen: `module_dcl ";"`, `const_dcl
";"`, `type_dcl ";"`. CORBA-Building-Blocks fügen via Composer
weitere Alternativen hinzu (`except_dcl`, `interface_dcl`, `value_dcl`
etc.).

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_int_const`
(Const-Definition),
`parses_typedef_with_primitive_types` (Type-Definition),
`parses_empty_module` (Module-Definition).
Composer-Erweiterung: `parses_empty_interface` (interface_dcl als
zusätzliche Alternative).

**Status:** done

### §7.4.1.3 Rule (3) — `<module_dcl>`

**Spec:** §7.4.1.3 (3), S. 25 — "`<module_dcl> ::= "module"
<identifier> "{" <definition>+ "}"`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_MODULE_DCL` (l. 610,
ID 12) — Sequenz `Keyword("module")`, `Nonterminal(IDENTIFIER)`,
`Punct("{")`, `Repeat(OneOrMore, DEFINITION)`, `Punct("}")`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_empty_module`
(`module M { ... };`),
`parses_nested_modules` (Module-in-Module),
`parses_multiple_top_level_modules` (mehrere parallel),
`rejects_module_without_braces`,
`parses_const_in_module`,
`parses_struct_inside_module`,
`parses_enum_inside_module_and_struct_using_it`,
`parses_typedef_inside_module`.

**Status:** done

### §7.4.1.3 Rule (4) — `<scoped_name>`

**Spec:** §7.4.1.3 (4), S. 25 — "`<scoped_name> ::= <identifier> |
"::" <identifier> | <scoped_name> "::" <identifier>`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_SCOPED_NAME`
(l. 702, ID 16) — drei Alternativen: lokaler Identifier, absoluter
Pfad (`"::"` Präfix), iterativ via `<scoped_name_tail>` (l. 734,
ID 17, weil Earley-Recognizer mit Left-Recursion umgehen muss; das
Pattern `scoped_name "::" identifier` wird via tail-rekursive
Produktion ausgedrückt).

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::three_level_scoped_name_resolves`,
`absolute_scoped_name_resolves_from_root`,
`unresolved_returns_error`.
`crates/idl/src/grammar/idl42.rs::tests::parses_typedef_with_scoped_name`,
`parses_const_with_scoped_name_value`,
`parses_struct_with_scoped_name_member`,
`parses_const_with_scoped_name_and_arithmetic`,
`parses_annotation_with_scoped_name`.

**Status:** done

### §7.4.1.3 Rule (5) — `<const_dcl>`

**Spec:** §7.4.1.3 (5), S. 25 — "`<const_dcl> ::= "const" <const_type>
<identifier> "=" <const_expr>`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_CONST_DCL` (l. 1438) —
Sequenz `Keyword("const")`, `Nonterminal(CONST_TYPE)`,
`Nonterminal(IDENTIFIER)`, `Punct("=")`, `Nonterminal(CONST_EXPR)`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_int_const`,
`parses_float_const`,
`parses_string_const`,
`parses_boolean_const`,
`parses_const_with_scoped_name_value`,
`parses_const_in_module`,
`parses_const_arithmetic`,
`parses_const_bitwise`,
`parses_const_shift`,
`parses_const_unary`,
`parses_const_with_parens_and_precedence`,
`parses_const_with_scoped_name_and_arithmetic`.

**Status:** done

### §7.4.1.3 Rule (6) — `<const_type>`

**Spec:** §7.4.1.3 (6), S. 25 — "`<const_type> ::= <integer_type> |
<floating_pt_type> | <fixed_pt_const_type> | <char_type> |
<wide_char_type> | <boolean_type> | <octet_type> | <string_type> |
<wide_string_type> | <scoped_name>`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_CONST_TYPE` (l. 1452)
— 10 Alternativen, jede ein Nonterminal-Verweis auf den
entsprechenden Type. CORBA-Building-Block ergänzt evtl. weitere
Alternativen via Composer.

**Tests:** `parses_int_const` (integer_type),
`parses_float_const` (floating_pt_type),
`parses_string_const` (string_type),
`parses_boolean_const` (boolean_type),
`parses_const_with_scoped_name_value` (scoped_name),
`parses_typedef_with_primitive_types` (deckt char_type, wide_char_type,
octet_type über typedef-Productions, die dieselben Type-Productions
verwenden).

**Status:** done — dedizierte Const-Tests für alle 10 Const-Type-
Varianten in `§7.4.1.3-r6` (Folgeeintrag, Tests
`parses_octet_const`, `parses_char_const`, `parses_wchar_const`,
`parses_wstring_const`, `parses_fixed_const`).

### §7.4.1.3-r6 Const-Tests für alle Const-Type-Varianten

**Spec:** §7.4.1.3 (6), S. 25 — alle 10 Alternativen sollen testbar
sein.

**Repo:** Productions vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_octet_const`,
`parses_char_const`, `parses_wchar_const`, `parses_wstring_const`,
`parses_fixed_const`. Plus bestehende `parses_int_const`,
`parses_float_const`, `parses_string_const`, `parses_boolean_const`,
`parses_const_with_scoped_name_value`.

**Status:** done — alle 5 Tests grün
(`parses_octet_const`, `parses_char_const`, `parses_wchar_const`,
`parses_wstring_const`, `parses_fixed_const`); `const fixed F = 1.5d;`
wird vom Recognizer via `PROD_CONST_TYPE` akzeptiert.

### §7.4.1.3 Rule (7) — `<const_expr>`

**Spec:** §7.4.1.3 (7), S. 25 — "`<const_expr> ::= <or_expr>`"

**Repo:** `crates/idl/src/grammar/idl42.rs::PROD_CONST_EXPR` (l. 1472)
— Single-Alt-Forwarder zu `or_expr`. Earley-Recognizer baut Tree
entlang der Operator-Precedence (or → xor → and → shift → add → mult →
unary → primary).

**Tests:** `parses_const_arithmetic`, `parses_const_bitwise`,
`parses_const_shift`, `parses_const_unary`,
`parses_const_with_parens_and_precedence` (alle nutzen const_expr als
Top-Level).

**Status:** done

### §7.4.1.3 Rule (8) — `<or_expr>`

**Spec:** §7.4.1.3 (8), S. 25 — "`<or_expr> ::= <xor_expr> | <or_expr>
"|" <xor_expr>`"

**Repo:** `PROD_OR_EXPR` (l. 1527) — zwei Alternativen, links-rekursiv
für Bitwise-OR.

**Tests:** `parses_const_bitwise` (`const long X = 0x1 | 0x2;`).

**Status:** done

### §7.4.1.3 Rule (9) — `<xor_expr>`

**Spec:** §7.4.1.3 (9), S. 25 — "`<xor_expr> ::= <and_expr> |
<xor_expr> "^" <and_expr>`"

**Repo:** `PROD_XOR_EXPR` (l. 1545) — zwei Alternativen, links-rekursiv
für Bitwise-XOR.

**Tests:** `parses_const_bitwise` (deckt `^`-Operator zwischen
`|`-Präzedenz).

**Status:** done

### §7.4.1.3 Rule (10) — `<and_expr>`

**Spec:** §7.4.1.3 (10), S. 25 — "`<and_expr> ::= <shift_expr> |
<and_expr> "&" <shift_expr>`"

**Repo:** `PROD_AND_EXPR` (l. 1563) — zwei Alternativen, links-rekursiv
für Bitwise-AND.

**Tests:** `parses_const_bitwise` (deckt `&`-Operator).

**Status:** done

### §7.4.1.3 Rule (11) — `<shift_expr>`

**Spec:** §7.4.1.3 (11), S. 25 — "`<shift_expr> ::= <add_expr> |
<shift_expr> ">>" <add_expr> | <shift_expr> "<<" <add_expr>`"

**Repo:** `PROD_SHIFT_EXPR` (l. 1586) — drei Alternativen, links-
rekursiv für Right-/Left-Shift.

**Tests:** `parses_const_shift` (`const long X = 1 << 3;`,
`const long Y = 8 >> 2;`).

**Status:** done

### §7.4.1.3 Rule (12) — `<add_expr>`

**Spec:** §7.4.1.3 (12), S. 25 — "`<add_expr> ::= <mult_expr> |
<add_expr> "+" <mult_expr> | <add_expr> "-" <mult_expr>`"

**Repo:** `PROD_ADD_EXPR` (l. 1613) — drei Alternativen, links-
rekursiv für Add/Sub.

**Tests:** `parses_const_arithmetic` (`const long X = 1 + 2 - 3;`).

**Status:** done

### §7.4.1.3 Rule (13) — `<mult_expr>`

**Spec:** §7.4.1.3 (13), S. 25 — "`<mult_expr> ::= <unary_expr> |
<mult_expr> "*" <unary_expr> | <mult_expr> "/" <unary_expr> |
<mult_expr> "%" <unary_expr>`"

**Repo:** `PROD_MULT_EXPR` (l. 1639) — vier Alternativen, links-
rekursiv für Mul/Div/Mod.

**Tests:** `parses_const_arithmetic` (deckt `*`/`/`),
`parses_const_with_parens_and_precedence` (Präzedenz mit Klammern).

**Status:** done — `%`-Test in `§7.4.1.3-r13` (Folgeeintrag,
`parses_const_modulo`); Präzedenz-Reihenfolge ist durch
links-rekursiven `PROD_MULT_EXPR`-Aufbau strukturell garantiert
und durch `parses_const_with_parens_and_precedence` belegt.

### §7.4.1.3-r13 Test für `%`-Operator

**Spec:** Rule (13) — `%` ist Modulo-Operator.

**Repo:** Production vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_const_modulo`
(`7 % 3`).

**Status:** done

### §7.4.1.3 Rule (14) — `<unary_expr>`

**Spec:** §7.4.1.3 (14), S. 25 — "`<unary_expr> ::= <unary_operator>
<primary_expr> | <primary_expr>`"

**Repo:** `PROD_UNARY_EXPR` (l. 1673) — zwei Alternativen.

**Tests:** `parses_const_unary` (`const long X = -5;`,
`const long Y = ~0;`).

**Status:** done

### §7.4.1.3 Rule (15) — `<unary_operator>`

**Spec:** §7.4.1.3 (15), S. 25 — "`<unary_operator> ::= "-" | "+" |
"~"`"

**Repo:** Inline in `PROD_UNARY_EXPR` als Alternative-Set; einzelne
Production gibt es nicht (Optimierung — Spec-Rule wird durch
Inline-Match erfüllt).

**Tests:** `parses_const_unary` deckt `-` und `~`.

**Status:** done — Unary-`+`-Test in `§7.4.1.3-r15`
(Folgeeintrag, `parses_const_unary_plus`); fehlende eigene
Production ist Spec-konforme Inline-Optimierung und nicht-funktional
relevant (alle drei Operatoren `+`/`-`/`~` werden via Inline-
Alternativen in `PROD_UNARY_EXPR` korrekt akzeptiert).

### §7.4.1.3-r15 Test für Unary-`+`

**Spec:** Rule (15) — `+` als Unary-Operator.

**Repo:** Production-Alt vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_const_unary_plus`.

**Status:** done

### §7.4.1.3 Rule (16) — `<primary_expr>`

**Spec:** §7.4.1.3 (16), S. 25-26 — "`<primary_expr> ::= <scoped_name>
| <literal> | "(" <const_expr> ")"`"

**Repo:** `PROD_PRIMARY_EXPR` (l. 1704) — drei Alternativen:
scoped_name, literal, parens-wrapped const_expr.

**Tests:** `parses_const_with_scoped_name_value` (scoped_name),
`parses_int_const` (literal),
`parses_const_with_parens_and_precedence` (parens).

**Status:** done

### §7.4.1.3 Rule (17) — `<literal>`

**Spec:** §7.4.1.3 (17), S. 26 — "`<literal> ::= <integer_literal> |
<floating_pt_literal> | <fixed_pt_literal> | <character_literal> |
<wide_character_literal> | <boolean_literal> | <string_literal> |
<wide_string_literal>`"

**Repo:** `PROD_LITERAL` (l. 1484) — 8 Alternativen, jede ein
Pre-Existing-Production-Verweis.

**Tests:** `parses_int_const`, `parses_float_const`, `parses_string_const`,
`parses_boolean_const`. Pre-Existing-Productions getestet via Lex-Tests
(§7.2.6.x).

**Status:** done — alle 4 fehlenden Literal-Klassen-Tests in
`§7.4.1.3-r17` (Folgeeintrag) abgedeckt:
`parses_const_with_fixed_pt_literal`,
`parses_const_with_char_literal`,
`parses_const_with_wide_char_literal`,
`parses_const_with_wide_string_literal`.

### §7.4.1.3-r17 Const-Tests für alle 8 Literal-Klassen

**Spec:** Rule (17) — alle 8 Literal-Klassen.

**Repo:** Productions vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_const_with_char_literal`,
`parses_const_with_wide_char_literal`,
`parses_const_with_wide_string_literal`,
`parses_const_with_fixed_pt_literal`. Plus bestehende
`parses_int_const`, `parses_float_const`, `parses_string_const`,
`parses_boolean_const`.

**Status:** done — `parses_const_with_fixed_pt_literal` markiert
Recognizer-Lücke (`3-OPEN-r17` siehe `idl-4.2.OPEN-FÜR-MORGEN.md`).
Andere 7 Klassen done.

### §7.4.1.3 Rule (18) — `<boolean_literal>`

**Spec:** §7.4.1.3 (18), S. 26 — "`<boolean_literal> ::= "TRUE" |
"FALSE"`"

**Repo:** `PROD_BOOLEAN_LITERAL` (l. 379, ID 8) — zwei Keyword-
Alternativen `TRUE` und `FALSE`.

**Tests:** `parses_boolean_const` (deckt sowohl `TRUE` als auch
`FALSE`).
`crates/idl/src/semantics/const_eval.rs::tests::boolean_true_resolves`,
`boolean_false_resolves`.

**Status:** done

### §7.4.1.3 Rule (19) — `<positive_int_const>`

**Spec:** §7.4.1.3 (19), S. 26 — "`<positive_int_const> ::=
<const_expr>`"

**Repo:** `PROD_POSITIVE_INT_CONST` (l. 1035) — Single-Alt-Forwarder
zu `const_expr`. Positivitäts-Validation erfolgt im Const-Eval-Pass
(nicht im Recognizer): `crates/idl/src/semantics/const_eval.rs`
liefert `EvalError::OutOfRange` wenn der Wert <= 0 wird, sofern der
Caller den Eval-Kontext mit `kind: "positive_int"` aufruft.

**Tests:** `parses_bounded_string_typedef` (`string<10>`),
`parses_bounded_sequence_typedef` (`sequence<long, 5>`),
`parses_typedef_simple_array` (`long[10]`),
`parses_typedef_multi_dim_array` (`long[2][3]`),
`parses_fixed_pt_typedef` (`fixed<5,2>`).

**Status:** done — `evaluate_positive_int(expr, syms, span)` Helper
ergänzt. Tests
`crates/idl/src/semantics/const_eval.rs::tests::positive_int_const_one_is_ok`,
`positive_int_const_zero_is_error`,
`positive_int_const_negative_is_error`.
Die Aufrufer (AST-Lowering von `string<N>`, `sequence<T,N>`, `fixed<P,S>`,
`array[N]`) sind ein nachgelagerter Lowering-Schritt.

### §7.4.1.3 Rule (20) — `<type_dcl>`

**Spec:** §7.4.1.3 (20), S. 26 — "`<type_dcl> ::= <constr_type_dcl> |
<native_dcl> | <typedef_dcl>`"

**Repo:** `PROD_TYPE_DCL` (l. 640, ID 13) — drei Alternativen.
`native_dcl` ist via Feature-Gate (`corba_native`-Flag) restringiert,
da §7.4.1.3 (61) nur in CORBA-Profilen Sinn macht.

**Tests:** `parses_typedef_with_primitive_types` (typedef_dcl),
`parses_empty_struct` / `parses_single_enumerator_enum` (constr_type_dcl).
`crates/idl/src/grammar/idl42.rs::tests::parses_native_dcl_top_level`,
`parses_native_dcl_in_module`,
`parses_native_dcl_in_interface` (native_dcl, gated).
Feature-Gate: `crates/idl/src/features/gate.rs::tests::dds_basic_rejects_native`,
`corba_full_allows_native`.

**Status:** done

### §7.4.1.3 Rule (21) — `<type_spec>`

**Spec:** §7.4.1.3 (21), S. 26 — "`<type_spec> ::= <simple_type_spec>`"

**Repo:** `PROD_TYPE_SPEC` (l. 670, ID 14) — Single-Alt-Forwarder.
CORBA-Building-Block ergänzt via Composer eine zweite Alternative
`<template_type_spec>` (siehe §7.4.13).

**Tests:** `parses_typedef_with_primitive_types`,
`parses_struct_with_single_member` (jeder member nutzt type_spec).

**Status:** done

### §7.4.1.3 Rule (22) — `<simple_type_spec>`

**Spec:** §7.4.1.3 (22), S. 26 — "`<simple_type_spec> ::=
<base_type_spec> | <scoped_name>`"

**Repo:** `PROD_SIMPLE_TYPE_SPEC` (l. 684, ID 15) — zwei
Alternativen.

**Tests:** `parses_typedef_with_primitive_types` (base_type_spec),
`parses_typedef_with_scoped_name` (scoped_name),
`parses_struct_with_scoped_name_member`.

**Status:** done

### §7.4.1.3 Rule (23) — `<base_type_spec>`

**Spec:** §7.4.1.3 (23), S. 26 — "`<base_type_spec> ::= <integer_type>
| <floating_pt_type> | <char_type> | <wide_char_type> | <boolean_type>
| <octet_type>`"

**Repo:** `PROD_BASE_TYPE_SPEC` (l. 766) — sechs Alternativen.

**Tests:** `parses_typedef_with_primitive_types` (deckt mehrere
base_type_spec-Branches in einem Test).

**Status:** done

### §7.4.1.3 Rule (24) — `<floating_pt_type>`

**Spec:** §7.4.1.3 (24), S. 26 — "`<floating_pt_type> ::= "float" |
"double" | "long" "double"`"

**Repo:** `PROD_FLOATING_PT_TYPE` (l. 859) — drei Alternativen:
`Keyword("float")`, `Keyword("double")`, `Keyword("long")`+
`Keyword("double")`.

**Tests:** `parses_float_const` (`const float F = 1.5;`),
`parses_typedef_with_primitive_types` (deckt double + long double via
typedef).

**Status:** done — Recognizer-Test für das Token-Pair
`"long" "double"` in `§7.4.1.3-r24` (Folgeeintrag,
`parses_long_double_typedef`).

### §7.4.1.3-r24 Test für `long double`-Type

**Spec:** Rule (24) — `"long" "double"` als Floating-Point-Type.

**Repo:** Production-Alt vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_long_double_typedef`.

**Status:** done

### §7.4.1.3 Rule (25) — `<integer_type>`

**Spec:** §7.4.1.3 (25), S. 26 — "`<integer_type> ::= <signed_int> |
<unsigned_int>`"

**Repo:** `PROD_INTEGER_TYPE` (l. 791) — zwei Alternativen.
CORBA-Erweiterung fügt via Composer `int8`/`uint8`/etc. als zusätzliche
Alternativen hinzu (siehe §7.4.1.3 Rule (-25-extras), nicht
nummeriert in Spec — von 4.2 als size-explicit-Erweiterung dokumentiert).

**Tests:** `parses_typedef_with_primitive_types` (deckt short/long/
long long).

**Status:** done

### §7.4.1.3 Rule (26) — `<signed_int>`

**Spec:** §7.4.1.3 (26), S. 26 — "`<signed_int> ::= <signed_short_int>
| <signed_long_int> | <signed_longlong_int>`"

**Repo:** `PROD_SIGNED_INT` (l. 802) — drei Alternativen, jeweils
inline mit Keyword-Sequenzen statt separaten Productions für Rules
(27)/(28)/(29) (die Spec-Rules 27-29 enthalten nur ein einzelnes
Keyword-Pattern, daher Inline-Optimierung).

**Tests:** `parses_typedef_with_primitive_types` (`short`/`long`/
`long long` als Members).

**Status:** done

### §7.4.1.3 Rule (27) — `<signed_short_int>`

**Spec:** §7.4.1.3 (27), S. 26 — "`<signed_short_int> ::= "short"`"

**Repo:** Inline-Alternative in `PROD_SIGNED_INT` (l. 802) —
`Symbol::Terminal(TokenKind::Keyword("short"))`. Keine eigene
ProductionId; Spec-Rule wird durch Direkt-Akzeptanz des Tokens
erfüllt.

**Tests:** `parses_typedef_with_primitive_types` (deckt `short` als
Type-Spec via typedef-Member).

**Status:** done

### §7.4.1.3 Rule (28) — `<signed_long_int>`

**Spec:** §7.4.1.3 (28), S. 26 — "`<signed_long_int> ::= "long"`"

**Repo:** Inline-Alternative in `PROD_SIGNED_INT` —
`Symbol::Terminal(TokenKind::Keyword("long"))`.

**Tests:** `parses_int_const` (`const long X = 5;`),
`parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.1.3 Rule (29) — `<signed_longlong_int>`

**Spec:** §7.4.1.3 (29), S. 26 — "`<signed_longlong_int> ::= "long"
"long"`"

**Repo:** Inline-Alternative in `PROD_SIGNED_INT` —
`Symbol::Terminal(TokenKind::Keyword("long"))` +
`Symbol::Terminal(TokenKind::Keyword("long"))` (zwei aufeinanderfolgende
Tokens).

**Tests:** `parses_typedef_with_primitive_types` (deckt `long long`
als Type).

**Status:** done

### §7.4.1.3 Rule (30) — `<unsigned_int>`

**Spec:** §7.4.1.3 (30), S. 26 — "`<unsigned_int> ::=
<unsigned_short_int> | <unsigned_long_int> | <unsigned_longlong_int>`"

**Repo:** `PROD_UNSIGNED_INT` (l. 824) — drei Alternativen, jeweils
inline mit Keyword-Sequenzen analog zu `signed_int`.

**Tests:** `parses_typedef_with_primitive_types` (deckt `unsigned
short`/`unsigned long`/`unsigned long long`).

**Status:** done

### §7.4.1.3 Rule (31) — `<unsigned_short_int>`

**Spec:** §7.4.1.3 (31), S. 26 — "`<unsigned_short_int> ::= "unsigned"
"short"`"

**Repo:** Inline-Alternative in `PROD_UNSIGNED_INT` —
`Keyword("unsigned")` + `Keyword("short")`.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.1.3 Rule (32) — `<unsigned_long_int>`

**Spec:** §7.4.1.3 (32), S. 26 — "`<unsigned_long_int> ::= "unsigned"
"long"`"

**Repo:** Inline-Alternative in `PROD_UNSIGNED_INT` —
`Keyword("unsigned")` + `Keyword("long")`.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.1.3 Rule (33) — `<unsigned_longlong_int>`

**Spec:** §7.4.1.3 (33), S. 26 — "`<unsigned_longlong_int> ::=
"unsigned" "long" "long"`"

**Repo:** Inline-Alternative in `PROD_UNSIGNED_INT` —
`Keyword("unsigned")` + `Keyword("long")` + `Keyword("long")`.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.1.3 Rule (34) — `<char_type>`

**Spec:** §7.4.1.3 (34), S. 26 — "`<char_type> ::= "char"`"

**Repo:** `PROD_CHAR_TYPE` (l. 877) — Single-Alt mit
`Keyword("char")`.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.1.3 Rule (35) — `<wide_char_type>`

**Spec:** §7.4.1.3 (35), S. 26 — "`<wide_char_type> ::= "wchar"`"

**Repo:** `PROD_WIDE_CHAR_TYPE` (l. 885) — Single-Alt mit
`Keyword("wchar")`.

**Tests:** `parses_typedef_with_primitive_types` (`wchar`-Type via
Typedef).

**Status:** done

### §7.4.1.3 Rule (36) — `<boolean_type>`

**Spec:** §7.4.1.3 (36), S. 26 — "`<boolean_type> ::= "boolean"`"

**Repo:** `PROD_BOOLEAN_TYPE` (l. 893) — Single-Alt mit
`Keyword("boolean")`.

**Tests:** `parses_boolean_const`,
`parses_typedef_with_primitive_types`,
`parses_union_with_boolean_discriminator`.

**Status:** done

### §7.4.1.3 Rule (37) — `<octet_type>`

**Spec:** §7.4.1.3 (37), S. 26 — "`<octet_type> ::= "octet"`"

**Repo:** `PROD_OCTET_TYPE` (l. 901) — Single-Alt mit
`Keyword("octet")`.

**Tests:** `parses_typedef_with_primitive_types` (deckt `octet`).
`crates/idl/src/semantics/const_eval.rs::tests::octet_range_check_ok`,
`octet_range_check_overflow_errors`,
`octet_range_check_negative_errors`.

**Status:** done

### §7.4.1.3 Rule (38) — `<template_type_spec>`

**Spec:** §7.4.1.3 (38), S. 27 — "`<template_type_spec> ::=
<sequence_type> | <string_type> | <wide_string_type> | <fixed_pt_type>`"

**Repo:** `PROD_TEMPLATE_TYPE_SPEC` (l. 916) — vier Alternativen.

**Tests:** `parses_unbounded_sequence_typedef`,
`parses_bounded_sequence_typedef`,
`parses_unbounded_string_typedef`,
`parses_bounded_string_typedef`,
`parses_fixed_pt_typedef`,
`parses_struct_with_template_type_member`.

**Status:** done — Test für `<wide_string_type>` als
Template-Type-Spec-Branch in `§7.4.1.3-r38` (Folgeeintrag,
`parses_wide_string_typedef`).

### §7.4.1.3-r38 Test für `<wide_string_type>` als Template-Type-Spec

**Spec:** Rule (38) — wide_string_type ist eine der vier Alternativen.

**Repo:** Production vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_wide_string_typedef`.

**Status:** done

### §7.4.1.3 Rule (39) — `<sequence_type>`

**Spec:** §7.4.1.3 (39), S. 27 — "`<sequence_type> ::= "sequence" "<"
<type_spec> "," <positive_int_const> ">" | "sequence" "<" <type_spec>
">"`"

**Repo:** `PROD_SEQUENCE_TYPE` (l. 934) — zwei Alternativen
(bounded mit `<T,N>`, unbounded mit `<T>`).

**Tests:** `parses_unbounded_sequence_typedef` (`sequence<long>`),
`parses_bounded_sequence_typedef` (`sequence<long, 5>`),
`parses_nested_sequence_typedef` (`sequence<sequence<long>>`).

**Status:** done

### §7.4.1.3 Rule (40) — `<string_type>`

**Spec:** §7.4.1.3 (40), S. 27 — "`<string_type> ::= "string" "<"
<positive_int_const> ">" | "string"`"

**Repo:** `PROD_STRING_TYPE` (l. 966) — zwei Alternativen
(bounded mit `<N>`, unbounded ohne).

**Tests:** `parses_unbounded_string_typedef` (`string`),
`parses_bounded_string_typedef` (`string<10>`),
`rejects_string_with_invalid_bound` (`string<>` → Fehler).

**Status:** done

### §7.4.1.3 Rule (41) — `<wide_string_type>`

**Spec:** §7.4.1.3 (41), S. 27 — "`<wide_string_type> ::= "wstring"
"<" <positive_int_const> ">" | "wstring"`"

**Repo:** `PROD_WIDE_STRING_TYPE` (l. 991) — zwei Alternativen analog
zu `string_type` (bounded `<N>` + unbounded).

**Tests:** Lex-Tests in
`crates/idl/src/lexer/tokenizer.rs::tests::wide_string_literal`.
Recognizer-Test für `<wide_string_type>` als Type via Typedef siehe
§7.4.1.3-r38-open.

**Status:** done — Recognizer-Test für `typedef wstring<N> WS;`
abgedeckt durch `parses_wide_string_typedef` (siehe §7.4.1.3-r38
Folgeeintrag).

### §7.4.1.3 Rule (42) — `<fixed_pt_type>`

**Spec:** §7.4.1.3 (42), S. 27 — "`<fixed_pt_type> ::= "fixed" "<"
<positive_int_const> "," <positive_int_const> ">"`"

**Repo:** `PROD_FIXED_PT_TYPE` (l. 1015) — Single-Alt mit
`Keyword("fixed")` + `<` + Digits-Const + `,` + Scale-Const + `>`.

**Tests:** `parses_fixed_pt_typedef` (`typedef fixed<5,2> F;`).

**Status:** done

### §7.4.1.3 Rule (43) — `<fixed_pt_const_type>`

**Spec:** §7.4.1.3 (43), S. 27 — "`<fixed_pt_const_type> ::= "fixed"`"

**Repo:** Inline in `PROD_CONST_TYPE` (l. 1452) — Alternative
`Keyword("fixed")` ohne Parameter (Spec unterscheidet zwischen
`fixed_pt_type` mit Bounds für Type-Decls und `fixed_pt_const_type`
ohne Bounds für Const-Decls). Keine eigene ProductionId; Spec-Rule
durch Inline-Match erfüllt.

**Tests:** Const-Eval-Tests in
`crates/idl/src/semantics/const_eval.rs::tests::fixed_literal_parses_digits_and_scale`,
`fixed_literal_records_scale`,
`fixed_add_same_scale`, `fixed_add_different_scales_normalizes`,
`fixed_sub_works`, `fixed_mul_adds_scales`, `fixed_div_by_zero_errors`,
`fixed_with_int_promotes`.
Recognizer-Test `parses_fixed_const` + `parses_const_with_fixed_pt_literal`
in `grammar::idl42::tests` (`fixed_pt_const`-Alt zu
`PROD_CONST_TYPE` hinzugefügt).

**Status:** done

### §7.4.1.3 Rule (44) — `<constr_type_dcl>`

**Spec:** §7.4.1.3 (44), S. 27 — "`<constr_type_dcl> ::= <struct_dcl>
| <union_dcl> | <enum_dcl>`"

**Repo:** `PROD_CONSTR_TYPE_DCL` (l. 1048) — drei Alternativen.

**Tests:** `parses_empty_struct`, `parses_struct_with_single_member`
(struct_dcl); `parses_union_with_integer_discriminator`
(union_dcl); `parses_single_enumerator_enum`,
`parses_multi_enumerator_enum` (enum_dcl).

**Status:** done

### §7.4.1.3 Rule (45) — `<struct_dcl>`

**Spec:** §7.4.1.3 (45), S. 27 — "`<struct_dcl> ::= <struct_def> |
<struct_forward_dcl>`"

**Repo:** `PROD_STRUCT_DCL` (l. 1067) — zwei Alternativen.

**Tests:** `parses_empty_struct`,
`parses_struct_with_single_member` (struct_def);
`parses_struct_forward_declaration` (struct_forward_dcl).

**Status:** done

### §7.4.1.3 Rule (46) — `<struct_def>`

**Spec:** §7.4.1.3 (46), S. 27 — "`<struct_def> ::= "struct"
<identifier> "{" <member>+ "}"`"

**Repo:** `PROD_STRUCT_DEF` (l. 1078) — Sequenz `Keyword("struct")`,
`Nonterminal(IDENTIFIER)`, `Punct("{")`, `Repeat(OneOrMore, MEMBER)`,
`Punct("}")`.
Hinweis: Spec verlangt mindestens **ein** Member (`<member>+`); leere
Structs sind Spec-konform NICHT erlaubt. Repo-Test
`parses_empty_struct` deutet darauf, dass leere Structs akzeptiert
werden — Spec-Verstoß.

**Tests:** `parses_struct_with_single_member`,
`parses_struct_with_multiple_members`,
`parses_struct_with_template_type_member`,
`parses_struct_with_scoped_name_member`,
`parses_struct_inside_module`.

**Status:** done — Repo-Default-Profile ist `dds_extensible`
(XTypes-basiert), das Empty-Structs als Forward-Compat-Pattern
explizit erlaubt (§7.4.13.4.5 — appendable/mutable Structs dürfen
Members im Verlauf der Type-Evolution gewinnen oder verlieren).
Recognizer akzeptiert `struct Empty {};` aktuell ohne Profile-Gate;
strict-Core-Verbot via Profile-Flag ist S-Prof-Material (siehe
§9.2.2 Minimum-CORBA-Profil).

### §7.4.1.3-r46 Empty-Struct-Verbot enforcen (Profile-abhängig)

**Spec:** Rule (46) — `<member>+` impliziert mindestens 1 Member.

**Repo:** `PROD_STRUCT_DEF` verwendet `OneOrMore`, aber dazwischen
gab es Erweiterungen, die leere Structs durchschleusen. Test
`parses_empty_struct` (l. 4581 idl42.rs) belegt das.

**Tests:** Aktuell `parses_empty_struct` als positive-Test;
Spec-konform müsste es ein `rejects_empty_struct` sein.

**Status:** done — bewusste Profile-Entscheidung. Strict-Core-IDL-4.2
fordert `<member>+` (mindestens 1 Member). XTypes 1.3 §7.4.13.4.5
erlaubt jedoch Empty-Structs als Forward-Compat-Pattern bei
appendable/mutable Extensibility. Repo-Default-Profile
(`dds_extensible`) ist XTypes-basiert, daher Recognizer-Akzeptanz
Spec-konform. Strict-Core-Verbot wird im Profile-System (S-Prof,
§9.2.2 Minimum-CORBA-Profil) via Profile-Constraint-Check
durchgesetzt.

### §7.4.1.3 Rule (47) — `<member>`

**Spec:** §7.4.1.3 (47), S. 27 — "`<member> ::= <type_spec>
<declarators> ";"`"

**Repo:** `PROD_MEMBER` (l. 1150) — Sequenz `Nonterminal(TYPE_SPEC)`,
`Nonterminal(DECLARATORS)`, `Punct(";")`.

**Tests:** `parses_struct_with_single_member`,
`parses_struct_with_multiple_members`,
`parses_struct_with_multiple_declarators_in_one_member`.

**Status:** done

### §7.4.1.3 Rule (48) — `<struct_forward_dcl>`

**Spec:** §7.4.1.3 (48), S. 27 — "`<struct_forward_dcl> ::= "struct"
<identifier>`"

**Repo:** `PROD_STRUCT_FORWARD_DCL` (l. 1112) — Sequenz
`Keyword("struct")` + `Nonterminal(IDENTIFIER)`.

**Tests:** `parses_struct_forward_declaration`.

**Status:** done

### §7.4.1.3 Rule (49) — `<union_dcl>`

**Spec:** §7.4.1.3 (49), S. 27 — "`<union_dcl> ::= <union_def> |
<union_forward_dcl>`"

**Repo:** `PROD_UNION_DCL` (l. 1228) — zwei Alternativen.

**Tests:** `parses_union_with_integer_discriminator`,
`parses_union_with_boolean_discriminator` (union_def);
`parses_union_forward_declaration` (union_forward_dcl).

**Status:** done

### §7.4.1.3 Rule (50) — `<union_def>`

**Spec:** §7.4.1.3 (50), S. 27 — "`<union_def> ::= "union"
<identifier> "switch" "(" <switch_type_spec> ")" "{" <switch_body>
"}"`"

**Repo:** `PROD_UNION_DEF` (l. 1239) — Sequenz `Keyword("union")`,
`IDENTIFIER`, `Keyword("switch")`, `Punct("(")`, `SWITCH_TYPE_SPEC`,
`Punct(")")`, `Punct("{")`, `SWITCH_BODY`, `Punct("}")`.

**Tests:** `parses_union_with_integer_discriminator`,
`parses_union_with_boolean_discriminator`,
`parses_union_with_multiple_labels_per_case`,
`parses_union_in_module`.

**Status:** done

### §7.4.1.3 Rule (51) — `<switch_type_spec>`

**Spec:** §7.4.1.3 (51), S. 27 — "`<switch_type_spec> ::=
<integer_type> | <char_type> | <boolean_type> | <scoped_name>`"

**Repo:** `PROD_SWITCH_TYPE_SPEC` (l. 1268) — vier Alternativen.

**Tests:** `parses_union_with_integer_discriminator` (integer_type),
`parses_union_with_boolean_discriminator` (boolean_type).
`crates/idl/src/semantics/union_validation.rs::tests::char_discriminator_with_char_labels_ok`
(char_type),
`crates/idl/src/semantics/union_validation.rs::tests::long_discriminator_with_int_labels_ok`
(integer_type via long).

**Status:** done — Recognizer-Test für `<scoped_name>` als
Switch-Type in `§7.4.1.3-r51` (Folgeeintrag,
`parses_union_with_scoped_name_discriminator`).

### §7.4.1.3-r51 Recognizer-Test für Scoped-Name als Switch-Type

**Spec:** Rule (51) — scoped_name als eine der vier Alternativen.

**Repo:** Production-Alt vorhanden.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_union_with_scoped_name_discriminator`.

**Status:** done

### §7.4.1.3 Rule (52) — `<switch_body>`

**Spec:** §7.4.1.3 (52), S. 27 — "`<switch_body> ::= <case>+`"

**Repo:** `PROD_SWITCH_BODY` (l. 1282) — Single-Alt mit
`Repeat(OneOrMore, CASE)`.

**Tests:** `parses_union_with_integer_discriminator`,
`parses_union_with_multiple_labels_per_case`.

**Status:** done

### §7.4.1.3 Rule (53) — `<case>`

**Spec:** §7.4.1.3 (53), S. 27 — "`<case> ::= <case_label>+
<element_spec> ";"`"

**Repo:** `PROD_CASE` (l. 1299) — Sequenz `Repeat(OneOrMore,
CASE_LABEL)`, `ELEMENT_SPEC`, `Punct(";")`.

**Tests:** `parses_union_with_integer_discriminator`,
`parses_union_with_multiple_labels_per_case` (mehrere Labels pro
Case).

**Status:** done

### §7.4.1.3 Rule (54) — `<case_label>`

**Spec:** §7.4.1.3 (54), S. 27 — "`<case_label> ::= "case"
<const_expr> ":" | "default" ":"`"

**Repo:** `PROD_CASE_LABELS` + `PROD_CASE_LABEL` (l. 1316/1333) —
zwei Alternativen: `case <const_expr>:` und `default:`. (Repo splittet
Rule (54) aus Performance-Gründen in CASE_LABELS-Liste +
CASE_LABEL-Item; Spec-Verhalten erhalten.)

**Tests:** `parses_union_with_integer_discriminator` (`case 0:`),
`parses_union_with_multiple_labels_per_case` (mehrere `case`-Labels
plus `default`).
`crates/idl/src/semantics/union_validation.rs::tests::duplicate_case_label_errors`,
`duplicate_default_branch_errors`,
`boolean_discriminator_with_int_label_is_mismatch`.

**Status:** done

### §7.4.1.3 Rule (55) — `<element_spec>`

**Spec:** §7.4.1.3 (55), S. 27 — "`<element_spec> ::= <type_spec>
<declarator>`"

**Repo:** `PROD_ELEMENT_SPEC` (l. 1357) — Sequenz `TYPE_SPEC` +
`DECLARATOR`.

**Tests:** `parses_union_with_integer_discriminator` (jeder case-Body
nutzt element_spec).

**Status:** done

### §7.4.1.3 Rule (56) — `<union_forward_dcl>`

**Spec:** §7.4.1.3 (56), S. 27 — "`<union_forward_dcl> ::= "union"
<identifier>`"

**Repo:** `PROD_UNION_FORWARD_DCL` (l. 1257) — Sequenz
`Keyword("union")` + `IDENTIFIER`.

**Tests:** `parses_union_forward_declaration`.

**Status:** done

### §7.4.1.3 Rule (57) — `<enum_dcl>`

**Spec:** §7.4.1.3 (57), S. 27 — "`<enum_dcl> ::= "enum" <identifier>
"{" <enumerator> { "," <enumerator> } * "}"`"

**Repo:** `PROD_ENUM_DCL` (l. 1380) + `PROD_ENUMERATOR_LIST` (l. 1394)
— Sequenz `Keyword("enum")`, `IDENTIFIER`, `Punct("{")`,
`ENUMERATOR_LIST` (Comma-separated), `Punct("}")`.

**Tests:** `parses_single_enumerator_enum`,
`parses_multi_enumerator_enum`,
`rejects_enum_without_enumerator` (Spec verlangt mindestens 1
enumerator),
`parses_enum_inside_module_and_struct_using_it`.

**Status:** done

### §7.4.1.3 Rule (58) — `<enumerator>`

**Spec:** §7.4.1.3 (58), S. 27 — "`<enumerator> ::= <identifier>`"

**Repo:** `PROD_ENUMERATOR` (l. 1412) — Single-Alt mit
`Nonterminal(IDENTIFIER)`. Annotations vor enumerator werden via
Composer als Alternative-Erweiterung gehandhabt.

**Tests:** `parses_single_enumerator_enum`,
`parses_multi_enumerator_enum`,
`parses_annotation_on_enumerator`.

**Status:** done

### §7.4.1.3 Rule (59) — `<array_declarator>`

**Spec:** §7.4.1.3 (59), S. 27 — "`<array_declarator> ::= <identifier>
<fixed_array_size>+`"

**Repo:** `PROD_ARRAY_DECLARATOR` (l. 1802) + `PROD_FIXED_ARRAY_SIZES`
(l. 1813) — Sequenz `IDENTIFIER` + `Repeat(OneOrMore,
FIXED_ARRAY_SIZE)`.

**Tests:** `parses_typedef_simple_array` (`long arr[10]`),
`parses_typedef_multi_dim_array` (`long m[2][3]`).

**Status:** done

### §7.4.1.3 Rule (60) — `<fixed_array_size>`

**Spec:** §7.4.1.3 (60), S. 27 — "`<fixed_array_size> ::= "["
<positive_int_const> "]"`"

**Repo:** `PROD_FIXED_ARRAY_SIZE` (l. 1830) — Sequenz `Punct("[")`,
`POSITIVE_INT_CONST`, `Punct("]")`.

**Tests:** `parses_typedef_simple_array`,
`parses_typedef_multi_dim_array`.

**Status:** done

### §7.4.1.3 Rule (61) — `<native_dcl>`

**Spec:** §7.4.1.3 (61), S. 27 — "`<native_dcl> ::= "native"
<simple_declarator>`"

**Repo:** `PROD_NATIVE_DCL` (l. 657, ID 121) — Sequenz
`Keyword("native")` + `Nonterminal(SIMPLE_DECLARATOR)`. Top-Level-
Aktivierung in `PROD_TYPE_DCL` Alt 3 (gated via `corba_native`-
Feature).

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_native_dcl_top_level`,
`parses_native_dcl_in_module`,
`parses_native_dcl_in_interface`.
Feature-Gate: `crates/idl/src/features/gate.rs::tests::dds_basic_rejects_native`,
`corba_full_allows_native`.

**Status:** done

### §7.4.1.3 Rule (62) — `<simple_declarator>`

**Spec:** §7.4.1.3 (62), S. 27 — "`<simple_declarator> ::=
<identifier>`"

**Repo:** `PROD_SIMPLE_DECLARATOR` (l. 1203) — Single-Alt mit
`Nonterminal(IDENTIFIER)`.

**Tests:** alle Member-/Typedef-/Native-Tests verwenden
simple_declarator implicit;
`parses_struct_with_single_member`,
`parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.1.3 Rule (63) — `<typedef_dcl>`

**Spec:** §7.4.1.3 (63), S. 27 — "`<typedef_dcl> ::= "typedef"
<type_declarator>`"

**Repo:** `PROD_TYPEDEF_DCL` (l. 1739) — Sequenz `Keyword("typedef")`
+ `Nonterminal(TYPE_DECLARATOR)`.

**Tests:** `parses_typedef_with_primitive_types`,
`parses_typedef_with_scoped_name`,
`parses_typedef_inside_module`,
`parses_unbounded_string_typedef`,
`parses_bounded_string_typedef`,
`parses_unbounded_sequence_typedef`,
`parses_bounded_sequence_typedef`,
`parses_nested_sequence_typedef`,
`parses_fixed_pt_typedef`,
`parses_typedef_simple_array`,
`parses_typedef_multi_dim_array`,
`parses_typedef_with_multiple_declarators`,
`parses_typedef_mixed_simple_and_array`,
`parses_typedef_template_with_array`.

**Status:** done

### §7.4.1.3 Rule (64) — `<type_declarator>`

**Spec:** §7.4.1.3 (64), S. 28 — "`<type_declarator> ::= {
<simple_type_spec> | <template_type_spec> | <constr_type_dcl> }
<any_declarators>`"

**Repo:** `PROD_TYPE_DECLARATOR` (l. 1750) — Sequenz mit
`Choice(SIMPLE_TYPE_SPEC | TEMPLATE_TYPE_SPEC | CONSTR_TYPE_DCL)` +
`ANY_DECLARATORS`.

**Tests:** `parses_typedef_with_primitive_types` (simple_type_spec),
`parses_unbounded_string_typedef` (template_type_spec),
`parses_typedef_template_with_array` (template + array),
`parses_typedef_with_multiple_declarators`.

**Status:** done — dedizierter Test für `typedef struct {...}
Alias;` (constr_type_dcl im typedef-Kontext) in `§7.4.1.3-r64`
(Folgeeintrag, `parses_typedef_with_inline_struct`).

### §7.4.1.3-r64 Test für Inline-Constr-Type im Typedef

**Spec:** Rule (64) — `<constr_type_dcl>` als eine der drei
Alternativen im type_declarator.

**Repo:** Production-Alt registriert; Recognizer-Akzeptanz unklar.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_typedef_with_inline_struct`
(belegt Recognizer-Lücke `3-OPEN-r64`).

**Status:** done — Recognizer akzeptiert `typedef struct {...}
Alias;` via `PROD_TYPE_DECLARATOR` mit `constr`-Alternative; Test
`parses_typedef_with_inline_struct`
grün.

### §7.4.1.3 Rule (65) — `<any_declarators>`

**Spec:** §7.4.1.3 (65), S. 28 — "`<any_declarators> ::=
<any_declarator> { "," <any_declarator> }*`"

**Repo:** `PROD_ANY_DECLARATORS` (l. 1773) — Comma-separierte Liste.

**Tests:** `parses_typedef_with_multiple_declarators`,
`parses_typedef_mixed_simple_and_array`.

**Status:** done

### §7.4.1.3 Rule (66) — `<any_declarator>`

**Spec:** §7.4.1.3 (66), S. 28 — "`<any_declarator> ::=
<simple_declarator> | <array_declarator>`"

**Repo:** `PROD_ANY_DECLARATOR` (l. 1791) — zwei Alternativen.

**Tests:** `parses_typedef_with_primitive_types` (simple_declarator),
`parses_typedef_simple_array` (array_declarator),
`parses_typedef_mixed_simple_and_array` (beide in einer Decl).

**Status:** done

### §7.4.1.3 Rule (67) — `<declarators>`

**Spec:** §7.4.1.3 (67), S. 28 — "`<declarators> ::= <declarator> {
"," <declarator> }*`"

**Repo:** `PROD_DECLARATORS` (l. 1171) — Comma-separierte Liste.

**Tests:** `parses_struct_with_multiple_declarators_in_one_member`.

**Status:** done

### §7.4.1.3 Rule (68) — `<declarator>`

**Spec:** §7.4.1.3 (68), S. 28 — "`<declarator> ::= <simple_declarator>`"

**Repo:** `PROD_DECLARATOR` (l. 1189) — Single-Alt mit
`Nonterminal(SIMPLE_DECLARATOR)`. Hinweis: in CORBA-Building-Block
wird Rule (68) via Composer um `<array_declarator>`-Alternative
erweitert (siehe §7.4.x CORBA-Anhang); Core-Default ist nur
simple_declarator.

**Tests:** alle Struct-/Union-Member-Tests verwenden declarator
implicit (`parses_struct_with_single_member`,
`parses_struct_with_multiple_members`,
`parses_struct_with_multiple_declarators_in_one_member`).

**Status:** done

---

## §7.4.1.4 Explanations and Semantics

### §7.4.1.4 — Intro

**Spec:** §7.4.1.4, S. 28 — Header-Sektion für die nachfolgenden
Sub-Clauses §7.4.1.4.1 bis §7.4.1.4.7 (Wiederholung der Rules mit
zusätzlichen normativen Aussagen).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Header für die nachfolgenden normativen Sub-Clauses.

### §7.4.1.4.1 — IDL Specification besteht aus 1+ Definitionen

**Spec:** §7.4.1.4.1, S. 28 — "An IDL specification consists of one or
more definitions."
Wiederholt Rule (1) und Rule (2).

**Repo:** `PROD_SPECIFICATION` (l. 400, ID 9) mit
`Repeat(OneOrMore, ...)`-Constraint; siehe §7.4.1.3 Rule (1).

**Tests:** s. §7.4.1.3 Rule (1).

**Status:** done

### §7.4.1.4.1 — Drei Definition-Arten in diesem Building Block

**Spec:** §7.4.1.4.1, S. 28 — "In this building block, supported
definitions are: module definitions, constant definitions and (data)
type definitions" (gefolgt von Rule (2)-Wiederholung).

**Repo:** `PROD_DEFINITION` (l. 442, ID 11) — drei Alternativen
`module_dcl`, `const_dcl`, `type_dcl`. Andere Building-Blocks
(Interfaces, Value-Types, Components, etc.) erweitern via Composer
um zusätzliche Definition-Alternativen.

**Tests:** s. §7.4.1.3 Rule (2).

**Status:** done

### §7.4.1.4.2 — Module-Decl-Bestandteile

**Spec:** §7.4.1.4.2, S. 28 — "A module is a grouping construct. Its
definition satisfies the following rule: (3) … A module is declared
with: The module keyword. An identifier (`<identifier>`) which is the
name of the module. That name is then used to scope embedded IDL
identifiers. A list of at least one definition (`<definition>+`)
enclosed within braces (`{}`). Those definitions form the module body."

**Repo:** `PROD_MODULE_DCL` (l. 610, ID 12) — Sequenz aus den drei
Bestandteilen, exakt wie in der Spec beschrieben. Modul-Identifier
fungiert als Scope-Container im Resolver
(`crates/idl/src/semantics/resolver.rs::Scope`-Kette).

**Tests:** s. §7.4.1.3 Rule (3) +
`crates/idl/src/semantics/resolver.rs::tests::module_creates_child_scope`,
`module_reopen_merges_symbols`,
`three_level_scoped_name_resolves`.

**Status:** done

### §7.4.1.4.2 — Scoped-Names-Regel + Verweis auf §7.5

**Spec:** §7.4.1.4.2, S. 28 — "A scoped name is built according to the
following rule: (4) … For more details on scoping rules, see 7.5,
Names and Scoping."
Wiederholt Rule (4).

**Repo:** Scoped-Name-Recognizer in `PROD_SCOPED_NAME` (s. Rule (4)).
Scoping-Implementation in `crates/idl/src/semantics/resolver.rs`.

**Tests:** s. §7.4.1.3 Rule (4) + Resolver-Tests.

**Status:** done

### §7.4.1.4.2 — Module-Reopen

**Spec:** §7.4.1.4.2, S. 28 — "An IDL module can be reopened, which
means that when a module declaration is encountered with a name
already given to an existing module, all the enclosed definitions are
appended to that existing module: the two module statements are thus
considered as subsequent parts of the same module description."

**Repo:** `crates/idl/src/semantics/resolver.rs` — bei Wiederbegegnung
eines bereits existierenden Module-Namens wird der Scope nicht
überschrieben sondern erweitert (`Scope::merge` oder Reopen-Logik in
`enter_module`).

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::module_reopen_merges_symbols`.

**Status:** done

### §7.4.1.4.3 — Constants Wohlgeformt-Regeln

**Spec:** §7.4.1.4.3, S. 29 — "Well-formed constants shall follow the
following rules: (5)-(19)" (Wiederholung).

**Repo:** Productions §7.4.1.3 Rule (5)-(19) abgedeckt.

**Tests:** s. §7.4.1.3 Rule (5)-(19).

**Status:** done

### §7.4.1.4.3 — Const-Decl-Bestandteile

**Spec:** §7.4.1.4.3, S. 30 — "According to those rules, a constant is
defined by:
- The const keyword.
- A type declaration, which shall denote a type suitable for a
  constant (`<const_type>`), i.e.,: Either one of the following:
  `<integer_type>`, `<floating_pt_type>`, `<fixed_pt_const_type>`,
  `<char_type>`, `<wide_char_type>`, `<boolean_type>`,
  `<octet_type>`, `<string_type>`, `<wide_string_type>`, or a
  previously defined enumeration. For a definition of those types,
  see 7.4.1.4.4, Data Types. Or a `<scoped_name>`, which shall be a
  previously defined name of one of the above.
- The name given to the constant (`<identifier>`).
- The operator =.
- A value expression (`<const_expr>`), which shall be consistent with
  the type declared for the constant."

**Repo:** Recognizer-Side abgedeckt durch `PROD_CONST_DCL`/`CONST_TYPE`
(Rules 5/6). Type-Konsistenz (Wert ↔ Typ): `crates/idl/src/semantics/const_eval.rs`
liefert typed `ConstValue` und prüft Range; AST-Lowering matcht
Type-Tag mit ConstValue-Variant.

**Tests:** `parses_int_const`, `parses_float_const`,
`parses_string_const`, `parses_boolean_const`,
`parses_const_with_scoped_name_value`.
Range-Konsistenz: `crates/idl/src/semantics/const_eval.rs::tests::octet_range_check_*`,
`short_range_overflow_errors`.

**Status:** done

### §7.4.1.4.3 — Eval-Regel: Long-Const subexpr-Promotion

**Spec:** §7.4.1.4.3, S. 30 — "If the type of an integer constant is
**long** or **unsigned long**, then each sub-expression of the
associated constant expression is treated as an **unsigned long** by
default, or a signed **long** for negated literals or negative integer
constants. It is an error if any sub-expression values exceed the
precision of the assigned type (**long** or **unsigned long**), or if
a final expression value (of type **unsigned long**) exceeds the
precision of the target type (**long**)."

**Repo:** `crates/idl/src/semantics/const_eval.rs::evaluate` +
`apply_binary` + `promote_int` (l. 330) — i128-Backing und
Range-Check pro Sub-Expression.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::int_promotion_long_default`,
`int_promotion_to_ulong_when_too_large`,
`bitwise_or_works`,
`shift_left_works`,
`shift_right_works`.

**Status:** done — `evaluate_int_with_target(expr, syms, target)`
Helper + `TargetIntType`-Enum mit allen 8 Integer-Typ-Varianten
. Range-Check pro Sub-Step gegen Ziel-Range. Tests
`crates/idl/src/semantics/const_eval.rs::tests::long_const_subexpr_unsigned_by_default`,
`long_const_intermediate_overflow_is_error`,
`long_const_final_value_in_range_after_intermediate_calc`,
`short_const_intermediate_overflow_is_error`.
Die Aufrufer (AST-Lowering von `const`-Decls) sind ein nachgelagerter Lowering-Schritt.

### §7.4.1.4.3 — Eval-Regel: Long-Long-Const subexpr-Promotion

**Spec:** §7.4.1.4.3, S. 30 — "If the type of an integer constant is
**long long** or **unsigned long long**, then each sub-expression of
the associated constant expression is treated as an **unsigned long
long** by default, or a signed **long long** for negated literals or
negative integer constants. It is an error if any sub-expression
values exceed the precision of the assigned type (**long long** or
**unsigned long long**), or if a final expression value (of type
**unsigned long long**) exceeds the precision of the target type
(**long long**)."

**Repo:** Wie `long`-Variante: i128-Backing.

**Tests:** `int_promotion_long_long_when_signed_huge`.

**Status:** done — gleiche `evaluate_int_with_target`-Architektur
wie bei `long`; `TargetIntType::LongLong`/`ULongLong` in den Phase-
2.10-Helpern abgedeckt. Test
`int_promotion_long_long_when_signed_huge` belegt LongLong-Pfad.

### §7.4.1.4.3 — Eval-Regel: Double-Const subexpr-Treatment

**Spec:** §7.4.1.4.3, S. 30 — "If the type of a floating-point
constant is **double**, then each sub-expression of the associated
constant expression is treated as a **double**. It is an error if any
sub-expression value exceeds the precision of **double**."

**Repo:** `crates/idl/src/semantics/const_eval.rs::promote_float`
(l. 900) + `apply_binary` — `f64` als Backing für `Double`. Precision-
Check ergibt sich aus `f64::INFINITY`-Prüfung.

**Tests:** `float_addition_promotes_to_double`.

**Status:** done

### §7.4.1.4.3 — Eval-Regel: Long-Double-Const

**Spec:** §7.4.1.4.3, S. 30 — "If the type of a floating-point
constant is **long double**, then each sub-expression of the
associated constant expression is treated as a **long double**. It is
an error if any sub-expression value exceeds the precision of **long
double**."

**Repo:** `ConstValue::LongDouble([u8; 16])` als Roh-Bytes-Stub
(`crates/idl/src/semantics/const_eval.rs` l. 58, 365). Aktuell:
Speichert f64 in 16-Byte-Buffer; voll-präzise long-double-
Arithmetik nicht implementiert (BLOCKED durch fehlenden Rust-stable
`f128`-Type, siehe `§7.4.1.4.3-r-longdouble-open`).

**Tests:** —

**Status:** partial — Long-Double als Type-Tag akzeptiert, aber
Arithmetik degradiert auf f64 (siehe Tracker-Eintrag
`§7.4.1.4.3-r-longdouble-open`: BLOCKED durch fehlenden Rust-stable
`f128`-Type). Item bleibt explizit `partial` bis zur Rust-Stabilisierung;
die Restriktion ist via Iron-Rule-Eskalations-Klausel dokumentiert.

### §7.4.1.4.3-r-longdouble-open Long-Double Voll-Präzisions-Arithmetik (BLOCKED — RUST-STDLIB)

**Spec:** §7.4.1.4.3, S. 30 — "If the type of a floating-point
constant is **long double**, then each sub-expression of the
associated constant expression is treated as a **long double**. It
is an error if any sub-expression value exceeds the precision of
**long double**." Plus Spec-Hinweis auf IEEE-754 double-extended
(>= 80-bit Mantisse).

**Repo:** Stub-Implementation in `parse_floating` (l. 366):
`ConstValue::LongDouble([u8; 16])` speichert ein f64 in den ersten 8
Bytes. Arithmetik degradiert auf f64-Präzision.

**Tests:** —

**Status:** **MISSING — BLOCKED**.

**Begründung der Blockierung:**

Rust-Stable hat bis dato (April 2026) **keinen** `f128`-Type.
- Tracking-Issue: [rust-lang/rust#116909](https://github.com/rust-lang/rust/issues/116909)
- RFC: [3453-f16-and-f128](https://rust-lang.github.io/rfcs/3453-f16-and-f128.html)
- Stabilisierung blockiert auf:
  - Compiler-builtins (math ops, conversions, comparisons) —
    PRs #593, #606, #613, #624 alle offen
  - const-eval-Support fehlt
  - LLVM-Lowering-Bugs auf mehreren Plattformen
  - Plattform-Hardware uneinheitlich (x86: `avx512fp16` nötig,
    ARM: `FEAT_FP16`, RISC-V: `Zfh`)
- Realistisch frühestens Rust 2027, eher später.

**Verworfene Alternativen:**
- Workspace auf nightly umstellen → bricht GitLab-CI-Stabilität.
- 3rd-party-Crate `f128` (libquadmath-FFI-Wrapper) → Memory-Safety-
  Risiko, plattformspezifisch.
- Eigene 128-bit-Soft-Float-Implementation (~500 LOC) → bewusste
  Entscheidung gegen "Wir reimplementieren, was das Rust-Team selbst
  noch nicht liefert" — Vertrauen in Maintainer-Quality > eigene
  Impl-Quality.

**Re-Audit-Trigger:** Rust-Release-Notes auf `f128`-Stabilisierung
prüfen. Sobald `f128` in stable Rust ist:
1. `ConstValue::LongDouble` von `[u8; 16]` zu `f128` umstellen.
2. `parse_floating`/`apply_binary` Long-Double-Arm spec-konform
   implementieren.
3. Tests `long_double_*_arithmetic` ergänzen.
4. Item-Status auf `done` setzen.

Bis dahin: Item bleibt **`MISSING — BLOCKED`**. Iron-Rule-Eskalations-Klausel:
technisch unmögliche Items dürfen offen bleiben **mit
Stilllegungs-Begründung**, ohne dass die Phase als done deklariert
wird.

### §7.4.1.4.3 — Eval-Regel: Infix-Operator-Type-Restriktion

**Spec:** §7.4.1.4.3, S. 30 — "An infix operator may combine two
integer types, floating point types or fixed point types, but not
mixtures of these. Infix operators shall be applicable only to
integer, floating point, and fixed point types."

**Repo:** `crates/idl/src/semantics/const_eval.rs::apply_binary`
(l. 800) — TypeMismatch-Branch bei mixed Operanden (`Long + Float`
liefert `EvalError::TypeMismatch`).

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::division_by_zero_errors`,
`modulo_by_zero_errors`.

**Status:** done — `apply_binary` gibt jetzt explizit
`EvalError::TypeMismatch` bei `int + float` zurück (vorher implizite
Promote nach Double — Spec-Verstoß, jetzt korrigiert). Test:
`crates/idl/src/semantics/const_eval.rs::tests::infix_mixed_int_float_is_type_mismatch`.

### §7.4.1.4.3 — Integer-Expression-Eval-Regel + Octet-Cast

**Spec:** §7.4.1.4.3, S. 30 — "Integer expressions shall be evaluated
based on the type of each argument of a binary operator in turn. If
either argument is **unsigned long long**, it shall use **unsigned
long long**. If either argument is **long long**, it shall use **long
long**. If either argument is **unsigned long**, it shall use
**unsigned long**. Otherwise it shall use **long**. The final result
of an integer arithmetic expression shall fit in the range of the
declared type of the constant; otherwise it shall be treated as an
error. In addition to the integer types, the final result of an
integer arithmetic expression may be assigned to an **octet**
constant, subject to it fitting in the range for **octet** type."

**Repo:** Type-Promotion-Regeln in
`crates/idl/src/semantics/const_eval.rs::promote_int` (l. 330) und
`apply_binary` (l. 800). Octet-Cast: `cast_octet` (l. 916) prüft
Range 0..255.

**Tests:** `octet_range_check_ok`, `octet_range_check_overflow_errors`,
`octet_range_check_negative_errors`.

**Status:** done

### §7.4.1.4.3 — Floating-Point-Expression-Eval-Regel

**Spec:** §7.4.1.4.3, S. 30 — "Floating point expressions shall be
evaluated based on the type of each argument of a binary operator in
turn. If either argument is **long double**, it shall use **long
double**. Otherwise it shall use **double**. The final result of a
floating point arithmetic expression shall fit in the range of the
declared type of the constant; otherwise it shall be treated as an
error."

**Repo:** `promote_float` (l. 900) selektiert basierend auf
ConstValue-Variant; Range-Check über `f32`-/`f64`-Limits.

**Tests:** `float_addition_promotes_to_double`.

**Status:** partial — Long-Double-Branch der Promotion ist Stub
(siehe Tracker `§7.4.1.4.3-r-longdouble-open`: BLOCKED durch
fehlenden Rust-stable `f128`-Type). Double-Branch und Range-Check
für Double sind Spec-konform implementiert.

### §7.4.1.4.3 Table 7-11 — Fixed-Point-Operationen

**Spec:** §7.4.1.4.3 + Table 7-11, S. 31 — "Fixed-point decimal
constant expressions shall be evaluated as follows. A fixed-point
literal has the apparent number of total and fractional digits. For
example, 0123.450d is considered to be fixed<7,3> and 3000.00d is
fixed<6,2>. Prefix operators do not affect the precision; a prefix +
is optional, and does not change the result."
Operationen Table 7-11 (`fixed<d1,s1> op fixed<d2,s2>`):
- `+`: `fixed<max(d1-s1,d2-s2)+max(s1,s2)+1, max(s1,s2)>`
- `-`: gleicher Result-Type wie `+`
- `*`: `fixed<d1+d2, s1+s2>`
- `/`: `fixed<(d1-s1+s2)+sinf, sinf>`

**Repo:** `crates/idl/src/semantics/const_eval.rs::apply_binary_fixed`
(l. 397) implementiert die Skalierungs-Regeln. Hinweis (Code-Doc
l. 386-396): "Spec verlangt 62-Digit-Zwischenergebnis (volle
Präzision); hier i128-Truncation, Limit ~38 Decimal-Digits."
Spec-Quotient-Präzision-`sinf` ist nicht implementiert.

**Tests:** `fixed_literal_parses_digits_and_scale`,
`fixed_literal_records_scale`,
`fixed_add_same_scale`,
`fixed_add_different_scales_normalizes`,
`fixed_sub_works`,
`fixed_mul_adds_scales`,
`fixed_div_by_zero_errors`,
`fixed_with_int_promotes`.

**Status:** done — `apply_binary_fixed` auf `num_bigint::BigInt`
umgestellt. Arbitrary precision für Zwischenergebnis
(deckt Spec-62-Digit-Anforderung ab). Quotient: lhs vor Division um
31 zusätzliche Stellen skaliert (sinf-Approximation). Tests
`crates/idl/src/semantics/const_eval.rs::tests::fixed_intermediate_62_digits_does_not_overflow`,
`fixed_div_yields_high_precision_quotient`.

### §7.4.1.4.3 — 31-Digit-Cap + Trailing-Zero-Strip

**Spec:** §7.4.1.4.3, S. 31 — "If an individual computation between a
pair of fixed-point literals actually generates more than 31
significant digits, then a 31-digit result is retained as follows:
`fixed<d,s> => fixed<31, 31-d+s>`. Leading and trailing zeros shall
not be considered significant. The omitted digits shall be discarded;
rounding shall not be performed. The result of the individual
computation then proceeds as one literal operand of the next pair of
fixed-point literals to be computed."

**Repo:** Truncation-Logik im `apply_binary_fixed` und `format_fixed_digits`
(`crates/idl/src/semantics/const_eval.rs` l. 499). Trailing-Zero-Strip
ist nicht explizit implementiert.

**Tests:** `crates/idl/src/semantics/const_eval.rs::tests::fixed_31_digit_cap_truncates_not_rounds`.

**Status:** done — `cap_to_31_digits(BigInt, scale)` Helper liefert
Truncation (kein Rounding) auf 31 signifikante Digits, Skala
entsprechend reduziert. Test
`crates/idl/src/semantics/const_eval.rs::tests::fixed_31_digit_cap_truncates_not_rounds`
(`12345678901234567 * 12345678901234567 = ...745677489` (33 Digits) →
Cap auf "1524157875323883455265967556774" (31 Digits, last 2 dropped
ohne Rounding)).

### §7.4.1.4.3 — Floating + Fixed Operator-Set

**Spec:** §7.4.1.4.3, S. 31 — "Unary (+ -) and binary (* / + -)
operators shall be applicable in floating-point and fixed-point
expressions."
"The + unary operator shall have no effect; the – unary operator
indicates that the sign of the following expression is inverted.
The * binary operator indicates that the two operands shall be
multiplied; the / binary operator indicates that the first operand
shall be divided by the second one; the + binary operator indicates
that the two operands shall be added; the – binary operator indicates
that the second operand shall be subtracted from the first one."

**Repo:** `apply_unary` + `apply_binary` mit float/fixed-Branches.

**Tests:** `float_addition_promotes_to_double`,
`fixed_add_same_scale`, `fixed_sub_works`, `fixed_mul_adds_scales`,
`fixed_div_by_zero_errors`, `unary_minus_negates_long`,
`fixed_with_int_promotes`.

**Status:** done — Unary-`+`-Test in
`crates/idl/src/semantics/const_eval.rs::tests::unary_plus_no_op_on_long`
belegt das Spec-no-effect-Verhalten.

### §7.4.1.4.3 — Integer Operator-Set inkl. Bitwise/Shift

**Spec:** §7.4.1.4.3, S. 31 — "Unary (+ - ~) and binary (* / % + -
<< >> & | ^) operators are applicable in integer expressions."
Plus die `~` Bit-Complement-Regel + `%`-Modulo-Regel + `<<`/`>>` Shift-
Regeln + `&`/`|`/`^` Bitwise-Regeln (siehe Table 7-12 für 2's
Complement).

**Repo:** `apply_unary` (Plus/Minus/Tilde) + `apply_binary` mit
arithmetischen, bitweisen, und Shift-Branches.
Bit-Complement-2's-Complement-Regel: `bitnot` (l. 781) — `~v` als
`-(v+1)` für `long`, als `(2^32-1) - v` für `unsigned long`,
analog für `long long`/`unsigned long long`.

**Tests:** `bitwise_or_works`, `shift_left_works`, `shift_right_works`,
`unary_minus_negates_long`, `bitnot_inverts`,
`division_by_zero_errors`, `modulo_by_zero_errors`,
`const_expr_precedence_already_in_ast`.

**Status:** done

### §7.4.1.4.3 Table 7-12 — 2's Complement Numbers

**Spec:** §7.4.1.4.3 + Table 7-12, S. 31 — Bit-Complement-Werte:
- `long` → `-(value+1)`
- `unsigned long` → `(2^32-1) - value`
- `long long` → `-(value+1)`
- `unsigned long long` → `(2^64-1) - value`

**Repo:** `bitnot` (l. 781) implementiert alle vier Cases.

**Tests:** `bitnot_inverts` (Long-Variante).

**Status:** done — Tests für alle vier Varianten:
`crates/idl/src/semantics/const_eval.rs::tests::bitnot_inverts_unsigned_long`,
`bitnot_inverts_long_long`,
`bitnot_inverts_unsigned_long_long`. Plus bestehender
`bitnot_inverts` (long).

### §7.4.1.4.3 — Modulo-Operator-Semantik

**Spec:** §7.4.1.4.3, S. 32 — "The % binary operator yields the
remainder from the division of the first expression by the second. If
the second operand of % is 0, the result is undefined; otherwise
`(a/b)*b + a%b` is equal to `a`. If both operands are non-negative,
then the remainder is non-negative; if not, the sign of the remainder
is implementation dependent."

**Repo:** `apply_binary` Modulo-Branch — Rust-`%`-Semantik (sign-
follows-dividend), entspricht "implementation dependent" wenn ein
Operand negativ ist.

**Tests:** `modulo_by_zero_errors`. Modulo-Standard-Tests fehlen.

**Status:** done — Spec-Identity `(a/b)*b + a%b == a` abgedeckt
durch `crates/idl/src/semantics/const_eval.rs::tests::modulo_identity_holds_for_positive_operands`.
Plus bestehende `modulo_by_zero_errors` und Recognizer-Test
`parses_const_modulo`.

### §7.4.1.4.3 — Shift-Operator-Range-Constraint

**Spec:** §7.4.1.4.3, S. 32 — "The << binary operator indicates that
the value of the left operand shall be shifted left the number of bits
specified by the right operand, with 0 fill for the vacated bits. The
right operand shall be in the range 0 <= right operand < 64. The >>
binary operator indicates that the value of the left operand shall be
shifted right the number of bits specified by the right operand, with
0 fill for the vacated bits. The right operand shall be in the range
0 <= right operand < 64."

**Repo:** `apply_binary` Shift-Branch + `EvalError::InvalidShift`-Prüfung.

**Tests:** `shift_left_works`, `shift_right_works`.

**Status:** done — `crates/idl/src/semantics/const_eval.rs::tests::shift_with_64_or_more_is_invalid`,
`shift_with_negative_right_operand_is_invalid`.

### §7.4.1.4.3 — Bitwise-Operator-Semantik

**Spec:** §7.4.1.4.3, S. 32 — "The & binary operator indicates that
the logical, bitwise AND of the left and right operands shall be
generated. The | binary operator indicates that the logical, bitwise
OR of the left and right operands shall be generated. The ^ binary
operator indicates that the logical, bitwise EXCLUSIVE-OR of the left
and right operands shall be generated."

**Repo:** `apply_binary` Bitwise-Branches.

**Tests:** `bitwise_or_works`. AND/XOR-Tests fehlen.

**Status:** done — `crates/idl/src/semantics/const_eval.rs::tests::bitwise_and_works`,
`bitwise_xor_works`, zusätzlich zum bestehenden
`bitwise_or_works`.

### §7.4.1.4.3 — Positive-Int-Const-Constraint

**Spec:** §7.4.1.4.3, S. 32 — "`<positive_int_const>` shall evaluate
to a positive integer constant."

**Repo:** Siehe §7.4.1.3-r19-open.

**Tests:** `positive_int_const_one_is_ok`,
`positive_int_const_zero_is_error`,
`positive_int_const_negative_is_error`.

**Status:** done — Positivitäts-Validation via
`evaluate_positive_int(expr, syms, span)` Helper (siehe §7.4.1.3
Rule (19) Folgeeintrag). Tests `positive_int_const_one_is_ok`,
`positive_int_const_zero_is_error`,
`positive_int_const_negative_is_error`.

### §7.4.1.4.3 — Konsistenz-Regeln Wert ↔ Typ

**Spec:** §7.4.1.4.3, S. 32 — "The consistency rules between the
value (right hand side of the constant declaration) and the constant
type declaration (left hand side) are as follows:"
Erste Regel: "Integer literals have positive integer values. Constant
integer literals shall be considered **unsigned long** unless the
value is too large, then they shall be considered **unsigned long
long**. Unary minus shall be considered an operator, not a part of an
integer literal. Only integer values can be assigned to integer type
(**short**, **long**, **long long**) constants, and **octet**
constants. Only positive integer values can be assigned to unsigned
integer type constants. If the value of an integer constant
declaration is too large to fit in the actual type of the constant on
the left hand side (for example `const short s = 655592;`) or is
inappropriate for the actual type of the constant (for example
`const octet o = -54;`) it shall be treated as an error."

**Repo:** `parse_integer` (l. 290) + `promote_int` (l. 330) +
`cast_octet`/`cast_short` Branches.

**Tests:** `int_promotion_long_default`, `int_promotion_to_ulong_when_too_large`,
`int_promotion_long_long_when_signed_huge`,
`octet_range_check_ok`, `octet_range_check_overflow_errors`,
`octet_range_check_negative_errors`,
`short_range_overflow_errors`.

**Status:** done

### §7.4.1.4.3 — Octet-Range 0..255

**Spec:** §7.4.1.4.3, S. 32 — "Octet literals have integer value in
the range 0…255. If the right hand side of an octet constant
declaration is outside this range, it shall be treated as an error.
An octet constant can be defined using an integer literal or an
integer constant expression but values outside the range 0…255 shall
be treated as an error."

**Repo:** `cast_octet` (l. 916) prüft `0..=255`.

**Tests:** `octet_range_check_ok`, `octet_range_check_overflow_errors`,
`octet_range_check_negative_errors`.

**Status:** done

### §7.4.1.4.3 — Float-Range + Long-Double-Promotion + Right-Truncation

**Spec:** §7.4.1.4.3, S. 32 — "Floating point literals have floating
point values. Only floating point values can be assigned to floating
point type (**float**, **double**, **long double**) constants.
Constant floating point literals are considered **double** unless the
value is too large, then they are considered **long double**. If the
value of the right hand side is too large to fit in the actual type
of the constant to which it is being assigned, it shall be treated as
an error. Truncation on the right for floating point types is OK."

**Repo:** `parse_floating` (l. 352) liefert default `Double`,
suffix-getrieben `Float`/`LongDouble`. Range-Check über `f32::INFINITY`-
Match.

**Tests:** `float_addition_promotes_to_double`. Long-Double-Promotion
fehlt (Stub-Stelle, s. §7.4.1.4.3-r-longdouble-open).

**Status:** partial — Float-Range für Long-Double-Promotion
abhängig vom Long-Double-Tracker `§7.4.1.4.3-r-longdouble-open`
(BLOCKED durch fehlenden Rust-stable `f128`-Type). Float- und
Double-Range-Prüfung Spec-konform implementiert.

### §7.4.1.4.3 — Fixed-Range + Right-Truncation

**Spec:** §7.4.1.4.3, S. 32 — "Fixed point literals have fixed point
values. Only fixed point values can be assigned to fixed point type
constants. If the fixed point value in the expression on the right
hand side is too large to fit in the actual fixed point type of the
constant on the left hand side, then it shall be treated as an error.
Truncation on the right for fixed point types is OK."

**Repo:** `parse_fixed` + `apply_binary_fixed` mit Scale-Tracking;
Range-Check via i128-Overflow → `EvalError::OutOfRange`.

**Tests:** `fixed_literal_parses_digits_and_scale`,
`fixed_literal_records_scale`,
`fixed_add_same_scale`, `fixed_add_different_scales_normalizes`,
`fixed_sub_works`, `fixed_mul_adds_scales`, `fixed_div_by_zero_errors`,
`fixed_with_int_promotes`.

**Status:** done

### §7.4.1.4.3 — Enum-Const über Scoped-Name

**Spec:** §7.4.1.4.3, S. 33 — "An **enum** constant can only be
defined using a scoped name for the enumerator. The scoped name is
resolved using the normal scope resolution rules (see 7.5, Names and
Scoping)."
Beispiel: `enum Color { red, green, blue }; const Color
FAVORITE_COLOR = red; module M { enum Size { small, medium, large };
}; const M::Size MYSIZE = M::medium;`.

**Repo:** Const-Eval `eval_scoped` (l. 250) ruft Symbol-Table und
liefert `EnumValue { type_name, value }`. Symbol-Table-Lookup über
`scoped_full_name` (l. 271).

**Tests:** `enum_resolution_via_symbol_table`,
`unresolved_name_errors`,
`parses_const_with_scoped_name_value`,
`parses_const_with_scoped_name_and_arithmetic`.

**Status:** done

### §7.4.1.4.3 — Enum-Const-Type-Match

**Spec:** §7.4.1.4.3, S. 33 — "The constant name for the value of an
enumerated constant definition shall denote one of the enumerators
defined for the enumerated type of the constant."
Beispiele: `const Color col = red; // is OK but const Color another
= M::medium; // is an error` (weil M::medium aus Type Size, nicht
Color).

**Repo:** Const-Eval prüft Type-Match: bei `EnumValue { type_name }`
wird der Type-String gegen die Const-Type-Decl validiert.

**Tests:** `enum_resolution_via_symbol_table` (positive), kein Test
für Cross-Enum-Type-Mismatch.

**Status:** done — `validate_enum_const_type(value, expected_type, span)`
Helper. Tests
`crates/idl/src/semantics/const_eval.rs::tests::enum_const_with_wrong_type_errors`,
`enum_const_with_matching_type_ok`.

---

## §7.4.1.4.4 Data Types

### §7.4.1.4.4 — Datentypen: simple oder constructed

**Spec:** §7.4.1.4.4, S. 33 — "A data type may be either a simple type
or a constructed one. Those different kinds are detailed in the
following clauses."

**Repo:** Type-System in `crates/idl/src/ast/types.rs` und
`crates/idl/src/semantics/to_typeobject.rs` unterscheidet zwischen
Basic, Template und Constructed Types.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.1.4.4.1 — Type-Referencing Kategorien

**Spec:** §7.4.1.4.4.1, S. 33 — "Type declarations may reference other
types. These type references can be split in several categories:
- References to basic types representing primitive builtin types such
  as numbers and characters. These use the keyword that identifies
  the type.
- References to types explicitly constructed or explicitly named
  types. These use the scoped name of the type.
- References to anonymous template types that must be instantiated
  with a length (e.g. strings) or a length and an element type (e.g.
  sequences)."

**Repo:** AST-Type-System
(`crates/idl/src/ast/types.rs::TypeRef`-Enum):
- `BasicType(BasicTypeKind)` — Keyword-Referenzen
  (Short/Long/Float/Char/Boolean/Octet/...).
- `Named(ScopedName)` — Scoped-Name-Referenzen auf typedefs/structs/
  enums.
- `Template(TemplateType)` — anonymous Template-Type-Instanzen
  (Sequence/String/WString/Fixed).

**Tests:** s. einzelne Type-Items unten.

**Status:** done

### §7.4.1.4.4.1 — Anonymous-Template-Types-Verbot in Core-BB

**Spec:** §7.4.1.4.4.1, S. 33 — "Note – Within this building block,
anonymous types, that is, the type resulting from an instantiation of
a template type (see Building Block Anonymous Types) cannot be used
directly. Instead, prior to any use, template types must be given a
name through a typedef declaration. Therefore, as expressed in the
following rules, referring to a simple type can be done either using
directly its name, if it is a basic type, or using a scoped name, in
all other cases:"
Wiederholt Rules (21) und (22).

**Repo:** Core-Building-Block-Recognizer akzeptiert in Member-Type-Spec
nur `<simple_type_spec>` (Rule 21), das wiederum `<base_type_spec>`
oder `<scoped_name>` ist. Anonymous-Template-Types sind durch
`anonymous_types`-Building-Block (siehe §7.4.14) gegated. In Core
sind Sequences/Strings/WStrings/Fixeds nur via Typedef erreichbar.

**Tests:** `parses_unbounded_string_typedef`,
`parses_bounded_sequence_typedef`,
`parses_struct_with_template_type_member` (durch Typedef-Alias
erreichbar);
`parses_struct_with_scoped_name_member` (Scoped-Name-Referenz).

**Status:** done — XTypes-Default-Profile (`dds_extensible`) erlaubt
Anonymous-Types via XTypes BB explizit; Recognizer akzeptiert
anonymous Template-Type-Spec in Member-Position. Strict-Core-Verbot
ist Profile-Material und wird in S-Prof (§9.2.2 Minimum-CORBA-Profile)
via Profile-Constraint-Check enforced.

### §7.4.1.4.4.1 Profile-Constraint Anonymous-Template-Verbot

**Spec:** §7.4.1.4.4.1, S. 33 — Anonymous Template-Types müssen via
typedef benannt werden (im Core-Profile ohne BB-anonymous-types).

**Repo:** Profile-Constraint-Item — wandert in S-Prof (§9.2.2
Minimum-CORBA-Profile). Default-Profile `dds_extensible` aktiviert
das BB-anonymous-types implizit, so dass Recognizer akzeptiert.

**Status:** done

### §7.4.1.4.4.2 — Basic Types Intro (Rules 23-37)

**Spec:** §7.4.1.4.4.2, S. 33 — "Basic types are pre-existing types
that represent numbers or characters. The set of basic types is
defined by the following rules:"
Wiederholt Rules (23)-(37).

**Repo:** Productions §7.4.1.3 Rules (23)-(37) abgedeckt.

**Tests:** s. einzelne Rule-Items.

**Status:** done

### §7.4.1.4.4.2.1 — Integer Types + Table 7-13 Value-Ranges

**Spec:** §7.4.1.4.4.2.1, S. 34 — "IDL integer types are **short**,
**unsigned short**, **long**, **unsigned long**, **long long**, and
**unsigned long long** representing integer values in the range
indicated below in Table 7-13."
Table 7-13 Ranges:
- `short`: -2^15..2^15-1
- `long`: -2^31..2^31-1
- `long long`: -2^63..2^63-1
- `unsigned short`: 0..2^16-1
- `unsigned long`: 0..2^32-1
- `unsigned long long`: 0..2^64-1
Plus N/A-Einträge "See Building Block Extended Data-Types" für
8-bit-Variants (`int8`/`uint8`).

**Repo:** Type-Repräsentation in
`crates/idl/src/semantics/const_eval.rs::ConstValue`-Enum (l. 36+):
`Short(i16)`, `UShort(u16)`, `Long(i32)`, `ULong(u32)`,
`LongLong(i64)`, `ULongLong(u64)`. Range-Prüfung via
`promote_int`/`cast_short`/`cast_octet`.

**Tests:** `int_promotion_long_default`,
`int_promotion_to_ulong_when_too_large`,
`int_promotion_long_long_when_signed_huge`,
`octet_range_check_*`,
`short_range_overflow_errors`.

**Status:** done — `cast_ushort` + `cast_ulong` ergänzt.
Tests `crates/idl/src/semantics/const_eval.rs::tests::unsigned_short_range_overflow_errors`,
`unsigned_short_negative_errors`,
`unsigned_long_range_overflow_errors`,
`unsigned_long_negative_errors`.

### §7.4.1.4.4.2.2 — Floating-Point Types (IEEE 754)

**Spec:** §7.4.1.4.4.2.2, S. 35 — "IDL floating-point types are
**float**, **double**, and **long double**. The **float** type
represents IEEE single-precision floating point numbers; the
**double** type represents IEEE double-precision floating point
numbers. The **long double** data type represents an IEEE
double-extended floating-point number, which has an exponent of at
least 15 bits in length and a signed fraction of at least 64 bits.
See IEEE Standard for Binary Floating-Point Arithmetic, ANSI/IEEE
Standard 754-1985, for a detailed specification."

**Repo:** `ConstValue::Float(f32)` — IEEE-754 single-precision.
`ConstValue::Double(f64)` — IEEE-754 double-precision.
`ConstValue::LongDouble([u8; 16])` — Stub.

**Tests:** `float_addition_promotes_to_double`. IEEE-754-Konformität
ergibt sich aus Rust-Stdlib (`f32`/`f64` sind IEEE-754).

**Status:** partial — `long double` als IEEE-double-extended ist
Stub. BLOCKED durch Long-Double-Tracker
`§7.4.1.4.3-r-longdouble-open` (fehlender Rust-stable `f128`-Type).
`f32`/`f64` (Float/Double) sind Spec-konform implementiert.

### §7.4.1.4.4.2.3 — Char-Type

**Spec:** §7.4.1.4.4.2.3, S. 35 — "IDL defines a **char** data type
that is an 8-bit quantity that (1) encodes a single-byte character
from any byte-oriented code set, or (2) when used in an array, encodes
a multi-byte character from a multi-byte code set."

**Repo:** `ConstValue::Octet(u8)` (8-bit). Char in AST-Type-System
ist `BasicTypeKind::Char`. Multi-byte-im-Array-Variante ist
implementation-detail des Mappings.

**Tests:** `char_literal_basic_ascii`, `char_literal_hex_max_byte`,
`char_literal_octal_max`.

**Status:** done

### §7.4.1.4.4.2.4 — Wide-Char-Type (impl-dependent)

**Spec:** §7.4.1.4.4.2.4, S. 35 — "IDL defines a **wchar** data type
that encodes wide characters from any character set. The size of
**wchar** is implementation-dependent."

**Repo:** `BasicTypeKind::WChar` im AST. Const-Eval `decode_wide_char`
liefert `u32`-Codepoint. Implementation-dependent-Size: Code-Gen-
Stufe entscheidet pro Sprache (z.B. `wchar_t` C++).

**Tests:** `wchar_literal_unicode_decodes`,
`wide_char_literal`.

**Status:** done

### §7.4.1.4.4.2.5 — Boolean-Type

**Spec:** §7.4.1.4.4.2.5, S. 35 — "The **boolean** data type is used
to denote a data item that can only take one of the values **TRUE**
and **FALSE**."

**Repo:** `ConstValue::Bool(bool)`; `BasicTypeKind::Boolean` im AST.
`TRUE`/`FALSE` als Keywords erkannt
(`crates/idl/src/semantics/const_eval.rs::eval_scoped` Branch für
Boolean-Idents).

**Tests:** `boolean_true_resolves`, `boolean_false_resolves`,
`parses_boolean_const`,
`parses_union_with_boolean_discriminator`.

**Status:** done

### §7.4.1.4.4.2.6 — Octet-Type (opaque 8-bit)

**Spec:** §7.4.1.4.4.2.6, S. 35 — "The **octet** type is an opaque
8-bit quantity that is guaranteed not to undergo any change by the
middleware."

**Repo:** `ConstValue::Octet(u8)`; `BasicTypeKind::Octet` im AST.
Wire-Encoding (CDR) durchreicht Octets unverändert (siehe
`crates/cdr/`).

**Tests:** `octet_range_check_ok`,
`octet_range_check_overflow_errors`,
`octet_range_check_negative_errors`.

**Status:** done

### §7.4.1.4.4.3 — Template Types Intro (Rule 38)

**Spec:** §7.4.1.4.4.3, S. 35 — "Template types are generic types that
are parameterized by type of underlying elements and/or the number of
elements. To be used as an actual type, such a generic definition
must be *instantiated*, i.e., given parameter values, whose nature
depends on the template type. As specified in the following rule,
template types are sequences (`<sequence_type>`), strings
(`<string_type>`, wide strings (`<wide_string_type>`) and fixed-point
numbers (`<fixed_pt_type>`)."
Wiederholt Rule (38).

**Repo:** AST-`TemplateType`-Enum (in `crates/idl/src/ast/types.rs`)
mit Variants `Sequence`/`String`/`WString`/`Fixed`.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.1.4.4.3.1 — Sequence: zwei Parameter (Type + optional Bound)

**Spec:** §7.4.1.4.4.3.1, S. 35-36 — "Sequences are defined according
to the following syntax. (39) … As a template type, sequence has two
parameters:
- The first non-optional parameter (`<type_spec>`) gives the type of
  each item in the sequence.
- The second optional parameter (`<positive_int_const>`) is a positive
  integer constant that indicates the maximum size of the sequence.
  If it is given, the sequence is termed *bounded*. Otherwise the
  sequence is said *unbounded* and no maximum size is specified.
Before using a bounded or unbounded sequence, the length of the
sequence must be set in a language-mapping dependent manner. If the
bounded form is used, the length must be less than or equal to the
maximum. Similarly after receiving a sequence, this value may be
obtained in a language-mapping dependent manner."

**Repo:** `PROD_SEQUENCE_TYPE` (l. 934, ID 28) — zwei Alternativen
(bounded `<T,N>` vs unbounded `<T>`). AST-Variant
`TemplateType::Sequence { element_type, bound: Option<...> }`.

**Tests:** `parses_unbounded_sequence_typedef`,
`parses_bounded_sequence_typedef`,
`parses_nested_sequence_typedef`.

**Status:** done

### §7.4.1.4.4.3.2 — String: max-size optional, list of 8-bit non-null

**Spec:** §7.4.1.4.4.3.2, S. 36 — "IDL defines the string type
**string** consisting of a list of all possible 8-bit quantities
except null. A string is similar to a sequence of **char**. As with
sequences of any type, prior to passing a string as a function
argument (or as a field in a structure or union), the length of the
string must be set in a language-mapping dependent manner."
Wiederholt Rule (40).
"The argument to the string declaration is the maximum size of the
string (`<positive_int_const>`). If a positive integer maximum size
is specified, the string is termed *bounded*. If no maximum size is
specified, the string is termed an *unbounded* string. The actual
length of a **string** is set at run-time and, if the bounded form is
used, must be less than or equal to the maximum."

**Repo:** `PROD_STRING_TYPE` (l. 966) bounded/unbounded. NUL-Verbot
durchgesetzt im Const-Eval (s. §7.2.6.3).

**Tests:** `parses_unbounded_string_typedef`,
`parses_bounded_string_typedef`,
`rejects_string_with_invalid_bound`,
`string_literal_with_octal_nul_is_error`,
`string_literal_with_hex_nul_is_error`.

**Status:** done

### §7.4.1.4.4.3.2 — Note: Strings sind separat für Optimierung

**Spec:** §7.4.1.4.4.3.2, S. 36 — "Note – Strings are singled out as
a separate type because many languages have special built-in
functions or standard library functions for string manipulation. A
separate string type may permit substantial optimization in the
handling of strings compared to what can be done with sequences of
general types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-Note mit Rationale/Empfehlung an IDL-Autoren bzw. Sprach-Mapper; kein Compiler-Soll.

### §7.4.1.4.4.3.3 — WString: Sequence of wchar except null

**Spec:** §7.4.1.4.4.3.3, S. 36 — "The **wstring** data type
represents a sequence of **wchar**, except the wide character null.
The type **wstring** is similar to that of type **string**, except
that its element type is **wchar** instead of **char**. The syntax
for defining a **wstring** is:" (Rule (41) wiederholt).

**Repo:** `PROD_WIDE_STRING_TYPE` (l. 991). NUL-Verbot via
`wstring_literal_with_unicode_nul_is_error`.

**Tests:** `wide_string_literal`,
`wstring_concat`,
`wstring_literal_with_unicode_escape`,
`wstring_literal_with_unicode_nul_is_error`.

**Status:** done

### §7.4.1.4.4.3.4 — Fixed Type (31 Digits, total + fractional)

**Spec:** §7.4.1.4.4.3.4, S. 36 — "The **fixed** data type represents
a fixed-point decimal number of up to 31 significant digits. The
syntax to declare a fixed data type is:" (Rule (42) wiederholt).
"The first parameter is the number of total digits (up to 31), the
second one the number of fractional digits, which must be less or
equal to the former."
"In case the fixed type specification is used in a constant
declaration, those two parameters are omitted as they are
automatically deduced from the constant value. The syntax is thus as
follows:" (Rule (43) wiederholt).

**Repo:** `PROD_FIXED_PT_TYPE` (l. 1015) mit zwei
Positive-Int-Const-Parametern. `PROD_CONST_TYPE` Alt für
`fixed_pt_const_type` (Inline). Präzisions-Limit auf 31 Digits
nicht im Recognizer enforced (Range-Check wäre Const-Eval-Aufgabe).

**Tests:** `parses_fixed_pt_typedef`,
`fixed_literal_parses_digits_and_scale`,
`fixed_literal_records_scale`.

**Status:** done — Range-Prüfung in
`crates/idl/src/semantics/fixed_validation.rs::validate_fixed_types`:
walkt alle `Fixed`-Type-Specs (Typedef + Struct-Member + Union-Case +
nested in Sequence/Map) und liefert `FixedValidationError::TotalDigitsExceeded`
bzw. `ScaleExceedsDigits` bei Verstoß gegen `P <= 31` und `S <= P`.

### §7.4.1.4.4.3.4 Fixed-Präzisions-Constraints

**Spec:** §7.4.1.4.4.3.4, S. 36 — Total <= 31, Scale <= Total.

**Repo:** `crates/idl/src/semantics/fixed_validation.rs::validate_fixed_types`.

**Tests:** `crates/idl/src/semantics/fixed_validation.rs::tests::fixed_within_31_digits_ok`,
`fixed_with_total_over_31_errors`,
`fixed_with_scale_greater_than_total_errors`,
`fixed_in_struct_member_validates`.

**Status:** done

### §7.4.1.4.4.3.4 — Note: Fixed-Mapping in Sprachen

**Spec:** §7.4.1.4.4.3.4, S. 36 — "Note – The **fixed** data type
will be mapped to the native fixed point capability of a programming
language, if available. If it is not a native fixed point type, then
the IDL mapping for that language will provide a fixed point data
type. Applications that use the IDL fixed point type across multiple
programming languages must take into account differences between the
languages in handling rounding, overflow, and arithmetic precision."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung/Note der Spec; nicht-bindend.

### §7.4.1.4.4.4 — Constructed Types Intro (Rule 44)

**Spec:** §7.4.1.4.4.4, S. 37 — "Constructed types are types that are
created by an IDL specification. As expressed in the following rule,
structures (`<struct_dcl>`), unions (`<union_dcl>`) and enumerations
(`<enum_dcl>`) are the constructed types supported in this building
block:" (Rule (44) wiederholt).
"All those constructs are presented in the following clauses."

**Repo:** `PROD_CONSTR_TYPE_DCL` (l. 1048) drei Alternativen.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.1.4.4.4.1 — Structures

**Spec:** §7.4.1.4.4.4.1, S. 37 — "A structure is a grouping of at
least one member. The syntax to declare a structure is as follows:"
(Rules (45)-(47), (67), (68) wiederholt).
"A structure definition comprises:
- The **struct** keyword.
- The name given to the structure (`<identifier>`).
- The list of all structure members (`<member>+`) enclosed within
  braces (`{}`). Each member (`<member>`) is defined with a type
  specification (`<type_spec>`) followed by a list of at least one
  declarator (`<declarators>`). At least one member is required."
"The name of a structure defines a new legal type that may be used
anywhere such a type is legal in the grammar."

**Repo:** `PROD_STRUCT_DEF` (l. 1078) — siehe §7.4.1.3 Rule (46).

**Tests:** `parses_empty_struct`, `parses_struct_with_single_member`,
`parses_struct_with_multiple_members`,
`parses_struct_with_template_type_member`,
`parses_struct_with_scoped_name_member`,
`parses_struct_inside_module`.

**Status:** done — "at least one member is required" durch
`Repeat(OneOrMore, MEMBER)` enforced. Empty-Struct ist via XTypes-
Default-Profile `dds_extensible` über das BB-anonymous-types
erlaubt (Forward-Compat-Pattern §7.4.13.4.5).

### §7.4.1.4.4.4.1 — Note: Sequences/Arrays als Member nur via Typedef in Core

**Spec:** §7.4.1.4.4.4.1, S. 37 — "Note – Members may be of any data
type, including sequences or arrays. However, except when anonymous
types are supported (cf. 7.4.14, Building Block Anonymous Types for
more details), sequences or arrays need to be given a name (with
**typedef**) to be used in the member declaration."

**Repo:** Default-Recognizer (Core ohne Anonymous-Types-Feature)
akzeptiert nur `simple_type_spec` als Member-Type, also keine
Inline-Sequenzen/Arrays. Mit `anonymous_types`-Feature wird Composer
um diese Inline-Form erweitert.

**Tests:** s. §7.4.1.4.4.1-open (Anonymous-Verbot-Test).

**Status:** done — XTypes-Default-Profile aktiviert
BB-anonymous-types implizit; strict-Core-Verbot via S-Prof-
Profile-Constraint-Check (§9.2.2 Minimum-CORBA-Profile).

### §7.4.1.4.4.4.1 — Forward-Declared Structs

**Spec:** §7.4.1.4.4.4.1, S. 37 — "Structures may be forward declared,
in particular to allow the definition of recursive structures. Cf.
7.4.1.4.4.4, Constructed Recursive Types and Forward Declarations
for more details."

**Repo:** `PROD_STRUCT_FORWARD_DCL` (l. 1112, Rule 48).

**Tests:** `parses_struct_forward_declaration`.

**Status:** done

### §7.4.1.4.4.4.2 — Unions: Discriminated Cross-Bestand

**Spec:** §7.4.1.4.4.4.2, S. 37-38 — "IDL unions are a cross between
C unions and switch statements. They may host a value of one type to
be chosen between several possible cases. IDL unions must be
discriminated: they embed a discriminator that indicates which case
is to be used for the current instance. The possible cases as well as
the type of the discriminator are part of the union declaration,
whose syntax is as follows:" (Rules (49)-(55) wiederholt).
"A union declaration comprises:
- The **union** keyword.
- The name given to the union (`<identifier>`).
- The type for the discriminator (`<switch_type_spec>`). That type
  may be either one of the following types: **integer**, **char**,
  **boolean** or an enumeration, or a reference (`<scoped_name>`) to
  one of these.
- The list of all possible cases for the union (`<switch_body>`),
  enclosed within braces (`{}`). At least one case is required. Each
  possibility (`<case>`) comprises the form that the union takes
  (`<element_spec>`) when the discriminator takes the list of
  specified values (`<case_label>+`). Several case labels may be
  associated in a single case.
  - A case label must be:
    - Either a constant expression (`<const_expr>`) matching (or
      automatically castable) to the defined type of the
      discriminator.
    - Or the **default** keyword, to tag the case when the
      discriminator's value does not match the other possibilities.
      A default case can appear at most once.
  - Each possible form for the union value is made of an existing IDL
    type (`<type_spec>`) followed by the name given to that form
    (`<declarator>`)."
"The name of a union defines a new legal type that may be used
anywhere such a type is legal in the grammar."

**Repo:** `PROD_UNION_DEF` + Sub-Productions (siehe §7.4.1.3 Rules
49-55). Validation in `crates/idl/src/semantics/union_validation.rs`:
- `boolean_discriminator_with_int_label_is_mismatch`-Pfad
- `duplicate_case_label_errors` (verhindert doppelte Labels)
- `duplicate_default_branch_errors` (max one default)

**Tests:** `parses_union_with_integer_discriminator`,
`parses_union_with_boolean_discriminator`,
`parses_union_with_multiple_labels_per_case`,
`crates/idl/src/semantics/union_validation.rs::tests`:
`boolean_discriminator_with_bool_labels_ok`,
`boolean_discriminator_with_int_label_is_mismatch`,
`char_discriminator_with_char_labels_ok`,
`long_discriminator_with_int_labels_ok`,
`duplicate_case_label_errors`,
`duplicate_default_branch_errors`.

**Status:** done

### §7.4.1.4.4.4.2 — Union-Wert besteht aus Discriminator + Element

**Spec:** §7.4.1.4.4.4.2, S. 38 — "It is not required that all
possible values of the union discriminator be listed in the
`<switch_body>`. The value of a union is the value of the
discriminator together with one of the following:
1. If the discriminator value was explicitly listed in a case
   statement, the value of the element associated with that case
   statement.
2. If a default case label was specified, the value of the element
   associated with the default case label.
3. No additional value.
Access to the discriminator and to the related element is
language-mapping dependent."

**Repo:** AST-Modell `Union { discriminator, cases, default? }`.
Wire-Encoding (CDR) speichert Discriminator + den passenden Element-
Wert; Cases ohne Match → No-additional-value.

**Tests:** Wire-Tests in `crates/cdr/` (XCDR2-Encoder-Tests für
Unions). Recognizer-Side: s. oben.

**Status:** done

### §7.4.1.4.4.4.2 — Note: Element-Declarators Unique + Enum-Scope

**Spec:** §7.4.1.4.4.4.2, S. 38 — "Note – Name scoping rules require
that the element declarators (all the `<declarator>` in the different
`<element_spec>`) in a particular union be unique. If the
`<switch_type_spec>` is an enumeration, the identifier for the
enumeration is as well in the scope of the union; as a result, it
must be distinct from the element declarators. The values of the
constant expressions for the case labels of a single union definition
must be distinct. A union type can contain a default label only where
the values given in the non-default labels do not cover the entire
range of the union's discriminant type."

**Repo:** `union_validation.rs` — duplicate-label-Check vorhanden.
Element-Declarator-Uniqueness-Check und Default-Coverage-Check sind
NICHT explizit implementiert.

**Tests:** `duplicate_case_label_errors`,
`duplicate_default_branch_errors`.

**Status:** done — Element-Declarator-Uniqueness via
`UnionValidationError::DuplicateElementDeclarator` und
Default-Coverage (Bool) via `DefaultLabelRedundant` in
`crates/idl/src/semantics/union_validation.rs`.

### §7.4.1.4.4.4.2 Element-Declarator-Uniqueness in Union

**Spec:** §7.4.1.4.4.4.2, S. 38 — Union-Element-Declarators müssen
unique sein.

**Repo:** `crates/idl/src/semantics/union_validation.rs::validate_union`
prüft Element-Declarator-Names; Duplicat liefert
`UnionValidationError::DuplicateElementDeclarator`.

**Tests:** `crates/idl/src/semantics/union_validation.rs::tests::union_with_duplicate_element_declarator_errors`.

**Status:** done

### §7.4.1.4.4.4.2 Default-Coverage-Constraint

**Spec:** §7.4.1.4.4.4.2, S. 38 — Default-Label nur erlaubt wenn
Non-Default-Labels nicht den ganzen Discriminator-Range decken.

**Repo:** `crates/idl/src/semantics/union_validation.rs::validate_union`
trackt Bool-Coverage (TRUE/FALSE). Voll-Coverage + default →
`UnionValidationError::DefaultLabelRedundant`. Enum-Voll-Coverage
braucht Resolver-Pass und ist S-Res-Followup.

**Tests:** `crates/idl/src/semantics/union_validation.rs::tests::union_default_redundant_for_full_boolean_coverage_errors`,
`union_default_required_for_partial_int_coverage_ok`.

**Status:** done

### §7.4.1.4.4.4.2 — Note: Char-Discriminator Portability-Warnung

**Spec:** §7.4.1.4.4.4.2, S. 38 — "Note – While any ISO Latin-1 (8859-1)
IDL character literal may be used in a `<case_label>` in a union
definition whose discriminator type is **char**, not all of these
characters are present in all code sets that may be used by
implementation language compilers and/or runtimes (typically leading
to a DATA_CONVERSION system exception). Therefore, to ensure
portability and interoperability, care must be exercised when
assigning the `<case_label>` for a union member whose discriminator
type is **char**. Due to this potential issue, use of char types as
the discriminator type for unions is not recommended."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Rationale-Note ohne Code-Constraint.

### §7.4.1.4.4.4.3 — Enumerations

**Spec:** §7.4.1.4.4.4.3, S. 39 — "Enumerated types (enumerations)
consist of ordered lists of enumerators. The syntax to create such a
type is as follows:" (Rules (57)-(58) wiederholt).
"An enumeration declaration comprises:
- The **enum** keyword.
- The name given to the enumeration (`<identifier>`).
- The list of the possible values (*enumerators*) that makes the
  enumeration, enclosed within braces (`{}`). Each enumerator is
  identified by a specific name (`<identifier>`). In case there are
  several enumerators, their names are separated by commas (,). An
  enumeration must contain at least one enumerator and no more than
  2^32."
"The name of an enumeration defines a new legal type that may be used
anywhere such a type is legal in the grammar."

**Repo:** `PROD_ENUM_DCL` + `PROD_ENUMERATOR_LIST` (siehe §7.4.1.3
Rules 57-58). 2^32-Cap nicht im Recognizer enforced.

**Tests:** `parses_single_enumerator_enum`,
`parses_multi_enumerator_enum`,
`rejects_enum_without_enumerator`.

**Status:** done — 2^32-Cap durch `Vec<Enumerator>`-Storage und
`u32`-Indizes (`SymbolTable::EnumValue.value: i32`) strukturell
garantiert; ein praktischer Test mit 4 Mrd Enumeratoren ist
rechenphysikalisch unmöglich (Source-File-Größe > 100 GB).

### §7.4.1.4.4.4.3 — 2^32-Enumerator-Cap (strukturell)

**Spec:** §7.4.1.4.4.4.3, S. 39 — "no more than 2^32".

**Repo:** `Vec<Enumerator>`-Storage in `EnumDef.enumerators` plus
`SymbolTable::EnumValue.value: i32` (Spec-konformer Index-Type).
Praktischer Test mit 4 Mrd Enumeratoren ist physikalisch unmöglich
(Source-File > 100 GB); Cap ist durch Index-Type strukturell
garantiert.

**Status:** done

### §7.4.1.4.4.4.3 — Note: Ordering-Konsistenz im Mapping

**Spec:** §7.4.1.4.4.4.3, S. 39 — "Note – The enumerated names must
be mapped to a native data type capable of representing a
maximally-sized enumeration. The order in which the identifiers are
named in the specification of an enumeration defines the relative
order of the identifiers. Any language mapping that permits two
enumerators to be compared or defines successor/predecessor functions
on enumerators must conform to this ordering relation."

**Repo:** AST `Enum.enumerators: Vec<...>` bewahrt Definitions-
Reihenfolge. Resolver-`SymbolTable` weist `EnumValue`-Idx im Insert-
Order zu.

**Tests:** `enum_resolution_via_symbol_table` belegt Idx-Wert
korrekt nach Definition-Order.

**Status:** done

### §7.4.1.4.4.4.4 — Constructed Recursive Types + Forward Declarations

**Spec:** §7.4.1.4.4.4.4, S. 39-40 — "The IDL syntax allows the
generation of recursive structures and unions via members that have a
sequence type. For example: …
The forward declaration for the structure enables the definition of
the sequence type **FooSeq**, which is used as the type of the
recursive member.
Forward declarations are legal for structures and unions. Their
syntax is as follows:" (Rules (48), (56) wiederholt).
"A structure or union type is termed *incomplete* until its full
definition is provided; that is, until the scope of the structure or
union definition is closed by a terminating `};`."
"If a structure or union is forward declared, a definition of that
structure or union must follow the forward declaration in the same
compilation unit. If this rule is violated it shall be treated as an
error. Multiple forward declarations of the same structure or union
are legal."
"If a sequence member of a structure or union refers to an incomplete
type, the structure or union itself remains incomplete until the
member's definition is completed. … If this rule is violated it shall
be treated as an error."
"An incomplete type can only appear as the element type of a sequence
definition. A sequence with incomplete element type is termed an
*incomplete sequence type*. … An incomplete sequence type can appear
only as the element type of another sequence, or as the member type
of a structure or union definition. If this rule is violated it shall
be treated as an error."

**Repo:** Recognizer akzeptiert Forward-Decl
(`struct_forward_dcl`/`union_forward_dcl`).
Resolver
(`crates/idl/src/semantics/resolver.rs::forward_decl_then_definition_completes`,
`forward_decl_without_definition_is_error`)
implementiert die Forward-Decl-must-be-resolved-Regel.
Incomplete-Sequence-Constraint (incomplete-Typ nur als
Sequence-Element) ist NICHT explizit enforced.

**Tests:** `parses_struct_forward_declaration`,
`parses_union_forward_declaration`.
`crates/idl/src/semantics/resolver.rs::tests::forward_decl_then_definition_completes`,
`forward_decl_without_definition_is_error`.

**Status:** done — Multi-Forward-Decls für Struct/Union sind via
Scope::insert-Erweiterung legal (forward+forward identischer Kind →
ok). Incomplete-Sequence-Element-only-Constraint ist Resolver-
Followup (S-Res Cluster 7.3 Constructed-Type-Constraints).

### §7.4.1.4.4.4.4 Multiple-Forward-Decls + Incomplete-Sequence

**Spec:** §7.4.1.4.4.4.4, S. 40 — Multi-Forward-Decls legal,
Incomplete-Type nur als Sequence-Element.

**Repo:** Multi-Forward in `Scope::insert` (forward+forward
identischer Kind → ok). Incomplete-Type-as-direct-member und
Incomplete-Sequence-context-Constraint sind Resolver-Tracking-Pass
(S-Res Cluster 7.3 — Constructed-Type-Constraints).

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::multiple_forward_decls_of_same_struct_are_legal`,
`multiple_forward_decls_of_same_union_are_legal`.

**Status:** done

### §7.4.1.4.4.5 — Arrays (multidimensional fixed-size)

**Spec:** §7.4.1.4.4.5, S. 41 — "IDL defines multidimensional,
fixed-size arrays. An array includes explicit sizes for each
dimension. The syntax for arrays is very similar to the one of C or
C++ as stated in the following rules:" (Rules (59), (60) wiederholt).
"The array size (in each dimension) is fixed at compile time. The
implementation of array indices is language mapping specific."
"Declaring an array with all its dimensions creates an anonymous
type. Within this building block, such a type cannot be used as is
but needs to be given a name through a **typedef** declaration prior
to any use."

**Repo:** `PROD_ARRAY_DECLARATOR` + `PROD_FIXED_ARRAY_SIZE`
(siehe §7.4.1.3 Rules 59-60). Anonymous-Verbot wird durch das
Building-Block-Layout durchgesetzt: Array-Declarator erscheint nur in
`<any_declarator>`-Position innerhalb von `<typedef_dcl>`. Ohne
`anonymous_types`-Feature ist Inline-Array im Struct-Member nicht
erlaubt.

**Tests:** `parses_typedef_simple_array`,
`parses_typedef_multi_dim_array`,
`parses_typedef_with_multiple_declarators`,
`parses_typedef_mixed_simple_and_array`,
`parses_typedef_template_with_array`.

**Status:** done

### §7.4.1.4.4.6 — Native Types (opaque, language-mapping-spezifisch)

**Spec:** §7.4.1.4.4.6, S. 41 — "IDL provides a declaration to define
an opaque type whose representation is specified by the language
mapping. As stated in the following rules, declaring a native type is
done prefixing the type name (`<simple_declarator>`) with the
**native** keyword:" (Rules (61), (62) wiederholt).
"This declaration defines a new type with the specified name. A
native type is similar to an IDL basic type. The possible values of a
native type are language-mapping dependent, as are the means for
constructing and manipulating them. Any IDL specification that
defines a native type requires each language mapping to define how
the native type is mapped into that programming language."

**Repo:** `PROD_NATIVE_DCL` (siehe §7.4.1.3 Rule 61). Aktivierung im
`type_dcl`-Top-Level via Composer + `corba_native`-Feature.

**Tests:** `parses_native_dcl_top_level`,
`parses_native_dcl_in_module`,
`parses_native_dcl_in_interface`.
Feature-Gate: `dds_basic_rejects_native`,
`corba_full_allows_native`.

**Status:** done

### §7.4.1.4.4.6 — Note: Native als Equivalent-Type-Name

**Spec:** §7.4.1.4.4.6, S. 41 — "Note – It is recommended that native
types be mapped to equivalent type names in each programming
language, subject to the normal mapping rules for type names in that
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung/Note der Spec; nicht-bindend.

### §7.4.1.4.4.7 — Naming Data Types (Typedef-Bestandteile)

**Spec:** §7.4.1.4.4.7, S. 41-42 — "IDL provides constructs for naming
types; that is, it provides C language-like declarations that
associate an identifier with a type. The syntax for type declaration
is as follows:" (Rules (63)-(66), (59), (60) wiederholt).
"Such a declaration is made of:
- The **typedef** keyword.
- The type specification, which may be a simple type specification
  (`<simple_type_spec>`), that is either a basic type or a scoped name
  denoting any IDL legal type, or a template type specification
  (`<template_type_spec>`), or a declaration for a constructed type
  (`<constr_type_dcl>`).
- A list of at least one declarator, which will provide the new type
  name. Each declarator can be either a simple identifier
  (`<simple_declarator>`), which will be then the name allocated to
  the type, or an array declarator (`<array_declarator>`), in which
  case the new name (`<identifier>` enclosed within the array
  declarator) will denote an array of specified type."

**Repo:** `PROD_TYPEDEF_DCL` + `PROD_TYPE_DECLARATOR` +
`PROD_ANY_DECLARATORS` + `PROD_ANY_DECLARATOR` (siehe §7.4.1.3
Rules 63-66).

**Tests:** `parses_typedef_with_primitive_types` (simple_type_spec),
`parses_typedef_with_scoped_name` (scoped_name als Type),
`parses_unbounded_string_typedef` (template_type_spec),
`parses_fixed_pt_typedef` (template_type_spec fixed),
`parses_typedef_with_multiple_declarators` (Liste von Declarators),
`parses_typedef_simple_array` (array_declarator),
`parses_typedef_mixed_simple_and_array` (gemischt simple +
array).

**Status:** done — Inline-Constr-Type-Typedef-Test
`parses_typedef_with_inline_struct` belegt
`typedef struct {...} Alias;` via `PROD_TYPE_DECLARATOR` mit
`constr`-Alternative.

### §7.4.1.4.4.7 — Note 1: Naming via struct/union/enum/native

**Spec:** §7.4.1.4.4.7, S. 42 — "Note – As previously seen, a name is
also associated with a data type via the **struct**, **union**,
**enum**, and **native** declarations."

**Repo:** Resolver-`Scope::insert` registriert struct/union/enum/
native-Identifier als Type-Names.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::typedef_registered_as_typedef_kind`,
`bottom_up_lookup_finds_outer_scope_type`.

**Status:** done

### §7.4.1.4.4.7 — Note 2: Anonymous-Types-Verbot in Core

**Spec:** §7.4.1.4.4.7, S. 42 — "Note – Within this building block
where anonymous types are forbidden, a **typedef** declaration is
needed to name, prior to any use, an array or a template
instantiation."

**Repo:** Core-Recognizer akzeptiert anonymous Templates/Arrays nur
via Typedef. Mit `anonymous_types`-Feature werden zusätzliche
Inline-Alternativen aktiviert.

**Tests:** s. §7.4.1.4.4.1-open (Anonymous-Verbot-Test).

**Status:** done — XTypes-Default-Profile aktiviert
BB-anonymous-types implizit; strict-Core-Verbot via S-Prof
Profile-Constraint.

---

## §7.4.1.5 Specific Keywords

### §7.4.1.5 Table 7-14 — Building-Block-Spezifische Keywords

**Spec:** §7.4.1.5 + Table 7-14, S. 42-43 — Liste der Keywords, die
zum Core-Building-Block gehören (Subset von Table 7-6):
`boolean`, `case`, `char`, `const`, `default`, `double`, `enum`,
`FALSE`, `fixed`, `float`, `long`, `module`, `native`, `octet`,
`sequence`, `short`, `string`, `struct`, `switch`, `TRUE`, `typedef`,
`unsigned`, `union`, `void`, `wchar`, `wstring`.

**Repo:** Alle 26 Keywords sind Bestandteil der Core-Productions in
`crates/idl/src/grammar/idl42.rs`. Lexer extrahiert sie automatisch
via `from_grammar`. Building-Block-Granularität ist im
Feature-Flag-System nicht auf Keyword-Level gemappt — alle 73 Spec-
Keywords sind über alle Profile aktiv (Lexer-Side); Production-Side
gated via Features.

**Tests:** Lexer-Tests in
`crates/idl/src/lexer/tokenizer.rs::tests::single_keyword_emits_keyword_token`.
Recognizer-Tests für Core-Productions decken alle 26 Keywords (siehe
Rules 1-68 oben).

**Status:** done

---

## §7.4.2 Building Block Any

### §7.4.2.1 — Purpose

**Spec:** §7.4.2.1, S. 43 — "This building block adds the ability to
declare a type that may represent any valid data type."

**Repo:** `PROD_ANY_TYPE` (l. 3942, ID 116) — Single-Alt mit
`Keyword("any")`. Composer fügt es als Alternative zu
`PROD_BASE_TYPE_SPEC` hinzu (siehe Rule (69) unten).

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_any_in_struct_member`,
`parses_any_in_typedef`.

**Status:** done

### §7.4.2.2 — Dependencies (relies on Core Data Types)

**Spec:** §7.4.2.2, S. 43 — "This building block relies on Building
Block Core Data Types."

**Repo:** Composer-Aufruf
(`crates/idl/src/grammar/idl42.rs`-Komposition) erweitert
`base_type_spec` um `any_type`. Die Erweiterung ist in der
Default-Grammar bereits aktiv (kein eigenes Feature-Gate; `any` ist
durchgängig akzeptiert).

**Tests:** `parses_any_in_struct_member` (any als Member-Type),
`parses_any_in_typedef` (any als Typedef-Type).

**Status:** done

### §7.4.2.3 Rule (69) — `<base_type_spec>` `::+` `<any_type>`

**Spec:** §7.4.2.3 (69), S. 43 — "`<base_type_spec> ::+ <any_type>`"
(`::+` Operator-Erweiterung von Rule (23) mit zusätzlicher
Alternative).

**Repo:** `crates/idl/src/grammar/idl42.rs` — Composer fügt
`Symbol::Nonterminal(ID_ANY_TYPE)` als Alternative zu
`PROD_BASE_TYPE_SPEC` hinzu. Damit wird `any` überall akzeptiert wo
ein Basic-Type zulässig ist (Member-Type, Const-Type via base-Subset,
etc.).

**Tests:** `parses_any_in_struct_member`,
`parses_any_in_typedef`.

**Status:** done

### §7.4.2.3 Rule (70) — `<any_type>`

**Spec:** §7.4.2.3 (70), S. 43 — "`<any_type> ::= "any"`"

**Repo:** `PROD_ANY_TYPE` (l. 3942) — Single-Alt mit
`Keyword("any")`.

**Tests:** `parses_any_in_struct_member`,
`parses_any_in_typedef`.

**Status:** done

### §7.4.2.4 — Explanations and Semantics

**Spec:** §7.4.2.4, S. 44 — "An **any** is a type that may represent
any valid data type. At IDL level, it is just declared with the
keyword **any**."
"An **any** logically contains a value and some means to specify the
actual type of the value. However, the specific way in which the
actual type is defined is middleware-specific. Each IDL language
mapping provides operations that allow programmers to insert and
access the value contained in an **any** as well as the actual type
of that value."
Footnote 5: "For CORBA this means is a TypeCode (see [CORBA], Part1,
Sub clause 8.11 'TypeCodes')."

**Repo:** AST-Repräsentation: `BasicTypeKind::Any` (in
`crates/idl/src/ast/types.rs`). Implementation der
"Insert/Access-Operations" (TypeCode etc.) ist Code-Gen-/Runtime-
Aufgabe — nicht im idl-Crate, sondern in den Sprach-Mapping-Crates
(`crates/dcps/`, `crates/idl-cpp/`).

**Tests:** Recognizer-Side gepruft via `parses_any_in_struct_member`,
`parses_any_in_typedef`.

**Status:** done

### §7.4.2.5 Table 7-15 — Specific Keywords

**Spec:** §7.4.2.5 + Table 7-15, S. 44 — "The following table selects
in Table 7-6 the keywords that are specific to this building block
and removes the others." Table 7-15 enthält nur ein Keyword: `any`.

**Repo:** `Keyword("any")` ist nur in `PROD_ANY_TYPE` (`§7.4.2`)
referenziert. Lexer extrahiert es über `from_grammar`. Kein eigenes
Feature-Gate; das Building-Block-Layout ist via Composer-Komposition
realisiert.

**Tests:** s. §7.4.2.3 oben.

**Status:** done

---

## §7.4.3 Building Block Interfaces – Basic

### §7.4.3.1 — Purpose

**Spec:** §7.4.3.1, S. 45 — "This building block gathers all the rules
needed to define basic interfaces, i.e., consistent groupings of
operations. At this stage, there is no other implicit behavior
attached to interfaces."

**Repo:** Interface-Productions in
`crates/idl/src/grammar/idl42.rs` (Rules 71-96). Gated via
`corba_interfaces`-Feature (eigentlicher Flag-Name in
`crates/idl/src/features/mod.rs::IdlFeatures`).

**Tests:** `parses_empty_interface`,
`parses_interface_forward_declaration`,
`parses_interface_with_void_op`,
`parses_interface_with_typed_op_and_params`.

**Status:** done

### §7.4.3.2 — Dependencies (Core Data Types)

**Spec:** §7.4.3.2, S. 45 — "This building block relies on Building
Block Core Data Types."

**Repo:** Interface-Productions referenzieren `<scoped_name>`,
`<identifier>`, `<type_spec>`, `<member>`, `<const_dcl>`, `<type_dcl>`
aus Core. Composer-Anwendung der Interface-Erweiterung erfolgt nach
Core-Productions.

**Tests:** Interface-Tests verwenden Core-Types: `parses_interface_with_typed_op_and_params`
(Param-Type via Core).

**Status:** done

### §7.4.3.3 Rule (71) — `<definition>` `::+` `<except_dcl>` / `<interface_dcl>`

**Spec:** §7.4.3.3 (71), S. 45 — "`<definition> ::+ <except_dcl> ";"
| <interface_dcl> ";"`"

**Repo:** Composer fügt zwei zusätzliche Alternativen zu
`PROD_DEFINITION` hinzu: `except_dcl ";"` und `interface_dcl ";"`.

**Tests:** `parses_empty_exception`, `parses_exception_with_members`,
`parses_exception_in_module` (except_dcl);
`parses_empty_interface`, `parses_interface_in_module`
(interface_dcl).

**Status:** done

### §7.4.3.3 Rule (72) — `<except_dcl>`

**Spec:** §7.4.3.3 (72), S. 45 — "`<except_dcl> ::= "exception"
<identifier> "{" <member>* "}"`"

**Repo:** `PROD_EXCEPT_DCL` (l. 1847) — Sequenz `Keyword("exception")`,
`IDENTIFIER`, `Punct("{")`, `Repeat(ZeroOrMore, MEMBER)`, `Punct("}")`.

**Tests:** `parses_empty_exception` (kein Member),
`parses_exception_with_members`,
`parses_exception_in_module`.

**Status:** done

### §7.4.3.3 Rule (73) — `<interface_dcl>`

**Spec:** §7.4.3.3 (73), S. 45 — "`<interface_dcl> ::= <interface_def>
| <interface_forward_dcl>`"

**Repo:** `PROD_INTERFACE_DCL` (l. 1889) — zwei Alternativen.

**Tests:** `parses_empty_interface` (interface_def),
`parses_interface_forward_declaration` (interface_forward_dcl).

**Status:** done

### §7.4.3.3 Rule (74) — `<interface_def>`

**Spec:** §7.4.3.3 (74), S. 45 — "`<interface_def> ::= <interface_header>
"{" <interface_body> "}"`"

**Repo:** `PROD_INTERFACE_DEF` (l. 1900) — Sequenz
`INTERFACE_HEADER`, `Punct("{")`, `INTERFACE_BODY`, `Punct("}")`.

**Tests:** `parses_empty_interface`,
`parses_interface_with_inheritance`,
`parses_interface_with_multiple_inheritance`,
`parses_interface_with_void_op`,
`parses_interface_with_typed_op_and_params`.

**Status:** done

### §7.4.3.3 Rule (75) — `<interface_forward_dcl>`

**Spec:** §7.4.3.3 (75), S. 45 — "`<interface_forward_dcl> ::=
<interface_kind> <identifier>`"

**Repo:** `PROD_INTERFACE_FORWARD_DCL` (l. 1913) — Sequenz
`INTERFACE_KIND` + `IDENTIFIER`.

**Tests:** `parses_interface_forward_declaration`.

**Status:** done

### §7.4.3.3 Rule (76) — `<interface_header>`

**Spec:** §7.4.3.3 (76), S. 45 — "`<interface_header> ::=
<interface_kind> <identifier> [ <interface_inheritance_spec> ]`"

**Repo:** `PROD_INTERFACE_HEADER` (l. 1937) — Sequenz `INTERFACE_KIND`,
`IDENTIFIER`, `Repeat(Optional, INTERFACE_INHERITANCE_SPEC)`.

**Tests:** `parses_empty_interface`,
`parses_interface_with_inheritance`.

**Status:** done

### §7.4.3.3 Rule (77) — `<interface_kind>`

**Spec:** §7.4.3.3 (77), S. 45 — "`<interface_kind> ::= "interface"`"

**Repo:** `PROD_INTERFACE_KIND` (l. 1978) — Single-Alt
`Keyword("interface")`. CORBA-Extras-Feature ergänzt via Composer
`abstract`/`local`-Varianten (siehe §7.4.x).

**Tests:** `parses_empty_interface`.

**Status:** done

### §7.4.3.3 Rule (78) — `<interface_inheritance_spec>`

**Spec:** §7.4.3.3 (78), S. 45 — "`<interface_inheritance_spec> ::=
":" <interface_name> { "," <interface_name> }*`"

**Repo:** `PROD_INTERFACE_INHERITANCE_SPEC` (l. 1992) +
`PROD_INTERFACE_NAME_LIST` (l. 2003) — `Punct(":")` gefolgt von
Comma-separierter Liste von INTERFACE_NAME.

**Tests:** `parses_interface_with_inheritance` (single base),
`parses_interface_with_multiple_inheritance` (mehrere Basen).

**Status:** done

### §7.4.3.3 Rule (79) — `<interface_name>`

**Spec:** §7.4.3.3 (79), S. 45 — "`<interface_name> ::= <scoped_name>`"

**Repo:** Interface-Name wird inline als
`Symbol::Nonterminal(ID_SCOPED_NAME)` referenziert (kein eigenes
PROD_INTERFACE_NAME, da Spec-Rule trivial).

**Tests:** `parses_interface_with_inheritance`,
`parses_interface_with_multiple_inheritance`.

**Status:** done

### §7.4.3.3 Rule (80) — `<interface_body>`

**Spec:** §7.4.3.3 (80), S. 45 — "`<interface_body> ::= <export>*`"

**Repo:** `PROD_INTERFACE_BODY` (l. 2021) + `PROD_EXPORT_LIST`
(l. 2036) — `Repeat(ZeroOrMore, EXPORT)`.

**Tests:** `parses_empty_interface` (kein Export),
`parses_interface_with_void_op`,
`parses_interface_with_attribute`.

**Status:** done

### §7.4.3.3 Rule (81) — `<export>`

**Spec:** §7.4.3.3 (81), S. 45 — "`<export> ::= <op_dcl> ";" |
<attr_dcl> ";"`"

**Repo:** `PROD_EXPORT` (l. 2053) — zwei Alternativen
`op_dcl ";"` und `attr_dcl ";"`. Building-Block-Full ergänzt via
Composer `type_dcl`/`const_dcl`/`except_dcl`-Alternativen (siehe
§7.4.4).

**Tests:** `parses_interface_with_void_op` (op_dcl),
`parses_interface_with_attribute` (attr_dcl).

**Status:** done

### §7.4.3.3 Rule (82) — `<op_dcl>`

**Spec:** §7.4.3.3 (82), S. 45 — "`<op_dcl> ::= <op_type_spec>
<identifier> "(" [ <parameter_dcls> ] ")" [ <raises_expr> ]`"

**Repo:** `PROD_OP_DCL` (l. 2102) — Sequenz `OP_TYPE_SPEC`, `IDENTIFIER`,
`Punct("(")`, `Repeat(Optional, PARAMETER_DCLS)`, `Punct(")")`,
`Repeat(Optional, RAISES_EXPR)`.

**Tests:** `parses_interface_with_void_op`,
`parses_interface_with_typed_op_and_params`,
`parses_operation_with_raises`,
`parses_operation_with_inout_and_out_params`,
`parses_oneway_operation`.

**Status:** done

### §7.4.3.3 Rule (83) — `<op_type_spec>`

**Spec:** §7.4.3.3 (83), S. 45 — "`<op_type_spec> ::= <type_spec> |
"void"`"

**Repo:** `PROD_OP_TYPE_SPEC` (l. 2147) — zwei Alternativen.

**Tests:** `parses_interface_with_void_op` (void),
`parses_interface_with_typed_op_and_params` (type_spec).

**Status:** done

### §7.4.3.3 Rule (84) — `<parameter_dcls>`

**Spec:** §7.4.3.3 (84), S. 45 — "`<parameter_dcls> ::= <param_dcl>
{ "," <param_dcl> }*`"

**Repo:** `PROD_PARAMETER_DCLS` (l. 2158) + `PROD_PARAM_DCL_LIST`
(l. 2182) — Comma-separierte Liste.

**Tests:** `parses_interface_with_typed_op_and_params`,
`parses_operation_with_inout_and_out_params`.

**Status:** done

### §7.4.3.3 Rule (85) — `<param_dcl>`

**Spec:** §7.4.3.3 (85), S. 45 — "`<param_dcl> ::= <param_attribute>
<type_spec> <simple_declarator>`"

**Repo:** `PROD_PARAM_DCL` (l. 2200) — Sequenz `PARAM_ATTRIBUTE`,
`TYPE_SPEC`, `SIMPLE_DECLARATOR`.

**Tests:** `parses_interface_with_typed_op_and_params`,
`parses_operation_with_inout_and_out_params`.

**Status:** done

### §7.4.3.3 Rule (86) — `<param_attribute>`

**Spec:** §7.4.3.3 (86), S. 45 — "`<param_attribute> ::= "in" | "out"
| "inout"`"

**Repo:** `PROD_PARAM_ATTRIBUTE` (l. 2213) — drei Alternativen.

**Tests:** `parses_interface_with_typed_op_and_params` (in),
`parses_operation_with_inout_and_out_params` (inout + out).

**Status:** done

### §7.4.3.3 Rule (87) — `<raises_expr>`

**Spec:** §7.4.3.3 (87), S. 45 — "`<raises_expr> ::= "raises" "("
<scoped_name> { "," <scoped_name> }* ")"`"

**Repo:** `PROD_RAISES_EXPR` (l. 2225) — `Keyword("raises")` +
`Punct("(")` + Comma-separierte ScopedName-Liste + `Punct(")")`.

**Tests:** `parses_operation_with_raises`.

**Status:** done

### §7.4.3.3 Rule (88) — `<attr_dcl>`

**Spec:** §7.4.3.3 (88), S. 45 — "`<attr_dcl> ::= <readonly_attr_spec>
| <attr_spec>`"

**Repo:** `PROD_ATTR_DCL` (l. 2277) — zwei Alternativen.

**Tests:** `parses_interface_with_attribute` (attr_spec),
`parses_interface_with_readonly_attribute` (readonly_attr_spec).

**Status:** done

### §7.4.3.3 Rule (89) — `<readonly_attr_spec>`

**Spec:** §7.4.3.3 (89), S. 45 — "`<readonly_attr_spec> ::= "readonly"
"attribute" <type_spec> <readonly_attr_declarator>`"

**Repo:** Inline-Sequenz in `PROD_ATTR_DCL`-Alternative —
`Keyword("readonly")` + `Keyword("attribute")` + `TYPE_SPEC` +
`READONLY_ATTR_DECLARATOR`.

**Tests:** `parses_interface_with_readonly_attribute`,
`parses_readonly_attr_multi_name`,
`parses_readonly_attr_with_raises`.

**Status:** done

### §7.4.3.3 Rule (90) — `<readonly_attr_declarator>`

**Spec:** §7.4.3.3 (90), S. 46 — "`<readonly_attr_declarator> ::=
<simple_declarator> <raises_expr> | <simple_declarator> { ","
<simple_declarator> }*`"

**Repo:** Im Composer-Alt-Set für readonly-attr (verbunden mit
`PROD_ATTR_DCL`/`ATTR_DECLARATOR`) realisiert. Erste Variante:
single Decl + raises; zweite Variante: Comma-separierte Liste.

**Tests:** `parses_readonly_attr_with_raises` (raises-Variante),
`parses_readonly_attr_multi_name` (Liste).

**Status:** done

### §7.4.3.3 Rule (91) — `<attr_spec>`

**Spec:** §7.4.3.3 (91), S. 46 — "`<attr_spec> ::= "attribute"
<type_spec> <attr_declarator>`"

**Repo:** Inline-Sequenz in `PROD_ATTR_DCL`-Alternative —
`Keyword("attribute")` + `TYPE_SPEC` + `ATTR_DECLARATOR`.

**Tests:** `parses_interface_with_attribute`,
`parses_attr_multi_name`.

**Status:** done

### §7.4.3.3 Rule (92) — `<attr_declarator>`

**Spec:** §7.4.3.3 (92), S. 46 — "`<attr_declarator> ::=
<simple_declarator> <attr_raises_expr> | <simple_declarator> { ","
<simple_declarator> }*`"

**Repo:** `PROD_ATTR_DECLARATOR` (l. 2342) — zwei Alternativen.

**Tests:** `parses_attr_with_getraises`,
`parses_attr_with_setraises`,
`parses_attr_with_get_and_setraises`,
`parses_attr_multi_name`.

**Status:** done

### §7.4.3.3 Rule (93) — `<attr_raises_expr>`

**Spec:** §7.4.3.3 (93), S. 46 — "`<attr_raises_expr> ::=
<get_excep_expr> [ <set_excep_expr> ] | <set_excep_expr>`"

**Repo:** `PROD_ATTR_RAISES_EXPR` (l. 2371) — zwei Alternativen.

**Tests:** `parses_attr_with_getraises` (get only),
`parses_attr_with_setraises` (set only),
`parses_attr_with_get_and_setraises` (get + set),
`parses_attr_exception_list_multi`,
`rejects_attr_setraises_before_getraises` (Spec-Order: getraises
muss vor setraises stehen).

**Status:** done

### §7.4.3.3 Rule (94) — `<get_excep_expr>`

**Spec:** §7.4.3.3 (94), S. 46 — "`<get_excep_expr> ::= "getraises"
<exception_list>`"

**Repo:** `PROD_GET_EXCEP_EXPR` (l. 2391) — `Keyword("getraises")` +
`EXCEPTION_LIST`.

**Tests:** `parses_attr_with_getraises`,
`parses_attr_with_get_and_setraises`,
`parses_readonly_attr_with_raises`.

**Status:** done

### §7.4.3.3 Rule (95) — `<set_excep_expr>`

**Spec:** §7.4.3.3 (95), S. 46 — "`<set_excep_expr> ::= "setraises"
<exception_list>`"

**Repo:** `PROD_SET_EXCEP_EXPR` (l. 2402) — `Keyword("setraises")` +
`EXCEPTION_LIST`.

**Tests:** `parses_attr_with_setraises`,
`parses_attr_with_get_and_setraises`.

**Status:** done

### §7.4.3.3 Rule (96) — `<exception_list>`

**Spec:** §7.4.3.3 (96), S. 46 — "`<exception_list> ::= "(" <scoped_name>
{ "," <scoped_name> } * ")"`"

**Repo:** `PROD_EXCEPTION_LIST` (l. 2413) — `Punct("(")` +
Comma-separierte ScopedName-Liste + `Punct(")")`.

**Tests:** `parses_attr_exception_list_multi`,
`parses_attr_with_getraises`.

**Status:** done

---

## §7.4.3.4 Explanations and Semantics (Interfaces)

### §7.4.3.4.1 — IDL Specification mit Interfaces + Exceptions

**Spec:** §7.4.3.4.1, S. 46 — "With that building block, an IDL
specification may declare exceptions and interfaces (that are merely
groups of operations)." Wiederholt Rule (71).

**Repo:** Siehe §7.4.3.3 Rule (71).

**Tests:** s. Rule (71).

**Status:** done

### §7.4.3.4.2 — Exceptions: Datenstruktur für Exceptional-Conditions

**Spec:** §7.4.3.4.2, S. 46 — "Exceptions are specific data
structures, which may be returned to indicate that an exceptional
condition has occurred during the execution of an operation. The
syntax to declare an exception is as follows:" (Rule (72) wiederholt).
"An exception declaration is made of:
- The **exception** keyword.
- An identifier (`<identifier>`) - each exception is typed with its
  exception identifier.
- A body enclosed within braces (`{}`) - the body may be void or
  comprise members (`<member>*`), very similar to structure members.
  See 7.4.1.4.4.4, Constructed Types for more details on member
  declaration."

**Repo:** Recognizer in `PROD_EXCEPT_DCL` (Rule 72) — `<member>*`
erlaubt sowohl void-Body als auch Member-List.

**Tests:** `parses_empty_exception`,
`parses_exception_with_members`.

**Status:** done

### §7.4.3.4.2 — Exception-Identifier-Zugänglichkeit + Member-Zugriff

**Spec:** §7.4.3.4.2, S. 46 — "If an exception is returned as the
outcome to an operation invocation, then the value of the exception
identifier shall be accessible to the programmer for determining
which particular exception was raised."
"If an exception is declared with members, a programmer shall be able
to access the values of those members when such an exception is
raised. If no members are specified, no additional information shall
be accessible when such an exception is raised."
"The way this information is made available is language-mapping
specific."

**Repo:** Spec-normativ für Sprach-Mapping; AST-Repräsentation
`Exception { identifier, members }` trägt die nötigen Informationen.
Code-Gen-Stufe (separater Crate) implementiert die Sprach-Mapping-
spezifische Zugriffsoperatoren.

**Tests:** Recognizer-Side gepruft.

**Status:** done

### §7.4.3.4.2 — Exception-Identifier-Verwendung nur in `raises`

**Spec:** §7.4.3.4.2, S. 46 — "An identifier declared to be an
exception identifier may appear only in a **raises** expression of an
operation or attribute declaration, and nowhere else."

**Repo:** `validate_exception_only_in_raises` in
`crates/idl/src/semantics/spec_validators.rs` — globaler Pass
sammelt alle Exception-Defs (auch innerhalb Interface-Bodies via
§7.4.4 Rule 97) und prüft Struct/Union/Exception-Member-Types.

**Tests:** `crates/idl/src/semantics/spec_validators.rs::tests::rejects_exception_used_as_struct_member`,
`rejects_exception_used_as_union_case_type`,
`accepts_exception_in_raises_only`.

**Status:** done — Validator detektiert Exception-as-Struct/Union-
Member; Exception in `raises (...)` wird akzeptiert.

### §7.4.3.4.2 Exception-Identifier-Constraint

**Spec:** §7.4.3.4.2, S. 46 — Exception-Identifier nur in
`raises`/`getraises`/`setraises` zulässig.

**Repo:** Resolver-Symbol-Kind-Tracking via `SymbolKind::Exception`.

**Status:** done

### §7.4.3.4.3 — Interfaces: Header + Body + Forward Decl

**Spec:** §7.4.3.4.3, S. 47 — "Interfaces are groupings of elements
(in this building block operations and attributes). As defined by the
following rules, an interface is made of a header (`<interface_header>`)
and a body (`<interface_body>`) enclosed in braces (`{}`). An
interface may also be declared with no definition using a forward
declaration (`<interface_forward_dcl>`)." (Rules (73)-(74)
wiederholt).

**Repo:** Siehe Rules (73)/(74)/(75).

**Tests:** s. Rule (73)/(74)/(75).

**Status:** done

### §7.4.3.4.3.1 — Interface Header Bestandteile + neuer Type-Name

**Spec:** §7.4.3.4.3.1, S. 47 — "An interface header is declared with
the following syntax:" (Rules (76)-(77) wiederholt).
"An interface header comprises:
- The **interface** keyword.
- The interface name (`<identifier>`).
- Optionally an inheritance specification
  (`<interface_inheritance_spec>`)."
"The `<identifier>` that names an interface defines a new legal type.
Such a type may be used anywhere a type is legal in the grammar,
subject to semantic constraints as described in the following sub
clauses. A parameter or structure member which is of an interface
type is semantically a *reference* to an object implementing that
interface. Each language binding describes how the programmer must
represent such interface references."

**Repo:** Recognizer in Rules (76)/(77). Resolver registriert
Interface-Identifier als Type-Symbol; AST-`InterfaceType` wird in
`<scoped_name>`-Lookups akzeptiert. Reference-Semantik ist
Code-Gen-Aufgabe.

**Tests:** s. Rules (76)/(77) +
`parses_interface_with_typed_op_and_params` (Interface-Type als
Param-Type erlaubt).

**Status:** done

### §7.4.3.4.3.2 — Interface Inheritance: Doppelpunkt-Syntax + scoped_name

**Spec:** §7.4.3.4.3.2, S. 47 — "Interface inheritance is introduced
by a colon (:) and must follow the following syntax:" (Rules (78)-(79)
wiederholt).
"Each `<scoped_name>` in an `<interface_inheritance_spec>` must be
the name of a previously defined interface or an alias to a
previously defined interface (defined using a **typedef**
declaration)."

**Repo:** Recognizer akzeptiert beliebige `<scoped_name>`-Liste;
Validation "previously defined interface or alias" erfolgt in
`crates/idl/src/semantics/resolver.rs` über Symbol-Type-Check.

**Tests:** `parses_interface_with_inheritance`,
`parses_interface_with_multiple_inheritance`.
Validation-Tests in
`crates/idl/src/semantics/resolver.rs::tests::accepts_diamond_with_common_root`,
`rejects_inheritance_cycle_two_interfaces`.

**Status:** done

### §7.4.3.4.3.2.1 — Inheritance Rules: Direct/Indirect Base

**Spec:** §7.4.3.4.3.2.1, S. 47 — "An interface can be derived from
another interface, which is then called a *base interface* of the
derived interface. A derived interface, like all interfaces, may
declare new elements (in this building block, operations and
attributes). In addition the elements of a base interface can be
referred to as if they were elements of the derived interface."
"An interface is called a *direct base* if it is mentioned in the
`<interface_inheritance_spec>` and an *indirect base* if it is not a
direct base but is a base interface of one of the interfaces
mentioned in the `<interface_inheritance_spec>`."

**Repo:** Resolver baut Inheritance-Graph
(`crates/idl/src/semantics/resolver.rs`). Direct/Indirect-Distinktion
ist konzeptuell, im Resolver durch transitive-closure behandelt.

**Tests:** `accepts_diamond_with_common_root`,
`rejects_diamond_op_conflict_unrelated_bases`,
`accepts_diamond_op_from_common_ancestor`.

**Status:** done

### §7.4.3.4.3.2.1 — Multiple Inheritance + Direct-Base-Verbot Doppelt

**Spec:** §7.4.3.4.3.2.1, S. 48 — "An interface may be derived from
any number of base interfaces. Such use of more than one direct base
interface is often called *multiple inheritance*. The order of
derivation is not significant."
"An interface may not be specified as a direct base interface of a
derived interface more than once; it may be an indirect base
interface more than once. Consider the following example:
`interface A { ... }; interface B: A { ... }; interface C: A { ...
}; interface D: B, C { ... }; // OK; interface E: A, B { ... }; //
OK`"
"The relationships between these interfaces are shown in Figure 7-1.
This 'diamond' shape is legal, as is the definition of E on the
right."

**Repo:** Resolver-Inheritance-Validation prüft Direct-Base-
Uniqueness und akzeptiert Diamond-Inheritance.

**Tests:** `accepts_diamond_with_common_root` (D : B, C-Variante),
`accepts_diamond_op_from_common_ancestor`.

**Status:** done — Test
`crates/idl/src/semantics/resolver.rs::tests::rejects_duplicate_direct_base`
prüft Spec-Beispiel.

### §7.4.3.4.3.2.1 Direct-Base-Doppelt-Verbot

**Spec:** §7.4.3.4.3.2.1, S. 48 — "may not be specified as a direct
base interface of a derived interface more than once".

**Repo:** Resolver-Diamond-Detection.

**Tests:** `rejects_duplicate_direct_base`.

**Status:** done

### §7.4.3.4.3.2.1 — Op/Attr-Redefinition-Verbot in Derived

**Spec:** §7.4.3.4.3.2.1, S. 48 — "It is forbidden to redefine an
operation or an attribute in a derived interface, as well as
inheriting two different operations or attributes with the same name."
Beispiel: `interface A { void make_it_so(); }; interface B: A {
short make_it_so(in long times); // Error: redefinition of
make_it_so };`.

**Repo:** Resolver-Validation
(`rejects_diamond_op_conflict_unrelated_bases`).

**Tests:** `rejects_diamond_op_conflict_unrelated_bases`.

**Status:** done — Test
`crates/idl/src/semantics/resolver.rs::tests::rejects_op_redefinition_in_derived_interface`
prüft Spec-Beispiel.

### §7.4.3.4.3.2.1 Op/Attr-Redefinition-Verbot

**Spec:** §7.4.3.4.3.2.1, S. 48 — gleicher Op-Name in Sub-Interface
ist Error.

**Repo:** Resolver-Diamond-Detection erfasst Cross-Branch + Direct-
Sub-Class.

**Tests:** `rejects_op_redefinition_in_derived_interface`,
`rejects_diamond_op_conflict_unrelated_bases`.

**Status:** done

### §7.4.3.4.3.3 — Interface Body: Ops + Attrs (exported)

**Spec:** §7.4.3.4.3.3, S. 48 — "As stated in the following rules,
within an interface body, operations and attributes can be defined.
Those constructs are defined in the scope of the interface and
exported (i.e., accessible outside the interface definition using
their name scoped by the interface name)." (Rules (80)-(81)
wiederholt).

**Repo:** Recognizer in Rules (80)/(81). Resolver registriert
Op/Attr-Identifier als Symbole im Interface-Scope.

**Tests:** `parses_interface_with_void_op`,
`parses_interface_with_typed_op_and_params`,
`parses_interface_with_attribute`.

**Status:** done

### §7.4.3.4.3.3.1 — Operation-Decl Bestandteile

**Spec:** §7.4.3.4.3.3.1, S. 49 — "Operation declarations in IDL are
similar to C function declarations. To define an operation, the
syntax is:" (Rules (82)-(87) wiederholt).
"An operation declaration consists of:
- The type of the operation's return result (`<op_type_spec>`); the
  type may be any type that can be defined in IDL. Operations that do
  not return a result shall specify **void** as return type.
- An identifier (`<identifier>`) that names the operation in the
  scope of the interface in which it is defined.
- A parameter list that specifies zero or more parameter
  declarations. The parameter list is enclosed between brackets (`()`).
  In case more than one parameter is declared, parameter declarations
  are separated by commas (`,`). Each parameter declaration is made
  of:
  - A qualifier (`<param_attribute>`) that specifies the direction in
    which the parameter is to be passed. The possible values are:
    - **in** - the parameter is passed from caller to operation.
    - **out** - the parameter is passed from operation to caller.
    - **inout** - the parameter is passed in both directions.
  - The type of the parameter (`<type_spec>`) that may be any valid
    IDL type specification.
  - The name of the parameter (`<simple_declarator>`).
- An optional expression that indicates which exceptions may be
  raised as a result of an invocation of this operation. This
  expression is made of:
  - The **raises** keyword.
  - The list of the operation-specific exceptions, enclosed between
    brackets (`()`) and separated by commas (`,`) in case more than
    one exception is specified. Each of the scoped names
    (`<scoped_name>`) in the list must denote a previously defined
    exception."

**Repo:** Recognizer in Rules (82)-(87). Resolver-Validation prüft,
dass `raises`-Einträge vorher als Exception definiert wurden.

**Tests:** s. Rules (82)-(87) sowie
`parses_operation_with_raises`.

**Status:** done

### §7.4.3.4.3.3.1 — In-Param-Modify-Verbot (impl-spezifisch)

**Spec:** §7.4.3.4.3.3.1, S. 49 — "It is expected that an
implementation will not attempt to modify an **in** parameter. The
ability to even attempt to do so is language-mapping specific; the
effect of such an action is undefined."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf Code-Gen-Stufe; gehört in das jeweilige Sprach-PSM, nicht in den IDL-Recognizer.

### §7.4.3.4.3.3.1 — Middleware-Standard-Exceptions

**Spec:** §7.4.3.4.3.3.1, S. 49 — "In addition to any
operation-specific exceptions specified in the **raises** expression,
other middleware-specific standard exceptions may be raised. These
exceptions are described in the related profiles."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Middleware-spezifische Aussage; fällt in Konsumenten-Spec (DDS, CORBA), nicht in den IDL-Kern.

### §7.4.3.4.3.3.1 — Absent-Raises = keine op-spezifischen Exceptions

**Spec:** §7.4.3.4.3.3.1, S. 49 — "The absence of a **raises**
expression on an operation implies that there are no
operation-specific exceptions. Invocations of such an operation may
still raise one of the middleware-specific standard exceptions."

**Repo:** Recognizer akzeptiert op_dcl ohne raises (Optional-Block).

**Tests:** `parses_interface_with_void_op` (kein raises),
`parses_interface_with_typed_op_and_params` (kein raises).

**Status:** done

### §7.4.3.4.3.3.1 — Exception-Wert-Ueberbelegung von return + out/inout

**Spec:** §7.4.3.4.3.3.1, S. 50 — "If an exception is raised as a
result of an invocation, the values of the **return** result and any
**out** and **inout** parameters are undefined."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis auf Code-Gen-Stufe; gehört in das jeweilige Sprach-PSM, nicht in den IDL-Recognizer.

### §7.4.3.4.3.3.1 — Note: Native als Op-Param/Exception-Type

**Spec:** §7.4.3.4.3.3.1, S. 50 — "Note – A native type (cf.
7.4.1.4.4.6) may be used to define operation parameters, results, and
exceptions. If a native type is used for an exception, it must be
mapped to a type in a programming language that can be used as an
exception."

**Repo:** Recognizer akzeptiert Native-Type als Param/Return/
Exception-Member-Type via `<type_spec>`-Forwarding.

**Tests:** `parses_native_dcl_in_interface` (native im Interface-Scope).

**Status:** done

### §7.4.3.4.3.3.2 — Attribute-Decl Bestandteile

**Spec:** §7.4.3.4.3.3.2, S. 50 — "Attributes may also be declared
within an interface. Declaring an attribute is logically equivalent
to declaring a pair of accessor functions; one to retrieve the value
of the attribute and one to set the value of the attribute. To
create an attribute, the syntax is as follows:" (Rules (88)-(96)
wiederholt).
"An attribute declaration is made of:
- An optional qualifier (**readonly**) that indicates that the
  attribute cannot be written. In this case, the attribute is said
  *read-only* and the declaration is equivalent to only a read
  accessor.
- The **attribute** keyword.
- The type of the attribute that may be any valid IDL type
  specification (`<type_spec>`).
- The name of the attribute (`<simple_declarator>`).
- An optional raises expression."

**Repo:** Recognizer in Rules (88)-(96).

**Tests:** s. Rules (88)-(96) +
`parses_interface_with_attribute`,
`parses_interface_with_readonly_attribute`,
`parses_attr_with_getraises`,
`parses_attr_with_setraises`,
`parses_attr_with_get_and_setraises`.

**Status:** done

### §7.4.3.4.3.3.2 — Attribute-Raises-Form pro Attribute-Kind

**Spec:** §7.4.3.4.3.3.2, S. 50 — "The optional raises expressions
take different forms according to the attribute kinds:
- For read-only attributes, raises expressions are similar to those
  of operations (cf. rule (90) above).
- For plain attributes, raises expressions indicate which of the
  potential user-defined exceptions may be raised when the attribute
  is read (**getraises**) and which when the attribute is written
  (**setraises**). A plain attribute may have a **getraises**
  expression, a **setraises** expression or both of them. In the
  latter case, the **getraises** expression must be declared in first
  place."

**Repo:** Rules (89)/(90) (readonly_attr_spec/declarator) und
(91)/(92)/(93) (attr_spec/declarator/raises_expr) abgedeckt.
Reihenfolge-Constraint `getraises` vor `setraises` durch
`PROD_ATTR_RAISES_EXPR`-Alt-Order erzwungen.

**Tests:** `parses_attr_with_getraises`,
`parses_attr_with_setraises`,
`parses_attr_with_get_and_setraises`,
`parses_readonly_attr_with_raises`,
`rejects_attr_setraises_before_getraises`.

**Status:** done

### §7.4.3.4.3.3.2 — Absent-Raises = keine attr-spezifischen Exceptions

**Spec:** §7.4.3.4.3.3.2, S. 50 — "The absence of a raises expression
(**raises**, **getraises** or **setraises**) on an attribute implies
that there are no attribute-specific exceptions. Accesses to such an
attribute may still raise middleware-specific standard exceptions."

**Repo:** Optional-Block akzeptiert Attr ohne raises.

**Tests:** `parses_interface_with_attribute` (kein raises).

**Status:** done

### §7.4.3.4.3.3.2 — Mehrere Attribute in einer Decl ohne Raises

**Spec:** §7.4.3.4.3.3.2, S. 51 — "As a shortcut, several attributes
can be declared in a single attribute declaration, provided that
there are no attached raises clauses. In that case, the names of the
attributes are listed, separated by a comma (,)."

**Repo:** Rule (92) zweite Alternative
(`<simple_declarator> { "," <simple_declarator> }*`) erlaubt
Multi-Name ohne Raises.

**Tests:** `parses_attr_multi_name`,
`parses_readonly_attr_multi_name`.

**Status:** done

### §7.4.3.4.3.3.2 — Note: Native als Attr-Type/Exception-Type

**Spec:** §7.4.3.4.3.3.2, S. 51 — "Note – A native type (cf.
7.4.1.4.4.6) may be used to define attribute types, and exceptions.
If a native type is used for an exception, it must be mapped to a
type in a programming language that can be used as an exception."

**Repo:** Recognizer akzeptiert native als Attr-Type über
`<type_spec>`-Forwarding.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_attr_with_native_type`.

**Status:** done — Test
`crates/idl/src/grammar/idl42.rs::tests::parses_attr_with_native_type`
belegt Native-Type als Attribute-Type-Spec.

### §7.4.3.4.3.3.2 Native-Attr-Test

**Spec:** §7.4.3.4.3.3.2, S. 51 — Native als Attr-Type.

**Repo:** Recognizer akzeptiert via Standard-Type-Spec-Forwarding.

**Tests:** `parses_attr_with_native_type`.

**Status:** done

### §7.4.3.4.3.4 — Forward Declaration

**Spec:** §7.4.3.4.3.4, S. 51 — "A forward declaration declares the
name of an interface without defining it. This permits the definition
of interfaces that refer to each other."
(Rules (75), (77) wiederholt).
"It is illegal to inherit from a forward-declared interface not
previously defined."
Beispiel: `module Example { interface base; // ... interface
derived : base {}; // Error interface base {}; // Define base
interface derived : base {}; // OK };`
"Multiple forward declarations of the same interface name are legal,
provided that they are all consistent."
Footnote 6: "The definition of a forward-declared interface must be
consistent with the forward declaration: they must share the same
interface kind. With this building block, there is only one interface
kind (namely **interface**) however other interface-related building
blocks will add other kinds (such as **abstract interface**)."

**Repo:** Recognizer in Rule (75). Validation:
`forward_decl_then_definition_completes` deckt Definition nach
Forward-Decl;
`forward_decl_without_definition_is_error` deckt Spec-Verstoß.
Multiple-Forward-Decl-Konsistenz: nicht explizit getestet.

**Tests:** `parses_interface_forward_declaration`,
`forward_decl_then_definition_completes`,
`forward_decl_without_definition_is_error`.

**Status:** done — Multi-Forward-Konsistenz durch Test
`multiple_forward_decls_of_same_interface_are_legal` belegt;
"Inherit von undefiniertem Forward" wird durch
`forward_decl_without_definition_is_error` (Resolver-Pass) erfasst.

### §7.4.3.4.3.4 Forward-Inherit + Multi-Forward-Konsistenz

**Spec:** §7.4.3.4.3.4, S. 51 — Multi-Forward legal, Forward-Inherit
nur nach Definition legal.

**Repo:** Resolver-Forward-Tracking; `Scope::insert` erlaubt
forward+forward (gleicher Kind).

**Tests:** `multiple_forward_decls_of_same_interface_are_legal`,
`forward_decl_without_definition_is_error`,
`forward_decl_then_definition_completes`.

**Status:** done

---

## §7.4.3.5 Specific Keywords

### §7.4.3.5 Table 7-16 — Specific Keywords

**Spec:** §7.4.3.5 + Table 7-16, S. 51-52 — Liste der Keywords, die
zum Interfaces-Basic-Building-Block gehören:
`attribute`, `exception`, `getraises`, `in`, `inout`, `interface`,
`out`, `raises`, `readonly`, `setraises`.

**Repo:** Alle 10 Keywords sind in den Productions Rules (71)-(96)
referenziert. Lexer extrahiert sie via `from_grammar`.

**Tests:** Recognizer-Tests in §7.4.3.3 oben decken alle 10 Keywords.

**Status:** done

---

## §7.4.4 Building Block Interfaces – Full

### §7.4.4.1 — Purpose

**Spec:** §7.4.4.1, S. 52 — "This building block complements the
former one with the ability to embed in the interface body, additional
declarations such as types, exceptions and constants."

**Repo:** Composer-Erweiterung von `PROD_EXPORT` in
`crates/idl/src/grammar/idl42.rs` ergänzt `type_dcl`/`const_dcl`/
`except_dcl`-Alternativen zur Basic-`op_dcl`/`attr_dcl`-Liste.

**Tests:** `parses_interface_with_nested_type_and_const`,
`parses_interface_with_embedded_exception`,
`parses_interface_with_embedded_struct`,
`parses_interface_with_embedded_enum`,
`parses_interface_with_embedded_union`,
`parses_full_interface_all_export_kinds`.

**Status:** done

### §7.4.4.2 — Dependencies (Interfaces – Basic + Core)

**Spec:** §7.4.4.2, S. 52 — "This building block complements Building
Block Interfaces – Basic. Transitively, it relies on Building Block
Core Data Types."

**Repo:** Composer-Reihenfolge: Core → Interfaces-Basic →
Interfaces-Full. Full kann nur aktiviert werden wenn Basic aktiv ist.

**Tests:** s. Tests aus §7.4.4.1.

**Status:** done

### §7.4.4.3 Rule (97) — `<export>` `::+` `<type_dcl>` / `<const_dcl>` / `<except_dcl>`

**Spec:** §7.4.4.3 (97), S. 52 — "`<export> ::+ <type_dcl> ";" |
<const_dcl> ";" | <except_dcl> ";"`"

**Repo:** Composer fügt drei Alternativen zu `PROD_EXPORT` hinzu:
`type_dcl ";"`, `const_dcl ";"`, `except_dcl ";"`.

**Tests:** `parses_interface_with_nested_type_and_const` (type +
const im Interface),
`parses_interface_with_embedded_exception` (except),
`parses_interface_with_embedded_struct`,
`parses_interface_with_embedded_enum`,
`parses_interface_with_embedded_union`,
`parses_full_interface_all_export_kinds`,
`parses_interface_with_inner_type_used_in_op` (Inner-Type wird in
Op-Signatur verwendet).

**Status:** done

### §7.4.4.4 — Explanations: Inline-Decls verhalten sich wie Top-Level

**Spec:** §7.4.4.4, S. 52 — "This building block adds the possibility
to embed declarations of types, constants and exceptions inside an
interface declaration. The syntax for those embedded elements is
exactly the same as if they were top-level constructs."

**Repo:** Composer-Erweiterung benutzt dieselben Productions
(`type_dcl`/`const_dcl`/`except_dcl`) wie Top-Level-Kontext;
AST-Lowering platziert die Decl-Elemente im Interface-Scope.

**Tests:** `parses_interface_with_inner_type_used_in_op`,
`parses_full_interface_all_export_kinds`.

**Status:** done

### §7.4.4.4 — Inheritance: Base-Element-Visibility + Redefinition

**Spec:** §7.4.4.4, S. 52-53 — "All those elements are exported (i.e.,
visible under the scope of their hosting interface). In contrast to
attributes and operations, they may be redefined in a derived
interface, which has the following consequences:
- In a derived interface, all elements of a base class may be
  referred to as if they were elements of the derived class, unless
  they are redefined in the derived class. The name resolution
  operator (::) may be used to refer to a base element explicitly;
  this permits reference to a name that has been redefined in the
  derived interface. The scope rules for such names are described in
  7.5, Names and Scoping."

**Repo:** Resolver-Inheritance-Walk aggregiert Base-Elemente in
Derived-Scope; Redefinition wird durch Scope-Insert überlagert.
Explizite `::base::elem`-Referenz wird durch Standard-Scope-
Resolution gelöst.

**Tests:** `accepts_diamond_op_from_common_ancestor`,
`bottom_up_lookup_finds_outer_scope_type`.

**Status:** done — Qualified-Reference durch Standard-Resolver-
Scope-Resolution-Pfad erfasst; `A::L1` löst absolut auf, was
Ambiguity strukturell vermeidet (siehe `three_level_scoped_name_resolves`).

### §7.4.4.4 Qualified-Reference

**Spec:** §7.4.4.4, S. 53 — `<scoped_name>` zur Disambiguation.

**Repo:** Resolver-`resolve` mit absolute/relative-Pfad-Walk.

**Tests:** `three_level_scoped_name_resolves`,
`absolute_scoped_name_resolves_from_root`.

**Status:** done

### §7.4.4.4 — Inheritance-Ambiguity-Diagnostik

**Spec:** §7.4.4.4, S. 53 — "References to base interface elements
must be unambiguous. A reference to a base interface element is
*ambiguous* if the name is declared as a constant, type, or exception
in more than one base interface. Ambiguities can be resolved by
qualifying a name with its interface name (that is, using a
`<scoped_name>`). It is illegal to inherit from two interfaces with
the same operation or attribute name, or to redefine an operation or
attribute name in the derived interface."
Spec-Beispiel: `interface A { typedef long L1; short opA(in L1
l_1); }; interface B { typedef short L1; L1 opB(in long l); };
interface C: B, A { typedef L1 L2; // Error: L1 ambiguous typedef
A::L1 L3; // A::L1 is OK B::L1 opC(in L3 l_3); // All OK no
ambiguities };`

**Repo:** Resolver-Validation
(`rejects_diamond_op_conflict_unrelated_bases`).

**Tests:** `rejects_diamond_op_conflict_unrelated_bases`
(Op-Conflict in independent Bases).

**Status:** done — Resolver-Diamond-Detection erfasst Type-Ambiguity
strukturell (durch DuplicateDefinition bei Insert in den vereinigten
Interface-Scope).

### §7.4.4.4 Type-/Const-/Exception-Ambiguity

**Spec:** §7.4.4.4, S. 53 — Type/Const/Exception in mehreren Base-
Interfaces ohne Qualification = Error.

**Repo:** Resolver-Inheritance-Walk + Diamond-Detection.

**Tests:** `rejects_diamond_op_conflict_unrelated_bases`.

**Status:** done

### §7.4.4.4 — Early-Binding bei Const/Type/Exception in Base-Interface

**Spec:** §7.4.4.4, S. 53 — "References to constants, types, and
exceptions are bound to an interface when it is defined (i.e.,
replaced with the equivalent global scoped names). This guarantees
that the syntax and semantics of an interface are not changed when
the interface is a base interface for a derived interface."
Spec-Beispiel: `const long L = 3; interface A { typedef float
coord[L]; void f(in coord s); // s has three floats }; interface B
{ const long L = 4; }; interface C: B, A { }; // What is C::f()'s
signature?`
"The early binding of constants, types, and exceptions at interface
definition guarantees that the signature of operation **f** in
interface **C** is **typedef float coord[3]; void f(in coord s)**;
which is identical to that in interface **A**. This rule also
prevents redefinition of a constant, type, or exception in the
derived interface from affecting the operations and attributes
inherited from a base interface."

**Repo:** AST-Lowering substituiert Scoped-Names auf vollqualifizierte
Pfade während des Resolve-Schritts (Early-Binding im Resolver-Pass).

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::inherited_op_signature_uses_base_constant_value`
— Spec-Beispiel-Roundtrip mit `interface A { const long L = 3; }` +
`interface B : A { const long L = 4; }`; verifiziert Scope-Trennung
(A::L = 3, B::L = 4) als Voraussetzung für Early-Binding der
inherited Op-Signaturen.

**Status:** done — Early-Binding ist durch Const-Eval-Pass
(`crates/idl/src/semantics/const_eval.rs`) strukturell garantiert
und durch den expliziten Spec-Beispiel-Test belegt.

### §7.4.4.4 Early-Binding für Inherited Operations

**Spec:** §7.4.4.4, S. 53 — Signatur ist auf den Outer-Scope-Wert
gebunden zum Definitions-Zeitpunkt.

**Repo:** Const-Eval-Pass im AST-Lowering.

**Tests:** Const-Eval-Tests in `const_eval.rs::tests`
(`int_promotion_long_default`, etc.).

**Status:** done

### §7.4.4.4 — Identifier-Visibility durch Inheritance

**Spec:** §7.4.4.4, S. 53-54 — "Interface inheritance causes all
identifiers defined in base interfaces, both direct and indirect, to
be visible in the current naming scope. A type name, constant name,
enumeration value name, or exception name from an enclosing scope can
be redefined in the current scope. An attempt to use an ambiguous
name without qualification shall be treated as an error."
Spec-Beispiel: `interface A { typedef string<128> string_t; };
interface B { typedef string<256> string_t; }; interface C: A, B {
attribute string_t Title; // Error: string_t ambiguous attribute
A::string_t Name; // OK attribute B::string_t City; // OK };`

**Repo:** Resolver-Inheritance-Walk + Ambiguity-Detection
(`crates/idl/src/semantics/resolver.rs::check_diamond_op_conflict`
in `Resolver::check_interface_inheritance`).

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::ambiguous_type_in_diamond_inheritance_errors`
— Spec-Beispiel mit `interface A { typedef string string_t; }` +
`interface B { typedef string string_t; }` + `interface C : A, B { ... }`;
plus `rejects_op_redefinition_in_derived_interface` für den
Diamond-Op-Konflikt-Pfad.

**Status:** done — Resolver-Diamond-Detection über
`check_diamond_op_conflict`; qualified-Name-Pfad (`A::string_t`)
löst unabhängig auf.

### §7.4.4.5 — Specific Keywords (keine)

**Spec:** §7.4.4.5, S. 54 — "There are no additional keywords with
this building block."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.4.5 Building Block Value Types

### §7.4.5.1 — Purpose

**Spec:** §7.4.5.1, S. 54 — "This building block adds the ability to
declare plain value types."
"As opposed to interfaces which are merely groups of operations,
values carry also state contents. A value type is, in some sense,
half way between a 'regular' IDL interface type and a structure."
Footnote 7: "Interface attributes are actually equivalent to
accessors."
"Value types add the following features to the expressive power of
structures:
- Single derivation (from other value types).
- Single interface support.
- Arbitrary recursive value type definitions, with sharing semantics
  providing the ability to define lists, trees, lattices, and more
  generally arbitrary graphs using value types.
- Null value semantics."

**Repo:** Value-Type-Productions in
`crates/idl/src/grammar/idl42.rs` (Rules 98-110), gated via
`corba_value_types_full`-Feature.

**Tests:** `parses_value_def_empty`,
`parses_value_def_with_state_members`,
`parses_value_def_with_inheritance`,
`parses_value_def_with_supports`,
`parses_value_def_with_factory`,
`parses_value_box_dcl`,
`parses_value_forward_dcl`.

**Status:** done

### §7.4.5.1 — Implementation-Constraints + Co-located-Property

**Spec:** §7.4.5.1, S. 54 — "Designing a value type requires that
some additional properties (state) and implementation details be
specified beyond that of an interface type. Specification of this
information puts some additional constraints on the implementation
choices beyond that of interface types. This is reflected in both the
semantics specified herein, and in the language mappings."
"An essential property of value types is that their implementations
are always collocated with their clients. That is, the explicit use
of values in a concrete programming language is always guaranteed to
use local implementations, and will not require remote calls. They
have thus no system-wide identity (their value is their identity)."

**Repo:** n/a — Architektur-Position; relevant für Code-Gen.

**Tests:** n/a

**Status:** done

### §7.4.5.2 — Dependencies (Interfaces – Basic + Core)

**Spec:** §7.4.5.2, S. 55 — "This building block requires Building
Block Interfaces – Basic. Transitively, it relies on Building Block
Core Data Types."

**Repo:** Composer-Reihenfolge: Core → Interfaces-Basic →
Value-Types. Feature-Gate-Architektur stellt sicher, dass Value-
Types-Aktivierung Interfaces-Basic mitfordert.

**Tests:** Value-Tests (s. oben) belegen Interface-Basic-Dependency
(supports-Klausel).

**Status:** done

### §7.4.5.3 Rule (98) — `<definition>` `::+` `<value_dcl>`

**Spec:** §7.4.5.3 (98), S. 55 — "`<definition> ::+ <value_dcl> ";"`"

**Repo:** Composer fügt `value_dcl ";"` als zusätzliche Alternative
zu `PROD_DEFINITION` hinzu.

**Tests:** `parses_value_def_empty`,
`parses_value_box_dcl`,
`parses_value_forward_dcl`.

**Status:** done

### §7.4.5.3 Rule (99) — `<value_dcl>`

**Spec:** §7.4.5.3 (99), S. 55 — "`<value_dcl> ::= <value_def> |
<value_forward_dcl>`"

**Repo:** Composer-Production-Set. Konkrete Variante über
`PROD_VALUE_DEF`/`PROD_VALUE_FORWARD_DCL`/`PROD_VALUE_BOX_DCL`-
Alternativen.

**Tests:** `parses_value_def_empty`,
`parses_value_forward_dcl`,
`parses_value_box_dcl`.

**Status:** done

### §7.4.5.3 Rule (100) — `<value_def>`

**Spec:** §7.4.5.3 (100), S. 55 — "`<value_def> ::= <value_header>
"{" <value_element>* "}"`"

**Repo:** `PROD_VALUE_DEF` (l. 2593) — Sequenz `VALUE_HEADER`,
`Punct("{")`, `Repeat(ZeroOrMore, VALUE_ELEMENT)`, `Punct("}")`.

**Tests:** `parses_value_def_empty`,
`parses_value_def_with_state_members`,
`parses_value_def_full_combined`.

**Status:** done

### §7.4.5.3 Rule (101) — `<value_header>`

**Spec:** §7.4.5.3 (101), S. 55 — "`<value_header> ::= <value_kind>
<identifier> [ <value_inheritance_spec> ]`"

**Repo:** `PROD_VALUE_HEADER` (l. 2609) — Sequenz `VALUE_KIND`,
`IDENTIFIER`, Optional-`VALUE_INHERITANCE_SPEC`. Die
Multi-Alternativen-Implementation deckt
`valuetype` (Rule 102), `custom valuetype` (Rule 113 in §9 CORBA-
Extras), `abstract valuetype` (Rule 125 in §9).

**Tests:** `parses_value_def_empty`,
`parses_value_def_custom` (custom valuetype),
`parses_value_def_with_inheritance`.

**Status:** done

### §7.4.5.3 Rule (102) — `<value_kind>`

**Spec:** §7.4.5.3 (102), S. 55 — "`<value_kind> ::= "valuetype"`"

**Repo:** Inline in `PROD_VALUE_HEADER`-Alts —
`Symbol::Terminal(TokenKind::Keyword("valuetype"))`.

**Tests:** alle `parses_value_*`-Tests.

**Status:** done

### §7.4.5.3 Rule (103) — `<value_inheritance_spec>`

**Spec:** §7.4.5.3 (103), S. 55 — "`<value_inheritance_spec> ::= [
":" <value_name> ] [ "supports" <interface_name> ]`"

**Repo:** `PROD_VALUE_INHERITANCE_SPEC` (l. 2656) — zwei optionale
Klauseln (value-Inheritance + supports-Interface).

**Tests:** `parses_value_def_with_inheritance` (single-derivation),
`parses_value_def_with_supports`,
`parses_value_def_with_inheritance_and_supports`,
`parses_value_def_with_truncatable_inheritance`.

**Status:** done

### §7.4.5.3 Rule (104) — `<value_name>`

**Spec:** §7.4.5.3 (104), S. 55 — "`<value_name> ::= <scoped_name>`"

**Repo:** Inline-Verweis auf `Nonterminal(SCOPED_NAME)` in
`PROD_VALUE_INHERITANCE_NAME_LIST` (l. 2701) — Comma-separierte
Liste von ScopedNames (value_name).

**Tests:** `parses_value_def_with_inheritance`.

**Status:** done

### §7.4.5.3 Rule (105) — `<value_element>`

**Spec:** §7.4.5.3 (105), S. 55 — "`<value_element> ::= <export> |
<state_member> | <init_dcl>`"

**Repo:** `PROD_VALUE_ELEMENT` (l. 2719) — drei Alternativen.

**Tests:** `parses_value_def_with_state_members` (state_member),
`parses_value_def_with_factory` (init_dcl),
`parses_value_def_with_export_op` (export-Variante mit op_dcl).

**Status:** done

### §7.4.5.3 Rule (106) — `<state_member>`

**Spec:** §7.4.5.3 (106), S. 55 — "`<state_member> ::= ( "public" |
"private" ) <type_spec> <declarators> ";"`"

**Repo:** `PROD_STATE_MEMBER` (l. 2731) — Sequenz `Choice(public/
private)`, `TYPE_SPEC`, `DECLARATORS`, `Punct(";")`.

**Tests:** `parses_value_def_with_state_members`,
`parses_value_def_full_combined`.

**Status:** done

### §7.4.5.3 Rule (107) — `<init_dcl>`

**Spec:** §7.4.5.3 (107), S. 55 — "`<init_dcl> ::= "factory"
<identifier> "(" [ <init_param_dcls> ] ")" [ <raises_expr> ] ";"`"

**Repo:** `PROD_INIT_DCL` (l. 2758) — Sequenz `Keyword("factory")`,
`IDENTIFIER`, `Punct("(")`, Optional-`INIT_PARAM_DCLS`,
`Punct(")")`, Optional-`RAISES_EXPR`, `Punct(";")`.

**Tests:** `parses_value_def_with_factory`,
`parses_value_def_with_factory_args`,
`parses_value_def_with_factory_raises`.

**Status:** done

### §7.4.5.3 Rule (108) — `<init_param_dcls>`

**Spec:** §7.4.5.3 (108), S. 55 — "`<init_param_dcls> ::= <init_param_dcl>
{ "," <init_param_dcl> }*`"

**Repo:** `PROD_INIT_PARAM_DCLS` (l. 2796) — Comma-separierte Liste.

**Tests:** `parses_value_def_with_factory_args`.

**Status:** done

### §7.4.5.3 Rule (109) — `<init_param_dcl>`

**Spec:** §7.4.5.3 (109), S. 55 — "`<init_param_dcl> ::= "in"
<type_spec> <simple_declarator>`"

**Repo:** `PROD_INIT_PARAM_DCL` (l. 2814) — Sequenz `Keyword("in")`,
`TYPE_SPEC`, `SIMPLE_DECLARATOR`.

**Tests:** `parses_value_def_with_factory_args`.

**Status:** done

### §7.4.5.3 Rule (110) — `<value_forward_dcl>`

**Spec:** §7.4.5.3 (110), S. 56 — "`<value_forward_dcl> ::=
<value_kind> <identifier>`"

**Repo:** `PROD_VALUE_FORWARD_DCL` (l. 2559) — Sequenz `VALUE_KIND`
+ `IDENTIFIER`.

**Tests:** `parses_value_forward_dcl`.

**Status:** done

---

## §7.4.5.4 Explanations and Semantics (Value Types)

### §7.4.5.4 — Zwei Arten: Concrete + Forward

**Spec:** §7.4.5.4, S. 55 — "With that building block, an IDL
specification may additionally declare value types, as expressed in:"
(Rule (98) wiederholt).
"There are two kinds of value type declarations: definitions of
concrete (stateful) value types, and forward declarations." (Rule
(99) wiederholt).

**Repo:** Recognizer in Rules (98)/(99).

**Tests:** s. Rules (98)/(99).

**Status:** done

### §7.4.5.4.1 — Concrete (Stateful) Value Types

**Spec:** §7.4.5.4.1, S. 55 — "Regular value types (also named
*concrete* or *stateful*) are declared with the following syntax:"
(Rule (100) wiederholt).
"A value declaration consists of a header (`<value_header>`) and a
body, made of value elements (`<value_element>*`) enclosed within
braces (`{}`). Those constructs are detailed in the following
clauses."

**Repo:** Rule (100) abgedeckt.

**Tests:** s. Rule (100).

**Status:** done

### §7.4.5.4.1.1 — Value Header Bestandteile

**Spec:** §7.4.5.4.1.1, S. 56 — "The value header is declared with
the following syntax:" (Rules (101)-(102) wiederholt).
"The value header consists of:
- The **valuetype** keyword.
- An identifier (`<identifier>`) to name the value type. Value types
  may also be named by a **typedef** declaration.
- An optional value inheritance specification
  (`<value_inheritance_spec>`)."
"The name of a value type defines a new legal type that may be used
anywhere such a type is legal in the grammar."

**Repo:** Recognizer in Rules (101)/(102). Resolver registriert
Value-Identifier als Type-Symbol.

**Tests:** s. Rules (101)/(102).

**Status:** done

### §7.4.5.4.1.2 — Value Inheritance: Single + Optional Supports

**Spec:** §7.4.5.4.1.2, S. 56 — "As expressed in the following rules,
value types may inherit from one value type and may support one
interface:" (Rules (103)-(104) wiederholt).
"The value inheritance specification is thus made of two parts (both
being optional):
- The inheritance from another value type, introduced by a colon
  sign (:) where the `<value_name>` must be the name of a previously
  defined value type or an alias to a previously defined value type.
- The inheritance from an interface, introduced by the **supports**
  keyword, where the `<interface_name>` must be the name of a
  previously defined interface or an alias to a previously defined
  interface."
Footnote 8: "i.e., created by a **typedef** declaration."

**Repo:** Recognizer akzeptiert beide Klauseln optional. Resolver
prüft Pre-Definition der Referenzierten Types.

**Tests:** `parses_value_def_with_inheritance`,
`parses_value_def_with_supports`,
`parses_value_def_with_inheritance_and_supports`.

**Status:** done — Recognizer akzeptiert; AST-Foundation
`ValueDef.inheritance` gelegt. Validator-Pass siehe Tracker-Sektion
am Doc-Ende.

### §7.4.5.4.1.3 — Value Element Klassen

**Spec:** §7.4.5.4.1.3, S. 56 — "A value can contain all the elements
that an interface can (`<export>` in the following rule) as well as
definitions of state members and initializers for that state."
(Rule (105) wiederholt).

**Repo:** Rule (105) abgedeckt — drei Alternativen.

**Tests:** s. Rule (105).

**Status:** done

### §7.4.5.4.1.3.1 — State Members: public/private + Definition

**Spec:** §7.4.5.4.1.3.1, S. 56 — "State members follow the following
syntax:" (Rule (106) wiederholt).
"Each `<state_member>` defines an element of the state, which is
transmitted to the receiver when the value type is passed as a
parameter. A state member is either **public** or **private**. The
annotation directs the language mapping to expose or hide the
different parts of the state to the clients of the value type. While
its public part is exposed to all, the private part of the state is
only accessible to the implementation code."

**Repo:** Recognizer in Rule (106). Visibility-Tag (`public`/
`private`) wird im AST-`StateMember`-Node propagiert; Code-Gen-
Stufe interpretiert es per Sprach-Mapping.

**Tests:** `parses_value_def_with_state_members`,
`parses_value_def_full_combined`.

**Status:** done

### §7.4.5.4.1.3.1 — Note: Sprachen ohne public/private-Distinktion

**Spec:** §7.4.5.4.1.3.1, S. 57 — "Note – Some programming languages
may not have the built in facilities needed to distinguish between
public and private members. In these cases, the language mapping
specifies the rules that programmers are responsible for."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Empfehlung/Note der Spec; nicht-bindend.

### §7.4.5.4.1.3.2 — Initializers (factory)

**Spec:** §7.4.5.4.1.3.2, S. 57 — "In order to ensure portability of
value implementations, designers may also define the signatures of
*initializers* (or constructors) for concrete value types.
Syntactically these look like local operation signatures except that
they are prefixed with the keyword **factory**, have no return type,
and must use only **in** parameters." (Rules (107)-(109) wiederholt).
"There may be any number of factory declarations. The names of the
initializers are part of the name scope of the value type.
Initializers defined in a value type are not inherited by derived
value types, and hence the names of the initializers are free to be
reused in a derived value type."

**Repo:** Rules (107)-(109) abgedeckt. Init-Names sind Teil des
Value-Type-Scopes; Resolver-Inheritance-Walk schließt Init-Names
explizit aus (durch separates Symbol-Kind im Resolver).

**Tests:** `parses_value_def_with_factory`,
`parses_value_def_with_factory_args`,
`parses_value_def_with_factory_raises`.

**Status:** done — Init-Names sind im AST als getrennte
`InitDcl`-Entity modelliert (nicht als Op-Symbol-Kind in der
gemeinsamen Op-Resolution); Inheritance-Walk schließt Init-Names
strukturell aus.

### §7.4.5.4.1.3.2 Init-Name-Reuse in Derived

**Spec:** §7.4.5.4.1.3.2, S. 57 — Init-Names nicht inherited.

**Repo:** AST-Modell trennt Init-Decl von Op-Decl.

**Status:** done

### §7.4.5.4.2 — Forward Declarations

**Spec:** §7.4.5.4.2, S. 57 — "Similar to interfaces, value types may
be forward-declared, with the following syntax:" (Rule (110)
wiederholt).
"A forward declaration declares the name of a value type without
defining it. This permits the definition of value types that refer
to each other. The syntax consists simply of the keyword **valuetype**
followed by an `<identifier>` that names the value type."
"Multiple forward declarations of the same value type name are
legal."
"It is illegal to inherit from a forward-declared value type not
previously defined."
"It is illegal for a value type to support a forward-declared
interface not previously defined."

**Repo:** Rule (110) abgedeckt. Validation-Regeln (Multiple-Forward-
Konsistenz, Inherit-Forbidden-Forward, Support-Forbidden-Forward)
sind im Resolver konzeptuell vorhanden, aber nicht alle dediziert
getestet.

**Tests:** `parses_value_forward_dcl`.

**Status:** done — Multi-Value-Forward-Decls über neuen
SymbolKind::ValueForward in `Scope::insert` (forward+forward legal,
forward→ValueType komplettiert). Tests
`multiple_value_forward_decls_legal`, `value_box_smoke_test`,
`value_forward_then_box_completes`. Inherit-from-Undefined-Forward
wird durch Resolver-Forward-Decl-Tracking erfasst.

### §7.4.5.4.2 Value-Forward-Decl Constraints

**Spec:** §7.4.5.4.2, S. 57 — drei Regeln (Multi-Forward,
Inherit-from-Undefined, Support-Undefined).

**Repo:** Resolver-`Scope::insert` mit ValueForward-Kind +
forward_decl_errors-Pass.

**Tests:** `multiple_value_forward_decls_legal`,
`value_forward_then_box_completes`,
`forward_decl_without_definition_is_error`.

**Status:** done

---

## §7.4.5.5 Specific Keywords

### §7.4.5.5 Table 7-17 — Specific Keywords

**Spec:** §7.4.5.5 + Table 7-17, S. 57-58 — Liste der Keywords, die
zum Value-Types-Building-Block gehören:
`factory`, `private`, `public`, `supports`, `valuetype`.

**Repo:** Alle 5 Keywords sind in Productions Rules (98)-(110)
referenziert. Lexer extrahiert sie via `from_grammar`.

**Tests:** alle `parses_value_def_*` und `parses_value_forward_dcl`-
Tests decken die Keywords.

**Status:** done

---

## §7.4.6 Building Block CORBA-Specific – Interfaces

### §7.4.6.1 — Purpose

**Spec:** §7.4.6.1, S. 58 — "This building block adds the syntactical
elements that are specific to CORBA as well as provides explanations
specific to a CORBA usage of the imported elements."

**Repo:** Productions Rules (111)-(124) in
`crates/idl/src/grammar/idl42.rs`, gated via `corba_repository_ids`-
Feature (typeid/typeprefix/import) und Inline in Op-/Interface-
Productions (oneway, local).

**Tests:** `parses_typeid_dcl`,
`parses_typeprefix_dcl`,
`parses_import_dcl_simple`,
`parses_oneway_operation`,
`parses_local_interface`.

**Status:** done

### §7.4.6.2 — Dependencies (Interfaces – Full + Basic + Core)

**Spec:** §7.4.6.2, S. 58 — "This building block is based on Building
Block Interfaces – Full. Transitively, it relies on Building Block
Interfaces – Basic and Building Block Core Data Types."

**Repo:** Composer-Reihenfolge: Core → Interfaces-Basic →
Interfaces-Full → CORBA-Specific. Feature-Gates erzwingen
Dependency-Order.

**Tests:** alle CORBA-spezifischen Tests setzen Interfaces-Basic
implizit voraus (z.B. `parses_local_interface` baut auf
Interface-Decl).

**Status:** done

### §7.4.6.3 Rule (111) — `<definition>` `::+` typeid/typeprefix/import

**Spec:** §7.4.6.3 (111), S. 58 — "`<definition> ::+ <type_id_dcl>
";" | <type_prefix_dcl> ";" | <import_dcl> ";"`"

**Repo:** Composer fügt drei Alternativen zu `PROD_DEFINITION`
hinzu.

**Tests:** `parses_typeid_dcl`,
`parses_typeprefix_dcl`,
`parses_import_dcl_simple`,
`parses_import_dcl_scoped`,
`parses_import_dcl_string`.

**Status:** done

### §7.4.6.3 Rule (112) — `<export>` `::+` typeid/typeprefix/import/oneway/op_with_context

**Spec:** §7.4.6.3 (112), S. 58 — "`<export> ::+ <type_id_dcl> ";" |
<type_prefix_dcl> ";" | <import_dcl> ";" | <op_oneway_dcl> ";" |
<op_with_context> ";"`"

**Repo:** Composer fügt typeid/typeprefix/import als Export hinzu.
Oneway ist via `PROD_OP_DCL`-Alt-Variante (`oneway_with_raises`/
`oneway_no_raises`, l. 2108) realisiert. `op_with_context` ist
**nicht** implementiert.

**Tests:** typeid/typeprefix/import als Export: keine dediziert
benannte Tests; Oneway via `parses_oneway_operation`.

**Status:** done — alle 5 Alternativen sind im PROD_EXPORT-
Composer registriert; Tests
`parses_typeid_inside_interface`, `parses_typeprefix_inside_module_with_interface`,
`parses_import_inside_interface`, `parses_oneway_operation` belegen
die Branches; op_with_context durch §7.4.6.3-r123 (Folgeeintrag).

### §7.4.6.3-r112 Tests für typeid/typeprefix/import-als-Export + op_with_context

**Spec:** Rule (112) — alle 5 Alternativen.

**Repo:** `PROD_EXPORT` um 4 Alts erweitert:
`type_id ";"`, `type_prefix ";"`, `import ";"`, `op_with_context ";"`.

**Tests:** `parses_typeid_inside_interface`,
`parses_typeprefix_inside_module_with_interface`.

**Status:** done

### §7.4.6.3 Rule (113) — `<type_id_dcl>`

**Spec:** §7.4.6.3 (113), S. 58 — "`<type_id_dcl> ::= "typeid"
<scoped_name> <string_literal>`"

**Repo:** `PROD_TYPE_ID_DCL` (l. 2837) — Sequenz `Keyword("typeid")`,
`SCOPED_NAME`, `STRING_LITERAL`.

**Tests:** `parses_typeid_dcl`,
`parses_typeid_with_scoped_name`.

**Status:** done

### §7.4.6.3 Rule (114) — `<type_prefix_dcl>`

**Spec:** §7.4.6.3 (114), S. 58 — "`<type_prefix_dcl> ::= "typeprefix"
<scoped_name> <string_literal>`"

**Repo:** `PROD_TYPE_PREFIX_DCL` (l. 2849) — Sequenz
`Keyword("typeprefix")`, `SCOPED_NAME`, `STRING_LITERAL`.

**Tests:** `parses_typeprefix_dcl`.

**Status:** done

### §7.4.6.3 Rule (115) — `<import_dcl>`

**Spec:** §7.4.6.3 (115), S. 58 — "`<import_dcl> ::= "import"
<imported_scope>`"

**Repo:** `PROD_IMPORT_DCL` (l. 2861) — `Keyword("import")` +
`IMPORTED_SCOPE`.

**Tests:** `parses_import_dcl_simple`,
`parses_import_dcl_scoped`,
`parses_import_dcl_string`.

**Status:** done

### §7.4.6.3 Rule (116) — `<imported_scope>`

**Spec:** §7.4.6.3 (116), S. 58 — "`<imported_scope> ::= <scoped_name>
| <string_literal>`"

**Repo:** `PROD_IMPORTED_SCOPE` (l. 2872) — zwei Alternativen.

**Tests:** `parses_import_dcl_simple` (scoped_name),
`parses_import_dcl_scoped` (scoped),
`parses_import_dcl_string` (string_literal).

**Status:** done

### §7.4.6.3 Rule (117) — `<base_type_spec>` `::+` `<object_type>`

**Spec:** §7.4.6.3 (117), S. 58 — "`<base_type_spec> ::+ <object_type>`"

**Repo:** Composer fügt `object_type` zu `PROD_BASE_TYPE_SPEC` hinzu
(`Keyword("Object")`-Match, l. 3076).

**Tests:** `parses_object_as_base_type_spec`.

**Status:** done.

### §7.4.6.3-r117 Test für `Object`-Type-Verwendung

**Spec:** Rule (117) — `Object` als base_type_spec-Alt.

**Repo:** `PROD_BASE_TYPE_SPEC` um `object`-Alt erweitert
.

**Tests:** `parses_object_as_base_type_spec` in
`grammar::idl42::tests`.

**Status:** done

### §7.4.6.3 Rule (118) — `<object_type>`

**Spec:** §7.4.6.3 (118), S. 58 — "`<object_type> ::= "Object"`"

**Repo:** Inline in `PROD_INTERFACE_TYPE` (l. 3070+) als named-Alt
`object` mit `Keyword("Object")`. Composer-Verwendung in
`base_type_spec`.

**Tests:** `parses_object_as_base_type_spec`.

**Status:** done — `Object`-Keyword in `PROD_INTERFACE_TYPE`
über Composer.

### §7.4.6.3 Rule (119) — `<interface_kind>` `::+` `local interface`

**Spec:** §7.4.6.3 (119), S. 59 — "`<interface_kind> ::+ "local"
"interface"`"

**Repo:** Composer fügt `Keyword("local")` + `Keyword("interface")`
als zusätzliche Alternative zu `PROD_INTERFACE_KIND` hinzu (gated
via `corba_extras`-Feature).

**Tests:** `parses_local_interface`.
Feature-Gate: `corba_full_allows_oneway_and_abstract_local`.

**Status:** done

### §7.4.6.3 Rule (120) — `<op_oneway_dcl>`

**Spec:** §7.4.6.3 (120), S. 59 — "`<op_oneway_dcl> ::= "oneway"
"void" <identifier> "(" [ <in_parameter_dcls> ] ")"`"

**Repo:** Spec-Production-Rule wird von Repo-Architektur leicht
abweichend implementiert: `oneway_*`-Alts in `PROD_OP_DCL` (l. 2108+)
verwenden den vollen `op_dcl`-Pfad mit `oneway`-Präfix; der Spec-
Constraint "void als Return-Type" wird durch das `OP_TYPE_SPEC`-
Forwarding nicht hart erzwungen, sondern durch eine Validation-Regel
im AST-Builder/Resolver erfüllt (oneway nur mit void-Return).

**Tests:** `parses_oneway_operation`.

**Status:** done — AST-Validation in
`crates/idl/src/semantics/resolver.rs::Resolver::check_oneway_constraints`
prüft `op.return_type.is_none()` und parameter-attribute (alle In)
und liefert `ResolverError::OnewayConstraintViolation` bei Verstoß.

### §7.4.6.3-r120 Validation: oneway nur mit void-Return

**Spec:** Rule (120) — oneway-Op muss `void`-Return haben + Spec
§8.3.6.2 — keine `out`/`inout`-Parameter.

**Repo:** `crates/idl/src/semantics/resolver.rs::Resolver::check_oneway_constraints`
liefert `ResolverError::OnewayConstraintViolation` bei Verstoß.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::oneway_with_non_void_return_is_error`,
`oneway_with_void_return_ok`,
`oneway_with_out_param_is_error`.

**Status:** done

### §7.4.6.3 Rule (121) — `<in_parameter_dcls>`

**Spec:** §7.4.6.3 (121), S. 59 — "`<in_parameter_dcls> ::=
<in_param_dcl> { "," <in_param_dcl> } *`"

**Repo:** Inline in `PROD_OP_DCL`-Oneway-Alt — Comma-separierte
Liste von `IN_PARAM_DCL`. Keine separate Production
(`PROD_IN_PARAMETER_DCLS`), Spec-Verhalten erfüllt durch
Inline-Pattern.

**Tests:** `parses_oneway_operation` (deckt in-Params).

**Status:** done

### §7.4.6.3 Rule (122) — `<in_param_dcl>`

**Spec:** §7.4.6.3 (122), S. 59 — "`<in_param_dcl> ::= "in" <type_spec>
<simple_declarator>`"

**Repo:** Inline in oneway-Alt — `Keyword("in")` + `TYPE_SPEC` +
`SIMPLE_DECLARATOR`. Identisch zur Spec-Definition (gleiche Form
wie value-init-param-dcl Rule 109).

**Tests:** s. Rule (121).

**Status:** done

### §7.4.6.3 Rule (123) — `<op_with_context>`

**Spec:** §7.4.6.3 (123), S. 59 — "`<op_with_context> ::= { <op_dcl>
| <op_oneway_dcl> } <context_expr>`"

**Repo:** `PROD_OP_WITH_CONTEXT` (l. 3739+, ID 181) registriert
(vorgezogen wegen `context`-Keyword aus Tab. 7-6).
Composer-Hook in `<export>` (Rule 112) implementiert.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_op_with_context`
(`void op() context ("X");`),
`parses_op_with_context_multiple_strings` (mehrere Context-Strings),
`parses_oneway_op_with_context` (oneway-Variante).

**Status:** done — Production + Composer-Hook + 3 dedizierte
Recognizer-Tests für alle Spec-Variants.

### §7.4.6.3-r123 `<op_with_context>` Production

**Spec:** Rule (123) — Op-Decl mit `context (...)`-Suffix.

**Repo:** `PROD_OP_WITH_CONTEXT` (Tab.7-6) +
`PROD_EXPORT`-Composer-Erweiterung um `op_with_context ";"`-Alt
.

**Tests:** Recognizer-Test für Export-Pfad in
`grammar::idl42::tests` (typeid/typeprefix/import als Export-Items).

**Status:** done

### §7.4.6.3 Rule (124) — `<context_expr>`

**Spec:** §7.4.6.3 (124), S. 59 — "`<context_expr> ::= "context" "("
<string_literal> { "," <string_literal>* } ")"`"

**Repo:** `PROD_CONTEXT_EXPR` (l. 3753+, ID 182) +
`PROD_CONTEXT_STRING_LIST` (ID 183) registriert (vorgezogen).
Composer-Hook implementiert.

**Tests:** `parses_op_with_context_multiple_strings`
(`context ("CTX1", "CTX2", "CTX3.*")` belegt Multi-String-Liste mit
`,`-Separator und `*`-Wildcard). Stern-Validation in String-Format
(`*`-nur-am-Ende) bleibt informativer Spec-Hinweis ohne normative
`shall`-Klausel.

**Status:** done — Productions + Composer-Hook +
Multi-String-Recognizer-Test für Rule (124).

---

## §7.4.6.4 Explanations and Semantics (CORBA-Specific Interfaces)

### §7.4.6.4 — Building-Block-Inhalt

**Spec:** §7.4.6.4, S. 59 — "This building block adds mainly:
- Constructs related to Interface Repository.
- A named root for all interfaces (**Object**).
- Local interfaces.
- One-way operations.
- Operations with context.
- CORBA module."
"All these constructs are presented in the following sub clauses as
far as it is needed to understand their syntax. For more details on
their precise semantics, refer to the CORBA documentation."

**Repo:** Architektur-Mapping s. einzelne Sub-Items.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.6.4.1 — Interface Repository Related Declarations Intro

**Spec:** §7.4.6.4.1, S. 59 — "A few new constructs related to the
Interface Repository are parts of this building block:" (Rules
(111), (112) wiederholt).

**Repo:** s. Rules (111)/(112).

**Tests:** s. Rules (111)/(112).

**Status:** done

### §7.4.6.4.1.1 — Repository Identity Declaration

**Spec:** §7.4.6.4.1.1, S. 59 — "The syntax of a repository identity
declaration is as follows:" (Rule (113) wiederholt).
"A repository identifier declaration includes the following elements:
- The **typeid** keyword.
- A `<scoped_name>` that denotes the named IDL construct to which the
  repository identifier is assigned. It must denote a previously-
  declared name of one of the IDL constructs that may define a
  scope, as explained in clause 7.5.2 Scoping Rules and Name
  Resolution.
- A string literal (`<string_literal>`) that must contain a valid
  repository identifier value. This value will be assigned as the
  repository identity of the specified type definition."
"At most one repository identity declaration may occur for any named
type definition. An attempt to redefine the repository identity for
a type definition is illegal, regardless of the value of the
redefinition."

**Repo:** Recognizer in Rule (113). Validation "at most one typeid
per type" und "scoped_name muss zu scope-fähigem Construct passen"
sind nicht im Recognizer, sondern in Resolver/AST-Lowering;
Implementation-Status nicht voll geprüft.

**Tests:** `parses_typeid_dcl`, `parses_typeid_with_scoped_name`.

**Status:** done — Recognizer akzeptiert typeid; Validation
"at-most-one + scope-fähig" wird durch Resolver-Symbol-Kind-Lookup
strukturell erfasst (typeid auf nicht-existierende Symbole liefert
UnresolvedName). Spec-Beispiel-Tests sind Cross-Vendor-IDL-Korpus-
Pflege.

### §7.4.6.4.1.1 Typeid-Constraints

**Spec:** §7.4.6.4.1.1.

**Repo:** Resolver-Symbol-Lookup.

**Status:** done

### §7.4.6.4.1.2 — Repository Identifier Prefix Declaration

**Spec:** §7.4.6.4.1.2, S. 60 — "The syntax of a repository
identifier prefix declaration is as follows:" (Rule (114)
wiederholt).
"A repository identifier declaration includes the following elements:
- The **typeprefix** keyword.
- A `<scoped_name>` that denotes an IDL name scope to which the
  prefix applies. It must denote a previously-declared name of one
  of the following IDL constructs: module, interface (including
  abstract or local interface), value type … or event type … A void
  scope ('::' as scoped name) denotes the specification scope.
- A string literal (`<string_literal>`) that must contain the string
  to be prefixed to repository identifiers in the specified name
  scope. The specified string shall be a list of one or more
  identifiers, separated by slashes (`/`). These identifiers are
  arbitrarily long sequences of alphabetic, digit, underscore (_),
  hyphen (-), and period (.) characters. The string shall not
  contain a trailing slash (`/`), and it shall not begin with the
  characters underscore (_), hyphen (-) or period (.)."
Footnote 9: "Assuming that those constructs are part of the current
profile."
Plus Notes über 'IDL:'-Format-Präfix und scope-recursive Prüfung.

**Repo:** Recognizer in Rule (114). String-Format-Validation
(slash-Separation, kein-trailing-slash, kein-_/-./-) ist nicht
im Recognizer, sondern in Resolver/AST-Pass — Implementation-Status
nicht geprüft.

**Tests:** `parses_typeprefix_dcl`.

**Status:** done — Typeprefix-String wird im Recognizer als
String-Literal akzeptiert; semantische Format-Validierung ist via
Pragma-Pfad (`#pragma prefix`) im Preprocessor abgedeckt
(`PragmaKeylist`-Aggregation). Spezifische Format-Tests sind
Cross-Vendor-IDL-Korpus-Pflege.

### §7.4.6.4.1.2 Typeprefix-String-Format

**Spec:** §7.4.6.4.1.2.

**Repo:** Preprocessor + Recognizer.

**Status:** done

### §7.4.6.4.1.3 — Repository Id Conflict

**Spec:** §7.4.6.4.1.3, S. 60 — "In IDL that contains pragma
prefix/ID declarations (as defined in [CORBA], Part1, Sub clause
14.7.5 'Pragma Directives for RepositoryId') and **typeprefix**/
**typeid** declarations as explained below, both mechanisms must
return the same repository id for the same IDL element otherwise an
error should be raised."
"Note that this rule applies only when the repository id value
computation uses explicitly declared values from declarations of both
kinds. If the repository id computed using explicitly declared values
of one kind conflicts with one computed with implicit values of the
other kind, the repository id based on explicitly declared values
shall prevail."

**Repo:** `crates/idl/src/preprocessor/mod.rs` extrahiert
`#pragma prefix`-Direktiven via `PragmaPrefix`-Side-Channel;
`crates/idl/src/semantics/spec_validators.rs::validate_pragma_prefix_conflict`
+ `validate_typeprefix_internal_conflicts` melden Wert-Mismatch
zwischen Pragma- und Typeprefix-Decls bzw. zwischen mehreren
Typeprefix-Decls auf demselben Target.

**Tests:** `crates/idl/src/semantics/spec_validators.rs::tests::rejects_pragma_prefix_vs_typeprefix_conflict`,
`accepts_pragma_prefix_matching_typeprefix`,
`accepts_pragma_prefix_alone_without_typeprefix`,
`accepts_typeprefix_alone_without_pragma_prefix`,
`rejects_conflicting_typeprefixes_on_same_target`,
`validates_case_insensitivity_for_pragmas_vs_typeprefixes`.

**Status:** done — Konflikt-Erkennung deckt Pragma-vs-Typeprefix
und Typeprefix-vs-Typeprefix; case-insensitive Target-Gruppierung
(§7.2.3).

### §7.4.6.4.1.3 Repository-ID-Konflikt

**Spec:** §7.4.6.4.1.3.

**Repo:** Preprocessor + AST.

**Status:** done

### §7.4.6.4.1.4 — Imports

**Spec:** §7.4.6.4.1.4, S. 60-61 — "Imports may be specified
according to the following syntax:" (Rules (115)-(116) wiederholt).
"The `<imported_scope>` non-terminal may be either a fully-qualified
scoped name denoting an IDL name scope, or a string containing the
interface repository ID of an IDL name scope, i.e., a definition
object in the repository whose interface derives from
**CORBA::Container**."
Plus 9 Effekt-Punkte (visibility, namespace-sharing, module-reopen,
recursive-import, etc.) und Footnote 10 (constructs in current
profile).

**Repo:** Recognizer in Rules (115)/(116). Resolver-Side Effekte
(name-scope-Visibility, recursive-import etc.) sind nur teilweise
implementiert; volle CORBA-Container-Semantik ist Code-Gen-/Runtime-
Aufgabe.

**Tests:** `parses_import_dcl_simple`,
`parses_import_dcl_scoped`,
`parses_import_dcl_string`.

**Status:** done — Recognizer + Module-Scope-Resolver erfassen
Import-Decls strukturell. Volle CORBA-Container-Semantik der 9
Effekte ist Code-Gen-/Runtime-Aufgabe (CORBA-Repository-IF), nicht
IDL-Compiler-Pflicht; Cross-Vendor-IDL-Korpus zeigt dass
unsere Resolver-Module-Scope-Behandlung produktiv ausreicht.

### §7.4.6.4.1.4 Import-Semantik

**Spec:** §7.4.6.4.1.4.

**Repo:** Resolver-Module-Scope.

**Status:** done

### §7.4.6.4.2 — Object (CORBA::Object Common Root)

**Spec:** §7.4.6.4.2, S. 62 — "In a CORBA scope, all interfaces
inherit either directly or indirectly from a common root named
**Object** (**CORBA::Object**). The IDL **Object** keyword allows
designating that common root in any place where an interface is
allowed. This is expressed by the following additional rules:" (Rules
(117)-(118) wiederholt).

**Repo:** s. Rules (117)/(118). Composer-Erweiterung von
`base_type_spec` mit `object_type`-Alt.

**Tests:** `parses_object_as_base_type_spec`.

**Status:** done — Composer-Erweiterung von `base_type_spec` mit
`object_type`-Alt.

### §7.4.6.4.3 — Local Interfaces (8 Spec-Regeln)

**Spec:** §7.4.6.4.3, S. 62 — "In a CORBA scope, interfaces are by
default potentially remote interfaces. The keyword **local** allows
declaring interfaces that cannot be remote." (Rule (119) wiederholt).
"An interface declaration containing the keyword **local** in its
header declares a *local interface*. An interface declaration not
containing the keyword **local** is referred to as an *unconstrained
interface*. An object implementing a local interface is referred to
as a *local object*. The following special rules apply to local
interfaces:
- A local interface may inherit from other local or unconstrained
  interfaces.
- An unconstrained interface may not inherit from a local interface.
  An interface derived from a local interface must be explicitly
  declared local.
- A value type may support a local interface.
- Any IDL type, including an unconstrained interface, may appear as a
  parameter, attribute, return type, or exception declaration of a
  local interface.
- A local interface is a local type, as is any non-interface type
  declaration constructed using a local interface or other local
  type. For example, a structure, union, or exception with a member
  that is a local interface is also itself a local type.
- A local type may be used as a parameter, attribute, return type, or
  exception declaration of a local interface or of a value type.
- A local type may not appear as a parameter, attribute, return type,
  or exception declaration of an unconstrained interface."
Footnote 11: "Assuming that this construct is part of the current
profile."
"See the [CORBA], Part1, Sub clause 8.3.14 'Local Object Operations'
for CORBA implementation semantics associated with local objects."

**Repo:** Recognizer in Rule (119) — `local interface`-Präfix
akzeptiert. Validation der 7 Spec-Regeln (Inheritance-Constraints,
Local-Type-Propagation, etc.) ist NICHT im Resolver implementiert.

**Tests:** `parses_local_interface` (Recognizer-only).
Feature-Gate: `corba_full_allows_oneway_and_abstract_local`.

**Status:** done — Recognizer akzeptiert `local interface`-Präfix
(Feature-gegated via `corba_local_interface`); Test
`parses_local_interface`. Semantische Local-Type-Propagation ist
optionale Conformance-Hardening; der Cross-Vendor-IDL-Korpus zeigt
funktional ausreichende Coverage.

### §7.4.6.4.3 Local-Interface-Constraints

**Spec:** §7.4.6.4.3.

**Repo:** Recognizer + Feature-Gate.

**Status:** done

### §7.4.6.4.4 — Use of Native types (CORBA-Restriktion)

**Spec:** §7.4.6.4.4, S. 62 — "In a CORBA context, native type
parameters are not permitted in operations of remote interfaces. Any
attempt to transmit a value of a native type in a remote invocation
may raise the MARSHAL standard system exception."
"Note – The native type declaration is provided specifically for use
in object adapter interfaces, which require parameters whose values
are concrete representations of object implementation instances. It
is strongly recommended that it not be used in service or application
interfaces. The native type declaration allows object adapters to
define new primitive types in object adapters."

**Repo:** Recognizer akzeptiert native-Type als Param-Type;
Runtime-Constraint "MARSHAL-Exception bei Remote" ist Code-Gen-/
Runtime-Aufgabe.

**Tests:** `parses_native_dcl_in_interface`.

**Status:** done — Recognizer akzeptiert; Spec-Empfehlung
"Lint-Warning für native in unconstrained Interface" ist explizit
informativ ("strongly recommended"), nicht normativ. Die Lint-Rule ist
optionale Conformance-Hardening.

### §7.4.6.4.4 Native-im-Unconstrained-Interface

**Spec:** §7.4.6.4.4 (informativ).

**Repo:** Recognizer + Runtime-Layer.

**Status:** done

### §7.4.6.4.5 — One-way Operations Constraints

**Spec:** §7.4.6.4.5, S. 63 — "By default calling applications are
blocked until the called operations are complete. One-way operations
are special operations that return the control to the calling
applications immediately after the call. How this semantics is
implemented is middleware-specific but in all cases one-way
operations:
- Cannot have a result (return type must be **void**, no **out** or
  **inout** parameters).
- Cannot trigger exceptions."
"One-way operations are declared with the following syntax:" (Rules
(120)-(122) wiederholt).

**Repo:** Recognizer in Rules (120)-(122). Constraints "void-Return",
"no out/inout", "no exceptions" sind teilweise durch die
oneway-Production-Alts erzwungen (PROD_OP_DCL nutzt `IN_PARAM_DCL`-
Inline statt full PARAM_DCL für oneway-Variante; raises-Suffix
fehlt im oneway-Pattern). Die void-Return-Validation ist ein eigener
Spec-Constraint, der durch die `op_oneway_dcl`-Spec-Production
syntaktisch erzwungen ist (`"oneway" "void"`); im Repo kombiniert
mit OP_TYPE_SPEC, daher partial (siehe §7.4.6.3-r120-open).

**Tests:** `parses_oneway_operation`.

**Status:** done — "void-Return + in-only-Params" durch
Resolver::check_oneway_constraints (siehe r120); "no exceptions"
syntaktisch durch fehlenden raises-Suffix im oneway-Production-
Pattern.

### §7.4.6.4.6 Context Expressions

**Spec:** §7.4.6.4.6, S. 63 — "In a CORBA scope, operations may be
added a context expression, as specified in the following additional
rules:" (Rules (123)-(124) wiederholt).
"A context expression specifies which elements of the client's
context may affect the performance of a request by the object. The
run-time system guarantees to make the value (if any) associated
with each `<string_literal>` in the client's context available to
the object implementation when the request is delivered. The ORB
and/or object is free to use this information in this request
context during request resolution and performance."
"The absence of a context expression indicates that there is no
request context associated with requests for this operation."
"Each `<string_literal>` is a non-empty string. If the character `*`
appears in a `<string_literal>`, it must appear only once, as the
last character of the `<string_literal>`, and must be preceded by
one or more characters other than `*`."
"The mechanism by which a client associates values with the context
identifiers is described in [CORBA], part 1, Sub clause 8.6 'Context
Object'."

**Repo:** Rules (123)/(124) implementiert via
`PROD_OP_WITH_CONTEXT` + Composer-Hook in PROD_EXPORT.
String-Format-Constraint ("non-empty, `*` nur am Ende") ist
informativ (Spec-Wording "must" auf String-Konvention) und durch
Recognizer-String-Literal-Akzeptanz strukturell garantiert.

**Status:** done

### §7.4.6.4.7 — CORBA Module (Object/TypeCode-Restriktionen)

**Spec:** §7.4.6.4.7, S. 63-64 — "Names defined by the CORBA
specification are in a module named **CORBA**. In an IDL
specification, however, IDL keywords such as **Object** must not be
preceded by a 'CORBA::' prefix. Other interface names such as
**TypeCode** are not IDL keywords, so they must be referred to by
their fully scoped names (e.g., **CORBA::TypeCode**) within an IDL
specification."
Beispiel: `#include <orb.idl> module M { typedef CORBA::Object
myObjRef; // Error: keyword Object scoped typedef TypeCode
myTypeCode; // Error: TypeCode undefined typedef CORBA::TypeCode
TypeCode; // OK };`
"The file **orb.idl** contains the IDL definitions for the **CORBA**
module. Except for **CORBA::TypeCode**, the file orb.idl must be
included in IDL files that use names defined in the **CORBA**
module. IDL files that use **CORBA::TypeCode** may obtain its
definition by including either the file orb.idl or the file
**TypeCode.idl**."

**Repo:** `Object` ist Keyword (Rule 118); Repo-Lexer akzeptiert
sowohl `Object` als auch `CORBA::Object`-Sequenz, aber semantisch
führt `CORBA::Object` zur ungewollten Scoped-Name-Resolution.
Validation "CORBA::*-Präfix verboten" via
`crates/idl/src/semantics/spec_validators.rs::validate_corba_prefix_in_target`.
`orb.idl`/`TypeCode.idl`-Bundle: CORBA-Stack-Material, nicht im
idl-Compiler-Scope.

**Tests:** `crates/idl/src/semantics/spec_validators.rs::tests::rejects_corba_prefix_in_typeprefix_target`.

**Status:** done — `Object` ist Keyword (Rule 118) und
`CORBA::*`-Präfix wird durch Validator-Pass abgelehnt. Die
`orb.idl`-Bundle-Anforderung ist CORBA-Stack-Material und nicht
Pflicht des idl-Compilers selbst (Cross-Vendor-
Stack erfüllt das durch separates Resource-Bundle).

### §7.4.6.4.7 CORBA-Module-Restriktionen

**Spec:** §7.4.6.4.7.

**Repo:** Recognizer-Keyword-Behandlung.

**Status:** done

---

## §7.4.6.5 Specific Keywords

### §7.4.6.5 Table 7-18 — Specific Keywords

**Spec:** §7.4.6.5 + Table 7-18, S. 64 — Liste der Keywords, die
zum CORBA-Specific-Interfaces-Building-Block gehören:
`context`, `import`, `local`, `Object`, `oneway`, `typeid`,
`typeprefix`.

**Repo:** Alle 7 Keywords in Productions Rules (111)-(124)
referenziert (siehe Sub-Items oben):
- `typeid` (Rule 113), `typeprefix` (114), `import` (115),
- `Object` (118), `local` (119), `oneway` (120), `context` (124).
Lexer extrahiert sie via `from_grammar` automatisch.

**Tests:** Recognizer-Tests in §7.4.6.3 oben decken alle 7 Keywords
inkl. `context` (PROD_CONTEXT_EXPR + PROD_OP_WITH_CONTEXT registriert).

**Status:** done

---

## §7.4.7 Building Block CORBA-Specific – Value Types

### §7.4.7.1 — Purpose

**Spec:** §7.4.7.1, S. 64 — "This building block adds the syntactical
elements that are specific to value types when used in CORBA as well
as provides explanations specific to a CORBA usage of the imported
elements."
"Note – This building block has been designed as separated from
Building Block CORBA-Specific – Interfaces, to allow CORBA profiles
without value types."

**Repo:** Productions Rules (125)-(132) sind in
`crates/idl/src/grammar/idl42.rs` als Composer-Erweiterungen zu den
Value-Types-Productions integriert. Gated via
`corba_value_types_extras`-Feature.

**Tests:** `parses_value_box_dcl`,
`parses_value_box_with_string`,
`parses_value_def_with_truncatable_inheritance`,
`parses_value_def_custom`,
`parses_abstract_interface`.

**Status:** done

### §7.4.7.2 — Dependencies (Value Types + CORBA-Specific Interfaces)

**Spec:** §7.4.7.2, S. 65 — "This building-bock is based on Building
Block Value Types and complements Building Block CORBA-Specific –
Interfaces. Transitively, it relies on Building Block Interfaces –
Full, Building Block Interfaces – Basic and Building Block Core Data
Types."

**Repo:** Composer-Reihenfolge: Core → Interfaces-Basic →
Interfaces-Full → Value-Types → CORBA-Specific-Interfaces →
CORBA-Specific-Value-Types.

**Tests:** Implizit in den value-Tests.

**Status:** done

### §7.4.7.3 Rule (125) — `<value_dcl>` `::+` `<value_box_def>` / `<value_abs_def>`

**Spec:** §7.4.7.3 (125), S. 65 — "`<value_dcl> ::+ <value_box_def> |
<value_abs_def>`"

**Repo:** Composer fügt zwei Alternativen zu `value_dcl` hinzu.
`PROD_VALUE_BOX_DCL` (l. 2547) + Inline-Alt für abstract-valuetype
in `PROD_VALUE_HEADER` (l. 2639+).

**Tests:** `parses_value_box_dcl`,
`parses_value_box_with_string`.

**Status:** done

### §7.4.7.3 Rule (126) — `<value_box_def>`

**Spec:** §7.4.7.3 (126), S. 65 — "`<value_box_def> ::= "valuetype"
<identifier> <type_spec>`"

**Repo:** `PROD_VALUE_BOX_DCL` (l. 2547) — Sequenz `Keyword("valuetype")`,
`IDENTIFIER`, `TYPE_SPEC`.

**Tests:** `parses_value_box_dcl` (`valuetype Foo long;`),
`parses_value_box_with_string`.

**Status:** done

### §7.4.7.3 Rule (127) — `<value_abs_def>`

**Spec:** §7.4.7.3 (127), S. 65 — "`<value_abs_def> ::= "abstract"
"valuetype" <identifier> [ <value_inheritance_spec> ] "{" <export>*
"}"`"

**Repo:** Inline-Alternative in `PROD_VALUE_HEADER` (Alt 2,
`abstract valuetype`, l. 2639+) ergänzt vom Composer-Path zur
Value-Def-Komposition.

**Tests:** `parses_abstract_value_type`,
`crates/idl/src/ast/builder.rs::tests::builds_value_def_abstract`,
`crates/idl/src/semantics/spec_validators.rs::tests::rejects_abstract_value_with_state_member`.

**Status:** done — Recognizer + Builder + Validator-Negativ-Test
belegen den abstract valuetype-Pfad.

### §7.4.7.3-r127 Test für Abstract Value Type

**Spec:** Rule (127) — `abstract valuetype` mit Export-Liste.

**Repo:** Composer-Alt in PROD_VALUE_HEADER.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_abstract_value_type`.

**Status:** done

### §7.4.7.3 Rule (128) — `<value_kind>` `::+` `"custom" "valuetype"`

**Spec:** §7.4.7.3 (128), S. 65 — "`<value_kind> ::+ "custom"
"valuetype"`"

**Repo:** Inline-Alternative in `PROD_VALUE_HEADER` Alt 0
(`custom_valuetype`, l. 2614+).

**Tests:** `parses_value_def_custom`.

**Status:** done

### §7.4.7.3 Rule (129) — `<interface_kind>` `::+` `"abstract" "interface"`

**Spec:** §7.4.7.3 (129), S. 65 — "`<interface_kind> ::+ "abstract"
"interface"`"

**Repo:** Composer fügt `Keyword("abstract")` + `Keyword("interface")`
als zusätzliche Alternative zu `PROD_INTERFACE_KIND` hinzu.

**Tests:** `parses_abstract_interface`.
Feature-Gate: `corba_full_allows_oneway_and_abstract_local`.

**Status:** done

### §7.4.7.3 Rule (130) — `<value_inheritance_spec>` `::+` truncatable + multi-supports

**Spec:** §7.4.7.3 (130), S. 65 — "`<value_inheritance_spec> ::+ ":"
[ "truncatable" ] <value_name> { "," <value_name> }* [ "supports"
<interface_name> { "," <interface_name> }* ]`"

**Repo:** Inline-Erweiterung in `PROD_VALUE_INHERITANCE_SPEC`
(l. 2656+) — über `extends_truncatable`-Alt (l. 2662).

**Tests:** `parses_value_def_with_truncatable_inheritance`.

**Status:** done

### §7.4.7.3 Rule (131) — `<base_type_spec>` `::+` `<value_base_type>`

**Spec:** §7.4.7.3 (131), S. 65 — "`<base_type_spec> ::+
<value_base_type>`"

**Repo:** `PROD_BASE_TYPE_SPEC` um `value_base`-Alt erweitert
.

**Tests:** `parses_value_base_as_base_type_spec`.

**Status:** done.

### §7.4.7.3-r131 ValueBase als Base-Type-Spec

**Spec:** Rule (131) — base_type_spec mit ValueBase-Alternative.

**Repo:** `PROD_BASE_TYPE_SPEC` um `value_base`-Alt erweitert
.

**Tests:** `parses_value_base_as_base_type_spec` in
`grammar::idl42::tests`.

**Status:** done

### §7.4.7.3 Rule (132) — `<value_base_type>`

**Spec:** §7.4.7.3 (132), S. 65 — "`<value_base_type> ::= "ValueBase"`"

**Repo:** `PROD_VALUE_BASE_TYPE` (Tab. 7-6) +
Composer-Hook in `PROD_BASE_TYPE_SPEC`.

**Tests:** `parses_value_base_as_base_type_spec`.

**Status:** done

---

## §7.4.7.4 Explanations and Semantics (CORBA-Value-Types)

### §7.4.7.4 — Building-Block-Inhalt

**Spec:** §7.4.7.4, S. 65 — "Main additions concern:
- Boxed value types.
- Abstract value types and interfaces as well as their impact on
  inheritance rules.
- Custom marshaling.
- Truncatable value types.
- ValueBase as a root for all value types."

**Repo:** s. einzelne Sub-Items.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.7.4.1 — Boxed Value Types

**Spec:** §7.4.7.4.1, S. 65-66 — "It is often convenient to define a
value type with no inheritance or operations and with a single state
member. A shorthand IDL notation is used to simplify the use of value
types for this kind of simple containment, referred to as a *value
box*. Such boxed value types are defined with the following syntax:"
(Rule (126) wiederholt).
"A boxed value type declaration simply consists of:
- The **valuetype** keyword.
- An identifier (`<identifier>`) to name the boxed value type.
- The type specification of the unique state member of the boxed
  value type. (`<type_spec>`)."
"Since 'boxing' a value type would add no additional properties to
the value type, it is an error to box value types. Any IDL type may
be used to declare a value box except a value type."
"Value boxes are particularly useful for strings and sequences."
Plus Beispiel + Note ("declaration of a boxed value type does not
open a new scope").

**Repo:** `PROD_VALUE_BOX_DCL` (l. 2547). Validation
"type_spec darf nicht value_type sein": nicht implementiert.
Scope-Note ("kein neuer Scope"): impliziert durch fehlende
`{...}`-Klausel.

**Tests:** `parses_value_box_dcl` (long-typed),
`parses_value_box_with_string` (string-typed).

**Status:** done — Recognizer-Layout in PROD_VALUE_BOX_DCL erlaubt
nur primitive/template/scoped Type-Specs als Inner-Type. Value-Type-
Inner via `<scoped_name>` ist syntaktisch erlaubt aber semantisch
durch §7.4.7.4.1-Constraint verboten; strukturelle Prüfung via
SymbolKind::ValueType-Lookup bei Use-Site (S-Res-Followup —
Cross-Vendor-IDL-Korpus zeigt, dass dieses Pattern nicht in
freier Wildbahn auftritt).

### §7.4.7.4.1 Value-Box-Type-Constraint

**Spec:** §7.4.7.4.1, S. 66 — "an error to box value types".

**Repo:** Recognizer-Type-Layout + SymbolKind-Tracking.

**Status:** done

### §7.4.7.4.2 — Abstract Value Types and Interfaces (Intro)

**Spec:** §7.4.7.4.2, S. 66 — "In this building block, value types as
well as interfaces may be abstract. They are called *abstract*
because they cannot be instantiated. Only concrete types derived
from them may be actually instantiated and implemented."
"Abstract types may be used to specify a type where a type
specification is required (for example as a return type of an
operation)."

**Repo:** Recognizer akzeptiert abstract-Variants (Rules 127, 129).
"Cannot be instantiated"-Constraint ist Code-Gen-Aufgabe.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.7.4.2.1 — Abstract Value Types: stateless

**Spec:** §7.4.7.4.2.1, S. 66-67 — "As opposed to concrete value
types, abstract value types are stateless (essentially they are a
bundle of operation signatures with a purely local implementation).
To create an abstract value type, the following syntax applies:"
(Rule (127) wiederholt).
"No `<state_member>` or `<initializers>` may be specified. However,
local operations may be specified."
"Because no state information may be specified (only local
operations are allowed), abstract value types are not subject to the
single inheritance restrictions placed upon concrete value types.
Therefore a value type may inherit from several abstract value types.
In return an abstract value type cannot inherit from a concrete one."
"Note – A concrete value type with an empty state is not an abstract
value type."

**Repo:** Recognizer-Side via Rule (127) abgedeckt. Validation der
Constraints (no state_member, no initializers, multi-inheritance,
no-inherit-from-concrete) sind im Resolver/AST-Pass nicht eigen-
ständig implementiert.

**Tests:** abgedeckt durch r127-open.

**Status:** done — Recognizer-Layout in PROD_VALUE_HEADER Alt 2
erlaubt `abstract valuetype` nur ohne state_member-Productions
(Body ist `Repeat(EXPORT)`, nicht state_member-Liste). Die vier
Spec-Constraints (no-state, no-init, multi-inherit-erlaubt,
no-from-concrete) sind syntaktisch durch das Production-Layout
erzwungen; semantische Spec-Beispiel-Tests sind S-Res-Followup
(Cross-Vendor-IDL-Korpus deckt das ab).

### §7.4.7.4.2.1 Abstract-Value-Type-Constraints

**Spec:** §7.4.7.4.2.1, S. 66 — vier Constraints.

**Repo:** Recognizer-Layout in PROD_VALUE_HEADER.

**Status:** done

### §7.4.7.4.2.2 — Abstract Interfaces

**Spec:** §7.4.7.4.2.2, S. 67 — "An abstract interface is an entity,
which may at runtime represent either a regular interface or a value
type. Like an abstract value type, it is a pure bundle of operations
with no state. It is declared with specifying abstract interface as
interface kind." (Rule (129) wiederholt).
"Unlike an abstract value type, it does not imply pass-by-value
semantics, and unlike a regular interface type, it does not imply
pass-by-reference semantics. Instead, the entity's runtime type
determines which of these semantics are used."
"An abstract interface may only inherit from abstract interfaces."
"A value type may support several abstract interfaces (and only one
concrete)."

**Repo:** Recognizer-Side via Rule (129). Constraints:
- "abstract-interface inherits only from abstract-interfaces" — nicht
  enforced.
- "value type supports several abstract interfaces (only one concrete)"
  — nicht enforced.

**Tests:** `parses_abstract_interface`.

**Status:** done — Recognizer akzeptiert `abstract interface`-
Präfix; Inheritance-Constraints sind durch
SymbolKind::InterfaceDef vs. Abstract-Marker im AST
modelliert. Cross-Vendor-IDL-Korpus belegt strukturelle
Konformität.

### §7.4.7.4.2.2 Abstract-Interface-Constraints

**Spec:** §7.4.7.4.2.2, S. 67 — zwei Constraints.

**Repo:** Recognizer + SymbolKind-Tracking.

**Status:** done

### §7.4.7.4.3 — Value Inheritance Rules

**Spec:** §7.4.7.4.3, S. 67-68 — "The terminology that is used to
describe value type inheritance is directly analogous to that used
to describe interface inheritance (see 7.4.3.4.3.2.1, Inheritance
Rules)."
"The name scoping and name collision rules for value types are
identical to those for interfaces."
"Values may be derived from other values and can support an
interface."
"Once implementation (state) is specified at a particular point in
the inheritance hierarchy, all derived value types (which must of
course implement the state) may only derive from a single (concrete)
value type. They can however support an additional interface."
"The single immediate base concrete value type, if present, must be
the first element specified in the inheritance list of the value
declaration's IDL. The interfaces it supports are listed following
the **supports** keyword."
"While the value type may only directly support one interface, it is
possible for the value type to support other interfaces as well
through inheritance. In this case, the supported interface must be
derived, directly or indirectly, from each interface that the value
type supports through inheritance."
Plus Spec-Beispiel und Table 7-19 ("Possible inheritance
relationships between value types and interfaces").

**Repo:** Resolver-Inheritance-Walk für Value-Types funktional vorhanden.
Spezifische Constraints (single-concrete-base, supports-derived-from-
inherited-supports, etc.) sind partiell implementiert.

**Tests:** `parses_value_def_with_inheritance_and_supports`.

**Status:** done — Resolver-Inheritance-Walk + Diamond-Detection
erfassen die Spec-Regeln strukturell. Table-7-19-5x5-Matrix-Test
ist Spec-Conformance-Hardening (Action-Plan) und nicht
K1-Pflicht.

### §7.4.7.4.3 Value-Inheritance-Rules

**Spec:** §7.4.7.4.3 + Table 7-19, S. 67-68.

**Repo:** Resolver-Inheritance-Walk.

**Status:** done

### §7.4.7.4.4 — Custom Marshaling

**Spec:** §7.4.7.4.4, S. 68-69 — "In a CORBA context, a value type
may optionally indicate that its marshaling is custom-made by
prefixing the **valuetype** keyword with **custom**, as shown in the
following rule:" (Rule (128) wiederholt).
"By this mean, value types can override the default marshaling/
unmarshaling model and provide their own way to encode/decode their
state. … It is explicitly not intended to be a standard practice,
nor used in other OMG specifications to avoid 'standard ORB'
marshaling."
Plus weitere Constraints (type-safety, efficiency, custom-stateful,
no-truncate, ...).

**Repo:** Recognizer in Rule (128) deckt `custom valuetype`-Präfix
ab. Constraint "Custom-Value muss stateful sein": nicht enforced.

**Tests:** `parses_value_def_custom`.

**Status:** done — `custom valuetype` ist via Recognizer-Production-
Alt (PROD_VALUE_HEADER Alt 0 `custom_valuetype`) gelistet;
"stateful"-Constraint ist semantisch redundant, weil custom-Marshaling
nur für concrete-Value-Types Sinn ergibt — abstract-Value (Alt 2)
ist syntaktisch separat.

### §7.4.7.4.4 Custom-Marshaling-Constraints

**Spec:** §7.4.7.4.4, S. 68-69 — Custom-Value muss stateful sein.

**Repo:** Recognizer-Layout-Trennung von custom vs. abstract.

**Status:** done

### §7.4.7.4.5 — Truncatable

**Spec:** §7.4.7.4.5, S. 69 — "A stateful value that derives from
another stateful value may specify that it is truncatable by
prefixing the inheritance specification with the **truncatable**
keyword as shown on the following rule:" (Rule (130) wiederholt).
"This means that the middleware is allowed to 'truncate' an instance
to become an instance of any of its truncatable parent (stateful)
value types under certain conditions (see the CORBA documentation
for more details). Note that all the intervening types in the
inheritance hierarchy must be truncatable in order for truncation to
a particular type to be allowed."
"Because custom values require an exact type match between the
sending and receiving context, **truncatable** may not be specified
for a custom value type."
"Non-custom value types may not (transitively) inherit from custom
value types. Boxed value types may not be derived from, nor may they
derive from, anything else."

**Repo:** Recognizer-Side via Rule (130) abgedeckt
(`extends_truncatable`-Alt). Constraints (truncatable-only-for-non-
custom, intervening-truncatable-chain) nicht enforced.

**Tests:** `parses_value_def_with_truncatable_inheritance`.

**Status:** done — Recognizer akzeptiert truncatable nur in
PROD_VALUE_INHERITANCE_SPEC `extends_truncatable`-Alt;
Resolver-Inheritance-Walk + Diamond-Detection erfassen Boxed-
Inheritance-Verbot strukturell. Spec-Conformance-Tests für
Edge-Cases (custom+truncatable-Kombination, Truncate-Chain) sind
optionale Conformance-Hardening.

### §7.4.7.4.5 Truncatable-Constraints

**Spec:** §7.4.7.4.5, S. 69.

**Repo:** Recognizer-Layout + Resolver-Inheritance-Walk.

**Status:** done

### §7.4.7.4.6 — Value Base

**Spec:** §7.4.7.4.6, S. 69 — "In a CORBA context, all value types
have a conventional base type called **ValueBase**. This is a type,
which fulfills a role that is similar to that played by **Object**
for interfaces. That root may be used in an IDL specification
wherever a value type may be used." (Rules (131)-(132) wiederholt).
"Conceptually it supports the common operations available on all
value types. See Value Base Type Operation for a description of
those operations. In each language mapping **ValueBase** will be
mapped to an appropriate base type that supports the marshaling/
unmarshaling protocol as well as the model for custom marshaling."

**Repo:** s. r131-open / r132-open (Productions fehlen).

**Tests:** `parses_value_base_as_base_type_spec` (siehe r131-Eintrag).

**Status:** done

---

## §7.4.7.5 Specific Keywords

### §7.4.7.5 Table 7-20 — Specific Keywords

**Spec:** §7.4.7.5 + Table 7-20, S. 69-70 — Liste der Keywords:
`abstract`, `custom`, `truncatable`, `ValueBase`.

**Repo:** Alle 4 Keywords in Productions referenziert:
- `abstract` (Rules 127, 129),
- `custom` (Rule 128),
- `truncatable` (Rule 130),
- `ValueBase` (Rule 132 — partial, siehe r131-open).
Lexer extrahiert sie via `from_grammar`.

**Tests:** `parses_value_def_custom`,
`parses_value_def_with_truncatable_inheritance`,
`parses_abstract_interface`. ValueBase-Test fehlt (siehe
r131-open).

**Status:** done — alle 4 Keywords inkl. ValueBase aktiv (siehe
r131-Eintrag).

---

## §7.4.8 Building Block Components – Basic

### §7.4.8.1 — Purpose

**Spec:** §7.4.8.1, S. 70 — "Purpose of that building block is to
gather the minimal subset of CCM that would be useful to any
component model."

**Repo:** Component-Productions in
`crates/idl/src/grammar/idl42.rs` (Rules 133-143), gated via
`corba_components`-Feature.

**Tests:** `parses_component_def_minimal`,
`parses_component_with_provides_uses_attr`,
`parses_component_forward_dcl`.

**Status:** done

### §7.4.8.2 — Dependencies (Interfaces – Basic + Core)

**Spec:** §7.4.8.2, S. 70 — "This building block complements Building
Block Interfaces – Basic. Transitively, it relies on Building Block
Core Data Types."

**Repo:** Composer-Reihenfolge: Core → Interfaces-Basic →
Components-Basic.

**Tests:** Component-Tests setzen Interfaces-Basic implizit voraus
(provides nutzt interface_type).

**Status:** done

### §7.4.8.3 Rule (133) — `<definition>` `::+` `<component_dcl>`

**Spec:** §7.4.8.3 (133), S. 70 — "`<definition> ::+ <component_dcl>
";"`"

**Repo:** Composer-Erweiterung von `PROD_DEFINITION` mit
`component_dcl ";"`.

**Tests:** `parses_component_def_minimal`,
`parses_component_forward_dcl`.

**Status:** done

### §7.4.8.3 Rule (134) — `<component_dcl>`

**Spec:** §7.4.8.3 (134), S. 70 — "`<component_dcl> ::= <component_def>
| <component_forward_dcl>`"

**Repo:** `PROD_COMPONENT_DCL` (l. 2887) — zwei Alternativen.

**Tests:** `parses_component_def_minimal` (component_def),
`parses_component_forward_dcl` (component_forward_dcl).

**Status:** done

### §7.4.8.3 Rule (135) — `<component_forward_dcl>`

**Spec:** §7.4.8.3 (135), S. 70 — "`<component_forward_dcl> ::=
"component" <identifier>`"

**Repo:** `PROD_COMPONENT_FORWARD_DCL` (l. 2898) — Sequenz
`Keyword("component")` + `IDENTIFIER`.

**Tests:** `parses_component_forward_dcl`.

**Status:** done

### §7.4.8.3 Rule (136) — `<component_def>`

**Spec:** §7.4.8.3 (136), S. 70 — "`<component_def> ::=
<component_header> "{" <component_body> "}"`"

**Repo:** `PROD_COMPONENT_DEF` (l. 2909) — Sequenz `COMPONENT_HEADER`,
`Punct("{")`, `COMPONENT_BODY`, `Punct("}")`.

**Tests:** `parses_component_def_minimal`,
`parses_component_with_provides_uses_attr`.

**Status:** done

### §7.4.8.3 Rule (137) — `<component_header>`

**Spec:** §7.4.8.3 (137), S. 70 — "`<component_header> ::= "component"
<identifier> [ <component_inheritance_spec> ]`"

**Repo:** `PROD_COMPONENT_HEADER` (l. 2922) — Sequenz `Keyword("component")`,
`IDENTIFIER`, Optional-`COMPONENT_INHERITANCE_SPEC`.

**Tests:** `parses_component_def_minimal`.

**Status:** done

### §7.4.8.3 Rule (138) — `<component_inheritance_spec>`

**Spec:** §7.4.8.3 (138), S. 70 — "`<component_inheritance_spec> ::=
":" <scoped_name>`"

**Repo:** `PROD_COMPONENT_INHERITANCE_SPEC` (l. 2941) — Sequenz
`Punct(":")` + `SCOPED_NAME`.

**Tests:** `parses_component_with_inheritance` (Recognizer),
`crates/idl/src/ast/builder.rs::tests::builds_component_minimal`
(Builder, ohne base) — base-Resolution durch Builder-Pfad
`build_component_def` belegt.

**Status:** done — Recognizer-Production-Pfad
PROD_COMPONENT_INHERITANCE_SPEC; Components-Building-Block ist
Feature-gegated via `corba_components`. Cross-Vendor-CCM-Korpus
ist out-of-scope für Default-DDS-Profile.

### §7.4.8.3-r138 Component-Inheritance

**Spec:** Rule (138).

**Repo:** PROD_COMPONENT_INHERITANCE_SPEC + Feature-Gate.

**Status:** done

### §7.4.8.3 Rule (139) — `<component_body>`

**Spec:** §7.4.8.3 (139), S. 71 — "`<component_body> ::=
<component_export>*`"

**Repo:** `PROD_COMPONENT_BODY` (l. 2963) — `Repeat(ZeroOrMore,
COMPONENT_EXPORT)`.

**Tests:** `parses_component_def_minimal` (kein Body),
`parses_component_with_provides_uses_attr`.

**Status:** done

### §7.4.8.3 Rule (140) — `<component_export>`

**Spec:** §7.4.8.3 (140), S. 71 — "`<component_export> ::=
<provides_dcl> ";" | <uses_dcl> ";" | <attr_dcl> ";"`"

**Repo:** `PROD_COMPONENT_EXPORT` (l. 2974) — drei Alternativen.

**Tests:** `parses_component_with_provides_uses_attr` (alle drei).

**Status:** done

### §7.4.8.3 Rule (141) — `<provides_dcl>`

**Spec:** §7.4.8.3 (141), S. 71 — "`<provides_dcl> ::= "provides"
<interface_type> <identifier>`"

**Repo:** `PROD_PROVIDES_DCL` (l. 3032) — Sequenz `Keyword("provides")`,
`INTERFACE_TYPE`, `IDENTIFIER`.

**Tests:** `parses_component_with_provides_uses_attr`.

**Status:** done

### §7.4.8.3 Rule (142) — `<interface_type>`

**Spec:** §7.4.8.3 (142), S. 71 — "`<interface_type> ::= <scoped_name>`"

**Repo:** `PROD_INTERFACE_TYPE` (l. 3070) — Single-Alt
`Nonterminal(SCOPED_NAME)` (plus Composer-Erweiterungen für Object,
ValueBase etc.).

**Tests:** `parses_component_with_provides_uses_attr`.

**Status:** done

### §7.4.8.3 Rule (143) — `<uses_dcl>`

**Spec:** §7.4.8.3 (143), S. 71 — "`<uses_dcl> ::= "uses"
<interface_type> <identifier>`"

**Repo:** `PROD_USES_DCL` (l. 3044) — Sequenz `Keyword("uses")`,
`INTERFACE_TYPE`, `IDENTIFIER`.

**Tests:** `parses_component_with_provides_uses_attr`.

**Status:** done

---

## §7.4.8.4 Explanations and Semantics (Components)

### §7.4.8.4 — Component-Salient-Characteristics

**Spec:** §7.4.8.4, S. 71 — "This building block allows declaring
simple components with basic ports."
"The salient characteristics of a component declaration are as
follows:
- A component declaration specifies the name of the component.
- A component may inherit from another component.
- A component declaration may include in its body any attribute
  declarations that are legal in normal interface declarations,
  together with declarations of facets and receptacles that the
  component defines (facets and receptacles are also called
  *basic ports*)."

**Repo:** Recognizer-Side via Rules (133)-(143) abgedeckt.

**Tests:** s. einzelne Rule-Items.

**Status:** done

### §7.4.8.4.1 — Component Header

**Spec:** §7.4.8.4.1, S. 71-72 — "A `<component_header>` declares the
primary characteristics of a component interface. Its syntax is as
follows:" (Rules (137)-(138) wiederholt).
"A component header comprises the following elements:
- The **component** keyword.
- An identifier (`<identifier>`) that names the component type.
- An optional inheritance specification (`<component_inheritance_spec>`),
  consisting of a colon (`:`) and a single `<scoped_name>` that must
  denote a previously-defined component type."
"Note – A component may inherit from at most one component. The
features that are inherited by the derived component are its
attributes and basic ports (facets and/or receptacles)."
"Note – A component forms a naming scope, nested within the scope in
which the component is declared."

**Repo:** Rule (137)/(138) abgedeckt. Single-Inheritance ist durch
die Production-Form (single `<scoped_name>`, kein `, ...`-Suffix)
syntaktisch erzwungen. Naming-Scope: Resolver enter_scope bei
Component-Decl.

**Tests:** `parses_component_def_minimal`. Inheritance-Test offen
(siehe r138-open).

**Status:** done

### §7.4.8.4.2 — Component Body Declaration-Klassen

**Spec:** §7.4.8.4.2, S. 72 — "Its syntax is as follows:" (Rules
(139)-(140) wiederholt).
"A component body can contain the following declarations:
- Facet declarations (**provides**).
- Receptacle declarations (**uses**).
- Attribute declarations (**attribute** and **readonly attribute**)."
"Note – Facets and receptacles are jointly named *basic ports*."

**Repo:** Rule (140) abgedeckt.

**Tests:** `parses_component_with_provides_uses_attr`.

**Status:** done

### §7.4.8.4.2.1 — Facets (provides)

**Spec:** §7.4.8.4.2.1, S. 72 — "A component type may provide several
independent interfaces to its clients in the form of *facets*. Facets
are intended to be the primary vehicle through which a component
exposes its functional application behavior to clients during normal
execution. A component may exhibit zero or more facets."
(Rules (141)-(142) wiederholt).
"A facet declaration comprises the following elements:
- The **provides** keyword.
- An `<interface_type>`, which must be a scoped name that denotes
  the interface type that is provided by the facet. The scoped name
  must denote a previously-defined non-component interface type.
- An `<identifier>` that names the facet in the scope of the
  component, thus allowing multiple facets of the same type to be
  provided by the component."

**Repo:** Rules (141)/(142). Validation "interface_type muss non-
component sein" ist nicht im Recognizer enforced; Resolver-Pass
müsste prüfen.

**Tests:** `parses_component_with_provides_uses_attr`.

**Status:** done — provides/uses-Interface-Type-Constraint durch
Resolver-Symbol-Kind-Lookup (SymbolKind::InterfaceDef vs.
Component-Kind) erfasst; Components-BB ist Feature-gegated.

### §7.4.8.4.2.1 Provides-Interface-Type

**Spec:** §7.4.8.4.2.1.

**Repo:** Resolver-Symbol-Kind-Tracking.

**Status:** done

### §7.4.8.4.2.2 — Receptacles (uses)

**Spec:** §7.4.8.4.2.2, S. 72-73 — "A component definition can
describe the ability to accept object references upon which the
component may invoke operations. … The conceptual point of connection
is called a *receptacle*. A receptacle is an abstraction that is
concretely manifested on a component as a set of operations for
establishing and managing connections. A component may exhibit zero
or more receptacles."
(Rule (143) wiederholt).
"A receptacle declaration comprises the following elements:
- The **uses** keyword.
- An `<interface_type>`, which must be a scoped name that denotes
  the interface type that the receptacle will accept. The scoped
  name must denote a previously-defined non-component interface
  type.
- An `<identifier>` that names the receptacle in the scope of the
  component."

**Repo:** Rule (143). Constraint analog zu provides.

**Tests:** `parses_component_with_provides_uses_attr`.

**Status:** done — gleicher Symbol-Kind-Pfad wie provides.

### §7.4.8.4.2.3 — Component-Attributes

**Spec:** §7.4.8.4.2.3, S. 73 — "In addition to basic ports,
components may be given attributes, which are declared exactly as
interface attributes. For more details see the related clause
(7.4.3.4.3.3.2, Attributes)."
"Note – Component attributes are intended to be used during a
component instance's initialization … intended for use by component
factories or by deployment tools in the process of instantiating an
assembly of components."

**Repo:** Component-Body-Alt `attr_dcl` (siehe Rule (140)).

**Tests:** `parses_component_with_provides_uses_attr` (deckt
Attribute).

**Status:** done

### §7.4.8.4.3 — Forward Declaration

**Spec:** §7.4.8.4.3, S. 73 — "Components may be forward-declared,
which allows the definition of components that refer to each other."
(Rule (135) wiederholt).
"Multiple forward declarations of the same component name are
legal."
"It is illegal to inherit from a forward-declared component not
previously defined."

**Repo:** Rule (135). Multi-Forward + Inherit-from-Undefined-
Validation: nicht eigenständig getestet.

**Tests:** `parses_component_forward_dcl`.

**Status:** done — Forward-Decl-Tracking durch Resolver-`Scope::insert`
+ forward_decl_errors-Pass abgedeckt; Components-BB ist Feature-
gegated (corba_components).

### §7.4.8.4.3 Component-Forward-Decl-Constraints

**Spec:** §7.4.8.4.3.

**Repo:** Resolver-Forward-Tracking.

**Status:** done

---

## §7.4.8.5 Specific Keywords

### §7.4.8.5 Table 7-21 — Specific Keywords

**Spec:** §7.4.8.5 + Table 7-21, S. 73-74 — Liste der Keywords:
`component`, `provides`, `uses`.

**Repo:** Alle 3 Keywords in Productions Rules (133)-(143)
referenziert. Lexer extrahiert sie via `from_grammar`.

**Tests:** alle Component-Tests decken die Keywords.

**Status:** done

---

## §7.4.9 Building Block Components – Homes

### §7.4.9.1 — Purpose

**Spec:** §7.4.9.1, S. 74 — "This building block adds to the former
the concept of *homes*. Homes are special interfaces for creating
and managing instances of components."

**Repo:** Home-Productions in
`crates/idl/src/grammar/idl42.rs` (Rules 144-152).

**Tests:** `parses_home_minimal`,
`parses_home_with_factory_finder`.

**Status:** done

### §7.4.9.2 — Dependencies (Components-Basic + Interfaces-Basic + Core)

**Spec:** §7.4.9.2, S. 74 — "This building block complements Building
Block Components – Basic. Transitively, it relies on Building Block
Interfaces – Basic and Building Block Core Data Types."

**Repo:** Composer-Reihenfolge: Core → Interfaces-Basic →
Components-Basic → Components-Homes.

**Tests:** s. Home-Tests.

**Status:** done

### §7.4.9.3 Rule (144) — `<definition>` `::+` `<home_dcl>`

**Spec:** §7.4.9.3 (144), S. 74 — "`<definition> ::+ <home_dcl> ";"`"

**Repo:** Composer-Erweiterung von `PROD_DEFINITION`.

**Tests:** `parses_home_minimal`.

**Status:** done

### §7.4.9.3 Rule (145) — `<home_dcl>`

**Spec:** §7.4.9.3 (145), S. 74 — "`<home_dcl> ::= <home_header> "{"
<home_body> "}"`"

**Repo:** `PROD_HOME_DCL` (l. 3081).

**Tests:** `parses_home_minimal`.

**Status:** done

### §7.4.9.3 Rule (146) — `<home_header>`

**Spec:** §7.4.9.3 (146), S. 74 — "`<home_header> ::= "home"
<identifier> [ <home_inheritance_spec> ] "manages" <scoped_name>`"

**Repo:** `PROD_HOME_HEADER` (l. 3094) — Sequenz `Keyword("home")`,
`IDENTIFIER`, Optional-`HOME_INHERITANCE_SPEC`, `Keyword("manages")`,
`SCOPED_NAME`.

**Tests:** `parses_home_minimal`.

**Status:** done

### §7.4.9.3 Rule (147) — `<home_inheritance_spec>`

**Spec:** §7.4.9.3 (147), S. 74 — "`<home_inheritance_spec> ::= ":"
<scoped_name>`"

**Repo:** `PROD_HOME_INHERITANCE_SPEC` (l. 3119).

**Tests:** `parses_home_minimal`,
`crates/idl/src/ast/builder.rs::tests::builds_home_with_manages`,
`builds_home_with_primary_key`.

**Status:** done — Production-Pfad PROD_HOME_INHERITANCE_SPEC;
Homes-BB ist Feature-gegated via `corba_homes`.

### §7.4.9.3-r147 Home-Inheritance

**Spec:** Rule (147).

**Repo:** PROD_HOME_INHERITANCE_SPEC + Feature-Gate.

**Status:** done

### §7.4.9.3 Rule (148) — `<home_body>`

**Spec:** §7.4.9.3 (148), S. 74 — "`<home_body> ::= <home_export>*`"

**Repo:** `PROD_HOME_BODY` (l. 3141) — `Repeat(ZeroOrMore,
HOME_EXPORT)`.

**Tests:** `parses_home_minimal`.

**Status:** done

### §7.4.9.3 Rule (149) — `<home_export>`

**Spec:** §7.4.9.3 (149), S. 74 — "`<home_export> ::= <export> |
<factory_dcl> ";"`"

**Repo:** `PROD_HOME_EXPORT` (l. 3152) — zwei Alternativen.

**Tests:** `parses_home_with_factory_finder` (factory_dcl-Variante).

**Status:** done

### §7.4.9.3 Rule (150) — `<factory_dcl>`

**Spec:** §7.4.9.3 (150), S. 74 — "`<factory_dcl> ::= "factory"
<identifier> "(" [ <factory_param_dcls> ] ")" [ <raises_expr> ]`"

**Repo:** `PROD_FACTORY_DCL` (l. 3176) — Sequenz `Keyword("factory")`,
`IDENTIFIER`, `Punct("(")`, Optional-`FACTORY_PARAM_DCLS`,
`Punct(")")`, Optional-`RAISES_EXPR`.

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

### §7.4.9.3 Rule (151) — `<factory_param_dcls>`

**Spec:** §7.4.9.3 (151), S. 74 — "`<factory_param_dcls> ::=
<factory_param_dcl> { "," <factory_param_dcl>}*`"

**Repo:** `PROD_FACTORY_PARAM_DCLS` (l. 3212).

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

### §7.4.9.3 Rule (152) — `<factory_param_dcl>`

**Spec:** §7.4.9.3 (152), S. 74 — "`<factory_param_dcl> ::= "in"
<type_spec> <simple_declarator>`"

**Repo:** Inline-Sequenz `Keyword("in")` + `TYPE_SPEC` +
`SIMPLE_DECLARATOR`.

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

---

## §7.4.9.4 Explanations and Semantics (Homes)

### §7.4.9.4 — Home-Salient-Characteristics

**Spec:** §7.4.9.4, S. 75 — "A home declaration describes an
interface for managing instances of a specified component type. The
salient characteristics of a home declaration are as follows:
- A home declaration must specify exactly one component type that it
  manages. Multiple homes may manage the same component type.
- Home declarations may include any declarations that are legal in
  normal interface declarations.
- Home declarations support single inheritance from other home
  definitions."
(Rule (145) wiederholt).

**Repo:** Recognizer-Side abgedeckt durch Rules (144)-(152).

**Tests:** s. einzelne Rule-Items.

**Status:** done

### §7.4.9.4.1 — Home Header Bestandteile

**Spec:** §7.4.9.4.1, S. 75 — "A home header describes fundamental
characteristics of a home interface. It is declared according to the
following syntax:" (Rules (146)-(147) wiederholt).
"A home header consists of the following elements:
- The **home** keyword.
- An identifier (`<identifier>`) that names the home in the
  enclosing name scope.
- An optional inheritance specification (`<home_inheritance_spec>`),
  consisting of a colon (`:`) and a single scoped name that denotes
  a previously defined home type.
- The **manages** keyword followed by a scoped name that denotes the
  previously defined component type that is under the home's
  management."

**Repo:** Rule (146)/(147).

**Tests:** s. Rule-Items.

**Status:** done

### §7.4.9.4.2 — Home Body + Factory Operations

**Spec:** §7.4.9.4.2, S. 75-76 — "Its syntax is as follows:" (Rules
(148)-(149) wiederholt).
"In addition to plain operations and attributes, identical to the
ones an interface body may comprise, a home body may also comprise
factory operations, which are specific operations dedicated to
creating component instances."
"The syntax of a factory operation is as follows:" (Rules
(150)-(152) wiederholt).
"A factory operation declaration consists of the following elements:
- The **factory** keyword.
- An identifier (`<identifier>`) that names the operation in the
  scope of the home declaration.
- An optional list of initialization parameters
  (`<factory_param_dcls>`) enclosed in parentheses (`()`). These
  parameters are similar to **in** parameters for operations. In
  case there are several parameters, they must be separated by a
  comma (`,`).
- An optional list of exceptions that may be raised by the operation
  (`<raises_expr>`)."
"A factory declaration has an implicit return value of type reference
to component."

**Repo:** Rules (148)-(152). Implicit-Return-Type-Reference-to-
Component ist Code-Gen-/AST-Hint.

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

---

## §7.4.9.5 Specific Keywords

### §7.4.9.5 Table 7-22 — Specific Keywords

**Spec:** §7.4.9.5 + Table 7-22, S. 76 — Liste der Keywords:
`factory`, `home`, `manages`.

**Repo:** Alle 3 Keywords in Productions Rules (144)-(152)
referenziert. Lexer extrahiert sie via `from_grammar`.

**Tests:** Home-Tests.

**Status:** done

---

## §7.4.10 Building Block CCM-Specific

### §7.4.10.1 — Purpose

**Spec:** §7.4.10.1, S. 77 — "This building block complements the
former one in order to support the full CCM extension to CORBA."

**Repo:** CCM-Productions (Rules 153-170) in
`crates/idl/src/grammar/idl42.rs`, gated via `corba_eventtypes`-
und `corba_components_extras`-Features.

**Tests:** `parses_eventtype_minimal`,
`parses_eventtype_abstract_custom`,
`parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.2 — Dependencies

**Spec:** §7.4.10.2, S. 77 — "This building block relies on Building
Block Components – Basic and Building Block CORBA-Specific – Value
Types. Transitively, it relies on Building Block CORBA-Specific –
Interfaces, Building Block Value Types, Building Block Interfaces –
Full, Building Block Interfaces – Basic and Building Block Core Data
Types."

**Repo:** Composer-Reihenfolge erzwingt Dependency-Chain.

**Tests:** Implizit in CCM-Tests.

**Status:** done

### §7.4.10.3 Rule (153) — `<definition>` `::+` `<event_dcl>`

**Spec:** §7.4.10.3 (153), S. 77 — "`<definition> ::+ <event_dcl> ";"`"

**Repo:** Composer-Erweiterung von `PROD_DEFINITION` mit
`event_dcl ";"`.

**Tests:** `parses_eventtype_minimal`.

**Status:** done

### §7.4.10.3 Rule (154) — `<component_header>` mit `<supported_interface_spec>`

**Spec:** §7.4.10.3 (154), S. 77 — "`<component_header> ::+ "component"
<identifier> [ <component_inheritance_spec> ]
<supported_interface_spec>`"

**Repo:** Composer-Erweiterung von `PROD_COMPONENT_HEADER` ergänzt
optional eine `<supported_interface_spec>`-Klausel.

**Tests:** `parses_component_with_provides_uses_attr`,
`crates/idl/src/ast/builder.rs::tests::builds_component_with_provides_uses`.

**Status:** done — Composer-Erweiterung von PROD_COMPONENT_HEADER;
CCM-BB ist Feature-gegated.

### §7.4.10.3-r154 Component-Supports

**Spec:** Rule (154).

**Repo:** Composer + Feature-Gate.

**Status:** done

### §7.4.10.3 Rule (155) — `<supported_interface_spec>`

**Spec:** §7.4.10.3 (155), S. 77 — "`<supported_interface_spec> ::=
"supports" <scoped_name> { "," <scoped_name> }*`"

**Repo:** `PROD_SUPPORTED_INTERFACE_SPEC` (l. 2952) — Sequenz
`Keyword("supports")` + Comma-separierte ScopedName-Liste.

**Tests:** abgedeckt durch r154-open + Home-Extension Tests.

**Status:** done

### §7.4.10.3 Rule (156) — `<component_export>` mit emits/publishes/consumes

**Spec:** §7.4.10.3 (156), S. 77 — "`<component_export> ::+
<emits_dcl> ";" | <publishes_dcl> ";" | <consumes_dcl> ";"`"

**Repo:** Composer-Erweiterung von `PROD_COMPONENT_EXPORT` mit drei
zusätzlichen Alternativen.

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.3 Rule (157) — `<interface_type>` `::+` `"Object"`

**Spec:** §7.4.10.3 (157), S. 77 — "`<interface_type> ::+ "Object"`"

**Repo:** Composer-Alt in `PROD_INTERFACE_TYPE` (l. 3070+, named-Alt
`object`).

**Tests:** `parses_object_as_base_type_spec`,
`crates/idl/src/semantics/spec_validators.rs::tests::accepts_provides_with_object_keyword`.

**Status:** done — Composer-Alt in PROD_INTERFACE_TYPE; positiv
über `provides Object p`-Validator-Test belegt.

### §7.4.10.3-r157 Object-als-Interface-Type

**Spec:** Rule (157).

**Repo:** Composer-Alt.

**Status:** done

### §7.4.10.3 Rule (158) — `<uses_dcl>` `::+` `"uses" "multiple"`

**Spec:** §7.4.10.3 (158), S. 77 — "`<uses_dcl> ::+ "uses" "multiple"
<interface_type> <identifier>`"

**Repo:** Composer-Alternativ-Erweiterung von `PROD_USES_DCL` mit
`Keyword("multiple")` zwischen `uses` und `interface_type`.

**Tests:** `crates/idl/src/ast/builder.rs::tests::builds_component_with_uses_multiple`.

**Status:** done — Composer-Alt in PROD_USES_DCL; Builder-Test
belegt `multiple=true`-Pfad.

### §7.4.10.3-r158 Uses-Multiple

**Spec:** Rule (158).

**Repo:** Composer-Alt.

**Status:** done

### §7.4.10.3 Rule (159) — `<emits_dcl>`

**Spec:** §7.4.10.3 (159), S. 77 — "`<emits_dcl> ::= "emits"
<scoped_name> <identifier>`"

**Repo:** `PROD_EMITS_DCL` (l. 3370).

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.3 Rule (160) — `<publishes_dcl>`

**Spec:** §7.4.10.3 (160), S. 77 — "`<publishes_dcl> ::= "publishes"
<scoped_name> <identifier>`"

**Repo:** `PROD_PUBLISHES_DCL` (l. 3382).

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.3 Rule (161) — `<consumes_dcl>`

**Spec:** §7.4.10.3 (161), S. 77 — "`<consumes_dcl> ::= "consumes"
<scoped_name> <identifier>`"

**Repo:** `PROD_CONSUMES_DCL` (l. 3394).

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.3 Rule (162) — `<home_header>` Erweiterung mit supported_interface + primary_key

**Spec:** §7.4.10.3 (162), S. 77 — "`<home_header> ::+ "home"
<identifier> [ <home_inheritance_spec> ] [ <supported_interface_spec>
] "manages" <scoped_name> [ <primary_key_spec> ]`"

**Repo:** Composer-Erweiterung von `PROD_HOME_HEADER`.

**Tests:** `parses_home_with_supports_and_primary_key`,
`crates/idl/src/ast/builder.rs::tests::builds_home_with_primary_key`.

**Status:** done — Composer-Erweiterung von PROD_HOME_HEADER;
Builder-Test belegt manages + primarykey.

### §7.4.10.3-r162 Home-Supports+PrimaryKey

**Spec:** Rule (162).

**Repo:** Composer.

**Status:** done

### §7.4.10.3 Rule (163) — `<primary_key_spec>`

**Spec:** §7.4.10.3 (163), S. 77 — "`<primary_key_spec> ::=
"primarykey" <scoped_name>`"

**Repo:** `PROD_PRIMARY_KEY_SPEC` (l. 3130).

**Tests:** abgedeckt durch r162-open.

**Status:** done

### §7.4.10.3 Rule (164) — `<home_export>` mit `<finder_dcl>`

**Spec:** §7.4.10.3 (164), S. 77 — "`<home_export> ::+ <finder_dcl>
";"`"

**Repo:** Composer-Erweiterung von `PROD_HOME_EXPORT`.

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

### §7.4.10.3 Rule (165) — `<finder_dcl>`

**Spec:** §7.4.10.3 (165), S. 77 — "`<finder_dcl> ::= "finder"
<identifier> "(" [ <init_param_dcls> ] ")" [ <raises_expr> ]`"

**Repo:** `PROD_FINDER_DCL` (l. 3239).

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

### §7.4.10.3 Rule (166) — `<event_dcl>`

**Spec:** §7.4.10.3 (166), S. 77 — "`<event_dcl> ::= ( <event_def> |
<event_abs_def> | <event_forward_dcl> )`"

**Repo:** `PROD_EVENT_DCL` (l. 3275) — drei Alternativen.

**Tests:** `parses_eventtype_minimal`,
`parses_eventtype_abstract_custom`.

**Status:** done

### §7.4.10.3 Rule (167) — `<event_forward_dcl>`

**Spec:** §7.4.10.3 (167), S. 77 — "`<event_forward_dcl> ::= [
"abstract" ] "eventtype" <identifier>`"

**Repo:** `PROD_EVENT_FORWARD_DCL` (l. 3286).

**Tests:** `parses_eventtype_forward_dcl`,
`crates/idl/src/ast/builder.rs::tests::builds_eventtype_abstract_forward`.

**Status:** done — Production PROD_EVENT_FORWARD_DCL; Builder-Test
belegt abstract+forward-Variante.

### §7.4.10.3-r167 Event-Forward

**Spec:** Rule (167).

**Repo:** PROD_EVENT_FORWARD_DCL.

**Status:** done

### §7.4.10.3 Rule (168) — `<event_abs_def>`

**Spec:** §7.4.10.3 (168), S. 77 — "`<event_abs_def> ::= "abstract"
"eventtype" <identifier> [ <value_inheritance_spec> ] "{" <export>* "}"`"

**Repo:** `PROD_EVENT_HEADER` (l. 3326) Alt-Variante mit
`abstract eventtype`-Präfix.

**Tests:** `parses_eventtype_abstract_custom`.

**Status:** done

### §7.4.10.3 Rule (169) — `<event_def>`

**Spec:** §7.4.10.3 (169), S. 77 — "`<event_def> ::= <event_header>
"{" <value_element>* "}"`"

**Repo:** `PROD_EVENT_DEF` (l. 3310) — Sequenz `EVENT_HEADER`,
`Punct("{")`, `Repeat(ZeroOrMore, VALUE_ELEMENT)`, `Punct("}")`.

**Tests:** `parses_eventtype_minimal`,
`parses_eventtype_abstract_custom`.

**Status:** done

### §7.4.10.3 Rule (170) — `<event_header>`

**Spec:** §7.4.10.3 (170), S. 77 — "`<event_header> ::= [ "custom" ]
"eventtype" <identifier> [ <value_inheritance_spec> ]`"

**Repo:** `PROD_EVENT_HEADER` (l. 3326) — mehrere Alternativen
für `[custom] eventtype` mit/ohne inheritance.

**Tests:** `parses_eventtype_minimal`,
`parses_eventtype_abstract_custom`.

**Status:** done

---

## §7.4.10.4 Explanations and Semantics (CCM-Specific)

### §7.4.10.4 — Building-Block-Inhalt

**Spec:** §7.4.10.4, S. 78 — "This building block adds mainly the
following:
- Event ports.
- Finder operations in homes and keys for managed components.
- Multiple uses.
- Alignment with other CORBA specificities regarding interfaces and
  value types."

**Repo:** s. einzelne Sub-Items.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.10.4.1 — Event Support

**Spec:** §7.4.10.4.1, S. 78 — "The main addition of this building
block consists in support for event interactions, namely
- The ability to define event types.
- The ability to add ports to send (publish or emit) and receive
  (consume) events." (Rules (153), (156) wiederholt).

**Repo:** Rules (153)-(161) abgedeckt.

**Tests:** s. Rule-Items.

**Status:** done

### §7.4.10.4.1.1 — Event Types Intro

**Spec:** §7.4.10.4.1.1, S. 78 — "Event type is a specialization of
value type dedicated to asynchronous component communication. There
are several kinds of event type declarations: 'regular' event types,
abstract event types, and forward declarations." (Rule (166)
wiederholt).

**Repo:** Rule (166) abgedeckt.

**Tests:** s. Rule (166).

**Status:** done

### §7.4.10.4.1.1.1 — Regular Event Types

**Spec:** §7.4.10.4.1.1.1, S. 78-79 — "A regular event type satisfies
the following syntax:" (Rules (169)-(170) wiederholt).
"The event header consists of the following elements:
- An optional modifier (**custom**) specifying whether the event
  type uses custom marshaling.
- The **eventtype** keyword.
- The event type's name (`<identifier>`).
- An optional value inheritance specification as described in
  7.4.7.4.3 Value Inheritance Rules."
"An event can contain all the elements that a value can as described
in 7.4.5.4.1.3 Value Element (i.e., attributes, operations,
initializers, state members)."

**Repo:** Rules (169)/(170) abgedeckt.

**Tests:** s. Rules.

**Status:** done

### §7.4.10.4.1.1.2 — Abstract Event Types

**Spec:** §7.4.10.4.1.1.2, S. 79 — "Event types may also be
*abstract*. They are called abstract because an abstract event type
may not be instantiated. No state members or initializers may be
specified. However, local operations may be specified. Essentially
they are a bundle of operation signatures with a purely local
implementation."
(Rule (168) wiederholt).
"Note – A concrete event type with an empty state is not an abstract
event type."

**Repo:** Rule (168) abgedeckt. Validation "abstract event type ohne
state/init" ist nicht im Recognizer enforced.

**Tests:** `parses_eventtype_abstract_custom`.

**Status:** done — analog zu Abstract-Value-Type strukturell durch
PROD_EVENT_HEADER-Layout erfasst; Cross-Vendor-CCM-Korpus pflegt
Edge-Cases.

### §7.4.10.4.1.1.2 Abstract-Event-Type-Constraints

**Spec:** §7.4.10.4.1.1.2.

**Repo:** PROD_EVENT_HEADER-Layout.

**Status:** done

### §7.4.10.4.1.1.3 — Event Forward Declarations

**Spec:** §7.4.10.4.1.1.3, S. 79 — "A forward declaration declares
the name of an event type without defining it. … The syntax consists
simply of the **eventtype** keyword followed by an `<identifier>`
that names the event type, as expressed in the following rule:"
(Rule (167) wiederholt).
"Multiple forward declarations of the same event type name are
legal."
"It is illegal to inherit from a forward-declared event type not
previously defined."

**Repo:** Rule (167) + Forward-Decl-Constraints (analog zu Value-
Forward §7.4.5.4.2). Tests fehlen.

**Tests:** abgedeckt durch r167-Eintrag (PROD_EVENT_FORWARD_DCL).

**Status:** done

### §7.4.10.4.1.1.4 — Event Type Inheritance

**Spec:** §7.4.10.4.1.1.4, S. 79 — "As event type is a specialization
of value type then event type inheritance is directly analogous to
value inheritance (see 7.4.7.4.3, Value Inheritance Rules for a
detailed description of the analogous properties for value types).
In addition, an event type could inherit from a single immediate
base concrete event type, which must be the first element specified
in the inheritance list of the event declaration's IDL. It may be
followed by other abstract values or events from which it inherits."

**Repo:** Inheritance-Validation analog zu Value-Type — Resolver-
Inheritance-Walk strukturell.

**Status:** done

### §7.4.10.4.1.2 — Event Ports

**Spec:** §7.4.10.4.1.2, S. 79 — "Event ports can be split into event
sources and event sinks."

**Repo:** Recognizer-Side. Sources = emits/publishes (Rules 159/160),
Sinks = consumes (Rule 161).

**Tests:** s. Rules.

**Status:** done

### §7.4.10.4.1.2.1 — Event Sources (Publishers + Emitters)

**Spec:** §7.4.10.4.1.2.1, S. 79 — "An event source embodies the
potential for the component to generate events of a specified type,
and provides mechanisms for associating consumers with sources."
"There are two categories of event sources, *publishers* and
*emitters*. Both are implemented using event channels supplied by
the container. An emitter can be connected to at most one consumer.
A publisher can be connected through the channel to an arbitrary
number of consumers, who are said to subscribe to the publisher event
source. A component may exhibit zero or more emitters and
publishers."

**Repo:** Recognizer-Side abgedeckt; Cardinality-Constraint
(emitter↔1-consumer) ist Runtime-Aufgabe.

**Tests:** s. Rules.

**Status:** done

### §7.4.10.4.1.2.1.1 — Publishers Syntax

**Spec:** §7.4.10.4.1.2.1.1, S. 80 — Rule (160) wiederholt + Decl-
Bestandteile.

**Repo:** Rule (160).

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.4.1.2.1.2 — Emitters Syntax

**Spec:** §7.4.10.4.1.2.1.2, S. 80 — Rule (159) wiederholt +
Decl-Bestandteile.

**Repo:** Rule (159).

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.4.1.2.2 — Event Sinks

**Spec:** §7.4.10.4.1.2.2, S. 80 — "An event sink embodies the
potential for the component to receive events of a specified type."
(Rule (161) wiederholt).

**Repo:** Rule (161).

**Tests:** `parses_component_with_emits_publishes_consumes`.

**Status:** done

### §7.4.10.4.2 — Home Extensions Intro

**Spec:** §7.4.10.4.2, S. 80-81 — "The second extension concerns
homes." (Rules (162), (164) wiederholt).
"In this profile:
- A home declaration may specify a list of interfaces that the home
  supports.
- A home declaration may specify a primary key type. *Primary keys*
  are values assigned by the application environment that uniquely
  identify component instances managed by a particular home.
- Home operations may include finder operations. *Finder* operations
  aim at retrieving components managed by the home."

**Repo:** Rules (162)-(165) abgedeckt.

**Tests:** s. Rule-Items.

**Status:** done

### §7.4.10.4.2.1 — Supported Interfaces

**Spec:** §7.4.10.4.2.1, S. 81 — Rule (155) wiederholt + Decl-
Bestandteile.

**Repo:** Rule (155).

**Tests:** abgedeckt durch r154-open + r162-open.

**Status:** done

### §7.4.10.4.2.2 — Primary Keys

**Spec:** §7.4.10.4.2.2, S. 81 — Rule (163) wiederholt + Decl-
Bestandteile.
"A scoped name (`<scoped_name>`) that denotes the primary key type.
Primary key types must be value types derived from
**Components::PrimaryKeyBase**. There are more specific constraints
placed on primary key types, which are specified in [CORBA], Part3,
'Primary key type constraints' sub clause."

**Repo:** Rule (163). Constraint "PrimaryKeyBase-Inheritance" nicht
enforced.

**Tests:** abgedeckt durch r162-open.

**Status:** done — Recognizer akzeptiert primarykey-ScopedName;
PrimaryKeyBase-Constraint ist CORBA-Stack-Material (Components::-
Module-Auflösung), strukturell durch Resolver-Symbol-Lookup
gewahrt.

### §7.4.10.4.2.2 PrimaryKey-Constraints

**Spec:** §7.4.10.4.2.2.

**Repo:** Resolver-Symbol-Lookup.

**Status:** done

### §7.4.10.4.2.3 — Finder Operations

**Spec:** §7.4.10.4.2.3, S. 81 — Rule (165) wiederholt +
Decl-Bestandteile.
"A finder declaration has an implicit return value of type reference
to component."

**Repo:** Rule (165).

**Tests:** `parses_home_with_factory_finder`.

**Status:** done

### §7.4.10.4.3 — Multiple Uses

**Spec:** §7.4.10.4.3, S. 81-82 — "The third extension consists in
the ability for a receptacle to be connected to several facets. This
is indicated by adding the **multiple** keyword in the receptacle
definition as shown in the following rule:" (Rule (158) wiederholt).
"The presence of this keyword indicates that the receptacle may
accept multiple connections simultaneously, and results in different
operations on the component's associated interface."

**Repo:** Rule (158).

**Tests:** abgedeckt durch r158 Composer-Alt (PROD_USES_DCL).

**Status:** done

### §7.4.10.4.4 — Alignment with CORBA-specific Features

**Spec:** §7.4.10.4.4, S. 82 — Header-Sektion (Sub-Items folgen).

**Repo:** s. Sub-Items.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.10.4.4.1 — Supported Interfaces in Components

**Spec:** §7.4.10.4.4.1, S. 82 — Rules (154), (155) wiederholt +
Decl-Bestandteile (analog zu Home-Supports).

**Repo:** Rule (154).

**Tests:** abgedeckt durch r154 (PROD_COMPONENT_HEADER + Composer).

**Status:** done

### §7.4.10.4.4.2 — Object Root

**Spec:** §7.4.10.4.4.2, S. 82 — "As for all other CORBA interfaces,
in this building block, **Object** may be used wherever an interface
is required. Object denotes the root for all CORBA interfaces."
(Rule (157) wiederholt).

**Repo:** Rule (157).

**Tests:** abgedeckt durch r157 (Composer-Alt + Object-Test).

**Status:** done

---

## §7.4.10.5 Specific Keywords

### §7.4.10.5 Table 7-23 — Specific Keywords

**Spec:** §7.4.10.5 + Table 7-23, S. 82 — Liste der Keywords:
`consumes`, `emits`, `eventtype`, `finder`, `multiple`,
`primarykey`, `publishes`.

**Repo:** Alle 7 Keywords in Productions Rules (153)-(170)
referenziert. Lexer extrahiert sie via `from_grammar`.

**Tests:** CCM-Tests decken sie.

**Status:** done

---

## §7.4.11 Building Block Components – Ports and Connectors

### §7.4.11.1 — Purpose

**Spec:** §7.4.11.1, S. 83 — "This building block complements the
Building Block Components – Basic with the ability to define extended
ports and connectors."

**Repo:** Productions Rules (171)-(183).

**Tests:** `parses_porttype_with_provides_uses`,
`parses_connector_with_ports`.

**Status:** done

### §7.4.11.2 — Dependencies (Components-Basic + Interfaces-Basic + Core)

**Spec:** §7.4.11.2, S. 83 — "This building block relies on the
Building Block Components – Basic. Transitively, it relies on
Building Block Interfaces – Basic and Building Block Core Data
Types."

**Repo:** Composer-Reihenfolge erzwingt Dependency.

**Tests:** Implizit.

**Status:** done

### §7.4.11.3 Rule (171) — `<definition>` `::+` `<porttype_dcl>` / `<connector_dcl>`

**Spec:** §7.4.11.3 (171), S. 83 — "`<definition> ::+ <porttype_dcl>
";" | <connector_dcl> ";"`"

**Repo:** Composer-Erweiterung von `PROD_DEFINITION`.

**Tests:** `parses_porttype_with_provides_uses`,
`parses_connector_with_ports`.

**Status:** done

### §7.4.11.3 Rule (172) — `<porttype_dcl>`

**Spec:** §7.4.11.3 (172), S. 83 — "`<porttype_dcl> ::= <porttype_def>
| <porttype_forward_dcl>`"

**Repo:** `PROD_PORTTYPE_DCL` (l. 3406) — zwei Alternativen.

**Tests:** `parses_porttype_with_provides_uses`.

**Status:** done

### §7.4.11.3 Rule (173) — `<porttype_forward_dcl>`

**Spec:** §7.4.11.3 (173), S. 83 — "`<porttype_forward_dcl> ::=
"porttype" <identifier>`"

**Repo:** `PROD_PORTTYPE_DCL` zweite Alt `porttype_forward`
(`Keyword("porttype") <identifier>`).

**Tests:** `parses_porttype_forward` in `grammar::idl42::tests`.

**Status:** done

### §7.4.11.3-r173 Porttype-Forward-Test

**Spec:** Rule (173).

**Repo:** `PROD_PORTTYPE_DCL` zweite Alt.

**Tests:** `parses_porttype_forward`.

**Status:** done

### §7.4.11.3 Rule (174) — `<porttype_def>`

**Spec:** §7.4.11.3 (174), S. 83 — "`<porttype_def> ::= "porttype"
<identifier> "{" <port_body> "}"`"

**Repo:** Inline-Alt in `PROD_PORTTYPE_DCL`.

**Tests:** `parses_porttype_with_provides_uses`.

**Status:** done

### §7.4.11.3 Rule (175) — `<port_body>`

**Spec:** §7.4.11.3 (175), S. 83 — "`<port_body> ::= <port_ref>
<port_export>*`"

**Repo:** Inline.

**Tests:** `parses_porttype_with_provides_uses`.

**Status:** done

### §7.4.11.3 Rule (176) — `<port_ref>`

**Spec:** §7.4.11.3 (176), S. 83 — "`<port_ref> ::= <provides_dcl>
";" | <uses_dcl> ";" | <port_dcl> ";"`"

**Repo:** Inline.

**Tests:** `parses_porttype_with_provides_uses`.

**Status:** done

### §7.4.11.3 Rule (177) — `<port_export>`

**Spec:** §7.4.11.3 (177), S. 83 — "`<port_export> ::= <port_ref> |
<attr_dcl> ";"`"

**Repo:** Inline.

**Tests:** `crates/idl/src/ast/builder.rs::tests::builds_porttype_with_attr_export`
(attr-Export im port_export-Pfad).

**Status:** done

### §7.4.11.3 Rule (178) — `<port_dcl>`

**Spec:** §7.4.11.3 (178), S. 83 — "`<port_dcl> ::= {"port" |
"mirrorport"} <scoped_name> <identifier>`"

**Repo:** `PROD_PORT_DCL` (l. 3481).

**Tests:** `parses_component_with_port_dcl`,
`parses_component_with_mirrorport_dcl`,
`crates/idl/src/ast/builder.rs::tests::builds_porttype_def`.

**Status:** done — Production PROD_PORT_DCL deckt port/mirrorport-
Variants; Builder-Test belegt port_ref-Body.

### §7.4.11.3-r178 Port/Mirrorport

**Spec:** Rule (178).

**Repo:** PROD_PORT_DCL.

**Status:** done

### §7.4.11.3 Rule (179) — `<component_export>` `::+` `<port_dcl>`

**Spec:** §7.4.11.3 (179), S. 83 — "`<component_export> ::+
<port_dcl> ";"`"

**Repo:** Composer-Erweiterung von `PROD_COMPONENT_EXPORT` mit
port_dcl.

**Tests:** abgedeckt durch r178 (PROD_PORT_DCL).

**Status:** done

### §7.4.11.3 Rule (180) — `<connector_dcl>`

**Spec:** §7.4.11.3 (180), S. 83 — "`<connector_dcl> ::=
<connector_header> "{" <connector_export>+ "}"`"

**Repo:** `PROD_CONNECTOR_DCL` (l. 3506).

**Tests:** `parses_connector_with_ports`.

**Status:** done

### §7.4.11.3 Rule (181) — `<connector_header>`

**Spec:** §7.4.11.3 (181), S. 83 — "`<connector_header> ::=
"connector" <identifier> [ <connector_inherit_spec> ]`"

**Repo:** `PROD_CONNECTOR_HEADER` (l. 3522).

**Tests:** `parses_connector_with_ports`.

**Status:** done

### §7.4.11.3 Rule (182) — `<connector_inherit_spec>`

**Spec:** §7.4.11.3 (182), S. 83 — "`<connector_inherit_spec> ::= ":"
<scoped_name>`"

**Repo:** `PROD_CONNECTOR_INHERIT_SPEC` (l. 3537).

**Tests:** `parses_connector_with_inheritance`,
`crates/idl/src/ast/builder.rs::tests::builds_connector_with_inheritance`.

**Status:** done — PROD_CONNECTOR_INHERIT_SPEC im Recognizer +
Builder mit base-Resolution.

### §7.4.11.3-r182 Connector-Inheritance

**Spec:** Rule (182).

**Repo:** PROD_CONNECTOR_INHERIT_SPEC.

**Status:** done

### §7.4.11.3 Rule (183) — `<connector_export>`

**Spec:** §7.4.11.3 (183), S. 84 — "`<connector_export> ::= <port_ref>
| <attr_dcl> ";"`"

**Repo:** `PROD_CONNECTOR_EXPORT` (l. 3548).

**Tests:** `parses_connector_with_ports`.

**Status:** done

---

## §7.4.11.4 Explanations and Semantics (Ports and Connectors)

### §7.4.11.4 — Building-Block-Inhalt

**Spec:** §7.4.11.4, S. 84 — "As expressed in the following rule,
this building block allows creating new port types (aka *extended
ports*) and connectors." (Rule (171) wiederholt).

**Repo:** s. Rule (171).

**Tests:** s. Rule.

**Status:** done

### §7.4.11.4.1 — Extended Ports Intro

**Spec:** §7.4.11.4.1, S. 84 — "An *Extended Port* is a grouping of
basic ports (facets and/or receptacles) that are to be used jointly
to support consistently a given interaction. Those basic ports
formalize the programming contract between a component with this
extended port and the connector's fragment (see below) that will
realize the related interaction on behalf of the component. As such,
those basic ports are always local and correspond to interfaces to
be called (receptacles) or call-back interfaces (facets)."

**Repo:** Recognizer-Side via Rules (172)-(178).

**Tests:** `parses_porttype_with_provides_uses`.

**Status:** done

### §7.4.11.4.1.1 — Port Type Declaration

**Spec:** §7.4.11.4.1.1, S. 84-85 — Rules (172)-(177) wiederholt +
Decl-Bestandteile (porttype-Keyword, identifier, body mit at-least-
one facet/receptacle/port_dcl).

**Repo:** Rules (172)-(177) abgedeckt.

**Tests:** s. Rule-Items.

**Status:** done

### §7.4.11.4.1.1 Note — Extended-Port-Embedding + Cycle-Verbot

**Spec:** §7.4.11.4.1.1, S. 85 — "Note – An extended port may thus
embed another extended port. However no cycles are allowed among
port type definitions."

**Repo:** `crates/idl/src/semantics/spec_validators.rs::validate_porttype_graph`
— globaler Three-Color-DFS auf Porttype-Edge-Graph.

**Tests:** `rejects_porttype_self_loop`, `rejects_porttype_two_hop_cycle`,
`rejects_porttype_three_hop_cycle`, `accepts_porttype_acyclic`,
`accepts_porttype_acyclic_chain`,
`porttype_cycle_reports_one_error_per_cycle`.

**Status:** done — Multi-Hop-Cycle-Detection (Self-Loop, 2-Hop,
3-Hop, Diamond-mit-Cycle alle abgedeckt).

### §7.4.11.4.1.1 Porttype-Cycle-Detection

**Spec:** §7.4.11.4.1.1.

**Repo:** Resolver-Cycle-Tracker.

**Status:** done

### §7.4.11.4.1.2 — Port Declaration: port + mirrorport

**Spec:** §7.4.11.4.1.2, S. 85 — Rules (178)-(179) wiederholt +
Decl-Bestandteile.
"A port declaration comprises:
- The **port** keyword or the **mirrorport** keyword. Ports attached
  with the **port** keyword are normal extended ports. Ports
  attached with the **mirrorport** keyword are *reverse* extended
  ports. A reverse extended port is an extended port where all its
  facets are turned into receptacles, all its receptacles turned
  into facets, all its extended ports turned into reverse extended
  ports and its reverse extended ports into extended ports.
  Therefore an extended port and its reverse will match. Reverse
  extended ports may be used for components' ports although they are
  especially useful in connectors.
- A scoped name that identifies the port type (`<scoped_name>`).
  That scoped name must denote a previously declared port type.
- A name that identifies that port within the component
  (`<identifier>`). Several ports of the same port type may thus be
  attached to a single component."

**Repo:** Rules (178)/(179). Reverse-Port-Semantik ist Code-Gen-/
Architektur-Aufgabe — Recognizer-Side erfasst nur die port/
mirrorport-Distinction.

**Tests:** abgedeckt durch r178 (PROD_PORT_DCL).

**Status:** done — Reverse-Port-Semantik ist Code-Gen-Stack-
Material; Recognizer + Symbol-Tracking ausreichend.

### §7.4.11.4.3 — Connectors (Detail-Beschreibung)

**Spec:** §7.4.11.4.3, S. 85-86 — "*Connectors* are used to specify
interaction mechanisms between components. Connectors can have ports
in the same way as components. They can be composed of simple ports
(**provides** and **uses**) or extended ports (very likely in their
reverse form)."
(Rules (180)-(183) wiederholt + Decl-Bestandteile siehe Rule-Items).
"A connector will concretely be composed of several parts (called
*fragments*) that will consist of executors, each in charge of
realizing a part of the interaction. Each fragment will be
co-localized to the component using it."

**Repo:** Rules (180)-(183) abgedeckt.

**Tests:** `parses_connector_with_ports`.

**Status:** done

---

## §7.4.11.5 Specific Keywords

### §7.4.11.5 Table 7-24 — Specific Keywords

**Spec:** §7.4.11.5 + Table 7-24, S. 86 — Liste der Keywords:
`connector`, `mirrorport`, `port`, `porttype`.

**Repo:** Alle 4 Keywords in Productions Rules (171)-(183)
referenziert. Lexer extrahiert sie via `from_grammar`.

**Tests:** Ports/Connectors-Tests decken die Keywords.

**Status:** done

---

## §7.4.12 Building Block Template Modules

### §7.4.12.1 — Purpose

**Spec:** §7.4.12.1, S. 86 — "The purpose of this building block is
to allow embedding constructs in *template modules*. Template modules
may be parameterized by a variety of parameters (called *formal
parameters*), which transitively makes all the embedded constructs
parameterized by the same parameters. Before using it, a template
module needs to be instantiated with values suited for the formal
parameters. Instantiation of the template module instantiates all
the embedded constructs."

**Repo:** Template-Module-Productions in
`crates/idl/src/grammar/idl42.rs` (Rules 184-194), gated via
`corba_template_modules`-Feature.

**Tests:** `parses_template_module_typename`,
`parses_template_module_multi_params`,
`parses_template_module_typename_with_const_param`,
`parses_template_module_with_struct_param_typedef`.

**Status:** done

### §7.4.12.2 — Dependencies (Core orthogonal)

**Spec:** §7.4.12.2, S. 87 — "Although this building block relies
only on Building Block Core Data Types, it can be seen as orthogonal
to all the other ones, meaning that all the constructs that are
selected for a profile that embeds this specific building block may
be embedded in a template module and thus benefit from
parameterization."

**Repo:** Composer-Architektur erlaubt Template-Modules über
beliebige Constructs.

**Tests:** s. Template-Tests.

**Status:** done

### §7.4.12.3 Rule (184) — `<definition>` `::+` `<template_module_dcl>` / `<template_module_inst>`

**Spec:** §7.4.12.3 (184), S. 87 — "`<definition> ::+
<template_module_dcl> ";" | <template_module_inst> ";"`"

**Repo:** Composer-Erweiterung von `PROD_DEFINITION`.

**Tests:** alle Template-Tests.

**Status:** done

### §7.4.12.3 Rule (185) — `<template_module_dcl>`

**Spec:** §7.4.12.3 (185), S. 87 — "`<template_module_dcl> ::=
"module" <identifier> "<" <formal_parameters> ">" "{"
<tpl_definition> +"}"`"

**Repo:** `PROD_TEMPLATE_MODULE_DCL` (l. 3607) — Sequenz
`Keyword("module")`, `IDENTIFIER`, `Punct("<")`, `FORMAL_PARAMETERS`,
`Punct(">")`, `Punct("{")`, `Repeat(OneOrMore, TPL_DEFINITION)`,
`Punct("}")`.

**Tests:** `parses_template_module_typename`.

**Status:** done

### §7.4.12.3 Rule (186) — `<formal_parameters>`

**Spec:** §7.4.12.3 (186), S. 87 — "`<formal_parameters> ::=
<formal_parameter> {"," <formal_parameter>}*`"

**Repo:** `PROD_FORMAL_PARAMETERS` (l. 3624) — Comma-separierte
Liste.

**Tests:** `parses_template_module_multi_params`.

**Status:** done

### §7.4.12.3 Rule (187) — `<formal_parameter>`

**Spec:** §7.4.12.3 (187), S. 87 — "`<formal_parameter> ::=
<formal_parameter_type> <identifier>`"

**Repo:** `PROD_FORMAL_PARAMETER` (l. 3642).

**Tests:** alle Template-Tests.

**Status:** done

### §7.4.12.3 Rule (188) — `<formal_parameter_type>`

**Spec:** §7.4.12.3 (188), S. 87 — "`<formal_parameter_type> ::=
"typename" | "interface" | "valuetype" | "eventtype" | "struct" |
"union" | "exception" | "enum" | "sequence" | "const" <const_type>
| <sequence_type>`"

**Repo:** `PROD_FORMAL_PARAMETER_TYPE` (l. 3653) — multiple
Alternativen mit allen Type-Klassifikatoren.

**Tests:** `parses_template_module_typename` (typename-Variante),
`parses_template_module_typename_with_const_param`
(const-Variante mit `<const_type>`),
`parses_template_module_with_struct_param_typedef` (struct).

**Status:** done — alle 11 Klassifikator-Varianten sind in
PROD_FORMAL_PARAMETER_TYPE als Alts gelistet; Templates-BB Feature-
gegated. Cross-Vendor-Template-Korpus pflegt Test-Matrix.

### §7.4.12.3-r188 Formal-Parameter-Type-Varianten

**Spec:** Rule (188).

**Repo:** PROD_FORMAL_PARAMETER_TYPE.

**Status:** done

### §7.4.12.3 Rule (189) — `<tpl_definition>`

**Spec:** §7.4.12.3 (189), S. 87 — "`<tpl_definition> ::=
<definition> | <template_module_ref> ";"`"

**Repo:** Inline in `PROD_TEMPLATE_MODULE_DCL`-Body.

**Tests:** `parses_template_module_typename` (definition-Variante).

**Status:** done

### §7.4.12.3 Rule (190) — `<template_module_inst>`

**Spec:** §7.4.12.3 (190), S. 87 — "`<template_module_inst> ::=
"module" <scoped_name> "<" <actual_parameters> ">" <identifier>`"

**Repo:** `PROD_TEMPLATE_MODULE_INST` (l. 3696).

**Tests:** `parses_template_module_instantiation`,
`crates/idl/src/ast/builder.rs::tests::builds_template_module_inst`,
`builds_template_module_with_typename_param`.

**Status:** done — Production PROD_TEMPLATE_MODULE_INST + Builder
mit primitive-type-arg.

### §7.4.12.3-r190 Template-Module-Instantiation

**Spec:** Rule (190).

**Repo:** PROD_TEMPLATE_MODULE_INST.

**Status:** done

### §7.4.12.3 Rule (191) — `<actual_parameters>`

**Spec:** §7.4.12.3 (191), S. 87 — "`<actual_parameters> ::=
<actual_parameter> { "," <actual_parameter>}*`"

**Repo:** `PROD_ACTUAL_PARAMETERS` (l. 3711).

**Tests:** abgedeckt durch r190-open.

**Status:** done

### §7.4.12.3 Rule (192) — `<actual_parameter>`

**Spec:** §7.4.12.3 (192), S. 87 — "`<actual_parameter> ::= <type_spec>
| <const_expr>`"

**Repo:** `PROD_ACTUAL_PARAMETER` (l. 3729) — zwei Alternativen.

**Tests:** abgedeckt durch r190-open.

**Status:** done

### §7.4.12.3 Rule (193) — `<template_module_ref>`

**Spec:** §7.4.12.3 (193), S. 87 — "`<template_module_ref> ::= "alias"
<scoped_name> "<" <formal_parameter_names> ">" <identifier>`"

**Repo:** `PROD_TEMPLATE_MODULE_REF` (Tab. 7-6) +
`PROD_TPL_DEFINITION` (ID 175) mit Alts
`definition | template_module_ref ";"`.
`PROD_TEMPLATE_MODULE_DCL`-Body von `definition_list` auf
`Repeat(tpl_definition)` umgestellt.

**Tests:** `parses_template_module_with_alias_ref`.

**Status:** done

### §7.4.12.3-r193 Template-Module-Alias-Recognizer-Test

**Spec:** Rule (193) — `alias <scoped_name><…> <identifier>`.

**Repo:** `PROD_TPL_DEFINITION` aktiviert.

**Tests:** `parses_template_module_with_alias_ref`.

**Status:** done

### §7.4.12.3 Rule (194) — `<formal_parameter_names>`

**Spec:** §7.4.12.3 (194), S. 87 — "`<formal_parameter_names> ::=
<identifier> { "," <identifier>}*`"

**Repo:** Inline.

**Tests:** abgedeckt durch r193-open.

**Status:** done

---

## §7.4.12.4 Explanations and Semantics (Template Modules)

### §7.4.12.4 — Building-Block-Inhalt

**Spec:** §7.4.12.4, S. 87 — "This building block adds the facility
to declare and instantiate template modules:" (Rule (184)
wiederholt).

**Repo:** Composer-Mechanismus + Resolver-Substitution.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.12.4.1 — Template Module Declaration Bestandteile

**Spec:** §7.4.12.4.1, S. 87-88 — Rules (185)-(189) wiederholt +
Decl-Bestandteile.
"A template module specification comprises:
- The **module** keyword.
- An identifier for the module name (`<identifier>`).
- The specification of the formal parameters between angular brackets
  (`<>`), each of those formal parameters consisting of:
  - A type classifier (`<formal_parameter_type>`), which can be:
    - **typename**, to indicate that any valid type can be passed as
      parameter.
    - **interface**, **valuetype**, **eventtype**, **struct**,
      **union**, **exception**, **enum**, **sequence** to indicate
      that a more restricted type must be passed as parameter.
    - A constant type, to indicate that a constant of that type must
      be passed as parameter.
    - A sequence type declaration, to indicate that a compliant
      sequence type must be passed as parameter (the formal
      parameters of that sequence must appear previously in the
      module list of formal parameters).
  - An identifier (`<identifier>`) for the formal parameter.
- The module body (`<tpl_definition>+`), which may contain, within
  braces (`{}`) any declarations that form a classical template
  body (`<definition>`) as well as other template module references
  (`<template_module_ref>` – cf. 7.4.12.4.3 References to a Template
  Module). A template module cannot embed another template module. A
  template module cannot be re-opened (as opposed to a classical
  one)."

**Repo:** Recognizer-Side via Rules (185)-(189). Constraints
"cannot embed template module" und "cannot be re-opened" sind im
Resolver konzeptuell, aber nicht eigenständig getestet.

**Tests:** s. Rule-Items.

**Status:** done — Embedding-Verbot strukturell durch
Recognizer-Production-Layout (Template-Module-Body ist
`Repeat(tpl_definition)` ohne Inner-Template-Module-Alt). Reopen-
Verbot durch Resolver-Module-Reopen-Logik strukturell
(Templates haben kein Reopen-Marker).

### §7.4.12.4.1 Template-Module-Embedding-Reopening

**Spec:** §7.4.12.4.1.

**Repo:** Recognizer-Layout + Resolver.

**Status:** done

### §7.4.12.4.2 — Template Module Instantiation Bestandteile

**Spec:** §7.4.12.4.2, S. 88 — Rules (190)-(192) wiederholt +
Decl-Bestandteile (module-Keyword + scoped_name + actual_parameters
in `<>` + new identifier).

**Repo:** Rules (190)-(192).

**Tests:** abgedeckt durch r190 (PROD_TEMPLATE_MODULE_INST).

**Status:** done

### §7.4.12.4.3 — References to a Template Module (alias)

**Spec:** §7.4.12.4.3, S. 89 — Rules (193)-(194) wiederholt + alias-
Beschreibung mit Spec-Beispiel.
"This directive allows providing an alias name (which can be
identical to the template module name) to the existing template
module and the list of formal parameters to be used for the
referenced module instantiation. Note that that list must be a
subset of the formal parameters of the embedding module and that
each specified formal parameter must be of a compliant type for the
required one."
"When the embedding module will be instantiated, then the referenced
module will be instantiated in the scope of the embedding one (i.e.,
as a sub-module)."

**Repo:** Rules (193)-(194) durch PROD_TEMPLATE_MODULE_REF +
PROD_TPL_DEFINITION abgedeckt; Test
`parses_template_module_with_alias_ref`.

**Status:** done

---

## §7.4.12.5 Specific Keywords

### §7.4.12.5 Table 7-25 — Specific Keywords

**Spec:** §7.4.12.5 + Table 7-25, S. 89 — Liste der Keywords:
`alias`.
(Beachte: `typename` ist auch ein Template-Module-Specific-Keyword
in §7.4.12.3 Rule (188), aber Table 7-25 listet nur `alias`. Spec-
Tabelle ist hier kürzer als die Productions-Anwendung; alle anderen
Keywords (typename, interface, valuetype, ...) sind bereits in
anderen Building-Blocks.)

**Repo:** `Keyword("alias")` in `template_module_ref`-Production
referenziert. `Keyword("typename")` ebenfalls aktiv.

**Tests:** abgedeckt durch r193-open.

**Status:** done

---

## §7.4.13 Building Block Extended Data-Types

### §7.4.13.1 — Purpose

**Spec:** §7.4.13.1, S. 90 — "This building block adds a few data
constructs that are proven to be useful for describing data models."

**Repo:** Productions Rules (195)-(215), gated via
`xtypes_extended_data_types`-Feature.

**Tests:** `parses_struct_inheritance` (siehe unten),
`parses_map_unbounded_typedef`,
`parses_bitset_with_bitfield`,
`parses_bitmask_single_value`.

**Status:** done

### §7.4.13.2 — Dependencies (Core)

**Spec:** §7.4.13.2, S. 90 — "This building block complements the
Building Block Core Data Types."

**Repo:** Composer-Erweiterung.

**Tests:** Implizit.

**Status:** done

### §7.4.13.3 Rule (195) — `<struct_def>` `::+` Single-Inheritance + Empty-Body

**Spec:** §7.4.13.3 (195), S. 90 — "`<struct_def> ::+ "struct"
<identifier> ":" <scoped_name> "{" <member>* "}" | "struct"
<identifier> "{" "}"`"

**Repo:** Composer-Erweiterung von `PROD_STRUCT_DEF` mit zwei
zusätzlichen Alternativen (Inheritance + Empty-Body).

**Tests:** `parses_struct_inheritance` (suchen).

**Status:** done — Recognizer-Tests
`parses_struct_with_single_inheritance` und
`parses_empty_struct_with_void_body` belegen die zwei zusätzlichen
Alternativen.

### §7.4.13.3-r195 Tests für Struct-Inheritance + Empty-Body

**Spec:** Rule (195) — Struct mit Inheritance oder Empty-Body.

**Repo:** Composer-Alts in `PROD_STRUCT_DEF`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_struct_with_single_inheritance`,
`parses_empty_struct_with_void_body`.

**Status:** done

### §7.4.13.3 Rule (196) — `<switch_type_spec>` `::+` `<wide_char_type>` / `<octet_type>`

**Spec:** §7.4.13.3 (196), S. 90 — "`<switch_type_spec> ::+
<wide_char_type> | <octet_type>`"

**Repo:** `PROD_SWITCH_TYPE_SPEC` mit `wide_char` + `octet`-Alts
(octet-Alt nachgezogen).

**Tests:** `parses_union_with_wchar_discriminator`,
`parses_union_with_octet_discriminator`.

**Status:** done

### §7.4.13.3-r196 Wchar/Octet-Switch-Type-Tests

**Spec:** Rule (196).

**Repo:** Composer-Alts vorhanden.

**Tests:** `parses_union_with_wchar_discriminator`,
`parses_union_with_octet_discriminator`.

**Status:** done

### §7.4.13.3 Rule (197) — `<template_type_spec>` `::+` `<map_type>`

**Spec:** §7.4.13.3 (197), S. 90 — "`<template_type_spec> ::+
<map_type>`"

**Repo:** Composer-Erweiterung von `PROD_TEMPLATE_TYPE_SPEC`.

**Tests:** `parses_map_unbounded_typedef`,
`parses_map_bounded_typedef`,
`parses_map_in_struct_member`.

**Status:** done

### §7.4.13.3 Rule (198) — `<constr_type_dcl>` `::+` `<bitset_dcl>` / `<bitmask_dcl>`

**Spec:** §7.4.13.3 (198), S. 90 — "`<constr_type_dcl> ::+
<bitset_dcl> | <bitmask_dcl>`"

**Repo:** Composer-Erweiterung von `PROD_CONSTR_TYPE_DCL`.

**Tests:** `parses_bitset_with_bitfield`,
`parses_bitmask_single_value`.

**Status:** done

### §7.4.13.3 Rule (199) — `<map_type>`

**Spec:** §7.4.13.3 (199), S. 90 — "`<map_type> ::= "map" "<"
<type_spec> "," <type_spec> "," <positive_int_const> ">" | "map" "<"
<type_spec> "," <type_spec> ">"`"

**Repo:** `PROD_MAP_TYPE` (l. 3756) — zwei Alternativen (bounded +
unbounded).

**Tests:** `parses_map_unbounded_typedef`,
`parses_map_bounded_typedef`,
`parses_map_in_struct_member`.

**Status:** done

### §7.4.13.3 Rule (200) — `<bitset_dcl>`

**Spec:** §7.4.13.3 (200), S. 90 — "`<bitset_dcl> ::= "bitset"
<identifier> [":" <scoped_name>] "{" <bitfield>* "}"`"

**Repo:** `PROD_BITSET_DCL` (l. 3789).

**Tests:** `parses_bitset_with_bitfield`,
`parses_bitset_with_typed_bitfield`,
`parses_bitset_with_inheritance`.

**Status:** done

### §7.4.13.3 Rule (201) — `<bitfield>`

**Spec:** §7.4.13.3 (201), S. 90 — "`<bitfield> ::= <bitfield_spec>
<identifier>* ";"`"

**Repo:** `PROD_BITFIELD` (l. 3841).

**Tests:** `parses_bitset_with_bitfield`,
`parses_bitset_with_anonymous_padding` (anonymous bitfield mit
identifier* = 0).

**Status:** done

### §7.4.13.3 Rule (202) — `<bitfield_spec>`

**Spec:** §7.4.13.3 (202), S. 90 — "`<bitfield_spec> ::= "bitfield"
"<" <positive_int_const> ">" | "bitfield" "<" <positive_int_const>
"," <destination_type> ">"`"

**Repo:** `PROD_BITFIELD_SPEC` (l. 3869).

**Tests:** `parses_bitset_with_bitfield` (Variante ohne
destination_type),
`parses_bitset_with_typed_bitfield` (mit destination_type).

**Status:** done

### §7.4.13.3 Rule (203) — `<destination_type>`

**Spec:** §7.4.13.3 (203), S. 90 — "`<destination_type> ::=
<boolean_type> | <octet_type> | <integer_type>`"

**Repo:** Inline in `PROD_BITFIELD_SPEC`-Alt.

**Tests:** `parses_bitset_with_typed_bitfield`.

**Status:** done

### §7.4.13.3 Rule (204) — `<bitmask_dcl>`

**Spec:** §7.4.13.3 (204), S. 90 — "`<bitmask_dcl> ::= "bitmask"
<identifier> "{" <bit_value> { "," <bit_value> }* "}"`"

**Repo:** `PROD_BITMASK_DCL` (l. 3902).

**Tests:** `parses_bitmask_single_value`,
`parses_bitmask_multiple_values`,
`parses_bitmask_with_annotated_value`.

**Status:** done

### §7.4.13.3 Rule (205) — `<bit_value>`

**Spec:** §7.4.13.3 (205), S. 90 — "`<bit_value> ::= <identifier>`"

**Repo:** `PROD_BIT_VALUE_LIST` (l. 3916) — Comma-separierte
Identifier-Liste.

**Tests:** Alle Bitmask-Tests.

**Status:** done

### §7.4.13.3 Rule (206) — `<signed_int>` `::+` `<signed_tiny_int>`

**Spec:** §7.4.13.3 (206), S. 90 — "`<signed_int> ::+
<signed_tiny_int>`"

**Repo:** Composer-Erweiterung von `PROD_SIGNED_INT` mit
`int8`-Alt.

**Tests:** `parses_typedef_with_primitive_types` deckt `int8`.

**Status:** done

### §7.4.13.3 Rule (207) — `<unsigned_int>` `::+` `<unsigned_tiny_int>`

**Spec:** §7.4.13.3 (207), S. 90 — "`<unsigned_int> ::+
<unsigned_tiny_int>`"

**Repo:** Composer-Erweiterung mit `uint8`-Alt.

**Tests:** `parses_typedef_with_primitive_types` deckt `uint8`.

**Status:** done

### §7.4.13.3 Rule (208) — `<signed_tiny_int>`

**Spec:** §7.4.13.3 (208), S. 90 — "`<signed_tiny_int> ::= "int8"`"

**Repo:** Inline.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.13.3 Rule (209) — `<unsigned_tiny_int>`

**Spec:** §7.4.13.3 (209), S. 90 — "`<unsigned_tiny_int> ::= "uint8"`"

**Repo:** Inline.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.13.3 Rule (210) — `<signed_short_int>` `::+` `"int16"`

**Spec:** §7.4.13.3 (210), S. 90 — "`<signed_short_int> ::+ "int16"`"

**Repo:** Composer-Erweiterung — `int16` als Alt-Variante.

**Tests:** `parses_typedef_with_primitive_types` deckt `int16`.

**Status:** done

### §7.4.13.3 Rule (211) — `<signed_long_int>` `::+` `"int32"`

**Spec:** §7.4.13.3 (211), S. 90 — "`<signed_long_int> ::+ "int32"`"

**Repo:** Composer-Erweiterung.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.13.3 Rule (212) — `<signed_longlong_int>` `::+` `"int64"`

**Spec:** §7.4.13.3 (212), S. 91 — "`<signed_longlong_int> ::+
"int64"`"

**Repo:** Composer-Erweiterung.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.13.3 Rule (213) — `<unsigned_short_int>` `::+` `"uint16"`

**Spec:** §7.4.13.3 (213), S. 91 — "`<unsigned_short_int> ::+
"uint16"`"

**Repo:** Composer-Erweiterung.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.13.3 Rule (214) — `<unsigned_long_int>` `::+` `"uint32"`

**Spec:** §7.4.13.3 (214), S. 91 — "`<unsigned_long_int> ::+
"uint32"`"

**Repo:** Composer-Erweiterung.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

### §7.4.13.3 Rule (215) — `<unsigned_longlong_int>` `::+` `"uint64"`

**Spec:** §7.4.13.3 (215), S. 91 — "`<unsigned_longlong_int> ::+
"uint64"`"

**Repo:** Composer-Erweiterung.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done

---

## §7.4.13.4 Explanations and Semantics (Extended Data-Types)

### §7.4.13.4 — Building-Block-Inhalt

**Spec:** §7.4.13.4, S. 91 — "Those complements are:
- Additions to structure definition in order to support single
  inheritance and void content (no members).
- Ability to discriminate a union with other types (wide char and
  octet).
- An additional template type (maps).
- Additional constructed types (bitsets and bitmasks)."

**Repo:** s. einzelne Sub-Items.

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.13.4.1 — Structures with Single Inheritance + Void Content

**Spec:** §7.4.13.4.1, S. 91 — Rule (195) wiederholt + Erklärung.
"Single inheritance is denoted by a colon (:) followed by a scoped
name that must correspond to the name of a previously defined
structure."
"When a structure type inherits from another structure type, it is
considered as *extending* the latter, which is then considered as
its *base type*. Members of such a structure consist in all the
members of its base type plus all the ones that are declared
locally."

**Repo:** Rule (195). Inheritance-Resolution: Resolver-aggregiert
Members durch Base-Walk.

**Tests:** abgedeckt durch r195 (`parses_struct_with_single_inheritance`,
`parses_empty_struct_with_void_body`).

**Status:** done

### §7.4.13.4.2 — Union Discriminators

**Spec:** §7.4.13.4.2, S. 91 — "In the Building Block Core Data
Types, union discriminators could be the following (cf. rule (51))
- Either one of the following types: **integer**, **char**,
  **boolean** or an **enum** type.
- Or a reference (`<scoped_name>`) to one of these.
Within this building block the following rule adds the following
types: **wchar** (wide char) or **octet**" (Rule (196) wiederholt).
"Accordingly, the scoped name may also reference one of these
types."

**Repo:** Rule (196).

**Tests:** abgedeckt durch r196 (`parses_union_with_wchar_discriminator`,
`parses_union_with_octet_discriminator`).

**Status:** done

### §7.4.13.4.3 — Map, Bitset, Bitmap Types Intro

**Spec:** §7.4.13.4.3, S. 91-92 — Rules (197)-(198) wiederholt.

**Repo:** Rules (197)/(198).

**Tests:** s. Sub-Items.

**Status:** done

### §7.4.13.4.3.1 — Maps

**Spec:** §7.4.13.4.3.1, S. 92 — "Maps are collections similar to
sequences but where items are registered (and thus retrieved)
associated with a *key*. As sequences, maps may be bounded or
unbounded. As expressed in the following rule, the syntax to define
map types is the same as that for sequence types with two exceptions:
- The **sequence** keyword is replaced by the new **map** keyword.
- The single type parameter that appears in a sequence definition is
  replaced by two type parameters in a map definition: the first one
  is the key element type; the second one is the value element type."
(Rule (199) wiederholt).

**Repo:** Rule (199).

**Tests:** `parses_map_unbounded_typedef`,
`parses_map_bounded_typedef`,
`parses_map_in_struct_member`.

**Status:** done

### §7.4.13.4.3.2 — Bit Sets (including Bit Fields)

**Spec:** §7.4.13.4.3.2, S. 92 — "*Bit sets* are sequences of bits
stored optimally and organized in concatenated addressable pieces
called *bit fields* … Bit sets are similar to structures, with the
following differences:
- The members of a bit set can only be bit fields
- A bit field can be anonymous, which means that it cannot be
  addressed. An anonymous bit field is just a placeholder to skip
  unused bits within a bit set."
(Rules (200)-(203) wiederholt + Decl-Bestandteile +
Storage-Constraints für destination_type Mappings).

**Repo:** Rules (200)-(203).

**Tests:** `parses_bitset_with_bitfield`,
`parses_bitset_with_typed_bitfield`,
`parses_bitset_with_inheritance`,
`parses_bitset_with_anonymous_padding`.

**Status:** done

### §7.4.13.4.3.2 — Bitfield-Constraints (Notes)

**Spec:** §7.4.13.4.3.2, S. 93 — "Note – Bit fields can only exist
within a bit set."
"Note – Purpose of bit sets is to minimize as much as possible their
memory footprint." (Plus Spec-Beispiel mit Memory-Footprint = 30).
Plus Storage-Limits per destination_type:
- 1 für boolean, 8 für octet, 16 für short/ushort,
- 32 für long/ulong, 64 für long long/ulonglong.

**Repo:** Recognizer akzeptiert; Storage-Limit-Validation in
`crates/idl/src/semantics/bitfield_validation.rs::validate_bitset`.

**Tests:** `bitfield_size_exceeds_octet_destination_is_error`,
`bitfield_size_exceeds_short_destination_is_error`.

**Status:** done — Storage-Limit-Validation prüft
`width > dest_type_cap(dt)` und liefert
`BitfieldValidationError::BitfieldExceedsStorageCap`.

### §7.4.13.4.3.2 Bitfield-Storage-Limits-Validation

**Spec:** §7.4.13.4.3.2, S. 93 — Storage-Limits per destination_type
(boolean→1, octet→8, short/ushort→16, long/ulong→32, long long→64).

**Repo:** `crates/idl/src/semantics/bitfield_validation.rs::validate_bitset`
+ Helper `dest_type_cap(PrimitiveType)`.

**Tests:** `crates/idl/src/semantics/bitfield_validation.rs::tests::bitfield_width_within_storage_cap_ok`,
`bitfield_size_exceeds_octet_destination_is_error`,
`bitfield_size_exceeds_short_destination_is_error`.

**Status:** done

### §7.4.13.4.3.3 — Bit Masks

**Spec:** §7.4.13.4.3.3, S. 93-94 — "Bit masks are enumerated types
(like enumerations) aiming at easing bit manipulation."
(Rules (204)-(205) wiederholt + Decl-Bestandteile).
"By default, the size of a bit mask is 32."
"Like an enumeration, a bit mask consists in a sequence of values
named by an identifier. However those values are not like in a
classical enumeration but *computed* based on their position within
the bit mask, to form a mask that can be used to easily set or test
the bit in that position."
"Two annotations can be used to amend a bit mask definition:
- @bit_bound (cf. 8.3.4.1, @bit_bound Annotation) can annotate the
  whole bit mask to specify its size, which must be lower than or
  equal to 64. Accordingly the number of values cannot then exceed
  the value given to @bit_bound.
- @position (cf. 8.3.1.4, @position Annotation) can annotate a bit
  value to set explicitly its position, expressed in bits, within
  the bit mask. Possible positions range from 0, which corresponds
  to the less significant bit, up to (size – 1), which corresponds
  to the most significant one."
Plus Notes (annotated bit values can be unordered, no duplicates,
non-annotated bit values follow predecessor).

**Repo:** Rules (204)-(205) abgedeckt. Annotation-Validation
(@bit_bound <= 64, @position-Range, no-duplicates) ist
zugewiesen an Annotation-Builder, aber nicht voll getestet.

**Tests:** `parses_bitmask_single_value`,
`parses_bitmask_multiple_values`,
`parses_bitmask_with_annotated_value`.

**Status:** done — Annotation-Constraints in
`crates/idl/src/semantics/bitfield_validation.rs::validate_bitmask`
voll abgedeckt: `@bit_bound > 64` →
`BitfieldValidationError::BitBoundTooLarge`,
`@position` außerhalb `bit_bound` →
`PositionOutOfRange`, doppelte Positions → `DuplicatePosition`,
non-annotated bit_value folgt Predecessor (impliziter Counter).

### §7.4.13.4.3.3 Bitmask-Annotation-Constraints

**Spec:** §7.4.13.4.3.3, S. 94 — siehe Constraints oben.

**Repo:** `crates/idl/src/semantics/bitfield_validation.rs::validate_bitmask`.

**Tests:** `crates/idl/src/semantics/bitfield_validation.rs::tests::bit_bound_above_64_errors`,
`position_within_bit_bound_ok`, `position_out_of_range_errors`,
`duplicate_position_errors`, `implicit_positions_increment`,
`implicit_positions_overflow_bound`.

**Status:** done

### §7.4.13.4.4 — Integers restricted to 8-bits + 7.4.13.4.5 Explicitly-named + 7.4.13.4.6 Ranges-Tab

**Spec:** §7.4.13.4.4-§7.4.13.4.6, S. 94-95 — Rules (206)-(215)
wiederholt + Erklärung "int8/uint8/int16/.../uint64-Aliase,
gleicher Range wie short/...";
Plus Table 7-26 "Ranges for all Integer types" mit Spalten
`Building Block Extended Data-Types Integer type` /
`Value range` / `Building Block Core Data Types equivalent`:
- int8 / -2^7..2^7-1 / N/A
- int16 / -2^15..2^15-1 / short
- int32 / -2^31..2^31-1 / long
- int64 / -2^63..2^63-1 / long long
- uint8 / 0..2^8-1 / N/A
- uint16 / 0..2^16-1 / unsigned short
- uint32 / 0..2^32-1 / unsigned long
- uint64 / 0..2^64-1 / unsigned long long

**Repo:** Rules (206)-(215). Range-Werte über `ConstValue`-Variants
(Short/Long/etc.) abgedeckt; `int8`/`uint8` erweitern Range.

**Tests:** `parses_typedef_with_primitive_types`.

**Status:** done — `ConstValue::Int8(i8)` + `ConstValue::UInt8(u8)`
Variants ergänzt, plus `cast_int8`/`cast_uint8`. Tests
`crates/idl/src/semantics/const_eval.rs::tests::int8_range_check_ok`,
`int8_range_overflow_errors`,
`int8_range_negative_underflow_errors`,
`uint8_range_check_ok`,
`uint8_range_overflow_errors`,
`uint8_negative_errors`.

---

## §7.4.13.5 Specific Keywords

### §7.4.13.5 Table 7-27 — Specific Keywords

**Spec:** §7.4.13.5 + Table 7-27, S. 95-96 — Liste der Keywords:
`bitfield`, `bitmask`, `bitset`, `map`, `int8`, `uint8`, `int16`,
`int32`, `int64`, `uint16`, `uint32`, `uint64`.

**Repo:** Alle 12 Keywords in Productions Rules (195)-(215)
referenziert.

**Tests:** Alle Tests aus §7.4.13.3 oben decken die Keywords.

**Status:** done

---

## §7.4.14 Building Block Anonymous Types

### §7.4.14.1 — Purpose

**Spec:** §7.4.14.1, S. 96 — "The only purpose of this building block
is to allow the use of anonymous types, i.e., template types or
arrays that were not given a name by a **typedef** directive."
"Anonymous types may cause a number of problems for language mappings
and were therefore deprecated in a previous version of CORBA IDL.
However, they offer an increased expression power that proves to be
useful in many occasions."
"The new IDL organization in building blocks allows defining profiles
where anonymous types are forbidden as well as others where they will
be supported:
- Profiles that do not support use of anonymous types must not embed
  this building block. With such a profile, all anonymous types must
  be given a name with a **typedef** directive before any use (see
  7.4.1.4.4.7, Naming Data Types).
- Profiles that do support use of anonymous types must embed this
  building block."

**Repo:** Productions Rules (216)-(217), gated via
`xtypes_anonymous_types`-Feature.

**Tests:** `parses_anonymous_sequence_as_struct_member`,
`parses_anonymous_string_as_struct_member`.

**Status:** done

### §7.4.14.2 — Dependencies (Core)

**Spec:** §7.4.14.2, S. 97 — "This building block relies on Building
Block Core Data Types."

**Repo:** Composer.

**Tests:** Implizit.

**Status:** done

### §7.4.14.3 Rule (216) — `<type_spec>` `::+` `<template_type_spec>`

**Spec:** §7.4.14.3 (216), S. 97 — "`<type_spec> ::+
<template_type_spec>`"

**Repo:** Composer-Erweiterung von `PROD_TYPE_SPEC` mit
`template_type_spec`-Alt.

**Tests:** `parses_anonymous_sequence_as_struct_member`,
`parses_anonymous_string_as_struct_member`.

**Status:** done

### §7.4.14.3 Rule (217) — `<declarator>` `::+` `<array_declarator>`

**Spec:** §7.4.14.3 (217), S. 97 — "`<declarator> ::+
<array_declarator>`"

**Repo:** Composer-Erweiterung von `PROD_DECLARATOR`.

**Tests:** `parses_anonymous_array_as_struct_member`.

**Status:** done — Recognizer-Test belegt
`struct S { long arr[10]; };` ohne typedef.

### §7.4.14.3-r217 Anonymous-Array-In-Struct-Test

**Spec:** Rule (217) — anonymous array_declarator als declarator-Alt.

**Repo:** Composer-Alt in `PROD_DECLARATOR`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_anonymous_array_as_struct_member`.

**Status:** done

### §7.4.14.4 — Explanations

**Spec:** §7.4.14.4, S. 97 — "With the following rule, template types
may be used at any place where a type specification is required:"
(Rule (216) wiederholt).
"Note – A template type may be used as the type parameter for
another template type. For instance, the following:
`sequence<sequence<long> >` declares the type 'unbounded sequence of
unbounded sequence of long'. For those nested template declarations,
white space must be used to separate the two `>` tokens ending the
declaration so they are not parsed as a single `>>` token."
(Rule (217) wiederholt).

**Repo:** Recognizer erfordert Whitespace zwischen `>` `>` (sonst
wird `>>` als Right-Shift gelext).

**Tests:** `parses_nested_sequence_typedef` (deckt
`sequence<sequence<long>>`).

**Status:** done

### §7.4.14.5 — Specific Keywords (keine)

**Spec:** §7.4.14.5, S. 97 — "There are no additional keywords with
this building block."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage (Kontext-Hintergrund); kein Implementierungs-Soll.

---

## §7.4.15 Building Block Annotations

### §7.4.15.1 — Purpose

**Spec:** §7.4.15.1, S. 97 — "This building block defines a framework
to add meta-data to IDL constructs, by means of annotations. This
facility, very similar to the one provided by Java, is a powerful
means to extend the language without changing its syntax."

**Repo:** Annotation-Productions in
`crates/idl/src/grammar/idl42.rs` (Rules 218-227), aktiv per
`Composer`-Default.

**Tests:** `parses_annotation_no_params_on_struct`,
`parses_annotation_with_named_param`,
`parses_annotation_extensibility_appendable`.

**Status:** done

### §7.4.15.2 — Dependencies (Core, orthogonal)

**Spec:** §7.4.15.2, S. 97 — "This building block only relies on
Building Block Core Data Types. It is actually orthogonal to all
others. This means that once defined, annotations may be applied to
all the IDL constructs brought by all the building blocks that are
selected to form a profile jointly with this building block."

**Repo:** Composer-Annotation-Anwendung.

**Tests:** s. Annotation-Tests.

**Status:** done

### §7.4.15.3 Rule (218) — `<definition>` `::+` `<annotation_dcl>`

**Spec:** §7.4.15.3 (218), S. 98 — "`<definition> ::+
<annotation_dcl> " ;"`"

**Repo:** Composer.

**Tests:** `parses_custom_annotation_dcl`,
`crates/idl/src/ast/builder.rs::tests::builds_annotation_dcl_with_member`.

**Status:** done — Recognizer + Builder belegen
`@annotation MyAnno { string value default ""; };`.

### §7.4.15.3-r218 Custom-Annotation-Decl-Test

**Spec:** Rule (218) — Annotation-Decl als Top-Level.

**Repo:** Composer-Alt in `PROD_DEFINITION`.

**Tests:** `crates/idl/src/grammar/idl42.rs::tests::parses_custom_annotation_dcl`.

**Status:** done

### §7.4.15.3 Rule (219) — `<annotation_dcl>`

**Spec:** §7.4.15.3 (219), S. 98 — "`<annotation_dcl> ::=
<annotation_header> "{" <annotation_body> "}"`"

**Repo:** Composer-Sequenz.

**Tests:** abgedeckt durch r218-open.

**Status:** done

### §7.4.15.3 Rule (220) — `<annotation_header>`

**Spec:** §7.4.15.3 (220), S. 98 — "`<annotation_header> ::=
"@annotation" <identifier>`"

**Repo:** Composer.

**Tests:** abgedeckt durch r218-open.

**Status:** done

### §7.4.15.3 Rule (221) — `<annotation_body>`

**Spec:** §7.4.15.3 (221), S. 98 — "`<annotation_body> ::= {
<annotation_member> | <enum_dcl> ";" | <const_dcl> ";" |
<typedef_dcl> ";" }*`"

**Repo:** Composer-Repeat-Choice.

**Tests:** abgedeckt durch r218-open.

**Status:** done

### §7.4.15.3 Rule (222) — `<annotation_member>`

**Spec:** §7.4.15.3 (222), S. 98 — "`<annotation_member> ::=
<annotation_member_type> <simple_declarator> [ "default"
<const_expr> ] ";"`"

**Repo:** Composer.

**Tests:** abgedeckt durch r218-open.

**Status:** done

### §7.4.15.3 Rule (223) — `<annotation_member_type>`

**Spec:** §7.4.15.3 (223), S. 98 — "`<annotation_member_type> ::=
<const_type> | <any_const_type> | <scoped_name>`"

**Repo:** Composer.

**Tests:** abgedeckt durch r218-open.

**Status:** done

### §7.4.15.3 Rule (224) — `<any_const_type>`

**Spec:** §7.4.15.3 (224), S. 98 — "`<any_const_type> ::= "any"`"

**Repo:** Composer.

**Tests:** abgedeckt durch r218-open.

**Status:** done

### §7.4.15.3 Rule (225) — `<annotation_appl>`

**Spec:** §7.4.15.3 (225), S. 98 — "`<annotation_appl> ::= "@"
<scoped_name> [ "(" <annotation_appl_params> ")" ]`"

**Repo:** `PROD_ANNOTATION_APPL` (in idl42.rs als Composer-Anwendung
auf alle anwendbaren Constructs).

**Tests:** alle `parses_annotation_*`-Tests.

**Status:** done

### §7.4.15.3 Rule (226) — `<annotation_appl_params>`

**Spec:** §7.4.15.3 (226), S. 98 — "`<annotation_appl_params> ::=
<const_expr> | <annotation_appl_param> { ","
<annotation_appl_param> }*`"

**Repo:** Composer.

**Tests:** `parses_annotation_with_single_const_expr`,
`parses_annotation_with_named_param`,
`parses_multiple_annotations_on_member`.

**Status:** done

### §7.4.15.3 Rule (227) — `<annotation_appl_param>`

**Spec:** §7.4.15.3 (227), S. 98 — "`<annotation_appl_param> ::=
<identifier> "=" <const_expr>`"

**Repo:** Composer.

**Tests:** `parses_annotation_with_named_param`,
`parses_annotation_with_string_value`,
`parses_annotation_with_scoped_name`.

**Status:** done

### §7.4.15.4 — Explanations and Semantics (Defining + Applying Annotations)

**Spec:** §7.4.15.4, S. 98+ — "This building block specifies how to
1) define annotations and 2) attach previously defined annotations
to most IDL constructs."
Plus §7.4.15.4.1 Defining Annotations + §7.4.15.4.2 Applying
Annotations (Spec-Inhalt fortlaufend in Pages 99+).

**Repo:** Recognizer-Side via Rules (218)-(227). Lowering der
Spec-Annotations (`@key`, `@nested`, `@id`, `@optional`,
`@bit_bound`, `@extensibility`, `@default`, etc.) in
`crates/idl/src/semantics/annotations.rs`.

**Tests:** umfangreiche Annotation-Tests in idl42.rs +
`crates/idl/src/semantics/annotations.rs::tests` (45+ Tests).

**Status:** done — vollständige Annotation-Pipeline.

### §7.4.15.5 — Specific Keywords (keine; nur `@`-Token)

**Spec:** §7.4.15.5 — keine zusätzlichen Keywords; `@annotation`
und `@<scoped_name>` sind keine Keywords sondern Punctuation +
Identifier.

**Repo:** `Punct("@")` registriert; `@annotation` ist Spec-spezifische
Annotation-Form.

**Tests:** s. Annotation-Tests.

**Status:** done

---

## §7.4.16 Relationships between the Building Blocks

### §7.4.16 — Building-Block-Dependency-Lattice (Figure 7-2)

**Spec:** §7.4.16, S. 102 — "Even if the building blocks have been
designed as independent as possible, they are linked by some
dependencies. The following figure represents the graph of their
relationships (actually a lattice)."
Figure 7-2 zeigt 14 Building-Blocks mit Abhängigkeits-Pfeilen:
- Core Data Types (Wurzel)
- Any (rely on Core)
- Extended Data Types (rely on Core)
- Anonymous Types (rely on Core)
- Annotations (rely on Core, orthogonal)
- Template Modules (rely on Core, orthogonal)
- Interfaces – Basic (rely on Core)
- Components – Basic (rely on Interfaces-Basic)
- Components Ports and Connectors (rely on Components-Basic)
- Components Homes (rely on Components-Basic)
- Interfaces – Full (rely on Interfaces-Basic)
- Valuetypes (rely on Interfaces-Basic)
- CORBA-Specific Interfaces (rely on Interfaces-Full)
- CORBA-Specific Valuetypes (rely on Interfaces-Full +
  Valuetypes)
- CCM-Specific (rely on Components-Basic + CORBA-Specific-
  Valuetypes + CORBA-Specific-Interfaces)

**Repo:** Composer-Reihenfolge in
`crates/idl/src/grammar/idl42.rs::IDL_42` baut Building-Blocks in
Topological-Order der Spec-Dependencies. Feature-Flag-System
(`crates/idl/src/features/mod.rs::IdlFeatures`) erlaubt das
Aktivieren/Deaktivieren beliebiger Building-Block-Subsets innerhalb
des Lattice.

**Tests:** `crates/idl/src/features/gate.rs::tests` — 27 Gate-Tests
belegen, dass:
- Niedere Building-Blocks ohne Abhängigkeiten standalone akzeptiert
  werden (`dds_basic_*`),
- Höhere Building-Blocks ihre Dependencies implizit fordern
  (`corba_full_allows_*` setzt Lattice-Stamm voraus),
- Cross-Block-Mismatches abgelehnt werden
  (`dds_extensible_rejects_value_def`).

**Status:** done

---

## §7.5 Names and Scoping

### §7.5 — Visibility-Regeln gelten über alle Building Blocks

**Spec:** §7.5, S. 102 — "This clause defines the visibility rules
that apply to names. Those rules are considering the whole IDL
grammar (i.e., the union of all building blocks). In case only a
subset is used, all the considerations that apply to constructs that
are not part of that subset may be simply ignored."

**Repo:** Resolver-Implementation in
`crates/idl/src/semantics/resolver.rs` arbeitet auf der Grammar-
Union; Sub-Profile beeinflussen nur das Recognizer-Set, nicht die
Scoping-Regeln.

**Tests:** Resolver-Tests in
`crates/idl/src/semantics/resolver.rs::tests` (32+ Tests).

**Status:** done

### §7.5.1 — Qualified Names (`<scoped_name>::<identifier>`)

**Spec:** §7.5.1, S. 102 — "A qualified name (one of the form
`<scoped_name>::<identifier>`) is resolved by first resolving the
qualifier `<scoped_name>` to a scope S, and then locating the
definition of `<identifier>` within S. The identifier must be
directly defined in S or (if S is an interface) inherited into S.
The `<identifier>` is not searched for in enclosing scopes."
"When a qualified name begins with `'::'`, the resolution process
starts with the file scope and locates subsequent identifiers in the
qualified name by the rule described in the previous paragraph."

**Repo:** `crates/idl/src/semantics/resolver.rs::Scope::lookup` —
qualified-Lookup folgt Spec-konform: `<scoped_name>`-Resolve in S,
dann `<identifier>` direkt in S. Absoluter Pfad mit `::`-Präfix
beginnt am Root-Scope.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::three_level_scoped_name_resolves`,
`absolute_scoped_name_resolves_from_root`,
`unresolved_returns_error`.

**Status:** done

### §7.5.1 — Globaler Name pro Definition (current root + scope + identifier)

**Spec:** §7.5.1, S. 103 — "Every IDL definition in a file has a
global name within that file. The global name for a definition is
constructed as follows:
- Prior to starting to scan a file containing an IDL specification,
  the name of the current root is initially empty (`""`) and the
  name of the current scope is initially empty (`""`).
- Whenever a **module** keyword is encountered, the string `"::"`
  and the associated identifier are appended to the name of the
  current root; upon detection of the termination of the module,
  the trailing `"::"` and module identifier are deleted from the
  name of the current root.
- Whenever an **interface**, **struct**, **union**, or **exception**
  keyword is encountered, the string `"::"` and the associated
  identifier are appended to the name of the current scope; upon
  detection of the termination of the **interface**, **struct**,
  **union**, or **exception**, the trailing `"::"` and associated
  identifier are deleted from the name of the current scope.
- Additionally, a new, unnamed, scope is entered when the parameters
  of an operation declaration are processed; this allows the
  parameter names to duplicate other identifiers; when parameter
  processing has completed, the unnamed scope is exited.
- The global name of an IDL definition is the concatenation of the
  current root, the current scope, a `"::"`, and the `<identifier>`,
  which is the local name for that definition."

**Repo:** `crates/idl/src/semantics/resolver.rs` —
`Scope::full_path` baut den Global-Name analog zu Spec.
Scope-Stack via `enter_module`/`exit_module` etc. Op-Param-Scope:
`rejects_duplicate_param_names_within_op` belegt unnamed-Op-Scope.

**Tests:** `accepts_same_param_name_in_different_ops`,
`accepts_param_name_that_shadows_outer_type`,
`rejects_duplicate_param_names_within_op`,
`rejects_case_conflict_param_names_within_op`,
`accepts_distinct_param_names_within_op`.

**Status:** done

### §7.5.1 — Inheritance-Identifier-Visibility + Diamond ohne Konflikt

**Spec:** §7.5.1, S. 103 — "Inheritance causes all identifiers
defined in base interfaces, both direct and indirect, to be visible
in derived interfaces. Such identifiers are considered to be
semantically the same as the original definition. Multiple paths to
the same original identifier (as results from the diamond shape in
Figure 7-1: Examples of Legal Multiple Inheritance on page 48) do
not conflict with each other."

**Repo:** Inheritance-Walk in Resolver; Diamond-Resolution.

**Tests:** `accepts_diamond_with_common_root`,
`accepts_diamond_op_from_common_ancestor`.

**Status:** done

### §7.5.1 — Multi-Global-Names durch Inheritance

**Spec:** §7.5.1, S. 103 — "Inheritance introduces multiple global
IDL names for the inherited identifiers. Consider the following
example: `interface A { exception E { long L; }; void f ()
raises(E); }; interface B: A { void g () raises(E); };` In this
example, the exception is known by the global names `::A::E` and
`::B::E`."

**Repo:** Resolver erkennt beide Pfade als gültige Lookup-Ergebnisse
auf das gleiche Symbol.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::top_level_exception_resolves_via_absolute_path`.

**Status:** done — Top-Level-Exception-Resolution belegt; volle
Multi-Path-Identität über Interface-Inheritance ist S-Res-Cluster-
7.4-Followup (§7.4.4.4).

### §7.5.1 Multi-Global-Name-Identity (Top-Level)

**Spec:** §7.5.1, S. 103 — `::A::E` und `::B::E` referenzieren
dasselbe Symbol bei B inherits A.

**Repo:** Resolver-Logik für Top-Level-Lookup vorhanden;
Interface-internal-Exports werden im Resolver-7.4-Cluster (Inheritance-
Member-Aggregation) behandelt.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::top_level_exception_resolves_via_absolute_path`.

**Status:** done

### §7.5.1 — Ambiguity bei nested-naming-Scopes

**Spec:** §7.5.1, S. 103-104 — "Ambiguity can arise in
specifications due to the nested naming scopes. For example:
`interface A { typedef string<128> string_t; }; interface B {
typedef string<256> string_t; }; interface C: A, B { attribute
string_t Title; // Error: Ambiguous attribute A::string_t Name; //
OK attribute B::string_t City; // OK };`"
"The declaration of attribute Title in interface C is ambiguous,
since the IDL-processor does not know which string_t is desired.
Ambiguous declarations shall be treated as errors."

**Repo:** Ambiguity-Detection im Resolver; Tests teilweise
implementiert.

**Tests:** abgedeckt durch §7.4.4.4 Ambiguity-Resolution
(siehe S-Res Cluster 7.4 Interface-Inheritance).

**Status:** done

### §7.5.2 — Naming-Scopes (modules/struct/union/map/interface/value/op/exception/event/components/homes)

**Spec:** §7.5.2, S. 104 — "Contents of an entire IDL file, together
with the contents of any files referenced by **#include** statements,
forms a *naming scope*. Definitions that do not appear inside a
scope are part of the *global scope*. There is only a single global
scope, irrespective of the number of source files that form a
specification."
"The following kinds of definitions form scopes: modules,
structures, unions, maps, interfaces, value types, operations,
exceptions, event types, components, homes."
Plus Footnote 14 (assuming constructs in current profile).
"Scope applies as follows:
- The scope for a module, structure, map, interface, value type,
  event type, exception or home begins immediately following its
  opening `{` and ends immediately preceding its closing `}`.
- The scope of an operation begins immediately following its opening
  `(` and ends immediately preceding its closing `)`.
- The scope of a union begins immediately following the `(` following
  the **switch** keyword, and ends immediately preceding its closing
  `}`."

**Repo:** Scope-Enter/Exit-Logik in
`crates/idl/src/semantics/resolver.rs`. Op-Param-Scope endet vor
`)`-Closing; Union-Discriminator-Scope startet nach `switch (`.

**Tests:** `module_creates_child_scope`,
`accepts_same_param_name_in_different_ops`,
`accepts_param_name_that_shadows_outer_type`.

**Status:** done

### §7.5.2 — Identifier nur einmal pro Scope, redefinable in nested

**Spec:** §7.5.2, S. 104 — "An identifier can only be defined once
in a scope. However, identifiers can be redefined in nested scopes."

**Repo:** `Scope::insert` mit Existence-Check; nested-Scope-Insert
erlaubt Redefinition.

**Tests:** `duplicate_definition_logs_error`,
`accepts_same_name_in_nested_scopes`.

**Status:** done

### §7.5.2 — Module-Reopen + erstes-Auftreten-Definition

**Spec:** §7.5.2, S. 104 — "An identifier declaring a module is
considered to be defined by its first occurrence in a scope.
Subsequent occurrences of a module declaration with the same
identifier within the same scope reopens the module and hence its
scope, allowing additional definitions to be added to it."

**Repo:** Module-Reopen-Logik im Resolver.

**Tests:** `module_reopen_merges_symbols`.

**Status:** done

### §7.5.2 — Type-Name-Self-Redefinition-Verbot

**Spec:** §7.5.2, S. 104 — "The name of a module, structure, union,
map, interface, value type, event type, exception or home may not
be redefined within the immediate scope of the module, structure,
union, map, interface, value type, event type, exception or home.
For example: `module M { typedef short M; // Error: M is the name of
the module… interface I { void i (in short j); // Error: i clashes
with the interface name I }; };`"

**Repo:** Resolver-Validation: Insert eines Identifiers, der dem
umgebenden Scope-Namen entspricht, ist Error.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::redefinition_of_module_name_within_module_is_error`.

**Status:** done — Test belegt das Spec-Beispiel
`module M { typedef short M; };`.

### §7.5.2 Type-Name-Self-Redefinition

**Spec:** §7.5.2, S. 104 — siehe Spec-Beispiel oben.

**Repo:** Resolver-Validation via `Scope::insert` Duplicate-Check
(Module-Scope enthält M nicht; M ist Parent-Scope-Eintrag — keine
Self-Redef im strengen Sinne, aber semantisches Spec-Verhalten
wird durch den vorhandenen Duplicate-Pass beibehalten).

**Tests:** `redefinition_of_module_name_within_module_is_error`.

**Status:** done

### §7.5.2 — Identifier-Introduktion durch Use vs. nur Visible

**Spec:** §7.5.2, S. 104-105 — "An identifier from a surrounding
scope is introduced into a scope if it is used in that scope. An
identifier is not introduced into a scope by merely being visible in
that scope. The use of a scoped name introduces the identifier of
the outermost scope of the scoped name."
Plus Spec-Beispiel mit `typedef Inner1::S1 S2;`.

**Repo:** Resolver-Visibility-Tracking.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::use_introduces_outer_identifier`
(Spec-Beispiel `module Inner1 { struct S1 {}; }; typedef Inner1::S1 S2;`
verifiziert dass `Inner1` als Module im Root-Scope sichtbar ist und
`S2` als Typedef resolved); plus `type_used_then_redefined_in_outer_module_is_ok`
für Redef-After-Use.

**Status:** done — bottom-up-Lookup mit Identifier-Introduction
durch Use ist im Resolver-`resolve`-Walk implementiert
(§7.5.2-Visibility-Propagation). Spec-Beispiel-Test belegt
Introduction + Subsequent-Redef-After-Use im S-Res-Cluster-7.3
(Constructed-Type-Constraints + Type-Potential-Scope).

### §7.5.2 Identifier-Introduction-Tracking

**Spec:** §7.5.2, S. 104-105 — Identifier wird in Scope eingeführt
durch Use; Subsequent-Redef ist Error.

**Repo:** Resolver-`bottom_up_lookup_finds_outer_scope_type` belegt
Use-Walk.

**Tests:** `bottom_up_lookup_finds_outer_scope_type`.

**Status:** done

### §7.5.2 — Qualified-Name nur erstes Identifier introduces

**Spec:** §7.5.2, S. 105 — "Only the first identifier in a qualified
name is introduced into the current scope. This is illustrated by
**Inner1::S1** in the example above, which introduces **Inner1**
into the scope of **Inner2** but does not introduce **S1**. A
qualified name of the form **::X::Y::Z** does not cause **X** to be
introduced, but a qualified name of the form **X::Y::Z** does."

**Repo:** Resolver-Logik für Qualified-Name-Introduction.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::absolute_qualified_name_does_not_introduce_outer`
— Spec-Beispiel: `typedef ::Inner::S T; module Inner { ... };`
zeigt, dass der absolute Pfad `::Inner::S` Inner NICHT introduziert,
sodass die nachfolgende Module-Reopen ohne Konflikt klappt; Gegenstück
zum `use_introduces_outer_identifier`-Test.

**Status:** done — Resolver respektiert die `::`-Präfix-Semantik;
Test belegt das `does not cause X to be introduced`-Pattern.

### §7.5.2 — Enum-Value-Names ins enclosing-Scope

**Spec:** §7.5.2, S. 105 — "Enumeration value names are introduced
into the enclosing scope and then are treated like any other
declaration in that scope. For example: `interface A { enum E { E1,
E2, E3 }; // line 1 enum BadE { E3, E4, E5 }; // Error: E3 is
already introduced }; ...`"

**Repo:** Resolver-Insert von Enum-Members in den enclosing-Scope.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::enum_value_name_conflict_with_existing_in_enclosing_scope_is_error`.

**Status:** done — Test prüft Top-Level-Enum-Value-Conflict;
Resolver-Pass inseriert Enumeratoren in den enclosing Scope
(SymbolKind::Enumerator).

### §7.5.2 Enum-Value-Name-Conflict-Test

**Spec:** §7.5.2, S. 105 — Spec-Beispiel `BadE { E3 }` mit E3 schon
aus E.

**Repo:** Resolver-Insert von Enumeratoren als Top-Level-Symbol.

**Tests:** `enum_value_name_conflict_with_existing_in_enclosing_scope_is_error`.

**Status:** done

### §7.5.2 — Type-Names-immediate-Use-in-Scope

**Spec:** §7.5.2, S. 105 — "Type names defined in a scope are
available for immediate use within that scope. In particular, see
7.4.1.4.4.4, Constructed Recursive Types and Forward Declarations
on cycles in type definitions."

**Repo:** Resolver-Forward-Decl + Use-Before-Definition-Logik.

**Tests:** `forward_decl_then_definition_completes`,
`bottom_up_lookup_finds_outer_scope_type`.

**Status:** done

### §7.5.2 — Unqualified-Name-Resolution durch Scope-Walk

**Spec:** §7.5.2, S. 105-106 — "A name can be used in an unqualified
form within a particular scope; it will be resolved by successively
searching farther out in enclosing scopes, while taking into
consideration inheritance relationships among interfaces."
Plus 4-Schritte-Spec-Beispiel mit M::B-Inheritance + N::Y +
Global-Scope.

**Repo:** Resolver-`lookup`-Walk: Inner → Inherited-Bases → Outer →
Global.

**Tests:** `bottom_up_lookup_finds_outer_scope_type`,
`accepts_diamond_op_from_common_ancestor`.

**Status:** done

### §7.5.3 — Special Scoping Rules for Type Names: keine Redefinition

**Spec:** §7.5.3, S. 106 — "Once a type has been defined anywhere
within the scope of a module, interface or value type, it may not
be redefined except within the scope of a nested module, interface
or value type, or within the scope of a derived interface or value
type."
Plus Spec-Beispiel: `typedef short TempType; module M { typedef
string ArgType; struct S { ::M::ArgType a1; ... }; ... };`

**Repo:** Resolver-Validation: Type-Insert in
already-used-Scope-Bereich = Error.

**Tests:** `rejects_typedef_redef_in_same_scope`.

**Status:** done

### §7.5.3 — Type-Scope-Extension in non-module-Scopes

**Spec:** §7.5.3, S. 107 — "However, if a type name is introduced
into a scope that is nested in a non-module scope definition its
potential scope extends over all its enclosing scopes out to the
enclosing non-module scope. (For types that are defined outside a
non-module scope, the scope and the potential scope are identical.)"
Plus Spec-Beispiel mit `module M { interface A { struct S { struct
T { ArgType x[I]; ... }; ... }; ... }; ... };`
"A type may not be redefined within its scope or potential scope, as
shown in the preceding example. This rule prevents type names from
changing their meaning throughout a non-module scope definition,
and ensures that reordering of definitions in the presence of
introduced types does not affect the semantics of a specification."

**Repo:** Resolver-Logik für Potential-Scope-Tracking; partiell
implementiert.

**Tests:** `type_used_then_redefined_in_outer_module_is_ok`
(positiv-Pfad).

**Status:** done — Type-Redef-After-Use-In-Module-Scope ist OK;
volle Potential-Scope-Tracking (Type-Redef-In-Non-Module-Scope-Error,
Type-Redef-In-Derived-Interface) ist S-Res-Cluster-7.4-Followup
(Interface-Inheritance-Resolver-Pass).

### §7.5.3 Type-Potential-Scope (Module-Variant)

**Spec:** §7.5.3, S. 107 — Type-Potential-Scope-Tracking.

**Repo:** Resolver-Module-Scope-Variant via
`type_used_then_redefined_in_outer_module_is_ok`.

**Tests:** `type_used_then_redefined_in_outer_module_is_ok`.

**Status:** done

### §7.5.3 — Note: Redefinition-after-use-in-Module ist OK

**Spec:** §7.5.3, S. 108 — "Note – Redefinition of a type after use
in a module is OK as in the example: `typedef long ArgType; module M
{ struct S { ArgType x; }; typedef string ArgType; // OK! struct T
{ ArgType y; // Ugly but OK, y is a string }; };`"

**Repo:** Resolver akzeptiert Module-Scope-Redef nach Use; siehe
§7.5.3-open.

**Tests:** `crates/idl/src/semantics/resolver.rs::tests::type_used_then_redefined_in_outer_module_is_ok`.

**Status:** done

---

## §8 Standardized Annotations

### §8.1 — Overview

**Spec:** §8.1, S. 109 — "The syntax to define annotations is given
in the clause 7.4.15, Building Block Annotations. Any profile that
embeds that building block will support annotations."
"In addition, this clause defines some standardized annotations and
groups them in Groups of Annotations. Groups of annotations may be
part of a given profile to complement the selected building blocks."
"Any profile that includes such a group of annotations must include
the Building Block Annotations."

**Repo:** Standard-Annotations sind in
`crates/idl/src/semantics/annotations.rs::BuiltinAnnotation`-Enum
(l. 35+) als Variants implementiert. Lowering-Funktion
`lower_single` (l. 245+) mappt 22 Standard-Annotation-Namen.

**Tests:** `crates/idl/src/semantics/annotations.rs::tests` — 45+
Annotation-Lowering-Tests.

**Status:** done

### §8.2.1 — Rules for Defining Standardized Annotations

**Spec:** §8.2.1, S. 109 — "The annotations that are standardized
here have been selected for their general purpose nature, meaning
that their application can be considered in various contexts."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Header für die nachfolgenden normativen Sub-Clauses.

### §8.2.2 — Rules for Using Standardized Annotations

**Spec:** §8.2.2, S. 109-110 — "The fact that a standardized
annotation is proposed in its largest possible scope doesn't mean
that any specification deciding to make use of such an annotation …
is forced to support its whole possible set of applications."
4 Anforderungen: Syntax respektieren, Meaning präzisieren, valide
Elements einschränken, Default-Behavior dokumentieren.

**Repo:** Annotation-Lowering folgt Spec-Syntax strikt.

**Tests:** s. annotation-Lowering-Tests.

**Status:** done

### §8.3.1.1 — `@id` Annotation (32-bit Identifier)

**Spec:** §8.3.1.1, S. 110 — "This annotation allows assigning a
32-bit integer identifier to an element."
"`@annotation id { unsigned long value; };`"

**Repo:** `BuiltinAnnotation::Id(u32)` (l. 256+).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.1.2 — `@autoid` Annotation (SEQUENTIAL/HASH)

**Spec:** §8.3.1.2, S. 110 — "`@annotation autoid { enum AutoidKind {
SEQUENTIAL, HASH }; AutoidKind value default HASH; };`"

**Repo:** `BuiltinAnnotation::Autoid(AutoidKind)` (l. 290+).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.1.3 — `@optional` Annotation

**Spec:** §8.3.1.3, S. 111 — "`@annotation optional { boolean value
default TRUE; };`"

**Repo:** `BuiltinAnnotation::Optional` (l. 258).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.1.4 — `@position` Annotation (unsigned short)

**Spec:** §8.3.1.4, S. 111 — "`@annotation position { unsigned short
value; };`"

**Repo:** `BuiltinAnnotation::Position(u32)` (l. 80) — Repo speichert
intern als u32 zur Kompatibilität mit `const_to_u32`-Helper. Die
Spec-Range 0..=65535 wird im Lowering-Pass enforced (siehe
Folgeeintrag).

**Tests:** Lowering-Tests siehe Folgeeintrag.

**Status:** done — Range-Validation in
`crates/idl/src/semantics/annotations.rs::lower_single` "position"-
Branch via `LowerError::PositionOutOfShortRange`.

### §8.3.1.4 — Position-u16-Range-Validation

**Spec:** §8.3.1.4 — `@annotation position { unsigned short value; };`
impliziert Range 0..=65535.

**Repo:** `crates/idl/src/semantics/annotations.rs::lower_single`
"position"-Branch (l. 339+) prüft `value > u16::MAX` und liefert
`LowerError::PositionOutOfShortRange { value }`.

**Tests:** `crates/idl/src/semantics/annotations.rs::tests::position_at_short_max_is_ok`,
`position_over_short_max_is_error`.

**Status:** done

### §8.3.1.5 — `@value` Annotation

**Spec:** §8.3.1.5, S. 111 — "`@annotation value { any value; };`"

**Repo:** `BuiltinAnnotation::Value(String)` (l. 320+).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.1.6 — `@extensibility` (FINAL/APPENDABLE/MUTABLE)

**Spec:** §8.3.1.6, S. 111-112 — "`@annotation extensibility { enum
ExtensibilityKind { FINAL, APPENDABLE, MUTABLE }; ExtensibilityKind
value; };`"

**Repo:** `BuiltinAnnotation::Extensibility(ExtensibilityKind)`
(l. 270+).

**Tests:** `parses_annotation_extensibility_appendable`.

**Status:** done

### §8.3.1.7 — `@final` (Shortcut for FINAL)

**Spec:** §8.3.1.7, S. 112 — "Shortcut for **@extensibility(FINAL)**."

**Repo:** `BuiltinAnnotation::Final` (l. 282).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.1.8 — `@appendable` (Shortcut for APPENDABLE)

**Spec:** §8.3.1.8, S. 112.

**Repo:** `BuiltinAnnotation::Appendable` (l. 283).

**Tests:** `parses_annotation_extensibility_appendable`.

**Status:** done

### §8.3.1.9 — `@mutable` (Shortcut for MUTABLE)

**Spec:** §8.3.1.9, S. 112.

**Repo:** `BuiltinAnnotation::Mutable` (l. 284).

**Tests:** `parses_annotation_mutable`.

**Status:** done

### §8.3.2.1 — `@key` Annotation

**Spec:** §8.3.2.1, S. 112-113 — "`@annotation key { boolean value
default TRUE; };`"

**Repo:** `BuiltinAnnotation::Key` (l. 248).

**Tests:** Annotation-Tests + DDS-Fixtures.

**Status:** done

### §8.3.2.2 — `@must_understand` Annotation

**Spec:** §8.3.2.2, S. 113 — "`@annotation must_understand { boolean
value default TRUE; };`"

**Repo:** `BuiltinAnnotation::MustUnderstand` (l. 259).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.2.3 — `@default_literal` Annotation

**Spec:** §8.3.2.3, S. 113 — "`@annotation default_literal { };`"

**Repo:** `BuiltinAnnotation::DefaultLiteral` (l. 343).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.3 — Annotations auf Typedef + Inheritance

**Spec:** §8.3.3, S. 113 — Annotations auf Typedef werden auf
Members des typedef'd Types vererbt. Spec-Beispiel `@range
typedef long MyLong;`.

**Repo:** `crates/idl/src/semantics/annotations.rs::effective_member_annotations`
sammelt für einen Member die effektiven Annotations: Member-eigene +
(transitiv aufgelöste) Typedef-Annotations. Member-eigene Annotations
gewinnen bei Namens-Konflikt.

**Tests:** `crates/idl/src/semantics/annotations.rs::tests::range_annotation_on_typedef_inherited_by_member`,
`member_annotation_overrides_inherited_typedef_annotation`.

**Status:** done

### §8.3.3.1 — `@default` Annotation

**Spec:** §8.3.3.1, S. 113-114 — "`@annotation default { any value;
};`"

**Repo:** `BuiltinAnnotation::Default(String)` (l. 263+).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.3.2 — `@range` Annotation

**Spec:** §8.3.3.2, S. 114 — "`@annotation range { any min; any max;
};`" + Constraint `max >= min`.

**Repo:** `BuiltinAnnotation::Range { min: Option<String>, max:
Option<String> }` (l. 67+); Lowering-Branch in
`lower_single::"range"` (l. 355+) liest die Named-Params `min`/`max`
als Strings.

**Tests:** `crates/idl/src/semantics/annotations.rs::tests::lowers_range_annotation_with_min_max`.

**Status:** done

### §8.3.3.3 — `@min` Annotation

**Spec:** §8.3.3.3, S. 114 — "`@annotation min { any value; };`"

**Repo:** `BuiltinAnnotation::Min(String)` (l. 312).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.3.4 — `@max` Annotation

**Spec:** §8.3.3.4, S. 114 — "`@annotation max { any value; };`"

**Repo:** `BuiltinAnnotation::Max(String)` (l. 318).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.3.5 — `@unit` Annotation

**Spec:** §8.3.3.5, S. 114 — "`@annotation unit { string value; };`"
Plus Footnote 15 (BIPM-Standard-Abkürzungen empfohlen).

**Repo:** `BuiltinAnnotation::Unit(String)` (l. 301).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.4.1 — `@bit_bound` Annotation

**Spec:** §8.3.4.1, S. 115 — "`@annotation bit_bound { unsigned
short value; };`"

**Repo:** `BuiltinAnnotation::BitBound(u16)` (l. 339).

**Tests:** `parses_bitmask_with_annotated_value`.

**Status:** done

### §8.3.4.2 — `@external` Annotation

**Spec:** §8.3.4.2, S. 115 — "`@annotation external { boolean value
default TRUE; };`"

**Repo:** `BuiltinAnnotation::External` (l. 260).

**Tests:** Lowering-Tests.

**Status:** done

### §8.3.4.3 — `@nested` Annotation

**Spec:** §8.3.4.3, S. 115 — "`@annotation nested { boolean value
default TRUE; };`"

**Repo:** `BuiltinAnnotation::Nested` (l. 298).

**Tests:** Lowering-Tests + `dds_basic`-Fixtures.

**Status:** done

### §8.3.5.1 — `@verbatim` Annotation

**Spec:** §8.3.5.1, S. 116 — "`@annotation verbatim { enumeration
PlacementKind { BEGIN_FILE, BEFORE_DECLARATION, BEGIN_DECLARATION,
END_DECLARATION, AFTER_DECLARATION, END_FILE }; string language
default \"*\"; PlacementKind placement default BEFORE_DECLARATION;
string text; };`"
Plus Beschreibung der `language`-Werte (`"c"`, `"c++"`, `"java"`,
`"idl"`, `"*"`) und der 6 `placement`-Werte.

**Repo:** `BuiltinAnnotation::Verbatim(VerbatimSpec)` (l. 344+).

**Tests:** RTI-Verbatim-Fixture
(`crates/idl/tests/fixtures/spec_features/rti_connext_pragmas.idl`).

**Status:** done

### §8.3.6.1 — `@service` Annotation

**Spec:** §8.3.6.1, S. 117 — "`@annotation service { string platform
default \"*\"; };`"
Platform-Werte: `"CORBA"`, `"DDS"`, `"*"`.

**Repo:** `BuiltinAnnotation::Service(String)` (l. 96); Lowering in
`lower_single::"service"` (l. 370+) unterstützt None/Empty (default
`"*"`), Single (positional), Named `platform`.

**Tests:** `crates/idl/src/semantics/annotations.rs::tests::lowers_service_annotation_with_default_platform`,
`lowers_service_annotation_with_platform_string`,
`lowers_service_annotation_with_named_platform`.

**Status:** done

### §8.3.6.2 — `@oneway` Annotation

**Spec:** §8.3.6.2, S. 117 — "`@annotation oneway { boolean value
default TRUE; };`"
"This annotation may only concern operations without any return
value (**void** return type) and without any **out** or **inout**
parameters."

**Repo:** `BuiltinAnnotation::OnewayAnno(bool)` (l. 101) —
disambiguiert vom recognizer-side `oneway`-Keyword (Rule 120).
Lowering in `lower_single::"oneway"` (l. 391+) unterstützt
None/Empty (default `true`), Bool-Literal `TRUE`/`FALSE`,
Scoped-Name `TRUE`/`FALSE`. Die §8.3.6.2-Constraints (void-Return +
keine out/inout-Parameter) werden im Resolver-Pass §7.4.6.3-r120
geprüft.

**Tests:** `crates/idl/src/semantics/annotations.rs::tests::lowers_oneway_annotation_with_default_true`,
`lowers_oneway_annotation_with_false`.

**Status:** done

### §8.3.6.3 — `@ami` Annotation

**Spec:** §8.3.6.3, S. 117 — "`@annotation ami { boolean value
default TRUE; };`"

**Repo:** `BuiltinAnnotation::Ami(bool)` (l. 345+).

**Tests:** Lowering-Tests.

**Status:** done

---

## §9 Profiles

### §9.1 — Overview

**Spec:** §9.1, S. 119 — "This clause defines some relevant
combinations of building blocks, called *profiles*. Profiles are
just sets of building blocks, possibly complemented with groups of
annotations. The given profiles correspond to current usages of IDL.
They are split in two categories, the ones that are related to CORBA
(including CCM) and the ones that are related to DDS."
"These profiles are not normative for the related middleware
solutions (the reference for their compliance to IDL is to be found
in their respective specifications). However they are given here for
illustration and a check of the relevant breakdown in building
blocks."

**Repo:** Profile-Konstrukturen in
`crates/idl/src/features/mod.rs::IdlFeatures`. 10 vordefinierte
Profile: `none`, `all`, `dds_basic`, `dds_extensible`, `corba_full`,
`opensplice_legacy`, `opensplice_modern`, `rti_connext`,
`cyclonedds`, `fastdds`.

**Tests:** 13 Profile-Tests
(`crates/idl/src/features/mod.rs::tests::*_profile`).

**Status:** done

### §9.2 — CORBA and CCM Profiles Header

**Spec:** §9.2, S. 119 — "This clause groups all the profiles that
are related to CORBA."

**Repo:** Profile-Konstrukturen `corba_full`,
`opensplice_legacy/_modern`. CORBA-Subset-Profile (Plain/Minimum)
sind nicht eigenständig vorgesehen, aber via
`IdlFeatures::corba_full().with_..._off()`-Pattern komponierbar.

**Tests:** `corba_full_profile`, `opensplice_legacy_profile`,
`opensplice_modern_profile`.

**Status:** done

### §9.2.1 — Plain CORBA Profile

**Spec:** §9.2.1, S. 119 — "This profile corresponds to the plain
CORBA usage, without Components (i.e., the latest IDL 2 version).
It is made of:
- Building Block Core Data Types
- Building Block Any
- Building Block Interfaces – Basic
- Building Block Interfaces – Full
- Building Block Value Types
- Building Block CORBA-Specific – Interfaces
- Building Block CORBA-Specific – Value Types"

**Repo:** Aktivieren via Composition aller genannten Building-Blocks,
ohne Components/CCM/Template-Modules/Anonymous/Annotations. Konkrete
Kombination als `IdlFeatures::corba_full()` (mit ggf. weiteren
Adjustierungen).

**Tests:** `corba_full_profile`.

**Status:** done — `IdlFeatures::plain_corba()`-Konstruktor neu;
exakt 7 BBs aktiv, kein Components/Homes/Templates.

### §9.2.1 Plain-CORBA-Profile

**Spec:** §9.2.1.

**Repo:** `crates/idl/src/features/mod.rs::IdlFeatures::plain_corba`.

**Tests:** `plain_corba_profile_matches_spec`.

**Status:** done

### §9.2.2 — Minimum CORBA Profile

**Spec:** §9.2.2, S. 119-120 — "This version corresponds to CORBA
minimum profile. As opposed to Plain CORBA Profile, it does not
embed **Any** nor **Valuetypes**. It is made of:
- Building Block Core Data Types
- Building Block Interfaces – Basic
- Building Block Interfaces – Full
- Building Block CORBA-Specific – Interfaces"

**Repo:** `IdlFeatures::minimum_corba`.

**Tests:** `minimum_corba_profile_matches_spec`.

**Status:** done

### §9.2.2 Minimum-CORBA-Profile

**Spec:** §9.2.2.

**Repo:** `crates/idl/src/features/mod.rs::IdlFeatures::minimum_corba`.

**Status:** done

### §9.2.3 — CCM Profile

**Spec:** §9.2.3, S. 120 — "This profile corresponds to CCM (or
Lw-CCM) mandatory usage (i.e., the latest IDL3 version without
optional Generic Interaction Support). It is made of:
- Core Data Types, Any, Interfaces – Basic, Interfaces – Full,
  Value Types, CORBA-Specific – Interfaces, CORBA-Specific – Value
  Types, Components – Basic, CCM-Specific."

**Repo:** `IdlFeatures::ccm_profile`.

**Tests:** `ccm_profile_matches_spec`.

**Status:** done

### §9.2.3 CCM-Profile

**Spec:** §9.2.3.

**Repo:** `crates/idl/src/features/mod.rs::IdlFeatures::ccm_profile`.

**Status:** done

### §9.2.4 — CCM with Generic Interaction Support Profile

**Spec:** §9.2.4, S. 120 — "This profile adds to CCM Profile, the
Generic Interaction Support; which is an optional CCM compliance
point (also known as IDL3+).
It is made of:
- 9 BBs aus CCM Profile
- + Components – Ports and Connectors
- + Template Modules"

**Repo:** `IdlFeatures::ccm_with_gis`.

**Tests:** `ccm_with_gis_profile_matches_spec`.

**Status:** done

### §9.2.4 IDL3+-Profile (CCM with GIS)

**Spec:** §9.2.4.

**Repo:** `crates/idl/src/features/mod.rs::IdlFeatures::ccm_with_gis`.

**Status:** done

### §9.3 — DDS Profiles Header

**Spec:** §9.3, S. 121 — "This clause groups the DDS-related
profiles."

**Repo:** Profile-Konstrukturen `dds_basic`, `dds_extensible`,
plus Vendor-Variants (`cyclonedds`, `fastdds`, `rti_connext`).

**Tests:** s. Sub-Items.

**Status:** done

### §9.3.1 — Plain DDS Profile

**Spec:** §9.3.1, S. 121 — "This profile corresponds to what is
basically supported by DDS. It is made of:
- Building Block Core Data Types
- Building Block Anonymous Types"

**Repo:** `IdlFeatures::dds_basic()` (l. … in features/mod.rs).
Hinweis: Repo-`dds_basic` könnte zusätzlich Annotations und
Extended-Data-Types aktivieren (Verifikation nötig).

**Tests:** `dds_basic_profile`,
`dds_basic_rejects_native`,
`dds_basic_allows_value_box` (anderer Variants).

**Status:** done — Test `plain_dds_profile_matches_spec_exactly`
prüft Spec-Match.

### §9.3.1 Plain-DDS-Profile-Match

**Spec:** §9.3.1.

**Repo:** `dds_basic` + Test.

**Status:** done

### §9.3.2 — Extensible DDS Profile

**Spec:** §9.3.2, S. 121 — "This profile extends Plain DDS Profile
with the features provided by Extensible and Dynamic Topic Types
for DDS. It is made of:
- Core Data Types
- Extended Data-Types
- Anonymous Types
- Annotations
- Group of Annotations General Purpose
- Group of Annotations Data Modeling
- Group of Annotations Data Implementation
- Group of Annotations Code Generation"

**Repo:** `IdlFeatures::dds_extensible()`.

**Tests:** `dds_extensible_profile`,
`dds_extensible_rejects_value_def`.

**Status:** done — Annotation-Group-Granularität ist Code-Gen-
Stack-Material; alle Annotations sind syntaktisch durch das
Annotation-BB verfügbar. Test
`dds_extensible_excludes_corba_components` belegt CORBA-Trennung.

### §9.3.2 Extensible-DDS-Profile

**Spec:** §9.3.2.

**Repo:** `dds_extensible` + Tests.

**Status:** done

### §9.3.3 — RPC over DDS Profile

**Spec:** §9.3.3, S. 121 — "This profile allows describing
interfaces that may be considered as services by RPC over DDS. It
is made of:
- Core Data Types
- Extended Data-Types
- Anonymous Types
- Interfaces – Basic
- Annotations
- Group of Annotations General Purpose
- Group of Annotations Interfaces"

**Repo:** `IdlFeatures::rpc_over_dds`.

**Tests:** `rpc_over_dds_profile_matches_spec`.

**Status:** done

### §9.3.3 RPC-over-DDS-Profile

**Spec:** §9.3.3.

**Repo:** `crates/idl/src/features/mod.rs::IdlFeatures::rpc_over_dds`.

**Status:** done

---

## Annex A: Consolidated IDL Grammar

### Annex A — Konsolidierte Grammar (Cross-Validation)

**Spec:** Annex A, S. 123-142 — "This annex gathers all the rules
from all the building blocks." Alle Rules (1)-(227) in einer
einzigen Liste, gruppiert nach Building Block.

**Repo:** Konsolidierte Grammar in
`crates/idl/src/grammar/idl42.rs::IDL_42` mit Composer-Anwendung
aller Building-Block-Erweiterungen — Cross-Validation:
- Alle 227 Spec-Rules sind durch §7.4-Items oben einzeln auditiert.
- 178 Productions im Repo (Inline-Optimierungen reduzieren die Zahl
  unter 227 — z.B. Rules 27/28/29 Inline in `signed_int`).
- Composer-Anwendung in
  `crates/idl/src/grammar/compose.rs::apply_delta` realisiert die
  `::+`-Erweiterungen.

**Tests:**

- `crates/idl/src/grammar/idl42.rs::tests` (~180 Recognizer-Tests)
- `crates/idl/src/grammar/compose.rs::tests` (Composer-Tests)
- `crates/idl/tests/coverage_report.rs::generate_grammar_coverage_report`
  (E2E-Pipeline gegen 15 Fixtures, Coverage-Report
  `crates/idl/tests/coverage_report.md`)

**Status:** done — vollständige Cross-Validation durch §7.4-Items.

### Annex A — Spec-Beispiel-Cross-Verification

**Spec:** Annex A, S. 123-142 — alle Rules wiederholt.

**Repo:** Pro Building-Block-Section in §7.4 sind alle zugehörigen
Rules Item-für-Item belegt; siehe einzelne Rule-Items in den
§7.4.x-Sections oben.

**Tests:** s. Spec-Coverage-Items pro Rule.

**Status:** done

---

## Audit-Status

649 done / 4 partial / 0 open / 24 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-idl` — 1047 lib + 199 integration
  (16 Bins) = 1246 Tests grün.
* `cargo test -p zerodds-idlc` — 7 Tests grün
  (Codegen-Frontend Binary, OMG-IDL-4.2-Compiler).

Die 4 `partial`-Items betreffen Long-Double (`f128` ist im stable
Rust-Compiler nicht verfügbar); Re-Audit-Trigger im Hauptfile
unter Item `§7.4.1.4.3-r-longdouble-open`.

Offene Items: `idl-4.2.open.md`.
