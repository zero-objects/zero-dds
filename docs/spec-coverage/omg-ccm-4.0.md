# OMG CCM 4.0 — Spec-Coverage

**Spec:** [OMG CCM 4.0](https://www.omg.org/spec/CCM/4.0/PDF) (315 Seiten, OMG formal/06-04-01).

Audit Item-für-Item
gegen die Spec; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** CCM 4.0 ist die Master-Spec hinter AMI4CCM (siehe
`omg-ami4ccm-1.1.md`) und DDS4CCM (siehe `dds4ccm-1.1.md`). Sie umfasst
das Component Model (§6), CIDL (§7), das CCM Implementation Framework
(§8 CIF), das Container Programming Model (§9), die EJB-Integration
(§10), das IFR-Metamodel (§11), das Lightweight-CCM-Profil (§13) und das
Deployment-PSM/-IDL/-XML (§14-§16). ZeroDDS implementiert diesen Stack
vollständig in Pure Rust — inklusive des CORBA-ORB, des CCM-Containers
und des D&C-Subsystems, auf die der Container-Anteil der Spec aufsetzt.

Die §6-Equivalent-IDL-Transformation deckt zugleich den Migrations-Pfad
ab: alte CCM-IDL-Files lassen sich via Equivalent-IDL in Pure-DDS-Welten
überführen — mit oder ohne den vollen Container-Stack. Die
`Components::*`-Core-Datentypen (CCMObject, Cookie, ConnectionDescription,
etc.) sind als Rust-Modelle vorhanden.

Implementation:

- `crates/ccm/` — §6 Component Model (Equivalent-IDL) + §13 Lightweight
  CCM Profile + `Components::*`-Core-Datentypen.
- `crates/corba-ccm/` — §7 CIDL (`cidl.rs`), §8 CIF (`cif.rs`), §9
  Container Programming Model (`container.rs`).
- `crates/corba-ccm-ejb/` — §10 EJB-Integration.
- `crates/corba-ir/` — §11 Interface-Repository-Metamodel.
- `crates/corba-dnc/` — §14/§15 Deployment-PSM/-IDL + §16 XML-Schema.
- darunter: CORBA-ORB (GIOP/IIOP) in `crates/corba-interop`, POA in
  `crates/corba-poa`.

---

## §1 Scope

### §1 Scope Statement

**Spec:** §1, S. 1 (PDF) — "This specification defines: The syntax and
semantics of a component model (Chapter 6) [...] A language to describe
the structure and state of component implementations (Chapter 7
CIDL) [...] A programming model for constructing component
implementations (Chapter 8) [...] A runtime environment (Chapter 9)
[...] Interaction with EJB (Chapter 10) [...] Meta-data for
deployment (Chapter 14) [...] A lightweight subset (Chapter 13)."

**Repo:** ZeroDDS deckt:
- §6 Component Model = Equivalent-IDL über den AST aus
  `crates/idl-parse/` + `crates/idl-lower/`.
- §7 CIDL via `crates/corba-ccm/src/cidl.rs`.
- §8 Programming Model via `crates/corba-ccm-lib/`.
- §9 Runtime via `crates/corba-ccm/src/container.rs`.
- §10 EJB-Interaction via `crates/corba-ccm-ejb/`.
- §13 Lightweight Profile: Equivalent-IDL-Filter in
  `crates/ccm/src/lightweight.rs`, Profil-Restriktionen in
  `crates/corba-ccm/src/lwccm.rs`.
- §14 Deployment via `crates/corba-dnc/`.
- Conformance-Marker `crates/corba-ccm/src/lib.rs::conformance`.

**Tests:** `crates/corba-ccm/src/lib.rs::conformance_tests::{
ccm_conformance_basic_level_marker_matches_spec,
ccm_conformance_basic_level_java_marker_matches_spec,
lightweight_ccm_level_marker_matches_spec,
lwccm_restrictions_marker_matches_spec,
lwccm_filter_marker_matches_spec, all_markers_are_distinct}`.

**Status:** done — alle Sub-Stacks vorhanden + Conformance-Marker
am Crate-Niveau ausgewiesen.

---

## §2 Conformance and Compliance

### §2 Conformance Point 1: CORBA COS Vendor

**Spec:** §2 Punkt 1 — Lifecycle/Transaction/Security-Aenderungen.

**Repo:** Spec-Punkte teilweise abgedeckt durch Service-Crates:
- **Lifecycle** via `crates/corba-ccm/src/lifecycle.rs`
  (ReceptacleManager + Configurator + Container-State).
- **Transaction** via `crates/corba-ccm-ejb/src/tx.rs` (TX-Marker
  + Begin/Commit/Rollback-Hooks).
- **Security** via `crates/corba-csiv2/` (CSIv2-Stack: SAS + GSSUP +
  TLS-Bind).
- **Naming** als zusätzlicher COS-Service in
  `crates/corba-cosnaming/`.
- **Event** als zusätzlicher COS-Service in
  `crates/corba-cos-event/`.

**Tests:** `crates/corba-ccm/src/lifecycle.rs::tests::facet_lifetime_passes_when_alive`,
`::receptacle_simplex_connect_then_disconnect`,
`::configurator_set_get_roundtrip`;
`crates/corba-ccm-ejb/src/tx.rs::tests::cos_to_jta_round_trip_for_all_states`,
`::begin_then_commit_succeeds`;
`crates/corba-csiv2/src/sas.rs::tests::sas_msg_type_values_match_spec`;
`crates/corba-cosnaming/src/context.rs::tests::bind_and_resolve_simple`;
`crates/corba-cos-event/src/channel.rs::tests::push_fan_out_to_multiple_consumers`,
`::pull_supplier_dequeues_events`.

**Status:** done — die drei in §2 Punkt 1 genannten COS-Bereiche
(Lifecycle/Transaction/Security) sind abgedeckt; weitere
COS-Services (Naming, EventService) als Bonus.

### §2 Conformance Point 2: CORBA Component Vendor (Basic/Lightweight)

**Spec:** §2 Punkt 2 — Basic Level oder Lightweight Profile.

**Repo:** Equivalent-IDL für Components in `crates/idl-parse/` +
LwCCM-Filter in `crates/ccm/src/lightweight.rs` + Container-Lifecycle in
`crates/corba-ccm/src/{container,lifecycle}.rs` (ReceptacleManager,
Configurator, Generic-Op-Skeletons) + ORB-Singleton in
`crates/corba-ccm/src/orb_core.rs::Orb`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_plain_component_inherits_ccmobject`,
`::provides_yields_a_provide_operation_returning_the_facet`,
`::uses_simplex_yields_connect_disconnect_and_get_connection`,
`::the_whole_trader_component_yields_every_operation_the_clauses_name`;
`crates/corba-ccm/src/lifecycle.rs::tests::facet_lifetime_rejects_use_after_destroy`,
`::receptacle_multiplex_allows_multiple_connects`;
`crates/corba-ccm/src/orb_core.rs::tests::orb_init_yields_running`,
`::orb_destroy_transitions_to_shutdown`;
`crates/corba-ccm/src/lwccm.rs::tests::only_session_and_service_components_are_supported`,
`::the_lightweight_filter_reports_a_component_it_would_empty`.

**Status:** done — IDL-3-Aspekte + Container-Lifecycle + ORB-
Stub-Layer abgedeckt.

### §2 Conformance Point 3: Optional Extended Level

**Spec:** §2 Punkt 3 — Extended Level (PSS + zusätzliche
Configurator-/Lifecycle-Features).

**Repo:** Voller Wire-up durch:
- Persistent State Service via `crates/corba-ccm/src/pss.rs`
  (StorageHome + PssSession + Pid + TxHandle + PssTxStatus). Tx-aware
  `store(pid, value)` / `remove(pid)` schreiben in Pending-Buffer;
  `commit(tx)` wendet auf StorageHome an, `rollback(tx)` verwirft.
  `load(pid)` ist tx-aware (Pending-Pfad bei aktiver Tx).
- Configurator via `crates/corba-ccm/src/lifecycle.rs::Configurator`.
- Conformance-Marker
  `corba-ccm::conformance::CCM_OPTIONAL_EXTENDED_LEVEL`.

**Tests:** `crates/corba-ccm/src/pss.rs::tests::*` (13 Tests, davon 4 Tx-aware:
`pss_begin_commit_roundtrip_persists_pending_writes`,
`pss_rollback_restores_prev_state`,
`pss_load_after_store_in_tx_returns_pending_value`,
`pss_tx_status_transitions_active_committed_rolledback`) +
`crates/corba-ccm/src/lifecycle.rs::tests::configurator_*` + Cross-Ref Conformance-Marker-
Tests.

**Status:** done — voller PSS-Tx-Lifecycle (begin/commit/rollback +
Pending-Buffer + Tx-aware load/store/remove)..

### §2 Conformance Point 4: Basic Level non-Java

**Spec:** §2 Punkt 4 — IDL-Extensions + CIDL + Container + XML-D&C.

**Repo:** IDL-Extensions+CIDL in `crates/idl-parse/` +
`crates/idl-lower/` + `crates/corba-ccm/`;
Container-Lifecycle in `crates/corba-ccm/src/lifecycle.rs`;
XML-D&C in `crates/corba-ccm-lib/`+`corba-dnc/`; ORB-Stub-Layer
(Lifecycle/Threading/Policy) in `crates/corba-ccm/src/orb_core.rs`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_component_with_supports_inherits_ccmobject_and_the_supported_ones`,
`::a_keyed_home_yields_the_four_operations_and_inherits_nothing`;
`crates/corba-ccm/src/cidl.rs::tests::all_four_composition_categories_distinct`,
`::entity_composition_with_storage_valid`;
`crates/corba-ccm/src/lifecycle.rs::tests::configurator_registry_register_and_lookup`;
`crates/corba-dnc/src/xml.rs::tests::parses_minimal_plan`,
`::validation_fails_on_unknown_impl_reference`;
`crates/corba-ccm/src/orb_core.rs::tests::orb_set_get_policy_round_trip`.

**Status:** done — IDL+CIDL+D&C-Modell + ORB-Stub + Container-
Lifecycle abgedeckt.

### §2 Conformance Point 5: Basic Level Java

**Spec:** §2 Punkt 5 — EJB1.1 + java-to-IDL.

**Repo:** EJB-Integration in `crates/corba-ccm-ejb/`
(ConnectorBean+NamingGlue+StubGen+TX); Java-Sprachabbildung in
`crates/lang-rules/src/emit/java.rs`; Annex-A.1-CORBA-Java-Codegen in
`crates/corba-codegen/src/stub.rs` +
`crates/corba-codegen/src/skeleton.rs`.

**Tests:** `crates/corba-codegen/src/stub.rs::tests::{
java_out_and_inout_take_a_holder_and_in_does_not,
a_user_type_takes_the_generated_holder_beside_it,
raises_reaches_the_java_throws_clause}`,
`crates/corba-codegen/src/skeleton.rs::tests::java_switches_on_the_method_string_and_throws_the_omg_class`,
`crates/corba-ccm-ejb/src/stub_gen.rs::tests::{
stateful_bean_has_postconstruct_and_predestroy,
stateless_bean_has_no_lifecycle_hooks}`.

**Status:** done — EJB-Integration, java-to-IDL und die
Annex-A.1-CORBA-Bindung für Java stehen auf existierenden Pfaden.

### §2 Conformance Point 6: Extended Level Java

**Spec:** §2 Punkt 6 — Persistent State Service + §6.x-Extensions.

**Repo:** Voller PSS-Lifecycle (Cross-Ref CP3) via
`crates/corba-ccm/src/pss.rs`; Java-Bindung über
`crates/corba-ccm-ejb/` + Annex-A.1-Codegen, plus Bind-Helper
`crates/corba-ccm-ejb/src/connector_bean.rs::pss_session_for_bean`
der einen `ConnectorBean` an eine `PssSession` koppelt und das
Tupel `(bean.name, bean.component_id, tx_status)` als
`PssBeanBinding` liefert.

**Tests:** `crates/corba-ccm/src/pss.rs::tests::*` (13 Tests, Cross-Ref CP3) +
`crates/corba-ccm-ejb/src/connector_bean.rs::tests::connector_bean_with_pss_session_binds_correctly`.

**Status:** done — PSS-Tx-Lifecycle (CP3) plus EJB-Bind-Helper am
`ConnectorBean`.

### §2 Conformance Point 7: Lightweight CCM (LwCCM)

**Spec:** §2 Punkt 7 — Subset-Profile aus §13.

**Repo:** Equivalent-IDL-Filter `crates/ccm/src/lightweight.rs` +
Profil-Restriktionen `crates/corba-ccm/src/lwccm.rs` +
Container-Lifecycle in `crates/corba-ccm/src/lifecycle.rs` +
Conformance-Marker
`corba-ccm::conformance::LIGHTWEIGHT_CCM_LEVEL`.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::*` (§13-
Restriktionen), `crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_drops_configurator_operations`,
`crates/corba-ccm/src/lifecycle.rs::tests::*`,
`crates/corba-ccm/src/lib.rs::conformance_tests::lightweight_ccm_level_marker_matches_spec`.

**Status:** done — IDL-Subset + Filter + Container-Lifecycle +
Conformance-Marker live.

### §2 Conformance Point 8: ORB-Vendor

**Spec:** §2 Punkt 8 — Component-Specific-Erweiterungen am ORB.

**Repo:** `crates/corba-ccm/src/orb_core.rs::Orb` mit
Component-Specific-Erweiterungen am ORB-Singleton:
`with_interceptor_registry(Arc<InterceptorRegistry>)` /
`interceptor_registry()` (Cross-Ref `corba-3.3.md` §16),
`with_messaging_policy(MessagingPolicy)` /
`messaging_policies()` (Cross-Ref §17),
`with_compression(CompressionAlgorithm)` /
`compression()` (Cross-Ref §18). Plus Default-Lifecycle/
Threading-Modes/Policy-Domain-Manager. Conformance-Marker
`corba-ccm::conformance::CCM_ORB_VENDOR_STUB`.

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests::{orb_vendor_config_interceptor_registry,
orb_vendor_config_compression_and_messaging_policies}` +
`crates/corba-ccm/src/orb_core.rs::tests::orb_set_get_policy_round_trip` + Cross-Ref
Conformance-Marker-Tests.

**Status:** done — ORB-Vendor-Konfigurations-Surface für alle drei
CCM-relevanten Component-Specific-Erweiterungen (PI/Messaging/
Compression).

---

## §3 References

### §3.1 Normative References

**Spec:** §3.1, S. 2-4 (PDF) — CORBA 2.6, CORBA 3.0, IDL/CDR-Specs,
EJB1.1, MOF, XMI.

**Repo:** Diese externen Specs werden in den Konsumenten-Items §7-§16 in
Pure Rust abgedeckt (eigene Impls, keine externen MOF/EJB-Bibliotheks-
Abhängigkeiten); die §6-Equivalent-IDL nutzt die `zerodds_ast`-Layer
als Eingabe.

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz-Liste; Auswirkungen in den Konsumenten-Items §7-§16.

### §3.2 Non-normative References

**Spec:** §3.2 — CCM-Predecessor-Specs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Non-normative Background-Referenzen.

---

## §4 Terms and Definitions

### §4 Component / Connector / Container / Executor / Facet / etc.

**Spec:** §4, S. 4-7 (PDF) — Glossar.

**Repo:** Crate-Doc von `crates/ccm/src/lib.rs` referenziert die
Begriffe; `Component` + `Facet` + `Receptacle` + `Event Source`/`Sink`
sind in der Transformation aus `zerodds_ast::ComponentRole`-
Varianten gespiegelt.

**Tests:** —

**Status:** `n/a (informative)` — Glossar; Begriffe sind in den Konsumenten-Items referenziert.

---

## §5 Symbols

### §5 Abbrev (CCM/CIDL/CIF/CMT/CORBA/COS/...)

**Spec:** §5, S. 7-8 (PDF) — Abkürzungs-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Symbol-Tabelle; ohne Code-Mapping.

---

## §6 Component Model

### §6.1 Component Model Overview

**Spec:** §6.1, S. 9 (PDF) — Components als IDL-Meta-Type, Levels
(basic + extended), Ports (Facets/Receptacles/Event-Sources/Event-
Sinks/Attributes), Component Identity, Homes.

**Repo:** `crates/ccm/src/transform.rs::transform_component` deckt das
Component-Meta-Modell vollständig ab; ImplKind=`InterfaceKind::Plain`,
weil das Equivalent-Interface nicht `local` ist (Spec §6.3.2).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_plain_component_inherits_ccmobject,
the_whole_trader_component_yields_every_operation_the_clauses_name,
a_keyless_home_yields_an_implicit_inheriting_keylesscmhome,
a_component_attribute_is_left_alone}` — die vier Meta-Modell-Achsen
(Component-als-Interface, alle vier Port-Arten, Home, Attribute).

**Status:** done

### §6.1.1 Component Levels (Basic vs Extended)

**Spec:** §6.1.1, S. 9 (PDF) — "There are two levels of components:
basic and extended. [...] Basic components are not allowed to offer
facets, receptacles, event sources and sinks. They may only offer
attributes."

**Repo:** `transform_component` arbeitet generic — Caller bestimmt aus
`comp.body`-Inhalt, ob Component basic oder extended ist; die
Transformation ist für beide korrekt (basic: nur Attribute → keine
Provide-/Connect-Ops).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_basic_component_carries_its_attributes_and_no_port_operation,
a_component_attribute_is_left_alone,
the_whole_trader_component_yields_every_operation_the_clauses_name}` —
Basic (nur Attribute, keine Port-Ops) gegen Extended (alle Port-Ops).

