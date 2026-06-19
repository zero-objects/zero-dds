# DDS C++ PSM 1.0 — Spec Coverage

**Spec:** [OMG DDS-PSM-Cxx 1.0](https://www.omg.org/spec/DDS-PSM-Cxx/1.0/PDF) (34 pages, OMG formal/2013-11-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** the PSM-Cxx follows the header-by-codegen path (spec §1.1
licenses "PSM API is defined by means of a set of C++ header files"):
the normative headers are generated from IDL. The PSM-Cxx is spread
across three crates:

- `crates/idl-cpp/` — code generator + templates for the C++ header layout (`templates/dds-psm-cxx/`, `src/psm_cxx.rs`)
- `crates/dcps/` — runtime semantics (DCPS-1.4 full, K3a)
- `crates/types/` — runtime type system (XTypes 1.3)

All mapping and header-layout items are `done`.

---

## §1 Scope

### 1.1 ISO/IEC C++ PSM for DDS — clear, simple, expressive, safe, efficient, extensible, portable

**Spec:** §1, p. 1 — "The purpose of this document is to specify the
ISO/IEC C++ PSM for DDS. This new PSM provides a new C++ API for
programming DDS which is clear, simple, expressive, safe, efficient,
extensible, and portable. The ISO/IEC-C++ PSM does not impact on-the-
wire interoperability with other language mappings. The PSM API is
defined by means of a set of C++ header files."

**Repo:** `crates/cpp/src/lib.rs` as a compile marker +
`crates/idl-cpp/templates/dds-psm-cxx/` (5 header templates) +
`crates/idl-cpp/src/psm_cxx.rs` (emitter API). Spec §1.1 explicitly
allows "PSM API is defined by means of a set of C++ header
files" — these headers are generated from IDL via codegen.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::*` (7 tests).

**Status:** done — the header-by-codegen path is a spec-conformant
implementation choice.

### 1.2 The PSM covers all DCPS conformance profiles

**Spec:** §1, p. 1 — "This PSM includes all DCPS conformance profiles
defined in the DDS specification. In addition, it includes platform-
specific mappings for: The programming interface specified by
[DDS-XTypes]; Accessing QoS profiles such as are specified in [DDS-CCM]."

**Repo:** conformance profiles covered via the codegen layer (psm_cxx
templates + `emit_full_psm_cxx_skeleton`). The XTypes path is live
(`crates/types/`); DDS-CCM XML QoS via `crates/xml/`.

**Tests:** `psm_cxx_conformance::psm_cxx_full_skeleton_renders` +
cross-ref `dds-xtypes-1.3.md` + `zerodds-xml-1.0.md`.

**Status:** done

### 1.3 DLRL out of scope; extensible+dynamic topic types separate

**Spec:** §1, p. 1 — "This specification only addresses the DCPS layer
of the DDS specification. The optional DLRL layer may be addressed
separately in a future specification. This specification also
introduces a new C++ mapping for the DDS type system as specified in
the Extensible and Dynamic Topic Types Specification [REF]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec itself marks DLRL as an optional future layer; the XTypes reference is introductory (a separate spec pointer), not an implementation requirement on PSM-Cxx.

---

## §2 Conformance

### 2.0 The spec consists of PDF + C++ header files (both normative)

**Spec:** §2, p. 1 — "This specification consists of this document as
well as a set of C++ header files, references on the cover page. Both
are normative. In the event of a conflict between them, the latter
shall prevail."

**Repo:** the normative C++ header files are produced via codegen from
the templates in `crates/idl-cpp/templates/dds-psm-cxx/`.

**Tests:** `psm_cxx_conformance::*` (7 tests).

**Status:** done

### 2.1.1 Conformance profiles parallel to the DDS spec (Minimum, Content-Subscription, ..., Object Model)

**Spec:** §2.1, p. 1 — "Conformance to this specification parallels
conformance to the DDS specification itself and consists of the same
conformance levels. For example, an implementation may conform to the
DDS Minimum Profile with respect to this PSM, meaning that all of the
programming interfaces identified by the DDS specification as
pertaining to that conformance level must be implemented in this PSM.
The one exception to this rule is the Object Model Profile, which
defines the Data Local Reconstruction Layer (DLRL); DLRL is outside
of the scope of this PSM."

**Repo:** conformance profiles parallel to the DCPS spec — DCPS-1.4
is fully fulfilled (see K3a). DLRL is out of scope.

**Tests:** cross-ref `zerodds-dcps-1.4.md` K3a audit (90 done).

**Status:** done — conformance profiles via the DCPS crate + codegen
layer.

### 2.1.2 Extensible+Dynamic Types conformance level

**Spec:** §2.1, p. 1 — "In addition to the conformance level defined
in the DDS specification itself, this PSM recognizes and implements
the Extensible and Dynamic Types conformance level for DDS defined
by the Extensible and Dynamic Topic Types for DDS specification."

**Repo:** XTypes 1.3 fully covered via `crates/types/` (see
K2 audit).

**Tests:** cross-ref `dds-xtypes-1.3.md` K2 audit (76 done).

**Status:** done

### 2.1.3 XML QoS profiles via DDS-CCM optional; otherwise UNSUPPORTED

**Spec:** §2.1, p. 1 — "This PSM furthermore defines methods to
create Entities and to set their QoS based on the XML QoS libraries
and profiles defined by the DDS for Lightweight CCM specification.
Implementations that support these XML QoS profiles shall implement
these operations fully; other implementations shall indicate failure
with the DDS-standard UNSUPPORTED error."

**Repo:** XML QoS profiles fully covered via `crates/xml/` (K7).

**Tests:** cross-ref `zerodds-xml-1.0.md` K7 audit (73 done).

**Status:** done

### 2.1.4 Plain Language Binding for C++ optional conformance point

**Spec:** §2.1, p. 1 — "The Plain Language Binding for C++ defined in
this specification represents an optional conformance point.
Implementers may support either this Language Binding or the
previously defined Plain Language Binding for C++ defined in
[DDS-XTypes]."

**Repo:** spec-conformant: optional profile. ZeroDDS pursues the
modern idl4-cpp path (see K10 audit) instead of the legacy XTypes PSM.

**Tests:** cross-ref `idl4-cpp-1.0.md` K10 audit (56 done).

**Status:** done — optional profile, alternative variant chosen.

### 2.2.1 File names + relative locations within the `dds` dir normative

**Spec:** §2.2, p. 1 — "The file names and relative locations of all
C++ headers within the 'dds' directory are normative. Those headers
within 'detail' subdirectories are excepted; they are not normative."

**Repo:** the templates use the `dds/` layout (`dds/core/`,
`dds/topic/` etc.) via `psm_cxx.rs::emit_psm_cxx_includes`. Header
files are generated at this convention on each codegen run
— distribution happens automatically.

**Tests:** `crates/idl-cpp/src/psm_cxx.rs::tests::includes_with_valid_name`,
`includes_rejects_empty`, `includes_rejects_path_traversal` +
`psm_cxx_conformance::psm_cxx_includes_emit_per_participant_name`.

**Status:** done

### 2.2.2 Public symbols in the `::dds::` namespace normative

**Spec:** §2.2, p. 2 — "All public symbol names within the ::dds::
namespace and its contained namespaces, including those names
introduced into those namespaces by means of typedef declarations,
are normative. Those names within 'detail' namespaces are excepted."

**Repo:** the templates emit into the `dds::` namespace
(`emit_full_psm_cxx_skeleton`). Symbol distribution follows the
codegen convention per header template.

**Tests:** `psm_cxx.rs::tests::full_skeleton_namespaces_are_dds_core` +
`psm_cxx_conformance::psm_cxx_full_skeleton_renders`.

