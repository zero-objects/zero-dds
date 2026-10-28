// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The generated `TypeSupport` companion, against the octets the Rust
 * column recorded for `tests/interop-corpus/struct-appendable/` — the
 * same ledger `runtime/typescript/tests/against-recorded-octets.ts`
 * holds its own hand-rolled `Writer` calls against, held here against
 * `zerodds-idlc generate --lang typescript`'s own companion instead.
 *
 * `tests/interop-corpus/struct-appendable/type.idl` is
 * `@appendable struct Sample { long a; double b; short c; };`
 * (top-level, no module); `values.json` is
 * `{ "a": -1234567, "b": 3.141592653589793, "c": -2 }`.
 *
 * A generated `TypeSupport` locks in the one representation it was
 * generated for (`TopicDescriptor.representation`'s own Python
 * counterpart), so the two representations need two separate
 * generations — `xcdr1_xcdr1_gen.py`/`xcdr1_xcdr2_gen.py`'s TypeScript
 * counterpart. And unlike Python, `runtime/typescript`'s `registerType`
 * (DDS-TS 1.0 chapter 8) refuses a second, structurally different
 * registration under the same qualified name — the reason this file
 * takes ONE generated module and is run twice, once per representation,
 * rather than importing both into the one process a single run would be.
 *
 * Usage: `npx tsx against-recorded-octets.ts GENERATED_MODULE_PATH
 * plain-cdr1|plain-cdr2`
 */

import { readFileSync } from "node:fs";

declare const process: { argv: string[]; exit(code: number): never };

function hex(octets: Uint8Array): string {
  return Array.from(octets)
    .map((octet) => octet.toString(16).padStart(2, "0"))
    .join("");
}

function repositoryRoot(startingAt: string): string {
  let at = startingAt;
  for (let depth = 0; depth < 10; depth += 1) {
    try {
      readFileSync(`${at}/tests/interop-corpus/struct-appendable/type.idl`);
      return at;
    } catch {
      at = `${at}/..`;
    }
  }
  throw new Error("the corpus is not above this file");
}

async function main(): Promise<void> {
  const modulePath = process.argv[2];
  const encoding = process.argv[3];
  if (!modulePath || (encoding !== "plain-cdr1" && encoding !== "plain-cdr2")) {
    throw new Error("usage: against-recorded-octets.ts GENERATED_MODULE_PATH plain-cdr1|plain-cdr2");
  }
  const representation = encoding === "plain-cdr1" ? 0 : 2;
  const sample = { a: -1234567, b: 3.141592653589793, c: -2 };

  const root = repositoryRoot(new URL(".", import.meta.url).pathname);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const generated: any = await import(modulePath);
  const support = generated.SampleTypeSupport;

  let failures = 0;
  let checked = 0;
  for (const endian of ["le", "be"] as const) {
    const recorded = readFileSync(
      `${root}/tests/interop-results/struct-appendable/zerodds-rust/${encoding}.${endian}.bin`,
    );
    const got = support.encode(sample, endian, representation);
    checked += 1;
    if (hex(got) !== hex(recorded)) {
      failures += 1;
      console.log(`${encoding}/${endian}`);
      console.log(`  typescript ${hex(got)}`);
      console.log(`  rust       ${hex(recorded)}`);
    }
    // The generated decoder reads the Rust reference's own bytes back to
    // the same values — a roundtrip through the SAME representation the
    // Rust column wrote, not through this run's own encoder alone.
    const back = support.decode(recorded, 0, recorded.length, endian, representation);
    checked += 1;
    if (back.a !== sample.a || back.b !== sample.b || back.c !== sample.c) {
      failures += 1;
      console.log(`${encoding}/${endian} decode: ${JSON.stringify(back)}`);
    }
  }
  console.log(`${checked - failures} of ${checked} agree with the Rust column (${encoding})`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