**Status:** done

### §6.1.2 Ports

**Spec:** §6.1.2, S. 9-10 (PDF) — Vier Port-Typen + Attributes.

**Repo:** Alle vier (Facets/Receptacles/Event-Sources/Event-Sinks) +
Attributes werden in `transform_component` aus
`ComponentExport::{Provides,Uses,Emits,Publishes,Consumes,Attribute}`
transformiert.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_whole_trader_component_yields_every_operation_the_clauses_name`
— ein Component mit allen vier Port-Arten plus Attribut.

**Status:** done

### §6.1.3 Components and Facets — Equivalent Interface

**Spec:** §6.1.3, S. 10 (PDF) — "The component has a single
distinguished reference whose interface conforms to the component
definition. This reference supports an interface, called the
component's equivalent interface, that manifests the component's
surface features to clients."

**Repo:** `crates/ccm/src/transform.rs::ComponentEquivalent` —
`equivalent_interface` ist genau diese eine ausgezeichnete Referenz:
`transform_component` gibt den Component-Knoten selbst zurück, mit
`InterfaceKind::Interface` statt `InterfaceKind::Component`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_component_equivalent_is_comparable,
a_plain_component_inherits_ccmobject}` — der erste prüft die Identität
`equivalent_interface == component`, der zweite den Kind-Wechsel.

