static int32_t plain_els[2] = { 1, 2 };
static int32_t alias_els[2] = { 3, 4 };
sample.plain._length = 2; sample.plain._maximum = 2;
sample.plain._buffer = plain_els; sample.plain._release = false;
sample.aliased._length = 2; sample.aliased._maximum = 2;
sample.aliased._buffer = alias_els; sample.aliased._release = false;
