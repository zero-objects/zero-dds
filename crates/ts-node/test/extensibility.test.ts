// extensibility.test.ts — `Writer` declares the type's extensibility to the
// C ABI.
//
// The encapsulation identifier the runtime stamps on a user payload is chosen
// from the writer slot's `wire_extensibility`, which defaults to FINAL —
// `PLAIN_CDR2_LE`, `0x0007`. An `@appendable` body opens with a DHEADER and a
// `@mutable` one with an EMHEADER per member, so a writer that never calls
// `zerodds_writer_set_wire_extensibility` announces `0x0007` over a body that
// is neither. Measured on the Linux test host against OpenDDS 3.35.0, three topics whose
// types differ in nothing but the extensibility, 24 samples each:
//
//   body ZdExtKFinal       07b201000e640300
//   body ZdExtKAppendable  0800000007b201000e640300
//   body ZdExtKMutable     100000000100002007b20100020000200e640300
//
//   SAMPLES ZdExtKFinal       24      <- the control
//   SAMPLES ZdExtKAppendable   0
//   SAMPLES ZdExtKMutable      0
//
//   ERROR: to_encoding_i: expected appendable extensibility, but got
//          XCDR2 Little Endian Plain                             (x24)
//   ERROR: to_encoding_i: expected mutable extensibility, but got
//          XCDR2 Little Endian Plain                             (x24)
//
// and all three bodies went out behind `00070000`. With the declaration the
// same run reads 24 / 24 / 24 behind `00070000`, `00090000` and `000b0000`,
// the bodies byte for byte the same. CycloneDDS accepts all three either way
// — it reads the body with its own type and not with the header — which is
// what made the defect invisible between two lenient peers.
//
// # Why a recording stub and not a live topic
//
// What is under test is one seam: the constructor hands the extensibility to
// the C ABI. A live pub/sub run would need a domain, discovery and a strict
// reader to observe a value this test can read straight off the call. So the
// C ABI is a recording stub here — a small shared library that prints what
// the constructor asked for. koffi resolves every symbol `native.ts` declares
// at load time, so the stub links against the real `libzerodds` and overrides
// only the five functions the constructor touches; the other sixty resolve
// through that dependency. The mapping from the recorded argument to the four
// octets on the wire is `crates/dcps/src/runtime.rs::user_payload_encap`.

import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { LIBRARY_PATH } from "../src/native.js";

const here = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(here, "..");

/// The C ABI, as far as `Writer`'s constructor reaches it.
///
/// Every line is printed from C with an explicit flush, so the order the
/// assertions read is the order the calls happened in — no interleaving with
/// the probe's own output, which goes to stderr.
const STUB_C = `
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int runtime_token = 1;
static int writer_token = 2;
static const char *last_topic = "none";

/* The status \`zerodds_writer_set_wire_extensibility\` answers with, so the
   refusal path is reachable without a second stub. */
static int set_rc(void) {
  const char *rc = getenv("ZD_STUB_SET_RC");
  return rc ? atoi(rc) : 0;
}

void *zerodds_runtime_create(uint32_t domain) {
  (void)domain;
  return &runtime_token;
}

void zerodds_runtime_destroy(void *runtime) { (void)runtime; }

void *zerodds_writer_create(void *runtime, const char *topic,
                            const char *type_name, int reliable) {
  (void)runtime; (void)type_name; (void)reliable;
  last_topic = topic;
  printf("CREATE %s\\n", topic);
  fflush(stdout);
  return &writer_token;
}

int zerodds_writer_set_wire_extensibility(void *writer, int extensibility) {
  (void)writer;
  printf("EXT %s %d\\n", last_topic, extensibility);
  fflush(stdout);
  return set_rc();
}

void zerodds_writer_destroy(void *writer) {
  (void)writer;
  printf("DESTROY %s\\n", last_topic);
  fflush(stdout);
}
`;

/// Four writers that differ only in what they declare, then the two refusals.
///
/// The no-argument writer is here because every existing caller is one: the
/// default has to stay FINAL, which is what the C ABI does without the call
/// and what those callers' bodies are framed as.
const PROBE_TS = `
import { Runtime, Writer, ZeroDdsError } from "PACKAGE_ROOT/src/index.js";

const rt = new Runtime(0);

new Writer(rt, "Default", "Default", true);
new Writer(rt, "Final", "Final", true, "final");
new Writer(rt, "Appendable", "Appendable", true, "appendable");
new Writer(rt, "Mutable", "Mutable", true, "mutable");

// A misspelt extensibility must not reach the C ABI: read as FINAL it is the
// guess this argument exists to stop. No CREATE line may follow.
try {
  new Writer(rt, "Typo", "Typo", true, "apendable" as never);
  console.error("NOTHROW badvalue");
} catch (e) {
  console.error("THREW badvalue", (e as ZeroDdsError).message);
}

// A refused declaration destroys the writer instead of handing out one that
// publishes under an identifier that does not name its framing.
process.env.ZD_STUB_SET_RC = "-7";
try {
  new Writer(rt, "Refused", "Refused", true, "appendable");
  console.error("NOTHROW refused");
} catch (e) {
  console.error("THREW refused", (e as ZeroDdsError).message);
}
`;

/// Builds the stub and runs the probe against it; returns its two streams.
function runProbe(): { out: string; err: string } {
  const dir = mkdtempSync(join(tmpdir(), "zerodds-ts-extensibility-"));
  try {
    const stubC = join(dir, "stub.c");
    const stubSo = join(dir, "libzerodds.so");
    const probe = join(dir, "probe.ts");
    writeFileSync(stubC, STUB_C);
    writeFileSync(probe, PROBE_TS.replace("PACKAGE_ROOT", packageRoot));

    // `--no-as-needed`: nothing in the stub references the real library, so
    // the linker would drop the dependency the other sixty symbols resolve
    // through. `-rpath` so the loader finds it from the temp directory.
    execFileSync(
      "cc",
      [
        "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
        "-o", stubSo, stubC,
        "-Wl,--no-as-needed",
        "-L", dirname(LIBRARY_PATH), "-lzerodds",
        `-Wl,-rpath,${dirname(LIBRARY_PATH)}`,
      ],
      { stdio: "pipe" },
    );

    const run = spawnSync(process.execPath, ["--import", "tsx", probe], {
      cwd: packageRoot,
      env: { ...process.env, ZERODDS_LIB: stubSo, ZD_STUB_SET_RC: "0" },
      encoding: "utf8",
    });
    assert.equal(
      run.status,
      0,
      `the probe did not run:\n${run.stdout}\n${run.stderr}`,
    );
    return { out: run.stdout, err: run.stderr };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("ts Writer declares the type's extensibility to the C ABI", () => {
  const { out, err } = runProbe();
  const lines = out.split("\n").filter((l) => l.length > 0);

  // `0`/`1`/`2` are `zerodds_writer_set_wire_extensibility`'s own numbering,
  // documented on the function and read there by `ExtensibilityForRepr`.
  assert.deepEqual(lines, [
    "CREATE Default",
    "EXT Default 0",
    "CREATE Final",
    "EXT Final 0",
    "CREATE Appendable",
    "EXT Appendable 1",
    "CREATE Mutable",
    "EXT Mutable 2",
    // No CREATE for "Typo": rejected before the writer exists.
    "CREATE Refused",
    "EXT Refused 1",
    "DESTROY Refused",
  ]);

  assert.match(err, /THREW badvalue .*"apendable"/);
  assert.match(err, /THREW refused .*zerodds_writer_set_wire_extensibility/);
  assert.doesNotMatch(err, /NOTHROW/);
});
