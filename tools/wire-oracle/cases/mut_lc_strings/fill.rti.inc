sample.se[0] = DDS_String_dup("ab");
sample.se[1] = DDS_String_dup("cde");
sample.bs = DDS_String_dup("fghi");
DDS_StringSeq_set_maximum(&sample.ss, 2); DDS_StringSeq_set_length(&sample.ss, 2);
*DDS_StringSeq_get_reference(&sample.ss, 0) = DDS_String_dup("pq");
*DDS_StringSeq_get_reference(&sample.ss, 1) = DDS_String_dup("rst");
sample.tail = 7;
