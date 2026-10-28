static enum Colour csv[2] = { GREEN, BLUE };
static Flags fsv[2] = { 1, 8 };
sample.cs._length = 2; sample.cs._maximum = 2; sample.cs._buffer = csv; sample.cs._release = false;
sample.fs._length = 2; sample.fs._maximum = 2; sample.fs._buffer = fsv; sample.fs._release = false;
sample.tail = 7;