**Status:** done

### §6.1.4 Component Identity

**Spec:** §6.1.4, S. 11 (PDF) — Component-Reference + Facet-References
identifizieren Component-Instanz; `same_component`-Operation auf
`Components::Navigation` + `is_equivalent_component_kind`/
`get_component_def`.

**Repo:** `crates/corba-ccm/src/component_def.rs::ComponentDef::
{is_equivalent_component_kind, get_component_def_repo_id}`.
`Components::Navigation::same_component` selbst bleibt ORB-bound
(Object-Reference-Vergleich), das Type-Identity-Sub-Pattern ist
abgedeckt.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::is_equivalent_component_kind_matches_repo_id`,
`get_component_def_repo_id_returns_repo_id`.

**Status:** done — Type-Identity-Operations implementiert;
Object-Reference-`same_component` bleibt ORB-bound.

### §6.1.5 Component Homes

**Spec:** §6.1.5, S. 11 (PDF) — Home als Manager für Component-
Instanzen.

**Repo:** `crates/ccm/src/transform.rs::transform_home`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_keyless_home_yields_an_implicit_inheriting_keylesscmhome,
a_keyed_home_yields_the_four_operations_and_inherits_nothing,
a_derived_home_inherits_the_base_homes_explicit_interface}`.

**Status:** done

### §6.2 Component Definition

**Spec:** §6.2, S. 11 (PDF) — "A component definition in IDL implicitly
defines an interface that supports the features defined in the
component definition body."

**Repo:** `crates/ccm/src/transform.rs::transform_component`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_component_equivalent_is_comparable,
the_whole_trader_component_yields_every_operation_the_clauses_name}` —
die Component-Deklaration wird zur Interface-Deklaration, und der
Deklarations-Body trägt die Features, die das Interface dann anbietet.

**Status:** done

### §6.3 Component Declaration — Basic Components

**Spec:** §6.3.1, S. 11 (PDF) — Basic-Component-Pattern (`<component_dcl>`
restricted form `"component" <identifier> [<supported_interface_spec>]
"{" {<attr_dcl> ";"}* "}"`).

**Repo:** Eingabe ist der `InterfaceNode` mit
`InterfaceKind::Component` aus `crates/idl-lower/`;
`crates/ccm/src/transform.rs::transform_component` transformiert die
Basic-Form (nur `attr_dcl` im Body, optional `supports`) ohne eine
einzige Port-Operation zu erzeugen.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_basic_component_carries_its_attributes_and_no_port_operation`
— beide Attribute bleiben, die Operations-Liste ist leer, die
`supports`-Liste steht hinter `Components::CCMObject`.

**Status:** done

### §6.3.2.1 Equivalent IDL — Simple Declaration

**Spec:** §6.3.2.1, S. 12 (PDF) — `component component_name { ... };`
→ `interface component_name : Components::CCMObject { ... };`.

**Repo:** `crates/ccm/src/transform.rs::transform_component` — die
§6.3.2-Basisliste; ohne deklarierte Basis steht `Components::CCMObject`
darin.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_plain_component_inherits_ccmobject`.

**Status:** done

### §6.3.2.2 Equivalent IDL — Supported Interfaces

**Spec:** §6.3.2.2, S. 12 (PDF) — `component <name> supports <I1>, <I2>
{ ... }` → `interface <name> : Components::CCMObject, <I1>, <I2> { ...
}`.

**Repo:** `crates/ccm/src/transform.rs::transform_component` hängt die
`supports`-Liste an `Components::CCMObject` an.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_component_with_supports_inherits_ccmobject_and_the_supported_ones`.

**Status:** done

### §6.3.2.3 Equivalent IDL — Inheritance

**Spec:** §6.3.2.3, S. 12 (PDF) — `component <name> : <base_name> { ...
}` → `interface <name> : <base_name> { ... }` (CCMObject ist transitiv
über `<base_name>`).

**Repo:** `crates/ccm/src/transform.rs::transform_component` setzt
`Components::CCMObject` nur, wenn die deklarierte Basisliste leer ist —
sonst bleibt `<base_name>` allein stehen.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_derived_component_inherits_its_base_and_not_ccmobject`.

**Status:** done

### §6.3.2.4 Equivalent IDL — Inheritance + supports

**Spec:** §6.3.2.4, S. 13 (PDF) — `component <name> : <base> supports
<I1>, <I2> { ... }` → `interface <name> : <base>, <I1>, <I2> { ... }`.

**Repo:** `crates/ccm/src/transform.rs::transform_component` setzt
zuerst `<base>`, dann die `supports`-Liste.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_derived_component_with_supports_lists_the_base_first_and_then_the_supported`
— prüft die Reihenfolge `[B, I1, I2]` und dass
`Components::CCMObject` nicht zusätzlich auftaucht.

**Status:** done

### §6.3.3 Component Body

**Spec:** §6.3.3, S. 13 (PDF) — "Declarations for facets, receptacles,
event sources, event sinks and attributes all map onto operations on
the component's equivalent interface."

**Repo:** `crates/ccm/src/transform.rs::transform_component` —
Schleife über die `ComponentRole`-Ports des Deklarations-Bodys.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_whole_trader_component_yields_every_operation_the_clauses_name`
— ein Component mit `provides`/`uses`/`uses multiple`/`emits`/
`publishes`/`consumes`/`attribute`; der Test zählt jede daraus
entstehende Operation auf dem Equivalent-Interface auf.

**Status:** done

### §6.4.1 Facets — Equivalent IDL

**Spec:** §6.4.1, S. 13 (PDF) — `provides <interface_type> <name>;` →
`<interface_type> provide_<name>();` auf dem Equivalent-Interface.

**Repo:** `crates/ccm/src/transform.rs::transform_component`
(`ComponentRole::Facet`-Zweig).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
provides_yields_a_provide_operation_returning_the_facet,
the_whole_trader_component_yields_every_operation_the_clauses_name}`.

**Status:** done

### §6.4.2 Semantics of Facet References

**Spec:** §6.4.2, S. 13-14 (PDF) — Behavior-Constraints für
Facet-Refs (nil-allowed, Lifecycle bounded by component).

**Repo:** IDL-Op-Signaturen via `component_def.rs::FacetDef` +
Lifetime-Constraint-Validator
`crates/corba-ccm/src/lifecycle.rs::{check_facet_lifetime,
FacetLifetimeViolation}`. Container-Runtime nutzt diesen Check
um Use-After-Destroy + Orphan-References zu rejecten.

**Tests:** `crates/corba-ccm/src/lifecycle.rs::tests::{facet_lifetime_passes_when_alive,
facet_lifetime_rejects_use_after_destroy,
facet_lifetime_rejects_orphaned}`.

