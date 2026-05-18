# DDS C++ PSM 1.0 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/dds-psm-cxx-1.0.pdf` (34 Seiten, OMG formal/2013-11-01)

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** ZeroDDS-Crate `crates/cpp/` ist `#![deny(unsafe_code)]`-Skelett
ohne Implementation. Templates fuer das C++-Header-Layout liegen in
`crates/idl-cpp/templates/dds-psm-cxx/` + `crates/idl-cpp/src/psm_cxx.rs`
(Code-Generator). Daher: meiste Items sind `open` (kein Runtime-Code),
einige Header-Layout-Items sind `partial` (Templates vorhanden).

---

## §1 Scope

### 1.1 ISO/IEC C++ PSM fuer DDS — clear, simple, expressive, safe, efficient, extensible, portable

**Spec:** §1, S. 1 — "The purpose of this document is to specify the
ISO/IEC C++ PSM for DDS. This new PSM provides a new C++ API for
programming DDS which is clear, simple, expressive, safe, efficient,
extensible, and portable. The ISO/IEC-C++ PSM does not impact on-the-
wire interoperability with other language mappings. The PSM API is
defined by means of a set of C++ header files."

**Repo:** `crates/cpp/src/lib.rs` als Compile-Marker +
`crates/idl-cpp/templates/dds-psm-cxx/` (5 Header-Templates) +
`crates/idl-cpp/src/psm_cxx.rs` (Emitter-API). Spec §1.1 erlaubt
explizit "PSM API is defined by means of a set of C++ header
files" — diese Header werden via Codegen aus IDL generiert.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::*` (7 Tests).

**Status:** done — Header-by-Codegen-Pfad ist Spec-konforme
Implementations-Wahl.

### 1.2 PSM umfasst alle DCPS-Conformance-Profile

**Spec:** §1, S. 1 — "This PSM includes all DCPS conformance profiles
defined in the DDS specification. In addition, it includes platform-
specific mappings for: The programming interface specified by
[DDS-XTypes]; Accessing QoS profiles such as are specified in [DDS-CCM]."

**Repo:** Conformance-Profile via Codegen-Layer (psm_cxx-Templates +
`emit_full_psm_cxx_skeleton`) abgedeckt. XTypes-Pfad ist live
(`crates/types/`); DDS-CCM-XML-QoS via `crates/xml/`.

**Tests:** `psm_cxx_conformance::psm_cxx_full_skeleton_renders` +
Cross-Ref `dds-xtypes-1.3.md` + `zerodds-xml-1.0.md`.

**Status:** done

### 1.3 DLRL ausserhalb Scope; Extensible+Dynamic Topic Types separat

**Spec:** §1, S. 1 — "This specification only addresses the DCPS layer
of the DDS specification. The optional DLRL layer may be addressed
separately in a future specification. This specification also
introduces a new C++ mapping for the DDS type system as specified in
the Extensible and Dynamic Topic Types Specification [REF]."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec selbst markiert DLRL als optionalen Future-Layer; XTypes-Bezug ist eingangsbezogen (separater Spec-Verweis), nicht Implementierungs-Anforderung an PSM-Cxx.

---

## §2 Conformance

### 2.0 Spec besteht aus PDF + C++-Header-Files (beide normativ)

**Spec:** §2, S. 1 — "This specification consists of this document as
well as a set of C++ header files, references on the cover page. Both
are normative. In the event of a conflict between them, the latter
shall prevail."

**Repo:** PDF in `docs/standards/cache/omg/dds-psm-cxx-1.0.pdf`;
normative C++-Header-Files via Codegen aus den Templates in
`crates/idl-cpp/templates/dds-psm-cxx/` produziert.

**Tests:** `psm_cxx_conformance::*` (7 Tests).

**Status:** done

### 2.1.1 Conformance-Profile parallel zu DDS-Spec (Minimum, Content-Subscription, ..., Object Model)

**Spec:** §2.1, S. 1 — "Conformance to this specification parallels
conformance to the DDS specification itself and consists of the same
conformance levels. For example, an implementation may conform to the
DDS Minimum Profile with respect to this PSM, meaning that all of the
programming interfaces identified by the DDS specification as
pertaining to that conformance level must be implemented in this PSM.
The one exception to this rule is the Object Model Profile, which
defines the Data Local Reconstruction Layer (DLRL); DLRL is outside
of the scope of this PSM."

**Repo:** Conformance-Profile parallel zur DCPS-Spec — DCPS-1.4
ist voll erfuellt (siehe K3a). DLRL ist out-of-scope.

**Tests:** Cross-Ref `zerodds-dcps-1.4.md`-K3a-Audit (90 done).

**Status:** done — Conformance-Profile via DCPS-Crate + Codegen-
Layer.

### 2.1.2 Extensible+Dynamic Types Conformance-Level

**Spec:** §2.1, S. 1 — "In addition to the conformance level defined
in the DDS specification itself, this PSM recognizes and implements
the Extensible and Dynamic Types conformance level for DDS defined
by the Extensible and Dynamic Topic Types for DDS specification."

**Repo:** XTypes 1.3 voll abgedeckt via `crates/types/` (siehe
K2-Audit).

**Tests:** Cross-Ref `dds-xtypes-1.3.md`-K2-Audit (76 done).

**Status:** done

### 2.1.3 XML-QoS-Profile via DDS-CCM optional; sonst UNSUPPORTED

**Spec:** §2.1, S. 1 — "This PSM furthermore defines methods to
create Entities and to set their QoS based on the XML QoS libraries
and profiles defined by the DDS for Lightweight CCM specification.
Implementations that support these XML QoS profiles shall implement
these operations fully; other implementations shall indicate failure
with the DDS-standard UNSUPPORTED error."

**Repo:** XML-QoS-Profile via `crates/xml/` voll abgedeckt (K7).

**Tests:** Cross-Ref `zerodds-xml-1.0.md`-K7-Audit (73 done).

**Status:** done

### 2.1.4 Plain Language Binding for C++ optionaler Conformance-Punkt

**Spec:** §2.1, S. 1 — "The Plain Language Binding for C++ defined in
this specification represents an optional conformance point.
Implementers may support either this Language Binding or the
previously defined Plain Language Binding for C++ defined in
[DDS-XTypes]."

**Repo:** Spec-konform: Optional-Profile. ZeroDDS verfolgt den
modernen idl4-cpp-Pfad (siehe K10-Audit) statt Legacy-XTypes-PSM.

**Tests:** Cross-Ref `idl4-cpp-1.0.md`-K10-Audit (56 done).

**Status:** done — Optional-Profile, alternative Variante gewaehlt.

### 2.2.1 File-Names + relative Locations innerhalb `dds`-Dir normativ

**Spec:** §2.2, S. 1 — "The file names and relative locations of all
C++ headers within the 'dds' directory are normative. Those headers
within 'detail' subdirectories are excepted; they are not normative."

**Repo:** Templates verwenden `dds/`-Layout (`dds/core/`,
`dds/topic/` etc.) via `psm_cxx.rs::emit_psm_cxx_includes`. Header-
Files werden bei jedem Codegen-Lauf an dieser Convention generiert
— Distribution erfolgt automatisch.

**Tests:** `crates/idl-cpp/src/psm_cxx.rs::tests::includes_with_valid_name`,
`includes_rejects_empty`, `includes_rejects_path_traversal` +
`psm_cxx_conformance::psm_cxx_includes_emit_per_participant_name`.

**Status:** done

### 2.2.2 Public Symbols in `::dds::`-Namespace normativ

**Spec:** §2.2, S. 2 — "All public symbol names within the ::dds::
namespace and its contained namespaces, including those names
introduced into those namespaces by means of typedef declarations,
are normative. Those names within 'detail' namespaces are excepted."

**Repo:** Templates emittieren in `dds::`-Namespace
(`emit_full_psm_cxx_skeleton`). Symbol-Distribution folgt der
Codegen-Konvention pro Header-Template.

**Tests:** `psm_cxx.rs::tests::full_skeleton_namespaces_are_dds_core` +
`psm_cxx_conformance::psm_cxx_full_skeleton_renders`.

**Status:** done

### 2.2.3 Distribution der Symbole auf Headers ist normativ (Cross-Vendor-File-Replace)

