# ROS 2 RMW Bridge — Spec-Coverage

**Quellen:**

* ROS Enhancement Proposals (REPs) — `internal/standards/cache/ros2/rep-{2003,2004,2005,2007,2008,2009}.html`.
* RMW C-API Headers — `rmw/rmw.h` und `rmw/qos_profiles.h` aus
  `rmw 4.x` (ROS 2 Iron/Jazzy Distribution); leben im upstream
  `ros2/rmw`-GitHub-Repository, **nicht** in `internal/standards/cache/`,
  da eigenständige Distribution. De-facto-Spezifikation ohne eigene
  REP, normativ über die Header-Definitionen.
* ROS 2 IDL Subset — Design-Article `design.ros2.org/articles/
  idl_interface_definition.html` (nicht im Cache; live-Quelle) +
  Wire-Naming-Convention aus `rosidl_typesupport_fastrtps_cpp`
  (de-facto, in `ros2/rosidl`-GitHub-Repository).
* Topic/Service-Naming-Convention — Design-Article
  `design.ros2.org/articles/topic_and_service_names.html`
  (nicht im Cache; live-Quelle). Implementiert in `rmw_dds_common`.

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`.

**Kontext:** ROS 2 Robot-Middleware-Wrapper bauen auf DDS auf. Die
RMW-API + RMW-QoS-Profile-Mapping + Topic-Mangling-Convention +
ROS-IDL→DDS-XTypes-Wire-Convention sind die zentralen Wire-Mappings.
Verteilt über:

- `crates/ros2-rmw/` — Mapping-Layer als pure-Rust no_std+alloc Library (RMW-API, QoS-Profile-Mapping, Topic-Mangling, ROS-IDL→DDS-XTypes)
- `crates/rmw-zerodds-shim/` — `rmw_zerodds`-C-FFI-Wrapper

**Crate-Mapping:**

| Spec-Bereich | Crate-File |
|---|---|
| REP-2003 Sensor/Map QoS | `crates/ros2-rmw/src/qos_profiles.rs` (Profile-Konstanten) |
| REP-2004 Quality-Levels | `crates/ros2-rmw/src/quality.rs` |
| Topic-Name-Mangling Convention | `crates/ros2-rmw/src/topic_mangling.rs` |
| Standard RMW QoS Profiles | `crates/ros2-rmw/src/qos_profiles.rs::profiles::*` |
| RMW C-API (`rmw/rmw.h`) | `crates/ros2-rmw/src/ffi_api.rs` |
| RMW-QoS-Mapping (`rmw/qos_profiles.h`) | `crates/ros2-rmw/src/rmw_qos_mapping.rs` |
| ROS-IDL → DDS-XTypes Wire-Mapping | `crates/ros2-rmw/src/type_mapping.rs` |
| REP-2007 Type-Adaptation / REP-2008 Beschleuniger-Speicher | `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h` |

Implementation: `crates/ros2-rmw/` (11 Module, 81 Tests grün via
`cargo test -p zerodds-ros2-rmw`).

---

## REP-2003 Sensor Data and Map QoS Settings

### Map QoS

**Spec:** REP-2003 §"Map Quality of Service" — "Map providers [...] are
expected to provide all maps over a reliable transient-local topic.
[...] The depth of the transient-local storage depth is left to the
designer, however a single map depth is a reasonable choice".

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::MAP` —
Reliable + TransientLocal + KeepLast(1).

**Tests:** `qos_profiles::tests::map_profile_matches_rep_2003_specification`.

**Status:** done

### Sensor Data QoS (Consumer-Side)

**Spec:** REP-2003 §"Sensor Driver Quality of Service" — wörtlich:
"Sensor data provided by a sensor driver from a camera, inertial
measurement unit, laser scanner, GPS, depth, range finder, or other
sensors are expected to be provided over a `SystemDefaultsQoS`
quality of service as provided by the implemented ROS 2 version API.
Consumers of sensor data are to use `SensorDataQoS` quality of service
as provided by the implemented ROS 2 version API."

Wichtig: REP-2003 verlangt für **Driver-Side** `SystemDefaultsQoS`
(= Reliable+Volatile+KeepLast(10), siehe `DEFAULT`-Item unter
"Standard RMW QoS Profiles" weiter unten). Die Consumer-Side
verwendet `SensorDataQoS` (= BestEffort+Volatile+KeepLast(5)).
Dieses Item beschreibt die Consumer-Side; die Driver-Side ist via
`profiles::DEFAULT` abgedeckt.

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::SENSOR_DATA` —
BestEffort + Volatile + KeepLast(5) (entspricht
`rmw_qos_profile_sensor_data` aus `rmw/qos_profiles.h`).

**Tests:** `qos_profiles::tests::sensor_data_profile_matches_rep_2003_specification`
prüft die Consumer-Side-Konstante (BestEffort+Volatile+KeepLast(5)).

**Status:** done — Consumer-Side abgedeckt; Driver-Side ist über
`DEFAULT`-Profile (siehe unten) gleichermaßen abgedeckt.

---

## REP-2004 Package Quality Categories

### Quality Levels Q1-Q5

**Spec:** REP-2004 — fünf Levels mit folgenden Definitionen
(zitiert aus REP-2004 §"Quality Level Categories"):

* **Quality Level 1** — "highest quality level; packages that are
  needed for production systems".
* **Quality Level 2** — "high quality packages that are either:
  needed for production systems or commonly used".
* **Quality Level 3** — "tooling quality packages".
* **Quality Level 4** — "demos, tutorials, and experiments".
* **Quality Level 5** — "default quality level" (Pakete ohne
  explizite Quality-Claims).

Numerische Repräsentation 1..5; Q1 ist die höchste, Q5 ist der
Default für Pakete ohne explizit deklariertes Quality-Niveau.

**Repo:** `crates/ros2-rmw/src/quality.rs::QualityLevel` mit
`numeric()`/`from_numeric()`-Konvertern.

**Tests:** `quality::tests::quality_level_numeric_round_trip`,
`quality::tests::quality_level_ordering_q1_is_highest`,
`quality::tests::quality_level_from_numeric_rejects_out_of_range`.

**Status:** done — Klassifikations-Modell exposed; tatsächliche
Quality-Audit ist Caller-Aufgabe (z.B. `package.xml`-Tag).

---

## REP-2005 ROS 2 Common Packages

### Common-Package-List

**Spec:** REP-2005 — informational; Liste der ROS-2-Common-Packages.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — REP-2005 markiert sich selbst als
informational; Common-Package-Liste ohne normative Anforderung an die
RMW-Bridge.

---

## REP-2007 Type Adaptation Feature

REP-2007 legt die Anwender-API (`rclcpp::TypeAdapter`) bewusst in `rclcpp` und
benennt im selben Abschnitt, was der Middleware bleibt: „Perhaps we can support
a form of this feature in other languages in rcl or rmw in the future. One
challenge in doing this is that it may require **custom type support, which may
be middleware specific**." Die vier Items unten sind dieser
Custom-Type-Support. Die Naht zu `rclcpp` ist die C++-Template-Syntax
(`TypeAdapter<Custom, Ros>`, `adapt_type::as`, `RCLCPP_USING_...`) und die
Ownership-Mechanik über `unique_ptr`/`shared_ptr`; alles darunter — Draht-Form,
Typ-Identität, Kopierzahl — liegt hier.

### Defining a Type Adapter — rmw-seitiges Custom-Type-Support

**Spec:** REP-2007 §"Defining a Type Adapter": ein Adapter benennt
`custom_type`, `ros_message_type` und die Konversionsfunktionen. §"Adding this
feature in rclcpp": „it may require custom type support, which may be middleware
specific".

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h`
(`rmw_zerodds_type_adapter_t` — `ros_type_name`, `serialized_size`,
`serialize`, `deserialize`) + `rmw_c/rmw_zerodds.c`
(`rmw_zerodds_publisher_set_type_adapter`,
`rmw_zerodds_subscription_set_type_adapter`; Adapter-Feld in
`zerodds_pub_data_t`/`zerodds_sub_data_t`). Der Endpoint selbst wird
unverändert aus dem rosidl-Type-Support des `ros_message_type` gebaut, also
bleiben DDS-Topic-Name, ROS-Typname und RIHS-Type-Hash die des unadaptierten
Endpoints.

