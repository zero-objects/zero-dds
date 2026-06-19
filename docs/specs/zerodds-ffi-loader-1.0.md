# `zerodds-ffi-loader` v1.0 — ABI-Stabilität & Per-Sprach-Loader-Patterns

ZeroDDS Vendor-Spec. Spezifiziert die ABI-Surface der dynamischen
Library `libzerodds.{so,dylib,dll}` sowie konkrete Loader-Patterns
für Python, Java, C#, C++, TypeScript/JS und Flutter.

## Motivation

ZeroDDS ist eine Rust-Implementation, die als nativer DDS-Vendor
mehrere Programmiersprachen bedienen muss. Statt N-mal die volle
DCPS-Logik nachzuimplementieren, exponiert ZeroDDS eine stabile
C-ABI-Library (`libzerodds.{so,dylib,dll}`) und definiert pro Sprache
ein **dünnes Loader-Pattern**, das gegen diese ABI bindet.

Diese Spec schreibt vor:
1. Die ABI-Stabilitäts-Garantien (semver auf C-Header-Ebene).
2. Welche Funktionen exposed sind.
3. Pro Sprache einen kanonischen Loader-Snippet (Python, Java
   (Pure-Java; Java-PSM braucht kein Native-Loader), C#-`DllImport`,
   C++-Header-Include, Node-N-API + WASM, Dart-FFI).
4. Wie Cross-Lang-Live-Tests die ABI verifizieren.

Komplementär zu den Per-Sprach-Crates `crates/zerodds-c-api/`,
`crates/py/`, `crates/cpp/`, `crates/cs/`, `crates/java/`,
`crates/java-omgdds/`, `crates/java-omgdds/java/`, `crates/ts-node/`,
`crates/ts-wasm/`.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — ABI** | Library exposed C-ABI gemäß `crates/zerodds-c-api/include/zerodds.h`; semver-stable auf Header-Ebene. |
| **L2 — Loader** | Per-Sprach-Loader-Pattern dokumentiert + lauffähig (siehe §3). |
| **L3 — Pub/Sub** | Per-Sprach-Live-Wire-Test gegen Rust-Peer auf demselben Domain (siehe §4 + §5). |

L1 ist Pflicht für jede Sprach-Anbindung. L2+L3 sind Pflicht für jede
publishable Per-Sprach-Crate.

## §2 ABI-Surface

### §2.1 Versionierung

C-Header trägt SemVer:
```c
#define ZERODDS_VERSION_MAJOR 1
#define ZERODDS_VERSION_MINOR 0
#define ZERODDS_VERSION_PATCH 0
#define ZERODDS_ABI_REVISION  1
```

Major-Bump = breaking ABI. Minor = additive (neue Symbole nur
ergänzt). Patch = Bugfixes ohne Symbol-Änderung. `ZERODDS_ABI_REVISION`
ändert sich nur bei Breaking, ist runtime-checkbar via
`zerodds_abi_revision()`.

### §2.2 Symbol-Namensschema

- Alle exportierten Symbole `extern "C"` mit Prefix `zerodds_`.
- Snake-Case.
- Keine C++-Mangling, keine Rust-Mangling (`#[no_mangle]`).
- Visibility: nur deklarierte Symbole exposed (`-fvisibility=hidden`
  + cargo-config `rustflags = ["-C", "link-args=-Wl,--version-script=zerodds.ver"]`).

### §2.3 Header-Auszug (`zerodds.h`)

