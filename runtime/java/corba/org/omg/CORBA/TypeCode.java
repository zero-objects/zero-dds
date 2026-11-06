// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.CORBA;

/**
 * The IDL {@code TypeCode} on the CORBA platform: IDL4 to Java Language
 * Mapping 1.0 Annex A.1.2, a public class {@code org.omg.CORBA.TypeCode}
 * with the nested exceptions {@link Bounds} and {@link BadKind} and the
 * operations the clause lists, whose behaviour is that of CORBA 3.3 part 1
 * §8.11.1.
 *
 * <p>The types the operations use besides {@link Any} and this class are
 * the Java of the CORBA module under the IDL naming scheme, as A.1.2
 * requires: {@link TCKind}, and {@code short} for {@code Visibility} and
 * {@code ValueModifier}, which are {@code typedef short} (§7.2.4.6).
 *
 * <p>A type code is made by the static {@code create_*_tc} operations,
 * which are the type code creation operations of the ORB (CORBA 3.3 part 1
 * §8.11.3) offered without an ORB, and by {@link #get_primitive_tc}.
 */
public class TypeCode implements java.io.Serializable {
    private static final long serialVersionUID = 1L;

    /** Raised for an index at or beyond the number of members. */
    public static class Bounds extends UserException {
        private static final long serialVersionUID = 1L;
    }

    /** Raised for an operation the kind of the type code does not have. */
    public static class BadKind extends UserException {
        private static final long serialVersionUID = 1L;
    }

    private TCKind kind;
    private String id = "";
    private String name = "";
    private String[] memberNames = new String[0];
    private TypeCode[] memberTypes = new TypeCode[0];
    private Any[] memberLabels = new Any[0];
    private short[] memberVisibilities = new short[0];
    private TypeCode discriminator;
    private int defaultIndex = -1;
    private int length;
    private TypeCode content;
    private short digits;
    private short scale;
    private short modifier;
    private TypeCode concreteBase;
    /** For a recursive placeholder: the id it stands for, and what it was bound to. */
    private String recursiveId;
    private TypeCode target;

    /** The type code of {@code tk_null}. */
    public TypeCode() {
        this(TCKind.tk_null);
    }

    private TypeCode(TCKind kind) {
        this.kind = kind;
    }

    // -- the operations of A.1.2 --------------------------------------------

    /**
     * Whether the set of legal operations is the same and every one of them
     * gives the same result (CORBA 3.3 part 1 §8.11.1).
     */
    public boolean equal(TypeCode tc) {
        return compare(this, tc, false, new java.util.HashSet<Pair>());
    }

    /** Type equivalence for values in an {@code any} (CORBA 3.3 part 1 §8.11.1). */
    public boolean equivalent(TypeCode tc) {
        return compare(this, tc, true, new java.util.HashSet<Pair>());
    }

    /** This type code without its optional name and member name fields. */
    public TypeCode get_compact_typecode() {
        return compact(this, new java.util.IdentityHashMap<TypeCode, TypeCode>());
    }

    public TCKind kind() {
        return definition().kind;
    }

    public String id() throws BadKind {
        TypeCode d = definition();
        d.require(ID_KINDS);
        return d.id;
    }

    public String name() throws BadKind {
        TypeCode d = definition();
        d.require(ID_KINDS);
        return d.name;
    }

    public int member_count() throws BadKind {
        TypeCode d = definition();
        d.require(MEMBER_KINDS);
        return d.memberNames.length;
    }

    public String member_name(int index) throws BadKind, Bounds {
        TypeCode d = definition();
        d.require(MEMBER_KINDS);
        return d.memberNames[d.within(index)];
    }

    public TypeCode member_type(int index) throws BadKind, Bounds {
        TypeCode d = definition();
        d.require(MEMBER_TYPE_KINDS);
        return d.memberTypes[d.within(index)];
    }

    public Any member_label(int index) throws BadKind, Bounds {
        TypeCode d = definition();
        d.require(UNION_KINDS);
        return d.memberLabels[d.within(index)];
    }

    public TypeCode discriminator_type() throws BadKind {
        TypeCode d = definition();
        d.require(UNION_KINDS);
        return d.discriminator;
    }

