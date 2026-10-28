// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.type.builtin.KeyedBytes;
import org.omg.dds.type.builtin.KeyedString;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

import java.util.Arrays;

/**
 * The {@link TopicTypeSupport}s of the built-in types {@code DDS::String},
 * {@code DDS::KeyedString} and {@code DDS::KeyedBytes} (DDS-Java-PSM 1.0
 * §7.8.2; {@code DDS::Bytes} is {@link ByteArrayTypeSupport}).
 *
 * <p>XTypes 1.3 Annex E defines all four as
 * {@code @extensibility(APPENDABLE)} structures:
 *
 * <pre>
 * struct _String     { string value; };            // type name DDS::String
 * struct KeyedString { @key string key; string value; };
 * struct KeyedBytes  { @key string key; ByteSeq value; };
 * </pre>
 *
 * <p>On the wire (XCDR2 DELIMITED_CDR2, XTypes 1.3 §7.4.3.4.2) each is a
 * 4-byte DHEADER with the size of the members that follow, then the members:
 * a string is a 4-byte length (octets plus the terminating NUL), the octets
 * and the NUL; a {@code ByteSeq} is a 4-byte length and the octets. A
 * {@code string} member after another starts at the next 4-byte boundary.
 *
 * <p>The key hash (XTypes 1.3 §7.6.8, DDSI-RTPS 2.5 §9.6.3.8) is the MD5 of
 * the big-endian serialization of the key member: a {@code string} is
 * unbounded, so its maximum serialized size exceeds 16 octets and the
 * zero-padded form never applies.
 */
public final class BuiltinTypeSupports {

    private BuiltinTypeSupports() {}

    /** Type name of {@code DDS::String}. */
    public static final String STRING_TYPE_NAME = "DDS::String";
    /** Type name of {@code DDS::KeyedString}. */
    public static final String KEYED_STRING_TYPE_NAME = "DDS::KeyedString";
    /** Type name of {@code DDS::KeyedBytes}. */
    public static final String KEYED_BYTES_TYPE_NAME = "DDS::KeyedBytes";

    private static Endianness order(EndianMode endian) {
        java.util.Objects.requireNonNull(endian, "endian");
        return endian == EndianMode.BIG_ENDIAN ? Endianness.BIG : Endianness.LITTLE;
    }

    private static Writer writer(EndianMode endian) {
        return new Writer(order(endian), 4, PlCdr1Framing.defaults(), false);
    }

    private static Reader reader(byte[] bytes, int offset, int length) {
        if (bytes == null || offset < 0 || length < 0 || offset + length > bytes.length) {
            throw new XcdrException("no data in the range given");
        }
        return new Reader(Arrays.copyOfRange(bytes, offset, offset + length),
                Endianness.LITTLE, 4, PlCdr1Framing.defaults());
    }

    /** Leaves the delimited body: a sender may have appended members this type does not know. */
    private static void finish(Reader r, int end) {
        if (r.position() > end) {
            throw new XcdrException("the members read exceed the DHEADER by "
                    + (r.position() - end) + " octet(s)");
        }
        while (r.position() < end) {
            r.skip(1);
        }
    }

    private static byte[] keyHash(String key) {
        Writer holder = Writer.bigEndian();
        NarrowStrings.write(holder, key, 0);
        return holder.md5();
    }

    /** {@code DDS::String}: the topic type {@code java.lang.String}. */
    public static final class StringTypeSupport implements TopicTypeSupport<String> {

        /** Shared instance; the type is stateless. */
        public static final StringTypeSupport INSTANCE = new StringTypeSupport();

        private StringTypeSupport() {}

        @Override
        public String getTypeName() {
            return STRING_TYPE_NAME;
        }

        @Override
        public boolean isKeyed() {
            return false;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(String sample) {
            return encode(sample, EndianMode.LITTLE_ENDIAN);
        }

        @Override
        public byte[] encode(String sample, EndianMode endian) {
            if (sample == null) {
                throw new XcdrException("a DDS::String sample must not be null");
            }
            Writer w = writer(endian);
            Writer.DHeader dh = w.writeDheader();
            NarrowStrings.write(w, sample, 0);
            w.finishDheader(dh);
            return w.finish();
        }

        @Override
        public String decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public String decode(byte[] bytes, int offset, int length) {
            try {
                Reader r = reader(bytes, offset, length);
                int size = r.readDheader();
                int end = r.position() + size;
                String value = NarrowStrings.read(r, 0);
                finish(r, end);
                return value;
            } catch (org.zerodds.wire.WireException malformed) {
                throw new XcdrException("malformed sample: " + malformed.getMessage(), malformed);
            }
        }

        @Override
        public byte[] keyHash(String sample) {
            return new byte[16];
        }
    }