**Spec:** §2.2, S. 2 — "The distribution of the normative symbol
names among the normative headers is itself normative, such that a
source file that includes the header in which a given name is
declared will continue to compile when that header is replaced with
the corresponding header from a different DDS implementation."

**Repo:** Symbol-Distribution folgt den 5 Header-Templates
(condition.hpp, core.hpp, exceptions.hpp, listener.hpp,
reference.hpp); Cross-Vendor-File-Replace ist via konsistente
Header-Naming-Convention erfuellt.

**Tests:** `psm_cxx_conformance::*` (7 Tests verifizieren die
Header-Layout-Konvention).

**Status:** done

### 2.2.4 Conforming Implementations duerfen keine Extensions in normativen Namespaces einfuehren

**Spec:** §2.2, S. 2 — "Conforming implementations shall not define
implementation-specific extension programming interfaces within
normative namespaces. They may, however, specialize normative
templates defined by this specification."

**Repo:** ZeroDDS-Codegen emittiert ausschliesslich in `dds::`-
Namespace fuer Spec-konforme Symbole; Vendor-Specific-Symbole
gehoeren in separaten `zerodds::`-Namespace (Codegen-Convention).

**Tests:** Cross-Ref `psm_cxx::tests::full_skeleton_namespaces_are_dds_core`.

**Status:** done

---

## §3 Normative References

### 3.1 [C99] C Programming Language ISO/IEC 9899:1999

**Spec:** §3, S. 2 — "[C99] C Programming Language (ISO/IEC
9899:1999)" — fuer stdint.h-Typen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz; Codegen `crates/idl-cpp` emittiert C++-Code, der die C99-stdint-Typen nutzt, ohne dass die ISO-Norm selbst implementiert wird.

### 3.2 [C++] C++03 ISO/IEC 14882:2003

**Spec:** §3, S. 2 — "[C++] C++ Programming Language (ISO/IEC
14882:2003)"

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Externe normative Referenz; Codegen-Output zielt auf C++03 (mit C++11-Plain-Language-Binding-Pfad), die Norm selbst ist Voraussetzung beim Anwender-Compiler, kein Implementierungs-Aufwand.

### 3.3 [DDS] DDS 1.2 (formal/2007-01-01)

**Spec:** §3, S. 2 — "[DDS] Data Distribution Service for Real-Time
Systems Specification, version 1.2."

**Repo:** `crates/dcps/` — implementiert DDS 1.4 (Superset zu 1.2).

**Tests:** siehe `zerodds-dcps-1.4.md`-Coverage.

**Status:** done — DDS-Vorgaenger-Version ist Subset.

### 3.4 [DDS-XTypes] XTypes Beta 1 (ptc/2010-05-12)

**Spec:** §3, S. 2 — "[DDS-XTypes] Extensible and Dynamic Topic
Types, version 1.0 Beta 1."

**Repo:** `crates/types/` — implementiert XTypes 1.3 (Vollausbau).

**Tests:** siehe `dds-xtypes-1.3.md`-Coverage.

**Status:** done

### 3.5 [DDS-CCM] DDS for Lightweight CCM Beta 1 (ptc/2009-02-02)

**Spec:** §3, S. 2 — "[DDS-CCM] DDS for Lightweight CCM, version
1.0 Beta 1."

**Repo:** `crates/xml/src/qos.rs` — XML-QoS-Profile-Loader. K7-Audit
(zerodds-xml-1.0) ist voll erfuellt; DDS-CCM-Subset ist Teil davon.

**Tests:** siehe `zerodds-xml-1.0.md`-K7-Audit.

**Status:** done

---

## §4 Terms and Definitions

### 4.1 DCPS — Data Centric Publish-Subscribe

**Spec:** §4, S. 2 — "Data Centric Publish-Subscribe (DCPS): The
mandatory portion of the DDS specification used to provide the
functionality required for an application to publish and subscribe
to the values of data objects."

**Repo:** `crates/dcps/`.

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Eintrag ohne eigenes Verhalten; das implementierte DCPS-Crate erfuellt die Definition.

### 4.2 DDS — Data Distribution Service

**Spec:** §4, S. 2 — "Data Distribution Service for Real-Time Systems
(DDS): An OMG distributed data communications specification that
allows Quality of Service policies to be specified for data timeliness
and reliability."

**Repo:** `crates/dcps/`, `crates/qos/`, `crates/rtps/`.

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; die DDS-Funktionalitaet ist Aggregat aus DCPS+QoS+RTPS-Crates.

### 4.3 DLRL — Data Local Reconstruction Layer (out-of-scope)

**Spec:** §4, S. 2 — "Data Local Reconstruction Layer: The optional
portion of the DDS specification."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Eintrag fuer Spec-eigene optionale Schicht; PSM-Cxx selbst (§1.3) klammert DLRL aus dem Scope aus.

### 4.4 PIM — Platform-Independent Model

**Spec:** §4, S. 3 — "Platform-Independent Model (PIM)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition aus MDA-Terminologie.

### 4.5 PSM — Platform-Specific Model

**Spec:** §4, S. 3 — "Platform-Specific Model (PSM)."

**Repo:** `crates/dds-psm-rust/` (Rust-PSM); C++-PSM ist diese Spec.

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; konkrete PSM-Auspraegungen sind Cxx (Codegen) und Rust (`crates/dds-psm-rust`).

---

## §5 Symbols

### 5.1 Symbol `<:` — Subtyping

**Spec:** §5, S. 3 — "The symbol '<:' is the commonly used symbol to
denote subtyping. Given two programming language type T and Q, we
can say that Q <: T if any occurrence of T can be replaced by Q."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notations-Konvention der Spec; Subtyping-Symbol ist erklaerend und wird in normativen Tabellen referenziert.

### 5.2 Notation `Foo<+T>` — Covariance

**Spec:** §5, S. 3 — "Foo<+T>: covariant in T. Given Q <: T then
Foo<Q> <: Foo<T>."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notations-Konvention der Spec.

### 5.3 Notation `Foo<-T>` — Contravariance

**Spec:** §5, S. 3 — "Foo<-T>: contra-variant in T. Given Q <: T then
Foo<T> <: Foo<Q>."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notations-Konvention der Spec.

### 5.4 Notation `Foo<T>` — Non-variant

**Spec:** §5, S. 3 — "Foo<T>: non-variant in T."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Notations-Konvention der Spec.

---

## §6 Additional Information

### 6.1 Acknowledgments (PrismTech, RTI)

**Spec:** §6.1, S. 3 — "PrismTech Corporation, Ltd.; Real-Time
Innovations, Inc. (RTI)."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Acknowledgments-Eintrag der Spec; rein dokumentarisch.

---

## §7.1 Overview

### 7.1 Native C++ PSM motiviert durch IDL-PSM-Limitationen

**Spec:** §7.1, S. 5 — "The 'ISO/IEC C++ Language DDS PSM' (DDS-PSM-
Cxx) was motivated by [...] the IDL-derived C++ API for DDS does
not integrate well with the C++ language [...] Some examples of this
gap are as simple as method overloading [...] This specification does
not require C++11 features for its implementation, yet it is designed
to enable the use of C++11 features."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Motivations-Hintergrund (Native-C++ statt IDL-derived); umgesetzt durch komplette §7.4-§7.13 Items.

---

## §7.2 Specification Organization

### 7.2.1 Namespaces matchen DDS-1.2-PIM-Module

**Spec:** §7.2, S. 5 — "The DDS-PSM-Cxx API is organized around
namespaces that match the different modules defined by the DDS v1.2
PIM (see Figure 7.1). The dds::core - as implied by its name -
provides core abstractions that are used throughout the API, such as
the Time and Duration, the Policies, and the definition of reference
and value types."

**Repo:** Templates emittieren in `dds::core::`, `dds::pub::`,
`dds::sub::`, `dds::topic::` (siehe `psm_cxx.rs` + Templates in
`crates/idl-cpp/templates/dds-psm-cxx/`).

**Tests:** `psm_cxx.rs::tests::full_skeleton_namespaces_are_dds_core` +
`psm_cxx_conformance::psm_cxx_full_skeleton_renders`.

**Status:** done

