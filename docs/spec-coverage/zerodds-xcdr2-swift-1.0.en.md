# `zerodds-xcdr2-swift` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Swift column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Swift column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/swift/`, `crates/lang-render/src/swift.rs`

**Tests:** `crates/lang-render/src/swift.rs::tests` (14 tests), among them `a_struct_writes_its_properties_and_its_public_initialiser`, `an_empty_initialiser_stays_on_one_line`, `a_parameter_with_a_second_name_writes_both`, `a_class_is_written_final`, `an_option_set_writes_its_conformance_and_its_flags`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/swift.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/swift.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_swift.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/swift`, endpoint SDK `endpoints/swift`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Swift type and none has an implementation-defined width

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::every_idl_primitive_reaches_a_swift_type`,
`crates/lang-rules/tests/swift_end_to_end.rs::no_generated_type_has_an_implementation_defined_width`

**Status:** done

### §7.2.2.2 `long double` becomes a string and never a `Float80` or a `Double`

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::a_long_double_is_a_string_and_never_a_float80_or_a_double`

**Status:** done

### §7.5.1 A struct carries a written public memberwise initialiser, even with no members

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::a_struct_carries_a_written_public_memberwise_initialiser`,
`crates/lang-rules/tests/swift_end_to_end.rs::a_struct_with_no_members_still_gets_an_initialiser`

**Status:** done

### §7.2.2.4.9 An `@external` member is boxed in a class because a struct cannot recurse

**Spec:** §7.2.2.4.9 — `@external` — the member is held by reference; self-reference is admitted.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::an_external_member_is_boxed_in_a_class_because_a_struct_cannot_recurse`,
`crates/lang-rules/tests/swift_end_to_end.rs::the_external_box_is_absent_when_nothing_reaches_it`,
`crates/lang-rules/tests/swift_codec.rs::an_external_member_reads_through_the_box_and_writes_its_value`

**Status:** done

### §7.2.2.4.3 A `map` becomes an array of entries and never a `Dictionary`

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::a_map_becomes_an_array_of_entries_and_never_a_dictionary`,
`crates/lang-rules/tests/swift_end_to_end.rs::a_large_array_is_still_an_array_and_never_a_tuple`

**Status:** done

### §7.2.2.4.1.2 A bitmask becomes an option set

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::a_bitmask_becomes_an_option_set`,
`crates/lang-rules/tests/swift_end_to_end.rs::a_bitset_becomes_a_struct_of_holder_typed_properties`

**Status:** done

### IDL 4.2 §7.2.3 A Swift keyword takes back quotes; `self` gets a second parameter name for the body

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::a_member_named_after_a_swift_keyword_takes_back_quotes_and_keeps_its_name`,
`crates/lang-rules/tests/swift_end_to_end.rs::a_member_named_self_gets_a_second_parameter_name_for_the_body`,
`crates/lang-rules/tests/swift_end_to_end.rs::the_escape_cannot_make_two_idl_names_collide`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the name and keeps the IDL spelling of each segment

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::a_module_flattens_onto_the_name_and_keeps_the_idl_spelling_of_each_segment`,
`crates/lang-rules/tests/swift_end_to_end.rs::two_modules_that_flatten_onto_one_name_are_refused_by_name`

**Status:** done

### §7.4.3 The read direction opens with Table 9; a union branch is a place bare and a value unwrapped

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_codec.rs::the_read_direction_opens_with_table_9_and_returns_the_sample`,
`crates/lang-rules/tests/swift_codec.rs::a_union_branch_is_a_place_bare_and_a_value_unwrapped`,
`crates/lang-rules/tests/swift_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/swift.rs`, `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/swift.rs`, `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::the_generated_swift_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/swift.rs`, `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/swift.rs`, `crates/lang-rules/src/adapter/swift.rs`

**Tests:** `crates/lang-rules/tests/swift_end_to_end.rs::every_construct_the_table_claims_for_swift_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/swift.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang swift`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-swift-gen/`, `tools/wireprobe/wireprobe-zerodds-swift`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

22 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
