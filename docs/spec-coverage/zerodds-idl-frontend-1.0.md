# `zerodds-idl-frontend` v1.0 — Coverage

**Quelle:** [`docs/specs/zerodds-idl-frontend-1.0.md`](../specs/zerodds-idl-frontend-1.0.md)

**Implementation:** `crates/idl-cst`, `crates/idl-parse`, `crates/idl-lower`

**Test-Lauf:** `cargo test -p zerodds-idl-cst -p zerodds-idl-parse -p zerodds-idl-lower` — 422 bestanden, 0 fehlgeschlagen.

Jede Zeile nennt den Code-Pfad und den Test, der ihn prüft. Eine Zahl akzeptierter Dateien ist keine Coverage: ein Parser, der alles annimmt und nichts versteht, parst die Fixtures ebenso. Deshalb nennt jede Zeile das Konstrukt, das sie abdeckt.

---

## §1.1 Trivia am Token

**Spec:** Jedes Token trägt die vorangehende Trivia; die abschließende Trivia der Datei hängt am EOF-Token. Kein Byte der Eingabe ist aus dem Token-Strom unerreichbar.

**Repo:** `idl-cst/src/lex.rs`, `Token::leading`.

**Tests:** `every_byte_is_reachable_for_a_realistic_file`, `every_byte_is_reachable_for_a_broken_file`, `a_comment_before_a_token_is_that_tokens_leading_trivia`, `a_file_of_only_trivia_puts_all_of_it_on_the_eof_token`.

**Status:** done

## §1.2 Was der Lexer unterscheidet

**Spec:** Bezeichner und Keywords; die Escape-Form `_name` mit Vermerk; Integer in Dezimal, Oktal und Hex unter Erhalt der Radix; Float und Fixed in der geschriebenen Form (`1.` ist nicht `1.0`); Zeichen-, Wide-, String- und Wide-String-Literale mit aufgelösten Escapes bei erhaltenem Rohtext; Interpunktion inklusive `<<` und `>>`. Der Lexer entscheidet nicht, ob `>>` Shift oder zwei Klammern ist.

**Repo:** `idl-cst/src/token.rs` (85 Keywords als Tabelle), `idl-cst/src/lex.rs`, `literal_value`.

**Tests:** `every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`, `an_escaped_identifier_is_the_name_without_the_underscore`, `every_radix_reaches_the_same_value_and_keeps_how_it_was_written`, `float_forms_stay_distinguishable_from_one_another`, `a_d_suffix_makes_a_fixed_point_literal`, `wide_literals_are_their_own_kinds`, `a_literal_value_is_recovered_from_the_token_text_alone`, `text_that_is_not_one_whole_token_of_that_kind_has_no_value`.

**Status:** done

## §2.1 Green/Red-Baum

**Spec:** Unveränderlicher grüner Baum aus Art und Breite, rote Knoten mit absolutem Offset und Elternverweis auf Abruf. Jeder Knoten kennt seine volle Breite inklusive Trivia.

**Repo:** `idl-cst/src/tree.rs`.

**Tests:** `a_nodes_width_is_the_sum_of_its_children`, `a_child_knows_its_offset_and_its_parent`, `a_node_knows_its_ancestors_up_to_the_root`, `identical_subtrees_can_be_one_allocation`.

**Status:** done

## §2.2 Verlustfreiheit

**Spec:** `parse(source).text() == source` für jede Fixture, auch für Dateien, die nicht parsen.

**Repo:** `idl-cst/src/tree.rs` (`text()`, iterativ), `idl-parse/src/parser.rs` (`bump()` als einzige Stelle, an der Trivia in den Baum kommt).

**Tests:** `a_clean_file_round_trips_byte_for_byte`, `a_broken_file_round_trips_too`, `a_tree_over_a_file_that_would_not_parse_still_round_trips`, `every_fixture_round_trips_byte_for_byte` (21 Fixtures), `a_rejected_extension_still_parses_and_round_trips`.

**Status:** done

## §2.3 Das `>>`-Problem

**Spec:** In einer Typargumentliste wird `>>` in zwei `>` gespalten, in einem Ausdruck nicht. Die Spaltung ist im CST sichtbar, der Round-Trip hält.

**Repo:** `idl-parse/src/type_spec.rs` (`close_type_arguments`, `split_shift_right`).

