# `zerodds-ffi-loader` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-ffi-loader-1.0.md`

Implementation:

- `crates/zerodds-c-api/` — FFI library loader (C-API bootstrap).

## §1 Conformance levels

### §1 L1-L3 conformance matrix

**Spec:** §1 — three levels (ABI/loader/pub-sub-live-wire); L1 mandatory per
language binding, L2+L3 mandatory per publishable crate.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 ABI surface

### §2.1 Versioning MAJOR/MINOR/PATCH/ABI_REVISION

**Spec:** §2.1 — `ZERODDS_VERSION_MAJOR/MINOR/PATCH` + `ZERODDS_ABI_REVISION`;
`zerodds_abi_revision()` checkable at runtime.

**Repo:** `crates/zerodds-c-api/include/zerodds.h`,
`crates/zerodds-c-api/src/lib.rs`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (abi_revision symbol
check).

**Status:** done

### §2.2 Symbol naming scheme extern "C" + zerodds_*

**Spec:** §2.2 — all symbols `extern "C"`, snake_case, no C++/Rust mangling,
`-fvisibility=hidden`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs,xcdr2.rs}`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs`.

**Status:** done

### §2.3 Header excerpt zerodds.h

**Spec:** §2.3 — a header with runtime/topic/writer/reader/qos opaque types;
result_t/bytes_t/time_t/instance_handle_t/sample_t structs; functions
runtime_create/destroy/qos_*/topic_*/writer_*/reader_*/strerror.

**Repo:** `crates/zerodds-c-api/include/zerodds.h`,
`crates/zerodds-c-api/include/zerodds_xcdr2.h`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_c_codegen.rs`,
`::xcdr2_c_compile` (compile-time check).

**Status:** done

### §2.4 Lifetime rules runtime > topic > writer/reader

**Spec:** §2.4 — lifetime hierarchy; `*_destroy` idempotent + null-safe;
`bytes_t.data` from `_take` valid only until the next call.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,topic_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs}`,
`crates/zerodds-c-api/src/entities.rs`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (destroy sequence).

**Status:** done

### §2.5 Thread safety

**Spec:** §2.5 — all ABI functions thread-safe; listener callbacks on
dedicated threads.

**Repo:** `crates/zerodds-c-api/src/{listener_ffi.rs,entities.rs}`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (Send+Sync implicit).

**Status:** done

### §2.6 Callback API listener

**Spec:** §2.6 — `zerodds_data_available_cb` + `zerodds_reader_set_listener`
with user_data.

**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `listener_ffi.rs`.

**Status:** done

## §3 Per-language loader patterns

### §3.1 Python (ctypes)

**Spec:** §3.1 — a loader with ENV override `ZERODDS_LIB`, a wheel-internal
`_lib/`, a system-linker fallback; `Runtime`/`Writer`/`Reader` classes.

**Repo:** `crates/py/python/`,
`examples/tutorials/dds-chat/ports/python-cli/src/dds_chat_tutorial/{__init__.py,main.py,live_pubsub.py,message.py}`.

**Tests:** `examples/tutorials/dds-chat/ports/python-cli/tests/{test_codec.py,test_fixtures.py,test_live_pubsub.py}`.

**Status:** done

### §3.2 Java (pure-Java)

**Spec:** §3.2 — a pure-Java DDS-Java-PSM without a native library on the
Java side (no `System.loadLibrary`, no JNI adapter). An earlier JNI bridge
was removed.

**Repo:** `crates/java-omgdds/java/src/main/java/`,
`examples/tutorials/dds-chat/ports/java-cli/src/main/java/io/zerodds/chat/Main.java`,
`examples/tutorials/dds-chat/ports/java-cli/pom.xml`.

**Tests:** `crates/java-omgdds/java/src/test/java/.../*Test.java`
(`mvn test`, 18 green);
`examples/tutorials/dds-chat/ports/java-cli/` (Maven test folder).

**Status:** done

### §3.3 C# (DllImport)

**Spec:** §3.3 — `[DllImport("zerodds")]`, a NuGet `runtimes/<rid>/native/`
layout, an IDisposable wrapper.

**Repo:** `crates/cs/csharp/`,
`examples/tutorials/dds-chat/ports/csharp-cli/src/Program.cs`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Tests:** `examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`,
`crates/cs/` tests.

**Status:** done

### §3.4 C++

**Spec:** §3.4 — header `<zerodds/Runtime.hpp>`, RAII wrapper, linker
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
wasm-bindgen in `crates/ts-wasm/`; the WASM build uses the WS bridge as a
daemon.

**Repo:** `crates/ts-node/src/`, `crates/ts-wasm/src/`,
`examples/tutorials/dds-chat/ports/ts-node/src/live_pubsub.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/src/live_pubsub.ts`.

**Tests:** `examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/test/live_pubsub.test.ts`.

**Status:** done

### §3.6 Flutter (dart:ffi)

**Spec:** §3.6 — `DynamicLibrary.open()` per platform; lookup + function
cast.

**Repo:** `examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart`,
`examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/live_pubsub.dart`
(cluster-C dart:ffi loader fully wired).

**Tests:** `examples/tutorials/dds-chat/apps/flutter-mobile/test/{fixtures_test.dart,live_pubsub_test.dart,message_codec_test.dart}`.

