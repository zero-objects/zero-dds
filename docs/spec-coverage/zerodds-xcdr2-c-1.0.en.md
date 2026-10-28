# `zerodds-xcdr2-c` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the C column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the C column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/c/`, `crates/lang-render/src/c.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_c_else_holding_one_if_renders_as_else_if`, `a_c_else_holding_anything_else_keeps_its_block`, `a_c_struct_renders_with_its_members`, `a_c_prototype_with_no_parameters_writes_void`, `a_c_pointer_type_binds_to_the_declarator`, `a_c_file_writes_its_include_guard_at_both_ends`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/c.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/c.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_c.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/c`, endpoint SDK `endpoints/c`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a fixed-width C type

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::every_idl_primitive_reaches_a_c_type`,
`crates/lang-rules/tests/c_end_to_end.rs::a_struct_of_primitives_becomes_a_c_struct`

**Status:** done

### §7.2.2.2 `long double` is one C type in the declaration and in both codecs

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_long_double_is_one_c_type_in_the_declaration_and_in_both_codecs`

**Status:** done

### §7.2.2.4.3 A sequence becomes a length and a buffer

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_sequence_becomes_a_length_and_a_buffer`,
`crates/lang-rules/tests/c_end_to_end.rs::two_collections_of_the_same_shape_are_one_declaration`

**Status:** done

### §7.2.2.4.4.4.7 An optional member is a pointer, and NULL is the absent one


**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::an_optional_member_is_a_pointer_and_null_is_the_absent_one`,
`crates/lang-rules/tests/c_end_to_end.rs::an_optional_member_is_written_only_when_it_is_there`

**Status:** done

### §7.2.2.4.1.1 An enumeration becomes an integer with constants and not a C `enum`

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::an_enumeration_becomes_an_integer_and_constants_not_a_c_enum`,
`crates/lang-rules/tests/c_end_to_end.rs::an_unknown_enumerator_is_refused_rather_than_handed_back_as_a_number`

**Status:** done

### IDL 4.2 §7.4.13 A `bitset` becomes separate integers and not C bit-fields

**Spec:** IDL 4.2 §7.4.13 — `bitset` with `bitfield` members of declared width.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_bitset_becomes_separate_integers_not_c_bitfields`,
`crates/lang-rules/tests/c_end_to_end.rs::a_bitset_that_inherits_packs_the_base_at_the_low_positions`

**Status:** done

### §7.2.2.4.4.3 A union becomes a discriminator beside a C `union`

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_union_becomes_a_discriminator_beside_a_union`,
`crates/lang-rules/tests/c_end_to_end.rs::a_union_selects_its_branch_by_the_discriminator`

**Status:** done

### IDL 4.2 §7.5.2 `module` becomes a prefix because C has no namespaces

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_module_becomes_a_prefix_because_c_has_no_namespaces`

**Status:** done

### §7.2.2.4.4.4.9 Every Table 9 row has a C default, and every defaulter called is defined

**Spec:** §7.2.2.4.4.4.9 — A member the stream did not carry takes its Table 9 default.

**Repo:** `crates/lang-rules/src/default_c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::every_table_nine_row_has_a_c_default`,
`crates/lang-rules/tests/c_end_to_end.rs::every_defaulter_the_decoder_calls_is_also_defined`,
`crates/lang-rules/tests/c_end_to_end.rs::an_explicit_default_beats_table_nine`

**Status:** done

### §7.4.3 Encoder and decoder do not share a name, nor do the data and key encoders

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::the_encoder_and_the_decoder_do_not_share_a_name`,
`crates/lang-rules/tests/c_end_to_end.rs::the_data_and_key_encoders_do_not_share_a_name`,
`crates/lang-rules/tests/c_end_to_end.rs::a_decoder_reads_a_primitive_through_the_runtime_and_not_by_hand`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_member_of_type_any_is_refused_by_name_rather_than_emitted`

**Status:** done

### §7.5.2 A type is declared before it is used

**Spec:** §7.5.2 — A type is declared before it is used. The order of the output is a property of the target language and not of the source.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_type_is_declared_before_it_is_used`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.4 The two C generators read the corpus identically

**Spec:** §7.4 — The two C generators read the corpus identically.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/wire-probe/tests/probe_agreement.rs::the_two_c_generators_read_the_corpus_identically`

**Status:** done

### §7.4 The column's octets are held against a fixed recording

**Spec:** §7.4 — The column's octets are held against a fixed recording. Measured against `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, not beside the other columns: three columns that moved together still resemble each other.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/c.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::c_places_the_file_slots_and_the_declaration_text_inside_the_struct`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang c`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-c-gen/`, `tools/wireprobe/wireprobe-zerodds-c`, `tools/wireprobe/wireprobe-zerodds-c-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
