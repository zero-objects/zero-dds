// SPDX-License-Identifier: Apache-2.0
package org.zerodds.typelookup;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import org.zerodds.cdr.HexBytes;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.UUID;
import org.zerodds.cdr.TypeObjectRegistry;
import org.zerodds.typelookup.generated.ZdCodec;
import org.zerodds.typelookup.generated.ZdTypes.DDS;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;
import org.zerodds.wire.TryConstruct;

/** Both standard service operations over source-derived objects. All wire
 * envelopes, identifiers and TypeObjects use the generated vendor-schema codec.
 * Transport capability advertisement is deliberately separate. */
public final class TypeLookupService {
    /**
     * The encoding a TypeLookup sample is written in. {@link #XCDR2} (CDR2_LE
     * {0x00,0x07}) is the default: DDS-XTypes 1.4 Beta 1 §7.6.3.3.3 makes the
     * data representation of these endpoints encoding version 2 (OMG issue
     * DDSXTY14-29). It is also the form Cyclone DDS, OpenDDS and RTI Connext
     * write and the only one they read (measured,
     * internal/release/evidence/rc7-typelookup-encapsulation). {@link #XCDR1}
     * (CDR_LE {0x00,0x01}) is the reading of DDS-XTypes 1.3, which names no
     * representation for these endpoints (§7.6.3.1.1, Table 60), and of
     * DDSI-RTPS 2.5 §10.4 Table 10.2. A received sample is read in the
     * encoding its identifier names, under either setting.
     */
    public enum Encoding {
        XCDR1(0x01, 8),
        XCDR2(0x07, 4);

        private final int identifier;
        private final int maxAlign;

        Encoding(int identifier, int maxAlign) {
            this.identifier = identifier;
            this.maxAlign = maxAlign;
        }

        /** {@code xcdr1} or {@code xcdr2}, else {@code null}. */
        public static Encoding parse(String value) {
            if ("xcdr1".equals(value)) return XCDR1;
            if ("xcdr2".equals(value)) return XCDR2;
            return null;
        }

        /**
         * The setting {@code wire.type_lookup_encoding}: the system property
         * {@code zerodds.wire.type_lookup_encoding}, else the environment variable
         * {@code ZERODDS_WIRE_TYPE_LOOKUP_ENCODING} the Rust runtime reads, else
         * {@link #XCDR2}. A value that is neither name is refused, not defaulted.
         */
        public static Encoding configured() {
            String value = System.getProperty("zerodds.wire.type_lookup_encoding");
            if (value == null) value = System.getenv("ZERODDS_WIRE_TYPE_LOOKUP_ENCODING");
            if (value == null) return XCDR2;
            Encoding parsed = parse(value.trim());
            if (parsed == null)
                throw new IllegalArgumentException("wire.type_lookup_encoding takes xcdr1 or xcdr2, not " + value);
            return parsed;
        }
    }

    /** What the encapsulation header of a received TypeLookup sample says. */
    public static final class Encapsulation {
        public final Encoding encoding;
        public final Endianness endian;
        /** The body without the header and without the padding the options count. */
        public final byte[] body;

        private Encapsulation(Encoding encoding, Endianness endian, byte[] body) {
            this.encoding = encoding;
            this.endian = endian;
            this.body = body;
        }

