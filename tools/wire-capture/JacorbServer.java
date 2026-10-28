import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import java.io.PrintWriter;
import java.util.Properties;

public class Server {
    static class SinkImpl extends SinkPOA {
        public void put(GapProbe g) {
            System.out.println("put s.len=" + g.s.length() + " b=" + g.b);
        }
    }
    public static void main(String[] args) throws Exception {
        Properties props = new Properties();
        props.put("org.omg.CORBA.ORBClass", "org.jacorb.orb.ORB");
        props.put("org.omg.CORBA.ORBSingletonClass", "org.jacorb.orb.ORBSingleton");
        props.put("OAIAddr", "127.0.0.1");
        props.put("OAPort", "12347");
        ORB orb = ORB.init(args, props);
        POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        poa.the_POAManager().activate();
        SinkImpl impl = new SinkImpl();
        org.omg.CORBA.Object ref = poa.servant_to_reference(impl);
        try (PrintWriter w = new PrintWriter("/root/gapprobe/jacorb/ior.txt")) {
            w.println(orb.object_to_string(ref));
        }
        orb.run();
    }
}