**Tests:** `rmw_c/type_adaptation_test.cpp` (rclcpp-e2e, Bindung an die
rmw-Handles lebender rclcpp-Entities; Prüfung „endpoint topic type unchanged by
adaptation" über `get_publishers_info_by_topic` vor und nach der Bindung) via
`rmw_c/run_type_adaptation_test.sh`;
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::every_declared_extension_symbol_is_defined`.

**Status:** done

### Type Adaptation in Topics

**Spec:** REP-2007 §"Type Adaptation in Topics" — ein adaptierter Publisher
bzw. eine adaptierte Subscription tauscht den `custom_type` aus, während auf dem
Draht der `ros_message_type` steht.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_zerodds_publish_adapted`,
`rmw_zerodds_take_adapted`, `zerodds_adapted_wire` — 4-Byte-CDR_LE-Encapsulation
identisch zum Introspection-Publish-Pfad). Als Teil davon füllen
`rmw_take_with_info`, `rmw_take_loaned_message_with_info` und
`rmw_take_serialized_message_with_info` jetzt `rmw_message_info_t`
(`zerodds_fill_message_info`: `received_timestamp`,
`reception_sequence_number`) statt einer Nullstruktur — rclcpp reicht diese
Struktur an den adaptierten Callback weiter.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_adaptation_test.cpp`:
`"2) adapted publish -> plain subscription"` (adaptierter Publisher,
gewöhnliche rclcpp-Subscription des ROS-Message-Typs),
`"3) plain publish -> adapted take"` (Gegenrichtung),
`"   adapted take fills rmw_message_info_t"`; gefahren über
`crates/rmw-zerodds-shim/rmw_c/run_type_adaptation_test.sh`.

**Status:** done

### Type Adaptation in Services und Actions

**Spec:** REP-2007 §"Type Adaptation in Services": Request- und Response-Hälfte
werden unabhängig adaptiert („either the request or the response can be
adapted"). §"Type Adaptation in Actions": dieselbe Struktur für Goal, Feedback
und Result.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_zerodds_client_set_type_adapters`,
`rmw_zerodds_service_set_type_adapters`, `rmw_zerodds_send_request_adapted`,
`rmw_zerodds_take_request_adapted`, `rmw_zerodds_send_response_adapted`,
`rmw_zerodds_take_response_adapted`; die adaptierte Nutzlast trägt denselben
24-Byte-Korrelations-Header wie der unadaptierte Pfad). Eine ROS-2-Action ist in
`rcl` aus drei Services (Goal, Cancel, Result) und zwei Topics (Feedback,
Status) zusammengesetzt — es gibt keine eigene Action-rmw-API —, also deckt die
Service- plus Topic-Fläche die Action-Adaption ab.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_adaptation_test.cpp` —
`"5) adapted service request/response roundtrip"` über
`example_interfaces/srv/AddTwoInts` mit `std::pair<int64_t,int64_t>` als
Request- und `int64_t` als Response-Custom-Type (beides keine
ROS-Nachrichten); gefahren über `crates/rmw-zerodds-shim/rmw_c/run_type_adaptation_test.sh`.

**Status:** done

### (De)Serialize statt Convert — die vermiedene Kopie

**Spec:** REP-2007 §"Comparison to ROS 1's Type Adaptation": „This feature will
support both convert and (de)serialize functions […] convert is superior for
intra-process communication and the (de)serialization functions are better for
inter-process communication." Und: „An advantage of following the ROS 1 approach
is that an extra copy can be avoided; although it is likely much more
challenging to implement this feature the ROS 1 way because of the middleware."

**Repo:** `rmw_c/rmw_zerodds.c` (`zerodds_adapted_wire`): der Draht-Puffer wird
auf `serialized_size(custom)` alloziert und der Adapter schreibt den
CDR-Rumpf direkt hinein. Es entsteht keine `ros_message_type`-Instanz, also
findet die vom REP als vermeidbar beschriebene zweite Kopie nicht statt.
Gegenrichtung analog in `rmw_zerodds_take_adapted`: der Rumpf wird direkt in den
`custom_type` gelesen.

**Tests:** `rmw_c/type_adaptation_test.cpp` misst drei Größen:
„adapted wire bytes == rmw_serialize bytes" (`memcmp` gegen die
`rmw_serialize`-Ausgabe derselben Nachricht — die Draht-Identität ist gemessen,
nicht per Konstruktion behauptet), „exactly one serialization per adapted
publish" (Zähler in der Adapter-`serialize`) und „ros_message_type never
materialized (no extra copy)" (der `convert_to_ros_message`-Zähler des Tests
muss 0 bleiben). Via `rmw_c/run_type_adaptation_test.sh`.
Draht-Identitäts-Regression:
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::adapted_publish_writes_the_same_encapsulation_header_as_plain_publish`.

**Status:** done

---

## REP-2008 Hardware Acceleration

REP-2008 ordnet die Schichten selbst zu: §"Kernel levels in ROS 2" stellt
`rmw` und die `rmw_adapters` auf Level 2 und die Kommunikations-Middleware
(„including the communication middleware (e.g. DDS)") auf Level 3, und §
"Methodology" Schritt 3.2 verlangt, „inter-Node exchanges" zu beschleunigen und
„the overhead of the ROS 2 message-passing system across all its abstraction
layers" zu senken. Die Naht liegt bei den Kernel-Sprachen (CUDA, HLS, OpenCL)
und der Firmware — beides erklärt der REP ausdrücklich zur Sache des
Silizium-Herstellers und des Anwendungs-Architekten. Was hier liegt, ist die
Speicherherkunft der Puffer, die zwischen Beschleuniger und Draht laufen.

### Kernel-Level-Zuordnung

**Spec:** REP-2008 §"Kernel levels in ROS 2" — Level 2: „we consider rclcpp,
rcl, rmw, and the corresponding rmw_adapters for each supported communication
middleware". Level 3: „Groups together all accelerators below the ROS 2 core
layers […] including the communication middleware (e.g. DDS)". Und: „Hardware
acceleration solutions complying with this REP should aspire to support multiple
kernel levels in ROS 2".

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h`
(`RMW_ZERODDS_ACCEL_LEVEL_1/2/3`) + `rmw_c/rmw_zerodds.c`
(`rmw_zerodds_acceleration_kernel_levels` → Level 2 | Level 3). Level 1 (ROS-2-
Anwendungen und -Bibliotheken) bleibt beim Anwender.

**Tests:** `rmw_c/accel_loan_memory_test.cpp` („REP-2008 kernel levels 2 and 3
declared") via `rmw_c/run_accel_loan_memory_test.sh`;
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::acceleration_kernel_levels_agree_across_header_impl_and_readme`
(Header, Implementierung und Vendor-Vorlage im README müssen dieselbe Zahl
nennen).

**Status:** done

### Pillar I — Erweiterungen des ament-Build-Systems

**Spec:** REP-2008 §"Pillar I - Extensions to ament" + §Specification:
„Package maintainers are encouraged to integrate hardware acceleration in their
ROS 2 packages by using the CMake hardware acceleration variables […] These
CMake macros are meant to be used by maintainers to conditionally compile their
ROS 2 packages". Variablen: `ROS_ACCELERATION` (`ament_hardware_acceleration`),
`ROS_VITIS` (`ament_vitis`).

**Repo:** `crates/rmw-zerodds-shim/ament/CMakeLists.txt`
(`find_package(ament_hardware_acceleration QUIET)`,
`find_package(ament_vitis QUIET)`, Auswertung von `ROS_ACCELERATION`/`ROS_VITIS`,
`RMW_ZERODDS_ACCELERATION_KERNEL_LEVELS`, `RMW_ZERODDS_LOAN_ALIGNMENT=page` im
Beschleuniger-Fall; Installation von `rmw_zerodds_ros2_ext.h`) +
`crates/rmw-zerodds-shim/ament/package.xml` (bedingtes
`ament_hardware_acceleration`-Build-Depend). Beide `find_package` sind `QUIET`:
ein Workspace ohne Beschleuniger konfiguriert unverändert.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/accel_loan_memory_test.cpp` prüft die Laufzeit-Seite derselben
Entscheidung: `"2) set_loan_alignment(page) accepted"`,
`"2) portable loan starts on a page boundary"` (mit
`ZERODDS_LOAN_ALIGNMENT=page`) und
`"   non-power-of-two alignment rejected"`; gefahren über
`crates/rmw-zerodds-shim/rmw_c/run_accel_loan_memory_test.sh`.

**Status:** done

### Pillar II und Template for Vendors — deklarierter Support-Umfang

**Spec:** REP-2008 §"Pillar II - firmware": „each vendor should provide and
maintain an `acceleration_firmware_<solution>` package specialization that
delivers the corresponding artifacts" — Firmware-Artefakte je Silizium-Lösung.
§"Template for Vendors": „Silicon vendors and solution manufacturers can help
set the expectations of the level of support their hardware acceleration
technology provides in alignment with this REP by providing a template in the
README.md files of their firmware and/or ament extensions."

**Repo:** `crates/rmw-zerodds-shim/README.md` §"Template for Vendors
(REP-2008 §"Template for Vendors")" — die vom REP verlangte Tabelle: bediente
Kernel-Level, Pillar I konsumiert, Pillar II nicht geliefert (Firmware ist das
`acceleration_firmware_<solution>`-Paket des Silizium-Herstellers, ZeroDDS
liefert keine Firmware-Artefakte), Technologie-Agnostik, die
Beschleuniger-zugewandte API und die Umgebungsschalter.

**Tests:**
`crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::acceleration_kernel_levels_agree_across_header_impl_and_readme`
(die Vorlage darf nicht von Header und Implementierung abweichen).

**Status:** done

### Methodology §3.2 — beschleuniger-registrierbarer Loan-Speicher

**Spec:** REP-2008 §"Methodology for developing ROS 2 packages that integrate
hardware acceleration", Schritt 3.2: „accelerate inter-Node exchanges and reduce
the overhead of the ROS 2 message-passing system across all its abstraction
layers. To do so, consider leveraging REP-2007 and REP-2009."

CUDA, ROCm und XRT können nur aus Host-Speicher DMA-en, den sie vorher
registriert (gepinnt) haben, und sie pinnen ganze Seiten. Ohne eine Aussage der
Middleware, wo ein geliehener Nachrichtenpuffer liegt und welche
seitenausgerichtete Ausdehnung ihn abdeckt, braucht ein beschleunigter Node
einen eigenen gepinnten Staging-Puffer und kopiert bei jedem Publish — genau
die Kopie, die Schritt 3.2 entfernen will.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_ros2_ext.h`
(`rmw_zerodds_loan_memory_t`: `kind`, `host_ptr`, `size`, `registration_base`,
`registration_size`, `page_size`, `cross_process`, `segment_link`) +
`rmw_c/rmw_zerodds.c` (`rmw_zerodds_publisher_get_loan_memory`,
`rmw_zerodds_subscription_get_loan_memory`, `zerodds_fill_registration`,
`rmw_zerodds_publisher_set_loan_alignment`, `zerodds_default_loan_alignment`
über `ZERODDS_LOAN_ALIGNMENT`, ausgerichtete Portable-Leihgabe in
`rmw_borrow_loaned_message`). `registration_base`/`registration_size` sind die
Argumente für `cudaHostRegister`, `hipHostRegister` oder `xrt::bo`.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/accel_loan_memory_test.cpp` über
`crates/rmw-zerodds-shim/rmw_c/run_accel_loan_memory_test.sh`, in beiden Delivery-Modi:
`"3) reported page size == sysconf(_SC_PAGESIZE)"`,
`"3) registration extent is page aligned on both ends"`,
`"3) registration extent covers the loaned buffer"`,
`"4) reported extent is pinnable (mlock)"` — der Aufruf führt dieselbe
Kernel-Operation aus, mit der eine Beschleuniger-Runtime Host-Speicher pinnt;
die Pinnbarkeit ist damit gemessen und nicht gerechnet —,
`"   the described buffer is the buffer that was delivered"` und im Modus
`raw-same-host` `"5) forked child maps the segment and finds the payload"`
sowie `"5) subscriber loan reports the same shared segment"`.

**Status:** done

---

## REP-2009 Type Negotiation Feature

REP-2009 verortet die Negotiation-State-Machine selbst und sagt zugleich, was
sie von den Schichten darunter erwartet — §"Implementation in the ROS 2 Core or
in a Separate Project": „While there are some advantages to having this project
in the ROS 2 core, **all of the needed APIs are available** to make this a
separate package. Thus, in the spirit of keeping the ROS 2 core smaller, this
should be implemented in a separate project."

Die rmw-Pflicht ist damit nicht, den Algorithmus zu fahren, sondern diesen Satz
wahr zu machen. Die fünf Items unten sind die APIs, die §"Negotiation
Algorithm", §"Defining Supported Message Types" und §"Renegotiating Selected
Message Types" namentlich brauchen; jedes ist gegen einen Lauf gemessen, der
den REP-Algorithmus von Hand über `rmw_zerodds` fährt. Die Naht liegt bei der
State-Machine selbst (Prioritätsrechnung, benutzerdefinierte Auswahlfunktionen,
Deadlock-Timeout) — die legt der REP ausdrücklich in ein eigenes Paket über rmw.

### Endpoint info by topic — `rmw_get_publishers/subscriptions_info_by_topic`

**Spec:** REP-2009 §"Negotiation Algorithm", Negotiating Publisher, Schritt 1:
„Use discovery to find all the connected subscriptions that are using the user
specified negotiation topic." Die rmw-Fläche dafür ist
`rmw/get_topic_endpoint_info.h`: pro Topic die Endpoint-Liste mit
Node-Name/-Namespace, Typ, Endpoint-Typ, 16-Byte-GUID und QoS (`ros2 topic
info -v`), dazu `rmw_count_publishers`/`rmw_count_subscribers`.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_get_publishers_info_by_topic`/
`_subscriptions_info_by_topic` → `zerodds_get_endpoint_info_by_topic`): enumeriert
pro-Endpoint via `rmw_zerodds_node_for_each_publication/subscription_endpoint`,
filtert auf das demanglede Topic, löst pro Endpoint die Node-Identität über
`rmw_zerodds_node_resolve_endpoint` (Endpoint-GUID → Node) auf und füllt
`rmw_topic_endpoint_info_array_t`. Datenpfad: `crates/dcps/src/runtime.rs`
(`discovered_publication/subscription_endpoints`, lokal + SEDP-remote) →
c-api (`zerodds_runtime_for_each_*_endpoint` + `ZeroDdsEndpointInfo`) → Shim.
Voraussetzung Endpoint→Node: die `ros_discovery_info`-Participant-gid ist die
**echte** DDS-Participant-GUID, und `ParticipantEntitiesInfo` trägt pro Node die
reader/writer-gid-Sequenzen (Endpoint-GUID-Prefix matcht den Participant).
QoS ist best-effort aus der Discovery (History/Depth liegen nicht auf dem Draht →
`UNKNOWN`).

