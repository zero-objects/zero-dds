# dlrl-1.2 — offene Punkte

Aggregat aus `dlrl-1.2.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.1.6.2 DLRL Entities

**Status:** `partial` — namentlich vollständig (`DlrlEntityKind` mit 17 Werten, `DlrlException` mit 8), als eigenständige Typen aber nur das Subset `ObjectCache`/`HomeFactory`/`HomeListener`/`ObjectListener`/ `Query`. Es fehlen `CacheFactory`, `CacheBase`, `CacheAccess`, `Contract`, `Selection`, `SelectionCriterion`, `FilterCriterion`, `SelectionListener`, `Collection`/`List`/`Set`/`StrMap`/`IntMap` als Typen — siehe das folgende `open`-Item.

## §8.1.6.2 (Rest) DLRL-Entity-Klassen als eigenständige Typen

**Status:** `open` — die 13 Klassen gibt es in `crates/dlrl` nicht als eigene Typen.

## §8.1.6.3 Details on DLRL Entities

**Status:** `partial` — die Exception-Typen und die Entity-Namen sind vollständig, die Operations nur als Subset. Die Methodensignatur-Konformität der Tabellen §8.1.6.3.1-§8.1.6.3.17 ist nicht hergestellt — siehe das folgende `open`-Item.

## §8.1.6.3 (Rest) Methodensignaturen der Entity-Tabellen

**Status:** `open` — die Attribute und Operationen der Tabellen §8.1.6.3.1 bis §8.1.6.3.17 sind nicht als Signaturen umgesetzt.

## §8.2.1 Run-time Entities

**Status:** `partial` — Namen und Repository-IDs des PSM stimmen, die Laufzeit-Operationen existieren als Rust-API. Die IDL-Interface- Definitionen und der `DLRL_initialize`-Bootstrap werden nicht emittiert — siehe das folgende `open`-Item.

## §8.2.1 (Rest) IDL-PSM-Emission + DLRL_initialize

**Status:** `open` — die IDL-Interfaces der Entities und `DLRL_initialize` werden nicht erzeugt.
