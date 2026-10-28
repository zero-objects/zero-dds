# `zerodds-xcdr2-lua` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Lua column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Lua column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/lua/`, `crates/lang-render/src/lua.rs`

**Tests:** `crates/lang-render/src/lua.rs::tests` (30 tests), among them `a_file_ends_with_the_return_its_module_needs`, `a_note_takes_two_dashes_and_an_annotation_three`, `a_luacats_tag_is_written_against_the_dashes_and_prose_is_not`, `a_commented_statement_starts_a_block_and_an_uncommented_one_does_not`, `an_empty_comment_line_carries_no_trailing_space`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/lua.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/lua.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_lua.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/lua`, endpoint SDK `endpoints/lua`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Lua holder and says which

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::every_idl_primitive_reaches_a_lua_holder_and_says_which`,
`crates/lang-rules/tests/lua_end_to_end.rs::the_one_integer_that_does_not_fit_says_so`,
`crates/lang-rules/tests/lua_end_to_end.rs::every_member_carries_the_facts_lua_cannot_state`

**Status:** done

### §7.2.2.2 `long double` becomes a string and never a Lua float

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_long_double_is_a_string_and_never_a_lua_float`,
`crates/lang-rules/tests/lua_end_to_end.rs::a_double_default_is_written_so_that_math_type_answers_float`

**Status:** done

### §7.2.2.4.3 A sequence says that it is one-based; an array nests outermost dimension first

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_sequence_says_that_it_is_one_based`,
`crates/lang-rules/tests/lua_end_to_end.rs::an_array_nests_outermost_dimension_first_and_is_filled`,
`crates/lang-rules/tests/lua_codec.rs::a_sequence_counts_zero_based_and_subscripts_one_based`

**Status:** done

### §7.2.2.4.3 A `map` becomes an array of pairs and never a Lua table keyed by K

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_map_becomes_an_array_of_pairs_and_never_a_lua_table_keyed_by_k`,
`crates/lang-rules/tests/lua_codec.rs::a_sorted_maps_char_key_wraps_and_its_u64_key_is_unsigned`

**Status:** done

### §7.2.2.4.4.4.7 An optional member gets no key at all

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::an_optional_member_gets_no_key_at_all`,
`crates/lang-rules/tests/lua_codec.rs::an_optional_member_is_the_key_being_there`

**Status:** done

### §7.2.2.4.1.1 An enum writes both directions and its values; its default is its first literal

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::an_enum_writes_both_directions_and_its_values`,
`crates/lang-rules/tests/lua_end_to_end.rs::an_enum_defaults_to_its_first_literal_and_not_to_zero`,
`crates/lang-rules/tests/lua_codec.rs::an_enumerations_wire_value_is_read_out_of_the_descriptor`

**Status:** done

### IDL 4.2 §7.2.3 A Lua keyword keeps its name and is reached in the bracket form

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_member_named_after_a_lua_keyword_keeps_its_name`,
`crates/lang-rules/tests/lua_codec.rs::a_keyword_member_keeps_its_name_in_the_bracket_form`,
`crates/lang-rules/tests/lua_end_to_end.rs::the_executed_module_reaches_a_member_named_after_a_lua_keyword`

**Status:** done

### IDL 4.2 §7.5.2 `module` nests rather than flattening onto the name; the chunk returns the module table

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::a_module_nests_rather_than_flattening_onto_the_name`,
`crates/lang-rules/tests/lua_end_to_end.rs::the_chunk_returns_the_module_table`,
`crates/lang-rules/tests/lua_end_to_end.rs::a_named_reference_is_resolved_at_load_and_not_left_to_a_reader`

**Status:** done

### §7.4.3 The short-circuiting operators are words and inequality is a tilde

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_codec.rs::the_short_circuiting_operators_are_words_and_inequality_is_tilde`,
`crates/lang-rules/tests/lua_codec.rs::a_decoder_returns_the_sample_it_built`,
`crates/lang-rules/tests/lua_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::the_generated_lua_loads`

**Status:** done

### §7.3.1 The executed module reports what the source declared

**Spec:** §7.3.1 — The executed module reports what the source declared. A chunk that loads is not yet a chunk that answers.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::the_executed_module_reports_its_members_in_idl_declaration_order`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/lua.rs`, `crates/lang-rules/src/adapter/lua.rs`

**Tests:** `crates/lang-rules/tests/lua_end_to_end.rs::every_construct_the_column_claims_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/lua.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang lua`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-lua-gen/`, `tools/wireprobe/wireprobe-zerodds-lua`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

23 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
