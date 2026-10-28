package org.zerodds.typeobject;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

import org.zerodds.cdr.XcdrException;
import org.zerodds.typeobject.generated.ZdCodec;
import org.zerodds.typeobject.generated.ZdTypes;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.PidFlagPolicy;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.TryConstruct;
import org.zerodds.wire.Writer;

/**
 * The serialization of the XTypes 1.1 TypeObject (Annex B) and the TypeId of a constructed
 * type (XTypes 1.1 7.3.4.1.2). The data classes and the encoders are generated from
 * {@code idl/typeobject/dds_xtypes_11_typeobject.idl} by {@code scripts/generate-typeobject.sh}.
 *
 * <p>The value of the parameter PID_TYPE (0x0072) is the TypeObject in PL_CDR (XCDR1,
 * little-endian), without an encapsulation header; its members carry the member ids of Annex B.
 */
public final class TypeObjectCodec {

    private static PlCdr1Framing framing() {
        return new PlCdr1Framing(PidFlagPolicy.SENTINEL_MUST_UNDERSTAND, 16128, true, true, true, true);
    }

    private TypeObjectCodec() {
    }

    /** The PID_TYPE value for {@code object}. */
    public static byte[] encode(ZdTypes.DDS.TypeObject object) {
        Writer w = new Writer(Endianness.LITTLE, 8, framing(), false);
        ZdCodec.zd_enc_DDS_TypeObject(w, object);
        return w.finish();
    }

    /**
     * The TypeObject in a PID_TYPE value.
     *
     * @throws XcdrException if the bytes are not a complete TypeObject
     */
    public static ZdTypes.DDS.TypeObject decode(byte[] bytes) {
        if (bytes == null) {
            throw new XcdrException("no TypeObject bytes");
        }
        Reader r = new Reader(bytes.clone(), Endianness.LITTLE, 8, framing());
        ZdTypes.DDS.TypeObject object = ZdCodec.zd_dec_DDS_TypeObject(r, TryConstruct.DISCARD);
        if (r.readerStatus() != Reader.Status.OK || r.remaining() != 0) {
            throw new XcdrException("the bytes are not a complete XTypes 1.1 TypeObject");
        }
        return object;
    }

    /**
     * The id of a constructed type (XTypes 1.1 7.3.4.1.2): the less significant 64 bits of the
     * MD5 hash of the big-endian CDR serialization of the type element. The standard leaves
     * open what the own {@code property.type_id} holds while hashing; it is {@code NO_TYPE}
     * (discriminator 0, id 0) here. The less significant half is the last eight octets of the
     * digest, read big-endian.
     */
    public static long constructedTypeId(ZdTypes.DDS.TypeLibraryElement element) {
        Writer w = new Writer(Endianness.BIG, 8, framing(), false);
        switch (element._d) {
            case ZdTypes.DDS.TypeLibraryElementKind.ALIAS_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_AliasType(w, element.alias_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.ANNOTATION_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_AnnotationType(w, element.annotation_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.ARRAY_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_ArrayType(w, element.array_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.BITSET_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_BitSetType(w, element.bitset_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.ENUMERATION_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_EnumerationType(w, element.enumeration_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.MAP_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_MapType(w, element.map_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.SEQUENCE_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_SequenceType(w, element.sequence_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.STRING_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_StringType(w, element.string_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.STRUCTURE_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_StructureType(w, element.structure_type);
                break;
            case ZdTypes.DDS.TypeLibraryElementKind.UNION_TYPE_ELEMENT:
                ZdCodec.zd_enc_DDS_UnionType(w, element.union_type);
                break;
            default:
                throw new XcdrException("a module has no TypeId");
        }
        byte[] digest;
        try {
            digest = MessageDigest.getInstance("MD5").digest(w.finish());
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("MD5 is required of every Java platform", e);
        }
        long id = 0;
        for (int i = 8; i < 16; i++) {
            id = (id << 8) | (digest[i] & 0xFF);
        }
        return id;
    }

    /** The TypeId of a primitive type. */
    public static ZdTypes.DDS.TypeId primitive(short kind) {
        ZdTypes.DDS.TypeId id = new ZdTypes.DDS.TypeId();
        id._d = kind;
        id.primitive_type_id = kind;
        return id;
    }

    /** The TypeId of a constructed type of {@code kind}. */
    public static ZdTypes.DDS.TypeId constructed(short kind, long value) {
        ZdTypes.DDS.TypeId id = new ZdTypes.DDS.TypeId();
        id._d = kind;
        id.constructed_type_id = value;
        return id;
    }

    /** Whether {@code kind} is one of the primitive kinds (BOOLEAN_TYPE to CHAR_32_TYPE). */
    public static boolean isPrimitive(short kind) {
        return kind >= ZdTypes.DDS.BOOLEAN_TYPE && kind <= ZdTypes.DDS.CHAR_32_TYPE;
    }

    /** The property every type element carries; its {@code type_id} is the id of the element. */
    public static ZdTypes.DDS.TypeProperty propertyOf(ZdTypes.DDS.TypeLibraryElement e) {
        switch (e._d) {
            case ZdTypes.DDS.TypeLibraryElementKind.ALIAS_TYPE_ELEMENT:
                return e.alias_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.ANNOTATION_TYPE_ELEMENT:
                return e.annotation_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.ARRAY_TYPE_ELEMENT:
                return e.array_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.BITSET_TYPE_ELEMENT:
                return e.bitset_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.ENUMERATION_TYPE_ELEMENT:
                return e.enumeration_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.MAP_TYPE_ELEMENT:
                return e.map_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.SEQUENCE_TYPE_ELEMENT:
                return e.sequence_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.STRING_TYPE_ELEMENT:
                return e.string_type.property;
            case ZdTypes.DDS.TypeLibraryElementKind.STRUCTURE_TYPE_ELEMENT:
                return e.structure_type.property;
            default:
                return e.union_type.property;
        }
    }
}
