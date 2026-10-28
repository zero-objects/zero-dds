// SPDX-License-Identifier: Apache-2.0
//
// ZeroDDS C-FFI roundtrip bench app. Apples-to-apples with Cyclone/Fast-DDS/RTI/OpenDDS.
//
// IDL: roundtrip.idl -> RoundtripBench::Roundtrip (via gen/zerodds/Roundtrip.hpp,
// XCDR2; the ZERODDS_BENCH_XCDR1 build takes gen/zerodds-xcdr1/Roundtrip.hpp).
// With BENCH_RICH defined (target zerodds-roundtrip-rich) the sample type is
// the codec-heavy roundtrip_rich.idl → RoundtripBench::RoundtripRich
// (gen/zerodds/RoundtripRich.hpp), on the topics RoundtripRichBench_* of the
// other rich builds.
//
// **Fully event-driven** — no busy-poll. Both roles hang on the
// data-available callback (`zerodds_reader_set_data_callback`), which the
// recv thread fires right after sample arrival:
//   * pong: callback decodes the sample and re-encodes it (typed,
//     no raw-byte echo) -> the peer's XCDR representation is
//     korrekt interpretiert.
//   * ping: callback captures the RTT and wakes the sender thread via
//     a condition variable.
// The callback gets the XCDR version (0=XCDR1, 1=XCDR2) as an argument
// — `decode` needs it for the alignment rule.
//
// Build via CMakeLists.txt.

#include "zerodds.h"
#ifdef BENCH_RICH
#include "RoundtripRich.hpp"
#else
#include "Roundtrip.hpp"
#endif
#include "bench_common.hpp"

#include <array>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

// Phase-Timing-Hook im ZeroDDS-Rust-Runtime — print on demand
// (Drop path fires only at process exit, because the worker threads
// hold the Arc refcount; hence call it explicitly here).
extern "C" void zerodds_phase_dump();

