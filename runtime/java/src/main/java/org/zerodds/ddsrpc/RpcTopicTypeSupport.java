// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import java.nio.ByteBuffer;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;

/**
 * The type support of a DDS-RPC topic as the DDS Java PSM sees it when the application holds
 * the topic's DataWriter or DataReader typed by the topic type (DDS-RPC 1.0 §7.11.2,
 * {@code Requester.getRequestDataWriter()} and the other three): a value of the topic type is
 * the sample {@link TopicSample} frames, and the topic is announced exactly as
 * {@link RawTypeSupport} announces it — the type name, the XTypes description, the
 * extensibility.
 *
 * <p>The requester and the replier keep sending and reading the octets they frame themselves:
 * a writer over this support takes them as they are ({@code DataWriterImpl.writeSerializedIdentified})
 * and a reader keeps them beside the value ({@link #retainsSerialized()},
 * {@code DataReaderImpl.takeSerialized}). A sample the codec of the topic type cannot read —
 * a request for an operation it does not know, a reply it was not made for — is held with no
 * value, so that the requester or replier still sees its octets and answers or reports it as
 * §7.5.2 says; the application reading the typed reader sees {@code getData()} return
 * {@code null} for it.
 *
 * @param <T> the topic type
 */
public final class RpcTopicTypeSupport<T> implements TopicTypeSupport<T> {

    private final RawTypeSupport announced;
    private final TopicSample<T> sample;

    /**
     * @param announced what the topic is announced as
     * @param sample the framing of a value of the topic type
     */
    public RpcTopicTypeSupport(RawTypeSupport announced, TopicSample<T> sample) {
        this.announced = java.util.Objects.requireNonNull(announced, "announced");
        this.sample = java.util.Objects.requireNonNull(sample, "sample");
    }

    /** The topic type. */
    public Class<T> type() {
        return sample.type();
    }

    @Override
    public String getTypeName() {
        return announced.getTypeName();
    }

    @Override
    public boolean isKeyed() {
        return false;
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        return announced.getExtensibility();
    }

    @Override
    public int writeRepresentation() {
        return announced.writeRepresentation();
    }

    @Override
    public int[] readRepresentations() {
        return announced.readRepresentations();
    }

    @Override
    public byte[] typeObject() {
        return announced.typeObject();
    }

    @Override
    public byte[] typeInformation() {
        return announced.typeInformation();
    }

    @Override
    public org.zerodds.cdr.TypeObjectRegistry typeObjects() {
        return announced.typeObjects();
    }

    /** As the announced topic: a described type pairs by its TypeInformation (§7.5.1.2.6). */
    @Override
    public boolean matchesByTypeInformation() {
        return announced.matchesByTypeInformation();
    }

    @Override
    public boolean retainsSerialized() {
        return true;
    }

    @Override
    public byte[] encode(T value) {
        return sample.encode(value);
    }

    @Override
    public byte[] encode(T value, EndianMode endian) {
        // The octets carry the RPC framing of their own WireEncoding, as RawTypeSupport's do.
        return sample.encode(value);
    }

    @Override
    public byte[] encode(T value, EndianMode endian, int representation) {
        return sample.encode(value);
    }

    @Override
    public T decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    public T decode(byte[] bytes, int offset, int length) {
        try {
            return sample.decode(java.util.Arrays.copyOfRange(bytes, offset, offset + length));
        } catch (RuntimeException unreadable) {
            return null;
        }
    }

    @Override
    public T decode(byte[] bytes, int offset, int length, EndianMode endian, int representation) {
        return decode(bytes, offset, length);
    }

    @Override
    public byte[] keyHash(T value) {
        return new byte[16];
    }

    @Override
    public void serialize(T value, ByteBuffer buf) {
        buf.put(sample.encode(value));
    }

    @Override
    public T deserialize(ByteBuffer buf) {
        byte[] octets = new byte[buf.remaining()];
        buf.get(octets);
        return decode(octets, 0, octets.length);
    }
}