**Status:** done

## §4 Per-language convention for the pub/sub sample

### §4.1 Sample lifecycle Open Runtime → write/take → Close

**Spec:** §4.1 — a 6-step lifecycle (Runtime/QoS/Topic/Writer/loop
encode+write/Close).

**Repo:** per-language loaders (see §3.x), `crates/zerodds-c-api/src/`.

**Tests:** cross-language live tests (see §5).

**Status:** done

### §4.2 CDR encoding per language

**Spec:** §4.2 — `crates/py/zerodds/cdr.py`, `crates/java/com/zerodds/cdr/`,
`ZeroDDS.Cdr`, `<zerodds/cdr.hpp>`, `@zerodds/cdr`, `crates/xcdr2-ts/`.

**Repo:** `crates/{xcdr2-c,xcdr2-cpp,xcdr2-csharp,xcdr2-java,xcdr2-rust,xcdr2-ts}/`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` covers wire
identity against the spec; per-language codec tests in the per-language
crates.

**Status:** done

## §5 Cross-language live test

### §5 tests/cross_lang_live/ per-language script + Rust pub subprocess

**Spec:** §5 — `conftest.py` spawns the Rust pub; per-language sub scripts
`test_<lang>_sub.{py,sh}`; a common `shared_topic.idl`.

**Repo:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (Rust-side round-trip),
per-language live pub/sub implementations under
`examples/tutorials/dds-chat/ports/`, `tests/cross_lang_live/` (cluster-C
central cross-language harness).

**Tests:** `tests/cross_lang_live/{live_pubsub_c.sh,live_pubsub_cpp.sh,live_pubsub_csharp.sh,live_pubsub_java.sh,live_pubsub_python.sh,live_pubsub_typescript.sh,run_all.sh}`
(per-language sub scripts with a Rust pub subprocess); per-language
`tests/test_live_pubsub.*` in the dds-chat ports.

**Status:** done

## §6 Packaging

### §6 Library + header + per-language packages

**Spec:** §6 — `libzerodds.{so,dylib,dll}` + `zerodds.h` in system paths;
PyPI/Maven Central/NuGet/npm-ESM/pub.dev.

**Repo:** `crates/zerodds-c-api/Cargo.toml` (cdylib),
`crates/zerodds-c-api/include/zerodds.h`, `packaging/linux/{deb,rpm}/`,
`packaging/macos/{homebrew,pkg}/`,
`packaging/windows/{msi,scoop,chocolatey}/`,
`packaging/linux/rpm/zerodds.pc`.

**Tests:** inline tests per per-language crate check the library lookup;
ABI compat see §8.2.

**Status:** done

## §7 Versioning + ABI compat promise

### §7 Patch/minor/major rules + abi_revision

**Spec:** §7 — patch=no ABI changes, minor=additive symbols, major=breaking
+ ABI_REVISION bump.

**Repo:** `crates/zerodds-c-api/src/lib.rs` (abi_revision function),
`crates/zerodds-c-api/include/zerodds.h` (version macros).

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::*abi_revision*`.

**Status:** done

## §8 Testing

### §8.1 Unit tests per symbol family

**Spec:** §8.1 — runtime/qos/topic/writer/reader/error/thread-safety, ≥ 5
tests each in `crates/zerodds-c-api/tests/`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs}`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs`,
`crates/zerodds-c-api/tests/xcdr2_c_codegen.rs`, `::xcdr2_c_compile`,
`::xcdr2_wire_vectors`.

**Status:** done

### §8.2 ABI compat test abi.snapshot.json

**Spec:** §8.2 — `tests/abi_compat.rs` compares the current symbol list
against `abi.snapshot.json`, fails on unintended changes.

**Repo:** `crates/zerodds-c-api/tests/abi_compat.rs`,
`crates/zerodds-c-api/tests/abi.snapshot.json` (185 symbols baseline).

**Tests:** `cargo test -p zerodds-c-api --test abi_compat` —
`abi_snapshot_matches` + `extract_symbols_finds_known_functions`, 2/2 green.
Regenerate via `ZERODDS_ABI_REGENERATE=1`.

**Status:** done

### §8.3 Per-language loader tests + cross-language live test

**Spec:** §8.3 — per language crate ≥ 5 loader tests + a cross-language live
test in `tests/cross_lang_live/`.

**Repo:** per-language crates `crates/{py,cpp,cs,java,ts-node,ts-wasm}/tests/`,
`examples/tutorials/dds-chat/ports/*/test*/`,
`tests/cross_lang_live/run_all.sh` (cluster-C central cross-language matrix
runner).

**Tests:** per-language `tests/test_live_pubsub.*` in the dds-chat ports +
`tests/cross_lang_live/live_pubsub_*.sh` per-language scripts.

**Status:** done

## §9 Cross-references

### §9 Related crates + wire format + packaging spec

**Spec:** §9 — library `crates/zerodds-c-api/`, per-language bindings, the
pure-Java implementation, wire format, packaging, the listener-callbacks
spec.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §10 Versioning

### §10 SemVer bump rules

**Spec:** §10 — patch=no ABI change, minor=additive symbols + a new language
loader, major=breaking ABI.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

20 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-c-api` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
