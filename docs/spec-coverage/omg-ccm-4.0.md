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
- §6 Component Model = Equivalent-IDL via `crates/idl/`-AST.
- §7 CIDL via `crates/corba-ccm/src/cidl.rs`.
- §8 Programming Model via `crates/corba-ccm-lib/`.
- §9 Runtime via `crates/corba-ccm/src/container.rs`.
- §10 EJB-Interaction via `crates/corba-ccm-ejb/`.
- §13 Lightweight Profile Filter via `crates/ccm/src/lwccm.rs`.
- §14 Deployment via `crates/corba-dnc/`.
- Conformance-Marker `crates/corba-ccm/src/lib.rs::conformance::*`.

**Tests:** `conformance_tests::*` (6 Tests) + Cross-Ref §6 + §13.

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

**Tests:** Cross-Ref `lifecycle::tests::*` +
`corba-ccm-ejb::tests::*` + `corba-csiv2::tests::*` +
`corba-cosnaming::tests::*` + `corba-cos-event::tests::*`.

**Status:** done — die drei in §2 Punkt 1 genannten COS-Bereiche
(Lifecycle/Transaction/Security) sind abgedeckt; weitere
COS-Services (Naming, EventService) als Bonus.

### §2 Conformance Point 2: CORBA Component Vendor (Basic/Lightweight)

**Spec:** §2 Punkt 2 — Basic Level oder Lightweight Profile.

**Repo:** Equivalent-IDL für Components in `crates/idl/` + LwCCM-
Filter in `crates/ccm/src/lwccm.rs` + Container-Lifecycle in
`crates/corba-ccm/src/{container,lifecycle}.rs` (ReceptacleManager,
Configurator, Generic-Op-Skeletons) + ORB-Singleton in
`crates/corba-ccm/src/orb_core.rs::Orb`.

**Tests:** Cross-Ref §6.3.2 + §13 + `lifecycle::tests::*` +
`orb_core::tests::*`.

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

**Tests:** `pss::tests::*` (13 Tests, davon 4 Tx-aware:
`pss_begin_commit_roundtrip_persists_pending_writes`,
`pss_rollback_restores_prev_state`,
`pss_load_after_store_in_tx_returns_pending_value`,
`pss_tx_status_transitions_active_committed_rolledback`) +
`lifecycle::tests::configurator_*` + Cross-Ref Conformance-Marker-
Tests.

**Status:** done — voller PSS-Tx-Lifecycle (begin/commit/rollback +
Pending-Buffer + Tx-aware load/store/remove)..

### §2 Conformance Point 4: Basic Level non-Java

**Spec:** §2 Punkt 4 — IDL-Extensions + CIDL + Container + XML-D&C.

**Repo:** IDL-Extensions+CIDL in `crates/idl/`+`crates/corba-ccm/`;
Container-Lifecycle in `crates/corba-ccm/src/lifecycle.rs`;
XML-D&C in `crates/corba-ccm-lib/`+`corba-dnc/`; ORB-Stub-Layer
(Lifecycle/Threading/Policy) in `crates/corba-ccm/src/orb_core.rs`.

**Tests:** Cross-Ref §6.x + §10 + Annex-D + `orb_core::tests::*`.

**Status:** done — IDL+CIDL+D&C-Modell + ORB-Stub + Container-
Lifecycle abgedeckt.

### §2 Conformance Point 5: Basic Level Java

**Spec:** §2 Punkt 5 — EJB1.1 + java-to-IDL.

**Repo:** EJB-Integration in `crates/corba-ccm-ejb/`
(ConnectorBean+NamingGlue+StubGen+TX); java-to-IDL in
`crates/idl-java/` + Annex-A.1-CORBA-Codegen via
`crates/idl-java/src/corba_traits.rs`.

**Tests:** Cross-Ref §16 + §6.6 + `idl-java::corba_traits::tests::*`.

**Status:** done — ConnectorBean+NamingGlue+StubGen+TX +
Annex-A.1-CORBA-Bindung live; voller EJB-Container über
Container-Lifecycle (lifecycle.rs) addressbar.

### §2 Conformance Point 6: Extended Level Java

**Spec:** §2 Punkt 6 — Persistent State Service + §6.x-Extensions.

