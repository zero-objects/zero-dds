# `org.zerodds.idlc` Gradle plugin

A `Plugin<Project>` with a `generateIdl` task: declared `@InputFiles` and
`@OutputDirectory` for Gradle's up-to-date check, the output directory
added to the `main` source set, and `compileJava` depending on the task.
The task runs `zerodds-idlc generate --lang <backend> --out <outputDir>
[-I …] <file.idl>` once per IDL file.

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

The sample (`../sample-consumer`) adds `runtime/java/src/main/java`
(`org.zerodds.wire.*`) as a source directory and depends on
`org.zerodds:omgdds:1.0.0-rc.7` (`org.zerodds.cdr.*`, `mvn install` from
`crates/java-omgdds/java`). The generated classes are in the unnamed
package, so its `Main` is too. Java writes the same three file names for
every input: one IDL file per output directory.

## Checked

`../check.sh` builds `zerodds-idlc`, installs the Java PSM into the local
Maven repository, and runs `gradle run` of the sample with Gradle 8.10.2: the
plugin compiles, `generateIdl` runs before `compileJava`, and the sample's
`Main` holds the 40 octets of a `Robot.Pose` from the generated type support
to DDS-XTypes 1.3 §7.4. The merge job `merge-java-gradle` runs it when a file
here changes, the weekly job `integration-java-gradle` every week
(`ci/integration_scenarios.py`); 29.6 s on the Linux test host with warm
caches (2026-11-02). Before that run the plugin had never been built, and
its Kotlin did not compile under `kotlin-dsl` (an `Action` lambda takes its
argument as the receiver, not as `it`).

## Not covered

- An `#include`d IDL file is not an input of the task.
- One backend per extension.
- Consumed through `includeBuild`; not published.
