<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Java endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading(id, value, label)` samples and delivers them; a subscriber decodes
**every field**.

```sh
cd endpoints/java
javac *.java
java ExampleSync     # subscriber owns the run-loop and polls
java ExampleAsync    # subscriber blocks on a BlockingQueue
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`ExampleSync`** — polls the transport in a loop (`null` when empty). The idiom
  when you own the run-loop.
- **`ExampleAsync`** — a reader `Thread` drains the transport into a
  `BlockingQueue`; the consumer blocks on `take()`. The idiomatic
  `java.util.concurrent` model.

## Transport

The examples use an in-memory FIFO (`ArrayDeque` / `ConcurrentLinkedQueue`); a real
UDP or serial link is a drop-in. XRCE WRITE_DATA framing is
`ZdwEndpoint.xrceWriteFrame` / `xrceReadBody`.

## Wire & codegen

The wire-core (`Zdw.java`) is pure JDK, byte-identical to the Rust core (XCDR2,
align cap 4, `f32` via `Float.floatToRawIntBits`). IDL types are generated with
`zerodds-idlc --java` (a full IDL4 codegen: struct/enum/union → sealed
interface + records/typedef/array/map/@mutable) — see
[`docs/specs/zerodds-xcdr2-java-1.0.md`](../../docs/specs/zerodds-xcdr2-java-1.0.md).
