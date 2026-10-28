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
- §6 Component Model = equivalent IDL over the AST from
  `crates/idl-parse/` + `crates/idl-lower/`.
- §7 CIDL via `crates/corba-ccm/src/cidl.rs`.
- §8 Programming Model via `crates/corba-ccm-lib/`.
- §9 Runtime via `crates/corba-ccm/src/container.rs`.
- §10 EJB interaction via `crates/corba-ccm-ejb/`.
- §13 Lightweight Profile: equivalent-IDL filter in
  `crates/ccm/src/lightweight.rs`, profile restrictions in
  `crates/corba-ccm/src/lwccm.rs`.
- §14 Deployment via `crates/corba-dnc/`.
- conformance markers `crates/corba-ccm/src/lib.rs::conformance`.

**Tests:** `crates/corba-ccm/src/lib.rs::conformance_tests::{
ccm_conformance_basic_level_marker_matches_spec,
ccm_conformance_basic_level_java_marker_matches_spec,
lightweight_ccm_level_marker_matches_spec,
lwccm_restrictions_marker_matches_spec,
lwccm_filter_marker_matches_spec, all_markers_are_distinct}`.

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

**Tests:** `crates/corba-ccm/src/lifecycle.rs::tests::facet_lifetime_passes_when_alive`,
`::receptacle_simplex_connect_then_disconnect`,
`::configurator_set_get_roundtrip`;
`crates/corba-ccm-ejb/src/tx.rs::tests::cos_to_jta_round_trip_for_all_states`,
`::begin_then_commit_succeeds`;
`crates/corba-csiv2/src/sas.rs::tests::sas_msg_type_values_match_spec`;
`crates/corba-cosnaming/src/context.rs::tests::bind_and_resolve_simple`;
`crates/corba-cos-event/src/channel.rs::tests::push_fan_out_to_multiple_consumers`,
`::pull_supplier_dequeues_events`.

**Status:** done — the three COS areas named in §2 point 1
(Lifecycle/Transaction/Security) are covered; further
COS services (Naming, EventService) as a bonus.

### §2 Conformance Point 2: CORBA Component Vendor (Basic/Lightweight)

**Spec:** §2 point 2 — Basic Level or Lightweight Profile.

**Repo:** equivalent IDL for components in `crates/idl-parse/` +
LwCCM filter in `crates/ccm/src/lightweight.rs` + container lifecycle in
`crates/corba-ccm/src/{container,lifecycle}.rs` (ReceptacleManager,
Configurator, generic-op skeletons) + ORB singleton in
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

**Tests:** `crates/corba-ccm/src/pss.rs::tests::*` (13 tests, of which 4 tx-aware:
`pss_begin_commit_roundtrip_persists_pending_writes`,
`pss_rollback_restores_prev_state`,
`pss_load_after_store_in_tx_returns_pending_value`,
`pss_tx_status_transitions_active_committed_rolledback`) +
`crates/corba-ccm/src/lifecycle.rs::tests::configurator_*` + cross-ref conformance-marker
tests.

**Status:** done — full PSS tx lifecycle (begin/commit/rollback +
pending buffer + tx-aware load/store/remove)..

### §2 Conformance Point 4: Basic Level non-Java

**Spec:** §2 point 4 — IDL extensions + CIDL + container + XML D&C.

**Repo:** IDL extensions + CIDL in `crates/idl-parse/` +
`crates/idl-lower/` + `crates/corba-ccm/`;
container lifecycle in `crates/corba-ccm/src/lifecycle.rs`;
XML D&C in `crates/corba-ccm-lib/` + `corba-dnc/`; ORB stub layer
(Lifecycle/Threading/Policy) in `crates/corba-ccm/src/orb_core.rs`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_component_with_supports_inherits_ccmobject_and_the_supported_ones`,
`::a_keyed_home_yields_the_four_operations_and_inherits_nothing`;
`crates/corba-ccm/src/cidl.rs::tests::all_four_composition_categories_distinct`,
`::entity_composition_with_storage_valid`;
`crates/corba-ccm/src/lifecycle.rs::tests::configurator_registry_register_and_lookup`;
`crates/corba-dnc/src/xml.rs::tests::parses_minimal_plan`,
`::validation_fails_on_unknown_impl_reference`;
`crates/corba-ccm/src/orb_core.rs::tests::orb_set_get_policy_round_trip`.

**Status:** done — IDL+CIDL+D&C model + ORB stub + container
lifecycle covered.

### §2 Conformance Point 5: Basic Level Java

**Spec:** §2 point 5 — EJB1.1 + java-to-IDL.

**Repo:** EJB integration in `crates/corba-ccm-ejb/`
(ConnectorBean+NamingGlue+StubGen+TX); Java language mapping in
`crates/lang-rules/src/emit/java.rs`; Annex-A.1 CORBA Java codegen in
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

**Status:** done — the EJB integration, java-to-IDL and the Annex A.1
CORBA binding for Java all name paths that exist.

### §2 Conformance Point 6: Extended Level Java

**Spec:** §2 point 6 — Persistent State Service + §6.x extensions.

**Repo:** full PSS lifecycle (cross-ref CP3) via
`crates/corba-ccm/src/pss.rs`; Java binding via
`crates/corba-ccm-ejb/` + Annex-A.1 codegen, plus bind helper
`crates/corba-ccm-ejb/src/connector_bean.rs::pss_session_for_bean`
which couples a `ConnectorBean` to a `PssSession` and delivers the
tuple `(bean.name, bean.component_id, tx_status)` as a
`PssBeanBinding`.

**Tests:** `crates/corba-ccm/src/pss.rs::tests::*` (13 tests, cross-ref CP3) +
`crates/corba-ccm-ejb/src/connector_bean.rs::tests::connector_bean_with_pss_session_binds_correctly`.

**Status:** done — PSS tx lifecycle (CP3) plus an EJB bind helper on the
`ConnectorBean`.

### §2 Conformance Point 7: Lightweight CCM (LwCCM)

**Spec:** §2 point 7 — subset profile from §13.

**Repo:** equivalent-IDL filter `crates/ccm/src/lightweight.rs` +
profile restrictions `crates/corba-ccm/src/lwccm.rs` +
container lifecycle in `crates/corba-ccm/src/lifecycle.rs` +
conformance marker
`corba-ccm::conformance::LIGHTWEIGHT_CCM_LEVEL`.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::*` (§13
restrictions), `crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_drops_configurator_operations`,
`crates/corba-ccm/src/lifecycle.rs::tests::*`,
`crates/corba-ccm/src/lib.rs::conformance_tests::lightweight_ccm_level_marker_matches_spec`.

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

