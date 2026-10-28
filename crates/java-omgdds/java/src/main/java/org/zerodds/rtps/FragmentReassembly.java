// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.Arrays;
import java.util.BitSet;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;

/** Shared bounded DATA_FRAG assembly, independent of topic/service payload. */
final class FragmentReassembly {
    private static final long MAX_BYTES = 16L * 1024 * 1024;
    private static final class Assembly {
        final byte[] bytes;
        final int fragmentSize, count;
        final BitSet received = new BitSet();
        final long created = System.nanoTime();
        byte[] readerId;
        final byte[] destination;
        final boolean keyFlag;
        ParameterList inlineQos;
        Assembly(int size, int fragmentSize, RtpsMessage.DataFragment fragment) {
            this.bytes = new byte[size]; this.fragmentSize = fragmentSize;
            this.count = (size + fragmentSize - 1) / fragmentSize;
            this.readerId = fragment.readerId.clone(); this.keyFlag = fragment.keyFlag;
            this.destination = fragment.destPrefix == null ? null : fragment.destPrefix.clone();
        }
    }
    private final Map<String, Assembly> pending = new LinkedHashMap<>();
    private long allocated;

    synchronized long missing(byte[] prefix, byte[] writer, long sequence) {
        Assembly assembly = pending.get(org.zerodds.cdr.HexBytes.encode(new Guid(prefix, writer).bytes()) + ":" + sequence);
        return assembly == null ? 1 : assembly.received.nextClearBit(0) + 1L;
    }

    synchronized void expire() {
        long now = System.nanoTime();
        Iterator<Assembly> iterator = pending.values().iterator();
        while (iterator.hasNext()) {
            Assembly old = iterator.next();
            if (now - old.created > java.util.concurrent.TimeUnit.SECONDS.toNanos(30)) {
                allocated -= old.bytes.length; iterator.remove();
            }
        }
    }

    synchronized RtpsMessage.Data accept(byte[] prefix, RtpsMessage.DataFragment fragment) {
        long size = fragment.sampleSize;
        if (size < 4 || size > MAX_BYTES || fragment.fragmentSize < 1
                || fragment.fragmentsInSubmessage < 1 || fragment.fragmentStartingNum < 1)
            throw new IllegalArgumentException("Invalid or oversized DATA_FRAG");
        long start = (fragment.fragmentStartingNum - 1L) * fragment.fragmentSize;
        long length = Math.min(size - start, (long) fragment.fragmentsInSubmessage * fragment.fragmentSize);
        if (start < 0 || start >= size || length < 1 || length != fragment.serializedPayload.length)
            throw new IllegalArgumentException("DATA_FRAG extent differs from sample size");
        long now = System.nanoTime();
        Iterator<Assembly> iterator = pending.values().iterator();
        while (iterator.hasNext()) {
            Assembly old = iterator.next();
            if (now - old.created > java.util.concurrent.TimeUnit.SECONDS.toNanos(30)) {
                allocated -= old.bytes.length; iterator.remove();
            }
        }
        String key = org.zerodds.cdr.HexBytes.encode(new Guid(prefix, fragment.writerId).bytes()) + ":" + fragment.writerSn;
        Assembly assembly = pending.get(key);
        if (assembly == null) {
            if (pending.size() >= 64 || allocated + size > MAX_BYTES)
                throw new IllegalStateException("DATA_FRAG assembly memory limit");
            assembly = new Assembly((int) size, fragment.fragmentSize, fragment);
            allocated += size;
            pending.put(key, assembly);
        } else if (assembly.bytes.length != size || assembly.fragmentSize != fragment.fragmentSize
                || assembly.keyFlag != fragment.keyFlag
                || !Arrays.equals(assembly.readerId, fragment.readerId)
                    && !Arrays.equals(assembly.readerId, Guid.ENTITYID_UNKNOWN)
                    && !Arrays.equals(fragment.readerId, Guid.ENTITYID_UNKNOWN)) {
            throw new IllegalArgumentException("Conflicting DATA_FRAG metadata");
        }
        if (Arrays.equals(fragment.readerId, Guid.ENTITYID_UNKNOWN)) assembly.readerId = Guid.ENTITYID_UNKNOWN.clone();
        if (fragment.fragmentStartingNum == 1) assembly.inlineQos = fragment.inlineQos;
        int first = (int) fragment.fragmentStartingNum - 1;
        for (int index = 0; index < fragment.fragmentsInSubmessage; index++) {
            int number = first + index;
            if (number >= assembly.count) throw new IllegalArgumentException("DATA_FRAG extends past final fragment");
            int offset = number * assembly.fragmentSize;
            int chunk = Math.min(assembly.fragmentSize, assembly.bytes.length - offset);
            int source = index * assembly.fragmentSize;
            if (assembly.received.get(number)) {
                for (int j = 0; j < chunk; j++) if (assembly.bytes[offset + j] != fragment.serializedPayload[source + j])
                    throw new IllegalArgumentException("Conflicting DATA_FRAG duplicate bytes");
            } else {
                System.arraycopy(fragment.serializedPayload, source, assembly.bytes, offset, chunk);
                assembly.received.set(number);
            }
        }
        if (assembly.received.cardinality() != assembly.count) return null;
        pending.remove(key);
        allocated -= assembly.bytes.length;
        RtpsMessage.Data data = new RtpsMessage.Data();
        data.destPrefix = assembly.destination;
        data.readerId = assembly.readerId;
        data.writerId = fragment.writerId;
        data.writerSn = fragment.writerSn;
        data.inlineQos = assembly.inlineQos;
        data.keyFlag = assembly.keyFlag;
        data.serializedPayload = assembly.bytes;
        data.sourceTimestampMillis = fragment.sourceTimestampMillis;
        return data;
    }
}
