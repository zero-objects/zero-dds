DDS_LongSeq_set_maximum(&sample.rows[0], 2);
DDS_LongSeq_set_length(&sample.rows[0], 2);
*DDS_LongSeq_get_reference(&sample.rows[0], 0) = 1;
*DDS_LongSeq_get_reference(&sample.rows[0], 1) = 2;
DDS_LongSeq_set_maximum(&sample.rows[1], 1);
DDS_LongSeq_set_length(&sample.rows[1], 1);
*DDS_LongSeq_get_reference(&sample.rows[1], 0) = 3;
sample.tail = 0xAB;