```c
#ifndef ZERODDS_H
#define ZERODDS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct zerodds_runtime  zerodds_runtime;
typedef struct zerodds_topic    zerodds_topic;
typedef struct zerodds_writer   zerodds_writer;
typedef struct zerodds_reader   zerodds_reader;
typedef struct zerodds_qos      zerodds_qos;

typedef enum {
    ZERODDS_OK              = 0,
    ZERODDS_ERR_INVALID     = -1,
    ZERODDS_ERR_NOMEM       = -2,
    ZERODDS_ERR_TIMEOUT     = -3,
    ZERODDS_ERR_PRECONDITION = -4,
    ZERODDS_ERR_BUSY        = -5,
    ZERODDS_ERR_NOT_FOUND   = -6,
    ZERODDS_ERR_INTERNAL    = -7
} zerodds_result_t;

typedef struct {
    const uint8_t *data;
    size_t         len;
} zerodds_bytes_t;

typedef struct {
    int64_t sec;
    uint32_t nanosec;
} zerodds_time_t;

typedef struct {
    uint8_t value[16];
} zerodds_instance_handle_t;

uint32_t zerodds_abi_revision(void);
const char* zerodds_version_string(void);

zerodds_result_t zerodds_runtime_create(uint32_t domain_id,
                                        zerodds_runtime **out);
void             zerodds_runtime_destroy(zerodds_runtime *rt);

zerodds_qos*     zerodds_qos_default(void);
void             zerodds_qos_free(zerodds_qos *qos);
zerodds_result_t zerodds_qos_set_reliability(zerodds_qos *qos, int kind);
zerodds_result_t zerodds_qos_set_durability(zerodds_qos *qos, int kind);
zerodds_result_t zerodds_qos_set_history(zerodds_qos *qos, int kind, int depth);
zerodds_result_t zerodds_qos_set_deadline(zerodds_qos *qos, zerodds_time_t period);
zerodds_result_t zerodds_qos_set_partition(zerodds_qos *qos, const char *partition);

zerodds_result_t zerodds_topic_create(zerodds_runtime *rt,
                                      const char *name,
                                      const char *type_name,
                                      const zerodds_qos *qos,
                                      zerodds_topic **out);
void             zerodds_topic_destroy(zerodds_topic *t);

zerodds_result_t zerodds_writer_create(zerodds_runtime *rt,
                                       zerodds_topic *t,
                                       const zerodds_qos *qos,
                                       zerodds_writer **out);
zerodds_result_t zerodds_writer_write(zerodds_writer *w,
                                      const uint8_t *cdr,
                                      size_t cdr_len,
                                      zerodds_instance_handle_t *handle);
zerodds_result_t zerodds_writer_dispose(zerodds_writer *w,
                                        zerodds_instance_handle_t handle);
zerodds_result_t zerodds_writer_wait_for_acks(zerodds_writer *w,
                                              zerodds_time_t timeout);
void             zerodds_writer_destroy(zerodds_writer *w);

typedef struct {
    zerodds_bytes_t            cdr_payload;
    zerodds_instance_handle_t  instance_handle;
    zerodds_time_t             source_timestamp;
    uint32_t                   flags;
    uint8_t                    valid_data;
    uint8_t                    sample_state;
    uint8_t                    view_state;
    uint8_t                    instance_state;
} zerodds_sample_t;

zerodds_result_t zerodds_reader_create(zerodds_runtime *rt,
                                       zerodds_topic *t,
                                       const zerodds_qos *qos,
                                       zerodds_reader **out);
zerodds_result_t zerodds_reader_take(zerodds_reader *r,
                                     zerodds_sample_t *out_samples,
                                     size_t max_samples,
                                     size_t *out_count);
zerodds_result_t zerodds_reader_wait_for_data(zerodds_reader *r,
                                              zerodds_time_t timeout);
void             zerodds_reader_destroy(zerodds_reader *r);

const char*      zerodds_strerror(zerodds_result_t code);

#ifdef __cplusplus
}
#endif
#endif
```

Der vollständige Header lebt in
`crates/zerodds-c-api/include/zerodds.h`.

### §2.4 Lifetime-Regeln

- `zerodds_runtime` muss länger leben als alle daraus abgeleiteten Topics/Writer/Reader.
- `zerodds_topic` muss länger leben als alle daraus abgeleiteten Writer/Reader.
- `zerodds_qos` darf nach dem Übergeben an `*_create` freigegeben werden — ZeroDDS kopiert intern.
- `zerodds_bytes_t.data` aus `zerodds_reader_take` ist nur gültig bis zum nächsten `_take`-Call (Caller muss kopieren wenn länger).
- Alle `*_destroy`-Funktionen sind idempotent + null-safe.

### §2.5 Thread-Safety

Alle ABI-Funktionen sind thread-safe (interne Lock-Strategie). `Send`+
`Sync` der Rust-Implementations ist garantiert. Callbacks (Listener-
API in §2.6) laufen auf dedizierten Listener-Threads.

### §2.6 Callback-API (additive)

```c
typedef void (*zerodds_data_available_cb)(zerodds_reader *r, void *user_data);

zerodds_result_t zerodds_reader_set_listener(
    zerodds_reader *r,
    zerodds_data_available_cb cb,
    void *user_data);
```

## §3 Per-Sprach-Loader-Patterns

### §3.1 Python (ctypes)

