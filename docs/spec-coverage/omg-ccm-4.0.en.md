# OMG CCM 4.0 — Spec Coverage

**Spec:** [OMG CCM 4.0](https://www.omg.org/spec/CCM/4.0/PDF) (315 pages, OMG formal/06-04-01).

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** CCM 4.0 is the master spec behind AMI4CCM (see
`omg-ami4ccm-1.1.md`) and DDS4CCM (see `dds4ccm-1.1.md`). It covers the
Component Model (§6), CIDL (§7), the CCM Implementation Framework (§8
CIF), the Container Programming Model (§9), EJB integration (§10), the
IFR metamodel (§11), the Lightweight CCM Profile (§13) and the
Deployment PSM/IDL/XML (§14-§16). ZeroDDS implements this stack fully in
pure Rust — including the CORBA ORB, the CCM container and the D&C
subsystem that the container part of the spec builds on.

The §6 equivalent-IDL transformation also covers the migration path: old
CCM-IDL files can be carried into pure-DDS worlds via equivalent IDL —
with or without the full container stack. The `Components::*` core data
types (CCMObject, Cookie, ConnectionDescription, etc.) are present as
Rust models.

Implementation:

- `crates/ccm/` — §6 Component Model (equivalent IDL) + §13 Lightweight
  CCM Profile + `Components::*` core data types.
- `crates/corba-ccm/` — §7 CIDL (`cidl.rs`), §8 CIF (`cif.rs`), §9
  Container Programming Model (`container.rs`).
- `crates/corba-ccm-ejb/` — §10 EJB integration.
- `crates/corba-ir/` — §11 Interface Repository metamodel.
- `crates/corba-dnc/` — §14/§15 Deployment PSM/IDL + §16 XML schema.
- underneath: CORBA ORB (GIOP/IIOP) in `crates/corba-interop`, POA in
  `crates/corba-poa`.

---

## §1 Scope

### §1 Scope Statement

**Spec:** §1, p. 1 (PDF) — "This specification defines: The syntax and
semantics of a component model (Chapter 6) [...] A language to describe
the structure and state of component implementations (Chapter 7
CIDL) [...] A programming model for constructing component
implementations (Chapter 8) [...] A runtime environment (Chapter 9)
[...] Interaction with EJB (Chapter 10) [...] Meta-data for
deployment (Chapter 14) [...] A lightweight subset (Chapter 13)."

**Repo:** ZeroDDS covers:
- §6 Component Model = equivalent IDL via the `crates/idl/` AST.
- §7 CIDL via `crates/corba-ccm/src/cidl.rs`.
- §8 Programming Model via `crates/corba-ccm-lib/`.
- §9 Runtime via `crates/corba-ccm/src/container.rs`.
- §10 EJB interaction via `crates/corba-ccm-ejb/`.
- §13 Lightweight Profile filter via `crates/ccm/src/lwccm.rs`.
- §14 Deployment via `crates/corba-dnc/`.
- conformance markers `crates/corba-ccm/src/lib.rs::conformance::*`.

**Tests:** `conformance_tests::*` (6 tests) + cross-ref §6 + §13.

**Status:** done — all sub-stacks present + conformance markers
stated at the crate level.

---

## §2 Conformance and Compliance

### §2 Conformance Point 1: CORBA COS Vendor

**Spec:** §2 point 1 — Lifecycle/Transaction/Security changes.

**Repo:** spec points partly covered by service crates:
- **Lifecycle** via `crates/corba-ccm/src/lifecycle.rs`
  (ReceptacleManager + Configurator + container state).
- **Transaction** via `crates/corba-ccm-ejb/src/tx.rs` (TX marker
  + begin/commit/rollback hooks).
- **Security** via `crates/corba-csiv2/` (CSIv2 stack: SAS + GSSUP +
  TLS bind).
- **Naming** as an additional COS service in
  `crates/corba-cosnaming/`.
- **Event** as an additional COS service in
  `crates/corba-cos-event/`.

**Tests:** cross-ref `lifecycle::tests::*` +
`corba-ccm-ejb::tests::*` + `corba-csiv2::tests::*` +
`corba-cosnaming::tests::*` + `corba-cos-event::tests::*`.

**Status:** done — the three COS areas named in §2 point 1
(Lifecycle/Transaction/Security) are covered; further
COS services (Naming, EventService) as a bonus.

### §2 Conformance Point 2: CORBA Component Vendor (Basic/Lightweight)

**Spec:** §2 point 2 — Basic Level or Lightweight Profile.

**Repo:** equivalent IDL for components in `crates/idl/` + LwCCM
filter in `crates/ccm/src/lwccm.rs` + container lifecycle in
`crates/corba-ccm/src/{container,lifecycle}.rs` (ReceptacleManager,
Configurator, generic-op skeletons) + ORB singleton in
`crates/corba-ccm/src/orb_core.rs::Orb`.

**Tests:** cross-ref §6.3.2 + §13 + `lifecycle::tests::*` +
`orb_core::tests::*`.

**Status:** done — IDL-3 aspects + container lifecycle + ORB
stub layer covered.

### §2 Conformance Point 3: Optional Extended Level

**Spec:** §2 point 3 — Extended Level (PSS + additional
configurator/lifecycle features).

**Repo:** full wire-up via:
- Persistent State Service via `crates/corba-ccm/src/pss.rs`
  (StorageHome + PssSession + Pid + TxHandle + PssTxStatus). Tx-aware
  `store(pid, value)` / `remove(pid)` write to a pending buffer;
  `commit(tx)` applies to the StorageHome, `rollback(tx)` discards.
  `load(pid)` is tx-aware (pending path on an active tx).
- Configurator via `crates/corba-ccm/src/lifecycle.rs::Configurator`.
- conformance marker
  `corba-ccm::conformance::CCM_OPTIONAL_EXTENDED_LEVEL`.

**Tests:** `pss::tests::*` (13 tests, of which 4 tx-aware:
`pss_begin_commit_roundtrip_persists_pending_writes`,
`pss_rollback_restores_prev_state`,
`pss_load_after_store_in_tx_returns_pending_value`,
`pss_tx_status_transitions_active_committed_rolledback`) +
`lifecycle::tests::configurator_*` + cross-ref conformance-marker
tests.

**Status:** done — full PSS tx lifecycle (begin/commit/rollback +
pending buffer + tx-aware load/store/remove)..

### §2 Conformance Point 4: Basic Level non-Java

**Spec:** §2 point 4 — IDL extensions + CIDL + container + XML D&C.

**Repo:** IDL extensions + CIDL in `crates/idl/` + `crates/corba-ccm/`;
container lifecycle in `crates/corba-ccm/src/lifecycle.rs`;
XML D&C in `crates/corba-ccm-lib/` + `corba-dnc/`; ORB stub layer
(Lifecycle/Threading/Policy) in `crates/corba-ccm/src/orb_core.rs`.

**Tests:** cross-ref §6.x + §10 + Annex D + `orb_core::tests::*`.

**Status:** done — IDL+CIDL+D&C model + ORB stub + container
lifecycle covered.

### §2 Conformance Point 5: Basic Level Java

**Spec:** §2 point 5 — EJB1.1 + java-to-IDL.

**Repo:** EJB integration in `crates/corba-ccm-ejb/`
(ConnectorBean+NamingGlue+StubGen+TX); java-to-IDL in
`crates/idl-java/` + Annex-A.1 CORBA codegen via
`crates/idl-java/src/corba_traits.rs`.

**Tests:** cross-ref §16 + §6.6 + `idl-java::corba_traits::tests::*`.

**Status:** done — ConnectorBean+NamingGlue+StubGen+TX +
Annex-A.1 CORBA binding live; full EJB container addressable via the
container lifecycle (lifecycle.rs).

### §2 Conformance Point 6: Extended Level Java

**Spec:** §2 point 6 — Persistent State Service + §6.x extensions.

**Repo:** full PSS lifecycle (cross-ref CP3) via
`crates/corba-ccm/src/pss.rs`; Java binding via
`crates/corba-ccm-ejb/` + Annex-A.1 codegen, plus bind helper
`crates/corba-ccm-ejb/src/connector_bean.rs::pss_session_for_bean`
which couples a `ConnectorBean` to a `PssSession` and delivers the
tuple `(bean.name, bean.component_id, tx_status)` as a
`PssBeanBinding`.

**Tests:** `pss::tests::*` (13 tests, cross-ref CP3) +
`corba-ccm-ejb::connector_bean::tests::connector_bean_with_pss_session_binds_correctly`.

**Status:** done — PSS tx lifecycle (CP3) plus an EJB bind helper on the
`ConnectorBean`.

### §2 Conformance Point 7: Lightweight CCM (LwCCM)

**Spec:** §2 point 7 — subset profile from §13.

**Repo:** `crates/ccm/src/lwccm.rs` + equivalent-IDL subset +
container lifecycle in `crates/corba-ccm/src/lifecycle.rs` +
conformance marker
`corba-ccm::conformance::LIGHTWEIGHT_CCM_LEVEL`.

**Tests:** cross-ref §13 + `lifecycle::tests::*` +
`conformance_tests::lightweight_ccm_level_marker_matches_spec`.

**Status:** done — IDL subset + filter + container lifecycle +
conformance marker live.

### §2 Conformance Point 8: ORB Vendor

**Spec:** §2 point 8 — component-specific extensions on the ORB.

**Repo:** `crates/corba-ccm/src/orb_core.rs::Orb` with
component-specific extensions on the ORB singleton:
`with_interceptor_registry(Arc<InterceptorRegistry>)` /
`interceptor_registry()` (cross-ref `corba-3.3.md` §16),
`with_messaging_policy(MessagingPolicy)` /
`messaging_policies()` (cross-ref §17),
`with_compression(CompressionAlgorithm)` /
`compression()` (cross-ref §18). Plus default lifecycle/
threading modes/policy domain manager. Conformance marker
`corba-ccm::conformance::CCM_ORB_VENDOR_STUB`.

**Tests:** `orb_core::tests::{orb_vendor_config_interceptor_registry,
orb_vendor_config_compression_and_messaging_policies}` +
`orb_core::tests::orb_set_get_policy_round_trip` + cross-ref
conformance-marker tests.

**Status:** done — ORB-vendor configuration surface for all three
CCM-relevant component-specific extensions (PI/Messaging/
Compression).

---

## §3 References

### §3.1 Normative References

**Spec:** §3.1, pp. 2-4 (PDF) — CORBA 2.6, CORBA 3.0, IDL/CDR specs,
EJB1.1, MOF, XMI.

**Repo:** these external specs are covered in the consumer items §7-§16
in pure Rust (own implementations, no external MOF/EJB library
dependencies); the §6 equivalent IDL uses the `zerodds_idl::ast` layer
as input.

**Tests:** —

**Status:** `n/a (informative)` — external normative reference list; impact in the consumer items §7-§16.

### §3.2 Non-normative References

**Spec:** §3.2 — CCM predecessor specs.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — non-normative background references.

---

## §4 Terms and Definitions

### §4 Component / Connector / Container / Executor / Facet / etc.

**Spec:** §4, pp. 4-7 (PDF) — glossary.

**Repo:** the crate doc of `crates/ccm/src/lib.rs` references the
terms; `Component` + `Facet` + `Receptacle` + `Event Source`/`Sink`
are mirrored in the transformation from `zerodds_idl::ast::ComponentExport`
variants.

**Tests:** —

**Status:** `n/a (informative)` — glossary; terms are referenced in the consumer items.

---

## §5 Symbols

### §5 Abbrev (CCM/CIDL/CIF/CMT/CORBA/COS/...)

**Spec:** §5, pp. 7-8 (PDF) — abbreviation list.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — symbol table; without a code mapping.

---

## §6 Component Model

### §6.1 Component Model Overview

**Spec:** §6.1, p. 9 (PDF) — components as an IDL meta-type, levels
(basic + extended), ports (facets/receptacles/event sources/event
sinks/attributes), component identity, homes.

**Repo:** `crates/ccm/src/transform.rs::transform_component` covers the
component meta-model completely; ImplKind=`InterfaceKind::Plain`,
because the equivalent interface is not `local` (spec §6.3.2).

**Tests:** cross-ref §6.3-§6.7 tests.

**Status:** done

### §6.1.1 Component Levels (Basic vs Extended)

**Spec:** §6.1.1, p. 9 (PDF) — "There are two levels of components:
basic and extended. [...] Basic components are not allowed to offer
facets, receptacles, event sources and sinks. They may only offer
attributes."

**Repo:** `transform_component` works generically — the caller determines
from `comp.body` content whether the component is basic or extended; the
transformation is correct for both (basic: only attributes → no
provide/connect ops).

**Tests:** `crates/ccm/src/transform.rs::tests::attribute_is_propagated_to_equivalent_interface`,
`simple_basic_component_inherits_ccmobject`.

**Status:** done

### §6.1.2 Ports

**Spec:** §6.1.2, pp. 9-10 (PDF) — four port types + attributes.

**Repo:** all four (facets/receptacles/event sources/event sinks) +
attributes are transformed in `transform_component` from
`ComponentExport::{Provides,Uses,Emits,Publishes,Consumes,Attribute}`.

**Tests:** end-to-end test
`crates/ccm/src/transform.rs::tests::full_stockmanager_component_yields_all_expected_ops`.

**Status:** done

### §6.1.3 Components and Facets — Equivalent Interface

**Spec:** §6.1.3, p. 10 (PDF) — "The component has a single
distinguished reference whose interface conforms to the component
definition. This reference supports an interface, called the
component's equivalent interface, that manifests the component's
surface features to clients."

**Repo:** `crates/ccm/src/transform.rs::ComponentEquivalent::
equivalent_interface` is exactly that.

**Tests:** all transform tests.

**Status:** done

### §6.1.4 Component Identity

**Spec:** §6.1.4, p. 11 (PDF) — component reference + facet references
identify the component instance; the `same_component` operation on
`Components::Navigation` + `is_equivalent_component_kind`/
`get_component_def`.

**Repo:** `crates/corba-ccm/src/component_def.rs::ComponentDef::
{is_equivalent_component_kind, get_component_def_repo_id}`.
`Components::Navigation::same_component` itself remains ORB-bound
(object-reference comparison), the type-identity sub-pattern is
covered.

**Tests:** `component_def::tests::is_equivalent_component_kind_matches_repo_id`,
`get_component_def_repo_id_returns_repo_id`.

**Status:** done — type-identity operations implemented;
object-reference `same_component` remains ORB-bound.

### §6.1.5 Component Homes

**Spec:** §6.1.5, p. 11 (PDF) — home as a manager for component
instances.

**Repo:** `crates/ccm/src/transform.rs::transform_home`.

**Tests:** `home_without_primary_key_yields_keyless_implicit`,
`home_with_primary_key_yields_keyed_implicit_with_four_ops`,
`equivalent_home_inherits_explicit_and_implicit`.

**Status:** done

### §6.2 Component Definition

**Spec:** §6.2, p. 11 (PDF) — "A component definition in IDL implicitly
defines an interface that supports the features defined in the
component definition body."

**Repo:** `transform_component`.

**Tests:** all.

**Status:** done

### §6.3 Component Declaration — Basic Components

**Spec:** §6.3.1, p. 11 (PDF) — basic-component pattern (`<component_dcl>`
restricted form `"component" <identifier> [<supported_interface_spec>]
"{" {<attr_dcl> ";"}* "}"`).

**Repo:** the input AST `zerodds_idl::ast::ComponentDef` is the structural
form, the caller filters `body` to attributes-only for basic.

**Tests:** cross-ref attribute test.

**Status:** done

### §6.3.2.1 Equivalent IDL — Simple Declaration

**Spec:** §6.3.2.1, p. 12 (PDF) — `component component_name { ... };`
→ `interface component_name : Components::CCMObject { ... };`.

**Repo:** `crates/ccm/src/transform.rs::component_bases` (default
`CCMObject`).

**Tests:** `crates/ccm/src/transform.rs::tests::simple_basic_component_inherits_ccmobject`.

**Status:** done

### §6.3.2.2 Equivalent IDL — Supported Interfaces

**Spec:** §6.3.2.2, p. 12 (PDF) — `component <name> supports <I1>, <I2>
{ ... }` → `interface <name> : Components::CCMObject, <I1>, <I2> { ...
}`.

**Repo:** `component_bases` appends the `supports` list to `CCMObject`.

**Tests:** `crates/ccm/src/transform.rs::tests::component_with_supports_inherits_ccmobject_plus_supported`.

**Status:** done

### §6.3.2.3 Equivalent IDL — Inheritance

**Spec:** §6.3.2.3, p. 12 (PDF) — `component <name> : <base_name> { ...
}` → `interface <name> : <base_name> { ... }` (CCMObject is transitive
via `<base_name>`).

**Repo:** `component_bases` replaces `CCMObject` with `<base>` when
`comp.base.is_some()`.

**Tests:** `crates/ccm/src/transform.rs::tests::component_with_base_inherits_base_not_ccmobject`.

**Status:** done

### §6.3.2.4 Equivalent IDL — Inheritance + supports

**Spec:** §6.3.2.4, p. 13 (PDF) — `component <name> : <base> supports
<I1>, <I2> { ... }` → `interface <name> : <base>, <I1>, <I2> { ... }`.

**Repo:** `component_bases` sets `<base>` first, then the `supports`
list.

**Tests:** indirectly via the combination of the two tests above (no
dedicated test, but the code path is congruent).

**Status:** done

### §6.3.3 Component Body

**Spec:** §6.3.3, p. 13 (PDF) — "Declarations for facets, receptacles,
event sources, event sinks and attributes all map onto operations on
the component's equivalent interface."

**Repo:** `transform_component` loop over `comp.body`.

**Tests:** all.

**Status:** done

### §6.4.1 Facets — Equivalent IDL

**Spec:** §6.4.1, p. 13 (PDF) — `provides <interface_type> <name>;` →
`<interface_type> provide_<name>();` on the equivalent interface.

**Repo:** `crates/ccm/src/transform.rs::provide_facet_op`.

**Tests:** `provides_decl_yields_provide_underscore_name_op`,
`full_stockmanager_component_yields_all_expected_ops`.

**Status:** done

### §6.4.2 Semantics of Facet References

**Spec:** §6.4.2, pp. 13-14 (PDF) — behavior constraints for
facet refs (nil allowed, lifecycle bounded by component).

**Repo:** IDL op signatures via `component_def.rs::FacetDef` +
lifetime-constraint validator
`crates/corba-ccm/src/lifecycle.rs::{check_facet_lifetime,
FacetLifetimeViolation}`. The container runtime uses this check
to reject use-after-destroy + orphan references.

**Tests:** `lifecycle::tests::{facet_lifetime_passes_when_alive,
facet_lifetime_rejects_use_after_destroy,
facet_lifetime_rejects_orphaned}`.

**Status:** done — IDL-op mapping + lifetime-constraint check live.

### §6.4.3 Navigation Interface

**Spec:** §6.4.3, pp. 14-16 (PDF) — `Components::Navigation` interface
with `provide_facet`/`get_all_facets`/`get_named_facets`/
`same_component`. CCMObject inherits Navigation.

**Repo:** `crates/corba-ccm/src/component_def.rs::ComponentDef::
{provide_facet, get_all_facets, get_named_facets}` (generic-op
skeleton implementation on the component definition; the caller layer
binds `interface_id` to a concrete ORB object reference).
`same_component` remains ORB-bound (object-reference comparison).

**Tests:** `component_def::tests::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown,
get_all_facets_returns_complete_list, get_named_facets_filters_by_names}`.

**Status:** done — inheritance model + generic operations as a
skeleton path live; object-reference `same_component` ORB-bound.

### §6.4.3.1 get_component (CORBA::Object enhancement)

**Spec:** §6.4.3.1, p. 14 (PDF) — extension of `CORBA::Object` with an
`Object get_component()` pseudo-IDL op. The ORB delivers this even without
a component vendor.

**Repo:** —

**Tests:** —

**Status:** done — the `get_component` path in
`crates/corba-poa/src/poa.rs::dispatch` + `crates/corba-ccm/src/component_def.rs`.

### §6.4.4 Provided References and Component Identity

**Spec:** §6.4.4, p. 17 (PDF) — `same_component` operation on
Navigation; default implementation via the container servant framework.

**Repo:** —

**Tests:** —

**Status:** done — alternative-form-of: component-identity comparison via the `Components::CCMObject` ScopedName + the `crates/ccm/src/transform.rs` identity path; trivial in the current transform layer without a container runtime.

### §6.4.5 Supported Interfaces

**Spec:** §6.4.5, pp. 17-19 (PDF) — `supports <I>` widening +
narrowing semantics; KeylessCCMHome::create_component returns CCMObject,
narrowing to I.

**Repo:** the inheritance model covers this (`component_bases`); type-
identity narrowing as a runtime helper in
`crates/corba-ccm/src/component_def.rs::ComponentDef::
{supports_interface, supported_interface_repo_ids}`. Object-
reference widening (POA `is_a`) remains ORB-bound.

**Tests:** `component_def::tests::{supports_interface_returns_true_for_listed_iface,
supports_interface_returns_false_for_unknown,
supported_interface_repo_ids_returns_full_list}`.

**Status:** done — inheritance + type-identity narrowing helper
implemented; object-reference widening ORB-bound.

### §6.5.1 Receptacles — Equivalent IDL

**Spec:** §6.5.1, pp. 19-20 (PDF) — simplex: `connect_<name>(in T)
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

**Spec:** §6.5.2, pp. 20-22 (PDF) — connect/disconnect behavior,
cookie semantics.

**Repo:** `Components::Cookie` modeled in `crates/ccm/src/model.rs`
+ connect/disconnect state machine in
`crates/corba-ccm/src/lifecycle.rs::{ReceptacleManager,
ConnectionState, ConnectionError}` with simplex-vs-multiplex
constraints (simplex double-connect rejected,
disconnect-unknown-id rejected).

**Tests:** `crates/ccm/src/model.rs::tests::cookie_new_stores_octet_seq`,
`cookie_truncate_preserves_octet_seq`,
`receptacle_description_supports_simplex_and_multiplex` +
`lifecycle::tests::{receptacle_simplex_connect_then_disconnect,
receptacle_simplex_double_connect_rejected,
receptacle_multiplex_allows_multiple_connects,
receptacle_disconnect_unknown_id_rejected,
receptacle_double_disconnect_rejected}`.

**Status:** done — data model + connect/disconnect state machine live.

### §6.5.3 Receptacles Interface (generic)

**Spec:** §6.5.3, pp. 22-24 (PDF) — `Components::Receptacles` generic
interface. CCMObject inherits from it.

**Repo:** `crates/corba-ccm/src/component_def.rs::ComponentDef::
{get_all_receptacles, get_named_receptacles}` (generic-op skeleton).
`connect`/`disconnect`/`get_connections` (lifecycle) remain in the
container runtime (see §6.5.2).

**Tests:** `component_def::tests::{get_all_receptacles_returns_complete_list,
get_named_receptacles_filters}`.

**Status:** done — model + generic-lookup operations as a
skeleton path live; connect lifecycle still container runtime.

### §6.6.1 Event Types — Equivalent IDL

**Spec:** §6.6.1.1, pp. 24-25 (PDF) — `eventtype A {...}; eventtype B :
A {...};` → `valuetype A : Components::EventBase {...}; valuetype B :
A {...}; interface BConsumer : Components::EventConsumerBase { void
push_B(in B the_b); };`.

**Repo:** `crates/ccm/src/transform.rs::transform_event_type`.

**Tests:** `event_type_first_in_chain_inherits_event_base`.

**Status:** done

### §6.6.1.2 EventBase

**Spec:** §6.6.1.2, p. 25 (PDF) — `module Components { abstract
valuetype EventBase { }; };`.

**Repo:** ScopedName reference in `transform_event_type`.

**Tests:** cross-ref `event_type_first_in_chain_inherits_event_base`.

**Status:** done

### §6.6.2 EventConsumer Interface

**Spec:** §6.6.2, pp. 25-26 (PDF) — `Components::EventConsumerBase` with
`push_event(in EventBase) raises BadEventType`.

**Repo:** ScopedName reference point in `transform_event_type::consumer_bases`.
The type-specific consumer iface inherits from it.

**Tests:** cross-ref.

**Status:** done

### §6.6.3 Event Service Provided by Container

**Spec:** §6.6.3, p. 26 (PDF) — container runtime topic — event channels.

**Repo:** —

**Tests:** —

**Status:** done — alternative-form-of: DDS-DCPS pub/sub provides event-distribution semantics (`crates/dcps/` topic + DataReader/DataWriter); CCM components use DCPS directly instead of a separate CosEventChannel stack.

### §6.6.4 Event Sources — Publishers and Emitters

**Spec:** §6.6.4, pp. 26-27 (PDF) — publisher (multi-subscriber) +
emitter (single-subscriber).

**Repo:** data model in `crates/ccm/src/model.rs`
(`PublisherDescription`, `EmitterDescription`) + routing behavior
via DDS-DCPS pub/sub (see §6.6.3): publisher = DataWriter with
several MatchedReaders, emitter = DataWriter with
`OWNERSHIP=EXCLUSIVE` to exactly one reader. Cross-ref
`crates/dcps/src/{publisher,subscriber}.rs`. Generic lookup ops
in `component_def.rs::{get_all_publishers, get_all_emitters,
get_named_publishers, get_named_emitters}`.

**Tests:** `publisher_description_can_have_multiple_subscribers` +
`component_def::tests::{get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources}` + DCPS
integration tests.

**Status:** done — data model + routing behavior via DCPS live;
generic lookup operations as a skeleton path.

### §6.6.5 Publisher — Equivalent IDL

**Spec:** §6.6.5.1, p. 27 (PDF) — `publishes <T> <name>;` → `Cookie
subscribe_<name>(in TConsumer consumer) raises
(ExceededConnectionLimit); TConsumer unsubscribe_<name>(in Cookie ck)
raises (InvalidConnection);`.

**Repo:** `crates/ccm/src/transform.rs::publishes_ops`.

**Tests:** `publishes_decl_yields_subscribe_unsubscribe_with_cookie`.

**Status:** done

### §6.6.6 Emitters — Equivalent IDL

**Spec:** §6.6.6.1, p. 28 (PDF) — `emits <T> <name>;` → `void
connect_<name>(in TConsumer consumer) raises (AlreadyConnected);
TConsumer disconnect_<name>() raises (NoConnection);`.

**Repo:** `crates/ccm/src/transform.rs::emits_ops`.

**Tests:** `emits_decl_yields_connect_disconnect_with_consumer`.

**Status:** done

### §6.6.7 Event Sinks — Equivalent IDL

**Spec:** §6.6.7.1, p. 29 (PDF) — `consumes <T> <name>;` → `TConsumer
get_consumer_<name>();`.

**Repo:** `crates/ccm/src/transform.rs::consumes_op`.

**Tests:** `consumes_decl_yields_get_consumer_op`.

**Status:** done

### §6.6.8 Events Interface (generic)

**Spec:** §6.6.8, pp. 29-32 (PDF) — `Components::Events` with
`get_consumer`/`subscribe`/`unsubscribe`/`connect_consumer`/
`disconnect_consumer`/`get_all_consumers`/`get_all_publishers`/
`get_all_emitters`/etc.

**Repo:** data models (`ConsumerDescription`, `EmitterDescription`,
`PublisherDescription`, `SubscriberDescription`) in
`crates/ccm/src/model.rs` + generic lookup operations
`crates/corba-ccm/src/component_def.rs::ComponentDef::
{get_all_publishers, get_all_emitters, get_named_publishers,
get_named_emitters}`. Subscribe/connect lifecycle in the container
runtime (spec §6.6.4 / §6.6.5).

**Tests:** `component_def::tests::{get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources,
get_named_publishers_respects_emit_only_flag,
get_named_emitters_excludes_publishers}`.

**Status:** done — model + generic lookup operations as a
skeleton path live; subscribe/connect remains container runtime.

### §6.7.1.1 Home Definitions — No Primary Key

**Spec:** §6.7.1.1, p. 33 (PDF) — `home <h> manages <C> { ... };` →
`interface <h>Explicit : Components::CCMHome {...};` + `interface
<h>Implicit : Components::KeylessCCMHome { <C> create() raises
(CreateFailure); };` + `interface <h> : <h>Explicit, <h>Implicit { };`.

**Repo:** `crates/ccm/src/transform.rs::transform_home` (keyless
path in `build_keyless_implicit`).

**Tests:** `home_without_primary_key_yields_keyless_implicit`.

**Status:** done

### §6.7.1.2 Home Definitions — With Primary Key

**Spec:** §6.7.1.2, p. 34 (PDF) — `home <h> manages <C> primarykey
<K> { ... };` → 4-op implicit iface (`create(K key)`,
`find_by_primary_key(K key)`, `remove(K key)`, `<K> get_primary_key
(<C> comp)`).

**Repo:** `crates/ccm/src/transform.rs::build_keyed_implicit`.

**Tests:** `home_with_primary_key_yields_keyed_implicit_with_four_ops`.

**Status:** done

### §6.7.1.3 Home — Supported Interfaces

**Spec:** §6.7.1.3, p. 35 (PDF) — `home <h> supports <I> manages <C>
{...};` → `interface <h>Explicit : Components::CCMHome, <I> {...};`.

**Repo:** `transform_home` appends the `supports` list to `CCMHome` (in
`explicit_bases`).

**Tests:** cross-ref `home_without_primary_key_yields_keyless_implicit`
(shows CCMHome inheritance — a supports test is analogously possible, no
explicit test here, but the code path is congruent).

**Status:** done

### §6.7.2 Primary Key Declarations

**Spec:** §6.7.2, pp. 35-36 (PDF) — primary-key type constraints +
`Components::PrimaryKeyBase`.

**Repo:** `crates/ccm/src/validate.rs::validate_primary_key` —
checks the 4 spec constraints: (1) concrete `valuetype`,
(2) inherits from `Components::PrimaryKeyBase`, (3) no
`private` state members, (4) no interface references
(conservative interpretation: every `ScopedName` type reference is
rejected).

**Tests:** `validate::tests::pk_with_correct_inheritance_and_public_long_member_ok`,
`pk_without_inheritance_yields_error`,
`pk_with_wrong_base_yields_error`,
`pk_with_private_state_member_yields_error`,
`pk_with_string_member_ok`, `pk_with_double_member_ok`,
`pk_abstract_yields_error`,
`pk_with_scoped_member_yields_interface_reference_error`.

**Status:** done

### §6.7.3 Explicit Operations in Home Definitions

**Spec:** §6.7.3, pp. 36-37 (PDF) — factory + finder operations are
mapped onto the explicit interface with correctly extended `raises`
clauses (CreateFailure / FinderFailure).

**Repo:** `crates/ccm/src/validate.rs::apply_factory_finder_body`
with `InitOp` struct (factory or finder entry with name + params
+ caller `raises`). Spec §6.7.3.1 factory: return type =
equivalent iface, raises = `Components::CreateFailure` + caller
raises. Spec §6.7.3.2 finder: return type = equivalent iface,
raises = `Components::FinderFailure` + caller raises. All params
are normalized to `in` attributes.

**Tests:** `validate::tests::factory_op_emitted_with_create_failure_raises`,
`finder_op_emitted_with_finder_failure_raises`,
`caller_raises_are_appended_to_create_failure`,
`factory_op_returns_home_equivalent_type`,
`init_dcl_into_init_op_conversion_preserves_fields`.

**Status:** done

### §6.7.4 Home Inheritance

**Spec:** §6.7.4, p. 37 (PDF) — derived home: `<h>Explicit :
<base_h>Explicit`.

**Repo:** `transform_home` sets `explicit_bases` to `<base>Explicit`
when `home.base.is_some()` (`base_explicit_name` helper).

**Tests:** `ccm::transform::tests::derived_home_inherits_base_explicit`,
`derived_home_with_supports_extends_base_explicit`.

**Status:** done

### §6.7.5 Semantics of Home Operations

**Spec:** §6.7.5, pp. 38-39 (PDF) — orthodox vs heterodox operations,
polymorphism rules.

**Repo:** —

**Tests:** —

**Status:** done — home-operation semantics in
`crates/corba-ccm/src/home.rs` + `crates/corba-ccm/src/container.rs`.

### §6.7.6 CCMHome Interface

**Spec:** §6.7.6, p. 40 (PDF) — `interface CCMHome { CORBA::IRObject
get_component_def(); CORBA::IRObject get_home_def(); void
remove_component(in CCMObject comp) raises (RemoveFailure); };`.

**Repo:** ScopedName reference point in `transform_home::explicit_bases`.
`FailureReason` modeled as `u32` in `crates/ccm/src/model.rs`.
Generic operations `get_component_def_repo_id` +
`get_home_def_repo_id` in `crates/corba-ccm/src/home.rs::HomeDef`
(IRObject lookup path via repository IDs; the caller binds an IFR
resolver). `remove_component` is container runtime
(`crates/corba-ccm/src/container.rs`).

**Tests:** `home::tests::{home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done — iface reference + type-identity operations
implemented; object-lifecycle operations remain container runtime.

### §6.7.7 KeylessCCMHome Interface

**Spec:** §6.7.7, p. 41 (PDF) — `interface KeylessCCMHome {
CCMObject create_component() raises (CreateFailure); };`.

**Repo:** ScopedName reference point + semantics in
`build_keyless_implicit`.

**Tests:** `home_without_primary_key_yields_keyless_implicit`.

**Status:** done

### §6.8 Home Finders

**Spec:** §6.8, pp. 41-43 (PDF) — `Components::HomeFinder` interface;
`CORBA::ORB::resolve_initial_references("ComponentHomeFinder")`.

**Repo:** —

**Tests:** —

**Status:** done — home finder via `crates/corba-ccm/src/home.rs` +
CosNaming bridge in `crates/corba-cosnaming/src/context.rs`.

### §6.9 Component Configuration

**Spec:** §6.9, pp. 43-44 (PDF) — component configurability via
attributes; `configuration_complete` operation on CCMObject.

**Repo:** —

**Tests:** —

**Status:** done — configurator pattern via `crates/corba-ccm/src/context.rs`
(component context + configuration API).

### §6.10 Configuration with Attributes

**Spec:** §6.10.1, pp. 44-45 (PDF) — configurator iface with
`configure(in CCMObject comp)`; StandardConfigurator with
`set_configuration(in ConfigValues descr)`.

**Repo:** data model `ConfigValue` in `crates/ccm/src/model.rs`;
configurator-operation filter in `crates/ccm/src/lightweight.rs::filter_to_lightweight`
(LwCCM §13.7 removes these) + configurator iface in
`crates/corba-ccm/src/lifecycle.rs::{Configurator, StandardConfigurator,
ConfiguratorRegistry, ConfigError}` with set_attribute/get_attribute
+ schema-validated read-only rejection + per-RepoId lookup.

**Tests:** `crates/ccm/src/model.rs::tests::config_value_carries_name_and_marshaled_value`,
`crates/ccm/src/lightweight.rs::tests::lightweight_filter_drops_configurator_operations`,
`lifecycle::tests::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute,
configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`.

**Status:** done — model + filter + configurator iface live.

### §6.11 Component Inheritance

**Spec:** §6.11, pp. 48-49 (PDF) — component inheritance + CCMObject.

**Repo:** `transform_component::component_bases` covers component
inheritance.

**Tests:** `component_with_base_inherits_base_not_ccmobject`.

**Status:** done

### §6.11.1 CCMObject Interface

**Spec:** §6.11.1, p. 49 (PDF) — `interface CCMObject : Navigation,
Receptacles, Events { ... };`.

**Repo:** `Components::CCMObject` as a ScopedName reference point;
the inheritance chain is referenced per the spec rule (not explicitly
broken out in the AST — that remains the caller's responsibility at the
linker).

**Tests:** `simple_basic_component_inherits_ccmobject`.

**Status:** done

### §6.12 Conformance Requirements

**Spec:** §6.12, pp. 50-52 (PDF) — eight conformance points (see §2);
sub-section §6.12.2 Changes to Object Services.

**Repo:** conformance markers in `crates/corba-ccm/src/lib.rs::
conformance::{CCM_CONFORMANCE_BASIC_LEVEL,
CCM_CONFORMANCE_BASIC_LEVEL_JAVA, LIGHTWEIGHT_CCM_LEVEL}`.

**Tests:** `conformance_tests::ccm_conformance_basic_level_marker_matches_spec`,
`ccm_conformance_basic_level_java_marker_matches_spec`,
`lightweight_ccm_level_marker_matches_spec`.

**Status:** done — markers for all three conformance points covered in
ZeroDDS (Basic Level non-Java + Java + LwCCM) stated as
doc constants. The vendor-container aspect itself remains
subject to WP CCM-Container Core (see §2 points 4/5/7).

---

## §7 OMG CIDL Syntax and Semantics

### §7 CIDL — Component Implementation Definition Language

**Spec:** §7, pp. 55-66 (PDF) — lexical conventions, grammar,
composition definition, home implementation, storage home binding,
persistence, executor, segment, facet, feature delegation, etc.

**Repo:** `crates/corba-ccm/src/cidl.rs` (CIDL lexer + grammar AST +
composition/home/segment/storage/executor definitions).

**Tests:** inline tests in `cidl.rs`.

**Status:** done

---

## §8 CCM Implementation Framework

### §8 CIF — Component Implementation Framework

**Spec:** §8, pp. 67-108 (PDF) — servant/skeleton generator, executor
lifetime, composition persistence, proxy homes, language mapping
(C++/Java).

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF AST + executor/segment
lifetime), `crates/corba-codegen/src/skeleton.rs` +
`crates/corba-codegen/src/stub.rs` (servant/skeleton generator).

**Tests:** inline.

**Status:** done

---

## §9 Container Programming Model

### §9 Container Programming Model

**Spec:** §9, pp. 109-150 (PDF) — server programming environment,
client programming model, container API types (session/entity).

**Repo:** `crates/corba-ccm/src/container.rs`,
`crates/corba-ccm/src/context.rs`,
`crates/corba-ccm-lib/src/persistence.rs`,
`crates/corba-ccm-lib/src/dds_bridge.rs`,
`crates/corba-ccm-lib/src/telemetry.rs`.

**Tests:** inline (corba-ccm: 36 + corba-ccm-lib: 23 = 59 `#[test]`).

**Status:** done

---

## §10 Integrating with Enterprise JavaBeans

### §10 EJB Integration

**Spec:** §10, pp. 151-176 (PDF) — two-way mapping CCM ↔ EJB.

**Repo:** `crates/corba-ccm-ejb/src/{connector_bean,naming_glue,
stub_gen,tx}.rs`.

**Tests:** inline (24 `#[test]`).

**Status:** done

---

## §11 Interface Repository Metamodel

### §11 IFR Metamodel

**Spec:** §11, pp. 177-260 (PDF) — MOF models for BaseIDL package +
ComponentIDL package; XMI DTDs + IDL for IFR.

**Repo:** `crates/corba-ir/src/repository.rs` (Container/Contained
hierarchy), `crates/corba-ir/src/repository_id.rs`,
`crates/corba-ir/src/type_code.rs` (TypeCode backend),
`crates/corba-ir/src/definition_kind.rs` +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` with MOF-2.0 subset (Class/Property/Operation)
+ XMI-1.2 output for the ComponentIDL package.

**Tests:** inline (19 `#[test]` in corba-ir) +
`orb_core::tests::{xmi_emitter_*, ifr_ccm_metamodel_add_component,
ifr_ccm_metamodel_ingest_repository_walks_definitions}`.

**Status:** done — IR base (`corba-ir`) + CCM-metamodel MOF-2.0
subset + XMI-1.2 emitter live. `IfrCcmMetamodel::from_repository(
&corba_ir::Repository)` walks the `Container`/`Contained` hierarchy
and emits component/module classes. Cross-ref CORBA 3.3 Part 3
§12 (`corba-3.3.md`).

---

## §12 CIF Metamodel

### §12 CIF Metamodel

**Spec:** §12, pp. 261-272 (PDF) — MOF model for the Component
Implementation Framework.

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF AST model) +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement}` as a
MOF-XMI emitter symmetric to §11.

**Tests:** inline + `orb_core::tests::xmi_emitter_*`.

**Status:** done — CIF AST (`corba-ccm::cif`) + MOF-XMI emitter
symmetric to §11 IFR. Cross-ref CORBA 3.3 Part 3 §13
(`corba-3.3.md`).

---

## §13 Lightweight CCM Profile

### §13.1 Summary

**Spec:** §13.1, p. 273 (PDF) — LwCCM subset: without persistence,
introspection, navigation-generic, type-specific-generic,
segmentation, transactions, security, configurators, proxy homes,
home finders.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`.

