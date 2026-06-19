// SPDX-License-Identifier: Apache-2.0
// Cross-ORB capture for truncatable valuetypes (§15.3.4.3 chunked).
// ENCODE: JacORB marshals Derived(42,"hi") as truncatable → chunked + repo-id list
//   → golden vector for value_wire::tests::chunked_encode_byte_identical_to_jacorb.
// DECODE truncation: read the same bytes as Base → only {id} (extra is discarded).
import org.omg.CORBA.ORB;
import org.omg.CORBA_2_3.portable.OutputStream;

public class DumperTrunc {
    static String hex(byte[] b, int n) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < n; i++) s.append(String.format("%02x", b[i] & 0xff));
        return s.toString();
    }
    public static void main(String[] a) throws Exception {
        ORB orb = ORB.init(new String[0], null);
        OutputStream out = (OutputStream) orb.create_output_stream();
        out.write_value(new DerivedImpl(42, "hi"), "IDL:Derived:1.0");
        org.jacorb.orb.CDROutputStream co = (org.jacorb.orb.CDROutputStream) out;
        byte[] buf = co.getBufferCopy();
        int len = co.size();
        System.out.println("DERIVED_LEN=" + len);
        System.out.println("DERIVED_HEX=" + hex(buf, len));
        org.jacorb.orb.CDRInputStream ci =
            new org.jacorb.orb.CDRInputStream((org.jacorb.orb.ORB) orb, buf);
        Base base = (Base) ci.read_value("IDL:Base:1.0"); // truncation
        System.out.println("TRUNC_BASE_ID=" + base.id);
    }
}