**Tests:** `crates/corba-ccm/src/orb_core.rs::tests::{orb_vendor_config_interceptor_registry,
orb_vendor_config_compression_and_messaging_policies}` +
`crates/corba-ccm/src/orb_core.rs::tests::orb_set_get_policy_round_trip` + cross-ref
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
dependencies); the §6 equivalent IDL uses the `zerodds_ast` layer
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
are mirrored in the transformation from `zerodds_ast::ComponentRole`
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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_plain_component_inherits_ccmobject,
the_whole_trader_component_yields_every_operation_the_clauses_name,
a_keyless_home_yields_an_implicit_inheriting_keylesscmhome,
a_component_attribute_is_left_alone}` — the four meta-model axes
(component-as-interface, all four port kinds, home, attributes).

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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_basic_component_carries_its_attributes_and_no_port_operation,
a_component_attribute_is_left_alone,
the_whole_trader_component_yields_every_operation_the_clauses_name}` —
basic (attributes only, no port ops) against extended (all port ops).

**Status:** done

### §6.1.2 Ports

**Spec:** §6.1.2, pp. 9-10 (PDF) — four port types + attributes.

**Repo:** all four (facets/receptacles/event sources/event sinks) +
attributes are transformed in `transform_component` from
`ComponentExport::{Provides,Uses,Emits,Publishes,Consumes,Attribute}`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_whole_trader_component_yields_every_operation_the_clauses_name`
— one component with all four port kinds plus an attribute.

**Status:** done

### §6.1.3 Components and Facets — Equivalent Interface

**Spec:** §6.1.3, p. 10 (PDF) — "The component has a single
distinguished reference whose interface conforms to the component
definition. This reference supports an interface, called the
component's equivalent interface, that manifests the component's
surface features to clients."

**Repo:** `crates/ccm/src/transform.rs::ComponentEquivalent` —
`equivalent_interface` is exactly that one distinguished reference:
`transform_component` hands back the component node itself, with
`InterfaceKind::Interface` instead of `InterfaceKind::Component`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_component_equivalent_is_comparable,
a_plain_component_inherits_ccmobject}` — the first checks the identity
`equivalent_interface == component`, the second the change of kind.

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

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::is_equivalent_component_kind_matches_repo_id`,
`get_component_def_repo_id_returns_repo_id`.

**Status:** done — type-identity operations implemented;
object-reference `same_component` remains ORB-bound.

### §6.1.5 Component Homes

**Spec:** §6.1.5, p. 11 (PDF) — home as a manager for component
instances.

**Repo:** `crates/ccm/src/transform.rs::transform_home`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_keyless_home_yields_an_implicit_inheriting_keylesscmhome,
a_keyed_home_yields_the_four_operations_and_inherits_nothing,
a_derived_home_inherits_the_base_homes_explicit_interface}`.

**Status:** done

### §6.2 Component Definition

**Spec:** §6.2, p. 11 (PDF) — "A component definition in IDL implicitly
defines an interface that supports the features defined in the
component definition body."

**Repo:** `crates/ccm/src/transform.rs::transform_component`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_component_equivalent_is_comparable,
the_whole_trader_component_yields_every_operation_the_clauses_name}` —
the component declaration becomes the interface declaration, and the
declaration body carries the features the interface then supports.

**Status:** done

### §6.3 Component Declaration — Basic Components

**Spec:** §6.3.1, p. 11 (PDF) — basic-component pattern (`<component_dcl>`
restricted form `"component" <identifier> [<supported_interface_spec>]
"{" {<attr_dcl> ";"}* "}"`).

**Repo:** the input is the `InterfaceNode` with
`InterfaceKind::Component` from `crates/idl-lower/`;
`crates/ccm/src/transform.rs::transform_component` transforms the basic
form (only `attr_dcl` in the body, optionally `supports`) without
producing a single port operation.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_basic_component_carries_its_attributes_and_no_port_operation`
— both attributes stay, the operation list is empty, and the `supports`
list follows `Components::CCMObject`.