namespace {

using bench::now_ns;
using bench::print_quantiles;

// Domain via ZERODDS_BENCH_DOMAIN (bench_common.hpp): lets the bench runner
// use its own domain ID per cell, so multi-process discovery state does not
// leak between sequential cells.
static uint32_t resolve_domain() {
    return static_cast<uint32_t>(bench::env_domain());
}

#ifdef BENCH_RICH
using Sample = RoundtripBench::RoundtripRich;
constexpr const char* kProgram   = "zerodds-app-rich";
constexpr const char* kCodecFile = "gen/zerodds/RoundtripRich.hpp";
constexpr const char* kReqTopic  = "RoundtripRichBench_Request";
constexpr const char* kEchoTopic = "RoundtripRichBench_Echo";
constexpr const char* kTypeName  = "RoundtripBench::RoundtripRich";
constexpr size_t kNumWaypoints = 8;

void populate(Sample& m, size_t payload_size) {
    using RoundtripBench::Vec3;
    using RoundtripBench::Waypoint;
    m.name = std::string("zerodds-rich-roundtrip-bench");
    std::array<double, 16> tr{};
    for (size_t i = 0; i < 16; ++i) tr[i] = static_cast<double>(i) * 1.5 + 0.25;
    m.transform = tr;
    std::vector<Waypoint> wps;
    wps.reserve(kNumWaypoints);
    for (size_t i = 0; i < kNumWaypoints; ++i) {
        Waypoint w;
        w.id = static_cast<uint32_t>(i);
        Vec3 pos; pos.x = 1.0 * i; pos.y = 2.0 * i; pos.z = 3.0 * i;
        Vec3 vel; vel.x = 0.1 * i; vel.y = 0.2 * i; vel.z = 0.3 * i;
        w.position = pos;
        w.velocity = vel;
        w.label = std::string("wp-") + std::to_string(i);
        w.weight = 0.5 * static_cast<double>(i) + 1.0;
        wps.push_back(w);
    }
    m.waypoints = wps;
    m.payload.assign(payload_size, static_cast<uint8_t>(0xAB));
}
#else
using Sample = RoundtripBench::Roundtrip;
constexpr const char* kProgram   = "zerodds-app";
constexpr const char* kCodecFile = "gen/zerodds/Roundtrip.hpp";
constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";
constexpr const char* kTypeName  = "RoundtripBench::Roundtrip";

void populate(Sample& m, size_t payload_size) {
    m.payload.assign(payload_size, static_cast<uint8_t>(0xAB));
}
#endif

#ifdef ZERODDS_BENCH_XCDR1
// The XCDR1 build (`zerodds-xcdr1-roundtrip`, CMakeLists.txt): the codec
// is gen/zerodds-xcdr1/Roundtrip.hpp (`--encoding xcdr1`), and the runtime
// has to stamp XCDR1 on what it writes and ask for XCDR1 from its readers.
// The C-FFI runtime reads that from ZERODDS_DATA_REPR_OFFER when it is
// created; set here, unless the caller asked for something else, which is
// refused for the reason the XCDR2 build refuses XCDR1.
static void refuse_unsupported_data_repr_offer() {
    const char* s = std::getenv("ZERODDS_DATA_REPR_OFFER");
    if (!s) {
        setenv("ZERODDS_DATA_REPR_OFFER", "XCDR1", 1);
        return;
    }
    std::string v(s);
    for (auto& c : v) c = static_cast<char>(::toupper(c));
    if (v != "XCDR1" && v != "1") {
        std::cerr << kProgram << ": this build holds the PlainCdr1 codec and offers XCDR1 "
                     "only; ZERODDS_DATA_REPR_OFFER=" << s << " is not supported.\n";
        std::exit(1);
    }
}
#else
// `gen/zerodds/Roundtrip.hpp` holds one codec (the "one codec per
// language" contract, `docs/specs/zerodds-ffi-loader-1.0.md` §4.2),
// planned for the `--encoding` it was generated under — `xcdr2` here, see
// the file's own preamble comment. `topic_type_support<Roundtrip>::encode`
// therefore always emits PlainCdr2, unconditionally; there is no more a
// runtime-selectable version to derive from ZERODDS_DATA_REPR_OFFER, the
// way an older, hand-written codec here once allowed. The runtime's own
// default offer is `[XCDR2]` (publication_data::DEFAULT_OFFER), which this
// app's fixed codec matches; ZERODDS_DATA_REPR_OFFER=XCDR1 would still ask
// the runtime for an XCDR1 encapsulation identifier over an XCDR2 body, so
// it is refused here rather than silently mis-framed.
static void refuse_unsupported_data_repr_offer() {
    const char* s = std::getenv("ZERODDS_DATA_REPR_OFFER");
    if (!s) return;
    std::string v(s);
    auto comma = v.find(',');
    std::string first = (comma == std::string::npos) ? v : v.substr(0, comma);
    for (auto& c : first) c = static_cast<char>(::toupper(c));
    if (first == "XCDR1" || first == "1") {
        std::cerr << kProgram << ": ZERODDS_DATA_REPR_OFFER=XCDR1 is not supported; "
                  << kCodecFile << " holds one codec (PlainCdr2). Regenerate "
                     "it with `zerodds-idlc generate --lang c++ --encoding xcdr1` for "
                     "that path.\n";
        std::exit(1);
    }
}
#endif

// =====================================================================
// DDS-Security 1.2 (env-driven) — analog FastDDS apply_security.
//   ZERODDS_BENCH_SECURITY=1                aktiviert Security
//   ZERODDS_BENCH_SEC_NAME=ping|pong|...    selects the cert/key/perm suffix
//   ZERODDS_BENCH_SEC_DIR=/abs/path         CA + signed XMLs Wurzel
//                                           (default /tmp/dds-bench-security)
// Asset-Tree:
//   ${SEC_DIR}/certs/identity_ca.pem
//   ${SEC_DIR}/certs/${SEC_NAME}_cert.pem
//   ${SEC_DIR}/certs/${SEC_NAME}_key.pem
//   ${SEC_DIR}/certs/permissions_ca.pem
//   ${SEC_DIR}/governance.p7s
//   ${SEC_DIR}/permissions_${SEC_NAME}.p7s
// On NULL return from runtime_create_secure → exit 1 with diag.
// =====================================================================
static zerodds_ZeroDdsRuntime* make_runtime() {
    const char* enable = std::getenv("ZERODDS_BENCH_SECURITY");
    if (!enable || std::string(enable) != "1") {
        return zerodds_runtime_create(resolve_domain());
    }
#ifdef ZERODDS_BENCH_NOSEC
    // Built against a non-security libzerodds (no `--features security`) so the
    // plain hot path can be benchmarked without the security feature compiled
    // in. The secure runtime is unavailable in this build.
    std::cerr << "ZERODDS_BENCH_NOSEC build: secure runtime not compiled in\n";
    return nullptr;
#else
    const char* sec_dir = std::getenv("ZERODDS_BENCH_SEC_DIR");
    if (!sec_dir) sec_dir = "/tmp/dds-bench-security";
    const char* who = std::getenv("ZERODDS_BENCH_SEC_NAME");
    if (!who) who = "ping";

    auto p = [&](const std::string& sub) {
        return std::string(sec_dir) + "/" + sub;
    };
    const std::string id_ca   = p("certs/identity_ca.pem");
    const std::string id_cert = p("certs/" + std::string(who) + "_cert.pem");
    const std::string id_key  = p("certs/" + std::string(who) + "_key.pem");
    const std::string perm_ca = p("certs/permissions_ca.pem");
    const std::string gov     = p("governance.p7s");
    const std::string perms   = p("permissions_" + std::string(who) + ".p7s");

    auto* cfg = zerodds_security_config_create();
    if (!cfg) { std::cerr << "security_config_create failed\n"; return nullptr; }

    int rc = 0;
    rc |= zerodds_security_set_identity_ca_path(cfg, id_ca.c_str());
    rc |= zerodds_security_set_identity_cert_path(cfg, id_cert.c_str());
    rc |= zerodds_security_set_private_key_path(cfg, id_key.c_str());
    rc |= zerodds_security_set_permissions_ca_path(cfg, perm_ca.c_str());
    rc |= zerodds_security_set_governance_path(cfg, gov.c_str());
    rc |= zerodds_security_set_permissions_path(cfg, perms.c_str());
    if (rc != 0) {
        std::cerr << "security setter rc=" << rc << "\n";
        zerodds_security_config_destroy(cfg);
        return nullptr;
    }

    auto* rt = zerodds_runtime_create_secure(resolve_domain(), cfg);
    // cfg lifetime: runtime_create_secure does NOT consume cfg — we
    // must free it ourselves after the call.
    zerodds_security_config_destroy(cfg);
    return rt;
#endif // ZERODDS_BENCH_NOSEC
}

// Decodes a received wire sample into a typed roundtrip.
// `repr` comes from the data callback (0=XCDR1, 1=XCDR2) and `big_endian`
// from the encapsulation header (DDSI-RTPS 2.5 §10.5); both control the
// XCDR-Alignment-Regel (XTypes 1.3 §7.4.3.4.2).
Sample decode_sample(const uint8_t* buf, size_t len, uint8_t repr,
                     uint8_t big_endian) {
    auto ver = (repr == 1) ? ::zerodds::wire::Encoding::PlainCdr2
                           : ::zerodds::wire::Encoding::PlainCdr1;
    return ::dds::topic::topic_type_support<Sample>::decode(
        buf, len, ver, big_endian != 0);
}

// =====================================================================
// Pong: data callback decodes the request sample and re-encodes it
// onto the echo topic. Runs in the recv thread, event-driven.
// =====================================================================
struct PongCtx {
    zerodds_ZeroDdsWriter* echo_writer;
    uint64_t echoed{0};
    uint32_t min_seq{0xFFFFFFFFu};
    uint32_t max_seq{0};
};

extern "C" void pong_on_data(void* user_data, const uint8_t* payload,
                             size_t len, uint8_t repr, uint8_t big_endian) {
    // Der Callback laeuft im Rust-Recv-Thread — KEINE C++-Exception
    // may escape (Rust cannot catch it -> abort). decode can
    // throw `std::out_of_range` on malformed/control samples (e.g. OpenDDS instance registration,
    // empty body); such samples are
    // schlicht uebersprungen.
    try {
        auto* ctx = static_cast<PongCtx*>(user_data);
        auto sample = decode_sample(payload, len, repr, big_endian);
        auto encoded =
            ::dds::topic::topic_type_support<Sample>::encode(sample);
        zerodds_writer_write(ctx->echo_writer, encoded.data(), encoded.size());
        // recv thread is single writer of these fields — no lock needed.
        ++ctx->echoed;
        uint32_t seq = sample.sequence_id;
        if (seq < ctx->min_seq) ctx->min_seq = seq;
        if (seq > ctx->max_seq) ctx->max_seq = seq;
    } catch (...) {
        // Sample verwerfen.
    }
}

int run_pong(uint64_t max_runtime_s) {
    refuse_unsupported_data_repr_offer();
    auto* rt = make_runtime();
    if (!rt) { std::cerr << "runtime_create failed\n"; return 1; }
    if (zerodds_runtime_wait_for_peers(rt, 1, 30000) != 0) {
        std::cerr << "pong: wait_for_peers timeout\n"; return 1;
    }
    auto* dr = zerodds_reader_create_kind(rt, kReqTopic, kTypeName, 1, 0);
    auto* dw = zerodds_writer_create_kind(rt, kEchoTopic, kTypeName, 1, 0);
    if (!dr || !dw) { std::cerr << "reader/writer create failed\n"; return 1; }
    if (zerodds_writer_wait_for_matched(dw, 1, 5000) != 0) {
        std::cerr << "pong: writer wait_for_matched timeout\n"; return 1;
    }
    if (zerodds_reader_wait_for_matched(dr, 1, 5000) != 0) {
        std::cerr << "pong: reader wait_for_matched timeout\n"; return 1;
    }

    PongCtx ctx;
    ctx.echo_writer = dw;
    zerodds_reader_set_data_callback(dr, pong_on_data, &ctx);
    std::cout << "pong: matched, typed echo (event-driven)\n" << std::flush;

    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));

    // Deregister the callback BEFORE reader/writer are destroyed.
    zerodds_reader_set_data_callback(dr, nullptr, nullptr);
    std::cout << "pong: echoed " << ctx.echoed << " samples\n";
    std::cout << "pong: seq-range [" << ctx.min_seq << ".." << ctx.max_seq
              << "], unknown_src=" << zerodds_reader_unknown_src_count(dr) << "\n";

    zerodds_writer_destroy(dw);
    zerodds_reader_destroy(dr);
    zerodds_phase_dump();
    zerodds_runtime_destroy(rt);
    return 0;
}