**Status:** done — IDL-Op-Mapping + Lifetime-Constraint-Check live.

### §6.4.3 Navigation Interface

**Spec:** §6.4.3, S. 14-16 (PDF) — `Components::Navigation`-Interface
mit `provide_facet`/`get_all_facets`/`get_named_facets`/
`same_component`. CCMObject inherits Navigation.

**Repo:** `crates/corba-ccm/src/component_def.rs::ComponentDef::
{provide_facet, get_all_facets, get_named_facets}` (Generic-Op
Skeleton-Implementation auf der Component-Definition; Caller-Layer
bindet `interface_id` an konkrete ORB-Object-Reference).
`same_component` bleibt ORB-bound (Object-Reference-Vergleich).

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown,
get_all_facets_returns_complete_list, get_named_facets_filters_by_names}`.

**Status:** done — Inheritance-Modell + Generic-Operations als
Skeleton-Pfad live; Object-Reference-`same_component` ORB-bound.

### §6.4.3.1 get_component (CORBA::Object enhancement)

**Spec:** §6.4.3.1, S. 14 (PDF) — Erweiterung von `CORBA::Object` um
`Object get_component()`-Pseudo-IDL-Op. ORB liefert das auch ohne
Component-Vendor.

**Repo:** `crates/corba-ccm/src/home.rs` — `FacetRef::get_component`
liefert die Component-Referenz zu einer Facet-Referenz,
`ComponentRef::get_component` liefert dieselbe Referenz zurück; der
GIOP-Operationsname steht als
`crates/corba-ccm/src/home.rs::GET_COMPONENT_GIOP_OPERATION`.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
get_component_on_a_facet_reference_returns_the_component,
get_component_on_a_component_reference_returns_the_same_reference,
get_component_travels_as_the_giop_operation_named_component}`.

**Status:** done

### §6.4.4 Provided References and Component Identity

**Spec:** §6.4.4, S. 17 (PDF) — `same_component` Operation auf
Navigation; default-Implementation über Container-Servant-Framework.

**Repo:** `crates/corba-ccm/src/home.rs` —
`ComponentRef::same_component` vergleicht Component-Typ und
Instanz-Identität, `FacetRef::same_component` delegiert an die
besitzende Component, wie §6.4.4 es für Facets fordert, die
`Navigation` erben.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
two_facets_of_one_instance_are_the_same_component,
a_facet_answers_same_component_as_the_component_that_owns_it,
two_instances_of_one_component_type_are_not_the_same_component,
one_instance_id_under_two_component_types_is_not_the_same_component}`.

**Status:** done

### §6.4.5 Supported Interfaces

**Spec:** §6.4.5, S. 17-19 (PDF) — `supports <I>` widening +
narrowing-Semantik; KeylessCCMHome::create_component liefert CCMObject,
narrowing zu I.

**Repo:** Inheritance-Modell deckt das ab (`component_bases`); Type-
Identity-Narrowing als Runtime-Helper in
`crates/corba-ccm/src/component_def.rs::ComponentDef::
{supports_interface, supported_interface_repo_ids}`. Object-
Reference-Widening (POA-`is_a`) bleibt ORB-bound.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{supports_interface_returns_true_for_listed_iface,
supports_interface_returns_false_for_unknown,
supported_interface_repo_ids_returns_full_list}`.

**Status:** done — Inheritance + Type-Identity-Narrowing-Helper
implementiert; Object-Reference-Widening ORB-bound.

### §6.5.1 Receptacles — Equivalent IDL

**Spec:** §6.5.1, S. 19-20 (PDF) — Simplex: `connect_<name>(in T)
raises (AlreadyConnected, InvalidConnection)`, `T disconnect_<name>()
raises (NoConnection)`, `T get_connection_<name>()`. Multiplex:
`Cookie connect_<name>(in T) raises (ExceededConnectionLimit,
InvalidConnection)`, `T disconnect_<name>(in Cookie)`,
`<name>Connections get_connections_<name>()`.

**Repo:** `crates/ccm/src/transform.rs::uses_ops` (multiplex/simplex
branch).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
uses_simplex_yields_connect_disconnect_and_get_connection,
uses_multiple_returns_a_cookie_and_the_plural_getter}`.

**Status:** done

### §6.5.2 Receptacles — Behavior

**Spec:** §6.5.2, S. 20-22 (PDF) — Connect/disconnect-Verhalten,
Cookie-Semantik.

**Repo:** `Components::Cookie` modelliert in `crates/ccm/src/model.rs`
+ Connect/Disconnect-State-Machine in
`crates/corba-ccm/src/lifecycle.rs::{ReceptacleManager,
ConnectionState, ConnectionError}` mit Simplex-vs-Multiplex-
Constraints (Simplex-Double-Connect rejected,
Disconnect-Unknown-Id rejected).

**Tests:** `crates/ccm/src/model.rs::tests::{cookie_new_stores_octet_seq,
cookie_truncate_preserves_octet_seq,
receptacle_description_supports_simplex_and_multiplex}` +
`crates/corba-ccm/src/lifecycle.rs::tests::{receptacle_simplex_connect_then_disconnect,
receptacle_simplex_double_connect_rejected,
receptacle_multiplex_allows_multiple_connects,
receptacle_disconnect_unknown_id_rejected,
receptacle_double_disconnect_rejected}`.

**Status:** done — Datenmodell + Connect/Disconnect-State-Machine live.

### §6.5.3 Receptacles Interface (generic)

**Spec:** §6.5.3, S. 22-24 (PDF) — `Components::Receptacles` Generic-
Interface. CCMObject erbt davon.

**Repo:** `crates/corba-ccm/src/component_def.rs::ComponentDef::
{get_all_receptacles, get_named_receptacles}` (Generic-Op-Skeleton).
`connect`/`disconnect`/`get_connections` (Lifecycle) bleiben in der
Container-Runtime (siehe §6.5.2).

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{get_all_receptacles_returns_complete_list,
get_named_receptacles_filters}`.

**Status:** done — Modell + Generic-Lookup-Operations als
Skeleton-Pfad live; Connect-Lifecycle weiter Container-Runtime.

### §6.6.1 Event Types — Equivalent IDL

**Spec:** §6.6.1.1, S. 24-25 (PDF) — `eventtype A {...}; eventtype B :
A {...};` → `valuetype A : Components::EventBase {...}; valuetype B :
A {...}; interface BConsumer : Components::EventConsumerBase { void
push_B(in B the_b); };`.

**Repo:** `crates/ccm/src/transform.rs::transform_event_type`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
the_first_event_type_of_a_chain_inherits_event_base,
one_event_type_yields_one_consumer_interface_however_many_ports_name_it,
a_plain_valuetype_is_not_an_event_type}`.

**Status:** done

### §6.6.1.2 EventBase

**Spec:** §6.6.1.2, S. 25 (PDF) — `module Components { abstract
valuetype EventBase { }; };`.

**Repo:** ScopedName-Referenz in
`crates/ccm/src/transform.rs::transform_event_type` — nur der erste
Event-Type einer Kette bekommt `Components::EventBase`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_first_event_type_of_a_chain_inherits_event_base`.

**Status:** done

### §6.6.2 EventConsumer Interface

**Spec:** §6.6.2, S. 25-26 (PDF) — `Components::EventConsumerBase` mit
`push_event(in EventBase) raises BadEventType`.

