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
            // The wire runtime the generated codec calls (org.zerodds.wire.*).
            srcDir("../../../runtime/java/src/main/java")
        }
    }
}

application {
    // The generated classes are in the unnamed package, so Main is too.
    mainClass.set("Main")
}

dependencies {
    // org.zerodds.cdr.* for the generated TypeSupport classes, from
    // crates/java-omgdds/java (`mvn install` it first).
    implementation("org.zerodds:omgdds:1.0.0-rc.7")
}

zeroddsIdlc {
    idlFiles.from("idl/Robot.idl")
    backend.set("java")
    // Main.java reads the fields: the OMG IDL-to-Java mapping (the default is psm, with accessors)
    extraArgs.set(listOf("--java-mapping", "omg"))
}
