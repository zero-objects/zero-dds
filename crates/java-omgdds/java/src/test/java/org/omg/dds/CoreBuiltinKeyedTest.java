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
    void javapsm_7_8_2_an_empty_keyed_string_has_the_octets_of_two_empty_strings() {
        KeyedString sample = KeyedString.newKeyedString(Qos.env());
        // DHEADER 13 | length 1, NUL, 3 octets of padding | length 1, NUL
        byte[] expected = bytes(0x0D, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0);
        assertArrayEquals(expected, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.encode(sample));
        assertEquals(sample, BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE.decode(expected));
    }

}
