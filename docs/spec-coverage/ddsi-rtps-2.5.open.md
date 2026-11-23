# ddsi-rtps-2.5 — offene Punkte

Aggregat aus `ddsi-rtps-2.5.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.3.3.1 Table 8.14 Header.vendorId

**Status:** `partial` — das Feld wird gesendet und gelesen; ZeroDDS sendet `0x01F0`, laut Kommentar an `VendorId::ZERODDS` ein Zwischenwert und kein von der OMG zugewiesener Wert (eigenes `blocked`-Item unter §8.3.5.2).

## §8.3.5.2 VendorId von der OMG zugewiesen

**Status:** `blocked` — Zuweisung einer VendorId durch die OMG (beantragt); hebt sich auf, sobald die OMG den Wert zuweist und `VendorId::ZERODDS` ihn trägt. Bis dahin sendet ZeroDDS `0x01F0` (`crates/rtps/src/wire_types.rs`, laut Kommentar ein Zwischenwert), einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

## §8.5.3.2 Table 8.78 vendorId

**Status:** `partial` — ZeroDDS sendet `0x01F0`, keinen von der OMG zugewiesenen Wert (`blocked`-Item unter §8.3.5.2).

## §8.7.2.2.4 TIME_BASED_FILTER auf der Writer-Seite

**Status:** `open` — eine optionale Funktion; ZeroDDS wendet TIME_BASED_FILTER nur auf der Reader-Seite an (`tbf_should_deliver`) und sendet kein GAP für zeitgefilterte Samples.

## §9.3.1.5 Erste zwei Oktette des guidPrefix = vendorId

**Status:** `partial` — die ersten zwei Oktette sind `01 F0`, kein von der OMG zugewiesener Wert (`blocked`-Item unter §8.3.5.2). `DcpsRuntime::start` nimmt auch einen frei gewählten Prefix an; unter DDS-Security ersetzt der aus der Identität abgeleitete Prefix den gebildeten. Dessen erste 48 Bit legt DDS-Security 1.2 §10.3.3 Table 59 (`validate_local_identity`) fest: erstes Bit 1, dann 47 Bit aus dem SHA-256 des SubjectName; die VendorId steht dort nach der spezielleren Spec nicht (`crates/security-pki/src/guid_adjust.rs`, Test `adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`).

## §9.4.4 Zugewiesene VendorId verwenden

**Status:** `blocked` — Zuweisung einer VendorId durch die OMG (beantragt); hebt sich auf, sobald die OMG den Wert zuweist und `VendorId::ZERODDS` ihn trägt. Bis dahin sendet ZeroDDS `0x01F0` (`crates/rtps/src/wire_types.rs`, laut Kommentar ein Zwischenwert), einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).
