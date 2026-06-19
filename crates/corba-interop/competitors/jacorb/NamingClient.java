// SPDX-License-Identifier: Apache-2.0
// JacORB CosNaming cross-ORB client: narrows a NamingContext IOR (published by
// ZeroDDS) and drives the real OMG CosNaming wire protocol
// (bind/resolve/rebind/unbind of a Name=NameComponent[]). Proves that a
// Java/JacORB client drives our NamingContext server.
// Usage: NamingClient <NAMING_IOR>
import org.omg.CORBA.*;
import org.omg.CosNaming.*;

public class NamingClient {
  static int fails = 0;
  static void check(String name, boolean ok) {
    System.out.println("  " + (ok ? "OK" : "FAIL") + " " + name);
    if (!ok) fails++;
  }

  public static void main(String[] args) {
    ORB orb = ORB.init(args, null);
    org.omg.CORBA.Object obj = orb.string_to_object(args[0]);
    NamingContext nc = NamingContextHelper.narrow(obj);
    if (nc == null) { System.err.println("narrow NamingContext failed"); System.exit(1); }

    // Single-level name → root-level binding.
    NameComponent[] name = new NameComponent[] { new NameComponent("jacorb-test", "obj") };
    try {
      nc.bind(name, nc);
      org.omg.CORBA.Object got = nc.resolve(name);
      check("resolve returns non-nil ref", got != null);
      nc.rebind(name, nc);
      nc.unbind(name);
      check("bind/resolve/rebind/unbind roundtrip", true);
      // resolve after unbind → typed NotFound exception (cross-ORB).
      boolean caught = false;
      try { nc.resolve(name); }
      catch (org.omg.CosNaming.NamingContextPackage.NotFound nf) { caught = true; }
      check("resolve(unbound) → NotFound", caught);
      // Federation: bind_new_context + leaf via sub-ref + compound resolve from root.
      NameComponent[] ctxName = new NameComponent[] { new NameComponent("sub", "ctx") };
      NamingContext sub = nc.bind_new_context(ctxName);
      NameComponent[] leaf = new NameComponent[] { new NameComponent("leaf", "obj") };
      sub.bind(leaf, nc);
      NameComponent[] compound = new NameComponent[] {
          new NameComponent("sub", "ctx"), new NameComponent("leaf", "obj") };
      org.omg.CORBA.Object fedGot = nc.resolve(compound);
      check("federation: bind_new_context + compound resolve", fedGot != null);
    } catch (Exception e) {
      System.err.println("naming op failed: " + e);
      fails++;
    }
    System.out.println(fails == 0 ? "ALL OK" : "FAILURES");
    System.exit(fails == 0 ? 0 : 1);
  }
}