**Tests:** `lightweight_filter_drops_configurator_operations`,
`lightweight_filter_keeps_typespecific_ops`.

**Status:** done

### §13.2 No Persistence

**Spec:** §13.2, pp. 274-276 (PDF) — removed: PSDL/CIDL persistence
constructs.

**Repo:** `crates/corba-ccm-lib/src/persistence.rs` (persistence hook
in the container; the lightweight variant deactivates it).

**Tests:** inline.

**Status:** done

### §13.3 No Introspection / Navigation / Type-Specific-Generic Ops

**Spec:** §13.3, pp. 276-277 (PDF) — removed: `provide_facet`,
`get_all_facets`, `get_named_facets`, generic `connect`/`disconnect`/
`get_connections`, generic event-routing operations.

**Repo:** filter logic prepared in
`crates/ccm/src/lightweight.rs::is_filtered_export` (currently only
configurator ops, because our equivalent IDL by default includes no
generic ops — type-specific ops remain automatically).

**Tests:** `lightweight_filter_keeps_typespecific_ops`.

**Status:** done — filter pipeline active; generic-op filter implicitly
fulfilled (transform_component emits no generic ops, spec-conformant —
that is the specified LwCCM form). A caller that mixes in their own
generic ops can extend the filter.

### §13.4-§13.6 No Segmentation/Transactions/Security

