# `zerodds-idl-frontend` v1.0 — coverage

**Source:** [`docs/specs/zerodds-idl-frontend-1.0.md`](../specs/zerodds-idl-frontend-1.0.md)

**Implementation:** `crates/idl-cst`, `crates/idl-parse`, `crates/idl-lower`

**Test run:** `cargo test -p zerodds-idl-cst -p zerodds-idl-parse -p zerodds-idl-lower` — 422 passed, 0 failed.

Each row names the code path and the test that exercises it. A count of accepted files is not coverage: a parser that accepts everything and understands nothing also parses the fixtures. Each row therefore names the construct it covers.

---

## §1.1 Trivia on the token

**Spec:** A token carries the trivia that precedes it; the file's trailing trivia hangs on the end-of-file token. No byte of the input is unreachable from the token stream.

**Repo:** `idl-cst/src/lex.rs`, `Token::leading`.

**Tests:** `every_byte_is_reachable_for_a_realistic_file`, `every_byte_is_reachable_for_a_broken_file`, `a_comment_before_a_token_is_that_tokens_leading_trivia`, `a_file_of_only_trivia_puts_all_of_it_on_the_eof_token`.

**Status:** done

## §1.2 What the lexer distinguishes

**Spec:** Identifiers and keywords; the escaped form `_name`, recorded as escaped; integers in decimal, octal and hexadecimal retaining their radix; floats and fixed-point in the form written (`1.` is not `1.0`); character, wide, string and wide-string literals with escapes resolved and the raw text retained; punctuation including `<<` and `>>`. The lexer does not decide whether `>>` is a shift or two brackets.

**Repo:** `idl-cst/src/token.rs` (85 keywords as a table), `idl-cst/src/lex.rs`, `literal_value`.

**Tests:** `every_keyword_lexes_as_its_own_kind_and_not_as_an_identifier`, `an_escaped_identifier_is_the_name_without_the_underscore`, `every_radix_reaches_the_same_value_and_keeps_how_it_was_written`, `float_forms_stay_distinguishable_from_one_another`, `a_d_suffix_makes_a_fixed_point_literal`, `wide_literals_are_their_own_kinds`, `a_literal_value_is_recovered_from_the_token_text_alone`, `text_that_is_not_one_whole_token_of_that_kind_has_no_value`.

**Status:** done

## §2.1 The green/red tree

**Spec:** An immutable green tree of kinds and widths, red nodes carrying an absolute offset and a parent link, built on demand. Every node knows its full width including trivia.

**Repo:** `idl-cst/src/tree.rs`.

**Tests:** `a_nodes_width_is_the_sum_of_its_children`, `a_child_knows_its_offset_and_its_parent`, `a_node_knows_its_ancestors_up_to_the_root`, `identical_subtrees_can_be_one_allocation`.

**Status:** done

## §2.2 Losslessness

**Spec:** `parse(source).text() == source` for every fixture, including files that do not parse.

**Repo:** `idl-cst/src/tree.rs` (`text()`, iterative), `idl-parse/src/parser.rs` (`bump()`, the only place trivia enters the tree).

**Tests:** `a_clean_file_round_trips_byte_for_byte`, `a_broken_file_round_trips_too`, `a_tree_over_a_file_that_would_not_parse_still_round_trips`, `every_fixture_round_trips_byte_for_byte` (21 fixtures), `a_rejected_extension_still_parses_and_round_trips`.

**Status:** done

## §2.3 The `>>` problem

**Spec:** In a type argument list `>>` splits into two `>`; in an expression it does not. The split is visible in the CST and the round trip still holds.

**Repo:** `idl-parse/src/type_spec.rs` (`close_type_arguments`, `split_shift_right`).

**Tests:** `a_nested_sequence_splits_the_shift_token_and_still_round_trips`, `shift_right_in_an_expression_is_not_split`, `three_deep_nesting_splits_as_far_as_it_needs_to`.

**Status:** done

## §3.1 Grammar coverage

**Spec:** The whole of OMG IDL 4.2 §7.4: the core building block, anonymous types, annotations, extended data types, and the building blocks for interfaces, values, CCM components, templates and `import`.

**Repo:** `idl-parse/src/decl.rs`, `idl-parse/src/type_spec.rs`.

**Tests:** `every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares` (21 fixtures, among them `omg/dds_xtypes.idl`, `spec_features/ccm_components.idl`, `spec_features/template_modules.idl`, `spec_features/corba_value_repository.idl`, `spec_features/attr_raises.idl`, `spec_features/user_annotations.idl`); `each_component_form_reaches_its_own_interface_kind`, `each_port_form_reaches_its_own_component_role`, `each_template_parameter_kind_is_distinguished`, `a_valuetype_keeps_its_modifier_bases_and_supported_interfaces`.

