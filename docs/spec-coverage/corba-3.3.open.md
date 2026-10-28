# corba-3.3 — offene Punkte

Aggregat aus `corba-3.3.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8 OMG CIDL Syntax and Semantics

**Status:** `partial` — Composition, Home-Executor und die Persistence- Konstrukte (`storagetype`/`storagehome`) sind als AST vorhanden und über die Kategorie-Regel geprüft. Es fehlen: ein CIDL-Parser (das Modul sagt selbst, die Sprache sei Caller-Layer), die Segment-Definition (`segment`-Executor je Facet-Gruppe), die Facet-/Feature-Delegation (`delegatesto`/`abstract storagehome`) und der Proxy Home. Der Rest steht als eigenes Item in `corba-3.3.open.md`.

## §8 OMG CIDL — verbleibende Konstrukte

**Status:** `open` — im Baum existiert nur das Datenmodell aus §8 oben. Es gibt keinen Lexer/Parser für CIDL-Quelltext (das Modul `crates/corba-ccm/src/cidl.rs` sagt selbst, die Sprache sei Caller-Layer), keinen `Segment`-Typ, keine Delegations-Klausel und keinen Proxy Home. Plan: die CIDL-Produktionen an den bestehenden IDL-Frontend (`crates/idl-cst` + `crates/idl-parse`) anhängen und die vier Konstrukte als AST-Knoten ergänzen; das Datenmodell aus §8 ist das Ziel des Lowerings.
