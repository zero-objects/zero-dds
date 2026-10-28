printf("a=%d n=%u [", (int)sample.a, (unsigned)sample.m.length());
for (CORBA::ULong i = 0; i < sample.m.length(); i++) printf("[%s] ", sample.m[i].in());
printf("] b=%d\n", (int)sample.b);
