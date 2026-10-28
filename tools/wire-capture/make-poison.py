#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# make-poison.py — the alternating sample, per corpus case.
#
#   tools/wire-capture/make-poison.py --cases-file tools/wire-capture/cases.tsv
#   tools/wire-capture/make-poison.py --case array-1d-primitives --force
#
# ## Why a poison sample exists at all
#
# `wire-golden.sh` writes **poison, corpus, poison, corpus, …** through
# one writer. Two of the four checks in `check-run.py` rest on the two
# samples being different octets:
#
#   - check 3, that the corpus sample serialises to the same octets
#     every time, is only a check if something else went through the
#     same writer in between;
#   - check 4, that the corpus payload differs from the poison payload,
#     is what catches an extractor reading something that is not the
#     sample. Two identical payloads would pass every other check.
#
# ## What this generator does, and what it deliberately does not
#
# It mutates **numeric leaves only**, and by a bijection: an integer
# becomes `v ^ 1`, a float becomes `-v`, a boolean flips. Nothing else
# in the JSON is touched.
#
# That is narrower than the hand-written poison files under `poison/`,
# which are *longer* than their corpus sample wherever the type has a
# variable-length member, so the corpus sample's alignment fill lands on
# octets the poison call wrote. Those twenty-three carry `longer` in
# `cases.tsv`'s control column and they answer A4's fill-provenance
# question. A generated poison carries `equal`: the two samples have the
# same layout, so neither can write into the other's fill, and the row's
# fill column reads `undetermined`. That is a real limit of the
# generated half and it is recorded rather than papered over — every
# other question a capture answers (the encapsulation identifier, the
# options field, the DHEADER, the EMHEADERs, the KeyHash, the byte
# order, the octets themselves) needs only that the two samples differ.
#
# **The mutations are bijections on purpose.** `v ^ 1` never maps two
# distinct integers onto one, so a `map` whose keys are mutated cannot
# come out with a duplicate key — which a vendor's own map would refuse
# at run time, and which would read as a refusal of the case.
#
# What is left alone, and why each one would break if it were not:
#
#   - `"_d"`, a union's discriminator: mutating it selects a different
#     branch or none, so the poison would exercise a different rule than
#     the corpus sample and check 3 would be comparing two shapes.
#   - enumerator names (`"BLUE"`) and bitmask flag lists: a shifted name
#     is not a name the IDL declares and `wireprobe-fill` refuses the
#     file. **The character fallback below used to shift them anyway**,
#     because it cannot see a type and a JSON string is a JSON string:
#     `enum-negative-value`'s sample is `{"first": "MINUS", "second":
#     "MAX"}` and has no numeric leaf at all, so the fallback fired and
#     wrote `"NJOVT"` and `"NBY"`. Six rows of the 2026-08-20 table then
#     read as Fast DDS, OpenDDS and RTI refusing a type all three
#     compile — `wireprobe-fill: Sample.first: not a member; the type
#     declares ["MINUS", "ZERO", "MAX"]` — and `refusal-check.py` is
#     what caught it. An enumerator now rotates to the **next
#     enumerator the case's own `type.idl` declares**, which is a
#     bijection on that enum and is always a name the file has.
#   - character and string values: a `char` is one code point wide and a
#     `string<N>` has a bound, so a general string mutation needs the
#     IDL's bound to stay inside it. Cases whose sample holds *no*
#     numeric leaf at all get the one narrow exception below.
#
# The exception: a case with nothing numeric to mutate — `primitive-wchar`
# is the whole of it — takes a **same-plane character** substitution, a
# code point of the same UTF-8 length and the same BMP/astral side, so
# neither the octet count nor the surrogate-pair question moves.
import argparse
import json
import pathlib
import re
import sys

INTEGER_TEXT = re.compile(r"^[+-]?[0-9]+$")
# A decimal or exponent literal carried as a string: `fixed<P,S>` and
# `long double` both arrive that way, because neither survives a JSON
# double. Toggling the sign keeps the digit count and the scale, which a
# `fixed<1,0>` needs — the sign is not one of its digits.
DECIMAL_TEXT = re.compile(r"^[+-]?(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$")


class Mutation:
    """How many leaves the walk changed, so a caller can refuse a file
    that came out identical to its input."""

    def __init__(self):
        self.count = 0


def mutate_number_text(text):
    if INTEGER_TEXT.match(text):
        return str(int(text) ^ 1)
    if DECIMAL_TEXT.match(text):
        return text[1:] if text.startswith("-") else "-" + text
    return None


def mutate_numbers(value, state, key=None):
    if isinstance(value, dict):
        return {
            name: (inner if name == "_d" else mutate_numbers(inner, state, name))
            for name, inner in value.items()
        }
    if isinstance(value, list):
        return [mutate_numbers(item, state, key) for item in value]
    if isinstance(value, bool):
        state.count += 1
        return not value
    if isinstance(value, int):
        state.count += 1
        return value ^ 1
    if isinstance(value, float):
        state.count += 1
        return -value if value != 0.0 else 1.0
    if isinstance(value, str):
        replaced = mutate_number_text(value)
        if replaced is not None:
            state.count += 1
            return replaced
    return value


def substitute_code_point(point):
    """A different code point of the same UTF-8 length and the same side
    of the BMP boundary, so a `char`, a `wchar` and a surrogate pair each
    stay the width the corpus case was written to measure."""
    if point < 0x80:
        base, span = 0x21, 0x5E
    elif point < 0x800:
        base, span = 0x80, 0x780
    elif point < 0x10000:
        # Skip the surrogate block: D800–DFFF is not a code point and
        # `wireprobe-fill` writes the sample as UTF-8.
        base, span = 0xE000, 0x1900
    else:
        base, span = 0x10000, 0x10000
    shifted = base + ((point - base + 1) % span)
    return shifted