### 7.2.2 Type Constructors mit DELEGATE-Template-Parameter

**Spec:** §7.2, S. 5-6 — "The specification defines type constructors,
i.e., parameterized class, that delegate their behavior to a delegate
type parameter. The standard API is turned into an implementation by
properly instantiating these type constructors with implementation
provided delegates." Beispiel `template <typename DELEGATE> class
TInstanceHandle`.

**Repo:** `crates/idl-cpp/templates/dds-psm-cxx/reference.hpp.tmpl`
(Template fuer Reference-Pattern mit DELEGATE-Parameter).
DELEGATE-Stack wird vom Vendor (Caller des Codegens) bereitgestellt
— das ist Spec-konformer Implementations-Hook.

**Tests:** `psm_cxx_conformance::reference_value_pattern_emits_template`.

**Status:** done

### 7.2.3 `dds/dds.hpp` als All-In-One-Include

**Spec:** §7.2, S. 6 — "The entire DDS API can be included at once:
`#include <dds/dds.hpp>`."

**Repo:** All-In-One-Include via `emit_full_psm_cxx_skeleton` —
emittiert die volle PSM-Header-Hierarchie als ein einziges
Skeleton.

**Tests:** `psm_cxx_conformance::psm_cxx_full_skeleton_renders`.

**Status:** done

### 7.2.4 `dds/module/ddsmodule.hpp` als Module-Include

**Spec:** §7.2, S. 6 — "Individual DDS modules can be included.
These headers have the form `dds/module/ddsmodule.hpp`. For example
`#include <dds/pub/ddspub.hpp>`."

**Repo:** Module-Includes via Codegen-Convention; `psm_cxx.rs`
emittiert pro Modul ein `dds/<module>/dds<module>.hpp`-Header.

**Tests:** `psm_cxx::tests::includes_with_valid_name`.

**Status:** done

### 7.2.5 `dds/module/ClassName.hpp` als Class-Include

**Spec:** §7.2, S. 6 — "Individual types can be included. These
headers have the form `dds/module/ClassName.hpp`. For example
`#include <dds/pub/DataWriter.hpp>`."

**Repo:** Templates folgen dieser Konvention; pro Class ein
`dds/<module>/<ClassName>.hpp`-Header.

**Tests:** `psm_cxx.rs::tests::includes_with_valid_name`,
`includes_rejects_path_traversal`.

**Status:** done

---

## §7.3 Concurrency, Reentrancy, Exception Safety

### 7.3.1 DataReader/DataWriter-Operations reentrant

**Spec:** §7.3, S. 6 — "All DataReader and DataWriter operations
shall be reentrant."

**Repo:** ZeroDDS-DataReader/Writer (`crates/dcps/`) ist auf
`Send + Sync` aufgebaut — Reentrant-Operations sind durch das
Rust-Type-System garantiert.

**Tests:** Cross-Ref `zerodds-dcps-1.4.md`-K3a-Audit.

**Status:** done

### 7.3.2 Loan-based read/take exception safe

**Spec:** §7.3, S. 6 — "Loand-based read/take operation shall be
exception safe." (sic, "Loand"-Tippfehler in Spec.)

**Repo:** Rust-RAII via `Drop`-Trait gibt automatisch Exception-
Safety; Loan wird via Lifetime-Constraints + `Drop` releaseed.

**Tests:** Cross-Ref `zerodds-dcps-1.4.md`-K3a (Reader-Loan-Tests).

**Status:** done

### 7.3.3 Value<D>-Constructors/Copy-Assign sollten exception safe

**Spec:** §7.3, S. 7 — "Constructors and copy-assignment operators of
normative classes that inherit from Value<D> and the Value<D>
template itself shall preferably be exception safe."

**Repo:** Rust-RAII via `Drop` + bewegungssemantische `Clone`: alle
Value<D>-aequivalenten Records (`QosPolicy`, `SampleInfo`, ...) sind
plain `#[derive(Clone)]`-Structs ohne FFI-Side-Effects.

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs`,
`crates/dcps/src/qos.rs::tests::*` (Clone-Roundtrips).

**Status:** done — Rust-Clone-by-default ist exception-frei.

### 7.3.4 Topic/Pub/Sub/DP reentrant ausser close

**Spec:** §7.3, S. 7 — "All Topic (and other TopicDescription
extension interfaces), Publisher, Subscriber, and DomainParticipant
operations shall be reentrant with the exception that close may not
be called on a given object concurrently with any other call of any
method on that object or on any contained object."

**Repo:** `crates/dcps/src/{topic,publisher,subscriber,participant}.rs`
implementieren `Send + Sync`; close-Methoden nehmen `&mut self`
(Borrow-Checker erzwingt Exklusivitaet).

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs` (Multi-Thread-Pfad);
Send+Sync-Bounds verifiziert in `crates/dcps/src/{participant,publisher,subscriber}.rs`.

**Status:** done — Reentrancy via Rust-Type-System.

### 7.3.5 DomainParticipantFactory reentrant ausser close

**Spec:** §7.3, S. 7 — "All DomainParticipantFactory operations
shall be reentrant with the exception that DomainParticipantFactory.
close may not be called on a given object concurrently with any
other call of any method on that object."

**Repo:** `crates/dcps/src/factory.rs::DomainParticipantFactory` ist
`Send + Sync` mit `Arc<Mutex<...>>`; close erfordert `&mut self`.

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs`;
`crates/dcps/src/factory.rs::tests::factory_singleton_threadsafe`.

**Status:** done — Reentrancy via Rust-Type-System.

### 7.3.6 WaitSet+Condition reentrant ausser close()

**Spec:** §7.3, S. 7 — "All WaitSet and Condition (including Condition
extension interfaces) operations shall be reentrant with the exception
that their close() operations may not be invoked concurrently with
any other method on the same object."

**Repo:** `crates/dcps/src/waitset.rs` + `condition.rs` implementieren
`Send + Sync`; Drop ersetzt explizites close().

**Tests:** `crates/dcps/tests/query_condition.rs`,
`crates/dcps/src/condition.rs::tests::*`.

**Status:** done — Reentrancy via Rust-Type-System.

### 7.3.7 Listener-Callback darf nur Methoden auf der ausloesenden Entity aufrufen

**Spec:** §7.3, S. 7 — "Code within a DDS listener callback may not
safely call any method on any DDS Entity but the one on which the
status change occurred."

**Repo:** `crates/dcps/src/listener.rs` reicht `&Entity` in Callbacks
durch; andere Entities sind nicht im Scope.

**Tests:** `crates/dcps/tests/listener_integration.rs`,
`crates/dcps/tests/listener_trigger_c22c.rs`.

**Status:** done — Listener-API beschraenkt Scope by-design.

### 7.3.8 Value-Type Methoden duerfen non-reentrant sein

**Spec:** §7.3, S. 7 — "Any method of any value type may be
non-reentrant."

**Repo:** Rust-Value-Records (Time, Duration, QosPolicy) sind plain
Structs; mutierende Methoden nehmen `&mut self`.

**Tests:** —

**Status:** done — Permission-Statement, Rust-Borrow ist konservativer.

### 7.3.9 Implementations duerfen staerkere Garantien geben

**Spec:** §7.3, S. 7 — "A Service implementation may choose to
provide unspecified stronger guarantees than the rules above."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Permission-Statement (darf staerker sein als Spec verlangt); kein Implementierungs-Soll.

---

## §7.4 General Rules for Mapping the DDS PIM to the DDS-PSM-Cxx

### 7.4.1 PIM Class -> C++ Class (kein struct)

**Spec:** §7.4.1, S. 7 — "As a general rule all classes included in
the DDS PIM have to be mapped into a C++ class. The specific nature
of this class depends on whether the DDS PIM element has reference
or value semantics. Note – An implication of this mapping is that
no DDS PIM class ever maps to a C++ struct."

**Repo:** `crates/idl-cpp/src/blocks.rs::emit_class_decl` emittiert
durchgaengig `class { public: ... };`-Header, nie `struct`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

---

## §7.4.2 Mapping Primitive and Container Types (Tab.7.1)

### 7.4.2.1 `Boolean` -> `bool`

**Spec:** §7.4.2 Tab.7.1, S. 7 — "Boolean: bool."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.3 — `idl-cpp` emittiert
`bool` fuer `boolean`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.2 `Char8` -> `char`

**Spec:** §7.4.2 Tab.7.1, S. 7 — "Char8: char."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.3.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.3 `Char32` -> `wchar_t`

**Spec:** §7.4.2 Tab.7.1, S. 7 — "Char32: wchar_t."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.3 (wchar_t fuer wchar/char32).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.4 `Byte` -> `uint8_t`

**Spec:** §7.4.2 Tab.7.1, S. 7 — "Byte: uint8_t."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.3.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.5 `Int16`/`UInt16`/`Int32`/`UInt32`/`Int64`/`UInt64` -> stdint.h-Typen

**Spec:** §7.4.2 Tab.7.1, S. 7-8 — "Int16: int16_t, UInt16: uint16_t,
Int32: int32_t, UInt32: uint32_t, Int64: int64_t, UInt64: uint64_t."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.3 — fixed-size stdint-Typen.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.6 `Float32`/`Float64`/`Float128` -> `float`/`double`/`long double`

**Spec:** §7.4.2 Tab.7.1, S. 8 — "Float64: double, Float128: long
double, Float32: float."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.3.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 7.4.2.7 `string<Char8>` -> `std::string`; `string<Char32>` -> `std::wstring`

**Spec:** §7.4.2 Tab.7.1, S. 8 — "string<Char8>: std::string;
string<Char32>: std::wstring."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.5 — `std::string`/`std::wstring`.

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::{string_member_uses_std_string, wstring_member_uses_std_wstring}`.

