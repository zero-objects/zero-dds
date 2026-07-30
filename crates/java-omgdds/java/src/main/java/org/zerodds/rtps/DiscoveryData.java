// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.List;

/**
 * SPDP {@code ParticipantBuiltinTopicData} and SEDP
 * {@code Publication/SubscriptionBuiltinTopicData} — build + parse. The PID
 * order and value layouts mirror {@code crates/rtps/src/participant_data.rs},
 * {@code publication_data.rs}, {@code subscription_data.rs} so the payload is
 * byte-compatible with the Rust discovery decoders (all little-endian,
 * PL_CDR_LE encapsulation {@code 00 03 00 00}).
 */
public final class DiscoveryData {
    private DiscoveryData() {}

    // ===================== SPDP participant =====================

    public static final class Participant {
        public byte[] guidPrefix; // 12
        public Locator metatrafficUnicast;
        public Locator defaultUnicast;
        public long builtinEndpointSet;
        public int leaseSeconds = 100;
        public long lastSeenMillis;
    }

    /** Build the SPDP participant payload (encapsulation + ParameterList). */
    public static byte[] buildParticipant(byte[] guidPrefix12, int domain,
                                          Locator metatrafficUnicast, Locator defaultUnicast,
                                          long builtinEndpointSet, int leaseSeconds) {
        ParameterList pl = new ParameterList();
        pl.add(Rtps.PID_PROTOCOL_VERSION,
                ParameterList.twoBytesPadded((byte) Rtps.VERSION_MAJOR, (byte) Rtps.VERSION_MINOR));
        pl.add(Rtps.PID_VENDOR_ID,
                ParameterList.twoBytesPadded(Rtps.VENDOR_ID[0], Rtps.VENDOR_ID[1]));
        pl.add(Rtps.PID_PARTICIPANT_GUID, new Guid(guidPrefix12, Guid.ENTITYID_PARTICIPANT).bytes());
        pl.add(Rtps.PID_DEFAULT_UNICAST_LOCATOR, locatorBytes(defaultUnicast));
        pl.add(Rtps.PID_METATRAFFIC_UNICAST_LOCATOR, locatorBytes(metatrafficUnicast));
        pl.add(Rtps.PID_DOMAIN_ID, ParameterList.u32(domain));
        pl.add(Rtps.PID_BUILTIN_ENDPOINT_SET, ParameterList.u32(builtinEndpointSet));
        pl.add(Rtps.PID_PARTICIPANT_LEASE_DURATION, ParameterList.duration(leaseSeconds, 0));
        return withEncap(Rtps.ENCAP_PL_CDR_LE, pl);
    }

    /** Parse an SPDP participant payload (encapsulation + ParameterList). */
    public static Participant parseParticipant(byte[] payload) {
        ParameterList pl = plFromPayload(payload);
        if (pl == null) {
            return null;
        }
        Participant p = new Participant();
        byte[] guid = pl.find(Rtps.PID_PARTICIPANT_GUID);
        if (guid != null && guid.length >= 12) {
            p.guidPrefix = Wire.slice(guid, 0, 12);
        }
        byte[] mu = pl.find(Rtps.PID_METATRAFFIC_UNICAST_LOCATOR);
        if (mu != null && mu.length >= 24) {
            p.metatrafficUnicast = Locator.readLe(mu, 0);
        }
        byte[] du = pl.find(Rtps.PID_DEFAULT_UNICAST_LOCATOR);
        if (du != null && du.length >= 24) {
            p.defaultUnicast = Locator.readLe(du, 0);
        }
        byte[] bes = pl.find(Rtps.PID_BUILTIN_ENDPOINT_SET);
        if (bes != null && bes.length >= 4) {
            p.builtinEndpointSet = Wire.u32le(bes, 0);
        }
        byte[] lease = pl.find(Rtps.PID_PARTICIPANT_LEASE_DURATION);
        if (lease != null && lease.length >= 4) {
            p.leaseSeconds = (int) Wire.u32le(lease, 0);
        }
        return p.guidPrefix == null ? null : p;
    }

    // ===================== SEDP endpoint =====================

    public static final class Endpoint {
        public boolean writer; // true = DCPSPublication, false = DCPSSubscription
        public byte[] participantPrefix; // 12
        public byte[] endpointGuid; // 16
        public String topicName;
        public String typeName;
        public int reliabilityKind = Rtps.RELIABILITY_BEST_EFFORT;
        public int durabilityKind = Rtps.DURABILITY_VOLATILE;
        public Locator unicastLocator; // where to send DATA to this endpoint
        public byte[] typeId; // ZeroDDS TypeIdentifier hash (may be null/empty)
    }

