# `zerodds-xcdr2-go` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Go column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Go column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/go/`, `crates/lang-render/src/go.rs`

**Tests:** `crates/lang-cst/src/go/mod.rs::tests` (9 tests):
`every_primitive_writes_its_own_spelling`,
`a_keyword_escapes_with_a_trailing_underscore`,
`a_predeclared_name_escapes_although_it_would_parse`,
`exporting_moves_the_first_letter_and_nothing_else`,
`an_array_type_nests_outermost_dimension_first`,
`a_field_carries_its_idl_spelling_in_a_tag`;
`crates/lang-render/tests/dispatch_chains.rs::a_go_dispatch_chain_of_ten_thousand_arms_renders_flat`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/go.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/go.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_go.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/go`, endpoint SDK `endpoints/go`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Go type and none has an implementation-defined width

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::every_idl_primitive_reaches_a_go_type`,
`crates/lang-rules/tests/go_end_to_end.rs::no_generated_type_has_an_implementation_defined_width`

**Status:** done

### §7.2.2.2 `long double` becomes a string and never a `float64`

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::a_long_double_is_a_string_and_never_a_float64`

**Status:** done

### §7.2.2.4.4.4.7 An optional member becomes a pointer and the others do not

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::an_optional_member_becomes_a_pointer_and_the_others_do_not`,
`crates/lang-rules/tests/go_codec.rs::an_optional_member_reads_through_a_pointer`

**Status:** done

### §7.2.2.4.3 A `map` becomes a slice of entries and never a Go map

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::a_map_becomes_a_slice_of_entries_and_never_a_go_map`

**Status:** done

### §7.2.2.4.2 A `typedef` becomes an alias and not a defined type

**Spec:** §7.2.2.4.2 — Alias — `typedef` onto `base_type`, with no kind of its own when types are compared.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::a_typedef_becomes_an_alias_and_not_a_defined_type`

**Status:** done

### §7.2.2.4.1.1 An enum becomes a defined type with its numbering written out

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::an_enum_becomes_a_defined_type_with_its_numbering_written_out`

**Status:** done

### §7.2.2.4.1.2 A bitmask becomes an unsigned type with one constant per bit; the complement is Go's own

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::a_bitmask_becomes_an_unsigned_type_and_one_constant_per_bit`,
`crates/lang-rules/tests/go_codec.rs::a_bitmask_mask_is_gos_own_and_typed`

**Status:** done — the reader keeps the declared bits with the mask of the declared bits
(a typed `uint64` value, for example `uint64(9)`); no complement is written.

### IDL 4.2 §7.2.3 A Go keyword and a predeclared identifier are both escaped

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::a_member_named_after_a_go_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/go_end_to_end.rs::a_member_named_after_a_predeclared_identifier_is_escaped_too`,
`crates/lang-rules/tests/go_end_to_end.rs::two_members_that_would_reach_one_go_name_are_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the name because a Go package is a directory

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::a_module_flattens_onto_the_name_because_a_go_package_is_a_directory`

**Status:** done

### §7.4.3 The read direction opens with Table 9; a mutable aggregate compiles with its seen flags

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/go.rs`

**Tests:** `crates/lang-rules/tests/go_codec.rs::the_read_direction_opens_with_table_9_and_returns_the_sample`,
`crates/lang-rules/tests/go_codec.rs::a_mutable_aggregate_compiles_with_its_seen_flags`,
`crates/lang-rules/tests/go_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/go.rs`, `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/go.rs`, `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::the_generated_go_compiles`

**Status:** done

### §7.3.1 The generated source is already in the language's canonical form

**Spec:** §7.3.1 — The generated source is already in the language's canonical form. A formatter that still changes the output is a difference a user sees in their own diff.

**Repo:** `crates/lang-rules/src/emit/go.rs`, `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::the_generated_go_is_gofmt_stable`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/go.rs`, `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/go.rs`, `crates/lang-rules/src/adapter/go.rs`

**Tests:** `crates/lang-rules/tests/go_end_to_end.rs::every_construct_the_table_claims_for_go_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/go.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang go`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-go-gen/`, `tools/wireprobe/wireprobe-zerodds-go`, `tools/wireprobe/wireprobe-go-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