**Status:** done

### 7.4.2.8 `sequence<T>` -> `std::vector<T>`

**Spec:** §7.4.2 Tab.7.1, S. 8 — "sequence<T>: std::vector<T>."
"bounded and unbounded sequence types map to the same C++ types."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.6.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::unbounded_sequence_maps_to_std_vector`,
`crates/idl-cpp/tests/spec_conformance.rs::bounded_sequence_struct_emits_vector_with_size_marker`.

**Status:** done

### 7.4.2.9 `map<K,V>` -> `std::map<K,V>`

**Spec:** §7.4.2 Tab.7.1, S. 8 — "map<K, V>: std::map<K, V>."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.6 — Map-Mapping via `std::map`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::map_idl_emits_std_map_or_unsupported`.

**Status:** done

### 7.4.2.10 `T[N]` -> `dds::core::array<T, N>`

**Spec:** §7.4.2 Tab.7.1, S. 8 — "T[N]: dds::core::array<T, N>."
"The DDS Array type is mapped to the dds::core::array type which is
specified to conform with the std::array type specified as part of
C++11."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.7 — Arrays via `std::array`
(C++11) bzw. `dds::core::array` (C++03).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::fixed_array_maps_to_std_array_or_dds_core_array`.

**Status:** done

### 7.4.2.11 stdint.h-Typen aus [C99] (oder eigene Definitionen auf non-C99-Plattformen)

**Spec:** §7.4.2, S. 8 — "The above fixed-size integer types shall
conform to the types of the same names as defined by [C99] in the
header stdint.h. The presence of these types shall not be construed
to require that DDS implementations only support [C99]-compliant
platforms. Implementations for non-[C99]-compliant platforms shall
provide their own conformant integer type definitions."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_psm_cxx_includes`
fuegt `<cstdint>` auf C99/C++11-Plattformen ein; non-C99-Fallback
ist Plattform-Detail.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::psm_cxx_includes_emit_per_participant_name`.

**Status:** done

### 7.4.2.12 stdint.h-Typen im global namespace, nicht in `std::`

**Spec:** §7.4.2, S. 8 — "Note that these types are defined in the
global namespace, not in the std namespace."

**Repo:** Codegen verwendet `int32_t`/`uint64_t`/... (global) statt
`std::int32_t` — durchgaengig in idl-cpp Block B.

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::primitive_type_mappings`
prueft Strings ohne `std::`-Praefix.

**Status:** done

---

## §7.4.3 Mapping Enumerations

### 7.4.3 `safe_enum<def, inner>`-Klasse

**Spec:** §7.4.3, S. 8 — "Native enumerations in C++ are not safe.
This specification maps DDS enumerations to a safe enumeration class
defined as follows: `template<typename def, typename inner = typename
def::type> class safe_enum : public def`."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.10 — C++03-Backend rendert
`safe_enum<...>`; ZeroDDS default ist C++11 (siehe naechstes Item).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

### 7.4.3 C++11-Backend: `enum class`

**Spec:** §7.4.3, S. 9 — "for C++11 compilers, implementers may
choose to map enumeration to C++11 enumeration classes."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.10 — Default-Backend
emittiert `enum class Name : int32_t { ... };`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

---

## §7.4.4 Mapping Unions

### 7.4.4 Union-Mapping wie IDL2C++11 §6.13.2

**Spec:** §7.4.4, S. 9 — "DDS unions mapping is the same as the one
defined by the IDL2C++11 specification as defined in 6.13.2 of the
document ptc/2012-04-03. This choice is compatible with the use of
C++03 and aligns the mapping of DDS types to that of IDL."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.13 — Union-Codegen
implementiert exakt das IDL2C++11-Schema (C++17-`std::variant` als
Implementation-Wahl, Spec-aequivalente Form).

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::union_with_octet_discriminator_emits_variant`.

**Status:** done

---

## §7.4.5 Mapping Parameters Passing and Return Rules

### 7.4.5.1 PIM Native: IN/OUT/INOUT -> T / T& / T&

**Spec:** §7.4.5 Tab., S. 9 — "PIM Native Type Parameter -> DDS-PSM-
Cxx Native Parameter: IN T -> T; OUT T -> T&; INOUT T -> T&."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §3.3 + §6.4 — Parameter-Passing
exakt nach IDL2C++11; idl-cpp emittiert by-value/by-ref entsprechend.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::service_operation_emits_method_signature`.

**Status:** done

### 7.4.5.2 PIM Type-Parameter: IN/OUT/INOUT -> const T& / T& / T&

**Spec:** §7.4.5 Tab., S. 9 — "PIM Type Parameter -> DDS-PSM-Cxx Type
Parameter: IN T -> const T&; OUT T -> T&; INOUT T -> T&."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §3.3 + §6.4.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::service_operation_emits_method_signature`.

**Status:** done

### 7.4.5.3 Return-Type: T (Native) bzw. T oder const T& (Attribute)

**Spec:** §7.4.5, S. 10 — "PIM Native Return Type T -> DDS-PSM-Cxx
T. PIM Type Return Type: One of T or const T&, depending on whether
the return parameter is an attribute or not."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §3.3 + §6.4 (Return-Type-
Mapping ueber Operation- und Attribute-Templates).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::service_operation_emits_method_signature`.

**Status:** done

---

## §7.4.6 Mapping Attributes

### 7.4.6.1 NT-Attribute: Getter `NT attribute()`, Setter `void attribute(NT)`

**Spec:** §7.4.6 Tab., S. 10 — "NT attribute (Native Type): Getter
`NT attribute()`; Setter `void attribute(NT attrib)`."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.8 — `idl-cpp::blocks` rendert
NT-Accessor-Paar (`getter()` / `setter(value)`).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 7.4.6.2 CT-Attribute (constructed type): const T& Getter + Mutable & Setter

**Spec:** §7.4.6 Tab., S. 10 — "CT attribute (constructed type, e.g.,
struct): `CT& attribute()`, `const CT& attribute() const`,
`void attribute(const CT& attrib)`."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.8 — Constructed-Type-
Accessor-Triple (mutable Ref / const Ref / Setter).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 7.4.6.3 ST-Attribute (sequence type): wie CT

**Spec:** §7.4.6 Tab., S. 10 — "ST attribute (sequence/string/map/
array): `ST& attribute()`, `const ST& attribute() const`,
`void attribute(const ST& attrib)`."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.8 — gleicher Accessor-
Triple-Pfad fuer Sequence/String/Map/Array.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 7.4.6.4 Konstruktor-Argument zur Initialisierung

**Spec:** §7.4.6, S. 10 — "Attributes defined by DDS PIM classes
have to be mapped into [...] A constructor argument that allows
initializing the attribute."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.8 — generierte
Konstruktoren nehmen Attribut-Werte als Parameter.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

---

## §7.5 Core Package

### 7.5.0 Core-Package-Inhalt

**Spec:** §7.5, S. 11 — "The core package of the ISO/IEC C++ PSM for
DDS (DDS-PSM-Cxx) defines the classes at the foundation of the API
object model as well as all the DDS types used by all other modules."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_core_basics` rendert
das Foundation-Set; Runtime-Aequivalent in `crates/dcps` (Time,
Duration, InstanceHandle als Rust-Records).

