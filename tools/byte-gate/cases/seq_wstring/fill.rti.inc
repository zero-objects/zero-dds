static __typeof__(*(*DDS_WstringSeq_get_reference(&sample.texts, 0))) zd_wstr_1[] = { 0x0061, 0x0062, 0x0000 };
static __typeof__(*(*DDS_WstringSeq_get_reference(&sample.texts, 1))) zd_wstr_2[] = { 0x0063, 0x0064, 0x0000 };
DDS_WstringSeq_set_maximum(&sample.texts, 2);
DDS_WstringSeq_set_length(&sample.texts, 2);
(*DDS_WstringSeq_get_reference(&sample.texts, 0)) = zd_wstr_1;
(*DDS_WstringSeq_get_reference(&sample.texts, 1)) = zd_wstr_2;
sample.tail = 171;
