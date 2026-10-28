# `zerodds-ast` v1.0 — Coverage

**Quelle:** [`docs/specs/zerodds-ast-1.0.md`](../specs/zerodds-ast-1.0.md)

**Implementation:** `crates/ast-core`, `crates/ast`, `crates/ast-layout`

**Test-Lauf:** `cargo test -p zerodds-ast-core -p zerodds-ast -p zerodds-ast-layout` — 321 bestanden, 0 fehlgeschlagen.
Fuzz: `snapshot_reader` 9.664.902 Läufe ohne Fund; `model_snapshot` 4.520.036 Läufe ohne Fund gegen das volle Format nach zwei Out-of-Memory-Fixes.

Jede Zeile nennt den Träger — Kern-Knoten, Facette oder Layout — mit Code-Pfad und dem Test, der ihn prüft. Ein Feature ohne Träger steht als offen mit seinem Grund.

---

## §1 Speicher und Identität

**Spec:** Generationsbehaftete Arenen pro Knotenart; ein veraltetes Handle muss fehlschlagen, statt den Knoten anzusprechen, der den Slot später belegt. `NodeId` ist über Snapshots und Prozesse stabil und wird deterministisch vergeben.

**Repo:** `ast-core/src/arena.rs`, `ast-core/src/id.rs`. Träger: Kern.

**Tests:** `arena::tests::handle_from_before_removal_is_stale_not_reused`, `arena::tests::removed_handles_never_read_back` (proptest über beliebige Verschränkungen), `id::tests::allocation_is_deterministic_for_the_same_sequence`, `two_fresh_models_agree_on_every_builtin_identity`.

**Status:** done

## §2 Referenzen und Auflösungszustand

**Spec:** Dreiwertig `Resolved | Pending | Broken`, nie `Option`. Ein gescheiterter Versuch hält seine Ursache fest und wird nicht implizit wiederholt.

**Repo:** `ast-core/src/reference.rs`, `ast/src/resolve.rs`. Träger: Kern.

**Tests:** `an_unresolvable_name_lands_in_broken_with_not_found`, `an_ambiguous_name_reports_every_candidate`, `a_broken_reference_is_not_retried_by_a_later_pass`, `a_type_arriving_later_resolves_every_waiter_in_one_step`.

**Status:** done

## §3.1 Module

**Spec:** Ein über Übersetzungseinheiten hinweg wiedergeöffnetes Modul verschmilzt zu einem Knoten.

**Repo:** `ast/src/node/module.rs`, `Ast::ensure_module`. Träger: Kern.

**Tests:** `reopening_a_module_merges_into_the_same_node`, `nested_modules_are_addressable_by_path`.

**Status:** done

## §3.2 Typen

**Spec:** Die `TypeKind`-Tabelle Zeile für Zeile, inklusive `float128`, `fixed`, Bitset, Valuetype, Value-Box, Native, Object-Reference und Annotation-Typ. Eine Forward-Deklaration ist ein Vollständigkeitszustand, keine eigene Art.

**Repo:** `ast/src/node/type_.rs`. Träger: Kern.

**Tests:** `every_primitive_of_the_spec_table_is_representable` (prüft, dass die Menge genau fünfzehn Einträge hat), `a_forward_declared_type_completes_without_invalidating_handles`, `every_valuetype_modifier_is_representable`.

**Status:** done

## §3.3 Member und Aggregat-Teile

**Spec:** Member, Cases, Enumeratoren, Bit-Werte, Bitfields. Ein explizites `@value` oder `@position` bleibt von einem positionellen unterscheidbar.

**Repo:** `ast/src/node/member.rs`. Träger: Kern.

**Tests:** `a_positional_enumerator_and_an_explicit_value_are_distinguishable`, `a_positional_bit_and_an_explicit_position_are_distinguishable`, `a_default_case_is_recorded_on_the_union_as_well_as_in_the_case_list`, `an_anonymous_bitfield_carries_no_name`.

**Status:** done

## §3.4 Interfaces und Operationen

**Spec:** Sieben Interface-Formen, Operationen mit Richtungen und `raises`, Attribute, Exceptions. CCM-Rollen sitzen in der CORBA-Facette.

**Repo:** `ast/src/node/interface.rs`, `ast/src/node/exception.rs`, `ast/src/facet/corba.rs`. Träger: Kern + Facette.

