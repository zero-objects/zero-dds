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

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::the_header_macro_and_the_exported_revision_agree`,
`crates/zerodds-c-api/tests/abi_compat.rs::abi_snapshot_matches`,
`crates/zerodds-c-api/tests/abi_compat.rs::extract_symbols_finds_known_functions`.

**Status:** done

### §2.2 Symbol-Namensschema extern "C" + zerodds_*

**Spec:** §2.2 — alle Symbole `extern "C"`, snake_case, kein C++/Rust-
Mangling, `-fvisibility=hidden`.

**Repo:** `crates/zerodds-c-api/src/{factory_ffi.rs,participant_ffi.rs,publisher_ffi.rs,subscriber_ffi.rs,topic_ffi.rs,qos_ffi.rs,condition_ffi.rs,listener_ffi.rs,builtin_ffi.rs,extra_ffi.rs,xcdr2.rs}`.

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::extract_symbols_finds_known_functions`,
`crates/zerodds-c-api/tests/abi_compat.rs::abi_snapshot_matches` — die Snapshot-Datei listet jedes
exportierte Symbol namentlich, ein neuer oder umbenannter Export bricht den
Vergleich.

**Status:** done

### §2.3 Header-Auszug zerodds.h

**Spec:** §2.3 — Header mit runtime, topic, writer, reader, qos opaque-
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

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_handles_null_safely`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_take_into_null_safe`,
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_runtime_create_domain_too_large_returns_null`,
`crates/zerodds-c-api/tests/waitset_event_driven.rs::waitset_wait_null_and_timeout_safe`.

**Status:** done

### §2.5 Thread-Safety

**Spec:** §2.5 — alle ABI-Funktionen thread-safe; Listener-Callbacks
auf dedizierten Threads.

**Repo:** —

**Tests:** —

**Status:** open — kein Test ruft die ABI aus mehreren Threads. `Send`/`Sync` stehen
im Typsystem, gemessen ist die Nebenläufigkeit nicht.
### §2.6 Callback-API Listener

**Spec:** §2.6 — `zerodds_data_available_cb` + `zerodds_reader_set_
listener` mit user_data.

**Repo:** `crates/zerodds-c-api/src/listener_ffi.rs`.

**Tests:** `crates/zerodds-c-api/src/listener_ffi.rs::tests::dp_set_get_listener_roundtrip`,
`::poll_listeners_returns_count_and_clears_state`,
`::poll_fires_data_available_and_data_on_readers`,
`::poll_fires_inconsistent_topic_for_topic_and_participant`,
`::dw_set_listener_clear_via_null`.

**Status:** done

## §3 Per-Sprach-Loader-Patterns

### §3.1 Python (ctypes)

**Spec:** §3.1 — Loader mit ENV-Override `ZERODDS_LIB`, Wheel-internes
`_lib/`, System-Linker Fallback; `Runtime`/`Writer`/`Reader`-Klassen.

**Repo:** `crates/py/python/zerodds/loader.py` (`load_library` mit der
Drei-Stufen-Auflösung `ZERODDS_LIB` → paketinternes `_lib/` → System-Linker;
`Runtime`, `Writer`, `Reader`, `DomainParticipantFactory`),
`examples/tutorials/dds-chat/ports/python-cli/src/dds_chat_tutorial/{__init__.py,main.py,live_pubsub.py,message.py}`.

**Tests:** `crates/py/python/tests/test_loader_smoke.py`
(`test_loader_resolves_library` und die drei folgenden; 4 passed gegen
`ZERODDS_LIB=target/release/libzerodds.dylib`),
`examples/tutorials/dds-chat/ports/python-cli/tests/{test_codec.py,test_fixtures.py,test_live_pubsub.py}`.

**Status:** done

### §3.2 Java (Pure-Java)

**Spec:** §3.2 — Pure-Java DDS-Java-PSM ohne Native-Library auf der
Java-Seite (kein `System.loadLibrary`, kein JNI-Adapter). Eine
frühere JNI-Bridge wurde entfernt.

**Repo:** `crates/java-omgdds/java/src/main/java/org/omg/dds/**` (Pure-Java-PSM:
`domain/`, `pub/`, `sub/`, `topic/`, `core/`) und
`crates/java-omgdds/java/src/main/java/org/zerodds/**` (`InProcessBus`,
`cdr/`, `rtps/`, `bridge/GrpcBridgeClient`); `crates/java-omgdds/java/pom.xml`
baut ein portables JAR ohne `.so`/`.dylib`/`.dll`,
`examples/tutorials/dds-chat/ports/java-cli/src/main/java/io/zerodds/chat/Main.java`,
`examples/tutorials/dds-chat/ports/java-cli/pom.xml`.

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::byteArrayQuickstartRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/PubSubLoopbackTest.java::singleWriterSingleReaderRoundTrip`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/Xcdr2CodecTest.java::primitivesRoundTrip`;
Lauf: `mvn -o test` in `crates/java-omgdds/java` — 103 Tests, 1
übersprungen.

**Status:** done

### §3.3 C# (DllImport)

**Spec:** §3.3 — `[DllImport("zerodds")]`, NuGet `runtimes/<rid>/
native/`-Layout, IDisposable-Wrapper.

**Repo:** `crates/cs/csharp/ZeroDDS/src/Native.cs` (77 `[DllImport("zerodds",
EntryPoint = …)]`-Deklarationen mit explizitem EntryPoint, NativeAOT-tauglich)
über `crates/cs/csharp/ZeroDDS/src/{Domain,Topic,Pub,Sub,Qos,Conditions,Listener}.cs`
als IDisposable-Wrapper; `crates/cs/csharp/ZeroDDS/ZeroDDS.csproj` trägt die
NuGet-Metadaten,
`examples/tutorials/dds-chat/ports/csharp-cli/src/Program.cs`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Tests:** `crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V1_EmptyFinal_EncodesToZeroBytes`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V2_PlainPrimitivesFinal_MatchesWire`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V4_StringFinal_MatchesWire`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V10_Mutable_MatchesWire`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V12_MutableSentinel_NoExplicitSentinelEmitted`,
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::FixedBcd_OracleVectors_Roundtrip`;
dazu `crates/cs/csharp/build_cs_smoke.sh`, das
`crates/cs/csharp/ZeroDDS.Tests/Program.cs` gegen die gebaute `libzerodds`
fährt, und `examples/tutorials/dds-chat/ports/csharp-cli/test/FixturesTests.cs`.

**Status:** done

### §3.4 C++

**Spec:** §3.4 — Header `<zerodds/Runtime.hpp>`, RAII-Wrapper, Linker
`-lzerodds`.

**Repo:** `crates/cpp/include/`,
`examples/tutorials/dds-chat/ports/cpp-tui/src/{main.cpp,live_pubsub.cpp}`,
`examples/tutorials/dds-chat/ports/cpp-tui/test/live_pubsub_test.cpp`,
`examples/tutorials/dds-chat/ports/cpp-tui/CMakeLists.txt`.

**Tests:** `crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`, `crates/cpp/tests/smoke_dds_psm.cpp::test_topic_creation`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_publisher_subscriber_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_writer_reader_lifecycle`,
`crates/cpp/tests/smoke_dds_psm.cpp::test_take_on_empty_returns_empty`, `crates/cpp/tests/smoke_dds_psm.cpp::test_status_getters`;
gefahren über `crates/cpp/tests/cpp_smoke_via_cargo.rs::dds_psm_cxx_smoke`.

**Status:** done

### §3.5 TypeScript/JavaScript Node N-API + WASM

**Spec:** §3.5 — Node via koffi-FFI in `crates/ts-node/`; WASM via
wasm-bindgen in `crates/ts-wasm/`; WASM-Build nutzt WS-Bridge als
Daemon.

**Repo:** `crates/ts-node/src/{native.ts,index.ts,dds.ts,qos.ts}`
(koffi-Deklarationen über `zerodds.h`, Library-Auflösung mit
`ZERODDS_LIB`-Übersteuerung),
`crates/ts-wasm/src/lib.rs` (`CdrEncoder`/`CdrDecoder` per wasm-bindgen über
`zerodds-wire`; die frühere Fassung band gegen das im Rebuild entfallene
`zerodds-cdr`) und `crates/ts-wasm/src/{index.ts,cdr/,dcps/}` (DCPS-über-
WebSocket-Fassade zum `crates/websocket-bridge/`-Daemon),
`examples/tutorials/dds-chat/ports/ts-node/src/live_pubsub.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/src/live_pubsub.ts`.

**Tests:** `cargo test -p zerodds-ts-wasm` — 6/6 (`tests::primitive_roundtrip_little_endian`
und fünf weitere); `npm test` in `crates/ts-wasm/` — 4/4 in
`test/smoke.test.mjs` gegen die aus dieser Quelle mit
`npm run build:wasm` erzeugte `pkg-node/` und 4/4 in `test/dcps.test.ts`;
`examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts`,
`examples/tutorials/dds-chat/ports/ts-browser/test/live_pubsub.test.ts`.

**Status:** done

### §3.6 Flutter (dart:ffi)

**Spec:** §3.6 — `DynamicLibrary.open()` per Platform; Lookup +
Function-Cast.

**Repo:** `examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/loader.dart`,
`examples/tutorials/dds-chat/apps/flutter-mobile/lib/native/live_pubsub.dart`
(Cluster-C dart:ffi-Loader voll wired).

**Tests:** `examples/tutorials/dds-chat/apps/flutter-mobile/test/live_pubsub_test.dart` — `"cross-process Pub/Sub round-trip"`;
`examples/tutorials/dds-chat/apps/flutter-mobile/test/message_codec_test.dart` — `"encapsulation header is PL_CDR2_LE"`,
`"message roundtrip"`, `"room field roundtrip"`;
`examples/tutorials/dds-chat/apps/flutter-mobile/test/fixtures_test.dart` — `"01-simple fixture decodes correctly"`,
`"03-unicode fixture decodes correctly"`. Die Dart-Tests tragen Sätze statt
Bezeichner; zitiert wird der Satz, den `flutter test` ausgibt.

**Status:** done

## §4 Per-Sprach-Convention für Pub/Sub-Sample

### §4.1 Sample-Lifecycle Open Runtime → write/take → Close

**Spec:** §4.1 — 6-Schritt-Lifecycle (Runtime/QoS/Topic/Writer/Loop
encode+write/Close).

**Repo:** Per-Sprach-Loader (siehe §3.x), `crates/zerodds-c-api/src/`.

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip` läuft genau die sechs Schritte:
`zerodds_runtime_create`, `zerodds_writer_create`/`zerodds_reader_create`
mit Topic- und Typnamen, fünf `zerodds_writer_write`, `zerodds_reader_take`
mit `zerodds_buffer_free`, dann die `*_destroy`-Kette.
`crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip_via_take_into` (Take in einen vom Aufrufer
gestellten Puffer), `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip_via_loan` (Loan-Pfad).
`tools/traceability/tests/ffi_loader_packaging.rs::the_shipped_header_declares_the_lifecycle_entry_points` prüft, dass
`zerodds.h` diese Einstiegspunkte deklariert. Gate: die drei Roundtrip-
Tests sind auf macOS `#[ignore]` (Multicast-Loopback), auf Linux laufen
sie im Default-Lauf; der Header-Test hat kein Gate.

**Status:** done

### §4.2 CDR-Encoding pro Sprache

**Spec:** §4.2 — der CDR-Codec je Sprache: die Wire-Laufzeit in
`runtime/<Sprache>` mit dem Code, den `zerodds-idlc generate` erzeugt.

**Repo:** Rust `crates/wire/src/{writer.rs,reader.rs}`;
C `runtime/c/include/zerodds_wire.h` + Code aus `zerodds-idlc generate
--lang c`, dazu `<stem>_typesupport.h` mit einer `zerodds_typesupport_t`
je Topic-Typ (`tools/idlc/src/c_support.rs`) für den Typed-Dispatch aus
`crates/zerodds-c-api/include/zerodds_xcdr2.h`, ohne eigenen Codec;
C++ `runtime/cpp/include/zerodds/wire.hpp` + Code aus `zerodds-idlc
generate --lang c++` (`crates/cpp/include/dds/topic/{xcdr2.hpp,
xcdr2_md5.hpp}` entfernt);
C# `runtime/csharp/src/{Writer.cs,Reader.cs}`;
Java `runtime/java/src/main/java/org/zerodds/wire/`;
TS/Node `runtime/typescript/src/`; WASM der wasm-bindgen-Codec
`crates/ts-wasm/src/lib.rs`, selbst über `zerodds-wire` (kein eigener
Codec — das ehemalige Verzeichnis crates/ts-wasm/src/cdr duplizierte ihn und ist
entfernt);
Python `runtime/python/zerodds_wire/`.

**Tests:** `crates/zerodds-c-api/tests/xcdr2_wire_vectors.rs` (13 Fälle:
`v1_empty_final_struct`, `v2_plain_primitives_final`,
`v3_mixed_primitives_final`, `v4_string_final`, `v5_sequence_int32_final`,
`v6_sequence_string_final`, `v7_nested_modules_final`, `v8_keyed_struct_final`,
`v9_appendable_struct`, `v10_mutable_struct`, `v11_optional_member_some`,
`v11_optional_member_none`, `v12_mutable_no_explicit_sentinel`) ist der
Referenz-Satz. Pro Sprache dieselben Vektoren:
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

## §5 Cross-Lang-Live-Test

### §5 tests/cross_lang_live/ Per-Sprach-Skript + Rust-Pub-Subprocess

**Spec:** §5 — `conftest.py` spawnt Rust-Pub; Per-Sprach-Sub-Skripte
`test_<lang>_sub.{py,sh}`; gemeinsame `shared_topic.idl`.

**Repo:** `crates/zerodds-c-api/tests/smoke_ffi.rs` (Rust-Side
Roundtrip), Per-Sprach-Live-Pub/Sub-Implementationen unter
`examples/tutorials/dds-chat/ports/`,
`tests/cross_lang_live/` (Cluster-C zentraler Cross-Lang-Harness).

**Tests:** `crates/zerodds-c-api/tests/smoke_ffi.rs::ffi_pub_sub_roundtrip` ist die
Rust-Seite des Roundtrips; die Per-Sprach-Sub-Skripte
`tests/cross_lang_live/{live_pubsub_c.sh,live_pubsub_cpp.sh,live_pubsub_csharp.sh,live_pubsub_java.sh,live_pubsub_python.sh,live_pubsub_typescript.sh}`
starten sie gegen den jeweiligen Port, `tests/cross_lang_live/run_all.sh`
fährt alle sechs. Die Port-Seite trägt Namen:
`examples/tutorials/dds-chat/ports/python-cli/tests/test_live_pubsub.py::test_pub_sub_round_trip`,
`examples/tutorials/dds-chat/ports/java-cli/src/test/java/io/zerodds/chat/LivePubSubTest.java::pubSubRoundTrip`,
`examples/tutorials/dds-chat/ports/csharp-cli/test/LivePubSubTests.cs::PubSubRoundTrip`,
`examples/tutorials/dds-chat/ports/ts-node/test/live_pubsub.test.ts` — `"cross-process pub/sub round-trip"`.

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

**Tests:** `tools/traceability/tests/ffi_loader_packaging.rs::the_shipped_header_declares_the_lifecycle_entry_points`,
`tools/traceability/tests/ffi_loader_packaging.rs::the_linux_packages_install_the_library_and_resolve_the_prefixes`
(SONAME-Kette `libzerodds.so.1.0.0` → `.so.1` → `.so` plus `zerodds.pc`
mit `includedir`/`libdir`/`-lzerodds`),
`tools/traceability/tests/ffi_loader_packaging.rs::the_macos_packages_install_the_header_and_the_dylib`
(Homebrew-Formula und `.pkg`-Build unter `/usr/local`),
`tools/traceability/tests/ffi_loader_packaging.rs::the_python_wheel_carries_the_distribution_name_of_the_clause`.
Kein Gate; alle vier laufen im Default-Lauf.

**Status:** partial — Library, Header und pkg-config sind für Linux und
macOS in den Paketen und werden gegen die Klausel geprüft; die
PyPI-Distribution heisst wie die Klausel sie nennt. Nicht erfüllt sind der
Windows-Anteil und drei der sechs Per-Sprach-Packages; beides als
§6b geführt.

### §6b Windows-Header/-Library und die fehlenden Per-Sprach-Packages

**Spec:** §6 — `%PROGRAMFILES%\ZeroDDS\include\zerodds.h`,
`bin\zerodds.dll`, `lib\zerodds.lib`; NuGet `ZeroDDS` mit
`runtimes/<rid>/native/`, npm `@zerodds/sdk` und `@zerodds/wasm`,
pub.dev `zerodds`, Maven Central `eu.ifyna:zerodds-java-omgdds`.

**Repo:** —

**Tests:** —

**Status:** open — `packaging/windows/msi/zerodds.wxs` hat das
`Devel`-Feature ausdrücklich entfernt, also liefert kein Windows-Paket
Header, DLL oder Import-Library aus. Von den Per-Sprach-Packages
existiert im Baum nur das PyPI-Wheel unter dem genannten Namen; das
npm-Paket heisst `@zerodds/wire` statt `@zerodds/sdk`/`@zerodds/wasm`,
ein NuGet- und ein pub.dev-Package gibt es nicht, und das Maven-Modul
trägt die Koordinaten `org.zerodds:omgdds` statt
`eu.ifyna:zerodds-java-omgdds`.

## §7 Versioning + ABI-Compat-Promise

### §7 Patch/Minor/Major-Regeln + abi_revision

**Spec:** §7 — Patch=keine ABI-Änderungen, Minor=additive Symbole,
Major=Breaking + ABI_REVISION-Bump.

**Repo:** `crates/zerodds-c-api/src/lib.rs::zerodds_abi_revision`
(exportierte Funktion) + `ZERODDS_ABI_REVISION` (Rust-Konstante);
`crates/zerodds-c-api/include/zerodds.h` (`#define ZERODDS_ABI_REVISION`,
aus dem cbindgen-Preamble in `crates/zerodds-c-api/cbindgen.toml`).

**Tests:** `crates/zerodds-c-api/tests/abi_compat.rs::the_header_macro_and_the_exported_revision_agree`
(Header-Macro, Rust-Konstante und Rückgabewert der Funktion tragen
dieselbe Zahl, und `zerodds_abi_revision` steht in der Symbol-Surface),
`crates/zerodds-c-api/tests/abi_compat.rs::abi_snapshot_matches`
(additiv ja, Entfernen/Umbenennen nein).

**Status:** done

## §8 Testing

### §8.1 Unit-Tests pro Symbol-Familie

**Spec:** §8.1 — runtime, qos, topic, writer, reader, error, thread-safety
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

**Repo:** Per-Sprach-Testbäume `crates/py/python/tests/`,
`crates/cpp/tests/`, `crates/cs/csharp/{ZeroDDS.Tests,ZeroDDS.Cdr.Tests}/`,
`crates/java-omgdds/java/src/test/java/`, `crates/ts-node/test/`,
`crates/ts-wasm/test/`, `examples/tutorials/dds-chat/ports/*/test*/`,
`tests/cross_lang_live/run_all.sh` (Cluster-C zentraler
Cross-Lang-Matrix-Runner).

**Tests:** Per Sprache:
`crates/py/python/tests/test_loader_smoke.py::test_loader_resolves_library`,
`::test_loader_runtime_create_participant_offline`,
`::test_loader_factory_singleton`,
`::test_loader_zerodds_error_is_runtime_error_subclass`;
`crates/cpp/tests/smoke_dds_psm.cpp::test_participant_lifecycle`;
`crates/cs/csharp/ZeroDDS.Cdr.Tests/Xcdr2WireVectorsTests.cs::V1_EmptyFinal_EncodesToZeroBytes`;
`crates/java-omgdds/java/src/test/java/org/omg/dds/QuickstartApiTest.java::serviceEnvironmentBootstrapBySystemProperty`;
`crates/ts-wasm/test/smoke.test.mjs` — `"wasm version string"`.
Der Cross-Lang-Live-Test selbst:
`examples/tutorials/dds-chat/ports/python-cli/tests/test_live_pubsub.py::test_pub_sub_round_trip` und
`examples/tutorials/dds-chat/ports/java-cli/src/test/java/io/zerodds/chat/LivePubSubTest.java::pubSubRoundTrip`,
gefahren über `tests/cross_lang_live/run_all.sh`.

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

18 done / 1 partial / 2 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-c-api` — Tests grün, 0 failed.
Per-Sprach-Seiten: `cargo test -p zerodds-ts-wasm` 6/6;
`npm test` in `crates/ts-wasm/` 8/8; `mvn test` in `crates/java-omgdds/java/`
95 Tests, 0 Failures; `dotnet test` in `crates/cs/csharp/` 31 + 6 grün und
`build_cs_smoke.sh` ohne Fehlschlag; `pytest crates/py/python/tests/test_loader_smoke.py`
4 passed.

Offene Punkte: siehe `zerodds-ffi-loader-1.0.open.md`. Decision-Records: keine.
