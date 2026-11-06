# OMG IDL 4.2 — Spec-Coverage

**Spec:** [OMG Interface Definition Language 4.2 — formal/18-01-05 (March 2018, 132 Seiten) →](https://www.omg.org/spec/IDL/4.2/)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Das IDL-Frontend ist über sechs Crates verteilt:

- `crates/idl-cst/` — Lexer und CST: Tokens, Trivia, Spans, Baum.
- `crates/idl-parse/` — Präprozessor nach ISO/IEC 14882:2003, Parser,
  Deklarationen, Typ-Specs, Vendor-Dialekte.
- `crates/ast/` — das Modell: Knoten, Facetten, Builtin-Annotationen,
  Profile, Resolve.
- `crates/idl-lower/` — Lowering, Annotationen, Scope, Validierung,
  Repository-Ids.
- `crates/idl-compose/` — die Stufe, die Frontend und Modell
  zusammensetzt und den unzulässigen Zyklus erkennt.
- `crates/ccm/` — die IDL→IDL-Umschreibung der CCM-Konstrukte;
  `crates/corba-codegen/` — Repository-Ids, Stubs, Skeletons, Annex-A1-Traits.

`tools/idlc/` ist der Compiler-Treiber. Die vollständige Negativliste zur
Grammatik (357 Proben, neun Spalten je Unterbefehl) steht in
[`idl-4.2.gaps.md`](idl-4.2.gaps.md); diese Seite ist die Positivliste.

**Offene Punkte:** keine.

---

## §7.1 Overview

### §7.1 Overview

**Spec:** §7.1, S. 13–14 — die Klausel führt die Bestandteile einer
IDL-Spezifikation ein und legt fest, dass die Regeln in atomare *building
blocks* gruppiert sind, die erst in einem Profil verwendet werden.

**Repo:** `crates/idl-parse/src/`, `crates/idl-lower/src/`,
`crates/idl-cst/src/`.

**Tests:** —

**Status:** `n/a (informative)` — §7.1 gliedert die Specification und
stellt keine Anforderung an eine Implementierung. Die Bausteine werden je
in §7.4.x auditiert.

---

## §7.2 Lexical Conventions

### §7.2.1 Tokens

**Spec:** §7.2.1, S. 17 — "There are five kinds of tokens: identifiers,
keywords, literals, operators, and other separators"; "the next token is
taken to be the longest string of characters that could possibly
constitute a token".

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`,
`crates/idl-cst/src/lex.rs::next_token`.

**Tests:** `crates/idl-cst/tests/lexer.rs::an_empty_file_yields_one_eof_token`,
`crates/idl-cst/tests/lexer.rs::every_byte_is_reachable_for_a_realistic_file`,
`crates/idl-cst/tests/lexer.rs::spans_never_run_past_the_input`,
`crates/idl-cst/tests/lexer.rs::a_span_covers_what_it_claims`,
`crates/idl-cst/tests/tree.rs::a_flat_tree_reproduces_its_source_byte_for_byte` —
`lexer.rs:390: assert_eq!(Span::new(0, 6).slice(source), Some("module"));`.

**Status:** done

### §7.2.2 Comments

**Spec:** §7.2.2, S. 17 — "The characters /* start a comment, which
terminates with the characters */. These comments do not nest."; "The
characters // start a comment, which terminates at the end of the line on
which they occur."

**Repo:** `crates/idl-cst/src/lex.rs::scan_trivia`; die Trivia wandert an
das nächste Token, der Baum führt sie mit.

**Tests:** `crates/idl-cst/tests/lexer.rs::a_comment_before_a_token_is_that_tokens_leading_trivia`,
`crates/idl-cst/tests/lexer.rs::the_eof_token_carries_the_files_trailing_trivia`,
`crates/idl-cst/tests/lexer.rs::a_block_comment_is_one_piece_of_trivia_even_across_lines`,
`crates/idl-cst/tests/lexer.rs::an_unterminated_block_comment_runs_to_end_of_file_rather_than_failing`,
`crates/idl-cst/tests/tree.rs::a_tree_over_comments_and_spacing_reproduces_them_too`,
`crates/idl-cst/tests/tree.rs::a_leading_comment_is_not_part_of_it_either` —
`lexer.rs:274: assert_eq!(comments.len(), 1);`.

**Status:** done

### §7.2.3 Identifiers

**Spec:** §7.2.3, S. 18 — "An identifier is an arbitrarily long sequence of
ASCII alphabetic, digit and underscore (_) characters. The first character
must be an ASCII alphabetic character."; "IDL identifiers are case
insensitive."

**Repo:** `crates/idl-cst/src/lex.rs::lex_word`,
`crates/idl-cst/src/token.rs::identifier_name`.

**Tests:** `crates/idl-cst/tests/lexer.rs::an_ordinary_identifier_is_not_marked_escaped`,
`crates/idl-cst/tests/lexer.rs::an_identifier_that_differs_from_a_keyword_only_in_case_is_named_with_it` —
`lexer.rs:87: assert_eq!(zerodds_idl_cst::keyword_collision("CUSTOM"), Some("custom"));`.

**Status:** done

### §7.2.3.1 Collision Rules

**Spec:** §7.2.3.1, S. 18 — "Upper- and lower-case letters are treated as
the same letter"; "There is only one namespace for IDL identifiers in each
scope. Using the same identifier for a constant and an interface, for
example, produces a compilation error."

**Repo:** `crates/idl-cst/src/token.rs::keyword_collision`,
`crates/idl-lower/src/lower.rs::identifiers_do_not_collide_with_keywords`,
`crates/idl-lower/src/scope.rs::ScopeStack`.

**Tests:** `crates/idl-lower/tests/validation.rs::two_names_differing_only_by_case_collide`,
`crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`,
`crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide`,
`crates/idl-lower/tests/validation.rs::an_identifier_differing_from_a_keyword_only_in_case_is_refused`,
`crates/idl-lower/tests/validation.rs::a_name_that_collides_with_no_keyword_is_left_alone`,
`crates/idl-lower/tests/lowering.rs::a_name_declared_in_two_openings_of_one_module_collides` —
`lowering.rs:122: .any(|d| d.code == DiagnosticCode::DuplicateName),`.

**Status:** done

### §7.2.3.2 Escaped Identifiers

**Spec:** §7.2.3.2, S. 18 — "users may lexically "escape" identifiers by
prepending an underscore (_) to an identifier. This is a purely lexical
convention that ONLY turns off keyword checking."

**Repo:** `crates/idl-cst/src/token.rs::identifier_name` (der führende
`_` gehört nicht zum Namen), `crates/idl-lower/src/lower.rs::identifiers_do_not_collide_with_keywords`.

**Tests:** `crates/idl-cst/tests/lexer.rs::an_escaped_identifier_is_the_name_without_the_underscore`,
`crates/idl-cst/tests/lexer.rs::an_underscore_prefixed_non_keyword_is_still_an_escaped_identifier`,
`crates/idl-lower/tests/validation.rs::the_underscore_escape_turns_the_keyword_check_off`,
`crates/idl-parse/tests/parse.rs::a_name_colliding_with_a_keyword_parses_as_a_name_and_is_refused_a_layer_up` —
`validation.rs:643: assert_clean("enum Ee { AA, _custom, BB };");`.

**Status:** done

### §7.2.4 Keywords (Table 7-6)

**Spec:** §7.2.4, S. 19 — "The identifiers listed in Table 7-6 are reserved
for use as keywords and may not be used for another purpose, unless escaped
with a leading underscore."; Tabelle 7-6 nennt 84 Keywords einschliesslich
der vier grossgeschriebenen `TRUE`, `FALSE`, `Object`, `ValueBase`.

**Repo:** `crates/idl-cst/src/token.rs::KEYWORDS` (die Tabelle selbst),
`crates/idl-cst/src/token.rs::keyword_collision`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`,
`crates/idl-cst/tests/lexer.rs::a_keyword_is_recognised_only_in_the_spelling_table_7_6_gives`,
`crates/idl-cst/tests/lexer.rs::the_four_keywords_table_7_6_capitalises_are_spelled_as_it_spells_them`,
`crates/idl-lower/tests/validation.rs::the_diagnostic_names_the_keyword_and_the_clause` —
`lexer.rs:30: for (text, expected) in KEYWORDS {`.

**Status:** done — der Test iteriert über alle Einträge der Tabelle, nicht
über eine Stichprobe.

### §7.2.5 Other Characters Recognized by IDL (Punctuation)

**Spec:** §7.2.5, S. 20 — Tabelle 7-7 (Punctuation) und Tabelle 7-8 (die
Präprozessor-Token `#`, `##`, `!`, `||`, `&&`).

**Repo:** `crates/idl-cst/src/lex.rs::lex_punct`, `crates/idl-cst/src/lex.rs::scan_escape`, `crates/idl-parse/src/preprocess/pptoken.rs::PUNCTUATORS`.

**Tests:** `crates/idl-cst/tests/lexer.rs::idl_7_2_5_every_punctuation_character_of_table_7_7_is_recognised`,
`crates/idl-cst/tests/lexer.rs::idl_7_2_5_the_iso_latin_1_letters_of_table_7_2_stand_in_a_string_literal`,
`crates/idl-parse/tests/preprocess.rs::idl_7_2_5_the_preprocessor_tokens_of_table_7_8_take_effect`.

**Status:** `done` — die 22 Zeichen aus Tabelle 7-7, die eigene Token sind, werden zu ihrer eigenen Art; Apostroph, Anführungszeichen und Backslash begrenzen und escapen Literale; die fünf Token aus Tabelle 7-8 wirken im Präprozessor (`#`, `##`, `!`, `||`, `&&`); die ISO-Latin-1-Buchstaben aus Tabelle 7-2 behalten ihren Wert in Zeichen- und String-Literalen.

### §7.2.6 Literals

**Spec:** §7.2.6, S. 20 — die Klausel benennt Integer-, Character-,
Floating-Point-, String- und Fixed-Point-Literale.

**Repo:** `crates/idl-cst/src/lex.rs::lex_number` (Integer, Gleitkomma,
Fixed), `crates/idl-cst/src/lex.rs::lex_char`, `crates/idl-cst/src/lex.rs::lex_string`.

**Tests:** `crates/idl-cst/tests/lexer.rs::a_literal_value_is_recovered_from_the_token_text_alone`,
`crates/idl-cst/tests/lexer.rs::text_that_is_not_one_whole_token_of_that_kind_has_no_value`,
`crates/idl-cst/tests/lexer.rs::a_literal_too_large_for_i128_is_an_error_not_a_wrapped_value` —
`lexer.rs:165: assert_eq!(kinds(&huge)[0], TokenKind::Error);`.

**Status:** done

### §7.2.6.1 Integer Literals

**Spec:** §7.2.6.1, S. 20–21 — Dezimal-, Oktal- (`0` vorangestellt) und
Hexadezimal- (`0x`/`0X`) Form; die Schreibweise bleibt erhalten.

**Repo:** `crates/idl-cst/src/lex.rs::lex_number`, `crates/idl-cst/src/token.rs::IntLiteral`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_radix_reaches_the_same_value_and_keeps_how_it_was_written`,
`crates/idl-cst/tests/lexer.rs::a_leading_zero_before_a_point_is_a_decimal_number_and_not_an_octal_one`,
`crates/idl-cst/tests/lexer.rs::a_literal_too_large_for_i128_is_an_error_not_a_wrapped_value` —
`lexer.rs:132: assert_eq!(tokens[0].kind, TokenKind::IntLiteral, "{text}");`.

**Status:** done

### §7.2.6.2 Character Literals

**Spec:** §7.2.6.2, S. 21–22 — ein Zeichenliteral ist ein einfaches
Anführungszeichen, genau ein Zeichen, ein einfaches Anführungszeichen.

**Repo:** `crates/idl-cst/src/lex.rs::lex_char`.

**Tests:** `crates/idl-cst/tests/lexer.rs::character_and_string_literals_resolve_their_escapes`,
`crates/idl-cst/tests/lexer.rs::a_raw_non_ascii_char_literal_is_its_code_point_and_not_its_last_octet`,
`crates/idl-cst/tests/lexer.rs::a_char_literal_the_type_cannot_hold_is_an_error_token` —
`lexer.rs:495: assert_eq!(lex("'ab'")[0].kind, TokenKind::Error);`.

**Status:** done

### §7.2.6.2.1 Wide and Non-wide Characters

**Spec:** §7.2.6.2.1, S. 21 — das `L`-Präfix (`L'x'`) bezeichnet ein Wide
Character; der Wertebereich ist ISO 10646.

**Repo:** `crates/idl-cst/src/lex.rs::next_token` (`TokenKind::WCharLiteral`).

**Tests:** `crates/idl-cst/tests/lexer.rs::wide_literals_are_their_own_kinds`,
`crates/idl-cst/tests/lexer.rs::a_char_literal_the_type_cannot_hold_is_an_error_token`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends` —
`lexer.rs:203: assert_eq!(kinds("L'x'")[0], TokenKind::WCharLiteral);`.

**Status:** done

### §7.2.6.2.2 Escape Sequences

**Spec:** §7.2.6.2.2, S. 21–22 — die Escape-Sequenzen des Spec-Absatzes zu
`character literals`: dezimale, oktale und hexadezimale Form sowie die
`\u`-/`\U`-Form für Wide Characters.

**Repo:** `crates/idl-cst/src/lex.rs::scan_escape`, `crates/idl-lower/src/expr.rs::literal` (die `\u`-Escape in einem schmalen Literal).

**Tests:** `crates/idl-cst/tests/lexer.rs::idl_7_2_6_2_2_every_escape_sequence_of_table_7_9_has_its_value`,
`crates/idl-cst/tests/lexer.rs::character_and_string_literals_resolve_their_escapes`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_2_6_2_2_a_unicode_escape_is_refused_in_a_narrow_literal`.

**Status:** `done` — jede Escape-Sequenz aus Tabelle 7-9 hat ihren Wert, auch `\ooo` mit ein bis drei Ziffern, `\xhh` mit einer oder zwei und `\uhhhh` mit ein bis vier (die Beispiele `\u002E` und `\u3BC` des Absatzes), und eine Ziffernfolge endet am ersten Zeichen, das keine Ziffer ist; eine `\u`-Escape in einem `char`- oder `string`-Literal ist ein Fehler (`IDL0019`).

### §7.2.6.3 String Literals und Adjacent Concatenation

**Spec:** §7.2.6.3, S. 22 — "Adjacent string literals are concatenated.
Characters in concatenated strings are kept distinct."; "Attempts to
assign a wide string literal to a non-wide string constant or to assign a
non-wide string literal to a wide string constant result in a compile-time
diagnostic."

**Repo:** `crates/idl-parse/src/parser.rs::eat_adjacent_string_literals`,
`crates/idl-cst/src/lex.rs::lex_string`.

**Tests:** `crates/idl-lower/tests/idl_4_2_grammar.rs::adjacent_string_literals_are_one_literal`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::the_characters_of_concatenated_strings_are_kept_distinct`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::adjacent_wide_string_literals_are_one_literal`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::a_narrow_and_a_wide_literal_are_not_concatenated`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::an_annotation_argument_takes_the_whole_concatenated_literal`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::a_typeid_takes_the_whole_concatenated_literal`,
`crates/idl-lower/tests/validation.rs::the_wide_and_narrow_string_pair_is_the_case_the_clause_names` —
`idl_4_2_grammar.rs:54: assert_eq!(constant(&ast, "K"), ConstValue::String("abcd".to_string()));`.

**Status:** done

### §7.2.6.4 Floating-point Literals

**Spec:** §7.2.6.4, S. 22–23 — die Gleitkomma-Literale mit und ohne
Exponent und mit führendem oder nachgestelltem Punkt.

**Repo:** `crates/idl-cst/src/lex.rs::lex_number`,
`crates/idl-lower/src/expr.rs`.

**Tests:** `crates/idl-cst/tests/lexer.rs::float_forms_stay_distinguishable_from_one_another`,
`crates/idl-cst/tests/lexer.rs::a_leading_zero_before_a_point_is_a_decimal_number_and_not_an_octal_one`,
`crates/idl-lower/tests/floating_constant_arithmetic.rs::a_difference_of_two_literals_evaluates`,
`crates/idl-lower/tests/floating_constant_arithmetic.rs::the_arithmetic_is_double`,
`crates/idl-lower/tests/floating_constant_arithmetic.rs::a_large_value_keeps_its_exponent`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`lex-7.2.6.4-float-leading-dot`, `lex-7.2.6.4-float-trailing-dot`) —
`lexer.rs:144: assert_eq!(kinds(text)[0], TokenKind::FloatLiteral, "{text}");`.

**Status:** done

### §7.2.6.5 Fixed-point Literals

**Spec:** §7.2.6.5, S. 23 — die `d`/`D`-Suffixform ist das
Fixed-Point-Literal; `f`/`F` ist die Gleitkommaform.

**Repo:** `crates/idl-cst/src/lex.rs::lex_number` (der `d`-Zweig),
`crates/idl-lower/src/expr.rs` (Auswertung als `fixed<digits, scale>`).

**Tests:** `crates/idl-cst/tests/lexer.rs::a_d_suffix_makes_a_fixed_point_literal`,
`crates/idl-lower/tests/lowering.rs::a_fixed_constant_takes_its_type_from_its_literal`,
`crates/idl-lower/tests/completion.rs::a_fixed_type_carries_its_digits_and_scale`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`lex-7.2.6.5-fixed-literal`) —
`lexer.rs:157: assert_eq!(kinds(text)[0], TokenKind::FixedLiteral, "{text}");`.

**Status:** done

---

## §7.3 Preprocessing

### §7.3 Preprocessing (ISO/IEC 14882:2003)

**Spec:** §7.3, S. 23 — "IDL shall be preprocessed according to the
specification of the preprocessor in ISO/IEC 14882:2003."

**Repo:** `crates/idl-parse/src/preprocess/mod.rs` (Direktivenlauf),
`crates/idl-parse/src/preprocess/condition.rs` (`#if`-Auswertung),
`crates/idl-parse/src/preprocess/macro_.rs` (Makrotabelle),
`crates/idl-parse/src/preprocess/pptoken.rs` (Präprozessor-Token).

**Tests:** `crates/idl-parse/tests/preprocess.rs::a_macro_is_expanded_before_the_parser_sees_it`,
`crates/idl-parse/tests/preprocess.rs::the_expanded_text_is_itself_lexable`,
`crates/idl-parse/tests/preprocess.rs::the_expanded_stream_carries_no_directive_tokens`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::the_diagnostic_code_is_stable` —
`preprocess.rs:493: assert!(text.contains('8'), "the macro was not expanded: {text}");`.

**Status:** done

### §7.3 Zeilenfortsetzung mit Backslash

**Spec:** §7.3, S. 23 — "A preprocessing directive (or any line) may be
continued on the next line in a source file by placing a backslash
character (\), immediately before the newline at the end of the line to be
continued."

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::splice` (am Anfang von
`directive`), `crates/idl-cst/src/lex.rs::scan_trivia` auf der
Präprozessor-Ebene.

**Tests:** `crates/idl-parse/tests/preprocessor_iso_14882.rs::a_backslash_continues_a_directive`,
`crates/idl-cst/tests/lexer.rs::a_preprocessor_line_is_trivia_including_its_continuations` —
`lexer.rs:296: assert_eq!(reconstruct(source), source);`.

**Status:** done

### §7.3 Makro-Expansion und Rescan mit Hide-Set

**Spec:** §7.3, S. 23 (Übernahme aus ISO/IEC 14882:2003 §16.3) — ein
Makro, das sich selbst expandiert, expandiert sich nicht erneut; der Rescan
greift auf die Tokens nach dem Aufruf zu.

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::run` und
`substitute`, `crates/idl-parse/src/preprocess/macro_.rs::MacroTable`.

**Tests:** `crates/idl-parse/tests/preprocess.rs::a_defined_name_is_replaced_by_its_body`,
`crates/idl-parse/tests/preprocess.rs::a_macro_expanding_to_another_macro_is_rescanned`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::a_macro_does_not_expand_inside_its_own_expansion`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::the_rescan_reaches_the_tokens_after_the_invocation`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::an_argument_is_expanded_before_it_is_substituted`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::an_expansion_that_is_not_an_idl_token_is_reported` —
`preprocess.rs:67: assert!(result.text.contains("= 1"), "{}", result.text);`.

**Status:** done

### §7.3 Funktionsartige Makros, Stringize `#`, Token-Paste `##`

**Spec:** §7.3 in Verbindung mit ISO/IEC 14882:2003 §16.3.1 und §16.3.2 —
objektartige und funktionsartige Makros, `#` als Stringize-Operator,
`##` als Token-Paste.

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::substitute` und
`arguments`, `crates/idl-parse/src/preprocess/pptoken.rs` (Punktuatoren).

**Tests:** `crates/idl-parse/tests/preprocessor_iso_14882.rs::a_function_like_macro_substitutes_its_arguments`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::stringize_spells_the_argument`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::token_paste_joins_two_tokens`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`lex-7.3-define-function`, `lex-7.3-token-paste`) —
`preprocessor_iso_14882.rs:137: "struct Sa { long m ; } ;"`.

**Status:** done

### §7.3 #include

**Spec:** §7.3, S. 23 — "Text in files included with a #include directive
is treated as if it appeared in the including file."

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::include` mit der
Schranke `MAX_INCLUDE_DEPTH`.

