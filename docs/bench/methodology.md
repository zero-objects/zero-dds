# Cross-Vendor Roundtrip-Matrix — Methodik

Stabile Methodik-Beschreibung für die `dds-roundtrip-bench`-Matrix.
Die datierten Ergebnis-Reports (`docs/bench/YYYY-MM-DD-*-roundtrip-matrix.md`)
referenzieren dieses Dokument; Roh-Daten liegen unter
`docs/bench/data/`, maschinenlesbar als JSON unter
`website/_data/roundtrip-matrix.json`.

## Was gemessen wird

End-to-End **DCPS-Roundtrip-Latenz** (Request → Echo → zurück) über
fünf DDS-Implementierungen, *jeder gegen jeden inklusive sich selbst*
(5×5 = 25 Vendor-Paare), über elf Payload-Größen.

Es ist ein **Apples-to-Apples**-Vergleich: alle Vendoren benutzen
dieselbe IDL und dasselbe Wire-Format, jeder über seinen *eigenen*
nativen Code-Generator und seine native DCPS-API. Gemessen wird die
*vollständige* Pipeline — typisierte App-Schicht, CDR-Encode/Decode,
DCPS (HistoryCache, QoS), RTPS-Wire-Protokoll, UDP-Transport — nicht
nur ein Teilstück.

## Vendoren

| Vendor | Version | Binding im Bench |
|---|---|---|
| ZeroDDS | 1.0.0-rc.2 | C-FFI (`zerodds-c-api`) + idl-cpp-Codegen |
| Cyclone DDS | 0.10.5 | ISO-C++-PSM, `idlc -l cxx` |
| eProsima Fast-DDS | 2.14 | C++-Binding, `fastddsgen` |
| RTI Connext | 7.7.0 | ISO-C++-PSM, `rtiddsgen` |
| OpenDDS | 3.31 | klassisches C++-Mapping, `opendds_idl` |

## Shared IDL

`tests/perf/dds-roundtrip-bench/roundtrip.idl`:

```idl
@topic @final @autoid(SEQUENTIAL)
struct Roundtrip {
    @id(0) unsigned long           sequence_id;
    @id(1) unsigned long long      t_send_ns;
    @id(2) sequence<octet, 8192>   payload;
};
```

* `@final` — keine Extensibility, kompakteste Wire-Form.
* `@id`/`@autoid(SEQUENTIAL)` — pinnt die Member-IDs, damit alle
  Vendor-Codegens einen byte-identischen COMPLETE-TypeObject ableiten
  (sonst lehnt RTIs strikte Type-Consistency fremde TypeObjects ab).
* `sequence<octet, 8192>` — bounded, damit kein Vendor einen
  abweichenden Default-Bound nimmt.

## Harness

* `matrix_sweep.sh` — fährt die 5×5×11 = 275 Zellen. Pro Zelle: pong
  im Hintergrund starten, ping zu Ende laufen lassen, pong killen.
  Schreibt eine CSV inkrementell, ein Retry bei Discovery-Flake.
* `matrix_report.py` — rendert den Markdown-Report (p50-Matrix pro
  Payload-Größe + Hinweise).
* `matrix_json.py` — rendert die maschinenlesbare JSON für den
  Website-Doc-Builder.
* Per-Vendor-App: `{zerodds,cyclone,fastdds,rti,opendds}_app.cpp` —
  typisiertes Ping/Pong, jeweils über das native Vendor-Codegen.

## QoS- und Wire-Konfiguration

Identisch über alle Vendoren — der verpflichtende Cross-Vendor-Baseline:

* **`RELIABLE`**, **`KEEP_LAST(64)`**.
* **Data-Representation `XCDR1`** — explizit erzwungen (OpenDDS-Default
  wäre XCDR2).
* Alle DataReader auf **`ALLOW_TYPE_COERCION`** — der Endpoint-Match
  läuft über den cross-vendor byte-identischen COMPLETE-TypeObject
  statt über den MINIMAL-Hash (XTypes 1.3 §7.6.3); ohne das lehnt
  Cyclone (DISALLOW-Default) RTIs abweichenden MINIMAL-TypeObject ab.
* OpenDDS-IDL mit **`-Gxtypes-complete`** gebaut (OpenDDS#4244 /
  cyclonedds-cxx#448) — sonst emittiert OpenDDS nur minimal-
  TypeObjects, die strikte Consumer ablehnen.
* OpenDDS zusätzlich mit RTPS-Discovery-`.ini` (Default wäre InfoRepo).

## Mess-Parameter

* **2000 Samples + 200 Warmup** pro Zelle.
* **Ein Sample in flight** (Ping wartet auf das Echo, bevor es das
  nächste schickt) — reine Latenz, kein Pipelining.
* **Pong event-driven** — Listener/Data-Callback, kein Busy-Poll.
* **Metrik: p50** (Median) der Roundtrip-Latenz. Median ist robust
  gegen Scheduler-Hiccups; min/p90/p99/p999/max stehen in der CSV.
* Payload-Achse: **0 … 8192 Byte in 11 Schritten** (10%-Stufen):
  0, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192.

## Host

`codepit` — LXC auf einem AMD Ryzen Threadripper PRO 3955WX
(16C/32T, Zen 2). Verkehr über Linux-Loopback (`lo`). Same-host —
es ist *kein* Netzwerk-Test, sondern misst die Stack-Pipeline ohne
Netz-Variabilität. Alle fünf Vendoren laufen auf demselben Host unter
identischen Bedingungen; der Vendor-Vergleich ist damit fair.

## Bewusste Grenzen — was *nicht* gemessen wird

* **Kein Netzwerk** — Loopback hat ~65 KB MTU und keine Paket-Verluste.
  Cross-Host-Verhalten (Fragmentierung am 1500-MTU-Pfad, Loss,
  Reordering) ist separat.
* **Loopback-MTU** — same-host fragmentiert ZeroDDS nicht (ein
  8-KB-Datagramm geht in einem Stück); auf einem echten Ethernet-Pfad
  greift die 1344-B-Fragmentierung.
* **p50, nicht Tail** — der Report zitiert den Median; die volle
  Quantil-Verteilung (bis max) steht in der CSV.
* **codepit ist kein getunter Bench-Host** — kein RT-Pinning, kein
  isolierter Governor, geteilte LXC. Absolute Zahlen sind nicht
  „zitierfähig" im Sinne von `docs/perf/methodology.md`; der
  *relative* Vendor-Vergleich auf identischem Host ist aussagekräftig.

## Reproduzieren

```bash
# Voraussetzung: die fünf <vendor>-roundtrip-Binaries in build/,
# Cyclone/Fast-DDS/RTI/OpenDDS installiert.
cd tests/perf/dds-roundtrip-bench
mkdir build && cd build
cmake -DZERODDS_REPO=$(git rev-parse --show-toplevel) ..
cmake --build .
cd ..
./matrix_sweep.sh                       # -> matrix-out/matrix_results.csv
python3 matrix_report.py matrix-out/matrix_results.csv > report.md
python3 matrix_json.py   matrix-out/matrix_results.csv roundtrip-matrix.json
```

## Bekannte Vendor-Lücke

`RTI Connext 7.7 ↔ OpenDDS 3.31` interoperiert nicht — ein
Discovery-Layer-Parsing-Bug in OpenDDS (verifiziert in 3.31 und 3.33;
RTIs Wire-Format ist RTPS-2.5-konform, pcap-belegt). Das betrifft
ausschließlich dieses eine Fremd-Stack-Paar; ZeroDDS interoperiert mit
beiden. Details im jeweiligen datierten Report.
