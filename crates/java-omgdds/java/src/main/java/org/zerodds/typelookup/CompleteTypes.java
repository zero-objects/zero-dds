// SPDX-License-Identifier: Apache-2.0
package org.zerodds.typelookup;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.omg.dds.type.dynamic.TypeDescriptor;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.cdr.HexBytes;
import org.zerodds.typelookup.generated.ZdCodec;
import org.zerodds.typelookup.generated.ZdTypes.DDS.XTypes;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.Reader;
import org.zerodds.wire.TryConstruct;

/**
 * The {@link DynamicType} a COMPLETE TypeObject of XTypes 1.3 (Annex B, §7.3.4) describes,
 * with the objects it names looked up by their TypeIdentifier. What
 * {@link org.zerodds.cdr.TypeAssignability} judges two types by: extensibility, member ids and
 * names, the key, optional and must-understand flags, base types, union discriminators and
 * labels, bounds and element types.
 *
 * <p>The Java PSM's {@link TypeKind} predates XTypes 1.2: {@code int8} and {@code uint8} are
 * {@code BYTE_TYPE} named {@code Int8} and {@code UInt8}, which
 * {@link org.zerodds.cdr.TypeAssignability} tells from an {@code octet}; {@code char16} is
 * {@code CHAR_32_TYPE}, the wide character of the PSM; a bitmask is a {@code BITSET_TYPE} named
 * {@code Bitmask} with its {@code bit_bound}.
 *
 * <p>Not described: annotation types (no topic type has a member of one), a strongly connected
 * component identifier (§7.3.4.9) and a MINIMAL hash, which a COMPLETE object does not name.
 */
public final class CompleteTypes {

    /** An object the type needs is not at hand: the identifier to ask the peer for. */
    public static final class Missing extends Exception {
        private static final long serialVersionUID = 1L;
        private final byte[] identifier;

        Missing(byte[] identifier) {
            super("no TypeObject for " + HexBytes.encode(identifier));
            this.identifier = identifier.clone();
        }

        /** The encoded TypeIdentifier of the object. */
        public byte[] identifier() {
            return identifier.clone();
        }
    }

    private final DynamicTypeFactoryImpl factory = new DynamicTypeFactoryImpl(null);
    private final Function<byte[], byte[]> objects;
    private final Map<String, DynamicType> built = new HashMap<String, DynamicType>();

    private CompleteTypes(Function<byte[], byte[]> objects) {
        this.objects = objects;
    }

    /**
     * The type the hash identifier {@code identifier} names.
     *
     * @param identifier the encoded TypeIdentifier (discriminator and 14 octets of hash)
     * @param objects the serialized TypeObject (XCDR2 little-endian) of an encoded
     *     identifier, or {@code null} when it is not at hand
     * @throws Missing when an object the type names is not at hand
     * @throws IllegalArgumentException when an object does not decode or names what this class
     *     does not describe
     */
    public static DynamicType of(byte[] identifier, Function<byte[], byte[]> objects)
            throws Missing {
        CompleteTypes c = new CompleteTypes(objects);
        return c.type(decodeIdentifier(identifier));
    }

    /** The encoded TypeIdentifier of {@code id}, as the registries key their objects. */
    public static byte[] encodeIdentifier(XTypes.TypeIdentifier id) {
        org.zerodds.wire.Writer writer = TypeLookupService.writer();
        ZdCodec.zd_enc_DDS_XTypes_TypeIdentifier(writer, id);
        return writer.finish();
    }

    /** The TypeIdentifier encoded in {@code bytes}. */
    public static XTypes.TypeIdentifier decodeIdentifier(byte[] bytes) {
        Reader reader = TypeLookupService.reader(bytes, Endianness.LITTLE);
        XTypes.TypeIdentifier value = ZdCodec.zd_dec_DDS_XTypes_TypeIdentifier(reader, TryConstruct.DISCARD);
        if (reader.remaining() != 0) {
            throw new IllegalArgumentException("trailing TypeIdentifier octets");
        }
        return value;
    }