**Tests:** `crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_endpoint_info_by_topic`
(rclpy `get_publishers_info_by_topic`: Typ + QoS + aus der Endpoint-GUID
aufgelöster Node-Name/-Namespace) auf ROS 2 Humble; Shim-Roundtrip-Units
`crates/rmw-zerodds-shim/src/lib.rs::tests::participant_info_roundtrips_endpoint_gids`,
`crates/rmw-zerodds-shim/src/lib.rs::tests::participant_info_roundtrips_without_endpoints`;
`rmw_c/type_negotiation_test.cpp` („negotiation subscription found through
discovery", „endpoint info names the negotiation message type") via
`rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### Supported-Message-Type-Identität — RIHS-Type-Hash

**Spec:** REP-2009 §"Defining Supported Message Types": „The combination of the
ROS message type and the string identifier must form a unique tuple, since
multiple supported message types may be sent with the same ROS 2 message type."
Die rmw-Hälfte dieses Tupels ist der Typ auf dem Draht: Typname und
RIHS-Type-Hash pro Endpoint (der String-Identifier lebt in der Negotiation-
Nutzlast, siehe nächstes Item).

**Repo:** `crates/rmw-zerodds-shim/src/lib.rs::rmw_zerodds_compute_type_hash`
(RIHS SHA-256) + der Endpoint-Info-Pfad oben.

**Tests:** `type_hash_sha256_is_deterministic` + 2 weitere Shim-Units;
`test_rclpy_endpoint_info_by_topic` (Humble);
`rmw_c/type_negotiation_test.cpp` („discovery reports the right ROS type per
selected type" — zwei gleichzeitig ausgewählte Typen auf zwei Topics, jeder mit
seinem eigenen ROS-Typ in der Discovery).

**Status:** done

### Übertragung der Supported-Message-Priority-Map

**Spec:** REP-2009 §"Negotiation Algorithm", Schritt 2: „Receive the supported
message priority maps from all connected subscriptions." §"Defining Negotiating
Publishers and Subscriptions": die Map wandert über „a negotiation topic that
will be used to negotiate the selected message types" — also über gewöhnliches
ROS-Pub/Sub. Ihre Form ist eine Sequenz verschachtelter Strukturen mit Strings
(ROS-Typname, String-Identifier, Topic, Priorität).

**Repo:** der Introspection-CDR-Codec in `rmw_c/rmw_zerodds.c`
(`cdr_ser_msg`/`cdr_de_msg`, Sequenz-Zweig über `size_function`/
`get_const_function`/`get_function`/`resize_function`, verschachtelte Nachricht
über `type_id_ == 18`) in beiden Typesupport-Varianten — siehe
„Introspection-Typesupport-Varianten (C und C++)".

**Tests:** `rmw_c/type_negotiation_test.cpp` mit
`diagnostic_msgs/msg/DiagnosticArray` — dieselbe Form wie
`negotiated_interfaces/msg/SupportedTypes` (`DiagnosticStatus[]` mit drei
Strings und einer verschachtelten `KeyValue[]`): „priority map survives
serialize/deserialize" (`rmw_serialize`/`rmw_deserialize`-Identität),
„supported message priority map delivered" (über den Draht) und „priority map
round-trips field for field" (Feld für Feld verglichen). Via
`rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### Ein regulärer Publisher je Selected Message Type

