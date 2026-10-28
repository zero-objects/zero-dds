// SPDX-License-Identifier: Apache-2.0
package org.zerodds.typelookup;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;
import org.zerodds.typelookup.generated.ZdTypes.DDS;
import org.zerodds.wire.Endianness;

/**
 * The TypeLookup samples of the Java runtime in both encodings.
 *
 * <p>The XCDR1 octets are the ones {@code crates/types/tests/type_lookup_xcdr1.rs}
 * writes out by hand from DDS-XTypes 1.3 §7.4.1 for the Rust runtime: the same
 * request and the same reply, so the two runtimes are held to one reading.
 * They are that reading and not a vendor capture; no measured vendor sends
 * CDR_LE TypeLookup samples.
 */
class TypeLookupEncodingTest {
    private static byte[] hex(String text) {
        String digits = text.replaceAll("[^0-9a-fA-F]", "");
        byte[] out = new byte[digits.length() / 2];
        for (int i = 0; i < out.length; i++) out[i] = (byte) Integer.parseInt(digits.substring(2 * i, 2 * i + 2), 16);
        return out;
    }

    /** The request of the Rust vector: one EK_MINIMAL identifier, hash 0x11 x 14. */
    private static DDS.Builtin.TypeLookup_Request request() {
        byte[] guid = new byte[16];
        java.util.Arrays.fill(guid, (byte) 0x5a);
        DDS.Builtin.TypeLookup_Request request = new DDS.Builtin.TypeLookup_Request();
        request.header = new DDS.RPC.RequestHeader();
        request.header.requestId = TypeLookupService.identity(guid, 0x0102030405060708L);
        request.header.instanceName = "dds.builtin.TOS.x";
        request.data = new DDS.Builtin.TypeLookup_Call();
        request.data._d = (int) DDS.Builtin.TypeLookup_getTypes_HashId;
        request.data.getTypes = new DDS.Builtin.TypeLookup_getTypes_In();
        DDS.XTypes.TypeIdentifier id = new DDS.XTypes.TypeIdentifier();
        id._d = DDS.XTypes.EK_MINIMAL;
        id.equivalence_hash = new byte[14];
        java.util.Arrays.fill(id.equivalence_hash, (byte) 0x11);
        request.data.getTypes.type_ids = new DDS.XTypes.TypeIdentifier[] {id};
        return request;
    }

    private static final byte[] REQUEST_XCDR1 = hex(
            "5a5a5a5a 5a5a5a5a 5a5a5a5a 5a5a5a5a"
            + "04030201 08070605"
            + "12000000 6464732e 6275696c 74696e2e 544f532e 7800"
            + "0000"
            + "d3528201"
            + "017f0800 6560530c 13000000"
            + "01000000 f1 1111111111111111111111111111"
            + "00"
            + "027f0000");

    /** The reply of the Rust vector: one pair, a minimal struct of one long. */
    private static final byte[] REPLY_XCDR1 = hex(
            "5a5a5a5a 5a5a5a5a 5a5a5a5a 5a5a5a5a"
            + "04030201 08070605"
            + "00000000"
            + "d3528201"
            + "00000000"
            + "017f0800 d14a8002 2b000000"
            + "01000000"
            + "f1 2222222222222222222222222222"
            + "f1"
            + "51"
            + "00 0200"
            + "00"
            + "000000 01000000"
            + "00000000 0100 04"
            + "0cc175b9"
            + "00"
            + "017f0800 77658e0b 04000000 00000000"
            + "027f0000");

    @Test
    void aGetTypesRequestIsWrittenInTheEncodingVersion1() {
        assertArrayEquals(REQUEST_XCDR1, TypeLookupService.encodeRequest(request(), TypeLookupService.Encoding.XCDR1));
    }

