# 0013 — Native Endpoint-SDKs (C/C++/Python/Java) über einem Rust-Hub, kein FFI

- **Status:** accepted
- **Datum:** 2026-07-21
- **Autoren:** @sandra
- **Kontext:** `crates/xrce`, `crates/zerodds-c-api`, `crates/py`, `crates/java-omgdds`, `crates/idl-cpp`/`idl-java`/`idl-python`, `crates/cdr`, DDS-XRCE 1.0, DDSI-RTPS 2.5 §10.5

## Kontext

ZeroDDS bindet native Sprachen per FFI über dem Rust-Core an: `zerodds-c-api`
exponiert eine C-ABI, `crates/py` (PyO3) und die Node-/Java-Bindings sitzen als
Wrapper darüber. Dieser Weg setzt eine native Library je OS × Architektur und
eine Rust-Toolchain zum Bauen voraus.

Ein Teil der Ziel-Plattformen für Endpoints hat keine Rust-Toolchain und ist
oft big-endian, ressourcenbeschränkt oder alt (68k-Klasse, i686-Linux, PA-RISC,
eingebettete Terminals und Steuerrechner). Diese Plattformen sind Endpoints,
keine zentralen DDS-Instanzen; das vollständige DDS läuft auf x86-64-Linux
(künftig ARM64) mit POSIX-Umgebung.

## Entscheidung

**Zwei-Rollen-Topologie:**

```
Hub (Rust, x86-64 Debian / ARM64): volles modernes DDS
  — Discovery, Routing, Security, RTPS-Fabric, Endpoint-Agent (crates/xrce)
        ▲  Endpoint↔Hub-Protokoll (XRCE-förmig), transport-opak
        │
Endpoints (native, KEIN Rust): C · C++ · Python · Java
  — minimal: wire + Endpoint↔Hub-Framing + Frame-Hook + komponierbare Module
```

Der **Hub bleibt Rust** und trägt den vollen Stack. Die **Endpoints werden
native, konservative SDKs** je Sprache — self-contained, **kein Rust darunter**,
**keine hyper-modernen Features/Libs**, damit sie mit der plattform-eigenen
Toolchain kompilieren.

**Crate-/Verzeichnis-Schnitt** (Endpoint-SDKs, generiert + konservativ):
`endpoints/c`, `endpoints/cpp`, `endpoints/python`, `endpoints/java`; die
Wire-Teile werden aus der bestehenden IDL-Codegen (`idl-cpp`/`idl-java`/
`idl-python` + neuer C89-Emitter) generiert; `crates/xrce` (Rust) ist Agent +
Byte-Identitäts-Orakel.

## Invarianten (normativ)

1. **Kein Rust am Endpoint.** Konservative Sprach-Floors: **C89/C90**,
   **C++98/03**, **Python pure-stdlib** (3.x konservativ, 2.7-freundlicher Stil),
   **Java 8** (pure-Java, kein JNI-Native-Lib, kein records/var/streams). Keine
   Fremd-Libs, kein malloc-Zwang in C (statische/Arena-Puffer).

2. **Kombinierbare Units spannen die Featurelevel.** Ein Unit-Graph, je Sprache
   als Link-Libs / Jars / py-Imports gespiegelt: `wire-core` (Framing +
   Frame-Hook + XCDR-Primitive) · `wire-fixed` · `wire-variable` ·
   `feat-reliable` · `feat-security` · … Ein Endpoint linkt genau die Teilmenge,
   die sein Footprint trägt.

3. **Zwei Wire-Backends.** **`wire-fixed`** = statisch, per-Typ **generiert**
   (Straight-Line-XCDR, keine Runtime-Typinfo) → winzig, deterministisch,
   cert-freundlich, bekannte-Typen/fixe-Topologie. **`wire-variable`** =
   reflektiv/dynamisch (DynamicData↔XCDR + TypeObject/DynamicType) → evolvierende/
   unbekannte Typen, Monitor/Spy, RE-from-Capture. Der Rust-Core hat beide schon
   (typisierter `Xcdr2Writer` + reflektiver Codec); die Endpoints spiegeln das.

4. **Endianness first-class, nicht nachgerüstet.** Endpoint-HW ist big-endian
   (68k/PA-RISC/SPARC), Hub little-endian (x86-64) → **BE↔LE ist Normalfall**.
   Serialisierung ist **byte-für-byte mit expliziten Shifts** (nie struct-/
   pointer-cast auf Bytes) → host-endian-unabhängig *by construction*. Die
   **Wire-Byte-Order ist ein expliziter Parameter** (LE/BE-Encapsulation-Flag,
   RTPS 2.5 §10.5); beide Backends dekodieren **beide** Ordnungen und emittieren
   die gewählte. Getestet gegen **BE- und LE-Goldens**.