**Tests:** `crates/idl-parse/tests/preprocess.rs::an_included_file_contributes_its_declarations`,
`crates/idl-parse/tests/preprocess.rs::both_include_spellings_resolve`,
`crates/idl-parse/tests/preprocess.rs::an_include_cycle_is_diagnosed_and_not_followed`,
`crates/idl-parse/tests/preprocess.rs::an_include_chain_past_the_nesting_bound_is_refused`,
`crates/idl-parse/tests/source_map.rs::a_token_from_an_include_names_that_file`,
`crates/idl-lower/tests/conformance.rs::a_fixture_with_an_include_gains_the_declarations_it_includes` —
`preprocess.rs:206: assert!(result.text.contains("Common"));`.

**Status:** done

### §7.3 #define und #undef

**Spec:** §7.3, S. 23 — `#define` und `#undef` mit objektartigem Namen;
die reservierten Namen der ISO-Norm dürfen nicht neu definiert werden.

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::directive`,
`crates/idl-parse/src/preprocess/macro_.rs::MacroTable::define` und
`undefine`.

**Tests:** `crates/idl-parse/tests/preprocess.rs::a_redefinition_replaces_the_earlier_one`,
`crates/idl-parse/tests/preprocess.rs::a_define_without_a_name_is_diagnosed`,
`crates/idl-parse/tests/preprocess.rs::an_undefined_name_is_left_alone`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::the_predefined_names_cannot_be_defined`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`lex-7.3-undef`) —
`preprocess.rs:41: assert!(expanded.contains("MAX"), "{expanded}");`.

**Status:** done

### §7.3 Konditionalen `#if` / `#ifdef` / `#ifndef` / `#else` / `#elif` / `#endif`

**Spec:** §7.3 in Verbindung mit ISO/IEC 14882:2003 §16.1 — die
Kondition wird über einen konstanten Ausdruck ausgewertet, `defined(X)`
fragt die Definition ab.

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::directive` und
`evaluate_condition`, `crates/idl-parse/src/preprocess/condition.rs::evaluate`.

**Tests:** `crates/idl-parse/tests/preprocess.rs::ifdef_keeps_only_the_taken_branch`,
`crates/idl-parse/tests/preprocess.rs::ifndef_is_the_other_way_round`,
`crates/idl-parse/tests/preprocess.rs::elif_takes_the_first_true_branch_and_no_later_one`,
`crates/idl-parse/tests/preprocess.rs::else_is_taken_only_when_nothing_before_it_was`,
`crates/idl-parse/tests/preprocess.rs::conditionals_nest`,
`crates/idl-parse/tests/preprocess.rs::a_define_inside_a_dropped_branch_does_not_take_effect`,
`crates/idl-parse/tests/preprocess.rs::an_if_with_a_literal_works_and_an_unevaluable_one_is_reported`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::if_evaluates_an_arithmetic_expression`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::a_division_by_zero_in_a_live_condition_is_an_error` —
`preprocess.rs:76: assert!(expanded.contains("Kept"), "{expanded}");`.

**Status:** done

### §7.3 #line

**Spec:** §7.3 in Verbindung mit ISO/IEC 14882:2003 §16.4 — `#line` setzt
Zeilennummer und Dateiname für die folgenden Tokens.

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::line_control`.

**Tests:** `crates/idl-parse/tests/preprocessor_iso_14882.rs::hash_line_renumbers_and_renames`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`lex-7.3-line-directive`) —
`preprocessor_iso_14882.rs:259: "const long K = 100 ; const string F = \"robot.idl\" ;"`.

**Status:** done

### §7.3 #pragma und #pragma once

**Spec:** §7.3, S. 23 — "The textual location of IDL-specific pragmas may
be semantically constrained."

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::directive` und
`flush_trailing_pragmas`; interpretiert wird eine `#pragma`-Zeile in
`crates/idl-lower/src/complete.rs`.

**Tests:** `crates/idl-parse/tests/preprocess.rs::a_pragma_is_not_the_preprocessors_business`,
`crates/idl-parse/tests/preprocess.rs::a_pragma_the_preprocessor_does_not_own_still_reaches_the_stream`,
`crates/idl-parse/tests/preprocess.rs::pragma_once_holds_the_way_an_ifndef_guard_does`,
`crates/idl-parse/tests/preprocess.rs::a_pragma_once_in_a_branch_that_is_cut_out_does_not_guard`,
`crates/idl-lower/tests/dialects.rs::a_pragma_id_sets_the_repository_id_like_typeid_does`,
`crates/idl-lower/tests/dialects.rs::a_cyclone_pragma_keylist_reaches_the_same_field_as_rtis_keylist`,
`crates/idl-lower/tests/dialects.rs::the_opendds_pragmas_reach_the_opendds_facet` —
`preprocess.rs:332: assert_eq!(kept, 1, "the keylist pragma did not survive");`.

**Status:** done

### §7.3 #error

**Spec:** §7.3 in Verbindung mit ISO/IEC 14882:2003 §16.5 — "#error causes
the implementation to produce a diagnostic message that includes the
specified sequence of preprocessing tokens."

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::directive`, Arm `error`
(`DiagnosticCode::ErrorDirective`).

**Tests:** `crates/idl-parse/tests/preprocess.rs::an_error_directive_is_a_diagnostic_and_carries_its_text`,
`crates/idl-parse/tests/preprocess.rs::an_error_directive_in_a_branch_that_is_cut_out_says_nothing`,
`crates/idl-parse/tests/preprocess.rs::a_bare_error_directive_is_still_a_diagnostic` —
`preprocess.rs:732: out.diagnostics.iter().any(|d| d.code.as_str() == "IDL0020"),`.

**Status:** done

### §7.3 __LINE__ und __FILE__

**Spec:** §7.3 in Verbindung mit ISO/IEC 14882:2003 §16.10 — die
vordefinierten Namen `__LINE__` und `__FILE__`.

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::predefined`.

**Tests:** `crates/idl-parse/tests/preprocessor_iso_14882.rs::line_and_file_name_where_they_stand`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::line_inside_a_macro_is_the_line_of_the_invocation`,
`crates/idl-parse/tests/preprocessor_iso_14882.rs::the_predefined_names_cannot_be_defined` —
`preprocessor_iso_14882.rs:239: "const long K = 3 ;"`.

**Status:** done

---

## §7.4 IDL Grammar

### §7.4 Grammatikrahmen und vorbestehende Nichtterminale

**Spec:** §7.4, S. 24 — "The grammar for a well-formed IDL specification is
described by rules expressed in Extended Backus Naur Form (EBNF) completed
to support rule extensions as explained in clause 7.1"; Tabelle 7-10 führt
`<identifier>`, `<integer_literal>`, `<string_literal>`,
`<wide_string_literal>`, `<character_literal>`, `<wide_character_literal>`,
`<fixed_pt_literal>`, `<floating_pt_literal>` als vorbestehende
Nichtterminale.

**Repo:** `crates/idl-parse/src/parser.rs`,
`crates/idl-parse/src/type_spec.rs::is_primitive_keyword`,
`crates/idl-parse/src/decl.rs::parse_definition`.

**Tests:** `crates/idl-parse/tests/parse.rs::a_file_using_most_of_the_grammar_parses_cleanly_and_round_trips`,
`crates/idl-parse/tests/parse.rs::the_multi_word_primitive_types_parse_as_one_type_each`,
`crates/idl-lower/tests/validation.rs::a_file_using_the_whole_language_reports_nothing` —
`parse.rs:449: assert_eq!(result.text(), source);`.

**Status:** done

---

## §7.4.1 Building Block Core Data Types

### §7.4.1.1 Purpose

**Spec:** §7.4.1.1, S. 24 — "The Core Data Types are the data types that
must be supported by every profile."

**Repo:** `crates/idl-lower/src/lower.rs::lower`,
`crates/ast/src/model.rs::Ast`.

**Tests:** —

**Status:** `n/a (informative)` — die Klausel begründet den Baustein und
verlangt nichts über die Form; die einzelnen Typen sind in §7.4.1.4.4.2
auditiert.

### §7.4.1.3 Syntax (Produktionen 1–68)

**Spec:** §7.4.1.3, S. 24–30 — die Produktionen der EBNF-Tabelle:
`specification`, `definition`, `module_dcl`, `const_dcl`, `const_type`,
`const_expr`, `type_dcl`, `typedef_dcl`, `type_declarator`, `constr_type_dcl`,
`struct_dcl`, `member`, `union_dcl`, `switch_type`, `case`, `case_label`,
`enum_dcl`, `enumerator`, `array_declarator`, `simple_type_spec`,
`native_type`, `forward_dcl`.

**Repo:** `crates/idl-parse/src/decl.rs::parse_definition` und die Produktionsfunktionen daneben (`parse_module`, `parse_const`, `parse_typedef`, `parse_struct_def`, `parse_union_case`, `parse_enum_def`, `parse_native`), `crates/idl-parse/src/type_spec.rs::parse_type_spec`.

**Tests:** `crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised`,
`crates/idl-parse/tests/parse.rs::several_declarators_share_one_type`,
`crates/idl-parse/tests/parse.rs::a_forward_declaration_is_its_own_form`,
`crates/idl-parse/tests/parse.rs::an_array_declarator_is_only_known_at_the_bracket`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::every_constructed_form_can_be_defined_inside_a_typedef`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_1_4_4_6_a_native_type_is_used_by_its_name_wherever_a_type_is`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_annex_a_every_probe_of_the_gap_scan_gets_the_specifications_answer`.

**Status:** `done` — die Produktionen (1) bis (68) sind erkannt; die Proben des Gap-Scans für jede von ihnen bekommen die Antwort der Spezifikation. Regel (62) ist `<simple_declarator> ::= <identifier>`: ein nativer Typ wird mit Regel (61) deklariert und über seinen Namen benutzt wie jeder deklarierte Typ; der Native-Test führt das durch ein Member, einen `typedef` und eine Operation.

### §7.4.1.4.1 IDL Specification

**Spec:** §7.4.1.4.1, S. 30 — eine IDL-Spezifikation besteht aus
Deklarationen; die Included-Dateien zählen dazu.

**Repo:** `crates/idl-lower/src/lower.rs::lower`.

**Tests:** `crates/idl-lower/tests/conformance.rs::every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares`,
`crates/idl-lower/tests/conformance.rs::every_fixture_produces_a_model_with_declarations_in_it`,
`crates/idl-lower/tests/conformance.rs::every_node_of_every_fixture_names_the_file_it_came_from`,
`crates/idl-lower/tests/conformance.rs::a_fixture_with_an_include_gains_the_declarations_it_includes` —
`conformance.rs:202: ast.types().count() > 0 || ast.interfaces().count() > 0,`.

**Status:** done

### §7.4.1.4.2 Modules

**Spec:** §7.4.1.4.2, S. 30–31 — ein **module** eröffnet einen Scope, der
bis zur schliessenden `}` reicht.

**Repo:** `crates/idl-lower/src/lower.rs::module`,
`crates/idl-lower/src/scope.rs::ScopeStack::push_container`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_module_becomes_a_module_node`,
`crates/idl-lower/tests/lowering.rs::nested_modules_nest_in_the_model_too`,
`crates/idl-lower/tests/conformance.rs::a_conditional_in_a_fixture_keeps_the_branch_out_of_the_model`,
`crates/ast/tests/node_space.rs::nested_modules_are_addressable_by_path`,
`crates/ast/tests/node_space.rs::a_fresh_model_has_an_anonymous_root_module` —
`lowering.rs:51: assert!(ast.module_by_path(&scoped(&["Robot"])).is_some());`.

**Status:** done

### §7.4.1.4.3 Constants

**Spec:** §7.4.1.4.3, S. 37 — eine Konstantendeklaration besteht unter
anderem aus "a value expression (`<const_expr>`), which shall be consistent
with the type declared for the constant".

**Repo:** `crates/idl-lower/src/lower.rs::value_disagrees_with` und
`check_const_value_form`, `crates/idl-lower/src/expr.rs` (Auswertung).

**Tests:** `crates/idl-lower/tests/validation.rs::a_constant_whose_value_is_not_of_its_declared_type_is_refused`,
`crates/idl-lower/tests/validation.rs::every_form_the_specification_admits_still_gets_through`,
`crates/idl-lower/tests/completion.rs::a_constant_takes_its_declared_type_not_the_expressions`,
`crates/idl-lower/tests/lowering.rs::a_string_constant_evaluates_even_though_no_integer_holds_it`,
`crates/idl-lower/tests/lowering.rs::eval_const_follows_a_reference_and_keeps_arithmetic_integral`,
`crates/ast/tests/node_space.rs::every_operator_of_idl_4_2_is_representable`,
`crates/ast/tests/node_space.rs::division_by_zero_is_an_error_not_a_panic` —
`validation.rs:701: assert_trips("const long K = \"x\";", DiagnosticCode::TypeMismatch);`.

**Status:** done

### §7.4.1.4.4 Data Types (Rahmen)

**Spec:** §7.4.1.4.4, S. 38 — die Klausel gliedert die Daten-Typen in
`referencing`, `basic`, `template` und `constructed`.

**Repo:** `crates/idl-lower/src/validate.rs::validate_type`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_file_using_the_whole_language_reports_nothing`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind` —
`lowering.rs:347: TypeKind::Primitive(expected),`.

**Status:** done

### §7.4.1.4.4.1 Referencing Types

**Spec:** §7.4.1.4.4.1, S. 38 — ein Referenz-Typ ist ein qualifizierter
Name; er darf auf eine später deklarierte Definition zeigen.

**Repo:** `crates/idl-lower/src/lower.rs::named_type_ref`,
`crates/idl-lower/src/scope.rs::ScopeStack::lookup_candidates`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_reference_to_a_type_declared_later_is_pending_not_an_error`,
`crates/idl-lower/tests/lowering.rs::a_reference_to_a_type_declared_earlier_resolves_immediately`,
`crates/idl-lower/tests/lowering.rs::a_pending_reference_resolves_once_the_name_arrives`,
`crates/idl-lower/tests/lowering.rs::a_name_that_never_arrives_ends_up_broken_rather_than_pending`,
`crates/idl-lower/tests/reported_idl.rs::a_fully_qualified_scoped_name_resolves`,
`crates/ast/tests/node_space.rs::an_unknown_path_resolves_to_nothing` —
`lowering.rs:381: assert!(report.resolved >= 1, "nothing resolved: {report:?}");`.

**Status:** done

### §7.4.1.4.4.2 Basic Types

**Spec:** §7.4.1.4.4.2, S. 38–39 — die Basis-Typen der Produktionen 30–37.

**Repo:** `crates/idl-lower/src/lower.rs::primitive_of`,
`crates/ast/src/node/type_.rs::PrimitiveKind`.

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable` —
`node_space.rs:86: assert_eq!(PrimitiveKind::ALL.len(), 15);`.

**Status:** done

### §7.4.1.4.4.2.1 Integer Types

**Spec:** §7.4.1.4.4.2.1, S. 38–39 — `short`, `long`, `long long`,
`unsigned short`, `unsigned long`, `unsigned long long`.

**Repo:** `crates/idl-lower/src/lower.rs::primitive_of`,
`crates/idl-parse/src/type_spec.rs::is_primitive_keyword`.

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/idl-parse/tests/parse.rs::the_multi_word_primitive_types_parse_as_one_type_each`,
`crates/idl-lower/tests/validation.rs::every_form_the_specification_admits_still_gets_through` —
`lowering.rs:327: ("unsigned long long", PrimitiveKind::UInt64),`.

**Status:** done

### §7.4.1.4.4.2.2 Floating-Point Types

**Spec:** §7.4.1.4.4.2.2, S. 34 — "IDL floating-point types are float,
double, and long double. The float type represents IEEE single-precision
floating point numbers; the double type represents IEEE double-precision
floating point numbers. The long double data type represents an IEEE
double-extended floating-point number".

**Repo:** `crates/idl-lower/src/lower.rs::primitive_of` (Arme `Float32`, `Float64`, `Float128`), `crates/float128/src/lib.rs::Float128`, `crates/float128/src/arith.rs` (die vier Grundrechenarten in binary128), `crates/ast/src/model.rs::eval_long_double_at`.

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/idl-lower/tests/floating_constant_arithmetic.rs::idl_7_4_1_4_2_long_double_arithmetic_runs_in_binary128`,
`crates/float128/tests/gcc_binary128_ops.rs::ieee754_5_4_1_the_four_operations_round_as_gcc_float128_does`.

**Status:** `done` — `float` ist IEEE binary32, `double` binary64, `long double` binary128; ein konstanter Ausdruck in `long double` wird in binary128 ausgewertet (Item darunter).

### §7.4.1.4.4.2.2 long double als IEEE double-extended

**Spec:** §7.4.1.4.4.2.2, S. 34 — "The long double data type represents an IEEE double-extended floating-point number, which has an exponent of at least 15 bits in length and a signed fraction of at least 64 bits".

**Repo:** `crates/float128/src/lib.rs` (`Float128`), `crates/float128/src/arith.rs` (`add`, `sub`, `mul`, `div`, jeweils einmal zum nächsten Wert gerundet, bei Gleichstand gerade), `crates/ast/src/model.rs::eval_long_double_root`, `crates/idl-lower/src/lower.rs` (`primitive_of`, Arm `Float128`).

**Tests:** `crates/idl-lower/tests/floating_constant_arithmetic.rs::idl_7_4_1_4_2_long_double_arithmetic_runs_in_binary128`,
`crates/float128/tests/gcc_binary128_ops.rs::ieee754_5_4_1_the_four_operations_round_as_gcc_float128_does`,
`crates/float128/tests/oracle.rs::our_parser_reads_what_gcc_prints`,
`crates/float128/tests/binary128.rs::x87_carries_sixty_four_significand_bits_and_says_when_it_cannot`.

**Status:** `done` — `long double` ist IEEE binary128: 15 Bit Exponent und 113 Bit Signifikand, mehr als die 15 und 64 Bit, die der Absatz verlangt. Konstante Ausdrücke in `long double` laufen in binary128 in `zerodds-float128`, das keinen Typ `f128` des Compilers braucht: `1.0 + 1e-20` behält seine letzte Ziffer, die dieselbe Summe in `double` verliert; ein Teilausdruck außerhalb des binary128-Bereichs ist `EvalError::Overflow`. Addition, Subtraktion, Multiplikation und Division ergeben für 6072 Operandenpaare die Bits, die GCCs `__float128` ergibt (Orakel `crates/float128/tests/oracle/quad_ops.c`).

### §7.4.1.4.4.2.3 char Type

**Spec:** §7.4.1.4.4.2.3, S. 39 — der Typ `char`, ISO Latin-1, ein Byte.

**Repo:** `crates/idl-lower/src/lower.rs::value_disagrees_with` (Arm
`PrimitiveKind::Char`), `crates/idl-cst/src/lex.rs::lex_char`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_constant_whose_value_is_not_of_its_declared_type_is_refused`,
`crates/idl-lower/tests/validation.rs::every_form_the_specification_admits_still_gets_through`,
`crates/idl-cst/tests/lexer.rs::a_raw_non_ascii_char_literal_is_its_code_point_and_not_its_last_octet` —
`lexer.rs:473: assert_eq!(tokens[0].value, TokenValue::Char { value: 0x00E4 });`.

**Status:** done

### §7.4.1.4.4.2.4 Wide Char Type

**Spec:** §7.4.1.4.4.2.4, S. 39 — der Typ `wchar`, ISO 10646.

**Repo:** `crates/idl-lower/src/lower.rs::value_disagrees_with` (Arm
`PrimitiveKind::WChar`), `crates/idl-cst/src/lex.rs::next_token`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_constant_whose_value_is_not_of_its_declared_type_is_refused`,
`crates/idl-lower/tests/validation.rs::the_wide_and_narrow_string_pair_is_the_case_the_clause_names`,
`crates/idl-cst/tests/lexer.rs::wide_literals_are_their_own_kinds` —
`lexer.rs:203: assert_eq!(kinds("L'x'")[0], TokenKind::WCharLiteral);`.

**Status:** done

### §7.4.1.4.4.2.5 Boolean Type

**Spec:** §7.4.1.4.4.2.5, S. 39 — der Typ `boolean`, Wertebereich 0/1,
konstante Werte `TRUE`/`FALSE`.

**Repo:** `crates/idl-lower/src/validate.rs::primitive_range` (Arm
`PrimitiveKind::Boolean`).

**Tests:** `crates/idl-lower/tests/validation.rs::every_form_the_specification_admits_still_gets_through`,
`crates/idl-lower/tests/validation.rs::a_constant_whose_value_is_not_of_its_declared_type_is_refused`,
`crates/idl-lower/tests/validation.rs::a_boolean_discriminator_admits_only_zero_and_one` —
`validation.rs:281: "union U switch (boolean) { case 0: long a; case 1: long b; };",`.

**Status:** done

### §7.4.1.4.4.2.6 Octet Type

**Spec:** §7.4.1.4.4.2.6, S. 39 — der Typ `octet`, ein vorzeichenloses
Byte.

**Repo:** `crates/idl-lower/src/validate.rs::primitive_range` (Arm
`Octet | UInt8`).

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/idl-lower/tests/completion.rs::a_constant_takes_its_declared_type_not_the_expressions`,
`crates/idl-lower/tests/validation.rs::every_form_the_specification_admits_still_gets_through` —
`completion.rs:320: TypeKind::Primitive(zerodds_ast::PrimitiveKind::Octet)`.

**Status:** done

### §7.4.1.4.4.3 Template Types

**Spec:** §7.4.1.4.4.3, S. 39–41 — Sequences, Strings, Wstrings und
Fixed als Schablonen-Typen mit optionaler Schranke.