    public int default_index() throws BadKind {
        TypeCode d = definition();
        d.require(UNION_KINDS);
        return d.defaultIndex;
    }

    public int length() throws BadKind {
        TypeCode d = definition();
        d.require(LENGTH_KINDS);
        return d.length;
    }

    public TypeCode content_type() throws BadKind {
        TypeCode d = definition();
        d.require(CONTENT_KINDS);
        return d.content;
    }

    public short fixed_digits() throws BadKind {
        TypeCode d = definition();
        d.require(FIXED_KINDS);
        return d.digits;
    }

    public short fixed_scale() throws BadKind {
        TypeCode d = definition();
        d.require(FIXED_KINDS);
        return d.scale;
    }

    public short member_visibility(int index) throws BadKind, Bounds {
        TypeCode d = definition();
        d.require(VALUE_KINDS);
        return d.memberVisibilities[d.within(index)];
    }

    public short type_modifier() throws BadKind {
        TypeCode d = definition();
        d.require(VALUE_KINDS);
        return d.modifier;
    }

    public TypeCode concrete_base_type() throws BadKind {
        TypeCode d = definition();
        d.require(VALUE_KINDS);
        return d.concreteBase;
    }

    // -- creating type codes, CORBA 3.3 part 1 §8.11.3 -----------------------

    /**
     * The type code of a kind without parameters: {@code tk_null} to
     * {@code tk_octet}, {@code tk_any}, {@code tk_TypeCode},
     * {@code tk_Principal}, {@code tk_longlong}, {@code tk_ulonglong},
     * {@code tk_longdouble}, {@code tk_wchar}; {@code tk_string} and
     * {@code tk_wstring} unbounded; {@code tk_objref} for {@code Object}.
     *
     * @throws BAD_PARAM for a kind that has parameters
     */
    public static TypeCode get_primitive_tc(TCKind kind) {
        switch (kind) {
            case tk_null:
            case tk_void:
            case tk_short:
            case tk_long:
            case tk_ushort:
            case tk_ulong:
            case tk_float:
            case tk_double:
            case tk_boolean:
            case tk_char:
            case tk_octet:
            case tk_any:
            case tk_TypeCode:
            case tk_Principal:
            case tk_longlong:
            case tk_ulonglong:
            case tk_longdouble:
            case tk_wchar:
                return new TypeCode(kind);
            case tk_string:
                return create_string_tc(0);
            case tk_wstring:
                return create_wstring_tc(0);
            case tk_objref:
                return create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object");
            default:
                throw new BAD_PARAM(0, completion_status.COMPLETED_NO);
        }
    }

    public static TypeCode create_struct_tc(
            String id, String name, java.util.List<StructMember> members) {
        return aggregate(TCKind.tk_struct, id, name, members);
    }

    public static TypeCode create_exception_tc(
            String id, String name, java.util.List<StructMember> members) {
        return aggregate(TCKind.tk_except, id, name, members);
    }

    public static TypeCode create_union_tc(
            String id, String name, TypeCode discriminator_type,
            java.util.List<UnionMember> members) {
        TypeCode tc = named(TCKind.tk_union, id, name);
        tc.discriminator = discriminator_type;
        int count = members.size();
        tc.memberNames = new String[count];
        tc.memberTypes = new TypeCode[count];
        tc.memberLabels = new Any[count];
        for (int i = 0; i < count; i++) {
            UnionMember member = members.get(i);
            tc.memberNames[i] = member.get_name();
            tc.memberTypes[i] = member.get_type();
            tc.memberLabels[i] = member.get_label();
            // §8.11.1: "For the default member, the label is the zero octet."
            if (isDefaultLabel(member.get_label())) {
                tc.defaultIndex = i;
            }
        }
        return bindRecursive(tc);
    }

    public static TypeCode create_enum_tc(
            String id, String name, java.util.List<String> members) {
        TypeCode tc = named(TCKind.tk_enum, id, name);
        tc.memberNames = members.toArray(new String[0]);
        return tc;
    }

    public static TypeCode create_alias_tc(String id, String name, TypeCode original_type) {
        TypeCode tc = named(TCKind.tk_alias, id, name);
        tc.content = original_type;
        return tc;
    }

