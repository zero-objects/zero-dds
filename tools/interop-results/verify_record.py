#!/usr/bin/env python3
"""Compare fresh vendor measurements with committed cells, without editing them."""
import argparse
import difflib
import itertools
import json
import os
from pathlib import Path
import re
import subprocess


def batch_build_count(log):
    records = re.findall(r'wireprobe-batch: [a-z0-9-]+: (\d+) batch build\(s\) \+ (\d+) per-case retries\s+for\s+(\d+) cells', log)
    if len(records) != 1:
        raise ValueError('missing or ambiguous vendor batch accounting')
    builds, retries, count = map(int, records[0])
    if builds + retries == 0 or count == 0:
        raise ValueError('empty vendor batch accounting')
    return builds + retries


def cells(path):
    result = {}
    for row in path.read_text().splitlines():
        fields = row.split('\t')
        if len(fields) != 6:
            raise ValueError(f'{path}: malformed cell')
        key = tuple(fields[:4])
        if key in result:
            raise ValueError(f'{path}: duplicate cell {key}')
        result[key] = row
    return result


def compare(committed, fresh, cases, vendors, log):
    expected = set(itertools.product(cases, vendors, ('xcdr1', 'xcdr2'), ('le', 'be')))
    if set(fresh) != expected:
        raise ValueError(f'fresh measurement identity mismatch: {len(expected - set(fresh))} missing, '
                         f'{len(set(fresh) - expected)} unexpected cells')
    if not expected <= set(committed):
        raise ValueError('selected corpus contains cells absent from the committed record')
    accounting = re.findall(r'vendor-corpus-sweep: (\d+) probe builds for\s+(\d+) cells', log)
    if len(accounting) != 1:
        raise ValueError('missing or ambiguous fresh build/cell accounting')
    builds, count = map(int, accounting[0])
    if count != len(expected) or not 0 < builds <= len(cases) * len(vendors):
        raise ValueError(f'invalid fresh accounting: {builds} builds / {count} cells')
    before = [committed[key] + '\n' for key in sorted(expected)]
    after = [fresh[key] + '\n' for key in sorted(expected)]
    return ''.join(difflib.unified_diff(before, after, fromfile='committed', tofile='fresh'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('vendors', nargs='?', default='cyclone,fastdds,opendds')
    parser.add_argument('--cases', help='comma separated names or @file')
    parser.add_argument('--shard-index', type=int, default=1)
    parser.add_argument('--shard-total', type=int, default=1)
    parser.add_argument('--batch-count', type=Path, help='read exact batch/retry accounting, do not run a sweep')
    args = parser.parse_args()
    if args.batch_count:
        print(batch_build_count(args.batch_count.read_text()))
        return 0
    root = Path(__file__).resolve().parents[2]
    corpus = root / 'tests/interop-corpus'
    available = sorted(path.parent.name for path in corpus.glob('*/type.idl'))
    requested = (Path(args.cases[1:]).read_text().split() if args.cases.startswith('@')
                 else args.cases.split(',')) if args.cases else available
    vendors = args.vendors.split(',')
    if not vendors or len(vendors) != len(set(vendors)) or any(not re.fullmatch(r'[a-z0-9-]+', v) for v in vendors):
        parser.error('invalid or duplicate vendor')
    if not requested or len(requested) != len(set(requested)) or not set(requested) <= set(available):
        parser.error('empty, duplicate or unknown case selection')
    if not 1 <= args.shard_index <= args.shard_total <= len(requested):
        parser.error('invalid or empty shard')
    selected = sorted(requested)[args.shard_index - 1::args.shard_total]
    output = Path(os.environ.get('INTEROP_OUT', root / 'interop-out'))
    output.mkdir(parents=True, exist_ok=True)
    fresh = output / 'fresh.tsv'
    if fresh.exists():
        raise RuntimeError('fresh.tsv already exists; refuse stale measurement reuse')
    names = output / 'cases.txt'
    names.write_text('\n'.join(selected) + '\n')
    command = [str(root / 'tools/interop-results/vendor-corpus-sweep.sh'), '--vendors', args.vendors,
               '--jobs', '1', '--cases', '@' + str(names.resolve()), str(fresh.resolve())]
    log_path = output / 'record.log'
    env = dict(os.environ, TOOL_LOGS=str((output / 'toolchain-logs').resolve()))
    with log_path.open('w') as handle:
        status = subprocess.run(command, cwd=root, env=env, stdout=handle, stderr=subprocess.STDOUT).returncode
    # The sweep log is vendor tool output and can carry bytes that are not
    # UTF-8 (a probe echoing raw octets). compare() only reads the ASCII
    # accounting lines from it, so decode with replacement instead of failing
    # the whole record on a stray byte -- and report how many bytes were
    # replaced so a non-UTF-8 log never goes unnoticed.
    raw_log = log_path.read_bytes()
    log = raw_log.decode('utf-8', errors='replace')
    print(log, end='', flush=True)
    result = {'status': 'FAIL', 'cases': selected, 'vendors': vendors,
              'shard_index': args.shard_index, 'shard_total': args.shard_total,
              'source_commit': os.environ.get('CI_COMMIT_SHA'), 'command': command, 'exit_code': status,
              'log_bytes_not_utf8': log.count('\ufffd')}
    try:
        if status:
            raise RuntimeError(f'vendor sweep failed with exit {status}')
        diff = compare(cells(root / 'tests/interop-results/sources/vendor-corpus.tsv'),
                       cells(fresh), selected, vendors, log)
        (output / 'vendor-corpus.diff').write_text(diff)
        if diff:
            raise RuntimeError('fresh vendor cells differ from committed evidence, see vendor-corpus.diff')
        result['status'] = 'PASS'
    except (OSError, ValueError, RuntimeError) as error:
        result['error'] = str(error)
    (output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
    return 0 if result['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
