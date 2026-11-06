#!/usr/bin/env python3
"""Runs a command with a gRPC bridge daemon up: the Java SDK's
GrpcBridgeClientTest (crates/java-omgdds) reads the daemon's port from
ZERODDS_BRIDGE_PORT and runs its publish/subscribe round trip through DDS
against it.

    python3 tests/integration/with_domains.py 1 -- \\
      python3 tests/integration/with_grpc_bridge.py <command>...

The daemon takes the domain with_domains.py leased (ZERODDS_TEST_DOMAINS) and
the topic crates/java-omgdds/run_grpc_bridge_e2e.sh gives it, TRANSIENT_LOCAL:
the test publishes first and subscribes after, and a Subscribe stream gets a
DataReader of its own when it opens (zerodds-grpc-bridge-1.0 §4.3), which
receives the writer's last sample as a late joiner (DDS 1.4 §2.2.3.4). The
command starts once the daemon reports `"event":"listening"` (an event, read
from its output as it is written, with a deadline), and the daemon stops
after it.
"""
import os
from pathlib import Path
import queue
import socket
import subprocess
import sys
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[2]


def main(argv):
    if len(argv) < 2:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    domain = os.environ.get("ZERODDS_TEST_DOMAINS", "").split(",")[0]
    if not domain:
        print("with_grpc_bridge: ZERODDS_TEST_DOMAINS is empty", file=sys.stderr)
        return 2
    subprocess.run(["cargo", "build", "--locked", "-q", "-p", "zerodds-grpc-bridge",
                    "--bin", "zerodds-grpc-bridged", "--features", "dds-runtime"], cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    config = tempfile.NamedTemporaryFile("w", suffix=".yaml", delete=False)
    config.write(f"""domain: {domain}
grpc:
  bind: "127.0.0.1:{port}"
topics:
  - dds_name: "Demo::Bytes"
    grpc_service: "DemoStream"
    qos:
      reliability: "reliable"
      durability: "transient_local"
""")
    config.close()
    daemon = subprocess.Popen([str(target / "debug" / "zerodds-grpc-bridged"),
                               "--config", config.name],
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    lines = queue.Queue()

    def read():
        for line in daemon.stdout:
            sys.stdout.write("[zerodds-grpc-bridged] " + line)
            lines.put(line)
        lines.put(None)

    threading.Thread(target=read, daemon=True).start()
    try:
        while True:
            line = lines.get(timeout=20)
            if line is None:
                print("with_grpc_bridge: the daemon ended before it listened", file=sys.stderr)
                return 1
            if '"event":"listening"' in line:
                break
        print(f"with_grpc_bridge: zerodds-grpc-bridged on 127.0.0.1:{port}, domain {domain}", flush=True)
        return subprocess.run(argv[1:], env=dict(os.environ, ZERODDS_BRIDGE_PORT=str(port))).returncode
    except queue.Empty:
        print("with_grpc_bridge: no listening event within 20 s", file=sys.stderr)
        return 1
    finally:
        daemon.kill()
        daemon.wait()
        os.unlink(config.name)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
