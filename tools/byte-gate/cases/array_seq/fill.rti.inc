RowSeq_set_maximum(&sample.rows, 2);
RowSeq_set_length(&sample.rows, 2);
{ Row *e0 = RowSeq_get_reference(&sample.rows, 0); (*e0)[0]=1; (*e0)[1]=2; (*e0)[2]=3;
  Row *e1 = RowSeq_get_reference(&sample.rows, 1); (*e1)[0]=4; (*e1)[1]=5; (*e1)[2]=6; }
sample.tail = 0xAB;
