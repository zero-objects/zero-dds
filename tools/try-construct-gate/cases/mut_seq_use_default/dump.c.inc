{ uint32_t i; printf("a=%d n=%u [", (int)sample.a, (unsigned)sample.m._length);
  for (i = 0; i < sample.m._length; i++) printf("%d ", (int)sample.m._buffer[i]);
  printf("] b=%d\n", (int)sample.b); }