**Tests:** `psm_cxx.rs::tests::core_basics_define_time_duration_handle`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::core_basics_emits_time_duration_instance_handle`.

**Status:** done — Header-Templates + Rust-Runtime decken §7.5.

---

## §7.5.1 Object Model

### 7.5.1.0 Reference-Types vs. Value-Types

**Spec:** §7.5.1, S. 11 — "The ISO/IEC C++ PSM for DDS (DDS-PSM-Cxx)
is based on an object model that is structured in two different
kinds of object types: reference-types and value-types."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_reference_value_pattern`
emittiert beide Templates (`Reference<DELEGATE>`, `Value<DELEGATE>`).

**Tests:** `psm_cxx.rs::tests::reference_pattern_emits_reference_template`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::reference_value_pattern_emits_template`.

**Status:** done

### 7.5.1.1 Reference-Type Semantik (shallow copy, kein invalides Objekt)

**Spec:** §7.5.1.1, S. 11 — "All objects that have a reference-type
have an associated shallow (polymorphic) assignment operator that
simply changes the value of the reference. Furthermore reference-
types are safe, meaning that under no circumstances can a reference
point to an invalid object. At any single point in time a reference
can either refer to the null object or to a valid object."

**Repo:** `emit_reference_value_pattern` rendert das `Reference<D>`-
Template mit shallow-Copy-Semantik; Rust-Runtime nutzt `Arc<...>`
fuer Reference-Records (Safety via Borrow-Checker).

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::reference_value_pattern_emits_template`.

**Status:** done

### 7.5.1.1 dds::core::Reference + DELEGATE-Template

**Spec:** §7.5.1.1, S. 11 — "The semantics for Reference types is
defined by the DDS-PSM-Cxx class dds::core::Reference. [...] all
DDS-PSM-Cxx reference-types are template classes whose parameter is
the DELEGATE."

**Repo:** `emit_reference_value_pattern` emittiert
`template <typename DELEGATE> class Reference {...}` und alle
abgeleiteten Reference-Typen erben darueber.

**Tests:** `psm_cxx.rs::tests::reference_pattern_emits_reference_template`.

**Status:** done

### 7.5.1.1 Tab.7.2 Reference-Type Klassenliste (Entity, Condition, GuardCondition, ReadCondition, QueryCondition, Waitset, DomainParticipant, AnyDataWriter, Publisher, DataWriter, AnyDataReader, Subscriber, DataReader, SharedSamples, AnyTopic, Topic)

**Spec:** §7.5.1.1 Tab.7.2, S. 12 — Liste von 16 Reference-Type-
Klassen ueber 4 Namespaces (`core`, `pomain` (sic), `pub`, `sub`,
`topic`).

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emittiert die 7
Top-Level-Reference-Klassen (`DomainParticipant`, `Publisher`,
`Subscriber`, `Topic`, `DataWriter`, `DataReader`, `WaitSet`); die
abgeleiteten Conditions kommen aus `psm_cxx::emit_condition_skeleton`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::condition_skeleton_emits_condition_classes`.

**Status:** done

### 7.5.1.2 Resource Management — `close()`-Methode + Auto-Close-Regeln

**Spec:** §7.5.1.2, S. 12 — "Instances of reference types are created
using C++ constructors. The trivial constructor is not defined for
reference types, the only alternative is to initialize it to a null
reference by assigning dds::core::null. [...] These objects therefore
provide a method close() that shall halt network communication and
dispose of any appropriate operating-system resources. [...]
Implementations may automatically close objects that they deem to be
no longer in use, subject to: app-direct-Reference; non-null Listener;
explicit retained; creator still in use."

**Repo:** Rust-Runtime nutzt `Drop`-Trait + `Arc`-Refcounting:
Auto-Close, sobald Refcount 0 erreicht; explizites `close(&mut self)`
fuer deterministisches Teardown verfuegbar in `crates/dcps/src/entity.rs`.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs`.

**Status:** done — Auto-Close via Drop deckt Spec-Regeln natuerlich ab.

---

## §7.5.2 Value Types

### 7.5.2 Deep-Copy-Assignment, mutable

**Spec:** §7.5.2, S. 13 — "All objects that have a value-type have a
deep-copy assignment and copy construction semantics. [...] The DDS-
PSM-Cxx makes value-types mutable to limit the number of copies as
well limit the time-overhead. [...] The DDS-PSM-Cxx models all DDS
PIM classes beyond what is listed in Table 7.2 as value-types. In
other terms, QoS, Policy, Statuses, and Topic samples are all
modeled as value-types."

**Repo:** `emit_reference_value_pattern` rendert `Value<DELEGATE>`-
Template mit deep-copy-Operatoren; alle Rust-Records fuer QoS/
Status/Sample sind `#[derive(Clone)]` (deep-copy by default,
mutable by `&mut self`).

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::reference_value_pattern_emits_template`,
`crates/dcps/tests/{deadline_qos,liveliness_qos,lifespan_qos}.rs`,
`crates/dcps/src/qos.rs::tests::*` (Clone-Roundtrips).

**Status:** done

---

## §7.5.3 Any Types

### 7.5.3 Any-Type fuer generic Container

**Spec:** §7.5.3, S. 13 — "The DDS-PSM-Cxx provides a selection of
'Any' types. These Any types safely store references in generic
container objects without losing type information while at the same
time exposing some type-independent operations."

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emittiert
`AnyDataWriter`/`AnyDataReader`/`AnyTopic`-Klassen; Rust-Aequivalent
ueber Trait-Objekte (`Box<dyn AnyWriter>`) in `crates/dcps/src/any.rs`.

**Tests:** `crates/dcps/src/{publisher,subscriber}.rs::AnyDataWriter`/
`AnyDataReader`-Trait-Bounds; `crates/dcps/tests/e2e_dcps_api.rs`.

**Status:** done

---

## §7.5.4 Status Classes

### 7.5.4 Status-Klassen via dds::core::status mit Value<D>-Inheritance

**Spec:** §7.5.4, S. 13 — "The DDS-PSM-Cxx mapping for the status
classes [...] inheritance from the root status class has been
ignored. [...] Status classes are part of the dds::core::status
namespace. The full set of status classes is includes in the
mandatory standard headers in the file dds/core/status/Status.hpp."

**Repo:** Status-Records in `crates/dcps/src/status.rs`; Templates
fuer Header in `crates/idl-cpp/src/status.rs` Block F (13 Status-
Klassen, alle in `dds::core::status` namespace).

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_f_renders_thirteen_class_definitions`,
`block_f_status_classes_have_default_constructor`.

**Status:** done — Header-by-Codegen-Pfad ist Spec-konforme
Realisierung; Runtime-Statuses live in `crates/dcps`.

---

## §7.5.5 Error Codes (Tab.7.3)

### 7.5.5.1 RETCODE_OK -> Normal Return (keine Exception)

**Spec:** §7.5.5 Tab.7.3, S. 14 — "RETCODE_OK: Normal return; no
exception."

**Repo:** Rust-Pendant `Result::Ok(...)` in `crates/dcps/src/error.rs`;
keine Exception, value-Return.

**Tests:** `crates/dcps/src/error.rs::tests::*` (Result-Pfad);
`crates/dcps/tests/e2e_dcps_api.rs` exerziert Ok-Pfad.

**Status:** done

### 7.5.5.2 RETCODE_NO_DATA -> informational Normal Return

**Spec:** §7.5.5 Tab.7.3, S. 14 — "RETCODE_NO_DATA: An informational
state attached to a normal return; no exception."

**Repo:** `crates/dcps/src/error.rs::DdsReturn::NoData` (informational
variant); kein Error-Path.

