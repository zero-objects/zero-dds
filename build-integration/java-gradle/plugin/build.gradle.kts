// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A real `Plugin<Project>` — not a task-only script plugin — packaged the
// same way `com.google.protobuf` or `org.jetbrains.kotlin.jvm` are: a
// `java-gradle-plugin` module exposing a plugin id, applied by consumers
// via `plugins { id("org.zerodds.idlc") }`. Mirrors the `zerodds-build`
// (Rust, `crates/zerodds-build`) and CMake (`cmake/zerodds_idlc_generate.cmake`)
// build-step integrations for the Gradle/Java ecosystem.

plugins {
    `java-gradle-plugin`
    `kotlin-dsl`
}

repositories {
    mavenCentral()
    gradlePluginPortal()
}

group = "org.zerodds"
version = "1.0.0-rc.7"

gradlePlugin {
    plugins {
        create("zeroddsIdlc") {
            id = "org.zerodds.idlc"
            implementationClass = "org.zerodds.idlc.gradle.ZeroddsIdlcPlugin"
            displayName = "ZeroDDS IDL compiler"
            description = "Runs zerodds-idlc as a Gradle build step, wiring generated " +
                "sources into the consuming source set with proper task " +
                "inputs/outputs for incremental builds."
        }
    }
}

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(17))
    }
}
