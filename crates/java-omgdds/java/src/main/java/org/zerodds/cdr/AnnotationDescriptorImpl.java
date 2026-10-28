// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS {@link AnnotationDescriptor} (DDS-Java-PSM 1.0 §7.8.1.5): immutable,
 * {@code with*} returns a new descriptor. Two descriptors are
 * {@linkplain #equals equal} when their types are equal and they hold the same
 * attribute values (XTypes 1.3 §7.5.2.3.2).
 */
public final class AnnotationDescriptorImpl implements AnnotationDescriptor {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final LinkedHashMap<String, String> values = new LinkedHashMap<>();
    private transient DynamicType type;

    public AnnotationDescriptorImpl(ServiceEnvironment environment, DynamicType type) {
        this.environment = environment;
        this.type = type;
    }

    private AnnotationDescriptorImpl(AnnotationDescriptorImpl other) {
        this.environment = other.environment;
        this.values.putAll(other.values);
        this.type = other.type;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public String getValue(String key) {
        return values.get(key);
    }

    @Override
    public Map<String, String> getAllValue() {
        return new LinkedHashMap<>(values);
    }

    @Override
    public boolean isConsistent() {
        return type != null && !values.isEmpty();
    }

    @Override
    public DynamicType getType() {
        return type;
    }

    @Override
    public AnnotationDescriptor withValue(String key, String value) {
        AnnotationDescriptorImpl copy = new AnnotationDescriptorImpl(this);
        copy.values.put(key, value);
        return copy;
    }

    @Override
    public AnnotationDescriptor withType(DynamicType type) {
        AnnotationDescriptorImpl copy = new AnnotationDescriptorImpl(this);
        copy.type = type;
        return copy;
    }

    /** XTypes 1.3 §7.5.2.3.2: equal types and the same attribute values. */
    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof AnnotationDescriptorImpl)) {
            return false;
        }
        AnnotationDescriptorImpl other = (AnnotationDescriptorImpl) o;
        return values.equals(other.values) && java.util.Objects.equals(type, other.type);
    }

    @Override
    public int hashCode() {
        return values.hashCode();
    }

    @Override
    public String toString() {
        return "AnnotationDescriptor[" + (type == null ? "?" : type.getName()) + values + "]";
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
        out.writeObject(type);
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
        type = (DynamicType) in.readObject();
    }
}