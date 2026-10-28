package org.zerodds.cdr;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;

/**
 * The is-assignable-from relationship of DDS-XTypes 1.3 7.2.4.4 between two
 * {@link DynamicType}s, and the declared relationship of
 * {@code DECLARED_TYPE_CONSISTENCY}.
 *
 * <p>{@code t1.assignableFrom(t2)} reads: an object of {@code t2} (what a writer sends) can
 * construct an object of {@code t1} (what a reader expects). The rules are those of Tables 14
 * to 19: aliases are resolved, primitives must be the same kind, strings of narrow and wide
 * characters are not mixed, array bounds are equal, sequence, map and string bounds may differ
 * (the per-object length check is done by the codec), enumerations agree on names and values,
 * structures agree on member ids and names, on key, must-understand and optional members, and
 * for appendable and final types on the members at the same position. Where the standard speaks
 * of the member type being "strongly assignable", {@link #strongly} applies (assignable, and the
 * serialized form of the sender is delimited, 7.2.4.2).
 *
 * <p>Not evaluated: the TryConstruct behaviors (they decide per object, not per type),
 * {@code @ignore_literal_names}, and the per-label rules of unions beyond the checks on
 * discriminator, member ids and names, member types of equal ids, and the label sets.
 */
public final class TypeAssignability {

    private TypeAssignability() {
    }

    /** Whether an object of {@code writer} can construct an object of {@code reader} (7.2.4.4). */
    public static boolean assignableFrom(DynamicType reader, DynamicType writer) {
        DynamicType t1 = resolve(reader);
        DynamicType t2 = resolve(writer);
        TypeKind k1 = t1.getKind();
        if (k1 != t2.getKind()) {
            return false;
        }
        if (isPrimitive(k1)) {
            // int8 and uint8 of XTypes 1.3, which the PSM's TypeKind has no kind for, are
            // BYTE_TYPE under their own name (org.zerodds.typelookup.CompleteTypes) and are
            // not an octet.
            return k1 != TypeKind.BYTE_TYPE || octetKind(t1).equals(octetKind(t2));
        }
        switch (k1) {
            case STRING_TYPE:
                return wide(t1) == wide(t2);
            case ARRAY_TYPE:
                return t1.getDescriptor().getBound().equals(t2.getDescriptor().getBound())
                        && strongly(element(t1), element(t2));
            case SEQUENCE_TYPE:
                return strongly(element(t1), element(t2));
            case MAP_TYPE:
                return strongly(t1.getDescriptor().getKeyElementType(),
                        t2.getDescriptor().getKeyElementType())
                        && strongly(element(t1), element(t2));
            case BITSET_TYPE:
                // A bitmask is a BITSET_TYPE named "Bitmask" (CompleteTypes); it is no bit set.
                return "Bitmask".equals(t1.getName()) == "Bitmask".equals(t2.getName())
                        && t1.getDescriptor().getBound().equals(t2.getDescriptor().getBound());
            case ENUMERATION_TYPE:
                return enumeration(t1, t2);
            case STRUCTURE_TYPE:
                return structure(t1, t2);
            case UNION_TYPE:
                return union(t1, t2);
            default:
                return t1.getName() != null && t1.getName().equals(t2.getName());
        }
    }

    /** Strong assignability (7.2.4.3): assignable, and the sender's type is delimited. */
    public static boolean strongly(DynamicType reader, DynamicType writer) {
        return assignableFrom(reader, writer) && delimited(resolve(writer));
    }

    /**
     * {@code DECLARED_TYPE_CONSISTENCY}: the writer's type is the reader's type, or declares the
     * reader's type as an ancestor of its base type chain (Java: a subclass).
     */
    public static boolean declared(DynamicType reader, DynamicType writer) {
        String wanted = reader.getName();
        for (DynamicType t = writer; t != null; t = t.getDescriptor().getBaseType()) {
            if (wanted != null && wanted.equals(t.getName())) {
                return true;
            }
        }
        return false;
    }

    // ---- type helpers ---------------------------------------------------------------------

    private static DynamicType resolve(DynamicType type) {
        DynamicType t = type;
        while (t.getKind() == TypeKind.ALIAS_TYPE && t.getDescriptor().getBaseType() != null) {
            t = t.getDescriptor().getBaseType();
        }
        return t;
    }

    private static String octetKind(DynamicType type) {
        String name = type.getName();
        return "Int8".equals(name) || "UInt8".equals(name) ? name : "Byte";
    }

    private static boolean isPrimitive(TypeKind kind) {
        return kind.ordinal() >= TypeKind.BOOLEAN_TYPE.ordinal()
                && kind.ordinal() <= TypeKind.CHAR_32_TYPE.ordinal();
    }

