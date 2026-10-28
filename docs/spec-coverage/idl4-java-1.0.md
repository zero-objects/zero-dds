# IDL4 to Java Language Mapping 1.0 — Spec-Coverage

**Spec:** [OMG IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0/PDF) (OMG formal/2021-08-01)

Item-für-Item-Audit gegen die Spec; jede Anforderung mit Spec-Zitat,
Repo-Pfad, Test-Pfad und Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** Der IDL4-Java-Generator `crates/idl-java` wurde in
`d045e43ea86` entfernt. Die lebende Java-Spalte sitzt heute in
`crates/lang-rules/src/adapter/java.rs`,
`crates/lang-rules/src/emit/java.rs`,
`crates/lang-rules/src/emit/interface_java.rs`,
`crates/lang-rules/src/default_java.rs`, dazu
`crates/lang-cst/src/java/mod.rs` und `crates/lang-render/src/java.rs`.
Sie ist ein Plain Language Binding für XCDR2 und ist in
`docs/spec-coverage/zerodds-xcdr2-java-1.0.md` beschrieben. An mehreren
Stellen weicht sie ausdrücklich und begründet von dieser Mapping-Spec ab
(`fixed` als `String` statt `BigDecimal`, `octet` als `int` statt `byte`,
`map` als Entry-Array statt `java.util.Map`, Enumeration als
`int`-Konstanten statt Java-`enum`, Modul als geschachtelte Klasse statt
Paket). Jede dieser Abweichungen ist hier ein Befund, kein `done`. Aus der
alten Crate überlebt nur die Laufzeit `crates/idl-java/runtime/`
(`org.zerodds.types.*`-Annotationen und `org.zerodds.rpc.*`); der heutige
Generator benutzt sie nicht.

---

## §1 Scope

### 1.1 Mapping IDL v4 -> Java

**Spec:** §1, S. 1 — "This specification defines the mapping of OMG
Interface Definition Language v4 [IDL4] to the Java programming
language. The language mapping covers all of the IDL constructs in
the current Interface Definition Language specification with the
exception of middleware specific constructs that are better
addressed in separate specifications."

**Repo:** —

**Tests:** —

**Status:** `open` — kein Generator bildet IDL auf die Java-Formen aus
Klausel 7 ab. Die Java-Spalte erzeugt ein Wire-Binding, dessen eigene
Dokumentation die Abweichungen von dieser Spec benennt.

### 1.2 Nutzung moderner Java-Sprach-Features

**Spec:** §1, S. 1 — "The language mapping makes use of modern Java
language features as appropriate and natural."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Scope-Aussage ohne prüfbares Soll.

---

## §2 Conformance

### 2.1 Implementation: IDL -> Java Source per §7

**Spec:** §2, S. 1 — "A conformant implementation shall transform
IDL input into Java source code output as specified in clause 7."

**Repo:** —

**Tests:** —

**Status:** `open` — die Java-Spalte transformiert IDL in Java-Quelle,
aber nach der PSM der XCDR2-Bindung, nicht nach Klausel 7.

### 2.2 User: Anwendungscode läuft cross-Implementation portabel

**Spec:** §2, S. 1 — "Application source code that conforms to this
specification makes use of the Java data types and API's as defined
in clause 7. [...] Conformant application source code, as a result,
will be portable across implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage; kein
Implementierungs-Soll.

---

## §3 Normative References

### 3.1 [CORBA-IFC] CORBA 3.3

**Spec:** §3, S. 1 — "[CORBA-IFC] CORBA 3.3."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung.

### 3.2 [IDL4] IDL 4.2 (2018)

**Spec:** §3, S. 1 — "[IDL4] OMG IDL 4.2."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung; das
Frontend steht in `crates/idl-parse/` und `crates/idl-lower/`.

### 3.3 [J2SE 8.0] Java SE 8 Edition (2015)

**Spec:** §3, S. 1 — "[J2SE 8.0] Java SE 8."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung.

### 3.4 [JavaBeans] JavaBeans 1.01

**Spec:** §3, S. 1 — "[JavaBeans] JavaBeans 1.01."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Referenz, keine Anforderung; die
Naming-Konventionen sind in §7.1.1.2 befundet.

---

## §4 Terms and Definitions

### 4.1 Building Block

**Spec:** §4, S. 1 — "A Building Block is a consistent set of IDL
rules [...] atomic, meaning that if selected, they must be totally
supported."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.2 Camel Case (Lower Camel Case)

**Spec:** §4, S. 2 — "Camel Case [...] the variation of Camel Case
commonly known as Lower Camel Case, where the first letter is not
capitalized."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; die Anwendung ist
in §7.1.1.2.2 befundet.

### 4.3 Java (general-purpose programming language)

**Spec:** §4, S. 2 — "Java is a general-purpose computer programming
language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.4 Language Mapping

**Spec:** §4, S. 2 — "An association of elements in one language to
elements in another language."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition.

### 4.5 Pascal Case (Upper Camel Case)

