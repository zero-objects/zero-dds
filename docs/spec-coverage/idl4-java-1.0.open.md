# idl4-java-1.0 — offene Punkte

Aggregat aus `idl4-java-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## 1.1 Mapping IDL v4 -> Java

**Status:** `open` — kein Generator bildet IDL auf die Java-Formen aus Klausel 7 ab. Die Java-Spalte erzeugt ein Wire-Binding, dessen eigene Dokumentation die Abweichungen von dieser Spec benennt.

## 2.1 Implementation: IDL -> Java Source per §7

**Status:** `open` — die Java-Spalte transformiert IDL in Java-Quelle, aber nach der PSM der XCDR2-Bindung, nicht nach Klausel 7.

## 7.1.1.0 Naming-Schemes: IDL vs. Java; selektiert via @java_mapping

**Status:** `open` — `@java_mapping` existiert im Baum nicht (`git grep java_mapping` findet nichts), und es gibt keine umschaltbare Naming-Scheme-Auswahl.

## 7.1.1.0 Name-Kollision mit Reserved-Names: Underscore-Präfix

**Status:** `partial` — Java-Keywords werden entschärft, aber durch ein **nachgestelltes** `_` (`class_`) statt des vorgeschriebenen vorangestellten (`_class`). Die Reserved-Name-Liste aus §7.1.2 ist zudem unvollständig (siehe dort).

## 7.1.1.1 IDL Naming Scheme: Names ohne Case-Transformation

**Status:** `open` — die Java-Spalte behält IDL-Namen zwar unverändert (`a_module_is_a_nested_class_and_two_modules_do_not_collide`), aber sie ist kein Naming-Scheme und Identity mit dem IDL-Naming-Scheme ist nicht beabsichtigt oder getestet.

## 7.1.1.2 Java Naming Scheme: JavaBeans-1.01-Konventionen

**Status:** `open` — getX/setX-Accessoren und die JavaBeans-Konventionen werden nicht erzeugt.

## 7.1.1.2.1 Pascal Case Transformation Rules

**Status:** `open` — keine Pascal-Case-Transformation für Typ- und Member-Namen.

## 7.1.1.2.2 Camel Case Transformation Rules

**Status:** `open` — die einzige Camel-Case-Umformung im Baum ist `Java::camel` in `crates/lang-rules/src/emit/java.rs` für Wire-Methodennamen; Member-Namen werden nicht transformiert.

## 7.1.1.2.3 All Uppercase Transformation Rules

**Status:** `open` — Konstanten behalten ihre IDL-Schreibweise; keine Uppercase-Transformation.

## 7.1.1.2.4 All Lowercase Transformation Rules

**Status:** `open` — Modulnamen werden nicht kleingeschrieben; ein Modul `a` wird die geschachtelte Klasse `a`, ein Modul `common` die Klasse `common`, aber ohne die Spec-Transformation.

## 7.1.1.3 Suffixes (z.B. Abstract); Underscore-Präfix bei Kollision

**Status:** `open` — keine Reserved-Suffix-Behandlung; es gibt kein `Abstract`-Suffix-Mapping (die Valuetype-Abbildung fehlt ohnehin).

## 7.1.2 Reserved Names

**Status:** `partial` — Java-Keywords plus `true`/`false`/`null` sind gelistet. `<type>Abstract`, `Constants` und die `java.lang.Object`-Methoden fehlen.

## 7.1.3 Holder<E>-Klasse für inout/out-Parameter

**Status:** `open` — eine Klasse `org.omg.type.Holder<E>` gibt es nicht; die frühere Laufzeitklasse `crates/idl-java/runtime/rpc/Holder.java` ist mit `b7589e6b904` entfernt. Die RPC-Bindung transportiert out/inout in einem vom DDS-RPC benannten `_Out`-Typ (`crates/lang-rules/src/emit/interface_java.rs`).

## 7.2.2 IDL Module -> Java Package

**Status:** `partial` — Module werden auf **geschachtelte Klassen** in einer Datei abgebildet, nicht auf Packages (die Crate begründet das mit dem Ein-Public-Typ-pro-Datei-Gesetz). Zwei gleichnamige Typen in verschiedenen Modulen kollidieren dadurch nicht, die Spec-Form (Paketname) ist aber nicht erreicht.

## 7.2.3 Default Mapping: IDL Constant -> public final class mit value-Field

**Status:** `open` — eine IDL-Konstante wird ein `public static final`-**Feld** im umschließenden Scope, nicht eine gleichnamige Wrapper-Klasse mit `value`.

## 7.2.3.1 Alternative Mapping: Constants-Container-Klasse

**Status:** `open` — kein `Constants`-Container und kein `constants_container`-Parameter.

## 7.2.4.1.1 Integer Types Mapping (Tab.7.2)

**Status:** `partial` — `int8 -> byte`, `short -> short`, `long -> int` und `long long -> long` stimmen. Die unsigned Typen weichen bewusst ab: `uint8`/`octet -> int` (Spec: `byte`), `unsigned short -> int` (Spec: `short`), `unsigned long -> long` (Spec: `int`), weil die Spalte den vollen Wertebereich halten will.

## 7.2.4.1.2 Floating-Point Mapping (Tab.7.3)

**Status:** `partial` — `float -> float` und `double -> double` stimmen. `long double` wird `org.zerodds.wire.Float128`, nicht `BigDecimal`.

## 7.2.4.1.5 IDL boolean + TRUE/FALSE -> Java boolean + true/false

**Status:** `partial` — `boolean` wird `boolean` (Test oben); dass die Konstanten `TRUE`/`FALSE` zu `true`/`false` werden, prüft kein Test.

## 7.2.4.1.6 IDL octet -> Java byte

**Status:** `partial` — `octet` wird `int`, nicht `byte`; die Spalte begründet das mit dem Wertebereich 0..255, den ein signiertes `byte` nicht hält.

## 7.2.4.2.1.1 Sequences of Basic Types -> type-specific Interfaces

**Status:** `open` — keine `BooleanSeq`/`CharSeq`/…-Interfaces; eine Sequenz wird ein Java-Array.

## 7.2.4.2.1.1 Bounds-Checking auf bounded Sequences

**Status:** `open` — es gibt keinen bounded-Sequence-Typ. Ein Java-Array wirft `IndexOutOfBoundsException` nativ, aber die Spec-Form (begrenzte Sequenz-Interfaces) fehlt.

## 7.2.4.2.1.2 Sequences of non-Basic Types -> java.util.List<E>

**Status:** `open` — eine Sequenz wird ein Java-Array `T[]`, nicht `java.util.List<E>`.

## 7.2.4.2.2 IDL string -> java.lang.String + IndexOutOfBoundsException

**Status:** `partial` — `string` wird `java.lang.String` (im Test als Elementtyp belegt). Die Range-Prüfung fehlt; der Bound wird nicht durchgesetzt.

## 7.2.4.2.3 IDL wstring -> java.lang.String

**Status:** `partial` — `wstring` und `string` teilen `java.lang.String` (UTF-16). Die Range-Prüfung fehlt.

## 7.2.4.2.4 IDL fixed -> java.math.BigDecimal + ArithmeticException

**Status:** `partial` — `fixed<P,S>` wird `java.math.BigDecimal` (CORBA 3.3 Teil 3 §1.5.6); der Codec schreibt und liest die Dezimalziffern über `Writer.writeFixed` und `Reader.readFixed`. Die Bereichsprüfung wirft `WireException.OutOfRange`, nicht `java.lang.ArithmeticException`; das ist der Rest. Log: `.agent/REPORT.md`.

## 7.2.4.3.1 IDL struct -> Java public class + Serializable + Bean-Accessoren + Konstruktoren

**Status:** `partial` — eine geschachtelte Klasse mit `public`-Feldern wird erzeugt. Es fehlen `implements java.io.Serializable`, Accessoren/ Modifier, der All-Values-Konstruktor und der öffentliche Default-Konstruktor.

## 7.2.4.3.1 Default-Constructor: Members initialisieren

**Status:** `open` — es gibt keinen öffentlichen Default-Konstruktor mit dieser Initialisierung. `default_java.rs` liefert einen `zd_def_<T>()`- Defaulter für den Decoder (DDS-XTypes Tab.9), nicht den Spec-Konstruktor.

## 7.2.4.3.1 Accessor/Modifier-Naming pro Naming-Scheme

**Status:** `open` — es werden keine Accessoren erzeugt.

## 7.2.4.3.2 IDL union -> Java public final class + Discriminator-Accessor

**Status:** `partial` — die Union wird eine Klasse mit `_d`-Feld und einem `public`-Feld pro Case. Accessoren/Modifier, der `getDiscriminator()`-Zugang und die `__default()`-Formen fehlen.

## 7.2.4.3.3 IDL enum -> Java public enum + valueOf(int)

**Status:** `partial` — die Enumeration wird eine Klasse mit einer Instanz je Enumerator, den Konstanten `public static final int _NAME`, `value()` und `from_int(int)` (CORBA 3.3 Teil 3 §1.5.9); ein Member ist eine Instanz der Klasse. Ein Java-`enum` und `valueOf(int)` der Spec fehlen: die Klasse trägt den Wert, den ein `enum` nicht für Werte hätte, die das IDL des Lesers nicht kennt. Log: `.agent/REPORT.md`.

## 7.2.4.3.4 Constructed Recursive Types

**Status:** `partial` — Java-Referenzsemantik trägt rekursive Typen, und der Korpus umfasst sie; ein Test, der die Rekursion namentlich hält, fehlt (der Genannte läuft nur mit `--include-ignored`).

## 7.2.4.6 IDL typedef -> kein Java-Type; Use replaced; Annotations propagieren

**Status:** `partial` — der `typedef` deklariert nichts und seine Verwendung nimmt den Zieltyp an (Test). Die Propagation von Annotationen auf die Verwendungen fehlt.

## 7.3 IDL any -> org.omg.type.Any

**Status:** `open` — `any` hat keine Java-Form in dieser Spalte und wird namentlich abgelehnt, nicht auf einen `Any`-Typ abgebildet.

## 7.4 IDL interface -> Java public interface

**Status:** `partial` — `@service`-Interfaces werden `public interface` mit Methoden und Attribut-Accessoren; die Bindung folgt aber DDS-RPC 1.0 §7.11.2 (kein `throws`, out/inout in `_Out`-Typen). Der Basis-Pfad für Nicht-Service-Interfaces fehlt.

## 7.4.1 IDL exception -> Java class extends RuntimeException

**Status:** `partial` — die Exception wird eine geschachtelte Datenklasse (als Struct-Form). Sie erbt ausdrücklich **nicht** von `RuntimeException`; die Crate begründet das mit `serialVersionUID`.

## 7.4.2 Interface Forward-Declaration: kein Java-Mapping

**Status:** `open` — es gibt keine Java-Forward-Declaration-Behandlung; die Bindung lehnt eine Interface-Forward-Declaration ohne Definition im RPC-Pfad als E002 ab, was nicht dasselbe ist.

## 7.5 Embedded Type-/Const-/Exception-Decls im Interface-Body

**Status:** `open` — eingebettete Deklarationen im Interface-Body werden nicht in den Scope des Java-Interface überführt.

## 7.6 IDL valuetype -> 2 Java-Klassen (Abstract + concrete)

**Status:** `open` — ein `valuetype` wird wie ein Struct als eine geschachtelte Klasse mit `public`-Feldern abgebildet; die Abstract+Concrete-Zweiteilung, `protected`-Member und `void`-Factory-Methoden fehlen.

## 7.9 Components Basic -> intermediate IDL

**Status:** `open` — keine CCM-Komponenten-Abbildung nach intermediate IDL.

## 7.10 Homes -> intermediate IDL

**Status:** `open` — wie §7.9; keine Home-Abbildung.

## 7.12 Ports/Connectors -> intermediate IDL

**Status:** `open` — wie §7.9; keine Port/Connector-Abbildung.

## 7.13 Template-Module Instances -> intermediate IDL

**Status:** `open` — keine Template-Module-Abbildung.

## 7.14.1 Struct mit Single Inheritance -> Java extends

**Status:** `open` — die Spalte flacht Basis-Member in die abgeleitete Klasse ab (`all_members_of`); ein Java-`extends` und der Base-Instanz-Konstruktor entstehen nicht.

## 7.14.2 Union-Discriminator: int8/uint8/wchar/octet zusätzlich erlaubt

**Status:** `partial` — der Discriminator wird generisch auf den Java-Typ des Diskriminator-IDL-Typs abgebildet, also auch für `octet`; ein Test, der einen 8-Bit- oder `octet`-Discriminator nennt, fehlt.

## 7.14.3.2 IDL bitset -> Java public class mit Bitfield-Accessoren

**Status:** `partial` — das Bitset wird eine Klasse mit einem `public`-Feld pro Bitfield. Bitfield-Accessoren und die Default-Typ-Regel pro Bitgröße fehlen (Felder sind `int`).

## 7.14.3.3 IDL bitmask -> Java enum + java.util.BitSet

**Status:** `partial` — die Bitmask wird Konstanten im unsigned Holder ihres `@bit_bound` (`int` bzw. `long`). Ein `<Name>Flags`-Java-`enum` und `java.util.BitSet` fehlen.

## 7.16.1 IDL @annotation -> Java @interface + <Name>Group

**Status:** `open` — User-Annotationen werden nicht als Java `@interface` propagiert.

## 7.16.2 Single-Instance/Multi-Instance-Anwendung

**Status:** `open` — wie §7.16.1; Annotationen erreichen das Java nicht.

## 7.17.1 General Purpose Tab.7.6

**Status:** `partial` — `@optional` boxt den Member, `@value` schreibt den Enum-Wert. `@id`, `@position` und die Extensibility-Annotationen werden nicht emittiert.

## 7.17.2 Data Modeling Tab.7.7

**Status:** `open` — keine `@key`-/`@must_understand`-Marker und keine `@default_literal`-Behandlung im generierten Java.

## 7.17.3 Units and Ranges Tab.7.8

**Status:** `partial` — `@default` erreicht den `zd_def_`-Defaulter (`default_java.rs::member_default`), aber nicht den Spec-Constructor; `@range`/`@min`/`@max`-Setter-Prüfungen fehlen.

## 7.17.4 Data Implementation Tab.7.9

**Status:** `partial` — `@bit_bound` wählt den Holder der Bitmask. `@external` und `@nested` erreichen das Java nicht.

## 7.17.6 Interfaces Tab.7.11

**Status:** `partial` — `@service` steuert die RPC-Interface-Erzeugung. `@oneway` und `@ami` werden nicht ausgewertet.

## 8.1.0 @java_mapping-Annotation Definition

**Status:** `open` — `@java_mapping` existiert im Baum nicht.

## 8.1.1 apply_naming_convention Tab.8.1

**Status:** `open` — es gibt keinen Naming-Scheme-Schalter; die Transformationen aus §7.1.1.2.x fehlen.

## 8.1.2 constants_container-Parameter

**Status:** `open` — kein Parameter, kein `Constants`-Container.

## 8.1.3 promote_integer_width-Parameter (Tab.8.2)

**Status:** `partial` — die Spalte verbreitert unsigned Typen (unbedingt, also der TRUE-Fall), aber es gibt keinen Schalter und damit nicht den Spec-Default FALSE.

## 8.1.4 string_type-Parameter

**Status:** `partial` — es wird immer `java.lang.String` benutzt; der Parameter fehlt, der Default ist damit erreicht.

## A.1 CORBA-Specific Mappings

**Status:** `open` — keine IDL4-Java-Annex-A.1-Abbildung. Die CORBA-RMI-Spalte `crates/corba-codegen/` implementiert die ältere IDL-to-Java-1.3-Abbildung (formal/2008-01-04) für CORBA, nicht diesen Annex.