    /** The TypeObject serialized in {@code bytes} (XCDR2 little-endian). */
    public static XTypes.TypeObject decodeObject(byte[] bytes) {
        Reader reader = TypeLookupService.reader(bytes, Endianness.LITTLE);
        XTypes.TypeObject value = ZdCodec.zd_dec_DDS_XTypes_TypeObject(reader, TryConstruct.DISCARD);
        if (reader.remaining() != 0) {
            throw new IllegalArgumentException("trailing TypeObject octets");
        }
        return value;
    }

    /** {@code object} serialized as its hash is computed over (§7.3.4.8): XCDR2 little-endian. */
    public static byte[] encodeObject(XTypes.TypeObject object) {
        org.zerodds.wire.Writer writer = TypeLookupService.writer();
        ZdCodec.zd_enc_DDS_XTypes_TypeObject(writer, object);
        return writer.finish();
    }

    // ---- identifiers ----------------------------------------------------------------------

    private DynamicType type(XTypes.TypeIdentifier id) throws Missing {
        switch (id._d) {
            case XTypes.TK_BOOLEAN: return factory.getPrimitiveType(TypeKind.BOOLEAN_TYPE);
            case XTypes.TK_BYTE: return factory.getPrimitiveType(TypeKind.BYTE_TYPE);
            case XTypes.TK_INT8: return named(TypeKind.BYTE_TYPE, "Int8");
            case XTypes.TK_UINT8: return named(TypeKind.BYTE_TYPE, "UInt8");
            case XTypes.TK_INT16: return factory.getPrimitiveType(TypeKind.INT_16_TYPE);
            case XTypes.TK_UINT16: return factory.getPrimitiveType(TypeKind.UINT_16_TYPE);
            case XTypes.TK_INT32: return factory.getPrimitiveType(TypeKind.INT_32_TYPE);
            case XTypes.TK_UINT32: return factory.getPrimitiveType(TypeKind.UINT_32_TYPE);
            case XTypes.TK_INT64: return factory.getPrimitiveType(TypeKind.INT_64_TYPE);
            case XTypes.TK_UINT64: return factory.getPrimitiveType(TypeKind.UINT_64_TYPE);
            case XTypes.TK_FLOAT32: return factory.getPrimitiveType(TypeKind.FLOAT_32_TYPE);
            case XTypes.TK_FLOAT64: return factory.getPrimitiveType(TypeKind.FLOAT_64_TYPE);
            case XTypes.TK_FLOAT128: return factory.getPrimitiveType(TypeKind.FLOAT_128_TYPE);
            case XTypes.TK_CHAR8: return factory.getPrimitiveType(TypeKind.CHAR_8_TYPE);
            case XTypes.TK_CHAR16: return factory.getPrimitiveType(TypeKind.CHAR_32_TYPE);
            case XTypes.TI_STRING8_SMALL: return factory.createStringType(octet(id.string_sdefn.bound));
            case XTypes.TI_STRING8_LARGE: return factory.createStringType(bound(id.string_ldefn.bound));
            case XTypes.TI_STRING16_SMALL: return factory.createWStringType(octet(id.string_sdefn.bound));
            case XTypes.TI_STRING16_LARGE: return factory.createWStringType(bound(id.string_ldefn.bound));
            case XTypes.TI_PLAIN_SEQUENCE_SMALL:
                return sequence(type(id.seq_sdefn.element_identifier), octet(id.seq_sdefn.bound));
            case XTypes.TI_PLAIN_SEQUENCE_LARGE:
                return sequence(type(id.seq_ldefn.element_identifier), bound(id.seq_ldefn.bound));
            case XTypes.TI_PLAIN_ARRAY_SMALL: {
                int[] bounds = new int[id.array_sdefn.array_bound_seq.length];
                for (int i = 0; i < bounds.length; i++) {
                    bounds[i] = octet(id.array_sdefn.array_bound_seq[i]);
                }
                return factory.createArrayType(type(id.array_sdefn.element_identifier), bounds);
            }
            case XTypes.TI_PLAIN_ARRAY_LARGE: {
                int[] bounds = new int[id.array_ldefn.array_bound_seq.length];
                for (int i = 0; i < bounds.length; i++) {
                    bounds[i] = bound(id.array_ldefn.array_bound_seq[i]);
                }
                return factory.createArrayType(type(id.array_ldefn.element_identifier), bounds);
            }
            case XTypes.TI_PLAIN_MAP_SMALL:
                return factory.createMapType(type(id.map_sdefn.key_identifier),
                        type(id.map_sdefn.element_identifier), octet(id.map_sdefn.bound));
            case XTypes.TI_PLAIN_MAP_LARGE:
                return factory.createMapType(type(id.map_ldefn.key_identifier),
                        type(id.map_ldefn.element_identifier), bound(id.map_ldefn.bound));
            case XTypes.EK_COMPLETE:
                return hashed(id);
            default:
                throw new IllegalArgumentException(
                        "a TypeIdentifier of kind 0x" + Integer.toHexString(id._d & 0xff) + " is not described");
        }
    }