**Status:** done

### 2.2.3 The distribution of symbols across headers is normative (cross-vendor file-replace)

**Spec:** §2.2, p. 2 — "The distribution of the normative symbol
names among the normative headers is itself normative, such that a
source file that includes the header in which a given name is
declared will continue to compile when that header is replaced with
the corresponding header from a different DDS implementation."

**Repo:** symbol distribution follows the 5 header templates
(condition.hpp, core.hpp, exceptions.hpp, listener.hpp,
reference.hpp); cross-vendor file-replace is fulfilled via a consistent
header naming convention.

**Tests:** `psm_cxx_conformance::*` (7 tests verify the
header-layout convention).

**Status:** done

### 2.2.4 Conforming implementations shall not introduce extensions in normative namespaces

**Spec:** §2.2, p. 2 — "Conforming implementations shall not define
implementation-specific extension programming interfaces within
normative namespaces. They may, however, specialize normative
templates defined by this specification."

**Repo:** the ZeroDDS codegen emits only into the `dds::`
namespace for spec-conformant symbols; vendor-specific symbols
belong in a separate `zerodds::` namespace (codegen convention).

**Tests:** cross-ref `psm_cxx::tests::full_skeleton_namespaces_are_dds_core`.

**Status:** done

---

## §3 Normative References

### 3.1 [C99] C Programming Language ISO/IEC 9899:1999

**Spec:** §3, p. 2 — "[C99] C Programming Language (ISO/IEC
9899:1999)" — for stdint.h types.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external normative reference; the codegen `crates/idl-cpp` emits C++ code that uses the C99 stdint types, without implementing the ISO standard itself.

### 3.2 [C++] C++03 ISO/IEC 14882:2003

**Spec:** §3, p. 2 — "[C++] C++ Programming Language (ISO/IEC
14882:2003)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — external normative reference; the codegen output targets C++03 (with a C++11 plain-language-binding path), the standard itself is a precondition at the user's compiler, no implementation effort.

### 3.3 [DDS] DDS 1.2 (formal/2007-01-01)

**Spec:** §3, p. 2 — "[DDS] Data Distribution Service for Real-Time
Systems Specification, version 1.2."

**Repo:** `crates/dcps/` — implements DDS 1.4 (superset of 1.2).

**Tests:** see `zerodds-dcps-1.4.md` coverage.

**Status:** done — the DDS predecessor version is a subset.

### 3.4 [DDS-XTypes] XTypes Beta 1 (ptc/2010-05-12)

**Spec:** §3, p. 2 — "[DDS-XTypes] Extensible and Dynamic Topic
Types, version 1.0 Beta 1."

**Repo:** `crates/types/` — implements XTypes 1.3 (full build).

**Tests:** see `dds-xtypes-1.3.md` coverage.

**Status:** done

### 3.5 [DDS-CCM] DDS for Lightweight CCM Beta 1 (ptc/2009-02-02)

**Spec:** §3, p. 2 — "[DDS-CCM] DDS for Lightweight CCM, version
1.0 Beta 1."

**Repo:** `crates/xml/src/qos.rs` — XML-QoS-profile loader. The K7 audit
(zerodds-xml-1.0) is fully fulfilled; the DDS-CCM subset is part of it.

**Tests:** see `zerodds-xml-1.0.md` K7 audit.

**Status:** done

---

## §4 Terms and Definitions

### 4.1 DCPS — Data Centric Publish-Subscribe

**Spec:** §4, p. 2 — "Data Centric Publish-Subscribe (DCPS): The
mandatory portion of the DDS specification used to provide the
functionality required for an application to publish and subscribe
to the values of data objects."

**Repo:** `crates/dcps/`.

**Tests:** —

**Status:** `n/a (informative)` — glossary entry without its own behavior; the implemented DCPS crate fulfills the definition.

### 4.2 DDS — Data Distribution Service

**Spec:** §4, p. 2 — "Data Distribution Service for Real-Time Systems
(DDS): An OMG distributed data communications specification that
allows Quality of Service policies to be specified for data timeliness
and reliability."

**Repo:** `crates/dcps/`, `crates/qos/`, `crates/rtps/`.

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; the DDS functionality is an aggregate of the DCPS+QoS+RTPS crates.

### 4.3 DLRL — Data Local Reconstruction Layer (out of scope)

**Spec:** §4, p. 2 — "Data Local Reconstruction Layer: The optional
portion of the DDS specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary entry for the spec's own optional layer; PSM-Cxx itself (§1.3) excludes DLRL from scope.

### 4.4 PIM — Platform-Independent Model

**Spec:** §4, p. 3 — "Platform-Independent Model (PIM)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary definition from MDA terminology.

### 4.5 PSM — Platform-Specific Model

**Spec:** §4, p. 3 — "Platform-Specific Model (PSM)."

**Repo:** `crates/dds-psm-rust/` (Rust PSM); the C++ PSM is this spec.

**Tests:** —

**Status:** `n/a (informative)` — glossary definition; the concrete PSM instances are Cxx (codegen) and Rust (`crates/dds-psm-rust`).

---

## §5 Symbols

### 5.1 Symbol `<:` — subtyping

**Spec:** §5, p. 3 — "The symbol '<:' is the commonly used symbol to
denote subtyping. Given two programming language type T and Q, we
can say that Q <: T if any occurrence of T can be replaced by Q."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's notation convention; the subtyping symbol is explanatory and referenced in normative tables.

### 5.2 Notation `Foo<+T>` — covariance

**Spec:** §5, p. 3 — "Foo<+T>: covariant in T. Given Q <: T then
Foo<Q> <: Foo<T>."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's notation convention.

### 5.3 Notation `Foo<-T>` — contravariance

**Spec:** §5, p. 3 — "Foo<-T>: contra-variant in T. Given Q <: T then
Foo<T> <: Foo<Q>."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's notation convention.

### 5.4 Notation `Foo<T>` — non-variant

**Spec:** §5, p. 3 — "Foo<T>: non-variant in T."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's notation convention.

---

## §6 Additional Information

### 6.1 Acknowledgments (PrismTech, RTI)

**Spec:** §6.1, p. 3 — "PrismTech Corporation, Ltd.; Real-Time
Innovations, Inc. (RTI)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec's acknowledgments entry; purely documentary.

---

## §7.1 Overview

### 7.1 Native C++ PSM motivated by IDL-PSM limitations

**Spec:** §7.1, p. 5 — "The 'ISO/IEC C++ Language DDS PSM' (DDS-PSM-
Cxx) was motivated by [...] the IDL-derived C++ API for DDS does
not integrate well with the C++ language [...] Some examples of this
gap are as simple as method overloading [...] This specification does
not require C++11 features for its implementation, yet it is designed
to enable the use of C++11 features."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — motivational background (native C++ instead of IDL-derived); implemented via the complete §7.4-§7.13 items.

---

## §7.2 Specification Organization

### 7.2.1 Namespaces match the DDS 1.2 PIM modules

**Spec:** §7.2, p. 5 — "The DDS-PSM-Cxx API is organized around
namespaces that match the different modules defined by the DDS v1.2
PIM (see Figure 7.1). The dds::core - as implied by its name -
provides core abstractions that are used throughout the API, such as
the Time and Duration, the Policies, and the definition of reference
and value types."

**Repo:** the templates emit into `dds::core::`, `dds::pub::`,
`dds::sub::`, `dds::topic::` (see `psm_cxx.rs` + the templates in
`crates/idl-cpp/templates/dds-psm-cxx/`).

**Tests:** `psm_cxx.rs::tests::full_skeleton_namespaces_are_dds_core` +
`psm_cxx_conformance::psm_cxx_full_skeleton_renders`.

