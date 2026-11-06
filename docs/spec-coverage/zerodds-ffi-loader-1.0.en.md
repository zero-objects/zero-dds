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

**Status:** n/a (informative) — assignment of the levels; the requirements are the items §2 to §5.

## §2 ABI surface

### §2.1 Versioning MAJOR/MINOR/PATCH/ABI_REVISION

**Spec:** §2.1 — `ZERODDS_VERSION_MAJOR/MINOR/PATCH` + `ZERODDS_ABI_REVISION`;
`zerodds_abi_revision()` checkable at runtime.

**Repo:** `crates/zerodds-c-api/include/zerodds.h`,
`crates/zerodds-c-api/src/lib.rs`.

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::deployment_1_2_the_header_macro_and_the_exported_revision_agree`,
`crates/zerodds-c-api/tests/abi_compat.rs::capi_10_1_abi_snapshot_matches`,
`crates/zerodds-c-api/tests/abi_compat.rs::ffiloader_2_1_extract_symbols_finds_known_functions`.

**Status:** done

### §2.2 Symbol naming scheme extern "C" + zerodds_*

**Spec:** §2.2 — all symbols `extern "C"`, snake_case, no C++/Rust mangling,
`-fvisibility=hidden`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs,xcdr2.rs}`.

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::ffiloader_2_1_extract_symbols_finds_known_functions`,
`crates/zerodds-c-api/tests/abi_compat.rs::capi_10_1_abi_snapshot_matches` — the snapshot file lists every
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

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::capi_11_1_ffi_pub_sub_roundtrip`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::deployment_1_2_ffi_handles_null_safely`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffiloader_2_4_ffi_take_into_null_safe`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffiloader_2_4_ffi_runtime_create_domain_too_large_returns_null`,
`crates/zerodds-c-api/tests/waitset_event_driven.rs::ffiloader_2_4_waitset_wait_null_and_timeout_safe`.

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

**Tests:** `crates/zerodds-c-api/src/listener_ffi.rs::tests::capi_14_5_dp_set_get_listener_roundtrip`,
`::ffiloader_2_6_poll_listeners_returns_count_and_clears_state`,
`::ffiloader_2_6_poll_fires_data_available_and_data_on_readers`,
`::ffiloader_2_6_poll_fires_inconsistent_topic_for_topic_and_participant`,
`::ffiloader_2_6_dw_set_listener_clear_via_null`.

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
(`py_1_3_4_loader_resolves_library` and the three that follow it; 4 passed against
`ZERODDS_LIB=target/release/libzerodds.dylib`),
`examples/tutorials/dds-chat/ports/python-cli/tests/{test_fixtures.py,test_live_pubsub.py}`.

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

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_8_2_a_byte_array_topic_moves_the_octets`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::javapsm_2_a_reader_takes_the_samples_of_a_writer_in_their_order`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::javaomgdds_1_3_xcdr2codec_writes_and_reads_the_primitives_with_the_octets_of_xtypes_7_4`;
run: `mvn -o test` in `crates/java-omgdds/java` — 103 tests, 1 skipped.

**Status:** done

### §3.3 C# (DllImport)

**Spec:** §3.3 — `[DllImport("zerodds")]`, a NuGet `runtimes/<rid>/native/`
layout, an IDisposable wrapper.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Native.cs` (78 `[DllImport("zerodds",
EntryPoint = …)]` declarations with an explicit EntryPoint, NativeAOT-compatible)
under `crates/cs/csharp/ZeroDDS/src/{Domain,Topic,Pub,Sub,Qos,Conditions,Listener}.cs`
as IDisposable wrappers; `crates/cs/csharp/ZeroDDS/ZeroDDS.csproj` carries the
NuGet metadata,
`examples/tutorials/dds-chat/ports/csharp-cli/src/Program.cs`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Tests:** `crates/cs/csharp/ZeroDDS.Tests/BindingTests.cs::ffiloader_3_3_a_participant_a_topic_and_a_reader_are_created_through_dllimport`,
`crates/cs/csharp/ZeroDDS.Tests/BindingTests.cs::ffiloader_3_3_a_byte_sample_is_written_and_taken_through_the_c_api`,
`crates/cs/csharp/ZeroDDS.Tests/BindingTests.cs::ffiloader_3_3_a_typed_sample_is_written_and_taken_with_its_type_support` (xUnit over `libzerodds`, in the job
`merge-cs`), and `examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Status:** done

### §3.4 C++

**Spec:** §3.4 — header `<zerodds/Runtime.hpp>`, RAII wrapper, linker
`-lzerodds`.

