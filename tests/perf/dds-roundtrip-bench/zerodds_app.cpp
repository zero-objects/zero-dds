// SPDX-License-Identifier: Apache-2.0
//
// ZeroDDS C-FFI Roundtrip-Bench-App. Apples-to-apples mit Cyclone/Fast-DDS/RTI/OpenDDS.
//
// IDL: roundtrip.idl -> RoundtripBench::Roundtrip (via gen/zerodds/Roundtrip.hpp)
//
// **Voll event-driven** — kein Busy-Poll. Beide Rollen haengen am
// Data-Available-Callback (`zerodds_reader_set_data_callback`), der vom
// Recv-Thread direkt nach Sample-Arrival feuert:
//   * pong: Callback dekodiert das Sample und re-encodiert es (typisiert,
//     kein Raw-Byte-Echo) -> die XCDR-Representation des Peers wird
//     korrekt interpretiert.
//   * ping: Callback erfasst die RTT und weckt den Sender-Thread ueber
//     eine Condition-Variable.
// Der Callback bekommt die XCDR-Version (0=XCDR1, 1=XCDR2) als Argument
// — `decode` braucht sie fuer die Alignment-Regel.
//
// Build via CMakeLists.txt.

#include "zerodds.h"
#include "Roundtrip.hpp"

#include <algorithm>
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
// (Drop-Path feuert erst beim Prozess-Exit, weil die Worker-Threads
// den Arc-Refcount halten; deshalb hier explizit aufrufen).
extern "C" void zerodds_phase_dump();

namespace {

// Domain via ZERODDS_BENCH_DOMAIN env-var ueberschreibbar (Default 200).
// Erlaubt dem Bench-Runner pro Cell eine eigene Domain-ID zu nutzen,
// damit Multi-Process-Discovery-State sich nicht zwischen sequenziellen
// Cells wiederfindet.
static uint32_t resolve_domain() {
    const char* s = std::getenv("ZERODDS_BENCH_DOMAIN");
    if (s) {
        try { return static_cast<uint32_t>(std::stoul(s)); } catch (...) {}
    }
    return 200;
}

// Encode-Wire-Version aus ZERODDS_DATA_REPR_OFFER ableiten — MUSS zum
// Runtime-Offer passen, denn das erste Offer-Element bestimmt das
// tatsaechlich emittierte Wire-Format (und damit den Encap-Header, den
// der Runtime prependet). Der Runtime-Default-Offer ist `[XCDR2]`
// (publication_data::DEFAULT_OFFER), also MUSS der Encode-Default
// ebenfalls XCDR2 sein — sonst stempelt der Runtime einen 0x0007-Encap
// auf einen XCDR1-Body und jeder Reader (auch zerodds selbst) scheitert
// an der Deserialisierung. "XCDR1"/"1" als erstes Token erzwingt explizit
// den Legacy-Pfad; alles andere (inkl. fehlender env) -> XCDR2.
static ::dds::topic::xcdr2::XcdrVersion resolve_encode_version() {
    const char* s = std::getenv("ZERODDS_DATA_REPR_OFFER");
    if (s) {
        std::string v(s);
        auto comma = v.find(',');
        std::string first = (comma == std::string::npos) ? v : v.substr(0, comma);
        for (auto& c : first) c = static_cast<char>(::toupper(c));
        if (first == "XCDR1" || first == "1") {
            return ::dds::topic::xcdr2::XcdrVersion::Xcdr1;
        }
    }
    return ::dds::topic::xcdr2::XcdrVersion::Xcdr2;
}

// =====================================================================
// DDS-Security 1.2 (env-driven) — analog FastDDS apply_security.
//   ZERODDS_BENCH_SECURITY=1                aktiviert Security
//   ZERODDS_BENCH_SEC_NAME=ping|pong|...    waehlt cert/key/perm-Suffix
//   ZERODDS_BENCH_SEC_DIR=/abs/path         CA + signed XMLs Wurzel
//                                           (default /tmp/dds-bench-security)
// Asset-Tree:
//   ${SEC_DIR}/certs/identity_ca.pem
//   ${SEC_DIR}/certs/${SEC_NAME}_cert.pem
//   ${SEC_DIR}/certs/${SEC_NAME}_key.pem
//   ${SEC_DIR}/certs/permissions_ca.pem
//   ${SEC_DIR}/governance.p7s
//   ${SEC_DIR}/permissions_${SEC_NAME}.p7s
// Bei NULL-Rueckgabe von runtime_create_secure → exit 1 mit Diag.
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
    // cfg-Lifetime: runtime_create_secure konsumiert cfg NICHT — wir
    // muessen es nach dem Aufruf selbst freigeben.
    zerodds_security_config_destroy(cfg);
    return rt;
#endif // ZERODDS_BENCH_NOSEC
}
constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";
constexpr const char* kTypeName  = "RoundtripBench::Roundtrip";

uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

// Dekodiert ein empfangenes Wire-Sample in einen typisierten Roundtrip.
// `repr` kommt aus dem Data-Callback (0=XCDR1, 1=XCDR2) und steuert die
// XCDR-Alignment-Regel (XTypes 1.3 §7.4.3.4.2).
RoundtripBench::Roundtrip decode_sample(const uint8_t* buf, size_t len, uint8_t repr) {
    auto ver = (repr == 1) ? ::dds::topic::xcdr2::XcdrVersion::Xcdr2
                           : ::dds::topic::xcdr2::XcdrVersion::Xcdr1;
    return ::dds::topic::topic_type_support<RoundtripBench::Roundtrip>::decode(buf, len, ver);
}

// =====================================================================
// Pong: Data-Callback dekodiert das Request-Sample und re-encodiert es
// auf das Echo-Topic. Laeuft im Recv-Thread, event-driven.
// =====================================================================
struct PongCtx {
    zerodds_ZeroDdsWriter* echo_writer;
    uint64_t echoed{0};
    uint32_t min_seq{0xFFFFFFFFu};
    uint32_t max_seq{0};
};

extern "C" void pong_on_data(void* user_data, const uint8_t* payload,
                             size_t len, uint8_t repr, uint8_t /*big_endian*/) {
    // Der Callback laeuft im Rust-Recv-Thread — KEINE C++-Exception
    // darf entkommen (Rust kann sie nicht fangen -> abort). decode kann
    // bei malformed/Control-Samples (z.B. OpenDDS Instance-Registration,
    // leerer Body) `std::out_of_range` werfen; solche Samples werden
    // schlicht uebersprungen.
    try {
        // Echo-Wire-Version = eigenes Offer (env-driven), einmal gecacht.
        // Muss zum Encap-Header passen, den der Runtime fuer diesen
        // Writer prependet (offer-`first`).
        static const auto kEchoVer = resolve_encode_version();
        auto* ctx = static_cast<PongCtx*>(user_data);
        auto sample = decode_sample(payload, len, repr);
        auto encoded =
            ::dds::topic::topic_type_support<RoundtripBench::Roundtrip>::encode(sample, kEchoVer);
        zerodds_writer_write(ctx->echo_writer, encoded.data(), encoded.size());
        // Recv-Thread ist Single-Writer dieser Felder — kein Lock noetig.
        ++ctx->echoed;
        uint32_t seq = sample.sequence_id();
        if (seq < ctx->min_seq) ctx->min_seq = seq;
        if (seq > ctx->max_seq) ctx->max_seq = seq;
    } catch (...) {
        // Sample verwerfen.
    }
}