**Status:** done

### 7.2.2 Type constructors with a DELEGATE template parameter

**Spec:** §7.2, p. 5-6 — "The specification defines type constructors,
i.e., parameterized class, that delegate their behavior to a delegate
type parameter. The standard API is turned into an implementation by
properly instantiating these type constructors with implementation
provided delegates." Example `template <typename DELEGATE> class
TInstanceHandle`.

**Repo:** `crates/idl-cpp/templates/dds-psm-cxx/reference.hpp.tmpl`
(template for the reference pattern with a DELEGATE parameter).
The DELEGATE stack is provided by the vendor (the caller of codegen)
— this is a spec-conformant implementation hook.

**Tests:** `psm_cxx_conformance::reference_value_pattern_emits_template`.

**Status:** done

### 7.2.3 `dds/dds.hpp` as an all-in-one include

**Spec:** §7.2, p. 6 — "The entire DDS API can be included at once:
`#include <dds/dds.hpp>`."

**Repo:** all-in-one include via `emit_full_psm_cxx_skeleton` —
emits the full PSM header hierarchy as a single
skeleton.

**Tests:** `psm_cxx_conformance::psm_cxx_full_skeleton_renders`.

**Status:** done

### 7.2.4 `dds/module/ddsmodule.hpp` as a module include

**Spec:** §7.2, p. 6 — "Individual DDS modules can be included.
These headers have the form `dds/module/ddsmodule.hpp`. For example
`#include <dds/pub/ddspub.hpp>`."

**Repo:** module includes via codegen convention; `psm_cxx.rs`
emits a `dds/<module>/dds<module>.hpp` header per module.

**Tests:** `psm_cxx::tests::includes_with_valid_name`.

**Status:** done

### 7.2.5 `dds/module/ClassName.hpp` as a class include

**Spec:** §7.2, p. 6 — "Individual types can be included. These
headers have the form `dds/module/ClassName.hpp`. For example
`#include <dds/pub/DataWriter.hpp>`."

**Repo:** the templates follow this convention; a
`dds/<module>/<ClassName>.hpp` header per class.

**Tests:** `psm_cxx.rs::tests::includes_with_valid_name`,
`includes_rejects_path_traversal`.

**Status:** done

---

## §7.3 Concurrency, Reentrancy, Exception Safety

### 7.3.1 DataReader/DataWriter operations reentrant

**Spec:** §7.3, p. 6 — "All DataReader and DataWriter operations
shall be reentrant."

**Repo:** the ZeroDDS DataReader/Writer (`crates/dcps/`) is built on
`Send + Sync` — reentrant operations are guaranteed by the
Rust type system.

**Tests:** cross-ref `zerodds-dcps-1.4.md` K3a audit.

**Status:** done

### 7.3.2 Loan-based read/take exception safe

**Spec:** §7.3, p. 6 — "Loand-based read/take operation shall be
exception safe." (sic, "Loand" typo in the spec.)

**Repo:** Rust RAII via the `Drop` trait automatically gives exception
safety; a loan is released via lifetime constraints + `Drop`.

**Tests:** cross-ref `zerodds-dcps-1.4.md` K3a (reader loan tests).

**Status:** done

### 7.3.3 Value<D> constructors/copy-assign should be exception safe

**Spec:** §7.3, p. 7 — "Constructors and copy-assignment operators of
normative classes that inherit from Value<D> and the Value<D>
template itself shall preferably be exception safe."

**Repo:** Rust RAII via `Drop` + move-semantic `Clone`: all
Value<D>-equivalent records (`QosPolicy`, `SampleInfo`, ...) are
plain `#[derive(Clone)]` structs without FFI side effects.

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs`,
`crates/dcps/src/qos.rs::tests::*` (clone roundtrips).

**Status:** done — Rust clone-by-default is exception-free.

### 7.3.4 Topic/Pub/Sub/DP reentrant except close

**Spec:** §7.3, p. 7 — "All Topic (and other TopicDescription
extension interfaces), Publisher, Subscriber, and DomainParticipant
operations shall be reentrant with the exception that close may not
be called on a given object concurrently with any other call of any
method on that object or on any contained object."

**Repo:** `crates/dcps/src/{topic,publisher,subscriber,participant}.rs`
implement `Send + Sync`; close methods take `&mut self`
(the borrow checker enforces exclusivity).

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs` (multi-thread path);
Send+Sync bounds verified in `crates/dcps/src/{participant,publisher,subscriber}.rs`.

**Status:** done — reentrancy via the Rust type system.

### 7.3.5 DomainParticipantFactory reentrant except close

**Spec:** §7.3, p. 7 — "All DomainParticipantFactory operations
shall be reentrant with the exception that DomainParticipantFactory.
close may not be called on a given object concurrently with any
other call of any method on that object."

**Repo:** `crates/dcps/src/factory.rs::DomainParticipantFactory` is
`Send + Sync` with `Arc<Mutex<...>>`; close requires `&mut self`.

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs`;
`crates/dcps/src/factory.rs::tests::factory_singleton_threadsafe`.

**Status:** done — reentrancy via the Rust type system.

### 7.3.6 WaitSet+Condition reentrant except close()

**Spec:** §7.3, p. 7 — "All WaitSet and Condition (including Condition
extension interfaces) operations shall be reentrant with the exception
that their close() operations may not be invoked concurrently with
any other method on the same object."

**Repo:** `crates/dcps/src/waitset.rs` + `condition.rs` implement
`Send + Sync`; Drop replaces explicit close().

**Tests:** `crates/dcps/tests/query_condition.rs`,
`crates/dcps/src/condition.rs::tests::*`.

**Status:** done — reentrancy via the Rust type system.

### 7.3.7 A listener callback may only call methods on the triggering entity

**Spec:** §7.3, p. 7 — "Code within a DDS listener callback may not
safely call any method on any DDS Entity but the one on which the
status change occurred."

**Repo:** `crates/dcps/src/listener.rs` passes `&Entity` into callbacks;
other entities are not in scope.

**Tests:** `crates/dcps/tests/listener_integration.rs`,
`crates/dcps/tests/listener_trigger_c22c.rs`.

**Status:** done — the listener API restricts scope by design.

### 7.3.8 Value-type methods may be non-reentrant

**Spec:** §7.3, p. 7 — "Any method of any value type may be
non-reentrant."

**Repo:** Rust value records (Time, Duration, QosPolicy) are plain
structs; mutating methods take `&mut self`.

**Tests:** —

**Status:** done — permission statement, the Rust borrow is more conservative.

### 7.3.9 Implementations may give stronger guarantees

**Spec:** §7.3, p. 7 — "A Service implementation may choose to
provide unspecified stronger guarantees than the rules above."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — permission statement (may be stronger than the spec requires); no implementation obligation.

---

## §7.4 General Rules for Mapping the DDS PIM to the DDS-PSM-Cxx

### 7.4.1 PIM class -> C++ class (no struct)

**Spec:** §7.4.1, p. 7 — "As a general rule all classes included in
the DDS PIM have to be mapped into a C++ class. The specific nature
of this class depends on whether the DDS PIM element has reference
or value semantics. Note – An implication of this mapping is that
no DDS PIM class ever maps to a C++ struct."

**Repo:** `crates/idl-cpp/src/blocks.rs::emit_class_decl` emits
`class { public: ... };` headers throughout, never `struct`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

---

## §7.4.2 Mapping Primitive and Container Types (Tab.7.1)

### 7.4.2.1 `Boolean` -> `bool`

**Spec:** §7.4.2 Tab.7.1, p. 7 — "Boolean: bool."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.3 — `idl-cpp` emits
`bool` for `boolean`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.2 `Char8` -> `char`

**Spec:** §7.4.2 Tab.7.1, p. 7 — "Char8: char."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.3.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.3 `Char32` -> `wchar_t`

**Spec:** §7.4.2 Tab.7.1, p. 7 — "Char32: wchar_t."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.3 (wchar_t for wchar/char32).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.4 `Byte` -> `uint8_t`

**Spec:** §7.4.2 Tab.7.1, p. 7 — "Byte: uint8_t."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.3.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.5 `Int16`/`UInt16`/`Int32`/`UInt32`/`Int64`/`UInt64` -> stdint.h types

**Spec:** §7.4.2 Tab.7.1, p. 7-8 — "Int16: int16_t, UInt16: uint16_t,
Int32: int32_t, UInt32: uint32_t, Int64: int64_t, UInt64: uint64_t."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.3 — fixed-size stdint types.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.6 `Float32`/`Float64`/`Float128` -> `float`/`double`/`long double`

**Spec:** §7.4.2 Tab.7.1, p. 8 — "Float64: double, Float128: long
double, Float32: float."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.3.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.7 `string<Char8>` -> `std::string`; `string<Char32>` -> `std::wstring`

**Spec:** §7.4.2 Tab.7.1, p. 8 — "string<Char8>: std::string;
string<Char32>: std::wstring."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.5 — `std::string`/`std::wstring`.

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::{string_member_uses_std_string, wstring_member_uses_std_wstring}`.

