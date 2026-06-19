# `zerodds-ffi-loader` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-ffi-loader-1.0.md`

Implementation:

- `crates/zerodds-c-api/` — FFI-Library-Loader (Bootstrap der C-API).

## §1 Conformance-Levels

### §1 L1-L3 Conformance-Matrix

**Spec:** §1 — drei Levels (ABI/Loader/Pub-Sub-Live-Wire); L1 Pflicht
pro Sprach-Anbindung, L2+L3 Pflicht pro publishable Crate.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 ABI-Surface

### §2.1 Versionierung MAJOR/MINOR/PATCH/ABI_REVISION

**Spec:** §2.1 — `ZERODDS_VERSION_MAJOR/MINOR/PATCH` + `ZERODDS_ABI_
REVISION`; `zerodds_abi_revision()` runtime-checkbar.

**Repo:** `crates/zerodds-c-api/include/zerodds.h`,
`crates/zerodds-c-api/src/lib.rs`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (abi_revision-
Symbol-Check).

**Status:** done

### §2.2 Symbol-Namensschema extern "C" + zerodds_*

**Spec:** §2.2 — alle Symbole `extern "C"`, snake_case, kein C++/Rust-
Mangling, `-fvisibility=hidden`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs,xcdr2.rs}`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs`.

**Status:** done

### §2.3 Header-Auszug zerodds.h

**Spec:** §2.3 — Header mit runtime/topic/writer/reader/qos opaque-
Types; result_t/bytes_t/time_t/instance_handle_t/sample_t Structs;
Funktionen runtime_create/destroy/qos_*/topic_*/writer_*/reader_*/
strerror.

**Repo:** `crates/zerodds-c-api/include/zerodds.h`,
`crates/zerodds-c-api/include/zerodds_xcdr2.h`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_c_codegen.rs`,
`::xcdr2_c_compile` (compile-time-Check).

**Status:** done

### §2.4 Lifetime-Regeln runtime > topic > writer/reader

**Spec:** §2.4 — Lifetime-Hierarchie; `*_destroy` idempotent + null-
safe; `bytes_t.data` aus `_take` nur bis zum nächsten Call gültig.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,topic_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs}`,
`crates/zerodds-c-api/src/entities.rs`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (destroy-Sequence).

**Status:** done

### §2.5 Thread-Safety

**Spec:** §2.5 — alle ABI-Funktionen thread-safe; Listener-Callbacks
auf dedizierten Threads.

**Repo:** `crates/zerodds-c-api/src/{listener_ffi.rs,entities.rs}`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (Send+Sync
implizit).

**Status:** done

### §2.6 Callback-API Listener

**Spec:** §2.6 — `zerodds_data_available_cb` + `zerodds_reader_set_
listener` mit user_data.

**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `listener_ffi.rs`.

**Status:** done

## §3 Per-Sprach-Loader-Patterns

### §3.1 Python (ctypes)

**Spec:** §3.1 — Loader mit ENV-Override `ZERODDS_LIB`, Wheel-internes
`_lib/`, System-Linker Fallback; `Runtime`/`Writer`/`Reader`-Klassen.

**Repo:** `crates/py/python/`,
`examples/tutorials/dds-chat/ports/python-cli/src/dds_chat_tutorial/{__init__.py,main.py,live_pubsub.py,message.py}`.

**Tests:** `examples/tutorials/dds-chat/ports/python-cli/tests/{test_codec.py,test_fixtures.py,test_live_pubsub.py}`.

**Status:** done

### §3.2 Java (Pure-Java)

**Spec:** §3.2 — Pure-Java DDS-Java-PSM ohne Native-Library auf der
Java-Seite (kein `System.loadLibrary`, kein JNI-Adapter). Eine
frühere JNI-Bridge wurde entfernt.