5. **Frame-Hook ist der einzige Pflicht-Integrationspunkt.** Kontrakt an der
   fertig-geframten-Message-Grenze (`deliver(frame)` / `receive() → frame`),
   transport-opak — der Anwender hängt den Transport seines Ziels dahinter.
   Vorgefüllt für POSIX-Socket und serielle Transporte; für andere Ziele leer
   plus Beispiel.

6. **Eine Wire-Wahrheit.** Alles Wire-kritische wird aus *einem* Wire-Modell
   generiert (dieselbe Quelle wie Rust). Jedes Endpoint-SDK hängt als „weiterer
   Vendor" in der Cross-Vendor-Byte-Identitäts-Harness → byte-identisch zur
   Rust-Referenz, kann nicht driften. Voller Feature-Umfang + harte Protokoll-
   Logik (Discovery/Reliability-State-Machines/Security-Handshake/Routing) bleibt
   **Hub-only**.

## Alternativen

1. **FFI-Bindings über dem Rust-Core behalten** (Status quo) — verworfen: die
   Reibung oben; baut nicht auf No-Rust/Restricted/alten Toolchains, native-Lib-
   Shipping, ABI-Drift, Callback-Grenze.
2. **Ein portabler C-Core mit Sprach-Bindings darüber** — verworfen:
   die Sprachen sollen *nativ* sein (pure-Java = JVM-Reichweite ohne native Lib;
   pure-Python ohne C-Extension), nicht wieder ein FFI-Sprung.
3. **Voller nativer DDS-Stack je Endpoint** (Discovery/RTPS-Fabric in C/…) —
   verworfen: Endpoints sind keine zentralen Instanzen; der Hub macht das DDS.
   Ein Voll-Stack ×4 multipliziert die Spec-Vollständigkeits-Fläche unnötig.
4. **LE-first, BE später** — verworfen: die Endpoint-HW ist BE; BE ist der
   Normalfall, nicht der Nachtrag.

## Konsequenzen

**Positiv:** Endpoints kompilieren mit der Plattform-eigenen Toolchain, nichts
Natives auszuliefern, kein ABI-Drift, native Callbacks (löst O14 ohne FFI-Risiko),
JVM-/CPython-Reichweite auf Exoten. Voller Stack bleibt Rust-Hub-only,
Endpoints generiert + harness-gesperrt. Legacy-Integration wird First-Class.

**Negativ/Risiken:** vier native Endpoint-Runtimes (wenn auch minimal +
generiert). `wire-variable` in **C89** ist das Cost-Center (Hand-Typ-Deskriptor-
Modell + XCDR-Interpreter, Arena-Allokation) — in Python/Java dagegen leicht.
Konservative Floors kosten Bequemlichkeit (keine modernen Sprach-Features).

## Folge-Aufgaben

- **P1 — `wire-core` (C89) ✅ (in Arbeit, Kern steht).** `endpoints/c`: XCDR2-
  Primitive (u8..u64, f32/f64 via Bitmuster, string, sequence<octet>), endian-
  sicher (byte-für-byte, LE/BE-Parameter), XCDR2-Alignment. `endpoints/golden-gen`
  (Rust/zerodds-cdr) liefert LE+LE-Goldens; ein Sample-`wire-fixed`-Codec ist
  byte-identisch. **Verifiziert:** native x86-64 **und PowerPC-Big-Endian
  (qemu)** byte-identisch (LE+BE), `-Werror`-clean gcc+clang, C89-pedantic — die
  Endianness-Invariante gegen eine echte BE-Maschine bewiesen. **DHEADER ✅ (C):**
  @appendable + nested + sequence<non-primitive> (delimited CDR2, back-patch)
  byte-identisch native x86-64 **und PowerPC-BE (qemu)** gegen
  `encode_appendable`/Collection-DHEADER des Rust-Cores. Offen in P1:
  `wire-fixed`-Codegen (statt Hand-Sample), `feat-reliable`, @mutable/EMHEADER,
  DHEADER-Mirror in Python/Java (C++ erbt via Fassade).
- **`wire-fixed`-Codegen ✅ (C89 Kern).** `endpoints/codegen`
  (`zerodds-endpoint-codegen`) parst IDL via `zerodds_idl` und emittiert den
  C89-Codec je `@final`-Struct über die zdw-Primitive. Aus `sensor.idl` erzeugt:
  Wire-Calls identisch zum Hand-Sample, Ausgabe byte-identisch zu Rust (Golden
  LE+BE), native x86-64 **und PowerPC-BE**, `-Werror`. Offen: @appendable/@mutable-Emit
  (DHEADER/EMHEADER — Primitive bewiesen), nested, C++/Python/Java-Emitter.