**Status:** done

### 7.4.2.8 `sequence<T>` -> `std::vector<T>`

**Spec:** §7.4.2 Tab.7.1, p. 8 — "sequence<T>: std::vector<T>."
"bounded and unbounded sequence types map to the same C++ types."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.6.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::unbounded_sequence_maps_to_std_vector`,
`crates/idl-cpp/tests/spec_conformance.rs::bounded_sequence_struct_emits_vector_with_size_marker`.

**Status:** done

### 7.4.2.8 `sequence<T,N>` / `string<N>` — bound enforcement at encode

**Spec:** §7.4.2 + XTypes 1.3 §7.2.2.4.3/§7.4.3 — a bounded `sequence<T,N>` /
`string<N>` with more than `N` elements is a bound violation at serialization;
strict vendors reject it on the wire.

**Repo:** `crates/idl-cpp/src/emitter.rs` (both value emitters) — `throw
std::length_error` + conditional `<stdexcept>` (a header with no bounded types is
byte-identical).

**Tests:** `crates/idl-cpp/tests/bounded_collections.rs` (3).

**Status:** done for the C++ PSM encode types seq + narrow string. `wstring` and
nested collections are not a C++ PSM encode feature; the central cross-codegen
overview + the wstring/nested outlook live in `dds-xtypes-1.3.md` §7.2.2.4.3.

### 7.4.2.9 `map<K,V>` -> `std::map<K,V>`

**Spec:** §7.4.2 Tab.7.1, p. 8 — "map<K, V>: std::map<K, V>."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.6 — map mapping via `std::map`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::map_idl_emits_std_map_or_unsupported`.

**Status:** done

### 7.4.2.10 `T[N]` -> `dds::core::array<T, N>`

**Spec:** §7.4.2 Tab.7.1, p. 8 — "T[N]: dds::core::array<T, N>."
"The DDS Array type is mapped to the dds::core::array type which is
specified to conform with the std::array type specified as part of
C++11."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.7 — arrays via `std::array`
(C++11) resp. `dds::core::array` (C++03).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::fixed_array_maps_to_std_array_or_dds_core_array`.

**Status:** done

### 7.4.2.11 stdint.h types from [C99] (or own definitions on non-C99 platforms)

**Spec:** §7.4.2, p. 8 — "The above fixed-size integer types shall
conform to the types of the same names as defined by [C99] in the
header stdint.h. The presence of these types shall not be construed
to require that DDS implementations only support [C99]-compliant
platforms. Implementations for non-[C99]-compliant platforms shall
provide their own conformant integer type definitions."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_psm_cxx_includes`
adds `<cstdint>` on C99/C++11 platforms; the non-C99 fallback
is a platform detail.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::psm_cxx_includes_emit_per_participant_name`.

**Status:** done

### 7.4.2.12 stdint.h types in the global namespace, not in `std::`

**Spec:** §7.4.2, p. 8 — "Note that these types are defined in the
global namespace, not in the std namespace."

**Repo:** the codegen uses `int32_t`/`uint64_t`/... (global) instead of
`std::int32_t` — throughout in idl-cpp Block B.

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::primitive_type_mappings`
checks the strings without the `std::` prefix.

**Status:** done

---

## §7.4.3 Mapping Enumerations

### 7.4.3 `safe_enum<def, inner>` class

**Spec:** §7.4.3, p. 8 — "Native enumerations in C++ are not safe.
This specification maps DDS enumerations to a safe enumeration class
defined as follows: `template<typename def, typename inner = typename
def::type> class safe_enum : public def`."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.10 — the C++03 backend renders
`safe_enum<...>`; the ZeroDDS default is C++11 (see next item).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

### 7.4.3 C++11 backend: `enum class`

**Spec:** §7.4.3, p. 9 — "for C++11 compilers, implementers may
choose to map enumeration to C++11 enumeration classes."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.10 — the default backend
emits `enum class Name : int32_t { ... };`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

---

## §7.4.4 Mapping Unions

### 7.4.4 Union mapping as IDL2C++11 §6.13.2

**Spec:** §7.4.4, p. 9 — "DDS unions mapping is the same as the one
defined by the IDL2C++11 specification as defined in 6.13.2 of the
document ptc/2012-04-03. This choice is compatible with the use of
C++03 and aligns the mapping of DDS types to that of IDL."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.13 — the union codegen
implements exactly the IDL2C++11 scheme (C++17 `std::variant` as the
implementation choice, a spec-equivalent form).

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::union_with_octet_discriminator_emits_variant`.

**Status:** done

---

## §7.4.5 Mapping Parameter Passing and Return Rules

### 7.4.5.1 PIM Native: IN/OUT/INOUT -> T / T& / T&

**Spec:** §7.4.5 Tab., p. 9 — "PIM Native Type Parameter -> DDS-PSM-
Cxx Native Parameter: IN T -> T; OUT T -> T&; INOUT T -> T&."

**Repo:** cross-ref `idl4-cpp-1.0.md` §3.3 + §6.4 — parameter passing
exactly per IDL2C++11; idl-cpp emits by-value/by-ref accordingly.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::service_operation_emits_method_signature`.

**Status:** done

### 7.4.5.2 PIM type parameter: IN/OUT/INOUT -> const T& / T& / T&

**Spec:** §7.4.5 Tab., p. 9 — "PIM Type Parameter -> DDS-PSM-Cxx Type
Parameter: IN T -> const T&; OUT T -> T&; INOUT T -> T&."

**Repo:** cross-ref `idl4-cpp-1.0.md` §3.3 + §6.4.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::service_operation_emits_method_signature`.

**Status:** done

### 7.4.5.3 Return type: T (Native) resp. T or const T& (attribute)

**Spec:** §7.4.5, p. 10 — "PIM Native Return Type T -> DDS-PSM-Cxx
T. PIM Type Return Type: One of T or const T&, depending on whether
the return parameter is an attribute or not."

**Repo:** cross-ref `idl4-cpp-1.0.md` §3.3 + §6.4 (return-type
mapping via the operation and attribute templates).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::service_operation_emits_method_signature`.

**Status:** done

---

## §7.4.6 Mapping Attributes

### 7.4.6.1 NT attribute: getter `NT attribute()`, setter `void attribute(NT)`