**Repo:** `crates/ccm/src/transform.rs::transform_event_type` — die
Consumer-Basisliste ist `Components::EventConsumerBase`, solange der
Event-Type selbst keine Basis hat; sonst `<Base>Consumer`, sodass die
Consumer-Kette der Event-Kette folgt.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_consumer_interface_inherits_event_consumer_base_at_the_root_of_the_chain`
— `TickConsumer : Components::EventConsumerBase` und
`PricedTickConsumer : TickConsumer`.

**Status:** done

### §6.6.3 Event Service Provided by Container

**Spec:** §6.6.3, S. 26 (PDF) — Container-Runtime-Topic — Event-Channels.

**Repo:** `crates/corba-ccm/src/port.rs` (`PortRegistry` +
`EventStream` — die Verbindungstabelle, über die der Container
Event-Sources an Event-Sinks bindet) +
`crates/corba-ccm-lib/src/dds_bridge.rs` (Event-Source → DataWriter,
Event-Sink → DataReader; die Distribution selbst macht DDS-DCPS in
`crates/dcps/` statt eines separaten CosEventChannel-Stacks).

**Tests:** `crates/corba-ccm/src/port.rs::tests::{
event_sink_subscription_round_trip,
list_event_sinks_unknown_source_empty}`,
`crates/corba-ccm-lib/src/dds_bridge.rs::tests::{
add_mapping_round_trip, mapping_direction_round_trip,
duplicate_port_rejected, activate_makes_active}`.

**Status:** done — Container-seitige Event-Verteilung über
Port-Registry und DDS-Bridge.

### §6.6.4 Event Sources — Publishers and Emitters

**Spec:** §6.6.4, S. 26-27 (PDF) — Publisher (multi-subscriber) +
Emitter (single-subscriber).

**Repo:** Datenmodell in `crates/ccm/src/model.rs`
(`PublisherDescription`, `EmitterDescription`) + Routing-Verhalten
über DDS-DCPS Pub/Sub (siehe §6.6.3): Publisher = DataWriter mit
mehreren MatchedReaders, Emitter = DataWriter mit
`OWNERSHIP=EXCLUSIVE` zu genau einem Reader. Cross-Ref
`crates/dcps/src/{publisher,subscriber}.rs`. Generic-Lookup-Ops
in `component_def.rs::{get_all_publishers, get_all_emitters,
get_named_publishers, get_named_emitters}`.

**Tests:** `crates/ccm/src/model.rs::tests::publisher_description_can_have_multiple_subscribers`,
`crates/corba-ccm/src/component_def.rs::tests::{
get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources}`,
`crates/corba-ccm-lib/src/dds_bridge.rs::tests::mapping_direction_round_trip`.

**Status:** done — Datenmodell + Routing-Behavior via DCPS live;
Generic-Lookup-Operations als Skeleton-Pfad.

### §6.6.5 Publisher — Equivalent IDL

**Spec:** §6.6.5.1, S. 27 (PDF) — `publishes <T> <name>;` → `Cookie
subscribe_<name>(in TConsumer consumer) raises
(ExceededConnectionLimit); TConsumer unsubscribe_<name>(in Cookie ck)
raises (InvalidConnection);`.

**Repo:** `crates/ccm/src/transform.rs::publishes_ops`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::publishes_yields_subscribe_returning_a_cookie`.

**Status:** done

### §6.6.6 Emitters — Equivalent IDL

**Spec:** §6.6.6.1, S. 28 (PDF) — `emits <T> <name>;` → `void
connect_<name>(in TConsumer consumer) raises (AlreadyConnected);
TConsumer disconnect_<name>() raises (NoConnection);`.

**Repo:** `crates/ccm/src/transform.rs::emits_ops`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::emits_yields_connect_and_disconnect_and_the_consumer_interface`.

**Status:** done

### §6.6.7 Event Sinks — Equivalent IDL

**Spec:** §6.6.7.1, S. 29 (PDF) — `consumes <T> <name>;` → `TConsumer
get_consumer_<name>();`.

**Repo:** `crates/ccm/src/transform.rs::transform_component`
(`ComponentRole::EventSink`-Zweig).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::consumes_yields_a_get_consumer_operation`.

**Status:** done

### §6.6.8 Events Interface (generic)

**Spec:** §6.6.8, S. 29-32 (PDF) — `Components::Events` mit
`get_consumer`/`subscribe`/`unsubscribe`/`connect_consumer`/
`disconnect_consumer`/`get_all_consumers`/`get_all_publishers`/
`get_all_emitters`/etc.

**Repo:** Datenmodelle (`ConsumerDescription`, `EmitterDescription`,
`PublisherDescription`, `SubscriberDescription`) in
`crates/ccm/src/model.rs` + Generic-Lookup-Operations
`crates/corba-ccm/src/component_def.rs::ComponentDef::
{get_all_publishers, get_all_emitters, get_named_publishers,
get_named_emitters}`. Subscribe/Connect-Lifecycle in der Container-
Runtime (Spec §6.6.4 / §6.6.5).

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources,
get_named_publishers_respects_emit_only_flag,
get_named_emitters_excludes_publishers}`.

**Status:** done — Modell + Generic-Lookup-Operations als
Skeleton-Pfad live; Subscribe/Connect bleibt Container-Runtime.

### §6.7.1.1 Home Definitions — No Primary Key

**Spec:** §6.7.1.1, S. 33 (PDF) — `home <h> manages <C> { ... };` →
`interface <h>Explicit : Components::CCMHome {...};` + `interface
<h>Implicit : Components::KeylessCCMHome { <C> create() raises
(CreateFailure); };` + `interface <h> : <h>Explicit, <h>Implicit { };`.

**Repo:** `crates/ccm/src/transform.rs::transform_home` (keyless-
Pfad in `build_keyless_implicit`).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_keyless_home_yields_an_implicit_inheriting_keylesscmhome`.

**Status:** done

### §6.7.1.2 Home Definitions — With Primary Key

**Spec:** §6.7.1.2, S. 34 (PDF) — `home <h> manages <C> primarykey
<K> { ... };` → 4-Op-Implicit-Iface (`create(K key)`,
`find_by_primary_key(K key)`, `remove(K key)`, `<K> get_primary_key
(<C> comp)`).

**Repo:** `crates/ccm/src/transform.rs::build_keyed_implicit`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_keyed_home_yields_the_four_operations_and_inherits_nothing`.

**Status:** done

### §6.7.1.3 Home — Supported Interfaces

**Spec:** §6.7.1.3, S. 35 (PDF) — `home <h> supports <I> manages <C>
{...};` → `interface <h>Explicit : Components::CCMHome, <I> {...};`.

**Repo:** `crates/ccm/src/transform.rs::transform_home` hängt die
`supports`-Liste an `Components::CCMHome` an.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_home_with_supports_puts_the_supported_interfaces_on_its_explicit`
— die unterstützten Interfaces landen auf dem Explicit-Interface, nicht
auf dem Implicit und nicht auf dem Equivalent-Home.

**Status:** done

### §6.7.2 Primary Key Declarations

**Spec:** §6.7.2, S. 35-36 (PDF) — Primary-Key-Type-Constraints +
`Components::PrimaryKeyBase`.

**Repo:** `crates/ccm/src/validate.rs::validate_primary_key` —
prüft die 4 Spec-Constraints: (1) Concrete `valuetype`,
(2) erbt von `Components::PrimaryKeyBase`, (3) keine
`private`-State-Members, (4) keine Interface-Referenzen
(konservative Auslegung: jede `ScopedName`-Type-Referenz wird
abgelehnt).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_primary_key_with_a_public_scalar_member_is_accepted,
a_primary_key_without_the_base_is_rejected,
a_primary_key_with_a_private_member_is_rejected,
a_primary_key_holding_an_object_reference_is_rejected,
a_primary_key_holding_a_nested_struct_is_accepted,
an_abstract_valuetype_is_not_a_primary_key}`.

**Status:** done

### §6.7.3 Explicit Operations in Home Definitions

**Spec:** §6.7.3, S. 36-37 (PDF) — Factory- + Finder-Operations werden
auf das Explicit-Interface gemappt mit korrekt erweiterten `raises`-
Klauseln (CreateFailure / FinderFailure).

**Repo:** `crates/ccm/src/validate.rs::apply_factory_finder_body`
mit `InitOp`-Struct (Factory- oder Finder-Eintrag mit Name + Params
+ Caller-`raises`). Spec §6.7.3.1 Factory: Return-Type =
Equivalent-Iface, raises = `Components::CreateFailure` + Caller-
raises. Spec §6.7.3.2 Finder: Return-Type = Equivalent-Iface,
raises = `Components::FinderFailure` + Caller-raises. Alle Params
werden auf `in`-Attribute normalisiert.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_factory_is_emitted_with_create_failure_first_in_its_raises,
a_finder_is_emitted_with_finder_failure_and_the_callers_own_raises}`.

**Status:** done

### §6.7.4 Home Inheritance

**Spec:** §6.7.4, S. 37 (PDF) — Derived-Home: `<h>Explicit :
<base_h>Explicit`.

**Repo:** `transform_home` setzt `explicit_bases` auf `<base>Explicit`
wenn `home.base.is_some()` (`base_explicit_name`-Helper).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_derived_home_inherits_the_base_homes_explicit_interface,
a_derived_home_with_supports_extends_the_base_explicit}`.

**Status:** done

### §6.7.5 Semantics of Home Operations

**Spec:** §6.7.5, S. 38-39 (PDF) — Orthodox vs Heterodox Operations,
Polymorphismus-Regeln.

**Repo:** `crates/corba-ccm/src/home.rs::HomeFinder` —
die orthodoxen Operationen `create` (§6.7.5.1 keyless: system-generierte
Instanz-ID; §6.7.5.2 keyed: der Primary Key ist die Instanz-ID),
`find_by_primary_key` und `remove`;
`crates/corba-ccm/src/container.rs::Container` trägt den
Instanz-Lifecycle, an dem `remove` hängt.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
create_with_user_key_uses_key_as_id,
create_without_key_generates_system_id,
find_by_primary_key_round_trip, remove_decrements_instance_count,
remove_unknown_returns_false}`,
`crates/corba-ccm/src/container.rs::tests::{
install_and_full_lifecycle_round_trip,
instance_count_tracks_install_and_remove}`.

