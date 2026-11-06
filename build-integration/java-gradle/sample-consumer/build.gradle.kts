// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Sample consumer for the `org.zerodds.idlc` Gradle plugin (../plugin).
// `gradle run` generates `Robot.java` from `idl/Robot.idl` via
// `generateIdl`, compiles it (Gradle wires `compileJava dependsOn
// generateIdl` automatically), then runs Main, which holds the octets of a
// `Robot.Pose` from the generated type support to DDS-XTypes 1.3 §7.4.
// build-integration/java-gradle/check.sh does all of it (job merge-java-gradle).

plugins {
    id("java")
    id("application")
    id("org.zerodds.idlc")
}

repositories {
    mavenCentral()
    mavenLocal() // org.zerodds:omgdds — `mvn install` crates/java-omgdds/java first, see ../../java-maven/README.md
}

// The JDK Gradle runs on compiles for Java 17: a toolchain of a fixed version is a JDK a
// runner may not have (the CI image carries one JDK).
tasks.withType<JavaCompile>().configureEach {
    options.release.set(17)
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
