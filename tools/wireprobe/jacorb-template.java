// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The JacORB probe's write direction. `wireprobe-jacorb` substitutes
// @TYPE@ with the Java name and @HELPER@ with its Helper class, then
// compiles this against `jacorb idl`'s output.
//
// The payload goes to stdout with no encapsulation header, for the
// reason the omniORB template gives: CORBA CDR has none for a top-level
// parameter.
//
// ## Why there is no byte-order substitution
//
// `org.jacorb.orb.CDROutputStream` has no `setLittleEndian` — measured
// 2026-08-10 with `javap` against jacorb-3.9.jar, where only
// `CDRInputStream` has one. JacORB writes big endian, which is Java's
// own order and CDR's default, and `wireprobe-jacorb` refuses
// `--endian le` rather than labelling these bytes as little-endian.
// It is the mirror image of TAO, which can only write little.
//
// ## The fill buffer, checked
//
// The defect the RTI probe carried — several serialisations of one case
// sharing a buffer that is never cleared, so each reports the previous
// one's octets at its alignment gaps — cannot arise here. `main` writes
// one sample through one `CDROutputStream` and the process ends; JacORB
// refuses `--endian le` outright, so even `--matrix` is a single write.
// The buffer under that stream comes from `BufferManager.getBuffer`,
// whose pool is empty in a fresh JVM, and a Java array from `newarray
// byte` is zero-filled by JLS 4.12.5. Neither argument is load-bearing:
// JacORB zeroes the gap itself in `CDROutputStream.check(int, int)`,
// which every aligned primitive calls, so the fill would be `00` out of
// a warm pool as well. `docs/peer-defects.md` carries the line. Across
// the whole corpus the record holds `00` at every JacORB fill position
// and both halves of every annotated/unannotated twin pair agree, which
// is the same thing measured from the outside.
//
// Nothing to clear, then, and the zeros are also what JacORB puts on the
// wire: over GIOP/TCP the fill spans are `00`, 10 calls with a long and
// a short sample alternating through one connection
// (`tools/wire-capture`). Checked 2026-08-13.

import org.jacorb.orb.CDROutputStream;

public final class Probe {
    public static void main(String[] argv) throws Exception {
        @TYPE@ sample = null;
#FILL#

        CDROutputStream out = new CDROutputStream();
        @HELPER@.write(out, sample);
        byte[] bytes = out.getBufferCopy();
        System.out.write(bytes, 0, bytes.length);
        System.out.flush();
    }
}