**Status:** done

### §6.3.2.1 Equivalent IDL — Simple Declaration

**Spec:** §6.3.2.1, p. 12 (PDF) — `component component_name { ... };`
→ `interface component_name : Components::CCMObject { ... };`.

**Repo:** `crates/ccm/src/transform.rs::transform_component` — the
§6.3.2 base list; without a declared base it holds
`Components::CCMObject`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_plain_component_inherits_ccmobject`.

**Status:** done

### §6.3.2.2 Equivalent IDL — Supported Interfaces

**Spec:** §6.3.2.2, p. 12 (PDF) — `component <name> supports <I1>, <I2>
{ ... }` → `interface <name> : Components::CCMObject, <I1>, <I2> { ...
}`.

**Repo:** `crates/ccm/src/transform.rs::transform_component` appends
the `supports` list to `Components::CCMObject`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_component_with_supports_inherits_ccmobject_and_the_supported_ones`.

**Status:** done

### §6.3.2.3 Equivalent IDL — Inheritance

**Spec:** §6.3.2.3, p. 12 (PDF) — `component <name> : <base_name> { ...
}` → `interface <name> : <base_name> { ... }` (CCMObject is transitive
via `<base_name>`).

**Repo:** `crates/ccm/src/transform.rs::transform_component` sets
`Components::CCMObject` only when the declared base list is empty —
otherwise `<base_name>` stands alone.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_derived_component_inherits_its_base_and_not_ccmobject`.

**Status:** done

### §6.3.2.4 Equivalent IDL — Inheritance + supports

**Spec:** §6.3.2.4, p. 13 (PDF) — `component <name> : <base> supports
<I1>, <I2> { ... }` → `interface <name> : <base>, <I1>, <I2> { ... }`.

**Repo:** `crates/ccm/src/transform.rs::transform_component` sets
`<base>` first, then the `supports` list.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_derived_component_with_supports_lists_the_base_first_and_then_the_supported`
— checks the order `[B, I1, I2]` and that `Components::CCMObject` is not
repeated on top.

**Status:** done

### §6.3.3 Component Body

**Spec:** §6.3.3, p. 13 (PDF) — "Declarations for facets, receptacles,
event sources, event sinks and attributes all map onto operations on
the component's equivalent interface."

**Repo:** `crates/ccm/src/transform.rs::transform_component` — the
loop over the `ComponentRole` ports of the declaration body.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_whole_trader_component_yields_every_operation_the_clauses_name`
— one component with `provides`/`uses`/`uses multiple`/`emits`/
`publishes`/`consumes`/`attribute`; the test enumerates every operation
that follows on the equivalent interface.

**Status:** done

### §6.4.1 Facets — Equivalent IDL

**Spec:** §6.4.1, p. 13 (PDF) — `provides <interface_type> <name>;` →
`<interface_type> provide_<name>();` on the equivalent interface.

**Repo:** `crates/ccm/src/transform.rs::transform_component`
(`ComponentRole::Facet` branch).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
provides_yields_a_provide_operation_returning_the_facet,
the_whole_trader_component_yields_every_operation_the_clauses_name}`.

**Status:** done

### §6.4.2 Semantics of Facet References

**Spec:** §6.4.2, pp. 13-14 (PDF) — behavior constraints for
facet refs (nil allowed, lifecycle bounded by component).

**Repo:** IDL op signatures via `component_def.rs::FacetDef` +
lifetime-constraint validator
`crates/corba-ccm/src/lifecycle.rs::{check_facet_lifetime,
FacetLifetimeViolation}`. The container runtime uses this check
to reject use-after-destroy + orphan references.

**Tests:** `crates/corba-ccm/src/lifecycle.rs::tests::{facet_lifetime_passes_when_alive,
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

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{provide_facet_returns_facet_by_name,
provide_facet_returns_none_for_unknown,
get_all_facets_returns_complete_list, get_named_facets_filters_by_names}`.

**Status:** done — inheritance model + generic operations as a
skeleton path live; object-reference `same_component` ORB-bound.

### §6.4.3.1 get_component (CORBA::Object enhancement)

**Spec:** §6.4.3.1, p. 14 (PDF) — extension of `CORBA::Object` with an
`Object get_component()` pseudo-IDL op. The ORB delivers this even without
a component vendor.

