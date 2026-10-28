printf("a=%d n=%u [", (int)sample.a, (unsigned)sample.m.length());
for (CORBA::ULong i = 0; i < sample.m.length(); i++) printf("%d ", (int)sample.m[i]);
printf("] b=%d\n", (int)sample.b);