**Tests:** `a_nested_sequence_splits_the_shift_token_and_still_round_trips`, `shift_right_in_an_expression_is_not_split`, `three_deep_nesting_splits_as_far_as_it_needs_to`.

**Status:** done

## §3.1 Grammatikabdeckung

**Spec:** Das ganze OMG IDL 4.2 §7.4: Kernbaustein, anonyme Typen, Annotationen, erweiterte Datentypen, Bausteine für Interfaces, Values, CCM-Komponenten, Templates und `import`.

**Repo:** `idl-parse/src/decl.rs`, `idl-parse/src/type_spec.rs`.

**Tests:** `every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares` (21 Fixtures, darunter `omg/dds_xtypes.idl`, `spec_features/ccm_components.idl`, `spec_features/template_modules.idl`, `spec_features/corba_value_repository.idl`, `spec_features/attr_raises.idl`, `spec_features/user_annotations.idl`); `each_component_form_reaches_its_own_interface_kind`, `each_port_form_reaches_its_own_component_role`, `each_template_parameter_kind_is_distinguished`, `a_valuetype_keeps_its_modifier_bases_and_supported_interfaces`.

**Status:** done

## §3.2 Recovery

**Spec:** Auf ein unerwartetes Token folgt eine Diagnose, ein Fehlerknoten über dem Bereich und Resynchronisation am nächsten `;`, `}` oder Top-Level-Keyword. Der Parser hält nicht an. Eine Datei liefert alle ihre Diagnosen.

**Repo:** `idl-parse/src/parser.rs` (`error_token`, `recover_to_declaration`, `MAX_STEPS`).

**Tests:** `three_errors_in_one_file_produce_three_diagnostics`, `a_missing_semicolon_recovers_at_the_next_declaration`, `a_file_of_pure_garbage_produces_diagnostics_and_a_tree_not_a_panic`.

**Status:** done

## §3.3 Mehrdeutigkeiten

**Spec:** `>>` als Shift gegen zwei Klammern; Annotation gegen `#pragma` mit derselben Aussage, beide erreichen dieselbe Facette; `::A` als globaler Scope, nie Operator; Array-Deklarator gegen einfachen Namen, entschieden am `[`.

**Repo:** `idl-parse/src/type_spec.rs`, `idl-parse/src/decl.rs` (`parse_declarator_list`, `operation_ahead`), `idl-lower/src/lower.rs` (`pragma`).

**Tests:** `shift_right_in_an_expression_is_not_split`, `a_pragma_id_sets_the_repository_id_like_typeid_does`, `a_cyclone_pragma_keylist_reaches_the_same_field_as_rtis_keylist`, `a_leading_double_colon_is_the_global_scope`, `an_array_declarator_is_only_known_at_the_bracket`.

**Status:** done

## §3.4 Präprozessor

**Spec:** `#include`, `#define`, `#undef`, die `#if`-Familie und die `#pragma`-Formen. Die Expansion erzeugt eine Source-Map: jedes Token löst auf Datei, Bereich **und** Makrokette auf. Ein `#include`-Zyklus wird gemeldet, nicht verfolgt.

**Repo:** `idl-parse/src/preprocess/mod.rs`, `idl-parse/src/preprocess/macro_.rs`, `parse_preprocessed`.

**Tests:** `an_included_file_reaches_the_tree`, `a_macro_is_expanded_before_the_parser_sees_it`, `a_conditional_branch_that_is_off_does_not_reach_the_tree`, `an_include_cycle_is_diagnosed_and_not_followed`, `the_map_covers_every_token_of_the_stream`, `tokens_from_an_included_file_name_that_file_not_the_includer`, `an_expanded_token_names_both_the_text_and_the_macro_that_produced_it`, `a_fixture_with_an_include_gains_the_declarations_it_includes`, `the_expanded_text_is_itself_lexable`, `crates/idl-parse/tests/source_map.rs::a_token_after_comments_maps_to_its_own_bytes`, `crates/idl-parse/tests/source_map.rs::a_token_from_an_include_names_that_file`.

**Status:** done

## §3.5 Dialekte

**Spec:** `Dialect` ist eine Flag-Menge, Voreinstellung striktes IDL 4.2. Ein Vendor-Flag lässt dessen Erweiterungen zu und leitet sie in die Facette. Ein nur per Flag zugelassenes Konstrukt hält fest, welches Flag es zuließ.