int run_pong(uint64_t max_runtime_s) {
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

    // Callback abmelden BEVOR Reader/Writer zerstoert werden.
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

void print_quantiles(std::vector<uint64_t>& rtts, size_t payload_size) {
    if (rtts.empty()) { std::cout << "no samples\n"; return; }
    std::sort(rtts.begin(), rtts.end());
    auto pct = [&](double p) {
        size_t idx = std::min(rtts.size() - 1,
                              static_cast<size_t>(p * (double)(rtts.size() - 1)));
        return rtts[idx];
    };
    std::cout
        << "payload=" << payload_size
        << "  n=" << rtts.size()
        << "  min=" << rtts.front()/1000.0 << "us"
        << "  p50=" << pct(0.50)/1000.0 << "us"
        << "  p90=" << pct(0.90)/1000.0 << "us"
        << "  p99=" << pct(0.99)/1000.0 << "us"
        << "  p999=" << pct(0.999)/1000.0 << "us"
        << "  max=" << rtts.back()/1000.0 << "us\n";
}

// =====================================================================
// Ping: typisiert schreiben; der Data-Callback erfasst das Echo und
// weckt den Sender-Thread ueber eine Condition-Variable. One-in-flight.
// =====================================================================
struct PingCtx {
    std::mutex              mu;
    std::condition_variable cv;
    uint64_t                echo_t_send{0};
    bool                    got{false};
};

extern "C" void ping_on_data(void* user_data, const uint8_t* payload,
                             size_t len, uint8_t repr, uint8_t /*big_endian*/) {
    // Keine C++-Exception darf in den Rust-Recv-Thread entkommen.
    try {
        auto* ctx = static_cast<PingCtx*>(user_data);
        auto echo = decode_sample(payload, len, repr);
        {
            std::lock_guard<std::mutex> lk(ctx->mu);
            ctx->echo_t_send = echo.t_send_ns();
            ctx->got = true;
        }
        ctx->cv.notify_one();
    } catch (...) {
        // Malformed/Control-Sample — ignorieren, kein RTT-Eintrag.
    }
}

int run_ping(size_t payload_size, uint64_t warmup, uint64_t samples) {
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

    RoundtripBench::Roundtrip msg;
    std::vector<uint8_t> payload(payload_size, 0xAB);
    msg.payload(payload);

    // Wire-Version = Runtime-Offer (env). Bei XCDR2 sendet ZeroDDS einen
    // PLAIN_CDR2-Body + 0x0007-Header -> von XCDR2-only Vendor-Readern
    // akzeptiert. Roundtrip ist @final -> Default-Extensibility FINAL
    // (Setter nicht noetig).
    const auto enc_ver = resolve_encode_version();

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id(static_cast<uint32_t>(seq));
        {
            std::lock_guard<std::mutex> lk(ctx.mu);
            ctx.got = false;
        }
        msg.t_send_ns(now_ns());
        auto encoded = ::dds::topic::topic_type_support<RoundtripBench::Roundtrip>::encode(msg, enc_ver);
        int wrc = zerodds_writer_write(dw, encoded.data(), encoded.size());
        if (wrc != 0) { std::cerr << "ping: writer_write rc=" << wrc << "\n"; return 1; }

        // Auf das Echo warten — Condition-Variable, kein Spin. Max 50ms.
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
            "  zerodds-app pong [max_runtime_s]\n"
            "  zerodds-app ping --payload N [--samples N] [--warmup N]\n";
        return 2;
    }
    std::string mode = argv[1];
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2) ? std::stoull(argv[2]) : 30;
        return run_pong(rt_s);
    } else if (mode == "ping") {
        size_t   payload = 64;
        uint64_t warmup  = 200;
        uint64_t samples = 5000;
        for (int i = 2; i + 1 < argc; i += 2) {
            std::string flag = argv[i];
            uint64_t   v    = std::stoull(argv[i+1]);
            if      (flag == "--payload") payload = static_cast<size_t>(v);
            else if (flag == "--samples") samples = v;
            else if (flag == "--warmup")  warmup  = v;
        }
        return run_ping(payload, warmup, samples);
    }
    std::cerr << "unknown mode: " << mode << "\n";
    return 2;
}
