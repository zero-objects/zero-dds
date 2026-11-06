# `zerodds-xcdr2-d` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die D-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die D-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/d/`, `crates/lang-render/src/d.rs`

**Tests:** `crates/lang-render/src/d.rs::tests` (20 Tests), darunter `xtypes_7_5_1_a_file_declares_its_module_first`, `xtypes_7_5_1_an_import_is_selective_when_it_names_symbols`, `xtypes_7_5_1_a_struct_writes_its_brace_on_a_line_of_its_own`, `xtypes_7_5_1_a_field_carries_its_uda_before_the_type`, `xtypes_7_5_1_an_array_writes_its_dimensions_right_to_left`;
querschnittlich `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/d.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/d.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_d.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/d`, Endpoint-SDK `endpoints/d`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen D-Typ, keiner ist zielabhängig

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_2_every_idl_primitive_reaches_a_d_type`,
`crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_2_no_generated_type_is_target_dependent_or_unimplemented`,
`crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_2_an_idl_long_is_a_d_int_and_an_idl_long_long_is_a_d_long`

**Status:** done

### §7.2.2.2.1.2 Ein IDL-`char` wird ein Integer und nie ein D-`char`

**Spec:** §7.2.2.2.1.2 — Char8 ist nicht spezifiziert (UTF-8 als Default), Char16 ist UTF-16 der BMP.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_2_1_2_an_idl_character_becomes_an_integer_and_never_a_d_char`

**Status:** done

### §7.2.2.2 `long double` wird ein String und nie ein `real`

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_2_a_long_double_is_a_string_and_never_a_real`

**Status:** done

### §7.2.2.4.3 Ein Array kehrt seine Dimensionen um, weil D sie von rechts liest

**Spec:** §7.2.2.4.3 — Array — mehrdimensional, feste Elementzahl, row-major.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_3_an_array_reverses_its_dimensions_because_d_reads_them_right_to_left`,
`crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_3_a_sequence_of_arrays_and_an_array_of_sequences_stay_apart`

**Status:** done

### §7.2.2.4.3 Eine `map` wird ein Slice von Einträgen und nie ein assoziatives Array

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_3_a_map_becomes_a_slice_of_entries_and_never_an_associative_array`

**Status:** done

### §7.2.2.4.9 Ein `@external`-Member wird ein Zeiger, unabhängig von seinem Typ

**Spec:** §7.2.2.4.9 — `@external` — das Member wird über eine Referenz gehalten; Selbstbezug ist erlaubt.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_9_an_external_member_becomes_a_pointer_whatever_its_type_is`,
`crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_3_1_2_1_4_an_optional_external_member_is_one_pointer_and_not_a_nullable_pointer`,
`crates/lang-rules/tests/d_codec.rs::xtypes_7_2_2_4_9_an_external_member_reads_through_zd_ptr_and_writes_the_dereference`

**Status:** done

### §7.2.2.4.9 Kein erzeugtes `struct` hält sich selbst per Wert; ein Zyklus ohne Abbruch wird benannt

**Spec:** §7.2.2.4.9 — Ein selbstbezüglicher Typ braucht eine Stelle, an der der Abstieg endet.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_9_no_generated_struct_holds_itself_by_value_anywhere_in_the_corpus`,
`crates/lang-rules/tests/d_codec.rs::xtypes_7_2_2_4_9_a_cycle_with_no_terminator_is_refused_by_name`,
`crates/lang-rules/tests/d_codec.rs::xtypes_7_2_2_4_9_the_recursive_external_shape_is_carried`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird ein `struct` mit Diskriminator und anonymer `union`

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_becomes_a_struct_with_a_discriminator_and_an_anonymous_union`,
`crates/lang-rules/tests/d_codec.rs::xtypes_7_2_2_4_4_3_a_union_assigns_through_the_anonymous_union`

**Status:** done

### IDL 4.2 §7.2.3 Ein D-Keyword wird entschärft und behält seinen IDL-Namen

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::idl_7_2_3_a_member_named_after_a_d_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/d_end_to_end.rs::idl_7_2_3_a_member_named_after_a_d_type_keyword_is_escaped_too`,
`crates/lang-rules/tests/d_end_to_end.rs::idl_7_2_3_two_members_that_would_reach_one_d_name_are_refused_by_name`

**Status:** done

### §7.4.3 Der Lesepfad öffnet mit Tab.9 und liefert das Sample zurück

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/d.rs`

**Tests:** `crates/lang-rules/tests/d_codec.rs::xtypes_7_4_3_a_structure_of_primitives_becomes_a_d_encoder`,
`crates/lang-rules/tests/d_codec.rs::xtypes_7_4_3_the_read_direction_opens_with_table_9_and_returns_the_sample`,
`crates/lang-rules/tests/d_codec.rs::xtypes_7_4_3_the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_3_1_the_generated_d_compiles`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::xtypes_7_2_2_4_every_construct_the_catalogue_covers_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/d.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang d`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-d-gen/`, `tools/wireprobe/wireprobe-zerodds-d`, `tools/wireprobe/wireprobe-d-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit-Status

23 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-d-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (Job `merge-rust-lang-render`) — 7 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 11 Tests grün.
* `python3 ci/run_integration.py merge-codegen-d` (Job `merge-codegen-d`) — 27 Tests grün.

47 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
