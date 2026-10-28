# `zerodds-xcdr2-ts` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die TypeScript-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die TypeScript-Spalte selbst entscheidet, und ihre eigenen Nachweise.

Die Seite heißt aus historischen Gründen `ts`; die Spalte selbst heißt `typescript` — so nimmt sie `idlc generate --lang`, so steht sie in `crates/lang-cst/src/common.rs` und so heißt ihr Verzeichnis unter `runtime/`.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/typescript/`, `crates/lang-render/src/typescript.rs`

**Tests:** `crates/lang-cst/src/typescript/mod.rs::tests` (9 Tests):
`every_type_keyword_writes_its_own_spelling`,
`a_reserved_binding_escapes_with_a_trailing_underscore`,
`a_contextual_keyword_is_a_legal_binding_and_keeps_its_spelling`,
`a_predefined_type_name_is_reserved_although_javascript_does_not_reserve_it`,
`a_property_name_keeps_its_text_whichever_form_it_takes`,
`a_bigint_literal_type_carries_its_suffix_in_the_written_form`;
`crates/lang-render/tests/dispatch_chains.rs::a_node_dispatch_chain_of_ten_thousand_arms_renders_flat`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/typescript.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_typescript.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/typescript`, Endpoint-SDK `endpoints/typescript`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Ein 64-Bit-Integer wird ein `bigint`, alles Schmalere eine `number`

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_sixty_four_bit_integer_is_a_bigint_and_everything_narrower_is_a_number`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_bigint_field_starts_from_a_bigint_literal`

**Status:** done

### §7.5.1 Ein Struct wird ein Interface und eine Factory, nie eine Klasse

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_struct_is_an_interface_and_a_factory_and_never_a_class`

**Status:** done

### §7.2.2.4.3 Ein `map`-Member wird eine `readonly Map` über einem gestaffelten Entry-Typ

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_map_member_is_a_readonly_map_over_a_staged_entry_type`,
`crates/lang-rules/src/adapter/typescript.rs::tests::two_maps_of_one_typescript_shape_and_two_idl_shapes_are_two_entry_types`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member ist ein Fragezeichen und kein `null`

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::an_optional_member_is_a_question_mark_and_not_a_null`,
`crates/lang-rules/tests/typescript_reader_generates.rs::an_optional_member_gets_a_reader`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird eine diskriminierte Union, verengt auf ein Literal

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_union_is_a_discriminated_union_narrowed_on_a_literal`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_union_discriminant_takes_the_discriminators_own_literal_form`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird ein `as const`-Objekt neben einer String-Literal-Union

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::an_enumeration_is_an_as_const_object_beside_a_string_literal_union`,
`crates/lang-rules/tests/typescript_reader_generates.rs::a_signed_enumerator_pattern_gets_a_reader`

**Status:** done

### IDL 4.2 §7.4.1.4 Eine Konstante jenseits des sicheren Integer-Bereichs wird abgelehnt statt gerundet

**Spec:** IDL 4.2 §7.4.1.4 — `const` — ein zur Übersetzungszeit ausgewerteter Wert seines deklarierten Typs.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_constant_past_the_safe_integer_range_is_refused_rather_than_rounded`,
`crates/lang-rules/src/adapter/typescript.rs::tests::an_unevaluated_array_length_is_named_and_not_substituted`

**Status:** done

### IDL 4.2 §7.5.2 `module` wird ein Namensraum; ein wiedereröffnetes Modul bleibt einer

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_module_is_a_namespace_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_module_reopened_beside_a_nested_one_is_one_namespace`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_nested_name_that_would_capture_a_root_reference_is_refused`

**Status:** done

### §7.4 Das erzeugte Modul benennt nichts, was die Laufzeit nicht hat

**Spec:** §7.4 — Data Representation — die erzeugte Quelle ruft eine Laufzeit, die §7.4 aus dem Klauseltext liest.

**Repo:** `runtime/typescript`

**Tests:** `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`,
`crates/lang-rules/tests/typescript_names_its_runtime.rs::what_the_runtime_carries_and_the_backend_has_not_reached`

**Status:** done

### §7.4.3 Jede Rahmungsform bekommt einen Leser, und der erzeugte Leser typprüft

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`

**Tests:** `crates/lang-rules/tests/typescript_reader_generates.rs::a_mutable_struct_gets_a_reader`,
`crates/lang-rules/tests/typescript_reader_generates.rs::an_appendable_struct_gets_a_reader`,
`crates/lang-rules/tests/typescript_reader_generates.rs::the_generated_reader_type_checks`,
`crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`

**Status:** done

### IDL 4.2 §7.4.3 `interface` wird ein Client, ein Handler und ein Service-Descriptor

**Spec:** IDL 4.2 §7.4.3 — `interface` — Operationen mit `in`/`out`/`inout`-Parametern.

**Repo:** `crates/lang-rules/src/adapter/interface.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`,
`crates/lang-rules/tests/interface_bindings.rs::typescript_aggregates_out_parameters_into_the_resolved_value`,
`crates/lang-rules/tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`

**Status:** done

### §7.4 Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen

**Spec:** §7.4 — Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen. Ein stillschweigend ausgelassenes Konstrukt ist die schlimmste Fehlerklasse: der Empfänger sieht keinen Fehler, sondern falsche Daten.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_construct_with_no_typescript_data_form_is_named_and_not_left_out`

**Status:** done

### §7.4 Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten

**Spec:** §7.4 — Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten. Gemessen gegen `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, nicht neben den anderen Spalten: drei Spalten, die sich gemeinsam bewegt haben, ähneln einander weiterhin.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/wire-probe/tests/typescript_column_gate.rs::the_typescript_column_writes_what_the_rust_column_recorded`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang typescript`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-typescript-gen/`, `tools/wireprobe/wireprobe-zerodds-typescript`, `tools/wireprobe/wireprobe-typescript-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
