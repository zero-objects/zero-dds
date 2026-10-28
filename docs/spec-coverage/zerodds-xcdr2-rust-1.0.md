# `zerodds-xcdr2-rust` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Rust-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Rust-Spalte selbst entscheidet, und ihre eigenen Nachweise.

Die Rust-Spalte ist die älteste und die Referenz, gegen die jede andere gemessen wird. Zwei Dinge sind an ihr eigen: ihr Materialisierer heißt `crates/lang-rules/src/adapter/cst.rs` — er war der erste und wurde nach der Aufgabe benannt, nicht nach der Sprache —, und ihr Codec wird von `crates/lang-rules/src/encode.rs` direkt gegen den Rust-CST gebaut statt über `emit::CodecTarget`. Ihre Wire-Laufzeit ist die Crate `crates/wire` und kein Verzeichnis unter `runtime/`.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/rust/`, `crates/lang-render/src/rust.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_rust_struct_renders_with_its_derives_on_one_line`, `a_rust_enum_renders_its_variants_and_discriminants`, `a_rust_impl_renders_its_method_body`, `a_rust_generic_type_renders_its_arguments`, `a_rust_one_element_tuple_keeps_its_comma`, `a_rust_array_length_renders_as_the_expression_it_is`;
querschnittlich `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/rust.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_rust.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `crates/wire`, Endpoint-SDK `endpoints/rust`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen Rust-Typ

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::every_idl_primitive_reaches_a_rust_type`,
`crates/lang-rules/tests/end_to_end.rs::a_struct_of_primitives_becomes_a_rust_struct`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member wird eine `Option`, die anderen nicht

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::an_optional_member_becomes_an_option_and_the_others_do_not`

**Status:** done

### §7.2.2.4.3 Eine Sequenz wird ein `Vec`, ein `string` ein `String`; ein Array verschachtelt äußerste Dimension zuerst

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_sequence_becomes_a_vec_and_a_string_a_string`,
`crates/lang-rules/tests/end_to_end.rs::an_array_nests_outermost_dimension_first`

**Status:** done

### §7.2.2.4.2 Ein `typedef` wird ein Type-Alias und löst zugleich im Member auf

**Spec:** §7.2.2.4.2 — Alias — `typedef` auf `base_type`, ohne eigenes Kind beim Typvergleich.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_typedef_becomes_a_type_alias_as_well_as_resolving_in_a_member`,
`crates/lang-rules/tests/end_to_end.rs::a_typedef_resolves_to_what_it_aliases`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird ein Rust-`enum` mit ausgeschriebener Numerierung

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::an_enum_becomes_a_rust_enum_with_its_numbering_written_out`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird ein `enum` mit einer Variante je Case

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, die Form in
`crates/lang-rules/src/rust_union.rs`: ein Case mit einem Label hält das
Member, ein Case mit mehreren Labels und der `default:`-Case halten den
Diskriminator davor, und `_NoMember(<Diskriminator>)` hält einen
Diskriminator, der kein Member wählt (IDL 4.2 §7.4.1.4.4.4). Der Codec
schreibt einen getragenen Diskriminator nach der Prüfung gegen die Labels
des Case, einen `boolean`- oder `char`-Diskriminator als `true`/`false`
und als Zeichen.

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_union_becomes_an_enum_with_one_variant_per_case`,
`crates/lang-rules/tests/rust_corpus_compiles.rs::a_union_keeps_its_discriminator_through_rustc_and_back`

**Status:** done

### §7.2.2.4.1.2 Eine Bitmask wird ein Integer mit einer Konstanten je Bit; ein Bitset ein Struct holdertypisierter Felder

**Spec:** §7.2.2.4.1.2 — Bitmask — Schranke 1..64, benannte Bits.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`,
`crates/lang-rules/tests/end_to_end.rs::a_bitset_becomes_a_struct_of_holder_typed_fields`

**Status:** done

### IDL 4.2 §7.2.3 Ein Member, das ein Rust-Keyword benennt, wird entschärft

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_member_named_after_a_rust_keyword_is_escaped`

**Status:** done

### IDL 4.2 §7.4.3 `interface` wird ein Trait von Signaturen; `native` ein opaker Alias

**Spec:** IDL 4.2 §7.4.3 — `interface` — Operationen mit `in`/`out`/`inout`-Parametern.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::an_interface_becomes_a_trait_of_signatures`,
`crates/lang-rules/tests/end_to_end.rs::a_native_declaration_becomes_an_opaque_alias`

**Status:** done

### §7.4 Die erzeugte Aufruffolge schreibt die Bytes, die der Plan nennt

**Spec:** §7.4 — Data Representation — die erzeugte Quelle ruft eine Laufzeit, die §7.4 aus dem Klauseltext liest.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::every_call_the_generator_emits_exists_in_the_runtime`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_mutable_struct_generates_carries_its_member_ids`,
`crates/lang-rules/tests/runtime_agreement.rs::the_key_hash_sequence_produces_a_stable_sixteen_bytes`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen

**Spec:** §7.4 — Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen. Ein stillschweigend ausgelassenes Konstrukt ist die schlimmste Fehlerklasse: der Empfänger sieht keinen Fehler, sondern falsche Daten.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_construct_with_no_rule_produces_nothing_rather_than_something_wrong`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::every_construct_the_table_claims_for_rust_produces_output`

**Status:** done

### §7.4 Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten

**Spec:** §7.4 — Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten. Gemessen gegen `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, nicht neben den anderen Spalten: drei Spalten, die sich gemeinsam bewegt haben, ähneln einander weiterhin.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/wire-probe/tests/three_column_gate.rs::the_rust_column_is_byte_for_byte_what_it_was`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang rust`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-gen/`, `crates/wire-probe/src/main.rs`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit-Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed (20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored` verlangen).

Offene Punkte: keine. Decision-Records: keine.
