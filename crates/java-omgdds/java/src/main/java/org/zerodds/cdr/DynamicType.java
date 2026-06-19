// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.util.Collections;
import java.util.List;

/**
 * Minimal runtime type description produced by reflecting a Java class —
 * DDS-Java-PSM 1.0 §7.8.1.3 (the result of {@code
 * DynamicTypeFactory.createType(Class<?>)}). It mirrors the structure the
 * {@link ReflectionTypeSupport} marshaller walks: a type name, an
 * {@link ExtensibilityKind}, and an ordered list of {@link Member}s.
 *
 * <p>This is a queryable type model (name + member kinds + nesting), not the
 * full OMG XTypes {@code DynamicType} API; it is sufficient to inspect an
 * IDL-less bean's structure at runtime and to drive reflective marshalling.
 */
public final class DynamicType {

    /** Member type kinds per XTypes 1.3 §8.2 Tab.8.1 + aggregates. */
    public enum Kind {
        BOOLEAN, BYTE, CHAR8, INT16, INT32, INT64, FLOAT32, FLOAT64,
        STRING, SEQUENCE, MAP, STRUCTURE
    }

    /** One member of a structured type. */
    public static final class Member {
        private final String name;
        private final Kind kind;
        private final DynamicType nestedType; // non-null for STRUCTURE.
        private final boolean key;
        private final int id;

        Member(String name, Kind kind, DynamicType nestedType, boolean key, int id) {
            this.name = name;
            this.kind = kind;
            this.nestedType = nestedType;
            this.key = key;
            this.id = id;
        }

        /** Member field name. */
        public String getName() {
            return name;
        }

        /** Member type kind. */
        public Kind getKind() {
            return kind;
        }

        /** Nested structure type (only for {@link Kind#STRUCTURE}), else null. */
        public DynamicType getNestedType() {
            return nestedType;
        }

        /** {@code true} if the member is part of the topic key. */
        public boolean isKey() {
            return key;
        }

        /** XTypes member id. */
        public int getId() {
            return id;
        }
    }

    private final String name;
    private final ExtensibilityKind extensibility;
    private final List<Member> members;

    DynamicType(String name, ExtensibilityKind extensibility, List<Member> members) {
        this.name = name;
        this.extensibility = extensibility;
        this.members = Collections.unmodifiableList(members);
    }

    /** DDS type name (convention {@code Module::Sub::Struct}). */
    public String getName() {
        return name;
    }

    /** Extensibility of the type. */
    public ExtensibilityKind getExtensibility() {
        return extensibility;
    }

    /** Ordered, immutable list of members. */
    public List<Member> getMembers() {
        return members;
    }

    /** {@code true} if any member carries the topic key. */
    public boolean isKeyed() {
        for (Member m : members) {
            if (m.isKey()) {
                return true;
            }
        }
        return false;
    }

    @Override
    public String toString() {
        return "DynamicType{" + name + ", " + extensibility + ", members=" + members.size() + "}";
    }
}