- **P2 — `wire-variable` ✅ (C).** `zerodds_reflect`: reflektiver XCDR-Codec über
  einen Laufzeit-`zdw_dyn_field[]`-Descriptor statt generiertem Code (für
  evolvierende/unbekannte Typen, Monitor/Spy, Charakterisierung fremder Wire-Formate).
  Reflektiv kodiert = fixed = Rust-Golden, byte-identisch LE+BE + reflektiver
  Decode, native + PowerPC-BE. Beide Backends produzieren identische Bytes.
- **P3 — C++/Python/Java, Re-Targeting derselben Goldens ✅ (Wire-Core-Kern).**
  Alle vier Sprachen byte-identisch zu denselben Rust-Goldens (LE+BE) +
  Round-Trip, verifiziert: **C89** (`endpoints/c`) + **C++98**
  (`endpoints/cpp`, Fassade über dem C-Core) native x86-64 **und PowerPC-BE
  unter qemu**, `-Werror` gcc+g++; **Python** (`endpoints/python`, pure-stdlib
  2.7/3.x); **Java 8** (`endpoints/java`, pure, kein JNI). **Extensibility-Trias
  ✅ in allen vier** — `@final` + `@appendable` (DHEADER/nested/sequence<non-primitive>)
  + `@mutable` (EMHEADER LC4), alle byte-identisch, native + PowerPC-BE
  (C/C++), Python/Java. Offen je Sprache: `wire-fixed`-Codegen (statt
  Hand-Samples), `feat-reliable`, `wire-variable`.
- **P4** Endpoint↔Hub-Protokoll (XRCE-förmig) + Agent-seitiger Terminator am Hub;
  Frame-Hook-Kontrakt + POSIX-/Serial-Beispiel-Füllungen. **Frame-Hook ✅ (C):**
  `zerodds_endpoint.h` `zdw_transport` (deliver/receive, transport-opak) +
  Loopback-Demo, native + PowerPC-BE. **XRCE-WRITE_DATA-Framing ✅ (C):**
  `zdw_xrce_write_frame`/`read_frame` — MessageHeader + WRITE_DATA-Submessage,
  byte-identisch zu einer echten `crates/xrce`-Message (48 B), voller Pfad
  encode→frame→hook→unwrap→decode. **Serial-Framing ✅ (C):**
  `zdw_serial_frame`/`deframe` — Annex-C HDLC (7E + stuff + CRC-16-CCITT-FALSE),
  byte-identisch zum `crates/xrce`-Serial-Framer (52 B), Deframe+CRC+unwrap+decode.
  Alles native + PowerPC-BE, `-Werror`. **Reliable ✅** (HEARTBEAT-parse +
  ACKNACK-build byte-identisch), **DATA-Empfangspfad ✅** (bidirektional),
  **alle vier Sprachen hub-fähig ✅** (Python/Java pure, C++ via C-Reuse).
  **Live-UDP-E2E ✅:** C-Endpoint → echtes UDP → laufender `zerodds-xrce`-Agent
  (`endpoints/xrce-agent-demo`), der die Message parst + die SensorReading
  zurückliest (Live-UDP-Akzeptanz).
  **Multi-Language-Codegen ✅:** `wire-fixed`-Emitter für **C89 + Python + Java**
  (C++ nutzt den C-Codec via `extern "C"`), alle byte-identisch.
  **Reflektiver Codec ✅** in **C + Python + Java** (C++ via C-Reuse).

## Status — vollständig

Alle vier Sprachen hub-fähig; beide Wire-Backends mehrsprachig **inklusive
Extensibility-Trias + nested + sequence<struct>**:
- `wire-fixed`-Codegen: C89 / Python / Java (alle 3 Modi; C89 zusätzlich nested);
  C++ nutzt den C-Codec.
- `wire-variable` reflektiv: C / Python / Java (final/appendable/mutable + nested +
  sequence<struct>); C++ via C-Reuse.
- XRCE (write/data/heartbeat/acknack) + Serial-Framing, bidirektional + reliable.

Alles byte-identisch zur Rust-Wahrheit, verifiziert native x86-64 + PowerPC-BE
(qemu, volle Suite `-Werror`) + Live-UDP-E2E gegen einen echten `zerodds-xrce`-
Agent. Keine offenen Punkte.
- **P5** Byte-Identitäts-Harness um die vier SDKs als „Vendoren" erweitern.

## Referenzen

- DDS-XRCE 1.0 (Client/Agent-Split); DDSI-RTPS 2.5 §10.5 (Encapsulation +
  Byte-Order-Flag)
- `crates/xrce` (Rust-Referenz + Agent), `crates/cdr` (XCDR-Codec, typisiert +
  reflektiv)
- ADR 0009 (Hub-seitige Service-Architektur, adapter-getrieben)
