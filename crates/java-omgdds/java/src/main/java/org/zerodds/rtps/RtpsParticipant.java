// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.io.IOException;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.NetworkInterface;
import java.net.StandardProtocolFamily;
import java.net.StandardSocketOptions;
import java.nio.ByteBuffer;
import java.nio.channels.DatagramChannel;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;

/**
 * Pure-Java RTPS participant: the native wire stack behind the DDS Java PSM.
 * No FFI, no Rust library — {@link DatagramChannel} UDP sockets carry SPDP
 * (multicast participant discovery), SEDP (unicast endpoint discovery, with the
 * cross-binding TypeIdentifier), and best-effort user DATA. Interoperates on
 * the wire with the Rust zerodds stack (DDSI-RTPS 2.5).
 *
 * <p>One participant per (domain, JVM) by default via {@link #get(int)}; the
 * sockets and discovery threads start on first use.
 */
public final class RtpsParticipant {

    // ---- global registry (one participant per domain) ----
    private static final Map<Integer, RtpsParticipant> BY_DOMAIN = new ConcurrentHashMap<>();

    public static RtpsParticipant get(int domainId) {
        return BY_DOMAIN.computeIfAbsent(domainId, RtpsParticipant::new);
    }

    /**
     * Create a fresh, non-cached participant on {@code domainId} (its own GUID
     * prefix and unicast port). Used for multi-participant loopback tests and
     * apps that want more than one participant per domain in a JVM.
     */
    public static RtpsParticipant newParticipant(int domainId) {
        return new RtpsParticipant(domainId);
    }

    /** Optional match/event log sink (System.err by default). */
    public static volatile Consumer<String> LOG = msg -> System.err.println("[rtps] " + msg);

    private static void log(String m) {
        Consumer<String> l = LOG;
        if (l != null) {
            l.accept(m);
        }
    }

    // ---- endpoint registrations ----

    public final class WireWriter {
        final byte[] entityId; // 4
        final byte[] guid; // 16
        final String topic;
        final String typeName;
        final byte[] encap; // 4-byte encapsulation header
        final byte[] typeId;
        final boolean keyed;
        final boolean reliable;
        long sn = 0;
        // matched remote readers: guidHex -> target
        final Map<String, MatchedReader> matched = new ConcurrentHashMap<>();

        WireWriter(byte[] entityId, String topic, String typeName, byte[] encap,
                   byte[] typeId, boolean keyed, boolean reliable) {
            this.entityId = entityId;
            this.guid = new Guid(guidPrefix, entityId).bytes();
            this.topic = topic;
            this.typeName = typeName;
            this.encap = encap;
            this.typeId = typeId == null ? new byte[0] : typeId;
            this.keyed = keyed;
            this.reliable = reliable;
        }

        /** Send one sample body (encapsulation is prepended) to all matched readers. */
        public synchronized void write(byte[] body) {
            sn++;
            byte[] payload = new byte[encap.length + body.length];
            System.arraycopy(encap, 0, payload, 0, encap.length);
            System.arraycopy(body, 0, payload, encap.length, body.length);
            long now = System.currentTimeMillis();
            int secs = (int) (now / 1000L);
            long frac = ((now % 1000L) << 32) / 1000L;
            for (MatchedReader mr : matched.values()) {
                RtpsMessage.Builder b = new RtpsMessage.Builder(guidPrefix)
                        .infoDestination(mr.participantPrefix)
                        .infoTimestamp(secs, frac)
                        .data(mr.readerEntityId, entityId, sn, null, payload, false);
                sendTo(b.toBytes(), mr.locator);
            }
        }

        public int matchedCount() {
            return matched.size();
        }
    }

    public final class WireReader {
        final byte[] entityId; // 4
        final byte[] guid; // 16
        final String topic;
        final String typeName;
        final boolean keyed;
        final Consumer<byte[]> onSample; // receives the body (encap stripped)
        // matched remote writer GUIDs (hex) that we accept DATA from
        final Map<String, Boolean> matchedWriters = new ConcurrentHashMap<>();

        WireReader(byte[] entityId, String topic, String typeName, boolean keyed,
                   Consumer<byte[]> onSample) {
            this.entityId = entityId;
            this.guid = new Guid(guidPrefix, entityId).bytes();
            this.topic = topic;
            this.typeName = typeName;
            this.keyed = keyed;
            this.onSample = onSample;
        }

