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

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::the_header_macro_and_the_exported_revision_agree`,
`crates/zerodds-c-api/tests/abi_compat.rs::abi_snapshot_matches`,
`crates/zerodds-c-api/tests/abi_compat.rs::extract_symbols_finds_known_functions`.

**Status:** done

### §2.2 Symbol naming scheme extern "C" + zerodds_*

**Spec:** §2.2 — all symbols `extern "C"`, snake_case, no C++/Rust mangling,
`-fvisibility=hidden`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs,xcdr2.rs}`.

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::extract_symbols_finds_known_functions`,
`crates/zerodds-c-api/tests/abi_compat.rs::abi_snapshot_matches` — the snapshot file lists every
exported symbol by name; a new or renamed export breaks the comparison.

**Status:** done

### §2.3 Header excerpt zerodds.h

**Spec:** §2.3 — a header with runtime, topic, writer, reader, qos opaque types;
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

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_handles_null_safely`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_take_into_null_safe`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_runtime_create_domain_too_large_returns_null`,
`crates/zerodds-c-api/tests/waitset_event_driven.rs::waitset_wait_null_and_timeout_safe`.

**Status:** done

### §2.5 Thread safety

**Spec:** §2.5 — all ABI functions thread-safe; listener callbacks on
dedicated threads.

**Repo:** —

**Tests:** —

**Status:** open — no test calls the ABI from more than one thread. `Send`/`Sync` are
in the type system; the concurrency itself is not measured.
### §2.6 Callback API listener

**Spec:** §2.6 — `zerodds_data_available_cb` + `zerodds_reader_set_listener`
with user_data.

**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs`.

**Tests:** `crates/zerodds-c-api/src/listener_ffi.rs::tests::dp_set_get_listener_roundtrip`,
`::poll_listeners_returns_count_and_clears_state`,
`::poll_fires_data_available_and_data_on_readers`,
`::poll_fires_inconsistent_topic_for_topic_and_participant`,
`::dw_set_listener_clear_via_null`.

**Status:** done

## §3 Per-language loader patterns

### §3.1 Python (ctypes)

**Spec:** §3.1 — a loader with ENV override `ZERODDS_LIB`, a wheel-internal
`_lib/`, a system-linker fallback; `Runtime`/`Writer`/`Reader` classes.

**Repo:** `crates/py/python/zerodds/loader.py` (`load_library` with the
three-step resolution `ZERODDS_LIB` → package-internal `_lib/` → system linker;
`Runtime`, `Writer`, `Reader`, `DomainParticipantFactory`),
`examples/tutorials/dds-chat/ports/python-cli/src/dds_chat_tutorial/{__init__.py,main.py,live_pubsub.py,message.py}`.

**Tests:** `crates/py/python/tests/test_loader_smoke.py`
(`test_loader_resolves_library` and the three that follow it; 4 passed against
`ZERODDS_LIB=target/release/libzerodds.dylib`),
`examples/tutorials/dds-chat/ports/python-cli/tests/{test_codec.py,test_fixtures.py,test_live_pubsub.py}`.

**Status:** done

### §3.2 Java (pure-Java)

**Spec:** §3.2 — a pure-Java DDS-Java-PSM without a native library on the
Java side (no `System.loadLibrary`, no JNI adapter). An earlier JNI bridge
was removed.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/**` (the pure-Java
PSM: `domain/`, `pub/`, `sub/`, `topic/`, `core/`) and
`crates/java-omgdds/java/src/main/java/org/zerodds/**`
(`InProcessBus`, `cdr/`, `rtps/`, `bridge/GrpcBridgeClient`);
`crates/java-omgdds/java/pom.xml` builds a portable JAR with no
`.so`/`.dylib`/`.dll`,
`examples/tutorials/dds-chat/ports/java-cli/src/main/java/io/zerodds/chat/Main.java`,
`examples/tutorials/dds-chat/ports/java-cli/pom.xml`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::primitivesRoundTrip`;
run: `mvn -o test` in `crates/java-omgdds/java` — 103 tests, 1 skipped.

**Status:** done

### §3.3 C# (DllImport)

**Spec:** §3.3 — `[DllImport("zerodds")]`, a NuGet `runtimes/<rid>/native/`
layout, an IDisposable wrapper.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Native.cs` (77 `[DllImport("zerodds",
EntryPoint = …)]` declarations with an explicit EntryPoint, NativeAOT-compatible)
under `crates/cs/csharp/ZeroDDS/src/{Domain,Topic,Pub,Sub,Qos,Conditions,Listener}.cs`
as IDisposable wrappers; `crates/cs/csharp/ZeroDDS/ZeroDDS.csproj` carries the
NuGet metadata,
`examples/tutorials/dds-chat/ports/csharp-cli/src/Program.cs`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V1_EmptyFinal_EncodesToZeroBytes`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V2_PlainPrimitivesFinal_MatchesWire`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V4_StringFinal_MatchesWire`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V10_Mutable_MatchesWire`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V12_MutableSentinel_NoExplicitSentinelEmitted`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::FixedBcd_OracleVectors_Roundtrip`;
plus `crates/cs/csharp/build_cs_smoke.sh`, which runs
`crates/cs/csharp/ZeroDDS.Tests/Program.cs` against the built `libzerodds`,
and `examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Status:** done

### §3.4 C++

**Spec:** §3.4 — header `<zerodds/Runtime.hpp>`, RAII wrapper, linker
`-lzerodds`.

**Repo:** `crates/cpp/include/`,
`examples/tutorials/dds-chat/ports/cpp-tui/src/{main.cpp,live_pubsub.cpp}`,
`examples/tutorials/dds-chat/ports/cpp-tui/test/live_pubsub_test.cpp`,
`examples/tutorials/dds-chat/ports/cpp-tui/CMakeLists.txt`.

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`, `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_writer_reader_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_on_empty_returns_empty`, `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`;
driven by `crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke`.

