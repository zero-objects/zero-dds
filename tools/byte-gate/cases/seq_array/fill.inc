static int32_t r0[2] = { 1, 2 };
static int32_t r1[1] = { 3 };
sample.rows[0]._length = 2; sample.rows[0]._maximum = 2; sample.rows[0]._buffer = r0; sample.rows[0]._release = false;
sample.rows[1]._length = 1; sample.rows[1]._maximum = 1; sample.rows[1]._buffer = r1; sample.rows[1]._release = false;
sample.tail = 0xAB;