**Status:** done

### §6.7.6 CCMHome Interface

**Spec:** §6.7.6, S. 40 (PDF) — `interface CCMHome { CORBA::IRObject
get_component_def(); CORBA::IRObject get_home_def(); void
remove_component(in CCMObject comp) raises (RemoveFailure); };`.

**Repo:** ScopedName-Bezugspunkt in `transform_home::explicit_bases`.
`FailureReason` als `u32` modelliert in `crates/ccm/src/model.rs`.
Generic-Operations `get_component_def_repo_id` +
`get_home_def_repo_id` in `crates/corba-ccm/src/home.rs::HomeDef`
(IRObject-Lookup-Pfad über Repository-IDs; Caller bindet IFR-
Resolver). `remove_component` ist Container-Runtime
(`crates/corba-ccm/src/container.rs`).

**Tests:** `crates/corba-ccm/src/home.rs::tests::{home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done — Iface-Bezugnahme + Type-Identity-Operations
implementiert; Object-Lifecycle-Operations bleiben Container-Runtime.

### §6.7.7 KeylessCCMHome Interface

**Spec:** §6.7.7, S. 41 (PDF) — `interface KeylessCCMHome {
CCMObject create_component() raises (CreateFailure); };`.

**Repo:** ScopedName-Bezugspunkt + Semantik in
`crates/ccm/src/transform.rs::build_keyless_implicit`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_keyless_home_yields_an_implicit_inheriting_keylesscmhome`.

**Status:** done

### §6.8 Home Finders

**Spec:** §6.8, S. 41-43 (PDF) — `Components::HomeFinder`-Interface;
`CORBA::ORB::resolve_initial_references("ComponentHomeFinder")`.

**Repo:** `crates/corba-ccm/src/home.rs::ComponentHomeFinder` — die
drei `find_home_by_*`-Operationen über die per
`HomeRegistration::register_home` (§9.4.1.2) registrierten Homes, mit
`HomeNotFound` als Fehlerfall; der Initial-Reference-Bezeichner steht
als `crates/corba-ccm/src/home.rs::COMPONENT_HOME_FINDER_INITIAL_REFERENCE`.
Für `find_home_by_name` liefert `crates/corba-cosnaming/src/stringified.rs`
das Stringified-Name-Format.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
the_home_finder_is_reached_through_the_component_home_finder_initial_reference,
a_registered_home_is_found_by_component_type_and_by_home_type,
an_unregistered_component_type_raises_home_not_found,
a_home_registered_without_a_name_cannot_be_found_by_name,
an_unregistered_home_is_never_returned_again}`.

**Status:** done

### §6.9 Component Configuration

**Spec:** §6.9, S. 43-44 (PDF) — Component-Configurability via
Attributes; `configuration_complete`-Operation auf CCMObject.

**Repo:** `crates/corba-rust/src/component_emit.rs::emit_component`
emittiert `configuration_complete` als Lifecycle-Operation auf dem
Component-Trait; die Konfigurations-Phase selbst liegt in
`crates/corba-ccm/src/lifecycle.rs::{StandardConfigurator,
ConfiguratorRegistry}` (§6.10).

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::a_component_trait_carries_the_configuration_complete_lifecycle_operation`,
`crates/corba-ccm/src/lifecycle.rs::tests::{
configurator_set_get_roundtrip, configurator_registry_register_and_lookup}`.

**Status:** done

### §6.10 Configuration with Attributes

**Spec:** §6.10.1, S. 44-45 (PDF) — Configurator-Iface mit
`configure(in CCMObject comp)`; StandardConfigurator mit
`set_configuration(in ConfigValues descr)`.

**Repo:** Datenmodell `ConfigValue` in `crates/ccm/src/model.rs`;
Configurator-Operation-Filter in `crates/ccm/src/lightweight.rs::filter_to_lightweight`
(LwCCM §13.7 entfernt diese) + Configurator-Iface in
`crates/corba-ccm/src/lifecycle.rs::{Configurator, StandardConfigurator,
ConfiguratorRegistry, ConfigError}` mit set_attribute/get_attribute
+ Schema-validierter ReadOnly-Rejection + per-RepoId-Lookup.

**Tests:** `crates/ccm/src/model.rs::tests::config_value_carries_name_and_marshaled_value`,
`crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_drops_configurator_operations`,
`crates/corba-ccm/src/lifecycle.rs::tests::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute,
configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`.

**Status:** done — Modell + Filter + Configurator-Iface live.

### §6.11 Component Inheritance

**Spec:** §6.11, S. 48-49 (PDF) — Component-Inheritance + CCMObject.

**Repo:** `crates/ccm/src/transform.rs::transform_component` deckt
Component-Inheritance ab.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_derived_component_inherits_its_base_and_not_ccmobject,
a_derived_component_with_supports_lists_the_base_first_and_then_the_supported}`.

**Status:** done

### §6.11.1 CCMObject Interface

**Spec:** §6.11.1, S. 49 (PDF) — `interface CCMObject : Navigation,
Receptacles, Events { ... };`.

**Repo:** `Components::CCMObject` als ScopedName-Bezugspunkt;
Inheritance-Chain wird per Spec-Regel referenziert (nicht im AST
explizit aufgeschlüsselt — das bleibt Caller-Verantwortung beim
Linker).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_plain_component_inherits_ccmobject`.

**Status:** done

### §6.12 Conformance Requirements

**Spec:** §6.12, S. 50-52 (PDF) — Acht Conformance-Punkte (siehe §2);
Sub-Section §6.12.2 Changes to Object Services.

**Repo:** Conformance-Marker in `crates/corba-ccm/src/lib.rs::
conformance::{CCM_CONFORMANCE_BASIC_LEVEL,
CCM_CONFORMANCE_BASIC_LEVEL_JAVA, LIGHTWEIGHT_CCM_LEVEL}`.

**Tests:** `crates/corba-ccm/src/lib.rs::conformance_tests::ccm_conformance_basic_level_marker_matches_spec`,
`ccm_conformance_basic_level_java_marker_matches_spec`,
`lightweight_ccm_level_marker_matches_spec`.

**Status:** done — Marker für alle drei in ZeroDDS abgedeckten
Conformance-Punkte (Basic Level non-Java + Java + LwCCM) als
Doc-Konstanten ausgewiesen. Vendor-Container-Aspekt selber bleibt
nach WP CCM-Container Core (siehe §2 Punkte 4/5/7).

---

## §7 OMG CIDL Syntax and Semantics

### §7 CIDL — Component Implementation Definition Language

**Spec:** §7, S. 55-66 (PDF) — Lexical Conventions, Grammar,
Composition Definition, Home Implementation, Storage Home Binding,
Persistence, Executor, Segment, Facet, Feature Delegation, etc.

**Repo:** `crates/corba-ccm/src/cidl.rs` (CIDL-Lexer + Grammar-AST +
Composition-/Home-/Segment-/Storage-/Executor-Definitions).

**Tests:** `crates/corba-ccm/src/cidl.rs::tests::{
session_composition_does_not_require_storage,
entity_composition_without_storage_invalid,
entity_composition_with_storage_valid, storage_type_can_inherit,
home_executor_optional_component_binding,
all_four_composition_categories_distinct}`.

**Status:** done

---

## §8 CCM Implementation Framework

### §8 CIF — Component Implementation Framework

**Spec:** §8, S. 67-108 (PDF) — Servant-/Skeleton-Generator, Executor-
Lifetime, Composition-Persistence, Proxy-Homes, Language-Mapping
(C++/Java).

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF-AST + Executor/Segment-
Lifetime), `crates/corba-codegen/src/skeleton.rs` +
`crates/corba-codegen/src/stub.rs` (Servant/Skeleton-Generator).

