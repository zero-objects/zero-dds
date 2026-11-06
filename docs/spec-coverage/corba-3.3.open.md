# corba-3.3 — offene Punkte

Aggregat aus `corba-3.3.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2 Conformance and Compliance

**Status:** `open` — die Einhaltung des Standards ist die Summe der Items dieses Dokuments; es fehlt der Nachweis einer OMG-Sprachabbildung. ZeroDDS bildet CORBA-IDL nach Rust ab (`zerodds-corba-rust-1.1`, eigene Spec, keine OMG-Abbildung); `crates/corba-codegen` erzeugt Stub- und Skeleton-Knoten für C++ und Java, ohne Coverage-Dokument gegen die OMG-Abbildungen für C++ oder Java.

## §7.6 Interoperable Object References (IOR)

**Status:** `partial` — IOR, Profile, Komponenten, stringifizierte IOR und `corbaloc`/`corbaname` sind kodiert und getestet. Es fehlen: ein IPv6-Host ohne Port, `rir` zusammen mit einem anderen Protokoll und die Minor-Codes 7–10; eigenes Item.

## §7.6.10 Object URLs — IPv6 ohne Port, `rir`-Ausschluss, Minor-Codes

**Status:** `open` — `parse_endpoint` in `crates/corba-ior/src/url.rs` trennt den Port am letzten Doppelpunkt ab; ein IPv6-Host ohne Port (`corbaloc::[::1]/key`) wird dabei zerlegt und abgewiesen. Eine Liste mit `rir` und `iiop` wird angenommen. Fehler kommen als `IorError`, nicht als `BAD_PARAM` mit Minor-Code.

## §7.7 Service Context

**Status:** `partial` — Kodierung der Liste und die Standard-IDs sind getestet; kein Test prüft die Verarbeitungsregeln §7.7.2 (unbekannte oder versionsfremde Kontexte ohne Exception weiterreichen und Interceptoren zugänglich machen).

## §7.8 Coder/Decoder Interfaces

**Status:** `open` — kein `Codec` und keine `CodecFactory` im Baum.

## §7.9 Feature Support and GIOP Versions

**Status:** `partial` — die Abhängigkeiten von GIOP 1.0 bis 1.2 sind umgesetzt und getestet; GIOP 1.3 und 1.4 fehlen, eigenes Item.

## §7.9 GIOP 1.3 und 1.4

**Status:** `open` — `crates/corba-giop/src/version.rs` kennt nur 1.0, 1.1 und 1.2.

## §7.10 Code Set Conversion

**Status:** `partial` — Aushandlung, Service-Kontext und Registry-IDs sind getestet; die Umsetzung der Zeichendaten in ein ausgehandeltes Codeset außer UTF-8/UTF-16 fehlt, eigenes Item.

## §7.10 Umsetzung in ein ausgehandeltes Codeset außer UTF-8/UTF-16

**Status:** `open` — `crates/corba-giop/src/lib.rs` beschreibt die ISO-Latin-Umsetzung als Aufgabe des Aufrufers; `crates/wire/` kodiert nur UTF-8 und UTF-16.

## §2 Conformance and Compliance

**Status:** `partial` — Container, Komponentenmodell, EJB-Integration und Lightweight-Profil sind als Items §6–§17 geführt; dem Basic Level fehlt der CIDL-Parser (Item „§8 OMG CIDL — verbleibende Konstrukte“, open).

## §8 OMG CIDL Syntax and Semantics

**Status:** `partial` — Composition, Home-Executor und die Persistence- Konstrukte (`storagetype`/`storagehome`) sind als AST vorhanden und über die Kategorie-Regel geprüft. Es fehlen: ein CIDL-Parser (das Modul sagt selbst, die Sprache sei Caller-Layer), die Segment-Definition (`segment`-Executor je Facet-Gruppe), die Facet-/Feature-Delegation (`delegatesto`/`abstract storagehome`) und der Proxy Home. Der Rest steht als eigenes Item in `corba-3.3.open.md`.

## §8 OMG CIDL — verbleibende Konstrukte

**Status:** `open` — im Baum existiert nur das Datenmodell aus §8 oben. Es gibt keinen Lexer/Parser für CIDL-Quelltext (das Modul `crates/corba-ccm/src/cidl.rs` sagt selbst, die Sprache sei Caller-Layer), keinen `Segment`-Typ, keine Delegations-Klausel und keinen Proxy Home. Plan: die CIDL-Produktionen an den bestehenden IDL-Frontend (`crates/idl-cst` + `crates/idl-parse`) anhängen und die vier Konstrukte als AST-Knoten ergänzen; das Datenmodell aus §8 ist das Ziel des Lowerings.
