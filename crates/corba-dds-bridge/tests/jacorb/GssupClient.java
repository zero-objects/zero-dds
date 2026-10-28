// JacORB CSIv2 client of `tests/jacorb_e2e.rs`: sets GSSUP credentials
// through GssUpContext (JacORB takes them from code, not from a property),
// calls `never` on the bridge and prints the outcome.
import org.jacorb.security.sas.GssUpContext;
import org.omg.CORBA.ORB;
import org.omg.CORBA.SystemException;

import zerodds_test.Calc;
import zerodds_test.CalcHelper;

public class GssupClient {
    public static void main(String[] args) throws Exception {
        if (args.length < 3) {
            System.err.println("usage: GssupClient <ior_file> <user> <password>");
            System.exit(2);
        }
        GssUpContext.setUsernamePassword(args[1], args[2]);
        ORB orb = ORB.init(new String[0], null);
        try {
            Calc never = CalcHelper.narrow(
                    orb.string_to_object(CalcClient.readIors(args[0]).get("never")));
            System.out.println("never " + never.never(1));
        } catch (SystemException e) {
            System.out.println("never EXCEPTION " + e.getClass().getSimpleName());
        }
        orb.shutdown(true);
    }
}