**Repo:** `crates/cpp/include/`,
`examples/tutorials/dds-chat/ports/cpp-tui/src/{main.cpp,live_pubsub.cpp}`,
`examples/tutorials/dds-chat/ports/cpp-tui/test/live_pubsub_test.cpp`,
`examples/tutorials/dds-chat/ports/cpp-tui/CMakeLists.txt`.

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_participant_lifecycle`, `crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_writer_reader_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_take_on_empty_returns_empty`, `crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_status_getters`;
driven by `crates/cpp/CMakeLists.txt::dds_psm_cxx_smoke`.

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
`crates/ts-wasm/src/{index.ts,dcps/}` (the DCPS-over-WebSocket facade
onto the `crates/websocket-bridge/` daemon),
`examples/tutorials/dds-chat/ports/ts-node/src/live_pubsub.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/src/live_pubsub.ts`.

**Tests:** `crates/ts-wasm/src/lib.rs::tests::xtypes_7_4_3_2_2_xcdr2_caps_the_alignment_at_four_as_the_vendors_write_it`,
`::xtypes_7_4_3_2_2_xcdr1_aligns_eight_octet_values_to_eight_as_the_vendors_write_it`
(against the vendors' octets in `tests/interop-corpus/expected-bytes.tsv`) and six more,
`crates/ts-wasm/tests/contract_wire.rs::xtypes_7_4_3_2_2_with_max_align_eight_and_four_are_the_two_encoding_versions`
(crate tier); `npm test` in `crates/ts-wasm/` — `test/smoke.test.mjs` against the
`pkg-node/` the wasm-bindgen CLI generates, and `test/dcps.test.ts` — in the job
`integration-ts-wasm` (`tests/integration/ts_wasm.sh`, 7 tests, pipeline 8879);
`examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/test/live_pubsub.test.ts`.

**Status:** done

### §3.6 Flutter (dart:ffi)

**Spec:** §3.6 — `DynamicLibrary.open()` per platform; lookup + function
cast.

**Repo:** `examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart`,
`examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/live_pubsub.dart`
(cluster-C dart:ffi loader fully wired).

**Tests:** `examples/tutorials/dds-chat/apps/flutter-mobile/test/live_pubsub_test.dart` — `"zerodds-ffi-loader 3.6: two Dart processes exchange samples through libzerodds"`
(the loader opens `libzerodds`, looks up the functions of `zerodds.h` and
casts them; a subscriber and a publisher process exchange three samples on a
leased domain). The Dart tests carry sentences rather than identifiers; the
citation is the sentence `flutter test` prints. They run in the weekly job
`integration-flutter` (`tests/integration/flutter_chat.sh`).

**Status:** done

## §4 Per-language convention for the pub/sub sample

### §4.1 Sample lifecycle Open Runtime → write/take → Close

**Spec:** §4.1 — a 6-step lifecycle (Runtime/QoS/Topic/Writer/loop
encode+write/Close).

**Repo:** per-language loaders (see §3.x), `crates/zerodds-c-api/src/` (`zerodds_runtime_create`,
`zerodds_writer_create`, `zerodds_reader_create`, `zerodds_writer_write`, `zerodds_reader_take`,
`zerodds_buffer_free`).

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::capi_11_1_ffi_pub_sub_roundtrip` walks exactly the six steps:
`zerodds_runtime_create`, `zerodds_writer_create`/`zerodds_reader_create`
with topic and type name, five `zerodds_writer_write`,
`zerodds_reader_take` with `zerodds_buffer_free`, then the `*_destroy`
chain. `crates/zerodds-c-api/tests/smoke_ffi.rs::ffiloader_4_1_ffi_pub_sub_roundtrip_via_take_into` (take into a
caller-provided buffer), `crates/zerodds-c-api/tests/smoke_ffi.rs::deployment_1_2_ffi_pub_sub_roundtrip_via_loan` (the loan
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
`ffiloader_4_2_v1_empty_final_struct`, `ffiloader_4_2_v2_plain_primitives_final`,
`ffiloader_4_2_v3_mixed_primitives_final`, `ffiloader_4_2_v4_string_final`, `ffiloader_4_2_v5_sequence_int32_final`,
`ffiloader_4_2_v6_sequence_string_final`, `ffiloader_4_2_v7_nested_modules_final`, `ffiloader_4_2_v8_keyed_struct_final`,
`ffiloader_4_2_v9_appendable_struct`, `ffiloader_4_2_v10_mutable_struct`, `ffiloader_4_2_v11_optional_member_some`,
`ffiloader_4_2_v11_optional_member_none`, `ffiloader_4_2_v12_mutable_no_explicit_sentinel`) is the
reference set. The same vectors per language:
C# in the self-test of the runtime (`runtime/csharp/run-tests.sh`, job
`integration-bindings-dotnet`);
`crates/lang-rules/tests/java_runtime_selftest.rs::the_java_runtime_selftest_passes`;
`crates/ts-node/test/xcdr2-wire-vectors.test.ts` —
`"V-1 Empty Final Struct: encode → 0 bytes; roundtrip"`;
WASM `crates/ts-wasm/src/lib.rs::tests::xtypes_7_4_3_2_2_xcdr2_caps_the_alignment_at_four_as_the_vendors_write_it`
against the vendors' octets in `tests/interop-corpus/expected-bytes.tsv`;
`crates/py/python/tests/test_loader_smoke.py::py_1_3_4_loader_resolves_library`.

