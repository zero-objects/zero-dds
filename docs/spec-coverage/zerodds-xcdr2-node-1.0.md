# `zerodds-xcdr2-node` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Node-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Node-Spalte selbst entscheidet, und ihre eigenen Nachweise.

Node ist die einzige Spalte ohne `crates/lang-rules/src/rule/node.rs`: Struct, Union, Bitset, Valuetype, Exception und Interface sind hier überall Objektliterale und nie eine benannte Deklaration, weshalb `crates/lang-rules/src/rule/mod.rs` für `Language::Node` eine leere Regeldatei liefert statt einer Datei aus Regeln ohne Inhalt.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/node/`, `crates/lang-render/src/node.rs`

**Tests:** `crates/lang-render/src/node.rs::tests` (8 Tests), darunter `a_const_declaration_ends_in_a_semicolon`, `an_object_literal_writes_every_key_bare`, `a_for_loop_counts_from_zero_with_no_off_by_one_correction`, `a_function_declaration_has_no_type_annotation_anywhere`, `a_nul_is_written_as_the_named_escape_because_javascript_has_one`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/node.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_node.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/node`, Endpoint-SDK `endpoints/node`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.4.1.1 Eine Enumeration wird ein eingefrorenes Objekt ihrer Namen

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::an_enumeration_is_a_frozen_object_of_its_names`

**Status:** done

### §7.2.2.4.1.2 Eine Bitmask deklariert eine Konstante je Bit und keinen eigenen Typ; breite Bits werden BigInt

**Spec:** §7.2.2.4.1.2 — Bitmask — Schranke 1..64, benannte Bits.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_bitmask_declares_one_constant_per_bit_and_no_type_of_its_own`,
`crates/lang-rules/tests/node_end_to_end.rs::a_wide_bitmask_writes_its_bits_as_bigint_literals`

**Status:** done

### §7.5.1 Struct, Union und Bitset deklarieren nichts: sie sind überall Objektliterale

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_struct_a_union_and_a_bitset_declare_nothing`,
`crates/lang-rules/src/adapter/node.rs::tests::a_member_name_never_moves`,
`crates/lang-rules/src/adapter/node.rs::tests::a_map_entry_has_no_declared_name`

**Status:** done

### IDL 4.2 §7.4.1.4 Eine 64-Bit-Konstante trägt das BigInt-Suffix; ein String-Literal bleibt eines

**Spec:** IDL 4.2 §7.4.1.4 — `const` — ein zur Übersetzungszeit ausgewerteter Wert seines deklarierten Typs.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_sixty_four_bit_constant_carries_the_bigint_suffix`,
`crates/lang-rules/tests/node_end_to_end.rs::a_string_constant_is_a_plain_string_literal`,
`crates/lang-rules/tests/node_end_to_end.rs::a_boolean_constant_writes_the_literal`

**Status:** done

### IDL 4.2 §7.2.3 Ein reserviertes Wort in Top-Level-Position wird entschärft; ein einfacher Name wird nie verschoben

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_reserved_word_used_as_a_top_level_declaration_is_escaped`,
`crates/lang-rules/tests/node_end_to_end.rs::no_predefined_type_name_is_reserved_here`,
`crates/lang-rules/tests/node_end_to_end.rs::escape_decl_never_moves_a_plain_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flacht auf die Bindung der Enumeration ab

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_module_flattens_onto_the_enumerations_binding`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/node.rs`, `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::every_corpus_case_reaches_the_materialiser_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/node.rs`, `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::the_generated_node_passes_node_check`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/node.rs`, `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/node.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang node`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-node-gen/`, `tools/wireprobe/wireprobe-zerodds-node`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

17 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