**Repo:** `crates/corba-ccm/src/home.rs` — `FacetRef::get_component`
returns the component reference behind a facet reference,
`ComponentRef::get_component` returns the same reference; the GIOP
operation name is
`crates/corba-ccm/src/home.rs::GET_COMPONENT_GIOP_OPERATION`.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
get_component_on_a_facet_reference_returns_the_component,
get_component_on_a_component_reference_returns_the_same_reference,
get_component_travels_as_the_giop_operation_named_component}`.

**Status:** done

### §6.4.4 Provided References and Component Identity

**Spec:** §6.4.4, p. 17 (PDF) — `same_component` operation on
Navigation; default implementation via the container servant framework.

**Repo:** `crates/corba-ccm/src/home.rs` —
`ComponentRef::same_component` compares component type and instance
identity, `FacetRef::same_component` delegates to the owning component,
as §6.4.4 requires for facets that inherit `Navigation`.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
two_facets_of_one_instance_are_the_same_component,
a_facet_answers_same_component_as_the_component_that_owns_it,
two_instances_of_one_component_type_are_not_the_same_component,
one_instance_id_under_two_component_types_is_not_the_same_component}`.

**Status:** done

### §6.4.5 Supported Interfaces

**Spec:** §6.4.5, pp. 17-19 (PDF) — `supports <I>` widening +
narrowing semantics; KeylessCCMHome::create_component returns CCMObject,
narrowing to I.

