// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.MemberDescriptor;

import java.util.ArrayList;
import java.util.List;

/**
 * ZeroDDS {@link MemberDescriptor} (DDS-Java-PSM 1.0 §7.8.1.5): immutable, the
 * {@code with*} methods return a new descriptor; {@link #getLabel()} is an
 * unmodifiable list. Two descriptors are {@linkplain #equals equal} when all
 * properties of XTypes 1.3 Table 53 are equal (§7.5.2.7.4).
 */
public final class MemberDescriptorImpl implements MemberDescriptor {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private String name;
    private int id;
    private transient DynamicType type;
    private String defaultValue;
    private int index;
    private ArrayList<Integer> label = new ArrayList<>();
    private boolean defaultLabel = true;

    public MemberDescriptorImpl(ServiceEnvironment environment, String name, int id, DynamicType type) {
        this.environment = environment;
        this.name = name;
        this.id = id;
        this.type = type;
    }

    private MemberDescriptorImpl(MemberDescriptorImpl other) {
        this.environment = other.environment;
        this.name = other.name;
        this.id = other.id;
        this.type = other.type;
        this.defaultValue = other.defaultValue;
        this.index = other.index;
        this.label = new ArrayList<>(other.label);
        this.defaultLabel = other.defaultLabel;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean isConsistent() {
        return name != null && id >= 0 && type != null;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public int getId() {
        return id;
    }

    @Override
    public DynamicType getType() {
        return type;
    }

    @Override
    public String getDefaultValue() {
        return defaultValue;
    }

    @Override
    public int getIndex() {
        return index;
    }

    @Override
    public List<Integer> getLabel() {
        return java.util.Collections.unmodifiableList(label);
    }

    @Override
    public boolean isDefaultLabel() {
        return defaultLabel;
    }

    @Override
    public MemberDescriptor withName(String name) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.name = name;
        return copy;
    }

    @Override
    public MemberDescriptor withId(int id) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.id = id;
        return copy;
    }

    @Override
    public MemberDescriptor withType(DynamicType type) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.type = type;
        return copy;
    }

    @Override
    public MemberDescriptor withDefaultValue(String defaultValue) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.defaultValue = defaultValue;
        return copy;
    }

    @Override
    public MemberDescriptor withIndex(int index) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.index = index;
        return copy;
    }

    @Override
    public MemberDescriptor withLabel(int... label) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.label = new ArrayList<>();
        for (int l : label) {
            copy.label.add(l);
        }
        copy.defaultLabel = false;
        return copy;
    }

    @Override
    public MemberDescriptor withDefaultLabel(boolean defaultLabel) {
        MemberDescriptorImpl copy = new MemberDescriptorImpl(this);
        copy.defaultLabel = defaultLabel;
        return copy;
    }

    /** XTypes 1.3 §7.5.2.7.4: all properties of Table 53 are equal. */
    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof MemberDescriptorImpl)) {
            return false;
        }
        MemberDescriptorImpl other = (MemberDescriptorImpl) o;
        return id == other.id
                && index == other.index
                && defaultLabel == other.defaultLabel
                && java.util.Objects.equals(name, other.name)
                && java.util.Objects.equals(type, other.type)
                && java.util.Objects.equals(defaultValue, other.defaultValue)
                && label.equals(other.label);
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(name, id, index, label);
    }

    @Override
    public String toString() {
        return "MemberDescriptor[name=" + name + ", id=" + id + "]";
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