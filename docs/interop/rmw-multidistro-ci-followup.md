<!-- SPDX-License-Identifier: Apache-2.0 -->
# rmw-zerodds-shim — Multi-Distro-Live-Smoke in CI (Follow-up)

- **Status:** deferred (rc4+)
- **Sprint-Kontext:** Website-Walkthrough; beim Klären der (stale) „Phase A/B"-Marker
  auf `docs/rmw-zerodds-shim.html` bestätigt.

## Erledigt (kein Gap mehr)

Der Shim implementiert die **volle RMW-Oberfläche**, die rclcpp ansteuert — kein
exportierter Entry-Point liefert `RMW_RET_UNSUPPORTED`:
Pub/Sub, Service-Layer (Client+Service, Request/Response), Wait-Sets +
Guard-Conditions, Zero-Copy-Loaning, REP-2009-Type-Hash. 14 Unit-Tests.
ROS-2-Live-Interop ist separat belegt (rclpy auf ZeroDDS, bidirektional —
siehe `project_ros2_rmw_shim_live` / `project_ros2_live_interop`).

## Was ist offen

**Multi-Distro-Live-Smoke in der CI.** `ci/jobs/rmw-distro-build.yml` baut die
drei Ziel-Distros (Humble / Iron / Jazzy) in separaten Docker-Layern, aber ein
durchgehender **Live-Smoke** (`ros2 topic pub`/`echo` mit
`RMW_IMPLEMENTATION=rmw_zerodds_cpp` als Pipeline-Gate über alle drei ament-Versionen)
ist noch nicht als grünes CI-Gate verdrahtet.

## Warum offen

Bewusst auf rc4 zurückgestellt: Code-seitig ist die RMW-Oberfläche komplett;
dies ist reine **Verifikations-/CI-Infrastruktur** (drei ROS-Distro-Images im
Pipeline-Matrix-Build), kein Funktions-Gap.

## Wann pick-up sinnvoll

**rc4-Vorbereitung** bzw. ROS-2-Release-Härtung: ament-Build + Live-`ros2`-Smoke
je Distro als Pipeline-Stage, damit Distro-Drift (ament_cmake 1.5.x vs 2.x,
Ubuntu 22.04 vs 24.04) automatisch gefangen wird.

## Implementations-Pfad

1. Pro Distro ein CI-Image (Humble/Iron/Jazzy).
2. ament-Wrapper bauen → `RMW_IMPLEMENTATION=rmw_zerodds_cpp` → `ros2 topic pub|echo`
   Roundtrip als Assertion.
3. Als required Stage ins Pipeline-Gate.
