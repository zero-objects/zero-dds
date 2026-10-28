// JacORB servant of `tests/jacorb_e2e.rs`: a Listener reachable as
// corbaloc::127.0.0.1:<port>/Listener; prints every on_value and exits after
// `count` of them or 15 seconds. The servant lives in a SINGLE_THREAD_MODEL
// POA: under JacORB's default ORB_CTRL_MODEL, oneways that arrive in order on
// one connection were measured reaching the servant out of order.
import java.util.Properties;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.PortableServer.ThreadPolicyValue;

import zerodds_test.ListenerPOA;

public class ListenerServer {
    static final class ListenerImpl extends ListenerPOA {
        private final CountDownLatch remaining;

        ListenerImpl(CountDownLatch remaining) {
            this.remaining = remaining;
        }

        @Override
        public synchronized void on_value(long v) {
            System.out.println("on_value " + v);
            System.out.flush();
            remaining.countDown();
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("usage: ListenerServer <port> <count>");
            System.exit(2);
        }
        Properties p = new Properties();
        p.setProperty("OAIAddr", "127.0.0.1");
        p.setProperty("OAPort", args[0]);
        ORB orb = ORB.init(new String[0], p);
        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
        CountDownLatch remaining = new CountDownLatch(Integer.parseInt(args[1]));
        POA serial = root.create_POA(
                "listener",
                root.the_POAManager(),
                new Policy[] {root.create_thread_policy(ThreadPolicyValue.SINGLE_THREAD_MODEL)});
        byte[] oid = serial.activate_object(new ListenerImpl(remaining));
        org.omg.CORBA.Object ref = serial.id_to_reference(oid);
        // JacORB maps a corbaloc object key to a reference through addObjectKey.
        ((org.jacorb.orb.ORB) orb).addObjectKey("Listener", orb.object_to_string(ref));
        System.out.println("ready");
        System.out.flush();
        remaining.await(15, TimeUnit.SECONDS);
        orb.shutdown(false);
    }
}
