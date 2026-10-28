# `zerodds-ffi-loader` v1.0 — ABI stability & per-language loader patterns

ZeroDDS vendor spec. Specifies the ABI surface of the dynamic
library `libzerodds.{so,dylib,dll}` as well as concrete loader patterns
for Python, Java, C#, C++, TypeScript/JS and Flutter.

## Motivation

ZeroDDS is a Rust implementation that, as a native DDS vendor,
has to serve several programming languages. Instead of re-implementing the full
DCPS logic N times, ZeroDDS exposes a stable
C ABI library (`libzerodds.{so,dylib,dll}`) and defines per language
a **thin loader pattern** that binds against this ABI.

This spec prescribes:
1. The ABI stability guarantees (semver at C header level).
2. Which functions are exposed.
3. Per language a canonical loader snippet (Python, Java
   (pure Java; the Java PSM needs no native loader), C# `DllImport`,
   C++ header include, Node N-API + WASM, Dart FFI).
4. How cross-language live tests verify the ABI.

Complementary to the per-language crates `crates/zerodds-c-api/`,
`crates/py/`, `crates/cpp/`, `crates/cs/`, `crates/java/`,
`crates/java-omgdds/`, `crates/java-omgdds/java/`, `crates/ts-node/`,
`crates/ts-wasm/`.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — ABI** | The library exposes the C ABI according to `crates/zerodds-c-api/include/zerodds.h`; semver-stable at header level. |
| **L2 — Loader** | Per-language loader pattern documented + runnable (see §3). |
| **L3 — Pub/Sub** | Per-language live wire test against a Rust peer on the same domain (see §4 + §5). |

L1 is mandatory for every language binding. L2+L3 are mandatory for every
publishable per-language crate.

## §2 ABI surface

### §2.1 Versioning

The C header carries SemVer:
```c
#define ZERODDS_VERSION_MAJOR 1
#define ZERODDS_VERSION_MINOR 0
#define ZERODDS_VERSION_PATCH 0
#define ZERODDS_ABI_REVISION  1
```

Major bump = breaking ABI. Minor = additive (new symbols only
added). Patch = bug fixes without symbol change. `ZERODDS_ABI_REVISION`
changes only on breaking changes, and can be checked at runtime via
`zerodds_abi_revision()`.

### §2.2 Symbol naming scheme

- All exported symbols `extern "C"` with prefix `zerodds_`.
- Snake case.
- No C++ mangling, no Rust mangling (`#[no_mangle]`).
- Visibility: only declared symbols exposed (`-fvisibility=hidden`
  + cargo config `rustflags = ["-C", "link-args=-Wl,--version-script=zerodds.ver"]`).

### §2.3 Header excerpt (`zerodds.h`)

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

The complete header lives in
`crates/zerodds-c-api/include/zerodds.h`.

### §2.4 Lifetime rules

- `zerodds_runtime` must live longer than all topics/writers/readers derived from it.
- `zerodds_topic` must live longer than all writers/readers derived from it.
- `zerodds_qos` may be freed after being passed to `*_create` — ZeroDDS copies internally.
- `zerodds_bytes_t.data` from `zerodds_reader_take` is only valid until the next `_take` call (the caller must copy if it needs it longer).
- All `*_destroy` functions are idempotent + null-safe.

### §2.5 Thread safety

All ABI functions are thread-safe (internal lock strategy). `Send`+
`Sync` of the Rust implementations is guaranteed. Callbacks (listener
API in §2.6) run on dedicated listener threads.

### §2.6 Callback API (additive)

```c
typedef void (*zerodds_data_available_cb)(zerodds_reader *r, void *user_data);

zerodds_result_t zerodds_reader_set_listener(
    zerodds_reader *r,
    zerodds_data_available_cb cb,
    void *user_data);
```

## §3 Per-language loader patterns

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
        # ... analogous ...
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

`zerodds.dll`/`libzerodds.dylib`/`libzerodds.so` via the `runtimes/<rid>/native/`
convention in the NuGet package.

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

Linker flag: `-lzerodds`. Header path in `crates/cpp/include/`.

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

**Browser** (WASM via `crates/ts-wasm/` with `wasm-bindgen`):
```typescript
// crates/ts-wasm/pkg/zerodds.d.ts (auto-generated)
import init, { Runtime, Writer, Reader } from "@zerodds/wasm";

await init();   // load wasm
const rt = new Runtime(0);
```

The WASM build copies no UDP socket — it uses the `crates/websocket-bridge/`
daemon as a bridge (see `zerodds-ws-bridge-1.0`).

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

## §4 Per-language convention for the pub/sub sample (live DDS loop)

A convergence pattern is defined per language:

### §4.1 Sample lifecycle

```
1. Open Runtime (domain_id)
2. Create QoS-Builder (or default)
3. Create Topic(name, type_name, qos)
4. Create Writer(topic, qos)
5. Loop:
   a. Encode app-struct → CDR-bytes (XCDR2-LE)
   b. writer.write(cdr_bytes)
6. Close Writer, Topic, Runtime
```

Subscribe side analogous with `Reader.take()` or a listener callback.

### §4.2 CDR encoding per language

