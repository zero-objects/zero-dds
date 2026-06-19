# ROS 2 RMW — Open + Partial Items

Aggregat aus `ros2-rmw.md`. Nicht von Hand pflegen — vor jedem Audit-
Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

— keine.

## Decision-Records (out of rmw scope)

### REP-2007 Type Adaptation Feature — `n/a (keine rmw-Fläche)`

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

### REP-2008 Hardware Acceleration — `n/a (out of rmw scope)`

**Begründung:** Driver-/Vendor-Konvention für GPU/FPGA-Beschleunigung
(CUDA/ROCm/Vitis). Lebt in der Acceleration-Hardware-Vendor-Schicht
und wird durch ROS-2-Anwender direkt orchestriert, nicht durch
DDS-Vendor. Andock-Punkt für ZeroDDS ist RMW (Pub/Sub-Wire), nicht
Driver-Layer.

### REP-2009 Type Negotiation Feature — rmw-Seite `done`, rclcpp-Teil out-of-scope

**Begründung:** Out of rmw scope ist nur die Negotiation-State-Machine selbst —
ein Runtime-Feature in `rclcpp`, verfügbar über jeden RMW; keine rmw-Fläche. Der
rmw-seitige Teil (RIHS-Type-Hash + Endpoint-Info) ist `done` — siehe `ros2-rmw.md`.
