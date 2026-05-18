# Apex.AI performance_test ZeroDDS-Plugin

CI-3d-Welle. Bridge-Plugin damit Apex.AI's `performance_test` ZeroDDS
als Communicator-Backend benutzen kann (`--communication ZeroDDS`).

## Architektur

```
Apex performance_test (C++ application)
   ↓
ZeroDdsCommunicator (dieses Plugin)
   ↓
zerodds.h (C-FFI, von dds-c-api/cbindgen erzeugt)
   ↓
libzerodds.so (Rust cdylib aus crates/dds-c-api)
```

## Standalone-Build (ohne Apex/ROS)

```bash
cargo build -p dds-c-api --release  # erzeugt libzerodds.so
cd tests/perf/apex/zerodds-plugin
cmake -B /tmp/build -DZERODDS_HOME=$(git rev-parse --show-toplevel)
cmake --build /tmp/build
# ergibt /tmp/build/libperformance_test_zerodds_plugin.{so,dylib}
```

Verifiziert dass das Plugin gegen die C-API linkt — ohne dass man
ROS oder Apex auf dem Build-Host braucht.

## In-Apex-Build (auf llvm-Host mit ROS)

```bash
ssh llvm@llvm
cd ~/performance_test  # Apex-Repo
ln -s /home/llvm/zerodds/tests/perf/apex/zerodds-plugin \
      ./performance_test_zerodds_plugin
. /opt/ros/jazzy/setup.bash
ZERODDS_HOME=/home/llvm/zerodds \
colcon build \
  --packages-select performance_test performance_test_zerodds_plugin
```

## Docker-Build (CI)

```bash
cd ~/zerodds
ln -s ~/performance_test ./performance_test
docker build -f tests/perf/apex/Dockerfile.zerodds -t apex-perf:zerodds .
```

Multi-Stage: Rust-Stage baut `libzerodds.so`, ROS-Stage baut Apex
mit Cyclone- + ZeroDDS-Plugin-Packages.

## Bench-Run

```bash
docker run --rm --net=host apex-perf:zerodds \
  --communication ZeroDDS --topic Array1k --rate 1000 --max_runtime 60 \
  --output_file /tmp/zd_self.csv
```

## Apex-Plugin-API-Hinweis

Apex.AI's `performance_test` 3.x exponiert pro Vendor ein
`Publisher`/`Subscriber`-Interface in `communication_abstractions/`.
Unser Plugin liefert die Implementierung als ament-Package
`performance_test_zerodds_plugin` und kann von Apex' Build-System
via Plugin-Selector eingebunden werden — der konkrete Patch in
Apex' CMake (z.B. `-DPERFORMANCE_TEST_PLUGIN=ZERODDS`) verlangt
Anpassung in deren `CMakeLists.txt`. Solange das nicht upstream
gemerged ist, laeuft der Run ueber einen Side-by-Side-Mode:
ZeroDDS-Plugin-Lib wird zur Laufzeit per `LD_PRELOAD` injected
und zwingt Apex' Default-Communicator-Factory zu unserem Backend.

## Discovery-Timing

Endpoint-Wiring (`wire_writer_to_remote_reader` /
`wire_reader_to_remote_writer`) im DcpsRuntime nutzt SPDP-discovered
Locators. Wenn ein Writer/Reader erstellt wird **bevor** SPDP einen
Peer gesehen hat, ist die Locator-Liste leer und der erste Wire-
Versuch findet kein Ziel.

**Selbstheilung:** SEDP-Builtin-Writer sind reliable und behalten
ihre History. Sobald SPDP einen Peer entdeckt, schickt
`sedp.on_participant_discovered` Heartbeats, der Peer NACK't, und
die historischen Pub/Sub-Daten werden nachgeliefert. `run_matching_pass`
laeuft beim Eintreffen jedes SEDP-Events und retried das Wiring —
diesmal mit gefuelltem SPDP-Cache. Empirisch (5×5 Runs auf llvm-Linux):
End-to-End-Pub-Sub kommt **ohne** explizites Warten in 600-720 ms
durch.

**Optionaler Helper:** `Communicator::wait_for_peers(min, timeout)`
bzw. `zerodds_runtime_wait_for_peers` blockt bis SPDP `min` Peers
gesehen hat. Das ist nuetzlich wenn man deterministisches Test-Setup
will oder einen langen Publish-Loop vermeiden moechte. Nicht
zwingend erforderlich — SEDP-Replay funktioniert auch ohne.

Spec-Hintergrund: gleiches Verhalten gilt fuer Cyclone DDS und
FastDDS — die internen `create_participant`-Calls warten ebenfalls
nicht auf Discovery, der Selbstheilungs-Cycle ist Teil der
RTPS-2.5-Discovery-Mechanik (§8.5.3).

## Status

- ✅ Plugin-Source `zerodds_communicator.{hpp,cpp}` C++17, RAII
- ✅ CMakeLists.txt baut standalone gegen libzerodds
- ✅ ament-Package-Manifest `package.xml`
- ✅ Dockerfile.zerodds (multi-stage Rust + ROS)
- ✅ Live-Smoke auf llvm-Linux: Pub-Sub-Roundtrip 3/3 Samples in 760 ms
- ✅ Race-Auflösung über `wait_for_peers` C-API (kein Sleep-Workaround)
- 🔲 Apex-CMake-Patch upstream (oder LD_PRELOAD-Variante) für
  performance_test --communication ZeroDDS
