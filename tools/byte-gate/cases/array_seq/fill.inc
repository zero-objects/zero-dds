static Row rows[2] = { { 1, 2, 3 }, { 4, 5, 6 } };
sample.rows._length = 2;
sample.rows._maximum = 2;
sample.rows._buffer = rows;
sample.rows._release = false;
sample.tail = 0xAB;
