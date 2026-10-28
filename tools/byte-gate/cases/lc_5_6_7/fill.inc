static uint8_t bb[3] = {1,2,3};
static int32_t ll[1] = {7};
static double dd[1] = {1.5};
static int16_t hh[2] = {4,5};
sample.s = "abc";
sample.b._length = 3; sample.b._maximum = 3; sample.b._buffer = bb; sample.b._release = false;
sample.l._length = 1; sample.l._maximum = 1; sample.l._buffer = ll; sample.l._release = false;
sample.d._length = 1; sample.d._maximum = 1; sample.d._buffer = dd; sample.d._release = false;
sample.h._length = 2; sample.h._maximum = 2; sample.h._buffer = hh; sample.h._release = false;