**Spec:** §4, S. 2 — "Pascal Case, also known as Upper Camel Case,
[...] 'these are my words' would be 'TheseAreMyWords'."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar-Definition; die Anwendung ist
in §7.1.1.2.1 befundet.

---

## §5 Symbols (Tab.5.1)

### 5.1 Akronyme: CCM, CORBA, DDS, J2SE, IDL, OMG

**Spec:** §5 Tab.5.1, S. 2 — Akronym-Liste.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §6 Additional Information

### 6.1 Alternative zur existierenden OMG IDL-Java-Mapping-Spec

**Spec:** §6.1, S. 3 — "This specification is an alternative to the
existing OMG IDL to Java Mapping specification; it is distinct in
that it provides a mapping for the constructs of IDL4."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

### 6.2 Acknowledgments (ADLINK, RTI, Twin Oaks)

**Spec:** §6.2, S. 3 — informativ.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.1 General

### 7.1.1.0 Naming-Schemes: IDL vs. Java; selektiert via @java_mapping

**Spec:** §7.1.1, S. 5 — "This specification defines two naming
schemes [...] The @java_mapping annotation defined in Clause 8.1
provides a mechanism to select the appropriate naming scheme."

**Repo:** —

**Tests:** —

**Status:** `open` — `@java_mapping` existiert im Baum nicht
(`git grep java_mapping` findet nichts), und es gibt keine
umschaltbare Naming-Scheme-Auswahl.

### 7.1.1.0 Name-Kollision mit Reserved-Names: Underscore-Präfix

**Spec:** §7.1.1, S. 5 — "if a mapped name or identifier collides
with one of the names reserved in Clause 7.1.2, the collision shall
be resolved by prepending an underscore ('_') to the mapped name."

**Repo:** `crates/lang-cst/src/java/mod.rs::escape_keyword`,
`crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-cst/src/java/mod.rs::tests::a_keyword_escapes_with_a_trailing_underscore`,
`crates/lang-rules/src/adapter/java.rs::tests::an_identifier_that_is_a_java_keyword_is_escaped`.

**Status:** `partial` — Java-Keywords werden entschärft, aber durch ein
**nachgestelltes** `_` (`class_`) statt des vorgeschriebenen
vorangestellten (`_class`). Die Reserved-Name-Liste aus §7.1.2 ist
zudem unvollständig (siehe dort).

### 7.1.1.1 IDL Naming Scheme: Names ohne Case-Transformation

**Spec:** §7.1.1.1, S. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers without case transformation,
maintaining the original IDL names."

**Repo:** —

**Tests:** —

**Status:** `open` — die Java-Spalte behält IDL-Namen zwar unverändert
(`a_module_is_a_nested_class_and_two_modules_do_not_collide`), aber sie
ist kein Naming-Scheme und Identity mit dem IDL-Naming-Scheme ist nicht
beabsichtigt oder getestet.

### 7.1.1.2 Java Naming Scheme: JavaBeans-1.01-Konventionen

**Spec:** §7.1.1.2, S. 5 — "IDL member names and type identifiers
shall map to Java names and identifiers that follow the coding
guidelines defined in the JavaBeans 1.01 [JavaBeans] specification."

**Repo:** —

**Tests:** —

**Status:** `open` — getX/setX-Accessoren und die
JavaBeans-Konventionen werden nicht erzeugt.

### 7.1.1.2.1 Pascal Case Transformation Rules

**Spec:** §7.1.1.2.1, S. 5-6 — "The first letter after each
underscore shall be capitalized and all underscores shall be removed.
The first letter of the IDL name shall be capitalized."

**Repo:** —

**Tests:** —

**Status:** `open` — keine Pascal-Case-Transformation für Typ- und
Member-Namen.

### 7.1.1.2.2 Camel Case Transformation Rules

**Spec:** §7.1.1.2.2, S. 6 — "The first letter after each underscore
shall be capitalized and all underscores shall be removed. The first
letter of the IDL name shall be lower case."

**Repo:** —

**Tests:** —

**Status:** `open` — die einzige Camel-Case-Umformung im Baum ist
`Java::camel` in `crates/lang-rules/src/emit/java.rs` für
Wire-Methodennamen; Member-Namen werden nicht transformiert.

### 7.1.1.2.3 All Uppercase Transformation Rules

**Spec:** §7.1.1.2.3, S. 6 — "Every letter shall be capitalized and
all underscores shall be removed."

**Repo:** —

**Tests:** —

**Status:** `open` — Konstanten behalten ihre IDL-Schreibweise; keine
Uppercase-Transformation.

### 7.1.1.2.4 All Lowercase Transformation Rules

**Spec:** §7.1.1.2.4, S. 6-7 — "Every letter shall be in lower case
and all underscores shall be removed."

**Repo:** —

**Tests:** —

**Status:** `open` — Modulnamen werden nicht kleingeschrieben; ein Modul
`a` wird die geschachtelte Klasse `a`, ein Modul `common` die Klasse
`common`, aber ohne die Spec-Transformation.

### 7.1.1.3 Suffixes (z.B. Abstract); Underscore-Präfix bei Kollision