    private static DynamicType element(DynamicType type) {
        return type.getDescriptor().getElementType();
    }

    /**
     * Whether a string type is one of wide characters: its element type is CHAR_32_TYPE, or it
     * was made by {@code createWStringType}, which names it "WString".
     */
    public static boolean isWide(DynamicType string) {
        DynamicType e = element(string);
        return (e != null && e.getKind() == TypeKind.CHAR_32_TYPE)
                || "WString".equals(string.getName());
    }

    private static boolean wide(DynamicType string) {
        return isWide(string);
    }

    private static String extensibility(DynamicType type) {
        AnnotationDescriptor a = type.getAnnotation("Extensibility");
        String v = a == null ? null : a.getValue("value");
        if (v == null) {
            return "APPENDABLE";
        }
        if (v.startsWith("FINAL")) {
            return "FINAL";
        }
        if (v.startsWith("MUTABLE")) {
            return "MUTABLE";
        }
        return "APPENDABLE";
    }

    /** 7.2.4.2: whether the serialized form of {@code type} carries its own length. */
    static boolean delimited(DynamicType type) {
        DynamicType t = resolve(type);
        TypeKind k = t.getKind();
        if (isPrimitive(k) || k == TypeKind.STRING_TYPE || k == TypeKind.ENUMERATION_TYPE
                || k == TypeKind.BITSET_TYPE) {
            return true;
        }
        if (k == TypeKind.SEQUENCE_TYPE || k == TypeKind.ARRAY_TYPE) {
            return delimited(element(t));
        }
        if (k == TypeKind.MAP_TYPE) {
            return delimited(t.getDescriptor().getKeyElementType()) && delimited(element(t));
        }
        return !"FINAL".equals(extensibility(t));
    }

    private static boolean has(DynamicTypeMember member, String annotation) {
        for (AnnotationDescriptor a : member.getAnnotations()) {
            if (a.getType() != null && annotation.equals(a.getType().getName())) {
                return true;
            }
        }
        return false;
    }

    /** The members of {@code type} with those of its base types in front (7.2.4.4.8). */
    private static List<DynamicTypeMember> expanded(DynamicType type) {
        List<DynamicTypeMember> out = new ArrayList<DynamicTypeMember>();
        DynamicType base = type.getDescriptor().getBaseType();
        if (base != null) {
            out.addAll(expanded(base));
        }
        out.addAll(type.getAllMembers().values());
        return out;
    }

    private static Map<Integer, DynamicTypeMember> byId(List<DynamicTypeMember> members) {
        Map<Integer, DynamicTypeMember> out = new LinkedHashMap<Integer, DynamicTypeMember>();
        for (DynamicTypeMember m : members) {
            out.put(m.getId(), m);
        }
        return out;
    }

    private static boolean namesAndIdsAgree(Map<Integer, DynamicTypeMember> m1,
            Map<Integer, DynamicTypeMember> m2) {
        for (DynamicTypeMember a : m1.values()) {
            for (DynamicTypeMember b : m2.values()) {
                boolean sameName = a.getName().equals(b.getName());
                boolean sameId = a.getId() == b.getId();
                if (sameName != sameId) {
                    return false;
                }
            }
        }
        return true;
    }

    // ---- enumerations, structures, unions -------------------------------------------------

    private static boolean enumeration(DynamicType t1, DynamicType t2) {
        if (!extensibility(t1).equals(extensibility(t2))) {
            return false;
        }
        Map<Integer, DynamicTypeMember> c1 = byId(new ArrayList<DynamicTypeMember>(
                t1.getAllMembers().values()));
        Map<Integer, DynamicTypeMember> c2 = byId(new ArrayList<DynamicTypeMember>(
                t2.getAllMembers().values()));
        if (!namesAndIdsAgree(c1, c2)) {
            return false;
        }
        return !"FINAL".equals(extensibility(t1)) || c1.keySet().equals(c2.keySet());
    }