    @Test
    void theRustReplyReadsAndIsWrittenBackOctetForOctet() {
        DDS.Builtin.TypeLookup_Reply reply =
                TypeLookupService.decodeReply(REPLY_XCDR1, Endianness.LITTLE, TypeLookupService.Encoding.XCDR1);
        assertEquals((int) DDS.Builtin.TypeLookup_getTypes_HashId, reply.return_data._d);
        assertEquals(DDS.DDS_RETCODE_OK, reply.return_data.getType._d);
        assertEquals(1, reply.return_data.getType.result.types.length);
        assertEquals(DDS.XTypes.EK_MINIMAL, reply.return_data.getType.result.types[0].type_identifier._d);
        assertEquals(0, reply.return_data.getType.result.complete_to_minimal.length);
        assertEquals(0x0102030405060708L, TypeLookupService.sequence(reply.header.relatedRequestId));
        assertArrayEquals(REPLY_XCDR1, TypeLookupService.encodeReply(reply, TypeLookupService.Encoding.XCDR1));
    }

    @Test
    void bothEncodingsCarryTheSameRequest() {
        for (TypeLookupService.Encoding encoding : TypeLookupService.Encoding.values()) {
            byte[] payload = TypeLookupService.payload(TypeLookupService.encodeRequest(request(), encoding), encoding);
            assertEquals(0, payload.length % 4, encoding.name());
            TypeLookupService.Encapsulation read = TypeLookupService.Encapsulation.read(payload);
            assertEquals(encoding, read.encoding);
            DDS.Builtin.TypeLookup_Request decoded = TypeLookupService.decodeRequest(read.body, read.endian, read.encoding);
            assertEquals("dds.builtin.TOS.x", decoded.header.instanceName);
            assertEquals(1, decoded.data.getTypes.type_ids.length);
            assertArrayEquals(request().data.getTypes.type_ids[0].equivalence_hash,
                    decoded.data.getTypes.type_ids[0].equivalence_hash);
        }
    }

    @Test
    void theIdentifierAndThePaddingCountAreWrittenAndRead() {
        byte[] payload = TypeLookupService.payload(new byte[] {9, 9, 9}, TypeLookupService.Encoding.XCDR1);
        assertArrayEquals(hex("0001 0001 090909 00"), payload);
        payload = TypeLookupService.payload(new byte[] {9, 9, 9, 9}, TypeLookupService.Encoding.XCDR2);
        assertArrayEquals(hex("0007 0000 09090909"), payload);
        TypeLookupService.Encapsulation read = TypeLookupService.Encapsulation.read(hex("0007 0001 090909 00"));
        assertArrayEquals(new byte[] {9, 9, 9}, read.body);
        assertEquals(Endianness.LITTLE, read.endian);
        assertEquals(TypeLookupService.Encoding.XCDR1, TypeLookupService.Encapsulation.read(hex("0000 0000")).encoding);
        assertEquals(Endianness.BIG, TypeLookupService.Encapsulation.read(hex("0000 0000")).endian);
        // PL_CDR_LE would call the outermost type mutable, and it is not.
        assertThrows(IllegalArgumentException.class, () -> TypeLookupService.Encapsulation.read(hex("0003 0000")));
    }

    @Test
    void theSettingTakesTwoNamesAndRefusesOthers() {
        assertEquals(TypeLookupService.Encoding.XCDR1, TypeLookupService.Encoding.parse("xcdr1"));
        assertEquals(TypeLookupService.Encoding.XCDR2, TypeLookupService.Encoding.parse("xcdr2"));
        assertEquals(null, TypeLookupService.Encoding.parse("cdr2"));
        String before = System.getProperty("zerodds.wire.type_lookup_encoding");
        try {
            System.clearProperty("zerodds.wire.type_lookup_encoding");
            if (System.getenv("ZERODDS_WIRE_TYPE_LOOKUP_ENCODING") == null)
                assertEquals(TypeLookupService.Encoding.XCDR2, TypeLookupService.Encoding.configured());
            System.setProperty("zerodds.wire.type_lookup_encoding", "xcdr1");
            assertEquals(TypeLookupService.Encoding.XCDR1, TypeLookupService.Encoding.configured());
            System.setProperty("zerodds.wire.type_lookup_encoding", "cdr2");
            assertThrows(IllegalArgumentException.class, TypeLookupService.Encoding::configured);
        } finally {
            if (before == null) System.clearProperty("zerodds.wire.type_lookup_encoding");
            else System.setProperty("zerodds.wire.type_lookup_encoding", before);
        }
    }
}
