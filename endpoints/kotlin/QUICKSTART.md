<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Kotlin endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/kotlin
kotlinc src/Zerodds.kt example_sync/Example.kt  -include-runtime -d sync.jar  && java -jar sync.jar
kotlinc src/Zerodds.kt example_async/Example.kt -include-runtime -d async.jar && java -jar async.jar
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `null` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader` runs a daemon thread that pushes decoded
  frames onto a `LinkedBlockingQueue`; you `samples.take()` (blocking) and
  `close()` when done. The idiom for channel-style consumers.

## Transport

`Transport` is a two-method interface (`deliver` / `receive`). The examples use
the thread-safe in-memory `MemTransport`; a real UDP or shared-memory link is a
drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Float.floatToRawIntBits`). Kotlin is JVM-native, so IDL types are generated with
`zerodds-idlc --java` and consumed directly — see
[`docs/specs/zerodds-xcdr2-kotlin-1.0.md`](../../docs/specs/zerodds-xcdr2-kotlin-1.0.md).