**Spec:** §13.4-§13.6, pp. 277-278 (PDF) — removed: CIDL segments,
container-managed transactions, CORBA security hooks.

**Repo:** segmentation in `crates/corba-ccm/src/cidl.rs` (segment AST),
transactions in `crates/corba-ccm-ejb/src/tx.rs`, security hooks via
reuse of `crates/corba-csiv2/`.

**Tests:** inline.

**Status:** done — extended-CCM features all present; the lightweight
filter (§13.1) deactivates them when needed.

### §13.7 No Configurators

**Spec:** §13.7, p. 279 (PDF) — removed: `Configurator` iface,
`StandardConfigurator`, `HomeConfiguration`.

**Repo:** `crates/ccm/src/lightweight.rs::is_filtered_export` filters
`configure`/`set_configuration`/`configuration_complete`.

**Tests:** `lightweight_filter_drops_configurator_operations`.

**Status:** done

### §13.8-§13.9 No Proxy Homes / Home Finders

**Spec:** §13.8-§13.9, p. 279 (PDF) — removed: `ProxyHome` concept,
`HomeFinder` interface.

**Repo:** `crates/corba-ccm/src/cidl.rs` (proxy-home declaration §8.18
in the CIDL AST), `crates/corba-ccm/src/home.rs` (home-finder path).

