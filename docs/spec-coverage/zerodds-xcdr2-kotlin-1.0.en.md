# `zerodds-xcdr2-kotlin` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Kotlin column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Kotlin column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/kotlin/`, `crates/lang-render/src/kotlin.rs`

**Tests:** `crates/lang-render/src/kotlin.rs::tests` (7 tests), among them `a_data_class_indents_by_four_and_ends_every_property_but_the_last_in_a_comma`, `an_int_array_is_the_specialised_form_and_a_named_type_is_boxed`, `the_array_literal_factory_matches_the_array_type`, `a_constructor_call_writes_no_new`, `a_nul_is_written_as_a_unicode_escape_because_kotlin_has_no_backslash_zero`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/kotlin.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/kotlin.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_kotlin.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/kotlin`, endpoint SDK `endpoints/kotlin`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Kotlin type

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::every_idl_primitive_reaches_a_kotlin_type`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::a_struct_of_primitives_becomes_a_kotlin_data_class`

**Status:** done

### §7.2.2.2 `long double` becomes a `Float128` because Kotlin has no binary128

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::a_long_double_is_a_float128_because_kotlin_has_no_binary128`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::a_fixed_holds_its_decimal_as_a_string`

**Status:** done

### §7.2.2.4.4.4.7 An optional member becomes a nullable type and defaults to null

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::an_optional_member_becomes_a_nullable_type_and_defaults_to_null`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::an_external_member_needs_no_indirection`

**Status:** done

### §7.2.2.4.3 A `map` becomes an array of a generated entry class

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::a_map_becomes_an_array_of_a_generated_entry_class`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::a_sequence_of_an_aggregate_is_a_boxed_array`

**Status:** done

### §7.2.2.4.1.1 An enumeration becomes an object of constants, because an `enum class` holds no gapped value

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::an_enum_becomes_an_object_of_constants_with_every_value_written_out`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::an_enum_typed_member_is_a_plain_int`

**Status:** done

### §7.2.2.4.1.2 A bitmask gets no declaration of its own; each bit becomes a `const val`

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::a_bitmask_becomes_no_declaration_of_its_own_and_one_const_val_per_bit`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::a_wide_bitmask_packs_its_bits_in_a_long`

**Status:** done

### IDL 4.2 §7.2.3 A hard keyword is escaped, a soft one keeps the IDL spelling

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::a_member_named_after_a_hard_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::a_member_named_after_a_soft_keyword_keeps_the_idl_spelling`,
`crates/lang-rules/tests/kotlin_end_to_end.rs::two_members_that_would_reach_one_kotlin_name_are_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the name because a Kotlin file has one package

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::a_module_flattens_onto_the_name_because_a_kotlin_file_has_one_package`

**Status:** done

### §7.4.3 Kotlin widens no number implicitly; every conversion is written out and `already` recognises the static shape

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::the_generated_kotlin_compiles`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/kotlin.rs`, `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/kotlin.rs`, `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::the_generated_kotlin_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/kotlin.rs`, `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/kotlin.rs`, `crates/lang-rules/src/adapter/kotlin.rs`

**Tests:** `crates/lang-rules/tests/kotlin_end_to_end.rs::every_construct_the_catalogue_covers_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/kotlin.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang kotlin`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-kotlin-gen/`, `tools/wireprobe/wireprobe-zerodds-kotlin`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

22 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