        public int matchedCount() {
            return matchedWriters.size();
        }
    }

    static final class MatchedReader {
        final byte[] participantPrefix;
        final byte[] readerEntityId;
        final Locator locator;

        MatchedReader(byte[] participantPrefix, byte[] readerEntityId, Locator locator) {
            this.participantPrefix = participantPrefix;
            this.readerEntityId = readerEntityId;
            this.locator = locator;
        }
    }

    // ---- state ----
    private final int domainId;
    private final int participantId;
    private final byte[] guidPrefix; // 12
    private final AtomicInteger entityKeyCounter = new AtomicInteger(1);

    private final List<WireWriter> writers = new ArrayList<>();
    private final List<WireReader> readers = new ArrayList<>();

    private final Map<String, DiscoveryData.Participant> peers = new ConcurrentHashMap<>();
    private final Map<String, DiscoveryData.Endpoint> remoteWriters = new ConcurrentHashMap<>();
    private final Map<String, DiscoveryData.Endpoint> remoteReaders = new ConcurrentHashMap<>();
    // Reliable builtin-reader bookkeeping: highest SN received + ACKNACK count
    // per remote builtin writer GUID (SPDP/SEDP).
    private final Map<String, Long> recvHigh = new ConcurrentHashMap<>();
    private final Map<String, Integer> ackCounters = new ConcurrentHashMap<>();

    private DatagramChannel spdpChannel; // multicast recv + send
    private DatagramChannel unicastChannel; // metatraffic + user recv/send
    private Locator announceLocator; // what we advertise as meta + default unicast
    private final List<NetworkInterface> mcastIfs = new ArrayList<>();
    private volatile boolean running = false;
    private long spdpWriterSn = 0;
    private long sedpPubSn = 0;
    private long sedpSubSn = 0;

    private RtpsParticipant(int domainId) {
        this.domainId = domainId;
        this.guidPrefix = new byte[12];
        Random rnd = new Random();
        // bytes 0..2 = vendor id (interop-friendly), 2..8 host/pid, 8..12 random
        guidPrefix[0] = Rtps.VENDOR_ID[0];
        guidPrefix[1] = Rtps.VENDOR_ID[1];
        int host;
        try {
            host = InetAddress.getLocalHost().getHostName().hashCode();
        } catch (Exception e) {
            host = rnd.nextInt();
        }
        guidPrefix[2] = (byte) (host >>> 24);
        guidPrefix[3] = (byte) (host >>> 16);
        long pidTag = ProcessHandle.current().pid();
        guidPrefix[4] = (byte) (pidTag >>> 8);
        guidPrefix[5] = (byte) pidTag;
        for (int i = 6; i < 12; i++) {
            guidPrefix[i] = (byte) rnd.nextInt();
        }
        this.participantId = start();
    }

    public byte[] guidPrefix() {
        return guidPrefix.clone();
    }

    // ---- socket + thread setup ----

    private int start() {
        try {
            String bindHost = System.getProperty("zerodds.rtps.bind", "0.0.0.0");
            String announceHost = System.getProperty("zerodds.rtps.host", primaryIpv4());

            // SPDP multicast socket, bound to the domain's multicast port.
            int mcastPort = Rtps.spdpMulticastPort(domainId);
            spdpChannel = DatagramChannel.open(StandardProtocolFamily.INET);
            spdpChannel.setOption(StandardSocketOptions.SO_REUSEADDR, true);
            spdpChannel.bind(new InetSocketAddress(mcastPort));
            spdpChannel.setOption(StandardSocketOptions.IP_MULTICAST_LOOP, true);
            InetAddress group = InetAddress.getByName(Rtps.SPDP_MULTICAST_ADDRESS);
            for (NetworkInterface nif : multicastInterfaces()) {
                try {
                    spdpChannel.join(group, nif);
                    mcastIfs.add(nif); // remember for per-interface beacon egress
                } catch (Exception ignore) {
                    // interface may not support the group; keep going
                }
            }

            // Unicast socket: probe participant ids for a free metatraffic port.
            int pid = 0;
            for (; pid < 120; pid++) {
                int port = Rtps.metatrafficUnicastPort(domainId, pid);
                try {
                    unicastChannel = DatagramChannel.open(StandardProtocolFamily.INET);
                    // No SO_REUSEADDR here: each participant must own a distinct
                    // unicast port, so the probe advances on a collision.
                    unicastChannel.bind(new InetSocketAddress(bindHost, port));
                    break;
                } catch (IOException busy) {
                    if (unicastChannel != null) {
                        unicastChannel.close();
                        unicastChannel = null;
                    }
                }
            }
            if (unicastChannel == null) {
                throw new IOException("no free RTPS unicast port");
            }
            int boundPort = ((InetSocketAddress) unicastChannel.getLocalAddress()).getPort();
            announceLocator = Locator.udpV4(announceHost, boundPort);

            running = true;
            Thread t1 = new Thread(this::spdpRecvLoop, "rtps-spdp-" + domainId);
            t1.setDaemon(true);
            t1.start();
            Thread t2 = new Thread(this::unicastRecvLoop, "rtps-unicast-" + domainId);
            t2.setDaemon(true);
            t2.start();
            Thread t3 = new Thread(this::beaconLoop, "rtps-beacon-" + domainId);
            t3.setDaemon(true);
            t3.start();

            log("participant up: domain=" + domainId + " prefix=" + hex(guidPrefix)
                    + " unicast=" + announceLocator + " spdp-mcast=" + mcastPort);
            return pid;
        } catch (IOException e) {
            throw new RuntimeException("RTPS participant start failed", e);
        }
    }