**Spec:** REP-2009 §"Negotiation Algorithm", Negotiating Publisher: „The
negotiating publisher then creates a regular publisher for each supported
message type." §"Negotiating Subscription": „The negotiating subscription will
then create exactly one subscription corresponding to the chosen type and topic
name." Die Endpoints entstehen also zur Laufzeit, nach der Entscheidung, und
tragen je einen anderen ROS-Typ.

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_create_publisher`/`rmw_create_subscription`
zur Laufzeit auf einem laufenden Node; der Typname kommt pro Endpoint aus
`zerodds_type_name` und geht in Discovery und Draht).

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_negotiation_test.cpp`:
`"4) both selected message types deliver on their topics"`
(`std_msgs/msg/String` und `std_msgs/msg/Int32` gleichzeitig — der Fall, den
REP-2009 als `N_1(x, y)` an `N_2(x)` und `N_3(y)` zeichnet),
`"4) discovery reports the right ROS type per selected type"`,
`"5) a regular publisher per selected message type"`,
`"5) the selected type appears in the graph with its ROS type"`; gefahren
über `crates/rmw-zerodds-shim/rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### Renegotiation — Endpoints verschwinden und entstehen neu

**Spec:** REP-2009 §"Renegotiating Selected Message Types": „If the selected
messages must change, the negotiating publisher destroys its publishers. The
negotiating publisher will then notify its subscriptions of the new selected
message types on the user specified negotiation topic and create new publishers
for the selected message types." Ein zerstörter Endpoint muss also sofort aus
dem Graph verschwinden, sonst rechnet die nächste Runde mit einem Peer, den es
nicht mehr gibt.

**Repo:** `crates/zerodds-c-api/src/lib.rs` (`zerodds_writer_destroy`/
`zerodds_reader_destroy` rufen jetzt
`DcpsRuntime::remove_user_writer`/`remove_user_reader`) +
`crates/dcps/src/runtime.rs` (die beiden Methoden waren `pub(crate)` und damit
nur über den DCPS-DataWriter erreichbar, nicht über den Runtime-FFI-Pfad, den
rmw benutzt; sie nehmen den Endpoint aus der lokalen Registry, bauen die
Intra-Runtime-Routen neu, stoppen die Ankündigung und senden ein
SEDP-Dispose).

Vorher blieb ein über rmw zerstörter Publisher für immer im Graph:
`rmw_count_publishers` und `rmw_get_publishers_info_by_topic` meldeten ihn
weiter, und ein entfernter Peer wartete auf den Liveliness-Timeout statt sofort
zu entmatchen.

**Tests:** `rmw_c/type_negotiation_test.cpp`: „destroyed publishers leave the
graph" (`rmw_destroy_publisher` auf dem rmw-Handle, danach
`count_publishers == 0` und leere Endpoint-Info), „discovery reports the
renegotiated ROS type" (dasselbe Topic trägt danach einen anderen ROS-Typ) und
„the renegotiated selected type delivers". Via
`rmw_c/run_type_negotiation_test.sh`.

**Status:** done

### Netzwerk-Änderung als Auslöser — Graph-Guard-Condition

**Spec:** REP-2009 §"Renegotiating Selected Message Types": „In the case that
the ROS 2 network is changing, the negotiating publisher may need to renegotiate
its selected message types." Zusammen mit Schritt 1 („Use discovery to find all
the connected subscriptions") heißt das: der Graph muss die Änderung **melden**,
nicht nur abfragbar sein. rcl reicht das als Graph-Guard-Condition des Nodes
durch.

**Repo:** `rmw_c/rmw_zerodds.c` (`zerodds_refresh_graph_guards`,
`zerodds_fp_cb`, `zerodds_node_register`/`_unregister`, `graph_fp` im
`zerodds_node_data_t`): `rmw_wait` bildet für jede Graph-Guard-Condition, die
tatsächlich in einem Wait-Set liegt, einen Fingerabdruck über alle entdeckten
Publications und Subscriptions des Nodes und hebt das Flag, sobald er sich
ändert. Ein Node, den niemand beobachtet, kostet nichts.

Dazu die Guard-Condition-Auswertung selbst: `rmw_wait` bekommt von rcl in
`rmw_guard_conditions_t.guard_conditions[i]` den **`data`-Zeiger** der
Guard-Condition, nicht die `rmw_guard_condition_t`
(`rcl_wait_set_add_guard_condition`: `RMWStorage[n] = rmw_handle->data`). Der
Shim las den Eintrag als Struktur und verglich deren
`implementation_identifier` — der Vergleich traf nie zu, also meldete **keine**
Guard-Condition je Bereitschaft: weder die Graph-Guard-Condition noch die
Interrupt-Guard-Condition des Executors noch eine benutzereigene. Der Eintrag
wird jetzt als das gelesen, was er ist.

Ebenfalls hier: `rmw_wait` gab bei unendlichem Timeout `RMW_RET_TIMEOUT`
zurück, wenn ein Weckruf ohne bereite Entität kam. rcls Graph-Listener behandelt
das als fatalen Fehler und beendet den Prozess mit „rcl_wait unexpectedly timed
out"; ein Wait ohne Deadline kann nicht ablaufen und meldet jetzt `RMW_RET_OK`.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_negotiation_test.cpp` —
`"6) the graph change raised the node graph guard"`: das Graph-Event wird vor
der Renegotiation scharf gestellt, der Wartepfad blockiert auf der
Guard-Condition, und das Event muss danach gesetzt sein. Gefahren über
`crates/rmw-zerodds-shim/rmw_c/run_type_negotiation_test.sh`.

**Status:** done

---

## Topic-Name-Mangling Convention (de-facto, `rmw_dds_common`)

### Prefix-Convention rt/rq/rr/rs

**Spec:** Design-Article `design.ros2.org/articles/
topic_and_service_names.html` §"DDS Topic Names" + de-facto-Konvention
aus `ros2/rmw_dds_common`-Implementation:

* `rt/<name>` — Topic-Pub-Sub (ROS-Topic).
* `rq/<name>Request` — Service-Request.
* `rr/<name>Reply` — Service-Reply.
* `rs/<name>` — Service-Discovery (legacy, vor REP-2009-Strukturen).

**Repo:** `crates/ros2-rmw/src/topic_mangling.rs::{RosKind,
mangle_topic_name, demangle_topic_name, is_ros_topic}`.

**Tests:** `topic_mangling::tests::mangle_topic_strips_leading_slash_and_prepends_rt`,
`topic_mangling::tests::mangle_preserves_internal_slashes`,
`topic_mangling::tests::mangle_each_kind_uses_correct_prefix`,
`topic_mangling::tests::mangle_handles_already_unprefixed_name`,
`topic_mangling::tests::mangle_rejects_empty_name`,
`topic_mangling::tests::mangle_rejects_invalid_leading_character`,
`topic_mangling::tests::mangle_accepts_underscore_leading`,
`topic_mangling::tests::demangle_round_trips_all_kinds`,
`topic_mangling::tests::demangle_rejects_unknown_prefix`,
`topic_mangling::tests::is_ros_topic_recognizes_all_four_prefixes`,
`topic_mangling::tests::mangle_demangle_round_trip`.

**Status:** done — Suffixe (`Request`/`Reply`) bleiben Caller-Aufgabe
(typisch automatisch vom Service-Codegen ergänzt).

---

## Standard RMW QoS Profiles

### `rmw_qos_profile_*` Konstanten

**Spec:** `rmw/qos_profiles.h` (rmw 4.x) — sieben Default-Profiles:
`rmw_qos_profile_default` (Reliable+Volatile+KeepLast(10)),
`rmw_qos_profile_sensor_data` (BestEffort+Volatile+KeepLast(5)),
`rmw_qos_profile_parameters` (Reliable+Volatile+KeepLast(1000)),
`rmw_qos_profile_services_default` (Reliable+Volatile+KeepLast(10)),
`rmw_qos_profile_parameter_events` (Reliable+Volatile+KeepLast(1000)),
`rmw_qos_profile_system_default` (alle Felder als
`*_SYSTEM_DEFAULT`-Sentinel; Implementation-defined),
`rmw_qos_profile_unknown` (alle Felder als `*_UNKNOWN`-Sentinel).

**Repo:** `crates/ros2-rmw/src/qos_profiles.rs::profiles::*`
(`DEFAULT`, `SENSOR_DATA`, `PARAMETERS`, `SERVICES_DEFAULT`,
`PARAMETER_EVENTS`, `SYSTEM_DEFAULT`, `UNKNOWN`, `MAP`) +
`is_unknown`/`is_system_default`-Predicates. Die Policy-Enums
(`Reliability`/`Durability`/`History`) tragen echte `SystemDefault`- und
`Unknown`-Sentinel-Varianten (spec-treu zu `rmw_qos_*_policy_t`);
`rmw_qos_mapping::rmw_to_dds` reicht sie durch (Auflösung erst bei
DDS-Entity-Erzeugung).

**Tests:** `qos_profiles::tests::default_profile_is_reliable_volatile_keep_last_10`,
`parameters_profile_uses_keep_last_1000`, `services_default_matches_rmw_defaults`,
`parameter_events_uses_keep_last_1000`, `system_default_is_all_sentinels`,
`unknown_profile_is_all_unknown`, `system_default_and_unknown_are_distinct`,
`is_unknown_recognizes_unknown_profile`, `is_unknown_rejects_real_and_system_default_profiles`;
`rmw_qos_mapping::tests::rmw_system_default_passes_through_as_sentinel`,
`rmw_unknown_passes_through_as_sentinel`.

**Status:** done — alle sieben rmw-Default-Profiles spec-treu:
* Sechs konkret 1:1 (DEFAULT, SENSOR_DATA, PARAMETERS, SERVICES_DEFAULT,
  PARAMETER_EVENTS, MAP).
* `SYSTEM_DEFAULT` = **jedes Feld** als `*_SYSTEM_DEFAULT`-Sentinel (nicht mehr
  Alias zu DEFAULT) → der DDS-Implementations-Default jedes Feldes greift.
* `UNKNOWN` = **jedes Feld** als `*_UNKNOWN`-Sentinel + `is_unknown`-Predicate.

---

## RMW C-API (`rmw/rmw.h`)

### `rmw_ret_t` Return-Codes

**Spec:** `rmw/types.h` (rmw 4.x) — `rmw_ret_t` als `int32` mit den
Werten OK=0, ERROR=1, TIMEOUT=2, UNSUPPORTED=3, BAD_ALLOC=10,
INVALID_ARGUMENT=11, INCORRECT_RMW_IMPLEMENTATION=12.

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::RmwRet` (`#[repr(i32)]`-
Enum mit allen sieben Werten + `map_to_rmw_ret`-Konverter).

**Tests:** `ffi_api::tests::error_codes_match_rmw_h`,
`ffi_api::tests::ok_is_zero`,
`ffi_api::tests::map_to_rmw_ret_ok`,
`ffi_api::tests::map_to_rmw_ret_err`.

**Status:** done

### Implementation-Identifier (`rmw_get_implementation_identifier`)

