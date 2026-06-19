// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * DDS-Java-PSM 1.0 §7.8.1.3 — the reflective {@code createType(Class<?>)}
 * factory method: "inspect the given type reflectively in accordance with the
 * Java Type Representation (Clause 8) and instantiate an equivalent DynamicType
 * object."
 *
 * <p>Builds a {@link DynamicType} from the same reflective introspection that
 * {@link ReflectionTypeSupport} uses for marshalling, so the inspected type
 * model and the wire encoding are guaranteed consistent.
 */
public final class DynamicTypeFactory {

    private static final DynamicTypeFactory INSTANCE = new DynamicTypeFactory();

    private DynamicTypeFactory() {}

    /** Process-wide singleton (mirrors the OMG factory access pattern). */
    public static DynamicTypeFactory getInstance() {
        return INSTANCE;
    }

    /**
     * Reflectively builds a {@link DynamicType} for {@code cls} per §7.8.1.3.
     *
     * @throws XcdrException if a member field's type cannot be mapped to a DDS
     *     kind.
     */
    public DynamicType createType(Class<?> cls) {
        if (cls == null) {
            throw new XcdrException("class must not be null");
        }
        ReflectionTypeSupport.TypeInfo ti = ReflectionTypeSupport.infoOf(cls);
        List<DynamicType.Member> members = new ArrayList<>(ti.members.size());
        for (ReflectionTypeSupport.MemberInfo m : ti.members) {
            DynamicType.Kind kind = kindOf(m.type);
            DynamicType nested = (kind == DynamicType.Kind.STRUCTURE)
                    ? createType(m.type)
                    : null;
            members.add(new DynamicType.Member(m.name, kind, nested, m.isKey, m.memberId));
        }
        return new DynamicType(ti.typeName, ti.extensibility, members);
    }

    private static DynamicType.Kind kindOf(Class<?> c) {
        if (c == boolean.class || c == Boolean.class) {
            return DynamicType.Kind.BOOLEAN;
        }
        if (c == byte.class || c == Byte.class) {
            return DynamicType.Kind.BYTE;
        }
        if (c == char.class || c == Character.class) {
            return DynamicType.Kind.CHAR8;
        }
        if (c == short.class || c == Short.class) {
            return DynamicType.Kind.INT16;
        }
        if (c == int.class || c == Integer.class) {
            return DynamicType.Kind.INT32;
        }
        if (c == long.class || c == Long.class) {
            return DynamicType.Kind.INT64;
        }
        if (c == float.class || c == Float.class) {
            return DynamicType.Kind.FLOAT32;
        }
        if (c == double.class || c == Double.class) {
            return DynamicType.Kind.FLOAT64;
        }
        if (c == String.class) {
            return DynamicType.Kind.STRING;
        }
        if (c.isArray() || List.class.isAssignableFrom(c)) {
            return DynamicType.Kind.SEQUENCE;
        }
        if (Map.class.isAssignableFrom(c)) {
            return DynamicType.Kind.MAP;
        }
        return DynamicType.Kind.STRUCTURE; // nested aggregate.
    }
}
