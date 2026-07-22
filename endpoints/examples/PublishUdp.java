// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Example: a pure-Java endpoint publishes a SensorReading to a ZeroDDS/XRCE hub
// over UDP (ADR 0013). Compile with Zdw.java + ZdwEndpoint.java on the
// classpath. Run the agent (endpoints/xrce-agent-demo) first.
//   java PublishUdp <host> <port>
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;

public class PublishUdp {
    public static void main(String[] a) throws Exception {
        String host = a.length > 0 ? a[0] : "127.0.0.1";
        int port = a.length > 1 ? Integer.parseInt(a[1]) : 7447;

        Zdw.Writer w = new Zdw.Writer(Zdw.LE);
        w.u32(0xA1B2C3D4L);
        w.u16(0x1234);
        w.u8(0x5A);
        w.f32(3.5f);
        w.u64(0x0102030405060708L);
        w.str("bay-12");
        w.seqU8(new byte[] { (byte) 0xDE, (byte) 0xAD, (byte) 0xBE, (byte) 0xEF });

        byte[] frame = ZdwEndpoint.xrceWriteFrame(
            ZdwEndpoint.SESSION_NOKEY, ZdwEndpoint.STREAM_BEST_EFFORT, 1, w.bytes());
        DatagramSocket s = new DatagramSocket();
        s.send(new DatagramPacket(frame, frame.length, InetAddress.getByName(host), port));
        s.close();
        System.out.println("java endpoint: sent " + frame.length
            + "-byte XRCE frame to " + host + ":" + port);
    }
}