```python
# crates/py/zerodds/__init__.py
import ctypes
import os
import sys
from pathlib import Path

def _load_library():
    if sys.platform == "darwin":
        name = "libzerodds.dylib"
    elif sys.platform == "win32":
        name = "zerodds.dll"
    else:
        name = "libzerodds.so"

    # 1) ENV-Override
    if "ZERODDS_LIB" in os.environ:
        return ctypes.CDLL(os.environ["ZERODDS_LIB"])
    # 2) Wheel-internal lib/
    here = Path(__file__).parent
    candidate = here / "_lib" / name
    if candidate.exists():
        return ctypes.CDLL(str(candidate))
    # 3) System-Linker
    return ctypes.CDLL(name)

_lib = _load_library()
_lib.zerodds_abi_revision.restype = ctypes.c_uint32
_lib.zerodds_runtime_create.argtypes = [ctypes.c_uint32, ctypes.POINTER(ctypes.c_void_p)]
_lib.zerodds_runtime_create.restype = ctypes.c_int

class Runtime:
    def __init__(self, domain_id: int = 0):
        ptr = ctypes.c_void_p()
        rc = _lib.zerodds_runtime_create(domain_id, ctypes.byref(ptr))
        if rc != 0:
            raise RuntimeError(f"runtime_create failed: {rc}")
        self._ptr = ptr

    def __del__(self):
        if self._ptr:
            _lib.zerodds_runtime_destroy(self._ptr)
            self._ptr = None

class Writer:
    def __init__(self, rt: Runtime, topic, qos):
        # ... analog ...
        pass
    def write(self, cdr: bytes) -> None:
        buf = (ctypes.c_uint8 * len(cdr)).from_buffer_copy(cdr)
        rc = _lib.zerodds_writer_write(self._ptr, buf, len(cdr), None)
        if rc != 0:
            raise RuntimeError(f"write failed: {rc}")

class Reader:
    def __init__(self, rt: Runtime, topic, qos): ...
    def take(self, max_samples: int = 16) -> list[bytes]: ...
```

### §3.2 Java (Pure-Java, no JNI)

ZeroDDS' Java-PSM (`zerodds-java-omgdds`) is a **Pure-Java**
implementation of OMG DDS-Java-PSM 1.0 — no `System.loadLibrary`,
no native artefact on the Java classpath. Application code uses
the `org.omg.dds.*` API directly:

```java
// crates/java-omgdds/java/src/main/java/...
import org.omg.dds.core.*;
import org.omg.dds.domain.*;
import org.omg.dds.pub.*;
import org.omg.dds.topic.*;

public final class Demo implements AutoCloseable {
    private final DomainParticipant dp;

    public Demo(int domainId) {
        DomainParticipantFactory factory =
            DomainParticipantFactory.getInstance();
        this.dp = factory.createParticipant(domainId);
    }

    @Override
    public void close() {
        dp.close();
    }
}
```

Pure-Java implementation lives in `crates/java-omgdds/java/`;
the build artefact is a portable `.jar` (no `.so` / `.dylib` /
`.dll` shipped). See `docs/specs/zerodds-java-omgdds-1.0.md`.

A previous JNI bridge crate (`crates/zerodds-java-jni/`) was
retired on 2026-05-07 — Java integrations no longer need a Rust
toolchain on the build host.

### §3.3 C#

```csharp
// crates/cs/ZeroDDS/Runtime.cs
using System;
using System.Runtime.InteropServices;

namespace ZeroDDS
{
    public sealed class Runtime : IDisposable
    {
        private IntPtr _handle;

        [DllImport("zerodds", CallingConvention = CallingConvention.Cdecl)]
        private static extern int zerodds_runtime_create(uint domainId, out IntPtr outRt);

        [DllImport("zerodds", CallingConvention = CallingConvention.Cdecl)]
        private static extern void zerodds_runtime_destroy(IntPtr rt);

        public Runtime(uint domainId = 0)
        {
            int rc = zerodds_runtime_create(domainId, out _handle);
            if (rc != 0) throw new InvalidOperationException($"zerodds: {rc}");
        }

        public void Dispose()
        {
            if (_handle != IntPtr.Zero)
            {
                zerodds_runtime_destroy(_handle);
                _handle = IntPtr.Zero;
            }
            GC.SuppressFinalize(this);
        }
    }
}
```

`zerodds.dll`/`libzerodds.dylib`/`libzerodds.so` per `runtimes/<rid>/native/`-
Konvention im NuGet-Package.

### §3.4 C++

