sample.s("abc");
sample.o1({ 1, 2, 3 });
sample.s2({ -1, 2 });
sample.l4({ 1, 2 });
sample.d8({ 1.5 });
{ Cell c; c.a(1); c.b(2); sample.sc({ c }); }
{ std::array<Cell, 2> a; a[0].a(3); a[0].b(4); a[1].a(5); a[1].b(6); sample.arr(a); }
sample.ap({ 7, 8 });
{ Cell n; n.a(9); n.b(10); sample.nested(n); }
sample.tail(11);
