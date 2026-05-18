# `zerodds-c-api`

ZeroDDS **C-FFI** — Cross-Language-Hub fuer C, C++, C# und TypeScript.
Exportiert eine `extern "C"`-Schicht ueber die ZeroDDS-DCPS-Runtime,
die alle Nicht-Rust-Bindings als gemeinsame Binary-Schnittstelle
nutzen.

Teil des Projekts [**ZeroDDS**](../../README.md). Safety-Klasse
**STANDARD** — `unsafe` erlaubt am FFI-Boundary, jeder Block traegt
einen `// SAFETY:`-Kommentar; interner Code ist `unsafe`-frei.

---

## Wer dockt hier an

| Binding | Pfad zur Crate | Mechanismus |
| --- | --- | --- |
| Reines C | direkt | `#include <zerodds.h>` + `-lzerodds` |
| C++17-RAII-Wrapper | [`crates/cpp`](../cpp/README.md) | `#include <zerodds/dds.hpp>` ueber die C-API |
| C# (P/Invoke, NativeAOT) | [`crates/cs`](../cs/README.md) | `DllImport("zerodds")` |
| TypeScript / Node.js | [`crates/ts-node`](../ts-node/README.md) | koffi-FFI ueber `libzerodds.so` |
| ROS-2-RMW-Shim | [`crates/rmw-zerodds-shim`](../rmw-zerodds-shim/README.md) | Apex.AI-Plugin-Modell |

Python (`crates/py`) und Java (`crates/java-omgdds`) gehen
**nicht** ueber die C-API — sie nutzen pyo3- bzw. jni-rs-direkte
Bruecken in den Rust-Stack.

## Quick Start — reines C

`zerodds.h` wird per `cbindgen` aus der Rust-Source generiert (siehe
`build.rs`) und in `include/zerodds.h` eingecheckt.

```c
#include <stdio.h>
#include <zerodds.h>

int main(void) {
    zerodds_runtime_t* rt = zerodds_runtime_create();
    zerodds_participant_t* p =
        zerodds_runtime_create_participant(rt, /* domain_id */ 0);

    zerodds_topic_t* t =
        zerodds_participant_create_topic(p, "Greetings", "Greeting");

    zerodds_writer_t* w = zerodds_participant_create_writer(p, t);
    uint8_t payload[] = { /* CDR-encoded "Greeting{id:42,text:'hi'}" */ };
    zerodds_writer_write(w, payload, sizeof(payload));

    zerodds_writer_destroy(w);
    zerodds_topic_destroy(t);
    zerodds_participant_destroy(p);
    zerodds_runtime_destroy(rt);
    return 0;
}
```

Build (Linux/macOS, mit gebauter `libzerodds.so` im `LD_LIBRARY_PATH`):

```bash
clang -std=c11 -lzerodds main.c -o demo
./demo
```

## Type-Modell — bewusst byte-orientiert

Das FFI nimmt fuer alle Samples **rohe CDR-Bytes**:

```c
zerodds_writer_write(writer, sample_bytes, sample_len);
zerodds_reader_take(reader, &out_buf, &out_len, ...);  // out_buf via _free()
```

Die CDR-Encode-/Decode-Logik lebt in den Sprach-Bindings:
`idl-cpp` emittiert C++-Encoder, `idl-csharp` C#-Encoder, etc. Das
C-FFI ist neutral — Wire-Drift-Tests gehen bytes-genau durch.

Vorteile:
* Keine Generic-Type-Akrobatik durch FFI-Grenze.
* Apex.AI-Plugin und ROS-2-RMW behalten ihre eigenen Marshaling-Pfade.
* Wire-Vector-Conformance ist trivial zu validieren.

## Handle-Modell

Alle Objekte sind opaque-Pointer. Caller muessen `*_destroy()` paaren:

| Constructor | Paar-Destructor |
| --- | --- |
| `zerodds_runtime_create()` | `zerodds_runtime_destroy()` |
| `zerodds_participant_create()` | `zerodds_participant_destroy()` |
| `zerodds_writer_create()` | `zerodds_writer_destroy()` — vor dem Participant |
| `zerodds_reader_take()` (Buffer) | `zerodds_buffer_free()` |

Memory-Ownership ist explizit dokumentiert in `include/zerodds.h`
und gegen Apex.AI-Plugin-Konformance getestet.

## Spec-Mapping

| Spec-Dokument | Abschnitt |
| --- | --- |
| ZeroDDS C-API 1.0 (vendor spec) | `docs/spec-coverage/zerodds-c-api-1.0.md` |
| OMG DDSI-RTPS 2.5 | §8 — Wire-Format |
| OMG DDS-XCDR2 | komplette CDR-Pipeline (byte-pass-through im FFI) |

## Features

* `default = []` — kein Feature noetig.
* Build-Output: `cdylib` (`libzerodds.so`), `staticlib` (`libzerodds.a`),
  `rlib` (Rust-Konsumenten).

## Stabilitaet

`1.0.0-rc.2`. ABI ist **stabil** ab 1.0.0-final; bis dahin sind
Symbol-Umbenennungen oder Handle-Type-Aenderungen moeglich (jeder
Bruch wird im CHANGELOG markiert).

## Tests

```bash
cargo test -p zerodds-c-api
```

Plus Cross-Vendor-Conformance gegen Cyclone DDS / Fast-DDS in
`crates/discovery/tests/cyclone_*.rs`.

## See also

- [`zerodds-dcps`](../dcps/README.md) — Rust-native DCPS-Runtime (Input-Seite).
- [`zerodds-cpp`](../cpp/README.md) — C++17-RAII-Wrapper auf dieser C-API.
- [`zerodds-cs`](../cs/README.md) — C# P/Invoke-Bindings.
- [`zerodds-ts-node`](../ts-node/README.md) — TypeScript / Node.js via koffi.
- [`zerodds-rmw-zerodds-shim`](../rmw-zerodds-shim/README.md) — ROS-2-RMW-Plugin.
- [`packaging/docker/cpp-runtime/`](../../packaging/docker/cpp-runtime/) — Sandbox-Image mit libzerodds.so + Headern.
