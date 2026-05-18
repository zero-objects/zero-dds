# CI-3c — Apex.AI performance_test Cross-Vendor

Stand 2026-05-02. **Status: ✅ Cyclone-Self via Docker funktioniert.
Cross-Vendor + ZeroDDS-Plugin = CI-3d (C-API-Welle).**

## TL;DR (was funktioniert)

`apex-perf:cyclonedds` Docker-Image auf llvm-Host gebaut (osrf/ros:
jazzy-desktop + ros-jazzy-cyclonedds + Apex performance_test compiled
mit `-DPERFORMANCE_TEST_PLUGIN=CYCLONEDDS`). 15 s Smoke-Run gegen
Array1k @ 1000 Hz lieferte:
* rate ~1000 samples/sec
* latency_mean ~170 µs (mit ~14 µs Variance pro Sekunde)
* latency_min ~120 µs, max ~1.1 ms (Cold-start-tail bei 1. Sekunde)
* 0 samples lost in steady-state

Image-Build steckt jetzt in `tests/perf/apex/Dockerfile` (im Repo);
Build-Schritt auf llvm:
```bash
cd ~/performance_test
docker build -f tests/perf/apex/Dockerfile -t apex-perf:cyclonedds .
```

`tests/perf/llvm_bench_runner.sh` Step 4d ruft das Image automatisch
falls vorhanden, sonst Skip mit Hinweis.

## Ziel

Cross-Vendor-Performance-Bench (Latenz + Throughput + CPU/RAM) zwischen
Cyclone DDS, eProsima Fast-DDS und ZeroDDS unter identischer Workload.
Apex.AI `performance_test` ist die de-facto-Standard-Suite für
DDS-Plugin-Vergleichs-Benchmarks (z.B. Apex Performance Report).

Ergaenzt CI-3b (`zerodds_perf` Self-Bench) durch:

* Strukturierten CSV-Output mit p50/p90/p99-Latenz-Histogrammen
* CPU + RAM-Usage pro Run
* Identische Workload-Definition (PlotJuggler-kompatible Logs)
* Vendor-Plugin-Architektur fuer ZeroDDS-Integration

## Voraussetzung: ROS 2 auf llvm-Host

Apex.AI nutzt `ament_cmake` aus dem ROS-2-Build-System. Standalone-
Build geht NICHT ohne ament. Optionen:

### Option A: ROS 2 jazzy via apt

```bash
ssh root@llvm
sudo apt install software-properties-common curl gnupg lsb-release
sudo add-apt-repository universe
sudo curl -sSL https://raw.githubusercontent.com/ros/rosdistro/master/ros.key \
    -o /usr/share/keyrings/ros-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) \
    signed-by=/usr/share/keyrings/ros-archive-keyring.gpg] \
    http://packages.ros.org/ros2/ubuntu $(lsb_release -cs) main" | \
    sudo tee /etc/apt/sources.list.d/ros2.list
sudo apt update
sudo apt install ros-jazzy-ros-base ros-jazzy-cyclonedds \
    ros-jazzy-rmw-cyclonedds-cpp ament-cmake
```

Footprint: ~1.5 GB, install dauert 10-15 min.

### Option B: Docker-Image (kein Apt-Pollute)

```bash
docker pull apexai/performance_test:latest
```

Vorteil: keine Host-Pollution. Nachteil: extra Docker-Layer in
jedem Run + Multicast-Loopback-Tunneling-Issues.

**Empfehlung: Option A**, weil llvm-Host eh dediziert für Bench-Runs ist.

## Apex.AI Build (nach Option A)

```bash
ssh llvm@llvm
. /opt/ros/jazzy/setup.bash
cd ~/performance_test  # bereits gecloned in CI-3-Welle
colcon build --cmake-args -DPERFORMANCE_TEST_RT_ENABLED=OFF \
    --packages-select performance_test
. install/setup.bash
which perf_test  # /home/llvm/performance_test/install/performance_test/bin/perf_test
```

## Vendor-Plugin-Auswahl

Apex.AI braucht pro Vendor einen Plugin-Adapter. Eingebaut sind:

* `--communication CycloneDDS` — funktioniert via ros-jazzy-cyclonedds
* `--communication FastRTPS` — braucht eProsima-Fast-DDS-CMake-Config
* `--communication rclcpp` — braucht rclcpp + RMW-Adapter
* **`--communication ZeroDDS`** — muss neu geschrieben werden