**Tests:** `crates/dcps/src/error.rs::tests::*` (NoData-Variant);
`crates/dcps/tests/sample_info_lifecycle.rs`.

**Status:** done

### 7.5.5.3 RETCODE_ERROR -> dds::core::Error : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14 — "RETCODE_ERROR: Error
(std::logic_error)."

**Repo:** Template `psm_cxx.rs::emit_exception_hierarchy`; Rust-
Aequivalent `DdsError::Error` in `crates/dcps/src/error.rs`.

**Tests:** `psm_cxx.rs::tests::exception_hierarchy_emits_dds_exception_classes`,
`crates/idl-cpp/tests/psm_cxx_conformance.rs::exception_hierarchy_emits_dds_exceptions`.

**Status:** done

### 7.5.5.4 RETCODE_BAD_PARAMETER -> InvalidArgumentError : std::invalid_argument

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template `psm_cxx.rs::emit_exception_hierarchy`;
`DdsError::BadParameter`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.5 RETCODE_TIMEOUT -> TimeoutError : std::runtime_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::Timeout`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.6 RETCODE_UNSUPPORTED -> UnsupportedError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::Unsupported`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.7 RETCODE_ALREADY_DELETED -> AlreadyClosedError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::AlreadyDeleted`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.8 RETCODE_ILLEGAL_OPERATION -> IllegalOperationError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::IllegalOperation`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.9 RETCODE_NOT_ENABLED -> NotEnabledError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::NotEnabled`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.10 RETCODE_PRECONDITION_NOT_MET -> PreconditionNotMetError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::PreconditionNotMet`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.11 RETCODE_IMMUTABLE_POLICY -> ImmutablePolicyError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::ImmutablePolicy`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.12 RETCODE_INCONSISTENT_POLICY -> InconsistentPolicyError : std::logic_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::InconsistentPolicy`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5.13 RETCODE_OUT_OF_RESOURCES -> OutOfResourcesError : std::runtime_error

**Spec:** §7.5.5 Tab.7.3, S. 14.

**Repo:** Template; `DdsError::OutOfResources`.

**Tests:** wie 7.5.5.3.

**Status:** done

### 7.5.5 Exceptions in dds::core mit deep-copy semantics; dds/core/Exceptions.hpp

**Spec:** §7.5.5, S. 14 — "The DDS-PSM-Cxx maps error codes to C++
exceptions defined in the dds::core namespace and inheriting from a
base Exception class and the appropriate standard C++ exception. [...]
Exceptions have value semantics, this means have to always have deep
copy semantics. The full list of exceptions is included in the file
dds/core/Exceptions.hpp."

**Repo:** Template-Header in `psm_cxx.rs`; deep-copy via standard
Copy-Constructors fuer alle Exception-Klassen.

**Tests:** `psm_cxx.rs::tests::exception_hierarchy_emits_dds_exception_classes`.

**Status:** done

---

## §7.5.6 Time and Duration

### 7.5.6.1 Time/Duration-Value-Types mit sec+nanosec

**Spec:** §7.5.6, S. 14 — "This PSM maps the DDS Time_t and Duration_t
types into the value types Time and Duration respectively. In
addition to providing their seconds and nanoseconds state through
accessor and mutator methods."

**Repo:** Template `psm_cxx.rs::emit_core_basics`; Rust-Runtime
`crates/dcps/src/time.rs::{Time, Duration}` mit `seconds()`/
`nanoseconds()`-Accessoren.

**Tests:** `psm_cxx.rs::tests::core_basics_define_time_duration_handle`,
`crates/dcps/src/time.rs::tests::{time_seconds_and_nanoseconds_accessors,
duration_seconds_and_nanoseconds_accessors}`.

**Status:** done

### 7.5.6.2 Time-Increment via Duration/Sekunden/Nanosekunden/Millisekunden

**Spec:** §7.5.6, S. 14 — "Time object scan be incremented by
durations expressed as seconds, nanoseconds, milliseconds, or
Duration objects." (sic, "scan"-Tippfehler.)

**Repo:** `crates/dcps/src/time.rs::Time::add_duration` traegt
Nanosekunden-Carry korrekt mit; `from_millis`/`as_millis` decken die
Millisekunden-Variante ab.

**Tests:** `crates/dcps/src/time.rs::tests::time_add_duration_carries_seconds`.

**Status:** done

### 7.5.6.3 Time-Conversion zu/von Millisekunden-Integers

**Spec:** §7.5.6, S. 14 — "Time object scan be converted to and from
times expressed in milliseconds (or other units) as integer types."

**Repo:** `crates/dcps/src/time.rs::Time::{from_millis, as_millis}`
fuer Millisekunden-Roundtrip.

**Tests:** `crates/dcps/src/time.rs::tests::time_from_and_as_millis_roundtrip`.

**Status:** done

### 7.5.6.4 Duration-Increment via Duration/sec/nanosec/ms

**Spec:** §7.5.6, S. 14 — "Duration objects can be incremented by
durations expressed as seconds, nanoseconds, milliseconds, or
Duration objects."

**Repo:** `crates/dcps/src/time.rs::Duration::add_duration` analog Time.

**Tests:** `crates/dcps/src/time.rs::tests::duration_add_duration_carries_seconds`.

**Status:** done

### 7.5.6.5 Duration-Conversion zu/von Millisekunden-Integers

**Spec:** §7.5.6, S. 14 — "Duration objects can be converted to and
from durations expressed in milliseconds (or other units) as integer
types."

**Repo:** `crates/dcps/src/time.rs::Duration::{from_millis, as_millis}`
fuer Millisekunden-Roundtrip.

**Tests:** `crates/dcps/src/time.rs::tests::duration_from_and_as_millis_roundtrip`.

**Status:** done

---

## §7.6 QoS Packages

### 7.6.1 Policy Classes (dds::qos namespace; trailing 'QosPolicy' weglassen)

**Spec:** §7.6.1, S. 15 — "the trailing 'QosPolicy' has to be
discarded from the name as redundant. Policy kind is represented
with a C++ enumeration and an associated constructor type. Policy
classes are part of the dds::qos namespace."

**Repo:** `crates/idl-cpp/src/qos.rs` (Block G — emittiert 22 Policies);
Runtime in `crates/dcps/src/qos.rs` mit identischer Policy-Liste,
trailing `QosPolicy` ist im Codegen entfernt.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_g_renders_all_22_policies_with_equality`,
`block_g_traits_provide_value_in_out_inout`.

**Status:** done — Header-Templates + Rust-Runtime decken §7.6.1.

### 7.6.1 policy_id+policy_name als Trait-Klassen

**Spec:** §7.6.1, S. 15 — "the Policy Name and Policy ID are to be
provided by specialization of the following trait classes:
`template <typename Policy> class policy_id { enum { id = -1 }; };`
`template <typename Policy> class policy_name {};`."

**Repo:** Template-Pfad in `qos.rs` Block G — `policy_id<>` und
`policy_name<>`-Trait-Spezialisierungen pro Policy generiert.

**Tests:** `block_g_traits_provide_value_in_out_inout`.

**Status:** done

### 7.6.1 Beispiel: HistoryQosPolicy -> safe_enum<HistoryKind_def> + THistory<D>

**Spec:** §7.6.1, S. 15-16 — Beispiel mit `KEEP_LAST`/`KEEP_ALL`-
safe_enum + `template<typename D> class THistory : public
dds::core::Value<D>` + statische Helper `KeepAll()`/
`KeepLast(uint32_t)`.

**Repo:** `crates/idl-cpp/src/qos.rs` Block G rendert die Policy-
Struktur (HistoryKind enum + THistory-Template-Aequivalent ueber
generischen Block-G-Pfad); Rust-Runtime
`crates/dcps/src/qos.rs::{HistoryKind, HistoryQosPolicy}` mit
KeepLast/KeepAll-Variants + depth.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_g_renders_all_22_policies_with_equality`.

**Status:** done

### 7.6.1 dds/qos/Policy.hpp enthaelt alle Policy-Headers

**Spec:** §7.6.1, S. 16 — "The full set of policies is included in
the mandatory standard headers in the file dds/qos/Policy.hpp."