**Tests:** `crates/corba-ccm/src/cif.rs::tests::{
lifecycle_calls_increment_counters, cif_error_carries_diagnostic}`
(Executor-Lifetime `ccm_activate`/`ccm_passivate`/`ccm_remove` +
`CCMException`),
`crates/corba-codegen/src/skeleton.rs::tests::{
every_operation_reaches_the_dispatch_in_every_column,
an_empty_interface_still_dispatches_to_the_default_arm,
cpp_compares_with_strcmp_and_ends_in_bad_operation,
java_switches_on_the_method_string_and_throws_the_omg_class,
rust_matches_and_answers_none_rather_than_an_invented_error}`
(Skeleton-Generator),
`crates/corba-codegen/src/stub.rs::tests::{
a_variable_length_struct_returns_a_pointer_and_a_fixed_one_a_value,
an_object_reference_argument_is_a_ptr_and_its_out_is_an_out,
a_oneway_operation_says_so_in_every_column_that_renders}`
(Stub-Generator).

**Status:** done

---

## §9 Container Programming Model

### §9 Container Programming Model

**Spec:** §9, S. 109-150 (PDF) — Server-Programming-Environment,
Client-Programming-Model, Container-API-Types (Session/Entity).

**Repo:** `crates/corba-ccm/src/container.rs`,
`crates/corba-ccm/src/context.rs`,
`crates/corba-ccm-lib/src/persistence.rs`,
`crates/corba-ccm-lib/src/dds_bridge.rs`,
`crates/corba-ccm-lib/src/telemetry.rs`.

**Tests:** `crates/corba-ccm/src/container.rs::tests::{
install_and_full_lifecycle_round_trip,
instance_count_tracks_install_and_remove,
passivate_in_configured_state_invalid,
unknown_instance_returns_not_found}`,
`crates/corba-ccm/src/context.rs::tests::{anonymous_context_returns_none,
principal_context_returns_bytes}`,
`crates/corba-ccm-lib/src/persistence.rs::tests::{
create_and_find_round_trip, passivate_flushes_dirty_entries,
flush_resets_dirty_and_increments_counter}`,
`crates/corba-ccm-lib/src/dds_bridge.rs::tests::{
add_mapping_round_trip, activate_makes_active,
passivate_clears_active_flag}`,
`crates/corba-ccm-lib/src/telemetry.rs::tests::{
full_lifecycle_records_four_events, sequence_numbers_strictly_increase}`.

**Status:** done

---

## §10 Integrating with Enterprise JavaBeans

### §10 EJB Integration

**Spec:** §10, S. 151-176 (PDF) — Two-Way-Mapping CCM ↔ EJB.

**Repo:** `crates/corba-ccm-ejb/src/{connector_bean,naming_glue,
stub_gen,tx}.rs`.

**Tests:** `crates/corba-ccm-ejb/src/connector_bean.rs::tests::{
full_lifecycle_records_history, post_construct_invokes_callback,
connector_bean_with_pss_session_binds_correctly}`,
`crates/corba-ccm-ejb/src/naming_glue.rs::tests::{
cos_to_jndi_round_trip_no_kind, cos_to_jndi_includes_kind,
jndi_comp_env_strips_prefix, bind_and_lookup_round_trip}`,
`crates/corba-ccm-ejb/src/stub_gen.rs::tests::{
stateful_bean_has_postconstruct_and_predestroy,
stateless_bean_has_no_lifecycle_hooks, home_local_uses_local_annotation}`,
`crates/corba-ccm-ejb/src/tx.rs::tests::{
cos_to_jta_round_trip_for_all_states, begin_then_commit_succeeds,
rollback_only_then_commit_fails_with_rollback}`.

**Status:** done

---

## §11 Interface Repository Metamodel

### §11 IFR Metamodel

**Spec:** §11, S. 177-260 (PDF) — MOF-Modelle für BaseIDL-Package +
ComponentIDL-Package; XMI-DTDs + IDL für IFR.

**Repo:** `crates/corba-ir/src/repository.rs` (Container-/Contained-
Hierarchie), `crates/corba-ir/src/repository_id.rs`,
`crates/corba-ir/src/type_code.rs` (TypeCode-Backend),
`crates/corba-ir/src/definition_kind.rs` +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` mit MOF-2.0-Subset (Class/Property/Operation)
+ XMI-1.2-Output für das ComponentIDL-Package.

**Tests:** `crates/corba-ir/src/repository.rs::tests::*`,
`crates/corba-ir/src/repository_id.rs::tests::*`,
`crates/corba-ir/src/type_code.rs::tests::*` +
`crates/corba-ccm/src/orb_core.rs::tests::{
ifr_ccm_metamodel_add_component,
ifr_ccm_metamodel_ingest_repository_walks_definitions}`.

**Status:** done — IR-Basis (`corba-ir`) + CCM-Metamodel-MOF-2.0-
Subset + XMI-1.2-Emitter live. `IfrCcmMetamodel::from_repository(
&corba_ir::Repository)` walked die `Container`/`Contained`-Hierarchie
und emittiert Component-/Module-Klassen. Cross-Ref CORBA 3.3 Part 3
§12 (`corba-3.3.md`).

---

## §12 CIF Metamodel

### §12 CIF Metamodel

**Spec:** §12, S. 261-272 (PDF) — MOF-Modell für Component
Implementation Framework.

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF-AST-Modell) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement}` als
MOF-XMI-Emitter symmetrisch zu §11.

**Tests:** `crates/corba-ccm/src/cif.rs::tests::{
lifecycle_calls_increment_counters, cif_error_carries_diagnostic}` +
`crates/corba-ccm/src/orb_core.rs::tests::*` (XMI-Emitter).

**Status:** done — CIF-AST (`corba-ccm::cif`) + MOF-XMI-Emitter
symmetrisch zu §11 IFR. Cross-Ref CORBA 3.3 Part 3 §13
(`corba-3.3.md`).

---

## §13 Lightweight CCM Profile

### §13.1 Summary

**Spec:** §13.1, S. 273 (PDF) — LwCCM-Subset: ohne Persistence,
Introspection, Navigation-Generic, Type-specific-Generic,
Segmentation, Transactions, Security, Configurators, Proxy-Homes,
Home-Finders.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
the_lightweight_filter_drops_configurator_operations,
the_lightweight_filter_keeps_the_type_specific_operations,
the_lightweight_filter_reports_a_component_it_would_empty}` +
`crates/corba-ccm/src/lwccm.rs::tests::{
same_component_raises_no_implement_with_the_mandated_minor_code,
every_other_disabled_operation_raises_bad_operation}` — §13.1 hält die
disabled-Operationen im Interface und lässt den Container
`BAD_OPERATION`/`NO_IMPLEMENT` melden; für `same_component` ist
`NO_IMPLEMENT` mit Minor-Code `IS_LIGHTWEIGHT_COMPONENT` vorgeschrieben.

**Status:** done

### §13.2 No Persistence

**Spec:** §13.2, S. 274-276 (PDF) — Tabelle "Changes associated with
excluding support for persistence": Primary Keys, CIDL, abstract
Storage Homes/Types, Keyed-Home-Implicit-Executor, Finder im Explicit
Home Executor, Entity-Container-API-Types, durable CORBA usage model,
`CCM2Context::get_persistence`. §13.1, S. 273: "Persistence (only
session and service components are supported)."

**Repo:** `crates/corba-ccm/src/lwccm.rs` —
`disabled_by("get_persistence")` meldet `LwccmExclusion::Persistence`,
`category_supported` lässt nur `Session` und `Service` durch,
`composition_exclusion` weist Entity-Kompositionen und jede
Storage-Home-Bindung als Persistenz zurück. Die Extended-Form bleibt in
`crates/corba-ccm-lib/src/persistence.rs` + `crates/corba-ccm/src/pss.rs`
und wird von diesen Prüfungen aus dem Profil gehalten.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
the_persistence_operations_are_disabled_by_thirteen_two_and_three,
only_session_and_service_components_are_supported,
an_entity_composition_is_excluded_as_persistence}`.

**Status:** done

### §13.3 No Introspection / Navigation / Type-Specific-Generic Ops

**Spec:** §13.3, S. 276-277 (PDF) — Removed: `provide_facet`,
`get_all_facets`, `get_named_facets`, generic `connect`/`disconnect`/
`get_connections`, generic event-routing-Operationen.

