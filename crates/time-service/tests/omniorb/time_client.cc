// omniORB client of the ZeroDDS Time Service (tests/omniorb_e2e.rs).
// argv[1]: corbaloc of the TimeService object. Prints one line per check.

#include <cstdio>
#include "CosTime.hh"

int main(int argc, char** argv) {
    CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
    if (argc < 2) {
        std::printf("usage: time_client <corbaloc>\n");
        return 2;
    }
    try {
        CORBA::Object_var obj = orb->string_to_object(argv[1]);
        CosTime::TimeService_var ts = CosTime::TimeService::_narrow(obj);
        if (CORBA::is_nil(ts)) {
            std::printf("narrow failed\n");
            return 1;
        }

        try {
            CosTime::UTO_var now = ts->universal_time();
            TimeBase::UtcT utc = now->utc_time();
            std::printf("universal_time %llu %u %u %d\n",
                        (unsigned long long)utc.time, (unsigned)utc.inacclo,
                        (unsigned)utc.inacchi, (int)utc.tdf);
            std::printf("attributes %llu %llu %d\n", (unsigned long long)now->time(),
                        (unsigned long long)now->inaccuracy(), (int)now->tdf());
        } catch (CosTime::TimeUnavailable&) {
            std::printf("universal_time TimeUnavailable\n");
        } catch (CORBA::NO_PERMISSION&) {
            std::printf("universal_time NO_PERMISSION\n");
        }

        // absolute_time of a UTO of time 0: the current time of the
        // service (Time Service 1.1 §1.3.4.5).
        try {
            CosTime::UTO_var zero = ts->new_universal_time(0, 0, 0);
            CosTime::UTO_var abs = zero->absolute_time();
            TimeBase::UtcT utc = abs->utc_time();
            std::printf("absolute_time %llu %llu %d\n", (unsigned long long)utc.time,
                        (unsigned long long)abs->inaccuracy(), (int)utc.tdf);
        } catch (CORBA::NO_PERMISSION&) {
            std::printf("absolute_time NO_PERMISSION\n");
        }

        try {
            ts->secure_universal_time();
            std::printf("secure_universal_time returned\n");
        } catch (CosTime::TimeUnavailable&) {
            std::printf("secure_universal_time TimeUnavailable\n");
        }

        try {
            ts->new_universal_time(1, 0x1000000000000ULL, 0);
            std::printf("new_universal_time accepted\n");
        } catch (CORBA::BAD_PARAM&) {
            std::printf("new_universal_time BAD_PARAM\n");
        }

        try {
            ts->new_interval(10, 5);
            std::printf("new_interval accepted\n");
        } catch (CORBA::BAD_PARAM&) {
            std::printf("new_interval BAD_PARAM\n");
        }

        CosTime::UTO_var a = ts->new_universal_time(1000, 50, 0);
        CosTime::UTO_var b = ts->new_universal_time(2000, 50, 0);
        std::printf("compare_time %d %d\n", (int)a->compare_time(CosTime::MidC, b),
                    (int)a->compare_time(CosTime::IntervalC, a));

        CosTime::TIO_var span = a->time_to_interval(b);
        TimeBase::IntervalT iv = span->time_interval();
        std::printf("time_to_interval %llu %llu\n", (unsigned long long)iv.lower_bound,
                    (unsigned long long)iv.upper_bound);

        CosTime::TIO_var overlap;
        CosTime::OverlapType ot = span->spans(a, overlap.out());
        TimeBase::IntervalT ov = overlap->time_interval();
        std::printf("spans %d %llu %llu\n", (int)ot, (unsigned long long)ov.lower_bound,
                    (unsigned long long)ov.upper_bound);

        CosTime::UTO_var mid = span->time();
        std::printf("time %llu %llu\n", (unsigned long long)mid->time(),
                    (unsigned long long)mid->inaccuracy());

        TimeBase::UtcT in;
        in.time = 777;
        in.inacclo = 1;
        in.inacchi = 2;
        in.tdf = -60;
        CosTime::UTO_var from = ts->uto_from_utc(in);
        TimeBase::UtcT back = from->utc_time();
        std::printf("uto_from_utc %llu %u %u %d\n", (unsigned long long)back.time,
                    (unsigned)back.inacclo, (unsigned)back.inacchi, (int)back.tdf);
    } catch (CORBA::SystemException& e) {
        std::printf("system exception %s\n", e._name());
        orb->destroy();
        return 1;
    }
    orb->destroy();
    return 0;
}
