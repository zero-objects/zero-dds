#include "EchoC.h"
#include <fstream>
#include <vector>
#include <algorithm>
#include <chrono>
#include <string>
#include <cstdio>
int main(int argc, char** argv){
  CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
  std::ifstream f("/tmp/echo_tao.ior"); std::string ior; std::getline(f, ior);
  CORBA::Object_var obj = orb->string_to_object(ior.c_str());
  Echo_var echo = Echo::_narrow(obj.in());
  int payload = argc>1?atoi(argv[1]):56;
  int n = argc>2?atoi(argv[2]):50000;
  std::string msg(payload, (char)120);
  for(int i=0;i<2000;i++){ CORBA::String_var r = echo->ping(msg.c_str()); }
  std::vector<long> s; s.reserve(n);
  for(int i=0;i<n;i++){
    auto t0=std::chrono::steady_clock::now();
    CORBA::String_var r = echo->ping(msg.c_str());
    s.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now()-t0).count());
  }
  std::sort(s.begin(), s.end());
  auto us=[&](double q){ size_t i=(size_t)(n*q); if(i>=(size_t)n)i=n-1; return s[i]/1000.0; };
  printf("TAO Echo roundtrip (IIOP loopback, payload=%dB, N=%d)\n", payload, n);
  printf("  min=%.1fus  p50=%.1fus  p90=%.1fus  p99=%.1fus  p99.9=%.1fus\n", s[0]/1000.0, us(0.50), us(0.90), us(0.99), us(0.999));
  return 0;
}