**Spec:** `rmw/rmw.h` — `rmw_get_implementation_identifier()` liefert
Vendor-String (Konvention `rmw_<vendor>_cpp`, z.B.
`"rmw_fastrtps_cpp"`/`"rmw_cyclonedds_cpp"`/`"rmw_connext_cpp"`).
`RMW_CHECK_*_FOR_NULL_*`-Macros (in `rmw/check_type_identifiers_match.h`)
prüfen Caller-übergebene Strings gegen diesen Identifier
(`RMW_RET_INCORRECT_RMW_IMPLEMENTATION` bei Mismatch).

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::check_rmw_identifier` plus
Konstante `RMW_IMPLEMENTATION_IDENTIFIER: &str = "rmw_zerodds_cpp"`
(folgt der Vendor-Naming-Convention).

**Tests:** `ffi_api::tests::implementation_identifier_matches_convention`,
`ffi_api::tests::check_rmw_identifier_accepts_correct`,
`ffi_api::tests::check_rmw_identifier_rejects_other_vendor`.

**Status:** done

### `rmw_node_t` Construction

**Spec:** `rmw/types.h` — `rmw_node_t`-Struct mit
`implementation_identifier`-Feld + `name`/`namespace`/`context`.

**Repo:** `crates/ros2-rmw/src/ffi_api.rs::RmwNode`.

**Tests:** `ffi_api::tests::rmw_node_construction`.

**Status:** done

---

## RMW-QoS-Mapping (`rmw/qos_profiles.h`)

### `rmw_qos_*_policy_t`-Enums (History/Reliability/Durability)

**Spec:** `rmw/qos_profiles.h` — drei C-Enums (`rmw_qos_history_policy_t`,
`rmw_qos_reliability_policy_t`, `rmw_qos_durability_policy_t`) mit
SYSTEM_DEFAULT / KEEP_LAST / KEEP_ALL / RELIABLE / BEST_EFFORT /
TRANSIENT_LOCAL / VOLATILE / UNKNOWN / BEST_AVAILABLE-Werten.

**Repo:** `crates/ros2-rmw/src/rmw_qos_mapping.rs::{RmwHistory,
RmwReliability, RmwDurability}` (`#[repr(u32)]`-Enums mit allen
spec-konformen Diskriminanten).

**Tests:** `rmw_qos_mapping::tests::enum_repr_is_c_compatible`.

**Status:** done

### `rmw_to_dds`-Konversion + `BEST_AVAILABLE`-Handling

**Spec:** `rmw/qos_profiles.h` + Design-Article — `rmw_qos_profile_t`
ist die ROS-2-Side-Repräsentation; muss bidirektional auf
DDS-`QosProfile` abgebildet werden. `BEST_AVAILABLE` (Iron+) wird auf
Sender-Seite zu `BEST_EFFORT` resolved.

**Repo:** `crates/ros2-rmw/src/rmw_qos_mapping.rs::{rmw_to_dds,
dds_to_rmw, RmwQosProfile}`.

**Tests:** `rmw_qos_mapping::tests::rmw_to_dds_round_trip_default`,
`rmw_qos_mapping::tests::rmw_to_dds_keep_all_passes_through`,
`crates/ros2-rmw/src/rmw_qos_mapping.rs::tests::rmw_system_default_passes_through_as_sentinel`,
`crates/ros2-rmw/src/rmw_qos_mapping.rs::tests::rmw_unknown_passes_through_as_sentinel`,
`rmw_qos_mapping::tests::rmw_best_available_maps_to_best_effort_on_sender`,
`rmw_qos_mapping::tests::transient_local_round_trips`,
`rmw_qos_mapping::tests::sensor_data_is_best_effort`,
`rmw_qos_mapping::tests::services_default_uses_keep_last_10`,
`rmw_qos_mapping::tests::parameters_uses_keep_last_1000`,
`rmw_qos_mapping::tests::default_profile_matches_rmw_spec`.

**Status:** done

---

## ROS-IDL → DDS-XTypes Wire-Mapping

### Sub-Namespace-Convention (msg/srv/action)

**Spec:** ROS 2 Design-Article
`design.ros2.org/articles/idl_interface_definition.html` §"Naming" —
Top-Level-Namespaces `msg/`, `srv/`, `action/` mit Convention auf
DDS-Wire `<package>::<sub-namespace>::dds_::<TypeName>_`.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::RosNamespace` mit
`as_str()`/`from_str()`-Konvertern; `RosTypeRef::to_dds_type_name`
für die DDS-Wire-Form.

**Tests:** `type_mapping::tests::namespace_str_repr`,
`type_mapping::tests::dds_wire_form_uses_dds_dunder`,
`type_mapping::tests::action_namespace_mapped_correctly`,
`type_mapping::tests::srv_namespace_mapped_correctly`.

**Status:** done

### ROS-Form → DDS-Form Round-Trip

**Spec:** Design-Article — Konvertierung zwischen ROS-Wire-Form
(`std_msgs/msg/String`) und DDS-Wire-Form (siehe oben). Roundtrip-
Pflicht.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::{RosTypeRef::new,
to_ros_form, from_ros_form}`.

**Tests:** `type_mapping::tests::ros_form_round_trip`,
`type_mapping::tests::from_ros_form_rejects_unknown_namespace`,
`type_mapping::tests::from_ros_form_rejects_wrong_segment_count`.

**Status:** done

### Builtin-Type-Tokens (`int32`, `float64`, `string`, ...)

**Spec:** Design-Article §"Field Types" — Builtin-Type-Tokens (`bool`,
`byte`, `char`, `float32`/`float64`, `int8`-`uint64`, `string`,
`wstring`) mappen auf DDS-IDL-Primitive.

**Repo:** `crates/ros2-rmw/src/type_mapping.rs::RosBuiltinType` mit
`from_ros_token()`-Parser und `cdr_size`-Helper.

**Tests:** `type_mapping::tests::from_ros_token_round_trip`,
`type_mapping::tests::from_ros_token_rejects_unknown`,
`type_mapping::tests::cdr_size_matches_omg_cdr2`,
`type_mapping::tests::builtin_idl_names_match_omg_idl`.

**Status:** done — rosidl-Builtin → OMG-IDL-4.2-/DDS-XTypes-1.3-Primitive-
Mapping verifiziert.

---

## RMW C-ABI Plugin (`crates/rmw-zerodds-shim`)

Die obigen 14 Items decken den Wire-**Mapping**-Crate (`crates/ros2-rmw`) ab.
Die eigentliche `rmw_*`-C-ABI-Plugin-Schicht (`rmw_c/rmw_zerodds.c` brückt auf
die Rust-Bridge `src/lib.rs`) ist hier separat geführt. Verifikation: rclpy-
Interop-Pytest auf ROS 2 Humble (`crates/py/python/tests/ros2/`), 4/4 grün.

### `rmw_wait` — event-driven

