// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.omg.dds.type.dynamic.TypeDescriptor;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * ZeroDDS {@link DynamicType} (DDS-Java-PSM 1.0 §7.8.1.5).
 *
 * <p>A descriptor plus its members and annotations. {@link #addMember} grows
 * the type in place; {@link #clone()} returns an independent copy, as
 * {@link DynamicType#clone()} requires (§7.8.1.3 maps {@code equals} and
 * {@code clone} to {@code Object.equals} and {@code Object.clone}). Two types are
 * {@linkplain #equals equal} when their descriptors, members and annotations are
 * equal (XTypes 1.3 §7.5.2.8.4, Table 54). The maps and lists that give out
 * annotations are unmodifiable (§7.8.1.3).
 */
public final class DynamicTypeImpl implements DynamicType, java.io.Serializable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private TypeDescriptor descriptor;
    private final LinkedHashMap<Integer, DynamicTypeMember> members = new LinkedHashMap<>();
    private final LinkedHashMap<Integer, AnnotationDescriptor> annotations = new LinkedHashMap<>();

    public DynamicTypeImpl(ServiceEnvironment environment, TypeDescriptor descriptor) {
        this.environment = environment;
        this.descriptor = descriptor;
    }

    private DynamicTypeImpl(DynamicTypeImpl other) {
        this.environment = other.environment;
        this.descriptor = other.descriptor;
        for (Map.Entry<Integer, DynamicTypeMember> e : other.members.entrySet()) {
            DynamicTypeMemberImpl member = (DynamicTypeMemberImpl) e.getValue();
            DynamicTypeMemberImpl copy =
                    new DynamicTypeMemberImpl(environment, member.getDescriptor());
            copy.setAnnotations(member.getAnnotations().toArray(new AnnotationDescriptor[0]));
            copy.belongsTo(this.descriptor);
            this.members.put(e.getKey(), copy);
        }
        this.annotations.putAll(other.annotations);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public TypeDescriptor getDescriptor() {
        return descriptor;
    }

    @Override
    public String getName() {
        return descriptor.getName();
    }

    @Override
    public TypeKind getKind() {
        return descriptor.getKind();
    }

    @Override
    public DynamicTypeMember getMember(int id) {
        return members.get(id);
    }

    @Override
    public DynamicTypeMember getMember(String name) {
        for (DynamicTypeMember member : members.values()) {
            if (member.getName().equals(name)) {
                return member;
            }
        }
        return null;
    }

    @Override
    public Map<Integer, DynamicTypeMember> getAllMembers() {
        return new LinkedHashMap<>(members);
    }

    @Override
    public Map<Integer, DynamicTypeMember> getAllMembers(int... id) {
        Map<Integer, DynamicTypeMember> out = new LinkedHashMap<>();
        for (int i : id) {
            DynamicTypeMember member = members.get(i);
            if (member != null) {
                out.put(i, member);
            }
        }
        return out;
    }

    @Override
    public Map<String, DynamicTypeMember> getAllMembers(String... id) {
        Map<String, DynamicTypeMember> out = new LinkedHashMap<>();
        for (String name : id) {
            DynamicTypeMember member = getMember(name);
            if (member != null) {
                out.put(name, member);
            }
        }
        return out;
    }

    @Override
    public Map<Integer, AnnotationDescriptor> getAllAnnotations() {
        return java.util.Collections.unmodifiableMap(new LinkedHashMap<>(annotations));
    }

    @Override
    public AnnotationDescriptor getAnnotation(int id) {
        return annotations.get(id);
    }

    @Override
    public AnnotationDescriptor getAnnotation(String name) {
        for (Map.Entry<Integer, AnnotationDescriptor> e : annotations.entrySet()) {
            if (name.equals(e.getValue().getType().getName())) {
                return e.getValue();
            }
        }
        return null;
    }

    @Override
    public Map<Integer, AnnotationDescriptor> getAnnotations(int... id) {
        Map<Integer, AnnotationDescriptor> out = new LinkedHashMap<>();
        for (int i : id) {
            AnnotationDescriptor a = annotations.get(i);
            if (a != null) {
                out.put(i, a);
            }
        }
        return java.util.Collections.unmodifiableMap(out);
    }

    @Override
    public Map<String, AnnotationDescriptor> getAnnotations(String... name) {
        Map<String, AnnotationDescriptor> out = new LinkedHashMap<>();
        for (String n : name) {
            AnnotationDescriptor a = getAnnotation(n);
            if (a != null) {
                out.put(n, a);
            }
        }
        return java.util.Collections.unmodifiableMap(out);
    }

    @Override
    public void setAnnotation(AnnotationDescriptor descriptor) {
        annotations.clear();
        annotations.put(0, descriptor);
    }

    @Override
    public void setAnnotations(AnnotationDescriptor... descriptors) {
        annotations.clear();
        for (int i = 0; i < descriptors.length; i++) {
            annotations.put(i, descriptors[i]);
        }
    }

    @Override
    public DynamicTypeMember addMember(MemberDescriptor descriptor) {
        DynamicTypeMemberImpl member = new DynamicTypeMemberImpl(environment, descriptor);
        member.belongsTo(this.descriptor);
        members.put(descriptor.getId(), member);
        return member;
    }

    @Override
    public List<DynamicTypeMember> addMembers(MemberDescriptor... descriptors) {
        List<DynamicTypeMember> added = new ArrayList<>();
        for (MemberDescriptor d : descriptors) {
            added.add(addMember(d));
        }
        return added;
    }

    @Override
    public DynamicType clone() {
        return new DynamicTypeImpl(this);
    }

    /** XTypes 1.3 §7.5.2.8.4: equal descriptors, members and annotations. */
    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof DynamicTypeImpl)) {
            return false;
        }
        DynamicTypeImpl other = (DynamicTypeImpl) o;
        return descriptor.equals(other.descriptor)
                && members.equals(other.members)
                && new ArrayList<>(annotations.values())
                        .equals(new ArrayList<>(other.annotations.values()));
    }

    @Override
    public int hashCode() {
        return descriptor.hashCode() * 31 + members.keySet().hashCode();
    }

    @Override
    public String toString() {
        return "DynamicType[" + getName() + ", " + getKind() + ", members="
                + Arrays.toString(members.keySet().toArray()) + "]";
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