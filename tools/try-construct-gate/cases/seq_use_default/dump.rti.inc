{ int i, n = DDS_LongSeq_get_length(&sample.m); printf("a=%d n=%d [", (int)sample.a, n);
  for (i = 0; i < n; i++) printf("%d ", (int)*DDS_LongSeq_get_reference(&sample.m, i));
  printf("] b=%d\n", (int)sample.b); }