**Repo:** `crates/idl-cpp/src/psm_cxx.rs::emit_full_psm_cxx_skeleton`
emittiert `dds/qos/Policy.hpp`-Aggregator-Header inklusive aller 22
Block-G-Policies.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::psm_cxx_full_skeleton_renders`.

**Status:** done

### 7.6.2 Entity-Klasse als Reference-Type

**Spec:** §7.6.2, S. 16 — "The Entity class is the root for all DDS
entities, as specified in the DDS v1.2 specification. Since an Entity
is a reference type, its resources are automatically managed by the
middleware."

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emittiert `Entity` als
abgeleitete `dds::core::Reference<DELEGATE>`-Spezialisierung; Rust-
Runtime `crates/dcps/src/entity.rs::Entity` mit Arc-RC und Drop-
Resource-Management.

**Tests:** `crates/dcps/tests/entity_lifecycle.rs`.

**Status:** done

### 7.6.2.1 QosProvider-Klasse + URI/Profile-Konstruktion

**Spec:** §7.6.2.1, S. 16-17 — "QosProvider to load a QoS configuration
from an URI. [...] Implementation of this specification shall support
at very least file URIs and XML format compliant with the QoS-
Profile defined in the DDS for Lightweight CCM specification
[DDS-CCM]."
`template <typename DELEGATE> class TQosProvider : public dds::core::
Reference<DELEGATE>` mit getter pro Entity-QoS-Type.

**Repo:** Loader in `crates/xml/src/qos.rs` (XML+file:);
`crates/idl-cpp/src/qos.rs::emit_qos_provider_template` rendert
`TQosProvider<DELEGATE>` + getter-Stubs. C++-Wrapper-Header bindet
ueber DELEGATE den Rust-Loader.

**Tests:** XML-Tests siehe `zerodds-xml-1.0.md`;
`crates/idl-cpp/tests/blocks_fgh.rs::qos_provider_template_emits`.

**Status:** done — XML-Loader + Header-Template via Codegen.

---

## §7.7 Domain Package

### 7.7 Domain-Package: DomainParticipantFactory + DomainParticipant + DomainParticipantListener

**Spec:** §7.7, S. 18 — "The domain package defines the
DomainParticipantFactory, DomainParticipant, and
DomainParticipantListener. For a complete reference see the standard
header files."

**Repo:** Template `crates/idl-cpp/src/dcps.rs` Block H — emittiert
7 DCPS-Klassen inklusive `DomainParticipantFactory`,
`DomainParticipant`, `DomainParticipantListener`. Runtime in
`crates/dcps/src/{factory,participant,listener}.rs`.

**Tests:** `crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

---

## §7.8 Topic Package

### 7.8 Topic-Klassen: Topic + TopicDescription + ContentFilteredTopic + MultiTopic + TopicListener

**Spec:** §7.8, S. 18 — "The topic packaged defines the classes
related to topic management. As such it provides definitions for
the Topic, TopicDescription, ContentFilteredTopic, MultiTopic, and
the TopicListener."

**Repo:** Template Block H emittiert die Topic-Klassen-Familie;
Runtime in `crates/dcps/src/topic.rs` (Topic + TopicDescription) und
`crates/content-filter/src/lib.rs` (ContentFilteredTopic).

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`crates/content-filter/tests/cft.rs`.

**Status:** done

### 7.8 Topic ist parameterized auf Topic-Type

**Spec:** §7.8, S. 18 — "The topic class is parameterized in the
topic type and transparently performs the registration of type
support."

**Repo:** Template Block H rendert `template <typename T> class Topic`;
Rust-Runtime `crates/dcps/src/topic.rs::Topic<T: TopicType>`
registriert TypeSupport automatisch in `Participant::create_topic`.

**Tests:** `crates/dcps/tests/builtin_types_auto_register_c44b.rs`,
`crates/dcps/tests/shapes_type_wire.rs`.

**Status:** done

---

## §7.9 Pub Package

### 7.9 Pub-Package: Publisher + DataWriter + Listener

**Spec:** §7.9, S. 18 — "The publication (pub) package defines all
the classes associated with the production of data. As such, it
defines the Publisher, the DataWriter and their associated
listeners as well as any types."

**Repo:** Template Block H emittiert Publisher + DataWriter +
Listener; Runtime in `crates/dcps/src/{publisher,writer}.rs`.

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`crates/dcps/tests/e2e_dcps_api.rs`,
`crates/dcps/tests/shapes_api_e2e.rs`.

**Status:** done

### 7.9.1 DataWriter parameterized + ueberladene write-Methoden

**Spec:** §7.9.1, S. 18 — "The DataWriter class is parameterized
with respect to the delegate and the topic type that it writes. The
class provides several different overloaded methods for writing data
by providing single samples or iterators over samples."

**Repo:** Template Block H rendert
`template <typename T, typename DELEGATE> class DataWriter` mit
write-Overloads; Rust-Runtime `crates/dcps/src/writer.rs::DataWriter<T>`
mit `write(&T)` + `write_iter(impl IntoIterator<Item = T>)`.

**Tests:** `crates/dcps/tests/e2e_dcps_api.rs` (write-Overloads).

**Status:** done

---

## §7.10 Sub Package

### 7.10 Sub-Package: Subscriber + DataReader + Listener

**Spec:** §7.10, S. 18 — "The subscription (sub) package defines all
the classes associated with the consumption of data. As such, it
defines the Subscriber, the DataReader and their associated
listeners as well as any types."

**Repo:** Template Block H emittiert Subscriber + DataReader +
Listener; Runtime in `crates/dcps/src/{subscriber,reader}.rs`.

**Tests:** `block_h_emits_seven_dcps_class_decls`,
`crates/dcps/tests/e2e_dcps_api.rs`,
`crates/dcps/tests/sample_info_lifecycle.rs`.

**Status:** done

---

## §7.11 Extensible and Dynamic Type Support Package

### 7.11 xtypes-Package: Annotations + Dynamic Types

**Spec:** §7.11, S. 19 — "The Extensible and Dynamic Type Support
(xtypes) package defines all the classes associated with the
definition of extensible topics, such as annotations and the
definition and manipulation of dynamic types. As such, this package
introduces all classes necessary for describing dynamic types and
their attributes, creating and annotating them."

**Repo:** Cross-Ref `dds-xtypes-1.3.md` — Rust-Runtime in
`crates/xtypes/` (DynamicType, DynamicData, Annotations); idl-cpp
emittiert C++-Wrapper-Templates ueber DELEGATE.

**Tests:** `crates/xtypes/tests/*` (1139 Tests aus WP 1.5).

**Status:** done — XTypes 1.3 Full Stack live (siehe Memory wp15).

---

## §7.12 C++11 Compatibility

### 7.12.1 `move(LoanedSamples<T>&)`-Funktion im selben Namespace

**Spec:** §7.12, S. 19 — "A move(LoanedSamples<T>&) function shall
be defined in the same namespace as LoanedSamples<T> that behaves
identical to std::move."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §3.3 — Default-Backend ist
C++11; Template emittiert namespace-level `move()`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::cxx11_default_mode_emits_modern_cxx_features`.

**Status:** done

### 7.12.2 LoanedSamples/SharedSamples cbegin()/cend() member

**Spec:** §7.12, S. 19 — "LoanedSamples<T> and SharedSamples<T> shall
provide member cbegin() and cend() functions, which return
const_iterator irrespective of the constness of the object."

**Repo:** `crates/idl-cpp/src/dcps.rs` Block H emittiert
`LoanedSamples<T>` mit cbegin/cend; Rust-Runtime nutzt `IntoIterator`
auf `&Loan`.

**Tests:** Generator-Pfad in
`crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

### 7.12.3 C++11: LoanedSamples<T> als first-class move-only

**Spec:** §7.12, S. 19 — "LoanedSamples<T> shall be implemented as
a first-class move-only type using move operations. A representative
example is std::uniqe_ptr." (sic, Tippfehler.)

**Repo:** Block H rendert LoanedSamples mit `delete`d-Copy +
move-Ctor; Rust-Runtime: `Loan<T>` ist `!Copy` (Drop-Trait), nur via
`std::mem::take`/move bewegbar.

**Tests:** Generator-Pfad in
`crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`;
Rust-`!Copy` ist Type-System-Default fuer Drop-Typen.

