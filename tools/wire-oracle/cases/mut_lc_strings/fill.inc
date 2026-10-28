static char *ssv[2] = { (char *)"pq", (char *)"rst" };
sample.se[0] = (char *)"ab";
sample.se[1] = (char *)"cde";
strcpy(sample.bs, "fghi");
sample.ss._length = 2; sample.ss._maximum = 2; sample.ss._buffer = ssv; sample.ss._release = false;
sample.tail = 7;
