// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

/**
 * RTPS 2.5 wire constants — ported from {@code crates/rtps/src}
 * ({@code header.rs}, {@code wire_types.rs}, {@code parameter_list.rs},
 * {@code submessage_header.rs}) and the ZeroDDS discovery layer
 * ({@code crates/discovery}, {@code crates/dcps}). Values are byte-exact so
 * this pure-Java stack interoperates on the wire with the Rust zerodds stack.
 */
public final class Rtps {
    private Rtps() {}

    // Header (§8.3.3)
    public static final byte[] MAGIC = {'R', 'T', 'P', 'S'};
    public static final int VERSION_MAJOR = 2;
    public static final int VERSION_MINOR = 5;
    /** ZeroDDS interim VendorId (OMG developer range). */
    public static final byte[] VENDOR_ID = {(byte) 0x01, (byte) 0xF0};

    // Submessage ids (§8.3.4, Table 8.13)
    public static final int SM_PAD = 0x01;
    public static final int SM_ACKNACK = 0x06;
    public static final int SM_HEARTBEAT = 0x07;
    public static final int SM_GAP = 0x08;
    public static final int SM_INFO_TS = 0x09;
    public static final int SM_INFO_SRC = 0x0A;
    public static final int SM_INFO_DST = 0x0E;
    public static final int SM_INFO_REPLY = 0x0F;
    public static final int SM_DATA = 0x15;
    public static final int SM_DATA_FRAG = 0x16;

    // Submessage-header flags
    public static final int FLAG_E = 0x01; // little-endian body

    // DATA submessage flags (§8.3.7.2)
    public static final int DATA_FLAG_INLINE_QOS = 0x02;
    public static final int DATA_FLAG_DATA = 0x04;
    public static final int DATA_FLAG_KEY = 0x08;

    // HEARTBEAT flags (§8.3.7.5)
    public static final int HB_FLAG_FINAL = 0x02;
    public static final int HB_FLAG_LIVELINESS = 0x04;

    // ACKNACK flags (§8.3.7.1)
    public static final int ACKNACK_FLAG_FINAL = 0x02;

    // INFO_TS flag
    public static final int INFO_TS_FLAG_INVALIDATE = 0x02;

    // Encapsulation headers (RTPS 2.5 §10.5) — identifier(2) + options(2)
    public static final byte[] ENCAP_CDR_LE = {0x00, 0x01, 0x00, 0x00}; // XCDR1 plain, LE
    public static final byte[] ENCAP_PL_CDR_LE = {0x00, 0x03, 0x00, 0x00}; // parameter list, LE
    public static final byte[] ENCAP_CDR2_LE = {0x00, 0x07, 0x00, 0x00}; // XCDR2 plain final, LE

    // ---- Parameter IDs (parameter_list.rs pid + qos/pid.rs) ----
    public static final int PID_SENTINEL = 0x0001;
    public static final int PID_PARTICIPANT_LEASE_DURATION = 0x0002;
    public static final int PID_TOPIC_NAME = 0x0005;
    public static final int PID_OWNERSHIP_STRENGTH = 0x0006;
    public static final int PID_TYPE_NAME = 0x0007;
    public static final int PID_DOMAIN_ID = 0x000F;
    public static final int PID_PROTOCOL_VERSION = 0x0015;
    public static final int PID_VENDOR_ID = 0x0016;
    public static final int PID_RELIABILITY = 0x001A;
    public static final int PID_LIVELINESS = 0x001B;
    public static final int PID_DURABILITY = 0x001D;
    public static final int PID_OWNERSHIP = 0x001F;
    public static final int PID_PRESENTATION = 0x0021;
    public static final int PID_DEADLINE = 0x0023;
    public static final int PID_PARTITION = 0x0029;
    public static final int PID_LIFESPAN = 0x002B;
    public static final int PID_UNICAST_LOCATOR = 0x002F;
    public static final int PID_MULTICAST_LOCATOR = 0x0030;
    public static final int PID_DEFAULT_UNICAST_LOCATOR = 0x0031;
    public static final int PID_METATRAFFIC_UNICAST_LOCATOR = 0x0032;
    public static final int PID_METATRAFFIC_MULTICAST_LOCATOR = 0x0033;
    public static final int PID_DEFAULT_MULTICAST_LOCATOR = 0x0048;
    public static final int PID_PARTICIPANT_GUID = 0x0050;
    public static final int PID_BUILTIN_ENDPOINT_SET = 0x0058;
    public static final int PID_ENDPOINT_GUID = 0x005A;
    public static final int PID_KEY_HASH = 0x0070;
    public static final int PID_STATUS_INFO = 0x0071;
    public static final int PID_DATA_REPRESENTATION = 0x0073;
    public static final int PID_TYPE_INFORMATION = 0x0075;
    /** ZeroDDS vendor PID carrying the cross-binding TypeIdentifier hash. */
    public static final int PID_ZERODDS_TYPE_ID = 0x8002;

    // Reliability kinds (qos wire)
    public static final int RELIABILITY_BEST_EFFORT = 1;
    public static final int RELIABILITY_RELIABLE = 2;

    // Durability kinds
    public static final int DURABILITY_VOLATILE = 0;
    public static final int DURABILITY_TRANSIENT_LOCAL = 1;

    // Data representation ids
    public static final int DATA_REP_XCDR1 = 0;
    public static final int DATA_REP_XCDR2 = 2;

    // BuiltinEndpointSet bits (discovery endpoint_flag)
    public static final long EP_PARTICIPANT_ANNOUNCER = 1L << 0;
    public static final long EP_PARTICIPANT_DETECTOR = 1L << 1;
    public static final long EP_PUBLICATIONS_ANNOUNCER = 1L << 2;
    public static final long EP_PUBLICATIONS_DETECTOR = 1L << 3;
    public static final long EP_SUBSCRIPTIONS_ANNOUNCER = 1L << 4;
    public static final long EP_SUBSCRIPTIONS_DETECTOR = 1L << 5;
    /** Default non-secure announced set (bits 0-5). */
    public static final long EP_ALL_BASIC = EP_PARTICIPANT_ANNOUNCER | EP_PARTICIPANT_DETECTOR
            | EP_PUBLICATIONS_ANNOUNCER | EP_PUBLICATIONS_DETECTOR
            | EP_SUBSCRIPTIONS_ANNOUNCER | EP_SUBSCRIPTIONS_DETECTOR;

    // ---- Ports (Spec §9.6.1.4.1) ----
    public static final int PORT_BASE = 7400; // PB
    public static final int DOMAIN_GAIN = 250; // DG
    public static final int PARTICIPANT_GAIN = 2; // PG
    public static final int D0_MULTICAST_META = 0;
    public static final int D1_UNICAST_META = 10;
    public static final int D2_MULTICAST_USER = 1;
    public static final int D3_UNICAST_USER = 11;
    public static final String SPDP_MULTICAST_ADDRESS = "239.255.0.1";

    public static int spdpMulticastPort(int domain) {
        return PORT_BASE + DOMAIN_GAIN * domain + D0_MULTICAST_META;
    }

    public static int metatrafficUnicastPort(int domain, int pid) {
        return PORT_BASE + DOMAIN_GAIN * domain + D1_UNICAST_META + PARTICIPANT_GAIN * pid;
    }

    public static int userUnicastPort(int domain, int pid) {
        return PORT_BASE + DOMAIN_GAIN * domain + D3_UNICAST_USER + PARTICIPANT_GAIN * pid;
    }
}
