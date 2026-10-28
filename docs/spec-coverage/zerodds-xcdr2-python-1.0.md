# `zerodds-xcdr2-python` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Python-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Python-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/python/`, `crates/lang-render/src/python.rs`

**Tests:** `crates/lang-render/src/python.rs::tests` (24 Tests), darunter `a_class_body_is_indented_by_four_and_closed_by_the_dedent_alone`, `a_class_with_a_docstring_and_no_members_is_still_legal_python`, `an_empty_body_takes_the_pass_the_tree_carries`, `an_import_carries_its_alias_and_its_from_form`, `a_call_takes_positional_and_keyword_arguments_in_written_order`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/python.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/python.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_python.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/python`, Endpoint-SDK `endpoints/python`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht eine Annotation, die seine Breite und sein Vorzeichen nennt

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::every_idl_primitive_reaches_an_annotation_that_states_its_width`,
`crates/lang-rules/src/adapter/python.rs::tests::every_width_alias_states_its_bits_and_its_signedness`

**Status:** done

### §7.2.2.2 `long double` wird der Dezimaltext und nie ein Python-Float

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::a_long_double_is_the_decimal_text_and_never_a_python_float`,
`crates/lang-rules/src/default_python.rs::tests::a_long_double_is_a_float128_and_never_a_float`

**Status:** done

### §7.2.2.4.3 Eine Sequenz ist eine Liste, ein Array ein Tupel seiner Länge; die Schranke erreicht die Annotation

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::a_sequence_is_a_list_and_an_array_is_a_tuple_of_its_length`,
`crates/lang-rules/src/adapter/python.rs::tests::a_bound_reaches_the_annotation_and_an_unbounded_one_leaves_it_alone`

**Status:** done

### §7.2.2.4.3 Eine `map` ist eine Liste von Paaren und nie ein `dict`

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::a_map_is_a_list_of_pairs_and_never_a_dict`,
`crates/lang-rules/tests/python_codec.rs::a_map_entry_is_written_into_as_a_list_and_answered_as_a_tuple`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member ist `Optional` mit Default `None` und wird per Identität geprüft

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::an_optional_member_is_optional_and_defaults_to_none`,
`crates/lang-rules/tests/python_codec.rs::an_optional_member_is_tested_against_none_by_identity`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird ein `IntEnum`; ein Member davon ist ein `int`

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::an_enumeration_is_an_intenum_and_a_member_of_it_is_an_int`,
`crates/lang-rules/src/adapter/python.rs::tests::a_bitmask_is_an_intflag_and_a_wide_shift_needs_no_suffix`

**Status:** done

### IDL 4.2 §7.4.1.4 Eine Konstante außerhalb ihres IDL-Bereichs wird abgelehnt, obwohl Python sie hielte

**Spec:** IDL 4.2 §7.4.1.4 — `const` — ein zur Übersetzungszeit ausgewerteter Wert seines deklarierten Typs.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::a_constant_outside_its_idl_range_is_refused_although_python_would_hold_it`,
`crates/lang-rules/src/adapter/python.rs::tests::a_constant_is_final_and_carries_its_idl_type`

**Status:** done

### IDL 4.2 §7.2.3 Ein Python-Keyword und ein von den Annotationen benutzter Builtin werden entschärft

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::an_identifier_that_is_a_python_keyword_is_escaped_and_the_idl_name_is_kept`,
`crates/lang-rules/src/adapter/python.rs::tests::a_member_named_after_a_builtin_the_annotations_use_is_escaped_too`,
`crates/lang-rules/src/adapter/python.rs::tests::two_names_that_would_escape_onto_one_are_refused`

**Status:** done

### IDL 4.2 §7.5.2 `module` wird eine geschachtelte Klasse; ein Verweis wird von der Wurzel geschrieben

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::a_module_is_a_nested_class_and_a_reference_is_written_from_the_root`,
`crates/lang-rules/src/adapter/python.rs::tests::a_module_reopened_beside_a_nested_one_is_one_class`

**Status:** done

### §7.4 Das erzeugte Modul benennt nichts, was die Laufzeit nicht hat

**Spec:** §7.4 — Data Representation — die erzeugte Quelle ruft eine Laufzeit, die §7.4 aus dem Klauseltext liest.

**Repo:** `runtime/python`

**Tests:** `crates/lang-rules/tests/python_names_its_runtime.rs::the_generated_module_names_nothing_the_runtime_does_not_have`,
`crates/lang-rules/tests/python_names_its_runtime.rs::every_policy_the_backend_writes_is_one_the_runtime_accepts`,
`crates/lang-rules/tests/python_names_its_runtime.rs::every_operation_the_walks_ask_for_is_one_the_runtime_answers_to`

**Status:** done

### §7.2.2.4.4.4.9 Table 9: jedes Pflichtfeld wird benannt, und der Closure erreicht jeden Defaulter, den ein Batch ruft

**Spec:** §7.2.2.4.4.4.9 — Ein Member, das der Strom nicht trug, nimmt seinen Default nach Tab.9.

**Repo:** `crates/lang-rules/src/default_python.rs`

**Tests:** `crates/lang-rules/src/default_python.rs::tests::a_structure_of_primitives_names_every_required_field`,
`crates/lang-rules/src/default_python.rs::tests::the_closure_reaches_every_defaulter_a_batch_calls`,
`crates/lang-rules/src/default_python.rs::tests::a_typedef_and_its_target_are_one_function_and_it_does_not_call_itself`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen

**Spec:** §7.4 — Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen. Ein stillschweigend ausgelassenes Konstrukt ist die schlimmste Fehlerklasse: der Empfänger sieht keinen Fehler, sondern falsche Daten.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::a_construct_with_no_python_data_form_is_named_and_not_left_out`

**Status:** done

### §7.4.3 Die Formen, die der Korpus hat, erreichen das Ende beider Walks

**Spec:** §7.4.3 — Die Formen, die der Korpus hat, erreichen das Ende beider Walks.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/tests/python_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

### §7.4 Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten

**Spec:** §7.4 — Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten. Gemessen gegen `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, nicht neben den anderen Spalten: drei Spalten, die sich gemeinsam bewegt haben, ähneln einander weiterhin.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/python.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang python`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-python-gen/`, `tools/wireprobe/wireprobe-zerodds-python-walk`, `tools/wireprobe/wireprobe-python-walk-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