**Status:** done

### §3.5 TypeScript/JavaScript Node N-API + WASM

**Spec:** §3.5 — Node via the koffi FFI in `crates/ts-node/`; WASM via
wasm-bindgen in `crates/ts-wasm/`; the WASM build uses the WS bridge as a
daemon.

**Repo:** `crates/ts-node/src/{native.ts,index.ts,dds.ts,qos.ts}` (koffi
declarations over `zerodds.h`, library resolution with a `ZERODDS_LIB`
override), `crates/ts-wasm/src/lib.rs` (`CdrEncoder`/`CdrDecoder` via
wasm-bindgen over `zerodds-wire`; the earlier form bound against
`zerodds-cdr`, which the rebuild removed) and
`crates/ts-wasm/src/{index.ts,cdr/,dcps/}` (the DCPS-over-WebSocket facade
onto the `crates/websocket-bridge/` daemon),
`examples/tutorials/dds-chat/ports/ts-node/src/live_pubsub.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/src/live_pubsub.ts`.

**Tests:** `cargo test -p zerodds-ts-wasm` — 6/6
(`tests::primitive_roundtrip_little_endian` and five more); `npm test` in
`crates/ts-wasm/` — 4/4 in `test/smoke.test.mjs` against the `pkg-node/`
built from this source with `npm run build:wasm`, and 4/4 in
`test/dcps.test.ts`;
`examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/test/live_pubsub.test.ts`.

**Status:** done

### §3.6 Flutter (dart:ffi)

**Spec:** §3.6 — `DynamicLibrary.open()` per platform; lookup + function
cast.

**Repo:** `examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart`,
`examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/live_pubsub.dart`
(cluster-C dart:ffi loader fully wired).

**Tests:** `examples/tutorials/dds-chat/apps/flutter-mobile/test/live_pubsub_test.dart` — `"cross-process Pub/Sub round-trip"`;
`examples/tutorials/dds-chat/apps/flutter-mobile/test/message_codec_test.dart` — `"encapsulation header is PL_CDR2_LE"`,
`"message roundtrip"`, `"room field roundtrip"`;
`examples/tutorials/dds-chat/apps/flutter-mobile/test/fixtures_test.dart` — `"01-simple fixture decodes correctly"`,
`"03-unicode fixture decodes correctly"`. The Dart tests carry sentences
rather than identifiers; the citation is the sentence `flutter test` prints.

**Status:** done

## §4 Per-language convention for the pub/sub sample

### §4.1 Sample lifecycle Open Runtime → write/take → Close

**Spec:** §4.1 — a 6-step lifecycle (Runtime/QoS/Topic/Writer/loop
encode+write/Close).

**Repo:** per-language loaders (see §3.x), `crates/zerodds-c-api/src/`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip` walks exactly the six steps:
`zerodds_runtime_create`, `zerodds_writer_create`/`zerodds_reader_create`
with topic and type name, five `zerodds_writer_write`,
`zerodds_reader_take` with `zerodds_buffer_free`, then the `*_destroy`
chain. `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip_via_take_into` (take into a
caller-provided buffer), `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip_via_loan` (the loan
path). `tools/traceability/tests/ffi_loader_packaging.rs::the_shipped_header_declares_the_lifecycle_entry_points` checks
that `zerodds.h` declares those entry points. Gate: the three roundtrip
tests are `#[ignore]` on macOS (multicast loopback) and run in the default
run on Linux; the header test has no gate.