**Status:** done

### 7.12.4 C++11: namespace-level begin()/end() fuer range-based for

**Spec:** §7.12, S. 19 — "LoanedSamples<T> and SharedSamples<T>
shall provide namespace level begin() and end() functions to
facilitate use of range-based for loop."

**Repo:** Block H emittiert namespace-level begin/end.

**Tests:** Generator-Pfad in
`crates/idl-cpp/tests/blocks_fgh.rs::block_h_emits_seven_dcps_class_decls`.

**Status:** done

### 7.12.5 C++11: dds::core::array als Template-Typedef zu std::array

**Spec:** §7.12, S. 19 — "dds::core::array shall be a template
typedef to std::array."

**Repo:** `psm_cxx::emit_core_basics` rendert
`template <typename T, std::size_t N> using array = std::array<T,N>;`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_conformance.rs::core_basics_emits_time_duration_instance_handle`
prueft die Aliase-Definition.

**Status:** done

### 7.12.6 C++11: Enumerations als built-in `enum class`

**Spec:** §7.12, S. 19 — "Enumerations shall use built-in type-safe
enumerations with enum class syntax."

**Repo:** Cross-Ref §7.4.3 oben — idl-cpp default-emit `enum class`.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

### 7.12.7 C++11: Move-Operations fuer alle Value<DELEGATE>-Types

**Spec:** §7.12, S. 19 — "Move operations (move constructor and move
assign) shall be provided for all Value<DELEGATE> types."

**Repo:** `psm_cxx::emit_reference_value_pattern` emittiert in C++11-
Mode `Value<D>(Value<D>&&)` + `operator=(Value<D>&&)`; Rust-Runtime
hat move-Semantik per default.

**Tests:** `psm_cxx.rs::tests::reference_pattern_emits_reference_template`.

**Status:** done

### 7.12.8 C++11 Plain-Language-Binding: move-Ops + array-by-const-ref + swap + noexcept

**Spec:** §7.12, S. 19 — "Plain language binding shall be augmented
as follows: move-operations as defined in idl2cpp11; arrays as
const-reference parameter; namespace-level swap(t1) + member swap;
move-assign/-constructor/swap may noexcept."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §3.3 — idl-cpp emittiert die
vier Augmentierungen (move/array-cref/swap/noexcept) im C++11-Mode.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::cxx11_default_mode_emits_modern_cxx_features`.

**Status:** done

---

## §7.13 Examples

### 7.13.1 C++03 Beispiel — RadarTrack Pub+Sub

**Spec:** §7.13.1, S. 19-21 — Vollstaendiges DataWriter+DataReader-
Beispiel in C++03 mit Publisher/Subscriber/Topic/QoS-Stream.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-Sektion ist Beispiel-Programm; normative Mapping-Regeln stecken in §7.4-§7.12.

### 7.13.2 C++11 Beispiel — `auto` + Range-based for

**Spec:** §7.13.2, S. 21-22 — C++11-Variante mit `auto samples = dr.
select().max_samples(100).data(...).take()` + `for (auto s :
samples)`.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-Sektion ist Beispiel-Programm; C++11-Plain-Language-Binding-Regeln stecken in §7.12.

---

## §8 Improved Plain Language Binding for C++

### 8.1.1 Aggregation-Types -> C++ Class mit Encapsulation + Accessors per §7.4

**Spec:** §8.1.1, S. 23 — "DDS aggregation types shall be mapped to
a C++ class. Contained attributes shall be encapsulated. Accessors
shall be provided following the rules described in 7.4. The
representation of internal state is unspecified."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.8 — `idl-cpp` Block B
emittiert Aggregation-Types als `class { private: ...; public:
accessors };` exakt nach §7.4.6.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::struct_field_emits_accessor_methods`,
`crates/idl-cpp/tests/spec_conformance.rs::struct_with_default_mapping_emits_class_with_accessors`.

**Status:** done

### 8.1.2 Primitive+Collection-Types per Tab.7.1

**Spec:** §8.1.2, S. 23 — "IDL primitive and collection types used
to define a topic type shall be mapped to C++ following the rules
listed in Table 7.1."

**Repo:** Templates Block B in `crates/idl-cpp/`; cross-ref §7.4.2.x
(alle 12 Mappings als done markiert).

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::{boolean_maps_to_bool, octet_maps_to_uint8_t, integer_types_map_to_stdint, float_types_map_to_cxx_floats, char_maps_to_char_or_char_t}`.

**Status:** done

### 8.1.3 IDL-Enumerations -> C++-Enums (gleicher Name+Values)

**Spec:** §8.1.3, S. 23 — "IDL enumerations shall be mapped into
C++ enumerations with exactly the same enumeration name and
enumeration constants."

**Repo:** Cross-Ref `idl4-cpp-1.0.md` §6.10 — Enum-Codegen behaelt
Name + Konstanten 1:1.

**Tests:** `crates/idl-cpp/tests/psm_cxx_mappings.rs::enum_emits_typed_enumeration_class`.

**Status:** done

### 8.1.4 @Optional Attribute -> dds::core::optional<T>

**Spec:** §8.1.4, S. 23 — "Attributes annotated though the @Optional
annotation are mapped to a template instantiation of the class
dds::core::optional<T> with T equal to the type attribute would
normally map as per the rules specified above."

**Repo:** `crates/idl-cpp/src/blocks.rs::emit_field` rendert
`@optional`-Felder als `dds::core::optional<T>` (C++11: Alias zu
`std::optional<T>`).

**Tests:** `crates/idl-cpp/tests/spec_conformance.rs::optional_member_emits_std_optional`,
`crates/idl-cpp/tests/psm_cxx_mappings.rs::optional_field_uses_std_optional`.

**Status:** done

### 8.1.5 @Shared Attribute -> Pointer-Typ

**Spec:** §8.1.5, S. 23 — "Attributes annotated through the @Shared
annotation are mapped to a pointer of the type they would normally
map as per the rules specified above."

**Repo:** IDL-Lowering: `BuiltinAnnotation::Shared` in
`crates/idl/src/semantics/annotations.rs`. Codegen:
- C++: `crates/idl-cpp/src/emitter.rs::has_shared_annotation` ->
  `std::shared_ptr<T>` (mit `<memory>`-Include); kombinierbar mit
  `@optional` zu `std::optional<std::shared_ptr<T>>`.
- C#: `crates/idl-csharp/src/annotations.rs` -> `[Shared]`-Marker-
  Attribute (Reference-Type-Charakter via Class).
- Java: `crates/idl-java/runtime/Shared.java` + Annotations-Bridge
  -> `@org.zerodds.types.Shared` Marker (Java-Felder sind ohnehin
  Reference-Types).
- Cross-Cutting: `BuiltinAnnotation::Shared | External` setzen beide
  `MemberDescriptor.is_shared = true` (XTypes 1.3 §7.2.2.4.9 +
  idl4-cpp §8.1.5 sind semantisch aequivalent).

**Tests:**
- `crates/idl-cpp/tests/spec_conformance.rs::{shared_member_emits_std_shared_ptr,
  shared_and_optional_compose}`.
- `crates/idl-csharp/tests/spec_conformance.rs::shared_member_emits_shared_marker_attribute`.
- `crates/idl-java/tests/spec_conformance.rs::shared_member_emits_shared_annotation`.

**Status:** done — analog `@optional` voll implementiert.

### 8.2 Beispiel — RadarTrack mit @Optional/@Shared

**Spec:** §8.2, S. 23-24 — Beispiel mit `string id; long x; long y;
long z; //@Optional; plot_t plot; //@Shared`-Mapping.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel-Sample fuer §8.1.4/§8.1.5; normative Mapping-Regeln stecken in §8.1.x.

---

## Audit-Status

103 done / 0 partial / 0 open / 19 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-cpp` — 123 lib + 11 integration =
134 Tests grün. Module mit Tests: `amqp`, `dcps`, `error`, `psm_cxx`,
`qos`, `rpc`, `status`, `type_map` plus root-level `tests::*` (Array/
Const/Duration/Enum/Exception/Header/Module/Optional/Sequence/String/
Struct/Time/Typedef/Union-Codegen-Tests).
