# `zerodds-xcdr2-lua` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Lua-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Lua-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/lua/`, `crates/lang-render/src/lua.rs`

**Tests:** `crates/lang-render/src/lua.rs::tests` (30 Tests), darunter `a_file_ends_with_the_return_its_module_needs`, `a_note_takes_two_dashes_and_an_annotation_three`, `a_luacats_tag_is_written_against_the_dashes_and_prose_is_not`, `a_commented_statement_starts_a_block_and_an_uncommented_one_does_not`, `an_empty_comment_line_carries_no_trailing_space`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/lua.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/lua.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_lua.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/lua`, Endpoint-SDK `endpoints/lua`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen Lua-Holder und sagt, welchen

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::every_idl_primitive_reaches_a_lua_holder_and_says_which`,
`crates/lang-rules/tests/lua_end_to_end.rs::the_one_integer_that_does_not_fit_says_so`,
`crates/lang-rules/tests/lua_end_to_end.rs::every_member_carries_the_facts_lua_cannot_state`

**Status:** done

### §7.2.2.2 `long double` wird ein String und nie ein Lua-Float

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_long_double_is_a_string_and_never_a_lua_float`,
`crates/lang-rules/tests/lua_end_to_end.rs::a_double_default_is_written_so_that_math_type_answers_float`

**Status:** done

### §7.2.2.4.3 Eine Sequenz sagt, dass sie eins-basiert ist; ein Array verschachtelt äußerste Dimension zuerst

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_sequence_says_that_it_is_one_based`,
`crates/lang-rules/tests/lua_end_to_end.rs::an_array_nests_outermost_dimension_first_and_is_filled`,
`crates/lang-rules/tests/lua_codec.rs::a_sequence_counts_zero_based_and_subscripts_one_based`

**Status:** done

### §7.2.2.4.3 Eine `map` wird ein Array von Paaren und nie eine nach K geschlüsselte Lua-Tabelle

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_map_becomes_an_array_of_pairs_and_never_a_lua_table_keyed_by_k`,
`crates/lang-rules/tests/lua_codec.rs::a_sorted_maps_char_key_wraps_and_its_u64_key_is_unsigned`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member bekommt gar keinen Schlüssel

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::an_optional_member_gets_no_key_at_all`,
`crates/lang-rules/tests/lua_codec.rs::an_optional_member_is_the_key_being_there`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration schreibt beide Richtungen und ihre Werte; ihr Default ist das erste Literal

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::an_enum_writes_both_directions_and_its_values`,
`crates/lang-rules/tests/lua_end_to_end.rs::an_enum_defaults_to_its_first_literal_and_not_to_zero`,
`crates/lang-rules/tests/lua_codec.rs::an_enumerations_wire_value_is_read_out_of_the_descriptor`

**Status:** done

### IDL 4.2 §7.2.3 Ein Lua-Keyword behält seinen Namen und wird in der Klammerform erreicht

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_member_named_after_a_lua_keyword_keeps_its_name`,
`crates/lang-rules/tests/lua_codec.rs::a_keyword_member_keeps_its_name_in_the_bracket_form`,
`crates/lang-rules/tests/lua_end_to_end.rs::the_executed_module_reaches_a_member_named_after_a_lua_keyword`

**Status:** done

### IDL 4.2 §7.5.2 `module` verschachtelt und flacht nicht auf den Namen ab; der Chunk gibt die Modultabelle zurück

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_module_nests_rather_than_flattening_onto_the_name`,
`crates/lang-rules/tests/lua_end_to_end.rs::the_chunk_returns_the_module_table`,
`crates/lang-rules/tests/lua_end_to_end.rs::a_named_reference_is_resolved_at_load_and_not_left_to_a_reader`

**Status:** done

### §7.4.3 Die kurzschließenden Operatoren sind Wörter, die Ungleichheit ist eine Tilde

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_codec.rs::the_short_circuiting_operators_are_words_and_inequality_is_tilde`,
`crates/lang-rules/tests/lua_codec.rs::a_decoder_returns_the_sample_it_built`,
`crates/lang-rules/tests/lua_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::the_generated_lua_loads`

**Status:** done

### §7.3.1 Das ausgeführte Modul meldet, was die Quelle deklariert hat

**Spec:** §7.3.1 — Das ausgeführte Modul meldet, was die Quelle deklariert hat. Ein Chunk, der lädt, ist noch kein Chunk, der antwortet.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::the_executed_module_reports_its_members_in_idl_declaration_order`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::every_construct_the_column_claims_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang lua`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-lua-gen/`, `tools/wireprobe/wireprobe-zerodds-lua`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

23 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