**Spec:** §7.1.1.3, S. 7 — "If an IDL name ends in a reserved suffix
(for example, Abstract), then an underscore is prepended to the
mapped name."

**Repo:** —

**Tests:** —

**Status:** `open` — keine Reserved-Suffix-Behandlung; es gibt kein
`Abstract`-Suffix-Mapping (die Valuetype-Abbildung fehlt ohnehin).

### 7.1.2 Reserved Names

**Spec:** §7.1.2, S. 7 — "The mapping in effect reserves the use of
several names: <type>Abstract, Constants (per <moduleName>-Package);
all Java keywords; literals (true/false/null); java.lang.Object
methods (clone/equals/getClass/hashCode/notify/notifyAll/finalize/
toString/wait)."

**Repo:** `crates/lang-cst/src/java/mod.rs` — die Keyword-Liste.

**Tests:** `crates/lang-cst/src/java/mod.rs::tests::a_keyword_escapes_with_a_trailing_underscore`.

**Status:** `partial` — Java-Keywords plus `true`/`false`/`null` sind
gelistet. `<type>Abstract`, `Constants` und die `java.lang.Object`-Methoden
fehlen.

### 7.1.3 Holder<E>-Klasse für inout/out-Parameter

**Spec:** §7.1.3, S. 8 — "Holder types are required in cases when
an IDL defined data type is passed to an operation as an inout or
out parameter. [...] `package org.omg.type; public class Holder<E>
{ public E value; }`."

**Repo:** `crates/lang-rules/src/emit/interface_java.rs`.

**Tests:** —

**Status:** `open` — eine Klasse `org.omg.type.Holder<E>` gibt es nicht; die
frühere Laufzeitklasse `crates/idl-java/runtime/rpc/Holder.java` ist mit
`b7589e6b904` entfernt. Die RPC-Bindung transportiert out/inout in einem vom
DDS-RPC benannten `_Out`-Typ (`crates/lang-rules/src/emit/interface_java.rs`).

### 7.1.4 Tab.7.1: Java-Sprachversionen pro Feature

**Spec:** §7.1.4, S. 8 — Tab.7.1 listet J2SE 5.0 für Enumerations,
Generics, Annotation Type-Declaration; Java SE 8.0 für Annotation
Type-Use.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Tab.7.1 beschreibt die Sprachversionen
der Spec; sie ist keine eigene Mapping-Anforderung.

### 7.1.5 Code Examples Notation `{...}` (informativ)

**Spec:** §7.1.5, S. 8 — "the notation {...} is used in describing
Java code [...] generated code is specific to a particular vendor's
implementation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.2 Core Data Types

### 7.2.1 IDL Specification (kein direktes Mapping)

**Spec:** §7.2.1, S. 9 — "There is no direct mapping of the IDL
Specification itself."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung.

### 7.2.2 IDL Module -> Java Package

**Spec:** §7.2.2, S. 9 — "An IDL module is mapped to a Java package
with the same name. [...] IDL declarations not enclosed in any
modules are mapped to classes or interfaces in the (unnamed) Java
global scope."

**Repo:** `crates/lang-rules/src/adapter/java.rs`,
`crates/lang-cst/src/java/mod.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_module_is_a_nested_class_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/java.rs::tests::a_module_reopened_beside_a_nested_one_is_one_class`.

**Status:** `partial` — Module werden auf **geschachtelte Klassen** in
einer Datei abgebildet, nicht auf Packages (die Crate begründet das mit
dem Ein-Public-Typ-pro-Datei-Gesetz). Zwei gleichnamige Typen in
verschiedenen Modulen kollidieren dadurch nicht, die Spec-Form
(Paketname) ist aber nicht erreicht.

### 7.2.3 Default Mapping: IDL Constant -> public final class mit value-Field

**Spec:** §7.2.3, S. 9 — "IDL constants shall be mapped to public
final classes of the same name within the equivalent scope and
package. The mapped class shall contain a public final static field
named value."

**Repo:** —

**Tests:** —

**Status:** `open` — eine IDL-Konstante wird ein
`public static final`-**Feld** im umschließenden Scope, nicht eine
gleichnamige Wrapper-Klasse mit `value`.

### 7.2.3.1 Alternative Mapping: Constants-Container-Klasse

**Spec:** §7.2.3.1, S. 10 — "Every scope containing a constant
declaration shall contain a public final class. By default, the
mapped class shall be named 'Constants'. The class name may be
modified using the @java_mapping annotation."

**Repo:** —

**Tests:** —

**Status:** `open` — kein `Constants`-Container und kein
`constants_container`-Parameter.

---

## §7.2.4 Data Types

### 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Spec:** §7.2.4.1.1 Tab.7.2, S. 11 — "int8/uint8 -> byte; short/
int16/unsigned short/uint16 -> short; long/int32/unsigned long/
uint32 -> int; long long/int64/unsigned long long/uint64 -> long."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`,
`crates/lang-rules/tests/one_walk_two_targets.rs::an_unsigned_local_is_wide_enough_in_java`.

