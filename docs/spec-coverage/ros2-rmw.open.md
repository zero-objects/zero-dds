# ROS 2 RMW — Open + Partial Items

Aggregat aus `ros2-rmw.md`. Nicht von Hand pflegen — vor jedem Audit-
Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

### `rmw_qos_profile_*` Konstanten

**Status:** `partial` — sechs der sieben rmw-Default-Profiles 1:1
abgebildet. Zwei Spec-Treue-Abweichungen:
* `SYSTEM_DEFAULT` ist als Alias zum `DEFAULT`-Profile modelliert
  statt als `Implementation-defined`-Sentinel-Form (RMW-Spec verlangt
  pro QoS-Feld ein `*_SYSTEM_DEFAULT`-Sentinel-Variant). Aufwand
  0.5 PW falls strict-Sentinel verlangt.
* `rmw_qos_profile_unknown` als Konstante nicht abgebildet. Aufwand
  0.25 PW.

## Decision-Records (`n/a (rejected)`)

### REP-2007 Type Adaptation Feature — `n/a (rejected)`

**Begründung:** Andere Architektur-Schicht. REP-2007 ist ein Compile-
Time-C++-Feature in `rclcpp` (Sprach-Binding-Layer); ZeroDDS dockt via
`rmw_zerodds`-C-FFI an die RMW-Schicht darunter an. ROS-2-Anwender
bekommen Type Adaptation durch das ROS-2-Projekt selbst (`rclcpp`),
unabhängig davon welcher DDS-Vendor unter RMW arbeitet. Schichten-
Trennung:

```
Application
    ↓
rclcpp / rclpy            ← REP-2007, REP-2009 leben hier
    ↓
rcl                       ← C-Layer
    ↓
rmw                       ← rmw_zerodds-C-FFI dockt hier an
    ↓
DDS-Vendor                ← crates/dcps + crates/rtps
```

Im Gegensatz zu CORBA-Coexistence (`corba-3.3.md`) und CCM
(`omg-ccm-4.0.md`), wo ZeroDDS die ganze Schicht ersetzt, ist die
RMW-Trennung sauber definiert und das ROS-2-Ökosystem **integrabel**.

**Impl-Auswirkung:** Eine `done`-Implementation würde ein eigenes
`zerodds-rclcpp`-Konkurrenz-Sprach-Binding verlangen (C++-Header-
Generierung mit Type-Adaptation-Templates, traits-API). Schätzaufwand
~10-15 PW pro Sprach-Binding plus laufende Wartung gegen `rclcpp`-
Upstream-Drift.

**Impl-Vorteil:** Würde ZeroDDS als Vollständig-Stack-Alternative zu
OpenSource-ROS-2 positionieren. Aktuell nicht prioritär: ROS-2-Markt
ist via `rmw_zerodds`-FFI bereits adressiert, `rclcpp` ist aktiv
weiterentwickelt mit großer Community. Decision-Reversal nur durch
konkrete Anforderung an einen ZeroDDS-eigenen ROS-2-Sprach-Stack.

### REP-2008 Hardware Acceleration — `n/a (rejected)`

**Begründung:** Driver-/Vendor-Konvention für GPU/FPGA-Beschleunigung
(CUDA/ROCm/Vitis). Lebt in der Acceleration-Hardware-Vendor-Schicht
und wird durch ROS-2-Anwender direkt orchestriert, nicht durch
DDS-Vendor. Andock-Punkt für ZeroDDS ist RMW (Pub/Sub-Wire), nicht
Driver-Layer.

**Impl-Auswirkung:** Eine `done`-Implementation würde GPU/FPGA-
Driver-Bindings (CUDA-Streams, ROCm-Kernels, Vitis-Kernels) ins Crate
ziehen. Aufwand ~15-20 PW pro Hardware-Plattform plus permanente
Wartung gegen Vendor-SDK-Releases.

**Impl-Vorteil:** Würde ZeroDDS für GPU-Pipelines (Image-Processing,
ML-Inference) als Direkt-Backend ohne Caller-Driver-Glue positionieren.
Aktuell nicht prioritär: Use-Case ist erfüllt durch Caller-side
GPU-Stack mit `crates/dcps`-Pub/Sub als Transport.

### REP-2009 Type Negotiation Feature — `n/a (rejected)`

**Begründung:** Andere Architektur-Schicht. REP-2009 implementiert
Runtime-Type-Negotiation zwischen Pub/Sub in `rclcpp` (Sprach-Binding-
Layer); nutzt RMW nur als Pub/Sub-Wire. Negotiation-State-Machine +
Type-Resolution gehören zum ROS-2-Stack selbst, nicht zum DDS-Vendor.
Selbe Schichten-Trennung wie REP-2007.

**Impl-Auswirkung:** Wie REP-2007 — eigenes Konkurrenz-Sprach-Binding
nötig. Aufwand ~5-8 PW (kleiner als REP-2007 weil reine Runtime-Logik
auf `rclcpp`-Pub/Sub-Layer).

**Impl-Vorteil:** Wie REP-2007. Decision-Reversal-Trigger identisch.
