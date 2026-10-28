DDS_LongSeq_set_maximum(&sample.ids, 3);
DDS_LongSeq_set_length(&sample.ids, 3);
*DDS_LongSeq_get_reference(&sample.ids, 0) = 1;
*DDS_LongSeq_get_reference(&sample.ids, 1) = 2;
*DDS_LongSeq_get_reference(&sample.ids, 2) = 3;
