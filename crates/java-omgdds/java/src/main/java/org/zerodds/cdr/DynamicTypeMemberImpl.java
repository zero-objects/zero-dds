// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;

import java.util.ArrayList;
import java.util.List;

/**
 * ZeroDDS {@link DynamicTypeMember} (DDS-Java-PSM 1.0 §7.8.1.3): one member of
 * a {@link DynamicTypeImpl} together with its annotations.
 *
 * <p>{@link #getAnnotations()} is an unmodifiable list. Two members are
 * {@linkplain #equals equal} when they belong to equal types and their
 * descriptors and annotations are equal (XTypes 1.3 §7.5.2.6.3); the PSM has no
 * {@code clone} for a member (it is a reference type obtained from
 * {@link DynamicTypeImpl#addMember}).
 */
public final class DynamicTypeMemberImpl implements DynamicTypeMember, java.io.Serializable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final MemberDescriptor descriptor;
    private final ArrayList<AnnotationDescriptor> annotations = new ArrayList<>();
    /** The descriptor of the type this member belongs to, for the equality of §7.5.2.6.3. */
    private org.omg.dds.type.dynamic.TypeDescriptor ownerDescriptor;

    public DynamicTypeMemberImpl(ServiceEnvironment environment, MemberDescriptor descriptor) {
        this.environment = environment;
        this.descriptor = descriptor;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public MemberDescriptor getDescriptor() {
        return descriptor;
    }

    @Override
    public List<AnnotationDescriptor> getAnnotations() {
        return java.util.Collections.unmodifiableList(new ArrayList<>(annotations));
    }

    /** Records the type this member was added to. */
    void belongsTo(org.omg.dds.type.dynamic.TypeDescriptor owner) {
        this.ownerDescriptor = owner;
    }

    @Override
    public void setAnnotation(AnnotationDescriptor descriptor) {
        annotations.clear();
        annotations.add(descriptor);
    }

    @Override
    public void setAnnotations(AnnotationDescriptor... descriptor) {
        annotations.clear();
        for (AnnotationDescriptor d : descriptor) {
            annotations.add(d);
        }
    }

    @Override
    public int getId() {
        return descriptor.getId();
    }

    @Override
    public String getName() {
        return descriptor.getName();
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof DynamicTypeMemberImpl)) {
            return false;
        }
        DynamicTypeMemberImpl other = (DynamicTypeMemberImpl) o;
        return descriptor.equals(other.descriptor)
                && annotations.equals(other.annotations)
                && java.util.Objects.equals(ownerDescriptor, other.ownerDescriptor);
    }

    @Override
    public int hashCode() {
        return descriptor.hashCode() * 31 + annotations.size();
    }

    @Override
    public String toString() {
        return "DynamicTypeMember[" + descriptor.getName() + ", annotations=" + annotations.size() + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}; a deserialized object belongs to the default
     * environment of this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = org.zerodds.ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}