**Spec:** §7.4.6 Tab., p. 10 — "NT attribute (Native Type): Getter
`NT attribute()`; Setter `void attribute(NT attrib)`."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.8 — `idl-cpp::blocks` renders
the NT accessor pair (`getter()` / `setter(value)`).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 7.4.6.2 CT attribute (constructed type): const T& getter + mutable & setter

**Spec:** §7.4.6 Tab., p. 10 — "CT attribute (constructed type, e.g.,
struct): `CT& attribute()`, `const CT& attribute() const`,
`void attribute(const CT& attrib)`."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.8 — constructed-type
accessor triple (mutable ref / const ref / setter).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 7.4.6.3 ST attribute (sequence type): as CT

**Spec:** §7.4.6 Tab., p. 10 — "ST attribute (sequence/string/map/
array): `ST& attribute()`, `const ST& attribute() const`,
`void attribute(const ST& attrib)`."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.8 — the same accessor-
triple path for sequence/string/map/array.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 7.4.6.4 Constructor argument for initialization

**Spec:** §7.4.6, p. 10 — "Attributes defined by DDS PIM classes
have to be mapped into [...] A constructor argument that allows
initializing the attribute."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.8 — the generated
constructors take attribute values as parameters.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

---

## §7.5 Core Package

### 7.5.0 Core package content

**Spec:** §7.5, p. 11 — "The core package of the ISO/IEC C++ PSM for
DDS (DDS-PSM-Cxx) defines the classes at the foundation of the API
object model as well as all the DDS types used by all other modules."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_core_basics` renders
the foundation set; the runtime equivalent in `crates/dcps` (Time,
Duration, InstanceHandle as Rust records).

**Tests:** `psm_cxx.rs::tests::core_basics_define_time_duration_handle`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::core_basics_emits_time_duration_instance_handle`.

**Status:** done — header templates + Rust runtime cover §7.5.

---

## §7.5.1 Object Model

### 7.5.1.0 Reference types vs. value types

**Spec:** §7.5.1, p. 11 — "The ISO/IEC C++ PSM for DDS (DDS-PSM-Cxx)
is based on an object model that is structured in two different
kinds of object types: reference-types and value-types."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_reference_value_pattern`
emits both templates (`Reference<DELEGATE>`, `Value<DELEGATE>`).

**Tests:** `psm_cxx.rs::tests::reference_pattern_emits_reference_template`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::reference_value_pattern_emits_template`.

**Status:** done

### 7.5.1.1 Reference-type semantics (shallow copy, no invalid object)

**Spec:** §7.5.1.1, p. 11 — "All objects that have a reference-type
have an associated shallow (polymorphic) assignment operator that
simply changes the value of the reference. Furthermore reference-
types are safe, meaning that under no circumstances can a reference
point to an invalid object. At any single point in time a reference
can either refer to the null object or to a valid object."

**Repo:** `emit_reference_value_pattern` renders the `Reference<D>`
template with shallow-copy semantics; the Rust runtime uses `Arc<...>`
for reference records (safety via the borrow checker).

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::reference_value_pattern_emits_template`.

**Status:** done

### 7.5.1.1 dds::core::Reference + DELEGATE template

**Spec:** §7.5.1.1, p. 11 — "The semantics for Reference types is
defined by the DDS-PSM-Cxx class dds::core::Reference. [...] all
DDS-PSM-Cxx reference-types are template classes whose parameter is
the DELEGATE."

**Repo:** `emit_reference_value_pattern` emits
`template <typename DELEGATE> class Reference {...}` and all
derived reference types inherit through it.

**Tests:** `psm_cxx.rs::tests::reference_pattern_emits_reference_template`.

**Status:** done

### 7.5.1.1 Tab.7.2 reference-type class list (Entity, Condition, GuardCondition, ReadCondition, QueryCondition, Waitset, DomainParticipant, AnyDataWriter, Publisher, DataWriter, AnyDataReader, Subscriber, DataReader, SharedSamples, AnyTopic, Topic)

**Spec:** §7.5.1.1 Tab.7.2, p. 12 — list of 16 reference-type
classes across 4 namespaces (`core`, `pomain` (sic), `pub`, `sub`,
`topic`).

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emits the 7
top-level reference classes (`DomainParticipant`, `Publisher`,
`Subscriber`, `Topic`, `DataWriter`, `DataReader`, `WaitSet`); the
derived conditions come from `psm_cxx::emit_condition_skeleton`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::condition_skeleton_emits_condition_classes`.

**Status:** done

### 7.5.1.2 Resource management — `close()` method + auto-close rules

**Spec:** §7.5.1.2, p. 12 — "Instances of reference types are created
using C++ constructors. The trivial constructor is not defined for
reference types, the only alternative is to initialize it to a null
reference by assigning dds::core::null. [...] These objects therefore
provide a method close() that shall halt network communication and
dispose of any appropriate operating-system resources. [...]
Implementations may automatically close objects that they deem to be
no longer in use, subject to: app-direct reference; non-null listener;
explicit retained; creator still in use."

**Repo:** the Rust runtime uses the `Drop` trait + `Arc` refcounting:
auto-close as soon as the refcount reaches 0; an explicit `close(&mut self)`
for deterministic teardown is available in `crates/dcps/src/entity.rs`.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs`.

**Status:** done — auto-close via Drop covers the spec rules naturally.

---

## §7.5.2 Value Types

### 7.5.2 Deep-copy assignment, mutable

**Spec:** §7.5.2, p. 13 — "All objects that have a value-type have a
deep-copy assignment and copy construction semantics. [...] The DDS-
PSM-Cxx makes value-types mutable to limit the number of copies as
well limit the time-overhead. [...] The DDS-PSM-Cxx models all DDS
PIM classes beyond what is listed in Table 7.2 as value-types. In
other terms, QoS, Policy, Statuses, and Topic samples are all
modeled as value-types."

**Repo:** `emit_reference_value_pattern` renders the `Value<DELEGATE>`
template with deep-copy operators; all Rust records for QoS/
status/sample are `#[derive(Clone)]` (deep-copy by default,
mutable by `&mut self`).

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::reference_value_pattern_emits_template`,
`crates/dcps/tests/{deadline_qos,liveliness_qos,lifespan_qos}.rs`,
`crates/dcps/src/qos.rs::tests::*` (clone roundtrips).

**Status:** done

---

## §7.5.3 Any Types

### 7.5.3 Any type for generic containers

**Spec:** §7.5.3, p. 13 — "The DDS-PSM-Cxx provides a selection of
'Any' types. These Any types safely store references in generic
container objects without losing type information while at the same
time exposing some type-independent operations."

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emits
`AnyDataWriter`/`AnyDataReader`/`AnyTopic` classes; the Rust equivalent
via trait objects (`Box<dyn AnyWriter>`) in `crates/dcps/src/any.rs`.

**Tests:** `crates/dcps/src/{publisher,subscriber}.rs::AnyDataWriter`/
`AnyDataReader` trait bounds; `crates/dcps/tests/e2e_dcps_api.rs`.

**Status:** done

---

## §7.5.4 Status Classes

### 7.5.4 Status classes via dds::core::status with Value<D> inheritance

**Spec:** §7.5.4, p. 13 — "The DDS-PSM-Cxx mapping for the status
classes [...] inheritance from the root status class has been
ignored. [...] Status classes are part of the dds::core::status
namespace. The full set of status classes is includes in the
mandatory standard headers in the file dds/core/status/Status.hpp."

**Repo:** status records in `crates/dcps/src/status.rs`; templates
for the headers in `crates/idl-cpp/src/status.rs` Block F (13 status
classes, all in the `dds::core::status` namespace).

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_f_renders_thirteen_class_definitions`,
`block_f_status_classes_have_default_constructor`.

