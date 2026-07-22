// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ZdwReflect -- the wire-variable unit for the Java endpoint SDK (ADR 0013): a
// reflective XCDR codec driven by a runtime kind[] + value[] instead of
// generated code. Same bytes as the fixed path. Java 8, no JNI.

public final class ZdwReflect {
    private ZdwReflect() {}

    /** kinds[i] in {u8,u16,u32,u64,f32,f64,bool,string,seq_u8}; values parallel. */
    public static void encode(Zdw.Writer w, String[] kinds, Object[] values) {
        for (int i = 0; i < kinds.length; i++) {
            Object v = values[i];
            if (kinds[i].equals("u8")) w.u8(((Number) v).intValue());
            else if (kinds[i].equals("u16")) w.u16(((Number) v).intValue());
            else if (kinds[i].equals("u32")) w.u32(((Number) v).longValue());
            else if (kinds[i].equals("u64")) w.u64(((Number) v).longValue());
            else if (kinds[i].equals("f32")) w.f32(((Number) v).floatValue());
            else if (kinds[i].equals("f64")) w.f64(((Number) v).doubleValue());
            else if (kinds[i].equals("string")) w.str((String) v);
            else if (kinds[i].equals("seq_u8")) w.seqU8((byte[]) v);
            else throw new IllegalArgumentException("kind " + kinds[i]);
        }
    }

    // --- extensibility + nested (recursive struct model) ---

    /** A field: kind + value (value is a Struct for nested, Struct[] for seq_struct). */
    public static final class Field {
        public final String kind; public final Object value;
        public Field(String kind, Object value) { this.kind = kind; this.value = value; }
    }
    /** A struct descriptor: ext ("final"/"appendable"/"mutable") + fields (+ ids for mutable). */
    public static final class Struct {
        public final String ext; public final Field[] fields; public final long[] ids;
        public Struct(String ext, Field[] fields, long[] ids) { this.ext = ext; this.fields = fields; this.ids = ids; }
    }

    private static void encodeScalar(Zdw.Writer w, Field f) {
        Object v = f.value;
        if (f.kind.equals("u8")) w.u8(((Number) v).intValue());
        else if (f.kind.equals("u16")) w.u16(((Number) v).intValue());
        else if (f.kind.equals("u32")) w.u32(((Number) v).longValue());
        else if (f.kind.equals("u64")) w.u64(((Number) v).longValue());
        else if (f.kind.equals("f32")) w.f32(((Number) v).floatValue());
        else if (f.kind.equals("f64")) w.f64(((Number) v).doubleValue());
        else if (f.kind.equals("string")) w.str((String) v);
        else if (f.kind.equals("seq_u8")) w.seqU8((byte[]) v);
        else throw new IllegalArgumentException("kind " + f.kind);
    }

    private static void encodeField(Zdw.Writer w, final Field f) {
        if (f.kind.equals("nested")) {
            encodeStruct(w, (Struct) f.value);
        } else if (f.kind.equals("seq_struct")) {
            final Struct[] elems = (Struct[]) f.value;
            w.dheader(new Zdw.Body() { public void write(Zdw.Writer w) {
                w.u32(elems.length);
                for (Struct e : elems) encodeStruct(w, e);
            } });
        } else {
            encodeScalar(w, f);
        }
    }

    public static void encodeStruct(Zdw.Writer w, final Struct s) {
        if (s.ext.equals("final")) {
            for (Field f : s.fields) encodeField(w, f);
        } else if (s.ext.equals("appendable")) {
            w.dheader(new Zdw.Body() { public void write(Zdw.Writer w) {
                for (Field f : s.fields) encodeField(w, f);
            } });
        } else if (s.ext.equals("mutable")) {
            w.dheader(new Zdw.Body() { public void write(final Zdw.Writer w) {
                for (int i = 0; i < s.fields.length; i++) {
                    final Field f = s.fields[i];
                    w.emheader((int) s.ids[i], false, new Zdw.Body() {
                        public void write(Zdw.Writer w) { encodeField(w, f); } });
                }
            } });
        } else {
            throw new IllegalArgumentException("ext " + s.ext);
        }
    }

    public static Object[] decode(Zdw.Reader r, String[] kinds) {
        Object[] out = new Object[kinds.length];
        for (int i = 0; i < kinds.length; i++) {
            if (kinds[i].equals("u8")) out[i] = (long) r.u8();
            else if (kinds[i].equals("u16")) out[i] = (long) r.u16();
            else if (kinds[i].equals("u32")) out[i] = r.u32();
            else if (kinds[i].equals("u64")) out[i] = r.u64();
            else if (kinds[i].equals("f32")) out[i] = r.f32();
            else if (kinds[i].equals("f64")) out[i] = r.f64();
            else if (kinds[i].equals("string")) out[i] = r.str();
            else if (kinds[i].equals("seq_u8")) out[i] = r.seqU8();
            else throw new IllegalArgumentException("kind " + kinds[i]);
        }
        return out;
    }
}
