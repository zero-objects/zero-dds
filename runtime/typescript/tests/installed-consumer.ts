// Existing primitive-long corpus, through a freshly generated installed consumer.
import { Endianness, Reader, Writer, TryConstruct, Status } from "@zerodds/wire";
import { zd_enc_Sample, zd_dec_Sample } from "./type.js";

const sample = { long_signed_a: -1234567, long_signed_b: 2147483647 };
const writer = new Writer(Endianness.Little, 4);
zd_enc_Sample(writer, sample);
const bytes = writer.finish();
const expected = [0x79, 0x29, 0xed, 0xff, 0xff, 0xff, 0xff, 0x7f];
if (bytes.length !== expected.length || bytes.some((value, i) => value !== expected[i])) {
  throw new Error("installed generated writer differs from primitive-long octets");
}
const reader = new Reader(bytes, Endianness.Little, 4);
const decoded = zd_dec_Sample(reader, TryConstruct.Discard);
if (reader.readerStatus() !== Status.Ok || decoded.long_signed_a !== sample.long_signed_a ||
    decoded.long_signed_b !== sample.long_signed_b) {
  throw new Error("installed generated reader lost primitive-long values");
}
console.log("INSTALLED_TYPESCRIPT_GENERATED_PASS: exact primitive-long octets and values");