**Status:** done

## §5 Cross-language live test

### §5 tests/cross_lang_live/ per-language script + Rust pub subprocess

**Spec:** §5 — `conftest.py` spawns the Rust pub; per-language sub scripts
`test_<lang>_sub.{py,sh}`; a common `shared_topic.idl`; the run on every push
that changes the C API or a binding, and weekly.

**Repo:** `tests/integration/tests/bindings_ffi.rs` (the Rust publisher as the role
`publisher` of the same test program, RELIABLE and KEEP_ALL, XCDR2),
`tests/integration/peers/ffi/` (one subscriber per binding: `tests/integration/peers/ffi/sub.c` through
`zerodds_reader_*`, `tests/integration/peers/ffi/sub.cpp` through `zerodds::Reader` of
`crates/cpp/include/zerodds/dds.hpp`, `tests/integration/peers/ffi/sub.py` through the ctypes loader
`crates/py/python/zerodds/loader.py`, `tests/integration/peers/ffi/sub.mts` through the koffi binding
`crates/ts-node`, `tests/integration/peers/ffi/csharp/Program.cs` through `DllImport` in `crates/cs`),
`tests/integration/ffi_peers.sh` (builds `libzerodds` and the subscribers
before the tests), `ci/integration_scenarios.py` (the entry `merge_beside`
of the job `integration-bindings-ffi`: the merge lane starts it beside the
job of the touched area). The tests are in the integration package, not under
`tests/cross_lang_live/`; there is no shared IDL file, the type
`@final struct FfiSample { unsigned long seq; string text; }` is stated at the
head of the test file.

**Tests:** `tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_c_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_cpp_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_python_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_typescript_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_csharp_subscriber_takes_the_samples_of_a_rust_publisher_in_order`
(job `integration-bindings-ffi`). Per binding the test starts the subscriber
and the Rust publisher as separate processes on a leased domain, waits for
`EVENT matched` from both and compares the 16 bodies the subscriber prints in
hex with the bodies DDS-XTypes 1.3 §7.4.3.5 gives for the fixed samples, which
the test states before the run: count, content and order. The job runs on
every push that changes the C API or a binding (`zerodds-c-api`, `zerodds-cpp`,
`zerodds-py`, `crates/cs`, `crates/ts-node`): the merge lane starts it beside
the job of the touched area (`merge_beside` in `ci/integration_scenarios.py`,
checked by `ci/test_integration_scenarios.py`), and weekly in the integration
pillar. Java (§3.2, a pure-Java implementation of its own without
`libzerodds`), WASM through the WebSocket bridge (§3.5) and Flutter (§3.6) are
the next item.

**Status:** done

### §5 Java, WASM and Flutter against the Rust publisher

**Spec:** §5 — `test_java_sub.sh`, `test_wasm_sub.sh`, `test_flutter_sub.sh`
against the Rust publisher.

**Repo:** `tests/integration/peers/ffi/java/FfiSub.java` (a DataReader of the
Java PSM on the RTPS path, `zerodds.rtps.enable`, built by
`tests/integration/java_psm_peer.sh`), `tests/integration/peers/ffi/sub_wasm.mts`
(the DCPS client of `crates/ts-wasm/src/dcps/` through the daemon
`zerodds-ws-bridged` with the binary frames of zerodds-ws-bridge-1.0 §4.3, a
connection per topic on the path of §4.2/§5.1, and the WebAssembly codec of
`pkg-node/` reading every body), `tests/integration/peers/ffi/sub.dart` (the
dart:ffi loader `examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart`
over `libzerodds`).