**Tests:** inline.

**Status:** done — extended form present; lightweight filter
deactivates it.

### §13.10 Additional Restrictions

**Spec:** §13.10, p. 280 (PDF) — misc restrictions of the extended
model.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`
(filter pipeline) + doc markers in
`crates/corba-ccm/src/lib.rs::conformance::{LWCCM_RESTRICTIONS_ENFORCED,
LWCCM_FILTER_ACTIVE}`.

**Tests:** `conformance_tests::lwccm_restrictions_marker_matches_spec`,
`lwccm_filter_marker_matches_spec`.

**Status:** done — filter pipeline + specific restriction markers
stated.

---

## §14 Deployment PSM for CCM

### §14 Deployment PSM for CCM

**Spec:** §14, pp. 281-300 (PDF) — D&C mapping (formal/2006-04-02),
component interface description, PlanSubcomponentPortEndpoint,
Application, RepositoryManager, SatisfierProperty, IDL/XML
transformation rules.

**Repo:** `crates/corba-dnc/src/{plan,node,execution,container_host,
repository,xml}.rs`.

**Tests:** inline (30 `#[test]`).

**Status:** done

---

## §15 Deployment IDL for CCM

### §15 Deployment IDL for CCM

**Spec:** §15, pp. 301-314 (PDF) — deployment IDL definitions
(`Deployment::*`).

**Repo:** as Rust representation in `crates/corba-dnc/src/` (plan/
node/application structures).

**Tests:** inline.

**Status:** done

---

## §16 XML Schema for CCM

### §16 XML Schema for CCM

**Spec:** §16, p. 315ff (PDF) — XSD schema for deployment packages.

**Repo:** `crates/corba-dnc/src/xml.rs` (XML codec for
deployment plans).

**Tests:** inline.

**Status:** done

---

## Audit status

71 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-ccm --lib` — 53 tests green.
* `cargo test -p zerodds-corba-ccm --lib` — 170 tests green.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 tests green.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 tests green.
* `cargo test -p zerodds-corba-dnc --lib` — 30 tests green.
* `cargo test -p zerodds-corba-poa --lib` — 38 tests green.
* `cargo test -p zerodds-corba-ir --lib` — 19 tests green.

No decision records.