**Repo:** `crates/idl-parse/src/type_spec.rs` (`parse_sequence_type`,
`parse_string_type`), `crates/idl-lower/src/validate.rs::validate_type`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type`,
`crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`,
`crates/idl-lower/tests/completion.rs::an_unbounded_collection_says_so_rather_than_carrying_a_zero` —
`completion.rs:418: assert!(!bound.is_bounded(), "{name} gained a bound");`.

**Status:** done

### §7.4.1.4.4.3.1 Sequences

**Spec:** §7.4.1.4.4.3.1, S. 39–40 — `sequence < <positive_int_const> >
< type_spec >`; die Schranke muss positiv sein.

**Repo:** `crates/idl-parse/src/type_spec.rs::parse_sequence_type`,
`crates/idl-lower/src/validate.rs::validate_type`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_negative_sequence_bound_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_positive_bound_is_accepted`,
`crates/idl-lower/tests/lowering.rs::a_sequence_member_carries_its_element_type` —
`validation.rs:481: "struct S { sequence<long, -1> a; };",`.

**Status:** done

### §7.4.1.4.4.3.2 Strings

**Spec:** §7.4.1.4.4.3.2, S. 40 — `string < <positive_int_const> >`.

**Repo:** `crates/idl-parse/src/type_spec.rs::parse_string_type`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_zero_string_bound_is_rejected`,
`crates/idl-lower/tests/completion.rs::a_bounded_string_and_sequence_carry_their_bound_expressions`,
`crates/ast/tests/node_space.rs::narrow_and_wide_strings_are_distinct_kinds` —
`validation.rs:475: assert_trips("struct S { string<0> a; };", DiagnosticCode::InvalidBound);`.

**Status:** done

### §7.4.1.4.4.3.3 Wstrings

**Spec:** §7.4.1.4.4.3.3, S. 40 — `wstring < <positive_int_const> >`,
ISO 10646.

**Repo:** `crates/idl-parse/src/type_spec.rs::parse_string_type`
(Zweig `TokenKind::Wstring`).

**Tests:** `crates/idl-lower/tests/validation.rs::a_zero_string_bound_is_rejected`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::adjacent_wide_string_literals_are_one_literal`,
`crates/ast/tests/node_space.rs::narrow_and_wide_strings_are_distinct_kinds`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`bb1-006-const-wstring`) —
`idl_4_2_grammar.rs:74: ConstValue::WString("abcd".chars().map(u32::from).collect())`.

**Status:** done

### §7.4.1.4.4.3.4 Fixed Type

**Spec:** §7.4.1.4.4.3.4, S. 40–41 — `fixed < <positive_int_const> ,
<positive_int_const> >` mit höchstens 31 Stellen; die Skala darf die
Stellenzahl nicht übersteigen.

**Repo:** `crates/idl-lower/src/validate.rs::fixed_rules` mit
`MAX_FIXED_DIGITS`, `crates/idl-lower/src/expr.rs` (Fixed-Arithmetik).

**Tests:** `crates/idl-lower/tests/validation.rs::a_fixed_with_more_than_thirty_one_digits_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_fixed_whose_scale_exceeds_its_digits_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_fixed_within_its_ranges_is_accepted`,
`crates/idl-lower/tests/fixed_constant_arithmetic.rs::a_sum_lines_the_points_up_and_drops_the_insignificant_zero`,
`crates/idl-lower/tests/fixed_constant_arithmetic.rs::a_product_carries_both_scales`,
`crates/idl-lower/tests/fixed_constant_arithmetic.rs::a_quotient_is_exact_or_cut_without_rounding`,
`crates/idl-lower/tests/lowering.rs::table_7_11_decides_the_type_of_a_fixed_expression`,
`crates/ast/tests/node_space.rs::fixed_retains_digits_and_scale` —
`validation.rs:503: assert_trips("struct S { fixed<32,2> a; };", DiagnosticCode::InvalidBound);`.

**Status:** done

### §7.4.1.4.4.4 Constructed Types

**Spec:** §7.4.1.4.4.4, S. 41–43 — Structures, Unions, Enumerations,
Forward Declarations und Native als konstruierte Typen.

**Repo:** `crates/idl-lower/src/lower.rs` (`structure`, `union`,
`enumeration`, `native`), `crates/idl-lower/src/validate.rs::union_rules`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/ast/tests/node_space.rs::struct_members_land_in_declaration_order_with_the_right_parent`,
`crates/idl-lower/tests/spans.rs::a_struct_member_names_its_declarator_and_not_the_space_before_it` —
`lowering.rs:492: assert_eq!(names, vec!["IDLE", "ACTIVE", "FAULT"]);`.

**Status:** done

### §7.4.1.4.4.4.1 Structures

**Spec:** §7.4.1.4.4.4.1, S. 41–42 — `struct <identifier> { <member> * }`,
eine Liste von `<member>`, jede mit eigenem Declarator.

**Repo:** `crates/idl-lower/src/lower.rs::structure`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_carries_its_members_in_order`,
`crates/idl-lower/tests/lowering.rs::several_declarators_share_the_type_and_become_separate_members`,
`crates/idl-lower/tests/lowering.rs::an_exception_carries_its_members`,
`crates/idl-lower/tests/spans.rs::the_second_declarator_of_a_list_names_its_own_line` —
`lowering.rs:304: assert_eq!(names, vec!["x", "y", "z"]);`.

**Status:** done

### §7.4.1.4.4.4.2 Unions und die Regeln zu Case-Labels

**Spec:** §7.4.1.4.4.4.2, S. 42 — "If the discriminator value was
explicitly listed in a case statement, the value of the element associated
with the case label is the discriminator"; ferner: kein Label darf zweimal
vorkommen, und es darf höchstens ein `default` geben.

**Repo:** `crates/idl-lower/src/validate.rs::union_rules`,
`crates/idl-lower/src/lower.rs::union`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_union_over_an_enumeration_lowers_with_its_labels_evaluated`,
`crates/idl-lower/tests/lowering.rs::a_case_label_may_name_its_enumerator_by_a_qualified_name`,
`crates/idl-lower/tests/validation.rs::a_repeated_case_label_is_rejected`,
`crates/idl-lower/tests/validation.rs::distinct_case_labels_are_accepted`,
`crates/idl-lower/tests/validation.rs::two_default_cases_are_rejected`,
`crates/idl-lower/tests/spans.rs::a_repeated_case_label_is_reported_on_the_branch_that_repeats_it`,
`crates/ast/tests/node_space.rs::a_default_case_is_recorded_on_the_union_as_well_as_in_the_case_list`,
`crates/ast/tests/node_space.rs::a_case_may_carry_several_labels` —
`lowering.rs:193: assert_eq!(labels, [0, 1, 2]);`.

**Status:** done

### §7.4.1.4.4.4.3 Enumerations

**Spec:** §7.4.1.4.4.4.3, S. 42 — `enum <identifier> { <enumerator>
{ "," <enumerator> } * }`; ein Enumerator kann einen expliziten Wert
tragen, die folgenden laufen von da an weiter.

**Repo:** `crates/ast/src/query.rs::effective_enumerator_value`,
`crates/idl-lower/src/lower.rs::enumeration`.

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enum_carries_its_enumerators_in_order`,
`crates/idl-lower/tests/lowering.rs::an_enumerator_evaluates_inside_a_larger_expression`,
`crates/idl-lower/tests/completion.rs::enumerators_without_values_run_from_zero_upwards`,
`crates/idl-lower/tests/completion.rs::an_explicit_enumerator_value_shifts_the_ones_after_it`,
`crates/ast/tests/node_space.rs::a_positional_enumerator_and_an_explicit_value_are_distinguishable` —
`completion.rs:271: assert_eq!(ast.effective_enumerator_value(enumerators[2]), Some(11));`.

**Status:** done

### §7.4.1.4.4.4.4 Constructed Recursive Types und Forward Declarations

**Spec:** §7.4.1.4.4.4.4, S. 42–43 — eine Struktur darf sich nur über ein
Element enthalten, das einen Wert terminiert; die Forward Declaration
`struct <identifier> ;` wird von der Definition vervollständigt.

**Repo:** `crates/idl-lower/src/validate.rs::forward_declarations_are_defined`,
`crates/ast-layout/src/xcdr.rs::types_that_close_an_inadmissible_cycle`,
`crates/idl-compose/src/lib.rs::compose_source_with`.

**Tests:** `crates/idl-compose/tests/recursion.rs::idl_7_4_1_4_4_4_4_a_cycle_with_no_terminator_on_it_is_refused`,
`crates/idl-compose/tests/recursion.rs::idl_7_4_1_4_4_4_4_a_cycle_with_a_terminator_on_it_is_admitted`,
`crates/idl-compose/tests/recursion.rs::idl_7_4_1_4_4_4_4_the_diagnostic_names_the_type_and_the_clause`,
`crates/idl-compose/tests/recursion.rs::idl_7_4_1_4_4_4_4_the_interop_corpus_case_for_this_is_refused_and_its_neighbours_are_not`,
`crates/idl-compose/tests/recursion.rs::idl_7_4_1_4_4_4_4_a_forward_declared_cycle_is_reported_once`,
`crates/idl-lower/tests/lowering.rs::a_forward_declaration_on_its_own_is_forward`,
`crates/idl-lower/tests/lowering.rs::the_definition_completes_the_forward_node_rather_than_adding_a_second`,
`crates/idl-lower/tests/validation.rs::a_forward_declaration_never_defined_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_forward_declaration_that_is_defined_is_accepted` —
`recursion.rs:104: assert!(message.contains("Ma::Sa"), "{message}");`.

**Status:** done

### §7.4.1.4.4.6 Native Types

**Spec:** §7.4.1.4.4.6, S. 43 — `native <identifier> ;` deklariert einen
nativen Typ; die Verwendung als Member-Typ-Spec steht in Produktion 62.

**Repo:** `crates/idl-lower/src/lower.rs::native`, `crates/idl-parse/src/decl.rs::parse_native`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_native_declaration_becomes_a_native_type`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_1_4_4_6_a_native_type_is_used_by_its_name_wherever_a_type_is`.

**Status:** `done` — `native Handle;` deklariert einen Typ der Art `Native`, und ein Member, ein `typedef` und ein Parameter eines lokalen Interfaces, die ihn nennen, lösen auf ihn auf.

### §7.4.1.4.4.7 Naming Data Types (Produktion 64)

**Spec:** §7.4.1.4.4.7, S. 43 — (64) `<type_declarator> ::=
{ <simple_type_spec> | <template_type_spec> | <constr_type_dcl> }
<any_declarators>`; die dritte Alternative erlaubt die Konstruktion im
`typedef` selbst.

**Repo:** `crates/idl-parse/src/decl.rs::parse_typedef`,
`crates/idl-lower/src/lower.rs::typedef`.

**Tests:** `crates/idl-lower/tests/idl_4_2_grammar.rs::a_struct_defined_inside_a_typedef_is_declared_and_aliased`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::every_constructed_form_can_be_defined_inside_a_typedef`,
`crates/idl-lower/tests/lowering.rs::a_typedef_becomes_an_alias_to_its_target`,
`crates/idl-lower/tests/lowering.rs::one_typedef_may_declare_several_names`,
`crates/idl-lower/tests/lowering.rs::a_typedef_declarator_keeps_its_array_dimensions`,
`crates/idl-lower/tests/lowering.rs::dimensions_belong_to_the_declarator_and_not_to_the_typedef` —
`idl_4_2_grammar.rs:153: TypeKind::Alias { aliased } => assert_eq!(aliased.resolved(), Some(inline)),`.

**Status:** done

### §7.4.1.5 Specific Keywords

**Spec:** §7.4.1.5, S. 43–44 — Tabelle 7-27: die Keywords dieses Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::KEYWORDS`,
`crates/idl-lower/src/lower.rs::identifiers_do_not_collide_with_keywords`.

**Tests:** `crates/idl-lower/tests/validation.rs::an_identifier_differing_from_a_keyword_only_in_case_is_refused`,
`crates/idl-lower/tests/validation.rs::the_diagnostic_names_the_keyword_and_the_clause`,
`crates/idl-lower/tests/validation.rs::the_underscore_escape_turns_the_keyword_check_off` —
`validation.rs:619: DiagnosticCode::KeywordCollision,`.

**Status:** done

---

## §7.4.2 Building Block Any

### §7.4.2.1 Purpose und §7.4.2.2 Dependencies

**Spec:** §7.4.2.1 / §7.4.2.2, S. 43–44 — der Baustein führt `any` ein
und stützt sich auf den Building Block Core Data Types.

**Repo:** `crates/idl-parse/src/type_spec.rs`,
`crates/ast/src/node/type_.rs::TypeKind`.

**Tests:** —

**Status:** `n/a (informative)` — die beiden Klauseln begründen und
benennen die Abhängigkeit; der Typ selbst steht in §7.4.2.3.

### §7.4.2.3 Syntax — der Typ `any`

**Spec:** §7.4.2.3, S. 44 — (69) `<type_spec> ::+ <any_type>`,
(70) `<type_declarator> ::= <simple_type_spec> <any_declarators>`;
`any` steht an jeder Stelle, an der `<type_spec>` steht.

**Repo:** `crates/ast/src/node/type_.rs::TypeKind::Any`,
`crates/idl-lower/src/lower.rs::primitive_of`.

**Tests:** `crates/idl-lower/tests/lowering.rs::an_any_member_reaches_the_model_as_a_type_rather_than_a_broken_reference`,
`crates/idl-lower/tests/lowering.rs::every_any_in_a_file_shares_one_built_in_node`,
`crates/idl-lower/tests/lowering.rs::any_is_reachable_in_every_position_the_grammar_gives_it` —
`lowering.rs:702: assert_eq!(lowered.ast.type_(type_).expect("any").kind, TypeKind::Any);`.

**Status:** done — das Modell trägt `any`; kein Sprachbackend emittiert
eine Sprachform dafür (`NoLanguageForm { construct: "any" }`, siehe
[`idl-4.2.gaps.md`](idl-4.2.gaps.md) §C).

### §7.4.2.5 Specific Keywords — `any`, `Object`, `ValueBase`

**Spec:** §7.4.2.5, S. 44 — die Keywords des Bausteins sind `any`,
`Object`, `ValueBase`; alle drei sind eingebaute Typnamen.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`,
`crates/ast/src/node/type_.rs::TypeKind`.

**Tests:** `crates/idl-lower/tests/lowering.rs::none_of_the_three_built_in_type_names_leaves_a_broken_reference`,
`crates/idl-lower/tests/lowering.rs::an_object_parameter_reaches_the_model_as_an_object_reference`,
`crates/idl-lower/tests/lowering.rs::object_is_reachable_in_every_position_rule_118_gives_it`,
`crates/idl-lower/tests/lowering.rs::a_valuebase_parameter_reaches_the_model_as_valuebase`,
`crates/idl-lower/tests/lowering.rs::valuebase_is_reachable_in_every_position_rule_132_gives_it` —
`lowering.rs:768: TypeKind::ObjectRef { interface: None }`.

**Status:** done

---

## §7.4.3 Building Block Interfaces – Basic

### §7.4.3.3 Syntax — interface, exception, forward

**Spec:** §7.4.3.3, S. 45–47 — (73)–(85): `interface_dcl`,
`interface_header`, `interface_body`, `export`, `interface_forward_dcl`,
`exception_dcl`.

**Repo:** `crates/idl-parse/src/decl.rs::parse_interface`,
`crates/idl-lower/src/lower.rs::interface`.

**Tests:** `crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised`,
`crates/idl-parse/tests/parse.rs::an_interface_with_operations_and_attributes_parses`,
`crates/idl-lower/tests/lowering.rs::an_exception_carries_its_members`,
`crates/idl-lower/tests/conformance.rs::every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares` —
`parse.rs:156: assert!(found.contains(&CstKind::Operation));`.

**Status:** done

### §7.4.3.4.2 Exceptions

**Spec:** §7.4.3.4.2, S. 48–49 — eine Exception ist ein Aggregat mit
Mitgliedern; ihr Name wird in den umgebenden Scope eingeführt.

**Repo:** `crates/idl-lower/src/lower.rs::structure` (Zweig Exception),
`crates/idl-lower/src/scope.rs::ScopeStack`.

**Tests:** `crates/idl-lower/tests/lowering.rs::an_exception_carries_its_members`,
`crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised` —
`lowering.rs:571: assert_eq!(any.kind(), zerodds_ast::NodeKind::Exception);`.

**Status:** done

### §7.4.3.4.3.1 Interface Header

**Spec:** §7.4.3.4.3.1, S. 49 — `interface <identifier>
[ : <interface_name> { "," <interface_name> } * ]`.

**Repo:** `crates/idl-lower/src/lower.rs::interface_bases`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_component_keeps_its_supports_clause_apart_from_its_base`,
`crates/idl-lower/tests/conformance.rs::every_fixture_produces_a_model_with_declarations_in_it` —
`lowering.rs:1392: assert_eq!(component.bases.len(), 1, "the base list took the supports");`.

**Status:** done

### §7.4.3.4.3.2 Interface Inheritance

**Spec:** §7.4.3.4.3.2, S. 49 — Basis-Interfaces werden im Header genannt;
jedes geerbte Element ist im abgeleiteten Interface sichtbar.

**Repo:** `crates/idl-lower/src/lower.rs::interface_bases`,
`crates/ast/src/node/interface.rs::InterfaceNode`.

**Tests:** `crates/lang-rules/tests/inherited_operations.rs::idl_7_4_3_4_3_2_the_cpp_class_reaches_the_bases_operation_through_its_base_clause`,
`crates/lang-rules/tests/inherited_operations.rs::idl_7_4_3_4_3_2_the_java_interface_reaches_the_bases_operation_through_extends`,
`crates/lang-rules/tests/inherited_operations.rs::idl_7_4_3_4_3_2_the_rust_trait_reaches_the_bases_operation_through_a_supertrait`,
`crates/lang-rules/tests/inherited_operations.rs::idl_7_4_3_4_3_2_every_incomplete_language_refuses_rather_than_flattens_or_drops` —
`inherited_operations.rs:102: assert!(out.contains("class Derived : public Base {"), "{out}");`.

**Status:** done

### §7.4.3.4.3.2.1 Inheritance Rules

**Spec:** §7.4.3.4.3.2.1, S. 49–50 — die Liste der Basis-Interfaces darf
keinen Namen zweimal nennen; die Vererbung ist transitiv; `virtual` und
nicht-`virtual` werden unterschieden.

**Repo:** `crates/idl-lower/src/validate.rs::interface_inheritance_rules`, `crates/idl-lower/src/scope.rs::ScopeStack::find`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_3_4_3_2_1_a_direct_base_is_named_once_and_no_operation_is_redefined`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_1_an_inherited_identifier_is_visible_under_every_global_name`.

**Status:** `done` — die Raute aus Figure 7-1 und `E: A, B` werden angenommen; eine zweimal genannte direkte Basis, eine in einem abgeleiteten Interface neu definierte Operation oder ein Attribut (`make_it_so` des Absatzes) und zwei verschiedene geerbte Operationen eines Namens werden abgewiesen (`IDL0021`); die Elemente einer direkten oder indirekten Basis sind im abgeleiteten Interface sichtbar. IDL 4.2 kennt kein Schlüsselwort `virtual`.

### §7.4.3.4.3.3.1 Operations

**Spec:** §7.4.3.4.3.3.1, S. 50–51 — `<parameter_dcl>` mit `in`, `out`,
`inout`; `oneway`; `raises ( <exception_identifier> { "," … } * )`;
`context ( "<string_literal>" { "," … } * )`.

**Repo:** `crates/idl-lower/src/lower.rs::operation`, `crates/ast/src/model.rs::add_context`, `crates/ast/src/node/interface.rs::OperationNode`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_3_4_3_3_1_an_operation_keeps_its_directions_raises_and_context`,
`crates/corba-codegen/tests/model_connection.rs::oneway_survives_lowering`,
`crates/corba-codegen/tests/model_connection.rs::a_raises_clause_reaches_the_stub`.

**Status:** `done` — eine aus IDL abgesenkte Operation behält `in`, `out` und `inout` je Parameter, ihre `raises`-Klausel aufgelöst auf die Exception und die Strings ihrer `context`-Klausel in Reihenfolge.

### §7.4.3.4.3.3.2 Attributes

**Spec:** §7.4.3.4.3.3.2, S. 51–52 — `attribute <parameter_mode>
<type_spec> <identifier> ;` mit `readonly`, sowie `getraises`/`setraises`
als eigene Attributformen.

**Repo:** `crates/idl-lower/src/lower.rs::attribute`, `crates/ast/src/model.rs::add_attribute_raises`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_3_4_3_3_2_an_attribute_keeps_its_getraises_and_setraises`,
`crates/idl-parse/tests/parse.rs::an_interface_with_operations_and_attributes_parses`,
`crates/lang-rules/tests/interface_bindings.rs::rpc_7_5_1_1_3_an_attribute_implies_the_operations_of_seven_five_one_one_three`.

**Status:** `done` — `getraises` und `setraises` erreichen `get_raises` und `set_raises` des Attributs, aufgelöst auf ihre Exceptions; das `raises` eines `readonly`-Attributs erreicht `get_raises` und lässt `set_raises` leer.

### §7.4.3.4.3.4 Forward Declaration

**Spec:** §7.4.3.4.3.4, S. 52 — `interface <identifier> ;` ist die
Forward Declaration; die spätere Definition ergänzt denselben Knoten.

