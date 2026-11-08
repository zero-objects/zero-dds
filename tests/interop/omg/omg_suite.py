#!/usr/bin/env python3
"""Run a part of an OMG interoperability suite and decide the job.

The interop pillar of rc8-test-suite.md runs the OMG `dds-rtps` and
`dds-xtypes` suites through their own harness (`interoperability_report.py`,
pinned in the `ci-omg` image), `dds-security` through
`tools/omg-security-main/run_matrix.py` with a PKI made for the job
(`security_setup.py`), and this script around them:

- it runs one chunk of a suite (`chunks.tsv`) or one named list for one
  publisher and one subscriber, so that a job stays inside the 5-minute
  budget. A chunk is a list of workers separated by ` | `; the workers run
  at the same time, each on DDS domains of its own, so that two pairs never
  share a domain. The cases wait on the harness's fixed durations and
  timeouts, not on the CPU: a case reads 500 samples at the writer's period,
  or waits out the harness's timeout where it expects no communication;
- it reads the JUnit report and fails the job on every failed, errored or
  missing case, except a case the other vendor fails against itself
  (`vendor-self-failures.tsv`, with the self-run that shows it; not run) and a
  case in which the other vendor answers otherwise than the specification
  while ZeroDDS follows it (`vendor-deviations.tsv`, with the clause, the
  evidence outside this setup and the OPEN-ITEMS entry; run, and declared only
  while it fails with the declared codes).

    omg_suite.py --suite dds-rtps --harness /opt/omg/dds-rtps \
        --pub zerodds_shape_main_linux \
        --sub /opt/omg/vendors/dds-rtps/eclipse_cyclone-11.0.1_shape_main_linux \
        --chunk smoke --out omg-out

The product name of an executable is what the harness itself takes: the
file name up to `_shape_main`, `_test_main` or `_security_main`.

`--domain auto` leases, for each worker, as many neighbouring domains from
`base` up as its cases name (`domains_needed`: two for a worker with a
`Test_Domain_*` case, one otherwise), for the job's lifetime, by the lease
protocol of `crates/test-domains` (`test_domains.py`): the lease files of
`$ZERODDS_TEST_DOMAIN_DIR`, the directory every job of a runner host mounts,
so a suite job and a Rust test on the same host never share a domain, and
from the share of the band that `$ZERODDS_TEST_DOMAIN_BAND` names, so two
hosts of the LAN never do either. (A file lock on the runner's /cache volume
did not hold: that volume is one per job slot, and two jobs took the same
domains, pipeline 8489.)

Both executables are started through a wrapper that adds `base` to every `-d`
the suite passes and appends `-d base` where it passes none: the suite names
domains 0 and 1 itself (`Test_Domain_*`) and leaves the default 0 to the rest,
and two pairs on one domain read each other's samples (measured on the Linux
test host, 2026-10-10).
"""
import argparse
import os
import re
import subprocess
import sys
import threading
import time
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
# How each harness names the two executables.
FLAGS = {"dds-rtps": ("-P", "-S"), "dds-xtypes": ("--pub-exe", "--sub-exe"),
         # dds-security has no harness of its own; tools/omg-security-main's
         # run_matrix.py drives it, with --writer and --reader profiles.
         "dds-security": ("--writer", "--reader")}
REPO = HERE.parents[2]
OURS = "zerodds"


def is_ours(name):
    """Whether a product name is ZeroDDS': `zerodds`, or `zerodds-<version>`,
    the form the OMG releases name an executable in. Compared with `OURS`
    alone, a versioned ZeroDDS executable was taken for another vendor: no
    `--ours-args`, and the dds-security profiles of the two sides swapped."""
    return name == OURS or name.startswith(OURS + "-")


WRAPPER = """#!/bin/bash
# omg_suite.py: every -d of the suite shifted by {base}, -d {base} where none.
args=(); have=0; prev=""
for a in "$@"; do
  if [ "$prev" = "-d" ]; then a=$(( a + {base} )); have=1; fi
  args+=("$a"); prev="$a"
done
[ $have = 1 ] || args+=(-d {base})
exec {real} "${{args[@]}}" {extra}
"""


