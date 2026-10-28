ACellSeq_set_maximum(&sample.cells, 2);
ACellSeq_set_length(&sample.cells, 2);
ACellSeq_get_reference(&sample.cells, 0)->a = 1;
ACellSeq_get_reference(&sample.cells, 0)->b = 2;
ACellSeq_get_reference(&sample.cells, 1)->a = 3;
ACellSeq_get_reference(&sample.cells, 1)->b = 4;
