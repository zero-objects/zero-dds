import org.omg.CORBA.*;
import org.omg.PortableServer.*;
import java.io.*;
class EchoImpl extends EchoPOA { public String ping(String msg){ return msg; } }
public class Server {
  public static void main(String[] a) throws Exception {
    ORB orb = ORB.init(a, null);
    POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
    root.the_POAManager().activate();
    org.omg.CORBA.Object ref = root.servant_to_reference(new EchoImpl());
    PrintWriter w = new PrintWriter(new FileWriter("/tmp/echo_jacorb.ior"));
    w.println(orb.object_to_string(ref)); w.close();
    orb.run();
  }
}