**Repo:** `crates/idl-lower/src/lower.rs::forward_interface_kind`,
`crates/ast/src/model.rs::add_forward_interface`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_forward_declared_interface_is_completed_by_its_definition`,
`crates/idl-lower/tests/lowering.rs::a_forward_interface_keeps_the_form_its_header_wrote`,
`crates/idl-lower/tests/lowering.rs::an_interface_forward_that_is_never_defined_is_still_reported`,
`crates/idl-lower/tests/lowering.rs::a_reference_taken_against_a_forward_interface_survives_the_definition`,
`crates/idl-lower/tests/lowering.rs::a_second_full_interface_definition_is_a_duplicate_not_a_completion`,
`crates/idl-lower/tests/spans.rs::an_unfinished_forward_interface_points_at_the_declaration_too`,
`crates/lang-rules/tests/interface_bindings.rs::ddsts_11_7_a_forward_declaration_without_a_definition_is_e002` —
`lowering.rs:1070: assert_eq!(lowered.ast.operations_of(id).len(), 1);`.

**Status:** done

### §7.4.3.5 Specific Keywords

**Spec:** §7.4.3.5, S. 52–53 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`,
`crates/idl-cst/tests/lexer.rs::a_keyword_is_recognised_only_in_the_spelling_table_7_6_gives` —
`lexer.rs:59: assert_eq!(kinds("custom")[0], TokenKind::Custom);`.

**Status:** done

---

## §7.4.4 Building Block Interfaces – Full

### §7.4.4.4 Ein Interface als Typ

**Spec:** §7.4.4.4, S. 53 — ein Interface-Name darf an jeder Stelle
stehen, an der `<type_spec>` steht, und löst zu einer Objektreferenz auf.

**Repo:** `crates/idl-lower/src/lower.rs::type_ref_from_scoped_name`,
`crates/idl-lower/tests/interface_as_type.rs`.

**Tests:** `crates/idl-lower/tests/interface_as_type.rs::an_interface_used_as_a_type_resolves_in_every_spelling`,
`crates/idl-lower/tests/interface_as_type.rs::what_it_resolves_to_is_an_object_reference_and_not_the_interface`,
`crates/idl-lower/tests/interface_as_type.rs::the_key_is_the_declaration_s_own_scoped_name_and_not_the_written_segment`,
`crates/idl-lower/tests/interface_as_type.rs::a_name_that_is_not_declared_still_breaks` —
`interface_as_type.rs:109: assert_eq!(ast.interface(interface).expect("I").name, "I");`.

**Status:** done

### §7.4.4.4 Mehrfachvererbung und Mehrdeutigkeit

**Spec:** §7.4.4.4, S. 53 — ein Element, das über zwei Wege geerbt wird,
gilt nicht als mehrdeutig; zwei verschiedene Deklarationen desselben
Namens in der Sichtbarkeit dagegen schon.

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::find`, `crates/idl-lower/src/lower.rs::use_scoped` (`AmbiguousName`).

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_4_4_a_name_declared_in_two_bases_is_ambiguous_unless_qualified`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_1_an_inherited_identifier_is_visible_under_every_global_name`,
`crates/lang-rules/tests/interface_bindings.rs::idl_7_4_4_4_all_operations_flattened_keeps_a_non_readonly_attributes_setter`.

**Status:** `done` — das Beispiel des Absatzes: `typedef L1 L2;` in `C: B, A` wird als mehrdeutig gemeldet (`IDL0009`), `A::L1` löst auf `long` auf und `B::L1` auf `short`; eine über zwei Wege einer Raute erreichte Deklaration ist eine und keine Mehrdeutigkeit.

### §7.4.4.4 Attribut und Operation mit demselben Namen

**Spec:** §7.4.4.4, S. 53 — Attribut- und Operationsnamen teilen sich
einen Namespace innerhalb des Interface-Rumpfs.

**Repo:** `crates/idl-lower/src/lower.rs::one_namespace_per_scope`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_4_4_an_attribute_and_an_operation_of_one_name_collide`.

**Status:** `done` — ein Attribut und eine Operation desselben Namens in einem Interface, auch nur in der Schreibweise verschieden, sind `IDL0008`.

### §7.4.4.5 Specific Keywords

**Spec:** §7.4.4.5, S. 53 — "There are no additional keywords with this
building block."

**Repo:** `crates/idl-cst/src/token.rs::KEYWORDS`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`.

**Status:** `n/a (informative)` — die Klausel sagt selbst, dass es keine
zusätzlichen Keywords gibt; es gibt nichts zu prüfen.

---

## §7.4.5 Building Block Value Types

### §7.4.5.3 Syntax

**Spec:** §7.4.5.3, S. 54 — (98)–(110): `value_dcl`, `value_def`,
`value_header`, `value_element` (ein `<export>`, ein `<state_member>` oder ein
`<init_dcl>`), `state_member`, `init_dcl`, `value_forward_dcl`.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype`,
`crates/idl-lower/src/lower.rs::valuetype_kind`.

**Tests:** `crates/idl-lower/tests/dialects.rs::a_valuetype_keeps_its_modifier_bases_and_supported_interfaces`,
`crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised`,
`crates/idl-lower/tests/conformance.rs::every_fixture_lowers_to_exactly_the_diagnostics_it_should`,
`crates/idl-lower/tests/dialects.rs::idl_7_4_5_3_an_attribute_in_a_value_type_body_is_an_attribute_of_it`,
`crates/corba-rust/tests/compile_check.rs::idl_7_4_5_3_a_value_types_attributes_are_methods_of_its_trait` —
`dialects.rs:550: assert_eq!(state.len(), 2, "a state member was lost");`.

**Status:** `done` — ein Attribut im Rumpf eines Value Types (Regel (105), `<export>`) ist ein Attribut des Value Types (`AttributeNode::parent` ist ein `AnyId`); `zerodds-corba-rust` deklariert Getter und, außer bei `readonly`, Setter im Trait des Value Types, der Compiler bestätigt beides und den fehlenden Setter.

### §7.4.5.4.1 Concrete (Stateful) Value Types

**Spec:** §7.4.5.4.1, S. 55 — ein konkreter Value Type trägt
State-Members und Modifikatoren (`custom`, `abstract`, `truncatable`).

**Repo:** `crates/idl-lower/src/lower.rs::valuetype`,
`crates/ast/src/node/type_.rs::ValuetypeKind`.

**Tests:** `crates/idl-lower/tests/dialects.rs::a_valuetype_keeps_its_modifier_bases_and_supported_interfaces`,
`crates/idl-lower/tests/lowering.rs::an_abstract_valuetype_parses_as_a_valuetype`,
`crates/ccm/tests/equivalent_idl.rs::a_plain_valuetype_is_not_an_event_type` —
`lowering.rs:1316: assert_eq!(kind, zerodds_ast::ValuetypeKind::Abstract);`.

**Status:** done

### §7.4.5.4.1.2 Value Inheritance Specification

**Spec:** §7.4.5.4.1.2, S. 55 — der Header erlaubt eine Basis
(`:` <value_type>) und `supports` <interface_identifier>.

**Repo:** `crates/idl-lower/src/lower.rs::interface_bases` (die
`supports`-Alternative).

**Tests:** `crates/idl-lower/tests/dialects.rs::a_valuetype_keeps_its_modifier_bases_and_supported_interfaces`,
`crates/idl-lower/tests/lowering.rs::a_component_keeps_its_supports_clause_apart_from_its_base` —
`lowering.rs:1398: assert_eq!(supports, 2, "the supports clause was dropped");`.

**Status:** done

### §7.4.5.4.1.3.1 State Members

**Spec:** §7.4.5.4.1.3.1, S. 55 — `public <member_type_spec>
<simple_declarator>` und `private …`; die Sichtbarkeit gehört zum Value
Type, nicht zum Struct-Member.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype` (Zweig State-Member),
`crates/ast/src/facet/corba.rs::state_visibility`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_state_member_keeps_the_keyword_it_was_written_with`,
`crates/idl-lower/tests/lowering.rs::a_struct_member_has_no_state_visibility` —
`lowering.rs:1414: Some(zerodds_ast::StateVisibility::Public),`.

**Status:** done

### §7.4.5.4.1.3.2 Initializers

**Spec:** §7.4.5.4.1.3.2, S. 55–56 — `factory <identifier> ( … ) ;` wird zu
einer Operation des Value Type.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype_operation`, `crates/idl-parse/src/decl.rs::parse_factory`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_5_4_1_3_2_a_factory_is_an_initializer_of_its_value_type`,
`crates/idl-lower/tests/dialects.rs::a_valuetype_initialiser_becomes_an_operation_of_it`.

**Status:** `done` — `factory create(in long initial, in string name) raises (Bad);` und `factory empty();` sind die Initialisierer des Value Types in Reihenfolge, mit ihren `in`-Parametern und der `raises`-Klausel, und zählen nicht als Zustand.

### §7.4.5.4.2 Forward Declarations

**Spec:** §7.4.5.4.2, S. 56 — `valuetype <identifier> ;` wird von der
Definition vervollständigt.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype_kind`,
`crates/ast/src/model.rs::add_forward_interface`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_forward_declared_valuetype_is_completed_by_its_definition`,
`crates/idl-lower/tests/lowering.rs::a_valuetype_forward_that_is_never_defined_is_still_reported`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`bb5-110-value-forward`) —
`lowering.rs:1361: assert_eq!(declared.len(), 1, "{source:?}: two nodes for one name");`.

**Status:** done

### §7.4.5.5 Specific Keywords

**Spec:** §7.4.5.5, S. 56 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier` —
`lexer.rs:73: assert_eq!(kinds("TRUE")[0], TokenKind::True);`.

**Status:** done

---

## §7.4.6 Building Block CORBA-Specific – Interfaces

### §7.4.6.3 Syntax — repository directives, local, native

**Spec:** §7.4.6.3, S. 58–59 — (112)–(124): `type_id_dcl`,
`type_prefix_dcl`, `import_dcl`, `local`, `native`, `context_dcl`,
`context_spec`, `op_oneway_dcl`.

**Repo:** `crates/idl-lower/src/complete.rs::apply_repository_directives`,
`crates/idl-parse/src/decl.rs` (Zweige `typeid`, `typeprefix`, `import`).

**Tests:** `crates/idl-lower/tests/idl_4_2_grammar.rs::typeprefix_and_import_inside_an_interface_body_parse`,
`crates/idl-lower/tests/lowering.rs::a_native_declaration_becomes_a_native_type`,
`crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised` —
`lowering.rs:578: assert_eq!(ast.type_(handle).expect("Handle").kind, TypeKind::Native);`.

**Status:** done

### §7.4.6.4.1.1 Repository Identity Declaration (`typeid`)

**Spec:** §7.4.6.4.1.1, S. 59 — `typeid <identifier> <string_literal> ;`
setzt die Repository-Id unmittelbar.

**Repo:** `crates/idl-lower/src/complete.rs::derive_repository_ids`,
`crates/corba-codegen/src/repository_id.rs`.

**Tests:** `crates/idl-lower/tests/dialects.rs::a_typeid_directive_sets_the_id_outright`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::a_typeid_inside_an_interface_body_sets_the_id`,
`crates/idl-lower/tests/dialects.rs::a_pragma_id_sets_the_repository_id_like_typeid_does`,
`crates/corba-codegen/tests/model_connection.rs::an_interfaces_repository_id_comes_from_the_model` —
`idl_4_2_grammar.rs:188: Some("IDL:Ia:2.0")`.

**Status:** done

### §7.4.6.4.1.2 Repository Identifier Prefix Declaration (`typeprefix`)

**Spec:** §7.4.6.4.1.2, S. 59–60 — `typeprefix <scoped_name>
<string_literal> ;` prägt alle darunter abgeleiteten Ids.

**Repo:** `crates/idl-lower/src/complete.rs::apply_repository_directives`
und `derived_repository_id`, `crates/corba-codegen/src/repository_id.rs`.

**Tests:** `crates/idl-lower/tests/dialects.rs::a_typeprefix_directive_prefixes_the_ids_derived_below_it`,
`crates/idl-lower/tests/dialects.rs::a_typeprefix_may_name_the_module_it_is_written_in`,
`crates/idl-lower/tests/dialects.rs::the_root_anchored_spelling_names_the_same_module`,
`crates/idl-lower/tests/dialects.rs::a_sibling_declaration_wins_over_one_further_out`,
`crates/idl-lower/tests/dialects.rs::a_directive_naming_nothing_is_still_reported`,
`crates/idl-lower/tests/dialects.rs::a_pragma_prefix_prefixes_the_ids_of_its_module`,
`crates/corba-codegen/tests/model_connection.rs::a_prefix_reaches_an_interface_and_an_exception` —
`model_connection.rs:447: assert_eq!(resolved.id, "IDL:omg.org/CosNaming/NamingContext:1.0");`.

**Status:** done

### §7.4.6.4.1.3 Repository Id Conflict

**Spec:** §7.4.6.4.1.3, S. 60 — zwei Deklarationen, die dieselbe
Repository-Id ergeben, sind ein Konflikt.

**Repo:** `crates/idl-lower/src/complete.rs::apply_repository_directives`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_6_4_1_3_a_pragma_and_a_typeid_disagreeing_on_one_element_conflict`.

**Status:** `done` — ein `#pragma ID` und ein `typeid`, die einem Element zwei verschiedene Repository-Ids geben, und ein `#pragma prefix` und ein `typeprefix`, die einem Scope zwei Präfixe geben, sind `IDL0013`; übereinstimmende werden angenommen.

### §7.4.6.4.1.4 Imports

**Spec:** §7.4.6.4.1.4, S. 60–61 — `import <scoped_name> ;` bleibt wie
geschrieben im Modell.

**Repo:** `crates/idl-lower/src/lower.rs::interface` (Zweig Import),
`crates/idl-parse/src/decl.rs`.

**Tests:** `crates/idl-lower/tests/dialects.rs::an_import_is_kept_as_written`,
`crates/idl-lower/tests/dialects.rs::a_declaration_in_an_interface_is_named_through_it`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::typeprefix_and_import_inside_an_interface_body_parse` —
`dialects.rs:349: assert_eq!(imports, vec!["IDL:Other:1.0".to_string()]);`.

**Status:** done

### §7.4.6.4.3 Local Interfaces

**Spec:** §7.4.6.4.3, S. 62 — `local interface <identifier> …` ist ein
Interface ohne Endpunkte.

**Repo:** `crates/idl-lower/src/lower.rs::interface_bases` (Setzen von
`InterfaceKind::Local`), `crates/lang-rules/src/adapter/interface.rs::Service::is_remote`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_component_form_reaches_its_own_interface_kind`,
`crates/idl-lower/tests/lowering.rs::a_forward_interface_keeps_the_form_its_header_wrote`,
`crates/corba-codegen/tests/annex_a1_traits.rs::table_a2_reads_local_and_abstract_off_the_interface_declaration`,
`crates/lang-rules/tests/interface_bindings.rs::rpc_7_3_1_1_a_local_interface_is_refused_by_the_seam` —
`interface_bindings.rs:742: assert!(interface_cpp::cpp_interface_items(&ast).is_err());`.

**Status:** done — ein `local interface` hat keine Endpunkte: die Dienst-Bindung weist ihn ab (DDS-RPC 1.0 §7.3.1.1 Produktion 7) und keine Sprache schreibt eine Klasse dafür.

### §7.4.6.4.4 Use of Native Types

**Spec:** §7.4.6.4.4, S. 62–63 — ein nativer Typ darf an jeder Stelle
stehen, an der ein Typ verlangt wird.

**Repo:** `crates/idl-lower/src/validate.rs::native_parameters_stay_local`, `crates/idl-lower/src/lower.rs::NativeInRemoteInterface`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_6_4_4_a_native_parameter_is_refused_in_a_remote_interface`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_1_4_4_6_a_native_type_is_used_by_its_name_wherever_a_type_is`.

**Status:** `done` — ein nativer Parameter in einer Operation eines Interfaces, abstrakten Interfaces, einer Komponente oder eines Homes ist `IDL0019`, in einem lokalen Interface wird er angenommen. TAO und JacORB nehmen den entfernten Fall an (`crates/corba-codegen/tests/orb-corpus/recorded/`), omniORB weist ihn ab; `Options::native_in_remote_interfaces` lässt ihn für einen Aufrufer zu, der sie nachbildet.

### §7.4.6.4.6 Context Expressions

**Spec:** §7.4.6.4.6, S. 63 — `context ( "<string>" { "," <string> } * )`
an einer Operation; die Zeichenketten sind Ausdrücke.

**Repo:** `crates/idl-lower/src/lower.rs::operation` (`OperationNode::contexts`), `crates/idl-lower/src/validate.rs::context_expressions_are_well_formed`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_3_4_3_3_1_an_operation_keeps_its_directions_raises_and_context`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_6_4_6_a_context_string_is_not_empty_and_ends_in_at_most_one_star`.

**Status:** `done` — die Strings einer `context`-Klausel erreichen die Operation in Reihenfolge; ein leerer String, ein alleinstehendes `*`, zwei `*` und ein `*`, das nicht am Ende steht, sind `IDL0001`.

### §7.4.6.4.7 CORBA Module

**Spec:** §7.4.6.4.7, S. 63–64 — "Names defined by the CORBA specification are in a module named CORBA. In an IDL specification, however, IDL keywords such as Object must not be preceded by a "CORBA::" prefix." "The file orb.idl contains the IDL definitions for the CORBA module. Except for CORBA::TypeCode, the file orb.idl must be included in IDL files that use names defined in the CORBA module. IDL files that use CORBA::TypeCode may obtain its definition by including either the file orb.idl or the file TypeCode.idl."

**Repo:** `crates/idl-parse/src/preprocess/mod.rs::builtin_include`, `crates/idl-parse/src/preprocess/corba/` (`orb.idl` und die `CORBA_*.idl`-Dateien, die es einbindet, wie die OMG sie mit CORBA 3.3 veröffentlicht; `TypeCode.idl`).

**Tests:** `crates/idl-compose/tests/corba_module.rs::idl_7_4_6_4_7_the_corba_module_comes_from_orb_idl_and_typecode_idl`.

**Status:** `done` — `#include <orb.idl>` bringt das Modul CORBA, `#include <TypeCode.idl>` bringt `CORBA::TypeCode`; das Beispiel des Absatzes ergibt seine drei Antworten (`CORBA::Object` ist ein Fehler, ein bloßes `TypeCode` ist undefiniert, `CORBA::TypeCode` wird aufgelöst); ohne `orb.idl` ist ein Name des Moduls undefiniert. Eine `orb.idl`, die der Loader des Aufrufers findet, wird statt der eingebauten gelesen.

### §7.4.6.5 Specific Keywords

**Spec:** §7.4.6.5, S. 64 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier` —
`lexer.rs:73: assert_eq!(kinds("TRUE")[0], TokenKind::True);`.

**Status:** done

---

## §7.4.7 Building Block CORBA-Specific – Value Types

### §7.4.7.3 Syntax

**Spec:** §7.4.7.3, S. 64–65 — die Werttyp-Produktionen mit `custom`,
`abstract`, `truncatable`, dem Value-Box und dem `ValueBase`.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype_kind`,
`crates/idl-lower/src/lower.rs::value_box`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_value_box_reaches_the_model_as_one`,
`crates/idl-lower/tests/lowering.rs::a_value_box_may_box_any_type_spec`,
`crates/idl-lower/tests/lowering.rs::the_other_valuetype_forms_are_unaffected_by_the_box`,
`crates/idl-lower/tests/lowering.rs::an_abstract_valuetype_parses_as_a_valuetype` —
`lowering.rs:1224: TypeKind::Primitive(PrimitiveKind::Int32)`.

**Status:** done

### §7.4.7.4.1 Boxed Value Types

**Spec:** §7.4.7.4.1, S. 65–66 — der Value-Box ist ein Zustand ohne
`public`-/`private`-Kennzeichnung und mit einer Boxvariable.

**Repo:** `crates/idl-lower/src/lower.rs::value_box`,
`crates/corba-codegen/src/traits.rs`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_value_box_reaches_the_model_as_one`,
`crates/idl-lower/tests/lowering.rs::a_value_box_may_box_any_type_spec`,
`crates/idl-lower/tests/lowering.rs::a_value_box_round_trips_byte_for_byte`,
`crates/corba-codegen/tests/annex_a1_traits.rs::table_a9_gives_a_valuebox_its_boxed_trait` —
`annex_a1_traits.rs:281: Some(TraitDefinition::TraitsOf("long".to_string()))`.

**Status:** done

### §7.4.7.4.2 Abstract Value Types und Abstract Interfaces

**Spec:** §7.4.7.4.2, S. 66–67 — `abstract valuetype` und `abstract
interface`.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype_kind`,
`crates/ast/src/node/type_.rs::ValuetypeKind`.

**Tests:** `crates/idl-lower/tests/lowering.rs::an_abstract_valuetype_parses_as_a_valuetype`,
`crates/idl-lower/tests/lowering.rs::abstract_still_reaches_the_interface_header_it_also_belongs_to`,
`crates/idl-lower/tests/dialects.rs::each_component_form_reaches_its_own_interface_kind`,
`crates/corba-codegen/tests/annex_a1_traits.rs::table_a8_reads_abstract_and_truncatable_off_the_declaration` —
`annex_a1_traits.rs:269: Some(TraitDefinition::Flag(true))`.

**Status:** done

### §7.4.7.4.3 Value Inheritance Rules

**Spec:** §7.4.7.4.3, S. 67–68 — die Regeln, welche Kombinationen aus
`abstract`, `custom`, `truncatable`, Basis und `supports` zulässig sind.

**Repo:** `crates/idl-lower/src/validate.rs::value_inheritance_rules`, `crates/idl-lower/src/validate.rs::interface_inheritance_rules` (abstrakte Interfaces).

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_7_4_3_a_value_type_has_one_concrete_base_first_and_supports_one_interface`.