**Status:** `partial` — `int8 -> byte`, `short -> short`,
`long -> int` und `long long -> long` stimmen. Die unsigned Typen weichen
bewusst ab: `uint8`/`octet -> int` (Spec: `byte`), `unsigned short ->
int` (Spec: `short`), `unsigned long -> long` (Spec: `int`), weil die
Spalte den vollen Wertebereich halten will.

### 7.2.4.1.2 Floating-Point Mapping (Tab.7.3)

**Spec:** §7.2.4.1.2 Tab.7.3, S. 11 — "float -> float; double ->
double; long double -> java.math.BigDecimal."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — `float -> float` und `double -> double` stimmen.
`long double` wird `org.zerodds.wire.Float128`, nicht `BigDecimal`.

### 7.2.4.1.3 IDL char -> Java char

**Spec:** §7.2.4.1.3, S. 11 — "The IDL char shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** done

### 7.2.4.1.4 IDL wchar -> Java char

**Spec:** §7.2.4.1.4, S. 11 — "The IDL wchar shall be mapped to the
Java primitive type char."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** done

### 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Spec:** §7.2.4.1.5, S. 11 — "The IDL boolean type shall be mapped
to the Java boolean, and the IDL constants TRUE and FALSE shall be
mapped to the corresponding Java boolean literals true and false."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — `boolean` wird `boolean` (Test oben); dass die Konstanten `TRUE`/`FALSE` zu `true`/`false` werden, prüft kein Test.

### 7.2.4.1.6 IDL octet -> Java byte

**Spec:** §7.2.4.1.6, S. 11 — "The IDL type octet, an 8-bit
quantity, shall be mapped to the Java type byte."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — `octet` wird `int`, nicht `byte`; die Spalte
begründet das mit dem Wertebereich 0..255, den ein signiertes `byte`
nicht hält.

### 7.2.4.2.1.1 Sequences of Basic Types -> type-specific Interfaces

**Spec:** §7.2.4.2.1.1 Tab.7.4, S. 12 — type-specific Sequence-Interfaces
in `org.omg.type` mit `extends java.util.List<Boxed>`.

**Repo:** —

**Tests:** —

**Status:** `open` — keine `BooleanSeq`/`CharSeq`/…-Interfaces; eine
Sequenz wird ein Java-Array.

### 7.2.4.2.1.1 Bounds-Checking auf bounded Sequences

**Spec:** §7.2.4.2.1.1, S. 13 — "Bounds checking on bounded
sequences shall raise a java.lang.IndexOutOfBoundsException."

**Repo:** —

**Tests:** —

**Status:** `open` — es gibt keinen bounded-Sequence-Typ. Ein Java-Array
wirft `IndexOutOfBoundsException` nativ, aber die Spec-Form
(begrenzte Sequenz-Interfaces) fehlt.

### 7.2.4.2.1.2 Sequences of non-Basic Types -> java.util.List<E>

**Spec:** §7.2.4.2.1.2, S. 13 — "IDL sequences of non basic types
shall be mapped to the Java generic java.util.List<E> interface."

**Repo:** —

**Tests:** —

**Status:** `open` — eine Sequenz wird ein Java-Array `T[]`, nicht
`java.util.List<E>`.

### 7.2.4.2.2 IDL string -> java.lang.String + IndexOutOfBoundsException

**Spec:** §7.2.4.2.2, S. 14 — "The IDL string, both bounded and
unbounded variants, shall be mapped to java.lang.String. Range
checking [...] shall raise a java.lang.IndexOutOfBoundsException
exception if necessary."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** `partial` — `string` wird `java.lang.String` (im Test als
Elementtyp belegt). Die Range-Prüfung fehlt; der Bound wird nicht
durchgesetzt.

### 7.2.4.2.3 IDL wstring -> java.lang.String

**Spec:** §7.2.4.2.3, S. 14 — analog §7.2.4.2.2.

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** `partial` — `wstring` und `string` teilen `java.lang.String`
(UTF-16). Die Range-Prüfung fehlt.

### 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException


**Spec:** §7.2.4.2.4, S. 14 — "The IDL fixed type shall be mapped
to the Java java.math.BigDecimal class. Range checking shall raise
a java.lang.ArithmeticException if necessary."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_fixed_is_a_bigdecimal`, `crates/lang-rules/tests/java_runtime_selftest.rs`

**Status:** `partial` — `fixed<P,S>` wird `java.math.BigDecimal` (CORBA 3.3 Teil 3 §1.5.6); der Codec schreibt und liest die Dezimalziffern über `Writer.writeFixed` und `Reader.readFixed`. Die Bereichsprüfung wirft `WireException.OutOfRange`, nicht `java.lang.ArithmeticException`; das ist der Rest. Log: `.agent/REPORT.md`.

### 7.2.4.3.1 IDL struct -> Java public class + Serializable + Bean-Accessoren + Konstruktoren

