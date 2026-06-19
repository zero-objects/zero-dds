// SPDX-License-Identifier: Apache-2.0
// Cross-ORB valuetype capture: JacORB 3.9 as the reference ORB.
//
// ENCODE: marshals Point(42,-7) through a real CDR OutputStream
//   (write_value, §15.3.4) and prints the wire bytes as hex — this is the
//   golden vector against which ZeroDDS' value_wire is checked byte-for-byte
//   (crates/corba-rust value_wire::tests::jacorb_capture_byte_identical).
// DECODE: optionally reads ZeroDDS-produced bytes (hex arg) back via read_value
//   and prints x/y — demonstrates the reverse direction.
import org.omg.CORBA.ORB;
import org.omg.CORBA_2_3.portable.OutputStream;

public class Dumper {
    static String hex(byte[] b, int n) {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < n; i++) s.append(String.format("%02x", b[i] & 0xff));
        return s.toString();
    }
    public static void main(String[] a) throws Exception {
        ORB orb = ORB.init(new String[0], null);
        OutputStream out = (OutputStream) orb.create_output_stream();
        out.write_value(new PointImpl(42, -7), "IDL:Point:1.0");
        org.jacorb.orb.CDROutputStream co = (org.jacorb.orb.CDROutputStream) out;
        byte[] buf = co.getBufferCopy();
        int len = co.size();
        System.out.println("JACORB_ENCODE_LEN=" + len);
        System.out.println("JACORB_ENCODE_HEX=" + hex(buf, len));
        if (a.length > 0) {
            byte[] zb = new byte[a[0].length() / 2];
            for (int i = 0; i < zb.length; i++)
                zb[i] = (byte) Integer.parseInt(a[0].substring(2 * i, 2 * i + 2), 16);
            org.jacorb.orb.CDRInputStream ci =
                new org.jacorb.orb.CDRInputStream((org.jacorb.orb.ORB) orb, zb);
            Point p = (Point) ci.read_value("IDL:Point:1.0");
            System.out.println("JACORB_DECODE_X=" + p.x + " Y=" + p.y);
        }
    }
}
