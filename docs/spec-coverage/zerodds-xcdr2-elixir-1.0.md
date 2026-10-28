# `zerodds-xcdr2-elixir` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Elixir-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Elixir-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/elixir/`, `crates/lang-render/src/elixir.rs`

**Tests:** `crates/lang-render/src/elixir.rs::tests` (20 Tests), darunter `a_module_is_a_do_end_block_indented_by_two_spaces`, `a_module_name_joins_its_segments_with_dots`, `a_moduledoc_is_a_heredoc_at_the_top_of_the_body`, `a_struct_with_no_fields_is_defstruct_with_an_empty_list`, `a_struct_of_several_short_fields_stays_on_one_line`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/elixir.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_elixir.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/elixir`, Endpoint-SDK `endpoints/elixir`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Breite und Vorzeichen jedes Primitivs stehen im Attribut, und jedes Attribut hat eine Lesefunktion

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::every_width_and_signedness_is_written_into_the_attribute`,
`crates/lang-rules/tests/elixir_end_to_end.rs::every_attribute_the_output_writes_has_a_function_that_reads_it`

**Status:** done

### §7.2.2.2 `long double` wird ein Binary und nie ein Float

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_long_double_is_a_binary_and_never_a_float`

**Status:** done

### §7.2.2.3 `string`, `wstring` und `fixed` sind Binaries mit ihren Fakten daneben

**Spec:** §7.2.2.3 — STRING8 über Char8 und STRING16 über Char16, begrenzt oder unbegrenzt.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_string_a_wstring_and_a_fixed_are_all_binaries_with_their_facts_beside_them`

**Status:** done

### §7.2.2.4.3 Eine Sequenz ist eine Liste, ein Array ein Tupel, und nichts wird abgeschnitten

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_sequence_is_a_list_and_an_array_is_a_tuple`,
`crates/lang-rules/tests/elixir_end_to_end.rs::an_array_of_a_thousand_is_written_out_and_nothing_truncates_it`

**Status:** done

### §7.2.2.4.3 Eine `map` ist eine Liste von Paaren und nie eine Elixir-Map

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_map_is_a_list_of_pairs_and_never_an_elixir_map`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird ein Modul von Atomen mit ausgeschriebener Numerierung

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::an_enum_becomes_a_module_of_atoms_with_its_numbering_written_out`,
`crates/lang-rules/tests/elixir_end_to_end.rs::a_member_of_an_enumeration_is_an_integer_and_not_the_enumeration`

**Status:** done

### §7.2.2.4.4.3 Eine Union ist ein Struct mit Diskriminator und kein getaggtes Tupel

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_union_is_a_struct_with_a_discriminator_and_not_a_tagged_tuple`,
`crates/lang-rules/tests/elixir_end_to_end.rs::a_default_case_is_recorded_as_the_default_and_not_as_a_number`

**Status:** done

### IDL 4.2 §7.2.3 Ein reserviertes Wort wird entschärft, und eine Großschreibung wird als Atom mit ihrer Schreibweise festgehalten

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_member_named_after_an_elixir_reserved_word_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/elixir_end_to_end.rs::an_idl_member_with_a_capital_becomes_a_lower_case_atom_that_records_its_spelling`,
`crates/lang-rules/tests/elixir_end_to_end.rs::two_members_that_would_reach_one_atom_are_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` verschachtelt, weil Elixir-Modulnamen verschachteln; `Elixir` selbst wird entschärft

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::a_module_nests_because_elixir_module_names_nest`,
`crates/lang-rules/tests/elixir_end_to_end.rs::a_module_named_elixir_is_escaped_because_it_is_the_root_namespace`

**Status:** done

### §7.4.3 Jede Form kodiert und dekodiert, und das erzeugte Elixir übersetzt ohne Warnung

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_codec.rs::every_shape_encodes_and_the_generated_elixir_compiles_without_warnings`,
`crates/lang-rules/tests/elixir_codec.rs::every_shape_decodes_and_the_generated_elixir_compiles_without_warnings`,
`crates/lang-rules/tests/elixir_codec.rs::the_compiler_check_catches_a_deliberate_error`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::the_generated_elixir_compiles`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::every_construct_the_column_claims_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang elixir`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-elixir-gen/`, `tools/wireprobe/wireprobe-zerodds-elixir`, `tools/wireprobe/wireprobe-elixir-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

23 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