**Spec:** §7.2.4.3.1, S. 15 — "An IDL struct shall be mapped to a
Java public class of the same name. The class shall provide:
implements java.io.Serializable, public accessor per member, public
modifier per member, public all-values constructor, public default
constructor."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — eine geschachtelte Klasse mit `public`-Feldern
wird erzeugt. Es fehlen `implements java.io.Serializable`, Accessoren/
Modifier, der All-Values-Konstruktor und der öffentliche Default-Konstruktor.

### 7.2.4.3.1 Default-Constructor: Members initialisieren

**Spec:** §7.2.4.3.1, S. 15 — "The default constructor shall
initialize member fields as follows: Primitives - Java default;
Strings - empty string; Arrays - declared size with element defaults;
Sequences - zero-length; Others - default constructor."

**Repo:** `crates/lang-rules/src/default_java.rs`.

**Tests:** —

**Status:** `open` — es gibt keinen öffentlichen Default-Konstruktor mit
dieser Initialisierung. `default_java.rs` liefert einen `zd_def_<T>()`-
Defaulter für den Decoder (DDS-XTypes Tab.9), nicht den Spec-Konstruktor.

### 7.2.4.3.1 Accessor/Modifier-Naming pro Naming-Scheme

**Spec:** §7.2.4.3.1, S. 15 — "Accessor and modifier methods shall
follow the pattern get_<MemberName>()/set_<MemberName>() in IDL
Naming Scheme, and get<MemberName>()/set<MemberName>() in Java
Naming Scheme."

**Repo:** —

**Tests:** —

**Status:** `open` — es werden keine Accessoren erzeugt.

### 7.2.4.3.2 IDL union -> Java public final class + Discriminator-Accessor

**Spec:** §7.2.4.3.2, S. 16-17 — "An IDL union shall be mapped to a
Java public final class with the same name. [...] public accessor for
discriminator [...] public accessor/modifier per member [...]"

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`.

**Status:** `partial` — die Union wird eine Klasse mit `_d`-Feld und
einem `public`-Feld pro Case. Accessoren/Modifier, der
`getDiscriminator()`-Zugang und die `__default()`-Formen fehlen.

### 7.2.4.3.3 IDL enum -> Java public enum + valueOf(int)


**Spec:** §7.2.4.3.3, S. 18 — "An IDL enum shall be mapped to a
Java public enum with the same name [...] private member to hold the
value [...] static helper method valueOf(int)."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one`, `crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`, `crates/lang-rules/tests/java_map_class.rs::an_enumeration_is_a_class_of_instances_with_value_and_from_int`

**Status:** `partial` — die Enumeration wird eine Klasse mit einer Instanz je Enumerator, den Konstanten `public static final int _NAME`, `value()` und `from_int(int)` (CORBA 3.3 Teil 3 §1.5.9); ein Member ist eine Instanz der Klasse. Ein Java-`enum` und `valueOf(int)` der Spec fehlen: die Klasse trägt den Wert, den ein `enum` nicht für Werte hätte, die das IDL des Lesers nicht kennt. Log: `.agent/REPORT.md`.

### 7.2.4.3.4 Constructed Recursive Types

**Spec:** §7.2.4.3.4, S. 19 — "Constructed recursive types are
supported by mapping the involved types directly to Java."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/tests/java_corpus_compiles.rs::every_corpus_case_that_renders_also_compiles`.

**Status:** `partial` — Java-Referenzsemantik trägt rekursive Typen, und
der Korpus umfasst sie; ein Test, der die Rekursion namentlich hält,
fehlt (der Genannte läuft nur mit `--include-ignored`).

### 7.2.4.4 IDL Array -> Java Array + IndexOutOfBoundsException

**Spec:** §7.2.4.4, S. 19 — "An IDL array shall be mapped to a Java
array of the mapped element type. Bound violations shall raise a
java.lang.IndexOutOfBoundsException."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** done

### 7.2.4.5 IDL native -> kein definiertes Mapping

**Spec:** §7.2.4.5, S. 20 — "This language mapping specification
does not define any native types."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Meta-Aussage zur Spec-Gliederung.

### 7.2.4.6 IDL typedef -> kein Java-Type; Use replaced; Annotations propagieren

**Spec:** §7.2.4.6, S. 20 — "Java does not have a typedef construct
[...] The use of an IDL typedef type shall be replaced with the
type referenced by the typedef type. This rule shall apply
recursively. Annotations on an IDL typedef shall be applied to
uses of the typedef in other type declarations."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_typedef_declares_nothing_and_says_so`.

**Status:** `partial` — der `typedef` deklariert nichts und seine
Verwendung nimmt den Zieltyp an (Test). Die Propagation von Annotationen
auf die Verwendungen fehlt.

---

## §7.3 Any

### 7.3 IDL any -> org.omg.type.Any

**Spec:** §7.3, S. 21 — "The IDL any type shall be mapped to
org.omg.type.Any type. The implementation of the org.omg.type.Any
is middleware specific."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_construct_with_no_java_data_form_is_named_and_not_left_out`.

**Status:** `open` — `any` hat keine Java-Form in dieser Spalte und wird
namentlich abgelehnt, nicht auf einen `Any`-Typ abgebildet.