```cpp
// crates/cpp/include/zerodds/Runtime.hpp
#pragma once
#include <zerodds.h>
#include <stdexcept>
#include <memory>

namespace zerodds {

class Runtime {
public:
    explicit Runtime(uint32_t domain_id = 0) {
        zerodds_runtime *rt = nullptr;
        auto rc = zerodds_runtime_create(domain_id, &rt);
        if (rc != ZERODDS_OK)
            throw std::runtime_error(zerodds_strerror(rc));
        rt_.reset(rt, zerodds_runtime_destroy);
    }

    zerodds_runtime* raw() const noexcept { return rt_.get(); }

private:
    std::shared_ptr<zerodds_runtime> rt_;
};

} // namespace zerodds
```

Linker-Flag: `-lzerodds`. Header-Pfad in `crates/cpp/include/`.

### §3.5 TypeScript/JavaScript (Node N-API + WASM)

**Node** (N-API via napi-rs in `crates/ts-node/`):
```typescript
// crates/ts-node/src/index.ts
import { createRequire } from "module";
const require = createRequire(import.meta.url);
const native = require("./native") as {
  Runtime: { new(domainId: number): RuntimeNative };
};

export class Runtime {
  private inner: RuntimeNative;
  constructor(domainId = 0) {
    this.inner = new native.Runtime(domainId);
  }
  // ...
}
```

**Browser** (WASM via `crates/ts-wasm/` mit `wasm-bindgen`):
```typescript
// crates/ts-wasm/pkg/zerodds.d.ts (auto-generated)
import init, { Runtime, Writer, Reader } from "@zerodds/wasm";

await init();   // load wasm
const rt = new Runtime(0);
```

WASM-Build kopiert kein UDP-Socket — nutzt `crates/websocket-bridge/`-
Daemon als Bridge (siehe `zerodds-ws-bridge-1.0`).

### §3.6 Flutter (dart:ffi)

```dart
// crates/flutter/lib/zerodds.dart
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

final DynamicLibrary _lib = _open();

DynamicLibrary _open() {
  if (Platform.isMacOS) return DynamicLibrary.open('libzerodds.dylib');
  if (Platform.isWindows) return DynamicLibrary.open('zerodds.dll');
  if (Platform.isAndroid) return DynamicLibrary.open('libzerodds.so');
  if (Platform.isIOS) return DynamicLibrary.process();
  return DynamicLibrary.open('libzerodds.so');
}

typedef _RuntimeCreateNative = Int32 Function(Uint32 domainId, Pointer<Pointer<Void>> outRt);
typedef _RuntimeCreate = int Function(int domainId, Pointer<Pointer<Void>> outRt);

final _runtimeCreate = _lib
    .lookup<NativeFunction<_RuntimeCreateNative>>('zerodds_runtime_create')
    .asFunction<_RuntimeCreate>();

class Runtime {
  late Pointer<Void> _handle;

  Runtime({int domainId = 0}) {
    final out = calloc<Pointer<Void>>();
    final rc = _runtimeCreate(domainId, out);
    if (rc != 0) throw StateError('zerodds: $rc');
    _handle = out.value;
    calloc.free(out);
  }
}
```

## §4 Per-Sprach-Convention für Pub/Sub-Sample (Live-DDS-Loop)

Pro Sprache ist ein Konvergenz-Pattern definiert:

### §4.1 Sample-Lifecycle

```
1. Open Runtime (domain_id)
2. Create QoS-Builder (oder default)
3. Create Topic(name, type_name, qos)
4. Create Writer(topic, qos)
5. Loop:
   a. Encode app-struct → CDR-bytes (XCDR2-LE)
   b. writer.write(cdr_bytes)
6. Close Writer, Topic, Runtime
```

Subscribe-Side analog mit `Reader.take()` oder Listener-Callback.

### §4.2 CDR-Encoding pro Sprache

Cross-Reference zu den Per-Sprach-Bindings:
| Sprache | CDR-Codec |
|---------|-----------|
| Python | `crates/py/zerodds/cdr.py` (pure Python ctypes-friendly) |
| Java | `crates/java/com/zerodds/cdr/` (per `crates/java-omgdds/`) |
| C# | `ZeroDDS.Cdr` Namespace |
| C++ | `<zerodds/cdr.hpp>` (per `crates/xcdr2-cpp/`) |
| TS/Node | `@zerodds/cdr` (npm) |
| WASM | gleicher Code wie Node |
| Flutter | `crates/xcdr2-ts/` Generator-Output kompiliert auf Dart |

Wire-Format ist gemäß `zerodds-xcdr2-bindings-conformance-1.0`.

## §5 Cross-Lang-Live-Test

