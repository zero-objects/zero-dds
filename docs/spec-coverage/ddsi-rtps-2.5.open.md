# ddsi-rtps-2.5 — offene Punkte

Aggregat aus `ddsi-rtps-2.5.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.3.3.1 Table 8.14 Header.vendorId

**Status:** `partial` — das Feld wird gesendet und gelesen; ZeroDDS sendet `0x01F0`, laut Kommentar an `VendorId::ZERODDS` ein Zwischenwert und kein von der OMG zugewiesener Wert (eigenes `blocked`-Item unter §8.3.5.2).

## §8.3.5.2 VendorId von der OMG zugewiesen

**Status:** `blocked` — Zuweisung einer VendorId durch die OMG (beantragt); hebt sich auf, sobald die OMG den Wert zuweist und `VendorId::ZERODDS` ihn trägt. Bis dahin sendet ZeroDDS `0x01F0` (`crates/rtps/src/wire_types.rs`, laut Kommentar ein Zwischenwert), einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

## §8.3.8.4 Gap: Inhalt von filteredCount

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt. Der Gap-Codec kennt das Feld (`GAP_FLAG_FILTERED_COUNT`), `append_gap` setzt `filtered_count: None`.

## §8.5.3.2 Table 8.78 vendorId

**Status:** `partial` — ZeroDDS sendet `0x01F0`, keinen von der OMG zugewiesenen Wert (`blocked`-Item unter §8.3.5.2).

## §8.7.2.2.4 TIME_BASED_FILTER auf der Writer-Seite

**Status:** `open` — eine optionale Funktion; ZeroDDS wendet TIME_BASED_FILTER nur auf der Reader-Seite an (`tbf_should_deliver`) und sendet kein GAP für zeitgefilterte Samples.

## §8.7.3 Filterung auf der Writer-Seite

**Status:** `open` — eine optionale Funktion; ZeroDDS filtert nicht auf der Writer-Seite. Die Filterangaben des Readers in SEDP sind unter §8.7.3.1 geführt.

## §8.7.3 Hinweise des Writers: ALIVE_FILTERED, ContentFilterInfo, ausgelassene Daten

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.

## §8.7.3.1 Writer ohne passende Filterklasse

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.

## §8.7.3.2 Gefiltertes Sample anzeigen

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt. Die Data-Submessage hat kein FilteredFlag; das FilteredFlag gehört zu StatusInfo_t (§9.6.4.9).

## §8.7.3.3 GAP statt DATA und Filterergebnisse in der Inline-QoS

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.

## §8.7.3.3 Table 8.87 filterResult

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt; PID_CONTENT_FILTER_INFO wird nicht gesendet.

## §8.7.3.3 Table 8.87 filterSignatures

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt; PID_CONTENT_FILTER_INFO wird nicht gesendet.

## §8.7.3.4 Angaben für ALIVE_FILTERED trotz Filterung beim Writer

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt; jedes Sample geht an jeden gematchten Reader.

## §8.7.4 Wert, der den Filter nicht passiert

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt; ZeroDDS sendet jeden Wert.

## §9.3.1.5 Erste zwei Oktette des guidPrefix = vendorId

**Status:** `partial` — die ersten zwei Oktette sind `01 F0`, kein von der OMG zugewiesener Wert (`blocked`-Item unter §8.3.5.2). `DcpsRuntime::start` nimmt auch einen frei gewählten Prefix an; unter DDS-Security ersetzt der aus der Identität abgeleitete Prefix den gebildeten. Dessen erste 48 Bit legt DDS-Security 1.2 §10.3.3 Table 59 (`validate_local_identity`) fest: erstes Bit 1, dann 47 Bit aus dem SHA-256 des SubjectName; die VendorId steht dort nach der spezielleren Spec nicht (`crates/security-pki/src/guid_adjust.rs`, Test `adjusted_prefix_sets_high_bit_and_is_deterministic_per_identity`).

## §9.3.2.1 ContentFilterInfo_t, FilterResult_t, FilterSignature_t

**Status:** `open` — die Typen fehlen im Code; Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.

## §9.4.4 Zugewiesene VendorId verwenden

**Status:** `blocked` — Zuweisung einer VendorId durch die OMG (beantragt); hebt sich auf, sobald die OMG den Wert zuweist und `VendorId::ZERODDS` ihn trägt. Bis dahin sendet ZeroDDS `0x01F0` (`crates/rtps/src/wire_types.rs`, laut Kommentar ein Zwischenwert), einen Wert, den die OMG-Liste nicht führt (vergeben bis `{0x01,0x1C}`, `{0x01,0x1D}` und `{0x01,0x1E}` reserviert).

## §9.6.4 Table 9.20 PID_CONTENT_FILTER_INFO (0x0055)

**Status:** `open` — ZeroDDS kennt den PID nicht; ein empfangener Parameter wird als unbekannter PID ohne Must-Understand-Bit übergangen. Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.

## §9.6.4.1 PID_CONTENT_FILTER_INFO: Leitungsbild, Bitmap, Signatur

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.

## §9.6.4.1 Table 9.21 filterResult

**Status:** `open` — Teil der optionalen Filterung auf der Writer-Seite (§8.7.3), die ZeroDDS nicht ausführt.
