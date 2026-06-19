// SPDX-License-Identifier: Apache-2.0
// Cross-ORB OTS capture against JacORB 3.9 (codepit, JDK8): a real
// TransactionService coordinator yields a live PropagationContext, serialized
// exactly as JacORB's ClientContextTransferInterceptor puts it into IIOP
// service context id=0. Feeds the ZeroDDS decode test
// `jacorb_live_capture::decodes_live_jacorb_propagation_context_with_real_iors`.
//
// Build + run (force JacORB ORB):
//   CP="$(ls /opt/jacorb/lib/*.jar | tr '\n' ':')"
//   /opt/jdk8/bin/javac -encoding UTF-8 -cp "$CP" -d build DumpOts.java
//   /opt/jdk8/bin/java -Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB \
//       -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton \
//       -cp "build:$CP" DumpOts
// Prints OTS_PROPCTX_HEX= (the big-endian PropagationContext, full live IORs).
import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.CosTransactions.*;
import org.jacorb.transaction.TransactionService;

public class DumpOts {
    static String hex(byte[] b, int n) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < n; i++) s.append(String.format("%02x", b[i] & 0xff));
        return s.toString();
    }
    public static void main(String[] a) throws Exception {
        ORB orb = ORB.init(a, null);
        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
        // 2nd arg = coordinator pool size; 0 → find_free throws INTERNAL.
        TransactionService.start(root, 10);
        TransactionFactory factory = TransactionService.get_reference();
        Control ctrl = factory.create(0);
        Coordinator coord = ctrl.get_coordinator();
        Terminator term = ctrl.get_terminator();
        // PropagationContext built exactly like TransactionCurrentImpl.begin().
        otid_t otid = new otid_t(7, 3, new byte[]{(byte)0xAA,(byte)0xBB,(byte)0xCC});
        TransIdentity cur = new TransIdentity(coord, term, otid);
        PropagationContext ctx = new PropagationContext(0, cur, new TransIdentity[0], orb.create_any());
        org.omg.CORBA.portable.OutputStream os = orb.create_output_stream();
        PropagationContextHelper.write(os, ctx);
        org.jacorb.orb.CDROutputStream c = (org.jacorb.orb.CDROutputStream) os;
        System.out.println("OTS_PROPCTX_HEX=" + hex(c.getBufferCopy(), c.size()));
        System.exit(0);
    }
}
