# `zerodds-ast` v1.0 — coverage

**Source:** [`docs/specs/zerodds-ast-1.0.md`](../specs/zerodds-ast-1.0.md)

**Implementation:** `crates/ast-core`, `crates/ast`, `crates/ast-layout`

**Test run:** `cargo test -p zerodds-ast-core -p zerodds-ast -p zerodds-ast-layout` — 321 passed, 0 failed.
Fuzz: `snapshot_reader` 9,664,902 runs clean; `model_snapshot` 4,520,036 runs clean against the full format after two out-of-memory fixes.

Each row names the carrier — core node, facet, or layout — with the code path and the test that exercises it. A feature with no carrier is marked open with its reason.

---

## §1 Storage and identity

**Spec:** Per-kind generational arenas; a stale handle must fail rather than address the node that later occupied the slot. `NodeId` is stable across snapshots and processes, allocated deterministically.

**Repo:** `ast-core/src/arena.rs`, `ast-core/src/id.rs`. Carrier: core.

**Tests:** `arena::tests::handle_from_before_removal_is_stale_not_reused`, `arena::tests::removed_handles_never_read_back` (proptest over arbitrary interleavings), `id::tests::allocation_is_deterministic_for_the_same_sequence`, `two_fresh_models_agree_on_every_builtin_identity`.

**Status:** done

## §2 References and resolution state

**Spec:** Three-valued `Resolved | Pending | Broken`, never `Option`. A failed attempt records its cause and is not retried implicitly.

**Repo:** `ast-core/src/reference.rs`, `ast/src/resolve.rs`. Carrier: core.

**Tests:** `an_unresolvable_name_lands_in_broken_with_not_found`, `an_ambiguous_name_reports_every_candidate`, `a_broken_reference_is_not_retried_by_a_later_pass`, `a_type_arriving_later_resolves_every_waiter_in_one_step`.

**Status:** done

## §3.1 Modules

**Spec:** Reopening a module across translation units merges into one node.

**Repo:** `ast/src/node/module.rs`, `Ast::ensure_module`. Carrier: core.

**Tests:** `reopening_a_module_merges_into_the_same_node`, `nested_modules_are_addressable_by_path`.

**Status:** done

## §3.2 Types

**Spec:** The `TypeKind` table, row for row, including `float128`, `fixed`, bitset, valuetype, value box, native, object reference and annotation type. A forward declaration is a completeness state, not a kind.

**Repo:** `ast/src/node/type_.rs`. Carrier: core.

**Tests:** `every_primitive_of_the_spec_table_is_representable` (asserts the set is exactly fifteen), `a_forward_declared_type_completes_without_invalidating_handles`, `every_valuetype_modifier_is_representable`.

**Status:** done

## §3.3 Members and aggregate parts

**Spec:** Members, cases, enumerators, bit values, bitfields. An explicit `@value` or `@position` stays distinguishable from a positional one.

**Repo:** `ast/src/node/member.rs`. Carrier: core.

**Tests:** `a_positional_enumerator_and_an_explicit_value_are_distinguishable`, `a_positional_bit_and_an_explicit_position_are_distinguishable`, `a_default_case_is_recorded_on_the_union_as_well_as_in_the_case_list`, `an_anonymous_bitfield_carries_no_name`.

**Status:** done

## §3.4 Interfaces and operations

**Spec:** Seven interface forms, operations with directions and raises, attributes, exceptions. CCM roles ride in the CORBA facet.

**Repo:** `ast/src/node/interface.rs`, `ast/src/node/exception.rs`, `ast/src/facet/corba.rs`. Carrier: core + facet.

**Tests:** `all_seven_interface_kinds_are_representable`, `an_operation_records_its_parameters_in_order_with_directions`, `interface_inheritance_records_every_base_in_order`.

**Status:** done

## §3.5 Constants

**Spec:** Expression trees retained, not folded. Evaluation is a query with checked arithmetic that fails loudly on an unresolved name.

**Repo:** `ast/src/node/constant.rs`, `Ast::eval_integral`. Carrier: core.

**Tests:** `an_expression_tree_survives_as_a_tree`, `evaluation_yields_the_folded_value_without_mutating_the_tree`, `division_by_zero_is_an_error_not_a_panic`, `overflow_is_an_error_not_a_wrap`, `evaluating_through_an_unresolved_name_fails_loudly`, `float_literals_retain_their_written_form`.

**Status:** done

## §3.6 Annotations

**Spec:** Thirty built-ins as ordinary declarations in a builtin module, identical across models.

**Repo:** `ast/src/node/annotation.rs`, `Ast::populate_builtins`. Carrier: core.

**Tests:** `every_builtin_annotation_of_the_spec_list_is_present`, `builtin_identities_come_from_the_builtin_space`, `two_fresh_models_agree_on_every_builtin_identity`.

**Status:** done

## §3.7 Deployment