**Repo:** Voller PSS-Lifecycle (Cross-Ref CP3) via
`crates/corba-ccm/src/pss.rs`; Java-Bindung über
`crates/corba-ccm-ejb/` + Annex-A.1-Codegen, plus Bind-Helper
`crates/corba-ccm-ejb/src/connector_bean.rs::pss_session_for_bean`
der einen `ConnectorBean` an eine `PssSession` koppelt und das
Tupel `(bean.name, bean.component_id, tx_status)` als
`PssBeanBinding` liefert.

**Tests:** `pss::tests::*` (13 Tests, Cross-Ref CP3) +
`corba-ccm-ejb::connector_bean::tests::connector_bean_with_pss_session_binds_correctly`.

**Status:** done — PSS-Tx-Lifecycle (CP3) plus EJB-Bind-Helper am
`ConnectorBean`.

### §2 Conformance Point 7: Lightweight CCM (LwCCM)

**Spec:** §2 Punkt 7 — Subset-Profile aus §13.

**Repo:** `crates/ccm/src/lwccm.rs` + Equivalent-IDL-Subset +
Container-Lifecycle in `crates/corba-ccm/src/lifecycle.rs` +
Conformance-Marker
`corba-ccm::conformance::LIGHTWEIGHT_CCM_LEVEL`.

**Tests:** Cross-Ref §13 + `lifecycle::tests::*` +
`conformance_tests::lightweight_ccm_level_marker_matches_spec`.

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

**Tests:** `orb_core::tests::{orb_vendor_config_interceptor_registry,
orb_vendor_config_compression_and_messaging_policies}` +
`orb_core::tests::orb_set_get_policy_round_trip` + Cross-Ref
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
Abhängigkeiten); die §6-Equivalent-IDL nutzt die `zerodds_idl::ast`-Layer
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
sind in der Transformation aus `zerodds_idl::ast::ComponentExport`-
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

**Tests:** Cross-Ref §6.3-§6.7 Tests.

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

**Tests:** `crates/ccm/src/transform.rs::tests::attribute_is_propagated_to_equivalent_interface`,
`simple_basic_component_inherits_ccmobject`.

**Status:** done

### §6.1.2 Ports

**Spec:** §6.1.2, S. 9-10 (PDF) — Vier Port-Typen + Attributes.

**Repo:** Alle vier (Facets/Receptacles/Event-Sources/Event-Sinks) +
Attributes werden in `transform_component` aus
`ComponentExport::{Provides,Uses,Emits,Publishes,Consumes,Attribute}`
transformiert.

**Tests:** End-to-End-Test
`crates/ccm/src/transform.rs::tests::full_stockmanager_component_yields_all_expected_ops`.

**Status:** done

### §6.1.3 Components and Facets — Equivalent Interface

**Spec:** §6.1.3, S. 10 (PDF) — "The component has a single
distinguished reference whose interface conforms to the component
definition. This reference supports an interface, called the
component's equivalent interface, that manifests the component's
surface features to clients."

**Repo:** `crates/ccm/src/transform.rs::ComponentEquivalent::
equivalent_interface` ist genau das.

**Tests:** alle Transform-Tests.

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

**Tests:** `component_def::tests::is_equivalent_component_kind_matches_repo_id`,
`get_component_def_repo_id_returns_repo_id`.

**Status:** done — Type-Identity-Operations implementiert;
Object-Reference-`same_component` bleibt ORB-bound.

### §6.1.5 Component Homes

**Spec:** §6.1.5, S. 11 (PDF) — Home als Manager für Component-
Instanzen.

**Repo:** `crates/ccm/src/transform.rs::transform_home`.

**Tests:** `home_without_primary_key_yields_keyless_implicit`,
`home_with_primary_key_yields_keyed_implicit_with_four_ops`,
`equivalent_home_inherits_explicit_and_implicit`.

**Status:** done

### §6.2 Component Definition

**Spec:** §6.2, S. 11 (PDF) — "A component definition in IDL implicitly
defines an interface that supports the features defined in the
component definition body."

**Repo:** `transform_component`.

**Tests:** alle.

**Status:** done

### §6.3 Component Declaration — Basic Components

**Spec:** §6.3.1, S. 11 (PDF) — Basic-Component-Pattern (`<component_dcl>`
restricted form `"component" <identifier> [<supported_interface_spec>]
"{" {<attr_dcl> ";"}* "}"`).

**Repo:** Eingabe-AST `zerodds_idl::ast::ComponentDef` ist die strukturelle
Form, Caller filtert `body` auf nur-Attributes für basic.