**Status:** done

### §4.2 CDR encoding per language

**Spec:** §4.2 — the CDR codec per language: the wire runtime in
`runtime/<language>` with the code `zerodds-idlc generate` emits.

**Repo:** Rust `crates/wire/src/{writer.rs,reader.rs}`;
C `runtime/c/include/zerodds_wire.h` + code from `zerodds-idlc generate
--lang c`, with `<stem>_typesupport.h` holding one
`zerodds_typesupport_t` per topic type (`tools/idlc/src/c_support.rs`)
for the typed dispatch of `crates/zerodds-c-api/include/zerodds_xcdr2.h`,
with no codec of its own; C++ `runtime/cpp/include/zerodds/wire.hpp` + code
from `zerodds-idlc generate --lang c++`
(`crates/cpp/include/dds/topic/{xcdr2.hpp,xcdr2_md5.hpp}` removed);
C# `runtime/csharp/src/{Writer.cs,Reader.cs}`;
Java `runtime/java/src/main/java/org/zerodds/wire/`;
TS/Node `runtime/typescript/src/`; WASM the wasm-bindgen codec
`crates/ts-wasm/src/lib.rs`, itself over `zerodds-wire` (no codec of its
own — the former directory crates/ts-wasm/src/cdr duplicated it and is
removed);
Python `runtime/python/zerodds_wire/`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 cases:
`v1_empty_final_struct`, `v2_plain_primitives_final`,
`v3_mixed_primitives_final`, `v4_string_final`, `v5_sequence_int32_final`,
`v6_sequence_string_final`, `v7_nested_modules_final`, `v8_keyed_struct_final`,
`v9_appendable_struct`, `v10_mutable_struct`, `v11_optional_member_some`,
`v11_optional_member_none`, `v12_mutable_no_explicit_sentinel`) is the
reference set. The same vectors per language:
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V1_EmptyFinal_EncodesToZeroBytes`,
`::V4_StringFinal_MatchesWire`, `::V10_Mutable_MatchesWire`;
`crates/java-omgdds/java/src/test/java/org/zerodds/cdr/Xcdr2WireVectorsTest.java::v1EmptyFinal`,
`::v3MixedPrimitives`, `::v4Greeting`;
`crates/ts-node/test/xcdr2-wire-vectors.test.ts` —
`"V-1 Empty Final Struct: encode → 0 bytes; roundtrip"`;
`crates/ts-wasm/test/smoke.test.mjs` —
`"wasm encode-decode primitive roundtrip (LE)"`;
`crates/py/python/tests/test_loader_smoke.py::test_loader_resolves_library`.

**Status:** done

## §5 Cross-language live test

### §5 tests/cross_lang_live/ per-language script + Rust pub subprocess

**Spec:** §5 — `conftest.py` spawns the Rust pub; per-language sub scripts
`test_<lang>_sub.{py,sh}`; a common `shared_topic.idl`.

**Repo:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (Rust-side round-trip),
per-language live pub/sub implementations under
`examples/tutorials/dds-chat/ports/`, `tests/cross_lang_live/` (cluster-C
central cross-language harness).

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip` is the Rust
side of the roundtrip; the per-language sub-scripts
`tests/cross_lang_live/{live_pubsub_c.sh,live_pubsub_cpp.sh,live_pubsub_csharp.sh,live_pubsub_java.sh,live_pubsub_python.sh,live_pubsub_typescript.sh}`
drive it against each port, and `tests/cross_lang_live/run_all.sh` runs all
six. The port side carries names:
`examples/tutorials/dds-chat/ports/python-cli/tests/test_live_pubsub.py::test_pub_sub_round_trip`,
`examples/tutorials/dds-chat/ports/java-cli/src/test/java/io/zerodds/chat/LivePubSubTest.java::pubSubRoundTrip`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/LivePubSubTests.cs::PubSubRoundTrip`,
`examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts` — `"cross-process pub/sub round-trip"`.

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

**Tests:** `tools/traceability/tests/ffi_loader_packaging.rs::the_shipped_header_declares_the_lifecycle_entry_points`,
`tools/traceability/tests/ffi_loader_packaging.rs::the_linux_packages_install_the_library_and_resolve_the_prefixes`
(the SONAME chain `libzerodds.so.1.0.0` → `.so.1` → `.so` plus
`zerodds.pc` with `includedir`/`libdir`/`-lzerodds`),
`tools/traceability/tests/ffi_loader_packaging.rs::the_macos_packages_install_the_header_and_the_dylib`
(the Homebrew formula and the `.pkg` build under `/usr/local`),
`tools/traceability/tests/ffi_loader_packaging.rs::the_python_wheel_carries_the_distribution_name_of_the_clause`.
No gate; all four run in the default run.

**Status:** partial — library, header and pkg-config are in the Linux and
macOS packages and are checked against the clause; the PyPI distribution
carries the name the clause gives it. The Windows half and three of the
six per-language packages are not met; both are tracked as §6b.

### §6b Windows header/library and the missing per-language packages

**Spec:** §6 — `%PROGRAMFILES%\ZeroDDS\include\zerodds.h`,
`bin\zerodds.dll`, `lib\zerodds.lib`; NuGet `ZeroDDS` with
`runtimes/<rid>/native/`, npm `@zerodds/sdk` and `@zerodds/wasm`,
pub.dev `zerodds`, Maven Central `eu.ifyna:zerodds-java-omgdds`.

**Repo:** —

**Tests:** —

**Status:** open — `packaging/windows/msi/zerodds.wxs` removed the `Devel`
feature explicitly, so no Windows package ships the header, the DLL or the
import library. Of the per-language packages only the PyPI wheel exists in
the tree under the name the clause gives; the npm package is called
`@zerodds/wire` rather than `@zerodds/sdk`/`@zerodds/wasm`, there is no
NuGet and no pub.dev package, and the Maven module carries the coordinates
`org.zerodds:omgdds` instead of `eu.ifyna:zerodds-java-omgdds`.

## §7 Versioning + ABI compat promise

### §7 Patch/minor/major rules + abi_revision

**Spec:** §7 — patch=no ABI changes, minor=additive symbols, major=breaking
+ ABI_REVISION bump.

**Repo:** `crates/zerodds-c-api/src/lib.rs::zerodds_abi_revision` (the
exported function) + `ZERODDS_ABI_REVISION` (the Rust constant);
`crates/zerodds-c-api/include/zerodds.h` (`#define ZERODDS_ABI_REVISION`,
from the cbindgen preamble in `crates/zerodds-c-api/cbindgen.toml`).

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::the_header_macro_and_the_exported_revision_agree`
(the header macro, the Rust constant and the function's return value carry
the same number, and `zerodds_abi_revision` is in the symbol surface),
`crates/zerodds-c-api/tests/abi_compat.rs::abi_snapshot_matches`
(additive yes, removal or rename no).

**Status:** done

## §8 Testing

### §8.1 Unit tests per symbol family

**Spec:** §8.1 — runtime, qos, topic, writer, reader, error, thread-safety, ≥ 5
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

**Repo:** per-language test trees `crates/py/python/tests/`,
`crates/cpp/tests/`, `crates/cs/csharp/{ZeroDDS.Tests,ZeroDDS.Cdr.Tests}/`,
`crates/java-omgdds/java/src/test/java/`, `crates/ts-node/test/`,
`crates/ts-wasm/test/`, `examples/tutorials/dds-chat/ports/*/test*/`,
`tests/cross_lang_live/run_all.sh` (cluster-C central cross-language matrix
runner).

**Tests:** Per language:
`crates/py/python/tests/test_loader_smoke.py::test_loader_resolves_library`,
`::test_loader_runtime_create_participant_offline`,
`::test_loader_factory_singleton`,
`::test_loader_zerodds_error_is_runtime_error_subclass`;
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`;
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V1_EmptyFinal_EncodesToZeroBytes`;
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`;
`crates/ts-wasm/test/smoke.test.mjs` — `"wasm version string"`.
The cross-language live test itself:
`examples/tutorials/dds-chat/ports/python-cli/tests/test_live_pubsub.py::test_pub_sub_round_trip` and
`examples/tutorials/dds-chat/ports/java-cli/src/test/java/io/zerodds/chat/LivePubSubTest.java::pubSubRoundTrip`,
driven by `tests/cross_lang_live/run_all.sh`.

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

18 done / 1 partial / 2 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-c-api` — tests green, 0 failed.
Per-language sides: `cargo test -p zerodds-ts-wasm` 6/6; `npm test` in
`crates/ts-wasm/` 8/8; `mvn test` in `crates/java-omgdds/java/` 95 tests,
0 failures; `dotnet test` in `crates/cs/csharp/` 31 + 6 green and
`build_cs_smoke.sh` with no failure;
`pytest crates/py/python/tests/test_loader_smoke.py` 4 passed.

Open items: see `zerodds-ffi-loader-1.0.open.en.md`. Decision records: none.