    public static TypeCode create_interface_tc(String id, String name) {
        return named(TCKind.tk_objref, id, name);
    }

    public static TypeCode create_string_tc(int bound) {
        TypeCode tc = new TypeCode(TCKind.tk_string);
        tc.length = bound;
        return tc;
    }

    public static TypeCode create_wstring_tc(int bound) {
        TypeCode tc = new TypeCode(TCKind.tk_wstring);
        tc.length = bound;
        return tc;
    }

    public static TypeCode create_fixed_tc(short digits, short scale) {
        TypeCode tc = new TypeCode(TCKind.tk_fixed);
        tc.digits = digits;
        tc.scale = scale;
        return tc;
    }

    public static TypeCode create_sequence_tc(int bound, TypeCode element_type) {
        TypeCode tc = new TypeCode(TCKind.tk_sequence);
        tc.length = bound;
        tc.content = element_type;
        return tc;
    }

    public static TypeCode create_array_tc(int length, TypeCode element_type) {
        TypeCode tc = new TypeCode(TCKind.tk_array);
        tc.length = length;
        tc.content = element_type;
        return tc;
    }

    public static TypeCode create_value_tc(
            String id, String name, short type_modifier, TypeCode concrete_base,
            java.util.List<ValueMember> members) {
        return value(TCKind.tk_value, id, name, type_modifier, concrete_base, members);
    }

    public static TypeCode create_event_tc(
            String id, String name, short type_modifier, TypeCode concrete_base,
            java.util.List<ValueMember> members) {
        return value(TCKind.tk_event, id, name, type_modifier, concrete_base, members);
    }

    public static TypeCode create_value_box_tc(String id, String name, TypeCode boxed_type) {
        TypeCode tc = named(TCKind.tk_value_box, id, name);
        tc.content = boxed_type;
        return bindRecursive(tc);
    }

    public static TypeCode create_native_tc(String id, String name) {
        return named(TCKind.tk_native, id, name);
    }

    public static TypeCode create_abstract_interface_tc(String id, String name) {
        return named(TCKind.tk_abstract_interface, id, name);
    }

    public static TypeCode create_local_interface_tc(String id, String name) {
        return named(TCKind.tk_local_interface, id, name);
    }

    public static TypeCode create_component_tc(String id, String name) {
        return named(TCKind.tk_component, id, name);
    }

    public static TypeCode create_home_tc(String id, String name) {
        return named(TCKind.tk_home, id, name);
    }

    /**
     * A placeholder for the type code of the structure, union, value type,
     * event type or boxed value type with repository id {@code id} that
     * contains it; creating that type code binds it (CORBA 3.3 part 1
     * §8.11.3). Until then every operation on it raises
     * {@code BAD_TYPECODE}.
     */
    public static TypeCode create_recursive_tc(String id) {
        TypeCode tc = new TypeCode();
        tc.recursiveId = id;
        return tc;
    }

    // -- the rest -------------------------------------------------------------

    private static final java.util.EnumSet<TCKind> ID_KINDS = java.util.EnumSet.of(
        TCKind.tk_objref, TCKind.tk_struct, TCKind.tk_union, TCKind.tk_enum, TCKind.tk_alias,
        TCKind.tk_value, TCKind.tk_value_box, TCKind.tk_native, TCKind.tk_abstract_interface,
        TCKind.tk_local_interface, TCKind.tk_except, TCKind.tk_component, TCKind.tk_home,
        TCKind.tk_event);
    private static final java.util.EnumSet<TCKind> MEMBER_KINDS = java.util.EnumSet.of(
        TCKind.tk_struct, TCKind.tk_union, TCKind.tk_enum, TCKind.tk_value, TCKind.tk_event,
        TCKind.tk_except);
    private static final java.util.EnumSet<TCKind> MEMBER_TYPE_KINDS = java.util.EnumSet.of(
        TCKind.tk_struct, TCKind.tk_union, TCKind.tk_value, TCKind.tk_event, TCKind.tk_except);
    private static final java.util.EnumSet<TCKind> UNION_KINDS = java.util.EnumSet.of(TCKind.tk_union);
    private static final java.util.EnumSet<TCKind> LENGTH_KINDS = java.util.EnumSet.of(
        TCKind.tk_string, TCKind.tk_wstring, TCKind.tk_sequence, TCKind.tk_array);
    private static final java.util.EnumSet<TCKind> CONTENT_KINDS = java.util.EnumSet.of(
        TCKind.tk_sequence, TCKind.tk_array, TCKind.tk_value_box, TCKind.tk_alias);
    private static final java.util.EnumSet<TCKind> FIXED_KINDS = java.util.EnumSet.of(TCKind.tk_fixed);
    private static final java.util.EnumSet<TCKind> VALUE_KINDS = java.util.EnumSet.of(
        TCKind.tk_value, TCKind.tk_event);

