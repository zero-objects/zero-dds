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
        if (rc == 0) System.out.println("ALL OK");
        System.exit(rc);
    }
}
