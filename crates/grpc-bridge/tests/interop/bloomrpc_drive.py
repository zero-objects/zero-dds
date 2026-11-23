#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Drives BloomRPC, the gRPC GUI client, against zerodds-grpc-bridged for
crates/grpc-bridge/tests/cross_vendor.rs (zerodds-grpc-bridge-1.0 §10).

    bloomrpc_drive.py BLOOMRPC ADDRESS PROTO_DIR PAYLOAD_BASE64

BloomRPC starts under Xvfb (xvfb-run) with a home directory of its own,
whose stored editor state is what BloomRPC writes itself when a user imports
PROTO_DIR/zerodds/chat/v1/bridge.proto and opens the methods PublishOne
(request `{"cdr_payload": PAYLOAD_BASE64}`) and Catalog (`{}`) with the
address ADDRESS: BloomRPC loads the .proto with its own loader and opens the
two tabs. Over the Chrome DevTools Protocol of its window the script presses
the play button of each tab, as a user clicks it, and prints the text of the
response pane BloomRPC shows, one line per call:

    publish-one {"sequence_number": "1", ...}
    catalog {"topics": [...]}

The response is the JSON BloomRPC renders from the decoded answer, or
`{"error": ...}` for a status other than OK. Needs xvfb-run (xvfb, xauth)
and Python websockets.
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

from websockets.sync.client import connect

SERVICE = "zerodds.chat.v1.ChatMessageStream"
# Electron keeps the stored state under ~/.config/<productName>; BloomRPC
# 1.5.3's package.json names the product "bloom-rpc-client".
APP_DIR = "bloom-rpc-client"


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def seed(home, address, proto_dir, payload):
    proto = os.path.join(proto_dir, "zerodds", "chat", "v1", "bridge.proto")
    tabs = [{"protoPath": proto, "methodName": m, "serviceName": SERVICE, "tabKey": SERVICE + m}
            for m in ("PublishOne", "Catalog")]
    requests = [
        {"id": SERVICE + "PublishOne", "url": address, "data": json.dumps({"cdr_payload": payload}),
         "metadata": "", "interactive": False, "grpcWeb": False},
        {"id": SERVICE + "Catalog", "url": address, "data": "{}", "metadata": "",
         "interactive": False, "grpcWeb": False},
    ]
    store = os.path.join(home, ".config", APP_DIR)
    os.makedirs(store)
    with open(os.path.join(store, "editor.json"), "w") as f:
        json.dump({"url": address, "protos": [proto],
                   "tabs": {"activeKey": tabs[0]["tabKey"], "tabs": tabs},
                   "requests": requests}, f)
    with open(os.path.join(store, "importPaths.json"), "w") as f:
        json.dump({"paths": [proto_dir]}, f)


class Page:
    """Runtime.evaluate over the DevTools websocket of BloomRPC's window."""

    def __init__(self, ws):
        self.ws = ws
        self.next_id = 0

    def eval(self, expression):
        self.next_id += 1
        self.ws.send(json.dumps({"id": self.next_id, "method": "Runtime.evaluate",
                                 "params": {"expression": expression, "returnByValue": True}}))
        while True:
            message = json.loads(self.ws.recv(timeout=10))
            if message.get("id") == self.next_id:
                result = message["result"]
                if "exceptionDetails" in result:
                    raise RuntimeError(json.dumps(result["exceptionDetails"])[:400])
                return result["result"].get("value")

    def until(self, expression, what, seconds=15):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            value = self.eval(expression)
            if value:
                return value
            time.sleep(0.1)
        raise RuntimeError(f"BloomRPC: no {what} within {seconds} s; page: "
                           + str(self.eval("document.body.innerText.slice(0, 600)")))


# The pane of the active tab of the tab list (TabList.tsx, class
# `draggable-tabs`); the response tabs inside each pane have active panes of
# their own.
ACTIVE = "document.querySelector('.draggable-tabs > .ant-tabs-content > .ant-tabs-tabpane-active')"


def call(page, title, label):
    """Activates the tab `title`, presses its play button, prints the text
    of its response pane."""
    page.until(f"""(() => {{
        const tab = [...document.querySelectorAll('.draggable-tabs .ant-tabs-tab')]
            .find(t => t.innerText.trim() === {json.dumps(title)});
        if (!tab) return false;
        tab.click();
        return true; }})()""", f"tab {title}")
    page.until(f"""(() => {{
        const pane = {ACTIVE};
        if (!pane) return false;
        const play = pane.querySelector('.anticon-play-circle');
        if (!play) return false;
        play.click();
        return true; }})()""", f"play button of {title}")
    output = page.until(f"""(() => {{
        const pane = {ACTIVE};
        const viewer = pane && pane.querySelector('.response-edit');
        return viewer && viewer.env && viewer.env.editor.getValue(); }})()""",
                        f"response of {title}")
    print(label, json.dumps(json.loads(output), separators=(",", ":")), flush=True)


def main(argv):
    bloomrpc, address, proto_dir, payload = argv[1:5]
    home = tempfile.mkdtemp(prefix="zd-bloomrpc-")
    port = free_port()
    seed(home, address, proto_dir, payload)
    env = dict(os.environ, HOME=home, XDG_CONFIG_HOME=os.path.join(home, ".config"))
    process = subprocess.Popen(
        ["xvfb-run", "-a", "-s", "-screen 0 1280x900x24", bloomrpc, "--no-sandbox",
         f"--remote-debugging-port={port}"],
        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    try:
        deadline = time.monotonic() + 15
        target = None
        while target is None:
            if time.monotonic() > deadline or process.poll() is not None:
                raise RuntimeError("BloomRPC did not open its DevTools endpoint")
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=1) as r:
                    target = next((t for t in json.load(r) if t.get("type") == "page"
                                   and "app.html" in t.get("url", "")), None)
            except OSError:
                pass
            if target is None:
                time.sleep(0.2)
        with connect(target["webSocketDebuggerUrl"], max_size=None, open_timeout=10) as ws:
            page = Page(ws)
            call(page, SERVICE + ".PublishOne", "publish-one")
            call(page, SERVICE + ".Catalog", "catalog")
    finally:
        try:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=5)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            os.killpg(process.pid, signal.SIGKILL)
        shutil.rmtree(home, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
