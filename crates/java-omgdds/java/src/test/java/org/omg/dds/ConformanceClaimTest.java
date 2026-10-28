// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 Clause 2: the conformance levels are those of DDS, plus Extensible and Dynamic
 * Types, without the Object Model Profile (DLRL). The README of the binding says which it is.
 */
class ConformanceClaimTest {

    @Test
    void theReadmeNamesTheConformanceLevelsOfClause2() throws Exception {
        Path readme = Paths.get("..", "README.md");
        assertTrue(Files.exists(readme), "README.md of the binding, from the module directory");
        String text = new String(Files.readAllBytes(readme), StandardCharsets.UTF_8);
        for (String level : new String[] {"Minimum Profile", "Content-Subscription Profile",
            "Persistence Profile", "Ownership Profile", "Extensible and Dynamic Types",
            "Object Model Profile"}) {
            assertTrue(text.contains(level), "the README names " + level);
        }
    }
}
