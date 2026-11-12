# idl4-cpp-1.0 — offene Punkte

Aggregat aus `idl4-cpp-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## A.1.1.11 Servant References (Tab.A.15)

**Status:** `open` — `CORBA::servant_traits` ist in `omg/corba.hpp` nur deklariert; der Emitter schreibt keine Spezialisierung.

## A.1.3 TypeCode für map, bitset und bitmask

**Status:** `open` — für `map`, `bitset` und `bitmask` schreibt der Emitter weder `_tc_` noch `Any`-Operatoren; `TCKind` aus CORBA 3.3 hat für sie keine Art.

## A.1.4 ORB_init

**Status:** `open` — `ORB_init` und eine Implementierung von `CORBA::ORB` fehlen; `Object::_get_orb` wirft `INITIALIZE`.

## A.1.8.8 Skeleton eines Valuetype, das ein Interface unterstützt

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.8.9.1, A.1.8.9.3 Fabriken am ORB

**Status:** `open` — `CORBA::ORB` hat die Operationen als rein virtuelle Funktionen; eine Implementierung des ORB fehlt.

## A.1.10.1 Implementing Interfaces

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.10.2 PortableServer::Servant

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.10.3 Servant References

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.10.4 Servant Argument Passing

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.10.5 Skeleton Operations

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.10.6 Inheritance-Based Interface Implementation

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.10.7 Implementing Operations

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.11.1 Mapping of ServerRequest

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.11.2 Mapping of PortableServer Dynamic Implementation Routine

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.12 PortableServer Functions

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.13.1 Mapping for Cookie

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.13.2 ServantManagers and AdapterActivators

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## A.1.13.3 Server Side Mapping for Abstract Interfaces

**Status:** `open` — weder `PortableServer` noch ein Skeleton wird erzeugt; `runtime/cpp/include/omg/` hat keinen Object Adapter.

## Annex C.4 CORBA-Specific Mappings für C++98 und C++03

**Status:** `open` — `omg/corba.hpp` setzt C++11 voraus; die CORBA-Plattform wird nur für C++11 und neuer erzeugt.

## Annex D: IDL4 Mapping Rules for Classic C++ Mapping Specifications

**Status:** `partial` — ZeroDDS hat eine klassische Abbildung nach [OMG-C++] (`--cpp-mapping classic`, C++03), für die Annex D gilt. Die Map-Klasse folgt D.2.1.3.1 nicht (eigenes Item); die übrigen Regeln von D.2 sind nicht einzeln gegen die Abbildung geprüft. D.3 betrifft die C++11-Abbildung [OMG-C++11], die ZeroDDS nicht hat.

## Annex D.2.1.3.1 Maps in der klassischen Abbildung

**Status:** `open` — `zerodds::Map` (`runtime/cpp/include/zerodds/map.hpp`) hat `get_size`, `get_max_size`, `set_max_size`, `insert` und `erase` mit `DDS::ReturnCode_t`, aber nicht `maximum()`, `length()`, `clear()`, `insert_or_assign` und die Rückgaben als `Boolean`.
