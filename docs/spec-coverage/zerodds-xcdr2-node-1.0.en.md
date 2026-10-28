# `zerodds-xcdr2-node` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Node column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Node column decides for itself, and its own evidence.

Node is the one column with no `crates/lang-rules/src/rule/node.rs`: a struct, a union, a bitset, a valuetype, an exception and an interface are all object literals here and never a named declaration, so `crates/lang-rules/src/rule/mod.rs` answers `Language::Node` with an empty rule file rather than a file of rules with no content.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/node/`, `crates/lang-render/src/node.rs`

**Tests:** `crates/lang-render/src/node.rs::tests` (8 tests), among them `a_const_declaration_ends_in_a_semicolon`, `an_object_literal_writes_every_key_bare`, `a_for_loop_counts_from_zero_with_no_off_by_one_correction`, `a_function_declaration_has_no_type_annotation_anywhere`, `a_nul_is_written_as_the_named_escape_because_javascript_has_one`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/node.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_node.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/node`, endpoint SDK `endpoints/node`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.4.1.1 An enumeration is a frozen object of its names

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::an_enumeration_is_a_frozen_object_of_its_names`

**Status:** done

### §7.2.2.4.1.2 A bitmask declares one constant per bit and no type of its own; wide bits become BigInt

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_bitmask_declares_one_constant_per_bit_and_no_type_of_its_own`,
`crates/lang-rules/tests/node_end_to_end.rs::a_wide_bitmask_writes_its_bits_as_bigint_literals`

**Status:** done

### §7.5.1 A struct, a union and a bitset declare nothing: they are object literals everywhere

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_struct_a_union_and_a_bitset_declare_nothing`,
`crates/lang-rules/src/adapter/node.rs::tests::a_member_name_never_moves`,
`crates/lang-rules/src/adapter/node.rs::tests::a_map_entry_has_no_declared_name`

**Status:** done

### IDL 4.2 §7.4.1.4 A sixty-four-bit constant carries the BigInt suffix; a string literal stays one

**Spec:** IDL 4.2 §7.4.1.4 — `const` — a value of its declared type, evaluated at translation time.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_sixty_four_bit_constant_carries_the_bigint_suffix`,
`crates/lang-rules/tests/node_end_to_end.rs::a_string_constant_is_a_plain_string_literal`,
`crates/lang-rules/tests/node_end_to_end.rs::a_boolean_constant_writes_the_literal`

**Status:** done

### IDL 4.2 §7.2.3 A reserved word used as a top-level declaration is escaped; a plain name is never moved

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_reserved_word_used_as_a_top_level_declaration_is_escaped`,
`crates/lang-rules/tests/node_end_to_end.rs::no_predefined_type_name_is_reserved_here`,
`crates/lang-rules/tests/node_end_to_end.rs::escape_decl_never_moves_a_plain_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the enumeration's binding

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::a_module_flattens_onto_the_enumerations_binding`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/node.rs`, `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::every_corpus_case_reaches_the_materialiser_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/node.rs`, `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::the_generated_node_passes_node_check`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/node.rs`, `crates/lang-rules/src/adapter/node.rs`

**Tests:** `crates/lang-rules/tests/node_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/node.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang node`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-node-gen/`, `tools/wireprobe/wireprobe-zerodds-node`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

17 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.
