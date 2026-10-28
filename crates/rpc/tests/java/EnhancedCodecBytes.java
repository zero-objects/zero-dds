// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The octets of the Enhanced Service Mapping (DDS-RPC 1.0 §7.5.1.2) that the
// generated `CalcCodec` writes and reads, against the octets of the clause
// computed by hand: PL_CDR2, a DHEADER, one EMHEADER per choice structure
// (length code 4, NEXTINT), `remoteEx` as a 32-bit member (length code 2).
// The member ids are HASH(name) & 0x0FFFFFFF and arrive as arguments.
//
// No DDS is involved: the codec is measured on its own.

import java.io.ByteArrayOutputStream;
import java.util.Arrays;

import org.omg.dds.rpc.EnhancedReplySupport;
import org.omg.dds.rpc.HeaderCodec;
import org.omg.dds.rpc.RemoteExceptionCode;
import org.omg.dds.rpc.WireEncoding;

public final class EnhancedCodecBytes {

    private static int failures = 0;

    static void check(boolean held, String what) {
        if (!held) {
            System.err.println("FAIL " + what);
            ++failures;
        }
    }

    /** A little-endian 32-bit word. */
    static void put32(ByteArrayOutputStream out, long value) {
        for (int shift = 0; shift < 32; shift += 8) {
            out.write((int) ((value >>> shift) & 0xFF));
        }
    }

    public static void main(String[] args) {
        long add = Long.parseLong(args[0]);
        long result = Long.parseLong(args[1]);
        long remoteEx = Long.parseLong(args[2]);
        long x = Long.parseLong(args[3]);

        // A request is a PL_CDR2 object with one member: DHEADER, the
        // EMHEADER of the operation with a NEXTINT, and `Calc_add_In`, two
        // `long`s without headers.
        ZdServices.robot.Calc_Request request = new ZdServices.robot.Calc_Request();
        request.add = new ZdServices.robot.Calc_add_In();
        request.add.a = 2;
        request.add.b = 40;
        ByteArrayOutputStream expected = new ByteArrayOutputStream();
        put32(expected, 16);
        put32(expected, (4L << 28) | add);
        put32(expected, 8);
        put32(expected, 2);
        put32(expected, 40);
        byte[] written = ZdServices.robot.CalcCodec.REQUEST.serialize(request);
        check(Arrays.equals(written, expected.toByteArray()), "the request of add(2, 40)");
        ZdServices.robot.Calc_Request read =
                ZdServices.robot.CalcCodec.REQUEST.deserialize(written, new HeaderCodec.Cursor(0));
        check(read.add != null && read.add.a == 2 && read.add.b == 40, "which reads back");
        check(read.ping == null, "and holds no other member");

        // The reply: DHEADER, the EMHEADER of `add`, the NEXTINT, then the
        // Result: DHEADER, the EMHEADER of `result`, the NEXTINT and the Out
        // structure, one `long` (final, no headers).
        ZdServices.robot.Calc_Reply reply = new ZdServices.robot.Calc_Reply();
        reply.add = new ZdServices.robot.Calc_add_Result();
        reply.add.result = new ZdServices.robot.Calc_add_Out();
        reply.add.result.return_ = 42;
        expected = new ByteArrayOutputStream();
        put32(expected, 24);
        put32(expected, (4L << 28) | add);
        put32(expected, 16);
        put32(expected, 12);
        put32(expected, (4L << 28) | result);
        put32(expected, 4);
        put32(expected, 42);
        written = ZdServices.robot.CalcCodec.REPLY.serialize(reply);
        check(Arrays.equals(written, expected.toByteArray()), "the reply of add");
        ZdServices.robot.Calc_Reply decoded =
                ZdServices.robot.CalcCodec.REPLY.deserialize(written, new HeaderCodec.Cursor(0));
        check(decoded.add != null && decoded.add.result != null && decoded.add.result.return_ == 42,
                "which reads back");

        // §7.5.1.2.5 rule 5: `remoteEx` is a 32-bit member, length code 2.
        @SuppressWarnings("unchecked")
        EnhancedReplySupport<ZdServices.robot.Calc_Reply> support =
                (EnhancedReplySupport<ZdServices.robot.Calc_Reply>) ZdServices.robot.CalcCodec.REPLY;
        ZdServices.robot.Calc_Reply failed =
                support.remoteExceptionSample(RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION);
        expected = new ByteArrayOutputStream();
        put32(expected, 8);
        put32(expected, (2L << 28) | remoteEx);
        put32(expected, 4);
        check(Arrays.equals(support.serialize(failed), expected.toByteArray()), "remoteEx");
        check(support.remoteExceptionOf(failed) == RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION,
                "and the code reads back");
        check(support.remoteExceptionOf(reply) == RemoteExceptionCode.REMOTE_EX_OK,
                "a reply with a result carries no code");

        // A mutable structure skips a member it does not know and takes the
        // default of one that is absent (XTypes 1.3 §7.2.4.4.8).
        ByteArrayOutputStream body = new ByteArrayOutputStream();
        put32(body, (4L << 28) | 0x123L);
        put32(body, 3);
        body.write(1);
        body.write(2);
        body.write(3);
        body.write(0);
        put32(body, (2L << 28) | x);
        put32(body, 9);
        ByteArrayOutputStream mutable = new ByteArrayOutputStream();
        put32(mutable, body.size());
        mutable.write(body.toByteArray(), 0, body.size());
        org.zerodds.wire.Reader reader =
                new org.zerodds.wire.Reader(
                        mutable.toByteArray(), org.zerodds.wire.Endianness.LITTLE, 4, null);
        ZdServices.robot.Fast_square_In skipped =
                ZdServices.robot.FastCodec.decode_Fast_square_In(reader);
        check(skipped.x == 9, "a mutable structure skips a member it does not know");
        ByteArrayOutputStream empty = new ByteArrayOutputStream();
        put32(empty, 0);
        org.zerodds.wire.Reader none =
                new org.zerodds.wire.Reader(
                        empty.toByteArray(), org.zerodds.wire.Endianness.LITTLE, 4, null);
        check(ZdServices.robot.FastCodec.decode_Fast_square_In(none).x == 0, "and one that is absent takes its default");

        // §7.7.2: a member the interface does not have leaves every member
        // of the request empty, which the service answers
        // REMOTE_EX_UNKNOWN_OPERATION.
        ByteArrayOutputStream unknown = new ByteArrayOutputStream();
        put32(unknown, 12);
        put32(unknown, (4L << 28) | 0x00ABCDEFL);
        put32(unknown, 4);
        put32(unknown, 0);
        ZdServices.robot.Calc_Request foreign =
                ZdServices.robot.CalcCodec.REQUEST.deserialize(
                        unknown.toByteArray(), new HeaderCodec.Cursor(0), WireEncoding.XCDR2);
        check(foreign.add == null && foreign.ping == null, "an unknown member leaves the request empty");

        if (failures == 0) {
            System.out.println("every check held");
        } else {
            System.exit(1);
        }
    }
}