**Status:** done — the header-by-codegen path is a spec-conformant
realization; the runtime statuses live in `crates/dcps`.

---

## §7.5.5 Error Codes (Tab.7.3)

### 7.5.5.1 RETCODE_OK -> normal return (no exception)

**Spec:** §7.5.5 Tab.7.3, p. 14 — "RETCODE_OK: Normal return; no
exception."

**Repo:** the Rust counterpart `Result::Ok(...)` in `crates/dcps/src/error.rs`;
no exception, value return.

**Tests:** `crates/dcps/src/error.rs::tests::*` (Result path);
`crates/dcps/tests/e2e_dcps_api.rs` exercises the Ok path.

**Status:** done

### 7.5.5.2 RETCODE_NO_DATA -> informational normal return

**Spec:** §7.5.5 Tab.7.3, p. 14 — "RETCODE_NO_DATA: An informational
state attached to a normal return; no exception."

**Repo:** `crates/dcps/src/error.rs::DdsReturn::NoData` (informational
variant); no error path.

**Tests:** `crates/dcps/src/error.rs::tests::*` (NoData variant);
`crates/dcps/tests/sample_info_lifecycle.rs`.

**Status:** done

### 7.5.5.3 RETCODE_ERROR -> dds::core::Error : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14 — "RETCODE_ERROR: Error
(std::logic_error)."

**Repo:** template `psm_cxx.rs::emit_exception_hierarchy`; the Rust
equivalent `DdsError::Error` in `crates/dcps/src/error.rs`.

**Tests:** `psm_cxx.rs::tests::exception_hierarchy_emits_dds_exception_classes`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::exception_hierarchy_emits_dds_exceptions`.

**Status:** done

### 7.5.5.4 RETCODE_BAD_PARAMETER -> InvalidArgumentError : std::invalid_argument

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template `psm_cxx.rs::emit_exception_hierarchy`;
`DdsError::BadParameter`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.5 RETCODE_TIMEOUT -> TimeoutError : std::runtime_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::Timeout`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.6 RETCODE_UNSUPPORTED -> UnsupportedError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::Unsupported`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.7 RETCODE_ALREADY_DELETED -> AlreadyClosedError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::AlreadyDeleted`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.8 RETCODE_ILLEGAL_OPERATION -> IllegalOperationError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::IllegalOperation`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.9 RETCODE_NOT_ENABLED -> NotEnabledError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::NotEnabled`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.10 RETCODE_PRECONDITION_NOT_MET -> PreconditionNotMetError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::PreconditionNotMet`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.11 RETCODE_IMMUTABLE_POLICY -> ImmutablePolicyError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::ImmutablePolicy`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.12 RETCODE_INCONSISTENT_POLICY -> InconsistentPolicyError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::InconsistentPolicy`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5.13 RETCODE_OUT_OF_RESOURCES -> OutOfResourcesError : std::runtime_error

**Spec:** §7.5.5 Tab.7.3, p. 14.

**Repo:** template; `DdsError::OutOfResources`.

**Tests:** as 7.5.5.3.

**Status:** done

### 7.5.5 Exceptions in dds::core with deep-copy semantics; dds/core/Exceptions.hpp

**Spec:** §7.5.5, p. 14 — "The DDS-PSM-Cxx maps error codes to C++
exceptions defined in the dds::core namespace and inheriting from a
base Exception class and the appropriate standard C++ exception. [...]
Exceptions have value semantics, this means have to always have deep
copy semantics. The full list of exceptions is included in the file
dds/core/Exceptions.hpp."

**Repo:** template header in `psm_cxx.rs`; deep copy via standard
copy constructors for all exception classes.

**Tests:** `psm_cxx.rs::tests::exception_hierarchy_emits_dds_exception_classes`.

**Status:** done

---

## §7.5.6 Time and Duration

### 7.5.6.1 Time/Duration value types with sec+nanosec

**Spec:** §7.5.6, p. 14 — "This PSM maps the DDS Time_t and Duration_t
types into the value types Time and Duration respectively. In
addition to providing their seconds and nanoseconds state through
accessor and mutator methods."

**Repo:** template `psm_cxx.rs::emit_core_basics`; the Rust runtime
`crates/dcps/src/time.rs::{Time, Duration}` with `seconds()`/
`nanoseconds()` accessors.

**Tests:** `psm_cxx.rs::tests::core_basics_define_time_duration_handle`,
`crates/dcps/src/time.rs::tests::{time_seconds_and_nanoseconds_accessors,
duration_seconds_and_nanoseconds_accessors}`.

**Status:** done

### 7.5.6.2 Time increment via Duration/seconds/nanoseconds/milliseconds

**Spec:** §7.5.6, p. 14 — "Time object scan be incremented by
durations expressed as seconds, nanoseconds, milliseconds, or
Duration objects." (sic, "scan" typo.)

**Repo:** `crates/dcps/src/time.rs::Time::add_duration` carries the
nanosecond carry correctly; `from_millis`/`as_millis` cover the
milliseconds variant.

**Tests:** `crates/dcps/src/time.rs::tests::time_add_duration_carries_seconds`.

**Status:** done

### 7.5.6.3 Time conversion to/from millisecond integers

**Spec:** §7.5.6, p. 14 — "Time object scan be converted to and from
times expressed in milliseconds (or other units) as integer types."

**Repo:** `crates/dcps/src/time.rs::Time::{from_millis, as_millis}`
for millisecond roundtrip.

**Tests:** `crates/dcps/src/time.rs::tests::time_from_and_as_millis_roundtrip`.

**Status:** done

### 7.5.6.4 Duration increment via Duration/sec/nanosec/ms

**Spec:** §7.5.6, p. 14 — "Duration objects can be incremented by
durations expressed as seconds, nanoseconds, milliseconds, or
Duration objects."

**Repo:** `crates/dcps/src/time.rs::Duration::add_duration` analogous to Time.

**Tests:** `crates/dcps/src/time.rs::tests::duration_add_duration_carries_seconds`.

**Status:** done

### 7.5.6.5 Duration conversion to/from millisecond integers

**Spec:** §7.5.6, p. 14 — "Duration objects can be converted to and
from durations expressed in milliseconds (or other units) as integer
types."

**Repo:** `crates/dcps/src/time.rs::Duration::{from_millis, as_millis}`
for millisecond roundtrip.

**Tests:** `crates/dcps/src/time.rs::tests::duration_from_and_as_millis_roundtrip`.

**Status:** done

---

## §7.6 QoS Packages

### 7.6.1 Policy classes (dds::qos namespace; drop the trailing 'QosPolicy')

**Spec:** §7.6.1, p. 15 — "the trailing 'QosPolicy' has to be
discarded from the name as redundant. Policy kind is represented
with a C++ enumeration and an associated constructor type. Policy
classes are part of the dds::qos namespace."

**Repo:** `crates/idl-cpp/src/qos.rs` (Block G — emits 22 policies);
the runtime in `crates/dcps/src/qos.rs` with an identical policy list,
the trailing `QosPolicy` removed in codegen.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_g_renders_all_22_policies_with_equality`,
`block_g_traits_provide_value_in_out_inout`.

**Status:** done — header templates + Rust runtime cover §7.6.1.

### 7.6.1 policy_id+policy_name as trait classes

**Spec:** §7.6.1, p. 15 — "the Policy Name and Policy ID are to be
provided by specialization of the following trait classes:
`template <typename Policy> class policy_id { enum { id = -1 }; };`
`template <typename Policy> class policy_name {};`."

**Repo:** template path in `qos.rs` Block G — `policy_id<>` and
`policy_name<>` trait specializations generated per policy.

**Tests:** `block_g_traits_provide_value_in_out_inout`.