Cross-reference to the per-language bindings:
| Language | CDR codec |
|---------|-----------|
| Python | `runtime/python` (`zerodds_wire`) + code from `zerodds-idlc generate --lang python` |
| Java | `runtime/java` (`org.zerodds.wire`) + code from `--lang java` |
| C# | `runtime/csharp` (`ZeroDds.Wire`) + code from `--lang c#` |
| C++ | `runtime/cpp` (`<zerodds/wire.hpp>`) + code from `zerodds-idlc generate --lang c++` |
| C | `runtime/c` (`zerodds_wire.h`) + code from `zerodds-idlc generate --lang c`; in addition `<stem>_typesupport.h` with one `zerodds_typesupport_t` per topic type for the typed dispatch from `crates/zerodds-c-api/include/zerodds_xcdr2.h`, without a codec of its own — its encoders and decoders call `zerodds_wire.h` |
| TS/Node | `runtime/typescript` (`@zerodds/wire`) + code from `--lang typescript` |
| WASM | the wasm-bindgen codec in `crates/ts-wasm/src/lib.rs`, itself via `zerodds-wire` (`crates/wire`) — no codec of its own (the former `crates/ts-wasm/src/cdr/` duplicated it and has been removed) |
| Flutter | `crates/xcdr2-ts/` generator output compiled to Dart |

The wire format is according to `zerodds-xcdr2-bindings-conformance-1.0`.

## §5 Cross-language live test

`tests/cross_lang_live/` contains one test script per language:

```
tests/cross_lang_live/
├── conftest.py                # spawns the Rust pub as a subprocess
├── test_python_sub.py         # Python reader against the Rust pub
├── test_java_sub.sh           # Java loader against the Rust pub
├── test_csharp_sub.sh         # C# loader against the Rust pub
├── test_cpp_sub.sh            # C++ loader against the Rust pub
├── test_node_sub.sh           # Node N-API against the Rust pub
├── test_wasm_sub.sh           # WASM build via headless Chrome + WS bridge
├── test_flutter_sub.sh        # Flutter test app
└── shared_topic.idl           # shared IDL for all tests
```

Per test:
1. Spawn the Rust pub (`cargo run -p zerodds-c-api --example test_pub`).
2. Spawn the language X sub.
3. Verify N received samples with byte-exact CDR agreement.
4. Clean up both.

CI: GitLab pipeline job `cross-lang-matrix` runs on every push.

## §6 Packaging

According to the `zerodds-deployment-1.0` spec:
- **Library**: `libzerodds.{so,dylib,dll}` shipped as a Cargo cdylib build.
- **Header**: `zerodds.h` in system paths:
  - Linux: `/usr/include/zerodds.h` + `/usr/lib/libzerodds.so` (+ pkg-config: `/usr/lib/pkgconfig/zerodds.pc`)
  - Mac: `/usr/local/include/zerodds.h` + `/usr/local/lib/libzerodds.dylib`
  - Win: `%PROGRAMFILES%\ZeroDDS\include\zerodds.h` + `%PROGRAMFILES%\ZeroDDS\bin\zerodds.dll` + `lib\zerodds.lib`
- **Per-language packages**:
  - Python: PyPI `zerodds` wheel with the library packed in (`_lib/`)
  - Java: Maven Central `eu.ifyna:zerodds-java-omgdds:1.0.0` (pure Java JAR, no native resource)
  - C#: NuGet `ZeroDDS` with `runtimes/<rid>/native/` layout
  - Node: npm `@zerodds/sdk` (with pre-built N-API bindings via `prebuildify`)
  - WASM: npm `@zerodds/wasm`
  - Flutter: `pub.dev` `zerodds` package with native binary pre-builds

## §7 Versioning + ABI compat promise

- Patch version (`1.0.x`): no ABI changes, only bug fixes.
- Minor version (`1.x.0`): only additive symbols, old ones keep working.
- Major version (`x.0.0`): breaking ABI; `ZERODDS_ABI_REVISION` bumped.

Loaders should check `zerodds_abi_revision()` at runtime against the expected
revision and fail fast on a mismatch.

## §8 Testing

### §8.1 Unit tests

Per symbol family ≥ 5 tests in `crates/zerodds-c-api/tests/`:
- runtime_create/destroy
- qos_builder
- topic_lifecycle
- writer_write
- reader_take
- error paths
- thread-safety

### §8.2 ABI compat test

`crates/zerodds-c-api/tests/abi_compat.rs`: compares the current
symbol list against `abi.snapshot.json` and fails on unintended
changes (symbol removed / signature changed).

### §8.3 Per-language loader tests

Per language crate (`crates/py/`, `crates/cpp/`, `crates/cs/`, etc.):
≥ 5 loader tests + cross-language live test in `tests/cross_lang_live/`.

## §9 Cross-references

- Library: `crates/zerodds-c-api/`
- Per-language bindings: `crates/{py,cpp,cs,java,java-omgdds,ts-node,ts-wasm}/`
- Pure Java implementation: `crates/java-omgdds/java/`
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0`.
- Packaging: `zerodds-deployment-1.0`.
- DCPS spec: `zerodds-listener-callbacks-1.1` for listener threads.

## §10 Versioning

`1.0` initial. Patch for bug fixes without ABI change, minor for additive
symbols + new per-language loaders, major for breaking ABI.
