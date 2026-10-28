// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The canonical-JSON helpers the TypeScript probe's decode direction
// calls. The TypeScript counterpart of `tools/wireprobe/Zdj.cs`, of
// `tools/wireprobe/Zdj.java` and of `tools/wireprobe/dumpjson.h`,
// printing the same form for the same value — a difference between two
// columns' answers must be a difference in the bytes and not in how the
// two were printed.
//
// Two things have to be right here, because no reader can recover them:
// a 64-bit integer is a **string**, since a JSON number is a double and
// loses the low bits past 2^53; and an absent `@optional` member is a
// key that was never printed, never a `null`.

/**
 * The two globals this module uses Node for, declared rather than
 * imported: `runtime/typescript` depends on nothing from Node and the
 * probe compiles without `@types/node`, so the names are stated here
 * with exactly the shape they are used in. `TextEncoder` is a Node
 * global since Node 11 and always encodes UTF-8 — the same octets
 * `dumpjson.h`'s `zdj_string` passes, and never the console encoding
 * that once turned a JacORB run's own output into a vendor finding.
 */
declare const process: {
  readonly stdout: { write(chunk: Uint8Array): boolean };
};

declare class TextEncoder {
  encode(text: string): Uint8Array;
}

/**
 * The first-member flag `Zdj.key` reads and clears.
 *
 * An object and not a `boolean`, because the flag is shared between the
 * printer's statements and this module: a `boolean` argument is a copy,
 * and the comma would come out before every member or before none.
 */
export interface ZdjFirst {
  v: boolean;
}

/** The pieces, joined and encoded once at `flush`. */
const parts: string[] = [];

function pushHex4(code: number): void {
  parts.push("\\u" + (code & 0xffff).toString(16).padStart(4, "0"));
}

/**
 * A `float` or a `double`, as the shortest decimal that reads back as
 * the same value — which is what `Number.prototype.toString` produces,
 * and what `Zdj.cs`'s `ToString("R")` and `Zdj.java`'s
 * `Double.toString` produce for theirs. The Rust side reduces both
 * sides to that same form before anything is compared, so the spelling
 * here only has to lose nothing.
 *
 * `-0` is spelled out: `String(-0)` is `"0"`, and a writer puts a
 * different octet on the wire for the two.
 */
function pushFloat(value: number): void {
  if (Object.is(value, -0)) {
    parts.push("-0");
    return;
  }
  parts.push(String(value));
}