---

## §7.4 Interfaces – Basic

### 7.4 IDL interface -> Java public interface

**Spec:** §7.4, S. 21 — "Each IDL interface shall be mapped to a
Java public interface with the same name. [...] Each operation
defined in the IDL interface shall map to a method in the Java
interface."

**Repo:** `crates/lang-rules/src/emit/interface_java.rs`,
`crates/lang-rules/src/adapter/interface.rs`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_a_future_of_the_boxed_result`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_the_out_type_where_cpp_takes_a_reference`,
`crates/lang-rules/tests/interface_bindings.rs::java_declares_no_throws_and_still_names_the_raises`.

**Status:** `partial` — `@service`-Interfaces werden `public interface`
mit Methoden und Attribut-Accessoren; die Bindung folgt aber DDS-RPC 1.0
§7.11.2 (kein `throws`, out/inout in `_Out`-Typen). Der Basis-Pfad für
Nicht-Service-Interfaces fehlt.

### 7.4.1 IDL exception -> Java class extends RuntimeException

**Spec:** §7.4.1, S. 22 — "An IDL exception shall be mapped to a
Java class extending the java.lang.RuntimeException class with the
same name as the IDL exception. Any members [...] following the
rules of the IDL struct mapping."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** —

**Status:** `partial` — die Exception wird eine geschachtelte
Datenklasse (als Struct-Form). Sie erbt ausdrücklich **nicht** von
`RuntimeException`; die Crate begründet das mit `serialVersionUID`.

### 7.4.2 Interface Forward-Declaration: kein Java-Mapping

**Spec:** §7.4.2, S. 23 — "An interface forward declaration has no
mapping to the Java language."

**Repo:** —

**Tests:** —

**Status:** `open` — es gibt keine Java-Forward-Declaration-Behandlung;
die Bindung lehnt eine Interface-Forward-Declaration ohne Definition im
RPC-Pfad als E002 ab, was nicht dasselbe ist.

---

## §7.5 Interfaces – Full

### 7.5 Embedded Type-/Const-/Exception-Decls im Interface-Body

**Spec:** §7.5, S. 23 — "The embedded elements shall be mapped to a
public declaration within the scope of the Java interface."

**Repo:** —

**Tests:** —

**Status:** `open` — eingebettete Deklarationen im Interface-Body werden
nicht in den Scope des Java-Interface überführt.

---

## §7.6 Value Types

### 7.6 IDL valuetype -> 2 Java-Klassen (Abstract + concrete)

**Spec:** §7.6, S. 23-25 — "An IDL valuetype type shall be mapped
to two Java classes: A helper abstract class with the suffix
Abstract; A class with the same name [...] private members are
protected [...] Each valuetype initializer is mapped to a method
returning void."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** —

**Status:** `open` — ein `valuetype` wird wie ein Struct als eine
geschachtelte Klasse mit `public`-Feldern abgebildet; die
Abstract+Concrete-Zweiteilung, `protected`-Member und
`void`-Factory-Methoden fehlen.

---

## §7.7 CORBA-Specific – Interfaces

### 7.7 CORBA-Specific -> Annex A.1

**Spec:** §7.7, S. 25 — "CORBA-specific mappings are defined in
clause A.1 of Annex A."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.8 CORBA-Specific – Value Types

### 7.8 CORBA-Specific Value Types -> Annex A.1

**Spec:** §7.8, S. 25 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.9 Components – Basic

### 7.9 Components Basic -> intermediate IDL

**Spec:** §7.9, S. 25 — "Basic components have no direct language
mapping; they shall be mapped to intermediate IDL."

**Repo:** —

**Tests:** —

**Status:** `open` — keine CCM-Komponenten-Abbildung nach
intermediate IDL.

---

## §7.10 Components – Homes

### 7.10 Homes -> intermediate IDL

**Spec:** §7.10, S. 25 — analog §7.9.

**Repo:** —

**Tests:** —

**Status:** `open` — wie §7.9; keine Home-Abbildung.

---

## §7.11 CCM-Specific

### 7.11 CCM-Specific -> Annex A.1

**Spec:** §7.11, S. 25 — analog §7.7.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.12 Components – Ports and Connectors

### 7.12 Ports/Connectors -> intermediate IDL

**Spec:** §7.12, S. 25 — analog §7.9.

**Repo:** —

**Tests:** —

**Status:** `open` — wie §7.9; keine Port/Connector-Abbildung.

---

## §7.13 Template Modules

### 7.13 Template-Module Instances -> intermediate IDL

**Spec:** §7.13, S. 25 — "Template module instances have no direct
language mapping; they shall be mapped to intermediate IDL."

**Repo:** —

**Tests:** —

**Status:** `open` — keine Template-Module-Abbildung.

---

## §7.14 Extended Data Types

### 7.14.1 Struct mit Single Inheritance -> Java extends

**Spec:** §7.14.1, S. 26 — "If the IDL struct inherits from a base
IDL struct, then the Java class shall be declared to extend the
base class [...] all values constructor [...] first parameter a
non-null instance of the base struct's Java class."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** —

