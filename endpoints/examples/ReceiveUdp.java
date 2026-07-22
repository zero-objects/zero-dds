// SPDX-License-Identifier: Apache-2.0
// Example receiver: a pure-Java endpoint receives a DATA message the hub pushes
// over UDP and decodes the SensorReading (ADR 0013).
//   java ReceiveUdp <port>
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.util.Arrays;

public class ReceiveUdp {
    public static void main(String[] a) throws Exception {
        int port = a.length > 0 ? Integer.parseInt(a[0]) : 7447;
        DatagramSocket s = new DatagramSocket(port);
        System.out.println("java receiver: listening on udp/" + port);
        byte[] buf = new byte[2048];
        DatagramPacket p = new DatagramPacket(buf, buf.length);
        s.receive(p);
        byte[] frame = Arrays.copyOf(buf, p.getLength());
        byte[] body = ZdwEndpoint.xrceReadBody(frame);
        Zdw.Reader r = new Zdw.Reader(body, Zdw.LE);
        long id = r.u32(); r.u16(); r.u8(); float value = r.f32(); r.u64();
        String label = r.str(); r.seqU8();
        if (id != 0xA1B2C3D4L || !"bay-12".equals(label)) throw new IllegalStateException("mismatch");
        System.out.println("JAVA RECEIVER OK: id=0x" + Long.toHexString(id).toUpperCase()
            + " label=" + label + " value=" + value);
        s.close();
    }
}