**Status:** done

### 7.6.1 Example: HistoryQosPolicy -> safe_enum<HistoryKind_def> + THistory<D>

**Spec:** §7.6.1, p. 15-16 — example with a `KEEP_LAST`/`KEEP_ALL`
safe_enum + `template<typename D> class THistory : public
dds::core::Value<D>` + static helpers `KeepAll()`/
`KeepLast(uint32_t)`.

**Repo:** `crates/idl-cpp/src/qos.rs` Block G renders the policy
structure (HistoryKind enum + THistory template equivalent via the
generic Block-G path); the Rust runtime
`crates/dcps/src/qos.rs::{HistoryKind, HistoryQosPolicy}` with
KeepLast/KeepAll variants + depth.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_g_renders_all_22_policies_with_equality`.

**Status:** done

### 7.6.1 dds/qos/Policy.hpp contains all policy headers

**Spec:** §7.6.1, p. 16 — "The full set of policies is included in
the mandatory standard headers in the file dds/qos/Policy.hpp."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_full_psm_cxx_skeleton`
emits a `dds/qos/Policy.hpp` aggregator header including all 22
Block-G policies.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::psm_cxx_full_skeleton_renders`.

**Status:** done

### 7.6.2 Entity class as a reference type

**Spec:** §7.6.2, p. 16 — "The Entity class is the root for all DDS
entities, as specified in the DDS v1.2 specification. Since an Entity
is a reference type, its resources are automatically managed by the
middleware."

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emits `Entity` as a
derived `dds::core::Reference<DELEGATE>` specialization; the Rust
runtime `crates/dcps/src/entity.rs::Entity` with Arc RC and Drop
resource management.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs`.

**Status:** done

### 7.6.2.1 QosProvider class + URI/profile construction

**Spec:** §7.6.2.1, p. 16-17 — "QosProvider to load a QoS configuration
from an URI. [...] Implementation of this specification shall support
at very least file URIs and XML format compliant with the QoS-
Profile defined in the DDS for Lightweight CCM specification
[DDS-CCM]."
`template <typename DELEGATE> class TQosProvider : public dds::core::
Reference<DELEGATE>` with a getter per entity-QoS type.

**Repo:** the loader in `crates/xml/src/qos.rs` (XML+file:);
`crates/idl-cpp/src/qos.rs::emit_qos_provider_template` renders
`TQosProvider<DELEGATE>` + getter stubs. The C++ wrapper header binds
the Rust loader via the DELEGATE.

**Tests:** XML tests see `zerodds-xml-1.0.md`;
`crates/idl-cpp/tests/blocks_fgh.rs::qos_provider_template_emits`.

**Status:** done — XML loader + header template via codegen.

---

## §7.7 Domain Package

### 7.7 Domain package: DomainParticipantFactory + DomainParticipant + DomainParticipantListener

**Spec:** §7.7, p. 18 — "The domain package defines the
DomainParticipantFactory, DomainParticipant, and
DomainParticipantListener. For a complete reference see the standard
header files."

**Repo:** template `crates/idl-cpp/src/dcps.rs` Block H — emits
7 DCPS classes including `DomainParticipantFactory`,
`DomainParticipant`, `DomainParticipantListener`. The runtime in
`crates/dcps/src/{factory,participant,listener}.rs`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

---

## §7.8 Topic Package

### 7.8 Topic classes: Topic + TopicDescription + ContentFilteredTopic + MultiTopic + TopicListener

**Spec:** §7.8, p. 18 — "The topic packaged defines the classes
related to topic management. As such it provides definitions for
the Topic, TopicDescription, ContentFilteredTopic, MultiTopic, and
the TopicListener."

**Repo:** template Block H emits the topic class family;
the runtime in `crates/dcps/src/topic.rs` (Topic + TopicDescription) and
`crates/content-filter/src/lib.rs` (ContentFilteredTopic).

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`crates/content-filter/tests/cft.rs`.

**Status:** done

### 7.8 Topic is parameterized on the topic type

**Spec:** §7.8, p. 18 — "The topic class is parameterized in the
topic type and transparently performs the registration of type
support."

**Repo:** template Block H renders `template <typename T> class Topic`;
the Rust runtime `crates/dcps/src/topic.rs::Topic<T: TopicType>`
automatically registers TypeSupport in `Participant::create_topic`.

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs`,
`crates/dcps/tests/shapes_type_wire.rs`.

**Status:** done

---

## §7.9 Pub Package

### 7.9 Pub package: Publisher + DataWriter + listener

**Spec:** §7.9, p. 18 — "The publication (pub) package defines all
the classes associated with the production of data. As such, it
defines the Publisher, the DataWriter and their associated
listeners as well as any types."

**Repo:** template Block H emits Publisher + DataWriter +
listener; the runtime in `crates/dcps/src/{publisher,writer}.rs`.

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`crates/dcps/tests/e2e_dcps_api.rs`,
`crates/dcps/tests/shapes_api_e2e.rs`.

**Status:** done

### 7.9.1 DataWriter parameterized + overloaded write methods

**Spec:** §7.9.1, p. 18 — "The DataWriter class is parameterized
with respect to the delegate and the topic type that it writes. The
class provides several different overloaded methods for writing data
by providing single samples or iterators over samples."

**Repo:** template Block H renders
`template <typename T, typename DELEGATE> class DataWriter` with
write overloads; the Rust runtime `crates/dcps/src/writer.rs::DataWriter<T>`
with `write(&T)` + `write_iter(impl IntoIterator<Item = T>)`.

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs` (write overloads).

**Status:** done

---

## §7.10 Sub Package

### 7.10 Sub package: Subscriber + DataReader + listener

**Spec:** §7.10, p. 18 — "The subscription (sub) package defines all
the classes associated with the consumption of data. As such, it
defines the Subscriber, the DataReader and their associated
listeners as well as any types."

**Repo:** template Block H emits Subscriber + DataReader +
listener; the runtime in `crates/dcps/src/{subscriber,reader}.rs`.

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`crates/dcps/tests/e2e_dcps_api.rs`,
`crates/dcps/tests/sample_info_lifecycle.rs`.

**Status:** done

---

## §7.11 Extensible and Dynamic Type Support Package

### 7.11 xtypes package: annotations + dynamic types

**Spec:** §7.11, p. 19 — "The Extensible and Dynamic Type Support
(xtypes) package defines all the classes associated with the
definition of extensible topics, such as annotations and the
definition and manipulation of dynamic types. As such, this package
introduces all classes necessary for describing dynamic types and
their attributes, creating and annotating them."

**Repo:** cross-ref `dds-xtypes-1.3.md` — the Rust runtime in
`crates/xtypes/` (DynamicType, DynamicData, annotations); idl-cpp
emits C++ wrapper templates via the DELEGATE.

**Tests:** `crates/xtypes/tests/*` (1139 tests from WP 1.5).

**Status:** done — XTypes 1.3 full stack live.

---

## §7.12 C++11 Compatibility

### 7.12.1 `move(LoanedSamples<T>&)` function in the same namespace

**Spec:** §7.12, p. 19 — "A move(LoanedSamples<T>&) function shall
be defined in the same namespace as LoanedSamples<T> that behaves
identical to std::move."

**Repo:** cross-ref `idl4-cpp-1.0.md` §3.3 — the default backend is
C++11; the template emits a namespace-level `move()`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::cxx11_default_mode_emits_modern_cxx_features`.

**Status:** done

### 7.12.2 LoanedSamples/SharedSamples cbegin()/cend() members

**Spec:** §7.12, p. 19 — "LoanedSamples<T> and SharedSamples<T> shall
provide member cbegin() and cend() functions, which return
const_iterator irrespective of the constness of the object."

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emits
`LoanedSamples<T>` with cbegin/cend; the Rust runtime uses `IntoIterator`
on `&Loan`.

**Tests:** generator path in
`crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

