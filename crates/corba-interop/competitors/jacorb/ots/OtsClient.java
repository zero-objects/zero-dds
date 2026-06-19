// SPDX-License-Identifier: Apache-2.0
// Live cross-ORB OTS client (codepit, JDK8, JacORB 3.9): begins a real JacORB
// transaction via the OTS `Current` (resolved from a running TransactionService
// `ts` daemon) and invokes a ZeroDDS server. JacORB's
// ClientContextTransferInterceptor attaches the OTS PropagationContext as IIOP
// service context id=0; the ZeroDDS server captures + decodes it.
//
// args: <IOR> [orb-args...]  (run_ots_client.sh passes -ORBInitRef NameService=…)
// Requires the OTS ORBInitializer:
//   -Dorg.omg.PortableInterceptor.ORBInitializerClass.transaction=org.jacorb.transaction.TransactionInitializer
import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.CosTransactions.*;

public class OtsClient {
    public static void main(String[] a) throws Exception {
        if (a.length < 1) {
            System.err.println("usage: OtsClient <IOR> [orb-args...]");
            System.exit(2);
        }
        // a[0] = ZeroDDS IOR; a[1..] = ORB args (-ORBInitRef NameService=…).
        String[] orbArgs = new String[a.length - 1];
        System.arraycopy(a, 1, orbArgs, 0, a.length - 1);
        ORB orb = ORB.init(orbArgs, null);
        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();

        // OTS Current from the running TransactionService (resolved at ORB init
        // by the TransactionInitializer via the NameService).
        Current tc = CurrentHelper.narrow(orb.resolve_initial_references("TransactionCurrent"));
        tc.begin(); // builds the PropagationContext + sets the PICurrent slot
        System.out.println("OTS_TX_BEGUN");

        org.omg.CORBA.Object target = orb.string_to_object(a[0]);
        org.omg.CORBA.Request req = target._request("ping");
        req.add_in_arg().insert_string("hi");
        req.set_return_type(orb.get_primitive_tc(org.omg.CORBA.TCKind.tk_string));
        try {
            req.invoke(); // client interceptor attaches SC id=0 before the call
            System.out.println("OTS_CLIENT_INVOKED ok");
        } catch (Exception e) {
            // Even on a reply error the SC id=0 was already transmitted.
            System.out.println("OTS_CLIENT_INVOKED (reply: " + e + ")");
        }
        try {
            tc.rollback();
        } catch (Exception e) {
            // best-effort cleanup
        }
        System.exit(0);
    }
}
