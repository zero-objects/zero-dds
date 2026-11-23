# OMG CORBA 3.3 — Spec Coverage (WP CORBA Coexistence)

**Source:** OMG CORBA 3.3 — three volumes as formal documents:
* Part 1 (Interfaces, 532 pp.) — [`formal/12-11-12`](https://www.omg.org/spec/CORBA/3.3/Interfaces/PDF)
* Part 2 (Interoperability, 249 pp.) — [`formal/12-11-14`](https://www.omg.org/spec/CORBA/3.3/Interoperability/PDF)
* Part 3 (Component Model, 380 pp.) — [`formal/12-11-16`](https://www.omg.org/spec/CORBA/3.3/Components/PDF)

**Context.** With the DDS core and the codegen in
`crates/lang-rules/` and `crates/corba-codegen/`, ZeroDDS fully covers
the DDS PSM. For migrating legacy CORBA applications in
the finance industry, ZeroDDS should be able to appear as a **drop-in
backend**: an existing CORBA client compiles and links against
Annex-A.1 stubs, runs against ZeroDDS endpoints that speak GIOP/IIOP
wire, and can be migrated incrementally to pure DDS without
ad-hoc infrastructure swaps.

CORBA coexistence is spread across several implementation crates:

- `crates/idl-parse/` — IDL grammar (Part 1 §7 IDL syntax)
- `crates/corba-codegen/` — Annex-A.1 codegen (stubs/skeletons, Part 1 §7)
- `crates/corba-ir/` — Interface Repository (Part 1 §14)
- `crates/corba-poa/` — Portable Object Adapter (Part 1 §15)
- `crates/corba-giop/` — GIOP wire protocol (Part 2 §9)
- `crates/corba-iiop/` — IIOP transport (Part 2 §9.7)
- `crates/corba-ior/` — IOR encoding (Part 2 §9.4 / §10.5)
- `crates/corba-csiv2/` — CSIv2 security (Part 2 §10)
- `crates/corba-ccm/` — Component Model (Part 3 §6-§9)
- `crates/corba-ccm-lib/` — CCM container (Part 3 §10)
- `crates/corba-ccm-ejb/` — EJB integration (Part 3 §11)
- `crates/corba-dnc/` — Deployment & Configuration (Part 3 §15-§17)
- `crates/corba-dds-bridge/` — ZeroDDS bridge (no OMG item)
- `crates/corba-cosnaming/` — COS Naming v1.3 (separate spec)
- `crates/corba-cos-event/` — COS Event Service v1.2 (separate spec)
- `crates/dcps/` — DDS PSM core (DDS migration target)
- `crates/rpc/` — DDS-RPC core (DDS migration target)

CosNaming and CosEventService are standalone OMG specs (formal/04-10-03
and formal/04-10-02); CosEvent has its own spec-coverage file
(`cos-event-service-1.4.md`). CosNaming coverage is carried inline here,
because CORBA Part 1 §8.5.2 requires it as a standard initial reference.

---

## Part 1: Interfaces

### §1 Scope

**Spec:** Part 1 §1, p. 1 — scope of the CORBA specification.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — scope.

### §2 Conformance and Compliance

**Spec:** Part 1 §2.1, p. 1 — "The minimum required for a
CORBA-compliant system is adherence to the specifications in this
standard and one mapping. Each additional language mapping is a
separate, optional compliance point."

**Repo:** —

**Tests:** —

**Status:** open — adherence to the standard is the sum of the items in
this document, plus an OMG language mapping whose generated client and
servant run on an ORB. Coverage documents against OMG mappings exist:
`idl4-cpp-1.0` with the CORBA platform of Annex A.1, where among others
`ORB_init` and an implementation of `CORBA::ORB` (A.1.4), the factories
on the ORB (A.1.8.9), `PortableServer`, servants and skeletons
(A.1.1.11, A.1.8.8, A.1.10 to A.1.13) are open; `idl4-java-1.0` and
`idl4-csharp-1.0` without open items, but without an ORB and object
adapter in `runtime/java` and `runtime/csharp` (the CORBA platform of
Java maps `TypeCode`, `Object`, `Any` and `UserException`). Missing: one
of these mappings on the ORB of ZeroDDS (`crates/corba-interop`,
`crates/corba-poa`), a test in which its generated client and servant
talk to a foreign ORB, and the open and partial items of this document.
The mapping to Rust (`zerodds-corba-rust-1.1`) is an own specification,
not an OMG mapping.

### §3 Normative References

**Spec:** Part 1 §3, p. 1 — list of normative references.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

### §4 Additional Information

**Spec:** Part 1 §4, p. 2 — outline + keyword conventions
(MUST/SHALL/SHOULD/MAY).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — outline of Part 1 and the reading of the
keywords per RFC 2119; no requirement on an implementation.

### §5 The Object Model

**Spec:** Part 1 §5, p. 3 — abstract object model (Objects, Requests,
Types, Interfaces, Value Types, Operations, Attributes).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — conceptual framework, no wire/
code obligation.

### §6 CORBA Overview

**Spec:** Part 1 §6, p. 11 — architecture overview (ORB, stubs, DSI,
Object Adapter, IR).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — overview; the requirements on ORB, DII,
DSI, object adapter and IR are in §7–§18.

### §7 IDL Syntax and Semantics

**Spec:** Part 1 §7, p. 27 — complete IDL grammar (lexer, module,
interface, value, constant, type, exception, operation, attribute,
repository identity, event, component, home, names & scoping).

**Repo:** `crates/idl-cst/src/lex.rs`, `crates/idl-parse/src/decl.rs`,
`crates/idl-lower/src/lower.rs`, `crates/corba-codegen/src/repository_id.rs`,
`crates/corba-codegen/src/special_types.rs`, `crates/corba-codegen/src/skeleton.rs`,
`crates/corba-codegen/src/stub.rs`.

**Tests:** `crates/idl-parse/tests/parse.rs::{each_declaration_form_is_recognised,
a_file_using_most_of_the_grammar_parses_cleanly_and_round_trips}`,
`crates/idl-lower/tests/conformance.rs::{every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares,
every_fixture_lowers_to_exactly_the_diagnostics_it_should}`,
`crates/corba-codegen/src/repository_id.rs::{a_nested_module_path,
splitting_takes_the_last_colon_and_checks_the_version_is_a_version}`.

**Status:** done — IDL 4.2 is a superset of CORBA-IDL §7. Rule-by-rule
coverage in `docs/spec-coverage/idl-4.2.md` (649 done / 4 partial).
Repository identity (§7.15) is implemented via
`corba-codegen::repository_id`. Component and home declarations
(§7.17/§7.18) are restated in Part 3 §6 and are `done` there.

### §8 ORB Interface

**Spec:** Part 1 §8, p. 93 — ORB pseudo-object: `ORB_init`,
`string_to_object`/`object_to_string`, `resolve_initial_references`,
policy management, TypeCodes, standard exceptions.

**Repo:** `crates/corba-ccm/src/orb_core.rs::{orb_init, OrbArguments, Orb,
ObjectRef, ResolveError}` (`ORB_init` with the `-ORB<suffix>` arguments,
ORB table by ORBid, `id`, `object_to_string`, `string_to_object` over
`IOR:`, `corbaloc:` with `iiop` and `rir`, `corbaname:` with `resolve` through
the installed `CorbaConnection`, `list_initial_services`,
`resolve_initial_references`, `register_initial_reference`,
`create_policy`, `policy_manager`, `policy_current`, `get_client_policy`,
`ObjectRef::{set_policy_overrides, get_policy_overrides}`, `shutdown`,
`destroy`); `crates/corba-ccm/src/policy_management.rs` (`PolicyManager`,
`PolicyCurrent`, `SetOverrideType`, `OverrideError`, `check_overrides`,
`set_object_overrides`, `get_object_overrides`, `client_policy`);
`crates/corba-rt/src/object.rs::{ObjectReference::policy_overrides,
PolicyOverride}` (the overrides of a reference, not part of the IOR);
`crates/corba-rt/src/exception.rs::{SystemException, SystemExceptionKind,
CompletionStatus, OMGVMCID}` (§8.12);
`crates/corba-rust/src/runtime.rs::CorbaException::System`;
`crates/corba-interop/src/runtime.rs::{orb_init, reply_status_and_body,
decode_system_exception}` (system exception with its own repository id
on the wire, ORB with the IIOP transport); `crates/corba-ior/src/{stringified.rs,
url.rs}`.

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests`:
`corba_p1_8_5_1_orb_init_takes_its_arguments_out_of_the_list`,
`corba_p1_8_5_1_an_unrecognised_orb_parameter_or_a_missing_value_is_bad_param`,
`corba_p1_8_5_1_the_same_orbid_gives_the_same_orb_until_destroy`,
`corba_p1_8_5_1_orb_init_runs_the_initializers_without_holding_other_callers`,
`corba_p1_8_5_3_2_an_init_ref_url_is_checked_and_corbaloc_rir_refused`,
`corba_p1_8_5_3_4_resolution_order_register_init_ref_configured_default`,
`corba_p1_8_5_2_an_unknown_or_nil_initial_reference_is_invalid_name`,
`corba_p1_16_10_register_initial_reference_refuses_empty_duplicate_and_nil`,
`corba_p1_8_5_2_a_local_initial_reference_narrows_to_its_type`,
`corba_p1_8_2_2_string_to_object_of_object_to_string_is_the_same_reference`,
`corba_p1_8_2_2_string_to_object_raises_the_bad_param_minor_codes`,
`corba_p1_8_2_2_corbaloc_iiop_builds_an_iiop_profile`,
`corba_p1_8_2_2_corbaloc_rir_resolves_an_initial_reference`,
`corba_p1_8_2_2_corbaname_resolves_the_name_on_the_naming_context`,
`corba_p1_8_2_5_after_shutdown_the_orb_operations_raise_bad_inv_order_4`,
`corba_p1_8_2_5_5_a_destroyed_orb_raises_object_not_exist`,
`corba_p1_8_5_3_3_a_rir_default_init_ref_is_refused_and_a_rir_cycle_ends`,
`corba_p1_8_9_1_policy_manager_and_policy_current_are_initial_references`;
`crates/corba-ccm/src/policy_management.rs::tests::{corba_p1_8_9_3_set_and_add_override_at_a_scope,
corba_p1_8_9_3_2_refused_lists_change_nothing,
corba_p1_8_9_1_policy_current_is_per_thread_and_starts_empty,
corba_p1_8_3_9_object_overrides_make_a_new_reference,
corba_p1_8_3_8_2_the_client_policy_is_the_reference_then_the_thread_then_the_orb}`;
`crates/corba-interop/tests/ziop.rs::corba_p1_8_9_1_a_servant_runs_without_the_thread_overrides`;
`crates/corba-rt/src/exception.rs::tests`:
`corba_p1_8_12_3_every_standard_system_exception_has_its_repository_id`,
`corba_p1_8_12_2_completion_status_values_follow_the_enum`,
`corba_p1_8_12_2_a_standard_minor_code_carries_the_omg_vmcid`,
`corba_p2_9_4_3_system_exception_body_round_trips_in_both_byte_orders`,
`corba_p1_8_12_2_a_non_standard_system_exception_is_presented_as_unknown_minor_2`,
`corba_p2_9_4_3_a_completion_status_outside_the_enum_is_refused`,
`corba_p1_8_12_display_names_kind_minor_and_completion`;
`crates/corba-interop/tests/system_exceptions.rs`:
`corba_p1_8_12_a_system_exception_crosses_with_kind_minor_and_completion`,
`corba_p2_9_4_3_the_system_exception_reply_body_names_the_exception`;
`crates/corba-interop/tests/orb_init.rs`:
`corba_p1_8_5_3_2_an_init_ref_corbaname_resolves_over_iiop`;
`crates/corba-interop/tests/amh.rs::corbarust_6_amh_deferred_exception`;
`crates/corba-interop/tests/generated_exceptions.rs`:
`corba_p1_8_12_2_the_skeleton_raises_marshal_completed_no_for_bad_arguments`,
`corba_p1_8_12_3_1_an_unlisted_user_exception_is_unknown_minor_1_completed_yes`,
`corba_p1_8_12_2_the_stub_raises_marshal_completed_yes_for_a_bad_reply`;
`crates/corba-rust/tests/codegen_mapping.rs`:
`corbarust_3_1_an_undeclared_user_exception_is_unknown_minor_1_completed_yes`,
`corbarust_4_1_a_factory_initialiser_raises_no_implement_minor_1`;
`crates/corba-ior/src/stringified.rs::tests`: `round_trip_be`,
`round_trip_le`, `upper_and_lower_hex_both_decoded`,
`missing_prefix_is_diagnostic`, `odd_hex_length_is_diagnostic`,
`non_hex_char_is_diagnostic`, `nil_ior_round_trip`;
`crates/corba-ior/src/url.rs::tests`: `parse_simple_iiop_corbaloc`,
`default_port_2809_when_missing`, `rir_endpoint`, `multi_endpoint_failover`,
`corbaname_with_fragment`.

**Status:** partial — built and green on the Linux test host:
§8.2.1 `id`; §8.2.2 `object_to_string`/`string_to_object` over `IOR:`,
`corbaloc:` (`iiop`, `rir`; one IIOP profile per address) and
`corbaname:` (resolution with `resolve` over IIOP), `file://`, `http://`
and `ftp://` (Part 2 §7.6.10), every profile of an IOR kept, BAD_PARAM
minor codes 7 to 10, MARSHAL minor 4 for a local object; §8.5.1 `ORB_init` with `-ORBid`, `-ORBInitRef`,
`-ORBDefaultInitRef`, `-ORBServerId`, `-ORBListenEndpoints`,
`-ORBNoProprietaryActivation` (removed from the argument list, an unknown
`-ORB<suffix>` or a missing value is BAD_PARAM, the same ORBid gives the
same ORB until `destroy`); §8.5.2 and §8.5.3 initial references in the
order of §8.5.3.4, never nil, else `InvalidName`; §16.10
`register_initial_reference`; a `rir` resolution that leads back to the
id being resolved (`-ORBInitRef X=corbaname:rir:/X#n`) ends with
`InvalidName` or BAD_PARAM minor 10, `-ORBDefaultInitRef corbaloc:rir:`
is BAD_PARAM minor 8; §8.12 system exceptions with identity,
minor code and completion status on the wire in both directions (before,
the server wrote `UNKNOWN` for every system exception), a foreign system
exception becomes UNKNOWN minor 2; the generated skeleton reports
arguments that do not unmarshal as MARSHAL COMPLETED_NO and an undeclared
user exception as UNKNOWN minor 1 COMPLETED_YES, the generated stub a reply
that does not unmarshal as MARSHAL COMPLETED_YES, a `factory` initialiser
of a valuetype NO_IMPLEMENT minor 1; BAD_INV_ORDER minor 4 after
`shutdown`, OBJECT_NOT_EXIST after `destroy` (§8.2.5.5; `destroy` shuts a
running ORB down first); `create_policy` over the policy factories of the
ORB initializers; §8.9 policy management: the ORB's `PolicyManager`
(initial reference `ORBPolicyManager`), `PolicyCurrent` (initial
reference `PolicyCurrent`, per thread, a new thread without overrides; a
servant runs without the overrides of its thread, and what it sets there
ends with the request), `set_policy_overrides` with `SET_OVERRIDE` and
`ADD_OVERRIDE` and `get_policy_overrides` at every level; §8.3.9
`set_policy_overrides` on a reference gives a new reference, the
overrides do not go on the wire; §8.3.8.3 `get_policy_overrides`;
§8.3.8.2 `get_client_policy` (reference before thread before ORB);
BAD_PARAM minor 30 for a type twice in the list, `NO_PERMISSION` for
every type except the four ZIOP policies, because the ORB applies only
these to an invocation, `InvalidPolicies` with the indices of invalid
values; a refused call changes nothing; a local object NO_IMPLEMENT
minor 8 (§8.3.14). `Orb::set_policy` and `get_policy`, an octet store
nothing read, are removed. Open: §8.2.5 `work_pending`, `perform_work`, `run` and
`shutdown(wait_for_completion)` as waiting for running requests, with it
`destroy` waiting for the end of the shutdown and BAD_INV_ORDER minor 3 for
`shutdown(TRUE)` and `destroy` in a thread that serves a request;
`create_policy`, the policy overrides and the CCM extensions
(`with_*`) do not check the state of the ORB (no BAD_INV_ORDER 4 after
`shutdown`, no OBJECT_NOT_EXIST after `destroy`); MARSHAL COMPLETED_NO when
the stub writes the request and COMPLETED_YES when the skeleton writes the
reply are not tested, because the IDL of the tests has no type whose
writing fails; §8.2.3
`get_service_information`; §8.2.4 and §8.6 Context object; §8.3 object
operations on `ObjectReference` (`is_a`, `non_existent`,
`is_equivalent`, `hash`, `get_interface`, `get_component`, `get_policy`
with the reconciliation against the policies of the IOR,
`validate_connection`); §8.4 and the value factory registry on the ORB;
§8.9 overrides of the policies of a policy factory (`NO_PERMISSION` until
`ClientRequestInfo::get_request_policy`, §16.4.13.8, shows them to an
interceptor); §8.10 domain managers;
§8.11 TypeCode operations (`equal`, `equivalent`, `get_compact_typecode`,
accessors) and the `create_*_tc` operations, which need the TypeCode
kinds union, array, fixed, value, value_box, native, abstract and local
interface, component, home and event (§9.3). Vendor proof (an
omniORB client reads identity and minor code of a system exception a
ZeroDDS servant raises) not run.

### §9 Value Type Semantics

**Spec:** Part 1 §9, p. 155 — valuetype marshaling (CDR representation,
boxed value, custom marshaling, sending-context runtime).

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(codec base), `crates/idl-parse/src/decl.rs` (`parse_valuetype`, value
box and `custom valuetype`), `crates/lang-rules/src/coverage.rs`
(valuetype rule per target language), no dedicated custom-marshal hook
in the codegen.

**Tests:** `crates/idl-lower/tests/lowering.rs::{the_other_valuetype_forms_are_unaffected_by_the_box,
an_abstract_valuetype_parses_as_a_valuetype,
a_forward_declared_valuetype_is_completed_by_its_definition}`,
`crates/corba-codegen/tests/orb_agreement.rs::the_valuebase_holder_is_the_omgs_and_jacorbs_names_a_class_that_does_not_exist`,
`crates/corba-codegen/tests/annex_a1_traits.rs::{table_a8_reads_abstract_and_truncatable_off_the_declaration,
table_a9_gives_a_valuebox_its_boxed_trait, every_traited_type_renders_in_every_column}`.

**Status:** done — valuetype IDL parses, lowers and carries an emission
rule per target language in `lang-rules`; a valuetype and a valuebox
carry the members Annex A.1.1 gives them in all four PSM columns.
Custom-marshal streams via
`crates/corba-ccm/src/orb_core.rs::StreamableValue` trait
(`marshal/unmarshal/repository_id`); sending-context runtime via
`SendingContext` with `code_set` + `truncation_codebase`.

### §10 Abstract Interface Semantics

**Spec:** Part 1 §10, p. 171 — `abstract interface`, marshalling as
Object or ValueType.

**Repo:** `crates/idl-parse/src/decl.rs` (`abstract` belongs to two
productions — the interface header (79) and `<value_abs_def>` (127);
the fork is in `parse_definition`), codegen corpus row in
`crates/corba-codegen/tests/orb-corpus/rows/03-abstractbase.idl`.

**Tests:** `crates/idl-lower/tests/lowering.rs::abstract_still_reaches_the_interface_header_it_also_belongs_to`,
`crates/idl-lower/tests/interface_as_type.rs::an_interface_used_as_a_type_resolves_in_every_spelling`
(the "an abstract interface" row).

**Status:** done — the parser accepts `abstract interface`, lowering
carries it as its own interface kind, and the codegen corpus holds the
row.

### §11 Dynamic Invocation Interface (DII)

**Spec:** Part 1 §11, p. 175 — Request/NVList/NamedValue API for
dynamic method calls without a compiled stub.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::{Request, NvList,
NamedValue, ArgFlag}` with `add_in_arg`/`add_out_arg` operations,
`ArgFlag::{In, Out, InOut}` (spec §11.1.2 wire values 1/2/3) and
NVList `add_value`/`count` (spec §11.1.3). `Request::invoke` keeps the
offset of the reply body in the reply message and the reply's byte
order with the result (`result_offset`, `result_endianness`,
`result_body`; Part 2 §9.4.3).

**Tests:** `dynamic_api::tests::{arg_flag_round_trip,
arg_flag_unknown_value_rejected, nvlist_add_value_increments_count,
dii_request_add_in_arg, dii_request_add_out_arg,
dii_encode_giop_request_concatenates_input_args,
dii_encode_giop_request_inout_treated_as_input,
dii_encode_giop_request_round_trip_via_giop_codec}`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_3_a_dii_result_of_another_orb_on_four_modulo_eight_is_read_at_its_offset`.

**Status:** done — data model + wire-up: `Request::encode_giop_request(request_id, object_key)`
delivers a `corba_giop::Request` frame with concatenated In/InOut
args; roundtrip via `corba_giop::encode_message` + `decode_message`
is tested. Spec §11.2 + §15.4.2.

### §12 Dynamic Skeleton Interface (DSI)

**Spec:** Part 1 §12, p. 191 — `ServerRequest` pseudo-object for
generic server implementations.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::ServerRequest` with
`set_result(value)`/`set_exception(id, value)` operations.

**Tests:** `dynamic_api::tests::{dsi_server_request_set_result,
dsi_server_request_set_exception, dsi_input_body_concatenates_in_and_inout,
dsi_servant_default_dispatch_via_input_body,
corba_p2_9_4_2_1_a_dsi_servant_reads_its_arguments_at_their_offset}`.

**Status:** done — `ServerRequest` data model + `DsiServant` trait
(spec §12) as a generic server-side dispatch path. Because of layer
separation (corba-ccm Layer 8.3 / corba-poa Layer 8.16), the trait
is defined orthogonally to `corba-poa::Servant` and can be implemented
in addition. `ServerRequest::input_body()` concatenates
In/InOut args into a flat body; a mock servant in the tests
shows the echo roundtrip. `dispatch_dsi` builds the `ServerRequest` with
`ServerRequest::received`: the offset of the body in the message
(`arguments_offset`) and its byte order (`endianness`);
`ServerRequest::input()` reads the arguments there (CORBA 3.3 Part 2
§9.4.2.1).

### §13 Dynamic Management of Any Values (DynAny)

**Spec:** Part 1 §13, p. 195 — DynAny API for iterating over `any`
contents without TypeCode-specific codegen.

**Repo:** `crates/corba-rt/src/dyn_any.rs::{DynAnyFactory, DynAny,
DynAnyError, InconsistentTypeCode, equivalent}` (re-exported in
`crates/corba-ccm/src/dynamic_api.rs`; the former data model of kind,
repository id and raw octets is removed);
`crates/corba-ccm/src/orb_core.rs::orb_init` (initial reference
`DynAnyFactory`).

**Tests:** `crates/corba-rt/src/dyn_any.rs::tests`:
`corba_p1_13_2_1_default_values_and_current_positions`,
`corba_p1_13_2_1_kinds_without_an_any_value_are_inconsistent`,
`corba_p1_13_2_2_8_insert_and_get_per_basic_type`,
`corba_p1_13_2_2_9_iteration_over_the_members_of_a_struct`,
`corba_p1_13_2_5_struct_members_get_and_set`,
`corba_p1_13_2_4_enum_as_string_and_ulong`,
`corba_p1_13_2_7_sequence_length_elements_and_bound`,
`corba_p1_13_2_2_from_any_assign_equal_and_copy`,
`corba_p1_13_2_7_a_recursive_sequence_member_builds_nested_values`,
`corba_p1_13_2_2_8_a_current_component_with_components_is_type_mismatch`,
`corba_p1_13_2_1_an_empty_struct_starts_at_0_an_empty_exception_has_no_components`;
`crates/corba-ccm/src/orb_core.rs::tests::corba_p1_8_5_1_orb_init_takes_its_arguments_out_of_the_list`
(`DynAnyFactory` through resolve_initial_references).

**Status:** partial — built and green on the Linux test host:
`DynAnyFactory` (`create_dyn_any`, `create_dyn_any_from_type_code` with
the default values of §13.2.1, `create_dyn_any_without_truncation`,
`create_multiple_dyn_anys`, `create_multiple_anys`, initial reference
`DynAnyFactory`); `DynAny` with `type`, `assign`, `from_any`, `to_any`,
`equal`, `copy`, `destroy`, `insert_*`/`get_*` for boolean, octet, char,
short, unsigned short, long, unsigned long, long long, unsigned long
long, float, double, wchar, string, wstring, any and `DynAny`,
`seek`/`rewind`/`next`/`component_count`/`current_component` with the
position rules of §13.2.1 and §13.2.2.9 (an empty struct starts at 0, an
empty exception at -1, `current_component` on an empty exception is
`TypeMismatch`), `insert_*`/`get_*` on a current component that itself
has components is `TypeMismatch` (§13.2.2.8); `DynEnum`, `DynStruct` (exceptions too),
`DynSequence` (length, bound, elements, recursive element types);
`TypeMismatch`, `InvalidValue`, `InconsistentTypeCode`. Open, although
`AnyValue` and `TypeCode` in `crates/corba-rt` now carry the kinds (§9.3;
`create_dyn_any_from_type_code` returns `InconsistentTypeCode` for them):
`DynFixed`, `DynUnion`, `DynArray`, `DynValueCommon`, `DynValue`,
`DynValueBox`, `insert_/get_reference`, `_typecode`, `_longdouble`,
`_val`, `_abstract` and the `insert_*_seq`/`get_*_seq` operations; an
`any` carries no string bound and no alias, so `to_any` gives the
TypeCode without them, and `tk_void` comes out as `tk_null` as in the
rest of the `any` model. Vendor proof (an `any` built with DynAny sent to
an omniORB servant and back) not run.

### §14 The Interface Repository

**Spec:** Part 1 §14, p. 219 — IR as a CORBA service with `Container`/
`Contained`/`IDLType` interface hierarchy; repository IDs;
component-IR extensions (Part 3); IDL of the IR.

**Repo:** `crates/corba-ir/src/repository.rs` (`Repository`,
`Container` hierarchy, `find_id` across the whole hierarchy,
`interface_is_a` as `InterfaceDef::is_a` after §14.5.24.1 over the base
interfaces), `crates/corba-ir/src/repository_id.rs`
(IDL/local-format repository IDs), `crates/corba-ir/src/type_code.rs`
(TypeCode backend), `crates/corba-ir/src/definition_kind.rs` (DK_*).

**Tests:** `crates/corba-ir/src/repository.rs::tests` (12 tests):
`scoped_lookup_descends_hierarchy`, `absolute_name_builds_scoped_path`,
`operation_def_carries_typed_signature`, `interface_def_details`,
`register_and_lookup_id`, `duplicate_id_yields_error`,
`lookup_name_at_top_level`, `nested_module_lookup_name_recurses`,
`describe_contents_filters_by_kind`, `definition_carries_type_code`,
`remove_deletes_definition`,
`corba_p1_14_5_24_1_is_a_follows_the_base_interfaces`;
`crates/corba-ir/src/definition_kind.rs::tests::well_known_definition_kinds_match_spec`,
`::at_least_26_kinds_modeled`;
`crates/corba-ir/src/repository_id.rs::tests::round_trip_omg_namingcontext`,
`::missing_idl_prefix_is_invalid`, `::missing_version_is_invalid`.

**Status:** done — IR interface hierarchy + RepositoryIds + TypeCodes
implemented. Component-IR extensions see Part 3 §12 (CCM
metamodel).

### §15 The Portable Object Adapter (POA)

**Spec:** Part 1 §15, p. 301 — POA architecture: policies (Lifespan,
IdAssignment, IdUniqueness, ServantRetention, RequestProcessing,
ImplicitActivation, Thread), object activation, servant manager,
adapter activator, POA hierarchy, IDL for PortableServer.

**Repo:** `crates/corba-poa/src/poa.rs` (hierarchy + dispatch),
`crates/corba-poa/src/policies.rs` (all 7 standard policies),
`crates/corba-poa/src/active_object_map.rs` (AOM),
`crates/corba-poa/src/servant.rs` + `crates/corba-poa/src/servant_manager.rs`,
`crates/corba-poa/src/object_id.rs`,
`crates/corba-poa/src/poa_manager.rs` (activation lifecycle).

**Tests:** `crates/corba-poa/src/poa.rs::tests` (12 tests):
`poa_hierarchy_create_find_destroy`, `default_root_dispatch_via_aom`,
`user_id_activation_with_explicit_oid`,
`use_default_servant_falls_back_to_default`,
`use_servant_manager_with_activator_caches_in_aom`,
`non_retain_with_locator_calls_pre_post_per_invocation`,
`dispatch_in_holding_state_is_rejected`,
`dispatch_in_inactive_state_yields_adapter_inactive`,
`dispatch_in_discarding_state_returns_obj_adapter`,
`invalid_policy_combination_is_rejected_at_construction`;
`crates/corba-poa/src/active_object_map.rs::tests` (6),
`crates/corba-poa/src/policies.rs::tests` (8),
`crates/corba-poa/src/poa_manager.rs::tests::full_state_cycle`,
`crates/corba-poa/src/object_id.rs::tests::system_id_is_8_bytes_big_endian`,
`crates/corba-poa/src/servant.rs::tests::is_a_checks_repository_ids`,
`crates/corba-poa/src/poa.rs::tests::corba_p2_9_4_2_1_a_servant_reads_a_long_long_at_the_body_offset`,
`crates/corba-poa/src/servant_manager.rs::tests::locator_preinvoke_returns_servant_and_cookie`.

**Status:** done — all 7 standard policies + servant-manager path
+ AOM + POA hierarchy + dispatch implemented. Adapter activator
(§15.3.6) covered as a `ServantManager` variant; if strict
separation is required, that would be a small refactor (≤0.5 PW).

### §16 Portable Interceptors

**Spec:** Part 1 §16, p. 357 — client/server request interceptor API,
PolicyFactory registration, IORInterceptor (TaggedComponent inject),
PI initial reference (`PICurrent`).

**Repo:** `crates/corba-ccm/src/portable_interceptor.rs::{InterceptionPoint,
ReplyStatus, InterceptorError, RequestException, RequestInfo,
ClientRequestInterceptor, ServerRequestInterceptor, InterceptorRegistry,
Outcome, PiCurrent, IorInfo, IorInterceptor, PolicyFactory, Policy,
PolicyError, OrbInitializer, OrbInitInfo, InitializerError,
RegistrationError, CurrentError, register_orb_initializer,
ServiceContextInjector}` (re-exported in `orb_extensions.rs`);
`crates/corba-ccm/src/orb_core.rs` (`orb_init` runs the ORB initializers,
`PICurrent` as an initial reference, `create_policy`, `destroy` calls
`Interceptor::destroy`); `crates/corba-interop/src/runtime.rs::{
IiopCorbaConnection::with_orb, CorbaServer::set_orb, intercepted_reply,
object_reference_for}` (request path over IIOP, IOR creation).

**Tests:** `crates/corba-ccm/tests/pi_service_contexts.rs`:
`corba_p1_16_4_7_starting_points_in_order_ending_points_in_reverse`,
`corba_p1_16_4_6_1_an_exception_in_send_request_pops_only_the_flow_stack`,
`corba_p1_16_4_6_1_forward_request_in_send_request_runs_receive_other`,
`corba_p1_16_4_6_3_an_exception_in_receive_reply_turns_the_rest_to_receive_exception`,
`corba_p1_16_4_6_4_receive_exception_may_change_the_exception_or_forward`,
`corba_p1_16_4_7_2_a_oneway_ends_in_receive_other_successful`,
`corba_p1_16_4_10_server_flow_with_an_exception_in_receive_request`,
`corba_p1_16_4_9_server_reply_and_forward_from_send_exception`,
`corba_p1_16_4_13_request_info_validity_and_service_context_rules`,
`corba_p1_16_9_2_a_duplicate_interceptor_name_is_refused`,
`corba_p1_16_4_ots_and_csiv2_service_contexts_route_through_interceptors`,
`corba_p1_16_4_12_exceptions_contexts_and_operation_context_follow_tables_16_1_and_16_2`,
`corba_p1_16_6_5_ior_info_checks_the_profile_id_and_the_policy_type`;
`crates/corba-ccm/src/orb_core.rs::tests::corba_p1_16_9_orb_init_info_rules_and_an_abandoned_initialization`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p1_16_9_2_5_orb_init_info_offers_the_codec_factory_in_pre_init`;
`crates/corba-interop/tests/portable_interceptors.rs::corba_p1_16_4_interceptors_run_in_the_iiop_request_path`;
`crates/corba-interop/tests/iop_cross_orb.rs::corba_p2_7_7_2_a_jacorb_vendor_context_reaches_the_zerodds_interceptor`.

**Status:** partial — built and green on the Linux test host: the ten
interception points with the flow-stack rules of §16.4.2, §16.4.7 and
§16.4.10 (starting points in registration order, ending points in
reverse only for interceptors whose starting point completed, change of
ending point after a system exception or ForwardRequest); `RequestInfo`
with the validity of Tables 16.1 and 16.2 (BAD_INV_ORDER minor 14),
`get_*_service_context` with BAD_PARAM minor 26, `add_*_service_context`
with BAD_INV_ORDER minor 15; `arguments`, `exceptions`, `contexts`,
`operation_context`, `result` as NO_RESOURCES minor 1 at the points
Tables 16.1 and 16.2 admit them, else BAD_INV_ORDER minor 14, as
§16.4.12.3 to §16.4.12.7 prescribe for environments without access to
them (this binding passes encoded CDR); PICurrent with thread and request
scope and the copies of §16.5.4.5: `receive_request_service_contexts`
runs in a thread scope of its own, the request scope is copied after it
and before `receive_request` into the thread scope of the servant,
`receive_request` runs in that thread scope (a `set_slot` on the request
scope does not reach the servant, one on PICurrent does), after the
servant the thread scope is copied back into the request scope, and the
thread scope of the connection thread applies again afterwards; IORInfo
with BAD_PARAM minor 29 for a profile id the adapter's references do not
carry, `get_effective_policy` with nil for a known policy type without a
policy and INV_POLICY minor 3 for an unknown one (known are the types of
the registered policy factories; the adapter of `CorbaServer` sets no
policies); ORBInitializer and ORBInitInfo (interceptors with
`DuplicateName`, `allocate_slot_id`, `register_policy_factory` with
BAD_INV_ORDER minor 16, `register_initial_reference` with the semantics
of `ORB::register_initial_reference`, so `InvalidName` for `PICurrent` and
`DynAnyFactory` too, `resolve_initial_references` only in `post_init`; in
`post_init` the `add_*_interceptor` operations and `allocate_slot_id`
raise BAD_INV_ORDER minor 26; while the initializers run, `get_slot` and
`set_slot` on PICurrent raise BAD_INV_ORDER minor 14 after §16.9.2.14 and
Annex A.6, where §16.5.3 names minor 10, which Annex A.6 gives to a DII
request sent twice; an error from `pre_init` or `post_init` abandons
`ORB_init`, the interceptors registered so far get `destroy`, and
`ORB_init` raises the system exception, or INITIALIZE for a user
exception); the request path over
IIOP in both directions: a service context from a client interceptor to a
server interceptor and a reply context back, a slot from a server
interceptor to the servant, a system exception from `receive_request`
without a servant call, ForwardRequest on server and client side with
the retry to the new target (LOCATION_FORWARD, at most 16 times); IOR
interceptors when a reference is made with `object_reference_for`. The
interceptor calls in `corba-iiop::Connection` are removed (they ran with
the operation name only and without reply information). Open:
`send_poll` and interceptors in `AmiClient`, `BiDirEndpoint` and
`AmhEndpoint`; IOR interceptors in `stringify_object_ref*`;
`IORInterceptor_3_0` and the ObjectReferenceTemplate (§16.6.3);
ServerRequestInfo `adapter_id`, `adapter_name`,
`target_most_derived_interface`, `target_is_a`, `get_server_policy`;
ClientRequestInfo `effective_profile`, `get_effective_component(s)`,
`get_request_policy`; ProcessingMode policy (§16.7).
`ORBInitInfo::codec_factory` (§16.9.2.5) returns the `CodecFactory` of
§7.8, already in `pre_init`. JacORB 3.9 on the Linux test host: a JacORB
client interceptor sends a request context to a ZeroDDS server
interceptor, whose reply context reaches the JacORB interceptor; the
other direction (ZeroDDS client, JacORB server) was not run.

### §17 CORBA Messaging

**Spec:** Part 1 §17, p. 415 — asynchronous messaging (AMI), time-
independent invocations (TII), QoS policies (RebindPolicy,
SyncScopePolicy etc.).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{MessagingPolicy,
AmiReplyHandler, AmiReplySink, dispatch_async_reply,
PersistentRequestStore, PersistentRequestEntry}` with all 10
policy types (Rebind/SyncScope/RequestPriority/ReplyPriority/Routing/
MaxHops/RequestTime/ReplyTime/RelativeRoundtripTimeout/
RoutingTypeRange) + wire mapping `policy_type() -> u32` to OMG
`Messaging.idl` (formal/2011-11-02 §B.5.1). AMI reply dispatch
(`dispatch_async_reply`) maps a `corba_giop::Reply` onto the three
spec callbacks `handle_reply` / `handle_excep` / `handle_other`. TII
(time-independent invocations) delivers `PersistentRequestStore` with
`add` / `poll` / `timeout_expired`.

**Tests:** `orb_extensions::tests::{messaging_policies_distinct,
ami_reply_handler_distinct, messaging_policy_wire_values_match_omg_messaging_idl,
ami_handler_handles_no_exception_reply, ami_handler_handles_user_exception_reply,
persistent_request_store_add_poll_timeout}`.

**Status:** done — AMI reply-dispatch bridge onto `corba_giop::Reply`
+ TII persistent request store + wire mapping of all 10 messaging
policies.

### §18 Compression

**Spec:** Part 1 §18, p. 491 — module `Compression`: `Compressor`,
`CompressorFactory`, `CompressionManager` (initial reference),
`CompressorId` constants, exceptions.

**Repo:** `crates/corba-ccm/src/compression.rs` — the ten
`COMPRESSORID_*` constants, `CompressionLevel`, `CompressionRatio`,
`CompressorIdLevel`(`List`), `Buffer`; `CompressionException`
(`reason`, `description`), `FactoryAlreadyRegistered`,
`UnknownCompressorId` with their repository ids; the traits `Compressor`
(`compress`, `decompress`, `compressor_factory`, `compression_level`,
`compressed_bytes`, `uncompressed_bytes`, `compression_ratio` =
`compressed_bytes / uncompressed_bytes`) and `CompressorFactory`
(`compressor_id`, `get_compressor`, the same instance per level,
`BAD_PARAM` minor 44 above level 9); `CompressionManager`
(`register_factory`, `unregister_factory`, `get_factory`,
`get_compressor`, `get_factories`). Built in: zlib (RFC 1950, required by
§18.1) and gzip (RFC 1952) on `flate2` (`FlateCompressorFactory`); level 0
writes stored blocks; `decompress` stops at a limit (default 16 MiB, the
largest GIOP message `corba-iiop` reads) with `Z_BUF_ERROR`. Every other
id (PKZIP, BZIP2, LZMA, LZO, RZIP, 7X, XAR or a vendor id) is served by a
registered factory of the user's; without one the manager raises
`UnknownCompressorId`. ORB:
`crates/corba-ccm/src/orb_core.rs::{Orb::compression_manager,
Orb::with_compression, orb_init}` — `orb_init` enters the manager as the
initial reference `CompressionManager`; `with_compression` takes a
`CompressorIdLevel` and refuses an id without a factory and a level
above 9. The former `CompressionAlgorithm` enumeration with its own
values 0 to 4 is removed.

**Tests:** `crates/corba-ccm/src/compression.rs::{corba_p1_18_5_compressor_ids_match_the_idl,
corba_p1_18_1_zlib_is_provided_by_default,
corba_p1_18_1_streams_of_the_reference_zlib_decompress,
corba_p1_18_2_every_level_of_the_built_in_compressors_round_trips,
corba_p1_18_2_6_the_counters_and_the_ratio_follow_compress,
corba_p1_18_3_2_the_same_level_gives_the_same_compressor_and_its_factory,
corba_p1_18_3_2_a_level_above_9_is_bad_param_44,
corba_p1_18_2_3_corrupt_input_is_a_compression_exception,
corba_p1_18_2_3_decompression_stops_at_the_limit,
corba_p1_18_4_register_get_and_unregister_a_custom_factory}`,
`crates/corba-ccm/src/orb_core.rs::tests::{corba_p1_8_5_1_orb_init_takes_its_arguments_out_of_the_list,
orb_vendor_config_compression_and_messaging_policies}`.

**Status:** done — every interface, constant and exception of §18.5
built, tests green on the Linux test host. Streams of the C library
zlib 1.3.1 decompress. An exchange with the compressors of a foreign ORB
(TAO, omniORB) runs over ZIOP and belongs to Part 2 §12.

---

## Part 2: Interoperability

### §1, §3–§5 Scope/References/Terms/Symbols

**Spec:** Part 2 §1, §3–§5, pp. 1-6 — scope, normative references, term
definitions, symbols.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — scope, reference lists and terms.

### §2 Conformance and Compliance — interoperability

**Spec:** Part 2 §2, p. 1 — "An ORB is considered to be
interoperability-compliant when it meets the following requirements":
the APIs for request-level bridges (Dynamic Invocation Interface,
Dynamic Skeleton Interface, the object identity operations) and IIOP
(GIOP transfer syntax and message formats over TCP/IP). ZIOP is an
optional compliance point (item §12).

**Repo:** `crates/corba-ccm/src/dynamic_api.rs` (DII, DSI),
`crates/corba-giop/src/object_ops.rs` (`_is_a`, `_non_existent`,
`_interface`, `_repository_id`, `_domain_managers`, `_component`),
`crates/corba-iiop/` (IIOP).

**Tests:** `crates/corba-ccm/src/dynamic_api.rs::tests::dii_encode_giop_request_round_trip_via_giop_codec`,
`crates/corba-ccm/src/dynamic_api.rs::tests::dsi_servant_default_dispatch_via_input_body`,
`crates/corba-giop/src/object_ops.rs::tests::is_object_operation_recognizes_all_six_and_the_legacy_spelling`,
`crates/corba-iiop/src/connection.rs::tests::round_trip_request_reply`.

**Status:** done

### §2.1 MIOP — extension of the IOP module

**Spec:** Part 2 §2.1, p. 2 — `const ProfileId TAG_UIPMC = 3; const
ComponentId TAG_GROUP = 39; const ComponentId TAG_GROUP_IIOP = 40`.

**Repo:** `crates/corba-ior/src/profile_tags.rs` (`ProfileId::Uipmc`),
`crates/corba-ior/src/component_tags.rs` (`ComponentId::Group`,
`ComponentId::GroupIiop`).

**Tests:** `crates/corba-ior/src/profile_tags.rs::tests::standard_profile_tag_values_match_spec`,
`crates/corba-ior/src/component_tags.rs::tests::round_trip_all_known_tags`.

**Status:** done

### §6 Interoperability Overview

**Spec:** Part 2 §6, p. 7 — architecture overview of ORB interop.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — overview; the requirements are in
§7–§12.

### §7.1–§7.5 Domains, ORB services, bridging, object addressing

**Spec:** Part 2 §7.1–§7.5, pp. 15–24 — domains, ORB services, bridging
between domains, interoperability approaches, domain-relative object
references.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — architecture description; the
fulfillable statements of §7 are in §7.6–§7.10.

### §7.6 Interoperable Object References (IOR)

**Spec:** Part 2 §7.6, pp. 25–37 — IOR structure, `TaggedProfile`,
standard profiles, `TaggedComponent`, standard components, composition,
stringified IOR (`IOR:` + hex of the CDR encapsulation; §7.6.9: "The
case for characters in a stringified IOR is not significant"), object
URLs `corbaloc` (`iiop`, `rir`, default port 2809, default version 1.0, IPv6
host in square brackets) and `corbaname`, the optional schemes
`file://`, `ftp://` and `http://` (Table 7.1); failed conversion:
`BAD_PARAM` with minor code 7 to 10. §7.6.3: Full IOR conformance (all
profiles kept in their order); when an ORB cannot use any profile of an
IOR, `TRANSIENT` minor 2 or `IMP_LIMIT` minor 1. §7.6.6.2: the addresses
of `TAG_ALTERNATE_IIOP_ADDRESS` components beside the address of the
profile.

**Repo:** `crates/corba-ior/src/ior.rs`, `crates/corba-ior/src/tagged_profile.rs`,
`crates/corba-ior/src/profile_tags.rs`, `crates/corba-ior/src/components.rs`,
`crates/corba-ior/src/component_tags.rs`, `crates/corba-ior/src/stringified.rs`,
`crates/corba-ior/src/url.rs`; `crates/corba-rt/src/object.rs`
(`ObjectReference::profiles`, `RawProfile`, `from_profiles`);
`crates/corba-ccm/src/orb_core.rs::Orb::{string_to_object, object_to_string}`,
`crates/corba-ccm/src/object_url.rs` (`file`, `http`, `ftp`);
`crates/corba-interop/src/runtime.rs`
(`IiopCorbaConnection::connect_reference`, `AmiClient::connect`,
`object_reference_from_ior`, `stringify_object_reference`);
`crates/corba-dds-bridge/src/bridge_daemon/config.rs::resolve_target`.

**Tests:** `crates/corba-ior/src/ior.rs::tests::round_trip_multi_profile_le`,
`crates/corba-ior/src/ior.rs::tests::nil_ior_is_recognized`,
`crates/corba-ior/src/tagged_profile.rs::tests::unknown_profile_tag_round_trips_as_other`,
`crates/corba-ior/src/component_tags.rs::tests::well_known_component_tag_values_match_spec`,
`crates/corba-ior/src/stringified.rs::tests::round_trip_be`,
`crates/corba-ior/src/stringified.rs::tests::corba_p2_7_6_9_the_prefix_case_is_not_significant`,
`crates/corba-ior/src/url.rs::tests::default_port_2809_when_missing`,
`crates/corba-ior/src/url.rs::tests::rir_endpoint`,
`crates/corba-ior/src/url.rs::tests::corbaname_with_fragment`,
`crates/corba-rt/src/object.rs::tests::corba_p2_7_6_3_every_profile_is_kept_in_its_order`,
`crates/corba-rt/src/object.rs::tests::corba_p2_7_6_3_a_reference_without_an_iiop_profile_keeps_its_profile`,
`crates/corba-rt/src/any.rs::tests::corba_p2_9_3_6_object_references_in_an_any_keep_their_profiles_or_are_refused`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p1_8_2_2_string_to_object_of_object_to_string_is_the_same_reference`,
`crates/corba-interop/tests/ior_profiles.rs::{corba_p2_7_6_9_object_reference_from_ior_ignores_the_prefix_case,
corba_p2_7_6_6_2_client_falls_back_to_an_alternate_address,
corba_p2_7_6_3_client_falls_back_to_the_next_profile,
corba_p2_9_4_2_1_reference_addr_names_the_profile_the_request_used,
corba_p2_7_6_3_no_usable_profile_is_transient_2,
corba_p2_7_6_3_a_reference_without_an_iiop_profile_is_imp_limit_1}`,
`crates/corba-dds-bridge/src/bridge_daemon/ior.rs::tests::corbabridge_4_4_corbaloc_and_ior_targets_resolve`;
the object URLs in item §7.6.10.

**Status:** partial — built and green on the Linux test host: IOR,
profiles, components and stringified IOR; the prefix and the hex digits
of a stringified IOR are read in any case (`ior:`, `Ior:`;
`from_stringified`, `object_reference_from_ior`, `string_to_object` and
the bridge's `targets`), text without the `IOR:` scheme is BAD_PARAM
minor 7, hex that does not decode minor 9; a reference keeps every
profile of its IOR in its order and writes it back unchanged, also a
single IIOP profile with empty `profile_data` and also in an `any` (Full
IOR conformance); nil is only a reference without a type id and without
a profile (§7.6.2); the client (`IiopCorbaConnection` for
requests and `locate`, `AmiClient`) tries the IIOP profiles in the order
of the IOR and in each first the address of the profile, then the
addresses of `TAG_ALTERNATE_IIOP_ADDRESS`; when none is reachable,
TRANSIENT minor 2, when the reference has no readable IIOP profile,
IMP_LIMIT minor 1, both `COMPLETED_NO`; `ReferenceAddr` carries the
whole IOR and the index of the profile used. An SSLIOP profile is
reached at its host and SSL port, a UIOP profile at its socket path,
without alternate addresses. Object URLs: item §7.6.10 (done). Open: the
optional component `TAG_ORB_TYPE` in the IORs of ZeroDDS, item §7.6.6.1
(blocked).

### §7.6.6.1 TAG_ORB_TYPE

**Spec:** Part 2 §7.6.6.1, p. 30 — `TAG_ORB_TYPE` carries, as the CDR
encapsulation of an `unsigned long`, "an ORB type ID allocated by the OMG
for the ORB type of the originating ORB"; at most once per profile,
optional in profiles of IIOP 1.1 and later.

**Repo:** `crates/corba-ior/src/components.rs::OrbType` (reads and
writes the component).

**Tests:** —

**Status:** blocked — assignment of an ORB type by the OMG (requested,
as in `zerodds-corba-bridge-1.0` §4.1a); without the assigned value
ZeroDDS does not write the component, any other value would be the
identity of another ORB. Lifted once the OMG assigns the value: then a
constant in `crates/corba-ior/src/components.rs` and the component once
per IIOP profile of 1.1 and later in the IORs of
`crates/corba-interop/src/runtime.rs` and
`crates/corba-dds-bridge/src/bridge_daemon/ior.rs`.

### §7.6.10 Object URLs — IPv6 without port, `rir` exclusion, minor codes

**Spec:** Part 2 §7.6.10, pp. 33–37 — `corbaloc` after §7.6.10.1:
`<obj_addr_list>["/"<key_string>]`, `<key_string>` with the escapes of
RFC 2396; `<host> = DNS_style_host_name | IPv4_address | "[" IPv6_address
"]"`, without a host the local host, port optional with default 2809,
without a version 1.0; `rir` without an address, "The rir protocol
cannot be used with any other protocol in a URL", an empty key is
`NameService` (§7.6.10.2); `corbaname` with the default key
`NameService` (§7.6.10.5); a further protocol (`<future_prot_addr>`,
§7.6.10.6); `file://`, `ftp://`, `http://` (Table 7.1, optional); "If
conversion fails, string_to_object raises a BAD_PARAM exception" with
minor code 7 (bad scheme name), 8 (bad address), 9 (bad schema specific
part), 10 (non-specific reason).

**Repo:** `crates/corba-ior/src/url.rs` (`parse_corbaloc`,
`parse_corbaname`, `parse_iiop_address`, `CorbalocEndpoint`,
`escape_key_string`), `crates/corba-ior/src/error.rs::IorError::bad_param_minor`;
`crates/corba-ccm/src/orb_core.rs::Orb::string_to_object`
(`corbaloc_to_object`, `corbaname_to_object`, `validate_init_ref_url`),
`crates/corba-ccm/src/object_url.rs` (`fetch`, `FetchUrl`);
`crates/corba-interop/competitors/omniorb/url_minor.cc`.

**Tests:** `crates/corba-ior/src/url.rs::tests::{corba_p2_7_6_10_every_example_url_of_the_clause_parses,
corba_p2_7_6_10_corbaloc_without_key_string_has_an_empty_key,
corba_p2_7_6_10_6_a_future_protocol_address_is_kept,
corba_p2_7_6_10_2_rir_with_an_empty_key_is_nameservice,
corba_p2_7_6_10_3_an_empty_iiop_address_is_the_local_host,
corba_p2_7_6_10_ipv6_host_without_port_takes_2809,
corba_p2_7_6_10_ipv6_host_with_port,
corba_p2_7_6_10_rir_with_another_protocol_is_refused,
corba_p2_7_6_10_failures_carry_bad_param_minor_7_to_10,
corba_p2_7_6_10_5_the_stringified_name_is_unescaped,
corba_p2_7_6_10_1_only_the_listed_characters_stay_unescaped}`;
`crates/corba-ccm/src/orb_core.rs::tests::{corba_p1_8_2_2_string_to_object_raises_the_bad_param_minor_codes,
corba_p1_8_5_3_2_an_init_ref_url_is_checked_and_corbaloc_rir_refused,
corba_p2_7_6_10_corbaloc_addresses_become_profiles_in_their_order,
corba_p2_7_6_10_2_an_empty_rir_key_resolves_the_nameservice,
corba_p2_7_6_10_7_a_file_url_yields_the_reference_in_the_file}`;
`crates/corba-ccm/src/object_url.rs::tests::{corba_p2_7_6_10_7_the_three_url_forms_parse,
corba_p2_7_6_10_7_bad_urls_carry_their_minor_code,
corba_p2_7_6_10_7_a_missing_file_is_minor_10,
corba_p2_7_6_10_7_a_redirect_is_resolved_against_the_request_url,
corba_p2_7_6_10_7_a_chunked_body_is_read_within_its_limit}`;
`crates/corba-interop/tests/ior_profiles.rs::{corba_p2_7_6_10_corbaloc_with_two_iiop_1_0_addresses_reaches_the_second,
corba_p2_7_6_10_an_ipv6_corbaloc_reaches_the_object,
corba_p2_7_6_10_7_an_http_url_yields_the_ior,
corba_p2_7_6_10_7_an_ftp_url_yields_the_ior}`;
`crates/corba-interop/tests/omniorb_cross_orb.rs::{corba_p2_7_6_10_4_an_omniorb_client_reaches_a_zerodds_server_by_corbaloc,
corba_p2_7_6_10_5_zerodds_resolves_corbaname_urls_through_omninames,
corba_p2_7_6_10_minor_codes_agree_with_omniorb}`.

**Status:** done — built and green on the Linux test host: the grammar
of §7.6.10.1 to §7.6.10.3 with every example URL of the clause (key
optional, IPv6 host with and without port, without a host the local
host, port 1 to 65535 in digits only, `rir` alone and without an
address, an empty `rir` key and a `corbaname` without a key are
`NameService`, the name of a `corbaname` is unescaped, a further
protocol such as `atm:` is kept with its address and skipped on
conversion); a key is written with `%XX` except for alphanumerics and
the characters the list of §7.6.10.1 names (`.` is not in it and is
escaped); `corbaloc` gives one IIOP profile per address in the order of
the URL; `file://` (own host only), `http://` (HTTP/1.1 `GET`,
`Content-Length`, chunked; path and query only of the characters of RFC
3986, otherwise minor 9; a redirect resolves `Location` against the
request URL, RFC 3986 §5.2, and leads only to an `http` URL, otherwise
minor 10) and `ftp://` (login, `TYPE I`, with `;type=a` `TYPE A`,
`;type=d` and other type codes minor 9, `CWD` per directory, `EPSV` or
`PASV`, `RETR`) yield text that is converted again, at most 8 levels;
when that text is empty, not UTF-8 or has no scheme the ORB converts,
minor 9; BAD_PARAM minor 7 (scheme), 8 (address, also
`rir` beside another address, `file` of another host, no address of a
protocol of this ORB), 9 (IOR, key or name not readable), 10 (`rir` or
`corbaname` finds nothing, file or server not readable). Against
omniORB 4.3.3 (weekly job `integration-bridges-vendor-corba`, run on the
Linux test host): an omniORB client reaches a ZeroDDS server over
`corbaloc` (IIOP 1.0, 1.2, a dead address ahead of the live one);
ZeroDDS resolves `corbaname` URLs through omniNames; for ten URLs both
report the same. omniORB deviates from the clause on other URLs (no
`corbaloc` without `/`, `atm:` as a bad scheme, minor 10 for escapes in
the key and for `rir` beside an address, port 0 and a non-IPv6 host in
brackets accepted); the test does not compare those URLs. The `http`
and `ftp` tests run against servers inside the test, not against a
third-party HTTP or FTP server.

### §7.7 Service Context

**Spec:** Part 2 §7.7, pp. 37–40 — `IOP::ServiceContext` (`context_id`,
`context_data` as a CDR encapsulation), standard IDs (§7.7.1),
processing rules (§7.7.2): a context outside the own GIOP version or
outside the OMG range may be ignored, but "must be passed on through a
bridge and must be made available to interceptors. No exception shall
be raised."

**Repo:** `crates/corba-giop/src/service_context.rs` (`ServiceContext`,
`ServiceContextList` with `get` and `processable`, `ServiceContextTag`
with the 22 IDs of §7.7.1 and `support`, `is_omg_range`, `is_valid_for`);
`crates/corba-interop/src/runtime.rs` (`CorbaServer` hands every received
context to the server interceptors and to `on_request_contexts`,
`IiopCorbaConnection::service_contexts` after Table 7.2,
`merge_contexts`, `IiopCorbaConnection::invoke_passing_contexts`);
`crates/corba-interop/src/miop.rs` (`MiopGateway::forward`).

**Tests:** `crates/corba-giop/src/service_context.rs::tests::corba_p2_7_7_1_all_standard_service_ids_match_the_idl`,
`crates/corba-giop/src/service_context.rs::tests::corba_p2_7_7_omg_range_is_vscid_0_to_15`,
`crates/corba-giop/src/service_context.rs::tests::corba_p2_7_7_2_contexts_follow_table_7_2_per_version`,
`crates/corba-giop/src/service_context.rs::tests::corba_p2_7_7_2_processable_keeps_the_valid_contexts_and_the_list_whole`,
`crates/corba-giop/src/service_context.rs::tests::standard_tag_values_match_spec`,
`crates/corba-giop/src/service_context.rs::tests::round_trip_with_two_entries_be`,
`crates/corba-giop/src/service_context.rs::tests::round_trip_le_byte_order`,
`crates/corba-interop/tests/service_contexts.rs::corba_p2_7_7_2_foreign_contexts_reach_the_server_interceptor_without_exception`,
`crates/corba-interop/tests/service_contexts.rs::corba_p2_7_10_2_6_codesets_context_precedes_the_sas_context`,
`crates/corba-interop/tests/miop.rs::corba_p2_7_7_2_the_gateway_passes_service_contexts_on_and_back`,
`crates/corba-interop/tests/miop.rs::corba_p2_7_7_2_the_gateway_passes_two_contexts_of_one_id`,
`crates/corba-interop/tests/iop_cross_orb.rs::corba_p2_7_7_2_a_jacorb_vendor_context_reaches_the_zerodds_interceptor`.

**Status:** partial — built and green on the Linux test host: every ID
of §7.7.1 (0 to 21), the OMG range (VSCID 0 to 15, IDs 0 to 4095) and
the validity of a context per GIOP version after Table 7.2; a context
outside the OMG range, the CodeSets context in a GIOP 1.0 request,
`RMICustomMaxStreamFormat` (GIOP 1.4 only) and `FIREWALL_PATH_RESP` (no
row in Table 7.2) reach the server interceptor with their data in GIOP
1.0, 1.1 and 1.2, without an exception, and the reply comes; a context a
client interceptor puts in place of one of the ORB's keeps its position,
the CodeSets context stays ahead of the SAS context (§7.10.2.6); the
MIOP gateway passes the contexts of an object operation on to the object
behind the group's IIOP profile and its reply contexts back to the
client, two contexts with one id both and in their order. JacORB 3.9 on
the Linux test host: a JacORB client interceptor
sends the vendor ID 0x4A430001, the ZeroDDS server interceptor sees it,
the call succeeds, the reply context of the ZeroDDS interceptor reaches
the JacORB interceptor. Open: the CORBA/DDS bridge daemon
(`zerodds-corba-bridge-1.0` §4.3) does not pass the contexts of a request
on to the DDS side, and the notify path (§4.4) sends none; the DDS sample
format of §4.3 has no place for them.

### §7.8 Coder/Decoder Interfaces

**Spec:** Part 2 §7.8, pp. 40–43 — `IOP::Codec` (`encode`, `decode`,
`encode_value`, `decode_value`) and `IOP::CodecFactory::create_codec`,
reachable via `resolve_initial_references("CodecFactory")`;
"The encodings which shall be supported are: ENCODING_CDR_ENCAPS,
version 1.0; [...] 1.1; [...] 1.2; [...] for all future versions of
GIOP as they arise".

**Repo:** `crates/corba-ccm/src/iop_codec.rs` (`Codec`, `CodecFactory`,
`Encoding`, `ENCODING_CDR_ENCAPS`, `CodecError`, `UnknownEncoding`);
`crates/corba-rt/src/any.rs` (`CorbaAny::encode_value`,
`CorbaAny::decode_value`); `crates/corba-ccm/src/orb_core.rs` (initial
reference `CodecFactory`); `crates/corba-ccm/src/portable_interceptor.rs`
(`OrbInitInfo::codec_factory`).

**Tests:** `crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_encode_carries_typecode_and_value`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_encode_value_carries_the_value_only`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_decode_reads_both_byte_orders`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_wstring_under_cdr_encaps_1_0_is_invalid_type_for_encoding`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_value_types_need_cdr_encaps_1_2`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_component_typecodes_need_cdr_encaps_1_3`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_decode_of_garbage_is_format_mismatch`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_decode_value_with_a_foreign_typecode_is_type_mismatch`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_1_union_and_sequence_round_trip`,
`crates/corba-ccm/src/iop_codec.rs::tests::corba_p2_7_8_2_factory_supports_cdr_encaps_1_0_to_1_4_and_refuses_others`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p2_7_8_codec_factory_is_an_initial_reference`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p1_16_9_2_5_orb_init_info_offers_the_codec_factory_in_pre_init`,
`crates/corba-interop/tests/iop_cross_orb.rs::corba_p2_7_8_codec_octets_agree_with_jacorb`.

**Status:** partial — built and green on the Linux test host: the four
operations with `InvalidTypeForEncoding`, `FormatMismatch` and
`TypeMismatch`, `create_codec` with `UnknownEncoding`,
`ENCODING_CDR_ENCAPS` 1.0 to 1.4; a type Table 7.2 ties to a later GIOP
version (`wstring`, `wchar`, `long long`, `fixed` before 1.1, value types
before 1.2, `tk_component`/`tk_home`/`tk_event` before 1.3) is invalid
for the encoding, also in a sequence, a struct, a nested `any` and as a
TypeCode value; `wchar` and `wstring` in their GIOP 1.2 form (§9.3.1.6,
§9.3.2.7: encapsulations assume GIOP 1.2 CDR); the encapsulation is
big-endian and read in both byte orders; the initial reference
`CodecFactory` and `ORBInitInfo::codec_factory` already in `pre_init`.
JacORB 3.9 on the Linux test host: `encode` and `encode_value` under 1.2
agree octet for octet for a struct, a sequence, a union, a `wstring` and a
`long long`, ZeroDDS decodes JacORB's octets; both raise
`UnknownEncoding` for another format; JacORB's 1.0 codec encodes a
`wstring` where §7.8.1.1 asks for `InvalidTypeForEncoding`, which ZeroDDS
raises (measured). TAO is not on the test host, omniORB not checked.
Open: an `any` with `tk_alias` keeps only the content type when decoded
(`AnyValue` has no alias variant), so `decode` returns an `any` with
another TypeCode than the octets carry.

### §7.9 Feature Support and GIOP Versions

**Spec:** Part 2 §7.9, pp. 43–44, Table 7.2 — which service contexts,
IOR components and features apply from which GIOP version 1.0 to 1.4.

**Repo:** `crates/corba-giop/src/feature.rs` (`Feature`, `Support`,
`support`, `Feature::of_tckind`, `Feature::of_operation`);
`crates/corba-giop/src/service_context.rs` (`ServiceContextTag::support`,
`is_valid_for`); `crates/corba-ior/src/component_tags.rs`
(`ComponentId::feature`, `ComponentId::support`);
`crates/corba-interop/src/runtime.rs` (`IiopCorbaConnection::service_contexts`,
reply contexts of `CorbaServer`); `crates/corba-ccm/src/iop_codec.rs`
(types per encoding); `crates/corba-giop/src/version.rs` (`Version::V1_0`,
`V1_1`, `V1_2`, `supports_fragments`, `uses_flags_octet`,
`uses_v1_2_request_layout`, `supports_bidirectional`);
`crates/corba-iiop/src/profile_body.rs` (components from IIOP 1.1; an
IIOP 1.0 profile with components is refused with
`CdrError::ComponentsNotInVersion`).

**Tests:** `crates/corba-giop/src/feature.rs::tests::corba_p2_7_9_table_7_2_every_row_and_column`,
`crates/corba-giop/src/feature.rs::tests::corba_p2_7_9_codesets_context_is_not_valid_in_giop_1_0`,
`crates/corba-giop/src/feature.rs::tests::corba_p2_7_9_alternate_iiop_address_needs_iiop_1_2`,
`crates/corba-giop/src/feature.rs::tests::corba_p2_7_9_versions_after_1_4_take_the_1_4_column`,
`crates/corba-giop/src/feature.rs::tests::corba_p2_7_9_tckinds_and_operations_map_to_their_rows`,
`crates/corba-ior/src/component_tags.rs::tests::corba_p2_7_9_component_tags_follow_table_7_2`,
`crates/corba-interop/tests/service_contexts.rs::corba_p2_7_9_a_1_0_request_carries_no_codesets_context`,
`crates/corba-interop/tests/service_contexts.rs::corba_p2_7_9_credentials_are_not_dropped_below_giop_1_2`,
`crates/corba-interop/tests/ziop.rs::corba_p2_7_9_the_client_sends_invocation_policies_from_giop_1_2_only`,
`crates/corba-interop/tests/ziop.rs::corba_p2_7_9_the_server_sends_invocation_policies_in_giop_1_2_replies_only`,
`crates/corba-giop/src/version.rs::tests::flags_octet_replaces_byte_order_at_1_1`,
`crates/corba-giop/src/version.rs::tests::v1_2_request_layout_only_in_1_2_plus`,
`crates/corba-iiop/src/profile_body.rs::tests::components_only_from_iiop_1_1`,
`crates/corba-iiop/src/profile_body.rs::tests::corba_p2_7_9_iiop_1_0_profile_with_components_is_refused`.

**Status:** done — built and green on the Linux test host: Table 7.2 as
data, every row and every column V1.0 to V1.4 checked against the text
("optional$"/"optional$$" as optional, the Realtime rows optional,
versions after 1.4 with the 1.4 column); the client sends its own
contexts only in the versions Table 7.2 names for them (CodeSets from
1.1, INVOCATION_POLICIES from 1.2), and refuses a request with CSIv2
credentials under GIOP 1.0/1.1 with NO_PERMISSION instead of sending it
without them; the server sends INVOCATION_POLICIES only in GIOP 1.2
replies; the codec checks the types per encoding (§7.8). Received
contexts, components and implicit operations of other versions are not
refused (§7.7.2); the client also sends `_component` and
`_repository_id` over an IIOP 1.2 profile, because the MIOP object
operations (§11.2.6.10, §11.3.2; Table 7.2: GIOP 1.4 optional) use them
there, tested in the MIOP and MGM tests of `zerodds-corba-interop`.

### §7.9 GIOP 1.3 and 1.4

**Spec:** Part 2 §7.9, pp. 43–44, Table 7.2 — columns V1.3 and V1.4;
§9.4 (the 1.3 headers as typedefs of the 1.2 forms), §9.4.1
(MessageError in the highest version, then close), §9.3.5.1.6
(indirection between top-level TypeCodes from 1.3), §9.3.1.6 (UCS-2/UCS-4
under 1.4), §9.7, §9.7.2 (IIOP 1.4, profile version).

**Repo:** `crates/corba-giop/src/version.rs` (`Version::V1_3`, `V1_4`,
`MAX`, `is_supported`, `supports_component_operation`,
`supports_repository_id_operation`,
`allows_indirection_between_top_level_typecodes`, `allows_ipv6_in_ior`,
`ucs_uses_utf16_byte_order`); `crates/corba-giop/src/header.rs`
(header 1.0 to 1.4); `crates/corba-iiop/src/profile_body.rs`
(`IiopVersion::V1_4`, `MAX`, `allows_ipv6_host`, `IiopVersion::for_host`,
`IiopProfileBody::for_endpoint`, `unbracketed_host`);
`crates/corba-iiop/src/connector.rs` (IPv6 host with and without brackets);
`crates/corba-cosnaming/src/graph.rs` (`ior_for_object_key`),
`crates/corba-cosnaming/src/servant.rs`
(`NamingServant::with_highest_version`),
`crates/corba-cosnaming/src/naming_daemon.rs` (reply in the version of
the request up to 1.4);
`crates/corba-rt/src/type_code.rs` (`TopLevelTypeCodes`,
`TypeCode::decode_top_level`); `crates/corba-rt/src/any.rs`
(`CorbaAny::decode_in_message`); `crates/corba-interop/src/runtime.rs`
(reply in the version of the request up to 1.4, `refuse_message`,
`DEFAULT_PROFILE_VERSION`, `CorbaServer::set_profile_version`,
`CorbaServer::set_component_reference`, `reference_with_components`).

**Tests:** `crates/corba-giop/src/version.rs::tests::corba_p2_7_9_giop_1_3_and_1_4_take_the_1_2_formats`,
`crates/corba-giop/src/version.rs::tests::corba_p2_7_9_features_of_1_3_and_1_4`,
`crates/corba-giop/src/header.rs::tests::corba_p2_7_9_giop_1_3_and_1_4_headers_are_the_1_2_headers`,
`crates/corba-giop/src/codec.rs::tests::corba_p2_7_9_every_message_round_trips_under_giop_1_3_and_1_4`,
`crates/corba-iiop/src/profile_body.rs::tests::corba_p2_7_9_ipv6_host_in_a_1_4_profile`,
`crates/corba-iiop/src/profile_body.rs::tests::corba_p2_7_9_an_endpoint_profile_for_an_ipv6_host_is_1_4_without_brackets`,
`crates/corba-iiop/src/connector.rs::tests::corba_p2_7_9_an_ipv6_profile_host_is_dialed_with_and_without_brackets`,
`crates/corba-cosnaming/src/graph.rs::tests::corba_p2_7_9_a_naming_reference_for_an_ipv6_host_is_iiop_1_4`,
`crates/corba-cosnaming/src/servant.rs::tests::corba_p2_7_9_a_servant_names_no_profile_version_above_its_server`,
`crates/corba-cosnaming/tests/naming_svc_e2e.rs::corba_p2_7_9_an_ipv6_daemon_publishes_1_4_profiles_and_replies_in_1_4`,
`crates/corba-rt/src/type_code.rs::tests::corba_p2_9_3_5_indirection_between_top_level_typecodes_under_1_3`,
`crates/corba-rt/src/any.rs::tests::corba_p2_9_3_5_a_second_any_indirects_to_the_first_typecode_under_1_3`,
`crates/corba-rt/src/type_code.rs::tests::corba_p2_9_3_5_every_tckind_round_trips`,
`crates/corba-interop/tests/giop_version.rs::server_replies_in_giop_1_3`,
`crates/corba-interop/tests/giop_version.rs::server_replies_in_giop_1_4`,
`crates/corba-interop/tests/giop_version.rs::corba_p2_9_4_1_higher_minor_gets_message_error_in_1_4_then_close`,
`crates/corba-interop/tests/giop_version.rs::corba_p2_9_7_the_client_sends_giop_1_4_to_a_1_4_profile`,
`crates/corba-interop/tests/giop_version.rs::corba_p2_9_7_2_the_profile_version_is_1_2_unless_set`,
`crates/corba-interop/tests/giop_version.rs::corba_p2_7_9_component_operation_answers_the_component_reference`,
`crates/corba-interop/tests/giop_version.rs::corba_p2_7_9_ipv6_host_in_a_1_4_profile_is_dialed`,
`crates/corba-interop/tests/omniorb_cross_orb.rs::corba_p2_9_7_2_an_omniorb_client_calls_through_a_1_4_profile_and_refuses_one_in_a_reply`,
`crates/corba-interop/tests/jacorb_cross_orb.rs::corba_p2_9_7_2_a_jacorb_client_takes_a_1_2_profile_and_refuses_a_1_4_profile`.

**Status:** partial — built and green on the Linux test host: GIOP 1.3
and 1.4 with the 1.2 formats in both directions, every message type; the
server answers in the version of the request, a higher minor version
with MessageError 1.4, and closes the connection; profiles IIOP 1.1 to
1.4 can be set, a ZeroDDS client speaks GIOP 1.4 through a 1.4 profile;
`_component` answers with the component reference of a facet or
component, else nil; `_repository_id` (1.4) is answered by `object_ops`;
tk_component, tk_home, tk_event; the server of `corba-interop`
(`CorbaServer` and the reference builders in `runtime.rs`) and the
naming daemon write an IPv6 host into a 1.4 profile only, as the address
without the brackets of the URL form (`[::1]` becomes `::1`); the naming
daemon replies in the version of the request up to 1.4; the client dials
the host with and without brackets; an indirection from a top-level
TypeCode to an earlier one of the same message from 1.3 through
`TypeCode::decode_top_level` and `CorbaAny::decode_in_message`, refused
below 1.3. The profiles carry 1.2 by default: measured, JacORB 3.9 does
not call through an IIOP 1.4 profile (`MARSHAL` "Unknown GIOP minor:
4"), omniORB 4.3.3 calls through one and raises `INV_OBJREF` when a
reply carries a reference with a 1.4 profile; §9.7.2 asks both to accept
any minor version. Missing: (1) the generated stubs and skeletons and
the MIOP group management (`crates/corba-interop/src/mgm.rs`,
`read_properties` and the exceptions `InvalidProperty`,
`InvalidCriteria`, `CannotMeetCriteria`) read the TypeCode of each `any`
on its own (`CorbaAny::decode`); an indirection between top-level
TypeCodes in a message to them is MARSHAL; `TypeCode::decode_top_level`
and `CorbaAny::decode_in_message` have no caller outside the tests,
since skeleton and stub are not told the GIOP version of the message
(`AlignedBody`, `CorbaConnection::invoke`); (2) UCS-2/UCS-4 with the
byte order of UTF-16 under 1.4 and IANA code set ids in the CodeSets
context under 1.4 (§7.10.2.5): no TCS-W other than UTF-16 is negotiated,
item "§7.10 Conversion into a negotiated code set other than
UTF-8/UTF-16"; (3) the CORBA/DDS bridge daemon
(zerodds-corba-bridge-1.0) reads GIOP 1.0 to 1.2 and writes an IPv6 host
into a profile of the configured version `corba.iiop.giop_version` (1.0
to 1.2), the references of the naming service it hosts
(`corba.naming.serve`) into a 1.2 profile; a 1.4 profile needs it to
serve messages in 1.4 (read from the code, not run). No vendor ORB with
GIOP 1.3 or 1.4 is known; the 1.3/1.4 formats are checked against the
own codec only.

### §7.10 Code Set Conversion

**Spec:** Part 2 §7.10, pp. 45–58 — code set component of the IOR
(`TAG_CODE_SETS`), negotiation of the transmission code set for `char`
and `wchar`, `CodeSets` service context, OSF registry IDs, conversion of
character data into the negotiated code set.

**Repo:** `crates/corba-ior/src/components.rs` (`CodeSetComponent::negotiate`,
`CodeSetComponentInfo::negotiate`, `CodeSetComponentInfo::transmission_code_sets`,
`TransmissionCodeSets`), `crates/corba-giop/src/code_set_registry.rs`
(`ENTRIES`, `compatible`, `is_iana`), `crates/corba-giop/src/code_set.rs`
(`CodeSetContext`), `crates/corba-interop/src/runtime.rs`
(`code_sets_component`, `IiopCorbaConnection::service_contexts`).

**Tests:** `crates/corba-ior/src/components.rs::tests::negotiate_client_converts_to_server_native`,
`crates/corba-ior/src/components.rs::tests::negotiate_universal_fallback`,
`crates/corba-ior/src/components.rs::tests::corba_p2_7_10_2_6_intermediate_set_is_the_servers_most_preferred`,
`crates/corba-ior/src/components.rs::tests::corba_p2_7_10_2_6_the_fallback_needs_compatible_natives`,
`crates/corba-ior/src/components.rs::tests::corba_p2_7_10_2_6_incompatible_character_sets_raise_codeset_incompatible_1`,
`crates/corba-ior/src/components.rs::tests::corba_p2_7_10_2_4_defaults_without_the_component_or_a_wchar_set`,
`crates/corba-giop/src/code_set_registry.rs::tests::corba_p2_7_10_2_6_unicode_code_sets_are_compatible_latin_1_is_not`,
`crates/corba-giop/src/code_set_registry.rs::tests::corba_p2_7_10_2_6_iana_ids_are_always_compatible`,
`crates/corba-giop/src/code_set.rs::tests::well_known_ids_match_osf_registry`,
`crates/corba-giop/src/code_set.rs::tests::service_context_roundtrip_via_list`.

**Status:** partial — built and green on the Linux test host: the
algorithm of §7.10.2.6 as a function — step 4 takes the common
conversion code set most preferable to the server (first in its list),
step 5 the fallback UTF-8/UTF-16 when `compatible(CNCS, SNCS)` holds (a
shared character set after the OSF registry rows as ACE carries them
from `code_set_registry1.2g.txt`; IANA ids always compatible; the
boundary IANA < 0x10000 is inferred from the values of both registries),
else CODESET_INCOMPATIBLE minor 1; without a code set component ISO
8859-1 and no TCS-W (INV_OBJREF minor 2 for wide data), a null char code
set counts as ISO 8859-1, also with conversion code sets (§7.10.2.4); a
null native wchar code set gives no TCS-W, also with wchar conversion
code sets (INV_OBJREF minor 1: §7.10.2.6 requires the native wchar code
set of a server with wide character data). Missing: (1) the client does
not negotiate per connection against the profile's `TAG_CODE_SETS` but
sends `CodeSetContext::default_pair()` (UTF-8, UTF-16) with every
request; the server does not read the client's context
(CODESET_INCOMPATIBLE minor 2, MARSHAL minor 9, BAD_PARAM minors 23 and
42, INV_OBJREF 1 and 2 at run time, MARSHAL 5/6 for `wchar` under GIOP
1.0); (2) the IOR names ISO 8859-1 and UCS-2 as conversion code sets
without a converter; (3) the native code set does not come from the
locale or a configuration (§7.10.2.2). All three depend on the
conversion of character data in the marshalling, item "§7.10 Conversion
into a negotiated code set other than UTF-8/UTF-16".

### §7.10 Conversion into a negotiated code set other than UTF-8/UTF-16

**Spec:** Part 2 §7.10.2, pp. 48–55 — client and server convert `char`
and `wchar` data into the negotiated transmission code set (e.g. ISO
8859-1).

**Repo:** —

**Tests:** —

**Status:** open — `crates/corba-giop/src/lib.rs` describes the
ISO-Latin conversion as the caller's task; `crates/wire/` encodes only
UTF-8 and UTF-16.

### §8 Building Inter-ORB Bridges

**Spec:** Part 2 §8, p. 63 — inline bridges, request-level bridges.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Part 2 §8.1: "an implementation-oriented conceptual framework for the construction of bridges"; the clause states no requirement. The CORBA↔DDS bridge is covered in `zerodds-corba-bridge-1.0.en.md`.

### §9 General Inter-ORB Protocol (GIOP)

**Spec:** Part 2 §9, p. 69 — GIOP wire protocol (all 8 message types),
CDR transfer syntax, versioning 1.0/1.1/1.2, IIOP mapping, bi-dir
GIOP.

**Repo:** `crates/corba-giop/src/` with one file per message type plus
`crates/corba-giop/src/header.rs`, `crates/corba-giop/src/codec.rs`,
`crates/corba-giop/src/service_context.rs`,
`crates/corba-giop/src/target_address.rs`,
`crates/corba-giop/src/flags.rs`, `crates/corba-giop/src/version.rs`.

**Tests:** `crates/corba-giop/src/codec.rs::{round_trip_request_giop_1_0_be,
round_trip_request_giop_1_2_le, giop_1_2_body_is_eight_aligned_from_the_message_start,
giop_1_0_body_is_not_padded, header_message_size_matches_actual_body,
message_size_larger_than_the_buffer_is_refused}`,
`crates/corba-giop/src/header.rs::{magic_bytes_match_spec,
header_size_is_12_bytes, round_trip_big_endian_request_header,
round_trip_little_endian_reply_header}`,
`crates/corba-giop/src/version.rs::{version_constants_match_spec,
flags_octet_replaces_byte_order_at_1_1, v1_2_request_layout_only_in_1_2_plus}`.

**Status:** done

#### §9.3 CDR Transfer Syntax

**Spec:** §9.3, p. 71 — CDR encoding, endianness marking,
primitive/construct layout, GIOP 1.2 alignment, value types, pseudo
objects (TypeCode, Any, Principal), object references, abstract
interfaces.

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(primitives + alignment + encapsulation), re-used by the GIOP codec in
`crates/corba-giop/src/codec.rs`.
`crates/corba-rt/src/type_code.rs` — all 37 `TCKind`s of Table 9.2 with
their parameter lists (empty, simple: `tk_string`, `tk_wstring`,
`tk_fixed`; complex in an encapsulation: also `tk_union` with
discriminant type, default index and typed labels `UnionLabel`,
`tk_array`, `tk_value`/`tk_event` with `ValueModifier`, concrete base
and `ValueMember` visibility, `tk_value_box`, `tk_native`,
`tk_abstract_interface`, `tk_local_interface`, `tk_component`,
`tk_home`); an indirection (§9.3.5.1.6) is read and written:
`TypeCode::Recursive` goes out as `0xffffffff` and a negative offset to
the `TCKind` of the enclosing type, counted across encapsulation
boundaries. `crates/corba-rt/src/any.rs` — `AnyValue` for every kind with
a value encoding: `LongDouble`, `Fixed` (BCD, read to the sign nibble),
`Principal`, `TypeCodeValue`, `Array`, `Union` (the selected case, else
the default, else no value), `Except` (repository id before the members,
§9.3.5.5), `ObjRef` (also `tk_component`/`tk_home`; nil as the empty
type id with no profile, the empty type id with an IIOP profile as a
reference to `Object`, §9.3.5.1.3; every profile of the IOR is kept
in its order and written back unchanged, Part 2 §7.6.3; a type id
without a profile is refused), `Abstract` (§9.3.7: boolean
discriminator, reference or value), `Value` (§9.3.4: null tag, value tag
with repository id or id list, codebase URL read into
`ValueInstance::codebase` and written with bit `0x01`, indirections to
values, repository ids and id lists, chunked state with continuation
chunks and shared end tags, truncation to a known base type; a value in
skipped state is recorded at its value tag and read at the first
indirection to it with the formal type of that slot, §9.3.4.6;
`ValueRef::Shared` for shared and cyclic graphs;
truncatable types are written chunked with an id list, the others
unchunked with one repository id); `CorbaAny::decode_with` takes known
value types for values in abstract interfaces and derived types.
`tk_native` and `tk_local_interface` have no value encoding and are
refused. The parts of a value (members, elements, union branch, alias,
recursive member, value state, an `any` inside an `any`) are read at most
`MAX_VALUE_DEPTH` (128) levels deep; a deeper value is a `WireError`, not
a stack overflow. A decode reads at most one part without octets (a
value of `tk_null` or `tk_void`, of an empty struct, of an array of
length 0, or one made of such values only) per octet it has read since
it started, and one more; nested sequences and arrays of such values are
refused before they allocate. A repeated `TypeCode` (an indirection to a
`TypeCode` read before that does not enclose it) is decoded by reading
its encapsulation again at the place of the indirection; these re-reads
of one `TypeCode` together take at most `MAX_TYPECODE_REREAD_FACTOR` (32)
times its octets, and the copies that top-level indirections make
(§9.3.5.1.6) at most 32 times the octets of the message read up to them.

**Tests:** `crates/corba-rt/src/type_code.rs::{corba_p2_9_3_5_every_tckind_round_trips,
corba_p2_9_3_5_1_simple_parameters_are_inline,
corba_p2_9_3_5_1_union_parameters_follow_table_9_2,
corba_p2_9_3_5_1_a_union_label_of_another_type_is_refused,
corba_p2_9_3_5_1_a_recursive_typecode_is_written_with_an_indirection,
indirection_recursive_struct, indirection_repeated_typecode,
recursive_marker_refuses_to_encode}`,
`crates/corba-rt/src/any.rs::{corba_p2_9_3_5_2_any_with_every_constructed_type_round_trips,
corba_p2_9_3_5_5_an_exception_in_an_any_starts_with_its_repository_id,
corba_p2_9_3_7_a_nil_reference_and_the_two_abstract_branches_on_the_wire,
corba_p2_9_3_union_member_must_be_the_selected_case,
corba_p2_9_3_local_and_native_values_are_refused,
corba_p2_9_3_4_a_value_is_tag_repository_id_and_state,
corba_p2_9_3_4_3_a_shared_and_a_cyclic_value_are_indirections,
corba_p2_9_3_4_6_a_truncatable_value_is_chunked_with_its_id_list,
corba_p2_9_3_4_6_chunks_continuations_and_a_shared_end_tag_are_read,
corba_p2_9_3_4_1_a_value_of_an_unknown_derived_type_is_truncated,
corba_p2_9_3_4_6_an_indirection_into_truncated_state_reads_the_skipped_value,
corba_p2_9_3_4_1_a_codebase_url_is_kept_and_written_back,
corba_p2_9_3_6_object_references_in_an_any_keep_their_profiles_or_are_refused,
corba_p2_9_3_4_a_value_box_and_an_abstract_value_round_trip,
corba_p2_9_3_a_value_nested_beyond_max_value_depth_is_refused_not_a_stack_overflow}`,
`crates/corba-rt/tests/decode_budget.rs::{corba_p2_9_3_nested_arrays_of_null_are_refused_without_a_large_allocation,
corba_p2_9_3_nested_sequences_of_null_are_refused_without_a_large_allocation,
corba_p2_9_3_parts_without_octets_up_to_the_octets_read_still_decode,
corba_p2_9_3_5_1_a_doubling_chain_of_repeated_typecodes_is_refused_quickly,
corba_p2_9_3_5_1_6_top_level_copies_stay_within_the_octets_read}`,
`crates/corba-giop/src/codec.rs::{giop_1_2_body_is_eight_aligned_from_the_message_start,
giop_1_2_reply_body_is_eight_aligned_from_the_message_start,
giop_1_0_body_is_not_padded}`,
`crates/corba-giop/src/request.rs::tests::giop_1_2_request_body_is_8_aligned`,
`crates/corba-giop/src/service_context.rs::{round_trip_with_two_entries_be,
round_trip_le_byte_order}`.

**Status:** partial — the TypeCodes of every kind, indirection in both
directions and the `any` values of every kind green on the Linux test
host. `array<array<null, 1000>, 1000>` and `sequence<sequence<null>>`
with 10 times 100000 elements are refused, a doubling chain of 24
repeated structs (1.4 KB) is refused, each while the decoding thread
holds less than 1 MiB; a chain of 3 decodes to its copies (Linux test
host). Open: a decoded value carries a copy of the type information of
its kind (the element `TypeCode` of each sequence and array, the
`TypeCode` of each union, value and reference, the enumerators of each
enum, the member names of each struct); many small values of a large
type copy it once per value, and the memory then grows with the product
of the two sizes, which neither bound limits (an `AnyValue` that shares
the type information is not built). Open also: custom-marshalled values (`VM_CUSTOM`) have no state a
TypeCode describes and are refused; an object reference with more than
one profile or without an IIOP profile is refused in an `any`, because
`ObjectReference` carries only the IIOP profile; the proof against
omniORB and JacORB (an `any` with union, array, fixed and valuetype in
both directions) is not written and not run.

#### §9.4 GIOP Message Formats

**Spec:** §9.4, p. 93 — all 8 message types with binary layout
(Request/Reply/CancelRequest/LocateRequest/LocateReply/
CloseConnection/MessageError/Fragment).

**Repo:** one file per message type in `crates/corba-giop/src/` —
`crates/corba-giop/src/request.rs`, `crates/corba-giop/src/reply.rs`,
`crates/corba-giop/src/cancel_request.rs`,
`crates/corba-giop/src/locate_request.rs`,
`crates/corba-giop/src/locate_reply.rs`,
`crates/corba-giop/src/close_connection.rs`,
`crates/corba-giop/src/message_error.rs`,
`crates/corba-giop/src/fragment.rs`. The server
(`crates/corba-interop/src/runtime.rs::server_other_message`, used by
`CorbaServer` and `MiopGateway`) takes every message type explicitly:
`CancelRequest` keeps the connection open (each request is answered
before the next message is read), `CloseConnection` and `MessageError`
end it, `Reply`, `LocateReply` and a `Fragment` without a fragmented
message before it get `MessageError`, then the connection ends. The
body at its offset in the message (§9.4.2.1, §9.4.3.1: under GIOP 1.0
and 1.1 it follows the header without padding, and CDR aligns it from
the first octet of the message): `crates/corba-giop/src/body.rs`
(`AlignedBody`, `body_writer`, `finish_body`),
`crates/corba-giop/src/request.rs` and `crates/corba-giop/src/reply.rs`
(`body_offset`), `crates/corba-giop/src/object_ops.rs` (reads at the
offset); `crates/corba-rust/src/runtime.rs` (`MarshalArgs`,
`no_arguments`, `marshaled_body`, `CorbaConnection`,
`AsyncCorbaChannel`), `crates/corba-rust/src/interface_emit.rs` and
`crates/corba-rust/src/ami_emit.rs` (the stub writes the arguments once
the connection knows the header; the skeleton reads them from the
`AlignedBody`); `crates/corba-interop/src/runtime.rs` (`request_body`,
`received_body`, `IiopCorbaConnection`, `AmiClient`, `CorbaServer`,
`BiDirEndpoint`, `AmhEndpoint`), `crates/corba-interop/src/miop.rs`
(`MiopClient::request_message`, `Goa`, `MiopGateway`),
`crates/corba-interop/src/mgm.rs`.

**Tests:** one octet round trip per message type —
`crates/corba-giop/src/request.rs::{round_trip_giop_1_0_request,
round_trip_giop_1_1_request_with_reserved_bytes,
round_trip_giop_1_2_request_with_target_address}`,
`crates/corba-giop/src/reply.rs::{round_trip_giop_1_0_no_exception,
round_trip_giop_1_2_user_exception, round_trip_location_forward_perm_in_1_2}`,
`crates/corba-giop/src/cancel_request.rs::tests::round_trip`,
`crates/corba-giop/src/locate_request.rs::{round_trip_giop_1_0,
round_trip_giop_1_2_with_profile_addr}`,
`crates/corba-giop/src/locate_reply.rs::{round_trip_giop_1_0_object_here,
round_trip_giop_1_2_object_forward_perm,
corba_p2_9_4_6_2_the_locate_reply_body_follows_the_header_without_padding}`,
`crates/corba-giop/src/target_address.rs::{corba_p2_9_4_2_1_profile_addr_is_the_tagged_profile_struct,
corba_p2_9_4_2_1_reference_addr_is_the_ior_addressing_info_struct}`
(the layout of the IDL structs, not only a round trip),
`crates/corba-giop/src/fragment.rs::{round_trip_giop_1_2_with_header,
round_trip_giop_1_1_without_header}`. All eight through the
whole-message codec, CloseConnection and MessageError (no body) only there: `crates/corba-giop/src/codec.rs::{round_trip_request_giop_1_0_be,
round_trip_close_connection, round_trip_message_error,
round_trip_fragment_with_more_bit, round_trip_cancel_request,
round_trip_locate_request_giop_1_2, round_trip_locate_reply_object_here}`.
The type discriminators themselves:
`crates/corba-giop/src/message_type.rs::tests::message_type_values_match_spec_table_15_1`.
The server: `crates/corba-interop/tests/giop_version.rs::corba_p2_9_4_the_server_takes_every_message_type_explicitly`.
The body at its offset:
`crates/corba-giop/src/codec.rs::tests::corba_p2_9_4_2_a_long_long_under_giop_1_0_and_1_1_is_read_at_its_body_offset`,
`crates/corba-giop/src/object_ops.rs::tests::corba_p2_9_4_2_is_a_reads_its_argument_at_the_body_offset`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_2_an_8_octet_argument_after_an_octet_is_aligned_from_the_message_start`,
`crates/corba-poa/src/poa.rs::tests::corba_p2_9_4_2_1_a_servant_reads_a_long_long_at_the_body_offset`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_2_1_a_poa_servant_reads_an_8_octet_argument_at_the_body_offset`,
`crates/corba-cosnaming/src/servant.rs::tests::corba_p2_9_4_2_1_a_request_body_is_read_at_its_offset_in_the_message`,
`crates/corba-ccm/src/dynamic_api.rs::tests::corba_p2_9_4_2_1_a_dsi_servant_reads_its_arguments_at_their_offset`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_2_1_a_dsi_servant_reads_an_8_octet_argument_at_the_body_offset`,
`crates/corba-giop/src/reply.rs::tests::corba_p2_9_4_3_a_giop_1_0_reply_body_is_placed_on_eight`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_3_a_giop_1_0_reply_body_with_reply_contexts_is_aligned_from_the_message_start`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_3_a_reply_body_of_another_orb_on_four_modulo_eight_is_read_at_its_offset`,
`crates/corba-giop/src/reply.rs::tests::corba_p2_9_4_3_a_body_written_for_four_modulo_eight_is_placed_there`,
`crates/corba-interop/tests/giop_body_offset.rs::{corba_p2_9_4_3_a_user_exception_of_another_orb_on_four_modulo_eight_is_read_at_its_offset,
corba_p2_9_4_3_a_relayed_user_exception_keeps_its_offset_modulo_eight,
corba_p2_9_4_3_a_relayed_user_exception_goes_out_in_its_own_byte_order,
corba_p2_9_4_3_an_ami_reply_of_another_orb_on_four_modulo_eight_is_read_at_its_offset,
corba_p2_9_4_3_a_dii_result_of_another_orb_on_four_modulo_eight_is_read_at_its_offset}`,
`crates/corba-interop/tests/amh.rs::corba_p2_9_4_3_an_amh_user_exception_goes_out_in_its_own_byte_order`,
`crates/corba-interop/tests/bidir_giop.rs::corba_p2_9_4_3_a_bidir_user_exception_goes_out_in_its_own_byte_order`,
`crates/corba-interop/tests/miop.rs::corba_p2_9_4_3_the_gateway_relays_a_reply_body_on_its_offset_modulo_eight`.

**Status:** partial — the eight message types with their layout and
their handling in the server green on the Linux test host. The body of
a request stands at its offset: the client writes the arguments once
the request header is fixed (again after a forward), the server hands
the body over with its offset, the generated skeleton and the object
operations read there; over IIOP 1.0, 1.1 and 1.2 profiles, in both byte
orders, an 8-octet argument after an octet stands on a multiple of 8
from the message start and reaches the servant, at body offsets of 0
and 4 modulo 8 (Linux test host). The reply body of `CorbaServer` under
GIOP 1.0/1.1 stands on a multiple of 8: when the service contexts (those
of the interceptors included) put it on 4 modulo 8, a padding context
(`PADDING_CONTEXT_ID`, four octets) moves it by 12; over IIOP 1.0 and 1.1
profiles with reply contexts of 1 to 8 octets the `double` of the reply
stands on a multiple of 8 and the generated stub reads it (Linux test
host). The generated stub reads a reply body at its offset
(`CorbaConnection::invoke_aligned`; `IiopCorbaConnection` takes it from
the version and the header of the reply): the GIOP 1.0/1.1 reply of
another ORB whose body starts on 4 modulo 8 is read right, and read from
offset 0 the same octets give another `double` (Linux test host). The
body of a user exception keeps its offset (`CorbaException::UserException`
and `RequestException::User` carry an `AlignedBody`; `AmiClient` and
`BiDirEndpoint` take it from the version and the header of the reply):
the generated stub reads the `long long` and the `double` after the
repository id right from another ORB's GIOP 1.0/1.1 reply whose body
starts on 4 modulo 8, and read from offset 0 the same body gives another
`long long` (Linux test host); the MGM proxy reads its exceptions there
too. When a servant passes on a user exception received that way,
`CorbaServer` (and AMH and `BiDirEndpoint`, `Reply::place_body_at`)
writes its body under GIOP 1.0/1.1 on the same offset modulo 8, and the
stub reads the members; a GIOP 1.2 reply, whose body is on 8, cannot
carry it and becomes `MARSHAL`, `COMPLETED_YES` (Linux test host).
`invoke_passing_contexts` returns the reply body with its offset, and the
MIOP gateway writes it on the same offset modulo 8: the body of a GIOP 1.2
object reaches the client's GIOP 1.1 reply on 8, that of a GIOP 1.1
object the GIOP 1.0 reply on 4 modulo 8, both times with the `double` on
a multiple of 8; to a GIOP 1.2 client this body becomes `MARSHAL`,
`COMPLETED_YES` (Linux test host). A user exception that a servant
passes on in the other byte order than the request's goes out in a
reply of its own byte order (`CorbaServer` with and without server
interceptors, AMH, `BiDirEndpoint`, as the MIOP gateway does), and the
client reads its members, under GIOP 1.0, 1.1 and 1.2 (Linux test
host). The body of a `NO_EXCEPTION` reply keeps its offset on the AMI
path (`AsyncReply` carries an `AlignedBody`; the generated `sendc_`
callback and `sendp_` poller read through `AlignedBody::reader`), in
`BiDirEndpoint::collect_reply` and `invoke`, and in the DII
(`Request::invoke` keeps `result_offset` and `result_endianness`,
`Request::result_body`): from another ORB's GIOP 1.0/1.1 reply whose
body starts on 4 modulo 8, in both byte orders, the poller, the callback
and the DII read the `double` right, and read from offset 0 the same
octets give another value (Linux test host). The POA (`Poa::dispatch`, `Servant::invoke` with the body
at its offset and the byte order of the message; the server
`zerodds_corba_interop::serve` replies in the version and byte order of
the request), `NamingServant::dispatch` (naming daemon and CORBA/DDS
bridge daemon; the request decoders of `wire` read from a `Reader` at
the offset) and the DSI servant (`ServerRequest::received` and
`input()` with `arguments_offset` and `endianness`) read the body at its
offset: over IIOP 1.0, 1.1 and 1.2 profiles, in both byte orders, a
`double` (from GIOP 1.0) and a `long long` (from GIOP 1.1, Table 7.2)
after an octet reach the POA servant and the DSI servant right at body
offsets of 0 and 4 modulo 8; the naming servant resolves a name at
offsets 13, 14, 15 and 17 (Linux test host). Open: the CORBA/DDS bridge
daemon puts the body into the DDS sample without its offset. `BiDirEndpoint` sends a body written from offset 0 (GIOP
1.2) and refuses to forward it to a target whose body starts on 4
modulo 8 with `MARSHAL`. Also open: §9.4.6.3 — a `ForwardRequest` of a POA
`ServantLocator` as a `LocateReply` with `OBJECT_FORWARD`:
`crates/corba-poa/src/servant_manager.rs::ServantLocator::preinvoke`
has no `ForwardRequest`; only the location agent of
`CorbaServer::forward` sends `OBJECT_FORWARD` (§9.6).

#### §9.5 GIOP Message Transport

**Spec:** §9.5, p. 107 — transport requirements (ordered, lossless),
connection management.

**Repo:** `crates/corba-iiop/src/connection.rs`,
`crates/corba-iiop/src/acceptor.rs`,
`crates/corba-iiop/src/connector.rs` for TCP transport.

**Tests:** `crates/corba-iiop/src/acceptor.rs::tests::round_trip_via_acceptor_and_connector`,
`crates/corba-iiop/src/connection.rs::{round_trip_request_reply,
shutdown_propagates_eof_to_peer, close_connection_message_round_trip}`,
`crates/corba-iiop/src/framing.rs::tests::read_back_to_back_messages`
(message boundaries in the stream),
`crates/corba-iiop/src/connector.rs::{connect_reuses_pooled_connection,
invalidated_connection_is_not_returned_to_pool,
max_connections_per_endpoint_is_enforced}`.

**Status:** done — TCP via IIOP. Other transport bindings (e.g.
SCTP) not offered.

#### §9.6 Object Location

**Spec:** §9.6, p. 110 — LocateRequest/LocateReply semantics, forward-
ing.

**Repo:** Client: `crates/corba-interop/src/runtime.rs::{IiopCorbaConnection::send,
IiopCorbaConnection::locate, IiopCorbaConnection::forwarded_reference,
Route, LocateOutcome, classify_reply, target_address}` — an invocation
follows `LOCATION_FORWARD` and `LOCATION_FORWARD_PERM` (at most 16 times,
then `TRANSIENT`), and an interceptor's `ForwardRequest`;
`LOCATION_FORWARD_PERM` and `OBJECT_FORWARD_PERM` are recorded per target
reference and used first by later invocations; when the recorded target
cannot be reached or answers `TRANSIENT`, `COMM_FAILURE` or
`OBJECT_NOT_EXIST` with `COMPLETED_NO`, the record is dropped and the
invocation starts at the original reference. `NEEDS_ADDRESSING_MODE`
resends the request in the requested disposition (`ProfileAddr`,
`ReferenceAddr`); the interceptors see `receive_other` with
`TRANSPORT_RETRY` (`crates/corba-ccm/src/portable_interceptor.rs::Outcome::Retry`).
`locate` sends `LocateRequest` and handles all six `LocateStatusType`
values. `AmiClient::perform_work` and `BiDirEndpoint::collect_reply`
follow forwards over a connection of their own and readdress after
`NEEDS_ADDRESSING_MODE`. Server:
`crates/corba-interop/src/runtime.rs::{CorbaServer::forward,
CorbaServer::stop_forwarding, locate_reply, forward_reply,
target_object_key, needs_key_addr_reply}` — a server is a location agent
for forwarded object keys (`LOCATION_FORWARD(_PERM)`,
`OBJECT_FORWARD(_PERM)`, `_PERM` from GIOP 1.2 on), serves all three
dispositions and asks for `KeyAddr` when a target names no readable key
(`crates/corba-interop/src/lib.rs::serve` as well). Wire:
`crates/corba-giop/src/target_address.rs::{TargetProfile,
IorAddressingInfo, AddressingDisposition}` (`ProfileAddr` and
`ReferenceAddr` as the IDL structs; before, with an extra length field),
`crates/corba-giop/src/locate_reply.rs` (the body right after the
header, without 8-octet alignment, §9.4.6.2).

**Tests:** `crates/corba-interop/tests/location_forward.rs::{corba_p2_9_6_client_follows_a_location_forward,
corba_p2_9_6_forward_perm_replaces_the_reference,
corba_p2_9_6_an_unreachable_recorded_forward_restarts_at_the_original_address,
corba_p2_9_6_a_recorded_forward_without_the_object_restarts_at_the_original_address,
corba_p2_9_6_a_forward_cycle_ends_with_transient,
corba_p2_9_6_the_forwarded_request_carries_the_service_contexts,
corba_p2_9_4_needs_addressing_mode_is_resent_with_the_profile,
corba_p2_9_4_needs_addressing_mode_is_resent_with_the_ior,
corba_p2_9_4_an_agent_asking_again_for_the_disposition_it_got_is_transient,
corba_p2_9_6_locate_follows_object_forward_and_reports_unknown_object,
corba_p2_9_4_6_loc_needs_addressing_mode_resends_the_locate_request,
corba_p2_9_4_6_loc_system_exception_is_raised,
corba_p2_9_4_2_1_a_server_serves_every_addressing_disposition,
corba_p2_9_6_a_location_agent_answers_locate_with_object_forward,
corba_p2_9_6_an_ami_request_follows_a_location_forward,
corba_p2_9_4_an_ami_request_is_readdressed_and_keeps_its_id,
corba_p2_9_6_a_bidir_request_follows_a_location_forward}`,
`crates/corba-giop/src/target_address.rs::{corba_p2_9_4_2_1_profile_addr_is_the_tagged_profile_struct,
corba_p2_9_4_2_1_reference_addr_is_the_ior_addressing_info_struct,
a_reference_addr_with_more_profiles_than_octets_is_refused}`,
`crates/corba-giop/src/locate_reply.rs::{locate_status_values_match_spec,
corba_p2_9_4_6_2_the_locate_reply_body_follows_the_header_without_padding,
round_trip_giop_1_2_object_forward_perm, forward_perm_in_1_0_is_rejected}`,
`crates/corba-interop/tests/portable_interceptors.rs::corba_p1_16_4_interceptors_run_in_the_iiop_request_path`.

**Status:** partial — client and agent behaviour green against own
servers and raw GIOP peers on the Linux test host. Open: the proof
against a foreign location agent (an omniORB `ServantLocator` raising
`ForwardRequest`, with a ZeroDDS client); not written, not run.

#### §9.7 Internet Inter-ORB Protocol (IIOP)

**Spec:** §9.7, p. 111 — IIOP as TCP mapping; IIOP::ProfileBody
(host/port/object key + TaggedComponents).

**Repo:** `crates/corba-iiop/src/profile_body.rs`,
`crates/corba-iiop/src/framing.rs`, `crates/corba-iiop/src/acceptor.rs`,
`crates/corba-iiop/src/connector.rs`, `crates/corba-iiop/src/connection.rs`.

**Tests:** `crates/corba-iiop/src/profile_body.rs::{iiop_version_constants_match_spec,
components_only_from_iiop_1_1, round_trip_v1_0_be, round_trip_v1_0_le,
round_trip_v1_2_with_components, v1_0_does_not_emit_components_field}`,
`crates/corba-iiop/src/framing.rs::{round_trip_request_through_cursor,
round_trip_reply_le, outbound_fragmentation_be_le_many_fragments,
fragment_reassembly_three_fragments, oversized_message_size_is_rejected_cleanly}`.

**Status:** done

#### §9.8/§9.9 Bi-Directional GIOP

**Spec:** §9.8 + §9.9, pp. 115-118 — BiDirIIOP service context,
BiDirGIOP policy.

**Repo:** `crates/corba-iiop/src/bidir.rs` (wire codec) +
`crates/corba-ccm/src/orb_extensions.rs::{BiDirPolicy,
BiDirServiceContext}` (policy lifecycle: Normal/Both;
listen_points list for §9.9.1).

**Tests:** inline tests in `bidir.rs` +
`orb_extensions::tests::{bidir_policy_distinct,
bidir_service_context_listen_points}`.

**Status:** done — BiDir codec + policy lifecycle live.

#### §9.10 OMG IDL for GIOP

**Spec:** §9.10, p. 119 — IDL for GIOP module constants + service
context IDs.

**Repo:** as Rust constants in `crates/corba-giop/src/service_context.rs`
(`ServiceContextTag`), `crates/corba-giop/src/message_type.rs` (`MessageType`),
`crates/corba-giop/src/flags.rs` (`Flags`),
`crates/corba-giop/src/version.rs` (`Version`),
`crates/corba-giop/src/reply.rs` (`ReplyStatusType`),
`crates/corba-giop/src/locate_reply.rs` (`LocateStatusType`),
`crates/corba-giop/src/target_address.rs` (`AddressingDisposition`).

**Tests:** one value comparison against the IDL per constant group —
`crates/corba-giop/src/service_context.rs::tests::standard_tag_values_match_spec`,
`crates/corba-giop/src/message_type.rs::tests::message_type_values_match_spec_table_15_1`,
`crates/corba-giop/src/flags.rs::{fragment_bit_value_matches_spec,
byte_order_bit_value_matches_spec}`,
`crates/corba-giop/src/version.rs::tests::version_constants_match_spec`,
`crates/corba-giop/src/reply.rs::tests::reply_status_values_match_spec`,
`crates/corba-giop/src/locate_reply.rs::tests::locate_status_values_match_spec`,
`crates/corba-giop/src/target_address.rs::tests::disposition_values_match_spec`.

**Status:** done

### §10 Secure Interoperability (CSIv2)

**Spec:** Part 2 §10, p. 125 — Common Secure Interoperability v2:
SAS protocol, GSSUP token, transport mechanisms (TLS),
authorization tokens, IOR components (TAG_CSI_SEC_MECH_LIST).

**Repo:** `crates/corba-csiv2/` with `sas.rs`, `gssup.rs`, `mech_list.rs`,
`association_options.rs`; IOR tag codec in `crates/corba-ior/src/component_tags.rs`.

**Tests:** `crates/corba-csiv2/src/sas.rs::tests` (9 tests):
`sas_msg_type_values_match_spec`,
`sas_context_body_roundtrip_all_messages_be_le`,
`server_eval_decodes_foreign_gss_wrapped_establish_context`,
`identity_token_discriminator_matches_spec`,
`establish_context_holds_full_payload`, `complete_context_indicates_stateful`,
`context_error_carries_gss_status`;
`crates/corba-csiv2/src/session.rs::tests` (9), among them
`a_stateful_target_answers_establish_with_a_stateful_complete`,
`a_full_session_survives_the_wire_in_both_byte_orders`;
`crates/corba-csiv2/src/gssup.rs::tests::gssup_byte_identical_to_jacorb`,
`::initial_context_token_tag_is_0x60`,
`::encapsulation_is_wire_correct_octet_seq_len_at_offset_4`.

**Status:** done

#### §10.2 Protocol Message Definitions

**Spec:** §10.2, p. 127 — SAS message codec (EstablishContext,
CompleteEstablishContext, ContextError, MessageInContext).

**Repo:** `crates/corba-csiv2/src/sas.rs` (`SasMessage` with
`encode_encapsulation`/`decode_encapsulation`, `IdentityToken`).

**Tests:** `crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
sas_context_body_roundtrip_all_messages_be_le,
sas_identity_token_absent_anonymous_roundtrip,
identity_token_discriminator_matches_spec, establish_context_holds_full_payload,
complete_context_indicates_stateful, message_in_context_can_request_discard,
context_error_carries_gss_status}`.

**Status:** done

#### §10.3 Security Attribute Service (SAS)

**Spec:** §10.3, p. 137 — SAS state machine, stateful/stateless
contexts.

**Repo:** `crates/corba-csiv2/src/session.rs::{SasTarget, SasClient,
TargetAction, RetainedContext, CONTEXT_ERROR_NO_CONTEXT,
CONTEXT_ERROR_INVALID_EVIDENCE}` — the target and client halves of the
state machine over the `SasMessage` types of §10.2. A stateful target
retains the context under its `ContextId` and the client then sends
`MessageInContext` instead of fresh credentials, until a
`discard_context` or a `ContextError` puts it back into the establish
state.

**Tests:** `crates/corba-csiv2/src/session.rs::{a_stateful_target_answers_establish_with_a_stateful_complete,
a_stateless_target_retains_nothing_and_says_so,
the_second_request_on_a_stateful_context_is_a_message_in_context,
a_stateless_client_re_establishes_on_every_request,
discarding_a_context_makes_the_next_reference_to_it_a_context_error,
a_message_in_context_for_an_unknown_id_is_refused,
a_context_error_puts_the_client_back_into_the_establish_state,
a_target_to_client_message_arriving_at_a_target_is_not_evidence,
a_full_session_survives_the_wire_in_both_byte_orders}`.

**Status:** done — the last test drives the whole session over the
§10.2 octets: every message crosses `encode_encapsulation` /
`decode_encapsulation` before the other half sees it.

#### §10.4 Transport Security Mechanisms

**Spec:** §10.4, p. 149 — TLS/SSL bindings, mutual auth.

**Repo:** TLS stack via `crates/security-pki/` (X.509 + TLS).
CSIv2-→-IIOP-TLS bind via `crates/corba-csiv2::association_options`
+ `crates/corba-iiop` acceptor (TAG_TLS_SEC_TRANS profile tags
in `crates/corba-ior/src/component_tags.rs`); the glue is bound in the
acceptor lifecycle.

**Tests:** `crates/corba-csiv2/src/mech_list.rs::tests`:
`build_mech_list_with_tls_and_gssup`, `empty_list_has_no_mechanisms`,
`cdr_roundtrip_compound_sec_mech_list`,
`octet_seq_seq_rejects_oversized_count`,
`compound_sec_mech_list_rejects_oversized_count`, `cdr_roundtrip_empty_list`;
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`::has_and_with_round_trip`;
`crates/corba-ior/src/component_tags.rs::tests::well_known_component_tag_values_match_spec`,
`::round_trip_all_known_tags`, `::at_least_32_well_known_tags_modelled`.

**Status:** done — TLS stack + CSIv2-IIOP bind via IOR tags live.

#### §10.5 Interoperable Object References (IOR with security tags)

**Spec:** §10.5, p. 150 — IOR format with TAG_CSI_SEC_MECH_LIST,
TAG_NULL_TAG, TAG_TLS_SEC_TRANS.

**Repo:** `crates/corba-ior/src/component_tags.rs` (`ComponentId` with
`CsiSecMechList` = 33, `NullTag` = 34, `TlsSecTrans` = 36),
`crates/corba-ior/src/components.rs` (`StructuredComponent`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`).

**Tests:** `crates/corba-ior/src/ior.rs::tests::a_security_ior_carries_mech_list_tls_and_null_tag_through_a_round_trip`
— a complete IOR carrying the three tags in the IIOP profile's
component list, through `encode_encapsulation`/`decode_encapsulation`
and back into the structured form. Tag values and the component bodies
on their own:
`crates/corba-ior/src/component_tags.rs::{well_known_component_tag_values_match_spec,
round_trip_all_known_tags, at_least_32_well_known_tags_modelled}`,
`crates/corba-ior/src/components.rs::{csi_sec_mech_list_round_trip,
tls_sec_trans_with_addresses_round_trip, ssl_sec_trans_round_trip}`.

**Status:** done

#### §10.6 Conformance Levels (CSIv2)

**Spec:** §10.6, p. 160 — conformance levels 0, 1 and 2 and stateful
conformance.

**Repo:** Level 0: SAS in the service context protocol over TLS
(SSLIOP, rustls) and over unprotected IIOP, stateless CSS/TSS
(`crates/corba-csiv2/src/session.rs`), GSSUP
(`crates/corba-csiv2/src/gssup.rs`, `authenticator.rs`), identity
assertion: `crates/corba-csiv2/src/identity.rs::{ScopedUsername,
exported_gssup_name, parse_exported_name, AssertedIdentity,
AssertionTrust}` (GSS exported name per RFC 2743 with the GSSUP OID and
a scoped-username, token directly or as a CDR encapsulation) and
`authenticator.rs::authenticate_gssup_with` (backward trust evaluation
per §10.3.1.1: a user may assert itself and the anonymous identity,
others only under a trust rule; otherwise `ContextError` invalid
evidence; an unsupported identity type, a principal name of another
mechanism and an identity extension are invalid mechanism);
`authenticator.rs::authenticate_with_verifier` the same with a check
function in place of the password file. TSS of `CorbaServer`:
`crates/corba-interop/src/runtime.rs::CorbaServer::{require_credentials,
set_identity_assertion}` (GSSUP, supported identity types and trust
rules through `authenticate_with_verifier`, reply with
`CompleteEstablishContext`, or `NO_PERMISSION` and `ContextError`) and
`crates/corba-interop/src/runtime.rs::received_principal` (the invocation principal of Table 10.4
for the dispatcher). Client:
`crates/corba-interop/src/runtime.rs::IiopCorbaConnection::{with_csiv2_credentials,
with_csiv2_identity_assertion, with_csiv2_authorization_token}` —
identity assertion and the push model of authorization tokens
(§10.6.2.1, CSS side).

**Tests:** `crates/corba-csiv2/src/identity.rs::{corba_p2_10_2_5_2_scoped_username_forms_and_quoting,
corba_p2_10_2_5_1_the_gssup_exported_name_follows_rfc_2743,
corba_p2_10_2_5_a_token_is_read_directly_or_from_an_encapsulation,
corba_p2_10_3_1_1_backward_trust_evaluation}`,
`crates/corba-csiv2/src/authenticator.rs::{corba_p2_10_3_1_1_an_identity_the_user_is_not_trusted_to_assert_is_invalid_evidence,
corba_p2_10_2_5_a_principal_name_of_another_mechanism_is_invalid_mechanism,
corba_p2_10_2_5_an_identity_type_the_target_does_not_support_is_invalid_mechanism,
a_supported_identity_assertion_is_accepted_as_the_asserted_principal,
valid_credentials_with_no_identity_assertion_are_accepted,
wrong_password_is_rejected_with_invalid_evidence}`,
`crates/corba-csiv2/src/session.rs::{a_stateless_target_retains_nothing_and_says_so,
a_stateless_client_re_establishes_on_every_request}`,
`crates/corba-interop/tests/csiv2.rs::{csiv2_gssup_authenticates_and_rejects,
corba_p2_10_6_2_1_a_client_asserts_an_identity_and_pushes_an_authorization_token,
corba_p2_10_3_1_1_a_corba_server_evaluates_the_asserted_identity,
corba_p2_10_6_1_1_the_sas_context_is_evaluated_over_tls_1_2_and_1_3}`.

**Status:** partial — built and green on the Linux test host: the Level 0
parts SAS over TLS 1.2 and 1.3 (SSLIOP server of `CorbaServer`, with a
server that offers only TLS 1.2 and with a TLS 1.3 server) and over
IIOP, stateless mode, GSSUP, identity assertion with GSSUP exported
names and backward trust evaluation, also in the TSS of `CorbaServer`;
of Level 1 the CSS side (a client sends an authorization token). A TSS
does not evaluate a received authorization token; Level 0 allows that
(§10.6.1.2.4). Blocked: SSL 3.0 and TLS 1.0 with the mandatory ciphersuites of
RFC 2246 (§10.6.1.1.1) — rustls offers only TLS 1.2 and 1.3, SSL 3.0 and
TLS 1.0 are prohibited by RFC 7568 and RFC 8996. Open: of Level 1 the TSS
evaluation of an authorization token (X.509 attribute certificates per
the IETF AC profile, trusted privilege authorities, access identity
against the client or asserted identity) and `privilege_authorities` in
the IOR; Level 2 (Proxy Info extension, forward trust evaluation,
`DelegationByClient` and the IOR flags of Table 10.17); stateful
conformance (§10.6.4): only the stateless mode is built, no TSS retains
and no CSS establishes reusable contexts (`MessageInContext`,
§10.3.2.2); without a `client_authentication_token` no TSS derives the
client authentication identity from the transport layer (§10.3.1.1,
Table 10.3 row P2A/None/P1): `authenticate_gssup_with`,
`authenticate_with_verifier` and with them `CorbaServer` require a GSSUP
token and refuse an identity assertion with a TLS client certificate and
an empty token as invalid evidence, `corba-iiop` does not pass on the
client certificate of an SSLIOP connection; the daemons
`zerodds-corba-bridged` and `zerodds-naming-svc` have no
configuration key for trust rules — they call `authenticate_gssup` with
`AssertionTrust::new()`, so a user can assert only itself and the
anonymous identity there; the proof against JacORB 3.9 (identity
assertion with `ITTPrincipalName` in both directions) is not written and
not run.

#### §10.7 Sample Message Flows and Scenarios

**Spec:** §10.7, p. 162 — examples.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — sample flows.

#### §10.8 References

**Spec:** §10.8, p. 171 — reference list.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — reference list.

#### §10.9 IDL for CSIv2

**Spec:** §10.9, p. 172 — IDL of the CSI modules.

**Repo:** as Rust representation in
`crates/corba-csiv2/src/association_options.rs` (`AssociationOptions`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`,
`CompoundSecMech`, `AsContextSec`, `SasContextSec`),
`crates/corba-csiv2/src/sas.rs` (`SasMsgType`, `IdentityToken`, the
`SASContextBody` union as `SasMessage`),
`crates/corba-csiv2/src/gssup.rs` (`GssupCredentialToken`).

**Tests:** one value/layout comparison against the IDL per module —
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
identity_token_discriminator_matches_spec}`,
`crates/corba-csiv2/src/mech_list.rs::{cdr_roundtrip_compound_sec_mech_list,
build_mech_list_with_tls_and_gssup, cdr_roundtrip_empty_list}`,
`crates/corba-csiv2/src/gssup.rs::{initial_context_token_tag_is_0x60,
gssup_oid_starts_with_der_tag, encapsulation_is_wire_correct_octet_seq_len_at_offset_4}`.

**Status:** done

### §11 Unreliable Multicast Inter-ORB Protocol (MIOP)

**Spec:** Part 2 §11, p. 181 — GIOP requests over UDP multicast:
`PacketHeader_1_0` and packet collections (§11.1), the UIPMC profile,
`TAG_GROUP`, `TAG_GROUP_IIOP`, the group IOR (§11.2.2, §11.2.3),
`PortableGroup::GOA` (§11.2.4), the gateway (§11.2.5), the Multicast
Group Manager with its properties (§11.2.6), `corbaloc:miop` (§11.2.7),
requests in GIOP 1.0 to 1.2 (§11.3).

**Repo:** `crates/corba-iiop/src/miop.rs` — `PacketHeader` (magic,
version 1.0, endian and stop flag, reserved bits zero, `packet_length`,
`packet_number`, `number_of_packets`, `UniqueId` up to 252 octets, the
GIOP data from the next 8-octet boundary, field values in the byte order
of the flag), `packetize` (packets of equal length, a shorter last one
with the stop flag), `Reassembler` (by sender and `UniqueId`, any order;
a stop packet with packets missing drops the collection, as do
contradictions, a packet number at or beyond the `number_of_packets` of
an earlier packet, a size over 16 MiB and the timeout, counted from the
last packet of the collection; all pending collections together hold at
most 64 MiB, each packet counted with 64 octets of overhead, and beyond
that the collection with the oldest last packet goes; every collection
not delivered is in `ReassemblyStats`), `MiopSender` (TTL or hops,
`unique_id` of 16 octets), `join_group` and `MiopReceiver` (IPv4 and
IPv6, `SO_REUSEADDR` and `SO_REUSEPORT`, a chosen interface).
`crates/corba-ior/src/miop.rs` — `UipmcProfileBody`, `GroupInfo`,
`group_iiop_component`, `group_profiles`, `is_group_reference`,
`is_equivalent` and `hash` after §11.2.6.10, `parse_miop_corbaloc` (group
id with domain, group and reference version, IPv6 in brackets, group IIOP
after `;`, gateway addresses after `,`, multicast addresses only).
`crates/corba-ccm/src/orb_core.rs` — `ObjectRef::Group`:
`string_to_object` turns `corbaloc:miop:` and an IOR with a UIPMC group
profile into a group reference, `object_to_string` keeps the profile.
`crates/corba-interop/src/miop.rs` — `MiopClient` (oneway request to the
group, GIOP 1.2 with the UIPMC profile in the target, GIOP 1.0/1.1 with
`"MIOP"` and the profile in the object key; `is_a` over the IIOP profile
of `TAG_GROUP_IIOP`, without that component through the interface
repository of `with_interface_repository` with the base interfaces
(`crates/corba-ir/src/repository.rs::Repository::interface_is_a`,
`InterfaceDef::is_a` after Part 1 §14.5.24.1), `non_existent`,
`validate_connection` of a group reference), `two_way_reference`
(`TAG_GROUP_IIOP` or the gateway profile), `Goa` with
`create_id_for_reference`, `reference_to_ids`,
`associate_reference_with_id`, `disassociate_reference_with_id`,
`activate_object_with_id`; the GOA joins the group and delivers every
request to all servants of the group; requests expecting a reply,
messages that are no request and groups without a member are counted in
`GoaStats`. `MiopGateway` forwards the requests of IIOP clients to the
group (`void` operations with an empty reply); the operations of
`CORBA::Object` go to the IIOP profile of `TAG_GROUP_IIOP` in the UIPMC
profile, without that component `_non_existent` is `true` and the others
are `NO_IMPLEMENT`; a `LocateRequest` gets `OBJECT_HERE` for a target
with a group profile, else `UNKNOWN_OBJECT`; a header it cannot read, a
`Reply`, a `LocateReply` and a `Fragment` without a fragmented message
before it get `MessageError`, then the connection ends; `CancelRequest`
is counted (`GatewayStats`).
`crates/corba-interop/src/mgm.rs` — `Mgm` with the PropertyManager,
`get_object_group_id`, `get_object_group_ref` (the other
ObjectGroupManager operations `NO_IMPLEMENT` after §11.2.6.5),
`create_object` and `delete_object` with the exceptions of their
`raises` clauses (criteria `create_object` does not understand are
`InvalidCriteria`, properties with which no reference can be made are
`InvalidProperty` in `set_properties_dynamically`), every property of
§11.2.6.8 (`GroupCreationMode` at creation only, `GroupDomainId` once,
endpoints and gateway at any time, a new reference version when the
reference changes), an address pool for `CREATE_ADDRESS_GENERATED`.
`Mgm::serve` makes the MGM a CORBA object on a `CorbaServer`: every
operation of `MGM::ObjectGroupFactory` and `CORBA::Object` over IIOP,
exceptions with their members; per group an object under
`<key>/group/<id>` at the same address that answers `_is_a` for the type
of the group and its base interfaces (interface repository) and
`_non_existent` = `false`, after `delete_object` `_is_a` = `false` and
`_non_existent` = `true` (§11.3.2). A nil `SupportImplicitOperations`
puts that object into `TAG_GROUP_IIOP`; an MGM without `serve` has no
reference and refuses it (`CannotMeetCriteria`, in
`set_properties_dynamically` `InvalidProperty`). `MgmProxy` calls an MGM
over IIOP. The 14-octet
header, `MiopConfig` and `MiopSender` of
`crates/corba-ccm/src/orb_extensions.rs` are removed.

**Tests:** `crates/corba-iiop/src/miop.rs::tests::{corba_p2_11_1_4_packet_header_layout_and_alignment,
corba_p2_11_1_4_malformed_packets_are_refused,
corba_p2_11_1_3_packetize_splits_into_equal_packets_and_a_short_last,
corba_p2_11_1_3_collection_reassembles_in_any_order,
corba_p2_11_1_6_2_a_stop_packet_with_packets_missing_drops_the_collection,
corba_p2_11_1_6_2_duplicates_and_contradictions_are_reported,
corba_p2_11_1_6_2_the_timeout_counts_from_the_last_packet_of_a_collection,
corba_p2_11_1_4_a_packet_number_at_or_beyond_the_count_of_its_collection_is_refused,
corba_p2_11_1_6_2_the_pending_collections_are_bounded_in_octets}`,
`crates/corba-iiop/tests/miop.rs::{corba_p2_11_1_5_every_receiver_of_the_ipv4_group_gets_the_collection,
corba_p2_11_1_5_a_receiver_of_another_group_on_the_same_port_gets_nothing,
corba_p2_11_1_5_the_ipv6_group_gets_the_collection,
corba_p2_11_1_5_joining_needs_a_multicast_address_and_a_matching_interface,
corba_p2_11_1_6_2_a_foreign_datagram_on_the_group_port_is_counted}`,
`crates/corba-ior/src/miop.rs::tests::{corba_p2_11_2_2_uipmc_profile_round_trips,
corba_p2_11_2_2_4_group_info_layout,
corba_p2_11_2_3_a_group_reference_and_its_profiles,
corba_p2_11_2_6_10_is_equivalent_and_hash_of_group_references,
corba_p2_11_2_7_corbaloc_miop_url_parses,
corba_p2_11_2_7_malformed_miop_urls_are_refused,
corba_p2_11_2_2_2_multicast_addresses}`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p2_11_2_7_string_to_object_makes_a_group_reference_of_corbaloc_miop`,
`crates/corba-interop/tests/miop.rs::{corba_p2_11_3_a_oneway_request_reaches_every_servant_of_the_group,
corba_p2_11_2_4_2_a_disassociated_id_gets_nothing_and_can_be_associated_again,
corba_p2_11_2_4_1_a_reference_without_a_group_profile_is_not_a_group_object,
corba_p2_11_3_1_2_giop_1_0_and_1_1_requests_carry_the_profile_in_the_object_key,
corba_p2_11_2_6_one_endpoint_carries_two_groups,
corba_p2_11_1_3_a_collection_that_is_no_oneway_request_is_counted_not_dispatched,
corba_p2_11_2_5_the_gateway_forwards_the_requests_of_an_iiop_client,
corba_p2_11_2_2_4_2_two_way_calls_use_the_group_iiop_profile,
corba_p2_11_2_6_10_object_operations_on_a_group_reference,
corba_p2_11_2_5_the_gateway_answers_locate_requests_and_refuses_what_it_cannot_take,
corba_p2_11_2_6_10_the_gateway_forwards_object_operations_to_the_group_iiop_profile}`,
`crates/corba-ir/src/repository.rs::tests::corba_p1_14_5_24_1_is_a_follows_the_base_interfaces`,
`crates/corba-interop/tests/mgm.rs::{corba_p2_11_2_6_6_create_object_makes_a_group_reference,
corba_p2_11_2_6_8_1_the_creation_mode_and_the_endpoints_must_agree,
corba_p2_11_2_6_4_properties_come_from_the_most_specific_place,
corba_p2_11_2_6_3_unknown_repeated_and_mistyped_properties_are_refused,
corba_p2_11_2_6_8_3_implicit_operations_go_into_the_group_iiop_component,
corba_p2_11_3_3_1_a_group_the_mgm_created_is_reached_over_multicast,
corba_p2_11_3_3_1_1_the_mgm_is_called_over_iiop}`,
`crates/corba-interop/tests/miop_cross_orb.rs::{corba_p2_11_a_jacorb_goa_member_receives_what_zerodds_multicasts,
corba_p2_11_a_zerodds_goa_member_receives_what_jacorb_multicasts}`.

**Status:** partial — §11.1 to §11.3 built, tests green on the Linux test
host (multicast over 239.255.x.x and ff15::/16 in the network namespace
of the test run), among them against JacORB 3.9 in both directions: a
JacORB GOA member receives the oneway requests ZeroDDS sends to the
group, a ZeroDDS GOA member those of JacORB, one request in one packet
and one in a collection of several packets; ZeroDDS reads JacORB's group
reference, JacORB the `corbaloc:miop` URL. Not run: TAO (not on the test
host); omniORB has no MIOP. Open: §11.3.4.3 — the runtime has no initial
reference `RootPOA` to narrow to `GOA`; the GOA is made with `Goa::new`.

### §12 ZIOP Protocol

**Spec:** Part 2 §12, p. 219 — the ZIOP message (magic `ZIOP`,
`CompressionData` in place of the GIOP body, GIOP 1.2 and later,
fragments included, §12.1), ZIOP only towards a peer that declared it
(§12.2), the four compression policies (§12.3) and their propagation in
`TAG_POLICIES` and `INVOCATION_POLICIES` (§12.4, Part 1 §17.4).

**Repo:** `crates/corba-giop/src/ziop.rs` (`CompressionData`,
`compress_message`, `decompress_message`, `ZiopCodec`, `ZiopPolicies`,
`PeerZiop`, `ZiopSend`, policy types 64 to 67 with their `pvalue`
formats), `crates/corba-giop/src/messaging.rs` (`PolicyValue`,
`TAG_POLICIES`, `INVOCATION_POLICIES`, Part 1 §17.4),
`crates/corba-giop/src/header.rs::MessageHeader::decode_frame` (magic
`ZIOP` from GIOP 1.2), `crates/corba-iiop/src/framing.rs::{read_giop_message_ziop,
write_giop_message_ziop}`, `crates/corba-iiop/src/connection.rs::Connection::{set_ziop_codec,
write_message_ziop}`, `crates/corba-ccm/src/compression.rs`
(`CompressionManager` as `ZiopCodec`), `crates/corba-ccm/src/orb_core.rs::{ziop_policy,
ziop_policies, apply_ziop_policies, client_ziop_policies}`
(`Orb::create_policy` for the types 64 to 67, `Orb::{client_ziop_policies,
server_ziop_policies}`), `crates/corba-ccm/src/policy_management.rs`
(overrides at the ORB, thread and reference level, Part 1 §8.9),
`crates/corba-interop/src/runtime.rs` (`IiopCorbaConnection::with_ziop`,
`CorbaServer::{set_ziop, object_reference}`). The client policies of an
invocation: per ZIOP policy type the override of the reference
(`Object::set_policy_overrides`), else of the calling thread
(`PolicyCurrent`), else of the ORB (`ORBPolicyManager`), over the own
policies of `with_ziop` or the ORB defaults; the compressors of
`with_ziop`, else the ORB's `CompressionManager`. A client that enabled
compression once declares it disabled for an invocation without ZIOP
policies, because the server keeps the last declaration for the
connection. A `CorbaServer` without `set_ziop` that is bound to an ORB
takes the ZIOP overrides at the ORB level with that ORB's
`CompressionManager`. The client sends its
client-exposed policies (64, 65) in every request as
`INVOCATION_POLICIES`. A request goes out as a ZIOP message only when the
reference enables ZIOP in `TAG_POLICIES` and names a compressor in
common: the first entry of the own list that the other side lists, at the
lower level (§12.3.2). The server reads ZIOP requests only when it
enabled ZIOP; otherwise it answers with `MessageError` and closes the
connection. The policies of a client hold for its connection until it
sends others (omniORB sends them in the first request only). The
sender's low value and minimum ratio keep a message GIOP; without a
minimum ratio a message goes out compressed only when the ZIOP message is
shorter. ORB defaults: compression off, zlib level 6, low value 1024
octets. A `TAG_POLICIES` component that does not decode is `INV_OBJREF`,
such an `INVOCATION_POLICIES` context `MARSHAL`.

**Tests:** `crates/corba-giop/src/ziop.rs::tests::{corba_p2_12_1_ziop_message_layout,
corba_p2_12_1_the_header_accepts_ziop_from_giop_1_2_only,
corba_p2_12_1_a_fragment_message_is_compressed_with_its_flags,
corba_p2_12_3_low_value_and_min_ratio_keep_a_message_uncompressed,
corba_p2_12_1_a_ziop_body_is_checked_before_and_after_decompression,
corba_p2_12_2_no_ziop_to_a_peer_without_the_policies,
corba_p2_12_3_2_the_first_common_compressor_at_the_lowest_level,
corba_p2_12_4_client_exposed_policies_round_trip_as_policy_values}`,
`crates/corba-giop/src/messaging.rs::tests::{corba_p1_17_4_policy_value_seq_layout,
corba_p1_17_4_little_endian_policy_values_round_trip,
corba_p1_17_4_a_count_beyond_the_octets_is_refused,
corba_p1_17_4_invocation_policies_is_service_context_7}`,
`crates/corba-iiop/src/framing.rs::tests::{corba_p2_12_1_a_ziop_request_is_read_back_as_its_giop_message,
corba_p2_12_1_each_fragment_is_a_ziop_message_of_its_own,
corba_p2_12_1_a_fragment_the_low_value_keeps_giop_is_mixed_with_ziop_ones,
corba_p2_12_2_a_ziop_message_without_a_codec_is_refused}`,
`crates/corba-ccm/src/compression.rs::tests::corba_p2_12_1_the_compression_manager_is_the_ziop_codec`,
`crates/corba-ccm/src/orb_core.rs::tests::{corba_p2_12_3_create_policy_makes_the_four_ziop_policies,
corba_p1_8_9_1_policy_manager_and_policy_current_are_initial_references}`,
`crates/corba-interop/tests/ziop.rs::{corba_p2_12_2_a_request_and_its_reply_travel_as_ziop_messages,
corba_p2_12_2_no_ziop_to_a_peer_without_the_policies,
corba_p2_12_3_low_value_and_min_ratio_keep_a_message_uncompressed,
corba_p2_12_1_a_oneway_request_is_compressed_too,
corba_p2_12_2_a_ziop_request_to_a_server_without_ziop_is_a_message_error,
corba_p2_12_2_invocation_policies_that_do_not_decode_are_marshal,
corba_p2_12_2_a_broken_tag_policies_component_is_inv_objref,
corba_p2_12_2_the_client_policies_hold_for_the_connection,
corba_p2_12_2_client_ziop_policies_at_the_orb_level_through_create_policy,
corba_p2_12_2_client_ziop_policies_at_the_thread_level,
corba_p2_12_2_client_ziop_policies_at_the_reference_level}`,
`crates/corba-interop/tests/omniorb_cross_orb.rs::{corba_p2_12_zerodds_sends_ziop_to_an_omniorb_server_and_reads_its_ziop_replies,
corba_p2_12_an_omniorb_client_sends_ziop_to_a_zerodds_server_and_reads_its_ziop_replies}`.

**Status:** done — tests green on the Linux test host, among them
against omniORB 4.3.3 (omniZIOP, zlib) in both directions: requests and
replies went over the wire as ZIOP messages (seen by a relay) and were
read on the other side. The client policies at the ORB, thread and
reference level (§12.2) through the overrides of Part 1 §8.9, set only
through `create_policy`, each level checked with a relay, as is the
precedence reference before thread before ORB; a server bound to an ORB
declares and uses the ZIOP policies of the ORB level. Not run: TAO (not on the test host)
and JacORB (no ZIOP peer written).

---

## Part 3: Component Model (CCM)

### §1, §3–§5 Scope/References/Terms/Symbols

**Spec:** Part 3 §1, §3–§5, pp. 1-7 — scope, references, terms,
symbols.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — scope, reference lists and terms.

### §2 Conformance and Compliance

**Spec:** Part 3 §2, pp. 1–3 — compliance points: Basic Level (for
non-Java products IDL extensions, CIDL without segments, a container,
XML deployment descriptors), Java Basic Level (EJB 1.1, RMI/IIOP,
EJB-to-IDL), Extended Level, Lightweight CCM profile, EJB clients
(optional), Generic Interaction Support (`porttype`, `mirrorport`,
`port`, templates, connectors, D&C extensions).

**Repo:** `crates/corba-ccm/src/container.rs`, `crates/corba-ccm/src/cidl.rs`,
`crates/corba-ccm-ejb/`, `crates/corba-ccm-lib/`.

**Tests:** `crates/corba-ccm/src/container.rs::tests::install_and_full_lifecycle_round_trip`.

**Status:** partial — the conformance points rest on the items of
Part 3 and follow their status: point 1 (changes to the Life Cycle,
Transaction and Security Services) on §6.13 (partial: the Security part
is missing); points 2 and 4 (Basic level: IDL extensions, CIDL,
container, XML descriptors) on §6, §8, §9, §10 and §15 — CIDL with its
parser is built (§8, done), missing is the rest of §9 (partial); points 3 and 6 (Extended level) on §6 to §10 and
§15; points 5 and 8 (Java Basic level, EJB clients) on §11 (done); point
7 (Lightweight profile) on §14 (done, `crates/corba-ccm/src/lwccm.rs`);
point 9 (Generic Interaction Support with the D&C extension) on §7
(done). A test that ties each point to its evidence
(`crates/corba-ccm/tests/conformance_points.rs`) is not built: the
CIDL parser it needs exists (§8); points 2 to 4 also need the CIF of §9,
which is partial.

### §6 Component Model

**Spec:** Part 3 §6, p. 9 — component definition, facets, receptacles,
events, homes, home finders, configuration, inheritance, conformance.

**Repo:** `crates/corba-ccm/src/component_def.rs`,
`crates/corba-ccm/src/home.rs`, `crates/corba-ccm/src/port.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm/src/lifecycle.rs`.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests` (30 tests), among them
`trader_has_4_ports`, `keyed_component_detected`,
`provide_facet_returns_facet_by_name`, `get_all_facets_returns_complete_list`,
`get_all_publishers_excludes_emit_only_sources`,
`supports_interface_returns_true_for_listed_iface`,
`the_base_chain_runs_from_the_derived_component_to_the_root`,
`flattening_takes_over_every_inherited_feature_bases_first`,
`a_redefined_port_name_is_refused_rather_than_shadowed`,
`a_second_primary_key_in_the_chain_is_refused`;
`crates/ast/src/cidl.rs::tests::corba_p3_8_5_1_session_and_service_forbid_binding_primary_key_finders_and_segments`;
the sub-items §6.5-§6.13 name one test per clause each.

**Status:** done

#### §6.5 Facets and Navigation

**Spec:** §6.5, p. 13 — `provide_facet()`, navigation.

**Repo:** `crates/corba-ccm/src/component_def.rs`
(`provide_facet`/`get_all_facets`/`get_named_facets` — the navigation
interface), `crates/corba-ccm/src/lifecycle.rs` (`check_facet_lifetime`
— facet reference lifetime).

**Tests:** `crates/corba-ccm/src/component_def.rs::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown, get_all_facets_returns_complete_list,
get_named_facets_filters_by_names}`,
`crates/corba-ccm/src/lifecycle.rs::{facet_lifetime_passes_when_alive,
facet_lifetime_rejects_use_after_destroy, facet_lifetime_rejects_orphaned}`.

**Status:** done

#### §6.6 Receptacles

**Spec:** §6.6, p. 20 — `connect_*`/`disconnect_*`, multiple
receptacles.

**Repo:** `crates/corba-ccm/src/port.rs` (`PortRegistry::connect_receptacle`/
`disconnect_receptacle`/`get_connections`), `crates/corba-ccm/src/lifecycle.rs`
(`ReceptacleManager` with the simplex/multiplex rule),
`crates/corba-ccm/src/component_def.rs` (`ReceptacleMultiplicity`,
`get_all_receptacles`/`get_named_receptacles`).

**Tests:** `crates/corba-ccm/src/port.rs::{connect_and_get_connections,
disconnect_removes_only_target_id, disconnect_unknown_returns_false}`,
`crates/corba-ccm/src/lifecycle.rs::{receptacle_simplex_connect_then_disconnect,
receptacle_simplex_double_connect_rejected,
receptacle_multiplex_allows_multiple_connects,
receptacle_disconnect_unknown_id_rejected, receptacle_double_disconnect_rejected}`,
`crates/corba-ccm/src/component_def.rs::{receptacle_multiplicity_distinct,
get_all_receptacles_returns_complete_list, get_named_receptacles_filters}`.

**Status:** done

#### §6.7 Events

**Spec:** §6.7 — EventSource (publishers/emitters) and EventSink.

**Repo:** `crates/corba-ccm/src/port.rs` (event ports, `EventStream`),
`crates/corba-ccm/src/component_def.rs` (`EventSourceDef.emit_only`
separates `emits` from `publishes`; `EventSinkDef`).

**Tests:** `crates/corba-ccm/src/port.rs::{event_sink_subscription_round_trip,
list_event_sinks_unknown_source_empty}`,
`crates/corba-ccm/src/component_def.rs::{event_source_emit_only_default_false,
get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources,
get_named_publishers_respects_emit_only_flag,
get_named_emitters_excludes_publishers}`.

**Status:** done — event-port API live; wire path for emitters goes
through CosEvent (see `cos-event-service-1.4.md`).

#### §6.8 Homes / §6.9 Home Finders

**Spec:** §6.8/§6.9, pp. 34/42 — home lifecycle, HomeFinder.

**Repo:** `crates/corba-ccm/src/home.rs` (`HomeDef` with an optional
primary key, `HomeFinder` with `create`/`find_by_primary_key`/`remove`).

**Tests:** `crates/corba-ccm/src/home.rs::{home_def_default_has_no_primary_key,
keyed_home_has_primary_key, create_with_user_key_uses_key_as_id,
create_without_key_generates_system_id, find_by_primary_key_round_trip,
remove_decrements_instance_count, remove_unknown_returns_false,
home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done

#### §6.10 Component Configuration / §6.11 Attributes

**Spec:** §6.10/§6.11 — configurator API, attribute set.

**Repo:** `crates/corba-ccm/src/lifecycle.rs` (`Configurator` trait,
`StandardConfigurator` over the `AttributeDef` set,
`ConfiguratorRegistry`), `crates/corba-ccm/src/context.rs`
(`ComponentContext`), `crates/corba-ccm/src/component_def.rs`
(`AttributeDef` with `readonly`/`set_raises`/`get_raises`).

**Tests:** `crates/corba-ccm/src/lifecycle.rs::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute, configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`,
`crates/corba-ccm/src/component_def.rs::tests::attribute_readonly_skips_set_raises`,
`crates/corba-ccm/src/context.rs::{anonymous_context_returns_none,
principal_context_returns_bytes}`.

**Status:** done

#### §6.12 Component Inheritance

**Spec:** §6.12, p. 49 — inheritance of component definitions.

**Repo:** `crates/corba-ccm/src/component_def.rs::{ComponentTypeTable,
InheritanceError}` — `base_chain` walks the `base_component` links from
the derived type to the root, `is_equivalent_component_kind` checks a
repository ID against the whole chain, and `flatten` takes over the
facets, receptacles, event ports, attributes, `supports` interfaces and
primary key of the bases while refusing a redefined port or attribute
name and a second primary key.

**Tests:** `crates/corba-ccm/src/component_def.rs::{the_base_chain_runs_from_the_derived_component_to_the_root,
a_derived_component_is_also_of_every_kind_it_inherits,
flattening_takes_over_every_inherited_feature_bases_first,
an_unknown_base_is_reported_against_the_component_that_names_it,
an_unregistered_repository_id_is_not_a_component,
a_cycle_in_the_base_links_stops_the_walk,
a_redefined_port_name_is_refused_rather_than_shadowed,
a_second_primary_key_in_the_chain_is_refused,
a_keyed_base_makes_the_derived_component_keyed}`.

**Status:** done

#### §6.13 Conformance Requirements

**Spec:** §6.13, p. 51 — ten conformance points. Point 1 with §6.13.2:
`CosLifeCycle::FactoryFinder::find_factory` (the key as an INS name to
`HomeFinder::find_home_by_name`, the default factory of the home as a
`GenericFactory`), `CosTransactions::Current` and the listed interfaces
of `SecurityLevel1`, `SecurityLevel2` and `SecurityReplacable` as local
interfaces. Points 2 to 9: the Basic and Extended levels, Lightweight
CCM and EJB access. Point 10: every product parses any valid IDL.

**Repo:** `crates/corba-ccm/src/cos_lifecycle.rs` (`FactoryFinder` with
`find_factory` and `find_factories` over
`ComponentHomeFinder::{find_home_by_name, homes_by_name}`,
`GenericFactory`, `HomeFactory` as the default factory of a home without
a primary key, `NoFactory`, `InvalidCriteria`, `Key` = `CosNaming::Name`
in INS form); `crates/corba-cos-transactions/src/current.rs::TransactionCurrent`
(local `CosTransactions::Current`, one `Current` per thread) as the
ORB's initial reference `"TransactionCurrent"`
(`crates/corba-ccm/src/orb_core.rs::orb_init`); a local object has no
reference, `object_to_string` gives `MARSHAL` minor 4. Point 10:
`zerodds-idl-parse` and `zerodds-idl-lower` read the grammar of Part 3
(component, home with factory, finder and primary key, eventtype, every
port kind, `porttype`, `port`, `mirrorport`, `connector`, template
modules and their instances, `typeprefix`, `typeid`) in every dialect
without an error, neither in parsing nor in lowering. Exception: a
connector with an empty body. Part 3 rule (6) allows it
(`<connector_export>*`), OMG IDL 4.2 rule (180), the newer text, asks
for at least one export (`+`); lowering follows IDL 4.2 and refuses it in
every dialect (`IDL0001`).

**Tests:** `crates/corba-ccm/src/cos_lifecycle.rs::tests::{corba_p3_6_13_2_1_find_factory_goes_through_the_home_finder,
corba_p3_6_13_2_1_no_factory_without_a_keyless_home_bound_to_the_key,
corba_p3_6_13_2_1_find_factories_returns_every_keyless_home_of_the_name,
corba_p3_6_13_2_1_the_default_factory_takes_no_criteria_and_no_other_key}`,
`crates/corba-cos-transactions/src/current.rs::tests::{corba_p3_6_13_2_2_each_thread_has_its_own_transaction_context,
corba_p3_6_13_2_2_two_transaction_currents_do_not_share_a_context,
corba_p3_6_13_2_2_set_timeout_reaches_the_next_transaction}`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p3_6_13_2_2_transaction_current_is_a_local_initial_reference`,
`crates/idl-lower/tests/dialects.rs::corba_p3_6_13_point_10_the_component_grammar_parses_in_every_dialect`.

**Status:** partial — point 1 (Life Cycle, Transaction) and point 10
built, tests green on the Linux test host. Security Service: ZeroDDS
ships no CORBA Security Service (`SecurityLevel1`, `SecurityLevel2`,
`SecurityReplacable`); the security of the ORB layer is CSIv2 (Part 2
§10). The change of §6.13.2.3 binds the vendor of that service. Open:
points 2 to 9 are the conformance points of Part 3 §2 and are proven
there (open there: CIF §9).

### §7 Generic Interaction Support (IDL3+)

**Spec:** Part 3 §7, p. 55 — extended ports (`porttype`, `port`,
`mirrorport`) and their translation into basic ports (§7.2.2.2),
connectors with attributes, inheritance and composite implementations
(§7.2.3), template modules (§7.3, §7.4), the programming model of
fragments (§7.5: `CCMObject` with Navigation and Receptacles,
`KeylessCCMHome`, `HomeConfiguration`, `configuration_complete`), the
D&C extension (§7.6: `ConnectorPackageDescription`,
`ConnectorImplementationDescription`, the port kinds `ExtendedPort` and
`ExtendedMirrorPort` with `templateParam`, an assembly connection through
a connector, flattening into fragment instances, the execution
parameter "home factory", the configuration of fragments) and the
metamodel classes `ExtendedPortType`, `ExtendedPortDef`, `ConnectorDef`
(§7.6.5).

**Repo:** IDL3+: `crates/idl-parse/src/decl.rs` (`porttype`, `port`,
`mirrorport`, `connector` with inheritance), `crates/idl-lower`
(template modules, instances, `alias`; `src/lower.rs::{bind_actual,
constraint_violation}` checks each actual parameter against the type
constraint of its formal), `crates/ccm/src/transform.rs::{expand_extended_ports,
transform_connector}` (equivalent IDL). Runtime model:
`crates/corba-ccm/src/connector.rs` (`PortTypeDef`, `ConnectorDef`,
`ConnectorTypeTable::basic_ports` per §7.2.2.2 and §7.2.3.3,
`ConnectorTypeTable::port_types_match` per §7.6.5);
`crates/corba-ccm/src/fragment.rs` (`ConnectorFragment` as
`CCMObject`, `ConnectorHome` with `create_component`,
`remove_component`, `set_configuration_values`, `FragmentExecutor`,
`ConnectorHomeExecutor` as `HomeExecutorBase`, `HomeFactories`,
`LocalFacet` for the type check of `connect`);
`crates/corba-ccm/src/orb_core.rs::IfrCcmMetamodel::{add_port_type,
add_extended_port, add_connector}`. D&C: `crates/corba-dnc/src/connector.rs`
(data model of §7.6.2 with `Capability`, `SatisfierProperty`,
`Requirement` and `ConnectorPackageDescription::select_implementation`,
`flatten_assembly` per §7.6.3),
`crates/corba-dnc/src/property.rs` (typed `configProperty`),
`crates/corba-dnc/src/xml.rs` (reads and writes the plan, `.ccd`, the
connector package with `<capability>` and the assembly with
`<selectRequirement>`),
`crates/corba-dnc/src/launch.rs::{PlanLauncher, launch_plan}` (entry
point "home factory", `set_configuration_values`, provided references
per node, basic connections across nodes, `configuration_complete`,
teardown).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{idl_7_4_11_4_2_an_extended_port_is_the_basic_ports_of_its_port_type,
idl_7_4_11_4_3_a_connector_maps_through_its_ports_as_a_component,
corba_p3_7_2_3_3_a_derived_connector_inherits_the_ports_of_its_base}`,
`crates/idl-lower/tests/dialects.rs::{an_instantiation_lowers_the_body_with_the_actual_substituted,
two_instantiations_of_one_template_are_two_independent_modules,
an_alias_hands_the_enclosing_templates_parameter_on,
an_instantiation_with_the_wrong_number_of_actuals_is_reported,
corba_p3_6_13_point_10_the_component_grammar_parses_in_every_dialect,
corba_p3_7_3_2_1_each_constraint_accepts_the_kind_it_names,
corba_p3_7_3_2_1_each_constraint_refuses_another_kind,
corba_p3_7_3_2_1_an_alias_hands_on_only_what_the_inner_constraint_admits,
corba_p3_7_3_2_1_an_interface_actual_is_bound_in_the_instance,
corba_p3_7_3_2_1_an_exception_actual_is_raised_by_the_instance}`,
`crates/corba-ccm/src/connector.rs::tests::{corba_p3_7_2_2_2_a_mirrorport_turns_provides_into_uses_and_names_the_basic_ports,
corba_p3_7_2_2_2_a_port_keeps_the_direction_of_its_basic_ports,
corba_p3_7_2_3_3_a_derived_connector_has_the_ports_and_attributes_of_its_base,
corba_p3_7_2_3_3_a_redefined_name_an_unknown_base_and_a_cycle_are_refused,
corba_p3_7_6_5_port_types_match_one_by_one}`,
`crates/corba-ccm/src/fragment.rs::tests::{corba_p3_7_5_a_fragment_answers_navigation_and_receptacles,
corba_p3_7_5_connect_checks_the_type_of_the_object,
corba_p3_7_5_a_multiplex_receptacle_hands_out_cookies,
corba_p3_7_5_4_configuration_values_reach_every_new_fragment,
corba_p3_7_5_2_configuration_complete_and_remove,
corba_p3_7_6_3_1_home_factories_by_entry_point_name,
corba_p3_7_5_exceptions_carry_the_components_repository_ids}`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p3_7_6_5_metamodel_has_extended_port_types_ports_and_connectors`,
`crates/corba-dnc/src/connector.rs::tests::{corba_p3_7_6_3_an_assembly_with_a_connector_flattens_to_fragment_instances,
corba_p3_7_6_3_flattening_refuses_what_has_no_plan,
corba_p3_7_6_3_a_basic_connection_and_a_direct_extended_port_link,
corba_p3_7_2_3_4_a_composite_connector_flattens_through_its_fragment_assemblies,
corba_p3_7_2_3_4_a_component_part_of_a_connector_assembly_is_deployed_with_the_component_it_serves,
corba_p3_7_6_2_3_selection_requirements_choose_among_implementations,
corba_p3_7_6_2_3_each_satisfier_kind_meets_a_requirement_by_its_rule}`,
`crates/corba-dnc/src/property.rs::tests::{corba_p3_7_6_4_typed_property_values_round_trip_through_text,
corba_p3_7_6_4_a_value_out_of_range_or_of_an_unknown_type_is_refused}`,
`crates/corba-dnc/src/xml.rs::tests::{corba_p3_7_6_4_plan_xml_carries_typed_config_properties_and_exec_parameters,
corba_p3_7_6_4_an_instance_names_its_implementation_by_reference_or_by_label,
corba_p3_7_6_2_4_extended_port_kinds_and_template_params_in_xml,
corba_p3_7_6_2_connector_package_and_assembly_round_trip_through_xml,
corba_p3_7_6_2_3_capabilities_and_selection_requirements_round_trip_through_xml}`,
`crates/corba-dnc/src/launch.rs::tests::{corba_p3_7_6_3_1_home_factory_entry_point_creates_the_fragment,
corba_p3_7_6_4_fragment_config_properties_reach_set_configuration_values,
corba_p3_7_2_3_4_launchers_per_node_connect_the_fragments_of_a_composite_connector}`.

**Status:** done — all parts built, tests green on the Linux test host.
Template modules: each actual parameter is checked against the type
constraint of its formal (§7.3.2.1: `typename`, `interface`,
`valuetype`, `eventtype`, `struct`, `union`, `sequence`, `enum`, `const`
of a type, a sequence specification; `exception` per IDL 4.2
§7.4.12.4.1); a `typedef` counts as the type it names. A violation is a
diagnostic (`IDL0019`), also for what an `alias` hands on. An interface
as the actual is an object reference and a base in the body, an
exception stands in `raises`. A monolithic implementation gives one
fragment per component it serves, on that component's node; an assembly
implementation places each part with the component whose connector port
is mapped to it, and nested parts with that part; a part no component
reaches is refused. A part is a connector fragment or a component
(§7.2.3.4); a component part is placed and configured like the
fragments beside it and created by the container. Planning takes the
first implementation of a package whose capabilities satisfy the
selection requirements of the use or of the part (§7.6.2.3, §15.9.1);
none satisfying them is an error. The six kinds of `SatisfierProperty`
check the types of §15.4.5. The direction of `Minimum` (the requirement
at least the value) and `Maximum` (the requirement at most the value) is
taken from D&C 4.0 and not checked against its text, which is not in the
house. Component implementations carry no capabilities in this data
model; a component instance with selection requirements is refused. The
launch follows the D&C split: `start_launch` returns per node the facet
references of the connections provided there, `finish_launch` connects
the node's receptacles to facets on other nodes too, and `launch_plan`
thus makes each connection of the plan exactly once; a missing reference
of another node is an error. The test runs the launchers of both nodes
in one process. The XML is the descriptor form of `crates/corba-dnc`
(elements after the class and field names of §7.6.2 and §16), not the
XMI schema of §17 (an item of its own); the plan folds
`ArtifactDeploymentDescription` into the implementation's artifact list.
An instance names its implementation as text or by reference as the
example of §7.6.4 does (`<implementation ref="fragment_impl_1"/>`, also
`xmi:idref`). No proof against a foreign D&C tool (CIAO DAnCE is not in
the house).

### §8 OMG CIDL Syntax and Semantics

**Spec:** Part 3 §8, p. 81 — Component Implementation Definition
Language (composition, home/segment executor, persistence,
facet/feature delegation, proxy home).

**Repo:** `crates/ast/src/cidl.rs::{CidlModel, Composition,
CompositionCategory, HomeExecutorDef, ExecutorDef, SegmentDef,
FeatureDelegation, Delegation, ProxyHomeDef, HomeFacts,
StorageHomeFacts, StorageDecl, Composition::validate}` — one data model
for the compiler and the runtime: the composition with its home
executor (`implements`, `bindsTo`, `storedOn`, executor, delegations,
`abstract`), segments with their segment id (the component segment is
0), feature delegation, proxy home and the PSDL declarations;
`validate` checks the rules that read names: Tables 8.3 to 8.5 against
the primary key, explicit finders and segments, §8.11 (segment names
and ids distinct, not 0), §8.13 (a facet of the component, in at most
one segment), §8.14 (only with `bindsTo`, a stateful feature, the
storage member exists, one delegation per feature), §8.15 (only with
`bindsTo`, a home operation, a target on the storage home, one target
per operation and one operation per target), §8.16 (factories only,
explicit or `create`), §8.17 and §8.18 (home operations).
`crates/corba-ccm/src/cidl.rs` re-exports the model and builds the
facts from `HomeDef` and `ComponentDef` with `home_facts`;
`crates/corba-ccm/src/lwccm.rs::composition_exclusion` reads it
(persistence, segmentation, CIDL after LwCCM §13).

**Tests:** `crates/ast/src/cidl.rs::tests::{corba_p3_8_5_1_session_and_service_forbid_binding_primary_key_finders_and_segments,
corba_p3_8_5_1_process_forbids_a_primary_key,
corba_p3_8_5_1_entity_requires_a_primary_key,
corba_p3_8_13_a_facet_belongs_to_one_segment_and_the_rest_to_the_component_segment,
corba_p3_8_11_segment_names_and_ids_are_distinct_and_not_zero,
corba_p3_8_14_feature_delegation_names_a_bound_stateful_feature_and_a_member,
corba_p3_8_15_storage_home_delegation_targets_one_operation_each_way,
corba_p3_8_16_only_factories_delegate_to_the_executor,
corba_p3_8_17_8_18_abstract_and_proxy_operations_are_home_operations,
corba_p3_8_5_the_category_keywords_round_trip}`,
`crates/corba-ccm/src/cidl.rs::tests::corba_p3_8_5_1_an_entity_composition_needs_a_keyed_home_at_run_time`,
`crates/corba-ccm/src/lwccm.rs::tests::{an_entity_composition_is_excluded_as_persistence,
a_session_composition_is_still_excluded_because_cidl_is_gone}`.

**Status:** done — built and green on the Linux test host. The model
lives in `zerodds-ast` because the compiler (`zerodds-idl-lower`)
produces it and cannot depend on `zerodds-corba-ccm`;
`zerodds-corba-ccm` re-exports it. The rules that read types (Table
8.6, the signatures of §8.15) are the compiler's; they are under the
item "§8 OMG CIDL — Remaining Constructs". The runtime's `HomeDef`
knows no explicit operations; a caller that knows them adds them to the
facts.

#### §8 OMG CIDL — Remaining Constructs

**Spec:** Part 3 §8, p. 81 — the CIDL grammar itself (§8.2 to §8.18,
productions (1) to (34)) with the PSDL declarations it contains
(Persistent State Service 2.0 §3.2.3, productions (1) to (48)), the
keywords of Table 8.2, `.cdl` files (§8.1) and the rules of the
categories, segments, feature, storage home and executor delegation,
`abstract` and the proxy home.

**Repo:** `crates/idl-cst/src/token.rs::{CIDL_KEYWORDS, cidl_keyword,
cidl_keyword_collision}` and the CST kinds in `crates/idl-cst/src/tree.rs`;
`crates/idl-parse/src/dialect.rs::Dialect::cidl`,
`crates/idl-parse/src/parser.rs::Parser::mark_cidl_keywords`,
`crates/idl-parse/src/cidl.rs` (the parser of CIDL and PSDL),
`crates/idl-parse/src/diagnostic.rs` (`IDL0024`, `CompositionRule`);
`crates/idl-lower/src/cidl.rs` and `crates/idl-lower/src/lower.rs::lower_cidl`
(PSDL declarations as local interfaces of the model; name resolution;
Table 8.6; the signature rules of §8.15 with the orthodox operations of
§6.8.1; the name and member rules of PSS 2.0 §3.2.5 and §3.2.6; the
keyword collision by case); `crates/idl-compose/src/lib.rs::Composed::cidl`;
`tools/idlc` (`.cdl` is read as CIDL, `dump-cidl`).

**Tests:** `crates/idl-parse/tests/cidl.rs::{corba_p3_8_3_every_production_parses,
corba_p3_8_2_1_keywords_are_identifiers_outside_cidl_and_escapable_inside,
corba_p3_8_14_both_spellings_of_a_feature_delegation_parse,
corba_p3_9_2_9_1_the_schematic_segment_form_parses_too,
corba_p3_8_5_a_composition_needs_its_category_and_its_home_executor,
pss_3_2_5_10_an_anonymous_sequence_is_no_state_type,
pss_3_2_5_4_const_is_for_a_storage_type_operation,
corba_p3_8_6_each_place_takes_only_its_kinds_of_delegation,
corba_p3_8_6_the_home_executor_body_is_read_in_the_order_of_production_8,
corba_p3_8_10_an_executor_body_a_segment_and_a_proxy_home_are_not_empty}`,
`crates/idl-lower/tests/cidl.rs::{corba_p3_8_a_composition_with_every_part_lowers_without_a_diagnostic,
corba_p3_8_11_segments_get_ids,
corba_p3_8_13_a_facet_in_two_segments_is_refused,
corba_p3_8_5_1_categories_are_checked_through_the_compiler,
corba_p3_8_7_names_denote_a_home_and_abstract_storage_homes,
corba_p3_8_14_feature_delegation_needs_a_binding_and_a_compatible_member,
corba_p3_8_15_storage_home_delegation_signature_rules,
corba_p3_8_16_only_factories_delegate_to_the_executor,
corba_p3_8_17_abstract_operations_are_home_operations,
corba_p3_8_18_proxy_home_delegates_to_home,
corba_p3_9_2_5_the_toon_example_of_the_specification,
corba_p3_8_5_a_composition_name_is_a_name_of_its_module,
corba_p3_8_2_1_an_identifier_differing_from_a_cidl_keyword_in_case_collides,
pss_3_2_5_psdl_names_and_members_are_checked,
corba_p3_8_15_a_key_finder_raises_not_found_and_its_ref_finder_returns_a_ref,
corba_p3_8_6_a_misplaced_member_that_reaches_the_lowering_is_reported,
pss_3_2_5_4_a_const_operation_is_const_in_the_model}`,
`tools/idlc/tests/cidl.rs::{corba_p3_8_1_a_cdl_file_is_checked_as_cidl,
corba_p3_8_11_dump_cidl_shows_the_compositions_and_their_segment_ids,
corba_p3_8_1_cidl_is_refused_where_idl_is_read_and_dump_cidl_reads_only_cidl,
corba_p3_8_6_a_member_out_of_its_place_fails_the_check}`.

**Status:** done — built and green on the Linux test host. Where the
text and the grammar of §8 disagree, the parser reads both forms:
`delegatesTo storage` (production (20)) and `delegatesTo abstract
storagetype` (the text of §8.14); `provides facet a, b;` (production
(19)) and `provides (a, b);` with a `;` after the segment (the schematic
of §9.2.9.1). The catalogs of the §9.2.7 examples (`uses catalog`) come
from PSS 1.0 and are in neither grammar; they are syntax errors. The
parser keeps the order of production (8) (`implements`, `bindsTo`,
`storedOn`, `manages`, `delegatesTo abstract storagehome`, `delegatesTo
executor`, `abstract`, each at most once, `implements` and `manages`
required) and reports a member out of that order, a second one of a
kind, a missing `implements` or `manages`, a delegation at a place its
production does not give it (home executor: `abstract storagehome` and
`executor`; executor body: `storage`; proxy home: `home`), and an empty
executor body, segment or proxy home (the `+` of productions (14), (16),
(32)); the member stays in an error node. The lowering reports such a
node that reaches it anyway, and a second `storedOn` of a segment
(§8.12). `find_by_<key>` raises `CosPersistentState::NotFound`,
`find_ref_by_<key>` returns `ref<S>` (PSS 2.0 §3.2.5.7); the exception
rule of §8.15 applies to them. The `const` of a local operation is in
the model (`StorageDecl::operations`, `LocalOperation::is_const`) and in
`dump-cidl`. Readings the specification leaves open: the home
operations a delegation may name are the explicit ones of the home and
its bases and the orthodox ones of §6.8.1, not those of
`Components::CCMHome`; a parameter of a value type follows the object
reference rule in §8.15. The suppression of default delegations at the
end of §8.15 concerns the generated implementation and belongs to the
CIF (§9). Of PSS 2.0 the compiler checks the names (`of`, bases,
`implements`, `as`, `scope`, `ref<…>`) and the state members of keys,
factories, `ref(…)` and `stores`; not checked are the comparability of
key types (§3.2.5.7), name clashes under multiple inheritance and the
implicit base `CosPersistentState::StorageObject` (§3.2.5.5); PSS 2.0
has no coverage document of its own in this repository. No proof
against another CIDL compiler: none is in the house (OpenCCM, MicoCCM
and CIAO were not checked); the proof rests on the grammar text and the
fixtures.

### §9 CCM Implementation Framework (CIF)

**Spec:** Part 3 §9, p. 93 — CIF architecture: the composition as the
unit of implementation (§9.2.5, §9.2.6), managed storage and the
delegation of home operations to the abstract storage home or the
executor (§9.2.7, §9.2.8), the executor definition with segments and the
delegation of feature state (§9.2.9), proxy homes (§9.2.10), reference
information with facet, segment and state identifiers (§9.2.11); the
language mapping through equivalent local interfaces (§9.3).

**Repo:** `crates/ccm/src/executor.rs` (§9.3.3: `CCM_<I>` per
interface, `CCM_<E>Consumer` per event type, `CCM_<C>_Executor`,
`CCM_<C>`, `CCM_<C>_Context` per component with the port operations of
§9.3.3.5, `CCM_<H>Explicit`, `CCM_<H>Implicit` keyless and keyed,
`CCM_<H>` per home; `executor_idl` for a whole model);
`crates/corba-ccm/src/cif.rs` (§9.3.2: `ExecutorLocator` with
`obtain_executor`, `release_executor`, `configuration_complete`,
`ComponentImplementation` monolithic or a locator with `invoke` per
invocation; §9.2.9.1: `SegmentLayout` with facet and segment ids and the
rules for segments, `SegmentedComponent` activates only the segment of
the requested facet through `SegmentFactory::create_executor_segment`;
§9.2.11: `ComponentReferenceInfo` monolithic and segmented with its
encoding in the `object_key` and the component identity).

**Tests:** `crates/ccm/tests/executor_idl.rs::{corba_p3_9_3_3_4_the_hello_world_example,
corba_p3_9_3_3_5_ports_become_operations_of_the_executor_and_the_context,
corba_p3_9_3_3_3_a_derived_component_inherits_the_executors_of_its_base,
corba_p3_9_3_3_5_an_extended_port_maps_through_its_basic_ports,
corba_p3_9_3_3_6_9_the_bank_example,
corba_p3_9_3_3_6_4_a_keyed_home_and_a_keyless_home_derived_from_it,
corba_p3_9_3_the_executor_transformations_refuse_other_declarations}`,
`crates/corba-ccm/src/cif.rs::tests::{corba_p3_9_3_2_a_locator_is_asked_before_and_released_after_each_invocation,
corba_p3_9_3_2_a_monolithic_executor_serves_every_port,
corba_p3_9_2_9_1_segments_get_ids_and_the_component_segment_keeps_the_rest,
corba_p3_9_2_11_reference_information_round_trips_and_names_the_component,
corba_p3_9_2_9_1_a_request_activates_only_the_segment_of_its_facet,
lifecycle_calls_increment_counters}`.

**Status:** partial — built and green on the Linux test host: the
executor equivalent IDL of §9.3 with every rule of §9.3.3, the two
strategies of §9.3.2 as building blocks (`ComponentImplementation`,
`ExecutorLocator`), segments per §9.2.9.1 (`SegmentedComponent`) and the
encoding of the reference information per §9.2.11
(`ComponentReferenceInfo`). Missing is their integration into the
container and the ORB: `Container` installs a `ComponentExecutor` and
routes no invocations to facets, so it calls no `obtain_executor` and
`release_executor` per invocation (§9.3.2: "the container will invoke
its obtain_executor operation prior to each invocation"); `ExecutorLocator`
is not tied to `SessionComponent` and `EntityComponent` as §9.3.2
requires of a locator implementation; no object reference of a component
carries `ComponentReferenceInfo` in its `object_key` (§9.2.11: "It is
the responsibility of the container and the underlying ORB to encode
this information for insertion into object references"). Missing as well
is the generation from CIDL
compositions: the complete implementation of the home executor and the
executor skeletons per segment (§9.2.5, §9.2.6, §9.2.9), the implicit
delegation of finders to the abstract storage home and the binding of
the primary key to the matching key (§9.2.8.1, §9.2.8.2), the explicit
delegation to the storage home and the executor with the factory facet
and `abstract` (§9.2.8.3), the delegation of feature state to storage
members (§9.2.9.2) and proxy homes (§9.2.10). What they are generated
from exists: the CIDL parser, the composition model and the PSDL
declarations of the abstract storage homes (§8, done; `lower_cidl`,
`zerodds_ast::cidl`). The executor IDL
is a transformation of the model; `idlc` and the language backends
(`crates/lang-rules/src/emit/intermediate_idl.rs` holds only the client
side) do not emit it yet.

### §10 The Container Programming Model

**Spec:** Part 3 §10, p. 135 — server and client programming
environments, basic/extended component programming interfaces.

**Repo:** `crates/corba-ccm/src/container.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm-lib/src/`
(persistence bridge, telemetry, DDS bridge).

**Tests:** `crates/corba-ccm/src/container.rs::tests`:
`install_and_full_lifecycle_round_trip`, `passivate_in_configured_state_invalid`,
`unknown_instance_returns_not_found`, `instance_count_tracks_install_and_remove`;
`crates/corba-ccm/src/context.rs::tests::anonymous_context_returns_none`,
`::principal_context_returns_bytes`;
`crates/corba-ccm-lib/src/dds_bridge.rs::tests` (7),
`crates/corba-ccm-lib/src/persistence.rs::tests` (8),
`crates/corba-ccm-lib/src/telemetry.rs::tests` (8).

**Status:** done

### §11 Integrating with Enterprise JavaBeans (EJB)

**Spec:** Part 3 §11, p. 177 — CCM↔EJB view mapping (CCM component →
EJB view, EJB bean → CCM view), TX bridging, naming glue.

**Repo:** `crates/corba-ccm-ejb/` with `connector_bean.rs`,
`naming_glue.rs`, `stub_gen.rs`, `tx.rs`.

**Tests:** `crates/corba-ccm-ejb/src/naming_glue.rs::tests` (8 tests):
`bind_and_lookup_round_trip`, `rebind_replaces_ior`, `unbind_removes_entry`,
`jndi_global_strips_prefix`, `jndi_comp_env_strips_prefix`,
`cos_to_jndi_round_trip_no_kind`, `cos_to_jndi_includes_kind`;
`crates/corba-ccm-ejb/src/tx.rs::tests` (8): `cos_to_jta_round_trip_for_all_states`,
`begin_then_commit_succeeds`, `rollback_only_then_commit_fails_with_rollback`;
`crates/corba-ccm-ejb/src/connector_bean.rs::tests` (4),
`crates/corba-ccm-ejb/src/stub_gen.rs::tests` (5).

**Status:** done

### §12 Interface Repository Metamodel

**Spec:** Part 3 §12, p. 203 — IR metamodel MOF DTDs + IDL.

**Repo:** extensions in `crates/corba-ir/src/repository.rs` (MOF-
capable `Container`/`Contained` hierarchy) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` with MOF-2.0 subset (Class/Property/Operation) +
XMI-1.2 output for the component model.

**Tests:** `orb_core::tests::{xmi_emitter_empty_yields_minimal_doc,
xmi_emitter_class_with_inheritance, xmi_emitter_property_emits,
xmi_emitter_operation_with_parameters,
ifr_ccm_metamodel_add_component,
ifr_ccm_metamodel_ingest_repository_walks_definitions}`.

**Status:** done — base IR (`corba-ir`) + MOF-2.0 subset + XMI-1.2
emitter live; `IfrCcmMetamodel::from_repository(&corba_ir::Repository)`
walks the `Container`/`Contained` hierarchy and produces the
component-model classes. The repository walker (`ingest_definition_into`)
is tested; metamodel output is serializable as an XMI doc.

### §13 CIF Metamodel

**Spec:** Part 3 §13, p. 289 — CIF metamodel MOF DTDs + IDL.

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF AST) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement}`
for the XMI emitter symmetric to §12.

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests::xmi_emitter_empty_yields_minimal_doc`,
`::xmi_emitter_class_with_inheritance`, `::xmi_emitter_property_emits`;
`crates/corba-ccm/src/cif.rs::tests::lifecycle_calls_increment_counters`,
`::cif_error_carries_diagnostic`.

**Status:** done — CIF AST (`corba-ccm::cif`) + MOF-XMI emitter
symmetric to §12 IFR; the repository walker ingests component/
composition definitions via `IfrCcmMetamodel::from_repository`.

### §14 Lightweight CCM Profile

**Spec:** Part 3 §14, p. 301 — Lightweight profile with explicitly
excluded features (persistence, introspection,
segmentation, transactions, security, configurators, proxy homes,
home finders).

**Repo:** implicit in the current container programming model:
persistence is optional (`crates/corba-ccm-lib/src/persistence.rs`),
TX optional via `crates/corba-ccm-ejb/src/tx.rs`.

**Tests:** `conformance_tests::corba_part3_14_marker_namespace`.

**Status:** done — Lightweight profile marker
`corba-ccm::conformance::CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE`
+ existing `LIGHTWEIGHT_CCM_LEVEL` as a formal conformance
statement.

### §15 Deployment PSM for CCM

**Spec:** Part 3 §15, p. 309 — D&C PSM: PIM→PSM transformation,
PSM→IDL/XML.

**Repo:** `crates/corba-dnc/src/plan.rs`, `crates/corba-dnc/src/node.rs`,
`crates/corba-dnc/src/execution.rs`, `crates/corba-dnc/src/container_host.rs`,
`crates/corba-dnc/src/repository.rs`.

**Tests:** `crates/corba-dnc/src/execution.rs::{full_lifecycle_round_trip,
invalid_plan_rejected_at_prepare, double_start_fails, destroy_before_running_fails}`
(the PIM→PSM prepare/start/destroy lifecycle),
`crates/corba-dnc/src/container_host.rs::{install_creates_container_on_demand,
install_then_activate_reaches_active, full_lifecycle_round_trip,
entity_and_session_share_host}` (the PSM→container mapping).

**Status:** done

### §16 Deployment IDL for CCM

**Spec:** Part 3 §16, p. 329 — D&C IDL.

**Repo:** as Rust representation in
`crates/corba-dnc/src/plan.rs` (`DeploymentPlan`,
`InstanceDeploymentDescription`, `PlanConnection`),
`crates/corba-dnc/src/repository.rs` (`RepositoryManager`,
`PackageConfiguration`), `crates/corba-dnc/src/node.rs`
(`NodeManager`, `NodeApplication`, `NodeApplicationManager`),
`crates/corba-dnc/src/execution.rs` (`ExecutionManager`,
`DomainApplicationManager`).

**Tests:** one test per IDL interface operation group —
`crates/corba-dnc/src/plan.rs::{valid_plan_passes_validation,
instance_with_unknown_impl_fails, connection_with_unknown_instance_fails,
co_location_with_unknown_instance_fails}`,
`crates/corba-dnc/src/repository.rs::{install_and_find_round_trip,
list_packages_returns_all_labels, delete_existing_returns_true,
delete_unknown_returns_false, install_overwrites_duplicate}`,
`crates/corba-dnc/src/node.rs::{prepare_plan_then_start_launch,
destroy_application_clears_active, register_and_unregister_application,
unregister_unknown_returns_none}`,
`crates/corba-dnc/src/execution.rs::tests::execution_manager_lists_prepared_plans`.

**Status:** done

### §17 XML Schema for CCM

**Spec:** Part 3 §17, p. 343 — XML schema for deployment plans.

**Repo:** `crates/corba-dnc/src/xml.rs`.

**Tests:** `crates/corba-dnc/src/xml.rs::{parses_minimal_plan,
parses_multiple_artifacts, validation_fails_on_unknown_impl_reference,
missing_root_tag_returns_expected_tag, co_locate_with_parsed}`.

**Status:** done

---

## Adjacent OMG service: COS Naming v1.3 (formal/04-10-03)

The CosNaming service is its own spec, carried here as an initial
reference per Part 1 §8.5.2.

### CosNaming::NamingContext

**Spec:** CosNaming v1.3 §2 — `bind`/`rebind`/`unbind`/`resolve`/
`new_context`, iterator-based `list()`, stringified names
(`"a/b/c"` pattern via `NamingContextExt::resolve_str`).

**Repo:** `crates/corba-cosnaming/src/context.rs`,
`crates/corba-cosnaming/src/name.rs`,
`crates/corba-cosnaming/src/stringified.rs`.

**Tests:** `crates/corba-cosnaming/src/context.rs::tests` (13 tests):
`bind_and_resolve_simple`, `double_bind_yields_already_bound`,
`rebind_replaces_existing`, `nested_context_resolve`,
`resolve_to_context_returns_context`, `resolve_unknown_yields_missing_node`,
`unbind_removes_entry`, `destroy_with_bindings_yields_not_empty`,
`destroy_empty_is_ok`, `list_returns_all_bindings_with_correct_type`,
`resolve_str_via_naming_context_ext`, `to_url_constructs_corbaname`,
`empty_name_is_invalid`;
`crates/corba-cosnaming/src/stringified.rs::tests` (9),
`crates/corba-cosnaming/src/name.rs::tests` (3).

**Status:** done

---

## ZeroDDS-specific bridges (no OMG item)

### CORBA object ↔ DDS topic bridge

**Spec:** no OMG spec item; ZeroDDS-specific migration layer
for the incremental replacement of CORBA endpoints by DDS wire.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/mod.rs` (the daemon
`zerodds-corba-bridged`).

**Tests:** —

**Status:** n/a (informative) — no OMG item; the daemon
`zerodds-corba-bridged` is covered in `zerodds-corba-bridge-1.0.md`, with
its tests.

---

## Audit status

44 done / 16 partial / 2 open / 1 blocked / 13 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document corba-3.3.md`):

* `cargo nextest run --profile merge -p zerodds-corba-ccm --all-targets` (job `merge-rust-corba-ccm`) — 125 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-ccm-ejb --all-targets` (job `merge-rust-corba-ccm-ejb`) — 10 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-codegen --all-targets` (job `merge-rust-corba-codegen`) — 10 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-cosnaming --all-targets` (job `merge-rust-corba-cosnaming`) — 13 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-csiv2 --all-targets` (job `merge-rust-corba-csiv2`) — 30 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-dnc --all-targets` (job `merge-rust-corba-dnc`) — 27 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-giop --all-targets` (job `merge-rust-corba-giop`) — 48 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-iiop --all-targets` (job `merge-rust-corba-iiop`) — 22 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-ior --all-targets` (job `merge-rust-corba-ior`) — 25 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-ir --all-targets` (job `merge-rust-corba-ir`) — 21 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-poa --all-targets` (job `merge-rust-corba-poa`) — 14 tests green.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (job `merge-rust-idl-lower`) — 7 tests green.
* `cargo nextest run --profile merge -p zerodds-idl-parse --all-targets` (job `merge-rust-idl-parse`) — 2 tests green.

354 cited tests green, 0 not green.

Open items: `corba-3.3.open.en.md`.
