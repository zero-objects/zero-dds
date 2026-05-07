// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;

import static org.junit.jupiter.api.Assertions.*;

class Xcdr2CodecTest {
    @Test
    void primitivesRoundTrip() {
        ByteBuffer enc = Xcdr2Codec.encoder(64);
        Xcdr2Codec.writeBoolean(enc, true);
        Xcdr2Codec.writeByte(enc, (byte) 0x42);
        Xcdr2Codec.writeShort(enc, (short) 0x1234);
        Xcdr2Codec.writeInt(enc, 0x12345678);
        Xcdr2Codec.writeLong(enc, 0x123456789ABCDEF0L);
        Xcdr2Codec.writeFloat(enc, 3.5f);
        Xcdr2Codec.writeDouble(enc, 2.5);

        ByteBuffer dec = Xcdr2Codec.decoder(Xcdr2Codec.copyToBytes(enc));
        assertTrue(Xcdr2Codec.readBoolean(dec));
        assertEquals(0x42, Xcdr2Codec.readByte(dec));
        assertEquals((short) 0x1234, Xcdr2Codec.readShort(dec));
        assertEquals(0x12345678, Xcdr2Codec.readInt(dec));
        assertEquals(0x123456789ABCDEF0L, Xcdr2Codec.readLong(dec));
        assertEquals(3.5f, Xcdr2Codec.readFloat(dec), 0.0f);
        assertEquals(2.5, Xcdr2Codec.readDouble(dec), 0.0);
    }

    @Test
    void utf8StringRoundTrip() {
        ByteBuffer enc = Xcdr2Codec.encoder(64);
        Xcdr2Codec.writeString(enc, "Käfer");
        ByteBuffer dec = Xcdr2Codec.decoder(Xcdr2Codec.copyToBytes(enc));
        assertEquals("Käfer", Xcdr2Codec.readString(dec));
    }

    @Test
    void multipleStringsRoundTrip() {
        ByteBuffer enc = Xcdr2Codec.encoder(128);
        Xcdr2Codec.writeString(enc, "first");
        Xcdr2Codec.writeString(enc, "second");
        Xcdr2Codec.writeString(enc, "third");
        ByteBuffer dec = Xcdr2Codec.decoder(Xcdr2Codec.copyToBytes(enc));
        assertEquals("first", Xcdr2Codec.readString(dec));
        assertEquals("second", Xcdr2Codec.readString(dec));
        assertEquals("third", Xcdr2Codec.readString(dec));
    }

    @Test
    void shortAlignmentSkipsByte() {
        // After byte 0x42 at pos 0, short is aligned to position 2.
        ByteBuffer enc = Xcdr2Codec.encoder(16);
        Xcdr2Codec.writeByte(enc, (byte) 0x42);
        Xcdr2Codec.writeShort(enc, (short) 0xABCD);
        // Position should be 4 now (1 byte + 1 padding + 2 short).
        assertEquals(4, enc.position());
    }
}
