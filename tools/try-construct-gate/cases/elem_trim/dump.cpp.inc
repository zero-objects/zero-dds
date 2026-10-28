printf("a=%d n=%zu [", (int)sample.a(), sample.m().size());
for (size_t i = 0; i < sample.m().size(); i++) printf("[%s] ", sample.m()[i].c_str());
printf("] b=%d\n", (int)sample.b());
