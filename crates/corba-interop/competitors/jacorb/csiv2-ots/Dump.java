// SPDX-License-Identifier: Apache-2.0
// Cross-ORB byte-conformance capture against JacORB 3.9 (the Linux test host, JDK8):
//   OTID_HEX  — org.omg.CosTransactions.otid_t (OTS, #15)
//   GSSUP_HEX — org.omg.GSSUP.InitialContextToken (CSIv2, #14)
// Invocation (force JacORB as the ORB):
//   javac -encoding UTF-8 -cp "$JACORB/lib/*" -d build Dump.java
//   java -Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB \
//        -Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton \
//        -cp "build:$JACORB/lib/*" Dump
// Expected:
//   OTID_HEX=000000070000000300000003aabbcc
//   GSSUP_HEX=00000005616c69636500000000000006736563726574000000000006746172676574
import org.omg.CORBA.ORB;

public class Dump {
    static String hex(byte[] b, int n) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < n; i++) s.append(String.format("%02x", b[i] & 0xff));
        return s.toString();
    }
    public static void main(String[] a) throws Exception {
        ORB orb = ORB.init(a, null);
        // --- OTS otid_t(formatID=7, bequeath_length=3, tid=AA BB CC) ---
        org.omg.CosTransactions.otid_t o =
            new org.omg.CosTransactions.otid_t(7, 3, new byte[]{(byte) 0xAA, (byte) 0xBB, (byte) 0xCC});
        org.omg.CORBA.portable.OutputStream o1 = orb.create_output_stream();
        org.omg.CosTransactions.otid_tHelper.write(o1, o);
        org.jacorb.orb.CDROutputStream c1 = (org.jacorb.orb.CDROutputStream) o1;
        System.out.println("OTID_HEX=" + hex(c1.getBufferCopy(), c1.size()));
        // --- CSIv2 GSSUP InitialContextToken(alice/secret/target) ---
        org.omg.GSSUP.InitialContextToken t = new org.omg.GSSUP.InitialContextToken(
            "alice".getBytes("UTF-8"), "secret".getBytes("UTF-8"), "target".getBytes("UTF-8"));
        org.omg.CORBA.portable.OutputStream o2 = orb.create_output_stream();
        org.omg.GSSUP.InitialContextTokenHelper.write(o2, t);
        org.jacorb.orb.CDROutputStream c2 = (org.jacorb.orb.CDROutputStream) o2;
        System.out.println("GSSUP_HEX=" + hex(c2.getBufferCopy(), c2.size()));
    }
}