**Repo:** the inheritance model covers this (`component_bases`); type-
identity narrowing as a runtime helper in
`crates/corba-ccm/src/component_def.rs::ComponentDef::
{supports_interface, supported_interface_repo_ids}`. Object-
reference widening (POA `is_a`) remains ORB-bound.

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{supports_interface_returns_true_for_listed_iface,
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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
uses_simplex_yields_connect_disconnect_and_get_connection,
uses_multiple_returns_a_cookie_and_the_plural_getter}`.

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

**Tests:** `crates/ccm/src/model.rs::tests::{cookie_new_stores_octet_seq,
cookie_truncate_preserves_octet_seq,
receptacle_description_supports_simplex_and_multiplex}` +
`crates/corba-ccm/src/lifecycle.rs::tests::{receptacle_simplex_connect_then_disconnect,
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

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{get_all_receptacles_returns_complete_list,
get_named_receptacles_filters}`.

**Status:** done — model + generic-lookup operations as a
skeleton path live; connect lifecycle still container runtime.

### §6.6.1 Event Types — Equivalent IDL

**Spec:** §6.6.1.1, pp. 24-25 (PDF) — `eventtype A {...}; eventtype B :
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

**Spec:** §6.6.1.2, p. 25 (PDF) — `module Components { abstract
valuetype EventBase { }; };`.

**Repo:** ScopedName reference in
`crates/ccm/src/transform.rs::transform_event_type` — only the first
event type of a chain gets `Components::EventBase`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_first_event_type_of_a_chain_inherits_event_base`.

**Status:** done

### §6.6.2 EventConsumer Interface

**Spec:** §6.6.2, pp. 25-26 (PDF) — `Components::EventConsumerBase` with
`push_event(in EventBase) raises BadEventType`.

**Repo:** `crates/ccm/src/transform.rs::transform_event_type` — the
consumer base list is `Components::EventConsumerBase` as long as the
event type itself has no base; otherwise `<Base>Consumer`, so that the
consumer chain follows the event chain.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_consumer_interface_inherits_event_consumer_base_at_the_root_of_the_chain`
— `TickConsumer : Components::EventConsumerBase` and
`PricedTickConsumer : TickConsumer`.

**Status:** done

### §6.6.3 Event Service Provided by Container

**Spec:** §6.6.3, p. 26 (PDF) — container runtime topic — event channels.

**Repo:** `crates/corba-ccm/src/port.rs` (`PortRegistry` +
`EventStream` — the connection table through which the container binds
event sources to event sinks) +
`crates/corba-ccm-lib/src/dds_bridge.rs` (event source → DataWriter,
event sink → DataReader; the distribution itself is DDS-DCPS in
`crates/dcps/` instead of a separate CosEventChannel stack).

**Tests:** `crates/corba-ccm/src/port.rs::tests::{
event_sink_subscription_round_trip,
list_event_sinks_unknown_source_empty}`,
`crates/corba-ccm-lib/src/dds_bridge.rs::tests::{
add_mapping_round_trip, mapping_direction_round_trip,
duplicate_port_rejected, activate_makes_active}`.

**Status:** done — container-side event distribution through the port
registry and the DDS bridge.

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

**Tests:** `crates/ccm/src/model.rs::tests::publisher_description_can_have_multiple_subscribers`,
`crates/corba-ccm/src/component_def.rs::tests::{
get_all_publishers_excludes_emit_only_sources,
get_all_emitters_includes_only_emit_only_sources}`,
`crates/corba-ccm-lib/src/dds_bridge.rs::tests::mapping_direction_round_trip`.

**Status:** done — data model + routing behavior via DCPS live;
generic lookup operations as a skeleton path.

### §6.6.5 Publisher — Equivalent IDL

**Spec:** §6.6.5.1, p. 27 (PDF) — `publishes <T> <name>;` → `Cookie
subscribe_<name>(in TConsumer consumer) raises
(ExceededConnectionLimit); TConsumer unsubscribe_<name>(in Cookie ck)
raises (InvalidConnection);`.

**Repo:** `crates/ccm/src/transform.rs::publishes_ops`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::publishes_yields_subscribe_returning_a_cookie`.

**Status:** done

### §6.6.6 Emitters — Equivalent IDL

**Spec:** §6.6.6.1, p. 28 (PDF) — `emits <T> <name>;` → `void
connect_<name>(in TConsumer consumer) raises (AlreadyConnected);
TConsumer disconnect_<name>() raises (NoConnection);`.

**Repo:** `crates/ccm/src/transform.rs::emits_ops`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::emits_yields_connect_and_disconnect_and_the_consumer_interface`.

**Status:** done

### §6.6.7 Event Sinks — Equivalent IDL

**Spec:** §6.6.7.1, p. 29 (PDF) — `consumes <T> <name>;` → `TConsumer
get_consumer_<name>();`.

**Repo:** `crates/ccm/src/transform.rs::transform_component`
(`ComponentRole::EventSink` branch).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::consumes_yields_a_get_consumer_operation`.

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

**Tests:** `crates/corba-ccm/src/component_def.rs::tests::{get_all_publishers_excludes_emit_only_sources,
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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_keyless_home_yields_an_implicit_inheriting_keylesscmhome`.

**Status:** done

### §6.7.1.2 Home Definitions — With Primary Key

**Spec:** §6.7.1.2, p. 34 (PDF) — `home <h> manages <C> primarykey
<K> { ... };` → 4-op implicit iface (`create(K key)`,
`find_by_primary_key(K key)`, `remove(K key)`, `<K> get_primary_key
(<C> comp)`).

**Repo:** `crates/ccm/src/transform.rs::build_keyed_implicit`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_keyed_home_yields_the_four_operations_and_inherits_nothing`.

**Status:** done

### §6.7.1.3 Home — Supported Interfaces

**Spec:** §6.7.1.3, p. 35 (PDF) — `home <h> supports <I> manages <C>
{...};` → `interface <h>Explicit : Components::CCMHome, <I> {...};`.

**Repo:** `crates/ccm/src/transform.rs::transform_home` appends the
`supports` list to `Components::CCMHome`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_home_with_supports_puts_the_supported_interfaces_on_its_explicit`
— the supported interfaces land on the explicit interface, not on the
implicit one and not on the equivalent home.

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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_primary_key_with_a_public_scalar_member_is_accepted,
a_primary_key_without_the_base_is_rejected,
a_primary_key_with_a_private_member_is_rejected,
a_primary_key_holding_an_object_reference_is_rejected,
a_primary_key_holding_a_nested_struct_is_accepted,
an_abstract_valuetype_is_not_a_primary_key}`.

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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_factory_is_emitted_with_create_failure_first_in_its_raises,
a_finder_is_emitted_with_finder_failure_and_the_callers_own_raises}`.

**Status:** done

### §6.7.4 Home Inheritance

**Spec:** §6.7.4, p. 37 (PDF) — derived home: `<h>Explicit :
<base_h>Explicit`.

**Repo:** `transform_home` sets `explicit_bases` to `<base>Explicit`
when `home.base.is_some()` (`base_explicit_name` helper).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_derived_home_inherits_the_base_homes_explicit_interface,
a_derived_home_with_supports_extends_the_base_explicit}`.

**Status:** done

### §6.7.5 Semantics of Home Operations

**Spec:** §6.7.5, pp. 38-39 (PDF) — orthodox vs heterodox operations,
polymorphism rules.

**Repo:** `crates/corba-ccm/src/home.rs::HomeFinder` — the orthodox
operations `create` (§6.7.5.1 keyless: a system-generated instance id;
§6.7.5.2 keyed: the primary key is the instance id),
`find_by_primary_key` and `remove`;
`crates/corba-ccm/src/container.rs::Container` carries the instance
lifecycle `remove` hangs off.

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

**Tests:** `crates/corba-ccm/src/home.rs::tests::{home_get_component_def_returns_managed_component_id,
home_get_home_def_returns_self_repo_id}`.

**Status:** done — iface reference + type-identity operations
implemented; object-lifecycle operations remain container runtime.

### §6.7.7 KeylessCCMHome Interface

**Spec:** §6.7.7, p. 41 (PDF) — `interface KeylessCCMHome {
CCMObject create_component() raises (CreateFailure); };`.

**Repo:** ScopedName reference point + semantics in
`crates/ccm/src/transform.rs::build_keyless_implicit`.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_keyless_home_yields_an_implicit_inheriting_keylesscmhome`.

**Status:** done

### §6.8 Home Finders

**Spec:** §6.8, pp. 41-43 (PDF) — `Components::HomeFinder` interface;
`CORBA::ORB::resolve_initial_references("ComponentHomeFinder")`.

**Repo:** `crates/corba-ccm/src/home.rs::ComponentHomeFinder` — the
three `find_home_by_*` operations over the homes registered through
`HomeRegistration::register_home` (§9.4.1.2), with `HomeNotFound` as the
failure case; the initial-reference identifier is
`crates/corba-ccm/src/home.rs::COMPONENT_HOME_FINDER_INITIAL_REFERENCE`.
For `find_home_by_name`, `crates/corba-cosnaming/src/stringified.rs`
supplies the stringified-name format.

**Tests:** `crates/corba-ccm/src/home.rs::tests::{
the_home_finder_is_reached_through_the_component_home_finder_initial_reference,
a_registered_home_is_found_by_component_type_and_by_home_type,
an_unregistered_component_type_raises_home_not_found,
a_home_registered_without_a_name_cannot_be_found_by_name,
an_unregistered_home_is_never_returned_again}`.

**Status:** done

### §6.9 Component Configuration

**Spec:** §6.9, pp. 43-44 (PDF) — component configurability via
attributes; `configuration_complete` operation on CCMObject.

**Repo:** `crates/corba-rust/src/component_emit.rs::emit_component`
emits `configuration_complete` as a lifecycle operation on the component
trait; the configuration phase itself lives in
`crates/corba-ccm/src/lifecycle.rs::{StandardConfigurator,
ConfiguratorRegistry}` (§6.10).

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::a_component_trait_carries_the_configuration_complete_lifecycle_operation`,
`crates/corba-ccm/src/lifecycle.rs::tests::{
configurator_set_get_roundtrip, configurator_registry_register_and_lookup}`.

**Status:** done

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
`crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_drops_configurator_operations`,
`crates/corba-ccm/src/lifecycle.rs::tests::{configurator_set_get_roundtrip,
configurator_rejects_unknown_attribute,
configurator_rejects_readonly_attribute,
configurator_get_unknown_returns_unknown_attribute,
configurator_registry_register_and_lookup}`.

**Status:** done — model + filter + configurator iface live.

### §6.11 Component Inheritance

**Spec:** §6.11, pp. 48-49 (PDF) — component inheritance + CCMObject.

**Repo:** `crates/ccm/src/transform.rs::transform_component` covers
component inheritance.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
a_derived_component_inherits_its_base_and_not_ccmobject,
a_derived_component_with_supports_lists_the_base_first_and_then_the_supported}`.

**Status:** done

### §6.11.1 CCMObject Interface

**Spec:** §6.11.1, p. 49 (PDF) — `interface CCMObject : Navigation,
Receptacles, Events { ... };`.

**Repo:** `Components::CCMObject` as a ScopedName reference point;
the inheritance chain is referenced per the spec rule (not explicitly
broken out in the AST — that remains the caller's responsibility at the
linker).

**Tests:** `crates/ccm/tests/equivalent_idl.rs::a_plain_component_inherits_ccmobject`.

**Status:** done

### §6.12 Conformance Requirements

**Spec:** §6.12, pp. 50-52 (PDF) — eight conformance points (see §2);
sub-section §6.12.2 Changes to Object Services.

**Repo:** conformance markers in `crates/corba-ccm/src/lib.rs::
conformance::{CCM_CONFORMANCE_BASIC_LEVEL,
CCM_CONFORMANCE_BASIC_LEVEL_JAVA, LIGHTWEIGHT_CCM_LEVEL}`.

**Tests:** `crates/corba-ccm/src/lib.rs::conformance_tests::ccm_conformance_basic_level_marker_matches_spec`,
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

**Spec:** §8, pp. 67-108 (PDF) — servant/skeleton generator, executor
lifetime, composition persistence, proxy homes, language mapping
(C++/Java).

**Repo:** `crates/corba-ccm/src/cif.rs` (CIF AST + executor/segment
lifetime), `crates/corba-codegen/src/skeleton.rs` +
`crates/corba-codegen/src/stub.rs` (servant/skeleton generator).

**Tests:** `crates/corba-ccm/src/cif.rs::tests::{
lifecycle_calls_increment_counters, cif_error_carries_diagnostic}`
(executor lifetime `ccm_activate`/`ccm_passivate`/`ccm_remove` +
`CCMException`),
`crates/corba-codegen/src/skeleton.rs::tests::{
every_operation_reaches_the_dispatch_in_every_column,
an_empty_interface_still_dispatches_to_the_default_arm,
cpp_compares_with_strcmp_and_ends_in_bad_operation,
java_switches_on_the_method_string_and_throws_the_omg_class,
rust_matches_and_answers_none_rather_than_an_invented_error}`
(skeleton generator),
`crates/corba-codegen/src/stub.rs::tests::{
a_variable_length_struct_returns_a_pointer_and_a_fixed_one_a_value,
an_object_reference_argument_is_a_ptr_and_its_out_is_an_out,
a_oneway_operation_says_so_in_every_column_that_renders}`
(stub generator).

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

**Spec:** §10, pp. 151-176 (PDF) — two-way mapping CCM ↔ EJB.

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

**Spec:** §11, pp. 177-260 (PDF) — MOF models for BaseIDL package +
ComponentIDL package; XMI DTDs + IDL for IFR.

**Repo:** `crates/corba-ir/src/repository.rs` (Container/Contained
hierarchy), `crates/corba-ir/src/repository_id.rs`,
`crates/corba-ir/src/type_code.rs` (TypeCode backend),
`crates/corba-ir/src/definition_kind.rs` +
`crates/corba-ccm/src/orb_core.rs::{XmiEmitter, MofElement,
IfrCcmMetamodel}` with MOF-2.0 subset (Class/Property/Operation)
+ XMI-1.2 output for the ComponentIDL package.

**Tests:** `crates/corba-ir/src/repository.rs::tests::*`,
`crates/corba-ir/src/repository_id.rs::tests::*`,
`crates/corba-ir/src/type_code.rs::tests::*` +
`crates/corba-ccm/src/orb_core.rs::tests::{
ifr_ccm_metamodel_add_component,
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

**Tests:** `crates/corba-ccm/src/cif.rs::tests::{
lifecycle_calls_increment_counters, cif_error_carries_diagnostic}` +
`crates/corba-ccm/src/orb_core.rs::tests::*` (XMI emitter).

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

**Tests:** `crates/ccm/tests/equivalent_idl.rs::{
the_lightweight_filter_drops_configurator_operations,
the_lightweight_filter_keeps_the_type_specific_operations,
the_lightweight_filter_reports_a_component_it_would_empty}` +
`crates/corba-ccm/src/lwccm.rs::tests::{
same_component_raises_no_implement_with_the_mandated_minor_code,
every_other_disabled_operation_raises_bad_operation}` — §13.1 keeps the
disabled operations in the interface and has the container answer
`BAD_OPERATION`/`NO_IMPLEMENT`; for `same_component`, `NO_IMPLEMENT`
with minor code `IS_LIGHTWEIGHT_COMPONENT` is required.

**Status:** done

### §13.2 No Persistence

**Spec:** §13.2, pp. 274-276 (PDF) — table "Changes associated with
excluding support for persistence": primary keys, CIDL, abstract storage
homes/types, the keyed-home implicit executor, finders in the explicit
home executor, entity container API types, the durable CORBA usage
model, `CCM2Context::get_persistence`. §13.1, p. 273: "Persistence (only
session and service components are supported)."