**Status:** done

## §3.2 Recovery

**Spec:** An unexpected token yields a diagnostic, an error node covering the range, and resynchronisation at the next `;`, `}` or top-level keyword. The parser does not stop. A file yields every diagnostic it has.

**Repo:** `idl-parse/src/parser.rs` (`error_token`, `recover_to_declaration`, `MAX_STEPS`).

**Tests:** `three_errors_in_one_file_produce_three_diagnostics`, `a_missing_semicolon_recovers_at_the_next_declaration`, `a_file_of_pure_garbage_produces_diagnostics_and_a_tree_not_a_panic`.

**Status:** done

## §3.3 Ambiguities

**Spec:** `>>` as a shift against two brackets; an annotation against a `#pragma` saying the same thing, both reaching the same facet; `::A` as the global scope, never an operator; an array declarator against a simple name, decided at the `[`.

**Repo:** `idl-parse/src/type_spec.rs`, `idl-parse/src/decl.rs` (`parse_declarator_list`, `operation_ahead`), `idl-lower/src/lower.rs` (`pragma`).

**Tests:** `shift_right_in_an_expression_is_not_split`, `a_pragma_id_sets_the_repository_id_like_typeid_does`, `a_cyclone_pragma_keylist_reaches_the_same_field_as_rtis_keylist`, `a_leading_double_colon_is_the_global_scope`, `an_array_declarator_is_only_known_at_the_bracket`.

**Status:** done

## §3.4 Preprocessing

**Spec:** `#include`, `#define`, `#undef`, the `#if` family and the `#pragma` forms. Expansion produces a source map: every token resolves to a file, a range **and** the macro chain that produced it. An `#include` cycle is reported, not followed.

**Repo:** `idl-parse/src/preprocess/mod.rs`, `idl-parse/src/preprocess/macro_.rs`, `parse_preprocessed`.

**Tests:** `an_included_file_reaches_the_tree`, `a_macro_is_expanded_before_the_parser_sees_it`, `a_conditional_branch_that_is_off_does_not_reach_the_tree`, `an_include_cycle_is_diagnosed_and_not_followed`, `the_map_covers_every_token_of_the_stream`, `tokens_from_an_included_file_name_that_file_not_the_includer`, `an_expanded_token_names_both_the_text_and_the_macro_that_produced_it`, `a_fixture_with_an_include_gains_the_declarations_it_includes`, `the_expanded_text_is_itself_lexable`, `crates/idl-parse/tests/source_map.rs::a_token_after_comments_maps_to_its_own_bytes`, `crates/idl-parse/tests/source_map.rs::a_token_from_an_include_names_that_file`.

**Status:** done

## §3.5 Dialects

**Spec:** `Dialect` is a set of flags, defaulting to strict IDL 4.2. A vendor flag admits that vendor's extensions and routes them to its facet. A construct admitted only by a flag records which flag admitted it.

**Repo:** `idl-parse/src/dialect.rs`, `idl-parse/src/decl.rs` (`parse_keylist`), `idl-lower/src/lower.rs` (`keylist` → `RtiFacet::keylist`).

**Tests:** `a_vendor_extension_is_rejected_under_the_strict_dialect`, `the_same_file_is_accepted_under_the_vendors_dialect_and_still_says_so`, `a_rejected_extension_still_parses_and_round_trips`, `an_rti_keylist_reaches_the_vendor_facet_and_the_members`.

**Status:** done

## §3.6 Suffix annotations — DDS-XTypes 1.3 §7.3.1.2.5

**Spec:** DDS-XTypes 1.3 §7.3.1.2.5, Table 24, p. 91 — `{ "//@"<annotation_type_name> [ "(" <arguments> ")" ] }*`, explained as: "Alternately and equivalently, apply an annotation to a type or type member by suffixing it with an annotation type name using slash-slash-at (“//@”) instead of the at sign by itself." §7.3.1.2.3, p. 89 fixes the recognition — "there must be no space in between the double slash (“//”) and the at-sign (‘@’)" — and shows the examples `long my_integer; //@my_member_annotation("Hello")`, `}; //@my_type_annotation`, and two annotations on one member with the second on the line below.

