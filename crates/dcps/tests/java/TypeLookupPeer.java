// SPDX-License-Identifier: Apache-2.0
import java.util.Arrays;
import java.util.HexFormat;
import java.util.concurrent.TimeUnit;
import org.zerodds.cdr.TypeObjectRegistry;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.typelookup.TypeLookupService;
import org.zerodds.typelookup.generated.ZdCodec;
import org.zerodds.typelookup.generated.ZdTypes.DDS;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.TryConstruct;

/**
 * The Java side of `crates/dcps/tests/java_typelookup_live.rs`: the pure-Java
 * RTPS participant of `crates/java-omgdds` against a Rust runtime, both with
 * the default TypeLookup encoding (CDR2_LE, XCDR2).
 *
 * <ul>
 *   <li>{@code ask <domain> <peer-prefix> <identifier>}: fetch the object of
 *       {@code identifier} (a TypeIdentifier, XCDR2 octets in hex) from the
 *       peer with {@code getTypes} and print it as {@code OBJECT=<hex>}, the
 *       XCDR2 octets of the TypeObject.</li>
 *   <li>{@code serve <domain> <identifier> <object> <seconds>}: hold the
 *       object (XCDR2 octets in hex) under the identifier, print
 *       {@code PREFIX=<hex>} and answer for {@code seconds}.</li>
 * </ul>
 */
public final class TypeLookupPeer {
    public static void main(String[] args) throws Exception {
        int domain = Integer.parseInt(args[1]);
        RtpsParticipant participant = RtpsParticipant.newParticipant(domain);
        System.out.println("ENCODING=" + TypeLookupService.Encoding.configured().name());
        try {
            if (args[0].equals("serve")) {
                byte[] identifier = HexFormat.of().parseHex(args[2]);
                byte[] object = HexFormat.of().parseHex(args[3]);
                participant.registerTypeObjects(new TypeObjectRegistry(
                        new TypeObjectRegistry.Entry[] {new TypeObjectRegistry.Entry(identifier, object)}));
                System.out.println("PREFIX=" + HexFormat.of().formatHex(participant.guidPrefix()));
                System.out.flush();
                Thread.sleep(TimeUnit.SECONDS.toMillis(Long.parseLong(args[4])));
                return;
            }
            byte[] peer = HexFormat.of().parseHex(args[2]);
            byte[] identifier = HexFormat.of().parseHex(args[3]);
            DDS.XTypes.TypeIdentifier id = ZdCodec.zd_dec_DDS_XTypes_TypeIdentifier(
                    TypeLookupService.reader(identifier, Endianness.LITTLE), TryConstruct.DISCARD);
            DDS.Builtin.TypeLookup_Call call = new DDS.Builtin.TypeLookup_Call();
            call._d = (int) DDS.Builtin.TypeLookup_getTypes_HashId;
            call.getTypes = new DDS.Builtin.TypeLookup_getTypes_In();
            call.getTypes.type_ids = new DDS.XTypes.TypeIdentifier[] {id};
            DDS.Builtin.TypeLookup_Reply reply = null;
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
            while (reply == null && System.nanoTime() < deadline) {
                try {
                    reply = participant.queryTypeLookup(peer, call).get(4, TimeUnit.SECONDS);
                } catch (IllegalArgumentException notDiscoveredYet) {
                    Thread.sleep(100);
                } catch (java.util.concurrent.TimeoutException unanswered) {
                    // asked before the peer's endpoints were up; ask again
                }
            }
            if (reply == null) throw new AssertionError("no getTypes reply within 15 s");
            for (DDS.XTypes.TypeIdentifierTypeObjectPair pair : reply.return_data.getType.result.types) {
                org.zerodds.wire.Writer w = TypeLookupService.writer();
                ZdCodec.zd_enc_DDS_XTypes_TypeIdentifier(w, pair.type_identifier);
                if (!Arrays.equals(w.finish(), identifier)) continue;
                org.zerodds.wire.Writer o = TypeLookupService.writer();
                ZdCodec.zd_enc_DDS_XTypes_TypeObject(o, pair.type_object);
                System.out.println("OBJECT=" + HexFormat.of().formatHex(o.finish()));
                return;
            }
            throw new AssertionError("the reply holds no object for the identifier asked for");
        } finally {
            participant.close();
        }
    }
}
