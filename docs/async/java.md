<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Java (binding async surface)

The Java binding (`org.omg.dds`, OMG DDS-PSM) gets an idiomatic
**`CompletableFuture`** async surface via `org.zerodds.DdsAsync` — the blocking
wait/write calls lifted onto an `Executor` (the common `ForkJoinPool` by
default) so they compose with `CompletableFuture` pipelines without blocking the
caller.

Source: [`DdsAsync.java`](../../crates/java-omgdds/java/src/main/java/org/zerodds/DdsAsync.java) ·
tests: [`DdsAsyncTest.java`](../../crates/java-omgdds/java/src/test/java/org/omg/dds/DdsAsyncTest.java).

## Surface

```java
import org.zerodds.DdsAsync;
import java.time.Duration;

// write — completes with the ReturnCode
DdsAsync.writeAsync(writer, sample);

// wait — true once samples are present, false at timeout
CompletableFuture<Boolean> ready = DdsAsync.waitForSamplesAsync(reader, Duration.ofSeconds(2));

// take — waits up to the timeout, then take()s (empty list on timeout)
CompletableFuture<List<Sample<T>>> got = DdsAsync.takeAsync(reader, Duration.ofSeconds(2));

got.thenAccept(samples -> samples.forEach(s -> handle(s.data())));
```

Each method has an overload taking an explicit `Executor`, so the wait/write
work can run on a caller-supplied pool.

## Tests (CI job `endpoints-java-async`)

`mvn test` on `crates/java-omgdds/java` runs the whole binding suite including
`DdsAsyncTest`: `writeAsync` → `takeAsync` round-trip, `waitForSamplesAsync`
true-when-present and false-on-timeout. Toolchain: `maven` + JDK 17 from apt.
