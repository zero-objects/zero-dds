// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;

/**
 * The permanent storage behind DURABILITY kind PERSISTENT (DDS 1.4 §2.2.3.4): an append-only
 * journal per (domain, topic, type) in the directory named by the system property
 * {@value #DIRECTORY_PROPERTY}. {@link DurabilityService} writes a record for each change of a
 * PERSISTENT instance and replays the journal when it first touches the topic, so that the samples
 * outlive the process. Without the property PERSISTENT is kept in memory like TRANSIENT, and
 * survives the DomainParticipant but not the process.
 *
 * <p>A journal starts with the magic {@code ZDJ1} and the channel key. A record is a tag octet
 * followed by its fields; a record cut off at the end of the file (a write that did not finish) is
 * ignored, any other malformed content is an {@link IOException}.
 */
final class DurabilityJournal {

    /** The system property that names the directory of the journals; unset means memory only. */
    static final String DIRECTORY_PROPERTY = "zerodds.durability.dir";

    private static final int MAGIC = 0x5A444A31; // "ZDJ1"
    private static final int RETAINED = 1;
    private static final int DISPOSED = 2;
    private static final int PURGED = 3;

    private DurabilityJournal() {}

    /** What replaying a journal reports. */
    interface Sink {
        void retained(byte[] key, byte[] payload, long timestampMillis, long lifespanMillis,
                      int cap, long cleanupDelayMillis);

        void disposed(byte[] key, long cleanupDelayMillis);

        void purged(byte[] key);
    }

    /** The directory of the journals, or {@code null} when PERSISTENT is kept in memory. */
    static String directory() {
        String dir = System.getProperty(DIRECTORY_PROPERTY);
        return dir == null || dir.isEmpty() ? null : dir;
    }

    static void appendRetained(String dir, String channel, byte[] key, byte[] payload,
                               long timestampMillis, long lifespanMillis, int cap,
                               long cleanupDelayMillis) throws IOException {
        try (DataOutputStream out = open(dir, channel)) {
            out.writeByte(RETAINED);
            writeBytes(out, key);
            writeBytes(out, payload);
            out.writeLong(timestampMillis);
            out.writeLong(lifespanMillis);
            out.writeInt(cap);
            out.writeLong(cleanupDelayMillis);
        }
    }

    static void appendDisposed(String dir, String channel, byte[] key, long cleanupDelayMillis)
            throws IOException {
        try (DataOutputStream out = open(dir, channel)) {
            out.writeByte(DISPOSED);
            writeBytes(out, key);
            out.writeLong(cleanupDelayMillis);
        }
    }

    static void appendPurged(String dir, String channel, byte[] key) throws IOException {
        try (DataOutputStream out = open(dir, channel)) {
            out.writeByte(PURGED);
            writeBytes(out, key);
        }
    }

    /**
     * Replays the journal of {@code channel} into {@code sink}.
     *
     * @return whether a journal exists.
     */
    static boolean load(String dir, String channel, Sink sink) throws IOException {
        Path file = fileOf(dir, channel);
        if (!Files.isRegularFile(file)) {
            return false;
        }
        try (InputStream raw = Files.newInputStream(file);
             DataInputStream in = new DataInputStream(new java.io.BufferedInputStream(raw))) {
            if (in.readInt() != MAGIC || !channel.equals(in.readUTF())) {
                throw new IOException("not the journal of " + channel + ": " + file);
            }
            while (true) {
                int tag;
                try {
                    tag = in.readUnsignedByte();
                } catch (EOFException end) {
                    return true;
                }
                try {
                    switch (tag) {
                        case RETAINED: {
                            byte[] key = readBytes(in);
                            byte[] payload = readBytes(in);
                            long timestamp = in.readLong();
                            long lifespan = in.readLong();
                            int cap = in.readInt();
                            long delay = in.readLong();
                            sink.retained(key, payload, timestamp, lifespan, cap, delay);
                            break;
                        }
                        case DISPOSED: {
                            byte[] key = readBytes(in);
                            sink.disposed(key, in.readLong());
                            break;
                        }
                        case PURGED:
                            sink.purged(readBytes(in));
                            break;
                        default:
                            throw new IOException("unknown record " + tag + " in " + file);
                    }
                } catch (EOFException cut) {
                    return true; // a record that was being written when the process ended
                }
            }
        }
    }

    private static DataOutputStream open(String dir, String channel) throws IOException {
        Path file = fileOf(dir, channel);
        Files.createDirectories(file.getParent());
        boolean fresh = !Files.exists(file);
        OutputStream raw = Files.newOutputStream(file, StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        DataOutputStream out = new DataOutputStream(new java.io.BufferedOutputStream(raw));
        if (fresh) {
            out.writeInt(MAGIC);
            out.writeUTF(channel);
        }
        return out;
    }

    /** The file of a channel: its key made safe for a file name, and a hash against collisions. */
    static Path fileOf(String dir, String channel) {
        StringBuilder name = new StringBuilder();
        for (char c : channel.toCharArray()) {
            boolean safe = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')
                    || c == '.' || c == '-';
            name.append(safe ? c : '_');
        }
        byte[] bytes = channel.getBytes(StandardCharsets.UTF_8);
        long hash = 1125899906842597L;
        for (byte b : bytes) {
            hash = 31 * hash + b;
        }
        return Paths.get(dir, name + "-" + Long.toHexString(hash) + ".journal");
    }

    private static void writeBytes(DataOutputStream out, byte[] bytes) throws IOException {
        out.writeInt(bytes.length);
        out.write(bytes);
    }

    private static byte[] readBytes(DataInputStream in) throws IOException {
        int length = in.readInt();
        if (length < 0 || length > (1 << 28)) {
            throw new IOException("length " + length + " is no length of a sample");
        }
        byte[] bytes = new byte[length];
        in.readFully(bytes);
        return bytes;
    }
}
