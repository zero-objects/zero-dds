// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.idlc.gradle

import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.model.ObjectFactory
import org.gradle.api.provider.Property
import javax.inject.Inject

/**
 * The `zeroddsIdlc { ... }` project extension. Mirrors `zerodds-build`'s
 * `Config` builder (Rust, `crates/zerodds-build/src/lib.rs`) — one entry
 * per `zerodds-idlc generate` flag this plugin forwards.
 */
abstract class ZeroddsIdlcExtension @Inject constructor(objects: ObjectFactory) {
    /** `.idl` inputs (equivalent to the CLI's positional `<file.idl>`, one task run per file). */
    val idlFiles: ConfigurableFileCollection = objects.fileCollection()

    /** `--<backend>`, e.g. `"java"`, `"csharp"`, `"rust"`. Defaults to `"java"`. */
    val backend: Property<String> = objects.property(String::class.java).convention("java")

    /** `-o <dir>`. Defaults to `build/generated/sources/zerodds-idlc/<backend>`. */
    val outputDir: DirectoryProperty = objects.directoryProperty()

    /** `-I <dir>` per entry, in order. */
    val includeDirs: ConfigurableFileCollection = objects.fileCollection()

    /** Path to the `zerodds-idlc` executable. Defaults to `"zerodds-idlc"` (resolved via `PATH`). */
    val executable: Property<String> = objects.property(String::class.java).convention("zerodds-idlc")

    /** Extra raw flags forwarded verbatim, e.g. `listOf("--default-extensibility", "final")`. */
    val extraArgs: org.gradle.api.provider.ListProperty<String> =
        objects.listProperty(String::class.java).convention(emptyList())
}
