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
        /**
         * The participant's USER_DATA (DDS 1.4 §2.2.5
         * ParticipantBuiltinTopicData, {@code PID_USER_DATA}); empty if the
         * announcement has none.
         */
        public byte[] userData = new byte[0];
    }

    /** Build the SPDP participant payload (encapsulation + ParameterList). */
    public static byte[] buildParticipant(byte[] guidPrefix12, int domain,
                                          Locator metatrafficUnicast, Locator defaultUnicast,
                                          long builtinEndpointSet, int leaseSeconds) {
        return buildParticipant(guidPrefix12, domain, metatrafficUnicast, defaultUnicast,
                builtinEndpointSet, leaseSeconds, new byte[0]);
    }

    /** As above, with the USER_DATA of the participant. */
    public static byte[] buildParticipant(byte[] guidPrefix12, int domain,
                                          Locator metatrafficUnicast, Locator defaultUnicast,
                                          long builtinEndpointSet, int leaseSeconds,
                                          byte[] userData) {
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
        QosParameters user = new QosParameters();
        user.userData = userData;
        user.writeTo(pl, QosParameters.Entity.PARTICIPANT);
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
        try {
            byte[] user = QosParameters.readFrom(pl, QosParameters.Entity.PARTICIPANT).userData;
            p.userData = user == null ? new byte[0] : user;
        } catch (IllegalArgumentException malformed) {
            return null;
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
        public byte[] typeInformation; // Standard 0x75 bytes; retained, not a type-resolution result
        // An absent DataRepresentation QoS has the DDS XCDR1 default.
        public int[] dataRepresentations = new int[] {Rtps.DATA_REP_XCDR1};
        /**
         * The QoS policies of the announcement (DDS 1.4 §2.2.5, DDSI-RTPS 2.5
         * Table 9.13); a policy with no parameter is {@code null} in it.
         */
        public QosParameters qos = new QosParameters();
        /** The DDS-RPC 1.0 §7.6.2.1 discovery extensions of the endpoint. */
        public RpcDiscovery rpc = RpcDiscovery.NONE;
    }

    /**
     * Build an SEDP endpoint payload. {@code unicast} is the locator on which
     * this endpoint's participant receives user traffic (default unicast).
     */
    public static byte[] buildEndpoint(boolean writer, byte[] participantPrefix12,
                                       byte[] endpointGuid16, String topic, String type,
                                       int reliabilityKind, int durabilityKind,
                                       int[] dataReps, Locator unicast, byte[] typeId) {
        return buildEndpoint(writer, participantPrefix12, endpointGuid16, topic, type,
                reliabilityKind, durabilityKind, dataReps, unicast, typeId, new byte[0]);
    }

    public static byte[] buildEndpoint(boolean writer, byte[] participantPrefix12,
                                       byte[] endpointGuid16, String topic, String type,
                                       int reliabilityKind, int durabilityKind,
                                       int[] dataReps, Locator unicast, byte[] typeId, byte[] typeInformation) {
        return buildEndpoint(writer, participantPrefix12, endpointGuid16, topic, type,
                reliabilityKind, durabilityKind, dataReps, unicast, typeId, typeInformation, null);
    }

    /**
     * Builds an SEDP endpoint payload that carries {@code qos}: every policy
     * of the endpoint's built-in topic data (DDS 1.4 §2.2.5) on its
     * ParameterId. With {@code qos == null} only the reliability and
     * durability kinds are announced, as the overloads without it do.
     */
    public static byte[] buildEndpoint(boolean writer, byte[] participantPrefix12,
                                       byte[] endpointGuid16, String topic, String type,
                                       int reliabilityKind, int durabilityKind,
                                       int[] dataReps, Locator unicast, byte[] typeId, byte[] typeInformation,
                                       QosParameters qos) {
        return buildEndpoint(writer, participantPrefix12, endpointGuid16, topic, type,
                reliabilityKind, durabilityKind, dataReps, unicast, typeId, typeInformation, qos,
                RpcDiscovery.NONE);
    }

    /**
     * As the overload with {@code qos}, and with the DDS-RPC 1.0 §7.6.2.1.2 parameters
     * {@code rpc} carries (nothing for {@link RpcDiscovery#NONE}).
     */
    public static byte[] buildEndpoint(boolean writer, byte[] participantPrefix12,
                                       byte[] endpointGuid16, String topic, String type,
                                       int reliabilityKind, int durabilityKind,
                                       int[] dataReps, Locator unicast, byte[] typeId, byte[] typeInformation,
                                       QosParameters qos, RpcDiscovery rpc) {
        ParameterList pl = new ParameterList();
        pl.add(Rtps.PID_PARTICIPANT_GUID,
                new Guid(participantPrefix12, Guid.ENTITYID_PARTICIPANT).bytes());
        pl.add(Rtps.PID_ENDPOINT_GUID, endpointGuid16.clone());
        pl.add(Rtps.PID_TOPIC_NAME, ParameterList.cdrString(topic));
        pl.add(Rtps.PID_TYPE_NAME, ParameterList.cdrString(type));
        if (qos == null) {
            pl.add(Rtps.PID_DURABILITY, ParameterList.u32(durabilityKind));
            pl.add(Rtps.PID_RELIABILITY, ParameterList.kindDuration(reliabilityKind, 0, 0x19999999L));
            if (writer) {
                pl.add(Rtps.PID_OWNERSHIP_STRENGTH, ParameterList.u32(0));
            }
        } else {
            // The data representations are the endpoint's own (the codec it
            // implements), whatever the QoS object holds.
            QosParameters announced = qos.shallowCopy();
            announced.dataRepresentation = dataReps;
            if (typeInformation != null && typeInformation.length > 0) {
                announced.typeInformation = typeInformation;
            }
            announced.writeTo(pl, writer ? QosParameters.Entity.PUBLICATION
                    : QosParameters.Entity.SUBSCRIPTION);
        }
        if (typeId != null && typeId.length > 0) {
            pl.add(Rtps.PID_ZERODDS_TYPE_ID, typeId.clone());
        }
        if (typeInformation == null) throw new IllegalArgumentException("Null TypeInformation");
        if (typeInformation.length > 0) {
            // ParameterList's length field is unsigned 16-bit, including its
            // four-byte padding. Refuse overflow rather than truncate metadata.
            if (typeInformation.length > 65532) throw new IllegalArgumentException("TypeInformation exceeds SEDP parameter length");
            if (pl.find(Rtps.PID_TYPE_INFORMATION) == null) {
                pl.add(Rtps.PID_TYPE_INFORMATION, typeInformation.clone());
            }
        }
        if (pl.find(Rtps.PID_DATA_REPRESENTATION) == null) {
            pl.add(Rtps.PID_DATA_REPRESENTATION, ParameterList.dataRepresentation(dataReps));
        }
        // SEDP endpoints announce their locator via PID_UNICAST_LOCATOR (0x2f),
        // not the SPDP PID_DEFAULT_UNICAST_LOCATOR (0x31).
        pl.add(Rtps.PID_UNICAST_LOCATOR, locatorBytes(unicast));
        rpc.writeTo(pl);
        return withEncap(Rtps.ENCAP_PL_CDR_LE, pl);
    }

    public static Endpoint parseEndpoint(boolean writer, byte[] payload) {
        ParameterList pl = plFromPayload(payload);
        if (pl == null) {
            return null;
        }
        Endpoint e = new Endpoint();
        e.writer = writer;
        byte[] reps = pl.find(Rtps.PID_DATA_REPRESENTATION);
        if (reps != null) {
            if (reps.length < 4) return null;
            long count = Wire.u32le(reps, 0);
            if (count == 0 || count > (reps.length - 4) / 2 || (writer && count != 1)) return null;
            e.dataRepresentations = new int[(int) count];
            for (int i = 0; i < count; i++) {
                e.dataRepresentations[i] = (reps[4 + 2 * i] & 255) | ((reps[5 + 2 * i] & 255) << 8);
            }
        }
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
        try {
            e.qos = QosParameters.readFrom(pl, writer
                    ? QosParameters.Entity.PUBLICATION : QosParameters.Entity.SUBSCRIPTION);
        } catch (IllegalArgumentException malformed) {
            return null;
        }
        RpcDiscovery rpc = RpcDiscovery.readFrom(pl);
        if (rpc == null) {
            return null; // a string beyond string<256> (DDS-RPC 1.0 §7.6.2.1.2)
        }
        e.rpc = rpc;
        e.typeId = pl.find(Rtps.PID_ZERODDS_TYPE_ID);
        e.typeInformation = pl.find(Rtps.PID_TYPE_INFORMATION);
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

    /** All matching parameters, used to ingest multiple endpoint locators. */
    static List<byte[]> findAll(ParameterList pl, int id) {
        List<byte[]> out = new ArrayList<>();
        for (ParameterList.Parameter p : pl.params()) {
            if (ParameterList.sameId(p.id, id)) {
                out.add(p.value);
            }
        }
        return out;
    }
}