        /**
         * CDR_BE and CDR_LE are XCDR1, CDR2_BE and CDR2_LE XCDR2 (Table 60); any
         * other identifier is refused. The two low bits of the options word are
         * the padding octets after the body (DDS-XTypes 1.3 §7.6.3.1.2).
         */
        public static Encapsulation read(byte[] payload) {
            if (payload.length < 4 || payload[0] != 0)
                throw new IllegalArgumentException("Expected a CDR or CDR2 TypeLookup encapsulation");
            Encoding encoding;
            Endianness endian;
            switch (payload[1]) {
                case 0: encoding = Encoding.XCDR1; endian = Endianness.BIG; break;
                case 1: encoding = Encoding.XCDR1; endian = Endianness.LITTLE; break;
                case 6: encoding = Encoding.XCDR2; endian = Endianness.BIG; break;
                case 7: encoding = Encoding.XCDR2; endian = Endianness.LITTLE; break;
                default:
                    throw new IllegalArgumentException("Expected a CDR or CDR2 TypeLookup encapsulation, not 00 "
                            + String.format("%02x", payload[1] & 255));
            }
            int padding = payload[3] & 0x03;
            int end = Math.max(4, payload.length - padding);
            return new Encapsulation(encoding, endian, Arrays.copyOfRange(payload, 4, end));
        }
    }

    /**
     * A whole sample: the identifier of {@code encoding}, the options word, the
     * body and the zero octets that end it four-aligned, which the options count
     * (DDS-XTypes 1.3 §7.6.3.1.2).
     */
    public static byte[] payload(byte[] body, Encoding encoding) {
        int padding = (4 - body.length % 4) % 4;
        byte[] payload = new byte[4 + body.length + padding];
        payload[1] = (byte) encoding.identifier;
        payload[3] = (byte) padding;
        System.arraycopy(body, 0, payload, 4, body.length);
        return payload;
    }

    private static final int PAGE_SIZE = 64;
    private static final class Page {
        final String query;
        final List<TypeObjectRegistry.Entry> entries;
        final int offset;
        Page(String query, List<TypeObjectRegistry.Entry> entries, int offset) {
            this.query = query;
            this.entries = entries;
            this.offset = offset;
        }
    }
    private final LinkedHashMap<String, Page> pages = new LinkedHashMap<>();
    private TypeObjectRegistry registry = new TypeObjectRegistry(new TypeObjectRegistry.Entry[0]);

    public synchronized void register(TypeObjectRegistry additional) {
        ArrayList<TypeObjectRegistry.Entry> entries = new ArrayList<>(registry.entries());
        entries.addAll(additional.entries());
        ArrayList<TypeObjectRegistry.IdentifierPair> mappings = new ArrayList<>(registry.mappings());
        mappings.addAll(additional.mappings());
        registry = new TypeObjectRegistry(entries.toArray(new TypeObjectRegistry.Entry[0]),
                mappings.toArray(new TypeObjectRegistry.IdentifierPair[0]));
    }

    /** The registered object of an encoded TypeIdentifier, or {@code null}. */
    public synchronized TypeObjectRegistry.Entry find(byte[] identifier) {
        return registry.find(identifier);
    }

    public static Reader reader(byte[] bytes, Endianness endian) {
        return new Reader(bytes, endian, 4, (org.zerodds.wire.PlCdr1Framing) null);
    }

    private static Reader reader(byte[] bytes, Endianness endian, Encoding encoding) {
        return new Reader(bytes, endian, encoding.maxAlign, (org.zerodds.wire.PlCdr1Framing) null);
    }

    private static Writer writer(Encoding encoding) {
        return new Writer(Endianness.LITTLE, encoding.maxAlign, null, false);
    }

    public static Writer writer() {
        return new Writer(Endianness.LITTLE, 4, null, false);
    }

    public static DDS.SampleIdentity identity(byte[] guid, long sequence) {
        if (guid.length != 16) throw new IllegalArgumentException("Expected writer GUID");
        DDS.SampleIdentity result = new DDS.SampleIdentity();
        result.writer_guid = new DDS.GUID_t();
        // CORBA 3.3 part 3 §1.5.2: an octet is a Java byte.
        result.writer_guid.guidPrefix = new byte[12];
        result.writer_guid.entityId = new DDS.EntityId_t();
        result.writer_guid.entityId.entityKey = new byte[3];
        System.arraycopy(guid, 0, result.writer_guid.guidPrefix, 0, 12);
        System.arraycopy(guid, 12, result.writer_guid.entityId.entityKey, 0, 3);
        result.writer_guid.entityId.entityKind = guid[15];
        result.sequence_number = new DDS.SequenceNumber();
        result.sequence_number.high = (int) (sequence >>> 32);
        result.sequence_number.low = sequence & 0xffffffffL;
        return result;
    }