**Spec:** `rmw/rmw.h` `rmw_wait` (de-facto) — blockiert bis eine Entity ready
ist oder Timeout.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c` (`rmw_wait`) +
`src/lib.rs` (`WaitNotify`, `subscription_on_data`, `rmw_zerodds_*_has_data`,
`rmw_zerodds_context_wait_block`) — Reader-Daten-Listener → per-Subscription-
Inbox → Context-Condvar; kein Spin, kein Fixed-Tick-Poll. Für die Raw-Delivery-Modi
(RawSameHost/Iceoryx), die den RTPS-Listener nicht feuern, weckt zusätzlich ein
Doorbell-Thread (`rmw_zerodds_subscription_start_doorbell` → `zerodds_reader_raw_wait`)
dieselbe Condvar event-getrieben (siehe „Loaned Messages" unten).

**Tests:** `rmw-zerodds-shim::tests::event_driven_wait_roundtrip_inprocess`,
`wait_notify_blocks_then_wakes_on_notify`; rclpy
`test_rclpy_publish_subscribe_string_roundtrip` (Executor fährt durch `rmw_wait`).

**Status:** done

### Services — Request/Reply

**Spec:** `rmw/rmw.h` `rmw_create_client/service`, `rmw_send_request`,
`rmw_take_request`, `rmw_send_response`, `rmw_take_response`,
`rmw_service_server_is_available`.

**Repo:** `rmw_zerodds.c` (Service-Typesupport-Introspection, CDR
`[encap][body]`, rcl-Request-IDs) + `src/services.rs` (Topics `rq/…Request`/
`rr/…Reply`, Typen `…_Request_`/`_Response_`, Cyclone-Form und
Related-Identity-Form, Warten auf den Reply-Reader bis 100 ms).

**Tests:** `crates/rmw-zerodds-shim/src/lib.rs::tests::{a_service_call_between_two_runtimes_correlates_by_the_related_identity,
two_clients_in_one_runtime_each_get_their_own_reply}`;
rclpy `test_rclpy_service_call_roundtrip` (AddTwoInts 41+1=42);
`crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh` (Linux-Testhost, gegen
rmw_cyclonedds_cpp und rmw_fastrtps_cpp in beide Richtungen).

**Status:** done

### `rmw_serialize` / `rmw_deserialize` + serialized publish/take

**Spec:** `rmw/rmw.h` `rmw_serialize`, `rmw_deserialize`,
`rmw_publish_serialized_message`, `rmw_take_serialized_message[_with_info]`.

**Repo:** `rmw_zerodds.c` (Introspection-CDR `[encap 4][body]`,
`cdr_ser_msg`/`cdr_de_msg`; serialized pub/take über den Bridge-Byte-Pfad).

**Tests:** dieselbe Introspection-CDR wie der verifizierte pub/sub-Pfad
(`test_rclpy_publish_subscribe_string_roundtrip`).

**Status:** done

### Topic-Graph — `rmw_get_topic_names_and_types` + `rmw_count_publishers/subscribers`

**Spec:** `rmw/rmw.h` Graph-Introspektion.

**Repo:** `crates/dcps/src/runtime.rs`
(`discovered_publication_topics`/`discovered_subscription_topics` — lokale
User-Endpoints + SEDP-remote), c-api `zerodds_runtime_for_each_publication/
_subscription`, `rmw_zerodds.c` (Demangle `rt/<t>→/<t>`, `::`/`__`→`/`,
dedup topic→types).

**Tests:** rclpy `test_rclpy_topic_graph_introspection`.

**Status:** done

### Service-Graph und Namen pro Node

**Spec:** `rmw/rmw.h` `rmw_get_service_names_and_types`,
`rmw_count_services/clients`, `rmw_get_{publisher,subscriber,service,client}_names_and_types_by_node`,
`no_demangle` von `rmw_get_topic_names_and_types`,
`rmw_qos_profile_check_compatible`.

**Repo:** `rmw_zerodds.c` (`zerodds_demangle_service`,
`zerodds_demangle_service_type`, `zerodds_names_and_types_by_node`: Knoten
jedes Endpoints aus `ros_discovery_info`, Services = Reader auf Request-Topics,
Clients = Writer darauf), `rmw_zerodds_qos.cpp`
(`rmw_dds_common::qos_profile_check_compatible`, wie rmw_cyclonedds_cpp);
`src/services.rs` meldet Service-Endpoints in `ros_discovery_info` an.
Vorher lieferten alle diese Funktionen nichts bzw. „kompatibel".

**Tests:** `crates/rmw-zerodds-shim/rmw_c/run_ros2_services_test.sh`
(`ros2 service list -t` und `ros2 node info` über RMW-Grenzen, Linux-Testhost).

**Status:** done

### Guard-Conditions + event-driven Wake

**Spec:** `rmw/rmw.h` `rmw_create_guard_condition` / `rmw_trigger_guard_condition`
/ `rmw_wait`. rcl legt in `rmw_guard_conditions_t.guard_conditions[i]` den
`data`-Zeiger der Guard-Condition ab, nicht die `rmw_guard_condition_t`
(`rcl_wait_set_add_guard_condition`: `RMWStorage[n] = rmw_handle->data`); nach
dem Wait liest rcl genau dieses Array aus, um zu entscheiden, welche
Guard-Condition ausgelöst hat.

**Repo:** `rmw_zerodds.c` (`rmw_create/trigger_guard_condition` →
`context_notify` weckt die Wait-Condvar; `rmw_wait` liest die Array-Einträge als
`unsigned int *`-Flag, konsumiert ein gesetztes Flag und nullt die übrigen
Einträge).

Vorher las `rmw_wait` den Eintrag als `rmw_guard_condition_t *` und prüfte
dessen `implementation_identifier`. Der Vergleich traf nie zu, also meldete
keine Guard-Condition je Bereitschaft — weder die Interrupt-Guard-Condition des
Executors noch die Graph-Guard-Condition eines Nodes noch eine benutzereigene.
Zweiter Fehler auf demselben Pfad: ein Wait mit unendlichem Timeout gab bei
einem Weckruf ohne bereite Entität `RMW_RET_TIMEOUT` zurück; rcls
Graph-Listener behandelt das als fatalen Fehler und beendet den Prozess mit
„rcl_wait unexpectedly timed out".

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_negotiation_test.cpp` —
`"6) the graph change raised the node graph guard"`: das Event wird nur
gesetzt, wenn die Guard-Condition wirklich Bereitschaft meldet; gefahren über
`crates/rmw-zerodds-shim/rmw_c/run_type_negotiation_test.sh`. Der `rmw_wait`-Pfad selbst:
`crates/rmw-zerodds-shim/src/lib.rs::tests::wait_notify_blocks_then_wakes_on_notify`,
`::event_driven_wait_roundtrip_inprocess`, `::wait_returns_invalid_for_null`,
`::has_data_and_context_block_null_safe`.

**Status:** done

### `rmw_get_node_names` + `_with_enclaves`

**Spec:** `rmw/rmw.h` `rmw_get_node_names[_with_enclaves]`.

**Repo:** `rmw_zerodds.c` (`rmw_get_node_names`, Accumulator) + `src/lib.rs`
(`NodeGraph`, `encode/decode_participant_info`, `discovery_on_data`,
`rmw_zerodds_for_each_node`) — jeder Context publiziert seine
`ParticipantEntitiesInfo` auf `ros_discovery_info` (hand-encodiertes XCDR1) und
aggregiert lokale + remote Nodes.

**Tests:** rclpy `test_rclpy_node_names_graph`.

**Status:** done — `ros_discovery_info` wie in rmw_dds_common: NoKey,
RELIABLE, TRANSIENT_LOCAL, KEEP_LAST(1) (`src/lib.rs`, Discovery-QoS); ein
später gestarteter Prozess erhält den letzten Stand jedes Participants.

### `on_new_message/request/response`-Callbacks (Events-Executor)

**Spec:** `rmw/rmw.h` `rmw_*_set_on_new_*_callback`.

**Repo:** `rmw_zerodds.c` (3 Setter → Bridge) + `src/lib.rs`
(`SubInbox.event`, `inbox_set_event`, Invoke in `subscription_on_data`).

**Tests:** `rmw-zerodds-shim::tests::event_callback_fires_on_arrival`.

**Status:** done

### `rmw_get_serialized_message_size`

**Spec:** `rmw/rmw.h` `rmw_get_serialized_message_size`.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c`
(`rmw_get_serialized_message_size`, `zerodds_cdr_max_msg`,
`zerodds_cdr_max_elem` — Introspection-Size-Walk, konservative obere
Schranke; Strings/Sequences gecappt).

**Tests:** `crates/rmw-zerodds-shim/tests/ros2_ext_surface.rs::the_serialized_size_walk_covers_the_header_and_every_member_kind`
— prüft, dass der Rückgabewert den 4-Byte-Encapsulation-Header addiert,
dass jede Member-Art (Primitive, String, Nested, Scalar, Fixed-Array,
Sequence) einen Zweig im Walk hat und dass `ZERODDS_STR_CAP`/
`ZERODDS_SEQ_CAP` Platz reservieren statt null. Kein Gate.

**Status:** partial — der Walk ist implementiert und strukturell geprüft.
Was fehlt, ist die Verhaltensprüfung „der zurückgegebene Wert ist
tatsächlich ≥ der Länge, die `rmw_serialize` schreibt"; die braucht die
rosidl-Introspection-Typesupports und damit einen ROS-2-Host. Rest als
eigenes Item unten geführt.

### `rmw_get_serialized_message_size` — Schranken-Nachweis auf einem ROS-2-Host

**Spec:** `rmw/rmw.h` `rmw_get_serialized_message_size` — der gelieferte
Wert muss für die Vorab-Allokation reichen.

**Repo:** —

**Tests:** —

**Status:** open — es gibt keinen Testfall, der für eine reale Nachricht
`rmw_get_serialized_message_size` gegen die von `rmw_serialize`
geschriebene Länge hält. Voraussetzung ist ein ROS-2-Host; der Fall gehört
neben die vorhandenen rclcpp-e2e-Tests in
`crates/rmw-zerodds-shim/rmw_c/`.

### Typed-Message-Loaning (`rmw_borrow/publish/take_loaned_message`)

**Spec:** `rmw/rmw.h` Loaned-Message-API + `can_loan_messages`. Delivery-Form:
`docs/specs/zerodds-delivery-modes-1.0.md` (Modes `Portable`/`RawSameHost`/`Iceoryx`).

**Repo:** `rmw_c/rmw_zerodds.c` (`rmw_borrow/publish/take_loaned_message`,
`rmw_return_loaned_message_from_publisher/subscription`, `can_loan_messages`
= fixed-POD aus rosidl-Introspection) + die SHM-Bridge in
`crates/rmw-zerodds-shim/src/lib.rs`
(`rmw_zerodds_publisher_enable_raw_loan`/`_loan`/`_commit`/`_discard`,
`rmw_zerodds_subscription_enable_shm`/`_take_shm`/`_has_shm_data`/`_release_shm`),
Feature `flatdata-loan` (default).

**Tests:** `crates/rmw-zerodds-shim/src/lib.rs::tests::loaning_null_args_rejected`,
`crates/rmw-zerodds-shim/src/lib.rs::tests::return_loan_null_args_rejected`
(Argument-Kontrakt der Loan-ABI, kein Gate, laufen im Default-Lauf).
Verhalten in beiden Delivery-Modi:
`crates/rmw-zerodds-shim/rmw_c/loaned_message_test.cpp`, gestartet von
`crates/rmw-zerodds-shim/rmw_c/run_loaned_message_test.sh` — rclcpp-e2e,
läuft nur auf einem Host mit ROS 2 und ist im cargo-Default-Lauf nicht
enthalten.

**Status:** done — die Loaned-Message-ABI ist implementiert und e2e verifiziert,
in zwei Delivery-Modi (Default per `ZERODDS_DELIVERY_MODE`):

* **`Portable`** (default): rclcpp übergibt einen getypten Struct-Buffer, der
  User schreibt die Struct, `publish_loaned` serialisiert Struct→CDR und
  publiziert über RTPS — interop-sicher (cross-host/cross-vendor), echtes CDR
  auf dem Draht. (`can_loan=1 got=42 PASS`.)
* **`RawSameHost`** (echtes Zero-Copy/Zero-Serialize, same-host-only): der
  Writer ist auf `set_delivery_mode(RawSameHost)` + `enable_shm_loan` gestellt;
  `borrow` liefert einen Zeiger in den POSIX-SHM-Slot (der User schreibt die
  Struct direkt in Shared-Memory), `commit` finalisiert in-place ohne
  Serialisierung **und ohne RTPS** (c-api-`publishes_to_wire`-Gate → keine
  Double-Delivery). Der Reader mappt dasselbe Segment per deterministischem
  topic-abgeleitetem flink-Pfad (lazy attach) und liest den Slot zero-copy
  (`take_loaned`) bzw. mit einem Struct-memcpy für normale Callbacks
  (`rmw_take`). (`can_loan=1 got=42 PASS`, Beweis für SHM: Raw geht nie auf den
  Draht.)
* **`Iceoryx`** (same-host cross-stack, Shim-Feature `delivery-iceoryx`):
  derselbe loan/commit/take-Pfad, aber Writer/Reader sind an einen iceoryx2-
  Service (topic-abgeleitet) gebunden — `commit` sendet über iceoryx2, der Reader
  empfängt davon. Ohne das Feature degradiert `ZERODDS_DELIVERY_MODE=iceoryx`
  beidseitig auf `Portable`. (`can_loan=1 got=42 PASS` via `ZERODDS_TEST_ICEORYX=1`.)

Readiness (Modi 1/2): **event-getrieben**. Pro Raw-Subscription parkt ein
„Doorbell"-Thread auf `zerodds_reader_raw_wait(reader, timeout_ms)` (c-api), das
auf der Raw-Quelle blockiert — SHM-Change-Generation-Futex
(`notify_generation`/`wait_for_change`) bzw. iceoryx2-Listener — und bei einer
**echten** Ankunft die Context-Condvar von `rmw_wait` weckt; der Sender notifiziert
das iceoryx2-Event beim `commit`. Der Doorbell wird lazy gestartet, sobald die
Raw-Quelle aktiv ist, und vor dem Reader-Destroy gestoppt + gejoint (er hält den
Reader-Pointer). Da iceoryx2's `take` ein **destruktiver** FIFO-Receive ist,
prefetcht die Readiness zusätzlich genau ein Sample in einen Pending-Buffer der
Subscription (nicht-konsumierend, idempotent); `rmw_take`/`take_loaned`
konsumieren das gehaltene Sample. Damit weckt ein blockierendes rclcpp-`spin()`
auch bei reinen Raw-Daten ohne Executor-Timeout.

### Introspection-Typesupport-Varianten (C und C++)

**Spec:** `rosidl` liefert zwei Introspection-Typesupports mit identischem
`MessageMembers`/`ServiceMembers`-Layout, aber unterschiedlichen Laufzeittypen
für die variabel langen Member: `rosidl_typesupport_introspection_c`
(`rosidl_runtime_c__String`/`__U16String`, von `rclpy` aufgelöst) und
`rosidl_typesupport_introspection_cpp` (`std::string`/`std::u16string`, von
`rclcpp` aufgelöst). Ein rmw muss beide bedienen.

**Repo:** `rmw_c/rmw_zerodds.c` (`zerodds_introspect_ex`,
`zerodds_service_introspect_ex` liefern die Variante als Flag; das Feld `cpp` in
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c::cdr_t` und
`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c::rdr_t` trägt sie durch
Serialisierung und Deserialisierung; Publisher,
Subscription, Client und Service speichern sie in `members_cpp`) +
`rmw_c/rmw_zerodds_cxx.cpp` (`zerodds_cxx_string_data/_assign`,
`zerodds_cxx_u16string_data/_assign` — der C-TU kann `std::string` nicht
anfassen) + `rmw_c/build_librmw_zerodds_cpp.sh` (übersetzt und linkt die
C++-Übersetzungseinheit, `-lstdc++`).

