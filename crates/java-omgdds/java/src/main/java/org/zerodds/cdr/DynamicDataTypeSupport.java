// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.Float128;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.WstringLengthPolicy;
import org.zerodds.wire.WstringUnitOrder;
import org.zerodds.wire.Writer;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * The XCDR2 codec of the samples of a {@link DynamicType}: a
 * {@link TopicTypeSupport} over {@link DynamicData}, so that a topic created
 * from a DynamicType (DDS-Java-PSM 1.0 §7.5.1, §7.8.1) can be written and
 * read like one created from a Java class.
 *
 * <h2>Layout</h2>
 * The bytes are those {@link ReflectionTypeSupport} writes for the Java
 * class that corresponds to the type (XTypes 1.3 §7.4): a structure with its
 * base type's members first; FINAL members in sequence,
 * APPENDABLE with a DHEADER, MUTABLE with a DHEADER and one EMHEADER per
 * member; sequences, arrays of non-primitive elements and maps delimited; a
 * union as discriminator and the selected member. Little-endian.
 *
 * <h2>Annotations</h2>
 * A type's extensibility is the {@code Extensibility} annotation of the
 * type ({@code FINAL}, {@code APPENDABLE}, {@code MUTABLE}); without it the
 * type is APPENDABLE (XTypes 1.3 §7.2.2.4.4.4), as a Java class that is not final is (EXTENSIBLE of
 * §8.6.1.2).
 * Members that carry the annotations {@code Key}, {@code Optional} or
 * {@code MustUnderstand} are keys, optional or must-understand members
 * (matched by simple name, as {@link ReflectionTypeSupport} matches).
 *
 * <h2>Values</h2>
 * The kinds map to Java values as Clause 8 and §7.8.1.4 state them: an
 * unsigned type is held in the signed type of the same size, FLOAT_128 in a
 * {@link BigDecimal}, an enumeration or bit mask in an {@code Integer} or
 * {@code Long}, a sequence or array of a primitive kind in a Java array of
 * it (or a list), a structure or union in a {@link DynamicData}. A member
 * that has no value is written as the default of its type (zero, false, the
 * empty string, the empty collection; for an optional member, nothing). The
 * union discriminator is kept under {@link DynamicDataImpl#DISCRIMINATOR_ID}.
 *
 * <p>A narrow string is written as {@link NarrowStrings} writes it (UTF-8,
 * XTypes 1.3 §7.4.1.1.2; the Java PSM §8.5.1 form is a named setting).
 * CHAR_8 is one octet ({@code Writer.writeChar} refuses a character above U+00FF); CHAR_32 is a four-octet code point.
 */
public final class DynamicDataTypeSupport implements TopicTypeSupport<DynamicData> {

    private final ServiceEnvironment environment;
    private final DynamicType type;
    private final ExtensibilityKind extensibility;
    private final boolean keyed;

    public DynamicDataTypeSupport(ServiceEnvironment environment, DynamicType type) {
        if (type == null) {
            throw new IllegalArgumentException("a DynamicType must not be null");
        }
        TypeKind kind = resolve(type).getKind();
        if (kind != TypeKind.STRUCTURE_TYPE && kind != TypeKind.UNION_TYPE) {
            throw new IllegalArgumentException("the samples of a topic are structures or"
                    + " unions; " + type.getName() + " is a " + kind);
        }
        this.environment = environment;
        this.type = type;
        this.extensibility = extensibilityOf(resolve(type));
        boolean anyKey = false;
        for (DynamicTypeMember member : membersOf(resolve(type))) {
            anyKey |= has(member.getAnnotations(), "Key");
        }
        this.keyed = anyKey;
    }

    /** The type this codec writes. */
    public DynamicType dynamicType() {
        return type;
    }

    /** A new, empty sample of the type. */
    public DynamicData newData() {
        return new DynamicDataImpl(environment, type);
    }

    @Override
    public String getTypeName() {
        return type.getName();
    }

    @Override
    public java.util.List<String> baseTypeNames() {
        java.util.List<String> names = new ArrayList<>();
        for (DynamicType base = type.getDescriptor().getBaseType(); base != null;
                base = base.getDescriptor().getBaseType()) {
            names.add(base.getName());
        }
        return names;
    }

    @Override
    public boolean isKeyed() {
        return keyed;
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        return extensibility;
    }

    // ------------------------------------------------------------------
    // TopicTypeSupport
    // ------------------------------------------------------------------

    @Override
    public byte[] encode(DynamicData sample) {
        return encode(sample, EndianMode.LITTLE_ENDIAN);
    }

    @Override
    public byte[] encode(DynamicData sample, EndianMode endian) {
        if (sample == null) {
            throw new XcdrException("sample must not be null");
        }
        Writer w = new Writer(order(endian), 4, PlCdr1Framing.defaults(), false);
        encodeValue(w, type, sample);
        return w.finish();
    }

    @Override
    public DynamicData decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    public DynamicData decode(byte[] bytes, int offset, int length) {
        Reader r = new Reader(Arrays.copyOfRange(bytes, offset, offset + length),
                Endianness.LITTLE, 4, PlCdr1Framing.defaults());
        return (DynamicData) decodeValue(r, type);
    }

    @Override
    public byte[] keyHash(DynamicData sample) {
        byte[] out = new byte[16];
        if (!keyed || sample == null) {
            return out;
        }
        // XTypes 1.3 §7.6.8: the key members, big-endian, in declaration order.
        Writer w = new Writer(Endianness.BIG, 4, PlCdr1Framing.defaults(), false);
        DynamicDataImpl data = data(sample);
        for (DynamicTypeMember member : membersOf(resolve(type))) {
            if (has(member.getAnnotations(), "Key")) {
                encodeValue(w, member.getDescriptor().getType(),
                        data.rawValue(member.getId()));
            }
        }
        byte[] holder = w.finish();
        // RTPS 2.5 §9.6.3.8 step 5: the maximum serialized size decides, not the actual one
        if (holder.length <= 16 && !hasKeyOfUnboundedSize()) {
            System.arraycopy(holder, 0, out, 0, holder.length);
            return out;
        }
        return Md5.hash(holder);
    }

    private boolean hasKeyOfUnboundedSize() {
        for (DynamicTypeMember member : membersOf(resolve(type))) {
            if (!has(member.getAnnotations(), "Key")) {
                continue;
            }
            DynamicType t = member.getDescriptor().getType();
            switch (t.getKind()) {
                case STRING_TYPE: {
                    int bound = bound(t);
                    if (bound == 0 || bound + 5 > 16) {
                        return true;
                    }
                    break;
                }
                case SEQUENCE_TYPE:
                case MAP_TYPE:
                    return true;
                default:
                    break;
            }
        }
        return false;
    }

    private static Endianness order(EndianMode endian) {
        java.util.Objects.requireNonNull(endian, "endian");
        switch (endian) {
            case LITTLE_ENDIAN:
                return Endianness.LITTLE;
            case BIG_ENDIAN:
                return Endianness.BIG;
            default:
                throw new IllegalArgumentException("Unsupported byte order");
        }
    }

    // ------------------------------------------------------------------
    // The model: aliases, members, annotations
    // ------------------------------------------------------------------

    /** The type behind any aliases. */
    static DynamicType resolve(DynamicType t) {
        DynamicType current = t;
        int depth = 0;
        while (current.getKind() == TypeKind.ALIAS_TYPE) {
            DynamicType base = current.getDescriptor().getBaseType();
            if (base == null || ++depth > 64) {
                throw new XcdrException("the alias " + current.getName() + " has no base type");
            }
            current = base;
        }
        return current;
    }

    /** The members of a structure: the base type's first. */
    static List<DynamicTypeMember> membersOf(DynamicType t) {
        List<DynamicTypeMember> out = new ArrayList<>();
        DynamicType base = t.getDescriptor().getBaseType();
        if (base != null && t.getKind() == TypeKind.STRUCTURE_TYPE) {
            out.addAll(membersOf(resolve(base)));
        }
        out.addAll(t.getAllMembers().values());
        return out;
    }

    static boolean has(List<AnnotationDescriptor> annotations, String simpleName) {
        return annotation(annotations, simpleName) != null;
    }

    private static AnnotationDescriptor annotation(List<AnnotationDescriptor> annotations,
            String simpleName) {
        if (annotations == null) {
            return null;
        }
        for (AnnotationDescriptor a : annotations) {
            DynamicType t = a.getType();
            if (t != null && t.getName() != null) {
                String name = t.getName();
                int cut = Math.max(name.lastIndexOf('.'), name.lastIndexOf(':'));
                if (name.substring(cut + 1).equalsIgnoreCase(simpleName)) {
                    String value = a.getValue("value");
                    if (value == null || !value.equalsIgnoreCase("false")) {
                        return a;
                    }
                }
            }
        }
        return null;
    }

    private static ExtensibilityKind extensibilityOf(DynamicType t) {
        AnnotationDescriptor a = annotation(new ArrayList<>(t.getAllAnnotations().values()),
                "Extensibility");
        if (a == null || a.getValue("value") == null) {
            return ExtensibilityKind.APPENDABLE;
        }
        String v = a.getValue("value").toUpperCase();
        if (v.contains("APPENDABLE")) {
            return ExtensibilityKind.APPENDABLE;
        }
        if (v.contains("MUTABLE")) {
            return ExtensibilityKind.MUTABLE;
        }
        if (v.contains("EXTENSIBLE")) {
            return ExtensibilityKind.APPENDABLE;
        }
        return ExtensibilityKind.FINAL;
    }

    private static DynamicDataImpl data(Object sample) {
        if (!(sample instanceof DynamicDataImpl)) {
            throw new XcdrException("a DynamicData of another implementation cannot be"
                    + " encoded: " + sample);
        }
        return (DynamicDataImpl) sample;
    }

    // ------------------------------------------------------------------
    // Encoding
    // ------------------------------------------------------------------

    private void encodeValue(Writer w, DynamicType declared, Object value) {
        DynamicType t = resolve(declared);
        switch (t.getKind()) {
            case BOOLEAN_TYPE:
                w.writeBool(value != null && (Boolean) value);
                break;
            case BYTE_TYPE:
                w.writeU8(value == null ? 0 : ((Number) value).intValue() & 0xff);
                break;
            case INT_16_TYPE:
                w.writeI16(value == null ? 0 : ((Number) value).shortValue());
                break;
            case UINT_16_TYPE:
                w.writeU16(value == null ? 0 : ((Number) value).intValue() & 0xffff);
                break;
            case INT_32_TYPE:
                w.writeI32(value == null ? 0 : ((Number) value).intValue());
                break;
            case UINT_32_TYPE:
                w.writeU32(value == null ? 0 : ((Number) value).longValue() & 0xffffffffL);
                break;
            case INT_64_TYPE:
                w.writeI64(value == null ? 0 : ((Number) value).longValue());
                break;
            case UINT_64_TYPE:
                w.writeU64(value == null ? 0
                        : value instanceof BigInteger
                                ? ((BigInteger) value).longValue() : ((Number) value).longValue());
                break;
            case FLOAT_32_TYPE:
                w.writeF32(value == null ? 0 : ((Number) value).floatValue());
                break;
            case FLOAT_64_TYPE:
                w.writeF64(value == null ? 0 : ((Number) value).doubleValue());
                break;
            case FLOAT_128_TYPE:
                w.writeF128(Float128.fromBigDecimal(value == null ? BigDecimal.ZERO
                        : value instanceof BigDecimal ? (BigDecimal) value
                                : new BigDecimal(value.toString())));
                break;
            case CHAR_8_TYPE:
                w.writeChar(value == null ? 0 : (Character) value);
                break;
            case CHAR_32_TYPE:
                w.writeU32(value == null ? 0
                        : value instanceof Character ? (Character) value
                                : ((Number) value).longValue() & 0xffffffffL);
                break;
            case ENUMERATION_TYPE:
                w.writeI32(value == null ? 0 : ((Number) value).intValue());
                break;
            case BITSET_TYPE:
                w.writeBitset(bitsetOctets(t), value == null ? 0 : ((Number) value).longValue());
                break;
            case STRING_TYPE:
                encodeString(w, t, value == null ? "" : value.toString());
                break;
            case ARRAY_TYPE:
                encodeArray(w, t, value);
                break;
            case SEQUENCE_TYPE:
                encodeSequence(w, t, value);
                break;
            case MAP_TYPE:
                encodeMap(w, t, value);
                break;
            case STRUCTURE_TYPE:
                encodeStructure(w, t, value == null ? new DynamicDataImpl(environment, t)
                        : data(value));
                break;
            case UNION_TYPE:
                encodeUnion(w, t, value == null ? new DynamicDataImpl(environment, t)
                        : data(value));
                break;
            default:
                throw new XcdrException("the TypeKind " + t.getKind()
                        + " has no XCDR2 form as a topic data type");
        }
    }

    private static int bitsetOctets(DynamicType t) {
        List<Integer> bound = t.getDescriptor().getBound();
        int bits = bound == null || bound.isEmpty() ? 32 : bound.get(0);
        return bits <= 8 ? 1 : bits <= 16 ? 2 : bits <= 32 ? 4 : 8;
    }

    private static boolean wide(DynamicType t) {
        return "WString".equals(t.getName());
    }

    private static int bound(DynamicType t) {
        List<Integer> bound = t.getDescriptor().getBound();
        return bound == null || bound.isEmpty() ? 0 : bound.get(0);
    }

    private static void encodeString(Writer w, DynamicType t, String value) {
        int bound = bound(t);
        if (wide(t)) {
            if (bound > 0) {
                w.writeWstringBounded(value, bound, WstringLengthPolicy.SPECIFICATION,
                        WstringUnitOrder.STREAM);
            } else {
                w.writeWstringUtf8(value, WstringLengthPolicy.SPECIFICATION,
                        WstringUnitOrder.STREAM);
            }
        } else {
            NarrowStrings.write(w, value, bound);
        }
    }

    private static boolean primitive(DynamicType declared) {
        switch (resolve(declared).getKind()) {
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
            case ENUMERATION_TYPE:
            case BITSET_TYPE:
                return true;
            default:
                return false;
        }
    }

    /** The elements of a stored collection value: a Java array of any component type, or a Collection. */
    private static List<Object> elements(Object value) {
        List<Object> out = new ArrayList<>();
        if (value == null) {
            return out;
        }
        if (value instanceof Collection) {
            out.addAll((Collection<?>) value);
        } else if (value instanceof CharSequence) {
            String text = value.toString();
            for (int i = 0; i < text.length(); i++) {
                out.add(text.charAt(i));
            }
        } else if (value.getClass().isArray()) {
            int n = java.lang.reflect.Array.getLength(value);
            for (int i = 0; i < n; i++) {
                out.add(java.lang.reflect.Array.get(value, i));
            }
        } else {
            throw new XcdrException("a collection member holds a "
                    + value.getClass().getName());
        }
        return out;
    }

    private void encodeSequence(Writer w, DynamicType t, Object value) {
        DynamicType element = t.getDescriptor().getElementType();
        List<Object> values = elements(value);
        int bound = bound(t);
        if (bound > 0) {
            w.checkSequenceBound(values.size(), bound);
        }
        if (primitive(element)) {
            w.writeU32(values.size());
            for (Object v : values) {
                encodeValue(w, element, v);
            }
        } else {
            Writer.DHeader dh = w.writeDheader();
            w.writeU32(values.size());
            for (Object v : values) {
                encodeValue(w, element, v);
            }
            w.finishDheader(dh);
        }
    }

    private static int arrayLength(DynamicType t) {
        int total = 1;
        for (int b : t.getDescriptor().getBound()) {
            total *= b;
        }
        return total;
    }

    private void encodeArray(Writer w, DynamicType t, Object value) {
        DynamicType element = t.getDescriptor().getElementType();
        List<Object> values = elements(value);
        int length = arrayLength(t);
        if (values.size() > length) {
            throw new XcdrException("the array " + t.getName() + " holds " + length
                    + " elements, not " + values.size());
        }
        Writer.DHeader dh = primitive(element) ? null : w.writeDheader();
        for (int i = 0; i < length; i++) {
            encodeValue(w, element, i < values.size() ? values.get(i) : null);
        }
        if (dh != null) {
            w.finishDheader(dh);
        }
    }

    private void encodeMap(Writer w, DynamicType t, Object value) {
        DynamicType key = t.getDescriptor().getKeyElementType();
        DynamicType element = t.getDescriptor().getElementType();
        Map<?, ?> map = value == null ? new LinkedHashMap<>() : (Map<?, ?>) value;
        int bound = bound(t);
        if (bound > 0) {
            w.checkMapBound(map.size(), bound);
        }
        Writer.DHeader dh = w.writeDheader();
        w.writeU32(map.size());
        for (Map.Entry<?, ?> e : map.entrySet()) {
            encodeValue(w, key, e.getKey());
            encodeValue(w, element, e.getValue());
        }
        w.finishDheader(dh);
    }

    /**
     * A member of a FINAL or APPENDABLE structure: an optional one is preceded by the boolean
     * is_present (XTypes 1.3 §7.4.3.5.2, rule 20), and written only when it has a value.
     */
    private void encodePlainMember(Writer w, DynamicTypeMember m, DynamicDataImpl sample) {
        if (has(m.getAnnotations(), "Optional")) {
            boolean present = sample.hasValue(m.getId());
            w.writeBool(present);
            if (!present) {
                return;
            }
        }
        encodeMember(w, m, sample);
    }

    private void decodePlainMember(Reader r, DynamicTypeMember m, DynamicDataImpl sample) {
        if (has(m.getAnnotations(), "Optional") && !r.readBool()) {
            return;
        }
        sample.putRaw(m.getId(), decodeValue(r, m.getDescriptor().getType()));
    }

    private void encodeStructure(Writer w, DynamicType t, DynamicDataImpl sample) {
        List<DynamicTypeMember> members = membersOf(t);
        ExtensibilityKind ext = extensibilityOf(t);
        switch (ext) {
            case FINAL:
            case EXTENSIBLE:
                for (DynamicTypeMember m : members) {
                    encodePlainMember(w, m, sample);
                }
                break;
            case APPENDABLE: {
                Writer.DHeader dh = w.writeDheader();
                for (DynamicTypeMember m : members) {
                    encodePlainMember(w, m, sample);
                }
                w.finishDheader(dh);
                break;
            }
            case MUTABLE: {
                Writer.DHeader dh = w.writeDheader();
                for (DynamicTypeMember m : members) {
                    boolean optional = has(m.getAnnotations(), "Optional");
                    if (optional && !sample.hasValue(m.getId())) {
                        continue; // an absent optional member has no EMHEADER.
                    }
                    Writer.EmHeader h = w.writeEmheader(m.getId(), 4,
                            has(m.getAnnotations(), "MustUnderstand"));
                    encodeValue(w, m.getDescriptor().getType(), sample.rawValue(m.getId()));
                    w.finishEmheader(h);
                }
                w.finishDheader(dh);
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ext);
        }
    }

    private void encodeMember(Writer w, DynamicTypeMember m, DynamicDataImpl sample) {
        encodeValue(w, m.getDescriptor().getType(), sample.rawValue(m.getId()));
    }

    /** The label that selects {@code member}, or the default selection of a default member. */
    private static int labelOf(DynamicType t, DynamicTypeMember member) {
        MemberDescriptor d = member.getDescriptor();
        if (d.getLabel() != null && !d.getLabel().isEmpty()) {
            return d.getLabel().get(0);
        }
        // The default member: a discriminator value no other member uses.
        List<Integer> used = new ArrayList<>();
        for (DynamicTypeMember other : t.getAllMembers().values()) {
            if (other.getDescriptor().getLabel() != null) {
                used.addAll(other.getDescriptor().getLabel());
            }
        }
        int candidate = 0;
        while (used.contains(candidate)) {
            candidate++;
        }
        return candidate;
    }

    private void encodeUnion(Writer w, DynamicType t, DynamicDataImpl sample) {
        DynamicType discriminator = t.getDescriptor().getDiscriminatorType();
        if (discriminator == null) {
            throw new XcdrException("the union " + t.getName() + " has no discriminator type");
        }
        DynamicTypeMember active = null;
        for (DynamicTypeMember m : t.getAllMembers().values()) {
            if (sample.hasValue(m.getId())) {
                active = m;
                break;
            }
        }
        Object disc = sample.rawValue(DynamicDataImpl.DISCRIMINATOR_ID);
        if (active != null && disc == null) {
            disc = labelOf(t, active);
        }
        if (active == null && disc != null) {
            // The discriminator names the member (or none).
            int label = ((Number) (disc instanceof Boolean ? ((Boolean) disc ? 1 : 0)
                    : disc instanceof Character ? (int) (Character) disc : disc)).intValue();
            for (DynamicTypeMember m : t.getAllMembers().values()) {
                if (m.getDescriptor().getLabel() != null
                        && m.getDescriptor().getLabel().contains(label)) {
                    active = m;
                    break;
                }
            }
        }
        ExtensibilityKind ext = extensibilityOf(t);
        Writer.DHeader dh = ext == ExtensibilityKind.FINAL || ext == ExtensibilityKind.EXTENSIBLE
                ? null : w.writeDheader();
        if (ext == ExtensibilityKind.MUTABLE) {
            Writer.EmHeader h = w.writeEmheader(0, 4, true);
            encodeValue(w, discriminator, disc == null ? 0 : disc);
            w.finishEmheader(h);
            if (active != null) {
                Writer.EmHeader m = w.writeEmheader(active.getId(), 4, false);
                encodeValue(w, active.getDescriptor().getType(), sample.rawValue(active.getId()));
                w.finishEmheader(m);
            }
        } else {
            encodeValue(w, discriminator, disc == null ? 0 : disc);
            if (active != null) {
                encodeValue(w, active.getDescriptor().getType(), sample.rawValue(active.getId()));
            }
        }
        if (dh != null) {
            w.finishDheader(dh);
        }
    }

    // ------------------------------------------------------------------
    // Decoding
    // ------------------------------------------------------------------

    private Object decodeValue(Reader r, DynamicType declared) {
        DynamicType t = resolve(declared);
        switch (t.getKind()) {
            case BOOLEAN_TYPE:
                return r.readBool();
            case BYTE_TYPE:
                return (byte) r.readU8();
            case INT_16_TYPE:
            case UINT_16_TYPE:
                return (short) (t.getKind() == TypeKind.INT_16_TYPE ? r.readI16() : r.readU16());
            case INT_32_TYPE:
                return r.readI32();
            case UINT_32_TYPE:
                return (int) r.readU32();
            case INT_64_TYPE:
                return r.readI64();
            case UINT_64_TYPE:
                return r.readU64();
            case FLOAT_32_TYPE:
                return r.readF32();
            case FLOAT_64_TYPE:
                return r.readF64();
            case FLOAT_128_TYPE:
                return r.readF128().toBigDecimal();
            case CHAR_8_TYPE:
                return (char) r.readChar();
            case CHAR_32_TYPE:
                return (int) r.readU32();
            case ENUMERATION_TYPE:
                return r.readI32();
            case BITSET_TYPE:
                return r.readBitset(bitsetOctets(t));
            case STRING_TYPE:
                return wide(t)
                        ? r.readWstring(WstringLengthPolicy.SPECIFICATION, WstringUnitOrder.STREAM)
                        : NarrowStrings.read(r, bound(t));
            case ARRAY_TYPE:
                return decodeArray(r, t);
            case SEQUENCE_TYPE:
                return decodeSequence(r, t);
            case MAP_TYPE:
                return decodeMap(r, t);
            case STRUCTURE_TYPE:
                return decodeStructure(r, t);
            case UNION_TYPE:
                return decodeUnion(r, t);
            default:
                throw new XcdrException("the TypeKind " + t.getKind()
                        + " has no XCDR2 form as a topic data type");
        }
    }

    /** A decoded list of {@code element} values in the Java form §7.8.1.4 reads them. */
    static Object collection(DynamicType element, List<Object> values) {
        switch (resolve(element).getKind()) {
            case BOOLEAN_TYPE: {
                boolean[] out = new boolean[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Boolean) values.get(i);
                }
                return out;
            }
            case BYTE_TYPE: {
                byte[] out = new byte[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Byte) values.get(i);
                }
                return out;
            }
            case INT_16_TYPE:
            case UINT_16_TYPE: {
                short[] out = new short[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Short) values.get(i);
                }
                return out;
            }
            case INT_32_TYPE:
            case UINT_32_TYPE:
            case ENUMERATION_TYPE: {
                int[] out = new int[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Integer) values.get(i);
                }
                return out;
            }
            case INT_64_TYPE:
            case UINT_64_TYPE: {
                long[] out = new long[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Long) values.get(i);
                }
                return out;
            }
            case FLOAT_32_TYPE: {
                float[] out = new float[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Float) values.get(i);
                }
                return out;
            }
            case FLOAT_64_TYPE: {
                double[] out = new double[values.size()];
                for (int i = 0; i < out.length; i++) {
                    out[i] = (Double) values.get(i);
                }
                return out;
            }
            default:
                return values;
        }
    }

    private Object decodeSequence(Reader r, DynamicType t) {
        DynamicType element = t.getDescriptor().getElementType();
        List<Object> out = new ArrayList<>();
        if (!primitive(element)) {
            r.readDheader();
        }
        int n = r.readSequenceLength();
        for (int i = 0; i < n; i++) {
            out.add(decodeValue(r, element));
        }
        return collection(element, out);
    }

    private Object decodeArray(Reader r, DynamicType t) {
        DynamicType element = t.getDescriptor().getElementType();
        if (!primitive(element)) {
            r.readDheader();
        }
        int length = arrayLength(t);
        List<Object> out = new ArrayList<>(length);
        for (int i = 0; i < length; i++) {
            out.add(decodeValue(r, element));
        }
        return collection(element, out);
    }

    private Object decodeMap(Reader r, DynamicType t) {
        DynamicType key = t.getDescriptor().getKeyElementType();
        DynamicType element = t.getDescriptor().getElementType();
        r.readDheader();
        int n = r.readSequenceLength();
        Map<Object, Object> out = new LinkedHashMap<>();
        for (int i = 0; i < n; i++) {
            Object k = decodeValue(r, key);
            out.put(k, decodeValue(r, element));
        }
        return out;
    }

    private Object decodeStructure(Reader r, DynamicType t) {
        DynamicDataImpl sample = new DynamicDataImpl(environment, t);
        List<DynamicTypeMember> members = membersOf(t);
        ExtensibilityKind ext = extensibilityOf(t);
        switch (ext) {
            case FINAL:
            case EXTENSIBLE:
                for (DynamicTypeMember m : members) {
                    decodePlainMember(r, m, sample);
                }
                break;
            case APPENDABLE: {
                int size = r.readDheader();
                int end = r.position() + size;
                for (DynamicTypeMember m : members) {
                    if (r.position() >= end) {
                        break; // a trailing member the sender does not have.
                    }
                    decodePlainMember(r, m, sample);
                }
                while (r.position() < end) {
                    r.skip(1); // members appended by a newer version of the type.
                }
                break;
            }
            case MUTABLE: {
                int size = r.readDheader();
                int end = r.position() + size;
                Map<Integer, DynamicTypeMember> byId = new LinkedHashMap<>();
                for (DynamicTypeMember m : members) {
                    byId.put(m.getId(), m);
                }
                while (r.position() < end) {
                    Reader.EmHeaderRead e = r.readEmheader();
                    DynamicTypeMember m = byId.get(e.memberId());
                    if (m == null) {
                        r.skipMember(e);
                        continue;
                    }
                    sample.putRaw(m.getId(), decodeValue(r, m.getDescriptor().getType()));
                }
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ext);
        }
        return sample;
    }

    private Object decodeUnion(Reader r, DynamicType t) {
        DynamicType discriminator = t.getDescriptor().getDiscriminatorType();
        if (discriminator == null) {
            throw new XcdrException("the union " + t.getName() + " has no discriminator type");
        }
        DynamicDataImpl sample = new DynamicDataImpl(environment, t);
        ExtensibilityKind ext = extensibilityOf(t);
        int end = -1;
        if (ext != ExtensibilityKind.FINAL && ext != ExtensibilityKind.EXTENSIBLE) {
            end = r.position() + r.readDheader();
        }
        if (ext == ExtensibilityKind.MUTABLE) {
            Map<Integer, DynamicTypeMember> byId = new LinkedHashMap<>();
            for (DynamicTypeMember m : t.getAllMembers().values()) {
                byId.put(m.getId(), m);
            }
            while (r.position() < end) {
                Reader.EmHeaderRead e = r.readEmheader();
                if (e.memberId() == 0) {
                    sample.putRaw(DynamicDataImpl.DISCRIMINATOR_ID, decodeValue(r, discriminator));
                    continue;
                }
                DynamicTypeMember m = byId.get(e.memberId());
                if (m == null) {
                    r.skipMember(e);
                    continue;
                }
                sample.putRaw(m.getId(), decodeValue(r, m.getDescriptor().getType()));
            }
            return sample;
        }
        Object disc = decodeValue(r, discriminator);
        sample.putRaw(DynamicDataImpl.DISCRIMINATOR_ID, disc);
        int label = disc instanceof Boolean ? ((Boolean) disc ? 1 : 0)
                : disc instanceof Character ? (Character) disc : ((Number) disc).intValue();
        DynamicTypeMember selected = null;
        DynamicTypeMember byDefault = null;
        for (DynamicTypeMember m : t.getAllMembers().values()) {
            MemberDescriptor d = m.getDescriptor();
            if (d.getLabel() != null && d.getLabel().contains(label)) {
                selected = m;
                break;
            }
            if (d.isDefaultLabel()) {
                byDefault = m;
            }
        }
        if (selected == null) {
            selected = byDefault;
        }
        if (selected != null && (end < 0 || r.position() < end)) {
            sample.putRaw(selected.getId(), decodeValue(r, selected.getDescriptor().getType()));
        }
        if (end >= 0) {
            while (r.position() < end) {
                r.skip(1);
            }
        }
        return sample;
    }
}