**Repo:** `crates/idl-parse/src/suffix.rs` (`parse_suffix_annotations`, `suffix_segments`, `parse_suffix_body`, `diagnose_stray_suffix`), `crates/idl-cst/src/token.rs` (`TokenKind::SlashSlashAt`), `crates/idl-parse/src/decl.rs` (`parse_annotation_tail`, the body both forms share), `crates/idl-parse/src/parser.rs` (`emit_leading_trivia`), `crates/idl-parse/src/preprocess/mod.rs` (`flush_trailing_pragmas`, the trivia loop in `read_file`), `crates/idl-lower/src/annotation.rs` (`suffix_annotations_of`), `crates/idl-lower/src/lower.rs` (`definition`).

**Tests:** `crates/idl-lower/tests/annotations.rs::the_suffix_form_marks_the_key_the_interop_suite_declares`, `crates/idl-lower/tests/annotations.rs::the_prefix_and_the_suffix_form_produce_the_same_model`, `crates/idl-lower/tests/annotations.rs::a_suffix_annotation_carries_its_arguments`, `crates/idl-lower/tests/annotations.rs::the_suffix_form_applies_to_the_type_whose_brace_precedes_it`, `crates/idl-lower/tests/annotations.rs::several_suffix_annotations_apply_to_the_same_element`, `crates/idl-lower/tests/annotations.rs::two_suffix_annotations_on_one_line_both_apply`, `crates/idl-lower/tests/annotations.rs::a_suffix_annotation_that_suffixes_nothing_is_refused`, `crates/idl-lower/tests/annotations.rs::an_unknown_suffix_annotation_gets_the_prefix_forms_answer`, `crates/idl-lower/tests/annotations.rs::text_after_a_suffix_annotation_is_reported`, `crates/idl-lower/tests/annotations.rs::only_the_exact_marker_opens_a_suffix_annotation`, `crates/idl-lower/tests/annotations.rs::a_file_with_suffix_annotations_round_trips`, `crates/idl-lower/tests/annotations.rs::the_suffix_form_reaches_literals_and_union_cases`, `crates/idl-parse/tests/preprocess.rs::a_suffix_annotation_survives_preprocessing`, `crates/idl-parse/tests/preprocess.rs::a_suffix_annotation_at_the_end_of_the_unit_survives_preprocessing`, `crates/idl-parse/tests/preprocess.rs::an_ordinary_comment_and_a_cut_branch_carry_nothing_through`, `crates/idl-parse/tests/parse.rs::a_suffix_annotation_reaches_the_tree_as_an_annotation`, `crates/idl-parse/tests/parse.rs::a_file_with_suffix_annotations_round_trips_byte_for_byte`, `crates/idl-parse/tests/parse.rs::a_suffix_annotation_in_a_position_that_admits_none_is_reported`, `crates/idl-parse/tests/parse.rs::only_the_exact_slash_slash_at_marker_is_the_suffix_form`.

The form is read where the declaration carrying it has just been parsed: after the `;` of a member, a union case member, a bitfield, an annotation member, an operation, an attribute, a port and every type declaration (`struct`, `union`, `enum`, `bitmask`, `bitset`, `typedef`, `const`, `native`, `exception`, `interface`, `valuetype`, `module`, `@annotation`, a template instantiation, `import`, `typeid`/`typeprefix`, `keylist`), and after the name of an enumerator or a bitmask value, on either side of the comma. Only white space may stand between the declaration and the annotation, and between two annotations; a comment in between ends the run. Every `//@` that is not read is reported by `emit_leading_trivia` as an error with a byte range, under `IDL0001` and with the spec reference `DDS-XTypes 1.3 §7.3.1.2.5`. An unknown name gets the prefix form's answer: `apply_user_annotation` puts it in the facet as an `AppliedAnnotation` with a `Ref::Pending`. The preprocessor emits no comments, so a `//@` did not reach the parser through `parse_preprocessed` at all — the path `zerodds-idlc` and every build script take through `zerodds-idl-compose`. It is now held like a `#pragma` and attached to the following token; one at the end of the translation unit gets an end-of-file token to lead, because the last token is the declaration itself and no annotation may stand there.

**Status:** done

## §4 Lowering to the common AST

**Spec:** One walk over the CST, built through the transactional edit surface. Module reopening converges through `Ast::ensure_module`. Every node created carries a `SourceFacet` naming the file, the byte range and the macro chain.

**Repo:** `idl-lower/src/lower.rs`, `idl-lower/src/scope.rs`, `idl-lower/src/expr.rs`, `idl-lower/src/annotation.rs`. Lowering reads the expanded text; `idl-compose/src/lib.rs` (`place_nodes_in_their_files`) then moves every node's `SourceFacet` to the file it was written in, the range in that file and the macro chain that wrote it, through `idl-parse/src/preprocess/mod.rs` (`Preprocessed::provenance`) — for a unit and for each unit of a bundle.

