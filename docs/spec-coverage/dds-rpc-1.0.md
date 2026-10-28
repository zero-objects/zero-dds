# DDS-RPC 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-rpc-1.0.pdf` (68 Seiten, OMG formal/2017-04-01) —
[OMG DDS-RPC 1.0](https://www.omg.org/spec/DDS-RPC/1.0/)

Audit Item für Item gegen die PDF nach `internal/spec-coverage/PROCESS.md`.
Seitenangaben sind PDF-Seiten (PDF-Seite = gedruckte Seite + 4). Jede Regel der
Abbildung auf Topics und Typen, jede Annotation, jedes Feld der Erweiterungen
der Discovery, jede Regel der Evolution, jeder Schritt der Korrelation und jede
Klasse der Sprachbindungen ist ein eigenes Item. `done` steht nur, wo Code und
Test die Anforderung abdecken; ein zitierter Test nennt die Testfunktion als
`datei::modul::name`. Eine Fundstelle im Repo nennt Datei und Symbol; die
Commits, die eine Anforderung erfüllt haben, stehen mit ihrem 12-stelligen
Hash beim Item. Jede Aussage über Verhalten ist aus dem Code gelesen. Auf dem
Mac wurde nichts ausgeführt (dort wird nur kompiliert). Die Tests liefen auf
einem Linux-Testhost in den Runden 1 bis 10; die Runde 10 war ohne Fehler
(Quelle: Ergebnisse der Testläufe des Maintainers, nicht Teil dieser Datei).

**Kontext:** DDS-RPC ist in ZeroDDS über diese Teile verteilt:

- `crates/rpc/` — Laufzeit in Rust (`Requester`, `Replier`, `wire_codec`),
  Datenmodell der Abbildung (`service_mapping`, `codegen`, `enhanced`),
  Namen der Topics (`topic_naming`), Annotationen (`annotations`), gemeinsame
  Typen (`common_types`), Paar von Typen mit eigenem `header` (`type_pair`,
  `header_pair`), Function-call (`function_call`), Leser für
  Dienstdefinitionen in Java (`java_service_def`), Tabellen der
  Sprachbindungen (`language_binding`), QoS (`qos_profile`), Daten der
  Discovery (`discovery_ext`, `endpoint`), Tabelle der Evolution
  (`evolution_rules`).
- `crates/lang-rules/src/adapter/interface.rs` — Leser eines IDL-`interface`
  für die Generatoren; `crates/lang-rules/src/emit/interface_cpp.rs` mit
  `interface_cpp/`, `interface_java.rs` mit `interface_java/` und
  `interface_rust.rs` mit `interface_rust/` — erzeugter Code für C++, Java und
  Rust (Basic und Enhanced, Function-call und Paar von Typen).
- `runtime/cpp/include/dds/rpc.hpp` — Laufzeit der C++-Bindung;
  `runtime/cpp/include/dds/rpc/zerodds.hpp` — ihre Anbindung an die C-API von
  ZeroDDS.
- `runtime/java/src/main/java/org/omg/dds/rpc/` (die Schnittstellen der Spec) und `runtime/java/src/main/java/org/zerodds/ddsrpc/` (die Implementierungen `RequesterImpl`, `ReplierImpl`, `ServerImpl`) — Laufzeit der Java-Bindung.
  `runtime/java/src/main/java/org/zerodds/ddsrpc/` (`ZeroDdsParticipant`,
  `RawTypeSupport`) bindet sie an die DDS-Java-PSM-Implementierung in
  `crates/java-omgdds/`, einschließlich `PID_RELATED_SAMPLE_IDENTITY`
  (Merge `830064b55c76`). Die Tests der Laufzeit benutzen zusätzlich eine
  Schleife im Speicher (`crates/rpc/tests/java/RpcRuntimeRoundtrip.java`).
- `crates/rtps/` — Parameter-IDs 0x0080 bis 0x0083 und ihre Codecs;
  `crates/dcps/src/runtime.rs` — Senden und Empfangen von
  `PID_RELATED_SAMPLE_IDENTITY` und der Erweiterungen der Discovery.
- `crates/zerodds-c-api/` — Funktionen für RPC: `zerodds_writer_write_related`,
  `zerodds_reader_take_wire`, `rpc_discovery_ffi.rs`, `rpc_profile_ffi.rs`,
  `rpc_qos_ffi.rs`; das Feature `rpc` schaltet `rpc_discovery_ffi.rs` und
  `rpc_profile_ffi.rs`.
- `crates/rpc-rust-e2e/` — erzeugte Rust-Bindung über einer echten
  `DcpsRuntime` auf Loopback.

Die Spec definiert Sprachbindungen für C++ und Java (§7.11). Die Rust-API in
`crates/rpc/` und die erzeugte Rust-Bindung sind keine Bindung der Spec; sie
sind als Fundstelle genannt, wo sie das Verhalten tragen, das eine Regel
verlangt. Wo sich die Laufzeiten für Rust, C++ und Java unterscheiden, sagt das
Item es. Weitere Sprachen der Generatoren (C#, Go, TypeScript u. a.) regelt die
Spec nicht; sie sind nicht als Items geführt. Eine C-Bindung kennt die Spec
nicht; die C-API von ZeroDDS trägt die Funktionen, auf denen die C++-Bindung
aufsetzt (siehe oben), und ist selbst keine Bindung der Spec.

Die Spec nennt drei maschinenlesbare Dateien als normativ
(`omg-dds-rpc-cxx.zip`, `omg-dds-rpc-java.zip`, `omg-dds-rpc-idl.zip`,
S. 1). Sie liegen in `internal/standards/cache/omg/` und sind abgeglichen: die
C++-Header liegen unverändert in `runtime/cpp/include/dds/rpc/normative/` (Item zu
§7.11.1), die Java-Dateien Signatur für Signatur gegen die Java-Laufzeit
(`SpecSignatureCheck`, Item zu §7.11.2), `robot.idl` gegen die Namen der Generatoren
(`crates/lang-rules/tests/rpc_idl_zip_agrees.rs`).

**Offene Punkte:** keine; jedes Item ist `done` oder `n/a (informative)`.

---

## §1 Scope

### §1 Scope

**Spec:** §1, S. 13 — "This specification defines a Remote Procedure Calls
(RPC) framework using the basic building blocks of DDS, such as topics, types,
DataReaders, and DataWriters to provide request/reply semantics. [...] It
supports synchronous and asynchronous method invocation."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Bausteine und die Arten des
Aufrufs sind in §7 einzeln geführt.

---

## §2 Conformance

### §2 Konformitätspunkte Basic und Enhanced

**Spec:** §2, S. 13 — "This specification defines two conformance points:
Basic and Enhanced. [1] Basic conformance (mandatory) [2] Enhanced
conformance (optional)".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; ob ein Punkt erfüllt ist, folgt
aus dem Status der zugeordneten Items.

### §2 Basic conformance — Zuordnung

**Spec:** §2, S. 13 — "The basic conformance point includes support for the
Basic service mapping and both the functional and the request-reply language
binding styles."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; die Basic-Abbildung steht in
§7.4.2, §7.5.1.1, §7.6.1, §7.7.1 und §7.8.1, die beiden Stile in §7.2.2 und
§7.11.

### §2 Enhanced conformance — Zuordnung

**Spec:** §2, S. 13 — "The enhanced conformance point includes the basic
conformance and adds support for the Enhanced Service mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Zuordnung; die Enhanced-Abbildung steht in
§7.4.3, §7.5.1.2, §7.6.2, §7.7.2 und §7.8.2.

---

## §3 Normative References

### §3 Normative References

**Spec:** §3, S. 13–14 — [DDS] 1.2, [RTPS], [DDS-Xtypes] 1.0 Beta 1,
[CORBA] 3.3, [IDL35], [EBNF], [Java-Grammar], [DDS-Cxx-PSM], [DDS-Java-PSM],
[DDS4CCM], [IDL2Java], [SOA-RM].

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Literaturliste; wo eine referenzierte Norm
eine Anforderung trägt (Grammatik von IDL und Java, DDS-Cxx-PSM, DDS-Java-PSM,
DDS4CCM), steht sie beim Item, das sie verwendet.

---

## §4 Terms and Definitions

### §4 Service, Remote Procedure Call

**Spec:** §4, S. 14 — Definitionen von "Service" (nach [SOA-RM]) und "Remote
Procedure Call".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffe.

---

## §5 Symbols and Abbreviated Terms

### §5 Abkürzungen

**Spec:** §5, S. 14 — DDS, GUID, RPC, RTPS, SN.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Abkürzungen.

---

## §6 Additional Information

### §6.1 Acknowledgements

**Spec:** §6.1, S. 15 — einreichende und unterstützende Firmen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Danksagung.

---

## §7.1 Overview

### §7.1 Overview

**Spec:** §7.1, S. 17 — "The intent of this specification is to specify
higher-level abstractions built on top of DDS to achieve first-class
request/reply communication."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Absicht der Spec.

---

## §7.2 General Concepts

### §7.2.1 Client: DataWriter für Requests, DataReader für Replies

**Spec:** §7.2.1, S. 17 — "Structurally, every client uses a data writer for
sending requests and a data reader for receiving replies."

**Repo:** `crates/rpc/src/requester.rs` (`Requester`, Felder `request_writer`
und `reply_reader`, angelegt in `Requester::from_topics`);
`runtime/cpp/include/dds/rpc.hpp` (`Requester`);
`runtime/java/src/main/java/org/omg/dds/rpc/Requester.java` (Interface),
`RequesterImpl.java`

**Tests:** `crates/rpc/tests/runtime_e2e.rs::e2e_single_request_response_roundtrip`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — alle drei Laufzeiten halten je einen Writer für Requests
und einen Reader für Replies. Die Java-Laufzeit als Interfaces: `97c20505e85c`.

### §7.2.1 Service: DataReader für Requests, DataWriter für Replies

**Spec:** §7.2.1, S. 17 — "Symmetrically, every service uses a data reader for
receiving the requests and a data writer for sending the replies."

**Repo:** `crates/rpc/src/replier.rs` (`Replier`, Felder `request_reader` und
`reply_writer`, angelegt in `Replier::from_topics`);
`runtime/cpp/include/dds/rpc.hpp` (`Replier`);
`runtime/java/src/main/java/org/omg/dds/rpc/Replier.java` (Interface),
`ReplierImpl.java`

**Tests:** `crates/rpc/src/replier.rs::tests::tick_processes_request_and_writes_reply`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — alle drei Laufzeiten halten je einen Reader für Requests
und einen Writer für Replies. Die Java-Laufzeit als Interfaces: `97c20505e85c`.

### §7.2.1 Replies anderer Clients werden ausgefiltert

**Spec:** §7.2.1, S. 17 — "To ensure that the client receives a response to a
previous call made by itself, a content-based filter is used by the reader at
the client-side. This ensures that responses for remote invocations of other
clients are filtered."

**Repo:** `crates/rpc/src/requester.rs::Requester::tick` (ein Reply, dessen
`related_request_id` keinem offenen Request entspricht, wird verworfen);
`runtime/cpp/include/dds/rpc.hpp` (`Requester`, `decode_reply` und
`decode_enhanced_reply`: Vergleich der GUID in `related_request_id` mit der
GUID des eigenen Request-Writers `request_writer_guid_`);
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java` (`drain`:
derselbe Vergleich mit `requestWriterGuid`)

**Tests:** `crates/rpc/src/requester.rs::tests::tick_drops_reply_without_matching_request`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Abschnitt "a reply to another client is filtered out" in
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp`),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(derselbe Abschnitt in `crates/rpc/tests/java/RpcRuntimeRoundtrip.java`)

**Status:** `done` — alle drei Laufzeiten reichen keinen Reply an die
Anwendung, der zu einem Request eines anderen Clients gehört. Der Filter
arbeitet im `Requester` nach dem Lesen, nicht als `ContentFilteredTopic` des
DataReaders; die Spec verlangt die Wirkung. C++ und Java: `d6f6b5e975c8`.

### §7.2.1 Filter nach Inhalt in C++ und Java

**Spec:** §7.2.1, S. 17 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Requester`, `request_writer_guid_`);
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java`
(`requestWriterGuid`, `drain`)

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — `Requester::receive_replies` (C++) und `drain` (Java)
verwerfen einen Reply, dessen `related_request_id` die GUID eines anderen
Request-Writers trägt, bevor er an die Anwendung oder an ihre Fehlerbehandlung
geht. Im Enhanced-Pfad gilt derselbe Vergleich für die `related_identity` aus
`PID_RELATED_SAMPLE_IDENTITY`. `d6f6b5e975c8`.

### §7.2.1 Mehrere offene Requests eines Clients

**Spec:** §7.2.1, S. 17 — "It is possible for a client to have more than one
outstanding request [...] As a consequence, each individual request must be
correlated with the corresponding reply."

**Repo:** `crates/rpc/src/requester.rs` (`Requester::pending`, Zuordnung in
`Requester::tick`); `runtime/cpp/include/dds/rpc.hpp` (`Requester::pending_`,
`Requester::receive_reply`); `runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java`
(`receiveReply(SampleIdentity)`)

**Tests:** `crates/rpc/tests/runtime_e2e.rs::e2e_multi_pending_requests_all_get_their_reply`,
`crates/rpc/src/requester.rs::tests::tick_correlates_reply_with_pending_slot`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Abschnitt "two outstanding requests"),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(derselbe Abschnitt)

**Status:** `done` — Zuordnung über die `SampleIdentity` des Requests in allen
drei Laufzeiten; ein Reply wird dem Request zugeordnet, auch wenn die Replies in
anderer Reihenfolge eintreffen.

### §7.2.1 Ein Request ist durch seine SampleIdentity identifiziert

**Spec:** §7.2.1, S. 17 — "Requests, like all samples in the DDS data space,
are identified using a unique SampleIdentity defined as a struct composed of
GUID_t and a SequenceNumber_t defined in sub clause 7.5.1.1.1 Common Types."

**Repo:** `crates/rpc/src/common_types.rs::SampleIdentity`;
`crates/rpc/src/requester.rs::Requester::next_request_id`;
`runtime/cpp/include/dds/rpc.hpp` (`dds::SampleIdentity`);
`runtime/java/src/main/java/org/omg/dds/rpc/SampleIdentity.java`

**Tests:** `crates/rpc/src/requester.rs::tests::send_request_async_assigns_unique_sample_ids`,
`crates/rpc/src/common_types.rs::tests::sample_identity_invalid_constant_is_rtps_unknown`

**Status:** `done` — der Typ besteht aus GUID und `SequenceNumber_t` in allen drei
Laufzeiten, und jeder Request erhält eine eindeutige Identität. Dass diese
Identität die des Samples ist, steht im folgenden Item.

### §7.2.1 Identität des Requests ist die des Samples

**Spec:** §7.2.1, S. 17 — wie oben.

**Repo:** Rust: `crates/rpc/src/requester.rs::Requester::next_request_id`
(GUID des Request-Writers und `DataWriter::next_sequence_number`, aufgerufen
unter `send_lock`); `crates/dcps/src/publisher.rs` (`DataWriter::next_sequence_number`).
Java: `runtime/java/src/main/java/org/zerodds/ddsrpc/ZeroDdsParticipant.java`
(`nextIdentity` fragt `psm.nextIdentity()` des Writers in
`crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java`).
C++: `runtime/cpp/include/dds/rpc/zerodds.hpp` (`Writer::next_identity` fragt
`zerodds_writer_next_sequence_number` in `crates/zerodds-c-api/src/lib.rs`)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::a_live_request_identity_is_the_rtps_identity_of_the_request_writer`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/ZeroDdsParticipantTest.java::aWriteReturnsItsIdentityAndTheReaderSeesItWithTheRelatedOne`,
`crates/zerodds-c-api/tests/writer_next_sequence_number.rs::the_next_sequence_number_is_the_one_the_next_write_carries`

**Status:** `done` — Rust und Java tragen die Identität des Samples im `RequestHeader` (Rust `10ba007046af`, Java über die PSM-Bindung `830064b55c76`). In C++ liefert `Writer::next_identity` die Folgenummer vom Writer über `zerodds_writer_next_sequence_number` (`976400f7d40a`). Das Programm `crates/rpc/tests/cpp/real_library_roundtrip.cpp` lässt die C++-Anbindung (`runtime/cpp/include/dds/rpc/zerodds.hpp`) gegen die echte C-Bibliothek `libzerodds` laufen: zwei Runtimes einer Domain, erzeugter Client und Service der Enhanced-Abbildung, jeder Aufruf wird über die Identität des Samples und `PID_RELATED_SAMPLE_IDENTITY` dem Reply zugeordnet (Test `crates/rpc/tests/cpp_rpc_real_library.rs::the_cpp_binding_runs_against_libzerodds`, Commit `0d2790e2987c`; auf dem Linux-Testhost gelaufen, grün). Der Lauf fand einen Fehler, behoben in `78a70b4c44c8`: `ClientEndpoint::bind` band die Requester, über die der Client sendet, nicht an die Instanz.

### §7.2.1 Der Reply nennt die Identität des Requests

**Spec:** §7.2.1, S. 17 — "When a service implementation sends a reply to a
specific remote invocation, it is necessary to identify the original request by
providing the sample-identity of the request."

**Repo:** `crates/rpc/src/replier.rs::Replier::tick` (`ReplyHeader::new` mit
`header.request_id`; Enhanced: `write_related`);
`runtime/cpp/include/dds/rpc.hpp` (`Replier::send_reply` mit
`related_request_id`);
`runtime/java/src/main/java/org/omg/dds/rpc/Replier.java`
(`sendReply(reply, relatedRequestId)`)

**Tests:** `crates/rpc/src/replier.rs::tests::tick_processes_request_and_writes_reply`,
`crates/rpc/tests/runtime_e2e.rs::e2e_client_side_request_identity_is_the_one_the_reply_relates_to`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Prüfung "related_request_id is the request's identity")

**Status:** `done` — Rust, C++ und Java schreiben die Identität des Requests in
den Reply (Basic: `ReplyHeader::related_request_id`; Enhanced:
`PID_RELATED_SAMPLE_IDENTITY`). C++ und Java: `8202f8c67446`; Enhanced:
`3d4c42cad194`, C++ `a8058d1272c9`.

### §7.2.1 Eigene Identität des Replys

**Spec:** §7.2.1, S. 17 — "Note that a reply data sample has its own unique
message-id (sample identity), which represents the reply message itself and is
independent of the request sample-identity."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis auf eine Eigenschaft jedes
DDS-Samples.

### §7.2.2 Zwei Stile der Sprachbindung

**Spec:** §7.2.2, S. 18 — "This specification includes a higher-level language
binding with function-call style and a lower-level language binding with
request/reply style."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Pflicht steht in §7.2.2.3.

### §7.2.2.1 Function-call Style

**Spec:** §7.2.2.1, S. 18 — "To provide function-call style, a common approach
is to generate stubs that serve as client-side proxies for remote operations
and skeletons to support service-side implementations."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung; die Entitäten stehen in
§7.11.1.5 und §7.11.2.3.

### §7.2.2.2 Request/Reply Style

**Spec:** §7.2.2.2, S. 18 — "The request/reply style provides a flat
interface, such as send_request, receive_request, and send_reply,
receive_reply".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beschreibung; die Operationen stehen in
§7.11.1.4 und §7.11.2.2.

### §7.2.2.3 Vor- und Nachteile der Stile

**Spec:** §7.2.2.3, S. 19 — Vergleich der Stile; "It is possible for a client
to use function-call style language binding to invoke remote operations on a
server that uses request/reply style language binding to implement the
service."

**Repo:** `crates/rpc/src/topic_naming.rs` (`BindingStyle`, `ServiceTopicNames`:
der Stil Request/Reply nimmt den Namen des Interface nicht auf, §7.4.1);
`crates/rpc/src/requester.rs`, `crates/rpc/src/replier.rs`
(`with_instance_style`, `with_topics_style`, `with_options`)

**Tests:** `crates/rpc/src/requester.rs::tests::request_reply_style_drops_the_interface_segment`,
`crates/rpc/tests/language_binding_agreement.rs::both_readings_of_seven_four_produce_the_same_topic_names`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`
(ein `Requester` im Stil Request/Reply ruft den erzeugten Service im Stil
Function-call auf),
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`
(dieselbe Richtung in C++ und Java),
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::a_basic_function_call_client_calls_a_request_reply_service`
(erzeugter Client im Stil Function-call gegen einen `Replier` im Stil
Request/Reply)

**Status:** `done` — die Namen der Topics folgen §7.4.1 für beide Stile
(`575f889c965b`), die Entitäten im Stil Request/Reply sprechen mit dem
erzeugten Service im Stil Function-call, und der erzeugte Client im Stil
Function-call (`CalculatorClient`) ruft einen `Replier` mit
`BindingStyle::RequestReply` auf gleich gesetzten Namen der Topics auf
(`d79e4dac24d0`). Der Test prüft die Namen `demo_Calculator_Service_Request`
und `demo_Calculator_Service_Reply` und zwei Aufrufe von `add`.

### §7.2.2.3 Beide Stile werden unterstützt

**Spec:** §7.2.2.3, S. 19 — "In light of the above observation, a conforming
implementation to this specification shall support both styles of language
binding."

**Repo:** Rust: `crates/rpc/src/requester.rs`, `crates/rpc/src/replier.rs`
(Request/Reply), `crates/lang-rules/src/emit/interface_rust.rs`
(`client_impl`, `dispatcher_impl`; Function-call). C++:
`runtime/cpp/include/dds/rpc.hpp` (`Requester`, `Replier`),
`crates/lang-rules/src/emit/interface_cpp/calls.rs` (`client_class`,
`service_class`). Java: `runtime/java/src/main/java/org/omg/dds/rpc/`
(`RequesterImpl`, `ReplierImpl`, `FunctionCallClient`, `FunctionCallService`),
`crates/lang-rules/src/emit/interface_java/calls.rs` (`client_class`,
`service_class`)

**Tests:** `crates/rpc/tests/runtime_e2e.rs::e2e_single_request_response_roundtrip`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`

**Status:** `done` — beide Stile laufen in Rust, C++ und Java. C++
Function-call `24e3443ce819`, Java Function-call `c4a249bd251f`.

### §7.2.2.3 Function-call in C++ und Java

**Spec:** §7.2.2.3, S. 19 — wie oben.

**Repo:** `crates/lang-rules/src/emit/interface_cpp/calls.rs` (`client_class`,
`service_class`; `<I>Client` leitet von `dds::rpc::ClientEndpoint` ab,
`<I>Service` von `dds::rpc::Service`, das `dds::rpc::Server::add` annimmt);
`runtime/cpp/include/dds/rpc.hpp` (`Service`, `Server`);
`crates/lang-rules/src/emit/interface_java/calls.rs` (`client_class`,
`service_class`: `<I>ClientImpl`, `<I>ServiceImpl`);
`runtime/java/src/main/java/org/omg/dds/rpc/FunctionCallClient.java`,
`FunctionCallService.java`

**Tests:** `crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`

**Status:** `done` — der erzeugte Client ruft über einen `Requester`, der
erzeugte Service antwortet über einen `Replier`; in C++ nimmt `Server::add` den
Service an. C++: `24e3443ce819`, Java: `c4a249bd251f`, Enhanced in C++
`99bda16e455b` und in Java `9ab3b695e13c`.

### §7.2.3 Identität an Requester und Service abrufbar

**Spec:** §7.2.3, S. 19 — "Request-reply correlation requires an ability to
retrieve the sample identity (GUID, SN) at both the requester and service
side."

**Repo:** `crates/rpc/src/requester.rs::Requester::send_request_async` (gibt die
`SampleIdentity` zurück); `crates/rpc/src/replier.rs` (`Replier::receive_requests`
liefert `PendingRequest::identity`; `AsyncReplierHandler` erhält die Identität);
`runtime/cpp/include/dds/rpc.hpp` (`WriteSample::identity`, `Sample::identity`);
`runtime/java/src/main/java/org/omg/dds/rpc/Sample.java` (`identity()`)

**Tests:** `crates/rpc/tests/runtime_e2e.rs::e2e_client_side_request_identity_is_the_one_the_reply_relates_to`,
`crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`,
`crates/rpc/src/replier.rs::tests::dispatch_async_hands_the_replier_and_the_identity_to_the_listener`

**Status:** `done` — die Identität ist am Requester und am Service abrufbar. Der
synchrone `ReplierHandler::handle` der Rust-Laufzeit erhält nur den Wert des
Requests; die Identität liefern `receive_requests` und der asynchrone Handler
(`2933b7d28a88`).

### §7.2.3 Explizite und implizite Weitergabe

**Spec:** §7.2.3, S. 20 — "Explicit sample-identity implies that the request-id
is visible in the top-level data type for the DDS topic. Implicit
sample-identity, on the other hand, implies that the request sample-identity is
communicated via extensibility mechanisms supported by the DDS-RTPS protocol.
Specifically using the inlineQoS sub-message element."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Begriffe; die Pflichten stehen in §7.8.1 und
§7.8.2.

### §7.2.3 APIs für die request-id an Client und Service

**Spec:** §7.2.3, S. 20 — "In both cases, the specification provides APIs to
get the request-id at the client side and get/set the request-id at the service
side."

**Repo:** `crates/rpc/src/requester.rs::Requester::send_request_async`;
`crates/rpc/src/replier.rs` (`Replier::receive_requests`, `Replier::send_reply`
mit `RequestIdentity`); `runtime/cpp/include/dds/rpc.hpp`
(`Replier::receive_requests`, `Replier::send_reply` mit `related_request_id`);
`runtime/java/src/main/java/org/omg/dds/rpc/Replier.java`
(`receiveRequests`, `sendReply(reply, relatedRequestId)`)

**Tests:** `crates/rpc/tests/runtime_e2e.rs::e2e_client_side_request_identity_is_the_one_the_reply_relates_to`,
`crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — Lesen am Client und Lesen und Setzen am Service in Rust,
C++ und Java. Rust am Service: `2933b7d28a88`.

### §7.2.4 Basic: keine Erweiterung von DDS und RTPS, explizite request-id

**Spec:** §7.2.4, S. 20 — "The Basic service mapping enables RPC over DDS
functionality without any extensions to the [DDS] and [DDS-RTPS]
specifications. It uses explicit request-id for correlation."

**Repo:** `crates/rpc/src/wire_codec.rs` (`encode_request_frame`,
`encode_reply_frame`); `crates/rpc/src/requester.rs::Requester::tick`;
`runtime/cpp/include/dds/rpc.hpp` (`RequestHeader::request_id`,
`ReplyHeader::related_request_id`);
`runtime/java/src/main/java/org/omg/dds/rpc/HeaderCodec.java`

**Tests:** `crates/rpc/src/requester.rs::tests::tick_correlates_reply_with_pending_slot`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`,
`crates/rpc/tests/java_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`

**Status:** `done` — alle drei Laufzeiten tragen die request-id im Header des
Samples und brauchen für die Basic-Abbildung keine Erweiterung von DDS oder
RTPS. C++ und Java füllen `requestId` mit der Identität des Writers:
`8202f8c67446`. Die Frage, ob in C++ diese Identität die vom Writer vergebene
ist, steht im Item "Identität des Requests ist die des Samples".

### §7.2.4 Enhanced: Übersicht und Tabelle

**Spec:** §7.2.4, S. 20 — "The Enhanced service mapping uses implicit
request-id for correlation, allows use of the additional data-types defined in
DDS-XTypes, uses DDS-XTypes for type-compatibility checking, and provides more
robust service discovery." Dazu die Tabelle der Merkmale beider Abbildungen.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; jedes Merkmal ist in §7.4.3,
§7.5.1.2, §7.6.2 und §7.8.2 einzeln geführt.

### §7.2.5 Gleiche Abbildung auf beiden Seiten

**Spec:** §7.2.5, S. 20 — "Basic and Enhanced Service Mappings can be mixed in
an application but for any given service both client and the service side must
use the same service mapping."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Vorgabe an die Anwendung. Die Laufzeiten von
ZeroDDS kennen Basic und Enhanced (`RpcMapping::Basic` und `RpcMapping::Enhanced`
in `crates/rpc/src/endpoint_options.rs`); die Wahl trifft die Anwendung je Dienst.

### §7.2.5 Abbildung unabhängig vom Stil

**Spec:** §7.2.5, S. 20 — "The Basic and Enhanced Service mappings are
independent of the language binding style. For instance, it is possible for
clients and service implementations to communicate using different language
binding styles as long as their service mappings match."

**Repo:** `crates/rpc/src/requester.rs`, `crates/rpc/src/replier.rs`,
`runtime/cpp/include/dds/rpc.hpp`,
`runtime/java/src/main/java/org/omg/dds/rpc/` (jede Laufzeit hat Basic und
Enhanced für beide Stile; `RpcMapping` bzw. `ServiceMapping` wählt die
Abbildung, `BindingStyle` den Stil)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::a_basic_function_call_client_calls_a_request_reply_service`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::an_enhanced_function_call_client_calls_a_request_reply_service`

**Status:** `done` — beide Abbildungen laufen mit beiden Stilen. Basic: ein
`Requester` im Stil Request/Reply gegen den erzeugten Service im Stil
Function-call (Tests im Item §7.2.2.3 "Vor- und Nachteile der Stile") und
umgekehrt `a_basic_function_call_client_calls_a_request_reply_service`.
Enhanced: `an_enhanced_function_call_client_calls_a_request_reply_service`
(Client im Stil Function-call, `Replier` im Stil Request/Reply, gleich gesetzte
Namen der Topics `enh_Calc_Service_Request` und `enh_Calc_Service_Reply`;
`d79e4dac24d0`).

---

## §7.3 Service Definition

### §7.3 Zwei Arten der Dienstdefinition

**Spec:** §7.3, S. 21 — "A service definition is provided using the following
two alternatives. a) Interface [...] b) A Pair of Types [...] This
specification uses OMG Interface Definition Language (IDL) or Java 1.5 as a
concrete syntax to define services with either of the mechanisms above."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht; die Arten stehen in §7.3.1 bis
§7.3.2.2 einzeln.

### §7.3.1 Service Definition in IDL

**Spec:** §7.3.1, S. 21 — "This specification uses the interface definition
syntax specified in [IDL]. Additionally, annotations are supported to provide
extra information. [...] this specification provides synchronous and
asynchronous invocation capabilities only at the language binding level."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung; die Produktionen stehen in
§7.3.1.1, die synchronen und asynchronen Aufrufe in §7.11.1.1.2.

### §7.3.1.1 Produktion (1) specification

**Spec:** §7.3.1.1, S. 21 — "(1) <specification> ::= <definition>+"; Design
Rationale, S. 22: "Modified to remove the CORBA-specific import statement."

**Repo:** `crates/idl-parse/src/decl.rs` (Folge von Definitionen;
`parse_import`); `crates/rpc/src/service_mapping.rs::lower_service` (weist
`import` mit `RpcError::OutsideServiceGrammar` ab);
`crates/lang-rules/src/adapter/interface.rs::services`
(`reject_outside_service_grammar` weist `import` mit
`InterfaceError::OutsideServiceGrammar` ab)

**Tests:** `crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`,
`crates/rpc/src/service_mapping.rs::tests::an_import_directive_is_outside_the_service_grammar`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_import_directive`

**Status:** `done` — eine Folge von Definitionen wird gelesen. `lower_service`
(Dienstmodell in Rust) weist `import` ab (`d2a0e370a1ed`), und der Pfad der
Generatoren weist `import` ebenfalls ab (`bfd6f500aa94`). Der Frontend
`crates/idl-parse` liest weiterhin volles IDL 4.2 und nimmt `import` an; die
Abweisung gilt an der Naht zu den Diensten, nicht im Parser.

### §7.3.1.1 Produktion (2) definition

**Spec:** §7.3.1.1, S. 21 — "(2) <definition> ::= <type_dcl> ";"
<ann_appl_post> | <const_dcl> ";" | <except_dcl> ";" | <interface> ";" |
<module> ";" | <value> ";" | <annotation> ";" <ann_appl_post>"; Design
Rationale, S. 22: entfernt `<type_id_dcl>`, `<type_prefix_dcl>`, `<event>`,
`<component>`, `<home_dcl>`; fügt Deklarationen von Annotationen hinzu.

**Repo:** `crates/idl-parse/src/decl.rs` (Definitionen, Deklaration
`@annotation`, nachgestellte Annotationen `//@`);
`crates/rpc/src/service_mapping.rs::lower_service` (weist `typeid`,
`typeprefix` und `component` ab)

**Tests:** `crates/rpc/src/annotations.rs::tests::dds_request_and_reply_topics_lower_and_resolve`,
`crates/idl-parse/tests/parse.rs::a_suffix_annotation_reaches_the_tree_as_an_annotation`,
`crates/rpc/src/service_mapping.rs::tests::a_typeid_directive_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_typeprefix_directive_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_component_declaration_is_outside_the_service_grammar`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_typeid_and_typeprefix_directives`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_component`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_import_directive`

**Status:** done — die verbliebenen Alternativen und Deklarationen von
Annotationen werden gelesen. `lower_service` weist `typeid`, `typeprefix` und
`component` ab (d2a0e370a1ed, 1de24d22343a); dieselbe Prüfung macht
`crates/lang-rules/src/adapter/interface.rs::service` für die Generatoren
(bfd6f500aa94).

### §7.3.1.1 Produktion (7) interface_header mit Annotation

**Spec:** §7.3.1.1, S. 21 — "(7) <interface_header> ::= [ <annotation> ]
“interface” <identifier> [ <interface_inheritance_spec> ]".

**Repo:** `crates/idl-parse/src/decl.rs`; Annotationen am Interface gelesen in
`crates/rpc/src/annotations.rs::lower_rpc_annotations`

**Tests:** `crates/rpc/src/annotations.rs::tests::dds_request_and_reply_topics_lower_and_resolve`,
`crates/rpc/src/service_mapping.rs::tests::every_base_interface_becomes_a_topic_alias`,
`crates/rpc/src/service_mapping.rs::tests::a_local_interface_is_outside_the_service_grammar`

**Status:** done — Annotationen am Interface werden gelesen
(c9f21d8f9a8e); `lower_service` weist andere Formen als `interface`
(`local`, `abstract`, `component`) ab (1de24d22343a).

### §7.3.1.1 Produktion (9) export

**Spec:** §7.3.1.1, S. 22 — "(9) <export> ::= <attr_dcl> “;” | <op_dcl> “;”";
Design Rationale, S. 23: "This production also removes the rules that allow
embedding declarations of types, constants, and exceptions within an
interface."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::service` (Operationen
und Attribute); `crates/rpc/src/service_mapping.rs::lower_service`
(Operationen und Attribute; Konstanten, Typen und Exceptions im Interface
ergeben `RpcError::OutsideServiceGrammar`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::the_seam_reads_every_part_of_an_interface`,
`crates/rpc/src/service_mapping.rs::tests::a_nested_declaration_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_nested_type_declaration_is_outside_the_service_grammar`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_type_in_an_interface_body`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_constant_in_an_interface_body`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_exception_in_an_interface_body`

**Status:** done — in `lower_service` ist ein Member außer Attribut und
Operation ein Fehler (d2a0e370a1ed). `crates/lang-rules/src/adapter/interface.rs::service`
weist eine Konstante, einen Typ und eine Exception im Interface mit
`InterfaceError::OutsideServiceGrammar` ab (bfd6f500aa94).

### §7.3.1.1 Produktion (42) type_dcl mit Annotation

**Spec:** §7.3.1.1, S. 22 — "(42) <type_dcl> ::= [ <annotation> ] “typedef”
<type_declarator> | [ <annotation> ] <struct_type> | <union_type> |
<enum_type> | “native” <simple_declarator> | <constr_forward_decl>".

**Repo:** `crates/idl-parse/src/decl.rs`; `crates/idl-lower/src/annotation.rs`

**Tests:** `crates/idl-parse/tests/parse.rs::annotations_bind_to_the_declaration_that_follows`,
`crates/idl-lower/tests/annotations.rs::an_alias_supplies_a_default_that_the_use_site_overrides`

**Status:** done — Annotationen an Typdeklarationen werden gelesen
(b0a9331d9221).

### §7.3.1.1 Produktion (87) op_dcl

**Spec:** §7.3.1.1, S. 22 — "(87) <op_dcl> ::= [ <annotation> ] <op_type_spec>
<identifier> <parameter_dcls> [ <raises_expr> ]"; Design Rationale, S. 23:
"Removed the CORBA-specific “oneway” modifier and the “context” expressions."

**Repo:** `crates/idl-parse/src/decl.rs`; `crates/rpc/src/service_mapping.rs::lower_method`
(weist `oneway`, `@oneway` und `context` ab)

**Tests:** `crates/rpc/src/service_mapping.rs::tests::the_oneway_keyword_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::the_oneway_annotation_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_context_clause_is_outside_the_service_grammar`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_oneway_operation`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_the_oneway_annotation`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_context_clause`

**Status:** done — `lower_service` weist das Wort `oneway`, `@oneway` und
eine `context`-Klausel mit `RpcError::OutsideServiceGrammar` ab
(d2a0e370a1ed, 980f41ccd7aa). Der Leser der Generatoren
(`crates/lang-rules/src/adapter/interface.rs::one_operation`) weist sie mit
`InterfaceError::OutsideServiceGrammar` ab (bfd6f500aa94); die Quellen der Tests
der Generatoren und von `crates/rpc-rust-e2e` enthalten kein `oneway` mehr
(732364ad3635). Das Feld `Operation::oneway` und die Zweige für `oneway` in
`emit/interface_{cpp,java,rust,typescript}.rs` und in `crates/webdds-bridge` sind entfernt (`fb0f35afc27`). Die Design Rationale der Produktion nennt die Entfernung („Removed the CORBA-specific “oneway” modifier and the “context” expressions.“); §7.3.2 behält `@oneway` nur für Java-Quellen (`java_service_def`). Das IDL-`@oneway` wird ebenfalls abgewiesen: die Zeile der Rationale betrifft die IDL-Grammatik, und eine Annotation nur für Java hat keine Abbildung in den Topic-Typen.

### §7.3.1.1 Produktion (91) param_dcl mit Annotation

**Spec:** §7.3.1.1, S. 22 — "(91) <param_dcl> ::= [ <annotation> ]
<param_attribute> <param_type_spec> <simple_declarator>".

**Repo:** `crates/rpc/src/service_mapping.rs::lower_method` (Annotationen am
Parameter, `override_direction`)

**Tests:** `crates/rpc/src/service_mapping.rs::tests::param_annotation_in_overrides_declared_direction`,
`crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`

**Status:** done — Annotationen am Parameter ändern die Richtung
(0ecf76d597c0).

### §7.3.1.1 Produktion (104) readonly_attr_spec mit Annotation

**Spec:** §7.3.1.1, S. 22 — "(104) <readonly_attr_spec> ::= [ <annotation> ]
“readonly” “attribute” <param_type_spec> <readonly_attr_declarator>".

**Repo:** `crates/idl-parse/src/decl.rs`

**Tests:** `crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`
(`@key readonly attribute long version;` in `dds_xtypes.idl`)

**Status:** done — der Frontend liest die Annotation vor `readonly attribute`
(784e1e121a5f).

### §7.3.1.1 Produktion (106) attr_spec mit Annotation

**Spec:** §7.3.1.1, S. 22 — "(106) <attr_spec> ::= [ <annotation> ]
“attribute” <param_type_spec> <attr_declarator>".

**Repo:** `crates/idl-parse/src/decl.rs`

**Tests:** `crates/idl-parse/tests/parse.rs::annotations_bind_to_the_declaration_that_follows`,
`crates/idl-parse/tests/parse.rs::an_annotation_precedes_a_writable_attribute`

**Status:** done — der Frontend liest Annotationen vor Deklarationen
allgemein; der Test mit `@id(1) attribute long x;` (ohne `readonly`) zeigt die
Annotation als erstes Kind des Knotens `AttributeDecl` (5c45d169f4d1).

### §7.3.1.1 Produktion (106) attr_spec: Test mit schreibbarem Attribut

**Spec:** §7.3.1.1, S. 22 — wie oben.

**Repo:** `crates/idl-parse/src/decl.rs`

**Tests:** `crates/idl-parse/tests/parse.rs::an_annotation_precedes_a_writable_attribute`

**Status:** done — `an_annotation_precedes_a_writable_attribute` parst
`interface I { @id(1) attribute long x; };` ohne Diagnostik und prüft, dass der
Knoten kein `readonly` enthält (5c45d169f4d1).

### §7.3.1.1 — Konstrukte außerhalb der Grammatik der Abbildung

**Spec:** §7.3.1.1, S. 21–23 — "The IDL 3.5 grammar shall be modified with the
productions shown below." Die Design Rationale nennt, was entfällt: `import`,
`<type_id_dcl>`, `<type_prefix_dcl>`, `<event>`, `<component>`, `<home_dcl>`,
Deklarationen von Typen, Konstanten und Exceptions im Interface, `oneway`,
`context`.

**Repo:** `crates/rpc/src/service_mapping.rs::lower_service` und
`lower_method` (`RpcError::OutsideServiceGrammar` in `crates/rpc/src/error.rs`
für `import`, `typeid`/`typeprefix`, `component`/`home`/`local`/`abstract`,
Typ-, Konstanten- und Exception-Deklarationen im Interface, `oneway`,
`@oneway`, `context`); `crates/idl-lower/src/lower.rs` und
`crates/ast/src/facet/corba.rs` (`repository_directives` für `typeid` und
`typeprefix`)

**Tests:** `crates/rpc/src/service_mapping.rs::tests::the_oneway_keyword_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::the_oneway_annotation_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_context_clause_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_nested_declaration_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_nested_type_declaration_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::an_import_directive_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_typeid_directive_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_typeprefix_directive_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_component_declaration_is_outside_the_service_grammar`,
`crates/rpc/src/service_mapping.rs::tests::a_local_interface_is_outside_the_service_grammar`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_oneway_operation`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_context_clause`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_type_in_an_interface_body`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_constant_in_an_interface_body`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_exception_in_an_interface_body`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_import_directive`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_typeid_and_typeprefix_directives`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_abstract_interface`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_component`,
`crates/lang-rules/tests/interface_bindings.rs::a_local_interface_is_refused_by_the_seam`

**Status:** done — das Dienstmodell in Rust (`lower_service`) weist die
entfallenen Konstrukte mit `RpcError::OutsideServiceGrammar` ab (d2a0e370a1ed,
1de24d22343a). Dieselbe Prüfung macht der Leser der Generatoren,
`crates/lang-rules/src/adapter/interface.rs::service` und `services` (von
`idlc generate` benutzt), mit `InterfaceError::OutsideServiceGrammar`
(bfd6f500aa94); `a_local_interface_is_refused_by_the_seam` zeigt zusätzlich, dass
`cpp_interface_items` und `java_interface_items` für ein `local interface`
fehlschlagen. Der Frontend (`crates/idl-parse`) liest volles IDL 4.2 und hat
kein Profil der Grammatik von §7.3.1.1; die Abweisung liegt in `lower_service`
und im Leser der Generatoren.

### §7.3.1.1 Annotationen der Basic-Abbildung bleiben nicht im Ergebnis

**Spec:** §7.3.1.1, S. 23 — "The Basic Service Mapping uses annotations only
for interface declarations and not on regular type declarations. These
interface annotations are resolved in the mapping such that the resulting IDL
used by DDS does not have annotations."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (die
synthetisierten Typen sind Strukturen ohne Annotation)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** done — Commit d047dc5f497c.

### §7.3.1.2 Typsystem von XTypes in Signaturen (Enhanced)

**Spec:** §7.3.1.2, S. 23 — "The Enhanced Service Mapping allows use of the
full type-system defined in the DDS-XTypes specification in the declaration of
interface attributes, operation parameters, and return values. [...] extends
the IDL productions defined in the Basic Service Mapping with all the
productions [...] in sub clause 7.3.1.12.1 (New Productions) of the DDS-XTypes
specification."

**Repo:** `crates/idl-parse/`, `crates/idl-lower/` (IDL 4.2 mit den Typen von
XTypes 1.3)

**Tests:** `crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`

**Status:** done — Commit 784e1e121a5f.

### §7.3.1.2 Geänderte Produktionen von XTypes, <definition> wie Basic

**Spec:** §7.3.1.2, S. 23 — "In addition, the Enhanced Service Mapping also
uses all the modified productions defined in sub clause 7.3.1.12.2 (Modified
Productions) of the [DDS-Xtypes] specification with the exception of the
production for <definition>, which shall remain, as defined for the Basic
Service Mapping."

**Repo:** `crates/idl-parse/src/decl.rs`; `crates/rpc/src/service_mapping.rs::lower_service`

**Tests:** `crates/idl-lower/tests/conformance.rs::the_xtypes_specifications_own_idl_lowers_completely`,
`crates/rpc/src/service_mapping.rs::tests::a_nested_declaration_is_outside_the_service_grammar`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_type_in_an_interface_body`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_a_constant_in_an_interface_body`

**Status:** done — die geänderten Produktionen von XTypes liest der
Frontend. `<definition>` bleibt für das Dienstmodell in Rust (`lower_service`,
d2a0e370a1ed) und für den Leser der Generatoren in `crates/lang-rules`
(bfd6f500aa94) die der Basic-Abbildung; der Frontend selbst liest IDL 4.2
(Item „§7.3.1.1 — Konstrukte außerhalb der Grammatik der Abbildung“ oben).

### §7.3.1.3 Example of an Interface in IDL

**Spec:** §7.3.1.3, S. 23–24 — Beispiel `robot::RobotControl`
("Non-normative").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.3.1.4 Paar von Typen mit @RPCRequestType und @RPCReplyType

**Spec:** §7.3.1.4, S. 24 — "A service definition may simply include a pair of
types, which may be the same. The request and reply types shall be marked as
such using the @RPCRequestType and @RPCReplyType annotations."

**Repo:** `crates/rpc/src/type_pair.rs::lower_type_pairs` (liest die Paare aus
einem Modell; `TypePairService`); `crates/rpc/src/annotations.rs::lower_single`
(`RpcAnnotation::RpcRequestType`, `RpcAnnotation::RpcReplyType`);
`crates/ast/src/node/annotation.rs` (`BUILTIN_ANNOTATIONS`, die Annotationen
brauchen keine Deklaration)

**Tests:** `crates/rpc/src/type_pair.rs::tests::a_marked_pair_with_headers_is_a_basic_service`,
`crates/rpc/src/type_pair.rs::tests::one_type_may_be_both_halves`,
`crates/rpc/src/annotations.rs::tests::the_pair_annotations_lower_and_resolve`,
`crates/rpc/src/annotations.rs::tests::the_spec_annotations_need_no_declaration`

**Status:** done — 3c21f462c6e2 (vordefinierte Annotationen), 03b9867ea4df
(`lower_type_pairs`; `@RPCRequest` und `@RPCReply`, die die Spec nicht kennt,
entfernt).

### §7.3.1.4 Auswertung des Paars von Typen in IDL

**Spec:** §7.3.1.4, S. 24 — wie oben.

**Repo:** `crates/rpc/src/type_pair.rs::lower_type_pairs`;
`crates/lang-rules/src/emit/type_pair_rust.rs`,
`crates/lang-rules/src/emit/type_pair_cpp.rs`,
`crates/lang-rules/src/emit/type_pair_java.rs` (erzeugte Bindung über den
Endpunkten, `has_pair_types`)

**Tests:** `crates/lang-rules/tests/type_pair_rust.rs::a_pair_with_headers_gets_the_header_traits_and_the_endpoints`,
`crates/lang-rules/tests/type_pair_rust.rs::the_service_name_is_read_from_either_annotation_and_can_be_given_at_run_time`,
`crates/lang-rules/tests/type_pair_cpp.rs::a_pair_with_headers_gets_traits_and_support`,
`crates/lang-rules/tests/type_pair_cpp.rs::the_service_name_is_run_time_then_annotation_then_default`,
`crates/lang-rules/tests/type_pair_java.rs::the_generated_pair_declares_the_support_and_uses_the_runtimes_headers`,
`crates/rpc/src/type_pair.rs::tests::the_service_name_comes_from_either_annotation_on_either_type`

**Status:** done — `lower_type_pairs` macht aus einem Paar markierter Typen
einen Dienst (03b9867ea4df). Die Generatoren erzeugen eine Bindung dafür:
Rust ca7c0cb2c78d, C++ 83d80ac9bf48 und 35a69aa60d98, Java f46b42ca8b3a
(Laufzeit) und 639d549b4bfd (Generator). Der Name des Dienstes kommt aus
`@service` oder `@DDSService` an einem der Typen oder zur Laufzeit
(e4ca8f81ff8e). Java: `crates/lang-rules/tests/type_pair_java.rs` hat
Tests; ihr Namensabgleich wurde hier nicht geprüft.

### §7.3.1.4 Nur Strukturen als Typen des Paars

**Spec:** §7.3.1.4, S. 24 — "Only struct types shall be marked as
request/reply types."

**Repo:** `crates/rpc/src/type_pair.rs::lower_type_pairs` (`RpcError::InvalidTypePair`
für einen markierten Typ, der keine Struktur ist)

**Tests:** `crates/rpc/src/type_pair.rs::tests::a_marked_type_that_is_not_a_struct_is_refused`,
`crates/lang-rules/tests/type_pair_rust.rs::a_marked_type_that_is_not_a_struct_is_refused`

**Status:** done — 03b9867ea4df.

### §7.3.1.4 Basic: Member header im Paar von Typen

**Spec:** §7.3.1.4, S. 24 — "In the Basic Service Mapping, the types annotated
as @RPCRequestType shall have a member named header of type
dds::rpc::RequestHeader and the types annotated as @RPCReplyType shall have a
member named header of type dds::rpc::RequestHeader."

**Repo:** `crates/rpc/src/type_pair.rs::lower_type_pairs` (`header_type`;
Request-Typ: `dds::rpc::RequestHeader`; Reply-Typ: `dds::rpc::ReplyHeader`, mit
`TypePairOptions::reply_header` auf `dds::rpc::RequestHeader` umstellbar)

**Tests:** `crates/rpc/src/type_pair.rs::tests::a_marked_pair_with_headers_is_a_basic_service`,
`crates/rpc/src/type_pair.rs::tests::a_header_of_the_wrong_type_is_refused`,
`crates/rpc/src/type_pair.rs::tests::a_header_in_only_one_type_is_refused`,
`crates/rpc/src/type_pair.rs::tests::the_reply_header_type_is_a_setting`

**Status:** done — 03b9867ea4df. Für den Reply-Typ nennt der Satz der Spec
`dds::rpc::RequestHeader`; §7.5.1.1.1 definiert `ReplyHeader`, und
`RequestHeader` hat kein `related_request_id`. Voreinstellung ist
`ReplyHeader`; der Wortlaut des Satzes ist über `TypePairOptions::reply_header`
wählbar.

### §7.3.1.4 Enhanced: keine Annotationen, keine besonderen Member

**Spec:** §7.3.1.4, S. 24 — "When using the Enhanced Service Mapping, the
annotations are not necessary and the special data members must not be
defined."

**Repo:** `crates/rpc/src/type_pair.rs::lower_type_pairs` (`PairMapping::Enhanced`
für ein Paar ohne `header`-Member in beiden Typen)

**Tests:** `crates/rpc/src/type_pair.rs::tests::a_marked_pair_without_headers_is_an_enhanced_service`,
`crates/lang-rules/tests/type_pair_rust.rs::a_pair_without_headers_is_the_enhanced_mapping`,
`crates/lang-rules/tests/type_pair_cpp.rs::a_pair_without_headers_is_the_enhanced_mapping`,
`crates/lang-rules/tests/type_pair_java.rs::the_enhanced_pair_has_no_header_and_selects_the_enhanced_mapping`,
`crates/rpc/src/type_pair.rs::tests::a_header_in_only_one_type_is_refused`

**Status:** done — 03b9867ea4df. Ein Paar ohne `header`-Member ist Enhanced;
ein `header` in nur einem Typ wird abgewiesen. Ein Paar wird am Modell nur an
den Annotationen erkannt; Typen ohne Annotation bilden kein Paar, und die Spec
nennt kein anderes Merkmal.

### §7.3.2 Java: InterfaceDeclaration

**Spec:** §7.3.2, S. 24 — "Conforming implementations shall support the syntax
generated by the following modified productions from the Java grammar.
[...] InterfaceDeclaration: NormalInterfaceDeclaration"; Design Rationale,
S. 25: "Removed annotation type declaration."

**Repo:** `crates/rpc/src/java_service_def.rs::parse`

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_example_of_seven_three_two_one_parses`,
`crates/rpc/src/java_service_def.rs::tests::the_removed_productions_are_refused`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: NormalInterfaceDeclaration

**Spec:** §7.3.2, S. 24 — "NormalInterfaceDeclaration: interface Identifier
[extends TypeList] InterfaceBody"; Design Rationale, S. 25: "Removed generic
type parameters."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::interface`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_extends_list_survives`,
`crates/rpc/src/java_service_def.rs::tests::the_removed_productions_are_refused`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: InterfaceBody

**Spec:** §7.3.2, S. 24 — "InterfaceBody: { InterfaceBodyDeclaration
{InterfaceBodyDeclaration} }"; Design Rationale, S. 25: "At least one method
must be present in an interface."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::interface`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::an_empty_interface_body_is_refused`,
`crates/rpc/src/java_service_def.rs::tests::the_example_of_seven_three_two_one_parses`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: InterfaceMemberDecl

**Spec:** §7.3.2, S. 24 — "InterfaceMemberDecl: InterfaceMethodOrFieldDecl
void Identifier VoidInterfaceMethodDeclaratorRest InterfaceDeclaration
ClassDeclaration"; Design Rationale, S. 25: "Removed generic method
declaration."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::interface`,
`Cursor::method`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::a_return_type_and_a_void_are_told_apart`,
`crates/rpc/src/java_service_def.rs::tests::a_nested_declaration_is_kept`,
`crates/rpc/src/java_service_def.rs::tests::a_field_in_an_interface_body_is_a_constant`

**Status:** done — Methoden, `void`-Methoden und verschachtelte Deklarationen
werden gelesen. Ein Feld im Interface-Körper liest `Cursor::interface` als
Konstante (`JavaInterface::constants`), auch mit Dimensionen und mehreren
Deklaratoren; ein Feld ohne Wert wird mit `RpcError::JavaSyntax` abgewiesen
(bd1f56dfddbb).

### §7.3.2 Java: NormalClassDeclaration

**Spec:** §7.3.2, S. 24 — "NormalClassDeclaration: class Identifier [extends
Type] [implements TypeList] ClassBody"; Design Rationale, S. 25: "Removed
generic type parameters."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::class`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_example_of_seven_three_two_one_parses`,
`crates/rpc/src/java_service_def.rs::tests::the_removed_productions_are_refused`,
`crates/rpc/src/java_service_def.rs::tests::the_implements_list_of_a_class_survives`

**Status:** done — `class`, `extends`, `implements` (`JavaClass::implements`)
und die Abweisung von Generics sind getestet (bd1f56dfddbb).

### §7.3.2 Java: ClassBody

**Spec:** §7.3.2, S. 24–25 — "ClassBody: { ClassBodyDeclaration
{ClassBodyDeclaration} }"; Design Rationale, S. 25: "At least one data member
must be defined in the class body."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::class`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::an_empty_class_body_is_refused`,
`crates/rpc/src/java_service_def.rs::tests::the_pair_of_types_of_seven_three_two_two_is_found`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: ClassBodyDeclaration

**Spec:** §7.3.2, S. 25 — "ClassBodyDeclaration: ; {Modifier} MemberDecl";
Design Rationale, S. 25: "Removed Java’s static block support."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::class`,
`Cursor::modifiers`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_removed_productions_are_refused`,
`crates/rpc/src/java_service_def.rs::tests::the_pair_of_types_of_seven_three_two_two_is_found`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: MemberDecl

**Spec:** §7.3.2, S. 25 — "MemberDecl: MethodOrFieldDecl"; Design Rationale,
S. 25: "This specification shall support data member declarations inside a
Java class."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::class`,
`JavaClass::fields`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_pair_of_types_of_seven_three_two_two_is_found`,
`crates/rpc/src/java_service_def.rs::tests::the_removed_productions_are_refused`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: MethodOrFieldRest

**Spec:** §7.3.2, S. 25 — "MethodOrFieldRest: VariableDeclaratorRest"; Design
Rationale, S. 25: "Removed method declarator."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::class`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_pair_of_types_of_seven_three_two_two_is_found`,
`crates/rpc/src/java_service_def.rs::tests::dimensions_after_the_name_belong_to_the_type`,
`crates/rpc/src/java_service_def.rs::tests::an_initialised_member_is_read_without_its_value`,
`crates/rpc/src/java_service_def.rs::tests::several_declarators_share_one_type`,
`crates/rpc/src/java_service_def.rs::tests::a_field_does_not_make_a_method_acceptable_in_a_class`

**Status:** done — ein Feld mit und ohne Initialisierung, mehrere
Deklaratoren (`int a, b;`) und Dimensionen nach dem Namen (`int a[];`, addiert
zu den Dimensionen des Typs) werden gelesen und getestet (bd1f56dfddbb).

### §7.3.2 Java: Feld im Interface, implements und VariableDeclaratorRest

**Spec:** §7.3.2, S. 24–25 — wie oben.

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::interface`, `Cursor::class`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::a_field_in_an_interface_body_is_a_constant`,
`crates/rpc/src/java_service_def.rs::tests::dimensions_after_the_name_belong_to_the_type`,
`crates/rpc/src/java_service_def.rs::tests::an_initialised_member_is_read_without_its_value`,
`crates/rpc/src/java_service_def.rs::tests::several_declarators_share_one_type`,
`crates/rpc/src/java_service_def.rs::tests::the_implements_list_of_a_class_survives`

**Status:** done — ein Feld im Interface-Körper ist eine Konstante und
braucht einen Wert; Dimensionen nach dem Namen, Initialisierung, mehrere
Deklaratoren und `implements` sind gelesen und getestet (bd1f56dfddbb).

### §7.3.2 Java: @in, @out, @inout

**Spec:** §7.3.2, S. 25 — "In, out, and inout parameters are specified using
the @in, @out and @inout annotations. By default the parameters are read only,
and the use of @in is optional."

**Repo:** `crates/rpc/src/java_service_def.rs::direction_of`

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_direction_annotations_of_seven_three_two_are_read`

**Status:** done — d047dc5f497c.

### §7.3.2 Java: @oneway

**Spec:** §7.3.2, S. 25 — "One or more operations may be marked oneway using
the @oneway annotation."

**Repo:** `crates/rpc/src/java_service_def.rs` (`Cursor::method`,
`JavaMethod::oneway`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_oneway_annotation_is_read_and_checked`

**Status:** done — d047dc5f497c.

### §7.3.2.1 Example of an interface in Java

**Spec:** §7.3.2.1, S. 26 — Beispiel `RobotControl` ("Non-normative").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.3.2.2 Java: Paar von Typen mit @RPCRequestType und @RPCReplyType

**Spec:** §7.3.2.2, S. 26 — "A service definition may simply include a pair of
types, which may be the same. The request and reply types shall be marked as
such using the @RPCRequestType and @RPCReplyType annotations."

**Repo:** `crates/rpc/src/java_service_def.rs::request_reply_pair`

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_pair_of_types_of_seven_three_two_two_is_found`,
`crates/rpc/src/java_service_def.rs::tests::one_class_may_be_both_halves_of_the_pair`

**Status:** done — d047dc5f497c.

### §7.3.2.2 Java: nur Klassen als Typen des Paars

**Spec:** §7.3.2.2, S. 26 — "Only class types shall be marked as request/reply
types."

**Repo:** `crates/rpc/src/java_service_def.rs::request_reply_pair` (betrachtet
nur Klassen)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_pair_of_types_of_seven_three_two_two_is_found`,
`crates/rpc/src/java_service_def.rs::tests::a_marked_interface_is_reported`

**Status:** done — `request_reply_pair` meldet ein markiertes Interface, welche
Hälfte auch immer, mit `RpcError::InvalidTypePair` („only class types“)
(bd1f56dfddbb).

### §7.3.2.2 Java Basic: Member header vom Typ dds.rpc.RequestHeader

**Spec:** §7.3.2.2, S. 26 — "In the Basic Service Mapping, the types annotated
as @RPCRequestType shall have a member named header of type
dds.rpc.RequestHeader and the types annotated as @RPCReplyType shall have a
member named header of type dds.rpc.RequestHeader."

**Repo:** `crates/rpc/src/java_service_def.rs::request_reply_pair`, `JavaClass::has_header_member`

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_type_of_the_header_member_is_checked`,
`crates/rpc/src/java_service_def.rs::tests::a_header_in_only_one_class_is_reported`

**Status:** done — `request_reply_pair` prüft den Typ von `header`
(`dds.rpc.RequestHeader` für den Request-Typ, `dds.rpc.ReplyHeader` für den
Reply-Typ, mit `TypePairOptions::reply_header` auf `RequestHeader` umstellbar)
und meldet ein `header` in nur einer Klasse (bd1f56dfddbb).

### §7.3.2.2 — Prüfungen des Paars in Java

**Spec:** §7.3.2.2, S. 26 — wie die beiden Items oben.

**Repo:** `crates/rpc/src/java_service_def.rs::request_reply_pair`

**Tests:** `crates/rpc/src/java_service_def.rs::tests::a_marked_interface_is_reported`,
`crates/rpc/src/java_service_def.rs::tests::the_type_of_the_header_member_is_checked`,
`crates/rpc/src/java_service_def.rs::tests::a_header_in_only_one_class_is_reported`,
`crates/rpc/src/java_service_def.rs::tests::an_incomplete_or_ambiguous_pair_is_reported`,
`crates/rpc/src/java_service_def.rs::tests::no_marked_class_is_no_pair`

**Status:** done — ein markiertes Interface, ein falscher Typ von `header`, ein
`header` in nur einer Klasse, eine fehlende Hälfte und zwei Klassen derselben
Hälfte ergeben `RpcError::InvalidTypePair`, wie `crates/rpc/src/type_pair.rs`
es für IDL tut (bd1f56dfddbb). Ohne markierte Klasse gibt es kein Paar und
keinen Fehler.

### §7.3.2.2 Java Enhanced: keine Annotationen, keine besonderen Member

**Spec:** §7.3.2.2, S. 26 — "When using the Enhanced Service Mapping, the
annotations are not necessary and the special data members must not be
defined."

**Repo:** `crates/rpc/src/java_service_def.rs::request_reply_pair` (`JavaTypePair::mapping`, `PairMapping`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::a_pair_without_headers_is_an_enhanced_pair`,
`crates/rpc/src/java_service_def.rs::tests::a_header_in_only_one_class_is_reported`

**Status:** done — `request_reply_pair` entscheidet wie `PairMapping` in
`crates/rpc/src/type_pair.rs`: `header` in beiden Klassen ist Basic, in keiner
Enhanced, in einer ein Fehler (bd1f56dfddbb). Die Java-Bindung für ein Paar aus
IDL (`crates/lang-rules/src/emit/type_pair_java.rs`, `ServiceMapping.ENHANCED`)
ist davon unabhängig.

---

## §7.4 Mapping Service Specification to DDS Topics

### §7.4.1 <topic_name> aus Interface, Service-Name und Suffix

**Spec:** §7.4.1, S. 26 — "<topic_name> ::= <interface_name> “_”
<service_name> “_” [ “Request” | “Reply” ] | <user_def_alpha_num>".

**Repo:** `crates/rpc/src/topic_naming.rs` (`compose_topic_name`,
`ServiceTopicNames::synthesize`);
`crates/lang-rules/src/adapter/interface.rs::Service::request_topic_name`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_default_name_carries_the_service_segment`,
`crates/rpc/src/service_mapping.rs::tests::calculator_service_with_in_params_lowers`

**Status:** done — 902a5609dbb6.

### §7.4.1 <topic_name> als Name des Benutzers

**Spec:** §7.4.1, S. 26 — "Topic name is either a user-defined string literal
or a composite string consisting <interface_name> and <service_name>."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::one` (ganzer Name
aus der Annotation)

**Tests:** `crates/rpc/src/topic_naming.rs::tests::an_annotation_gives_the_whole_name_and_no_alias`,
`crates/rpc/tests/runtime_e2e.rs::e2e_annotated_whole_topic_name_replaces_the_default_dds_rpc_1_0_section_7_4_2_2`

**Status:** done — 902a5609dbb6 (Namen), 2c975ce36483 (Requester und Replier
nehmen den Namen aus der Annotation).

### §7.4.1 <service_name> mit Vorgabe "Service"

**Spec:** §7.4.1, S. 26–27 — "<service_name> ::= “Service” |
<user_def_alpha_num> [...] The <service_name> non-terminal may be
user-supplied. When it is not, it defaults to “Service.”"

**Repo:** `crates/rpc/src/topic_naming.rs::DEFAULT_SERVICE_NAME`;
`crates/rpc/src/service_mapping.rs::lower_service`

**Tests:** `crates/rpc/src/service_mapping.rs::tests::without_the_annotation_the_service_segment_is_the_default`,
`crates/rpc/src/topic_naming.rs::tests::service_topic_names_pair`

**Status:** done — 902a5609dbb6.

### §7.4.1 <user_def_alpha_num>

**Spec:** §7.4.1, S. 26–27 — "<user_def_alpha_num> ::= ^[[:alnum:]_ ]+$ [...]
a regular expression that includes alphanumeric characters (a-zA-Z0-9),
underscore and space."

**Repo:** `crates/rpc/src/topic_naming.rs::validate_user_def_alpha_num`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_published_character_set_admits_a_space_and_a_leading_digit`,
`crates/rpc/src/topic_naming.rs::tests::everything_outside_the_set_is_refused`

**Status:** done — 902a5609dbb6.
### §7.4.1 <interface_name> voll qualifiziert

**Spec:** §7.4.1, S. 26 — "The <interface_name> non-terminal represents the
fully-qualified name of the interface, which is captured automatically for
interface-based service definitions. A fully qualified interface-name includes
concatenation of module names separates by underscores followed by the name of
the interface."

**Repo:** `crates/rpc/src/topic_naming.rs::qualified_interface_name`;
`crates/rpc/src/service_mapping.rs` (`qualified_name_of`);
`crates/lang-rules/src/adapter/interface.rs::qualified_interface_name`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_interface_segment_is_the_fully_qualified_name`,
`crates/rpc/src/service_mapping.rs::tests::the_module_chain_reaches_the_topic_name`

**Status:** `done` — Code und Tests vorhanden. Commit 902a5609dbb6 (der Topic-Name ist `<interface>_<service>_Request`).

### §7.4.1 Angegebener <service_name> mit den Suffixen Request und Reply

**Spec:** §7.4.1, S. 27 — "If the <service_name> is specified by the user,
“Request” and “Reply” topic suffixes are used automatically."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::one` (Schritt 3);
in IDL über `@service(name = "…")` (IDL 4.2), gelesen in
`crates/rpc/src/annotations.rs::lower_service_annotation`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::a_user_supplied_service_name_replaces_only_that_segment`,
`crates/rpc/src/service_mapping.rs::tests::the_service_annotation_supplies_the_service_segment_only`

**Status:** `done` — Code und Tests vorhanden. Commits 902a5609dbb6 (Bildung der Namen), 0ecf76d597c0 (Lesen von `@service`).

### §7.4.1 Java: <service_name> aus @DDSService

**Spec:** §7.4.1, S. 26–27 — wie oben: der Benutzer liefert `<service_name>`;
`<interface_name>` wird aus dem Interface übernommen.

**Repo:** `crates/rpc/src/java_service_def.rs` (`JavaInterface::service_name`,
`JavaServiceDef::service_name`, `JavaServiceDef::topic_names`);
`crates/lang-rules/src/emit/interface_java.rs` (erzeugte `Support`-Klasse setzt `interfaceName` und `annotatedServiceName`)

**Tests:** `crates/rpc/src/java_service_def.rs::tests::the_service_annotation_overrides_the_interface_name`,
`crates/rpc/tests/java/FunctionCallRoundtrip.java` (Prüfung von `TopicNames.of(named)`; gefahren von
`crates/rpc/tests/java_rpc_runtime.rs`)

**Status:** `done` — `@DDSService(name = "…")` liefert `<service_name>`, `<interface_name>` bleibt der Name des Interface: `Calculator_Arithmetic_Request`. Ohne Namen gilt `Service`. Commits 2a1fd4a2d51e (Rust-Leser), ebf6ead2d67a (erzeugte Java-`Support`-Klasse nennt das Interface für einen Service-Namen zur Laufzeit).

### §7.4.1 Vorrang angegebener Topic-Namen vor dem Service-Namen

**Spec:** §7.4.1, S. 27 — "When <service_name> and user-defined topic names
are both provided by the user, the user-defined topic names take precedence."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::one`;
C++: `runtime/cpp/include/dds/rpc/core.hpp::dds::rpc::synthesize_topic_names`;
Java: `runtime/java/src/main/java/org/omg/dds/rpc/TopicNames.java::resolve`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_carries_the_topic_names_of_seven_four`,
`crates/rpc/src/topic_naming.rs::tests::the_run_time_specification_beats_the_annotation`,
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::check_class_form` (gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`)

**Status:** `done` — innerhalb einer Ebene gilt der Topic-Name (Annotation vor `@service`; Suffix zur Laufzeit vor dem Service-Namen zur Laufzeit). Für einen Service-Namen zur Laufzeit neben einem Topic-Namen aus der Annotation folgt der Code §7.4.2.3 (S. 28: Laufzeit vor Annotation); dort widersprechen sich S. 27 und S. 28. Commit 902a5609dbb6.

### §7.4.1 Stil Request/Reply: kein <interface_name>

**Spec:** §7.4.1, S. 27 — "For the request-reply style language binding,
<interface_name> and the following underscore shall not be captured
automatically."

**Repo:** `crates/rpc/src/topic_naming.rs` (`BindingStyle`, `compose_topic_name`);
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::binding_style`;
Rust-Laufzeit im nächsten Item;
C++: `runtime/cpp/include/dds/rpc/core.hpp::dds::rpc::synthesize_topic_names` (`BindingStyle::RequestReply`);
Java: `runtime/java/src/main/java/org/omg/dds/rpc/TopicNames.java::resolve` (leerer `interfaceName` lässt das Segment weg)

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_request_reply_style_drops_the_interface_segment`,
`crates/rpc/src/endpoint.rs::tests::the_binding_style_decides_the_topic_name`,
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::check_class_form` (gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`)

**Status:** `done` — Bildung der Namen in Rust und C++ mit Test. Für Java ist der Fall in `TopicNames.resolve` im Code vorhanden; ein Test mit leerem `interfaceName` besteht nicht. Commits 902a5609dbb6 (Bildung), e143bb6bfa0b (C++), a0f9fe096716 (Java).

### §7.4.1 Rust-Laufzeit: Topic-Namen im Stil Request/Reply

**Spec:** §7.4.1, S. 27 — wie oben.

**Repo:** `crates/rpc/src/requester.rs` (`Requester::with_instance_style`, `with_topics_style`, `with_options`);
`crates/rpc/src/replier.rs` (`Replier::with_instance_style`, `with_topics_style`, `with_options`)

**Tests:** `crates/rpc/src/requester.rs::tests::request_reply_style_drops_the_interface_segment`,
`crates/rpc/src/replier.rs::tests::request_reply_style_drops_the_interface_segment`

**Status:** `done` — `with_instance` und `with_topics` bilden den Stil Funktionsaufruf; die Varianten `*_style` nehmen den Stil entgegen und lassen das Segment `<interface_name>` weg. Die Tests prüfen die Namen am `Requester` und am `Replier`. Commits 575f889c965b (Bildung), e8821ba9dae4 (Test am `Replier`).

### §7.4.2 Basic: ein Request- und ein Reply-Topic je Interface

**Spec:** §7.4.2, S. 27 — "The Basic Service mapping maps every interface to a
request topic and a reply topic."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair`;
`crates/lang-rules/src/emit/interface_rust.rs` (`client_impl`; ein `Requester` je Interface)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_pair_topic_names`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`

**Status:** `done` — Code und Tests vorhanden. Commits b5224bd70dbb (`build_basic_pair`), 15c8509f56b1 (Test mit erzeugter Bindung).

### §7.4.2 Drei Mechanismen, gleiche Namen auf beiden Seiten

**Spec:** §7.4.2, S. 27 — "It provides three alternative mechanisms to specify
the names of the topics. It is possible to use different mechanism at the
client and server sides. However, to ensure successful end-point matching, the
topic names must match."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Übersicht und Vorgabe an die Anwendung; die Mechanismen stehen in §7.4.2.1 bis §7.4.2.3.

### §7.4.2 Basic: eigenes Paar von Topics je Interface der Hierarchie

**Spec:** §7.4.2, S. 27 — "In case of an interface inheritance hierarchy,
including multiple inheritance, each interface in the hierarchy shall be mapped
to its own pair of request and reply topics."

**Repo:** `crates/lang-rules/src/emit/interface_rust.rs` (`client_impl`: ein Feld mit dem Client der Basis je Basis; ein Dispatcher je Ebene);
`crates/lang-rules/src/emit/interface_cpp.rs::support_class` (Namen je Interface)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::a_derived_client_reaches_an_inherited_operation_through_its_base_field`

**Status:** `done` — Code und Test vorhanden. Commit 15c8509f56b1.

### §7.4.2.1 Default Topic Names

**Spec:** §7.4.2.1, S. 27 — "The default topic names are synthesized using the
rules defined in sub clause 7.4.1 Rules for Synthesizing DDS Topic Names."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::new`;
`crates/rpc/src/service_mapping.rs::ServiceDef::topic_names`;
`crates/lang-rules/src/emit/interface_cpp.rs::support_class`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::service_topic_names_pair`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_carries_the_topic_names_of_seven_four`

**Status:** `done` — Code und Tests vorhanden. Commit 902a5609dbb6.

### §7.4.2.2 @DDSRequestTopic und @DDSReplyTopic an Interfaces

**Spec:** §7.4.2.2, S. 27 — "Interfaces and the request/reply types may be
annotated to specify the names of the request and reply topics.
@DDSRequestTopic and @DDSReplyTopic annotations are pre-defined for this
purpose."

**Repo:** `crates/rpc/src/annotations.rs::lower_single`;
`crates/rpc/src/service_mapping.rs::ServiceDef` (`request_topic`, `reply_topic`);
`crates/lang-rules/src/adapter/interface.rs::service` (`annotation_string`)

**Tests:** `crates/rpc/src/annotations.rs::tests::dds_request_and_reply_topics_lower_and_resolve`,
`crates/rpc/src/service_mapping.rs::tests::the_topic_annotations_and_the_run_time_layer_have_their_order`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_carries_the_topic_names_of_seven_four`

**Status:** `done` — an Interfaces gelesen; an den Typen eines Paars und als vordefinierte Annotationen in den beiden folgenden Items. Commit 902a5609dbb6.

### §7.4.2.2 Annotationen an den Typen eines Paars

**Spec:** §7.4.2.2, S. 27 — wie oben.

**Repo:** `crates/rpc/src/type_pair.rs::lower_type_pairs` (liest `@DDSRequestTopic` und `@DDSReplyTopic` an den Typen, §7.3.1.4)

**Tests:** `crates/rpc/src/type_pair.rs::tests::the_topic_annotations_on_the_types_name_the_topics`

**Status:** `done` — Code und Test vorhanden. Commit 03b9867ea4df.

### §7.4.2.2 DDSRequestTopic und DDSReplyTopic vordefiniert

**Spec:** §7.4.2.2, S. 27 — "module dds { module rpc { @Annotation local
interface DDSRequestTopic { attribute String name; }; @Annotation local
interface DDSReplyTopic { attribute String name; }; }; };"

**Repo:** `crates/ast/src/node/annotation.rs` (`BUILTIN_ANNOTATIONS`: `DDSRequestTopic`, `DDSReplyTopic`);
`crates/rpc/src/annotations.rs::lower_single`

**Tests:** `crates/rpc/src/annotations.rs::tests::the_spec_annotations_need_no_declaration`,
`crates/rpc/src/annotations.rs::tests::dds_request_and_reply_topics_lower_and_resolve`

**Status:** `done` — eine Datei verwendet beide Annotationen ohne Deklaration. Sie sind Built-in-Annotationen des Frontends; die IDL-Deklaration der Spec (`module dds { module rpc { @Annotation local interface … } }`) wird nicht als IDL-Text angeboten. Gelesen wird nach dem letzten Namensteil; neben `name = "…"` gilt auch die Kurzform ohne Namen. Ein Aufruf mit vollständigem Namen (`@dds::rpc::DDSRequestTopic`) ist nicht geprüft. Commit 3c21f462c6e2.

### §7.4.2.2 XTypes in der Implementierung nicht nötig

**Spec:** §7.4.2.2, S. 27 — "Note that support for [DDS-XTypes] in the
underlying DDS implementation is not required to interpret the annotations.
These annotations simply control the generated DDS wrapper code."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Hinweis.

### §7.4.2.3 Topic-Namen zur Laufzeit über die Params-Klassen

**Spec:** §7.4.2.3, S. 28 — "DDS Topic names may also be specified at
run-time. The ServiceParams, ClientParams, RequesterParams, and ReplierParams
classes [...] provide functions to specify the service name and the topic name
suffix. When used as such, the rules defined in 7.4.1 Rules for Synthesizing
DDS Topic Names apply."

**Repo:** `crates/rpc/src/topic_naming.rs::TopicNameOverrides` (`service_name`, `request_suffix`, `reply_suffix`), weitergegeben über `AnnotatedTopics` an `Requester::with_topics` und `Replier::with_topics`;
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::topic_name_overrides`;
C++ und Java im nächsten Item

**Tests:** `crates/rpc/src/topic_naming.rs::tests::a_user_supplied_suffix_replaces_the_whole_tail`,
`crates/rpc/src/endpoint.rs::tests::the_run_time_service_name_reaches_the_endpoints`

**Status:** `done` — Rust: Service-Name und Suffix zur Laufzeit mit den Regeln aus §7.4.1; C++ und Java im Item „Params-Klassen in C++ und Java“. Commits 902a5609dbb6, 2c975ce36483.

### §7.4.2.3 Params-Klassen in C++ und Java

**Spec:** §7.4.2.3, S. 28 — wie oben.

**Repo:** C++: `runtime/cpp/include/dds/rpc.hpp` (`EndpointParams` mit `service_name`, `request_topic_name_suffix`, `reply_topic_name_suffix`; `RequesterParams`, `ReplierParams`, `ServiceParams`, `ClientParams`; `synthesize_topic_names`);
Java: `runtime/java/src/main/java/org/omg/dds/rpc/` (`RequesterParams`, `ReplierParams` mit `serviceName`, `requestTopicNameSuffix`, `replyTopicNameSuffix`; `TopicNames`)

**Tests:** `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::check_class_form` (gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`),
`crates/rpc/tests/java/FunctionCallRoundtrip.java` (`TopicNames.of(named)`; gefahren von `crates/rpc/tests/java_rpc_runtime.rs`)

**Status:** `done` — die Params-Klassen nehmen Service-Name und Suffix auf und bilden die Namen nach §7.4.1. `ServiceParams` und `ClientParams` stehen in der Spec nur in der C++-Bindung (§7.11.1.5); Java hat sie nicht. Commits e143bb6bfa0b (C++), a0f9fe096716 (Java).

### §7.4.2.3 Vorrang: Laufzeit, Annotation, Vorgabe

**Spec:** §7.4.2.3, S. 28 — "When more than one method of specifying topic
names is used, the run-time specification shall take precedence over the IDL
annotations and the default mechanism in that order."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::one`;
C++: `runtime/cpp/include/dds/rpc/core.hpp::dds::rpc::synthesize_topic_names`;
Java: `runtime/java/src/main/java/org/omg/dds/rpc/TopicNames.java::resolve`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_run_time_specification_beats_the_annotation`,
`crates/rpc/src/service_mapping.rs::tests::the_topic_annotations_and_the_run_time_layer_have_their_order`,
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::check_class_form` (gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`)

**Status:** `done` — Code und Tests vorhanden. Commits 902a5609dbb6, e143bb6bfa0b (C++), a0f9fe096716 (Java).

### §7.4.3 Enhanced: Aliase in der Discovery angekündigt

**Spec:** §7.4.3, S. 28 — "To support single and multiple inheritance of
interfaces, the Enhanced Service mapping introduces a notion called “topic
aliases.” [...] Topic aliases are announced during the endpoint discovery
protocol using either of the PublicationBuiltinTopicDataExt and
SubscriptionBuiltinTopicDataExt structures described in 7.6.2.1".

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::aliases`;
`crates/rpc/src/requester.rs` und `crates/rpc/src/replier.rs` (`with_options` tragen `topic_aliases` in `RpcDiscoveryExt` ein);
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder`;
`crates/rtps/src/publication_data.rs` (`topic_aliases`, Codec von `PID_TOPIC_ALIASES`);
`crates/dcps` (Zuordnung nach Alias, Funktionen in `crates/discovery`);
C++: `runtime/cpp/include/dds/rpc.hpp` (`topic_aliases`) und `runtime/cpp/include/dds/rpc/zerodds.hpp`

**Tests:** `crates/rpc/src/endpoint.rs::tests::the_parent_interfaces_are_announced_as_topic_aliases`,
`crates/rpc/src/topic_naming.rs::tests::the_request_reply_style_has_no_aliases_for_the_bases_either`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`,
`crates/rtps/src/publication_data.rs::tests::rpc_discovery_pids_roundtrip`,
`crates/rtps/src/publication_data.rs::tests::a_discovery_string_beyond_the_bound_is_refused`,
`crates/dcps/tests/topic_alias_matching.rs::a_reader_on_an_alias_matches_the_writer_over_the_wire`,
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp` (Prüfung `request_reader.aliases.size() == 1`; gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs`)

**Status:** `done` — Rust, C++ und Java kündigen die Aliase an, `crates/dcps` ordnet nach Alias zu, und `crates/rtps` lehnt einen Alias über `string<256>` ab. In Java setzt `ReplierParams.parents` (und `RequesterParams.parents`) die Basis-Interfaces; `TopicNames` bildet daraus je Basis einen Alias (`95056920ba2f`, Test `RpcInheritanceTest.java::specOperations`, `RpcDiscoveryTest.java::aTopicCorrespondsByNameOrByAlias`). Ein Client einer Basis erreicht einen abgeleiteten Dienst über den Alias-Treffer, obwohl die Typnamen der Request- und Reply-Typen verschieden sind: die Topics der Enhanced-Abbildung kündigen jetzt die `TypeInformation` ihrer Typen an (`lang-rules` erzeugt die `CompleteTypeObject` von Request, Reply, In, Out, Result, Exceptions und `RemoteExceptionCode_t`: `da6bf28aade1`; `DomainParticipant::create_topic_with_type_information` in `crates/dcps`; `Requester` und `Replier` rufen sie auf, wenn die erzeugten Optionen die Objekte tragen). Test: `crates/rpc-rust-e2e/tests/enhanced_type_information.rs::base_and_derived::a_base_client_calls_a_derived_service` (zwei Participants; auf dem Linux-Testhost grün; ohne die Objekte endet derselbe Aufruf mit `ServiceNotMatched`, gemessen mit einem Kontrolllauf). Die Rust-Laufzeit trägt die Typinformation; C++ und Java kündigen keine an (siehe die Items zu §7.5.1.2.6 und §7.7.2.5). Commits 3d4c42cad194 (Rust), 95f5e6617035 (C++), 928df3c39a42 (Zuordnung nach Alias in `crates/dcps`), 9a3940227ee4 (Grenze in `crates/rtps`), e8821ba9dae4 und d79e4dac24d0 (Tests).

### §7.4.3 Enhanced: Java kündigt die Aliase der Params an

**Spec:** §7.4.3, S. 28 — "To support single and multiple inheritance of
interfaces, the Enhanced Service mapping introduces a notion called “topic
aliases.” [...] Topic aliases are announced during the endpoint discovery
protocol using either of the PublicationBuiltinTopicDataExt and
SubscriptionBuiltinTopicDataExt structures described in 7.6.2.1".

**Repo:** —

**Tests:** `crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface`

**Status:** `done` — `RequesterParams.parents` und `ReplierParams.parents` nehmen die Basis-Interfaces; `TopicNames.resolve` bildet je Basis einen Alias (nicht unter dem Stil Request/Reply und nicht bei einem ganzen Topic-Namen, wie in Rust), und `RequesterImpl` und `ReplierImpl` melden sie über `DataWriter.announce` und `DataReader.announce` (`ZeroDdsParticipant` → `DataWriterImpl.setRpcDiscovery`, PID 0x0082; `95056920ba2f`, `9f9cd6cfc777`). Tests: `crates/rpc/tests/java/RpcInheritanceTest.java::specOperations` (Namen und Aliase), `RpcDiscoveryTest.java::theThreeParametersTravelWithTheEndpoint` (PID 0x0082 auf der Leitung). Die erzeugte Java-Bindung der Enhanced-Abbildung setzt `parents` für ein abgeleitetes Interface (`86817fa5f076`, geprüft an `CalcSupport` mit `Calc : Base2` in `EnhancedRuntimeChecks.support()`, `7d0250078a28`). Gemessen: ein Java-Client gegen einen Rust-Dienst und ein Rust-Client gegen einen Java-Dienst über RTPS, beide mit der Ankündigung der Aliase und der `TypeInformation` (`b3748d506a39`). Die Java-PSM paart die Endpunkte der Enhanced-Abbildung über die Aliase der Gegenstelle (`RpcDiscovery.topicsCorrespond`) und nach der Zuweisbarkeit der angekündigten `TypeInformation` (XTypes 1.3 §7.6.3.4.2; `RawTypeSupport.matchesByTypeInformation`, `org.zerodds.rtps.TypeConsistency`, `org.zerodds.typelookup.CompleteTypes`, `95818352de1`). Objekte der Gegenstelle holt sie mit `getTypes` von deren TypeLookup-Dienst; die Java-Beschreibung trägt auch die MINIMAL-Objekte, nach denen ein Rust-Peer fragt (`e6af3ee9150`), und ein Reader eines beschriebenen Typs kündigt ALLOW_TYPE_COERCION an (`64134babe64`). Gemessen über RTPS gegen Rust-Peers (Linux-Testhost): ein Java-Client von `Base2` ruft einen Rust-Dienst von `Derived2`, ein Rust-Client von `Base2` einen Java-Dienst von `Derived2`; beide Zellen schlugen auf `7484a8f0d46`, vor der Paarung nach Zuweisbarkeit, fehl.

### §7.4.3 Enhanced: genau ein Request- und ein Reply-Topic je Interface

**Spec:** §7.4.3, S. 28 — "The Enhanced Service Mapping shall map an interface
to exactly one request topic and a reply topic. The names of the topics are
obtained using the rules defined in 7.4.1 Rules for Synthesizing DDS Topic
Names."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (`request_topic`, `reply_topic`);
`crates/rpc/src/endpoint_options.rs::RpcMapping::Enhanced`;
`Requester::with_options` und `Replier::with_options` (`crates/rpc/src/requester.rs`, `crates/rpc/src/replier.rs`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_service_has_one_topic_pair`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`

**Status:** `done` — die Enhanced-Abbildung bildet ein Paar von Topics je Interface; die Laufzeit in Rust benutzt es (`RpcMapping::Enhanced`). C++ und Java: Commits 51bb6973e062 und 2cb2bcbe4eca. Commits 60dbad781c53 (Abbildung), 3d4c42cad194 (Laufzeit in Rust).

### §7.4.3 Enhanced: Aliase je Basis nach §7.4.1

**Spec:** §7.4.3, S. 28 — "In case of single and multiple inheritance, the
request and reply topics shall additionally have topic-aliases, which are
obtained using the rules in sub clause 7.4.1 for each parent interface."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::aliases`;
`crates/rpc/src/service_mapping.rs::ServiceDef::bases`;
`crates/lang-rules/src/adapter/interface.rs::Service::topic_aliases`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::every_parent_interface_contributes_one_alias`,
`crates/rpc/tests/language_binding_agreement.rs::the_published_table_comes_out_of_the_idl`

**Status:** `done` — Aliase entstehen für die direkten Basen; ob „each parent interface“ auch Basen der Basen meint, sagt die Spec nicht. Commit 902a5609dbb6.

### §7.4.3 Beispiel der Hierarchie Calculator

**Spec:** §7.4.3, S. 28–29 — Tabelle mit Topic-Namen und Aliasen für
`Calculator : Adder, Subtractor` ("Non-normative example").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel.

### §7.4.3 Stil Request/Reply: kein Name des Interface

**Spec:** §7.4.3, S. 29 — "For the request-reply style language binding, the
interfaces name shall not be annexed automatically."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::aliases` (für den Stil Request/Reply keine Aliase);
C++: `runtime/cpp/include/dds/rpc.hpp` (`topic_aliases`)

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_request_reply_style_drops_the_interface_segment` (ohne Basen; Aliasliste leer)

**Status:** `done` — der Stil Request/Reply bildet keinen Namen mit dem Interface und keine Aliase; die Rust-Laufzeit nimmt den Stil entgegen (Item zu §7.4.1). Der Test übergibt keine Basen. Commits 902a5609dbb6, 575f889c965b.

### §7.4.3.1 Default Topic Names (Enhanced)

**Spec:** §7.4.3.1, S. 29 — "The default topic names are synthesized as
described above."

**Repo:** `crates/rpc/src/topic_naming.rs::ServiceTopicNames::synthesize`;
`crates/rpc/src/codegen.rs::build_enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_service_has_one_topic_pair`,
`crates/rpc/src/topic_naming.rs::tests::the_default_name_carries_the_service_segment`

**Status:** `done` — Code und Tests vorhanden. Commits 902a5609dbb6, 60dbad781c53.

### §7.4.3.2 Annotationen für die Namen (Enhanced)

**Spec:** §7.4.3.2, S. 29 — "The same built-in @DDSRequestTopic and
@DDSReplyTopic annotations defined in the Basic Service Mapping may be used
for this purpose."

**Repo:** `crates/rpc/src/service_mapping.rs::ServiceDef` (`request_topic`, `reply_topic`);
`crates/rpc/src/type_pair.rs::lower_type_pairs`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::an_annotation_gives_the_whole_name_and_no_alias`,
`crates/rpc/src/type_pair.rs::tests::the_topic_annotations_on_the_types_name_the_topics`

**Status:** `done` — die Annotationen wirken unabhängig von der Abbildung (Items zu §7.4.2.2). Commits 902a5609dbb6, 03b9867ea4df.

### §7.4.3.3 Topic-Namen zur Laufzeit (Enhanced)

**Spec:** §7.4.3.3, S. 29 — gleicher Wortlaut wie §7.4.2.3, einschließlich
"the run-time specification shall take precedence over the IDL annotations and
the default mechanism in that order."

**Repo:** `crates/rpc/src/topic_naming.rs::TopicNameOverrides`;
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::topic_name_overrides`;
C++ und Java: Params-Klassen (Item zu §7.4.2.3)

**Tests:** `crates/rpc/src/topic_naming.rs::tests::the_run_time_specification_beats_the_annotation`,
`crates/rpc/src/endpoint.rs::tests::the_run_time_service_name_reaches_the_endpoints`

**Status:** `done` — Code und Tests vorhanden. Commits 902a5609dbb6, 60dbad781c53.

---

## §7.5 Mapping Service Specification to DDS Topics Types

### §7.5 Topic-Typen aus der Definition des Interface

**Spec:** §7.5, S. 30 — "The request and reply DDS Topic types shall be
synthesized from the interface definition."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair`, `crates/rpc/src/codegen.rs::build_enhanced_service`;
`crates/lang-rules/src/emit/interface_cpp.rs::declare`;
`crates/lang-rules/src/emit/interface_java.rs`;
`crates/lang-rules/src/emit/interface_rust.rs::declare`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/rpc/tests/language_binding_agreement.rs::both_readings_of_seven_five_produce_the_same_sample_structure`

**Status:** `done` — die Typen werden synthetisiert, und die Topics der Rust-Laufzeit tragen sie unter ihren Namen (nächstes Item). Commits d047dc5f497c (Generatoren), 3d4c42cad194 (Laufzeit in Rust).

### §7.5 Typ und Form der Samples auf der Leitung

**Spec:** §7.5, S. 30 — wie oben; §7.5.1.1.6 und §7.5.1.1.7 legen
`${interfaceName}_Request` und `${interfaceName}_Reply` als Typen der Topics
fest.

**Repo:** `crates/rpc/src/requester.rs` und `crates/rpc/src/replier.rs` (`create_topic_with_type_name` mit `${I}_Request` und `${I}_Reply`; `Requester::request_type_name`);
`crates/rpc/src/wire_codec.rs` (`encode_request_frame`: Auffüllung auf 4 vor dem `Call`; `ERROR_REPLY_DATA`);
C++: `runtime/cpp/include/dds/rpc.hpp` (`Requester` und `Replier` übergeben `dds_type_traits<T>::type_name()`), `runtime/cpp/include/dds/rpc/zerodds.hpp`

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_topics_are_announced_with_the_interface_type_names`,
`crates/rpc/src/wire_codec.rs::tests::the_call_union_starts_on_a_4_octet_boundary`,
`crates/rpc/src/wire_codec.rs::tests::the_error_reply_data_is_the_default_case_of_the_union`,
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp` (angekündigte Typnamen `robot::Calculator_Request` und `_Reply`; gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs`)

**Status:** `done` — die Topics melden `${interfaceName}_Request` und `${interfaceName}_Reply` als Typnamen; ein Sample ist der Header, die Auffüllung auf 4 Oktette und die Kodierung von `${I}_Call` bzw. `${I}_Return`; eine Antwort mit Fehlercode trägt den `default`-Fall der `Return`-Union. Die Rust-Laufzeit schreibt die Oktette dieser Strukturen über `RawBytes`; die erzeugten Rust-Strukturen `${I}_Request` und `${I}_Reply` sind keine Typen eines Topics der Laufzeit, ihre Kodierung ergibt dieselben Oktette (Spalten-Test im Item zur Kodierung). Commits 3d4c42cad194 (Rust), eb4b24caa473 (C++).

### §7.5.1.1 Basic: Typen für DDS 1.3 ohne XTypes

**Spec:** §7.5.1.1, S. 30 — "The Basic Service Mapping maps interfaces to
data-types that can be used by DDS version 1.3 compliant implementations,
which may lack support for the [DDS-XTypes] specification."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (Strukturen und Unions ohne Annotationen von XTypes)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — die Typen nutzen nichts aus XTypes; die Kodierung ist klassisches CDR (nächstes Item). Commit d047dc5f497c.

### §7.5.1.1 Kodierung der Samples: klassisches CDR und XCDR2

**Spec:** §7.5.1.1, S. 30 — wie oben.

**Repo:** `crates/rpc/src/endpoint_options.rs` (`WireEncoding`, Vorgabe `Xcdr1`; `RpcEndpointOptions`);
`crates/lang-rules/src/emit/interface_rust.rs` (`rpc_layout_key_xcdr1`, `rpc_layout_key`);
C++: `runtime/cpp/include/dds/rpc.hpp` (`WireEncoding`);
Java: `runtime/java/src/main/java/org/omg/dds/rpc/WireEncoding.java`

**Tests:** `crates/rpc/src/endpoint_options.rs::tests::the_defaults_are_the_spec_readings`,
`crates/rpc-rust-e2e/tests/wire_bytes_match_columns.rs::the_rust_and_cpp_columns_put_eight_octet_members_where_the_same_octets`,
`crates/rpc-rust-e2e/tests/wire_bytes_match_columns.rs::the_rust_and_java_columns_put_eight_octet_members_where_the_same_octets`,
`crates/rpc-rust-e2e/tests/xcdr1_nested.rs::an_appendable_member_has_no_dheader_under_xcdr1`,
`crates/rpc-rust-e2e/tests/xcdr1_nested.rs::xcdr2_frames_the_same_value_differently`

**Status:** `done` — die Spec nennt für die Basic-Abbildung nur „DDS 1.3 ohne XTypes“. Vorgabe in allen drei Laufzeiten ist klassisches CDR (XCDR1; ein Writer bietet nur diese Darstellung an); XCDR2 ist eine benannte Einstellung. Die Enhanced-Abbildung ist immer XCDR2. Die Spalten für Rust, C++ und Java setzen Member von 8 Oktetten an dieselben Stellen (Test), auch verschachtelte `@appendable`/`@mutable`-Member. Gegen einen fremden Peer ist das in diesem Audit nicht geprüft. Commits 33d1d98b1f92 (Rust), 24e3443ce819 (C++), 43c170c7bca6 (Java), 0e7a9dab792b (verschachtelte Member), 2a06d562261e (Test der Spalten).

### §7.5.1.1.1 GUID_t, SequenceNumber_t und SampleIdentity

**Spec:** §7.5.1.1.1, S. 30 — `module dds { typedef octet GuidPrefix_t[12];
struct EntityId_t { octet entityKey[3]; octet entityKind; }; struct GUID_t {
GuidPrefix_t guidPrefix; EntityId_t entityId; }; struct SequenceNumber_t { long
high; unsigned long low; }; struct SampleIdentity { GUID_t writer_guid;
SequenceNumber_t sequence_number; }; ...`

**Repo:** `crates/rpc/src/common_types.rs::SampleIdentity` (GUID als 16 Oktette, Folgenummer als `high`/`low` auf der Leitung, `encode_sample_identity`);
`runtime/cpp/include/dds/rpc.hpp::dds::SampleIdentity` und `dds::SequenceNumber_t` (`high`, `low`; Member `sequence_number`);
`runtime/java/src/main/java/org/omg/dds/rpc/SampleIdentity.java`

**Tests:** `crates/rpc/src/common_types.rs::tests::xcdr2_layout_sample_identity_le_is_24_bytes_no_padding`,
`crates/rpc/src/common_types.rs::tests::sample_identity_roundtrip_le`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`

**Status:** `done` — die Oktette stimmen in Rust, C++ und Java überein; `dds::SampleIdentity` in C++ hat das Member `sequence_number` vom Typ `dds::SequenceNumber_t` mit `high` und `low`. Commit 569f84427581.

### §7.5.1.1.1 UnknownOperation, UnknownException, UnusedMember

**Spec:** §7.5.1.1.1, S. 31 — "typedef octet UnknownOperation; typedef octet
UnknownException; typedef octet UnusedMember;"

**Repo:** `crates/rpc/src/common_types.rs` (`UnusedMember`, `UnknownOperation`, `UnknownException`);
`runtime/cpp/include/dds/rpc.hpp` (`UnusedMember`, `UnknownOperation`, `UnknownException`);
`runtime/java/src/main/java/org/omg/dds/rpc/` (`UnusedMember.java`, `UnknownOperation.java`, `UnknownException.java`)

**Tests:** `crates/rpc/tests/language_binding_agreement.rs::the_common_types_and_member_names_are_spelled_once`

**Status:** `done` — Code und Test vorhanden. Commit d047dc5f497c.

### §7.5.1.1.1 RemoteExceptionCode_t mit sechs Werten

**Spec:** §7.5.1.1.1, S. 31 — "enum RemoteExceptionCode_t { REMOTE_EX_OK,
REMOTE_EX_UNSUPPORTED, REMOTE_EX_INVALID_ARGUMENT, REMOTE_EX_OUT_OF_RESOURCES,
REMOTE_EX_UNKNOWN_OPERATION, REMOTE_EX_UNKNOWN_EXCEPTION };"

**Repo:** `crates/rpc/src/common_types.rs::RemoteExceptionCode`;
`runtime/cpp/include/dds/rpc/base.hpp::RemoteExceptionCode_t`;
`runtime/java/src/main/java/org/omg/dds/rpc/RemoteExceptionCode.java`

**Tests:** `crates/rpc/src/common_types.rs::tests::reply_header_roundtrip_all_codes`,
`crates/rpc/src/common_types.rs::tests::remote_exception_code_six_is_outside_the_spec_enum`,
`crates/rpc/src/language_binding.rs::tests::the_table_of_seven_eleven_one_three_is_the_published_one`

**Status:** `done` — die sechs Werte mit 0 bis 5 in Rust, C++ und Java; ein siebter Wert besteht nicht mehr (nächstes Item). Commit 099e1d0fd595.

### §7.5.1.1.1 Keine zusätzlichen Werte in RemoteExceptionCode_t

**Spec:** §7.5.1.1.1, S. 31 — wie oben.

**Repo:** `crates/rpc/src/common_types.rs::RemoteExceptionCode::from_u32` (jeder Wert über 5 ist `RpcError::UnknownExceptionCode`);
`runtime/java/src/main/java/org/omg/dds/rpc/RemoteExceptionCode.java::fromWire` (wirft `RemoteUnknownExceptionError`)

**Tests:** `crates/rpc/src/common_types.rs::tests::remote_exception_code_six_is_outside_the_spec_enum`,
`crates/rpc/src/language_binding.rs::tests::the_exception_table_covers_the_six_spec_codes`

**Status:** `done` — `UnknownInterface = 6` ist entfernt; der Wert 6 von der Leitung wird abgelehnt. Commit 099e1d0fd595.

### §7.5.1.1.1 InstanceName als string<255>

**Spec:** §7.5.1.1.1, S. 31 — "typedef string<255> InstanceName;"

**Repo:** `crates/rpc/src/common_types.rs::RequestHeader::instance_name`, `MAX_STRING_LEN` (255), `RequestHeader::to_cdr_le`, `encode_string`;
`crates/rpc/src/wire_codec.rs::encode_request_frame`

**Tests:** `crates/rpc/src/common_types.rs::tests::request_header_roundtrip_le`,
`crates/rpc/src/common_types.rs::tests::request_header_instance_name_over_255_is_rejected_on_encode`,
`crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::main` (Block „InstanceName of 255 characters“; gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`),
`crates/rpc/tests/java/RpcRuntimeRoundtrip.java::headerCodec` (gefahren von `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`)

**Status:** `done` — Rust lehnt einen Namen über 255 beim Kodieren ab, und der Decoder lässt höchstens 256 Oktette einschließlich NUL zu. C++ (`InvalidArgumentError` beim Kodieren, `false` beim Dekodieren) und Java (`IllegalArgumentException` beim Kodieren, `RemoteInvalidArgumentError` beim Dekodieren, Grenze in Oktetten) halten dieselbe Grenze. Commits c0cd73188af3 (Rust), c9a42b1c29af (C++), b86e4eed3d9f (Java).

### §7.5.1.1.1 Grenze 255 von InstanceName in C++ und Java

**Spec:** §7.5.1.1.1, S. 31 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc/core.hpp::dds::rpc::detail::put_string`, `get_string`, `INSTANCE_NAME_BOUND`;
`runtime/java/src/main/java/org/omg/dds/rpc/HeaderCodec.java` (`putString`, `getString`, `MAX_INSTANCE_NAME_OCTETS`)

**Tests:** `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::main` (Block „InstanceName of 255 characters“: 255 Zeichen laufen hin und zurück, 256 werden beim Kodieren und beim Dekodieren abgelehnt; gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`),
`crates/rpc/tests/java/RpcRuntimeRoundtrip.java::headerCodec` (255 Oktette, 256 Oktette, 128 Zeichen mit zwei Oktetten, Längenfeld 257 und über 2^31, fehlende NUL; gefahren von `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`)

**Status:** `done` — beide Laufzeiten prüfen die Grenze 255 beim Kodieren und beim Dekodieren des `RequestHeader`. Commits c9a42b1c29af (C++), b86e4eed3d9f (Java).

### §7.5.1.1.1 RequestHeader

**Spec:** §7.5.1.1.1, S. 31 — "struct RequestHeader { SampleIndentity_t
requestId; InstanceName instanceName; };"

**Repo:** `crates/rpc/src/common_types.rs::RequestHeader`;
`runtime/cpp/include/dds/rpc/base.hpp::RequestHeader`;
`runtime/java/src/main/java/org/omg/dds/rpc/RequestHeader.java`

**Tests:** `crates/rpc/src/common_types.rs::tests::request_header_roundtrip_le`,
`crates/rpc/src/common_types.rs::tests::xcdr2_layout_string_includes_nul`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`,
`crates/rpc/tests/java_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`

**Status:** `done` — Aufbau und Oktette stimmen in Rust, C++ und Java überein. Die C++-Member heißen `requestId` und `instanceName`, und beide Laufzeiten prüfen die Grenze 255 des Namens (Items „§7.5.1.1.1 — Namen der Member in C++“ und „Grenze 255 von InstanceName in C++ und Java“). Commits 569f84427581 (Namen), c9a42b1c29af (C++), b86e4eed3d9f (Java).

### §7.5.1.1.1 ReplyHeader

**Spec:** §7.5.1.1.1, S. 31 — "struct ReplyHeader { dds::SampleIdentity
relatedRequestId; dds::rpc::RemoteExceptionCode_t remoteEx; };"

**Repo:** `crates/rpc/src/common_types.rs::ReplyHeader`;
`runtime/cpp/include/dds/rpc/base.hpp::ReplyHeader`;
`runtime/java/src/main/java/org/omg/dds/rpc/ReplyHeader.java`

**Tests:** `crates/rpc/src/common_types.rs::tests::reply_header_roundtrip_all_codes`,
`crates/rpc/src/wire_codec.rs::tests::reply_header_consumed_28_bytes`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`,
`crates/rpc/tests/java_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`

**Status:** `done` — Aufbau und Oktette stimmen in Rust, C++ und Java überein; die C++-Member heißen `relatedRequestId` und `remoteEx` (Item „§7.5.1.1.1 — Namen der Member in C++“). Commit 569f84427581.

### §7.5.1.1.1 — Namen der Member in C++

**Spec:** §7.5.1.1.1, S. 31 — `requestId`, `instanceName`,
`relatedRequestId`, `remoteEx`.

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`RequestHeader::requestId`, `instanceName`; `ReplyHeader::relatedRequestId`, `remoteEx`; `dds::SequenceNumber_t::high`, `low`; `dds::SampleIdentity::sequence_number`);
erzeugter Code mit dem Präfix `cxx_` für Member, Enumeratoren und Konstanten, die in C++ reserviert sind: `crates/lang-rules/src/emit/interface_cpp.rs`

**Tests:** `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp::print_header_octets` (setzt und liest `requestId`, `instanceName`, `relatedRequestId`, `remoteEx`, `sequence_number.high`, `sequence_number.low`; gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_two_headers_take_the_same_octets_in_both_columns`),
`crates/lang-rules/tests/cxx_prefix_members.rs::a_member_that_is_a_reserved_word_is_prefixed_by_cxx`

**Status:** `done` — die Member heißen wie in der IDL der Spec (`requestId`, `instanceName`, `relatedRequestId`, `remoteEx`), `sequence_number` ist ein eigenes Struktur-Member mit `high` und `low`. Die Java-Laufzeit verwendet die Namen der IDL (`requestId()`, `instanceName()`). Commit 569f84427581.

### §7.5.1.1.2 The Hashing Algorithm

**Spec:** §7.5.1.1.2, S. 31–32 — "long HASH (string arg) { octet md5_hash
[16] ; md5_hash = compute_md5 ( arg ) ; return md5_hash[0] + 256* md5_hash[1]
+ 256*256* md5_hash[2] + 256*256*256* md5_hash[3]; }"

**Repo:** `crates/rpc/src/rpc_hash.rs::rpc_member_hash`;
`crates/lang-rules/src/adapter/interface.rs::operation_hash`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_label_is_the_published_hash_arithmetic`,
`crates/rpc/src/rpc_hash.rs::tests::rpc_hash_uses_first_4_bytes_not_last`,
`crates/rpc/src/rpc_hash.rs::tests::rpc_hash_empty_string_is_md5_empty_first_4_bytes`

**Status:** `done` — die Summe kann 2^31 überschreiten; ZeroDDS liest die 32 Bit als `long` mit Vorzeichen. Die Spec deklariert `long HASH` und sagt nichts zum Überlauf. Commits 4085939c47aa (Algorithmus), 9467d977720c (Labels der Unions).

### §7.5.1.1.2 Kollisionen von *_Hash nicht zu erkennen

**Spec:** §7.5.1.1.2, S. 32 — "Conforming implementation are not required to
detect collisions of identifier names of the form *_Hash".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis. Die Basic-Abbildung prüft keine Kollision (in `build_call_union` nicht vorhanden); die Enhanced-Abbildung meldet `RpcError::MemberIdCollision` (§7.5.1.2.1.2, anderes Item).

### §7.5.1.1.3 Regel 1: get_attribute_ und set_attribute_

**Spec:** §7.5.1.1.3, S. 32 — "1. Each attribute in an interface maps to a pair
of IDL operations. get_attribute_<attribute-name> and
set_attribute_<attribute-name> in the same interface."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations`;
`crates/rpc/src/service_mapping.rs::lower_service` (implizite Operationen je Attribut)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::an_attribute_implies_the_operations_of_seven_five_one_one_three`,
`crates/rpc/src/service_mapping.rs::tests::attributes_lower_into_the_implied_operations_of_seven_five_one_one_three`,
`crates/rpc/src/service_mapping.rs::tests::a_declared_operation_colliding_with_an_implied_one_is_refused`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`

**Status:** `done` — jedes Attribut ergibt `get_attribute_<name>` und, wenn nicht `readonly`, `set_attribute_<name>`, in den Generatoren und im Datenmodell von `crates/rpc`. Commit fb1b6a402a94.

### §7.5.1.1.3 Namen der impliziten Operationen

**Spec:** §7.5.1.1.3, S. 32 — wie oben.

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations` (`get_attribute_<name>`, `set_attribute_<name>`);
`crates/rpc/src/service_mapping.rs::lower_service`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::an_attribute_implies_the_operations_of_seven_five_one_one_three`,
`crates/rpc/src/service_mapping.rs::tests::attributes_lower_into_the_implied_operations_of_seven_five_one_one_three`

**Status:** `done` — der Name der Operation geht über HASH in die Labels von Call- und Return-Union ein (§7.5.1.1.6 Regel 5a); mit den Namen `get_attribute_<name>` und `set_attribute_<name>` wählt ein Peer nach der Spec dieselben Labels. Commit fb1b6a402a94.
### §7.5.1.1.3 Regel 1: Konflikt mit gleichnamigen Operationen

**Spec:** §7.5.1.1.3, S. 32 — "It is illegal to have an interface with an
attribute named <attribute-name> and user-defined operations named
get_attribute_<attribute-name> and set_attribute_<attribute-name>."

**Repo:** `crates/rpc/src/service_mapping.rs::lower_service` (Prüfung über
`ServiceDef::all_methods`, Fehler `RpcError::DuplicateMethod`);
`crates/lang-rules/src/adapter/interface.rs::service` (Fehler
`InterfaceError::DuplicateOperation`, gilt auch für `get_attribute_<name>` und
`set_attribute_<name>`)

**Tests:** `crates/rpc/src/service_mapping.rs::tests::a_declared_operation_colliding_with_an_implied_one_is_refused`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_operation_named_like_an_implied_getter`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_operation_named_like_an_implied_setter`

**Status:** `done` — Commits `fb1b6a402a94`, `bfd6f500aa94`. `lower_service` und
die Stelle `adapter::interface::service` der Generatoren für C++, Java und Rust
weisen ein Interface ab, das eine Operation mit dem Namen einer impliziten
Operation deklariert. Bei einem `readonly`-Attribut gibt es keinen impliziten
Setter, der Name `set_attribute_<name>` ist dort erlaubt (zweiter Test).

### §7.5.1.1.3 Regel 1 — Prüfung in den Generatoren

**Spec:** §7.5.1.1.3, S. 32 — wie oben.

**Repo:** `crates/lang-rules/src/adapter/interface.rs::service`
(`InterfaceError::DuplicateOperation`). `crates/lang-rules` ruft `lower_service`
nicht auf und führt `zerodds-rpc` nicht als Abhängigkeit (Zyklus `zerodds-rpc` →
`zerodds-dcps` → `zerodds-lang-rules`); die Prüfung steht deshalb in
`adapter::interface::service` selbst.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_operation_named_like_an_implied_getter`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_an_operation_named_like_an_implied_setter`,
`crates/lang-rules/tests/interface_bindings.rs::the_seam_refuses_two_operations_of_one_name`

**Status:** `done` — Commit `bfd6f500aa94`. Die Generatoren erhalten für ein
solches Interface den Fehler `InterfaceError::DuplicateOperation` und erzeugen
keine zwei Fälle mit gleichem Namen und Label.

### §7.5.1.1.3 Regel 2: Getter ohne Argumente mit dem Typ des Attributs

**Spec:** §7.5.1.1.3, S. 32 — "2. The return type of the
get_attribute_<attribute-name> operation is the same as the attribute's type.
This operation accepts no arguments."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations`;
`crates/rpc/src/service_mapping.rs::ServiceDef::all_methods`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::an_attribute_implies_the_operations_of_seven_five_one_one_three`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`
(der erzeugte Getter `get_attribute_memory()` hat kein Argument und liefert
`i32`)

**Status:** `done` — Commit `fb1b6a402a94`.

### §7.5.1.1.3 Regel 3: Setter void mit einem Argument

**Spec:** §7.5.1.1.3, S. 32 — "3. The return type of the
set_attribute_<attribute-name> operation is void and it accepts an argument of
the same type as that of the attribute and the name of the argument is the same
as the attribute name."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations`
(Parameter mit `attribute.name`);
`crates/rpc/src/service_mapping.rs::ServiceDef::all_methods`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::an_attribute_implies_the_operations_of_seven_five_one_one_three`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/lang-rules/tests/interface_bindings.rs::the_implied_setter_takes_one_argument_named_like_the_attribute`,
`crates/rpc/src/service_mapping.rs::tests::the_implied_setter_takes_one_argument_named_like_the_attribute`

**Status:** `done` — Commits `fb1b6a402a94`, `272968665473`, `dfd8c6d7f0ec`.
Rückgabe `void` und ein Argument prüft der erste Test; den Typ `i32` erzwingt
der zweite beim Übersetzen von `set_attribute_memory(&self, memory: i32)`. Die
beiden letzten Tests prüfen Name, Richtung `in` und Typ des Arguments gegen das
Attribut und den Member gleichen Namens in `Calculator_set_attribute_label_In`.

### §7.5.1.1.3 Regel 3 — Name des Arguments

**Spec:** §7.5.1.1.3, S. 32 — wie oben.

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations`
(`Operation::params`); `crates/rpc/src/service_mapping.rs::ServiceDef::all_methods`
(`ParamDef::name`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::the_implied_setter_takes_one_argument_named_like_the_attribute`,
`crates/rpc/src/service_mapping.rs::tests::the_implied_setter_takes_one_argument_named_like_the_attribute`

**Status:** `done` — Commits `272968665473`, `dfd8c6d7f0ec`. Beide Tests prüfen,
dass der Parameter des Setters den Namen des Attributs trägt.

### §7.5.1.1.3 Regel 4: getraises

**Spec:** §7.5.1.1.3, S. 32 — "4. Exception types listed in getraises, if any,
are treated as if the get_attribute_<attribute-name> operation has the same set
of exceptions listed as raises."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations`
und `crates/rpc/src/service_mapping.rs::ServiceDef::all_methods` (beide setzen
`raises: attribute.get_raises.clone()`); `lower_attribute` liest `get_raises`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::getraises_and_setraises_become_the_raises_of_the_implied_operations`,
`crates/rpc/src/service_mapping.rs::tests::getraises_and_setraises_become_the_raises_of_the_implied_operations`

**Status:** `done` — Commits `fb1b6a402a94`, `272968665473`, `dfd8c6d7f0ec`. Beide
Tests enthalten ein Attribut mit `getraises` und `setraises` und prüfen, dass
die implizite Operation `get_attribute_<name>` die Exception aus `getraises` in
`raises` führt.

### §7.5.1.1.3 Regel 5: setraises

**Spec:** §7.5.1.1.3, S. 32 — "5. Exception types listed in setraises, if any,
are treated as if the set_attribute_<attribute-name> operation has the same set
of exceptions listed as raises."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::Service::all_operations`
und `crates/rpc/src/service_mapping.rs::ServiceDef::all_methods` (beide setzen
`raises: attribute.set_raises.clone()`); `lower_attribute` liest `set_raises`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::getraises_and_setraises_become_the_raises_of_the_implied_operations`,
`crates/rpc/src/service_mapping.rs::tests::getraises_and_setraises_become_the_raises_of_the_implied_operations`

**Status:** `done` — Commits `fb1b6a402a94`, `272968665473`, `dfd8c6d7f0ec`. Beide
Tests enthalten ein Attribut mit `getraises` und `setraises` und prüfen, dass
die implizite Operation `set_attribute_<name>` die Exception aus `setraises` in
`raises` führt.

### §7.5.1.1.3 Regel 4 und 5 — Result-Union der impliziten Operationen

**Spec:** §7.5.1.1.3, S. 32 — wie oben.

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (Result-Union je
Operation einschließlich der impliziten)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::getraises_and_setraises_become_the_raises_of_the_implied_operations`

**Status:** `done` — Commit `272968665473`. Der Test prüft für
`Guarded_get_attribute_level_Result` den Fall `notreadable_ex` ohne
`notwritable_ex` und für `Guarded_set_attribute_level_Result` den Fall
`notwritable_ex` ohne `notreadable_ex`. Die Prüfung von `all_methods` steht im
Test `crates/rpc/src/service_mapping.rs::tests::getraises_and_setraises_become_the_raises_of_the_implied_operations`
(Commit `dfd8c6d7f0ec`).

### §7.5.1.1.4 Unqualifizierte Namen in den Regeln

**Spec:** §7.5.1.1.4, S. 32 — "In these rules the token ${interfaceName} shall
be replaced with the name of the interface and the token ${operationName}_shall
be replaced with the name of the operation. The substituted names are not fully
qualified (i.e., any module prefixes shall be removed)."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::in_struct_name`;
`crates/rpc/src/codegen.rs::build_call_union` (`svc.name`, der einfache Name)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`
(Interface im Modul `robot`, Typname `Calculator_add_Result`)

**Status:** `done` — Commits `9467d977720c`, `d047dc5f497c`.

### §7.5.1.1.4 Regel 1: In-Struktur ${interfaceName}_${operationName}_In

**Spec:** §7.5.1.1.4, S. 32 — "1. Each operation in the interface shall map to
an In structure with name “${interfaceName}_${operationName}_In.”"

**Repo:** `crates/rpc/src/codegen.rs::build_call_union`;
`crates/lang-rules/src/adapter/interface.rs::in_struct_name`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — Commits `b5224bd70dbb`, `d047dc5f497c`.

### §7.5.1.1.4 Regel 2: In-Struktur im Modul des Interface

**Spec:** §7.5.1.1.4, S. 32 — "2. The In structure shall be defined within the
same module as the original interface."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (jede
Deklaration mit `service.module`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/lang-rules/tests/interface_bindings.rs::the_topic_types_and_constants_stand_in_the_module_of_the_interface`

**Status:** `done` — Commits `b5224bd70dbb`, `d047dc5f497c`, `272968665473`. Der Code übergibt jeder Deklaration das Modul des Interface. Der Test legt zwei Module
an, dazu eine Exception in einem dritten, und prüft für C++, dass die Deklaration
im Namespace ihres Interface steht und weder im globalen Namespace noch im
Namespace des anderen Moduls; die Rust-Bindung legt die Items im Modul ihres
Interface ab. Die Lage der Java-Klassen ist nicht geprüft; die Rust-Bindung hat
keine `_Hash`-Konstanten.

### §7.5.1.1.4 Regel 3: in- und inout-Parameter als Member

**Spec:** §7.5.1.1.4, S. 32 — "3. The In structure shall contain as members the
in and inout parameters defined in the operation signature."

**Repo:** `crates/rpc/src/codegen.rs::method_in_members`;
`crates/lang-rules/src/emit/interface_cpp.rs` (`request_fields`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_in_params_become_case_members`,
`crates/rpc/src/codegen.rs::tests::enhanced_inout_param_is_in_both_structures`
(die Enhanced-Abbildung ruft dieselbe Funktion `method_in_members` auf),
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`
(`swap(inout long v)` über die erzeugte Rust-Bindung)

**Status:** `done` — Commit `b5224bd70dbb`.

### §7.5.1.1.4 Regel 3a: Reihenfolge der Parameter

**Spec:** §7.5.1.1.4, S. 32 — "a. The members shall appear in the same order as
the parameters in the operation, starting from the left."

**Repo:** `crates/rpc/src/codegen.rs::method_in_members`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_in_params_become_case_members`
(Member `a`, dann `b`)

**Status:** `done` — Commit `b5224bd70dbb`.

### §7.5.1.1.4 Regel 3b: Namen der Parameter

**Spec:** §7.5.1.1.4, S. 32 — "b. The names of the members shall be the same as
the formal parameter names."

**Repo:** `crates/rpc/src/codegen.rs::method_in_members`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_in_params_become_case_members`

**Status:** `done` — Commit `b5224bd70dbb`.

### §7.5.1.1.4 Regel 3c: dummy vom Typ UnusedMember

**Spec:** §7.5.1.1.4, S. 32 — "c. If an operation has no in and inout
parameters the resulting structure shall contain a single member of type
UnusedMember and name “dummy.”"

**Repo:** `crates/rpc/src/codegen.rs::method_in_members` (`DUMMY_MEMBER`);
`crates/lang-rules/src/emit/interface_cpp.rs` (`request_fields`)

**Tests:** `crates/rpc/src/codegen.rs::tests::an_empty_in_or_out_carries_the_dummy_member`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Out-Struktur und Result-Union je Operation

**Spec:** §7.5.1.1.5, S. 33 — "Each operation in the interface shall map to an
Out structure and a Result union."

**Repo:** `crates/rpc/src/codegen.rs` (`method_out_members`,
`build_result_union`); `crates/lang-rules/src/emit/interface_cpp.rs`
(`reply_fields`, `result_union_of`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`,
`crates/rpc/src/codegen.rs::tests::basic_reply_excludes_oneway_methods`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — Commits `9467d977720c`, `d047dc5f497c`. Eine
`oneway`-Operation, die die Grammatik von §7.3.1.1 nicht kennt (`lower_service`
weist sie ab), erhält weder Out-Struktur noch Result-Union und keinen Fall in der
Return-Union (Item zu Produktion (87)).

### §7.5.1.1.5 Out Regel 1: Name ${interfaceName}_${operationName}_Out

**Spec:** §7.5.1.1.5, S. 33 — "1. The name of the Out structure shall be
“${interfaceName}_${operationName}_Out.”"

**Repo:** `crates/rpc/src/codegen.rs::build_result_union` (`out_type_name`);
`crates/lang-rules/src/adapter/interface.rs::out_struct_name`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`,
`crates/rpc/tests/language_binding_agreement.rs::both_readings_of_seven_five_produce_the_same_sample_structure`

**Status:** `done` — Commits `9467d977720c`, `d047dc5f497c`.

### §7.5.1.1.5 Out Regel 2: im Modul des Interface

**Spec:** §7.5.1.1.5, S. 33 — "2. The Out structure shall be defined within the
same module as the original interface."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/lang-rules/tests/interface_bindings.rs::the_topic_types_and_constants_stand_in_the_module_of_the_interface`

**Status:** `done` — Commits `9467d977720c`, `d047dc5f497c`, `272968665473`. Der Code übergibt das Modul des Interface. Der Test legt zwei Module
an, dazu eine Exception in einem dritten, und prüft für C++, dass die Deklaration
im Namespace ihres Interface steht und weder im globalen Namespace noch im
Namespace des anderen Moduls; die Rust-Bindung legt die Items im Modul ihres
Interface ab. Die Lage der Java-Klassen ist nicht geprüft; die Rust-Bindung hat
keine `_Hash`-Konstanten.

### §7.5.1.1.5 Out Regel 3: out- und inout-Parameter, dazu return_

**Spec:** §7.5.1.1.5, S. 33 — "3. The Out structure shall contain as members the
out and inout parameters defined in the operation signature. In addition it may
contain a member named “return_.” a. The Out structure shall include one member
for each out and inout parameter in the operation signature."

**Repo:** `crates/rpc/src/codegen.rs::method_out_members`;
`crates/lang-rules/src/emit/interface_cpp.rs` (`reply_fields`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_return_value_is_the_last_member`,
`crates/rpc/src/codegen.rs::tests::enhanced_inout_param_is_in_both_structures`
(die Enhanced-Abbildung ruft dieselbe Funktion `method_out_members` auf),
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Out Regel 3a.i: Reihenfolge der Parameter

**Spec:** §7.5.1.1.5, S. 33 — "i. The members shall appear in the same order as
the parameters in the operation, starting from the left."

**Repo:** `crates/rpc/src/codegen.rs::method_out_members`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_return_value_is_the_last_member`,
`crates/rpc/src/codegen.rs::tests::basic_out_members_keep_the_order_of_the_parameters`,
`crates/lang-rules/tests/interface_bindings.rs::the_out_structure_keeps_the_order_of_the_parameters`

**Status:** `done` — Commits `9467d977720c`, `dfd8c6d7f0ec`, `272968665473`. Der Code hält die
Reihenfolge der Deklaration. Der erste Test hat mehrere `out`- und
`inout`-Parameter; der zweite prüft mit `mix(in long a, inout long b, out double c)`
und `mix2(out double c, in long a, inout long b)` die Reihenfolge der Member in
C++ und Java (`return_` zuletzt, `in`-Parameter nicht enthalten). Für die
Rust-Bindung gibt es keinen eigenen Test der Reihenfolge mehrerer Member.

### §7.5.1.1.5 Out Regel 3a.i — mehrere Parameter

**Spec:** §7.5.1.1.5, S. 33 — wie oben.

**Repo:** `crates/rpc/src/codegen.rs::method_out_members`;
`crates/lang-rules/src/emit/interface_cpp.rs` (`reply_fields`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_out_members_keep_the_order_of_the_parameters`,
`crates/lang-rules/tests/interface_bindings.rs::the_out_structure_keeps_the_order_of_the_parameters`

**Status:** `done` — Commits `dfd8c6d7f0ec`, `272968665473`. Die Tests prüfen die
Reihenfolge mehrerer `out`- und `inout`-Parameter in der Out-Struktur (siehe
Item „Out Regel 3a.i: Reihenfolge der Parameter“).

### §7.5.1.1.5 Out Regel 3a.ii: Namen der Parameter

**Spec:** §7.5.1.1.5, S. 33 — "ii. The name of the members shall be the same as
the formal parameter names."

**Repo:** `crates/rpc/src/codegen.rs::method_out_members`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_return_value_is_the_last_member`
(Member `wrapped`)

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Out Regel 3b: return_ als letzter Member

**Spec:** §7.5.1.1.5, S. 33 — "b. If the operation defines a non-void return
value the Out structure shall have its last member named “return_.” The type of
the “return_” member shall be the return type of the operation. In the case
where the function does not define a return value this member shall not be
present."

**Repo:** `crates/rpc/src/codegen.rs::method_out_members` (`RETURN_MEMBER`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_return_value_is_the_last_member`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Out Regel 3b: return_N bei Kollision

**Spec:** §7.5.1.1.5, S. 33 — "If an operation has an out argument named
return_ and the operation has a regular return value, the regular return value
in the Out structure shall be represented as return_N member of the appropriate
type where N is the first integer in the range 1 to 2^31 that avoids the
collision."

**Repo:** `crates/rpc/src/codegen.rs::return_member_name`;
`crates/lang-rules/src/emit/interface_cpp.rs::return_member_name`,
`crates/lang-rules/src/emit/interface_java.rs::return_member_name`,
`crates/lang-rules/src/emit/interface_rust.rs::return_member_name`

**Tests:** `crates/rpc/src/codegen.rs::tests::a_colliding_return_member_takes_the_first_free_number`,
`crates/lang-rules/tests/interface_bindings.rs::a_colliding_return_member_is_numbered_in_cpp_and_java`,
`crates/lang-rules/tests/interface_bindings.rs::a_colliding_return_member_is_numbered_in_rust`

**Status:** `done` — Commits `9467d977720c`, `272968665473`. Der erste Test deckt
`return_member_name` in `crates/rpc`; die beiden anderen prüfen mit
`long f(out long return_)` die Member `return_` und `return_1` in dieser
Reihenfolge in C++, Java und Rust.

### §7.5.1.1.5 Out Regel 3b: return_N — Generatoren

**Spec:** §7.5.1.1.5, S. 33 — wie oben.

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::return_member_name`,
`crates/lang-rules/src/emit/interface_java.rs::return_member_name`,
`crates/lang-rules/src/emit/interface_rust.rs::return_member_name`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::a_colliding_return_member_is_numbered_in_cpp_and_java`,
`crates/lang-rules/tests/interface_bindings.rs::a_colliding_return_member_is_numbered_in_rust`

**Status:** `done` — Commit `272968665473`. Die Tests erwarten `return_1` in
`Odd_f_Out` für `long f(out long return_)` in C++ und Java (Text der
Deklaration) und in Rust (Felder des Items).

### §7.5.1.1.5 Out Regel 3c: dummy ohne Rückgabe und ohne out

**Spec:** §7.5.1.1.5, S. 33 — "c. If the operation has no return value, no
out/inout parameters, the Out structure shall contain a single member named
“dummy” of type dds::rpc::UnusedMember."

**Repo:** `crates/rpc/src/codegen.rs::method_out_members`;
`crates/lang-rules/src/emit/interface_cpp.rs` (`reply_fields`)

**Tests:** `crates/rpc/src/codegen.rs::tests::an_empty_in_or_out_carries_the_dummy_member`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 1: Name ${interfaceName}_${operationName}_Result

**Spec:** §7.5.1.1.5, S. 34 — "1. The Result union name shall be
“${interfaceName}}_${operationName}_Result.”"

**Repo:** `crates/rpc/src/codegen.rs::build_result_union`;
`crates/lang-rules/src/adapter/interface.rs::result_union_name`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`

**Status:** `done` — Commits `9467d977720c`, `d047dc5f497c`.

### §7.5.1.1.5 Result Regel 2: Diskriminator long

**Spec:** §7.5.1.1.5, S. 34 — "2. The Result union discriminator type shall be
long."

**Repo:** `crates/rpc/src/codegen.rs::ExceptionCase::discriminator` (`i32`);
`crates/lang-rules/src/emit/interface_cpp.rs::discriminator_field` (`Int32`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_exception_label_hashes_the_scoped_name`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 3: Fall dds::RETCODE_OK

**Spec:** §7.5.1.1.5, S. 34 — "3. The Result union shall have a case with label
dds::RETCODE_OK which is used to represent a successful return."

**Repo:** `crates/rpc/src/codegen.rs::RETCODE_OK`;
`crates/lang-rules/src/emit/interface_cpp.rs::result_union_of`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`
(`RETCODE_OK == 0`)

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 3a: Member result vom Typ der Out-Struktur

**Spec:** §7.5.1.1.5, S. 34 — "a. This case label shall contain a single member
with name “result” and type ${interfaceName}_${operationName}_Out."

**Repo:** `crates/rpc/src/codegen.rs::RESULT_MEMBER`;
`crates/lang-rules/src/emit/interface_cpp.rs::result_union_of`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 4a: Konstante ${exceptionType}_Ex_Hash

**Spec:** §7.5.1.1.5, S. 34 — "a. A constant of type long and with name
“${exceptionType}_Ex_Hash” shall be available in the same namespace as the
interface is defined in."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (Konstante mit
`service.module`); `crates/lang-rules/src/adapter/interface.rs::exception_hash_constant`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`,
`crates/lang-rules/tests/interface_bindings.rs::the_topic_types_and_constants_stand_in_the_module_of_the_interface`

**Status:** `done` — Commits `9467d977720c`, `272968665473`. Name, Typ (`std::int32_t`) und Vorkommen der Konstante `Overflow_Ex_Hash` sind getestet. Der Test legt zwei Module
an, dazu eine Exception in einem dritten, und prüft für C++, dass die Deklaration
im Namespace ihres Interface steht und weder im globalen Namespace noch im
Namespace des anderen Moduls; die Rust-Bindung legt die Items im Modul ihres
Interface ab. Die Lage der Java-Klassen ist nicht geprüft; die Rust-Bindung hat
keine `_Hash`-Konstanten.

### §7.5.1.1.5 Result Regel 4b: HASH des voll qualifizierten Namens

**Spec:** §7.5.1.1.5, S. 34 — "b. The value of the constant shall be the HASH
of the fully qualified name of the exception type where module separator to be
used is “::” (2 colons)."

**Repo:** `crates/rpc/src/service_mapping.rs::scoped_exception_name`;
`crates/lang-rules/src/adapter/interface.rs::exception_hash`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_exception_label_hashes_the_scoped_name`
(MD5 von `robot::Overflow`),
`crates/rpc/tests/language_binding_agreement.rs::both_readings_of_seven_five_produce_the_same_sample_structure`
(`exception_hash` im Generator gleich dem Wert der Laufzeit)

**Status:** `done` — Commit `9467d977720c`. Der Name wird ohne führendes `::`
gebildet (`robot::Overflow`); das Beispiel der Spec auf S. 34 hasht `"TooFast"`
ohne Modul.

### §7.5.1.1.5 Result Regel 5: ein Fall je Exception

**Spec:** §7.5.1.1.5, S. 34 — "5. The union shall have a case label for each
exception declared as a possible outcome of the operation."

**Repo:** `crates/rpc/src/codegen.rs::build_result_union`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`,
`crates/rpc/tests/language_binding_agreement.rs::both_readings_of_seven_five_produce_the_same_sample_structure`
(Anzahl der Fälle gleich `raises.len()`)

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 5a: Label ${exceptionType}_Ex_Hash

**Spec:** §7.5.1.1.5, S. 34 — "a. The integral value of the case label shall be
${exceptionType}_Ex_Hash."

**Repo:** `crates/rpc/src/codegen.rs::ExceptionCase` (`discriminator`,
`hash_constant`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_exception_label_hashes_the_scoped_name`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 5b: Member <exception>_ex in Kleinschreibung

**Spec:** §7.5.1.1.5, S. 34 — "b. The case label shall contain a single member
with name synthetized as the lower-case of the exception name with the suffix
“_ex” added."

**Repo:** `crates/rpc/src/codegen.rs::build_result_union` (`member`);
`crates/lang-rules/src/adapter/interface.rs::exception_member_name`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_result_union_of_seven_five_one_one_five_is_built`
(`toofast_ex`)

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.5 Result Regel 5c: Typ der Exception

**Spec:** §7.5.1.1.5, S. 34 — "c. The type associated with the case member
shall be the exception type."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::result_union_of`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`
(`Overflow overflow_ex;`)

**Status:** `done` — Commits `9467d977720c`, `d047dc5f497c`.

### §7.5.1.1.4 und §7.5.1.1.5 Beispiele

**Spec:** §7.5.1.1.4–§7.5.1.1.5, S. 32–35 — Beispiele für `RobotControl`
("Non-normative Example").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiele.

### §7.5.1.1.6 Call Regel 1: ${interfaceName}_Call im Modul des Interface

**Spec:** §7.5.1.1.6, S. 35 — "Each interface shall map to a Call union and a
Request structure. [...] 1. The Call union name shall be “${interfaceName}_Call”
in the same module as the interface."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union`;
`crates/lang-rules/src/adapter/interface.rs::call_union_name`;
`crates/lang-rules/src/emit/interface_cpp.rs::declare`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/lang-rules/tests/interface_bindings.rs::the_topic_types_and_constants_stand_in_the_module_of_the_interface`

**Status:** `done` — Commits `b5224bd70dbb`, `d047dc5f497c`, `272968665473`. Der Name `Calculator_Call` ist getestet. Der Test legt zwei Module
an, dazu eine Exception in einem dritten, und prüft für C++, dass die Deklaration
im Namespace ihres Interface steht und weder im globalen Namespace noch im
Namespace des anderen Moduls; die Rust-Bindung legt die Items im Modul ihres
Interface ab. Die Lage der Java-Klassen ist nicht geprüft; die Rust-Bindung hat
keine `_Hash`-Konstanten.

### §7.5.1.1.6 Call Regel 2: Diskriminator long

**Spec:** §7.5.1.1.6, S. 35 — "2. The Call union discriminator type shall be of
type long."

**Repo:** `crates/rpc/src/codegen.rs::CallUnionCase::discriminator` (`i32`);
`crates/lang-rules/src/emit/interface_cpp.rs::discriminator_field` (`Int32`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_label_is_the_published_hash_arithmetic`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.6 Call Regel 3: default mit unknownOp

**Spec:** §7.5.1.1.6, S. 35 — "3. The Call union shall have a default case
label containing a member named “unknownOp” of type
dds::rpc::UnknownOperation."

**Repo:** `crates/rpc/src/codegen.rs::UNKNOWN_OP_MEMBER`;
`crates/lang-rules/src/emit/interface_cpp.rs::union_of`

**Tests:** `crates/rpc/src/codegen.rs::tests::both_unions_carry_the_default_unknown_operation_case`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done` — Commit `9467d977720c`.

### §7.5.1.1.6 Call Regel 4: Konstante ${interfaceName}_${operationName}_Hash

**Spec:** §7.5.1.1.6, S. 35 — "4. For each operation in the interface, an
integral constant of type long and name “$(interfaceName}_$ {operationName}_Hash”
shall be present in the same module as the interface. The value of this
constant is the HASH of unqualified ${operationName}."

**Repo:** `crates/rpc/src/codegen.rs::CallUnionCase::hash_constant`;
`crates/lang-rules/src/emit/interface_cpp.rs::declare`
(`hash_constant`, `operation_hash`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/lang-rules/tests/interface_bindings.rs::the_topic_types_and_constants_stand_in_the_module_of_the_interface`

**Status:** `done` — Commits `9467d977720c`, `272968665473`. Name, Typ (`std::int32_t`) und Wert (HASH des unqualifizierten Namens) sind getestet. Der Test legt zwei Module
an, dazu eine Exception in einem dritten, und prüft für C++, dass die Deklaration
im Namespace ihres Interface steht und weder im globalen Namespace noch im
Namespace des anderen Moduls; die Rust-Bindung legt die Items im Modul ihres
Interface ab. Die Lage der Java-Klassen ist nicht geprüft; die Rust-Bindung hat
keine `_Hash`-Konstanten.

### §7.5.1.1.6 Call Regel 5: ein Fall je Operation

**Spec:** §7.5.1.1.6, S. 35 — "5. The Call union shall have a case label for
each operation in the interface."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (über
`ServiceDef::all_methods`, also die impliziten Operationen der Attribute
eingeschlossen)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`

**Status:** `done` — Commits `b5224bd70dbb`, `fb1b6a402a94`.

### §7.5.1.1.6 Call Regel 5a: Label HASH des Namens der Operation

**Spec:** §7.5.1.1.6, S. 35 — "a. The integral value of the case label shall be
the HASH of ${operationName}."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union`
(`rpc_member_hash(&m.name)`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_label_is_the_published_hash_arithmetic`,
`crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`

**Status:** `done` — Commit `9467d977720c`.
### §7.5.1.1.6 Call Regel 5b: Member mit dem Namen der Operation

**Spec:** §7.5.1.1.6, S. 35 — "b. The member name for the case label shall be
the operation name."

**Repo:** `crates/rpc/src/codegen.rs::CallUnionCase::method`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`

**Status:** `done` — Commit `b5224bd70dbb`.

### §7.5.1.1.6 Call Regel 5c: Typ ${interfaceName}_${operationName}_In

**Spec:** §7.5.1.1.6, S. 35 — "c. The type for the case label member shall be
${interfaceName}_${operationName}_In as defined in sub clause 7.5.1.1.2 The
Hashing Algorithm."

**Repo:** `crates/rpc/src/codegen.rs::CallUnionCase::case_type_name`

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`

**Status:** `done` — Commit `b5224bd70dbb`.

### §7.5.1.1.4 bis §7.5.1.1.6 Lage der Typen und Konstanten im Modul — ohne Test

**Spec:** §7.5.1.1.4 Regel 2, §7.5.1.1.5 Out Regel 2 und Result Regel 4a,
§7.5.1.1.6 Regel 1 und 4 — wie oben.

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (Namespace je Modul des Interface);
`crates/lang-rules/src/emit/interface_rust.rs` (Rust-Modul je Modul des Interface)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::the_topic_types_and_constants_stand_in_the_module_of_the_interface`

**Status:** `done` — der Test hat zwei Module `a` und `b` und eine Exception in einem dritten Modul. Er prüft für C++, dass `Svc_f_In`, `Svc_f_Out`, `Svc_f_Result`, `Svc_Call`, `Svc_Return`, `Svc_Request`, `Svc_Reply`, `Svc_f_Hash` und `Bad_Ex_Hash` genau im Namespace `a` stehen und `Other_g_In`, `Other_Call`, `Other_g_Hash` genau im Namespace `b`. Für Rust prüft er die Lage der Strukturen und Aufzählungen im Modul des Interface; Rust erzeugt keine `_Hash`-Konstanten, sie sind dort nicht geprüft. Commit `272968665473`.
### §7.5.1.1.6 Request Regel 1: ${interfaceName}_Request im Modul

**Spec:** §7.5.1.1.6, S. 36 — "1. The name of Request structure shall be
“${interfaceName}_Request” in the same module as the interface."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`);
`crates/lang-rules/src/adapter/interface.rs::request_type_name` (Commit `d047dc5f497c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_pair_topic_names`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.6 Request Regel 2: zwei Member

**Spec:** §7.5.1.1.6, S. 36 — "2. The Request structure shall have two
members".

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`

**Status:** `done`

### §7.5.1.1.6 Request Regel 2a: header vom Typ RequestHeader

**Spec:** §7.5.1.1.6, S. 36 — "a. The first member of the Request structure
shall be named “header” and be of type dds::rpc::RequestHeader."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`

**Status:** `done`

### §7.5.1.1.6 Request Regel 2b: data vom Typ ${interfaceName}_Call

**Spec:** §7.5.1.1.6, S. 36 — "b. The second member of the Request structure
shall be named “data” and be of type ${interfaceName}_Call."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_request_has_header_and_call_union`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.6 Beispiele

**Spec:** §7.5.1.1.6, S. 36–37 — `RobotControl_Call` und `RobotControl_Request`
("Non-normative Example").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiele.

### §7.5.1.1.7 Return Regel 1: ${interfaceName}_Return im Modul

**Spec:** §7.5.1.1.7, S. 37 — "Each interface shall map to a Return union and a
Reply structure. [...] 1. The Return union name shall be
“${interfaceName}_Return” in the same module as the interface."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (mit `reply = true`; Commit `9467d977720c`);
`crates/lang-rules/src/adapter/interface.rs::return_union_name` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_excludes_oneway_methods`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.7 Return Regel 2: Diskriminator long

**Spec:** §7.5.1.1.7, S. 37 — "2. The Return union discriminator type shall be
of type long."

**Repo:** `crates/rpc/src/codegen.rs::CallUnionCase::discriminator` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`

**Status:** `done`

### §7.5.1.1.7 Return Regel 3: default mit unknownOp

**Spec:** §7.5.1.1.7, S. 37 — "3. The Return union shall have a default case
label containing a member named “unknownOp” of type
dds::rpc::UnknownOperation."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (`default_member`, `UNKNOWN_OP_MEMBER`; Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::both_unions_carry_the_default_unknown_operation_case`

**Status:** `done`

### §7.5.1.1.7 Return Regel 4a: Konstante ${interfaceName}_${operationName}_Hash

**Spec:** §7.5.1.1.7, S. 37 — "a. A constant of type long and with name
“${interfaceName}_${operationName}_Hash” shall be available in the same
namespace as the interface is defined in."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (`hash_constant`, eine Konstante je Operation für beide Unions; Commit `9467d977720c`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.7 Return Regel 4b: HASH des unqualifizierten Namens

**Spec:** §7.5.1.1.7, S. 37 — "b. The value of the constant shall be the HASH
of the name of the operation (not qualified)."

**Repo:** `crates/lang-rules/src/adapter/interface.rs::operation_hash` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`

**Status:** `done`

### §7.5.1.1.7 Return Regel 5: ein Fall je Operation

**Spec:** §7.5.1.1.7, S. 37 — "5. The Return union shall have a case label for
each operation in the interface".

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_excludes_oneway_methods`

**Status:** `done` — `oneway`-Operationen erhalten keinen Fall (Item zu Produktion (87) in §7.3.1.1).

### §7.5.1.1.7 Return Regel 5a: Label ${interfaceName}_${operationName}_Hash

**Spec:** §7.5.1.1.7, S. 37 — "a. The integral value of the case label shall be
${interfaceName}_${operationName}_Hash as computed using the HASH algorithm of
the unqualified name of the operation."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`

**Status:** `done`

### §7.5.1.1.7 Return Regel 5b: Member und Typ ${interfaceName}_${operationName}_Result

**Spec:** §7.5.1.1.7, S. 37 — "b. The member name for the case label shall be
the operation name and the type shall be ${interfaceName}_${operationName}_Result."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (`case_type_name`; Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_excludes_oneway_methods`

**Status:** `done`

### §7.5.1.1.7 Reply Regel 1: ${interfaceName}_Reply

**Spec:** §7.5.1.1.7, S. 38 — "1. The Reply structure name shall be
“${interfaceName}_Reply,”."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`);
`crates/lang-rules/src/adapter/interface.rs::reply_type_name` (Commit `d047dc5f497c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_pair_topic_names`

**Status:** `done`

### §7.5.1.1.7 Reply Regel 2: im Modul des Interface

**Spec:** §7.5.1.1.7, S. 38 — "2. The Reply structure shall be defined within
the same module as the original interface."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (Commit `d047dc5f497c`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.7 Reply Regel 3: zwei Member

**Spec:** §7.5.1.1.7, S. 38 — "3. The Reply structure shall have two members".

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_excludes_oneway_methods`

**Status:** `done`

### §7.5.1.1.7 Reply Regel 3a: header vom Typ ReplyHeader

**Spec:** §7.5.1.1.7, S. 38 — "a. The first member of the Reply structure shall
be named “header” and be of type dds::rpc::ReplyHeader."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`)

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.7 Reply Regel 3b: data vom Typ ${interfaceName}_Return

**Spec:** §7.5.1.1.7, S. 38 — "b. The second member of the Reply type shall be
named “data” and be of type ${interfaceName}_Return."

**Repo:** `crates/rpc/src/codegen.rs::build_basic_pair` (Commit `9467d977720c`)

**Tests:** `crates/rpc/src/codegen.rs::tests::basic_reply_excludes_oneway_methods`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`

**Status:** `done`

### §7.5.1.1.7 Beispiele

**Spec:** §7.5.1.1.7, S. 37–38 — `RobotControl_Return` und `RobotControl_Reply`
("Non-normative Example").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiele. Das Beispiel nennt den zweiten
Member von `RobotControl_Reply` `reply`, Regel 3b `data`.

### §7.5.1.1.8 Vererbung ändert die Typen nicht

**Spec:** §7.5.1.1.8, S. 38 — "Inheritance has no effect on the generated
structures and unions. [...] The Request and Reply types for a derived
interface includes operations defined only in the derived interface."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare` (Topic-Typen aus `service.all_operations()`, nur die eigenen Operationen; Commit `d047dc5f497c`);
`crates/lang-rules/src/emit/interface_rust.rs::declare` (Commit `638ba5279072`)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::a_derived_client_reaches_an_inherited_operation_through_its_base_field` (Commit `15c8509f56b1`)

**Status:** `done`

### §7.5.1.1.8 N Topics, N DataWriter und N DataReader

**Spec:** §7.5.1.1.8, S. 38 — "A DDS service that implements a derived
interface uses two topics for every interface in the hierarchy. As a result, a
service implementing a hierarchy of N interfaces, shall have N request topics
and N reply topics. Consequently, it will necessitate N DataWriters and N
DataReaders. The types and the topic names are obtained as specified in 7.4.2
Basic Service Mapping."

**Repo:** `crates/lang-rules/src/emit/interface_rust.rs::client_impl` (ein `Requester` je Ebene); `crates/lang-rules/src/emit/interface_rust.rs` (ein Dispatcher je Ebene, jeder mit eigenem `Replier`; Commit `638ba5279072`)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::a_derived_client_reaches_an_inherited_operation_through_its_base_field` (Commit `15c8509f56b1`)

**Status:** `done`

### §7.5.1.1.8 Beispiel Calculator

**Spec:** §7.5.1.1.8, S. 38–43 — Basic-Abbildung von `Calculator : Adder,
Subtractor` ("Non-normative Example").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiel. Es weicht von den Regeln ab: die
Result-Unions haben `default: dds::rpc::UnknownException unknownEx` und das
Label `dds::rpc::REMOTE_EX_OK` (Regel 3 nennt `dds::RETCODE_OK`),
`Calculator_off_Result` ist als `struct ... switch (long)` geschrieben, und
`Calculator_Reply` hat einen `header` vom Typ `RequestHeader`.

### §7.5.1.2 Enhanced-Abbildung

**Spec:** §7.5.1.2, S. 43 — "To accurately capture the semantics of the method
call invocation and return, this specification defines additional built-in
annotations. [...] The Enhanced Service Mapping uses the [DDS-XTypes] type
system in addition to the annotations defined in 7.5.1.2.1".

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` und `build_enhanced_service_with` (Commit `60dbad781c53`);
Topic-Typen und Codecs in `crates/lang-rules/src/emit/interface_rust/enhanced.rs` (Commit `076900132ad5`), `crates/lang-rules/src/emit/interface_cpp/enhanced.rs`, `crates/lang-rules/src/emit/interface_cpp/enhanced_codec.rs` (Commit `99bda16e455b`), `crates/lang-rules/src/emit/interface_java/enhanced.rs`, `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs` (Commit `9ab3b695e13c`);
Auswahl über `zerodds-idlc generate --rpc-mapping basic|enhanced|enhanced-empty` (Commit `f3a2d2137b4a`);
Laufzeit `zerodds_rpc::endpoint_options::RpcMapping::Enhanced` (Commit `3d4c42cad194`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_request_is_a_choice_over_the_operations`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`

**Status:** `done` — Request-, Reply-, In-, Out- und Result-Strukturen mit `@Choice`, `@Autoid`, `remoteEx` und flacher Vererbung entstehen im Modell und in den Generatoren für Rust, C++ und Java; die Rust-Laufzeit korreliert über `PID_RELATED_SAMPLE_IDENTITY`.

### §7.5.1.2.1 Annotationen im Modul dds::rpc

**Spec:** §7.5.1.2.1, S. 43 — "All annotations below are defined in the
dds::rpc module."

**Repo:** `crates/ast/src/node/annotation.rs` (`BUILTIN_ANNOTATIONS`: `Choice`, `Autoid`, `Empty`, `Mutable`; Commit `3c21f462c6e2`);
`crates/idl-lower/src/annotation.rs::Lowering::apply_user_annotation` (Commit `20ca736326e4`)

**Tests:** `crates/ast/tests/node_space.rs::every_builtin_annotation_of_the_spec_list_is_present`,
`crates/idl-lower/tests/annotations.rs::the_dds_rpc_annotations_resolve_to_their_builtin_declarations`,
`crates/idl-lower/tests/annotations.rs::the_qualified_dds_rpc_spelling_is_the_same_annotation`

**Status:** `done` — die Annotationen des Moduls sind vordefiniert und lösen unter ihrem einfachen Namen (`@Choice`) und in der qualifizierten Schreibung (`@dds::rpc::Choice`, `@dds::rpc::Autoid`) auf dieselbe Deklaration auf (Test `crates/idl-lower/tests/annotations.rs::the_qualified_dds_rpc_spelling_is_the_same_annotation`, Commit `ff347687da13`). Das Modell hält die Deklarationen im eingebauten Modul `@builtin`; ein Modul `dds::rpc` in `crates/ast` würde die festen Indizes der eingebauten Knoten verschieben, und die Spec verlangt den Namen, nicht eine Modulstruktur des Modells. Eine eingebaute Annotation hat keine `TypeObject` und erscheint nicht als `ann_custom`; der Modulname ist in keiner Ausgabe sichtbar (kein `TypeObject` und keine erzeugte Datei nennt die Deklaration; nur `scoped_name_of` liefert intern `@builtin::Choice`; aus dem Code gelesen, nicht gemessen) (`61c11da59a10`, Test `crates/lang-rules/tests/enhanced_bindings.rs::a_choice_structure_written_in_idl_has_a_mutable_type_object_with_optional_members`).

### §7.5.1.2.1.1 @Choice: Deklaration

**Spec:** §7.5.1.2.1.1, S. 43 — "The @Choice annotation is a placeholder
annotation defined as follows. @Annotation local interface Choice { };"

**Repo:** `crates/ast/src/node/annotation.rs` (`BUILTIN_ANNOTATIONS`, Eintrag `Choice`; Commit `3c21f462c6e2`)

**Tests:** `crates/ast/tests/node_space.rs::every_builtin_annotation_of_the_spec_list_is_present`

**Status:** `done` — `Choice` ist eine vordefinierte Annotation ohne Member.

### §7.5.1.2.1.1 @Choice nur an Strukturen

**Spec:** §7.5.1.2.1.1, S. 43 — "The @Choice annotation is allowed on
structures only."

**Repo:** `crates/idl-lower/src/annotation.rs::Lowering::misplaced` (Eintrag für `Choice`; Commit `ff347687da13`)

**Tests:** `crates/idl-lower/tests/annotations.rs::choice_is_refused_on_anything_but_a_structure`

**Status:** `done` — `@Choice` an einer Union, einem Member, einem Interface und einer Aufzählung wird mit einem Fehler abgelehnt; an einer Struktur wird es angenommen.

### §7.5.1.2.1.1 @Choice: MUTABLE und alle Member @Optional

**Spec:** §7.5.1.2.1.1, S. 43 — "When present, the structure shall be
interpreted as if the structure had the @Extensibility(MUTABLE_EXTENSIBILITY)
annotation and all the members of the structure had the @Optional annotation."

**Repo:** `crates/rpc/src/codegen.rs::autoid_choice`;
`crates/lang-rules/src/emit/interface_rust/enhanced.rs` (`choice_codec`: DHEADER und ein EMHEADER je vorhandenem Member), `crates/lang-rules/src/emit/interface_cpp/enhanced.rs` (`std::optional`-Member), `crates/lang-rules/src/emit/interface_cpp/enhanced_codec.rs`, `crates/lang-rules/src/emit/interface_java/enhanced.rs`, `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs` (Commits `076900132ad5`, `99bda16e455b`, `9ab3b695e13c`)

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_request_is_a_mutable_structure_with_one_member`,
`crates/lang-rules/tests/enhanced_cpp_calls.rs::the_codec_writes_the_member_ids_and_length_codes_of_the_clause`,
`crates/lang-rules/tests/enhanced_java_calls.rs::the_codec_writes_the_member_ids_and_length_codes_of_the_clause`,
`crates/idl-lower/tests/annotations.rs::a_choice_structure_is_mutable_with_every_member_optional`,
`crates/idl-lower/tests/annotations.rs::a_choice_structure_cannot_be_final_or_appendable`,
`crates/lang-rules/tests/enhanced_bindings.rs::a_choice_structure_written_in_idl_has_a_mutable_type_object_with_optional_members`

**Status:** `done` — die vom Generator erzeugten Strukturen (`Request`, `Reply`, `Result`) sind MUTABLE mit optionalen Membern. Eine im IDL geschriebene `@Choice`-Struktur setzt `crates/idl-lower` auf MUTABLE und alle Member auf `@optional`; ihr TypeObject ist MUTABLE mit optionalen Membern. Eine ausdrücklich `@final` oder `@appendable` erklärte `@Choice`-Struktur ist ein Fehler. Commit `ff347687da13`.

### §7.5.1.2.1.1 @Choice: genau ein Member vorhanden

**Spec:** §7.5.1.2.1.1, S. 43 — "Furthermore, exactly one member shall be
present at any given time."

**Repo:** `crates/lang-rules/src/emit/interface_rust/enhanced.rs` (`enum` mit einer Variante je Member); `crates/lang-rules/src/emit/interface_cpp/enhanced_codec.rs` und `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs` (Zählung der gesetzten Member beim Schreiben, Fehler bei einer Zahl ungleich 1; Commits `076900132ad5`, `99bda16e455b`, `9ab3b695e13c`)

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_request_is_a_mutable_structure_with_one_member`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_request_with_two_active_members_is_refused`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_choice_without_a_member_is_refused`,
`crates/lang-rules/tests/enhanced_java_calls.rs::a_choice_holds_exactly_one_member_on_both_sides_of_the_codec`,
`crates/rpc/tests/java_enhanced_codec.rs::the_generated_java_service_and_client_answer_the_error_cases_of_the_clauses`

**Status:** `done` — in Rust ist durch die `enum`-Form genau ein Member vorhanden; der Decoder lehnt null oder zwei Member ab (Tests; Commit `8891b6d7f067`). In Java lehnen Encoder und Decoder null und mehr als einen Member ab, die erzeugte Laufzeit antwortet mit `RemoteInvalidArgumentError` (Tests; Commit `2051ce1a2a87`). In C++ zählt der Codec die gesetzten Member beim Schreiben und wirft `dds::rpc::UnsupportedError` bei einer Zahl ungleich 1; der Decoder lehnt null und zwei Member ab (`576ea3df00b1`). Test: `crates/rpc/tests/cpp/enhanced_function_call_roundtrip.cpp` über `crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service` (Oktette ohne Member und mit zwei Membern werden abgewiesen und auf der Leitung mit `REMOTE_EX_INVALID_ARGUMENT` beantwortet; `c398e1587cf7`, auf dem Linux-Testhost gelaufen).

### §7.5.1.2.1.2 @Autoid an Strukturen und Membern

**Spec:** §7.5.1.2.1.2, S. 43 — "The @Autoid annotation shall be allowed on
structures and data members."

**Repo:** `crates/ast/src/node/annotation.rs` (`BUILTIN_ANNOTATIONS`, Eintrag `Autoid`; Commit `3c21f462c6e2`);
`crates/idl-lower/src/annotation.rs::enumerated_parameter` (liest das Argument `HASH`/`SEQUENTIAL`)

**Tests:** `crates/ast/tests/node_space.rs::every_builtin_annotation_of_the_spec_list_is_present`,
`crates/idl-lower/tests/annotations.rs::autoid_on_a_structure_assigns_hashed_member_ids`,
`crates/idl-lower/tests/annotations.rs::autoid_on_a_member_hashes_that_members_name`,
`crates/idl-lower/tests/annotations.rs::autoid_is_refused_where_there_are_no_member_ids`,
`crates/lang-rules/tests/enhanced_bindings.rs::an_autoid_structure_written_in_idl_has_hashed_member_ids_in_its_type_object`

**Status:** `done` — `Lowering::misplaced` lässt `@Autoid` an Strukturen und Strukturmembern zu und lehnt es an anderen Elementen ab. An einer im IDL geschriebenen Struktur setzt `Lowering::dds_rpc_autoid` für jeden Member die Member-ID als HASH des Membernamens (`hashed_member_id`); an einem einzelnen Member gilt der HASH seines Namens. Das TypeObject trägt diese IDs. Commit `ff347687da13`.

### §7.5.1.2.1.2 Synthetisierte Strukturen tragen @Autoid

**Spec:** §7.5.1.2.1.2, S. 43 — "The structures of the topic types synthesized
from the interface shall be annotated as @Autoid."

**Repo:** `crates/rpc/src/codegen.rs::autoid`; `crates/lang-rules/src/adapter/interface.rs::autoid_struct` (Commit `60dbad781c53`)

**Tests:** `crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done` — jede synthetisierte Struktur hat `autoid: true`; der Test vergleicht die Auszeichnung (`@Autoid` oder `@Choice @Autoid`) aller Strukturen des Beispiels `Calculator` mit der Liste aus §7.5.1.2.

### §7.5.1.2.1.2 Member-ID ist HASH des Namens

**Spec:** §7.5.1.2.1.2, S. 43 — "The @Autoid annotation indicates that the
member-id of each member shall be computed using the HASH algorithm (defined in
7.5.1.1.2 The Hashing Algorithm) applied to the name of the member."

**Repo:** `crates/rpc/src/codegen.rs::autoid` mit `MEMBER_ID_MASK` und `crate::rpc_hash::rpc_member_hash`;
`crates/lang-rules/src/adapter/interface.rs::member_id` (Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_autoid_member_ids_are_the_hash_of_the_name`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_member_ids_are_the_hash_of_the_names`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_request_is_a_mutable_structure_with_one_member`

**Status:** `done` — die Member-ID ist der HASH des Namens (§7.5.1.1.2), auf die unteren 28 Bit maskiert (`0x0FFF_FFFF`), weil ein EMHEADER nach XTypes 1.3 §7.4.3.4.2 eine Member-ID mit 28 Bit trägt. Der HASH selbst hat 32 Bit.

### §7.5.1.2.1.2 Kollisionen der Member-IDs als Fehler melden

**Spec:** §7.5.1.2.1.2, S. 44 — "The collisions of member-ids produced as a
result of @Autoid annotation must be detected. If a structure annotated as
@Autoid has a hash collision due to two or more member names map to the same
hash-code, the collision shall be reported as an error."

**Repo:** `crates/rpc/src/codegen.rs::autoid` (`RpcError::MemberIdCollision`; Commit `60dbad781c53`);
`crates/lang-rules/src/adapter/interface.rs::autoid_struct` (`BindingError::Codec`; Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_member_id_collision_is_an_error`

**Status:** `done` — `m9852` und `m12254` liefern dieselbe 28-Bit-ID und ergeben `MemberIdCollision`. Der Pfad des Generators (`autoid_struct` in `interface.rs`) prüft ebenso; er hat keinen eigenen Test.

### §7.5.1.2.1.3 @Empty erlaubt eine leere Struktur

**Spec:** §7.5.1.2.1.3, S. 44 — "The @Empty annotation allows a structure to be
empty. The code synthesized from an empty structure is implementation
dependent."

**Repo:** `crates/rpc/src/codegen.rs::EmptyOperationStyle::EmptyAnnotation` und `EnhancedOptions` (Commit `153b17a841b9`);
`zerodds-idlc generate --rpc-mapping enhanced-empty` (Commit `f3a2d2137b4a`);
Codecs für eine Struktur ohne Member in `crates/lang-rules/src/emit/interface_rust/enhanced.rs`, `crates/lang-rules/src/emit/interface_cpp/enhanced_codec.rs`, `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_empty_operation_can_be_an_empty_structure`,
`crates/lang-rules/tests/enhanced_empty_style.rs::the_empty_structure_reading_gives_it_no_member`,
`crates/rpc-rust-e2e/tests/enhanced_empty.rs::an_empty_structure_has_no_octets`

**Status:** `done` — eine Operation ohne zu tragende Werte kann als leere Struktur mit `@Empty` abgebildet werden; die Voreinstellung bleibt das Member `dummy` (§7.5.1.1.4 Regel 3c, Beispiel in §7.5.1.2.6).

### §7.5.1.2.1.4 @Mutable an Operationen

**Spec:** §7.5.1.2.1.4, S. 44 — "This annotation shall be allowed on interface
operations."

**Repo:** `crates/lang-rules/src/adapter/interface.rs` (`Operation::mutable` aus `applied_annotation(&op.facets, "Mutable")`);
`crates/rpc/src/service_mapping.rs` (`MethodDef::mutable`); Commits `3c21f462c6e2`, `20ca736326e4`

**Tests:** `crates/idl-lower/tests/annotations.rs::the_dds_rpc_annotations_resolve_to_their_builtin_declarations`,
`crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`

**Status:** `done` — `@Mutable` an einer Operation löst auf und wird gelesen.

### §7.5.1.2.1.4 @Mutable macht Call- und Return-Strukturen MUTABLE

**Spec:** §7.5.1.2.1.4, S. 44 — "The use of the @Mutable operation changes the
type declaration of the synthesized call and return structures so that they are
declared with the @Extensibility(MUTABLE_EXTENSIBILITY) annotation."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (`method.mutable` für `In`, `Out` und `Result`);
`crates/lang-rules/src/emit/interface_rust/enhanced.rs::mutable_struct_codec` (Commit `076900132ad5`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_mutable_operation_has_mutable_structures`,
`crates/lang-rules/tests/enhanced_cpp_calls.rs::the_codec_writes_the_member_ids_and_length_codes_of_the_clause`

**Status:** `done` — die Strukturen der Operation mit `@Mutable` sind MUTABLE, die der anderen Operationen nicht.

### §7.5.1.2.2 In-Struktur wie in der Basic-Abbildung

**Spec:** §7.5.1.2.2, S. 44 — "The mapping for interface operations is the same
as defined in 7.5.1.1.2 The Hashing Algorithm with the following additional
rules."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` mit `method_in_members` (Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_inout_param_is_in_both_structures`,
`crates/rpc/src/codegen.rs::tests::enhanced_empty_operation_has_a_dummy_member_by_default`

**Status:** `done` — die In-Struktur hat die Member der Basic-Abbildung (Parameter `in` und `inout`, sonst `dummy`).

### §7.5.1.2.2 @Mutable: In-Struktur MUTABLE

**Spec:** §7.5.1.2.2, S. 44 — "If the operation has the @Mutable annotation,
then the synthesized In structure shall have the annotation
@Extensibility(MUTABLE_EXTENSIBILITY)."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (`autoid_struct(..., method.mutable, ...)`; Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_mutable_operation_has_mutable_structures`

**Status:** `done`

### §7.5.1.2.3 Out-Struktur wie in der Basic-Abbildung, @Mutable

**Spec:** §7.5.1.2.3, S. 44 — "The rules for mapping the Out structure are
identical to that defined in 7.5.1.1.5 [...] 1. If the operation has the
@Mutable annotation, then the synthesized Out structure shall have the
annotation @Extensibility(MUTABLE_EXTENSIBILITY)."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` mit `method_out_members` (Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_out_carries_the_return_value`,
`crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`

**Status:** `done`

### §7.5.1.2.3 Result-Struktur ${interfaceName}_${operationName}_Result

**Spec:** §7.5.1.2.3, S. 44 — "Each operation in the interface shall map to an
Out structure and a Result structure. [...] 2. The Result structure name shall
be “${interfaceName}_${operationName}_Result.”"

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_result_carries_out_and_the_exceptions`

**Status:** `done`

### §7.5.1.2.3 Result-Struktur mit @Choice

**Spec:** §7.5.1.2.3, S. 44 — "3. The Result structure shall have the
annotation @Choice."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (`autoid_choice`; Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_result_carries_out_and_the_exceptions`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done`

### §7.5.1.2.3 Result-Struktur mit @Autoid

**Spec:** §7.5.1.2.3, S. 44 — "4. The Result structure shall have the
annotation @Autoid."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (`autoid_choice`; Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_result_carries_out_and_the_exceptions`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done`

### §7.5.1.2.3 Result-Struktur: erster Member result

**Spec:** §7.5.1.2.3, S. 44 — "5. The Result structure shall have a first
member with name “result” and with type ${interfaceName}_${operationName}_Out."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (`RESULT_MEMBER`; Commit `60dbad781c53`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_result_carries_out_and_the_exceptions`

**Status:** `done`
### §7.5.1.2.3 Result-Struktur: ein Member je Exception

**Spec:** §7.5.1.2.3, S. 44 — "6. The Result structure name shall have members
for each exception an operation may raise. a. The member name shall be
synthetized as the lower-case version of the exception name with the suffix
“_ex” added. b. The type associated with the member shall be the exception
type."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (Member `<exception>_ex`,
Name in Kleinbuchstaben),
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_result_carries_out_and_the_exceptions`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_result_is_a_choice_of_the_result_and_the_exceptions`

**Status:** `done` — das Modell legt je Exception ein Member
`<kleingeschriebener Name>_ex` vom Typ der Exception an (60dbad781c53;
076900132ad5 für die Generatoren). Das Rust-Binding kodiert das Member mit der
Member-ID `toobig_ex`.

### §7.5.1.2.3 @Mutable: Result-Struktur MUTABLE

**Spec:** §7.5.1.2.3, S. 44 — "7. If the operation has the @Mutable
annotation, then the synthesized Result structure shall have the annotation
@Extensibility(MUTABLE_EXTENSIBILITY)."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_operation` (`MethodDef::mutable` setzt
`EnhancedStruct::mutable` an `In`, `Out` und `Result`),
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_mutable_operation_has_mutable_structures`

**Status:** `done` — Hat die Operation `@Mutable`, sind `In`, `Out` und `Result`
MUTABLE, bei anderen Operationen nicht (60dbad781c53; 076900132ad5). Der zweite
Test prüft die Oktette der `In`-Struktur (DHEADER und EMHEADER je Member); für
`Result` prüft ihn nur das Modell.

### §7.5.1.2.4 Request-Struktur ${interfaceName}_Request

**Spec:** §7.5.1.2.4, S. 45 — "Each interface shall map to a Request structure.
[...] 1. The Request structure name shall be “${interfaceName}_Request.”"

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service_with` (`${interfaceName}_Request`),
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_request_is_a_choice_over_the_operations`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done` — jedes Interface erhält genau eine Struktur
`${interfaceName}_Request` (60dbad781c53; 076900132ad5). Die Generatoren für
Rust, C++ und Java deklarieren sie
(`enhanced_bindings.rs::rust_declares_the_structures_of_the_clause`,
`cpp_declares_the_structures_of_the_clause`,
`java_declares_the_structures_of_the_clause`).

### §7.5.1.2.4 Request-Struktur mit @Choice

**Spec:** §7.5.1.2.4, S. 45 — "2. The Request structure shall have the @Choice
annotation."

**Repo:** `crates/rpc/src/codegen.rs::autoid_choice`,
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_request_is_a_choice_over_the_operations`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_request_is_a_mutable_structure_with_one_member`

**Status:** `done` — `EnhancedStruct::choice` ist für die Request-Struktur
gesetzt (60dbad781c53; 076900132ad5). Das Rust-Binding kodiert sie als
PL_CDR2-Objekt mit genau einem Member (DHEADER, EMHEADER, Member).

### §7.5.1.2.4 Request-Struktur mit @Autoid

**Spec:** §7.5.1.2.4, S. 45 — "3. The Request structure shall have the @Autoid
annotation."

**Repo:** `crates/rpc/src/codegen.rs::autoid` (Member-ID = HASH des Namens, 28 Bit),
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_autoid_member_ids_are_the_hash_of_the_name`,
`crates/rpc/src/codegen.rs::tests::enhanced_member_id_collision_is_an_error`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_member_ids_are_the_hash_of_the_names`

**Status:** `done` — die Request-Struktur trägt `@Autoid`; die Member-ID ist der
auf 28 Bit gekürzte HASH des Member-Namens (60dbad781c53; 076900132ad5). Eine
Kollision zweier Namen ist ein Fehler (`RpcError::MemberIdCollision`).

### §7.5.1.2.4 Request-Struktur: ein Member je Operation

**Spec:** §7.5.1.2.4, S. 45 — "4. The Request structure shall contain a member
for each operation in the interface. a. The member name shall be the operation
name. b. The member type shall be ${interfaceName}_${operationName}_In."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service_with` (Member je Operation, Typ
`${I}_${op}_In`), `crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_request_is_a_choice_over_the_operations`,
`crates/rpc/src/codegen.rs::tests::enhanced_inheritance_is_flattened`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done` — Member-Name ist der Operationsname, Typ
`${interfaceName}_${operationName}_In`; geerbte Operationen tragen den Typnamen
ihres Basis-Interface (`Adder_add_In`). 60dbad781c53; 076900132ad5.

### §7.5.1.2.5 Reply-Struktur ${interfaceName}_Reply

**Spec:** §7.5.1.2.5, S. 46 — "Each interface shall map to a Reply structure.
[...] 1. The Reply structure name shall be “${interfaceName}_Reply”."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service_with` (`${interfaceName}_Reply`),
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_reply_has_the_results_and_remote_ex`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done` — jedes Interface erhält genau eine Struktur
`${interfaceName}_Reply` (60dbad781c53; 076900132ad5). Die drei Generatoren
deklarieren sie
(`enhanced_bindings.rs::rust_declares_the_structures_of_the_clause`,
`cpp_declares_the_structures_of_the_clause`,
`java_declares_the_structures_of_the_clause`).

### §7.5.1.2.5 Reply-Struktur mit @Choice

**Spec:** §7.5.1.2.5, S. 46 — "2. The Reply structure shall have the @Choice
annotation."

**Repo:** `crates/rpc/src/codegen.rs::autoid_choice`,
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_reply_has_the_results_and_remote_ex`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_reply_carries_remote_ex`

**Status:** `done` — `EnhancedStruct::choice` ist für die Reply-Struktur gesetzt
(60dbad781c53; 076900132ad5). Das Rust-Binding kodiert einen Reply mit
`remoteEx` als Objekt mit einem Member.

### §7.5.1.2.5 Reply-Struktur mit @Autoid

**Spec:** §7.5.1.2.5, S. 46 — "3. The Reply structure shall have the @Autoid
annotation."

**Repo:** `crates/rpc/src/codegen.rs::autoid`,
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_autoid_member_ids_are_the_hash_of_the_name`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_member_ids_are_the_hash_of_the_names`

**Status:** `done` — die Reply-Struktur trägt `@Autoid`, Member-ID wie bei der
Request-Struktur (60dbad781c53; 076900132ad5).
`the_member_ids_are_the_hash_of_the_names` prüft alle Strukturen des Modells,
die Reply-Struktur eingeschlossen.

### §7.5.1.2.5 Reply-Struktur: ein Member je Operation

**Spec:** §7.5.1.2.5, S. 46 — "4. The Reply structure shall contain a member
for each operation in the interface. a. The member name shall be the operation
name. b. The member type shall be ${interfaceName}_${operationName}_Result."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service_with` (Member je Operation, Typ
`${I}_${op}_Result`), `crates/lang-rules/src/adapter/interface.rs::enhanced_service`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_reply_has_the_results_and_remote_ex`,
`crates/rpc/src/codegen.rs::tests::enhanced_inheritance_is_flattened`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`

**Status:** `done` — Member-Name ist der Operationsname, Typ
`${interfaceName}_${operationName}_Result`; geerbte Operationen tragen den
Typnamen der Basis (`Subtractor_sub_Result`). 60dbad781c53; 076900132ad5.

### §7.5.1.2.5 Reply-Struktur: Member remoteEx

**Spec:** §7.5.1.2.5, S. 46 — "5. The Reply structure shall contain a member
named “remoteEx” of type dds::rpc::RemoteException_t. If an interface has an
operation named remoteEx (case-sensitive), the corresponding Reply structure
shall contain a member named remoteEx_N and type RemoteException_t, where N is
the first integer in the range 1 to 2^31 that avoids the collision."

**Repo:** `crates/rpc/src/codegen.rs::remote_ex_member_name`,
`crates/lang-rules/src/adapter/interface.rs::remote_ex_member_name`,
`crates/rpc/src/enhanced.rs::encode_remote_ex_reply`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_reply_has_the_results_and_remote_ex`,
`crates/rpc/src/codegen.rs::tests::enhanced_remote_ex_avoids_an_operation_of_that_name`,
`crates/rpc/src/enhanced.rs::tests::a_remote_ex_member_renamed_for_a_colliding_operation_is_found_by_its_name`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_remote_exception_is_the_remote_ex_member`

**Status:** `done` — die Reply-Struktur hat das Member `remoteEx`; heißt eine
Operation so, lautet es `remoteEx_N` mit dem ersten freien N ab 1 (60dbad781c53;
076900132ad5). Als Typ steht `RemoteExceptionCode_t`: den von der Spec genannten
Typ `dds::rpc::RemoteException_t` definiert die Spec nicht, §7.5.1.1.1 und
§7.5.2 definieren `RemoteExceptionCode_t` (Widerspruch innerhalb der Spec). Die
Tests prüfen N = 1; ein Fall mit einer Operation `remoteEx_1` (N = 2) hat keinen
Test.

### §7.5.1.2.2 bis §7.5.1.2.5 Beispiele

**Spec:** §7.5.1.2.3–§7.5.1.2.5, S. 45–46 — Beispiele für `RobotControl`
("Non-normative example").

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Beispiele. Die Struktur des Beispiels ist
durch die Items zu §7.5.1.2.3 bis §7.5.1.2.5 abgedeckt.

### §7.5.1.2.6 Vererbung wird flach gemacht

**Spec:** §7.5.1.2.6, S. 46 — "A derived interface is interpreted as if all the
inherited operations are declared in-place in the derived interface. That is,
the inheritance structure is flattened."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service_with` (geerbte Operationen
zuerst, Basis für Basis, dann die eigenen),
`crates/lang-rules/src/adapter/interface.rs::enhanced_service`
(über `operations_with_interface_flattened`),
`crates/lang-rules/src/emit/interface_rust/enhanced.rs` (abgeleiteter Dispatcher)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_inheritance_is_flattened`,
`crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_derived_request_has_the_inherited_operation`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_derived_dispatcher_serves_an_inherited_member`

**Status:** `done` — `Request` und `Reply` des abgeleiteten Interface enthalten
die geerbten Operationen unter den Typnamen der Basis; der abgeleitete
Dispatcher beantwortet sie (60dbad781c53, 076900132ad5, 2b68e33cef9b).

### §7.5.1.2.6 Beispiel Calculator

**Spec:** §7.5.1.2.6, S. 46–49 — Enhanced-Abbildung von `Calculator : Adder,
Subtractor` ("Non-normative Example").

**Repo:** —

**Tests:** `crates/lang-rules/tests/enhanced_bindings.rs::the_model_is_the_clause` (nimmt das Beispiel als Vorlage)

**Status:** `n/a (informative)` — Beispiel. Die Strukturen des Beispiels stehen
als Erwartung in `the_model_is_the_clause`.

### §7.5.1.2.6 Kompatibilität über die Regeln der Zuweisbarkeit

**Spec:** §7.5.1.2.6, S. 49 — "The Enhanced Service Mapping leverages the
assignability rules defined in the [DDS-XTypes] specification to ensure that
the topic types remain compatible in case of inheritance. A client using the
base interface can invoke an operation in a service that implements a derived
interface as long as the same topic names are used."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service_with` (gleiche Member-IDs und
Typnamen der Basis, `topic_aliases` je Elternteil über `ServiceDef::topic_names`),
`crates/discovery/src/sedp/cache.rs::topics_correspond`
(Zuordnung über `topic_aliases`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_inheritance_is_flattened`,
`crates/rpc/src/topic_naming.rs::tests::every_parent_interface_contributes_one_alias`,
`crates/dcps/tests/topic_alias_matching.rs::a_reader_on_an_alias_matches_the_writer_over_the_wire`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_member_the_interface_does_not_have_is_unknown_operation`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/TypeConsistencyTest.java`

**Status:** `done` — Rust und C++ sind umgesetzt und gemessen: gleiche Member-IDs für geerbte Operationen, Topic-Aliase der Eltern, Zuordnung über Aliase (928df3c39a42), Überspringen unbekannter Member, und die Topics tragen die `TypeInformation` ihrer Typen, so dass `decide_type_consistency` die Zuweisbarkeit auswertet (Rust `da6bf28aade1`; C++ über `zerodds_writer_create_qos_typed` und `zerodds_reader_create_qos_typed` der C-API, `d6f81c0ac615`, `b5363f647417`). Tests: `crates/rpc-rust-e2e/tests/enhanced_type_information.rs::base_and_derived::a_base_client_calls_a_derived_service` (Rust), `crates/rpc/tests/cpp_rpc_real_library.rs::the_cpp_binding_runs_against_libzerodds` (C++, Alt-Client gegen Neu-Service über `ev_old`/`ev_new`), Struktur der Objekte: `crates/lang-rules/tests/enhanced_type_information.rs` (Linux-Testhost, grün). Die Java-Laufzeit kündigt die `TypeInformation` an (`TypeDescription`, `RawTypeSupport`, vom Generator geschrieben: `05372ae52d05`, `d81b41c4459a`); der erzeugte Java-Code ist übersetzt und gelaufen (`crates/rpc/tests/java_rpc_runtime.rs`), und ein Java-Peer gegen einen Rust-Peer ist gemessen (`b3748d506a39`, Tests `java_peer.rs`, Linux-Testhost). Java paart wie Rust nach der Zuweisbarkeit (`95818352de1`, `e6af3ee9150`, `64134babe64`; Item „§7.5.1.2.6 Java: Paarung der Endpunkte nach Aliasen und Zuweisbarkeit“): `crates/rpc-rust-e2e/tests/java_peer.rs` misst gegen Rust-Peers den Client der Basis gegen den Dienst des abgeleiteten Interface in beiden Richtungen, einen geänderten Parametertyp und einen auf `long long` erweiterten Rückgabetyp (beide ohne Match auf der Java-Seite) und ein an eine `@mutable`-Struktur angehängtes Member (Match, die Antwort wird gelesen); Einheitstest `TypeConsistencyTest.java` (9 Fälle).

### §7.5.2 Lokale und entfernte Fehlercodes

**Spec:** §7.5.2, S. 49–50 — "Error codes are (conceptually) classified as
local and remote. The error codes described in the [DDS] PIM are all local
error codes. This specification further adds remote error codes [...]
represented using the RemoteExceptionCode_t IDL enumeration below."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einordnung; die Aufzählung ist bei §7.5.1.1.1
geführt.

### §7.5.2 REMOTE_EX_OK

**Spec:** §7.5.2, S. 50 — "REMOTE_EX_OK — The request was executed
successfully."

**Repo:** `crates/rpc/src/replier.rs::Replier::tick` (`RemoteExceptionCode::Ok` nach
erfolgreichem Handler), `Replier::send_reply`

**Tests:** `crates/rpc/src/replier.rs::tests::tick_processes_request_and_writes_reply`,
`crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`

**Status:** `done` — der Replier schreibt den Code `Ok` mit dem Reply; Commit
der Tests f06c1eaf3d3a (Replier), 2933b7d28a88 (`send_reply`).

### §7.5.2 REMOTE_EX_UNSUPPORTED

**Spec:** §7.5.2, S. 50 — "REMOTE_EX_UNSUPPORTED — Operation is valid but it is
unsupported (a.k.a. not implemented)."

**Repo:** `crates/lang-rules/src/emit/interface_rust.rs::dispatcher_handle_fn` (unbekannte
Operation, §7.7.1.1), `crates/lang-rules/src/emit/interface_java/calls.rs` (Dispatcher
des Java-Service), `crates/rpc/src/replier.rs::Replier::send_exception`

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`
(`FunctionCallRoundtrip.java`),
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(`function_call_roundtrip.cpp`),
`crates/rpc/src/replier.rs::tests::send_exception_answers_with_the_code`

**Status:** `done` — der erzeugte Rust-Dispatcher, der erzeugte Java-Service und
der erzeugte C++-Service antworten in der Basic-Abbildung auf eine Operation,
die der Service nicht kennt, mit `REMOTE_EX_UNSUPPORTED` (§7.7.1.1; Rust
fbc219a6b735, Java c4a249bd251f, C++ 3f74f05ed2b8). Der Default-Zweig von
`answer_method` in `crates/lang-rules/src/emit/interface_cpp/calls.rs` wirft
`RemoteUnsupportedError`; `function_call_roundtrip.cpp` erwartet diesen Fehler
beim Client.

### §7.5.2 REMOTE_EX_INVALID_ARGUMENT

**Spec:** §7.5.2, S. 50 — "REMOTE_EX_INVALID_ARGUMENT — The value of the
parameter passed has an illegal value (e.g., two options active in a @Choice
structure)."

**Repo:** `crates/rpc/src/replier.rs::Replier::tick` (Nutzlast nicht dekodierbar, und
Code des Handlers), `runtime/cpp/include/dds/rpc/core.hpp::detail::remote_exception_code_of`
(`RemoteInvalidArgumentError`)

**Tests:** `crates/rpc/src/replier.rs::tests::tick_propagates_handler_error_into_reply`,
`crates/rpc/src/replier.rs::tests::tick_answers_an_undecodable_payload_with_invalid_argument`,
`crates/rpc/src/replier.rs::tests::receive_requests_answers_an_undecodable_payload_with_invalid_argument`,
`crates/rpc/tests/runtime_e2e.rs::e2e_handler_error_surfaces_as_remote_exception_at_caller`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_request_with_two_active_members_is_refused`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_choice_without_a_member_is_refused`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::the_generated_service_answers_two_active_members_with_invalid_argument`,
`crates/rpc/tests/java_enhanced_codec.rs::the_generated_java_service_and_client_answer_the_error_cases_of_the_clauses`
(`EnhancedRuntimeChecks.java`),
`crates/lang-rules/tests/enhanced_java_calls.rs::a_choice_holds_exactly_one_member_on_both_sides_of_the_codec`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(`rpc_runtime_roundtrip.cpp`)

**Status:** `done` — der Code wird gesendet, wenn der Handler ihn liefert, wenn die Nutzlast eines Requests nicht dekodierbar ist und wenn eine `@Choice`-Struktur zwei aktive Optionen oder keine trägt. Rust: `Replier::tick` und `Replier::receive_requests` (Commits 8891b6d7f067, 57d849ba9b26). Java: der erzeugte Decoder lehnt ab, die Laufzeit antwortet (2051ce1a2a87). C++: `Replier::pull` in `runtime/cpp/include/dds/rpc.hpp` beantwortet einen Request, den es nicht dekodieren kann, mit `REMOTE_EX_INVALID_ARGUMENT`, in der Enhanced-Abbildung über die Identität des Samples, in der Basic-Abbildung über die Identität im Header (vorher wurde er verworfen; `c398e1587cf7`). Test: `crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service` (Antwort auf der Leitung, auf dem Linux-Testhost gelaufen). Der Weg über den Handler ist in C++ getestet (`RemoteInvalidArgumentError`). Commits der früheren Teile: f06c1eaf3d3a (Weg über den Handler), 3d4c42cad194 (Fehler-Reply mit Default-Fall der `Return`-Union).

### §7.5.2 REMOTE_EX_OUT_OF_RESOURCES

**Spec:** §7.5.2, S. 50 — "REMOTE_EX_OUT_OF_RESOURCES — The remote service ran
out of resources while processing the request."

**Repo:** `crates/rpc/src/replier.rs::Replier::send_exception`,
`runtime/cpp/include/dds/rpc/core.hpp::detail::remote_exception_code_of`
(`RemoteOutOfResourcesError`)

**Tests:** `crates/rpc/src/replier.rs::tests::tick_answers_out_of_resources`,
`crates/rpc/tests/runtime_e2e.rs::e2e_out_of_resources_surfaces_as_remote_exception_at_caller`,
`crates/rpc/src/replier.rs::tests::send_exception_answers_with_the_code`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(`RpcRuntimeRoundtrip.java`, `RemoteOutOfResourcesError`)

**Status:** `done` — eine Anwendung sendet den Code über den Handler
(`Err(RemoteExceptionCode::OutOfResources)`), in C++ und Java über
`RemoteOutOfResourcesError`. Rust: der Replier schreibt den Code, der Aufrufer
erhält `RemoteException(OutOfResources)` (57d849ba9b26). Java: der Test wirft den
Fehler im Service und prüft ihn beim Client. In C++ prüft
`rpc_runtime_roundtrip.cpp` nur die Klassenbeziehung der Fehlerklasse
(`static_assert`). Keine Laufzeit erkennt Ressourcenmangel selbst; die Spec legt
nicht fest, wann ein Service Ressourcenmangel meldet. Die Rust-Laufzeit sendet
den Code nicht für einen Fehler beim Kodieren des Replies; dieser Fall meldet
`REMOTE_EX_UNKNOWN_EXCEPTION` (fbc219a6b735).

### §7.5.2 REMOTE_EX_UNKNOWN_OPERATION

**Spec:** §7.5.2, S. 50 — "REMOTE_EX_UNKNOWN_OPERATION — The operation called
is unknown."

**Repo:** `crates/lang-rules/src/emit/interface_rust/enhanced.rs::answer_unknown_operations`
(Enhanced, §7.7.2), `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs`,
`crates/lang-rules/src/emit/interface_cpp/calls.rs::answer_member`

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_unknown_member_is_answered_unknown_operation`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_member_the_interface_does_not_have_is_unknown_operation`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_remote_exception_is_the_remote_ex_member`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`
(`enhanced_function_call_roundtrip.cpp`),
`crates/rpc/tests/java_enhanced_codec.rs::the_generated_java_service_and_client_answer_the_error_cases_of_the_clauses`
(`EnhancedRuntimeChecks.java`)

**Status:** `done` — in der Enhanced-Abbildung beantworten die erzeugten
Dispatcher eine unbekannte Operation mit `REMOTE_EX_UNKNOWN_OPERATION`
(§7.7.2.1; Rust 076900132ad5). Das Verhalten ist für Rust, C++
(`enhanced_function_call_roundtrip.cpp`) und Java (`EnhancedRuntimeChecks.java`,
2051ce1a2a87) mit einer laufenden Gegenstelle getestet. Die Basic-Abbildung
sendet in diesem Fall `REMOTE_EX_UNSUPPORTED` (§7.7.1.1, Item oben).

### §7.5.2 REMOTE_EX_UNKNOWN_EXCEPTION

**Spec:** §7.5.2, S. 50 — "REMOTE_EX_UNKNOWN_EXCEPTION — A generic,
unspecified exception was raised by the service implementation."

**Repo:** `crates/lang-rules/src/emit/interface_rust.rs::dispatcher_call_arm_via` (Wildcard-Zweig),
`crates/rpc/src/replier.rs::Replier::tick` (Fehler beim Kodieren des Replies),
`runtime/cpp/include/dds/rpc/core.hpp::detail::remote_exception_code_of`,
`crates/lang-rules/src/emit/interface_java/calls.rs`

**Tests:** `crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(`function_call_roundtrip.cpp`),
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(`rpc_runtime_roundtrip.cpp`),
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`
(`FunctionCallRoundtrip.java`),
`crates/rpc-rust-e2e/tests/e2e.rs::an_exception_the_operation_does_not_declare_is_unknown_exception`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_exception_the_operation_does_not_declare_is_unknown_exception`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::the_generated_client_reports_unknown_exception_for_an_unknown_result_member`

**Status:** `done` — eine Ausnahme der Implementierung, die weder die Operation
deklariert noch ein Remote-Code ist, wird in C++ und Java als
`REMOTE_EX_UNKNOWN_EXCEPTION` gesendet und beim Client als
`RemoteUnknownExceptionError` ausgelöst (C++ 24e3443ce819, Java c4a249bd251f).
Der erzeugte Rust-Dispatcher sendet den Code für eine nicht deklarierte
Exception (Tests oben, 8891b6d7f067); ein unbekanntes Member des Results meldet
der Rust-Client als `UnknownException`. Für `Replier::tick` bei einem Reply, der
sich nicht kodieren lässt (fbc219a6b735), gibt es keinen Test.

### §7.5.2 — Verwendung der Codes

**Spec:** §7.5.2, S. 50 — Tabelle der Bedeutungen.

**Repo:** `crates/rpc/src/replier.rs::Replier::send_error_reply`,
`crates/rpc/src/wire_codec.rs::ERROR_REPLY_DATA`

**Tests:** `crates/rpc/src/replier.rs::tests::tick_propagates_handler_error_into_reply`,
`crates/rpc/src/replier.rs::tests::send_exception_answers_with_the_code`

**Status:** `done` — die Zuordnung der Codes zu den Fällen steht in den Items zu den einzelnen Codes; alle sechs entsprechen der Spec in Rust, C++ und Java (`REMOTE_EX_INVALID_ARGUMENT` in C++: `c398e1587cf7`). Ein Reply mit Fehlercode enthält den Header und den Default-Fall der `Return`-Union (§7.5.1.1.7 Regel 3); beide Tests prüfen `payload == ERROR_REPLY_DATA` (3d4c42cad194).

### §7.5.2 Abbildung auf Exceptions in C++ und Java

**Spec:** §7.5.2, S. 50 — "Both local and remote exception codes map to
exceptions in the C++ and Java language bindings defined in 7.11 Language
Bindings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Verweis; die Abbildung steht in §7.11.1.3 und
§7.11.2.1.

---

## §7.6 Discovering and Matching RPC Services

### §7.6.1 Basic: eingebaute Discovery ohne Erweiterung

**Spec:** §7.6.1, S. 50 — "The Basic Service Mapping relies on the built-in
discovery provided by the underlying [DDS] compliant implementation. It does not
use any extensions to the DDS discovery built-in topics."

**Repo:** `crates/rpc/src/requester.rs::Requester::from_topics`,
`crates/rpc/src/replier.rs::Replier::from_topics` (gewöhnliche `DataWriter` und
`DataReader` der DCPS-Schicht; `set_rpc_discovery` wird nur bei
`RpcMapping::Enhanced` aufgerufen)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/src/requester.rs::tests::the_basic_mapping_sends_without_a_service_match`

**Status:** `done` — die Basic-Abbildung benutzt die eingebaute Discovery von
DCPS; kein Parameter der Erweiterung wird angekündigt (3d4c42cad194).

### §7.6.1 Endpunkte von Client und Service über die eingebauten Topics

**Spec:** §7.6.1, S. 50 — "A client comprises one (or more) DataWriter to send
requests and one (or more) DataReader to receive replies. Similarly, a service
implementation comprises one (or more) DataReader to receive requests and one
(or more) DataWriter to send replies. The built-in publication and subscription
topics are used to discover the DataWriter and DataReader at both sides."

**Repo:** `crates/rpc/src/requester.rs::Requester::from_topics`,
`crates/rpc/src/replier.rs::Replier::from_topics`

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`

**Status:** `done` — der Requester erzeugt einen `DataWriter` für Requests und
einen `DataReader` für Replies, der Replier einen `DataReader` und einen
`DataWriter`; die Zuordnung läuft über die eingebauten Publication- und
Subscription-Topics (f06c1eaf3d3a).

### §7.6.1 Hinweis auf die Wettlaufsituation

**Spec:** §7.6.1, S. 50 — "Non-normative Note: Service discovery for the Basic
Service Mapping is not robust because discovery race conditions can cause the
service replies to be lost."

**Repo:** —

**Tests:** `crates/rpc/src/requester.rs::tests::the_basic_mapping_sends_without_a_service_match`

**Status:** `n/a (informative)` — Hinweis. Unter der Basic-Abbildung schreibt
der Rust-`Requester` einen Request sofort, ohne auf das Matching des
Reply-Readers zu warten (`Requester::service_matched` ist dort immer wahr).

### §7.6.2 Enhanced: Erweiterung der eingebauten Topics

**Spec:** §7.6.2, S. 51 — "The Enhanced Service Mapping relies on the built-in
discovery service provided by DDS. However, it extends the built-in publication
and subscription topic data to avoid the discovery race conditions." Dazu die
Design Rationale.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Erweiterung und die Regeln
stehen in den Items unten.

### §7.6.2 Client: Request erst nach Discovery des Service-DataWriter

**Spec:** §7.6.2, S. 51 — "A client must ensure that the client-side DataReader
has discovered the service-side DataWriter before writing a request to the
service-side DataReader."

**Repo:** `crates/rpc/src/requester.rs::Requester::wait_for_service`,
`Requester::service_matched`,
`crates/rpc/src/discovery_ext.rs::service_matching_complete` (Figure 7.4),
`runtime/cpp/include/dds/rpc/base.hpp::service_matching_complete`,
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java`

**Tests:** `crates/rpc/src/requester.rs::tests::an_enhanced_request_waits_for_the_service_and_fails_without_sending`,
`crates/rpc/src/requester.rs::tests::the_fail_setting_does_not_wait`,
`crates/rpc/src/discovery_ext.rs::tests::the_client_waits_for_both_service_endpoints`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — Rust (3d4c42cad194) und C++ (5b7e99bb2b84, 95f5e6617035)
senden einen Enhanced-Request erst, wenn Request-Writer und Reply-Reader einen
Service gemeinsam gefunden haben (Figure 7.4: die Endpunkte nennen einander über
den related key, und der Request-Reader des Service kündigt den Instanznamen an,
den der Client nennt); `ServiceMatching::Queue` ist die Vorgabe, `Fail` und
`Off` sind benannte Alternativen. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2 Service: Request erst nach Discovery des Client-DataReader

**Spec:** §7.6.2, S. 51 — "A service must ensure that service-side DataWriter
has discovered the client-side DataReader before a request is accepted by the
service-side DataReader."

**Repo:** `crates/rpc/src/replier.rs::Replier::take_requests` (hält Requests eines nicht
vollständig entdeckten Clients zurück),
`crates/rpc/src/discovery_ext.rs::client_matching_complete` (Figure 7.3),
`runtime/cpp/include/dds/rpc/base.hpp::client_matching_complete`,
`runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java`

**Tests:** `crates/rpc/src/replier.rs::tests::an_enhanced_request_of_an_undiscovered_client_is_held`,
`crates/rpc/src/replier.rs::tests::the_named_alternatives_release_a_request_of_an_undiscovered_client`,
`crates/rpc/src/discovery_ext.rs::tests::the_service_unmasks_a_client_once_both_its_endpoints_are_discovered`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`

**Status:** `done` — Rust (3d4c42cad194) und C++ (95f5e6617035) halten einen
Enhanced-Request zurück, bis der Reply-Reader des sendenden Clients entdeckt ist
(Figure 7.3); `ClientMatching::Hold` ist die Vorgabe, `Off` und
`accept_unannounced` sind benannte Alternativen. Die Rust-Laufzeit hält
höchstens `HELD_REQUESTS_MAX` Requests, weitere zählt `Replier::held_overflow`. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.1 Erweiterte Typen der eingebauten Topics

**Spec:** §7.6.2.1, S. 51 — "This specification extends both
PublicationBuiltinTopicData and SubscriptionBuiltinTopicData." Dazu die Design
Rationale zur ParameterList.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Member stehen in §7.6.2.1.1
und §7.6.2.1.2.

### §7.6.2.1.1 service_instance_name mit PID 0x0080

**Spec:** §7.6.2.1.1, S. 51 — "service_instance_name string<256>
PID_SERVICE_INSTANCE_NAME 0x0080".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SERVICE_INSTANCE_NAME`),
`crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::service_instance_name`, Codec),
`crates/dcps/src/runtime.rs::RpcDiscoveryExt::check` (Grenze 256)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/rtps/src/publication_data.rs::tests::rpc_discovery_pids_roundtrip`,
`crates/dcps/tests/rpc_discovery_extension.rs::a_discovery_string_longer_than_256_is_refused`

**Status:** `done` — PID 0x0080 mit CDR-String; `DataWriter::set_rpc_discovery`
und `DataReader::set_rpc_discovery` weisen einen Namen über 256 Zeichen ab
(`DdsError::BadParameter`). PID und Codec b5224bd70dbb, Ankündigung und Grenze
0e62f93df137. Der Decoder prüft die Grenze bei einer empfangenen Ankündigung
nicht.

### §7.6.2.1.1 related_datareader_key mit PID 0x0081

**Spec:** §7.6.2.1.1, S. 51 — "related_datareader_key GUID_t
PID_RELATED_ENTITY_GUID 0x0081".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::RELATED_ENTITY_GUID`),
`crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::related_entity_guid`),
`crates/rtps/src/subscription_data.rs` (dasselbe Feld, §7.6.2.1.2)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/rtps/src/publication_data.rs::tests::rpc_discovery_pids_roundtrip`,
`crates/rtps/src/subscription_data.rs::tests::rpc_discovery_pids_roundtrip_subscription`

**Status:** `done` — PID 0x0081 mit 16 Oktetten GUID; das Feld heißt
`related_entity_guid` und trägt `related_datareader_key` einer Publication und
`related_datawriter_key` einer Subscription. b5224bd70dbb.

### §7.6.2.1.1 topic_aliases mit PID 0x0082

**Spec:** §7.6.2.1.1, S. 51 — "topic_aliases sequence<string<256>>
PID_TOPIC_ALIASES 0x0082".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::TOPIC_ALIASES`),
`crates/rtps/src/publication_data.rs`
(`PublicationBuiltinTopicData::topic_aliases`),
`crates/dcps/src/runtime.rs::RpcDiscoveryExt::check` (Grenze 256 je Alias)

**Tests:** `crates/rtps/src/parameter_list.rs::tests::rpc_pids_roundtrip_in_parameter_list`,
`crates/rtps/src/publication_data.rs::tests::rpc_discovery_pids_roundtrip`,
`crates/dcps/tests/topic_alias_matching.rs::a_reader_on_an_alias_matches_the_writer_over_the_wire`

**Status:** `done` — PID 0x0082 mit einer Folge von Strings; `set_rpc_discovery`
weist einen Alias über 256 Zeichen ab, und die Zuordnung der Endpunkte benutzt
die Aliase (928df3c39a42; PID b5224bd70dbb, Ankündigung 0e62f93df137).

### §7.6.2.1.1 PublicationBuiltinTopicDataExt

**Spec:** §7.6.2.1.1, S. 51 — "@extensibility(MUTABLE_EXTENSIBILITY) struct
PublicationBuiltinTopicDataExt : PublicationBuiltinTopicData { @ID(0x0080)
string<256> service_instance_name; @ID(0x0081) GUID_t related_datareader_key;
@ID(0x0082) sequence<string<256>> topic_aliases; };"

**Repo:** `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData` (die drei Member als
optionale Parameter der ParameterList), `crates/dcps/src/runtime.rs::RpcDiscoveryExt`

**Tests:** `crates/rtps/src/publication_data.rs::tests::rpc_discovery_pids_roundtrip`,
`crates/rtps/src/publication_data.rs::tests::rpc_pids_optional_legacy_peer_parses_ok`,
`crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`

**Status:** `done` — die Leitung trägt die drei Member; ein Peer ohne die
Parameter wird gelesen, die Member sind dann `None`. Der Typ
`PublicationBuiltinTopicDataExt` mit den Member-Namen der Spec steht in
`crates/rpc` (nächstes Item). b5224bd70dbb, 0e62f93df137.

### §7.6.2.1.1 Typ PublicationBuiltinTopicDataExt in crates/rpc

**Spec:** §7.6.2.1.1, S. 51 — wie oben.

**Repo:** `crates/rpc/src/discovery_ext.rs::PublicationBuiltinTopicDataExt` und
`SubscriptionBuiltinTopicDataExt` (`announcement`, `from_peer`)

**Tests:** `crates/rpc/src/discovery_ext.rs::tests::the_extension_carries_the_three_members_of_the_spec`,
`crates/rpc/src/discovery_ext.rs::tests::an_absent_related_key_reads_as_guid_unknown`

**Status:** `done` — die Typen haben die Member der Spec
(`service_instance_name`, `related_datareader_key` bzw.
`related_datawriter_key`, `topic_aliases`); `service_name` und `mapping_profile`
sind entfernt. `announcement()` wandelt sie in die Ankündigung von
`crates/dcps`. `Requester` und `Replier` bilden `RpcDiscoveryExt` unmittelbar
mit denselben drei Membern und benutzen diese Typen nicht (3d4c42cad194).

### §7.6.2.1.1 service_instance_name: Name der Instanz am Service

**Spec:** §7.6.2.1.1, S. 52 — "The service_instance_name is the instance name
specified by the user at the service-side. If none is specified, there is no
default value."

**Repo:** `crates/rpc/src/replier.rs::Replier::from_topics` (Name nur, wenn angegeben),
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::instance_name`,
`runtime/cpp/include/dds/rpc.hpp`, `runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java`

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::a_service_without_an_instance_name_or_parents_announces_neither`,
`crates/rpc/src/endpoint.rs::tests::instance_name_propagated_to_both_sides`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(`rpc_runtime_roundtrip.cpp`, `check_instance_addressing`)

**Status:** `done` — Rust (3d4c42cad194, Tests e8821ba9dae4, d79e4dac24d0) und
C++ (95f5e6617035) kündigen den Namen an den Service-Endpunkten an, wenn der
Benutzer einen angibt; ohne Namen wird nichts angekündigt (Test dafür vorhanden).
In C++ erreicht ein an eine Instanz gebundener Enhanced-Request nur Reader, die
diesen Namen angekündigt haben (86c89466df19). `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.1.1 Client-DataWriter ohne service_instance_name

**Spec:** §7.6.2.1.1, S. 52 — "Likewise, the client-side DataWriter shall not
include service_instance_name."

**Repo:** `crates/rpc/src/requester.rs::Requester::from_topics` (`service_instance_name: None`),
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::publication` (nur auf der Service-Seite)

**Tests:** `crates/rpc/src/endpoint.rs::tests::instance_name_propagated_to_both_sides`
(prüft den Builder; der Name des Tests stammt aus dem früheren Verhalten),
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — der Client-Writer kündigt keinen `service_instance_name`
an, in Rust, C++ (Test: `request_writer.has_instance_name` ist falsch) und Java
(der Client erhält keinen Namen). 7689fef2a37b (Builder), 3d4c42cad194
(Laufzeit). Kein Rust-Test liest die Ankündigung eines `Requester` von der
Gegenstelle zurück.

### §7.6.2.1.1 topic_aliases nach §7.4.3, ohne Vorgabe

**Spec:** §7.6.2.1.1, S. 52 — "The topic_aliases are as per defined in 7.4.3
The Enhanced Service Mapping. There is no default value."

**Repo:** `crates/rpc/src/requester.rs::Requester::from_topics`,
`crates/rpc/src/replier.rs::Replier::from_topics` (Aliase aus
`ServiceTopicNames::request_aliases` und `reply_aliases`),
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::aliases`

**Tests:** `crates/rpc/src/topic_naming.rs::tests::every_parent_interface_contributes_one_alias`,
`crates/rpc/src/endpoint.rs::tests::the_parent_interfaces_are_announced_as_topic_aliases`,
`crates/rpc/src/endpoint.rs::tests::topic_aliases_empty_yields_none`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::a_service_without_an_instance_name_or_parents_announces_neither`,
`crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface`

**Status:** `done` — Rust (3d4c42cad194) und C++ kündigen die Aliase der Elternteile an (§7.4.3), ohne Eltern und ohne Angabe nichts. Ein laufender `Requester` und `Replier` kündigt die Aliase an, die Gegenstelle liest sie; ohne Eltern bleibt das Feld leer (Tests oben, e8821ba9dae4, d79e4dac24d0). Java kündigt die Aliase an, die `parents` der Params ergeben (PID 0x0082 auf der Leitung: `RpcDiscoveryTest.java::theThreeParametersTravelWithTheEndpoint`, `9f9cd6cfc777`, `95056920ba2f`); die erzeugte Java-Bindung setzt `parents` für ein abgeleitetes Interface (`86817fa5f076`). Die Java-PSM ordnet über die Aliase zu und paart die Typen nach Zuweisbarkeit (`95818352de1`); gemessen mit einem Java-Client der Basis gegen einen Rust-Dienst des abgeleiteten Interface und umgekehrt (Item zu §7.4.3, Java).

### §7.6.2.1.1 related_datareader_key nicht vorhanden: GUID_UNKNOWN

**Spec:** §7.6.2.1.1, S. 52 — "When not present the default value for the
related_datareader_key shall be interpreted as GUID_UNKNOWN as defined by sub
clause 9.3.1.5 of the [RTPS] specification."

**Repo:** `crates/rpc/src/discovery_ext.rs::GUID_UNKNOWN` und `from_peer`,
`crates/rtps/src/publication_data.rs` (der Decoder liefert `None`)

**Tests:** `crates/rpc/src/discovery_ext.rs::tests::an_absent_related_key_reads_as_guid_unknown`,
`crates/rpc/src/discovery_ext.rs::tests::an_unannounced_service_completes_only_when_accepted`,
`crates/rtps/src/publication_data.rs::tests::rpc_pids_optional_legacy_peer_parses_ok`

**Status:** `done` — ein fehlender Schlüssel liest sich als `GUID_UNKNOWN` (16
Nullen); die Matching-Funktionen behandeln ihn als nicht genannt, ein Paar ohne
Nennung ist nur mit `accept_unannounced` vollständig (3d4c42cad194).

### §7.6.2.1.1 Client-DataWriter: related_datareader_key des Reply-Readers

**Spec:** §7.6.2.1.1, S. 52 — "The PublicationBuiltinTopicDataExt that
corresponds to the client-side DataWriter used for sending requests for a
service shall have the related_datareader_key set to the BuiltinTopicKey_t of
the client-side DataReader used for receiving the replies."

**Repo:** `crates/rpc/src/requester.rs::Requester::from_topics` (`set_rpc_discovery` am
Request-Writer mit der GUID des Reply-Readers),
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::build_requester`

**Tests:** `crates/rpc/src/endpoint.rs::tests::requester_related_entity_guids_cross_link`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`

**Status:** `done` — Rust (3d4c42cad194) und C++ (95f5e6617035) setzen den
Schlüssel am Client-Writer auf den Reply-Reader und umgekehrt. Der Rust-Test
`the_runtime_announces_aliases_the_instance_name_and_the_related_keys` liest die
Schlüssel an den Gegenstellen eines laufenden `Requester` und `Replier`
(e8821ba9dae4, d79e4dac24d0). `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.1.1 Service-DataWriter: related_datareader_key des Request-Readers

**Spec:** §7.6.2.1.1, S. 52 — "The PublicationBuiltinTopicDataExt that
corresponds to the service-side DataWriter used for sending replies shall have
the related_datareader_key set to the BuiltinTopicKey_t of the service-side
DataReader used to receive the requests."

**Repo:** `crates/rpc/src/replier.rs::Replier::from_topics` (`set_rpc_discovery` am
Reply-Writer mit der GUID des Request-Readers),
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::build_replier`

**Tests:** `crates/rpc/src/endpoint.rs::tests::replier_related_entity_guids_cross_link`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`

**Status:** `done` — Rust (3d4c42cad194) und C++ (95f5e6617035) setzen den Schlüssel am
Service-Writer auf den Request-Reader und umgekehrt. Für Rust liest ein Test die
Ankündigung des `Replier` an der Gegenseite: `related_entity_guid` des
Reply-Writers ist die GUID des Request-Readers (d79e4dac24d0). `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.1.2 service_instance_name mit PID 0x0080

**Spec:** §7.6.2.1.2, S. 52 — "service_instance_name string<256>
PID_SERVICE_INSTANCE_NAME 0x0080".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::SERVICE_INSTANCE_NAME`);
`crates/rtps/src/subscription_data.rs` (`SubscriptionBuiltinTopicData::service_instance_name`,
Codec in `to_pl_cdr_le` und `from_pl_cdr_le`);
`crates/dcps/src/subscriber.rs` (`DataReader::set_rpc_discovery`,
`DataReader::matched_rpc_writers`) mit `RpcDiscoveryExt` in
`crates/dcps/src/runtime.rs`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::rpc_discovery_pids_roundtrip_subscription`,
`crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`
(nur Linux)

**Status:** `done` — PID 0x0080 wird kodiert, dekodiert und vom DataReader angekündigt
(Codec b5224bd70dbb, Ankündigung 0e62f93df137). `RpcDiscoveryExt::check`
(`crates/dcps/src/runtime.rs`) weist beim Ankündigen eine Zeichenkette über 256
zurück (Test: `crates/dcps/tests/rpc_discovery_extension.rs::a_discovery_string_longer_than_256_is_refused`).
Der Decoder in `crates/rtps` lehnt einen Wert über 256 mit
`WireError::ValueOutOfRange` ab (9a3940227ee4; Test:
`crates/rtps/src/subscription_data.rs::tests::a_discovery_string_beyond_the_bound_is_refused_subscription`).

### §7.6.2.1.2 related_datawriter_key mit PID 0x0081

**Spec:** §7.6.2.1.2, S. 52 — "related_datawriter_key GUID_t
PID_RELATED_ENTITY_GUID 0x0081".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::RELATED_ENTITY_GUID`);
`crates/rtps/src/subscription_data.rs` (`SubscriptionBuiltinTopicData::related_entity_guid`,
Codec in `to_pl_cdr_le` und `from_pl_cdr_le`);
`crates/dcps/src/subscriber.rs` (`DataReader::set_rpc_discovery`,
`DataReader::matched_rpc_writers`) mit `RpcDiscoveryExt` in
`crates/dcps/src/runtime.rs`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::rpc_discovery_pids_roundtrip_subscription`,
`crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`
(nur Linux)

**Status:** `done` — PID 0x0081 (`PID_RELATED_ENTITY_GUID`) wird kodiert, dekodiert und
angekündigt (Codec b5224bd70dbb, Ankündigung 0e62f93df137). Das Feld heißt im
Code `related_entity_guid`; beim DataReader ist es der `related_datawriter_key`.

### §7.6.2.1.2 topic_aliases mit PID 0x0082

**Spec:** §7.6.2.1.2, S. 52 — "topic_aliases sequence<string<256>>
PID_TOPIC_ALIASES 0x0082".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::TOPIC_ALIASES`);
`crates/rtps/src/subscription_data.rs` (`SubscriptionBuiltinTopicData::topic_aliases`,
Codec in `to_pl_cdr_le` und `from_pl_cdr_le`);
`crates/dcps/src/subscriber.rs` (`DataReader::set_rpc_discovery`,
`DataReader::matched_rpc_writers`) mit `RpcDiscoveryExt` in
`crates/dcps/src/runtime.rs`

**Tests:** `crates/rtps/src/subscription_data.rs::tests::rpc_discovery_pids_roundtrip_subscription`,
`crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`,
`crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`
(nur Linux)

**Status:** `done` — PID 0x0082 wird kodiert (Folge von Zeichenketten), dekodiert und
angekündigt (Codec b5224bd70dbb, Ankündigung 0e62f93df137). Das Matching nach
Alias steht beim Item zu §7.4.3.

### §7.6.2.1.2 SubscriptionBuiltinTopicDataExt

**Spec:** §7.6.2.1.2, S. 52 — "@extensibility(MUTABLE_EXTENSIBILITY) struct
SubscriptionBuiltinTopicDataExt : SubscriptionBuiltinTopicData { @ID(0x0080)
string<256> service_instance_name; @ID(0x0081) GUID_t related_datawriter_key;
@ID(0x0082) sequence<string<256>> topic_aliases; };"

**Repo:** `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData` (Wire);
`crates/rpc/src/discovery_ext.rs::SubscriptionBuiltinTopicDataExt` (Datenmodell
mit den drei Members der Spec, `announcement` und `from_peer` zu
`zerodds_dcps::runtime::RpcDiscoveryExt`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::rpc_discovery_pids_roundtrip_subscription`,
`crates/rtps/src/subscription_data.rs::tests::rpc_pids_optional_legacy_subscription_parses_ok`,
`crates/rpc/src/discovery_ext.rs::tests::the_extension_carries_the_three_members_of_the_spec`

**Status:** `done` — der Typ in `crates/rpc` hat die drei Members der Spec
(`service_instance_name`, `related_datawriter_key`, `topic_aliases`); auf der
Leitung stehen sie als Parameter 0x0080 bis 0x0082 der Parameterliste (Typ in
`crates/rpc` 3d4c42cad194, Wire b5224bd70dbb). Ein Endpunkt ohne die Parameter
wird gelesen und liefert `None` bzw. `GUID_UNKNOWN`.

### §7.6.2.1.2 service_instance_name: Name der Instanz am Service

**Spec:** §7.6.2.1.2, S. 52 — "The service_instance_name is the instance name
specified by the user at the service-side. If none is specified, there is no
default value."

**Repo:** `crates/rpc/src/replier.rs::Replier::with_options` (Enhanced: der Name der Instanz
geht an Request-Reader und Reply-Writer, ohne Namen entfällt der Wert);
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder::instance_name`

**Tests:** `crates/rpc/src/endpoint.rs::tests::instance_name_propagated_to_both_sides`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`

**Status:** `done` — der Service kündigt den vom Nutzer gesetzten Namen an, ohne Namen
`None` (3d4c42cad194). Der Test zeigt, dass der Client nur die Instanz erreicht,
deren Namen er trägt. C++ kündigt denselben Wert an (95f5e6617035; Test:
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`).
Java siehe Item „§7.6.2.1 Ankündigung der Erweiterungen durch die Laufzeiten“.

### §7.6.2.1.2 Client-DataReader ohne service_instance_name

**Spec:** §7.6.2.1.2, S. 52 — "Likewise, the client-side DataReader shall not
include service_instance_name."

**Repo:** `crates/rpc/src/requester.rs::Requester::with_options` (Enhanced:
`service_instance_name: None` an Request-Writer und Reply-Reader);
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder` (7689fef2a37b)

**Tests:** `crates/rpc/src/endpoint.rs::tests::instance_name_propagated_to_both_sides`
(Builder); C++: `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`

**Status:** `done` — der Reply-Reader des Requesters kündigt in Rust (3d4c42cad194), C++
(95f5e6617035) und im Builder (7689fef2a37b) keinen Namen an. Für Rust liest ein
Test die Ankündigung von `Requester` und `Replier` an der Gegenseite
(`matched_rpc_readers`, `matched_rpc_writers`): die Endpunkte des Clients tragen
`service_instance_name: None` (d79e4dac24d0). `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.1.2 topic_aliases nach §7.4.3, ohne Vorgabe

**Spec:** §7.6.2.1.2, S. 52 — "The topic_aliases are as per defined in sub
clause 7.4.3. There is no default value."

**Repo:** `crates/rpc/src/requester.rs::Requester::with_options` und
`crates/rpc/src/replier.rs::Replier::with_options` (Aliase aus
`ServiceTopicNames::request_aliases` und `reply_aliases`, ohne Aliase `None`);
`crates/rpc/src/endpoint.rs::RpcEndpointBuilder` (`aliases`);
`crates/dcps/src/runtime.rs` (Matching nach Alias)

**Tests:** `crates/rpc/src/endpoint.rs::tests::topic_aliases_propagated_to_both_sides`,
`crates/dcps/tests/topic_alias_matching.rs` (nur Linux),
`crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`,
`crates/rpc/tests/language_binding_agreement.rs` (Namen der Aliase);
C++: `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::a_service_without_an_instance_name_or_parents_announces_neither`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface`

**Status:** `done` — die Aliase gehen bei Vererbung auf die Leitung (Rust 3d4c42cad194, C++ 95f5e6617035) und lassen Endpunkte matchen (928df3c39a42); ohne Aliase wird nichts angekündigt. Für Rust liest ein Test die Aliase von `Requester` und `Replier` an der Gegenseite (`robot_Base_Service_Request` und `robot_Base_Service_Reply`) und prüft, dass ein Service ohne Basis keine Aliase ankündigt (d79e4dac24d0). Java kündigt die Aliase an, die `parents` der Params ergeben (`9f9cd6cfc777`, `95056920ba2f`); die erzeugte Java-Bindung setzt `parents` für ein abgeleitetes Interface (`86817fa5f076`). Die Java-PSM ordnet über die Aliase zu und paart die Typen nach Zuweisbarkeit (`95818352de1`); gemessen mit einem Java-Client der Basis gegen einen Rust-Dienst des abgeleiteten Interface und umgekehrt (Item zu §7.4.3, Java).

### §7.6.2.1.2 related_datawriter_key fehlt: GUID_UNKNOWN

**Spec:** §7.6.2.1.2, S. 53 — "When not present the default value for the
related_datareader_key shall be interpreted as GUID_UNKNOWN as defined by sub
clause 9.3.1.5 of the [RTPS] specification."

**Repo:** `crates/rpc/src/discovery_ext.rs::SubscriptionBuiltinTopicDataExt::from_peer`
(`GUID_UNKNOWN`); `crates/rtps/src/subscription_data.rs` (der Decoder liefert
`None`)

**Tests:** `crates/rtps/src/subscription_data.rs::tests::rpc_pids_optional_legacy_subscription_parses_ok`,
`crates/rpc/src/discovery_ext.rs::tests::an_absent_related_key_reads_as_guid_unknown`,
`crates/rpc/src/discovery_ext.rs::tests::an_unannounced_service_completes_only_when_accepted`

**Status:** `done` — ein fehlender Parameter 0x0081 liest als `GUID_UNKNOWN`
(3d4c42cad194). Der Abschnitt der Spec über die Subscription nennt hier
`related_datareader_key`.

### §7.6.2.1.2 Client-DataReader: related_datawriter_key des Request-Writers

**Spec:** §7.6.2.1.2, S. 53 — "The SubscriptionBuiltinTopicDataExt that
corresponds to the client-side DataReader used to receive replies shall have
the related_datawriter_key set to the BuiltinTopicKey_t of the client-side
DataWriter used to send the requests to the DDS-RPC Service."

**Repo:** `crates/rpc/src/requester.rs::Requester::with_options` (Reply-Reader nennt den
Request-Writer); `crates/rpc/src/endpoint.rs::RpcEndpointBuilder::build_requester`

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`,
`crates/rpc/src/endpoint.rs::tests::requester_related_entity_guids_cross_link`,
`crates/rpc/src/discovery_ext.rs::tests::the_service_unmasks_a_client_once_both_its_endpoints_are_discovered`

**Status:** `done` — Rust 3d4c42cad194, C++ 95f5e6617035. Der Aufruf im ersten Test
wird nur beantwortet, wenn der Service den Reply-Reader des Clients über
`related_datawriter_key` dem Request-Writer zuordnen kann
(`client_matching_complete`, Standardeinstellung `ClientMatching::Hold`).

### §7.6.2.1.2 Service-DataReader: related_datawriter_key des Reply-Writers

**Spec:** §7.6.2.1.2, S. 53 — "The SubscriptionBuiltinTopicDataExt that
corresponds to the service-side DataReader used to write replies shall have the
related_datawriter_key set to the BuiltinTopicKey_t of the service-side
DataWriter used to receive requests."

**Repo:** `crates/rpc/src/replier.rs::Replier::with_options` (Request-Reader nennt den
Reply-Writer); `crates/rpc/src/endpoint.rs::RpcEndpointBuilder::build_replier`

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc/src/endpoint.rs::tests::replier_related_entity_guids_cross_link`,
`crates/rpc/src/discovery_ext.rs::tests::the_two_endpoints_must_name_each_other`

**Status:** `done` — Rust 3d4c42cad194, C++ 95f5e6617035. Der Satz der Spec vertauscht die
Rollen („DataReader used to write replies“, „DataWriter used to receive
requests“); der Request-Reader nennt den Reply-Writer, wie Figure 7.4 es
verlangt. `service_matching_complete` ist nur erfüllt, wenn Request-Reader und
Reply-Writer einander nennen (Test 1 und 3).

### §7.6.2.1 Ankündigung der Erweiterungen durch die Laufzeiten

**Spec:** §7.6.2.1.1 und §7.6.2.1.2, S. 51–53 — wie die Items oben.

**Repo:** `crates/dcps/src/publisher.rs` und `crates/dcps/src/subscriber.rs` (`set_rpc_discovery`,
`matched_rpc_readers`, `matched_rpc_writers`);
`crates/rpc/src/requester.rs::Requester::with_options` und
`crates/rpc/src/replier.rs::Replier::with_options` (Enhanced);
C++: `runtime/cpp/include/dds/rpc.hpp` (`announce_rpc_discovery`) und
`runtime/cpp/include/dds/rpc/zerodds.hpp` über
`zerodds_writer_set_rpc_discovery` und `zerodds_reader_set_rpc_discovery`
(`crates/zerodds-c-api/src/rpc_discovery_ffi.rs`)

**Tests:** `crates/dcps/tests/rpc_discovery_extension.rs::the_rpc_extension_of_a_writer_and_a_reader_reaches_the_matched_peer`
(nur Linux),
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::the_runtime_announces_aliases_the_instance_name_and_the_related_keys`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::a_service_without_an_instance_name_or_parents_announces_neither`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface`

**Status:** `done` — Rust, C++ und Java kündigen `service_instance_name`, `related_*_key` und `topic_aliases` ihrer Endpunkte an und lesen sie bei entdeckten Endpunkten (dcps 0e62f93df137, Rust 3d4c42cad194, C++ 95f5e6617035, Java `9f9cd6cfc777` und `95056920ba2f`); die Grenze 256 prüft `RpcDiscoveryExt::check` beim Ankündigen, der Decoder in `crates/rtps` und der von `crates/java-omgdds` lehnen Werte darüber ab (9a3940227ee4, `9f9cd6cfc777`). Für Rust liest ein Test die Ankündigung von `Requester` und `Replier` an der Gegenseite (d79e4dac24d0), für Java `RpcDiscoveryTest.java`. Die erzeugte Java-Bindung setzt die Aliase eines abgeleiteten Interface (`86817fa5f076`); ein Java-Peer gegen einen Rust-Peer liest die Ankündigung (`b3748d506a39`). Die Java-PSM ordnet über die gelesenen Aliase zu und paart nach der Zuweisbarkeit (`95818352de1`), gemessen gegen Rust-Peers in beiden Richtungen (Item zu §7.4.3, Java).

### §7.6.2.2 Zusätzliche Regeln an Client-Writer und Service-Reader

**Spec:** §7.6.2.2, S. 53 — "Additional rules shall be used at the client-side
DataWriter and service-side DataReader".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Regeln folgen als Items.

### §7.6.2.2 Client-Writer sendet erst nach dem Matching des Reply-Writers

**Spec:** §7.6.2.2, S. 53 — "The client-side DataWriter (request DataWriter)
shall not send out any request to a service DataReader unless the client-side
DataReader (reply DataReader) has discovered and matched the corresponding
service DataWriter (reply DataWriter)."

**Repo:** `crates/rpc/src/requester.rs::Requester::service_matched` und
`Requester::wait_for_service` (Enhanced);
`crates/rpc/src/discovery_ext.rs::service_matching_complete`;
C++: `runtime/cpp/include/dds/rpc.hpp` (`Requester::wait_for_service`,
`DataWriter::service_matching_complete`); Java:
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java`

**Tests:** `crates/rpc/src/requester.rs::tests::an_enhanced_request_waits_for_the_service_and_fails_without_sending`,
`crates/rpc/src/requester.rs::tests::the_fail_setting_does_not_wait`,
`crates/rpc/src/requester.rs::tests::the_basic_mapping_sends_without_a_service_match`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc/src/discovery_ext.rs::tests::the_client_waits_for_both_service_endpoints`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — Rust (3d4c42cad194) und C++ (5b7e99bb2b84, 95f5e6617035): der Request
geht erst hinaus, wenn Request-Reader und Reply-Writer des Services entdeckt
sind und einander nennen. Der Rust-Aufruf hält den Request bis höchstens
`request_timeout` der QoS und endet danach mit `RpcError::ServiceNotMatched`,
ohne gesendet zu haben (`ServiceMatching::Queue`; `Fail` und `Off` sind die
benannten Alternativen). Die Basic-Abbildung hat die Regel nicht (§7.6.1). `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.2 Service-Reader weist Requests vor dem Matching ab

**Spec:** §7.6.2.2, S. 53 — "The service-side DataReader (request DataReader)
shall reject requests from a client DataWriter (request DataWriter) as long as
the service-side DataWriter (reply DataWriter) has not discovered and matched
the corresponding client-side DataReader (reply DataReader)."

**Repo:** `crates/rpc/src/replier.rs::Replier::take_enhanced_requests` (hält Requests
eines Clients, der nicht vollständig entdeckt ist);
`crates/rpc/src/discovery_ext.rs::client_matching_complete`; C++:
`runtime/cpp/include/dds/rpc.hpp` (`Replier`, `DataReader::client_matching_complete`);
Java: `runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java`

**Tests:** `crates/rpc/src/replier.rs::tests::an_enhanced_request_of_an_undiscovered_client_is_held`,
`crates/rpc/src/replier.rs::tests::the_named_alternatives_release_a_request_of_an_undiscovered_client`,
`crates/rpc/src/discovery_ext.rs::tests::the_service_unmasks_a_client_once_both_its_endpoints_are_discovered`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — Rust (3d4c42cad194) und C++ (5b7e99bb2b84, 95f5e6617035): der Service
bearbeitet keinen Request eines Clients, dessen Reply-Reader der Reply-Writer
nicht entdeckt hat. Rust hält den Request (bis `HELD_REQUESTS_MAX` = 4096, der
Überlauf wird in `held_overflow()` gezählt) und bearbeitet ihn, sobald der
Client vollständig ist, statt ihn zu verwerfen. Die Basic-Abbildung hat die
Regel nicht. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.2 Umsetzung außerhalb der DDS-Schicht zulässig

**Spec:** §7.6.2.2, S. 53 — "The above rules may be implemented outside “DDS
layer” because no discovery algorithm modifications are required in the
underlying DDS implementation".

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §7.6.2.2.1 Client Matching am Service: keine Requests bis zum Abschluss

**Spec:** §7.6.2.2.1, S. 54 — "“client matching” does not complete unless the
service-side has discovered both client entities (DataReader and DataWriter).
While “client matching” is still in progress, service-side does not process
requests from the client."

**Repo:** `crates/rpc/src/discovery_ext.rs::client_matching_complete`;
`crates/rpc/src/replier.rs::Replier::take_enhanced_requests`;
`crates/zerodds-c-api/src/rpc_discovery_ffi.rs` (`zerodds_rpc_client_matching_complete`);
C++: `runtime/cpp/include/dds/rpc.hpp`

**Tests:** `crates/rpc/src/discovery_ext.rs::tests::the_service_unmasks_a_client_once_both_its_endpoints_are_discovered`,
`crates/rpc/src/discovery_ext.rs::tests::an_unannounced_client_is_unmasked_only_when_accepted`,
`crates/rpc/src/replier.rs::tests::an_enhanced_request_of_an_undiscovered_client_is_held`,
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`

**Status:** `done` — Rust (3d4c42cad194) und C++ (95f5e6617035): „Client Matching“ ist
vollständig, wenn der Reply-Writer den Reply-Reader entdeckt hat, den der
Request-Writer des Clients in `related_datareader_key` nennt, und dieser Reader
den Request-Writer nennt (Figure 7.3). Bis dahin bearbeitet der Service keinen
Request des Clients. Ein Client ohne Ankündigung wird nur mit
`accept_unannounced` freigegeben. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.2.1 Client Matching: local_mask und Freigabe des Client-Writers

**Spec:** §7.6.2.2.1, S. 54 und Figure 7.3 — "At the beginning, the
service-side DataReader is enabled with local_mask=All_DW so that it does not
process any incoming requests. When the service-side DataWriter completes the
discovery of a client DataReader, the service DataReader is ready to accept
requests from that specific client. Therefore, it unmasks the related client
DataWriter."

**Repo:** `crates/rpc/src/replier.rs::Replier::take_enhanced_requests` (Haltelisten je
Request-Writer des Clients, GUID aus der Identität des Requests);
C++: `runtime/cpp/include/dds/rpc.hpp` (`DataReader::client_matching_complete`
je Client)

**Tests:** `crates/rpc/src/replier.rs::tests::an_enhanced_request_of_an_undiscovered_client_is_held`,
`crates/rpc/src/replier.rs::tests::the_named_alternatives_release_a_request_of_an_undiscovered_client`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/ReplierClientMatchingTest.java`,
`crates/rpc-rust-e2e/tests/replier_activate_client_matching.rs::an_activated_replier_answers_a_held_request_once_its_client_is_matched`

**Status:** `done` — Rust (3d4c42cad194) und C++ (5b7e99bb2b84, 95f5e6617035) maskieren je
Client: ein Request wird erst bearbeitet, wenn `client_matching_complete` für
den Request-Writer dieses Clients gilt; andere Clients werden dadurch nicht
aufgehalten. Rust setzt keine `local_mask` am DataReader der DDS-Schicht; der
Replier nimmt die Samples und hält sie in einer eigenen Liste. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Linux-Testhost grün, 3 Tests, `64134babe64`). Die Freigabe hing an einem neuen Sample: ein Replier, dessen Listener bei Ankunft läuft (§7.9.2.2 Weg 1), behielt einen gehaltenen Request, wenn sein Client danach durch Discovery vollständig wurde, ohne dass ein weiteres Sample kam (gemessen mit `crates/rpc-rust-e2e/tests/java_peer.rs`, Rust-Client von `Base2` gegen den Java-Dienst von `Derived2` unter Last: „held … replyMatched=0“, danach der Match des Reply-Writers und keine Antwort). Seit `f6abe1435f1` prüft ein solcher Replier alle 5 ms, solange Requests gehalten sind, ob einer ihrer Clients vollständig ist, und gibt dann frei; ein abgefragter Replier (`receiveRequests`, `waitForRequests`, `Server`) prüft bei jeder Abfrage. Test: `ReplierClientMatchingTest.java` (rot auf `d913a72f8c0`). Dieselbe Lücke hatte `Replier::activate` (und `activate_async`) in Rust, das nur auf DATA_AVAILABLE hörte; seit `30f16f76c91` sieht ein Thread des Repliers die gehaltenen Requests alle 5 ms an, solange welche gehalten sind. Test: `replier_activate_client_matching.rs` (der Reply-Reader des Clients tritt erst nach dem Request in die Partition des Reply-Writers; ohne den Fix gehalten 1, beantwortet 0).

### §7.6.2.2.2 Service Matching am Client: keine Requests bis zum Abschluss

**Spec:** §7.6.2.2.2, S. 55 — "“service matching” does not complete unless the
client-side has discovered both service DDS Entities (DataReader and
DataWriter) and it does not send requests to the service until “service
matching” completes."

**Repo:** `crates/rpc/src/discovery_ext.rs::service_matching_complete`;
`crates/rpc/src/requester.rs::Requester::service_matched`;
`crates/zerodds-c-api/src/rpc_discovery_ffi.rs` (`zerodds_rpc_service_matching_complete`);
C++: `runtime/cpp/include/dds/rpc.hpp`

**Tests:** `crates/rpc/src/discovery_ext.rs::tests::the_client_waits_for_both_service_endpoints`,
`crates/rpc/src/discovery_ext.rs::tests::the_two_endpoints_must_name_each_other`,
`crates/rpc/src/discovery_ext.rs::tests::the_client_addresses_the_instance_it_names`,
`crates/rpc/src/discovery_ext.rs::tests::an_unannounced_service_completes_only_when_accepted`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/zerodds-c-api/tests/rpc_discovery.rs::the_extension_is_announced_read_and_decides_the_matching`

**Status:** `done` — Rust (3d4c42cad194) und C++ (95f5e6617035): „Service Matching“ ist
vollständig, wenn Request-Reader und Reply-Writer des Services entdeckt sind,
einander nennen und der Request-Reader den `instance_name` des Clients
ankündigt (Figure 7.4); der Client sendet vorher nicht. Ein Service ohne
Ankündigung wird nur mit `accept_unannounced` als vollständig gelesen. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

### §7.6.2.2.2 Service Matching: local_mask, Warteschlange, Freigabe

**Spec:** §7.6.2.2.2, S. 55 und Figure 7.4 — "At the beginning, the
client-side DataWriter is enabled local_mask=All_DR such that it does not send
any request at all. Attempts to send requests would be queued. When the
client-side DataReader completes the discovery of a service DataWriter, the
client DataWriter updates the local mask so that it can send requests to the
service DataReader as long as it has been discovered."

**Repo:** `crates/rpc/src/requester.rs::Requester::wait_for_service` (Rust: der sendende
Aufruf hält den Request); C++: `runtime/cpp/include/dds/rpc.hpp`
(`Requester::queued_requests`); Java:
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java` (Warteschlange
`queue`, `flushQueue`)

**Tests:** `crates/rpc/src/requester.rs::tests::an_enhanced_request_waits_for_the_service_and_fails_without_sending`,
`crates/rpc/src/requester.rs::tests::the_fail_setting_does_not_wait`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — Rust (3d4c42cad194): Der Client sendet nicht; der Aufruf hält den
Request bis höchstens `request_timeout` und schlägt dann mit
`RpcError::ServiceNotMatched` fehl, der Request steht danach nicht in einer
Warteschlange. C++ (5b7e99bb2b84, 95f5e6617035) stellt Requests in eine
Warteschlange und gibt sie frei, sobald `service_matching_complete` gilt; eine
Maske `local_mask = All_DR` gibt es in keiner Laufzeit, die Freigabe ersetzt sie. `crates/java-omgdds` schreibt und liest die PIDs 0x0080 bis 0x0082 (`rtps/RpcDiscovery.java`, `DiscoveryData.buildEndpoint` und `parseEndpoint`, Grenze `string<256>`: eine Ankündigung darüber wird abgewiesen wie im Decoder von `crates/rtps`; `9f9cd6cfc777`). `ZeroDdsParticipant` kündigt `service_instance_name` an den Endpunkten des Dienstes und `related_*_key` an beiden Seiten an (`95056920ba2f`). Tests: `crates/java-omgdds/java/src/test/java/org/zerodds/rtps/RpcDiscoveryTest.java` (`theThreeParametersTravelWithTheEndpoint`, `aStringBeyondTheBoundIsRefused`, `aTopicCorrespondsByNameOrByAlias`). Die Java-Laufzeit hält die Matching-Regel aus den angekündigten Endpunkten ein (`f6a4bcb079e1`): `ReplierImpl` hält die Requests eines Clients zurück, dessen Reply-Reader der Reply-Writer nicht kennt (Figure 7.3, je Client); `RequesterImpl` sendet, wenn der Request-Reader der Instanz einen Reply-Writer nennt, den der Reply-Reader entdeckt hat (Figure 7.4). Ein Endpunkt ohne Ankündigung und eine Bindung ohne Discovery-Daten behalten das Matching nach dem Topic. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` über `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together` (braucht ein JDK; auf dem Mac und dem Linux-Testhost nicht gelaufen).

## §7.7 Interface Evolution

### §7.7.1 Evolution in der Basic-Abbildung

**Spec:** §7.7.1, S. 56 — "The Basic Service Profile has limited support for
interface evolution [...] Interface evolution is constrained by the rules
described herein."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.7.1.1 Client mit weniger Operationen

**Spec:** §7.7.1.1, S. 56 — "If a client uses an interface that has fewer
methods than that of the service, it is simply oblivious of the fact that the
service has more capabilities than the client can use."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (Labels aus dem Namen der
Operation, nicht aus der Position); `crates/rpc/src/function_call.rs`
(`dispatch_request` nach Opcode)

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`,
`crates/rpc/tests/profile_conformance.rs::function_call_processing_dispatches_to_correct_operation`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::versions::an_old_client_calls_a_new_service`

**Status:** `done` — das Label einer Operation ist der HASH ihres Namens
(9467d977720c, 1944de30d46f) und hängt nicht von den anderen Operationen des
Interface ab; der Service ruft nach Label auf. Für Enhanced läuft ein Client der
alten Fassung gegen den Service der neuen Fassung (Test 3, d79e4dac24d0); für
Basic gibt es drei erzeugte Fassungen (`basic_old`, `basic_new`, `basic_fewer` in
`crates/rpc-rust-e2e/build.rs`, `b21c700bc86e`) und die Tests
`evolution_styles_discovery.rs::basic_versions::a_client_with_fewer_operations_calls_a_service_with_more`,
`an_old_basic_client_calls_a_new_service` und `a_new_basic_client_calls_an_old_service`.

### §7.7.1.1 Client mit mehr Operationen: REMOTE_EX_UNSUPPORTED

**Spec:** §7.7.1.1, S. 56 — "If the client uses an interface with more
operations than that of the service, invocation of an unsupported operation
shall result in REMOTE_EX_UNSUPPORTED remote exception code."

**Repo:** `crates/lang-rules/src/emit/interface_rust.rs` (Dispatcher, Fall unbekannte
Operation: `RemoteExceptionCode::Unsupported`);
`crates/lang-rules/src/emit/interface_java/calls.rs`;
`crates/lang-rules/src/emit/interface_cpp/calls.rs` (Methode `answer`, Fall
`default`)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`
(erwartet `Unsupported`),
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`
(`FunctionCallRoundtrip.java`, erwartet `RemoteUnsupportedError`),
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(`function_call_roundtrip.cpp`, erwartet `RemoteUnsupportedError`)

**Status:** `done` — der erzeugte Rust-Dispatcher (fbc219a6b735), der erzeugte Java-Dienst
(c4a249bd251f) und der erzeugte C++-Dienst der Basic-Abbildung (3f74f05ed2b8)
antworten auf eine unbekannte Operation mit REMOTE_EX_UNSUPPORTED; der
C++-Client wirft `RemoteUnsupportedError`. Die Spec hat hier zwei Aussagen
(§7.7.1.1: REMOTE_EX_UNSUPPORTED; Tabelle in §7.5.2: REMOTE_EX_UNKNOWN_OPERATION
für „The operation called is unknown“); die Abbildung folgt §7.7.1.1. Der
Enhanced-Dispatcher antwortet mit REMOTE_EX_UNKNOWN_OPERATION (§7.7.2.1;
`enhanced_function_call_roundtrip.cpp`).

### §7.7.1.2 Reihenfolge von Operationen und Basen ohne Wirkung

**Spec:** §7.7.1.2, S. 56 — "Reordering operations has no impact on the
semantics of the interface because every interface has set semantics. [...]
Reordering base interfaces has no impact on the semantics of the interface."

**Repo:** `crates/rpc/src/codegen.rs::build_call_union` (Labels aus dem Namen, nicht aus
der Position); `crates/rpc/src/evolution_rules.rs::is_compatible`

**Tests:** `crates/rpc/src/codegen.rs::tests::the_case_labels_are_the_operation_hashes`,
`crates/rpc/src/evolution_rules.rs::tests::basic_mapping_reorder_operations_is_compatible`

**Status:** `done` — das Label einer Operation ist der HASH ihres Namens (9467d977720c);
Reihenfolge von Operationen und Basen ändert kein Label. Die Tabelle
`evolution_rules` nennt das Umordnen in Basic kompatibel (b0468d6ed0f0).

### §7.7.1.3 Keine Änderung der Signatur in der Basic-Abbildung

**Spec:** §7.7.1.3, S. 56 — "The Basic Service Profile shall not support any
modifications to the operation signatures including exception specification."

**Repo:** `crates/rpc/src/evolution_rules.rs::is_compatible` (Basic: `ChangeSignature`
und die Enhanced-Änderungen inkompatibel)

**Tests:** `crates/rpc/src/evolution_rules.rs::tests::basic_mapping_change_signature_is_breaking`

**Status:** `done` — in der Basic-Abbildung gibt es keine Änderung der Signatur, die als
kompatibel geführt wird (b0468d6ed0f0); die Basic-Typen sind `@final` und tragen
keine Member-IDs.

### §7.7.2 Evolution in der Enhanced-Abbildung

**Spec:** §7.7.2, S. 56 — "The Enhanced Service Profile supports flexible
interface evolution. [...] The interface mapping rules in the previous sub
clause are designed such that the assignability rules defined in [DDS-XTypes]
will ensure seamless evolution from Iold to Inew."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung und Begriffe; die Regeln folgen
als Items.

### §7.7.2.1 Neue Operation

**Spec:** §7.7.2.1, S. 56–57 — "Cold can invoke Snew because the additional
member in Requestnew is never populated by Cold. [...] When Cnew invokes Sold,
Sold receives a sample with no active member [...] In such cases, the service
(Sold) returns Replyold with remoteEx = REMOTE_EX_UNKNOWN_OPERATION."

**Repo:** `crates/lang-rules/src/emit/interface_rust/enhanced.rs` (`choice_codec`: eine
unbekannte Member-ID wird `UnknownOp`; `answer_unknown_operations`);
C++: `crates/lang-rules/src/emit/interface_cpp/enhanced_codec.rs`,
`crates/lang-rules/src/emit/interface_cpp/calls.rs`; Java: `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs`

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_member_the_interface_does_not_have_is_unknown_operation`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_unknown_member_is_answered_unknown_operation`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_reply_carries_remote_ex`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/rpc/tests/java_enhanced_codec.rs::the_generated_java_codec_writes_the_octets_of_the_clause`

**Status:** `done` — die Member-IDs sind der HASH des Namens; ein Member, den die Fassung des
Services nicht kennt, lässt die Request-Choice ohne aktives Member (Rust
`UnknownOp`) und der Service antwortet mit `remoteEx` =
REMOTE_EX_UNKNOWN_OPERATION (Rust 076900132ad5, C++ 99bda16e455b, Java
9ab3b695e13c). Ein alter Client erreicht einen neuen Service, weil er das
zusätzliche Member nie setzt. Rust und C++ führen den Fall aus; für Java prüfen
die Tests den Codec und den erzeugten Text (`enhanced_java_calls.rs`).

### §7.7.2.2 Entfernte Operation

**Spec:** §7.7.2.2, S. 57 — "When Cold calls Snew, Cold receives
REMOTE_EX_UNKNOWN_OPERATION RemoteExceptionCode."

**Repo:** wie bei „§7.7.2.1 Neue Operation“; der Client liest `remoteEx` als
`RpcError::RemoteException` (`crates/rpc/src/enhanced.rs`)

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_unknown_member_is_answered_unknown_operation`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_remote_exception_is_the_remote_ex_member`,
`crates/rpc/src/enhanced.rs::tests::a_remote_ex_reply_round_trips_every_code`

**Status:** `done` — ruft ein Client eine Operation auf, die der neue Service nicht mehr hat,
erhält er REMOTE_EX_UNKNOWN_OPERATION (076900132ad5, 3d4c42cad194). Ein neuer
Client mit einer Teilmenge der Operationen des alten Services ist der Fall von
§7.7.1.1 „weniger Operationen“, hier ohne Fehler.

### §7.7.2.3 Reihenfolge von Operationen und Basen

**Spec:** §7.7.2.3, S. 57 — "Due to the use of md5 hash algorithm to compute
the member ids, this specification considers two interfaces equivalent as long
as they contain the same operations."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (`@Autoid`: Member-ID =
HASH des Namens, 28 Bit); `crates/rpc/src/evolution_rules.rs::is_compatible`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_autoid_member_ids_are_the_hash_of_the_name`,
`crates/rpc/src/codegen.rs::tests::enhanced_inheritance_is_flattened`,
`crates/rpc/src/evolution_rules.rs::tests::enhanced_mapping_reorder_operations_is_compatible`

**Status:** `done` — die Member-ID einer Operation ist der HASH ihres Namens, nicht ihre
Position (60dbad781c53); zwei Interfaces mit denselben Operationen haben
dieselben IDs, gleich in welcher Reihenfolge Operationen und Basen stehen.

### §7.7.2.4 Duck Typing

**Spec:** §7.7.2.4, S. 57 — "Enhanced Service Profile supports “duck typing”
evolution of an interface."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (Vererbung flach, Typnamen
der Basis); `crates/lang-rules/src/emit/interface_rust/enhanced.rs`
(abgeleiteter Dispatcher bedient geerbte Operationen)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_inheritance_is_flattened`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_derived_request_has_the_inherited_operation`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_derived_dispatcher_serves_an_inherited_member`

**Status:** `done` — die Request-Choice eines abgeleiteten Interface enthält die geerbten
Operationen mit denselben Member-IDs, die sie im Interface selbst hätten
(60dbad781c53, 2b68e33cef9b); die Topic-Namen kommen aus dem Namen des
Interface. Ein Interface mit geerbten und eines mit direkt definierten
Operationen gleichen Namens sind auf der Leitung gleich.

### §7.7.2.5.1 Parameter hinzufügen oder entfernen

**Spec:** §7.7.2.5.1, S. 57 — "Adding and removing parameters in an operation
shall break compatibility with the older interface. The @final annotation on
the In and Out structures enforce this restriction. It is, however, possible to
mark an operation @mutable to allow the In and Out structures to be mutable".

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (`In`, `Out`, `Result` sind
`@final`, bei `@Mutable` an der Operation `@mutable`);
`crates/lang-rules/src/emit/interface_rust/enhanced.rs`; die Codecs für C++
und Java

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_mutable_operation_has_mutable_structures`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_mutable_structure_skips_a_member_it_does_not_know`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_absent_member_of_a_mutable_structure_takes_its_default`,
`crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`,
`crates/rpc/src/evolution_rules.rs::tests::enhanced_mapping_add_remove_param_is_breaking`;
C++: `crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`;
Java: `crates/rpc/tests/java_enhanced_codec.rs::the_generated_java_codec_writes_the_octets_of_the_clause`

**Status:** `done` — ohne `@Mutable` sind die Strukturen `@final` und tragen keine
Member-Header (ein weiterer Parameter bricht die Kompatibilität); mit `@Mutable`
trägt jeder Parameter einen EMHEADER mit der ID des Namens, ein unbekannter
Member wird übersprungen und ein fehlender nimmt seinen Default (076900132ad5,
2a06d562261e). Die Tabelle `evolution_rules` nennt das Hinzufügen und Entfernen
von Parametern inkompatibel (b0468d6ed0f0).

### §7.7.2.5.2 Parameter umordnen

**Spec:** §7.7.2.5.2, S. 57 — "Reordering parameters in an operation shall
break compatibility with the older interface. [...] It is, however, possible to
mark an operation @mutable to allow reordering of parameters. In that case the
synthesized structures (In and Out) shall be annotated @mutable."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (Member-IDs aus dem
Parameternamen; `@final` ohne `@Mutable`)

**Tests:** `crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_mutable_operation_has_mutable_structures`,
`crates/rpc/src/evolution_rules.rs::tests::enhanced_mapping_reorder_params_is_breaking`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::versions::an_old_client_calls_a_new_service`,
`crates/rpc-rust-e2e/tests/evolution_styles_discovery.rs::versions::a_new_client_calls_an_old_service`

**Status:** `done` — ohne `@Mutable` stehen die Parameter ohne Header in der Reihenfolge der Deklaration (Test 1, Fall `Calc_add_In`), ein Umordnen bricht die Kompatibilität; mit `@Mutable` hat jeder Parameter eine ID aus seinem Namen (076900132ad5). Zwei erzeugte Fassungen von `interface Svc` haben `@Mutable void mix(in long x, in long y)` und `@Mutable void mix(in long y, in long x)`; Client und Service der jeweils anderen Fassung tauschen die Argumente unter ihren Namen aus. Rust: Tests 3 und 4 (d79e4dac24d0). C++: `crates/rpc/tests/cpp/enhanced_evolution_roundtrip.cpp` über `crates/rpc/tests/cpp_enhanced_function_call.rs::two_generated_cpp_versions_of_an_interface_run_against_each_other` (`c398e1587cf7`, auf dem Linux-Testhost gelaufen). Java: `crates/rpc/tests/java/EvolutionChecks.java` über `crates/rpc/tests/java_enhanced_codec.rs::two_generated_java_versions_of_an_interface_run_against_each_other` (`88711477f7cb`, braucht ein JDK).

### §7.7.2.5.3 Typ eines Parameters ändern

**Spec:** §7.7.2.5.3, S. 57 — "Changing the type of the parameters is allowed
by this specification. The type assignability rules shall be as described in
[DDS-XTypes]."

**Repo:** —

**Tests:** `crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::versions::a_changed_parameter_type_does_not_match_the_java_client`,
`crates/rpc-rust-e2e/tests/java_peer.rs::versions::an_old_java_client_calls_an_old_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::appended_member::a_member_appended_to_a_mutable_structure_matches_the_java_client`

**Status:** `done` — Rust und C++ sind umgesetzt: die Topics tragen die `TypeInformation`, die Paarung folgt der Zuweisbarkeit nach XTypes. Tests (Linux-Testhost, grün): `crates/rpc-rust-e2e/tests/enhanced_type_information.rs::appended_member::a_member_appended_to_a_parameter_type_is_matched` (ein an eine `@mutable`-Struktur angehängtes Member), `...::versions::a_changed_parameter_type_does_not_match` (`long` zu `string`: `ServiceNotMatched`; mit Zuordnung nach Typname würde der Request gesendet und vom Dienst abgewiesen, mit einem Kontrolllauf gemessen); für C++ `cpp_rpc_real_library.rs::the_cpp_binding_runs_against_libzerodds` (`ev_changed`: der Client findet den Dienst nicht). Java (`95818352de1`, `e6af3ee9150`, `64134babe64`), gegen Rust-Peers über RTPS: `crates/rpc-rust-e2e/tests/java_peer.rs::versions::a_changed_parameter_type_does_not_match_the_java_client` (`long` zu `string`: der Java-Client findet den Dienst nicht, der Rust-Dienst sieht keinen Request; auf `7484a8f0d46`, vor der Paarung nach Zuweisbarkeit, paarte der Java-Client nach dem Typnamen, rief den Dienst und bekam keine Antwort), Kontrolle `...::versions::an_old_java_client_calls_an_old_rust_service`, und `...::appended_member::a_member_appended_to_a_mutable_structure_matches_the_java_client` (ein an `Pose` angehängtes Member: Match, das Argument erreicht den Dienst). Die Tabelle `evolution_rules` nennt die Änderung kompatibel (`enhanced_mapping_change_param_type_is_compatible`).

### §7.7.2.5.4 Rückgabetyp hinzufügen oder entfernen

**Spec:** §7.7.2.5.4, S. 57 — "Adding and removing return type shall break
compatibility with the older interface. [...] It is, however, possible to
annotate an operation @Mutable to allow the In and Out structures to be mutable
and consequently support addition or removal of return type".

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (der Rückgabewert ist das
Member `return` der `Out`-Struktur; `@final`, mit `@Mutable` `@mutable`)

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_out_carries_the_return_value`,
`crates/rpc/src/codegen.rs::tests::enhanced_mutable_operation_has_mutable_structures`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_absent_member_of_a_mutable_structure_takes_its_default`,
`crates/rpc/src/evolution_rules.rs::tests::enhanced_mapping_add_remove_return_type_is_breaking`

**Status:** `done` — der Rückgabewert ist ein Member der `Out`-Struktur; ohne `@Mutable` ist
sie `@final` (Hinzufügen oder Entfernen bricht die Kompatibilität), mit
`@Mutable` ist sie `@mutable` und wird wie die `In`-Struktur gelesen
(076900132ad5, 2a06d562261e). Die Tests zur Dekodierung eines fehlenden und eines
unbekannten Members laufen an der `In`-Struktur; der Codec der `Out`-Struktur
wird vom selben Generator erzeugt.

### §7.7.2.5.5 Rückgabetyp ändern

**Spec:** §7.7.2.5.5, S. 58 — "Changing the type of the return value is allowed
by this specification. The type assignability rules shall be as described in
[DDS-Xtypes]."

**Repo:** —

**Tests:** `crates/rpc/tests/cpp_rpc_real_library.rs::the_cpp_binding_runs_against_libzerodds`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_calls_a_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_calls_a_java_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::versions::a_return_type_the_old_one_is_not_assignable_to_does_not_match_the_java_client`,
`crates/rpc-rust-e2e/tests/java_peer.rs::appended_member::a_member_appended_to_a_mutable_structure_matches_the_java_client`

**Status:** `done` — Rust ist umgesetzt wie bei „§7.7.2.5.3 Typ eines Parameters ändern“. Tests (Linux-Testhost, grün): `crates/rpc-rust-e2e/tests/enhanced_type_information.rs::return_type::a_return_type_the_old_one_is_not_assignable_to_does_not_match` (`long` zu `long long`: `ServiceNotMatched`) und `...::appended_member::a_return_type_with_an_appended_member_is_matched` (ein an eine `@mutable`-Struktur im Rückgabetyp angehängtes Member: der alte Client liest die Antwort des neuen Dienstes, `ca27bd49b395`). C++ trägt dieselbe `TypeInformation` (Item zu §7.5.1.2.6); `cpp_rpc_real_library.rs::the_cpp_binding_runs_against_libzerodds` prüft einen Rückgabetyp mit angehängtem Member (der alte Client liest die Antwort) und einen von `long` auf `long long` erweiterten (kein Match) (`383e7958484`). Java (`95818352de1`, `64134babe64`), gegen Rust-Peers über RTPS: `crates/rpc-rust-e2e/tests/java_peer.rs::versions::a_return_type_the_old_one_is_not_assignable_to_does_not_match_the_java_client` (`long` zu `long long`: der Reply-Reader des Java-Clients paart nicht mit dem Writer des Dienstes; auf `7484a8f0d46` paarte er nach dem Typnamen, und der Aufruf von `add` kam durch) und `...::appended_member::a_member_appended_to_a_mutable_structure_matches_the_java_client` (Rückgabetyp `Pose` mit angehängtem Member: der Java-Client liest `origin.x=3`). Die Tabelle `evolution_rules` nennt die Änderung kompatibel (`enhanced_mapping_change_return_type_is_compatible`).

### §7.7.2.5.6 Exception hinzufügen oder entfernen

**Spec:** §7.7.2.5.6, S. 58 — "This specification supports adding and removing
one or more exceptions on an operation. The @mutable annotation on the return
structure will support this interface evolution."

**Repo:** `crates/rpc/src/codegen.rs::build_enhanced_service` (`Result` ist eine Choice
über `result` und `<name>_ex`, Member-IDs aus dem Namen);
`crates/lang-rules/src/emit/interface_rust/enhanced.rs` (`choice_codec` der
`Result` ohne `UnknownOp`); C++: `crates/lang-rules/src/emit/interface_cpp/calls.rs`;
Java: `crates/lang-rules/src/emit/interface_java/enhanced_calls.rs`

**Tests:** `crates/rpc/src/codegen.rs::tests::enhanced_result_carries_out_and_the_exceptions`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::a_result_is_a_choice_of_the_result_and_the_exceptions`,
`crates/rpc/src/evolution_rules.rs::tests::enhanced_mapping_add_remove_exception_is_compatible`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_unknown_exception_member_of_a_result_decodes_as_unknown_ex`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::the_generated_client_reports_unknown_exception_for_an_unknown_result_member`,
`crates/rpc-rust-e2e/tests/enhanced_generated.rs::an_exception_the_operation_does_not_declare_is_unknown_exception`,
`crates/rpc/tests/java_enhanced_codec.rs::the_generated_java_service_and_client_answer_the_error_cases_of_the_clauses`

**Status:** `done` — jede Exception ist ein eigenes Member `<name>_ex` der `Result`-Choice mit ID aus dem Namen (076900132ad5); das Hinzufügen oder Entfernen einer Exception ändert die IDs der übrigen nicht. Der erzeugte Rust-Client meldet ein unbekanntes `_ex`-Member der `Result` als `RpcError::RemoteException` mit REMOTE_EX_UNKNOWN_EXCEPTION, eine nicht deklarierte Exception des Service ebenso (8891b6d7f067). Java: `EnhancedRuntimeChecks.java` (2051ce1a2a87) und `EvolutionChecks.java` (88711477f7cb). C++: `enhanced_function_call_roundtrip.cpp` prüft eine nicht deklarierte Exception der Implementierung und eine `Result` mit unbekanntem Member (der Client wirft `RemoteUnknownExceptionError`), `enhanced_evolution_roundtrip.cpp` die Fassung mit zusätzlicher Exception gegen die ohne sie (`c398e1587cf7`, auf dem Linux-Testhost gelaufen).

### §7.7.2.5.6 REMOTE_EX_UNKNOWN_EXCEPTION für eine unbekannte Exception

**Spec:** §7.7.2.5.6, S. 58 — "The language binding that provides function
call/return semantics may throw REMOTE_EX_UNKNOWN_EXCEPTION if the RPC call
results into an exception that client does not understand."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Erlaubnis.

### §7.7 — Tabelle der Evolution

**Spec:** §7.7.1.1 bis §7.7.2.5.6, S. 56–58 — wie die Items oben.

**Repo:** `crates/rpc/src/evolution_rules.rs::is_compatible` und
`compatible_evolutions`

**Tests:** `crates/rpc/src/evolution_rules.rs::tests::basic_mapping_add_remove_operation_is_compatible`,
`crates/rpc/src/evolution_rules.rs::tests::basic_mapping_has_four_compatible_evolutions`,
`crates/rpc/src/evolution_rules.rs::tests::enhanced_compatible_evolutions_includes_8_of_12`,
`crates/rpc/src/evolution_rules.rs::tests::a_mutable_operation_allows_parameter_changes`,
`crates/rpc/src/evolution_rules.rs::tests::mutable_changes_nothing_that_was_compatible_or_never_was`,
`crates/rpc/src/evolution_rules.rs::tests::a_mutable_operation_has_eleven_compatible_enhanced_evolutions`

**Status:** `done` — die Tabelle stimmt mit §7.7.1.1 bis §7.7.2.5.6 überein (b0468d6ed0f0):
Basic nennt das Hinzufügen, Entfernen und Umordnen von Operationen kompatibel,
jede Änderung der Signatur inkompatibel; Enhanced nennt Typänderungen und das
Hinzufügen oder Entfernen von Exceptions kompatibel und das Hinzufügen,
Entfernen und Umordnen von Parametern und Rückgabetyp inkompatibel. Mit
`mutable = true` in `is_compatible` und `compatible_evolutions` nennt sie die
drei Änderungen kompatibel, die die Spec mit `@mutable` erlaubt (§7.7.2.5.1, .2,
.4; e1e005ef9a09). Kein Code außerhalb der Tests ruft die Tabelle auf; die Spec
verlangt keine Prüfung zur Laufzeit (aus dem Code gelesen).

---

## §7.8 Request and Reply Correlation

### §7.8.1 Basic: Korrelation über RequestHeader und ReplyHeader

**Spec:** §7.8.1, S. 58 — "The Basic service profile uses the RequestHeader and
ReplyHeader described in sub clause 7.5.1.1.1."

**Repo:** `crates/rpc/src/requester.rs::Requester::tick` (`related_request_id` gegen die
offenen Requests); `crates/rpc/src/replier.rs::Replier::tick`

**Tests:** `crates/rpc/src/requester.rs::tests::tick_correlates_reply_with_pending_slot`,
`crates/rpc/tests/runtime_e2e.rs::e2e_multi_pending_requests_all_get_their_reply`,
`crates/rpc/tests/runtime_e2e.rs::e2e_client_side_request_identity_is_the_one_the_reply_relates_to`,
`crates/rpc-rust-e2e/tests/e2e.rs::a_live_request_identity_is_the_rtps_identity_of_the_request_writer`

**Status:** `done` — die Rust-Laufzeit ordnet über `RequestHeader::request_id` und
`ReplyHeader::related_request_id` zu (f06c1eaf3d3a); die `request_id` ist GUID und
Folgenummer des Request-Writers (10ba007046af). C++ und Java siehe das nächste
Item.

### §7.8.1 C++ und Java: requestId im Header, Zuordnung über relatedRequestId

**Spec:** §7.8.1, S. 58 — wie oben.

**Repo:** C++: `runtime/cpp/include/dds/rpc.hpp` (`Requester::send_request` trägt die
Identität aus `DataWriter::next_identity()` in `RequestHeader::request_id` ein,
`Replier::dispatch_requests` antwortet mit `header.request_id`);
Java: `runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java`,
`ReplierImpl.java`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(die Anbindung im Speicher gibt keine Identität heraus),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — der Header trägt die explizite `request_id`, der Replier liest sie aus
dem Header, nicht aus `SampleInfo` (8202f8c67446); Requester filtern fremde
Replies nach der GUID ihres Request-Writers (d6f6b5e975c8). Die Anbindungen an
ZeroDDS in C++ und Java geben die Identität des Samples ebenfalls aus
(a8058d1272c9, 830064b55c76).

### §7.8.2 PID_RELATED_SAMPLE_IDENTITY 0x0083 in der Inline-QoS

**Spec:** §7.8.2, S. 58 — "To support propagation of the sample identity of a
related request this specification extends the [RTPS] specification adding a
new parameter id PID_RELATED_SAMPLE_IDENTITY with value 0x0083. This parameter
may appear in the inlineQos sub-message element of the DATA sub-message".

**Repo:** `crates/rtps/src/parameter_list.rs` (`pid::RELATED_SAMPLE_IDENTITY`);
`crates/rtps/src/inline_qos.rs` (`related_sample_identity_param`,
`find_related_sample_identity`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::related_sample_identity_param_uses_pid_0x0083`,
`crates/rtps/src/inline_qos.rs::tests::find_related_sample_identity_finds_pid`,
`crates/rtps/src/parameter_list.rs::tests::rpc_pid_constants_match_spec`

**Status:** `done` — Parameter-ID 0x0083, Kodierung und Suche in der Inline-QoS (b5224bd70dbb);
das Lesen maskiert nur das Must-Understand-Bit, eine herstellereigene ID 0x8083
wird nicht als 0x0083 gelesen (dfaa3df22ad0).

### §7.8.2 PID_RELATED_SAMPLE_IDENTITY an jedem Reply

**Spec:** §7.8.2, S. 58 — "The PID_RELATED_SAMPLE_IDENTITY shall be used for
data samples sent as reply to a request."

**Repo:** `crates/dcps/src/publisher.rs::DataWriter::write_related` und
`crates/dcps/src/runtime.rs::DcpsRuntime::write_user_sample_related` (schreiben
0x0083 und eProsimas 0x800f); `crates/rpc/src/replier.rs` (Enhanced-Reply über
`write_related`); C++: `zerodds_writer_write_related` in `crates/zerodds-c-api`;
Java: `runtime/java/src/main/java/org/zerodds/ddsrpc/ZeroDdsParticipant.java`
(`Writer.write(sample, related)`); `crates/rmw-zerodds-shim/src/services.rs`

**Tests:** `crates/rtps/src/inline_qos.rs::tests::reply_inline_qos_contains_only_pid_0x0083`,
`crates/dcps/src/runtime.rs::tests::the_related_identity_is_0x0083_or_eprosimas_0x800f_from_a_writer_that_means_it`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`,
`crates/rpc/tests/cpp_rpc_zerodds_related_identity.rs::the_zerodds_binding_sends_and_reads_the_related_sample_identity`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/ZeroDdsParticipantTest.java::aWriteReturnsItsIdentityAndTheReaderSeesItWithTheRelatedOne`

**Status:** `done` — in der Enhanced-Abbildung trägt jede Antwort den Parameter 0x0083 mit der
Identität des Requests und der Requester ordnet danach zu: Rust (84416dc1319c,
3d4c42cad194), C++ (a8058d1272c9, daf40ded82e0), Java (830064b55c76, der
Java-PSM-Writer gibt die Identität zurück und nimmt eine verwandte entgegen).
Die Basic-Abbildung korreliert über den Header (§7.8.1). Die ROS-2-Dienste
nutzen denselben Schreibpfad.

### §7.8.2 RPC-Laufzeiten schreiben und lesen PID_RELATED_SAMPLE_IDENTITY

**Spec:** §7.8.2, S. 58 — wie oben.

**Repo:** Rust: `crates/rpc/src/replier.rs::Replier::write_reply` (Enhanced: `DataWriter::write_related`),
`crates/rpc/src/requester.rs::Requester::tick` (liest die verwandte Identität aus `SampleWire`),
`crates/dcps/src/publisher.rs::DataWriter::write_related`,
`crates/dcps/src/runtime.rs::DcpsRuntime::write_user_sample_related`,
`crates/dcps/src/subscriber.rs::DataReader::take_with_wire`,
`crates/rtps/src/inline_qos.rs::find_related_sample_identity`. C++:
`runtime/cpp/include/dds/rpc/zerodds.hpp` (`Writer::write_related`,
`Reader::take`) über `zerodds_writer_write_related` und
`zerodds_reader_take_wire` der C-API. Java:
`runtime/java/src/main/java/org/zerodds/ddsrpc/ZeroDdsParticipant.java`
(`Writer.write(sample, related)`, `Reader.take`) über
`DataWriterImpl.writeIdentified` der Java PSM. Commits: `dfaa3df22ad0` (rtps:
0x8083 wird nicht mehr als 0x0083 gelesen), `84416dc1319c` (dcps),
`daf40ded82e0` (c-api), `3d4c42cad194` (Rust), `a8058d1272c9` (C++),
`830064b55c76` (Java)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::a_vendor_specific_0x8083_is_not_the_related_sample_identity`,
`crates/rtps/src/inline_qos.rs::tests::find_related_sample_identity_with_must_understand_bit_works`,
`crates/dcps/src/subscriber.rs::tests::take_with_wire_hands_over_the_identity_and_the_related_identity`,
`crates/dcps/src/publisher.rs::tests::write_related_without_a_runtime_is_refused`,
`crates/zerodds-c-api/tests/related_sample_identity.rs::a_related_sample_identity_reaches_the_reader_with_the_wire_identity`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`,
`crates/rpc/tests/cpp_rpc_zerodds_related_identity.rs::the_zerodds_binding_sends_and_reads_the_related_sample_identity`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/ZeroDdsParticipantTest.java::aWriteReturnsItsIdentityAndTheReaderSeesItWithTheRelatedOne`

**Status:** `done` — in Rust, C++ und Java schreibt der Replier der Enhanced-Abbildung die
Antwort mit dem Parameter, und der Requester liest ihn. Die Basic-Abbildung
trägt die Identität im `ReplyHeader` (§7.8.1). Die Rust-Tests des
Enhanced-Pfads lassen Requester und Replier auf einem Teilnehmer laufen
(Zustellung innerhalb der Laufzeit); die Kodierung im Inline-QoS prüfen die
Tests in `crates/rtps`.

### §7.8.2 Wert ist die CDR-Kodierung von SampleIdentity

**Spec:** §7.8.2, S. 58 — "When present, it shall contain the CDR
serialization of the SampleIdentity structure defined below."

**Repo:** `crates/rtps/src/inline_qos.rs::SampleIdentityBytes` (`to_bytes`,
`from_bytes`)

**Tests:** `crates/rtps/src/inline_qos.rs::tests::sample_identity_layout_is_24_bytes`,
`crates/rtps/src/inline_qos.rs::tests::sample_identity_roundtrip_le`,
`crates/rtps/src/inline_qos.rs::tests::sample_identity_roundtrip_be`

**Status:** `done` — 24 Oktette: GUID und Folgenummer in der Byte-Reihenfolge des
Parameters.

### §7.8.2 Wert gleich der Identität des Request-Samples

**Spec:** §7.8.2, S. 58 — "The value of the PID_RELATED_SAMPLE_IDENTITY in a
reply message shall be identical to the sample identity (GUID, SN) of the
request sample that triggered the reply."

**Repo:** Rust: `crates/rpc/src/replier.rs::Replier::receive_requests`
(`RequestIdentity::from_inline_qos` aus Writer-GUID und Folgenummer des DATA),
`Replier::send_reply` und `Replier::write_reply` (`write_related` mit dieser
Identität). C++: `runtime/cpp/include/dds/rpc/core.hpp::Replier::send_reply` mit
`Correlation::SAMPLE_IDENTITY` (`Writer::write_related`). Java:
`runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java::sendReply`
(`DataWriter.write(sample, relatedRequestId)`). Commits: `3d4c42cad194`
(Rust), `a8058d1272c9` (C++), `97c20505e85c` und `830064b55c76` (Java)

**Tests:** `crates/rpc/src/replier.rs::tests::an_enhanced_request_is_identified_by_its_data`,
`crates/rpc/src/requester.rs::tests::an_enhanced_reply_is_correlated_by_its_related_identity`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::an_enhanced_call_is_answered_through_the_related_sample_identity`,
`crates/rpc/tests/runtime_e2e.rs::e2e_client_side_request_identity_is_the_one_the_reply_relates_to`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — die Antwort trägt die Identität des Request-Samples, das der Reader
des Replier geliefert hat; ein Request ohne Identität am DATA wird in Rust
gezählt (`Replier::unidentified_requests`), nicht beantwortet.

### §7.8.2 RELATED_SAMPLE_IDENTITY_INVALID

**Spec:** §7.8.2, S. 58 — "The RELATED_SAMPLE_IDENTITY_INVALID constant is used
to indicate an invalid/non-existing sample identity. [...]
RELATED_SAMPLE_IDENTITY_INVALID is defined as { GUID_UNKNOWN ,
SEQUENCENUMBER_UNKNOWN } both of which are defined in the [RTPS]
specification."

**Repo:** `crates/rpc/src/common_types.rs::SampleIdentity::RELATED_SAMPLE_IDENTITY_INVALID`
(`SampleIdentity::UNKNOWN` ist ein Alias);
`runtime/cpp/include/dds/rpc/base.hpp::related_sample_identity_invalid`;
`runtime/java/src/main/java/org/omg/dds/rpc/SampleIdentity.java::RELATED_SAMPLE_IDENTITY_INVALID`.
Commits: `099e1d0fd595`, `15cbcb328c65`

**Tests:** `crates/rpc/src/common_types.rs::tests::sample_identity_invalid_constant_is_rtps_unknown`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm `rpc_runtime_roundtrip.cpp`: „the invalid related identity has SEQUENCENUMBER_UNKNOWN {-1, 0}“),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm `RpcRuntimeRoundtrip.java`: „SEQUENCENUMBER_UNKNOWN.high is -1“ und „SEQUENCENUMBER_UNKNOWN.low is 0“)

**Status:** `done` — alle drei Laufzeiten haben die Konstante mit
`SEQUENCENUMBER_UNKNOWN = {high = -1, low = 0}`; C++ und Java halten den
„nicht gesetzt“-Wert (Folgenummer 0) davon getrennt. Die Programme von C++
(c9a42b1c29af) und Java prüfen den Wert von `related_sample_identity_invalid()`
und `RELATED_SAMPLE_IDENTITY_INVALID`; das Java-Programm prüft zusätzlich die
Oktette im `ReplyHeader`. Die Programme sind im Code gelesen; ein Lauf auf dem
Linux-Testhost nach diesen Commits steht aus.

### §7.8.2.1 Identität des Requests am Service abrufen

**Spec:** §7.8.2.1, S. 58 — "This specification provides language bindings
(7.11 Language Bindings) to retrieve the sample identity of the request in the
form of a SampleIdentity object."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Replier::receive_requests`,
`Sample::identity`; gefüllt von `Reader::take` in
`runtime/cpp/include/dds/rpc/zerodds.hpp`);
`runtime/java/src/main/java/org/omg/dds/rpc/Sample.java` (`identity`;
`TakenSample.identity` aus `SampleImpl.identity()` der Java PSM in
`ZeroDdsParticipant.Reader.take`); Rust:
`crates/rpc/src/replier.rs::PendingRequest::identity`. Commits: `e143bb6bfa0b`
(C++ Klassen), `a8058d1272c9` (C++), `97c20505e85c` und `830064b55c76` (Java),
`2933b7d28a88` (Rust)

**Tests:** `crates/rpc/tests/cpp_rpc_zerodds_related_identity.rs::the_zerodds_binding_sends_and_reads_the_related_sample_identity`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/ZeroDdsParticipantTest.java::aWriteReturnsItsIdentityAndTheReaderSeesItWithTheRelatedOne`,
`crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`

**Status:** `done` — C++ und Java (die Sprachbindungen der Spec) liefern die Identität
des Requests am `Sample`; die Rust-Laufzeit liefert sie am `PendingRequest`.

### §7.8.2.1 Identität des Requests in der Rust-Laufzeit

**Spec:** §7.8.2.1, S. 58 — wie oben.

**Repo:** `crates/rpc/src/replier.rs` (`ReplierHandler::handle` erhält nur den Wert;
`AsyncReplierHandler::on_request(replier, request, identity)` und
`PendingRequest::identity` tragen ein `RequestIdentity`);
`crates/rpc/src/request_identity.rs::RequestIdentity`. Commit: `2933b7d28a88`

**Tests:** `crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`,
`crates/rpc/src/replier.rs::tests::dispatch_async_hands_the_replier_and_the_identity_to_the_listener`

**Status:** `done` — der synchrone Listener der Rust-Laufzeit (Weg 1) erhält nur den Wert
des Requests, die Wege 2 und 3 erhalten die Identität. Die Spec definiert die
Operation für C++ und Java; Rust ist keine ihrer Sprachbindungen. Die
Moduldoku von `request_identity.rs` nennt eine Funktion
`current_request_identity`, die es nicht gibt.

### §7.8.2.2 Verwandte Identität am Client aus SampleInfo

**Spec:** §7.8.2.2, S. 58 — "This specification provides language bindings
(7.11 Language Bindings) to retrieve the related sample identity of the reply
from the reply SampleInfo in the form of a SampleIdentity object."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Sample::related_identity` aus
`SampleInfo::related_identity`; gefüllt von `Reader::take` in
`runtime/cpp/include/dds/rpc/zerodds.hpp` aus dem Parameter, in der Basic-Abbildung
von `Requester::decode_reply` aus dem `ReplyHeader`);
`runtime/java/src/main/java/org/omg/dds/rpc/Sample.java` (`relatedIdentity`;
`TakenSample.relatedIdentity` aus `SampleImpl.relatedIdentity()` der Java PSM).
Commits: `a8058d1272c9` (C++), `97c20505e85c` und `830064b55c76` (Java)

**Tests:** `crates/rpc/tests/cpp_rpc_zerodds_related_identity.rs::the_zerodds_binding_sends_and_reads_the_related_sample_identity`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/ZeroDdsParticipantTest.java::aWriteReturnsItsIdentityAndTheReaderSeesItWithTheRelatedOne`

**Status:** `done` — C++ und Java liefern die verwandte Identität am Reply-Sample aus
der Information des Samples; in der Enhanced-Abbildung kommt sie aus
`PID_RELATED_SAMPLE_IDENTITY`.

### §7.8.2.2 Verwandte Identität in der Rust-Laufzeit

**Spec:** §7.8.2.2, S. 58 — wie oben.

**Repo:** `crates/dcps/src/subscriber.rs::DataReader::take_with_wire`
(`SampleWire::related_sample_identity`);
`crates/rpc/src/requester.rs::Requester::tick` (ordnet Replies der
Enhanced-Abbildung über die verwandte Identität zu;
`Requester::uncorrelated_replies` zählt Replies ohne sie). Commits:
`84416dc1319c`, `3d4c42cad194`

**Tests:** `crates/dcps/src/subscriber.rs::tests::take_with_wire_hands_over_the_identity_and_the_related_identity`,
`crates/rpc/src/requester.rs::tests::an_enhanced_reply_is_correlated_by_its_related_identity`,
`crates/rpc/src/requester.rs::tests::an_enhanced_reply_without_a_related_identity_is_counted`

**Status:** `done` — `SampleInfo` der DCPS-Schicht hat kein Feld dafür; die Laufzeit
liest sie aus `SampleWire`. Der Rust-`Requester` gibt dem Aufrufer die
Nutzlast des Replies, den er über diese Identität dem Request zugeordnet hat.
Die Spec definiert die Operation für C++ und Java; Rust ist keine ihrer
Sprachbindungen.

### §7.8.2.3 Verwandte Identität beim Senden setzen

**Spec:** §7.8.2.3, S. 58 — "Before sending a reply, the service implementation
needs to set the related sample identity of the reply sample. [...] This
specification provides language bindings to set the related sample identity
when sending the reply to a request."

**Repo:** `runtime/cpp/include/dds/rpc/core.hpp::Replier::send_reply` (mit
`related_request_id`; `Writer::write_related` in
`runtime/cpp/include/dds/rpc/zerodds.hpp`);
`runtime/java/src/main/java/org/omg/dds/rpc/Replier.java::sendReply` (mit
`relatedRequestId`; `DataWriter.write(sample, related)`); Rust:
`crates/rpc/src/replier.rs::Replier::send_reply` (mit `RequestIdentity`) und
`crates/dcps/src/runtime.rs::DcpsRuntime::write_user_sample_related`. Commits:
`a8058d1272c9` (C++), `97c20505e85c` und `830064b55c76` (Java), `2933b7d28a88`
(Rust)

**Tests:** `crates/rpc/tests/cpp_rpc_zerodds_related_identity.rs::the_zerodds_binding_sends_and_reads_the_related_sample_identity`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`,
`crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`

**Status:** `done` — der Replier setzt die Identität des Requests beim Senden: in der
Enhanced-Abbildung als `PID_RELATED_SAMPLE_IDENTITY`, in der Basic-Abbildung
im `ReplyHeader`.

---

## §7.9 Service Lifecycle

### §7.9.1 Aktivierung durch Erzeugen von Service oder Replier

**Spec:** §7.9.1, S. 59 — "In Basic and Enhanced Service Mappings, when using
the function-call style language binding, creating an instance of the Service
type activates the service. Likewise, when using the request/reply style
language binding, creating an instance of a Replier activates the service."

**Repo:** `crates/rpc/src/replier.rs::Replier::new`;
`crates/lang-rules/src/emit/interface_rust.rs` (Dispatcher mit `serve`);
`runtime/cpp/include/dds/rpc.hpp::Replier` (Konstruktor mit `ReplierParams`);
`crates/lang-rules/src/emit/interface_cpp/calls.rs` und
`crates/lang-rules/src/emit/interface_java/calls.rs` (`<I>Service` über einem
`Replier`, im Konstruktor angelegt). Commits: `24e3443ce819` (C++),
`c4a249bd251f` (Java)

**Tests:** `crates/rpc/src/replier.rs::tests::replier_new_creates_endpoints`,
`crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`

**Status:** `done` — das Erzeugen des `Replier`, von `serve` (Rust) und von `<I>Service`
(C++, Java) aktiviert den Dienst.

### §7.9.1 Aktivierung legt DataReader und DataWriter an

**Spec:** §7.9.1, S. 59 — "Service activation shall result in creation of the
underlying DataReader and DataWriter entities and they shall be enabled."

**Repo:** `crates/rpc/src/replier.rs::Replier::from_topics`
(`create_datareader`, `create_datawriter`);
`runtime/cpp/include/dds/rpc.hpp::Replier`;
`runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java`

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(Programm `function_call_roundtrip.cpp`: „creating the service creates its DataReader and DataWriter“)

**Status:** `done` — die Entitäten werden beim Erzeugen angelegt und sind aktiv.

### §7.9.1 Aktivierter Dienst ist auffindbar

**Spec:** §7.9.1, S. 59 — "An activated service shall be discoverable."

**Repo:** `crates/rpc/src/replier.rs::Replier::from_topics` (Topics unter dem
Typnamen `${I}_Request` und `${I}_Reply`; in der Enhanced-Abbildung mit der
Discovery-Erweiterung §7.6.2.1)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc-rust-e2e/tests/enhanced_e2e.rs::the_client_waits_for_the_service_instance_it_names`,
`crates/rpc-rust-e2e/tests/lifecycle_e2e.rs::closing_a_service_deletes_its_entities`

**Status:** `done` — der Client findet den aktivierten Dienst über die Discovery
(`service_matched`); die Tests laufen mit Client und Dienst auf einem
Teilnehmer.

### §7.9.2.1 Function-call: Aufruf der Implementierung

**Spec:** §7.9.2.1, S. 59 — "Concrete implementations of the interface
operations shall be invoked as a consequence of receiving requests. The
operations must return a reply consistent with the language binding."

**Repo:** `crates/lang-rules/src/emit/interface_rust.rs` (`dispatcher_impl`,
`dispatcher_handle_fn`); `crates/lang-rules/src/emit/interface_cpp/calls.rs`
und `crates/lang-rules/src/emit/interface_java/calls.rs` (`<I>Service`, ruft
die Implementierung und übersetzt, was sie wirft, in einen Remote-Exception-Code).
Commits: `24e3443ce819` (C++), `c4a249bd251f` (Java)

**Tests:** `crates/rpc-rust-e2e/tests/e2e.rs::the_generated_calculator_binding_runs_every_call`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_service_translates_what_the_implementation_throws`

**Status:** `done` — in Rust, C++ und Java ruft der erzeugte Dienst die Implementierung
der Operation auf und gibt die Antwort zurück.

### §7.9.2.2 Drei Wege der Bearbeitung

**Spec:** §7.9.2.2, S. 59 — "The request/reply style language binding provides
three mutually exclusive ways to process requests."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die drei Wege folgen als Items.

### §7.9.2.2 Weg 1: synchroner Listener

**Spec:** §7.9.2.2, S. 59 — "1. A synchronous service listener callback can be
installed during service activation. The synchronous callback must return the
reply before the callback returns. It is invoked for each received request."

**Repo:** `crates/rpc/src/replier.rs` (`ReplierHandler`, übergeben an `Replier::new`;
`Replier::tick`; `Replier::activate`);
`runtime/cpp/include/dds/rpc.hpp` (`SimpleReplierListener`,
`Replier::dispatch_requests`, Aufruf bei Ankunft über `Replier::Impl::listen`);
`runtime/java/src/main/java/org/omg/dds/rpc/SimpleReplierListener.java`
(`ListenerDelivery.ON_ARRIVAL`). Commits: `c6c468fbbe42` (Rust),
`e530fc03ed85` (C++), `97c20505e85c` (Java)

**Tests:** `crates/rpc/src/replier.rs::tests::tick_processes_request_and_writes_reply`,
`crates/rpc-rust-e2e/tests/lifecycle_e2e.rs::a_call_is_answered_without_anyone_ticking`,
`crates/rpc/tests/cpp_rpc_zerodds_listener.rs::the_zerodds_reader_calls_its_callback_on_arrival_and_not_after_removal`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — der Listener wird beim Erzeugen übergeben, gibt die Antwort zurück
und wird bei Ankunft jedes Requests aufgerufen (Rust: `activate`; Java: je
nach `ListenerDelivery`, Vorgabe `ON_ARRIVAL`).

### §7.9.2.2 Weg 2: asynchroner Listener mit dem Replier

**Spec:** §7.9.2.2, S. 59 — "2. An asynchronous service listener callback can
be installed during service activation. The asynchronous callback does not
return the reply. The callback receives a handle to the Replier instance and
the Replier API can be used to retrieve the requests and send the replies."

**Repo:** `crates/rpc/src/replier.rs` (`AsyncReplierHandler`,
`Replier::activate_async`, `Replier::dispatch_async`);
`runtime/cpp/include/dds/rpc.hpp` (`ReplierListener`);
`runtime/java/src/main/java/org/omg/dds/rpc/ReplierListener.java`. Commits:
`2933b7d28a88` und `c6c468fbbe42` (Rust), `e530fc03ed85` (C++),
`97c20505e85c` (Java)

**Tests:** `crates/rpc/src/replier.rs::tests::dispatch_async_hands_the_replier_and_the_identity_to_the_listener`,
`crates/rpc-rust-e2e/tests/lifecycle_e2e.rs::the_asynchronous_listener_is_called_as_the_request_arrives`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — der Callback erhält den Replier und gibt keine Antwort zurück; in
allen drei Laufzeiten.

### §7.9.2.2 Weg 3: Requests über die API des Replier abholen

**Spec:** §7.9.2.2, S. 59 — "3. The Replier API may be used to retrieve the
requests and send replies."

**Repo:** `crates/rpc/src/replier.rs` (`Replier::receive_requests`,
`Replier::send_reply`, `Replier::send_exception`);
`runtime/cpp/include/dds/rpc.hpp` (`Replier::receive_requests`,
`Replier::send_reply`);
`runtime/java/src/main/java/org/omg/dds/rpc/Replier.java` (`receiveRequests`,
`sendReply`). Commits: `2933b7d28a88` (Rust), `e143bb6bfa0b` (C++),
`97c20505e85c` (Java)

**Tests:** `crates/rpc/src/replier.rs::tests::receive_requests_and_send_reply_expose_the_identity`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — die drei Laufzeiten haben das Abholen der Requests und das Senden
der Replies. Dass die Namen der Operationen von denen der normativen
C++-Datei abweichen (`take_request`, `read_request`, `wait_for_requests`),
steht im Item „§7.11.1 Maschinenlesbare Dateien für C++“.

### §7.9.2.2 Listener werden bei Ankunft eines Requests aufgerufen

**Spec:** §7.9.2.2, S. 59 — wie die drei Items oben.

**Repo:** `crates/rpc/src/replier.rs::Replier::activate`, `Replier::activate_async`
(Listener am Request-Reader);
`runtime/cpp/include/dds/rpc.hpp::Replier` (Konstruktor ruft
`Impl::listen`; `Replier::on_arrival`);
`runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java` und
`ListenerDelivery.java` (`ON_ARRIVAL` über
`DataReader.setDataAvailableListener`). Commits: `c6c468fbbe42` (Rust),
`e530fc03ed85` (C++), `97c20505e85c` (Java)

**Tests:** `crates/rpc-rust-e2e/tests/lifecycle_e2e.rs::a_call_is_answered_without_anyone_ticking`,
`crates/rpc-rust-e2e/tests/lifecycle_e2e.rs::the_asynchronous_listener_is_called_as_the_request_arrives`,
`crates/rpc/tests/cpp_rpc_zerodds_listener.rs::the_zerodds_reader_calls_its_callback_on_arrival_and_not_after_removal`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm `RpcRuntimeRoundtrip.java`, Abschnitt über die Java PSM: „the replier listener ran on arrival“)

**Status:** `done` — in allen drei Laufzeiten ruft die Laufzeit den Listener bei Ankunft
eines Requests auf; die Anwendung muss `tick()`, `dispatch_requests()` oder
`dispatchRequests()` nicht aufrufen. Java kennt zusätzlich
`ListenerDelivery.ON_PUMP` für eine Anbindung, die nicht benachrichtigen kann.

### §7.9.3 Deaktivierung löscht DataReader und DataWriter

**Spec:** §7.9.3, S. 59 — "Deleting (subject to language binding) the service
instance deactivates the service. In Java language binding, Closeable.Close()
deactivates the service. In C++ language binding, the destructor of the service
implementation shall deactivate the service. Deactivation shall delete the
underlying DDS entities (i.e., DataReader and DataWriter only)."

**Repo:** `crates/rpc/src/replier.rs::Replier::close` (der letzte Handle löscht die
Entitäten);
`runtime/cpp/include/dds/rpc.hpp::Replier::close` (gibt Reader und Writer frei;
der Destruktor der Anbindung ruft `zerodds_writer_destroy` und
`zerodds_reader_destroy`);
`runtime/java/src/main/java/org/zerodds/ddsrpc/ReplierImpl.java::close` und
`runtime/java/src/main/java/org/zerodds/ddsrpc/RequesterImpl.java::close` (`DomainParticipant.deleteWriter`,
`deleteReader`). Commits: `c6c468fbbe42` (Rust), `24e3443ce819` (C++),
`b272ea523d51` und `97c20505e85c` (Java)

**Tests:** `crates/rpc-rust-e2e/tests/lifecycle_e2e.rs::closing_a_service_deletes_its_entities`,
`crates/rpc/src/requester.rs::tests::duplicate_instance_name_freed_after_drop`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(Programm `function_call_roundtrip.cpp`: „destroying the last copy deletes the DataReader and the DataWriter“),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm `RpcRuntimeRoundtrip.java`: „closing the service deleted its replier's DataWriter and DataReader“)

**Status:** `done` — Deaktivierung löscht in Rust, C++ und Java DataReader und DataWriter
und sonst keine Entität. Der Rust-Test prüft, dass der Peer das Match
verliert.

### §7.9.3 Deaktivierung in Java und im erzeugten C++

**Spec:** §7.9.3, S. 59 — wie oben.

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/RPCEntity.java` (`Closeable`);
`runtime/java/src/main/java/org/zerodds/ddsrpc/ZeroDdsParticipant.java`
(`deleteWriter`, `deleteReader` schließen die Entitäten der Java PSM);
`runtime/cpp/include/dds/rpc.hpp::ServiceEndpoint::close`;
`crates/lang-rules/src/emit/interface_cpp/calls.rs` (`<I>Service` hält den
`Replier`, der Destruktor gibt ihn frei). Commits: `b272ea523d51`,
`24e3443ce819` (C++), `c4a249bd251f` (Java), `830064b55c76`

**Tests:** `crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — `close()` in Java und der Destruktor oder `close()` der erzeugten
C++-Klassen löschen DataReader und DataWriter. Die Programme der Tests
zählen die gelöschten Entitäten einer aufzeichnenden Anbindung; die
Löschung der Entitäten der Java PSM durch `ZeroDdsParticipant` ist aus dem Code
gelesen (`psm.close()`), nicht von einem Test geprüft.

---

## §7.10 Service QoS

### §7.10 QoS-Annotation am Interface

**Spec:** §7.10, S. 59 — "Both Basic and Enhanced Service Mappings allow QoS
annotation with an interface."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung; die Annotation steht in §7.10.1.

### §7.10.1 Annotation @Qos mit RequestProfile und ReplyProfile

**Spec:** §7.10.1, S. 59–60 — "To set the specific Qos for an interface, the
annotation @Qos shall be used. It is defined as follows. @annotation local
interface Qos { attribute string RequestProfile; attribute string
ReplyProfile; };"

**Repo:** `crates/rpc/src/annotations.rs` (`lower_single`, Zweig `"Qos"`;
`RpcAnnotation::Qos`; `LoweredRpc::qos_profiles`); `@Qos` ist als Annotation
der Spec vordefiniert (Commit `3c21f462c6e2`);
`crates/lang-rules/src/adapter/interface.rs` (`qos_request_profile`,
`qos_reply_profile`);
`crates/lang-rules/src/emit/interface_rust.rs::declared_qos_fn`
(`new_with_declared_qos`, `serve_with_declared_qos`);
`crates/lang-rules/src/emit/interface_cpp/calls.rs::defaults_method`;
`crates/lang-rules/src/emit/interface_java/calls.rs` (`qosProfiles`).
`@RPCInterfaceQos(profile = …)` bleibt als Erweiterung neben `@Qos`. Commits:
`fe0931797640` (Rust), `24e3443ce819` (C++), `c4a249bd251f` (Java)

**Tests:** `crates/rpc/src/annotations.rs::tests::the_spec_annotations_need_no_declaration`,
`crates/rpc/src/qos_profile.rs::tests::the_request_and_the_reply_profile_apply_to_their_own_endpoints`,
`crates/lang-rules/tests/interface_rust_binding.rs::the_qos_the_interface_declares_is_applied_by_a_constructor`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm `rpc_runtime_roundtrip.cpp`: Profile der Params)

**Status:** `done` — `@Qos` mit `RequestProfile` und `ReplyProfile` wird erkannt und
erreicht die Endpunkte in Rust, C++ und Java; ein fehlendes `ReplyProfile`
lässt beide Richtungen aus dem `RequestProfile` bauen.

### §7.10.1 Profil als URL, file:// Pflicht, #library_name

**Spec:** §7.10.1, S. 60 — "The profile attributes are string URLs of the form:
<protocol>://path/resource where protocol, “:”, “//” and path are optional.
Support for the file:// protocol is mandatory. If the protocol:// is omitted,
the string attributes are interpreted as if the protocol is file://. The URL
may end with an optional library name. The profile filename and library_name
are separated by a #."

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::from_qos_url` (`split_qos_url`);
`crates/zerodds-c-api/src/rpc_profile_ffi.rs::zerodds_endpoint_qos_from_profile_url`
(C++: `runtime/cpp/include/dds/rpc/zerodds.hpp`);
`runtime/java/src/main/java/org/omg/dds/rpc/RpcQos.java::fromUrl`. Commits:
`1f980bd7b223` (Rust), `c6d883cd6e1e` (c-api), `cee4ca1b418b` (C++),
`97c20505e85c` (Java)

**Tests:** `crates/rpc/src/qos_profile.rs::tests::split_qos_url_defaults_the_protocol_to_file`,
`crates/rpc/src/qos_profile.rs::tests::from_qos_url_reads_the_file_and_applies_the_library`,
`crates/zerodds-c-api/tests/rpc_qos.rs::a_profile_url_resolves_the_named_library_over_the_rpc_defaults`,
`crates/rpc/tests/cpp_rpc_zerodds_qos.rs::the_zerodds_binding_carries_the_rpc_qos_to_the_c_api`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm `RpcRuntimeRoundtrip.java`: `RpcQos.fromUrl`)

**Status:** `done` — `file://` wird gelesen, ohne Protokoll gilt `file://`, `#` trennt
Dateinamen und Bibliothek; ein anderes Protokoll ist ein Fehler, kein Abruf.

### §7.10.1 Profile im Format von DDS4CCM

**Spec:** §7.10.1, S. 60 — "The QoS profiles are XML QoS Profiles as defined in
the [DDS4CCM] specification."

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::from_xml_profile` und
`RpcQos::from_qos_url` (Profile aus `zerodds-xml`, DDS-XML 1.0:
`<dds><qos_library><qos_profile>`); Commit: `f1a7a3ea5731`

**Tests:** `crates/rpc/src/qos_profile.rs::tests::from_xml_profile_overrides_history_depth`,
`crates/rpc/src/qos_profile.rs::tests::from_xml_profile_follows_base_name_chain`,
`crates/rpc/tests/runtime_e2e.rs::e2e_qos_profile_from_xml_drives_history_depth`

**Status:** `done` — DDS4CCM 1.1 §7.4.2.3 setzt `<qos_profile>` unmittelbar unter `<dds>` und kennt die Kurzform eines einzelnen `<datawriter_qos name="…">` (§7.4.2.3.3); `qos_library` kommt dort nicht vor. DDS-XML 1.0 kennt diese Form nicht, `crates/xml` führt sie deshalb als Vendor-Form: der Standard lehnt sie mit einem Fehler ab, der sie nennt, `VendorForms::Accept` liest sie in die Bibliothek `CCM_LIBRARY` (`(dds)`) und meldet jede (`crates/xml/src/qos_parser.rs::parse_ccm_top_level_profile`). `RpcQos::from_qos_url` und `zerodds_endpoint_qos_from_profile_url` lesen die Dateien mit dieser Einstellung, weil §7.10.1 DDS4CCM als Format nennt. Commit `ebebeff79b79`. Tests: `crates/xml/src/zerodds_xml.rs::tests::ccm_profiles_directly_under_dds_are_a_vendor_form`, `a_qos_library_and_ccm_profiles_can_share_a_document`, `crates/rpc/src/qos_profile.rs::tests::from_qos_url_reads_a_ddscm_profile_file`, `crates/zerodds-c-api/tests/rpc_qos.rs` (auf dem Linux-Testhost gelaufen für `crates/xml`).

### §7.10.2 Vorgaben ohne Angabe

**Spec:** §7.10.2, S. 60 — "When no QoS is specified, default QoS are in
effect. [...] The users may modify the mutable QoS policies at run-time."

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::default_basic`;
`runtime/cpp/include/dds/rpc/base.hpp::EndpointQos`;
`runtime/java/src/main/java/org/omg/dds/rpc/RpcQos.java::defaults`

**Tests:** `crates/rpc/src/qos_profile.rs::tests::default_is_basic`,
`crates/rpc/src/qos_profile.rs::tests::default_basic_matches_spec_710_2`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm: „the defaults are §7.10.2's“),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm: „§7.10.2: RELIABLE, KEEP_ALL, VOLATILE“)

**Status:** `done` — ohne Angabe gelten in Rust, C++ und Java die Vorgaben der drei
Richtlinien (folgende Items). Die Änderung zur Laufzeit steht im nächsten
Item.

### §7.10.2 Änderbare QoS zur Laufzeit

**Spec:** §7.10.2, S. 60 — wie oben.

**Repo:** `crates/rpc/src/replier.rs::Replier::set_qos`,
`crates/rpc/src/requester.rs::Requester::set_qos`;
`runtime/cpp/include/dds/rpc.hpp` (`Requester::set_qos`, `Replier::set_qos`,
`ServiceEndpoint::set_qos`) über `zerodds_writer_set_qos` und
`zerodds_reader_set_qos` der C-API;
`runtime/java/src/main/java/org/omg/dds/rpc/Requester.java::setQos`,
`runtime/java/src/main/java/org/omg/dds/rpc/Replier.java::setQos`. Commits: `6e3bbd2758fd` (Rust), `cee4ca1b418b` (C++),
`731994c71c51` (c-api: nur DEADLINE und LIFESPAN änderbar), `97c20505e85c`
(Java), `4884eac92a83` (Java: `ZeroDdsParticipant` reicht die Änderung an die PSM weiter)

**Tests:** `crates/rpc/tests/runtime_e2e.rs::e2e_set_qos_changes_the_mutable_policies_at_run_time`,
`crates/zerodds-c-api/tests/rpc_qos.rs::the_mutable_policies_of_a_live_endpoint_change_and_the_immutable_ones_are_refused`,
`crates/rpc/tests/cpp_rpc_zerodds_qos.rs::the_zerodds_binding_carries_the_rpc_qos_to_the_c_api`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** `done` — Rust und C++ ändern die änderbaren Richtlinien und verweigern die unveränderbaren. In Java gibt `ZeroDdsParticipant.Writer.setQos` (und `Reader.setQos`) die geänderte QoS an `DataWriterImpl.setQos` bzw. `DataReaderImpl.setQos` der PSM weiter; die PSM wendet die Änderung einer änderbaren Richtlinie (DEADLINE, LIFESPAN) an und weist eine unveränderbare mit `ImmutablePolicyException` ab (`QosRules.checkChange`, `2e8078b94b5d`), was die Laufzeit als `RemoteUnsupportedError` meldet. Test: `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::overTheJavaPsm` (DEADLINE ändert sich, RELIABILITY nicht; braucht ein JDK). Der frühere Satz, die PSM verweigere jede Änderung, galt vor `2e8078b94b5d`.

### §7.10.2 Reliability RELIABLE

**Spec:** §7.10.2, S. 60 — "Reliability DDS_RELIABLE_RELIABILITY_QOS".

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::default_basic`;
`runtime/cpp/include/dds/rpc/base.hpp::EndpointQos`;
`runtime/java/src/main/java/org/omg/dds/rpc/RpcQos.java::defaults`

**Tests:** `crates/rpc/src/qos_profile.rs::tests::default_basic_matches_spec_710_2`,
`crates/rpc/src/qos_profile.rs::tests::writer_qos_carries_reliability_and_history`,
`crates/rpc/src/qos_profile.rs::tests::reader_qos_carries_reliability`

**Status:** `done` — Vorgabe `RELIABLE` in Rust, C++ und Java.

### §7.10.2 History KEEP_ALL

**Spec:** §7.10.2, S. 60 — "History DDS_KEEP_ALL_HISTORY_QOS".

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::default_basic`,
`RpcQos::default_enhanced` (`DEFAULT_HISTORY_QOS`);
`runtime/cpp/include/dds/rpc/base.hpp::EndpointQos`;
`runtime/java/src/main/java/org/omg/dds/rpc/RpcQos.java::defaults`. Commit:
`4e526346c547`

**Tests:** `crates/rpc/src/qos_profile.rs::tests::default_basic_matches_spec_710_2`,
`crates/rpc/src/qos_profile.rs::tests::default_enhanced_uses_the_same_history`,
`crates/rpc/src/qos_profile.rs::tests::writer_qos_carries_reliability_and_history`

**Status:** `done` — Vorgabe `KEEP_ALL` in Rust, C++ und Java.

### §7.10.2 Durability VOLATILE

**Spec:** §7.10.2, S. 60 — "Durability DDS_VOLATILE_DURABILITY_QOS".

**Repo:** `crates/rpc/src/qos_profile.rs::RpcQos::default_basic`;
`runtime/cpp/include/dds/rpc/base.hpp::EndpointQos`;
`runtime/java/src/main/java/org/omg/dds/rpc/RpcQos.java::defaults`

**Tests:** `crates/rpc/src/qos_profile.rs::tests::default_basic_matches_spec_710_2`

**Status:** `done` — Vorgabe `VOLATILE` in Rust, C++ und Java.

---

## §7.11 Language Bindings

### §7.11 Figure 7.5 — Vererbung der RPC-Entitäten

**Spec:** §7.11, S. 60, Figure 7.5 — `RPCEntity` ist Basis von `Server`,
`ServiceEndpoint`, `Replier` und `ServiceProxy`; `ServiceProxy` ist Basis von
`Requester` und `ClientEndpoint`; `ServiceEndpoint` von `Service`;
`ClientEndpoint` von `Client`.

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`RPCEntity`, `ServiceProxy`, `Requester`,
`Replier`, `ServiceEndpoint`, `ClientEndpoint`, `Service`, `Client`);
`runtime/java/src/main/java/org/omg/dds/rpc/` (`RPCEntity`, `ServiceProxy`,
`Requester`, `Replier`, `ServiceEndpoint`, `ClientEndpoint`, `Service`,
`Client`). Commit: `e143bb6bfa0b` (C++)

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`,
`crates/rpc/tests/class_summaries_are_declared.rs::the_java_package_declares_every_type_of_the_two_summaries`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm `rpc_runtime_roundtrip.cpp`: `static_assert(std::is_base_of<…>)` für alle Beziehungen der Figur),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `Server` und `ClientEndpoint`)

**Status:** `done` — C++: `rpc_runtime_roundtrip.cpp` prüft beim Übersetzen jede
Beziehung der Figur mit `std::is_base_of`, einschließlich `Server`
(`cf75b773f40b`). Java: `RpcInheritanceTest.java` prüft die Basisschnittstellen
der Figur Glied für Glied (`ae439a055394`).

### §7.11 Server als RPCEntity

**Spec:** §7.11, S. 60, Figure 7.5 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Server` (`class Server : public RPCEntity`,
`is_null()`, `close()`, `run()`, `run(Duration)`);
`runtime/java/src/main/java/org/omg/dds/rpc/Server.java`
(`Server extends RPCEntity, Runnable`; `run`, `getDefaultServiceParams`,
`isNull`, `close`). Commits: `cf75b773f40b` (C++), `998e1fc5e576` (Java)

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm `rpc_runtime_roundtrip.cpp`: `static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::Server>)`),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: „Server extends RPCEntity“, „Server extends Runnable“, `run`, `getDefaultServiceParams`, `isNull`, `close`)

**Status:** `done` — `Server` ist in C++ und Java ein `RPCEntity`, wie die normativen
Dateien `function_call.h` und `runtime/java/src/main/java/org/omg/dds/rpc/Server.java` es deklarieren. C++:
`is_null()`, `close()`, `run()` und `run(Duration)`; `add` weist einen Dienst ab,
den ein anderer `Server` hält (`IllegalOperationError`). Java: `ServerImpl.add`
weist `null`, einen geschlossenen und einen schon gebundenen Dienst ab. Die
Tests sind im Code gelesen; ein Lauf auf dem Linux-Testhost nach diesen
Commits steht aus.

### §7.11.1 Maschinenlesbare Dateien für C++

**Spec:** §7.11.1, S. 61 — "The machine readable files associated with this
specification represent the normative language bindings for C++."

**Repo:** `runtime/cpp/include/dds/rpc/normative/` (die drei Header unverändert), `runtime/cpp/include/dds/rpc/vendor_dependent.h`, `runtime/cpp/include/dds/rpc/request_reply.hpp`, `runtime/cpp/include/dds/rpc/function_call.hpp`, `runtime/cpp/include/dds/rpc/core.hpp`, `runtime/cpp/include/dds/rpc/base.hpp`, `runtime/cpp/include/dds/rpc.hpp`; verglichen mit den Headern `request_reply`,
`function_call` und `sample` aus `omg-dds-rpc-cxx.zip`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm `rpc_runtime_roundtrip.cpp`, Namespace `signatures`),
`crates/rpc/tests/cpp_normative_headers.rs::the_shipped_headers_are_the_omg_files`,
`crates/rpc/tests/cpp_normative_headers.rs::the_normative_headers_compile_against_vendor_dependent_h`,
`crates/rpc/tests/cpp_normative_headers.rs::a_program_against_the_normative_headers_runs_against_libzerodds`

**Status:** done — `request_reply.h`, `function_call.h` und `sample.h` aus `omg-dds-rpc-cxx.zip` liegen unverändert in `runtime/cpp/include/dds/rpc/normative/` (`NOTICE`, SHA-256 in `omg-dds-rpc-cxx.sha256`). `vendor_dependent.h` füllt, was die Header „vendor dependent“ nennen (`OMG_DDS_RPC_REFERENCE_IMPLEMENTATION`, `dds::dds_entity_traits`, `details::vendor_dependent<T>`, Vorwärtsdeklarationen). `request_reply.hpp` und `function_call.hpp` definieren die Klassen der Header als Handles auf die Implementierungsklassen in `dds::rpc::core` (`core.hpp`, `base.hpp`); `rpc.hpp` ist die Schicht darüber (`Service`, `Client`, `RPCRuntime`, `add(Server&, …)`), und die erzeugten Header binden sie ein (`48637795f336`, `2226f63a15da`, `cae4d8ec831b`, `99ad7c1ed406`). Gemessen auf dem Linux-Testhost: die Übersetzungseinheit aus beiden normativen Headern, die vorher 254 Fehler hatte, übersetzt mit `-Wall -Wextra -Werror`; ein Programm, das gegen die normativen Header geschrieben ist (`normative_roundtrip.cpp`), führt Request/Reply (`Requester`, `Replier`) und einen Funktionsaufruf (`ClientEndpoint`/`ServiceEndpoint` des erzeugten Clients und Dienstes) gegen `libzerodds` aus (`182962baecf5`). Die SHA-256-Summen der drei Dateien stehen in `omg-dds-rpc-cxx.sha256`; der Vergleich mit dem Zip läuft, wo `ZD_DDS_RPC_CXX_ZIP`, `internal/standards/cache/omg/` oder `~/.cache/zerodds-spec/` es nennen; ob es auf dem Testhost dort lag, wurde nicht geprüft. Zwei Fehler der OMG-Header werden umgangen, nicht geändert: der private Destruktor von `ListenerBase` (beim Einlesen steht vor `virtual` ein `public:`, `vendor_dependent.h`) und `RequesterParams` vor seiner Deklaration. Die Header geben `Server` kein `add`, `run_once` und `size`; `dds::rpc::add`, `run_once` und `size` in `rpc.hpp` ergänzen sie.

### §7.11.1.1.1 Request/Reply: IDL-Strukturen des Benutzers oder synthetisierte

**Spec:** §7.11.1.1.1, S. 61 — "The request-reply style language binding for
the Basic Service Mapping shall use either the user-defined IDL structs directly
or the structs synthesized from an IDL interface."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Requester<TReq, TRep>`,
`Replier<TReq, TRep>` über `dds_type_traits`);
`crates/lang-rules/src/emit/interface_cpp.rs::requester_alias`. Commit:
`d047dc5f497c`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_binds_the_requester_and_replier_of_seven_eleven_one_one_one`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — `Requester` und `Replier` nehmen die Strukturen des Benutzers und
die synthetisierten über `dds_type_traits`; der Generator bindet beide
(`CalculatorRequester`, `CalculatorReplier`).

### §7.11.1.1.1 Abbildung von IDL nach C++ nach [IDL35]

**Spec:** §7.11.1.1.1, S. 61 — "The IDL shall be mapped to C++ types as defined
in [IDL35] (including the vendor-specific mappings for DDS v1.3)."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs` (Vorgabe: eigene Abbildung);
`crates/lang-rules/src/adapter/cpp_psm.rs` (`--cpp-mapping psm`: Abbildung
nach DDS-PSM-Cxx §8.1); `tools/idlc/src/main.rs` (Option `--cpp-mapping`).
Commit: `56ff5b5f25ff`

**Tests:** `tools/idlc/src/generate.rs::tests::the_psm_mapping_is_a_layer_over_the_native_types`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`

**Status:** done — Die Voreinstellung ist `--cpp-mapping psm` (`CppMapping::Psm` in `adapter/cpp.rs`); `--cpp-mapping native` ist eine benannte Einstellung. Die Klassen `<I>`, `<I>Async`, Client und Dienst schreiben die Typen von DDS-PSM-Cxx Table 7.1 (`std::wstring`, `std::map`, `::dds::core::array`) in den Namespaces der IDL und wandeln über `zerodds::psm::Conv` in die Topic-Typen, die nativ in `zd_native` bleiben (`68e7459fb040`, Test `interface_cpp_psm_types.rs::the_topic_types_are_native_and_the_classes_are_the_applications`). Die `future`-Typen der `_async`-Operationen halten die Typen der Anwendung, in der Basic- und in der Enhanced-Abbildung (`38c035d26d8d`). Die C++-Tests dieses Pakets laufen unter der Voreinstellung (`cpp_function_call`, `cpp_enhanced_function_call`, `cpp_rpc_real_library`, `cpp_rpc_runtime`). Nicht geprüft: [IDL35] liegt nicht im Cache; die Klausel nennt für die Typen DDS-PSM-Cxx Table 7.1.

### §7.11.1.1.1 Templates für Senden, Empfangen, Anlegen, Initialisieren, Zerstören

**Spec:** §7.11.1.1.1, S. 61 — "The Requester, Replier, Sample, SampleRef,
WriteSample, and WriteSampleRef templates provide a uniform interface to send,
receive, allocate, initialize, and destroy data."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Sample`, `SampleRef`, `WriteSample`,
`WriteSampleRef`, `Requester`, `Replier`)

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — die sechs Templates sind vorhanden und tragen Senden
(`Requester::send_request`), Empfangen (`Replier::receive_requests`,
`Requester::receive_reply`), Anlegen und Initialisieren (Konstruktoren) und
Zerstören (Destruktoren). Der Abgleich der einzelnen Operationen mit
`sample.h` steht im Item „§7.11.1 Maschinenlesbare Dateien für C++“.

### §7.11.1.1.2 Function-call: Topic-Typen nach DDS-Cxx-PSM

**Spec:** §7.11.1.1.2, S. 61 — "The function-call style language binding for
the Basic Service Mapping shall use the structs (and unions) synthesized from
the user-defined IDL interface [...] The topic-types shall be mapped to C++
types as per defined in [DDS-Cxx-PSM] with the following additional rules".

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::declare`;
`crates/lang-rules/src/adapter/cpp.rs::cpp_value_type`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_topic_types_of_seven_five`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`

**Status:** done — Ganzzahlen, `string`, `sequence`, `T[N]`, `wstring` und `map` stehen in der Voreinstellung wie in DDS-Cxx-PSM Table 7.1 (`56ff5b5f25ff`, `68e7459fb040`; Test `interface_cpp_psm_types.rs::the_interface_spells_the_types_of_table_7_1`). Die Voreinstellung ist `--cpp-mapping psm` (`CppMapping::Psm` in `adapter/cpp.rs`); `--cpp-mapping native` ist eine benannte Einstellung. Die Topic-Typen bleiben nativ (`zd_native`), die Futures der `_async`-Operationen halten die Typen der Anwendung (`38c035d26d8d`).

### §7.11.1.1.2 Regel 1: ${interface} und ${interface}Async im Namespace des Moduls

**Spec:** §7.11.1.1.2, S. 61 — "1. Each IDL interface shall map to an abstract
class named “${interface}” and an abstract class named “${interface}Async” in
the same namespace corresponding to the module of the interface. If there is no
module, no C++ namespace is generated."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`sync_interface`,
`async_interface`). Commit: `d047dc5f497c`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`

**Status:** `done` — zwei abstrakte Klassen im Namespace des Moduls; ohne Modul kein
Namespace.

### §7.11.1.1.2 Regel 2: öffentlicher virtueller Destruktor

**Spec:** §7.11.1.1.2, S. 61 — "2. Both “${interface}” and “${interface}Async”
abstract classes shall have a public virtual destructor."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`sync_interface`,
`async_interface`). Commit: `d047dc5f497c`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`

**Status:** `done` — `virtual ~<I>() = default;` und `virtual ~<I>Async() = default;`.

### §7.11.1.1.2 Regel 3: SupportType, RequestType, ReplyType

**Spec:** §7.11.1.1.2, S. 61 — "3. Both “${interface}” and “${interface}Async”
abstract classes shall have public SupportType, RequestType, and ReplyType
typedefs for “${interface}Support,” “${interface}_Request,” and
“${interface}_Reply” types."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`sync_interface`,
`async_interface`, `support_class`). Commit: `d047dc5f497c`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`

**Status:** `done` — beide Klassen haben `SupportType`, `RequestType` und `ReplyType`.

### §7.11.1.1.2 Regel 4: AsyncInterfaceType

**Spec:** §7.11.1.1.2, S. 61 — "4. The “${interface}” abstract class shall
contain a public AsyncInterfaceType typedef for “${interface}Async.”"

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::sync_interface`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`

**Status:** `done`

### §7.11.1.1.2 Regel 5: InterfaceType

**Spec:** §7.11.1.1.2, S. 61 — "5. The “${interface}Async” abstract class shall
contain a public InterfaceType typedef for “${interface}.”"

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_interface`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`

**Status:** `done`

### §7.11.1.1.2 Regel 6: rein virtuelle Funktionen für Operationen und Attribute

**Spec:** §7.11.1.1.2, S. 61 — "6. The “${interface}” abstract class shall
contain public pure virtual functions for all the operations and attributes
defined in the IDL using the following rules".

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`sync_interface`, `sync_method`, `attribute_method`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_names_an_attribute_accessor_after_the_attribute`

**Status:** `done`

### §7.11.1.1.2 Regel 6a: Name der Operation

**Spec:** §7.11.1.1.2, S. 61 — "a. The name of the function shall be same as
the name of the IDL operation."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::sync_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`

**Status:** `done`

### §7.11.1.1.2 Regel 6b: Zahl und Reihenfolge der Argumente

**Spec:** §7.11.1.1.2, S. 61 — "b. The number and the order of the arguments
shall be as defined in the IDL."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::sync_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`

**Status:** `done` — Regel 6h stellt bei konstruierter Rückgabe `cxx_return` vor die Argumente.

### §7.11.1.1.2 Regel 6c: keine Exception-Spezifikation

**Spec:** §7.11.1.1.2, S. 61 — "c. The functions shall not have any exception
specification even if the IDL operation has an exception specification."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::sync_method` (`is_noexcept: false`, kein `throw(...)`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`

**Status:** `done`

### §7.11.1.1.2 Regel 6d: einfache Typen und Container nach DDS-Cxx-PSM §7.4.2

**Spec:** §7.11.1.1.2, S. 61 — "d. The mapping of IDL primitive and container
types to C++ types is provided in sub clause 7.4.2 in [DDS-Cxx-PSM]."

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::cpp_value_type` und `crates/lang-rules/src/adapter/cpp_psm.rs::psm_type`, aufgerufen aus `crates/lang-rules/src/emit/interface_cpp.rs::user_type` und `sync_method`; Commits `d047dc5f497c`, `68e7459fb040`, `0fc844e3e5fd`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_interface_spells_the_types_of_table_7_1`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_bodies_convert_between_the_psm_types_and_the_topic_structures`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`

**Status:** done — `std::wstring`, `std::map` und `::dds::core::array` stehen in den Argumenten, Rückgaben und Attributen der synchronen Schnittstelle; das ist die Voreinstellung `--cpp-mapping psm`, `--cpp-mapping native` ist eine benannte Einstellung (`68e7459fb040`, Test `interface_cpp_psm_types.rs::the_interface_spells_the_types_of_table_7_1`; die C++-Läufe unter der Voreinstellung: `cpp_function_call`, `cpp_enhanced_function_call`).

### §7.11.1.1.2 Regel 6e: Übergabe nach DDS-Cxx-PSM §7.4.5

**Spec:** §7.11.1.1.2, S. 61 — "e. The mapping of In, Out, and InOut primitive
and constructed types (e.g., struct) is provided in sub clause 7.4.5 in
[DDS-Cxx-PSM]."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`passed`, `is_native`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`

**Status:** `done`

### §7.11.1.1.2 Regel 6f: Getter und Setter nach DDS-Cxx-PSM §7.4.6

**Spec:** §7.11.1.1.2, S. 61 — "f. Getter/Setter functions for attributes in an
interface is specified in sub clause 7.4.6 in [DDS-Cxx-PSM]."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::attribute_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_names_an_attribute_accessor_after_the_attribute`

**Status:** `done`

### §7.11.1.1.2 Regel 6g: einfache Typen und Aufzählungen als Wert

**Spec:** §7.11.1.1.2, S. 61 — "g. IDL operations that return primitive types
and enumeration types the corresponding C++ function shall return the C++ type
by value".

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::sync_method` (`is_native`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`

**Status:** `done`

### §7.11.1.1.2 Regel 6h: konstruierte Rückgabe als cxx_return

**Spec:** §7.11.1.1.2, S. 61 — "h. IDL operations that return constructed type
(e.g., struct) shall map to the first parameter with name “cxx_return” of type a
T& to the function and the function shall return void."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::sync_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`

**Status:** `done` — Zeichenketten und Folgen behandelt ZeroDDS wie konstruierte Typen; die Regeln 6g und 6h nennen sie nicht.

### §7.11.1.1.2 Regel 7: rein virtuelle asynchrone Funktionen

**Spec:** §7.11.1.1.2, S. 62 — "7. The “${interface}Async” abstract class shall
contain asynchronous public pure virtual functions for all the operations and
attributes defined in the IDL using the following rules".

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`async_interface`, `async_method`, `async_attribute_method`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 Regel 7a: Name ${operation}_async

**Spec:** §7.11.1.1.2, S. 62 — "a. The name of the function shall be
“${operation}_async”."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 Regel 7b: nur In und InOut

**Spec:** §7.11.1.1.2, S. 62 — "b. The function shall accept only In and InOut
parameters and shall have no Out parameters."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method` (`operation.inputs()`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 Regel 7c: keine Exception-Spezifikation

**Spec:** §7.11.1.1.2, S. 62 — "c. The functions shall not have any exception
specification even if the IDL operation has an exception specification."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 Regel 7d: einfache Typen und Container, alle const

**Spec:** §7.11.1.1.2, S. 62 — "d. The mapping of IDL primitive and container
types to C++ types is provided in sub clause 7.4.2 in [DDS-Cxx-PSM]. All the
arguments shall be const."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method` (Argumente über `user_type`); Commits `d047dc5f497c`, `68e7459fb040`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_futures_hold_the_applications_types`

**Status:** done — alle Argumente sind `const`, mit den Typen von Table 7.1 in der Voreinstellung `--cpp-mapping psm`, und `future<…>` hält die Typen der Anwendung (`050e54b2b089`, `38c035d26d8d`; Tests `interface_cpp_psm_types.rs::the_futures_hold_the_applications_types` und `::the_enhanced_futures_hold_the_applications_types`).

### §7.11.1.1.2 Regel 7e: In und InOut nach §7.4.5, alle const

**Spec:** §7.11.1.1.2, S. 62 — "e. The mapping of In and InOut primitive and
constructed types (e.g.,struct) is provided in sub clause 7.4.5 in
[DDs-Cxx-PSM]. All the arguments shall be const."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 Regel 7f: Getter liefert future des Attributtyps

**Spec:** §7.11.1.1.2, S. 62 — "f. Attribute getter functions shall take no
parameters and shall return dds::rpc::future of the same type as the
attribute."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`async_interface`, `async_attribute_method`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_names_an_attribute_accessor_after_the_attribute`

**Status:** `done`

### §7.11.1.1.2 Regel 7g: Setter liefert future<void>

**Spec:** §7.11.1.1.2, S. 62 — "g. Attribute setter functions shall take a
parameter as per specified in sub clause 7.4.6 in [DDS-Cxx-PSM]. It shall
return a dds::rpc::future<void>."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`async_interface`, `async_attribute_method`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_names_an_attribute_accessor_after_the_attribute`

**Status:** `done`

### §7.11.1.1.2 Regel 7h: Rückgabe als future des Typs

**Spec:** §7.11.1.1.2, S. 62 — "h. Operation that produce a return value in the
form of a regular return (primitive or constructed type) shall return a value of
dds::rpc::future of that type."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method`; Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 Regel 7i: future der Out-Struktur oder future<void>

**Spec:** §7.11.1.1.2, S. 62 — "i. Operation that produces return values with
one or more Out/InOut parameters shall return a value of dds::rpc::future of
“${interface}_${operation}_Out” type. Otherwise, it will return
dds::rpc::future<void>."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::async_method` (`async_payload`, `has_outputs`); Commit `d047dc5f497c`.

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`

**Status:** `done`

### §7.11.1.1.2 — Abweichungen von Table 7.1

**Spec:** §7.11.1.1.2, S. 61–62 — Regeln 6d und 7d; DDS-Cxx-PSM §7.4.2,
Table 7.1: "string<Char32> std::wstring", "map<K, V> std::map<K, V>",
"T[N] dds::core::array<T, N>".

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`user_type`, `converted`, `async_payload`), `crates/lang-rules/src/adapter/cpp_psm.rs::psm_type`, `crates/cpp/include/zerodds/psm/convert.hpp` (`zerodds::psm::Conv`); Commits `56ff5b5f25ff`, `68e7459fb040`, `0fc844e3e5fd`.

**Tests:** `crates/lang-rules/tests/interface_cpp_psm_types.rs::the_interface_spells_the_types_of_table_7_1`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_native_mapping_keeps_the_native_spelling`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_bodies_convert_between_the_psm_types_and_the_topic_structures`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_header_compiles`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`

**Status:** done — In der Voreinstellung `--cpp-mapping psm` schreiben abstrakte Schnittstelle, Client und Dienst die Zeilen von Table 7.1 (`std::wstring`, `std::map`, `::dds::core::array`) und wandeln in den Rümpfen mit `zerodds::psm::Conv` in die Topic-Typen. Die Futures der `_async`-Funktionen und der Attribut-Zugriffe halten die Typen der Anwendung, die Out-Struktur ist die der Anwendung, in der Basic-Abbildung (`050e54b2b089`, `6d6d02eb4209`) und in der Enhanced-Abbildung (`38c035d26d8d`, Test `interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`; der Linux-Testhost übersetzt beide Fassungen in `the_header_compiles`). `--cpp-mapping native` schreibt die nativen Typen und ist eine benannte Einstellung.

### §7.11.1.1.2 Regel 8: Präfix cxx_ für reservierte Wörter

**Spec:** §7.11.1.1.2, S. 62 — "8. Any identifier (e.g., parameter, operation
name, exception, interface, module) in an IDL file that is a reserved word in
C++ shall be prefixed by “cxx_”."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs::cxx_escape` (ruft `crates/lang-rules/src/adapter/cpp.rs::cxx_name`); Commits `d047dc5f497c` (Interface, Operation, Parameter), `f8678b4486c1` (Exception), `2d3ebbfa06dc` (Module, Typen).

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_prefixes_a_reserved_word_with_cxx`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_names_a_reserved_exception_with_the_cxx_prefix`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_names_a_reserved_module_and_type_with_the_cxx_prefix`

**Status:** `done` — Interface, Operation, Parameter, Exception, Modul, deklarierter Typ, Mitglieder, Aufzählungswerte und Konstanten erhalten `cxx_` (Item „Mitglieder, Aufzählungswerte und Konstanten“ unten, Commit `569f84427581`).

### §7.11.1.1.2 Regel 8 — Mitglieder, Aufzählungswerte und Konstanten

**Spec:** §7.11.1.1.2, S. 62 — wie oben.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs::cxx_name`, aufgerufen für Mitglieder (Struktur, Union, Exception, Bitset), Aufzählungswerte und Konstanten; Commit `569f84427581`.

**Tests:** `crates/lang-rules/tests/cxx_prefix_members.rs::a_member_that_is_a_reserved_word_is_prefixed_by_cxx`,
`crates/lang-rules/tests/cxx_prefix_members.rs::an_enumerator_and_a_constant_that_are_reserved_words_are_prefixed_by_cxx`,
`crates/lang-rules/tests/cxx_prefix_members.rs::the_codec_refers_to_the_prefixed_member`,
`crates/lang-rules/tests/cxx_prefix_members.rs::the_idl_names_table_maps_the_prefixed_name_back_to_the_idl_spelling`

**Status:** `done` — `long static;` wird `std::int32_t cxx_static;`; Aufzählungswerte und Konstanten erhalten `cxx_`. Codec und die Tabellen `_idl_names` verwenden den Namen mit Präfix und führen die IDL-Schreibweise zurück.

### §7.11.1.2 Enhanced: Request/Reply

**Spec:** §7.11.1.2, S. 62 — "The request-reply style language binding for the
Enhanced Service Mapping shall use either the user-defined IDL structs directly
or the structs synthesized from an IDL interface. The IDL shall be mapped to C++
types as defined in [DDS-Cxx-PSM]."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Requester<TReq, TRep>`, `Replier<TReq, TRep>`); `crates/lang-rules/src/emit/interface_cpp/enhanced.rs`; Commits `99bda16e455b`, `e143bb6bfa0b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/lang-rules/tests/enhanced_cpp_calls.rs::the_enhanced_header_compiles`

**Status:** done — die Templates nehmen die Strukturen der Enhanced-Abbildung (`enhanced.rs` erzeugt sie, `Correlation::SAMPLE_IDENTITY`). Die Abbildung der IDL-Typen auf C++ folgt DDS-Cxx-PSM Table 7.1 in der Voreinstellung `--cpp-mapping psm` (Item „§7.11.1.1.2 — Abweichungen von Table 7.1“).

### §7.11.1.2 Enhanced: Function-call nach §7.11.1.1.2

**Spec:** §7.11.1.2, S. 62 — "The function-call style language binding for the
Enhanced Service Mapping shall use the mapping rules defined in 7.11.1.1.2
Function-Call Style Language Binding."

**Repo:** `crates/lang-rules/src/emit/interface_cpp.rs` (`sync_interface`, `async_interface`), `crates/lang-rules/src/emit/interface_cpp/calls.rs`; Commit `99bda16e455b`.

**Tests:** `crates/lang-rules/tests/enhanced_cpp_calls.rs::the_function_call_classes_are_written_over_the_choice_structures`,
`crates/lang-rules/tests/enhanced_cpp_calls.rs::a_derived_interface_answers_the_inherited_operation_itself`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`,
`crates/lang-rules/tests/interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`

**Status:** done — abstrakte Klassen, Client und Dienst gibt es für die Enhanced-Abbildung. Die Regeln 6d und 7d gelten wie in der Basic-Abbildung: Table 7.1 in der Voreinstellung `--cpp-mapping psm`, und `future<…>` hält die Typen der Anwendung (`38c035d26d8d`, Test `interface_cpp_psm_types.rs::the_enhanced_futures_hold_the_applications_types`).

### §7.11.1.3 Lokale Exceptions nach DDS-Cxx-PSM §7.5.5

**Spec:** §7.11.1.3, S. 62 — "The C++ language binding may throw locally
generated and remotely generated exceptions. As such, all DDS exceptions defined
in sub clause 7.5.5 in [DDS-Cxx-PSM] are valid exceptions."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`LocalError` und zehn abgeleitete Klassen: `NullReferenceError`, `AlreadyClosedError`, `IllegalOperationError`, `PreconditionNotMetError`, `OutOfResourcesError`, `TimeoutError`, `ImmutablePolicyError`, `InconsistentPolicyError`, `UnsupportedError`, `InvalidArgumentError`); Commit `e143bb6bfa0b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — lokale Fehler werfen diese Klassen in `dds::rpc`, benannt nach den Klassen aus DDS-Cxx-PSM §7.5.5 (`e143bb6bfa0b`). Liegt `crates/cpp/include` auf dem Pfad (`ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION`), erbt jede Klasse von der gleichnamigen Klasse in `dds::core` (über die virtuelle Basis `dds::core::Exception`; `crates/cpp/include/dds/core/Exception.hpp` erbt dazu virtuell): ein `catch (const dds::core::TimeoutError&)` fängt `dds::rpc::TimeoutError`, ebenso `catch (const dds::core::Exception&)` (`404a34c295b2`). Ein Status der C-API ohne zugeordnete Klasse wirft `UnspecifiedError`. Test: `static_assert`s und `catch`-Fälle in `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp`, gefahren von `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together` (Linux-Testhost, grün). Ohne `crates/cpp/include` auf dem Pfad gibt es die Klassen von DDS-Cxx-PSM nicht, und die Basis ist `std::runtime_error`.

### §7.11.1.3 Tabelle: Codes auf Exceptions

**Spec:** §7.11.1.3, S. 63 — "REMOTE_EX_OK No exception;
REMOTE_EX_UNSUPPORTED dds::rpc::RemoteUnsupportedError;
REMOTE_EX_INVALID_ARGUMENT dds::rpc::RemoteInvalidArgumentError;
REMOTE_EX_OUT_OF_RESOURCES dds::rpc::RemoteOutOfResourcesError;
REMOTE_EX_UNKNOWN_OPERATION dds::rpc::RemoteUnknownOperationError;
REMOTE_EX_UNKNOWN_EXCEPTION dds::rpc::RemoteUnknownExceptionError".

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::raise_remote_exception`; `crates/rpc/src/language_binding.rs::cpp_exception_class`; Commits `d047dc5f497c`, `099e1d0fd595` (sechs Codes).

**Tests:** `crates/rpc/src/language_binding.rs::tests::the_table_of_seven_eleven_one_three_is_the_published_one`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done`

### §7.11.1.3 Figure 7.6 — Vererbung der Exceptions

**Spec:** §7.11.1.3, S. 63, Figure 7.6 — `RemoteOutOfResourcesError` unter
`std::runtime_error`; `RemoteUnsupportedError`, `RemoteUnknownOperationError`,
`RemoteUnknownExceptionError` unter `std::logic_error`;
`RemoteInvalidArgumentError` unter `std::invalid_argument`; die Klassen der
rechten Spalte zusätzlich unter `dds::core::Exception`.

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`RemoteOutOfResourcesError`, `RemoteUnsupportedError`, `RemoteInvalidArgumentError`, `RemoteUnknownOperationError`, `RemoteUnknownExceptionError`, `detail::ExceptionRoot`); Commits `d047dc5f497c`, `cf75b773f40b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — die Basen aus `std::` stimmen; alle fünf Klassen erben zusätzlich von `detail::ExceptionRoot`, das mit `dds/core/Exception.hpp` auf dem Include-Pfad `dds::core::Exception` ist (`static_assert(std::is_base_of<…>)` in `check_exception_hierarchy` von `rpc_runtime_roundtrip.cpp`; `cpp_rpc_runtime.rs` übergibt `crates/cpp/include`).

### §7.11.1.3 Basis dds::core::Exception

**Spec:** §7.11.1.3, S. 63, Figure 7.6 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`detail::ExceptionRoot`, `ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION`, `LocalError`); `crates/cpp/include/dds/core/Exception.hpp`; Commit `cf75b773f40b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — `rpc.hpp` bindet `dds/core/Exception.hpp` ein, wenn `__has_include` sie findet; dann ist `detail::ExceptionRoot` gleich `dds::core::Exception`. Ohne die PSM-Header steht dort eine Klasse gleicher Form (`what()`). `check_exception_hierarchy` prüft die fünf `Remote*Error` und `LocalError` mit `static_assert`.

### §7.11.1.4.1 Namespaces dds und dds::rpc mit 22 Klassen

**Spec:** §7.11.1.4.1, S. 64 — "The dds, and dds::rpc namespaces define the
classes and functions for the request-reply style language bindings.
Specifically, the language binding includes RPCEntity, Requester, Replier,
ServiceProxy, ListenerBase, SimpleReplierListener, ReplierListener,
SimpleRequesterListener, RequesterListener, RequesterParams, ReplierParams,
future, shared_future, Sample, SampleRef, WriteSample, WriteSampleRef,
LoanedSamples, SharedSamples, SampleIterator, dds_type_traits, and
dds_entity_traits."

**Repo:** `runtime/cpp/include/dds/rpc.hpp`; `crates/rpc/src/language_binding.rs` (`CPP_NAMESPACES`, `CPP_REQUEST_REPLY_CLASSES`); Commits `d047dc5f497c`, `e143bb6bfa0b`.

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`

**Status:** `done`

### §7.11.1.4.1 Bindung unabhängig vom Profil

**Spec:** §7.11.1.4.1, S. 64 — "C++ request-reply language binding uses
templates. [...] Depending upon the profile in use (Basic/Enhanced), the actual
structure of the TReq and TRep types will vary. However, the request-reply
language binding is independent of the profile in use."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Requester<TReq, TRep>`, `Replier<TReq, TRep>`, `Correlation`); Commits `d047dc5f497c`, `99bda16e455b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_enhanced_function_call.rs::the_generated_enhanced_client_calls_the_generated_enhanced_service`

**Status:** `done` — dieselben Templates tragen die Strukturen der Basic-Abbildung (`Correlation::HEADER`) und der Enhanced-Abbildung (`Correlation::SAMPLE_IDENTITY`).

### §7.11.1.4.2 RPCEntity mit close() und is_null()

**Spec:** §7.11.1.4.2, S. 64 — "RPCEntity is the base abstract class extended
by all the active RPC entities. It supports close() and is_null() operations."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::RPCEntity` (abgeleitet: `ServiceProxy`, `ServiceEndpoint`, `Server`); Commits `d047dc5f497c`, `cf75b773f40b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — `close()` und `is_null()` sind rein virtuell in `RPCEntity`; `Requester`, `Replier`, `ServiceEndpoint` und `Server` erben davon (`static_assert(std::is_base_of<dds::rpc::RPCEntity, …>)` in `rpc_runtime_roundtrip.cpp`).

### §7.11.1.4.3 Requester sendet Requests und empfängt Replies

**Spec:** §7.11.1.4.3, S. 64 — "A requester sends requests and receives
replies."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Requester`; Commit `d047dc5f497c`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done`

### §7.11.1.4.3 Requester ist Referenztyp

**Spec:** §7.11.1.4.3, S. 64 — "Requester is a reference type and when copied
it makes a shallow copy."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Requester` (`std::shared_ptr<Impl>`, `ServiceProxy::Binding`); Commit `e143bb6bfa0b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — eine Kopie teilt `Impl` (Writer, Reader, Antworten, offene `future`) und die Bindung an die Instanz; `close()` an der Kopie schließt für alle.

### §7.11.1.4.3 Konfiguration über RequesterParams

**Spec:** §7.11.1.4.3, S. 64 — "An instance of a Requester is configured at the
time of construction using RequesterParams, which is a container of
configuration parameters, such as domain participant, QoS, listeners and more."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`EndpointParams`, `RequesterParams`, `Requester(const RequesterParams&)`); Commits `e143bb6bfa0b`, `cee4ca1b418b` (QoS), `e530fc03ed85` (Listener).

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_rpc_zerodds_qos.rs::the_zerodds_binding_carries_the_rpc_qos_to_the_c_api`

**Status:** `done` — `RequesterParams` trägt Participant, Publisher, Subscriber, Namen der Topics, Namen des Dienstes und der Instanz, `RpcQos` oder Profil-URLs und einen Listener.

### §7.11.1.4.3 Empfang über Listener, Abfrage und future

**Spec:** §7.11.1.4.3, S. 64 — "Requester allows listener-based, polling-based,
and future-based reception of replies. [...] Future-based notification of
replies is analogous to callback-based notification, however, no request-reply
correlation is necessary because every future represents a reply to a unique
request."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Requester::receive_replies`, `receive_reply`, `send_request_async`, `dispatch_replies`, Konstruktor mit `impl_->listen`); Commits `d047dc5f497c`, `e530fc03ed85` (Listener und `future` bei Ankunft).

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_rpc_zerodds_listener.rs::the_zerodds_reader_calls_its_callback_on_arrival_and_not_after_removal`

**Status:** `done`

### §7.11.1.4.3 Gebundener Requester erreicht nur seine Instanz

**Spec:** §7.11.1.4.3, S. 64 — "A requester reference may be bound to a
specific service instance. Requests sent through a bound requester reference
shall be sent to the bound service instance only."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`ServiceProxy::bind`, `Requester::Impl::send_locked` setzt `RequestHeader::instance_name`, `Replier::Impl::receive_requests` filtert); `crates/rpc/src/replier.rs::Replier::tick`; Commits `d6f6b5e975c8` (Filter), `e143bb6bfa0b` (Bindung geteilt).

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/src/replier.rs::tests::tick_filters_requests_for_other_instance_name`,
`crates/rpc/src/replier.rs::tests::a_named_replier_does_not_answer_a_request_without_an_instance`,
`crates/rpc/tests/runtime_e2e.rs::e2e_instance_routing_filters_per_replier`

**Status:** `done` — Basic-Profil: Der Request trägt den Namen der Instanz im Header; ein Replier mit ungleichem Namen verwirft ihn (strenge Gleichheit, der leere Name passt nur auf den leeren Namen), auch auf dem Weg über `dispatch_requests`. Enhanced-Profil: Item unten.

### §7.11.1.4.3 Bindung an die Instanz im Enhanced-Profil

**Spec:** §7.11.1.4.3, S. 64 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`Requester::Impl::send_locked`, `readers_of_instance`, `DataWriter::write_to`); `runtime/cpp/include/dds/rpc/zerodds.hpp::write_to`; `crates/zerodds-c-api` (`zerodds_writer_write_to_readers`, `zerodds_writer_next_sequence_number`); `crates/dcps` (`DcpsRuntime::write_user_sample_to_readers`); `crates/rtps` (`CacheChange::destination_readers`); Commits `86c89466df19`, `976400f7d40a`, `ce04885de39d`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/zerodds-c-api/tests/write_to_readers.rs::a_sample_written_to_one_reader_reaches_that_reader_only`,
`crates/rtps/src/reliable_writer.rs::tests::a_change_for_one_reader_reaches_the_other_readers_of_its_participant_as_a_gap`

**Status:** `done` — der Request der Enhanced-Abbildung trägt keinen Namen; `Requester::Impl::send_locked` schreibt mit `write_to` nur an die Reader, die den Namen der Instanz angekündigt haben. Die anderen Reader desselben Participants erhalten die Änderung als Gap. `check_instance_addressing` in `rpc_runtime_roundtrip.cpp` prüft zwei benannte Replier und den unbenannten (Schleife im Speicher); `write_to_readers.rs` prüft die Zustellung über die C-API. Grenze: Eine `TRANSIENT_LOCAL`-Aufbewahrung kennt die Adressaten nicht; das RPC-QoS ist `VOLATILE` (§7.10.2). Ein ungebundener Enhanced-Request im Discovery-Modus geht nur an Reader ohne Namen. Ohne Discovery der Erweiterung lehnt der Requester einen gebundenen Request mit `UnsupportedError` ab (aus dem Code gelesen).

### §7.11.1.4.4 ServiceProxy

**Spec:** §7.11.1.4.4, S. 64 — "ServiceProxy class defines type-independent
operations for Requester and the Client. For example, binding to a specific
instance, waiting for an instance to discover, closing the service, and more.
ServiceProxy shall not be instantiated directly."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::ServiceProxy` (`bind`, `unbind`, `wait_for_service`, geschützter Konstruktor); Commits `d047dc5f497c`, `e143bb6bfa0b`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done` — `wait_for_service()` prüft, ob der Dienst entdeckt ist; die Überladung mit Zeitspanne wartet bis dahin.

### §7.11.1.4.5 Replier

**Spec:** §7.11.1.4.5, S. 64 — "A replier receives requests and send replies.
Replier is a reference type and it is inexpensive to copy [...] An instance of
Replier is configured at the time of construction using ReplierParams, which is
a container of configuration parameters such as domain participant, QoS,
listeners and more. Replier allows listener-based and polling-based reception
of requests."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Replier` (`std::shared_ptr<Impl>`, `receive_requests`, `send_reply`, `dispatch_requests`, `set_qos`); Commits `d047dc5f497c`, `e143bb6bfa0b`, `cee4ca1b418b`, `e530fc03ed85`.

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_rpc_zerodds_listener.rs::the_zerodds_reader_calls_its_callback_on_arrival_and_not_after_removal`

**Status:** `done` — Referenztyp mit geteiltem `Impl`; `ReplierParams` trägt Participant, QoS und Listener; Empfang über Listener (bei Ankunft) und Abfrage.

### §7.11.1.4.6 ListenerBase

**Spec:** §7.11.1.4.6, S. 65 — "ListenerBase is a “marker” abstract base class,
which all generic listener abstract classes inherit from. It defines no member
functions."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::ListenerBase` (nur der virtuelle Destruktor); Commit `d047dc5f497c`.

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`

**Status:** `done`
### §7.11.1.4.7 SimpleReplierListener

**Spec:** §7.11.1.4.7, S. 65 — "SimpleReplierListener<TReq, TRep> is used to
provide a synchronous request listener for a Replier. ReplierParams is used to
pass an instance [...] the callback provides the request object and expects the
reply as the return value."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::SimpleReplierListener`
(`process_request`); Commit `d047dc5f497c`, Aufruf bei Ankunft `e530fc03ed85`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** `done`

### §7.11.1.4.8 ReplierListener

**Spec:** §7.11.1.4.8, S. 65 — "ReplierListener<TReq, TRep> interface is used to
provide an asynchronous request listener for a Replier. [...] the callback
provides the Replier object and returns void."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::ReplierListener`
(`on_request_available(Replier&)`); Commit `d047dc5f497c`, Aufruf bei Ankunft
`e530fc03ed85`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.9 SimpleRequesterListener

**Spec:** §7.11.1.4.9, S. 65 — "SimpleRequesterListener<TRep> abstract class is
used to receive notifications of arrival of replies. [...] the callback provides
access to the reply sample and returns void."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::SimpleRequesterListener`
(`process_reply`); Commit `d047dc5f497c`, Aufruf bei Ankunft `e530fc03ed85`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.10 RequesterListener

**Spec:** §7.11.1.4.10, S. 65 — "RequesterListener<TReq,TRep> interface is used
to receive notification of reply arrival. [...] the callback provides the
replier object and returns void."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::RequesterListener`
(`on_reply_available(Requester&)`); Commit `d047dc5f497c`, Aufruf bei Ankunft
`e530fc03ed85`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done — der Rückruf erhält den Requester; die Spec schreibt
„replier object“.

### §7.11.1.4.11 RequesterParams

**Spec:** §7.11.1.4.11, S. 65 — "RequesterParams is a valuetype that serves as a
container of configuration parameters of a Requester."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::RequesterParams`; Commit
`d047dc5f497c`, Form nach §7.11.1.3 bis §7.11.1.5 `e143bb6bfa0b`, QoS
`cee4ca1b418b`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.12 ReplierParams

**Spec:** §7.11.1.4.12, S. 65 — "ReplierParams is a valuetype that serves as a
container of configuration parameters of a Replier."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::ReplierParams`; Commit
`d047dc5f497c`, Form nach §7.11.1.3 bis §7.11.1.5 `e143bb6bfa0b`, QoS
`cee4ca1b418b`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.13 future<T>

**Spec:** §7.11.1.4.13, S. 65 — "In C++11 and C++14 environments,
dds::rpc::future<T> is a typedef for std::future<T>. [...] Future is not
copyable but it is movable."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`using future = std::future<T>`);
Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.14 shared_future<T>

**Spec:** §7.11.1.4.14, S. 65 — "shared_future<T> is closely related to
future<T> and the only difference is that shared_future<T> is copyable and
movable."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`using shared_future =
std::shared_future<T>`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`

**Status:** done

### §7.11.1.4.15 Sample<T>

**Spec:** §7.11.1.4.15, S. 65 — "Sample<T> is a valetype that combines a value of
type T and a value of type SampleInfo. Sample is conceptually immutable."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Sample` (nur lesende Zugriffe);
Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.16 SampleRef<T>

**Spec:** §7.11.1.4.16, S. 65 — "SampleRef<T> is a reference type that combines
a value of type T and a value of type SampleInfo. Copying SampleRef<T> makes a
shall copy."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::SampleRef` (über
`std::shared_ptr<Sample<T>>`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.17 WriteSample<T>

**Spec:** §7.11.1.4.17, S. 66 — "WriteSample<T> is a valuetype that combines a
value of type T and a value of type dds::SampleIdentity. The user populates the
value of T and the middleware populates the value of dds::SampleIdentity. When
a request is sent as a WriteSample, upon function return, the
WriteSample.identity()uniquely identifies the request sent."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`WriteSample`;
`Requester::send_request(WriteSample<TReq>&)`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.18 WriteSampleRef<T>

**Spec:** §7.11.1.4.18, S. 66 — "WriteSampleRef<T> is a reference type, which
groups a reference to T and a value of type dds::SampleIdentity. [...] When a
request is sent as a WriteSampleRef<T>, upon function return, the
WriteSample.identity()uniquely identifies the request sent."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::WriteSampleRef` (über
`std::shared_ptr<WriteSample<T>>`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done — das Senden als `WriteSampleRef` steht im Item unten.

### §7.11.1.4.18 Senden als WriteSampleRef

**Spec:** §7.11.1.4.18, S. 66 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc/core.hpp::Requester::send_request(WriteSampleRef<TReq>)`;
Commit `e143bb6bfa0b`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(Programm `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp`: die Identität steht
nach dem Senden im Sample, auf das die Referenz zeigt)

**Status:** done

### §7.11.1.4.19 LoanedSamples<T>

**Spec:** §7.11.1.4.19, S. 66 — "LoanedSamples<T> is conceptually a container
of loaned Sample<T> from the middleware. LoanedSamples<T> is not copyable but it
is movable. LoanedSamples<T>::value_type is SampleRef<T>. Upon destruction,
LoanedSamples<T> returns the loaned samples to the middleware."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::LoanedSamples` (Kopieren gelöscht,
Verschieben erlaubt); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done — die Samples sind Kopien; zurückzugeben ist nichts.

### §7.11.1.4.20 SharedSamples<T>

**Spec:** §7.11.1.4.20, S. 66 — "All copies of a SharedSamples<T> refer to the
same set of loaned samples. SharedSamples<T>::value_type is SampleRef<T>. [...]
Note that SharedSamples<T> can be obtained from LoanedSample<T> but not vice
versa."

**Repo:** `runtime/cpp/include/dds/rpc.hpp` (`SharedSamples`;
`LoanedSamples::to_shared`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`

**Status:** done

### §7.11.1.4.21 SampleIterator<T>

**Spec:** §7.11.1.4.21, S. 66 — "SampleIterator<T> is a random-access iterator
over LoanedSamples<T> and SharedSamples<T>. SampleIterator<T>::value_type is a
SampleRef<T>."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::SampleIterator`
(`std::random_access_iterator_tag`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.4.22 dds_type_traits<T>

**Spec:** §7.11.1.4.22, S. 66 — "dds_type_traits<T> is a collection of
meta-functions that given a type T, provides commonly needed dependent types,
such as a DataReader for T, DataWriter for T, Sample<T>, LoanedSamples<T>,
etc."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::dds_type_traits_base` (`Seq`, `ConstSeq`,
`DataReader`, `DataWriter`, `SampleRefType`, `LoanedSamplesType`,
`SharedSamplesType`, `iterator`, `const_iterator`); `dds_type_traits<T>` leitet
von `dds_type_traits_base<T>` ab, die erzeugten Spezialisierungen ebenfalls
(`crates/lang-rules/src/emit/type_pair_cpp.rs`,
`crates/lang-rules/src/emit/interface_cpp/codec.rs`,
`crates/lang-rules/src/emit/interface_cpp/enhanced_codec.rs`); Commit
`cf75b773f40b`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(`static_assert` in `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp` auf `Seq`,
`DataReader`, `LoanedSamplesType`, `SharedSamplesType`, `iterator`,
`const_iterator`)

**Status:** done — `DataReader` und `DataWriter` sind die byteorientierten Typen der
Laufzeit (`std::shared_ptr<::dds::rpc::DataReader>`), nicht die Typen einer
DDS-C++-Bibliothek.

### §7.11.1.4.23 dds_entity_traits

**Spec:** §7.11.1.4.23, S. 66 — "dds_entity_traits abstracts over the DDS
entity types and provides consistent syntax to get DDS entity types
irrespective of the DDS C++ language bindings. (e.g., DomainParticipant,
Publisher, Subscriber, DataReaderQos, and DataWriterQos)."

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::dds_entity_traits`
(`DomainParticipant`, `Publisher`, `Subscriber`, `DataWriter`, `DataReader`,
`DataWriterQos`, `DataReaderQos`); Commit `d047dc5f497c`, Publisher, Subscriber
und QoS-Typen `e143bb6bfa0b`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`,
`crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`
(`static_assert` in `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp` auf
`DataWriterQos`, `Publisher`, `Subscriber`)

**Status:** done

### §7.11.1.4.23 dds_entity_traits: Publisher, Subscriber und QoS-Typen

**Spec:** §7.11.1.4.23, S. 66 — wie oben.

**Repo:** `runtime/cpp/include/dds/rpc/base.hpp::dds_entity_traits`; Commit
`e143bb6bfa0b`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done — `Publisher` und `Subscriber` sind benannt, `DataWriterQos` und
`DataReaderQos` sind `EndpointQos`.

### §7.11.1.5 Klassen des Stils Function-call

**Spec:** §7.11.1.5, S. 66 — "Service, ServiceEndpoint, Server, Client, and
ClientEndpoint classes are specific to the function-call style language binding
and are described below."

**Repo:** `runtime/cpp/include/dds/rpc.hpp`;
`crates/rpc/src/language_binding.rs::CPP_FUNCTION_CALL_CLASSES`; Commit
`d047dc5f497c`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`

**Status:** done

### §7.11.1.5.1 Service

**Spec:** §7.11.1.5.1, S. 66 — "Service is a reference type and accepts a
reference to the service implementation. A Service instantiates the underlying
DDS entities and makes the service discoverable. Every Service inherits from
ServiceEndpoint. A Service may belong to only one Server."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Service` (`claim`, `release_claim`);
`crates/lang-rules/src/emit/interface_cpp/calls.rs::service_class` (`<I>Service`
leitet von `dds::rpc::Service` ab, nimmt `<I>&` und `ServiceParams`, legt den
Replier an); Commit `24e3443ce819`, Zugehörigkeit zu einem Server
`cf75b773f40b`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_binds_the_function_call_entities_of_seven_eleven_one_five`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(Programm `crates/rpc/tests/cpp/function_call_roundtrip.cpp`: ein Service, den
ein Server hält, wird von einem zweiten Server und vom selben Server ein
zweites Mal mit `IllegalOperationError` abgewiesen)

**Status:** done — Referenztyp (Kopien teilen die Entitäten), Referenz auf die
Implementierung, Entitäten und Basis `ServiceEndpoint` stimmen; `Server::add`
weist einen Service ab, der schon einem Server gehört.

### §7.11.1.5.2 ServiceEndpoint

**Spec:** §7.11.1.5.2, S. 67 — "A ServiceEndpoint provides type-independent
functions to manage a service (e.g., pause, resume, close, etc.) A
ServiceEndpoint shall not be instantiated directly; it can be obtained from a
Service object. ServiceEndpoint is a reference type."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::ServiceEndpoint` (`pause`, `resume`,
`close`, `set_qos`, geschützter Konstruktor, Zustand über `std::shared_ptr`);
Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`

**Status:** done

### §7.11.1.5.3 Server

**Spec:** §7.11.1.5.3, S. 67 — "A Server is a container of one or more
Services. Server.run() function begins dispatching the requests to the Service
implementation. Server is a reference type."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Server` (`add`, `run_once`, `run()`,
`run(until)`, `run(Duration)`, `close`, `is_null`, `default_service_params`; die
Dienste liegen in einem gemeinsamen Zustand, Kopien teilen ihn; `Server` erbt
von `RPCEntity`); Commit `d047dc5f497c`, Basis, `run()` und Zugehörigkeit
`cf75b773f40b`

**Tests:** `crates/rpc/tests/cpp_rpc_runtime.rs::the_cpp_rpc_runtime_and_the_generated_header_run_together`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(`static_assert(std::is_base_of<dds::rpc::RPCEntity, dds::rpc::Server>)` und
`runner.run()` in `crates/rpc/tests/cpp/function_call_roundtrip.cpp`)

**Status:** done — Container, Verteilung der Requests, `run()` ohne Parameter und die Basis `RPCEntity` stimmen. Der Zustand liegt in einem gemeinsamen Objekt (`core::Server`), das Kopien teilen; `rpc_runtime_roundtrip.cpp` kopiert einen `Server` und lässt die Kopie einen Request beantworten (`79dc3800b8cf`), und prüft, dass ein Dienst eines anderen Servers abgewiesen wird. Unter den normativen Headern ist `Server` das Handle auf `core::Server` (`48637795f336`).

### §7.11.1.5.4 Client

**Spec:** §7.11.1.5.4, S. 67 — "Client is a reference type and provides
functions to invoke operations on a remote service synchronously and
asynchronously. A Client instantiates the underlying DDS entities and makes
them discoverable. Every Client inherits publicly from “${interface}”,
“${interface}Async”, and ClientEndpoint (public virtual)."

**Repo:** `crates/lang-rules/src/emit/interface_cpp/calls.rs::client_class`
(`<I>Client` mit den Basen `<I>`, `<I>Async` und `public virtual
dds::rpc::ClientEndpoint`, Operationen über den Requester, Entitäten im
Konstruktor);
`runtime/cpp/include/dds/rpc.hpp::Client`; Commit `24e3443ce819`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_binds_the_function_call_entities_of_seven_eleven_one_five`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`

**Status:** done — Kopien teilen die Entitäten (Programm
`crates/rpc/tests/cpp/function_call_roundtrip.cpp`).

### §7.11.1.5.5 ClientEndpoint

**Spec:** §7.11.1.5.5, S. 67 — "A ClientEndpoint provides functions to obtain
the underlying DDS entities at the client side. ClientEndpoint inherits from
ServiceProxy. A ClientEndpoint shall not be instantiated directly; it can be
obtained from a Client object."

**Repo:** `runtime/cpp/include/dds/rpc.hpp::ClientEndpoint`
(`get_request_datawriter<TReq>()`, `get_reply_datareader<TRep>()`); Commit
`d047dc5f497c`, Zugriff auf die Entitäten `cf75b773f40b`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_cpp_runtime_declares_every_class_of_the_two_summaries`,
`crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`
(Programm `crates/rpc/tests/cpp/function_call_roundtrip.cpp`: beide Zugriffe
liefern Entitäten, der Writer hat mindestens einen Reader)

**Status:** done — erbt von `ServiceProxy`, geschützter Konstruktor, `close`,
`wait_for_service`, `set_qos`, `qos`, Zugriff auf Request-DataWriter und
Reply-DataReader.

### §7.11.1.5 — Service, Server, ClientEndpoint

**Spec:** §7.11.1.5.1, §7.11.1.5.3, §7.11.1.5.5, S. 66–67 — wie die Items oben.

**Repo:** `runtime/cpp/include/dds/rpc.hpp::Server::add`, `Server::run`,
`ClientEndpoint::get_request_datawriter`;
`runtime/java/src/main/java/org/omg/dds/rpc/Server.java` (`run()`),
`runtime/java/src/main/java/org/omg/dds/rpc/ClientEndpoint.java`
(`requestWriter`, `replyReader`); Commits `cf75b773f40b` (C++),
`998e1fc5e576` (Java)

**Tests:** `crates/rpc/tests/cpp_function_call.rs::the_generated_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `Server` hat `run()`,
`ClientEndpoint` gibt Writer und Reader, ein gebundener Service wird von einem
zweiten Server abgewiesen)

**Status:** done — (1) `Server::add` (C++) weist einen Service ab, der einem Server
gehört (`IllegalOperationError`); die Java-Bindung tut es mit
`IllegalStateException`. (2) `Server` hat `run()` ohne Parameter in C++ und
Java. (3) `ClientEndpoint` gibt in C++ und Java den Request-Writer und den
Reply-Reader heraus.

### §7.11.2 Maschinenlesbare Dateien für Java

**Spec:** §7.11.2, S. 67 — "The machine readable files associated with this
specification represent the normative language bindings for Java."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/`, `crates/rpc/tests/java/omg-dds-rpc-java.signatures` (die Deklarationen der Zip-Datei), `crates/rpc/tests/java/extract_spec_signatures.py`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_binding_has_every_signature_of_the_machine_readable_files` (`SpecSignatureCheck.java`), `crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`, `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::nondataSamples` (`…::the_java_rpc_runtime_and_the_generated_services_run_together`)

**Status:** `done` — `omg-dds-rpc-java.zip` (von `https://www.omg.org/spec/DDS-RPC/20160215/`, in `internal/standards/cache/omg/`, nicht im Repository) ist Deklaration für Deklaration gegen die Laufzeit geprüft: `extract_spec_signatures.py` schreibt die 212 öffentlichen Deklarationen der Dateien (jeder Typ mit Art, Zahl der Typparameter und Obertypen, jede Methode mit Name, `static`, Zahl der Typparameter, Parameter- und Rückgabetyp nach Löschung der Typargumente, die Konstruktoren, die Enum-Konstanten) nach `omg-dds-rpc-java.signatures`, und `SpecSignatureCheck.java` prüft die übersetzte Laufzeit per Reflexion dagegen. Gemessen auf dem Linux-Testhost: vor der Änderung 76 Abweichungen (`4a362a70769`), danach 0 (`02563569a84`). Behoben sind: die Params ohne Typparameter, mit `withDataWriterQos`, `withDataReaderQos`, `withPublisher`, `withSubscriber` und den Gettern, und die generischen Methoden `simpleRequesterListener`/`requesterListener`/`simpleReplierListener`/`replierListener` (Item „§7.11.2 Java: Params und DataWriter-Sichten der PSM“); `Requester.getRequestDataWriter()`, `getReplyDataReader()`, `Replier.getRequestDataReader()`, `getReplyDataWriter()`, `ClientEndpoint.getRequestDataWriter(Class)`, `getReplyDataReader(Class)`, `ServiceEndpoint.getRequestDataReader(Class)` und `getReplyDataWriter(Class)` (dasselbe Item); die Parameter vom Typ `org.omg.dds.core.SampleIdentity` (`receiveReply`, `waitForReplies`, `takeReply`, `takeReplies`, `readReply`, `readReplies`, `sendReply`); `Sample.Iterator` ist ein `org.omg.dds.sub.Sample.Iterator` (§7.11.2.2.7: "the same as in [DDS-Java-PSM]"); die Konstruktoren `RequestHeader()`, `ReplyHeader()` und `RPCRuntime()`; `ServiceProxy.waitForService…` und `waitForServices…` geben nichts zurück, warten, und werfen `TimeoutError`, wenn die Wartezeit abläuft, wie die C++-Bindung (`isServiceMatched()` fragt ohne zu warten). Die Rückgabetypen sind die der Dateien (`825670c769d`): `sendRequest` gibt `org.omg.dds.core.SampleIdentity`, `sendRequestAsync` und die vier `waitFor…Async` geben `java.util.concurrent.Future`, die Operationen, die Requests oder Replies empfangen, nehmen oder lesen, geben `org.omg.dds.sub.Sample.Iterator`; `rpc.Sample` deklariert keinen eigenen `Iterator`, wie in der Datei `Sample.java`. Die Objekte sind die der Laufzeit (`org.omg.dds.rpc.SampleIdentity`, `org.omg.dds.rpc.Future` mit dem Completion-Listener, `org.zerodds.ddsrpc.RpcSampleIterator` mit `rpc.Sample`-Elementen). Ein Rückgabetyp, der nur ein Untertyp ist, zählt der Test als Abweichung (`NARROWER`): gegen die Dateien übersetzter Code fände die Methode nicht, weil ihr Deskriptor den Rückgabetyp nennt. Gemessen: vor der Änderung 17 (`9fc062e79fa`), danach 0. `receiveNondataSamples(true)` gibt bei `Requester` und `Replier` die Samples ohne Daten heraus (`valid_data` false, DDS 1.4 §2.2.2.5.1) — als Sample mit `validData()` false, `getData()` `null` und dem Zustand der Instanz —, ohne die Einstellung nicht (Vorgabe); der Rückgabewert ist die ersetzte Einstellung. Test: `RpcRuntimeRoundtrip.java::nondataSamples` über die PSM: ein gelöschter Reply-Writer hinterlässt beim Requester mit der Einstellung ein Sample ohne Daten im Zustand NOT_ALIVE_DISPOSED (die Vorgabe der WRITER_DATA_LIFECYCLE, autodispose TRUE, DDS 1.4 §2.2.3.21), beim Requester ohne sie nichts; ein gelöschter Request-Writer ebenso beim Replier. Vor der Änderung reichte die Bindung solche Samples nicht weiter, beide Prüfungen schlugen fehl (`9fc062e79fa`). Ein `SimpleReplierListener` wird für ein Sample ohne Daten nicht gerufen, es bleibt für die nehmenden Operationen. Was die Laufzeit über die Dateien hinaus hat (`RpcQos`, `ServiceMapping`, `ServiceMatching`, `WireEncoding`, die Bindungsschnittstelle `org.omg.dds.rpc.DomainParticipant`), prüft der Test nicht.

### §7.5.1.2.6 Java: Paarung der Endpunkte nach Aliasen und Zuweisbarkeit

**Spec:** §7.5.1.2.6, S. 49 — "A client using the base interface can invoke an operation in a service that implements a derived interface as long as the same topic names are used." Und §7.7.2.5.3 und §7.7.2.5.5: Typ eines Parameters und Rückgabetyp ändern nach den Regeln der Zuweisbarkeit.

**Repo:** `crates/java-omgdds/java/src/main/java/org/zerodds/rtps/TypeConsistency.java`,
`crates/java-omgdds/java/src/main/java/org/zerodds/typelookup/CompleteTypes.java`,
`crates/java-omgdds/java/src/main/java/org/zerodds/rtps/RtpsParticipant.java` (`typesMatch`, `askTypeObject`, `matchAgain`),
`crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TopicTypeSupport.java::matchesByTypeInformation`,
`runtime/java/src/main/java/org/zerodds/ddsrpc/RawTypeSupport.java`,
`runtime/java/src/main/java/org/zerodds/ddsrpc/ZeroDdsParticipant.java::describedReaderQos`,
`crates/lang-rules/src/emit/interface_rust/type_information.rs` (`minimal`, `JavaDescription`)

**Tests:** `crates/rpc-rust-e2e/tests/java_peer.rs::a_java_client_of_the_base_calls_a_rust_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::a_rust_client_of_the_base_calls_a_java_service_of_the_derived_interface`,
`crates/rpc-rust-e2e/tests/java_peer.rs::versions::a_changed_parameter_type_does_not_match_the_java_client`,
`crates/rpc-rust-e2e/tests/java_peer.rs::versions::an_old_java_client_calls_an_old_rust_service`,
`crates/rpc-rust-e2e/tests/java_peer.rs::appended_member::a_member_appended_to_a_mutable_structure_matches_the_java_client`,
`crates/rpc-rust-e2e/tests/java_peer.rs::versions::a_return_type_the_old_one_is_not_assignable_to_does_not_match_the_java_client`,
`crates/java-omgdds/java/src/test/java/org/zerodds/rtps/TypeConsistencyTest.java`,
`crates/lang-rules/tests/enhanced_type_information.rs::every_minimal_identifier_the_information_announces_has_its_object`

**Status:** `done` — `RawTypeSupport.matchesByTypeInformation()` lässt die Endpunkte eines beschriebenen Topic-Typs nach XTypes 1.3 §7.6.3.4.2 paaren: gleiche COMPLETE- oder MINIMAL-Kennung, sonst die Zuweisbarkeit des Reader-Typs vom Writer-Typ (§7.2.4.4) auf den COMPLETE-Objekten (`TypeConsistency`, `CompleteTypes`, `TypeAssignability`); ohne `TypeInformation` auf einer Seite entscheiden die Typnamen. Objekte der Gegenstelle holt `RtpsParticipant.askTypeObject` mit `getTypes` (höchstens drei Anfragen je Kennung und Peer; ein Objekt, dessen Serialisierung nicht zu seiner Kennung hasht, wird nicht behalten) und paart nach der Antwort erneut (`95818352de1`). Die Java-Beschreibung trägt die MINIMAL-Objekte, die ihre `TypeInformation` nennt (§7.6.3.3.4, `e6af3ee9150`); vorher fragte der Rust-Peer danach, bekam keine Antwort und lehnte das Paar nach der Wartezeit der TypeLookup ab. Ein Reader eines beschriebenen Typs kündigt ALLOW_TYPE_COERCION an (`64134babe64`); mit der Vorgabe der PSM (EXACT_TYPE, auf der Leitung DISALLOW_TYPE_COERCION) lehnte der Rust-Dienst den Reply-Reader ab. `TypeAssignability` nimmt zwei Strukturen ohne Member als zuweisbar (wie `crates/types`) und trennt `int8`/`uint8` vom `octet`. Gemessen über RTPS gegen Rust-Peers (Linux-Testhost, `java_peer.rs`, 8 Tests): Basis gegen abgeleitet in beiden Richtungen, geänderter Parametertyp und auf `long long` erweiterter Rückgabetyp ohne Match, angehängtes Member mit Match und gelesener Antwort, Kontrolle gleicher Versionen; auf `7484a8f0d46`, vor der Paarung nach Zuweisbarkeit, 5 der 8 rot. Die übrigen Topics der Java-PSM paaren weiter nach dem Typnamen und der TypeConsistencyEnforcement mit PID 0x0072.

### §7.11.2 Java: Params und DataWriter-Sichten der PSM

**Spec:** §7.11.2, S. 67 — "The machine readable files associated with this specification represent the normative language bindings for Java." In `omg-dds-rpc-java.zip` tragen die Params `DataWriterQos`, `DataReaderQos`, `Publisher` und `Subscriber` der DDS-Java-PSM, und `Requester.getRequestDataWriter()`, `getReplyDataReader()`, `Replier.getRequestDataReader()` und `getReplyDataWriter()` geben `DataWriter<TReq>` und die verwandten Sichten.

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/RequesterParams.java` (und `ReplierParams`, `ServiceParams`, `ClientParams`), `runtime/java/src/main/java/org/zerodds/ddsrpc/TopicSample.java`, `RpcTopicTypeSupport.java`, `PsmEndpoint.java`, `RpcTopicTypes.java`, `ZeroDdsParticipant.java`, `crates/java-omgdds/java/src/main/java/org/zerodds/pub/DataWriterImpl.java::writeSerializedIdentified`, `crates/java-omgdds/java/src/main/java/org/zerodds/sub/DataReaderImpl.java::takeSerialized`, `crates/java-omgdds/java/src/main/java/org/zerodds/cdr/TypeSupportResolver.java`

**Tests:** `crates/rpc/tests/java/RpcRuntimeRoundtrip.java::psmEntities` (läuft in `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`), `crates/rpc/tests/java/FunctionCallRoundtrip.java::run` (läuft in `…::the_generated_function_call_client_calls_the_generated_service`), `crates/rpc-rust-e2e/tests/java_peer.rs` (Linux-Testhost, grün)

**Status:** `done` — (`02563569a84`) Die Params tragen keine Typparameter mehr und haben `withDataWriterQos`, `withDataReaderQos`, `withPublisher` und `withSubscriber` mit den Gettern; eine DataWriterQos oder DataReaderQos, die sie nennen, ist die QoS der Entität, wie sie steht, und das `RpcQos`, mit dem die Laufzeit arbeitet, wird daraus abgeleitet; die Entitäten entstehen im genannten Publisher und Subscriber (einer des Teilnehmers, sonst `IllegalArgumentException`). `ServiceParams` und `ClientParams` geben die vier Angaben an ihre Replier und Requester weiter, die Endpunkte der Basis-Interfaces erhalten sie mit `sameSettingsAs`. Die Entitäten der Bindung über die PSM (`ZeroDdsParticipant`) sind `DataWriter<T>` und `DataReader<T>` der PSM, getypt durch den Topic-Typ, auf einem `Topic<T>`, wenn die TypeSupport der Params die Klasse ihres Typs nennt (`TopicSample.classOf`, was jede erzeugte tut): ein über `getRequestDataWriter()` geschriebener Wert ist das Sample des Topics, wie er steht, Kopf eingeschlossen (`TopicSample` setzt im Basic Mapping den Kopf aus dem Member `header` davor und liest ihn dorthin zurück), und die Laufzeit schreibt und liest ihre eigenen Oktette weiter durch dieselben Entitäten (`DataWriterImpl.writeSerializedIdentified`, `DataReaderImpl.takeSerialized` mit den beim Empfang behaltenen Oktetten). Die Typregistrierung aus einer `Class`: `TypeSupport.newTypeSupport(<I>_Request.class, …)` ergibt über `TypeSupportResolver` und `RpcTopicTypes` die Rahmung der Laufzeit statt der reflektiven XCDR2-Codierung. Gemessen (Linux-Testhost): `psmEntities` erzeugt Requester und Replier über `RPCRuntime.getInstance(…)`, `createRequesterParams().withDomainParticipant(…).withPublisher(…).withSubscriber(…).withDataWriterQos(…).withDataReaderQos(…)` und `createRequester(Class, Class, params)`; der Request-Writer liegt im Publisher und hat die LIFESPAN der DataWriterQos, der Reply-Reader liegt im Subscriber und hat KEEP_LAST 11 aus der DataReaderQos; die Topics sind durch `Calculator_Request` und `Calculator_Reply` getypt; ein Request der Laufzeit und seine Antwort laufen durch die getypten Entitäten; ein über `getRequestDataWriter()` geschriebener Request erreicht den Replier unter der Identität seines Kopfes; eine über `getReplyDataWriter()` geschriebene Antwort wird über `getReplyDataReader()` mit Wert und Kopf gelesen; ein über `getRequestDataReader()` genommener Request trägt die Identität, die `sendRequest` zurückgab, und ist danach nicht mehr beim Replier; die aus der Klasse gemachte TypeSupport schreibt dieselben Oktette wie der Requester, und ein Topic daraus trägt einen Request mit Kopf. `FunctionCallRoundtrip.java` prüft die `Class`-Formen von `ClientEndpoint` und `ServiceEndpoint` für das eigene und das Basis-Interface und die Ablehnung einer fremden Klasse. Ohne getypte Entität geben die vier Operationen `null`: bei einer Bindung, die nicht die PSM ist (sie lehnt Publisher, Subscriber und PSM-QoS mit `RemoteUnsupportedError` ab, statt sie zu übergehen), bei einer TypeSupport, die keine Klasse nennt (etwa ein Lambda), und wenn der Teilnehmer das Topic schon unter einem anderen Typ hat. Ein Sample, das der Codec des Topic-Typs nicht lesen kann, hält der getypte Reader ohne Wert (`getData()` ist `null`); die Laufzeit sieht seine Oktette und antwortet nach §7.5.2. Weitere Läufe: Maven `verify` der Java-PSM 925 Tests, 0 Fehler, 3 übersprungen; `java_peer.rs` 2/2 (Java-Client gegen Rust-Dienst und umgekehrt, mit den getypten Entitäten); `lang-rules` `enhanced_java_calls`, `java_corpus_compiles`, `java_psm_mapping`, `type_pair_java` 28 bestanden, 2 ignoriert (die Korpus-Vollläufe, wie vorher).

### §7.11.2 Typabbildung nach DDS-Java-PSM

**Spec:** §7.11.2, S. 67 — "The Java language binding shall use IDL type
mapping as per [DDS-Java-PSM]."

**Repo:** `crates/lang-rules/src/adapter/java.rs::java_member_type` (Abbildung
nach DDS-Java-PSM Clause 9, wählbar mit `idlc generate --lang java
--java-mapping psm`); `crates/lang-rules/src/emit/interface_java.rs`; Commit
`f8b91f3fdc1d`, Interfaces `80e6a9531577`

**Tests:** `crates/lang-rules/tests/java_psm_mapping.rs::an_aggregate_is_a_final_class_with_private_state_and_bean_accessors`,
`crates/lang-rules/tests/java_psm_mapping.rs::an_unbounded_sequence_is_a_collection_and_a_bounded_one_an_array`,
`crates/lang-rules/tests/java_psm_mapping.rs::an_interface_is_generated_under_the_psm_mapping`,
`crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`

**Status:** done — Datentypen und die Typen eines `interface` (Request, Reply, Parameter,
Dienste) lassen sich nach Clause 9 abbilden (Item unten).

### §7.11.2 Typabbildung der Interfaces unter DDS-Java-PSM Clause 9

**Spec:** §7.11.2, S. 67 — wie oben.

**Repo:** `crates/lang-rules/src/emit/interface_java.rs` (`java_interface_items` nimmt
`--java-mapping psm`; die Halter `Request`, `Reply`, `In`, `Out`, `Result`
behalten öffentliche Felder und nehmen die Typen der Abbildung, der Codec
greift über Zugriffsfunktionen auf die Datentypen zu); Commit `80e6a9531577`

**Tests:** `crates/lang-rules/tests/java_psm_mapping.rs::an_interface_is_generated_under_the_psm_mapping`,
`crates/lang-rules/tests/java_psm_mapping.rs::an_interface_under_the_psm_mapping_compiles`,
`crates/lang-rules/tests/java_psm_mapping.rs::a_type_pair_under_the_psm_mapping_compiles`

**Status:** done — Basic und Enhanced; die erzeugten Quellen werden mit
`-Xlint:all -Werror` übersetzt. Die Dienste stehen wie die Datentypen in der
äußeren Klasse `ZdServices` (`interface_java.rs::OUTER`), unter beiden
Abbildungen gleich.

### §7.11.2 Unterschiede zur C++-Bindung

**Spec:** §7.11.2, S. 67 — "The following sub clauses describe the entities
that are either different or not described in the C++ language bindings."

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Einleitung.

### §7.11.2.1 Mapping of Exceptions

**Spec:** §7.11.2.1, S. 67 — Überschrift ohne Text.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — der Abschnitt hat keinen Inhalt. Die
Java-Laufzeit hat fünf Klassen `Remote*Error` über `RuntimeException`
(`runtime/java/src/main/java/org/omg/dds/rpc/`).

### §7.11.2.2.1 Pakete org.omg.dds und org.omg.dds.rpc

**Spec:** §7.11.2.2.1, S. 67 — "The org.omg.dds, and org.omg.dds.rpc packages
define the interfaces for the request-reply style language binding.
Specifically, the language binding includes Requester, Replier, ServiceProxy,
RPCEntity, RPCRuntime, SimpleReplierListener, ReplierListener,
SimpleRequesterListener, RequesterListener, RequesterParams, ReplierParams,
Future, FutureCompletionListener, Sample, and Sample.Iterator."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/`;
`crates/rpc/src/language_binding.rs` (`JAVA_PACKAGES`,
`JAVA_REQUEST_REPLY_TYPES`); Commit `97c20505e85c`

**Tests:** `crates/rpc/tests/class_summaries_are_declared.rs::the_java_package_declares_every_type_of_the_two_summaries`

**Status:** done — alle 15 Typen sind da; `Sample.Iterator` ist in `Sample.java`
verschachtelt. Die Form als Interface steht im Item unten.

### §7.11.2.2.1 Typen als Interfaces

**Spec:** §7.11.2.2.1 und §7.11.2.2.3, S. 67 — "define the interfaces";
"RPCRuntime is the only abstract class with placeholder implementation in the
Java language binding."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/` (`Requester`, `Replier`,
`RequesterParams`, `ReplierParams`, `Server` sind Interfaces; Implementierungen
`RequesterImpl`, `ReplierImpl`, `RequesterParamsImpl`, `ReplierParamsImpl`,
`ServerImpl`; `RPCRuntime` ist die abstrakte Klasse); Commit `97c20505e85c`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(das Programm übersetzt `RequesterImpl implements Requester` und die weiteren
Implementierungen)

**Status:** done

### §7.11.2.2.2 RPCEntity

**Spec:** §7.11.2.2.2, S. 67 — "RPCEntity is the base interface extended by all
the active entities. It inherits from the DDSObject interface defined in
[DDS-Java-PSM] and java.io.Closeable. Its purpose is to provide quick access to
the RPCRuntime object."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/RPCEntity.java`; `runtime/java/src/main/java/org/omg/dds/rpc/Server.java`
(`extends RPCEntity, Runnable`); Commit `dcdb4fd4a073`, Zugriff auf die Laufzeit
`97c20505e85c`, `getRPCRuntime` `13f4277e0a04`, `Server` `998e1fc5e576`

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::rpcEntityInheritsDdsObjectAndCloseable`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::anEntityAnRpcRuntimeMadeReturnsThatRuntime`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `Server extends
RPCEntity`)

**Status:** done — Vererbung, Zugriff auf die Laufzeit und `Server` als `RPCEntity`.

### §7.11.2.2.3 RPCRuntime: abstrakte Klasse über ServiceEnvironment

**Spec:** §7.11.2.2.3, S. 67 — "RPCRuntime is the only abstract class with
placeholder implementation in the Java language binding. It extends
ServiceEnvironment abstract class defined in [DDS-Java-PSM]."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/RPCRuntime.java`; Commit
`97c20505e85c`

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::aServiceEnvironmentReturnsItself`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** done

### §7.11.2.2.3 RPCRuntime als Singleton und Fabrik aller Entitäten

**Spec:** §7.11.2.2.3, S. 67 — "Its purpose is to provide access to the
RPCRuntime singleton, which serves as a factory for all other entities."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/RPCRuntime.java`
(`over`, `createRequester`, `createReplier`, `createServer`,
`createRequesterParams`, `createReplierParams`, `createServiceParams`,
`createClientParams`, `createService`, `createClient`, `release`); Commit
`97c20505e85c`, Service und Client `998e1fc5e576`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm `crates/rpc/tests/java/RpcRuntimeRoundtrip.java`: `over()` liefert die
eine Laufzeit eines Participants),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `createService` und
`createClient` liefern Entitäten der Laufzeit)

**Status:** done — `over(participant)` liefert je Participant dieselbe Instanz; Fabriken
für `Requester`, `Replier`, `Server`, `Service`, `Client` und die Params.

### §7.11.2.2.3 RPCRuntime-Fabrik für Service und Client

**Spec:** §7.11.2.2.3, S. 67 — wie oben.

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/RPCRuntime.java::createService`,
`RPCRuntime.java::createClient` (nehmen den Konstruktor der erzeugten
`<I>ServiceImpl` beziehungsweise `<I>ClientImpl`, auch mit Instanzname);
`ServiceParams`, `ClientParams`; Commit `998e1fc5e576`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `service.getEnvironment()
== runtime`, ein Aufruf des Clients erreicht den Service, Instanzname gebunden,
`createService` ohne Participant wird abgewiesen)

**Status:** done

### §7.11.2.2.4 Future<T>

**Spec:** §7.11.2.2.4, S. 67 — "The rpc.Future<T> interface extends the
java.util.concurrent.Future<T> interface and adds a single operation to specify
a FutureCompletionListener."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/Future.java`
(`setListener`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** done

### §7.11.2.2.5 FutureCompletionListener<T>

**Spec:** §7.11.2.2.5, S. 68 — "The FutureCompletionListener allows callback
notification when the corresponding future becomes ready."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/FutureCompletionListener.java`
(`onFutureComplete`); Commit `d047dc5f497c`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`

**Status:** done

### §7.11.2.2.6 Sample<T>

**Spec:** §7.11.2.2.6, S. 68 — "The rpc.Sample<T> interface extends
dds.sub.Sample<T> interface and defines two additional operations to retrieve
the SampleIdentity and the related sample identity, if any."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/Sample.java` (`identity`,
`relatedIdentity`); Commit `97c20505e85c`

**Tests:** `crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::rpcSampleExtendsThePsmSampleOverTheSameParameter`,
`crates/java-omgdds/java/src/test/java/org/omg/dds/rpc/RpcInheritanceTest.java::anRpcSampleIsAPsmSample`

**Status:** done — zusätzlich deklariert das Interface `validData()`.

### §7.11.2.2.7 Sample.Iterator wie in DDS-Java-PSM

**Spec:** §7.11.2.2.7, S. 68 — "The Sample.Iterator interface is the same as in
[DDS-Java-PSM]." DDS-Java-PSM §7.7.2 (S. 16 der PDF `dds-java-psm-1.0.pdf`):
"an iterator that extends java.util.ListIterator"; Seite 17: "the iterator
implements the Java.io.Closeable interface".

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/Sample.java`
(`Sample.Iterator extends java.io.Closeable, java.util.ListIterator<Sample<T>>`,
`Sample.iterate`; `remove`, `set` und `add` werfen
`UnsupportedOperationException`); Commit `97c20505e85c`, `Closeable`
`998e1fc5e576`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_rpc_runtime_and_the_generated_services_run_together`
(Programm `crates/rpc/tests/java/RpcRuntimeRoundtrip.java`: `instanceof
java.util.ListIterator`, Rückwärtslauf, `remove` verweigert),
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `Closeable`,
`close()` ohne geprüfte Ausnahme, `next()` nach `close()`,
try-with-resources)

**Status:** done — `ListIterator`, lesender Zugriff und `java.io.Closeable` stimmen.

### §7.11.2.2.7 Sample.Iterator: Closeable und returnLoan

**Spec:** §7.11.2.2.7, S. 68 — wie oben.

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/Sample.java::Sample.Iterator`
(`close`, `returnLoan`, `isClosed`); Commit `998e1fc5e576`

**Tests:** `crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`
(Programm `crates/rpc/tests/java/RpcInheritanceTest.java`: `Closeable` ist
Supertyp, `close()` beendet den Lauf, zweites `close()` ist erlaubt, das
Verlassen von try-with-resources gibt das Loan zurück)

**Status:** done

### §7.11.2.3 Function-call in Java mit den Entitäten von C++

**Spec:** §7.11.2.3, S. 68 — "Function-Call style language binding in Java
shall define the same entities as that of C++."

**Repo:** `runtime/java/src/main/java/org/omg/dds/rpc/` (`Service`,
`ServiceEndpoint`, `Server`, `Client`, `ClientEndpoint`, `FunctionCallService`,
`FunctionCallClient`);
`crates/lang-rules/src/emit/interface_java.rs` (`client_interface`,
`service_interface`);
`crates/lang-rules/src/emit/interface_java/calls.rs` (`client_class`,
`service_class`); Commit `c4a249bd251f`, Server, ClientEndpoint und Laufzeit `998e1fc5e576`

**Tests:** `crates/rpc/src/language_binding.rs::tests::the_java_function_call_types_are_the_cpp_ones`,
`crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`,
`crates/rpc/tests/java_rpc_runtime.rs::the_generated_function_call_client_calls_the_generated_service`,
`crates/rpc/tests/java_rpc_runtime.rs::the_java_entities_are_interfaces_and_the_runtime_makes_services_and_clients`

**Status:** done — die fünf Entitäten sind deklariert und implementiert; `Server` bindet
einen `Service` an genau einen Server (`IllegalStateException`), hat `run()` und
ist ein `RPCEntity`; `ClientEndpoint` gibt Request-Writer und Reply-Reader.

---

## Audit-Status

371 done / 0 partial / 0 open / 44 n/a (informative) / 0 n/a (rejected).

Statuswerte nach `internal/spec-coverage/PROCESS.md`. `n/a (rejected)` kommt
nicht vor.

Stand: Code des Branches `agent/rpc`. Die Tests liefen in den Runden 1 bis 10 auf
dem Linux-Testhost; Runde 10 war fehlerfrei. Diese Angabe stammt aus dem Bericht
der Runden, nicht aus einem Lauf dieses Audits. Auf dem Mac wurde nichts
ausgeführt.

Offene Punkte: keine.