**Spec:** Domains, topics, QoS profiles with typed policy values, participants, endpoints, libraries. An unwritten policy field stays distinguishable from a defaulted one.

**Repo:** `ast/src/node/deployment.rs`. Carrier: core.

**Tests:** `all_twenty_four_qos_policies_are_representable`, `an_unwritten_policy_field_stays_distinguishable_from_a_default_one`, `a_profile_may_inherit_from_another_profile`.

**Status:** done

## §4 Facets

**Spec:** XTypes, CORBA, ROS 2, DDS-XML, FlatData, vendor and source, each a struct. Vendor directives are not normalised into core constructs.

**Repo:** `ast/src/facet/`. Carrier: facet.

**Tests:** `facets_from_different_sources_coexist_on_one_node`, `vendor_directives_are_not_normalised_into_core_constructs`, `an_absent_facet_is_none_not_a_default_populated_struct`, `the_effective_member_id_and_its_origin_are_both_retained`.

**Status:** done

## §5 Transactions and events

**Spec:** One mutation path; a rejected commit restores the model exactly; observers see only committed state.

**Repo:** `ast/src/edit.rs`, `ast-core/src/txn.rs`, `ast-core/src/observer.rs`. Carrier: core.

**Tests:** `a_rejected_commit_restores_the_model_and_notifies_nobody`, `a_transaction_dropped_unfinished_rolls_back`, `observers_never_see_an_intermediate_state`, `deleting_a_referenced_type_breaks_incoming_references_rather_than_dangling`.

**Status:** done

## §6 Traversal and indices

**Spec:** Cycle-safe walking; four indices maintained from the changeset.

**Repo:** `ast-core/src/walk.rs`, `ast-core/src/index.rs`, `ast/src/query.rs`. Carrier: core.

**Tests:** `a_mutual_cycle_terminates_and_visits_each_node_once`, `a_deep_chain_does_not_depend_on_stack_depth` (100k nodes), `deleting_a_type_removes_it_from_the_index_in_the_same_commit`, `descendants_of_a_recursive_type_terminate_with_the_type_appearing_once`.

**Status:** done

## §7 Layout view

**Spec:** Plans per encoding, endianness and purpose; offsets endianness-invariant; every wire rule in one place.

**Repo:** `ast-layout/src/xcdr.rs`, `ast-layout/src/cache.rs`, `ast-layout/src/keyhash.rs`. Carrier: layout.

**Tests:** `offsets_are_shared_between_little_and_big_endian`, `a_change_evicts_the_type_and_its_dependents_and_nothing_else`, `an_unresolved_member_type_fails_loudly_rather_than_guessing_a_size`, plus V-1…V-12 in `tests/wire_vectors.rs`.

**Status:** done — XCDR2 against the OMG vectors V-1…V-12 in `tests/wire_vectors.rs`; XCDR1 `PL_CDR1` against the rules in `crates/cdr/src/xcdr1.rs` in `tests/xcdr1_gate.rs`, which caught the missing `length > 0xFFFF` extended-header condition and the absent `PID_LIST_END` sentinel. Length codes cross-checked against what Fast DDS and Cyclone emit.

## §7.1 Length-code policy

**Spec:** Both framings XTypes 1.3 §7.4.3.4.2 permits are configurable; the default is the one the other implementations emit, and an unrecognised setting is rejected rather than silently defaulted.

**Repo:** `ast-layout/src/key.rs` — `LengthCodePolicy`, `LayoutKey::with_length_codes`. Carrier: layout.

**Decision:** `Compact` (LC5/LC6/LC7) is the default. Verified in the reference sources on the Linux test host: Fast DDS joins the NEXTINT with the member's length word (Fast-CDR v2.3.5, `include/fastcdr/CdrSizeCalculator.hpp::get_serialized_member_size`), Cyclone returns `LENGTH_CODE_ALSO_NEXTINT` for strings (Cyclone DDS 11.0.1, `src/core/cdr/src/dds_cdrstream.c::get_length_code`). RTI keeps its selection in the binaries and is evidence in neither direction. `UniversalNextInt` (LC4 throughout) stays reachable as a flag, because the pre-rebuild emitters produced it and OMG wire vector V-10 is pinned in that shape.

**Tests:** `the_default_policy_is_the_one_the_other_implementations_emit`, `the_policy_parses_from_a_configuration_value_and_rejects_a_typo`, `the_two_policies_produce_different_framing_for_the_same_type`, plus the V-10 pair in `tests/wire_vectors.rs` covering both forms.

**Status:** done

## §8.1 Structural invariants

**Spec:** Checked on every commit, all violations reported, pending and broken references not violations.

**Repo:** `ast/src/invariant.rs`. Carrier: core.

**Tests:** `duplicate_member_ids_are_rejected_with_both_offenders_named`, `a_pending_reference_is_not_an_invariant_violation`, `the_same_label_is_accepted_while_the_discriminator_is_still_pending`, `every_violation_is_reported_rather_than_only_the_first`.

