// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.List;

import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.TypeObjectRegistry;

/**
 * A codec under the name its type was registered with (DDS-Java-PSM 1.0 §7.5.1:
 * {@code TypeSupport.newTypeSupport(type, registeredName, env)}). Everything but the type
 * name is the codec's own, so a topic, its writers and its readers, and the SEDP announcement
 * of both, all use the one name the application chose.
 */
public final class RegisteredNameTypeSupport<T> implements TopicTypeSupport<T> {

    private final TopicTypeSupport<T> codec;
    private final String typeName;

    private RegisteredNameTypeSupport(TopicTypeSupport<T> codec, String typeName) {
        this.codec = codec;
        this.typeName = typeName;
    }

    /** {@code codec} under {@code typeName}; the codec itself when it already has that name. */
    public static <T> TopicTypeSupport<T> named(TopicTypeSupport<T> codec, String typeName) {
        if (typeName.equals(codec.getTypeName())) {
            return codec;
        }
        return new RegisteredNameTypeSupport<>(codec, typeName);
    }

    /** The codec under its registered name. */
    public TopicTypeSupport<T> unwrap() {
        return codec;
    }

    @Override
    public String getTypeName() {
        return typeName;
    }

    @Override
    public boolean isKeyed() {
        return codec.isKeyed();
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        return codec.getExtensibility();
    }

    @Override
    public int writeRepresentation() {
        return codec.writeRepresentation();
    }

    @Override
    public int[] readRepresentations() {
        return codec.readRepresentations();
    }

    @Override
    public byte[] encode(T sample, EndianMode endian, int representation) {
        return codec.encode(sample, endian, representation);
    }

    @Override
    public T decode(byte[] bytes, int offset, int length, EndianMode endian, int representation) {
        return codec.decode(bytes, offset, length, endian, representation);
    }

    @Override
    public byte[] typeObject() {
        return codec.typeObject();
    }

    @Override
    public byte[] typeInformation() {
        return codec.typeInformation();
    }

    @Override
    public boolean matchesByTypeInformation() {
        return codec.matchesByTypeInformation();
    }

    @Override
    public List<String> baseTypeNames() {
        return codec.baseTypeNames();
    }

    @Override
    public List<String> equivalentTypeNames() {
        return codec.equivalentTypeNames();
    }

    @Override
    public TypeObjectRegistry typeObjects() {
        return codec.typeObjects();
    }

    @Override
    public byte[] typeIdentifier() {
        return codec.typeIdentifier();
    }

    @Override
    public byte[] encode(T sample) {
        return codec.encode(sample);
    }

    @Override
    public byte[] encode(T sample, EndianMode endian) {
        return codec.encode(sample, endian);
    }

    @Override
    public T decode(byte[] bytes) {
        return codec.decode(bytes);
    }

    @Override
    public T decode(byte[] bytes, int offset, int length) {
        return codec.decode(bytes, offset, length);
    }

    @Override
    public byte[] keyHash(T sample) {
        return codec.keyHash(sample);
    }

    @Override
    public void serialize(T value, java.nio.ByteBuffer buf) {
        codec.serialize(value, buf);
    }

    @Override
    public T deserialize(java.nio.ByteBuffer buf) {
        return codec.deserialize(buf);
    }
}
