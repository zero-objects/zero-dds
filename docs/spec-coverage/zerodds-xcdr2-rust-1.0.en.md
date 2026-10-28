# `zerodds-xcdr2-rust` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Rust column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Rust column decides for itself, and its own evidence.

The Rust column is the oldest and the reference every other one is measured against. Two things are its own: its materialiser is `crates/lang-rules/src/adapter/cst.rs` — it was the first and the file was named for the job rather than for the language — and its codec is built by `crates/lang-rules/src/encode.rs` directly against the Rust CST rather than through `emit::CodecTarget`. Its wire runtime is the `crates/wire` crate and not a directory under `runtime/`.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/rust/`, `crates/lang-render/src/rust.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_rust_struct_renders_with_its_derives_on_one_line`, `a_rust_enum_renders_its_variants_and_discriminants`, `a_rust_impl_renders_its_method_body`, `a_rust_generic_type_renders_its_arguments`, `a_rust_one_element_tuple_keeps_its_comma`, `a_rust_array_length_renders_as_the_expression_it_is`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/rust.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_rust.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `crates/wire`, endpoint SDK `endpoints/rust`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Rust type

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::every_idl_primitive_reaches_a_rust_type`,
`crates/lang-rules/tests/end_to_end.rs::a_struct_of_primitives_becomes_a_rust_struct`

**Status:** done

### §7.2.2.4.4.4.7 An optional member becomes an `Option` and the others do not

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::an_optional_member_becomes_an_option_and_the_others_do_not`

**Status:** done

### §7.2.2.4.3 A sequence becomes a `Vec` and a `string` a `String`; an array nests outermost dimension first

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_sequence_becomes_a_vec_and_a_string_a_string`,
`crates/lang-rules/tests/end_to_end.rs::an_array_nests_outermost_dimension_first`

**Status:** done

### §7.2.2.4.2 A `typedef` becomes a type alias as well as resolving in a member

**Spec:** §7.2.2.4.2 — Alias — `typedef` onto `base_type`, with no kind of its own when types are compared.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_typedef_becomes_a_type_alias_as_well_as_resolving_in_a_member`,
`crates/lang-rules/tests/end_to_end.rs::a_typedef_resolves_to_what_it_aliases`

**Status:** done

### §7.2.2.4.1.1 An enum becomes a Rust enum with its numbering written out

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::an_enum_becomes_a_rust_enum_with_its_numbering_written_out`

**Status:** done

### §7.2.2.4.4.3 A union becomes an enum with one variant per case

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, the shape in
`crates/lang-rules/src/rust_union.rs`: a case with one label holds the
member, a case with several labels and the `default:` case hold the
discriminator ahead of it, and `_NoMember(<discriminator>)` holds a
discriminator that selects no member (IDL 4.2 §7.4.1.4.4.4). The codec
writes a carried discriminator after checking it against the case's
labels, and a `boolean` or `char` discriminator as `true`/`false` and a
character.

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_union_becomes_an_enum_with_one_variant_per_case`,
`crates/lang-rules/tests/rust_corpus_compiles.rs::a_union_keeps_its_discriminator_through_rustc_and_back`

**Status:** done

### §7.2.2.4.1.2 A bitmask becomes an integer with one constant per bit; a bitset a struct of holder-typed fields

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_bitmask_becomes_an_integer_and_one_constant_per_bit`,
`crates/lang-rules/tests/end_to_end.rs::a_bitset_becomes_a_struct_of_holder_typed_fields`

**Status:** done

### IDL 4.2 §7.2.3 A member named after a Rust keyword is escaped

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_member_named_after_a_rust_keyword_is_escaped`

**Status:** done

### IDL 4.2 §7.4.3 `interface` becomes a trait of signatures; `native` an opaque alias

**Spec:** IDL 4.2 §7.4.3 — `interface` — operations with `in`/`out`/`inout` parameters.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::an_interface_becomes_a_trait_of_signatures`,
`crates/lang-rules/tests/end_to_end.rs::a_native_declaration_becomes_an_opaque_alias`

**Status:** done

### §7.4 The generated call sequence writes the bytes the plan names

**Spec:** §7.4 — Data representation — the generated source calls a runtime that reads §7.4 from the clause text.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::every_call_the_generator_emits_exists_in_the_runtime`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_mutable_struct_generates_carries_its_member_ids`,
`crates/lang-rules/tests/runtime_agreement.rs::the_key_hash_sequence_produces_a_stable_sixteen_bytes`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 A construct with no data form in this language is named and not left out

**Spec:** §7.4 — A construct with no data form in this language is named and not left out. A silently dropped construct is the worst class of fault: the receiver sees no error, it sees wrong data.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::a_construct_with_no_rule_produces_nothing_rather_than_something_wrong`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::every_construct_the_table_claims_for_rust_produces_output`

**Status:** done

### §7.4 The column's octets are held against a fixed recording

**Spec:** §7.4 — The column's octets are held against a fixed recording. Measured against `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, not beside the other columns: three columns that moved together still resemble each other.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/wire-probe/tests/three_column_gate.rs::the_rust_column_is_byte_for_byte_what_it_was`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang rust`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-gen/`, `crates/wire-probe/src/main.rs`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