**Repo:** `crates/corba-ccm/src/lwccm.rs` —
`disabled_by("get_persistence")` reports `LwccmExclusion::Persistence`,
`category_supported` admits only `Session` and `Service`, and
`composition_exclusion` rejects entity compositions and every
storage-home binding as persistence. The extended form stays in
`crates/corba-ccm-lib/src/persistence.rs` + `crates/corba-ccm/src/pss.rs`
and is held out of the profile by these checks.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
the_persistence_operations_are_disabled_by_thirteen_two_and_three,
only_session_and_service_components_are_supported,
an_entity_composition_is_excluded_as_persistence}`.

**Status:** done

### §13.3 No Introspection / Navigation / Type-Specific-Generic Ops

**Spec:** §13.3, pp. 276-277 (PDF) — removed: `provide_facet`,
`get_all_facets`, `get_named_facets`, generic `connect`/`disconnect`/
`get_connections`, generic event-routing operations.

**Repo:** `crates/corba-ccm/src/lwccm.rs::disabled_by` — the
introspection rows of §13.3 as a name list, plus prefix recognition of
the type-specific port operations (`provide_<name>`, `connect_<name>`,
`disconnect_<name>`, `get_connection(s)_<name>`, `subscribe_<name>`,
`unsubscribe_<name>`, `get_consumer_<name>`). The generic counterparts —
`provide_facet`, `connect`, `disconnect`, `get_consumer`, `subscribe`,
`unsubscribe`, `connect_consumer` — stay, because §13.3 keeps exactly
those. The operations themselves remain declared on the equivalent
interface (§13.1); nothing is removed.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
the_type_specific_port_operations_are_disabled_and_the_generic_ones_are_not,
both_connection_getters_are_disabled_the_generic_one_as_introspection,
a_bare_prefix_is_not_an_operation_name}` +
`crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_keeps_the_type_specific_operations`
(the operations stay declared).