**Status:** `open` — die Spalte flacht Basis-Member in die abgeleitete
Klasse ab (`all_members_of`); ein Java-`extends` und der
Base-Instanz-Konstruktor entstehen nicht.

### 7.14.2 Union-Discriminator: int8/uint8/wchar/octet zusätzlich erlaubt

**Spec:** §7.14.2, S. 26 — "This IDL4 block adds int8, uint8, wchar
and octet to the set of valid types for a discriminator."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`.

**Status:** `partial` — der Discriminator wird generisch auf den
Java-Typ des Diskriminator-IDL-Typs abgebildet, also auch für `octet`;
ein Test, der einen 8-Bit- oder `octet`-Discriminator nennt, fehlt.

### 7.14.3.1 IDL map -> java.util.Map<K,V>


**Spec:** §7.14.3.1, S. 26-27 — "An IDL map shall be mapped to a
Java generic java.util.Map instantiated with the Java equivalent
key type and value type."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_map_is_a_java_util_map_over_a_generated_entry_class`, `crates/lang-rules/src/adapter/java.rs::tests::a_map_keyed_by_a_structure_is_a_java_util_map_over_a_class_with_equals`, `crates/lang-rules/tests/java_map_class.rs::a_map_is_a_java_util_map_with_the_operations_of_table_47`

**Status:** done — eine `map<K,V>` wird eine Klasse `ZdMap_…`, die `java.util.LinkedHashMap` über die geboxten Typen erweitert und die Operationen von Table 47 trägt; Struktur, Union und Bitset sind Schlüssel (erzeugte `equals` und `hashCode`). Log: `.agent/REPORT.md`.

### 7.14.3.2 IDL bitset -> Java public class mit Bitfield-Accessoren

**Spec:** §7.14.3.2, S. 28 — "An IDL bitset shall map to Java as a
public class with the same name. [...] Default member type: boolean
if size is 1, octet 2-8, unsigned short 9-16, unsigned long 17-32,
unsigned long long 33-64."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_bitset_is_a_class_of_one_field_per_bitfield`.

**Status:** `partial` — das Bitset wird eine Klasse mit einem
`public`-Feld pro Bitfield. Bitfield-Accessoren und die
Default-Typ-Regel pro Bitgröße fehlen (Felder sind `int`).

### 7.14.3.3 IDL bitmask -> Java enum + java.util.BitSet

**Spec:** §7.14.3.3, S. 28-29 — "The IDL bitmask type shall map to
a Java enum and a java.util.BitSet. The Java enum name shall be the
IDL bitmask name with the Flags suffix appended. [...] If the size
exceeds @bit_bound, IndexOutOfBoundsException shall be raised."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`.

**Status:** `partial` — die Bitmask wird Konstanten im unsigned Holder
ihres `@bit_bound` (`int` bzw. `long`). Ein `<Name>Flags`-Java-`enum`
und `java.util.BitSet` fehlen.

---

## §7.15 Anonymous Types

### 7.15 Anonymous Types: kein Impact auf Java-Mapping

**Spec:** §7.15, S. 29 — "No impact to the Java language mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Spec-eigene non-binding Aussage.

---

## §7.16 Annotations

### 7.16.1 IDL @annotation -> Java @interface + <Name>Group

**Spec:** §7.16.1, S. 29-30 — "An IDL annotation type [...] shall be
represented by Java annotation types <AnnotationName> and
<AnnotationName>Group."

**Repo:** —

**Tests:** —

**Status:** `open` — User-Annotationen werden nicht als Java
`@interface` propagiert.

### 7.16.2 Single-Instance/Multi-Instance-Anwendung

**Spec:** §7.16.2, S. 30 — "the corresponding Java element shall be
annotated with the mapped Java annotation [...] For multiple
instances, the Group-suffix annotation."

**Repo:** —

**Tests:** —

**Status:** `open` — wie §7.16.1; Annotationen erreichen das Java nicht.

---

## §7.17 Standardized Annotations

### 7.17.1 General Purpose Tab.7.6

**Spec:** §7.17.1 Tab.7.6, S. 32 — Impact pro General-Purpose-
Annotation (`@id`, `@autoid`, `@optional`, `@position`, `@value`,
`@extensibility`/`@final`/`@mutable`/`@appendable`).

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_optional_member_is_boxed_and_a_primitive_one_too`,
`crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`.

**Status:** `partial` — `@optional` boxt den Member, `@value` schreibt
den Enum-Wert. `@id`, `@position` und die
Extensibility-Annotationen werden nicht emittiert.

### 7.17.2 Data Modeling Tab.7.7

**Spec:** §7.17.2 Tab.7.7, S. 32 — Impact von `@key`,
`@must_understand`, `@default_literal`.

**Repo:** —

**Tests:** —

**Status:** `open` — keine `@key`-/`@must_understand`-Marker und keine
`@default_literal`-Behandlung im generierten Java.

### 7.17.3 Units and Ranges Tab.7.8

**Spec:** §7.17.3 Tab.7.8, S. 32-33 — `@default` im Default-
Constructor; `@range`/`@min`/`@max` lösen im Setter
`IllegalArgumentException` aus; `@unit` ohne Wirkung.

**Repo:** `crates/lang-rules/src/default_java.rs`.

**Tests:** —

**Status:** `partial` — `@default` erreicht den `zd_def_`-Defaulter
(`default_java.rs::member_default`), aber nicht den Spec-Constructor;
`@range`/`@min`/`@max`-Setter-Prüfungen fehlen.

### 7.17.4 Data Implementation Tab.7.9

**Spec:** §7.17.4 Tab.7.9, S. 33 — `@bit_bound` (Bitmask),
`@external` (boxed), `@nested` (kein Impact).

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_bitmask_takes_the_holder_its_bound_calls_for`.

