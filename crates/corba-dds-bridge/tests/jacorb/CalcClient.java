// JacORB client of `tests/jacorb_e2e.rs`: reads the bridge's IOR file
// (`<repo_id> <operation> IOR:...` per line), calls `add`, `never`, the add
// object's `never` and `echo`, prints one line per outcome in the form the
// omniORB client prints.
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.Map;

import org.omg.CORBA.ORB;
import org.omg.CORBA.SystemException;

import zerodds_test.Calc;
import zerodds_test.CalcHelper;

public class CalcClient {
    public static void main(String[] args) throws Exception {
        if (args.length < 1) {
            System.err.println("usage: CalcClient <ior_file>");
            System.exit(2);
        }
        Map<String, String> iors = readIors(args[0]);
        ORB orb = ORB.init(new String[0], null);
        try {
            Calc add = CalcHelper.narrow(orb.string_to_object(iors.get("add")));
            System.out.println("add " + add.add(2, 40));
        } catch (SystemException e) {
            System.out.println("add EXCEPTION " + e.getClass().getSimpleName());
        }
        try {
            Calc never = CalcHelper.narrow(orb.string_to_object(iors.get("never")));
            System.out.println("never " + never.never(1));
        } catch (SystemException e) {
            System.out.println("never EXCEPTION " + e.getClass().getSimpleName());
        }
        try {
            // The add object, asked for an operation it does not map.
            Calc add = CalcHelper.narrow(orb.string_to_object(iors.get("add")));
            System.out.println("wrong " + add.never(1));
        } catch (SystemException e) {
            System.out.println("wrong EXCEPTION " + e.getClass().getSimpleName());
        }
        try {
            // 200 000 bytes: above the bridge's fragment_size in both directions.
            Calc echo = CalcHelper.narrow(orb.string_to_object(iors.get("echo")));
            byte[] data = new byte[200000];
            for (int i = 0; i < data.length; i++) {
                data[i] = (byte) (i % 251);
            }
            byte[] back = echo.echo(data);
            boolean same = java.util.Arrays.equals(back, data);
            System.out.println("echo " + back.length + (same ? " same" : " differs"));
        } catch (SystemException e) {
            System.out.println("echo EXCEPTION " + e.getClass().getSimpleName());
        }
        orb.shutdown(true);
    }

    static Map<String, String> readIors(String path) throws Exception {
        Map<String, String> iors = new HashMap<>();
        for (String line : Files.readAllLines(Paths.get(path))) {
            String[] f = line.trim().split("\\s+");
            if (f.length == 3) {
                iors.put(f[1], f[2]);
            }
        }
        return iors;
    }
}
