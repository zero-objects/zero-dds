#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""A minimal client of the board (the host reservation service, an MCP
server over HTTP) for the speed pillar's CI jobs.

The speed pillar measures on an exclusive host (rc8-test-suite.md, decision
5). A CI job cannot measure in its own container; it reserves the speed host
through the board, starts the measurement there with the board's `run`, and
waits for it with `wait`. Every call prints the board's answer.

Environment: BOARD_URL (the board's MCP endpoint, a CI variable),
BOARD_TOKEN (bearer token, a masked CI variable).

  board.py call <tool> '<json arguments>'      one tool call, answer on stdout
  board.py reserve --host H --minutes M --dir D [--ram-gb N]
        login, exclusive request, wait until active; prints
        BOARD_SESSION=... and BOARD_RESERVATION=... (a dotenv for later jobs)
  board.py run --session S --reservation R -- <command...>
        start a process on the host and wait for its end; exit status 1
        unless the board reports exit code 0
  board.py release --session S --reservation R
"""

import argparse
import json
import os
import re
import sys
import time
import urllib.request

URL = os.environ.get("BOARD_URL", "")


class Board:
    def __init__(self):
        if not URL:
            sys.exit("board.py: BOARD_URL is not set (CI variable with the board's MCP endpoint)")
        token = os.environ.get("BOARD_TOKEN")
        if not token:
            sys.exit("board.py: BOARD_TOKEN is not set (masked CI variable with the board's bearer token)")
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "Authorization": f"Bearer {token}",
        }
        self.next_id = 1
        self.session_header = None
        self._rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                                 "clientInfo": {"name": "zerodds-speed-pillar", "version": "1"}})
        self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _post(self, body):
        headers = dict(self.headers)
        if self.session_header:
            headers["Mcp-Session-Id"] = self.session_header
        req = urllib.request.Request(URL, data=json.dumps(body).encode(), headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=120) as resp:
            sid = resp.headers.get("Mcp-Session-Id")
            if sid:
                self.session_header = sid
            raw = resp.read().decode()
            ctype = resp.headers.get("Content-Type", "")
        if not raw.strip():
            return None
        if "text/event-stream" in ctype:
            # The answer is the last `data:` event that carries a result.
            msg = None
            for line in raw.splitlines():
                if line.startswith("data:"):
                    d = json.loads(line[5:].strip())
                    if "result" in d or "error" in d:
                        msg = d
            return msg
        return json.loads(raw)

    def _rpc(self, method, params):
        rid = self.next_id
        self.next_id += 1
        msg = self._post({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        if msg is None:
            raise RuntimeError(f"board: no answer to {method}")
        if "error" in msg:
            raise RuntimeError(f"board: {method}: {msg['error']}")
        return msg["result"]

    def call(self, tool, **arguments):
        res = self._rpc("tools/call", {"name": tool, "arguments": arguments})
        text = "\n".join(c.get("text", "") for c in res.get("content", []) if c.get("type") == "text")
        if res.get("isError"):
            raise RuntimeError(f"board: {tool}: {text}")
        return text


def handle_requests(board, session, text):
    """Lines with AUFFORDERUNG: requests of the board. One to give the host
    back (Freigabe) stops the step: the pillar's jobs are short, they stop
    and report instead of carrying on. Any other (a directory to clean up or
    to register) is printed and the step goes on; pipeline 8732 stopped its
    build on a note about the working directory."""
    requests = [line for line in text.splitlines() if "AUFFORDERUNG:" in line]
    for line in requests:
        print(line)
    if any(re.search(r"frei(gabe|geben)", line, re.IGNORECASE) for line in requests):
        sys.exit("board.py: the board asks to release the host; the measurement stops here")


def cmd_reserve(a):
    b = Board()
    project = os.environ.get("CI_PROJECT_URL", "zerodds")
    if not project.endswith(".git") and project.startswith("http"):
        project += ".git"
    login = b.call("login", project=project, agent="other")
    print(login)
    m = re.search(r"Sitzungskennung:\s*(\S+)", login)
    if not m:
        sys.exit("board.py: no session in the login answer")
    session = m.group(1)
    if a.store:
        # The clone, the cargo target directory and the vendors' apps stay
        # between runs: a project store, not a working directory. Already
        # registered is fine.
        try:
            print(b.call("add_store", session=session, host=a.host, path=a.store))
        except RuntimeError as e:
            print(e)
    kw = dict(session=session, host=a.host, exclusive=True, max_minutes=a.minutes, dirs=[a.dir])
    if a.ram_gb:
        kw["ram_gb"] = a.ram_gb
    text = b.call("request", **kw)
    print(text)
    m = re.search(r"Reservierung (\d+): (\w+)", text)
    if not m:
        sys.exit("board.py: no reservation in the answer")
    res = int(m.group(1))
    deadline = time.time() + a.wait_s
    state = m.group(2)
    while state != "active":
        if time.time() > deadline:
            print(b.call("cancel", session=session, reservation=res))
            print(b.call("logout", session=session))
            sys.exit(f"board.py: {a.host} not free within {a.wait_s} s; request withdrawn")
        text = b.call("wait", session=session, reservation=res)
        handle_requests(b, session, text)
        if re.search(rf"Reservierung {res}: active|\bactive\b", text) and "Wartet noch" not in text:
            state = "active"
        else:
            print(text.splitlines()[0] if text else "")
    with open(a.dotenv, "w") as f:
        f.write(f"BOARD_SESSION={session}\nBOARD_RESERVATION={res}\n")
    print(f"BOARD_SESSION={session}\nBOARD_RESERVATION={res}")


def reservation_of(a):
    """The reservation number, or the end of the step: without one (the
    reserving job failed and its dotenv never came) nothing is measured,
    and the job says so instead of failing on an empty argument."""
    if not str(a.reservation).strip():
        sys.exit(f"{os.environ.get('CI_JOB_NAME', 'board.py')}: no reservation of the speed host came "
                 "about (speed-reserve failed, see its log); nothing measured")
    return int(a.reservation)


def cmd_run(a):
    a.reservation = reservation_of(a)
    b = Board()
    text = b.call("run", session=a.session, reservation=a.reservation, command=a.command)
    print(text)
    m = re.search(r"Job (\d+)", text)
    if not m:
        sys.exit("board.py: no job number in the answer")
    job = int(m.group(1))
    while True:
        text = b.call("wait", session=a.session, job=job)
        handle_requests(b, a.session, text)
        if "Wartet noch" in text:
            continue
        print(text)
        break
    # The answer names the exit code ("Job N: exited, Exit-Code 0"); the
    # process's output stays in the log on the host.
    m = re.search(r"Exit-Code\s*(-?\d+)", text)
    if not m or int(m.group(1)) != 0:
        sys.exit(1)


def cmd_release(a):
    b = Board()
    for tool, kw in (("release", dict(reservation=a.reservation)), ("logout", {})):
        try:
            print(b.call(tool, session=a.session, **kw))
        except RuntimeError as e:
            print(e)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("call")
    c.add_argument("tool")
    c.add_argument("arguments", nargs="?", default="{}")
    r = sub.add_parser("reserve")
    r.add_argument("--host", required=True)
    r.add_argument("--minutes", type=int, default=30)
    r.add_argument("--dir", required=True)
    r.add_argument("--ram-gb", type=float)
    r.add_argument("--wait-s", type=int, default=240)
    r.add_argument("--dotenv", default="board.env")
    r.add_argument("--store", help="project store on the host to register (add_store)")
    u = sub.add_parser("run")
    u.add_argument("--session", required=True)
    u.add_argument("--reservation", required=True)
    u.add_argument("command", nargs="+")
    e = sub.add_parser("release")
    e.add_argument("--session", required=True)
    e.add_argument("--reservation", type=int, required=True)
    a = ap.parse_args()
    if a.cmd == "call":
        print(Board().call(a.tool, **json.loads(a.arguments)))
    elif a.cmd == "reserve":
        cmd_reserve(a)
    elif a.cmd == "run":
        cmd_run(a)
    else:
        cmd_release(a)


if __name__ == "__main__":
    main()