    private DynamicType hashed(XTypes.TypeIdentifier id) throws Missing {
        byte[] encoded = encodeIdentifier(id);
        String key = HexBytes.encode(encoded);
        DynamicType known = built.get(key);
        if (known != null) {
            return known;
        }
        byte[] bytes = objects.apply(encoded);
        if (bytes == null) {
            throw new Missing(encoded);
        }
        XTypes.TypeObject object = decodeObject(bytes);
        if (object._d != XTypes.EK_COMPLETE || object.complete == null) {
            throw new IllegalArgumentException("the object of a COMPLETE hash is not COMPLETE");
        }
        return complete(key, object.complete);
    }

    /** An {@code octet} of the codec (a Java {@code byte}) as the unsigned value it is. */
    private static int octet(byte value) {
        return value & 0xff;
    }

    private static int bound(long value) {
        if (value < 0 || value > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("a bound of " + value + " is not described");
        }
        return (int) value;
    }

    private DynamicType sequence(DynamicType element, long bound) {
        return bound == 0 ? factory.createSequenceType(element)
                : factory.createSequenceType(element, bound(bound));
    }

    private DynamicType named(TypeKind kind, String name) {
        return factory.createType(factory.newTypeDescriptor().withKind(kind).withName(name));
    }

    private AnnotationDescriptor mark(String name, String value) {
        TypeDescriptor d = factory.newTypeDescriptor().withKind(TypeKind.ANNOTATION_TYPE).withName(name);
        AnnotationDescriptor a = factory.newAnnotationDescriptor().withType(factory.createType(d));
        return value == null ? a : a.withValue("value", value);
    }

    private void extensibility(DynamicType type, int flags) {
        String kind = (flags & XTypes.TypeFlag.IS_FINAL) != 0 ? "FINAL"
                : (flags & XTypes.TypeFlag.IS_MUTABLE) != 0 ? "MUTABLE" : "APPENDABLE";
        type.setAnnotation(mark("Extensibility", kind));
    }

    private void flags(DynamicTypeMember member, int flags) {
        List<AnnotationDescriptor> marks = new ArrayList<AnnotationDescriptor>();
        if ((flags & XTypes.MemberFlag.IS_KEY) != 0) {
            marks.add(mark("Key", null));
        }
        if ((flags & XTypes.MemberFlag.IS_OPTIONAL) != 0) {
            marks.add(mark("Optional", null));
        }
        if ((flags & XTypes.MemberFlag.IS_MUST_UNDERSTAND) != 0) {
            marks.add(mark("MustUnderstand", null));
        }
        if (!marks.isEmpty()) {
            member.setAnnotations(marks.toArray(new AnnotationDescriptor[0]));
        }
    }

    private static String name(XTypes.CompleteTypeDetail detail) {
        return detail == null ? null : detail.type_name;
    }

    // ---- objects --------------------------------------------------------------------------