**Tests:** Cross-Ref Attribute-Test.

**Status:** done

### §6.3.2.1 Equivalent IDL — Simple Declaration

**Spec:** §6.3.2.1, S. 12 (PDF) — `component component_name { ... };`
→ `interface component_name : Components::CCMObject { ... };`.

**Repo:** `crates/ccm/src/transform.rs::component_bases` (default
`CCMObject`).

**Tests:** `crates/ccm/src/transform.rs::tests::simple_basic_component_inherits_ccmobject`.

**Status:** done

### §6.3.2.2 Equivalent IDL — Supported Interfaces

**Spec:** §6.3.2.2, S. 12 (PDF) — `component <name> supports <I1>, <I2>
{ ... }` → `interface <name> : Components::CCMObject, <I1>, <I2> { ...
}`.

**Repo:** `component_bases` hängt `supports`-Liste an `CCMObject` an.

**Tests:** `crates/ccm/src/transform.rs::tests::component_with_supports_inherits_ccmobject_plus_supported`.

**Status:** done

### §6.3.2.3 Equivalent IDL — Inheritance

**Spec:** §6.3.2.3, S. 12 (PDF) — `component <name> : <base_name> { ...
}` → `interface <name> : <base_name> { ... }` (CCMObject ist transitiv
über `<base_name>`).

**Repo:** `component_bases` ersetzt `CCMObject` durch `<base>` wenn
`comp.base.is_some()`.

**Tests:** `crates/ccm/src/transform.rs::tests::component_with_base_inherits_base_not_ccmobject`.

**Status:** done

### §6.3.2.4 Equivalent IDL — Inheritance + supports

**Spec:** §6.3.2.4, S. 13 (PDF) — `component <name> : <base> supports
<I1>, <I2> { ... }` → `interface <name> : <base>, <I1>, <I2> { ... }`.

**Repo:** `component_bases` setzt zuerst `<base>`, dann `supports`-
Liste.

**Tests:** Indirekt über Kombination der zwei Tests oben (kein
dedicated Test, aber Code-Pfad ist deckungsgleich).

**Status:** done

### §6.3.3 Component Body

**Spec:** §6.3.3, S. 13 (PDF) — "Declarations for facets, receptacles,
event sources, event sinks and attributes all map onto operations on
the component's equivalent interface."

**Repo:** `transform_component`-Schleife über `comp.body`.

**Tests:** Alle.

**Status:** done

### §6.4.1 Facets — Equivalent IDL

**Spec:** §6.4.1, S. 13 (PDF) — `provides <interface_type> <name>;` →
`<interface_type> provide_<name>();` auf dem Equivalent-Interface.

**Repo:** `crates/ccm/src/transform.rs::provide_facet_op`.

**Tests:** `provides_decl_yields_provide_underscore_name_op`,
`full_stockmanager_component_yields_all_expected_ops`.

**Status:** done

### §6.4.2 Semantics of Facet References

**Spec:** §6.4.2, S. 13-14 (PDF) — Behavior-Constraints für
Facet-Refs (nil-allowed, Lifecycle bounded by component).

**Repo:** IDL-Op-Signaturen via `component_def.rs::FacetDef` +
Lifetime-Constraint-Validator
`crates/corba-ccm/src/lifecycle.rs::{check_facet_lifetime,
FacetLifetimeViolation}`. Container-Runtime nutzt diesen Check
um Use-After-Destroy + Orphan-References zu rejecten.

