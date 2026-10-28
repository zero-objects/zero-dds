// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.builtin.KeyedBytes;
import org.omg.dds.type.builtin.KeyedString;
import org.zerodds.cdr.BuiltinTypeSupports;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.XcdrException;
import org.zerodds.internal.InProcessBus;

import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.8.2: the built-in types KeyedString and KeyedBytes are
 * topic types. XTypes 1.3 Annex E gives them as appendable structs
 * {@code @key string key; string value;} and {@code @key string key; ByteSeq value;}.
 */
class CoreBuiltinKeyedTest {

    private static final int DOMAIN = 1504;

    private DomainParticipant participant;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
    }

    @AfterEach
    void tearDown() {
        participant.close();
        InProcessBus.instance().reset();
    }

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < values.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    @Test
    void keyedStringHasTheDelimitedCdr2Layout() {
        KeyedString sample = KeyedString.newKeyedString(Qos.env()).setKey("ab").setValue("c");
        // DHEADER 14 | length 3, 'a' 'b' NUL, one octet of padding | length 2, 'c' NUL
        byte[] expected = bytes(
                0x0E, 0, 0, 0,
                0x03, 0, 0, 0, 0x61, 0x62, 0, 0,
                0x02, 0, 0, 0, 0x63, 0);
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.encode(sample));
        assertEquals(sample, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.decode(expected));
        assertEquals("DDS::KeyedString", BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.getTypeName());
        assertTrue(BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.isKeyed());
    }

    @Test
    void keyedStringInBigEndian() {
        KeyedString sample = KeyedString.newKeyedString(Qos.env()).setKey("ab").setValue("c");
        byte[] expected = bytes(
                0, 0, 0, 0x0E,
                0, 0, 0, 0x03, 0x61, 0x62, 0, 0,
                0, 0, 0, 0x02, 0x63, 0);
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.encode(sample, EndianMode.BIG_ENDIAN));
    }

    @Test
    void keyedBytesHasTheDelimitedCdr2Layout() {
        KeyedBytes sample = KeyedBytes.newKeyedBytes(Qos.env()).setKey("k").setValue(bytes(1, 2, 3));
        // DHEADER 15 | length 2, 'k' NUL, two octets of padding | sequence length 3, 1 2 3
        byte[] expected = bytes(
                0x0F, 0, 0, 0,
                0x02, 0, 0, 0, 0x6B, 0, 0, 0,
                0x03, 0, 0, 0, 1, 2, 3);
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedBytesTypeSupport.INSTANCE.encode(sample));
        assertEquals(sample, BuiltinTypeSupports.KeyedBytesTypeSupport.INSTANCE.decode(expected));
        assertEquals("DDS::KeyedBytes", BuiltinTypeSupports.KeyedBytesTypeSupport.INSTANCE.getTypeName());
    }

    @Test
    void anEmptyKeyedStringEncodesTheTwoEmptyStrings() {
        KeyedString sample = KeyedString.newKeyedString(Qos.env());
        // DHEADER 13 | length 1, NUL, 3 octets of padding | length 1, NUL
        byte[] expected = bytes(0x0D, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0);
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.encode(sample));
        assertEquals(sample, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.decode(expected));
    }

    @Test
    void aTruncatedOrInconsistentSampleIsRefused() {
        byte[] good = BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.encode(
                KeyedString.newKeyedString(Qos.env()).setKey("ab").setValue("c"));
        byte[] shorter = java.util.Arrays.copyOf(good, good.length - 1);
        assertThrows(XcdrException.class, () -> BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.decode(shorter));
        assertThrows(XcdrException.class, () -> BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.decode(new byte[2]));
        byte[] noNul = good.clone();
        noNul[10] = 0x7A; // the NUL after "ab" replaced
        assertThrows(XcdrException.class, () -> BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.decode(noNul));
    }

    @Test
    void theKeyHashIsTheMd5OfTheBigEndianKey() throws Exception {
        KeyedString sample = KeyedString.newKeyedString(Qos.env()).setKey("ab").setValue("whatever");
        // XTypes 1.3 §7.6.8: the unbounded string key is hashed: length 3 big-endian, 'a' 'b' NUL
        byte[] expected = MessageDigest.getInstance("MD5").digest(bytes(0, 0, 0, 3, 0x61, 0x62, 0));
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.keyHash(sample));
        // the value is not part of the key
        assertArrayEquals(expected,
                BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.keyHash(sample.clone().setValue("other")));
        assertFalse(java.util.Arrays.equals(expected,
                BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.keyHash(sample.clone().setKey("ac"))));
        KeyedBytes keyed = KeyedBytes.newKeyedBytes(Qos.env()).setKey("ab").setValue(bytes(1));
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedBytesTypeSupport.INSTANCE.keyHash(keyed));
    }

    @Test
    void aTopicOfKeyedStringCarriesTheSamplesOfTwoInstances() throws Exception {
        Topic<KeyedString> topic = participant.createTopic("CoreKeyedStrings", KeyedString.class);
        DataReader<KeyedString> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        DataWriter<KeyedString> writer = participant.createPublisher().createDataWriter(topic);
        writer.write(KeyedString.newKeyedString(Qos.env()).setKey("a").setValue("1"));
        writer.write(KeyedString.newKeyedString(Qos.env()).setKey("b").setValue("2"));
        writer.write(KeyedString.newKeyedString(Qos.env()).setKey("a").setValue("3"));
        List<Sample<KeyedString>> taken = reader.take(new ArrayList<Sample<KeyedString>>());
        assertEquals(3, taken.size());
        Set<String> seen = new HashSet<>();
        for (Sample<KeyedString> sample : taken) {
            seen.add(sample.getData().getKey() + "=" + sample.getData().getValue());
        }
        Set<String> expected = new HashSet<>();
        expected.add("a=1");
        expected.add("b=2");
        expected.add("a=3");
        assertEquals(expected, seen);
        // two keys, two instances
        assertFalse(writer.lookupInstance(KeyedString.newKeyedString(Qos.env()).setKey("a")).isNil());
        assertFalse(writer.lookupInstance(KeyedString.newKeyedString(Qos.env()).setKey("b")).isNil());
        assertTrue(writer.lookupInstance(KeyedString.newKeyedString(Qos.env()).setKey("c")).isNil());
        assertFalse(writer.lookupInstance(KeyedString.newKeyedString(Qos.env()).setKey("a"))
                .equals(writer.lookupInstance(KeyedString.newKeyedString(Qos.env()).setKey("b"))));
    }

    @Test
    void aTopicOfKeyedBytesCarriesTheOctets() throws Exception {
        Topic<KeyedBytes> topic = participant.createTopic("CoreKeyedBytes", KeyedBytes.class);
        DataReader<KeyedBytes> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<KeyedBytes> writer = participant.createPublisher().createDataWriter(topic);
        writer.write(KeyedBytes.newKeyedBytes(Qos.env()).setKey("k").setValue(bytes(9, 8, 7)));
        List<Sample<KeyedBytes>> taken = reader.take(new ArrayList<Sample<KeyedBytes>>());
        assertEquals(1, taken.size());
        assertEquals("k", taken.get(0).getData().getKey());
        assertArrayEquals(bytes(9, 8, 7), taken.get(0).getData().getValue());
    }
}