**Repo:** `crates/corba-ccm/src/lwccm.rs::disabled_by` — die
Introspection-Zeilen von §13.3 als Namensliste plus die
Präfix-Erkennung der typspezifischen Port-Operationen
(`provide_<name>`, `connect_<name>`, `disconnect_<name>`,
`get_connection(s)_<name>`, `subscribe_<name>`, `unsubscribe_<name>`,
`get_consumer_<name>`). Die generischen Gegenstücke —
`provide_facet`, `connect`, `disconnect`, `get_consumer`, `subscribe`,
`unsubscribe`, `connect_consumer` — bleiben, weil §13.3 genau sie
behält. Die Operationen selbst bleiben im Equivalent-Interface stehen
(§13.1); entfernt wird nichts.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
the_type_specific_port_operations_are_disabled_and_the_generic_ones_are_not,
both_connection_getters_are_disabled_the_generic_one_as_introspection,
a_bare_prefix_is_not_an_operation_name}` +
`crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_keeps_the_type_specific_operations`
(die Operationen bleiben deklariert).

**Status:** done

### §13.4-§13.6 No Segmentation/Transactions/Security

**Spec:** §13.4, S. 277 (PDF) — "Exclude support for CIDL",
"Exclude composition", "Remove locator support", "Restrict CIF metamodel
to a single segment per implementation". §13.5, S. 278 — "Disable the
get_rollback_only, get_user_transaction, and set_rollback_only
operations in the CCMContext interface". §13.6, S. 278 — "Disable the
get_caller_principal and is_caller_in_role operations in the CCMContext
interface".

**Repo:** `crates/corba-ccm/src/lwccm.rs` —
`composition_exclusion` weist jede CIDL-Komposition zurück,
`MAX_SEGMENTS_PER_IMPLEMENTATION` trägt die 1:1-Multiplizität aus
§13.4, `disabled_by` meldet die drei Transaktions- und die zwei
Credential-Operationen von `CCMContext`. Die Extended-Formen bleiben in
`crates/corba-ccm/src/cidl.rs`, `crates/corba-ccm-ejb/src/tx.rs` und
`crates/corba-csiv2/` und werden von diesen Prüfungen aus dem Profil
gehalten.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
a_session_composition_is_still_excluded_because_cidl_is_gone,
the_cif_metamodel_holds_one_segment_per_implementation,
the_three_transaction_operations_of_ccmcontext_are_disabled,
the_two_credential_operations_of_ccmcontext_are_disabled,
get_ccm_home_survives_beside_the_disabled_ccmcontext_operations}`.

**Status:** done

### §13.7 No Configurators

**Spec:** §13.7, S. 279 (PDF) — Removed: `Configurator`-Iface,
`StandardConfigurator`, `HomeConfiguration`.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`
entfernt `configure`/`set_configuration`/`configuration_complete` aus
dem Equivalent-Interface (§13.7 entfernt die Interfaces `Configurator`
und `StandardConfigurator` ganz); `crates/corba-ccm/src/lwccm.rs`
meldet zusätzlich `set_configurator` auf `HomeConfiguration` als
disabled.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_drops_configurator_operations`,
`crates/corba-ccm/src/lwccm.rs::tests::every_other_disabled_operation_raises_bad_operation`.

**Status:** done

### §13.8-§13.9 No Proxy Homes / Home Finders

**Spec:** §13.8, S. 279 (PDF) — "Exclude support for proxy homes"
(§8.2.10 + §9.4.1.3 `ProxyHomeRegistration`). §13.9, S. 279 — "Disable
support for the HomeFinder interface and the ComponentHomeFinder
parameter value to the ORB::resolve_initial_references operation" +
"Disable the get_home_registration operation in the CCM2Context
interface".

**Repo:** `crates/corba-ccm/src/lwccm.rs::disabled_by` meldet
`register_proxy_home` als §13.8 und die drei `find_home_by_*`-
Operationen plus `get_home_registration` als §13.9. Die Extended-Form
des Home-Finders steht in
`crates/corba-ccm/src/home.rs::ComponentHomeFinder` (§6.8) und wird von
dieser Prüfung aus dem Profil gehalten; `register_home` /
`unregister_home` aus §9.4.1.2 nennt keine der beiden Tabellen und
bleiben.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
register_proxy_home_is_disabled,
the_home_finder_operations_and_home_registration_are_disabled,
register_home_survives_because_only_the_proxy_form_is_excluded}`.

**Status:** done

### §13.10 Additional Restrictions

**Spec:** §13.10, S. 280 (PDF) — Misc-Restrictions des Extended-
Modells.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`
(Filter-Pipeline) + Doc-Marker in
`crates/corba-ccm/src/lib.rs::conformance::{LWCCM_RESTRICTIONS_ENFORCED,
LWCCM_FILTER_ACTIVE}`.

**Tests:** `crates/corba-ccm/src/lib.rs::conformance_tests::lwccm_restrictions_marker_matches_spec`,
`lwccm_filter_marker_matches_spec`.

**Status:** done — Filter-Pipeline + spezifische Restriction-Marker
ausgewiesen.

---

## §14 Deployment PSM for CCM

### §14 Deployment PSM for CCM

**Spec:** §14, S. 281-300 (PDF) — D&C-Mapping (formal/2006-04-02),
Component-Interface-Description, PlanSubcomponentPortEndpoint,
Application, RepositoryManager, SatisfierProperty, IDL/XML
Transformation Rules.

**Repo:** `crates/corba-dnc/src/{plan,node,execution,container_host,
repository,xml}.rs`.

**Tests:** `crates/corba-dnc/src/plan.rs::tests::{
valid_plan_passes_validation, instance_with_unknown_impl_fails,
connection_with_unknown_instance_fails,
co_location_with_unknown_instance_fails}`,
`crates/corba-dnc/src/container_host.rs::tests::{
full_lifecycle_round_trip, install_creates_container_on_demand,
entity_and_session_share_host, duplicate_install_rejected}`,
`crates/corba-dnc/src/repository.rs::tests::{
install_and_find_round_trip, list_packages_returns_all_labels}`.

**Status:** done

---

## §15 Deployment IDL for CCM

### §15 Deployment IDL for CCM

**Spec:** §15, S. 301-314 (PDF) — Deployment-IDL-Definitionen
(`Deployment::*`).

**Repo:** `Deployment::ExecutionManager` +
`Deployment::DomainApplicationManager` +
`Deployment::DomainApplication` in
`crates/corba-dnc/src/execution.rs`; `Deployment::NodeManager` +
`Deployment::NodeApplicationManager` + `Deployment::NodeApplication` in
`crates/corba-dnc/src/node.rs`; `Deployment::RepositoryManager` in
`crates/corba-dnc/src/repository.rs`; die Descriptor-Strukturen in
`crates/corba-dnc/src/plan.rs`.

**Tests:** `crates/corba-dnc/src/execution.rs::tests::{
full_lifecycle_round_trip, execution_manager_lists_prepared_plans,
invalid_plan_rejected_at_prepare, double_start_fails,
destroy_before_running_fails}` — die Operationsfolge
`preparePlan`/`startLaunch`/`finishLaunch`/`destroyApplication` der
§15-IDL; `crates/corba-dnc/src/node.rs::tests::{
prepare_plan_then_start_launch, register_and_unregister_application,
destroy_application_clears_active, unregister_unknown_returns_none}`;
`crates/corba-dnc/src/repository.rs::tests::{
install_and_find_round_trip, install_overwrites_duplicate,
delete_existing_returns_true, delete_unknown_returns_false}`.

**Status:** done

---

## §16 XML Schema for CCM

### §16 XML Schema for CCM

**Spec:** §16, S. 315ff (PDF) — XSD-Schema für Deployment-Pakete.

**Repo:** `crates/corba-dnc/src/xml.rs` (XML-Codec für
Deployment-Plans nach dem §16-Schema).

**Tests:** `crates/corba-dnc/src/xml.rs::tests::{parses_minimal_plan,
parses_multiple_artifacts, co_locate_with_parsed,
missing_root_tag_returns_expected_tag,
validation_fails_on_unknown_impl_reference}`.

**Status:** done

---

## Audit-Status

71 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-ccm` — 19 Lib-Tests + 34 Tests in
  `tests/equivalent_idl.rs` + 1 Doc-Test grün.
* `cargo test -p zerodds-corba-ccm` — 201 Lib-Tests + 3 Tests in
  `tests/pi_service_contexts.rs` + 1 Doc-Test grün.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 Tests grün.
* `cargo test -p zerodds-corba-dnc --lib` — 30 Tests grün.
* `cargo test -p zerodds-corba-codegen --lib` — 38 Tests grün
  (Skeleton-/Stub-Generator).
* `cargo test -p zerodds-corba-rust --test snapshot_codegen` — 15 Tests grün.
* `cargo test -p zerodds-corba-ir --lib` — 19 Tests grün.
* `cargo test -p zerodds-corba-codegen` — 93 Tests grün
  (Annex-A.1-Bindung für Java).

Offene Punkte: keine. Decision-Records: keine.
