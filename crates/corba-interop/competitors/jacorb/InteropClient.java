// SPDX-License-Identifier: Apache-2.0
// JacORB cross-ORB interop client: calls an IOR (produced by ZeroDDS).
// Usage: InteropClient echo|bench <IOR>
import org.omg.CORBA.*;

public class InteropClient {
  static int fails = 0;
  static void check(String name, boolean ok) {
    System.out.println("  " + (ok ? "OK" : "FAIL") + " " + name);
    if (!ok) fails++;
  }

  public static void main(String[] args) {
    ORB orb = ORB.init(args, null);
    String mode = args[0];
    org.omg.CORBA.Object obj = orb.string_to_object(args[1]);

    if (mode.equals("echo")) {
      Echo echo = EchoHelper.narrow(obj);
      check("ping(small)", echo.ping("orb<->zerodds").equals("orb<->zerodds"));
      StringBuilder sb = new StringBuilder();
      for (int i = 0; i < 4096; i++) sb.append('x');
      String big = sb.toString();
      check("ping(4k)", echo.ping(big).equals(big));
    } else if (mode.equals("bench")) {
      Bench b = BenchHelper.narrow(obj);
      check("add", b.add(2, 3) == 5);
      check("scale (double, 8-aligned)", b.scale(2.5, 4.0) == 10.0);
      check("add64 (long long, 8-aligned)", b.add64(1000000000000L, 1) == 1000000000001L);
      check("next_char (char = 1 byte)", b.next_char('A') == 'B');
      check("concat", b.concat("foo", "bar").equals("foobar"));
      // wstring (UTF-16 wire, §15.3.1.6): non-ASCII roundtrip.
      String wide = "wíde-€-Ω";
      check("wecho (wstring/UTF-16)", b.wecho(wide).equals(wide));
      // Structured any (§15.3.5 TypeCode): struct AnyPair + sequence<long>.
      AnyPair p = new AnyPair(7, "seven");
      org.omg.CORBA.Any ain = orb.create_any();
      AnyPairHelper.insert(ain, p);
      org.omg.CORBA.Any aout = b.aecho(ain);
      AnyPair pr = AnyPairHelper.extract(aout);
      check("aecho(struct AnyPair)", pr.k == 7 && pr.v.equals("seven"));
      int[] ls = {10, 20, 30};
      org.omg.CORBA.Any sin = orb.create_any();
      LongSeqHelper.insert(sin, ls);
      org.omg.CORBA.Any sout = b.aecho(sin);
      int[] lr = LongSeqHelper.extract(sout);
      check("aecho(sequence<long>)", lr.length == 3 && lr[0] == 10 && lr[2] == 30);
      int[] rev = b.reverse(new int[]{1, 2, 3});
      check("reverse", rev.length == 3 && rev[0] == 3 && rev[1] == 2 && rev[2] == 1);
      IntHolder q = new IntHolder(), r = new IntHolder();
      b.divmod(17, 5, q, r);
      check("divmod.q", q.value == 3);
      check("divmod.r", r.value == 2);
      IntHolder x = new IntHolder(41);
      b.increment(x);
      check("increment", x.value == 42);
      // Object reference: pass our own Bench ref through, narrow it back, call it live.
      org.omg.CORBA.Object ret = b.echo_ref(b);
      Bench rb = BenchHelper.narrow(ret);
      check("echo_ref (Object/IOR)", rb != null && rb.add(2, 3) == 5);
      // Typed user exception: checked(15,10) must raise RangeError(15,10).
      try { check("checked(ok)", b.checked(3, 10) == 3); }
      catch (RangeError e) { check("checked(ok)", false); }
      boolean caught = false;
      try { b.checked(15, 10); }
      catch (RangeError e) { caught = (e.requested == 15 && e.limit == 10); }
      check("checked raises RangeError(15,10)", caught);
    } else {
      System.err.println("unknown mode " + mode);
      System.exit(2);
    }
    System.out.println(fails == 0 ? "ALL OK" : "FAILURES");
    System.exit(fails == 0 ? 0 : 1);
  }
}