**Status:** `done` — das Beispiel des Absatzes (`V3` zulässig, `V4` unzulässig) und die Zellen von Tabelle 7-19: eine konkrete Basis, an erster Stelle; ein abstrakter Value Type erbt nur abstrakte; ein unterstütztes Interface, das nicht abstrakt ist und von denen der Basen abgeleitet ist; kein Boxed Value als Basis; ein abstraktes Interface erbt nur abstrakte Interfaces (`IDL0021`).

### §7.4.7.4.4 Custom Marshaling

**Spec:** §7.4.7.4.4, S. 68–69 — "Once a value type has been marked as custom, it needs to provide an implementation that marshals and unmarshals the value type. [...] It is the responsibility of the implementation to marshal the state of all of its base types." "Custom value types can never be safely truncated to base".

**Repo:** `crates/idl-lower/src/lower.rs::valuetype_kind` (setzt `ValuetypeKind::Custom`), `crates/idl-lower/src/validate.rs::value_inheritance_rules`, `crates/corba-rust/src/valuetype_emit.rs::emit_custom_marshal` (der Trait `<Name>CustomMarshal`), `emit_value_struct` (`ValueMarshal` und die Factory rufen ihn).

**Tests:** `crates/corba-rust/tests/compile_check.rs::idl_7_4_7_4_4_a_custom_valuetype_is_marshaled_by_the_application`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_7_4_5_truncatable_needs_a_stateful_base_and_custom_stays_custom`,
`crates/idl-lower/tests/dialects.rs::a_valuetype_keeps_its_modifier_bases_and_supported_interfaces`.

**Status:** `done` — der erzeugte Code eines `custom valuetype` deklariert `<Name>CustomMarshal` mit `marshal` und `unmarshal` über dem CDR-Strom; das erzeugte `ValueMarshal` und die registrierte Factory rufen ihn, statt den deklarierten Zustand zu schreiben und zu lesen, sodass der Code der Anwendung auch den Zustand der Basen schreibt. Mit `cargo check` übersetzt: mit der Implementierung der Anwendung übersetzt der Code, ohne sie meldet der Compiler das fehlende `PointCustomMarshal` (E0277). `truncatable` an einem custom-Werttyp und ein nicht-custom-Werttyp unter einem custom-Werttyp werden abgewiesen.

### §7.4.7.4.5 Truncatable

**Spec:** §7.4.7.4.5, S. 69 — `truncatable` erlaubt das Abschneiden einer
unbekannten Erweiterung beim Marshalisieren.

**Repo:** `crates/idl-lower/src/lower.rs::truncatable_is_for_a_plain_value`, `crates/idl-lower/src/validate.rs::value_inheritance_rules`, `crates/corba-rust/src/value_wire.rs` (Chunked-Kodierung und Abschneiden), `crates/corba-rust/src/valuetype_emit.rs` (Basis-Repository-Ids).

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_7_4_5_truncatable_needs_a_stateful_base_and_custom_stays_custom`,
`crates/corba-rust/src/value_wire.rs::tests::corbarust_4_3_an_unknown_most_derived_type_truncates_to_its_base`,
`crates/corba-rust/tests/codegen_mapping.rs::corbarust_4_1_a_truncatable_valuetype_lists_its_base_ids`.

**Status:** `done` — `truncatable` an einem custom oder abstrakten Value Type und an einem ohne zustandsbehaftete Basis wird abgewiesen, ebenso ein nicht-custom Value Type, der von einem custom erbt (`IDL0021`); die Rust-CORBA-Abbildung schreibt einen abschneidbaren Wert mit seinen Basis-Ids, und ein Leser, der den am weitesten abgeleiteten Typ nicht kennt, schneidet auf seine Basis ab (die Oktette, die JacORB schreibt).

### §7.4.7.4.6 Value Base

**Spec:** §7.4.7.4.6, S. 69–70 — "In a CORBA context, all value types have a conventional base type called ValueBase. [...] That root may be used in an IDL specification wherever a value type may be used." "In each language mapping ValueBase will be mapped to an appropriate base type that supports the marshaling/unmarshaling protocol as well as the model for custom marshaling."

**Repo:** `crates/ast/src/node/type_.rs::TypeKind::ValueBase`, `crates/corba-rust/src/valuetype_emit.rs` (der Zustandsträger jedes Werttyps implementiert `zerodds_corba_rust::ValueBase` und `value_wire::ValueMarshal`; ein custom-Werttyp `<Name>CustomMarshal`).

**Tests:** `crates/idl-lower/tests/lowering.rs::a_valuebase_parameter_reaches_the_model_as_valuebase`,
`crates/idl-lower/tests/lowering.rs::valuebase_is_reachable_in_every_position_rule_132_gives_it`,
`crates/idl-lower/tests/lowering.rs::none_of_the_three_built_in_type_names_leaves_a_broken_reference`,
`crates/corba-rust/tests/compile_check.rs::corbarust_3_the_generated_code_compiles_with_the_data_column`,
`crates/corba-rust/tests/compile_check.rs::idl_7_4_7_4_4_a_custom_valuetype_is_marshaled_by_the_application`.

**Status:** `done` — `ValueBase` wird überall angenommen, wo ein Werttyp steht (Regel 132), und erreicht das Modell als `TypeKind::ValueBase`. Der Basistyp der CORBA-Abbildung nach Rust ist der Trait `zerodds_corba_rust::ValueBase` mit `ValueMarshal` für das Marshaling-Protokoll und `<Name>CustomMarshal` für custom marshaling; der erzeugte Code übersetzt. Die Form von `ValueBase` in der C++-Abbildung ist IDL4-C++ §A.1.8.5 (`idl4-cpp-1.0.md`). Die DDS-Spalten von `zerodds-idlc` (Probe `bb7-132-valuebase`, `NoLanguageForm`) sind kein CORBA-Kontext.

---

## §7.4.8 Building Block Components – Basic

### §7.4.8.3 Syntax

**Spec:** §7.4.8.3, S. 70–71 — (135)–(144): `component_dcl`,
`component_header`, `component_body`, `export`, `provides_dcl`,
`uses_dcl`, `port_dcl`.

**Repo:** `crates/idl-parse/src/decl.rs::parse_port` und
`parse_component`, `crates/idl-lower/src/lower.rs::interface` und `port`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_component_form_reaches_its_own_interface_kind`,
`crates/idl-lower/tests/dialects.rs::each_port_form_reaches_its_own_component_role`,
`crates/idl-lower/tests/conformance.rs::every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_4_8_3_the_fixture_directory_holds_exactly_the_named_fixtures` —
`dialects.rs:103: ComponentRole::Receptacle { multiple.. } => assert!(!multiple),`.

**Status:** done

### §7.4.8.4.1 Component Header

**Spec:** §7.4.8.4.1, S. 71 — `component <identifier> [ :
<component_name> { "," … } * ] [ supports <interface_identifier>
{ "," … } * ]`.

**Repo:** `crates/idl-lower/src/lower.rs::interface_bases`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_component_keeps_its_supports_clause_apart_from_its_base`,
`crates/ccm/tests/equivalent_idl.rs::a_component_with_supports_inherits_ccmobject_and_the_supported_ones`,
`crates/ccm/tests/equivalent_idl.rs::a_derived_component_with_supports_lists_the_base_first_and_then_the_supported` —
`equivalent_idl.rs:169: assert_eq!(base_names(&ast, component), ["B", "I1", "I2"]);`.

**Status:** done

### §7.4.8.4.2.1 Facets und §7.4.8.4.2.2 Receptacles

**Spec:** §7.4.8.4.2.1 / §7.4.8.4.2.2, S. 71–72 — `provides` ergibt ein
Facet, `uses` einen Receptacle; beide nennen das Interface, das sie
tragen.

**Repo:** `crates/idl-lower/src/lower.rs::port`, `crates/idl-lower/src/validate.rs::component_model_rules`, `crates/ast/src/facet/corba.rs::ComponentRole::{Facet, Receptacle}`, `crates/ccm/src/transform.rs::transform_component`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_8_4_2_1_a_facet_or_receptacle_names_an_interface_or_object`,
`crates/idl-lower/tests/dialects.rs::each_port_form_reaches_its_own_component_role`,
`crates/ccm/tests/equivalent_idl.rs::provides_yields_a_provide_operation_returning_the_facet`,
`crates/ccm/tests/equivalent_idl.rs::uses_simplex_yields_connect_disconnect_and_get_connection`.

**Status:** `done` — `provides` und `uses` erreichen ihre Rollen mit aufgelöstem Interface; ein Interface-Typ, der eine Komponente ist, ist `IDL0019`. Die Formen einer Komponente in einer Programmiersprache sind Sache der Sprachabbildungen (IDL4 to Java/C#/C++ §7.9: intermediäres IDL, hier `crates/ccm`).

### §7.4.8.4.2.3 Component Attributes

**Spec:** §7.4.8.4.2.3, S. 72 — `attribute <param_mode> <type_spec>
<identifier> ;` im Komponentenrumpf.

**Repo:** `crates/idl-lower/src/lower.rs::attribute`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_basic_component_carries_its_attributes_and_no_port_operation`,
`crates/ccm/tests/equivalent_idl.rs::a_component_attribute_is_left_alone` —
`equivalent_idl.rs:324: assert_eq!(ast.attribute(attributes[0]).expect("rate").name, "rate");`.

**Status:** done

### §7.4.8.4.3 Forward Declaration

**Spec:** §7.4.8.4.3, S. 73 — `component <identifier> ;`.

**Repo:** `crates/idl-lower/src/lower.rs::forward_interface_kind`,
`crates/ast/src/model.rs::add_forward_interface`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_forward_declared_component_is_completed_by_its_definition`,
`crates/idl-lower/tests/lowering.rs::a_component_forward_that_is_never_defined_is_still_reported`,
`crates/idl-lower/tests/lowering.rs::every_component_model_forward_keeps_its_own_form` —
`lowering.rs:1195: assert_eq!(node.kind, expected, "{source:?}");`.

**Status:** done

### §7.4.8.5 Specific Keywords

**Spec:** §7.4.8.5, S. 73 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier` —
`lexer.rs:73: assert_eq!(kinds("TRUE")[0], TokenKind::True);`.

**Status:** done

---

## §7.4.9 Building Block Components – Homes

### §7.4.9.3 Syntax

**Spec:** §7.4.9.3, S. 74–75 — (145)–(154): `home_dcl`, `home_header`,
`home_body`, `manages_dcl`, `primarykey_dcl`, `factory_dcl`,
`finder_dcl`.

**Repo:** `crates/idl-parse/src/decl.rs::parse_factory`,
`crates/idl-lower/src/lower.rs::manages_clause`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_component_form_reaches_its_own_interface_kind`,
`crates/idl-lower/tests/dialects.rs::a_home_records_what_it_manages_and_the_key_it_does_so_by`,
`crates/idl-lower/tests/conformance.rs::every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares` —
`dialects.rs:146: assert!(manages.component.is_resolved(), "the component was lost");`.

**Status:** done

### §7.4.9.4.1 Home Header

**Spec:** §7.4.9.4.1, S. 75 — `abstract home <identifier> …`,
`home <identifier> …`, `home <identifier> for <scoped_name> …` und die
Klausel `manages <component> ;`.

**Repo:** `crates/idl-lower/src/lower.rs::manages_clause`, `crates/idl-lower/src/validate.rs::component_model_rules`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_9_4_1_a_home_derives_from_a_home_and_manages_a_component`,
`crates/idl-lower/tests/lowering.rs::a_home_resolves_what_it_manages_and_the_key_it_manages_by`.

**Status:** `done` — Regeln (146) und (147): die eine Basis eines Homes löst auf ein Home auf und was es verwaltet auf eine Komponente; eine Basis, die kein Home ist, ist `IDL0021`, ein verwalteter Typ, der keine Komponente ist, `IDL0019`. IDL 4.2 kennt kein `abstract home` und kein `home … for`.

### §7.4.9.4.2 Home Body — factory, finder, primarykey

**Spec:** §7.4.9.4.2, S. 75–76 — `factory` und `finder` werden zu
Operationen des Homes; `primary key` nennt den Schlüsseltyp.

**Repo:** `crates/idl-lower/src/lower.rs::manages_clause`,
`crates/ccm/src/transform.rs::transform_home`,
`crates/ccm/src/validate.rs::validate_primary_key`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_factory_is_emitted_with_create_failure_first_in_its_raises`,
`crates/ccm/tests/equivalent_idl.rs::a_finder_is_emitted_with_finder_failure_and_the_callers_own_raises`,
`crates/ccm/tests/equivalent_idl.rs::a_primary_key_with_a_public_scalar_member_is_accepted`,
`crates/ccm/tests/equivalent_idl.rs::a_primary_key_without_the_base_is_rejected`,
`crates/ccm/tests/equivalent_idl.rs::a_primary_key_with_a_private_member_is_rejected`,
`crates/ccm/tests/equivalent_idl.rs::a_primary_key_holding_an_object_reference_is_rejected`,
`crates/ccm/tests/equivalent_idl.rs::a_primary_key_holding_a_nested_struct_is_accepted` —
`equivalent_idl.rs:691: assert_eq!(raises.len(), 2, "FinderFailure plus the caller's own");`.

**Status:** done

### §7.4.9.5 Specific Keywords

**Spec:** §7.4.9.5, S. 76 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier` —
`lexer.rs:73: assert_eq!(kinds("TRUE")[0], TokenKind::True);`.

**Status:** done

---

## §7.4.10 Building Block CCM-Specific

### §7.4.10.3 Syntax — eventtype und Event-Ports

**Spec:** §7.4.10.3, S. 77–78 — (166)–(172): `eventtype_dcl`,
`eventtype_header`, `eventtype_body`, `eventtype_forward_dcl`,
`emits_dcl`, `publishes_dcl`, `consumes_dcl`.

**Repo:** `crates/idl-lower/src/lower.rs::valuetype` und `port`,
`crates/ccm/src/transform.rs::transform_event_type`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_port_form_reaches_its_own_component_role`,
`crates/idl-lower/tests/dialects.rs::each_template_parameter_kind_is_distinguished`,
`crates/ccm/tests/equivalent_idl.rs::the_first_event_type_of_a_chain_inherits_event_base`,
`crates/ccm/tests/equivalent_idl.rs::one_event_type_yields_one_consumer_interface_however_many_ports_name_it`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends` —
`equivalent_idl.rs:492: assert_eq!(bases.len(), 1, "EventBase was not added");`.

**Status:** done

### §7.4.10.4.1.2 Event Ports

**Spec:** §7.4.10.4.1.2, S. 78–79 — `emits`, `publishes` und `consumes`
unterscheiden sich darin, ob ein Cookie zurückkommt.

**Repo:** `crates/idl-lower/src/lower.rs::port`
(`ComponentRole::EventSource` mit `publisher`),
`crates/ccm/src/transform.rs::transform_component`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_port_form_reaches_its_own_component_role`,
`crates/ccm/tests/equivalent_idl.rs::emits_yields_connect_and_disconnect_and_the_consumer_interface`,
`crates/ccm/tests/equivalent_idl.rs::publishes_yields_subscribe_returning_a_cookie`,
`crates/ccm/tests/equivalent_idl.rs::consumes_yields_a_get_consumer_operation` —
`equivalent_idl.rs:283: assert!(names.contains(&"unsubscribe_ticker".to_string()));`.

**Status:** done

### §7.4.10.4.2.1 Supported Interfaces

**Spec:** §7.4.10.4.2.1, S. 79 — `supports` im Komponenten- und im
Home-Header.

**Repo:** `crates/idl-lower/src/lower.rs::interface_bases`,
`crates/ccm/src/transform.rs`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_component_keeps_its_supports_clause_apart_from_its_base`,
`crates/ccm/tests/equivalent_idl.rs::a_component_with_supports_inherits_ccmobject_and_the_supported_ones`,
`crates/ccm/tests/equivalent_idl.rs::a_home_with_supports_puts_the_supported_interfaces_on_its_explicit`,
`crates/ccm/tests/equivalent_idl.rs::a_derived_home_with_supports_extends_the_base_explicit` —
`equivalent_idl.rs:476: ["CManagerBaseExplicit", "IExtra"]`.

**Status:** done

### §7.4.10.4.3 Multiple Uses

**Spec:** §7.4.10.4.3, S. 80 — `uses <type> <name> for
<multiport_type> ;` erlaubt mehrere Ports unter einem Namen.

**Repo:** `crates/idl-lower/src/lower.rs::port`
(`ComponentRole::Receptacle { multiple }`),
`crates/ccm/src/transform.rs::transform_component`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_port_form_reaches_its_own_component_role`,
`crates/ccm/tests/equivalent_idl.rs::uses_multiple_returns_a_cookie_and_the_plural_getter` —
`equivalent_idl.rs:250: assert!(names.contains(&"get_connections_managers".to_string()));`.

**Status:** done

### §7.4.10.4.4.2 Object Root

**Spec:** §7.4.10.4.4.2, S. 80 — eine Komponente darf `Object` als
Object-Root tragen.

**Repo:** `crates/idl-parse/src/decl.rs::parse_port`, `crates/idl-lower/src/lower.rs::port` (die generische Objektreferenz aus Regel (117)).

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_8_4_2_1_a_facet_or_receptacle_names_an_interface_or_object`.

**Status:** `done` — `provides Object root;` und `uses multiple Object many;` sind ein Facet und ein mehrfaches Receptacle, deren Port die Objektreferenz ohne Interface hält (`TypeKind::ObjectRef { interface: None }`).

### §7.4.10.5 Specific Keywords

**Spec:** §7.4.10.5, S. 80 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier` —
`lexer.rs:73: assert_eq!(kinds("TRUE")[0], TokenKind::True);`.

**Status:** done

---

## §7.4.11 Building Block Components – Ports and Connectors

### §7.4.11.3 Syntax

**Spec:** §7.4.11.3, S. 83–84 — (173)–(183): `porttype_dcl`,
`porttype_header`, `porttype_body`, `connector_dcl`, `connector_header`,
`connector_body`, `bridge_dcl`, `provides_dcl`, `uses_dcl`.

**Repo:** `crates/idl-parse/src/decl.rs`, `crates/idl-lower/src/lower.rs::port`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_component_form_reaches_its_own_interface_kind`,
`crates/idl-lower/tests/lowering.rs::every_component_model_forward_keeps_its_own_form` —
`dialects.rs:67: ("connector K { };", "K", InterfaceKind::Connector),`.

**Status:** done

### §7.4.11.4.1.1 Port Type Declaration

**Spec:** §7.4.11.4.1.1, S. 83 — `porttype <identifier> [ :
<porttype_name> … ] { <porttype_export> * }`.

**Repo:** `crates/idl-lower/src/lower.rs::interface` (`InterfaceKind::PortType`), `crates/idl-lower/src/lower.rs::port`, `crates/idl-lower/src/validate.rs::component_model_rules`, `crates/idl-lower/src/validate.rs::port_types_have_no_cycle`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_11_4_1_1_a_port_type_holds_ports_and_does_not_embed_itself`,
`crates/idl-lower/tests/lowering.rs::every_component_model_forward_keeps_its_own_form`,
`crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_2_an_extended_port_is_the_basic_ports_of_its_port_type`.

**Status:** `done` — ein Port Type mit Facets, Receptacles und eingebetteten Ports wird angenommen, `port` und `mirrorport` erreichen `ComponentRole::PortTypeRole` und `ConnectorRole`; ein Port Type ohne Port (`IDL0001`), ein Port, der keinen Port Type nennt (`IDL0019`), und ein Zyklus von Port Types (`IDL0021`) werden abgewiesen. Ein erweiterter Port ist die Gruppe der Basis-Ports seines Port Types: `zerodds-ccm` bildet sie als `<port>_<basic port>` auf die äquivalente Schnittstelle ab, `mirrorport` vertauscht Facets und Receptacles (§7.4.11.4.2).

### §7.4.11.4.3 Connectors

**Spec:** §7.4.11.4.3, S. 84 — `connector <identifier> …` beschreibt eine
Brücke; `<provides>`/`<uses>` mit `port`/`mirrorport` bilden sie.

**Repo:** `crates/idl-lower/src/lower.rs::port` (`ComponentRole::ConnectorRole`), `crates/idl-lower/src/validate.rs::component_model_rules`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_11_4_3_a_connector_has_exports_and_derives_from_a_connector`,
`crates/idl-lower/tests/dialects.rs::each_component_form_reaches_its_own_interface_kind`,
`crates/ccm/tests/equivalent_idl.rs::idl_7_4_11_4_3_a_connector_maps_through_its_ports_as_a_component`.

**Status:** `done` — ein Connector, der von einem Connector ableitet, mit `port`- und `mirrorport`-Membern in ihren Rollen, wird angenommen, ebenso einer, dessen Exporte nur Attribute sind (Regel (183)); ein Connector mit leerem Rumpf (`IDL0001`) und einer, der von einer Komponente ableitet (`IDL0021`), werden abgewiesen. `zerodds-ccm` bildet einen Connector über seine Ports wie eine Komponente ab.

### §7.4.11.5 Specific Keywords