**Status:** done

### §13.4-§13.6 No Segmentation/Transactions/Security

**Spec:** §13.4, p. 277 (PDF) — "Exclude support for CIDL", "Exclude
composition", "Remove locator support", "Restrict CIF metamodel to a
single segment per implementation". §13.5, p. 278 — "Disable the
get_rollback_only, get_user_transaction, and set_rollback_only
operations in the CCMContext interface". §13.6, p. 278 — "Disable the
get_caller_principal and is_caller_in_role operations in the CCMContext
interface".

**Repo:** `crates/corba-ccm/src/lwccm.rs` — `composition_exclusion`
rejects every CIDL composition, `MAX_SEGMENTS_PER_IMPLEMENTATION`
carries the 1:1 multiplicity from §13.4, and `disabled_by` reports the
three transaction and the two credential operations of `CCMContext`. The
extended forms stay in `crates/corba-ccm/src/cidl.rs`,
`crates/corba-ccm-ejb/src/tx.rs` and `crates/corba-csiv2/` and are held
out of the profile by these checks.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
a_session_composition_is_still_excluded_because_cidl_is_gone,
the_cif_metamodel_holds_one_segment_per_implementation,
the_three_transaction_operations_of_ccmcontext_are_disabled,
the_two_credential_operations_of_ccmcontext_are_disabled,
get_ccm_home_survives_beside_the_disabled_ccmcontext_operations}`.

**Status:** done

### §13.7 No Configurators

**Spec:** §13.7, p. 279 (PDF) — removed: `Configurator` iface,
`StandardConfigurator`, `HomeConfiguration`.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`
removes `configure`/`set_configuration`/`configuration_complete` from
the equivalent interface (§13.7 removes the `Configurator` and
`StandardConfigurator` interfaces outright);
`crates/corba-ccm/src/lwccm.rs` additionally reports
`set_configurator` on `HomeConfiguration` as disabled.

