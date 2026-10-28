# zerodds-opcua-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-opcua-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5.3 Typformen-Abdeckung

**Status:** `partial` — Primitive/Strings (narrow und wide, `wchar`→ `UInt16`/`wstring`→`String`)/Enums/Bitmasks/verschachtelte Structs/ `map<K,V>` (ein Leaf pro Eintrag, Pfad `<feld>.<schlüssel>`)/Arrays- Sequences von alledem: done. Array/Sequence von Struct/Union, `bitset`, `fixed<P,S>`, `float128`: offen, benannt statt angenähert — kein OPC-UA-Builtin-`VariantValue`-Fall passt (Part 6 §5.1.2s Typtabelle hat keine Zeile für keinen der drei; aus dieser Abwesenheit gefolgert, nicht aus einer konkreten Gateway-1.0-§9.2-Textstelle, die dieses Repo vorliegen hätte). Offen, nicht stillschweigend: ein neuer `map`-Schlüssel, den eine Probe noch nie gesehen hat, wird als unbekannter Pfad gemeldet, nicht live als neuer Knoten materialisiert (`AddressSpace`/`PublishedDataSet` bauen ihre Knoten-/Feldmenge einmalig aus dem Table-9-Default).

## §8.2 Prometheus-Metriken

**Status:** `partial` — Endpunkt funktioniert (gemessen), kein automatisierter Test.

## §9.3 SIGHUP

**Status:** `partial` — Code-Pfad vorhanden, nicht automatisiert getestet; Reload-Umfang bewusst nicht erweitert: `opcua.server.historical.topics`, `acl.users`/`acl.allow_anonymous`, `opcua.pubsub.connections[].security` und `metrics.otlp_endpoint` brauchen alle einen Neustart, wie zuvor `<node_id>`/`<browse_path>` und der IDL/Gateway-Config-Themensatz.

## §10 Cross-Vendor-Interop

**Status:** `partial` — Daemon-Ebene-Test existiert und ist strukturell verifiziert (Skip/Hart-Fehl-Pfade), aber nicht gegen ein echtes open62541 gelaufen; Bibliotheksebene weiterhin gemessen (`crates/opcua-server/proofs/open62541.sh`).
