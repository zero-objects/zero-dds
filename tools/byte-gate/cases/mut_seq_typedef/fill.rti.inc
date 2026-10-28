DDS_LongSeq_set_maximum(&sample.plain, 2);
DDS_LongSeq_set_length(&sample.plain, 2);
*DDS_LongSeq_get_reference(&sample.plain, 0) = 1;
*DDS_LongSeq_get_reference(&sample.plain, 1) = 2;
DDS_LongSeq_set_maximum(&sample.aliased, 2);
DDS_LongSeq_set_length(&sample.aliased, 2);
*DDS_LongSeq_get_reference(&sample.aliased, 0) = 3;
*DDS_LongSeq_get_reference(&sample.aliased, 1) = 4;
