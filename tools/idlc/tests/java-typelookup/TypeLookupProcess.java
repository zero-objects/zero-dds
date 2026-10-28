// SPDX-License-Identifier: Apache-2.0
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.concurrent.TimeUnit;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.rtps.RtpsMessage;
import org.zerodds.rtps.Guid;
import org.zerodds.rtps.Locator;
import org.zerodds.rtps.DiscoveryData;
import org.zerodds.rtps.Rtps;
import org.zerodds.typelookup.TypeLookupService;
import org.zerodds.typelookup.generated.ZdTypes.DDS;
import org.zerodds.typelookup.generated.ZdCodec;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.TryConstruct;

/** Two actual JVMs, source-derived objects, both operations and continuation. */
public final class TypeLookupProcess {
    private static void check(boolean value, String why) {
        if (!value) throw new AssertionError(why);
    }

    // §7.6.3.3.4.1 lets a getTypes reply carry more than was asked
    // (dependencies, complete/minimal pairs), and the native server puts
    // dependencies first; the requested object is found by its identifier,
    // never by position. TypeIdentifierTypeObjectPair/TypeIdentifier are
    // generated plain data classes with no equals()/hashCode(), so identity
    // is decided by encoding both sides with the same codec used on the wire
    // and comparing the resulting bytes.
    private static DDS.XTypes.TypeIdentifierTypeObjectPair findRequested(
            DDS.XTypes.TypeIdentifierTypeObjectPair[] types, DDS.XTypes.TypeIdentifier requested) {
        var wantedWriter = TypeLookupService.writer();
        ZdCodec.zd_enc_DDS_XTypes_TypeIdentifier(wantedWriter, requested);
        byte[] wanted = wantedWriter.finish();
        for (var pair : types) {
            var candidateWriter = TypeLookupService.writer();
            ZdCodec.zd_enc_DDS_XTypes_TypeIdentifier(candidateWriter, pair.type_identifier);
            if (Arrays.equals(candidateWriter.finish(), wanted)) return pair;
        }
        check(false, "requested type absent from reply");
        throw new AssertionError("unreachable");
    }

