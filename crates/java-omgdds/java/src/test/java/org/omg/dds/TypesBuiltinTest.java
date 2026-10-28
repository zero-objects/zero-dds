// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.builtin.KeyedBytes;
import org.omg.dds.type.builtin.KeyedString;
import org.zerodds.cdr.BuiltinTypeSupports;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportResolver;
import org.zerodds.cdr.XcdrException;
import org.zerodds.internal.InProcessBus;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.8.2: the built-in types of DDS-XTypes. {@code DDS::String} is
 * {@code java.lang.String}, {@code DDS::KeyedString} and {@code DDS::KeyedBytes} are the
 * modifiable value types {@link KeyedString} and {@link KeyedBytes}; each is a topic type
 * that a generic writer and reader move. XTypes 1.3 Annex E defines all three as
 * {@code @extensibility(APPENDABLE)}, so a sample is a 4-byte DHEADER with the size of
 * the members, then the members; a {@code string} is a 4-byte length (octets plus the
 * terminating NUL), the octets and the NUL, and a {@code ByteSeq} is a 4-byte length and
 * the octets. Little-endian, expected octets derived by hand from those clauses.
 *
 * <p>Domain ids 1600 to 1609.
 */
class TypesBuiltinTest {

    private static final ServiceEnvironment ENV = Qos.env();

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    private static KeyedString keyedString(String key, String value) {
        return KeyedString.newKeyedString(ENV).setKey(key).setValue(value);
    }

    private static KeyedBytes keyedBytes(String key, byte... value) {
        return KeyedBytes.newKeyedBytes(ENV).setKey(key).setValue(value);
    }

    @SuppressWarnings("unchecked")
    private static <T> T javaSerializationRoundTrip(T value) throws Exception {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (ObjectOutputStream stream = new ObjectOutputStream(out)) {
            stream.writeObject(value);
        }
        try (ObjectInputStream in = new ObjectInputStream(new ByteArrayInputStream(out.toByteArray()))) {
            return (T) in.readObject();
        }
    }

    // --- DDS::String ---------------------------------------------------------

    @Test
    void stringIsTheBuiltInTypeDdsString() {
        TopicTypeSupport<String> support = TypeSupportResolver.resolve(String.class);
        assertEquals("DDS::String", support.getTypeName());
        assertFalse(support.isKeyed());
        assertEquals(ExtensibilityKind.APPENDABLE, support.getExtensibility());
        // DHEADER 7 (length word 4 + 'h' 'i' NUL 3), length 3, "hi" and the NUL.
        assertArrayEquals(bytes(7, 0, 0, 0, 3, 0, 0, 0, 'h', 'i', 0), support.encode("hi"));
        assertEquals("hi", support.decode(bytes(7, 0, 0, 0, 3, 0, 0, 0, 'h', 'i', 0)));
        // Big-endian stream: the same members with their words reversed.
        assertArrayEquals(bytes(0, 0, 0, 7, 0, 0, 0, 3, 'h', 'i', 0),
                support.encode("hi", EndianMode.BIG_ENDIAN));
        // The empty string is the length 1 and the NUL: DHEADER 5.
        assertArrayEquals(bytes(5, 0, 0, 0, 1, 0, 0, 0, 0), support.encode(""));
        assertEquals("", support.decode(support.encode("")));
    }

