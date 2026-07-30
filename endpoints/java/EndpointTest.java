// SPDX-License-Identifier: Apache-2.0
// Java XRCE + serial framing byte-identical to crates/xrce (ADR 0013).
// usage: java EndpointTest <golden_dir>
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Arrays;

public class EndpointTest {
    static byte[] sample() {
        Zdw.Writer w = new Zdw.Writer(Zdw.LE);
        w.u32(0xA1B2C3D4L); w.u16(0x1234); w.u8(0x5A); w.f32(3.5f);
        w.u64(0x0102030405060708L); w.str("bay-12");
        w.seqU8(new byte[]{(byte)0xDE,(byte)0xAD,(byte)0xBE,(byte)0xEF});
        return w.bytes();
    }
    static byte[] load(String d, String n) throws Exception { return Files.readAllBytes(Paths.get(d, n)); }

    // Negative frame vectors (self-contained, no goldens): the reader must bound
    // the body to the declared submessage_length and cleanly reject malformed
    // frames instead of reading out of bounds.
    static int negativeFrameVectors() {
        int rc = 0;
        byte[] first = ZdwEndpoint.xrceWriteFrame(0x80, 0x01, 1, new byte[]{(byte)0xAA,(byte)0xBB,(byte)0xCC});
        byte[] second = ZdwEndpoint.xrceWriteFrame(0x80, 0x01, 2, new byte[]{(byte)0xDD,(byte)0xEE});
        byte[] concat = Arrays.copyOf(first, first.length + second.length);
        System.arraycopy(second, 0, concat, first.length, second.length);
        if (!Arrays.equals(ZdwEndpoint.xrceReadBody(concat), new byte[]{(byte)0xAA,(byte)0xBB,(byte)0xCC})) {
            System.out.println("appended-submessage leak"); rc = 1;
        } else System.out.println("appended submessage bounded out");
        byte[] overlong = first.clone(); overlong[6] = (byte)0xFF; overlong[7] = (byte)0xFF;
        try { ZdwEndpoint.xrceReadBody(overlong); System.out.println("over-long length not rejected"); rc = 1; }
        catch (IllegalArgumentException e) { System.out.println("over-long length rejected"); }
        try { ZdwEndpoint.xrceReadBody(new byte[]{(byte)0x80,0x01,0x00,0x00,0x07}); System.out.println("truncated not rejected"); rc = 1; }
        catch (IllegalArgumentException e) { System.out.println("truncated header rejected"); }
        try { ZdwEndpoint.xrceWriteFrame(0x80, 0x01, 1, new byte[0x10000]); System.out.println("writer >0xFFFF not refused"); rc = 1; }
        catch (IllegalArgumentException e) { System.out.println("writer >0xFFFF refused"); }
        return rc;
    }
    public static void main(String[] a) throws Exception {
        String d = a.length > 0 ? a[0] : ".";
        int rc = 0;
        byte[] body = sample();
        byte[] frame = ZdwEndpoint.xrceWriteFrame(ZdwEndpoint.SESSION_NOKEY, ZdwEndpoint.STREAM_BEST_EFFORT, 1, body);
        if (!Arrays.equals(frame, load(d, "golden_xrce_le.bin"))) { System.out.println("WRITE_DATA mismatch"); rc = 1; }
        else System.out.println("XRCE WRITE_DATA byte-identical (" + frame.length + " bytes)");
        if (!Arrays.equals(ZdwEndpoint.serialFrame(frame), load(d, "golden_serial_le.bin"))) { System.out.println("serial mismatch"); rc = 1; }
        else System.out.println("serial byte-identical");
        if (!Arrays.equals(ZdwEndpoint.xrceReadBody(load(d, "golden_data_le.bin")), body)) { System.out.println("DATA mismatch"); rc = 1; }
        else System.out.println("DATA receive: body ok");
        if (!Arrays.equals(ZdwEndpoint.serialDeframe(ZdwEndpoint.serialFrame(frame)), frame)) { System.out.println("deframe mismatch"); rc = 1; }
        else System.out.println("serial deframe+crc round-trip ok");
        int[] hb = ZdwEndpoint.heartbeatRead(load(d, "golden_heartbeat_le.bin"));
        if (hb[0] != 1 || hb[1] != 3 || hb[2] != 0x80) { System.out.println("heartbeat vals"); rc = 1; }
        else System.out.println("HEARTBEAT parsed: first=" + hb[0] + " last=" + hb[1]);
        if (!Arrays.equals(ZdwEndpoint.acknackFrame(0x80, 0x00, 1, 1, 0, 0, 0x80), load(d, "golden_acknack_le.bin"))) { System.out.println("ACKNACK mismatch"); rc = 1; }
        else System.out.println("ACKNACK byte-identical");
        rc |= negativeFrameVectors();
        if (rc == 0) System.out.println("ALL OK");
        System.exit(rc);
    }
}
