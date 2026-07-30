# `org.zerodds.idlc` Gradle plugin

A real `Plugin<Project>` (not a script-only convention) that runs
`zerodds-idlc` as a Gradle build step, wired the way `com.google.protobuf`
wires `protoc`: a `generateIdl` task with declared `@InputFiles`/
`@OutputDirectory` (so Gradle's up-to-date check skips regeneration when no
`.idl` changed), auto-added to the `main` source set's `java` dirs, and
`compileJava` made to `dependsOn` it.

## Usage

```kotlin
// settings.gradle.kts
pluginManagement {
    includeBuild("../path/to/build-integration/java-gradle/plugin")
    // or, once published: repositories { gradlePluginPortal() }
}
```

```kotlin
// build.gradle.kts
plugins {
    id("java")
    id("org.zerodds.idlc")
}

zeroddsIdlc {
    idlFiles.from("idl/Robot.idl")
    backend.set("java")               // default
    // outputDir.set(layout.buildDirectory.dir("generated/idl"))  // optional override
    // includeDirs.from("idl/common")                              // -I search dirs
    // executable.set("/opt/zerodds/bin/zerodds-idlc")             // override PATH lookup
}
```

`./gradlew build` now runs `generateIdl` before `compileJava` automatically,
and only re-invokes `zerodds-idlc` when an input `.idl` file changed.

## Validation status

No `gradle`/JVM-build-tool install was available for this pass on either
the local macOS dev machine or the shared Linux validation host (the
host was flagged mid-task as overloaded — further installs/builds there
were stopped rather than pushed through). The Kotlin source, task/plugin
wiring, and `settings.gradle.kts` composite-build setup were written and
reviewed but **not build-validated** — flagged here rather than claimed.
Central serial validation (once the shared host has room) should run
`./gradlew build` in `sample-consumer/`.

What *is* independently confirmed, because the equivalent Maven path was
built end-to-end against a real, locally-built `zerodds-idlc`: the
`--java` backend's generated output needs the `org.zerodds:omgdds`
runtime (`crates/java-omgdds/java`, `mvn install`ed locally first — this
sample's `build.gradle.kts` already declares `mavenLocal()` +
`implementation("org.zerodds:omgdds:0.0.0")`) **and still won't fully
compile** — `org.zerodds.types.{Extensibility,Key}` and
`org.omg.dds.topic.TopicType<T>` don't exist anywhere in the repository.
See `../../java-maven/README.md` for the full finding (exact generated
source lines, grep evidence). This plugin/task wiring is unaffected by
that gap (it is a runtime-completeness issue, not a build-step-plumbing
one) but `./gradlew run` will hit the same missing symbols Maven's
`mvn compile` did.

## What a fuller pass would add

This plugin covers the common case (one backend, flat `.idl` set,
`generate` sub-command). Not yet implemented, flagged rather than silently
skipped:

- **Per-file `#include` dependency tracking.** `zerodds-build` (Rust) and
  `print-deps` (the CLI sub-command) expose the transitive `#include` graph;
  this task only declares the top-level `.idl` files as `@InputFiles`, so
  editing an `#include`d file will not by itself invalidate the
  up-to-date check. Fix: shell out to `zerodds-idlc print-deps` at
  configuration time (or after each run) and feed the result back into
  `idlFiles`.
- **Multi-backend fan-out in one task invocation** (today: one
  `zeroddsIdlc` extension = one backend; consumers wanting Java *and* C#
  from the same `.idl` apply the extension/task pattern twice under
  different names).
- **Publishing to Gradle Plugin Portal / an internal Maven repo** — this
  module is consumed today via `includeBuild` (composite build) from the
  sample consumer, not `id("org.zerodds.idlc") version "..."` from a
  registry.
