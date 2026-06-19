// SPDX-License-Identifier: Apache-2.0
// omniORB cross-ORB interop client: parses a stringified IOR (produced by
// ZeroDDS) and calls Echo or Bench — proof that a foreign ORB drives a
// ZeroDDS server. Exit 0 = all assertions pass, 1 = mismatch.
//
// Usage: interop_client echo|bench <IOR>
#include <omniORB4/CORBA.h>
#include "interop.hh"
#include <cstdio>
#include <cstring>
#include <string>
#include <stdlib.h>

static int fails = 0;
static void check(const char* name, bool ok) {
  printf("  %s %s\n", ok ? "OK" : "FAIL", name);
  if (!ok) fails++;
}

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  if (argc < 3) { fprintf(stderr, "usage: interop_client echo|bench <IOR>\n"); return 2; }
  const char* mode = argv[1];
  CORBA::Object_var obj = orb->string_to_object(argv[2]);

  if (strcmp(mode, "echo") == 0) {
    Echo_var echo = Echo::_narrow(obj);
    if (CORBA::is_nil(echo)) { fprintf(stderr, "narrow Echo failed\n"); return 1; }
    CORBA::String_var r = echo->ping("orb<->zerodds");
    check("ping(small)", strcmp(r, "orb<->zerodds") == 0);
    std::string big(4096, 'x');
    CORBA::String_var r2 = echo->ping(big.c_str());
    check("ping(4k)", big == (const char*)r2);
  } else if (strcmp(mode, "bench") == 0) {
    Bench_var b = Bench::_narrow(obj);
    if (CORBA::is_nil(b)) { fprintf(stderr, "narrow Bench failed\n"); return 1; }
    check("add", b->add(2, 3) == 5);
    check("scale (double, 8-aligned)", b->scale(2.5, 4.0) == 10.0);
    check("add64 (long long, 8-aligned)", b->add64(1000000000000LL, 1) == 1000000000001LL);
    check("next_char (char = 1 byte)", b->next_char('A') == 'B');
    CORBA::String_var c = b->concat("foo", "bar");
    check("concat", strcmp(c, "foobar") == 0);
    // wstring (UTF-16 wire with BOM §15.3.1.6): "wíde-€-Ω" as WChar units.
    const CORBA::WChar win[] = {0x77, 0x00ED, 0x64, 0x65, 0x2D, 0x20AC, 0x2D, 0x03A9, 0};
    CORBA::WString_var wout = b->wecho(win);
    bool wok = (wout.in() != 0);
    if (wok) for (int i = 0; i <= 8; i++) { if (wout[i] != win[i]) { wok = false; break; } }
    check("wecho (wstring/UTF-16 BOM)", wok);
    // Structured any (§15.3.5 TypeCode): struct AnyPair into a CORBA::Any
    // (generated TypeCode) → ZeroDDS decodes + re-encodes → extracted back.
    {
      AnyPair p; p.k = 7; p.v = CORBA::string_dup("seven");
      CORBA::Any ain; ain <<= p;
      CORBA::Any_var aout = b->aecho(ain);
      AnyPair* pr = 0;
      bool ok = (aout.in() >>= pr) && pr->k == 7 && strcmp(pr->v, "seven") == 0;
      check("aecho(struct AnyPair)", ok);
      LongSeq ls; ls.length(3); ls[0] = 10; ls[1] = 20; ls[2] = 30;
      CORBA::Any sin; sin <<= ls;
      CORBA::Any_var sout = b->aecho(sin);
      LongSeq* lr = 0;
      bool ok2 = (sout.in() >>= lr) && lr->length() == 3 && (*lr)[0] == 10 && (*lr)[2] == 30;
      check("aecho(sequence<long>)", ok2);
    }
    LongSeq in;
    in.length(3); in[0] = 1; in[1] = 2; in[2] = 3;
    LongSeq_var out = b->reverse(in);
    check("reverse", out->length() == 3 && out[0] == 3 && out[1] == 2 && out[2] == 1);
    CORBA::Long q = 0, r = 0;
    b->divmod(17, 5, q, r);
    check("divmod.q", q == 3);
    check("divmod.r", r == 2);
    CORBA::Long x = 41;
    b->increment(x);
    check("increment", x == 42);
    // Object reference: pass our own Bench ref through, narrow it back, call it live.
    CORBA::Object_var ret = b->echo_ref(b);
    Bench_var rb = Bench::_narrow(ret);
    check("echo_ref (Object/IOR)", !CORBA::is_nil(rb) && rb->add(2, 3) == 5);
    // Typed user exception: checked(15,10) must raise RangeError(15,10).
    check("checked(ok)", b->checked(3, 10) == 3);
    bool caught = false;
    try { b->checked(15, 10); }
    catch (const RangeError& e) { caught = (e.requested == 15 && e.limit == 10); }
    check("checked raises RangeError(15,10)", caught);
  } else {
    fprintf(stderr, "unknown mode %s\n", mode);
    return 2;
  }
  printf("%s\n", fails == 0 ? "ALL OK" : "FAILURES");
  return fails == 0 ? 0 : 1;
}
