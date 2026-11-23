# dlrl-1.2 — offene Punkte

Aggregat aus `dlrl-1.2.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.1.3.1 DLRL Objects

**Status:** `partial` — Modell, lokale und shared Attribute, Typprüfung und oid sind gebaut. Offen: eine Speicherzelle je Identität (Cache mit `ObjectRoot` als geteiltem Handle, Item §8.1.6.2 Rest) und der Aufruf von `Model::check_value` aus den `set_<attribute>`-Operationen der erzeugten Klassen (Item §8.1.6.3 Rest).

## §8.1.3.2.1 Inheritance

**Status:** `partial` — die Vererbung im Modell ist gebaut. Offen: die erzeugten Klassen (Track3D erweitert die erzeugte Klasse Track, Item §8.2.2) und `ObjectHome` mit `parent` und `children` (Item §8.1.6.2 Rest).

## §8.1.3.2.2 Associations

**Status:** `partial` — Relationen, Kompositionen und die beiden Regeln für Inverse sind im Modell gebaut. Offen zur Laufzeit: das Setzen eines Endes aktualisiert das andere (1-1, 1-n, n-m) und `destroy` eines Ganzen zerstört seine Teile (Item §8.1.6.3 Rest, `ObjectRoot` und die Sammlungen).

## §8.1.4.5 Mapping when DCPS Model is Fixed

**Status:** `partial` — gebaut und gegen Cyclone DDS 0.10.5 gemessen: Objekte über ein festes DCPS-Modell in beide Richtungen und die Verwaltung der Zuordnung Schlüssel ↔ oid. Nicht gemessen: gegen OpenSplice (nicht auf dem Linux-Testhost) und gegen Cyclone DDS 11.0.1 des Images im Job `vendor-live-dcps`. Offen: Vererbung zwischen Anwendungsklassen mit `NoOid`-Schlüssel. Eine abgeleitete Klasse im Haupt-Topic ihrer Basisklasse wird abgewiesen (`MappingError::ClassPartRequired`), weil ohne Klassenfeld die Klasse einer Zeile nicht im Topic steht; §8.1.4.5 nennt diese Vererbung schwierig, aber machbar, und das Mapping unterstützt. Einschränkungen dieser Implementierung, die die Spezifikation nicht nennt: `ObjectHome::create_object` einer `NoOid`-Klasse ergibt `PreconditionNotMet`, weil ein Objekt von `create_object` registriert ist und die Identitätsfelder eines registrierten Objekts nach §8.1.6.3.7 nicht mehr geändert werden dürfen; es entsteht über `create_unregistered_object` und `register_object`. Eine einwertige Relation auf eine `NoOid`-Klasse, die kein Objekt bezeichnet, wird nicht geschrieben (`RowError::NoObjectInNoOidReference`), weil die Schlüsselfelder des Ziels keinen Wert für „kein Objekt“ haben.

## §8.1.4.6 How is this Mapping Indicated?

**Status:** `partial` — die zwei Eingaben und die DCPS-Beschreibung sind gebaut. Offen: der Generator, der aus Model-Description und Model-Tags die native Modellbeschreibung und die typisierten DLRL-Entities über `crates/dlrl/src/cache/` erzeugt (Item §8.2.2); `DDS::DLRLOid` in der Array-Form des PSM, sobald `zerodds-wire-codec` ein Array als Schlüsselmember annimmt.

## §8.2.2 Generation Process

**Status:** `partial` — gebaut und auf dem Linux-Testhost grün: das Lesen der Model Tags gegen die DTD von §8.2.2.3.1 und der Generator, der aus Modellbeschreibung und Model Tags Modell und Mapping macht. Offen: (1) `zerodds-idlc` macht aus den IDL-Valuetypes der Modellbeschreibung und den Model Tags (`--tags`) Modell und Mapping; heute findet `dlrl --lang idl` die Valuetypes mit Basis `DDS::ObjectRoot` nur für das implizite IDL (Item §8.2.1 Rest), die vier Sprachen lesen den Pragma-Dialekt, `DLRL::ObjectRoot` und `#include "dlrl.idl"` wie in §8.2.3.2 erkennt es nicht. (2) Von den Ausgaben der Figure 8.7 fehlt die DCPS-Beschreibung als IDL; das implizite IDL schreibt `dlrl --lang idl`, die DLRL-Entities als IDL liegen als `crates/dlrl/idl/dds_dlrl.idl` vor. (3) Die generierten Klassen nach §8.1.6.6 (`FooHome` mit `create_object(access)`, `FooListener`, `FooSelection`, `FooSelectionListener`, `FooFilter`, `FooQuery`, typisierte Sammlungen) für Rust und die vier Sprachen; die heutigen Klassen haben ein `delete_object`, das die Spezifikation nicht kennt, und erben von `DLRL::ObjectRoot`, für das in C++, Java, C# und TypeScript keine Laufzeit existiert (`tools/idlc/tests/dlrl_compiles.rs` übersetzt gegen Stubs). (4) Eine Laufzeit für C++, Java, C# und TypeScript über `crates/zerodds-c-api`.
