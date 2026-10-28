// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.ByteBuffer;

/**
 * ZeroDDS-extended TypeSupport interface for XCDR2 bindings.
 *
 * <p>Spec: zerodds-xcdr2-java-1.0 §2. The DDS-Java-PSM 1.0 §7.5.1 TypeSupport trait extended with
 * concrete encode/decode/keyHash methods and extensibility reflection.
 *
 * <p>This is the runtime adapter contract of the ZeroDDS wire runtime. Codec-only generated classes do
 * not automatically implement it or publish discovery TypeInformation.
 *
 * @param <T> sample class (POJO with bean accessors).
 */
public interface TopicTypeSupport<T> {

    /** DDS-Type-Name (Convention {@code Module::Sub::Struct}, ASCII, max 256 Bytes). */
    String getTypeName();

    /** {@code true} if at least one member carries {@code @key}. */
    boolean isKeyed();

    /** Extensibility (Final/Appendable/Mutable) per OMG XTypes 1.3 §7.2.2.4.4. */
    ExtensibilityKind getExtensibility();

    int XCDR1 = 0;
    int XCDR2 = 2;

    /** Representation used by the writer and the in-process codec bridge. */
    default int writeRepresentation() { return XCDR2; }

    /** Representations actually implemented by this descriptor, not vendor capabilities. */
    default int[] readRepresentations() { return new int[] {XCDR2}; }

    /** Explicit representation boundary for generated codec adapters. */
    default byte[] encode(T sample, EndianMode endian, int representation) {
        if (representation != XCDR2) {
            throw new XcdrException("Descriptor has no encoder for representation " + representation);
        }
        return encode(sample, endian);
    }

    /**
     * Legacy decoders are little-endian XCDR2 only. An adapter must override
     * this method to claim another representation or byte order.
     */
    default T decode(byte[] bytes, int offset, int length, EndianMode endian, int representation) {
        if (representation != XCDR2 || endian != EndianMode.LITTLE_ENDIAN) {
            throw new XcdrException("Descriptor has no decoder for " + representation + "/" + endian);
        }
        return decode(bytes, offset, length);
    }

    /**
     * Optional serialized COMPLETE {@code TypeObject} supplied by an adapter.
     * Empty means unavailable, not an empty type. The codec generator and
     * standard discovery TypeInformation connection require separate support.
     */
    default byte[] typeObject() {
        return new byte[0];
    }

    /**
     * Standard PID_TYPE_INFORMATION payload, XCDR2 little-endian without an
     * encapsulation header. Independent of the historical vendor hash below.
     * Empty means no standard metadata is supplied. Generated supports refuse
     * explicitly when their complete dependency closure cannot be represented.
     * Supplying this parameter does not implement or advertise TypeLookup.
     */
    default byte[] typeInformation() {
        return new byte[0];
    }

    /**
     * Whether the endpoints of this type pair with remote endpoints by the assignability of the
     * TypeInformation both announce (DDS-XTypes 1.3 7.6.3.4.2: equal identifiers, else the
     * reader's type assignable from the writer's, judged on the COMPLETE TypeObjects, fetched
     * from the peer's TypeLookup service when needed; the type names only where a side announced
     * no TypeInformation), and not by the type name. The topic types of the DDS-RPC Enhanced
     * Service Mapping do (DDS-RPC 1.0 7.5.1.2.6): a client of a base interface pairs with a
     * service of a derived one, whose request type has another name. The default is
     * {@code false}: the topics of the PSM match the type name, or what their
     * TypeConsistencyEnforcement accepts of an announced TypeObject (PID 0x0072).
     */
    default boolean matchesByTypeInformation() {
        return false;
    }

    /**
     * The names of the base types of the topic type, nearest first (DDS-Java-PSM 1.0
     * {@code baseTypeName} of the built-in topic data). Empty for a type without a
     * base type.
     */
    default java.util.List<String> baseTypeNames() {
        return java.util.Collections.emptyList();
    }

    /**
     * The names of types equivalent to the topic type ({@code equivalentTypeName} of the
     * built-in topic data). A Java type states no equivalent names, so the default is empty.
     */
    default java.util.List<String> equivalentTypeNames() {
        return java.util.Collections.emptyList();
    }

    /** Source-derived minimal and complete dependency closure. No service capability implied. */
    default TypeObjectRegistry typeObjects() {
        return new TypeObjectRegistry(new TypeObjectRegistry.Entry[0]);
    }

    /**
     * First 14 bytes of the MD5 of {@link #typeObject()} (EquivalenceHash).
     * This is the historical vendor hash contract, not a serialized standard
     * TypeIdentifier or TypeInformation parameter. Empty when unavailable.
     */
    default byte[] typeIdentifier() {
        byte[] to = typeObject();
        if (to.length == 0) {
            return new byte[0];
        }
        byte[] md5 = Md5.hash(to);
        byte[] id = new byte[14];
        System.arraycopy(md5, 0, id, 0, 14);
        return id;
    }

    /** Encode with default endianness (LE). */
    /**
     * Whether a reader keeps the octets each sample arrived as beside the value it decoded from
     * them ({@code SampleImpl.serialized()}). The DDS-RPC 1.0 binding (§7.11.2) reads requests
     * and replies from their octets and hands the application the same reader typed by the
     * topic type; every other type support has no use for them.
     */
    default boolean retainsSerialized() {
        return false;
    }

    byte[] encode(T sample);

    /** Encode with the chosen endianness. */
    byte[] encode(T sample, EndianMode endian);

    /** Decode from the full buffer. */
    T decode(byte[] bytes);

    /** Decode aus Subrange. */
    T decode(byte[] bytes, int offset, int length);

    /**
     * Key hash computation: 16 bytes MD5 over {@code PlainCdr2BeKeyHolder}
     * of the {@code @key} fields (XTypes §7.6.8). Returns all-zero if
     * {@link #isKeyed()} {@code false}.
     */
    default byte[] keyHash(T sample) {
        return new byte[0];
    }

    /**
     * Overwrites the key fields of {@code into} with those of {@code from} and
     * no other field (DDS 1.4 §2.2.2.5.3.29 get_key_value). The default finds
     * the key fields by their {@code @Key} annotation.
     */
    default void copyKey(T from, T into) {
        KeyFields.copy(from, into);
    }

    /**
     * The result of {@code get_key_value(keyHolder, handle)} (DDS 1.4 §2.2.2.5.3.29): {@code holder}
     * with its key fields overwritten, or, when the key fields of its class are final (a record),
     * {@code from} itself, which has the key set.
     */
    default T keyValue(T from, T holder) {
        if (KeyFields.overwritable(holder)) {
            copyKey(from, holder);
            return holder;
        }
        return from;
    }

    // ------------------------------------------------------------------
    // DDS-Java-PSM 1.0 §7.5.1 TypeSupport bridge
    // ------------------------------------------------------------------

    /**
     * Serialize via the encode path into {@code buf}; the position grows by
     * the serialized size.
     */
    default void serialize(T value, ByteBuffer buf) {
        byte[] bytes = encode(value, EndianMode.LITTLE_ENDIAN, writeRepresentation());
        buf.put(bytes);
    }

    /**
     * Deserialize via the decode path. Reads all remaining bytes of the
     * buffer; callers that need subranges call {@link #decode(byte[], int, int)}
     * directly.
     */
    default T deserialize(ByteBuffer buf) {
        int len = buf.remaining();
        byte[] tmp = new byte[len];
        buf.get(tmp);
        return decode(tmp, 0, tmp.length, EndianMode.LITTLE_ENDIAN, writeRepresentation());
    }
}