def wrap(out, executable, base, extra=""):
    """A wrapper under the executable's own name, so the harness reads the
    same product name off it."""
    real = os.path.abspath(executable)
    # One directory per base: two runs sharing an output directory must not
    # overwrite each other's wrapper and meet on one domain.
    path = out / f"wrap-{base}" / Path(real).name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(WRAPPER.format(base=base, real=real, extra=extra))
    path.chmod(0o755)
    return str(path)


def product(executable):
    name = Path(executable.split()[0]).name
    for marker in ("_shape_main", "-shape_main", "_test_main", "-test_main", "_security_main"):
        if marker in name:
            return name.split(marker)[0]
    return name


def read_tsv(path):
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        rows.append(line.split("\t"))
    return rows


def workers_of(words):
    """A case list with ` | ` between the workers, as lists of cases."""
    groups, group = [], []
    for word in words:
        if word == "|":
            groups.append(group)
            group = []
        else:
            group.append(word)
    groups.append(group)
    return [group for group in groups if group]


def chunk_tests(suite, chunk):
    """The workers of a chunk, each a list of cases."""
    for row in read_tsv(HERE / "chunks.tsv"):
        if row[0] == suite and row[1] == chunk:
            return workers_of(row[2].split())
    raise SystemExit(f"omg_suite.py: no chunk `{chunk}` of {suite} in chunks.tsv")


def lease_order(spans):
    """The order the workers lease their domains in: the most neighbouring
    domains first. Leased in the order of the workers, the one-domain workers
    took the band from the top down and left no two neighbours for a worker
    with a `Test_Domain_*` case on a host whose share has few of them
    (measured on the Linux test host, whose share is 90, 92 and 215-218: four
    workers, "No 2 neighbouring DDS domains left ... after 180 s")."""
    return sorted(range(len(spans)), key=lambda index: -spans[index])


def domains_needed(suite, harness, tests):
    """How many neighbouring domains, from the leased base up, these cases
    use: one more than the highest `-d` a case of test_suite.py passes (the
    wrapper shifts every `-d` by the base). dds-security runs on one domain."""
    if suite == "dds-security":
        return 1
    import ast
    source = (harness / "test_suite.py").read_text(encoding="utf-8")
    wanted, highest = set(tests), 0
    for node in ast.parse(source).body:
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Dict):
            for key, value in zip(node.value.keys, node.value.values):
                if isinstance(key, ast.Constant) and key.value in wanted:
                    for domain in re.findall(r"-d (\d+)", ast.get_source_segment(source, value) or ""):
                        highest = max(highest, int(domain))
    return highest + 1


def vendor_failures(suite):
    """(vendor, test) -> (role, the evidence that the vendor fails it alone)."""
    found = {}
    for row in read_tsv(HERE / "vendor-self-failures.tsv"):
        if row[0] == suite:
            found[(row[1], row[2])] = (row[3], row[4])
    return found


def suite_names(harness):
    """The harness's report name of every case: `<dictionary>_<case>`, as
    `interoperability_report.py` writes it, mapped to the case."""
    import ast
    names = {}
    tree = ast.parse((harness / "test_suite.py").read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Dict):
            for key in node.value.keys:
                if isinstance(key, ast.Constant):
                    names[f"{node.targets[0].id}_{key.value}"] = key.value
    return names


def suite_dictionaries(harness):
    """case -> the dictionary of test_suite.py that holds it. The dds-xtypes
    harness runs a `-t` list only when one dictionary holds all of it."""
    return {case: full[: -len(case) - 1] for full, case in suite_names(harness).items()}