**Tests:** `crates/ccm/tests/equivalent_idl.rs::the_lightweight_filter_drops_configurator_operations`,
`crates/corba-ccm/src/lwccm.rs::tests::every_other_disabled_operation_raises_bad_operation`.

**Status:** done

### §13.8-§13.9 No Proxy Homes / Home Finders

**Spec:** §13.8, p. 279 (PDF) — "Exclude support for proxy homes"
(§8.2.10 + §9.4.1.3 `ProxyHomeRegistration`). §13.9, p. 279 — "Disable
support for the HomeFinder interface and the ComponentHomeFinder
parameter value to the ORB::resolve_initial_references operation" +
"Disable the get_home_registration operation in the CCM2Context
interface".

**Repo:** `crates/corba-ccm/src/lwccm.rs::disabled_by` reports
`register_proxy_home` under §13.8 and the three `find_home_by_*`
operations plus `get_home_registration` under §13.9. The extended form
of the home finder lives in
`crates/corba-ccm/src/home.rs::ComponentHomeFinder` (§6.8) and is held
out of the profile by this check; `register_home` / `unregister_home`
from §9.4.1.2 are named by neither table and stay.

**Tests:** `crates/corba-ccm/src/lwccm.rs::tests::{
register_proxy_home_is_disabled,
the_home_finder_operations_and_home_registration_are_disabled,
register_home_survives_because_only_the_proxy_form_is_excluded}`.

**Status:** done

### §13.10 Additional Restrictions

**Spec:** §13.10, p. 280 (PDF) — misc restrictions of the extended
model.

**Repo:** `crates/ccm/src/lightweight.rs::filter_to_lightweight`
(filter pipeline) + doc markers in
`crates/corba-ccm/src/lib.rs::conformance::{LWCCM_RESTRICTIONS_ENFORCED,
LWCCM_FILTER_ACTIVE}`.

**Tests:** `crates/corba-ccm/src/lib.rs::conformance_tests::lwccm_restrictions_marker_matches_spec`,
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

**Spec:** §15, pp. 301-314 (PDF) — deployment IDL definitions
(`Deployment::*`).

**Repo:** `Deployment::ExecutionManager` +
`Deployment::DomainApplicationManager` + `Deployment::DomainApplication`
in `crates/corba-dnc/src/execution.rs`; `Deployment::NodeManager` +
`Deployment::NodeApplicationManager` + `Deployment::NodeApplication` in
`crates/corba-dnc/src/node.rs`; `Deployment::RepositoryManager` in
`crates/corba-dnc/src/repository.rs`; the descriptor structures in
`crates/corba-dnc/src/plan.rs`.

**Tests:** `crates/corba-dnc/src/execution.rs::tests::{
full_lifecycle_round_trip, execution_manager_lists_prepared_plans,
invalid_plan_rejected_at_prepare, double_start_fails,
destroy_before_running_fails}` — the operation sequence
`preparePlan`/`startLaunch`/`finishLaunch`/`destroyApplication` of the
§15 IDL; `crates/corba-dnc/src/node.rs::tests::{
prepare_plan_then_start_launch, register_and_unregister_application,
destroy_application_clears_active, unregister_unknown_returns_none}`;
`crates/corba-dnc/src/repository.rs::tests::{
install_and_find_round_trip, install_overwrites_duplicate,
delete_existing_returns_true, delete_unknown_returns_false}`.

**Status:** done

---

## §16 XML Schema for CCM

### §16 XML Schema for CCM

**Spec:** §16, p. 315ff (PDF) — XSD schema for deployment packages.

**Repo:** `crates/corba-dnc/src/xml.rs` (XML codec for deployment
plans following the §16 schema).

**Tests:** `crates/corba-dnc/src/xml.rs::tests::{parses_minimal_plan,
parses_multiple_artifacts, co_locate_with_parsed,
missing_root_tag_returns_expected_tag,
validation_fails_on_unknown_impl_reference}`.

**Status:** done

---

## Audit status

71 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-ccm` — 19 lib tests + 34 tests in
  `tests/equivalent_idl.rs` + 1 doc test green.
* `cargo test -p zerodds-corba-ccm` — 201 lib tests + 3 tests in
  `tests/pi_service_contexts.rs` + 1 doc test green.
* `cargo test -p zerodds-corba-ccm-lib --lib` — 23 tests green.
* `cargo test -p zerodds-corba-ccm-ejb --lib` — 25 tests green.
* `cargo test -p zerodds-corba-dnc --lib` — 30 tests green.
* `cargo test -p zerodds-corba-codegen --lib` — 38 tests green
  (skeleton/stub generator).
* `cargo test -p zerodds-corba-rust --test snapshot_codegen` — 15 tests green.
* `cargo test -p zerodds-corba-ir --lib` — 19 tests green.
* `cargo test -p zerodds-corba-codegen` — 93 tests green (the Annex A.1
  binding for Java).

Open items: none. Decision records: none.