    public static void main(String[] args) throws Exception {
        if (args[0].equals("oracle")) {
            for (String line : Files.readAllLines(Path.of(args[1]))) {
                String[] parts = line.split("=", 2);
                byte[] bytes = HexFormat.of().parseHex(parts[1]);
                byte[] encoded = parts[0].endsWith("REQUEST")
                        ? TypeLookupService.encodeRequest(TypeLookupService.decodeRequest(bytes, Endianness.LITTLE))
                        : TypeLookupService.encodeReply(TypeLookupService.decodeReply(bytes, Endianness.LITTLE));
                // The generic generator may choose a different legal EMHEADER
                // length code. Decode the vendor bytes and our output equally.
                if (parts[0].endsWith("REQUEST"))
                    check(TypeLookupService.decodeRequest(encoded, Endianness.LITTLE).data._d
                        == TypeLookupService.decodeRequest(bytes, Endianness.LITTLE).data._d, "operation changed");
                else check(TypeLookupService.decodeReply(encoded, Endianness.LITTLE).return_data._d
                        == TypeLookupService.decodeReply(bytes, Endianness.LITTLE).return_data._d, "operation changed");
                check(Arrays.equals(encoded, bytes), "nonidentical oracle bytes " + parts[0]);
            }
            System.out.println("JAVA_CYCLONE_ENVELOPES=4");
            return;
        }
        if (args[0].equals("loss")) { loss(args); return; }
        var participant = RtpsParticipant.newParticipant(Integer.parseInt(args[1]));
        var support = ZdTypeSupports.Support_service_Root.INSTANCE;
        try {
            if (args[0].equals("server")) {
                participant.registerTypeObjects(support.typeObjects());
                var channelField = RtpsParticipant.class.getDeclaredField("unicastChannel");
                channelField.setAccessible(true);
                var channel = (java.nio.channels.DatagramChannel) channelField.get(participant);
                int port = ((java.net.InetSocketAddress) channel.getLocalAddress()).getPort();
                Files.writeString(Path.of(args[2]), HexFormat.of().formatHex(participant.guidPrefix()) + "\n" + port);
                Thread.sleep(30000);
                return;
            }
            byte[] peer = HexFormat.of().parseHex(args[2]);
            byte[] root = new byte[15];
            root[0] = (byte) 0xf2;
            System.arraycopy(support.typeIdentifier(), 0, root, 1, 14);
            var id = ZdCodec.zd_dec_DDS_XTypes_TypeIdentifier(TypeLookupService.reader(root, Endianness.LITTLE), TryConstruct.DISCARD);
            var call = new DDS.Builtin.TypeLookup_Call();
            call._d = (int) DDS.Builtin.TypeLookup_getTypes_HashId;
            call.getTypes = new DDS.Builtin.TypeLookup_getTypes_In();
            call.getTypes.type_ids = new DDS.XTypes.TypeIdentifier[] {id};
            DDS.Builtin.TypeLookup_Reply reply = null;
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(8);
            while (reply == null && System.nanoTime() < deadline) {
                try { reply = participant.queryTypeLookup(peer, call).get(4, TimeUnit.SECONDS); }
                catch (IllegalArgumentException notDiscovered) { Thread.sleep(100); }
            }
            check(reply != null, "peer not discovered");
            // This "client" peer talks to the pure-Java TypeLookupService.handle
            // (TypeLookupProcess "server" below), which answers getTypes with
            // exactly the identifiers asked for and no dependency expansion
            // (see TypeLookupService.java handle(), getTypes branch) — unlike
            // the native Rust server (server.rs handle_get_types), which
            // returns the transitive closure. So exactly one pair is expected
            // here; the closure case is exercised against the native server
            // in loss() below.
            check(reply.return_data.getType.result.types.length == 1, "missing root object");
            var writer = TypeLookupService.writer();
            ZdCodec.zd_enc_DDS_XTypes_TypeObject(writer, findRequested(reply.return_data.getType.result.types, id).type_object);
            check(Arrays.equals(writer.finish(), support.typeObject()), "returned object differs from source object");
            call = new DDS.Builtin.TypeLookup_Call();
            call._d = (int) DDS.Builtin.TypeLookup_getDependencies_HashId;
            call.getTypeDependencies = new DDS.Builtin.TypeLookup_getTypeDependencies_In();
            call.getTypeDependencies.type_ids = new DDS.XTypes.TypeIdentifier[] {id};
            call.getTypeDependencies.continuation_point = new byte[0];
            int count = 0, pages = 0;
            do {
                reply = participant.queryTypeLookup(peer, call).get(4, TimeUnit.SECONDS);
                var result = reply.return_data.getTypeDependencies.result;
                count += result.dependent_typeids.length;
                call.getTypeDependencies.continuation_point = result.continuation_point;
                pages++;
            } while (call.getTypeDependencies.continuation_point.length != 0 && pages < 3);
            check(count == 65 && pages == 2, "dependency pagination " + count + "/" + pages);
            root[1] ^= 0x55;
            id = ZdCodec.zd_dec_DDS_XTypes_TypeIdentifier(TypeLookupService.reader(root, Endianness.LITTLE), TryConstruct.DISCARD);
            call = new DDS.Builtin.TypeLookup_Call();
            call._d = (int) DDS.Builtin.TypeLookup_getTypes_HashId;
            call.getTypes = new DDS.Builtin.TypeLookup_getTypes_In();
            call.getTypes.type_ids = new DDS.XTypes.TypeIdentifier[] {id};
            reply = participant.queryTypeLookup(peer, call).get(4, TimeUnit.SECONDS);
            check(reply.return_data.getType.result.types.length == 0, "unknown identifier fabricated");
            System.out.println("JAVA_TYPELOOKUP_LIVE=ROOT_EXACT,DEPENDENCIES_65,PAGES_2,UNKNOWN_EMPTY");
        } finally { participant.close(); }
    }

