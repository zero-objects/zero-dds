// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// topic-typed-smoke.ts — Referenz-Smoke fuer zerodds-xcdr2-ts-1.0 §10.
//
// Zeigt encode/decode-Roundtrip eines hand-implementierten
// `DdsTopicType<Point>` (entspricht dem was idl-ts pro IDL-`struct`
// emittieren wuerde). Kein Live-DDS — nur das XCDR2-Wire-Layer.
//
// Run:
//   cd crates/ts-node
//   npx tsx examples/topic-typed-smoke.ts

import {
    DdsTopicType,
    EndianMode,
    Xcdr2Reader,
    Xcdr2Writer,
} from "../src/cdr/index.js";

interface Point {
    x: number;
    y: number;
}

const PointTypeSupport: DdsTopicType<Point> = {
    typeName: "Point",
    isKeyed: false,
    extensibility: "final",

    encode(s: Point, endian: EndianMode = "le"): Uint8Array {
        const w = new Xcdr2Writer(endian);
        w.writeInt32(s.x);
        w.writeInt32(s.y);
        return w.toBytes();
    },

    decode(bytes: Uint8Array, offset = 0, length = bytes.length - offset): Point {
        const r = new Xcdr2Reader(bytes, offset, length, "le");
        return { x: r.readInt32(), y: r.readInt32() };
    },

    keyHash(_s: Point): Uint8Array {
        return new Uint8Array(16);
    },
};

function toHex(bytes: Uint8Array): string {
    return Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, "0").toUpperCase())
        .join(" ");
}

function main(): number {
    const p: Point = { x: 42, y: -7 };
    const ts = PointTypeSupport;

    console.log(`typeName = ${ts.typeName}`);
    console.log(`extensibility = ${ts.extensibility}`);
    console.log(`sample = Point(x=${p.x}, y=${p.y})`);

    const bytes = ts.encode(p);
    console.log(`wire = ${toHex(bytes)}`);

    const roundtripped = ts.decode(bytes);
    console.log(`roundtrip = Point(x=${roundtripped.x}, y=${roundtripped.y})`);

    if (p.x !== roundtripped.x || p.y !== roundtripped.y) {
        console.error("FAIL: roundtrip mismatch");
        return 1;
    }
    console.log("OK");
    return 0;
}

process.exit(main());