**Tests:** `all_seven_interface_kinds_are_representable`, `an_operation_records_its_parameters_in_order_with_directions`, `interface_inheritance_records_every_base_in_order`.

**Status:** done

## §3.5 Konstanten

**Spec:** Ausdrucksbäume bleiben erhalten, werden nicht gefaltet. Auswertung ist eine Query mit geprüfter Arithmetik, die bei einem unaufgelösten Namen laut fehlschlägt.

**Repo:** `ast/src/node/constant.rs`, `Ast::eval_integral`. Träger: Kern.

**Tests:** `an_expression_tree_survives_as_a_tree`, `evaluation_yields_the_folded_value_without_mutating_the_tree`, `division_by_zero_is_an_error_not_a_panic`, `overflow_is_an_error_not_a_wrap`, `evaluating_through_an_unresolved_name_fails_loudly`, `float_literals_retain_their_written_form`.

**Status:** done

## §3.6 Annotationen

**Spec:** Dreißig Builtins als gewöhnliche Deklarationen in einem Builtin-Modul, über Modelle hinweg identisch.

**Repo:** `ast/src/node/annotation.rs`, `Ast::populate_builtins`. Träger: Kern.

**Tests:** `every_builtin_annotation_of_the_spec_list_is_present`, `builtin_identities_come_from_the_builtin_space`, `two_fresh_models_agree_on_every_builtin_identity`.

**Status:** done

## §3.7 Deployment

**Spec:** Domains, Topics, QoS-Profile mit typisierten Policy-Werten, Participants, Endpoints, Libraries. Ein nicht geschriebenes Policy-Feld bleibt von einem auf Default gesetzten unterscheidbar.

**Repo:** `ast/src/node/deployment.rs`. Träger: Kern.

**Tests:** `all_twenty_four_qos_policies_are_representable`, `an_unwritten_policy_field_stays_distinguishable_from_a_default_one`, `a_profile_may_inherit_from_another_profile`.

**Status:** done

## §4 Facetten

**Spec:** XTypes, CORBA, ROS 2, DDS-XML, FlatData, Vendor und Source, jede ein `struct`. Vendor-Direktiven werden nicht in Kern-Konstrukte normalisiert.

**Repo:** `ast/src/facet/`. Träger: Facette.

**Tests:** `facets_from_different_sources_coexist_on_one_node`, `vendor_directives_are_not_normalised_into_core_constructs`, `an_absent_facet_is_none_not_a_default_populated_struct`, `the_effective_member_id_and_its_origin_are_both_retained`.

**Status:** done

## §5 Transaktionen und Events

**Spec:** Ein Mutationspfad; ein abgelehnter Commit stellt das Modell exakt wieder her; Observer sehen nur committeten Zustand.

**Repo:** `ast/src/edit.rs`, `ast-core/src/txn.rs`, `ast-core/src/observer.rs`. Träger: Kern.

**Tests:** `a_rejected_commit_restores_the_model_and_notifies_nobody`, `a_transaction_dropped_unfinished_rolls_back`, `observers_never_see_an_intermediate_state`, `deleting_a_referenced_type_breaks_incoming_references_rather_than_dangling`.

**Status:** done

## §6 Traversierung und Indizes

**Spec:** Zyklensicheres Walking; vier Indizes, aus dem Changeset gepflegt.

**Repo:** `ast-core/src/walk.rs`, `ast-core/src/index.rs`, `ast/src/query.rs`. Träger: Kern.

**Tests:** `a_mutual_cycle_terminates_and_visits_each_node_once`, `a_deep_chain_does_not_depend_on_stack_depth` (100k Knoten), `deleting_a_type_removes_it_from_the_index_in_the_same_commit`, `descendants_of_a_recursive_type_terminate_with_the_type_appearing_once`.

**Status:** done

## §7 Layout-View

**Spec:** Pläne pro Encoding, Endianness und Zweck; Offsets endianness-invariant; jede Wire-Regel an genau einer Stelle.

**Repo:** `ast-layout/src/xcdr.rs`, `ast-layout/src/cache.rs`, `ast-layout/src/keyhash.rs`. Träger: Layout.

**Tests:** `offsets_are_shared_between_little_and_big_endian`, `a_change_evicts_the_type_and_its_dependents_and_nothing_else`, `an_unresolved_member_type_fails_loudly_rather_than_guessing_a_size`, dazu V-1…V-12 in `tests/wire_vectors.rs`.

