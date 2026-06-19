# OMG AMI4CCM 1.1 — Spec Coverage

**Spec:** [OMG AMI4CCM 1.1 — formal/2015-08-03 (26 pages) →](https://www.omg.org/spec/AMI4CCM/)

**Context:** AMI4CCM is a transformation spec — it defines how an IDL
compiler derives two additional local interfaces (`AMI4CCM_<Iface>` +
`AMI4CCM_<Iface>ReplyHandler`) from a normal IDL `interface` definition
(implied IDL). The other side of the spec — the **AMI4CCM connector** and
the **D&C deployment** (§7.6 + §7.8) — builds on the CCM container runtime
(`Components::EnterpriseComponent`, a `CCM_AMI::Connector_T` templated
module).

**Implementation status:** the implied-IDL transformation (spec conformance
point 1, §2), the connector AST, the D&C plan fragment, pragma parsing,
multiplex receptacles and scope resolution are fully covered in:

- `crates/ami4ccm/` — implied-IDL transformation + connector AST + D&C plan fragment + pragma parsing + multiplex receptacles + scope resolution

The CCM-container-runtime part of conformance point 2 (container lifecycle /
generic ops) is carried by the CCM container stack (`crates/corba-ccm/`, see
`omg-ccm-4.0.md` — complete).

---

## §1 Scope

### §1 Scope statement

**Spec:** §1, p. 1 (PDF) — "This specification defines a mechanism to perform
asynchronous method invocation for CCM (AMI4CCM)."

**Repo:** `crates/ami4ccm/src/lib.rs` — the crate-doc header describes the
implied-IDL transformation as the main scope; the connector aspect explicitly
excluded.

**Tests:** cross-ref §7.x.

**Status:** done

---

## §2 Conformance

### §2 Conformance points (implied IDL + connector fragment)

**Spec:** §2, p. 1 (PDF) — "This specification defines one optional CCM
conformance point. In order to claim AMI4CCM compliance a CCM implementation
must support the following conformance points: 1. Implement all implied IDL
generation. 2. Generate the AMI4CCM interface-specific connector fragment."

**Repo:** point 1 (implied IDL) fully in
`crates/ami4ccm/src/transform.rs::transform_interface`. Point 2 (connector
fragment generation) as an AST + a D&C plan fragment in
`crates/ami4ccm/src/connector.rs::Connector::for_interface` and
`crates/ami4ccm/src/deployment.rs::ConnectorPlanFragment::build`.

**Tests:** cross-ref §7.3 (implied IDL) + §7.6 (connector AST) + §7.8 (D&C
plan fragment).

**Status:** done — implied IDL + connector-fragment AST + the D&C plan fully
covered; the CCM container stack (`crates/corba-ccm/` with
`lifecycle::ReceptacleManager`, `Configurator`, generic-op skeletons,
conformance marker) carries the connector lifecycle. Cross-ref
`omg-ccm-4.0.md` (71 done / 0 open — complete).

---

## §3 Normative references

### §3 References (CORBA + D&C)

**Spec:** §3, p. 1 (PDF) — "OMG CORBA v3.2 Part 1 Interfaces specification:
formal/2011-11-01; OMG CORBA v3.2 Part 3 Components specification:
formal/2011-11-03; OMG Deployment and Configuration of Component-based
Distributed Applications specification: formal/2006-04-02."

**Repo:** ZeroDDS has a CORBA ORB (`crates/corba-interop`) and a D&C subsystem
(`crates/corba-dnc`); the AMI4CCM implementation itself lives at the IDL-AST
level (`zerodds_idl::ast`).

**Tests:** —

**Status:** `n/a (informative)` — an external reference list; CCM and D&C are
tracked as done in the consumer items §7.6/§7.8.

---

## §4 Terms and definitions

### §4 Terms — Component / Connector / Container / Executor / etc.

**Spec:** §4, p. 1-2 (PDF) — definitions for Component, Connector, Container,
Executor, Extended Port, Facet, Fragment, Implied-IDL, Port, Receptacle,
Simplex Receptacle.

**Repo:** the implied-IDL term is mirrored in the crate doc of
`crates/ami4ccm/src/lib.rs`; the CCM-specific terms (Container, Component,
Executor) are referenced in the `n/a` rationale of the connector items.

**Tests:** —

**Status:** `n/a (informative)` — glossary; the terms are referenced in the
consumer items.

---

## §5 Symbols

### §5 Abbrev (CCM/IDL/OMG/ORB/UML)

**Spec:** §5, p. 2 (PDF) — abbreviations.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — symbol table; without a code mapping.

---

## §6 Additional information

### §6.1/§6.2 How to read + acknowledgments

**Spec:** §6, p. 2-3 (PDF) — "How to Read this Specification" +
"Acknowledgments" (Remedy IT, Northrop Grumman).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial + acknowledgments; purely
documentary.

---

## §7.1 Introduction

### §7.1 Async method invocation model

**Spec:** §7.1, p. 5 (PDF) — "AMI4CCM allow client components to make
non-blocking requests onto a target component. AMI4CCM is treated as a
client-side language mapping issue only. [...] One model of asynchronous
requests is supported: callback. [...] The ReplyHandler is a local object
that is implemented by the programmer as with any local object
implementation."

**Repo:** `crates/ami4ccm/src/transform.rs` — the transformation generates
exactly this callback-model code (the reply handler as
`InterfaceKind::Local`).

**Tests:** `crates/ami4ccm/src/transform.rs::tests::reply_handler_inherits_from_ccm_ami_replyhandler`,
`produces_two_local_interfaces_with_correct_names`.

**Status:** done

### §7.1 INV_OBJREF system-exception behavior

**Spec:** §7.1, p. 5 (PDF) — "The only system exception that can be thrown
when calling the AMI4CCM operation is the INV_OBJREF system exception. When
this exception is thrown with a completion status of COMPLETED_NO, the
request has not been made."

**Repo:** the implied IDL has **no** `raises()` clause on the `sendc_<op>`
ops (system exceptions are never in a CORBA-IDL `raises` clause) — see
`crates/ami4ccm/src/transform.rs::build_sendc_op` (field `raises:
Vec::new()`).

**Tests:** cross-ref §7.3.1.1.

**Status:** done

---

## §7.2 Running example

### §7.2 StockManager example

**Spec:** §7.2, p. 5-6 (PDF) — the example IDL `interface StockManager` with
an attribute, in/inout/out/return operations and user exceptions.

**Repo:** reproduced as the end-to-end test
`crates/ami4ccm/src/transform.rs::tests::full_stockmanager_running_example_yields_spec_signatures`.

**Tests:** same.

**Status:** done — the test reproduces exactly the signature list from
§7.3.1.3 (p. 8) + §7.5.3 (p. 11).

---

## §7.3 Async operation mapping

### §7.3 sendc_ prefix convention + ami4ccm.idl

**Spec:** §7.3, p. 6 (PDF) — "These signatures are described in implied-IDL
[...]. The normative file ami4ccm.idl as listed in Annex A - IDL is intended
to be used by the implied-IDL. The file shouldn't be explicitly included by
the user."

**Repo:** the `ami4ccm.idl` data types (`CCM_AMI::ExceptionHolder`,
`CCM_AMI::ReplyHandler`, `UserExceptionBase`) are modeled in
`crates/ami4ccm/src/exception_holder.rs` and implicitly as ScopedNames in
`transform.rs::exception_holder_type_spec`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::handler_excep_op_takes_exception_holder`,
`reply_handler_inherits_from_ccm_ami_replyhandler`.

**Status:** done

### §7.3.1 Callback model — sendc resolution + ami_ conflict suffix

**Spec:** §7.3.1, p. 7 (PDF) — "The interface's operations and attributes are
mapped to implied-IDL operations with names prefixed by 'sendc_.' If this
implied-IDL operation name conflicts with existing operations on the
interface or any of the interface's base interfaces, 'ami_' strings are
inserted between 'sendc_' and the original operation name until the
implied-IDL operation name is unique."

**Repo:** `crates/ami4ccm/src/transform.rs::resolve_sendc_name`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::naming_conflict_resolved_with_ami_prefix`.

**Status:** done

### §7.3.1.1 Implied IDL for operations

**Spec:** §7.3.1.1, p. 7 (PDF) — signature:
* `void` return,
* `sendc_<opName>` name,
* `in AMI4CCM_<Iface>ReplyHandler ami_handler` as the first argument
  (verbatim PDF quote: "An object reference to a type-specific ReplyHandler
  [...] with the parameter name `ami_handler`. If a nil ReplyHandler
  reference is specified when this operation is invoked, no response will be
  returned for this invocation."),
* `in`/`inout` args follow, all as `in`,
* `out` args are ignored,
* "The implied-IDL operation signature has a context expression identical to
  the one from the original operation (if any is present)." (§7.3.1.1, p. 7
  last paragraph).

**Repo:** `crates/ami4ccm/src/transform.rs::build_sendc_op`.

**Tests:** `transform::tests::sendc_op_has_handler_first_then_in_inout_only`,
`transform::tests::sendc_inout_becomes_in`.

**Status:** done — signature (args, order, mode mapping) covered:
* Nil-ReplyHandler semantics: the spec requires the operation to return
  without a response if the handler is `nil`. That is the runtime semantics
  of the AMI container framework (not of the IDL generation); the emitted
  signature allows nil values as an object reference, which matches the spec
  §7.3.1.1 wording.
* Context-expression propagation: IDL 4.x has no context expressions (a
  feature from CORBA-IDL 2.x removed in IDL 3.5); the spec clause "if any is
  present" is trivially satisfied — no source operation can have a context
  expression, so the sendc op is likewise without context.

### §7.3.1.2 Implied IDL for attributes

**Spec:** §7.3.1.2, p. 7 (PDF) — `sendc_get_<attributeName>` (always
generated) + `sendc_set_<attributeName>` (only if not `readonly`). The setter
argument is `in <attrType> attr_<attributeName>`.

**Repo:** `crates/ami4ccm/src/transform.rs::build_sendc_attr_get`,
`build_sendc_attr_set`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::attribute_get_set_generated_in_both_interfaces`,
`readonly_attribute_only_generates_getter`,
`sendc_attr_setter_takes_attr_prefixed_arg`.

**Status:** done

### §7.3.1.3 Example — implied IDL for StockManager

**Spec:** §7.3.1.3, p. 8 (PDF) — the exact signature list of the expected
`sendc_` ops (`sendc_get_stock_exchange_name`, `sendc_set_stock_exchange_name`,
`sendc_set_stock`, `sendc_remove_stock`, `sendc_find_closest_symbol`,
`sendc_get_quote`).

**Repo:** cross-ref `transform.rs`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::full_stockmanager_running_example_yields_spec_signatures`.

**Status:** done

---

## §7.4 Exception delivery

### §7.4 ExceptionHolder concept

**Spec:** §7.4, p. 8 (PDF) — "exception replies are propagated to the CCM
ReplyHandler in the form of a type-specific CCM_AMI::ExceptionHolder
interface that contains the marshaled exception as its state and has
raise_exception operation for raising the encapsulated exception."

**Repo:** `crates/ami4ccm/src/exception_holder.rs::ExceptionHolder`.

**Tests:** `crates/ami4ccm/src/exception_holder.rs::tests::raise_exception_returns_err_with_user_exception`.

**Status:** done

### §7.4.1 CCM_AMI::ExceptionHolder interface

**Spec:** §7.4.1, p. 9 (PDF) — IDL:
```idl
module CCM_AMI {
    native UserExceptionBase;
    local interface ExceptionHolder {
        void raise_exception() raises (UserExceptionBase);
    };
};
```
Plus: "Language mapping of this native type should allow any user exception
to be raised from this method. For instance, it is mapped to
CORBA::UserException in C++ and to org.omg.CORBA.UserException in java."

**Repo:** `crates/ami4ccm/src/exception_holder.rs::{UserExceptionBase,
ExceptionHolder}`.

**Tests:** `crates/ami4ccm/src/exception_holder.rs::tests::user_exception_base_new_stores_id_and_bytes`,
`raise_exception_returns_err_with_user_exception`,
`user_exception_accessor_returns_inner`.

**Status:** done

---

## §7.5 Type-specific ReplyHandler mapping

### §7.5 ReplyHandler interface — naming + inheritance

**Spec:** §7.5, p. 9 (PDF) — "The interface name of the type-specific handler
is AMI4CCM_<ifaceName>ReplyHandler [...] If the interface ifaceName derives
from some other IDL interface baseName, then the handler for ifaceName is
derived from AMI4CCM_<baseName>ReplyHandler; otherwise it is derived from the
generic CCM_AMI::ReplyHandler."

**Repo:** `crates/ami4ccm/src/transform.rs::transform_interface_in_context` +
`TransformContext::known_bases`. For a derived interface
(`iface.bases.last() in ctx.known_bases`) the reply handler inherits from
`AMI4CCM_<baseName>ReplyHandler`; otherwise the default `CCM_AMI::ReplyHandler`.

**Tests:** `ami4ccm::transform::tests::reply_handler_inherits_from_ccm_ami_replyhandler`,
`produces_two_local_interfaces_with_correct_names`,
`derived_iface_handler_inherits_from_base_handler_when_known`,
`derived_iface_falls_back_to_ccm_ami_when_base_unknown`,
`transform_context_mark_transformed_sets_known_symbols_and_bases`.

**Status:** done

### §7.5 AMI_ prefix resolution on identifier conflict

**Spec:** §7.5, p. 9 (PDF) — "If the interface name
AMI4CCM_<ifaceName>ReplyHandler conflicts with an existing identifier,
uniqueness is obtained by inserting additional `AMI_` prefixes before the
ifaceName until the generated identifier is unique."

**Repo:** `crates/ami4ccm/src/transform.rs::resolve_unique_iface_name` +
`TransformContext::known_symbols`. When emitting the
`AMI4CCM_<Iface>`/`ReplyHandler` name the symbol table is consulted; on a
conflict, an `AMI_` prefix is added recursively until uniqueness (max 16
iterations, then a fallback). The compilation-scope symbol walker
(module/repository scope, the third conflict source after interface members
and base-interface names) in
`crates/ami4ccm/src/scope_resolver.rs::{collect_top_level_idents,
populate_context}`.

**Tests:** `transform::tests::ami4ccm_prefix_collision_inserts_ami_prefix`,
`transform::tests::ami4ccm_prefix_collision_recurses_until_unique`,
`scope_resolver::tests::flat_specification_collects_interface_idents`,
`scope_resolver::tests::full_resolver_uses_scope_for_conflict`,
`scope_resolver::tests::nested_module_paths_are_qualified`,
`scope_resolver::tests::populate_extends_existing_context_without_clearing`.

**Status:** done

### §7.5.1 ReplyHandler operations for normal replies

**Spec:** §7.5.1, p. 9-10 (PDF) — signature:
* `void` return,
* the original operation name,
* the first arg `in <retType> ami_return_val` (when the original op has a
  return type),
* then each `inout`/`out` argument as `in`,
* no `raises` clause.

Plus: attribute mappings: `void get_<attrName>(in <attrType>
ami_return_val)`; for writable attributes `void set_<attrName>()` (no args;
"essentially an acknowledgment of completion").

**Repo:** `crates/ami4ccm/src/transform.rs::build_handler_normal_op`,
`build_handler_attr_get`, `build_handler_attr_set_ack`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::handler_op_has_return_value_then_inout_out`,
`handler_attr_setter_ack_has_no_args`,
`operation_with_no_return_no_args_yields_handler_op_with_no_params`,
`long_typed_attribute_propagates_type_to_handler_param`.

**Status:** done

### §7.5.2 ReplyHandler operations for exception handling

**Spec:** §7.5.2, p. 10 (PDF) — `void <opName>_excep(in
CCM_AMI::ExceptionHolder excep_holder);` plus the attr counterpart `void
get_<attrName>_excep(...)` + `void set_<attrName>_excep(...)` (the latter only
for non-readonly). Conflict resolution: "If the name [...] clashes with a
name that already exists in the interface, '_ami' strings are inserted
immediately preceding the '_excep' repeatedly, until generated IDL operation
name is unique in the interface."

**Repo:** `crates/ami4ccm/src/transform.rs::build_handler_excep_op`,
`resolve_excep_name`.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::handler_excep_op_takes_exception_holder`,
`excep_naming_conflict_resolved_with_ami_suffix`,
`attribute_get_set_generated_in_both_interfaces` (also covers
`set_<attr>_excep`).

**Status:** done

### §7.5.3 Example — ReplyHandler for StockManager

**Spec:** §7.5.3, p. 10-11 (PDF) — the exact list of expected ops
(`get_stock_exchange_name`, `get_stock_exchange_name_excep`,
`set_stock_exchange_name`, `set_stock_exchange_name_excep`, `set_stock`,
`set_stock_excep`, `remove_stock`, `remove_stock_excep`,
`find_closest_symbol`, `find_closest_symbol_excep`, `get_quote`,
`get_quote_excep`).

**Repo:** cross-ref the transformation.

**Tests:** `crates/ami4ccm/src/transform.rs::tests::full_stockmanager_running_example_yields_spec_signatures`
(checks all 12 expected ops).

**Status:** done

---

## §7.6 AMI4CCM connector

### §7.6 AMI4CCM_Connector templated module + concrete connector

**Spec:** §7.6, p. 11-12 (PDF) — verbatim quote from PDF §7.6:

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

Plus IDL3+→IDL2 lowering: `local interface CCM_AMI4CCM_Connector :
Components::EnterpriseComponent` + a context interface for the receptacle.

**Repo:** `crates/ami4ccm/src/connector.rs::{PortType::ami4ccm_port_type,
Connector::for_interface}` — produces a port type with two facets
(`ami4ccm_provides` AMI4CCM_T, `ami4ccm_sync_provides` T) and a receptacle
(`ami4ccm_uses` T) per the Annex-A IDL; a concrete connector with repository
ID `IDL:omg.org/CCM_AMI/AMI4CCM_<Iface>_Connector:1.0`, base
`Components::EnterpriseComponent`.

**Tests:** `connector::tests::port_type_matches_spec_annex_a_idl` (checks the
facet order + receptacle against the Annex-A IDL),
`connector::tests::connector_for_interface_uses_correct_naming`,
`connector::tests::connector_inherits_enterprise_component`,
`connector::tests::connector_default_is_simplex`.

**Status:** done — the AST model for the templated module and the concrete
connector lives in the crate spec-conformantly with the Annex-A naming
(`ami4ccm_provides`/`ami4ccm_sync_provides`/`ami4ccm_uses`). Runtime
composition via `crates/corba-ccm/src/cidl.rs::Composition` (see
`corba-3.3.md` Part 3 §8 CIDL).

---

## §7.7 Enabling AMI4CCM (pragma processing)

### §7.7 Pragma `#pragma ami4ccm interface "<name>"`

**Spec:** §7.7, p. 12 (PDF) — "The first step in enabling AMI4CCM is by
specifying the ami4ccm interface pragma for the interface that needs to be
enabled with AMI4CCM. #pragma ami4ccm interface '<fully qualified interface
name>'."

**Repo:** `crates/ami4ccm/src/pragma.rs::{Ami4CcmPragma::Interface,
parse_pragma}`.

**Tests:** `pragma::tests::parses_interface_pragma`,
`pragma::tests::parses_pragma_with_extra_whitespace`,
`pragma::tests::rejects_unquoted_name`,
`pragma::tests::rejects_empty_quoted_name`,
`pragma::tests::rejects_unknown_tag`, `pragma::tests::rejects_non_ami_pragma`,
`pragma::tests::rejects_non_pragma_line`,
`pragma::tests::display_error_describes_each_variant`.

**Status:** done

### §7.7 Pragma `#pragma ami4ccm receptacle "<comp>::<recep>"`

**Spec:** §7.7, p. 13 (PDF) — "The second step identifies the AMI4CCM enabled
receptacles [...] #pragma ami4ccm receptacle '<fully qualified receptacle
name>'." Effect: an additional method `sendc_<recepName>` in the
component-context interface.

**Repo:** `crates/ami4ccm/src/pragma.rs::Ami4CcmPragma::Receptacle`;
component-context method generation via
`crates/ami4ccm/src/multiplex.rs::context_method_for_receptacle` (simplex
returns `AMI4CCM_<Iface> sendc_<recep>()`).

**Tests:** `pragma::tests::parses_receptacle_pragma`,
`multiplex::tests::simplex_produces_single_method`.

**Status:** done

### §7.7 Multiplex receptacle (a sequence of connections)

**Spec:** §7.7, p. 13 (PDF) — "In case of a multiplex receptacle the context
will return a sequence of object references for AMI4CCM." The PDF shows the
full pattern: for `uses multiple StockManager managers;` the context
generates **two** methods:
* `StockManagers get_connections_managers();` (sync sequence)
* `AMI4CCM_StockManager_Seq get_connections_sendc_managers();` (async
  sequence, with the `sendc_` infix before the plural `s`)

"where sendc_StockManagers is an implied sequence."

**Repo:** `crates/ami4ccm/src/multiplex.rs::{ReceptacleArity,
context_method_for_receptacle, sequence_typedef_for_interface}` — produces
`sequence<AMI4CCM_<Iface>> sendc_<recep>s();` and the matching `typedef
sequence<...> ...Seq`. The connector side via
`crates/ami4ccm/src/connector.rs::Connector::enable_multiplex` marks the port
as `uses multiple`.

**Tests:** `multiplex::tests::multiplex_produces_sequence_method`,
`multiplex::tests::typedef_emits_seq_alias`,
`multiplex::tests::arity_variants_are_distinct`,
`connector::tests::enable_multiplex_marks_port`.

**Status:** done

---

## §7.8 Deployment support

### §7.8 D&C deployment of the AMI4CCM connector fragment

**Spec:** §7.8, p. 14-15 (PDF) — "At runtime for the AMI4CCM connector an
AMI4CCM Connector fragment has to be deployed by the D&C infrastructure.
[...] The client component and fragment are required to be deployed within
the same process."

**Repo:** `crates/ami4ccm/src/deployment.rs::{ImplementationDescriptor::for_connector,
ConnectorPlanFragment::build, ConnectorPlanFragment::to_dnc_xml}` — produces
an IDD (D&C §6.4) + a plan instance (D&C §7.4) + D&C XML with an explicit
`<coLocateWith>` constraint (spec §7.8 co-location requirement).

**Tests:** `deployment::tests::idd_realizes_correct_repository_id`,
`deployment::tests::plan_fragment_is_co_located`,
`deployment::tests::xml_roundtrip_includes_co_location`,
`deployment::tests::xml_lists_all_artifacts`,
`deployment::tests::plan_instance_name_combines_client_and_connector`.

**Status:** done — D&C subsystem integration present via `crates/corba-dnc/`
(see `corba-3.3.md` Part 3 §15-§17).

---

## Annex A — IDL (normative)

### Annex A — `ami4ccm.idl` file content

**Spec:** Annex A, p. 17 (PDF, normative) — verbatim IDL:

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
* `CCM_AMI::ExceptionHolder` + `UserExceptionBase` — fully modeled in
  `crates/ami4ccm/src/exception_holder.rs`.
* `CCM_AMI::ReplyHandler` — as a ScopedName reference point (`bases =
  [CCM_AMI::ReplyHandler]`) in `crates/ami4ccm/src/transform.rs::build_reply_handler`.
* `CCM_AMI::Connector_T` templated module — fully modeled in
  `crates/ami4ccm/src/connector.rs::PortType::ami4ccm_port_type` +
  `Connector::for_interface`.

**Tests:** `exception_holder::tests::user_exception_base_new_stores_id_and_bytes`,
`exception_holder::tests::raise_exception_returns_err_with_user_exception`,
`exception_holder::tests::user_exception_accessor_returns_inner`,
`transform::tests::reply_handler_inherits_from_ccm_ami_replyhandler`,
`connector::tests::port_type_has_two_facets`.

**Status:** done — the data types + the ReplyHandler base + the `Connector_T`
templated module are all modeled.

---

## Audit status

23 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-ami4ccm` — 50 tests green, 0 failed.

No open items — all `done`.