    private static List<NetworkInterface> multicastInterfaces() {
        List<NetworkInterface> out = new ArrayList<>();
        try {
            Enumeration<NetworkInterface> e = NetworkInterface.getNetworkInterfaces();
            while (e.hasMoreElements()) {
                NetworkInterface nif = e.nextElement();
                if (nif.isUp() && nif.supportsMulticast()) {
                    out.add(nif);
                }
            }
        } catch (Exception ignore) {
            // fall through
        }
        return out;
    }

    // ---- public API ----

    public synchronized WireWriter createWriter(String topic, String typeName, byte[] encap,
                                                byte[] typeId, boolean keyed, boolean reliable) {
        int k = entityKeyCounter.getAndIncrement();
        byte[] eid = Guid.userEntityId(0, k >>> 8, k,
                keyed ? Guid.KIND_USER_WRITER_WITH_KEY : Guid.KIND_USER_WRITER_NO_KEY);
        WireWriter w = new WireWriter(eid, topic, typeName, encap, typeId, keyed, reliable);
        writers.add(w);
        // match against already-discovered remote readers
        for (DiscoveryData.Endpoint r : remoteReaders.values()) {
            tryMatchWriter(w, r);
        }
        announceAllToPeers();
        return w;
    }

    public synchronized WireReader createReader(String topic, String typeName, boolean keyed,
                                                Consumer<byte[]> onSample) {
        int k = entityKeyCounter.getAndIncrement();
        byte[] eid = Guid.userEntityId(0, k >>> 8, k,
                keyed ? Guid.KIND_USER_READER_WITH_KEY : Guid.KIND_USER_READER_NO_KEY);
        WireReader r = new WireReader(eid, topic, typeName, keyed, onSample);
        readers.add(r);
        for (DiscoveryData.Endpoint w : remoteWriters.values()) {
            tryMatchReader(r, w);
        }
        announceAllToPeers();
        return r;
    }

    // ---- SPDP ----

