package org.zerodds.typeobject;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.typeobject.TypeObject;
import org.zerodds.cdr.DynamicDataTypeSupport;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.cdr.XcdrException;

/** The XTypes 1.1 TypeObject of a type support and of what a remote entity announced. */
public final class TypeObjects {

    private TypeObjects() {
    }

    /**
     * The PID_TYPE value for the type {@code support} encodes, or {@code null} if the support
     * has no DynamicType (a built-in type of raw octets, a registered name) or the type cannot
     * be written as a TypeObject (it contains itself, or a member maps to no TypeKind). An
     * entity then announces no {@code type}; the member is optional in the built-in topic data
     * (XTypes 1.3 7.6.3.2.2) and TypeInformation tells the peer the type.
     */
    public static byte[] announce(ServiceEnvironment env, Object support) {
        DynamicType type = dynamicTypeOf(env, support);
        if (type == null) {
            return null;
        }
        try {
            return TypeObjectCodec.encode(TypeObjectBuilder.build(type));
        } catch (IllegalArgumentException notRepresentable) {
            return null;
        }
    }

    /**
     * The DynamicType of the type {@code support} encodes, or {@code null} if it has none (a
     * built-in type of raw octets, a registered name) or its Java class maps to no TypeKind.
     */
    public static DynamicType dynamicTypeOf(ServiceEnvironment env, Object support) {
        if (support instanceof org.zerodds.internal.RegisteredNameTypeSupport) {
            return dynamicTypeOf(env, ((org.zerodds.internal.RegisteredNameTypeSupport<?>) support).unwrap());
        }
        if (support instanceof DynamicDataTypeSupport) {
            return ((DynamicDataTypeSupport) support).dynamicType();
        }
        if (support instanceof ReflectionTypeSupport) {
            try {
                return DynamicTypeFactoryImpl.getInstance(env)
                        .createType(((ReflectionTypeSupport<?>) support).javaType());
            } catch (IllegalArgumentException notRepresentable) {
                return null;
            }
        }
        return null;
    }

    /**
     * The DynamicType of a PID_TYPE value, or {@code null} if the bytes are not a TypeObject of
     * XTypes 1.1 or name no type their library holds.
     */
    public static DynamicType dynamicTypeOrNull(ServiceEnvironment env, byte[] bytes) {
        try {
            return TypeObjectToDynamic.of(env, TypeObjectCodec.decode(bytes));
        } catch (XcdrException notATypeObject) {
            return null;
        } catch (IllegalArgumentException notATypeObject) {
            return null;
        }
    }

    /**
     * The interfaces over a PID_TYPE value, or {@code null} if the bytes are not a TypeObject
     * of XTypes 1.1; the bytes stay available as {@code getTypeObjectBytes()} of the built-in
     * topic data.
     */
    public static TypeObject decodeOrNull(ServiceEnvironment env, byte[] bytes) {
        try {
            return TypeObjectModel.of(env, TypeObjectCodec.decode(bytes));
        } catch (XcdrException notATypeObject) {
            return null;
        }
    }
}