**Status:** done — XCDR2 gegen die OMG-Vektoren V-1…V-12 in `tests/wire_vectors.rs`; XCDR1-`PL_CDR1` gegen die Regeln in `crates/cdr/src/xcdr1.rs` in `tests/xcdr1_gate.rs`, was die fehlende `length > 0xFFFF`-Bedingung für den extended Header und den fehlenden `PID_LIST_END`-Sentinel aufdeckte. Length-Codes gegen das abgeglichen, was Fast DDS und Cyclone emittieren.

## §7.1 Length-Code-Policy

**Spec:** Beide Rahmungen, die XTypes 1.3 §7.4.3.4.2 erlaubt, sind konfigurierbar; der Default ist die Form, die die anderen Implementierungen emittieren, und ein unbekannter Wert wird abgelehnt statt still auf den Default zu fallen.

**Repo:** `ast-layout/src/key.rs` — `LengthCodePolicy`, `LayoutKey::with_length_codes`. Träger: Layout.

**Entscheidung:** `Compact` (LC5/LC6/LC7) ist der Default. In den Referenz-Quellen auf dem Linux-Testhost verifiziert: Fast DDS legt den NEXTINT mit dem Längenwort des Members zusammen (Fast-CDR v2.3.5, `include/fastcdr/CdrSizeCalculator.hpp::get_serialized_member_size`), Cyclone liefert `LENGTH_CODE_ALSO_NEXTINT` für Strings (Cyclone DDS 11.0.1, `src/core/cdr/src/dds_cdrstream.c::get_length_code`). RTI hält seine Auswahl in den Binaries und ist damit kein Beleg in beide Richtungen. `UniversalNextInt` (durchgehend LC4) bleibt als Flag erreichbar, weil die Emitter vor dem Rebuild das erzeugten und der OMG-Wire-Vektor V-10 in dieser Form fixiert ist.

**Tests:** `the_default_policy_is_the_one_the_other_implementations_emit`, `the_policy_parses_from_a_configuration_value_and_rejects_a_typo`, `the_two_policies_produce_different_framing_for_the_same_type`, dazu das V-10-Paar in `tests/wire_vectors.rs`, das beide Formen abdeckt.

**Status:** done

## §8.1 Strukturelle Invarianten

**Spec:** Bei jedem Commit geprüft, alle Verletzungen gemeldet, `Pending` und `Broken` sind keine Verletzungen.

**Repo:** `ast/src/invariant.rs`. Träger: Kern.

**Tests:** `duplicate_member_ids_are_rejected_with_both_offenders_named`, `a_pending_reference_is_not_an_invariant_violation`, `the_same_label_is_accepted_while_the_discriminator_is_still_pending`, `every_violation_is_reported_rather_than_only_the_first`.

**Status:** done

## §8.2 Profil-Validierung

**Spec:** Regeln pro Spezifikation mit Paragrafen-Referenz; dasselbe Modell kann ein Profil bestehen und ein anderes nicht.

**Repo:** `ast/src/profile/mod.rs`, `ast/src/profile/verdict.rs`. Träger: Kern.

**Tests:** `a_map_is_valid_under_xtypes_and_invalid_under_corba_33`, `a_violation_names_its_rule_and_its_specification_paragraph`, `ros_2_rejects_a_union_and_a_wide_string`, `a_plain_flatdata_type_may_not_reach_a_string`.

**Status:** done — `profile/verdict.rs` urteilt über jedes Konstrukt (21) und jede Facetten-Eigenschaft (14) in jedem Profil (7), ohne Catch-all-Arm; eine nicht beurteilte Paarung ist ein Compile-Fehler. `tests/profile_table.rs` baut je eine Instanz jedes Konstrukts und prüft, dass validate() genau meldet, was die Tabelle ablehnt.

## §9 Snapshot

**Spec:** Round-Trip erhält jede `NodeId`; Re-Serialisierung ist byte-identisch; fehlerhafte Eingabe ist ein typisierter Fehler.

**Repo:** `ast-core/src/snapshot.rs`, `ast/src/snapshot.rs`. Träger: Kern.

