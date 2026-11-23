// SPDX-License-Identifier: Apache-2.0
// JacORB 3.9 MIOP sender (CORBA 3.3 Part 2 §11): sends greeting_oneway to
// the group of the corbaloc:miop URL it is given, once with a short string
// and once with a string of 6000 characters (a collection of several
// packets), and prints ALL OK.
import java.util.Properties;

public class MiopClient {
    public static void main(String[] args) throws Exception {
        String url = System.getProperty("miop.url");
        Properties props = new Properties();
        props.setProperty("jacorb.transport.factories",
            "org.jacorb.orb.iiop.IIOPFactories,org.jacorb.orb.miop.MIOPFactories");
        props.setProperty("jacorb.transport.client.selector",
            "org.jacorb.orb.miop.MIOPProfileSelector");
        org.omg.CORBA.ORB orb = org.omg.CORBA.ORB.init(args, props);
        GreetingService group = GreetingServiceHelper.unchecked_narrow(orb.string_to_object(url));
        group.greeting_oneway("short greeting");
        StringBuilder sb = new StringBuilder("long greeting ");
        while (sb.length() < 6000) {
            sb.append("jacorb miop ");
        }
        group.greeting_oneway(sb.substring(0, 6000));
        orb.shutdown(false);
        System.out.println("ALL OK");
    }
}
