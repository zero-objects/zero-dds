package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;

/**
 * {@code GroupDigest_t} (DDSI-RTPS 2.5 9.3.2.5): the leading four octets of the MD5 digest of the
 * CDR big-endian serialization of the {@code sequence<EntityId_t>} of a set of entities, sorted by
 * increasing value of the EntityId_t read as a little-endian signed 32-bit integer. The empty
 * group is four zero octets, not a hash.
 */
public final class GroupDigest {
    private GroupDigest() {
    }

    /** The digest of a set of entity ids (four octets each); order and duplicates do not matter. */
    public static byte[] of(Collection<byte[]> entityIds) {
        List<byte[]> sorted = new ArrayList<byte[]>();
        for (byte[] id : entityIds) {
            boolean duplicate = false;
            for (byte[] known : sorted) {
                duplicate |= java.util.Arrays.equals(known, id);
            }
            if (!duplicate) {
                sorted.add(id);
            }
        }
        if (sorted.isEmpty()) {
            return new byte[4];
        }
        Collections.sort(sorted, new Comparator<byte[]>() {
            @Override
            public int compare(byte[] a, byte[] b) {
                return Integer.compare(asInt(a), asInt(b));
            }
        });
        byte[] cdr = new byte[4 + 4 * sorted.size()];
        int n = sorted.size();
        cdr[0] = (byte) (n >>> 24);
        cdr[1] = (byte) (n >>> 16);
        cdr[2] = (byte) (n >>> 8);
        cdr[3] = (byte) n;
        for (int i = 0; i < n; i++) {
            System.arraycopy(sorted.get(i), 0, cdr, 4 + 4 * i, 4);
        }
        try {
            byte[] md5 = java.security.MessageDigest.getInstance("MD5").digest(cdr);
            return java.util.Arrays.copyOf(md5, 4);
        } catch (java.security.NoSuchAlgorithmException e) {
            throw new IllegalStateException("MD5 is required of every Java platform", e);
        }
    }

    private static int asInt(byte[] id) {
        return (id[0] & 0xFF) | (id[1] & 0xFF) << 8 | (id[2] & 0xFF) << 16 | (id[3] & 0xFF) << 24;
    }
}