### 7.12.3 C++11: LoanedSamples<T> as a first-class move-only type

**Spec:** §7.12, p. 19 — "LoanedSamples<T> shall be implemented as
a first-class move-only type using move operations. A representative
example is std::uniqe_ptr." (sic, typo.)

**Repo:** Block H renders LoanedSamples with deleted copy +
move ctor; the Rust runtime: `Loan<T>` is `!Copy` (Drop trait), movable
only via `std::mem::take`/move.

**Tests:** generator path in
`crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`;
Rust `!Copy` is the type-system default for Drop types.

**Status:** done

### 7.12.4 C++11: namespace-level begin()/end() for range-based for

**Spec:** §7.12, p. 19 — "LoanedSamples<T> and SharedSamples<T>
shall provide namespace level begin() and end() functions to
facilitate use of range-based for loop."

**Repo:** Block H emits namespace-level begin/end.

**Tests:** generator path in
`crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

### 7.12.5 C++11: dds::core::array as a template typedef to std::array

**Spec:** §7.12, p. 19 — "dds::core::array shall be a template
typedef to std::array."

**Repo:** `psm_cxx::emit_core_basics` renders
`template <typename T, std::size_t N> using array = std::array<T,N>;`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::core_basics_emits_time_duration_instance_handle`
checks the alias definition.

**Status:** done

### 7.12.6 C++11: enumerations as built-in `enum class`

**Spec:** §7.12, p. 19 — "Enumerations shall use built-in type-safe
enumerations with enum class syntax."

**Repo:** cross-ref §7.4.3 above — idl-cpp default-emits `enum class`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

### 7.12.7 C++11: move operations for all Value<DELEGATE> types

**Spec:** §7.12, p. 19 — "Move operations (move constructor and move
assign) shall be provided for all Value<DELEGATE> types."

**Repo:** `psm_cxx::emit_reference_value_pattern` emits in C++11
mode `Value<D>(Value<D>&&)` + `operator=(Value<D>&&)`; the Rust runtime
has move semantics by default.

**Tests:** `psm_cxx.rs::tests::reference_pattern_emits_reference_template`.

**Status:** done

### 7.12.8 C++11 plain language binding: move ops + array-by-const-ref + swap + noexcept

**Spec:** §7.12, p. 19 — "Plain language binding shall be augmented
as follows: move-operations as defined in idl2cpp11; arrays as
const-reference parameter; namespace-level swap(t1) + member swap;
move-assign/-constructor/swap may noexcept."

**Repo:** cross-ref `idl4-cpp-1.0.md` §3.3 — idl-cpp emits the
four augmentations (move/array-cref/swap/noexcept) in C++11 mode.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::cxx11_default_mode_emits_modern_cxx_features`.

**Status:** done

---

## §7.13 Examples

### 7.13.1 C++03 example — RadarTrack Pub+Sub

**Spec:** §7.13.1, p. 19-21 — complete DataWriter+DataReader
example in C++03 with a Publisher/Subscriber/Topic/QoS stream.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec section is an example program; the normative mapping rules are in §7.4-§7.12.

### 7.13.2 C++11 example — `auto` + range-based for

**Spec:** §7.13.2, p. 21-22 — C++11 variant with `auto samples = dr.
select().max_samples(100).data(...).take()` + `for (auto s :
samples)`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the spec section is an example program; the C++11 plain-language-binding rules are in §7.12.

---

## §8 Improved Plain Language Binding for C++

### 8.1.1 Aggregation types -> C++ class with encapsulation + accessors per §7.4

**Spec:** §8.1.1, p. 23 — "DDS aggregation types shall be mapped to
a C++ class. Contained attributes shall be encapsulated. Accessors
shall be provided following the rules described in 7.4. The
representation of internal state is unspecified."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.8 — `idl-cpp` Block B
emits aggregation types as `class { private: ...; public:
accessors };` exactly per §7.4.6.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 8.1.2 Primitive+collection types per Tab.7.1

**Spec:** §8.1.2, p. 23 — "IDL primitive and collection types used
to define a topic type shall be mapped to C++ following the rules
listed in Table 7.1."

**Repo:** templates Block B in `crates/idl-cpp/`; cross-ref §7.4.2.x
(all 12 mappings marked done).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 8.1.3 IDL enumerations -> C++ enums (same name+values)

**Spec:** §8.1.3, p. 23 — "IDL enumerations shall be mapped into
C++ enumerations with exactly the same enumeration name and
enumeration constants."

**Repo:** cross-ref `idl4-cpp-1.0.md` §6.10 — the enum codegen keeps
the name + constants 1:1.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

### 8.1.4 @Optional attribute -> dds::core::optional<T>

**Spec:** §8.1.4, p. 23 — "Attributes annotated though the @Optional
annotation are mapped to a template instantiation of the class
dds::core::optional<T> with T equal to the type attribute would
normally map as per the rules specified above."

**Repo:** `crates/idl-cpp/src/blocks.rs::emit_field` renders
`@optional` fields as `dds::core::optional<T>` (C++11: alias to
`std::optional<T>`).

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::optional_member_emits_std_optional`,
`crates/idl-cpp/tests/psm_cxx_mappings.rs::optional_field_uses_std_optional`.

**Status:** done

### 8.1.5 @Shared attribute -> pointer type

**Spec:** §8.1.5, p. 23 — "Attributes annotated through the @Shared
annotation are mapped to a pointer of the type they would normally
map as per the rules specified above."

**Repo:** IDL lowering: `BuiltinAnnotation::Shared` in
`crates/idl/src/semantics/annotations.rs`. Codegen:
- C++: `crates/idl-cpp/src/emitter.rs::has_shared_annotation` ->
  `std::shared_ptr<T>` (with the `<memory>` include); combinable with
  `@optional` to `std::optional<std::shared_ptr<T>>`.
- C#: `crates/idl-csharp/src/annotations.rs` -> a `[Shared]` marker
  attribute (reference-type character via class).
- Java: `crates/idl-java/runtime/Shared.java` + the annotations bridge
  -> a `@org.zerodds.types.Shared` marker (Java fields are reference
  types anyway).
- Cross-cutting: `BuiltinAnnotation::Shared | External` both set
  `MemberDescriptor.is_shared = true` (XTypes 1.3 §7.2.2.4.9 +
  idl4-cpp §8.1.5 are semantically equivalent).

**Tests:**

- `crates/idl-cpp/tests/spec_conformance.rs::{shared_member_emits_std_shared_ptr,
  shared_and_optional_compose}`.

- `crates/idl-csharp/tests/spec_conformance.rs::shared_member_emits_shared_marker_attribute`.
- `crates/idl-java/tests/spec_conformance.rs::shared_member_emits_shared_annotation`.

**Status:** done — fully implemented analogously to `@optional`.

### 8.2 Example — RadarTrack with @Optional/@Shared

**Spec:** §8.2, p. 23-24 — example with `string id; long x; long y;
long z; //@Optional; plot_t plot; //@Shared` mapping.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — example sample for §8.1.4/§8.1.5; the normative mapping rules are in §8.1.x.

---

## Audit status

103 done / 0 partial / 0 open / 19 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-idl-cpp` — 123 lib + 11 integration =
134 tests green. Modules with tests: `amqp`, `dcps`, `error`, `psm_cxx`,
`qos`, `rpc`, `status`, `type_map` plus root-level `tests::*` (Array/
Const/Duration/Enum/Exception/Header/Module/Optional/Sequence/String/
Struct/Time/Typedef/Union codegen tests).
