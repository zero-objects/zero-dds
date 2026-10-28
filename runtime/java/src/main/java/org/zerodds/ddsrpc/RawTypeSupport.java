// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.nio.ByteBuffer;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;

/**
 * The type support of a DDS-RPC topic as the DDS Java PSM binding sees it: the
 * sample is its own octets.
 *
 * <p>DDS-RPC 1.0 §7.5.1.1.6 and §7.5.1.1.7 give the topic types of an
 * interface the names {@code ${interfaceName}_Request} and
 * {@code ${interfaceName}_Reply}, and the structure behind each is framed
 * by the RPC runtime (header, padding, union), so the DDS layer carries
 * the octets under the type name the interface has. The built-in
 * {@code ByteArrayTypeSupport} announces {@code DDS::Bytes} and frames every
 * sample with a DHEADER, which would make every interface's request topic one
 * type and put a second header in front of the RPC one.
 */
public final class RawTypeSupport implements TopicTypeSupport<byte[]> {

    private final String typeName;
    private final org.zerodds.ddsrpc.TypeDescription description;

    /**
     * @param typeName the DDS type name the topic is announced under
     */
    public RawTypeSupport(String typeName) {
        this(typeName, null);
    }

    /**
     * @param typeName the DDS type name the topic is announced under
     * @param description the XTypes description of the type, announced in discovery and served
     *     to peers, or {@code null} for none (the type is then matched by its name)
     */
    public RawTypeSupport(String typeName, org.zerodds.ddsrpc.TypeDescription description) {
        if (typeName == null || typeName.isEmpty()) {
            throw new IllegalArgumentException("a topic type has a name");
        }
        this.typeName = typeName;
        this.description = description;
    }

    @Override
    public byte[] typeObject() {
        return description == null ? new byte[0] : description.object();
    }

    @Override
    public byte[] typeInformation() {
        return description == null ? new byte[0] : description.information();
    }

    /**
     * A described type pairs by the assignability of its TypeInformation and the peer's
     * (DDS-RPC 1.0 §7.5.1.2.6, XTypes 1.3 §7.6.3.4.2): the request type of a client of a base
     * interface has another name than the one of a service of a derived interface (§7.4.3), and
     * a changed parameter or return type keeps the name (§7.7.2.5.3, §7.7.2.5.5).
     */
    @Override
    public boolean matchesByTypeInformation() {
        return description != null && description.information().length > 0;
    }

    @Override
    public org.zerodds.cdr.TypeObjectRegistry typeObjects() {
        return description == null
                ? new org.zerodds.cdr.TypeObjectRegistry(new org.zerodds.cdr.TypeObjectRegistry.Entry[0])
                : description.registry();
    }

    @Override
    public String getTypeName() {
        return typeName;
    }

    @Override
    public boolean isKeyed() {
        return false;
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        // A described type is a topic type of the Enhanced mapping: a @Choice structure,
        // which §7.5.1.2.1.1 reads as @mutable, so its samples are PL_CDR2 (XTypes 1.3
        // Table 59). The octets of the Basic mapping are final.
        return description == null ? ExtensibilityKind.FINAL : ExtensibilityKind.MUTABLE;
    }

    @Override
    public byte[] encode(byte[] sample) {
        return sample.clone();
    }

    @Override
    public byte[] encode(byte[] sample, EndianMode endian) {
        return sample.clone();
    }

    @Override
    public byte[] decode(byte[] bytes) {
        return bytes.clone();
    }

    @Override
    public byte[] decode(byte[] bytes, int offset, int length) {
        return java.util.Arrays.copyOfRange(bytes, offset, offset + length);
    }

    @Override
    public byte[] keyHash(byte[] sample) {
        return new byte[16];
    }

    @Override
    public void serialize(byte[] value, ByteBuffer buf) {
        buf.put(value);
    }

    @Override
    public byte[] deserialize(ByteBuffer buf) {
        byte[] out = new byte[buf.remaining()];
        buf.get(out);
        return out;
    }
}