    public static byte[] guid(DDS.SampleIdentity identity) {
        byte[] bytes = new byte[16];
        System.arraycopy(identity.writer_guid.guidPrefix, 0, bytes, 0, 12);
        System.arraycopy(identity.writer_guid.entityId.entityKey, 0, bytes, 12, 3);
        bytes[15] = identity.writer_guid.entityId.entityKind;
        return bytes;
    }

    public static long sequence(DDS.SampleIdentity identity) {
        return ((long) identity.sequence_number.high << 32) | identity.sequence_number.low;
    }

    public static DDS.Builtin.TypeLookup_Request decodeRequest(byte[] body, Endianness endian) {
        return decodeRequest(body, endian, Encoding.XCDR2);
    }

    public static DDS.Builtin.TypeLookup_Request decodeRequest(byte[] body, Endianness endian, Encoding encoding) {
        Reader reader = reader(body, endian, encoding);
        // TypeLookup_Request is FINAL (DDSXTY14-30): its header, then its
        // call, each with the generated codec of the encoding.
        DDS.Builtin.TypeLookup_Request request = new DDS.Builtin.TypeLookup_Request();
        if (encoding == Encoding.XCDR1) {
            request.header = ZdCodec.Xcdr1.zd_dec_DDS_RPC_RequestHeader(reader, TryConstruct.DISCARD);
            request.data = ZdCodec.Xcdr1.zd_dec_DDS_Builtin_TypeLookup_Call(reader, TryConstruct.DISCARD);
        } else {
            request.header = ZdCodec.zd_dec_DDS_RPC_RequestHeader(reader, TryConstruct.DISCARD);
            request.data = ZdCodec.zd_dec_DDS_Builtin_TypeLookup_Call(reader, TryConstruct.DISCARD);
        }
        if (reader.remaining() != 0) throw new IllegalArgumentException("Trailing TypeLookup request bytes");
        return request;
    }

    public static DDS.Builtin.TypeLookup_Reply decodeReply(byte[] body, Endianness endian) {
        return decodeReply(body, endian, Encoding.XCDR2);
    }

    public static DDS.Builtin.TypeLookup_Reply decodeReply(byte[] body, Endianness endian, Encoding encoding) {
        Reader reader = reader(body, endian, encoding);
        DDS.Builtin.TypeLookup_Reply reply = new DDS.Builtin.TypeLookup_Reply();
        if (encoding == Encoding.XCDR1) {
            reply.header = ZdCodec.Xcdr1.zd_dec_DDS_RPC_ReplyHeader(reader, TryConstruct.DISCARD);
            reply.return_data = ZdCodec.Xcdr1.zd_dec_DDS_Builtin_TypeLookup_Return(reader, TryConstruct.DISCARD);
        } else {
            reply.header = ZdCodec.zd_dec_DDS_RPC_ReplyHeader(reader, TryConstruct.DISCARD);
            reply.return_data = ZdCodec.zd_dec_DDS_Builtin_TypeLookup_Return(reader, TryConstruct.DISCARD);
        }
        if (reader.remaining() != 0) throw new IllegalArgumentException("Trailing TypeLookup reply bytes");
        return reply;
    }

    public static byte[] encodeRequest(DDS.Builtin.TypeLookup_Request request) {
        return encodeRequest(request, Encoding.XCDR2);
    }

