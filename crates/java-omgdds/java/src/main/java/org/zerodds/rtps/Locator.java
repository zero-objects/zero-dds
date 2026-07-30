// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.net.InetAddress;
import java.net.UnknownHostException;
import java.util.Arrays;

/**
 * RTPS Locator (24 bytes = 4-byte kind i32 + 4-byte port u32 + 16-byte
 * address). Ported from {@code crates/rtps/src/wire_types.rs}. For UDPv4 the
 * 4-byte IPv4 sits in the last 4 bytes of the 16-byte address field.
 */
public final class Locator {
    public static final int KIND_UDPV4 = 1;
    public static final int KIND_UDPV6 = 2;

    public final int kind;
    public final long port;
    public final byte[] address; // 16 bytes

    public Locator(int kind, long port, byte[] address16) {
        this.kind = kind;
        this.port = port;
        this.address = address16.clone();
    }

    public static Locator udpV4(byte[] ipv4, int port) {
        byte[] addr = new byte[16];
        System.arraycopy(ipv4, 0, addr, 12, 4);
        return new Locator(KIND_UDPV4, port & 0xFFFFFFFFL, addr);
    }

    public static Locator udpV4(String host, int port) {
        try {
            byte[] ip = InetAddress.getByName(host).getAddress();
            if (ip.length != 4) {
                throw new IllegalArgumentException("not IPv4: " + host);
            }
            return udpV4(ip, port);
        } catch (UnknownHostException e) {
            throw new IllegalArgumentException(e);
        }
    }

    public byte[] ipv4() {
        return Arrays.copyOfRange(address, 12, 16);
    }

    public String ipv4String() {
        byte[] ip = ipv4();
        return (ip[0] & 0xFF) + "." + (ip[1] & 0xFF) + "." + (ip[2] & 0xFF) + "." + (ip[3] & 0xFF);
    }

    /** 24-byte little-endian wire form. */
    public void writeLe(Wire w) {
        w.u32le(kind & 0xFFFFFFFFL);
        w.u32le(port);
        w.bytes(address);
    }

    /** Decode 24 bytes little-endian at {@code off}. */
    public static Locator readLe(byte[] b, int off) {
        int k = (int) Wire.u32le(b, off);
        long p = Wire.u32le(b, off + 4);
        byte[] addr = Wire.slice(b, off + 8, 16);
        return new Locator(k, p, addr);
    }

    public boolean isUsableUdpV4() {
        return kind == KIND_UDPV4 && port != 0;
    }

    @Override
    public String toString() {
        return "udp/" + ipv4String() + ":" + port;
    }
}
