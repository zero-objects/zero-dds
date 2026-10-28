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

## Not checked

No Gradle is installed on the development machine or on the Linux test host; neither
the plugin nor the sample has been built. The sample's Java sources and
runtime set-up are the ones `../../java-maven` builds and runs.

## Not covered

- An `#include`d IDL file is not an input of the task.
- One backend per extension.
- Consumed through `includeBuild`; not published.
