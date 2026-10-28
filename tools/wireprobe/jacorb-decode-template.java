// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The JacORB probe's read direction. The payload comes in on stdin with
// no encapsulation header and the values go to stdout as canonical
// JSON.
//
// Unlike the write direction this one takes both byte orders:
// `CDRInputStream.setLittleEndian` exists and is what a GIOP message's
// own header would drive.
//
// A payload JacORB refuses is exit 2 and not exit 1 — the stream is
// wrong, which is a different finding from the probe being unable to
// run. `org.omg.CORBA.MARSHAL` is what it raises when the bytes run out.

import java.io.ByteArrayOutputStream;
import java.io.InputStream;

import org.jacorb.orb.CDRInputStream;

public final class Probe {
    public static void main(String[] argv) throws Exception {
        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        InputStream in = System.in;
        byte[] chunk = new byte[4096];
        int got;
        while ((got = in.read(chunk)) > 0) {
            buffer.write(chunk, 0, got);
        }

        @TYPE@ sample;
        try {
            CDRInputStream cdr = new CDRInputStream(buffer.toByteArray());
            // The byte order is the probe's first argument and not a
            // splice: it used to be substituted before `javac` ran, so
            // the two orders of one case meant two runs of the JacORB
            // IDL compiler over identical generated code.
            cdr.setLittleEndian(!(argv.length > 0 && argv[0].equals("be")));
            sample = @HELPER@.read(cdr);
        } catch (org.omg.CORBA.SystemException error) {
            System.err.println("jacorb refused the payload: " + error);
            System.exit(1);
            return;
        }

#DUMP#
        System.out.flush();
    }
}