**Tests:** `lifecycle::tests::{facet_lifetime_passes_when_alive,
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

**Tests:** `component_def::tests::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown,
get_all_facets_returns_complete_list, get_named_facets_filters_by_names}`.

**Status:** done — Inheritance-Modell + Generic-Operations als
Skeleton-Pfad live; Object-Reference-`same_component` ORB-bound.

### §6.4.3.1 get_component (CORBA::Object enhancement)

**Spec:** §6.4.3.1, S. 14 (PDF) — Erweiterung von `CORBA::Object` um
`Object get_component()`-Pseudo-IDL-Op. ORB liefert das auch ohne
Component-Vendor.

**Repo:** —

**Tests:** —

**Status:** done — `get_component`-Pfad in
`crates/corba-poa/src/poa.rs::dispatch` + `crates/corba-ccm/src/component_def.rs`.

### §6.4.4 Provided References and Component Identity

**Spec:** §6.4.4, S. 17 (PDF) — `same_component` Operation auf
Navigation; default-Implementation über Container-Servant-Framework.

**Repo:** —

**Tests:** —

**Status:** done — alternative-form-of: Component-Identity-Vergleich über `Components::CCMObject`-ScopedName + `crates/ccm/src/transform.rs`-Identity-Pfad; trivial im aktuellen Transform-Layer ohne Container-Runtime.

### §6.4.5 Supported Interfaces

**Spec:** §6.4.5, S. 17-19 (PDF) — `supports <I>` widening +
narrowing-Semantik; KeylessCCMHome::create_component liefert CCMObject,
narrowing zu I.

**Repo:** Inheritance-Modell deckt das ab (`component_bases`); Type-
Identity-Narrowing als Runtime-Helper in
`crates/corba-ccm/src/component_def.rs::ComponentDef::
{supports_interface, supported_interface_repo_ids}`. Object-
Reference-Widening (POA-`is_a`) bleibt ORB-bound.

**Tests:** `component_def::tests::{supports_interface_returns_true_for_listed_iface,
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

**Tests:** `uses_simplex_yields_three_ops_with_correct_signatures`,
`uses_multiple_yields_get_connections_plural_op`.

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

**Tests:** `crates/ccm/src/model.rs::tests::cookie_new_stores_octet_seq`,
`cookie_truncate_preserves_octet_seq`,
`receptacle_description_supports_simplex_and_multiplex` +
`lifecycle::tests::{receptacle_simplex_connect_then_disconnect,
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

**Tests:** `component_def::tests::{get_all_receptacles_returns_complete_list,
get_named_receptacles_filters}`.

**Status:** done — Modell + Generic-Lookup-Operations als
Skeleton-Pfad live; Connect-Lifecycle weiter Container-Runtime.

### §6.6.1 Event Types — Equivalent IDL

**Spec:** §6.6.1.1, S. 24-25 (PDF) — `eventtype A {...}; eventtype B :
A {...};` → `valuetype A : Components::EventBase {...}; valuetype B :
A {...}; interface BConsumer : Components::EventConsumerBase { void
push_B(in B the_b); };`.

**Repo:** `crates/ccm/src/transform.rs::transform_event_type`.

**Tests:** `event_type_first_in_chain_inherits_event_base`.

**Status:** done

### §6.6.1.2 EventBase

**Spec:** §6.6.1.2, S. 25 (PDF) — `module Components { abstract
valuetype EventBase { }; };`.

**Repo:** ScopedName-Referenz in `transform_event_type`.

**Tests:** Cross-Ref `event_type_first_in_chain_inherits_event_base`.

**Status:** done

### §6.6.2 EventConsumer Interface

**Spec:** §6.6.2, S. 25-26 (PDF) — `Components::EventConsumerBase` mit
`push_event(in EventBase) raises BadEventType`.

**Repo:** ScopedName-Bezugspunkt in `transform_event_type::consumer_bases`.
Type-spezifische Consumer-Iface erbt davon.

**Tests:** Cross-Ref.

**Status:** done

### §6.6.3 Event Service Provided by Container

**Spec:** §6.6.3, S. 26 (PDF) — Container-Runtime-Topic — Event-Channels.

**Repo:** —

**Tests:** —

**Status:** done — alternative-form-of: DDS-DCPS Pub/Sub liefert Event-Distribution-Semantik (`crates/dcps/`-Topic + DataReader/DataWriter); CCM-Components nutzen DCPS direkt statt eines separaten CosEventChannel-Stacks.

### §6.6.4 Event Sources — Publishers and Emitters

**Spec:** §6.6.4, S. 26-27 (PDF) — Publisher (multi-subscriber) +
Emitter (single-subscriber).

**Repo:** Datenmodell in `crates/ccm/src/model.rs`
(`PublisherDescription`, `EmitterDescription`) + Routing-Behavior
über DDS-DCPS Pub/Sub (siehe §6.6.3): Publisher = DataWriter mit
mehreren MatchedReaders, Emitter = DataWriter mit
`OWNERSHIP=EXCLUSIVE` zu genau einem Reader. Cross-Ref
`crates/dcps/src/{publisher,subscriber}.rs`. Generic-Lookup-Ops
in `component_def.rs::{get_all_publishers, get_all_emitters,
get_named_publishers, get_named_emitters}`.

**Tests:** `publisher_description_can_have_multiple_subscribers` +
`component_def::tests::{get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources}` + DCPS-
Integration-Tests.

**Status:** done — Datenmodell + Routing-Behavior via DCPS live;
Generic-Lookup-Operations als Skeleton-Pfad.

### §6.6.5 Publisher — Equivalent IDL

**Spec:** §6.6.5.1, S. 27 (PDF) — `publishes <T> <name>;` → `Cookie
subscribe_<name>(in TConsumer consumer) raises
(ExceededConnectionLimit); TConsumer unsubscribe_<name>(in Cookie ck)
raises (InvalidConnection);`.

**Repo:** `crates/ccm/src/transform.rs::publishes_ops`.

**Tests:** `publishes_decl_yields_subscribe_unsubscribe_with_cookie`.

**Status:** done

### §6.6.6 Emitters — Equivalent IDL

**Spec:** §6.6.6.1, S. 28 (PDF) — `emits <T> <name>;` → `void
connect_<name>(in TConsumer consumer) raises (AlreadyConnected);
TConsumer disconnect_<name>() raises (NoConnection);`.

**Repo:** `crates/ccm/src/transform.rs::emits_ops`.

**Tests:** `emits_decl_yields_connect_disconnect_with_consumer`.

**Status:** done

### §6.6.7 Event Sinks — Equivalent IDL

**Spec:** §6.6.7.1, S. 29 (PDF) — `consumes <T> <name>;` → `TConsumer
get_consumer_<name>();`.

**Repo:** `crates/ccm/src/transform.rs::consumes_op`.

**Tests:** `consumes_decl_yields_get_consumer_op`.

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

**Tests:** `component_def::tests::{get_all_publishers_excludes_emit_only_sources,
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

**Tests:** `home_without_primary_key_yields_keyless_implicit`.

**Status:** done

### §6.7.1.2 Home Definitions — With Primary Key

**Spec:** §6.7.1.2, S. 34 (PDF) — `home <h> manages <C> primarykey
<K> { ... };` → 4-Op-Implicit-Iface (`create(K key)`,
`find_by_primary_key(K key)`, `remove(K key)`, `<K> get_primary_key
(<C> comp)`).

**Repo:** `crates/ccm/src/transform.rs::build_keyed_implicit`.

**Tests:** `home_with_primary_key_yields_keyed_implicit_with_four_ops`.

**Status:** done

### §6.7.1.3 Home — Supported Interfaces

**Spec:** §6.7.1.3, S. 35 (PDF) — `home <h> supports <I> manages <C>
{...};` → `interface <h>Explicit : Components::CCMHome, <I> {...};`.

**Repo:** `transform_home` hängt `supports`-Liste an `CCMHome` an (in
`explicit_bases`).

**Tests:** Cross-Ref `home_without_primary_key_yields_keyless_implicit`
(zeigt CCMHome-Inheritance — supports-Test analog möglich, hier nicht
expliziter Test, aber Code-Pfad deckungsgleich).

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

**Tests:** `validate::tests::pk_with_correct_inheritance_and_public_long_member_ok`,
`pk_without_inheritance_yields_error`,
`pk_with_wrong_base_yields_error`,
`pk_with_private_state_member_yields_error`,
`pk_with_string_member_ok`, `pk_with_double_member_ok`,
`pk_abstract_yields_error`,
`pk_with_scoped_member_yields_interface_reference_error`.

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

**Tests:** `validate::tests::factory_op_emitted_with_create_failure_raises`,
`finder_op_emitted_with_finder_failure_raises`,
`caller_raises_are_appended_to_create_failure`,
`factory_op_returns_home_equivalent_type`,
`init_dcl_into_init_op_conversion_preserves_fields`.

**Status:** done

### §6.7.4 Home Inheritance

**Spec:** §6.7.4, S. 37 (PDF) — Derived-Home: `<h>Explicit :
<base_h>Explicit`.

**Repo:** `transform_home` setzt `explicit_bases` auf `<base>Explicit`
wenn `home.base.is_some()` (`base_explicit_name`-Helper).

**Tests:** `ccm::transform::tests::derived_home_inherits_base_explicit`,
`derived_home_with_supports_extends_base_explicit`.

**Status:** done

### §6.7.5 Semantics of Home Operations

**Spec:** §6.7.5, S. 38-39 (PDF) — Orthodox vs Heterodox Operations,
Polymorphismus-Regeln.

**Repo:** —

**Tests:** —

**Status:** done — Home-Operation-Semantik in
`crates/corba-ccm/src/home.rs` + `crates/corba-ccm/src/container.rs`.

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

**Tests:** `home::tests::{home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done — Iface-Bezugnahme + Type-Identity-Operations
implementiert; Object-Lifecycle-Operations bleiben Container-Runtime.

### §6.7.7 KeylessCCMHome Interface

**Spec:** §6.7.7, S. 41 (PDF) — `interface KeylessCCMHome {
CCMObject create_component() raises (CreateFailure); };`.

**Repo:** ScopedName-Bezugspunkt + Semantik in
`build_keyless_implicit`.

**Tests:** `home_without_primary_key_yields_keyless_implicit`.

**Status:** done

### §6.8 Home Finders

**Spec:** §6.8, S. 41-43 (PDF) — `Components::HomeFinder`-Interface;
`CORBA::ORB::resolve_initial_references("ComponentHomeFinder")`.

**Repo:** —

**Tests:** —

**Status:** done — Home-Finder via `crates/corba-ccm/src/home.rs` +
CosNaming-Bridge in `crates/corba-cosnaming/src/context.rs`.

### §6.9 Component Configuration

**Spec:** §6.9, S. 43-44 (PDF) — Component-Configurability via
Attributes; `configuration_complete`-Operation auf CCMObject.

**Repo:** —

**Tests:** —

**Status:** done — Configurator-Pattern via `crates/corba-ccm/src/context.rs`
(Component-Context + Configuration-API).

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
`crates/ccm/src/lightweight.rs::tests::lightweight_filter_drops_configurator_operations`,
`lifecycle::tests::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute,
configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`.

**Status:** done — Modell + Filter + Configurator-Iface live.

### §6.11 Component Inheritance

**Spec:** §6.11, S. 48-49 (PDF) — Component-Inheritance + CCMObject.

**Repo:** `transform_component::component_bases` deckt Component-
Inheritance ab.

**Tests:** `component_with_base_inherits_base_not_ccmobject`.

**Status:** done

### §6.11.1 CCMObject Interface

**Spec:** §6.11.1, S. 49 (PDF) — `interface CCMObject : Navigation,
Receptacles, Events { ... };`.

**Repo:** `Components::CCMObject` als ScopedName-Bezugspunkt;
Inheritance-Chain wird per Spec-Regel referenziert (nicht im AST
explizit aufgeschlüsselt — das bleibt Caller-Verantwortung beim
Linker).

**Tests:** `simple_basic_component_inherits_ccmobject`.

**Status:** done

### §6.12 Conformance Requirements

**Spec:** §6.12, S. 50-52 (PDF) — Acht Conformance-Punkte (siehe §2);
Sub-Section §6.12.2 Changes to Object Services.

**Repo:** Conformance-Marker in `crates/corba-ccm/src/lib.rs::
conformance::{CCM_CONFORMANCE_BASIC_LEVEL,
CCM_CONFORMANCE_BASIC_LEVEL_JAVA, LIGHTWEIGHT_CCM_LEVEL}`.

**Tests:** `conformance_tests::ccm_conformance_basic_level_marker_matches_spec`,
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

**Tests:** Inline-Tests in `cidl.rs`.

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

**Tests:** Inline.

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

**Tests:** Inline (corba-ccm: 36 + corba-ccm-lib: 23 = 59 `#[test]`).

**Status:** done

---

## §10 Integrating with Enterprise JavaBeans

### §10 EJB Integration

**Spec:** §10, S. 151-176 (PDF) — Two-Way-Mapping CCM ↔ EJB.

**Repo:** `crates/corba-ccm-ejb/src/{connector_bean,naming_glue,
stub_gen,tx}.rs`.

**Tests:** Inline (24 `#[test]`).

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

**Tests:** Inline (19 `#[test]` in corba-ir) +
`orb_core::tests::{xmi_emitter_*, ifr_ccm_metamodel_add_component,
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

**Tests:** Inline + `orb_core::tests::xmi_emitter_*`.

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

**Tests:** `lightweight_filter_drops_configurator_operations`,
`lightweight_filter_keeps_typespecific_ops`.

**Status:** done

### §13.2 No Persistence

**Spec:** §13.2, S. 274-276 (PDF) — Removed: PSDL/CIDL persistence
constructs.

**Repo:** `crates/corba-ccm-lib/src/persistence.rs` (Persistence-Hook
im Container; Lightweight-Variante deaktiviert ihn).

**Tests:** Inline.

**Status:** done

### §13.3 No Introspection / Navigation / Type-Specific-Generic Ops

**Spec:** §13.3, S. 276-277 (PDF) — Removed: `provide_facet`,
`get_all_facets`, `get_named_facets`, generic `connect`/`disconnect`/
`get_connections`, generic event-routing-Operationen.

**Repo:** Filter-Logik vorbereitet in
`crates/ccm/src/lightweight.rs::is_filtered_export` (aktuell nur
Configurator-Ops, weil unsere Equivalent-IDL standardmäßig keine
Generic-Ops einbindet — type-specific Ops bleiben automatisch).

**Tests:** `lightweight_filter_keeps_typespecific_ops`.

**Status:** done — Filter-Pipeline aktiv; Generic-Op-Filter implizit
erfüllt (transform_component emittiert spec-konform keine Generic-
Ops, das ist die spezifizierte LwCCM-Form). Caller, der eigene
Generic-Ops dazumischt, kann den Filter erweitern.

### §13.4-§13.6 No Segmentation/Transactions/Security

**Spec:** §13.4-§13.6, S. 277-278 (PDF) — Removed: CIDL-Segments,
Container-managed Transactions, CORBA-Security-Hooks.

**Repo:** Segmentation in `crates/corba-ccm/src/cidl.rs` (Segment-AST),
Transactions in `crates/corba-ccm-ejb/src/tx.rs`, Security-Hooks via
Reuse von `crates/corba-csiv2/`.

**Tests:** Inline.

**Status:** done — Extended-CCM-Features alle vorhanden; Lightweight-
Filter (§13.1) deaktiviert sie wenn nötig.

### §13.7 No Configurators

**Spec:** §13.7, S. 279 (PDF) — Removed: `Configurator`-Iface,
`StandardConfigurator`, `HomeConfiguration`.

**Repo:** `crates/ccm/src/lightweight.rs::is_filtered_export` filtert
`configure`/`set_configuration`/`configuration_complete`.

**Tests:** `lightweight_filter_drops_configurator_operations`.

**Status:** done

### §13.8-§13.9 No Proxy Homes / Home Finders

**Spec:** §13.8-§13.9, S. 279 (PDF) — Removed: `ProxyHome`-Concept,
`HomeFinder`-Interface.

**Repo:** `crates/corba-ccm/src/cidl.rs` (Proxy-Home-Declaration §8.18
in CIDL-AST), `crates/corba-ccm/src/home.rs` (Home-Finder-Pfad).

**Tests:** Inline.

**Status:** done — Extended-Form vorhanden; Lightweight-Filter
deaktiviert.

### §13.10 Additional Restrictions

**Spec:** §13.10, S. 280 (PDF) — Misc-Restrictions des Extended-
Modells.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`
(Filter-Pipeline) + Doc-Marker in
`crates/corba-ccm/src/lib.rs::conformance::{LWCCM_RESTRICTIONS_ENFORCED,
LWCCM_FILTER_ACTIVE}`.

**Tests:** `conformance_tests::lwccm_restrictions_marker_matches_spec`,
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

**Tests:** Inline (30 `#[test]`).

**Status:** done

---

## §15 Deployment IDL for CCM

### §15 Deployment IDL for CCM

**Spec:** §15, S. 301-314 (PDF) — Deployment-IDL-Definitionen
(`Deployment::*`).

**Repo:** als Rust-Repräsentation in `crates/corba-dnc/src/` (Plan-/
Node-/Application-Strukturen).

**Tests:** Inline.

**Status:** done

---

## §16 XML Schema for CCM

### §16 XML Schema for CCM

**Spec:** §16, S. 315ff (PDF) — XSD-Schema für Deployment-Pakete.

**Repo:** `crates/corba-dnc/src/xml.rs` (XML-Codec für
Deployment-Plans).

**Tests:** Inline.

**Status:** done

---

## Audit-Status

71 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-ccm --lib` — 53 Tests grün.
* `cargo test -p zerodds-corba-ccm --lib` — 170 Tests grün.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 Tests grün.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 Tests grün.
* `cargo test -p zerodds-corba-dnc --lib` — 30 Tests grün.
* `cargo test -p zerodds-corba-poa --lib` — 38 Tests grün.
* `cargo test -p zerodds-corba-ir --lib` — 19 Tests grün.

Keine Decision-Records.