# `enum Name { A, B };` and `bitmask Name { F, G };`, with `@value(-1)`,
# `@position(3)` and comments in between. Read out of the case's own
# `type.idl` rather than out of a table here, so a case whose enum
# changes cannot leave this file describing the enum it used to have.
ENUM_BLOCK = re.compile(r"\b(?:enum|bitmask)\s+[A-Za-z_][A-Za-z0-9_]*\s*\{([^}]*)\}")
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
ANNOTATION = re.compile(r"@[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)]*\))?")


def enumerator_successor(idl_text):
    """enumerator -> the next enumerator of the same declaration.

    A rotation inside one enum, so the mutated name is always a name the
    IDL declares — which a letter shift is not — and the mapping stays a
    bijection, which is what keeps a mutated `map` key from colliding
    with another."""
    successor = {}
    for body in ENUM_BLOCK.findall(idl_text):
        names = IDENTIFIER.findall(ANNOTATION.sub(" ", body))
        if len(names) < 2:
            continue
        for index, name in enumerate(names):
            successor[name] = names[(index + 1) % len(names)]
    return successor


def mutate_characters(value, state, successor):
    if isinstance(value, dict):
        return {
            name: (inner if name == "_d"
                   else mutate_characters(inner, state, successor))
            for name, inner in value.items()
        }
    if isinstance(value, list):
        return [mutate_characters(item, state, successor) for item in value]
    if isinstance(value, str) and value:
        state.count += 1
        if value in successor:
            return successor[value]
        return "".join(chr(substitute_code_point(ord(c))) for c in value)
    return value


def poison_for(sample, idl_text=""):
    state = Mutation()
    poisoned = mutate_numbers(sample, state)
    if state.count == 0:
        poisoned = mutate_characters(sample, state, enumerator_successor(idl_text))
    return poisoned, state.count


def main():
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--root", default=None,
                        help="the repository root; default is this file's")
    parser.add_argument("--case", action="append", default=[],
                        help="one corpus case; repeatable")
    parser.add_argument("--cases-file", default=None,
                        help="a cases.tsv whose first column names the cases")
    parser.add_argument("--force", action="store_true",
                        help="overwrite the poison files named by --case. Has "
                             "no effect on --cases-file, which never "
                             "overwrites; see --force-every-case")
    parser.add_argument("--force-every-case", action="store_true",
                        help="overwrite every existing poison file, including "
                             "the twenty-three hand-written ones. Replaces a "
                             "hand-written fill control with a generated one")
    arguments = parser.parse_args()

    here = pathlib.Path(__file__).resolve().parent
    root = pathlib.Path(arguments.root).resolve() if arguments.root else here.parent.parent
    corpus = root / "tests" / "interop-corpus"
    out = here / "poison"
    out.mkdir(exist_ok=True)

    # `--force` names cases; `--cases-file` names the corpus.
    #
    # The two used to be one flag, and
    # `--cases-file tools/wire-capture/cases.tsv --force` overwrote all
    # **twenty-three hand-written** poison files with generated ones. It
    # reported "107 written" and looked like a regeneration. It is not a
    # formatting change: thirteen of the twenty-three are the ones whose
    # sample is *longer* than the corpus sample, which is the control
    # A4's fill-provenance question rests on — the corpus sample's fill
    # then lands on octets the poison call wrote, so a `00` there is an
    # octet the peer wrote and not a buffer that happened to be zero. A
    # generated sample has the same layout and cannot reach the other's
    # fill at all, and the loss would have surfaced as those cases
    # quietly answering `undetermined`.
    #
    # `cases.tsv`'s `control` column does not separate them: only ten of
    # the twenty-three carry `longer`, because the rest are hand-written
    # for a type with no variable-length member. So the guard is not on
    # the column. **Bulk never overwrites**; one case at a time does, and
    # a caller who really means every file says so in a word that cannot
    # be typed by accident.
    force_named = set(arguments.case) if arguments.force else set()

    names = list(arguments.case)
    if arguments.cases_file:
        for line in pathlib.Path(arguments.cases_file).read_text().splitlines():
            if not line or line.startswith("#") or line.startswith("case\t"):
                continue
            names.append(line.split("\t")[0])
    if not names:
        parser.error("--case or --cases-file")

    written, kept, failed = 0, 0, 0
    for name in names:
        target = out / f"{name}.json"
        if target.exists() and not (arguments.force_every_case
                                    or name in force_named):
            kept += 1
            continue
        source = corpus / name / "values.json"
        if not source.exists():
            print(f"make-poison: {name}: no values.json", file=sys.stderr)
            failed += 1
            continue
        sample = json.loads(source.read_text())
        idl = corpus / name / "type.idl"
        poisoned, changed = poison_for(
            sample, idl.read_text(encoding="utf-8") if idl.exists() else "")
        if changed == 0:
            # Silence here would be a case whose two samples are the same
            # octets, which passes checks 1 to 3 and fails 4 — and would
            # have looked like a decoder fault rather than a missing
            # poison.
            print(f"make-poison: {name}: nothing to mutate; no poison written",
                  file=sys.stderr)
            failed += 1
            continue
        target.write_text(json.dumps(poisoned, indent=2, ensure_ascii=False) + "\n")
        written += 1
    print(f"make-poison: {written} written, {kept} kept, {failed} refused",
          file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
