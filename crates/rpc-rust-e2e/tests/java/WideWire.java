// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The samples crates/rpc-rust-e2e/tests/wire_bytes_match_columns.rs holds
// against the Rust and the C++ column: header, padding and the union, for
// operations with members of eight octets, under classic CDR (XCDR1) and
// XCDR2, behind instance names of several lengths. One line per sample:
// "<kind> <encoding> <name length> <operation> <hex>".
//
// Also nested @appendable and @mutable members, in the topic types of an
// interface (Store) and in a pair of types (PReq, PRep): classic CDR has no
// DHEADER for the former and a parameter list for the latter.

import org.omg.dds.rpc.HeaderCodec;
import org.omg.dds.rpc.PlaceholderTypes;
import org.omg.dds.rpc.RemoteExceptionCode;
import org.omg.dds.rpc.ReplyHeader;
import org.omg.dds.rpc.RequestHeader;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.rpc.WireEncoding;

public final class WideWire {
    private WideWire() {}

    static SampleIdentity identity() {
        byte[] guid = new byte[16];
        java.util.Arrays.fill(guid, (byte) 0x42);
        return new SampleIdentity(guid, 0x0102030405060708L);
    }

    static void line(String kind, WireEncoding encoding, int length, String operation,
            byte[] header, byte[] union) {
        StringBuilder text = new StringBuilder();
        text.append(kind).append(' ').append(encoding == WireEncoding.XCDR1 ? "xcdr1" : "xcdr2")
                .append(' ').append(length).append(' ').append(operation).append(' ');
        for (byte octet : header) {
            text.append(String.format("%02x", octet & 0xff));
        }
        for (byte octet : union) {
            text.append(String.format("%02x", octet & 0xff));
        }
        System.out.println(text);
    }

    static byte[] padded(byte[] header) {
        int size = (header.length + 3) & ~3;
        return java.util.Arrays.copyOf(header, size);
    }