    private static boolean structure(DynamicType t1, DynamicType t2) {
        if (!extensibility(t1).equals(extensibility(t2))) {
            return false;
        }
        List<DynamicTypeMember> l1 = expanded(t1);
        List<DynamicTypeMember> l2 = expanded(t2);
        Map<Integer, DynamicTypeMember> m1 = byId(l1);
        Map<Integer, DynamicTypeMember> m2 = byId(l2);
        if (!namesAndIdsAgree(m1, m2)) {
            return false;
        }
        boolean common = false;
        for (Map.Entry<Integer, DynamicTypeMember> e : m2.entrySet()) {
            DynamicTypeMember a = m1.get(e.getKey());
            DynamicTypeMember b = e.getValue();
            if (a == null) {
                if (has(b, "Key") || (!has(b, "Optional") && has(b, "MustUnderstand"))) {
                    return false;
                }
                continue;
            }
            common = true;
            if (!assignableFrom(a.getDescriptor().getType(), b.getDescriptor().getType())) {
                return false;
            }
            if (has(a, "Key") != has(b, "Key") && (has(a, "Key") || has(b, "Key"))) {
                return false;
            }
            if (has(b, "Key") && !keyCompatible(a.getDescriptor().getType(),
                    b.getDescriptor().getType())) {
                return false;
            }
        }
        for (Map.Entry<Integer, DynamicTypeMember> e : m1.entrySet()) {
            if (!m2.containsKey(e.getKey())
                    && (has(e.getValue(), "Key")
                    || (!has(e.getValue(), "Optional") && has(e.getValue(), "MustUnderstand")))) {
                return false;
            }
        }
        // Two structures without members are the same type (7.2.4.1); the rule of a common
        // member presupposes members. An empty structure against one with members is refused.
        // As crates/types `at_least_one_common_member_id`.
        if (!common && !(l1.isEmpty() && l2.isEmpty())) {
            return false;
        }
        String ext = extensibility(t1);
        if (!"MUTABLE".equals(ext)) {
            int n = Math.min(l1.size(), l2.size());
            for (int i = 0; i < n; i++) {
                DynamicTypeMember a = l1.get(i);
                DynamicTypeMember b = l2.get(i);
                if (a.getId() != b.getId() || has(a, "Optional") != has(b, "Optional")
                        || !strongly(a.getDescriptor().getType(), b.getDescriptor().getType())) {
                    return false;
                }
            }
        }
        return !"FINAL".equals(ext) || m1.keySet().equals(m2.keySet());
    }

    /** The extra conditions on a key member (7.2.4.4.8): no loss of the key by the reader. */
    private static boolean keyCompatible(DynamicType keyOfReader, DynamicType keyOfWriter) {
        DynamicType k1 = resolve(keyOfReader);
        DynamicType k2 = resolve(keyOfWriter);
        TypeKind kind = k2.getKind();
        if (kind == TypeKind.STRING_TYPE || kind == TypeKind.SEQUENCE_TYPE
                || kind == TypeKind.MAP_TYPE) {
            long b1 = bound(k1);
            long b2 = bound(k2);
            return b1 == 0 || (b2 != 0 && b1 >= b2);
        }
        if (kind == TypeKind.ENUMERATION_TYPE) {
            for (DynamicTypeMember literal : k2.getAllMembers().values()) {
                if (!k1.getAllMembers().containsKey(literal.getId())) {
                    return false;
                }
            }
        }
        return true;
    }

    private static long bound(DynamicType type) {
        List<Integer> bound = type.getDescriptor().getBound();
        return bound == null || bound.isEmpty() ? 0 : bound.get(0) & 0xFFFFFFFFL;
    }

    private static boolean union(DynamicType t1, DynamicType t2) {
        if (!extensibility(t1).equals(extensibility(t2))) {
            return false;
        }
        DynamicType d1 = t1.getDescriptor().getDiscriminatorType();
        DynamicType d2 = t2.getDescriptor().getDiscriminatorType();
        if (d1 == null || d2 == null || !strongly(d1, d2)) {
            return false;
        }
        Map<Integer, DynamicTypeMember> m1 = byId(new ArrayList<DynamicTypeMember>(
                t1.getAllMembers().values()));
        Map<Integer, DynamicTypeMember> m2 = byId(new ArrayList<DynamicTypeMember>(
                t2.getAllMembers().values()));
        if (!namesAndIdsAgree(m1, m2)) {
            return false;
        }
        for (Map.Entry<Integer, DynamicTypeMember> e : m2.entrySet()) {
            DynamicTypeMember a = m1.get(e.getKey());
            if (a != null && !assignableFrom(a.getDescriptor().getType(),
                    e.getValue().getDescriptor().getType())) {
                return false;
            }
        }
        List<Integer> labels1 = labels(m1);
        List<Integer> labels2 = labels(m2);
        if ("FINAL".equals(extensibility(t1))) {
            return labels1.size() == labels2.size() && labels1.containsAll(labels2);
        }
        for (Integer label : labels2) {
            if (labels1.contains(label)) {
                return true;
            }
        }
        return false;
    }

    private static List<Integer> labels(Map<Integer, DynamicTypeMember> members) {
        List<Integer> out = new ArrayList<Integer>();
        for (DynamicTypeMember m : members.values()) {
            if (!m.getDescriptor().isDefaultLabel() && m.getDescriptor().getLabel() != null) {
                out.addAll(m.getDescriptor().getLabel());
            }
        }
        return out;
    }
}