def vendor_deviations(suite, extra=""):
    """(vendor, test, direction) -> the row of vendor-deviations.tsv: the
    clause, the codes the case must produce, the evidence, the OPEN-ITEMS
    entry. A row whose ninth column names harness arguments (`-x 1`) holds
    only for a run whose `--extra` carries them: the dds-xtypes harness runs
    XCDR2 unless told otherwise, and a deviation under XCDR1 alone would read
    as a stale row there."""
    found = {}
    given = " ".join(extra.split())
    for row in read_tsv(HERE / "vendor-deviations.tsv"):
        harness = " ".join(row[8].split()) if len(row) > 8 else ""
        if row[0] == suite and (not harness or f" {harness} " in f" {given} "):
            found[(row[3], row[1], row[2])] = {"clause": row[4], "codes": set(row[5].split()),
                                               "evidence": row[6], "item": row[7]}
    return found


# The harness's table in a failure: <th> who </th> <th> expected </th> <th> produced </th>.
PRODUCED = re.compile(r"<th>\s*((?:Publisher|Subscriber)_\d+)\s*</th>\s*<th>[^<]*</th>"
                      r"\s*<th>\s*([A-Z_]+)\s*</th>")


# What XML 1.0 does not allow in a document (§2.2 Char).
NOT_XML = re.compile(rb"[\x00-\x08\x0b\x0c\x0e-\x1f]")


def read_report(path):
    """The root of a JUnit report, with the octets XML 1.0 does not allow
    taken out first. The harness copies what the executables print into its
    failure messages, and zzdds 0.3.2 prints an interface name followed by NUL
    octets ("multicast join on eth0\\0\\0...: error.SetsockoptFailed");
    the report it wrote did not parse, and the run ended without a verdict."""
    return ET.fromstring(NOT_XML.sub(b"", path.read_bytes()))


def junit_cases(path, names):
    """case -> (verdict, the first line of the message, seconds, the codes
    produced as `Publisher_1=CODE`)."""
    cases = {}
    if not path.is_file() or path.stat().st_size == 0:
        return cases
    for case in read_report(path).iter("testcase"):
        verdict, detail, codes = "passed", "", set()
        for tag in ("failure", "error", "skipped"):
            node = case.find(tag)
            if node is not None:
                verdict = tag
                text = (node.get("message") or "") + " " + (node.text or "")
                codes = {f"{who}={code}" for who, code in PRODUCED.findall(text)}
                detail = (node.get("message") or node.text or "").strip().splitlines()[:1]
                detail = detail[0] if detail else ""
                break
        name = case.get("name") or ""
        cases[names.get(name, name)] = (verdict, detail, case.get("time"), codes)
    return cases


def classify(verdict, detail, codes, self_failure, deviation):
    """The mark of a case and why: `ok`, `declared` (the other vendor fails it
    against itself, or its own manifest skips it), `deviation` (it fails as
    vendor-deviations.tsv declares), or `RED`."""
    if deviation is not None:
        where = f"vendor-deviations.tsv, OPEN-ITEMS {deviation['item']}"
        if verdict == "passed":
            return "RED", f"declared as a deviation ({where}) and passed: the list is stale"
        if verdict == "failure" and codes == deviation["codes"]:
            return "deviation", f"{deviation['clause']}; {deviation['evidence']} ({where})"
        return "RED", (f"declared as a deviation ({where}) with {' '.join(sorted(deviation['codes']))}, "
                       f"{verdict} with {' '.join(sorted(codes)) or detail}")
    if verdict == "passed":
        return "ok", ""
    if verdict == "skipped":
        # The harness skips only what a vendor's own manifest
        # (unsupported_test_manifest/ of dds-xtypes) lists.
        return "declared", f"the harness skipped it: {detail}"
    if self_failure:
        return "declared", self_failure
    return "RED", detail


def default_signals():
    """SIGINT and SIGQUIT handled, so that the harness and its clients start
    with them at their defaults. A process started with `&` from a script, or
    under /usr/bin/time, has SIGINT ignored, its children inherit that, and a
    client without a handler of its own (Cyclone's shape_main) then outlives
    the harness's Ctrl-C until the harness kills it 30 s later, in every case
    (measured on the Linux test host, 2026-11-01). Called before any thread
    starts."""
    import signal
    signal.signal(signal.SIGINT, signal.default_int_handler)
    signal.signal(signal.SIGQUIT, signal.SIG_DFL)


