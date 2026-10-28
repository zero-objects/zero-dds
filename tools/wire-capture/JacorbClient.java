import org.omg.CORBA.ORB;
import java.util.Properties;

public class Client {
    public static void main(String[] args) throws Exception {
        Properties props = new Properties();
        props.put("org.omg.CORBA.ORBClass", "org.jacorb.orb.ORB");
        props.put("org.omg.CORBA.ORBSingletonClass", "org.jacorb.orb.ORBSingleton");
        ORB orb = ORB.init(args, props);
        Sink sink = SinkHelper.narrow(orb.string_to_object(args[0]));

        GapProbe a = new GapProbe();
        a.s = new String(new char[20]).replace("\0", "A");
        a.b = 0x11223344; a.c = (byte) 0x55; a.d = 1.5;
        a.e = (byte) 0x66; a.f = (short) 0x7788;

        GapProbe b = new GapProbe();
        b.s = new String(new char[16]).replace("\0", "B");
        b.b = 0x01020304; b.c = (byte) 0x05; b.d = 2.5;
        b.e = (byte) 0x06; b.f = (short) 0x0708;

        for (int i = 0; i < 5; i++) {
            sink.put(a); Thread.sleep(150);
            sink.put(b); Thread.sleep(150);
        }
        System.out.println("sent 10 calls");
        orb.shutdown(true);
    }
}