**Spec:** §7.4.11.5, S. 84 — die Keywords des Bausteins.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier` —
`lexer.rs:73: assert_eq!(kinds("TRUE")[0], TokenKind::True);`.

**Status:** done

---

## §7.4.12 Building Block Template Modules

### §7.4.12.3 Syntax

**Spec:** §7.4.12.3, S. 86–87 — (186)–(194): `template_dcl`,
`formal_parameters`, `formal_parameter`, `actual_parameters`,
`actual_parameter`, `template_module_instantiation`,
`template_module_ref`.

**Repo:** `crates/idl-lower/src/lower.rs::template_module_inst`,
`crates/idl-lower/src/lower.rs::template_formals`,
`crates/idl-parse/src/decl.rs::at_template_module_inst`.

**Tests:** `crates/idl-lower/tests/dialects.rs::a_template_modules_parameters_reach_its_facet`,
`crates/idl-lower/tests/dialects.rs::each_template_parameter_kind_is_distinguished`,
`crates/idl-lower/tests/conformance.rs::every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`bb12-185-template-module`) —
`dialects.rs:170: assert_eq!(params.len(), 2);`.

**Status:** done

### §7.4.12.4.1 Template Module Declaration

**Spec:** §7.4.12.4.1, S. 87–88 — `module <identifier> <
formal_parameters > { … }`; die formalen Typen sind `typename`,
`struct`, `union`, `enum`, `exception`, `interface`, `valuetype`,
`eventtype` und `sequence`.

**Repo:** `crates/idl-lower/src/lower.rs::template_formals`,
`crates/ast/src/facet/corba.rs::TemplateParamKind`.

**Tests:** `crates/idl-lower/tests/dialects.rs::each_template_parameter_kind_is_distinguished`,
`crates/idl-lower/tests/dialects.rs::a_sequence_type_formal_parameter_parses_and_carries_its_element`,
`crates/idl-lower/tests/dialects.rs::the_bare_sequence_classifier_still_parses_as_itself`,
`crates/idl-lower/tests/dialects.rs::a_bounded_sequence_parameter_keeps_its_bound`,
`crates/idl-lower/tests/dialects.rs::a_template_module_declares_no_types_of_its_own` —
`dialects.rs:205: assert_eq!(params.first().map(|p| p.kind), Some(expected), "{written}");`.

**Status:** done

### §7.4.12.4.2 Template Module Instantiation

**Spec:** §7.4.12.4.2, S. 88–89 — `module <identifier> <
actual_parameters > <identifier> ;`; `alias` übergibt einen formalen Typ
weiter.

**Repo:** `crates/idl-lower/src/lower.rs::template_module_inst`,
`crates/idl-lower/src/lower.rs::template_body`.

**Tests:** `crates/idl-lower/tests/dialects.rs::an_instantiation_lowers_the_body_with_the_actual_substituted`,
`crates/idl-lower/tests/dialects.rs::two_instantiations_of_one_template_are_two_independent_modules`,
`crates/idl-lower/tests/dialects.rs::a_const_actual_parameter_becomes_a_constant_of_the_instance`,
`crates/idl-lower/tests/dialects.rs::an_alias_hands_the_enclosing_templates_parameter_on`,
`crates/idl-lower/tests/dialects.rs::an_instantiation_of_a_template_nobody_declared_is_reported_by_name`,
`crates/idl-lower/tests/dialects.rs::an_instantiation_with_the_wrong_number_of_actuals_is_reported`,
`crates/idl-lower/tests/dialects.rs::a_plain_module_is_untouched_by_the_instantiation_lookahead` —
`dialects.rs:1462: zerodds_ast::TypeKind::Primitive(zerodds_ast::PrimitiveKind::Int32)`.

**Status:** done

### §7.4.12.5 Specific Keywords

**Spec:** §7.4.12.5, S. 89 — das einzige zusätzliche Keyword des
Bausteins ist `alias`.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`,
`crates/idl-lower/tests/dialects.rs::an_alias_hands_the_enclosing_templates_parameter_on` —
`dialects.rs:1516: assert!(names.contains(&"Concrete::S".to_string()), "{names:?}");`.

**Status:** done

---

## §7.4.13 Building Block Extended Data Types

### §7.4.13.4.1 Structures with Single Inheritance

**Spec:** §7.4.13.4.1, S. 89 — eine Struktur kann von höchstens einer
anderen erben; die Erweiterbarkeit der Basis gilt fort, ein in der
abgeleiteten Struktur neu vergebener `@id` ist zu Recht.

**Repo:** `crates/ast/src/node/type_.rs::TypeKind::Structure { base }`,
`crates/idl-lower/src/validate.rs::inheritance_rules`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_struct_with_a_base_records_it`,
`crates/idl-parse/tests/parse.rs::a_struct_with_a_base_records_its_inheritance`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_with_another_extensibility_than_its_base_is_refused`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_member_name_of_the_base_chain_cannot_be_declared_again`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_member_id_of_the_base_chain_cannot_be_given_again`,
`crates/idl-lower/tests/completion.rs::a_derived_structure_that_said_nothing_has_its_bases_extensibility`,
`crates/idl-lower/tests/completion.rs::the_callers_default_extensibility_does_not_override_a_bases` —
`keys_and_inheritance.rs:95: assert_eq!(out.ast.effective_extensibility(id), Extensibility::Mutable);`.

**Status:** done

### §7.4.13.4.1 Void Content

**Spec:** §7.4.13.4.1, S. 89 — eine Struktur ohne Member (`struct S { };`)
ist in diesem Baustein zulässig.

**Repo:** `crates/ast/src/node/type_.rs::TypeKind::Structure`,
`crates/idl-lower/src/validate.rs::validate_type`.

**Tests:** `crates/idl-lower/tests/validation.rs::the_two_bodies_the_grammar_does_let_be_empty_stay_empty`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`,
`tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::specification`
(die C-Spalte schreibt und liest die Oktette von `empty-struct`,
`empty-struct-as-base` und `empty-struct-in-sequence`, auf die sich die
Hersteller einigen).

**Status:** done — der leere Struct wird akzeptiert und das Modell trägt
ihn; Rust emittiert ihn, und C, wo C23 §6.7.2.1p8 eine Struktur ohne
benanntes Mitglied undefiniert lässt, gibt ihm ein Member, das für keines
steht und nicht serialisiert wird (`uint8_t zd_no_members`).

### §7.4.13.4.2 Union Discriminators

**Spec:** §7.4.13.4.2, S. 89–90 — zulässig sind die Integer-Typen,
`boolean`, `char`, `octet` und eine Enumeration; ein Case-Label muss im
Wertebereich des Diskriminators liegen.

**Repo:** `crates/idl-lower/src/validate.rs::is_legal_discriminator`,
`crates/idl-lower/src/validate.rs::label_is_in_range`.