`tests/cross_lang_live/` enthält ein Test-Skript pro Sprache:

```
tests/cross_lang_live/
├── conftest.py                # spawnt Rust-Pub als Subprocess
├── test_python_sub.py         # Python-Reader gegen Rust-Pub
├── test_java_sub.sh           # Java-Loader gegen Rust-Pub
├── test_csharp_sub.sh         # C#-Loader gegen Rust-Pub
├── test_cpp_sub.sh            # C++-Loader gegen Rust-Pub
├── test_node_sub.sh           # Node-N-API gegen Rust-Pub
├── test_wasm_sub.sh           # WASM-Build via Headless-Chrome + WS-Bridge
├── test_flutter_sub.sh        # Flutter-Test-App
└── shared_topic.idl           # gemeinsame IDL für alle Tests
```

Pro Test:
1. Spawn Rust-Pub (`cargo run -p zerodds-c-api --example test_pub`).
2. Spawn Lang-X-Sub.
3. Verify N empfangene Samples mit byte-genauer CDR-Übereinstimmung.
4. Cleanup beide.

CI: GitLab-Pipeline-Job `cross-lang-matrix` läuft auf jedem Push.

## §6 Packaging

Per `zerodds-deployment-1.0` Spec:
- **Library**: `libzerodds.{so,dylib,dll}` shipped als Cargo-Cdylib-Build.
- **Header**: `zerodds.h` in System-Pfaden:
  - Linux: `/usr/include/zerodds.h` + `/usr/lib/libzerodds.so` (+ pkg-config: `/usr/lib/pkgconfig/zerodds.pc`)
  - Mac: `/usr/local/include/zerodds.h` + `/usr/local/lib/libzerodds.dylib`
  - Win: `%PROGRAMFILES%\ZeroDDS\include\zerodds.h` + `%PROGRAMFILES%\ZeroDDS\bin\zerodds.dll` + `lib\zerodds.lib`
- **Per-Sprach-Packages**:
  - Python: PyPI `zerodds` Wheel mit eingepackter Library (`_lib/`)
  - Java: Maven Central `eu.ifyna:zerodds-java-omgdds:1.0.0` (Pure-Java JAR, kein Native-Resource)
  - C#: NuGet `ZeroDDS` mit `runtimes/<rid>/native/` Layout
  - Node: npm `@zerodds/sdk` (mit pre-built N-API-Bindings via `prebuildify`)
  - WASM: npm `@zerodds/wasm`
  - Flutter: `pub.dev` `zerodds`-Package mit native-bin-pre-builds

## §7 Versioning + ABI-Compat-Promise

- Patch-Version (`1.0.x`): keine ABI-Änderungen, nur Bugfixes.
- Minor-Version (`1.x.0`): nur additive Symbole, alte funktionieren weiter.
- Major-Version (`x.0.0`): Breaking-ABI; `ZERODDS_ABI_REVISION` bumped.

Loader sollten `zerodds_abi_revision()` zur Runtime gegen erwartete
Revision prüfen und bei Mismatch fail-fast.

## §8 Testing

### §8.1 Unit-Tests

Pro Symbol-Familie ≥ 5 Tests in `crates/zerodds-c-api/tests/`:
- runtime_create/destroy
- qos_builder
- topic_lifecycle
- writer_write
- reader_take
- error-Paths
- thread-safety

### §8.2 ABI-Compat-Test

`crates/zerodds-c-api/tests/abi_compat.rs`: vergleicht aktuelle
Symbol-Liste gegen `abi.snapshot.json` und failt bei unbeabsichtigten
Änderungen (Symbol entfernt / Signatur geändert).

### §8.3 Per-Sprach-Loader-Tests

Pro Sprach-Crate (`crates/py/`, `crates/cpp/`, `crates/cs/`, etc.):
≥ 5 Loader-Tests + Cross-Lang-Live-Test in `tests/cross_lang_live/`.

## §9 Cross-References

- Library: `crates/zerodds-c-api/`
- Per-Sprach-Bindings: `crates/{py,cpp,cs,java,java-omgdds,ts-node,ts-wasm}/`
- Pure-Java-Implementation: `crates/java-omgdds/java/`
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0`.
- Packaging: `zerodds-deployment-1.0`.
- DCPS-Spec: `zerodds-listener-callbacks-1.1` für Listener-Threads.

## §10 Versioning

`1.0` initial. Patch für Bugfixes ohne ABI-Change, Minor für additive
Symbole + neue Per-Sprach-Loader, Major für Breaking-ABI.
