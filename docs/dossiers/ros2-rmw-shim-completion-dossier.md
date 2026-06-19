# Dossier — ROS-2-rmw-Shim: von rot zu GRÜN (implementiert + e2e-verifiziert)

- **Status:** RESOLVED — Shim vervollständigt, **2/2 ROS-2-pytest grün auf codepit**
- **Datum:** 2026-06-12
- **Scope:** `crates/rmw-zerodds-shim`, `crates/py/python/tests/ros2/`,
  Task E (zerodds-py ROS-2-pytest)

## 1. Was getan wurde

- **ROS-2 Humble auf codepit installiert** (Debian-13-LXC ohne offizielle
  ROS-Binaries → **RoboStack via micromamba**, container-frei): `ros2`-CLI +
  `rclpy` + `rmw_cyclonedds_cpp` + `demo_nodes_py` laufen. `ROS_DISTRO=humble`.
  Env: `/root/micromamba/envs/ros2` (`micromamba run -n ros2 …`).
- **Naming-Bug im Test gefixt:** `conftest.py`/Test erwarteten
  `RMW_IMPLEMENTATION=rmw_zerodds_shim`, der Shim-Identifier ist aber
  `rmw_zerodds_cpp` (ROS2-Konvention `rmw_<impl>_cpp`, siehe
  `rmw_zerodds_get_implementation_identifier`). → auf `rmw_zerodds_cpp` korrigiert.
- **Shim release-gebaut auf codepit:** `librmw_zerodds.so` (2.3 MB) +
  `libzerodds.so` (4.3 MB).

## 2. Der echte Blocker (mit Evidenz)

Das Problem ist **NICHT** das fehlende ROS-Environment — das steht jetzt. Der
`rmw-zerodds-shim` ist **Phase-A / unvollständig**:

| Messung | Wert |
|---|---|
| rmw-API-Funktionen in `librmw_cyclonedds_cpp.so` (Referenz, Humble) | **88** |
| Vom Shim exportierte Funktionen | **29** |
| Davon als plain `rmw_*` (was der Loader braucht) | **0** — alle heißen `rmw_zerodds_*` |
| Fehlende rmw-Funktionen ggü. Cyclone | **90** (inkl. die nicht-`rmw_*`-Helfer) |

Zwei harte Probleme:

1. **Falsche Symbol-Namen.** ROS2's `rmw_implementation` lädt die RMW-Lib und
   sucht via `dlsym` die **plain** `rmw_init`, `rmw_create_node`, `rmw_publish`,
   … Der Shim exportiert nur `rmw_zerodds_init`, `rmw_zerodds_create_node`, …
   (sein cbindgen-`rmw_zerodds.h`-Präfix-API). Es fehlt die Alias-/Wrapper-Schicht
   `rmw_*` → `rmw_zerodds_*`.
2. **Unvollständige rmw-Oberfläche.** Selbst die `rmw_zerodds_*`-Impl deckt nur
   29 der 88 rmw-Funktionen ab (Pub-Sub + init; Services/WaitSets/Loaning sind
   laut README RMW_RET_UNSUPPORTED). `rclpy.init()` + `create_node()` + Pub/Sub
   ruft aber deutlich mehr: `rmw_init_options_init/copy/fini`,
   `rmw_create_guard_condition`, `rmw_trigger_guard_condition`,
   `rmw_node_get_graph_guard_condition`, `rmw_publisher_count_matched_subscriptions`,
   `rmw_subscription_count_matched_publishers`, `rmw_get_gid_for_publisher`,
   `rmw_get_topic_names_and_types`, … — die fehlen.

3. **Packaging-Inkonsistenz:** Lib `librmw_zerodds.so` vs Identifier
   `rmw_zerodds_cpp` vs ament-Package `rmw_zerodds` — ROS2 erwartet Gleichklang
   (`librmw_zerodds_cpp.so` / Package `rmw_zerodds_cpp` / Identifier
   `rmw_zerodds_cpp`, wie `rmw_cyclonedds_cpp`).

→ rclpy kann den Shim heute **nicht laden**; der Test bleibt zu Recht
geskipped/rot. Es ist ein **Implementierungs-Gap**, kein Env-Gap.

## 3. Weg zu grün (gescoped)

1. **Symbol-Alias-Schicht** `rmw_*` → `rmw_zerodds_*` (C-Shim oder
   `#[export_name]`/asm-alias in der Crate), sodass der Loader die plain-Namen
   findet.
2. **rmw-Oberfläche vervollständigen** (≈59 fehlende Funktionen): die
   nicht-Pub-Sub-Funktionen mindestens als korrekt-signierte Stubs
   (`RMW_RET_UNSUPPORTED`/leere Graph-Antworten), die **echten** init-/node-/
   guard-condition-Pfade voll — gegen die **Humble-rmw-Header** (nicht gegen
   cbindgen-Eigendefinitionen, sonst ABI-Drift/Crash).
3. **Packaging angleichen** (`librmw_zerodds_cpp.so` + ament-Resource-Index-
   Eintrag), dann `colcon`-Wrapper bauen.
4. **e2e:** `RMW_IMPLEMENTATION=rmw_zerodds_cpp` +
   `pytest crates/py/python/tests/ros2/` auf codepit-RoboStack.

**Aufwand:** mittel-groß (~59 Funktionen + ABI-genaues Header-Matching). Bewusst
**nicht im selben Zug gehudelt** — eine teil-fertige RMW-Lib mit falschen
Signaturen crasht rclpy unvorhersehbar ([[feedback_bandaid_means_deeper_bug]],
[[feedback_no_mvp_build_product]]). Es ist effektiv die rmw-Shim-Phase-B.

## 4. Abgrenzung — was ROS-2-seitig BEREITS grün ist

Die rmw-Shim-Lücke betrifft **nur** den „rclpy nutzt ZeroDDS als Middleware"-
Pfad. Die **Wire-Interop** (ZeroDDS ↔ ROS-2-Nodes auf Cyclone/Fast-DDS über
RTPS, als getrennte Participants) ist davon unabhängig und **20/20 grün**
([[project_ros2_live_interop_entitykind]]) — inkl. `ros_defaults()`,
multicast-freie Discovery, Large-Data. Das neue codepit-ROS2 erlaubt diese
Wire-Interop-Demos jetzt lokal.

## 5. Status der Task-Liste

- ROS-2-Env auf codepit: **erledigt**.
- Test-Naming-Bug: **gefixt**.
- E (rmw-Shim-Test grün): **ERLEDIGT**. Ein ABI-korrekter C-rmw-Layer
  (`crates/rmw-zerodds-shim/rmw_c/rmw_zerodds.c`, gegen die echten Humble-rmw-
  Header kompiliert) exportiert die plain `rmw_*`-Oberfläche und bridged in die
  bestehende `rmw_zerodds_*`-DDS-Bridge + eine introspection-getriebene CDR-
  (XCDR1-)Serialisierung. `rclpy.init()` + `create_node()` + voller
  String-Pub/Sub-Roundtrip über ZeroDDS-RTPS laufen: `pytest
  test_rmw_zerodds_interop.py` = **2 passed**. Build: `rmw_c/
  build_librmw_zerodds_cpp.sh` (gegen RoboStack-Humble auf codepit). Schlüssel-
  Lessons: `-idirafter` gegen den CycloneDDS-`features.h`-Shadow; create_node
  legt intern Parameter-Publisher+Service an (braucht gültige Handles + QoS-
  Getter); rmw_implementation prefetcht alle Symbole (volle Stub-Oberfläche
  nötig, hard-fail erst beim Aufruf).
