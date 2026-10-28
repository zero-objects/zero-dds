# `zerodds-xcdr2-fsharp` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die F#-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die F#-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/fsharp/`, `crates/lang-render/src/fsharp.rs`

**Tests:** `crates/lang-render/src/fsharp.rs::tests` (10 Tests), darunter `a_class_declaration_indents_by_four_and_writes_member_val`, `an_empty_class_writes_class_end`, `a_type_reads_left_to_right_with_no_declarator_to_split`, `an_enum_writes_its_backing_type_once_and_every_value`, `a_constant_writes_its_ascription_before_its_value`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/fsharp.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/fsharp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_fsharp.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/fsharp`, Endpoint-SDK `endpoints/fsharp`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen F#-Typ

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::every_idl_primitive_reaches_an_fsharp_type`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::a_struct_of_primitives_becomes_an_fsharp_class`

**Status:** done

### §7.2.2.2 `long double` hält Dezimaltext, weil .NET kein binary128 hat

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::a_long_double_holds_decimal_text_because_dotnet_has_no_binary128`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::a_fixed_holds_its_decimal_as_text`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member wird eine `option` mit Default `None`

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::an_optional_member_becomes_an_option_and_defaults_to_none`

**Status:** done

### §7.2.2.4.9 Ein `@external`-Member bleibt inline, weil eine Klasse bereits eine Referenz ist

**Spec:** §7.2.2.4.9 — `@external` — das Member wird über eine Referenz gehalten; Selbstbezug ist erlaubt.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::an_external_member_stays_inline_because_a_class_is_a_reference_already`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::an_external_union_branch_stays_inline_too`

**Status:** done

### §7.2.2.4.3 Eine `map` wird ein Array von Einträgen und nie ein Dictionary

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::a_map_becomes_an_array_of_entries_and_never_a_dictionary`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::a_sequence_becomes_an_array_and_a_string_stays_a_string`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration hält ihre Fälle mit jedem ausgeschriebenen Wert

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::an_enum_holds_its_cases_with_every_value_written_out`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::a_bitmask_becomes_a_type_abbreviation_and_one_constant_per_bit`

**Status:** done

### IDL 4.2 §7.2.3 Ein F#-Keyword wird entschärft und behält seinen IDL-Namen

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::a_member_named_after_an_fsharp_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::a_declaration_named_after_a_primitive_is_escaped`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::two_members_that_would_reach_one_fsharp_name_are_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flacht auf den Namen ab, weil `module rec` IDL-Module nicht verschachtelt

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::a_module_flattens_onto_the_name_because_module_rec_does_not_nest_idl_modules`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::the_module_line_always_carries_rec`

**Status:** done

### §7.4.3 Das erzeugte F# dekodiert Primitive, Sammlungen, Enums, Masken, Klassen und Rekursion

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::the_generated_fsharp_decodes_primitives_and_collections`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::the_generated_fsharp_decodes_enums_and_masks`,
`crates/lang-rules/tests/fsharp_end_to_end.rs::the_generated_fsharp_decodes_classes_and_recursion`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/fsharp.rs`, `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/fsharp.rs`, `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::the_generated_fsharp_compiles`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/fsharp.rs`, `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/emit/fsharp.rs`, `crates/lang-rules/src/adapter/fsharp.rs`

**Tests:** `crates/lang-rules/tests/fsharp_end_to_end.rs::every_construct_the_catalogue_covers_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/fsharp.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang f#`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-fsharp-gen/`, `tools/wireprobe/wireprobe-zerodds-fsharp`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

22 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
