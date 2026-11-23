// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

import org.omg.CORBA.Any;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.portable.OutputStream;
import org.omg.IOP.Codec;
import org.omg.IOP.CodecFactory;
import org.omg.IOP.CodecFactoryHelper;
import org.omg.IOP.CodecFactoryPackage.UnknownEncoding;
import org.omg.IOP.CodecPackage.InvalidTypeForEncoding;
import org.omg.IOP.ENCODING_CDR_ENCAPS;
import org.omg.IOP.Encoding;

/**
 * CORBA 3.3 Part 2 §7.8 with JacORB's IOP::Codec: prints the octets of
 * encode and encode_value under ENCODING_CDR_ENCAPS 1.2 for a struct, an
 * anonymous sequence, a union, a wstring and a long long, and what the
 * 1.0 codec and an unknown encoding raise. iop_cross_orb.rs compares
 * them with ZeroDDS's codec.
 */
public class CodecDump {
    static String hex(byte[] bytes) {
        StringBuilder s = new StringBuilder();
        for (byte b : bytes) {
            s.append(String.format("%02x", b & 0xff));
        }
        return s.toString();
    }

    static void dump(Codec codec, String name, Any any) throws Exception {
        System.out.println(name + " encode=" + hex(codec.encode(any)));
        System.out.println(name + " value=" + hex(codec.encode_value(any)));
    }

    public static void main(String[] args) throws Exception {
        ORB orb = ORB.init(args, null);
        CodecFactory factory =
            CodecFactoryHelper.narrow(orb.resolve_initial_references("CodecFactory"));
        Codec codec = factory.create_codec(
            new Encoding(ENCODING_CDR_ENCAPS.value, (byte) 1, (byte) 2));

        Any point = orb.create_any();
        PointHelper.insert(point, new Point(7, "p"));
        dump(codec, "point", point);

        TypeCode shortsTc =
            orb.create_sequence_tc(0, orb.get_primitive_tc(TCKind.tk_short));
        Any shorts = orb.create_any();
        OutputStream os = orb.create_output_stream();
        os.write_ulong(2);
        os.write_short((short) -1);
        os.write_short((short) 2);
        shorts.read_value(os.create_input_stream(), shortsTc);
        dump(codec, "shorts", shorts);

        U u = new U();
        u.s("two");
        Any union = orb.create_any();
        UHelper.insert(union, u);
        dump(codec, "union", union);

        Any wide = orb.create_any();
        wide.insert_wstring("ä€");
        dump(codec, "wstring", wide);

        Any longlong = orb.create_any();
        longlong.insert_longlong(0x0102030405060708L);
        dump(codec, "longlong", longlong);

        Codec codec10 = factory.create_codec(
            new Encoding(ENCODING_CDR_ENCAPS.value, (byte) 1, (byte) 0));
        try {
            codec10.encode(wide);
            System.out.println("wstring_1_0=encoded");
        } catch (InvalidTypeForEncoding e) {
            System.out.println("wstring_1_0=InvalidTypeForEncoding");
        }
        try {
            factory.create_codec(new Encoding((short) 1, (byte) 1, (byte) 2));
            System.out.println("format_1=created");
        } catch (UnknownEncoding e) {
            System.out.println("format_1=UnknownEncoding");
        }
        System.out.println("DONE");
        orb.shutdown(true);
    }
}
