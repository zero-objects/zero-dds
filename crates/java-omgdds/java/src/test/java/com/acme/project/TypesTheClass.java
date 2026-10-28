// SPDX-License-Identifier: Apache-2.0
package com.acme.project;

/**
 * A topic type in a package of its own: DDS-Java-PSM 1.0 §8.8 makes the segments of the
 * package name modules of the DDS type name, {@code com::acme::project::TypesTheClass}.
 */
public final class TypesTheClass {
    public int value;

    public TypesTheClass() {
    }
}
