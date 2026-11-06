// SPDX-License-Identifier: Apache-2.0
// omniORB client of Features and Shapes (features.idl): calls every operation
// on the IORs it is given (a ZeroDDS server's, tests/omniorb_cross_orb.rs)
// and checks each answer. Exit 0 = all checks pass, 1 = a mismatch.
//
// Usage: features_client <FEATURES_IOR> <SHAPES_IOR>
#include <omniORB4/CORBA.h>
#include "features.hh"
#include <cstdio>
#include <cstring>

static int fails = 0;
static void check(const char* name, bool ok) {
  printf("  %s %s\n", ok ? "OK" : "FAIL", name);
  if (!ok) fails++;
}

int main(int argc, char** argv) {
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  if (argc < 3) { fprintf(stderr, "usage: features_client <FEATURES_IOR> <SHAPES_IOR>\n"); return 2; }
  CORBA::Object_var fobj = orb->string_to_object(argv[1]);
  CORBA::Object_var sobj = orb->string_to_object(argv[2]);
  Features_var f = Features::_narrow(fobj);
  Shapes_var s = Shapes::_narrow(sobj);
  if (CORBA::is_nil(f) || CORBA::is_nil(s)) { fprintf(stderr, "narrow failed\n"); return 1; }

  check("add", f->add(2, 3) == 5);
  check("scale", f->scale(2.5, 4.0) == 10.0);
  check("toggle (boolean)", f->toggle(true) == false);
  check("xor_byte (octet)", f->xor_byte(0xF0, 0x0F) == 0xFF);
  check("neg_short (short)", f->neg_short(-7) == 7);
  check("uadd (unsigned long)", f->uadd(10, 20) == 30);
  check("add64 (long long)", f->add64(1000000000000LL, 1) == 1000000000001LL);
  CORBA::String_var c = f->concat("foo", "bar");
  check("concat", strcmp(c, "foobar") == 0);
  const CORBA::WChar win[] = {0x77, 0x00ED, 0x64, 0x65, 0x2D, 0x20AC, 0x2D, 0x03A9, 0};
  CORBA::WString_var wout = f->wecho(win);
  bool wok = wout.in() != 0;
  for (int i = 0; wok && i <= 8; i++) wok = wout[i] == win[i];
  check("wecho (wstring)", wok);
  {
    CORBA::Any a; a <<= (CORBA::Long)-99;
    CORBA::Any_var r = f->aecho(a);
    CORBA::Long v = 0;
    check("aecho (long)", (r.in() >>= v) && v == -99);
    CORBA::Any d; d <<= (CORBA::Double)2.5;
    CORBA::Any_var rd = f->aecho(d);
    CORBA::Double dv = 0;
    check("aecho (double)", (rd.in() >>= dv) && dv == 2.5);
    CORBA::Any st; st <<= "any-string";
    CORBA::Any_var rs = f->aecho(st);
    const char* sv = 0;
    check("aecho (string)", (rs.in() >>= sv) && strcmp(sv, "any-string") == 0);
    CORBA::Any b; b <<= CORBA::Any::from_boolean(true);
    CORBA::Any_var rb = f->aecho(b);
    CORBA::Boolean bv = false;
    check("aecho (boolean)", (rb.in() >>= CORBA::Any::to_boolean(bv)) && bv);
  }
  LongSeq xs; xs.length(3); xs[0] = 1; xs[1] = 2; xs[2] = 3;
  LongSeq_var rx = f->reverse(xs);
  check("reverse (sequence<long>)", rx->length() == 3 && rx[0] == 3 && rx[1] == 2 && rx[2] == 1);
  StringSeq ss; ss.length(2); ss[0] = CORBA::string_dup("a"); ss[1] = CORBA::string_dup("b");
  StringSeq_var rss = f->echo_seq(ss);
  check("echo_seq (sequence<string>)",
        rss->length() == 2 && strcmp(rss[0], "a") == 0 && strcmp(rss[1], "b") == 0);
  CORBA::Long q = 0, r = 0;
  f->divmod(17, 5, q, r);
  check("divmod (out, out)", q == 3 && r == 2);
  CORBA::Long x = 41;
  f->increment(x);
  check("increment (inout)", x == 42);
  CORBA::Long a1 = 1, b1 = 2;
  f->swap(a1, b1);
  check("swap (inout, inout)", a1 == 2 && b1 == 1);
  f->fire(99);
  check("answer (readonly attribute)", f->answer() == 42);
  CORBA::String_var l0 = f->label();
  check("label (attribute)", strcmp(l0, "init") == 0);
  f->label("changed");
  CORBA::String_var l1 = f->label();
  check("label = (attribute set)", strcmp(l1, "changed") == 0);

  Point p; p.x = 1; p.y = 2;
  Point t = s->translate(p, 10, 20);
  check("translate (struct in and return)", t.x == 11 && t.y == 22);
  Point p2; p2.x = 3; p2.y = 4;
  check("sum_point", s->sum_point(p2) == 7);
  CORBA::Long ox = 0, oy = 0;
  Point p3; p3.x = 5; p3.y = 6;
  s->split(p3, ox, oy);
  check("split (out)", ox == 5 && oy == 6);
  Point p4; p4.x = 7; p4.y = 8;
  s->bump(p4);
  check("bump (struct inout)", p4.x == 8 && p4.y == 9);
  check("next_color (enum)", s->next_color(RED) == GREEN && s->next_color(BLUE) == RED);
  Variant_var w = s->wrap_long(123);
  check("wrap_long (union)", w->_d() == 0 && w->as_long() == 123);
  Variant vl; vl.as_long(99);
  check("unwrap_long (union, long)", s->unwrap_long(vl) == 99);
  Variant vs; vs.as_string("abcd");
  check("unwrap_long (union, string)", s->unwrap_long(vs) == 4);
  Variant vf; vf.as_flag(true); vf._d(7);
  check("unwrap_long (union, default member)", s->unwrap_long(vf) == 1);
  check("checked", s->checked(3, 10) == 3);
  bool caught = false;
  try { s->checked(15, 10); }
  catch (const OutOfRange& e) { caught = e.requested == 15 && e.limit == 10; }
  check("checked raises OutOfRange(15, 10)", caught);

  printf("%s\n", fails == 0 ? "ALL OK" : "FAILURES");
  return fails == 0 ? 0 : 1;
}