    /** The type code itself, or what a bound placeholder stands for. */
    private TypeCode definition() {
        TypeCode tc = this;
        while (tc.recursiveId != null) {
            if (tc.target == null) {
                throw new BAD_TYPECODE(0, completion_status.COMPLETED_NO);
            }
            tc = tc.target;
        }
        return tc;
    }

    private void require(java.util.EnumSet<TCKind> kinds) {
        if (!kinds.contains(kind)) {
            throw new BadKind();
        }
    }

    private int within(int index) {
        if (index < 0 || index >= memberNames.length) {
            throw new Bounds();
        }
        return index;
    }

    private static TypeCode named(TCKind kind, String id, String name) {
        TypeCode tc = new TypeCode(kind);
        tc.id = id == null ? "" : id;
        tc.name = name == null ? "" : name;
        return tc;
    }

    private static TypeCode aggregate(
            TCKind kind, String id, String name, java.util.List<StructMember> members) {
        TypeCode tc = named(kind, id, name);
        int count = members.size();
        tc.memberNames = new String[count];
        tc.memberTypes = new TypeCode[count];
        for (int i = 0; i < count; i++) {
            tc.memberNames[i] = members.get(i).get_name();
            tc.memberTypes[i] = members.get(i).get_type();
        }
        return bindRecursive(tc);
    }

    private static TypeCode value(
            TCKind kind, String id, String name, short modifier, TypeCode base,
            java.util.List<ValueMember> members) {
        TypeCode tc = named(kind, id, name);
        tc.modifier = modifier;
        tc.concreteBase = base;
        int count = members.size();
        tc.memberNames = new String[count];
        tc.memberTypes = new TypeCode[count];
        tc.memberVisibilities = new short[count];
        for (int i = 0; i < count; i++) {
            tc.memberNames[i] = members.get(i).get_name();
            tc.memberTypes[i] = members.get(i).get_type();
            tc.memberVisibilities[i] = members.get(i).get_access();
        }
        return bindRecursive(tc);
    }

    private static boolean isDefaultLabel(Any label) {
        return label != null
            && label.type().kind() == TCKind.tk_octet
            && label.extract_octet() == 0;
    }

    /** Binds every unbound placeholder for {@code tc}'s id that {@code tc} contains. */
    private static TypeCode bindRecursive(TypeCode tc) {
        if (!tc.id.isEmpty()) {
            bind(tc, tc.id, tc, java.util.Collections.newSetFromMap(
                new java.util.IdentityHashMap<TypeCode, Boolean>()));
        }
        return tc;
    }

    private static void bind(TypeCode at, String id, TypeCode to, java.util.Set<TypeCode> seen) {
        if (at == null || !seen.add(at)) {
            return;
        }
        if (at.recursiveId != null) {
            if (at.target == null && at.recursiveId.equals(id)) {
                at.target = to;
            }
            return;
        }
        for (TypeCode member : at.memberTypes) {
            bind(member, id, to, seen);
        }
        bind(at.content, id, to, seen);
        bind(at.discriminator, id, to, seen);
        bind(at.concreteBase, id, to, seen);
    }

    /** Two type codes compared, once per pair: a pair met again on a cycle holds. */
    private static final class Pair {
        private final TypeCode left;
        private final TypeCode right;

        Pair(TypeCode left, TypeCode right) {
            this.left = left;
            this.right = right;
        }

        @Override
        public boolean equals(java.lang.Object other) {
            return other instanceof Pair
                && ((Pair) other).left == left
                && ((Pair) other).right == right;
        }