**Repo:** `crates/java-omgdds/java/src/main/java/`,
`examples/tutorials/dds-chat/ports/java-cli/src/main/java/io/zerodds/chat/Main.java`,
`examples/tutorials/dds-chat/ports/java-cli/pom.xml`.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java`
(`mvn test`, 18 grün);
`examples/tutorials/dds-chat/ports/java-cli/` (Maven Test-Folder).

**Status:** done

### §3.3 C# (DllImport)

**Spec:** §3.3 — `[DllImport("zerodds")]`, NuGet `runtimes/<rid>/
native/`-Layout, IDisposable-Wrapper.

**Repo:** `crates/cs/csharp/`,
`examples/tutorials/dds-chat/ports/csharp-cli/src/Program.cs`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Tests:** `examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`,
`crates/cs/`-Tests.

**Status:** done

### §3.4 C++

**Spec:** §3.4 — Header `<zerodds/Runtime.hpp>`, RAII-Wrapper, Linker
`-lzerodds`.

**Repo:** `crates/cpp/include/`,
`examples/tutorials/dds-chat/ports/cpp-tui/src/{main.cpp,live_pubsub.cpp}`,
`examples/tutorials/dds-chat/ports/cpp-tui/test/live_pubsub_test.cpp`,
`examples/tutorials/dds-chat/ports/cpp-tui/CMakeLists.txt`.

**Tests:** `examples/tutorials/dds-chat/ports/cpp-tui/test/live_pubsub_test.cpp`,
`crates/cpp/tests/`.

**Status:** done

### §3.5 TypeScript/JavaScript Node N-API + WASM

**Spec:** §3.5 — Node via napi-rs in `crates/ts-node/`; WASM via
wasm-bindgen in `crates/ts-wasm/`; WASM-Build nutzt WS-Bridge als
Daemon.

**Repo:** `crates/ts-node/src/`,
`crates/ts-wasm/src/`,
`examples/tutorials/dds-chat/ports/ts-node/src/live_pubsub.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/src/live_pubsub.ts`.

**Tests:** `examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/test/live_pubsub.test.ts`.

**Status:** done

### §3.6 Flutter (dart:ffi)

**Spec:** §3.6 — `DynamicLibrary.open()` per Platform; Lookup +
Function-Cast.

**Repo:** `examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart`,
`examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/live_pubsub.dart`
(Cluster-C dart:ffi-Loader voll wired).

**Tests:** `examples/tutorials/dds-chat/apps/flutter-mobile/test/{fixtures_test.dart,live_pubsub_test.dart,message_codec_test.dart}`.

**Status:** done

## §4 Per-Sprach-Convention für Pub/Sub-Sample

### §4.1 Sample-Lifecycle Open Runtime → write/take → Close

**Spec:** §4.1 — 6-Schritt-Lifecycle (Runtime/QoS/Topic/Writer/Loop
encode+write/Close).

**Repo:** Per-Sprach-Loader (siehe §3.x), `crates/zerodds-c-api/src/`.

**Tests:** Cross-Lang-Live-Tests (siehe §5).

**Status:** done

### §4.2 CDR-Encoding pro Sprache

**Spec:** §4.2 — `crates/py/zerodds/cdr.py`, `crates/java/com/zerodds/
cdr/`, `ZeroDDS.Cdr`, `<zerodds/cdr.hpp>`, `@zerodds/cdr`,
`crates/xcdr2-ts/`.

**Repo:** `crates/{xcdr2-c,xcdr2-cpp,xcdr2-csharp,xcdr2-java,xcdr2-rust,xcdr2-ts}/`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` deckt
Wire-Identität gegen Spec; Per-Sprach-Codec-Tests in den Per-Sprach-
Crates.

**Status:** done

## §5 Cross-Lang-Live-Test

### §5 tests/cross_lang_live/ Per-Sprach-Skript + Rust-Pub-Subprocess

**Spec:** §5 — `conftest.py` spawnt Rust-Pub; Per-Sprach-Sub-Skripte
`test_<lang>_sub.{py,sh}`; gemeinsame `shared_topic.idl`.

**Repo:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (Rust-Side
Roundtrip), Per-Sprach-Live-Pub/Sub-Implementationen unter
`examples/tutorials/dds-chat/ports/`,
`tests/cross_lang_live/` (Cluster-C zentraler Cross-Lang-Harness).

