#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""`<topic> <PID_KEY_HASH>` for every application writer in a capture.

Reads `tshark -V` text: per DATA submessage of an application-defined
writer, the topic tshark resolves from discovery (`[topic: …]`) and the
sixteen octets of PID_KEY_HASH, which tshark prints as a GUID. One line
per distinct (topic, hash) seen at least MIN_SEEN times; a topic whose DATA
carry no PID_KEY_HASH is printed with `none`.

tshark resolves the topic from the writer's entity id alone, and the
reading participant's own internal writers reuse those ids, so a stray
DATA can be labelled with a probe topic. The probe's writer sends thirty
samples per topic; a stray row appears once or twice."""
import re
import subprocess
import sys

text = subprocess.run(["tshark", "-r", sys.argv[1], "-V", "-Y", "rtps.sm.id == 0x15"],
                      capture_output=True, text=True, check=True).stdout
MIN_SEEN = 5
seen = {}
for sm in re.split(r"\n\s*submessageId: DATA \(0x15\)", text)[1:]:
    if not re.search(r"writerEntityId: 0x[0-9a-f]{8} \(Application-defined", sm):
        continue
    topic = re.search(r"\[topic: ([^\]]+)\]", sm)
    if not topic:
        continue
    kh = re.search(r"PID_KEY_HASH \(0x0070\)\n\s*parameterLength: 16\n\s*\w+: ([0-9a-f:]+)", sm)
    row = (topic.group(1), kh.group(1).replace(":", "") if kh else "none")
    seen[row] = seen.get(row, 0) + 1
for (topic, kh), count in seen.items():
    if count >= MIN_SEEN:
        print(topic, kh)
