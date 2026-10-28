{ ACell a; a.a(1); a.b(2); sample.app(a); }
{ MCell m; m.a(3); m.b(4); sample.mnest(m); }
sample.ce({ Colour::GREEN, Colour::BLUE });
{ std::array<Cell, 1> a; a[0].a(5); a[0].b(6); sample.arr1(a); }
sample.tail(7);
