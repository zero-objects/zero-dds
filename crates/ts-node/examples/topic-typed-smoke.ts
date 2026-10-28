// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// topic-typed-smoke.ts — reference smoke for zerodds-xcdr2-ts-1.0 §10.
//
// Shows the encode/decode roundtrip of a hand-implemented `TypeSupport<Point>`
// (matches the shape a generated companion has, calling straight into
// `@zerodds/wire` (`runtime/typescript`), the one XCDR2 codec this binding
// has). `zerodds-idlc generate --lang typescript` now emits the real thing —
// an `export const <Type>TypeSupport` per topic type, appended to the same
// generated file — see `tools/idlc/tests/typescript-support/` for a fresh
// generation exercised end to end (byte-compared to the Rust reference,
// type-checked against `dds.ts`'s own `TypeSupport<T>`) and
// `crates/ts-node/CHANGELOG.md`. No live DDS here — only the XCDR2 wire layer.
//
// `TypeSupport<T>` is `../src/dds.ts`'s own bridge interface: passing an
// object shaped like this to `DomainParticipant.createTypedTopic` is the
// real, current way to get a typed `Topic<T>` in this binding.
//
// Run:
//   cd crates/ts-node
//   npx tsx examples/topic-typed-smoke.ts

import {
    Writer,
    Reader,
    Endianness,
    PID_FLAGS_SENTINEL_MUST_UNDERSTAND,
    type PlCdr1Framing,
} from "@zerodds/wire";
import type { TypeSupport } from "../src/dds.js";

interface Point {
    x: number;
    y: number;
}

const XCDR2_FRAMING: PlCdr1Framing = {
    pidFlags: PID_FLAGS_SENTINEL_MUST_UNDERSTAND,
    shortMaxId: 0x3f00,
    padsDeclaredLength: true,
    padsOptionalLength: true,
    memberMustUnderstand: true,
    padAfterValue: true,
};

function order(endian: "le" | "be"): Endianness {
    return endian === "be" ? Endianness.Big : Endianness.Little;
}

const PointTypeSupport: TypeSupport<Point> = {
    typeName: "Point",
    isKeyed: false,

    encode(s: Point, endian: "le" | "be" = "le"): Uint8Array {
        const w = new Writer(order(endian), 4, XCDR2_FRAMING, false);
        w.writeI32(s.x);
        w.writeI32(s.y);
        return w.finish();
    },

    decode(bytes: Uint8Array, endian: "le" | "be" = "le"): Point {
        const r = new Reader(bytes, order(endian), 4, XCDR2_FRAMING);
        return { x: r.readI32(), y: r.readI32() };
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
