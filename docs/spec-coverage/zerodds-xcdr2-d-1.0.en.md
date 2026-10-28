# `zerodds-xcdr2-d` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the D column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the D column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/d/`, `crates/lang-render/src/d.rs`

**Tests:** `crates/lang-render/src/d.rs::tests` (20 tests), among them `a_file_declares_its_module_first`, `an_import_is_selective_when_it_names_symbols`, `a_struct_writes_its_brace_on_a_line_of_its_own`, `a_field_carries_its_uda_before_the_type`, `an_array_writes_its_dimensions_right_to_left`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/d.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/d.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_d.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/d`, endpoint SDK `endpoints/d`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a D type and none is target-dependent

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::every_idl_primitive_reaches_a_d_type`,
`crates/lang-rules/tests/d_end_to_end.rs::no_generated_type_is_target_dependent_or_unimplemented`,
`crates/lang-rules/tests/d_end_to_end.rs::an_idl_long_is_a_d_int_and_an_idl_long_long_is_a_d_long`

**Status:** done

### §7.2.2.2.1.2 An IDL `char` becomes an integer and never a D `char`

**Spec:** §7.2.2.2.1.2 — Char8 is unspecified (UTF-8 by default), Char16 is UTF-16 over the BMP.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::an_idl_character_becomes_an_integer_and_never_a_d_char`

**Status:** done

### §7.2.2.2 `long double` becomes a string and never a `real`

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::a_long_double_is_a_string_and_never_a_real`

**Status:** done

### §7.2.2.4.3 An array reverses its dimensions because D reads them right to left

**Spec:** §7.2.2.4.3 — Array — multi-dimensional, fixed element count, row-major.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::an_array_reverses_its_dimensions_because_d_reads_them_right_to_left`,
`crates/lang-rules/tests/d_end_to_end.rs::a_sequence_of_arrays_and_an_array_of_sequences_stay_apart`

**Status:** done

### §7.2.2.4.3 A `map` becomes a slice of entries and never an associative array

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::a_map_becomes_a_slice_of_entries_and_never_an_associative_array`

**Status:** done

### §7.2.2.4.9 An `@external` member becomes a pointer whatever its type is

**Spec:** §7.2.2.4.9 — `@external` — the member is held by reference; self-reference is admitted.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::an_external_member_becomes_a_pointer_whatever_its_type_is`,
`crates/lang-rules/tests/d_end_to_end.rs::an_optional_external_member_is_one_pointer_and_not_a_nullable_pointer`,
`crates/lang-rules/tests/d_codec.rs::an_external_member_reads_through_zd_ptr_and_writes_the_dereference`

**Status:** done

### §7.2.2.4.9 No generated struct holds itself by value; a cycle with no terminator is named

**Spec:** §7.2.2.4.9 — A self-referential type needs a place where the descent terminates.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::no_generated_struct_holds_itself_by_value_anywhere_in_the_corpus`,
`crates/lang-rules/tests/d_codec.rs::a_cycle_with_no_terminator_is_refused_by_name`,
`crates/lang-rules/tests/d_codec.rs::the_recursive_external_shape_is_carried`

**Status:** done

### §7.2.2.4.4.3 A union becomes a struct with a discriminator and an anonymous union

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::a_union_becomes_a_struct_with_a_discriminator_and_an_anonymous_union`,
`crates/lang-rules/tests/d_codec.rs::a_union_assigns_through_the_anonymous_union`

**Status:** done

### IDL 4.2 §7.2.3 A D keyword is escaped and keeps its IDL name

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::a_member_named_after_a_d_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/d_end_to_end.rs::a_member_named_after_a_d_type_keyword_is_escaped_too`,
`crates/lang-rules/tests/d_end_to_end.rs::two_members_that_would_reach_one_d_name_are_refused_by_name`

**Status:** done

### §7.4.3 The read direction opens with Table 9 and returns the sample

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/d.rs`

**Tests:** `crates/lang-rules/tests/d_codec.rs::a_structure_of_primitives_becomes_a_d_encoder`,
`crates/lang-rules/tests/d_codec.rs::the_read_direction_opens_with_table_9_and_returns_the_sample`,
`crates/lang-rules/tests/d_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::the_generated_d_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/d.rs`, `crates/lang-rules/src/adapter/d.rs`

**Tests:** `crates/lang-rules/tests/d_end_to_end.rs::every_construct_the_catalogue_covers_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/d.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang d`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-d-gen/`, `tools/wireprobe/wireprobe-zerodds-d`, `tools/wireprobe/wireprobe-d-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

23 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
