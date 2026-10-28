// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * Fresh-module contract probe for the TypeScript TypeSupport companion.
 * No DDS binding, no native FFI, no `koffi` — the codec/descriptor
 * boundary only, run against a fresh `zerodds-idlc generate --lang
 * typescript` output, the counterpart of `tools/idlc/tests/python-
 * support/consumer.py` and `tools/idlc/tests/csharp-support`.
 *
 * Usage: `node --import tsx consumer.ts GENERATED_MODULE_PATH`
 */

import { createHash } from "node:crypto";

declare const process: { argv: string[]; exit(code: number): never };

function require_(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

function hex(octets: Uint8Array): string {
  return Array.from(octets)
    .map((octet) => octet.toString(16).padStart(2, "0"))
    .join("");
}

// A dynamic `import()` of a path only known at run time (this is the
// same fresh, arbitrarily-named generated module `tools/idlc/tests/
// python-support/consumer.py` and `csharp-support`'s consumer load) has
// no static type — `unknown`, not `typeof import("./support.js")`, and
// deliberately so: casting it to the shape one particular generation
// happens to have would check this file against a fixture it does not
// actually load. Every shape check below is therefore a run-time one,
// which is what a "fresh module, no compile-time contract" probe means.
type Fresh = any;

async function main(): Promise<void> {
  const path = process.argv[2];
  if (!path) {
    throw new Error("usage: consumer.ts GENERATED_MODULE_PATH");
  }
  const generated: Fresh = await import(path);
  const bridge: Fresh = generated.bridge;

  // -- Point: a plain numeric key, well under the zero-pad threshold. --
  const point: Fresh = bridge.PointTypeSupport;
  require_(point.typeName === "bridge::Point", "type name lost");
  require_(point.typeObject.length > 0, "metadata absent");
  const sample: Fresh = { id: 42, reading: 65535 };
  require_(hex(point.encode(sample)) === "2a000000ffff", "LE octets differ");
  require_(hex(point.encode(sample, "be")) === "0000002affff", "BE octets differ");
  const decoded: Fresh = point.decode(new Uint8Array([0, 0, 0, 0x2a, 0xff, 0xff]), 0, 6, "be");
  require_(decoded.id === 42 && decoded.reading === 65535, "decoded values differ");
  const zero: Fresh = { id: 0, reading: 99 };
  const hashA: Uint8Array = point.keyHash(sample);
  const hashZero: Uint8Array = point.keyHash(zero);
  require_(hashA.length === 16, "KeyHash is not sixteen octets");
  require_(hex(hashZero) === "00000000000000000000000000000000".slice(0, 32), "zero key changed");
  require_(hex(hashA) !== hex(hashZero), "distinct keys collided");
  sample.reading = 7;
  require_(hex(point.keyHash(sample)) === hex(hashA), "non-key data changed the key");

  // -- ShortName: a bounded string<8> key, still under sixteen octets. --
  const shortName: Fresh = bridge.ShortNameTypeSupport;
  const a: Fresh = { name: "abcdX", value: 1 };
  const b: Fresh = { name: "abcdY", value: 1 };
  const ka: Uint8Array = shortName.keyHash(a);
  const kb: Uint8Array = shortName.keyHash(b);
  require_(ka.length === 16 && kb.length === 16, "short-name KeyHash length");
  require_(hex(ka) !== hex(kb), "short-name keys collided");
  const shortDecoded: Fresh = shortName.decode(shortName.encode(a));
  require_(shortDecoded.name === "abcdX" && shortDecoded.value === 1, "short-name roundtrip");

  // -- Named: a bounded string<32> key — the bound alone (not this
  // sample's actual length) exceeds sixteen octets, so the generated
  // `keyHash` must select MD5, never zero-padding, checked against
  // Node's own digest as an INDEPENDENT oracle (never the runtime's own
  // hand-rolled `md5()`, which is what is under test here). --
  const named: Fresh = bridge.NamedTypeSupport;
  const namedSample: Fresh = { name: "a", value: 1 };
  const namedHolder = new Uint8Array([0, 0, 0, 2, 0x61, 0]); // len(2) + "a\0", big-endian, PLAIN_CDR2-BE
  const expectedMd5 = createHash("md5").update(namedHolder).digest();
  require_(hex(named.keyHash(namedSample)) === hex(expectedMd5), "maximum-size MD5 choice differs");
  const namedDecoded: Fresh = named.decode(named.encode(namedSample));
  require_(namedDecoded.name === "a" && namedDecoded.value === 1, "named roundtrip");

  // -- Extended: @appendable, an @optional non-key member. --
  const extended: Fresh = bridge.ExtendedTypeSupport;
  for (const calibration of [undefined, 1.25]) {
    const value: Fresh = { id: 13, calibration };
    const got: Fresh = extended.decode(extended.encode(value));
    require_(got.id === 13 && got.calibration === calibration, "optional fidelity lost");
  }

  // -- Empty: no members, no key. --
  const empty: Fresh = bridge.EmptyTypeSupport;
  require_(empty.encode({}).length === 0, "empty type acquired bytes");
  require_(empty.isKeyed === false, "empty type must not claim a key");
  require_(!("keyHash" in empty), "a keyless companion must not offer keyHash");

  // -- Every representation refusal, structurally. --
  let refused = false;
  try {
    point.encode(sample, "le", 0);
  } catch {
    refused = true;
  }
  require_(refused, "wrong representation was accepted on encode");
  refused = false;
  try {
    point.decode(point.encode(sample), 0, undefined, "le", 0);
  } catch {
    refused = true;
  }
  require_(refused, "wrong representation was accepted on decode");
  refused = false;
  try {
    point.decode(new Uint8Array([0]));
  } catch {
    refused = true;
  }
  require_(refused, "truncated body was accepted");
  refused = false;
  try {
    point.decode(Uint8Array.from([...point.encode(sample), 0]));
  } catch {
    refused = true;
  }
  require_(refused, "trailing octets were accepted");

  console.log(
    "TYPESCRIPT_GENERATED_TYPESUPPORT_PASS: actual classes, LE/BE, optional, exact keys, MD5 selection, refusals",
  );
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
