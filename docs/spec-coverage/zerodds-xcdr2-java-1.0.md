# `zerodds-xcdr2-java` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Java-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Java-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/java/`, `crates/lang-render/src/java.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_java_class_renders_its_modifiers_annotations_and_members`, `a_java_enum_ends_its_constants_with_a_semicolon_only_when_members_follow`, `a_java_generic_type_renders_its_arguments_with_dots_in_the_path`, `a_java_file_writes_its_package_and_imports`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/java.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/java.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_java.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/java`, Endpoint-SDK `endpoints/java`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht den Java-Typ, der es hält

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`

**Status:** done

### IDL 4.2 §7.4.13 `fixed` wird ein `BigDecimal`


**Spec:** IDL 4.2 §7.4.13 — `fixed<P,S>` — eine Dezimalzahl mit deklarierter Stellenzahl und Skalierung.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_fixed_is_a_bigdecimal`, `crates/lang-rules/tests/java_runtime_selftest.rs`

**Status:** done

### §7.2.2.4.3 Sequenz und Array sind beide Java-Arrays

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`

**Status:** done

### §7.2.2.4.3 Eine `map` wird eine Klasse über `java.util.LinkedHashMap`; zwei IDL-Formen bleiben zwei Klassen


**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_map_is_a_java_util_map_over_a_generated_entry_class`,
`crates/lang-rules/src/adapter/java.rs::tests::two_maps_of_one_shape_are_one_entry_class`,
`crates/lang-rules/src/adapter/java.rs::tests::two_maps_of_one_java_shape_and_two_idl_shapes_are_two_entry_classes`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member wird geboxt, auch ein primitives

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_optional_member_is_boxed_and_a_primitive_one_too`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird eine Klasse mit `int`-Konstanten; ein Member davon ist eine Instanz


**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one`,
`crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`

**Status:** done

### IDL 4.2 §7.5.2 `module` wird eine geschachtelte Klasse; ein wiedereröffnetes Modul bleibt eine Klasse

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_module_is_a_nested_class_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/java.rs::tests::a_module_reopened_beside_a_nested_one_is_one_class`,
`crates/lang-rules/src/adapter/java.rs::tests::a_declaration_named_after_the_outer_class_is_refused`

**Status:** done

### IDL 4.2 §7.2.3 Ein Java-Keyword wird entschärft; ein Name, der die qualifizierten Verweise fangen würde, wird abgelehnt

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_identifier_that_is_a_java_keyword_is_escaped`,
`crates/lang-rules/src/adapter/java.rs::tests::a_name_that_would_capture_the_qualified_references_is_refused`

**Status:** done

### §7.4.3 Der Java-Backend buchstabiert die Formen, die der Materialisierer deklariert; ein unsigned Local ist breit genug

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/java.rs`

**Tests:** `crates/lang-rules/tests/one_walk_two_targets.rs::the_java_backend_spells_the_shapes_the_materialiser_declares`,
`crates/lang-rules/tests/one_walk_two_targets.rs::an_unsigned_local_is_wide_enough_in_java`

**Status:** done

### IDL 4.2 §7.4.3 `interface` bindet dieselben Entities wie C++, mit `Future` und Out-Typen

**Spec:** IDL 4.2 §7.4.3 — `interface` — Operationen mit `in`/`out`/`inout`-Parametern.

**Repo:** `crates/lang-rules/src/adapter/interface.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_a_future_of_the_boxed_result`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_the_out_type_where_cpp_takes_a_reference`,
`crates/lang-rules/tests/interface_bindings.rs::java_carries_the_topic_names_as_constants`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/java.rs`, `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen

**Spec:** §7.4 — Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen. Ein stillschweigend ausgelassenes Konstrukt ist die schlimmste Fehlerklasse: der Empfänger sieht keinen Fehler, sondern falsche Daten.

**Repo:** `crates/lang-rules/src/emit/java.rs`, `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_construct_with_no_java_data_form_is_named_and_not_left_out`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/emit/java.rs`, `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::java_wraps_the_file_and_fills_the_class_body`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang java`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-java-gen/`, `tools/wireprobe/wireprobe-zerodds-java`, `tools/wireprobe/wireprobe-java-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