    /**
     * Build an SEDP endpoint payload. {@code unicast} is the locator on which
     * this endpoint's participant receives user traffic (default unicast).
     */
    public static byte[] buildEndpoint(boolean writer, byte[] participantPrefix12,
                                       byte[] endpointGuid16, String topic, String type,
                                       int reliabilityKind, int durabilityKind,
                                       int[] dataReps, Locator unicast, byte[] typeId) {
        ParameterList pl = new ParameterList();
        pl.add(Rtps.PID_PARTICIPANT_GUID,
                new Guid(participantPrefix12, Guid.ENTITYID_PARTICIPANT).bytes());
        pl.add(Rtps.PID_ENDPOINT_GUID, endpointGuid16.clone());
        pl.add(Rtps.PID_TOPIC_NAME, ParameterList.cdrString(topic));
        pl.add(Rtps.PID_TYPE_NAME, ParameterList.cdrString(type));
        pl.add(Rtps.PID_DURABILITY, ParameterList.u32(durabilityKind));
        pl.add(Rtps.PID_RELIABILITY, ParameterList.kindDuration(reliabilityKind, 0, 0x19999999L));
        if (writer) {
            pl.add(Rtps.PID_OWNERSHIP_STRENGTH, ParameterList.u32(0));
        }
        if (typeId != null && typeId.length > 0) {
            pl.add(Rtps.PID_ZERODDS_TYPE_ID, typeId.clone());
        }
        pl.add(Rtps.PID_DATA_REPRESENTATION, ParameterList.dataRepresentation(dataReps));
        // SEDP endpoints announce their locator via PID_UNICAST_LOCATOR (0x2f),
        // not the SPDP PID_DEFAULT_UNICAST_LOCATOR (0x31).
        pl.add(Rtps.PID_UNICAST_LOCATOR, locatorBytes(unicast));
        return withEncap(Rtps.ENCAP_PL_CDR_LE, pl);
    }

    public static Endpoint parseEndpoint(boolean writer, byte[] payload) {
        ParameterList pl = plFromPayload(payload);
        if (pl == null) {
            return null;
        }
        Endpoint e = new Endpoint();
        e.writer = writer;
        byte[] pg = pl.find(Rtps.PID_PARTICIPANT_GUID);
        if (pg != null && pg.length >= 12) {
            e.participantPrefix = Wire.slice(pg, 0, 12);
        }
        byte[] eg = pl.find(Rtps.PID_ENDPOINT_GUID);
        if (eg != null && eg.length >= 16) {
            e.endpointGuid = Wire.slice(eg, 0, 16);
        }
        byte[] tn = pl.find(Rtps.PID_TOPIC_NAME);
        if (tn != null) {
            e.topicName = ParameterList.readCdrString(tn);
        }
        byte[] ty = pl.find(Rtps.PID_TYPE_NAME);
        if (ty != null) {
            e.typeName = ParameterList.readCdrString(ty);
        }
        byte[] rel = pl.find(Rtps.PID_RELIABILITY);
        if (rel != null && rel.length >= 4) {
            e.reliabilityKind = (int) Wire.u32le(rel, 0);
        }
        byte[] dur = pl.find(Rtps.PID_DURABILITY);
        if (dur != null && dur.length >= 4) {
            e.durabilityKind = (int) Wire.u32le(dur, 0);
        }
        // SEDP endpoints use PID_UNICAST_LOCATOR (0x2f); there may be several
        // (loopback + LAN). Pick the first usable UDPv4. Fall back to 0x31.
        for (byte[] loc : findAll(pl, Rtps.PID_UNICAST_LOCATOR)) {
            if (loc.length >= 24) {
                Locator l = Locator.readLe(loc, 0);
                if (l.isUsableUdpV4()) {
                    e.unicastLocator = l;
                    break;
                }
            }
        }
        if (e.unicastLocator == null) {
            byte[] loc = pl.find(Rtps.PID_DEFAULT_UNICAST_LOCATOR);
            if (loc != null && loc.length >= 24) {
                e.unicastLocator = Locator.readLe(loc, 0);
            }
        }
        e.typeId = pl.find(Rtps.PID_ZERODDS_TYPE_ID);
        if (e.topicName == null || e.typeName == null || e.endpointGuid == null) {
            return null;
        }
        return e;
    }

    // ===================== helpers =====================

    private static byte[] locatorBytes(Locator l) {
        Wire w = new Wire(24);
        (l == null ? Locator.udpV4("0.0.0.0", 0) : l).writeLe(w);
        return w.toBytes();
    }

    private static byte[] withEncap(byte[] encap, ParameterList pl) {
        Wire w = new Wire();
        w.bytes(encap);
        w.bytes(pl.toBytesLe());
        return w.toBytes();
    }

    private static ParameterList plFromPayload(byte[] payload) {
        if (payload == null || payload.length < 4) {
            return null;
        }
        // Accept PL_CDR_LE (00 03) — the only scheme the Rust discovery encoder emits.
        if (payload[0] != 0x00 || (payload[1] != 0x03 && payload[1] != 0x02)) {
            return null;
        }
        boolean le = payload[1] == 0x03;
        if (!le) {
            return null; // BE discovery not needed for interop
        }
        return ParameterList.fromBytesLe(payload, 4, payload.length);
    }

    /** Unused list helper kept for symmetry with the Rust find_all API. */
    static List<byte[]> findAll(ParameterList pl, int id) {
        List<byte[]> out = new ArrayList<>();
        for (ParameterList.Parameter p : pl.params()) {
            if ((p.id & 0x3FFF) == (id & 0x3FFF)) {
                out.add(p.value);
            }
        }
        return out;
    }
}