**Tests:** `tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_java_subscriber_takes_the_samples_of_a_rust_publisher_in_order` (job `integration-java-psm`),
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_wasm_subscriber_takes_the_samples_of_a_rust_publisher_in_order` (job `integration-ts-wasm`),
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_dart_subscriber_takes_the_samples_of_a_rust_publisher_in_order` (job `integration-flutter`).
The oracle of the item before; the WASM subscriber also prints `seq` and
`text` as the WebAssembly codec reads them from each body, and the test holds
them to the stated values. In the WASM case the Rust publisher starts once the
subscriber's connection is open: the daemon forwards to the connections open
when a sample arrives (VOLATILE). WASM runs in Node without a browser: the
wasm-bindgen glue for Node and the WebSocket client `ws` in place of the
browser's `WebSocket`.

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

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::deployment_1_2_the_header_macro_and_the_exported_revision_agree`
(the header macro, the Rust constant and the function's return value carry
the same number, and `zerodds_abi_revision` is in the symbol surface),
`crates/zerodds-c-api/tests/abi_compat.rs::capi_10_1_abi_snapshot_matches`
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
`capi_10_1_abi_snapshot_matches` + `ffiloader_2_1_extract_symbols_finds_known_functions`, 2/2 green.
Regenerate via `ZERODDS_ABI_REGENERATE=1`.

**Status:** done

### §8.3 Per-language loader tests + cross-language live test

**Spec:** §8.3 — per language crate ≥ 5 loader tests + a cross-language live
test in `tests/cross_lang_live/`.

**Repo:** per-language test trees `crates/py/python/tests/`,
`crates/cpp/tests/`, `crates/cs/csharp/ZeroDDS.Tests/`,
`crates/java-omgdds/java/src/test/java/`, `crates/ts-node/test/`,
`crates/ts-wasm/test/`, `examples/tutorials/dds-chat/ports/*/test*/`.

**Tests:** Per language:
`crates/py/python/tests/test_loader_smoke.py::py_1_3_4_loader_resolves_library`,
`::py_1_3_4_loader_runtime_create_participant_offline`,
`::py_1_2_1_loader_factory_singleton`,
`::py_1_3_4_loader_zerodds_error_is_runtime_error_subclass`;
`crates/cpp/tests/smoke_dds_psm.cpp::psmcxx_2_1_participant_lifecycle`;
`crates/cs/csharp/ZeroDDS.Tests/BindingTests.cs::ffiloader_3_3_a_participant_a_topic_and_a_reader_are_created_through_dllimport`;
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::javapsm_7_3_1_the_service_environment_is_created_from_the_class_named_by_the_system_property`;
`crates/ts-wasm/test/smoke.test.mjs` — `"wasm version string"`.
The live test against a Rust process for C, C++, Python, TypeScript, C#,
Java, WASM and Dart:
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_c_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_cpp_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_python_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_typescript_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_csharp_subscriber_takes_the_samples_of_a_rust_publisher_in_order`
(§5, job `integration-bindings-ffi`),
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_java_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_wasm_subscriber_takes_the_samples_of_a_rust_publisher_in_order`,
`tests/integration/tests/bindings_ffi.rs::ffiloader_5_a_dart_subscriber_takes_the_samples_of_a_rust_publisher_in_order`
(§5, jobs `integration-java-psm`, `integration-ts-wasm`, `integration-flutter`).

**Status:** done

## §9 Cross-references

### §9 Related crates + wire format + packaging spec

**Spec:** §9 — library `crates/zerodds-c-api/`, per-language bindings, the
pure-Java implementation, wire format, packaging, the listener-callbacks
spec.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — list of references.

## §10 Versioning

### §10 SemVer bump rules

**Spec:** §10 — patch=no ABI change, minor=additive symbols + a new language
loader, major=breaking ABI.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — rule for assigning the version number, not a property of the library.

---

## Audit status

19 done / 1 partial / 2 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9157 of `main` (`python3 ci/coverage_results.py --document zerodds-ffi-loader-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-c-api --all-targets` (job `merge-rust-c-api`) — 47 tests green.
* `cargo nextest run --profile merge -p zerodds-traceability --all-targets` (job `merge-rust-traceability`) — 4 tests green.
* `cargo nextest run --profile merge -p zerodds-ts-wasm --all-targets` (job `merge-rust-ts-wasm`) — 3 tests green.
* `python3 ci/run_integration.py merge-codegen-java` (job `merge-codegen-java`) — 1 test green.
* `python3 ci/run_integration.py merge-cs` (job `merge-cs`) — 3 tests green.
* `python3 ci/run_integration.py merge-java-gradle` (job `merge-java-gradle`) — 1 test green.
* `python3 ci/run_integration.py merge-java-omgdds` (job `merge-java-omgdds`) — 4 tests green.
* `python3 ci/run_integration.py integration-bindings-ffi` (job `integration-bindings-ffi`) — 5 tests green.
* `python3 ci/run_integration.py integration-flutter` (job `integration-flutter`) — 2 tests green.
* `python3 ci/run_integration.py integration-java-psm` (job `integration-java-psm`) — 1 test green.
* `python3 ci/run_integration.py integration-python` (job `integration-python`) — 4 tests green.
* `python3 ci/run_integration.py integration-sdk` (job `integration-sdk`) — 7 tests green.
* `python3 ci/run_integration.py integration-ts-wasm` (job `integration-ts-wasm`) — 2 tests green.
* job `ts-node-test` — 1 test green.

85 cited tests green, 0 not green.

Open items: `zerodds-ffi-loader-1.0.open.en.md`.
