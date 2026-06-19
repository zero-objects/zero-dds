# ROS 2 RMW — Open + Partial Items

Aggregated from `ros2-rmw.md`. Do not edit by hand — delete and regenerate
from the main file before each audit run.

## Open

— none.

## Partial

— none.

## Decision Records (out of rmw scope)

### REP-2007 Type Adaptation feature — `n/a (no rmw surface)`

**Rationale:** different architecture layer. REP-2007 is a compile-time C++
feature in `rclcpp` (the language-binding layer); ZeroDDS attaches via the
`rmw_zerodds` C-FFI to the RMW layer below it. ROS-2 users get type
adaptation through the ROS-2 project itself (`rclcpp`), independent of which
DDS vendor sits under RMW. Layer separation:

```
Application
    ↓
rclcpp / rclpy            ← REP-2007, REP-2009 live here
    ↓
rcl                       ← C layer
    ↓
rmw                       ← rmw_zerodds C-FFI attaches here
    ↓
DDS vendor                ← crates/dcps + crates/rtps
```

Unlike CORBA coexistence (`corba-3.3.md`) and CCM (`omg-ccm-4.0.md`), where
ZeroDDS replaces the whole layer, the RMW separation is cleanly defined and
the ROS-2 ecosystem is **integrable**.

### REP-2008 Hardware Acceleration — `n/a (out of rmw scope)`

**Rationale:** a driver/vendor convention for GPU/FPGA acceleration
(CUDA/ROCm/Vitis). It lives in the acceleration-hardware vendor layer and is
orchestrated directly by ROS-2 users, not by the DDS vendor. ZeroDDS'
attachment point is RMW (pub/sub wire), not the driver layer.

### REP-2009 Type Negotiation feature — rmw side `done`, rclcpp part out of scope

**Rationale:** Out of rmw scope is only the negotiation state machine itself — a
runtime feature in `rclcpp`, available over any RMW; no rmw surface. The rmw-side
part (RIHS type hash + endpoint info) is `done` — see `ros2-rmw.md`.