**Tests:** `crates/idl-lower/tests/validation.rs::each_legal_discriminator_type_is_accepted`,
`crates/idl-lower/tests/validation.rs::an_enum_discriminator_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_typedef_to_a_legal_discriminator_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_float_discriminator_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_string_discriminator_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_struct_discriminator_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_case_label_outside_the_discriminators_range_is_rejected`,
`crates/idl-lower/tests/validation.rs::a_case_label_inside_the_range_is_accepted`,
`crates/idl-lower/tests/validation.rs::a_boolean_discriminator_admits_only_zero_and_one`,
`crates/idl-lower/tests/validation.rs::only_a_unions_discriminator_may_be_a_key`,
`crates/idl-lower/tests/validation.rs::no_union_member_is_optional`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_refused_with_the_clause_that_permits_it` —
`validation.rs:184: assert!(!trips(&source, DiagnosticCode::InvalidDiscriminator),`.

**Status:** done

### §7.4.13.4.3.1 Maps

**Spec:** §7.4.13.4.3.1, S. 90 — `map < key_type , value_type >
< bound >`; eine Map darf kein Teil eines Schlüssels sein.

**Repo:** `crates/idl-parse/src/type_spec.rs::parse_map_type`,
`crates/idl-lower/src/validate.rs::validate_type`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_map_cannot_be_part_of_a_key`,
`crates/idl-lower/tests/annotations.rs::a_map_key_may_only_be_discard`,
`crates/idl-lower/tests/annotations.rs::external_on_an_element_a_map_key_or_a_map_value_is_carried`,
`crates/idl-lower/tests/annotations.rs::only_try_construct_is_accepted_on_a_collection_element`,
`crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely` —
`validation.rs:340: assert_trips(source, DiagnosticCode::InvalidAnnotation);`.

**Status:** done

### §7.4.13.4.3.2 Bit Sets (einschliesslich Bit Fields)

**Spec:** §7.4.13.4.3.2, S. 90–91 — `bitfield < width > [ , type_spec ]`;
"The maximum value is 64"; "When the destination type is given, the
number of stored bits cannot exceed its size (i.e., 1 for boolean, 8 for
octet, …)".

**Repo:** `crates/idl-lower/src/complete.rs::check_bitset_bounds` und
`holder_width`, `crates/idl-lower/src/complete.rs::MAX_BITFIELD_BITS`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_bitfield_wider_than_sixty_four_bits_is_refused`,
`crates/idl-lower/tests/validation.rs::a_bitfield_wider_than_its_destination_type_is_refused`,
`crates/idl-lower/tests/validation.rs::the_bit_forms_the_specification_admits_still_get_through`,
`crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`,
`crates/ast/tests/node_space.rs::an_anonymous_bitfield_carries_no_name` —
`validation.rs:785: assert_clean("bitset Ba { bitfield<64> f1; };");`.

**Status:** done

### §7.4.13.4.3.3 Bit Masks und ihre Annotation-Constraints

**Spec:** §7.4.13.4.3.3, S. 91–94 — "By default, the size of a bit mask is
32"; "A bit mask must contain at least one value and no more than its size
expressed in bits"; `@bit_bound` "must be lower than or equal to 64";
`@position` "Possible positions range from 0 … up to (size – 1)".

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds` und
`check_bit_bounds`, `crates/idl-lower/src/complete.rs::MAX_BIT_BOUND`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_bit_bound_over_sixty_four_is_refused`,
`crates/idl-lower/tests/validation.rs::a_bitmask_with_more_values_than_its_size_is_refused`,
`crates/idl-lower/tests/validation.rs::a_bitmask_with_no_bit_value_is_refused`,
`crates/idl-lower/tests/validation.rs::a_bit_position_outside_the_bitmasks_size_is_refused`,
`crates/idl-lower/tests/completion.rs::bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`,
`crates/idl-lower/tests/completion.rs::a_written_bit_bound_is_what_the_rule_reports`,
`crates/idl-lower/tests/annotations.rs::an_explicit_bit_position_reaches_the_bit` —
`validation.rs:813: assert_clean("@bit_bound(64) bitmask Ba { F1 };");`.

**Status:** done

### §7.4.13.4.3.3 Doppelte @position

**Spec:** §7.4.13.4.3.3, S. 92 — die Notizen der Klausel halten fest,
dass annotierte Bit-Werte ungeordnet sein dürfen, aber keine Dubletten.

**Repo:** `crates/idl-lower/src/complete.rs::check_bitmask_bounds`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_13_4_3_3_a_bit_position_taken_twice_is_refused`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_13_4_3_3_a_following_bit_value_past_the_bound_is_refused`.

**Status:** `done` — das Beispiel des Absatzes: `flagx` folgt auf `flag3` an Position 4, die `flag4` hat, und ist `IDL0016`; ungeordnete Werte ohne Doppelung werden angenommen, und ein folgender Wert jenseits der Schranke ist `IDL0014`.

### §7.4.13.4.4 Integers restricted to holding 8-bits of information

**Spec:** §7.4.13.4.4, S. 94 — (212) führt `int8` und `uint8` als eigene
Typen mit dem Wertebereich ±2^7 bzw. 0..2^8−1.

**Repo:** `crates/idl-lower/src/lower.rs::primitive_of`,
`crates/idl-parse/src/type_spec.rs::is_primitive_keyword`.

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`crates/idl-lower/tests/validation.rs::every_form_the_specification_admits_still_gets_through` —
`validation.rs:732: "const int8 K = 1;",`.

**Status:** done

### §7.4.13.4.5 Explicitly-named Integer Types

**Spec:** §7.4.13.4.5, S. 94 — (213)–(215) führen `int16`/`int32`/`int64`
und `uint16`/`uint32`/`uint64` als Aliase der klassischen Schreibweisen mit
gleichem Wertebereich.

**Repo:** `crates/idl-parse/src/type_spec.rs::is_primitive_keyword`,
`crates/idl-lower/src/lower.rs::primitive_of`.

**Tests:** `crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/ast/tests/node_space.rs::every_primitive_of_the_spec_table_is_representable`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_7_2_6_2_1_every_idl_4_2_section_d_fixture_passes_check_and_both_backends`
(`bb13-212-int64`) —
`lowering.rs:333: ("int64", PrimitiveKind::Int64),`.

**Status:** done

### §7.4.13.4.6 Ranges for all Integer Types (Table 7-26)

**Spec:** §7.4.13.4.6, S. 94–95 — Tabelle 7-26 nennt für jeden Integer-Typ
den Wertebereich und den gleichwertigen Core-Typ; `int8`/`uint8` haben
kein Core-Gegenstück (Spalte "N/A").

**Repo:** `crates/idl-lower/src/validate.rs::primitive_range`, `crates/idl-lower/src/validate.rs::constant_values_fit_their_type`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_1_4_3_a_constant_outside_its_types_range_is_refused`,
`crates/idl-lower/tests/validation.rs::a_case_label_outside_the_discriminators_range_is_rejected`.

**Status:** `done` — die Bereiche aus Tabelle 7-26 gelten für Case-Labels und für Konstanten: `const short s = 655592;` und `const octet o = -54;` (die Beispiele aus §7.4.1.4.3), `const int8 i = 200;` und die anderen Enden sind `IDL0019`, die Enden selbst werden angenommen.

### §7.4.13.5 Specific Keywords

**Spec:** §7.4.13.5, S. 95–96 — Tabelle 7-27 nennt `bitfield`, `bitmask`,
`bitset`, `map`, `int8`, `uint8`, `int16`, `int32`, `int64`, `uint16`,
`uint32`, `uint64`.

**Repo:** `crates/idl-cst/src/token.rs::KEYWORDS`,
`crates/idl-parse/src/type_spec.rs::is_primitive_keyword`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`,
`crates/idl-lower/tests/lowering.rs::each_primitive_keyword_reaches_its_model_kind`,
`crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised` —
`lowering.rs:331: ("int8", PrimitiveKind::Int8),`.

**Status:** done

---

## §7.4.14 Building Block Anonymous Types

### §7.4.14.3 Rule (216) — template_type_spec als type_spec

**Spec:** §7.4.14.3, S. 97 — (216) `<type_spec> ::+ <template_type_spec>`
erlaubt `sequence<…>` und `string<…>` dort, wo ein Typ verlangt wird.

**Repo:** `crates/idl-parse/src/type_spec.rs::parse_type_spec`, `crates/idl-lower/src/lower.rs::type_ref_of`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_14_3_rule_216_a_template_type_stands_anonymous_at_a_member`.

**Status:** `done` — `sequence<long>`, `sequence<long, 5>`, `string<8>`, `wstring<4>`, `fixed<5, 2>` und `map<long, string>` an einem Member geschrieben sind anonyme Typen ihrer Art.

### §7.4.14.3 Rule (217) — array_declarator als declarator

**Spec:** §7.4.14.3, S. 97 — (217) `<declarator> ::=
<complex_declarator> | <array_declarator>`, sodass `long a[3][4];` ohne
`typedef` entsteht.

**Repo:** `crates/idl-parse/src/decl.rs::parse_member_list`, `crates/idl-lower/src/lower.rs::declared`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_14_3_rule_217_an_array_declarator_stands_at_a_member`,
`crates/idl-parse/tests/parse.rs::an_array_declarator_is_only_known_at_the_bracket`.

**Status:** `done` — `long arr[3][4];` an einem Member ist ein Array der Dimensionen 3 und 4, und in `long plain, row[2];` ist nur `row` ein Array.

### §7.4.14.4 Verschachtelte Templates und `>>`

**Spec:** §7.4.14.4, S. 97 — "For those nested template declarations,
white space must be used to separate the two `>` tokens ending the
declaration so they are not parsed as a single `>>` token."

**Repo:** `crates/idl-parse/src/parser.rs::split_shift_right`, `crates/idl-parse/src/type_spec.rs::close_type_arguments`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_14_4_nested_templates_close_with_two_brackets`,
`crates/idl-parse/tests/parse.rs::shift_right_in_an_expression_is_not_split`.

**Status:** `done` — `sequence<sequence<long> >`, die Schreibweise, die der Absatz verlangt, und `sequence<sequence<long>>` sind beide eine Sequenz von Sequenzen; `16 >> 2` in einem Ausdruck bleibt eine Verschiebung und ist 4.

### §7.4.14.5 Specific keywords

**Spec:** §7.4.14.5, S. 97 — "There are no additional keywords with this
building block."

**Repo:** `crates/idl-cst/src/token.rs::KEYWORDS`.

**Tests:** —

**Status:** `n/a (informative)` — die Klausel sagt selbst, dass es keine
zusätzlichen Keywords gibt; es gibt nichts zu prüfen.

---

## §7.4.15 Building Block Annotations

### §7.4.15.1 Purpose

**Spec:** §7.4.15.1, S. 97 — "This building block defines a framework to
add meta-data to IDL constructs, by means of annotations."

**Repo:** `crates/idl-parse/src/decl.rs::parse_annotation_decl`,
`crates/idl-lower/src/annotation.rs::apply_one`.

**Tests:** —

**Status:** `n/a (informative)` — die Klausel begründet den Baustein; die
Produktionen stehen in §7.4.15.3.

### §7.4.15.2 Dependencies

**Spec:** §7.4.15.2, S. 97 — "This building block only relies on Building
Block Core Data Types. It is actually orthogonal to all others."

**Repo:** `crates/idl-lower/src/annotation.rs::enumerated_parameter`.

**Tests:** `tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_2_a_builtin_with_an_enumerated_parameter_reads_it_without_resolving_it`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_2_a_verbatim_directive_still_reaches_the_model_whole`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_2_the_rti_fixture_that_carried_both_defects_now_passes` —
`annotation_arguments.rs:145: assert!(found.is_empty(), "{source:?} was refused: {found:?}");`.

**Status:** done

### §7.4.15.3 Rules (218)–(227)

**Spec:** §7.4.15.3, S. 98 — (218) `<definition> ::+ <annotation_dcl> ";"`;
(219) `<annotation_dcl>`; (220) `<annotation_header>`; (221)
`<annotation_body>` = `{ <annotation_member> | <enum_dcl> ";" |
<const_dcl> ";" | <typedef_dcl> ";" }*`; (222) `<annotation_member>`;
(223) `<annotation_member_type>`; (224) `<any_const_type>`; (225)
`<annotation_appl>`; (226) `<annotation_appl_params>`; (227)
`<annotation_appl_param>`.

**Repo:** `crates/idl-parse/src/decl.rs::parse_annotation_decl` und
`parse_annotation_tail`, `crates/idl-lower/src/annotation.rs::apply_one`.

**Tests:** `crates/idl-parse/tests/parse.rs::an_annotation_body_admits_only_what_rule_221_lists`,
`crates/idl-parse/tests/parse.rs::the_declarations_rule_221_does_list_are_still_taken`,
`crates/idl-parse/tests/parse.rs::annotations_bind_to_the_declaration_that_follows`,
`crates/idl-parse/tests/parse.rs::an_annotation_takes_arguments_in_both_forms`,
`crates/idl-lower/tests/annotations.rs::a_root_anchored_annotation_name_reaches_the_builtin_it_names`,
`crates/idl-lower/tests/annotations.rs::a_qualified_annotation_name_reaches_its_declaration`,
`crates/idl-lower/tests/annotations.rs::every_spelling_of_one_builtin_says_the_same_thing`,
`crates/idl-lower/tests/annotations.rs::an_annotation_name_that_is_a_keyword_still_works_qualified`,
`crates/idl-lower/tests/annotations.rs::an_annotation_with_no_name_at_all_is_still_refused` —
`parse.rs:481: DiagnosticCode::UnexpectedToken,`.

**Status:** done

### §7.4.15.4.1 Defining Annotations

**Spec:** §7.4.15.4.1, S. 99 — der Rumpf einer Annotation ist ein eigener
Scope; an der Anwendungsstelle gilt "Members with no default value must be
given a value".

**Repo:** `crates/idl-parse/src/decl.rs::parse_annotation_decl` (die vier
zugelassenen Deklarationsformen), `crates/idl-lower/src/scope.rs::ScopeStack::push_container`,
`crates/idl-lower/src/annotation.rs::check_annotation_application`.

**Tests:** `tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_4_1_an_annotation_body_may_declare_the_types_its_members_use`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_4_1_the_user_annotation_fixture_passes`,
`crates/idl-lower/tests/validation.rs::an_annotation_member_with_no_default_needs_a_value_under_omg_idl`,
`crates/idl-lower/tests/validation.rs::an_annotation_member_with_no_default_takes_the_default_of_its_type`,
`crates/idl-lower/tests/validation.rs::an_annotations_declared_members_reach_the_model`,
`crates/idl-parse/tests/parse.rs::the_declarations_rule_221_does_list_are_still_taken`,
`crates/idl-parse/tests/parse.rs::an_annotation_body_admits_only_what_rule_221_lists` —
`validation.rs:904: "@annotation Aa { long v; };\n@Aa struct Sa { long m; };",`.

**Status:** `done` — unter `--strict` (OMG IDL 4.2 und nichts sonst) weist das
Front-End ein Annotationsmember ohne Default und ohne Wert ab, wie §7.4.15.4.2 es
verlangt; in den Vendordialekten gibt es ihm mit einer Warnung den Default seines
Typs (XTypes 1.3 §7.3.1.2.5). Die beiden Specs widersprechen sich; ein
Issue-Entwurf dazu fehlt noch.

### §7.4.15.4.2 Applying Annotations

**Spec:** §7.4.15.4.2, S. 100–101 — "in case an unknown annotation is
encountered, it should be ignored by the IDL-processing tools"; "A
constant expression, whose type must be compatible with the member's
declaration"; "In case there is only one member, the annotation
application may be shortened to the name of the annotation prefixed with @
and followed with the constant value of that unique member"; "malformed
definitions shall be treated as an error".

**Repo:** `crates/idl-lower/src/annotation.rs::check_annotation_application`
und `apply_one` (nur für aufgelöste Deklarationen aufgerufen).

**Tests:** `crates/idl-lower/tests/validation.rs::an_annotation_argument_has_to_have_the_members_type`,
`crates/idl-lower/tests/validation.rs::an_argument_naming_no_member_is_refused`,
`crates/idl-lower/tests/validation.rs::the_short_form_is_only_for_an_annotation_with_one_member`,
`crates/idl-lower/tests/validation.rs::none_of_the_four_rules_reaches_an_unknown_annotation`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_4_2_an_unknown_annotation_is_ignored_whatever_it_is_given`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_4_2_a_name_outside_an_unknown_annotation_still_has_to_resolve`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_4_2_a_literal_argument_of_an_unknown_annotation_still_reaches_the_model`,
`tools/idlc/tests/strict.rs::idl_7_4_15_4_2_strict_does_not_refuse_an_unknown_annotation`,
`crates/idl-lower/tests/dialects.rs::strict_does_not_refuse_an_unknown_annotation`,
`crates/ast/tests/node_space.rs::the_value_member_shorthand_is_recorded`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_15_4_2_an_annotation_prefixing_a_typedefs_type_annotates_the_typedef` —
`validation.rs:915: "@annotation Aa { long v default 0; };\n@Aa(nosuch = 1) struct Sa { long m; };",`.

**Status:** `done` — auch der Typ eines `typedef` lässt sich so annotieren (`typedef @max(100) long Length;`); die Annotation gilt für jeden deklarierten Alias.

### §7.4.15.5 Specific Keywords

**Spec:** §7.4.15.5, S. 102 — die Syntax nutzt `@` als Punctuator;
`@annotation` ist die Kopfmarke, kein Keyword.

**Repo:** `crates/idl-cst/src/token.rs::TokenKind::Annotation`,
`crates/idl-cst/src/tree.rs::CstKind::AnnotationDecl`.

**Tests:** `crates/idl-cst/tests/lexer.rs::every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`,
`crates/idl-parse/tests/parse.rs::each_declaration_form_is_recognised` —
`parse.rs:88: assert!(parses_cleanly(source), "{source:?} produced diagnostics");`.

**Status:** done

---

## §7.4.16 Relationships between the Building Blocks

### §7.4.16 Abhängigkeitsgitter (Figure 7-2)

**Spec:** §7.4.16, S. 102 — "Even if the building blocks have been
designed as independent as possible, they are linked by some
dependencies. The following figure represents the graph of their
relationships (actually a lattice)."

**Repo:** `crates/idl-parse/src/building_block.rs::BuildingBlock` (`relies_on`, `closure`, `rules`).

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_7_4_16_each_building_block_relies_on_what_its_subclause_lists`,
`crates/idl-parse/src/building_block.rs::tests::idl_annex_a_every_rule_belongs_to_one_building_block`.

**Status:** `done` — die fünfzehn Bausteine mit den Kanten aus Figure 7-2; der Abschluss jedes Bausteins ist, was sein Unterabschnitt "Dependencies with other Building Blocks" direkt und transitiv nennt, und jede Regel aus Annex A gehört zu einem Baustein.

---

## §7.5 Names and Scoping

### §7.5 Sichtbarkeitsregeln über alle Building Blocks

**Spec:** §7.5, S. 102 — "This clause defines the visibility rules that
apply to names. Those rules are considering the whole IDL grammar (i.e.,
the union of all building blocks)."

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack`,
`crates/ast/src/resolve.rs`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_constant_resolves_by_its_simple_name_in_its_own_scope`,
`crates/ast/tests/resolve_profile_query.rs::every_profile_accepts_a_model_of_plain_primitives`,
`crates/ast/tests/profile_table.rs::the_construct_table_is_total_over_profiles_and_constructs`,
`crates/ast/tests/profile_table.rs::the_facet_table_is_total_over_profiles_and_properties` —
`lowering.rs:1568: ast.eval_integral(bound).expect("the bound evaluates"),`.

**Status:** done

### §7.5.1 Qualified Names

**Spec:** §7.5.1, S. 102–103 — "A qualified name (one of the form
`<scoped_name>::<identifier>`) is resolved by first resolving the qualifier
`<scoped_name>` to a scope S, and then locating the definition of
`<identifier>` within S. The `<identifier>` is not searched for in enclosing
scopes."; "When a qualified name begins with `'::'`, the resolution process
starts with the file scope".

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::lookup_candidates`,
`crates/idl-lower/src/expr.rs::lookup_name`.

**Tests:** `crates/idl-lower/tests/reported_idl.rs::a_fully_qualified_scoped_name_resolves`,
`crates/idl-lower/tests/lowering.rs::a_qualified_constant_still_resolves_through_its_module`,
`crates/idl-parse/tests/parse.rs::a_leading_double_colon_is_the_global_scope`,
`crates/idl-lower/tests/dialects.rs::the_root_anchored_spelling_names_the_same_module`,
`crates/ast/tests/node_space.rs::nested_modules_are_addressable_by_path` —
`node_space.rs:43: ast.module_by_path(&scoped(&["Robot", "Sensors"])),`.

**Status:** done

### §7.5.1 Global Names

**Spec:** §7.5.1, S. 103 — "Every IDL definition in a file has a global
name within that file"; der globale Name ist die Verkettung aus aktuellem
Root, aktuellem Scope, `"::"` und dem Bezeichner.

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack` (`qualify` und
`qualify_segments`), `crates/idl-lower/tests/interface_as_type.rs`.

**Tests:** `crates/idl-lower/tests/interface_as_type.rs::the_key_is_the_declaration_s_own_scoped_name_and_not_the_written_segment`,
`crates/ast/tests/node_space.rs::nested_modules_are_addressable_by_path`,
`crates/idl-lower/tests/dialects.rs::a_typeprefix_may_name_the_module_it_is_written_in`,
`crates/idl-lower/tests/dialects.rs::a_sibling_declaration_wins_over_one_further_out` —
`dialects.rs:1307: "IDL:inner.example/outer/m/S:1.0"`.

**Status:** done

### §7.5.1 Mehrdeutigkeit

**Spec:** §7.5.1, S. 103–104 — "Ambiguous declarations shall be treated as
errors"; die Klausel nennt das Beispiel zweier geerbter `string_t`.

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::find`, `crates/idl-lower/src/lower.rs::report_ambiguous`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_1_an_attribute_typed_by_an_ambiguous_name_is_an_error`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_4_4_a_name_declared_in_two_bases_is_ambiguous_unless_qualified`,
`crates/ast/tests/resolve_profile_query.rs::an_ambiguous_name_reports_every_candidate`.

**Status:** `done` — das Beispiel des Absatzes: `attribute string_t Title;` in `C: A, B` ist `IDL0009`, `A::string_t` und `B::string_t` werden angenommen.

### §7.5.1 Sichtbarkeit durch Inheritance und mehrere globale Namen

**Spec:** §7.5.1, S. 103 — "Inheritance causes all identifiers defined in
base interfaces, both direct and indirect, to be visible in derived
interfaces"; "Multiple paths to the same original identifier … do not
conflict with each other"; "Inheritance introduces multiple global IDL
names for the inherited identifiers … the exception is known by the global
names `::A::E` and `::B::E`".

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::find`, `crates/idl-lower/src/scope.rs::ScopeStack::find_in`, `crates/idl-lower/src/lower.rs::use_scoped`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_1_an_inherited_identifier_is_visible_under_every_global_name`.

**Status:** `done` — das Beispiel des Absatzes: die Exception `E` aus `A` wird in `B: A` über ihren einfachen Namen und als `::B::E` gefunden, und in `D: B, C` (eine Raute über `A`) ohne Konflikt; jedes `raises` löst auf die eine Deklaration auf.

### §7.5.2 Scope-Arten und ihr Beginn und Ende

**Spec:** §7.5.2, S. 104 — "The following kinds of definitions form scopes:
modules, structures, unions, maps, interfaces, value types, operations,
exceptions, event types, components, homes."; "The scope of an operation
begins immediately following its opening `(` and ends immediately preceding
its closing `)`."

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::push_container` und
`pop`.

**Tests:** `crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`,
`crates/idl-lower/tests/lowering.rs::a_constant_resolves_by_its_simple_name_in_its_own_scope`,
`crates/ast/tests/node_space.rs::nested_modules_are_addressable_by_path` —
`validation.rs:51: "module A { struct S { long x; }; };",`.

**Status:** done

### §7.5.2 Ein Bezeichner einmal pro Scope

**Spec:** §7.5.2, S. 104 — "An identifier can only be defined once in a
scope. However, identifiers can be redefined in nested scopes."

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::bind`.

**Tests:** `crates/idl-lower/tests/validation.rs::the_same_name_in_two_scopes_does_not_collide`,
`crates/idl-lower/tests/lowering.rs::a_name_declared_in_two_openings_of_one_module_collides`,
`crates/idl-lower/tests/validation.rs::two_members_of_one_aggregate_differing_only_by_case_collide` —
`validation.rs:116: "struct S { long tail; short Tail; };",`.

**Status:** done

### §7.5.2 Wiedereröffnung eines Moduls

**Spec:** §7.5.2, S. 104 — "An identifier declaring a module is considered
to be defined by its first occurrence in a scope. Subsequent occurrences of
a module declaration with the same identifier within the same scope reopens
the module and hence its scope."

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::push`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_module_reopened_across_two_files_converges_on_one_node`,
`crates/idl-lower/tests/lowering.rs::a_reopened_module_still_sees_what_the_first_opening_declared`,
`crates/ast/tests/node_space.rs::reopening_a_module_merges_into_the_same_node` —
`node_space.rs:58: assert_eq!(first, second);`.

**Status:** done

### §7.5.2 Keine Selbst-Umbenennung des Scope-Namens

**Spec:** §7.5.2, S. 104 — "The name of a module, structure, union, map,
interface, value type, event type, exception or home may not be redefined
within the immediate scope of the module, structure, union, map,
interface, value type, event type, exception or home."

**Repo:** `crates/idl-lower/src/lower.rs::check_redefines_enclosing_scope`.

**Tests:** `crates/idl-lower/tests/validation.rs::a_declaration_may_not_reuse_the_name_of_its_own_scope`,
`crates/idl-lower/tests/validation.rs::only_the_immediate_scope_is_compared` —
`validation.rs:1016: assert_clean("module Ma { module Mb { typedef short Ma; }; };");`.

**Status:** done — die Klausel zählt Operationen nicht auf, und genau so
weit greift die Prüfung; siehe das Item zu §7.5.2 Identifier-Einführung
für die zweite, offene Regel dieser Klausel.

### §7.5.2 Einführung eines Bezeichners durch Benutzung

**Spec:** §7.5.2, S. 104–105 — "An identifier from a surrounding scope is
introduced into a scope if it is used in that scope. An identifier is not
introduced into a scope by merely being visible in that scope."

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::introduce`, `crates/idl-lower/src/lower.rs::check_introduced`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_2_a_used_identifier_is_introduced_and_a_visible_one_is_not`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_2_a_name_is_searched_in_the_inherited_scope_before_the_enclosing_one`,
`tools/idlc/tests/idl_4_2_gaps.rs::idl_annex_a_every_probe_of_the_gap_scan_gets_the_specifications_answer`.

**Status:** `done` — das Beispiel des Absatzes: `typedef string inner1;` in `Inner2` wird angenommen, solange `Inner1` nur sichtbar ist, und ist `IDL0008`, nachdem `typedef Inner1::S1 S2;` es eingeführt hat; `typedef string S1;` bleibt angenommen. Die Suchreihenfolge des zweiten Beispiels gilt: `ArgType` in Zeile l5 ist `M::B::ArgType` (string), gefunden im geerbten Scope.

### §7.5.2 Nur das erste Glied eines qualifizierten Namens wird eingeführt

**Spec:** §7.5.2, S. 105 — "Only the first identifier in a qualified name is
introduced into the current scope. This is illustrated by **Inner1::S1**
in the example above, which introduces **Inner1** into the scope of
**Inner2** but does not introduce **S1**. A qualified name of the form
**::X::Y::Z** does not cause **X** to be introduced, but a qualified name of
the form **X::Y::Z** does."

**Repo:** `crates/idl-lower/src/lower.rs::use_scoped`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_2_only_the_first_identifier_of_a_relative_name_is_introduced`.

**Status:** `done` — `X::Y::Z` führt `X` ein (ein späteres `x` ist `IDL0008`), nicht `Y` oder `Z`; `::X::Y::Z` führt nichts ein.

### §7.5.2 Enum-Wertnamen im umgebenden Scope

**Spec:** §7.5.2, S. 105 — "Enumeration value names are introduced into the
enclosing scope and then are treated like any other declaration in that
scope."; das Beispiel `enum BadE { E3, E4, E5 };` ist mit `// Error`
gekennzeichnet, weil `E3` schon aus `E` stammt.

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::bind`,
`crates/idl-lower/src/lower.rs::enumeration`.

**Tests:** `crates/idl-lower/tests/lowering.rs::an_enumerator_is_named_from_the_scope_around_its_enumeration`,
`crates/idl-lower/tests/lowering.rs::one_literal_name_in_two_enumerations_collides`,
`crates/idl-lower/tests/lowering.rs::a_union_over_an_enumeration_lowers_with_its_labels_evaluated`,
`crates/idl-lower/tests/lowering.rs::a_case_label_may_name_its_enumerator_by_a_qualified_name` —
`lowering.rs:217: assert_eq!(ast.eval_integral(expr), Ok(1));`.

**Status:** done

### §7.5.2 Auflösung eines unqualifizierten Namens

**Spec:** §7.5.2, S. 105–106 — "A name can be used in an unqualified form
within a particular scope; it will be resolved by successively searching
farther out in enclosing scopes, while taking into consideration
inheritance relationships among interfaces."

**Repo:** `crates/idl-lower/src/expr.rs::lookup_name`,
`crates/idl-lower/src/scope.rs::ScopeStack::lookup_candidates`.

**Tests:** `crates/idl-lower/tests/lowering.rs::a_constant_resolves_by_its_simple_name_in_its_own_scope`,
`crates/idl-lower/tests/lowering.rs::a_qualified_constant_still_resolves_through_its_module`,
`crates/idl-lower/tests/dialects.rs::a_sibling_declaration_wins_over_one_further_out` —
`lowering.rs:1597: assert_eq!(ast.eval_integral(bound).expect("evaluates"), 8);`.

**Status:** done

### §7.5.3 Special Scoping Rules for Type Names

**Spec:** §7.5.3, S. 106 — "Once a type has been defined anywhere within the
scope of a module, interface or value type, it may not be redefined except
within the scope of a nested module, interface or value type, or within the
scope of a derived interface or value type."

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::bind`, `crates/idl-lower/src/scope.rs::ScopeStack::find`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_3_a_derived_interface_may_redefine_a_type_of_its_base`,
`crates/idl-lower/tests/lowering.rs::a_name_declared_in_two_openings_of_one_module_collides`.

**Status:** `done` — das abgeleitete Interface `B` aus dem Beispiel des Absatzes definiert `ArgType` und `S` neu; in `B::S` ist `ArgType` `long` und `A::ArgType` `string`; eine zweite Deklaration in einem Scope bleibt `IDL0008`.

### §7.5.3 Potential Scope

**Spec:** §7.5.3, S. 107 — "if a type name is introduced into a scope that is
nested in a non-module scope definition its potential scope extends over
all its enclosing scopes out to the enclosing non-module scope"; "A type
may not be redefined within its scope or potential scope"; ferner die
Note, dass eine Redefinition *nach* der Benutzung in einem Modul zulässig
ist.

**Repo:** `crates/idl-lower/src/scope.rs::ScopeStack::introduce`, `crates/idl-lower/src/scope.rs::ScopeStack::push_body`, `crates/idl-lower/src/lower.rs::check_introduced`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_5_3_a_type_may_not_be_redefined_within_its_potential_scope`.

**Status:** `done` — das Beispiel des Absatzes: `ArgType` und `I`, benutzt in einer Struktur des Interfaces `A`, sind in `A` eingeführt, daher sind `typedef string ArgType;` und `enum I` dort `IDL0008`, `typedef short Y;` und das Member `long I;` nicht; die Neudefinition der Anmerkung nach einer Benutzung in einer Struktur eines Moduls wird angenommen, `x` bleibt `long`, `y` ist `string`.

---

## §8 Standardized Annotations

### §8.1 Overview

**Spec:** §8.1, S. 109 — "The syntax to define annotations is given in the
clause 7.4.15, Building Block Annotations. Any profile that embeds that
building block will support annotations."; "this clause defines some
standardized annotations and groups them in Groups of Annotations".

**Repo:** `crates/ast/src/node/annotation.rs::BUILTIN_ANNOTATIONS` (31
Namen), `crates/idl-lower/src/annotation.rs::apply_one`.

**Tests:** `crates/ast/tests/node_space.rs::every_builtin_annotation_of_the_spec_list_is_present`,
`crates/ast/tests/node_space.rs::builtin_identities_come_from_the_builtin_space`,
`crates/ast/tests/node_space.rs::builtin_identities_do_not_collide_with_parsed_content`,
`crates/idl-lower/tests/annotations.rs::a_user_declared_annotation_is_kept_whole` —
`node_space.rs:848: assert_eq!(BUILTIN_ANNOTATIONS.len(), 31);`.

**Status:** done — die Tabelle trägt die 24 Annotationen dieser Klausel
sowie sieben aus DDS-XTypes 1.3 §7.4.4 (`non_serialized`, `try_construct`,
`topic`, `hashid`, `data_representation`, `ignore_literal_names`,
`default_nested`); der Test prüft die Länge der Tabelle, nicht die
Zuordnung zu einer Gruppe.

### §8.2.1 Rules for Defining Standardized Annotations

**Spec:** §8.2.1, S. 109 — "The annotations that are standardized here have
been selected for their general purpose nature, meaning that their
application can be considered in various contexts."

**Repo:** `crates/ast/src/model.rs::Ast::populate_builtins`.

**Tests:** —

**Status:** `n/a (informative)` — der Absatz begründet die Auswahl; jede
Annotation wird in §8.3.x einzeln auditiert.

### §8.2.2 Rules for Using Standardized Annotations

**Spec:** §8.2.2, S. 109–110 — vier Anforderungen an eine Spezifikation,
die eine standardisierte Annotation verwendet: die Syntax der Spec
respektieren, die Bedeutung präzisieren, die gültigen Elemente
einschränken und das Default-Verhalten dokumentieren.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_8_2_2_every_standardized_annotation_is_read_in_its_declared_syntax`,
`crates/idl-lower/tests/annotations.rs::every_spelling_of_one_builtin_says_the_same_thing`.

**Status:** `done` — die vierundzwanzig Annotationen aus §8.3 werden in der Syntax ihrer Deklarationen gelesen, jeder Parameter beim Namen und die Kurzform, wo jeder Parameter einen Vorgabewert hat, und erreichen das Modell; ein Element ohne Annotation behält den Vorgabewert, den die benutzende Spezifikation (DDS-XTypes 1.3 §7.3.1.2.1) nennt.

### §8.3.1.1 `@id` Annotation

**Spec:** §8.3.1.1, S. 110 — "`@annotation id { unsigned long value; };`";
der Wert ist ein 32-Bit-Integer.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `id`),
`crates/idl-lower/src/complete.rs::derive_repository_ids`.

**Tests:** `crates/idl-lower/tests/annotations.rs::a_declared_id_records_that_it_was_declared`,
`crates/idl-lower/tests/annotations.rs::an_id_that_is_not_an_integer_is_reported_rather_than_ignored`,
`crates/idl-lower/tests/annotations.rs::an_id_and_a_hashid_on_one_member_are_rejected`,
`crates/idl-lower/tests/completion.rs::a_declared_id_wins_over_the_aggregates_autoid` —
`completion.rs:146: assert_eq!(member_id(&ast, "S", 0), (4, MemberIdOrigin::Declared));`.

**Status:** done

### §8.3.1.2 `@autoid` Annotation

**Spec:** §8.3.1.2, S. 110 — "`@annotation autoid { enum AutoidKind {
SEQUENTIAL, HASH }; AutoidKind value default HASH; };`".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `autoid`),
`crates/idl-lower/src/complete.rs::member_ids`.

**Tests:** `crates/idl-lower/tests/annotations.rs::both_autoid_policies_are_distinguished`,
`crates/idl-lower/tests/completion.rs::autoid_sequential_and_autoid_hash_differ`,
`crates/idl-lower/tests/completion.rs::an_autoid_hash_member_is_skipped_the_same_way` —
`annotations.rs:457: assert_eq!(xtypes_of_type(&ast, "S").autoid, Some(expected), "{source}");`.

**Status:** done

### §8.3.1.3 `@optional` Annotation

**Spec:** §8.3.1.3, S. 111 — "`@annotation optional { boolean value
default TRUE; };`"; ein optionales Member darf kein Key sein.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`optional`), `crates/idl-lower/src/validate.rs::validate_type`.

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/annotations.rs::several_annotations_on_one_member_all_apply`,
`crates/idl-lower/tests/validation.rs::a_key_member_must_not_be_optional` —
`annotations.rs:286: assert!(xtypes_of_member(&ast, "S", 1).is_optional);`.

**Status:** done

### §8.3.1.4 `@position` Annotation

**Spec:** §8.3.1.4, S. 111 — "`@annotation position { unsigned short
value; };`"; der Wertebereich ist 0..65535, und die Position muss innerhalb
der Bitmask-Grösse liegen (§7.4.13.4.3.3).

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`position`), `crates/idl-lower/src/complete.rs::check_bitmask_bounds`.

**Tests:** `crates/idl-lower/tests/annotations.rs::an_explicit_bit_position_reaches_the_bit`,
`crates/idl-lower/tests/completion.rs::bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`,
`crates/idl-lower/tests/validation.rs::a_bit_position_outside_the_bitmasks_size_is_refused`,
`crates/ast/tests/node_space.rs::a_positional_bit_and_an_explicit_position_are_distinguishable` —
`annotations.rs:744: assert_eq!(ast.bit_value(bits[0]).expect("A").position, Some(5));`.

**Status:** done

### §8.3.1.5 `@value` Annotation

**Spec:** §8.3.1.5, S. 111 — "`@annotation value { any value; };`"; der Wert
eines Enumerators wird festgeschrieben.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `value`),
`crates/ast/src/query.rs::effective_enumerator_value`.

**Tests:** `crates/idl-lower/tests/annotations.rs::an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`,
`crates/idl-lower/tests/completion.rs::enumerators_without_values_run_from_zero_upwards`,
`crates/idl-lower/tests/completion.rs::an_explicit_enumerator_value_shifts_the_ones_after_it`,
`crates/ast/tests/node_space.rs::a_positional_enumerator_and_an_explicit_value_are_distinguishable` —
`completion.rs:271: assert_eq!(ast.effective_enumerator_value(enumerators[2]), Some(11));`.

**Status:** done

### §8.3.1.6 `@extensibility` Annotation

**Spec:** §8.3.1.6, S. 111–112 — "`@annotation extensibility { enum
ExtensibilityKind { FINAL, APPENDABLE, MUTABLE }; ExtensibilityKind value;
};`".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`extensibility`).

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/annotations.rs::an_extensibility_argument_that_is_not_one_of_the_three_is_reported`,
`crates/idl-lower/tests/completion.rs::a_written_extensibility_is_what_the_rule_reports` —
`annotations.rs:229: ("@mutable struct S { long x; };", Extensibility::Mutable),`.