        @Override
        public int hashCode() {
            return System.identityHashCode(left) * 31 + System.identityHashCode(right);
        }
    }

    private static boolean compare(
            TypeCode a, TypeCode b, boolean equivalence, java.util.Set<Pair> open) {
        if (a == null || b == null) {
            return a == b;
        }
        a = a.definition();
        b = b.definition();
        if (equivalence) {
            // "recursively replace the TypeCode with the result of calling
            // content_type, until the kind is no longer tk_alias"
            while (a.kind == TCKind.tk_alias) {
                a = a.content.definition();
            }
            while (b.kind == TCKind.tk_alias) {
                b = b.content.definition();
            }
        }
        if (a == b || !open.add(new Pair(a, b))) {
            return true;
        }
        if (a.kind != b.kind) {
            return false;
        }
        boolean hasId = ID_KINDS.contains(a.kind);
        if (equivalence && hasId && !a.id.isEmpty() && !b.id.isEmpty()) {
            return a.id.equals(b.id);
        }
        if (!equivalence && hasId && (!a.id.equals(b.id) || !a.name.equals(b.name))) {
            return false;
        }
        if (MEMBER_KINDS.contains(a.kind)) {
            if (a.memberNames.length != b.memberNames.length) {
                return false;
            }
            if (!equivalence && !java.util.Arrays.equals(a.memberNames, b.memberNames)) {
                return false;
            }
        }
        if (MEMBER_TYPE_KINDS.contains(a.kind)) {
            for (int i = 0; i < a.memberTypes.length; i++) {
                if (!compare(a.memberTypes[i], b.memberTypes[i], equivalence, open)) {
                    return false;
                }
            }
        }
        if (UNION_KINDS.contains(a.kind)) {
            if (a.defaultIndex != b.defaultIndex
                    || !compare(a.discriminator, b.discriminator, equivalence, open)) {
                return false;
            }
            for (int i = 0; i < a.memberLabels.length; i++) {
                if (!a.memberLabels[i].equal(b.memberLabels[i])) {
                    return false;
                }
            }
        }
        if (LENGTH_KINDS.contains(a.kind) && a.length != b.length) {
            return false;
        }
        if (CONTENT_KINDS.contains(a.kind) && !compare(a.content, b.content, equivalence, open)) {
            return false;
        }
        if (FIXED_KINDS.contains(a.kind) && (a.digits != b.digits || a.scale != b.scale)) {
            return false;
        }
        if (VALUE_KINDS.contains(a.kind)) {
            if (a.modifier != b.modifier
                    || !java.util.Arrays.equals(a.memberVisibilities, b.memberVisibilities)
                    || !compare(a.concreteBase, b.concreteBase, equivalence, open)) {
                return false;
            }
        }
        return true;
    }

    private static TypeCode compact(TypeCode tc, java.util.Map<TypeCode, TypeCode> done) {
        if (tc == null) {
            return null;
        }
        TypeCode known = done.get(tc);
        if (known != null) {
            return known;
        }
        TypeCode out = new TypeCode(tc.kind);
        done.put(tc, out);
        if (tc.recursiveId != null) {
            out.recursiveId = tc.recursiveId;
            out.target = compact(tc.target, done);
            return out;
        }
        out.id = tc.id;
        // "strips out all optional name and member name fields"
        out.name = "";
        out.memberNames = new String[tc.memberNames.length];
        java.util.Arrays.fill(out.memberNames, "");
        out.memberTypes = new TypeCode[tc.memberTypes.length];
        for (int i = 0; i < tc.memberTypes.length; i++) {
            out.memberTypes[i] = compact(tc.memberTypes[i], done);
        }
        out.memberLabels = tc.memberLabels.clone();
        out.memberVisibilities = tc.memberVisibilities.clone();
        out.discriminator = compact(tc.discriminator, done);
        out.defaultIndex = tc.defaultIndex;
        out.length = tc.length;
        out.content = compact(tc.content, done);
        out.digits = tc.digits;
        out.scale = tc.scale;
        out.modifier = tc.modifier;
        out.concreteBase = compact(tc.concreteBase, done);
        return out;
    }
}