**Status:** done

## §8.2 Profile validation

**Spec:** Per-specification rules with paragraph references; the same model may pass one profile and fail another.

**Repo:** `ast/src/profile/mod.rs`, `ast/src/profile/verdict.rs`. Carrier: core.

**Tests:** `a_map_is_valid_under_xtypes_and_invalid_under_corba_33`, `a_violation_names_its_rule_and_its_specification_paragraph`, `ros_2_rejects_a_union_and_a_wide_string`, `a_plain_flatdata_type_may_not_reach_a_string`.

**Status:** done — `profile/verdict.rs` rules on every construct (21) and every facet property (14) in every profile (7) with no catch-all arm, so an unruled pair is a compile error. `tests/profile_table.rs` builds one instance of each construct and asserts validate() reports exactly what the table rejects.

## §9 Snapshot

**Spec:** Round-trip preserves every `NodeId`; re-serialisation is byte-identical; malformed input is a typed error.

**Repo:** `ast-core/src/snapshot.rs`, `ast/src/snapshot.rs`. Carrier: core.

**Tests:** `a_populated_model_round_trips_preserving_every_node_id`, `re_serialising_a_loaded_model_is_byte_identical`, `two_independently_built_identical_models_snapshot_identically`, `a_snapshot_declaring_more_elements_than_the_buffer_holds_is_rejected`, `a_snapshot_naming_a_slot_beyond_its_capacity_is_rejected`.

**Status:** done — every one of the 22 node kinds round-trips, with the full facet payload. `tests/snapshot_sections.rs` covers each section on its own so a break names its section (`a_module_round_trips`, `a_type_round_trips`, `a_struct_member_round_trips`, `an_interface_with_an_operation_round_trips`, `the_deployment_layer_round_trips`, `facets_survive`); `every_xtypes_field_survives_the_round_trip`, `every_corba_field_survives_the_round_trip` and `every_ros2_and_xml_field_survives_the_round_trip` assert beforehand that the fixture really holds every facet field. A `Pending` reference comes back on its key and resolves after the load (`a_pending_reference_comes_back_pending_on_the_same_key`), a `Broken` one with its cause (`a_broken_reference_keeps_its_cause`), and a constant keeps its type (`a_constant_round_trips`).

## §10 Error model

**Spec:** No panic on any reachable path; typed errors carrying the offending node.

**Repo:** `ast/src/error.rs`, `ast-core/src/snapshot.rs`, `ast-layout/src/plan.rs`; the workspace lints deny `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`. Carrier: all three.

**Tests:** `snapshot::tests::a_truncated_stream_is_an_error_not_a_partial_value`, `snapshot::tests::a_length_prefix_beyond_the_buffer_is_rejected`, `snapshot::tests::the_header_round_trips_and_a_wrong_magic_is_rejected`, `snapshot::tests::an_unknown_version_is_rejected`, `arena::tests::double_remove_is_an_error_not_a_second_value`, `an_unresolved_member_type_fails_loudly_rather_than_guessing_a_size` — each case yields a typed error rather than a panic. Plus the lint barrier `cargo clippy --all-targets -- -D warnings` across the three crates and both fuzz targets.

**Status:** done

## §11 Coverage obligation

**Spec:** §11 — completeness of the superset MUST be demonstrated in `docs/spec-coverage/zerodds-ast-1.0.md` as a bilingual pair; each row names one specification feature with its paragraph reference, its carrier (core node, facet or layout view), the implementing code path and the test that exercises it. An empty cell is an open break and is reported as one.

**Repo:** `docs/spec-coverage/zerodds-ast-1.0.md` + `docs/spec-coverage/zerodds-ast-1.0.en.md`. Carrier: all three — the obligation sits on the document, not on a crate.

**Tests:** `tools/traceability/tests/ast_coverage_obligation.rs::the_pair_carries_the_same_rows_in_the_same_order` (bilingual pair, same rows, same status), `tools/traceability/tests/ast_coverage_obligation.rs::no_done_row_leaves_a_cell_empty` (no empty cell under `done`), `tools/traceability/tests/ast_coverage_obligation.rs::every_cited_test_name_exists_in_the_three_crates` (every cited test name is defined in `ast-core`/`ast`/`ast-layout`), `tools/traceability/tests/ast_coverage_obligation.rs::every_row_names_its_carrier`, `tools/traceability/tests/ast_coverage_obligation.rs::the_audit_aggregate_matches_the_rows`.

**Status:** done

---

## Audit status

19 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-ast-core -p zerodds-ast -p zerodds-ast-layout` — 321 passed, 0 failed.

§11 obligation checked mechanically: `cargo test -p zerodds-traceability --test ast_coverage_obligation` — 5 passed, 0 failed.

Open items: none. Decision records: none.