**Status:** done

### §8.3.1.7 `@final`, §8.3.1.8 `@appendable`, §8.3.1.9 `@mutable`

**Spec:** §8.3.1.7–§8.3.1.9, S. 112 — die drei Kurzformen sind "Shortcut for
**@extensibility(FINAL)**" und entsprechend für APPENDABLE und MUTABLE.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweige `final`,
`appendable`, `mutable`).

**Tests:** `crates/idl-lower/tests/annotations.rs::each_extensibility_keyword_reaches_the_facet`,
`crates/idl-lower/tests/reported_idl.rs::an_unannotated_struct_is_appendable`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_derived_structure_that_repeats_its_bases_extensibility_is_accepted` —
`keys_and_inheritance.rs:90: assert!(found.is_empty(), "{found:?}");`.

**Status:** done

### §8.3.2.1 `@key` Annotation

**Spec:** §8.3.2.1, S. 112–113 — "`@annotation key { boolean value
default TRUE; };`"; nur der Diskriminator einer Union darf ein Key sein.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `key`),
`crates/idl-lower/src/validate.rs::is_legal_discriminator`.

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/validation.rs::key_on_its_own_is_accepted`,
`crates/idl-lower/tests/validation.rs::only_a_unions_discriminator_may_be_a_key`,
`crates/idl-lower/tests/validation.rs::a_map_cannot_be_part_of_a_key`,
`crates/idl-lower/tests/keys_and_inheritance.rs::a_key_written_in_a_derived_structure_is_refused`,
`crates/idl-lower/tests/annotations.rs::a_suffix_annotation_carries_its_arguments` —
`annotations.rs:285: assert!(xtypes_of_member(&ast, "S", 0).is_key);`.

**Status:** done

### §8.3.2.2 `@must_understand` Annotation

**Spec:** §8.3.2.2, S. 113 — "`@annotation must_understand { boolean value
default TRUE; };`".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (der Zweig `must_understand`), `crates/types/src/assignability.rs` (die Leserseite).

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_8_3_2_2_must_understand_is_true_by_its_compact_form_and_false_without_it`,
`crates/types/src/assignability.rs::tests::xtypes_7_2_3_a_mutable_writer_member_the_reader_lacks_and_must_understand_is_no`.

**Status:** `done` — `@must_understand` und `@must_understand(TRUE)` setzen das Flag, `(FALSE)` und die fehlende Annotation nicht; ein so markiertes Writer-Member, das der Leser nicht kennt, macht die Typen nicht zuweisbar, die Empfängerseite von "it cannot be unknown to the recipient".

### §8.3.2.3 `@default_literal` Annotation

**Spec:** §8.3.2.3, S. 113 — "`@annotation default_literal { };`"; sie
markiert den Enumerator, nicht das `enum`.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`default_literal`).

**Tests:** `crates/idl-lower/tests/annotations.rs::a_default_literal_marks_the_enumerator_and_not_the_enum` —
`annotations.rs:772: assert!(marked(1));`.

**Status:** done

### §8.3.3.1 `@default` Annotation

**Spec:** §8.3.3.1, S. 113–114 — "`@annotation default { any value; };`".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (der Zweig `default`), `crates/idl-lower/src/validate.rs::annotated_values_fit_their_element`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_8_3_3_1_a_default_is_kept_and_must_fit_the_element`.

**Status:** `done` — der Wert erreicht `XTypesFacet::default_value` eines Members und eines `typedef`; ein Wert, der nicht vom Typ des Elements ist oder außerhalb seines Bereichs liegt, ist `IDL0007`.

### §8.3.3.2 `@range` Annotation

**Spec:** §8.3.3.2, S. 114 — "`@annotation range { any min; any max; };`"
mit der Bedingung `max >= min`.

**Repo:** `crates/idl-lower/src/annotation.rs::range_annotation`, `crates/idl-lower/src/validate.rs::ranges_are_ordered`, `crates/idl-lower/src/validate.rs::annotated_values_fit_their_element`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_8_3_3_2_a_range_whose_max_is_below_its_min_is_refused`,
`crates/idl-lower/tests/annotations.rs::a_unit_and_a_range_reach_their_fields`.

**Status:** `done` — `@range(5, 1)`, `@range(min = 5, max = 1)`, `@min(2.5) @max(1.5)` und ein Bereich an einem `typedef` mit max unter min sind `IDL0007`; gleiche Enden werden angenommen; ein Bereichsende außerhalb des Typs des Elements ist `IDL0007`.

### §8.3.3.3 `@min` und §8.3.3.4 `@max`

**Spec:** §8.3.3.3 / §8.3.3.4, S. 114 — "`@annotation min { any value;
};`" und "`@annotation max { any value; };`"; getrennt geschrieben
ergeben sie dieselbe Range wie `@range`.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweige `min`
und `max`).

**Tests:** `crates/idl-lower/tests/annotations.rs::min_and_max_written_separately_combine_into_one_range` —
`annotations.rs:712: assert!(range.max.is_some(), "the max was lost");`.

**Status:** done

### §8.3.3.5 `@unit` Annotation

**Spec:** §8.3.3.5, S. 114 — "`@annotation unit { string value; };`" plus
der Hinweis auf die BIPM-Abkürzungen.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `unit`).

**Tests:** `crates/idl-lower/tests/annotations.rs::a_unit_and_a_range_reach_their_fields` —
`annotations.rs:702: assert_eq!(x.unit.as_deref(), Some("m/s"));`.

**Status:** done

### §8.3.4.1 `@bit_bound` Annotation

**Spec:** §8.3.4.1, S. 115 — "`@annotation bit_bound { unsigned short
value; };`"; der Wert darf höchstens 64 sein (§7.4.13.4.3.3), und ohne
Annotation sind es 32.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`bit_bound`), `crates/idl-lower/src/complete.rs::check_bit_bounds`.

**Tests:** `crates/idl-lower/tests/annotations.rs::a_bit_bound_reaches_the_type`,
`crates/idl-lower/tests/completion.rs::an_enum_without_a_bit_bound_is_thirty_two_bits`,
`crates/idl-lower/tests/completion.rs::a_written_bit_bound_is_what_the_rule_reports`,
`crates/idl-lower/tests/validation.rs::a_bit_bound_over_sixty_four_is_refused` —
`annotations.rs:752: assert_eq!(xtypes_of_type(&ast, "E").bit_bound, Some(16));`.

**Status:** done

### §8.3.4.2 `@external` Annotation

**Spec:** §8.3.4.2, S. 115 — "`@annotation external { boolean value
default TRUE; };`". XTypes 1.3 §7.3.1.2.1.4 lässt sie auch auf einem Element
einer Collection, einem Map-Schlüssel und einem Map-Wert zu; das Front-End trägt
sie dort.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`external`).

**Tests:** `crates/idl-lower/tests/annotations.rs::the_member_flags_each_reach_their_own_field`,
`crates/idl-lower/tests/annotations.rs::external_on_an_element_a_map_key_or_a_map_value_is_carried`,
`crates/idl-lower/tests/validation.rs::external_on_a_type_declaration_is_refused_by_name`,
`crates/idl-lower/tests/validation.rs::external_on_a_member_and_on_a_branch_is_accepted` —
`validation.rs:426: assert_trips(source, DiagnosticCode::InvalidAnnotation);`.

**Status:** done

### §8.3.4.3 `@nested` Annotation

**Spec:** §8.3.4.3, S. 115 — "`@annotation nested { boolean value default
TRUE; };`"; sie wirkt transitiv auf die in einem Modul deklarierten
Typen.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `nested`).

**Tests:** `crates/idl-lower/tests/annotations.rs::a_nested_annotation_reaches_the_type`,
`crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it` —
`annotations.rs:214: assert!(ast.effective_nested(by_name("Inner")));`.

**Status:** done

### §8.3.5.1 `@verbatim` Annotation

**Spec:** §8.3.5.1, S. 116 — "`@annotation verbatim { enumeration
PlacementKind { BEGIN_FILE, BEFORE_DECLARATION, BEGIN_DECLARATION,
END_DECLARATION, AFTER_DECLARATION, END_FILE }; string language default
"\*"; PlacementKind placement default BEFORE_DECLARATION; string text; };`";
die `language`-Werte sind `"c"`, `"c++"`, `"java"`, `"idl"` und `"*"`.

**Repo:** `crates/idl-lower/src/annotation.rs::verbatim_placement` (die
sechs Placements), `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`verbatim`).

**Tests:** `crates/idl-lower/tests/annotations.rs::a_verbatim_directive_keeps_its_language_placement_and_text`,
`crates/idl-lower/tests/annotations.rs::a_verbatim_directive_defaults_language_and_placement`,
`crates/idl-lower/tests/annotations.rs::repeated_verbatim_directives_accumulate_in_declaration_order`,
`crates/idl-lower/tests/annotations.rs::a_verbatim_directive_for_an_unknown_language_is_kept_as_written`,
`crates/idl-lower/tests/annotations.rs::an_unknown_verbatim_placement_is_reported`,
`tools/idlc/tests/annotation_arguments.rs::idl_7_4_15_2_a_verbatim_directive_still_reaches_the_model_whole` —
`annotation_arguments.rs:175: assert_eq!(verbatim.len(), 1, "{verbatim:?}");`.

**Status:** done

### §8.3.6.1 `@service` Annotation

**Spec:** §8.3.6.1, S. 117 — "`@annotation service { string platform
default "\*"; };`"; die Platform-Werte sind `"CORBA"`, `"DDS"` und `"*"`.

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig
`service`).

**Tests:** `crates/idl-lower/tests/annotations.rs::service_and_ami_reach_the_corba_facet` —
`annotations.rs:904: assert!(corba.is_service);`.

**Status:** done

### §8.3.6.2 `@oneway` Annotation

**Spec:** §8.3.6.2, S. 117 — "`@annotation oneway { boolean value
default TRUE; };`"; "This annotation may only concern operations without
any return value (**void** return type) and without any **out** or
**inout** parameters."

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (der Zweig `oneway`), `crates/idl-lower/src/validate.rs::oneway_operations_return_nothing`.

**Tests:** `crates/idl-lower/tests/idl_4_2_rules.rs::idl_8_3_6_2_a_oneway_operation_returns_nothing_and_takes_in_parameters_only`,
`crates/idl-lower/tests/idl_4_2_rules.rs::idl_7_4_6_4_5_a_oneway_operation_has_no_raises_clause`.

**Status:** `done` — `@oneway` in einer `.idl`-Zeile setzt das Flag der Operation, `@oneway(FALSE)` lässt sie zweiseitig; eine One-way-Operation mit Ergebnis, einem `out`- oder `inout`-Parameter oder einer `raises`-Klausel ist `IDL0007`, in der Annotations- und in der Schlüsselwortform.

### §8.3.6.3 `@ami` Annotation

**Spec:** §8.3.6.3, S. 117 — "`@annotation ami { boolean value default
TRUE; };`".

**Repo:** `crates/idl-lower/src/annotation.rs::apply_one` (Zweig `ami`).

**Tests:** `crates/idl-lower/tests/annotations.rs::service_and_ami_reach_the_corba_facet` —
`annotations.rs:905: assert!(corba.is_ami);`.

**Status:** done

---

## §9 Profiles

### §9.1 Overview

**Spec:** §9.1, S. 119 — "This clause defines some relevant combinations of
building blocks, called *profiles*. Profiles are just sets of building
blocks, possibly complemented with groups of annotations."; "These profiles
are not normative for the related middleware solutions."

**Repo:** `crates/idl-parse/src/building_block.rs` (`IdlProfile`, `AnnotationGroup`, `outside_profile`), `tools/idlc/src/main.rs` (`--idl-profile`), `crates/idl-compose/src/lib.rs::outside_idl_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — die sieben Profile aus Klausel 9 als Mengen von Bausteinen und Annotationsgruppen, jedes unter dem Gitter abgeschlossen, jedes mit Annotations, wo es eine Gruppe hat (§8.1); eine Datei wird mit `zerodds-idlc --idl-profile <P>` gegen eines geprüft (`IDL0023`), auf Anforderung, weil §9.1 die Profile nicht normativ nennt.

### §9.2.1 Plain CORBA Profile

**Spec:** §9.2.1, S. 119 — das Profil besteht aus Core Data Types, Any,
Interfaces – Basic, Interfaces – Full, Value Types, CORBA-Specific –
Interfaces und CORBA-Specific – Value Types.

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_2_1_the_plain_corba_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil Plain CORBA ist die Liste aus §9.2.1; dagegen geprüft Interfaces, Value Types, `any`, `typeid`, lokale und abstrakte Interfaces liegen innen; ein an einem Member geschriebener `string` (Regel (216)), ein Array an einem Member, eine Komponente, ein `map`, `int8` und eine Annotation liegen außen (`IDL0023`).

### §9.2.2 Minimum CORBA Profile

**Spec:** §9.2.2, S. 119–120 — "As opposed to Plain CORBA Profile, it does
not embed **Any** nor **Valuetypes**."

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_2_2_the_minimum_corba_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil Minimum CORBA ist die Liste aus §9.2.2; dagegen geprüft `any`, ein Value Type und ein abstrakter Value Type liegen außen, Interfaces mit eingebetteten Typen und einer Context-Klausel innen (`IDL0023`).

### §9.2.3 CCM Profile

**Spec:** §9.2.3, S. 120 — das Profil ergänzt Components – Basic und
CCM-Specific um die sieben CORBA-Bausteine.

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_2_3_the_ccm_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil CCM ist die Liste aus §9.2.3; dagegen geprüft Komponenten mit Facets, mehrfachen Receptacles, Event-Ports und `supports` sowie Event Types liegen innen; ein `home` liegt außen, weil der Absatz Components – Homes nicht nennt, obwohl die Regeln (162) bis (165) von CCM-Specific den Home-Header erweitern; ein Port Type liegt außen (`IDL0023`).

### §9.2.4 CCM with Generic Interaction Support Profile

**Spec:** §9.2.4, S. 120 — dieses Profil ergänzt Components – Ports and
Connectors und Template Modules um das CCM-Profil.

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_2_4_the_ccm_with_generic_interaction_support_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil CCM with Generic Interaction Support ist die Liste aus §9.2.4; dagegen geprüft Port Types, Connectors mit `port` und `mirrorport` und Template-Module liegen innen, `uint16` liegt außen (`IDL0023`).

### §9.3.1 Plain DDS Profile

**Spec:** §9.3.1, S. 121 — "It is made of: Building Block Core Data Types;
Building Block Anonymous Types."

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_3_1_the_plain_dds_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil Plain DDS ist die Liste aus §9.3.1; dagegen geprüft Strukturen, Unions und Enumerationen mit anonymen Strings, Sequenzen und Arrays liegen innen; `any`, ein `map`, eine Annotation, ein Interface und eine Struktur mit Basis liegen außen (`IDL0023`).

### §9.3.2 Extensible DDS Profile

**Spec:** §9.3.2, S. 121 — dieses Profil ergänzt Extended Data-Types,
Anonymous Types, Annotations und die vier Annotations-Gruppen.

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_3_2_the_extensible_dds_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil Extensible DDS ist die Liste aus §9.3.2; dagegen geprüft die Annotationen der vier Gruppen, Maps, Bitsets, Bitmasks, `int8` und Benutzerannotationen liegen innen; `@range` und `@default` (Units and Ranges) und ein Interface liegen außen (`IDL0023`).

### §9.3.3 RPC over DDS Profile

**Spec:** §9.3.3, S. 121 — das Profil ergänzt Interfaces – Basic und die
Gruppe der Interfaces-Annotationen.

**Repo:** `crates/idl-parse/src/building_block.rs::IdlProfile`, `crates/idl-parse/src/building_block.rs::outside_profile`.

**Tests:** `crates/idl-parse/tests/building_blocks.rs::idl_9_3_3_the_rpc_over_dds_profile`,
`crates/idl-parse/tests/building_blocks.rs::idl_9_1_every_profile_is_its_clauses_list_closed_under_the_lattice`.

**Status:** `done` — das Profil RPC over DDS ist die Liste aus §9.3.3; dagegen geprüft ein `@service`-Interface mit `@oneway`, Attributen und `@final`-Strukturen liegt innen; `@key` (Data Modeling), ein Value Type und eine in einem Interface deklarierte Struktur (Interfaces – Full) liegen außen (`IDL0023`).

---

## Annex A: Consolidated IDL Grammar

### Annex A Konsolidierte Grammatik (227 Produktionen)

**Spec:** Annex A, S. 123–132 — "This annex gathers all the rules from all
the building blocks"; die Produktionen (1)–(227) sind durchnummeriert.

**Repo:** `crates/idl-parse/src/decl.rs`, `crates/idl-parse/src/type_spec.rs`, `crates/idl-parse/src/parser.rs`, `tools/idl-4.2-gap-scan/cases.tsv`.

**Tests:** `tools/idlc/tests/idl_4_2_gaps.rs::idl_annex_a_every_probe_of_the_gap_scan_gets_the_specifications_answer`,
`crates/idl-parse/src/building_block.rs::tests::idl_annex_a_every_rule_belongs_to_one_building_block`,
`crates/idl-lower/tests/idl_4_2_grammar.rs::every_constructed_form_can_be_defined_inside_a_typedef`.

**Status:** `done` — jede der 227 Produktionen trägt eine Grammatik-Probe in `tools/idl-4.2-gap-scan/cases.tsv`, und ein Rust-Test führt alle 357 Proben durch das Front End und hält jede an ihre Spalte `expect`, die Antwort der Spezifikation (nur OMG IDL 4.2, die Herstellerproben mit ihren Dialekten).

---

## Audit-Status

183 done / 0 partial / 0 open / 0 blocked / 7 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9310 von `main` (`python3 ci/coverage_results.py --document idl-4.2.md`):

* `cargo nextest run --profile merge -p zerodds-ast --all-targets` (Job `merge-rust-ast`) — 23 Tests grün.
* `cargo nextest run --profile merge -p zerodds-ccm --all-targets` (Job `merge-rust-ccm`) — 24 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-codegen --all-targets` (Job `merge-rust-corba-codegen`) — 7 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-rust --all-targets` (Job `merge-rust-corba-rust`) — 5 Tests grün.
* `cargo nextest run --profile merge -p zerodds-float128 --all-targets` (Job `merge-rust-float128`) — 3 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-compose --all-targets` (Job `merge-rust-idl-compose`) — 6 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-cst --all-targets` (Job `merge-rust-idl-cst`) — 33 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (Job `merge-rust-idl-lower`) — 273 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-parse --all-targets` (Job `merge-rust-idl-parse`) — 68 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 12 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 8 Tests grün.
* `cargo nextest run --profile merge -p zerodds-types --all-targets` (Job `merge-rust-types`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-wireprobe-c-gen --all-targets` (Job `merge-rust-wireprobe-c-gen`) — 2 Tests grün.

465 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