    /** {@code DDS::KeyedString}: the topic type {@link KeyedString}. */
    public static final class KeyedStringTypeSupport implements TopicTypeSupport<KeyedString> {

        /** Shared instance; the type is stateless. */
        public static final KeyedStringTypeSupport INSTANCE = new KeyedStringTypeSupport();

        private KeyedStringTypeSupport() {}

        @Override
        public String getTypeName() {
            return KEYED_STRING_TYPE_NAME;
        }

        @Override
        public boolean isKeyed() {
            return true;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(KeyedString sample) {
            return encode(sample, EndianMode.LITTLE_ENDIAN);
        }

        @Override
        public byte[] encode(KeyedString sample, EndianMode endian) {
            if (sample == null) {
                throw new XcdrException("a DDS::KeyedString sample must not be null");
            }
            Writer w = writer(endian);
            Writer.DHeader dh = w.writeDheader();
            NarrowStrings.write(w, sample.getKey(), 0);
            NarrowStrings.write(w, sample.getValue(), 0);
            w.finishDheader(dh);
            return w.finish();
        }

        @Override
        public KeyedString decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public KeyedString decode(byte[] bytes, int offset, int length) {
            try {
                Reader r = reader(bytes, offset, length);
                int size = r.readDheader();
                int end = r.position() + size;
                KeyedString out = ServiceEnvironmentImpl.getDefaultEnvironment()
                        .getSPI().newKeyedString();
                out.setKey(NarrowStrings.read(r, 0));
                out.setValue(r.position() < end ? NarrowStrings.read(r, 0) : "");
                finish(r, end);
                return out;
            } catch (org.zerodds.wire.WireException malformed) {
                throw new XcdrException("malformed sample: " + malformed.getMessage(), malformed);
            }
        }

        @Override
        public void copyKey(KeyedString from, KeyedString into) {
            into.setKey(from.getKey()); // the key is the whole of the key; the value stays
        }

        @Override
        public KeyedString keyValue(KeyedString from, KeyedString holder) {
            copyKey(from, holder);
            return holder;
        }

        @Override
        public byte[] keyHash(KeyedString sample) {
            return sample == null ? new byte[16] : BuiltinTypeSupports.keyHash(sample.getKey());
        }
    }

    /** {@code DDS::KeyedBytes}: the topic type {@link KeyedBytes}. */
    public static final class KeyedBytesTypeSupport implements TopicTypeSupport<KeyedBytes> {

        /** Shared instance; the type is stateless. */
        public static final KeyedBytesTypeSupport INSTANCE = new KeyedBytesTypeSupport();

        private KeyedBytesTypeSupport() {}

        @Override
        public String getTypeName() {
            return KEYED_BYTES_TYPE_NAME;
        }

        @Override
        public boolean isKeyed() {
            return true;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(KeyedBytes sample) {
            return encode(sample, EndianMode.LITTLE_ENDIAN);
        }

        @Override
        public byte[] encode(KeyedBytes sample, EndianMode endian) {
            if (sample == null) {
                throw new XcdrException("a DDS::KeyedBytes sample must not be null");
            }
            Writer w = writer(endian);
            Writer.DHeader dh = w.writeDheader();
            NarrowStrings.write(w, sample.getKey(), 0);
            byte[] value = sample.getValue();
            w.writeU32(value.length);
            for (byte b : value) {
                w.writeU8(Byte.toUnsignedInt(b));
            }
            w.finishDheader(dh);
            return w.finish();
        }

        @Override
        public KeyedBytes decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public KeyedBytes decode(byte[] bytes, int offset, int length) {
            try {
                Reader r = reader(bytes, offset, length);
                int size = r.readDheader();
                int end = r.position() + size;
                KeyedBytes out = ServiceEnvironmentImpl.getDefaultEnvironment()
                        .getSPI().newKeyedBytes();
                out.setKey(NarrowStrings.read(r, 0));
                byte[] value = new byte[0];
                if (r.position() < end) {
                    int n = r.readSequenceLength();
                    value = new byte[n];
                    for (int i = 0; i < n; i++) {
                        value[i] = (byte) r.readU8();
                    }
                }
                out.setValue(value);
                finish(r, end);
                return out;
            } catch (org.zerodds.wire.WireException malformed) {
                throw new XcdrException("malformed sample: " + malformed.getMessage(), malformed);
            }
        }

        @Override
        public void copyKey(KeyedBytes from, KeyedBytes into) {
            into.setKey(from.getKey());
        }

        @Override
        public KeyedBytes keyValue(KeyedBytes from, KeyedBytes holder) {
            copyKey(from, holder);
            return holder;
        }

        @Override
        public byte[] keyHash(KeyedBytes sample) {
            return sample == null ? new byte[16] : BuiltinTypeSupports.keyHash(sample.getKey());
        }
    }
}
