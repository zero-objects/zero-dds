// JacORB client of `tests/jacorb_e2e.rs`: resolves a name through a
// `corbaname:` URL and calls `add` on what it gets back.
import org.omg.CORBA.ORB;
import org.omg.CORBA.SystemException;

import zerodds_test.Calc;
import zerodds_test.CalcHelper;

public class NamingClient {
    public static void main(String[] args) throws Exception {
        if (args.length < 1) {
            System.err.println("usage: NamingClient <corbaname-url>");
            System.exit(2);
        }
        ORB orb = ORB.init(new String[0], null);
        try {
            Calc add = CalcHelper.narrow(orb.string_to_object(args[0]));
            System.out.println("add " + add.add(2, 40));
        } catch (SystemException e) {
            System.out.println("add EXCEPTION " + e.getClass().getSimpleName());
        }
        orb.shutdown(true);
    }
}
