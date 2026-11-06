// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.idlc.gradle

import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.plugins.JavaPluginExtension

/**
 * `id("org.zerodds.idlc")` — registers the `zeroddsIdlc { ... }` extension
 * and the `generateIdl` task ([GenerateIdlTask]).
 *
 * When the `java` (or `java-library`) plugin is also applied, this wires
 * `generateIdl`'s output directory into the `main` source set's `java`
 * source dirs and makes `compileJava` depend on it — the equivalent of
 * `zerodds-build`'s `OUT_DIR` + `include!` pattern for Cargo, but native
 * to Gradle's source-set model (no `include!`-style splice needed; the
 * generated `.java` files are compiled as ordinary sources).
 */
class ZeroddsIdlcPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        val extension = project.extensions.create(
            "zeroddsIdlc",
            ZeroddsIdlcExtension::class.java,
        )

        val defaultOutputDir = project.layout.buildDirectory.dir("generated/sources/zerodds-idlc")

        // `kotlin-dsl` gives an Action its argument as the receiver (SAM with receiver).
        val generateIdl = project.tasks.register("generateIdl", GenerateIdlTask::class.java) {
            idlFiles.setFrom(extension.idlFiles)
            includeDirs.setFrom(extension.includeDirs)
            backend.set(extension.backend)
            executable.set(extension.executable)
            extraArgs.set(extension.extraArgs)
            outputDir.set(extension.outputDir.orElse(defaultOutputDir))
        }

        project.plugins.withId("java") {
            val javaExtension = project.extensions.getByType(JavaPluginExtension::class.java)
            val mainSourceSet = javaExtension.sourceSets.getByName("main")
            mainSourceSet.java.srcDir(generateIdl.map { it.outputDir })

            project.tasks.named("compileJava") {
                dependsOn(generateIdl)
            }
        }
    }
}
