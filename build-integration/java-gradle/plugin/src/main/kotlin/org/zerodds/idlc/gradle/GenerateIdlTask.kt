// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.idlc.gradle

import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

/**
 * `generateIdl` — runs `zerodds-idlc generate <idl> --<backend> -o <outputDir>`
 * once per `.idl` file.
 *
 * `@InputFiles`/`@OutputDirectory` give Gradle the up-to-date check for
 * free: `compileJava` (which `dependsOn` this task, wired in
 * [ZeroddsIdlcPlugin]) only re-runs codegen when an `.idl` file actually
 * changed, the same incrementality guarantee `zerodds-build`'s
 * `cargo:rerun-if-changed` gives Cargo and CMake's `add_custom_command
 * DEPENDS` gives Ninja/Make.
 */
abstract class GenerateIdlTask : DefaultTask() {

    @get:Inject
    abstract val execOperations: ExecOperations

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val idlFiles: ConfigurableFileCollection

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    @get:Optional
    abstract val includeDirs: ConfigurableFileCollection

    @get:Input
    abstract val backend: Property<String>

    @get:Input
    abstract val executable: Property<String>

    @get:Input
    abstract val extraArgs: ListProperty<String>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    init {
        group = "zerodds"
        description = "Runs zerodds-idlc over the configured .idl files"
    }

    @TaskAction
    fun generate() {
        val out = outputDir.get().asFile
        out.mkdirs()

        val includeFlags = includeDirs.files.flatMap { listOf("-I", it.absolutePath) }

        idlFiles.files.forEach { idl ->
            val args = mutableListOf(
                executable.get(),
                "generate",
                idl.absolutePath,
                "--${backend.get()}",
                "-o",
                out.absolutePath,
            )
            args.addAll(includeFlags)
            args.addAll(extraArgs.get())

            logger.info("zerodds-idlc: {}", args.joinToString(" "))
            execOperations.exec {
                it.commandLine = args
            }
        }
    }
}
