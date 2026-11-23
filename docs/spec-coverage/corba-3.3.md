# OMG CORBA 3.3 — Spec-Coverage (WP CORBA-Coexistence)

**Quelle:** OMG CORBA 3.3 — drei Bände als formal-Dokumente:
* Part 1 (Interfaces, 532 S.) — [`formal/12-11-12`](https://www.omg.org/spec/CORBA/3.3/Interfaces/PDF)
* Part 2 (Interoperability, 249 S.) — [`formal/12-11-14`](https://www.omg.org/spec/CORBA/3.3/Interoperability/PDF)
* Part 3 (Component Model, 380 S.) — [`formal/12-11-16`](https://www.omg.org/spec/CORBA/3.3/Components/PDF)

**Kontext.** ZeroDDS deckt mit dem DDS-Kern und dem Codegen aus
`crates/lang-rules/` und `crates/corba-codegen/` das DDS-PSM
vollständig ab. Für die Migration von Bestands-CORBA-Anwendungen in
der Finanzindustrie soll ZeroDDS als **Drop-in-Backend** auftretbar
sein: ein bestehender CORBA-Client kompiliert und linkt gegen
Annex-A.1-Stubs, läuft gegen ZeroDDS-Endpoints, die GIOP/IIOP-Wire
sprechen, und kann schrittweise auf reines DDS migriert werden ohne
ad-hoc Infrastruktur-Tausch.

Die CORBA-Coexistence ist über mehrere Implementierungs-Crates verteilt:

- `crates/idl-parse/` — IDL-Grammatik (Part 1 §7 IDL-Syntax)
- `crates/corba-codegen/` — Annex-A.1-Codegen (Stubs/Skeletons, Part 1 §7)
- `crates/corba-ir/` — Interface Repository (Part 1 §14)
- `crates/corba-poa/` — Portable Object Adapter (Part 1 §15)
- `crates/corba-giop/` — GIOP-Wire-Protokoll (Part 2 §9)
- `crates/corba-iiop/` — IIOP-Transport (Part 2 §9.7)
- `crates/corba-ior/` — IOR-Kodierung (Part 2 §9.4 / §10.5)
- `crates/corba-csiv2/` — CSIv2-Security (Part 2 §10)
- `crates/corba-ccm/` — Component Model (Part 3 §6-§9)
- `crates/corba-ccm-lib/` — CCM-Container (Part 3 §10)
- `crates/corba-ccm-ejb/` — EJB-Integration (Part 3 §11)
- `crates/corba-dnc/` — Deployment & Configuration (Part 3 §15-§17)
- `crates/corba-dds-bridge/` — ZeroDDS-Bridge (kein OMG-Item)
- `crates/corba-cosnaming/` — COS Naming v1.3 (separate Spec)
- `crates/corba-cos-event/` — COS Event Service v1.2 (separate Spec)
- `crates/dcps/` — DDS-PSM-Kern (DDS-Migrationsziel)
- `crates/rpc/` — DDS-RPC-Kern (DDS-Migrationsziel)

CosNaming und CosEventService sind eigenständige OMG-Specs (formal/04-10-03
und formal/04-10-02); CosEvent hat eine eigene Spec-Coverage-Datei
(`cos-event-service-1.4.md`). CosNaming-Coverage wird hier inline mitgeführt,
weil CORBA Part 1 §8.5.2 sie als Standard-Initial-Reference verlangt.

---

## Part 1: Interfaces

### §1 Scope

**Spec:** Part 1 §1, S. 1 — Geltungsbereich der CORBA-Spezifikation.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Geltungsbereich.

### §2 Conformance and Compliance

**Spec:** Part 1 §2.1, S. 1 — "The minimum required for a
CORBA-compliant system is adherence to the specifications in this
standard and one mapping. Each additional language mapping is a
separate, optional compliance point."

**Repo:** —

**Tests:** —

**Status:** open — die Einhaltung des Standards ist die Summe der Items
dieses Dokuments; dazu kommt eine OMG-Sprachabbildung, deren erzeugter
Client und Servant auf einem ORB laufen. Coverage-Dokumente gegen
OMG-Abbildungen gibt es: `idl4-cpp-1.0` mit der CORBA-Plattform von
Annex A.1, dort offen unter anderem `ORB_init` und eine Implementierung
von `CORBA::ORB` (A.1.4), die Fabriken am ORB (A.1.8.9),
`PortableServer`, Servants und Skeletons (A.1.1.11, A.1.8.8, A.1.10 bis
A.1.13); `idl4-java-1.0` und `idl4-csharp-1.0` ohne offene Items,
aber ohne ORB und Object Adapter in `runtime/java` und `runtime/csharp`
(die CORBA-Plattform von Java bildet `TypeCode`, `Object`, `Any` und
`UserException` ab). Es fehlen: eine dieser Abbildungen auf dem ORB von
ZeroDDS (`crates/corba-interop`, `crates/corba-poa`), ein Test, in dem
ihr erzeugter Client und Servant mit einem fremden ORB sprechen, und die
offenen und teilweisen Items dieses Dokuments. Die Abbildung nach Rust
(`zerodds-corba-rust-1.1`) ist eine eigene Spezifikation, keine
OMG-Abbildung.

### §3 Normative References

**Spec:** Part 1 §3, S. 1 — Liste normativer Referenzen.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

### §4 Additional Information

**Spec:** Part 1 §4, S. 2 — Outline + Keyword-Konventionen
(MUST/SHALL/SHOULD/MAY).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Gliederung von Part 1 und Lesart der
Schlüsselwörter nach RFC 2119; keine Anforderung an eine
Implementierung.

### §5 The Object Model

**Spec:** Part 1 §5, S. 3 — abstraktes Objektmodell (Objects, Requests,
Types, Interfaces, Value Types, Operations, Attributes).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — konzeptioneller Rahmen, keine Wire-/
Code-Pflicht.

### §6 CORBA Overview

**Spec:** Part 1 §6, S. 11 — Architektur-Überblick (ORB, Stubs, DSI,
Object Adapter, IR).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Überblick; die Anforderungen an ORB,
DII, DSI, Object Adapter und IR stehen in §7–§18.

### §7 IDL Syntax and Semantics

**Spec:** Part 1 §7, S. 27 — komplette IDL-Grammatik (Lexer, Module,
Interface, Value, Constant, Type, Exception, Operation, Attribute,
Repository-Identity, Event, Component, Home, Names & Scoping).

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

**Status:** done — IDL 4.2 ist Superset zu CORBA-IDL §7. Rule-für-Rule
Coverage in `docs/spec-coverage/idl-4.2.md` (649 done / 4 partial).
Repository-Identity (§7.15) ist via `corba-codegen::repository_id`
implementiert. Component- und Home-Declaration (§7.17/§7.18) werden
in Part 3 §6 erneut gefasst und sind dort `done`.

### §8 ORB Interface

**Spec:** Part 1 §8, S. 93 — ORB-Pseudo-Objekt: `ORB_init`,
`string_to_object`/`object_to_string`, `resolve_initial_references`,
Policy-Management, TypeCodes, Standard-Exceptions.

**Repo:** `crates/corba-ccm/src/orb_core.rs::{orb_init, OrbArguments, Orb,
ObjectRef, ResolveError}` (`ORB_init` mit den `-ORB<suffix>`-Argumenten,
ORB-Tabelle nach ORBid, `id`, `object_to_string`, `string_to_object` über
`IOR:`, `corbaloc:` mit `iiop` und `rir`, `corbaname:` mit `resolve` über
die installierte `CorbaConnection`, `list_initial_services`,
`resolve_initial_references`, `register_initial_reference`,
`create_policy`, `policy_manager`, `policy_current`, `get_client_policy`,
`ObjectRef::{set_policy_overrides, get_policy_overrides}`, `shutdown`,
`destroy`); `crates/corba-ccm/src/policy_management.rs` (`PolicyManager`,
`PolicyCurrent`, `SetOverrideType`, `OverrideError`, `check_overrides`,
`set_object_overrides`, `get_object_overrides`, `client_policy`);
`crates/corba-rt/src/object.rs::{ObjectReference::policy_overrides,
PolicyOverride}` (die Overrides einer Referenz, nicht Teil der IOR);
`crates/corba-rt/src/exception.rs::{SystemException, SystemExceptionKind,
CompletionStatus, OMGVMCID}` (§8.12);
`crates/corba-rust/src/runtime.rs::CorbaException::System`;
`crates/corba-interop/src/runtime.rs::{orb_init, reply_status_and_body,
decode_system_exception}` (Systemausnahme mit eigener Repository-ID auf
dem Draht, ORB mit IIOP-Transport); `crates/corba-ior/src/{stringified.rs,
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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün:
§8.2.1 `id`; §8.2.2 `object_to_string`/`string_to_object` über `IOR:`,
`corbaloc:` (`iiop`, `rir`; ein IIOP-Profil je Adresse) und `corbaname:`
(Auflösung mit `resolve` über IIOP), `file://`, `http://` und `ftp://`
(Part 2 §7.6.10), jedes Profil einer IOR bleibt erhalten,
BAD_PARAM-Minor-Codes 7 bis 10, MARSHAL Minor 4 für ein lokales Objekt; §8.5.1 `ORB_init` mit `-ORBid`, `-ORBInitRef`,
`-ORBDefaultInitRef`, `-ORBServerId`, `-ORBListenEndpoints`,
`-ORBNoProprietaryActivation` (aus der Argumentliste entfernt, ein
unbekanntes `-ORB<suffix>` oder ein fehlender Wert ist BAD_PARAM, dieselbe
ORBid liefert dasselbe ORB bis `destroy`); §8.5.2 und §8.5.3 Initial
References in der Reihenfolge von §8.5.3.4, nie nil, sonst `InvalidName`;
§16.10 `register_initial_reference`; eine `rir`-Auflösung, die zur gerade
aufgelösten ID zurückführt (`-ORBInitRef X=corbaname:rir:/X#n`), endet mit
`InvalidName` bzw. BAD_PARAM Minor 10, `-ORBDefaultInitRef corbaloc:rir:`
ist BAD_PARAM Minor 8; §8.12 Systemausnahmen mit Kennung,
Minor-Code und Completion-Status auf dem Draht in beiden Richtungen (vorher
schrieb der Server für jede Systemausnahme `UNKNOWN`), eine fremde
Systemausnahme wird zu UNKNOWN Minor 2; der erzeugte Skeleton meldet nicht
lesbare Argumente als MARSHAL COMPLETED_NO und eine nicht deklarierte
User-Exception als UNKNOWN Minor 1 COMPLETED_YES, der erzeugte Stub eine
nicht lesbare Antwort als MARSHAL COMPLETED_YES, ein `factory`-Initialisierer
eines Valuetypes NO_IMPLEMENT Minor 1; BAD_INV_ORDER Minor 4 nach
`shutdown`, OBJECT_NOT_EXIST nach `destroy` (§8.2.5.5; `destroy` fährt ein
laufendes ORB vorher herunter); `create_policy` über die PolicyFactories
der ORBInitializer; §8.9 Policy-Management: der `PolicyManager` des ORB
(Initial Reference `ORBPolicyManager`), `PolicyCurrent` (Initial
Reference `PolicyCurrent`, je Thread, ein neuer Thread ohne Overrides;
ein Servant läuft ohne die Overrides seines Threads, und was er dort
setzt, endet mit der Anfrage), `set_policy_overrides` mit `SET_OVERRIDE`
und `ADD_OVERRIDE` und `get_policy_overrides` auf jeder Ebene; §8.3.9
`set_policy_overrides` einer Referenz gibt eine neue Referenz, die
Overrides gehen nicht auf den Draht; §8.3.8.3 `get_policy_overrides`;
§8.3.8.2 `get_client_policy` (Referenz vor Thread vor ORB); BAD_PARAM
Minor 30 für einen Typ doppelt in der Liste, `NO_PERMISSION` für jeden
Typ außer den vier ZIOP-Policies, weil der ORB nur diese auf eine
Invocation anwendet, `InvalidPolicies` mit den Indizes ungültiger Werte;
ein abgewiesener Aufruf ändert nichts; ein lokales Objekt NO_IMPLEMENT
Minor 8 (§8.3.14). `Orb::set_policy` und `get_policy`, ein
Oktett-Speicher, den nichts las, sind entfernt. Offen: §8.2.5 `work_pending`, `perform_work`, `run` und
`shutdown(wait_for_completion)` als Warten auf laufende Anfragen, damit
auch das Warten von `destroy` auf das Ende des Herunterfahrens und
BAD_INV_ORDER Minor 3 für `shutdown(TRUE)` und `destroy` in einem Thread,
der eine Anfrage bedient; `create_policy`, die Policy-Overrides und die
CCM-Erweiterungen (`with_*`) prüfen den Zustand des ORB nicht (kein
BAD_INV_ORDER 4 nach `shutdown`, kein OBJECT_NOT_EXIST nach `destroy`);
MARSHAL COMPLETED_NO beim Schreiben der Anfrage im Stub und
COMPLETED_YES beim Schreiben der Antwort im Skeleton sind nicht getestet,
weil die IDL der Tests keinen Typ hat, dessen Schreiben fehlschlägt;
§8.2.3 `get_service_information`; §8.2.4 und §8.6 Context-Objekt;
§8.3 Objektoperationen auf `ObjectReference` (`is_a`, `non_existent`,
`is_equivalent`, `hash`, `get_interface`, `get_component`,
`get_policy` mit dem Abgleich gegen die Policies der IOR,
`validate_connection`); §8.4 und die Value-Factory-Registry am ORB;
§8.9 Overrides der Policies einer PolicyFactory (bis
`ClientRequestInfo::get_request_policy`, §16.4.13.8, sie einem
Interceptor zeigt, `NO_PERMISSION`); §8.10 Domain-Manager; §8.11 TypeCode-Operationen (`equal`,
`equivalent`, `get_compact_typecode`, Zugriffsfunktionen) und die
`create_*_tc`-Operationen, die die TypeCode-Arten union, array, fixed,
value, value_box, native, abstract und local interface, component, home
und event voraussetzen (§9.3). Herstellernachweis
(omniORB-Client liest Kennung und Minor-Code einer Systemausnahme eines
ZeroDDS-Servants) nicht gelaufen.

### §9 Value Type Semantics

**Spec:** Part 1 §9, S. 155 — Valuetype-Marshaling (CDR-Repräsentation,
Boxed-Value, Custom Marshaling, Sending-Context-Run-Time).

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(Codec-Basis), `crates/idl-parse/src/decl.rs` (`parse_valuetype`,
Value-Box und `custom valuetype`), `crates/lang-rules/src/coverage.rs`
(Valuetype-Regel je Zielsprache), kein dediziertes Custom-Marshal-Hook
im Codegen.

**Tests:** `crates/idl-lower/tests/lowering.rs::{the_other_valuetype_forms_are_unaffected_by_the_box,
an_abstract_valuetype_parses_as_a_valuetype,
a_forward_declared_valuetype_is_completed_by_its_definition}`,
`crates/corba-codegen/tests/orb_agreement.rs::the_valuebase_holder_is_the_omgs_and_jacorbs_names_a_class_that_does_not_exist`,
`crates/corba-codegen/tests/annex_a1_traits.rs::{table_a8_reads_abstract_and_truncatable_off_the_declaration,
table_a9_gives_a_valuebox_its_boxed_trait, every_traited_type_renders_in_every_column}`.

**Status:** done — Valuetype-IDL parst, lowert und trägt in
`lang-rules` eine Emissions-Regel je Zielsprache; Valuetype und Valuebox
tragen in allen vier PSM-Spalten die Member, die Annex A.1.1 ihnen gibt.
Custom-Marshal-Streams via
`crates/corba-ccm/src/orb_core.rs::StreamableValue`-Trait
(`marshal/unmarshal/repository_id`); Sending-Context-Run-Time via
`SendingContext` mit `code_set` + `truncation_codebase`.

### §10 Abstract Interface Semantics

**Spec:** Part 1 §10, S. 171 — `abstract interface`, Marshalling als
Object oder ValueType.

**Repo:** `crates/idl-parse/src/decl.rs` (`abstract` gehört zu zwei
Produktionen — Interface-Header (79) und `<value_abs_def>` (127); die
Weiche steht in `parse_definition`), Codegen-Zeile in
`crates/corba-codegen/tests/orb-corpus/rows/03-abstractbase.idl`.

**Tests:** `crates/idl-lower/tests/lowering.rs::abstract_still_reaches_the_interface_header_it_also_belongs_to`,
`crates/idl-lower/tests/interface_as_type.rs::an_interface_used_as_a_type_resolves_in_every_spelling`
(Zeile „an abstract interface").

**Status:** done — Parser akzeptiert `abstract interface`, das Lowering
führt es als eigenen Interface-Kind, und der Codegen-Korpus enthält die
Zeile.

### §11 Dynamic Invocation Interface (DII)

**Spec:** Part 1 §11, S. 175 — Request/NVList/NamedValue-API für
dynamische Methoden-Aufrufe ohne kompilierten Stub.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::{Request, NvList,
NamedValue, ArgFlag}` mit `add_in_arg`/`add_out_arg`-Operations,
`ArgFlag::{In, Out, InOut}` (Spec §11.1.2 Wire-Werte 1/2/3) und
NVList-`add_value`/`count` (Spec §11.1.3). `Request::invoke` hält mit
dem Ergebnis den Offset des Antwort-Bodys in der Antwort und die
Byte-Reihenfolge der Antwort fest (`result_offset`, `result_endianness`,
`result_body`; Part 2 §9.4.3).

**Tests:** `dynamic_api::tests::{arg_flag_round_trip,
arg_flag_unknown_value_rejected, nvlist_add_value_increments_count,
dii_request_add_in_arg, dii_request_add_out_arg,
dii_encode_giop_request_concatenates_input_args,
dii_encode_giop_request_inout_treated_as_input,
dii_encode_giop_request_round_trip_via_giop_codec}`,
`crates/corba-interop/tests/giop_body_offset.rs::corba_p2_9_4_3_a_dii_result_of_another_orb_on_four_modulo_eight_is_read_at_its_offset`.

**Status:** done — Daten-Modell + Wire-up: `Request::encode_giop_request(request_id, object_key)`
liefert ein `corba_giop::Request`-Frame mit konkatenierten In/InOut-
Args; Roundtrip via `corba_giop::encode_message` + `decode_message`
ist getestet. Spec §11.2 + §15.4.2.

### §12 Dynamic Skeleton Interface (DSI)

**Spec:** Part 1 §12, S. 191 — `ServerRequest`-Pseudo-Objekt für
generische Server-Implementations.

**Repo:** `crates/corba-ccm/src/dynamic_api.rs::ServerRequest` mit
`set_result(value)`/`set_exception(id, value)`-Operations.

**Tests:** `dynamic_api::tests::{dsi_server_request_set_result,
dsi_server_request_set_exception, dsi_input_body_concatenates_in_and_inout,
dsi_servant_default_dispatch_via_input_body,
corba_p2_9_4_2_1_a_dsi_servant_reads_its_arguments_at_their_offset}`.

**Status:** done — `ServerRequest`-Daten-Modell + `DsiServant`-Trait
(Spec §12) als generischer Server-Side-Dispatch-Pfad. Wegen Layer-
Trennung (corba-ccm Layer 8.3 / corba-poa Layer 8.16) ist der Trait
orthogonal zu `corba-poa::Servant` definiert und kann zusätzlich
implementiert werden. `ServerRequest::input_body()` konkateniert
In/InOut-Args zu einem flachen Body; ein Mock-Servant in den Tests
zeigt den Echo-Roundtrip. `dispatch_dsi` baut den `ServerRequest` mit
`ServerRequest::received`: Offset des Bodys in der Nachricht
(`arguments_offset`) und Byte-Reihenfolge (`endianness`);
`ServerRequest::input()` liest die Argumente dort (CORBA 3.3 Part 2
§9.4.2.1).

### §13 Dynamic Management of Any Values (DynAny)

**Spec:** Part 1 §13, S. 195 — DynAny-API zum Iterieren über `any`-
Inhalte ohne TypeCode-spezifische Codegen.

**Repo:** `crates/corba-rt/src/dyn_any.rs::{DynAnyFactory, DynAny,
DynAnyError, InconsistentTypeCode, equivalent}` (in
`crates/corba-ccm/src/dynamic_api.rs` re-exportiert; das bisherige
Datenmodell aus Art, Repository-ID und rohen Oktetten ist entfernt);
`crates/corba-ccm/src/orb_core.rs::orb_init` (Initial Reference
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
(`DynAnyFactory` über resolve_initial_references).

**Status:** partial — gebaut und auf dem Linux-Testrechner grün:
`DynAnyFactory` (`create_dyn_any`, `create_dyn_any_from_type_code` mit den
Vorgabewerten von §13.2.1, `create_dyn_any_without_truncation`,
`create_multiple_dyn_anys`, `create_multiple_anys`, Initial Reference
`DynAnyFactory`); `DynAny` mit `type`, `assign`, `from_any`, `to_any`,
`equal`, `copy`, `destroy`, `insert_*`/`get_*` für boolean, octet, char,
short, unsigned short, long, unsigned long, long long, unsigned long
long, float, double, wchar, string, wstring, any und `DynAny`,
`seek`/`rewind`/`next`/`component_count`/`current_component` mit den
Positionsregeln von §13.2.1 und §13.2.2.9 (ein leeres Struct beginnt bei 0,
eine leere Ausnahme bei -1, `current_component` auf einer leeren Ausnahme
ist `TypeMismatch`), `insert_*`/`get_*` auf einer aktuellen Komponente, die
selbst Komponenten hat, ist `TypeMismatch` (§13.2.2.8); `DynEnum`, `DynStruct` (auch Ausnahmen),
`DynSequence` (Länge, Schranke, Elemente, rekursive Elementtypen);
`TypeMismatch`, `InvalidValue`, `InconsistentTypeCode`. Offen, obwohl
`AnyValue` und `TypeCode` in `crates/corba-rt` die Arten jetzt tragen
(§9.3; `create_dyn_any_from_type_code` gibt für sie `InconsistentTypeCode`
zurück): `DynFixed`, `DynUnion`, `DynArray`, `DynValueCommon`,
`DynValue`, `DynValueBox`, `insert_/get_reference`, `_typecode`,
`_longdouble`, `_val`, `_abstract` und die `insert_*_seq`/`get_*_seq`-
Operationen; ein `any` trägt keine String-Schranke und kein Alias, `to_any`
liefert deshalb den TypeCode ohne sie, und `tk_void` wird wie im übrigen
`any`-Modell als `tk_null` ausgegeben. Herstellernachweis (ein mit DynAny
gebautes `any` an einen omniORB-Servant und zurück) nicht gelaufen.

### §14 The Interface Repository

**Spec:** Part 1 §14, S. 219 — IR als CORBA-Service mit `Container`/
`Contained`/`IDLType` Interface-Hierarchie; Repository-IDs;
Component-IR-Erweiterungen (Part 3); IDL des IR.

**Repo:** `crates/corba-ir/src/repository.rs` (`Repository`,
`Container`-Hierarchie, `find_id` über die ganze Hierarchie,
`interface_is_a` als `InterfaceDef::is_a` nach §14.5.24.1 über die
Basis-Interfaces), `crates/corba-ir/src/repository_id.rs`
(IDL-/Local-Format Repository-IDs), `crates/corba-ir/src/type_code.rs`
(TypeCode-Backend), `crates/corba-ir/src/definition_kind.rs` (DK_*).

**Tests:** `crates/corba-ir/src/repository.rs::tests` (12 Tests):
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

**Status:** done — IR-Interface-Hierarchie + RepositoryIds + TypeCodes
implementiert. Component-IR-Erweiterungen siehe Part 3 §12 (CCM-
Metamodel).

### §15 The Portable Object Adapter (POA)

**Spec:** Part 1 §15, S. 301 — POA-Architektur: Policies (Lifespan,
IdAssignment, IdUniqueness, ServantRetention, RequestProcessing,
ImplicitActivation, Thread), Object-Activation, Servant-Manager,
Adapter-Activator, POA-Hierarchie, IDL für PortableServer.

**Repo:** `crates/corba-poa/src/poa.rs` (Hierarchie + Dispatch),
`crates/corba-poa/src/policies.rs` (alle 7 Standard-Policies),
`crates/corba-poa/src/active_object_map.rs` (AOM),
`crates/corba-poa/src/servant.rs` + `crates/corba-poa/src/servant_manager.rs`,
`crates/corba-poa/src/object_id.rs`,
`crates/corba-poa/src/poa_manager.rs` (Aktivierungs-Lifecycle).

**Tests:** `crates/corba-poa/src/poa.rs::tests` (12 Tests):
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

**Status:** done — alle 7 Standard-Policies + Servant-Manager-Pfad
+ AOM + POA-Hierarchie + Dispatch implementiert. Adapter-Activator
(§15.3.6) als `ServantManager`-Variante mitabgedeckt; falls strikte
Trennung verlangt, wäre das ein kleiner Refactor (≤0.5 PW).

### §16 Portable Interceptors

**Spec:** Part 1 §16, S. 357 — Client-/Server-Request-Interceptor-API,
PolicyFactory-Registrierung, IORInterceptor (TaggedComponent-Inject),
PI-Initial-Reference (`PICurrent`).

**Repo:** `crates/corba-ccm/src/portable_interceptor.rs::{InterceptionPoint,
ReplyStatus, InterceptorError, RequestException, RequestInfo,
ClientRequestInterceptor, ServerRequestInterceptor, InterceptorRegistry,
Outcome, PiCurrent, IorInfo, IorInterceptor, PolicyFactory, Policy,
PolicyError, OrbInitializer, OrbInitInfo, InitializerError,
RegistrationError, CurrentError, register_orb_initializer,
ServiceContextInjector}` (in `orb_extensions.rs` re-exportiert);
`crates/corba-ccm/src/orb_core.rs` (`orb_init` führt die ORBInitializer
aus, `PICurrent` als Initial Reference, `create_policy`, `destroy` ruft
`Interceptor::destroy`); `crates/corba-interop/src/runtime.rs::{
IiopCorbaConnection::with_orb, CorbaServer::set_orb, intercepted_reply,
object_reference_for}` (Anfragepfad über IIOP, IOR-Erzeugung).

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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: die zehn
Interception Points mit den Flow-Stack-Regeln von §16.4.2, §16.4.7 und
§16.4.10 (Startpunkte in Registrierungsreihenfolge, Endpunkte rückwärts
nur für Interceptoren mit abgeschlossenem Startpunkt, Wechsel des
Endpunkts nach Systemausnahme oder ForwardRequest); `RequestInfo` mit der
Gültigkeit nach Tabelle 16.1 und 16.2 (BAD_INV_ORDER Minor 14),
`get_*_service_context` mit BAD_PARAM Minor 26,
`add_*_service_context` mit BAD_INV_ORDER Minor 15; `arguments`,
`exceptions`, `contexts`, `operation_context`, `result` als NO_RESOURCES
Minor 1 an den Punkten, an denen Tabelle 16.1 und 16.2 sie zulassen, sonst
BAD_INV_ORDER Minor 14, wie §16.4.12.3 bis §16.4.12.7 es für Umgebungen
ohne Zugriff vorsieht (diese Abbildung reicht kodiertes CDR durch);
PICurrent mit Thread- und Request-Scope und den Kopien nach §16.5.4.5:
`receive_request_service_contexts` läuft in einem eigenen Thread-Scope,
der Request-Scope wird danach und vor `receive_request` in den
Thread-Scope des Servants kopiert, `receive_request` läuft in diesem
Thread-Scope (ein `set_slot` am Request-Scope erreicht den Servant nicht,
eines an PICurrent schon), nach dem Servant wird der Thread-Scope in den
Request-Scope zurückkopiert, danach gilt wieder der Thread-Scope des
Verbindungs-Threads; IORInfo mit BAD_PARAM Minor 29 für eine Profil-ID,
die die Referenzen des Adapters nicht tragen, `get_effective_policy` mit
nil für einen bekannten Policy-Typ ohne Policy und INV_POLICY Minor 3 für
einen unbekannten (bekannt sind die Typen der registrierten
PolicyFactories; der Adapter von `CorbaServer` setzt keine Policies);
ORBInitializer und ORBInitInfo (Interceptoren mit `DuplicateName`,
`allocate_slot_id`, `register_policy_factory` mit BAD_INV_ORDER Minor 16,
`register_initial_reference` mit der Semantik von
`ORB::register_initial_reference`, also `InvalidName` auch für
`PICurrent` und `DynAnyFactory`, `resolve_initial_references` nur in
`post_init`; in `post_init` werfen `add_*_interceptor` und
`allocate_slot_id` BAD_INV_ORDER Minor 26; während der Initializer werfen
`get_slot` und `set_slot` an PICurrent BAD_INV_ORDER Minor 14 nach
§16.9.2.14 und Annex A.6, §16.5.3 nennt dafür Minor 10, den Annex A.6
einer doppelt gesendeten DII-Anfrage gibt; ein Fehler aus `pre_init` oder
`post_init` bricht `ORB_init` ab, die schon registrierten Interceptoren
erhalten `destroy`, `ORB_init` wirft die Systemausnahme bzw. INITIALIZE
für eine User-Exception); der Anfragepfad über IIOP in beiden Richtungen:
ein Service-Context vom Client-Interceptor zum Server-Interceptor und ein
Reply-Context zurück, ein Slot vom Server-Interceptor zum Servant, eine
Systemausnahme aus `receive_request` ohne Servant-Aufruf, ForwardRequest
auf Server- und Client-Seite mit Wiederholung an das neue Ziel
(LOCATION_FORWARD, höchstens 16 Mal); IOR-Interceptoren beim Erzeugen
einer Referenz mit `object_reference_for`. Die Interceptor-Aufrufe in
`corba-iiop::Connection` sind entfernt (sie liefen nur mit dem
Operationsnamen und ohne Reply-Information). Offen: `send_poll` und
Interceptoren in `AmiClient`, `BiDirEndpoint` und `AmhEndpoint`;
IOR-Interceptoren in `stringify_object_ref*`; `IORInterceptor_3_0` und
ObjectReferenceTemplate (§16.6.3); ServerRequestInfo `adapter_id`,
`adapter_name`, `target_most_derived_interface`, `target_is_a`,
`get_server_policy`; ClientRequestInfo `effective_profile`,
`get_effective_component(s)`, `get_request_policy`; ProcessingMode-Policy
(§16.7). `ORBInitInfo::codec_factory` (§16.9.2.5) liefert die
`CodecFactory` von §7.8 schon in `pre_init`. JacORB 3.9 auf dem
Linux-Testrechner: ein JacORB-Client-Interceptor sendet einen
Request-Kontext an einen ZeroDDS-Server-Interceptor, dessen Reply-Kontext
erreicht den JacORB-Interceptor; die Gegenrichtung (ZeroDDS-Client,
JacORB-Server) ist nicht gelaufen.

### §17 CORBA Messaging

**Spec:** Part 1 §17, S. 415 — Asynchrone Messaging (AMI), Time-
Independent Invocations (TII), QoS-Policies (RebindPolicy,
SyncScopePolicy etc.).

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::{MessagingPolicy,
AmiReplyHandler, AmiReplySink, dispatch_async_reply,
PersistentRequestStore, PersistentRequestEntry}` mit allen 10
Policy-Types (Rebind/SyncScope/RequestPriority/ReplyPriority/Routing/
MaxHops/RequestTime/ReplyTime/RelativeRoundtripTimeout/
RoutingTypeRange) + Wire-Mapping `policy_type() -> u32` nach OMG
`Messaging.idl` (formal/2011-11-02 §B.5.1). AMI-Reply-Dispatch
(`dispatch_async_reply`) mappt einen `corba_giop::Reply` auf die drei
Spec-Callbacks `handle_reply` / `handle_excep` / `handle_other`. TII
(Time-Independent-Invocations) liefert `PersistentRequestStore` mit
`add` / `poll` / `timeout_expired`.

**Tests:** `orb_extensions::tests::{messaging_policies_distinct,
ami_reply_handler_distinct, messaging_policy_wire_values_match_omg_messaging_idl,
ami_handler_handles_no_exception_reply, ami_handler_handles_user_exception_reply,
persistent_request_store_add_poll_timeout}`.

**Status:** done — AMI-Reply-Dispatch-Bridge auf `corba_giop::Reply`
+ TII Persistent-Request-Store + Wire-Mapping aller 10 Messaging-
Policies.

### §18 Compression

**Spec:** Part 1 §18, S. 491 — Modul `Compression`: `Compressor`,
`CompressorFactory`, `CompressionManager` (Initial-Reference),
`CompressorId`-Konstanten, Ausnahmen.

**Repo:** `crates/corba-ccm/src/compression.rs` — die zehn
`COMPRESSORID_*`-Konstanten, `CompressionLevel`, `CompressionRatio`,
`CompressorIdLevel`(`List`), `Buffer`; `CompressionException`
(`reason`, `description`), `FactoryAlreadyRegistered`,
`UnknownCompressorId` mit ihren Repository-IDs; die Traits `Compressor`
(`compress`, `decompress`, `compressor_factory`, `compression_level`,
`compressed_bytes`, `uncompressed_bytes`, `compression_ratio` =
`compressed_bytes / uncompressed_bytes`) und `CompressorFactory`
(`compressor_id`, `get_compressor`, dieselbe Instanz je Level,
`BAD_PARAM` Minor 44 über Level 9); `CompressionManager`
(`register_factory`, `unregister_factory`, `get_factory`,
`get_compressor`, `get_factories`). Eingebaut: zlib (RFC 1950, von §18.1
verlangt) und gzip (RFC 1952) über `flate2` (`FlateCompressorFactory`),
Level 0 schreibt Stored-Blöcke, `decompress` endet an einer Grenze
(Vorgabe 16 MiB, die größte GIOP-Nachricht von `corba-iiop`) mit
`Z_BUF_ERROR`. Jede weitere ID (PKZIP, BZIP2, LZMA, LZO, RZIP, 7X, XAR
oder eine Hersteller-ID) bedient eine registrierte eigene Factory; ohne
sie meldet der Manager `UnknownCompressorId`. ORB:
`crates/corba-ccm/src/orb_core.rs::{Orb::compression_manager,
Orb::with_compression, orb_init}` — `orb_init` trägt den Manager als
Initial-Reference `CompressionManager` ein; `with_compression` nimmt
einen `CompressorIdLevel` und weist eine ID ohne Factory und Level über 9
ab. Die frühere `CompressionAlgorithm`-Aufzählung mit eigenen Werten 0
bis 4 ist entfernt.

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

**Status:** done — alle Schnittstellen, Konstanten und Ausnahmen von
§18.5 gebaut, Tests auf dem Linux-Testrechner grün. Die zlib- und
gzip-Ströme der C-Bibliothek zlib 1.3.1 werden dekomprimiert. Ein
Austausch mit den Kompressoren eines fremden ORB (TAO, omniORB) läuft
über ZIOP und gehört zu Part 2 §12.

---

## Part 2: Interoperability

### §1, §3–§5 Scope/References/Terms/Symbols

**Spec:** Part 2 §1, §3–§5, S. 1-6 — Geltungsbereich, normative
Referenzen, Begriffsdefinitionen, Symbole.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Geltungsbereich, Verweislisten und
Begriffe.

### §2 Conformance and Compliance — Interoperabilität

**Spec:** Part 2 §2, S. 1 — "An ORB is considered to be
interoperability-compliant when it meets the following requirements":
die APIs für Request-Level-Bridges (Dynamic Invocation Interface,
Dynamic Skeleton Interface, die Objektidentitäts-Operationen) und IIOP
(GIOP-Transfersyntax und Nachrichtenformate über TCP/IP). ZIOP ist ein
optionaler Konformitätspunkt (Item §12).

**Repo:** `crates/corba-ccm/src/dynamic_api.rs` (DII, DSI),
`crates/corba-giop/src/object_ops.rs` (`_is_a`, `_non_existent`,
`_interface`, `_repository_id`, `_domain_managers`, `_component`),
`crates/corba-iiop/` (IIOP).

**Tests:** `crates/corba-ccm/src/dynamic_api.rs::tests::dii_encode_giop_request_round_trip_via_giop_codec`,
`crates/corba-ccm/src/dynamic_api.rs::tests::dsi_servant_default_dispatch_via_input_body`,
`crates/corba-giop/src/object_ops.rs::tests::is_object_operation_recognizes_all_six_and_the_legacy_spelling`,
`crates/corba-iiop/src/connection.rs::tests::round_trip_request_reply`.

**Status:** done

### §2.1 MIOP — Erweiterung des Moduls IOP

**Spec:** Part 2 §2.1, S. 2 — `const ProfileId TAG_UIPMC = 3; const
ComponentId TAG_GROUP = 39; const ComponentId TAG_GROUP_IIOP = 40`.

**Repo:** `crates/corba-ior/src/profile_tags.rs` (`ProfileId::Uipmc`),
`crates/corba-ior/src/component_tags.rs` (`ComponentId::Group`,
`ComponentId::GroupIiop`).

**Tests:** `crates/corba-ior/src/profile_tags.rs::tests::standard_profile_tag_values_match_spec`,
`crates/corba-ior/src/component_tags.rs::tests::round_trip_all_known_tags`.

**Status:** done

### §6 Interoperability Overview

**Spec:** Part 2 §6, S. 7 — Architektur-Überblick zur ORB-Interop.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Überblick; die Anforderungen stehen in
§7–§12.

### §7.1–§7.5 Domains, ORB-Services, Bridging, Objektadressierung

**Spec:** Part 2 §7.1–§7.5, S. 15–24 — Domänen, ORB-Services,
Bridging zwischen Domänen, Interoperabilitätsansätze,
domänenrelative Objektreferenzen.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Architekturbeschreibung; die
erfüllbaren Aussagen von §7 stehen in §7.6–§7.10.

### §7.6 Interoperable Object References (IOR)

**Spec:** Part 2 §7.6, S. 25–37 — IOR-Struktur, `TaggedProfile`,
Standard-Profile, `TaggedComponent`, Standard-Komponenten,
Zusammensetzung, stringifizierte IOR (`IOR:` + Hex der
CDR-Encapsulation; §7.6.9: „The case for characters in a stringified IOR
is not significant“), Object URLs `corbaloc` (`iiop`, `rir`, Default-Port
2809, Default-Version 1.0, IPv6-Host in eckigen Klammern) und
`corbaname`, die optionalen Schemata `file://`, `ftp://` und `http://`
(Tabelle 7.1); fehlgeschlagene Umwandlung: `BAD_PARAM` mit Minor-Code 7
bis 10. §7.6.3: Full IOR conformance (alle Profile in ihrer Reihenfolge
erhalten); kann ein ORB kein Profil einer IOR nutzen, `TRANSIENT` Minor
2 oder `IMP_LIMIT` Minor 1. §7.6.6.2: die Adressen der Komponenten
`TAG_ALTERNATE_IIOP_ADDRESS` neben der Adresse des Profils.

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
die Object URLs im Item §7.6.10.

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: IOR,
Profile, Komponenten und stringifizierte IOR; Präfix und Hex-Ziffern
einer stringifizierten IOR werden in jeder Schreibung gelesen (`ior:`,
`Ior:`; `from_stringified`, `object_reference_from_ior`, `string_to_object`
und die `targets` der Bridge), Text ohne das Schema `IOR:` ist BAD_PARAM
Minor 7, Hex, das sich nicht dekodieren lässt, Minor 9; eine Referenz
behält jedes Profil ihrer IOR in seiner Reihenfolge und schreibt es
unverändert zurück, auch ein einzelnes IIOP-Profil mit leeren
`profile_data` und auch in einem `any` (Full IOR conformance); nil ist
nur eine Referenz ohne Type-ID und ohne Profil (§7.6.2); der Client
(`IiopCorbaConnection` für Anfragen und `locate`, `AmiClient`) versucht
die IIOP-Profile in der Reihenfolge der IOR und in jedem zuerst die
Adresse des Profils, dann die Adressen aus `TAG_ALTERNATE_IIOP_ADDRESS`;
ist keine erreichbar, TRANSIENT Minor 2, hat die Referenz kein lesbares
IIOP-Profil, IMP_LIMIT Minor 1, beide `COMPLETED_NO`; `ReferenceAddr`
trägt die ganze IOR und den Index des benutzten Profils. Ein
SSLIOP-Profil wird an seinem Host und SSL-Port erreicht, ein
UIOP-Profil an seinem Socket-Pfad, ohne alternative Adressen. Object
URLs: Item §7.6.10 (done). Offen: die optionale Komponente
`TAG_ORB_TYPE` in den IORs von ZeroDDS, Item §7.6.6.1 (blocked).

### §7.6.6.1 TAG_ORB_TYPE

**Spec:** Part 2 §7.6.6.1, S. 30 — `TAG_ORB_TYPE` trägt als
CDR-Encapsulation eines `unsigned long` „an ORB type ID allocated by the
OMG for the ORB type of the originating ORB“; höchstens einmal je
Profil, in Profilen ab IIOP 1.1 optional.

**Repo:** `crates/corba-ior/src/components.rs::OrbType` (lesen und
schreiben der Komponente).

**Tests:** —

**Status:** blocked — Zuweisung eines ORB-Typs durch die OMG (beantragt,
wie in `zerodds-corba-bridge-1.0` §4.1a); ohne den zugewiesenen Wert
schreibt ZeroDDS die Komponente nicht, ein anderer Wert wäre die Kennung
eines fremden ORB. Hebt sich auf, sobald die OMG den Wert zuweist: dann
eine Konstante in `crates/corba-ior/src/components.rs` und die
Komponente einmal je IIOP-Profil ab 1.1 in den IORs von
`crates/corba-interop/src/runtime.rs` und
`crates/corba-dds-bridge/src/bridge_daemon/ior.rs`.

### §7.6.10 Object URLs — IPv6 ohne Port, `rir`-Ausschluss, Minor-Codes

**Spec:** Part 2 §7.6.10, S. 33–37 — `corbaloc` nach §7.6.10.1:
`<obj_addr_list>["/"<key_string>]`, `<key_string>` mit den Escapes nach
RFC 2396; `<host> = DNS_style_host_name | IPv4_address | "[" IPv6_address
"]"`, ohne Host der lokale Host, Port optional mit Default 2809, ohne
Version 1.0; `rir` ohne Adresse, "The rir protocol cannot be used with
any other protocol in a URL", ein leerer Schlüssel ist `NameService`
(§7.6.10.2); `corbaname` mit Default-Schlüssel `NameService` (§7.6.10.5);
ein weiteres Protokoll (`<future_prot_addr>`, §7.6.10.6); `file://`,
`ftp://`, `http://` (Tabelle 7.1, optional); "If conversion fails,
string_to_object raises a BAD_PARAM exception" mit Minor-Code 7 (bad
scheme name), 8 (bad address), 9 (bad schema specific part), 10
(non-specific reason).

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

**Status:** done — gebaut und auf dem Linux-Testrechner grün: die
Grammatik von §7.6.10.1 bis §7.6.10.3 mit allen Beispiel-URLs des
Abschnitts (Schlüssel optional, IPv6-Host mit und ohne Port, ohne Host
der lokale Host, Port 1 bis 65535 nur aus Ziffern, `rir` allein und ohne
Adresse, leerer `rir`-Schlüssel und `corbaname` ohne Schlüssel sind
`NameService`, der Name eines `corbaname` wird entschlüsselt, ein
weiteres Protokoll wie `atm:` bleibt mit seiner Adresse erhalten und
wird beim Umwandeln übersprungen); ein Schlüssel wird mit `%XX`
geschrieben bis auf alphanumerische Zeichen und die Zeichen der Liste
von §7.6.10.1 (`.` steht nicht darin und wird maskiert); `corbaloc` gibt
ein IIOP-Profil je Adresse in der Reihenfolge der URL; `file://` (nur der
eigene Host), `http://` (HTTP/1.1 `GET`, `Content-Length`, chunked; Pfad
und Query nur aus den Zeichen von RFC 3986, sonst Minor 9; eine
Weiterleitung löst `Location` gegen die URL der Anfrage auf, RFC 3986
§5.2, und führt nur zu einer `http`-URL, sonst Minor 10) und `ftp://`
(Anmeldung, `TYPE I`, mit `;type=a` `TYPE A`, `;type=d` und andere
Typcodes Minor 9, `CWD` je Verzeichnis, `EPSV` oder `PASV`, `RETR`)
liefern Text, der erneut umgewandelt wird, höchstens 8 Stufen; ist
dieser Text leer, nicht UTF-8 oder ohne ein Schema, das der ORB
umwandelt, Minor 9; BAD_PARAM Minor 7 (Schema), 8 (Adresse, auch `rir`
neben einer weiteren Adresse, `file` eines anderen Hosts, keine Adresse
eines Protokolls dieses ORB), 9 (IOR, Schlüssel oder Name nicht lesbar),
10 (`rir` oder `corbaname` findet nichts, Datei oder Server nicht
lesbar). Gegen
omniORB 4.3.3 (Wochen-Job `integration-bridges-vendor-corba`, auf dem
Linux-Testrechner gelaufen): ein omniORB-Client erreicht einen
ZeroDDS-Server über `corbaloc` (IIOP 1.0, 1.2, tote Adresse vor der
lebenden); ZeroDDS löst `corbaname`-URLs über omniNames auf; für zehn
URLs melden beide dasselbe. omniORB weicht bei anderen URLs vom
Abschnitt ab (kein `corbaloc` ohne `/`, `atm:` als falsches Schema,
Minor 10 für Escapes im Schlüssel und für `rir` neben einer Adresse,
Port 0 und ein Nicht-IPv6-Host in Klammern angenommen); diese URLs
vergleicht der Test nicht. Die `http`- und `ftp`-Tests laufen gegen
Server im Test selbst, nicht gegen einen fremden HTTP- oder FTP-Server.

### §7.7 Service Context

**Spec:** Part 2 §7.7, S. 37–40 — `IOP::ServiceContext` (`context_id`,
`context_data` als CDR-Encapsulation), Standard-IDs (§7.7.1),
Verarbeitungsregeln (§7.7.2): ein Kontext außerhalb der eigenen
GIOP-Version oder außerhalb des OMG-Bereichs darf ignoriert werden,
"must be passed on through a bridge and must be made available to
interceptors. No exception shall be raised."

**Repo:** `crates/corba-giop/src/service_context.rs` (`ServiceContext`,
`ServiceContextList` mit `get` und `processable`, `ServiceContextTag` mit
den 22 IDs von §7.7.1 und `support`, `is_omg_range`, `is_valid_for`);
`crates/corba-interop/src/runtime.rs` (`CorbaServer` gibt jeden
empfangenen Kontext an die Server-Interceptoren und an
`on_request_contexts`, `IiopCorbaConnection::service_contexts` nach
Tabelle 7.2, `merge_contexts`, `IiopCorbaConnection::invoke_passing_contexts`);
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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: alle
IDs von §7.7.1 (0 bis 21), der OMG-Bereich (VSCID 0 bis 15, IDs 0 bis
4095) und die Gültigkeit eines Kontexts je GIOP-Version nach Tabelle 7.2;
ein Kontext außerhalb des OMG-Bereichs, der CodeSets-Kontext in einer
GIOP-1.0-Anfrage, `RMICustomMaxStreamFormat` (nur GIOP 1.4) und
`FIREWALL_PATH_RESP` (ohne Zeile in Tabelle 7.2) erreichen in GIOP 1.0,
1.1 und 1.2 den Server-Interceptor mit ihren Daten, ohne Ausnahme, die
Antwort kommt; ein Kontext, den ein Client-Interceptor an die Stelle eines
ORB-eigenen setzt, behält dessen Position, der CodeSets-Kontext bleibt
vor dem SAS-Kontext (§7.10.2.6); das MIOP-Gateway reicht die Kontexte
einer Objektoperation an das Objekt hinter dem IIOP-Profil der Gruppe
weiter und dessen Reply-Kontexte an den Client zurück, zwei Kontexte
mit derselben ID beide und in ihrer Reihenfolge. JacORB 3.9 auf dem
Linux-Testrechner: ein JacORB-Client-Interceptor sendet die Vendor-ID
0x4A430001, der ZeroDDS-Server-Interceptor sieht sie, der Aufruf gelingt,
der Reply-Kontext des ZeroDDS-Interceptors erreicht den JacORB-Interceptor.
Offen: der CORBA/DDS-Bridge-Daemon (`zerodds-corba-bridge-1.0` §4.3)
reicht die Kontexte einer Anfrage nicht an die DDS-Seite weiter, und der
Notify-Pfad (§4.4) sendet keine; das Format der DDS-Samples von §4.3 hat
keinen Platz dafür.

### §7.8 Coder/Decoder Interfaces

**Spec:** Part 2 §7.8, S. 40–43 — `IOP::Codec` (`encode`, `decode`,
`encode_value`, `decode_value`) und `IOP::CodecFactory::create_codec`,
erreichbar über `resolve_initial_references("CodecFactory")`;
"The encodings which shall be supported are: ENCODING_CDR_ENCAPS,
version 1.0; [...] 1.1; [...] 1.2; [...] for all future versions of
GIOP as they arise".

**Repo:** `crates/corba-ccm/src/iop_codec.rs` (`Codec`, `CodecFactory`,
`Encoding`, `ENCODING_CDR_ENCAPS`, `CodecError`, `UnknownEncoding`);
`crates/corba-rt/src/any.rs` (`CorbaAny::encode_value`,
`CorbaAny::decode_value`); `crates/corba-ccm/src/orb_core.rs` (Initial
Reference `CodecFactory`); `crates/corba-ccm/src/portable_interceptor.rs`
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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: die vier
Operationen mit `InvalidTypeForEncoding`, `FormatMismatch` und
`TypeMismatch`, `create_codec` mit `UnknownEncoding`, `ENCODING_CDR_ENCAPS`
1.0 bis 1.4; ein Typ, den Tabelle 7.2 einer späteren GIOP-Version
zuordnet (`wstring`, `wchar`, `long long`, `fixed` vor 1.1, Value-Typen
vor 1.2, `tk_component`/`tk_home`/`tk_event` vor 1.3), ist für die
Kodierung ungültig, auch in einer Sequenz, einem Struct, einem
geschachtelten `any` und als TypeCode-Wert; `wchar` und `wstring` in der
Form von GIOP 1.2 (§9.3.1.6, §9.3.2.7: Encapsulations nehmen GIOP-1.2-CDR
an); Encapsulation big-endian, gelesen in beiden Byte-Reihenfolgen; die
Initial Reference `CodecFactory` und `ORBInitInfo::codec_factory` schon in
`pre_init`. JacORB 3.9 auf dem Linux-Testrechner: `encode` und
`encode_value` unter 1.2 stimmen für ein Struct, eine Sequenz, eine Union,
einen `wstring` und ein `long long` Oktett für Oktett überein, ZeroDDS
dekodiert JacORBs Oktette; beide werfen `UnknownEncoding` für ein anderes
Format; JacORBs 1.0-Codec kodiert einen `wstring`, wo §7.8.1.1
`InvalidTypeForEncoding` verlangt, das ZeroDDS wirft (gemessen). TAO ist
auf dem Testrechner nicht vorhanden, omniORB nicht geprüft. Offen: ein
`any` mit `tk_alias` behält beim Dekodieren nur den Inhaltstyp
(`AnyValue` hat keine Alias-Variante), `decode` gibt dafür ein `any` mit
anderem TypeCode zurück, als die Oktette tragen.

### §7.9 Feature Support and GIOP Versions

**Spec:** Part 2 §7.9, S. 43–44, Table 7.2 — welche Service-Kontexte,
IOR-Komponenten und Funktionen ab welcher GIOP-Version 1.0 bis 1.4
gelten.

**Repo:** `crates/corba-giop/src/feature.rs` (`Feature`, `Support`,
`support`, `Feature::of_tckind`, `Feature::of_operation`);
`crates/corba-giop/src/service_context.rs` (`ServiceContextTag::support`,
`is_valid_for`); `crates/corba-ior/src/component_tags.rs`
(`ComponentId::feature`, `ComponentId::support`);
`crates/corba-interop/src/runtime.rs` (`IiopCorbaConnection::service_contexts`,
Reply-Kontexte von `CorbaServer`); `crates/corba-ccm/src/iop_codec.rs`
(Typen je Kodierung); `crates/corba-giop/src/version.rs` (`Version::V1_0`,
`V1_1`, `V1_2`, `supports_fragments`, `uses_flags_octet`,
`uses_v1_2_request_layout`, `supports_bidirectional`);
`crates/corba-iiop/src/profile_body.rs` (Komponenten ab IIOP 1.1; ein
IIOP-1.0-Profil mit Komponenten wird mit `CdrError::ComponentsNotInVersion`
abgewiesen).

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

**Status:** done — gebaut und auf dem Linux-Testrechner grün: Tabelle 7.2
als Daten, jede Zeile und jede Spalte V1.0 bis V1.4 gegen den Text
geprüft ("optional$"/"optional$$" als optional, die Realtime-Zeilen
optional, Versionen nach 1.4 mit der Spalte 1.4); der Client sendet seine
eigenen Kontexte nur in den Versionen, die Tabelle 7.2 dafür nennt
(CodeSets ab 1.1, INVOCATION_POLICIES ab 1.2), eine Anfrage mit
CSIv2-Zugangsdaten unter GIOP 1.0/1.1 lehnt er mit NO_PERMISSION ab, statt
sie ohne die Zugangsdaten zu senden; der Server sendet INVOCATION_POLICIES
erst in GIOP-1.2-Antworten; der Codec prüft die Typen je Kodierung
(§7.8). Empfangene Kontexte, Komponenten und implizite Operationen
anderer Versionen werden nicht abgewiesen (§7.7.2); `_component` und
`_repository_id` sendet der Client auch über ein IIOP-1.2-Profil, weil
die MIOP-Objektoperationen (§11.2.6.10, §11.3.2, Tabelle 7.2: GIOP 1.4
optional) sie dort verwenden, getestet in den MIOP- und MGM-Tests von
`zerodds-corba-interop`.

### §7.9 GIOP 1.3 und 1.4

**Spec:** Part 2 §7.9, S. 43–44, Table 7.2 — Spalten V1.3 und V1.4;
§9.4 (die Kopfformate 1.3 als typedefs der 1.2-Formate), §9.4.1
(MessageError in der höchsten Version, dann Schließen), §9.3.5.1.6
(Indirektion zwischen Top-Level-TypeCodes ab 1.3), §9.3.1.6 (UCS-2/UCS-4
unter 1.4), §9.7, §9.7.2 (IIOP 1.4, Profilversion).

**Repo:** `crates/corba-giop/src/version.rs` (`Version::V1_3`, `V1_4`,
`MAX`, `is_supported`, `supports_component_operation`,
`supports_repository_id_operation`,
`allows_indirection_between_top_level_typecodes`, `allows_ipv6_in_ior`,
`ucs_uses_utf16_byte_order`); `crates/corba-giop/src/header.rs`
(Kopf 1.0 bis 1.4); `crates/corba-iiop/src/profile_body.rs`
(`IiopVersion::V1_4`, `MAX`, `allows_ipv6_host`, `IiopVersion::for_host`,
`IiopProfileBody::for_endpoint`, `unbracketed_host`);
`crates/corba-iiop/src/connector.rs` (IPv6-Host mit und ohne Klammern);
`crates/corba-cosnaming/src/graph.rs` (`ior_for_object_key`),
`crates/corba-cosnaming/src/servant.rs`
(`NamingServant::with_highest_version`),
`crates/corba-cosnaming/src/naming_daemon.rs` (Antwort in der Version
der Anfrage bis 1.4);
`crates/corba-rt/src/type_code.rs` (`TopLevelTypeCodes`,
`TypeCode::decode_top_level`); `crates/corba-rt/src/any.rs`
(`CorbaAny::decode_in_message`); `crates/corba-interop/src/runtime.rs`
(Antwort in der Version der Anfrage bis 1.4, `refuse_message`,
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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: GIOP
1.3 und 1.4 mit den Formaten von 1.2 in beiden Richtungen, jede
Nachrichtenart; der Server antwortet in der Version der Anfrage, auf
eine höhere Minor-Version mit MessageError 1.4 und schließt die
Verbindung; Profile IIOP 1.1 bis 1.4 einstellbar, ein ZeroDDS-Client
spricht über ein 1.4-Profil GIOP 1.4; `_component` antwortet mit der
Komponentenreferenz einer Facette oder Komponente, sonst nil;
`_repository_id` (1.4) beantwortet `object_ops`; tk_component, tk_home,
tk_event; der Server von `corba-interop` (`CorbaServer` und die
Referenz-Builder in `runtime.rs`) und der Naming-Daemon schreiben einen
IPv6-Host nur in ein 1.4-Profil, als Adresse ohne die Klammern der
URL-Form (`[::1]` wird `::1`); der Naming-Daemon antwortet in der
Version der Anfrage bis 1.4; der Client wählt den Host mit und ohne
Klammern; Indirektion von einem Top-Level-TypeCode auf einen früheren
derselben Nachricht ab 1.3 über `TypeCode::decode_top_level` und
`CorbaAny::decode_in_message`, unter 1.3 abgewiesen. Die Profile tragen
standardmäßig 1.2: gemessen ruft JacORB 3.9 über ein IIOP-1.4-Profil
nicht auf (`MARSHAL` „Unknown GIOP minor: 4“), omniORB 4.3.3 ruft
darüber auf und wirft `INV_OBJREF`, wenn eine Antwort eine Referenz mit
1.4-Profil trägt; §9.7.2 verlangt von beiden, jede Minor-Version
anzunehmen. Es fehlt: (1) die generierten Stubs und Skeletons und die
MIOP-Gruppenverwaltung (`crates/corba-interop/src/mgm.rs`,
`read_properties` und die Ausnahmen `InvalidProperty`,
`InvalidCriteria`, `CannotMeetCriteria`) lesen den TypeCode jedes `any`
für sich (`CorbaAny::decode`); eine Indirektion zwischen
Top-Level-TypeCodes in einer Nachricht an sie ergibt MARSHAL;
`TypeCode::decode_top_level` und `CorbaAny::decode_in_message` haben
außerhalb der Tests keinen Aufrufer, denn Skeleton und Stub erfahren die
GIOP-Version der Nachricht nicht (`AlignedBody`,
`CorbaConnection::invoke`); (2) UCS-2/UCS-4 mit der Bytefolge von UTF-16
unter 1.4 und IANA-Codeset-IDs im CodeSets-Kontext unter 1.4
(§7.10.2.5): ein TCS-W außer UTF-16 wird nicht ausgehandelt, Item „§7.10
Umsetzung in ein ausgehandeltes Codeset außer UTF-8/UTF-16“; (3) der
CORBA/DDS-Bridge-Daemon (zerodds-corba-bridge-1.0) liest GIOP 1.0 bis
1.2 und schreibt einen IPv6-Host in ein Profil der eingestellten Version
`corba.iiop.giop_version` (1.0 bis 1.2), die Referenzen des von ihm
bedienten Naming-Dienstes (`corba.naming.serve`) in ein 1.2-Profil; ein
1.4-Profil setzt voraus, dass er Nachrichten in 1.4 bedient (aus dem
Code gelesen, nicht ausgeführt). Kein Hersteller-ORB mit GIOP 1.3 oder
1.4 bekannt; die Formate 1.3/1.4 sind nur gegen den eigenen Codec
geprüft.

### §7.10 Code Set Conversion

**Spec:** Part 2 §7.10, S. 45–58 — Codeset-Komponente der IOR
(`TAG_CODE_SETS`), Aushandlung des Übertragungs-Codesets für `char`
und `wchar`, `CodeSets`-Service-Kontext, OSF-Registry-IDs, Umsetzung
der Zeichendaten in das ausgehandelte Codeset.

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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: der
Algorithmus von §7.10.2.6 als Funktion — Schritt 4 nimmt den gemeinsamen
Konversions-Codeset, der dem Server am liebsten ist (erste Stelle seiner
Liste), Schritt 5 den Fallback UTF-8/UTF-16, wenn `compatible(CNCS,
SNCS)` gilt (gemeinsamer Zeichensatz nach den OSF-Registry-Zeilen, wie
ACE sie aus `code_set_registry1.2g.txt` führt; IANA-IDs immer
kompatibel; die Grenze IANA < 0x10000 ist aus den Werten beider
Registries geschlossen), sonst CODESET_INCOMPATIBLE Minor 1; ohne
Codeset-Komponente ISO 8859-1 und kein TCS-W (INV_OBJREF Minor 2 für
`wchar`-Daten), ein Null-Codeset für `char` gilt als ISO 8859-1, auch
mit Konversions-Codesets (§7.10.2.4); ein nativer `wchar`-Codeset null
ergibt kein TCS-W, auch mit `wchar`-Konversions-Codesets (INV_OBJREF
Minor 1: §7.10.2.6 verlangt den nativen `wchar`-Codeset eines Servers
mit `wchar`-Daten). Es fehlt: (1) der Client handelt nicht je Verbindung
gegen die `TAG_CODE_SETS` des Profils aus, sondern sendet mit jeder
Anfrage `CodeSetContext::default_pair()` (UTF-8, UTF-16); der Server
liest den Kontext des Clients nicht (CODESET_INCOMPATIBLE Minor 2,
MARSHAL Minor 9, BAD_PARAM Minor 23 und 42, INV_OBJREF 1 und 2 zur
Laufzeit, MARSHAL 5/6 für `wchar` unter GIOP 1.0); (2) die IOR nennt ISO
8859-1 und UCS-2 als Konversions-Codesets ohne Konverter; (3) der native
Codeset kommt nicht aus Locale oder Konfiguration (§7.10.2.2). Alle drei
hängen an der Umsetzung der Zeichendaten im Marshalling, Item „§7.10
Umsetzung in ein ausgehandeltes Codeset außer UTF-8/UTF-16“.

### §7.10 Umsetzung in ein ausgehandeltes Codeset außer UTF-8/UTF-16

**Spec:** Part 2 §7.10.2, S. 48–55 — Client und Server setzen `char`-
und `wchar`-Daten in das ausgehandelte Übertragungs-Codeset um (z. B.
ISO 8859-1).

**Repo:** —

**Tests:** —

**Status:** open — `crates/corba-giop/src/lib.rs` beschreibt die
ISO-Latin-Umsetzung als Aufgabe des Aufrufers; `crates/wire/` kodiert
nur UTF-8 und UTF-16.

### §8 Building Inter-ORB Bridges

**Spec:** Part 2 §8, S. 63 — Inline-Bridges, Request-Level-Bridges.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Part 2 §8.1: „an implementation-oriented conceptual framework for the construction of bridges“; die Klausel stellt keine Anforderung. Die CORBA↔DDS-Bridge steht in `zerodds-corba-bridge-1.0.md`.

### §9 General Inter-ORB Protocol (GIOP)

**Spec:** Part 2 §9, S. 69 — GIOP Wire-Protokoll (alle 8 Message-Typen),
CDR-Transfer-Syntax, Versionierung 1.0/1.1/1.2, IIOP-Mapping, Bi-Dir
GIOP.

**Repo:** `crates/corba-giop/src/` mit einer Datei je Message-Typ plus
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

**Spec:** §9.3, S. 71 — CDR-Encoding, Endianness-Markierung,
Primitive/Konstrukt-Layout, GIOP 1.2 Alignment, Value Types,
Pseudo-Objekte (TypeCode, Any, Principal), Object References, Abstract
Interfaces.

**Repo:** `crates/wire/src/writer.rs` + `crates/wire/src/reader.rs`
(Primitive + Alignment + Encapsulation), Re-Use durch den
GIOP-Codec in `crates/corba-giop/src/codec.rs`.
`crates/corba-rt/src/type_code.rs` — alle 37 `TCKind`s von Tabelle 9.2
mit ihren Parameterlisten (leer, einfach: `tk_string`, `tk_wstring`,
`tk_fixed`; komplex in einer Encapsulation: auch `tk_union` mit
Diskriminantentyp, Default-Index und typisierten Labels `UnionLabel`,
`tk_array`, `tk_value`/`tk_event` mit `ValueModifier`, konkreter Basis
und `ValueMember`-Sichtbarkeit, `tk_value_box`, `tk_native`,
`tk_abstract_interface`, `tk_local_interface`, `tk_component`,
`tk_home`); eine Indirektion (§9.3.5.1.6) wird gelesen und geschrieben:
`TypeCode::Recursive` geht als `0xffffffff` und negativer Offset zum
`TCKind` des umschließenden Typs hinaus, über Encapsulation-Grenzen
gezählt. `crates/corba-rt/src/any.rs` — `AnyValue` für jede Art mit
Wert-Encoding: `LongDouble`, `Fixed` (BCD, gelesen bis zum
Vorzeichen-Nibble), `Principal`, `TypeCodeValue`, `Array`, `Union`
(gewählter Fall, sonst Default, sonst kein Wert), `Except`
(Repository-ID vor den Membern, §9.3.5.5), `ObjRef` (auch
`tk_component`/`tk_home`; nil als leere Type-ID ohne Profil, die leere
Type-ID mit einem IIOP-Profil als Referenz auf `Object`, §9.3.5.1.3;
jedes Profil der IOR bleibt in seiner Reihenfolge erhalten und wird
unverändert zurückgeschrieben, Part 2 §7.6.3; eine Type-ID ohne Profil
wird abgewiesen), `Abstract` (§9.3.7: Boolean-Diskriminator, Referenz oder
Wert), `Value` (§9.3.4: Null-Tag, Value-Tag mit Repository-ID oder
ID-Liste, Codebase-URL in `ValueInstance::codebase` gelesen und mit Bit
`0x01` geschrieben, Indirektionen auf Werte, Repository-IDs und
ID-Listen, gechunkter Zustand mit Fortsetzungs-Chunks und gemeinsamen
End-Tags, Kürzung auf einen bekannten Basistyp; ein Wert im
übersprungenen Zustand wird an seinem Value-Tag vermerkt und bei der
ersten Indirektion auf ihn mit dem formalen Typ dieser Stelle gelesen,
§9.3.4.6; `ValueRef::Shared` für geteilte und zyklische Graphen; trunkierbare Typen werden gechunkt mit ID-Liste geschrieben,
die übrigen ungechunkt mit einer Repository-ID); `CorbaAny::decode_with`
nimmt bekannte Value-Typen für Werte in abstrakten Interfaces und
abgeleitete Typen. `tk_native` und `tk_local_interface` haben kein
Wert-Encoding und werden abgewiesen. Die Teile eines Werts (Member,
Elemente, Union-Zweig, Alias, rekursiver Member, Value-Zustand, `any` im
`any`) sind beim Lesen höchstens `MAX_VALUE_DEPTH` (128) Ebenen tief; ein
tieferer Wert ist ein `WireError` und kein Stapelüberlauf. Ein Dekodieren
liest höchstens einen Teil ohne Oktette (einen Wert von `tk_null` oder
`tk_void`, eines leeren Structs, eines Arrays der Länge 0 oder einen nur
aus solchen Werten) je Oktett, das es seit seinem Beginn gelesen hat, und
einen mehr; verschachtelte Sequenzen und Arrays solcher Werte werden
abgewiesen, bevor sie Speicher anfordern. Ein wiederholter `TypeCode`
(eine Indirektion auf einen vorher gelesenen `TypeCode`, der sie nicht
umschließt) wird dekodiert, indem seine Kapselung an der Stelle der
Indirektion erneut gelesen wird; diese erneuten Lesevorgänge eines
`TypeCode` umfassen zusammen höchstens das `MAX_TYPECODE_REREAD_FACTOR`-fache
(32) seiner Oktette, die Kopien durch Top-Level-Indirektionen
(§9.3.5.1.6) höchstens das 32-fache der bis zu ihnen gelesenen Oktette
der Nachricht.

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

**Status:** partial — die TypeCodes aller Arten, die Indirektion in
beide Richtungen und die `any`-Werte aller Arten auf dem
Linux-Testrechner grün. `array<array<null, 1000>, 1000>` und
`sequence<sequence<null>>` mit 10 mal 100000 Elementen werden abgewiesen,
eine sich verdoppelnde Kette aus 24 wiederholten Structs (1,4 KB) wird
abgewiesen, jeweils während der dekodierende Thread weniger als 1 MiB
hält; eine Kette aus 3 wird zu ihren Kopien dekodiert
(Linux-Testrechner). Offen: ein dekodierter Wert trägt eine Kopie der
Typinformation seiner Art (den Element-`TypeCode` jeder Sequenz und jedes
Arrays, den `TypeCode` jeder Union, jedes Werts und jeder Referenz, die
Enumeratoren jedes Enums, die Member-Namen jedes Structs); viele kleine
Werte eines großen Typs kopieren ihn einmal je Wert, der Speicher wächst
dann mit dem Produkt beider Größen, und keine der beiden Grenzen
beschränkt das (ein `AnyValue`, das die Typinformation teilt, ist nicht
gebaut). Offen auch: benutzerdefiniert gemarshallte Werte
(`VM_CUSTOM`) haben keinen Zustand, den ein TypeCode beschreibt, und
werden abgewiesen; eine Objektreferenz mit mehr als einem Profil oder
ohne IIOP-Profil wird in einem `any` abgewiesen, weil
`ObjectReference` nur das IIOP-Profil trägt; der Nachweis gegen omniORB
und JacORB (`any` mit Union, Array, Fixed und Valuetype in beide
Richtungen) ist nicht geschrieben und nicht gelaufen.

#### §9.4 GIOP Message Formats

**Spec:** §9.4, S. 93 — alle 8 Message-Typen mit binärem Layout
(Request/Reply/CancelRequest/LocateRequest/LocateReply/
CloseConnection/MessageError/Fragment).

**Repo:** Eine Datei pro Message-Typ in `crates/corba-giop/src/` —
`crates/corba-giop/src/request.rs`, `crates/corba-giop/src/reply.rs`,
`crates/corba-giop/src/cancel_request.rs`,
`crates/corba-giop/src/locate_request.rs`,
`crates/corba-giop/src/locate_reply.rs`,
`crates/corba-giop/src/close_connection.rs`,
`crates/corba-giop/src/message_error.rs`,
`crates/corba-giop/src/fragment.rs`. Der Server
(`crates/corba-interop/src/runtime.rs::server_other_message`, von
`CorbaServer` und `MiopGateway` genutzt) nimmt jeden Message-Typ
ausdrücklich: `CancelRequest` lässt die Verbindung offen (jede Anfrage
ist beantwortet, bevor die nächste Nachricht gelesen wird),
`CloseConnection` und `MessageError` beenden sie, `Reply`,
`LocateReply` und ein `Fragment` ohne vorangehende fragmentierte
Nachricht erhalten `MessageError`, dann endet die Verbindung. Der Body
an seinem Offset in der Nachricht (§9.4.2.1, §9.4.3.1: unter GIOP 1.0
und 1.1 folgt er dem Header ohne Auffüllung, CDR richtet ihn ab dem
ersten Oktett der Nachricht aus): `crates/corba-giop/src/body.rs`
(`AlignedBody`, `body_writer`, `finish_body`),
`crates/corba-giop/src/request.rs` und `crates/corba-giop/src/reply.rs`
(`body_offset`), `crates/corba-giop/src/object_ops.rs` (liest am
Offset); `crates/corba-rust/src/runtime.rs` (`MarshalArgs`,
`no_arguments`, `marshaled_body`, `CorbaConnection`,
`AsyncCorbaChannel`), `crates/corba-rust/src/interface_emit.rs` und
`crates/corba-rust/src/ami_emit.rs` (der Stub schreibt die Argumente,
wenn die Verbindung den Header kennt; der Skeleton liest sie aus dem
`AlignedBody`); `crates/corba-interop/src/runtime.rs`
(`request_body`, `received_body`, `IiopCorbaConnection`, `AmiClient`,
`CorbaServer`, `BiDirEndpoint`, `AmhEndpoint`),
`crates/corba-interop/src/miop.rs` (`MiopClient::request_message`,
`Goa`, `MiopGateway`), `crates/corba-interop/src/mgm.rs`.

**Tests:** Ein Oktett-Round-Trip je Message-Typ —
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
(das Layout nach den IDL-Structs, nicht nur Round-Trip),
`crates/corba-giop/src/fragment.rs::{round_trip_giop_1_2_with_header,
round_trip_giop_1_1_without_header}`. Alle acht über den Gesamt-Codec,
CloseConnection und MessageError (ohne Body) nur dort: `crates/corba-giop/src/codec.rs::{round_trip_request_giop_1_0_be,
round_trip_close_connection, round_trip_message_error,
round_trip_fragment_with_more_bit, round_trip_cancel_request,
round_trip_locate_request_giop_1_2, round_trip_locate_reply_object_here}`.
Die Typ-Diskriminatoren selbst:
`crates/corba-giop/src/message_type.rs::tests::message_type_values_match_spec_table_15_1`.
Der Server: `crates/corba-interop/tests/giop_version.rs::corba_p2_9_4_the_server_takes_every_message_type_explicitly`.
Der Body an seinem Offset:
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

**Status:** partial — die acht Message-Typen mit ihrem Layout und ihre
Behandlung im Server auf dem Linux-Testrechner grün. Der Body einer
Anfrage steht an seinem Offset: der Client schreibt die Argumente, wenn
der Header der Anfrage feststeht (auch neu nach einer Weiterleitung),
der Server übergibt den Body mit seinem Offset, der generierte Skeleton
und die Objektoperationen lesen dort; über IIOP-1.0-, -1.1- und
-1.2-Profile, in beiden Byte-Reihenfolgen, steht ein Argument von 8
Oktetten nach einem Oktett auf einem Vielfachen von 8 ab Nachrichtenbeginn und
kommt beim Servant an, bei Offsets des Bodys von 0 und 4 modulo 8
(Linux-Testrechner). Der Body einer Antwort von `CorbaServer` unter GIOP
1.0/1.1 steht auf einem Vielfachen von 8: setzen die Service-Kontexte
(auch die der Interceptoren) ihn auf 4 modulo 8, verschiebt ein
Padding-Kontext (`PADDING_CONTEXT_ID`, vier Oktette) ihn um 12; über
IIOP-1.0- und -1.1-Profile mit Antwort-Kontexten von 1 bis 8 Oktetten
steht der `double` der Antwort auf einem Vielfachen von 8 und der
generierte Stub liest ihn (Linux-Testrechner). Der generierte Stub liest
den Body einer Antwort an seinem Offset (`CorbaConnection::invoke_aligned`,
`IiopCorbaConnection` nimmt ihn aus Version und Kopf der Antwort): die
GIOP-1.0/1.1-Antwort eines anderen ORB, deren Body auf 4 modulo 8
beginnt, liest er richtig, ab Offset 0 gelesen ergeben dieselben Oktette
einen anderen `double` (Linux-Testrechner). Der Body einer User Exception
behält seinen Offset (`CorbaException::UserException` und
`RequestException::User` tragen einen `AlignedBody`; `AmiClient` und
`BiDirEndpoint` nehmen ihn aus Version und Kopf der Antwort): der
generierte Stub liest den `long long` und den `double` nach der
Repository-ID aus der GIOP-1.0/1.1-Antwort eines anderen ORB, deren Body
auf 4 modulo 8 beginnt, richtig, ab Offset 0 gelesen ergibt derselbe Body
einen anderen `long long` (Linux-Testrechner); der MGM-Proxy liest seine
Ausnahmen ebenso dort. Gibt ein Servant eine so empfangene User Exception
weiter, schreibt `CorbaServer` (ebenso AMH und `BiDirEndpoint`,
`Reply::place_body_at`) ihren Body unter GIOP 1.0/1.1 auf denselben
Offset modulo 8, und der Stub liest die Member; eine GIOP-1.2-Antwort, deren
Body auf 8 steht, kann ihn nicht tragen und wird `MARSHAL`,
`COMPLETED_YES` (Linux-Testrechner). `invoke_passing_contexts` gibt den
Body der Antwort mit seinem Offset zurück, und das MIOP-Gateway schreibt
ihn auf denselben Offset modulo 8: der Body eines GIOP-1.2-Objekts kommt
in der GIOP-1.1-Antwort an den Client auf 8, der eines GIOP-1.1-Objekts
auf 4 modulo 8 in der GIOP-1.0-Antwort, beide Male mit dem `double` auf
einem Vielfachen von 8; an einen GIOP-1.2-Client wird dieser Body
`MARSHAL`, `COMPLETED_YES` (Linux-Testrechner). Eine User Exception, die
ein Servant in der anderen Byte-Reihenfolge als der der Anfrage
weitergibt, geht in einer Antwort ihrer eigenen Byte-Reihenfolge hinaus
(`CorbaServer` mit und ohne Server-Interceptoren, AMH, `BiDirEndpoint`,
wie es das MIOP-Gateway tut), und der Client liest ihre Member, unter
GIOP 1.0, 1.1 und 1.2 (Linux-Testrechner). Der Body einer
`NO_EXCEPTION`-Antwort behält seinen Offset auf dem AMI-Weg
(`AsyncReply` trägt einen `AlignedBody`; Callback von `sendc_` und Poller
von `sendp_` lesen über `AlignedBody::reader`), in
`BiDirEndpoint::collect_reply` und `invoke` und in der DII
(`Request::invoke` hält `result_offset` und `result_endianness` fest,
`Request::result_body`): aus der GIOP-1.0/1.1-Antwort eines anderen ORB,
deren Body auf 4 modulo 8 beginnt, lesen Poller, Callback und DII in
beiden Byte-Reihenfolgen den `double` richtig, ab Offset 0 gelesen
ergeben dieselben Oktette einen anderen Wert (Linux-Testrechner). Der POA (`Poa::dispatch`, `Servant::invoke` mit dem
Body an seinem Offset und der Byte-Reihenfolge der Nachricht; der Server
`zerodds_corba_interop::serve` antwortet in Version und Byte-Reihenfolge
der Anfrage), `NamingServant::dispatch` (Naming-Daemon und
CORBA/DDS-Bridge-Daemon; die Anfrage-Decoder von `wire` lesen aus einem
`Reader` am Offset) und der DSI-Servant (`ServerRequest::received` und
`input()` mit `arguments_offset` und `endianness`) lesen den Body an
seinem Offset: über IIOP-1.0-, -1.1- und -1.2-Profile, in beiden
Byte-Reihenfolgen, kommen ein `double` (ab GIOP 1.0) und ein `long long`
(ab GIOP 1.1, Tabelle 7.2) nach einem Oktett bei Body-Offsets von 0 und 4
modulo 8 beim POA-Servant und beim DSI-Servant richtig an; der
Naming-Servant löst einen Namen an den Offsets 13, 14, 15 und 17 auf
(Linux-Testrechner). Offen: der CORBA/DDS-Bridge-Daemon gibt den Body
ohne Offset in das DDS-Sample.
`BiDirEndpoint` sendet einen ab Offset 0 geschriebenen Body (GIOP 1.2)
und lehnt dessen Weiterleitung an ein Ziel, dessen Body auf 4 modulo 8
beginnt, mit `MARSHAL` ab. Offen auch: §9.4.6.3 — ein `ForwardRequest` eines
POA-`ServantLocator` als `LocateReply` mit `OBJECT_FORWARD`:
`crates/corba-poa/src/servant_manager.rs::ServantLocator::preinvoke`
kennt kein `ForwardRequest`; `OBJECT_FORWARD` schickt nur der
Location-Agent von `CorbaServer::forward` (§9.6).

#### §9.5 GIOP Message Transport

**Spec:** §9.5, S. 107 — Transport-Anforderungen (geordnet, ohne
Verlust), Connection-Management.

**Repo:** `crates/corba-iiop/src/connection.rs`,
`crates/corba-iiop/src/acceptor.rs`,
`crates/corba-iiop/src/connector.rs` für TCP-Transport.

**Tests:** `crates/corba-iiop/src/acceptor.rs::tests::round_trip_via_acceptor_and_connector`,
`crates/corba-iiop/src/connection.rs::{round_trip_request_reply,
shutdown_propagates_eof_to_peer, close_connection_message_round_trip}`,
`crates/corba-iiop/src/framing.rs::tests::read_back_to_back_messages`
(Message-Grenzen im Strom),
`crates/corba-iiop/src/connector.rs::{connect_reuses_pooled_connection,
invalidated_connection_is_not_returned_to_pool,
max_connections_per_endpoint_is_enforced}`.

**Status:** done — TCP via IIOP. Andere Transport-Bindings (z.B.
SCTP) nicht angeboten.

#### §9.6 Object Location

**Spec:** §9.6, S. 110 — LocateRequest/LocateReply-Semantik, Forward-
ing.

**Repo:** Client: `crates/corba-interop/src/runtime.rs::{IiopCorbaConnection::send,
IiopCorbaConnection::locate, IiopCorbaConnection::forwarded_reference,
Route, LocateOutcome, classify_reply, target_address}` — ein Aufruf folgt
`LOCATION_FORWARD` und `LOCATION_FORWARD_PERM` (höchstens 16 Mal, danach
`TRANSIENT`), auch dem `ForwardRequest` eines Interceptors;
`LOCATION_FORWARD_PERM` und `OBJECT_FORWARD_PERM` werden je Ziel-Referenz
vermerkt und von späteren Aufrufen zuerst benutzt; ist das vermerkte Ziel
nicht erreichbar oder antwortet es `TRANSIENT`, `COMM_FAILURE` oder
`OBJECT_NOT_EXIST` mit `COMPLETED_NO`, fällt der Vermerk weg und der
Aufruf beginnt bei der ursprünglichen Referenz. `NEEDS_ADDRESSING_MODE`
schickt die Anfrage in der verlangten Adressierung (`ProfileAddr`,
`ReferenceAddr`) erneut, die Interceptoren sehen `receive_other` mit
`TRANSPORT_RETRY` (`crates/corba-ccm/src/portable_interceptor.rs::Outcome::Retry`).
`locate` schickt `LocateRequest` und wertet alle sechs `LocateStatusType`
aus. `AmiClient::perform_work` und `BiDirEndpoint::collect_reply` folgen
Forwards über eine eigene Verbindung und adressieren nach
`NEEDS_ADDRESSING_MODE` neu. Server:
`crates/corba-interop/src/runtime.rs::{CorbaServer::forward,
CorbaServer::stop_forwarding, locate_reply, forward_reply,
target_object_key, needs_key_addr_reply}` — ein Server ist Location-Agent
für weitergeleitete Object-Keys (`LOCATION_FORWARD(_PERM)`,
`OBJECT_FORWARD(_PERM)`, `_PERM` erst ab GIOP 1.2), bedient alle drei
Adressierungen und verlangt `KeyAddr`, wenn ein Target keinen lesbaren
Key nennt (`crates/corba-interop/src/lib.rs::serve` ebenso). Wire:
`crates/corba-giop/src/target_address.rs::{TargetProfile,
IorAddressingInfo, AddressingDisposition}` (`ProfileAddr` und
`ReferenceAddr` als die IDL-Structs, vorher mit einem zusätzlichen
Längenfeld), `crates/corba-giop/src/locate_reply.rs` (Body direkt nach
dem Header, ohne 8-Byte-Ausrichtung, §9.4.6.2).

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

**Status:** partial — Client- und Agent-Verhalten gegen eigene Server
und rohe GIOP-Gegenstellen auf dem Linux-Testrechner grün. Offen: der
Nachweis gegen einen fremden Location-Agent (ein omniORB-`ServantLocator`,
der `ForwardRequest` wirft, mit einem ZeroDDS-Client); nicht geschrieben,
nicht gelaufen.

#### §9.7 Internet Inter-ORB Protocol (IIOP)

**Spec:** §9.7, S. 111 — IIOP als TCP-Mapping; IIOP::ProfileBody
(Host/Port/Object-Key + TaggedComponents).

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

**Spec:** §9.8 + §9.9, S. 115-118 — BiDirIIOP-ServiceContext,
BiDirGIOP-Policy.

**Repo:** `crates/corba-iiop/src/bidir.rs` (Wire-Codec) +
`crates/corba-ccm/src/orb_extensions.rs::{BiDirPolicy,
BiDirServiceContext}` (Policy-Lifecycle: Normal/Both;
listen_points-Liste für §9.9.1).

**Tests:** Inline-Tests in `bidir.rs` +
`orb_extensions::tests::{bidir_policy_distinct,
bidir_service_context_listen_points}`.

**Status:** done — BiDir-Codec + Policy-Lifecycle live.

#### §9.10 OMG IDL für GIOP

**Spec:** §9.10, S. 119 — IDL für GIOP-Module-Konstanten + Service-
Context-IDs.

**Repo:** als Rust-Konstanten in `crates/corba-giop/src/service_context.rs`
(`ServiceContextTag`), `crates/corba-giop/src/message_type.rs` (`MessageType`),
`crates/corba-giop/src/flags.rs` (`Flags`),
`crates/corba-giop/src/version.rs` (`Version`),
`crates/corba-giop/src/reply.rs` (`ReplyStatusType`),
`crates/corba-giop/src/locate_reply.rs` (`LocateStatusType`),
`crates/corba-giop/src/target_address.rs` (`AddressingDisposition`).

**Tests:** Ein Wert-Vergleich gegen die IDL je Konstanten-Gruppe —
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

**Spec:** Part 2 §10, S. 125 — Common Secure Interoperability v2:
SAS-Protokoll, GSSUP-Token, Transport-Mechanismen (TLS),
Authorization-Tokens, IOR-Komponenten (TAG_CSI_SEC_MECH_LIST).

**Repo:** `crates/corba-csiv2/` mit `sas.rs`, `gssup.rs`, `mech_list.rs`,
`association_options.rs`; IOR-Tag-Codec in `crates/corba-ior/src/component_tags.rs`.

**Tests:** `crates/corba-csiv2/src/sas.rs::tests` (9 Tests):
`sas_msg_type_values_match_spec`,
`sas_context_body_roundtrip_all_messages_be_le`,
`server_eval_decodes_foreign_gss_wrapped_establish_context`,
`identity_token_discriminator_matches_spec`,
`establish_context_holds_full_payload`, `complete_context_indicates_stateful`,
`context_error_carries_gss_status`;
`crates/corba-csiv2/src/session.rs::tests` (9), darunter
`a_stateful_target_answers_establish_with_a_stateful_complete`,
`a_full_session_survives_the_wire_in_both_byte_orders`;
`crates/corba-csiv2/src/gssup.rs::tests::gssup_byte_identical_to_jacorb`,
`::initial_context_token_tag_is_0x60`,
`::encapsulation_is_wire_correct_octet_seq_len_at_offset_4`.

**Status:** done

#### §10.2 Protocol Message Definitions

**Spec:** §10.2, S. 127 — SAS-Message-Codec (EstablishContext,
CompleteEstablishContext, ContextError, MessageInContext).

**Repo:** `crates/corba-csiv2/src/sas.rs` (`SasMessage` mit
`encode_encapsulation`/`decode_encapsulation`, `IdentityToken`).

**Tests:** `crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
sas_context_body_roundtrip_all_messages_be_le,
sas_identity_token_absent_anonymous_roundtrip,
identity_token_discriminator_matches_spec, establish_context_holds_full_payload,
complete_context_indicates_stateful, message_in_context_can_request_discard,
context_error_carries_gss_status}`.

**Status:** done

#### §10.3 Security Attribute Service (SAS)

**Spec:** §10.3, S. 137 — SAS-State-Machine, Stateful/Stateless
Contexts.

**Repo:** `crates/corba-csiv2/src/session.rs::{SasTarget, SasClient,
TargetAction, RetainedContext, CONTEXT_ERROR_NO_CONTEXT,
CONTEXT_ERROR_INVALID_EVIDENCE}` — Target- und Client-Hälfte der
State-Machine über den `SasMessage`-Typen aus §10.2; ein
stateful Target hält den Kontext unter der `ContextId` und der Client
schickt danach `MessageInContext` statt neuer Credentials, bis ein
`discard_context` oder ein `ContextError` ihn zurück in den
Establish-Zustand setzt.

**Tests:** `crates/corba-csiv2/src/session.rs::{a_stateful_target_answers_establish_with_a_stateful_complete,
a_stateless_target_retains_nothing_and_says_so,
the_second_request_on_a_stateful_context_is_a_message_in_context,
a_stateless_client_re_establishes_on_every_request,
discarding_a_context_makes_the_next_reference_to_it_a_context_error,
a_message_in_context_for_an_unknown_id_is_refused,
a_context_error_puts_the_client_back_into_the_establish_state,
a_target_to_client_message_arriving_at_a_target_is_not_evidence,
a_full_session_survives_the_wire_in_both_byte_orders}`.

**Status:** done — der letzte Test fährt die volle Sitzung über die
§10.2-Oktette: jede Nachricht kreuzt `encode_encapsulation` /
`decode_encapsulation`, bevor die Gegenseite sie sieht.

#### §10.4 Transport Security Mechanisms

**Spec:** §10.4, S. 149 — TLS/SSL-Bindings, Mutual-Auth.

**Repo:** TLS-Stack via `crates/security-pki/` (X.509 + TLS).
CSIv2-→-IIOP-TLS-Bind via `crates/corba-csiv2::association_options`
+ `crates/corba-iiop`-Acceptor (TAG_TLS_SEC_TRANS-Profile-Tags
in `crates/corba-ior/src/component_tags.rs`); Glue ist im
Acceptor-Lifecycle gebunden.

**Tests:** `crates/corba-csiv2/src/mech_list.rs::tests`:
`build_mech_list_with_tls_and_gssup`, `empty_list_has_no_mechanisms`,
`cdr_roundtrip_compound_sec_mech_list`,
`octet_seq_seq_rejects_oversized_count`,
`compound_sec_mech_list_rejects_oversized_count`, `cdr_roundtrip_empty_list`;
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`::has_and_with_round_trip`;
`crates/corba-ior/src/component_tags.rs::tests::well_known_component_tag_values_match_spec`,
`::round_trip_all_known_tags`, `::at_least_32_well_known_tags_modelled`.

**Status:** done — TLS-Stack + CSIv2-IIOP-Bind via IOR-Tags live.

#### §10.5 Interoperable Object References (IOR mit Security-Tags)

**Spec:** §10.5, S. 150 — IOR-Format mit TAG_CSI_SEC_MECH_LIST,
TAG_NULL_TAG, TAG_TLS_SEC_TRANS.

**Repo:** `crates/corba-ior/src/component_tags.rs` (`ComponentId` mit
`CsiSecMechList` = 33, `NullTag` = 34, `TlsSecTrans` = 36),
`crates/corba-ior/src/components.rs` (`StructuredComponent`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`).

**Tests:** `crates/corba-ior/src/ior.rs::tests::a_security_ior_carries_mech_list_tls_and_null_tag_through_a_round_trip`
— ein vollständiger IOR mit den drei Tags in der IIOP-Profile-
Component-Liste, über `encode_encapsulation`/`decode_encapsulation`
und zurück in die strukturierte Form. Tag-Werte und die
Component-Bodies einzeln:
`crates/corba-ior/src/component_tags.rs::{well_known_component_tag_values_match_spec,
round_trip_all_known_tags, at_least_32_well_known_tags_modelled}`,
`crates/corba-ior/src/components.rs::{csi_sec_mech_list_round_trip,
tls_sec_trans_with_addresses_round_trip, ssl_sec_trans_round_trip}`.

**Status:** done

#### §10.6 Conformance Levels (CSIv2)

**Spec:** §10.6, S. 160 — Conformance-Level 0, 1 und 2 und Stateful
Conformance.

**Repo:** Level 0: SAS im Service-Context-Protokoll über TLS (SSLIOP,
rustls) und über ungeschütztes IIOP, zustandslose CSS/TSS
(`crates/corba-csiv2/src/session.rs`), GSSUP
(`crates/corba-csiv2/src/gssup.rs`, `authenticator.rs`), Identity
Assertion: `crates/corba-csiv2/src/identity.rs::{ScopedUsername,
exported_gssup_name, parse_exported_name, AssertedIdentity,
AssertionTrust}` (GSS-Exported-Name nach RFC 2743 mit GSSUP-OID und
Scoped-Username, Token direkt oder als CDR-Encapsulation) und
`authenticator.rs::authenticate_gssup_with` (Backward Trust Evaluation
nach §10.3.1.1: ein Nutzer darf sich selbst und die anonyme Identität
behaupten, andere nur nach einer Vertrauensregel; sonst `ContextError`
invalid evidence; ein nicht unterstützter Identitätstyp, ein
Principal-Name eines anderen Mechanismus und eine Identity-Extension
sind invalid mechanism); `authenticator.rs::authenticate_with_verifier`
dasselbe mit einer Prüffunktion statt der Passwortdatei. TSS des
`CorbaServer`: `crates/corba-interop/src/runtime.rs::CorbaServer::{require_credentials,
set_identity_assertion}` (GSSUP, unterstützte Identitätstypen und
Vertrauensregeln über `authenticate_with_verifier`, Antwort mit
`CompleteEstablishContext` bzw. `NO_PERMISSION` und `ContextError`) und
`crates/corba-interop/src/runtime.rs::received_principal` (der Invocation-Principal nach Tabelle
10.4 für den Dispatcher). Client: `crates/corba-interop/src/runtime.rs::IiopCorbaConnection::{with_csiv2_credentials,
with_csiv2_identity_assertion, with_csiv2_authorization_token}` —
Identity Assertion und das Push-Modell der Autorisierungstoken
(§10.6.2.1, CSS-Seite).

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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: die
Level-0-Teile SAS über TLS 1.2 und 1.3 (SSLIOP-Server von `CorbaServer`
mit einem nur TLS 1.2 anbietenden und einem TLS-1.3-Server) und über
IIOP, zustandsloser Modus, GSSUP, Identity Assertion mit
GSSUP-Exported-Names und Backward Trust Evaluation, auch im TSS des
`CorbaServer`; von Level 1 die CSS-Seite (ein Client schickt ein
Autorisierungstoken). Ein TSS wertet ein empfangenes
Autorisierungstoken nicht aus; das erlaubt Level 0 (§10.6.1.2.4). Blockiert: SSL 3.0 und TLS 1.0 mit den
Pflicht-Ciphersuites von RFC 2246 (§10.6.1.1.1) — rustls bietet nur TLS
1.2 und 1.3, SSL 3.0 und TLS 1.0 sind durch RFC 7568 und RFC 8996
untersagt. Offen: von Level 1 die TSS-Auswertung eines
Autorisierungstokens (X.509-Attributzertifikate nach dem IETF-AC-Profil,
vertrauenswürdige Privilege Authorities, Access Identity gegen Client-
oder behauptete Identität) und die `privilege_authorities` im IOR; Level
2 (Proxy-Info-Extension, Forward Trust Evaluation, `DelegationByClient`
und die IOR-Flags von Tabelle 10.17); Stateful Conformance (§10.6.4):
nur der zustandslose Modus ist gebaut, wiederverwendbare Kontexte
(`MessageInContext`, §10.3.2.2) hält kein TSS und baut kein CSS auf;
ohne `client_authentication_token` leitet kein TSS die
Client-Authentisierungsidentität aus der Transportschicht ab
(§10.3.1.1, Tabelle 10.3 Zeile P2A/None/P1): `authenticate_gssup_with`,
`authenticate_with_verifier` und damit `CorbaServer` verlangen ein
GSSUP-Token und weisen eine Identity Assertion mit TLS-Client-Zertifikat
und leerem Token mit invalid evidence ab, `corba-iiop` reicht das
Client-Zertifikat einer SSLIOP-Verbindung nicht weiter; die Daemons
`zerodds-corba-bridged` und `zerodds-naming-svc` haben keinen
Konfigurationsschlüssel für Vertrauensregeln — sie rufen
`authenticate_gssup` mit `AssertionTrust::new()` auf, ein Nutzer kann
dort nur sich selbst und die anonyme Identität behaupten; der Nachweis
gegen JacORB 3.9 (Identity Assertion mit `ITTPrincipalName` in beide
Richtungen) ist nicht geschrieben und nicht gelaufen.

#### §10.7 Sample Message Flows and Scenarios

**Spec:** §10.7, S. 162 — Beispiele.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Beispielabläufe.

#### §10.8 References

**Spec:** §10.8, S. 171 — Referenzliste.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Verweisliste.

#### §10.9 IDL für CSIv2

**Spec:** §10.9, S. 172 — IDL der CSI-Module.

**Repo:** als Rust-Repräsentation in
`crates/corba-csiv2/src/association_options.rs` (`AssociationOptions`),
`crates/corba-csiv2/src/mech_list.rs` (`CompoundSecMechList`,
`CompoundSecMech`, `AsContextSec`, `SasContextSec`),
`crates/corba-csiv2/src/sas.rs` (`SasMsgType`, `IdentityToken`,
`SASContextBody`-Union als `SasMessage`),
`crates/corba-csiv2/src/gssup.rs` (`GssupCredentialToken`).

**Tests:** Ein Wert-/Layout-Vergleich gegen die IDL je Modul —
`crates/corba-csiv2/src/association_options.rs::tests::flag_values_match_spec_table_24_1`,
`crates/corba-csiv2/src/sas.rs::{sas_msg_type_values_match_spec,
identity_token_discriminator_matches_spec}`,
`crates/corba-csiv2/src/mech_list.rs::{cdr_roundtrip_compound_sec_mech_list,
build_mech_list_with_tls_and_gssup, cdr_roundtrip_empty_list}`,
`crates/corba-csiv2/src/gssup.rs::{initial_context_token_tag_is_0x60,
gssup_oid_starts_with_der_tag, encapsulation_is_wire_correct_octet_seq_len_at_offset_4}`.

**Status:** done

### §11 Unreliable Multicast Inter-ORB Protocol (MIOP)

**Spec:** Part 2 §11, S. 181 — GIOP-Requests über UDP-Multicast:
`PacketHeader_1_0` und Packet Collections (§11.1), UIPMC-Profil,
`TAG_GROUP`, `TAG_GROUP_IIOP`, Group-IOR (§11.2.2, §11.2.3),
`PortableGroup::GOA` (§11.2.4), Gateway (§11.2.5), Multicast Group
Manager mit Properties (§11.2.6), `corbaloc:miop` (§11.2.7), Requests
in GIOP 1.0 bis 1.2 (§11.3).

**Repo:** `crates/corba-iiop/src/miop.rs` — `PacketHeader` (Magic,
Version 1.0, Endian- und Stop-Flag, reservierte Bits null,
`packet_length`, `packet_number`, `number_of_packets`, `UniqueId` bis
252 Oktette, GIOP-Daten ab der nächsten 8-Oktett-Grenze, Feldwerte in
der Byte-Reihenfolge des Flags), `packetize` (gleich lange Pakete, ein
kürzeres letztes mit Stop-Flag), `Reassembler` (nach Sender und
`UniqueId`, jede Reihenfolge; ein Stop-Paket bei fehlenden Paketen
verwirft die Collection, ebenso Widersprüche, eine Paketnummer ab
`number_of_packets` eines früheren Pakets, Größe über 16 MiB und
Timeout, gezählt ab dem letzten Paket der Collection; alle offenen
Collections zusammen halten höchstens 64 MiB, jedes Paket mit 64 Oktetten
Verwaltung gezählt, darüber geht die Collection mit dem ältesten letzten
Paket; jede nicht gelieferte Collection steht in `ReassemblyStats`),
`MiopSender` (TTL bzw. Hops, `unique_id` mit 16 Oktetten),
`join_group` und `MiopReceiver` (IPv4 und IPv6, `SO_REUSEADDR` und
`SO_REUSEPORT`, Interface wählbar). `crates/corba-ior/src/miop.rs` —
`UipmcProfileBody`, `GroupInfo`, `group_iiop_component`,
`group_profiles`, `is_group_reference`, `is_equivalent` und `hash` nach
§11.2.6.10, `parse_miop_corbaloc` (Group-ID mit Domain, Gruppe und
Referenzversion, IPv6 in Klammern, Group-IIOP nach `;`,
Gateway-Adressen nach `,`, nur Multicast-Adressen).
`crates/corba-ccm/src/orb_core.rs` — `ObjectRef::Group`:
`string_to_object` macht aus `corbaloc:miop:` und aus einem IOR mit
UIPMC-Gruppenprofil eine Gruppenreferenz, `object_to_string` behält das
Profil. `crates/corba-interop/src/miop.rs` — `MiopClient` (Oneway-Request
an die Gruppe, GIOP 1.2 mit dem UIPMC-Profil im Target, GIOP 1.0/1.1 mit
`"MIOP"` und dem Profil im Object-Key; `is_a` über das IIOP-Profil von
`TAG_GROUP_IIOP`, ohne diese Komponente über das Interface Repository
von `with_interface_repository` mit den Basis-Interfaces
(`crates/corba-ir/src/repository.rs::Repository::interface_is_a`,
`InterfaceDef::is_a` nach Part 1 §14.5.24.1), `non_existent`,
`validate_connection` einer Gruppenreferenz), `two_way_reference`
(`TAG_GROUP_IIOP` oder Gateway-Profil), `Goa` mit
`create_id_for_reference`, `reference_to_ids`,
`associate_reference_with_id`, `disassociate_reference_with_id`,
`activate_object_with_id`; der GOA tritt der Gruppe bei und liefert
jeden Request an alle Servants der Gruppe; Requests mit Antwort,
Nachrichten ohne Request und Gruppen ohne Mitglied zählt `GoaStats`.
`MiopGateway` leitet die Requests von IIOP-Clients an die Gruppe weiter
(`void`-Operationen mit leerer Antwort); die Operationen von
`CORBA::Object` gehen an das IIOP-Profil von `TAG_GROUP_IIOP` im
UIPMC-Profil, ohne diese Komponente ist `_non_existent` = `true` und die
übrigen sind `NO_IMPLEMENT`; ein `LocateRequest` erhält `OBJECT_HERE`
für ein Ziel mit Gruppenprofil, sonst `UNKNOWN_OBJECT`; ein nicht
lesbarer Header, `Reply`, `LocateReply` und ein `Fragment` ohne
vorangehende fragmentierte Nachricht erhalten `MessageError`, dann endet
die Verbindung; `CancelRequest` wird gezählt (`GatewayStats`).
`crates/corba-interop/src/mgm.rs` — `Mgm` mit PropertyManager,
`get_object_group_id`, `get_object_group_ref` (übrige
ObjectGroupManager-Operationen `NO_IMPLEMENT` nach §11.2.6.5),
`create_object` und `delete_object` mit den Ausnahmen ihrer
`raises`-Klauseln (nicht verstandene Kriterien von `create_object` sind
`InvalidCriteria`, Properties ohne mögliche Referenz bei
`set_properties_dynamically` `InvalidProperty`), allen Properties von
§11.2.6.8 (`GroupCreationMode` nur bei Erzeugung, `GroupDomainId` nur
einmal, Endpunkte und Gateway jederzeit, neue Referenzversion bei
geänderter Referenz), Adresspool für `CREATE_ADDRESS_GENERATED`.
`Mgm::serve` macht den MGM zum CORBA-Objekt auf einem `CorbaServer`:
alle Operationen von `MGM::ObjectGroupFactory` und `CORBA::Object` über
IIOP, Ausnahmen mit ihren Membern; je Gruppe ein Objekt unter
`<key>/group/<id>` an derselben Adresse, das `_is_a` für den Typ der
Gruppe und seine Basis-Interfaces (Interface Repository) und
`_non_existent` = `false` beantwortet, nach `delete_object` `_is_a` =
`false` und `_non_existent` = `true` (§11.3.2). Ein nil
`SupportImplicitOperations` legt dieses Objekt in `TAG_GROUP_IIOP`; ein
MGM ohne `serve` hat keine Referenz und weist es ab
(`CannotMeetCriteria`, bei `set_properties_dynamically`
`InvalidProperty`). `MgmProxy` ruft einen MGM über IIOP. Die 14-Oktett-
Kopfzeile, `MiopConfig` und `MiopSender` aus
`crates/corba-ccm/src/orb_extensions.rs` sind entfernt.

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

**Status:** partial — §11.1 bis §11.3 gebaut, Tests grün auf dem
Linux-Testrechner (Multicast über 239.255.x.x und ff15::/16 im
eigenen Netzwerk-Namespace des Testlaufs), darunter gegen JacORB 3.9 in beiden
Richtungen: ein JacORB-GOA-Mitglied empfängt die Oneway-Requests, die
ZeroDDS an die Gruppe sendet, ein ZeroDDS-GOA-Mitglied die von JacORB,
je ein Request in einem Paket und einer in einer Collection aus mehreren
Paketen; ZeroDDS liest die Gruppenreferenz von JacORB, JacORB die
`corbaloc:miop`-URL. Nicht gelaufen: TAO (nicht auf dem Testrechner);
omniORB hat kein MIOP. Offen: §11.3.4.3 — die Laufzeit hat keine Initial
Reference `RootPOA`, die auf `GOA` eingeengt würde; der GOA wird mit
`Goa::new` erzeugt.

### §12 ZIOP Protocol

**Spec:** Part 2 §12, S. 219 — ZIOP-Nachricht (Magic `ZIOP`,
`CompressionData` an Stelle des GIOP-Body, GIOP 1.2 und später,
Fragmente eingeschlossen, §12.1), ZIOP nur gegenüber einer Gegenseite,
die es erklärt hat (§12.2), die vier Compression-Policies (§12.3) und
ihre Weitergabe in `TAG_POLICIES` und `INVOCATION_POLICIES` (§12.4,
Part 1 §17.4).

**Repo:** `crates/corba-giop/src/ziop.rs` (`CompressionData`,
`compress_message`, `decompress_message`, `ZiopCodec`, `ZiopPolicies`,
`PeerZiop`, `ZiopSend`, Policy-Typen 64 bis 67 mit ihren `pvalue`-Formen),
`crates/corba-giop/src/messaging.rs` (`PolicyValue`, `TAG_POLICIES`,
`INVOCATION_POLICIES`, Part 1 §17.4),
`crates/corba-giop/src/header.rs::MessageHeader::decode_frame` (Magic
`ZIOP` ab GIOP 1.2), `crates/corba-iiop/src/framing.rs::{read_giop_message_ziop,
write_giop_message_ziop}`, `crates/corba-iiop/src/connection.rs::Connection::{set_ziop_codec,
write_message_ziop}`, `crates/corba-ccm/src/compression.rs`
(`CompressionManager` als `ZiopCodec`), `crates/corba-ccm/src/orb_core.rs::{ziop_policy,
ziop_policies, apply_ziop_policies, client_ziop_policies}` (`Orb::create_policy`
für die Typen 64 bis 67, `Orb::{client_ziop_policies,
server_ziop_policies}`), `crates/corba-ccm/src/policy_management.rs`
(Overrides auf ORB-, Thread- und Referenz-Ebene, Part 1 §8.9),
`crates/corba-interop/src/runtime.rs` (`IiopCorbaConnection::with_ziop`,
`CorbaServer::{set_ziop, object_reference}`). Die Client-Policies einer
Invocation: je ZIOP-Policy-Typ der Override der Referenz
(`Object::set_policy_overrides`), sonst des aufrufenden Threads
(`PolicyCurrent`), sonst des ORB (`ORBPolicyManager`), über den eigenen
Policies von `with_ziop` oder den ORB-Defaults; Kompressoren aus
`with_ziop`, sonst der `CompressionManager` des ORB. Ein Client, der
Kompression einmal freigegeben hat, erklärt sie für eine Invocation ohne
ZIOP-Policies als abgeschaltet, weil der Server die letzte Erklärung für
die Verbindung behält. Ein `CorbaServer` ohne `set_ziop`, der an ein ORB
gebunden ist, nimmt die ZIOP-Overrides der ORB-Ebene mit dessen
`CompressionManager`. Der Client sendet seine
client-exposed Policies (64, 65) in jedem Request als
`INVOCATION_POLICIES`. Ein Request geht nur dann als ZIOP-Nachricht, wenn
die Referenz in `TAG_POLICIES` ZIOP freigibt und einen gemeinsamen
Kompressor nennt: der erste Eintrag der eigenen Liste, den auch die
Gegenseite nennt, mit der niedrigeren Stufe (§12.3.2). Der Server liest
ZIOP-Requests nur, wenn er ZIOP freigegeben hat; sonst antwortet er mit
`MessageError` und schließt die Verbindung. Die Policies eines Clients
gelten für seine Verbindung, bis er andere sendet (omniORB sendet sie nur
im ersten Request). Low Value und Min Ratio des Senders halten eine
Nachricht als GIOP; ohne Min Ratio geht eine Nachricht nur komprimiert,
wenn die ZIOP-Nachricht kürzer ist. ORB-Defaults: Kompression aus, zlib
Stufe 6, Low Value 1024 Oktette. Ein `TAG_POLICIES`, das nicht dekodiert,
ergibt `INV_OBJREF`, ein solches `INVOCATION_POLICIES` `MARSHAL`.

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

**Status:** done — Tests grün auf dem Linux-Testrechner, darunter gegen
omniORB 4.3.3 (omniZIOP, zlib) in beiden Richtungen: Requests und Replies
gingen als ZIOP-Nachrichten über die Leitung (von einem Relay
festgestellt) und wurden auf der jeweils anderen Seite gelesen. Die
Client-Policies auf ORB-, Thread- und Referenz-Ebene (§12.2) über die
Overrides von Part 1 §8.9, nur über `create_policy` gesetzt, je Ebene
mit Relay geprüft, ebenso der Vorrang Referenz vor Thread vor ORB; ein an
ein ORB gebundener Server erklärt und nutzt die ZIOP-Policies der
ORB-Ebene. Nicht gelaufen:
TAO (nicht auf dem Testrechner) und JacORB (ZIOP-Peer nicht geschrieben).

---

## Part 3: Component Model (CCM)

### §1, §3–§5 Scope/References/Terms/Symbols

**Spec:** Part 3 §1, §3–§5, S. 1-7 — Geltungsbereich, Verweise,
Begriffe, Symbole.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Geltungsbereich, Verweislisten und
Begriffe.

### §2 Conformance and Compliance

**Spec:** Part 3 §2, S. 1–3 — Konformitätspunkte: Basic Level (für
Nicht-Java-Produkte IDL-Erweiterungen, CIDL ohne Segmente, ein
Container, XML-Deployment-Deskriptoren), Java-Basic-Level (EJB 1.1,
RMI/IIOP, EJB-zu-IDL), Extended Level, Lightweight-CCM-Profil, EJB-Clients
(optional), Generic Interaction Support (`porttype`, `mirrorport`,
`port`, Templates, Connectoren, D&C-Erweiterungen).

**Repo:** `crates/corba-ccm/src/container.rs`, `crates/corba-ccm/src/cidl.rs`,
`crates/corba-ccm-ejb/`, `crates/corba-ccm-lib/`.

**Tests:** `crates/corba-ccm/src/container.rs::tests::install_and_full_lifecycle_round_trip`.

**Status:** partial — die Konformitätspunkte hängen an den Items von
Part 3 und folgen deren Status: Punkt 1 (Änderungen an Life Cycle,
Transaction und Security Service) an §6.13 (partial: der Security-Teil
fehlt); Punkt 2 und 4 (Basic Level: IDL-Erweiterungen, CIDL, Container,
XML-Deskriptoren) an §6, §8, §9, §10 und §15 — CIDL mit Parser ist
gebaut (§8, done), es fehlt der Rest von §9 (partial); Punkt 3 und 6 (Extended Level) an §6 bis §10 und
§15; Punkt 5 und 8 (Java Basic Level, EJB-Clients) an §11 (done); Punkt 7
(Lightweight-Profil) an §14 (done, `crates/corba-ccm/src/lwccm.rs`);
Punkt 9 (Generic Interaction Support mit D&C-Erweiterung) an §7 (done).
Ein Test, der jeden Punkt mit seinem Beleg verbindet
(`crates/corba-ccm/tests/conformance_points.rs`), ist nicht gebaut: der
CIDL-Parser, den er braucht, existiert (§8); die Punkte 2 bis 4 brauchen
außerdem den CIF von §9, der partial ist.

### §6 Component Model

**Spec:** Part 3 §6, S. 9 — Component-Definition, Facets, Receptacles,
Events, Homes, Home-Finders, Configuration, Inheritance, Conformance.

**Repo:** `crates/corba-ccm/src/component_def.rs`,
`crates/corba-ccm/src/home.rs`, `crates/corba-ccm/src/port.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm/src/lifecycle.rs`.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests` (30 Tests), darunter
`trader_has_4_ports`, `keyed_component_detected`,
`provide_facet_returns_facet_by_name`, `get_all_facets_returns_complete_list`,
`get_all_publishers_excludes_emit_only_sources`,
`supports_interface_returns_true_for_listed_iface`,
`the_base_chain_runs_from_the_derived_component_to_the_root`,
`flattening_takes_over_every_inherited_feature_bases_first`,
`a_redefined_port_name_is_refused_rather_than_shadowed`,
`a_second_primary_key_in_the_chain_is_refused`;
`crates/ast/src/cidl.rs::tests::corba_p3_8_5_1_session_and_service_forbid_binding_primary_key_finders_and_segments`;
die Unter-Items §6.5-§6.13 nennen je einen Test pro Klausel.

**Status:** done

#### §6.5 Facets and Navigation

**Spec:** §6.5, S. 13 — `provide_facet()`, navigation.

**Repo:** `crates/corba-ccm/src/component_def.rs`
(`provide_facet`/`get_all_facets`/`get_named_facets` — Navigations-
Interface), `crates/corba-ccm/src/lifecycle.rs` (`check_facet_lifetime`
— Facet-Referenz-Lebensdauer).

**Tests:** `crates/corba-ccm/src/component_def.rs::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown, get_all_facets_returns_complete_list,
get_named_facets_filters_by_names}`,
`crates/corba-ccm/src/lifecycle.rs::{facet_lifetime_passes_when_alive,
facet_lifetime_rejects_use_after_destroy, facet_lifetime_rejects_orphaned}`.

**Status:** done

#### §6.6 Receptacles

**Spec:** §6.6, S. 20 — `connect_*`/`disconnect_*`, Multiple
Receptacles.

**Repo:** `crates/corba-ccm/src/port.rs` (`PortRegistry::connect_receptacle`/
`disconnect_receptacle`/`get_connections`), `crates/corba-ccm/src/lifecycle.rs`
(`ReceptacleManager` mit Simplex-/Multiplex-Regel),
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

**Spec:** §6.7 — EventSource (publishers/emitters) und EventSink.

**Repo:** `crates/corba-ccm/src/port.rs` (Event-Ports, `EventStream`),
`crates/corba-ccm/src/component_def.rs` (`EventSourceDef.emit_only`
trennt `emits` von `publishes`; `EventSinkDef`).

**Tests:** `crates/corba-ccm/src/port.rs::{event_sink_subscription_round_trip,
list_event_sinks_unknown_source_empty}`,
`crates/corba-ccm/src/component_def.rs::{event_source_emit_only_default_false,
get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources,
get_named_publishers_respects_emit_only_flag,
get_named_emitters_excludes_publishers}`.

**Status:** done — Event-Port-API live; Wire-Pfad für Emitter geht
über CosEvent (siehe `cos-event-service-1.4.md`).

#### §6.8 Homes / §6.9 Home Finders

**Spec:** §6.8/§6.9, S. 34/42 — Home-Lifecycle, HomeFinder.

**Repo:** `crates/corba-ccm/src/home.rs` (`HomeDef` mit optionalem
Primary Key, `HomeFinder` mit `create`/`find_by_primary_key`/`remove`).

**Tests:** `crates/corba-ccm/src/home.rs::{home_def_default_has_no_primary_key,
keyed_home_has_primary_key, create_with_user_key_uses_key_as_id,
create_without_key_generates_system_id, find_by_primary_key_round_trip,
remove_decrements_instance_count, remove_unknown_returns_false,
home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done

#### §6.10 Component Configuration / §6.11 Attributes

**Spec:** §6.10/§6.11 — Configurator-API, Attribute-Set.

**Repo:** `crates/corba-ccm/src/lifecycle.rs` (`Configurator`-Trait,
`StandardConfigurator` über den `AttributeDef`-Satz,
`ConfiguratorRegistry`), `crates/corba-ccm/src/context.rs`
(`ComponentContext`), `crates/corba-ccm/src/component_def.rs`
(`AttributeDef` mit `readonly`/`set_raises`/`get_raises`).

**Tests:** `crates/corba-ccm/src/lifecycle.rs::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute, configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`,
`crates/corba-ccm/src/component_def.rs::tests::attribute_readonly_skips_set_raises`,
`crates/corba-ccm/src/context.rs::{anonymous_context_returns_none,
principal_context_returns_bytes}`.

**Status:** done

#### §6.12 Component Inheritance

**Spec:** §6.12, S. 49 — Vererbung von Component-Definitionen.

**Repo:** `crates/corba-ccm/src/component_def.rs::{ComponentTypeTable,
InheritanceError}` — `base_chain` läuft die `base_component`-Kette vom
abgeleiteten Typ zur Wurzel, `is_equivalent_component_kind` prüft eine
Repository-ID gegen die ganze Kette, `flatten` übernimmt Facets,
Receptacles, Event-Ports, Attribute, `supports`-Interfaces und den
Primary Key der Basen und weist eine redefinierte Port- oder
Attribut-Bezeichnung sowie einen zweiten Primary Key zurück.

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

**Spec:** §6.13, S. 51 — zehn Conformance-Punkte. Punkt 1 mit §6.13.2:
`CosLifeCycle::FactoryFinder::find_factory` (Schlüssel als INS-Name an
`HomeFinder::find_home_by_name`, Default-Factory der Home als
`GenericFactory`), `CosTransactions::Current` und die aufgezählten
Interfaces von `SecurityLevel1`, `SecurityLevel2` und
`SecurityReplacable` als lokale Interfaces. Punkte 2 bis 9: die Stufen
Basic, Extended, Lightweight CCM und der EJB-Zugang. Punkt 10: jedes
Produkt parst jede gültige IDL.

**Repo:** `crates/corba-ccm/src/cos_lifecycle.rs` (`FactoryFinder` mit
`find_factory` und `find_factories` über
`ComponentHomeFinder::{find_home_by_name, homes_by_name}`,
`GenericFactory`, `HomeFactory` als Default-Factory einer Home ohne
Primary Key, `NoFactory`, `InvalidCriteria`, `Key` = `CosNaming::Name`
in INS-Form); `crates/corba-cos-transactions/src/current.rs::TransactionCurrent`
(lokales `CosTransactions::Current`, je Thread ein `Current`) als
Initial Reference `"TransactionCurrent"` des ORB
(`crates/corba-ccm/src/orb_core.rs::orb_init`); ein lokales Objekt hat
keine Referenz, `object_to_string` ergibt `MARSHAL` Minor 4. Punkt 10:
`zerodds-idl-parse` und `zerodds-idl-lower` lesen die Grammatik von Part
3 (Component, Home mit Factory, Finder und Primary Key, Eventtype, alle
Port-Arten, `porttype`, `port`, `mirrorport`, `connector`,
Template-Module und ihre Instanzen, `typeprefix`, `typeid`) in jedem
Dialekt ohne Fehler, weder beim Parsen noch beim Lowering. Ausnahme: ein
Connector mit leerem Rumpf. Part 3 Regel (6) erlaubt ihn
(`<connector_export>*`), OMG IDL 4.2 Regel (180), der neuere Text,
verlangt mindestens einen Export (`+`); das Lowering folgt IDL 4.2 und
weist ihn in jedem Dialekt ab (`IDL0001`).

**Tests:** `crates/corba-ccm/src/cos_lifecycle.rs::tests::{corba_p3_6_13_2_1_find_factory_goes_through_the_home_finder,
corba_p3_6_13_2_1_no_factory_without_a_keyless_home_bound_to_the_key,
corba_p3_6_13_2_1_find_factories_returns_every_keyless_home_of_the_name,
corba_p3_6_13_2_1_the_default_factory_takes_no_criteria_and_no_other_key}`,
`crates/corba-cos-transactions/src/current.rs::tests::{corba_p3_6_13_2_2_each_thread_has_its_own_transaction_context,
corba_p3_6_13_2_2_two_transaction_currents_do_not_share_a_context,
corba_p3_6_13_2_2_set_timeout_reaches_the_next_transaction}`,
`crates/corba-ccm/src/orb_core.rs::tests::corba_p3_6_13_2_2_transaction_current_is_a_local_initial_reference`,
`crates/idl-lower/tests/dialects.rs::corba_p3_6_13_point_10_the_component_grammar_parses_in_every_dialect`.

**Status:** partial — Punkt 1 (Life Cycle, Transaction) und Punkt 10
gebaut, Tests grün auf dem Linux-Testrechner. Security Service: ZeroDDS
liefert keinen CORBA Security Service (`SecurityLevel1`,
`SecurityLevel2`, `SecurityReplacable`); die Sicherheit der ORB-Schicht
ist CSIv2 (Part 2 §10). Die Änderung nach §6.13.2.3 bindet den
Hersteller dieses Dienstes. Offen: Punkte 2 bis 9 sind die
Conformance-Punkte von Part 3 §2 und haben dort ihren Nachweis (dort
offen: CIF §9).

### §7 Generic Interaction Support (IDL3+)

**Spec:** Part 3 §7, S. 55 — Extended Ports (`porttype`, `port`,
`mirrorport`) und ihre Übersetzung in Basic Ports (§7.2.2.2), Connectoren
mit Attributen, Vererbung und zusammengesetzten Implementierungen
(§7.2.3), Template-Module (§7.3, §7.4), das Programmiermodell der
Fragmente (§7.5: `CCMObject` mit Navigation und Receptacles,
`KeylessCCMHome`, `HomeConfiguration`, `configuration_complete`), die
D&C-Erweiterung (§7.6: `ConnectorPackageDescription`,
`ConnectorImplementationDescription`, Port-Arten `ExtendedPort` und
`ExtendedMirrorPort` mit `templateParam`, Assembly-Verbindung über einen
Connector, Flattening in Fragment-Instanzen, Ausführungsparameter
„home factory“, Konfiguration der Fragmente) und die Metamodell-Klassen
`ExtendedPortType`, `ExtendedPortDef`, `ConnectorDef` (§7.6.5).

**Repo:** IDL3+: `crates/idl-parse/src/decl.rs` (`porttype`, `port`,
`mirrorport`, `connector` mit Vererbung), `crates/idl-lower`
(Template-Module, Instanzen, `alias`; `src/lower.rs::{bind_actual,
constraint_violation}` prüft jeden Aktualparameter gegen die
Typbeschränkung seines Formals), `crates/ccm/src/transform.rs::{expand_extended_ports,
transform_connector}` (äquivalentes IDL). Laufzeitmodell:
`crates/corba-ccm/src/connector.rs` (`PortTypeDef`, `ConnectorDef`,
`ConnectorTypeTable::basic_ports` nach §7.2.2.2 und §7.2.3.3,
`ConnectorTypeTable::port_types_match` nach §7.6.5);
`crates/corba-ccm/src/fragment.rs` (`ConnectorFragment` als
`CCMObject`, `ConnectorHome` mit `create_component`,
`remove_component`, `set_configuration_values`, `FragmentExecutor`,
`ConnectorHomeExecutor` als `HomeExecutorBase`, `HomeFactories`,
`LocalFacet` für die Typprüfung von `connect`);
`crates/corba-ccm/src/orb_core.rs::IfrCcmMetamodel::{add_port_type,
add_extended_port, add_connector}`. D&C: `crates/corba-dnc/src/connector.rs`
(Datenmodell von §7.6.2 mit `Capability`, `SatisfierProperty`,
`Requirement` und `ConnectorPackageDescription::select_implementation`,
`flatten_assembly` nach §7.6.3),
`crates/corba-dnc/src/property.rs` (typisierte `configProperty`),
`crates/corba-dnc/src/xml.rs` (Plan, `.ccd`,
Connector-Package mit `<capability>`, Assembly mit `<selectRequirement>`
lesen und schreiben),
`crates/corba-dnc/src/launch.rs::{PlanLauncher, launch_plan}`
(Einstiegspunkt „home factory“, `set_configuration_values`, bereitgestellte
Referenzen je Knoten, Basic-Verbindungen über Knoten hinweg,
`configuration_complete`, Abbau).

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

**Status:** done — alle Teile gebaut, Tests grün auf dem
Linux-Testrechner. Template-Module: jeder Aktualparameter wird gegen die
Typbeschränkung seines Formals geprüft (§7.3.2.1: `typename`,
`interface`, `valuetype`, `eventtype`, `struct`, `union`, `sequence`,
`enum`, `const` eines Typs, Sequenzspezifikation; `exception` nach IDL
4.2 §7.4.12.4.1); ein `typedef` gilt als der Typ, den er nennt. Ein
Verstoß ist eine Diagnose (`IDL0019`), auch für das, was ein `alias`
weiterreicht. Ein Interface als Aktualparameter ist im Rumpf
Objektreferenz und Basis, eine Exception steht in `raises`. Eine
Monolith-Implementierung gibt je bedienter Komponente ein Fragment auf
deren Knoten; eine Assembly-Implementierung setzt jeden Teil zu der
Komponente, deren Connector-Port auf ihn abgebildet ist, und
verschachtelte Teile mit ihm; ein Teil, den keine Komponente erreicht,
wird abgewiesen. Ein Teil ist ein Connector-Fragment oder eine
Komponente (§7.2.3.4); eine Komponente als Teil wird wie die Fragmente
neben ihr platziert und konfiguriert und vom Container erzeugt. Die
Planung nimmt die erste Implementierung eines Package, deren
Capabilities die Selection Requirements der Verwendung oder des Teils
erfüllen (§7.6.2.3, §15.9.1); erfüllt keine sie, ist das ein Fehler. Die
sechs Arten von `SatisfierProperty` prüfen die Typen von §15.4.5. Die
Richtung von `Minimum` (Anforderung mindestens der Wert) und `Maximum`
(Anforderung höchstens der Wert) ist aus D&C 4.0 übernommen und nicht
gegen dessen Text geprüft; der Text ist nicht im Haus. Implementierungen
von Komponenten tragen in diesem Datenmodell keine Capabilities; eine
Komponenteninstanz mit Selection Requirements wird abgewiesen. Der Start
folgt der Aufteilung von D&C: `start_launch` gibt je Knoten die
Facet-Referenzen der dort bereitgestellten Verbindungen zurück,
`finish_launch` verbindet die Receptacles des Knotens auch mit Facets
anderer Knoten, `launch_plan` macht so jede Verbindung des Plans genau
einmal; fehlt die Referenz eines anderen Knotens, ist das ein Fehler.
Der Test führt die Launcher beider Knoten in einem Prozess aus. Das XML
ist die Deskriptorform von `crates/corba-dnc` (Elemente nach den
Klassen- und Feldnamen von §7.6.2 und §16), nicht das XMI-Schema von
§17 (eigenes Item); der Plan legt `ArtifactDeploymentDescription` in die
Artefaktliste der Implementierung. Eine Instanz nennt ihre
Implementierung als Text oder als Referenz wie im Beispiel von §7.6.4
(`<implementation ref="fragment_impl_1"/>`, auch `xmi:idref`). Kein
Nachweis gegen ein fremdes D&C-Werkzeug (CIAO DAnCE ist nicht im
Haus).

### §8 OMG CIDL Syntax and Semantics

**Spec:** Part 3 §8, S. 81 — Component Implementation Definition
Language (Composition, Home/Segment Executor, Persistence,
Facet/Feature Delegation, Proxy Home).

**Repo:** `crates/ast/src/cidl.rs::{CidlModel, Composition,
CompositionCategory, HomeExecutorDef, ExecutorDef, SegmentDef,
FeatureDelegation, Delegation, ProxyHomeDef, HomeFacts,
StorageHomeFacts, StorageDecl, Composition::validate}` — ein
Datenmodell für Compiler und Laufzeit: Composition mit Home Executor
(`implements`, `bindsTo`, `storedOn`, Executor, Delegationen,
`abstract`), Segmente mit Segment-ID (Komponentensegment 0), Feature-
Delegation, Proxy Home und die PSDL-Deklarationen; `validate` prüft die
Regeln, die Namen lesen: Tabellen 8.3 bis 8.5 gegen Primärschlüssel,
explizite Finder und Segmente, §8.11 (Segmentnamen und -IDs eindeutig,
nicht 0), §8.13 (Facet der Komponente, in höchstens einem Segment),
§8.14 (nur mit `bindsTo`, zustandsbehaftetes Feature, Storage-Member
vorhanden, je Feature eine Delegation), §8.15 (nur mit `bindsTo`,
Home-Operation, Ziel auf der Storage Home, je Operation ein Ziel und je
Ziel eine Operation), §8.16 (nur Factories, explizit oder `create`),
§8.17 und §8.18 (Home-Operationen). `crates/corba-ccm/src/cidl.rs`
exportiert das Modell und baut mit `home_facts` die Fakten aus
`HomeDef` und `ComponentDef`; `crates/corba-ccm/src/lwccm.rs::composition_exclusion`
liest es (Persistenz, Segmentierung, CIDL nach LwCCM §13).

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

**Status:** done — gebaut und auf dem Linux-Testrechner grün. Das
Modell liegt in `zerodds-ast`, weil der Compiler (`zerodds-idl-lower`)
es erzeugt und nicht von `zerodds-corba-ccm` abhängen kann;
`zerodds-corba-ccm` exportiert es. Die Regeln, die Typen lesen
(Tabelle 8.6, Signaturen nach §8.15), prüft der Compiler; sie stehen im
Item „§8 OMG CIDL — verbleibende Konstrukte“. `HomeDef` der Laufzeit
kennt keine expliziten Operationen; ein Aufrufer, der sie kennt, trägt
sie in die Fakten ein.

#### §8 OMG CIDL — verbleibende Konstrukte

**Spec:** Part 3 §8, S. 81 — die CIDL-Grammatik selbst (§8.2 bis §8.18,
Produktionen (1) bis (34)) mit den PSDL-Deklarationen, die sie enthält
(Persistent State Service 2.0 §3.2.3, Produktionen (1) bis (48)), die
Schlüsselwörter von Tabelle 8.2, `.cdl`-Dateien (§8.1) und die Regeln
der Kategorien, Segmente, Feature-, Storage-Home- und Executor-
Delegation, `abstract` und Proxy Home.

**Repo:** `crates/idl-cst/src/token.rs::{CIDL_KEYWORDS, cidl_keyword,
cidl_keyword_collision}` und die CST-Arten in `crates/idl-cst/src/tree.rs`;
`crates/idl-parse/src/dialect.rs::Dialect::cidl`,
`crates/idl-parse/src/parser.rs::Parser::mark_cidl_keywords`,
`crates/idl-parse/src/cidl.rs` (Parser für CIDL und PSDL),
`crates/idl-parse/src/diagnostic.rs` (`IDL0024`, `CompositionRule`);
`crates/idl-lower/src/cidl.rs` und `crates/idl-lower/src/lower.rs::lower_cidl`
(PSDL-Deklarationen als lokale Interfaces des Modells; Auflösung der
Namen; Tabelle 8.6; Signaturregeln von §8.15 mit den orthodoxen
Operationen nach §6.8.1; Namens- und Member-Regeln von PSS 2.0 §3.2.5
und §3.2.6; Schlüsselwort-Kollision nach Groß-/Kleinschreibung);
`crates/idl-compose/src/lib.rs::Composed::cidl`; `tools/idlc` (`.cdl`
wird als CIDL gelesen, `dump-cidl`).

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

**Status:** done — gebaut und auf dem Linux-Testrechner grün. Wo Text
und Grammatik von §8 auseinandergehen, liest der Parser beide Formen:
`delegatesTo storage` (Produktion (20)) und `delegatesTo abstract
storagetype` (Text von §8.14); `provides facet a, b;` (Produktion (19))
und `provides (a, b);` mit `;` nach dem Segment (Schema in §9.2.9.1).
Die Kataloge der Beispiele in §9.2.7 (`uses catalog`) stammen aus PSS
1.0 und stehen in keiner der beiden Grammatiken; sie sind Syntaxfehler.
Der Parser hält die Reihenfolge von Produktion (8) ein (`implements`,
`bindsTo`, `storedOn`, `manages`, `delegatesTo abstract storagehome`,
`delegatesTo executor`, `abstract`, jedes höchstens einmal,
`implements` und `manages` Pflicht) und meldet ein Mitglied außerhalb
dieser Reihenfolge, ein zweites derselben Art, ein fehlendes
`implements` oder `manages`, eine Delegation an einer Stelle, die ihre
Produktion nicht vorsieht (Home Executor: `abstract storagehome` und
`executor`; Executor-Rumpf: `storage`; Proxy Home: `home`), und einen
leeren Executor-Rumpf, ein leeres Segment und eine leere Proxy Home
(`+` der Produktionen (14), (16), (32)); das Mitglied bleibt in einem
Fehlerknoten. Die Übersetzung meldet einen solchen Knoten, der sie
trotzdem erreicht, und ein zweites `storedOn` eines Segments (§8.12).
`find_by_<key>` hebt `CosPersistentState::NotFound`, `find_ref_by_<key>`
gibt `ref<S>` zurück (PSS 2.0 §3.2.5.7); die Regel von §8.15 zu
Ausnahmen gilt auch für sie. `const` einer lokalen Operation steht im
Modell (`StorageDecl::operations`, `LocalOperation::is_const`) und in
`dump-cidl`. Lesarten, die die Spezifikation offen lässt: die
Home-Operationen, die eine Delegation nennen kann, sind die expliziten
des Homes und seiner Basen und die orthodoxen von §6.8.1, nicht die von
`Components::CCMHome`; ein Parameter vom Valuetype folgt in §8.15 der
Regel für Objektreferenzen. Die Unterdrückung der Default-Delegation am Ende von §8.15 betrifft die
erzeugte Implementierung und gehört zum CIF (§9). Von PSS 2.0 prüft
die Übersetzung die Namen (`of`, Basen, `implements`, `as`, `scope`,
`ref<…>`) und die State-Member von Schlüsseln, Factories, `ref(…)` und
`stores`; nicht geprüft werden die Vergleichbarkeit der Schlüsseltypen
(§3.2.5.7), Namenskonflikte bei Mehrfachvererbung und die implizite
Basis `CosPersistentState::StorageObject` (§3.2.5.5); PSS 2.0 hat in
diesem Repository kein eigenes Abdeckungsdokument. Kein Nachweis gegen
einen fremden CIDL-Compiler: keiner ist im Haus (OpenCCM, MicoCCM und
CIAO wurden nicht geprüft); der Nachweis stützt sich auf den
Grammatiktext und die Fixtures.

### §9 CCM Implementation Framework (CIF)

**Spec:** Part 3 §9, S. 93 — CIF-Architektur: Composition als
Implementierungseinheit (§9.2.5, §9.2.6), Managed Storage und Delegation
der Home-Operationen an die Abstract Storage Home oder den Executor
(§9.2.7, §9.2.8), Executor-Definition mit Segmenten und Delegation des
Feature-Zustands (§9.2.9), Proxy Homes (§9.2.10), Referenzinformation
mit Facet-, Segment- und State-Identifiern (§9.2.11); Sprach-Mapping
über äquivalente lokale Interfaces (§9.3).

**Repo:** `crates/ccm/src/executor.rs` (§9.3.3: `CCM_<I>` je Interface,
`CCM_<E>Consumer` je Eventtype, `CCM_<C>_Executor`, `CCM_<C>`,
`CCM_<C>_Context` je Komponente mit den Port-Operationen von §9.3.3.5,
`CCM_<H>Explicit`, `CCM_<H>Implicit` schlüssellos und mit Schlüssel,
`CCM_<H>` je Home; `executor_idl` für ein ganzes Modell);
`crates/corba-ccm/src/cif.rs` (§9.3.2: `ExecutorLocator` mit
`obtain_executor`, `release_executor`, `configuration_complete`,
`ComponentImplementation` monolithisch oder Locator mit `invoke` je
Aufruf; §9.2.9.1: `SegmentLayout` mit Facet- und Segment-IDs und den
Regeln für Segmente, `SegmentedComponent` aktiviert nur das Segment der
angefragten Facet über `SegmentFactory::create_executor_segment`;
§9.2.11: `ComponentReferenceInfo` monolithisch und segmentiert mit
Kodierung im `object_key` und Komponentenidentität).

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

**Status:** partial — gebaut und auf dem Linux-Testrechner grün: das
äquivalente Executor-IDL von §9.3 mit allen Regeln von §9.3.3, die beiden
Strategien von §9.3.2 als Bausteine (`ComponentImplementation`,
`ExecutorLocator`), Segmente nach §9.2.9.1 (`SegmentedComponent`) und
die Kodierung der Referenzinformation nach §9.2.11
(`ComponentReferenceInfo`). Es fehlt ihre Einbindung in Container und
ORB: `Container` installiert ein `ComponentExecutor` und leitet keine
Aufrufe an Facets weiter, ruft also kein `obtain_executor` und
`release_executor` je Aufruf (§9.3.2: „the container will invoke its
obtain_executor operation prior to each invocation“); `ExecutorLocator`
ist nicht an `SessionComponent` und `EntityComponent` gebunden, wie
§9.3.2 es für eine Locator-Implementierung verlangt; keine
Objektreferenz einer Komponente trägt `ComponentReferenceInfo` im
`object_key` (§9.2.11: „It is the responsibility of the container and
the underlying ORB to encode this information for insertion into object
references“). Es fehlt außerdem die Erzeugung aus
CIDL-Compositions: die vollständige Implementierung des Home-Executors
und die Executor-Skelette je Segment (§9.2.5, §9.2.6, §9.2.9), die
implizite Delegation der Finder an die Abstract Storage Home und die
Bindung des Primary Key an den Matching Key (§9.2.8.1, §9.2.8.2), die
explizite Delegation an Storage Home und Executor mit Factory-Facet und
`abstract` (§9.2.8.3), die Delegation des Feature-Zustands an
Storage-Member (§9.2.9.2) und Proxy Homes (§9.2.10). Woraus sie erzeugt
wird, existiert: der CIDL-Parser, das Composition-Modell und die
PSDL-Deklarationen der Abstract Storage Homes (§8, done; `lower_cidl`,
`zerodds_ast::cidl`). Das
Executor-IDL ist ein Transform des Modells; `idlc` und die
Sprach-Backends (`crates/lang-rules/src/emit/intermediate_idl.rs` hält
nur die Client-Seite) geben es noch nicht aus.

### §10 The Container Programming Model

**Spec:** Part 3 §10, S. 135 — Server- und Client-Programming
Environments, Basic/Extended Component Programming Interfaces.

**Repo:** `crates/corba-ccm/src/container.rs`,
`crates/corba-ccm/src/context.rs`, `crates/corba-ccm-lib/src/`
(Persistence-Bridge, Telemetry, DDS-Bridge).

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

**Spec:** Part 3 §11, S. 177 — CCM↔EJB-View-Mapping (CCM-Component →
EJB-View, EJB-Bean → CCM-View), TX-Bridging, Naming-Glue.

**Repo:** `crates/corba-ccm-ejb/` mit `connector_bean.rs`,
`naming_glue.rs`, `stub_gen.rs`, `tx.rs`.

**Tests:** `crates/corba-ccm-ejb/src/naming_glue.rs::tests` (8 Tests):
`bind_and_lookup_round_trip`, `rebind_replaces_ior`, `unbind_removes_entry`,
`jndi_global_strips_prefix`, `jndi_comp_env_strips_prefix`,
`cos_to_jndi_round_trip_no_kind`, `cos_to_jndi_includes_kind`;
`crates/corba-ccm-ejb/src/tx.rs::tests` (8): `cos_to_jta_round_trip_for_all_states`,
`begin_then_commit_succeeds`, `rollback_only_then_commit_fails_with_rollback`;
`crates/corba-ccm-ejb/src/connector_bean.rs::tests` (4),
`crates/corba-ccm-ejb/src/stub_gen.rs::tests` (5).

**Status:** done

### §12 Interface Repository Metamodel

**Spec:** Part 3 §12, S. 203 — IR-Metamodel-MOF-DTDs + IDL.

**Repo:** Erweiterungen in `crates/corba-ir/src/repository.rs` (MOF-
fähige `Container`/`Contained`-Hierarchie) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` mit MOF-2.0-Subset (Class/Property/Operation) +
XMI-1.2-Output für das Component-Modell.

**Tests:** `orb_core::tests::{xmi_emitter_empty_yields_minimal_doc,
xmi_emitter_class_with_inheritance, xmi_emitter_property_emits,
xmi_emitter_operation_with_parameters,
ifr_ccm_metamodel_add_component,
ifr_ccm_metamodel_ingest_repository_walks_definitions}`.

**Status:** done — Basis-IR (`corba-ir`) + MOF-2.0-Subset + XMI-1.2-
Emitter live; `IfrCcmMetamodel::from_repository(&corba_ir::Repository)`
walked die `Container`/`Contained`-Hierarchie und produziert die
Component-Modell-Klassen. Repository-Walker (`ingest_definition_into`)
ist getestet; Metamodel-Output ist als XMI-Doc serialisierbar.

### §13 CIF Metamodel

**Spec:** Part 3 §13, S. 289 — CIF-Metamodel-MOF-DTDs + IDL.

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF-AST) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement}`
für den XMI-Emitter symmetrisch zu §12.

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests::xmi_emitter_empty_yields_minimal_doc`,
`::xmi_emitter_class_with_inheritance`, `::xmi_emitter_property_emits`;
`crates/corba-ccm/src/cif.rs::tests::lifecycle_calls_increment_counters`,
`::cif_error_carries_diagnostic`.

**Status:** done — CIF-AST (`corba-ccm::cif`) + MOF-XMI-Emitter
symmetrisch zu §12 IFR; Repository-Walker ingestiert Component-/
Composition-Definitionen über `IfrCcmMetamodel::from_repository`.

### §14 Lightweight CCM Profile

**Spec:** Part 3 §14, S. 301 — Lightweight-Profile mit explizit
ausgenommenen Features (Persistence, Introspection,
Segmentation, Transactions, Security, Configurators, Proxy Homes,
Home Finders).

**Repo:** Implizit im aktuellen Container-Programming-Model:
Persistence ist optional (`crates/corba-ccm-lib/src/persistence.rs`),
TX optional via `crates/corba-ccm-ejb/src/tx.rs`.

**Tests:** `conformance_tests::corba_part3_14_marker_namespace`.

**Status:** done — Lightweight-Profile-Marker
`corba-ccm::conformance::CORBA_PART3_14_LIGHTWEIGHT_CCM_PROFILE`
+ vorhandene `LIGHTWEIGHT_CCM_LEVEL` als formales Conformance-
Statement.

### §15 Deployment PSM for CCM

**Spec:** Part 3 §15, S. 309 — D&C-PSM: PIM→PSM-Transformation,
PSM→IDL/XML.

**Repo:** `crates/corba-dnc/src/plan.rs`, `crates/corba-dnc/src/node.rs`,
`crates/corba-dnc/src/execution.rs`, `crates/corba-dnc/src/container_host.rs`,
`crates/corba-dnc/src/repository.rs`.

**Tests:** `crates/corba-dnc/src/execution.rs::{full_lifecycle_round_trip,
invalid_plan_rejected_at_prepare, double_start_fails, destroy_before_running_fails}`
(PIM→PSM-Lebenszyklus prepare/start/destroy),
`crates/corba-dnc/src/container_host.rs::{install_creates_container_on_demand,
install_then_activate_reaches_active, full_lifecycle_round_trip,
entity_and_session_share_host}` (PSM→Container-Abbildung).

**Status:** done

### §16 Deployment IDL for CCM

**Spec:** Part 3 §16, S. 329 — D&C-IDL.

**Repo:** als Rust-Repräsentation in
`crates/corba-dnc/src/plan.rs` (`DeploymentPlan`,
`InstanceDeploymentDescription`, `PlanConnection`),
`crates/corba-dnc/src/repository.rs` (`RepositoryManager`,
`PackageConfiguration`), `crates/corba-dnc/src/node.rs`
(`NodeManager`, `NodeApplication`,
`NodeApplicationManager`), `crates/corba-dnc/src/execution.rs`
(`ExecutionManager`, `DomainApplicationManager`).

**Tests:** Ein Test je IDL-Interface-Operation-Gruppe —
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

**Spec:** Part 3 §17, S. 343 — XML-Schema für Deployment-Pläne.

**Repo:** `crates/corba-dnc/src/xml.rs`.

**Tests:** `crates/corba-dnc/src/xml.rs::{parses_minimal_plan,
parses_multiple_artifacts, validation_fails_on_unknown_impl_reference,
missing_root_tag_returns_expected_tag, co_locate_with_parsed}`.

**Status:** done

---

## Beigeordnete OMG-Service: COS Naming v1.3 (formal/04-10-03)

CosNaming-Service ist eigene Spec, wird hier als Initial-Reference
gemäß Part 1 §8.5.2 mitgeführt.

### CosNaming::NamingContext

**Spec:** CosNaming v1.3 §2 — `bind`/`rebind`/`unbind`/`resolve`/
`new_context`, Iterator-basiertes `list()`, stringified Names
(`"a/b/c"`-Pattern via `NamingContextExt::resolve_str`).

**Repo:** `crates/corba-cosnaming/src/context.rs`,
`crates/corba-cosnaming/src/name.rs`,
`crates/corba-cosnaming/src/stringified.rs`.

**Tests:** `crates/corba-cosnaming/src/context.rs::tests` (13 Tests):
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

## ZeroDDS-spezifische Bridges (kein OMG-Item)

### CORBA-Object ↔ DDS-Topic Bridge

**Spec:** kein OMG-Spec-Item; ZeroDDS-spezifische Migrations-Schicht
zur schrittweisen Ablösung von CORBA-Endpoints durch DDS-Wire.

**Repo:** `crates/corba-dds-bridge/src/bridge_daemon/mod.rs` (der Daemon
`zerodds-corba-bridged`).

**Tests:** —

**Status:** n/a (informative) — kein OMG-Item; der Daemon
`zerodds-corba-bridged` ist in `zerodds-corba-bridge-1.0.md` erfasst, mit
seinen Tests.

---

## Audit-Status

44 done / 16 partial / 2 open / 1 blocked / 13 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document corba-3.3.md`):

* `cargo nextest run --profile merge -p zerodds-corba-ccm --all-targets` (Job `merge-rust-corba-ccm`) — 125 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-ccm-ejb --all-targets` (Job `merge-rust-corba-ccm-ejb`) — 10 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-codegen --all-targets` (Job `merge-rust-corba-codegen`) — 10 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-cosnaming --all-targets` (Job `merge-rust-corba-cosnaming`) — 13 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-csiv2 --all-targets` (Job `merge-rust-corba-csiv2`) — 30 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-dnc --all-targets` (Job `merge-rust-corba-dnc`) — 27 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-giop --all-targets` (Job `merge-rust-corba-giop`) — 48 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-iiop --all-targets` (Job `merge-rust-corba-iiop`) — 22 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-ior --all-targets` (Job `merge-rust-corba-ior`) — 25 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-ir --all-targets` (Job `merge-rust-corba-ir`) — 21 Tests grün.
* `cargo nextest run --profile merge -p zerodds-corba-poa --all-targets` (Job `merge-rust-corba-poa`) — 14 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-lower --all-targets` (Job `merge-rust-idl-lower`) — 7 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idl-parse --all-targets` (Job `merge-rust-idl-parse`) — 2 Tests grün.

354 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `corba-3.3.open.md`.