**Tests:** `tests/cross_lang_live/{live_pubsub_c.sh,live_pubsub_cpp.sh,live_pubsub_csharp.sh,live_pubsub_java.sh,live_pubsub_python.sh,live_pubsub_typescript.sh,run_all.sh}`
(Per-Sprach-Sub-Skripte mit Rust-Pub-Subprocess);
Per-Sprach-`tests/test_live_pubsub.*` in den dds-chat-Ports.

**Status:** done

## §6 Packaging

### §6 Library + Header + Per-Sprach-Packages

**Spec:** §6 — `libzerodds.{so,dylib,dll}` + `zerodds.h` in System-
Pfaden; PyPI/Maven Central/NuGet/npm-ESM/pub.dev.

**Repo:** `crates/zerodds-c-api/Cargo.toml` (cdylib),
`crates/zerodds-c-api/include/zerodds.h`,
`packaging/linux/{deb,rpm}/`,
`packaging/macos/{homebrew,pkg}/`,
`packaging/windows/{msi,scoop,chocolatey}/`,
`packaging/linux/rpm/zerodds.pc`.

**Tests:** Inline-Tests pro Per-Sprach-Crate prüfen Library-Lookup;
ABI-Compat siehe §8.2.

**Status:** done

## §7 Versioning + ABI-Compat-Promise

### §7 Patch/Minor/Major-Regeln + abi_revision

**Spec:** §7 — Patch=keine ABI-Änderungen, Minor=additive Symbole,
Major=Breaking + ABI_REVISION-Bump.

**Repo:** `crates/zerodds-c-api/src/lib.rs` (abi_revision-Funktion),
`crates/zerodds-c-api/include/zerodds.h` (Versions-Macros).

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::*abi_revision*`.

**Status:** done

## §8 Testing

### §8.1 Unit-Tests pro Symbol-Familie

**Spec:** §8.1 — runtime/qos/topic/writer/reader/error/thread-safety
je ≥ 5 Tests in `crates/zerodds-c-api/tests/`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs}`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs`,
`crates/zerodds-c-api/tests/xcdr2_c_codegen.rs`,
`::xcdr2_c_compile`,
`::xcdr2_wire_vectors`.

**Status:** done

### §8.2 ABI-Compat-Test abi.snapshot.json

**Spec:** §8.2 — `tests/abi_compat.rs` vergleicht aktuelle Symbol-
Liste gegen `abi.snapshot.json`, fail bei unbeabsichtigten Änderungen.

**Repo:** `crates/zerodds-c-api/tests/abi_compat.rs`,
`crates/zerodds-c-api/tests/abi.snapshot.json` (185 Symbole baseline).

**Tests:** `cargo test -p zerodds-c-api --test abi_compat` —
`abi_snapshot_matches` + `extract_symbols_finds_known_functions`,
2/2 grün. Regenerate via `ZERODDS_ABI_REGENERATE=1`.

**Status:** done

### §8.3 Per-Sprach-Loader-Tests + Cross-Lang-Live-Test

**Spec:** §8.3 — pro Sprach-Crate ≥ 5 Loader-Tests + Cross-Lang-Live-
Test in `tests/cross_lang_live/`.

**Repo:** Per-Sprach-Crates `crates/{py,cpp,cs,java,ts-node,ts-wasm}/
tests/`, `examples/tutorials/dds-chat/ports/*/test*/`,
`tests/cross_lang_live/run_all.sh` (Cluster-C zentraler
Cross-Lang-Matrix-Runner).

**Tests:** Per-Sprach-`tests/test_live_pubsub.*` in den dds-chat-
Ports + `tests/cross_lang_live/live_pubsub_*.sh` Per-Sprach-Skripte.

**Status:** done

## §9 Cross-References

### §9 Verwandte Crates + Wire-Format + Packaging-Spec

**Spec:** §9 — Library `crates/zerodds-c-api/`, Per-Sprach-Bindings,
Pure-Java-Implementation, Wire-Format, Packaging, Listener-Callbacks-Spec.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §10 Versioning

### §10 SemVer-Bump-Regeln

**Spec:** §10 — Patch=ohne ABI-Change, Minor=additive Symbole + neuer
Sprach-Loader, Major=Breaking-ABI.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

20 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-c-api` — Tests grün, 0 failed.

Keine offenen Punkte oder Decision-Records — alle Items `done` / `n/a (informative)`.