    private void beaconLoop() {
        while (running) {
            try {
                sendSpdpBeacon();
            } catch (Exception e) {
                log("beacon send failed: " + e);
            }
            try {
                Thread.sleep(1000);
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    private void sendSpdpBeacon() throws IOException {
        byte[] payload = DiscoveryData.buildParticipant(guidPrefix, domainId,
                announceLocator, announceLocator, Rtps.EP_ALL_BASIC, 100);
        spdpWriterSn++;
        RtpsMessage.Builder b = new RtpsMessage.Builder(guidPrefix)
                .infoTimestamp((int) (System.currentTimeMillis() / 1000L), 0)
                .data(Guid.ENTITYID_UNKNOWN, Guid.SPDP_WRITER, spdpWriterSn, null, payload, false);
        byte[] dg = b.toBytes();
        InetSocketAddress groupAddr = new InetSocketAddress(
                InetAddress.getByName(Rtps.SPDP_MULTICAST_ADDRESS).getHostAddress(),
                Rtps.spdpMulticastPort(domainId));
        // Egress the beacon on every joined multicast interface — the peer may
        // listen on a different one than our default multicast route.
        for (NetworkInterface nif : mcastIfs) {
            try {
                spdpChannel.setOption(StandardSocketOptions.IP_MULTICAST_IF, nif);
                spdpChannel.send(ByteBuffer.wrap(dg), groupAddr);
            } catch (Exception ignore) {
                // interface may not route the group; try the next
            }
        }
    }

    /** First non-loopback IPv4 address, or 127.0.0.1. Advertised so peers reach us. */
    private static String primaryIpv4() {
        try {
            for (NetworkInterface nif : multicastInterfaces()) {
                if (nif.isLoopback()) {
                    continue;
                }
                Enumeration<InetAddress> addrs = nif.getInetAddresses();
                while (addrs.hasMoreElements()) {
                    InetAddress a = addrs.nextElement();
                    if (a.getAddress().length == 4 && !a.isLoopbackAddress()) {
                        return a.getHostAddress();
                    }
                }
            }
        } catch (Exception ignore) {
            // fall through
        }
        return "127.0.0.1";
    }

    private void spdpRecvLoop() {
        ByteBuffer buf = ByteBuffer.allocate(65536);
        while (running) {
            try {
                buf.clear();
                java.net.SocketAddress src = spdpChannel.receive(buf);
                buf.flip();
                byte[] arr = new byte[buf.remaining()];
                buf.get(arr);
                handleDatagram(arr, arr.length, src);
            } catch (Exception e) {
                if (running) {
                    // transient; keep going
                }
            }
        }
    }

    private void unicastRecvLoop() {
        ByteBuffer buf = ByteBuffer.allocate(65536);
        while (running) {
            try {
                buf.clear();
                java.net.SocketAddress src = unicastChannel.receive(buf);
                buf.flip();
                byte[] arr = new byte[buf.remaining()];
                buf.get(arr);
                handleDatagram(arr, arr.length, src);
            } catch (Exception e) {
                if (running) {
                    // transient; keep going
                }
            }
        }
    }

    private static final boolean DEBUG =
            Boolean.parseBoolean(System.getProperty("zerodds.rtps.debug", "false"));

    private void handleDatagram(byte[] arr, int len, java.net.SocketAddress src) {
        RtpsMessage.Parsed p = RtpsMessage.decode(arr, len);
        if (p == null || p.header.guidPrefix == null) {
            if (DEBUG) {
                log("RX undecodable dg len=" + len);
            }
            return;
        }
        if (equalsBytes(p.header.guidPrefix, guidPrefix)) {
            return; // our own multicast loopback
        }
        if (DEBUG) {
            StringBuilder sb = new StringBuilder("RX dg len=" + len + " from "
                    + hex(p.header.guidPrefix) + " datas=" + p.data.size());
            for (RtpsMessage.Data d : p.data) {
                sb.append(" [w=").append(hex(d.writerId)).append(" r=").append(hex(d.readerId))
                        .append(" sn=").append(d.writerSn)
                        .append(" plen=").append(d.serializedPayload.length);
                if (d.serializedPayload.length >= 2) {
                    sb.append(" encap=").append(String.format("%02x%02x",
                            d.serializedPayload[0], d.serializedPayload[1]));
                }
                sb.append("]");
            }
            log(sb.toString());
        }
        for (RtpsMessage.Data d : p.data) {
            int writerKind = d.writerId[3] & 0xFF;
            if (equalsBytes(d.writerId, Guid.SPDP_WRITER)) {
                noteBuiltinReceived(p.header.guidPrefix, d.writerId, d.writerSn);
                onSpdp(d);
            } else if (equalsBytes(d.writerId, Guid.SEDP_PUB_WRITER)) {
                noteBuiltinReceived(p.header.guidPrefix, d.writerId, d.writerSn);
                onSedpEndpoint(true, d);
            } else if (equalsBytes(d.writerId, Guid.SEDP_SUB_WRITER)) {
                noteBuiltinReceived(p.header.guidPrefix, d.writerId, d.writerSn);
                onSedpEndpoint(false, d);
            } else if (writerKind == Guid.KIND_USER_WRITER_NO_KEY
                    || writerKind == Guid.KIND_USER_WRITER_WITH_KEY) {
                onUserData(p.header.guidPrefix, d);
            }
        }
        // Reliable-reader ACKNACK: a reliable writer (builtin SEDP discovery, or
        // a matched remote user writer) only sends DATA once we ACKNACK its
        // heartbeat (§8.4.15). Builtins are always pulled; user writers only if
        // one of our readers matched them.
        for (RtpsMessage.Heartbeat hb : p.heartbeats) {
            int wk = hb.writerId[3] & 0xFF;
            boolean builtin = wk == Guid.KIND_BUILTIN_WRITER_WITH_KEY || wk == 0xC3;
            boolean userMatched = (wk == Guid.KIND_USER_WRITER_NO_KEY
                    || wk == Guid.KIND_USER_WRITER_WITH_KEY)
                    && readerMatchesWriter(hex(new Guid(p.header.guidPrefix, hb.writerId).bytes()));
            if (builtin || userMatched) {
                onWriterHeartbeat(p.header.guidPrefix, hb, src);
            }
        }
    }

    private boolean readerMatchesWriter(String writerGuidHex) {
        for (WireReader r : readers) {
            if (r.matchedWriters.containsKey(writerGuidHex)) {
                return true;
            }
        }
        return false;
    }

    private void noteBuiltinReceived(byte[] senderPrefix, byte[] writerId, long sn) {
        String wkey = hex(new Guid(senderPrefix, writerId).bytes());
        recvHigh.merge(wkey, sn, Math::max);
    }

    private void onWriterHeartbeat(byte[] senderPrefix, RtpsMessage.Heartbeat hb,
                                    java.net.SocketAddress src) {
        String wkey = hex(new Guid(senderPrefix, hb.writerId).bytes());
        long recv = recvHigh.getOrDefault(wkey, 0L);
        long base = Math.max(1L, recv + 1);
        int numBits = 0;
        if (hb.lastSn >= base) {
            numBits = (int) Math.min(256, hb.lastSn - base + 1);
        }
        byte[] readerId = hb.readerId;
        if (readerId == null || equalsBytes(readerId, Guid.ENTITYID_UNKNOWN)) {
            readerId = deriveBuiltinReader(hb.writerId);
        }
        int count = ackCounters.merge(wkey, 1, Integer::sum);
        byte[] dg = new RtpsMessage.Builder(guidPrefix)
                .infoDestination(senderPrefix)
                .ackNack(readerId, hb.writerId, base, numBits, count, false)
                .toBytes();
        DiscoveryData.Participant peer = peers.get(hex(senderPrefix));
        String tgt;
        if (peer != null && peer.metatrafficUnicast != null
                && peer.metatrafficUnicast.isUsableUdpV4()) {
            sendTo(dg, peer.metatrafficUnicast);
            tgt = peer.metatrafficUnicast.toString();
        } else if (src instanceof InetSocketAddress) {
            sendToAddr(dg, (InetSocketAddress) src);
            tgt = src.toString();
        } else {
            tgt = "(none)";
        }
        if (DEBUG) {
            log("ACKNACK w=" + hex(hb.writerId) + " r=" + hex(readerId) + " base=" + base
                    + " numBits=" + numBits + " (hb first=" + hb.firstSn + " last=" + hb.lastSn
                    + ") -> " + tgt);
        }
    }

    private static byte[] deriveBuiltinReader(byte[] writerId) {
        byte[] r = writerId.clone();
        int k = r[3] & 0xFF;
        r[3] = (byte) (k == Guid.KIND_BUILTIN_WRITER_WITH_KEY ? Guid.KIND_BUILTIN_READER_WITH_KEY
                : (k == 0xC3 ? 0xC4 : k));
        return r;
    }

    private void onSpdp(RtpsMessage.Data d) {
        DiscoveryData.Participant peer = DiscoveryData.parseParticipant(d.serializedPayload);
        if (peer == null || peer.guidPrefix == null) {
            return;
        }
        if (equalsBytes(peer.guidPrefix, guidPrefix)) {
            return;
        }
        peer.lastSeenMillis = System.currentTimeMillis();
        String key = hex(peer.guidPrefix);
        boolean isNew = !peers.containsKey(key);
        peers.put(key, peer);
        if (isNew) {
            log("SPDP discovered participant " + key
                    + (peer.metatrafficUnicast != null ? " meta=" + peer.metatrafficUnicast : ""));
            announceAllTo(peer);
        }
    }

    private void onSedpEndpoint(boolean writer, RtpsMessage.Data d) {
        DiscoveryData.Endpoint e = DiscoveryData.parseEndpoint(writer, d.serializedPayload);
        if (e == null) {
            if (DEBUG) {
                log("SEDP parse FAILED (" + (writer ? "pub" : "sub") + ") plen="
                        + d.serializedPayload.length + " encap="
                        + (d.serializedPayload.length >= 2 ? String.format("%02x%02x",
                        d.serializedPayload[0], d.serializedPayload[1]) : "?"));
            }
            return;
        }
        String key = hex(e.endpointGuid);
        if (writer) {
            if (remoteWriters.put(key, e) == null) {
                log("SEDP discovered remote WRITER topic='" + e.topicName + "' type='"
                        + e.typeName + "' guid=" + key
                        + (e.typeId != null && e.typeId.length > 0 ? " typeId=" + hex(e.typeId) : ""));
            }
            synchronized (this) {
                for (WireReader r : readers) {
                    tryMatchReader(r, e);
                }
            }
        } else {
            if (remoteReaders.put(key, e) == null) {
                log("SEDP discovered remote READER topic='" + e.topicName + "' type='"
                        + e.typeName + "' guid=" + key
                        + (e.typeId != null && e.typeId.length > 0 ? " typeId=" + hex(e.typeId) : ""));
            }
            synchronized (this) {
                for (WireWriter w : writers) {
                    tryMatchWriter(w, e);
                }
            }
        }
    }

    private void tryMatchWriter(WireWriter w, DiscoveryData.Endpoint remoteReader) {
        if (!w.topic.equals(remoteReader.topicName) || !w.typeName.equals(remoteReader.typeName)) {
            return;
        }
        if (remoteReader.unicastLocator == null || !remoteReader.unicastLocator.isUsableUdpV4()) {
            return;
        }
        String key = hex(remoteReader.endpointGuid);
        if (w.matched.containsKey(key)) {
            return;
        }
        byte[] readerEntity = Wire.slice(remoteReader.endpointGuid, 12, 4);
        w.matched.put(key, new MatchedReader(
                Wire.slice(remoteReader.endpointGuid, 0, 12), readerEntity,
                remoteReader.unicastLocator));
        log("SEDP endpoint-match: local WRITER topic='" + w.topic + "' type='" + w.typeName
                + "' -> remote reader " + key + " @ " + remoteReader.unicastLocator
                + typeIdNote(w.typeId, remoteReader.typeId));
    }

    private void tryMatchReader(WireReader r, DiscoveryData.Endpoint remoteWriter) {
        if (!r.topic.equals(remoteWriter.topicName) || !r.typeName.equals(remoteWriter.typeName)) {
            return;
        }
        String key = hex(remoteWriter.endpointGuid);
        if (r.matchedWriters.putIfAbsent(key, Boolean.TRUE) == null) {
            log("SEDP endpoint-match: local READER topic='" + r.topic + "' type='" + r.typeName
                    + "' <- remote writer " + key
                    + typeIdNote(new byte[0], remoteWriter.typeId));
        }
    }

    private static String typeIdNote(byte[] localId, byte[] remoteId) {
        if (remoteId == null || remoteId.length == 0) {
            return "";
        }
        return " [TypeIdentifier " + hex(remoteId) + "]";
    }

    private void onUserData(byte[] senderPrefix, RtpsMessage.Data d) {
        byte[] writerGuid = new byte[16];
        System.arraycopy(senderPrefix, 0, writerGuid, 0, 12);
        System.arraycopy(d.writerId, 0, writerGuid, 12, 4);
        String wkey = hex(writerGuid);
        // Track highest received SN so our ACKNACK to the reliable writer
        // advances (positive-acks delivered samples, stops retransmits).
        recvHigh.merge(wkey, d.writerSn, Math::max);
        byte[] body = stripEncap(d.serializedPayload);
        if (body == null) {
            return;
        }
        for (WireReader r : readers) {
            boolean addressed = equalsBytes(d.readerId, r.entityId)
                    || equalsBytes(d.readerId, Guid.ENTITYID_UNKNOWN);
            if (addressed && r.matchedWriters.containsKey(wkey)) {
                try {
                    r.onSample.accept(body);
                } catch (Exception ex) {
                    log("reader callback threw: " + ex);
                }
            }
        }
    }

    /** Strip the 4-byte encapsulation header (RTPS 2.5 §10.5). */
    static byte[] stripEncap(byte[] payload) {
        if (payload.length < 4 || payload[0] != 0x00) {
            return payload; // no known scheme; pass through
        }
        int scheme = payload[1] & 0xFF;
        if ((scheme >= 0x00 && scheme <= 0x03) || (scheme >= 0x06 && scheme <= 0x0b)) {
            return Wire.slice(payload, 4, payload.length - 4);
        }
        return payload;
    }

    // ---- SEDP announce ----

    private void announceAllToPeers() {
        for (DiscoveryData.Participant peer : peers.values()) {
            announceAllTo(peer);
        }
    }

    private synchronized void announceAllTo(DiscoveryData.Participant peer) {
        Locator target = peer.metatrafficUnicast;
        if (target == null || !target.isUsableUdpV4()) {
            return;
        }
        if (!writers.isEmpty() || !readers.isEmpty()) {
            log("SEDP announce " + writers.size() + " writer(s) + " + readers.size()
                    + " reader(s) to peer " + hex(peer.guidPrefix) + " @ " + target);
        }
        for (WireWriter w : writers) {
            byte[] payload = DiscoveryData.buildEndpoint(true, guidPrefix, w.guid, w.topic,
                    w.typeName, w.reliable ? Rtps.RELIABILITY_RELIABLE : Rtps.RELIABILITY_BEST_EFFORT,
                    Rtps.DURABILITY_VOLATILE, new int[] {encapDataRep(w.encap)}, announceLocator,
                    w.typeId);
            sedpPubSn++;
            RtpsMessage.Builder b = new RtpsMessage.Builder(guidPrefix)
                    .infoDestination(peer.guidPrefix)
                    .data(Guid.SEDP_PUB_READER, Guid.SEDP_PUB_WRITER, sedpPubSn, null, payload, false);
            sendTo(b.toBytes(), target);
        }
        for (WireReader r : readers) {
            // A reader advertises every representation it can decode (XCDR2+XCDR1)
            // so it matches writers that offer either.
            byte[] payload = DiscoveryData.buildEndpoint(false, guidPrefix, r.guid, r.topic,
                    r.typeName, Rtps.RELIABILITY_BEST_EFFORT, Rtps.DURABILITY_VOLATILE,
                    new int[] {Rtps.DATA_REP_XCDR2, Rtps.DATA_REP_XCDR1}, announceLocator, new byte[0]);
            sedpSubSn++;
            RtpsMessage.Builder b = new RtpsMessage.Builder(guidPrefix)
                    .infoDestination(peer.guidPrefix)
                    .data(Guid.SEDP_SUB_READER, Guid.SEDP_SUB_WRITER, sedpSubSn, null, payload, false);
            sendTo(b.toBytes(), target);
        }
    }

    private static int encapDataRep(byte[] encap) {
        // encap[1] >= 0x06 => XCDR2, else XCDR1
        return (encap.length >= 2 && (encap[1] & 0xFF) >= 0x06) ? Rtps.DATA_REP_XCDR2 : Rtps.DATA_REP_XCDR1;
    }

    // ---- send ----

    private void sendTo(byte[] datagram, Locator loc) {
        try {
            InetSocketAddress addr = new InetSocketAddress(
                    InetAddress.getByAddress(loc.ipv4()), (int) loc.port);
            unicastChannel.send(ByteBuffer.wrap(datagram), addr);
        } catch (Exception e) {
            log("send failed to " + loc + ": " + e);
        }
    }

    private void sendToAddr(byte[] datagram, InetSocketAddress addr) {
        try {
            unicastChannel.send(ByteBuffer.wrap(datagram), addr);
        } catch (Exception e) {
            log("send failed to " + addr + ": " + e);
        }
    }

    public void close() {
        running = false;
        try {
            if (spdpChannel != null) {
                spdpChannel.close();
            }
            if (unicastChannel != null) {
                unicastChannel.close();
            }
        } catch (IOException ignore) {
            // best effort
        }
        BY_DOMAIN.remove(domainId, this);
    }

    // ---- utils ----

    private static boolean equalsBytes(byte[] a, byte[] b) {
        if (a == null || b == null || a.length != b.length) {
            return false;
        }
        for (int i = 0; i < a.length; i++) {
            if (a[i] != b[i]) {
                return false;
            }
        }
        return true;
    }

    static String hex(byte[] b) {
        StringBuilder sb = new StringBuilder(b.length * 2);
        for (byte x : b) {
            sb.append(String.format("%02x", x));
        }
        return sb.toString();
    }
}