    @Test
    void aStringTopicIsDdsStringAndMovesSamples() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1600);
        Topic<String> topic = participant.createTopic("TypesStringTopic", String.class);
        assertEquals("DDS::String", topic.getTypeName());
        DataWriter<String> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<String> reader = participant.createSubscriber().createDataReader(topic);

        writer.write("hello");

        List<Sample<String>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertEquals("hello", samples.get(0).getData());
    }

    @Test
    void aTruncatedOrInconsistentStringIsRefused() {
        TopicTypeSupport<String> support = TypeSupportResolver.resolve(String.class);
        assertThrows(Exception.class, () -> support.decode(bytes(7, 0, 0)));
        // The DHEADER announces more octets than the buffer holds.
        assertThrows(Exception.class, () -> support.decode(bytes(9, 0, 0, 0, 3, 0, 0, 0, 'h', 'i', 0)));
        assertThrows(XcdrException.class, () -> support.encode(null));
    }

    // --- DDS::KeyedString: the value type -----------------------------------

    @Test
    void keyedStringIsAModifiableValueType() {
        KeyedString a = ENV.getSPI().newKeyedString();
        assertNotNull(a);
        assertSame(ENV, a.getEnvironment());
        assertEquals("", a.getKey());
        assertEquals("", a.getValue());

        // setKey and setValue return the object, so that calls chain.
        assertSame(a, a.setKey("k1"));
        assertSame(a, a.setValue("v1"));
        assertEquals("k1", a.getKey());
        assertEquals("v1", a.getValue());

        // A CharSequence that is not a String is copied to a String.
        a.setKey(new StringBuilder("sb"));
        assertEquals("sb", a.getKey());
        a.setKey(null);
        assertEquals("", a.getKey());
    }

    @Test
    void keyedStringCopyFromCloneAndEquals() {
        KeyedString a = keyedString("k", "v");
        KeyedString b = ENV.getSPI().newKeyedString();
        assertNotEquals(a, b);

        b.copyFrom(a);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        // copyFrom copies the contents: a later change of the source does not reach the copy.
        a.setValue("changed");
        assertEquals("v", b.getValue());
        assertNotEquals(a, b);

        KeyedString c = a.clone();
        assertNotSame(a, c);
        assertEquals(a, c);
        assertEquals(a.hashCode(), c.hashCode());
        c.setKey("other");
        assertEquals("k", a.getKey());
        assertNotEquals(a, c);
        assertNotEquals(a, "k");
    }

    @Test
    void keyedStringSurvivesJavaSerialization() throws Exception {
        KeyedString a = keyedString("key", "value");
        KeyedString back = javaSerializationRoundTrip(a);
        assertNotSame(a, back);
        assertEquals(a, back);
        assertNotNull(back.getEnvironment(), "a deserialized value belongs to the default environment");
    }

    // --- DDS::KeyedString: the topic type -----------------------------------

    @Test
    void keyedStringIsTheBuiltInTypeDdsKeyedString() {
        TopicTypeSupport<KeyedString> support = TypeSupportResolver.resolve(KeyedString.class);
        assertSame(BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE, support);
        assertEquals("DDS::KeyedString", support.getTypeName());
        assertTrue(support.isKeyed());
        assertEquals(ExtensibilityKind.APPENDABLE, support.getExtensibility());
        // The implementation class resolves to the same support.
        assertSame(support, TypeSupportResolver.resolve(keyedString("k", "v").getClass()));

        // key "k": length 2, 'k', NUL (offsets 4 to 9); the next string's length is
        // 4-aligned, so two padding octets (10, 11); value "vv": length 3, 'v' 'v' NUL
        // (12 to 18). Members occupy offsets 4 to 18, 15 octets: DHEADER 15.
        byte[] wire = bytes(15, 0, 0, 0, 2, 0, 0, 0, 'k', 0, 0, 0, 3, 0, 0, 0, 'v', 'v', 0);
        assertArrayEquals(wire, support.encode(keyedString("k", "vv")));
        KeyedString back = support.decode(wire);
        assertEquals("k", back.getKey());
        assertEquals("vv", back.getValue());
    }

    @Test
    void theKeyHashOfAKeyedStringIsTheMd5OfItsKeyInBigEndian() {
        TopicTypeSupport<KeyedString> support = TypeSupportResolver.resolve(KeyedString.class);
        // XTypes 1.3 §7.6.8: the key holder is the key member, big-endian: length 2, 'k',
        // NUL = 00 00 00 02 6b 00. A string has no maximum size, so the hash is the MD5 of
        // the holder (DDSI-RTPS 2.5 §9.6.3.8), and never the holder padded to 16 octets.
        // MD5(00 00 00 02 6b 00) = 302cf0a69dae0d4732bd21ea86fec48d (hashlib.md5).
        assertArrayEquals(bytes(0x30, 0x2c, 0xf0, 0xa6, 0x9d, 0xae, 0x0d, 0x47,
                0x32, 0xbd, 0x21, 0xea, 0x86, 0xfe, 0xc4, 0x8d),
                support.keyHash(keyedString("k", "ignored")));
        // The value is not part of the key.
        assertArrayEquals(support.keyHash(keyedString("k", "a")),
                support.keyHash(keyedString("k", "b")));
        assertFalse(java.util.Arrays.equals(support.keyHash(keyedString("k", "a")),
                support.keyHash(keyedString("j", "a"))));
    }

    @Test
    void aKeyedStringTopicHasOneInstancePerKey() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1601);
        Topic<KeyedString> topic = participant.createTopic("TypesKeyedStringTopic", KeyedString.class);
        assertEquals("DDS::KeyedString", topic.getTypeName());
        DataWriter<KeyedString> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<KeyedString> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(keyedString("one", "first"));
        writer.write(keyedString("two", "second"));

        List<Sample<KeyedString>> samples = Qos.take(reader);
        assertEquals(2, samples.size(), "one instance per key");
        java.util.Map<String, String> byKey = new java.util.TreeMap<>();
        for (Sample<KeyedString> sample : samples) {
            byKey.put(sample.getData().getKey(), sample.getData().getValue());
        }
        assertEquals("first", byKey.get("one"));
        assertEquals("second", byKey.get("two"));
        assertNotEquals(samples.get(0).getInstanceHandle(), samples.get(1).getInstanceHandle());
    }

    @Test
    void aKeyedStringTopicSampleIsANewValueObject() {
        DomainParticipant participant = Qos.factory().createParticipant(1602);
        Topic<KeyedString> topic = participant.createTopic("TypesKeyedStringNew", KeyedString.class);
        // TypeSupport.newData: a sample for a reader to fill, made through the SPI.
        KeyedString fresh = topic.getTypeSupport().newData();
        assertNotNull(fresh);
        assertEquals("", fresh.getKey());
        assertEquals("", fresh.getValue());
    }

    @Test
    void aTruncatedKeyedStringIsRefused() {
        TopicTypeSupport<KeyedString> support = TypeSupportResolver.resolve(KeyedString.class);
        assertThrows(Exception.class, () -> support.decode(bytes(15, 0, 0, 0, 2, 0, 0, 0, 'k')));
        assertThrows(XcdrException.class, () -> support.encode(null));
    }

    // --- DDS::KeyedBytes: the value type ------------------------------------

    @Test
    void keyedBytesIsAModifiableValueType() {
        KeyedBytes a = ENV.getSPI().newKeyedBytes();
        assertSame(ENV, a.getEnvironment());
        assertEquals("", a.getKey());
        assertArrayEquals(new byte[0], a.getValue());

        byte[] source = {1, 2, 3, 4, 5};
        assertSame(a, a.setKey("k"));
        assertSame(a, a.setValue(source));
        // The value is copied in and copied out.
        source[0] = 99;
        assertArrayEquals(new byte[] {1, 2, 3, 4, 5}, a.getValue());
        byte[] got = a.getValue();
        got[1] = 99;
        assertArrayEquals(new byte[] {1, 2, 3, 4, 5}, a.getValue());

        // setValue(value, offset, length) takes the range.
        assertSame(a, a.setValue(new byte[] {9, 8, 7, 6, 5}, 1, 3));
        assertArrayEquals(new byte[] {8, 7, 6}, a.getValue());
    }

    @Test
    void keyedBytesCopyFromCloneAndEquals() {
        KeyedBytes a = keyedBytes("k", (byte) 1, (byte) 2);
        KeyedBytes b = ENV.getSPI().newKeyedBytes();
        assertNotEquals(a, b);

        b.copyFrom(a);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        a.setValue(new byte[] {7});
        assertArrayEquals(new byte[] {1, 2}, b.getValue());
        assertNotEquals(a, b);

        KeyedBytes c = a.clone();
        assertNotSame(a, c);
        assertEquals(a, c);
        c.setValue(new byte[] {8});
        assertArrayEquals(new byte[] {7}, a.getValue());
        assertNotEquals(a, "k");
    }

    @Test
    void keyedBytesSurvivesJavaSerialization() throws Exception {
        KeyedBytes a = keyedBytes("key", (byte) 1, (byte) 2, (byte) 3);
        KeyedBytes back = javaSerializationRoundTrip(a);
        assertNotSame(a, back);
        assertEquals(a, back);
        assertNotNull(back.getEnvironment());
    }

    // --- DDS::KeyedBytes: the topic type ------------------------------------

    @Test
    void keyedBytesIsTheBuiltInTypeDdsKeyedBytes() {
        TopicTypeSupport<KeyedBytes> support = TypeSupportResolver.resolve(KeyedBytes.class);
        assertSame(BuiltinTypeSupports.KeyedBytesTypeSupport.INSTANCE, support);
        assertEquals("DDS::KeyedBytes", support.getTypeName());
        assertTrue(support.isKeyed());
        assertEquals(ExtensibilityKind.APPENDABLE, support.getExtensibility());

        // key "k" at offsets 4 to 9; the ByteSeq length is 4-aligned (padding at 10, 11),
        // 3 at 12 to 15, the octets 1 2 3 at 16 to 18: 15 octets after the DHEADER.
        byte[] wire = bytes(15, 0, 0, 0, 2, 0, 0, 0, 'k', 0, 0, 0, 3, 0, 0, 0, 1, 2, 3);
        assertArrayEquals(wire, support.encode(keyedBytes("k", (byte) 1, (byte) 2, (byte) 3)));
        KeyedBytes back = support.decode(wire);
        assertEquals("k", back.getKey());
        assertArrayEquals(new byte[] {1, 2, 3}, back.getValue());

        // An empty value is a ByteSeq of length 0: offsets 4 to 9 key, 12 to 15 length;
        // 12 octets after the DHEADER.
        assertArrayEquals(bytes(12, 0, 0, 0, 2, 0, 0, 0, 'k', 0, 0, 0, 0, 0, 0, 0),
                support.encode(keyedBytes("k")));
        assertArrayEquals(new byte[0], support.decode(support.encode(keyedBytes("k"))).getValue());
        // An octet above 0x7f is an unsigned octet on the wire and a negative byte in Java.
        byte[] high = support.encode(keyedBytes("k", (byte) 0xff));
        assertEquals((byte) 0xff, high[high.length - 1]);
        assertEquals((byte) 0xff, support.decode(high).getValue()[0]);
    }

    @Test
    void theKeyHashOfKeyedBytesIsTheMd5OfItsKey() {
        TopicTypeSupport<KeyedBytes> support = TypeSupportResolver.resolve(KeyedBytes.class);
        // The same key holder as KeyedString: 00 00 00 02 6b 00.
        assertArrayEquals(bytes(0x30, 0x2c, 0xf0, 0xa6, 0x9d, 0xae, 0x0d, 0x47,
                0x32, 0xbd, 0x21, 0xea, 0x86, 0xfe, 0xc4, 0x8d),
                support.keyHash(keyedBytes("k", (byte) 5)));
    }

    @Test
    void aKeyedBytesTopicHasOneInstancePerKey() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1603);
        Topic<KeyedBytes> topic = participant.createTopic("TypesKeyedBytesTopic", KeyedBytes.class);
        assertEquals("DDS::KeyedBytes", topic.getTypeName());
        DataWriter<KeyedBytes> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<KeyedBytes> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(keyedBytes("a", (byte) 1, (byte) 2));
        writer.write(keyedBytes("b", (byte) 3));

        List<Sample<KeyedBytes>> samples = Qos.take(reader);
        assertEquals(2, samples.size(), "one instance per key");
        java.util.Map<String, byte[]> byKey = new java.util.TreeMap<>();
        for (Sample<KeyedBytes> sample : samples) {
            byKey.put(sample.getData().getKey(), sample.getData().getValue());
        }
        assertArrayEquals(new byte[] {1, 2}, byKey.get("a"));
        assertArrayEquals(new byte[] {3}, byKey.get("b"));
    }

    @Test
    void aKeyedBytesTopicSampleIsANewValueObject() {
        DomainParticipant participant = Qos.factory().createParticipant(1604);
        Topic<KeyedBytes> topic = participant.createTopic("TypesKeyedBytesNew", KeyedBytes.class);
        KeyedBytes fresh = topic.getTypeSupport().newData();
        assertNotNull(fresh);
        assertArrayEquals(new byte[0], fresh.getValue());
    }

    @Test
    void aTruncatedKeyedBytesIsRefused() {
        TopicTypeSupport<KeyedBytes> support = TypeSupportResolver.resolve(KeyedBytes.class);
        assertThrows(Exception.class,
                () -> support.decode(bytes(15, 0, 0, 0, 2, 0, 0, 0, 'k', 0, 0, 0, 9, 0, 0, 0, 1)));
        assertThrows(XcdrException.class, () -> support.encode(null));
    }

    // --- DDS::Bytes next to them --------------------------------------------

    @Test
    void bytesAreStillTheBuiltInTypeDdsBytes() {
        TopicTypeSupport<byte[]> support = TypeSupportResolver.resolve(byte[].class);
        assertEquals("DDS::Bytes", support.getTypeName());
        DomainParticipant participant = Qos.factory().createParticipant(1605);
        Topic<byte[]> topic = participant.createTopic("TypesBytesTopic", byte[].class);
        assertEquals("DDS::Bytes", topic.getTypeName());
        byte[] fresh = topic.getTypeSupport().newData();
        assertEquals(0, fresh.length);
    }
}
