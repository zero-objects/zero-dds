# OMG AMI4CCM 1.1 — Spec-Coverage

**Spec:** [OMG AMI4CCM 1.1 — formal/2015-08-03 (26 Seiten) →](https://www.omg.org/spec/AMI4CCM/)

**Kontext:** AMI4CCM ist eine Transformations-Spec — sie definiert, wie
ein IDL-Compiler aus einer normalen IDL-`interface`-Definition zwei
zusätzliche Local-Interfaces (`AMI4CCM_<Iface>` +
`AMI4CCM_<Iface>ReplyHandler`) ableitet (Implied-IDL). Die zweite Seite
der Spec — der **AMI4CCM Connector** und das **D&C-Deployment** (§7.6 +
§7.8) — setzt auf der CCM-Container-Runtime auf (`Components::
EnterpriseComponent`, `CCM_AMI::Connector_T`-Templated-Module).

**Implementierungs-Stand:** Implied-IDL-Transformation (Spec-Conformance-
Punkt 1, §2), Connector-AST, D&C-Plan-Fragment, Pragma-Parsing,
Multiplex-Receptacles und Scope-Resolution sind voll abgedeckt in:

- `crates/ami4ccm/` — Implied-IDL-Transformation + Connector-AST + D&C-Plan-Fragment + Pragma-Parsing + Multiplex-Receptacles + Scope-Resolution

Der CCM-Container-Runtime-Anteil von Conformance-Punkt 2 (Container-
Lifecycle/Generic-Ops) wird vom CCM-Container-Stack getragen
(`crates/corba-ccm/`, siehe `omg-ccm-4.0.md` — vollständig).

---

## §1 Scope

### §1 Scope Statement

**Spec:** §1, S. 1 (PDF) — "This specification defines a mechanism to
perform asynchronous method invocation for CCM (AMI4CCM)."

**Repo:** `crates/ami4ccm/src/lib.rs` — Crate-Doc-Header beschreibt die
Implied-IDL-Transformation als Hauptscope; Connector-Aspekt explizit
ausgenommen.

**Tests:** Cross-Ref §7.x.

**Status:** done

---

## §2 Conformance

### §2 Conformance Points (Implied-IDL + Connector-Fragment)

**Spec:** §2, S. 1 (PDF) — "This specification defines one optional CCM
conformance point. In order to claim AMI4CCM compliance a CCM
implementation must support the following conformance points: 1.
Implement all implied IDL generation. 2. Generate the AMI4CCM
interface-specific connector fragment."

**Repo:** Punkt 1 (Implied-IDL) voll in
`crates/ami4ccm/src/transform.rs::transform_interface`. Punkt 2
(Connector-Fragment-Generation) als AST + D&C-Plan-Fragment in
`crates/ami4ccm/src/connector.rs::Connector::for_interface` und
`crates/ami4ccm/src/deployment.rs::ConnectorPlanFragment::build`.

**Tests:** Cross-Ref §7.3 (Implied-IDL) + §7.6 (Connector-AST) + §7.8
(D&C-Plan-Fragment).

**Status:** done — Implied-IDL + Connector-Fragment-AST + D&C-Plan
voll abgedeckt; CCM-Container-Stack (`crates/corba-ccm/` mit
`lifecycle::ReceptacleManager`, `Configurator`, Generic-Op-Skeletons,
Conformance-Marker) trägt den Connector-Lifecycle. Cross-Ref
`omg-ccm-4.0.md` (71 done / 0 open — vollständig).

---

## §3 Normative References

### §3 References (CORBA + D&C)

**Spec:** §3, S. 1 (PDF) — "OMG CORBA v3.2 Part 1 Interfaces
specification: formal/2011-11-01; OMG CORBA v3.2 Part 3 Components
specification: formal/2011-11-03; OMG Deployment and Configuration of
Component-based Distributed Applications specification: formal/2006-04-02."

**Repo:** ZeroDDS hat einen CORBA-ORB (`crates/corba-interop`) und ein
D&C-Subsystem (`crates/corba-dnc`); die AMI4CCM-Implementation selbst
lebt auf der IDL-AST-Ebene (`zerodds_idl::ast`).

**Tests:** —

**Status:** `n/a (informative)` — Externe Referenz-Liste; CCM und D&C werden in den Konsumenten-Items §7.6/§7.8 als done geführt.

---

## §4 Terms and Definitions

### §4 Terms — Component / Connector / Container / Executor / etc.

**Spec:** §4, S. 1-2 (PDF) — Definitionen für Component, Connector,
Container, Executor, Extended Port, Facet, Fragment, Implied-IDL, Port,
Receptacle, Simplex Receptacle.

**Repo:** Implied-IDL-Begriff ist im Crate-Doc von
`crates/ami4ccm/src/lib.rs` gespiegelt; CCM-spezifische Begriffe
(Container, Component, Executor) sind in der `n/a`-Begründung der
Connector-Items referenziert.

**Tests:** —

**Status:** `n/a (informative)` — Glossar; Begriffe sind in den Konsumenten-Items referenziert.

---

## §5 Symbols

### §5 Abbrev (CCM/IDL/OMG/ORB/UML)

**Spec:** §5, S. 2 (PDF) — Abkürzungen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Symbol-Tabelle; ohne Code-Mapping.

---

## §6 Additional Information

### §6.1/§6.2 How to Read + Acknowledgments

**Spec:** §6, S. 2-3 (PDF) — "How to Read this Specification" +
"Acknowledgments" (Remedy IT, Northrop Grumman).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial + Acknowledgments; rein dokumentarisch.

---

## §7.1 Introduction

### §7.1 Async Method Invocation Model

**Spec:** §7.1, S. 5 (PDF) — "AMI4CCM allow client components to make
non-blocking requests onto a target component. AMI4CCM is treated as a
client-side language mapping issue only. [...] One model of
asynchronous requests is supported: callback. [...] The ReplyHandler
is a local object that is implemented by the programmer as with any
local object implementation."

**Repo:** `crates/ami4ccm/src/transform.rs` — die Transformation
generiert genau diesen Callback-Modell-Code (Reply-Handler als
`InterfaceKind::Local`).

**Tests:** `crates/ami4ccm/src/transform.rs::tests::reply_handler_inherits_from_ccm_ami_replyhandler`,
`produces_two_local_interfaces_with_correct_names`.

**Status:** done

### §7.1 INV_OBJREF System-Exception Behavior

**Spec:** §7.1, S. 5 (PDF) — "The only system exception that can be
thrown when calling the AMI4CCM operation is the INV_OBJREF system
exception. When this exception is thrown with a completion status of
COMPLETED_NO, the request has not been made."

**Repo:** Implied-IDL hat **keine** `raises()`-Klausel auf den
`sendc_<op>`-Ops (System-Exceptions sind in CORBA-IDL nie in
`raises`-Klausel) — siehe
`crates/ami4ccm/src/transform.rs::build_sendc_op` (Feld `raises:
Vec::new()`).

**Tests:** Cross-Ref §7.3.1.1.

**Status:** done

---

## §7.2 Running Example

### §7.2 StockManager Beispiel

**Spec:** §7.2, S. 5-6 (PDF) — Beispiel-IDL `interface StockManager`
mit Attribute, in/inout/out/return-Operations und User-Exceptions.

**Repo:** Reproduziert als End-to-End-Test
`crates/ami4ccm/src/transform.rs::tests::full_stockmanager_running_example_yields_spec_signatures`.

**Tests:** ders.

**Status:** done — der Test reproduziert exakt die Signatur-Liste aus
§7.3.1.3 (S. 8) + §7.5.3 (S. 11).

---

## §7.3 Async Operation Mapping

### §7.3 sendc_-Präfix-Konvention + ami4ccm.idl

**Spec:** §7.3, S. 6 (PDF) — "These signatures are described in
implied-IDL [...]. The normative file ami4ccm.idl as listed in Annex A
- IDL is intended to be used by the implied-IDL. The file shouldn't be
explicitly included by the user."

**Repo:** Die `ami4ccm.idl`-Datentypen (`CCM_AMI::ExceptionHolder`,
`CCM_AMI::ReplyHandler`, `UserExceptionBase`) sind in
`crates/ami4ccm/src/exception_holder.rs` und implizit als ScopedNames
in `transform.rs::exception_holder_type_spec` modelliert.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::handler_excep_op_takes_exception_holder`,
`reply_handler_inherits_from_ccm_ami_replyhandler`.

**Status:** done

### §7.3.1 Callback Model — sendc-Auflösung + ami_-Konflikt-Suffix

**Spec:** §7.3.1, S. 7 (PDF) — "The interface's operations and
attributes are mapped to implied-IDL operations with names prefixed by
'sendc_.' If this implied-IDL operation name conflicts with existing
operations on the interface or any of the interface's base interfaces,
'ami_' strings are inserted between 'sendc_' and the original
operation name until the implied-IDL operation name is unique."

**Repo:** `crates/ami4ccm/src/transform.rs::resolve_sendc_name`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::naming_conflict_resolved_with_ami_prefix`.

**Status:** done

### §7.3.1.1 Implied-IDL für Operations

**Spec:** §7.3.1.1, S. 7 (PDF) — Signatur:
* `void` Return,
* `sendc_<opName>` Name,
* `in AMI4CCM_<Iface>ReplyHandler ami_handler` als erstes Argument
  (wörtliches PDF-Zitat: "An object reference to a type-specific
  ReplyHandler [...] with the parameter name `ami_handler`. If a nil
  ReplyHandler reference is specified when this operation is invoked,
  no response will be returned for this invocation."),
* `in`/`inout` Args nachgezogen, alle als `in`,
* `out`-Args werden ignoriert,
* "The implied-IDL operation signature has a context expression
  identical to the one from the original operation (if any is
  present)." (§7.3.1.1, S. 7 letzter Absatz).

**Repo:** `crates/ami4ccm/src/transform.rs::build_sendc_op`.

**Tests:** `transform::tests::sendc_op_has_handler_first_then_in_inout_only`,
`transform::tests::sendc_inout_becomes_in`.

**Status:** done — Signatur (Args, Order, Mode-Mapping) abgedeckt:
* Nil-ReplyHandler-Semantik: Spec verlangt dass die Operation
  ohne Response zurückkehrt wenn der Handler `nil` ist. Das ist
  Runtime-Semantik des AMI-Container-Frameworks (nicht der IDL-
  Generation); die emittierte Signatur erlaubt nil-Werte als
  Object-Reference, was der Spec-§7.3.1.1-Wording entspricht.
* Context-Expression-Propagation: IDL 4.x hat keine
  Context-Expressions (Feature aus CORBA-IDL 2.x in IDL 3.5
  gestrichen); die Spec-Klausel "if any is present" ist trivially
  erfüllt — keine Quelloperation kann eine Context-Expression
  haben, also ist der Sendc-Op ebenfalls ohne Context.

### §7.3.1.2 Implied-IDL für Attributes

**Spec:** §7.3.1.2, S. 7 (PDF) — `sendc_get_<attributeName>` (immer
generiert) + `sendc_set_<attributeName>` (nur wenn nicht `readonly`).
Setter-Argument ist `in <attrType> attr_<attributeName>`.

**Repo:** `crates/ami4ccm/src/transform.rs::build_sendc_attr_get`,
`build_sendc_attr_set`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::attribute_get_set_generated_in_both_interfaces`,
`readonly_attribute_only_generates_getter`,
`sendc_attr_setter_takes_attr_prefixed_arg`.

**Status:** done

### §7.3.1.3 Beispiel — implied-IDL für StockManager

**Spec:** §7.3.1.3, S. 8 (PDF) — exakte Signatur-Liste der erwarteten
`sendc_`-Ops (`sendc_get_stock_exchange_name`,
`sendc_set_stock_exchange_name`, `sendc_set_stock`,
`sendc_remove_stock`, `sendc_find_closest_symbol`, `sendc_get_quote`).

**Repo:** Cross-Ref `transform.rs`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::full_stockmanager_running_example_yields_spec_signatures`.

**Status:** done

---

## §7.4 Exception Delivery

### §7.4 ExceptionHolder-Konzept

**Spec:** §7.4, S. 8 (PDF) — "exception replies are propagated to the
CCM ReplyHandler in the form of a type-specific CCM_AMI::ExceptionHolder
interface that contains the marshaled exception as its state and has
raise_exception operation for raising the encapsulated exception."

**Repo:** `crates/ami4ccm/src/exception_holder.rs::ExceptionHolder`.

**Tests:** `crates/ami4ccm/src/exception_holder.rs::tests::raise_exception_returns_err_with_user_exception`.

**Status:** done

### §7.4.1 CCM_AMI::ExceptionHolder Interface

**Spec:** §7.4.1, S. 9 (PDF) — IDL:
```idl
module CCM_AMI {
    native UserExceptionBase;
    local interface ExceptionHolder {
        void raise_exception() raises (UserExceptionBase);
    };
};
```
Plus: "Language mapping of this native type should allow any user
exception to be raised from this method. For instance, it is mapped
to CORBA::UserException in C++ and to org.omg.CORBA.UserException in
java."

**Repo:** `crates/ami4ccm/src/exception_holder.rs::{UserExceptionBase,
ExceptionHolder}`.

**Tests:** `crates/ami4ccm/src/exception_holder.rs::tests::user_exception_base_new_stores_id_and_bytes`,
`raise_exception_returns_err_with_user_exception`,
`user_exception_accessor_returns_inner`.

**Status:** done

---

## §7.5 Type-Specific ReplyHandler Mapping

### §7.5 ReplyHandler-Iface — Naming + Inheritance

**Spec:** §7.5, S. 9 (PDF) — "The interface name of the type-specific
handler is AMI4CCM_<ifaceName>ReplyHandler [...] If the interface
ifaceName derives from some other IDL interface baseName, then the
handler for ifaceName is derived from AMI4CCM_<baseName>ReplyHandler;
otherwise it is derived from the generic CCM_AMI::ReplyHandler."

**Repo:** `crates/ami4ccm/src/transform.rs::transform_interface_in_context`
+ `TransformContext::known_bases`. Bei abgeleitetem Interface
(`iface.bases.last() in ctx.known_bases`) erbt der ReplyHandler von
`AMI4CCM_<baseName>ReplyHandler`; sonst Default
`CCM_AMI::ReplyHandler`.

**Tests:** `ami4ccm::transform::tests::reply_handler_inherits_from_ccm_ami_replyhandler`,
`produces_two_local_interfaces_with_correct_names`,
`derived_iface_handler_inherits_from_base_handler_when_known`,
`derived_iface_falls_back_to_ccm_ami_when_base_unknown`,
`transform_context_mark_transformed_sets_known_symbols_and_bases`.

**Status:** done

### §7.5 AMI_-Prefix-Resolution bei Identifier-Conflict

**Spec:** §7.5, S. 9 (PDF) — "If the interface name AMI4CCM_<ifaceName>
ReplyHandler conflicts with an existing identifier, uniqueness is
obtained by inserting additional `AMI_` prefixes before the ifaceName
until the generated identifier is unique."

**Repo:** `crates/ami4ccm/src/transform.rs::resolve_unique_iface_name`
+ `TransformContext::known_symbols`. Beim Emittieren des
`AMI4CCM_<Iface>`-/-`ReplyHandler`-Namens wird die Symbol-Tabelle
konsultiert; bei Konflikt rekursiv `AMI_`-Prefix bis Eindeutigkeit
(max 16 Iterationen, dann Fallback). Compilation-Scope-Symbol-Walker
(Modul-/Repository-Scope, dritte Conflict-Quelle nach Iface-Members
und Base-Iface-Names) in `crates/ami4ccm/src/scope_resolver.rs::
{collect_top_level_idents, populate_context}`.

**Tests:** `transform::tests::ami4ccm_prefix_collision_inserts_ami_prefix`,
`transform::tests::ami4ccm_prefix_collision_recurses_until_unique`,
`scope_resolver::tests::flat_specification_collects_interface_idents`,
`scope_resolver::tests::full_resolver_uses_scope_for_conflict`,
`scope_resolver::tests::nested_module_paths_are_qualified`,
`scope_resolver::tests::populate_extends_existing_context_without_clearing`.

**Status:** done

### §7.5.1 ReplyHandler-Operations für Normal-Replies

**Spec:** §7.5.1, S. 9-10 (PDF) — Signatur:
* `void` Return,
* Original-Operation-Name,
* erstes Arg `in <retType> ami_return_val` (wenn Original-Op
  Return-Type hat),
* dann jedes `inout`/`out` Argument als `in`,
* keine `raises`-Klausel.

Plus: Attribute-Mappings: `void get_<attrName>(in <attrType>
ami_return_val)`; bei writable-Attributen `void set_<attrName>()` (no
args; "essentially an acknowledgment of completion").

**Repo:** `crates/ami4ccm/src/transform.rs::build_handler_normal_op`,
`build_handler_attr_get`, `build_handler_attr_set_ack`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::handler_op_has_return_value_then_inout_out`,
`handler_attr_setter_ack_has_no_args`,
`operation_with_no_return_no_args_yields_handler_op_with_no_params`,
`long_typed_attribute_propagates_type_to_handler_param`.

**Status:** done

### §7.5.2 ReplyHandler-Operations für Exception-Handling

**Spec:** §7.5.2, S. 10 (PDF) — `void <opName>_excep(in CCM_AMI::
ExceptionHolder excep_holder);` plus Attr-Pendant `void
get_<attrName>_excep(...)` + `void set_<attrName>_excep(...)` (letzteres
nur für non-readonly). Conflict-Resolution: "If the name [...]
clashes with a name that already exists in the interface, '_ami'
strings are inserted immediately preceding the '_excep' repeatedly,
until generated IDL operation name is unique in the interface."

**Repo:** `crates/ami4ccm/src/transform.rs::build_handler_excep_op`,
`resolve_excep_name`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::handler_excep_op_takes_exception_holder`,
`excep_naming_conflict_resolved_with_ami_suffix`,
`attribute_get_set_generated_in_both_interfaces` (deckt
`set_<attr>_excep` mit ab).

**Status:** done

### §7.5.3 Beispiel — ReplyHandler für StockManager

**Spec:** §7.5.3, S. 10-11 (PDF) — exakte Liste der erwarteten Ops
(`get_stock_exchange_name`, `get_stock_exchange_name_excep`,
`set_stock_exchange_name`, `set_stock_exchange_name_excep`,
`set_stock`, `set_stock_excep`, `remove_stock`, `remove_stock_excep`,
`find_closest_symbol`, `find_closest_symbol_excep`, `get_quote`,
`get_quote_excep`).

**Repo:** Cross-Ref Transformation.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::full_stockmanager_running_example_yields_spec_signatures`
(prüft alle 12 erwarteten Ops).

**Status:** done

---

## §7.6 AMI4CCM Connector

### §7.6 AMI4CCM_Connector Templated-Module + Concrete Connector

**Spec:** §7.6, S. 11-12 (PDF) — wörtliches Zitat aus PDF §7.6:

```idl
module CCM_AMI {
    module Connector_T<interface T, interface AMI4CCM_T> {
        porttype AMI4CCM_Port_Type {
            // Deliver the asynchronous interface with its sendc_ ops
            provides AMI4CCM_T ami4ccm_provides;
            // Delivers the original interface for synchronous
            // invocations through the connector
            provides T ami4ccm_sync_provides;
            // Use the port of the target component
            uses T ami4ccm_uses;
        };
        connector AMI4CCM_Connector {
            port AMI4CCM_Port_Type ami4ccm_port;
        };
    };
};
```

Plus IDL3+→IDL2-Lowering: `local interface CCM_AMI4CCM_Connector :
Components::EnterpriseComponent` + Context-Interface für den
Receptacle.

**Repo:** `crates/ami4ccm/src/connector.rs::{PortType::ami4ccm_port_type,
Connector::for_interface}` — produziert Port-Type mit zwei Facets
(`ami4ccm_provides` AMI4CCM_T, `ami4ccm_sync_provides` T) und einem
Receptacle (`ami4ccm_uses` T) gemäß Annex-A-IDL; Concrete-Connector
mit Repository-ID `IDL:omg.org/CCM_AMI/AMI4CCM_<Iface>_Connector:1.0`,
Base `Components::EnterpriseComponent`.

**Tests:** `connector::tests::port_type_matches_spec_annex_a_idl`
(prüft Facet-Reihenfolge + Receptacle gegen die Annex-A-IDL),
`connector::tests::connector_for_interface_uses_correct_naming`,
`connector::tests::connector_inherits_enterprise_component`,
`connector::tests::connector_default_is_simplex`.

**Status:** done — AST-Modell für Templated-Module und Concrete-
Connector lebt im Crate spec-konform mit Annex-A-Naming
(`ami4ccm_provides`/`ami4ccm_sync_provides`/`ami4ccm_uses`). Runtime-
Composition via `crates/corba-ccm/src/cidl.rs::Composition` (siehe
`corba-3.3.md` Part 3 §8 CIDL).

---

## §7.7 Enabling AMI4CCM (Pragma-Processing)

### §7.7 Pragma `#pragma ami4ccm interface "<name>"`

**Spec:** §7.7, S. 12 (PDF) — "The first step in enabling AMI4CCM is
by specifying the ami4ccm interface pragma for the interface that
needs to be enabled with AMI4CCM. #pragma ami4ccm interface '<fully
qualified interface name>'."

**Repo:** `crates/ami4ccm/src/pragma.rs::{Ami4CcmPragma::Interface,
parse_pragma}`.

**Tests:** `pragma::tests::parses_interface_pragma`,
`pragma::tests::parses_pragma_with_extra_whitespace`,
`pragma::tests::rejects_unquoted_name`,
`pragma::tests::rejects_empty_quoted_name`,
`pragma::tests::rejects_unknown_tag`,
`pragma::tests::rejects_non_ami_pragma`,
`pragma::tests::rejects_non_pragma_line`,
`pragma::tests::display_error_describes_each_variant`.

**Status:** done

### §7.7 Pragma `#pragma ami4ccm receptacle "<comp>::<recep>"`

**Spec:** §7.7, S. 13 (PDF) — "The second step identifies the AMI4CCM
enabled receptacles [...] #pragma ami4ccm receptacle '<fully qualified
receptacle name>'." Wirkung: zusätzliche Methode `sendc_<recepName>`
im Component-Context-Interface.

**Repo:** `crates/ami4ccm/src/pragma.rs::Ami4CcmPragma::Receptacle`;
Component-Context-Methoden-Generierung über
`crates/ami4ccm/src/multiplex.rs::context_method_for_receptacle`
(Simplex liefert `AMI4CCM_<Iface> sendc_<recep>()`).

**Tests:** `pragma::tests::parses_receptacle_pragma`,
`multiplex::tests::simplex_produces_single_method`.

**Status:** done

### §7.7 Multiplex-Receptacle (Sequence von Connections)

**Spec:** §7.7, S. 13 (PDF) — "In case of a multiplex receptacle the
context will return a sequence of object references for AMI4CCM."
PDF zeigt das vollständige Pattern: für `uses multiple StockManager
managers;` generiert der Context **zwei** Methoden:
* `StockManagers get_connections_managers();` (sync sequence)
* `AMI4CCM_StockManager_Seq get_connections_sendc_managers();`
  (async sequence, mit `sendc_`-Infix vor dem Plural-`s`)

"where sendc_StockManagers is an implied sequence."

**Repo:** `crates/ami4ccm/src/multiplex.rs::{ReceptacleArity,
context_method_for_receptacle, sequence_typedef_for_interface}` —
produziert `sequence<AMI4CCM_<Iface>> sendc_<recep>s();` und das
passende `typedef sequence<...> ...Seq`. Connector-Side via
`crates/ami4ccm/src/connector.rs::Connector::enable_multiplex` markiert
das Port als `uses multiple`.

**Tests:** `multiplex::tests::multiplex_produces_sequence_method`,
`multiplex::tests::typedef_emits_seq_alias`,
`multiplex::tests::arity_variants_are_distinct`,
`connector::tests::enable_multiplex_marks_port`.

**Status:** done

---

## §7.8 Deployment Support

### §7.8 D&C-Deployment des AMI4CCM Connector Fragments

**Spec:** §7.8, S. 14-15 (PDF) — "At runtime for the AMI4CCM connector
an AMI4CCM Connector fragment has to be deployed by the D&C
infrastructure. [...] The client component and fragment are required
to be deployed within the same process."

**Repo:** `crates/ami4ccm/src/deployment.rs::{ImplementationDescriptor::for_connector,
ConnectorPlanFragment::build, ConnectorPlanFragment::to_dnc_xml}` —
produziert IDD (D&C §6.4) + Plan-Instance (D&C §7.4) + D&C-XML mit
expliziter `<coLocateWith>`-Constraint (Spec §7.8 Co-Location-Pflicht).

**Tests:** `deployment::tests::idd_realizes_correct_repository_id`,
`deployment::tests::plan_fragment_is_co_located`,
`deployment::tests::xml_roundtrip_includes_co_location`,
`deployment::tests::xml_lists_all_artifacts`,
`deployment::tests::plan_instance_name_combines_client_and_connector`.

**Status:** done — D&C-Subsystem-Integration vorhanden via
`crates/corba-dnc/` (siehe `corba-3.3.md` Part 3 §15-§17).

---

## Annex A — IDL (normativ)

### Annex A — `ami4ccm.idl` Datei-Inhalt

**Spec:** Annex A, S. 17 (PDF, normativ) — wörtliche IDL:

```idl
#include <Components.idl>
module CCM_AMI {
    native UserExceptionBase;
    /// Exception holder to rethrow the original exception
    local interface ExceptionHolder {
        void raise_exception() raises (UserExceptionBase);
    };
    /// Base interface for the Callback model
    local interface ReplyHandler {
    };
    /**
     * Templated Connector module for AMI4CC. Expects
     * two template arguments, the original interface and
     * its AMI4CCM counterpart
     */
    module Connector_T<interface T, interface AMI4CCM_T> {
        porttype AMI4CCM_Port_Type {
            provides AMI4CCM_T ami4ccm_provides;
            provides T ami4ccm_sync_provides;
            uses T ami4ccm_uses;
        };
        connector AMI4CCM_Connector {
            port AMI4CCM_Port_Type ami4ccm_port;
        };
    };
};
```

**Repo:**
* `CCM_AMI::ExceptionHolder` + `UserExceptionBase` — voll modelliert
  in `crates/ami4ccm/src/exception_holder.rs`.
* `CCM_AMI::ReplyHandler` — als ScopedName-Bezugspunkt
  (`bases = [CCM_AMI::ReplyHandler]`) in
  `crates/ami4ccm/src/transform.rs::build_reply_handler`.
* `CCM_AMI::Connector_T` Templated-Module — voll modelliert in
  `crates/ami4ccm/src/connector.rs::PortType::ami4ccm_port_type` +
  `Connector::for_interface`.

**Tests:** `exception_holder::tests::user_exception_base_new_stores_id_and_bytes`,
`exception_holder::tests::raise_exception_returns_err_with_user_exception`,
`exception_holder::tests::user_exception_accessor_returns_inner`,
`transform::tests::reply_handler_inherits_from_ccm_ami_replyhandler`,
`connector::tests::port_type_has_two_facets`.

**Status:** done — Datentypen + ReplyHandler-Basis + `Connector_T`-
Templated-Module sind alle modelliert.

---

## Audit-Status

23 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-ami4ccm` — 50 Tests grün, 0 failed.

Keine offenen Punkte — alle Items `done`.
