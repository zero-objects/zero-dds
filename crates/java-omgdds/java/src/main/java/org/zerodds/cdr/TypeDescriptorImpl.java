// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.TypeDescriptor;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * ZeroDDS {@link TypeDescriptor} (DDS-Java-PSM 1.0 §7.8.1.5).
 *
 * <p>Immutable: every {@code with*} method returns a new descriptor with one
 * property replaced and leaves the receiver as it was, so a descriptor can be
 * built up step by step before it becomes the descriptor of a
 * {@link DynamicType}. {@link #getBound()} is an unmodifiable list. Two
 * descriptors are {@linkplain #equals equal} when all properties of XTypes 1.3
 * Table 50 are equal (§7.5.2.4.6).
 */
public final class TypeDescriptorImpl implements TypeDescriptor {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private TypeKind kind;
    private String name;
    private transient DynamicType baseType;
    private transient DynamicType discriminatorType;
    private ArrayList<Integer> bound;
    private transient DynamicType elementType;
    private transient DynamicType keyElementType;

    public TypeDescriptorImpl(ServiceEnvironment environment, TypeKind kind) {
        this.environment = environment;
        this.kind = kind;
        this.bound = new ArrayList<>();
    }

    /** An alias of {@code other} with the named property replaced. */
    private TypeDescriptorImpl(TypeDescriptorImpl other) {
        this.environment = other.environment;
        this.kind = other.kind;
        this.name = other.name;
        this.baseType = other.baseType;
        this.discriminatorType = other.discriminatorType;
        this.bound = new ArrayList<>(other.bound);
        this.elementType = other.elementType;
        this.keyElementType = other.keyElementType;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    /**
     * A descriptor is consistent when it names its type, except for the
     * anonymous primitive and collection types of §7.8.1.2, which carry no name.
     */
    @Override
    public boolean isConsistent() {
        if (kind == null) {
            return false;
        }
        switch (kind) {
            case BOOLEAN_TYPE:
            case BYTE_TYPE:
            case INT_16_TYPE:
            case UINT_16_TYPE:
            case INT_32_TYPE:
            case UINT_32_TYPE:
            case INT_64_TYPE:
            case UINT_64_TYPE:
            case FLOAT_32_TYPE:
            case FLOAT_64_TYPE:
            case FLOAT_128_TYPE:
            case CHAR_8_TYPE:
            case CHAR_32_TYPE:
            case STRING_TYPE:
            case SEQUENCE_TYPE:
            case MAP_TYPE:
                return elementType != null || kind == TypeKind.STRING_TYPE;
            default:
                return name != null;
        }
    }

    @Override
    public TypeKind getKind() {
        return kind;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public DynamicType getBaseType() {
        return baseType;
    }

    @Override
    public DynamicType getDiscriminatorType() {
        return discriminatorType;
    }

    @Override
    public List<Integer> getBound() {
        return java.util.Collections.unmodifiableList(bound);
    }

    @Override
    public DynamicType getElementType() {
        return elementType;
    }

    @Override
    public DynamicType getKeyElementType() {
        return keyElementType;
    }

    @Override
    public TypeDescriptor withKind(TypeKind kind) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.kind = kind;
        return copy;
    }

    @Override
    public TypeDescriptor withName(String name) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.name = name;
        return copy;
    }

    @Override
    public TypeDescriptor withBaseType(DynamicType baseType) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.baseType = baseType;
        return copy;
    }

    @Override
    public TypeDescriptor withDiscriminatorType(DynamicType discriminatorType) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.discriminatorType = discriminatorType;
        return copy;
    }

    @Override
    public TypeDescriptor withBound(int... bound) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.bound = new ArrayList<>();
        for (int b : bound) {
            copy.bound.add(b);
        }
        return copy;
    }

    @Override
    public TypeDescriptor withElementType(DynamicType elementType) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.elementType = elementType;
        return copy;
    }

    @Override
    public TypeDescriptor withKeyElementType(DynamicType keyElementType) {
        TypeDescriptorImpl copy = new TypeDescriptorImpl(this);
        copy.keyElementType = keyElementType;
        return copy;
    }

    /** XTypes 1.3 §7.5.2.4.6: all properties of Table 50 are equal. */
    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof TypeDescriptorImpl)) {
            return false;
        }
        TypeDescriptorImpl other = (TypeDescriptorImpl) o;
        return kind == other.kind
                && java.util.Objects.equals(name, other.name)
                && java.util.Objects.equals(baseType, other.baseType)
                && java.util.Objects.equals(discriminatorType, other.discriminatorType)
                && bound.equals(other.bound)
                && java.util.Objects.equals(elementType, other.elementType)
                && java.util.Objects.equals(keyElementType, other.keyElementType);
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(kind, name, bound);
    }

    @Override
    public String toString() {
        return "TypeDescriptor[kind=" + kind + ", name=" + name + ", bound=" + Arrays.toString(
                bound.toArray()) + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3, {@code TypeDescriptor} is
     * {@link java.io.Serializable}). The referenced {@link DynamicType}s are
     * written as objects: {@link DynamicTypeImpl} is serializable, and any other
     * implementation that is not makes the write fail with
     * {@link java.io.NotSerializableException} instead of dropping the type.
     */
    private void writeObject(java.io.ObjectOutputStream out) throws java.io.IOException {
        out.defaultWriteObject();
        out.writeObject(baseType);
        out.writeObject(discriminatorType);
        out.writeObject(elementType);
        out.writeObject(keyElementType);
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
        baseType = (DynamicType) in.readObject();
        discriminatorType = (DynamicType) in.readObject();
        elementType = (DynamicType) in.readObject();
        keyElementType = (DynamicType) in.readObject();
    }
}