def worker_commands(args, tests, base, tag, pub, sub):
    """The commands of one worker on the domains from `base` up, its report
    and the directory they run in."""
    pub_flag, sub_flag = FLAGS[args.suite]
    junit = args.out / f"{args.suite}-{pub}--{sub}-{tag}.xml"
    junit.unlink(missing_ok=True)
    if args.suite == "dds-security":
        import security_setup
        vendor_exe = args.sub if is_ours(pub) else args.pub
        ours_exe = args.pub if is_ours(pub) else args.sub
        ours_profile, vendor_profile = security_setup.setup(
            args.harness, args.out / f"pki-{base}", base, ours_exe, vendor_exe)
        command = [args.python, str(REPO / "tools/omg-security-main/run_matrix.py"),
                   pub_flag, str(ours_profile if is_ours(pub) else vendor_profile),
                   sub_flag, str(ours_profile if is_ours(sub) else vendor_profile),
                   *args.extra.split(), "--cells",
                   *[test.replace("__R-", ":").replace("__W-", ":") for test in tests],
                   "-o", str(junit), "--outdir", str(args.out / f"logs-{base}")]
        return [command], junit, args.out
    # One harness run per dictionary of test_suite.py, all appending to one
    # report (the harness adds to an existing -o file).
    dictionary = suite_dictionaries(args.harness)
    groups = {}
    for test in tests:
        groups.setdefault(dictionary.get(test, ""), []).append(test)
    commands = [[args.python, "interoperability_report.py",
                 pub_flag, wrap(args.out, args.pub, base, args.ours_args if is_ours(pub) else ""),
                 sub_flag, wrap(args.out, args.sub, base, args.ours_args if is_ours(sub) else ""),
                 *args.extra.split(), "-t", *group, "-o", str(junit)]
                for group in groups.values()]
    return commands, junit, args.harness


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--suite", choices=sorted(FLAGS), required=True)
    parser.add_argument("--harness", type=Path, required=True,
                        help="the suite checkout holding interoperability_report.py")
    parser.add_argument("--pub", required=True)
    parser.add_argument("--sub", required=True)
    parser.add_argument("--domain", default="auto",
                        help="`auto` (leases, see above) or the first domain of the workers, "
                             "which then take neighbouring domains one after another")
    which = parser.add_mutually_exclusive_group(required=True)
    which.add_argument("--chunk")
    which.add_argument("--tests", nargs="+",
                       help="cases, with `|` between the workers")
    parser.add_argument("--extra", default="",
                        help="further harness arguments, e.g. `-x 1`")
    parser.add_argument("--ours-args", default="",
                        help="arguments for ZeroDDS' client only, e.g. the dds-xtypes axis "
                             "settings the suite's expectations assume")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--python", default=sys.executable)
    args = parser.parse_args()
    # The harness runs in its own directory: every path handed to it is absolute.
    args.out = args.out.resolve()
    default_signals()

    workers = chunk_tests(args.suite, args.chunk) if args.chunk else workers_of(args.tests)
    label = args.chunk or "tests"
    pub, sub = product(args.pub), product(args.sub)
    other = {name for name in (pub, sub) if not is_ours(name)}
    known = vendor_failures(args.suite)
    roles = {vendor: role for vendor, role in ((pub, "pub"), (sub, "sub")) if not is_ours(vendor)}
    # A case the other vendor fails against itself, in the role it has here,
    # is not run: its result says nothing about this pair, and its timeouts
    # are most of a run.
    every = [test for group in workers for test in group]
    not_run = {test: known[(vendor, test)][1] for test in every for vendor in sorted(other)
               if (vendor, test) in known and known[(vendor, test)][0] in ("both", roles.get(vendor))}
    workers = [group for group in ([t for t in group if t not in not_run] for group in workers)
               if group]
    tests = [test for group in workers for test in group]
    args.out.mkdir(parents=True, exist_ok=True)

    spans = [domains_needed(args.suite, args.harness, group) for group in workers]
    bases = [None] * len(workers)
    names = ({test: test for test in tests} if args.suite == "dds-security"
             else suite_names(args.harness))
    one = len(workers) == 1
    results = [None] * len(workers)

    def work(index):
        tag = label if one else f"{label}-w{index + 1}"
        began = time.monotonic()
        commands, junit, cwd = worker_commands(args, workers[index], bases[index], tag, pub, sub)
        log = args.out / f"{args.suite}-{pub}--{sub}-{tag}.log"
        code = 0
        with log.open("w", encoding="utf-8") as handle:
            for command in commands:
                ran = subprocess.run(command, cwd=cwd, stdout=handle,
                                     stderr=subprocess.STDOUT, check=False)
                code = code or ran.returncode
        results[index] = (junit, log, code, time.monotonic() - began)

    # Each worker starts as soon as its domains are in hand: leased one
    # worker after another in this thread (a lease waits while the host's
    # share is taken), or counted up from the given base.
    started = time.monotonic()
    threads = []
    base = None if args.domain == "auto" else int(args.domain)
    for index in lease_order(spans):
        span = spans[index]
        if base is None:
            import test_domains
            bases[index] = test_domains.lease_run(span)
        else:
            bases[index] = base
            base += span
        thread = threading.Thread(target=work, args=(index,))
        thread.start()
        threads.append(thread)
    for thread in threads:
        thread.join()
    took = time.monotonic() - started

    cases, logs, exits = {}, {}, []
    for index, result in enumerate(results):
        if result is None:
            continue
        junit, log, code, seconds = result
        exits.append(code)
        found = junit_cases(junit, names)
        cases.update(found)
        for test in workers[index]:
            logs[test] = log
        span = spans[index]
        domains = f"{bases[index]}" if span == 1 else f"{bases[index]}-{bases[index] + span - 1}"
        print(f"worker {index + 1}: domain {domains}, {len(workers[index])} cases, "
              f"{seconds:.0f} s, harness exit {code}")
    deviations = vendor_deviations(args.suite, args.extra)
    direction = "zerodds->vendor" if is_ours(pub) else "vendor->zerodds"
    unexpected, declared, deviating = [], [], []
    for test in tests:
        verdict, detail, seconds, codes = cases.get(
            test, ("missing", "no testcase in the report", None, set()))
        evidence = [known[(vendor, test)][1] for vendor in sorted(other)
                    if (vendor, test) in known
                    and known[(vendor, test)][0] in ("both", roles.get(vendor))]
        deviation = next((deviations[(vendor, test, direction)] for vendor in sorted(other)
                          if (vendor, test, direction) in deviations), None)
        mark, why = classify(verdict, detail, codes, evidence[0] if evidence else "", deviation)
        {"declared": declared, "deviation": deviating, "RED": unexpected}.get(mark, []).append(
            (test, verdict, why))
        print(f"{mark:9} {test:32} {verdict:8} {seconds or '-':>10}  {why if mark != 'ok' else ''}")
    for test, evidence in sorted(not_run.items()):
        print(f"{'not run':9} {test:32} {'declared':8} {'-':>10}  {evidence}")
    passed = len(tests) - len(unexpected) - len(declared) - len(deviating)
    print(f"{args.suite} {pub} -> {sub}, {len(workers)} workers, {label}: {len(tests)} cases run, "
          f"{passed} passed, {len(declared)} failed and declared, {len(deviating)} declared "
          f"deviations, {len(unexpected)} red, {len(not_run)} not run (declared); harness exit "
          f"{max(exits, default=0)}, {took:.0f} s")
    for test, verdict, evidence in declared:
        print(f"  declared {test} ({verdict}): {evidence}")
    for test, verdict, evidence in deviating:
        print(f"  deviation {test} ({verdict}): {evidence}")
    if unexpected:
        for log in sorted({str(logs[test]) for test, _, _ in unexpected if test in logs}):
            print(f"  harness log: {log}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