**Tests:** `a_module_reopened_across_two_files_converges_on_one_node`, `every_lowered_type_names_the_file_it_came_from`, `every_node_of_every_fixture_names_the_file_it_came_from`, `the_definition_completes_the_forward_node_rather_than_adding_a_second`, `a_reference_taken_against_a_forward_declaration_survives_the_definition`; `crates/idl-compose/tests/node_provenance.rs` (included file, macro expansion, bundle, the position of an unresolved name).

**Status:** done

## §5 Completion — member id

**Spec:** `@id(n)`, else `@hashid`, else the enclosing `@autoid`, else the previous member plus one starting at 0. Every value records its origin.

**Repo:** `idl-lower/src/complete.rs` (`member_ids`, `hash_id` with the MD5 XTypes 1.3 §7.3.1.2.1.1 names).

**Tests:** `members_without_annotations_number_from_zero_upwards`, `a_declared_id_shifts_the_numbering_of_what_follows`, `autoid_sequential_and_autoid_hash_differ`, `a_hashid_uses_the_name_it_names_rather_than_the_members_own`, `a_declared_id_wins_over_the_aggregates_autoid`, `a_hashed_id_stays_within_the_28_bit_member_id_space`, `the_digest_matches_the_published_vectors` (RFC 1321 appendix A.5).

**Status:** done

## §5 Completion — extensibility

**Spec:** The annotation, else `@extensibility(...)`, else the enclosing `#pragma`, else `@appendable`.

**Repo:** `ast/src/query.rs` (`effective_extensibility`), `ast/src/lib.rs` (`DEFAULT_EXTENSIBILITY`), `idl-lower/src/complete.rs`.

**Tests:** `a_type_that_said_nothing_is_appendable_by_rule_and_silent_in_the_facet`, `a_written_extensibility_is_what_the_rule_reports`, `each_extensibility_keyword_reaches_the_facet`.

**Status:** done

## §5 Completion — enumerator values and bit positions

**Spec:** `@value(n)` and `@position(k)` respectively, else the previous plus one starting at 0.

**Repo:** `ast/src/query.rs` (`effective_enumerator_value`, `effective_bit_position`).

**Tests:** `enumerators_without_values_run_from_zero_upwards`, `an_explicit_enumerator_value_shifts_the_ones_after_it`, `bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest`, `an_explicit_enumerator_value_keeps_the_expression_it_was_written_as`.

**Status:** done

## §5 Completion — bit bound

**Spec:** `@bit_bound(n)`, else 32 for enum and bitmask.

**Repo:** `ast/src/query.rs` (`effective_bit_bound`), `ast/src/lib.rs` (`DEFAULT_BIT_BOUND`).

**Tests:** `an_enum_without_a_bit_bound_is_thirty_two_bits`, `a_written_bit_bound_is_what_the_rule_reports`.

**Status:** done

## §5 Completion — constant type and array dimension

**Spec:** The declared type; the expression is checked against it, not used to derive it. The dimension is the evaluated constant expression.

**Repo:** `idl-lower/src/lower.rs` (`constant`, `array_dimensions`), `idl-lower/src/expr.rs`, `ast/src/model.rs` (`eval_integral`, `eval_const`). A floating-point expression is evaluated in `double` (IDL 4.2 §7.4.1.4.2); `long double` arithmetic is refused by name (`EvalError::LongDouble`), because the tree has no arithmetic of that width. A fixed-point expression is evaluated exactly after Table 7-11 and cut to 31 significant digits without rounding (`ast/src/decimal.rs`).

**Tests:** `a_constant_takes_its_declared_type_not_the_expressions`, `an_array_dimension_is_the_evaluated_constant_expression`, `a_multi_dimensional_array_keeps_its_dimensions_in_order`, `a_bounded_string_and_sequence_carry_their_bound_expressions`, `crates/idl-lower/tests/floating_constant_arithmetic.rs::the_arithmetic_is_double`, `crates/idl-lower/tests/floating_constant_arithmetic.rs::long_double_arithmetic_is_refused_and_a_long_double_literal_is_not`, `crates/idl-lower/tests/fixed_constant_arithmetic.rs::a_quotient_is_exact_or_cut_without_rounding`.

**Status:** done

## §5 Completion — repository id

**Spec:** `#pragma ID`, else derived from the scoped name with `#pragma prefix` and `#pragma version` applied.

**Repo:** `idl-lower/src/complete.rs` (`derive_repository_ids`, `derived_repository_id`), `idl-lower/src/lower.rs` (`repository_directive`, `pragma`).