Vorher wurde die C++-Variante wie die C-Variante behandelt. Ein
`std::string`-Member landete in `rosidl_runtime_c__String__assignn`, das den
SSO-Puffer der `std::string` als Heap-Zeiger realloziert — `realloc(): invalid
pointer`, Prozessabbruch beim ersten `std_msgs/msg/String` über `rclcpp`.
`zerodds_service_introspect` fragte außerdem nur die C-Variante ab, sodass
`rmw_create_client`/`rmw_create_service` für `rclcpp`-Services NULL lieferten.

**Tests:** `crates/rmw-zerodds-shim/rmw_c/type_adaptation_test.cpp` —
`"2) adapted publish -> plain subscription"`,
`"3) plain publish -> adapted take"`,
`"5) adapted service request/response roundtrip"` (`std_msgs/msg/String`
über `rclcpp`-Publisher und -Subscription in beiden Richtungen,
`example_interfaces/srv/AddTwoInts` über `rclcpp`-Client und -Service),
gefahren über `crates/rmw-zerodds-shim/rmw_c/run_type_adaptation_test.sh`. Regression der C-Variante:
`crates/py/python/tests/ros2/test_rmw_zerodds_interop.py::test_rclpy_publish_subscribe_string_roundtrip`
und `::test_rclpy_service_call_roundtrip` über `crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh`.

**Status:** done

---

## ROS 2 Jazzy Dynamic Type Support (`rosidl_dynamic_typesupport`)

### Interface-Tabelle + reflektive Abbildung

**Spec:** `rosidl_dynamic_typesupport/api/serialization_support_interface.h`
(ROS 2 Jazzy) — `rosidl_dynamic_typesupport_serialization_support_interface_t`,
192 Funktionszeiger: Dynamic-Type-Builder (jede Primitiv-/String-Art ×
skalar/Array/gebundene/ungebundene Sequenz, verschachtelte Member über
`dynamic_type_impl_t` und über `dynamic_type_builder_impl_t`), Dynamic-Data
(get/set/insert für jede Art, Sequenz-Operationen, verschachtelte
Werte, Loan, Serialisierung). Von `rosidl_dynamic_typesupport_fastrtps`
für Fast DDS und von diesem Repo für ZeroDDS implementiert — die
Beschreibung "Downstream middlewares should populate this interface as
appropriate" im Header selbst.

**Repo:** `crates/rmw-zerodds-shim/src/dynamic.rs` — jede
Interface-Funktion bildet auf `zerodds_types::dynamic::{DynamicTypeBuilder,
DynamicType, DynamicData}` ab, dieselbe reflektive Typ-Familie, die
`zerodds-dcps`s `SerializedTypeSupport` für laufzeitbeschriebene Samples
nutzt; Serialisierung/Deserialisierung geht durch `zerodds_wire_codec::
{encode,decode}`, denselben Codec-Pfad wie jeder andere `DynamicData`-Nutzer
im Repo. `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_dynamic.cpp` füllt die
192 Funktionszeiger; die ~90 dahinterliegenden `extern "C"`-Einstiege
sind händisch deklariert (cbindgen expandiert keine `macro_rules!`,
siehe Datei-Kommentar), nicht aus dem generierten `include/rmw_zerodds.h`
übernommen — derselbe Stil, den `rmw_c/rmw_zerodds.c` für seine eigenen
`extern`-Deklarationen schon nutzt.

Da `dynamic_type_builder_init` keinen gemeinsamen Modell-Zustand über
mehrere Aufrufe hinweg annehmen kann (jeder Aufruf bekommt ein frisches
`..._impl_t`, ohne Elternreferenz), baut jeder Builder sein eigenes,
privates `zerodds_types::dynamic::DynamicTypeBuilder`/`Ast`; ein
verschachtelter Typ wird beim Zusammensetzen (`add_complex_member[_builder]`)
per Graft — Knoten für Knoten neu deklariert, memoisiert — in das Ziel-`Ast`
kopiert, statt ein Modell zu teilen. Begründung im Modul-Kommentar von
`crates/rmw-zerodds-shim/src/dynamic.rs`. Der Graft kopiert jedes Facet, das ein Member oder der
Struct-Typ selbst trägt — `@key`, `@optional`, `@must_understand`,
`@external`, `@try_construct`, die effektive `@id`, `@default`, sowie
`@nested`/`@autoid(HASH)` und die Extensibility auf dem Typ —, nicht nur
Typ und effektive Id: eine frühere Fassung ließ jedes dieser Facets
außer der Id stillschweigend fallen, ein neu zusammengesetzter Typ
verlor also seinen Key, sein `@optional`, seinen `@default` und mehr,
ohne dass ein Fehler das meldete. Ein `@default`-Wert überlebt den
Graft über seinen Text, nicht über seinen `Id<ConstExprNode>` — der ist
zwischen zwei `Ast`-Instanzen nicht portabel — via
`zerodds_types::dynamic::default_expr_text`, der Umkehrfunktion zu
demselben Parser, der einen `@default`-Text erst gegen den Member-Typ
liest.

**Tests:** `cargo test -p rmw-zerodds-shim --lib dynamic::tests::` (15
grün): jede Primitiv-/String-Art rund-tripped über get/set/insert
(`every_primitive_kind_round_trips`), verschachtelte Structs + Sequenzen
(`nested_struct_and_sequence_round_trip`), Klon/Gleichheit auf Typ und
Daten (`clone_and_equals_agree`), Loan/Return-Loan mit korrektem
Zurückschreiben an die geliehene Stelle
(`a_loan_writes_back_at_its_own_origin`), eine gebundene Zeichenkette wird
bei Überlänge abgelehnt statt abgeschnitten
(`a_bound_violation_is_refused_not_truncated`), die Repräsentations-Kopfbytes
stimmen mit DDSI-RTPS 2.5 §10.5 überein
(`representation_of_matches_the_rtps_table`) — und, als Beleg für "byte-identisch
mit der statischen Typesupport-CDR derselben Nachricht":
`a_mixed_alignment_struct_matches_plain_cdr_byte_for_byte` vergleicht die von
`dynamic_data_serialize` erzeugten Bytes eines Structs mit `bool`+`int32`+
`float64` Byte für Byte gegen die von Hand berechnete Plain-CDR-Kodierung
(Alignment relativ zum Body-Anfang nach dem 4-Byte-Encapsulation-Header,
genau wie `rmw_zerodds.c`s eigener `cdr_align`/`rdr_align` — `(len - 4) % a`).
Für den Graft-Fix speziell: `a_grafted_members_facets_survive_and_match_idl_encoding`
baut einen verschachtelten Struct-Typ mit `@key`-, `@default(42)`- und
`@optional`-Member direkt über `DynamicTypeBuilder`/`NewMember` (die
C-Schnittstelle selbst hat für keines der drei einen Parameter), fügt
ihn über `zerodds_dyn_builder_add_nested_member` in einen äußeren Typ
ein, liest den überlebenden `@default` nach
`dynamic_data_init_from_dynamic_type` zurück und vergleicht die
serialisierten Bytes octet-für-octet mit derselben Deklaration, in IDL
geschrieben und direkt mit `zerodds_wire_codec::encode` kodiert — genau
dort, wo ein fallengelassenes `@key` oder `@optional` sich zeigen würde:
§7.4.1.1s Presence-Flag vor einem `@optional`-Member verschiebt jedes
Byte danach.

