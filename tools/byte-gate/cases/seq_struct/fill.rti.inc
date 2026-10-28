CellSeq_set_maximum(&sample.cells, 2);
CellSeq_set_length(&sample.cells, 2);
CellSeq_get_reference(&sample.cells, 0)->a = 1;
CellSeq_get_reference(&sample.cells, 0)->b = 2;
CellSeq_get_reference(&sample.cells, 1)->a = 3;
CellSeq_get_reference(&sample.cells, 1)->b = 4;