**Tests:** `a_type_gets_a_repository_id_derived_from_its_scoped_name`, `a_typeid_directive_sets_the_id_outright`, `a_typeprefix_directive_prefixes_the_ids_derived_below_it`, `a_pragma_id_sets_the_repository_id_like_typeid_does`, `a_pragma_version_changes_the_version_a_derived_id_ends_in`, `a_pragma_prefix_prefixes_the_ids_of_its_module`.

**Status:** done

## §6 Validation

**Spec:** A collision differing only by case; a name resolving to more than one declaration; a union discriminator that is integral, boolean, character or an enum; case labels unique and in range; `@key` not `@optional`; a positive array dimension; `fixed<d,s>` with `0 < d <= 31` and `0 <= s <= d`; a positive string bound; forward declarations defined.

**Repo:** `idl-lower/src/validate.rs`, `ast/src/invariant.rs` (the structural ones), `idl-lower/src/lower.rs` (`report_model`, `classify`).

**Tests:** `crates/idl-lower/tests/validation.rs` — 34 tests, one per rule that trips it and one that does not: `two_names_differing_only_by_case_collide`, `a_float_discriminator_is_rejected`, `a_typedef_to_a_legal_discriminator_is_accepted`, `a_repeated_case_label_is_rejected`, `a_case_label_outside_the_discriminators_range_is_rejected`, `a_key_member_must_not_be_optional`, `a_zero_array_dimension_is_rejected`, `a_fixed_with_more_than_thirty_one_digits_is_rejected`, `a_forward_declaration_never_defined_is_rejected`.

**Status:** done

## §7 Diagnostics

**Spec:** Stable, greppable codes; no diagnostic without a specification reference; a rule with no paragraph is ZeroDDS policy and says so.

**Repo:** `idl-parse/src/diagnostic.rs` (`IDL0001`–`IDL0022`, `Diagnostic::file`), `idl-parse/src/preprocess/mod.rs` (`SourceMap::original`, `Preprocessed::locate`), `idl-lower/src/validate.rs` (`rule_name`), `idl-lower/src/lower.rs` (`dedup`). A range points into the file it stands in, not into the expanded text.

**Tests:** `every_diagnostic_carries_a_code_and_a_specification_reference`, `every_diagnostic_names_a_specification_paragraph`, `every_diagnostic_points_at_a_byte_range`, `every_diagnostic_code_has_its_own_string`, `a_preprocessor_diagnostic_comes_before_the_parsers`, `tools/idlc/tests/diagnostic_positions.rs::comments_above_an_error_do_not_move_it`, `tools/idlc/tests/diagnostic_positions.rs::an_error_in_an_include_names_the_include`.

**Status:** done

## §8 Conformance

**Spec:** Every fixture from `e1f3af895` parses; the round trip is byte-exact; the model produced satisfies `validate_complete()`.

**Repo:** `crates/idl-parse/tests/fixtures/` (21 files, each vendor set's `LICENSES.md` preserved), `crates/idl-lower/tests/conformance.rs`.

**Tests:** `every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares`, `every_fixture_round_trips_byte_for_byte`, `every_fixture_lowers_to_exactly_the_diagnostics_it_should`, `the_expected_diagnostics_are_the_ones_the_fixtures_actually_earn`, `every_fixture_produces_a_model_with_declarations_in_it`, `every_fixture_survives_a_snapshot_round_trip`, `the_xtypes_specifications_own_idl_lowers_completely`.

Three fixtures earn a diagnostic, named and reasoned in `EXPECTED`: `ccm_components.idl` forward-declares `CmdBase` and `CompForward` without defining them; `fastdds_pragmas.idl` gives two members the same `@hashid("position")`; `corba_value_repository.idl` names a `Bar` in `typeid`/`typeprefix` that the file never declares.

**Status:** done

---

## Audit status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-cst -p zerodds-idl-parse -p zerodds-idl-lower` — 422 passed, 0 failed.
Across the six crates of this arc (`ast-core`, `ast`, `ast-layout`, `idl-cst`, `idl-parse`, `idl-lower`): 744 passed.

**Open items:**

- One item of this arc is in `docs/OPEN-ITEMS.md` rather than here: DDS-XTypes 1.3 has had no coverage page of its own since the demolition removed it (`internal/spec-coverage/legacy-audit.md` §B.2). That is why §3.6 sits in this document, and why Table 22 of the same sub-clause — `struct A { … }; //@annotation` as a *declaration* of an annotation — sits in none.
- The language CSTs (`zerodds-lang-cst-1.0`) are not built yet; their coverage is a document of its own.
