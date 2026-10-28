# `zerodds-xcdr2-ocaml` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the OCaml column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the OCaml column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/ocaml/`, `crates/lang-render/src/ocaml.rs`

**Tests:** `crates/lang-render/src/ocaml.rs::tests` (12 tests), among them `a_record_writes_one_field_per_line_with_a_trailing_semicolon`, `a_group_of_two_joins_them_with_and`, `a_postfix_type_needs_no_brackets_at_any_depth`, `a_tuple_is_always_bracketed_because_star_binds_loosest`, `a_variant_writes_a_leading_bar_per_constructor`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/ocaml.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_ocaml.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/ocaml`, endpoint SDK `endpoints/ocaml`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches an OCaml type; no width a member can reach is platform-dependent

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::every_idl_primitive_reaches_an_ocaml_type`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::no_generated_type_is_platform_dependent_in_a_width_a_member_can_reach`

**Status:** done

### §7.2.2.2 `long double` becomes a string and never a float

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_long_double_is_a_string_and_never_a_float`

**Status:** done

### §7.2.2.4.3 A sequence becomes an array and never a list; an array loses its bound to the comment

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_sequence_becomes_an_array_and_never_a_list`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::an_array_loses_its_bound_to_the_comment_because_ocaml_has_no_fixed_size_array`

**Status:** done

### §7.2.2.4.3 A `map` becomes an array of entry records and never a `Hashtbl`

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_map_becomes_an_array_of_entry_records_and_never_a_hashtbl`

**Status:** done

### §7.2.2.4.4.3 A union becomes a record and never a variant

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_union_becomes_a_record_and_never_a_variant`

**Status:** done

### §7.2.2.4.1.1 An enum becomes a variant and a pair of conversions

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::an_enum_becomes_a_variant_and_a_pair_of_conversions`,
`crates/lang-rules/tests/ocaml_codec.rs::an_enumeration_crosses_through_its_conversions`

**Status:** done

### IDL 4.2 §7.2.3 An OCaml keyword and a predefined type are escaped; the case fold adds no new collision

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_member_named_after_an_ocaml_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::a_member_named_after_a_predefined_type_is_escaped_too`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::the_case_fold_adds_no_collision_the_frontend_does_not_already_report`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the name because a recursive group cannot cross one

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_module_flattens_onto_the_name_because_a_recursive_group_cannot_cross_one`

**Status:** done

### §7.2.2.4.9 Two mutually recursive types come out as one `and`-joined group

**Spec:** §7.2.2.4.9 — A self-referential type needs a place where the descent terminates.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::a_self_recursive_type_is_one_declaration_that_names_itself`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::two_mutually_recursive_types_come_out_as_one_and_joined_group`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::a_recursive_group_compiles`

**Status:** done

### §7.4.3 A member store is a functional record update; a mutable aggregate compiles with its seen flags

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_codec.rs::a_member_store_is_a_functional_record_update`,
`crates/lang-rules/tests/ocaml_codec.rs::a_mutable_aggregate_compiles_with_its_seen_flags`,
`crates/lang-rules/tests/ocaml_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::the_generated_ocaml_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::every_construct_the_table_claims_for_ocaml_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang ocaml`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-ocaml-gen/`, `tools/wireprobe/wireprobe-zerodds-ocaml`, `tools/wireprobe/wireprobe-ocaml-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

23 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
