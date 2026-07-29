// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Sample consumer for the `org.zerodds.idlc` Gradle plugin (../plugin).
// `./gradlew run` generates `Robot.java` from `idl/Robot.idl` via
// `generateIdl`, compiles it (Gradle wires `compileJava dependsOn
// generateIdl` automatically), then runs Main, which round-trips a
// `Robot.Pose` through the generated encode/decode.

plugins {
    id("java")
    id("application")
    id("org.zerodds.idlc")
}

repositories {
    mavenCentral()
    mavenLocal() // org.zerodds:omgdds — `mvn install` crates/java-omgdds/java first, see ../../java-maven/README.md
}

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(17))
    }
}

sourceSets {
    main {
        java {
            // ZeroDDS Java runtime the generated Robot.java references but
            // the omgdds artifact does NOT ship: the org.zerodds.types.*
            // annotations (Extensibility, Key, …) and the
            // org.omg.dds.topic.TopicType<T> interface. Compile them from
            // source alongside the generated + hand-written classes.
            // (mirrors the C# sample's <Compile Include=".../idl-csharp/runtime/Omg.Types.cs">)
            srcDir("../../../crates/idl-java/runtime")
        }
    }
}

application {
    mainClass.set("com.example.Main")
}

dependencies {
    // Generated Java needs org.zerodds.cdr / org.omg.dds.topic.TopicTypeSupport
    // from crates/java-omgdds (same runtime the Maven sample needs —
    // ../../java-maven/README.md documents a further gap this alone does
    // NOT close: org.zerodds.types.{Extensibility,Key} and
    // org.omg.dds.topic.TopicType<T> don't exist anywhere in the repo).
    implementation("org.zerodds:omgdds:0.0.0")
}

zeroddsIdlc {
    idlFiles.from("idl/Robot.idl")
    backend.set("java")
}
