#!/usr/bin/env python3
"""`vendor-self-failures.tsv` rows from a vendor's self-run report.

    self_failures.py --suite dds-rtps --harness <suite checkout> \
        --label "self-run 2026-10-31, harness 863b86f97ce" <JUnit report>...

Each report is one vendor against itself (publisher and subscriber the same
executable). Every failed or errored case becomes a row, with the codes the
harness recorded as produced, so the declaration says what the vendor did and
not only that it failed.
"""
import argparse
import re
import xml.etree.ElementTree as ET
from pathlib import Path

from omg_suite import suite_names


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--suite", required=True)
    parser.add_argument("--harness", type=Path)
    parser.add_argument("--product", help="the product, if the report does not name it")
    parser.add_argument("--label", required=True, help="where and when the self-run was taken")
    parser.add_argument("reports", type=Path, nargs="+")
    args = parser.parse_args()
    names = suite_names(args.harness) if args.harness else {}
    for report in args.reports:
        root = ET.parse(report).getroot()
        suite_node = root if root.tag == "testsuite" else next(root.iter("testsuite"))
        product = args.product or (suite_node.get("name") or "").split("---")[0]
        product = product.split("-reader")[0]
        for case in root.iter("testcase"):
            node = case.find("failure")
            if node is None:
                node = case.find("error")
            if node is None:
                continue
            text = (node.get("message") or "") + " " + (node.text or "")
            # The harness's table: <th> who </th> <th> expected </th> <th> produced </th>.
            produced = sorted(set(re.findall(
                r"<th>\s*(?:Publisher|Subscriber)_\d+\s*</th>\s*<th>[^<]*</th>\s*<th>\s*([A-Z_]+)\s*</th>",
                text)))
            name = case.get("name") or ""
            codes = set(produced)
            case_name = names.get(name, name)
            # `sub` only for a feature of the DataReader alone (a content
            # filtered topic, DDS 1.4 2.2.2.3.3; TIME_BASED_FILTER, 2.2.3.12)
            # that the vendor's subscriber refused: its publisher then fails
            # only because no reader came up. Everything else, PRESENTATION
            # among it (set on Publisher and Subscriber), is `both`.
            reader_only = case_name.startswith(("Test_Cft_", "Test_TimeBasedFilter_"))
            if reader_only and codes <= {"SUB_UNSUPPORTED_FEATURE", "READER_NOT_MATCHED"}:
                role = "sub"
            else:
                role = "both"
            print("\t".join([args.suite, product, case_name, role,
                             f"fails against itself, {args.label}: "
                             + (", ".join(produced) if produced else (node.get("message") or "")[:80])]))


if __name__ == "__main__":
    main()
