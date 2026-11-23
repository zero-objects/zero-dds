// SPDX-License-Identifier: Apache-2.0
// JacORB 3.9 MIOP receiver (CORBA 3.3 Part 2 §11): joins the group of the
// corbaloc:miop URL it is given through PortableGroup::GOA, prints the
// group reference as GROUP=IOR:... and every request it receives as
// RECEIVED <length> <first 16 characters>.
import java.util.Properties;
import org.omg.PortableGroup.GOA;
import org.omg.PortableGroup.GOAHelper;

public class MiopServer {
    static class Greeting extends GreetingServicePOA {
        public void greeting_oneway(String s) {
            System.out.println("RECEIVED " + s.length() + " " + s.substring(0, Math.min(16, s.length())));
            System.out.flush();
        }
    }

    public static void main(String[] args) throws Exception {
        String url = System.getProperty("miop.url");
        Properties props = new Properties();
        props.setProperty("jacorb.transport.factories",
            "org.jacorb.orb.iiop.IIOPFactories,org.jacorb.orb.miop.MIOPFactories");
        props.setProperty("jacorb.transport.client.selector",
            "org.jacorb.orb.miop.MIOPProfileSelector");
        org.omg.CORBA.ORB orb = org.omg.CORBA.ORB.init(args, props);
        org.omg.CORBA.Object group = orb.string_to_object(url);
        org.omg.PortableServer.POA poa =
            org.omg.PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        poa.the_POAManager().activate();
        GOA goa = GOAHelper.narrow(poa);
        byte[] oid = poa.activate_object(new Greeting());
        goa.associate_reference_with_id(group, oid);
        System.out.println("GROUP=" + orb.object_to_string(group));
        System.out.flush();
        orb.run();
    }
}