    private static void loss(String[] args) throws Exception {
        byte[] peer = HexFormat.of().parseHex(args[2]);
        byte[] prefix = HexFormat.of().parseHex("010f00001122334455667788");
        var target = new java.net.InetSocketAddress("127.0.0.1", Integer.parseInt(args[3]));
        var support = ZdTypeSupports.Support_service_Root.INSTANCE;
        byte[] root = new byte[15]; root[0] = (byte) 0xf2;
        System.arraycopy(support.typeIdentifier(), 0, root, 1, 14);
        var request = new DDS.Builtin.TypeLookup_Request();
        request.header = new DDS.RPC.RequestHeader();
        request.header.requestId = TypeLookupService.identity(new Guid(prefix, Guid.TL_REQ_WRITER).bytes(), 73);
        request.header.instanceName = "dds.builtin.TOS." + HexFormat.of().formatHex(new Guid(peer, Guid.ENTITYID_PARTICIPANT).bytes());
        var requestedId = ZdCodec.zd_dec_DDS_XTypes_TypeIdentifier(TypeLookupService.reader(root, Endianness.LITTLE), TryConstruct.DISCARD);
        request.data = new DDS.Builtin.TypeLookup_Call();
        request.data._d = (int) DDS.Builtin.TypeLookup_getTypes_HashId;
        request.data.getTypes = new DDS.Builtin.TypeLookup_getTypes_In();
        request.data.getTypes.type_ids = new DDS.XTypes.TypeIdentifier[] {requestedId};
        byte[] body = TypeLookupService.encodeRequest(request);
        byte[] payload = new byte[body.length + 4]; payload[1] = 7;
        System.arraycopy(body, 0, payload, 4, body.length);
        try (var socket = new java.net.DatagramSocket(0, java.net.InetAddress.getByName("127.0.0.1"))) {
            socket.setSoTimeout(3000);
            var locator = Locator.udpV4("127.0.0.1", socket.getLocalPort());
            byte[] discovery = new RtpsMessage.Builder(prefix).data(Guid.ENTITYID_UNKNOWN, Guid.SPDP_WRITER, 1, null,
                DiscoveryData.buildParticipant(prefix, Integer.parseInt(args[1]), locator, locator, Rtps.EP_ALL_BASIC, 30), false).toBytes();
            socket.send(new java.net.DatagramPacket(discovery, discovery.length, target));
            byte[] wireRequest = new RtpsMessage.Builder(prefix).infoDestination(peer)
                .data(Guid.TL_REQ_READER, Guid.TL_REQ_WRITER, 1, null, payload, false).toBytes();
            socket.send(new java.net.DatagramPacket(wireRequest, wireRequest.length, target));
            byte[] assembled = null;
            var seen = new java.util.BitSet();
            int count = 0, dropped = 0, repaired = 0;
            long sequence = 0;
            boolean requested = false;
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(8);
            while (System.nanoTime() < deadline) {
                byte[] buffer = new byte[65536];
                var packet = new java.net.DatagramPacket(buffer, buffer.length);
                socket.receive(packet);
                var parsed = RtpsMessage.decode(buffer, packet.getLength());
                if (parsed == null || !Arrays.equals(parsed.header.guidPrefix, peer)) continue;
                for (var fragment : parsed.dataFragments) {
                    if (!Arrays.equals(fragment.writerId, Guid.TL_REPLY_WRITER)) continue;
                    if (assembled == null) {
                        assembled = new byte[(int) fragment.sampleSize];
                        count = (assembled.length + fragment.fragmentSize - 1) / fragment.fragmentSize;
                        sequence = fragment.writerSn;
                        check(count > 1, "large object was not fragmented");
                    }
                    if (fragment.fragmentStartingNum == 1 && dropped == 0) { dropped++; continue; }
                    int index = (int) fragment.fragmentStartingNum - 1;
                    System.arraycopy(fragment.serializedPayload, 0, assembled, index * fragment.fragmentSize, fragment.serializedPayload.length);
                    seen.set(index);
                    if (index == 0 && requested) repaired++;
                }
                if (assembled != null && seen.cardinality() == count - 1 && !seen.get(0) && !requested) {
                    byte[] nack = new RtpsMessage.Builder(prefix).infoDestination(peer)
                        .nackFrag(Guid.TL_REPLY_READER, Guid.TL_REPLY_WRITER, sequence, 1, 1, 1).toBytes();
                    socket.send(new java.net.DatagramPacket(nack, nack.length, target));
                    requested = true;
                }
                if (assembled != null && seen.cardinality() == count) break;
            }
            check(dropped == 1 && repaired == 1 && requested && seen.cardinality() == count, "fragment repair did not occur");
            // The reply comes in the encoding its identifier names (CDR_LE by
            // default, DDSI-RTPS 2.5 section 10.4) and with the padding its
            // options count.
            var encapsulation = TypeLookupService.Encapsulation.read(assembled);
            var reply = TypeLookupService.decodeReply(encapsulation.body, encapsulation.endian, encapsulation.encoding);
            check(TypeLookupService.sequence(reply.header.relatedRequestId) == 73, "RPC correlation changed");
            // This "loss" peer talks to the native Rust server (see
            // crates/dcps/tests/type_lookup_transport_live.rs), which answers
            // getTypes with the transitive closure of the requested type's
            // dependencies, dependencies first (server.rs handle_get_types) —
            // types[0] is a dependency here, not the requested root.
            var writer = TypeLookupService.writer();
            ZdCodec.zd_enc_DDS_XTypes_TypeObject(writer, findRequested(reply.return_data.getType.result.types, requestedId).type_object);
            check(Arrays.equals(writer.finish(), support.typeObject()), "repaired object differs");
            System.out.println("JAVA_TYPELOOKUP_LOSS=FRAGMENT_DROPPED_1,NACK_FRAG_REPAIRED_1,ROOT_EXACT");
        }
    }
}