**Status:** done

### RMW-Integration (`rmw_serialization_support_init`, `rmw_take_dynamic_message[_with_info]`)

**Spec:** `rmw/dynamic_message_type_support.h` (ROS 2 Jazzy) — die drei
rmw-Einstiegspunkte, vorher mit `RMW_RET_UNSUPPORTED` beantwortet (wie
`rmw_cyclonedds_cpp`), jetzt echt implementiert.

**Repo:** `crates/rmw-zerodds-shim/rmw_c/rmw_zerodds_dynamic.cpp` —
`rmw_serialization_support_init` prüft den `serialization_lib_name` gegen
die eigene Kennung `"rmw_zerodds_cpp"` (RMW_RET_INCORRECT_RMW_IMPLEMENTATION
bei Fehlpassung) und ruft `rosidl_dynamic_typesupport_serialization_support_init`
mit der oben gefüllten Tabelle; `rmw_take_dynamic_message[_with_info]`
nutzt denselben rohen Sample-Pfad wie der statische Introspection-Codec
(`rmw_zerodds_take`/`_with_origin`, `librmw_zerodds.so`) und übergibt den
CDR-Stream — Encapsulation-Header inklusive — an
`dynamic_data_deserialize`, das die Repräsentation aus dem Header selbst
liest statt sie zu raten. `crates/rmw-zerodds-shim/ament/CMakeLists.txt`
und `rmw_c/build_librmw_zerodds_cpp.sh` bauen die neue Datei mit;
`find_package(rosidl_dynamic_typesupport QUIET)` (CMake) und
`<depend condition="$ROS_DISTRO != 'humble'">` (`package.xml`) halten
Humble unverändert baubar.

Der `default_value` eines Members deckt auch Array-/Sequence-Formen ab,
nicht nur Skalare: XTypes' `@default` (§7.3.4.5.4) kennt keine
Collection-Form, also bleibt der Default eines fixen Arrays/einer
Bounded-/Unbounded-Sequence komplett außerhalb des Modells und wird
(`crates/rmw-zerodds-shim/src/dynamic/collection_default.rs`) direkt in
`Vec<DynamicData>`-Elemente geparst, neben dem Typ gehalten
(`ZddsDynBuilder`/`ZddsDynType`/`ZddsDynData`) und bei
`dynamic_data_init_from_dynamic_type*` sowie bei
`dynamic_data_clear_value`/`clear_all_values`/`clear_nonkey_values`
angewendet. Der Text ist `rosidl_adapter`s `repr(tuple(value))`
innerhalb eines IDL-String-Literals (`@default(value="(1, 2, 3)")`) —
die Modul-Notiz gibt die Element-Grammatik (Bool `True`/`False`, eine
einfache Dezimalzahl für jede Integer-Art inklusive `byte`/`char`, eine
Dezimal- oder Exponential-Fließkommazahl oder `inf`/`-inf`/`nan`, ein
gequoteter, backslash-escapter String). Ein Text, der nicht parst, nicht
in den Wertebereich seiner Element-Art passt, über die eigene Bound
eines beschränkten Strings hinausgeht oder die falsche Elementanzahl
ergibt, wird abgelehnt, nie stillschweigend gekürzt, geklemmt oder
aufgefüllt.

**Tests:** Syntax-Check gegen die kopierten ROS 2 Jazzy Header
(`rmw`, `rcutils`, `rosidl_dynamic_typesupport`, `rosidl_runtime_c`,
`rosidl_typesupport_introspection_{c,cpp}`, plus einem selbst
geschriebenen Ersatz für `rclcpp`/`test_msgs`, der jedes von dieser
Datei aufgerufene Symbol abdeckt) via
`c++ -std=c++17 -fsyntax-only -Wall -Wextra -Wpedantic -Wshadow
-Wconversion` — 0 Fehler, 0 Warnungen; jeder von dieser Datei
aufgerufene `rosidl_dynamic_typesupport`-Funktionsname (63 insgesamt)
namentlich gegen diese Header geprüft.
`crates/rmw-zerodds-shim/rmw_c/dynamic_typesupport_test.cpp` +
`run_dynamic_typesupport_test.sh`: `std_msgs/msg/String` und
`test_msgs/msg/BasicTypes` (jede Primitiv-Art) über einen echten
Publisher, verglichen zwischen einer statisch typisierten
`rclcpp::Subscription` und einem über `rmw_take_dynamic_message`
genommenen `rosidl_dynamic_typesupport_dynamic_data_t` auf demselben
Topic; `test_msgs/msg/Arrays` und `test_msgs/msg/BoundedSequences`
ebenso, jede Primitiv-/String-Art erneut als fixes Array/Bounded
Sequence, je einmal ohne und einmal mit `default_value`, plus die drei
Nested-Message-Member und das abschließende Alignment-Skalar, die die
echten Nachrichten ebenfalls deklarieren. Gegen ROS 2 Jazzy (RoboStack)
auf dem Linux-Testhost gelaufen: 17 von 17 Prüfungen grün — vor den
seither ergänzten Checks für einen Member-Default, einen `long
double`-Roundtrip und Arrays/BoundedSequences; auf diesem Host noch
nicht neu gemessen.

`crates/rmw-zerodds-shim/src/dynamic/collection_default.rs`: 18
Unit-Tests für den Parser (leere/einelementige/mehrelementige Tupel,
der eigene Wertebereich jeder Integer-Art, `inf`/`-inf`/`nan`, Quoting
und Escaping auf beiden Seiten, ein Komma innerhalb eines
String-Literals, die falsche Elementanzahl für ein fixes Array und über
die Bound einer Bounded Sequence hinaus — jeweils abgelehnt).
`crates/rmw-zerodds-shim/src/dynamic.rs`: eine Struct mit je einem
Member jeder Collection-Form (fixes Array, Bounded Sequence, Unbounded
Sequence), jeder mit eigenem `default_value`, belegt, dass
`dynamic_data_init_from_dynamic_type`/`clear_value`/`clear_all_values`
den eigenen erfassten Default eines Collection-Members anwenden statt
Tabelle 9s leerer Collection, plus zwei Ablehnungs-Tests (falsche Länge
für ein fixes Array, über die Bound einer Bounded Sequence hinaus).

**Status:** partial — Interface-Tabelle und RMW-Wiring laufen
(`run_dynamic_typesupport_test.sh` gegen ROS 2 Jazzy; zuletzt gemessen
17/17 grün, vor den oben genannten Checks). Zwei benannte,
bewusste Lücken, keine davon
verdeckt: (1) `dynamic_data_serialize` kennt keine
verhandelte `DataRepresentation`-QoS — die Schnittstelle gibt der
Funktion keinen Writer/QoS-Kontext — und schreibt deshalb immer
`@final`, Little-Endian, XCDR1 (PLAIN_CDR1), identisch zur Framing-Wahl
des statischen Introspection-Codecs in `rmw_zerodds.c`. (2)
`long double`/`float128` wird nur für die x86-64-80-Bit-Extended- und
die aarch64-binäre128-Speicherform gebrückt (`Float128::
{to,from}_x87_padded_bytes`/`{to,from}_le_bytes`); 32-Bit-ARM und
PowerPCs Double-Double-`long double` werden mit RCUTILS_RET_UNSUPPORTED
abgelehnt statt geraten.

---

## Audit-Status

38 done / 2 partial / 1 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-ros2-rmw` (81 grün) +
`cargo test -p rmw-zerodds-shim` (45 grün: 24 Unit, 12 `shim_cli_e2e`,
5 CLI-Binary, 4 `ros2_ext_surface`) +
rclpy-Interop `crates/py/python/tests/ros2/` auf ROS 2 Humble (6 grün: init,
pub/sub, service-call, topic-graph, node-names, endpoint-info) via
`run_ros2_pytest.sh` +
rclcpp-Loan-e2e via `run_loaned_message_test.sh` (Portable + RawSameHost, je
`can_loan=1 got=42 PASS`) +
REP-2007-e2e via `run_type_adaptation_test.sh` (13 Prüfungen PASS, 0 failure) +
REP-2008-e2e via `run_accel_loan_memory_test.sh` (Portable 15 Prüfungen PASS,
RawSameHost 16 Prüfungen PASS, je 0 failure) +
REP-2009-e2e via `run_type_negotiation_test.sh` (16 Prüfungen PASS, 0 failure).

Alle ROS-2-Läufe auf dem Linux-Testhost (ROS 2 Humble via RoboStack/micromamba).

Offene Punkte: siehe `ros2-rmw.open.md`. Decision-Records: keine.
