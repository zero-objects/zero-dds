# CORBA Cross-ORB-Interop — ZeroDDS ↔ omniORB / TAO / JacORB (2026-06-06)

Nachweis echter GIOP-1.2-IIOP-Interoperabilität des ZeroDDS-CORBA-Stacks mit
den drei aktiv verfügbaren Fremd-ORBs — **bidirektional** (ZeroDDS als Client
*und* als Server) und über eine breite CORBA-Feature-Matrix.

Harness: `crates/corba-interop/competitors/run_interop.sh` (Host: codepit,
Debian 13, Loopback). Referenz-Bench (Latenz): siehe
[`corba-iiop-roundtrip-2026-06-06.md`](corba-iiop-roundtrip-2026-06-06.md).

## Ergebnis — alle 6 Kombinationen GRÜN

| Fremd-ORB | Version | Wire-Byte-Order | ZeroDDS-Client → Fremd-Server | Fremd-Client → ZeroDDS-Server |
|---|---|---|---|---|
| omniORB | 4.3.3 | Little-Endian | ✅ Echo + Bench | ✅ Echo + Bench |
| TAO | 2.5.24 (ACE+TAO) | Little-Endian | ✅ Echo + Bench | ✅ Echo + Bench |
| JacORB | 3.9 (JDK 8) | **Big-Endian** | ✅ Echo + Bench | ✅ Echo + Bench |

JacORB sendet Big-Endian-IORs **und** Big-Endian-GIOP-Requests; omniORB/TAO
senden Little-Endian. Beide Pole grün ⇒ „receiver makes it right" (CDR §15.4.1,
Byte-Order als Per-Message-Flag) ist korrekt umgesetzt.

## Getestete Feature-Matrix (gemeinsame `competitors/interop.idl`)

Beide Seiten generieren Stubs/Skeletons aus **identischer IDL** und tauschen
portable stringified-IORs (`IOR:<hex>`, §13.6.10) aus.

| Operation | Abgedeckte CORBA-Features |
|---|---|
| `Echo::ping(string)` | `string` (inkl. 4 kB), GIOP-Request/Reply-Roundtrip |
| `Bench::add(long,long)` | `long` (4-Byte-Primitive) |
| `Bench::scale(double,double)` | **`double`** (8-Byte-aligned) |
| `Bench::add64(long long,long long)` | **`long long`** (8-Byte-aligned) |
| `Bench::concat(string,string)` | mehrere `string`-Argumente |
| `Bench::reverse(sequence<long>)` | `sequence<long>` (CDR-Sequence) |
| `Bench::divmod(.., out long, out long)` | **`out`**-Parameter |
| `Bench::increment(inout long)` | **`inout`**-Parameter |

Die 8-Byte-Typen (`double`, `long long`) sind der schärfste Test: GIOP 1.2
richtet den Request-/Reply-Body unbedingt auf 8 Byte aus, gemessen ab
Message-Anfang.

## Auf dem Weg gefundene & behobene Wire-Bugs

Echte Cross-ORB-Tests deckten vier Spec-Konformitätslücken auf, die im
reinen ZeroDDS↔ZeroDDS-Self-Test unsichtbar waren (beide Seiten teilten
dieselbe Fehl-Konvention):

1. **GIOP-1.2-Body-Alignment-Origin** (§15.4.2/§15.4.4) — der Body wurde in
   einen Sub-Stream encodiert, dessen Alignment ab Body-Anfang statt ab
   Message-Anfang (inkl. 12-Byte-Header) zählte. 8-aligned-Felder landeten bei
   abs. Offset ≡4 mod 8. Fix: `BufferWriter/Reader::with_align_origin`.
2. **CDR-Encapsulation-Alignment-Origin** (§15.3.3) — das Endianness-Octet
   (Index 0) zählt zur Alignment-Origin; ZeroDDS sliced `&bytes[1..]` und
   misalignte das erste 4-Byte-Feld (host/type_id) → Fremd-IORs unparsbar.
3. **LocateRequest-Handling** — omniORB/TAO proben vor dem ersten Call die
   Object-Location (§15.4.5); der Server muss mit LocateReply antworten,
   sonst blockiert der Client (COMM_FAILURE).
4. **LocateReply-Trailing-Padding** — bei leerem Body (ObjectHere) durfte
   kein 8-Byte-Align-Padding emittiert werden, sonst „Garbage left at end".

## Codegen-Pfad — Speed-Gegenprobe

Der frühere Latenz-Bench fuhr einen hand-marshallten Servant. Der
generierte Stub/Skeleton-Pfad (`echo_bench_codegen`) liegt praktisch gleichauf:

| Pfad | p50 (32 B, codepit) |
|---|---|
| Hand-marshalled | 17.1 µs |
| **Generierter Codegen** | **16.8 µs** |

Kein relevanter Codegen-Overhead — die Latenz-Aussagen gelten auch für den
realen, IDL-generierten Aufrufpfad.

## Reproduktion

```sh
# auf codepit (omniORB, TAO via /opt/opendds-secure, JacORB via /opt/jacorb + JDK8)
bash crates/corba-interop/competitors/run_interop.sh
```

Self-Interop (ohne Fremd-ORBs) + Feature-Matrix-Roundtrips:

```sh
cargo test -p zerodds-corba-interop --test codegen_roundtrip
```
