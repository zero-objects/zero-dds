sample.a = 7;
S_String16Seq_ensure_length(&sample.m, 1, 10);
strcpy(*S_String16Seq_get_reference(&sample.m, 0), "Hello World");
sample.b = 9;
