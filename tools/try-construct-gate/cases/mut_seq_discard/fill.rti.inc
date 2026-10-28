sample.a = 7;
DDS_LongSeq_ensure_length(&sample.m, 8, 32);
{ int i; for (i = 0; i < 8; i++) *DDS_LongSeq_get_reference(&sample.m, i) = i + 1; }
sample.b = 9;