export const Zdj = {
  flush(): void {
    process.stdout.write(new TextEncoder().encode(parts.join("")));
    parts.length = 0;
  },

  raw(text: string): void {
    parts.push(text);
  },

  /**
   * The separator and the key together: the comma belongs to the second
   * and later members, and only the printer knows which those are.
   */
  key(first: ZdjFirst, name: string): void {
    if (!first.v) {
      parts.push(",");
    }
    first.v = false;
    parts.push('"' + name + '":');
  },

  bool(value: boolean): void {
    parts.push(value ? "true" : "false");
  },

  i64(value: number | bigint): void {
    parts.push(value.toString());
  },

  u64(value: number | bigint): void {
    parts.push(value.toString());
  },

  i64String(value: bigint): void {
    parts.push('"' + value.toString() + '"');
  },

  u64String(value: bigint): void {
    parts.push('"' + value.toString() + '"');
  },

  f32(value: number): void {
    pushFloat(value);
  },

  f64(value: number): void {
    pushFloat(value);
  },

  /**
   * An IDL `char` is one octet, so 0xE4 is U+00E4 — the same mapping the
   * Rust reader makes. The TypeScript mapping already handed it over as
   * a one-character string, so the octet is the low eight bits of its
   * only code unit.
   */
  ch(value: string): void {
    parts.push('"');
    pushHex4(value.charCodeAt(0) & 0xff);
    parts.push('"');
  },

  /**
   * A `wchar`, as the UTF-16 code units it is made of. A code point
   * above the Basic Multilingual Plane is already the surrogate pair
   * §7.4.1.1.2 puts on the wire, and both units are escaped.
   */
  wch(value: string): void {
    parts.push('"');
    for (let at = 0; at < value.length; at += 1) {
      pushHex4(value.charCodeAt(at));
    }
    parts.push('"');
  },

  /** A narrow string, escaped the way `Zdj.cs` escapes one. */
  string(text: string): void {
    parts.push('"');
    for (const c of text) {
      switch (c) {
        case '"':
          parts.push('\\"');
          break;
        case "\\":
          parts.push("\\\\");
          break;
        case "\n":
          parts.push("\\n");
          break;
        case "\r":
          parts.push("\\r");
          break;
        case "\t":
          parts.push("\\t");
          break;
        default:
          if (c < " ") {
            pushHex4(c.charCodeAt(0));
          } else {
            parts.push(c);
          }
          break;
      }
    }
    parts.push('"');
  },

  /**
   * A wide string, every UTF-16 code unit as a `\uXXXX` escape. The
   * escape *is* JSON's own carrier for a unit, so no UTF-8 encoder is
   * involved and none can be got wrong.
   */
  wstring(text: string): void {
    parts.push('"');
    for (let at = 0; at < text.length; at += 1) {
      pushHex4(text.charCodeAt(at));
    }
    parts.push('"');
  },

  /**
   * A `fixed<P,S>`, as the decimal text the column holds it as.
   * `adapter/typescript.rs` says why it is a `string`; the Rust side
   * normalises to the declared scale before comparing.
   */
  fixed(value: string): void {
    parts.push('"' + value + '"');
  },

  /**
   * A value that names no enumerator. Printed as a string no enumerator
   * has, so the comparison refuses it by name — a bare number here
   * would read as some other member's value and pass.
   */
  enumUnknown(value: number | bigint | string): void {
    parts.push('"<enumerator ' + value.toString() + '>"');
  },

  /**
   * A `long double`, from the sixteen octets `runtime/typescript` holds
   * it as, least-significant first — as a string holding every digit
   * the value has.
   *
   * Byte for byte the same text `runtime/csharp/src/Float128.cs`'s
   * `ToExactDecimal`, `runtime/c/src/float128_decimal.c` and
   * `runtime/java`'s `Float128` produce for the same pattern: the
   * columns are compared as text, so the renderers have to agree on the
   * form as well as on the value. `BigInt` is arbitrary precision, so
   * `significand * 5^-exponent` is exact and nothing is rounded.
   */
  f128(octets: Uint8Array): void {
    if (octets.length !== 16) {
      // A refusal comes out as `null` rather than as an empty string,
      // so it fails a comparison instead of matching a value nobody
      // wrote — `dumpjson.h`'s rule.
      parts.push("null");
      return;
    }
    let low = 0n;
    let high = 0n;
    for (let at = 7; at >= 0; at -= 1) {
      low = (low << 8n) | BigInt(octets[at] ?? 0);
    }
    for (let at = 15; at >= 8; at -= 1) {
      high = (high << 8n) | BigInt(octets[at] ?? 0);
    }
    const negative = high >> 63n !== 0n;
    const biased = Number((high >> 48n) & 0x7fffn);
    const fraction = ((high & 0xffffffffffffn) << 64n) | low;
    if (biased === 0x7fff) {
      parts.push(fraction === 0n ? (negative ? '"-inf"' : '"inf"') : '"nan"');
      return;
    }
    const significand = biased === 0 ? fraction : fraction | (1n << 112n);
    if (significand === 0n) {
      parts.push(negative ? '"-0.0"' : '"0.0"');
      return;
    }
    const exponent = (biased === 0 ? 1 : biased) - 16383 - 112;
    let digits: string;
    let point: number;
    if (exponent >= 0) {
      digits = (significand << BigInt(exponent)).toString();
      point = digits.length;
    } else {
      digits = (significand * 5n ** BigInt(-exponent)).toString();
      point = digits.length + exponent;
    }
    let end = digits.length;
    while (end > 0 && digits[end - 1] === "0") {
      end -= 1;
    }
    digits = end === 0 ? "0" : digits.slice(0, end);
    let text = negative ? "-" : "";
    if (point > 0 && point <= 34) {
      text +=
        digits.length <= point
          ? digits + "0".repeat(point - digits.length) + ".0"
          : digits.slice(0, point) + "." + digits.slice(point);
    } else if (point <= 0 && point > -6) {
      text += "0." + "0".repeat(-point) + digits;
    } else {
      const shifted = point - 1;
      text +=
        digits.slice(0, 1) +
        "." +
        (digits.length === 1 ? "0" : digits.slice(1)) +
        "e" +
        (shifted >= 0 ? "+" : "-") +
        Math.abs(shifted).toString();
    }
    parts.push('"' + text + '"');
  },
};