    private DynamicType complete(String key, XTypes.CompleteTypeObject object) throws Missing {
        switch (object._d) {
            case XTypes.TK_STRUCTURE: {
                XTypes.CompleteStructType s = object.struct_type;
                TypeDescriptor d = factory.newTypeDescriptor().withKind(TypeKind.STRUCTURE_TYPE)
                        .withName(name(s.header.detail));
                if (s.header.base_type != null && s.header.base_type._d != XTypes.TK_NONE) {
                    d = d.withBaseType(type(s.header.base_type));
                }
                DynamicType t = factory.createType(d);
                extensibility(t, s.struct_flags);
                built.put(key, t);
                for (XTypes.CompleteStructMember m : s.member_seq) {
                    MemberDescriptor md = factory.newMemberDescriptor().withName(m.detail.name)
                            .withId((int) m.common.member_id).withType(type(m.common.member_type_id));
                    flags(t.addMember(md), m.common.member_flags);
                }
                return t;
            }
            case XTypes.TK_UNION: {
                XTypes.CompleteUnionType u = object.union_type;
                DynamicType t = factory.createType(factory.newTypeDescriptor()
                        .withKind(TypeKind.UNION_TYPE).withName(name(u.header.detail))
                        .withDiscriminatorType(type(u.discriminator.common.type_id)));
                extensibility(t, u.union_flags);
                built.put(key, t);
                for (XTypes.CompleteUnionMember m : u.member_seq) {
                    MemberDescriptor md = factory.newMemberDescriptor().withName(m.detail.name)
                            .withId((int) m.common.member_id).withType(type(m.common.type_id))
                            .withLabel(m.common.label_seq.clone());
                    if ((m.common.member_flags & XTypes.MemberFlag.IS_DEFAULT) != 0) {
                        md = md.withDefaultLabel(true);
                    }
                    flags(t.addMember(md), m.common.member_flags);
                }
                return t;
            }
            case XTypes.TK_ENUM: {
                XTypes.CompleteEnumeratedType e = object.enumerated_type;
                DynamicType t = factory.createType(factory.newTypeDescriptor()
                        .withKind(TypeKind.ENUMERATION_TYPE).withName(name(e.header.detail)));
                extensibility(t, e.enum_flags);
                for (XTypes.CompleteEnumeratedLiteral l : e.literal_seq) {
                    t.addMember(factory.newMemberDescriptor().withName(l.detail.name).withId(l.common.value));
                }
                built.put(key, t);
                return t;
            }
            case XTypes.TK_BITMASK: {
                XTypes.CompleteBitmaskType b = object.bitmask_type;
                DynamicType t = factory.createType(factory.newTypeDescriptor()
                        .withKind(TypeKind.BITSET_TYPE).withName("Bitmask")
                        .withBound(b.header.common.bit_bound));
                built.put(key, t);
                return t;
            }
            case XTypes.TK_BITSET: {
                int bits = 0;
                for (XTypes.CompleteBitfield f : object.bitset_type.field_seq) {
                    bits = Math.max(bits, f.common.position + octet(f.common.bitcount));
                }
                DynamicType t = factory.createBitSetType(bits);
                built.put(key, t);
                return t;
            }
            case XTypes.TK_ALIAS: {
                XTypes.CompleteAliasType a = object.alias_type;
                DynamicType t = factory.createType(factory.newTypeDescriptor()
                        .withKind(TypeKind.ALIAS_TYPE).withName(name(a.header.detail))
                        .withBaseType(type(a.body.common.related_type)));
                built.put(key, t);
                return t;
            }
            case XTypes.TK_SEQUENCE: {
                XTypes.CompleteSequenceType s = object.sequence_type;
                DynamicType t = sequence(type(s.element.common.type), s.header.common.bound);
                built.put(key, t);
                return t;
            }
            case XTypes.TK_ARRAY: {
                XTypes.CompleteArrayType a = object.array_type;
                int[] bounds = new int[a.header.common.bound_seq.length];
                for (int i = 0; i < bounds.length; i++) {
                    bounds[i] = bound(a.header.common.bound_seq[i]);
                }
                DynamicType t = factory.createArrayType(type(a.element.common.type), bounds);
                built.put(key, t);
                return t;
            }
            case XTypes.TK_MAP: {
                XTypes.CompleteMapType m = object.map_type;
                DynamicType t = factory.createMapType(type(m.key.common.type),
                        type(m.element.common.type), bound(m.header.common.bound));
                built.put(key, t);
                return t;
            }
            default:
                throw new IllegalArgumentException(
                        "a TypeObject of kind 0x" + Integer.toHexString(object._d & 0xff) + " is not described");
        }
    }
}
