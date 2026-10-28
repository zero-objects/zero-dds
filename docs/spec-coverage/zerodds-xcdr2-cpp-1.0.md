# `zerodds-xcdr2-cpp` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die C++-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die C++-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/cpp/`, `crates/lang-render/src/cpp.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_cpp_class_renders_its_access_sections_in_order`, `a_cpp_accessor_with_one_statement_stays_on_one_line`, `a_cpp_constructor_with_initialisers_puts_them_on_their_own_line`, `a_cpp_expression_brackets_only_where_the_tree_asked_for_it`, `a_cpp_namespace_body_is_not_indented`, `a_cpp_template_specialisation_renders_its_header_above_the_declaration`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/cpp.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_cpp.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/cpp`, Endpoint-SDK `endpoints/cpp`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes Member trägt den Laufzeit-Typ und nie den des Übersetzers

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_long_double_is_the_runtime_type_and_never_the_compiler_s`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member ist ein Zeiger und kein zweites Feld


**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`,
`crates/lang-rules/tests/cpp_reader_generates.rs::an_optional_member_gets_a_reader`

**Status:** done

### §7.2.2.4.3 Eine Sequenz wird ein `vector`, eine `map` die Klasse `zerodds::Map` über einem `vector` von Paaren


**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs`

**Status:** done

### §7.2.2.4.3 Ein Array verschachtelt mit der äußersten Dimension zuerst

**Spec:** §7.2.2.4.3 — Array — mehrdimensional, feste Elementzahl, row-major.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::an_array_nests_with_the_outer_dimension_outermost`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration nimmt ihren Underlying Type aus dem `bit_bound`

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::an_enumeration_takes_an_underlying_type_from_its_bit_bound`,
`crates/lang-rules/tests/cpp_reader_generates.rs::an_enumeration_whose_first_literal_is_not_zero`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird eine Klasse mit Diskriminator und einem Feld je Case

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`,
`crates/lang-rules/tests/cpp_reader_generates.rs::a_union_gets_a_reader`

**Status:** done

### IDL 4.2 §7.5.2 `module` wird ein Namensraum und kein Präfix; Verweise werden von der Wurzel geschrieben

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_module_is_a_namespace_and_not_a_prefix`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_reference_across_modules_is_written_from_the_root`

**Status:** done

### IDL 4.2 §7.2.3 Das Tag hält auseinander, was die C++-Schreibweise zusammenzieht

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::the_tag_keeps_apart_what_the_cpp_spelling_collapses`,
`crates/lang-rules/src/adapter/cpp.rs::tests::an_any_member_is_refused_by_name_at_any_depth`

**Status:** done

### §7.2.2.4.4.4.9 Jede Art, für die Tab.9 antwortet, bekommt einen Defaulter

**Spec:** §7.2.2.4.4.4.9 — Ein Member, das der Strom nicht trug, nimmt seinen Default nach Tab.9.

**Repo:** `crates/lang-rules/src/default_cpp.rs`

**Tests:** `crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** done

### §7.4.3 Rahmung: ein appendable und ein mutable Aggregat bekommen jeweils einen Leser

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`

**Tests:** `crates/lang-rules/tests/cpp_reader_generates.rs::an_appendable_struct_gets_a_reader`,
`crates/lang-rules/tests/cpp_reader_generates.rs::a_mutable_struct_gets_a_reader`,
`crates/lang-rules/tests/cpp_reader_generates.rs::a_bounded_string_gets_a_reader`

**Status:** done

### IDL 4.2 §7.4.3 `interface` bindet die Entities von DDS-RPC in C++

**Spec:** IDL 4.2 §7.4.3 — `interface` — Operationen mit `in`/`out`/`inout`-Parametern.

**Repo:** `crates/lang-rules/src/adapter/interface.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_prefixes_a_reserved_word_with_cxx`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`, `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::every_corpus_case_renders_or_names_its_refusal`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`, `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_places_all_six_slots_in_spec_order`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang c++`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-cpp-gen/`, `tools/wireprobe/wireprobe-zerodds-cpp`, `tools/wireprobe/wireprobe-cpp-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