**Repo:** `idl-parse/src/dialect.rs`, `idl-parse/src/decl.rs` (`parse_keylist`), `idl-lower/src/lower.rs` (`keylist` → `RtiFacet::keylist`).

**Tests:** `a_vendor_extension_is_rejected_under_the_strict_dialect`, `the_same_file_is_accepted_under_the_vendors_dialect_and_still_says_so`, `a_rejected_extension_still_parses_and_round_trips`, `an_rti_keylist_reaches_the_vendor_facet_and_the_members`.

**Status:** done

## §3.6 Suffix-Annotationen — DDS-XTypes 1.3 §7.3.1.2.5

**Spec:** DDS-XTypes 1.3 §7.3.1.2.5, Tabelle 24, S. 91 — `{ "//@"<annotation_type_name> [ "(" <arguments> ")" ] }*`, mit der Erläuterung: „Alternately and equivalently, apply an annotation to a type or type member by suffixing it with an annotation type name using slash-slash-at (“//@”) instead of the at sign by itself.“ §7.3.1.2.3, S. 89 legt die Erkennung fest — „there must be no space in between the double slash (“//”) and the at-sign (‘@’)“ — und zeigt die Beispiele `long my_integer; //@my_member_annotation("Hello")`, `}; //@my_type_annotation` sowie zwei Annotationen an einem Member, die zweite auf der Folgezeile.

**Repo:** `crates/idl-parse/src/suffix.rs` (`parse_suffix_annotations`, `suffix_segments`, `parse_suffix_body`, `diagnose_stray_suffix`), `crates/idl-cst/src/token.rs` (`TokenKind::SlashSlashAt`), `crates/idl-parse/src/decl.rs` (`parse_annotation_tail`, gemeinsamer Rumpf beider Formen), `crates/idl-parse/src/parser.rs` (`emit_leading_trivia`), `crates/idl-parse/src/preprocess/mod.rs` (`flush_trailing_pragmas`, die Trivia-Schleife in `read_file`), `crates/idl-lower/src/annotation.rs` (`suffix_annotations_of`), `crates/idl-lower/src/lower.rs` (`definition`).

**Tests:** `crates/idl-lower/tests/annotations.rs::the_suffix_form_marks_the_key_the_interop_suite_declares`, `crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`, `crates/idl-lower/tests/annotations.rs::a_suffix_annotation_carries_its_arguments`, `crates/idl-lower/tests/annotations.rs::the_suffix_form_applies_to_the_type_whose_brace_precedes_it`, `crates/idl-lower/tests/annotations.rs::several_suffix_annotations_apply_to_the_same_element`, `crates/idl-lower/tests/annotations.rs::two_suffix_annotations_on_one_line_both_apply`, `crates/idl-lower/tests/annotations.rs::a_suffix_annotation_that_suffixes_nothing_is_refused`, `crates/idl-lower/tests/annotations.rs::an_unknown_suffix_annotation_gets_the_prefix_forms_answer`, `crates/idl-lower/tests/annotations.rs::text_after_a_suffix_annotation_is_reported`, `crates/idl-lower/tests/annotations.rs::only_the_exact_marker_opens_a_suffix_annotation`, `crates/idl-lower/tests/annotations.rs::a_file_with_suffix_annotations_round_trips`, `crates/idl-lower/tests/annotations.rs::the_suffix_form_reaches_literals_and_union_cases`, `crates/idl-parse/tests/preprocess.rs::a_suffix_annotation_survives_preprocessing`, `crates/idl-parse/tests/preprocess.rs::a_suffix_annotation_at_the_end_of_the_unit_survives_preprocessing`, `crates/idl-parse/tests/preprocess.rs::an_ordinary_comment_and_a_cut_branch_carry_nothing_through`, `crates/idl-parse/tests/parse.rs::a_suffix_annotation_reaches_the_tree_as_an_annotation`, `crates/idl-parse/tests/parse.rs::a_file_with_suffix_annotations_round_trips_byte_for_byte`, `crates/idl-parse/tests/parse.rs::a_suffix_annotation_in_a_position_that_admits_none_is_reported`, `crates/idl-parse/tests/parse.rs::only_the_exact_slash_slash_at_marker_is_the_suffix_form`.

Gelesen wird die Form dort, wo die Deklaration, die sie trägt, gerade fertig geparst ist: nach dem `;` eines Members, eines Union-Case-Members, eines Bitfields, eines Annotation-Members, einer Operation, eines Attributs, eines Ports und jeder Typdeklaration (`struct`, `union`, `enum`, `bitmask`, `bitset`, `typedef`, `const`, `native`, `exception`, `interface`, `valuetype`, `module`, `@annotation`, Template-Instanziierung, `import`, `typeid`/`typeprefix`, `keylist`) sowie nach dem Namen eines Enumerators oder Bitmask-Werts, auf beiden Seiten des Kommas. Zwischen Deklaration und Annotation und zwischen zwei Annotationen darf nur Leerraum stehen; ein Kommentar dazwischen beendet den Lauf. Jedes nicht gelesene `//@` wird in `emit_leading_trivia` als Fehler mit Byte-Bereich gemeldet, unter `IDL0001` und mit Spec-Referenz `DDS-XTypes 1.3 §7.3.1.2.5`. Ein unbekannter Name bekommt dieselbe Antwort wie in der Präfix-Form: `apply_user_annotation` legt ihn als `AppliedAnnotation` mit `Ref::Pending` in die Facette. Der Präprozessor gibt keine Kommentare aus, ein `//@` erreichte den Parser über `parse_preprocessed` also gar nicht — den Weg, den `zerodds-idlc` und jedes Build-Skript über `zerodds-idl-compose` nehmen. Es wird jetzt wie ein `#pragma` gehalten und an das folgende Token gehängt; eines am Ende der Übersetzungseinheit bekommt ein Dateiende-Token zum Führen, weil das letzte Token die Deklaration selbst ist und dort keine Annotation stehen darf.

**Status:** done

## §4 Lowering in den Common AST

**Spec:** Ein Durchlauf über den CST, gebaut über die transaktionale Edit-Fläche. Modulwiederöffnung konvergiert über `Ast::ensure_module`. Jeder erzeugte Knoten trägt eine `SourceFacet` mit Datei, Byte-Bereich und Makrokette.

**Repo:** `idl-lower/src/lower.rs`, `idl-lower/src/scope.rs`, `idl-lower/src/expr.rs`, `idl-lower/src/annotation.rs`. Das Lowering liest den expandierten Text; `idl-compose/src/lib.rs` (`place_nodes_in_their_files`) setzt danach die `SourceFacet` jedes Knotens auf die Datei, in der er steht, den Bereich in dieser Datei und die Makrokette, die ihn geschrieben hat, über `idl-parse/src/preprocess/mod.rs` (`Preprocessed::provenance`) — für eine Einheit und für jede Einheit eines Bündels.

**Tests:** `a_module_reopened_across_two_files_converges_on_one_node`, `every_lowered_type_names_the_file_it_came_from`, `every_node_of_every_fixture_names_the_file_it_came_from`, `the_definition_completes_the_forward_node_rather_than_adding_a_second`, `a_reference_taken_against_a_forward_declaration_survives_the_definition`; `crates/idl-compose/tests/node_provenance.rs` (eingebundene Datei, Makroexpansion, Bündel, Position eines nicht aufgelösten Namens).

**Status:** done

## §5 Completion — Member-ID

**Spec:** `@id(n)`, sonst `@hashid`, sonst das umgebende `@autoid`, sonst der vorige Member plus eins ab 0. Jeder Wert hält seine Herkunft fest.

**Repo:** `idl-lower/src/complete.rs` (`member_ids`, `hash_id` mit MD5 nach XTypes 1.3 §7.3.1.2.1.1).

**Tests:** `members_without_annotations_number_from_zero_upwards`, `a_declared_id_shifts_the_numbering_of_what_follows`, `autoid_sequential_and_autoid_hash_differ`, `a_hashid_uses_the_name_it_names_rather_than_the_members_own`, `a_declared_id_wins_over_the_aggregates_autoid`, `a_hashed_id_stays_within_the_28_bit_member_id_space`, `the_digest_matches_the_published_vectors` (RFC 1321 Anhang A.5).

**Status:** done

## §5 Completion — Extensibility

**Spec:** Die Annotation, sonst `@extensibility(...)`, sonst das umgebende `#pragma`, sonst `@appendable`.

**Repo:** `ast/src/query.rs` (`effective_extensibility`), `ast/src/lib.rs` (`DEFAULT_EXTENSIBILITY`), `idl-lower/src/complete.rs`.

**Tests:** `a_type_that_said_nothing_is_appendable_by_rule_and_silent_in_the_facet`, `a_written_extensibility_is_what_the_rule_reports`, `each_extensibility_keyword_reaches_the_facet`.

**Status:** done

## §5 Completion — Enum-Werte und Bit-Positionen

**Spec:** `@value(n)` bzw. `@position(k)`, sonst der vorige plus eins ab 0.

**Repo:** `ast/src/query.rs` (`effective_enumerator_value`, `effective_bit_position`).

**Tests:** `enumerators_without_values_run_from_zero_upwards`, `an_explicit_enumerator_value_shifts_the_ones_after_it`, `bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`, `an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`.

**Status:** done

## §5 Completion — Bit-Bound

**Spec:** `@bit_bound(n)`, sonst 32 für Enum und Bitmask.

**Repo:** `ast/src/query.rs` (`effective_bit_bound`), `ast/src/lib.rs` (`DEFAULT_BIT_BOUND`).

**Tests:** `an_enum_without_a_bit_bound_is_thirty_two_bits`, `a_written_bit_bound_is_what_the_rule_reports`.

**Status:** done

## §5 Completion — Konstantentyp und Array-Dimension

**Spec:** Der deklarierte Typ; der Ausdruck wird gegen ihn geprüft, nicht zur Ableitung benutzt. Die Dimension ist der ausgewertete Konstantenausdruck.

**Repo:** `idl-lower/src/lower.rs` (`constant`, `array_dimensions`), `idl-lower/src/expr.rs`, `ast/src/model.rs` (`eval_integral`, `eval_const`). Ein Gleitkommaausdruck wird in `double` ausgewertet (IDL 4.2 §7.4.1.4.2); `long double`-Arithmetik wird benannt abgelehnt (`EvalError::LongDouble`), weil der Baum keine Arithmetik dieser Breite hat. Ein Festkommaausdruck wird exakt nach Tabelle 7-11 ausgewertet und auf 31 signifikante Stellen gekappt, ohne Rundung (`ast/src/decimal.rs`).

**Tests:** `a_constant_takes_its_declared_type_not_the_expressions`, `an_array_dimension_is_the_evaluated_constant_expression`, `a_multi_dimensional_array_keeps_its_dimensions_in_order`, `a_bounded_string_and_sequence_carry_their_bound_expressions`, `crates/idl-lower/tests/floating_constant_arithmetic.rs::the_arithmetic_is_double`, `crates/idl-lower/tests/floating_constant_arithmetic.rs::long_double_arithmetic_is_refused_and_a_long_double_literal_is_not`, `crates/idl-lower/tests/fixed_constant_arithmetic.rs::a_quotient_is_exact_or_cut_without_rounding`.

**Status:** done

## §5 Completion — Repository-ID

**Spec:** `#pragma ID`, sonst aus dem Scoped Name abgeleitet, mit `#pragma prefix` und `#pragma version` angewandt.

**Repo:** `idl-lower/src/complete.rs` (`derive_repository_ids`, `derived_repository_id`), `idl-lower/src/lower.rs` (`repository_directive`, `pragma`).

**Tests:** `a_type_gets_a_repository_id_derived_from_its_scoped_name`, `a_typeid_directive_sets_the_id_outright`, `a_typeprefix_directive_prefixes_the_ids_derived_below_it`, `a_pragma_id_sets_the_repository_id_like_typeid_does`, `a_pragma_version_changes_the_version_a_derived_id_ends_in`, `a_pragma_prefix_prefixes_the_ids_of_its_module`.

**Status:** done

## §6 Validierung

**Spec:** Kollision nur nach Groß-/Kleinschreibung; mehrdeutig aufgelöster Name; Union-Diskriminator integral, boolean, Zeichen oder Enum; Case-Labels eindeutig und im Bereich; `@key` nicht `@optional`; Array-Dimension positiv; `fixed<d,s>` mit `0 < d <= 31` und `0 <= s <= d`; String-Bound positiv; Forward-Deklarationen definiert.

**Repo:** `idl-lower/src/validate.rs`, `ast/src/invariant.rs` (die strukturellen), `idl-lower/src/lower.rs` (`report_model`, `classify`).

**Tests:** `crates/idl-lower/tests/validation.rs` — 34 Tests, für jede Regel einer, der sie auslöst, und einer, der es nicht tut: `two_names_differing_only_by_case_collide`, `a_float_discriminator_is_rejected`, `a_typedef_to_a_legal_discriminator_is_accepted`, `a_repeated_case_label_is_rejected`, `a_case_label_outside_the_discriminators_range_is_rejected`, `a_key_member_must_not_be_optional`, `a_zero_array_dimension_is_rejected`, `a_fixed_with_more_than_thirty_one_digits_is_rejected`, `a_forward_declaration_never_defined_is_rejected`.

**Status:** done

## §7 Diagnosen

**Spec:** Stabile, greppbare Codes; keine Diagnose ohne Spec-Referenz; eine Regel ohne Absatz ist ZeroDDS-Policy und sagt das.

**Repo:** `idl-parse/src/diagnostic.rs` (`IDL0001`–`IDL0022`, `Diagnostic::file`), `idl-parse/src/preprocess/mod.rs` (`SourceMap::original`, `Preprocessed::locate`), `idl-lower/src/validate.rs` (`rule_name`), `idl-lower/src/lower.rs` (`dedup`). Ein Bereich verweist auf die Datei, in der er steht, nicht auf den expandierten Text.

**Tests:** `every_diagnostic_carries_a_code_and_a_specification_reference`, `every_diagnostic_names_a_specification_paragraph`, `every_diagnostic_points_at_a_byte_range`, `every_diagnostic_code_has_its_own_string`, `a_preprocessor_diagnostic_comes_before_the_parsers`, `tools/idlc/tests/diagnostic_positions.rs::comments_above_an_error_do_not_move_it`, `tools/idlc/tests/diagnostic_positions.rs::an_error_in_an_include_names_the_include`.

**Status:** done

## §8 Konformanz

**Spec:** Jede Fixture aus `e1f3af895` parst; der Round-Trip ist byte-exakt; das erzeugte Modell erfüllt `validate_complete()`.

**Repo:** `crates/idl-parse/tests/fixtures/` (21 Dateien, `LICENSES.md` je Vendor-Satz erhalten), `crates/idl-lower/tests/conformance.rs`.

**Tests:** `every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares`, `every_fixture_round_trips_byte_for_byte`, `every_fixture_lowers_to_exactly_the_diagnostics_it_should`, `the_expected_diagnostics_are_the_ones_the_fixtures_actually_earn`, `every_fixture_produces_a_model_with_declarations_in_it`, `every_fixture_survives_a_snapshot_round_trip`, `the_xtypes_specifications_own_idl_lowers_completely`.

Zwei Fixtures verdienen eine Diagnose, benannt und begründet in `EXPECTED`: `ccm_components.idl` deklariert `CmdBase` und `CompForward` vorwärts ohne Definition; `fastdds_pragmas.idl` gibt zwei Membern dasselbe `@hashid("position")`; `corba_value_repository.idl` nennt in `typeid`/`typeprefix` ein `Bar`, das die Datei nicht deklariert.

**Status:** done

---

## Audit-Status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-cst -p zerodds-idl-parse -p zerodds-idl-lower` — 422 bestanden, 0 fehlgeschlagen.
Gesamt über die sechs Crates des Bogens (`ast-core`, `ast`, `ast-layout`, `idl-cst`, `idl-parse`, `idl-lower`): 744 bestanden.

**Offene Punkte:**

- Ein Item dieses Bogens steht in `docs/OPEN-ITEMS.md` und nicht hier: DDS-XTypes 1.3 hat keine eigene Coverage-Seite, seit die Demolition sie entfernt hat (`internal/spec-coverage/legacy-audit.md` §B.2). §3.6 liegt deshalb in diesem Dokument, und Tabelle 22 desselben Unterabschnitts — `struct A { … }; //@annotation` als *Deklaration* einer Annotation — liegt in keinem.
- Die Sprach-CSTs (`zerodds-lang-cst-1.0`) sind noch nicht gebaut; ihre Coverage ist ein eigenes Dokument.