    public static byte[] encodeRequest(DDS.Builtin.TypeLookup_Request request, Encoding encoding) {
        Writer writer = writer(encoding);
        if (encoding == Encoding.XCDR1) {
            ZdCodec.Xcdr1.zd_enc_DDS_RPC_RequestHeader(writer, request.header);
            ZdCodec.Xcdr1.zd_enc_DDS_Builtin_TypeLookup_Call(writer, request.data);
        } else {
            ZdCodec.zd_enc_DDS_RPC_RequestHeader(writer, request.header);
            ZdCodec.zd_enc_DDS_Builtin_TypeLookup_Call(writer, request.data);
        }
        return writer.finish();
    }

    public static byte[] encodeReply(DDS.Builtin.TypeLookup_Reply reply) {
        return encodeReply(reply, Encoding.XCDR2);
    }

    public static byte[] encodeReply(DDS.Builtin.TypeLookup_Reply reply, Encoding encoding) {
        Writer writer = writer(encoding);
        if (encoding == Encoding.XCDR1) {
            ZdCodec.Xcdr1.zd_enc_DDS_RPC_ReplyHeader(writer, reply.header);
            ZdCodec.Xcdr1.zd_enc_DDS_Builtin_TypeLookup_Return(writer, reply.return_data);
        } else {
            ZdCodec.zd_enc_DDS_RPC_ReplyHeader(writer, reply.header);
            ZdCodec.zd_enc_DDS_Builtin_TypeLookup_Return(writer, reply.return_data);
        }
        return writer.finish();
    }

    private static byte[] identifierBytes(DDS.XTypes.TypeIdentifier identifier) {
        Writer writer = writer();
        ZdCodec.zd_enc_DDS_XTypes_TypeIdentifier(writer, identifier);
        return writer.finish();
    }

    private static DDS.XTypes.TypeIdentifier identifier(byte[] bytes) {
        Reader reader = reader(bytes, Endianness.LITTLE);
        DDS.XTypes.TypeIdentifier value = ZdCodec.zd_dec_DDS_XTypes_TypeIdentifier(reader, TryConstruct.DISCARD);
        if (reader.remaining() != 0) throw new IllegalArgumentException("Trailing identifier bytes");
        return value;
    }