**Tests:** `a_populated_model_round_trips_preserving_every_node_id`, `re_serialising_a_loaded_model_is_byte_identical`, `two_independently_built_identical_models_snapshot_identically`, `a_snapshot_declaring_more_elements_than_the_buffer_holds_is_rejected`, `a_snapshot_naming_a_slot_beyond_its_capacity_is_rejected`.

**Status:** done — alle 22 Knotenarten laufen durch den Roundtrip, samt voller Facetten-Nutzlast. `tests/snapshot_sections.rs` deckt jede Sektion einzeln ab, damit ein Bruch seine Sektion benennt (`a_module_round_trips`, `a_type_round_trips`, `a_struct_member_round_trips`, `an_interface_with_an_operation_round_trips`, `the_deployment_layer_round_trips`, `facets_survive`); `every_xtypes_field_survives_the_round_trip`, `every_corba_field_survives_the_round_trip` und `every_ros2_and_xml_field_survives_the_round_trip` prüfen vorher, dass das Fixture wirklich jedes Facetten-Feld hält. Eine `Pending`-Referenz kommt mit ihrem Schlüssel zurück und löst sich nach dem Laden auf (`a_pending_reference_comes_back_pending_on_the_same_key`), eine `Broken`-Referenz mit ihrer Ursache (`a_broken_reference_keeps_its_cause`), und eine Konstante behält ihren Typ (`a_constant_round_trips`).

## §10 Fehlermodell

**Spec:** Kein Panic auf erreichbarem Pfad; typisierte Fehler mit dem betroffenen Knoten.

**Repo:** `ast/src/error.rs`, `ast-core/src/snapshot.rs`, `ast-layout/src/plan.rs`; die Workspace-Lints setzen `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable` auf deny. Träger: alle drei.

**Tests:** `snapshot::tests::a_truncated_stream_is_an_error_not_a_partial_value`, `snapshot::tests::a_length_prefix_beyond_the_buffer_is_rejected`, `snapshot::tests::the_header_round_trips_and_a_wrong_magic_is_rejected`, `snapshot::tests::an_unknown_version_is_rejected`, `arena::tests::double_remove_is_an_error_not_a_second_value`, `an_unresolved_member_type_fails_loudly_rather_than_guessing_a_size` — jeder Fall liefert einen typisierten Fehler statt eines Panics. Dazu die Lint-Schranke `cargo clippy --all-targets -- -D warnings` über die drei Crates und beide Fuzz-Targets.

**Status:** done

## §11 Coverage-Pflicht

**Spec:** §11 — die Vollständigkeit des Supersets MUSS in `docs/spec-coverage/zerodds-ast-1.0.md` als zweisprachiges Paar nachgewiesen werden; jede Zeile nennt ein Spec-Feature mit Paragrafen-Referenz, ihren Träger (Kern-Knoten, Facette oder Layout-View), den Code-Pfad und den Test, der ihn prüft. Eine leere Zelle ist ein offener Bruch und wird als solcher gemeldet.

**Repo:** `docs/spec-coverage/zerodds-ast-1.0.md` + `docs/spec-coverage/zerodds-ast-1.0.en.md`. Träger: alle drei — die Pflicht liegt auf dem Dokument, nicht auf einer Crate.

**Tests:** `tools/traceability/tests/ast_coverage_obligation.rs::the_pair_carries_the_same_rows_in_the_same_order` (zweisprachiges Paar, gleiche Zeilen, gleicher Status), `tools/traceability/tests/ast_coverage_obligation.rs::no_done_row_leaves_a_cell_empty` (keine leere Zelle unter `done`), `tools/traceability/tests/ast_coverage_obligation.rs::every_cited_test_name_exists_in_the_three_crates` (jeder zitierte Testname ist in `ast-core`/`ast`/`ast-layout` definiert), `tools/traceability/tests/ast_coverage_obligation.rs::every_row_names_its_carrier`, `tools/traceability/tests/ast_coverage_obligation.rs::the_audit_aggregate_matches_the_rows`.

**Status:** done

---

## Audit-Status

19 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-ast-core -p zerodds-ast -p zerodds-ast-layout` — 321 bestanden, 0 fehlgeschlagen.

§11-Pflicht mechanisch geprüft: `cargo test -p zerodds-traceability --test ast_coverage_obligation` — 5 bestanden, 0 fehlgeschlagen.

Offene Punkte: keine. Decision-Records: keine.
