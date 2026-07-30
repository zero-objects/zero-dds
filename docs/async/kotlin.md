<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Kotlin (native endpoint)

A native **pure-Kotlin** endpoint SDK (ADR 0013) on the JVM: a from-scratch XCDR
wire-core, byte-identical to the Rust core and the other SDKs. **Sync** (blocking)
and **async** (a background receive thread pushing decoded bodies onto a
blocking-queue channel) — no external dependencies, just the Kotlin stdlib + JDK.

Sources: [`endpoints/kotlin/src`](../../endpoints/kotlin/src) · example:
[`endpoints/kotlin/example`](../../endpoints/kotlin/example).

## Sync

```kotlin
val c = Client(transport)            // transport: you implement deliver/receive
c.write(sampleXCDR)                   // frame as XRCE WRITE_DATA + deliver
c.poll()?.let { body -> Reader(body, Endian.LITTLE).getU32() }
```

## Async (background thread + channel)

```kotlin
val r = AsyncReader(transport)        // spawns a daemon receive thread
val body = r.samples.poll(5, TimeUnit.SECONDS)   // decoded bodies on a channel
r.close()
```

## Wire-core

`Writer` / `Reader` cover the XCDR primitives with alignment and LE/BE.
`Float.floatToRawIntBits` gives the IEEE bit pattern, so `f32` matches the Rust
core.

## Tests (CI job `endpoints-kotlin`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (background thread + channel)
- the runnable example

Built with Kotlin 1.9 on JDK 17.