// =====================================================================
// Ping: write typed; the data callback captures the echo and
// wakes the sender thread via a condition variable. One-in-flight.
// =====================================================================
struct PingCtx {
    std::mutex              mu;
    std::condition_variable cv;
    uint64_t                echo_t_send{0};
    bool                    got{false};
};

extern "C" void ping_on_data(void* user_data, const uint8_t* payload,
                             size_t len, uint8_t repr, uint8_t big_endian) {
    // No C++ exception may escape into the Rust recv thread.
    try {
        auto* ctx = static_cast<PingCtx*>(user_data);
        auto echo = decode_sample(payload, len, repr, big_endian);
        {
            std::lock_guard<std::mutex> lk(ctx->mu);
            ctx->echo_t_send = echo.t_send_ns;
            ctx->got = true;
        }
        ctx->cv.notify_one();
    } catch (...) {
        // Malformed/control sample — ignore, no RTT entry.
    }
}

int run_ping(size_t payload_size, uint64_t warmup, uint64_t samples) {
    // Before the runtime: the XCDR1 build sets the offer the runtime reads
    // when it is created.
    refuse_unsupported_data_repr_offer();
    auto* rt = make_runtime();
    if (!rt) { std::cerr << "runtime_create failed\n"; return 1; }
    if (zerodds_runtime_wait_for_peers(rt, 1, 30000) != 0) {
        std::cerr << "ping: wait_for_peers timeout\n"; return 1;
    }
    auto* dw = zerodds_writer_create_kind(rt, kReqTopic, kTypeName, 1, 0);
    auto* dr = zerodds_reader_create_kind(rt, kEchoTopic, kTypeName, 1, 0);
    if (!dr || !dw) { std::cerr << "reader/writer create failed\n"; return 1; }
    if (zerodds_writer_wait_for_matched(dw, 1, 5000) != 0) {
        std::cerr << "ping: writer wait_for_matched timeout\n"; return 1;
    }
    if (zerodds_reader_wait_for_matched(dr, 1, 5000) != 0) {
        std::cerr << "ping: reader wait_for_matched timeout\n"; return 1;
    }

    PingCtx ctx;
    zerodds_reader_set_data_callback(dr, ping_on_data, &ctx);

    std::vector<uint64_t> rtts;
    rtts.reserve(samples);

    Sample msg;
    populate(msg, payload_size);

    // Wire version = whatever `gen/zerodds/Roundtrip.hpp` was generated
    // for (fixed at generation time, `--encoding xcdr2` by default here).
    // With XCDR2, ZeroDDS sends a PLAIN_CDR2 body + 0x0007 header -> by
    // XCDR2-only vendor readers akzeptiert. Roundtrip ist @final ->
    // Default-Extensibility FINAL (setter not needed). The XCDR1 build's
    // offer is set at the top of run_ping.

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id = static_cast<uint32_t>(seq);
        {
            std::lock_guard<std::mutex> lk(ctx.mu);
            ctx.got = false;
        }
        msg.t_send_ns = now_ns();
        auto encoded = ::dds::topic::topic_type_support<Sample>::encode(msg);
        int wrc = zerodds_writer_write(dw, encoded.data(), encoded.size());
        if (wrc != 0) { std::cerr << "ping: writer_write rc=" << wrc << "\n"; return 1; }

        // Wait for the echo — condition variable, no spin. Max 50ms.
        std::unique_lock<std::mutex> lk(ctx.mu);
        bool ok = ctx.cv.wait_for(lk, std::chrono::milliseconds(50),
                                  [&] { return ctx.got; });
        uint64_t now = now_ns();
        if (ok) {
            uint64_t rtt = (ctx.echo_t_send != 0 && now > ctx.echo_t_send)
                               ? now - ctx.echo_t_send : 1;
            if (seq >= warmup) rtts.push_back(rtt);
        }
    }

    zerodds_reader_set_data_callback(dr, nullptr, nullptr);
    print_quantiles(rtts, payload_size);
    std::cout << "ping: unknown_src_count=" << zerodds_reader_unknown_src_count(dr) << "\n";

    zerodds_phase_dump();
    zerodds_writer_destroy(dw);
    zerodds_reader_destroy(dr);
    zerodds_runtime_destroy(rt);
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr <<
            "Usage:\n"
            "  " << kProgram << " pong [max_runtime_s]\n"
            "  " << kProgram << " ping --payload N [--samples N] [--warmup N]\n";
        return 2;
    }
    std::string mode = argv[1];
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2) ? std::stoull(argv[2]) : 30;
        return run_pong(rt_s);
    } else if (mode == "ping") {
        bench::PingArgs a = bench::parse_ping_args(argc, argv, 2);
        return run_ping(a.payload, a.warmup, a.samples);
    }
    std::cerr << "unknown mode: " << mode << "\n";
    return 2;
}