    public static void main(String[] args) {
        int[] lengths = {0, 1, 3, 4, 5};
        for (WireEncoding encoding : new WireEncoding[] {WireEncoding.XCDR1, WireEncoding.XCDR2}) {
            for (int length : lengths) {
                String name = "n".repeat(length);
                byte[] header = padded(
                        HeaderCodec.encodeRequestHeader(new RequestHeader(identity(), name)));

                ZdServices.wide.W_Request scale = new ZdServices.wide.W_Request();
                scale.data = new ZdServices.wide.W_Call();
                scale.data.scale = new ZdServices.wide.W_scale_In();
                scale.data._d = ZdServices.wide.W_scale_Hash;
                scale.data.scale.d = 1.5;
                scale.data.scale.n = -2L;
                scale.data.scale.u = 0x1122334455667788L;
                line("request", encoding, length, "scale", header,
                        ZdServices.wide.WCodec.REQUEST.serialize(scale, header.length, encoding));

                ZdServices.wide.W_Request mix = new ZdServices.wide.W_Request();
                mix.data = new ZdServices.wide.W_Call();
                mix.data.mix = new ZdServices.wide.W_mix_In();
                mix.data._d = ZdServices.wide.W_mix_Hash;
                mix.data.mix.o = (byte) 0x7f;
                mix.data.mix.u = 0x0102030405060708L;
                line("request", encoding, length, "mix", header,
                        ZdServices.wide.WCodec.REQUEST.serialize(mix, header.length, encoding));
            }

            for (int length : new int[] {0, 3}) {
                String name = "n".repeat(length);
                byte[] header = padded(
                        HeaderCodec.encodeRequestHeader(new RequestHeader(identity(), name)));

                ZdServices.nested.Store_Request put = new ZdServices.nested.Store_Request();
                put.data = new ZdServices.nested.Store_Call();
                put.data.put = new ZdServices.nested.Store_put_In();
                put.data._d = ZdServices.nested.Store_put_Hash;
                put.data.put.a = new ZdTypes.nested.Appended();
                put.data.put.a.a = 1;
                put.data.put.a.b = 2.0;
                put.data.put.m = new ZdTypes.nested.Mutated();
                put.data.put.m.m = 3;
                put.data.put.m.n = 4;
                line("request", encoding, length, "put", header,
                        ZdServices.nested.StoreCodec.REQUEST.serialize(put, header.length, encoding));

                ZdTypes.nested.PReq pair = new ZdTypes.nested.PReq();
                pair.header = new RequestHeader(identity(), name);
                pair.ap = new ZdTypes.nested.Appended();
                pair.ap.a = 1;
                pair.ap.b = 2.0;
                pair.mu = new ZdTypes.nested.Mutated();
                pair.mu.m = 3;
                pair.mu.n = 4;
                line("request", encoding, length, "pair", new byte[0],
                        new ZdServices.nested.PReqSupport.RequestTypeSupport()
                                .serialize(pair, 0, encoding));
            }

            byte[] header = HeaderCodec.encodeReplyHeader(
                    new ReplyHeader(identity(), RemoteExceptionCode.REMOTE_EX_OK));

            ZdServices.nested.Store_Reply get = new ZdServices.nested.Store_Reply();
            get.data = new ZdServices.nested.Store_Return();
            get.data.get = new ZdServices.nested.Store_get_Result();
            get.data.get.result = new ZdServices.nested.Store_get_Out();
            get.data._d = ZdServices.nested.Store_get_Hash;
            get.data.get._d = PlaceholderTypes.RETCODE_OK;
            get.data.get.result.return_ = new ZdTypes.nested.Mutated();
            get.data.get.result.return_.m = 3;
            get.data.get.result.return_.n = 4;
            line("reply", encoding, 0, "get", header,
                    ZdServices.nested.StoreCodec.REPLY.serialize(get, header.length, encoding));

            ZdTypes.nested.PRep pairReply = new ZdTypes.nested.PRep();
            pairReply.header = new ReplyHeader(identity(), RemoteExceptionCode.REMOTE_EX_OK);
            pairReply.ap = new ZdTypes.nested.Appended();
            pairReply.ap.a = 1;
            pairReply.ap.b = 2.0;
            pairReply.mu = new ZdTypes.nested.Mutated();
            pairReply.mu.m = 3;
            pairReply.mu.n = 4;
            line("reply", encoding, 0, "pair", new byte[0],
                    new ZdServices.nested.PReqSupport.ReplyTypeSupport()
                            .serialize(pairReply, 0, encoding));

            ZdServices.wide.W_Reply scale = new ZdServices.wide.W_Reply();
            scale.data = new ZdServices.wide.W_Return();
            scale.data.scale = new ZdServices.wide.W_scale_Result();
            scale.data.scale.result = new ZdServices.wide.W_scale_Out();
            scale.data._d = ZdServices.wide.W_scale_Hash;
            scale.data.scale._d = PlaceholderTypes.RETCODE_OK;
            scale.data.scale.result.return_ = 3.0;
            line("reply", encoding, 0, "scale", header,
                    ZdServices.wide.WCodec.REPLY.serialize(scale, header.length, encoding));

            ZdServices.wide.W_Reply mix = new ZdServices.wide.W_Reply();
            mix.data = new ZdServices.wide.W_Return();
            mix.data.mix = new ZdServices.wide.W_mix_Result();
            mix.data.mix.result = new ZdServices.wide.W_mix_Out();
            mix.data._d = ZdServices.wide.W_mix_Hash;
            mix.data.mix._d = PlaceholderTypes.RETCODE_OK;
            mix.data.mix.result.return_ = 0xAABBCCDDEEFF0011L;
            line("reply", encoding, 0, "mix", header,
                    ZdServices.wide.WCodec.REPLY.serialize(mix, header.length, encoding));
        }
    }
}
