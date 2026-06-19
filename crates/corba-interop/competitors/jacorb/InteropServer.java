// SPDX-License-Identifier: Apache-2.0
// JacORB cross-ORB interop server: Echo + Bench, prints IORs to stdout.
// JacORB emittiert Big-Endian-IORs/Requests — testet receiver-makes-right.
import org.omg.CORBA.*;
import org.omg.PortableServer.*;

class EchoImpl extends EchoPOA {
  public String ping(String msg) { return msg; }
}

class BenchImpl extends BenchPOA {
  public int add(int a, int b) { return a + b; }
  public double scale(double x, double f) { return x * f; }
  public long add64(long a, long b) { return a + b; }
  public char next_char(char c) { return (char)(c + 1); }
  public String concat(String a, String b) { return a + b; }
  public String wecho(String w) { return w; }
  public org.omg.CORBA.Any aecho(org.omg.CORBA.Any a) { return a; }
  public int[] reverse(int[] xs) {
    int[] out = new int[xs.length];
    for (int i = 0; i < xs.length; i++) out[i] = xs[xs.length - 1 - i];
    return out;
  }
  public void divmod(int a, int b, IntHolder q, IntHolder r) {
    q.value = a / b; r.value = a % b;
  }
  public void increment(IntHolder x) { x.value = x.value + 1; }
  public org.omg.CORBA.Object echo_ref(org.omg.CORBA.Object o) { return o; }
  public int checked(int idx, int limit) throws RangeError {
    if (idx < limit) return idx;
    throw new RangeError(idx, limit);
  }
}

public class InteropServer {
  public static void main(String[] args) throws Exception {
    ORB orb = ORB.init(args, null);
    POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
    root.the_POAManager().activate();
    org.omg.CORBA.Object eref = root.servant_to_reference(new EchoImpl());
    org.omg.CORBA.Object bref = root.servant_to_reference(new BenchImpl());
    System.out.println("ECHO_IOR=" + orb.object_to_string(eref));
    System.out.println("BENCH_IOR=" + orb.object_to_string(bref));
    System.out.flush();
    orb.run();
  }
}