    public synchronized DDS.Builtin.TypeLookup_Reply handle(DDS.Builtin.TypeLookup_Request request) {
        DDS.Builtin.TypeLookup_Reply reply = new DDS.Builtin.TypeLookup_Reply();
        reply.header = new DDS.RPC.ReplyHeader();
        reply.header.relatedRequestId = request.header.requestId;
        reply.header.remoteEx = DDS.RPC.RemoteExceptionCode.REMOTE_EX_OK;
        reply.return_data = new DDS.Builtin.TypeLookup_Return();
        reply.return_data._d = request.data._d;
        if (request.data._d == DDS.Builtin.TypeLookup_getTypes_HashId) {
            DDS.Builtin.TypeLookup_getTypes_Result result = new DDS.Builtin.TypeLookup_getTypes_Result();
            result._d = DDS.DDS_RETCODE_OK;
            result.result = new DDS.Builtin.TypeLookup_getTypes_Out();
            ArrayList<DDS.XTypes.TypeIdentifierTypeObjectPair> pairs = new ArrayList<>();
            ArrayList<DDS.XTypes.TypeIdentifierPair> mappings = new ArrayList<>();
            for (DDS.XTypes.TypeIdentifier id : request.data.getTypes.type_ids) {
                byte[] encoded = identifierBytes(id);
                TypeObjectRegistry.Entry entry = registry.find(encoded);
                if (entry == null) continue;
                DDS.XTypes.TypeIdentifierTypeObjectPair pair = new DDS.XTypes.TypeIdentifierTypeObjectPair();
                pair.type_identifier = id;
                Reader reader = reader(entry.object(), Endianness.LITTLE);
                pair.type_object = ZdCodec.zd_dec_DDS_XTypes_TypeObject(reader, TryConstruct.DISCARD);
                if (reader.remaining() != 0) throw new IllegalArgumentException("Trailing TypeObject bytes");
                pairs.add(pair);
                for (TypeObjectRegistry.IdentifierPair mapping : registry.mappings()) {
                    if (!Arrays.equals(encoded, mapping.complete())) continue;
                    DDS.XTypes.TypeIdentifierPair mapped = new DDS.XTypes.TypeIdentifierPair();
                    mapped.type_identifier1 = identifier(mapping.complete());
                    mapped.type_identifier2 = identifier(mapping.minimal());
                    mappings.add(mapped);
                }
            }
            result.result.types = pairs.toArray(new DDS.XTypes.TypeIdentifierTypeObjectPair[0]);
            result.result.complete_to_minimal = mappings.toArray(new DDS.XTypes.TypeIdentifierPair[0]);
            reply.return_data.getType = result;
        } else if (request.data._d == DDS.Builtin.TypeLookup_getDependencies_HashId) {
            DDS.Builtin.TypeLookup_getTypeDependencies_In query = request.data.getTypeDependencies;
            String queryKey = Arrays.stream(query.type_ids).map(TypeLookupService::identifierBytes)
                    .map(HexBytes::encode).reduce("", (a, b) -> a + ":" + b);
            List<TypeObjectRegistry.Entry> entries;
            int offset = 0;
            if (query.continuation_point.length == 0) {
                LinkedHashMap<String, TypeObjectRegistry.Entry> closure = new LinkedHashMap<>();
                ArrayDeque<TypeObjectRegistry.Entry> queue = new ArrayDeque<>();
                for (DDS.XTypes.TypeIdentifier id : query.type_ids) {
                    TypeObjectRegistry.Entry root = registry.find(identifierBytes(id));
                    if (root != null) queue.addAll(registry.dependencies(root.identifier()));
                }
                while (!queue.isEmpty()) {
                    TypeObjectRegistry.Entry entry = queue.remove();
                    if (closure.putIfAbsent(HexBytes.encode(entry.identifier()), entry) == null)
                        queue.addAll(registry.dependencies(entry.identifier()));
                }
                entries = Collections.unmodifiableList(new ArrayList<>(closure.values()));
            } else {
                Page page = pages.get(Arrays.toString(query.continuation_point));
                if (page == null || !page.query.equals(queryKey))
                    throw new IllegalArgumentException("Unknown or mismatched dependency continuation");
                entries = page.entries;
                offset = page.offset;
            }
            int end = Math.min(entries.size(), offset + PAGE_SIZE);
            DDS.Builtin.TypeLookup_getTypeDependencies_Result result = new DDS.Builtin.TypeLookup_getTypeDependencies_Result();
            result._d = DDS.DDS_RETCODE_OK;
            result.result = new DDS.Builtin.TypeLookup_getTypeDependencies_Out();
            ArrayList<DDS.XTypes.TypeIdentifierWithSize> values = new ArrayList<>();
            for (TypeObjectRegistry.Entry entry : entries.subList(offset, end)) {
                DDS.XTypes.TypeIdentifierWithSize value = new DDS.XTypes.TypeIdentifierWithSize();
                value.type_id = identifier(entry.identifier());
                value.typeobject_serialized_size = entry.serializedSize();
                values.add(value);
            }
            result.result.dependent_typeids = values.toArray(new DDS.XTypes.TypeIdentifierWithSize[0]);
            result.result.continuation_point = new byte[0];
            if (end < entries.size()) {
                byte[] token = UUID.randomUUID().toString().replace("-", "").getBytes(java.nio.charset.StandardCharsets.US_ASCII);
                while (pages.size() >= 256) pages.remove(pages.keySet().iterator().next());
                pages.put(Arrays.toString(token), new Page(queryKey, entries, end));
                result.result.continuation_point = token;
            }
            reply.return_data.getTypeDependencies = result;
        } else {
            throw new IllegalArgumentException("Unknown TypeLookup operation");
        }
        return reply;
    }
}