**Status:** `partial` — `@bit_bound` wählt den Holder der Bitmask.
`@external` und `@nested` erreichen das Java nicht.

### 7.17.5 Code Generation Tab.7.10: @verbatim

**Spec:** §7.17.5 Tab.7.10, S. 33 — "@verbatim copies verbatim text
to the indicated output position when the indicated language is
'*' or 'java'."

**Repo:** `crates/lang-rules/src/verbatim.rs`,
`crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::java_wraps_the_file_and_fills_the_class_body`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`.

**Status:** done — die `java`- und `*`-Aliase werden an den sechs
Spec-Platzierungen in die Java-Ausgabe eingesetzt.

### 7.17.6 Interfaces Tab.7.11

**Spec:** §7.17.6 Tab.7.11, S. 33 — `@service`, `@oneway`, `@ami`;
der Impact ist middleware-spezifisch.

**Repo:** `crates/lang-rules/src/emit/interface_java.rs`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`.

**Status:** `partial` — `@service` steuert die RPC-Interface-Erzeugung.
`@oneway` und `@ami` werden nicht ausgewertet.

---

## §8 IDL to Java Language Mapping Annotations

### 8.1.0 @java_mapping-Annotation Definition

**Spec:** §8.1, S. 35 — "@annotation java_mapping { enum
NamingConvention {IDL_NAMING_CONVENTION,JAVA_NAMING_CONVENTION};
NamingConvention apply_naming_convention; string constants_container
default 'Constants'; boolean promote_integer_width default FALSE;
string string_type default 'java.lang.String'; }"

**Repo:** —

**Tests:** —

**Status:** `open` — `@java_mapping` existiert im Baum nicht.

### 8.1.1 apply_naming_convention Tab.8.1

**Spec:** §8.1.1 Tab.8.1, S. 35-36 — die Tabelle listet 17
IDL-Konstrukt-Typen mit IDL_NAMING vs. JAVA_NAMING.

**Repo:** —

**Tests:** —

**Status:** `open` — es gibt keinen Naming-Scheme-Schalter; die
Transformationen aus §7.1.1.2.x fehlen.

### 8.1.2 constants_container-Parameter

**Spec:** §8.1.2, S. 37 — "constants_container activates the
alternative mapping for constants defined in §7.2.3.1."

**Repo:** —

**Tests:** —

**Status:** `open` — kein Parameter, kein `Constants`-Container.

### 8.1.3 promote_integer_width-Parameter (Tab.8.2)

**Spec:** §8.1.3 Tab.8.2, S. 37-38 — "whether IDL unsigned integers
shall be mapped to a Java primitive type of the same size (FALSE,
default) or to a bigger type (TRUE)."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`.

**Status:** `partial` — die Spalte verbreitert unsigned Typen
(unbedingt, also der TRUE-Fall), aber es gibt keinen Schalter und
damit nicht den Spec-Default FALSE.

### 8.1.4 string_type-Parameter

**Spec:** §8.1.4, S. 38 — "string_type defines the Java type IDL
string and wstring types shall be mapped to. By default,
java.lang.String."

**Repo:** `crates/lang-rules/src/adapter/java.rs`.

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`.

**Status:** `partial` — es wird immer `java.lang.String` benutzt; der
Parameter fehlt, der Default ist damit erreicht.

---

## Annex A: Platform-Specific Mappings (CORBA)

### A.1 CORBA-Specific Mappings

**Spec:** Annex A, S. 39+ — CORBA-spezifische Anpassungen.

**Repo:** —

**Tests:** —

**Status:** `open` — keine IDL4-Java-Annex-A.1-Abbildung. Die
CORBA-RMI-Spalte `crates/corba-codegen/` implementiert die ältere
IDL-to-Java-1.3-Abbildung (formal/2008-01-04) für CORBA, nicht diesen
Annex.

---

## Audit-Status

5 done / 26 partial / 34 open / 22 n/a (informative) / 0 n/a (rejected).

Keine Test-Ausführung im Rahmen dieses Re-Syncs (Cargo ist für diesen
Task nicht erlaubt). Die Belege sind Pfade und Testnamen im heutigen Baum;
`python3 website/_tools/verify_coverage_items.py --check` prüft Zitat und
Testnamen.