### ZeroDDS-Plugin-Skelett

`performance_test/plugins/zerodds/` (zu erstellen):

```cpp
// publisher_zerodds.hpp — Apex-Plugin-Interface
class ZeroDdsPublisher : public Publisher {
    void publish(const Sample &s) override {
        // FFI-Call in ZeroDDS C-API
    }
};
```

Voraussetzung: ZeroDDS muss eine C-API exportieren. Aktuell: nur
Rust-API. C-API wäre eine separate WP (~3-5 PT für initial-bindings via
`cbindgen`-generated header + `extern "C"` shim).

**Pragmatisch für CI-3c v1:** Nur Cyclone+Fast-DDS-Cross-Vendor, kein
ZeroDDS-Plugin. ZeroDDS-Numbers kommen aus CI-3b (`zerodds_perf`).
ZeroDDS-Plugin als CI-3d.

## Test-Skelett (vorbereitend)

`tests/perf/llvm_apex_runner.sh` (zu erstellen):

```bash
#!/usr/bin/env bash
# Apex.AI cross-vendor perf-test runner.
. /opt/ros/jazzy/setup.bash
. ~/performance_test/install/setup.bash

OUT="$WORKDIR/apex-output"
mkdir -p "$OUT"

# Matrix: CycloneDDS-vs-CycloneDDS (baseline), FastRTPS-vs-FastRTPS,
# Cross: Cyclone-Pub vs FastRTPS-Sub (echter Cross-Vendor-Wire-Test).
for vendor in CycloneDDS FastRTPS; do
    perf_test --communication "$vendor" \
              --topic Array1k --rate 1000 --max_runtime 60 \
              --output_file "$OUT/${vendor}_self.csv" \
              --logfile "$OUT/${vendor}_self.log"
done

# Cross-Vendor (zwei Prozesse, gleiches Topic):
perf_test --communication CycloneDDS --topic Array1k \
    --pub_loop --max_runtime 60 \
    > "$OUT/cyc_pub.log" 2>&1 &
PUB_PID=$!
perf_test --communication FastRTPS --topic Array1k \
    --sub_loop --max_runtime 60 \
    --output_file "$OUT/cyc_pub_fastrtps_sub.csv" \
    > "$OUT/fastrtps_sub.log" 2>&1 &
SUB_PID=$!
wait "$PUB_PID" "$SUB_PID"
```

## CI-Job-Skelett

```yaml
bench-llvm-apex:
  stage: bench
  needs: [bench-compile]
  timeout: 30 minutes
  resource_group: dcps-multicast   # nutzt Cyclone-Multicast
  rules:
    - if: '$RUN_BENCH_APEX == "true"'
      when: on_success
    - if: '$CI_COMMIT_BRANCH == "main"'
      when: manual
      allow_failure: true
  variables:
    LLVM_BENCH_HOST: "llvm"
    LLVM_BENCH_USER: "llvm"
  before_script:
    - !reference [default, before_script]
    - mkdir -p ~/.ssh && chmod 700 ~/.ssh
    - cp "$LLVM_BENCH_SSH_KEY" ~/.ssh/id_ed25519
    - chmod 600 ~/.ssh/id_ed25519
    - echo "$LLVM_BENCH_HOST_KEY" >> ~/.ssh/known_hosts
  script:
    - scp tests/perf/llvm_apex_runner.sh "$LLVM_BENCH_USER@$LLVM_BENCH_HOST:/tmp/llvm_apex_runner.sh"
    - ssh "$LLVM_BENCH_USER@$LLVM_BENCH_HOST" "bash /tmp/llvm_apex_runner.sh"
    - mkdir -p apex-bench-out
    - scp -r "$LLVM_BENCH_USER@$LLVM_BENCH_HOST:zerodds-bench-apex/apex-output/*" apex-bench-out/
  artifacts:
    when: always
    paths:
      - apex-bench-out/
    expire_in: 30 days
```

## Folgeschritte

* [ ] ROS 2 jazzy auf llvm-Host installieren (Option A, ~10-15 min)
* [ ] Apex.AI mit colcon bauen (cyclonedds + fastrtps Plugins)
* [ ] `tests/perf/llvm_apex_runner.sh` schreiben + verifizieren
* [ ] CI-Job `bench-llvm-apex` aktivieren
* [ ] (CI-3d) ZeroDDS-C-API exponieren + Apex-Plugin schreiben
